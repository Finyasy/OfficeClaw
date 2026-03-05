# OfficeClaw

Rust-first, production-grade agent platform for Microsoft Teams that automates Outlook mail and calendar via Microsoft Graph.

## Monorepo skeleton

- `proto/agent.proto`: gRPC contract shared by adapter and core.
- `apps/teams-adapter`: TypeScript Teams/Bot edge service.
- `apps/agent-core`: Rust policy/orchestration core.
- `db/migrations`: Postgres schema and indexes.
- `infra/aca/bicep`: Azure Container Apps deployment skeleton.
- `docs/`: architecture and operational documentation.

## What is implemented in this skeleton

- Contract-first message shapes for activity handling and proactive messaging.
- Rust policy engine with approval-state machine and guardrail rules.
- Rust orchestrator stub with action routing paths.
- TypeScript activity normalization and conversation-reference storage.
- Local development compose stack for Postgres + both services.

## Test coverage included

### Rust (`apps/agent-core/tests`)

- Approval state transitions and invalid-transition handling.
- Policy engine edge cases:
  - malformed recipients
  - missing recipients
  - sensitive content blocking
  - external domain high-risk gating
  - unknown recipient medium-risk gating
  - attendee disambiguation for scheduling
  - business-hours boundaries
- Orchestrator edge cases:
  - summarize path
  - schedule path with unknown attendee
  - draft/approve/send blocked and success paths
  - unknown actions and fallback response
  - webhook action handling

### TypeScript (`apps/teams-adapter/tests`)

- Config validation (missing env and invalid port).
- Conversation reference upsert/read/overwrite behavior.
- Activity normalization edge cases:
  - missing tenant/user/conversation identifiers
  - malformed recipients
  - invalid request hour fallback
  - action parsing behavior

## Run tests

- Rust: `cd apps/agent-core && cargo test`
- TypeScript: `cd apps/teams-adapter && npm install && npm test`
