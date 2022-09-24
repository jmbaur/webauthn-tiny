use crate::app;

use app::SharedAppState;
use async_trait::async_trait;
use axum::extract::FromRequest;
use axum::extract::Path;
use axum::extract::RequestParts;
use axum::Json;
use axum::{extract, http::StatusCode, response::IntoResponse, Extension};
use axum_macros::debug_handler;
use axum_sessions::extractors::ReadableSession;
use axum_sessions::extractors::WritableSession;
use std::sync::Arc;
use webauthn_rs::Webauthn;
use webauthn_rs_proto::CreationChallengeResponse;
use webauthn_rs_proto::CredentialID;
use webauthn_rs_proto::PublicKeyCredential;
use webauthn_rs_proto::RegisterPublicKeyCredential;
use webauthn_rs_proto::RequestChallengeResponse;

#[derive(Clone, Debug)]
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
pub async fn validate_handler(session: ReadableSession) -> StatusCode {
    if session.get::<bool>("logged_in").unwrap_or(false) {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
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
                .map(|c| c.cred_id().to_owned())
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

#[debug_handler]
pub async fn register_end_handler(
    XRemoteUser(username): XRemoteUser,
    payload: extract::Json<RegisterPublicKeyCredential>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<(), StatusCode> {
    let mut state = shared_state.write().await;

    let passkey_reg = match state.in_progress_registrations.get(&username) {
        Some(r) => r,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let passkey = match webauthn.finish_passkey_registration(&payload, passkey_reg) {
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
        .any(|c| *c.cred_id() == *passkey.cred_id())
    {
        eprintln!("credential already registered");
        return Err(StatusCode::BAD_REQUEST);
    }

    state.add_credential(username, passkey).await?;

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

    let (req_chal, passkey_auth) = match webauthn.start_passkey_authentication(&user.credentials) {
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
        state
            .increment_credential_counter(auth_result.cred_id(), auth_result.counter())
            .await?;
    }

    if let Err(e) = session.insert("logged_in", true) {
        eprintln!("session.insert: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    state.in_progress_authentications.remove(&username);

    Ok(())
}

#[debug_handler]
pub async fn get_credentials_handler() -> impl IntoResponse {
    todo!()
}

#[debug_handler]
pub async fn delete_credentials_handler(
    Path(id): Path<String>,
    session: ReadableSession,
    shared_state: Extension<SharedAppState>,
) -> Result<(), StatusCode> {
    // TODO(jared): pull this out into a middleware
    if !session.get::<bool>("logged_in").unwrap_or(false) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let cred_id = CredentialID::from(id.as_bytes().to_vec());
    let state = shared_state.read().await;
    Ok(state.delete_credential(cred_id).await?)
}
