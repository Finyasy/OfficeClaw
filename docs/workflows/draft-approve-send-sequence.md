# Workflow: Draft, Approve, Send Sequence

## User intent

"Reply to this email proposing Thu 10am"

## Expected behavior

- Retrieve message context.
- Create draft reply.
- Show draft preview with `Send`, `Edit`, `Cancel`.
- Send only after explicit `ApproveSend` callback.

## Policy and audit requirements

- Policy denies direct send without approval action.
- External/new recipients require extra confirmation.
- Audit events log draft generation, approval decision, and send outcome.

## Diagram references

- Source: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/source/06-seq-draft-approve-send-v2.mmd`
- Render: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/rendered/06-seq-draft-approve-send-v2.png`

## Safety branch

If approval expires or recipient policy fails, send is blocked and a corrective prompt is returned.
