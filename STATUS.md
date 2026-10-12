# Status

- Current version: `0.6.4`
- Current tag: `v0.6.4`
- Release status: released from `main`

## Latest Included Changes

- Reduced UPN packet size by sharing fonts across slips, embedding only used font characters, and applying lossless compression in development and release builds
- Regenerated the reported five-slip packet at 84,636 bytes versus 9,288,356 bytes, with identical text, QR payment data, and pixel-identical renders at 216 DPI
- Added regression coverage for size, shared fonts, page dimensions and order, exact QR pixels, later-page Slovenian characters, and empty packets
- Preserved the v0.6.3 email service-month wording fix, including January-to-December year rollover

## Verification

- Rust cargo check passed; full Rust test suite passed with 113 tests and 3 existing private-sample tests ignored
- Independent subagent review found no actionable issues in the PDF size change
- Affected manual QA cases are documented and remain unchecked pending packaged desktop and email verification
