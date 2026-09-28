# Provider Identity Rules

> Settings -> Providers, typed whole-building invoice identity rules

## Pre-conditions

- Seeded providers are present.
- A draft billing month and representative local invoice samples are available.

## Cases

- [ ] Confirm electricity is seeded as an OR rule for customer `C0367125` or meter `3-82858`.
- [ ] Confirm gas, waste, and water preserve leading zeroes and the water slash in their labeled values.
- [ ] Confirm ZLM displays the temporary `Kamniška 36` address rule with a visible weaker-evidence warning and no placeholder offer number.
- [ ] Switch a provider to Unconfigured, save, and confirm future previews say Unconfigured rather than Matched.
- [ ] Configure an AND/OR labeled-value rule, reload Settings, and confirm its type, operator, labels, and values persist.
- [ ] Try saving an incomplete or invalid typed rule and confirm backend validation rejects it with a useful message.
- [ ] Replace the ZLM address rule with a real offer-number labeled-value rule when one is available and confirm no code or regex editing is required.

## Notes

Editing a provider rule affects final revalidation of new imports. It does not rewrite the historical evidence snapshot stored on already imported bills.
