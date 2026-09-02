# Image blob (`.map`, `.min`, `.pak`, `.sxt`, `*_t` thumbnails, SRES pictures)

Status: container **verified**; pixel format **partial** (16 bits per pixel established, channel order to confirm by rendering).

## Layout

| Offset | Type | Meaning |
|---|---|---|
| 0 | u16 | width in pixels |
| 2 | u16 | height in pixels |
| 4 | u32 | compression: `1` = zlib (stream starts `78 DA`), `2` = bzip2 (stream starts `BZh9`) |
| 8 | u32 | compressed size in bytes |
| 12 | bytes | compressed stream |

Decompressed size is always `width * height * 2` in every retail file inspected, i.e. 16 bits per pixel.
The likely encoding is RGB565 (the original renders through DirectDraw in 16-bit modes; the `STATUS HARDWARE`
console command prints "Current display bit depth"). RGB565 vs. ARGB1555 is to be confirmed by rendering
a `.map` and comparing with an in-game screenshot.

## Where it is used

| File | Example dimensions | Compression |
|---|---|---|
| `Levels/{Day,Night,Fog,Custom*}/<map>.map` | 1408x960 (Croisement01); cities are larger | bzip2 |
| `Levels/.../<map>.min` (minimap) | 225x183 | bzip2 |
| `Interface/Loading.pak`, `Interface/Start.sxt`, `Slideshow_in.pak` | 1024x768 | bzip2 |
| `Savegame/Profile_xxx/*_t` (thumbnails) | 160x120 | bzip2 |
| SRES `PIC ` / `PICC` / `BTTN` items | 8x4 .. 400x200 | zlib in `DEFAULT.RES`, bzip2 in `Level.res` |

The prerendered map backgrounds live in `Levels/Day`, `Levels/Night`, `Levels/Fog` (lighting variants of the same map)
and `Levels/Custom1..4` (mission-specific variants). Not every map exists in every variant.

## Provenance

Observation (hexdumps of all files of these types, decompression with zlib/bzip2, size arithmetic).
