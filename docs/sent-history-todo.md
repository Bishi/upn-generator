# Sent History TODO

The updated dashboard mock includes a complete workflow stage with copy such as
"emails sent", "PDF saved manually", and a closed monthly status. The current app
cannot render that faithfully because send/download activity is not persisted.

## Why This Is Deferred

- `WorkflowSnapshot.sent` is currently always `false`.
- `sendEmails` returns per-run results but does not store delivery history.
- `saveAllUpns` saves files but does not store which apartment packets were saved.
- The dashboard cannot distinguish "ready to send" from "already sent" after reload.

## Proposed Data Model

Add a persisted `upn_delivery_events` table:

- `id`
- `billing_period_id`
- `apartment_id`
- `delivery_type`: `email` or `pdf`
- `success`
- `recipient`
- `output_path`
- `error`
- `created_at`

Optionally add a monthly rollup view/helper that returns:

- packet count
- email sent count
- PDF saved count
- failed count
- last delivery timestamp
- whether the period is complete

## UI Work

- Update `WorkflowContextBar` to mark the UPN step done when all apartments have
  either a successful email delivery or a saved PDF event.
- Update Dashboard stage 3 to show complete-state actions:
  - `View history`
  - `Next month`
- Update UPN Preview to show persisted delivery status after reload.
- Add a history/detail panel for failed sends and manually saved PDFs.

## Backend Work

- Add migrations for the delivery-events table.
- Insert events from `send_emails`.
- Insert events from `save_all_upns` and apartment-level preview/save flows if
  those become user-facing save actions.
- Expose an IPC command for delivery rollups by billing period.
