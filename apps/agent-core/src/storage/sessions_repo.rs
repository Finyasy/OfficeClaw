use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::Row;
use tokio::sync::Mutex;

use crate::crypto::envelope::CryptoError;
use crate::domain::{SessionKey, SessionState};
use crate::storage::db::Database;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageError {
    pub message: String,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for StorageError {}

impl From<sqlx::Error> for StorageError {
    fn from(value: sqlx::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(value: serde_json::Error) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

impl From<CryptoError> for StorageError {
    fn from(value: CryptoError) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

#[async_trait]
pub trait SessionsRepo: Send + Sync {
    async fn load(&self, key: &SessionKey) -> Result<Option<SessionState>, StorageError>;
    async fn upsert(&self, key: &SessionKey, state: &SessionState) -> Result<(), StorageError>;
}

#[derive(Clone, Default)]
pub struct InMemorySessionsRepo {
    records: Arc<Mutex<HashMap<SessionKey, SessionState>>>,
}

impl InMemorySessionsRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionsRepo for InMemorySessionsRepo {
    async fn load(&self, key: &SessionKey) -> Result<Option<SessionState>, StorageError> {
        Ok(self.records.lock().await.get(key).cloned())
    }

    async fn upsert(&self, key: &SessionKey, state: &SessionState) -> Result<(), StorageError> {
        self.records.lock().await.insert(key.clone(), state.clone());
        Ok(())
    }
}

#[derive(Clone)]
pub struct PostgresSessionsRepo {
    database: Database,
}

impl PostgresSessionsRepo {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl SessionsRepo for PostgresSessionsRepo {
    async fn load(&self, key: &SessionKey) -> Result<Option<SessionState>, StorageError> {
        let row = sqlx::query(
            r#"
            select state_json
            from sessions
            where tenant_id = $1 and user_id = $2 and channel = $3 and conversation_id = $4
            "#,
        )
        .bind(&key.tenant_id)
        .bind(&key.user_id)
        .bind(&key.channel)
        .bind(&key.conversation_id)
        .fetch_optional(self.database.pool())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let state_json: serde_json::Value = row.try_get("state_json")?;
        let state = serde_json::from_value(state_json)?;
        Ok(Some(state))
    }

    async fn upsert(&self, key: &SessionKey, state: &SessionState) -> Result<(), StorageError> {
        let state_json = serde_json::to_value(state)?;

        sqlx::query(
            r#"
            insert into sessions (tenant_id, user_id, channel, conversation_id, state_json)
            values ($1, $2, $3, $4, $5)
            on conflict (tenant_id, user_id, channel, conversation_id)
            do update set state_json = excluded.state_json, updated_at = now()
            "#,
        )
        .bind(&key.tenant_id)
        .bind(&key.user_id)
        .bind(&key.channel)
        .bind(&key.conversation_id)
        .bind(state_json)
        .execute(self.database.pool())
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemorySessionsRepo, SessionsRepo};
    use crate::domain::{SessionKey, SessionState};

    fn key() -> SessionKey {
        SessionKey {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            channel: "teams".to_string(),
            conversation_id: "conv-1".to_string(),
        }
    }

    #[tokio::test]
    async fn in_memory_sessions_repo_round_trips_state() {
        let repo = InMemorySessionsRepo::new();
        let state = SessionState {
            last_intent: Some("summarize_unread".to_string()),
            unread_summary_count: Some(3),
            proposed_slots: vec![],
        };

        repo.upsert(&key(), &state).await.unwrap();
        let loaded = repo.load(&key()).await.unwrap();

        assert_eq!(loaded, Some(state));
    }

    #[tokio::test]
    async fn in_memory_sessions_repo_returns_none_for_missing_state() {
        let repo = InMemorySessionsRepo::new();
        let loaded = repo.load(&key()).await.unwrap();
        assert_eq!(loaded, None);
    }
}
