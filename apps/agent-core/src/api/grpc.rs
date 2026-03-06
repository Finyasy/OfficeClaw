use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::agent::orchestrator::Orchestrator;
use crate::domain::{
    Action, ActivityEnvelope, Actor, AttachmentRef, Conversation, OAuthTokenBundle,
};
use crate::proto::agent_gateway_server::AgentGateway;
use crate::proto::{
    Ack, AgentResponse as ProtoAgentResponse, AuthEnvelope, ProactiveMessage, UiAction,
};

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
        action_payload_json: empty_to_none(input.action_payload_json),
        recipients: input.recipients,
        attendee_email: empty_to_none(input.attendee_email),
        attendee_known: input.attendee_known,
        contains_sensitive: input.contains_sensitive,
        request_hour_local: normalize_hour(input.request_hour_local),
        conversation_ref_json: empty_to_none(input.conversation_ref_json),
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

fn empty_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn map_response(output: crate::domain::AgentResponse) -> ProtoAgentResponse {
    let actions = output
        .actions
        .into_iter()
        .map(|action| UiAction {
            id: action.id,
            label: action.label,
            payload_json: action.payload_json,
            style: action.style.unwrap_or_default(),
        })
        .collect();

    ProtoAgentResponse {
        text: output.text,
        adaptive_card_json: String::new(),
        actions,
        correlation_id: output.correlation_id,
    }
}

fn map_auth(input: AuthEnvelope) -> Result<(Actor, OAuthTokenBundle), Status> {
    let AuthEnvelope {
        actor,
        provider,
        access_token,
        refresh_token,
        expires_at_utc,
        scope,
    } = input;

    let actor = actor.ok_or_else(|| Status::invalid_argument("actor is required"))?;

    if provider.trim() != "graph" {
        return Err(Status::invalid_argument("provider must be graph"));
    }

    if access_token.trim().is_empty() {
        return Err(Status::invalid_argument("access_token is required"));
    }

    Ok((
        Actor {
            tenant_id: actor.tenant_id,
            user_id: actor.user_id,
        },
        OAuthTokenBundle {
            access_token,
            refresh_token: if refresh_token.trim().is_empty() {
                None
            } else {
                Some(refresh_token)
            },
            expires_at_utc: if expires_at_utc.trim().is_empty() {
                None
            } else {
                Some(expires_at_utc)
            },
            scope: if scope.trim().is_empty() {
                None
            } else {
                Some(scope)
            },
        },
    ))
}

#[tonic::async_trait]
impl AgentGateway for AgentGatewayService {
    async fn handle_activity(
        &self,
        request: Request<crate::proto::ActivityEnvelope>,
    ) -> Result<Response<ProtoAgentResponse>, Status> {
        let activity = map_activity(request.into_inner());
        let response = self.orchestrator.handle_activity(&activity).await;
        Ok(Response::new(map_response(response)))
    }

    async fn o_auth_callback(
        &self,
        request: Request<AuthEnvelope>,
    ) -> Result<Response<Ack>, Status> {
        let (actor, token) = map_auth(request.into_inner())?;
        self.orchestrator
            .handle_oauth_callback(&actor, &token)
            .await
            .map_err(|error| Status::internal(error.message))?;

        Ok(Response::new(Ack {
            ok: true,
            message: "oauth token stored".to_string(),
        }))
    }

    async fn send_proactive(
        &self,
        request: Request<ProactiveMessage>,
    ) -> Result<Response<Ack>, Status> {
        let message = request.into_inner();

        if message.text.trim().is_empty() {
            return Err(Status::invalid_argument(
                "text is required for proactive messages",
            ));
        }

        let actor = message
            .actor
            .ok_or_else(|| Status::invalid_argument("actor is required"))?;
        let conversation = message
            .conversation
            .ok_or_else(|| Status::invalid_argument("conversation is required"))?;

        self.orchestrator
            .send_proactive(
                &Actor {
                    tenant_id: actor.tenant_id,
                    user_id: actor.user_id,
                },
                &normalize_or_default(conversation.channel, "teams"),
                &conversation.conversation_id,
                &message.text,
                message.correlation_id,
            )
            .await
            .map_err(|error| Status::failed_precondition(error.message))?;

        Ok(Response::new(Ack {
            ok: true,
            message: "proactive delivery accepted".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{map_auth, normalize_hour};
    use crate::proto::{Actor as ProtoActor, AuthEnvelope};

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

    #[test]
    fn map_auth_requires_graph_provider() {
        let err = map_auth(AuthEnvelope {
            actor: Some(ProtoActor {
                tenant_id: "tenant-1".to_string(),
                user_id: "user-1".to_string(),
                user_display_name: String::new(),
            }),
            provider: "other".to_string(),
            access_token: "token".to_string(),
            refresh_token: String::new(),
            expires_at_utc: String::new(),
            scope: String::new(),
        })
        .expect_err("non-graph provider must fail");

        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }
}
