# Original Documents

> Bills page staged and retained PDF/image viewing

## Pre-conditions

- App is running with a disposable database and representative combined PDF and image sources.
- Include a combined PDF with at least one accepted and one rejected or unselected candidate.

## Cases

- [ ] In local and inbox preview, open supported accepted, rejected, and unselected staged sources; confirm unsupported/oversized items show an explicit unavailable reason.
- [ ] In the packaged Windows app, confirm a staged PDF opens through WebView2 at the candidate's `#page=N`, with toolbar page navigation, zoom, and search working.
- [ ] Close and reopen the PDF viewer, switch between documents, and confirm no stale document/page remains visible; verify Blob URLs are revoked on close, replacement, unmount, and failed load.
- [ ] Open a staged image, confirm fit behavior and bounded 25%-400% zoom, then close and reopen it.
- [ ] Open staged and stored `.tif`/`.tiff` originals and confirm the first TIFF image renders with the normal image zoom controls while retained/downloaded bytes remain unchanged.
- [ ] Open a malformed TIFF with a cyclic directory pointer and confirm the viewer reports a timeout without freezing the app; close or switch the viewer during TIFF decoding and confirm the worker is terminated without a stale result.
- [ ] From an inbox source viewer, press Escape once and confirm only the viewer closes; confirm the drawer, selections, and exception notes remain intact.
- [ ] Import one candidate from a mixed combined PDF, remove the user's source file, restart the app, and confirm View original still opens the exact full PDF while only imported bills have links.
- [ ] Import the same exact source for another bill/month and confirm storage is deduplicated while both bills can view it.
- [ ] Confirm different files with identical parsed fields remain distinct retained documents.
- [ ] Deselect every candidate or cancel preview and confirm no source document is retained.
- [ ] Close a billing month and confirm View original remains available without reopening the month.
- [ ] Confirm manual, legacy, and pre-feature-restored bills display Original unavailable and offer no attachment/replacement control.
- [ ] Delete one of two bills sharing a source and confirm the remaining bill still views it; delete the last linked bill/period and confirm the source is cleaned up without a normal-delete VACUUM.
- [ ] Factory reset a disposable database and confirm all originals are cleared; if compaction fails, confirm the app reports cleanup trouble without claiming the logical reset rolled back.
- [ ] Exercise forged/expired local handles and inbox session/candidate IDs, explicit close/cancel, successful finalization, inactivity expiry, and hard expiry; confirm reads fail safely and inbox temp bytes are removed.

## Notes

Exact source-file retention includes every page in a combined source once any candidate is imported. Cases remain unchecked until personally verified on the current packaged build.
