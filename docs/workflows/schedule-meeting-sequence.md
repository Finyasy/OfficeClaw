# Workflow: Schedule Meeting Sequence

## User intent

"Schedule 30 mins with James next week"

## Expected behavior

- Resolve attendee identity.
- Retrieve calendar view and propose three slots.
- Await explicit user slot selection.
- Create event and return confirmation.

## Policy and audit requirements

- Policy checks business hours and attendee domain rules.
- Approval state is explicit for invite side effects.
- Audit event is written for slot proposal and event creation.

## Diagram references

- Source: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/source/05-seq-schedule-meeting-v2.mmd`
- Render: `/Users/bryan.bosire/anaconda_projects/OfficeClaw/pics/v2/rendered/05-seq-schedule-meeting-v2.png`

## Ambiguity branch

If attendee resolution is ambiguous, assistant asks for explicit email confirmation before slot generation.
