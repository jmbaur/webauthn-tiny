use crate::app;

use app::persist;
use app::SharedAppState;
use argon2::Argon2;
use argon2::PasswordHash;
use argon2::PasswordVerifier;
use async_trait::async_trait;
use axum::{
    body::{boxed, Full},
    extract::{self, FromRequest, Path, RequestParts},
    http::{
        header::{self, AUTHORIZATION},
        HeaderMap, Response, StatusCode,
    },
    response::IntoResponse,
    Extension,
};
use axum_auth::AuthBasic;
use rust_embed::RustEmbed;
use serde::Serialize;
use std::sync::Arc;
use webauthn_rs::Webauthn;
use webauthn_rs_proto::CreationChallengeResponse;
use webauthn_rs_proto::CredentialID;
use webauthn_rs_proto::PublicKeyCredential;
use webauthn_rs_proto::RegisterPublicKeyCredential;
use webauthn_rs_proto::RequestChallengeResponse;

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

pub async fn session_handler() {}

pub async fn register_start_handler(
    MyBasicAuth(AuthBasic((username, password))): MyBasicAuth,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> impl IntoResponse {
    let mut state = shared_state.write().expect("failed to lock state");
    let user = match state.users.get(&username) {
        Some(hash) => match passwords_match(password, hash.to_owned()) {
            true => match state.credentials.get(&username) {
                Some(user) => user,
                None => return (StatusCode::UNAUTHORIZED, String::from("{}")),
            },
            false => return (StatusCode::UNAUTHORIZED, String::from("{}")),
        },
        None => return (StatusCode::UNAUTHORIZED, String::from("{}")),
    };

    let existing_credentials: Vec<CredentialID> = user
        .credentials
        .iter()
        .map(|c| c.cred_id().to_owned())
        .collect();

    let (req_chal, passkey_reg) = match webauthn.start_passkey_registration(
        user.id,
        &username,
        &username,
        Some(existing_credentials),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("failed to start passkey registration: {}", e);
            return (StatusCode::UNAUTHORIZED, String::from("{}"));
        }
    };

    state
        .in_progress_registrations
        .insert(username.clone(), passkey_reg);

    #[derive(Serialize)]
    struct Return {
        username: String,
        challenge_response: CreationChallengeResponse,
    }

    match serde_json::to_string(&Return {
        username,
        challenge_response: req_chal,
    }) {
        Ok(j) => (StatusCode::OK, j),
        Err(e) => {
            eprintln!("failed to serialize JSON: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, String::from("{}"))
        }
    }
}

pub async fn register_end_handler(
    Path(username): Path<String>,
    payload: extract::Json<RegisterPublicKeyCredential>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> impl IntoResponse {
    let mut state = shared_state.write().expect("failed to lock state");

    let passkey_reg = dbg!(match state.in_progress_registrations.get(&username) {
        Some(r) => r,
        None => return StatusCode::NOT_FOUND,
    });

    eprintln!("{:#?}", payload);

    let passkey = dbg!(
        match webauthn.finish_passkey_registration(&payload, passkey_reg) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{e}");
                return StatusCode::UNAUTHORIZED;
            }
        }
    );

    let user = dbg!(match state.credentials.get_mut(&username) {
        Some(u) => u,
        None => return StatusCode::NOT_FOUND,
    });

    if user
        .credentials
        .iter()
        .any(|c| c.cred_id() == passkey.cred_id())
    {
        eprintln!("credential already registered");
        return StatusCode::BAD_REQUEST;
    }

    dbg!(user.credentials.push(passkey));

    if let Err(e) = dbg!(persist(&state.credential_file, &state.credentials)) {
        eprintln!("{e}");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::OK
}

pub async fn authenticate_start_handler(
    MyBasicAuth(AuthBasic((username, password))): MyBasicAuth,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> impl IntoResponse {
    let mut state = shared_state.write().expect("failed to lock state");
    let user = match state.users.get(&username) {
        Some(hash) => match passwords_match(password, hash.to_owned()) {
            true => match state.credentials.get(&username) {
                Some(user) => user,
                None => return (StatusCode::UNAUTHORIZED, String::from("{}")),
            },
            false => return (StatusCode::UNAUTHORIZED, String::from("{}")),
        },
        None => return (StatusCode::UNAUTHORIZED, String::from("{}")),
    };

    if user.credentials.is_empty() {
        return (StatusCode::UNAUTHORIZED, String::from("{}"));
    }

    let (req_chal, passkey_auth) = match webauthn.start_passkey_authentication(&user.credentials) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("failed to start passkey authentication: {}", e);
            return (StatusCode::UNAUTHORIZED, String::from("{}"));
        }
    };

    state
        .in_progress_authentications
        .insert(username.clone(), passkey_auth);

    #[derive(Serialize)]
    struct Return {
        username: String,
        challenge_response: RequestChallengeResponse,
    }

    match serde_json::to_string(&Return {
        username,
        challenge_response: req_chal,
    }) {
        Ok(j) => (StatusCode::OK, j),
        Err(e) => {
            eprintln!("failed to serialize JSON: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, String::from("{}"))
        }
    }
}

pub async fn authenticate_end_handler(
    Path(username): Path<String>,
    payload: extract::Json<PublicKeyCredential>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> impl IntoResponse {
    let state = shared_state.read().expect("failed to lock state");

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

#[derive(RustEmbed)]
#[folder = "$ASSETS_DIR"]
struct Assets;

pub async fn assets_handler(Path(raw_path): Path<String>) -> impl IntoResponse {
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
