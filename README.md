# UPN Generator - Apartment Bill Splitting App

A Tauri desktop app (Windows) for the building accountant at **Kamniška ulica 36, Ljubljana**.

Replaces the manual Minimax workflow: import utility bills, split costs across 5 apartments, and generate authentic Slovenian UPN payment slips for each tenant.

---

## The Plan

### ✅ Phase 1 — Scaffold + Settings UI
- Tauri v2 + React + TypeScript + Tailwind v4 + TanStack Router
- SQLite DB via `rusqlite` (bundled)
- Settings page: Building info, Apartments (5), Providers (4), SMTP config
- Pre-configured provider templates (Elektro energija, JP VOKA SNAGA ×2, Energetika Ljubljana, ZLM)

### ✅ Phase 1.5 — UI Polish (v0.2.0)
- Dark mode (zinc-based theme)
- Seed data: building, 5 apartments, 5 providers pre-loaded on first run
- Bills page redesigned: year selector, month tabs, Add Year button
- Delete period confirmation modal (replaced broken `window.confirm`)
- Multi-bill PDF import: auto-splits one PDF into multiple bills by provider
- Manual bill entry without a PDF

### 🔲 Phase 2 — Bill Import
- Import monthly utility bills (PDF or structured data)
- Parse bill totals using per-provider regex templates
- Store imported bills in DB linked to provider + month

### 🔲 Phase 3 — UPN Generation
- Split each bill across apartments by occupancy share
- Generate authentic Slovenian UPN payment slip per apartment per bill
- UPN output uses official template as background with precisely positioned text overlay
- Reference examples: `file-examples/1.PNG`, `2.PNG`, `3.PNG`

### 🔲 Phase 4 — Email Delivery + Security
- Send generated UPN slips to tenants via SMTP
- Secure SMTP password using system keyring (`tauri-plugin-keyring`)

---

## Building Data

| | |
|---|---|
| **Address** | Kamniška ulica 36, 1000 Ljubljana |
| **Apartments** | 5 |
| **Occupants** | 12 |

**Monthly bills (5 bills from 4 providers):**

| Provider | Service | IBAN |
|---|---|---|
| Elektro energija d.o.o. | Electricity | SI56 0400 1004 8988 093 |
| JP VOKA SNAGA d.o.o. | Waste | SI56 0400 1004 9142 226 |
| JP VOKA SNAGA d.o.o. | Water | SI56 2900 0000 3057 588 |
| Energetika Ljubljana d.o.o. | Gas | SI56 0292 4025 3764 022 |
| ZLM d.o.o. | Cleaning | SI56 0201 1025 7890 131 |

---

## Tech Stack

- **Frontend:** React + TypeScript, TanStack Router (file-based), Tailwind v4
- **Backend:** Rust + Tauri v2, `rusqlite` (bundled) for local SQLite
- **Build:** Vite + `@tauri-apps/cli`

## Dev Commands

```bash
npm run tauri dev      # dev with hot reload
npm run tauri build    # production build
npm run dev            # frontend only (no Tauri)
```

## Releases

Download the latest `.msi` installer from [Releases](https://github.com/Bishi/upn-generator/releases).

Versioning: `PATCH` for fixes (`0.1.0 → 0.1.1`), `MINOR` for features (`0.1.x → 0.2.0`), `MAJOR` for milestones.
