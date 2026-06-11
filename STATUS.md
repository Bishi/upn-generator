# Status

- Current version: `0.5.0`
- Current tag: `v0.5.0`
- Release status: tagged and pushed to `origin/main`

## Latest Included Changes

- Stored SMTP and IMAP passwords in Windows Credential Manager instead of SQLite
- Kept mail password fields write-only while showing saved-password state
- Cleared legacy database password columns after successful credential writes
- Preserved password-free SQLite backups and username-matched credential reuse after restore
- Fixed blocked email safety attempts so they can be recorded without requiring an SMTP password
- Surfaced factory-reset warnings when saved Windows mail credentials cannot be deleted
