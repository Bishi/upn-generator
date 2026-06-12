# UPN Generator - User Manual

Desktop application for the building manager at Kamniska ulica 36, Ljubljana.

Each month: import the combined utility bill PDF, a photographed/scanned image, or bill attachments from an email inbox, split costs across apartments, and generate UPN payment slips for each tenant.

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

### Settings -> Delivery -> Email (SMTP)

Enter your outgoing mail server credentials so the app can send UPN slips to tenants. Gmail server settings are prefilled by default:

| Field | Value |
|-------|-------|
| Server | `smtp.gmail.com` |
| Port | `587` |
| Username | Your Gmail address |
| Password | App password (not your regular Gmail password) |
| Security | STARTTLS on port `587` or TLS on port `465` |

Gmail note: you must create an **App Password** in your Google Account security settings. Your regular Gmail password will not work.

The default database stores the SMTP server, port, TLS setting, `kamniska.racuni@gmail.com` as the username/from address, and the same address in the test-recipient allowlist. Enter the app password before sending real email.

SMTP passwords are saved in Windows Credential Manager for the current Windows user. The password field is write-only: leave it blank to keep the saved password, or enter a new app password to replace it.

Use **Email safety** while testing SMTP delivery. The recipient allowlist is enabled by default and starts with `kamniska.racuni@gmail.com` as the allowed test recipient, so **Send Emails** remains limited until you add more allowed recipients or turn the allowlist off. **Test Email** sends a small real email to the test recipient using the current form values; when the allowlist is enabled, that test recipient must also be listed.

### Settings -> Delivery -> Inbox

Enter your incoming IMAP mailbox settings so the app can manually import bill attachments from email. Gmail server settings are prefilled by default:

| Field | Value |
|-------|-------|
| Server | `imap.gmail.com` |
| Port | `993` |
| Username | Your Gmail address |
| Password | App password (not your regular Gmail password) |
| Folder | `INBOX` |
| TLS | Enabled |

Use **Sender allowlist** to limit imports to known bill senders. The app reads the mailbox in read-only mode and does not mark messages as read, move messages, or delete mail. On the Bills page, **Import from Inbox** opens a preview step where you can override the scan window for that run, inspect parsed attachments, and import only selected ready candidates. A scan window of 0 means today only; higher values include today plus earlier calendar dates. Previewing does not create bills or inbox audit rows; imported attachments must still match the selected billing month and a configured provider that is still missing for that month.

The default database stores the IMAP server, port, TLS setting, folder, scan window, `kamniska.racuni@gmail.com` as the username, and an empty sender allowlist. Enter the app password before importing from the inbox.

IMAP passwords are saved in Windows Credential Manager for the current Windows user. The password field is write-only and is never loaded back into the form.

### Settings -> App -> Appearance

Choose the app theme. **Refined** is the default and the polished production direction. The selected theme is saved in the app database and included in manual backups.

### Settings -> App -> Data

Use **Create Backup** to save a manual backup of the app data to any folder you choose. The backup is stored as a `.sqlite3` SQLite file and includes building settings, apartments, providers, billing periods, bills, splits, and the selected appearance theme.

Use **Restore Backup** to replace the current app data with a previously saved backup. For safety, saved SMTP and inbox passwords are not included in backups. Existing Windows Credential Manager passwords are kept and reused only when the restored username still matches; otherwise enter the password again in **Settings -> Delivery**.

---

## Monthly Workflow

### Step 1 - Select a billing month

Go to the **Bills** page.

Use the month picker to choose the billing month. Years and months can be browsed directly; the app creates the underlying billing period only when you import or add bills for a month.

### Step 2 - Import bills

Select the billing month and click **Import Bills** to choose a local file, or click **Import from Inbox** to scan the configured mailbox for bill attachments. For example, bills titled `02.2026` belong in the February 2026 billing month, even when the provider charges for January usage.

The app supports importing a single combined PDF or a supported image file (`.jpg`, `.jpeg`, `.png`, `.bmp`, `.tif`, `.tiff`). PDFs can contain all bills together; image imports are OCR'd on Windows before the same provider-detection pipeline runs.

Inbox import supports the same PDF and image attachment types. It scans recent messages only, skips messages and attachments that are too large, validates attachment type before parsing, skips attachments that do not clearly match the selected billing month, skips unknown providers, skips configured providers that already have a bill in that month, avoids duplicate attachments and duplicate parsed bill content by hash, and deletes its temporary attachment file when that attachment has finished importing.

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
| **Download All PDFs** | Saves one combined UPN packet PDF per apartment to a folder of your choice for review or external use |
| **Mark Delivered** | After a confirmation prompt, marks all current apartment UPN packets as delivered |
| **Send Emails** | Sends one combined apartment PDF to configured recipient addresses allowed by the current email safety settings |

UPN Preview keeps delivery history for the selected month. After reload, apartment rows can show sent email, manually delivered, failed, blocked, partial, or changed status based on current packet hashes and persisted delivery events. Downloaded PDFs do not mark a month delivered by themselves.

---

## Pages Overview

### Bills

Overview of all imported bills by year and month. Import PDFs, image scans, or inbox attachments, add manual entries, edit or delete rows.

### Splits

The split matrix: rows are bills, columns are apartments. Shows how much each apartment owes for each bill in the selected month. Values can be manually adjusted.

### UPN

Generate and distribute UPN payment slips. Each apartment card shows the total amount due and individual line items. Send emails or download PDFs from here.

### Settings

Five tabs for configuring the application:

- **Building** - Building address and contact details
- **Apartments** - List of apartments with names, unit codes, occupants, m2 percentages, and comma-separated email recipients
- **Providers** - Utility providers with IBANs, purpose text templates, and split basis rules (`People`, `m2`, or `Equal`)
- **Delivery** - SMTP settings for sending emails and IMAP settings for manual read-only bill attachment import
- **App** - Database-backed visual theme selector plus manual SQLite backup and restore

---

## Data & Privacy

All data is stored locally in a SQLite database at:

```text
%APPDATA%\si.upn-generator\upn-generator.db
```

Manual backups are saved wherever you choose as `.sqlite3` files. They contain app data, the selected appearance theme, inbox import history, and UPN email/manual delivery history, but intentionally exclude saved SMTP and inbox passwords. Mail passwords are stored in Windows Credential Manager and are matched to the configured username before use.

Nothing is sent to the cloud. Emails are sent directly via the SMTP server configured in Settings. Inbox imports connect directly to the IMAP server you configure, store only import metadata, and do not persist raw extracted text from inbox attachments.

---

## Troubleshooting

**Bill import does not find all bills**

A parse log is written on every import:

```text
%APPDATA%\si.upn-generator\import_debug.log
```

Open it to see the raw extracted text and what each detection phase found or missed. For local image imports, this log shows the OCR text that was parsed. For inbox imports, raw extracted text and detailed payment fields are redacted from the debug log.

**UPN preview does not open**

Make sure a PDF viewer is installed (for example Adobe Acrobat or Microsoft Edge). The eye button saves a temporary PDF and opens it with Windows' default PDF app. If the launch fails, the error is shown directly on the UPN page instead of failing silently.

**Email not sending**

Check the SMTP settings under **Settings -> Delivery**. For Gmail, you must use an **App Password** - your regular account password will be rejected.

If rows show **blocked**, the Email safety allowlist is enabled and the recipient is not listed. Add the test recipient to the allowlist or turn the allowlist off when you are ready to send to tenants.

**Inbox import not connecting**

Check the IMAP settings under **Settings -> Delivery**. For Gmail, use `imap.gmail.com`, port `993`, TLS enabled, and an **App Password**.
