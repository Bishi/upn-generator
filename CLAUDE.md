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
- Main DB lives at `%APPDATA%\si.upn-generator\upn-generator.db`
- Manual backups are user-chosen `.sqlite3` SQLite snapshots created from the live DB
- Manual backups intentionally blank `smtp_config.password`
- `building` table always has exactly 1 row (`id=1`)
- `smtp_config` table always has exactly 1 row (`id=1`)
- Apartments store both a display name (`label`) and a cadastral/unit code (`unit_code`)
- Apartment `contact_email` remains the persisted field name and supports comma-separated recipients
- Provider split logic is configured per provider via `split_basis` (`occupants`, `m2_percentage`, or `equal_apartments`)
- Factory reset reseeds building/apartments/providers/SMTP defaults and clears periods/bills/splits

## Key Files

- `src-tauri/src/lib.rs` - Tauri setup, DB init, command registration
- `src-tauri/src/db/migrations.rs` - all `CREATE TABLE` statements
- `src-tauri/src/commands/config.rs` - CRUD IPC commands plus `DbState`
- `src-tauri/src/commands/backup.rs` - manual DB backup and restore commands
- `src/lib/types.ts` - TypeScript types mirroring Rust structs
- `src/lib/ipc.ts` - typed `invoke()` wrappers for all IPC commands
- `src/routes/settings.tsx` - Settings page (5 tabs, including Data backup/restore)
- `src/components/settings/` - per-tab setting components
- `src-tauri/src/commands/bills.rs` - bill import, PDF/image text extraction and parsing, billing period commands
- `src-tauri/src/commands/splits.rs` - split calculation logic
- `src-tauri/src/commands/upn.rs` - UPN QR form rendering, preview, save, and email sending
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
- Phase 4 next - Email delivery + security hardening (SMTP send works; keyring for password storage is still pending)

Current status: **v0.4.14. Phases 2 and 3 are largely complete, including provider-based split rules, equal apartment split support, chimney-service provider support, OCR image import, timeout protection, improved OCR normalization, review-state warnings, year/month navigation improvements, multi-bill import stability fixes, corrected Dimnikar OCR confidence checks, richer import debug logging, multi-recipient apartment emails, and a manual SQLite backup/restore workflow.**

## Documentation

After implementing a feature or completing a plan, update docs as needed:

- `CLAUDE.md` - phase status, architecture decisions, key files
- `README.md` - user-facing features and workflows
- `STATUS.md` - current released version/tag and release snapshot when preparing a release

## Versioning & Releases

Use semantic versioning `MAJOR.MINOR.PATCH`:

- Patch: bug fixes, small tweaks, copy changes
- Minor: new feature or considerable improvement
- Major: breaking change or full milestone release

To release, bump the version in `src-tauri/tauri.conf.json`, commit, then tag:

```bash
git tag v0.4.14 && git push origin main && git push origin v0.4.14
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
