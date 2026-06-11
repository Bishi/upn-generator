# Bills Import

> Bills page, bill parser pipeline, manual entry, import debug log

## Pre-conditions

- App is running with seeded building, apartment, and provider data.
- A billing year/month exists on the Bills page.
- Test files are available from `file-examples/` or another known local bill sample set.

## Cases

- [ ] Add a new billing year and confirm all 12 months are available.
- [ ] Import a combined PDF and confirm one row is created per detected configured provider.
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

