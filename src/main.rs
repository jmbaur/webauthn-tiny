mod app;
mod handlers;

use app::App;
use axum::{
    middleware,
    routing::{delete, get},
    Extension, Router, Server,
};
use axum_sessions::{async_session::MemoryStore, SessionLayer};
use clap::Parser;
use handlers::{
    authenticate_end_handler, authenticate_start_handler, delete_credentials_handler,
    get_credentials_handler, register_end_handler, register_start_handler, RequireLoggedIn,
};
use rand_core::{OsRng, RngCore};
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
    #[clap(short, long, value_parser, help= "Address to bind on", default_value_t = ("[::]:8080").parse().expect("invalid address"))]
    address: SocketAddr,
    #[clap(short, long, value_parser, help = "Relying Party ID")]
    id: String,
    #[clap(short, long, value_parser, help = "Relying Party origin")]
    origin: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_env("WEBAUTHN_TINY_LOG"))
        .init();

    let cli = Cli::parse();
    let origin_url = Url::parse(&cli.origin)?;
    let webauthn = WebauthnBuilder::new(&cli.id, &origin_url)?
        .allow_subdomains(true)
        .build()?;

    let store = MemoryStore::new();
    let mut secret = [0u8; 64];
    OsRng.fill_bytes(&mut secret);
    let session_layer = SessionLayer::new(store, &secret).with_cookie_domain(cli.id.clone());

    let state_dir = env::var("STATE_DIRECTORY")?;
    let mut db_path = PathBuf::from(state_dir);
    db_path.push("webauthn-tiny.db");
    let db = Connection::open(db_path).await?;

    let app = App::new(db, cli.id, cli.origin);
    app.init().await?;

    let require_logged_in = middleware::from_extractor::<RequireLoggedIn>();

    let router = Router::new()
        .route(
            "/api/validate",
            get(|| async {
                // returns empty 200 as long as the middleware passes
            })
            .layer(require_logged_in.clone()),
        )
        .route(
            "/api/credentials",
            get(get_credentials_handler).layer(require_logged_in.clone()),
        )
        .route(
            "/api/credentials/:name",
            delete(delete_credentials_handler).layer(require_logged_in.clone()),
        )
        .route(
            "/api/register",
            get(register_start_handler)
                .post(register_end_handler)
                .layer(require_logged_in),
        )
        .route(
            "/api/authenticate",
            get(authenticate_start_handler).post(authenticate_end_handler),
        )
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(session_layer)
                .layer(Extension(Arc::new(RwLock::new(app))))
                .layer(Extension(Arc::new(webauthn))),
        );

    tracing::debug!("listening on {}", cli.address);
    Ok(Server::bind(&cli.address)
        .serve(router.into_make_service())
        .await?)
}
