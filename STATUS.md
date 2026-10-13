# Status

- Current version: `0.8.1`
- Current tag: `v0.8.1`
- Release status: pending merge and tag from `main`

## Latest Included Changes

- The UPN page button reads `Review & Send Emails...` and opens a compact review drawer: the subtitle states how many emails go to how many addresses with which attachments, the footer button reads `Send N emails`, a warning names already-emailed apartments with dates only when relevant, one Attachments box covers UPN/originals and files for everyone, and a recipients table replaces per-apartment cards; the body editor auto-grows and stays resizable
- Settings > Email Content saves the default subject and body, with previous-service-month placeholders, concise help, backup/restore support, factory-reset defaults, and unsaved-change handling
- Send All Emails lets the accountant edit the resolved subject/body for one batch; the built-in subject uses `Položnice`, and custom saved subjects are preserved
- Additional global attachments go to every recipient, remain in memory only, and support add/remove before confirmation with a limit of 10 files and 15 MiB combined
- Email and inbox import use a shared configurable drawer with responsive width, fading backdrop, clipped animation, fixed header/footer, keyboard focus handling, close guards, and companion-viewer support
- Retained the v0.7.3 in-app UPN previews, combined supplier PDF attachments, exact original-image attachments, source-document retention, recipient safety, and UPN delivery audit semantics

## Verification

- Frontend production build and TypeScript check passed for the v0.8.1 drawer changes; Rust is untouched since v0.8.0
- Rust suite: 127 passed, 3 ignored because they require private PDF/OCR fixtures
- Headless Edge checks with mocked IPC covered configurable drawer widths, animation clipping, vertical scrolling, focus containment/wrap/restoration, close guards, companion focus/Escape handoff, inbox popovers, scan cancellation and late-session cleanup, plus global attachment selection/removal/limits/cleanup and exact IPC bytes
- Independent subagent code review found no actionable correctness, security, or regression issues; stale manual QA expectations were corrected and re-reviewed with no remaining findings
- Live SMTP/IMAP and desktop manual QA remain unverified; affected checklist cases remain unchecked
