# Proto Contracts v1

## Contract policy

- Service namespace: `officeclaw.agent.v1`
- Backward compatibility: additive fields only.
- Removed fields: field numbers are reserved and never reused.
- Breaking changes require `v2` package.

## gRPC methods

- `SendActivity(ActivityEnvelope) -> AgentResponse`
- `OAuthCallback(AuthEnvelope) -> Ack`
- `ProactiveNotify(NotifyEnvelope) -> Ack`

## Message contracts

### ActivityEnvelope

- `request_id`: unique per incoming adapter request.
- `correlation_id`: end-to-end trace identifier.
- `channel`: expected `teams` in MVP.
- `tenant_id`, `user_id`, `conversation_id`, `message_id`.
- `message_text`: user utterance or action context.
- `action_type`: optional card callback action, e.g. `SelectSlot`, `ApproveSend`.
- `email_id`, `draft_id`, `slot_start_utc`, `slot_end_utc` for workflow callbacks.
- `attachments_json`: structured card/action payload from adapter.

### AgentResponse

- `request_id`, `correlation_id`.
- `response_text`: primary natural-language reply.
- `adaptive_card_json`: optional card payload.
- `actions[]`: available next actions.
- `requires_approval`: indicates side effects blocked pending approval.
- `side_effect_intent`: descriptive intent, never direct execution signal.

### ApprovalRequest

- `approval_id`, `correlation_id`, `user_id`.
- `approval_type`: `send_mail` or `calendar_invite`.
- `risk_level`: low, medium, high.
- `draft_payload_json`.
- `expires_at_utc`.

### ApprovalDecision

- `approval_id`, `decision`: approve, cancel, edit.
- `approver_user_id`.
- `decision_reason`.
- `decided_at_utc`.

### PolicyDecision

- `policy_code`: deterministic reason code.
- `result`: allow, deny, require_approval, require_disambiguation.
- `explanation`: user-safe explanation.

### AuditEvent

- `event_id`, `correlation_id`, `request_id`.
- `actor_user_id`, `tenant_id`.
- `action_type`, `target_type`, `target_id`.
- `policy_code`, `policy_result`.
- `external_endpoint`, `external_status`.
- `request_summary`, `result_summary`.
- `created_at_utc`.

## Error model

- `AUTH_REQUIRED`
- `POLICY_DENIED`
- `RETRYABLE_UPSTREAM`
- `NON_RETRYABLE_UPSTREAM`
- `INVALID_ACTION_PAYLOAD`

## Sequence alignment

The fields above are used directly in workflow diagrams:

- `action_type=SelectSlot` for meeting confirmation.
- `action_type=ApproveSend` and `draft_id` for mail send approval.
- `email_id` for reply context retrieval.
