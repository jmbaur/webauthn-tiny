use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_rusqlite::Connection;
use webauthn_rs::prelude::{
    AuthenticationResult, Passkey, PasskeyAuthentication, PasskeyRegistration, Uuid,
};
use webauthn_rs_proto::CredentialID;

#[derive(Deserialize, Serialize, Debug)]
pub struct CredentialState {
    pub id: Uuid,
    pub credentials: Vec<Passkey>,
}

#[derive(Debug)]
pub struct AppError;

impl From<AppError> for StatusCode {
    fn from(_error: AppError) -> Self {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(_error: rusqlite::Error) -> Self {
        todo!()
    }
}

impl From<serde_json::Error> for AppError {
    fn from(_error: serde_json::Error) -> Self {
        todo!()
    }
}

impl From<uuid::Error> for AppError {
    fn from(_error: uuid::Error) -> Self {
        todo!()
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

pub struct CredentialWithName {
    pub name: String,
    pub credential: Passkey,
}

#[derive(Default)]
pub struct UserWithCredentials {
    pub id: Uuid,
    pub username: String,
    pub credentials: Vec<CredentialWithName>,
}

impl App {
    pub fn new(db: Connection, id: String, origin: String) -> anyhow::Result<Self> {
        Ok(Self {
            db,
            id,
            in_progress_authentications: HashMap::new(),
            in_progress_registrations: HashMap::new(),
            origin,
        })
    }

    pub async fn init(&self) -> Result<(), rusqlite::Error> {
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

                Ok::<_, rusqlite::Error>(())
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
                           join credentials c on u.id = c.user
                           where username = ?1"#,
                    )?
                    .query_map((username,), |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })?
                    .into_iter()
                    .filter_map(|v| if let Ok(v_ok) = v { Some(v_ok) } else { None })
                    .fold(UserWithCredentials::default(), |mut acc, curr| {
                        if let Ok(id) = Uuid::from_slice(curr.0.as_bytes()) {
                            acc.id = id;
                        }
                        if let Ok(passkey) = serde_json::from_str::<Passkey>(&curr.3) {
                            acc.credentials.push(CredentialWithName {
                                name: curr.2,
                                credential: passkey,
                            });
                        }
                        acc.username = curr.1;
                        acc
                    });

                Ok::<_, AppError>(user)
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
                eprintln!("add_credential: {e}");
                return Err(AppError);
            }
        };
        if let Err(e) = self
            .db
            .call(|conn| {
                conn.execute(
                    r#"insert into credentials (name, user, value)
                       values (?1, (select id from users where username = ?2), ?3)"#,
                    (credential_name, username, value),
                )?;
                Ok::<_, rusqlite::Error>(())
            })
            .await
        {
            eprintln!("add_credential: {e}");
            return Err(AppError);
        }

        Ok(())
    }

    pub async fn update_credential(
        &self,
        auth_result: &AuthenticationResult,
    ) -> Result<(), AppError> {
        todo!()
    }

    pub async fn delete_credential(&self, _cred_id: CredentialID) -> Result<(), AppError> {
        todo!()
    }
}
