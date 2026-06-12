# UPN Preview And Send

> UPN page, UPN PDF generation, download, SMTP send, delivery history

## Pre-conditions

- Selected billing month has calculated splits.
- Apartment payer names and contact emails are configured.
- SMTP settings are configured under Settings -> Delivery -> Email when testing send actions.
- Email safety allowlist state is known before testing.

## Cases

- [ ] Select a billing period and confirm each apartment card shows line items and total due.
- [ ] Confirm the validation panel appears for the selected billing period and summarizes blocking errors and warnings without pushing content down unexpectedly.
- [ ] Remove an apartment recipient for an apartment with a UPN packet and confirm Send Emails is disabled while Download All PDFs and Mark Delivered remain available.
- [ ] Enter an invalid recipient email and confirm Send Emails is disabled with a clear validation issue.
- [ ] With email safety allowlist enabled, use a non-allowlisted recipient and confirm Send Emails is blocked before SMTP password lookup or delivery history creation.
- [ ] Remove or invalidate a bill IBAN, reference, due date, purpose code, or purpose text and confirm Send Emails, Download All PDFs, and Mark Delivered are disabled.
- [ ] Create a duplicate provider bill or split total mismatch and confirm Send Emails, Download All PDFs, and Mark Delivered are disabled until the issue is fixed.
- [ ] Open a single apartment preview and confirm a PDF is generated without an app error.
- [ ] Confirm single-apartment previews remain available when validation only blocks delivery actions.
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
- [ ] Send emails with allowlist enabled and confirm all non-allowlisted recipients are blocked by validation.
- [ ] Send a test email to an allowed recipient and confirm delivery succeeds.
- [ ] Send apartment emails with multiple comma-separated recipients and confirm per-recipient status is recorded.
- [ ] Reload the UPN page and confirm delivery history is restored for sent, failed, blocked, or partial rows.
- [ ] Change packet content after a prior send/manual confirmation and confirm the prior delivery is shown as no longer current until sent or manually marked delivered again.

## Notes

Use [upn-visual-reference.md](./upn-visual-reference.md) when the rendered UPN geometry or printed output changes.
