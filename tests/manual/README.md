# Manual QA Checklists

Manual checks for UPN Generator flows that are not fully covered by automated tests.

## Convention

- One file per feature area, not one file per PR.
- Keep related sub-cases in the same file.
- Mark a case with `[x]` only after verifying it on the current build.
- Reset affected cases to `[ ]` when the feature changes.
- Add or update the checklist in the same commit/PR as the feature change.

## File Format

```markdown
# <Feature name>

> Affected routes / pages / components

## Pre-conditions
- Required app state, data, credentials, or files

## Cases
- [ ] Happy path
- [ ] Edge case
- [ ] Regression guard

## Notes
Known caveats, environment details, or sample files used.
```

## Index

| File | Covers |
| --- | --- |
| [bills-import.md](./bills-import.md) | Bills page PDF/image/manual import, provider detection, debug log, month matching |
| [inbox-import.md](./inbox-import.md) | Manual read-only IMAP scan, attachment preview, duplicate/allowlist handling |
| [splits.md](./splits.md) | Split calculation, provider split basis, manual split edits |
| [upn-preview-send.md](./upn-preview-send.md) | UPN preview, download, email sending, delivery history, email safety |
| [settings-delivery.md](./settings-delivery.md) | SMTP/IMAP settings, credential storage, test actions, allowlists |
| [backup-restore.md](./backup-restore.md) | Manual SQLite backup/restore, password exclusion, theme/data preservation |
| [factory-reset.md](./factory-reset.md) | Factory reset warnings, reseeded data, credential cleanup behavior |
| [upn-visual-reference.md](./upn-visual-reference.md) | UPN form size, field geometry, visual comparison with `file-examples/` |

