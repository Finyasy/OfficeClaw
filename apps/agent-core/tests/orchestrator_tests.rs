use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::Mutex;

use agent_core::agent::orchestrator::Orchestrator;
use agent_core::agent::proactive::{NoopProactiveNotifier, ProactiveNotifier, ProactiveNotifierError};
use agent_core::domain::{
    Action, ActivityEnvelope, Actor, Conversation, OAuthTokenBundle, ProposedSlot,
    ProactiveDeliveryRequest, ResponseAction, SessionKey, SessionState,
};
use agent_core::skills::graph::calendar::{StaticCalendarEventCreator, StaticCalendarReader};
use agent_core::skills::graph::mail::{
    MailReadError, MailSendError, StaticMailReader, StaticMailSender, UnreadMessage,
};
use agent_core::storage::approvals_repo::{ApprovalsRepo, InMemoryApprovalsRepo};
use agent_core::storage::audit_repo::{AuditRepo, InMemoryAuditRepo};
use agent_core::storage::conversation_refs_repo::{ConversationRefsRepo, InMemoryConversationRefsRepo};
use agent_core::storage::sessions_repo::{InMemorySessionsRepo, SessionsRepo};
use agent_core::storage::tokens_repo::{InMemoryTokensRepo, TokensRepo};

#[derive(Clone, Default)]
struct RecordingNotifier {
    deliveries: Arc<Mutex<Vec<ProactiveDeliveryRequest>>>,
}

#[async_trait]
impl ProactiveNotifier for RecordingNotifier {
    async fn send(&self, request: &ProactiveDeliveryRequest) -> Result<(), ProactiveNotifierError> {
        self.deliveries.lock().await.push(request.clone());
        Ok(())
    }
}

impl RecordingNotifier {
    async fn count(&self) -> usize {
        self.deliveries.lock().await.len()
    }
}

fn action_ids(actions: &[ResponseAction]) -> Vec<String> {
    actions.iter().map(|action| action.id.clone()).collect()
}

fn approval_payload(actions: &[ResponseAction], action_id: &str) -> String {
    actions
        .iter()
        .find(|action| action.id == action_id)
        .map(|action| action.payload_json.clone())
        .expect("expected action payload to exist")
}

fn build_orchestrator(
    mail_reader: StaticMailReader,
    mail_sender: StaticMailSender,
    calendar_reader: StaticCalendarReader,
    calendar_event_creator: StaticCalendarEventCreator,
    notifier: Arc<dyn ProactiveNotifier>,
) -> (
    Orchestrator,
    Arc<InMemorySessionsRepo>,
    Arc<InMemoryAuditRepo>,
    Arc<InMemoryTokensRepo>,
    Arc<InMemoryApprovalsRepo>,
    Arc<InMemoryConversationRefsRepo>,
) {
    let allowlist = HashSet::from(["contoso.com".to_string()]);
    let known = HashSet::from(["james@contoso.com".to_string()]);
    let sessions = Arc::new(InMemorySessionsRepo::new());
    let audit = Arc::new(InMemoryAuditRepo::new());
    let tokens = Arc::new(InMemoryTokensRepo::new());
    let approvals = Arc::new(InMemoryApprovalsRepo::new());
    let conversation_refs = Arc::new(InMemoryConversationRefsRepo::new());

    let orchestrator = Orchestrator::new(
        allowlist,
        known,
        sessions.clone(),
        audit.clone(),
        approvals.clone(),
        conversation_refs.clone(),
        tokens.clone(),
        Arc::new(mail_reader),
        Arc::new(mail_sender),
        Arc::new(calendar_reader),
        Arc::new(calendar_event_creator),
        notifier,
    );

    (
        orchestrator,
        sessions,
        audit,
        tokens,
        approvals,
        conversation_refs,
    )
}

fn base_activity() -> ActivityEnvelope {
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
        attachments: vec![],
        action: None,
        action_payload_json: None,
        recipients: vec![],
        attendee_email: None,
        attendee_known: true,
        contains_sensitive: false,
        request_hour_local: 10,
        conversation_ref_json: None,
    }
}

