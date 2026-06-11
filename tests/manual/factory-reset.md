# Factory Reset

> Settings -> App -> Data, factory reset, reseeded defaults

## Pre-conditions

- App has disposable data because factory reset clears periods, bills, splits, and delivery history.
- Saved SMTP/IMAP credential state is known before testing.

## Cases

- [ ] Start factory reset and confirm the warning clearly describes destructive data loss.
- [ ] Cancel factory reset and confirm data remains unchanged.
- [ ] Confirm factory reset and verify building, apartments, providers, SMTP defaults, and inbox defaults are reseeded.
- [ ] Confirm billing periods, bills, splits, and UPN delivery history are cleared.
- [ ] Confirm selected theme returns to the expected default.
- [ ] Confirm saved Windows mail credentials are deleted or a clear warning is shown if deletion fails.
- [ ] Restart the app and confirm the reset state persists.

## Notes

Do not run this checklist against valuable local data without a backup.

