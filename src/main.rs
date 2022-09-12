use argon2::{Argon2, PasswordHash};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension};
use clap::{App, Arg, Parser, SubCommand};
use std::{collections::HashMap, fs, sync::Arc};

/// Simple webauthn server
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to a password file
    #[clap(short, long, value_parser)]
    password_file: String,
}

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

    let a = Argon2::default();
    let res = parsed_hash.verify_password(&[&Argon2::default()], "bar");
    eprintln!("{:#?}", res);

    (StatusCode::OK, "")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::new("webauthn-server").subcommand(
        SubCommand::with_name("add").about("Add a new user").arg(
            Arg::with_name("username")
                .help("The username for a user")
                .index(1),
        ),
    );

    let _args = Args::parse();

    let password_file_contents = fs::read_to_string(_args.password_file)?;

    let passwords = serde_yaml::from_str(&password_file_contents)?;

    let app_state = Arc::new(AppState { passwords });

    let app = axum::Router::new()
        .route("/auth", get(auth_handler))
        .layer(Extension(app_state));

    let sock_addr: std::net::SocketAddr =
        "[::]:8080".parse().expect("Failed to parse socket address");

    eprintln!("listening on {}", sock_addr);
    axum::Server::bind(&sock_addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())

    //     let reg_pub_key_cred: RegisterPublicKeyCredential = serde_json::from_str(
    //         r#"
    // {}
    //         "#,
    //     )?;
    //
    //     let passkey = webauthn
    //         .finish_passkey_registration(&reg_pub_key_cred, &reg)
    //         .expect("Failed to finish registration");
    //
    //     // TODO(jared): assert that passkey.cred_id is not registered with any other users.
    //
    //     eprintln!("{:#?}", passkey);
}
