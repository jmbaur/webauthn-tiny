use async_trait::async_trait;
use axum_sessions::async_session::{Result, Session, SessionStore};
use tokio_rusqlite::Connection;

#[derive(Clone, Debug)]
pub struct SqliteSessionStore {
    db: Connection,
}

impl SqliteSessionStore {
    pub fn new(db: Connection) -> Self {
        Self { db }
    }

    pub async fn init(&self) -> Result {
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
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn load_session(&self, cookie_value: String) -> Result<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;
        let value = self
            .db
            .call(|conn| {
                Ok(conn.query_row(
                    r#"select value from sessions where id = ?1"#,
                    (id,),
                    |row| row.get::<_, String>(0),
                )?)
            })
            .await?;

        let session: Session = serde_json::from_str(&value)?;
        Ok(session.validate())
    }

    async fn store_session(&self, session: Session) -> Result<Option<String>> {
        let session_id = session.id().to_string();
        let session_str = serde_json::to_string(&session)?;

        self.db
            .call(|conn| {
                Ok(conn.execute(
                    r#"insert or replace into sessions (id, value) values(?1, ?2)"#,
                    (session_id, session_str),
                ))
            })
            .await??;

        Ok(session.into_cookie_value())
    }

    async fn destroy_session(&self, session: Session) -> Result {
        self.db
            .call(move |conn| {
                Ok(conn.execute(
                    r#"delete from sessions where id = ?1"#,
                    (session.id().to_string(),),
                ))
            })
            .await??;
        Ok(())
    }

    async fn clear_store(&self) -> Result {
        self.db
            .call(|conn| Ok(conn.execute(r#"delete from sessions"#, [])))
            .await??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_lifecycle() {
        let db = Connection::open(":memory:").await.unwrap();
        let store = SqliteSessionStore::new(db);
        store.init().await.unwrap();

        let session = Session::new();
        let stored = store.store_session(session).await.unwrap().unwrap();
        let loaded = store.load_session(stored).await.unwrap().unwrap();
        store.destroy_session(loaded).await.unwrap();
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

        let session = Session::new();
        store.store_session(session).await.unwrap().unwrap();
        store.clear_store().await.unwrap();
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
