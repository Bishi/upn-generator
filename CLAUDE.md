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
- Apartments store both a display name (`label`) and a cadastral/unit code (`unit_code`)
- Apartment `contact_email` remains the persisted field name, but now supports comma-separated recipients
- Provider split logic is configured per provider via `split_basis` (`occupants`, `m2_percentage`, or `equal_apartments`)
- Factory reset should reseed building/apartments/providers/SMTP defaults and clear periods/bills/splits

## Key Files

- `src-tauri/src/lib.rs` — Tauri setup, DB init, command registration
- `src-tauri/src/db/migrations.rs` — all CREATE TABLE statements
- `src-tauri/src/commands/config.rs` — all CRUD IPC commands + `DbState`
- `src/lib/types.ts` — TypeScript types mirroring Rust structs
- `src/lib/ipc.ts` — typed `invoke()` wrappers for all IPC commands
- `src/routes/settings.tsx` — Settings page (4 tabs)
- `src/components/settings/` — per-tab setting components
- `src-tauri/src/commands/bills.rs` — bill import, PDF/image text extraction + parsing, billing period commands
- `src-tauri/src/commands/splits.rs` — split calculation logic
- `src-tauri/src/commands/upn.rs` — UPN QR form rendering (official ZBS layout), email sending
- `src/routes/bills.tsx` — Bills page (year/month UI, import, manual entry)
- `src/routes/splits.tsx` — Splits matrix page
- `src/routes/upn.tsx` — UPN preview + send page (system PDF opener with visible preview errors)
- `/` redirects to Bills; the dashboard landing page has been removed from navigation

## Dev Commands

```bash
npm run tauri dev      # dev with hot reload
npm run tauri build    # production build
npm run dev            # frontend only (no Tauri)
```

## UPN Forms

UPN output must follow the official ZBS UPN QR technical standard: 210 mm × 99 mm form size, the standard two-part layout, official field geometry, and Courier New-style machine print. Reference examples in `file-examples/` (1.PNG, 2.PNG, 3.PNG) and the ZBS technical standard / IzpisUPNQR documentation when adjusting layout.

## Plan Status — "UPN Generator - Apartment Bill Splitting App"

**→ CANONICAL PLAN:** `~/.claude/plans/linked-sprouting-reddy.md`
To reference in new sessions, use `EnterPlanMode` to load it.

- ✅ **Phase 1** — Scaffold + Settings UI (Tauri, DB, apartments/providers/SMTP config)
- ✅ **Phase 1.5** — UI polish: dark mode, seed data, bills page redesign, multi-bill PDF import
- ✅ **Phase 2** — Bill Import: smart 3-phase parser (UPN stubs + Elektro + ZLM), IBAN-based provider matching, PDF/image OCR import, manual entry, debug log
- ✅ **Phase 3** — UPN Generation: mixed split basis (occupants, m² percentage, or equal apartments), render official-style UPN QR PDFs via printpdf, preview + download + email send
- 🔲 **Phase 4** — Email Delivery + Security (SMTP send working; keyring for password storage pending)

Current status: **v0.4.8. Phases 2 + 3 largely complete, including provider-based split rules, equal apartment split support, chimney-service provider support, image-based bill OCR import, import timeout protection, corrected JP VOKA split defaults, OCR-tolerant Dimnikar parsing, stronger OCR normalization, review-state warnings for fallback parses, improved year/month navigation, and multi-recipient apartment emails. Phase 4 (email + keyring) next.**

## Documentation

After implementing a feature or completing a plan, always update documentation as needed:
- `CLAUDE.md` — update phase status, current version in tag example, any new key files or architecture decisions
- `README.md` — add completed phases, update feature list
- `STATUS.md` — update the current released version/tag and short release snapshot

## Versioning & Releases

Use semantic versioning `MAJOR.MINOR.PATCH`:

- **Patch** `0.1.0 → 0.1.1` — bug fixes, small tweaks, copy changes
- **Minor** `0.1.x → 0.2.0` — new feature or considerable improvement
- **Major** `0.x.0 → 1.0.0` — breaking change or full milestone release

To release, bump the version in `src-tauri/tauri.conf.json`, commit, then tag:

```bash
git tag v0.4.8 && git push origin main && git push origin v0.4.8
```

This triggers the GitHub Actions workflow which builds the `.msi` first and then publishes it in a separate GitHub Release upload step.

**IMPORTANT: Every push to `main` must be accompanied by a version bump and a tag.** Never push commits without also tagging. The pushed commit and the pushed `vX.Y.Z` tag must refer to the same release state. Steps every time:
1. Bump version in `src-tauri/tauri.conf.json`
2. Commit the version bump
3. Tag with `git tag vX.Y.Z`
4. Push both: `git push origin main && git push origin vX.Y.Z`

## Building Data

- 6 apartments, 12 occupants, 5 utility providers (5 bills/month)
- Pre-configured provider templates live in DB, testable against `file-examples/`

| Provider | Service | IBAN |
|---|---|---|
| Elektro energija d.o.o. | Electricity | SI56 0400 1004 8988 093 |
| JP VOKA SNAGA d.o.o. | Waste | SI56 0400 1004 9142 226 |
| JP VOKA SNAGA d.o.o. | Water | SI56 2900 0000 3057 588 |
| Energetika Ljubljana d.o.o. | Gas | SI56 0292 4025 3764 022 |
| ZLM d.o.o. | Cleaning | SI56 0201 1025 7890 131 |
