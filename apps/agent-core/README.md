# agent-core (Rust)

Rust policy and orchestration core for OfficeClaw.

## Included

- Approval state machine (`policy/state_machine.rs`)
- Policy evaluation rules (`policy/rules.rs`)
- Orchestrator stub (`agent/orchestrator.rs`)
- Extended module boundaries for api/skills/storage/crypto/jobs
- Integration tests in `tests/`

## Run

```bash
cargo test
```

## Container build

```bash
docker build -f apps/agent-core/Dockerfile .
```
