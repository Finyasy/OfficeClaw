use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::{Action, ActivityEnvelope, AgentResponse};
use crate::policy::rules::{evaluate, OperationKind, PolicyDecision, RuleInput};

static CORRELATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct Orchestrator {
    pub allowlist_domains: HashSet<String>,
    pub known_recipients: HashSet<String>,
}

impl Orchestrator {
    pub fn new(allowlist_domains: HashSet<String>, known_recipients: HashSet<String>) -> Self {
        Self {
            allowlist_domains,
            known_recipients,
        }
    }

    pub fn handle_activity(&self, activity: &ActivityEnvelope) -> AgentResponse {
        let correlation_id = format!("corr-{}", CORRELATION_COUNTER.fetch_add(1, Ordering::Relaxed));

        if let Some(action) = &activity.action {
            return self.handle_action(activity, action, correlation_id);
        }

        let text = activity.text.to_lowercase();

        if text.contains("summarize unread") {
            return AgentResponse {
                text: "Unread summary ready (simulated).".to_string(),
                actions: vec!["DRAFT_REPLIES".to_string(), "MARK_AS_READ".to_string()],
                correlation_id,
            };
        }

        if text.contains("schedule") {
            if !activity.attendee_known {
                return AgentResponse {
                    text: "Please confirm attendee email before scheduling.".to_string(),
                    actions: vec!["PROVIDE_ATTENDEE_EMAIL".to_string()],
                    correlation_id,
                };
            }

            return AgentResponse {
                text: "Proposed three available slots.".to_string(),
                actions: vec![
                    "SELECT_SLOT".to_string(),
                    "SELECT_SLOT_ALT_1".to_string(),
                    "SELECT_SLOT_ALT_2".to_string(),
                ],
                correlation_id,
            };
        }

        if text.contains("reply") {
            return AgentResponse {
                text: "Draft reply prepared. Review before sending.".to_string(),
                actions: vec![
                    "APPROVE_SEND".to_string(),
                    "EDIT_DRAFT".to_string(),
                    "CANCEL".to_string(),
                ],
                correlation_id,
            };
        }

        AgentResponse {
            text: "I can summarize unread emails, schedule meetings, or draft replies.".to_string(),
            actions: vec!["HELP".to_string()],
            correlation_id,
        }
    }

    fn handle_action(&self, activity: &ActivityEnvelope, action: &Action, correlation_id: String) -> AgentResponse {
        match action {
            Action::SelectSlot => AgentResponse {
                text: "Slot selected. Confirm invite to proceed.".to_string(),
                actions: vec!["APPROVE_INVITE".to_string(), "CANCEL".to_string()],
                correlation_id,
            },
            Action::ApproveInvite => {
                let decision = evaluate(RuleInput {
                    kind: OperationKind::CreateEvent,
                    recipients: &activity.recipients,
                    known_recipients: &self.known_recipients,
                    allowlist_domains: &self.allowlist_domains,
                    contains_sensitive: false,
                    local_hour: activity.request_hour_local,
                    attendee_known: activity.attendee_known,
                });

                match decision {
                    PolicyDecision::RequireDisambiguation(reason) => AgentResponse {
                        text: format!("Cannot create event yet: {}", reason),
                        actions: vec!["PROVIDE_ATTENDEE_EMAIL".to_string()],
                        correlation_id,
                    },
                    _ => AgentResponse {
                        text: "Booked. Invite sent (simulated).".to_string(),
                        actions: vec![],
                        correlation_id,
                    },
                }
            }
            Action::ApproveSend | Action::ConfirmExternalSend => {
                let decision = evaluate(RuleInput {
                    kind: OperationKind::SendMail,
                    recipients: &activity.recipients,
                    known_recipients: &self.known_recipients,
                    allowlist_domains: &self.allowlist_domains,
                    contains_sensitive: activity.contains_sensitive,
                    local_hour: activity.request_hour_local,
                    attendee_known: activity.attendee_known,
                });

                match decision {
                    PolicyDecision::Deny(reason) => AgentResponse {
                        text: format!("Send blocked: {}", reason),
                        actions: vec!["EDIT_DRAFT".to_string()],
                        correlation_id,
                    },
                    PolicyDecision::RequireApproval(_) => AgentResponse {
                        text: "Approval accepted. Email sent (simulated).".to_string(),
                        actions: vec![],
                        correlation_id,
                    },
                    _ => AgentResponse {
                        text: "Action not executable in current context.".to_string(),
                        actions: vec!["CANCEL".to_string()],
                        correlation_id,
                    },
                }
            }
            Action::WebhookNotification => AgentResponse {
                text: "Webhook processed. Proactive summary queued.".to_string(),
                actions: vec![],
                correlation_id,
            },
            Action::Cancel => AgentResponse {
                text: "Action cancelled.".to_string(),
                actions: vec![],
                correlation_id,
            },
            Action::Unknown(value) => AgentResponse {
                text: format!("Unknown action: {}", value),
                actions: vec!["HELP".to_string()],
                correlation_id,
            },
        }
    }
}
