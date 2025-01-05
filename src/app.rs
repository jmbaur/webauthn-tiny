use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use libsqlite3_sys::ErrorCode::ConstraintViolation;
use rusqlite::Error::{QueryReturnedNoRows, SqliteFailure};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, sync::Arc};
use tokio::sync::RwLock;
use tokio_rusqlite::Connection;
use webauthn_rs::prelude::{AuthenticationResult, CredentialID, Passkey, Uuid};

#[derive(Deserialize, Serialize, Debug)]
pub struct CredentialState {
    pub id: Uuid,
    pub credentials: Vec<Passkey>,
}

#[derive(Debug, Copy, Clone)]
pub enum AppError {
    MissingUserInfo,
    UserNotFound,
    CredentialNotFound,
    BadUrl,
    OriginNotAllowed,
    MismatchingCredential,
    DuplicateCredential,
    BadInput,
    EntityNotFound,
    BadSession,
    WebauthnFailed,
    UnknownError,
    NoUserCredentials,
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            AppError::MissingUserInfo => "user info is missing",
            AppError::BadInput => "bad input",
            AppError::EntityNotFound => "could not find data",
            AppError::BadSession => "session is invalid",
            AppError::DuplicateCredential => "credential already exists",
            AppError::MismatchingCredential => "incorrect credential used",
            AppError::CredentialNotFound => "credential not found",
            AppError::WebauthnFailed => "webauthn process failed",
            AppError::UserNotFound => "user not found",
            _ => "unknown error",
        };
        write!(f, "{msg}")
    }
}

impl std::error::Error for AppError {}

impl Default for AppError {
    fn default() -> Self {
        AppError::UnknownError
    }
}

