# Navigation: layers, sectors, doors, lifts and the path finder (behaviour specification)

Status: `draft`, revision 3 (answers Codex review 20; awaiting the next review). Build: GOG English edition,
`Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base
`0x00400000`; every address below is a virtual address in that image. Analyst: 2026-09-13, session
`a275bfc2e1e321f17` (analyst role, ADR-0009). Reviewer: Codex `gpt-6-astra`, reviews 15 and 20 (archived as
`docs/decisions/reviews/2026-09-13-codex-review-15-spec-navigation.md` and
`2026-09-13-codex-review-20-spec-navigation.md`). Publication approval: pending (separate from factual approval).

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text, and no prescribed internal
structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer chooses
the organisation. Chunk tags (`STAT`, `WOAW`, `007 `, `FARM`, ` AZ `, `TEXT`, `TUPO`, `PPPP`) appear because
they are file-format compatibility tokens.

Claim ids are `NAV-nnn`, inline; every statement an implementer relies on carries one. Status: `observed` (read in
the program at the address, and where possible confirmed on the nine `.rhp` maps), `inferred` (the reading that
fits every branch and every map, with the evidence), `unknown`. Confidence is high unless stated.

Sibling specifications, pinned: `spec-movement-animation-camera.md` revision 2 (commit `b0cd053`, `ANIM-nnn`:
the frame, play modes, per-frame displacement, turning, the collision-aware move, element completion, layer
changes inside action lists, the snapshot of the position record); `spec-ai-combat.md` revision 2 (`e966b05`,
`AI-nnn`: the random stream, actor classes); `spec-script-vm.md` revision 3 (`25b6dbe`, `VM-nnn`: handles, the
native table, sequence elements). Clock: the program requests a minimum frame of 40 ms and realises 46.875 ms on
the reference host (ANIM-001/002); the engine's fixed logic frame of 46.875 ms is the decision of
`docs/decisions/ADR-0010-logic-frame.md`, not a property of the original. "Frame" and "tick" below mean that
logic frame; every count in frames transfers one to one.

## Identity and exposure

- **Analyst**: session `a275bfc2e1e321f17`, 2026-09-13, analyst role under ADR-0009. It has read decompiled
  code of the level loader's navigation chunks, the cell grid, the click resolution, the walk-order pipeline,
  the path finder and its worker, the door, building and lift objects, the bond crossing, the availability
  masks, the position record and the navigation natives (section 10). It must not implement any of them, and no
  implementer session may inherit its context, notes or tool output.
- **Delegated readers**: none. The throwaway probes and overlays in `re/notes/nav/` are this session's.
- **Spec reviewer**: Codex `gpt-6-astra`; review 15 (23 findings, redo) answered by revision 2, review 20 (20
  findings, redo) answered by this revision; both archived under `docs/decisions/reviews/` (their session ids are
  not recorded there). The reviewer read `re/` and this file; its output is corrections to this file only.
- **Implementation reviewer**: pending, must be a session that has never read `re/`.
- **Publication approval**: pending, separate from factual approval.
- **Edition**: GOG English edition, the maintainer's lawfully acquired copy; the executable hash above.

## 0. Necessity record

- **Interoperability target.** Walking the player's own maps the way the missions and scripts expect: the
  `.rhp` motion data (layers, sectors, obstacles, the path graph with its availability masks, the sight areas,
  bonds, doors, lifts, materials, patches), the `.rhm` placements that name sectors by number, and the compiled
  scripts that address doors and buildings by index and expect the natives of section 6 to behave as the
  original's.
- **Information the earlier investigations did not establish.** `docs/formats/rhp.md` left the second half of
  `STAT` "undecoded" and guessed a visibility graph; `docs/formats/layers-and-doors.md` (data observation only)
  reached a per-sector model and hypothesised the door bytes, the meaning of natives 182/186-189/191, the
  walkability source (`WOAW`) and the graph framing; `crates/opensherwood-core/src/nav.rs` is an 8 px grid
  hypothesis; `docs/original/h01-win-path.md` lists the layer model as the first mission's blocker. Those
  documents record what data and manual could establish (the projection rule, the sector numbering, the bond and
  door record framing); what they hypothesised about behaviour is contradicted by the reading (section 11). The
  oracle recordings measure walks but not the rules that produce them.
- **Scope read.** About 125 functions in the ranges `0x004c0510`-`0x004c3930` (loader), `0x004e8070`-
  `0x004fa8e0` (grid, resolution, clearance, routes), `0x00552290`-`0x0055b000` (path finder),
  `0x0051a690`-`0x0051c0d0` (doors), `0x0057ca80`-`0x0057f8c0` (sectors, lifts, buildings), `0x005a3450`-
  `0x005a5960` (sight areas), `0x00582640`, `0x00583630` (walk sequence), `0x00467a50`, `0x0046a900`,
  `0x00469770`, `0x0046a000`, `0x00472070` (order execution), `0x0055f290`-`0x0055fc20` (position record),
  `0x00462aa0`, `0x004af540` (bonds), `0x004d7880`, `0x004cac00`, `0x004ccad0`, `0x004cd600` (clicks),
  `0x004d23d0`, `0x004d28e0` (per-tick consumer, patch application), the native adapters `0x00404000`-
  `0x00407000` and the handlers of section 6; section 10 names them by purpose.
- **Unsuccessful steps on record.** The analyst's first probes read the second edge word as a float, took the
  regions for the polygons, took the node vectors for the polygon edges and mis-assigned the door admission
  branches by type; reviews 15 and 20 caught these and the re-reads are in this revision. Three routines were
  disassembled because their decompilation lost stack arguments.
- **Stopping condition.** Reading stopped when every layout was byte-exact on the nine maps (7.1), and the
  click, route, search, door, building, lift, bond and availability rules had a claim for every branch the
  analyst inspected in the listed routines. This is coverage of the inspected routines, not of every branch a
  shipped mission may exercise. What remains **unread** is in section 9, and section 8.7 gives each of those
  gaps an explicit fallback so that the implementation contract is complete without them.
- **Analyst authorisation.** On behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

Covers: the coordinate model (screen, world, height); the navigation content of a map as the loader reads it;
the availability masks and the per-sector state that patches change; the cell grid; how a screen point resolves
to a layer, a sector and a target (the click); the walk pipeline: route across sectors through doors, path search
inside a sector on the map's own graph, the resulting waypoints; bonds and the height of a character; doors
(state, who may pass, crossing); buildings (entering, occupancy); lifts (stairs, ladders, climbs); the natives 4,
8, 64, 98, 152, 156, 182, 186-189, 191. Inputs: the map and mission files, player clicks, script orders. Outputs:
waypoints and orders for the movement of the movement specification, the layer / sector / area of each
character, door and building state for the scripts.

Handed to the sibling specifications: the per-frame displacement, turning, the collision-aware move and its
sliding, the completion of move actions and of the door-approach element (ANIM-200 to ANIM-209, ANIM-120,
ANIM-132, ANIM section 3.4 table), layer changes inside action lists (ANIM-311), the random stream (AI-005/006),
actor classes (AI section 2.3), handles and the native table (VM-030, VM section 6).

Implementation clearance is **narrowed** (8.7) to what is read; the unread items of section 9 carry explicit
fallbacks that an implementer follows until an analyst reads them.

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
- NAV-003 (observed, `0x0055fa70`). A character's position record (contents and snapshot rules: ANIM-021)
  includes, for this specification: layer index, current sector (a polygon of 2.2, by number), current
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
        u32 sector_mask          stored; no routine read consumes it (unknown); 0 on every retail sector
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
- NAV-012 (observed, `0x004eb120`, `0x004e99e0`, `0x0051fdc0`). Sector outlines and obstacle outlines are
  **wall polygons**: every edge is a blocking segment on the sector's layer carrying the enabled state of its
  polygon and an oriented unit normal (perpendicular to the edge; its side is chosen at load from whether the
  polygon is a floor or an obstacle, so that, with the maps' authored winding, the normal points to the free
  side of the wall). A sector outline is additionally a floor (its interior is walkable); an obstacle's interior
  is not.
- NAV-013 (observed, `0x004eb120`, `0x004ebbd0`). A kind-1 sector is a lift sector; the ` AZ ` chunk attaches
  its ends (2.6). An ` AZ ` record naming a sector that is not kind 1 is a load error.
- NAV-014 (observed, `0x00558670`, `0x00559660`). Path requests name a sector by polygon number; the program
  converts it to the sector's position within its layer. Converting a number that is not a sector (an obstacle,
  or an unknown number) reports an error and yields position 0, which the original then uses (it searches the
  first sector of the layer); OpenSherwood rejects such a request instead (8.4).

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
  encoded independently of the polygon lists and must be kept as read. On five maps the region count of every
  sector equals 1 + its obstacle count and every node is a vertex of the sector's polygons; on Croisement02,
  Croisement03, Nottingham and York four sectors have one region fewer and some nodes are not polygon vertices
  (7.1). Regions are therefore *usually* the sector's polygons (inferred, medium); an implementer reads the
  graph and never derives it.
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
  32-bit word after it is the edge's availability mask (2.9), nonzero on Croisement01, Croisement02,
  Croisement03, Nottingham and York (Croisement01 edge 12: 512).
- NAV-025 (observed, `0x00553a10`, `0x00554b80`, `0x00556490`; medium). The two node vectors are stored and
  kept but none of the search, opening, reconstruction or wedge routines reads them; the wedge test is
  axis-aligned (NAV-143). Their orientation relative to the polygon's vertex order is inconsistent in the data
  (Lincoln: 149 of 633 nodes match a next/previous-vertex reading) and must not be regenerated. Purpose:
  unknown.

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
  on the area's link layer. Areas are never walkable ground.
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
  wall (it blocks nothing), it is the door's click target (3.1) and native 191 enables or disables it (6). A door
  whose leaf polygon is empty has no click target.
- NAV-041 (observed, `0x0051b110`, `0x0051b2a0`). For types 2, 6 and for type 4 when the lift is a climb, the
  threshold is moved 60 px (65 px for type 6) further along the direction from the threshold to the far side
  (the adjusted threshold is the door's point for native 64). Each door has a crossing cost:
  `|B point - A point| + 50` (types 0, 3..7) or `+ 100` (types 1, 2).
- NAV-042 (observed, `0x0057e060`, `0x004ebbd0`, `0x005716a0`). A lift record refers to a kind-1 sector by
  number; `lift_type` 1 = stairs, 2 = ladder, 3 = climb; 0 is corrected to 1 with a warning. The lift's door with
  the largest `ay` is its lower end, the one with the smallest `ay` its upper end. Lift doors are appended to the
  same flat door list as the `FARM` doors (chunk order `FARM` then ` AZ ` on every retail map), so native 4
  addresses them after the `FARM` doors (Lincoln: 59 + 20 = 79 entries). The list also receives the jump lines
  of `PPPP` as non-door entries (section 9).
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
  `(layer, index)` references that switch bonds and door leaves on or off, and the parameters of the availability
  toggle of NAV-057. The complete layout, and which fields carry the toggle parameters, is unread (section 9,
  fallback 8.7).

### 2.8 The cell grid

- NAV-050 (observed, `0x004eb120`, `0x004e8070`, `0x004ed690`, `0x004e8bd0`). Every layer index of NAV-010
  has a grid of 64 x 64 px cells over the background (`cell = trunc(coordinate) >> 6`, clamped to the grid).
  A cell knows the polygons whose outline meets it, the wall segments (with their enabled state and normal), the
  bonds and the sight lines that meet it. Every geometric query of this specification examines the cells of
  **one** layer; nothing is tested across layers.

### 2.9 Availability masks and the per-sector state (dynamic navigation)

- NAV-055 (observed, `0x005563f0`, `0x00556470`, `0x005559b0`, `0x00555a10`, `0x0055aa60`). Every sector
  (each layer, each sector position) has a 32-bit **state word** of sixteen 2-bit fields. At level
  initialisation, after the map is loaded, every state word is `0x55555555` (each field = `01`); the loader's
  interim value is never visible. The state words are part of the save game; after a load the availability of
  NAV-056 is recomputed from the restored words.
- NAV-056 (observed, `0x00555a10`). An object with mask `m` (a node's `node_mask`, an edge's `edge_mask`, an
  obstacle's `obstacle_mask`) is **available** in a sector with state `s` iff `(m & s) == m`. Mask 0 is always
  available; a mask with bit `2k` set requires field `k` = `01` (the initial state); a mask with bit `2k+1` set
  requires field `k` = `10`. Only available nodes and edges take part in the search (3.5); an unavailable
  obstacle is disabled (its polygon leaves the cells, its wall segments stop blocking) and an available one is
  enabled.
- NAV-057 (observed, `0x004d28e0`, `0x00556230`, `0x005559b0`; the parameters' origin in the patch record is
  unread). **Applying a patch** toggles field `k` of sector `(layer, number)` named by the patch between `01`
  and `10` and recomputes that sector's availability. Its effect on path requests by state (NAV-147): a request
  **queued** is later searched against the new availability; a request **executing** at that moment is
  abandoned and searched again from scratch; a request **completed but not yet consumed** is discarded and
  searched again from scratch (it is still the queue head); a result already **consumed** (turned into move
  actions) is not revised. When the patch asks for it, every actor on that layer and sector whose walker box
  meets a newly enabled obstacle is marked and receives an eviction element whose behaviour is unread (section
  9, fallback 8.7).
- NAV-058 (observed, `0x00555a10`, `0x00556260`; ordering consequence inferred, medium). **Ordering after a
  toggle.** In the original, a node or edge that becomes unavailable leaves its list, and one that becomes
  available again is appended at the **end** of its list (the region's node list, the node's edge list); after
  a save and load, availability is recomputed from the file order, so the lists are the file order filtered by
  availability. Consequently the original's tie-breaks of NAV-141 can differ between "after a toggle in this
  session" and "after loading the same state". OpenSherwood's choice is 8.3 (canonical file order, a declared
  deviation for the in-session case); test 7.2-14 fixes both orders.

## 3. Behaviour

### 3.0 Execution order per tick and the request states

- NAV-090 (observed, `0x004c6ef0` via ANIM-101, `0x004d23d0`, `0x005532a0`, `0x005546e0`, `0x00554750`).
  A path request is in exactly one state: **queued**, **executing**, **completed** (result not yet consumed),
  **consumed** (it no longer exists) or **failed-pending** (an empty result, NAV-146). In the original, one
  point of the level tick (the *consumer slot*, before the element updates of ANIM-101) does, in this order:
  (1) if a request is completed, its result is consumed (NAV-146) and the request removed; (2) if no request is
  executing and the queue is not empty, the queue is ordered (NAV-147) and its head becomes executing. The
  search itself runs off the game thread and completes at an unspecified wall-clock moment; a request can
  therefore be consumed only at a consumer slot after the slot at which it started, i.e. a request created during
  the element updates of tick `t` starts at the slot of `t + 1` and is consumed at the slot of `t + 2` at the
  earliest, later if the search takes longer. Applying a patch (NAV-057) is a further point at which the head of
  the queue starts executing. Cancelling an element removes its request from any state; an abandoned execution
  produces nothing. OpenSherwood's deterministic policy for the same states is 8.1.

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
- NAV-101 (observed, `0x004e9920`). **Alternate resolution**, used when a level flag is set (which flag:
  unread, fallback 8.7): the layers are scanned from the top; the first layer that does not answer "nothing" is
  remembered; the **second** such layer's answer is returned (with lift-shape replacement); if there is no
  second, the first one's answer is returned. Two stacked floors under the cursor give the lower one.
- NAV-102 (observed, `0x004debf0`). The cursor is **valid** (walk cursor) when the hit is a patch target, a
  walkable sector (floor with wall edges), a door leaf or a jump zone; otherwise **invalid** (cross cursor). A
  patch target with its refuse byte set makes the selected character play its refusal reaction instead of
  walking. When no layer answers, the target layer defaults to the selected character's layer.

### 3.2 Clearance and straight walks

- NAV-110 (observed, `0x004f5750`, `0x004ed070`, `0x004ecf60`, `0x00608f10`). **Clearance of a box** on a
  layer holds iff the box overlaps the level rectangle (a box wholly outside the map is not clear; partial
  overlap counts) and no enabled wall segment of the layer (sector outlines and obstacles alike) meets it.
  Clearance says nothing about sector membership: a box that overlaps the map but lies on no sector is clear.
- NAV-111 (observed, `0x004f5890`, `0x0051fdc0`; the normal's side per NAV-012). **Unsticking a box.** First,
  a box that does not overlap the map is translated onto it: if its right edge is left of the map it is moved so
  that its left edge is at `x = 0`, if its left edge is right of the map so that its right edge is at the map
  width; likewise vertically. Then at most 50 rounds; a round takes the enabled wall segments meeting the box in
  cell order and, for each segment whose oriented normal `n` (NAV-012) has the box centre on its `n` side
  (`n . (centre - p1) > 0`, `p1` an end of the segment), examines the box's corners in the order (min x, min y),
  (max x, max y), (max x, min y), (min x, max y): a corner whose signed distance `d = n . (corner - p1)` is
  below 0.1 moves the **whole box** along `n` by `1 - d`, so that the corner comes to lie exactly 1 px on the
  `n` side; later corners are measured after that translation. A segment whose `n` side does not hold the box
  centre does not move the box in that round. The box is clear when a round meets no segment; otherwise, after
  50 rounds, unsticking fails and the box stays where the last round left it. A diagonal wall therefore
  produces a diagonal translation, and two walls may move the box twice in one round, the second measured after
  the first.
- NAV-112 (observed, `0x00556990`; `0x004f6c20` is the live variant the orders use). **Corridor test** from `p`
  to `q` for half-size `(w, h)`: the corridor is the rectangle spanned by the two boxes of half-size
  `(w - 1, h - 1)` centred at `p` and `q`, its sides taken from the boxes' corners according to the signs of
  `q - p` (a purely horizontal or vertical leg gives an axis-aligned box); it is free iff no enabled wall
  segment of the layer, taken from the cells the corridor's bounding box covers, crosses a side of the corridor or
  has an end inside it.

### 3.3 The route across sectors (doors as a graph)

- NAV-120 (observed, `0x0051a690`). At load, for every pair of doors that share a sector on one of their
  sides, a **link** is recorded on that sector with length = the straight distance between the two doors' points
  on that sector (each door's A or B point, whichever faces the shared sector).
- NAV-121 (observed, `0x004f9a80`, `0x004f9d60`, `0x004fa6f0`). **Route search** from the character (position
  `P`, sector `S0`) to `target_sector` and target point `T`. Doors are examined in order of least
  `f = door_cost + g + h`, newest-queued first among equal `f`, where for a door `D` examined as a crossing from
  sector `X` into its other side `Y`: `door_cost` is NAV-041; `g` is the length walked to `D`'s point on `X`:
  from `P` for the doors of `S0`, otherwise the `g` of the door it was reached from plus that door's cost plus
  the link length; `h = |D's point on Y - T|`, fixed when `D` is first reached and not recomputed. The doors of
  `S0` that the character may pass from `S0` (NAV-170, planning form) are the initial set. Examining `D`: if
  `Y == target_sector`, the route is `D`'s chain of predecessors (at most 102 doors; longer chains are a
  failure: no route). Otherwise each link of `D` on `Y` to another door `N` is a candidate unless it is the link
  `D` was reached by or it is excluded by NAV-123; `N` is reached through `D` when it has not been reached before
  or its new `g` is smaller than its recorded one; it joins the examination only if the character may pass it
  from `Y` (planning form). No route when nothing remains to examine.
