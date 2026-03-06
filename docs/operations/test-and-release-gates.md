# Test and Release Gates

## Documentation-level test scenarios

1. Unread summary success and Graph transient failure fallback.
2. Meeting scheduling with attendee ambiguity branch.
3. Draft/approve/send with mandatory approval gate.
4. External recipient extra-confirmation branch.
5. Webhook validation token and renewal handling.
6. Duplicate callback idempotency behavior.
7. Audit completeness for each side effect.

## Runtime integration gates

- TypeScript adapter test coverage includes a local end-to-end path for adapter -> gRPC -> Rust -> proactive Bot Connector delivery.
- Rust integration coverage includes Postgres-backed repo tests with mocked Graph and mocked Key Vault endpoints.
- Token persistence tests must exercise envelope encryption and decryption through the Key Vault client path, not only the plaintext fallback.
- Approval execution tests must prove that persisted approval payloads are used as the execution source of truth.
- CI runs the Rust persistence integration test against a Postgres service container on every pull request.

## Release gates (Phase 1)

- Every workflow doc references one source `.mmd` and one rendered `.png`.
- Side effect branches in diagrams include policy + audit steps.
- Contract fields used in sequence labels match contract doc.
- Existing legacy files in `pics/*.png` remain unchanged.
- No `.rs`, `.ts`, proto codegen, migrations, or executable runtime artifacts added.
- Azure decision matrix is present and locked in deployment docs.
- MVP checklist includes required Azure resources and env vars.

## Exit criteria for moving to Phase 2

- Architecture docs approved.
- Contract doc approved.
- Workflow and operations docs approved.
- Diagram set v2 rendered and cross-linked.
- Deployment checklist approved by infra/security owners.
