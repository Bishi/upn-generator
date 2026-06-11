# UPN Generator

Tauri desktop app (Windows) for splitting apartment utility bills and generating UPN payment slips. Replaces manual Minimax workflow for the building accountant at Kamniska ulica 36, Ljubljana.

## Tech Stack

- **Frontend:** React + TypeScript, TanStack Router (file-based), Tailwind v4 via `@tailwindcss/vite`
- **Backend:** Rust + Tauri v2, `rusqlite` (bundled) for local SQLite DB
- **Build:** Vite + `@tauri-apps/cli`

## Key Architecture Decisions

- `rusqlite` (bundled) is used directly - no `tauri-plugin-sql`
- TanStack Router route tree is auto-generated at `src/routeTree.gen.ts` - do not edit manually
- No `tailwind.config.js` - Tailwind v4 config is inline via CSS
- Theme colors are tokenized in `src/index.css`; refined is the CSS fallback/default theme, and the selected theme is stored in the SQLite `app_settings` singleton row so it is included in backups
- Main DB lives at `%APPDATA%\si.upn-generator\upn-generator.db`
- Manual backups are user-chosen `.sqlite3` SQLite snapshots created from the live DB
- Manual backups intentionally blank `smtp_config.password` and `inbox_config.password`
- SMTP and IMAP passwords are stored in Windows Credential Manager under stable app targets and matched against the configured username before use; legacy DB password columns remain only for schema compatibility and are cleared after successful credential writes
- `building` table always has exactly 1 row (`id=1`)
- `smtp_config` table always has exactly 1 row (`id=1`)
- SMTP email safety is stored on `smtp_config`; the recipient allowlist defaults enabled and empty so bulk UPN sends are blocked until test recipients are listed or the allowlist is disabled
- `app_settings` table always has exactly 1 row (`id=1`) for database-backed UI preferences such as theme
- `inbox_config` table always has exactly 1 row (`id=1`) for manual IMAP inbox import settings
- Inbox imports are manual, read-only IMAP scans using `EXAMINE` and `BODY.PEEK`; attachments are parsed from temporary files and raw extracted text is not persisted for inbox imports
- Apartments store both a display name (`label`) and a cadastral/unit code (`unit_code`)
- Apartment `contact_email` remains the persisted field name and supports comma-separated recipients
- UPN email delivery history is stored per recipient in `upn_delivery_events`; SMTP and inbox passwords are write-only settings backed by Windows Credential Manager and excluded from backups
- Provider split logic is configured per provider via `split_basis` (`occupants`, `m2_percentage`, or `equal_apartments`)
- Factory reset reseeds building/apartments/providers/SMTP defaults and clears periods/bills/splits/delivery history

## Key Files

- `src-tauri/src/lib.rs` - Tauri setup, DB init, command registration
- `src-tauri/src/credentials.rs` - Windows Credential Manager access and shared mail password resolver
- `src-tauri/src/db/migrations.rs` - all `CREATE TABLE` statements
- `src-tauri/src/commands/config.rs` - CRUD IPC commands plus `DbState`
- `src-tauri/src/commands/backup.rs` - manual DB backup and restore commands
- `src/lib/types.ts` - TypeScript types mirroring Rust structs
- `src/lib/ipc.ts` - typed `invoke()` wrappers for all IPC commands
- `src/routes/settings.tsx` - Settings page (5 horizontal tabs; Delivery contains Email/Inbox, App contains Appearance/Data)
- `src/components/settings/` - per-tab setting components
- `src/lib/theme.tsx` - database-backed theme preference runtime
- `src-tauri/src/commands/bills.rs` - bill import, PDF/image text extraction and parsing, billing period commands
- `src-tauri/src/commands/inbox.rs` - read-only IMAP inbox configuration, connection test, and attachment import commands
- `src-tauri/src/commands/splits.rs` - split calculation logic
- `src-tauri/src/commands/upn.rs` - UPN QR form rendering, preview, save, email safety, delivery history, SMTP test, and email sending
- `src/routes/bills.tsx` - Bills page
- `src/routes/splits.tsx` - Splits matrix page
- `src/routes/upn.tsx` - UPN preview and send page

## Dev Commands

```bash
npm run tauri dev
npm run tauri build
npm run dev
```

## UPN Forms

UPN output must follow the official ZBS UPN QR technical standard: 210 mm x 99 mm form size, the standard two-part layout, official field geometry, and Courier New-style machine print. Use the examples in `file-examples/` as the visual reference.

## Plan Status - "UPN Generator - Apartment Bill Splitting App"

**Canonical plan:** `~/.claude/plans/linked-sprouting-reddy.md`

