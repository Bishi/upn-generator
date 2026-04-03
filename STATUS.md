# Status

- Current version: `0.4.12`
- Current tag: `v0.4.12`
- Release status: ready to be tagged and pushed to `origin`

## Latest Included Changes

- Added a manual `Data` settings tab for SQLite backup and restore
- Backups now save as user-chosen `.sqlite3` snapshots of the app database
- Backup files intentionally exclude the saved SMTP password
- Restore now replaces the live app data from a selected backup and prompts the user to re-enter SMTP credentials
- Documented the backup workflow in `README.md` and refreshed the project handbook in `CLAUDE.md`
