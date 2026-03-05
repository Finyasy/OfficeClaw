# TypeScript Edge Design v1

## Purpose

Keep Teams-specific logic out of Rust core by using a thin adapter.

## Responsibilities

- Receive Teams/Bot Framework activities.
- Normalize activities into `ActivityEnvelope`.
- Present OAuth sign-in UX (cards and callback handling).
- Render `AgentResponse` text and adaptive cards.
- Translate card button callbacks into action envelopes.
- Send proactive messages when requested by Rust core.
- Persist Teams conversation references after first user interaction.
- Handle Graph webhook ingress and forward normalized notifications to Rust core.

## Design constraints

- No business-policy decisions in TS edge.
- No direct Graph mail/calendar execution in TS edge except auth UX flows.
- Keep channel-specific transformation logic isolated.

## Adapter-to-core contract

- Calls `SendActivity` for inbound user text and callback actions.
- Calls `OAuthCallback` for authentication completion metadata.
- Receives proactive notifications via `ProactiveNotify` requests from core.
- Uses `SendActivity` with `action_type=WebhookNotification` for Graph webhook events.

## Reliability behavior

- Preserve and forward `correlation_id` and `request_id`.
- Retry transient gRPC failures with bounded backoff.
- Avoid duplicate callback delivery using idempotency keys.

## Deferred from this phase

- Concrete Bot Framework SDK implementation.
- Runtime token store implementation.
