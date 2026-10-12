# Status

- Current version: `0.6.3`
- Current tag: `v0.6.3`
- Release status: released from `main`

## Latest Included Changes

- Corrected sent UPN email subjects and messages to name the service month before the selected billing period, including January-to-December year rollover
- Added regression tests and manual QA cases for the email month wording

## Verification

- Rust cargo check passed
- All 19 UPN tests passed, including the email month and year-rollover regressions
- Independent subagent review found no issues
- Affected email manual QA cases are documented and remain unchecked pending desktop verification
