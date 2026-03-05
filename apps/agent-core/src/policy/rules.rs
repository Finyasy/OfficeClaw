use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    ReadOnly,
    SendMail,
    CreateEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    RequireApproval(RiskLevel),
    Deny(&'static str),
    RequireDisambiguation(&'static str),
}

#[derive(Debug, Clone)]
pub struct RuleInput<'a> {
    pub kind: OperationKind,
    pub recipients: &'a [String],
    pub known_recipients: &'a HashSet<String>,
    pub allowlist_domains: &'a HashSet<String>,
    pub contains_sensitive: bool,
    pub local_hour: u8,
    pub attendee_known: bool,
}

pub fn is_business_hours(hour: u8) -> bool {
    (8..=18).contains(&hour)
}

fn parse_domain(recipient: &str) -> Option<&str> {
    let mut parts = recipient.split('@');
    let _name = parts.next()?;
    let domain = parts.next()?;
    if parts.next().is_some() || domain.is_empty() {
        return None;
    }
    Some(domain)
}

pub fn evaluate(input: RuleInput<'_>) -> PolicyDecision {
    use OperationKind::*;

    match input.kind {
        ReadOnly => PolicyDecision::Allow,
        SendMail => evaluate_send_mail(input),
        CreateEvent => evaluate_create_event(input),
    }
}

fn evaluate_send_mail(input: RuleInput<'_>) -> PolicyDecision {
    if input.recipients.is_empty() {
        return PolicyDecision::Deny("NO_RECIPIENTS");
    }

    if input.contains_sensitive {
        return PolicyDecision::Deny("SENSITIVE_CONTENT_REQUIRES_EDIT");
    }

    let mut external_found = false;
    let mut unknown_found = false;

    for recipient in input.recipients {
        let domain = match parse_domain(recipient) {
            Some(value) => value,
            None => return PolicyDecision::Deny("MALFORMED_RECIPIENT"),
        };

        if !input.allowlist_domains.contains(domain) {
            external_found = true;
        }

        if !input.known_recipients.contains(recipient) {
            unknown_found = true;
        }
    }

    if external_found {
        return PolicyDecision::RequireApproval(RiskLevel::High);
    }

    if unknown_found {
        return PolicyDecision::RequireApproval(RiskLevel::Medium);
    }

    PolicyDecision::RequireApproval(RiskLevel::Low)
}

fn evaluate_create_event(input: RuleInput<'_>) -> PolicyDecision {
    if !input.attendee_known {
        return PolicyDecision::RequireDisambiguation("ATTENDEE_UNKNOWN");
    }

    if !is_business_hours(input.local_hour) {
        return PolicyDecision::RequireApproval(RiskLevel::Medium);
    }

    PolicyDecision::RequireApproval(RiskLevel::Low)
}
