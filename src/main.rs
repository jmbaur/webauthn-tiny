use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension};
use axum_auth::AuthBasic;
use clap::{arg, Command};
use std::{collections::HashMap, fs, io::ErrorKind, sync::Arc};

type UserPass = HashMap<String, String>;

struct AppState {
    passwords: UserPass,
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
    let mut passwords = serde_yaml::from_str::<UserPass>(&password_file_contents)?;

    let argon = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);

    let hash = match argon.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    if let Some(current) = passwords.get_mut(username) {
        *current = hash.to_string();
    } else {
        passwords.insert(username.to_string(), hash.to_string());
    }

    let updated_password_file_contents = serde_yaml::to_string(&passwords)?;

    fs::write(password_file, updated_password_file_contents)?;

    Ok(())
}

async fn register_handler() -> impl IntoResponse {
    // let rp_id = "localhost";
    // let rp_origin = Url::parse("http://localhost:8080").expect("Invalid URL");
    // let webauthn = WebauthnBuilder::new(rp_id, &rp_origin)
    //     .expect("Invalid configuration")
    //     .allow_subdomains(false)
    //     .build()
    //     .expect("Invalid configuration");
    //
    // let uuid = Uuid::new_v4();
    // let username = "jared";
    // let userdisplayname = "Jared Baur";
    // let (chal, _) = webauthn
    //     .start_passkey_registration(uuid, username, userdisplayname, None)
    //     .expect("Failed to start registration");
    //
    // let chal_json = serde_json::to_string(&chal).expect("Failed to serialize public key challenge");

    // let parsed_hash = PasswordHash::new(&password_hash)?;
    // assert!(Argon2::default().verify_password(password, &parsed_hash).is_ok());

    StatusCode::OK
}

async fn auth_handler(
    AuthBasic((username, password)): AuthBasic,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    if password.is_none() {
        return StatusCode::UNAUTHORIZED;
    }

    let password_hash = match state.passwords.get(&username) {
        Some(p) => p,
        None => return StatusCode::UNAUTHORIZED,
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(p) => p,
        Err(_) => return StatusCode::UNAUTHORIZED,
    };

    let argon = Argon2::default();
    if !argon
        .verify_password(
            password.expect("already checked is not none").as_bytes(),
            &parsed_hash,
        )
        .is_ok()
    {
        return StatusCode::UNAUTHORIZED;
    }

    StatusCode::OK
}

async fn serve(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    let password_file = sub_m
        .get_one::<String>("passwordfile")
        .expect("`passwordfile` is required");
    let password_file_contents = fs::read_to_string(password_file)?;
    let passwords = serde_yaml::from_str(&password_file_contents)?;
    let app_state = Arc::new(AppState { passwords });

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
            arg!(-u --username <NAME> "The name for the user"),
            arg!(-p --password <PASSWORD> "The password for the user"),
            arg!(-f --passwordfile <FILE> "The path to a password file"),
        ]))
        .subcommand(Command::new("serve").args(&[
            arg!(-a --address [ADDRESS] "Socket address to bind to"),
            arg!(-f --passwordfile <FILE> "The path to a password file"),
        ]))
        .get_matches();

    match cmd.subcommand() {
        Some(("adduser", sub_m)) => adduser(sub_m),
        Some(("serve", sub_m)) => serve(sub_m).await,
        _ => unreachable!(),
    }
}
