# RHP map file (`.rhp`, magic `MEUH`)

Status: **mostly decoded**. Every chunk's record framing is known and consumed exactly on all 9 files. The
geometry chunks (`STAT` polygons, `WOAW`, `007 `, `FACE`, `PPPP`, `TEXT`, `DARK`, `FLIM`) are decoded and
verified by overlay; `FARM`, ` AZ `, `TUPO`, `LOUD` and the second half of `STAT` are only partially understood
and stay raw in the parser (`crates/opensherwood-formats/src/rhp.rs`).

One `.rhp` per location: `sherwood`, `nottingham`, `lincoln`, `york`, `derby`, `leicester`, `Croisement01..03`
(forest crossroads used by ambush missions). The prerendered background is *not* inside; it is in
`Levels/<Variant>/<map>.map` (RGB565 image blob). All 2D coordinates below are pixels of that background,
origin top-left, `x` to the right, `y` down. Background sizes: Croisement01/03 1408x960, Croisement02 1792x1152,
Sherwood 1920x1088, Derby 1920x2752, Leicester 3136x1984, Lincoln 2944x2176, Nottingham 2304x3520, York 3136x2318.

## Container

The file is one chunk: `tag char[4]`, `size u32` (= file size - 8), then a body that starts with `version u32`
(2) followed by child chunks with the same `tag,size,body` shape; each child body also starts with a `u32`
version. The engine checks versions ("version %d does not match %d in chunk %s"). All offsets below are relative
to the child body *after* its version word.

| Tag | Version | Content | Parser |
|---|---|---|---|
| `SPOK` | 3 | 9-byte header | typed |
| `STAT` | 2 | boundary of the walkable ground, segments, obstacle polygons, then path graph (undecoded) | typed + raw rest |
| `TEXT` | 2 | polygons with a kind byte (0..8) | typed |
| `WOAW` | 3 | layers and 3D "projection areas" | typed |
| `007 ` | 2 | bonds between projection areas ("007" = Bond) | typed |
| `FACE` | 2 | foreground occluder masks (trees, rocks, walls) | typed |
| `FLIM` | 2 | animated background elements (sprite instances) | typed |
| `FARM` | 4 | buildings (door position, entry points) | raw |
| ` AZ ` | 2 | records with points; doors/lifts? | raw |
| `DARK` | 2 | dark zone polygons | typed |
| `TUPO` | 3 | named "patch" records (`pixel_vert`, `notpatch`); the leading count, with the `FLIM` count, sizes the map's part of the script element table (`scb.md`, Index spaces) | raw, count typed (`Rhp::tupo_count`) |
| `LOUD` | 2 | sound sources / zones (`RHSoundGeometry`) | raw |
| `PPPP` | 4 | zone polygons and jump lines | typed |

## Common framing

* `Point`: `u16 x, u16 y`. `Point3`: `u16 x, u16 y, u16 z` (z = height above the ground plane).
* `Polyline` / polygon: `u8 id, u16 count, count x Point, u8 id2`. The two id bytes are the same in every map
  for the same list position (e.g. the first `STAT` polygon is always `0x5a ... 0x82`, the first obstacle
  `0xe6 ... 0x42`, then `0x88 ... 0xcd`, `0xbb ... 0x0f`): they are editor-assigned pseudo-random ids, not
  checksums (sum/xor of the list bytes do not match) and not data. They are exposed as `id` / `id2`.
* Counts are `u16` unless stated.

## `SPOK` (9 bytes)

| Offset | Type | Field | Values |
|---|---|---|---|
| 0x00 | u32 | `unknown_0x00` | 100 / 86 / 101 (Croisement01..03), 92 (sherwood), 413 derby, 639 leicester, 194 lincoln, 444 nottingham, 916 york; equals the first `u32` of the `FOOT` chunk of missions on that map |
| 0x04 | u32 | `unknown_0x04` | 1 (forest maps, sherwood) or 0 (towns) |
| 0x08 | u8 | `unknown_0x08` | 0 |

## `STAT` (static geometry)

```
u16 unknown_0x00        2 (Croisement*), 12 derby, 7 leicester, 14 lincoln, 9 nottingham, 9 sherwood, 7 york
u16 unknown_0x02        1, 1, 2, 4, 10, 8, 10, 1, 10 (same order)
u8  unknown_0x04        0
u32 unknown_0x05        0
u8  boundary_id         0x5a
u16 n; n x Point        boundary: outline of the walkable ground (79 .. 344 points), hugs map borders
u8  segments_id         0x82
u16 n; n x (Point a, Point b)   segments (0 in forest maps, 9 derby/lincoln, 12 nottingham, 27 york)
u32 unknown             0
u16 obstacle_count      46, 42, 22, 5, 32, 23, 12, 25, 22
u32 unknown             0
obstacle_count x { Polyline polygon; u32 flags }
... rest (undecoded, see below)
```

