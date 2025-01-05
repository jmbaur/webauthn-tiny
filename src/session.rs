use async_trait::async_trait;
use rusqlite::OptionalExtension;
use tokio_rusqlite::Connection;
use tower_sessions::{
    session::{Id, Record},
    session_store::{Error, Result, SessionStore},
};

#[derive(Clone, Debug)]
pub struct SqliteSessionStore {
    db: Connection,
}

impl SqliteSessionStore {
    pub fn new(db: Connection) -> Self {
        Self { db }
    }

    pub async fn init(&self) -> anyhow::Result<()> {
        self.db
            .call(|conn| {
                Ok(conn.execute(
                    r#"create table if not exists sessions (
                         id text primary key not null,
                         value json not null
                       )"#,
                    [],
                ))
            })
            .await??;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn clear(&self) -> anyhow::Result<()> {
        self.db
            .call(|conn| Ok(conn.execute(r#"delete from sessions"#, [])))
            .await??;

        Ok(())
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    /// Saves the provided session record to the store.
    ///
    /// This method is intended for updating the state of an existing session.
    async fn save(&self, session_record: &Record) -> Result<()> {
        let session_id = session_record.id.to_string();
        let session_value =
            serde_json::to_string(session_record).map_err(|err| Error::Backend(err.to_string()))?;

        _ = self
            .db
            .call(|conn| {
                Ok(conn.execute(
                    r#"insert or replace into sessions (id, value) values(?1, ?2)"#,
                    (session_id, session_value),
                ))
            })
            .await
            .map_err(|err| Error::Backend(err.to_string()))?
            .map_err(|err| Error::Backend(err.to_string()))?;

        Ok(())
    }

    /// Loads an existing session record from the store using the provided ID.
    ///
    /// If a session with the given ID exists, it is returned. If the session
    /// does not exist or has been invalidated (e.g., expired), `None` is
    /// returned.
    async fn load(&self, session_id: &Id) -> Result<Option<Record>> {
        let session_id = session_id.to_string();

        let Some(value) = self
            .db
            .call(|conn| {
                Ok(conn
                    .query_row(
                        r#"select value from sessions where id = ?1"#,
                        (session_id,),
                        |row| row.get::<_, String>(0),
                    )
                    .optional())
            })
            .await
            .map_err(|err| Error::Backend(err.to_string()))?
            .map_err(|err| Error::Backend(err.to_string()))?
        else {
            return Ok(None);
        };

        let session: Record =
            serde_json::from_str(&value).map_err(|err| Error::Backend(err.to_string()))?;

        Ok(Some(session))
    }

    /// Deletes a session record from the store using the provided ID.
    ///
    /// If the session exists, it is removed from the store.
    async fn delete(&self, session_id: &Id) -> Result<()> {
        let session_id = session_id.to_string();
        let _n_deleted = self
            .db
            .call(move |conn| {
                Ok(conn.execute(r#"delete from sessions where id = ?1"#, (session_id,)))
            })
            .await
            .map_err(|err| Error::Backend(err.to_string()))?
            .map_err(|err| Error::Backend(err.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tower_sessions::cookie::time::OffsetDateTime;

    #[tokio::test]
    async fn test_session_lifecycle() {
        let db = Connection::open(":memory:").await.unwrap();
        let store = SqliteSessionStore::new(db);
        store.init().await.unwrap();

        {
            let mut session = Record {
                id: Id::default(),
                data: HashMap::default(),
                expiry_date: OffsetDateTime::now_utc(),
            };

            store.create(&mut session).await.unwrap();
            let loaded = store.load(&session.id).await.unwrap().unwrap();
            store.delete(&loaded.id).await.unwrap();
            assert_eq!(
                store
                    .db
                    .call(|conn| {
                        let exists: usize = conn
                            .query_row("select exists(select id from sessions)", [], |row| {
                                row.get(0)
                            })
                            .unwrap();
                        Ok(exists)
                    })
                    .await
                    .unwrap(),
                0
            );
        }

        {
            let mut session = Record {
                id: Id::default(),
                data: HashMap::default(),
                expiry_date: OffsetDateTime::now_utc(),
            };
            store.create(&mut session).await.unwrap();
            store.clear().await.unwrap();
            assert_eq!(
                store
                    .db
                    .call(|conn| {
                        let exists: usize = conn
                            .query_row("select exists(select id from sessions)", [], |row| {
                                row.get(0)
                            })
                            .unwrap();
                        Ok(exists)
                    })
                    .await
                    .unwrap(),
                0
            );
        }
    }
}
