use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use sqlx::Row;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::domain::{ActivityEnvelope, ApprovalKind, ApprovalRecord};
use crate::policy::state_machine::ApprovalStatus;
use crate::storage::db::Database;
use crate::storage::sessions_repo::StorageError;

#[async_trait]
pub trait ApprovalsRepo: Send + Sync {
    async fn create(
        &self,
        activity: &ActivityEnvelope,
        kind: ApprovalKind,
        risk_level: &str,
        payload_json: serde_json::Value,
        policy_snapshot_json: serde_json::Value,
    ) -> Result<ApprovalRecord, StorageError>;
    async fn load(&self, approval_id: Uuid) -> Result<Option<ApprovalRecord>, StorageError>;
    async fn update_status(
        &self,
        approval_id: Uuid,
        status: ApprovalStatus,
    ) -> Result<Option<ApprovalRecord>, StorageError>;
}

#[derive(Clone, Default)]
pub struct InMemoryApprovalsRepo {
    records: Arc<Mutex<HashMap<Uuid, ApprovalRecord>>>,
}

impl InMemoryApprovalsRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ApprovalsRepo for InMemoryApprovalsRepo {
    async fn create(
        &self,
        activity: &ActivityEnvelope,
        kind: ApprovalKind,
        risk_level: &str,
        payload_json: serde_json::Value,
        policy_snapshot_json: serde_json::Value,
    ) -> Result<ApprovalRecord, StorageError> {
        let record = build_record(
            activity,
            kind,
            risk_level,
            payload_json,
            policy_snapshot_json,
        );
        self.records
            .lock()
            .await
            .insert(record.approval_id, record.clone());
        Ok(record)
    }

    async fn load(&self, approval_id: Uuid) -> Result<Option<ApprovalRecord>, StorageError> {
        Ok(self.records.lock().await.get(&approval_id).cloned())
    }

    async fn update_status(
        &self,
        approval_id: Uuid,
        status: ApprovalStatus,
    ) -> Result<Option<ApprovalRecord>, StorageError> {
        let mut guard = self.records.lock().await;
        let Some(record) = guard.get_mut(&approval_id) else {
            return Ok(None);
        };
        record.status = approval_status_string(status);
        Ok(Some(record.clone()))
    }
}

#[derive(Clone)]
pub struct PostgresApprovalsRepo {
    database: Database,
}

impl PostgresApprovalsRepo {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait]
impl ApprovalsRepo for PostgresApprovalsRepo {
    async fn create(
        &self,
        activity: &ActivityEnvelope,
        kind: ApprovalKind,
        risk_level: &str,
        payload_json: serde_json::Value,
        policy_snapshot_json: serde_json::Value,
    ) -> Result<ApprovalRecord, StorageError> {
        let record = build_record(
            activity,
            kind,
            risk_level,
            payload_json,
            policy_snapshot_json,
        );

        sqlx::query(
            r#"
            insert into approvals (
                approval_id,
                tenant_id,
                user_id,
                channel,
                conversation_id,
                kind,
                status,
                risk_level,
                payload_json,
                policy_snapshot_json,
                expires_at
            )
            values ($1, $2, $3, $4, $5, $6, $7::approval_status, $8, $9, $10, $11::timestamptz)
            "#,
        )
        .bind(record.approval_id)
        .bind(&record.tenant_id)
        .bind(&record.user_id)
        .bind(&record.channel)
        .bind(&record.conversation_id)
        .bind(&record.kind)
        .bind(&record.status)
        .bind(&record.risk_level)
        .bind(&record.payload_json)
        .bind(&record.policy_snapshot_json)
        .bind(&record.expires_at_utc)
        .execute(self.database.pool())
        .await?;

        Ok(record)
    }

    async fn load(&self, approval_id: Uuid) -> Result<Option<ApprovalRecord>, StorageError> {
        let row = sqlx::query(
            r#"
            select
                approval_id,
                tenant_id,
                user_id,
                channel,
                conversation_id,
                kind,
                status::text as status,
                risk_level,
                payload_json,
                policy_snapshot_json,
                expires_at::text as expires_at_utc
            from approvals
            where approval_id = $1
            "#,
        )
        .bind(approval_id)
        .fetch_optional(self.database.pool())
        .await?;

        row.map(map_row).transpose()
    }

