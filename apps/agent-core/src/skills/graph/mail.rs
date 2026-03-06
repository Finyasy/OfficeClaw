use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Datelike, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::{Actor, MailApprovalPayload};
use crate::skills::graph::client::{GraphClient, GraphClientError};
use crate::storage::tokens_repo::TokensRepo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadMessage {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub received_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailReadError {
    Retryable(String),
    Permanent(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailSendError {
    Retryable(String),
    Permanent(String),
}

#[async_trait]
pub trait MailReader: Send + Sync {
    async fn list_unread_today(&self, actor: &Actor) -> Result<Vec<UnreadMessage>, MailReadError>;
}

#[async_trait]
pub trait MailSender: Send + Sync {
    async fn send_draft(
        &self,
        actor: &Actor,
        draft: &MailApprovalPayload,
    ) -> Result<(), MailSendError>;
}

#[derive(Clone)]
pub struct GraphMailReader {
    client: GraphClient,
    tokens_repo: Arc<dyn TokensRepo>,
}

impl GraphMailReader {
    pub fn new(client: GraphClient, tokens_repo: Arc<dyn TokensRepo>) -> Self {
        Self {
            client,
            tokens_repo,
        }
    }
}

#[async_trait]
impl MailReader for GraphMailReader {
    async fn list_unread_today(&self, actor: &Actor) -> Result<Vec<UnreadMessage>, MailReadError> {
        let token = self
            .tokens_repo
            .load_graph_token(actor)
            .await
            .map_err(|error| MailReadError::Permanent(error.message))?
            .ok_or_else(|| MailReadError::Permanent("Graph token unavailable".to_string()))?;
        let now = Utc::now();
        let start_of_day = Utc
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .unwrap_or_else(|| now - Duration::hours(24));
        let filter = unread_today_path(start_of_day);

        let response: GraphListMessagesResponse = self
            .client
            .get_json(&filter, &token.access_token)
            .await
            .map_err(map_graph_error)?;

        Ok(response
            .value
            .into_iter()
            .map(|message| UnreadMessage {
                id: message.id,
                subject: normalize_subject(message.subject),
                from: message
                    .from
                    .and_then(|value| value.email_address)
                    .and_then(|value| value.name.or(value.address))
                    .unwrap_or_else(|| "Unknown sender".to_string()),
                received_at: message.received_date_time,
            })
            .collect())
    }
}

#[derive(Clone)]
pub struct GraphMailSender {
    client: GraphClient,
    tokens_repo: Arc<dyn TokensRepo>,
}

impl GraphMailSender {
    pub fn new(client: GraphClient, tokens_repo: Arc<dyn TokensRepo>) -> Self {
        Self {
            client,
            tokens_repo,
        }
    }
}

#[async_trait]
impl MailSender for GraphMailSender {
    async fn send_draft(
        &self,
        actor: &Actor,
        draft: &MailApprovalPayload,
    ) -> Result<(), MailSendError> {
        let token = self
            .tokens_repo
            .load_graph_token(actor)
            .await
            .map_err(|error| MailSendError::Permanent(error.message))?
            .ok_or_else(|| MailSendError::Permanent("Graph token unavailable".to_string()))?;

        let request = GraphSendMailRequest {
            message: GraphOutgoingMessage {
                subject: "Drafted by OfficeClaw".to_string(),
                body: GraphOutgoingBody {
                    content_type: "Text".to_string(),
                    content: draft.draft_text.clone(),
                },
                to_recipients: draft
                    .recipients
                    .iter()
                    .cloned()
                    .map(|address| GraphOutgoingRecipient {
                        email_address: GraphOutgoingEmailAddress { address },
                    })
                    .collect(),
            },
            save_to_sent_items: true,
        };

        self.client
            .post_no_content("/me/sendMail", &token.access_token, &request)
            .await
            .map_err(map_graph_send_error)
    }
}

#[derive(Clone)]
pub struct StaticMailReader {
    result: Result<Vec<UnreadMessage>, MailReadError>,
}

impl StaticMailReader {
    pub fn succeed(messages: Vec<UnreadMessage>) -> Self {
        Self {
            result: Ok(messages),
        }
    }

    pub fn fail(error: MailReadError) -> Self {
        Self { result: Err(error) }
    }
}

#[async_trait]
impl MailReader for StaticMailReader {
    async fn list_unread_today(&self, _actor: &Actor) -> Result<Vec<UnreadMessage>, MailReadError> {
        self.result.clone()
    }
}

#[derive(Clone)]
pub struct StaticMailSender {
    result: Result<(), MailSendError>,
}

impl StaticMailSender {
    pub fn succeed() -> Self {
        Self { result: Ok(()) }
    }

    pub fn fail(error: MailSendError) -> Self {
        Self { result: Err(error) }
    }
}

#[async_trait]
impl MailSender for StaticMailSender {
    async fn send_draft(
        &self,
        _actor: &Actor,
        _draft: &MailApprovalPayload,
    ) -> Result<(), MailSendError> {
        self.result.clone()
    }
}

pub fn summarize_unread_messages(messages: &[UnreadMessage]) -> String {
    if messages.is_empty() {
        return "You have no unread emails from today.".to_string();
    }

    let lines = messages
        .iter()
        .take(5)
        .map(|message| format!("- {}: {}", message.from, message.subject))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You have {} unread emails from today.\n{}",
        messages.len(),
        lines
    )
}

fn unread_today_path(start_of_day: chrono::DateTime<Utc>) -> String {
    format!(
        "/me/mailFolders/inbox/messages?$select=id,subject,receivedDateTime,from&$orderby=receivedDateTime%20desc&$filter=isRead%20eq%20false%20and%20receivedDateTime%20ge%20{}",
        start_of_day.format("%Y-%m-%dT%H:%M:%SZ")
    )
}

fn normalize_subject(subject: String) -> String {
    if subject.trim().is_empty() {
        return "(no subject)".to_string();
    }

    subject
}

fn map_graph_error(error: GraphClientError) -> MailReadError {
    if error.retryable {
        return MailReadError::Retryable(error.message);
    }

    MailReadError::Permanent(error.message)
}

fn map_graph_send_error(error: GraphClientError) -> MailSendError {
    if error.retryable {
        return MailSendError::Retryable(error.message);
    }

    MailSendError::Permanent(error.message)
}

#[derive(Debug, Deserialize)]
struct GraphListMessagesResponse {
    value: Vec<GraphMessage>,
}

#[derive(Debug, Deserialize)]
struct GraphMessage {
    id: String,
    subject: String,
    #[serde(rename = "receivedDateTime")]
    received_date_time: String,
    from: Option<GraphRecipient>,
}

#[derive(Debug, Deserialize)]
struct GraphRecipient {
    #[serde(rename = "emailAddress")]
    email_address: Option<GraphEmailAddress>,
}

#[derive(Debug, Deserialize)]
struct GraphEmailAddress {
    name: Option<String>,
    address: Option<String>,
}

#[derive(Debug, Serialize)]
struct GraphSendMailRequest {
    message: GraphOutgoingMessage,
    #[serde(rename = "saveToSentItems")]
    save_to_sent_items: bool,
}

#[derive(Debug, Serialize)]
struct GraphOutgoingMessage {
    subject: String,
    body: GraphOutgoingBody,
    #[serde(rename = "toRecipients")]
    to_recipients: Vec<GraphOutgoingRecipient>,
}

#[derive(Debug, Serialize)]
struct GraphOutgoingBody {
    #[serde(rename = "contentType")]
    content_type: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct GraphOutgoingRecipient {
    #[serde(rename = "emailAddress")]
    email_address: GraphOutgoingEmailAddress,
}

#[derive(Debug, Serialize)]
struct GraphOutgoingEmailAddress {
    address: String,
}

#[cfg(test)]
mod tests {
    use super::{
        summarize_unread_messages, unread_today_path, MailReadError, StaticMailReader,
        UnreadMessage,
    };
    use crate::domain::Actor;
    use crate::skills::graph::mail::MailReader;
    use chrono::{TimeZone, Utc};

    #[test]
    fn summarize_unread_messages_handles_empty_mailbox() {
        assert_eq!(
            summarize_unread_messages(&[]),
            "You have no unread emails from today."
        );
    }

    #[test]
    fn summarize_unread_messages_formats_top_messages() {
        let messages = vec![
            UnreadMessage {
                id: "1".to_string(),
                subject: "Budget review".to_string(),
                from: "James".to_string(),
                received_at: "2026-03-06T08:00:00Z".to_string(),
            },
            UnreadMessage {
                id: "2".to_string(),
                subject: "Ops update".to_string(),
                from: "Ops".to_string(),
                received_at: "2026-03-06T09:00:00Z".to_string(),
            },
        ];

        let summary = summarize_unread_messages(&messages);

        assert!(summary.contains("2 unread emails"));
        assert!(summary.contains("- James: Budget review"));
        assert!(summary.contains("- Ops: Ops update"));
    }

    #[tokio::test]
    async fn static_mail_reader_returns_configured_failure() {
        let reader = StaticMailReader::fail(MailReadError::Retryable("graph busy".to_string()));
        let actor = Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
        };

        let result = reader.list_unread_today(&actor).await;
        assert_eq!(
            result,
            Err(MailReadError::Retryable("graph busy".to_string()))
        );
    }

    #[test]
    fn unread_today_path_encodes_spaces_for_graph_query() {
        let start_of_day = Utc.with_ymd_and_hms(2026, 3, 6, 0, 0, 0).single().unwrap();

        let path = unread_today_path(start_of_day);

        assert!(!path.contains(' '));
        assert!(path.contains("receivedDateTime%20desc"));
        assert!(path.contains("isRead%20eq%20false"));
    }
}
