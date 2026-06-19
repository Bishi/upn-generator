# Bills Import

> Bills page, bill parser pipeline, manual entry, import debug log

## Pre-conditions

- App is running with seeded building, apartment, and provider data.
- Test files are available from `file-examples/` or another known local bill sample set.

## Cases

- [ ] Open the month picker, confirm clicking outside or pressing Escape closes it.
- [ ] Mark a billing month delivered from UPN Preview and confirm the month picker shows that month as closed.
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

## Notes

For inbox imports, use [inbox-import.md](./inbox-import.md). For split recalculation after bill changes, use [splits.md](./splits.md).
