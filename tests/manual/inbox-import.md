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
- [ ] Select a closed billing month and confirm inbox import controls on Bills are disabled.
- [ ] If an inbox preview session exists before closing the month, close the month and confirm importing selected preview candidates is blocked until the month is reopened.
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
- [ ] Preview at least two attachments, import only one, then view and import the remaining attachment without rescanning; confirm its selections and exception notes remain available.
- [ ] Confirm previewing does not mark email as read, move email, delete email, or persist raw extracted text.
- [ ] Import one attachment from a multi-attachment preview, choose Continue reviewing, and confirm the results and viewer close without a mailbox rescan; confirm the imported attachment is gone while unselected staged attachments remain viewable and importable, then close the drawer and confirm their temporary files are cleaned up.
- [ ] Confirm every parsed candidate shows its building-identity result and source-page range, including failed/unconfigured candidates instead of silently hiding them.
- [ ] Confirm matched invoices show a compact verified status, with explanation and expected/observed values available through Evidence; failed identity details remain visible without expanding anything.
- [ ] Select a failed identity candidate and confirm a required exception note is enforced before final import.
- [ ] Select distinct same-provider/month candidates and confirm the entire selection rolls back with a conflict; exact duplicates remain deduplicated.
- [ ] Create a preview, then change the provider identity rule or import another bill for that provider/month, and confirm finalization rechecks current state and saves nothing on failure.
- [ ] Make OCR recognize an additional invoice only during finalization; confirm the entire selected batch is rejected with a refresh-preview error and no newly recognized invoice is saved.

## Notes

Inbox imports should use read-only IMAP behavior (`EXAMINE` / `BODY.PEEK`) and should not alter mailbox state.
