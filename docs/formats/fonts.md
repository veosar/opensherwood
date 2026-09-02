# Fonts (`.bfn`, `.fnt`, `.tfn`, `manager.cfg`)

Status: **verified** (bitmap font container, glyph table and pixel layers; `.tfn` fields; `manager.cfg`).
The layout rule built from the two small per-glyph adjustments is **inferred** (see Unknowns).

All files live in `DATA/Interface/Fonts/`. Parser: `crates/opensherwood-formats/src/font.rs`. Tools:
`opensherwood-tools font-sheet <file> <out.png>` (all glyphs in a grid) and
`opensherwood-tools font-text <file> "<text>" <out.png>`. Probe: `harness/tools/re/font_probe.py`.

## Inventory

| File | Size | Face name | Glyphs | Strip (w x h) | Spacing |
|---|---|---|---|---|---|
| `Debrief.bfn` | 37,988 | Scroll | 161 | 3520 x 19 | 0 |
| `EditFields.bfn` | 28,280 | Scroll | 161 | 3520 x 22 | 1 |
| `InfoScroll.bfn` | 10,669 | Lydian | 162 | 2592 x 19 | -2 |
| `MenuButtonDisabled.bfn` | 24,061 | Scroll | 161 | 3520 x 22 | 1 |
| `MenuButtonEnabled.bfn` | 27,867 | Scroll | 161 | 3520 x 22 | 1 |
| `Scroll.bfn` | 36,932 | Scroll | 161 | 3520 x 18 | -1 |
| `ShortBriefingActive.bfn` | 4,259 | ShortBriefing | 161 | 1118 x 13 | 0 |
| `ShortBriefingInactive.bfn` | 4,327 | ShortBriefing | 161 | 1118 x 13 | 0 |
| `Title.bfn` | 56,059 | Title | 162 | 2144 x 29 | 0 |
| `tooltips.bfn` | 18,024 | Lydian | 162 | 2592 x 20 | -2 |
| `dialog.fnt` | 36,058 | Lydian | 162 | 4212 x 30 | -4 |

`.fnt` and `.bfn` are the same format. Sixteen `.tfn` descriptors (90 bytes each) and `simsun.ttc` (a stock
CJK TrueType collection, 10.5 MB) sit next to them.

## Bitmap font (`SBFONT`)

All integers little-endian.

### Header (70 bytes)

| Offset | Type | Field | Value in retail files |
|---|---|---|---|
| 0x00 | char[6] | magic | `SBFONT` |
| 0x06 | u32 | version | `0x200` (bytes `00 02 00 00`; the `.tfn` has `0x100`), read as 8.8 "2.0" |
| 0x0A | char[36] | name | NUL-padded face/project name: `Lydian`, `Scroll`, `Title`, `ShortBriefing` |
| 0x2E | u32 | `unknown_2e` | 2 for the three Lydian fonts, 0 otherwise |
| 0x32 | u32 | cell_height | equals the strip height (13 .. 30) |
| 0x36 | u32 | `unknown_36` | 0; 15 (`InfoScroll`), 25 (`dialog.fnt`) |
| 0x3A | u32 | `unknown_3a` | 11 .. 25; identical (13) for the four `Scroll`-face fonts of heights 18/19/22, so not a pixel metric of the strip |
| 0x3E | u32 | glyph_count | 161 or 162 |
| 0x42 | i32 | spacing | -4 .. 1; global advance adjustment (inferred) |

### Glyph table (`glyph_count` x 18 bytes at 0x46)

