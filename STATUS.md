# Status

- Current version: `0.6.0`
- Current tag: `v0.6.0`
- Release status: prepared on `codex/month-close-reopen`

## Latest Included Changes

- Added persisted month close/reopen status with closure timestamps and consistent workflow status across Bills, Splits, UPN, and the month picker
- Required complete current-packet delivery and backend validation before closing a month
- Kept closed months viewable and PDF-exportable while blocking bill, import, split, delivery, period-deletion, and relevant apartment-deletion mutations until reopened
- Prevented month closing while email delivery batches are active
- Preserved close state through current and legacy backup restores
- Consolidated Bills, Splits, and UPN preview table chrome for more consistent row, header, footer, and empty-state styling
- Stopped tracking local `docs/` notes and `file-examples/` bill samples; both folders are now ignored for local reference use
- Moved the Splits total column to the end and simplified the Splits header labels/dividers
- Added custom horizontal table scrollbars so wide tables preserve rounded table chrome
- Added today-only inbox scan support with `0` as a valid scan window, while preserving calendar-date IMAP `SINCE` semantics for positive values
- Documented and added manual QA coverage for inbox scan-window calendar-date behavior
- Stored SMTP and IMAP passwords in Windows Credential Manager instead of SQLite
- Kept mail password fields write-only while showing saved-password state
- Cleared legacy database password columns after successful credential writes
- Preserved password-free SQLite backups and username-matched credential reuse after restore
- Fixed backup restore failures caused by foreign-key delete order and improved restore error display in the Data Backup card
- Moved mail and inbox network commands off the UI thread and added SMTP timeout/timing diagnostics
- Fixed blocked email safety attempts so they can be recorded without requiring an SMTP password
- Added current-packet delivery rollups with manual Mark/Unmark Delivered confirmation while keeping PDF downloads as export-only actions
- Added backend-owned UPN pre-send validation gates for blocking incorrect sends/delivery state
- Added settings dirty-form discard confirmations and guarded settings tab switching
- Restyled the UPN validation issue panel with grouped details and a calmer collapsed summary
- Surfaced factory-reset warnings when saved Windows mail credentials cannot be deleted
