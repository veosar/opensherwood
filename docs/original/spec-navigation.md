# Navigation: layers, sectors, doors, lifts and the path finder (behaviour specification)

Status: `draft` (analyst session 2026-09-13, awaiting Codex review). Build: GOG, executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base `0x00400000`; every address
below is a virtual address of that image. Analyst session: 2026-09-13, Claude (analyst role, ADR-0009).

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, no identifiers
or strings of the binary, no tables copied from its data (ADR-0009). Chunk tags (`STAT`, `WOAW`, ...) appear
because they are file-format compatibility tokens.

Claim ids are `NAV-nnn`. Status: `observed` (read directly in the decompiled routine at the address, and where
possible confirmed on the nine `.rhp` maps), `inferred` (the reading that fits every branch and every map, with
the evidence), `unknown`. Confidence is high unless stated.

## 0. Necessity record

Interoperability target: walk the game's own maps (the `.rhp` motion data and the `.rhm` placements) the way the
scripts and the missions expect: the same layers, sectors, doors, lifts, buildings and path graph, so that a
mission's route, its door choreography and its placements work unchanged. Everything that only the executable
decides was read, nothing more: the readers of the navigation chunks, the point-to-sector resolution, the walk
order pipeline (route across sectors, path search inside a sector, movement per frame), doors, buildings, lifts,
bonds, and the natives 4, 8, 64, 98, 152, 156, 182, 186-189, 191. Reading stopped when every branch of those
routines was explained and the layouts were confirmed byte-exact on all nine maps (section 8). Rendering,
AI decisions, combat and the script VM were not read beyond the native handlers named here.

Functions read, by purpose (details in section 9): level loader and chunk dispatch `0x004c0510`; motion chunk
`0x004c3630` -> `0x004eb120` (sectors) and `0x00558670` (path graph); polygon reader `0x0057cd60`; sight areas
`0x004ef340`, `0x005a3810`, `0x005a5780`; bonds `0x004ed9b0`, `0x0051faf0`, `0x004ed690`; buildings and doors
`0x004ebc80`, `0x0057e710`, `0x0051b2a0`, `0x0051b110`, `0x0057d760`, `0x0051abe0`, `0x0057de30`; lifts
`0x004ebbd0`, `0x0057e060`; materials `0x004eb830`; patches `0x0054eea0` (partly); grid `0x004e8bd0`,
`0x004e99e0`, `0x004ed070`, `0x004e89d0`, `0x004e8070`, `0x0051a690`; point resolution `0x004e8fc0`,
`0x004e9760`, `0x004e9800`, `0x004e9920`, `0x004debf0`; walkability `0x004f5750`, `0x004f5890`, `0x004f6c20`
(head); click to orders `0x004ccad0`, `0x004cd600`, `0x004cac00` (parts), `0x004d7880`, `0x00582640`; sector
route `0x004f9a80`, `0x004fa080`, `0x004f9d60`, `0x004fa6f0`; order execution `0x00467a50`, `0x00472070`
(structure), `0x0046a900`, `0x00469770`, `0x0046a000`; path finder `0x00552290`, `0x00552e00`, `0x005532a0`,
`0x00553100`, `0x005546e0`, `0x00554750`, the worker at `0x005545c0`, `0x005547c0`, `0x00553e60`, `0x00556490`,
`0x00556990`, `0x00553a10`, `0x00553d20`, `0x00554360`, `0x00554b80`, `0x005553d0`, `0x00557e70`, `0x00557f90`,
`0x005581a0`, `0x00558210` (head), `0x00554560`, `0x00559660`, `0x0055aa60` (save-game layout of a request),
`0x004d23d0`; position record `0x0055f290`, `0x0055fa70`, `0x0055fc20`; bond crossing `0x00462aa0`,
`0x004af540`; movement `0x005b86b0` (parts), `0x00561040` (head), `0x00563bb0`; buildings `0x0057ebd0`,
`0x0057ec80`, `0x0057e680`; natives: table at `0x00407400`-`0x00408200` (registration), handlers `0x005716a0`,
`0x00571760`, `0x00574860`, `0x00577cf0`, `0x00577150`, `0x005781f0`, `0x00579810`, `0x00579850`,
`0x00579870`, `0x00579880`, `0x005798a0`, `0x005787f0`; 2D helpers `0x005fc6a0`, `0x005fc680`, `0x005fc110`,
`0x005fc4d0`, `0x005fc7c0`, `0x005fc1e0`, `0x005fc200`, `0x005fc190`, `0x00608400`, `0x00609050`.

## 1. Scope

Covers: the coordinate model (screen, world, height); the navigation content of a map as the loader reads it
(`STAT`, `WOAW`, `007 `, `FARM`, ` AZ `, `TEXT`, and what is known of `TUPO` and `PPPP`); the per-layer cell
grid; how a screen point resolves to a layer and a sector (the click); the walk pipeline: route across sectors
through doors, path search inside a sector on the map's own graph, movement per frame; bonds and the height of
a character; doors (state, who may pass, crossing); buildings (entering, occupancy); lifts (stairs, ladders,
climbs); and the natives listed above. Inputs: the map and mission files, player clicks, script orders. Outputs:
character positions per frame, layer / sector / area of each character, door and building state for the scripts.

Not covered (open, section 6): the geometry of perception (line of sight), the exact sliding rule on collision,
jump lines, the patch record beyond what the click needs, the building tenants chunk of the mission file.

## 2. Data model

### 2.1 Coordinates and height

- NAV-001 (observed, `0x0055f290`, `0x0055fc20`, `0x005a3810`). Screen coordinates are background pixels,
  `x` right, `y` down. A world point is `(x, y_w, z)` with `z` up; its screen row is `y_s = y_w - z` (1 px per
  height unit, straight up the screen). Characters move in screen coordinates; their world row and height are
  derived from the projection area they stand on (NAV-002). Both are `f32` at run time; map data is `i16`.
- NAV-002 (observed, `0x005a3810`, `0x0055f290`). Every projection area (a `WOAW` record, 2.4) carries a plane
  `z = a*x + b*y_w + c` through its first three vertices (normal oriented upward). For a character at screen
  `(x, y_s)` on that area: `z = (a*x + b*y_s + c) / (1 - b)`, `y_w = y_s + z`. Without an area, `z = 0` and
  `y_w = y_s`. The same plane converts the screen movement direction into a world direction
  (`dz = (a*dx + b*dy_s) / (1 - b)`), from which the sprite facing is taken: one of 16 directions by angle of the
  world direction, mirrored (index xor 8) when the character's "reverse" flag is set (`0x0055fc20`).
