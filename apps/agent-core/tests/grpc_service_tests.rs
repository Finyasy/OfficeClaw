use std::collections::HashSet;
use std::sync::Arc;

use agent_core::agent::orchestrator::Orchestrator;
use agent_core::agent::proactive::NoopProactiveNotifier;
use agent_core::api::grpc::AgentGatewayService;
use agent_core::domain::Actor as DomainActor;
use agent_core::proto::agent_gateway_server::AgentGateway;
use agent_core::proto::{ActivityEnvelope, Actor, AuthEnvelope, Conversation, ProactiveMessage};
use agent_core::skills::graph::calendar::{StaticCalendarEventCreator, StaticCalendarReader};
use agent_core::skills::graph::mail::{StaticMailReader, StaticMailSender, UnreadMessage};
use agent_core::storage::approvals_repo::InMemoryApprovalsRepo;
use agent_core::storage::audit_repo::{AuditRepo, InMemoryAuditRepo};
use agent_core::storage::conversation_refs_repo::InMemoryConversationRefsRepo;
use agent_core::storage::sessions_repo::InMemorySessionsRepo;
use agent_core::storage::tokens_repo::{InMemoryTokensRepo, TokensRepo};
use tonic::Request;

fn service() -> (
    AgentGatewayService,
    Arc<InMemoryTokensRepo>,
    Arc<InMemoryAuditRepo>,
) {
    let allowlist = HashSet::from(["contoso.com".to_string()]);
    let known = HashSet::from(["james@contoso.com".to_string()]);
    let messages = vec![UnreadMessage {
        id: "mail-1".to_string(),
        subject: "Budget review".to_string(),
        from: "James".to_string(),
        received_at: "2026-03-06T08:00:00Z".to_string(),
    }];
    let tokens = Arc::new(InMemoryTokensRepo::new());
    let audit = Arc::new(InMemoryAuditRepo::new());

    (
        AgentGatewayService::new(Arc::new(Orchestrator::new(
            allowlist,
            known,
            Arc::new(InMemorySessionsRepo::new()),
            audit.clone(),
            Arc::new(InMemoryApprovalsRepo::new()),
            Arc::new(InMemoryConversationRefsRepo::new()),
            tokens.clone(),
            Arc::new(StaticMailReader::succeed(messages)),
            Arc::new(StaticMailSender::succeed()),
            Arc::new(StaticCalendarReader::succeed(vec![])),
            Arc::new(StaticCalendarEventCreator::succeed("event-1")),
            Arc::new(NoopProactiveNotifier),
        ))),
        tokens,
        audit,
    )
}

fn base_request() -> ActivityEnvelope {
    ActivityEnvelope {
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
        text: String::new(),
        attachments: vec![],
        action: String::new(),
        action_payload_json: String::new(),
        recipients: vec![],
        contains_sensitive: false,
        request_hour_local: 10,
        attendee_known: true,
        conversation_ref_json: String::new(),
        attendee_email: String::new(),
    }
}

#[tokio::test]
async fn handle_activity_maps_message_and_returns_response() {
    let (svc, _tokens, _audit) = service();
    let mut request = base_request();
    request.text = "summarize unread emails from today".to_string();

    let response = svc
        .handle_activity(Request::new(request))
        .await
        .expect("grpc handle_activity should succeed")
        .into_inner();

    assert!(response.text.contains("1 unread emails"));
    assert!(!response.correlation_id.is_empty());
    assert!(response.actions.iter().any(|action| action.id == "DRAFT_REPLIES"));
}