fn sample_slots() -> Vec<ProposedSlot> {
    vec![
        ProposedSlot {
            start_utc: "2026-03-09T09:30:00+00:00".to_string(),
            end_utc: "2026-03-09T10:00:00+00:00".to_string(),
        },
        ProposedSlot {
            start_utc: "2026-03-09T10:00:00+00:00".to_string(),
            end_utc: "2026-03-09T10:30:00+00:00".to_string(),
        },
        ProposedSlot {
            start_utc: "2026-03-09T10:30:00+00:00".to_string(),
            end_utc: "2026-03-09T11:00:00+00:00".to_string(),
        },
    ]
}

#[tokio::test]
async fn summarize_unread_returns_actions_and_persists_session_state() {
    let messages = vec![UnreadMessage {
        id: "mail-1".to_string(),
        subject: "Budget review".to_string(),
        from: "James".to_string(),
        received_at: "2026-03-06T08:00:00Z".to_string(),
    }];
    let (orchestrator, sessions, audit, _tokens, _approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(messages),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut act = base_activity();
    act.text = "summarize unread emails from today".to_string();

    let res = orchestrator.handle_activity(&act).await;

    assert!(res.text.contains("1 unread emails"));
    assert!(action_ids(&res.actions).contains(&"DRAFT_REPLIES".to_string()));

    let state = sessions
        .load(&SessionKey::from_activity(&act))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.last_intent.as_deref(), Some("summarize_unread"));
    assert_eq!(state.unread_summary_count, Some(1));

    let events = audit.list().await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "MAIL_SUMMARY_POLICY_EVALUATED");
    assert_eq!(events[1].event_type, "MAIL_SUMMARY_SUCCEEDED");
}

#[tokio::test]
async fn summarize_unread_retryable_failure_returns_retry_action_and_audit() {
    let (orchestrator, _sessions, audit, _tokens, _approvals, _refs) = build_orchestrator(
        StaticMailReader::fail(MailReadError::Retryable("graph busy".to_string())),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut act = base_activity();
    act.text = "summarize unread emails from today".to_string();

    let res = orchestrator.handle_activity(&act).await;

    assert!(res.text.contains("Please retry"));
    assert_eq!(action_ids(&res.actions), vec!["RETRY_SUMMARY".to_string()]);

    let events = audit.list().await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].event_type, "MAIL_SUMMARY_RETRYABLE_FAILURE");
}

