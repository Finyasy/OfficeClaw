use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::policy::state_machine::ApprovalStatus;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub tenant_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub channel: String,
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRef {
    pub kind: String,
    pub id: String,
    pub data_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    ApproveSend,
    EditDraft,
    SelectSlot,
    ApproveInvite,
    Cancel,
    ConfirmExternalSend,
    WebhookNotification,
    Unknown(String),
}

impl Action {
    pub fn from_str(value: &str) -> Self {
        match value {
            "APPROVE_SEND" => Self::ApproveSend,
            "EDIT_DRAFT" => Self::EditDraft,
            "APPROVE_INVITE" => Self::ApproveInvite,
            "CANCEL" => Self::Cancel,
            "CONFIRM_EXTERNAL_SEND" => Self::ConfirmExternalSend,
            "WEBHOOK_NOTIFICATION" => Self::WebhookNotification,
            value if value.starts_with("SELECT_SLOT") => Self::SelectSlot,
            other => Self::Unknown(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityEnvelope {
    pub actor: Actor,
    pub conversation: Conversation,
    pub text: String,
    pub attachments: Vec<AttachmentRef>,
    pub action: Option<Action>,
    pub action_payload_json: Option<String>,
    pub recipients: Vec<String>,
    pub attendee_email: Option<String>,
    pub attendee_known: bool,
    pub contains_sensitive: bool,
    pub request_hour_local: u8,
    pub conversation_ref_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResponse {
    pub text: String,
    pub actions: Vec<ResponseAction>,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseAction {
    pub id: String,
    pub label: String,
    pub payload_json: String,
    pub style: Option<String>,
}

impl ResponseAction {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        payload_json: impl Into<String>,
        style: Option<&str>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            payload_json: payload_json.into(),
            style: style.map(str::to_string),
        }
    }

    pub fn simple(id: &str) -> Self {
        Self::new(id, id, "", None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub tenant_id: String,
    pub user_id: String,
    pub channel: String,
    pub conversation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionState {
    pub last_intent: Option<String>,
    pub unread_summary_count: Option<usize>,
    pub proposed_slots: Vec<ProposedSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedSlot {
    pub start_utc: String,
    pub end_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthTokenBundle {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_utc: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalKind {
    SendMail,
    CreateEvent,
}

impl ApprovalKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SendMail => "SEND_MAIL",
            Self::CreateEvent => "CREATE_EVENT",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "SEND_MAIL" => Some(Self::SendMail),
            "CREATE_EVENT" => Some(Self::CreateEvent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub approval_id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub channel: String,
    pub conversation_id: String,
    pub kind: String,
    pub status: String,
    pub risk_level: String,
    pub payload_json: Value,
    pub policy_snapshot_json: Value,
    pub expires_at_utc: String,
}

impl ApprovalRecord {
    pub fn status_enum(&self) -> Option<ApprovalStatus> {
        match self.status.as_str() {
            "PENDING" => Some(ApprovalStatus::Pending),
            "APPROVED" => Some(ApprovalStatus::Approved),
            "REJECTED" => Some(ApprovalStatus::Rejected),
            "EXPIRED" => Some(ApprovalStatus::Expired),
            "CANCELLED" => Some(ApprovalStatus::Cancelled),
            "EXECUTED" => Some(ApprovalStatus::Executed),
            "FAILED" => Some(ApprovalStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailApprovalPayload {
    pub recipients: Vec<String>,
    pub draft_text: String,
    pub contains_sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateEventApprovalPayload {
    pub slot_index: usize,
    pub start_utc: String,
    pub end_utc: String,
    pub attendee_email: Option<String>,
    pub request_hour_local: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationRefRecord {
    pub tenant_id: String,
    pub user_id: String,
    pub channel: String,
    pub conversation_id: String,
    pub ref_json: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProactiveDeliveryRequest {
    pub actor: Actor,
    pub conversation: Conversation,
    pub conversation_ref_json: String,
    pub text: String,
    pub adaptive_card_json: Option<String>,
    pub correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEventRecord {
    pub event_id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub channel: String,
    pub conversation_id: String,
    pub correlation_id: String,
    pub event_type: String,
    pub event_json: Value,
}

impl SessionKey {
    pub fn from_activity(activity: &ActivityEnvelope) -> Self {
        Self {
            tenant_id: activity.actor.tenant_id.clone(),
            user_id: activity.actor.user_id.clone(),
            channel: activity.conversation.channel.clone(),
            conversation_id: activity.conversation.conversation_id.clone(),
        }
    }
}

impl ConversationRefRecord {
    pub fn from_activity(activity: &ActivityEnvelope) -> Option<Self> {
        let ref_json = activity.conversation_ref_json.as_ref()?;
        let parsed = serde_json::from_str(ref_json).ok()?;

        Some(Self {
            tenant_id: activity.actor.tenant_id.clone(),
            user_id: activity.actor.user_id.clone(),
            channel: activity.conversation.channel.clone(),
            conversation_id: activity.conversation.conversation_id.clone(),
            ref_json: parsed,
        })
    }
}

impl AuditEventRecord {
    pub fn from_activity(
        activity: &ActivityEnvelope,
        correlation_id: &str,
        event_type: impl Into<String>,
        event_json: Value,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            tenant_id: activity.actor.tenant_id.clone(),
            user_id: activity.actor.user_id.clone(),
            channel: activity.conversation.channel.clone(),
            conversation_id: activity.conversation.conversation_id.clone(),
            correlation_id: correlation_id.to_string(),
            event_type: event_type.into(),
            event_json,
        }
    }
}
