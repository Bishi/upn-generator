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

## Building Data

- 5 apartments, 12 occupants, 4 utility providers (5 bills/month)
- Providers: Elektro energija, JP VOKA SNAGA (×2), Energetika Ljubljana, ZLM
- Pre-configured provider templates live in DB, testable against `file-examples/`
