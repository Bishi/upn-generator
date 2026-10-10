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
- [ ] Start Fetch preview or Re-scan and choose Cancel scan; confirm the spinner stops, another scan can be started, and no late result replaces the new preview or creates bills.
- [ ] Click Fetch preview and confirm the button and scan introduction stay in place when the helper text changes to Cancel scan; cancel and confirm switching back causes no vertical jump.
- [ ] During Fetch preview, close using X, Close, the backdrop, or Escape; reopen immediately and confirm the cancelled scan cannot populate the reopened drawer. Navigate away during a scan and confirm its pending result is discarded.
- [ ] Cancel while a message is being downloaded and while PDF OCR is running; confirm mailbox state is unchanged, no preview session survives cancellation, and temporary attachments are removed after the current extraction releases them.
- [ ] Cancel during PDF OCR and immediately Fetch preview again, including after closing/reopening or navigating back to Bills; confirm the new scan waits for the OCR slot and produces the normal verified invoices rather than unreadable identity results or missing image-only invoices. Cancel the waiting retry and confirm it stops without disturbing the previous worker's cleanup.
- [ ] On a desktop window, confirm the drawer is 900px wide and all five preview columns, including Wrong period, fit for the utility sample emails without horizontal scrolling. On smaller windows, confirm the drawer stays within the viewport and overflow remains scrollable when needed.
- [ ] Select a closed billing month and confirm inbox import controls on Bills are disabled.
- [ ] If an inbox preview session exists before closing the month, close the month and confirm importing selected preview candidates is blocked until the month is reopened.
- [ ] Confirm scan-window overrides apply only to the current preview run unless saved in settings.
- [ ] Set the scan window override to 0 and confirm the drawer labels the scan as today-only.
- [ ] Set the scan window override to 1 and confirm the drawer labels the scan as today plus the previous calendar day.
- [ ] Confirm allowed sender messages appear when sender allowlist is enabled.
- [ ] Confirm disallowed sender messages are skipped or shown as blocked without import.
- [ ] Confirm unsupported attachment types are skipped.
- [ ] Scan five separate forwarded utility emails for the same service month (electricity, gas, water, waste, cleaning); confirm each PDF yields one candidate in the following billing month with correct amount, reference, due date, and matched building identity.
- [ ] Confirm PDF attachments labeled `application/octet-stream` are previewable/importable, while files with an incorrect signature or a conflicting specific MIME type remain rejected.
- [ ] Confirm tiny inline CID logos/signature JPGs do not appear as invoice candidates; ordinary image attachments and larger inline invoice scans remain available.
- [ ] Confirm attachments with no invoice payment fields show an empty result rather than Unknown period. Rejection reasons are collapsed behind Why? in the Bill column; expand/collapse them with mouse and keyboard and confirm long explanations wrap within a bounded width without widening the table. Full reasons also remain available in the Status popover.
- [ ] Confirm `od 01.09.2026 do 30.09.2026`, two-digit-year ranges, and VOKA `09/2026` service-purpose text resolve to September service / October billing; payment due dates alone must not establish a service month.
- [ ] Preview an invoice whose service range ends immediately before a numeric field on the next line (for example `01.09.2026-30.09.2026` followed by `15.10.2026`); confirm September service / October billing is detected, including OCR-spaced dates.
- [ ] Confirm small-print gas identity evidence is verified and adjacent dates/numeric fields do not extend its identifier; a different or longer account identifier must still fail verification.
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
