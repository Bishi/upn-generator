# UPN Preview And Send

> UPN page, UPN PDF generation, download, SMTP send, delivery history

## Pre-conditions

- Selected billing month has calculated splits.
- Apartment payer names and contact emails are configured.
- SMTP settings are configured under Settings -> Delivery -> Email when testing send actions.
- Email safety allowlist state is known before testing.

## Cases

- [ ] Select a billing period and confirm each apartment card shows line items and total due.
- [ ] Open a single apartment preview and confirm a PDF is generated without an app error.
- [ ] Confirm the preview opens through the system PDF viewer or reports a clear launch failure.
- [ ] Download all PDFs and confirm one combined PDF packet is created for each apartment.
- [ ] Confirm Download All PDFs does not mark the month delivered or create delivery history.
- [ ] Click Mark Delivered, cancel the confirmation prompt, and confirm the month remains undelivered.
- [ ] Click Mark Delivered, accept the confirmation prompt, and confirm all current apartment packets are marked delivered.
- [ ] Confirm the successful Mark Delivered message appears as a toast and does not push page content down.
- [ ] Trigger a UPN action error and confirm it appears as a manually dismissed error toast without pushing page content down.
- [ ] Trigger a workflow error, navigate away from UPN Preview and back, and confirm no duplicate error toast appears.
- [ ] Click Unmark Delivered, cancel the confirmation prompt, and confirm manual delivery marks remain.
- [ ] Click Unmark Delivered, accept the confirmation prompt, and confirm manual delivery marks are removed while email delivery history remains.
- [ ] Confirm a mixed month with some emailed packets and a manual delivered confirmation marks the UPN workflow step complete.
- [ ] Confirm generated UPN PDFs use the selected billing month, payer, recipient, IBAN, reference, purpose, and amount.
- [ ] Open single and apartment previews and confirm preview/open temp PDFs do not create delivery history.
- [ ] Navigate away from and back to UPN Preview after manual delivery and confirm delivered pills, row statuses, and Mark/Unmark button do not flash through an incorrect state.
- [ ] Send emails with allowlist enabled and confirm non-allowlisted recipients are blocked.
- [ ] Send a test email to an allowed recipient and confirm delivery succeeds.
- [ ] Send apartment emails with multiple comma-separated recipients and confirm per-recipient status is recorded.
- [ ] Reload the UPN page and confirm delivery history is restored for sent, failed, blocked, or partial rows.
- [ ] Change packet content after a prior send/manual confirmation and confirm the prior delivery is shown as no longer current until sent or manually marked delivered again.
- [ ] Confirm a blocked send attempt does not require an SMTP password just to record blocked delivery history.

## Notes

Use [upn-visual-reference.md](./upn-visual-reference.md) when the rendered UPN geometry or printed output changes.
