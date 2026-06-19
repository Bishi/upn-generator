# Splits

> Splits page, split calculation command, provider split basis rules

## Pre-conditions

- Selected billing month has at least one imported or manual bill.
- Apartments have occupant counts, m2 percentages, and active/inactive state configured as needed.
- Providers include at least one `occupants`, one `m2_percentage`, and one `equal_apartments` split basis.

## Cases

- [ ] Recalculate splits for a month and confirm every bill has apartment allocations.
- [ ] Confirm occupant-based providers split according to occupant counts.
- [ ] Confirm m2-based providers split according to configured apartment percentages.
- [ ] Confirm equal-apartment providers split evenly across active apartments.
- [ ] Confirm split amounts add up to the original bill total after rounding.
- [ ] Manually adjust a split cell and confirm the change persists after reload.
- [ ] Change a source bill amount and confirm the Splits page warns or requires recalculation.
- [ ] Recalculate after a bill/provider/apartment change and confirm stale split data is replaced.
- [ ] Confirm an unreviewed imported bill warning shows a warning indicator on Splits, then mark the bill reviewed on Bills and confirm Splits no longer shows it as unresolved after refresh.
- [ ] Confirm inactive apartments are handled according to the current product rule.
- [ ] Confirm UPN validation blocks delivery actions when a bill has no splits, split totals do not match the bill total, or a split belongs to an inactive apartment.
- [ ] Set all active apartments to zero occupants for an occupant-based provider and confirm UPN validation blocks delivery actions until occupants or split basis are corrected.
- [ ] Change m2 percentages so active apartments no longer total 100 and confirm UPN validation shows a warning without blocking delivery actions.

## Notes

After split changes, verify UPN totals in [upn-preview-send.md](./upn-preview-send.md).