- NAV-122 (observed, `0x004fa080`). A route **to a door** (a click on a leaf) is the same search with the door
  itself as the goal.
- NAV-123 (observed, `0x004f9d60`; orientation dependence confirmed). **Link exclusion.** Let `D` be examined
  as a crossing into its side B (side A respectively). A link from `D` to `N` is excluded when the two doors'
  recorded **side-A** sectors are equal (their recorded **side-B** sectors, respectively). The comparison uses
  the doors' serialised sides, not their sides relative to the shared sector: for two parallel doors `D1`
  (A = X, B = Y) and `D3` both leading from `Y` to `X`, the link is excluded when `D3` is recorded as
  (A = X, B = Y) and followed when `D3` is recorded as (A = Y, B = X) (test 7.2-8).

### 3.4 The walk sequence

- NAV-130 (observed, `0x00582640`; element kinds by role; their ids and completion rules are ANIM section 3.4
  and VM section 6). Given a character, a target point with `(sector, layer)` and the order flags, the sequence
  of elements pushed to the character is:
  1. if the character stands on a lift end: a walk to that end's point on its current side;
  2. same sector: one **walk** to the target (NAV-140);
  3. other sector: the route of NAV-121 (NAV-122 for a door target); for each door `D` of the route in order,
     with `near` / `far` its points on the entered / left side:
     - if `D`'s far side is not a building: a walk to `near`, then a **door-approach test** with tolerance 10 px
       at `near` (NAV-150a);
     - if it is a building: (unless `D` is the first element) a wait of 50 frames, then a wait of
       `(r1 & 15) + (r2 & 15)` frames with `r1`, `r2` two consecutive draws of the random stream (AI-005, drawn
       while the sequence is built, in that order), then an **enter** at `near` facing the door;
     - if `D` is a door from `FARM` / ` AZ `: when the character is a player character, `lock_pc` is set and it
       has the lock-picking ability, a **lock-pick** at `far` and a **pick lock of D**, and the sequence
       **ends here** (the rest of the route is dropped); otherwise, when the far side is a ladder lift, a
       **ladder** element for `D`; then a **pass door D** (NAV-171) and a door-approach test with tolerance
       10 px at `far`;
     - if `D` is a jump line: a **jump** element with the line's two points (unread, fallback 8.7);
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
- NAV-141 (observed, `0x005547c0`, `0x00557e70`, `0x00553e60`, `0x00556490`, `0x00554360`, `0x005581a0`;
  `0x00558210` tail and `0x005557c0` unread, fallback 8.7). **Search** for `(layer, sector, start, goal,
  flag)` with half-size `(w, h)` of size class 0, over the sector's regions, seeing only available nodes and
  edges (NAV-056), in their list order (NAV-058). Required results:
  0. **Goal check**: the enabled wall segments of the layer meeting the box of half-size `(w - 1, h - 1)` around
     `goal` are collected and decide whether the goal is admissible; the decision's tail is unread; the fallback
     (8.7) is: the goal is inadmissible iff that collection is non-empty. An inadmissible goal is "no path".
  1. **Direct leg**, only when the request's unstuck flag is 1: if `start -> goal` crosses none of the sector's
     barrier segments (2.2) and the corridor (NAV-112) is free, the path is `[goal]`.
  2. **Opening**: the search window is the bounding box of `start` and `goal` grown by 400 px on every side. An
     available node is *openable* when its corner is in the window, its walker box (half-size `(w, h)`) does not
     contain `start`, its corner is not separated from `start` by a barrier segment, and at least one of its
     candidates (NAV-021) both satisfies the quadrant rule NAV-143 with respect to `start` and has a free
     corridor `start -> candidate`. Every openable node, taken in list order, is opened with `g = |start -
     corner|`, `h = |corner - goal|`, the set of its passing candidates remembered.
  3. **Examination order and answer**: nodes are examined in order of least `f = g + h`, the **most recently
     opened first** among equal `f`; a node whose byte is 5 or 10, or whose corner is separated from `goal` by a
     barrier segment, cannot be the answer; otherwise the first examined node with a candidate that satisfies
     the quadrant rule with respect to `goal` and has a free corridor `candidate -> goal` is the **answer**, its
     passing candidates remembered. "No path" when nothing remains to examine or the request was abandoned.
  4. **Reaching neighbours**: when an examined node (byte not 5 or 10) is not the answer, every available edge
     that enters it and has a record for the size class reaches the far node `A` with cost `g + edge cost`; `A`'s
     recorded `g` is the least cost by which it has been reached, its recorded predecessor the edge that produced
     that least cost, its `h = |A - goal|` fixed when first reached; a reaching that improves `g` (and is below
     the answer's `g` if one were recorded) makes `A` open again (its position in the examination order is that
     of a newly opened node).
  5. **Waypoints** (NAV-144, NAV-145): the answer's predecessor chain fixes the sequence of graph nodes from the
     first opened node to the answer; the path is `start`, then the corner walks at those nodes in that order,
     then `goal`.
  6. **Smoothing**: only when the path has more than three points: for `i = 2, 3, ...` while `i` is inside the
     list, with `d = p[i] - p[i-2]`: if the corridor from `p[i-2] + 0.00005 d` to `p[i] - 0.00005 d` is free
     (NAV-112), `p[i-1]` is removed and the same `i` tested again; otherwise `i` advances.
