# Fonts (`.bfn`, `.fnt`, `.tfn`, `manager.cfg`)

Status: **stub**.

- `Interface/Fonts/manager.cfg`: text file mapping logical font roles to files, e.g.
  `Tooltips : tooltips.bfn,` and `ListDefault : , ListDefault.tfn` (bitmap font in column 1, TrueType descriptor in
  column 2). Roles: Loading, Version, Titbits, PCPortrait, Tooltips, Default, MissionTitle, PopupScroll, Dialogue,
  ActiveShortBriefing, InactiveShortBriefing, ListDefault, ListFocused, ListSelected, MenuButtonEnabled,
  MenuButtonDisabled, InfoScroll, Debrief, EditField, MenuText.
- `.bfn` / `.fnt`: magic `SBFONT`, `u16 version = 2`, then `char[36]` face name ("Lydian"), then u32 metrics
  (size, height, ...) and a glyph table; glyph bitmaps follow. `dialog.fnt` (36 KB) is the same format.
- `.tfn`: magic `SBTTFT`, 90 bytes: version, name ("New font"), a TrueType face name ("SimSun"), size and style
  fields. `simsun.ttc` (10.5 MB) is shipped for CJK text.

## Provenance

Observation.
