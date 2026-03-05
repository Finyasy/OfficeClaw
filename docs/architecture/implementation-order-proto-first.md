# Implementation Order (Proto and Rust First)

## Phase 1 (this repository state)

- Documentation complete.
- Mermaid source and rendered diagrams complete.
- Placeholder repo structure complete.

## Phase 2 order (coding phase, deferred)

### Milestone A: core contract + Rust skeleton

1. Define `agent.proto` from `proto-contracts-v1.md` (`ActivityEnvelope`, `AgentResponse`, `ApprovalRequest`).
2. Scaffold Rust gRPC server with session store, policy/approval scaffolding, and audit writer.
3. Add dry-run skill stubs for mail and calendar actions.

### Milestone B: real Graph integration

1. Wire OAuth token handoff from TS adapter to Rust core and encrypted token handling.
2. Implement mail skill operations: list unread, fetch message, draft reply, send mail.
3. Implement calendar skill operations: calendar view, slot computation, event creation.
4. Add retries, idempotency, and deterministic error mapping.

### Milestone C: RAG + templates

1. Define knowledge packs for templates, policies, and FAQ/KB content.
2. Add ingestion pipeline plan: chunking, embedding, and indexing.
3. Integrate retrieval in Rust orchestration before LLM generation.

### Milestone D: event-driven automation

1. Implement Graph subscriptions and webhook flow.
2. Add proactive Teams messaging path using stored conversation references.
3. Add renewal jobs and alerting for subscription lifecycle failures.

### Milestone E (optional, only if needed): external planning worker

1. Add optional planning worker behind the existing proto/RAG boundary.
2. Restrict worker output to plan/draft proposals.
3. Keep Rust core as final policy gate and Graph execution authority.

## Why this order

Core contract and policy logic are long-lived and channel-agnostic, while Teams adapter behavior is channel-specific and easier to evolve later.
