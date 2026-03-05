# Security Threat Model

## Assets

- User mail/calendar data via Graph delegated scopes.
- Access and refresh tokens.
- Approval state and audit records.
- Session context and conversation metadata.

## Threats and mitigations

### Spoofed callback actions

- Mitigation: verify adapter signatures and auth claims.
- Mitigation: require valid `approval_id`, `user_id`, and expiration checks.

### Replay of prior approval callbacks

- Mitigation: idempotency keys and single-use approval transitions.

### Privilege overreach

- Mitigation: least-privilege Graph scopes and policy checks per action.

### Data exfiltration via tool misuse

- Mitigation: strict skill allowlist and output filtering.

### Audit tampering

- Mitigation: append-only audit store and restricted write path.

## Security defaults

- Draft-only fallback when auth or policy state is uncertain.
- Explicit deny with deterministic policy code for blocked actions.