#[tokio::test]
async fn handle_activity_maps_approval_action_and_executes_send() {
    let (svc, _tokens, _audit) = service();

    let draft_response = svc
        .handle_activity(Request::new(ActivityEnvelope {
            text: "reply to this email".to_string(),
            recipients: vec!["james@contoso.com".to_string()],
            ..base_request()
        }))
        .await
        .expect("draft request should succeed")
        .into_inner();
    let approval_payload = draft_response
        .actions
        .iter()
        .find(|action| action.id == "APPROVE_SEND")
        .map(|action| action.payload_json.clone())
        .expect("approval payload must exist");

    let response = svc
        .handle_activity(Request::new(ActivityEnvelope {
            action: "APPROVE_SEND".to_string(),
            action_payload_json: approval_payload,
            ..base_request()
        }))
        .await
        .expect("grpc handle_activity should succeed")
        .into_inner();

    assert!(response.text.contains("Email sent"));
}

#[tokio::test]
async fn oauth_callback_stores_token_and_emits_ack() {
    let (svc, tokens, audit) = service();
    let request = AuthEnvelope {
        actor: Some(Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            user_display_name: "Bryan".to_string(),
        }),
        provider: "graph".to_string(),
        access_token: "access".to_string(),
        refresh_token: "refresh".to_string(),
        expires_at_utc: "2026-03-06T12:00:00Z".to_string(),
        scope: "Mail.Read Calendars.Read".to_string(),
    };

    let response = svc
        .o_auth_callback(Request::new(request))
        .await
        .expect("oauth callback should succeed")
        .into_inner();

    assert!(response.ok);
    let token = tokens
        .load_graph_token(&DomainActor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(token.access_token, "access");

    let events = audit.list().await.unwrap();
    assert_eq!(events[0].event_type, "GRAPH_TOKEN_STORED");
}

#[tokio::test]
async fn send_proactive_requires_text() {
    let (svc, _tokens, _audit) = service();

    let request = ProactiveMessage {
        actor: Some(Actor {
            tenant_id: "tenant-1".to_string(),
            user_id: "user-1".to_string(),
            user_display_name: String::new(),
        }),
        conversation: Some(Conversation {
            channel: "teams".to_string(),
            conversation_id: "conv-1".to_string(),
            thread_id: String::new(),
            message_id: String::new(),
        }),
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
async fn send_proactive_requires_stored_conversation_ref() {
    let (svc, _tokens, _audit) = service();

    let err = svc
        .send_proactive(Request::new(ProactiveMessage {
            actor: Some(Actor {
                tenant_id: "tenant-1".to_string(),
                user_id: "user-1".to_string(),
                user_display_name: String::new(),
            }),
            conversation: Some(Conversation {
                channel: "teams".to_string(),
                conversation_id: "conv-1".to_string(),
                thread_id: String::new(),
                message_id: String::new(),
            }),
            text: "Draft ready".to_string(),
            adaptive_card_json: String::new(),
            correlation_id: "corr-2".to_string(),
        }))
        .await
        .expect_err("missing conversation ref must fail");

    assert_eq!(err.code(), tonic::Code::FailedPrecondition);
}

#[tokio::test]
async fn send_proactive_accepts_valid_message_after_activity_persists_reference() {
    let (svc, _tokens, _audit) = service();

    let _ = svc
        .handle_activity(Request::new(ActivityEnvelope {
            text: "summarize unread emails from today".to_string(),
            conversation_ref_json: serde_json::json!({
                "serviceUrl": "https://smba.trafficmanager.net/teams/",
                "conversation": { "id": "conv-1" }
            })
            .to_string(),
            ..base_request()
        }))
        .await
        .expect("seed activity should succeed");

    let response = svc
        .send_proactive(Request::new(ProactiveMessage {
            actor: Some(Actor {
                tenant_id: "tenant-1".to_string(),
                user_id: "user-1".to_string(),
                user_display_name: String::new(),
            }),
            conversation: Some(Conversation {
                channel: "teams".to_string(),
                conversation_id: "conv-1".to_string(),
                thread_id: String::new(),
                message_id: String::new(),
            }),
            text: "Draft ready".to_string(),
            adaptive_card_json: String::new(),
            correlation_id: "corr-3".to_string(),
        }))
        .await
        .expect("valid proactive message should pass")
        .into_inner();

    assert!(response.ok);
}
