use crate::app::{self, AppError};
use app::SharedAppState;
use argon2::PasswordVerifier;
use argon2::{password_hash::PasswordHash, Argon2};
use async_trait::async_trait;
use axum::{
    body::{boxed, Empty, Full},
    extract::{self, FromRequestParts, Path, Query},
    http::{header, request::Parts, HeaderMap, Request, StatusCode, Uri},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    Extension, Json,
};
use axum_macros::debug_handler;
use axum_sessions::extractors::{ReadableSession, WritableSession};
use base64::{engine::general_purpose, Engine as _};
use liquid::Template;
use metrics::increment_counter;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::{collections::HashMap, convert::Infallible, sync::Arc};
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
impl<B> FromRequestParts<B> for LoggedIn
where
    B: Send + std::marker::Sync,
{
    type Rejection = Infallible;
    async fn from_request_parts(parts: &mut Parts, state: &B) -> Result<Self, Self::Rejection> {
        tracing::trace!("LoggedIn extractor");
        let session = ReadableSession::from_request_parts(parts, state).await?;
        Ok(LoggedIn(
            session.get::<bool>(SESSIONKEY_LOGGEDIN).unwrap_or_default(),
        ))
    }
}

pub async fn require_logged_in<B>(
    LoggedIn(logged_in): LoggedIn,
    req: Request<B>,
    next: Next<B>,
) -> Response {
    if logged_in {
        increment_counter!("authorized_requests");
        next.run(req).await
    } else {
        increment_counter!("unauthorized_requests");
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// Middleware that only allows connections from a loopback address. NOTE: This assumes the server
/// is running behind a reverse proxy (and is only safe if it this is the case).
pub async fn allow_only_localhost<B>(req: Request<B>, next: Next<B>) -> Response {
    if req
        .headers()
        .get("x-forwarded-for")
        .and_then(|s| s.to_str().ok())
        .and_then(|s| {
            s.split(',')
                .last()
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
        })
        .filter(|ip| ip.is_loopback())
        .is_some()
    {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

#[debug_handler]
pub async fn register_start_handler(
    mut session: WritableSession,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<CreationChallengeResponse>, AppError> {
    tracing::trace!("register_start_handler");

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME) else {
        return Err(AppError::BadSession);
    };

    let app = shared_state.read().await;
    let user = app.get_user_with_credentials(username).await?;

    let existing_credentials: Vec<Base64UrlSafeData> = user
        .credentials
        .iter()
        .map(|c| c.credential.cred_id().to_owned())
        .collect();

    let Ok((req_chal, passkey_reg)) = webauthn.start_passkey_registration(
        user.id,
        &user.username,
        &user.username, // use username as display name
        if existing_credentials.is_empty() {
            None
        } else {
            Some(existing_credentials)
        },
    ) else {
        return Err(AppError::WebauthnFailed);
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
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
    payload: extract::Json<RegisterEndRequestPayload>,
) -> Result<(), AppError> {
    tracing::trace!("register_end_handler");

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME) else {
        return Err(AppError::BadSession);
    };

    let app = shared_state.read().await;

    let Some(passkey_reg) = session.get::<PasskeyRegistration>(SESSIONKEY_PASSKEYREGISTRATION)
    else {
        return Err(AppError::BadSession);
    };

    let Ok(passkey) = webauthn.finish_passkey_registration(&payload.credential, &passkey_reg)
    else {
        increment_counter!("failed_webauthn_registrations");
        session.remove(SESSIONKEY_PASSKEYREGISTRATION);
        return Err(AppError::WebauthnFailed);
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

    app.add_credential(username, payload.name.clone(), &passkey)
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

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME) else {
        return Err(AppError::BadSession);
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
        return Err(AppError::NoUserCredentials);
    }

    let passkeys: Vec<_> = user
        .credentials
        .iter()
        .map(|c| c.credential.to_owned())
        .collect();

    let Ok((req_chal, passkey_auth)) = webauthn.start_passkey_authentication(&passkeys) else {
        increment_counter!("failed_webauthn_authentications");
        return Err(AppError::WebauthnFailed);
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
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
    payload: extract::Json<PublicKeyCredential>,
) -> Result<(), AppError> {
    tracing::trace!("authenticate_end_handler");

    let Some(passkey_authentication) =
        session.get::<PasskeyAuthentication>(SESSIONKEY_PASSKEYAUTHENTICATION)
    else {
        return Err(AppError::BadSession);
    };

    let Ok(auth_result) =
        webauthn.finish_passkey_authentication(&payload.0, &passkey_authentication)
    else {
        increment_counter!("failed_webauthn_authentications");
        return Err(AppError::WebauthnFailed);
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

const MAIN_JS: &str = include_str!("./main.js");

pub async fn root_handler(uri: Uri) -> Response {
    match uri.path() {
        "/" => Redirect::permanent("/credentials").into_response(),
        "/main.js" => Response::builder()
            .header(header::CONTENT_TYPE, "text/javascript")
            .body(boxed(MAIN_JS.to_string()))
            .expect("could not build response"),
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(boxed(Full::from("404")))
            .expect("could not build response"),
    }
}

pub struct Templates {
    pub credentials_template: Template,
    pub authenticate_template: Template,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CredentialIDWithName {
    id: String,
    name: String,
}

#[debug_handler]
pub async fn get_credentials_template_handler(
    LoggedIn(logged_in): LoggedIn,
    session: ReadableSession,
    templates: Extension<Arc<Templates>>,
    shared_state: Extension<SharedAppState>,
) -> Response {
    tracing::trace!("get_credentials_template_handler");

    let app = shared_state.read().await;

    if !logged_in {
        return Redirect::temporary("/authenticate?redirect_url=/credentials").into_response();
    }

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME) else {
        return AppError::BadSession.into_response();
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

            return match templates.credentials_template.render(&tmpl_data) {
                Ok(html) => Html(finish_html(html)).into_response(),
                Err(e) => {
                    tracing::error!("templates.credentials_template.render: {e}");
                    AppError::UnknownError.into_response()
                }
            };
        }
        Err(e) => {
            tracing::error!("app.get_user_with_credentials: {e}");
            e.into_response()
        }
    };
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
    templates: Extension<Arc<Templates>>,
    webauthn: Extension<Arc<Webauthn>>,
    passwords: Extension<HashMap<String, String>>,
) -> Response {
    tracing::trace!("get_authenticate_template_handler");

    if logged_in {
        if let Some(redirect_url) = session.get::<String>(SESSIONKEY_REDIRECTURL) {
            session.remove(SESSIONKEY_REDIRECTURL);
            return Redirect::temporary(&redirect_url).into_response();
        }
    }

    let needs_basic_auth_response = Response::builder()
        .header(header::WWW_AUTHENTICATE, "Basic")
        .status(StatusCode::UNAUTHORIZED)
        .body(boxed(Empty::new()))
        .expect("could not build response");
    let Some(authorization) = headers.get("Authorization") else {
        return needs_basic_auth_response;
    };

    let Some((username, password)) = authorization
        .to_str()
        .ok()
        .and_then(|authorization_header| {
            general_purpose::STANDARD
                .decode(authorization_header.trim_start_matches("Basic "))
                .ok()
                .and_then(|decoded_auth| {
                    String::from_utf8(decoded_auth).ok().and_then(|str_auth| {
                        str_auth
                            .split_once(':')
                            .map(|(u, p)| (String::from(u), String::from(p)))
                    })
                })
        })
    else {
        return needs_basic_auth_response;
    };

    if passwords
        .get(&username)
        .and_then(|hashed_password| {
            PasswordHash::new(hashed_password)
                .ok()
                .and_then(|parsed_hash| {
                    Argon2::default()
                        .verify_password(password.as_bytes(), &parsed_hash)
                        .ok()
                })
        })
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Html(finish_html(String::from(
                "<main><p>Unauthorized</p></main>",
            ))),
        )
            .into_response();
    }

    if let Err(e) = session.insert(SESSIONKEY_USERNAME, username.clone()) {
        tracing::error!("session.insert: {e}");
        return AppError::BadSession.into_response();
    }

    if !logged_in {
        if let Some(app_error) = params.redirect_url.as_ref().and_then(|redirect_url| {
            get_redirect_url(redirect_url.to_string(), webauthn.get_allowed_origins())
                .ok()
                .and_then(|accepted_redirect_url| {
                    session
                        .insert(SESSIONKEY_REDIRECTURL, accepted_redirect_url)
                        .err()
                        .and(Some(AppError::BadSession))
                })
        }) {
            return app_error.into_response();
        }
    }

    let tmpl_data = liquid::object!({ "username": username, "logged_in": logged_in });
    match templates.authenticate_template.render(&tmpl_data) {
        Ok(html) => Html(finish_html(html)).into_response(),
        Err(e) => {
            tracing::error!("parsed_template.render: {e}");
            AppError::UnknownError.into_response()
        }
    }
}

const TOP_HTML: &str = r#"
<!DOCTYPE html>
<head>
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <link rel="icon" type="image/png" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAMAAABEpIrGAAAABGdBTUEAALGPC/xhBQAAACBjSFJNAAB6JgAAgIQAAPoAAACA6AAAdTAAAOpgAAA6mAAAF3CculE8AAABp1BMVEUAAAC7ZEnAaE3BaE3AaE7AaE7AZ07AZ07AaEuqVVXAZ03AaU7AaE7BaE7AaE7AZ06/X0+iubmouMCotcKot7+jtr+qVQDAaE3BZ06ZZjO/aE3DaUv///+pt8Cpt8KquMGpt8Gpt8KqtsHAaVDBZ07DaUvAZ06ptsKpuMHAa1LAaE7BaEzAZUyotsHBaE3AaU3AaE7AaE6puMGpt8Kpt8Cotb6qtsCpuMKpuMHAaE7/AACpt7+puMKls8Glsr+/b1bJXUOpt8GquMGotsDBaE7CYUiqtL+otsHAaE3/fwCntsGotcHBZ02/aE2qtsKqt8DEYk7AaE7AZ0vAZ06pqbPAaE7/qjHzrEb4qzr/qjL8rDbBaE67ZkT/qi//qzLAZ03BZUr+rDLBaE7+qzLBaE7AaE3BaU7BaE7BaE+/Z0zAaE3AaU/BZkzBaE3AaE7+rDLAaE3+rDK/aE3+qy7/rDLAaE7+rDH9qTPGbUvBaE7AaE7EaE7BaU+quMKsrrS0kIi6e2q4gnTqrlTrrlPrrVHdiEHCak7/rDP1oTbPeUfchkLljz3///9akrvWAAAAfHRSTlMAJo7S9fndoD0Dh/rk1/6sEBZBUEQcA6uzBYERAVbF/f7PY4JbIrMVu/6ja0nYy7v9sLz7pDtRmPbwAVnnNijxE8r6NdsVGKLyAkZTebJUORr2Snsb0CSx0sXLqQ8w8eI6vMb4w3Pk4/uFxfVX7zH8wdG1Uv3E0tuz980nD8+cyQAAAAFiS0dEHJwEQQcAAAFGSURBVDjLpZBlUwMxEIYXisvhTqG4U9zd3Z1Ci7u2eIIVOORPk9wlQHp0MgPvl53Z55ndZAGM8fE1+fkHBAbB7wkOCUUoLFxBSkSkAKKiY2LjAOITEEKJSZBMiinli6aa09IxxpaMzCwCUDZADq25nOflY5bbAgoKAYpoVYrZ9BLO8R3SUmot02q5Pr+CkMoqc3VNbd09ElLfQIVGwpuaNbelVRSUNtptx7ijk72mSxRQN+n1WDDu5c+1iryvn/QGyIZBLgyJwjDtjRBhlAtj44IwYRBg8ifXDyUKUw/ffFo79czso9s9N8+z8PT8wr64aKN8aVn1zOsb5XaHPnJFNeadCqts55pMUP8srEsEZUMibIJE2JIJ2zv//sWuRNjblwgHh0w48rbimAkn3oRTJjhdRuGD8DMbP4Tz3HOL6+Ly6vqG4U+ir1fMK9R+pgAAAABJRU5ErkJggg==">
  <script type="module" src="/main.js" defer></script>
  <title>WebAuthnTiny</title>
</head>
<html>
<body>
"#;

const BOTTOM_HTML: &str = r#"
</body>
</html>
"#;

fn finish_html(page_html: String) -> String {
    format!("{}{}{}", TOP_HTML, page_html, BOTTOM_HTML)
}

fn get_redirect_url(requested_url: String, allowed_origins: &[Url]) -> Result<String, AppError> {
    if let Ok(url) = Url::parse(&requested_url) {
        if allowed_origins.iter().any(|u| u.origin() == url.origin()) {
            Ok(requested_url)
        } else {
            Err(AppError::OriginNotAllowed)
        }
    } else if requested_url.starts_with('/') {
        Ok(requested_url)
    } else {
        Err(AppError::BadUrl)
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
