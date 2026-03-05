# OfficeClaw Phase 1 Documentation

This folder is the source of truth for the docs-first implementation phase.

## Scope

- Create production-grade system design and workflow specifications.
- Lock interface contracts before runtime implementation.
- Provide Mermaid sources and rendered diagrams in `pics/v2`.

## Out of scope

- Rust runtime code.
- TypeScript runtime code.
- Proto codegen and migrations.

## Reading order

1. `architecture/mvp-goals-and-scope.md`
2. `architecture/selected-production-route.md`
3. `architecture/architecture-thoughts-and-decisions.md`
4. `architecture/system-design-v2.md`
5. `architecture/inspiration-mapping.md`
6. `architecture/proto-contracts-v1.md`
7. `architecture/data-model-v1.md`
8. `architecture/rust-core-design-v1.md`
9. `architecture/ts-edge-design-v1.md`
10. `architecture/implementation-order-proto-first.md`
11. `workflows/*.md`
12. `operations/*.md`

Recommended first operations docs:

1. `operations/azure-deployment-single-region-ha.md`
2. `operations/azure-mvp-deployment-checklist.md`

## Diagram index

- Mermaid source: `pics/v2/source`
- PNG renders: `pics/v2/rendered`
- Existing top-level `pics/*.png` are retained as legacy artifacts.

## Thread decisions captured in docs

- Rust-first hybrid with minimal TypeScript edge.
- Production sequencing: proto and Rust skeleton first, TS adapter second.
- Approval-gated outbound actions and audit-first policy enforcement.
- Azure single-region HA baseline for deployment architecture.
- Azure production choices are locked with selected defaults (Container Apps, App Gateway WAF, Postgres Flexible Server HA, Key Vault, Service Bus, App Insights/Monitor).
- Official architecture choice is Option 1: TS Teams adapter + Rust core + Azure OpenAI + Graph + first-party RAG.
- Copilot connectors/MCP are documented as optional later retrieval providers behind the same RAG boundary.
