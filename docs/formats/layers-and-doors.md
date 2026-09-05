# Layers, doors and the path graph (`WOAW`, `007 `, `FARM`, ` AZ `, `PPPP`, the `STAT` remainder)

Status: **data observation** (analyst session 2026-09-06, ADR-0003: the player's files, the compiled scripts
through the probes, the manual by eye, and earlier oracle notes; no executable, disassembly or debugger). Every
claim carries a status: `observed` (read from a file or a run), `inferred` (a reading that fits every case
found, with the count), `hypothesis`, `unknown`. Ids, counts and roles only: no game text, no designer names;
mission elements are named by index and role as in `docs/original/h01-win-path.md`.

Scope: what an implementer needs for work-list item 4 of `docs/original/h01-win-path.md` ("Layers (`WOAW`)
and doors (`FARM`, natives 4 / 191 / 186-189)"): (1) the layer model of the projection areas, (2) doors,
buildings and stairs, (3) the path graph, (4) the first mission's intended route through them, (5) an
implementation plan, (6) provenance. This document supersedes the `WOAW`, `007 `, `FARM` and ` AZ ` sections
of `rhp.md`, the "placement qualifier" hypothesis of `rhm.md` and the rows of natives 4, 8, 182, 186-189 and 191
of `scb.md` where they differ; those files should be updated to point here when the parser lands.

Coordinates: **screen** coordinates are pixels of the prerendered background (every `.rhm` placement, every
`STAT`, `FARM`, ` AZ `, `007 `, `PPPP` point). The `WOAW` vertices are the one exception: they are **world**
coordinates with a height, see 1.1.

## 1. Layers and projection areas (`WOAW`)

### 1.1 The projection rule (inferred, confidence high)

A `WOAW` vertex `(x, y, unknown_0x08, z)` is drawn at screen `(x, y - z)`: `y` is the ground-plane row of the
point, `z` its height, and the camera projects heights straight up the screen at 1 px per unit.

Evidence:

- Every placed record of every mission (`BORG`, `OILE`, `SCOT`, `TOTO`, `BOOM`, `SKRO`, `ZORG`: 4304 records in
  the 39 files) carries the placement triple `(P, Q, R)` of `rhm.md` ("Placement", `unknown_0x0c / 0x0e /
  0x10`). With `P >= 0` (1814 records) the record's screen position lies inside the polygon of `WOAW` area
  `P` **after** the shift `y - z` in **1813 of 1814** cases (the one miss is an object of `S02_Lei_MP`, a few
  px outside its area), and inside the unshifted polygon in far fewer (H01: 25 of 64). A sweep of the shift
  factor over `-2.0..2.0` in steps of 0.1 peaks sharply at `-1.0` (64 of 64 in H01; 57 at `-1.1`, 42 at
  `-0.9`).
- The `007 ` bond end points lie on an edge of the shifted polygon of their `area_a` in 105 of 109 Lincoln
  bonds (2 px tolerance) and on an edge of the unshifted polygon in 3.
- Overlay (`scratch keep_overlay.png`, Lincoln day background): the shifted polygons follow the wall walks,
  the stone floors, the hall floor and the stair flights exactly; the unshifted ones sit 220 px too low in
  the castle (the yard is at `z` 220).

Consequence for the engine: `opensherwood-app` (`engine.rs`, `geometry.areas`) currently rasterises the
**unshifted** polygons as walkable ground, so the whole castle is walkable 220 px south of where it is and the
keep's interior is not walkable at all (`h01-win-path.md` 3, objective 0). The win path of `test_win.py`
crosses the river bank and the curtain wall through that error.

### 1.2 Record fields

```
u16 layer_count; layer_count x u16 layer_id
u16 area_count
area_count x {
    u16 n; n x { f32 x, f32 y, f32 unknown_0x08, f32 z }
    f32 min[3]; f32 max[3]
    u8 link_count; link_count x { u16 sector, u16 layer }
    u8 unknown_flags[4]; u8 unknown_a
    u16 m; m x u16 unknown_list
}
```

| Field | Role | Status | Evidence |
|---|---|---|---|
| `layer_id` list | editor ids of the map's layers; **not** what the rest of the data indexes | observed | Lincoln lists 14 ids 27..=40 while its links, placements and bonds use layer numbers 0..=12; Derby lists 6 ids (0..=5) while its links use 1..=11. The count is echoed by `STAT unknown_0x00` (14 Lincoln, 2 forest maps) |
| vertex `x`, `y` | world ground-plane position (screen `x`, screen `y + z`) | inferred, high | 1.1 |
| vertex `z` | height above the ground plane; screen row = `y - z` | inferred, high | 1.1. Per vertex: 282 of Lincoln's 466 areas are flat (one `z`), 138 have two heights (ramps, stair flights: e.g. area 301 rises 220 -> 330, area 230 420 -> 550), the rest up to ten. Interpolate linearly across the polygon (planar for quads; a general polygon needs a triangulation - the retail ones are near-planar) |
| vertex `unknown_0x08` | a second height, 0 on 1911 of 2748 Lincoln vertices, otherwise 3..=52 below `z` (wall walks, floors on layers 2..=12) | unknown | not the projection (sweep, 1.1); hypotheses: the floor's underside for depth sorting, or a step height. Ignore for navigation |
| `min`, `max` | bounding box of `(x, y, unknown_0x08)` / `(x, y, z)` in world coordinates | observed | `rhp.md` |
| `links` | the **sector** and **layer** the area belongs to; 0 or 1 entry per area (no area of any map has two) | inferred, high | the `(Q, R)` of every placed record equals the single link of its area `P` in 1814 of 1814 cases; `GULP` points / polygons and `RAIL` points carry the same pairs in `q_b / q_c` |
| `unknown_flags[4]` | (1,1,1,1) 309, (0,1,1,1) 143, (1,0,0,1) 10, (1,1,1,0) 3, (0,0,0,1) 1 in Lincoln; flag 0 is 0 on 65 of the 131 areas with a nonzero `unknown_0x08` and on 79 without | unknown | |
| `unknown_a` | 0..=4; 1, 2 and 4 occur only on linked areas (14 / 52 / 3 of them), 15 linked areas have 0 | unknown | 2 = most floors, 1 = several halls and stair flights, 4 = the three climb areas of the ` AZ ` lifts with flag 3 (sectors 62/3, 63/3, 77/5) and area 222; a "surface kind" is the guess |
| `unknown_list` | 0..=26 in Lincoln, consecutive runs owned by the large layer-0 areas (area 62 owns 17..=24, 63 4..=8, 64 9..=13, 67 14..=16, 53 1..=3, 55 0, 189 25..=26); 26 for a few areas of Croisement02 | unknown | not the layer ids, not `FACE` indices; possibly indices of the per-sector motion polygons of the `STAT` remainder (section 3.1) |

### 1.3 Sectors, layers and the ground (inferred, high)

- A **sector** is a walkable floor: the set of areas that carry the same `(sector, layer)` link. Lincoln has
  51 sectors over 87 linked areas (of 466); York 184 linked areas, Nottingham 102, Derby 64, Leicester 60,
  Sherwood 18, the forest maps 12..=22. Sector ids are per map (Lincoln 0..=92 with gaps).
- A **layer** is a number 0..=12 (Lincoln; Derby to 11, Nottingham 8, Sherwood 8, York 6, forests 1). Layer
  0 is the ground. Even layers are floors; odd layers are the stairs / climbs between them in Lincoln (all
  ten ` AZ ` lifts sit on layers 1, 3, 5; the cross-layer bonds join 0-1, 1-2, 2-3, 3-4, 4-5, 5-6 through
  them, and 0-2 / 2-5 / 0-3 directly where a sloped area is the ramp or the climb itself), but York has two
  lifts on layer 0, so "odd = transition" is a Lincoln convention, not a rule.
- **Sector 0 / layer 0** is the outer ground of the town maps (Lincoln: eleven areas at `z` 0..=220, the road
  from the river bank up to the castle) and the whole ground of the forest maps.
- `P = -1, Q = 0, R = 0` (2490 of the 4304 placed records) means "on the `STAT` motion area": inside the
  `STAT` boundary and outside its obstacles, at `z` 0, sector 0, layer 0. The `STAT` boundary of Lincoln is
  the lower town (the son's scroll 114 at (253,380), the poor man's scroll 125); the castle yard is **outside**
  it (`rhp.md`, "Walkable ground") because it is a `WOAW` sector (24 / 0 at `z` 220).
- The 379 **unlinked** areas of Lincoln are not floors: no record of any mission is placed on one, no bond
  touches one, and by overlay they are the volumes of walls, roofs, rocks and towers (`rhp.md`: "the small
  ones sit exactly on rocks and bushes"). Their role is presentation (occlusion / depth: the `FACE` masks with
  reference lists point mostly at them, 1.6) and possibly sight; they must **not** be rasterised as walkable.

Lincoln, the areas the first mission uses (screen bbox = shifted polygon):

| Area | Sector / layer | `z` | Screen bbox | Role in H01 |
|---|---|---|---|---|
| 62 | 24 / 0 | 220 | (664,1180)-(2619,1948) | the castle yard: the start (1937,1384), the climbing / knock-out tutorial scrolls 115 / 117 |
| 67 | 27 / 0 | 220 | (1601,454)-(2941,1237) | the archery yard: archers 68..=74, targets 95..=98, scrolls 112 / 119, items 101 / 106 / 108 |
| 65, 73 | 36 / 0 | 220 | (366,1126)-(968,1588) | the west court: civilian 50, purse items 104 / 105, location 3 |
| 301, 288 | 55 / 2 | 220 -> 330, 330 -> 420 | (1081,1288)-(1490,1463) | the ramp out of the yard and the stair landing (zone 135 sits here) |
| 230 | 76 / 5 | 420 -> 550 | (1456,910)-(1644,1125) | the stair flight up to the keep (lift 3) |
| 198 | 78 / 6 | 550 | (1614,681)-(1945,945) | the keep's passage (zones 127 / 133 / 136) |
| 190 | 79 / 6 | 550 | (1470,498)-(1816,732) | the room between the passage and the hall (soldier 92, zone 6 of `PPPP`) |
| 229 | 81 / 6 | 550 | (1239,604)-(1517,735) | the great hall (zone 134: message 5) |
| 233, 234, 257 | 82 / 6 | 550 / 558 | (1020,650)-(1623,963) | the servant's hall: servant 53 at (1266,776), his scroll 113 at (1173,836), zone 137 (page 18), location 4 |
| 242, 243 | 86 / 6 | 530 -> 550 | (1174,941)-(1431,1075) | the lower central room: zone 131, the knight-tip scroll 121 at (1288,982), soldier 57 |
| 416 | 45 / 2 | 390 | (738,1019)-(1222,1285) | the terrace of the knight (78 at (861,1135), rail 22 entirely on it) |
| 281 | 71 / 4 | 435 | (803,1085)-(1198,1291) | the west tower's floor: zone 129 (message 3), the steward-tip scroll 120 at (941,1192) |
| 77 | 50 / 2 | 350 | (1966,1060)-(2326,1155) | the gatehouse walkway: the training-start scroll 111, the jump / beggar tutorial scrolls 116 / 118, arrow pile 100 |
| 123 | 62 / 3 | 220 -> 350 | (2238,1135)-(2308,1284) | the ivy climb to the walkway (lift 7, oracle-observed: `h01-measurements-2.md` 1.2 / 3) |
| 188 | 59 / 2 | 303 -> 358 | (1723,1114)-(1967,1252) | the wall walk over the yard gate: the hall-opening tutorial scroll 123 |
| 189, 228 | 69 / 4 | 472 | (1660,895)-(1926,1049) | the floor above: purse item 109, location 12, building door 23 |
| 209 | 90 / 8 | 800 | (1471,226)-(1817,472) | the tower floor with the arrows scroll 124 and arrow pile 102 |

### 1.4 Heights and depth (partly hypothesis)

- The height of a point on an area is the plane through the area's vertices (observed: at most two distinct
  `z` on 420 of 466 Lincoln areas; ramps are quads with two heights). A character standing at screen
  `(sx, sy)` on sector `S` has world row `sy + z` and is drawn at `sy`.
