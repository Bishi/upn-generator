# Status

- Current version: `0.7.1`
- Current tag: `v0.7.1`
- Release status: released from `main`

## Latest Included Changes

- Send All Emails opens a summary drawer showing the billing month, apartment totals, UPN slip counts, recipient addresses, and original-bill attachment scope
- Confirm send explicitly starts the batch; Cancel, Escape, the close button, and backdrop dismissal leave delivery history unchanged
- Confirmation blocks during initial loading and background workflow refreshes, and a synchronous guard prevents duplicate batches
- Retained default-on original bill attachments, compact UPN PDFs, and previous service-month email wording

## Verification

- Frontend production build passed after integration with the original-bill attachment feature
- Independent subagent review finding fixed; fix and main-branch integration re-reviewed with no actionable findings
- Affected manual QA cases remain unchecked pending desktop and live SMTP verification
