# Status

- Current version: `0.6.2`
- Current tag: `v0.6.2`
- Release status: released from `main`

## Latest Included Changes

- Accepted valid PDF attachments sent with generic binary MIME types after verifying the filename and file signature
- Recognized additional service-period formats, including date ranges, two-digit years, and month/year hints, while preserving boundaries between extracted fields
- Improved gas identity verification with higher-resolution OCR, preserved line boundaries, and better label matching while keeping identifier values exact
- Skipped tiny inline email graphics and distinguished documents without invoice fields from invoices with an unknown billing period
- Made inbox rejection explanations available through a compact Why? link and widened the review drawer
- Added cancellable Fetch Preview scans with immediate network cancellation and cleanup of cancelled results
- Kept the Fetch Preview button stationary when Cancel scan appears
- Made immediate retries wait cancellably for pending PDF OCR, preventing resource contention from becoming an invoice diagnostic
- Released the OCR slot correctly when document path resolution fails before the worker starts

## Verification

- Frontend production build and Rust cargo check passed
- 109 Rust tests passed, plus all three private invoice regression tests
- Affected inbox manual QA cases are documented and remain unchecked pending desktop verification