`flags` is a bit mask: 0 for most obstacles; bits 0..5, 8, 10..13, 15, 16, 17 occur. Overlay evidence
(`rhp-overlay`, Croisement01): the boundary follows the forest edge, the rock face and the map borders; every
obstacle polygon outlines a tree trunk, rock, bush or stump. This is the "motion area" with its "obstacle masks"
of the console command MOTION.

Rest of the chunk (56 KB of 108 KB in Croisement01): further polygons with the same `u8 id, u16 n, points`
framing and different inter-record fields, then a table of 28-byte records
`u16 1, u16 index (incrementing), u32 0, u16 a, u16 0, u32 0, u16 0, u16 b, f32 length, u32 0` and node
records holding a `Point` plus small `i16` offset pairs and lists of `u16` edge ids. It is almost certainly the
pathfinder graph (console EULER); its framing is not established and the bytes are exposed as `Stat::rest`.

### Remainder of `STAT` (observation, 2026-09-02, `harness/tools/probe/probe_stat_layers.py`)

After the obstacles the remainder starts with further polygon records framed like the obstacles
(`u8 id, u16 n, n x Point, u8 id2`), each followed by `u16 nseg` (0 in every record seen), `u32 0`,
`u16 nobst`, `u16 a`, `u16 b`, then `nobst x { polyline, u32 flags }` and, when `a >= 2`, one more `u16`;
records are separated by a zero byte in the town maps. The sequential parse with this rule walks 26 records in
Lincoln (header `unknown_0x00` = 14 = the map's `WOAW` layer count, so these are not one record per layer),
40+ in Nottingham, and fails on the first record in the forest maps, Derby and Sherwood (three zero bytes
precede the first polygon there). The first Lincoln record (107 points) outlines the castle yard where the
first mission starts. Meaning and exact framing remain open; the engine does not use these records.

### Walkable ground and the projection areas (observation, 2026-09-02)

The `STAT` boundary alone does not describe where characters may walk in the town maps: the start position of
the first mission (Lincoln yard, map point 1937,1384) is outside the boundary and inside no obstacle, but
inside `WOAW` area 67 (20 vertices, linked to layer 27). Sampled coverage of the `WOAW` areas is 57 % of the
Lincoln map. The engine therefore treats a point as walkable when it is inside the boundary **or** inside any
projection area, and outside every obstacle (`opensherwood-core` `Geometry::areas`, `docs/features.md`).
Layer transitions (stairs, doors, ladders) are still ignored: every area is flat and connected through the
navigation grid wherever polygons touch or overlap, which is an approximation of the original's layer/sector
model.

## `TEXT`

`u16 n`, then `n x { u8 kind; Polyline polygon }`. Kinds 0..8 (3 most common). Polygons of 3..55 points inside
the map. Purpose not verified (terrain kind zones?). Croisement01: 4, sherwood 10, leicester 51.

## `WOAW` (projection areas)

```
u16 layer_count; layer_count x u16 layer_id      ids are 0..n-1 except lincoln (14 ids up to 40)
u16 area_count
area_count x {
    u16 n; n x { f32 x, f32 y, f32 unknown_0x08, f32 z }     n >= 3
    f32 min[3]      min of (x, y, unknown_0x08) over the vertices
    f32 max[3]      max of (x, y, z) over the vertices
    u8 link_count; link_count x { u16 unknown_ref, u16 unknown_layer }
    u8 unknown_flags[4]      mostly 1,1,1,1
    u8 unknown_a             0..7
    u16 m; m x u16           layer ids (m = 0 for nearly every area; 26 for a few areas of Croisement02)
}
```

Counts: Croisement01 4 layers / 85 areas, sherwood 10 / 127, derby 6 / 271, york 14 / 981. `unknown_0x08` is
0.001 for most vertices (a second height for some: 6..32); `z` is the height of the vertex (30, 61, 96 ... or
0.001). Overlay evidence: the large areas follow the sloped ground with edges parallel to the road, the small
ones sit exactly on rocks and bushes. These are the 3D "projection areas" of the console command PROJECTION.
The `(unknown_ref, unknown_layer)` link pairs of a map contain all `PPPP` zone id pairs in Croisement01 (and a
subset elsewhere); the link target is not established.

## `007 ` (bonds)

`u16 n`, then `n x 14 bytes`: `i16 x1, y1, x2, y2; u16 area_a; u16 area_b; u16 unknown_0x0c`. `area_*` index
`WOAW` areas (`area_b` = 0xffff for "none"); `unknown_0x0c` is 0..10 (several bonds share the same segment and
areas with different values). Coordinates may be slightly negative (-19). Overlay: each bond lies on the shared
edge of two adjacent areas. "007" = James Bond = the "bond" between projection areas (console ELEVATION).
Croisement01 29, york 180.

## `FACE` (foreground masks)

```
u16 count
count x {
    u16 unknown_0x00              0..12 (same range as DARK unknown_0x00)
    u8  kind                      bits 0,1: number of polylines; bit 4 (0x10): reference list follows the mask
    popcount(kind & 3) x Polyline     depth-sorting lines near the base of the object
    u16 x, u16 y                  position of the mask on the background
    u16 width, u16 height
    u16 packed_size
    height x row                  row = u8 packed_len, then control bytes: 0x80|n = repeat next byte n times,
                                  n < 0x80 = copy n literal bytes; a row decodes to ceil(width / 8) bytes,
                                  MSB first, 1 = pixel belongs to the mask
    if kind & 0x10: u16 ref_count; ref_count x u16 refs     values below the WOAW area count
}
```

Kinds seen: 1, 2, 3, 4, 5, 6, 7, 8, 0x12, 0x16, 0x17 (7 = two polylines is the most common; 4 = mask only).
`packed_size` equals the total byte count of the rows. Overlay evidence (Croisement01, all 103 masks): every
mask lands exactly on a tree trunk, rock or bush of the background and the masks are the parts of the background
drawn *in front of* sprites. Largest mask 1606x831 (lincoln). Counts: 103, 142, 131, 236, 466, 428, 527, 166,
828 (file order as above).

## `FLIM` (animated elements)

`u16 n`, then `n x { pstring16 sprite ("Treecr01", "Cr01fx", "notpatch"...); pstring16 name ("Croisement01 -
Arbre01"); u16 x; u16 y; u16 unknown_0x04 (0..1200); u8 flags[3] (each 0/1); Polyline line }`. `line` has 0 or 2
points (a horizontal sorting line for trees and butterflies; one point of Croisement01 is 0xffff = -1).

## `DARK`

`u16 n`, then `n x { u16 unknown_0x00 (0..12); Polyline polygon; u32 unknown_value (2, 4 or 6) }`. Empty in the
forest maps, 1 in sherwood, 24..37 in towns.

## `PPPP`

```
u16 n; n x { Polyline polygon; u16 unknown_ref; u16 unknown_layer; u8 unknown_flag }
u16 m; m x { Point3 from[2]; u16 unknown_a; Point3 to[2]; u16 unknown_b; u8 unknown_c }    29 bytes each
```

Zones are polygons of 4..19 points; `unknown_ref` is 0 or an id also found in `WOAW` links. The second table
holds jump lines: a raised segment (`z` 50..100) and a ground segment (`z` 0) a few pixels apart (console
JUMP-ZONE / jump-line). Croisement01: 11 zones, 13 jump lines; sherwood 2 / 1.

## Raw chunks (layout partially known, not parsed)

* `FARM`: `u16 n`, then records of 37 bytes in Croisement01: `u8 0, u8 1, u16 0, u8 1, u16 0, u8 flag,
  u8[5] 0, u16 a, u8 0, u8 id, Point door, u16 b, u16 c, Point p1, Point p2, u16 d, u16 e`; the door lies on a
  bond and `p1`/`p2` are just inside it. Sherwood records are 53 bytes (an extra point list); not resolved.
* ` AZ `: `u16 n`, records of about 40 bytes with a `u16 0x1a/0x1e/0x23...` word, an id byte and three points.
* `TUPO`: `u16 n`, then `pstring16 short name, pstring16 long name` and a 100-byte record. `n` is
  `Rhp::tupo_count()`: the script element table of a level starts with the `FLIM` entries and then these `n`
  patches (`scb.md`, Index spaces; `sherwood-hub.md` 4.1). Counts: Croisement01 6, Croisement02 9,
  Croisement03 9, Derby 7, Nottingham 11, Leicester 16, Lincoln 12, York 10, Sherwood 0.
* `LOUD`: `u16 n`, then `u32 id, u8 1, u8 kind (1 or 2)` and a variable record ending in `u8 100, u8[6],
  u32 0xff/0xfb`; kind 2 carries `u16 500, u16 1500` (sound distances?).

## Provenance

Observation only (no disassembly). Scripts in `harness/tools/probe/` (`rhp_chunks.py`, `probe_*.py`,
`overlay.py`, `map_png.py`) parse every chunk of all 9 files under the layouts above and check that each
chunk is consumed exactly and that all point coordinates fall inside the background of that map
(`probe_stat6.py`, `probe_woaw.py`, `probe_face_model.py`, `probe_pppp_007.py`, `probe_text_dark.py`,
`probe_flim.py`). The interpretation of `STAT` polygons, `WOAW` areas, `007 ` bonds and `FACE` masks was
verified by drawing them over the decoded Day background with `overlay.py` / `probe_face_place.py` (and the
Rust `opensherwood-tools rhp-overlay`) and looking at the result: masks coincide with trunks and rocks, obstacle
polygons outline them, the boundary follows the terrain edge, bonds lie on shared area edges. The FACE header
framing was found by brute-forcing the header length until the RLE rows decoded exactly, then explaining the
bytes (kind byte, `u8 id / u16 count` polylines, trailer). OpenDeathValley (github.com/OpenDeathValley,
GPL-3.0, the Desperados engine of the same Spellbound family) was looked at for shared concepts; its `.dvd`
map files use the same `tag, u32 size, u32 version` chunk framing but none of these chunk types, so nothing
from it was used. The community Blender importer for `.rhp` (closed source) was not consulted.
