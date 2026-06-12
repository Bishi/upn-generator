# Bills Import

> Bills page, bill parser pipeline, manual entry, import debug log

## Pre-conditions

- App is running with seeded building, apartment, and provider data.
- Test files are available from `file-examples/` or another known local bill sample set.

## Cases

- [ ] Open the month picker, confirm clicking outside or pressing Escape closes it.
- [ ] Use the previous/next year arrows, select a month that has no bills yet, and confirm the Bills page shows the normal empty month state.
- [ ] Cancel a local bill import for a newly selected empty month and confirm no billing period row is created.
- [ ] Import a combined PDF and confirm one row is created per detected configured provider.
- [ ] Confirm importing, adding a manual bill, or fetching an inbox preview for a newly selected month creates and selects that billing period.
- [ ] Select an empty month, reload the app, and confirm the same month remains selected.
- [ ] Import a supported image file and confirm OCR text is parsed into provider, amount, reference, purpose, and due date fields.
- [ ] Confirm bills whose document title/period belongs to the selected month are accepted.
- [ ] Confirm a bill for the wrong selected month is rejected or clearly blocked before save.
- [ ] Confirm unknown providers are not silently added as configured providers.
- [ ] Confirm duplicate provider/month imports do not create duplicate bill rows.
- [ ] Add a bill manually and confirm it participates in the month total.
- [ ] Edit an imported bill and confirm amount/reference/purpose/due date changes persist after reload.
- [ ] Delete a bill and confirm totals and downstream split warnings update.
- [ ] Open `%APPDATA%\si.upn-generator\import_debug.log` and confirm it contains useful parser diagnostics for local imports.

## Notes

For inbox imports, use [inbox-import.md](./inbox-import.md). For split recalculation after bill changes, use [splits.md](./splits.md).
