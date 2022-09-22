use std::{
    collections::HashMap,
    fs,
    sync::{Arc, RwLock},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio_rusqlite::Connection;
use webauthn_rs::prelude::{Passkey, PasskeyAuthentication, PasskeyRegistration, Uuid};

pub type Users = HashMap<String, String>; // username:password
pub type Credentials = HashMap<String, CredentialState>;

#[derive(Deserialize, Serialize, Debug)]
pub struct CredentialState {
    pub id: Uuid,
    pub credentials: Vec<Passkey>,
}

pub fn load<T>(path: &str) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    let contents = fs::read_to_string(path)?;
    let data: T = serde_yaml::from_str(&contents)?;
    anyhow::Ok(data)
}

pub fn persist<T>(path: &str, data: &T) -> anyhow::Result<()>
where
    T: Serialize,
{
    let contents = serde_yaml::to_string(data)?;
    fs::write(path, contents)?;
    Ok(())
}

#[derive(Debug)]
pub struct App {
    pub id: String,
    pub in_progress_authentications: HashMap<String, PasskeyAuthentication>,
    pub in_progress_registrations: HashMap<String, PasskeyRegistration>,
    pub origin: String,
    db: Connection,
}

pub type SharedAppState = Arc<RwLock<App>>;

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

    pub async fn get_credentials() -> Result<(), rusqlite::Error> {
        Ok(())
    }

    pub async fn add_credential() -> Result<(), rusqlite::Error> {
        Ok(())
    }

    pub async fn update_credential() -> Result<(), rusqlite::Error> {
        Ok(())
    }

    pub async fn delete_credential() -> Result<(), rusqlite::Error> {
        Ok(())
    }
}
