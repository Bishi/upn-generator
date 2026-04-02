# Status

- Current version: `0.4.3`
- Current tag: `v0.4.3`
- Release status: ready to be tagged and pushed to `origin`

## Latest Included Changes

- Added a default chimney-service provider based on `file-examples/dimnikar.jpg`
- Added `COST` purpose-code support for provider config and bill parsing
- Existing databases now backfill the chimney provider without requiring a reset
- `/` now redirects to Bills, and the dashboard landing page is removed from navigation
- Bills is now the primary landing page for the monthly workflow
- Added bill image import support so JPG/PNG/TIFF scans can be OCR'd and parsed like PDFs
- Prevented image import OCR from hanging the app indefinitely by running it with a timeout
