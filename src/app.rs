use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tokio_rusqlite::Connection;
use webauthn_rs::prelude::{Passkey, PasskeyAuthentication, PasskeyRegistration, Uuid};
use webauthn_rs_proto::CredentialID;

#[derive(Deserialize, Serialize, Debug)]
pub struct CredentialState {
    pub id: Uuid,
    pub credentials: Vec<Passkey>,
}

#[derive(Debug, Clone)]
pub struct AppError;

impl From<AppError> for StatusCode {
    fn from(_error: AppError) -> Self {
        StatusCode::INTERNAL_SERVER_ERROR
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

pub struct UserWithCredentials {
    pub id: Uuid,
    pub username: String,
    pub credentials: Vec<Passkey>,
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
                         username text not null,
                       )"#,
                    [],
                )?;

                conn.execute(r#"create table if not exists credentials ()"#, [])?;

                Ok::<_, rusqlite::Error>(())
            })
            .await
    }

    pub async fn get_user_with_credentials(
        &self,
        _username: String,
    ) -> Result<UserWithCredentials, AppError> {
        todo!()
    }

    pub async fn add_credential(
        &self,
        _username: String,
        _credential: Passkey,
    ) -> Result<(), AppError> {
        todo!()
    }

    pub async fn increment_credential_counter(
        &self,
        _cred_id: &CredentialID,
        _counter: u32,
    ) -> Result<(), AppError> {
        todo!()
    }

    pub async fn delete_credential(&self, _cred_id: CredentialID) -> Result<(), AppError> {
        todo!()
    }
}
