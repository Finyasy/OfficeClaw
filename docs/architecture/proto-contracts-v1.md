# Proto Contracts v1

## Contract policy

- Service namespace: `teamsagent.v1`
- Backward compatibility: additive fields only.
- Removed fields: field numbers are reserved and never reused.
- Breaking changes require `v2` package.

## gRPC methods

- `HandleActivity(ActivityEnvelope) -> AgentResponse`
- `OAuthCallback(AuthEnvelope) -> Ack`
- `SendProactive(ProactiveMessage) -> Ack`

## Message contracts

### ActivityEnvelope

- `actor`: `tenant_id`, `user_id`, `user_display_name`.
- `conversation`: `channel`, `conversation_id`, `thread_id`, `message_id`.
- `text`: user utterance or action context.
- `action`: optional card callback action, e.g. `SELECT_SLOT`, `APPROVE_SEND`.
- `action_payload_json`: structured callback payload, including `approval_id` and selected slot data.
- `recipients`, `attendee_known`, `attendee_email`, `contains_sensitive`, `request_hour_local`.
- `conversation_ref_json`: Teams conversation reference captured by the adapter and persisted by Rust for proactive delivery.
- `attachments[]`: structured attachment metadata from the adapter.

### AgentResponse

- `correlation_id`.
- `text`: primary natural-language reply.
- `adaptive_card_json`: optional card payload.
- `actions[]`: available next actions.

### AuthEnvelope

- `actor`, `provider`, `access_token`.
- Optional `refresh_token`, `expires_at_utc`, `scope`.
- Rust stores this bundle in Postgres using envelope encryption.

### ProactiveMessage

- `actor`, `conversation`, `text`.
- Optional `adaptive_card_json`.
- `correlation_id`.

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

- `action=SELECT_SLOT` creates a persisted invite approval.
- `action=APPROVE_SEND` and `action_payload_json.approval_id` execute persisted mail approvals.
- `conversation_ref_json` is captured on inbound Teams activities and later used by `SendProactive`.
