mod app;
mod handlers;

use app::AppState;
use app::{load, persist, Users};
use argon2::{
    password_hash::{
        rand_core::{OsRng, RngCore},
        PasswordHasher, SaltString,
    },
    Argon2,
};
use axum::{
    handler::Handler,
    routing::{get, post},
    Extension, Router,
};
use axum_sessions::{async_session::MemoryStore, SessionLayer};
use clap::{arg, Command};
use handlers::{
    authenticate_end_handler, authenticate_start_handler, fallback_handler, register_end_handler,
    register_start_handler, session_handler,
};
use std::io::Write;
use std::{
    io,
    sync::{Arc, RwLock},
};
use tower::ServiceBuilder;
use tower_http::trace::{DefaultOnRequest, TraceLayer};
use webauthn_rs::{prelude::Url, Webauthn, WebauthnBuilder};

fn adduser(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    let user_file = sub_m
        .get_one::<String>("userfile")
        .expect("`userfile` is required");

    print!("username: ");
    io::stdout().flush()?;
    let stdin = io::stdin();
    let mut iterator = stdin.lines();
    let username = iterator.next().expect("")?;
    let password = rpassword::prompt_password("password: ")?;
    let confirmed_password = rpassword::prompt_password("confirm password: ")?;

    if password != confirmed_password {
        anyhow::bail!("passwords do not match")
    }

    let mut users = load::<Users>(user_file)?;

    let argon = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);

    let hash = match argon.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h,
        Err(e) => anyhow::bail!(e),
    };

    if let Some(password) = users.get_mut(&username) {
        *password = hash.to_string();
    } else {
        users.insert(username.to_string(), hash.to_string());
    }

    persist(user_file, &users)
}

fn build_webauthn(id: &str, origin: &Url) -> anyhow::Result<Webauthn> {
    let webauthn = WebauthnBuilder::new(id, origin)?
        .allow_subdomains(false)
        .build()?;
    Ok(webauthn)
}

async fn serve(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    let user_file = sub_m
        .get_one::<String>("userfile")
        .expect("`userfile` is required");

    let credential_file = sub_m
        .get_one::<String>("credentialfile")
        .expect("`credentialfile` is required");

    let origin = sub_m
        .get_one::<String>("origin")
        .expect("`origin` is required");

    let id = sub_m.get_one::<String>("id").expect("`id` is required");

    let app_state = AppState::new(
        id.to_string(),
        origin.to_string(),
        user_file.to_string(),
        credential_file.to_string(),
    )?;

    let default_socket_addr = &"[::]:8080".to_string();
    let socket_addr: &String = match sub_m.get_one("address") {
        Some(a) => a,
        None => default_socket_addr,
    };
    let sock_addr: std::net::SocketAddr = socket_addr.parse()?;

    let origin_url = Url::parse(origin)?;
    let webauthn = build_webauthn(id.as_str(), &origin_url)?;

    let store = MemoryStore::new();
    let mut secret = [0u8; 64];
    OsRng.fill_bytes(&mut secret);
    let session_layer = SessionLayer::new(store, &secret);

    let app = Router::new()
        .route("/session", get(session_handler))
        .route("/authenticate/start", get(authenticate_start_handler))
        .route(
            "/authenticate/end/:username",
            post(authenticate_end_handler),
        )
        .route("/register/start", get(register_start_handler))
        .route("/register/end/:username", post(register_end_handler))
        .fallback(fallback_handler.into_service())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http().on_request(DefaultOnRequest::new()))
                .layer(session_layer)
                .layer(Extension(Arc::new(RwLock::new(app_state))))
                .layer(Extension(Arc::new(webauthn))),
        );

    eprintln!("listening on {}", sock_addr);
    axum::Server::bind(&sock_addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cmd = Command::new("webauthn-tiny")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("adduser")
                .args(&[arg!(-u --userfile <FILE> "The path to a users file (YAML)")])
                .about("Register a new user"),
        )
        .subcommand(
            Command::new("serve")
                .args(&[
                    arg!(-a --address [ADDRESS] "Socket address to bind to"),
                    arg!(-c --credentialfile <FILE> "The path to a credentials file"),
                    arg!(-i --id <ID> "ID to use for webauthn"),
                    arg!(-o --origin <ORIGIN> "Origin to use for webauthn"),
                    arg!(-u --userfile <FILE> "The path to a users file (YAML)"),
                ])
                .about("Start the HTTP server"),
        )
        .get_matches();

    match cmd.subcommand() {
        Some(("adduser", sub_m)) => adduser(sub_m),
        Some(("serve", sub_m)) => serve(sub_m).await,
        _ => unreachable!(),
    }
}