- NAV-003 (observed, `0x0055fa70`). A character's position record holds: screen position, world position and
  height, previous position, direction, layer index, current sector (a polygon of 2.2), current projection area
  (may be none), and a "ground kind" taken from the area's kind byte (2.4) or, when off any area, from the
  material polygon under the feet (the `TEXT` chunk, 2.7), else the level's default.

### 2.2 Motion chunk `STAT` (version 2): layers, sectors, obstacles

All nine maps are consumed exactly by this layout (section 8). Offsets are relative to the body after the version
word. A `polygon` is `u8 (ignored), u16 n, n x (i16 x, i16 y), u8 (ignored)` (`0x0057cd60`; the two bytes are
read and discarded, they carry nothing).

```
u16 layer_count
layer_count x Layer {
    u16 sector_count
    sector_count x Sector {
        u8  kind                 0 = plain sector, 1 = lift sector (stairs / ladder / climb, 2.6)
        u32 unknown_id           0 on every retail sector; stored, use not found
        polygon outline          screen coordinates; the walkable floor of the sector
        u16 segment_count; segment_count x (i16 x1, y1, x2, y2)      "search barriers", 3.5 (0 on forest maps)
        u32 unknown_flag         0 on every retail sector; consulted by the door-crossing sequence, 3.8
        u16 obstacle_count; obstacle_count x { u32 unknown_id (0); polygon }   holes in the floor
    }
}
Graph (2.3)
```

- NAV-010 (observed, `0x004eb120`). Layers are numbered by position, 0 first. The loader keeps one extra
  "layer" index `L = layer_count` for characters that are inside a building (3.9); the cell grid (2.8) is
  allocated for `layer_count + 1` layers.
- NAV-011 (observed, `0x0057ca80`, `0x004e8bd0`, `0x00558670`). Every polygon object gets a **polygon number**
  in construction order: sector 0 of layer 0, then its obstacles in file order, then sector 1 of layer 0, its
  obstacles, ..., then layer 1, and so on. This number is what every other record means by "sector": the
  `WOAW` link, the bond, the door and lift records, the mission placements (`rhm.md` triple `Q`), all index the
  table of polygons by this number (Lincoln: 52 sectors + 41 obstacles = numbers 0..92, matching the gaps seen
  in `layers-and-doors.md` 1.3). A separate "position within its layer" is derived for the path finder
  (NAV-060). One more plain sector is created after the chunk (the level's default sector, number
  `sector_count_total + obstacle_count_total`) for characters that are on no sector.
- NAV-012 (observed, `0x004eb120`, `0x004e99e0`). Sector outlines and obstacle outlines are **wall polygons**:
  each edge becomes a blocking segment on the sector's layer. A sector outline is additionally a floor (its
  interior is walkable); an obstacle's interior is not. Walkability on a layer is therefore: inside a sector
  outline of that layer and outside every obstacle of it (3.2).
- NAV-013 (observed, `0x004eb120`). A kind-1 sector is a lift sector; the ` AZ ` chunk later attaches its ends
  (2.6). The loader refuses (assertion) an ` AZ ` record whose sector is not kind 1.

### 2.3 The path graph (the console's "EULER" graph), inside `STAT` after the sectors

```
u16 size_class_count; size_class_count x (f32 half_width, f32 half_height)     one class on every retail map: (6, 4)
u16 layer_count                                     equals STAT layer_count
layer_count x { u16 sector_count                    equals the layer's sector_count
    sector_count x { u16 region_count               = 1 + obstacle_count of that sector (one region per polygon)
        region_count x { u16 node_count; node_count x Node } } }
u16 edge_count; edge_count x Edge
u16 record_count; record_count x Record             record_count == edge_count on every map
Node {
    u16 class_count; class_count x u8 offset_bits   one byte per size class (always 1 byte on retail)
    i16 x, i16 y                                    the corner (a vertex of the region's polygon, 2.2)
    i16 d1x, i16 d1y                                vector from this vertex to the next vertex of the polygon
    i16 d2x, i16 d2y                                vector from the previous vertex to this one
    u32 flags                                       0 on the town maps; single bits on forest maps; use unknown
    u16 edge_count; edge_count x u16 edge_index     the edges that ENTER this node (NAV-022)
}
Edge {
    u16 la, sa, ra, na                              node A = layer la, sector sa (position in layer), region ra, node na
    u16 lb, sb, rb, nb                              node B likewise; la == lb and sa == sb on every retail edge
    f32 cost                                        the straight distance between the corners (within 1.5 px)
    f32 unknown                                     0.0 on every retail edge
    u16 record_count; record_count x u16 record     one record per size class; record == edge index on retail
}
Record {
    i8 kind                                         -1 = the edge is impassable for this size class (no more fields)
    u8 mask
    u16 n; n x u8 bits_b                            pairs (bits_b[i], bits_a[i]): offset at B -> offset at A, 3.5
    u16 n; n x u8 bits_a                            same n
}
```

- NAV-020 (observed on 9/9 maps). Each node is a corner of a sector outline or obstacle polygon (positions
  coincide with polygon vertices on all corners of six maps and on 78-95 % of the others; the rest are corners
  of the same polygons displaced by the editor, use unaffected). Regions are the polygons of the sector in file
  order (outline first, then the obstacles).
- NAV-021 (observed, `0x00553d20`, `0x00553e60`). `offset_bits` says which of the four **candidate waypoints**
  of the corner are usable for the size class `(w, h)`: bit 1 = `(x - w, y - h)`, bit 2 = `(x + w, y - h)`,
  bit 4 = `(x + w, y + h)`, bit 8 = `(x - w, y + h)`. A path never visits a corner itself, only these offset
  points. Nodes whose byte is 5 or 10 (only two opposite candidates) are never expanded nor tested as a goal
  (`0x00556490`).
- NAV-022 (observed on 9/9 maps, `0x00558670`, `0x00556490`). An edge stored in node B's list leads **to** A;
  every retail edge has its reverse edge with its own record, so the graph is undirected in effect. Edges never
  leave the sector (never the layer); they do connect regions of the same sector.
