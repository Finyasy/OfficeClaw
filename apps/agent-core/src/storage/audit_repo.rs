use std::sync::Arc;

use async_trait::async_trait;
use sqlx::Row;
use tokio::sync::Mutex;

use crate::domain::AuditEventRecord;
use crate::storage::db::Database;
use crate::storage::sessions_repo::StorageError;

#[async_trait]
pub trait AuditRepo: Send + Sync {
    async fn append(&self, event: AuditEventRecord) -> Result<(), StorageError>;
    async fn list(&self) -> Result<Vec<AuditEventRecord>, StorageError>;
}

#[derive(Clone, Default)]
pub struct InMemoryAuditRepo {
    events: Arc<Mutex<Vec<AuditEventRecord>>>,
}

impl InMemoryAuditRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AuditRepo for InMemoryAuditRepo {
    async fn append(&self, event: AuditEventRecord) -> Result<(), StorageError> {
        self.events.lock().await.push(event);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<AuditEventRecord>, StorageError> {
        Ok(self.events.lock().await.clone())
    }
}

#[derive(Clone)]
pub struct PostgresAuditRepo {
    database: Database,
}

impl PostgresAuditRepo {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl AuditRepo for PostgresAuditRepo {
    async fn append(&self, event: AuditEventRecord) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            insert into audit_events (
                event_id,
                tenant_id,
                user_id,
                channel,
                conversation_id,
                correlation_id,
                event_type,
                event_json
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(event.event_id)
        .bind(event.tenant_id)
        .bind(event.user_id)
        .bind(event.channel)
        .bind(event.conversation_id)
        .bind(event.correlation_id)
        .bind(event.event_type)
        .bind(event.event_json)
        .execute(self.database.pool())
        .await
        .map_err(StorageError::from)?;

        Ok(())
    }

    async fn list(&self) -> Result<Vec<AuditEventRecord>, StorageError> {
        let rows = sqlx::query(
            r#"
            select
                event_id,
                tenant_id,
                user_id,
                channel,
                conversation_id,
                correlation_id,
                event_type,
                event_json
            from audit_events
            order by created_at asc
            "#,
        )
        .fetch_all(self.database.pool())
        .await
        .map_err(StorageError::from)?;

        rows.into_iter()
            .map(|row| {
                Ok(AuditEventRecord {
                    event_id: row.try_get("event_id")?,
                    tenant_id: row.try_get("tenant_id")?,
                    user_id: row.try_get("user_id")?,
                    channel: row.try_get("channel")?,
                    conversation_id: row.try_get("conversation_id")?,
                    correlation_id: row.try_get("correlation_id")?,
                    event_type: row.try_get("event_type")?,
                    event_json: row.try_get("event_json")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{AuditRepo, InMemoryAuditRepo};
    use crate::domain::AuditEventRecord;
    use serde_json::json;
    use uuid::Uuid;

    #[tokio::test]
    async fn in_memory_audit_repo_preserves_append_order() {
        let repo = InMemoryAuditRepo::new();
        let first = AuditEventRecord {
            event_id: Uuid::new_v4(),
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            channel: "teams".to_string(),
            conversation_id: "conv-1".to_string(),
            correlation_id: "corr-1".to_string(),
            event_type: "FIRST".to_string(),
            event_json: json!({"index": 1}),
        };
        let second = AuditEventRecord {
            event_id: Uuid::new_v4(),
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            channel: "teams".to_string(),
            conversation_id: "conv-1".to_string(),
            correlation_id: "corr-2".to_string(),
            event_type: "SECOND".to_string(),
            event_json: json!({"index": 2}),
        };

        repo.append(first.clone()).await.unwrap();
        repo.append(second.clone()).await.unwrap();
        let stored = repo.list().await.unwrap();

        assert_eq!(stored, vec![first, second]);
    }
}
