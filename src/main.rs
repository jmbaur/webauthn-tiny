mod app;
mod handlers;
mod session;

use app::App;
use axum::{
    middleware,
    routing::{delete, get},
    Extension, Router, Server,
};
use axum_sessions::SessionLayer;
use clap::Parser;
use handlers::{
    authenticate_end_handler, authenticate_start_handler, delete_credentials_api_handler,
    get_authenticate_template_handler, get_credentials_template_handler, register_end_handler,
    register_start_handler, RequireLoggedIn,
};
use std::{env, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tokio_rusqlite::Connection;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use webauthn_rs::{prelude::Url, WebauthnBuilder};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)] // Read from `Cargo.toml`
struct Cli {
    #[clap(env, long, value_parser, help= "Address to bind on", default_value_t = ("[::]:8080").parse().expect("invalid address"))]
    address: SocketAddr,
    #[clap(env, long, value_parser, help = "Relying Party ID")]
    rp_id: String,
    #[clap(env, long, value_parser, help = "Relying Party origin")]
    rp_origin: String,
    #[clap(env, long, value_parser, help = "Extra allowed origins")]
    extra_allowed_origins: Vec<String>,
    #[clap(env, long, value_parser, help = "Session secret")]
    session_secret: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("WEBAUTHN_TINY_LOG"))
        .init();

    let cli = Cli::parse();
    let origin_url = Url::parse(&cli.rp_origin)?;
    let mut builder = WebauthnBuilder::new(&cli.rp_id, &origin_url)?.allow_subdomains(true);
    for url in cli.extra_allowed_origins {
        builder = builder.append_allowed_origin(&Url::parse(&url)?);
    }
    let webauthn = builder.build()?;

    let state_dir = env::var("STATE_DIRECTORY")?;
    let mut db_path = PathBuf::from(state_dir);
    db_path.push("webauthn-tiny.db");
    let db = Connection::open(db_path).await?;

    let store = session::SqliteSessionStore::new(db.clone());
    store.init().await?;
    let session_layer = SessionLayer::new(store, cli.session_secret.as_bytes())
        .with_cookie_domain(cli.rp_id.clone());

    let app = App::new(db, cli.rp_id, cli.rp_origin);
    app.init().await?;

    let require_logged_in = middleware::from_extractor::<RequireLoggedIn>();

    let parser = liquid::ParserBuilder::with_stdlib().build()?;

    let router = Router::new()
        .route(
            "/api/register",
            get(register_start_handler)
                .post(register_end_handler)
                .layer(require_logged_in.clone()),
        )
        .route(
            "/api/credentials/:cred_id",
            delete(delete_credentials_api_handler).layer(require_logged_in.clone()),
        )
        .route(
            "/api/authenticate",
            get(authenticate_start_handler).post(authenticate_end_handler),
        )
        .route(
            "/validate",
            get(|| async {}).layer(require_logged_in.clone()),
        )
        .route("/authenticate", get(get_authenticate_template_handler))
        .route(
            "/credentials",
            get(get_credentials_template_handler).layer(require_logged_in),
        )
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(session_layer)
                .layer(Extension(Arc::new(RwLock::new(app))))
                .layer(Extension(Arc::new(webauthn)))
                .layer(Extension(Arc::new(parser))),
        );

    tracing::debug!("listening on {}", cli.address);
    Ok(Server::bind(&cli.address)
        .serve(router.into_make_service())
        .await?)
}
