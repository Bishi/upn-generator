# Status

- Current version: `0.7.0`
- Current tag: `v0.7.0`
- Release status: released from `main`

## Latest Included Changes

- Added Attach original bills to UPN email sending, checked by default; each apartment receives its UPN slips and the exact complete stored supplier PDFs/images for its bills
- Combined PDFs are sent in full, shared sources are attached once per email, and attachment encoding preserves the exact original bytes
- Missing legacy/manual originals show a warning while available originals still send; linked-file integrity failures fail the affected apartment's send
- Preserved the valid UPN packet hash on attachment preparation failures so failure history remains current after reload and failed resends retain warnings alongside earlier successful delivery
- Retained the compact UPN PDF rendering and previous service-month email wording from v0.6.4 and v0.6.3

## Verification

- Frontend production build and Rust cargo check passed
- All 44 UPN and pre-send validation tests passed, including original attachment bytes, apartment/month scoping, deduplication, integrity failures, current failure-history association, and compact PDF rendering
- Independent subagent review finding fixed; re-review of the exact fix commit found no further defects
- Affected manual QA cases are documented and remain unchecked pending desktop and live SMTP verification
