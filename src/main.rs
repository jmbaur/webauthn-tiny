use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    async_trait,
    body::{boxed, Full},
    extract::{self, FromRequest, Path, RequestParts},
    http::{header, HeaderMap, Response},
    http::{header::AUTHORIZATION, StatusCode},
    response::IntoResponse,
    routing::get,
    Extension,
};
use axum_auth::AuthBasic;
use clap::{arg, Command};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::ErrorKind,
    sync::{Arc, RwLock},
};
use webauthn_authenticator_rs::{u2fhid, WebauthnAuthenticator};
use webauthn_rs::{
    prelude::{Passkey, PasskeyAuthentication, Url, Uuid},
    Webauthn, WebauthnBuilder,
};
use webauthn_rs_proto::{CredentialID, PublicKeyCredential};

#[derive(Deserialize, Serialize)]
struct UserState {
    id: Uuid,
    hash: String,
    credentials: Vec<Passkey>,
}

fn load_users(path: &str) -> anyhow::Result<HashMap<String, UserState>> {
    let user_file_contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => match e.kind() {
            ErrorKind::NotFound => String::new(),
            e => anyhow::bail!(e),
        },
    };
    match serde_yaml::from_str::<HashMap<String, UserState>>(&user_file_contents) {
        Ok(m) => Ok(m),
        Err(e) => anyhow::bail!(e),
    }
}

fn persist_users(path: &str, users: &HashMap<String, UserState>) -> anyhow::Result<()> {
    let updated_user_file_contents = serde_yaml::to_string(users)?;
    match fs::write(path, updated_user_file_contents) {
        Err(e) => anyhow::bail!(e),
        _ => Ok(()),
    }
}

struct AppState {
    id: String,
    origin: String,
    user_file: String,
    in_progress_authentications: HashMap<String, PasskeyAuthentication>,
    users: HashMap<String, UserState>,
}

type SharedAppState = Arc<RwLock<AppState>>;

impl AppState {
    fn new(id: String, origin: String, user_file: String) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            origin,
            user_file: user_file.clone(),
            users: load_users(user_file.as_str())?,
            in_progress_authentications: HashMap::new(),
        })
    }
}

trait PersistentUserDB {
    fn save(&self) -> anyhow::Result<()>;
}

impl PersistentUserDB for AppState {
    fn save(&self) -> anyhow::Result<()> {
        persist_users(&self.user_file, &self.users)
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

        let existing_credentials: Vec<CredentialID> = user
            .credentials
            .iter()
            .map(|c| c.cred_id().to_owned())
            .collect();

        match add_credential(
            state.id.clone(),
            state.origin.clone(),
            user.id,
            username.to_string(),
            Some(existing_credentials),
        ) {
            Ok(c) => user.credentials.push(c),
            Err(e) => anyhow::bail!(e),
        }
    } else {
        let mut new_user = UserState {
            id: Uuid::new_v4(),
            hash: hash.to_string(),
            credentials: vec![],
        };
        match add_credential(
            state.id.clone(),
            state.origin.clone(),
            new_user.id,
            username.to_string(),
            None,
        ) {
            Ok(c) => new_user.credentials.push(c),
            Err(e) => anyhow::bail!(e),
        }
        state.users.insert(username.to_string(), new_user);
    }

    state.save()
}

fn add_credential(
    id: String,
    origin: String,
    user_id: Uuid,
    username: String,
    excluded_credentials: Option<Vec<CredentialID>>,
) -> anyhow::Result<Passkey> {
    let rp_id = &id;
    let rp_origin = Url::parse(&origin)?;
    let webauthn = build_webauthn(rp_id.as_str(), &rp_origin)?;

    let (chal, passkey_registration) = webauthn.start_passkey_registration(
        user_id,
        &username,
        &username,
        excluded_credentials.clone(),
    )?;

    let mut authenticator = WebauthnAuthenticator::new(u2fhid::U2FHid::default());
    let reg_credential = match authenticator.do_registration(rp_origin, chal) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{:#?}", e);
            anyhow::bail!("could not perform registration")
        }
    };

    let passkey = match webauthn.finish_passkey_registration(&reg_credential, &passkey_registration)
    {
        Ok(p) => p,
        Err(e) => anyhow::bail!(e),
    };

    if let Some(excluded) = excluded_credentials {
        if excluded.contains(passkey.cred_id()) {
            anyhow::bail!("credential already registered");
        }
    }

    Ok(passkey)
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

pub struct MyBasicAuth(AuthBasic);

