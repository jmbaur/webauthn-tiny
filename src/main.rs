use argon2::{Argon2, PasswordHash};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension};
use clap::{arg, Command};
use std::{collections::HashMap, sync::Arc};

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

struct AppState {
    passwords: HashMap<String, String>,
}

async fn auth_handler(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    let password_hash = match state.passwords.get("foo") {
        Some(p) => p,
        None => return (StatusCode::UNAUTHORIZED, ""),
    };

    let parsed_hash = match PasswordHash::new(&password_hash) {
        Ok(p) => p,
        Err(_) => return (StatusCode::UNAUTHORIZED, ""),
    };

    let res = parsed_hash.verify_password(&[&Argon2::default()], "bar");
    eprintln!("{:#?}", res);

    (StatusCode::OK, "")
}

fn adduser(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    // let a = Argon2::default();
    // sub_m.args_present()
    // a.hash_password_into();
    println!("{:#?}", sub_m);
    // sub_m.get_many();

    Ok(())
}

async fn serve(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    println!("{:#?}", sub_m);

    // let password_file_contents = fs::read_to_string()?;
    //
    // let passwords = serde_yaml::from_str(&password_file_contents)?;
    //
    // let app_state = Arc::new(AppState { passwords });

    let app = axum::Router::new().route("/auth", get(auth_handler));
    // .layer(Extension(app_state));

    let sock_addr: std::net::SocketAddr =
        "[::]:8080".parse().expect("Failed to parse socket address");

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
        ]))
        .subcommand(Command::new("serve").args(&[
            arg!(-p --passwordfile <FILE> "Path to a password file"),
            arg!(-a --address [ADDRESS] "Socket address to bind to"),
        ]))
        .get_matches();

    match cmd.subcommand() {
        Some(("adduser", sub_m)) => adduser(sub_m),
        Some(("serve", sub_m)) => serve(sub_m).await,
        _ => unreachable!(),
    }
}
