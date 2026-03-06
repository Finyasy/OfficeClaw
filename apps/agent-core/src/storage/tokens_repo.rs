use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::Row;
use tokio::sync::Mutex;

use crate::crypto::envelope::EnvelopeCipher;
use crate::domain::{Actor, OAuthTokenBundle};
use crate::storage::db::Database;
use crate::storage::sessions_repo::StorageError;

#[async_trait]
pub trait TokensRepo: Send + Sync {
    async fn store_graph_token(
        &self,
        actor: &Actor,
        token: &OAuthTokenBundle,
    ) -> Result<(), StorageError>;
    async fn load_graph_token(
        &self,
        actor: &Actor,
    ) -> Result<Option<OAuthTokenBundle>, StorageError>;
}

#[derive(Clone, Default)]
pub struct InMemoryTokensRepo {
    records: Arc<Mutex<HashMap<(String, String), OAuthTokenBundle>>>,
}

impl InMemoryTokensRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl TokensRepo for InMemoryTokensRepo {
    async fn store_graph_token(
        &self,
        actor: &Actor,
        token: &OAuthTokenBundle,
    ) -> Result<(), StorageError> {
        self.records.lock().await.insert(
            (actor.tenant_id.clone(), actor.user_id.clone()),
            token.clone(),
        );
        Ok(())
    }

    async fn load_graph_token(
        &self,
        actor: &Actor,
    ) -> Result<Option<OAuthTokenBundle>, StorageError> {
        Ok(self
            .records
            .lock()
            .await
            .get(&(actor.tenant_id.clone(), actor.user_id.clone()))
            .cloned())
    }
}

#[derive(Clone)]
pub struct PostgresTokensRepo {
    database: Database,
    cipher: Arc<dyn EnvelopeCipher>,
}

impl PostgresTokensRepo {
    pub fn new(database: Database, cipher: Arc<dyn EnvelopeCipher>) -> Self {
        Self { database, cipher }
    }
}

#[async_trait]
impl TokensRepo for PostgresTokensRepo {
    async fn store_graph_token(
        &self,
        actor: &Actor,
        token: &OAuthTokenBundle,
    ) -> Result<(), StorageError> {
        let serialized = serde_json::to_vec(token)?;
        let sealed = self.cipher.seal(&serialized).await?;

        sqlx::query(
            r#"
            insert into oauth_tokens (
                tenant_id,
                user_id,
                provider,
                encrypted_blob,
                key_version
            )
            values ($1, $2, 'graph', $3, $4)
            on conflict (tenant_id, user_id, provider)
            do update set
                encrypted_blob = excluded.encrypted_blob,
                key_version = excluded.key_version,
                updated_at = now()
            "#,
        )
        .bind(&actor.tenant_id)
        .bind(&actor.user_id)
        .bind(sealed.ciphertext)
        .bind(sealed.key_version)
        .execute(self.database.pool())
        .await?;

        Ok(())
    }

    async fn load_graph_token(
        &self,
        actor: &Actor,
    ) -> Result<Option<OAuthTokenBundle>, StorageError> {
        let row = sqlx::query(
            r#"
            select encrypted_blob, key_version
            from oauth_tokens
            where tenant_id = $1 and user_id = $2 and provider = 'graph'
            "#,
        )
        .bind(&actor.tenant_id)
        .bind(&actor.user_id)
        .fetch_optional(self.database.pool())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let ciphertext: Vec<u8> = row.try_get("encrypted_blob")?;
        let key_version: String = row.try_get("key_version")?;
        let plaintext = self.cipher.open(&key_version, &ciphertext).await?;
        let token = serde_json::from_slice(&plaintext)?;

        Ok(Some(token))
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryTokensRepo, TokensRepo};
    use crate::domain::{Actor, OAuthTokenBundle};

    fn actor() -> Actor {
        Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
        }
    }

    #[tokio::test]
    async fn in_memory_tokens_repo_round_trips_graph_token() {
        let repo = InMemoryTokensRepo::new();
        let token = OAuthTokenBundle {
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at_utc: Some("2026-03-06T12:00:00Z".to_string()),
            scope: Some("Mail.Read Calendars.Read".to_string()),
        };

        repo.store_graph_token(&actor(), &token).await.unwrap();
        let loaded = repo.load_graph_token(&actor()).await.unwrap();

        assert_eq!(loaded, Some(token));
    }
}
