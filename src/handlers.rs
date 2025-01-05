use crate::app::{AppError, SharedAppState};
use argon2::{password_hash::PasswordHash, Argon2, PasswordVerifier};
use axum::{
    body::Body,
    extract::{self, ConnectInfo, FromRequestParts, Path, Query},
    http::{header, request::Parts, HeaderMap, Request, StatusCode, Uri},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    Extension, Json,
};
use axum_macros::debug_handler;
use base64::{engine::general_purpose, Engine as _};
use liquid::Template;
use metrics::counter;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tower_sessions::Session;
use tracing::{error, info, trace};
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

impl<S> FromRequestParts<S> for LoggedIn
where
    S: Send + Sync,
{
    type Rejection = (axum::http::StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        trace!("LoggedIn extractor");
        let session = Session::from_request_parts(parts, state).await?;
        Ok(LoggedIn(
            session
                .get::<bool>(SESSIONKEY_LOGGEDIN)
                .await
                .unwrap_or_default()
                .unwrap_or_default(),
        ))
    }
}

pub async fn require_logged_in(
    LoggedIn(logged_in): LoggedIn,
    req: Request<Body>,
    next: Next,
) -> Response {
    if logged_in {
        counter!("authorized_requests").increment(1);
        next.run(req).await
    } else {
        counter!("unauthorized_requests").increment(1);
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// Middleware that only allows connections from a loopback address. This first checks the client
/// address from the X-Forwarded-For header to determine if the request is coming from a local
/// client. If X-Forwarded-For is not present (i.e. the request is not coming from a proxy), then
/// the direct connection info is used.
pub async fn allow_only_localhost(
    connect_info: ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if req
        .headers()
        .get("x-forwarded-for")
        .map(|x_forwarded_for| {
            x_forwarded_for
                .to_str()
                .ok()
                .and_then(|s| s.split(',').next())
                .and_then(|s| s.trim().parse::<IpAddr>().ok())
                .filter(|ip| ip.to_canonical().is_loopback())
                .is_some()
        })
        .unwrap_or_else(|| connect_info.ip().to_canonical().is_loopback())
    {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

#[debug_handler]
pub async fn register_start_handler(
    session: Session,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<CreationChallengeResponse>, AppError> {
    trace!("register_start_handler");

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME).await? else {
        return Err(AppError::BadSession);
    };

    let app = shared_state.read().await;
    let user = app.get_user_with_credentials(username).await?;

    let existing_credentials: Vec<CredentialID> = user
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

    if let Err(e) = session
        .insert(SESSIONKEY_PASSKEYREGISTRATION, passkey_reg)
        .await
    {
        error!("session.insert: {e}");
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
    session: Session,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
    payload: extract::Json<RegisterEndRequestPayload>,
) -> Result<(), AppError> {
    trace!("register_end_handler");

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME).await? else {
        return Err(AppError::BadSession);
    };

    let app = shared_state.read().await;

    let Some(passkey_reg) = session
        .get::<PasskeyRegistration>(SESSIONKEY_PASSKEYREGISTRATION)
        .await?
    else {
        return Err(AppError::BadSession);
    };

    let Ok(passkey) = webauthn.finish_passkey_registration(&payload.credential, &passkey_reg)
    else {
        counter!("failed_registrations").increment(1);
        _ = session
            .remove::<PasskeyRegistration>(SESSIONKEY_PASSKEYREGISTRATION)
            .await?;

        return Err(AppError::WebauthnFailed);
    };

    let user = app.get_user_with_credentials(username.clone()).await?;

    if user
        .credentials
        .iter()
        .any(|c| *c.credential.cred_id() == *passkey.cred_id())
    {
        info!("credential already registered");
        return Err(AppError::DuplicateCredential);
    }

    app.add_credential(username, payload.name.clone(), &passkey)
        .await?;

    _ = session
        .remove::<PasskeyRegistration>(SESSIONKEY_PASSKEYREGISTRATION)
        .await?;

    counter!("successful_registrations").increment(1);

    Ok(())
}

#[debug_handler]
pub async fn authenticate_start_handler(
    session: Session,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
) -> Result<Json<RequestChallengeResponse>, AppError> {
    trace!("authenticate_start_handler");

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME).await? else {
        return Err(AppError::BadSession);
    };

    let user = shared_state
        .read()
        .await
        .get_user_with_credentials(username.clone())
        .await?;

    if user.credentials.is_empty() {
        info!("user does not have any credentials");
        if let Err(e) = session.insert(SESSIONKEY_LOGGEDIN, true).await {
            error!("session.insert: {e}");
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
        counter!("failed_authentications").increment(1);
        return Err(AppError::WebauthnFailed);
    };

    if let Err(e) = session
        .insert(SESSIONKEY_PASSKEYAUTHENTICATION, passkey_auth)
        .await
    {
        error!("session.insert: {e}");
        return Err(AppError::BadSession);
    }

    Ok(Json(req_chal))
}

#[debug_handler]
pub async fn authenticate_end_handler(
    session: Session,
    shared_state: Extension<SharedAppState>,
    webauthn: Extension<Arc<Webauthn>>,
    payload: extract::Json<PublicKeyCredential>,
) -> Result<(), AppError> {
    trace!("authenticate_end_handler");

    let Some(passkey_authentication) = session
        .get::<PasskeyAuthentication>(SESSIONKEY_PASSKEYAUTHENTICATION)
        .await?
    else {
        return Err(AppError::BadSession);
    };

    let Ok(auth_result) =
        webauthn.finish_passkey_authentication(&payload.0, &passkey_authentication)
    else {
        counter!("failed_authentications").increment(1);
        return Err(AppError::WebauthnFailed);
    };

    let state = shared_state.read().await;
    if auth_result.needs_update() {
        state.update_credential(auth_result).await?;
    }

    _ = session
        .remove::<PasskeyAuthentication>(SESSIONKEY_PASSKEYAUTHENTICATION)
        .await?;

    if let Err(e) = session.insert(SESSIONKEY_LOGGEDIN, true).await {
        error!("session.insert: {e}");
        return Err(AppError::BadSession);
    }

    counter!("successful_authentications").increment(1);

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct GetCredentialsResponsePayload {
    pub data: Vec<CredentialIDWithName>,
}

#[debug_handler]
pub async fn delete_credentials_api_handler(
    Path(cred_id): Path<CredentialID>,
    shared_state: Extension<SharedAppState>,
) -> Result<StatusCode, AppError> {
    trace!("delete_credentials_handler");

    let app = shared_state.read().await;
    app.delete_credential(cred_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn root_handler(uri: Uri) -> Response {
    match uri.path() {
        "/" => Redirect::permanent("/credentials").into_response(),
        "/favicon.ico" => Response::builder()
            .header(header::CONTENT_TYPE, "image/svg+xml")
            .body(Body::from(include_bytes!("./favicon.svg").as_slice()))
            .expect("could not build response"),
        "/main.js" => Response::builder()
            .header(header::CONTENT_TYPE, "text/javascript")
            .body(Body::from(include_bytes!("./main.js").as_slice()))
            .expect("could not build response"),
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .expect("could not build response"),
    }
}

pub struct Templates {
    pub credentials_template: Template,
    pub authenticate_template: Template,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CredentialIDWithName {
    id: CredentialID,
    name: String,
}

#[debug_handler]
pub async fn get_credentials_template_handler(
    LoggedIn(logged_in): LoggedIn,
    session: Session,
    templates: Extension<Arc<Templates>>,
    shared_state: Extension<SharedAppState>,
) -> Result<Response, AppError> {
    trace!("get_credentials_template_handler");

    let app = shared_state.read().await;

    if !logged_in {
        return Ok(Redirect::temporary("/authenticate?redirect_url=/credentials").into_response());
    }

    let Some(username) = session.get::<String>(SESSIONKEY_USERNAME).await? else {
        return Err(AppError::BadSession);
    };

    let user = app.get_user_with_credentials(username).await?;
    let credentials: Vec<CredentialIDWithName> = user
        .credentials
        .iter()
        .map(|c| {
            let c = c.clone();
            CredentialIDWithName {
                id: c.credential.cred_id().to_owned(),
                name: c.name,
            }
        })
        .collect();

    let tmpl_data = liquid::object!({ "credentials": credentials });

    match templates.credentials_template.render(&tmpl_data) {
        Ok(html) => Ok(Html(finish_html(html)).into_response()),
        Err(e) => {
            error!("templates.credentials_template.render: {e}");
            Err(AppError::UnknownError)
        }
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
    session: Session,
    templates: Extension<Arc<Templates>>,
    webauthn: Extension<Arc<Webauthn>>,
    passwords: Extension<HashMap<String, String>>,
) -> Result<Response, AppError> {
    trace!("get_authenticate_template_handler");

    if logged_in {
        if let Some(redirect_url) = session.get::<String>(SESSIONKEY_REDIRECTURL).await? {
            _ = session.remove::<String>(SESSIONKEY_REDIRECTURL).await?;
            return Ok(Redirect::temporary(&redirect_url).into_response());
        }
    }

    let needs_basic_auth_response = Response::builder()
        .header(header::WWW_AUTHENTICATE, "Basic")
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::empty())
        .expect("could not build response");
    let Some(authorization) = headers.get("Authorization") else {
        return Ok(needs_basic_auth_response);
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
        return Ok(needs_basic_auth_response);
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
        return Ok((
            StatusCode::UNAUTHORIZED,
            Html(finish_html(String::from(
                "<main><p>Unauthorized</p></main>",
            ))),
        )
            .into_response());
    }

    session
        .insert(SESSIONKEY_USERNAME, username.clone())
        .await?;

    if !logged_in {
        if let Some(redirect_url) = params.redirect_url.as_ref() {
            if let Ok(accepted_redirect_url) =
                get_redirect_url(redirect_url.to_string(), webauthn.get_allowed_origins())
            {
                session
                    .insert(SESSIONKEY_REDIRECTURL, accepted_redirect_url)
                    .await?;
            }
        }
    }

    let tmpl_data = liquid::object!({ "username": username, "logged_in": logged_in });
    match templates.authenticate_template.render(&tmpl_data) {
        Ok(html) => Ok(Html(finish_html(html)).into_response()),
        Err(e) => {
            error!("parsed_template.render: {e}");
            Err(AppError::UnknownError)
        }
    }
}

const TOP_HTML: &str = r#"
<!DOCTYPE html>
<head>
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
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