#[async_trait]
impl<B: Send> FromRequest<B> for MyBasicAuth {
    type Rejection = (HeaderMap, StatusCode);

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if req.headers().get(AUTHORIZATION).is_some() {
            if let Ok(auth_basic) = AuthBasic::from_request(req).await {
                Ok(Self(auth_basic))
            } else {
                Err((HeaderMap::new(), StatusCode::INTERNAL_SERVER_ERROR))
            }
        } else {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::WWW_AUTHENTICATE,
                "Basic".parse().expect("failed to parse header value"),
            );
            Err((headers, StatusCode::UNAUTHORIZED))
        }
    }
}

async fn start_handler(
    MyBasicAuth(AuthBasic((username, password))): MyBasicAuth,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> impl IntoResponse {
    let mut state = match shared_state.write() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, String::from("{}"));
        }
    };
    let user = match state.users.get(&username) {
        Some(user) => match passwords_match(password, user.hash.to_owned()) {
            true => user,
            false => return (StatusCode::UNAUTHORIZED, String::from("{}")),
        },
        None => return (StatusCode::UNAUTHORIZED, String::from("{}")),
    };

    let (req_chal, passkey_auth) = match webauthn.start_passkey_authentication(&user.credentials) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("failed to start passkey authentication: {}", e);
            return (StatusCode::UNAUTHORIZED, String::from("{}"));
        }
    };

    state
        .in_progress_authentications
        .insert(username, passkey_auth);

    match serde_json::to_string(&req_chal) {
        Ok(j) => (StatusCode::OK, j),
        Err(e) => {
            eprintln!("failed to serialize JSON: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, String::from("{}"))
        }
    }
}

async fn end_handler(
    Path(username): Path<String>,
    payload: extract::Json<PublicKeyCredential>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> impl IntoResponse {
    let state = match shared_state.read() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let passkey = match state.in_progress_authentications.get(&username) {
        Some(p) => p,
        None => return StatusCode::NO_CONTENT,
    };

    let auth_result = match webauthn.finish_passkey_authentication(&payload.0, passkey) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return StatusCode::UNAUTHORIZED;
        }
    };

    eprintln!("{:#?}", auth_result);

    StatusCode::OK
}

async fn auth_handler() -> impl IntoResponse {
    StatusCode::UNAUTHORIZED
}

#[derive(RustEmbed)]
#[folder = "assets"]
struct Assets;

async fn assets_handler(Path(raw_path): Path<String>) -> impl IntoResponse {
    let path = raw_path.trim_start_matches('/');
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let (status, body) = match Assets::get(path) {
        Some(content) => (StatusCode::OK, boxed(Full::from(content.data))),
        None => (StatusCode::NOT_FOUND, boxed(Full::default())),
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(body)
        .expect("failed to build response")
}

async fn index_handler() -> impl IntoResponse {
    assets_handler(Path("index.html".to_string())).await
}

async fn favicon_handler() -> impl IntoResponse {
    assets_handler(Path("favicon.ico".to_string())).await
}

fn build_webauthn(id: &str, origin: &Url) -> anyhow::Result<Webauthn> {
    match WebauthnBuilder::new(id, origin)?
        .allow_subdomains(false)
        .build()
    {
        Ok(w) => Ok(w),
        Err(e) => anyhow::bail!(e),
    }
}

async fn serve(sub_m: &clap::ArgMatches) -> anyhow::Result<()> {
    let user_file = sub_m
        .get_one::<String>("userfile")
        .expect("`userfile` is required");

    let origin = sub_m
        .get_one::<String>("origin")
        .expect("`origin` is required");

    let id = sub_m.get_one::<String>("id").expect("`id` is required");

    let app_state = AppState::new(id.to_string(), origin.to_string(), user_file.to_string())?;

    let default_socket_addr = &"[::]:8080".to_string();
    let socket_addr: &String = match sub_m.get_one("address") {
        Some(a) => a,
        None => default_socket_addr,
    };
    let sock_addr: std::net::SocketAddr = socket_addr.parse()?;

    let origin_url = Url::parse(origin)?;
    let webauthn = build_webauthn(id.as_str(), &origin_url)?;

    let app = axum::Router::new()
        .route("/auth", get(auth_handler))
        .route("/start", get(start_handler))
        .route("/end/:username", get(end_handler))
        .route("/assets/*path", get(assets_handler))
        .route("/favicon.ico", get(favicon_handler))
        .route("/", get(index_handler))
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
