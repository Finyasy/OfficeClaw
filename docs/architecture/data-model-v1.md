# Data Model v1 (Documentation Contract)

## sessions

- `tenant_id`
- `user_id`
- `conversation_id`
- `state_json`
- `updated_at_utc`

Purpose: conversation context, last intents, last references, and user preferences.

## conversation_references

- `tenant_id`
- `user_id`
- `conversation_id`
- `service_url`
- `channel_id`
- `conversation_ref_json`
- `updated_at_utc`

Purpose: required storage for Teams proactive messaging after initial user interaction.

## approvals

- `approval_id`
- `tenant_id`
- `user_id`
- `approval_type`
- `draft_payload_json`
- `status` (`pending`, `approved`, `cancelled`, `expired`)
- `expires_at_utc`
- `updated_at_utc`

Purpose: explicit side-effect gating lifecycle.

## audit_events

- `event_id`
- `request_id`
- `correlation_id`
- `tenant_id`
- `actor_user_id`
- `action_type`
- `target_type`
- `target_id`
- `policy_code`
- `policy_result`
- `external_endpoint`
- `external_status`
- `request_summary`
- `result_summary`
- `created_at_utc`

Purpose: immutable trace of decisions and effects.

## graph_subscriptions

- `subscription_id`
- `tenant_id`
- `resource_path`
- `notification_url`
- `expiration_at_utc`
- `renewal_status`
- `updated_at_utc`

Purpose: track Graph change-notification subscriptions and renewal lifecycle.

## storage defaults

- PostgreSQL is the Phase 1 baseline target.
- Redis is optional and not required for Phase 1.
