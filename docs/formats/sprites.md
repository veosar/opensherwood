# Sprite bank (`.rhs`, `robinhood.dic`, `robinhood.bks`)

Status: `.rhs` **verified** (all 233 files parse to the exact end and every frame reference resolves);
`.dic` frame table **verified**; dictionary region and `.bks` symbol streams **stub** (pixel decoding unknown).
This is milestone M1's critical path.

## Files

- `DATA/Characters/*.rhs` (117 files) and `DATA/Animations/{Day,Night,Fog}/*.rhs` (116): sprite *profiles*:
  named sequences of animations whose frames reference the global frame table.
- `DATA/robinhood.dic` (9,699,680 B): a dictionary region followed by the global frame table.
- `DATA/robinhood.bks` (592,261,466 B): the concatenated symbol streams of all frames, in frame-table order.

All three start with the same `u32 0x0003EBC9`: a bank generation id (the executable reports an RHS that "was not
generated with the current bank"), not a file-type magic.

## `.rhs` profile

```
u32  bank_generation        (0x0003EBC9)
u16  sequence_count
sequence[sequence_count]:
    char[32] name           NUL padded, French ("Robin des bois", "ACCESSOIRES Piece d'or", "Croisement01 - papillon01")
    u16  animation_count
    u16  width, u16 height  bounding box of all frames
    u32  unknown_0x26       150 for characters/objects, 0 or 70 for map animations
    u32  unknown_0x2a       150, 10, 140 ...
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

Examples: `RobinHood.rhs` = 1 sequence "Robin des bois", 90x108, 2048 animations, 13,472 frame refs (actions x 8
directions x variants); `Child.rhs` = 736 animations; `ACCESSORIES_Coin.rhs` = 48 one-frame animations of a 6x6 coin
(probably one per ground orientation); `Cr01fx.rhs` = 4 map animations (butterflies, woodpecker) with 66-99 frame
loops. Idle animations use ping-pong frame orders (a, a+1, a+2, a+3, a+2, a+1) with durations 6,2,2,15,4,...

Across all profiles: 962,305 frame references, 404,807 unique frame indices, range 0..404854.

## `.dic` layout

```
u32  bank_generation
u16  page_count             134
u16  symbols_per_page       4096
u8[] dictionary_region      4,031,702 bytes, undecoded (see below)
frame[404855]:              located from the end of file; records chain contiguously through .bks
    u16  width
    u16  height
    u32  offset             byte offset of the symbol stream in .bks
    u32  length             byte length of the stream (always even; u16 symbols)
    u16  page               dictionary page 0..133, or 0xFFFF for 10,134 large frames (up to 419x363)
```

`offset[i] + length[i] == offset[i+1]` for every record and the last record ends exactly at the `.bks` size, so the
bank is nothing but the streams back to back. Frame 0 is a 4x1 placeholder of 2 bytes. Pages are assigned in
increasing frame order (page 0 = frames 0..270, page 1 = 271..453, ...), i.e. a page is a dictionary trained on a
consecutive group of frames. Frames with page `0xFFFF` are the largest ones and use a different or no dictionary.

## `.bks` symbol streams (observed, undecoded)

- Every `u16` in the sampled streams is `< 4096`, matching `symbols_per_page`.
- Symbol `0x066D` (1645) makes up ~63% of the first megabytes: probably "transparent run" or the most common block.
- Pixels per symbol vary per frame (1.7 .. 10), so a symbol expands to a variable-length run of pixels, not a
  fixed block: a dictionary of pixel runs (LZ78/VQ-like) with per-page codebooks of 4096 entries.
- The dictionary region starts with 16-bit values that look like RGB565 colours (`0x07C0`, i.e. bright green,
  recurs and is probably the transparent key), but the region is not a whole number of fixed-size pages
  (4,031,702 / 134 is not an integer), so pages are variable-length and there must be an index or per-entry
  lengths still to be found.

## Approach (see the `format-investigation` skill)

1. Find page boundaries in the dictionary region (search for a page index table: 134 monotonically increasing u32s,
   or per-page symbol length tables of 4096 small values).
2. Take the smallest frames (6x6 coin, 4x1 frame 0, 12x12 butterflies), decode candidate expansions and render them.
3. Statistics of symbol streams per frame: run lengths, whether streams start/end with fixed symbols (row markers).
4. Behavioural cross-check with the original's `STATUS FRAMECACHE` output.

## Provenance

Observation only (Python and Rust parsers over all 233 `.rhs` files and the `.dic`; cross-reference of frame
indices between `.rhs` and the table; statistics of `.bks` samples).