- Phase 1 complete - Scaffold + Settings UI
- Phase 1.5 complete - UI polish, seed data, bills page redesign, multi-bill PDF import
- Phase 2 complete - Bill import with parser pipeline, OCR image import, manual entry, debug log
- Phase 3 complete - UPN generation with mixed split basis, PDF render, preview, download, and email send
- Phase 4 in progress - Email delivery, manual IMAP inbox import, and security hardening (SMTP send, read-only inbox import, and Windows Credential Manager password storage work)

Current status: **v0.5.0. Phases 2 and 3 are largely complete, with Phase 4 in progress. The app includes provider-based split rules, equal apartment split support, chimney-service provider support, OCR image import, timeout protection, improved OCR normalization, review-state warnings, year/month navigation improvements, multi-bill import stability fixes, corrected Dimnikar OCR confidence checks, richer manual-import debug logging, guarded multi-recipient apartment emails with persisted delivery history, a manual SQLite backup/restore workflow, manual read-only inbox attachment import, and Windows Credential Manager storage for mail passwords.**

## Documentation

After implementing a feature or completing a plan, update docs as needed:

- `CLAUDE.md` - phase status, architecture decisions, key files
- `README.md` - user-facing features and workflows
- `STATUS.md` - current released version/tag and release snapshot when preparing a release

## Workflow Rules

### Code Reviews

When asked to review a commit, uncommitted changes, a branch, or a PR, treat it as a code-reading task unless the user explicitly asks for verification. Do not run lint, typecheck, tests, or builds for a review by default.

Review findings must be concrete defects grounded in source or diff context. If the concern is only "verify this", "check whether that", or "consider maybe", trace it to a yes/no defect before reporting it, or leave it out.

### Understand Before Building

For credentials, imports, exports, backups, settings, privacy-sensitive data, new database storage, and security-sensitive behavior, answer these before writing code:

1. What is this feature for?
2. Is "off" or "not configured" a valid state?
3. What data is stored, exported, backed up, restored, or intentionally excluded?
4. What constraints might be missing from the initial request?

If the answers are unclear and the choice would change data handling, security posture, or user workflow, stop and ask before implementing.

### Deferred Work

If a plan intentionally defers something to a later phase, call it out clearly before implementation. State what is not being built, why it is deferred, and what decision or request would bring it into scope.

### Verification

Use targeted checks while iterating, then run broader checks before calling work complete. Match verification to the risk of the change:

- Rust command, database, parsing, backup, or import changes: run `cargo check`; run relevant `cargo test` when behavior or validation logic changed.
- Frontend or IPC shape changes: run `npm.cmd run build`.
- Documentation-only changes: no build is required unless the docs describe behavior that should be verified.

In PowerShell, use `npm.cmd ...` instead of bare `npm ...`; bare `npm` can resolve to `npm.ps1` and fail under Windows execution policy.

### Implementation Checklists

- Rust IPC changes must keep command registration, Rust types, `src/lib/ipc.ts`, and `src/lib/types.ts` in sync.
- Database schema changes must check migrations, backup/restore behavior, and factory reset behavior.
- UPN rendering changes must remain visually close to real Slovenian bank UPN forms and should be compared with `file-examples/`.

## Versioning & Releases

Use semantic versioning `MAJOR.MINOR.PATCH`:

- Patch: bug fixes, small tweaks, copy changes
- Minor: new feature or considerable improvement
- Major: breaking change or full milestone release

To release, bump the version in `src-tauri/tauri.conf.json`, commit, then tag:

```bash
git tag v0.5.0 && git push origin main && git push origin v0.5.0
```

Every push to `main` must be accompanied by a version bump and a tag. The pushed commit and the pushed `vX.Y.Z` tag must refer to the same release state.

Release steps every time:

1. Bump version in `src-tauri/tauri.conf.json`
2. Commit the version bump
3. Tag with `git tag vX.Y.Z`
4. Push both: `git push origin main && git push origin vX.Y.Z`

## Building Data

- 6 apartments
- 12 occupants
- 5 recurring utility providers per month
- Pre-configured provider templates live in the DB and are testable against `file-examples/`

| Provider | Service | IBAN |
|---|---|---|
| Elektro energija d.o.o. | Electricity | SI56 0400 1004 8988 093 |
| JP VOKA SNAGA d.o.o. | Waste | SI56 0400 1004 9142 226 |
| JP VOKA SNAGA d.o.o. | Water | SI56 2900 0000 3057 588 |
| Energetika Ljubljana d.o.o. | Gas | SI56 0292 4025 3764 022 |
| ZLM d.o.o. | Cleaning | SI56 0201 1025 7890 131 |
