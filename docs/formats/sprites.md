# Sprite bank (`.rhs`, `robinhood.dic`, `robinhood.bks`)

Status: `.rhs` **verified** (all 233 files parse to the exact end and every frame reference resolves);
`.dic` header, dictionary pages and frame table **verified**; `.bks` pixel decoding **verified** for both
encodings (every one of the 404,855 streams is consumed exactly; rendered frames are recognisable sprites).
Open points: the meaning of the second key colour `0x001F` (drawn as a shadow?) and of the `.rhs` `unknown_*`
fields. Parsers: `crates/opensherwood-formats/src/{rhs.rs,dic.rs,sprite_decode.rs}`.

## Files

- `DATA/Characters/*.rhs` (117 files) and `DATA/Animations/{Day,Night,Fog}/*.rhs` (116): sprite *profiles*:
  named sequences of animations whose frames reference the global frame table.
- `DATA/robinhood.dic` (9,699,680 B): 134 dictionary pages followed by the global frame table.
- `DATA/robinhood.bks` (592,261,466 B): the pixel streams of all frames, back to back, in frame-table order.

All three start with the same `u32 0x0003EBC9`: a bank generation id (the executable reports an RHS that "was not
generated with the current bank"), not a file-type magic.

The organisation of the animation list (16-direction blocks per action, action ids, timing) is in
[sprite-animations.md](sprite-animations.md).

## `.rhs` profile

```
u32  bank_generation        (0x0003EBC9)
u16  sequence_count
sequence[sequence_count]:
    char[32] name           NUL padded, French ("Robin des bois", "ACCESSOIRES Piece d'or", "Croisement01 - papillon01")
    u16  animation_count
    u16  width, u16 height  bounding box of all frames
    u32  origin_x           150 for characters/objects, 0 or 70 for map animations: the canvas point that
    u32  origin_y           coincides with the entity position (150, 10, 140 ...); see "Placing frames" below
    animation[animation_count]:
        u16  frame_count
        u16  unknown_0x02   observed = frame_count - 1 (loop/key frame?)
        u32  unknown_0x04   150 / 161 / 171 ... or 0
        u32  unknown_0x08   150 / 136 / 137 ...
        u16  unknown_0x0c   0, 195, ...
        frame[frame_count]:
            u32  frame      index into the frame table
            u32  duration   ticks (1..15 observed)
            u16  anchor_x   hotspot inside the sequence box
            u16  anchor_y
            u16  unknown_0x0c   0; 414/421 on cart animations
```

Examples: `RobinHood.rhs` = 1 sequence "Robin des bois", 90x108, 2272 animations, 13,472 frame refs (actions x 8
directions x variants); `Child.rhs` = 736 animations; `ACCESSORIES_Coin.rhs` = 48 one-frame animations of a 6x6 coin
(probably one per ground orientation); `Cr01fx.rhs` = 4 map animations (butterflies, woodpecker) with 66-99 frame
loops. Idle animations use ping-pong frame orders (a, a+1, a+2, a+3, a+2, a+1) with durations 6,2,2,15,4,...

Across all profiles: 962,305 frame references, 404,807 unique frame indices, range 0..404854.

## Placing frames

A frame is drawn with its top-left corner at `(entity_x - origin_x + anchor_x, entity_y - origin_y + anchor_y)`.
For characters the canvas is 300x300 with the entity position at its centre (150,150): the 6x5 coin has anchors
(147,148), i.e. it is centred on its position; Robin's idle frames (36x69, anchors (133,87)) put the feet a few
pixels below the position. Verified by rendering Robin and a soldier on the Sherwood background at known map
positions with the engine (`harness/tools/drive.py --scenario map:sherwood`): with this rule the sprites stand
on their positions; with anchors taken as offsets from the position they land 150 pixels up-left.

## `.dic` layout

```
u32  bank_generation                 0x0003EBC9
u16  page_count                      134
page[page_count]:
    u16  entry_count                 4096 for most pages; 16..2748 for 18 pages (see below)
    entry[entry_count]:
        u16[4] pixels                one horizontal run of 4 RGB565 pixels (8 bytes)
u32  frame_count                     404855 (= number of records below)
frame[frame_count]:                  14-byte records, to the exact end of the file
    u16  width
    u16  height
    u32  offset                      byte offset of the frame's stream in .bks
    u32  length                      byte length of the stream (always even)
    u16  page                        dictionary page 0..133, or 0xFFFF (span encoding, 10,134 frames)
```

Size check: `4 + 2 + 134*2 + 503,929*8 + 4 + 404,855*14 = 9,699,680`, the file size. The third header word (4096)
that `dic.rs` still calls `symbols_per_page` is simply page 0's `entry_count`; the field name should be updated
when the parser is next touched.

Page entry counts other than 4096: pages 22..26 (2568, 2649, 2285, 2748, 2402), 30, 31 (2048), 44 (16),
45..47 (801, 971, 1233), 49..53 (992, 806, 1318, 2350, 1519), 55, 56 (888, 1149). In 130 pages every entry is
referenced by at least one stream; pages 26, 47, 50 and 51 have 3, 1, 2 and 1 unreferenced trailing entries.

`offset[i] + length[i] == offset[i+1]` for every record and the last record ends exactly at the `.bks` size.
Pages are assigned in increasing frame order (page 0 = frames 0..270, page 1 = 271..453, ...): a page is a
codebook trained on a consecutive group of frames. Frame 0 is a 4x1 placeholder of one symbol (four transparent
pixels).

## Pixel data (`.bks` streams)

Pixels are RGB565 little-endian words, as in `docs/formats/image-blob.md`. Two colours are keys:

| Value | Meaning |
|---|---|
| `0x07C0` | transparent (bright green). 17.3% of dictionary pixels, ~56% of span-frame pixels including the area outside spans. |
| `0x001F` | second key (pure blue): 10.5% of dictionary pixels, found where a drop shadow would lie (under a character's feet, next to the coin). Inferred: the original draws it as a translucent shadow; **unverified**. |

`0x07E0` (pure green) occurs 4 times in 2 million dictionary pixels and is *not* a key; `0xF81F` and `0xFFFF`
never occur.

### Dictionary-page frames (`page != 0xFFFF`, 394,721 frames)

The stream is exactly `ceil(width / 4) * height` little-endian `u16` symbols. Symbol `s` of row `y` expands to
`page.entry[s]`, four pixels left to right; the last symbol of a row is truncated to the frame width (rows are
padded to a multiple of 4 pixels in the encoded form). There are no row markers and no escape codes; every
symbol is `< entry_count` of its page. The most frequent symbol of a page is that page's all-transparent entry
(page 0: symbol 1645 = `0x066D`; the index differs per page). Only 478 of the 503,929 entries are four equal pixels,
so the codebook is a vector quantiser over 4x1 blocks, not a run-length table.

### Span frames (`page == 0xFFFF`, 10,134 frames, up to 674x583)

```
row[height]:
    u16  first_x
    u16  last_x                      inclusive; 0xFFFF = empty row (first_x is then always 0)
    u16  pixels[last_x - first_x + 1] RGB565, may contain the key colours
```

Pixels outside the span are transparent. Over the whole bank: 752,635 spans, 11,947 empty rows, 73,049 spans
covering the full width (those frames cost `2*w*h + 4*h` bytes), 5,073,299 key pixels inside spans. This is the
same row-span idea as the Desperados `.dvf` sprite rows (header of two `u16` with `0xFFFF` for an empty row, and
both `0x07C0` and `0x001F` skipped when blitting), except that here the second word is an inclusive last column.

### Rendered checks

Decoding with the rules above gives (looked at as PNG, not committed): frame 17970 (`ACCESSORIES_Coin.rhs`) a
6x5 gold coin with two `0x001F` pixels above it; frames 1097.. (`Cr01fx.rhs` "papillon01") a blue butterfly;
frame 1394 ("picvert01") a green woodpecker on a trunk; frame 286393 (`RobinHood.rhs` animation 0) a 36x69
archer in green with a bow, standing on a blue `0x001F` blob; frames 2603/2604 (span encoded, 419x363 and
117x133) a stone building and a stone wall.

## Provenance

Observation only; no executable analysis. Scripts under `harness/tools/re/` (`spritebank.py` loader;
`sprite_stats1..8.py`; `sprite_render.py`), run with `OPENSHERWOOD_GAME_DIR` set:

- `sprite_stats2.py`: for all 394,721 page frames `length / 2 == ceil(width / 4) * height` (the alternatives
  `ceil(w/2)*ceil(h/2)` and `w*ceil(h/4)` fail on about half of the frames); per page, the largest symbol used
  plus one, summed over pages, times 8 bytes leaves 326 bytes of the region unexplained, i.e. entries are 8 bytes.
- `sprite_stats3.py` / `sprite_stats4.py`: walking the region as `u16 count` + `count * 8` from file offset 6
  reproduces the per-page maxima (4 pages have a few unreferenced entries) and leaves exactly one `u32`, whose
  value is the frame-table length. The most frequent symbol of pages 0, 1, 2, 131, 132, 133 maps to four `0x07C0`
  pixels only under this walk (off by 2 bytes per page under a header-less layout).
- `sprite_stats7.py`: the span rule consumes all 10,134 page-less streams to the exact byte; the count-based
  alternatives (`skip`, `count`, `count-1`) consume only 62 / 1,590 of them (`sprite_stats6.py`).
- `sprite_stats8.py`: colour histograms quoted above.
- `sprite_render.py`: the PNGs listed under "Rendered checks", inspected visually.
- Rust: `cargo test -p opensherwood-formats` decodes every 97th frame plus 500 span frames and checks the
  output size; `opensherwood-tools export-frame robinhood.dic robinhood.bks <index> out.png` reproduces the renders.
- Cross-check of shared concepts with the GPLv3 Desperados engine reimplementation
  <https://github.com/OpenDeathValley/OpenDeathValley> (`components/files/odv_dvf_handler.c`): row headers of two
  `u16`, `0xFFFF` empty rows, both key colours. Only the concepts were compared; no code was taken from it.
