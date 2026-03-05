# Workflow: Unread Summary Sequence

## User intent

"Summarize unread emails from today"

## Expected behavior

- Retrieve unread headers from inbox.
- Summarize safely with no outbound sending.
- Return summary and follow-up action buttons.

## Policy and audit requirements

- Policy check runs before Graph read.
- Audit event is written for Graph read and response generation.

## Diagram references

- Source: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/source/04-seq-unread-summary-v2.mmd`
- Render: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/rendered/04-seq-unread-summary-v2.png`

## Fallback path

If Graph returns transient errors, respond with retry message and no side effects.
