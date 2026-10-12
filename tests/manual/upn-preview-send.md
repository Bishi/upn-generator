# UPN Preview And Send

> UPN page, UPN PDF generation, download, SMTP send, delivery history

## Pre-conditions

- Selected billing month has calculated splits.
- Apartment payer names and contact emails are configured.
- SMTP settings are configured under Settings -> Delivery -> Email when testing send actions.
- Email safety allowlist state is known before testing.

## Cases

- [ ] Email uses the shared drawer shell at 720 px on wide windows and fits narrow windows; backdrop fade, panel motion, header, and footer match inbox import.
- [ ] Tab/Shift+Tab stay in the drawer, Escape/backdrop/Close dismiss it, and focus returns to Send All Emails.
- [ ] Background content cannot scroll while the drawer is open; internal scrolling keeps the header and footer visible.

- [ ] Drawer uses concise attachment summaries, with no email-content help sentence; global attachment summary still identifies every recipient and the file limits.

- [ ] Body editor is white with readable dark text, including when the app uses a dark theme.
- [ ] Add multiple global attachments, confirm filenames/sizes, remove one, and send to allowlisted test recipients: every recipient gets the remaining exact files with UPN slips.
- [ ] Global attachments are still included when original bills are unchecked; leaving global attachments empty preserves the usual UPN/originals behavior.
- [ ] Reject selections exceeding 10 files or 15 MiB combined without adding any files from the rejected selection; previously selected files remain.
- [ ] Confirm send stays disabled while files are being read; close during a read and confirm no late result appears in a reopened drawer.
- [ ] Cancel and reopen, change billing period, or navigate away: all global attachment selections are discarded.
- [ ] Duplicate filenames, including a name matching the UPN PDF or an original bill, arrive under distinct safe filenames.
- [ ] After sending with global attachments, delivery status continues to track the UPN packet; backups contain no global attachment contents.

- [ ] Open Send All Emails at narrow and wide window sizes: no horizontal scrollbar appears during the slide-in animation; the drawer body still scrolls vertically.

- [ ] Send All loads saved subject/body with placeholders resolved before Confirm send is available.
- [ ] Edit subject and multiline body, send to allowlisted test recipients, and confirm every recipient gets the exact edited content and expected attachments.
- [ ] Clear the subject: Confirm send is disabled. Clear only the body: sending remains available.
- [ ] Cancel and reopen: batch edits are discarded and saved defaults are loaded again.
- [ ] Original-bill note appears only when originals are enabled and available; partial availability uses wording about available originals.
- [ ] A content loading error is visible and blocks sending; closing/reopening retries.

