# Navigation: layers, sectors, doors, lifts and the path finder (behaviour specification)

Status: `draft`, revision 2 (answers Codex review 15; awaiting review 16). Build: GOG English edition,
`Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base
`0x00400000`; every address below is a virtual address in that image. Analyst: 2026-09-13, session
`a275bfc2e1e321f17` (analyst role, ADR-0009). Reviewer: Codex `gpt-6-astra`, review 15 (verdict redo; this
revision answers it). Publication approval: pending (separate from factual approval).

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text, and no prescribed internal
structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer chooses
the organisation. Chunk tags (`STAT`, `WOAW`, `007 `, `FARM`, ` AZ `, `TEXT`, `TUPO`, `PPPP`) appear because
they are file-format compatibility tokens.

Claim ids are `NAV-nnn`, inline (the sibling specifications do the same); every statement an implementer relies
on carries one. Status: `observed` (read in the program at the address, and where possible confirmed on the nine
`.rhp` maps), `inferred` (the reading that fits every branch and every map, with the evidence), `unknown`.
Confidence is high unless stated. Sibling specifications referenced instead of repeated:
`spec-movement-animation-camera.md` (`ANIM-nnn`: the frame, play modes, per-frame displacement, turning,
collision entry points, arrival of move actions, the snapshot of the position record) and `spec-ai-combat.md`
(`AI-nnn`: the clock, the random stream, actor classes, orders as the AI issues them). The realised frame is
46.875 ms (ANIM-002); "frame" and "tick" below mean that frame.

## Identity and exposure

- **Analyst**: session `a275bfc2e1e321f17`, 2026-09-13, analyst role under ADR-0009. It has read decompiled
  code of the level loader's navigation chunks, the cell grid, the click resolution, the walk-order pipeline,
  the path finder and its worker, the door, building and lift objects, the bond crossing, the availability
  masks, the position record and the navigation natives (section 10). It must not implement any of them, and no
  implementer session may inherit its context, notes or tool output.
- **Delegated readers**: none. The throwaway probes and overlays in `re/notes/nav/` are this session's.
- **Spec reviewer**: Codex `gpt-6-astra`, review 15 (23 findings, verdict redo), answered by this revision;
  review 16 pending. A reviewer may read `re/`; its output is corrections to this file only.
- **Implementation reviewer**: pending, must be a session that has never read `re/`.
- **Publication approval**: pending, separate from factual approval.
- **Edition**: GOG English edition, the maintainer's lawfully acquired copy; the executable hash above.

## 0. Necessity record

- **Interoperability target.** Walking the player's own maps the way the missions and scripts expect: the
  `.rhp` motion data (layers, sectors, obstacles, the path graph, the sight areas, bonds, doors, lifts,
  materials, patches), the `.rhm` placements that name sectors by number, and the compiled scripts that address
  doors and buildings by index and expect the natives of section 6 to behave as the original's.
- **Information not otherwise available.** The earlier attempts are on record and did not settle it:
  `docs/formats/rhp.md` left the second half of `STAT` "undecoded" and guessed a visibility graph;
  `docs/formats/layers-and-doors.md` (data observation only) reached a per-sector model but hypothesised the
  door bytes, the meaning of natives 182/186-189/191, the walkability source (`WOAW`) and the graph framing, all
  of which turned out wrong (section 11); `crates/opensherwood-core/src/nav.rs` is an 8 px grid hypothesis;
  `docs/original/h01-win-path.md` lists the layer model as the first mission's blocker; no black-box run can
  show which polygons are walkable, how a click picks a layer, how doors admit characters or how the search
  orders its candidates.
- **Scope read.** About 120 functions in the ranges `0x004c0510`-`0x004c3930` (loader), `0x004e8070`-
  `0x004fa8e0` (grid, resolution, clearance, routes), `0x00552290`-`0x0055b000` (path finder),
  `0x0051a690`-`0x0051c0d0` (doors), `0x0057ca80`-`0x0057f8c0` (sectors, lifts, buildings), `0x005a3450`-
  `0x005a5960` (sight areas), `0x00582640`, `0x00583630` (walk sequence), `0x00467a50`, `0x0046a900`,
  `0x00469770`, `0x0046a000`, `0x00472070` (order execution), `0x0055f290`-`0x0055fc20` (position record),
  `0x00462aa0`, `0x004af540` (bonds), `0x004d7880`, `0x004cac00`, `0x004ccad0`, `0x004cd600` (clicks),
  `0x004d23d0`, `0x004d28e0` (per-tick consumer, patch application), the native adapters `0x00404000`-
  `0x00407000` and handlers listed in section 6. Section 10 names them by purpose.
- **Stopping condition, honestly.** Reading stopped when every layout was byte-exact on the nine maps (section
  7.1), the click, route, search, door, building, lift and bond rules were read to the point where each
  branch that a walk of the shipped missions exercises has a claim, and the review's disputed points were
  re-read. What remains **unread** and is listed in section 9: the tail of the goal-validity test, the node
  reset, the interior of the sliding routine, the sector-route link filter's second condition, the jump-line
  and jump-zone objects, most of the patch record, the tenants' trailing byte, the perception test, the
  player's door interaction, and the eviction that follows a patch. Their absence is stated where it matters.
- **Analyst authorisation.** On behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

Covers: the coordinate model (screen, world, height); the navigation content of a map as the loader reads it;
the availability masks and the per-sector state that patches change; the cell grid; how a screen point resolves
to a layer, a sector and a target (the click); the walk pipeline: route across sectors through doors, path search
inside a sector on the map's own graph, the resulting waypoints; bonds and the height of a character; doors
(state, who may pass, crossing); buildings (entering, occupancy); lifts (stairs, ladders, climbs); the natives 4,
8, 64, 98, 152, 156, 182, 186-189, 191. Inputs: the map and mission files, player clicks, script orders. Outputs:
waypoints and orders for the movement of `spec-movement-animation-camera.md`, the layer / sector / area of each
character, door and building state for the scripts.

Handed to the sibling specifications: the per-frame displacement, turning, the collision-aware move, the
completion of move actions (ANIM-200 to ANIM-209, ANIM-120, ANIM-132), the clock (ANIM-001/002), the random
stream (AI-005/006), actor classes (AI section 2.3).

Not covered (open, section 9): line of sight, jump lines and jump zones, the patch record beyond what the click
and the availability toggle need, the eviction after a patch, the player's door interaction.

## 2. Data model

Units: map pixels (screen or world as stated), 32-bit floats at run time unless a width is given, `i16` in the
files. Widths and signedness are those of the file layouts below; run-time counts are 16-bit unsigned.

### 2.1 Coordinates and height

- NAV-001 (observed, `0x0055f290`, `0x0055fc20`, `0x005a3810`). Screen coordinates are background pixels,
  `x` right, `y` down. A world point is `(x, y_w, z)` with `z` up; its screen row is `y_s = y_w - z` (one
  pixel per height unit, straight up the screen). Characters move in screen coordinates; their world row and
  height follow from the projection area they stand on (NAV-002).
- NAV-002 (observed, `0x005a3810`, `0x0055f290`). Every projection area (2.4) has a plane
  `z = a*x + b*y_w + c` through its first three vertices, oriented so that its normal points up. For a character
  at screen `(x, y_s)` on that area: `z = (a*x + b*y_s + c) / (1 - b)` and `y_w = y_s + z`; without an area
  `z = 0`, `y_w = y_s`. The same plane converts a screen direction into a world direction
  (`dz = (a*dx + b*dy_s) / (1 - b)`); facing selection from the world direction is ANIM-210/211.
- NAV-003 (observed, `0x0055fa70`). A character's position record (its contents and snapshot rules are
  ANIM-021) includes, for this specification: layer index, current sector (a polygon of 2.2, by number), current
  projection area (may be none), and the ground kind: the area's kind byte (2.4) when on an area, else the kind
  of the material polygon under the feet (2.7), else the level's default kind.

### 2.2 Motion chunk `STAT` (version 2): layers, sectors, obstacles

Consumed byte-exactly on all nine maps (7.1). Offsets are relative to the chunk body after its version word.
A `polygon` is `u8 (read and discarded), u16 n, n x (i16 x, i16 y), u8 (read and discarded)` (`0x0057cd60`).

```
u16 layer_count
layer_count x Layer {
    u16 sector_count
    sector_count x Sector {
        u8  kind                 0 = plain sector, 1 = lift sector (2.6)
        u32 sector_mask          availability mask of the sector's own record (2.9); 0 on every retail sector
        polygon outline          screen coordinates; the walkable floor of the sector
        u16 segment_count; segment_count x (i16 x1, y1, x2, y2)      search barriers (3.5); 0 on forest maps
        u32 unknown_flag         0 on every retail sector; consulted when a player character passes a door (3.8)
        u16 obstacle_count; obstacle_count x { u32 obstacle_mask (2.9); polygon }   holes in the floor
    }
}
Graph (2.3)
```

- NAV-010 (observed, `0x004eb120`). Layers are numbered by position, 0 first. Two further layer indices exist
  at run time: `layer_count` (a character put inside a building by native 156, NAV-190) and `layer_count - 1`
  (the side-B layer of building and lift doors and the layer of a building's initial tenants, NAV-040/193).
  The cell grid (2.8) exists for `layer_count + 1` layers.
- NAV-011 (observed, `0x0057ca80`, `0x004e8bd0`, `0x00558670`; confirmed by the placement data of
  `layers-and-doors.md` 1.3). Every polygon record gets a **polygon number** in reading order: sector 0 of
  layer 0, its obstacles in file order, sector 1 of layer 0, its obstacles, ..., then layer 1, and so on. This
  number is what every other record means by "sector": the `WOAW` link, the door and lift records, the mission
  placements (`rhm.md`, the `Q` of the placement triple). Lincoln: 52 sectors + 41 obstacles = numbers 0..92.
  One more plain sector, the level's default sector, follows the last number and holds characters that are on
  no sector.
- NAV-012 (observed, `0x004eb120`, `0x004e99e0`). Sector outlines and obstacle outlines are **wall polygons**:
  every edge is a blocking segment on the sector's layer, carrying the enabled state of its polygon. A sector
  outline is additionally a floor (its interior is walkable); an obstacle's interior is not.
- NAV-013 (observed, `0x004eb120`, `0x004ebbd0`). A kind-1 sector is a lift sector; the ` AZ ` chunk attaches
  its ends (2.6). An ` AZ ` record naming a sector that is not kind 1 is a load error.
- NAV-014 (observed, `0x00558670`, `0x00559660`). Path requests name a sector by polygon number; the program
  converts it to the sector's position within its layer. The conversion of a number that is not a sector
  (an obstacle, or an unknown number) reports an error and yields position 0, which the original then uses;
  OpenSherwood rejects the request instead (8.4).

### 2.3 The path graph, inside `STAT` after the sectors

```
u16 size_class_count; size_class_count x (f32 half_width, f32 half_height)   one class on every retail map: (6, 4)
u16 layer_count                                     equals STAT layer_count
layer_count x { u16 sector_count                    equals the layer's sector_count
    sector_count x { u16 region_count
        region_count x { u16 node_count; node_count x Node } } }
