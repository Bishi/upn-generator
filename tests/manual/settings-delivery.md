# Settings Delivery

> Settings -> Delivery, SMTP settings, IMAP settings, Windows Credential Manager passwords

## Pre-conditions

- App is running on Windows.
- Test SMTP and IMAP credentials are available if connection tests are performed.
- Windows Credential Manager state is known or disposable for the test account.

## Cases

- [ ] Built-in subject uses "Položnice". An unchanged old default upgrades automatically; a customized subject is preserved.

- [ ] Email Content shows one short description per placeholder row, has no introductory paragraph, and uses a white body editor with dark text.

- [ ] Email Content appears before App; save a subject and multiline body, restart, and confirm both persist.
- [ ] Save `{month}`, `{year}`, and `{originals_note}` placeholders; confirm Send All resolves them, including January's previous year.
- [ ] Empty/whitespace subjects are rejected; an empty body saves successfully.
- [ ] Saving email content preserves the appearance theme; changing the theme preserves email content.

- [ ] Save SMTP settings with a new password and confirm the form reports a saved password state after reload.
- [ ] Leave SMTP password blank on a later save and confirm the existing saved password is kept.
- [ ] Change SMTP username and confirm the old saved password is not reused for the new username.
- [ ] Send a test email using the current SMTP form values.
- [ ] Toggle email safety allowlist and confirm the saved state survives reload.
- [ ] With the allowlist enabled, confirm UPN validation blocks Send Emails for non-allowlisted apartment recipients before any SMTP connection is attempted.
- [ ] Save IMAP settings with a new password and confirm the form reports a saved password state after reload.
- [ ] Leave IMAP password blank on a later save and confirm the existing saved password is kept.
- [ ] Change IMAP username and confirm the old saved password is not reused for the new username.
- [ ] Test the IMAP connection using the current Inbox settings.
- [ ] Save the IMAP mailbox scan window as 0 and confirm it persists as today-only after reload.
- [ ] Save sender allowlist changes and confirm the saved state is used by inbox import.

## Notes

Passwords are write-only in the UI and should not be displayed back to the user.
