use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::agent::orchestrator::Orchestrator;
use crate::domain::{
    Action, ActivityEnvelope, Actor, AttachmentRef, Conversation,
};
use crate::proto::agent_gateway_server::AgentGateway;
use crate::proto::{Ack, AgentResponse as ProtoAgentResponse, ProactiveMessage, UiAction};

#[derive(Clone)]
pub struct AgentGatewayService {
    orchestrator: Arc<Orchestrator>,
}

impl AgentGatewayService {
    pub fn new(orchestrator: Arc<Orchestrator>) -> Self {
        Self { orchestrator }
    }
}

fn map_activity(input: crate::proto::ActivityEnvelope) -> ActivityEnvelope {
    let actor = input.actor.unwrap_or_default();
    let conversation = input.conversation.unwrap_or_default();

    ActivityEnvelope {
        actor: Actor {
            tenant_id: actor.tenant_id,
            user_id: actor.user_id,
        },
        conversation: Conversation {
            channel: normalize_or_default(conversation.channel, "teams"),
            conversation_id: conversation.conversation_id,
            message_id: conversation.message_id,
        },
        text: input.text,
        attachments: input
            .attachments
            .into_iter()
            .map(|attachment| AttachmentRef {
                kind: attachment.kind,
                id: attachment.id,
                data_json: attachment.data_json,
            })
            .collect(),
        action: to_action(input.action),
        recipients: input.recipients,
        attendee_email: None,
        attendee_known: input.attendee_known,
        contains_sensitive: input.contains_sensitive,
        request_hour_local: normalize_hour(input.request_hour_local),
    }
}

fn normalize_or_default(value: String, default: &str) -> String {
    if value.trim().is_empty() {
        return default.to_string();
    }

    value
}

fn normalize_hour(value: u32) -> u8 {
    if value <= 23 {
        return value as u8;
    }

    10
}

fn to_action(action: String) -> Option<Action> {
    if action.trim().is_empty() {
        return None;
    }

    Some(Action::from_str(action.trim()))
}

fn map_response(output: crate::domain::AgentResponse) -> ProtoAgentResponse {
    let actions = output
        .actions
        .into_iter()
        .map(|action| UiAction {
            id: action.clone(),
            label: action,
            payload_json: String::new(),
            style: String::new(),
        })
        .collect();

    ProtoAgentResponse {
        text: output.text,
        adaptive_card_json: String::new(),
        actions,
        correlation_id: output.correlation_id,
    }
}

#[tonic::async_trait]
impl AgentGateway for AgentGatewayService {
    async fn handle_activity(
        &self,
        request: Request<crate::proto::ActivityEnvelope>,
    ) -> Result<Response<ProtoAgentResponse>, Status> {
        let activity = map_activity(request.into_inner());
        let response = self.orchestrator.handle_activity(&activity);
        Ok(Response::new(map_response(response)))
    }

    async fn send_proactive(
        &self,
        request: Request<ProactiveMessage>,
    ) -> Result<Response<Ack>, Status> {
        let message = request.into_inner();

        if message.text.trim().is_empty() {
            return Err(Status::invalid_argument("text is required for proactive messages"));
        }

        Ok(Response::new(Ack {
            ok: true,
            message: "proactive delivery accepted".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_hour;

    #[test]
    fn normalize_hour_keeps_valid_values() {
        assert_eq!(normalize_hour(0), 0);
        assert_eq!(normalize_hour(23), 23);
    }

    #[test]
    fn normalize_hour_defaults_invalid_values() {
        assert_eq!(normalize_hour(24), 10);
        assert_eq!(normalize_hour(255), 10);
    }
}