u16 edge_count; edge_count x Edge
u16 record_count; record_count x Record             record_count == edge_count on every map
Node {
    u16 class_count; class_count x u8 offset_bits   one byte per size class (always one on retail), NAV-021
    i16 x, i16 y                                    the corner
    i16 v1x, i16 v1y                                stored geometric input, not consumed by any routine read (NAV-025)
    i16 v2x, i16 v2y                                idem
    u32 node_mask                                   availability mask (2.9)
    u16 edge_count; edge_count x u16 edge_index     the edges that ENTER this node (NAV-022)
}
Edge {
    u16 la, sa, ra, na                              node A: layer, sector position in layer, region, node
    u16 lb, sb, rb, nb                              node B likewise; la == lb and sa == sb on every retail edge
    f32 cost                                        bytes 16..19 of the record; corner distance within 1.5 px
    u32 edge_mask                                   bytes 20..23; availability mask (2.9); nonzero on five maps
    u16 record_count; record_count x u16 record     one per size class; record index == edge index on retail
}
Record {
    i8 kind                                         -1 = impassable for this size class (no further fields)
    u8 mask
    u16 n; n x u8 bits_b                            pair i: candidate bits_b[i] at B <-> candidate bits_a[i] at A
    u16 n; n x u8 bits_a                            same n
}
```

- NAV-020 (observed on 9/9 maps, `0x00558670`). The graph hierarchy (layer, sector position, region, node) is
  encoded independently of the polygon lists and must be kept as read. On 5 of 9 maps the region count of every
  sector equals 1 + its obstacle count and every node is a vertex of the sector's polygons; on Croisement02,
  Croisement03, Nottingham and York four sectors have one region fewer and some nodes are not polygon vertices
  (7.1). Regions are therefore *usually* the sector's polygons (inferred, medium); an implementer reads the
  graph, never derives it.
- NAV-021 (observed, `0x00553d20`, `0x00553e60`). `offset_bits` says which of the four **candidate
  waypoints** of the corner are usable for the size class `(w, h)`: bit 1 = `(x - w, y - h)`, bit 2 =
  `(x + w, y - h)`, bit 4 = `(x + w, y + h)`, bit 8 = `(x - w, y + h)`. Paths visit candidates, never corners.
  A node whose byte is 5 or 10 is never expanded and never accepted as the answer (`0x00556490`).
- NAV-022 (observed on 9/9 maps, `0x00558670`, `0x00556490`). An edge listed in node B's list leads **to** A.
  Every retail edge has its reverse with its own record, so the graph is undirected in effect; edges never leave
  the sector or the layer, they do connect regions of one sector.
- NAV-023 (observed, `0x00554b80`). A record lists the candidate pairs joined by a straight walkable segment:
  pair `i` joins candidate `bits_b[i]` of B with `bits_a[i]` of A. `mask` is the OR of `bits_a`, `kind` the OR
  of `bits_b` (redundant with the lists).
- NAV-024 (observed, `0x00558670`, `0x00555a10`). `cost` is the edge's length as used by the search; the
  32-bit word after it is the edge's availability mask (2.9), which the review confirmed nonzero on Croisement01,
  Croisement02, Croisement03, Nottingham and York (Croisement01 edge 12: 512).
- NAV-025 (observed, `0x00553a10`, `0x00554b80`, `0x00556490`; medium). The two node vectors are stored and
  kept but none of the search, opening, reconstruction or wedge routines reads them; the wedge test is
  axis-aligned (NAV-143). Their orientation relative to the polygon's vertex order is inconsistent in the data
  (Lincoln: 149 of 633 nodes match a next/previous-vertex reading) and must not be regenerated. Status of their
  purpose: unknown.

### 2.4 Sight areas `WOAW` (version 3): projection areas and sight volumes

```
u16 material_count; material_count x u16 material_index      materials of the level's default sight volume
u16 area_count
area_count x Area {
    u16 n; n x { f32 x, f32 y_w, f32 h2, f32 z }     world outline; z = top height; h2 = second height (below 0.1 -> 0)
    f32 min[3]; f32 max[3]                            read; the loader recomputes its own boxes
    u8 linked; if linked: u16 sector_number, u16 layer   the area is the elevation source of that sector (NAV-011)
    u8 f_b1, u8 f_b0, u8 f_b3, u8 f_x20               four flag bytes (bit 3 is kept only when f_b0 != 0)
    u8 kind                                           the ground kind of characters standing on it (NAV-003)
    u16 m; m x u16 material_index                     materials (2.7) belonging to this area
}
```

- NAV-030 (observed, `0x005a3810`, `0x004ef340`, `0x005a5780`). An area is a 3D volume: world outline, a top
  plane through `(x, y_w, z)` of the first three vertices, a second plane through `(x, y_w, h2)` (use unknown),
  a screen box from `x` and `y_w - z`. A linked area belongs to its sector (the sector must be a floor with wall
  edges: load error otherwise) and its **screen outline** (`x`, `y_w - z`) is registered on layer 0 with the
  area as owner. Unlinked areas are volumes for sight and presentation only. Materials of an area are registered
  on the area's link layer. Areas are never walkable ground (contradicts `layers-and-doors.md` 1.3/5.2).
- NAV-031 (inferred, `0x00462aa0`, `0x0055fa70`). A character's area changes only by crossing a bond (3.7) or by
  placement; it decides height and ground kind.

### 2.5 Bonds `007 ` (version 2)

`u16 n; n x { i16 x1, y1, x2, y2; u16 area_a; u16 area_b; u16 layer }` (`0x0051faf0`, `0x004ed9b0`).

- NAV-035 (observed). `area_a` / `area_b` index the `WOAW` list (`0xffff` = none); the segment is in screen
  coordinates; the bond lives on `layer` (its cells and the layer's bond list). Crossing it swaps the character's
  area between the two (3.7).

### 2.6 Doors, buildings `FARM` (version 4) and lifts ` AZ ` (version 2)

```
FARM: u16 n; n x { u8 kind; u16 door_count; door_count x Door }      kind 0 = passage record, else a building
 AZ : u16 n; n x { u16 sector_number; u16 layer (skipped: equals the sector's layer on retail); u8 lift_type;
                    polygon shape (0 points on retail, discarded); u16 door_count; door_count x Door; u16 unknown }
Door {
    u8  type              0 default, 3 gate, 7 gate variant (a passage record accepts only 0, 3, 7);
                          1, 2 building door; 4 upper end of stairs, 5 lower end, 6 upper end of a climb
    u8  open              initial open state (1 = open)
    u8  lock_pc, lock_x, lock_civilian, lock_soldier      the active lock set, in this file order
    u8  alt_pc, alt_x, alt_civilian, alt_soldier          a second lock set, swapped in by a patch (NAV-172)
    polygon leaf          the door leaf outline on the background; an empty polygon means "no leaf"
    i16 ax, ay            side-A point, inside sector A (7.1)
    u16 sector_a, u16 layer_a
    i16 tx, ty            the threshold, between the sides
    i16 bx, by            side-B point, inside sector B (7.1)
    u16 sector_b, u16 layer_b   ignored for building and lift doors: side B is the building / lift itself
}
```

- NAV-040 (observed, `0x0051b2a0`, disassembled). Side A is `sector_a` on `layer_a`. For a passage door, side B
  is `sector_b` on `layer_b`; for a door of a building or lift record, side B is that building or lift, with
  layer index `layer_count - 1` (NAV-010). A door is known to both sides. A non-empty leaf polygon is registered
  on the higher of the two layers (unless that is `layer_count - 1`) with the door as owner: it is **not** a
  wall (it blocks nothing), it is the door's click target (3.1) and native 191 enables or disables it (6).
- NAV-041 (observed, `0x0051b110`, `0x0051b2a0`). For types 2, 6 and for type 4 when the lift is a climb, the
  threshold is moved 60 px (65 px for type 6) further along the direction from the threshold to the far side.
  Each door has a crossing cost: `|B point - A point| + 50` (types 0, 3..7) or `+ 100` (types 1, 2).
- NAV-042 (observed, `0x0057e060`, `0x004ebbd0`, `0x005716a0`). A lift record refers to a kind-1 sector by
  number; `lift_type` 1 = stairs, 2 = ladder, 3 = climb; 0 is corrected to 1 with a warning. The lift's door with
  the largest `ay` is its lower end, the one with the smallest `ay` its upper end. **Lift doors are appended to
  the same flat door list as the `FARM` doors** (in chunk order `FARM` then ` AZ ` on every retail map), so
  native 4 addresses them after the `FARM` doors (Lincoln: 59 + 20 = 79 entries).
- NAV-043 (observed, `0x0057e710`, `0x0057e4d0`). A building record is only its doors. A building's capacity
  is 65535 (never reached) and its admission kind (NAV-192) is that same value: everyone may enter, subject to
  the door rules. Its tenants come from the mission file (NAV-193).

### 2.7 Materials `TEXT` (version 2) and patches `TUPO` (version 3)

- NAV-045 (observed, `0x004eb830`). `u16 n; n x { u8 kind; polygon }`; kinds 9 and above become the level's
  default kind. Materials are ground kinds (NAV-003) and are attached to sight areas by index (2.4).
- NAV-046 (observed in part, `0x0054eea0`, `0x004d28e0`; medium). A patch record owns a clickable polygon
  ("patch target", 3.1) that redirects a click on it to a stored screen point on a stored `(layer,
  sector_number)` pair (`0xffff` = the default sector), a "refuse" byte (the selected character plays its
  refusal reaction instead of walking), obstacle polygons in an active and an inactive variant, lists of
  `(layer, index)` references that switch bonds and door leaves on or off, and the coordinates of the
  availability field it toggles (2.9). The complete layout is open (section 9).

### 2.8 The cell grid

- NAV-050 (observed, `0x004eb120`, `0x004e8070`, `0x004ed690`, `0x004e8bd0`). Every layer index of NAV-010
  has a grid of 64 x 64 px cells over the background (`cell = trunc(coordinate) >> 6`, clamped to the grid).
  A cell knows the polygons whose outline meets it, the wall segments (with their enabled state), the bonds and
  the sight lines that meet it. Every geometric query of this specification examines the cells of **one** layer;
  nothing is tested across layers.

### 2.9 Availability masks and the per-sector state (dynamic navigation)

- NAV-055 (observed, `0x005563f0`, `0x00556470`, `0x005559b0`, `0x00555a10`, `0x0055aa60`). Every sector
  (each layer, each sector position) has a 32-bit **state word** made of sixteen 2-bit fields. At level
  initialisation, after the map is loaded, every state word is `0x55555555` (each field = `01`); the loader's
  interim value is not visible to the game. The state words are part of the save game; after a load the
  availability of NAV-056 is recomputed from the restored words.
- NAV-056 (observed, `0x00555a10`). An object with mask `m` (a node's `node_mask`, an edge's `edge_mask`, an
  obstacle's `obstacle_mask`) is **available** in a sector with state `s` iff `(m & s) == m`. Available nodes and
  edges are the only ones the search sees (3.5); an unavailable obstacle is disabled: its polygon is removed from
  the cells and its wall segments stop blocking, an available one is enabled and registered. Mask 0 is always
  available; a mask with bit `2k` set requires field `k` = `01` (the initial state); a mask with bit `2k+1` set
  requires field `k` = `10`. A patch toggles one field of one sector between `01` and `10` (NAV-057). The
  sector's own `sector_mask` is stored; no routine read consumes it (unknown).
- NAV-057 (observed, `0x004d28e0`, `0x00556230`, `0x005559b0`; the parameters' origin in the patch record is
  inferred). Applying a patch: the in-flight path search (if any) is discarded and the worker restarted; field
  `k` of sector `(layer, number)` named by the patch is toggled and the availability of that sector recomputed;
  then, when the patch asks for it, every actor on that layer and sector whose walker box meets a newly enabled
  obstacle is marked and given an eviction element (its behaviour is unread: section 9). Search results computed
  before the toggle are not revised; a request queued before it is searched after it.

## 3. Behaviour

### 3.0 Execution order (what must hold per tick)

- NAV-090 (observed, `0x004c6ef0` via ANIM-101, `0x004d23d0`, `0x005532a0`). Within a level tick, in this
  order: (1) the path-result consumer runs once: a finished search becomes move actions of its character
  (NAV-146); (2) then, if no search is in flight and the request queue is not empty, the queue is re-ordered
  (NAV-147) and its head is handed to the search; (3) the element updates of ANIM-101 run, during which orders
  create requests (NAV-140) that join the queue for the next tick. A search handed over on tick `t` yields its
  result to the consumer on tick `t + 1` at the earliest (the original computes it on another thread; the
  required visible behaviour is only this ordering, 8.1). Clicks are processed before the level tick and turn
  into orders (3.4) executed in the element updates of the same tick.

### 3.1 From a screen point to a layer, a sector and a target (the click)

- NAV-100 (observed, `0x004e8fc0`, `0x004e9760`, `0x004debf0`). **Resolution on one layer** examines only the
  cell containing the point and only enabled, file-loaded polygons, in the cell's list order (registration order
  = file order), in three passes: (1) the first polygon that is a patch target, a lift shape or a door leaf and
  whose box and outline contain the point is the hit (the three kinds share this one traversal; there is no
  ranking between them beyond list order); (2) otherwise, over the wall polygons: the **last** sector outline
  containing the point becomes the candidate, and if any obstacle outline contains the point the layer answers
  "blocked"; (3) with a candidate, the jump-zone polygons (`PPPP`) containing the point are examined and the one
  whose first edge's midpoint is nearest to the reference point (the selected character's position) replaces the
  candidate. Answers: "nothing", "blocked", or a hit polygon. **Ordinary resolution** tries the layers from
  `layer_count - 1` down to 0 and stops at the first layer that does not answer "nothing"; a "blocked" answer
  records the layer and yields no target. A lift-shape hit becomes the lift sector on the lift's layer; a
  patch-target hit becomes the patch's stored `(layer, sector, point)`.
- NAV-101 (observed, `0x004e9920`). **Alternate resolution**, used when the level's flag for it is set (which
  flag: unknown): the layers are scanned from the top; the first layer that does not answer "nothing" is
  remembered; the **second** such layer's answer is returned (with lift-shape replacement); if there is no second,
  the first one's answer is returned. Test: two stacked floors under the cursor give the lower one.
- NAV-102 (observed, `0x004debf0`). The cursor is **valid** (walk cursor) when the hit is a patch target, a
  walkable sector (floor with wall edges), a door leaf or a jump zone; otherwise **invalid** (cross cursor). A
  patch target with its refuse byte set makes the selected character play its refusal reaction instead of
  walking. When no layer answers, the target layer defaults to the selected character's layer.

### 3.2 Clearance and straight walks

- NAV-110 (observed, `0x004f5750`, `0x004ed070`, `0x004ecf60`). **Clearance of a box** on a layer: the box must
  be valid (non-empty), and no enabled wall segment of the layer (sector outlines and obstacles alike) may meet
  it. Clearance says nothing about sector membership: a box entirely off every sector is clear.
- NAV-111 (observed, `0x004f5890`). **Unsticking a box**: at most 50 rounds; in each round every enabled wall
  segment meeting the box pushes it by its penetration depth plus 1 px along the axis of least penetration (the
  four axis directions are tried against the box's sides); a box that is off the map is first moved onto it;
  success when a round finds no segment. Failure leaves the box where the last round put it.
- NAV-112 (observed, `0x00556990`; `0x004f6c20` is the live variant the orders use). **Corridor test** from `p`
  to `q` for half-size `(w, h)`: the corridor is the rectangle spanned by the two boxes of half-size
  `(w - 1, h - 1)` centred at `p` and `q`, its sides taken from the boxes' corners according to the signs of
  `q - p` (a purely horizontal or vertical leg gives an axis-aligned box); it is free iff no enabled wall
  segment of the layer, taken from the cells the corridor's bounding box covers, crosses a side of the corridor or
  has an end inside it. The live variant additionally sees the segments that patches have enabled since load
  (they are the same enabled set, 2.9).

### 3.3 The route across sectors (doors as a graph)

- NAV-120 (observed, `0x0051a690`). At load, for every pair of doors that share a sector on one of their
  sides, a **link** is recorded on that sector with length = the straight distance between the two doors' points
  on that sector (each door's A or B point, whichever faces the shared sector).
- NAV-121 (observed, `0x004f9a80`, `0x004f9d60`, `0x004fa6f0`; the second link filter condition is inferred,
  medium). **Route search** from the character (position `P`, sector `S0`) to `target_sector` and target
  point `T`: a best-first search over doors, ordered by `f = door_cost + g + h` where, for a door `D` reached
  from sector `X` (crossing towards its other side `Y`): `g` = the distance walked so far to `D`'s point on `X`
  (from `P` for the doors of `S0`, through link lengths and door costs for later doors), `h = |D's point on Y -
  T|`, and `door_cost` is NAV-041. The doors of `S0` that the character may pass from `S0` (NAV-170, planning
  form) start the search. Popping `D`: if `Y == target_sector` the route is found. Otherwise every link of `D`
  on `Y` to another door `N` is followed except the link `D` was reached by and except links whose two doors
  lead from `Y` to the same sector; `N` is improved when unvisited or when `g_D + door_cost_D + link < g_N`;
  its `h` is computed on first visit and kept; it is queued only if the character may pass it from `Y`
  (planning form). Ties in `f`: the most recently queued door first. The route is `D`'s chain of predecessors;
  it may hold at most 102 doors (the found door plus 101 predecessors), more is a failure (no route).
- NAV-122 (observed, `0x004fa080`). A route **to a door** (a click on a leaf) is the same search with the door
  itself as the goal.

### 3.4 The walk sequence

- NAV-130 (observed, `0x00582640`; element kinds by role, their ids and completion rules are ANIM section
  3.4). Given a character, a target point with `(sector, layer)` and the order flags, the sequence of elements
  pushed to the character is:
  1. if the character stands on a lift end: a walk to that end's point on its current side;
  2. same sector: one **walk** to the target (NAV-140);
  3. other sector: the route of NAV-121 (NAV-122 for a door target); for each door `D` of the route in order,
     with `near` / `far` its points on the entered / left side:
     - if `D`'s far side is not a building: a walk to `near`, then an **approach** with tolerance 10 px;
     - if it is a building: (unless `D` is the first element) a wait of 50 frames, then a wait of
       `(r1 & 15) + (r2 & 15)` frames with `r1`, `r2` two consecutive draws of the random stream (AI-005, drawn
       while the sequence is built, in that order), then an **enter** at `near` facing the door;
     - if `D` is a door from `FARM` / ` AZ `: when the character is a player character, `lock_pc` is set and it
       has the lock-picking ability, a **lock-pick** at `far` and a **pick lock of D**, and the sequence
       **ends here** (the rest of the route is dropped); otherwise, when the far side is a ladder lift, a
       **ladder** element for `D`; then a **pass door D** (NAV-171) and an approach with tolerance 10 px at
       `far`;
     - if `D` is a jump line (the door list also holds the jump lines of `PPPP`): a **jump** element with the
       line's two points; its behaviour is unread (section 9);
  4. finally, unless the target sector is a building, a walk to the target point; when the flags ask for it and
     the last sector is a building, a walk to that door's B point instead.

### 3.5 The walk element and the path search inside a sector

- NAV-140 (observed, `0x00467a50`, `0x00552e00`). Executing a **walk** to `T` on the character's layer and
  sector: (1) the character's walker box, grown by 0.5 px on every side, is checked for clearance (NAV-110); if
  not clear it is unstuck (NAV-111) and the character moved to the box's centre; (2) if the order flags say
  "direct", or the live corridor test (NAV-112, with the gait's half-size) from the character to `T` is free:
  one move action to `T` (ANIM-208) and the element is done; (3) otherwise a **path request** is submitted:
  layer, sector number, size class 0, start = the character's position, goal `T`, gait, priority 0 for player
  characters and 1 for others. At submission the walker box at the start is checked again for clearance: if
  clear the request's *unstuck flag* is 0; otherwise the box is unstuck, the request's start becomes the box
  centre and the flag is 1; if unsticking fails the request is dropped and the element fails. The element then
  waits for the result (NAV-146).
- NAV-141 (observed, `0x005547c0`, `0x00557e70`, `0x00553e60`, `0x00558210` head; `0x005557c0` unread).
  **Search** for `(layer, sector, start, goal, flag)` with half-size `(w, h)` of size class 0, over the sector's
  regions, seeing only available nodes and edges (NAV-056):
  0. **Goal check**: the segments of the layer meeting the box of half-size `(w - 1, h - 1)` around `goal` are
     collected; the goal is rejected when that collection makes the goal unreachable (the decision's tail is
     unread: section 9; a goal box crossed by an enabled segment is the reading that fits every branch read).
  1. **Direct leg**, only when the request's unstuck flag is 1: if `start -> goal` crosses none of the sector's
     barrier segments (2.2) and the corridor (NAV-112) is free, the path is `[goal]`.
  2. **Opening**: the search window is the bounding box of `start` and `goal` grown by 400 px on every side.
     Every available node whose corner is in the window, whose walker box (half-size `(w, h)`) does not contain
     `start`, and whose corner is not separated from `start` by a barrier segment, is tested candidate by
     candidate (NAV-021): the candidate must satisfy the quadrant rule NAV-143 with respect to `start` and the
     corridor `start -> candidate` must be free. A node with at least one passing candidate is opened with
     `g = |start - corner|`, `h = |corner - goal|`, remembering its passing candidates.
  3. **Expansion**: repeatedly take the open node with the least `f = g + h` (ties: the most recently opened
     first); stop with "no path" when none is left or the request was cancelled. For the taken node, if its
     corner is not separated from `goal` by a barrier segment and its byte is not 5 or 10, test its candidates
     against `goal` (quadrant rule, corridor); the first taken node with a passing candidate is the **answer**.
     Otherwise, for every available edge entering the node with a record for the size class, and unless the
     node's byte is 5 or 10, the far node `A` is improved when `g + cost < g_A` (and below the best answer so
     far): its predecessor becomes this edge, `g_A = g + cost`, `h_A = |A - goal|` computed on first improvement
     and kept, and `A` is (re)opened.
  4. **Waypoints** (NAV-144): from the answer back to the start along predecessors, then reversed: the path
     starts at `start` (its own candidate walk, NAV-144 applied at the start point) and ends at `goal`.
  5. **Smoothing**: only when the path has more than three points: for `i = 2, 3, ...` while `i` is inside the
     list, let `d = p[i] - p[i-2]`; if the corridor from `p[i-2] + 0.00005 d` to `p[i] - 0.00005 d` is free
     (NAV-112), remove `p[i-1]` and test the same `i` again; else advance `i`.
- NAV-142 (observed, `0x00552290`). The answer is the **first** node found goal-visible in expansion order
  (the program keeps a "candidates to collect" count initialised to 1); the result is not necessarily the
  shortest path.
- NAV-143 (observed, `0x00553a10`, disassembled). **Quadrant rule.** For candidate bit `k` at
  `o = corner + (s_x w, s_y h)` (`s_x, s_y in {-1, +1}` per NAV-021) and a point `Q` (the start in the opening,
  the goal in the expansion), with `v = Q - o`: the candidate passes iff `(s_x v_x >= 0 and s_y v_y < 0)` or
  `(s_y v_y >= 0 and s_x v_x < 0)`: `Q` lies on the candidate's outer side in exactly one axis, strictly on the
  inner side in the other. The node's stored vectors play no part.
- NAV-144 (observed, `0x00554b80`, `0x005553d0`, `0x005568f0`, `0x00556920`). **Walk around a corner.** When
  the path enters node `N` through edge `e` with the set `R` of candidates usable on arrival (from the previous
  step's record) and must leave through the candidates `L` allowed by `e`'s record at `N`: (a) if `R & L` has
  exactly one bit, that candidate is the single waypoint at `N`; (b) else if `R` has exactly one bit `r`: rotate
  from `r` clockwise (1 -> 2 -> 4 -> 8 -> 1) and counter-clockwise in lockstep, each rotation stopping when it
  reaches a bit outside `N`'s `offset_bits` (that rotation dies) or a bit in `L` (found); the waypoints are the
  candidates from `r` to the found bit inclusive along the winning rotation; when both find at the same step
  the **counter-clockwise** rotation wins; (c) else (several arrival bits): for each `r` in `R` in increasing bit
  order run the lockstep of (b) counting the intermediate candidates passed; the `r` and rotation with the fewest
  intermediates wins, ties going to the earlier `r` and, within one `r`, to the **clockwise** rotation. The
  candidates usable at the next node are the partners (NAV-023) of the final bit. At the start point the same
  rule applies with `R` = the candidates that passed in the opening.
- NAV-145 (observed, `0x005547c0`). The path list built from the answer contains `goal` last; the start's own
  candidate walk is prepended; the consumer of NAV-146 skips the first waypoint when the unstuck flag is 0
  (the first waypoint is then the start itself) and keeps it when the flag is 1 (the corrected start).
- NAV-146 (observed, `0x004d23d0`, `0x00554560`). **Consuming a result**: the waypoints become one move action
  each (ANIM-208, with the request's gait and run flag), followed by the element's own completion. An empty
  result is kept with the tick it arrived; when 100 ticks have passed the element fails and a player character
  plays its refusal reaction; whether the request is re-run in between is unread (section 9).
- NAV-147 (observed, `0x005532a0`; the box's meaning unread). **Queue order** at hand-over: requests are
  ordered by priority 0 (first) to 3, stable for equal priority; before ordering, a request at priority 3 whose
  reference box contains its reference point is promoted to 2 and one at 2 whose box no longer contains it is
  demoted to 3 (both are dynamic-priority states of non-player requests; player requests stay at 0). Cancelling an
  element removes its request; a search in flight for a cancelled request is abandoned and its result discarded.

### 3.6 Movement along the waypoints

- NAV-150 (observed, `0x005b86b0`, `0x00561040`, `0x00467a50`). Movement is the sibling specification's:
  per-frame displacement ANIM-200/201/206, turning ANIM-203, the collision-aware move ANIM-204/205, move-action
  arrival ANIM-208. Two facts belong here: (a) the **approach** element of NAV-130 completes when
  `max(|dx|, |dy|) < tolerance + 5` px between the character and its point (a Chebyshev distance, not the
  move-action arrival rule); (b) inside the collision-aware move, another element becomes a proximity partner
  only when it is on the same layer and sector, eligible by class and state (ANIM-205a), its box meets the
  mover's box and the dot product of `(partner - mover)` with the movement direction is at least 5 px; the
  partner's callback then decides (ANIM-205, open).

### 3.7 Bonds: changing area (and height)

- NAV-160 (observed, `0x004af540`, `0x00462aa0`; the multi-bond chain order inferred). After a mobile element
  moved from `p` to `q` on its layer, the bonds of the cells crossed by `p -> q` are collected; duplicates (same
  segment and areas) are dropped; bonds the step does not actually cross are dropped. Exactly one bond: if the
  element's area is the bond's `area_a` or `area_b` it becomes the other; otherwise the crossing is illegal and
  the program recovers by looking, in the element's cell on its layer, for a linked area whose screen outline
  contains the position, then at `position + 2 * direction`; failing that, the area is unchanged. Several bonds:
  they are ordered into a chain in which consecutive bonds share an area and crossed in that order. Bonds never
  change layer or sector; those change through the walk sequence (3.4).

### 3.8 Doors

- NAV-170 (observed, `0x0051abe0`, disassembled). **Admission.** A character `C` may pass door `D` from side
  `s` (`s = A` when coming from `D`'s side A, `s = B` otherwise) in the *planning* form (route search) or the
  *crossing* form (NAV-171) iff all of the following hold. Classes are those of AI section 2.3: "player" = a
  player character, "soldier family" = the family that contains soldiers, player characters and civilians,
  "plain soldier" and "civilian" its subclasses; "class-0 family" = the remaining family.
  1. In the crossing form, `D`'s type is not 8.
  2. Not (`lock_pc` set and `D`'s run-time "player barred" byte set and `C` is a player).
  3. `D` is open.
  4. By type:
     - **0, 3, 7, 8**: a player passes iff `lock_pc` is clear, or `C` has ability 0x1c and `lock_x` is set;
       a plain soldier iff `lock_soldier` is clear; a civilian iff `lock_civilian` is clear; a member of the
       class-0 family passes iff its state is 7 and `lock_soldier` is clear; the special class 0x103 passes iff
       `lock_soldier` is clear; every other character is refused.
     - **1** (building door): a player as for type 0; otherwise `C` must be of the soldier family, and when
       `s = A` (entering) the building must not be full (occupants < capacity; capacity is 65535, NAV-043);
       then a plain soldier passes iff `lock_soldier` is clear, a civilian iff its "cannot pass" test is false
       and `lock_civilian` is clear; other subclasses are refused.
     - **2** (building door variant): as type 1, except that a **plain soldier is always refused**.
     - **4, 5, 6** (lift ends): a soldier-family member whose "cannot pass" test is true is refused; otherwise
       the lift's admission rule NAV-192 decides.
- NAV-171 (observed, `0x00467a50`). **Crossing** a passage door (types 0, 3, 7) from side `s`: the crossing
  admission is re-checked (failure fails the element); `C` records the door and side; then, in order: a move
  to the threshold, a marker, a move to the far point, a marker (a player entering a sector whose
  `unknown_flag` is set gets a different pair of actions there; the flag is 0 on all retail sectors). Building
  doors (1, 2) run the building entry (3.9); lift ends (4, 5, 6) run the lift sequence (3.10).
- NAV-172 (observed, `0x00579850`, `0x00579880`, `0x005798a0`, `0x005787f0`, `0x0051b660`). **State
  changes.** The natives of section 6 set the lock bytes; clearing `lock_pc`, `lock_soldier` or `lock_civilian`
  also **opens** the door. Native 191 enables or disables the leaf's click target. Applying a patch swaps the
  active lock set with the alternate set of every door the patch names. No native closes a door; closing and the
  player's opening by click go through the player's door interaction, which is unread (section 9).

### 3.9 Buildings

- NAV-190 (observed, `0x005781f0`, `0x0057ebd0`). **Putting a character inside** (native 156): layer index
  `layer_count`, sector = the building, position = the B point of the building's first door, "not displayed";
  appended to the occupants; if the character carries another, the carried one is put inside too; when the
  character is a player, every occupant of the soldier family that is out of action, or flagged as
  "revealed", and unassigned becomes "displayed".
- NAV-191 (observed, `0x0057ec80`). **Taking a character out** (native 152): removed from the occupants,
  "displayed"; a carried character is taken out too; when the character is a player and no player remains inside,
  every occupant's "displayed" mark is cleared. Layer, sector and position are set by the walk that follows.
- NAV-192 (observed, `0x0057de30`). **Admission kind** of a lift (its `lift_type`) and of a building (65535,
  NAV-043) for the door rule: kind 1 (stairs): everyone except a class-0-family character whose state is not 1;
  kind 2 (ladder): only soldier-family characters that are **not plain soldiers** (player characters and
  civilians pass, plain soldiers do not); kind 3 (climb): only a player with ability 0x15; any other kind:
  everyone.
- NAV-193 (observed, `0x004c1f90`, `0x0057e680`). The mission file's tenants chunk lists, per building in
  building-list order, the element indices of its initial occupants (each must be of the soldier family; others
  are a load error) and one trailing byte per building (kept, meaning unread). An initial occupant is placed on
  layer index `layer_count - 1` (not `layer_count`), in the building, at the B point of the building's first
  door, "not displayed".

### 3.10 Lifts

- NAV-200 (observed, `0x00467a50`, `0x0046a900`, `0x00469770`, `0x0046a000`; the action ids are the animation
  table's, ANIM-010). Using a lift end from side `s`: admission (NAV-170), then `C` records the door and side and
  plays, by `lift_type`: **stairs** - a move to the threshold with the current gait, a marker, a move to the far
  point with the gait's stair variant, a marker; **ladder** - mount at the threshold, the climb loop whose length
  is the animation table's loop length for that action, a marker, dismount at the far point (distinct actions
  for up and down and for the carrying variant of civilians); **climb** - the ivy actions likewise, with the
  distinct set for type-6 ends. The flight is an ordinary kind-1 sector (walking on it is ordinary movement).

### 3.11 Sight volumes

- NAV-210 (observed, `0x004e99e0`, `0x005a3810`; the consumer unread). The `WOAW` volumes and the material
  polygons are the geometry kept for line of sight; the perception test is open (section 9).

## 4. Claims

Claims are inline (NAV-001 .. NAV-210 above, NAV-300+ in section 6); each names its status, addresses and
confidence. Unknown claims map to `Assumption` variants (ADR-0008): NAV-025 (node vectors), NAV-046 (patch
layout), NAV-057 (patch parameters, eviction), NAV-101 (which flag), NAV-141 step 0 (goal check tail), NAV-146
(re-run within the window), NAV-147 (reference box), NAV-193 (trailing byte), NAV-210.

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| grid cell | 64 | px | `0x004ed690`, `0x004e8bd0`, `0x004ed070` | high |
| walker half-size, size class 0 | (6, 4) | px | `STAT` graph header, all nine maps | high |
| corridor and goal box half-size | (w - 1, h - 1) | px | `0x00556990`, `0x00558210` | high |
| search window margin | 400 | px | `0x00553e60` | high |
| walk-order clearance box growth | 0.5 per side | px | `0x00467a50` | high |
| unstick rounds | 50 | - | `0x004f5890` | high |
| unstick push | penetration + 1 | px | `0x004f5890` | high |
| door crossing cost | `|B - A| + 50` (types 0, 3-7), `+ 100` (types 1, 2) | px | `0x0051b2a0` | high |
| threshold push | 60 (types 2, 4-on-climb), 65 (type 6) | px | `0x0051b110` | high |
| route length limit | 102 | doors | `0x004f9a80` | high |
| approach completion | `max(|dx|, |dy|) < tolerance + 5` | px | `0x00467a50`, `0x005fc700` | high |
| door approach tolerance | 10 | px | `0x00582640` | high |
| building entry waits | 50; then `(r1 & 15) + (r2 & 15)` | frames | `0x00582640` | high |
| proximity partner threshold | dot product >= 5 | px | `0x00561040` | high |
| path failure window | 100 | ticks | `0x004d23d0` | high |
| smoothing endpoint shrink | 0.00005 of the leg | - | `0x005547c0` | high |
| building door search radius (native 64) | 300 | px | `0x00574860` | high |
| answers collected before stopping | 1 | nodes | `0x00552290` | high |
| initial sector state | `0x55555555` | - | `0x00556470` | high |
| building capacity | 65535 | - | `0x0057e4d0` | high |

## 6. Interfaces to the script VM

Native ids are the `.scb` ids (`scb.md`). The program registers 265 argument adapters (ids 0..264, table filled
at `0x00407400`-`0x00408200`); the adapters below normalise their arguments and return values as stated
(NAV-300, observed at `0x00404b20`, `0x00404ba0`, `0x00406810`, `0x004068d0`-`0x00406960`, `0x004069c0`).

| Id | Arity | Meaning | Failure / edge cases | Where |
|---|---|---|---|---|
| 4 | (i) -> door | door `i` of the flat door list: `FARM` doors in file order, then the lift doors (NAV-042); the result is a handle | `i = -1` -> none; `i` out of range -> logged error, none | `0x005716a0` |
| 8 | (i) -> building | building `i` (kind != 0 `FARM` records in order) | no bounds check in the original: `-1` reads the word before the list (OpenSherwood: none, 8.4) | `0x00571760` |
| 64 | (actor, location, flag) -> bool | walk into a building: the door of the flat list, of a `FARM` / ` AZ ` door object, nearest to the location's point within 300 px; issues the walk sequence (NAV-130) to that door's B point on its side-B sector and layer; returns whether it was issued | a location that is not a point, or no door within 300 px: logged error, 0 | `0x00574860` |
| 98 | (actor, building) -> bool | `building` none: 1 iff the actor's sector is **a building** (i.e. the actor is inside); else 1 iff the actor's sector is that building | - | `0x00577cf0` |
| 152 | (actor) | take the actor out of its building (NAV-191) | actor not human or not inside: logged error, nothing | `0x00577150` |
| 156 | (actor, building) | put the actor inside (NAV-190) | actor none or not human: logged error, then proceeds | `0x005781f0` |
| 182 | (door) -> int | the door's `lock_pc` byte, zero-extended | - | `0x00579810` |
| 186 | (door, v) -> 0 | `lock_pc := (v != 0)`; when clearing, the door opens | - | `0x00579850` |
| 187 | (door, v) -> 0 | `lock_x := (v != 0)` (the byte tested with ability 0x1c) | - | `0x00579870` |
| 188 | (door, v) -> 0 | `lock_soldier := (v != 0)`; when clearing, opens | - | `0x00579880` |
| 189 | (door, v) -> 0 | `lock_civilian := (v != 0)`; when clearing, opens | - | `0x005798a0` |
| 191 | (state, door) -> 0 | the door's leaf click target becomes enabled iff `state != 0` | door none: logged error; a door without a leaf: the original writes through a null leaf (undefined; OpenSherwood: no-op, 8.4) | `0x005787f0` |

All of them act immediately, none yields.

## 7. Acceptance tests

### 7.1 Reproducible byte checks (all nine maps, offsets relative to the `STAT` body after its version word)

The layouts of 2.2 and 2.3 read each `STAT` body to its last byte. Graph header = the size-class count word;
spans are half-open; "end" equals the body length.

| Map | header | first node | edge count | first edge | record count | first record | end |
|---|---:|---:|---:|---:|---:|---:|---:|
| Croisement01 | 2122 | 2140-2171 | 13607 | 13609-13637 | 87473 | 87475-87483 | 108915 |
| Croisement02 | 2179 | 2197-2238 | 13829 | 13831-13859 | 82935 | 82937-82945 | 102805 |
| Croisement03 | 2017 | 2035-2056 | 9733 | 9735-9763 | 45127 | 45129-45137 | 55309 |
| Derby | 4266 | 4284-4313 | 14792 | 14794-14822 | 53210 | 53212-53220 | 64256 |
| Leicester | 6467 | 6485-6512 | 26317 | 26319-26347 | 110151 | 110153-110161 | 134321 |
| Lincoln | 6770 | 6788-6809 | 26309 | 26311-26339 | 109135 | 109137-109145 | 133005 |
| Nottingham | 9600 | 9618-9653 | 37371 | 37373-37401 | 157661 | 157663-157671 | 192199 |
| Sherwood | 2379 | 2397-2420 | 11385 | 11387-11415 | 56859 | 56861-56869 | 70125 |
| York | 10593 | 10611-10634 | 38378 | 38380-38408 | 157100 | 157102-157110 | 191310 |

Further checks an implementation's parser must pass: `FARM` and ` AZ ` consumed exactly (doors 3, 1, 5, 42, 59,
59, 100, 5, 123; lifts 0, 0, 0, 12, 10, 10, 12, 4, 21 in the map order above); every lift sector is kind 1 and
the skipped word equals its layer; edge costs within 1.5 px of the corner distance; every edge appears in exactly
its node B's list and has a reverse; region counts equal 1 + obstacle count except for Croisement03 (layer 0,
sector 0), Nottingham (0, 0) and (2, 7), York (0, 0), which have one fewer; edge masks are nonzero on
Croisement01/02/03, Nottingham and York and zero elsewhere; all 140 passage-door A points lie inside sector A
and all B points inside sector B (138 and 133 of them exclusively so).

### 7.2 Synthetic tests (expected results)

1. **Availability.** One sector with state `0x55555555`; an obstacle with mask 1 is enabled, with mask 2
   disabled, with mask 0 enabled. Toggling field 0 turns the first off and the second on; toggling again
   restores. A save taken after the toggle restores the toggled availability.
2. **Clearance.** A 12 x 8 box straddling a sector outline edge is not clear; the same box 1 px inside is
   clear; a box on no sector at all is clear.
3. **Corridor.** Start `(100, 100)`, goal `(200, 100)`, class `(6, 4)`: a wall segment from `(150, 96)` to
   `(150, 104)` blocks; one from `(150, 104)` to `(150, 120)` does not block (the corridor's lower side is at
   `y = 103`); one from `(150, 90)` to `(150, 103)` blocks (an end inside the corridor).
4. **Quadrant rule.** Corner `(100, 100)`, candidate bit 1 at `(94, 96)`: `Q = (50, 120)` passes (outer in x,
   inner in y), `Q = (120, 80)` passes, `Q = (50, 80)` fails, `Q = (120, 120)` fails, `Q = (94, 120)` passes
   (`v_x = 0` counts as outer), `Q = (50, 96)` fails (`v_y = 0` is not strictly inner).
5. **Corner walk.** A node with `offset_bits = 15`, arrival `R = 1`, allowed `L = 4`: both rotations reach 4 in
   two steps; the counter-clockwise one wins (NAV-144b): waypoints 1, 8, 4. With `R = 3`, `L = 4`: from `r = 1`
   both rotations need one intermediate, from `r = 2` clockwise needs none: the walk is 2, 4. With `R = 1`,
   `L = 1`: one waypoint (case a).
6. **Search order.** A rectangular sector with one square obstacle between start and goal: the answer is the
   first goal-visible corner in `f` order, ties to the most recently opened; with two symmetric corners at
   equal `f` the later-opened one (file order) is chosen. Expected waypoints: start, that corner's candidate,
   goal; the smoothing pass (three points) does nothing.
7. **Smoothing.** Four waypoints in a straight free corridor collapse to two.
8. **Route.** Sectors X - Y - Z joined by doors `D1` (X|Y) and `D2` (Y|Z), a third door `D3` (Y|X) parallel to
   `D1`: a route X -> Z is `D1, D2`; the link `D1 - D3` is never followed (both lead from Y to X). Closing
   `D2` (open = 0) makes Z unreachable; setting `lock_soldier` on `D2` makes it unreachable for a plain soldier
   but not for a player.
9. **Admission.** Type-2 door, plain soldier from side A: refused; civilian with `lock_civilian` clear:
   admitted; player with `lock_pc` set and no ability 0x1c: refused; the same player with ability 0x1c and
   `lock_x` set: admitted. Ladder lift: player admitted, plain soldier refused.
10. **Natives.** `n186(d, 7)` sets `lock_pc = 1` and does not open a closed door; `n186(d, 0)` clears it and
    opens; `n182(d)` reads 1 then 0. `n191(0, d)` removes the leaf from click resolution: a click on the leaf
    then falls through to the sector below it.
11. **Click.** Two floors stacked at a point: ordinary resolution gives the upper; the alternate resolution
    gives the lower; an obstacle on the upper layer at the point gives "blocked" and no target in ordinary
    resolution.
12. **Approach completion.** Tolerance 10: a character at `(dx, dy) = (14, 0)` has completed, at `(15, 0)` not,
    at `(14, 14)` completed.
13. **Tick order.** A walk that needs a search on tick `t` produces its move actions on tick `t + 1` at the
    earliest; two requests submitted on the same tick by a player and a non-player are served player first.

### 7.3 Oracle procedures (pending, `harness/tools/original`)

Record, on Lincoln from the first mission's start, a click on the servant's hall floor and on a door leaf: the
route through the ramp, the stair lift and the passage doors of `layers-and-doors.md` section 4 must be the
one NAV-121 produces from the data (door costs and link lengths), and the arrival frames must match ANIM-208
within one frame.

## 8. Implementation choices, determinism, departures

1. **Synchronous search with the original's visible timing (deviation, approved by ADR-0004's determinism
   contract).** The original searches on a worker thread; its only visible effects are NAV-090 and NAV-147. The
   engine runs the search inside the tick, at most one per tick, and delivers the result at the consumer's slot
   of the next tick; a request never waits longer than in the original because the original never starts a
   second search before the first finishes. No thread priorities exist.
2. **Snapshot contract.** A snapshot carries: per sector the state word (NAV-055); per door the open flag,
   the active and alternate lock sets, the "player barred" byte, the leaf's enabled flag and the run-time
   "planning" marks (which can be recomputed and need not be saved); per building its occupant list and the
   "displayed" marks; per character the layer, sector number and area of NAV-003 (ANIM-021 for the rest); the
   request queue with each request's priority state, unstuck flag and submission tick; an in-flight search's
   inputs (it is re-run from them after restore, never resumed); the pending-failure list with its ticks.
   Availability (NAV-056) is derived from the state words after restore.
3. **Random draws.** The only random consumption of this specification is the building-entry wait of NAV-130:
   two consecutive draws of the global stream (AI-005) at the moment the sequence is built, in the order the
   sequence is built (per character in the click's selection order).
4. **Invalid inputs (OpenSherwood decisions where the original is undefined).** NAV-014: a request naming an
   obstacle or an unknown number is rejected (the element fails) instead of searching sector position 0.
   Native 8 with `-1` returns none. Native 191 on a door without a leaf is a no-op. A door index of native 64
   whose object is a jump line is skipped.
5. **Numeric precision.** Distances, costs, `g`/`h`/`f` sums, corridor sides and the 0.00005 shrink are 32-bit
   floats in the original; the engine uses `f32` for them with the same operation order as stated (sum of the
   two terms, then compare) so that tie decisions agree. Cell indices use truncation toward zero of the `f32`
   coordinate then a right shift.
6. **Clock.** Waits and windows of section 5 are in frames of 46.875 ms (ANIM-002); the engine's tick is that
   frame (ANIM section 8, choice 1).
7. **What is not replicated (open, section 9)** must be an `Assumption` variant until read.

## 9. Open questions

1. Line of sight: the routine and its rule (perception in `0x00486260` / `0x0048b5d0`; sight lines from
   `0x004e99e0`).
2. The sliding rule after a failed corridor test and the proximity callback (`0x00561040` second half,
   `0x00563e90`, `0x00564020`-`0x00564390`, `0x00560840`; ANIM-205c).
3. The goal check's decision tail (`0x00558210` after the segment collection) and the node reset before a
   search (`0x005557c0`).
4. The second condition of the route search's link filter (`0x004f9d60`): links whose two doors lead to the
   same sector are skipped; whether a further condition exists is unread.
5. Jump lines and jump zones (`PPPP`, `0x004fb510`, `0x0051e230`, `0x0051b6e0`, `0x00583630`, `0x0049dbc0`) and
   the jump element of NAV-130.
6. The patch record (`TUPO`, `0x0054eea0`, `0x0054f870`, `0x0054fe10`): which fields name the availability
   field, the sector and the layer of NAV-057; the eviction element pushed at `0x004d28e0`.
7. The building tenants' trailing byte (`0x004c1f90`).
8. The player's door interaction on a leaf click (the player-character method called from `0x004d7880`) and
   what closes a door.
9. `STAT` `sector_mask` use, `unknown_flag`, the second `WOAW` plane, the ` AZ ` trailer word, the node
   vectors' purpose (NAV-025).
10. The request re-prioritisation box (NAV-147) and re-runs within the failure window (NAV-146).
11. Which level flag selects the alternate click resolution (NAV-101); which input reaches the gait-6 and the
    gait-10 click handlers (`0x004ccad0`, `0x004cd600`).
12. Lift and door action ids as animation names (`0x005bddb0` table; `sprite-animations.md`).

## 10. Provenance

- Ghidra project `re/ghidra/robinhood` (never committed); exported decompilation and inventories from
  `scripts/ghidra/`; `scripts/ghidra/peek.py` for data bytes; a capstone helper in `re/notes/nav/` for the
  routines whose decompilation lost stack arguments (`0x0051b2a0`, `0x0051abe0`, `0x00553a10`, the worker at
  `0x005545c0`, the native adapters). The native table was recovered by scanning the registration code
  (`re/notes/nav/native_table.txt`, 263 slots, ids 0..264).
- Functions read, by purpose: loader and chunk dispatch `0x004c0510`, `0x004c3630`, `0x004c1f90`; sectors
  and graph `0x004eb120`, `0x00558670`, `0x0057cd60`, `0x0057ca80`; sight areas `0x004ef340`, `0x005a3810`,
  `0x005a5780`, `0x005a3450`; bonds `0x004ed9b0`, `0x0051faf0`, `0x004ed690`; doors, buildings, lifts
  `0x004ebc80`, `0x0057e710`, `0x0057e060`, `0x004ebbd0`, `0x0051b2a0`, `0x0051b110`, `0x0057d760`,
  `0x0051abe0`, `0x0057de30`, `0x0057e4d0`, `0x0051a690`, `0x0051b660`, `0x0057ebd0`, `0x0057ec80`,
  `0x0057e680`; materials `0x004eb830`; patches `0x0054eea0`, `0x004d28e0`; availability `0x005563f0`,
  `0x00556470`, `0x00556260`, `0x005559b0`, `0x00555a10`, `0x00556230`; grid `0x004e8bd0`, `0x004e99e0`,
  `0x004ed070`, `0x004ecf60`, `0x004e89d0`, `0x004e8070`; resolution `0x004e8fc0`, `0x004e9760`, `0x004e9800`,
  `0x004e9920`, `0x004debf0`; clearance `0x004f5750`, `0x004f5890`, `0x004f6c20` (head); clicks and sequences
  `0x004ccad0`, `0x004cd600`, `0x004cac00` (parts), `0x004d7880`, `0x00582640`; routes `0x004f9a80`,
  `0x004fa080`, `0x004f9d60`, `0x004fa6f0`; orders `0x00467a50`, `0x00472070` (structure), `0x0046a900`,
  `0x00469770`, `0x0046a000`; path finder `0x00552290`, `0x00552e00`, `0x005532a0`, `0x00553100`,
  `0x005546e0`, `0x00554750`, `0x005545c0`, `0x005547c0`, `0x00553e60`, `0x00556490`, `0x00556990`,
  `0x00553a10`, `0x00553d20`, `0x00554360`, `0x00554b80`, `0x005553d0`, `0x005568f0`, `0x00556920`,
  `0x00557e70`, `0x00557f90`, `0x005581a0`, `0x00558210` (head), `0x00554560`, `0x00559660`, `0x0055aa60`,
  `0x004d23d0`; position record `0x0055f290`, `0x0055fa70`, `0x0055fc20`; bonds at run time `0x00462aa0`,
  `0x004af540`; movement entry `0x005b86b0` (parts), `0x00561040` (head), `0x00563bb0`; natives `0x005716a0`,
  `0x00571760`, `0x00574860`, `0x00577cf0`, `0x00577150`, `0x005781f0`, `0x00579810`, `0x00579850`,
  `0x00579870`, `0x00579880`, `0x005798a0`, `0x005787f0` and their adapters; helpers `0x005fc6a0`,
  `0x005fc680`, `0x005fc700`, `0x005fc110`, `0x00608400`, `0x00608f10`, `0x00609050`.
- Data: `C:\Users\przem\source\gamedata\robinhood\DATA\Levels\*.rhp`, all nine, read only; probes and overlays
  in `re/notes/nav/` (never committed).
- Documents compared: `docs/formats/rhp.md`, `docs/formats/layers-and-doors.md`, `docs/formats/rhm.md`,
  `docs/formats/scb.md`, `crates/opensherwood-core/src/{nav,geom}.rs`, `spec-movement-animation-camera.md`,
  `spec-ai-combat.md`.
- No oracle run in this session (7.3 pending).
- Tests that will depend on this document: the byte checks of 7.1, the synthetic tests of 7.2, the door-native
  load-time expectations of `harness/tests/data/test_script.py`.

## 11. Precedence and differences from the current engine

**Precedence.** This specification supersedes: `docs/formats/rhp.md` sections `STAT`, `WOAW`, `007 `, `TEXT`,
`FARM`, ` AZ ` and "Walkable ground"; `docs/formats/layers-and-doors.md` 1.2 (`unknown_list` = materials,
`unknown_a` = ground kind, `unknown_0x08` = second height, `unknown_flags` = the four flag bytes), 1.3 and 5.2
(walkability from `WOAW`: wrong, NAV-012/030), 1.5 (bonds as sector portals: wrong, NAV-160), 2.1-2.2 (door
bytes and point roles: NAV-040 and 2.6), 2.4 (native 4 also holds the lift doors, NAV-042), 2.5 (natives 182,
186-189, 191: section 6), 3 (the graph: 2.3), 5 (its implementation plan); `docs/formats/scb.md` rows of natives
4, 8, 98, 152, 156, 182, 186-189, 191. What `layers-and-doors.md` established and this specification keeps: the
projection rule (1.1), the link pair as (sector number, layer) (1.2), the sector numbering with gaps (1.3), the
bond record framing, `FARM` kind 0 / kind 1 and the native-8 index space, door types 0/3/7 passage, 1/2
building, 4/5/6 lift ends with 5 the lower end. `rhm.md`'s placement triple is unchanged.

**Differences from `nav.rs` / `geom.rs`** (each a required change):

1. Corner graph per sector with availability masks (2.3, 2.9) instead of an 8 px grid; the search of NAV-141
   with its tie rules, corner walks and smoothing.
2. Walkable ground = sector outlines minus enabled obstacles per layer (NAV-012), never `WOAW` areas.
3. Layer and sector change only through doors and lift ends; bonds change the area only (3.7).
4. A door-graph route (NAV-121) and the walk sequence (NAV-130) precede any path search.
5. Click resolution per NAV-100/101/102 (top layer first, three-pass cell scan, blocked layers stop).
6. One search per tick with next-tick delivery (NAV-090, 8.1) instead of a synchronous unlimited search.
7. Door semantics of NAV-170/172 and section 6 (lock bytes, unlock-opens, 191 = click target).
8. Buildings, tenants and occupancy (3.9); lifts with admission kinds (NAV-192, 3.10).
9. Patches toggle availability (NAV-057); the sector state words are simulation state.
10. Movement is the sibling specification's; the engine's constant grid speed goes.