- NAV-142 (observed, `0x00552290`). The answer is the first node found goal-visible in examination order (the
  program keeps a "candidates to collect" count initialised to 1); the result is not necessarily the shortest
  path.
- NAV-143 (observed, `0x00553a10`, disassembled; the argument order at the call site `0x005541db`-`0x005541e5`
  and the subtraction at `0x00553a72`-`0x00553a81`, the flag tests at `0x00553b55`-`0x00553b84` and
  `0x00553c27`-`0x00553c4f`). **Quadrant rule.** For candidate bit `k` of a corner, at
  `o = corner + (s_x w, s_y h)` with `(s_x, s_y)` = (-1,-1) for bit 1, (+1,-1) for bit 2, (+1,+1) for bit 4,
  (-1,+1) for bit 8, and a point `Q` (the start in the opening, the goal in the examination), let
  `v = Q - o`. The candidate passes iff `(s_x v_x >= 0 and s_y v_y < 0)` or `(s_y v_y >= 0 and s_x v_x < 0)`:
  `Q` lies on the candidate's outer side (or exactly on its line) in one axis and strictly on its inner side in
  the other. Per bit, with `v = (v_x, v_y)`:

  | bit | first form | second form |
  |---|---|---|
  | 1 `(-w,-h)` | `v_x <= 0 and v_y > 0` | `v_y <= 0 and v_x > 0` |
  | 2 `(+w,-h)` | `v_x >= 0 and v_y > 0` | `v_y <= 0 and v_x < 0` |
  | 4 `(+w,+h)` | `v_x >= 0 and v_y < 0` | `v_y >= 0 and v_x < 0` |
  | 8 `(-w,+h)` | `v_x <= 0 and v_y < 0` | `v_y >= 0 and v_x > 0` |

  Review 20 finding 9 states the opposite ownership of the axis boundaries; the analyst re-read the call site
  and the routine and maintains this table (`v` is `Q - o`, the first form is tested first, the second only
  when the first fails). Test 7.2-4 encodes it so that either reading is falsifiable by an implementer.
