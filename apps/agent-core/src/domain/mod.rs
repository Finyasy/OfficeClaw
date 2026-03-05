#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub tenant_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
            "SELECT_SLOT" => Self::SelectSlot,
            "APPROVE_INVITE" => Self::ApproveInvite,
            "CANCEL" => Self::Cancel,
            "CONFIRM_EXTERNAL_SEND" => Self::ConfirmExternalSend,
            "WEBHOOK_NOTIFICATION" => Self::WebhookNotification,
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
    pub recipients: Vec<String>,
    pub attendee_email: Option<String>,
    pub attendee_known: bool,
    pub contains_sensitive: bool,
    pub request_hour_local: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResponse {
    pub text: String,
    pub actions: Vec<String>,
    pub correlation_id: String,
}