- NAV-023 (observed, `0x00554b80`). A record lists the pairs of offset candidates that are connected by a
  straight walkable segment: pair `i` connects candidate `bits_b[i]` of B with candidate `bits_a[i]` of A.
  `mask` is the OR of `bits_a`, `kind` the OR of `bits_b` (both are recomputable; the loader keeps them).
- NAV-024 (observed, `0x00558670`). Every node also carries a per-node search state at run time (g, h, f,
  parent edge, open flag, chosen bits) and a per-sector `u32` that the save game stores (one per sector per
  layer, initial 0, meaning unknown: `0x0055aa60`).
- NAV-060 (observed, `0x00558670`, `0x00559660`). The loader builds a conversion from polygon number (NAV-011)
  to "position of the sector within its layer" for every sector, and every path request names its sector by
  polygon number; the conversion fails (error, no path) for an obstacle or an unknown number.

### 2.4 Sight areas `WOAW` (version 3): projection areas and sight volumes

```
u16 material_count; material_count x u16 material_index      materials of the level's default sight volume
u16 area_count
area_count x Area {
    u16 n; n x { f32 x, f32 y_w, f32 h2, f32 z }     world outline; z = top height; h2 = a second height (h2 < 0.1 -> 0)
    f32 min[3]; f32 max[3]                            3D bounding box (read, then overwritten by the loader's own)
    u8 linked; if linked: u16 sector_number, u16 layer   NAV-011 number; the area is the elevation of that sector
    u8 f_b1, u8 f_b0, u8 f_b3, u8 f_x20               flag bits 1, 0, 3 (bit 3 only if f_b0 != 0), 0x20
    u8 kind                                           0..7: the "ground kind" of characters standing on it (NAV-003)
    u16 m; m x u16 material_index                     materials (TEXT polygons) that belong to this area
}
```

- NAV-030 (observed, `0x005a3810`, `0x004ef340`). An area is a 3D volume: outline in world coordinates,
  a top plane through `(x, y_w, z)` of the first three vertices, a second plane through `(x, y_w, h2)`
  (use unknown), a screen bounding box from `x` and `y_w - z`. A linked area is appended to its sector's area
  list (the sector must be a floor with wall edges, else an assertion) and is registered in the cell grid of
  layer 0 by its **screen outline** (`x`, `y_w - z`), owner = the area. Unlinked areas are volumes for sight and
  presentation only. The materials of an area are registered on the area's link layer.
- NAV-031 (inferred, `0x00462aa0`, `0x0055fa70`). The area a character stands on is only changed by crossing a
  bond (3.7) or by placement; it is the source of the character's height (NAV-002) and ground kind.

### 2.5 Bonds `007 ` (version 2)

`u16 n; n x { i16 x1, y1, x2, y2; u16 area_a; u16 area_b; u16 layer }` (`0x0051faf0`, `0x004ed9b0`).

- NAV-035 (observed). `area_a` / `area_b` index the `WOAW` area list (`0xffff` = none). The segment is in screen
  coordinates. A bond is registered in every 64 px cell its bounding box covers on `layer`, and in the layer's
  bond list. Crossing it swaps the character's area between `area_a` and `area_b` (3.7).

### 2.6 Doors, buildings `FARM` (version 4) and lifts ` AZ ` (version 2)

```
FARM: u16 n; n x { u8 kind; u16 door_count; door_count x Door }      kind 0 = passage record, else a building
 AZ : u16 n; n x { u16 sector_number; u16 layer (skipped by the loader); u8 lift_type;
                    polygon shape (0 points on retail: discarded); u16 door_count; door_count x Door; u16 unknown }
Door {
    u8  type              0 default, 3 gate, 7 gate variant (passages: only 0, 3, 7 are accepted);
                          1, 2 building door; 4 upper end of stairs, 5 lower end, 6 upper end of a climb
    u8  open              initial open state (1 = open)
    u8  lock_pc, u8 lock_x, u8 lock_civilian, u8 lock_soldier     the active lock set (order in the file: pc, x, civilian, soldier)
    u8  alt_pc, u8 alt_x, u8 alt_civilian, u8 alt_soldier          a second lock set, swapped in by a patch (NAV-172)
    polygon leaf          the door leaf outline on the background (may be empty)
    i16 ax, ay            point on side A, inside sector A (138 of 140 retail passage doors)
    u16 sector_a, u16 layer_a
    i16 tx, ty            the threshold, on the boundary between the two sectors
    i16 bx, by            point on side B, inside sector B (133 of 140)
    u16 sector_b, u16 layer_b   ignored for building and lift doors: side B is the building / lift itself
}
```

- NAV-040 (observed, `0x0051b2a0` with the disassembly at `0x0051b4ad`-`0x0051b51b`). Side A is the sector
  named in the record. For a passage door, side B is `sector_b` on `layer_b`; for a door read inside a building
  or lift record, side B is that building or lift and its layer index is the special value `layer_count - 1`
  (the loader's constant, not the file's; note that a character placed inside a building gets layer index
  `layer_count`, NAV-190: the two values differ and both are observed). The door is added to both sides' door lists with a side flag. The
  leaf polygon, when it has points, is registered in the cell grid on the higher of the two layers (unless that
  is the building layer) with the door as owner; it is **not** a wall (its edges do not block), it is the click
  target of the door (3.1) and its "active" flag is native 191 (section 5).
- NAV-041 (observed, `0x0051b110`, `0x0051b2a0`). For types 2, 4 (when the lift is a climb) and 6 the
  threshold point is pushed 60 px (65 px for type 6) further along the direction from the threshold towards
  the far side. Each door also gets a crossing cost: the distance from the A point to the B point plus 50
  (types 0, 3..7) or 100 (types 1, 2) (`0x0051b2a0` end).
