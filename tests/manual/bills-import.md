# Bills Import

> Bills page, bill parser pipeline, manual entry, import debug log

## Pre-conditions

- App is running with seeded building, apartment, and provider data.
- Test files are available from `file-examples/` or another known local bill sample set.

## Cases

- [ ] Open the month picker, confirm clicking outside or pressing Escape closes it.
- [ ] Close a billing month from UPN Preview and confirm the month picker shows that month as closed.
- [ ] Select a closed billing month and confirm Add Bill, Import Bills, Import from Inbox, row edit/delete, and review-state buttons are disabled.
- [ ] Reopen the closed billing month from UPN Preview and confirm Bills page import/edit/delete actions become available again.
- [ ] Use the previous/next year arrows, select a month that has no bills yet, and confirm the Bills page shows the normal empty month state.
- [ ] Change the picker year, close the picker without selecting a month, and confirm reopening starts on the current year.
- [ ] Cancel a local bill import for a newly selected empty month and confirm no billing period row is created.
- [ ] Import a combined PDF and confirm one row is created per detected configured provider.
- [ ] Import or preview a combined PDF where one detected bill is missing a due date, and confirm that bill is marked for review on Bills instead of appearing as a clean auto-match.
- [ ] Click Mark reviewed on an imported warning bill and confirm it changes to a reviewed state while the import note remains visible.
- [ ] Click Unreview on a reviewed warning bill and confirm it returns to the unresolved review state.
- [ ] Edit a reviewed warning bill amount, reference, purpose, or due date and confirm the bill becomes unresolved again after save.
- [ ] Confirm missing visible payment fields on Bills and inbox preview rows render as red `missing` text instead of an empty cell.
- [ ] Confirm importing, adding a manual bill, or fetching an inbox preview for a newly selected month creates and selects that billing period.
- [ ] Select an empty month, reload the app, and confirm the same month remains selected.
- [ ] Import a supported image file and confirm OCR text is parsed into provider, amount, reference, purpose, and due date fields.
- [ ] Confirm bills whose document title/period belongs to the selected month are accepted.
- [ ] Confirm a bill for the wrong selected month is rejected or clearly blocked before save.
- [ ] Confirm unknown providers are not silently added as configured providers.
- [ ] Confirm duplicate provider/month imports do not create duplicate bill rows.
- [ ] If duplicate provider bills are created manually or from restored data, confirm UPN validation blocks delivery actions for that month.
- [ ] Add a bill manually and confirm it participates in the month total.
- [ ] Edit an imported bill and confirm amount/reference/purpose/due date changes persist after reload.
- [ ] Clear or invalidate a bill IBAN, reference, purpose code, purpose text, or due date and confirm UPN validation blocks delivery actions.
- [ ] Delete a bill and confirm totals and downstream split warnings update.
- [ ] Open `%APPDATA%\si.upn-generator\import_debug.log` and confirm it contains useful parser diagnostics for local imports.
- [ ] Select one or more local files and confirm a review dialog appears before any bill row is saved, with provider, amount, identity result, and source-page range for each candidate.
- [ ] In the August combined sample, confirm electricity, gas, waste, water, and ZLM candidates all show matched building identity; gas and waste must succeed through rendered-page OCR rather than payment-purpose digits alone.
- [ ] Change water identity from `5495/109463` to `5495/1094630` or `5495/109463-0` while leaving the configured value in a later unrelated field, and confirm the invoice is mismatched rather than verified.
- [ ] Confirm extracted values `5495 / 109463` and `3 - 82858` match their configured water and electricity identifiers, while suffixed variants remain mismatches.
- [ ] Place an unparsed invoice with matching identity evidence before or after a parsed invoice that lacks its own evidence, and confirm the parsed invoice remains unverified unless the pages share its complete reference or invoice number; `SI12 1110`, `SI12 111 0`, and `SI12 111 - 0` must not link to `SI12 111`, while `SI12 111` followed by an unrelated numeric line still links.
- [ ] Preview a scanned invoice whose native PDF text contains only a header/footer while OCR contains the payment fields, and confirm the OCR candidate is recovered without duplicating a matching native candidate.
- [ ] Deselect a candidate, confirm import, and verify only selected candidates are saved.
- [ ] Select two distinct invoices for the same provider/month across one or more files and confirm neither is saved; exact duplicate content should be deduplicated.
- [ ] Change a source file after preview and confirm final import is rejected without partial writes.
- [ ] In the local review dialog, trigger missing exception note, provider/month conflict, and changed source file errors; confirm each error appears inside the open dialog and the current selections and notes remain available for correction.
- [ ] Select a mismatched, missing, unreadable, or unconfigured candidate and confirm import requires a non-empty exception note; verify the saved row says Exception rather than Matched.
- [ ] Edit payment content on an exception bill and confirm its identity returns to Not checked and splitting remains blocked until a new noted exception is approved.
- [ ] In local import review, click View source/View page for a PDF and image selected outside the app directory; confirm each opens inside the app without exposing or reopening the caller-supplied path.
- [ ] Remove, rename, or change a selected local source after preview; confirm staged viewing/finalization rejects it, clears the handle, and requires a fresh preview without deleting the user's file.

## Notes

For inbox imports, use [inbox-import.md](./inbox-import.md). For split recalculation after bill changes, use [splits.md](./splits.md).
