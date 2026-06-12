# Backup And Restore

> Settings -> App -> Data, manual SQLite backup and restore

## Pre-conditions

- App has non-default data worth preserving: bills, splits, theme, delivery settings, and delivery history.
- A temporary folder is available for backup files.
- SMTP/IMAP credentials are saved if credential reuse is being tested.

## Cases

- [ ] Create a backup and confirm a `.sqlite3` file is written to the selected folder.
- [ ] Change visible app data after the backup, then restore the backup and confirm previous data returns.
- [ ] Confirm building, apartments, providers, billing periods, bills, splits, theme, inbox import history, and UPN delivery history restore as expected.
- [ ] Confirm SMTP and IMAP password columns are blanked in the backup and are not restored from SQLite.
- [ ] Restore a backup with the same SMTP/IMAP usernames and confirm matching Windows Credential Manager passwords can still be reused.
- [ ] Restore a backup with different SMTP/IMAP usernames and confirm the app requires entering new passwords.
- [ ] Confirm restore shows a clear success or failure message and does not leave the app half-updated after an error.

## Notes

Use a disposable backup file when validating failure cases.
