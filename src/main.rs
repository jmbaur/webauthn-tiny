mod app;
mod handlers;

use app::App;
use axum::{
    routing::{delete, get},
    Extension, Router, Server,
};
use axum_sessions::{async_session::MemoryStore, SessionLayer};
use clap::Parser;
use handlers::{
    authenticate_end_handler, authenticate_start_handler, delete_credentials_handler,
    get_credentials_handler, register_end_handler, register_start_handler, validate_handler,
};
use rand_core::{OsRng, RngCore};
use std::{env, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tokio_rusqlite::Connection;
use webauthn_rs::{prelude::Url, WebauthnBuilder};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)] // Read from `Cargo.toml`
struct Cli {
    #[clap(default_value_t = String::from("[::]:8080"), value_parser)]
    address: String,
    #[clap(long, value_parser)]
    id: String,
    #[clap(long, value_parser)]
    url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let origin_url = Url::parse(&cli.url)?;
    let webauthn = WebauthnBuilder::new(&cli.id, &origin_url)?
        .allow_subdomains(false)
        .build()?;

    let store = MemoryStore::new();
    let mut secret = [0u8; 64];
    OsRng.fill_bytes(&mut secret);
    let session_layer = SessionLayer::new(store, &secret);

    let state_dir = env::var("STATE_DIRECTORY")?;
    let mut db_path = PathBuf::from(state_dir);
    db_path.push("webauthn-tiny.db");
    let db = Connection::open(db_path).await?;

    let app = App::new(db, cli.id, cli.url)?;
    app.init().await?;

    let router = Router::new()
        .route("/validate", get(validate_handler))
        .route("/api/credentials", get(get_credentials_handler))
        .route("/api/credentials/:id", delete(delete_credentials_handler))
        .route(
            "/api/register",
            get(register_start_handler).post(register_end_handler),
        )
        .route(
            "/api/authenticate",
            get(authenticate_start_handler).post(authenticate_end_handler),
        )
        .layer(session_layer)
        .layer(Extension(Arc::new(RwLock::new(app))))
        .layer(Extension(Arc::new(webauthn)));

    let sock_addr: std::net::SocketAddr = cli.address.parse()?;
    eprintln!("listening on {}", sock_addr);
    Server::bind(&sock_addr)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}
