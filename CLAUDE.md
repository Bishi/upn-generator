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
- 🔲 **Phase 2** — Bill Import (PDF parsing, per-provider regex, store in DB)
- 🔲 **Phase 3** — UPN Generation (split by apartment, render authentic UPN slips)
- 🔲 **Phase 4** — Email Delivery + Security (SMTP send, keyring for password)

Current status: **v0.2.0 released. Phase 2 (Bill Import) next.**

See `README.md` for phase summary.

## Versioning & Releases

Use semantic versioning `MAJOR.MINOR.PATCH`:

- **Patch** `0.1.0 → 0.1.1` — bug fixes, small tweaks, copy changes
- **Minor** `0.1.x → 0.2.0` — new feature or considerable improvement
- **Major** `0.x.0 → 1.0.0` — breaking change or full milestone release

To release, bump the version in `src-tauri/tauri.conf.json`, commit, then tag:

```bash
git tag v0.2.0 && git push origin v0.2.0
```

This triggers the GitHub Actions workflow which builds the `.msi` and publishes it as a GitHub Release.

## Building Data

- 5 apartments, 12 occupants, 4 utility providers (5 bills/month)
- Providers: Elektro energija, JP VOKA SNAGA (×2), Energetika Ljubljana, ZLM
- Pre-configured provider templates live in DB, testable against `file-examples/`
