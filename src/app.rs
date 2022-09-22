use std::{
    collections::HashMap,
    fs,
    sync::{Arc, RwLock},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
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
pub struct AppState {
    pub credential_file: String,
    pub credentials: Credentials,
    pub id: String,
    pub in_progress_authentications: HashMap<String, PasskeyAuthentication>,
    pub in_progress_registrations: HashMap<String, PasskeyRegistration>,
    pub origin: String,
    pub user_file: String,
    pub users: Users,
}

pub type SharedAppState = Arc<RwLock<AppState>>;

impl AppState {
    pub fn new(
        id: String,
        origin: String,
        user_file: String,
        credential_file: String,
    ) -> anyhow::Result<Self> {
        let users = load::<Users>(user_file.as_str())?;
        let mut credentials = load::<Credentials>(credential_file.as_str())?;

        let mut touched = false;
        for username in users.keys() {
            if credentials.get(username).is_none() {
                credentials.insert(
                    username.to_string(),
                    CredentialState {
                        id: Uuid::new_v4(),
                        credentials: vec![],
                    },
                );
                touched = true;
            }
        }
        if touched {
            persist(&credential_file, &credentials)?;
        }

        Ok(Self {
            credential_file,
            credentials,
            id,
            in_progress_authentications: HashMap::new(),
            in_progress_registrations: HashMap::new(),
            origin,
            user_file,
            users,
        })
    }
}
