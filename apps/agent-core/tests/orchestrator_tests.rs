use std::collections::HashSet;

use agent_core::agent::orchestrator::Orchestrator;
use agent_core::domain::{Action, ActivityEnvelope, Actor, Conversation};

fn orchestrator() -> Orchestrator {
    let allowlist = HashSet::from(["contoso.com".to_string()]);
    let known = HashSet::from(["james@contoso.com".to_string()]);
    Orchestrator::new(allowlist, known)
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
        recipients: vec![],
        attendee_email: None,
        attendee_known: true,
        contains_sensitive: false,
        request_hour_local: 10,
    }
}

#[test]
fn summarize_unread_returns_actions() {
    let mut act = base_activity();
    act.text = "summarize unread emails from today".to_string();

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("Unread summary"));
    assert!(res.actions.contains(&"DRAFT_REPLIES".to_string()));
}

#[test]
fn schedule_requires_disambiguation_when_attendee_unknown() {
    let mut act = base_activity();
    act.text = "schedule 30 mins with James".to_string();
    act.attendee_known = false;

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("confirm attendee email"));
    assert_eq!(res.actions, vec!["PROVIDE_ATTENDEE_EMAIL".to_string()]);
}

#[test]
fn schedule_known_attendee_returns_slots() {
    let mut act = base_activity();
    act.text = "schedule 30 mins with James".to_string();

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("Proposed"));
    assert!(res.actions.iter().any(|a| a.starts_with("SELECT_SLOT")));
}

#[test]
fn select_slot_requires_invite_confirmation() {
    let mut act = base_activity();
    act.action = Some(Action::SelectSlot);

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("Confirm invite"));
    assert!(res.actions.contains(&"APPROVE_INVITE".to_string()));
}

#[test]
fn approve_send_with_safe_recipient_succeeds() {
    let mut act = base_activity();
    act.action = Some(Action::ApproveSend);
    act.recipients = vec!["james@contoso.com".to_string()];

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("Email sent"));
}

#[test]
fn approve_send_with_sensitive_content_is_blocked() {
    let mut act = base_activity();
    act.action = Some(Action::ApproveSend);
    act.recipients = vec!["james@contoso.com".to_string()];
    act.contains_sensitive = true;

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("Send blocked"));
    assert!(res.actions.contains(&"EDIT_DRAFT".to_string()));
}

#[test]
fn approve_send_with_malformed_recipient_is_blocked() {
    let mut act = base_activity();
    act.action = Some(Action::ApproveSend);
    act.recipients = vec!["not-an-email".to_string()];

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("MALFORMED_RECIPIENT"));
}

#[test]
fn unknown_text_returns_help() {
    let mut act = base_activity();
    act.text = "tell me a joke".to_string();

    let res = orchestrator().handle_activity(&act);

    assert!(res.actions.contains(&"HELP".to_string()));
}

#[test]
fn webhook_action_returns_proactive_message() {
    let mut act = base_activity();
    act.action = Some(Action::WebhookNotification);

    let res = orchestrator().handle_activity(&act);

    assert!(res.text.contains("Webhook processed"));
}
