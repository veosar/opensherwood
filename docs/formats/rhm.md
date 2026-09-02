# RHM mission file (`.rhm`, magic `DUTY`)

Status: **decoded** (container, every chunk consumed exactly by `crates/opensherwood-formats/src/rhm.rs` on all
39 retail files). Field *semantics* are established where stated; everything else is `unknown_*` with its observed
value set. Open questions are listed at the end.

39 files, one per mission (`H01_Lin_VL.rhm` = Hood mission 1 in Lincoln; `Emb01_FoA_EC` = ambush in forest area A;
`S0x` = Sherwood/story; `Str0x` = street; `Tac0x` = tactical; `EmbTut` = tutorial; `Sherwood.rhm` and
`SherwoodOutro.rhm` = camp). Each mission also has a `.scb` script with the same base name (except `Sherwood.rhm`
which uses `sherwood.scb`). The two-letter designer initials at the end of the name (EC, MK, MP, VL, JMS) match the
source path `C:\DOCUME~1\ECoste\...\script.scs` embedded in the compiled scripts.

The executable calls this file the "RHD" (level description); its version-check strings name the chunks
`Header`, `Tenant`, `Actor`, `Bonus`, `Tactic`, `Path`, `Scroll`, `Mobile`, `Script`, `Jump` (and, for the `.rhp`,
`Building`, `Lift`, `Material`, `Sound`, `Patch`, `Bond`, `Miscellaneous`, `Sight`, `Animation`, `Motion`, `Mask`,
`Light`). The mapping of these names to tags below is inferred from content and marked as such.

## Container

Same as [RHP](rhp.md): `DUTY size version=2` then child chunks `tag size version body`. All 39 files have exactly the
ten children below, in this order, with these versions. The loader is tag driven and skips unknown chunks with a
warning (executable strings), so the reader keeps unknown tags in `unknown_chunks`.

| Tag | Version | Content | Executable name (inferred) |
|---|---|---|---|
| `FOOT` | 4 | header: map id, variant, map name, mission id | Header |
| `POUF` | 3 | animated map elements used by the mission (traps, hiding places) | Tenant? / Animation? |
| `BOYZ` | 3 | six actor class groups `MEOW SCOT OILE TOTO BORG BOOM` | Actor |
| `ZORG` | 2 | small placed items, 5 kinds | Bonus (hypothesis) |
| `HIRN` | 2 | AI data: `HOLE` waypoints, `BUSH` points, `POW ` beam-me points, `NLIP` zones | Tactic |
| `RAIL` | 3 | patrol paths with per-waypoint command programs | Path |
| `SKRO` | 4 | scrolls (parchments) | Scroll |
| `TING` | 3 | mobile elements (carts) | Mobile |
| `GULP` | 2 | script sectors (polygons named in scripts) plus a point list | Script |
| `CAVE` | 3 | per-map table of id lists | Jump? / Building? |

Common conventions: all integers little-endian; `pstring16` = `u16 length` + Latin-1 bytes; an *optional name* is
`u8 has_name` (0/1) followed by a `pstring16` when 1. Names of scriptable elements end in `_8XXXXXXX` (a hex id,
unique in the file); **every** such name has a script class of the same name in the mission's `.scb`
(1409 of 1409 classes over the 39 files resolve; see [scb.md](scb.md)). Coordinates are background-image pixels
(1408x960 for Croisement03; up to ~3500 for York).

### Placement (18 bytes)

Shared by actors, beam-me points, scrolls and `ZORG` entries.

| Offset | Type | Field | Observed |
|---|---|---|---|
| 0x00 | u16 | x | inside the map |
| 0x02 | u16 | y | inside the map |
| 0x04 | u32 | direction | 0..=15 (16 facings; overlays are consistent with 0 = east, counter-clockwise) |
| 0x08 | u32 | unknown_0x08 | actors: 3 (most), 0x88 = 136 (hidden PCs), 0x86, 0x0e, 0x9f, 0x10e, 0x2f, 0xa2, 0x2d, 0x30, 0; scrolls: always 190; `ZORG`: 189 + `unknown_b` |
| 0x0c | i16 | unknown_0x0c | -1 or an id (0..=~800). Together with the next two words this is the "placement qualifier" |
| 0x0e | u16 | unknown_0x0e | 0 or 6..=154 |
| 0x10 | u16 | unknown_0x10 | 0..=11 |

