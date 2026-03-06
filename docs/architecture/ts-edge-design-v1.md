# TypeScript Edge Design v1

## Purpose

Keep Teams-specific logic out of Rust core by using a thin adapter.

## Responsibilities

- Receive Teams/Bot Framework activities.
- Normalize activities into `ActivityEnvelope`.
- Present OAuth sign-in UX (cards and callback handling).
- Render `AgentResponse` text and adaptive cards.
- Translate card button callbacks into action envelopes.
- Send proactive messages when requested by Rust core by calling the Bot Connector REST API with bot credentials.
- Capture Teams conversation references after first user interaction and forward them to Rust for persistence.
- Handle Graph webhook ingress and forward normalized notifications to Rust core.

## Design constraints

- No business-policy decisions in TS edge.
- No direct Graph mail/calendar execution in TS edge except auth UX flows.
- Keep channel-specific transformation logic isolated.
- Rust remains the system of record for tokens, approvals, audit events, and conversation references.

## Adapter-to-core contract

- Calls `HandleActivity` for inbound user text and callback actions.
- Calls `OAuthCallback` for authentication completion metadata.
- Receives proactive notifications from Rust over the adapter's `/api/proactive` HTTP endpoint.
- Uses `HandleActivity` with `action=WEBHOOK_NOTIFICATION` for Graph webhook events.
- Uses Bot Framework client-credentials auth to obtain a Connector token before proactive sends.

## Reliability behavior

- Preserve and forward `correlation_id` and `request_id`.
- Retry transient gRPC failures with bounded backoff.
- Avoid duplicate callback delivery using idempotency keys.

## Deferred from this phase

- Concrete Bot Framework SDK implementation.
- Runtime token store implementation.
