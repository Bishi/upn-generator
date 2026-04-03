# Status

- Current version: `0.4.11`
- Current tag: `v0.4.11`
- Release status: ready to be tagged and pushed to `origin`

## Latest Included Changes

- Added a default chimney-service provider based on `file-examples/dimnikar.jpg`
- Added `COST` purpose-code support for provider config and bill parsing
- Existing databases now backfill the chimney provider without requiring a reset
- `/` now redirects to Bills, and the dashboard landing page is removed from navigation
- Bills is now the primary landing page for the monthly workflow
- Added bill image import support so JPG/PNG/TIFF scans can be OCR'd and parsed like PDFs
- Prevented image import OCR from hanging the app indefinitely by running it with a timeout
- Corrected JP VOKA split defaults and backfill: waste is by m2, water is by people
- Added an OCR-tolerant Dimnikar parser so noisy image imports extract the chimney-service bill correctly
- Hardened the Dimnikar OCR matcher against damaged text like corrupted bullets, dashes, and provider spelling
- Preserved the selected month when switching years, sorted year buttons ascending, and stabilized scrollbar layout between tabs
- Marked fallback-parsed and failed-parsed bills as needing review, with visible yellow warnings in Bills and Splits
- Fixed a multi-bill import SQL placeholder mismatch that broke combined PDF imports
- Replaced the intrusive review warning rows with a small hoverable yellow indicator in Bills and Splits
- Fixed the Dimnikar OCR fallback so payment references no longer absorb IBAN digits
