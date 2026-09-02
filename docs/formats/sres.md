# SRES resource archive (`.res`, `.RES`)

Status: container **verified** for all three retail archives (`Interface/DEFAULT.RES`, `Text/actors.res`,
`<lang>/data/Text/Level.res`), trailer verified. Which `BTTN` bit is which visual state is not yet confirmed.

## Header

| Offset | Type | Meaning |
|---|---|---|
| 0 | char[4] | `SRES` |
| 4 | u32 | version, `0x100` |
| 8 | u32 | entry count |
| 12 | ... | entries, back to back |
| end | trailer | see below |

## Entry

Every entry starts with `tag: char[4]`, `id: u32`, `zero: u32` (always 0 in retail data; purpose unknown, possibly
language or flags). The rest depends on the tag:

| Tag | Body |
|---|---|
| `PIC ` | one [image blob](image-blob.md) |
| `PICC` | `count: u32`, then `count` image blobs (an animation / icon set; e.g. id 6 = nine 32x32 frames) |
| `BTTN`, `RDO `, `NPTF`, `SLID` | UI widgets: `states: u32` bit mask, then one image blob per set bit in ascending bit order. Buttons use 14 (`0b1110`, 3 pictures) or 15 (4 pictures); radio buttons 0x7F (7 pictures); the input field and the slider 0x3F (6 pictures: five 424x39 backgrounds and an 8x28 caret for `NPTF`; knobs and a 23x1 track for `SLID`). Which bit is which visual state (normal / hover / pressed / disabled / checked...) is not confirmed |
| `CUR ` | `unknown_0x0c: u16` (2), `hotspot_x: u16`, `hotspot_y: u16`, `unknown_0x12: u16` (0 or 2), `count: u32`, then `count` image blobs: an animated mouse cursor (e.g. id 22 = 12 frames of 30x30) |
| `TEXT` | `count: u16`, then `count` strings, each `len: u16` + `len` UTF-16LE code units |
| `WAVE` | `count: u16`, then `count` strings, each `len: u16` + `len` bytes (Latin-1 path relative to `Data\Sounds`) |

`DEFAULT.RES` has no trailer (its `count` field says 292 while 292 entries were found; the file ends with the
last entry). `actors.res` and `Level.res` have the offset trailer described below.

Ids are global constants used by the game: `Level.res` uses `1000000..1000507` (mission title / one-line goal /
briefing text triplets, briefing pictures 158x154, dialogue voice lists), `actors.res` uses `2000000..2000033`
(one 54-entry list of exclamation wavs per actor type), `DEFAULT.RES` uses small ids (UI).

`Level.res` TEXT entries with three strings are `[title, short goal, briefing]`. Longer TEXT entries hold dialogues
and popup texts. `.red` files map per-level ids into this space (see [red.md](red.md)).

## Trailer

After the last entry (optional; absent in `DEFAULT.RES`): an offset table of `entry_count + 1` values of `u32`: the file offset of every entry in
order (the first is always 12, the end of the header) followed by the offset of the table itself, so that
`offset[i+1] - offset[i]` is the size of entry `i`. Sequential readers verify it; random-access readers use it.

## Provenance

Observation only (a Python parser walked every entry of all three files to the exact end of data and decompressed
every picture).
