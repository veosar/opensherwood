# RHP map file (`.rhp`, magic `MEUH`)

Status: **typed navigation framing; runtime integration incomplete**. `STAT` layers, sectors,
corner graph and availability masks, `FARM` doors/buildings and ` AZ ` lifts are parsed into typed
records and consumed exactly on all nine local maps. Their layout comes from
[`spec-navigation.md`, section 2](../original/spec-navigation.md), with component reviews 37/48.
`TUPO` and `LOUD` remain raw. Parsing all maps does not establish faithful gameplay; the
[integration review](../decisions/reviews/2026-10-02-navigation-disposition.md) lists open gates.

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
| `STAT` | 2 | layers/sectors, obstacle polygons, size classes, corner graph and edge records | typed |
| `TEXT` | 2 | polygons with a kind byte (0..8) | typed |
| `WOAW` | 3 | layers and 3D "projection areas" | typed |
| `007 ` | 2 | bonds between projection areas ("007" = Bond) | typed |
| `FACE` | 2 | foreground occluder masks (trees, rocks, walls) | typed |
| `FLIM` | 2 | animated background elements (sprite instances) | typed |
| `FARM` | 4 | passages and buildings with counted doors | typed |
| ` AZ ` | 2 | lift sectors, shapes and end doors | typed |
| `DARK` | 2 | dark zone polygons | typed |
| `TUPO` | 3 | named "patch" records (`pixel_vert`, `notpatch`); the leading count, with the `FLIM` count, sizes the map's part of the script element table (`scb.md`, Index spaces) | raw, count typed (`Rhp::tupo_count`) |
| `LOUD` | 2 | sound sources / zones (`RHSoundGeometry`) | raw |
| `PPPP` | 4 | zone polygons and jump lines | typed |

## Common framing

* `Point`: `i16 x, i16 y` (signed, including map-edge coordinates). `Point3`: `u16 x, u16 y, u16 z` (z = height above the ground plane).
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

## `STAT` (layered navigation)

The typed layout is specified in `spec-navigation.md` 2.2/2.3 (NAV-010 through NAV-025).
`Stat.layers` preserves sector and obstacle file order, barriers, availability masks and unresolved
flags. `Stat.graph` preserves size classes, nested layer/sector/region node tables, directed edges,
class-specific edge records and their index references. Edge cost bits remain available without
formatting or normalising floating-point values. `boundary`, `segments` and `obstacles` retain the
first sector's geometry for existing overlay consumers; `rest` is empty after successful parsing.

Counts are checked against the remaining bytes before allocation. A node needs at least 20 bytes
before its variable candidate bytes and entering-edge indices. Invalid cross-references are
rejected by graph validation; map construction separately checks geometry and allocation bounds.
All nine retail maps are covered by `opensherwood-formats/tests/gamedata.rs`.

The earlier union of the first `STAT` boundary and `WOAW` projection polygons was an engine
approximation. Map-backed mission orders now use the typed navigation map. The flattened geometry
and legacy grid remain available for diagnostics; their connectivity is not the layer/sector
movement contract. Building/lift integration is still incomplete.

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

## `FARM` and ` AZ `

`FARM` is a counted list of records with a kind and counted door records. Nonzero kinds identify
buildings; passage records do not create buildings. ` AZ ` is a counted list of lifts with sector,
layer, kind, shape polygon and end doors. Both share the typed door record: side points and sector/
layer qualifiers, threshold, lock bytes and optional leaf polygon. Exact framing and special
side-B interpretation are in `spec-navigation.md` 2.5/2.6, NAV-040 through NAV-043.

The parsers reject truncation/trailing bytes and are included in the malformed-byte smoke test.
Runtime endpoint and layer ambiguities remain listed in the integration review.

## Raw chunks (layout partially known, not parsed)

* `TUPO`: `u16 n`, then `pstring16 short name, pstring16 long name` and a 100-byte record. `n` is
  `Rhp::tupo_count()`: the script element table of a level starts with the `FLIM` entries and then these `n`
  patches (`scb.md`, Index spaces; `sherwood-hub.md` 4.1). Counts: Croisement01 6, Croisement02 9,
  Croisement03 9, Derby 7, Nottingham 11, Leicester 16, Lincoln 12, York 10, Sherwood 0.
* `LOUD`: `u16 n`, then `u32 id, u8 1, u8 kind (1 or 2)` and a variable record ending in `u8 100, u8[6],
  u32 0xff/0xfb`; kind 2 carries `u16 500, u16 1500` (sound distances?).

## Provenance

The older geometry and overlay findings below are observation-based. Typed navigation framing
was implemented from the public behaviour specification cited above; the implementer did not
read decompilation. On 2026-10-02, the format data tests consumed all nine maps, including the
complete `STAT`, `FARM` and ` AZ ` records. Scripts in `harness/tools/probe/` (`rhp_chunks.py`, `probe_*.py`,
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