#[derive(Serialize)]
struct AppErrorResponse {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::from(self),
            Json(AppErrorResponse {
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}

impl From<AppError> for StatusCode {
    fn from(error: AppError) -> Self {
        eprintln!("{:#?}", error);
        match error {
            AppError::BadInput => StatusCode::BAD_REQUEST,
            AppError::UserNotFound => StatusCode::NOT_FOUND,
            AppError::CredentialNotFound => StatusCode::NOT_FOUND,
            AppError::NoUserCredentials => StatusCode::NO_CONTENT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        match error {
            SqliteFailure(err, _) => match err.code {
                ConstraintViolation => AppError::BadInput,
                _ => AppError::UnknownError,
            },
            QueryReturnedNoRows => AppError::EntityNotFound,
            _ => AppError::UnknownError,
        }
    }
}

impl From<tokio_rusqlite::Error> for AppError {
    fn from(error: tokio_rusqlite::Error) -> Self {
        match error {
            tokio_rusqlite::Error::Rusqlite(error) => error.into(),
            _ => AppError::UnknownError,
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(_error: serde_json::Error) -> Self {
        AppError::BadInput
    }
}

impl From<uuid::Error> for AppError {
    fn from(_error: uuid::Error) -> Self {
        AppError::BadInput
    }
}

pub struct App {
    db: Connection,
}

pub type SharedAppState = Arc<RwLock<App>>;

#[derive(Clone, Debug)]
pub struct CredentialWithName {
    pub name: String,
    pub credential: Passkey,
}

#[derive(Default, Debug, Clone)]
pub struct UserWithCredentials {
    pub id: Uuid,
    pub username: String,
    pub credentials: Vec<CredentialWithName>,
}

impl UserWithCredentials {
    fn exists(&self) -> bool {
        self.id != Uuid::default()
    }
}

impl App {
    pub fn new(db: Connection) -> Self {
        Self { db }
    }

    pub async fn init(&self) -> Result<(), AppError> {
        _ = self
            .db
            .call(|conn| {
                conn.execute(
                    r#"create table if not exists users (
                         id uuid primary key not null,
                         username text not null unique
                       )"#,
                    [],
                )?;

                conn.execute(
                    r#"create table if not exists credentials (
                         name text not null,
                         user uuid not null,
                         value json not null,
                         foreign key(user) references users(id),
                         unique(name, user)
                       )"#,
                    [],
                )?;

                Ok(())
            })
            .await?;

        Ok(())
    }

    pub async fn get_user_with_credentials(
        &self,
        username: String,
    ) -> Result<UserWithCredentials, AppError> {
        let username_ = username.clone();

        let users = self
            .db
            .call(move |conn| {
                Ok(conn
                    .prepare(
                        r#"select u.id, u.username, c.name, c.value
                           from users u
                           left join credentials c on u.id = c.user
                           where username = ?1"#,
                    )?
                    .query_map((&username_,), |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    })?
                    .into_iter()
                    .filter_map(|v| if let Ok(v_ok) = v { Some(v_ok) } else { None })
                    .fold(Vec::new(), |mut accumulator, current| {
                        accumulator.push(current);
                        accumulator
                    }))
            })
            .await?;

        let user = users
            .into_iter()
            .fold(UserWithCredentials::default(), |mut user, u| {
                if let Ok(id) = Uuid::from_slice(&u.0.as_bytes()[..16]) {
                    user.id = id;
                }

                if u.2.is_some() && u.3.is_some() {
                    if let Ok(passkey) =
                        serde_json::from_str::<Passkey>(&u.3.expect("is_some guard"))
                    {
                        user.credentials.push(CredentialWithName {
                            name: u.2.expect("is_some guard"),
                            credential: passkey,
                        });
                    }
                }
                user.username = u.1;
                user
            });

        if user.exists() {
            return Ok::<_, AppError>(user);
        }

        let new_user = self
            .db
            .call(move |conn| {
                Ok(conn.query_row(
                    r#"insert into users (id, username)
                       values (?1, ?2)
                       returning id, username"#,
                    (&Uuid::new_v4().to_string(), &username),
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                ))
            })
            .await??;

        Ok::<_, AppError>(UserWithCredentials {
            id: Uuid::from_slice(&new_user.0.as_bytes()[..16])?,
            username: new_user.1,
            credentials: vec![],
        })
    }

    pub async fn add_credential(
        &self,
        username: String,
        credential_name: String,
        credential: &Passkey,
    ) -> Result<(), AppError> {
        let Ok(cred_val) = serde_json::to_string(&credential) else {
            return Err(AppError::UnknownError);
        };

        let n_added = self
            .db
            .call(|conn| {
                Ok(conn.execute(
                    r#"insert into credentials (name, user, value)
                       values (?1, (select id from users where username = ?2), json(?3))"#,
                    (credential_name, username, cred_val),
                ))
            })
            .await??;

        if n_added != 1 {
            Err(AppError::UserNotFound)
        } else {
            Ok::<_, AppError>(())
        }
    }

    pub async fn update_credential(
        &self,
        auth_result: AuthenticationResult,
    ) -> Result<(), AppError> {
        let cred_id = serde_json::to_string(auth_result.cred_id())?;
        let cred_id_ = cred_id.clone(); // TODO(jared): don't do this

        let cred_json = self
            .db
            .call(move |conn| {
                Ok(conn.query_row(
                    r#"select value from credentials
                       where value->'$.cred.cred_id' = ?1"#,
                    (cred_id_,),
                    |row| row.get::<_, String>(0),
                ))
            })
            .await??;

        let mut credential = serde_json::from_str::<Passkey>(&cred_json)?;
        if credential.update_credential(&auth_result).is_none() {
            return Err(AppError::MismatchingCredential);
        }

        let cred_json = serde_json::to_string(&credential)?;

        _ = self
            .db
            .call(move |conn| {
                Ok(conn.execute(
                    r#"update credentials set value = ?1
                       where value->'$.cred.cred_id' = ?2"#,
                    (cred_json, cred_id),
                ))
            })
            .await??;

        Ok(())
    }

    pub async fn delete_credential(&self, cred_id: CredentialID) -> Result<(), AppError> {
        let cred_id = serde_json::to_string(&cred_id)?;

        let n_deleted = self
            .db
            .call(move |conn| {
                Ok(conn.execute(
                    r#"delete from credentials where value->'$.cred.cred_id' = ?1"#,
                    (&cred_id,),
                ))
            })
            .await??;

        if n_deleted != 1 {
            Err(AppError::CredentialNotFound)
        } else {
            Ok::<_, AppError>(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use tokio_rusqlite::Connection;
    use webauthn_authenticator_rs::{prelude::Url, softtoken::SoftToken, WebauthnAuthenticator};
    use webauthn_rs_core::WebauthnCore;

    async fn get_app_with_db() -> App {
        let db = Connection::open(":memory:").await.unwrap();
        let app = App::new(db);
        app.init().await.unwrap();
        app
    }

    #[tokio::test]
    async fn test_init_is_idempotent() {
        let app = get_app_with_db().await;
        app.init().await.unwrap();
    }

    #[tokio::test]
    async fn test_get_user_with_credentials() {
        let app = get_app_with_db().await;

        assert_eq!(
            app.db
                .call(|conn| {
                    let exists: usize = conn
                        .query_row("select exists(select id from users)", [], |row| row.get(0))
                        .unwrap();

                    Ok(exists)
                })
                .await
                .unwrap(),
            0
        );

        app.get_user_with_credentials("foo_user".to_string())
            .await
            .unwrap(); // user is created if they do not exist

        assert_eq!(
            app.db
                .call(|conn| {
                    let exists: usize = conn
                        .query_row("select exists(select id from users)", [], |row| row.get(0))
                        .unwrap();

                    Ok(exists)
                })
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn test_credential_lifecycle() {
        let (soft_token, _) = SoftToken::new(true).unwrap();

        let wan = WebauthnCore::new_unsafe_experts_only(
            "https://localhost:8080/auth",
            "localhost",
            vec![Url::parse("https://localhost:8080").unwrap()],
            Duration::from_secs(1),
            None,
            None,
        );
        let mut wa = WebauthnAuthenticator::new(soft_token);

        let app = get_app_with_db().await;
        let user = app
            .get_user_with_credentials("bar_user".to_string())
            .await
            .unwrap();

        assert!(user.credentials.is_empty());

        let (chal, reg_state) = wan
            .generate_challenge_register(
                wan.new_challenge_register_builder(
                    &user.id.into_bytes(),
                    &user.username,
                    &user.username,
                )
                .unwrap(),
            )
            .unwrap();

        let r = wa
            .do_registration(Url::parse("https://localhost:8080").unwrap(), chal)
            .unwrap();

        let cred = wan.register_credential(&r, &reg_state, None).unwrap();

        app.add_credential(
            user.username,
            "bar_credential".to_string(),
            &Passkey::from(cred.clone()),
        )
        .await
        .unwrap();

        let user = app
            .get_user_with_credentials("bar_user".to_string())
            .await
            .unwrap();
        assert!(user.credentials.len() == 1);

        // TODO(jared): test this
        // app.update_credential();

        app.delete_credential(cred.cred_id).await.unwrap();

        let user = app
            .get_user_with_credentials("bar_user".to_string())
            .await
            .unwrap();
        assert!(user.credentials.is_empty());
    }
}
