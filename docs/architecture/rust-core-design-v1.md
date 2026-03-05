# Rust Core Design v1

## Responsibilities

- Expose `AgentCoreV1` gRPC service.
- Own session loading and persistence.
- Execute orchestrator loop (plan, check policy, act, verify).
- Execute skills for Graph mail/calendar operations.
- Enforce approval and policy gates.
- Emit immutable audit records.

## Core modules

- `api`: gRPC handlers and request validation.
- `agent`: orchestration and intent routing.
- `policy`: rule evaluation and approval gating.
- `skills`: Graph-backed mail and calendar tools.
- `storage`: sessions, approvals, audit persistence adapters.
- `audit`: event schema and append-only writer.

## Orchestrator flow

1. Validate envelope and auth context.
2. Load session and conversation memory.
3. Derive intent and required skill(s).
4. Run policy pre-check.
5. Execute read operations and draft generation.
6. If side effect intent exists, produce `ApprovalRequest` path.
7. On approval action, re-check policy and execute side effect.
8. Emit audit event(s) and respond.

## Non-functional requirements

- Idempotency on duplicate `request_id`.
- Bounded retries with exponential backoff for Graph transient failures.
- Deterministic policy reason codes.
- Correlation ID propagation to logs and metrics.

## Deferred from this phase

- Concrete crate layout and runtime implementation.
- Database migrations and repository code.