#[tokio::test]
async fn summarize_unread_empty_mailbox_returns_refresh_action() {
    let (orchestrator, _sessions, _audit, _tokens, _approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut act = base_activity();
    act.text = "summarize unread emails from today".to_string();

    let res = orchestrator.handle_activity(&act).await;

    assert!(res.text.contains("no unread emails"));
    assert_eq!(action_ids(&res.actions), vec!["REFRESH_SUMMARY".to_string()]);
}

#[tokio::test]
async fn schedule_requires_disambiguation_when_attendee_unknown() {
    let (orchestrator, _, _, _, _, _) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut act = base_activity();
    act.text = "schedule 30 mins with James".to_string();
    act.attendee_known = false;

    let res = orchestrator.handle_activity(&act).await;

    assert!(res.text.contains("confirm attendee email"));
    assert_eq!(
        action_ids(&res.actions),
        vec!["PROVIDE_ATTENDEE_EMAIL".to_string()]
    );
}

#[tokio::test]
async fn schedule_known_attendee_returns_slots_and_persists_session_state() {
    let (orchestrator, sessions, audit, _tokens, _approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut act = base_activity();
    act.text = "schedule 30 mins with James".to_string();

    let res = orchestrator.handle_activity(&act).await;

    assert!(res.text.contains("Proposed three available slots."));
    assert!(res.text.contains("2026-03-09T09:30:00+00:00"));
    assert_eq!(res.actions.len(), 3);
    assert!(res.actions[0].payload_json.contains("\"slot_index\":0"));

    let state = sessions
        .load(&SessionKey::from_activity(&act))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.last_intent.as_deref(), Some("schedule_meeting"));
    assert_eq!(state.proposed_slots.len(), 3);

    let events = audit.list().await.unwrap();
    assert_eq!(events[0].event_type, "SLOT_PROPOSAL_REQUESTED");
    assert_eq!(events[1].event_type, "SLOT_PROPOSAL_SUCCEEDED");
}

#[tokio::test]
async fn select_slot_creates_pending_invite_approval() {
    let (orchestrator, sessions, audit, _tokens, approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let act = base_activity();
    sessions
        .upsert(
            &SessionKey::from_activity(&act),
            &SessionState {
                last_intent: Some("schedule_meeting".to_string()),
                unread_summary_count: None,
                proposed_slots: sample_slots(),
            },
        )
        .await
        .unwrap();
    let mut action_activity = act.clone();
    action_activity.action = Some(Action::SelectSlot);
    action_activity.action_payload_json = Some(json!({ "slot_index": 0 }).to_string());

    let res = orchestrator.handle_activity(&action_activity).await;

    assert!(res.text.contains("2026-03-09T09:30:00+00:00"));
    assert!(action_ids(&res.actions).contains(&"APPROVE_INVITE".to_string()));

    let payload = approval_payload(&res.actions, "APPROVE_INVITE");
    let approval_id = serde_json::from_str::<serde_json::Value>(&payload)
        .unwrap()
        .get("approval_id")
        .and_then(|value| value.as_str())
        .unwrap()
        .parse()
        .unwrap();
    let approval = approvals.load(approval_id).await.unwrap().unwrap();
    assert_eq!(approval.status, "PENDING");

    let events = audit.list().await.unwrap();
    assert_eq!(events[0].event_type, "APPROVAL_CREATED");
}

#[tokio::test]
async fn approve_send_uses_persisted_approval_and_executes() {
    let (orchestrator, _sessions, audit, _tokens, approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut draft_activity = base_activity();
    draft_activity.text = "reply to this email".to_string();
    draft_activity.recipients = vec!["james@contoso.com".to_string()];

    let draft_response = orchestrator.handle_activity(&draft_activity).await;
    let payload = approval_payload(&draft_response.actions, "APPROVE_SEND");
    let approval_id = serde_json::from_str::<serde_json::Value>(&payload)
        .unwrap()
        .get("approval_id")
        .and_then(|value| value.as_str())
        .unwrap()
        .parse()
        .unwrap();

    let mut approval_activity = base_activity();
    approval_activity.action = Some(Action::ApproveSend);
    approval_activity.action_payload_json = Some(payload);
    let res = orchestrator.handle_activity(&approval_activity).await;

    assert!(res.text.contains("Email sent"));
    let approval = approvals.load(approval_id).await.unwrap().unwrap();
    assert_eq!(approval.status, "EXECUTED");

    let events = audit.list().await.unwrap();
    assert!(events.iter().any(|event| event.event_type == "APPROVAL_APPROVED"));
    assert!(events.iter().any(|event| event.event_type == "APPROVAL_EXECUTED"));
}

#[tokio::test]
async fn approve_send_with_sensitive_content_marks_approval_failed() {
    let (orchestrator, _sessions, _audit, _tokens, approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut draft_activity = base_activity();
    draft_activity.text = "reply to this email".to_string();
    draft_activity.recipients = vec!["james@contoso.com".to_string()];
    draft_activity.contains_sensitive = true;

    let draft_response = orchestrator.handle_activity(&draft_activity).await;
    let payload = approval_payload(&draft_response.actions, "APPROVE_SEND");
    let approval_id = serde_json::from_str::<serde_json::Value>(&payload)
        .unwrap()
        .get("approval_id")
        .and_then(|value| value.as_str())
        .unwrap()
        .parse()
        .unwrap();

    let mut approval_activity = base_activity();
    approval_activity.action = Some(Action::ApproveSend);
    approval_activity.action_payload_json = Some(payload);
    let res = orchestrator.handle_activity(&approval_activity).await;

    assert!(res.text.contains("Send blocked"));
    assert!(action_ids(&res.actions).contains(&"EDIT_DRAFT".to_string()));
    let approval = approvals.load(approval_id).await.unwrap().unwrap();
    assert_eq!(approval.status, "FAILED");
}

#[tokio::test]
async fn approve_send_retryable_failure_marks_approval_failed() {
    let (orchestrator, _sessions, _audit, _tokens, approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::fail(MailSendError::Retryable("graph busy".to_string())),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut draft_activity = base_activity();
    draft_activity.text = "reply to this email".to_string();
    draft_activity.recipients = vec!["james@contoso.com".to_string()];

    let draft_response = orchestrator.handle_activity(&draft_activity).await;
    let payload = approval_payload(&draft_response.actions, "APPROVE_SEND");
    let approval_id = serde_json::from_str::<serde_json::Value>(&payload)
        .unwrap()
        .get("approval_id")
        .and_then(|value| value.as_str())
        .unwrap()
        .parse()
        .unwrap();

    let mut approval_activity = base_activity();
    approval_activity.action = Some(Action::ApproveSend);
    approval_activity.action_payload_json = Some(payload);

    let res = orchestrator.handle_activity(&approval_activity).await;

    assert!(res.text.contains("Retry later"));
    assert!(action_ids(&res.actions).contains(&"RETRY_SEND".to_string()));
    let approval = approvals.load(approval_id).await.unwrap().unwrap();
    assert_eq!(approval.status, "FAILED");
}

#[tokio::test]
async fn approve_invite_uses_persisted_payload_and_executes() {
    let (orchestrator, sessions, _audit, _tokens, approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-42"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut act = base_activity();
    act.attendee_email = Some("james@contoso.com".to_string());
    sessions
        .upsert(
            &SessionKey::from_activity(&act),
            &SessionState {
                last_intent: Some("schedule_meeting".to_string()),
                unread_summary_count: None,
                proposed_slots: sample_slots(),
            },
        )
        .await
        .unwrap();

    let mut select_activity = act.clone();
    select_activity.action = Some(Action::SelectSlot);
    select_activity.action_payload_json = Some(json!({ "slot_index": 0 }).to_string());
    let select_response = orchestrator.handle_activity(&select_activity).await;
    let payload = approval_payload(&select_response.actions, "APPROVE_INVITE");
    let approval_id = serde_json::from_str::<serde_json::Value>(&payload)
        .unwrap()
        .get("approval_id")
        .and_then(|value| value.as_str())
        .unwrap()
        .parse()
        .unwrap();

    let mut approval_activity = base_activity();
    approval_activity.action = Some(Action::ApproveInvite);
    approval_activity.action_payload_json = Some(payload);

    let res = orchestrator.handle_activity(&approval_activity).await;

    assert!(res.text.contains("Booked. Invite sent"));
    let approval = approvals.load(approval_id).await.unwrap().unwrap();
    assert_eq!(approval.status, "EXECUTED");
}

#[tokio::test]
async fn approve_invite_without_attendee_email_fails() {
    let (orchestrator, sessions, _audit, _tokens, approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let act = base_activity();
    sessions
        .upsert(
            &SessionKey::from_activity(&act),
            &SessionState {
                last_intent: Some("schedule_meeting".to_string()),
                unread_summary_count: None,
                proposed_slots: sample_slots(),
            },
        )
        .await
        .unwrap();

    let mut select_activity = act.clone();
    select_activity.action = Some(Action::SelectSlot);
    select_activity.action_payload_json = Some(json!({ "slot_index": 0 }).to_string());
    let select_response = orchestrator.handle_activity(&select_activity).await;
    let payload = approval_payload(&select_response.actions, "APPROVE_INVITE");
    let approval_id = serde_json::from_str::<serde_json::Value>(&payload)
        .unwrap()
        .get("approval_id")
        .and_then(|value| value.as_str())
        .unwrap()
        .parse()
        .unwrap();

    let mut approval_activity = base_activity();
    approval_activity.action = Some(Action::ApproveInvite);
    approval_activity.action_payload_json = Some(payload);

    let res = orchestrator.handle_activity(&approval_activity).await;

    assert!(res.text.contains("Cannot create event yet"));
    assert!(action_ids(&res.actions).contains(&"PROVIDE_ATTENDEE_EMAIL".to_string()));
    let approval = approvals.load(approval_id).await.unwrap().unwrap();
    assert_eq!(approval.status, "FAILED");
}

#[tokio::test]
async fn cancel_action_updates_pending_approval() {
    let (orchestrator, _sessions, _audit, _tokens, approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut draft_activity = base_activity();
    draft_activity.text = "reply to this email".to_string();

    let draft_response = orchestrator.handle_activity(&draft_activity).await;
    let payload = approval_payload(&draft_response.actions, "CANCEL");
    let approval_id = serde_json::from_str::<serde_json::Value>(&payload)
        .unwrap()
        .get("approval_id")
        .and_then(|value| value.as_str())
        .unwrap()
        .parse()
        .unwrap();

    let mut cancel_activity = base_activity();
    cancel_activity.action = Some(Action::Cancel);
    cancel_activity.action_payload_json = Some(payload);

    let res = orchestrator.handle_activity(&cancel_activity).await;

    assert!(res.text.contains("cancelled"));
    let approval = approvals.load(approval_id).await.unwrap().unwrap();
    assert_eq!(approval.status, "CANCELLED");
}

#[tokio::test]
async fn unknown_text_returns_help() {
    let (orchestrator, _, _, _, _, _) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let mut act = base_activity();
    act.text = "tell me a joke".to_string();

    let res = orchestrator.handle_activity(&act).await;

    assert!(action_ids(&res.actions).contains(&"HELP".to_string()));
}

#[tokio::test]
async fn webhook_action_sends_proactive_message_when_conversation_ref_exists() {
    let notifier = Arc::new(RecordingNotifier::default());
    let (orchestrator, _sessions, _audit, _tokens, _approvals, refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        notifier.clone(),
    );
    let mut act = base_activity();
    act.conversation_ref_json = Some(
        serde_json::json!({
            "serviceUrl": "https://service",
            "conversation": { "id": "conv-1" }
        })
        .to_string(),
    );

    let _ = orchestrator.handle_activity(&act).await;
    let stored = refs
        .load("tenant-1", "user-1", "teams", "conv-1")
        .await
        .unwrap();
    assert!(stored.is_some());

    let mut webhook_activity = act.clone();
    webhook_activity.action = Some(Action::WebhookNotification);

    let res = orchestrator.handle_activity(&webhook_activity).await;

    assert!(res.text.contains("Webhook processed"));
    assert_eq!(notifier.count().await, 1);
}

#[tokio::test]
async fn oauth_callback_stores_graph_token() {
    let (orchestrator, _sessions, audit, tokens, _approvals, _refs) = build_orchestrator(
        StaticMailReader::succeed(vec![]),
        StaticMailSender::succeed(),
        StaticCalendarReader::succeed(sample_slots()),
        StaticCalendarEventCreator::succeed("event-1"),
        Arc::new(NoopProactiveNotifier),
    );
    let actor = Actor {
        tenant_id: "tenant-1".to_string(),
        user_id: "user-1".to_string(),
    };
    let token = OAuthTokenBundle {
        access_token: "access-token".to_string(),
        refresh_token: Some("refresh-token".to_string()),
        expires_at_utc: Some("2026-03-06T12:00:00Z".to_string()),
        scope: Some("Mail.Read Calendars.Read".to_string()),
    };

    orchestrator
        .handle_oauth_callback(&actor, &token)
        .await
        .unwrap();

    let loaded = tokens.load_graph_token(&actor).await.unwrap();
    assert_eq!(loaded, Some(token));

    let events = audit.list().await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "GRAPH_TOKEN_STORED");
}