- Sprite depth on a layer: **hypothesis** - the draw order key is the world row `sy + z` (a character on the
  wall walk sorts behind the parapet in front of him and in front of the yard floor below), and the `FACE`
  masks that carry a reference list (31 of 428 in Lincoln, `kind & 0x10`) are drawn over sprites **only** when
  the sprite stands on one of the referenced areas (mask 172 references area 188, the wall walk it fronts;
  mask 271 references the three areas of sector 64 / 4 plus two unlinked ones). Needs the oracle: walk on the
  wall walk of Lincoln and record which masks cover Robin. The camera itself is not affected by layers
  (screen coordinates throughout).

### 1.5 Entry and exit edges: the bonds (`007 `) (inferred, high)

`u16 n; n x { i16 x1, y1, x2, y2; u16 area_a; u16 area_b; u16 layer }` (`rhp.md` framing unchanged).

- A bond is a **screen-space segment** on the shared edge of two linked areas (105 of 109 on `area_a`'s
  shifted edge) across which a character may pass. Bonds connect areas of the same sector (Lincoln: 41),
  areas of different sectors on the same layer (19: e.g. the yard 24 / 0 to the west court 36 / 0, bonds 66
  and 86) and areas of **different layers** (46: the ramp 61 / 62 joins 24 / 0 to 55 / 2; the stair 56 / 57
  joins 55 / 2 to 76 / 5, 35 / 36 join 76 / 5 to 78 / 6).
