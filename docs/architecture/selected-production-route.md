# Selected Production Route (Official)

## Chosen option

Option 1 is the official path for OfficeClaw:

- Teams Bot TypeScript adapter (thin edge)
- Rust agent core (primary runtime)
- Azure OpenAI (LLM)
- Microsoft Graph API (mail/calendar action layer)
- First-party RAG layer (templates, policy guidance, KB)

## Hybrid clarification

This project uses a practical hybrid of TS + Rust:

- TS exists only as a Teams/Bot edge adapter.
- Rust remains the only core decision and side-effect authority.
- No additional Agent Framework worker is included in MVP.

## Why this is the best fit

- Product-grade control over approvals, audit trails, and policy rules.
- Portability: channel adapters can change without replacing the core brain.
- Multi-channel readiness: Teams-first now, additional channels later.
- Clear security boundaries: side effects controlled in Rust policy engine.

## Non-negotiable platform dependency

Microsoft Graph is the required action layer for real Outlook mail/calendar control.

## Role boundaries

### TypeScript Teams adapter

- Teams/Bot Framework activity handling
- OAuth sign-in UX and callback surface
- Adaptive card rendering and callback translation
- Proactive messaging delivery using stored conversation references
- Forward normalized envelopes to Rust core

### Rust agent core

- Orchestration and tool-calling
- Policy and approval gates
- Graph skill execution
- Session and memory handling
- Audit logging and safety checks

### RAG layer

- Stores templates, KB content, scheduling policies, and compliance guidance
- Retrieval returns grounded passages to the Rust core
- Policy engine validates generated actions before side effects

## Optional later extension: Copilot connectors / MCP

Use MCP/connectors as additional retrieval providers behind the same RAG interface, not as a replacement for the product core.

Target interface:

- `retrieve(query) -> passages`
- `retrieve_from_mcp(query) -> passages`
- merged and ranked before LLM usage

This preserves OfficeClaw control over approval flows, auditing, and multi-channel UX.

## North-star portability model

- Interfaces: channel adapters (Teams now, others later)
- Brain: Rust core remains stable
- Actions: Graph layer remains stable
- Context: RAG providers are pluggable
