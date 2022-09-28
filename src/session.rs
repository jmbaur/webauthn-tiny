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
                conn.execute(
                    r#"create table if not exists sessions (
                         id text primary key not null,
                         value json not null
                       )"#,
                    [],
                )
            })
            .await?;
        Ok(())
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn load_session(&self, cookie_value: String) -> Result<Option<Session>> {
        let id = Session::id_from_cookie_value(&cookie_value)?;
        self.db
            .call(|conn| {
                let value = conn.query_row(
                    r#"select value from sessions where id = ?1"#,
                    (id,),
                    |row| row.get::<_, String>(0),
                )?;

                let session: Session = serde_json::from_str(&value)?;
                Ok::<_, anyhow::Error>(session.validate())
            })
            .await
    }

    async fn store_session(&self, session: Session) -> Result<Option<String>> {
        let session_id = session.id().to_string();
        let session_str = serde_json::to_string(&session)?;

        self.db
            .call(|conn| {
                conn.execute(
                    r#"insert or replace into sessions (id, value) values(?1, ?2)"#,
                    (session_id, session_str),
                )
            })
            .await?;

        Ok(session.into_cookie_value())
    }

    async fn destroy_session(&self, session: Session) -> Result {
        self.db
            .call(move |conn| {
                conn.execute(
                    r#"delete from sessions where id = ?1"#,
                    (session.id().to_string(),),
                )
            })
            .await?;
        Ok(())
    }

    async fn clear_store(&self) -> Result {
        self.db
            .call(|conn| conn.execute(r#"delete from sessions"#, []))
            .await?;
        Ok(())
    }
}
