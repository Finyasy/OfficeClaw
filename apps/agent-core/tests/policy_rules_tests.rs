use std::collections::HashSet;

use agent_core::policy::rules::{
    evaluate, is_business_hours, OperationKind, PolicyDecision, RiskLevel, RuleInput,
};

fn base_known() -> HashSet<String> {
    HashSet::from([
        "james@contoso.com".to_string(),
        "ops@contoso.com".to_string(),
    ])
}

fn base_allowlist() -> HashSet<String> {
    HashSet::from(["contoso.com".to_string()])
}

#[test]
fn read_only_operation_is_allowed() {
    let recipients: Vec<String> = vec![];
    let decision = evaluate(RuleInput {
        kind: OperationKind::ReadOnly,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 10,
        attendee_known: true,
    });

    assert_eq!(decision, PolicyDecision::Allow);
}

#[test]
fn send_mail_requires_recipient() {
    let recipients: Vec<String> = vec![];
    let decision = evaluate(RuleInput {
        kind: OperationKind::SendMail,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 10,
        attendee_known: true,
    });

    assert_eq!(decision, PolicyDecision::Deny("NO_RECIPIENTS"));
}

#[test]
fn malformed_recipient_is_denied() {
    let recipients = vec!["invalid-address".to_string()];
    let decision = evaluate(RuleInput {
        kind: OperationKind::SendMail,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 10,
        attendee_known: true,
    });

    assert_eq!(decision, PolicyDecision::Deny("MALFORMED_RECIPIENT"));
}

#[test]
fn sensitive_content_send_is_denied() {
    let recipients = vec!["james@contoso.com".to_string()];
    let decision = evaluate(RuleInput {
        kind: OperationKind::SendMail,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: true,
        local_hour: 10,
        attendee_known: true,
    });

    assert_eq!(
        decision,
        PolicyDecision::Deny("SENSITIVE_CONTENT_REQUIRES_EDIT")
    );
}

#[test]
fn external_recipient_is_high_risk() {
    let recipients = vec!["legal@external.com".to_string()];
    let decision = evaluate(RuleInput {
        kind: OperationKind::SendMail,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 10,
        attendee_known: true,
    });

    assert_eq!(decision, PolicyDecision::RequireApproval(RiskLevel::High));
}

#[test]
fn unknown_internal_recipient_is_medium_risk() {
    let recipients = vec!["newhire@contoso.com".to_string()];
    let decision = evaluate(RuleInput {
        kind: OperationKind::SendMail,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 10,
        attendee_known: true,
    });

    assert_eq!(decision, PolicyDecision::RequireApproval(RiskLevel::Medium));
}

#[test]
fn known_internal_recipient_is_low_risk_approval() {
    let recipients = vec!["james@contoso.com".to_string()];
    let decision = evaluate(RuleInput {
        kind: OperationKind::SendMail,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 10,
        attendee_known: true,
    });

    assert_eq!(decision, PolicyDecision::RequireApproval(RiskLevel::Low));
}

#[test]
fn unknown_attendee_requires_disambiguation() {
    let recipients = vec![];
    let decision = evaluate(RuleInput {
        kind: OperationKind::CreateEvent,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 10,
        attendee_known: false,
    });

    assert_eq!(
        decision,
        PolicyDecision::RequireDisambiguation("ATTENDEE_UNKNOWN")
    );
}

#[test]
fn create_event_outside_business_hours_is_medium_risk() {
    let recipients = vec![];
    let decision = evaluate(RuleInput {
        kind: OperationKind::CreateEvent,
        recipients: &recipients,
        known_recipients: &base_known(),
        allowlist_domains: &base_allowlist(),
        contains_sensitive: false,
        local_hour: 21,
        attendee_known: true,
    });

    assert_eq!(decision, PolicyDecision::RequireApproval(RiskLevel::Medium));
}

#[test]
fn business_hours_boundaries_are_valid() {
    assert!(is_business_hours(8));
    assert!(is_business_hours(18));
    assert!(!is_business_hours(7));
    assert!(!is_business_hours(19));
}
