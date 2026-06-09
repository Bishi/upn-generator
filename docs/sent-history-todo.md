# Remaining Delivery History TODO

The updated dashboard mock includes a complete workflow stage with copy such as
"emails sent", "PDF saved manually", and a closed monthly status.

Email sending activity is now persisted in `upn_delivery_events`, including
sent, failed, and allowlist-blocked recipient outcomes. The remaining gap is
PDF-save history and dashboard/workflow completion rollups.

## Why This Is Deferred

- `WorkflowSnapshot.sent` is currently still always `false`.
- `saveAllUpns` saves files but does not store which apartment packets were saved.
- The dashboard does not yet use persisted email delivery history.

## Current Data Model

The app has a persisted `upn_delivery_events` table:

- `id`
- `billing_period_id`
- `apartment_id`
- `delivery_type`: `email` or `pdf`
- `status`: `sent`, `failed`, or `blocked`
- `recipient`
- `original_recipient`
- `attachment_sha256`
- `error`
- `created_at`

Remaining work should add PDF events and a monthly rollup helper that returns:

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
- UPN Preview already shows persisted email delivery status after reload.
- Add a history/detail panel for failed sends and manually saved PDFs.

## Backend Work

- Insert events from `save_all_upns` and apartment-level preview/save flows if
  those become user-facing save actions.
- Expose an IPC command for delivery rollups by billing period.
