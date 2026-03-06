use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::proactive::ProactiveNotifier;
use crate::domain::{
    Action, ActivityEnvelope, Actor, AgentResponse, ApprovalKind, ApprovalRecord, AuditEventRecord,
    ConversationRefRecord, CreateEventApprovalPayload, MailApprovalPayload, OAuthTokenBundle,
    ProactiveDeliveryRequest, ResponseAction, SessionKey,
};
use crate::policy::rules::{evaluate, OperationKind, PolicyDecision, RiskLevel, RuleInput};
use crate::policy::state_machine::{transition, ApprovalEvent, ApprovalStatus};
use crate::skills::graph::calendar::{
    CalendarEventCreator, CalendarReadError, CalendarReader, CalendarWriteError,
};
use crate::skills::graph::mail::{
    summarize_unread_messages, MailReadError, MailReader, MailSendError, MailSender,
};
use crate::storage::approvals_repo::ApprovalsRepo;
use crate::storage::audit_repo::AuditRepo;
use crate::storage::conversation_refs_repo::ConversationRefsRepo;
use crate::storage::sessions_repo::{SessionsRepo, StorageError};
use crate::storage::tokens_repo::TokensRepo;

static CORRELATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct Orchestrator {
    pub allowlist_domains: HashSet<String>,
    pub known_recipients: HashSet<String>,
    sessions_repo: Arc<dyn SessionsRepo>,
    audit_repo: Arc<dyn AuditRepo>,
    approvals_repo: Arc<dyn ApprovalsRepo>,
    conversation_refs_repo: Arc<dyn ConversationRefsRepo>,
    tokens_repo: Arc<dyn TokensRepo>,
    mail_reader: Arc<dyn MailReader>,
    mail_sender: Arc<dyn MailSender>,
    calendar_reader: Arc<dyn CalendarReader>,
    calendar_event_creator: Arc<dyn CalendarEventCreator>,
    proactive_notifier: Arc<dyn ProactiveNotifier>,
}

impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        allowlist_domains: HashSet<String>,
        known_recipients: HashSet<String>,
        sessions_repo: Arc<dyn SessionsRepo>,
        audit_repo: Arc<dyn AuditRepo>,
        approvals_repo: Arc<dyn ApprovalsRepo>,
        conversation_refs_repo: Arc<dyn ConversationRefsRepo>,
        tokens_repo: Arc<dyn TokensRepo>,
        mail_reader: Arc<dyn MailReader>,
        mail_sender: Arc<dyn MailSender>,
        calendar_reader: Arc<dyn CalendarReader>,
        calendar_event_creator: Arc<dyn CalendarEventCreator>,
        proactive_notifier: Arc<dyn ProactiveNotifier>,
    ) -> Self {
        Self {
            allowlist_domains,
            known_recipients,
            sessions_repo,
            audit_repo,
            approvals_repo,
            conversation_refs_repo,
            tokens_repo,
            mail_reader,
            mail_sender,
            calendar_reader,
            calendar_event_creator,
            proactive_notifier,
        }
    }

    pub async fn handle_activity(&self, activity: &ActivityEnvelope) -> AgentResponse {
        let correlation_id = next_correlation_id();
        self.persist_conversation_ref(activity).await;

        if let Some(action) = &activity.action {
            return self.handle_action(activity, action, correlation_id).await;
        }

        let text = activity.text.to_lowercase();

        if text.contains("summarize unread") {
            return self.handle_unread_summary(activity, correlation_id).await;
        }

        if text.contains("schedule") {
            return self.handle_schedule_request(activity, correlation_id).await;
        }

        if text.contains("reply") {
            return self.handle_reply_request(activity, correlation_id).await;
        }

        AgentResponse {
            text: "I can summarize unread emails, schedule meetings, or draft replies.".to_string(),
            actions: vec![ResponseAction::simple("HELP")],
            correlation_id,
        }
    }

    pub async fn handle_oauth_callback(
        &self,
        actor: &Actor,
        token: &OAuthTokenBundle,
    ) -> Result<(), StorageError> {
        self.tokens_repo.store_graph_token(actor, token).await?;
        let _ = self
            .audit_repo
            .append(AuditEventRecord {
                event_id: Uuid::new_v4(),
                tenant_id: actor.tenant_id.clone(),
                user_id: actor.user_id.clone(),
                channel: "teams".to_string(),
                conversation_id: "oauth-callback".to_string(),
                correlation_id: next_correlation_id(),
                event_type: "GRAPH_TOKEN_STORED".to_string(),
                event_json: json!({
                    "provider": "graph",
                    "scope": token.scope.clone(),
                    "expires_at_utc": token.expires_at_utc.clone()
                }),
            })
            .await;

        Ok(())
    }

    pub async fn send_proactive(
        &self,
        actor: &Actor,
        channel: &str,
        conversation_id: &str,
        text: &str,
        correlation_id: String,
    ) -> Result<(), StorageError> {
        let Some(conversation_ref) = self
            .conversation_refs_repo
            .load(&actor.tenant_id, &actor.user_id, channel, conversation_id)
            .await?
        else {
            return Err(StorageError {
                message: "Conversation reference unavailable for proactive delivery".to_string(),
            });
        };

        self.proactive_notifier
            .send(&ProactiveDeliveryRequest {
                actor: actor.clone(),
                conversation: crate::domain::Conversation {
                    channel: channel.to_string(),
                    conversation_id: conversation_id.to_string(),
                    message_id: String::new(),
                },
                conversation_ref_json: conversation_ref.ref_json.to_string(),
                text: text.to_string(),
                adaptive_card_json: None,
                correlation_id,
            })
            .await
            .map_err(|error| StorageError {
                message: error.message,
            })
    }

    async fn persist_conversation_ref(&self, activity: &ActivityEnvelope) {
        if let Some(record) = ConversationRefRecord::from_activity(activity) {
            let _ = self.conversation_refs_repo.upsert(&record).await;
        }
    }

    async fn handle_action(
        &self,
        activity: &ActivityEnvelope,
        action: &Action,
        correlation_id: String,
    ) -> AgentResponse {
        match action {
            Action::SelectSlot => self.handle_slot_selection(activity, correlation_id).await,
            Action::ApproveInvite => self.handle_invite_approval(activity, correlation_id).await,
            Action::ApproveSend | Action::ConfirmExternalSend => {
                self.handle_send_approval(activity, correlation_id).await
            }
            Action::EditDraft => AgentResponse {
                text: "Draft remains pending. Edit the draft before approving again.".to_string(),
                actions: vec![ResponseAction::simple("HELP")],
                correlation_id,
            },
            Action::WebhookNotification => {
                let proactive_result = self
                    .send_proactive(
                        &activity.actor,
                        &activity.conversation.channel,
                        &activity.conversation.conversation_id,
                        "Webhook processed. Proactive summary queued.",
                        correlation_id.clone(),
                    )
                    .await;

                let event_type = if proactive_result.is_ok() {
                    "PROACTIVE_DELIVERY_SUCCEEDED"
                } else {
                    "PROACTIVE_DELIVERY_FAILED"
                };

                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        event_type,
                        json!({
                            "result": proactive_result.as_ref().err().map(|error| error.message.clone())
                        }),
                    ))
                    .await;

                AgentResponse {
                    text: if proactive_result.is_ok() {
                        "Webhook processed. Proactive summary queued.".to_string()
                    } else {
                        "Webhook processed, but proactive delivery could not be queued."
                            .to_string()
                    },
                    actions: vec![],
                    correlation_id,
                }
            }
            Action::Cancel => self.handle_cancel(activity, correlation_id).await,
            Action::Unknown(value) => AgentResponse {
                text: format!("Unknown action: {}", value),
                actions: vec![ResponseAction::simple("HELP")],
                correlation_id,
            },
        }
    }

    async fn handle_unread_summary(
        &self,
        activity: &ActivityEnvelope,
        correlation_id: String,
    ) -> AgentResponse {
        let session_key = SessionKey::from_activity(activity);
        let mut state = self
            .sessions_repo
            .load(&session_key)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        state.last_intent = Some("summarize_unread".to_string());

        let policy_decision = self.evaluate_read_only_policy(activity);
        if policy_decision != PolicyDecision::Allow {
            let _ = self
                .audit_repo
                .append(AuditEventRecord::from_activity(
                    activity,
                    &correlation_id,
                    "MAIL_SUMMARY_POLICY_DENIED",
                    json!({
                        "operation": "read_only",
                        "policy_result": format!("{:?}", policy_decision)
                    }),
                ))
                .await;

            return AgentResponse {
                text: "Unread summary request is blocked by policy.".to_string(),
                actions: vec![ResponseAction::simple("HELP")],
                correlation_id,
            };
        }

        let _ = self
            .audit_repo
            .append(AuditEventRecord::from_activity(
                activity,
                &correlation_id,
                "MAIL_SUMMARY_POLICY_EVALUATED",
                json!({
                    "operation": "read_only",
                    "policy_result": "ALLOW",
                    "graph_endpoint": "/me/mailFolders/inbox/messages"
                }),
            ))
            .await;

        match self.mail_reader.list_unread_today(&activity.actor).await {
            Ok(messages) => {
                state.unread_summary_count = Some(messages.len());
                let _ = self.sessions_repo.upsert(&session_key, &state).await;
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "MAIL_SUMMARY_SUCCEEDED",
                        json!({ "message_count": messages.len() }),
                    ))
                    .await;

                let actions = if messages.is_empty() {
                    vec![ResponseAction::simple("REFRESH_SUMMARY")]
                } else {
                    vec![
                        ResponseAction::simple("DRAFT_REPLIES"),
                        ResponseAction::simple("MARK_AS_READ"),
                    ]
                };

                AgentResponse {
                    text: summarize_unread_messages(&messages),
                    actions,
                    correlation_id,
                }
            }
            Err(MailReadError::Retryable(reason)) => {
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "MAIL_SUMMARY_RETRYABLE_FAILURE",
                        json!({ "reason": reason }),
                    ))
                    .await;

                AgentResponse {
                    text: "Could not fetch unread emails right now. Please retry.".to_string(),
                    actions: vec![ResponseAction::simple("RETRY_SUMMARY")],
                    correlation_id,
                }
            }
            Err(MailReadError::Permanent(reason)) => {
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "MAIL_SUMMARY_PERMANENT_FAILURE",
                        json!({ "reason": reason }),
                    ))
                    .await;

                AgentResponse {
                    text: "Unread summary is unavailable until Graph access is configured."
                        .to_string(),
                    actions: vec![ResponseAction::simple("HELP")],
                    correlation_id,
                }
            }
        }
    }

    async fn handle_schedule_request(
        &self,
        activity: &ActivityEnvelope,
        correlation_id: String,
    ) -> AgentResponse {
        if !activity.attendee_known {
            return AgentResponse {
                text: "Please confirm attendee email before scheduling.".to_string(),
                actions: vec![ResponseAction::simple("PROVIDE_ATTENDEE_EMAIL")],
                correlation_id,
            };
        }

        let _ = self
            .audit_repo
            .append(AuditEventRecord::from_activity(
                activity,
                &correlation_id,
                "SLOT_PROPOSAL_REQUESTED",
                json!({
                    "duration_minutes": 30,
                    "graph_endpoint": "/me/calendarView"
                }),
            ))
            .await;

        match self
            .calendar_reader
            .propose_slots_next_week(&activity.actor, 30)
            .await
        {
            Ok(slots) => {
                let session_key = SessionKey::from_activity(activity);
                let mut state = self
                    .sessions_repo
                    .load(&session_key)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                state.last_intent = Some("schedule_meeting".to_string());
                state.proposed_slots = slots.clone();
                let _ = self.sessions_repo.upsert(&session_key, &state).await;
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "SLOT_PROPOSAL_SUCCEEDED",
                        json!({ "slot_count": slots.len() }),
                    ))
                    .await;

                let response_text = if slots.is_empty() {
                    "No suitable slots were found next week.".to_string()
                } else {
                    format!(
                        "Proposed three available slots.\n{}",
                        slots
                            .iter()
                            .enumerate()
                            .map(|(index, slot)| format!(
                                "{}. {} to {}",
                                index + 1,
                                slot.start_utc,
                                slot.end_utc
                            ))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                };

                let actions = slots
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        ResponseAction::new(
                            "SELECT_SLOT",
                            format!("Select slot {}", index + 1),
                            json!({ "slot_index": index }).to_string(),
                            if index == 0 { Some("primary") } else { None },
                        )
                    })
                    .collect::<Vec<_>>();

                AgentResponse {
                    text: response_text,
                    actions,
                    correlation_id,
                }
            }
            Err(CalendarReadError::Retryable(reason)) => {
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "SLOT_PROPOSAL_RETRYABLE_FAILURE",
                        json!({ "reason": reason }),
                    ))
                    .await;

                AgentResponse {
                    text: "Could not read the calendar right now. Please retry.".to_string(),
                    actions: vec![ResponseAction::simple("RETRY_SCHEDULE")],
                    correlation_id,
                }
            }
            Err(CalendarReadError::Permanent(reason)) => {
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "SLOT_PROPOSAL_PERMANENT_FAILURE",
                        json!({ "reason": reason }),
                    ))
                    .await;

                AgentResponse {
                    text: "Scheduling is unavailable until Graph calendar access is configured."
                        .to_string(),
                    actions: vec![ResponseAction::simple("HELP")],
                    correlation_id,
                }
            }
        }
    }

    async fn handle_reply_request(
        &self,
        activity: &ActivityEnvelope,
        correlation_id: String,
    ) -> AgentResponse {
        let decision = evaluate(RuleInput {
            kind: OperationKind::SendMail,
            recipients: &activity.recipients,
            known_recipients: &self.known_recipients,
            allowlist_domains: &self.allowlist_domains,
            contains_sensitive: activity.contains_sensitive,
            local_hour: activity.request_hour_local,
            attendee_known: activity.attendee_known,
        });
        let action_id = if matches!(
            decision,
            PolicyDecision::RequireApproval(RiskLevel::High)
        ) {
            "CONFIRM_EXTERNAL_SEND"
        } else {
            "APPROVE_SEND"
        };
        let approval = self
            .approvals_repo
            .create(
                activity,
                ApprovalKind::SendMail,
                risk_level_string(&decision),
                serde_json::to_value(MailApprovalPayload {
                    recipients: activity.recipients.clone(),
                    draft_text: "Draft reply prepared. Review before sending.".to_string(),
                    contains_sensitive: activity.contains_sensitive,
                })
                .unwrap_or_else(|_| json!({})),
                json!({
                    "policy": "EXPLICIT_APPROVAL_REQUIRED"
                }),
            )
            .await;

        match approval {
            Ok(record) => {
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "APPROVAL_CREATED",
                        json!({
                            "approval_id": record.approval_id,
                            "kind": record.kind
                        }),
                    ))
                    .await;

                AgentResponse {
                    text: "Draft reply prepared. Review before sending.".to_string(),
                    actions: approval_actions(action_id, &record, true),
                    correlation_id,
                }
            }
            Err(error) => AgentResponse {
                text: format!("Draft reply could not be prepared: {}", error.message),
                actions: vec![ResponseAction::simple("HELP")],
                correlation_id,
            },
        }
    }

    async fn handle_slot_selection(
        &self,
        activity: &ActivityEnvelope,
        correlation_id: String,
    ) -> AgentResponse {
        let state = self
            .sessions_repo
            .load(&SessionKey::from_activity(activity))
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let slot_index = slot_index_from_payload(activity).unwrap_or(0);
        let Some(selected_slot) = state.proposed_slots.get(slot_index).cloned() else {
            return AgentResponse {
                text: "No slot is available to confirm. Request new slot proposals.".to_string(),
                actions: vec![ResponseAction::simple("HELP")],
                correlation_id,
            };
        };

        let response_text = format!(
            "Slot selected: {} to {}. Confirm invite to proceed.",
            selected_slot.start_utc, selected_slot.end_utc
        );
        let approval = self
            .approvals_repo
            .create(
                activity,
                ApprovalKind::CreateEvent,
                risk_level_string(&PolicyDecision::RequireApproval(RiskLevel::Low)),
                serde_json::to_value(CreateEventApprovalPayload {
                    slot_index,
                    start_utc: selected_slot.start_utc.clone(),
                    end_utc: selected_slot.end_utc.clone(),
                    attendee_email: activity
                        .attendee_email
                        .clone()
                        .or_else(|| activity.recipients.first().cloned()),
                    request_hour_local: activity.request_hour_local,
                })
                .unwrap_or_else(|_| json!({})),
                json!({
                    "policy": "EXPLICIT_INVITE_CONFIRMATION_REQUIRED"
                }),
            )
            .await;

        match approval {
            Ok(record) => {
                let _ = self
                    .audit_repo
                    .append(AuditEventRecord::from_activity(
                        activity,
                        &correlation_id,
                        "APPROVAL_CREATED",
                        json!({
                            "approval_id": record.approval_id,
                            "kind": record.kind
                        }),
                    ))
                    .await;

                AgentResponse {
                    text: response_text,
                    actions: approval_actions("APPROVE_INVITE", &record, false),
                    correlation_id,
                }
            }
            Err(error) => AgentResponse {
                text: format!("Slot confirmation could not be created: {}", error.message),
                actions: vec![ResponseAction::simple("HELP")],
                correlation_id,
            },
        }
    }

    async fn handle_invite_approval(
        &self,
        activity: &ActivityEnvelope,
        correlation_id: String,
    ) -> AgentResponse {
        let approval = match self.load_pending_approval(activity).await {
            Ok(record) => record,
            Err(response) => return response.with_correlation_id(correlation_id),
        };
        let payload = match serde_json::from_value::<CreateEventApprovalPayload>(
            approval.payload_json.clone(),
        ) {
            Ok(value) => value,
            Err(_) => {
                return AgentResponse::error_with_correlation(
                    "Approval payload is invalid for invite execution.",
                    correlation_id,
                );
            }
        };

        let recipients = payload
            .attendee_email
            .clone()
            .into_iter()
            .collect::<Vec<_>>();
        let decision = evaluate(RuleInput {
            kind: OperationKind::CreateEvent,
            recipients: &recipients,
            known_recipients: &self.known_recipients,
            allowlist_domains: &self.allowlist_domains,
            contains_sensitive: false,
            local_hour: payload.request_hour_local,
            attendee_known: payload.attendee_email.is_some(),
        });

        match decision {
            PolicyDecision::RequireDisambiguation(reason) => {
                self.fail_approval(
                    activity,
                    approval.approval_id,
                    &correlation_id,
                    "APPROVAL_EXECUTION_BLOCKED",
                    &format!("Cannot create event yet: {}", reason),
                    vec![ResponseAction::simple("PROVIDE_ATTENDEE_EMAIL")],
                )
                .await
            }
            _ => self
                .execute_event_approval(activity, approval, payload, correlation_id)
                .await,
        }
    }

    async fn handle_send_approval(
        &self,
        activity: &ActivityEnvelope,
        correlation_id: String,
    ) -> AgentResponse {
        let approval = match self.load_pending_approval(activity).await {
            Ok(record) => record,
            Err(response) => return response.with_correlation_id(correlation_id),
        };
        let payload = match serde_json::from_value::<MailApprovalPayload>(approval.payload_json.clone()) {
            Ok(value) => value,
            Err(_) => {
                return AgentResponse::error_with_correlation(
                    "Approval payload is invalid for send execution.",
                    correlation_id,
                );
            }
        };

        let decision = evaluate(RuleInput {
            kind: OperationKind::SendMail,
            recipients: &payload.recipients,
            known_recipients: &self.known_recipients,
            allowlist_domains: &self.allowlist_domains,
            contains_sensitive: payload.contains_sensitive,
            local_hour: activity.request_hour_local,
            attendee_known: activity.attendee_known,
        });

        match decision {
            PolicyDecision::Deny(reason) => {
                self.fail_approval(
                    activity,
                    approval.approval_id,
                    &correlation_id,
                    "APPROVAL_EXECUTION_BLOCKED",
                    &format!("Send blocked: {}", reason),
                    vec![ResponseAction::simple("EDIT_DRAFT")],
                )
                .await
            }
            PolicyDecision::RequireApproval(_) => {
                self.execute_send_approval(activity, approval, payload, correlation_id)
                    .await
            }
            _ => AgentResponse {
                text: "Action not executable in current context.".to_string(),
                actions: vec![ResponseAction::simple("CANCEL")],
                correlation_id,
            },
        }
    }

    async fn handle_cancel(&self, activity: &ActivityEnvelope, correlation_id: String) -> AgentResponse {
        if let Some(approval_id) = approval_id_from_payload(activity) {
            let _ = self
                .approvals_repo
                .update_status(approval_id, ApprovalStatus::Cancelled)
                .await;
        }

        AgentResponse {
            text: "Action cancelled.".to_string(),
            actions: vec![],
            correlation_id,
        }
    }

    async fn load_pending_approval(
        &self,
        activity: &ActivityEnvelope,
    ) -> Result<ApprovalRecord, AgentResponse> {
        let Some(approval_id) = approval_id_from_payload(activity) else {
            return Err(AgentResponse::error("Approval context is missing."));
        };

        let approval = self
            .approvals_repo
            .load(approval_id)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| AgentResponse::error("Approval was not found or has expired."))?;

        if approval.status_enum() != Some(ApprovalStatus::Pending) {
            return Err(AgentResponse::error("Approval is no longer pending."));
        }

        Ok(approval)
    }

    async fn execute_send_approval(
        &self,
        activity: &ActivityEnvelope,
        approval: ApprovalRecord,
        payload: MailApprovalPayload,
        correlation_id: String,
    ) -> AgentResponse {
        let Ok(approved_status) = transition(ApprovalStatus::Pending, ApprovalEvent::Approve) else {
            return AgentResponse::error_with_correlation(
                "Approval transition failed.",
                correlation_id,
            );
        };

        let _ = self
            .approvals_repo
            .update_status(approval.approval_id, approved_status)
            .await;
        let _ = self
            .audit_repo
            .append(AuditEventRecord::from_activity(
                activity,
                &correlation_id,
                "APPROVAL_APPROVED",
                json!({ "approval_id": approval.approval_id }),
            ))
            .await;
        match self.mail_sender.send_draft(&activity.actor, &payload).await {
            Ok(()) => {
                self.mark_approval_executed(
                    activity,
                    approval.approval_id,
                    &correlation_id,
                    json!({
                        "approval_id": approval.approval_id,
                        "kind": "SEND_MAIL"
                    }),
                )
                .await;

                AgentResponse {
                    text: "Approval accepted. Email sent.".to_string(),
                    actions: vec![],
                    correlation_id,
                }
            }
            Err(error) => self
                .fail_approval(
                    activity,
                    approval.approval_id,
                    &correlation_id,
                    "APPROVAL_EXECUTION_FAILED",
                    &mail_send_error_message(&error),
                    vec![ResponseAction::simple("RETRY_SEND")],
                )
                .await,
        }
    }

    async fn execute_event_approval(
        &self,
        activity: &ActivityEnvelope,
        approval: ApprovalRecord,
        payload: CreateEventApprovalPayload,
        correlation_id: String,
    ) -> AgentResponse {
        let Ok(approved_status) = transition(ApprovalStatus::Pending, ApprovalEvent::Approve) else {
            return AgentResponse::error_with_correlation(
                "Approval transition failed.",
                correlation_id,
            );
        };

        let _ = self
            .approvals_repo
            .update_status(approval.approval_id, approved_status)
            .await;
        let _ = self
            .audit_repo
            .append(AuditEventRecord::from_activity(
                activity,
                &correlation_id,
                "APPROVAL_APPROVED",
                json!({ "approval_id": approval.approval_id }),
            ))
            .await;

        match self
            .calendar_event_creator
            .create_event(&activity.actor, &payload)
            .await
        {
            Ok(event_id) => {
                self.mark_approval_executed(
                    activity,
                    approval.approval_id,
                    &correlation_id,
                    json!({
                        "approval_id": approval.approval_id,
                        "kind": "CREATE_EVENT",
                        "event_id": event_id
                    }),
                )
                .await;

                AgentResponse {
                    text: "Booked. Invite sent.".to_string(),
                    actions: vec![],
                    correlation_id,
                }
            }
            Err(error) => self
                .fail_approval(
                    activity,
                    approval.approval_id,
                    &correlation_id,
                    "APPROVAL_EXECUTION_FAILED",
                    &calendar_write_error_message(&error),
                    vec![ResponseAction::simple("RETRY_SCHEDULE")],
                )
                .await,
        }
    }

    async fn mark_approval_executed(
        &self,
        activity: &ActivityEnvelope,
        approval_id: Uuid,
        correlation_id: &str,
        event_json: Value,
    ) {
        let Ok(executed_status) =
            transition(ApprovalStatus::Approved, ApprovalEvent::ExecuteSuccess)
        else {
            return;
        };
        let _ = self
            .approvals_repo
            .update_status(approval_id, executed_status)
            .await;
        let _ = self
            .audit_repo
            .append(AuditEventRecord::from_activity(
                activity,
                correlation_id,
                "APPROVAL_EXECUTED",
                event_json,
            ))
            .await;
    }

    async fn fail_approval(
        &self,
        activity: &ActivityEnvelope,
        approval_id: Uuid,
        correlation_id: &str,
        event_type: &str,
        user_message: &str,
        actions: Vec<ResponseAction>,
    ) -> AgentResponse {
        let _ = self
            .approvals_repo
            .update_status(approval_id, ApprovalStatus::Failed)
            .await;
        let _ = self
            .audit_repo
            .append(AuditEventRecord::from_activity(
                activity,
                correlation_id,
                event_type,
                json!({
                    "approval_id": approval_id,
                    "message": user_message
                }),
            ))
            .await;

        AgentResponse {
            text: user_message.to_string(),
            actions,
            correlation_id: correlation_id.to_string(),
        }
    }

    fn evaluate_read_only_policy(&self, activity: &ActivityEnvelope) -> PolicyDecision {
        evaluate(RuleInput {
            kind: OperationKind::ReadOnly,
            recipients: &activity.recipients,
            known_recipients: &self.known_recipients,
            allowlist_domains: &self.allowlist_domains,
            contains_sensitive: activity.contains_sensitive,
            local_hour: activity.request_hour_local,
            attendee_known: activity.attendee_known,
        })
    }
}

