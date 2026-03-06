use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::Row;
use tokio::sync::Mutex;

use crate::domain::ConversationRefRecord;
use crate::storage::db::Database;
use crate::storage::sessions_repo::StorageError;

#[async_trait]
pub trait ConversationRefsRepo: Send + Sync {
    async fn upsert(&self, record: &ConversationRefRecord) -> Result<(), StorageError>;
    async fn load(
        &self,
        tenant_id: &str,
        user_id: &str,
        channel: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRefRecord>, StorageError>;
}

#[derive(Clone, Default)]
pub struct InMemoryConversationRefsRepo {
    records: Arc<Mutex<HashMap<(String, String, String, String), ConversationRefRecord>>>,
}

impl InMemoryConversationRefsRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConversationRefsRepo for InMemoryConversationRefsRepo {
    async fn upsert(&self, record: &ConversationRefRecord) -> Result<(), StorageError> {
        self.records.lock().await.insert(
            (
                record.tenant_id.clone(),
                record.user_id.clone(),
                record.channel.clone(),
                record.conversation_id.clone(),
            ),
            record.clone(),
        );
        Ok(())
    }

    async fn load(
        &self,
        tenant_id: &str,
        user_id: &str,
        channel: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRefRecord>, StorageError> {
        Ok(self
            .records
            .lock()
            .await
            .get(&(
                tenant_id.to_string(),
                user_id.to_string(),
                channel.to_string(),
                conversation_id.to_string(),
            ))
            .cloned())
    }
}

#[derive(Clone)]
pub struct PostgresConversationRefsRepo {
    database: Database,
}

impl PostgresConversationRefsRepo {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl ConversationRefsRepo for PostgresConversationRefsRepo {
    async fn upsert(&self, record: &ConversationRefRecord) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            insert into conversation_refs (
                tenant_id,
                user_id,
                channel,
                conversation_id,
                ref_json
            )
            values ($1, $2, $3, $4, $5)
            on conflict (tenant_id, user_id, channel, conversation_id)
            do update set ref_json = excluded.ref_json, updated_at = now()
            "#,
        )
        .bind(&record.tenant_id)
        .bind(&record.user_id)
        .bind(&record.channel)
        .bind(&record.conversation_id)
        .bind(&record.ref_json)
        .execute(self.database.pool())
        .await?;

        Ok(())
    }

    async fn load(
        &self,
        tenant_id: &str,
        user_id: &str,
        channel: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationRefRecord>, StorageError> {
        let row = sqlx::query(
            r#"
            select tenant_id, user_id, channel, conversation_id, ref_json
            from conversation_refs
            where tenant_id = $1 and user_id = $2 and channel = $3 and conversation_id = $4
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(channel)
        .bind(conversation_id)
        .fetch_optional(self.database.pool())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(ConversationRefRecord {
            tenant_id: row.try_get("tenant_id")?,
            user_id: row.try_get("user_id")?,
            channel: row.try_get("channel")?,
            conversation_id: row.try_get("conversation_id")?,
            ref_json: row.try_get("ref_json")?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversationRefsRepo, InMemoryConversationRefsRepo};
    use crate::domain::ConversationRefRecord;
    use serde_json::json;

    #[tokio::test]
    async fn in_memory_conversation_refs_repo_round_trips_record() {
        let repo = InMemoryConversationRefsRepo::new();
        let record = ConversationRefRecord {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            channel: "teams".to_string(),
            conversation_id: "conv-1".to_string(),
            ref_json: json!({"serviceUrl": "https://service"}),
        };

        repo.upsert(&record).await.unwrap();
        let loaded = repo
            .load("tenant-1", "user-1", "teams", "conv-1")
            .await
            .unwrap();

        assert_eq!(loaded, Some(record));
    }
}
