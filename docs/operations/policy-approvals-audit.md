# Policy, Approvals, and Audit

## Policy rules (MVP)

1. No outbound email send without explicit user approval action.
2. No meeting invite side effect without explicit confirmation.
3. External or new recipient requires extra confirmation.
4. Scheduling outside business hours requires explicit override phrase.
5. Unknown attendee identity requires disambiguation before slot proposal.

## Approval lifecycle

- `pending`: created from side-effect intent.
- `approved`: explicit user approval callback accepted.
- `executed`: Graph send or event creation completed successfully.
- `failed`: policy re-check or Graph execution failed after approval processing began.
- `cancelled`: user cancelled or policy invalidated.
- `expired`: timeout reached before approval.

## Audit requirements

Every policy decision and external side effect writes an immutable event with:

- actor, tenant, correlation ID
- action type and target
- policy result and reason code
- endpoint called and status
- approval ID and approval status transitions when applicable
- request/result summaries

## Enforcement guarantees

- Side effects are gated by both policy and approval checks.
- Approval execution rehydrates the stored approval payload instead of trusting callback data alone.
- Duplicate callbacks are handled idempotently by `request_id` and `approval_id`.
