# Sprite bank (`.rhs`, `robinhood.dic`, `robinhood.bks`)

Status: **stub**. This is the hardest and most important format; it is milestone M1's critical path.

## Files

- `DATA/Characters/*.rhs` (117 files, 500 B - 1 MB): one per character/object/relic/bonus type.
- `DATA/Animations/{Day,Night,Fog}/*.rhs` (116): map-specific animated elements (butterflies, cows, rivers, targets, flags).
- `DATA/robinhood.dic` (9.7 MB): dictionary. Same leading `u32` as `.rhs`.
- `DATA/robinhood.bks` (565 MB): bank of pixel data. Referenced by the executable as `Data\RobinHood.bks`/`.dic`;
  an error string says "... version. Recreating the archive." and another says an RHS "was not generated with the
  current bank", which suggests the `.rhs` files hold per-sprite metadata (frames, offsets, symbol references) and
  the bank holds shared pixel data addressed through the dictionary.

## `.rhs` header (observed)

| Offset | Type | Meaning |
|---|---|---|
| 0 | u32 | `0x0003EBC9` = 257,001: identical across all 233 RHS files and the DIC; a bank generation id, not a file-type magic |
| 4 | u16 | count (1 for most characters, 4-9 for animation sets) |
| 6 | char[32] | name, NUL padded, in French ("ACCESSOIRES Ale", "Manteau", "Croisement01 - papillon01") |
| 38 | ... | tables: many `0x96` (150) values, pairs like `(0xC3, 0x4582)`, dimensions such as 12x14, 44x43 |

## `.dic` (observed)

`C9 EB 03 00 86 00 00 10 ...` then dense 16-bit data. Values such as `0x07C0`, `0x1880`, `0x2008` recur, which
looks like 16-bit colour or packed run/skip codes rather than offsets.

## `.bks` (observed)

Runs of the 16-bit value `0x066D`, interrupted by other 16-bit values; no header magic. Sampled words never exceed
4095, which points to a 12-bit symbol stream stored as `u16` (dictionary indices into `.dic`), i.e. a
dictionary-coded sprite format ("a new system of compression for sprites" per the 2002 press material).

## Approach (see the `format-reverse-engineering` skill)

1. Statistics over `.bks`: word histograms, entropy per block, alignment search, search for the dimensions found in
   `.rhs` headers.
2. Correlate `.rhs` tables with offsets into `.bks`/`.dic` (monotonically increasing values below the bank size).
3. Decode a small `.rhs` (e.g. `ACCESSORIES_Coin.rhs`, 6x6) end to end before touching characters; render candidates
   as PNG locally and look at them.
4. Behavioural oracle: the original's console `STATUS FRAMECACHE` / `STATUS SHADOW` print sprite cache statistics.

## Provenance

Observation only so far.
