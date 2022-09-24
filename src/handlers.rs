use crate::app;

use app::SharedAppState;
use async_trait::async_trait;
use axum::extract::{FromRequest, Path, RequestParts};
use axum::{extract, http::StatusCode, Extension, Json};
use axum_macros::debug_handler;
use axum_sessions::extractors::{ReadableSession, WritableSession};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use webauthn_rs::Webauthn;
use webauthn_rs_proto::{
    CreationChallengeResponse, CredentialID, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};

pub struct RequireLoggedIn;

#[async_trait]
impl<B> FromRequest<B> for RequireLoggedIn
where
    B: Send,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if let Ok(session) = ReadableSession::from_request(req).await {
            if session.get::<bool>("logged_in").unwrap_or(false) {
                Ok(Self)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        } else {
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub struct XRemoteUser(String);

#[async_trait]
impl<B> FromRequest<B> for XRemoteUser
where
    B: Send,
{
    type Rejection = StatusCode;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        if let Some(x_remote_user) = req.headers().get("x-remote-user") {
            if let Ok(val) = x_remote_user.to_str() {
                Ok(XRemoteUser(String::from(val)))
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        } else {
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[debug_handler]
pub async fn register_start_handler(
    XRemoteUser(username): XRemoteUser,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<CreationChallengeResponse>, StatusCode> {
    let mut state = shared_state.write().await;
    let user = state.get_user_with_credentials(username).await?;

    let (req_chal, passkey_reg) = match webauthn.start_passkey_registration(
        user.id,
        &user.username,
        &user.username, // use username as display name
        Some(
            user.credentials
                .iter()
                .map(|c| c.credential.cred_id().to_owned())
                .collect(),
        ),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("webauthn.start_passkey_registration: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    state
        .in_progress_registrations
        .insert(user.username, passkey_reg);

    Ok(Json(req_chal))
}

#[derive(Deserialize)]
pub struct RegisterEndRequestPayload {
    credential_name: String,
    public_key: RegisterPublicKeyCredential,
}

#[debug_handler]
pub async fn register_end_handler(
    XRemoteUser(username): XRemoteUser,
    payload: extract::Json<RegisterEndRequestPayload>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<(), StatusCode> {
    let mut state = shared_state.write().await;

    let passkey_reg = match state.in_progress_registrations.get(&username) {
        Some(r) => r,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let passkey = match webauthn.finish_passkey_registration(&payload.public_key, passkey_reg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("webauthn.finish_passkey_registration: {e}");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let user = state.get_user_with_credentials(username.clone()).await?;

    if user
        .credentials
        .iter()
        .any(|c| *c.credential.cred_id() == *passkey.cred_id())
    {
        eprintln!("credential already registered");
        return Err(StatusCode::BAD_REQUEST);
    }

    state
        .add_credential(username, payload.credential_name.clone(), passkey)
        .await?;

    state.in_progress_registrations.remove(&user.username);

    Ok(())
}

#[debug_handler]
pub async fn authenticate_start_handler(
    XRemoteUser(username): XRemoteUser,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<RequestChallengeResponse>, StatusCode> {
    let mut state = shared_state.write().await;

    let user = state.get_user_with_credentials(username).await?;
    if user.credentials.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let passkeys: Vec<_> = user
        .credentials
        .iter()
        .map(|c| c.credential.to_owned())
        .collect();
    let (req_chal, passkey_auth) = match webauthn.start_passkey_authentication(&passkeys) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("webauthn.start_passkey_authentication: {e}");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    state
        .in_progress_authentications
        .insert(user.username, passkey_auth);

    Ok(Json(req_chal))
}

#[debug_handler]
pub async fn authenticate_end_handler(
    XRemoteUser(username): XRemoteUser,
    mut session: WritableSession,
    payload: extract::Json<PublicKeyCredential>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<(), StatusCode> {
    let mut state = shared_state.write().await;

    let passkey_authentication = match state.in_progress_authentications.get(&username) {
        Some(p) => p,
        None => return Err(StatusCode::NO_CONTENT),
    };

    let auth_result =
        match webauthn.finish_passkey_authentication(&payload.0, passkey_authentication) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("webauthn.finish_passkey_authentication: {e}");
                return Err(StatusCode::UNAUTHORIZED);
            }
        };

    if auth_result.needs_update() {
        state.update_credential(auth_result).await?;
    }

    if let Err(e) = session.insert("logged_in", true) {
        eprintln!("session.insert: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    state.in_progress_authentications.remove(&username);

    Ok(())
}

#[derive(Serialize)]
pub struct GetCredentialsResponsePayload {
    credentials: Vec<(CredentialID, String)>,
}

#[debug_handler]
pub async fn get_credentials_handler() -> Result<Json<GetCredentialsResponsePayload>, StatusCode> {
    Ok(Json(GetCredentialsResponsePayload {
        credentials: vec![],
    }))
}

#[debug_handler]
pub async fn delete_credentials_handler(
    Path(id): Path<String>,
    shared_state: Extension<SharedAppState>,
) -> Result<(), StatusCode> {
    let cred_id = CredentialID::from(id.as_bytes().to_vec());
    let state = shared_state.read().await;
    Ok(state.delete_credential(cred_id).await?)
}
