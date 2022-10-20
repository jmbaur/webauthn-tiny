use crate::app::{self, AppError};
use app::SharedAppState;
use async_trait::async_trait;
use axum::{
    body::{boxed, Full},
    extract::{self, FromRequest, Path, Query, RequestParts},
    http::{header, HeaderMap, Request, StatusCode, Uri},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    Extension, Json,
};
use axum_macros::debug_handler;
use axum_sessions::extractors::{ReadableSession, WritableSession};
use metrics::increment_counter;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use webauthn_rs::{prelude::*, Webauthn};
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
                increment_counter!("authorized_requests");
                Ok(next.run(req).await)
            } else {
                increment_counter!("unauthorized_requests");
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
) -> Result<Json<CreationChallengeResponse>, AppError> {
    tracing::trace!("register_start_handler");

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return Err(AppError::BadSession),
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
            return Err(AppError::WebauthnFailed);
        }
    };

    if let Err(e) = session.insert(SESSIONKEY_PASSKEYREGISTRATION, passkey_reg) {
        tracing::error!("session.insert: {e}");
        return Err(AppError::BadSession);
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
) -> Result<(), AppError> {
    tracing::trace!("register_end_handler");

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return Err(AppError::BadSession),
    };

    let app = shared_state.read().await;

    let passkey_reg = match session.get::<PasskeyRegistration>(SESSIONKEY_PASSKEYREGISTRATION) {
        Some(s) => s,
        _ => return Err(AppError::BadSession),
    };

    let passkey = match webauthn.finish_passkey_registration(&payload.credential, &passkey_reg) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("webauthn.finish_passkey_registration: {e}");
            increment_counter!("failed_webauthn_registrations");
            session.remove(SESSIONKEY_PASSKEYREGISTRATION);
            return Err(AppError::WebauthnFailed);
        }
    };

    let user = app.get_user_with_credentials(username.clone()).await?;

    if user
        .credentials
        .iter()
        .any(|c| *c.credential.cred_id() == *passkey.cred_id())
    {
        tracing::info!("credential already registered");
        return Err(AppError::DuplicateCredential);
    }

    app.add_credential(username, payload.name.clone(), passkey)
        .await?;

    session.remove(SESSIONKEY_PASSKEYREGISTRATION);

    increment_counter!("successful_webauthn_registrations");

    Ok(())
}

#[debug_handler]
pub async fn authenticate_start_handler(
    mut session: WritableSession,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<RequestChallengeResponse>, AppError> {
    tracing::trace!("authenticate_start_handler");

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return Err(AppError::BadSession),
    };

    let user = shared_state
        .read()
        .await
        .get_user_with_credentials(username.clone())
        .await?;

    if user.credentials.is_empty() {
        tracing::info!("user does not have any credentials");
        if let Err(e) = session.insert(SESSIONKEY_LOGGEDIN, true) {
            tracing::error!("session.insert: {e}");
            return Err(AppError::BadSession);
        }
        return Err(AppError::CredentialNotFound);
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
            increment_counter!("failed_webauthn_authentications");
            return Err(AppError::WebauthnFailed);
        }
    };

    if let Err(e) = session.insert(SESSIONKEY_PASSKEYAUTHENTICATION, passkey_auth) {
        tracing::error!("session.insert: {e}");
        return Err(AppError::BadSession);
    }

    Ok(Json(req_chal))
}

#[debug_handler]
pub async fn authenticate_end_handler(
    mut session: WritableSession,
    payload: extract::Json<PublicKeyCredential>,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<(), AppError> {
    tracing::trace!("authenticate_end_handler");

    let passkey_authentication =
        match session.get::<PasskeyAuthentication>(SESSIONKEY_PASSKEYAUTHENTICATION) {
            Some(a) => a,
            None => return Err(AppError::BadSession),
        };

    let auth_result =
        match webauthn.finish_passkey_authentication(&payload.0, &passkey_authentication) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("webauthn.finish_passkey_authentication: {e}");
                increment_counter!("failed_webauthn_authentications");
                return Err(AppError::WebauthnFailed);
            }
        };

    let state = shared_state.read().await;
    if auth_result.needs_update() {
        state.update_credential(auth_result).await?;
    }

    session.remove(SESSIONKEY_PASSKEYAUTHENTICATION);
    if let Err(e) = session.insert(SESSIONKEY_LOGGEDIN, true) {
        tracing::error!("session.insert: {e}");
        return Err(AppError::BadSession);
    }

    increment_counter!("successful_webauthn_authentications");

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
) -> Result<StatusCode, AppError> {
    tracing::trace!("delete_credentials_handler");

    let app = shared_state.read().await;
    app.delete_credential(cred_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(RustEmbed)]
#[folder = "$ASSETS_DIRECTORY"]
pub struct Assets;

pub struct AssetFile<T>(pub T);

impl<T> IntoResponse for AssetFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.0.into();

        match Assets::get(path.as_str()) {
            Some(content) => {
                let body = boxed(Full::from(content.data));
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                Response::builder()
                    .header(header::CONTENT_TYPE, mime.as_ref())
                    .body(body)
                    .expect("could not build response")
            }
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(boxed(Full::from("404")))
                .expect("could not build response"),
        }
    }
}