The qualifier triple (also found on waypoints, bushes, `GULP` points and polygons, rail points, objects) is
hypothesised to be (projection-area / sector index, its height or layer, layer index): the loader asserts that
characters "lie on a motion area, on the right layer and sector" and the triple is `(-1, 0, 0)` for ground-level
placements and non-trivial on the rock plateau of the tutorial map. Not verified against the `.rhp` geometry yet.

### Polygon

`u8 unknown_a`, `u16 n`, `n x (u16 x, u16 y)`, `u8 unknown_b`. The two bytes look random (an editor colour or
hash); they are stored but not interpreted.

## `FOOT` (version 4)

`u32 map_id` (100 Croisement01, 101 Croisement03, 86 Croisement02, 194 Lincoln, 444 Nottingham, 413 Derby,
639 Leicester, 916 York, 92 Sherwood; equals the first word of the `.rhp` `SPOK` chunk, which is how the loader
matches "proto-level" and mission), `u32 variant` (1 for 28 files, 2 for 4, 4 for 6, 16 for H07; the Day / Night /
Fog / Custom ambiance as bits is the hypothesis, consistent with `Levels/Custom1/Nottingham.map` existing for H07),
`pstring16 map name`, `u32 mission_id` (0..=0x33; 0 for eight ambush/tactical missions).

## `POUF` (version 3)

`u16 count`, then entries. Each entry starts with `pstring16 sprite` (an `Animations/<Variant>/<name>.rhs` bank,
e.g. `Trapcr03`, `Derpatch`) and `pstring16 label` (`Croisement03 - piege02h`, `Derby - Pont_levis01`). The rest
of the entry is **not decoded**; the reader locates the next entry by its two printable strings (this splits all 39
files cleanly and the `count` is honoured). What is visible in the body: a position, a `u16`, three flag bytes, an
empty polygon, `01 00 00 00 00`, three flag bytes, a second position, ten bytes, then a run of seven or more
4-byte groups `XX 00 00 YY` (empty polygons?) and a tail `00 01 <u16 n> <n x u16> 00 00`. Objects (`BOOM`) refer to
an entry by repeating its two strings.

## `BOYZ` (version 3): actors

`u16 count (6)` then six sub-chunks (`tag size version body`) in fixed order. Every body starts with `u16 count`.

| Tag | Version | Records | Meaning |
|---|---|---|---|
| `MEOW` | 2 | 0 in all files | unknown (animals?) |
| `SCOT` | 4 | 1..=5 per mission (50 in `Sherwood.rhm`) | player characters |
| `OILE` | 3 | 0..=77 | civilians |
| `TOTO` | 2 | 0..=11 | named NPCs (wedding guests, prisoners, trainer) |
| `BORG` | 4 | 3..=184 (2463 total) | non-player humans: soldiers, guards, and merry-men NPCs |
| `BOOM` | 5 | 1..=35 (478 total) | objects: targets, traps, cart parts, mechanisms |

### `SCOT` record

| Offset | Type | Field | Observed |
|---|---|---|---|
| 0x00 | Placement | placement | `unknown_0x08` = 3 (visible PC) or 136/134 (hidden PC to be activated by script), 14, 0 |
| 0x12 | u32 | unknown_0x12 | 0..=4 |
| 0x16 | u8[10] | unknown_0x16 | zero except single `01` bytes at offsets 2, 4, 5, 6, 8 or 9 in 30 records |
| 0x20 | opt name | name | `hidden_pc01_80000048`, `BeamMeRobin_8000002f`, `Heros_8000001f`; ordinary PCs have none |
| | u8 | unknown_trailer | 0; 4 in exactly one record of 12 story missions; 2 (H09); 5 (outro) |

### `BORG` record