- `unknown_0x0c` is the **layer** the bond record belongs to. A cross-layer crossing is stored **twice**, one
  record per layer on the same segment (Lincoln: 23 such pairs = the 46 cross-layer bonds; every same-layer
  bond's value equals its areas' layer); the engine must merge the pair into one undirected portal.
- `area_b = 0xffff` (3 in Lincoln: 55, 97, 108) is an exit from a layer-0 area onto the `STAT` motion area
  (bond 108 is the low edge of the river-bank ramp 52 at `z` 0), not the map edge.
- Bonds do not exist between an area and its neighbour where the polygons merely touch or overlap on screen:
  the wall walk (50 / 2) overlaps the yard (24 / 0) on screen and is reachable only through the ivy climb
  (lift 7). **Walkability is per sector**; a move that leaves the union of a sector's shifted polygons is
  allowed only across a bond (or a door / lift leaf, section 2), never by polygon overlap.

### 1.6 Placement qualifier (`rhm.md` update)

`(unknown_0x0c, unknown_0x0e, unknown_0x10)` of every placement, `GULP` point / polygon and `RAIL` point is
`(area index into WOAW.areas or -1, sector, layer)` (inferred, high: 1.1). The loader's assertion
(`executable-notes.md`: "characters lie on a motion area, on the right layer and sector") is this triple. 71 of
73 H01 rails stay in one sector; the two mixed ones are the walkway guard's (`h01-measurements-2.md` 3) and the
girl's, so NPC patrols cross layers too and the AI's walks need the same navigation as the player's.

## 2. Doors, buildings (`FARM`), stairs and climbs (` AZ `)

### 2.1 `FARM` (version 4), consumed exactly on all 9 maps (observed)

```
u16 record_count
record_count x {
    u8 kind                 0 = passage door (a door between two sectors), 1 = building
    u8 leaf_count           1..=10
    u8 unknown_zero         0 on all 235 records
    leaf_count x Leaf
}
Leaf (both chunks) {
    u8  unknown_lead        FARM: 1 (256), 0 (126), 3 (13), 7 (1), 2 (1); AZ: 5 (94), 4 (67), 6 (6)
    u8  block[9]            010000000000000000 (328 leaves), 010100010100000000 (54), 010000010100000000 (11),
                            010101010100000000 (2), 010000000001000101 (2); all AZ leaves 0100.. (see 2.3)
    Polyline polygon        0 points (passages without a leaf) or 4..=9 points: the door leaf's outline on the
                            background (a tall thin quad on the wall, e.g. (1406..1425, 884..958) for door 0)
    Point door              the door's position on the screen (on the bond for kind 0, 2.2)
    u16 sector_a, layer_a   the sector the door opens from
    Point p1, p2            two points 8..=35 px from `door`, on one side of it (the approach / stand points)
    u16 sector_b, layer_b   the other side; (0, 0) for building doors ("inside", not sector 0)
}
```

Counts (records / leaves): Croisement01 3 / 3, Croisement02 1 / 1, Croisement03 4 / 5, Derby 20 / 42,
Leicester 27 / 59, Lincoln 33 / 59, Nottingham 58 / 100, Sherwood 5 / 5, York 84 / 123. Kinds: Lincoln 14 kind
0 (records 0..=12, 14) and 19 kind 1; York 10 / 74; Nottingham 13 / 45; Leicester 11 / 16; Derby 6 / 14;
Sherwood 5 kind 1; the forest maps kind 0 except Croisement02 (1). Every point lies inside its map.

The earlier `rhp.md` note ("37-byte records in Croisement01, 53 in Sherwood") is this layout with one leaf
and an empty / 4-point polygon; the "`u16 a, u8 0, u8 id`" it lists is the empty polyline.

### 2.2 What a door is (inferred)

- **Kind 0, a passage door**: joins `sector_a` to `sector_b`. Its `door` point lies on the bond between the two
  sectors: for all 20 kind-0 leaves of Lincoln the nearest bond is 9..=37 px away and joins exactly the two
  sectors of the leaf (confidence high). A door with a polygon has a visible leaf (`unknown_lead` 3: doors 0,
  4, 6, 8, 22 of Lincoln); one without (`unknown_lead` 0) is an open passage or a gate the script only
  closes. Closed, it blocks the bond (hypothesis: the polygon is the blocking mask and the visual patch).
