# Settings Unsaved Changes

> Settings tabs, editable settings forms, discard confirmation

## Pre-conditions

- App is running with seeded or disposable test data.
- At least two apartments and two providers exist.

## Cases

- [ ] On Building, edit a field and confirm Save changes and Discard become enabled.
- [ ] Click Discard on Building, choose Keep editing, and confirm the edit remains.
- [ ] Click Discard on Building, choose Discard changes, and confirm the field resets.
- [ ] Edit Building, click another Settings tab, choose Stay, and confirm the tab does not change.
- [ ] Edit Building, click another Settings tab, choose Discard and switch, and confirm the edit resets before switching.
- [ ] On Delivery, edit both Inbox and Email settings, click another Settings tab, and confirm one dialog discards both dirty sections before switching.
- [ ] On Apartments, edit the selected apartment, click another apartment, choose Keep editing, and confirm the original draft remains selected.
- [ ] On Apartments, edit the selected apartment, click Add Apartment, choose Discard changes, and confirm the new apartment draft opens.
- [ ] On Providers, edit the selected provider, click another provider, choose Keep editing, and confirm the original draft remains selected.
- [ ] On Providers, edit the selected provider, click Add Provider, choose Discard changes, and confirm the new provider draft opens.

## Notes

Appearance changes save immediately and do not participate in the unsaved-changes guard.
