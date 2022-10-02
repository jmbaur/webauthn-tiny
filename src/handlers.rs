use crate::app;
use app::SharedAppState;
use async_trait::async_trait;
use axum::extract::{FromRequest, Path, RequestParts};
use axum::{extract, http::StatusCode, Extension, Json};
use axum_macros::debug_handler;
use axum_sessions::extractors::{ReadableSession, WritableSession};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use webauthn_rs::prelude::{PasskeyAuthentication, PasskeyRegistration};
use webauthn_rs::Webauthn;
use webauthn_rs_proto::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};

const SESSIONKEY_LOGGEDIN: &str = "logged_in";
const SESSIONKEY_PASSKEYREGISTRATION: &str = "passkey_registration";
const SESSIONKEY_PASSKEYAUTHENTICATION: &str = "passkey_authentication";

pub struct RequireLoggedIn;

#[async_trait]
impl<B> FromRequest<B> for RequireLoggedIn
where
    B: Send,
{
    type Rejection = StatusCode;

    // TODO(jared): On top of ensuring the client's cookie is associated with a session where the
    // user is logged in, should we also ensure that the username for the session matches with the
    // authenticated user? This would mean that a user who exists in the database could not use the
    // cookie from another user in order to pass authentication. Currently, only the username value
    // from the X-Remote-User header is used for fetching a users information, however it could use
    // the username value from the existing session. This really only makes sense if the username
    // is validated with the existing session.
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        tracing::trace!("RequireLoggedIn extractor");

        if let Ok(session) = ReadableSession::from_request(req).await {
            if session.get::<bool>(SESSIONKEY_LOGGEDIN).unwrap_or_default() {
                Ok(Self)
            } else {
                tracing::info!("user not logged in");
                tracing::info!("headers for request:");
                let header_map = req.headers();
                header_map.iter().for_each(|(hkey, hval)| {
                    tracing::info!("{:?}: {:?}", hkey, hval);
                });
                Err(StatusCode::UNAUTHORIZED)
            }
        } else {
            tracing::error!("could not get session");
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
        tracing::trace!("XRemoteUser extractor");

        if let Some(x_remote_user) = req.headers().get("X-Remote-User") {
            if let Ok(val) = x_remote_user.to_str() {
                Ok(XRemoteUser(String::from(val)))
            } else {
                tracing::error!("could not conver header value to string");
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        } else {
            tracing::info!("no X-Remote-User header present");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[debug_handler]
pub async fn register_start_handler(
    XRemoteUser(username): XRemoteUser,
    mut session: WritableSession,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<CreationChallengeResponse>, StatusCode> {
    tracing::trace!("register_start_handler");

    let app = shared_state.read().await;
    let user = app.get_user_with_credentials(username).await?;

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
            tracing::error!("webauthn.start_passkey_registration: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if let Err(e) = session.insert(SESSIONKEY_PASSKEYREGISTRATION, passkey_reg) {
        tracing::error!("session.insert: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    Ok(Json(req_chal))
}

#[derive(Serialize, Deserialize)]
pub struct RegisterEndRequestPayload {
    name: String,
    credential: RegisterPublicKeyCredential,
}

#[debug_handler]
pub async fn register_end_handler(
    XRemoteUser(username): XRemoteUser,
    mut session: WritableSession,
    payload: extract::Json<RegisterEndRequestPayload>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<(), StatusCode> {
    tracing::trace!("register_end_handler");

    let app = shared_state.read().await;

    let passkey_reg = match session.get::<PasskeyRegistration>(SESSIONKEY_PASSKEYREGISTRATION) {
        Some(s) => s,
        _ => return Err(StatusCode::NOT_FOUND),
    };

    let passkey = match webauthn.finish_passkey_registration(&payload.credential, &passkey_reg) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("webauthn.finish_passkey_registration: {e}");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let user = app.get_user_with_credentials(username.clone()).await?;

    if user
        .credentials
        .iter()
        .any(|c| *c.credential.cred_id() == *passkey.cred_id())
    {
        tracing::error!("credential already registered");
        return Err(StatusCode::BAD_REQUEST);
    }

    app.add_credential(username, payload.name.clone(), passkey)
        .await?;

    session.remove(SESSIONKEY_PASSKEYREGISTRATION);

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct AuthenticateStartResponsePayload {
    challenge: Option<RequestChallengeResponse>,
}

#[debug_handler]
pub async fn authenticate_start_handler(
    XRemoteUser(username): XRemoteUser,
    mut session: WritableSession,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<AuthenticateStartResponsePayload>, StatusCode> {
    tracing::trace!("authenticate_start_handler");

    if session.get::<bool>(SESSIONKEY_LOGGEDIN).unwrap_or_default() {
        tracing::debug!("user already logged in");
        return Ok(Json(AuthenticateStartResponsePayload { challenge: None }));
    }

    let user = shared_state
        .read()
        .await
        .get_user_with_credentials(username.clone())
        .await?;

    if user.credentials.is_empty() {
        tracing::debug!("user does not have any credentials");
        if let Err(e) = session.insert(SESSIONKEY_LOGGEDIN, true) {
            tracing::error!("session.insert: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        return Ok(Json(AuthenticateStartResponsePayload { challenge: None }));
    }

    let passkeys: Vec<_> = user
        .credentials
        .iter()
        .map(|c| c.credential.to_owned())
        .collect();

    let (req_chal, passkey_auth) = match webauthn.start_passkey_authentication(&passkeys) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("webauthn.start_passkey_authentication: {e}");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    if let Err(e) = session.insert(SESSIONKEY_PASSKEYAUTHENTICATION, passkey_auth) {
        tracing::error!("session.insert: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(AuthenticateStartResponsePayload {
        challenge: Some(req_chal),
    }))
}

#[debug_handler]
pub async fn authenticate_end_handler(
    mut session: WritableSession,
    payload: extract::Json<PublicKeyCredential>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<(), StatusCode> {
    tracing::trace!("authenticate_end_handler");

    let passkey_authentication =
        match session.get::<PasskeyAuthentication>(SESSIONKEY_PASSKEYAUTHENTICATION) {
            Some(a) => a,
            None => return Err(StatusCode::NO_CONTENT),
        };

    let auth_result =
        match webauthn.finish_passkey_authentication(&payload.0, &passkey_authentication) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("webauthn.finish_passkey_authentication: {e}");
                return Err(StatusCode::UNAUTHORIZED);
            }
        };

    if auth_result.needs_update() {
        shared_state
            .read()
            .await
            .update_credential(auth_result)
            .await?;
    }

    session.remove(SESSIONKEY_PASSKEYAUTHENTICATION);
    if let Err(e) = session.insert(SESSIONKEY_LOGGEDIN, true) {
        tracing::error!("session.insert: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct CredentialIDWithName {
    id: String,
    name: String,
}

#[derive(Serialize, Deserialize)]
pub struct GetCredentialsResponsePayload {
    pub data: Vec<CredentialIDWithName>,
}

#[debug_handler]
pub async fn get_credentials_handler(
    XRemoteUser(username): XRemoteUser,
    shared_state: Extension<SharedAppState>,
) -> Result<Json<GetCredentialsResponsePayload>, StatusCode> {
    tracing::trace!("get_credentials_handler");

    let app = shared_state.read().await;
    let user = app.get_user_with_credentials(username).await?;
    Ok(Json(GetCredentialsResponsePayload {
        data: user
            .credentials
            .iter()
            .map(|c| {
                let c = c.clone();
                CredentialIDWithName {
                    id: c.credential.cred_id().to_string(),
                    name: c.name,
                }
            })
            .collect(),
    }))
}

#[debug_handler]
pub async fn delete_credentials_handler(
    Path(name): Path<String>,
    shared_state: Extension<SharedAppState>,
) -> Result<(), StatusCode> {
    tracing::trace!("delete_credentials_handler");

    let app = shared_state.read().await;
    Ok(app.delete_credential(name).await?)
}