- NAV-042 (observed, `0x0057e060`, `0x004ebbd0`). A lift record refers to a kind-1 `STAT` sector by number;
  the second word is skipped (it equals the sector's layer on every retail lift). `lift_type` 1 = stairs, 2 =
  ladder, 3 = climb (ivy); 0 is corrected to 1 with a warning. Of its doors the one with the largest `ay` is
  remembered as the lower end and the one with the smallest `ay` as the upper end.
- NAV-043 (observed, `0x0057e710`). A building record is only its doors. Its kind / capacity (see NAV-193) is
  not in this chunk.

### 2.7 Materials `TEXT` (version 2) and patches `TUPO` (version 3)

- NAV-045 (observed, `0x004eb830`). `u16 n; n x { u8 kind; polygon }`: a material is a polygon with a kind
  0..8 (9 and above become the level's default kind). Materials are the ground kinds of NAV-003 and are attached
  to sight areas by index (2.4).
- NAV-046 (observed in part, `0x0054eea0`). A patch record (the console "patch") owns, among other things: a
  clickable polygon (flag bit 2, 3.1) that redirects a click on it to a stored screen point on a stored
  `(layer, sector_number)` pair (`0xffff` = the default sector), a "refuse" byte that makes the player character
  react instead of walking, an obstacle polygon, an inactive variant of both, and lists of `(layer, index)`
  references that switch other objects (bonds, doors) on or off when the patch is applied. The full layout is
  open (section 6).

### 2.8 The cell grid

- NAV-050 (observed, `0x004eb120`, `0x004e8070`, `0x004ed690`). Every layer (plus the building layer) has a
  grid of 64 x 64 px cells over the background (cell = `floor(coordinate / 64)` after truncation to `i16`,
  clamped to the grid). Each cell lists the polygons whose outline intersects the cell, the wall segments
  (every edge of a wall polygon, as a segment object with an "enabled" flag copied from its polygon), the
  bonds, and the sight lines. All lookups (walkability, corridor tests, click resolution, bond crossing) go
  through the cells of one layer; nothing is ever tested across layers.

## 3. Behaviour

### 3.1 From a screen point to a layer, a sector and a target (the click)

NAV-100 (observed, `0x004e8fc0`, `0x004e9760`, `0x004e9920`, `0x004debf0`).

1. Layers are tried from the topmost (`layer_count - 1`) down to 0. On a layer, only the cell containing the
   point is examined, and only polygons that are enabled and come from the map file.
2. Pass 1: the first polygon (cell list order = file order) with flag "patch target" (bit 2), "lift" (bit 9)
   or "door leaf" (bit 12) whose bounding box and outline contain the point is the hit.
3. Pass 2: among wall polygons, the **last** sector outline containing the point becomes the candidate; if any
   obstacle polygon contains the point the answer for this layer is "blocked" (and the search stops, the layer
   is recorded, no target).
4. Pass 3 (only with a candidate): among jump-zone polygons (`PPPP`, flag 0x8000) containing the point, the one
   whose first edge's midpoint is nearest to the reference point (the selected character's position) replaces
   the candidate.
5. "Nothing here" on a layer continues with the next lower layer. A hit that is a lift's shape polygon is
   replaced by the lift sector; a patch hit is replaced by the patch's stored `(layer, sector, point)`.
6. The cursor is "valid" (walk cursor) when the hit is a patch, a walkable sector (floor with wall edges), a
   door leaf or a jump zone; otherwise "invalid" (cross cursor). A patch whose refuse byte is set makes the
   selected character play its refusal reaction instead of walking.
7. A variant used when a level flag is set (`0x004e9920`) takes the first layer from the top that has anything
   at the point (even "blocked") instead of skipping blocked layers; which flag is open.

The cursor also gets the layer of the selected character as a default when no layer answers.

### 3.2 Walkability and straight walks

- NAV-110 (observed, `0x004f5750`, `0x004ed070`). A point is free on a layer when it is inside the map and no
  wall polygon of that layer that is an obstacle (walls without floor) contains it. (The sector outline is not
  tested here: being off every sector is not "blocked" for this test.)
- NAV-111 (observed, `0x004f5890`). Unsticking a box: up to 50 iterations, for every obstacle polygon
  overlapping the box, the box is pushed out along the shortest penetration (four candidate pushes of
  `penetration + 1` px); returns success when no obstacle overlaps. Used for the path start (3.5) and for
  placements.
- NAV-112 (observed, `0x00556990`; `0x004f6c20` is the live variant used by orders). A straight walk from `p`
  to `q` for a walker of half-size `(w, h)` is allowed when no enabled wall segment of the layer intersects the
  corridor: the rectangle spanned by the two walker boxes of half-size `(w - 1, h - 1)` at `p` and `q`, whose
  four sides are built from the boxes' corners according to the sign of `q - p` (a degenerate direction gives a
  box). Segments are collected from the cells the corridor's bounding box covers; a segment blocks if it crosses
  the corridor or has an end inside it (four cross-product sign tests).

### 3.3 The route across sectors (doors as a graph)

NAV-120 (observed, `0x004f9a80`, `0x004f9d60`, `0x004fa6f0`, `0x0051a690`).

- At load, doors sharing a sector are linked pairwise: for every pair whose sides meet on a common sector, a
  link with the straight distance between the two doors' points on that sector (A point or B point, whichever
  faces the sector). Links are kept per door and side.
- To reach `target_sector` from the character's sector: every door of the current sector that the character may
  pass from that side (3.8) is opened with `g = |character - near point|`, `h = |far point - target point|`,
  `f = door_cost + g + h` (`door_cost` from NAV-041). Then A*: pop the lowest `f`; if the door's far side is
  `target_sector`, done; else for every link of the door on its far side leading to another door `N` (skipping
  the link that is the parent): `g' = g + door_cost + link_distance`; if `N` is unvisited or `g' < N.g`: set
  parent, `g`, side (the side of `N` facing the sector just entered), `h = |N far point - target|`, `f`, and
  push `N` if the character may pass it (can-pass with the "planning" flag). The route is rebuilt through parents,
  at most 100 doors, else failure. Priority queue: insertion before the first door with `f >=` (ties: newest
  first). A door target (a click on a leaf) uses the same search with the door itself as the goal
  (`0x004fa080`).

### 3.4 The walk sequence