- **Kind 1, a building**: one record = one building, its leaves = its doors, each leaf on one sector and layer
  and `sector_b = (0, 0)` = "inside". A building with several leaves on different layers is a **tower with an
  internal staircase**: record 22 of Lincoln has doors 36 (86 / 6), 37 (70 / 4) and 38 (45 / 2) - the only way
  to the knight's terrace 45 / 2 (section 4); record 16 has doors 25 (79 / 6), 26 (90 / 8), 27 (54 / 2) - the
  way to the arrows scroll on 90 / 8; record 13 is the house of the west court with doors 20 (36 / 0) and 21
  (35 / 0), the girl (civilian 50) comes out of it when message 3 opens door 20 (2.5). The manual (printed p.
  16, paraphrased): the cursor becomes a door over doors; a click on the door makes the selected characters
  **enter the building**, which may be full of enemies. Natives 8 / 98 / 156 / 152 (`scb.md`) are this: an
  actor inside a building is off the map until he comes out.
- `p1` / `p2`: two points just outside the door on the `sector_a` side (kind 0 and 1 alike); hypothesis: the
  stand point of the actor opening it and the point he walks to on the far side. Not needed for a first
  implementation (walk to `door`).
- The 9-byte `block`: byte 0 is 1 on all 397 `FARM` and 171 ` AZ ` leaves; bytes 1..=4 are 0 / 1 and come in the patterns above (bytes
  3 and 4 always equal; bytes 1 and 2 set only with 3 and 4); bytes 5..=8 are 0 except on two Croisement03
  leaves. **Hypothesis**: byte 0 = initial state open, bytes 1..=4 = the four lock kinds of natives 186..=189
  (the door-locking helper of the scripts calls the four in a row; door 8 of Lincoln, the west gate with all
  four set, is the one the level closes at load and never opens; the 54 leaves with bytes 1, 3, 4 set are gates
  and the six-leaf record 9 of Lincoln, the drawbridge tower's three passages to the yard and three to the
  ground). Needs the oracle or a script that reads 182 on a known door.
- `unknown_lead`: 0 / 3 on kind 0 (3 = with a polygon), 1 on kind 1, 7 and 2 once (Leicester, York); on the
  lifts 5 = the lower end (9 of 10 Lincoln lifts, the tenth has 5 / 5), 4 = the upper end of a stair, 6 = the
  upper end of a climb (flag 3 lifts). A "door type" byte; `unknown` beyond that.

### 2.3 ` AZ ` (version 2), consumed exactly on all 9 maps (observed): the lifts

```
u16 lift_count
lift_count x {
    u16 sector, u16 layer   the lift's own sector (the stair flight or the ivy): an odd layer in Lincoln
    u8  flag                1 = stairs (Lincoln 6, Nottingham 12), 2 (Lincoln 1, Derby 2, Sherwood 3), 3 =
                            climb (Lincoln 3: the ivy east of the gate, the wall below the archery yard, the
                            flight to 75 / 4; York 5, Leicester 1)
    Polyline polygon        0 points on every lift of every map
    u16 leaf_count          2 (Lincoln all ten; up to 8 in York)
    leaf_count x Leaf       as in FARM: `door` = the foot of the flight on `sector_a`, `sector_b` = the lift's
                            own sector; the two leaves are the two ends
    u16 unknown_trailer     0..=4 or 11..=15
}
```

