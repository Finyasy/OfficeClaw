# System Design v2 (Rust-first Hybrid)

## Goal

Build a Teams-first agentic assistant where the TypeScript edge handles Teams/Bot Framework concerns and Rust owns orchestration, policy, approvals, skills, and audit.

## Official stack selection

- Teams Bot adapter in TypeScript (thin edge only).
- Rust agent core as the stable decision and execution layer.
- Azure OpenAI for LLM workloads.
- Microsoft Graph as the mail/calendar action layer.
- First-party RAG for templates, policies, and internal knowledge grounding.

## Architectural principles

- Contract-first: internal API boundaries are defined before runtime implementation.
- Approval-first: no outbound side effect without explicit approval.
- Audit-first: every external effect and policy decision is logged with correlation IDs.
- Replaceable edge: Teams adapter can evolve without changing core decision logic.

## External best-practice alignment

- OpenClaw pattern: gateway-centric architecture with session-aware core.
- NanoClaw pattern: isolation posture for risky operations.
- Google Workspace CLI pattern: typed, evolvable command surface with controlled scope.

## Context and boundaries

- Channel ingress: Microsoft Teams via Bot Framework activity model.
- Edge service: TypeScript adapter handles parsing, OAuth UX, proactive delivery, and card rendering.
- Core service: Rust gRPC server handles sessions, policy, tool routing, Graph calls, audit, and response shaping.
- External systems: Microsoft Graph, Azure OpenAI, Postgres, Azure observability stack.

## High-level flow

1. User message enters TS edge.
2. TS adapter normalizes to `ActivityEnvelope` and sends to Rust `SendActivity`.
3. Rust loads session and applies policy checks.
4. Rust calls Graph skills and optional LLM summarization.
5. Rust emits audit events.
6. Rust returns `AgentResponse` to TS for Teams rendering.

## Production defaults

- Single-region HA deployment baseline.
- Single-tenant-first model with future extension points.
- Retry and idempotency required for callback and webhook paths.

## Diagrams

- System context: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/source/01-system-context.mmd`
- Container view: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/source/02-rust-first-container-view.mmd`
- Core components: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/source/03-rust-core-components.mmd`
- Deployment: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/source/10-deployment-ha-azure.mmd`
