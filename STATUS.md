# Status

- Current version: `0.8.2`
- Current tag: `v0.8.2`
- Release status: tagged from `main`; the Release workflow publishes the Windows installer

## Latest Included Changes

- ZLM invoice import accepts both normal `SI00` references and references extracted with spaces between their model digits. Reference lookup stays within the invoice's TRR-to-total section to avoid using a neighbouring invoice's reference.
- The UPN page button reads `Review & Send Emails...` and opens a compact review drawer with batch email content, attachment selection, and a recipients table
- Settings > Email Content saves the default subject and body, with previous-service-month placeholders, concise help, backup/restore support, factory-reset defaults, and unsaved-change handling
- Send All Emails lets the accountant edit the resolved subject/body for one batch; the built-in subject uses `Položnice`, and custom saved subjects are preserved
- Additional global attachments go to every recipient, remain in memory only, and support add/remove before confirmation with a limit of 10 files and 15 MiB combined
- Email and inbox import use a shared configurable drawer with responsive width, fading backdrop, clipped animation, fixed header/footer, keyboard focus handling, close guards, and companion-viewer support
- Retained the v0.7.3 in-app UPN previews, combined supplier PDF attachments, exact original-image attachments, source-document retention, recipient safety, and UPN delivery audit semantics

## Verification

- Both supplied ZLM PDFs passed the app's PDF extractor/parser reproduction with correct amount, IBAN, reference, and due date; the newer invoice failed before the fix
- Rust suite: 130 passed, 3 existing private PDF/OCR fixture tests ignored; focused ZLM regression tests, `cargo check`, formatting, and diff checks passed
- TypeScript and frontend production build passed; the standalone Windows debug app built and launched successfully
- One-round Sol source review approved the parser change with no findings
- Live SMTP/IMAP and desktop import manual QA remain unverified; affected checklist cases remain unchecked
