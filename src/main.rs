use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension};
use axum_auth::AuthBasic;
use clap::{arg, Command};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::ErrorKind, sync::Arc};
use webauthn_rs::{
    prelude::{Url, Uuid},
    WebauthnBuilder,
};

#[derive(Deserialize, Serialize)]
struct UserState {
    id: Uuid,
    hash: String,
}
type UserMap = HashMap<String, UserState>;

struct AppState {
    users: UserMap,
    origin: String,
    id: String,
}

fn adduser(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    let username = sub_m
        .get_one::<String>("username")
        .expect("`username` is required");
    let password = sub_m
        .get_one::<String>("password")
        .expect("`password` is required");

    let password_file = sub_m
        .get_one::<String>("passwordfile")
        .expect("`passwordfile` is required");
    let password_file_contents = match fs::read_to_string(password_file) {
        Ok(c) => c,
        Err(e) => match e.kind() {
            ErrorKind::NotFound => String::new(),
            e => return Err(anyhow::anyhow!(e)),
        },
    };
    let mut users = serde_yaml::from_str::<UserMap>(&password_file_contents)?;

    let argon = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);

    let hash = match argon.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    if let Some(user) = users.get_mut(username) {
        user.hash = hash.to_string();
    } else {
        users.insert(
            username.to_string(),
            UserState {
                id: Uuid::new_v4(),
                hash: hash.to_string(),
            },
        );
    }

    let updated_password_file_contents = serde_yaml::to_string(&users)?;

    fs::write(password_file, updated_password_file_contents)?;

    Ok(())
}

fn passwords_match(password: Option<String>, hash: String) -> bool {
    if password.is_none() {
        return false;
    }

    let parsed_hash = match PasswordHash::new(&hash) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let argon = Argon2::default();
    argon
        .verify_password(
            password.expect("already checked is not none").as_bytes(),
            &parsed_hash,
        )
        .is_ok()
}
async fn register_handler(
    AuthBasic((username, password)): AuthBasic,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    let maybe_user = state.users.get(&username);
    if maybe_user.is_none() {
        return (StatusCode::UNAUTHORIZED, String::from("{}"));
    }
    let user = maybe_user.expect("already checked is not none");

    if !passwords_match(password, user.hash.to_owned()) {
        return (StatusCode::UNAUTHORIZED, String::from("{}"));
    }

    let rp_id = &state.id;
    let rp_origin = Url::parse(state.origin.as_str()).expect("Invalid URL");
    let webauthn = WebauthnBuilder::new(rp_id.as_str(), &rp_origin)
        .expect("Invalid configuration")
        .allow_subdomains(false)
        .build()
        .expect("Invalid configuration");

    let (chal, _) = webauthn
        .start_passkey_registration(user.id, username.as_str(), username.as_str(), None)
        .expect("Failed to start registration");

    (
        StatusCode::OK,
        serde_json::to_string(&chal).expect("Failed to serialize public key challenge"),
    )
}

async fn auth_handler(
    AuthBasic((username, password)): AuthBasic,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    match state.users.get(&username) {
        Some(user) => match passwords_match(password, user.hash.to_owned()) {
            true => StatusCode::OK,
            false => StatusCode::UNAUTHORIZED,
        },
        None => StatusCode::UNAUTHORIZED,
    }
}

async fn serve(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    let password_file = sub_m
        .get_one::<String>("passwordfile")
        .expect("`passwordfile` is required");
    let password_file_contents = fs::read_to_string(password_file)?;
    let passwords = serde_yaml::from_str(&password_file_contents)?;

    let origin = sub_m
        .get_one::<String>("origin")
        .expect("`origin` is required");

    let id = sub_m.get_one::<String>("id").expect("`id` is required");

    let app_state = Arc::new(AppState {
        users: passwords,
        origin: origin.to_string(),
        id: id.to_string(),
    });

    let default_socket_addr = &"[::]:8080".to_string();
    let socket_addr: &String = match sub_m.get_one("address") {
        Some(a) => a,
        None => default_socket_addr,
    };
    let sock_addr: std::net::SocketAddr = socket_addr.parse()?;

    let app = axum::Router::new()
        .route("/auth", get(auth_handler))
        .route("/register", get(register_handler))
        .layer(Extension(app_state));

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
        .subcommand(Command::new("adduser").args(&[
            arg!(-f --passwordfile <FILE> "The path to a password file"),
            arg!(-p --password <PASSWORD> "The password for the user"),
            arg!(-u --username <NAME> "The name for the user"),
        ]))
        .subcommand(Command::new("serve").args(&[
            arg!(-a --address [ADDRESS] "Socket address to bind to"),
            arg!(-f --passwordfile <FILE> "The path to a password file"),
            arg!(-i --id [ID] "ID to use for webauthn"),
            arg!(-o --origin [ORIGIN] "Origin to use for webauthn"),
        ]))
        .get_matches();

    match cmd.subcommand() {
        Some(("adduser", sub_m)) => adduser(sub_m),
        Some(("serve", sub_m)) => serve(sub_m).await,
        _ => unreachable!(),
    }
}