- [ ] Select an empty billing month and confirm there is no red validation blocker or `0 validation issues` chip; the page prompts for bill import and bulk UPN actions remain disabled.
- [ ] Select a billing period and confirm each apartment card shows line items and total due.
- [ ] Confirm the validation panel appears for the selected billing period and summarizes blocking errors and warnings without pushing content down unexpectedly.
- [ ] Remove an apartment recipient for an apartment with a UPN packet and confirm Send Emails is disabled while Download All PDFs and Mark Delivered remain available.
- [ ] Enter an invalid recipient email and confirm Send Emails is disabled with a clear validation issue.
- [ ] With email safety allowlist enabled, use a non-allowlisted recipient and confirm Send Emails is blocked before SMTP password lookup or delivery history creation.
- [ ] Remove or invalidate a bill IBAN, reference, due date, purpose code, or purpose text and confirm Send Emails, Download All PDFs, and Mark Delivered are disabled.
- [ ] Import a bill with a parser/OCR review warning and confirm Send Emails, Download All PDFs, and Mark Delivered are disabled until the bill is marked reviewed on Bills.
- [ ] Mark the warning bill reviewed and confirm the same UPN bulk actions become available again when no other blockers remain.
- [ ] Create a duplicate provider bill or split total mismatch and confirm Send Emails, Download All PDFs, and Mark Delivered are disabled until the issue is fixed.
- [ ] Open a single apartment preview and confirm a PDF is generated without an app error.
- [ ] Confirm single-apartment previews remain available when validation only blocks delivery actions.
- [ ] Open one slip and an apartment's Preview all packet; confirm both appear in the in-app PDF viewer, show the correct slips in order, and close with the button or Escape.
- [ ] Switch between UPN previews and confirm no stale slip remains; a preview generation failure appears inside the viewer and closing it returns focus to the opening button.
- [ ] Download all PDFs and confirm the save dialog is pre-populated with a `.zip` filename for the selected month.
- [ ] While Download All PDFs is preparing the export, confirm the page shows a loading overlay and blocks other clicks until the export completes or fails.
- [ ] Open the exported ZIP and confirm it contains one combined PDF packet for each apartment.
- [ ] Confirm Download All PDFs does not mark the month delivered or create delivery history.
- [ ] Try closing a month before every current packet is delivered and confirm the action is blocked.
- [ ] With every current apartment packet delivered and Mark Delivered validation passing, click Close Month once and confirm it closes immediately without a confirmation dialog; confirm the month picker shows it as closed.
- [ ] Start an email batch for the month, attempt Close Month through another app window or direct command while the batch is active, and confirm closing is rejected with an in-progress message; retry after the batch completes and confirm closing succeeds when eligibility still passes.
- [ ] Close a month, reload the app, and confirm the closed state and closure timestamp remain visible across Bills, Splits, UPN Preview, and the month picker.
- [ ] In a closed month, confirm single/apartment previews and Download All PDFs still work, while Send Emails, Mark Delivered, and Unmark Delivered are disabled.
- [ ] Upgrade or restore a previously closed month whose bills have `Not checked` identity; confirm Download All PDFs remains available, delivery stays blocked, and reopening restores the bulk export identity gate.
- [ ] With a closed month containing an apartment's splits or delivery history, try deleting that apartment in Settings and confirm a visible error says the affected month must be reopened, while the apartment and closed-month records remain unchanged.
- [ ] Click Reopen Month once and confirm it reopens immediately without a confirmation dialog; confirm delivery actions become available again when validation allows them.
- [ ] Click Mark Delivered, cancel the confirmation prompt, and confirm the month remains undelivered.
- [ ] Click Mark Delivered, accept the confirmation prompt, and confirm all current apartment packets are marked delivered.
- [ ] Confirm the successful Mark Delivered message appears as a toast and does not push page content down.
- [ ] Trigger a UPN action error and confirm it appears as a manually dismissed error toast without pushing page content down.
- [ ] Trigger a workflow error, navigate away from UPN Preview and back, and confirm no duplicate error toast appears.
- [ ] Click Unmark Delivered, cancel the confirmation prompt, and confirm manual delivery marks remain.
- [ ] Click Unmark Delivered, accept the confirmation prompt, and confirm manual delivery marks are removed while email delivery history remains.
- [ ] Confirm a mixed month with some emailed packets and a manual delivered confirmation marks the UPN workflow step complete.
- [ ] Confirm generated UPN PDFs use the selected billing month, payer, recipient, IBAN, reference, purpose, and amount.
- [ ] Open single and apartment previews and confirm they do not create delivery history or temporary PDF files.
- [ ] Navigate away from and back to UPN Preview after manual delivery and confirm delivered pills, row statuses, and Mark/Unmark button do not flash through an incorrect state.
- [ ] Send emails with allowlist enabled and confirm all non-allowlisted recipients are blocked by validation.
- [ ] Send a test email to an allowed recipient and confirm delivery succeeds.
- [ ] Click Send All Emails and confirm a summary drawer shows the selected billing month, apartment packet count, total slip count, and each apartment's amount, slip count, and recipient addresses, including multiple comma-separated recipients; no emails are sent yet.
- [ ] Close the summary with Cancel, the close icon, Escape, or the backdrop and confirm no emails are sent and delivery history plus displayed results remain unchanged.
- [ ] Use Tab and Shift+Tab in the drawer and confirm focus stays inside it, starts on Cancel, and returns to Send All Emails when dismissed.
- [ ] Click Confirm send and confirm the drawer closes and exactly one batch starts; repeated clicks must not start duplicate batches or show an additional native confirmation prompt.
- [ ] With validation warnings present, confirm the drawer includes the warning count before sending.
- [ ] After a prior successful send, click Send All Emails and confirm the drawer warns that previously emailed packets will be sent again and sent emails cannot be recalled.
- [ ] If workflow data refreshes while the drawer is open, confirm its summary follows the current snapshot and Confirm send is disabled while loading or when validation blocks sending.
- [ ] At a narrow window size or with many recipients, confirm the summary scrolls while Cancel and Confirm send remain visible in the footer.
- [ ] With Attach original bills checked, confirm the drawer describes full original PDFs/images, the available file count, and unavailable originals; uncheck it and confirm the drawer says UPN slips only and the resulting emails omit originals.
- [ ] Send UPN emails for the October 2026 billing period and confirm the subject is `Poloznice za 09/2026` and the message says `v priponki najdete UPN položnice za 09/2026.`.
- [ ] Open UPN Preview and confirm Attach original bills is checked by default; navigate away and back and confirm a newly opened page defaults to checked again.
- [ ] With the checkbox checked, send an apartment email and confirm it includes the UPN PDF plus one Original_bills.pdf with all supplier PDF pages in source order; verify page sizes, rotation, text, images, and readability against the sources. Original images remain separate exact-byte attachments.
- [ ] Open the Send All Emails confirmation drawer with originals enabled and confirm its attachment summary describes one combined supplier PDF per apartment plus separate image bills.
- [ ] Import several bills from one combined PDF and confirm every page is included once in Original_bills.pdf, including unselected/informational pages; confirm View original still opens the unchanged stored source.
- [ ] Use separate source documents for different apartments/months and confirm only sources linked to the current apartment packet are sent.
- [ ] Import distinct PDF originals with the same filename and confirm both documents' pages appear in Original_bills.pdf; confirm distinct image originals with the same filename retain distinct readable attachment names and unchanged bytes.
- [ ] Use a malformed, encrypted, or layered PDF among multiple originals in a disposable database and confirm combining fails visibly before sending (layered PDFs must report that they cannot be safely combined); confirm UPN-only sending remains available when Attach original bills is unchecked.
- [ ] Uncheck Attach original bills and confirm the helper text says only UPN slips will be sent; send and confirm the email contains only the UPN PDF and no mention of supplier originals.
- [ ] Include a manual/legacy bill with no original and confirm the page identifies the bill as unavailable before sending; confirm sending still includes UPN slips and any available originals.
- [ ] In a disposable database, corrupt a linked original's bytes or remove the linked source row; confirm that apartment's send fails visibly without sending an incomplete email, while unaffected apartments can still send. Uncheck the checkbox and confirm UPN-only sending succeeds.
- [ ] After a send fails because a linked original is corrupt or missing, reload or navigate away and back; confirm the unchanged UPN packet shows Failed rather than Changed. If it was successfully sent earlier, confirm it retains delivery completion but displays a warning for the latest failed attempt.
- [ ] While sending, confirm the checkbox is disabled; in a closed month, confirm it and Send All Emails are disabled.
- [ ] Send with originals and multiple recipients and confirm each recipient receives the same attachments and per-recipient delivery history remains correct after reload; toggling the checkbox alone does not change delivery completion.
- [ ] Send UPN emails for the October 2026 billing period and confirm the subject is `Poloznice za 09/2026` and the message says `v priponki najdete UPN položnice za 09/2026.`; when originals are attached, confirm the message also says `Priloženi so tudi izvirni računi dobaviteljev.`.
- [ ] Send UPN emails for the January 2027 billing period and confirm both subject and message name `12/2026`.
- [ ] Send UPN emails for a past billing period and confirm the subject and message use the month before the selected period regardless of today's date.
- [ ] Send apartment emails with multiple comma-separated recipients and confirm per-recipient status is recorded.
- [ ] Reload the UPN page and confirm delivery history is restored for sent, failed, blocked, or partial rows.
- [ ] Change packet content after a prior send/manual confirmation and confirm the prior delivery is shown as no longer current until sent or manually marked delivered again.
- [ ] Set a bill identity status to an unverified state and confirm Send Emails, Download All PDFs, and Mark Delivered are blocked by `building_identity_unverified`.
- [ ] Approve a non-empty identity exception and confirm those actions are eligible again when no unrelated validation issue remains; the UI must not call the exception a match.

- [ ] Generate a five-slip apartment packet in both development and packaged builds; confirm the PDF is comfortably below 200 KB with the standard Windows fonts.
- [ ] Confirm previews, ZIP downloads, and received email attachments all contain the expected slips in order, including Slovenian letters that appear only on later pages.
- [ ] Scan the QR code on each slip from a received attachment and confirm its amount, IBAN, reference, and payment details match the bill.

## Notes

Use [upn-visual-reference.md](./upn-visual-reference.md) when the rendered UPN geometry or printed output changes.