NAV-130 (observed, `0x00582640`; element ids are the program's own order types, listed as facts).

Given a character, a target point with its `(sector, layer)` and flags, the sequence pushed to the character is:

1. If the character is on a lift end, first the lift's end point on its current side.
2. Same sector: one "go to point" order (type 0x14, section 3.5).
3. Other sector: the route of 3.3 (with the door-target variant when the target is a door leaf). For each door
   `D` of the route, with `near` / `far` = the A and B points ordered by the side of crossing:
   - far side not a building: "go to `near`" (0x14), then "arrive within 10 px of `near`" (type 2);
   - far side a building: (if not the first element) a wait of 50 units, then a wait of `rand&15 + rand&15`
     units, then "enter at `near` facing the door" (type 0x19);
   - then, when `D` is a door from `FARM` / ` AZ ` (its kind word is 1; the door list also holds the jump lines
     of `PPPP` as pseudo-doors with another kind): if the character is a player character, the door is
     locked for players and the character has the lock-picking ability: "lock-pick at the far point" (0x1a) and
     "pick lock of D" (10) and the sequence **ends** (the rest of the route is dropped); else, if the far side
     is a ladder lift: "climb ladder" (0xa9) with `D`; then "pass door `D`" (type 0x13, 3.8) and "arrive within
     10 px of `far`" (type 2);
   - otherwise (a jump line): a "jump" order (0x53) carrying the line's two points and their layers.
4. Finally, unless the target sector is a building: "go to the target point" (0x14); when the flags say so and
   the last sector is a building, "go to the door's B point" instead.

Orders are executed in sequence by the character (`0x00472070` dispatches human-only orders and defers the ones
above to `0x00467a50`).

### 3.5 "Go to point" and the path search inside a sector

NAV-140 (observed, `0x00467a50` case 0x14; `0x00552e00`). Executing "go to point" on the current sector and layer:

1. If the character is off the free ground (NAV-110) it is unstuck by NAV-111 with a box of `±25` px first
   (the position is corrected in place).
2. If the order flags say "direct" (bits 1 or 2), or the live straight-walk test (NAV-112 with the gait's walker
   half-size) succeeds: one move action to the point; done.
3. Otherwise a path request is queued: layer, the sector's polygon number, size class 0, the start (the
   character's position), the goal, the gait, priority 0 for player characters and 1 for others. The order waits
   (state 0x17) until the result arrives (3.6).

NAV-141 (observed, `0x005547c0`, `0x00558210` head, `0x00557e70`, `0x00553e60`). The search, for request `(layer,
sector, start, goal)` with half-size `(w, h)` = size class 0 (retail: `(6, 4)`), on the sector's regions only:

0. The goal is rejected (no path) when the walker box of half-size `(w - 1, h - 1)` around it overlaps an
   enabled wall segment of the layer (inferred from the collection loop; the test's tail was not read).
1. If the request's start had been unstuck (flag set by NAV-140 step 1), the direct leg start -> goal is tried
   first: allowed when it crosses none of the sector's barrier segments (2.2, `0x00557e70`) and the corridor of
   NAV-112 is free; the path is then just the goal.
2. Opening: the search box is the bounding box of start and goal enlarged by 400 px on every side. For every
   node of every region of the sector whose corner lies in that box, whose walker box does not contain the start
   (`0x00557f90`), and whose corner is not separated from the start by a barrier segment: for each candidate bit
   of the node (NAV-021) the offset point must lie outside the corner's wedge (cross-product signs against `d1`
   and `d2`, `0x00553a10`) and the corridor start -> offset point must be free; if any candidate passes, the node
   is opened with `g = |start - corner|`, `h = |corner - goal|`, `f = g + h`, no parent, the passing bits kept.
3. Main loop (`0x00556490`): pop the lowest-`f` node (queue as in 3.3: newest first among equal `f`); stop with
   "no path" when the queue is empty or the request was cancelled. If the corner is not separated from the goal
   by a barrier segment, test each candidate bit of the node (wedge test, then corridor offset point -> goal,
   skipped when the node's byte is 5 or 10); on success the node is the answer (the first popped node from which
   the goal is straight-reachable; the program has a countdown initialised to 1 that would allow more
   candidates). Else expand: for every incoming edge `e` of the node (2.3), if the edge's record for the size
   class exists and the node's byte is not 5 or 10: `g' = g + e.cost`; if `g' < A.g` and `g' < best_g`:
   `A.parent = e`, `A.g = g'`, `A.h = |A - goal|` (computed once), `A.f = g' + h`, push `A` (a node may be pushed
   more than once; the pop order handles it).
4. Reconstruction (`0x00554b80`, `0x005553d0`): from the answer back to the start along parent edges, each edge
   contributes the offset waypoints of the node it leaves: given the bits usable on arrival and the record's
   pairs, the walk uses one candidate when arrival and departure candidates coincide, else it goes **around the
   corner** through consecutive candidates (cyclic order of the four offsets, in the direction with the fewer
   steps, only through candidates the node allows), pushing one waypoint per candidate visited. The start's own
   candidates are handled the same way. The list is ordered start first, goal last.
5. Smoothing: when the list has more than 3 points, for `i = 2 ..`: if the corridor `p[i-2] -> p[i]` is free
   (NAV-112) drop `p[i-1]` and continue from the same place.

NAV-142 (observed, `0x005545c0`, `0x005546e0`, `0x00554750`, `0x005532a0`, `0x004d23d0`). The search runs on a
worker thread; the game thread never waits for it. States: stopped, idle, request handed over, result ready,
computing. Once per game tick the game: (a) when a result is ready, turns it into move actions for the requesting
character (one action per waypoint, first waypoint skipped unless the start had been unstuck; each action carries
the gait and the run flag of the order) and appends the order's completion; (b) when idle and the queue is not
empty, re-sorts the queue by priority (0 highest .. 3; a request is demoted to 3 while its reference box no longer
contains its reference point, and promoted back to 2 when it does; the box's meaning is open), sets the worker's
thread priority from the head request's priority (0 -> above normal, 1/2 -> normal, 3 -> below normal), and hands
the head request over. At most one search is started per tick and a result is consumed at the earliest on the
next tick. Cancelling an order removes its request; a request in flight is cancelled through a flag the search
polls. An empty result is kept for 100 clock units and then fails the order (player characters play the refusal
reaction); whether it is re-run meanwhile is open.

### 3.6 Movement per frame

- NAV-150 (observed, `0x005b86b0`, `0x005bdc20`). The distance moved per animation frame is the frame's advance
  value from the animation set (a 16-bit value per frame; the sprite data, not the code) times the time factor
  of the frame. While the character's facing differs from the wanted facing it turns one of 16 steps per frame
  and the advance is scaled: gait 6 doubles it, any other gait multiplies by 0.6; an advance below 0.7 becomes
  0.7. Gaits 7 and 8, and moves flagged "no collision", move without collision handling.
- NAV-151 (observed, `0x00561040` head, `0x00563bb0`; inferred for the sliding). The collision-aware move: (1)
  every other element on the same layer and sector whose box overlaps the walker's and whose distance is below 5
  px triggers the pair's collision callback (pushing / stopping, not read); (2) the move is tested with the live
  straight-walk test; if it fails, the move is resolved against the wall segments and bonds of the cells it
  crosses (sliding; rule open, section 6) and, when the character still ends inside a wall, the frame is
  rejected ("anticollision error").
- NAV-152 (observed, `0x00467a50` case 2). A "go to point" completes when `|position - target| < radius + 5`
  where `radius` is the order's tolerance (10 for the door approach points of 3.4); a "go to sector" variant
  completes when the character's sector equals the order's sector.

### 3.7 Bonds: changing area (and height)

NAV-160 (observed, `0x004af540`, `0x00462aa0`). After a mobile element moved from `p` to `q` on its layer, the
bonds in the cells crossed by `p -> q` are collected; exact duplicates (same segment, same areas) are dropped with
a warning; bonds not actually crossed (cross-product test) are dropped. One bond crossed: if the element's current
area is the bond's `area_a` or `area_b`, the element's area becomes the other one (and with it the plane for
height and the ground kind, NAV-002/003). If the current area is neither (an illegal crossing), the program
recovers by searching the element's cell for a linked area on the element's layer whose outline contains the
position, then a second time at `position + 2 * direction`; if none, the crossing is logged as fatal and the
area stays. Several bonds crossed in one step are ordered into a chain in which consecutive bonds share an area,
and crossed in that order (inferred). Bonds never change the layer or the sector; those change through the walk
sequence (3.4), which is why every cross-layer crossing of the data goes through a lift or door.

### 3.8 Doors

NAV-170 (observed, `0x0051abe0`). A door may be passed by a character from side `s` (0 = coming from side B,
1 = from side A) when all of these hold (`false` otherwise):

| Rule | Condition |
|---|---|
| activation | the request is a planning request, or the door type is not 8 |
| player lock-out | not (lock_pc set and the door's "player barred" run-time byte set and the character is a player character) |
| open | the door's open flag is set |
| type 0 (default), player character | lock_pc clear, or (the character has ability 0x1c and lock_x is set) |
| type 0, soldier-class character | side `s == 1` implies the far building is not full (occupants < capacity); then pass iff lock_soldier clear |
| type 0, civilian-class character | pass iff lock_civilian clear, unless the character's "cannot pass" virtual says so |
| types 1, 2 (building) | player character as type 0; soldier-class: also the building's occupancy rule; civilian-class: `lock_civilian` clear; else the building's "may enter" rule (NAV-192) |
| types 4, 5, 6 (lift ends) | soldier-class characters whose "cannot pass" virtual says so are refused; else the lift's "may use" rule (NAV-192) |
| types 3, 7 (gates) | as type 0 for player and soldier classes; other classes: `lock_soldier` clear |

NAV-171 (observed, `0x00467a50` case 0x13). Crossing a passage door (types 0, 3, 7) from the character's side:
the door's can-pass rule is checked again (failure fails the order); the character records the door and side; then
the actions: go to the threshold, a "door pass" marker action, go to the far point (player characters entering a
sector whose `unknown_flag` (2.2) is set get a special pair of actions instead of the plain walk; the flag is 0 on
all retail sectors), then another marker. Building doors (types 1, 2) run the building entry (3.9); lift ends
(types 4, 5, 6) run the lift sequence (3.10).

NAV-172 (observed, `0x00579850`, `0x00579880`, `0x005798a0`, `0x005787f0`). Door state changes: the natives set
the lock bytes; clearing `lock_pc`, `lock_soldier` or `lock_civilian` also sets the open flag. Native 191 sets the
leaf polygon's enabled flag (the door's click target). No native closes a door; closing happens through the door
interaction of the player (open, section 6). A patch swaps the active lock set with the alternate set
(`0x0051b660`, from the patch module).

### 3.9 Buildings

- NAV-190 (observed, `0x005781f0`, `0x0057ebd0`). Putting a character inside a building: its layer index
  becomes `layer_count` (the building layer), its sector the building, its position the B point of the building's
  first door; it is marked "not displayed"; it is appended to the building's occupant list; when it is a player
  character, every soldier-class occupant that is out of action and unassigned is marked "displayed".
- NAV-191 (observed, `0x0057ec80`). Taking a character out: removed from the occupants, marked "displayed";
  when it was a player character and no player character remains inside, every occupant's "displayed" mark is
  cleared. The layer / sector / position are set by the walk that follows (the door's A point).
- NAV-192 (observed, `0x0057de30`). A building or lift has a kind byte deciding who may enter through its doors:
  1 = everyone except characters of class 0 whose state is not 1; 2 = only soldier-class characters of the
  plain subclass; 3 = only player characters with ability 0x15 (the climb ability, used by climb lifts); other
  values = everyone.
- NAV-193 (unknown). The building's kind / capacity and the initial occupants come from the mission file (the
  loader's "building tenants" chunk, `0x004c1f90`, `0x0057e680`), not from `FARM`; layout open.

### 3.10 Lifts

NAV-200 (observed, `0x00467a50` case 0x13 types 4-6, `0x0046a900`, `0x00469770`, `0x0046a000`). Using a lift
end door from the character's side: the door's rule (NAV-170), then the character records the door and side and
runs the lift's action list by `lift_type`:

- stairs (1): go to the threshold with the current gait, a marker, go to the far point with the gait's stair
  variant (walk -> stair walk, sneak -> stair sneak; other gaits unchanged), a marker;
- ladder (2): the ladder mount action at the threshold, the climb loop action with the length taken from the
  animation table, a marker, the dismount at the far point (distinct actions for up and down and for the
  carrying variant of peasant-class characters);
- climb (3): the ivy climb actions likewise (mount, loop with length from the animation table, dismount), with
  the distinct set for type-6 (upper) ends.

The flight itself is an ordinary kind-1 sector: walking on it is ordinary movement (its outline is a floor with
walls), and its ends are doors, so the route of 3.3 treats lifts like doors with the lift's "may use" rule.

### 3.11 Sight volumes

- NAV-210 (observed, `0x004e99e0`, `0x005a3810`). The `WOAW` volumes and the material polygons are the geometry
  the program keeps for line of sight (edges of polygons with the "sight" flag become sight-line objects in the
  grid; areas carry two planes and a 3D box). Which routine performs the perception test, and its rule, was not
  read: open (section 6).

## 4. Constants

| Name (ours) | Value | Unit | Where it comes from | Confidence |
|---|---|---|---|---|
| grid cell | 64 | px | shift by 6 in every cell computation (`0x004ed690`, `0x004e8bd0`, `0x004ed070`) | high |
| walker half-size, size class 0 | (6, 4) | px | `STAT` graph header, all nine maps | high |
| corridor half-size | (w - 1, h - 1) | px | `0x00556990` | high |
| goal box half-size | (w - 1, h - 1) | px | `0x00558210` | high |
| search box margin | 400 | px | `0x00553e60` | high |
| unstick box for a walk order | 25 | px | `0x00467a50` case 0x14 | high |
| unstick iterations | 50 | - | `0x004f5890` | high |
| unstick push | penetration + 1 | px | `0x004f5890` | high |
| door crossing cost | 50 (types 0, 3-7), 100 (types 1, 2) | px | `0x0051b2a0` | high |
| threshold push | 60 (types 2, 4), 65 (type 6) | px | `0x0051b110` | high |
| route length limit | 100 | doors | `0x004f9a80` | high |
| arrival tolerance | radius + 5 | px | `0x00467a50` case 2 | high |
| door approach tolerance | 10 | px | `0x00582640` | high |
| building entry waits | 50; then `rand & 15 + rand & 15` | clock units | `0x00582640` | high |
| character collision distance | 5 | px | `0x00561040` | high |
| turning advance factor | 0.6 (gait 6: x2); floor 0.7 | px / frame | `0x005b86b0` | high |
| facings | 16 | - | `0x0055fc20` | high |
| path retry window | 100 | clock units | `0x004d23d0` | medium (unit) |
| building door search radius (native 64) | 300 | px | `0x00574860` (squared: 90000) | high |
| max candidates kept before answering | 1 | nodes | `0x00552290` sets the countdown | high |
| pass-flag bits per size class | 4 | - | `0x00553d20` | high |

## 5. Interfaces to the script VM

Native ids are the `.scb` ids (`scb.md`). The dispatcher is a table of 265 argument-adapter stubs filled at
`0x00407400`-`0x00408200`; id = slot. Handlers:

| Id | Arity | Meaning | Where |
|---|---|---|---|
| 4 | (i) -> door | door `i` of the flat door list (all `FARM` doors in file order, passages and buildings alike; lift doors are **not** in it); -1 -> none; out of range -> error and none | `0x005716a0` |
| 8 | (i) -> building | building `i` of the building list (`FARM` records with kind != 0 in order); no bounds check (-1 reads the word before the list) | `0x00571760` |
| 64 | (actor?, location, x) -> bool | walk into a building: among the doors of the flat list with record flag 1, the nearest to the location's point within 300 px; issues the walk sequence to that door's B point with the door's side-B sector and layer; errors when the location is not a point or no door is near | `0x00574860` |
| 98 | (actor, building) -> bool | `building` none: 1 iff the actor's sector is a building; else 1 iff the actor's sector is that building | `0x00577cf0` |
| 152 | (actor) | take the actor out of its building (NAV-191); errors when the actor is not a human or not in a building | `0x00577150` |
| 156 | (actor, building) | put the actor inside (NAV-190); errors when the actor is not a human | `0x005781f0` |
| 182 | (door) -> byte | the door's `lock_pc` byte | `0x00579810` |
| 186 | (door, v) | `lock_pc = v`; `v == 0` also opens the door | `0x00579850` |
| 187 | (door, v) | `lock_x = v` (the byte tested with the lock-picking ability) | `0x00579870` |
| 188 | (door, v) | `lock_soldier = v`; `v == 0` also opens | `0x00579880` |
| 189 | (door, v) | `lock_civilian = v`; `v == 0` also opens | `0x005798a0` |
| 191 | (state, door) | the door leaf's enabled flag = (state != 0): the leaf becomes / stops being a click target (it never blocks walking); error when the door is none | `0x005787f0` |

The natives read and write the objects of 2.6 directly; nothing is deferred.

## 6. Open questions

1. Line of sight: which routine tests visibility between two characters against the `WOAW` volumes / sight
   lines, and its rule (start from the AI perception in `0x00486260` / `0x0048b5d0` and the sight-line objects
   built in `0x004e99e0`; the volume translation `0x005a5960` is for movable obstacles).
2. Sliding on collision: the second half of `0x00561040` (after the straight test fails), `0x00563e90`,
   `0x00564020`-`0x00564390`, `0x00560840`.
3. Completion of a "move to point" action and the exact frame at which the next waypoint is taken:
   `0x005883a0`, `0x00588770`, `0x005886b0` (the sequence elements), `0x0058a9c0` (manager).
4. Jump lines and jump zones (`PPPP`): `0x004fb510`, `0x0051e230`, `0x0051b6e0`, `0x00583630`, `0x0049dbc0`.
5. The patch record (`TUPO`) beyond NAV-046: `0x0054eea0`, `0x0054f870`, `0x0054fe10`.
6. The building tenants chunk of the mission file and the building kind / capacity: `0x004c1f90`, `0x0057e680`.
7. The door interaction by the player (open / close on a leaf click): the player-character virtual called at
   `0x004d7880` (slot at offset 0x170 of the player-character class), and what closes a door.
8. The meaning of `STAT` `unknown_id`, `unknown_flag` (2.2), the node `flags` (2.3, forest maps), the second
   `WOAW` plane (`h2`), the per-sector save-game word (NAV-024), the ` AZ ` trailer word.
9. The goal-validity test's tail (`0x00558210` after the segment collection) and the node reset
   (`0x005557c0`).
10. The request re-prioritisation box (3.5, `0x005532a0`) and whether an empty result is re-run within the
    100-unit window.
11. The two click handlers (`0x004ccad0` gait 6, `0x004cd600` gait 10): which input maps to which.
12. Lift and door action ids as animation names (`0x005bddb0` table; `sprite-animations.md`).

## 7. Differences from the current engine

Against `crates/opensherwood-core/src/nav.rs`, `geom.rs` and `docs/formats/layers-and-doors.md`:

1. The path finder is not a grid: it is a corner graph per sector (2.3), searched with the algorithm of NAV-141
   (offset candidates around corners, straight-line goal test, corridor tests against wall segments), with an
   explicit smoothing pass. `nav.rs` (8 px cells, A* over cells, erosion) computes different paths.
2. Walkable ground is the set of `STAT` sector outlines of a layer minus its obstacles (NAV-012), per layer.
   `geom.rs` uses the layer-0 boundary plus any `WOAW` area as walkable; `WOAW` areas are sight volumes and
   elevation sources, never walkable ground (NAV-030). `layers-and-doors.md` 5.2 planned to rasterise the
   shifted `WOAW` polygons: wrong source; the sector outlines are already in screen space.
3. Sector membership and layer changes happen only through doors and lift ends (3.3, 3.4); bonds only change
   the projection area (3.7). The engine currently has neither sectors nor doors on the walk path.
4. Cross-sector routing is an A* over doors with straight-line link distances and per-door costs (3.3), then a
   sequence of orders per door (3.4). Nothing of this exists in the engine.
5. Click resolution is top layer first with the priority order patch / lift / door leaf, then sector, blocked by
   obstacles, then jump zones (3.1). The engine resolves clicks on one flat grid.
6. The path search is asynchronous: at most one request handed over per tick, results consumed next tick,
   priorities by character class (3.5). The engine searches synchronously in the tick.
7. Door semantics: `open` is file byte 1, the four lock bytes are bytes 2-5 (player, key, civilian, soldier),
   natives 186/188/189 unlock **and open**, 182 reads the player lock, 191 toggles the leaf's click target and
   never blocks walking (3.8, 5). `layers-and-doors.md` 2.2/2.5 has byte 0 as "open", 191 as open/close and 182
   as "is open".
8. Door geometry: the first point is inside side A, the third inside side B, the second is the threshold (2.6);
   `layers-and-doors.md` 2.1 has the first point on the bond and the others on one side.
9. Buildings: entering sets layer `layer_count`, the building as sector, the door's B point as position, with
   occupant lists and display rules (3.9); native 8 has no bounds check and -1 is not "outdoors" in the
   handler (98 with "none" is the outdoors test). The engine's stubs return the index.
10. Lifts are kind-1 `STAT` sectors with typed ends; stairs walk, ladders and ivy run climb action lists
    gated by ability 0x15 for climbs (3.10). The engine has no lifts.
11. Movement speed is the animation's per-frame advance scaled by the time factor, with turning at 16 facings
    and the 0.6 / x2 / 0.7 rule (NAV-150); arrival is `radius + 5` (NAV-152). The engine moves at a constant
    speed on the grid.
12. `layers-and-doors.md` 3: the "graph header (u16 1, f32 6.0, f32 4.0)" is the size-class list; "groups" are
    polygons (regions); the "mid-section" is layers 1.. of the same structure; the edge record and the trailing
    table are decoded (2.3); the statement that the engine does not need this graph is wrong: it is the path
    finder's only graph.

What `layers-and-doors.md` got right: the projection rule `y_s = y_w - z` (1.1); the link pair as (sector
number, layer) (1.2); the sector numbering with gaps (1.3); bonds as segments between two areas with a layer
(1.5); `FARM` kind 0 / kind 1 and the flat leaf list as native 4's index space (2.1, 2.4); native 8 over the
kind-1 records (2.4); lifts on kind-1 sectors with two typed ends, 5 = lower (2.3); door types 0/3/7 passage,
1/2 building, 4/5/6 lift ends (2.2); the "trailing table" being per-edge crossing data (3.6, guessed).

## 8. Validation against the data

Throwaway probes in `re/notes/nav/` (`nav_stat.py`, `probe_graph.py`, `probe_doors.py`, `overlay_graph.py`),
not committed:

- `STAT` under 2.2 + 2.3 is consumed exactly (0 bytes left) on all nine maps. Sector counts per layer:
  Croisement01 [1, 7], Croisement02 [1, 4], Croisement03 [2, 9], Derby 12 layers / 40 sectors, Leicester 7 / 51,
  Lincoln 14 / 52 (10 kind-1), Nottingham 9 / 80, Sherwood 9 / 11, York 7 / 109. Obstacles: 46, 42, 24, 13, 54,
  41, 61, 28, 46. Nodes: 289, 314, 242, 360, 644, 633, 891, 268, 893. Edges: 2638, 2468, 1264, 1372, 2994, 2958,
  4296, 1624, 4240; records equal edges; every record index equals its edge index; every edge has one record.
- Every edge is listed in exactly its node B, none in A; every edge has its reverse; no edge crosses a sector or a
  layer; region counts equal 1 + obstacle count for every sector (three exceptions of one region on three maps).
- Edge cost equals the corner distance within 1.5 px on every edge; the second float is 0 on every edge.
- Node class bytes take every value 0..14; pair lists of a record have equal length; bits are only 1, 2, 4, 8.
- Overlay of Croisement01 layer 0 and Lincoln layer 0 (`re/notes/nav/overlay_*.png`): nodes sit on the polygon
  corners, edges join mutually visible corners across the floor; nothing crosses an obstacle.
- `FARM` under 2.6 is consumed exactly on all nine maps (doors 3, 1, 5, 42, 59, 59, 100, 5, 123); ` AZ ` likewise
  (lifts 0, 0, 0, 12, 10, 10, 12, 4, 21); every lift sector is a kind-1 `STAT` sector; the skipped word equals the
  sector's layer on every lift; lift door types are only 4, 5, 6; passage door types only 0, 3, 7; building
  door types 1 (and one 2).
- Passage door points against the sector outlines (by polygon number): first point inside side A on 138 of 140,
  third inside side B on 133 of 140, second inside neither on 89 (on the shared boundary).

## 9. Provenance

- Ghidra project `re/ghidra/robinhood` (never committed); exported decompilation `re/out/decomp_all/<addr>.c`,
  `re/out/inventory.tsv`, `re/out/strings.tsv` from `scripts/ghidra/`; `scripts/ghidra/peek.py` for data bytes;
  a capstone disassembly helper written in `re/notes/nav/` for the three routines whose decompilation lost
  stack arguments (`0x0051b2a0`, `0x0051abe0`, the worker at `0x005545c0`).
- The native table: registration code scanned for the slot stores (`re/notes/nav/native_table.txt`, 263 slots,
  ids 0..264).
- Data: `C:\Users\przem\source\gamedata\robinhood\DATA\Levels\*.rhp`, all nine, read only.
- Existing documents compared: `docs/formats/rhp.md`, `docs/formats/layers-and-doors.md`, `docs/formats/rhm.md`,
  `docs/formats/scb.md`, `crates/opensherwood-core/src/{nav,geom}.rs`.
- No oracle run in this session; timing claims are frame- and tick-relative as the program computes them.
- Tests that will depend on this document: the format pins of `STAT`, `FARM`, ` AZ ` (counts above), the
  synthetic corner-graph tests, the sector route tests, the door natives' load-time expectations.
