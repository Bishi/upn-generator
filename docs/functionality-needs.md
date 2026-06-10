# Functionality Needs

This product is already past MVP for the core monthly workflow: create/select a
billing period, import bills, calculate apartment splits, generate UPN slips,
download PDFs, and send emails.

The next functional priorities are mostly about trust, auditability, and closing
the monthly workflow cleanly.

## Highest Priority

### Secure Credential Storage

SMTP and IMAP passwords should be moved out of SQLite and into Windows
Credential Manager or another keyring-backed store.

Backups already strip passwords, but local plaintext password storage remains the
main unfinished security item in Phase 4.

### Month Close and Delivery Status

The app needs a real "this month is done" state.

Email delivery events now exist, but the workflow snapshot still does not treat a
period as sent/complete, and PDF downloads are not persisted as delivery events.

Needed behavior:

- Persist successful and failed email delivery per apartment/recipient.
- Persist manually saved PDF packets.
- Show period delivery state as ready, partial, sent, failed, or closed.
- Mark the workflow UPN step complete only when every apartment has either a
  successful email delivery or a saved PDF packet.
- Add a clear month-close action once delivery is complete.

### Pre-Send Validation Gates

Before sending UPN packets, the app should block or warn on conditions that could
produce incorrect payment slips.

Recommended checks:

- Unmatched bills.
- Bills with unreviewed OCR/import warnings.
- Split totals that do not match bill totals.
- Apartments with missing email recipients.
- Invalid recipient email addresses.
- Invalid or missing IBAN, reference, due date, purpose code, or purpose text.
- Apartment m2 percentages that do not sum to the expected total.
- Occupant-based providers when all active apartments have zero occupants.

### Historical Correctness

Apartment settings and provider rules can change over time. Recalculating an old
month should not accidentally use today's data if occupants, m2 shares, active
status, contacts, or split rules changed.

Recommended approach:

- Snapshot apartment split inputs per billing period.
- Snapshot provider split rules per billing period.
- Keep the generated/sent packet tied to the data used at the time.

### Delivery Recovery

Users need a practical way to recover from partial sends.

Needed behavior:

- Retry failed recipients.
- Resend one apartment packet.
- View exact delivery errors after reload.
- Distinguish "sent by email" from "saved manually as PDF".
- Show when a recipient was blocked by the test allowlist.

## Next Useful Layer

### Payment Tracking

The workflow currently ends at sending UPNs. The product should eventually track
whether each apartment has paid.

Initial version:

- Manual paid/unpaid status per apartment and period.
- Paid date and optional note.
- Monthly paid/unpaid summary.

Later version:

- Import bank CSV/CAMT statements.
- Match incoming payments to references and apartment totals.

### Monthly Archive and Export

The accountant should be able to archive a completed month in one action.

Useful exports:

- All generated apartment UPN PDFs.
- Imported bill files or bill metadata.
- Split summary by provider and apartment.
- Delivery history.
- Payment status, once payment tracking exists.

Possible formats:

- ZIP archive.
- CSV summary.
- Accountant-friendly PDF report.

### Explicit Review Workflow

Bills with parser or OCR warnings should have a real reviewed/accepted state.

This is better than relying only on `parse_note`, because a warning can remain
informational after the user has verified it.

Needed behavior:

- Mark a bill as reviewed.
- Store reviewed timestamp.
- Optionally store reviewed-by text or local note.
- Block sending until warning bills are reviewed, unless the user explicitly
  overrides.

### Provider and Parser Maintenance

Provider invoice formats may change. The app needs a practical way to keep import
rules healthy without editing code.

Useful functionality:

- Show expected providers for the selected month.
- Show missing providers after import.
- Test a provider parser against a sample file.
- Display parsed fields side-by-side with the extracted text.
- Preserve sample files for regression testing where privacy permits.

### Correction Handling

Monthly utility accounting may need manual corrections.

Useful functionality:

- Credit notes or negative adjustments.
- Manual rounding adjustments.
- Apartment-specific adjustment rows.
- Reason/note fields for manual changes.
- Clear display of automatic split rows versus manual overrides.

## Lower Priority or Defer

These are likely not needed until the product has a broader audience:

- Multi-building support.
- Cloud sync.
- Scheduled automatic inbox imports.
- Multi-user roles and permissions.
- Complex accounting integrations.

The current product is focused on one building accountant running a local Windows
desktop workflow. The highest-value functional work is making the existing flow
harder to misuse and easier to prove correct.
