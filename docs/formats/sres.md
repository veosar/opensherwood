# SRES resource archive (`.res`, `.RES`)

Status: container **verified** for all three retail archives (`Interface/DEFAULT.RES`, `Text/actors.res`,
`<lang>/data/Text/Level.res`). `BTTN` item layout partial. Trailer partial.

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
| `BTTN` | `count: u32` (observed 14), then image blobs; exact semantics (button states) to confirm |
| `TEXT` | `count: u16`, then `count` strings, each `len: u16` + `len` UTF-16LE code units |
| `WAVE` | `count: u16`, then `count` strings, each `len: u16` + `len` bytes (Latin-1 path relative to `Data\Sounds`) |

Ids are global constants used by the game: `Level.res` uses `1000000..1000507` (mission title / one-line goal /
briefing text triplets, briefing pictures 158x154, dialogue voice lists), `actors.res` uses `2000000..2000033`
(one 54-entry list of exclamation wavs per actor type), `DEFAULT.RES` uses small ids (UI).

`Level.res` TEXT entries with three strings are `[title, short goal, briefing]`. Longer TEXT entries hold dialogues
and popup texts. `.red` files map per-level ids into this space (see [red.md](red.md)).

## Trailer

After the last entry: `n: u32` followed by `n` u32 values (12 in both files that have a trailer). The values are
increasing offsets within the file (e.g. 1808, 3548, ... in `actors.res`) and look like a seek index (one per N
entries). Unknown; not needed to read the archive sequentially.

## Provenance

Observation only (a Python parser walked every entry of all three files to the exact end of data and decompressed
every picture).