| Offset | Type | Field | Observed |
|---|---|---|---|
| 0x00 | Placement | placement | |
| 0x12 | u32 | unknown_0x12 | 0 (1279), 2 (662), 3 (404), 1 (85), 4 (33) |
| 0x16 | u32 | profile | character profile index, 62 distinct values 1..=~60. Tutorial: 30 = "Lancier" (lancer), 6 = "Epee" (swordsman), 18 = "Officier", 42 / 43 = merry men with bow / staff. The profile table (index -> `Characters/*.rhs` / stats) is not located yet |
| 0x1a | u8 | unknown_0x1a | 0 or 1 (127 records); "patrol chief" is the hypothesis |
| 0x1b | u32 | unknown_0x1b | 0 (2073), 1..=20, 50, 99, 100 |
| 0x1f | u32 | unknown_0x1f | always 0 |
| 0x23 | u32 | unknown_0x23 | 0 (2293) or 10..=100 (a percentage) |
| 0x27 | u16 + u16[] | members | list of `BORG` indices (0..=count-1); 63 records have one, 1..=7 entries |
| | i16 | rail | index into `RAIL` (patrol path) or -1 (1748 of 2463) |
| | i16 | unknown_i16 | -1 (2152) or 7..=22 |
| | opt name | name | `Lancier03_800000db`, `SQD03_Officier_80000153`, ... |

### `OILE` record

| Offset | Type | Field | Observed |
|---|---|---|---|
| 0x00 | Placement | placement | `unknown_0x08` 3, 159, 270, 47, 45, or 0 |
| 0x12 | u32 | unknown_0x12 | 0..=3 |
| 0x16 | u32 | profile | 0..=21 |
| 0x1a | i16 | unknown_i16_a | -1 (322 of 427) or 0..=63 |
| 0x1c | i16 | unknown_i16_b | 0, 25, 1500, 2000, 3000, 4000, 4500 |
| 0x1e | u16 | unknown_u16 | always 0 |
| 0x20 | lists | lists | **only when `profile == 1`** (28 records, all with `unknown_0x08 == 0`): ten lists, each `u16 n` + `n x u16` (ids up to 0x2c; 0..=3 per list) |
| | opt name | name | `PoorWeepingOne_80000344`, `JeuneCollecteur_80000109`, ... |

### `TOTO` record

Placement, `u32 unknown_0x12` (0..=3), `u32 profile` (1..=9), `i16 unknown_i16_a` (0/1), `i16 unknown_i16_b` (0),
optional name (`Scarlett_800001e1`, `LittleJohn_800001e2`, `Mariane_800001e3`).

### `BOOM` record (objects)

| Offset | Type | Field | Observed |
|---|---|---|---|
| 0x00 | u16, u16 | x, y | |
| 0x04 | i16 | unknown_0x04 | -1 (370 of 478) or 75..=300 (bow targets: 80, 150, 170, 180 ...) |
| 0x06 | u16 | unknown_0x06 | 0 |
| 0x08 | u16 | unknown_0x08 | 0 |
| 0x0a | u16 | unknown_0x0a | 0 or 148..=195 |
| 0x0c | u16 | unknown_0x0c | 0 |
| 0x0e | i16, u16, u16 | placement qualifier | as in Placement |
| 0x14 | pstring16 | sprite | `TG_BowTarget`, `TG_MerryManStaff`, `Trapcr03`, `chariot05`, `Linpatch`, `Derpatch` |
| | pstring16 | label | `Bow Target`, `TARGET_PaysanDbaton 08`, or a `POUF` label |
| | u32 | unknown_flags | 0 (292), 1 (115), 16 (31), 2, 4, 97, 128, 68 |
| | u16, u16 | x2, y2 | anchor position, shared by the parts of one element (`chariot05_b1..b4`) |
| | u16, u16 | unknown_q2, unknown_r2 | qualifier b, c of the anchor |
| | Polygon | polygon | 0..=8 points |
| | u8 | unknown_u8 | always 1 |
| | opt name | name | every retail object is named (`cible_filet02_8000002a`, `rocher_b1_800000ab`) |

## `ZORG` (version 2)