| Offset | Type | Field | Notes |
|---|---|---|---|
| 0 | u16 | code | Unicode scalar value; ascending in every file |
| 2 | u32 | x | first column of the glyph in both strips; monotonic |
| 6 | u32 | width | 0 .. 25; `x + width <= strip width` for every glyph |
| 10 | i32 | x_adjust | -2 .. 4; non-zero on narrow/tall glyphs (`!`, `1`, `|`, `^`, `` ` ``) and on swash letters of `Title` |
| 14 | i32 | advance_adjust | -8 .. 5; `Q` of the Scroll face is -8 (its tail runs under the next letter), `T`/`W`/`f` -2; the `Title` space has width 0 and +5 |

Code set: the 95 printable ASCII codes 0x20..0x7E, then 0xA1 0xA3 0xA4 0xA7 0xB0 0xB2 0xB5 0xBF, 0xC0..0xCF,
0xD1..0xD6, 0xD8..0xDC, 0xDF, 0xE0..0xEF, 0xF1..0xF6, 0xF8..0xFC, then U+0152 U+0153 (Œ œ) and U+2026 (…) = 161
glyphs. The 162-glyph fonts add U+20AC (€). These are the Windows-1252 characters mapped to Unicode, so the
table is indexed by the UTF-16 units of the SRES `TEXT` strings.

Cells: the `Scroll`-face fonts place glyphs on a fixed 22-pixel pitch (`x` = 0, 0, 22, 44, ...), Lydian on 16
(`tooltips`, `InfoScroll`) or 26 (`dialog.fnt`); `Title` and `ShortBriefing` pack glyphs at `x + width (+1)`.
Only `x` and `width` matter for decoding. The space record (0x20) is never meant to be drawn: in the
`Scroll`-face fonts it has `x = 0`, the same columns as `!` (rendering it shows a faint `!`), and in
`ShortBriefingInactive` it has `x = 75`, inside another glyph. A renderer must advance by the space record
and draw nothing. The `Scroll` face draws no accents on capitals (`À`..`Ï` look like `A`/`E`/`I`: the cell has
one row above cap height), which is a property of the artwork, not of the format.

### Pixel layers

Immediately after the table come two image blobs in the `docs/formats/image-blob.md` layout (`u16 w, u16 h,
u32 2 = bzip2, u32 size, stream`), both `w x h` RGB565 with `w x h x 2` decompressed bytes, and the second one
ends exactly at end of file:

1. **Colour layer**: the glyph colours/texture. Black (`0x0000`) where the glyph has no colour. Faces with a
   textured fill (`Scroll` family, `ShortBriefingInactive` vertical gradient, `ShortBriefingActive` one flat
   colour `0xFF93`) fill the whole strip; the mask carves the letters out of it.
2. **Coverage mask**: greyscale RGB565, `0x0000` = transparent, `0xFFDF` (31, 62, 31) = opaque, 16 .. 47
   intermediate greys per font (anti-aliased; the shared `EditFields`/`MenuButton` mask has 15 pixels that
   are off-grey by up to 2 units of blue and 4 of green); `ShortBriefingActive` is 1-bit (two values). For `Title`,
   `tooltips`, `InfoScroll`, `dialog.fnt` and the `MenuButton`/`EditFields` fonts the mask is dilated one or
   two pixels beyond the coloured shape, so blitting the colour layer with the mask as alpha produces a dark
   outline over any background. No colour key is involved.

Decoding a glyph: for `y in 0..h`, `x in glyph.x .. glyph.x + glyph.width`: `rgb = colour[y][x]` (RGB565),
`alpha = red5(mask[y][x]) * 255 / 31`.

### Layout rule (inferred)

`pen += x_adjust; draw at pen; pen += width + advance_adjust + spacing`. This is the reading that makes the
signs sensible (`Q` -8 tucks the next letter over its tail; the `Title` space advances by 5 with an empty
bitmap; the outlined `EditFields` mask is 1 px wider than the letter on each side and its glyphs carry -2).
Whether the original applies `x_adjust` to the pen or only to the blit position, and whether `spacing` is
added per glyph, is not verified against the original renderer.

## TrueType descriptor (`SBTTFT`, 90 bytes)

| Offset | Type | Field | Retail values |
|---|---|---|---|
| 0x00 | char[6] | magic | `SBTTFT` |
| 0x06 | u32 | version | `0x100` |
| 0x0A | char[36] | name | `New font` (all but the list fonts), `List Default` |
| 0x2E | u32 | `unknown_2e` | 1 for `Dialog`, `EditFields`, `MenuButton*`, `Title`, `Title_grand`; 0 otherwise (bold flag?) |
| 0x32 | u32 | size | 11 (`tooltips`), 12, 14, 15 (`List*`), 16, 18, 21 (`Dialog`), 23 (`Title`), 34 (`Title_grand`) |
| 0x36 | char[32] | face | `SimSun` (13 files) or `Arial` (the three `List*`); bytes after the NUL are junk (`U`, `a`) |
| 0x56 | u32 | colour | Windows `COLORREF` (`R, G, B, 0`): e.g. white for `ListSelected`, yellow for `ListFocused`, black for `tooltips_black`, dark red for `Title` |

The descriptors give the role's text colour and size for the TrueType fallback path (CJK locales use
`simsun.ttc`); the western locales draw with the bitmap fonts.

## `manager.cfg`

Text, one role per line: `Role<tabs>:<tabs><bitmap file>,<tabs><truetype file>`; either column may be empty.
20 roles: Loading, Version, Titbits, PCPortrait, Tooltips, Default, MissionTitle, PopupScroll, Dialogue,
ActiveShortBriefing, InactiveShortBriefing, ListDefault, ListFocused, ListSelected, MenuButtonEnabled,
MenuButtonDisabled, InfoScroll, Debrief, EditField, MenuText. Six roles share `tooltips.bfn`; `Loading` and
`Dialogue` name `Dialog.fnt` although the file is `dialog.fnt` (lookup must be case-insensitive); the three
`List*` roles have only a `.tfn`. `Title_grand.tfn` and `tooltips_black.tfn` are not referenced by the table.

## Unknowns

- `unknown_2e`, `unknown_36`, `unknown_3a` in the bitmap header and `unknown_2e` in the descriptor.
- Exact use of `x_adjust`, `advance_adjust` and `spacing` by the original renderer (see Layout rule).
- Vertical placement (baseline) is not stored explicitly; text lines are presumably stacked by `cell_height`.

## Provenance

Observation of the 11 bitmap fonts, 16 descriptors and `manager.cfg` with `harness/tools/re/font_probe.py`:

- Hexdumps of every header; the 18-byte record pitch from the code column (0x20 at 0x46, 0x21 at 0x58); the
  glyph count word at 0x3E matches the number of records before the first blob header in every file.
- Both blobs decompress (bzip2) to exactly `w x h x 2` bytes and the second ends at end of file for all 11 files;
  `x + width <= w` for every glyph; the strip height equals the header word at 0x32.
- Rendering the first 520 columns of the colour layer, the mask and their product for `Debrief`, `Title`,
  `tooltips`, `ShortBriefingActive`, `MenuButtonEnabled` at 2x: the ASCII run `!"#$%&'()*+,-./0123456789:;<=>?@AB…`
  is legible in the colour layer and in the mask, in the order of the glyph table. Row bounding boxes of
  `H`/`x`/`n`/`o` in the mask give consistent cap heights and x-heights per font.
- Mask value histograms: two values (`0`, `0xFFDF`) in `ShortBriefingActive`, greys of the form `(r, 2r, r)` in
  the others; the colour layer of `ShortBriefingActive` is one word, so the mask must carry the shape.
- `.tfn`: the last four bytes are black for `tooltips_black`, white for `ListSelected`, yellow (`ff ff 00`) for
  `ListFocused`; the size word grows from `tooltips` (11) to `Title_grand` (34).
