use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension};
use axum_auth::AuthBasic;
use clap::{arg, Command};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::ErrorKind, sync::Arc};
use webauthn_authenticator_rs::{u2fhid, WebauthnAuthenticator};
use webauthn_rs::{
    prelude::{Base64UrlSafeData, Url, Uuid},
    WebauthnBuilder,
};
use webauthn_rs_proto::{CredentialID, RegisterPublicKeyCredential};

#[derive(Deserialize, Serialize)]
struct UserState {
    id: Uuid,
    hash: String,
    credentials: Vec<RegisterPublicKeyCredential>,
}

type UserMap = HashMap<String, UserState>;

trait PersistentUserDB {
    fn persist(&self, path: &str) -> anyhow::Result<()>;
}

struct AppState {
    id: String,
    origin: String,
    users: UserMap,
}

impl AppState {
    fn load(id: String, origin: String, path: &str) -> anyhow::Result<Self> {
        let password_file_contents = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => match e.kind() {
                ErrorKind::NotFound => String::new(),
                e => return Err(anyhow::anyhow!(e)),
            },
        };
        match serde_yaml::from_str::<UserMap>(&password_file_contents) {
            Ok(users) => Ok(Self { id, origin, users }),
            Err(e) => Err(anyhow::anyhow!(e)),
        }
    }
}

impl PersistentUserDB for AppState {
    fn persist(&self, path: &str) -> anyhow::Result<()> {
        let updated_password_file_contents = serde_yaml::to_string(&self.users)?;
        match fs::write(path, updated_password_file_contents) {
            Err(e) => Err(anyhow::anyhow!(e)),
            _ => Ok(()),
        }
    }
}

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

    let password_file = sub_m
        .get_one::<String>("passwordfile")
        .expect("`passwordfile` is required");

    let mut app_state = AppState::load(id.to_string(), origin.to_string(), password_file)?;

    let argon = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);

    let hash = match argon.hash_password(password.as_bytes(), &salt) {
        Ok(h) => h,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    if let Some(user) = app_state.users.get_mut(username) {
        user.hash = hash.to_string();

        let existing_credentials: Vec<Base64UrlSafeData> = user
            .credentials
            .iter()
            .map(|c| Base64UrlSafeData::from(c.id.as_bytes().to_vec()))
            .collect();

        match add_credential(
            app_state.id.clone(),
            app_state.origin.clone(),
            user.id,
            username.to_string(),
            Some(existing_credentials),
        ) {
            Ok(c) => user.credentials.push(c),
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
    } else {
        let mut new_user = UserState {
            id: Uuid::new_v4(),
            hash: hash.to_string(),
            credentials: vec![],
        };
        match add_credential(
            app_state.id.clone(),
            app_state.origin.clone(),
            new_user.id,
            username.to_string(),
            None,
        ) {
            Ok(c) => new_user.credentials.push(c),
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
        app_state.users.insert(username.to_string(), new_user);
    }

    app_state.persist(password_file)
}

fn add_credential(
    id: String,
    origin: String,
    user_id: Uuid,
    username: String,
    exclude_credentials: Option<Vec<CredentialID>>,
) -> anyhow::Result<RegisterPublicKeyCredential> {
    let rp_id = &id;
    let rp_origin = Url::parse(&origin)?;
    let webauthn = WebauthnBuilder::new(rp_id.as_str(), &rp_origin)?
        .allow_subdomains(false)
        .build()?;

    let (chal, _) =
        webauthn.start_passkey_registration(user_id, &username, &username, exclude_credentials)?;

    let mut webauthn = WebauthnAuthenticator::new(u2fhid::U2FHid::default());
    let credential = match webauthn.do_registration(rp_origin, chal) {
        Ok(c) => c,
        Err(_) => todo!(), // TODO(jared): non-standard error?
    };

    Ok(credential)
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

async fn auth_handler(
    AuthBasic((username, password)): AuthBasic,
    Extension(state): Extension<Arc<AppState>>,
) -> impl IntoResponse {
    let _user = match state.users.get(&username) {
        Some(user) => match passwords_match(password, user.hash.to_owned()) {
            true => user,
            false => return StatusCode::UNAUTHORIZED,
        },
        None => return StatusCode::UNAUTHORIZED,
    };

    // TODO(jared): perform webauthn authentication
    StatusCode::OK
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
        .subcommand(
            Command::new("register")
                .args(&[
                    arg!(-f --passwordfile <FILE> "The path to a password file"),
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
                    arg!(-f --passwordfile <FILE> "The path to a password file"),
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