`u16 count`; records: `u16 unknown_a` (0..=18; 9 and 0 most common), `u16 unknown_b` (1..=5), Placement with
direction 0 and `unknown_0x08 == 189 + unknown_b` in all 449 records. Entries cluster in groups of 2-3 near
scroll or actor positions (in the tutorial two entries sit next to the "Tresor" scroll). Bonus / pick-up items with a
kind and a stack size (the executable's `Bonus[...]` chunk) is the working hypothesis; not verified.

## `HIRN` (version 2)

`u16 count (4)` then four sub-chunks, each starting with `u16 count`:

| Tag | Version | Record | Meaning |
|---|---|---|---|
| `HOLE` | 2 | `u16 x, u16 y, u16 q_b, u16 q_c, u16 direction` (10 bytes) | waypoints; the same position repeats with different directions (turn-in-place targets). 0..=674 per file |
| `BUSH` | 2 | `u16 x, u16 y, u16 q_b, u16 q_c` (8 bytes) | positions; 0..=607 per file, only in town maps and a few forests. Hiding places is the hypothesis |
| `POW ` | 2 | Placement (18 bytes) | "beam-me" points: all lie on the map border with the direction pointing inwards (tutorial: 10 points at x <= 50 or y <= 20 or y >= 944); used to bring PCs / reinforcements in. `unknown_0x08` is 0 |
| `NLIP` | 2 | `u32 unknown (0)`, Polygon, `u16 n`, `n x (u16 x, u16 y, u16 q_b, u16 q_c, u8 flag, u16 value)` | tactical zone with points inside; only Emb04 / Tac18 (2 entries) |

## `RAIL` (version 3): patrol paths

`u16 count`, then paths: `u16 n` points. Point:

| Field | Type | Meaning |
|---|---|---|
| x, y | u16, u16 | position (may be off-map for cart entry / exit paths: the first two paths of every ambush have 4 points ending outside the map) |
| q_b, q_c | u16, u16 | placement qualifier b, c |
| kind | u8 | 0 = command program, 1 = named point |
| payload | `u16 length` + bytes | kind 1: the name (`Point1__0___8000039f`, `Sold01__1___80000130`: `<label>__<n>___<id>`, referenced by scripts); kind 0: a program (may be empty) |

A program (length > 0) is a small offset-addressed structure; all offsets are relative to the program start and,
in every retail file, equal to the position of the next byte, so it reads sequentially:

```
u16 table_count; table_count x (u8 table_id, u16 table_offset)
per table: u16 block_count; block_count x (u8 percent, u16 block_offset)
per block:  u16 length; commands
```

`table_id` is 0 for a single table (2361 cases) or the ids 1 and 2 when a waypoint has two tables (11 cases; the
id could select the actor role or travel direction: unknown). `percent` values of one table sum to 100 in all but a
handful of tables (`100`; `50 50`; `25 50 25`; `25 25 25 25`; `60 40`; `70 30`; single `50`, `25`, `10`, `30`, `60`,
`75`): a block is executed with that probability.

Commands are `u8 opcode` + fixed-size operands (sizes established by exhaustive parse of all 2879 programs;
meanings are not: the engine's interpretation of six of them is an inference, see "Rail programs" below):

| Opcode | Operands | Count | Observed operand values |
|---|---|---|---|
| 0x00 | none | 159 | |
| 0x01 | none | 35 | |
| 0x02 | u16 | 122 | 0..=0x2f |
| 0x03 | u16 | 2203 | 0..=15 (a direction) |
| 0x04 | u16 | 3450 | 10, 12, 25, 50, 75, 100, 150, 200, 250, 300, 500 (a duration) |
| 0x05 | u16, u16 | 104 | e.g. (38, 125) |
| 0x06 | u16, u16, u16 | 12 | e.g. (9, 1250, 5), (11, 300, 0) |
| 0x07 | none | 370 | |
| 0x08 | u16 | 12 | 0x13..=0xb1 |
| 0x09 | none | 111 | |
| 0x0a | none | 76 | |
| 0x0b | none | 364 | |
| 0x0c | none | 364 | |
| 0x0d | u16 | 101 | 25..=1000 |
| 0x0e | none | 3 | |
| 0x0f | u16 | 2 | 10, 12 |
| 0x10 | none | 2 | |
| 0x81 | f32 | 52 | 0.0, 5.0, 6.0 |
| 0x82 | f32, u16 | 26 | (8.0, 1), (8.0, 2) |

The executable names eight waypoint commands (`Bend`, `LookLeft`, `LookRight`, `CheckFor`, `CheckForSync`,
`PatrolStart`, `PatrolDirection`, `PatrolStop`) and "mobile element waypoint commands"; 0x81/0x82 with floats
appear only on the cart paths and are the mobile commands. Which opcode is which name is **not** established.

### Rail programs: structure observed across the 39 files

Counts below are over all 1638 rails (542 of them assigned to a `BORG` actor through `rail`; the rest are
referenced by scripts). Status: **inferred from data layout only**, not verified against the original's behaviour.

*Table ids select the travel direction.* Where a point has a single table its id is 0 on 2361 points; ids 1 and 2
are placed asymmetrically: id 2 alone on 137 first points, 155 middle points and 1 last point; id 1 alone on 146
last points, 67 middle points and 1 first point; both ids on 11 points (3 first, 7 middle, 1 last). Single-point
rails (802) only ever carry table 0. A rail is therefore walked back and forth (`0, 1, .., n-1, n-2, .., 1, 0, ...`):
you can only reach the last point travelling forward and the first point travelling backward, so **id 1 = program
run when arriving forward, id 2 = when arriving backward, id 0 = either direction**. A point that only has the
table of the other direction runs nothing on that arrival.

*Command pairs.* The most frequent block shapes are `03 04` (721 blocks), `04` (696), `03 07` (289), `03 04 03 04`
(129) and `03 04 0b 04 0c 04` (27, plus longer repetitions of the same triple). The bigram `03 -> 04` occurs 1816
times; `0b` and `0c` (364 each) are almost always followed by `04` (291 and 298 times) and appear as a pair in one
block, in either order, after a `03 04`.

*0x02 operand is a point index.* In all 122 occurrences the operand is smaller than the rail's point count (rails
with 3, 5, 7 ... points included); it is 0 on 79 last points (72 times as the last command of the block) and a
larger index in the remaining cases, e.g. `H04_Lei_VL` rail 9 (36 points): point 0 has `02(12)` and point 35 has
`02(22)`; `H12_Not_MP` rail 1 (7 points): point 2 jumps to 4 and point 4 to 2.

*0x07 ends a program.* It is always the last command of its block (370 of 370), on single-point rails (284), last
points (62) or first points (23), never on a middle point; 343 of the 370 are on rails that no actor is assigned
to (script-driven walks that end at the point).

*0x00 / 0x01 / 0x09 / 0x0a and the operand opcodes 0x05, 0x06, 0x08, 0x0d* have no positional pattern that suggests
a meaning; `0x00` is a whole block by itself 113 times, `0x09` ends blocks (`03 04 09`, 43 middle points).

### Rail programs: engine interpretation

`crates/opensherwood-app/src/mission.rs` (`compile_rail`) translates each rail assigned to an actor into a core
waypoint program that walks the points back and forth as above, running the arrival table as a probabilistic
choice over its blocks (a roll no block covers runs nothing). The per-opcode mapping, with its status:

| Opcode | Engine behaviour | Status |
|---|---|---|
| 0x03 `dir` | face the 16-way direction (same convention as Placement) | inferred (operand range 0..=15, precedes waits) |
| 0x04 `n` | stand for `n` hundredths of a second (`n * tick rate / 100` ticks) | inferred: the value set 10, 12, 25, 50, 75, 100 ... 500 reads as 1/100 s; unit **not verified** |
| 0x02 `p` | continue the patrol at point `p` of the same rail, walking forward if `p` is a later point and backward if earlier (`02(0)` on the last point turns the back-and-forth walk into a loop) | inferred (operand always a point index) |
| 0x07 | stop: stand here for good | inferred (always terminal) |
| 0x0b / 0x0c | glance 45 degrees to one side / the other of the block's last 0x03 facing (relative to the current facing if there is none) | inferred; which one is left is **not** established, the choice only mirrors the glance order |
| all others (0x00, 0x01, 0x05, 0x06, 0x08, 0x09, 0x0a, 0x0d, 0x0e, 0x0f, 0x10, 0x81, 0x82) | no-op, counted and logged at load (`opensherwood: mission ...: N translated, M unknown`) | unknown |

Coverage at load of `H01_Lin_VL` (18 rails assigned to actors): 113 commands, 102 translated, 11 unknown
(`0x00` x1, `0x01` x2, `0x05` x4, `0x0d` x4); the tutorial `EmbTut_FoC_EC`: 3 rails, 3 commands, all translated.

## `SKRO` (version 4): scrolls

`u16 count`; records: Placement (direction 0, `unknown_0x08` 190 in all 242 records), `u8[5] unknown_flags`
(`01 01 01 00 00` in 167 records, `01 01 01 01 01`, `01 01 01 01 00`, `01 01 01 00 01`, `00 00 00 00 00`,
`00 01 01 00 00`), optional name (`Archer01_8000012d`, `ParchArgent_8000047f`; unnamed only when the flags are
all zero).

## `TING` (version 3): mobile elements

`u16 count` (0 or 1) then per entry:

- `FLIM` sub-chunk (version 2): `u16 count (1)`, items: `pstring16 sprite` ("chariot05"), `pstring16 animation`
  ("chariot05_cart8"), `i16 dx`, `i16 dy` (negative offsets, e.g. -200, -71), `u16 0`, `u8[3] = 01 01 01`,
  Polygon (0 or 6 points).
- `WOAW` sub-chunk (version 3): `u16 count` (0 in retail data); any further bytes are kept raw.
- Polygon (3 points: the cart footprint), `u16 x, u16 y` (the cart position; it equals the first point of the
  second `RAIL` path in the tutorial), `u16 0`, `u32 unknown_b` (0 or 1), `u16 0`, `u32 3`, `i16 -1`.

## `GULP` (version 2): script sectors

`u16 n` points (`x, y, q_b, q_c`; 0..=213; in the tutorial they coincide with actor / object positions), then
`u16 m` polygons: `u8 unknown_a`, `u16 k`, `k x (x, y)`, `u8 unknown_b`, `u16 q_b`, `u16 q_c`, optional name
(`filet02_80000027`, `trou01_v2_8000003e`, `prison_entree01_800003c9`, `DuelZone_80000209`). These are the zones
scripts test with `EnterZone` / `ExitZone` (the script classes carry those handlers, see scb.md). The executable
requires script sectors to have at least 3 points; retail polygons have 4..=17.

## `CAVE` (version 3)

`u16 count`; entries: `u16 n`, `n x u16 id`, `u8 flag`. The count is constant per map (Lincoln 19, Nottingham 45,
Derby 14, Leicester 16, York 74, Sherwood 5, Croisement02 1, others 0) so it indexes a per-map table of the
`.rhp` (which one is not established: it is not the `WOAW`, `AZ `, `FARM`, `LOUD`, `DARK` or `TUPO` count for every
map). Ids up to 0x90; 24 of 596 entries are non-empty; flag 1 in 7 entries. Buildings with their initial occupants
("Tenant"), lifts, or jump zones are the candidates.

## Cross-references established

- `FOOT.map_id` == `.rhp` `SPOK` first word for all nine maps.
- Every script class name in the paired `.scb` (1409 classes, excluding the level class `StartUp`) is the name of an
  actor, object, scroll, script polygon or named rail point of the mission (100 %); the reverse also holds.
- `BORG.rail` indexes `RAIL` (all values < path count), `BORG.members` index `BORG`.
- Overlay of the tutorial over `Levels/Day/Croisement03.map` (`opensherwood-tools rhm-overlay`): soldiers stand
  on the road, hidden PCs behind the river, archers next to their bow targets, patrol paths follow the road, beam-me
  points sit on the border, script polygons surround the net / hole positions.

## Open questions

- The placement qualifier triple against `.rhp` geometry; the `profile` index table; the waypoint command
  names; `POUF` entry body; `CAVE` target table; `ZORG` kind / count; `OILE` ten-list block; `SCOT` flag bytes and
  trailer; `BOOM` unknown words; the `NLIP` point values.

## Provenance

Observation only: chunk walker and record-grammar probes over all 39 files
(`harness/tools/re/rhm_inventory.py`, `rhm_probe.py`, `rhm_chunks_probe.py`, `rhm_full.py`; every grammar is
accepted only when it consumes every chunk of every file exactly), cross-file value histograms, the class-name join
with the `.scb` files, and PNG overlays over the decoded map backgrounds (`rhm_overlay.py`, and the
`rhm-overlay` tool). Executable knowledge is limited to printable strings (chunk names in version-check messages,
waypoint command names, loader assertions), see `docs/original/executable-notes.md`.
