# Image blob (`.map`, `.min`, `.pak`, `.sxt`, `*_t` thumbnails, SRES pictures)

Status: **verified** (container and pixel format). Pixels are RGB565: decoding `Levels/Day/sherwood.min` and the
Spellbound logo slide of `Slideshow_in.pak` with `opensherwood-tools export-image` gives natural colours (forest
greens and browns, blue and gold logo); a swapped channel order would not.

## Layout

| Offset | Type | Meaning |
|---|---|---|
| 0 | u16 | width in pixels |
| 2 | u16 | height in pixels |
| 4 | u32 | compression: `1` = zlib (stream starts `78 DA`), `2` = bzip2 (stream starts `BZh9`) |
| 8 | u32 | compressed size in bytes |
| 12 | bytes | compressed stream |

Decompressed size is always `width * height * 2` in every retail file inspected: 16 bits per pixel, RGB565,
little-endian words (bits 15..11 red, 10..5 green, 4..0 blue). The original renders through DirectDraw in 16-bit
modes, so backgrounds are stored in the display format. Whether any picture uses a colour key for transparency
(the sprite dictionary suggests bright green `0x07C0`/`0x07E0`) is still to be determined for SRES pictures.

A `.pak` file is several blobs concatenated without an index: read blobs until end of file.

## Where it is used

| File | Example dimensions | Compression |
|---|---|---|
| `Levels/{Day,Night,Fog,Custom*}/<map>.map` | 1408x960 (Croisement01); cities are larger | bzip2 |
| `Levels/.../<map>.min` (minimap) | 225x183 | bzip2 |
| `Interface/Start.sxt` | 1024x768 | bzip2 |
| `Interface/Loading.pak` (2 blobs back to back), `<lang>/data/Interface/Slideshow_in.pak` (many 640x480 slides back to back) | 1024x768 / 640x480 | bzip2 |
| `Savegame/Profile_xxx/*_t` (thumbnails) | 160x120 | bzip2 |
| SRES `PIC ` / `PICC` / `BTTN` items | 8x4 .. 400x200 | zlib in `DEFAULT.RES`, bzip2 in `Level.res` |

The prerendered map backgrounds live in `Levels/Day`, `Levels/Night`, `Levels/Fog` (lighting variants of the same map)
and `Levels/Custom1..4` (mission-specific variants). Not every map exists in every variant.

## Provenance

Observation (hexdumps of all files of these types, decompression with zlib/bzip2, size arithmetic).

## UI colour key (observation, 2026-09-02)

Pictures of `DEFAULT.RES` that are composited over other content (button plates `BTTN` 190, seals 145/146,
parchments `PIC` 147 and 38, cursor 284, HUD pieces) carry the RGB565 value `0x07C0` (pure green) in the areas
that the original shows transparent (plate corners, parchment margins, cursor surround). Evidence: the engine's
main menu drawn with `0x07C0` keyed out matches the original's `menu_main.png` outside the text areas; the
1024x512 backgrounds (186..189) and the 1024x768 credits background are drawn opaque. Whether the original
uses a shadow key in UI pictures is unknown; the engine applies none there (`ui_assets.rs`).
