mod app;
mod handlers;

use app::{AppState, UserState};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    extract::Path,
    routing::{get, post},
    Extension,
};
use clap::{arg, Command};
use handlers::{
    assets_handler, authenticate_end_handler, authenticate_start_handler, register_end_handler,
    register_start_handler, session_handler,
};
use std::sync::{Arc, RwLock};
use webauthn_rs::{
    prelude::{Url, Uuid},
    Webauthn, WebauthnBuilder,
};

fn register(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    let id = sub_m.get_one::<String>("id").expect("`id` is required");
    let origin = sub_m
        .get_one::<String>("origin")
        .expect("`origin` is required");

    let username = sub_m
        .get_one::<String>("username")
        .expect("`username` is required");
    let password = sub_m
        .get_one::<String>("password")
        .expect("`password` is required");

    let user_file = sub_m
        .get_one::<String>("userfile")
        .expect("`userfile` is required");

    let mut state = AppState::new(id.to_string(), origin.to_string(), user_file.to_string())?;

    let argon = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);

    let hash = match argon.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h,
        Err(e) => anyhow::bail!(e),
    };

    if let Some(user) = state.users.get_mut(username) {
        user.hash = hash.to_string();
    } else {
        let new_user = UserState {
            id: Uuid::new_v4(),
            hash: hash.to_string(),
            credentials: vec![],
        };

        state.users.insert(username.to_string(), new_user);
    }

    state.save()
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

    let origin = sub_m
        .get_one::<String>("origin")
        .expect("`origin` is required");

    let id = sub_m.get_one::<String>("id").expect("`id` is required");

    let app_state = dbg!(AppState::new(
        id.to_string(),
        origin.to_string(),
        user_file.to_string()
    ))?;

    let default_socket_addr = &"[::]:8080".to_string();
    let socket_addr: &String = match sub_m.get_one("address") {
        Some(a) => a,
        None => default_socket_addr,
    };
    let sock_addr: std::net::SocketAddr = socket_addr.parse()?;

    let origin_url = Url::parse(origin)?;
    let webauthn = build_webauthn(id.as_str(), &origin_url)?;

    let app = axum::Router::new()
        .route("/session", get(session_handler))
        .route("/authenticate/start", get(authenticate_start_handler))
        .route(
            "/authenticate/end/:username",
            post(authenticate_end_handler),
        )
        .route("/register/start", get(register_start_handler))
        .route("/register/end/:username", post(register_end_handler))
        .route("/assets/*path", get(assets_handler))
        .route(
            "/favicon.ico",
            get(|| async { assets_handler(Path("favicon.ico".to_string())).await }),
        )
        .route(
            "/",
            get(|| async { assets_handler(Path("index.html".to_string())).await }),
        )
        .layer(Extension(Arc::new(RwLock::new(app_state))))
        .layer(Extension(Arc::new(webauthn)));

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
            Command::new("register")
                .args(&[
                    arg!(-f --userfile <FILE> "The path to a password file"),
                    arg!(-i --id <ID> "ID to use for webauthn"),
                    arg!(-o --origin <ORIGIN> "Origin to use for webauthn"),
                    arg!(-p --password <PASSWORD> "The password for the user"),
                    arg!(-u --username <NAME> "The name for the user"),
                ])
                .about("Register a new user"),
        )
        .subcommand(
            Command::new("serve")
                .args(&[
                    arg!(-a --address [ADDRESS] "Socket address to bind to"),
                    arg!(-f --userfile <FILE> "The path to a password file"),
                    arg!(-i --id <ID> "ID to use for webauthn"),
                    arg!(-o --origin <ORIGIN> "Origin to use for webauthn"),
                ])
                .about("Start the HTTP server"),
        )
        .get_matches();

    match cmd.subcommand() {
        Some(("register", sub_m)) => register(sub_m),
        Some(("serve", sub_m)) => serve(sub_m).await,
        _ => unreachable!(),
    }
}
