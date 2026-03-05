use std::collections::HashSet;
use std::sync::Arc;

use agent_core::agent::orchestrator::Orchestrator;
use agent_core::api::grpc::AgentGatewayService;
use agent_core::proto::agent_gateway_server::AgentGateway;
use agent_core::proto::{
    ActivityEnvelope, Actor, Conversation, ProactiveMessage,
};
use tonic::Request;

fn service() -> AgentGatewayService {
    let allowlist = HashSet::from(["contoso.com".to_string()]);
    let known = HashSet::from(["james@contoso.com".to_string()]);
    AgentGatewayService::new(Arc::new(Orchestrator::new(allowlist, known)))
}

#[tokio::test]
async fn handle_activity_maps_message_and_returns_response() {
    let svc = service();

    let request = ActivityEnvelope {
        actor: Some(Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            user_display_name: "Bryan".to_string(),
        }),
        conversation: Some(Conversation {
            channel: "teams".to_string(),
            conversation_id: "conv-1".to_string(),
            thread_id: String::new(),
            message_id: "msg-1".to_string(),
        }),
        text: "summarize unread emails from today".to_string(),
        attachments: vec![],
        action: String::new(),
        action_payload_json: String::new(),
        recipients: vec![],
        contains_sensitive: false,
        request_hour_local: 10,
        attendee_known: true,
    };

    let response = svc
        .handle_activity(Request::new(request))
        .await
        .expect("grpc handle_activity should succeed")
        .into_inner();

    assert!(response.text.contains("Unread summary"));
    assert!(!response.correlation_id.is_empty());
}

#[tokio::test]
async fn handle_activity_maps_action_and_blocks_sensitive_send() {
    let svc = service();

    let request = ActivityEnvelope {
        actor: Some(Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            user_display_name: String::new(),
        }),
        conversation: Some(Conversation {
            channel: "teams".to_string(),
            conversation_id: "conv-1".to_string(),
            thread_id: String::new(),
            message_id: "msg-1".to_string(),
        }),
        text: String::new(),
        attachments: vec![],
        action: "APPROVE_SEND".to_string(),
        action_payload_json: String::new(),
        recipients: vec!["james@contoso.com".to_string()],
        contains_sensitive: true,
        request_hour_local: 10,
        attendee_known: true,
    };

    let response = svc
        .handle_activity(Request::new(request))
        .await
        .expect("grpc handle_activity should succeed")
        .into_inner();

    assert!(response.text.contains("Send blocked"));
}

#[tokio::test]
async fn send_proactive_requires_text() {
    let svc = service();

    let request = ProactiveMessage {
        actor: None,
        conversation: None,
        text: String::new(),
        adaptive_card_json: String::new(),
        correlation_id: "corr-1".to_string(),
    };

    let err = svc
        .send_proactive(Request::new(request))
        .await
        .expect_err("empty proactive text must fail");

    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn send_proactive_accepts_valid_message() {
    let svc = service();

    let request = ProactiveMessage {
        actor: None,
        conversation: None,
        text: "Draft ready".to_string(),
        adaptive_card_json: String::new(),
        correlation_id: "corr-2".to_string(),
    };

    let response = svc
        .send_proactive(Request::new(request))
        .await
        .expect("valid proactive message should pass")
        .into_inner();

    assert!(response.ok);
}