Counts (lifts / leaves): Derby 12 / 26, Leicester 10 / 23, Lincoln 10 / 20, Nottingham 12 / 34, Sherwood 4 /
9, York 21 / 59, forests 0. Each leaf's `door` lies on the bond between `sector_a` and the lift's sector
(Lincoln: lift 0's leaves at (1756,1751) and (1858,1580) sit on bonds 87 / 88 and 92 / 93), so lifts are bonds
with an animation: the flight is itself a walkable sector (area 230 for lift 3, area 123 for the ivy) and the
crossing is ordinary navigation plus the stair / climb gait. Flag 3 needs the **climb** ability: the manual
(p. 16) says only some characters climb ivy-covered walls (double-arrow cursor), and the first mission's
`Initialize` disables player action 3 with `n196(3, 0)` next to actions 6..=9 (`scb.md`), which the climbing
tutorial scroll 115 - lying at the foot of lift 7 at (2235,1297) - presumably re-enables (hypothesis; the
oracle saw Robin climb that ivy on a plain walk order, `h01-measurements-2.md` 1.2 / 3).

Lincoln's lifts: 0: 24/0 <-> 48/2 by 41/1 (drawbridge tower); 1: 27/0 <-> 53/2 by 40/1; 2: 27/0 <-> 54/2 by
39/1; 3: 55/2 <-> 78/6 by 76/5 (**the keep's stair**); 4: 24/0 <-> 59/2 by 43/1 (flag 2; the wall walk over the
gate); 5: 36/0 <-> 56/2 by 42/1; 6: 56/2 <-> 73/4 by 61/3 (to the west tower); 7: 24/0 <-> 50/2 by 62/3 (flag
3, the ivy); 8: 0/0 <-> 53/2 by 63/3 (flag 3); 9: 59/2 <-> 75/4 by 77/5 (flag 3).

### 2.4 The index spaces of natives 4 and 8 (inferred)

- **Native 4 (door handle)** indexes the **flat list of `FARM` leaves** in file order (record by record, leaf
  by leaf): York's 123 leaves are exactly native 4's largest immediate + 1 (122, `scb.md`), Nottingham 100
  leaves >= 95, Lincoln 59 >= 53, Leicester 59 >= 56, Derby 42 >= 30; the alternative "records, then lift
  leaves" gives 92 for Nottingham and 50 for Leicester, below the immediates used. The first mission fixes the
  choice: door 20 is the west house's door at (525,1367), 38 px from civilian 50 whom the same message 3
  activates, and door 23 is the building door at (1912,966) beside item 109 and location 12 that message 7
  activates (2.5). Whether the ` AZ ` leaves follow at index 59.. is `unknown` (no script addresses them;
  York's immediates stop at 122 = the last `FARM` leaf). Confidence high.
- **Native 8 (building handle)** indexes the **kind-1 records** in file order, skipping kind-0 records: York has
  74 kind-1 records and native 8's largest immediate there is 73, Leicester 16 and 15 (`scb.md`); Nottingham's
  immediates 4..=41 fit both numberings. Confidence medium. `-1` = outdoors.

### 2.5 The door natives as the scripts use them (observed over the 39 scripts)

| Native | Arity | Calls | Reading | Confidence |
|---|---|---|---|---|
| 4 `(i) -> door` | 1 | 460, always an immediate, always fed to 191 / 186..=189 / 182 / a helper | door leaf `i` | high |
| 191 `(state, door)` | 2 | 221: `state` 0 (156) or 1 (65); 72 in `Initialize` (all with 0), 107 in `ProcessMessage`, 19 in `EnterZone`, 10 in `Hourglass` | 0 = close, 1 = open. Doors start **open** (block byte 0 = 1 on every leaf) and the levels close the ones they want shut; messages open them area by area | high for the direction (a level never "closes" a door it did not close first... it opens 20, 23, 25, 37 that `Initialize` closed) |
| 186..=189 `(door, 0/1)` | 2 | 105 / 35 / 36 / 50; second argument 1 in 79 / 22 / 30 / 43 calls, 0 otherwise; mostly `Initialize`; a "lock door" helper calls all four | four lock flags per door (block bytes 1..=4 by hypothesis); 1 = lock. Which of the four the player's lock-pick (manual p. 28, a context action) or an enemy respects is `unknown`; the first implementation treats **any** set flag as "locked: cannot be opened by walking into it" | medium for "lock", low for the four kinds |
| 182 `(door) -> int` | 1 | 3, all in `Hourglass`, compared with 0 once per door, then a patch, an actor move and `n191(0, door)` | door is open (1) / closed (0), or "has been opened by the player" (a run-once trigger) | low; `scb.md` stub note stands (return 1 avoids a spurious close at tick 1) |

The first mission (`H01_Lin_VL`, level class; indices = `FARM` leaves; sectors from 2.1):

| When | Call | Leaf | Kind | Screen | Sides | Meaning |
|---|---|---|---|---|---|---|
| `Initialize` | `n191(0, 8)` | 8 (record 6) | 0, polygon, all four lock bytes set | (435,1578) | 33/0 - 35/0 | the west gate of the lower town shut for the mission |
| `Initialize` | `n186(20, 1)`, `n191(0, 20)`; `n191(0, 21)` | 20, 21 (record 13) | 1 | (525,1367), (443,1447) | 36/0, 35/0 -> inside | the west house locked and shut; the girl (50) waits inside, hidden |
| `Initialize` | `n186(28, 1)` | 28 (record 17) | 1 | (1931,786) | 27/0 -> inside | a building in the archery yard locked |
| `Initialize` | `n191(0, 37)`, `n191(0, 25)`, `n191(0, 23)` | 37 (record 22), 25 (record 16), 23 (record 15) | 1 | (1304,1066), (1755,633), (1912,966) | 70/4, 79/6, 69/4 -> inside | three tower doors shut until their area is entered |
| message 3 (zone 129, the west tower 71/4) | `n191(1, 20)` | 20 | 1 | | | the house opens, civilian 50 and soldier 79 appear, purse items 104 / 105 activate |
| message 5 (zone 134, the great hall 81/6) | `n191(0, 0)` | 0 (record 0) | 0, polygon | (1399,961) | 86/6 - 82/6 | the door between the lower central room and the servant's hall shuts behind the player |
| message 6 (zones 127 / 128) | `n191(1, 25)`, `n191(0, 4)`, `n191(0, 5)` | 25; 4, 5 (record 3) | 1; 0 | (1755,633); (1747,680), (1476,627) | 79/6 -> inside; 79/6 - 78/6, 81/6 - 79/6 | the tower door in room 79 opens and soldier 92 comes out along path 53; both passage doors of room 79 shut |
| message 7 (zones 132 / 133) | `n191(1, 23)`, `n191(0, 22)` | 23; 22 (record 14) | 1; 0, polygon | (1912,966); (1727,1013) | 69/4 -> inside; 75/4 - 69/4 | the hall-opening event: item 109 activates, scroll 123's state changes |
| zone 131 (the lower central room 86/6) | `n191(1, 37)`, `n191(0, 6)` | 37; 6 (record 4) | 1; 0, polygon | (1304,1066); (1167,1100) | 70/4 -> inside; 71/4 - 70/4 | the tower's middle door opens (the way down to the knight's terrace through building 22), map element 23 (the room's torch, `FLIM` 23 at (1383,982); `scb.md`'s "soldier 23" is this light) activates |

So the keep is **not** gated by closed doors at the start: `Initialize` shuts a gate and building doors only;
every passage door on the route of section 4 (19, 2, 3, 4, 5, 1, 0) starts open. What the engine lacks for the
route is the layer model, not the doors; the doors matter for the side plots (the girl's house, the tower to
the knight) and for the rooms the script shuts behind the player.

### 2.6 The manual (printed pages 16 and 28, paraphrased)

Some places are inaccessible to some characters (the cursor becomes a cross); some characters climb
ivy-covered walls (double-arrow cursor; a click on a roof makes a climber find his own way); some jump at
marked places (curved-arrow cursor with a blue trajectory: the `PPPP` jump lines); the cursor becomes a door
over doors and a click makes the selected characters enter the building; "pick locks" and "activate
mechanism" are context actions (cursor change over a valid target). Nothing in the manual says a passage door
opens on approach; the oracle question stays open (section 5, step 7).

## 3. The path graph (the `STAT` remainder, console EULER)

Framing established on all 9 maps up to the edge table; the edge target encoding and the trailing table are
not. The engine does **not** need this graph for item 4 (the plan builds its own per-sector grid from section
1); it is what an "original routes" milestone would decode.

### 3.1 Per-sector motion polygons (partial)

After the layer-0 obstacles the chunk holds further polygon records framed like the obstacles with their own
obstacle lists (`rhp.md`, "Remainder of `STAT`": 26 records in Lincoln by the sequential rule, which fails on
the forest maps, Derby and Sherwood). The first Lincoln record (107 points) outlines the castle yard in screen
coordinates and the others the courts, the hall floors and the wall walks, so these are the **2D motion areas
per sector** (the screen-space equivalent of the shifted `WOAW` floors, with the per-sector obstacles the
`STAT` prefix only gives for the ground). Framing of the inter-record fields: `unknown`; consumption not
exact. The `WOAW` `unknown_list` values (1.2) probably index these records.

### 3.2 Graph header (observed, 9 / 9)

Located by the byte pattern `u16 1, f32 6.0, f32 4.0` (offsets 2124 Croisement01 .. 10595 York), then `u16 a`
(= `STAT unknown_0x00`: 2 forests, 14 Lincoln), `u16 b` (= `STAT unknown_0x02`), `u16 group_count` (47, 43, 22,
6, 33, 24, 12, 26, 22 in the map order of `rhp.md`).

### 3.3 Node groups (observed, consumed exactly on 9 / 9 up to the mid-section)

```
group_count x { u16 n; n x Node }
Node { u16 1; u8 unknown_k (0..=14); Point p; i16 d[4]; u32 unknown_flags; u16 e; e x u16 edge_id }
```

The first group is the largest (Lincoln 173 of 288 nodes, Croisement01 43 of 285). `p` is a screen point inside
the map (0 out of bounds over 9 maps); `d` are two offset pairs the size of polygon edges (e.g. (54,-16),
(-17,-17)): the node is a **corner of a motion polygon** and `d` its two edges (the visibility-graph reading of
`rhp.md`, now framed); `unknown_k` is a 4-bit value (a corner / quadrant type); `unknown_flags` is 0 on all
nodes of the town maps and a single bit (1..=8192, in pairs of nodes) on the forest maps and Nottingham / York
(jump-line ends is the guess). The edge ids of all nodes are consecutive across the whole node list (0..=1713 in
Lincoln): each node lists its outgoing edges.

### 3.4 Mid-section (partial)

Between the last group and the edge table: 144 bytes in Croisement01, 7148 Derby, 10025 Lincoln, 17095 York:
`u16 count` then lists of `{ u16 1, u16 n, n x Node }` whose edge ids continue the numbering (Croisement01: 7
lists, Sherwood 2) - the framing fits the forest maps and Sherwood but not the town maps' first words
(Lincoln `3, 53`, Leicester `3, 49`); `unknown`.

### 3.5 Edge table (observed, count and position on 9 / 9; fields partial)

28-byte records `u16 1, u16 index (0..), u32 unknown_a, u16 group, u16 unknown_b, u32 unknown_a again, u16
unknown_c, u16 unknown_d, f32 length, u32 unknown_e`: 2638 Croisement01, 2468, 1264, 1372 Derby, 2994
Leicester, 2958 Lincoln, 4296 Nottingham, 1624 Sherwood, 4240 York (a few more than the ids the nodes list).
`group` is a group index (max = `group_count - 1` on every map); `length` is 0..=1672 px. `unknown_d` is
**not** the target node's index within `group` (the distance test fails), so the target encoding stays
`unknown`.

### 3.6 Trailing table (unknown)

After the edges: 21418 Croisement01 .. 34514 Nottingham bytes of small values (0..=13, dominated by 1, 0, 4, 8,
2) with a strong period-8 texture; not a whole-map cell grid at 4 / 8 / 16 / 32 px (no size matches with a
0..=15-byte header). The "fast-find grid" of `executable-notes.md` is the guess.

## 4. The first mission's intended route (inferred from sections 1-2 and the script)

The sector graph of Lincoln (nodes = sectors, edges = bonds, passage doors and lift leaves; scratch
`lay5.py`) gives one route from the start to the servant's hall and none to the knight's terrace or the arrows
scroll except through buildings, which matches the script's door choreography:

| Step | From -> to | Crossing | Where (screen) | What happens |
|---|---|---|---|---|
| 1 | 24 / 0 (yard, area 62) -> 55 / 2 (ramp 301) | bonds 61 / 62 = passage door 19 (open, no leaf) | (1081,1438)-(1108,1461) | Robin walks up the ramp; `z` rises 220 -> 330 |
| 2 | 55 / 2 (landing, area 288) | inside the sector | zone 135 at (1384..1482, 1103..1184) is the staircase zone: patch 10, map elements 24 / 25 (two torches) activate | |
| 3 | 55 / 2 -> 76 / 5 (stair flight, area 230) | bonds 56 / 57 = door 2 (open) = lift 3's lower leaf (1455,1118) | (1455,1099)-(1483,1123) | the stair gait; `z` 420 -> 550 |
| 4 | 76 / 5 -> 78 / 6 (the keep's passage, area 198) | bonds 35 / 36 = door 3 (open) = lift 3's upper leaf (1642,916) | (1614,909)-(1643,936) | zone 136 (the other staircase zone) and zone 127 / 133 (message 6 / 7) sit here |
| 5 | 78 / 6 -> 79 / 6 (the room, area 190) | bond 33 = door 4 (open, leaf polygon) | (1741,699)-(1773,686) | message 6 will shut doors 4 and 5 behind the player and open the tower door 25 |
| 6 | 79 / 6 -> 81 / 6 (the great hall, area 229) | bond 34 = door 5 (open, leaf polygon) | (1482,604)-(1516,638) | zone 134: message 5 (the hall's lights, the servant 53 appears at (1266,776) and walks to his scroll 113, door 0 shuts) |
| 7 | 81 / 6 -> 82 / 6 (the servant's hall, areas 233 / 257 / 234) | bond 98 = door 1 (open, no leaf) | (1238,703)-(1277,734) | zone 137 (page 18); **the servant's scroll 113 at (1173,836) on area 233** - objective 0 |
| 8 | 82 / 6 -> 86 / 6 (the lower central room, areas 242 / 243) | bond 40 = door 0 (**closed by message 5**) | (1399,941)-(1424,963) | the knight-tip scroll 121 at (1288,982) on area 243 (objective 4 added); zone 131 opens door 37 and shuts door 6 |
| 9 | 86 / 6 -> building 22 -> 45 / 2 (the terrace, area 416) | building door 36 at (1302,968) in, door 38 at (1136,1048) out (door 37 at (1304,1066) is the middle floor 70 / 4) | | the knight 78 at (861,1135) on rail 22 (all five points on 45 / 2): knock out and search (objective 4) |
| 10 | back to 24 / 0, then 24 / 0 -> 0 / 0 (areas 64 / 63) -> the river road (areas 53, 54..=57, 52) -> the `STAT` ground | bonds 65 / 84, 59, 77, 80, 89, 60, then bond 108 (`area_b` none) at `z` 0 | | the son's scroll 114 at (253,380) is on the `STAT` motion area (`P` = -1): objective 1, the win |

Side plots: the west court 36 / 0 (bonds 66 / 86 from the yard) holds the purse items 104 / 105 and the girl's
house (doors 20 / 21); the west tower 71 / 4 with the steward-tip scroll 120 is reached by lift 5 (36 / 0 ->
56 / 2), lift 6 (56 / 2 -> 73 / 4) and door 7 (73 / 4 - 71 / 4); the archery walkway 50 / 2 with scroll 111 by
the ivy lift 7 (observed in the original on a plain walk order); the arrows scroll 124 on 90 / 8 only through
building 16 (doors 27 on 54 / 2, 25 on 79 / 6, 26 on 90 / 8), i.e. the tower door message 6 opens. Step 8 means
the room with the knight-tip scroll is entered **before** the servant's hall in a natural walk (the door shuts
once the hall is entered), or from the hall while door 0 is still open; the script's order in
`h01-win-path.md` 4.2 (steps 6 -> 7) is therefore reachable either way.

## 5. Implementation plan (`opensherwood-formats`, `opensherwood-core` `geom.rs` / `nav.rs` / `natives.rs`, app)

Effort is for one implementer, tests included; the total is 9..=12 days, inside the 1-2 weeks of the work list.

### 5.1 Formats (1.5 days)

- `rhp.rs`: rename `AreaPoint::unknown_0x08` stays; document `y` as world row and add `Area::screen_points()`
  (`(x, y - z)` rounded, deterministic: `f32` -> `i32` through a fixed rounding of the file's values at load,
  never at run time). `AreaLink { sector, layer }`. `Bond::layer` (was `unknown_0x0c`).
- New `Farm { records: Vec<Building> }` / `Building { kind, unknown_zero, leaves }`, `Leaf { unknown_lead,
  block: [u8; 9], polygon, door, side_a: (u16, u16), p1, p2, side_b }`, `Az { lifts }` / `Lift { sector, layer,
  flag, polygon, leaves, unknown_trailer }` with exact consumption; `Rhp::door_leaves()` = the flat leaf list
  (native 4's index space), `Rhp::buildings()` = kind-1 records in order (native 8's).
- `opensherwood-tools rhp`: print sectors per layer, doors (leaf index, kind, sides), lifts; `rhp-overlay`
  draws shifted polygons, bonds, door points (the scratch overlay of this session is the model).
- Tests: `gamedata.rs` pins per map the leaf counts of 2.1 and the lift counts of 2.3, `max link layer`,
  linked-area counts; a synthetic round trip for both chunks; the projection rule re-checked as a data test:
  every H01 placement inside its area `P` after the shift (64 / 64) and `(Q, R)` equal to the link.

### 5.2 Core geometry: sectors and layers (3 days)

Data model (`geom.rs`), all `i32` screen pixels, serialised in the snapshot, hashed under a new `geometry`
domain (bump `HASH_SCHEMA_VERSION` 20 -> 21, `SNAPSHOT_VERSION` 21 -> 22, `RULESET_VERSION` 18 -> 19):

```
Geometry {
    boundary, obstacles            // the STAT ground = sector 0, layer 0, z 0 (as today)
    sectors: Vec<Sector>           // one per distinct (sector, layer) link, in ascending (layer, sector) order
    portals: Vec<Portal>           // merged bonds: segment, sector_a, sector_b (or Ground), kind
}
Sector { id: u16, layer: u16, polygons: Vec<Vec<(i32,i32)>> /* shifted areas */, heights: Vec<Plane> }
Portal { a: SectorRef, b: SectorRef, p1, p2, kind: Bond | Door(leaf) | Lift(lift, leaf) }
```

- `is_walkable(x, y, sector)`: inside one of the sector's polygons and outside the layer-0 obstacles when the
  sector is on layer 0 (per-sector obstacles come with 3.1 later); the ground sector = the boundary rule of
  today. **Drop** the current "any area = walkable" rule.
- `nav.rs`: one `NavGrid` per sector (the ground grid as today; sector grids rasterise only the sector's
  polygons, so they are small), plus portal cells: the cells a portal segment crosses, marked in both
  sectors. A* runs over `(sector, cell)` with portal crossings as unit-cost moves; the budgets and the
  deterministic tie-breaks stay. Erosion by one cell must not close a portal (skip erosion on portal cells).
- Entities get `sector: SectorRef` (hashed). Placement: from the record's `P` (area -> sector) or the ground
  when `P = -1`; a placement whose point is not walkable in its sector is a load error (the original's
  assertion). Movement stays on the sector's grid; crossing a portal updates `sector`. `z` for rendering =
  the sector plane at the feet (`Plane` from the area's vertices; a flat area is the common case).
- Click resolution (`World::left_click`): candidates = every sector whose polygon contains the point, plus the
  ground; order the selected character to the candidate with a path (shortest by cost), else to the nearest
  reachable cell of his own sector (`find_path_near` as today). Tainted `Assumption::ClickLayer` until the
  oracle shows the cross-cursor rule.
- `debug.nav` reports `sector`, `layer`, the sectors containing the point and the path length per candidate.

### 5.3 Doors as world elements (2 days)

- `World::doors: Vec<Door { leaf: u16, kind, sides, point, polygon, open: bool, locks: [bool; 4] }]`, built from
  `Rhp::door_leaves()`, initial `open = block[0] == 1`, `locks = block[1..=4] == 1` (hypothesis, tainted
  `Assumption::DoorBlock`); hashed under `doors`; snapshotted; `debug.vm.doors` lists them.
- Natives: 4 returns the leaf index (already `Observed`); 191 sets `open`; 186..=189 set `locks[k]`; 182 returns
  `open` (policy taint stays until measured). Remove them from `STUB_NATIVES`; the load-time stub counts of
  `test_script.py` (`186: 2, 191: 6` in H01) become door states: re-pin `EXPECTED_AT_LOAD`.
- Navigation: a **closed** kind-0 door blocks its portal (the portal cells are unwalkable in both sectors while
  `open` is false); an **open** one is a plain portal. A walk order that needs a closed, **unlocked** door opens
  it on arrival at the door point (hypothesis; the alternative is a click on the door: step 7 decides) and
  records `Assumption::DoorOpenOnApproach`; a locked door is a wall. AI walks (rails crossing a closed door)
  use the same rule.
- Rendering: while closed, draw the leaf polygon filled from the background of the closed state is not
  available (the closed door is the background as painted; the open door needs a patch: `TUPO` / `FLIM`,
  `unknown`). First version: no visual change, the door's state only in the HUD tooltip and `debug.vm`.

### 5.4 Buildings (1.5 days)

- `World::buildings` from `Rhp::buildings()`: for each kind-1 record the leaf indices. Native 8 returns the
  building index (-1 outdoors), 98 = the actor's `inside: Option<building>`, 156 puts him inside (hidden, no
  sector), 152 takes him out at his current door. Entering: a click on a building door leaf (the manual's
  door cursor) orders a walk to the door point; on arrival with the door open and not locked the actor goes
  inside; the path finder treats a building as a node joining all its open doors (a walk through the tower:
  door 36 in, door 38 out, the actor invisible for the crossing time; hypothesis, `Assumption::BuildingWalk`).
  The girl's exit (message 3: activate 50, open door 20, give path 24) works with 5.3 alone since she is
  placed on 36 / 0 already.

### 5.5 Lifts and climbs (1 day)

- Portals of kind `Lift` from ` AZ ` leaves (they coincide with bonds; keep the bond portal and tag it): the
  crossing uses the stair gait / animation (`sprite-animations.md`: the climb sequences) instead of the walk;
  flag 3 requires the character's climb ability (player action 3 by hypothesis; when disabled the portal is
  closed for that character; `Assumption::ClimbAbility`). Jump lines (`PPPP`) are a later item (one-way
  portals down, the jump animation).

### 5.6 Rendering on layers (1 day + oracle)

Sprite screen row = feet `y`; depth key = `y + z(sector plane)` (5.2); `FACE` masks with reference lists cover
only sprites on the referenced sectors (1.4 hypothesis, `Assumption::MaskRefs`). The mini-map and the camera
are untouched.

### 5.7 Tests (in the estimates above)

- Synthetic: two stacked sectors joined by a portal; a walk crosses it; without the portal the target is
  unreachable; a closed door blocks, `n191(1, d)` unblocks, a lock keeps it blocked; a building joins two
  doors; snapshot / restore / hash round trips; the budget tests of `nav.rs` per sector.
- Data (H01): `debug.nav` at (1173,836) reports sector 82 / 6 walkable; the route of section 4 exists from
  the start (`path_cells` > 0 to (1173,836) with the door states of tick 2); `test_first_mission_objective_0`
  walks it through play (select, click the ramp, the stair, the hall, the servant's scroll: objective 0 done,
  1 added, message 14's pages), tainted by the assumptions above; the existing win test is re-recorded (the
  son's scroll is now reached by the river road, not across the curtain wall); the knock-out and purse tests
  keep passing (the yard and the west court are layer 0 and unchanged except for the 220 px shift, which the
  tests' coordinates already use since they are screen coordinates).

### 5.8 Step 7: the oracle session this plan wants (0.5 day)

From `robinhood_oracle` with `harness/tools/original`: (a) order Robin to the servant's hall by a click on the
hall floor at (1180,830) and record the path (ramp, stair, passage) and whether door 4 / 5 open on approach or
need a click; (b) on the wall walk (50 / 2) record which masks cover him and his draw order against the
parapet; (c) click a building door (36 at (1302,968)) and record the cursor, the entry, where he comes out;
(d) read 182 through a scratch harness on a door the script closes. These fix the three hypotheses of 5.3 /
5.4 / 5.6.

## 6. Provenance

- Data: `C:\Users\przem\source\gamedata\robinhood` (GOG English build, executable SHA-256
  `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`): all nine `DATA/Levels/*.rhp`, all 39
  `.rhm` and `.scb`, `Levels/Day/Lincoln.map` for the overlays, `Manual.pdf` printed pages 16-17 and 26-28
  rendered with PyMuPDF (no text layer) and read by eye.
- Tools (repository): `harness/tools/probe/rhp_chunks.py`, `probe_woaw.py` (the `WOAW` framing),
  `probe_stat_layers.py` (the `STAT` remainder), `chunkdump.py`, `probe_dump.py`, `rhm_full.py --json` (the
  placement triples), `scb_semantics.py --pseudo / --natives` (the H01 script and the corpus statistics of
  natives 4, 182, 186..=189, 191), `map_png.py`; `opensherwood-tools rhp / chunks` (release build of
  `931f496`).
- Scratch scripts in the session scratchpad (not committed, generic, no game bytes): `lay1.py` (`WOAW` fields
  and points of interest), `lay2.py` / `lay4.py` (`FARM` / ` AZ ` parsers, leaf layout, all maps), `lay3.py`
  (the projection sweep and the `(Q, R)` test on H01), `lay5.py` (bonds, doors on bonds, the sector graph and
  its routes), `lay6.py` (the corpus-wide placement test over 39 missions, `PPPP` of Lincoln, native 8's
  index), `lay7.py` .. `lay9.py` (the path graph); the overlay pictures `keep_overlay.png`, `west_overlay.png`,
  `yard_overlay.png`.
- Engine: `crates/opensherwood-core/src/{geom,nav,natives,vm,world}.rs`, `crates/opensherwood-formats/src/rhp.rs`,
  `crates/opensherwood-app/src/engine.rs` (the geometry build and `debug.nav`) read for the plan; nothing
  written or run in the engine for this document beyond what `h01-win-path.md` already records.
- Original: not run in this session. The layer observations cited from the oracle are those of
  `docs/original/h01-measurements-2.md` (the ivy climb on a plain walk order, the walkway, the walkway guard),
  same build hash.
- Who: analyst session (a Claude agent), data observation only; no decompiler or disassembler output was
  consulted.
- Tests that will depend on this document: the format pins of 5.1 (`crates/opensherwood-formats` data tests),
  the synthetic sector / door tests of 5.7, `harness/tests/data/test_mission.py` (the intended-route test, the
  re-pinned `EXPECTED_AT_LOAD` of `test_script.py`), `test_win.py` (re-recorded replay), and the golden digests
  of the app once the geometry hash domain changes.
