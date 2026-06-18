# Backup And Restore

> Settings -> App -> Data, manual SQLite backup and restore

## Pre-conditions

- App has non-default data worth preserving: bills, splits, theme, delivery settings, and delivery history.
- A temporary folder is available for backup files.
- SMTP/IMAP credentials are saved if credential reuse is being tested.

## Cases

- [ ] Create a backup and confirm a `.sqlite3` file is written to the selected folder.
- [ ] While backup creation is running, confirm a loading overlay appears and page clicks are blocked until it finishes.
- [ ] Confirm the Backup Created dialog appears after the loading overlay is gone, not on top of it.
- [ ] Change visible app data after the backup, then restore the backup and confirm previous data returns.
- [ ] While backup restore is running, confirm a loading overlay appears and page clicks are blocked until restore completes or fails.
- [ ] Confirm the Restore Complete dialog appears after the loading overlay is gone, not on top of it.
- [ ] After restore reloads the app, confirm Settings stays on the App tab instead of jumping to Building.
- [ ] Confirm building, apartments, providers, billing periods, bills, splits, theme, inbox import history, and UPN delivery history restore as expected.
- [ ] Confirm SMTP and IMAP password columns are blanked in the backup and are not restored from SQLite.
- [ ] Restore a backup with the same SMTP/IMAP usernames and confirm matching Windows Credential Manager passwords can still be reused.
- [ ] Restore a backup with different SMTP/IMAP usernames and confirm the app requires entering new passwords.
- [ ] Confirm restore shows a clear success or failure message and does not leave the app half-updated after an error.

## Notes

Use a disposable backup file when validating failure cases.
