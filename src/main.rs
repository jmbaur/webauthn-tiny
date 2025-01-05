mod app;
mod handlers;
mod session;

use app::App;
use axum::{
    middleware,
    routing::{delete, get},
    Extension, Router,
};
use clap::Parser;
use handlers::{
    allow_only_localhost, authenticate_end_handler, authenticate_start_handler,
    delete_credentials_api_handler, get_authenticate_template_handler,
    get_credentials_template_handler, register_end_handler, register_start_handler,
    require_logged_in, root_handler, Templates,
};
use metrics::counter;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::{collections::HashMap, env, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tokio_rusqlite::Connection;
use tower_http::trace::TraceLayer;
use tower_sessions::SessionManagerLayer;
use tracing::debug;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use webauthn_rs::{prelude::Url, WebauthnBuilder};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)] // Read from `Cargo.toml`
struct Cli {
    #[clap(
        env,
        long,
        value_parser,
        help = "Address to bind on",
        default_value = "[::]:8080"
    )]
    address: SocketAddr,
    #[clap(env, long, value_parser, help = "Relying Party ID")]
    rp_id: String,
    #[clap(env, long, value_parser, help = "Relying Party origin")]
    rp_origin: String,
    #[clap(env, long, value_parser, help = "Extra allowed origin")]
    extra_allowed_origin: Vec<String>,
    #[clap(env, long, value_parser, help = "Session secret file")]
    session_secret_file: PathBuf,
    #[clap(env, long, value_parser, help = "Password file")]
    password_file: PathBuf,
    #[clap(
        env,
        long,
        value_parser,
        help = "Directory to store program state",
        default_value = "/var/lib/webauthn-tiny"
    )]
    state_directory: PathBuf,
}

fn read_password_file(filepath: PathBuf) -> anyhow::Result<HashMap<String, String>> {
    Ok(std::fs::read_to_string(filepath)?
        .lines()
        .fold(HashMap::new(), |mut acc, cur| {
            if let Some((username, hash)) = cur.split_once(':') {
                acc.insert(String::from(username), String::from(hash));
            }
            acc
        }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("WEBAUTHN_TINY_LOG"))
        .init();

    let prometheus_handle = PrometheusBuilder::new().install_recorder()?;

    counter!("successful_registrations").absolute(0);
    counter!("failed_registrations").absolute(0);
    counter!("successful_authentications").absolute(0);
    counter!("failed_authentications").absolute(0);
    counter!("authorized_requests").absolute(0);
    counter!("unauthorized_requests").absolute(0);

    let cli = Cli::parse();
    let origin_url = Url::parse(&cli.rp_origin)?;
    let mut builder = WebauthnBuilder::new(&cli.rp_id, &origin_url)?.allow_subdomains(true);
    for url in cli.extra_allowed_origin {
        builder = builder.append_allowed_origin(&Url::parse(&url)?);
    }
    let webauthn = builder.build()?;

    let mut db_path = cli.state_directory;
    db_path.push("webauthn-tiny.db");
    let db = Connection::open(db_path).await?;

    let store = session::SqliteSessionStore::new(db.clone());
    store.init().await?;

    // TODO(jared): std::fs::read_to_string(cli.session_secret_file)?.as_bytes(),
    let session_layer = SessionManagerLayer::new(store)
        .with_always_save(false)
        .with_domain(cli.rp_id);

    let app = App::new(db);
    app.init().await?;

    let parser = liquid::ParserBuilder::with_stdlib().build()?;
    let templates = Templates {
        credentials_template: parser.parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/templates/credentials.liquid"
        )))?,
        authenticate_template: parser.parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/templates/authenticate.liquid"
        )))?,
    };

    let router = Router::new()
        .route(
            "/metrics",
            get(
                |prom_handle: Extension<Arc<PrometheusHandle>>| async move { prom_handle.render() },
            )
            .layer(middleware::from_fn(allow_only_localhost)),
        )
        .route(
            "/api/validate",
            get(|| async {}).layer(middleware::from_fn(require_logged_in)),
        )
        .route(
            "/api/register",
            get(register_start_handler)
                .post(register_end_handler)
                .layer(middleware::from_fn(require_logged_in)),
        )
        .route(
            "/api/authenticate",
            get(authenticate_start_handler).post(authenticate_end_handler),
        )
        .route(
            "/api/credentials/{cred_id}",
            delete(delete_credentials_api_handler).layer(middleware::from_fn(require_logged_in)),
        )
        .route("/authenticate", get(get_authenticate_template_handler))
        .route("/credentials", get(get_credentials_template_handler))
        .fallback(root_handler)
        .layer(TraceLayer::new_for_http())
        .layer(session_layer)
        .layer(Extension(Arc::new(RwLock::new(app))))
        .layer(Extension(Arc::new(webauthn)))
        .layer(Extension(Arc::new(templates)))
        .layer(Extension(Arc::new(prometheus_handle)))
        .layer(Extension(read_password_file(cli.password_file)?))
        .into_make_service_with_connect_info::<SocketAddr>();

    debug!("listening on {}", cli.address);

    let listener = tokio::net::TcpListener::bind(&cli.address).await?;

    axum::serve(listener, router).await?;

    Ok(())
}
