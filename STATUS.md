# Status

- Current version: `0.5.1`
- Current tag: `v0.5.1`
- Release status: prepared on `feat/tweaks` for PR review

## Latest Included Changes

- Consolidated Bills, Splits, and UPN preview table chrome for more consistent row, header, footer, and empty-state styling
- Moved the Splits total column to the end and simplified the Splits header labels/dividers
- Added today-only inbox scan support with `0` as a valid scan window, while preserving calendar-date IMAP `SINCE` semantics for positive values
- Documented and added manual QA coverage for inbox scan-window calendar-date behavior
- Stored SMTP and IMAP passwords in Windows Credential Manager instead of SQLite
- Kept mail password fields write-only while showing saved-password state
- Cleared legacy database password columns after successful credential writes
- Preserved password-free SQLite backups and username-matched credential reuse after restore
- Fixed blocked email safety attempts so they can be recorded without requiring an SMTP password
- Surfaced factory-reset warnings when saved Windows mail credentials cannot be deleted
