# UPN Generator

Tauri desktop app (Windows) for splitting apartment utility bills and generating UPN payment slips. Replaces manual Minimax workflow for building accountant at Kamniška ulica 36, Ljubljana.

## Tech Stack

- **Frontend:** React + TypeScript, TanStack Router (file-based), Tailwind v4 via `@tailwindcss/vite`
- **Backend:** Rust + Tauri v2, `rusqlite` (bundled) for local SQLite DB
- **Build:** Vite + `@tauri-apps/cli`

## Key Architecture Decisions

- `rusqlite` (bundled) used directly — no `tauri-plugin-sql`
- TanStack Router route tree auto-generated at `src/routeTree.gen.ts` — do not edit manually
- No `tailwind.config.js` — Tailwind v4 config is inline via CSS
- DB at `%APPDATA%\si.upn-generator\upn-generator.db`
- `building` table: always exactly 1 row (id=1)
- `smtp_config` table: always exactly 1 row (id=1)

## Key Files

- `src-tauri/src/lib.rs` — Tauri setup, DB init, command registration
- `src-tauri/src/db/migrations.rs` — all CREATE TABLE statements
- `src-tauri/src/commands/config.rs` — all CRUD IPC commands + `DbState`
- `src/lib/types.ts` — TypeScript types mirroring Rust structs
- `src/lib/ipc.ts` — typed `invoke()` wrappers for all IPC commands
- `src/routes/settings.tsx` — Settings page (4 tabs)
- `src/components/settings/` — per-tab setting components
- `src-tauri/src/commands/bills.rs` — bill import, PDF parsing, billing period commands
- `src-tauri/src/commands/splits.rs` — split calculation logic
- `src-tauri/src/commands/upn.rs` — UPN PDF rendering, email sending
- `src/routes/bills.tsx` — Bills page (year/month UI, import, manual entry)
- `src/routes/splits.tsx` — Splits matrix page
- `src/routes/upn.tsx` — UPN preview + send page

## Dev Commands

```bash
npm run tauri dev      # dev with hot reload
npm run tauri build    # production build
npm run dev            # frontend only (no Tauri)
```

## UPN Forms

UPN output must look as close to real Slovenian bank UPN forms as possible. Tenants receive these to pay bills — they must feel legitimate. Use official UPN template as background image with precisely positioned text overlay. Reference examples in `file-examples/` (1.PNG, 2.PNG, 3.PNG).

## Plan Status — "UPN Generator - Apartment Bill Splitting App"

**→ CANONICAL PLAN:** `~/.claude/plans/linked-sprouting-reddy.md`
To reference in new sessions, use `EnterPlanMode` to load it.

- ✅ **Phase 1** — Scaffold + Settings UI (Tauri, DB, apartments/providers/SMTP config)
- ✅ **Phase 1.5** — UI polish: dark mode, seed data, bills page redesign, multi-bill PDF import
- ✅ **Phase 2** — Bill Import: smart 3-phase PDF parser (UPN stubs + Elektro + ZLM), IBAN-based provider matching, manual entry, debug log
- ✅ **Phase 3** — UPN Generation: split by occupant ratio, render UPN PDFs via printpdf, preview + download + email send
- 🔲 **Phase 4** — Email Delivery + Security (SMTP send working; keyring for password storage pending)

Current status: **v0.2.7. Phases 2 + 3 largely complete. Phase 4 (email + keyring) next.**

## Documentation

After implementing a feature or completing a plan, always update documentation as needed:
- `CLAUDE.md` — update phase status, current version in tag example, any new key files or architecture decisions
- `README.md` — add completed phases, update feature list

## Versioning & Releases

Use semantic versioning `MAJOR.MINOR.PATCH`:

- **Patch** `0.1.0 → 0.1.1` — bug fixes, small tweaks, copy changes
- **Minor** `0.1.x → 0.2.0` — new feature or considerable improvement
- **Major** `0.x.0 → 1.0.0` — breaking change or full milestone release

To release, bump the version in `src-tauri/tauri.conf.json`, commit, then tag:

```bash
git tag v0.2.7 && git push origin main && git push origin v0.2.7
```

This triggers the GitHub Actions workflow which builds the `.msi` and publishes it as a GitHub Release.

**IMPORTANT: Every push to `main` must be accompanied by a version bump and a tag.** Never push commits without also tagging. Steps every time:
1. Bump version in `src-tauri/tauri.conf.json`
2. Commit the version bump
3. Tag with `git tag vX.Y.Z`
4. Push both: `git push origin main && git push origin vX.Y.Z`

## Building Data

- 5 apartments, 12 occupants, 4 utility providers (5 bills/month)
- Pre-configured provider templates live in DB, testable against `file-examples/`

| Provider | Service | IBAN |
|---|---|---|
| Elektro energija d.o.o. | Electricity | SI56 0400 1004 8988 093 |
| JP VOKA SNAGA d.o.o. | Waste | SI56 0400 1004 9142 226 |
| JP VOKA SNAGA d.o.o. | Water | SI56 2900 0000 3057 588 |
| Energetika Ljubljana d.o.o. | Gas | SI56 0292 4025 3764 022 |
| ZLM d.o.o. | Cleaning | SI56 0201 1025 7890 131 |
