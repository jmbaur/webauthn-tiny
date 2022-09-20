use std::{
    collections::HashMap,
    fs,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use webauthn_rs::prelude::{Passkey, PasskeyAuthentication, PasskeyRegistration, Uuid};

#[derive(Deserialize, Serialize, Debug)]
pub struct UserState {
    pub id: Uuid,
    pub hash: String,
    pub credentials: Vec<Passkey>,
}

fn load_users(path: &str) -> anyhow::Result<HashMap<String, UserState>> {
    let user_file_contents = fs::read_to_string(path)?;
    let contents = serde_yaml::from_str::<HashMap<String, UserState>>(&user_file_contents)?;
    anyhow::Ok(contents)
}

fn persist_users(path: &str, users: &HashMap<String, UserState>) -> anyhow::Result<()> {
    let updated_user_file_contents = serde_yaml::to_string(users)?;
    fs::write(path, updated_user_file_contents)?;
    Ok(())
}

#[derive(Debug)]
pub struct AppState {
    user_file: String,

    pub id: String,
    pub origin: String,
    pub in_progress_registrations: HashMap<String, PasskeyRegistration>,
    pub in_progress_authentications: HashMap<String, PasskeyAuthentication>,
    pub users: HashMap<String, UserState>,
}

pub type SharedAppState = Arc<RwLock<AppState>>;

impl AppState {
    pub fn new(id: String, origin: String, user_file: String) -> anyhow::Result<Self> {
        Ok(Self {
            id,
            origin,
            user_file: user_file.clone(),
            users: load_users(user_file.as_str())?,
            in_progress_registrations: HashMap::new(),
            in_progress_authentications: HashMap::new(),
        })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        persist_users(&self.user_file, &self.users)
    }
}