fn approval_actions(base_action: &str, approval: &ApprovalRecord, include_edit: bool) -> Vec<ResponseAction> {
    let payload = json!({ "approval_id": approval.approval_id }).to_string();
    let mut actions = vec![ResponseAction::new(
        base_action,
        base_action,
        payload.clone(),
        Some("primary"),
    )];
    if include_edit {
        actions.push(ResponseAction::new("EDIT_DRAFT", "EDIT_DRAFT", payload.clone(), None));
    }
    actions.push(ResponseAction::new("CANCEL", "CANCEL", payload, None));
    actions
}

fn approval_id_from_payload(activity: &ActivityEnvelope) -> Option<Uuid> {
    payload_value(activity)
        .and_then(|value| value.get("approval_id").and_then(|inner| inner.as_str().map(str::to_string)))
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn slot_index_from_payload(activity: &ActivityEnvelope) -> Option<usize> {
    payload_value(activity)
        .and_then(|value| value.get("slot_index").and_then(|inner| inner.as_u64()))
        .map(|value| value as usize)
}

fn payload_value(activity: &ActivityEnvelope) -> Option<Value> {
    activity
        .action_payload_json
        .as_ref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
}

fn risk_level_string(decision: &PolicyDecision) -> &'static str {
    match decision {
        PolicyDecision::RequireApproval(RiskLevel::Low) | PolicyDecision::Allow => "LOW",
        PolicyDecision::RequireApproval(RiskLevel::Medium) => "MEDIUM",
        PolicyDecision::RequireApproval(RiskLevel::High) => "HIGH",
        PolicyDecision::Deny(_) => "BLOCKED",
        PolicyDecision::RequireDisambiguation(_) => "DISAMBIGUATION",
    }
}

