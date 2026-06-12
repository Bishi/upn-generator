# Inbox Import

> Bills page Import from Inbox flow, read-only IMAP scanning, attachment preview

## Pre-conditions

- IMAP settings are configured under Settings -> Delivery -> Inbox.
- Test mailbox has messages with PDF or supported image attachments.
- Sender allowlist state is known before testing.
- A target billing month exists on the Bills page.

## Cases

- [ ] Test the inbox connection and confirm success/failure messages are clear.
- [ ] Run Import from Inbox and confirm the preview opens before any bill rows are created.
- [ ] Confirm scan-window overrides apply only to the current preview run unless saved in settings.
- [ ] Set the scan window override to 0 and confirm the drawer labels the scan as today-only.
- [ ] Set the scan window override to 1 and confirm the drawer labels the scan as today plus the previous calendar day.
- [ ] Confirm allowed sender messages appear when sender allowlist is enabled.
- [ ] Confirm disallowed sender messages are skipped or shown as blocked without import.
- [ ] Confirm unsupported attachment types are skipped.
- [ ] Confirm oversized messages or attachments are skipped with understandable feedback.
- [ ] Confirm attachments for the wrong billing month are blocked before import.
- [ ] Confirm unknown providers are blocked before import.
- [ ] Confirm providers already present for the selected month are not duplicated.
- [ ] Import selected ready candidates and confirm only selected bills are created.
- [ ] Confirm previewing does not mark email as read, move email, delete email, or persist raw extracted text.
- [ ] Confirm temporary attachment files are cleaned up after import.

## Notes

Inbox imports should use read-only IMAP behavior (`EXAMINE` / `BODY.PEEK`) and should not alter mailbox state.
