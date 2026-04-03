# UPN Generator - User Manual

Desktop application for the building manager at Kamniska ulica 36, Ljubljana.

Each month: import the combined utility bill PDF or a photographed/scanned image, split costs across apartments, and generate UPN payment slips for each tenant.

The app opens on the **Bills** page, which is the main landing page for the monthly workflow.

---

## Installation

1. Download the latest `.msi` file from [Releases](https://github.com/Bishi/upn-generator/releases)
2. Run the installer and follow the prompts
3. Launch the app from the Start menu - **UPN Generator**

---

## First-Time Setup

On first launch, all providers, apartments, and building details are pre-configured for Kamniska ulica 36. Before first use, verify the data is correct.

### Settings -> Building

Check the building address and contact details. These appear on UPN slips as the payer address.

### Settings -> Apartments

All apartments are pre-configured. For each apartment, verify:

| Field | Description |
|-------|-------------|
| **Payer name** | Tenant or owner name as it will appear on the UPN slip |
| **Address / postal code / city** | Payer address on the UPN slip |
| **Email address(es)** | One or more recipients, separated by commas, for the combined apartment PDF |
| **Number of occupants** | Used for providers that split by people |
| **m2 percentage** | Used for providers that split by apartment square-meter share |
| **Unit code** | Extra apartment identifier such as `1287/6` |

### Settings -> Providers

Utility providers are pre-configured with the correct IBANs, payment purpose templates, and split basis. You normally do not need to change these.

| Provider | Service | IBAN |
|----------|---------|------|
| Elektro energija d.o.o. | Electricity | SI56 0400 1004 8988 093 |
| JP VOKA SNAGA d.o.o. | Waste collection | SI56 0400 1004 9142 226 |
| JP VOKA SNAGA d.o.o. | Water | SI56 2900 0000 3057 588 |
| Energetika Ljubljana d.o.o. | Gas | SI56 0292 4025 3764 022 |
| ZLM d.o.o. | Cleaning | SI56 0201 1025 7890 131 |
| Dimnikarstvo Energetski Servis d.o.o. | Chimney service | SI56 6100 0000 5243 585 |

### Settings -> Email (SMTP)

Enter your outgoing mail server credentials so the app can send UPN slips to tenants. Typical Gmail settings:

| Field | Value |
|-------|-------|
| Server | `smtp.gmail.com` |
| Port | `587` |
| Username | Your Gmail address |
| Password | App password (not your regular Gmail password) |
| TLS | Enabled |

Gmail note: you must create an **App Password** in your Google Account security settings. Your regular Gmail password will not work.

### Settings -> Data

Use **Create Backup** to save a manual backup of the app data to any folder you choose. The backup is stored as a `.sqlite3` SQLite file and includes building settings, apartments, providers, billing periods, bills, and splits.

Use **Restore Backup** to replace the current app data with a previously saved backup. For safety, the SMTP password is not included in backups, so after restore you must enter it again in **Settings -> Email** before sending emails.

---

## Monthly Workflow

### Step 1 - Create a billing period

Go to the **Bills** page.

- First time using a new year: click **Add Year**, enter the year (for example `2026`) - all 12 months are created at once.
- In subsequent months, the periods already exist; just select the correct month.

### Step 2 - Import bills

Select the month and click **Import Bills**.

The app supports importing a single combined PDF or a supported image file (`.jpg`, `.jpeg`, `.png`, `.bmp`, `.tif`, `.tiff`). PDFs can contain all bills together; image imports are OCR'd on Windows before the same provider-detection pipeline runs.

| Provider | Service | Detection method |
|----------|---------|-----------------|
| Elektro energija d.o.o. | Electricity | `ZA PLACILO Z DDV:` text |
| JP VOKA SNAGA d.o.o. | Waste / Water | UPN stub `***amount` |
| Energetika Ljubljana d.o.o. | Gas | UPN stub `***amount` |
| ZLM d.o.o. | Cleaning | `Za placilo EUR:` text |

After import, check the bills table: amount, reference, due date, and purpose should all be filled in correctly.

Manual entry: if a bill was not detected, click **Add manually** and enter the details yourself.

Editing: click the pencil icon on any row to correct a bill.

### Step 3 - Verify bills

The Bills page should show one row per provider with correct amounts. The total of all bills for the month is shown at the bottom.

### Step 4 - Calculate splits

Go to the **Splits** page and click **Recalculate**.

The app divides each bill using the provider's configured split basis. Providers can split by number of occupants, m2 share, or equally across active apartments. By default, water splits by occupants and all other seeded providers split by m2.

Individual amounts can be manually adjusted by clicking a cell.

### Step 5 - Preview and send UPN slips

Go to the **UPN** page and select the billing period.

Each apartment card shows its line items and the total amount due.

| Action | Description |
|--------|-------------|
| **Eye icon** | Generates the UPN PDF and opens it in your default PDF viewer |
| **Download All PDFs** | Saves all UPN slips to a folder of your choice |
| **Send Emails** | Sends one combined apartment PDF to all configured recipient addresses for that apartment |

---

## Pages Overview

### Bills

Overview of all imported bills by year and month. Import PDFs or image scans, add manual entries, edit or delete rows.

### Splits

The split matrix: rows are bills, columns are apartments. Shows how much each apartment owes for each bill in the selected month. Values can be manually adjusted.

### UPN

Generate and distribute UPN payment slips. Each apartment card shows the total amount due and individual line items. Send emails or download PDFs from here.

### Settings

Five tabs for configuring the application:

- **Building** - Building address and contact details
- **Apartments** - List of apartments with names, unit codes, occupants, m2 percentages, and comma-separated email recipients
- **Providers** - Utility providers with IBANs, purpose text templates, and split basis rules (`People`, `m2`, or `Equal`)
- **Email** - SMTP settings for sending emails
- **Data** - Manual SQLite backup and restore

---

## Data & Privacy

All data is stored locally in a SQLite database at:

```text
%APPDATA%\si.upn-generator\upn-generator.db
```

Manual backups are saved wherever you choose as `.sqlite3` files. They contain app data but intentionally exclude the saved SMTP password.

Nothing is sent to the cloud. Emails are sent directly via the SMTP server configured in Settings.

---

## Troubleshooting

**Bill import does not find all bills**

A parse log is written on every import:

```text
%APPDATA%\si.upn-generator\import_debug.log
```

Open it to see the raw extracted text and what each detection phase found or missed. For image imports, this log shows the OCR text that was parsed.

**UPN preview does not open**

Make sure a PDF viewer is installed (for example Adobe Acrobat or Microsoft Edge). The eye button saves a temporary PDF and opens it with Windows' default PDF app. If the launch fails, the error is shown directly on the UPN page instead of failing silently.

**Email not sending**

Check the SMTP settings under **Settings -> Email**. For Gmail, you must use an **App Password** - your regular account password will be rejected.