fn mail_send_error_message(error: &MailSendError) -> String {
    match error {
        MailSendError::Retryable(reason) => {
            format!("Send could not complete right now: {}. Retry later.", reason)
        }
        MailSendError::Permanent(reason) => {
            format!("Send could not complete: {}.", reason)
        }
    }
}

fn calendar_write_error_message(error: &CalendarWriteError) -> String {
    match error {
        CalendarWriteError::Retryable(reason) => {
            format!("Invite creation could not complete right now: {}. Retry later.", reason)
        }
        CalendarWriteError::Permanent(reason) => {
            format!("Invite creation could not complete: {}.", reason)
        }
    }
}

fn next_correlation_id() -> String {
    format!(
        "corr-{}",
        CORRELATION_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

trait ErrorResponse {
    fn error(message: &str) -> Self;
    fn error_with_correlation(message: &str, correlation_id: String) -> Self;
    fn with_correlation_id(self, correlation_id: String) -> Self;
}

impl ErrorResponse for AgentResponse {
    fn error(message: &str) -> Self {
        Self {
            text: message.to_string(),
            actions: vec![ResponseAction::simple("HELP")],
            correlation_id: String::new(),
        }
    }

    fn error_with_correlation(message: &str, correlation_id: String) -> Self {
        Self {
            text: message.to_string(),
            actions: vec![ResponseAction::simple("HELP")],
            correlation_id,
        }
    }

    fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = correlation_id;
        self
    }
}
