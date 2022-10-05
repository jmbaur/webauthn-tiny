use crate::app;
use app::SharedAppState;
use async_trait::async_trait;
use axum::{
    extract::{self, FromRequest, Path, Query, RequestParts},
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    Extension, Json,
};
use axum_macros::debug_handler;
use axum_sessions::extractors::{ReadableSession, WritableSession};
use rust_embed::RustEmbed;
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
const SESSIONKEY_REDIRECTURL: &str = "redirect_url";
const SESSIONKEY_USERNAME: &str = "username";

pub struct LoggedIn(bool);

#[async_trait]
impl<B> FromRequest<B> for LoggedIn
where
    B: Send,
{
    type Rejection = StatusCode;
    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        tracing::trace!("LoggedIn extractor");
        if let Ok(session) = ReadableSession::from_request(req).await {
            if session.get::<bool>(SESSIONKEY_LOGGEDIN).unwrap_or_default() {
                return Ok(LoggedIn(true));
            }
        }
        Ok(LoggedIn(false))
    }
}

pub async fn require_logged_in<B>(req: Request<B>, next: Next<B>) -> Result<Response, StatusCode>
where
    B: Send,
{
    let mut request_parts = RequestParts::new(req);
    match LoggedIn::from_request(&mut request_parts).await {
        Ok(LoggedIn(logged_in)) => {
            if logged_in {
                let req = request_parts.try_into_request().expect("body extracted");
                Ok(next.run(req).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        Err(_) => unreachable!(),
    }
}

/// redirector expects there to be an existing (validated) redirect URL in the session's data.
pub async fn redirector<B>(req: Request<B>, next: Next<B>) -> Result<Response, StatusCode>
where
    B: Send,
{
    let mut request_parts = RequestParts::new(req);
    match LoggedIn::from_request(&mut request_parts).await {
        Ok(LoggedIn(logged_in)) => {
            if logged_in {
                if let Ok(mut session) = WritableSession::from_request(&mut request_parts).await {
                    if let Some(redirect_url) = session.get::<String>(SESSIONKEY_REDIRECTURL) {
                        session.remove(SESSIONKEY_REDIRECTURL);
                        return Ok(Redirect::temporary(&redirect_url).into_response());
                    }
                }
            }
            let req = request_parts.try_into_request().expect("body extracted");
            Ok(next.run(req).await)
        }
        Err(_) => unreachable!(),
    }
}

#[debug_handler]
pub async fn register_start_handler(
    mut session: WritableSession,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<CreationChallengeResponse>, StatusCode> {
    tracing::trace!("register_start_handler");

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

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
    mut session: WritableSession,
    payload: extract::Json<RegisterEndRequestPayload>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<(), StatusCode> {
    tracing::trace!("register_end_handler");

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

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

#[debug_handler]
pub async fn authenticate_start_handler(
    mut session: WritableSession,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<RequestChallengeResponse>, StatusCode> {
    tracing::trace!("authenticate_start_handler");

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

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
        return Err(StatusCode::NO_CONTENT);
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

    Ok(Json(req_chal))
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

    let state = shared_state.read().await;
    if auth_result.needs_update() {
        state.update_credential(auth_result).await?;
    }

    session.remove(SESSIONKEY_PASSKEYAUTHENTICATION);
    if let Err(e) = session.insert(SESSIONKEY_LOGGEDIN, true) {
        tracing::error!("session.insert: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct GetCredentialsResponsePayload {
    pub data: Vec<CredentialIDWithName>,
}

#[debug_handler]
pub async fn delete_credentials_api_handler(
    Path(cred_id): Path<String>,
    shared_state: Extension<SharedAppState>,
) -> Result<StatusCode, StatusCode> {
    tracing::trace!("delete_credentials_handler");

    let app = shared_state.read().await;
    app.delete_credential(cred_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(RustEmbed)]
#[folder = "templates"]
struct Templates;

#[derive(Serialize, Deserialize, Debug)]
pub struct CredentialIDWithName {
    id: String,
    name: String,
}

#[debug_handler]
pub async fn get_credentials_template_handler(
    session: ReadableSession,
    parser: Extension<Arc<liquid::Parser>>,
    shared_state: Extension<SharedAppState>,
) -> Result<Html<String>, StatusCode> {
    tracing::trace!("get_credentials_template_handler");

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if let Some(template) = Templates::get("credentials.liquid") {
        let parsed_template =
            match parser.parse(std::str::from_utf8(&template.data).unwrap_or_default()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("parser.parse: {e}");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };

        let app = shared_state.read().await;
        if let Ok(user) = app.get_user_with_credentials(username).await {
            let credentials: Vec<CredentialIDWithName> = user
                .credentials
                .iter()
                .map(|c| {
                    let c = c.clone();
                    CredentialIDWithName {
                        id: c.credential.cred_id().to_string(),
                        name: c.name,
                    }
                })
                .collect();

            let tmpl_data = liquid::object!({ "credentials": credentials });
            if let Ok(output) = parsed_template.render(&tmpl_data) {
                Ok(Html(output))
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        } else {
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Deserialize)]
pub struct GetAuthenticateQueryParams {
    pub redirect_url: Option<String>,
}

#[debug_handler]
pub async fn get_authenticate_template_handler(
    LoggedIn(logged_in): LoggedIn,
    params: Query<GetAuthenticateQueryParams>,
    headers: HeaderMap,
    mut session: WritableSession,
    parser: Extension<Arc<liquid::Parser>>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Html<String>, StatusCode> {
    tracing::trace!("get_authenticate_template_handler");
    let username = match headers.get("X-Remote-User") {
        Some(h) => {
            if let Ok(val) = h.to_str() {
                String::from(val)
            } else {
                tracing::error!(
                    "could not convert X-Remote-User header value '{:#?}' to string",
                    h
                );
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    if let Err(e) = session.insert(SESSIONKEY_USERNAME, username.clone()) {
        tracing::error!("session.insert: {e}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    if let Some(template) = Templates::get("authenticate.liquid") {
        let parsed_template =
            match parser.parse(std::str::from_utf8(&template.data).unwrap_or_default()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("parser.parse: {e}");
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };
        let tmpl_data = liquid::object!({
            "username": username,
            "logged_in": logged_in,
        });
        if let Some(redirect_url) = &params.redirect_url {
            let url = match url::Url::parse(redirect_url) {
                Ok(u) => u,
                Err(e) => {
                    tracing::error!("url::Url::parse: {e}");
                    return Err(StatusCode::BAD_REQUEST);
                }
            };
            if !webauthn
                .get_allowed_origins()
                .iter()
                .any(|u| u.origin() == url.origin())
            {
                tracing::info!("denied client request for redirect to {}", redirect_url);
                return Err(StatusCode::FORBIDDEN);
            }
            if let Err(e) = session.insert(SESSIONKEY_REDIRECTURL, redirect_url) {
                tracing::error!("session.insert: {e}");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
        if let Ok(output) = parsed_template.render(&tmpl_data) {
            Ok(Html(output))
        } else {
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[cfg(test)]
mod tests {
    use webauthn_rs::prelude::*;
    use webauthn_rs::WebauthnBuilder;

    #[test]
    fn exploration() {
        let webauthn =
            WebauthnBuilder::new("foo.com", &Url::parse("https://auth.foo.com").unwrap())
                .unwrap()
                .allow_subdomains(true)
                .append_allowed_origin(&Url::parse("https://foo.com").unwrap())
                .append_allowed_origin(&Url::parse("https://bar.foo.com").unwrap())
                .build()
                .unwrap();

        // passes
        [
            "https://bar.foo.com",
            "https://auth.foo.com",
            "https://foo.com",
        ]
        .iter()
        .for_each(|&url| {
            assert!(webauthn
                .get_allowed_origins()
                .iter()
                .any(|u| u.origin() == Url::parse(url).unwrap().origin()));
        });

        // fails
        ["https://fo.com", "https://foo.bar.com"]
            .iter()
            .for_each(|&url| {
                assert!(webauthn
                    .get_allowed_origins()
                    .iter()
                    .any(|u| u.origin() != Url::parse(url).unwrap().origin()));
            });
    }
}