- NAV-144 (observed, `0x00554b80`, `0x005553d0`, `0x005568f0`, `0x00556920`). **Corner walk.** At a graph node
  `N` of the path let `U` be `N`'s usable candidates (`offset_bits`), `G` the candidates of `N` from which the
  leg towards the goal departs (for the answer node: the candidates that passed the goal test; for any other
  node: the partners, through the record of the edge to the next node, of the candidate chosen at that next
  node), and `S` the candidates of `N` at which the leg from the start side arrives (for the first node: the
  candidates that passed the opening test; otherwise the candidates named for `N` by the record of the edge from
  the previous node). The waypoints at `N`, in travel order, are:
  - if `G & S` has exactly one bit: that candidate alone;
  - else if `G` has exactly one bit `g`: the shorter of the two runs from `g` through consecutive members of `U`
    around the corner to the first member of `S` reached (one run in the sense 1 -> 2 -> 4 -> 8 -> 1, one in the
    opposite sense), listed from that member of `S` to `g`; a run that meets a bit outside `U` before reaching
    `S` is void; when both runs have equal length the run whose travel order goes 1 -> 2 -> 4 -> 8 (clockwise on
    the screen) is taken;
  - else: over every `g` in `G` in increasing bit order, both runs as above, counting the candidates passed
    strictly between `g` and the member of `S`; the run with the fewest such candidates is taken; ties go to the
    smaller `g` and, for one `g`, to the run whose travel order goes 8 -> 4 -> 2 -> 1 (counter-clockwise).
  The candidate chosen at `N` for the leg towards the start (the member of `S` reached) determines `G` at the
  previous node through the record's pairs (NAV-023). There is no corner walk at `start` itself.
- NAV-145 (observed, `0x005547c0`, `0x004d23d0`). The path list is `start` first and `goal` last; the consumer
  of NAV-146 skips `start` when the request's unstuck flag is 0 and keeps it (the corrected start) when the flag
  is 1.
