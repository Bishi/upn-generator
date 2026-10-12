# Status

- Current version: `0.7.3`
- Current tag: `v0.7.3`
- Release status: released from `main`

## Latest Included Changes

- Single UPN slips and apartment packets open in the in-app PDF viewer without temporary PDF files or the system PDF opener
- Apartment emails combine available supplier PDFs into one `Original_bills.pdf` alongside the existing UPN PDF, including every source page and deduplicating shared sources
- Original images remain separate exact-byte attachments; retained source documents stay unchanged
- Multi-PDF merging rejects encrypted or layered sources rather than sending altered or incomplete bills; UPN-only sending remains available through the attachment checkbox
- Page and confirmation drawer summaries describe the combined supplier PDF; confirmation, delivery history, compact UPN PDFs, and previous service-month email wording remain available

## Verification

- UPN preview frontend build and `cargo check` passed; both focused compact UPN PDF tests passed; manual Windows viewer QA remains unchecked
- Frontend production build passed after integrating the Send All Emails confirmation drawer
- `cargo check` and all 29 UPN tests passed, including page order, inherited resources and rotation, attachment MIME handling, failure audit identity, and layered-PDF rejection
- Independent subagent review finding fixed; the pushed fix was re-reviewed with no remaining actionable findings
- Affected manual QA cases remain unchecked pending desktop and live SMTP verification
