# Architecture Thoughts and Decisions

## Decision summary

For MVP and early production, keep a two-service runtime:

1. TypeScript Teams adapter (thin edge)
2. Rust agent core (single policy and execution brain)

Do not introduce a separate Agent Framework worker in this phase.

## Why this is the right choice now

- Fewer moving parts and lower operational risk.
- Governance is easier when all side effects are enforced in one Rust policy path.
- Faster delivery for the current feature set: summarize, schedule, draft/approve/send.
- Better portability for future channels because the core contract stays stable.

## Stack that remains official

- Teams Bot adapter in TypeScript
- Rust core for orchestration, policy, approvals, audit, Graph skills
- Azure OpenAI for LLM workloads
- Microsoft Graph for mail/calendar actions
- First-party RAG for templates, policy guidance, and internal knowledge

## What from the proposed skeleton is adopted

- Monorepo boundaries: `apps/teams-adapter`, `apps/agent-core`, `proto`, `db`, `infra`
- Contract-first gRPC boundary between adapter and core
- Production tables for sessions, approvals, audit, tokens, and conversation references
- Explicit approval state machine for send and calendar side effects
- Azure Container Apps + Postgres + Key Vault baseline

## What is deferred

- Separate Agent Framework worker or extra orchestration runtime
- Multi-agent planner/researcher/compliance pipelines
- Additional channel adapters beyond Teams

## When to revisit a separate worker

Re-open the decision only if one or more of these occur:

1. Multi-agent workflows become a core product requirement.
2. Experimentation speed on orchestration becomes the main bottleneck.
3. The team needs Python/.NET ecosystem capabilities that would be expensive to replicate in Rust.

## Guardrail if a worker is added later

- Worker can propose plans/drafts only.
- Rust core remains final policy authority and Graph side-effect executor.
- Approval and audit cannot be bypassed by worker output.

## Decision status

- Status: accepted
- Scope: MVP and early production
- Revisit trigger: after usage data shows orchestration complexity that warrants extra runtime components
