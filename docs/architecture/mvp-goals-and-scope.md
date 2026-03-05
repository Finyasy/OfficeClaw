# MVP Goals and Scope (Two Weeks)

## MVP outcomes

1. "Summarize unread emails" returns concise summary with safe follow-up actions.
2. "Schedule 30 mins with James next week" proposes slots and books only after confirmation.
3. "Draft reply proposing Thu 10am" drafts first and sends only after explicit approval.

## Hard safety rule

No autonomous sending of mail or invites without explicit user approval.

## Channel and platform scope

- Primary interface: Microsoft Teams (DM first).
- Calendar and mail operations via Microsoft Graph.
- Proactive messages allowed for confirmations and webhook-triggered summaries.

## Non-goals in Phase 1

- Runtime implementation of Rust/TS services.
- Proto code generation.
- Persistence migrations.
- Additional channels beyond Teams.