pub async fn root_handler(uri: Uri) -> Response {
    let path = uri.path().to_string();
    if path == "/" {
        Redirect::permanent("/credentials").into_response()
    } else {
        AssetFile(path.trim_start_matches('/')).into_response()
    }
}

#[derive(RustEmbed)]
#[folder = "templates"]
pub struct Templates;

#[derive(Serialize, Deserialize, Debug)]
pub struct CredentialIDWithName {
    id: String,
    name: String,
}

#[debug_handler]
pub async fn get_credentials_template_handler(
    LoggedIn(logged_in): LoggedIn,
    session: ReadableSession,
    parser: Extension<Arc<liquid::Parser>>,
    shared_state: Extension<SharedAppState>,
) -> Response {
    tracing::trace!("get_credentials_template_handler");

    let app = shared_state.read().await;

    if !logged_in {
        return Redirect::temporary("/authenticate?redirect_url=/credentials").into_response();
    }

    let username = match session.get::<String>(SESSIONKEY_USERNAME) {
        Some(u) => u,
        None => return AppError::BadSession.into_response(),
    };

    if let Some(template) = Templates::get("credentials.liquid") {
        let parsed_template =
            match parser.parse(std::str::from_utf8(&template.data).unwrap_or_default()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("parser.parse: {e}");
                    return AppError::UnknownError.into_response();
                }
            };

        return match app.get_user_with_credentials(username).await {
            Ok(user) => {
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

                return match parsed_template.render(&tmpl_data) {
                    Ok(html) => Html(html).into_response(),
                    Err(e) => {
                        tracing::error!("parsed_template.render: {e}");
                        AppError::UnknownError.into_response()
                    }
                };
            }
            Err(e) => {
                tracing::error!("app.get_user_with_credentials: {e}");
                e.into_response()
            }
        };
    } else {
        AppError::UnknownError.into_response()
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
) -> Result<Html<String>, AppError> {
    tracing::trace!("get_authenticate_template_handler");
    let username = match headers.get("X-Remote-User") {
        Some(h) => {
            if let Ok(val) = h.to_str() {
                val.to_string()
            } else {
                tracing::error!(
                    "could not convert X-Remote-User header value '{:#?}' to string",
                    h
                );
                return Err(AppError::BadInput);
            }
        }
        None => return Err(AppError::MissingUserInfo),
    };

    if let Err(e) = session.insert(SESSIONKEY_USERNAME, username.clone()) {
        tracing::error!("session.insert: {e}");
        return Err(AppError::BadSession);
    }

    if let Some(template) = Templates::get("authenticate.liquid") {
        let parsed_template =
            match parser.parse(std::str::from_utf8(&template.data).unwrap_or_default()) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("parser.parse: {e}");
                    return Err(AppError::UnknownError);
                }
            };
        let tmpl_data = liquid::object!({
            "username": username,
            "logged_in": logged_in,
        });
        if let Some(redirect_url) = &params.redirect_url {
            if let Ok(accepted_redirect_url) =
                get_redirect_url(redirect_url.to_string(), webauthn.get_allowed_origins())
            {
                if !logged_in {
                    if let Err(e) = session.insert(SESSIONKEY_REDIRECTURL, accepted_redirect_url) {
                        tracing::error!("session.insert: {e}");
                        return Err(AppError::BadSession);
                    }
                }
            }
        }
        match parsed_template.render(&tmpl_data) {
            Ok(html) => Ok(Html(html)),
            Err(e) => {
                tracing::error!("parsed_template.render: {e}");
                Err(AppError::UnknownError)
            }
        }
    } else {
        Err(AppError::UnknownError)
    }
}

fn get_redirect_url(requested_url: String, allowed_origins: &[Url]) -> anyhow::Result<String> {
    if let Ok(url) = Url::parse(&requested_url) {
        if allowed_origins.iter().any(|u| u.origin() == url.origin()) {
            Ok(requested_url)
        } else {
            anyhow::bail!("origin not allowed")
        }
    } else if requested_url.starts_with('/') {
        Ok(requested_url)
    } else {
        anyhow::bail!("bad url")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webauthn_rs::WebauthnBuilder;

    #[test]
    fn test_get_redirect_url() {
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
            "/somepath",
            "https://bar.foo.com",
            "https://auth.foo.com",
            "https://foo.com",
        ]
        .iter()
        .for_each(|&url| {
            assert_eq!(
                url,
                get_redirect_url(url.to_string(), webauthn.get_allowed_origins()).unwrap(),
                "url not accepted by get_redirect_url: {}",
                url
            );
        });

        // fails
        ["https://fo.com", "https://foo.bar.com"]
            .iter()
            .for_each(|&url| {
                assert!(
                    get_redirect_url(url.to_string(), webauthn.get_allowed_origins()).is_err(),
                    "url accepted by get_redirect_url: {}",
                    url
                );
            });
    }
}
