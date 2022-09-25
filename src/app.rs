use axum::http::StatusCode;
use libsqlite3_sys::ErrorCode::ConstraintViolation;
use rusqlite::Error::{QueryReturnedNoRows, SqliteFailure};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, sync::Arc};
use tokio::sync::RwLock;
use tokio_rusqlite::Connection;
use webauthn_rs::prelude::{
    AuthenticationResult, Passkey, PasskeyAuthentication, PasskeyRegistration, Uuid,
};

#[derive(Deserialize, Serialize, Debug)]
pub struct CredentialState {
    pub id: Uuid,
    pub credentials: Vec<Passkey>,
}

#[derive(Debug)]
pub enum AppError {
    UserNotFound,
    CredentialNotFound,
    MismatchingCredentialID,
    SqlError(rusqlite::Error),
    SerdeError(serde_json::Error),
    UuidError(uuid::Error),
    UnknownError,
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SqlError(e) => write!(f, "{e}"),
            Self::SerdeError(e) => write!(f, "{e}"),
            Self::UuidError(e) => write!(f, "{e}"),
            _ => write!(f, "AppError"), // TODO(jared): better display impl
        }
    }
}

impl std::error::Error for AppError {}

impl Default for AppError {
    fn default() -> Self {
        AppError::UnknownError
    }
}

impl From<AppError> for StatusCode {
    fn from(error: AppError) -> Self {
        eprintln!("{:#?}", error);
        match error {
            AppError::SqlError(QueryReturnedNoRows) => StatusCode::NOT_FOUND,
            AppError::SqlError(SqliteFailure(err, _)) => match err.code {
                ConstraintViolation => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            AppError::UserNotFound => StatusCode::NOT_FOUND,
            AppError::CredentialNotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        AppError::SqlError(error)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        AppError::SerdeError(error)
    }
}

impl From<uuid::Error> for AppError {
    fn from(error: uuid::Error) -> Self {
        AppError::UuidError(error)
    }
}

pub struct App {
    pub id: String,
    pub in_progress_authentications: HashMap<String, PasskeyAuthentication>,
    pub in_progress_registrations: HashMap<String, PasskeyRegistration>,
    pub origin: String,
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
    pub fn new(db: Connection, id: String, origin: String) -> Self {
        Self {
            db,
            id,
            in_progress_authentications: HashMap::new(),
            in_progress_registrations: HashMap::new(),
            origin,
        }
    }

    pub async fn init(&self) -> Result<(), AppError> {
        self.db
            .call(|conn| {
                conn.execute(
                    r#"create table if not exists users (
                         id uuid primary key,
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

                Ok::<_, AppError>(())
            })
            .await
    }

    pub async fn get_user_with_credentials(
        &self,
        username: String,
    ) -> Result<UserWithCredentials, AppError> {
        self.db
            .call(|conn| {
                let user = conn
                    .prepare(
                        r#"select u.id, u.username, c.name, c.value
                           from users u
                           left join credentials c on u.id = c.user
                           where username = ?1"#,
                    )?
                    .query_map((username.clone(),), |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    })?
                    .into_iter()
                    .filter_map(|v| if let Ok(v_ok) = v { Some(v_ok) } else { None })
                    .fold(UserWithCredentials::default(), |mut acc, curr| {
                        if let Ok(id) = Uuid::from_slice(&curr.0.as_bytes()[..16]) {
                            acc.id = id;
                        }
                        if curr.2.is_some() && curr.3.is_some() {
                            if let Ok(passkey) =
                                serde_json::from_str::<Passkey>(&curr.3.expect("is_some guard"))
                            {
                                acc.credentials.push(CredentialWithName {
                                    name: curr.2.expect("is_some guard"),
                                    credential: passkey,
                                });
                            }
                        }
                        acc.username = curr.1;
                        acc
                    });

                if user.exists() {
                    return Ok::<_, AppError>(user);
                }

                let new_user = conn.query_row(
                    r#"insert into users (id, username)
                       values (?1, ?2)
                       returning id, username"#,
                    (Uuid::new_v4().to_string(), username),
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?;

                Ok::<_, AppError>(UserWithCredentials {
                    id: Uuid::from_slice(&new_user.0.as_bytes()[..16])?,
                    username: new_user.1,
                    credentials: vec![],
                })
            })
            .await
    }

    pub async fn add_credential(
        &self,
        username: String,
        credential_name: String,
        credential: Passkey,
    ) -> Result<(), AppError> {
        let value = match serde_json::to_string(&credential) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("add_credential: {e}");
                return Err(AppError::UnknownError);
            }
        };
        self.db
            .call(|conn| {
                conn.execute(
                    r#"insert into credentials (name, user, value)
                       values (?1, (select id from users where username = ?2), json(?3))"#,
                    (credential_name, username, value),
                )?;
                Ok::<_, AppError>(())
            })
            .await
    }

    pub async fn update_credential(
        &self,
        auth_result: AuthenticationResult,
    ) -> Result<(), AppError> {
        self.db
            .call(move |conn| {
                let cred_id = auth_result.cred_id().to_string();
                let cred_json: String = conn.query_row(
                    r#"select value from credentials
                       where value->>'$.cred.cred_id' = ?1"#,
                    (auth_result.cred_id().to_string(),),
                    |row| row.get(0),
                )?;

                let mut credential = serde_json::from_str::<Passkey>(&cred_json)?;
                if credential.update_credential(&auth_result).is_none() {
                    return Err(AppError::MismatchingCredentialID);
                }

                let cred_json = serde_json::to_string(&credential)?;
                conn.execute(
                    r#"update credentials set value = ?1
                       where value->>'$.cred.cred_id' = ?2"#,
                    (cred_json, cred_id),
                )?;

                Ok::<_, AppError>(())
            })
            .await
    }

    pub async fn delete_credential(&self, cred_name: String) -> Result<(), AppError> {
        self.db
            .call(move |conn| {
                let count =
                    conn.execute(r#"delete from credentials where name = ?1"#, (cred_name,))?;
                if count != 1 {
                    Err(AppError::CredentialNotFound)
                } else {
                    Ok::<_, AppError>(())
                }
            })
            .await
    }
}