    async fn update_status(
        &self,
        approval_id: Uuid,
        status: ApprovalStatus,
    ) -> Result<Option<ApprovalRecord>, StorageError> {
        let row = sqlx::query(
            r#"
            update approvals
            set status = $2::approval_status, updated_at = now()
            where approval_id = $1
            returning
                approval_id,
                tenant_id,
                user_id,
                channel,
                conversation_id,
                kind,
                status::text as status,
                risk_level,
                payload_json,
                policy_snapshot_json,
                expires_at::text as expires_at_utc
            "#,
        )
        .bind(approval_id)
        .bind(approval_status_string(status))
        .fetch_optional(self.database.pool())
        .await?;

        row.map(map_row).transpose()
    }
}

fn build_record(
    activity: &ActivityEnvelope,
    kind: ApprovalKind,
    risk_level: &str,
    payload_json: serde_json::Value,
    policy_snapshot_json: serde_json::Value,
) -> ApprovalRecord {
    ApprovalRecord {
        approval_id: Uuid::new_v4(),
        tenant_id: activity.actor.tenant_id.clone(),
        user_id: activity.actor.user_id.clone(),
        channel: activity.conversation.channel.clone(),
        conversation_id: activity.conversation.conversation_id.clone(),
        kind: kind.as_str().to_string(),
        status: approval_status_string(ApprovalStatus::Pending),
        risk_level: risk_level.to_string(),
        payload_json,
        policy_snapshot_json,
        expires_at_utc: (Utc::now() + Duration::minutes(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    }
}

fn approval_status_string(status: ApprovalStatus) -> String {
    match status {
        ApprovalStatus::Pending => "PENDING",
        ApprovalStatus::Approved => "APPROVED",
        ApprovalStatus::Rejected => "REJECTED",
        ApprovalStatus::Expired => "EXPIRED",
        ApprovalStatus::Cancelled => "CANCELLED",
        ApprovalStatus::Executed => "EXECUTED",
        ApprovalStatus::Failed => "FAILED",
    }
    .to_string()
}

fn map_row(row: sqlx::postgres::PgRow) -> Result<ApprovalRecord, StorageError> {
    Ok(ApprovalRecord {
        approval_id: row.try_get("approval_id")?,
        tenant_id: row.try_get("tenant_id")?,
        user_id: row.try_get("user_id")?,
        channel: row.try_get("channel")?,
        conversation_id: row.try_get("conversation_id")?,
        kind: row.try_get("kind")?,
        status: row.try_get("status")?,
        risk_level: row.try_get("risk_level")?,
        payload_json: row.try_get("payload_json")?,
        policy_snapshot_json: row.try_get("policy_snapshot_json")?,
        expires_at_utc: row.try_get("expires_at_utc")?,
    })
}

#[cfg(test)]
mod tests {
    use super::{ApprovalsRepo, InMemoryApprovalsRepo};
    use crate::domain::{ActivityEnvelope, Actor, AttachmentRef, ApprovalKind, Conversation};
    use serde_json::json;

    fn activity() -> ActivityEnvelope {
        ActivityEnvelope {
            actor: Actor {
                tenant_id: "tenant-1".to_string(),
                user_id: "user-1".to_string(),
            },
            conversation: Conversation {
                channel: "teams".to_string(),
                conversation_id: "conv-1".to_string(),
                message_id: "msg-1".to_string(),
            },
            text: String::new(),
            attachments: Vec::<AttachmentRef>::new(),
            action: None,
            action_payload_json: None,
            recipients: vec!["james@contoso.com".to_string()],
            attendee_email: None,
            attendee_known: true,
            contains_sensitive: false,
            request_hour_local: 10,
            conversation_ref_json: None,
        }
    }

    #[tokio::test]
    async fn in_memory_approvals_repo_creates_and_updates_records() {
        let repo = InMemoryApprovalsRepo::new();
        let record = repo
            .create(
                &activity(),
                ApprovalKind::SendMail,
                "LOW",
                json!({"draft": "ok"}),
                json!({"policy": "ALLOW"}),
            )
            .await
            .unwrap();

        assert_eq!(record.kind, "SEND_MAIL");
        assert_eq!(record.status, "PENDING");

        let updated = repo
            .update_status(record.approval_id, crate::policy::state_machine::ApprovalStatus::Approved)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, "APPROVED");
    }
}
