# UPN Visual Reference

> UPN PDF rendering, preview/download output, `file-examples/`

## Pre-conditions

- A billing month has calculated splits and at least one apartment UPN can be generated.
- Reference examples are available in `file-examples/`.
- A PDF viewer or print-preview tool is available for visual inspection.

## Cases

- [ ] Confirm the generated form is 210 mm x 99 mm.
- [ ] Confirm the standard two-part UPN layout is preserved.
- [ ] Confirm fields align visually with the official/reference form geometry.
- [ ] Confirm machine-print text uses a Courier New-style monospaced appearance.
- [ ] Confirm payer, recipient, amount, IBAN, reference, purpose, due date, and QR area appear in the expected locations.
- [ ] Confirm long payer or purpose values do not overlap neighboring fields.
- [ ] Confirm output remains visually close to examples in `file-examples/`.
- [ ] Print or print-preview at actual size and confirm scaling is not applied unexpectedly.

## Notes

Any UPN rendering change should be checked here before release.