- NAV-146 (observed, `0x004d23d0`, `0x00554560`). **Consuming a result**: the waypoints become one move action
  each (ANIM-208, with the request's gait and run flag), followed by the element's own completion. An empty
  result puts the request into the failed-pending state with the tick of arrival; when 100 ticks have passed the
  element fails and a player character plays its refusal reaction; whether the request is searched again in
  between is unread (fallback 8.7: it is not).
- NAV-147 (observed, `0x005532a0`; the reference box's meaning unread). **Queue order** when the head is taken:
  requests are ordered by priority 0 (first) to 3, stable for equal priority; before ordering, a non-player
  request at priority 3 whose reference box contains its reference point is promoted to 2 and one at 2 whose box
  no longer contains it is demoted to 3 (player requests stay at 0; the box and point are the request's own,
  their meaning is unread, fallback 8.7).

### 3.6 Movement along the waypoints

- NAV-150 (observed, `0x00467a50`, `0x005fc700`, `0x00561040`). Movement is the sibling specification's:
  per-frame displacement ANIM-200/201/206, turning ANIM-203, the collision-aware move ANIM-204/205, move-action
  arrival ANIM-208, layer changes inside action lists ANIM-311. Two facts belong here:
  (a) the **door-approach test** element of NAV-130 (ANIM section 3.4, kind 2) is an immediate two-way
  branch: in its point form it is *done* when `max(|dx|, |dy|) < tolerance + 5` px between the character and its
  point and otherwise *cancels the rest of the sequence*; in its sector form it is done when the character's
  sector is the named sector and cancels otherwise;
  (b) inside the collision-aware move, the mover's **proximity query** is the axis-aligned rectangle spanned by
  `position + r . dir` and `position - r . dir`, where `dir` is the unit movement direction and `r` = the
  element's proximity radius field plus 60 px (the field's source is unread, fallback 8.7: 0). A character
  becomes a proximity partner when it is displayed, is not the mover and not the element the mover carries, is
  on the mover's layer and sector, is eligible by class and state (ANIM-205a), its **position** lies in the
  query rectangle, and the dot product of `(partner position - mover position)` with `dir` is at least 5 px. A
  non-character object becomes a partner when it answers "blocking" and its position lies in the rectangle (no
  dot-product test). The partner's response is ANIM-205 (open).

### 3.7 Bonds: changing area (and height)

- NAV-160 (observed, `0x004af540`, `0x00462aa0`; the multi-bond chain order inferred). After a mobile element
  moved from `p` to `q` on its layer, the bonds of the cells crossed by `p -> q` are collected; duplicates (same
  segment and areas) are dropped; bonds the step does not actually cross are dropped. Exactly one bond: if the
  element's area is the bond's `area_a` or `area_b` it becomes the other; otherwise the crossing is illegal and
  the program recovers by looking, in the element's cell on its layer, for a linked area whose screen outline
  contains the position, then at `position + 2 * direction`; failing that, the area is unchanged. Several bonds:
  they are ordered into a chain in which consecutive bonds share an area and crossed in that order. Bonds never
  change layer or sector; those change inside the door and lift action lists (ANIM-311).

### 3.8 Doors

- NAV-170 (observed, `0x0051abe0`, disassembled; the type dispatch at `0x0051ae0c`). **Admission.** A character
  `C` may pass door `D` from side `s` (`s = A` when coming from `D`'s side A, `s = B` otherwise) in the
  *planning* form (route search) or the *crossing* form (NAV-171) iff all of the following hold. Classes are
  those of AI section 2.3: "player" = a player character, "soldier family" = the family that contains soldiers,
  player characters and civilians, "plain soldier" and "civilian" its subclasses; "class-0 family" = the
  remaining family.
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
- NAV-171 (observed, `0x00467a50`; ANIM-311 for the moment of the layer change). **Crossing** a passage door
  (types 0, 3, 7) from side `s`: the crossing admission is re-checked (failure fails the element); `C` records
  the door and side; then, in order: a move to the threshold, a marker, a move to the far point, a marker; the
  character's layer and sector change at the marker between the two moves (ANIM-311). A player entering a
  sector whose `unknown_flag` is set gets a different pair of actions there (the flag is 0 on all retail
  sectors; fallback 8.7: as the plain pair). Building doors (1, 2) run the building entry (3.9); lift ends (4, 5,
  6) run the lift sequence (3.10).
- NAV-172 (observed, `0x00579850`, `0x00579880`, `0x005798a0`, `0x005787f0`, `0x0051b660`). **State
  changes.** The natives of section 6 set the lock bytes; clearing `lock_pc`, `lock_soldier` or `lock_civilian`
  also **opens** the door. Native 191 enables or disables the leaf's click target. Applying a patch swaps the
  active lock set with the alternate set of every door the patch names. No native closes a door; closing, and
  the player's opening by a click on a leaf, go through the player's door interaction, which is unread (fallback
  8.7).

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
  are a load error) and one trailing byte per building (kept; meaning unread, no behaviour depends on it in the
  routines read). An initial occupant is placed on layer index `layer_count - 1` (not `layer_count`), in the
  building, at the B point of the building's first door, "not displayed".

### 3.10 Lifts

- NAV-200 (observed, `0x00467a50`, `0x0046a900`, `0x00469770`, `0x0046a000`; the action ids are the animation
  table's, ANIM-010; the moment of the layer change is ANIM-311). Using a lift end from side `s`: admission
  (NAV-170), then `C` records the door and side and plays, by `lift_type`: **stairs** - a move to the threshold
  with the current gait, a marker, a move to the far point with the gait's stair variant, a marker; **ladder** -
  mount at the threshold, the climb loop whose length is the animation table's loop length for that action, a
  marker, dismount at the far point (distinct actions for up and down and for the carrying variant of
  civilians); **climb** - the ivy actions likewise, with the distinct set for type-6 ends. The flight is an
  ordinary kind-1 sector (walking on it is ordinary movement).

### 3.11 Sight volumes

- NAV-210 (observed, `0x004e99e0`, `0x005a3810`; the consumer unread). The `WOAW` volumes and the material
  polygons are the geometry kept for line of sight; the perception test is open (section 9; the AI
  specification owns it).

## 4. Claims

Claims are inline (NAV-001 .. NAV-210 above, NAV-300 in section 6); each names its status, addresses and
confidence. Claims with an unread part and their fallbacks are collected in 8.7.

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| grid cell | 64 | px | `0x004ed690`, `0x004e8bd0`, `0x004ed070` | high |
| walker half-size, size class 0 | (6, 4) | px | `STAT` graph header, all nine maps | high |
| corridor and goal box half-size | (w - 1, h - 1) | px | `0x00556990`, `0x00558210` | high |
| search window margin | 400 | px | `0x00553e60` | high |
| walk-order clearance box growth | 0.5 per side | px | `0x00467a50` | high |
| unstick rounds | 50 | - | `0x004f5890` | high |
| unstick corner target distance | 1; corners below 0.1 are moved | px | `0x004f5890` | high |
| door crossing cost | `|B - A| + 50` (types 0, 3-7), `+ 100` (types 1, 2) | px | `0x0051b2a0` | high |
| threshold push | 60 (types 2, 4-on-climb), 65 (type 6) | px | `0x0051b110` | high |
| route length limit | 102 | doors | `0x004f9a80` | high |
| door-approach test | `max(|dx|, |dy|) < tolerance + 5` | px | `0x00467a50`, `0x005fc700` | high |
| door approach tolerance | 10 | px | `0x00582640` | high |
| building entry waits | 50; then `(r1 & 15) + (r2 & 15)` | frames | `0x00582640` | high |
| proximity query half-extent | element radius field + 60, along the movement direction | px | `0x00561040` | high |
| proximity partner threshold | dot product >= 5 | px | `0x00561040` | high |
| path failure window | 100 | ticks | `0x004d23d0` | high |
| smoothing endpoint shrink | 0.00005 of the leg | - | `0x005547c0` | high |
| building door search radius (native 64) | squared distance, truncated to integer, strictly below 90000 | px^2 | `0x00574860` | high |
| answers collected before stopping | 1 | nodes | `0x00552290` | high |
| initial sector state | `0x55555555` | - | `0x00556470` | high |
| building capacity | 65535 | - | `0x0057e4d0` | high |

## 6. Interfaces to the script VM

Native ids are the `.scb` ids (VM section 6 holds the whole table; the rows here are the navigation ones and
supersede the older `scb.md` rows). The program registers 265 argument adapters (ids 0..264, table filled at
`0x00407400`-`0x00408200`).

- NAV-300 (observed at the adapters `0x00404b20`, `0x00404ba0`, `0x00406810`, `0x004068d0`-`0x00406960`,
  `0x004069c0`, and the handlers). Adapters of 186-191 normalise the value argument to `0` or `1` (`v != 0`)
  and return 0; 182's adapter zero-extends the byte. The handlers of 182 and 186-189 use the door handle without
  a null check: a null handle is undefined behaviour in the original (a memory fault). Native 191 checks the
  handle (null: logged error, nothing) but not the leaf: a door without a leaf is undefined in the original.
  Native 8 indexes without a bounds check for any value. OpenSherwood's behaviour for these cases is 8.4.

| Id | Arity | Meaning | Original failure / edge cases | Where |
|---|---|---|---|---|
| 4 | (i) -> door | door `i` of the flat door list: `FARM` doors in file order, then the lift doors (NAV-042) | `i = -1` -> none; `i` out of range -> logged error, none | `0x005716a0` |
| 8 | (i) -> building | building `i` (kind != 0 `FARM` records in order) | no bounds check: any invalid `i` reads outside the list (undefined) | `0x00571760` |
| 64 | (actor, location, flag) -> bool | walk into a building: among the entries of the flat door list that are door objects (jump lines excluded by the original), the one whose **adjusted threshold** (NAV-041) is nearest to the location's point by squared distance truncated to an integer, strictly below 90000, an earlier entry winning equal scores; issues the walk sequence (NAV-130) to that door's B point on its side-B sector and layer; returns whether it was issued | location not a point, or no door qualifies: logged error, 0 | `0x00574860` |
| 98 | (actor, building) -> bool | `building` none: 1 iff the actor's sector is **a building** (the actor is inside one); else 1 iff the actor's sector is that building | - | `0x00577cf0` |
| 152 | (actor) | take the actor out of its building (NAV-191) | actor not human or not inside: logged error, nothing | `0x00577150` |
| 156 | (actor, building) | put the actor inside (NAV-190) | actor none or not human: logged error, then proceeds | `0x005781f0` |
| 182 | (door) -> int | the door's `lock_pc` byte, zero-extended | null handle: undefined (NAV-300) | `0x00579810` |
| 186 | (door, v) -> 0 | `lock_pc := (v != 0)`; when clearing, the door opens | null: undefined | `0x00579850` |
| 187 | (door, v) -> 0 | `lock_x := (v != 0)` (the byte tested with ability 0x1c) | null: undefined | `0x00579870` |
| 188 | (door, v) -> 0 | `lock_soldier := (v != 0)`; when clearing, opens | null: undefined | `0x00579880` |
| 189 | (door, v) -> 0 | `lock_civilian := (v != 0)`; when clearing, opens | null: undefined | `0x005798a0` |
| 191 | (state, door) -> 0 | the door's leaf click target becomes enabled iff `state != 0` | door none: logged error, nothing; door without a leaf: undefined | `0x005787f0` |

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

Further checks: `FARM` and ` AZ ` consumed exactly (doors 3, 1, 5, 42, 59, 59, 100, 5, 123; lifts 0, 0, 0, 12,
10, 10, 12, 4, 21 in the map order above); every lift sector is kind 1 and the skipped word equals its layer;
edge costs within 1.5 px of the corner distance; every edge appears in exactly its node B's list and has a
reverse; region counts equal 1 + obstacle count except for Croisement03 (layer 0, sector 0), Nottingham (0, 0)
and (2, 7), York (0, 0), which have one fewer; edge masks are nonzero on Croisement01/02/03, Nottingham and York
and zero elsewhere; all 140 passage-door A points lie inside sector A and all B points inside sector B (138 and
133 of them exclusively so).

### 7.2 Synthetic tests (complete inputs, exact outputs)

The **fixture F1** used by several tests: one layer, one plain sector with outline `(0,0) (400,0) (400,200)
(0,200)` and one obstacle with outline `(180,80) (220,80) (220,120) (180,120)` (vertex order as written), no
barrier segments, size class `(6, 4)`, sector state `0x55555555`. Graph: one region for the outline (its four
corners, `offset_bits` 4, 8, 1, 2 for `(0,0)`, `(400,0)`, `(400,200)`, `(0,200)`) and one region for the
obstacle with nodes in the order `N1 (180,80)` bits 11, `N2 (220,80)` bits 7, `N3 (220,120)` bits 14,
`N4 (180,120)` bits 13, all node masks 0 unless a test says otherwise. Edges (both directions, cost = corner
distance, mask 0): `N1-N2` (record pairs joining {1,2} of N1 with {1,2} of N2, all four), `N2-N3` (pairs joining
{2,4} of N2 with {2,4} of N3), `N3-N4` (pairs joining {4,8} of N3 with {4,8} of N4), `N4-N1` (pairs joining {8,1}
of N4 with {8,1} of N1); no other edges. Start `(100,100)`, goal `(300,100)`, unstuck flag 0.

1. **Availability.** In F1 give N2 mask 2. With state `0x55555555` N2 is unavailable; after toggling field 0 it
   is available; toggling again removes it. An obstacle with mask 1 is enabled at the initial state; with mask
   2 it is disabled and its four wall segments do not block. A save taken after a toggle restores the toggled
   availability.
2. **Clearance.** A `12 x 8` box centred at `(200,100)` (inside the obstacle, meeting its walls) is not clear; the
   same box at `(100,100)` is clear; a box at `(100,100)` on a layer with no sector at all is clear; a box
   wholly outside the map (`(-40,-40)` to `(-28,-32)`) is not clear; a box straddling the map's left edge
   (`(-6,100)` to `(6,108)`) is clear when no segment meets it.
3. **Corridor.** Start `(100,100)`, goal `(200,100)`, class `(6, 4)`: the corridor's sides are at `y = 97` and
   `y = 103`. A segment `(150,96)-(150,104)` blocks; `(150,104)-(150,120)` does not; `(150,90)-(150,103)` blocks
   (an end inside).
4. **Quadrant rule** (NAV-143), corner `(100,100)`, candidate bit 1 at `(94,96)`: `Q = (94,120)` passes
   (`v = (0,24)`: first form), `Q = (120,80)` passes (second form), `Q = (50,80)` fails, `Q = (120,120)`
   fails, `Q = (50,96)` fails (`v = (-44,0)`: first form needs `v_y > 0`, second needs `v_y <= 0 and v_x > 0`).
   Candidate bit 4 at `(106,104)`: `Q = (130,90)` passes, `Q = (80,130)` passes, `Q = (130,130)` fails,
   `Q = (80,90)` fails.
5. **Corner walk** (NAV-144) at a node with `U = 15`: `G = {1}`, `S = {4}`: both runs from 1 reach 4 in two
   steps; the clockwise travel order is taken: waypoints in travel order `4, 8, 1`. `G = {1}`, `S = {2}`: the
   run 1 -> 2 (one step) beats 1 -> 8 -> 4 -> 2: `2, 1`. `G = {3}` (bits 1 and 2), `S = {4}`: from `g = 1`
   both runs pass one candidate; from `g = 2` the run 2 -> 4 passes none: `4, 2`. `G = {1}`, `S = {1}`: `1`.
   `U = 7` (bit 8 unusable), `G = {1}`, `S = {4}`: only the run 1 -> 2 -> 4 is valid: `4, 2, 1`.
6. **Search** on F1. Openable nodes: N1 through candidate bit 1 at `(174,76)` (`g = h` as below), N4 through
   candidate bit 8 at `(174,124)`; N2 and N3 are not openable (every candidate corridor from the start crosses
   the obstacle's left wall or fails the quadrant rule). `g = sqrt(6800)` and `h = sqrt(14800)` for both, so
   `f` ties exactly; N4 was opened later and is examined first; it is not goal-visible (its candidate `(174,124)`
   fails the quadrant rule for the goal); it reaches N3 with `g = sqrt(6800) + 40`, `h = sqrt(6800)`. N1 is
   examined next (`f = sqrt(6800) + sqrt(14800) < sqrt(6800) + 40 + sqrt(6800)`), not goal-visible, reaches N2
   with the same `f` as N3; N2 was opened later and is examined first: its candidate bit 2 at `(226,76)` passes
   the quadrant rule for the goal and its corridor to the goal is free: N2 is the answer. Corner walks: at N2
   `G = {2}`, `S = {1,2}` (the N1-N2 record) -> `(226,76)`; at N1 `G` = partners of 2 = `{1,2}`, `S = {1}` ->
   `(174,76)`. Path `(100,100), (174,76), (226,76), (300,100)`; smoothing removes nothing (both three-point
   corridors cross the obstacle). Consumed as three move actions (the start is skipped).
7. **Smoothing.** In F1 with the obstacle removed, a path `(100,100), (150,100), (200,100), (300,100)` collapses
   to `(100,100), (300,100)`.
8. **Route.** Sectors `X`, `Y`, `Z`; doors `D1` (A = X at `(200,100)`, threshold `(210,100)`, B = Y at
   `(220,100)`), `D2` (A = Y at `(400,100)`, threshold `(410,100)`, B = Z at `(420,100)`), `D3` (A = Y at
   `(220,300)`, threshold `(210,300)`, B = X at `(200,300)`), all type 0, open, no locks; character at
   `(100,100)` in X, player; target `(500,100)` in Z. Door costs 70 each. Initial: `D1` from A with `g = 100`,
   `h = 280`, `f = 450`; `D3` from B with `g = sqrt(50000)`, `h = sqrt(118400)`, `f` about 637.6. `D1` is
   examined first; entering Y through `D1`'s side B, links on Y are compared on their side-A sectors: `D1-D2`
   (X vs Y) and `D1-D3` (X vs Y) are followed; `D2` is reached with `g = 350`, `h = 80`, `f = 500`; `D3`'s new
   `g = 370` does not improve it. `D2` examined next: its far side is Z: route `D1, D2`. Variant: with `D3`
   recorded as (A = X at `(200,300)`, B = Y at `(220,300)`) the link `D1-D3` is excluded (X vs X) and the result
   is the same route; with `D2` closed there is no route.
9. **Admission.** Door `D` type 2, open, `lock_pc = 1`, `lock_x = 1`, `lock_civilian = 0`, `lock_soldier = 0`,
   "player barred" clear, building capacity 65535 with 0 occupants: a plain soldier from side A is refused; a
   civilian whose "cannot pass" test is false is admitted; a player without ability 0x1c is refused; the same
   player with ability 0x1c is admitted. The same door with `open = 0`: everyone refused. A ladder lift end
   (type 5, open, no locks) with lift kind 2: player admitted, plain soldier refused, civilian admitted.
10. **Natives.** `n186(d, 7)` sets `lock_pc = 1` and leaves a closed door closed; `n186(d, 0)` clears it and
    opens; `n182(d)` reads 1 then 0. `n191(0, d)` removes the leaf from click resolution: a click on the leaf
    then falls through to the sector below it. `n8(-1)` and `n8(1000)` on a two-building map return none
    (8.4).
11. **Click.** Two floors stacked at a point: ordinary resolution gives the upper; the alternate resolution
    gives the lower; an obstacle on the upper layer at the point gives "blocked" and no target in ordinary
    resolution.
12. **Door-approach test** with tolerance 10: a character at `(dx, dy) = (14, 0)` completes; at `(15, 0)` the
    element cancels the rest of the sequence at once; at `(14, 14)` it completes. Sector form: completes iff the
    character's sector is the named one.
13. **Scheduling** (8.1): a walk element that needs a search, executed during the element updates of tick `t`,
    is dispatched at the slot of `t + 1` and its move actions exist after the slot of `t + 2`; two such requests
    from a player character and a non-player character in the same tick are dispatched player first (`t + 1`)
    and non-player second (`t + 2`), consumed at `t + 2` and `t + 3`. A patch applied during the updates of tick
    `t + 1` (after the player's dispatch) makes the player's completed result be discarded and recomputed against
    the new availability; its move actions still exist after the slot of `t + 2`.
14. **Toggle, snapshot, restore** on F1 with N1 mask 2 (unavailable at the initial state). Initial search:
    only N4 is openable; the path is `(100,100), (174,124), (226,124), (300,100)` (N3 answers through bit 4 at
    `(226,124)`). After toggling field 0: with the original's in-session order (N1 appended after N4) the search
    examines N1 first among the tied openings and the path is again `(100,100), (174,124), (226,124), (300,100)`
    (N3 is opened before N2 and wins the tie); with the file order (after a save and load in the original, and
    always in OpenSherwood per 8.3) the path is that of test 6. A snapshot taken after the toggle restores state
    `0x55555556` for the sector (field 0 = `10`) and the file-order result.

### 7.3 Oracle procedures (pending, `harness/tools/original`)

Record, on Lincoln from the first mission's start, a click on the servant's hall floor and on a door leaf: the
route through the ramp, the stair lift and the passage doors of `layers-and-doors.md` section 4 must be the
one NAV-121 produces from the data (door costs and link lengths), and the arrival frames must match ANIM-208
within one frame.

## 8. Implementation choices, determinism, departures

1. **Scheduling policy (proposed deterministic replacement of the worker; requires the maintainer's
   approval as a deviation).** Per tick, in this order: (P1) *consumer slot*: a completed request whose
   delivery tick is at most the current tick is consumed (NAV-146); (P2) *dispatch*: if no request is executing
   and the queue is not empty, order it (NAV-147), take the head, run the search to completion synchronously
   within a work budget, and mark the request completed with delivery tick = current tick + 1; (P3) element
   updates, which submit requests (they enter the queue and are dispatched at P2 of the next tick at the
   earliest). A request submitted during P3 of tick `t` is dispatched at `t + 1` and consumed at `t + 2`; this
   equals the original's earliest case and never its later cases (the original may deliver later when its
   search runs long). Applying a patch during P3 (NAV-057): an executing request cannot exist during P3; a
   completed request is returned to the queue head, the availability is recomputed, and the head is dispatched
   again immediately (still delivered at its original delivery tick or later, never earlier than P1 of the next
   tick). Cancellation removes the request from any state. A search that exhausts the work budget is an empty
   result (failed-pending), a deviation recorded here; the budget is a constant of the engine, large enough
   that no retail map reaches it.
2. **Snapshot contract (ADR-0004).** Authoritative and hashed: per sector the state word (NAV-055) and, if 8.3 is
   not adopted, the availability list orders; per door the open flag, both lock sets, the "player barred" byte,
   the leaf's enabled flag; per building the occupant list and the "displayed" marks; per character the layer,
   sector number and area of NAV-003 (ANIM-021 for the rest); the request queue in order, each entry with its
   owner element, layer, sector number, start, goal, gait, run flag, priority state, unstuck flag, submission
   tick, reference box and point; every completed request with its owner, its waypoint list and its delivery
   tick; every failed-pending request with its owner and arrival tick. There is never an executing request at a
   snapshot boundary (P2 completes within the tick). A completed result is restored as data and never
   recomputed: it is authoritative for its delivery tick, and a patch between its computation and its delivery
   has already discarded it (8.1). Restore rebuilds availability from the state words.
3. **Availability order (proposed deviation, requires approval).** OpenSherwood always keeps nodes and edges in
   file order filtered by availability (the original's own order after a load). The original's in-session order
   after a re-enable (NAV-058) is not reproduced; the difference is confined to tie-breaks among equal `f` after
   a patch re-enables an object, within the same session. Test 7.2-14 fixes both orders.
4. **Invalid inputs (OpenSherwood decisions where the original is undefined).** NAV-014: a request naming an
   obstacle or an unknown number is rejected (the element fails). Native 8 with any index outside `0 ..
   count - 1` returns none. Natives 182 and 186-189 with a null or unknown handle: 182 returns 0, the setters do
   nothing. Native 191 on a door without a leaf does nothing. Native 64 keeps the original's selection rule
   (section 6) including its exclusion of jump-line entries.
5. **Numeric precision (deviation).** The original computes distances, costs, sums and cross products with
   80-bit intermediates and stores 32-bit results; the comparisons of NAV-121 (`door_cost + g + h`), NAV-141
   (`g + h`, `g + cost`) and NAV-143 are made on those intermediates. OpenSherwood evaluates every operation in
   `f32` in the order written (`door_cost + g` then `+ h`; `g + h`; `g + cost`). Results may differ from the
   original only when two candidates' sums are within about `1e-6` relative of each other; the acceptance
   tests use inputs whose ties are exact in `f32` (identical operands) and whose non-ties are separated by more
   than 1 px.
6. **Clock.** Frame counts of section 5 are logic frames; the engine's fixed 46.875 ms frame is
   ADR-0010's decision, the original's requested pacing is 40 ms and its realised cadence host-dependent.
7. **Fallbacks for the unread items (narrowed clearance).** Each is an `Assumption` variant (ADR-0008) with the
   behaviour below until an analyst reads the routine:
   - *Goal check tail* (NAV-141 step 0): the goal is inadmissible iff any enabled wall segment meets the goal
     box.
   - *Node reset* (`0x005557c0`): every node's search state is cleared before each search (no carry-over).
   - *Patch toggle parameters and eviction* (NAV-046/057): until the patch record's fields are read, a patch
     toggles nothing and evicts nobody; the missions on the five maps with nonzero masks (Croisement01/02/03,
     Nottingham, York) therefore keep the initial availability; an actor found inside a newly enabled obstacle
     (when toggles are implemented) is unstuck by NAV-111 at once.
   - *Player door interaction* (NAV-172): a click on a leaf routes to the door (NAV-122); on arrival, if the
     character is admitted (NAV-170, crossing form) the door is opened when closed; nothing closes a door except
     a script-driven patch (none read).
   - *Jump elements* (NAV-130): a jump line in a route fails the sequence (no jump).
   - *Failed-pending re-run* (NAV-146): none; the element fails after 100 ticks.
   - *Reference box of NAV-147*: non-player requests keep priority 1 (no promotion or demotion).
   - *Alternate resolution flag* (NAV-101): never set.
   - *`unknown_flag` pair of actions* (NAV-171): the plain pair.
   - *Proximity radius field* (NAV-150b): 0 (query half-extent 60 px).
   - *Proximity response and sliding* (ANIM-205): a blocked step does not move the element this frame.
   - *Line of sight* (NAV-210): owned by the AI specification's assumption.

## 9. Open questions

1. Line of sight: the routine and its rule (perception in `0x00486260` / `0x0048b5d0`; sight lines from
   `0x004e99e0`).
2. The sliding rule after a failed corridor test and the proximity response (`0x00561040` second half,
   `0x00563e90`, `0x00564020`-`0x00564390`, `0x00560840`; ANIM-205c); the source of the proximity radius field.
3. The goal check's decision tail (`0x00558210` after the segment collection) and the node reset before a
   search (`0x005557c0`).
4. Jump lines and jump zones (`PPPP`, `0x004fb510`, `0x0051e230`, `0x0051b6e0`, `0x00583630`, `0x0049dbc0`) and
   the jump element of NAV-130.
5. The patch record (`TUPO`, `0x0054eea0`, `0x0054f870`, `0x0054fe10`): which fields name the availability
   field, the sector and the layer of NAV-057; the eviction element pushed at `0x004d28e0`.
6. The building tenants' trailing byte (`0x004c1f90`).
7. The player's door interaction on a leaf click (the player-character method called from `0x004d7880`) and
   what closes a door.
8. `STAT` `sector_mask` use, `unknown_flag`, the second `WOAW` plane, the ` AZ ` trailer word, the node
   vectors' purpose (NAV-025).
9. The request reference box (NAV-147) and re-runs within the failure window (NAV-146).
10. Which level flag selects the alternate click resolution (NAV-101); which input reaches the gait-6 and the
    gait-10 click handlers (`0x004ccad0`, `0x004cd600`).
11. Lift and door action ids as animation names (`0x005bddb0` table; `sprite-animations.md`).

## 10. Provenance

- Ghidra project `re/ghidra/robinhood` (never committed); exported decompilation and inventories from
  `scripts/ghidra/`; `scripts/ghidra/peek.py` for data bytes; a capstone helper in `re/notes/nav/` for the
  routines whose decompilation lost stack arguments (`0x0051b2a0`, `0x0051abe0`, `0x00553a10`, `0x00553e60`,
  the worker at `0x005545c0`, the native adapters). The native table was recovered by scanning the registration
  code (`re/notes/nav/native_table.txt`, 263 slots, ids 0..264).
- Functions read, by purpose: loader and chunk dispatch `0x004c0510`, `0x004c3630`, `0x004c1f90`; sectors
  and graph `0x004eb120`, `0x00558670`, `0x0057cd60`, `0x0057ca80`; sight areas `0x004ef340`, `0x005a3810`,
  `0x005a5780`, `0x005a3450`; bonds `0x004ed9b0`, `0x0051faf0`, `0x004ed690`; doors, buildings, lifts
  `0x004ebc80`, `0x0057e710`, `0x0057e060`, `0x004ebbd0`, `0x0051b2a0`, `0x0051b110`, `0x0057d760`,
  `0x0051abe0`, `0x0057de30`, `0x0057e4d0`, `0x0051a690`, `0x0051b660`, `0x0057ebd0`, `0x0057ec80`,
  `0x0057e680`; materials `0x004eb830`; patches `0x0054eea0`, `0x004d28e0`; availability `0x005563f0`,
  `0x00556470`, `0x00556260`, `0x005559b0`, `0x00555a10`, `0x00556230`; grid `0x004e8bd0`, `0x004e99e0`,
  `0x0051fdc0`, `0x004ed070`, `0x004ecf60`, `0x004e89d0`, `0x004e8070`; resolution `0x004e8fc0`,
  `0x004e9760`, `0x004e9800`, `0x004e9920`, `0x004debf0`; clearance `0x004f5750`, `0x004f5890`, `0x004f6c20`
  (head), `0x00608f10`; clicks and sequences `0x004ccad0`, `0x004cd600`, `0x004cac00` (parts), `0x004d7880`,
  `0x00582640`, `0x005883a0`; routes `0x004f9a80`, `0x004fa080`, `0x004f9d60`, `0x004fa6f0`; orders
  `0x00467a50`, `0x00472070` (structure), `0x0046a900`, `0x00469770`, `0x0046a000`; path finder
  `0x00552290`, `0x00552e00`, `0x005532a0`, `0x00553100`, `0x005546e0`, `0x00554750`, `0x005545c0`,
  `0x005547c0`, `0x00553e60`, `0x00556490`, `0x00556990`, `0x00553a10`, `0x00553d20`, `0x00554360`,
  `0x00554b80`, `0x005553d0`, `0x005568f0`, `0x00556920`, `0x00557e70`, `0x00557f90`, `0x005581a0`,
  `0x00558210` (head), `0x005557c0` (head), `0x00554560`, `0x00559660`, `0x0055aa60`, `0x004d23d0`; position
  record `0x0055f290`, `0x0055fa70`, `0x0055fc20`; bonds at run time `0x00462aa0`, `0x004af540`; movement
  entry `0x005b86b0` (parts), `0x00561040` (head), `0x00563bb0`; natives `0x005716a0`, `0x00571760`,
  `0x00574860`, `0x00577cf0`, `0x00577150`, `0x005781f0`, `0x00579810`, `0x00579850`, `0x00579870`,
  `0x00579880`, `0x005798a0`, `0x005787f0` and their adapters; helpers `0x005fc6a0`, `0x005fc680`,
  `0x005fc660`, `0x005fc700`, `0x005fc110`, `0x006091b0`, `0x00608400`, `0x00609050`.
- Data: `C:\Users\przem\source\gamedata\robinhood\DATA\Levels\*.rhp`, all nine, read only; probes and overlays
  in `re/notes/nav/` (never committed).
- Documents compared: `docs/formats/rhp.md`, `docs/formats/layers-and-doors.md`, `docs/formats/rhm.md`,
  `docs/formats/scb.md`, `crates/opensherwood-core/src/{nav,geom}.rs`, the pinned sibling specifications,
  `docs/decisions/ADR-0010-logic-frame.md`.
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
4, 8, 98, 152, 156, 182, 186-189, 191 **and 64** (its earlier reading, "place / send an actor at a location", is
a historical hypothesis; the requirement is the section 6 row and VM section 6). What `layers-and-doors.md`
established and this specification keeps: the projection rule (1.1), the link pair as (sector number, layer)
(1.2), the sector numbering with gaps (1.3), the bond record framing, `FARM` kind 0 / kind 1 and the native-8
index space, door types 0/3/7 passage, 1/2 building, 4/5/6 lift ends with 5 the lower end. `rhm.md`'s placement
triple is unchanged.

**Differences from `nav.rs` / `geom.rs`** (each a required change):

1. Corner graph per sector with availability masks (2.3, 2.9) instead of an 8 px grid; the search of NAV-141
   with its tie rules, corner walks and smoothing.
2. Walkable ground = sector outlines minus enabled obstacles per layer (NAV-012), never `WOAW` areas.
3. Layer and sector change only through doors and lift ends; bonds change the area only (3.7).
4. A door-graph route (NAV-121) and the walk sequence (NAV-130) precede any path search.
5. Click resolution per NAV-100/101/102 (top layer first, one cell traversal in file order, blocked layers stop).
6. One search per tick with next-tick delivery (NAV-090, 8.1) instead of a synchronous unlimited search.
7. Door semantics of NAV-170/172 and section 6 (lock bytes, unlock-opens, 191 = click target).
8. Buildings, tenants and occupancy (3.9); lifts with admission kinds (NAV-192, 3.10).
9. Patches toggle availability (NAV-057); the sector state words are simulation state.
10. Movement is the sibling specification's; the engine's constant grid speed goes.
