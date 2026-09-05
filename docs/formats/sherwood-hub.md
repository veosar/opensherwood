# The Sherwood camp (`Sherwood.rhm` / `sherwood.scb`) and the outro (`SherwoodOutro.rhm` / `.scb`)

Status: **implemented** (2026-09-05, implementer session: steps 1..=3 of section 6 are in the engine, all 39
missions load strictly and run 300 ticks; step 4, the native rows, is with the lead; step 5 is campaign work).
The analysis below is **from data only** (2026-09-05, analyst session; no executable analysis). The two levels are
ordinary `DUTY` missions on the `sherwood` map; nothing in their container, actor records, profile indices or
sprites differs from the 37 loadable missions. The single reason the strict loader refuses them is the script
binding: `map_element_count("sherwood")` is `None`, so `docs/formats/scb.md` "Index spaces" cannot place the
script's element indices. This document establishes that index space, and in doing so corrects the model for
every map (the player-character slots sit at the *end* of the element table, not at its start, and the
per-map prefix is the `.rhp` `FLIM` count plus its `TUPO` count). Confidence is given per claim.

Vocabulary: `nNN(...)` is native id NN of `docs/formats/scb.md`; element indices are the immediates passed to
native 3; locations are the immediates of native 6 (`GULP` points then polygons). Designer names, labels and
game text are never reproduced: elements are named by chunk, index and role.

## 1. Files

| Level | Mission file | Script | Map | Background | Text index | Level record |
|---|---|---|---|---|---|---|
| Camp (hub) | `Data/Levels/Sherwood.rhm` (`FOOT`: map id 92, variant 1, mission id 0) | `Data/Levels/sherwood.scb` (lower-case base name, the only such pair; the engine's VFS is case-insensitive, `crates/opensherwood-assets`) | `sherwood.rhp` (1920x1088) | `Levels/Day/Sherwood.map`, `Night/sherwood.map`; `.min` in Day / Night / Fog | `Text/RHLevelHQ.red` (192 bytes: a 37-entry text list, 1 won, 1 lost, 7 short briefings) | `HQ` in `profile.cpf` (kind 4 = Sherwood, location 8) |
| Outro | `Data/Levels/SherwoodOutro.rhm` (map id 92, **variant 4**, mission id 0) | `Data/Levels/SherwoodOutro.scb` | same | variant 4 has no `Fog/sherwood.map` (only a `.min`); which background the original uses is **open** (since 2026-09-05 the engine selects the ambiance directory by variant, `rhm.md` `FOOT`, and, this picture being absent, falls back to `Day` with a log line) | `RHLevelVO.red` (148 bytes, 4 texts) is the campaign-flow mapping; the level record whose map is Sherwood and whose code is not `HQ` is `EY` (`profile.md`). The outro script shows no text, so the choice does not matter for loading | `EY` (or `VO`), `unknown_h` = 0, `unknown_i` = 0 |

Confidence: high (file listing, `FOOT` chunks, `.red` sizes, `profile.md`).

## 2. Why the loader refused them (observed before 2026-09-05)

`Engine.reset({"mission": "Sherwood"})`, `"sherwood"` and `"SherwoodOutro"` all fail with

```
engine error -32000: mission script: no element index space known for map sherwood
```

(`crates/opensherwood-app/src/mission.rs`, `translate_script`, via `build_spec_checked`). Everything before
that step passes: the `.rhm` parses, the `.scb` parses (31 and 11 classes, all 30 + 10 element classes resolve
to a named record of the mission), every `BORG` / `OILE` / `TOTO` profile index is inside its table and every
referenced sprite profile loads (with `OPENSHERWOOD_LENIENT_ASSETS=1` the only log line is `script not attached:
no element index space known for map sherwood`; no profile or sprite fallback fires). In lenient mode the hub
runs 300 ticks with 54 entities (50 player slots, 3 `BORG`, 1 `TOTO`) and the outro with 36 (2 + 34), without a
script. Confidence: high (run on the current `target/release/opensherwood.exe`).

So "profile / sprite indices beyond the other missions' ranges" is **not** what is special about the hub: its
profile indices are 44 (the camp trainer), 28 x2 (knight family) and `TOTO` 9 (a generic merry man); the outro's
are `OILE` 2, 3, 5, 6, 8, 15, `TOTO` 2, 3, 5, 6, 7, 8, 9 and `BORG` 8, 14, 21, 27, 32. All inside the tables
(`rhm_profiles.py`). The "unknown index space" is the *script element* space of native 3.

## 3. What the two mission files contain

Counts (`harness/tools/probe/rhm_inventory.py`, `opensherwood-tools rhm`):

| Chunk | Hub | Outro | Typical mission (for contrast) |
|---|---|---|---|
| `POUF` | 0 | 0 | 0..=26 |
| `SCOT` | **50**, all unnamed, `unknown_0x08` = 3 (46) or 0 (3) or 14 (1); 2 with a non-trivial placement qualifier `(86, 31, 4)` | 2: #0 named (the hero), `unknown_0x08` = 0, qualifier `(59, 27, 1)`; #1 unnamed, trailer 5 | 1..=5 |
| `OILE` | 0 | 18 (profiles 2 x5, 3 x4, 5, 6 x3, 8 x3, 15 x2) | 0..=77 |
| `TOTO` | 1 (profile 9, a generic merry man) | 11 (profiles 3, 5, 2, 6, 7 x3, 8 x2, 9 x2; three named: the big man, the swordsman, the lady) | 0..=11 |
| `BORG` | 3: #0 the trainer (profile 44, named), #1 / #2 profile 28, unnamed | 5, all named, profiles 21, 27, 8, 14, 32 (an officer and four soldiers) | 3..=184 |
| `BOOM` | 4: three archery targets (`TG_` sprites, flags 1, named; one with `unknown_0x04` = 90) and one arrow object (`TG_` sprite, `unknown_0x0a` = 190, named) | 9 unnamed marker objects (empty polygon, flags 0), one per outro actor role | 1..=35 |
| `ZORG` | 0 | 7 (`unknown_a` 12..=18, `unknown_b` 1) | 0..=70 |
| `HIRN` | empty | empty | waypoints, bushes, beam-me points |
| `RAIL` | 0 | 1 (5 points, 3 named) | 0..=73 |
| `SKRO` | **23**, all named: 12 with flags `01 01 01 01 01`, 1 with `01 01 01 01 00`, 7 with `01 01 01 00 00`, 1 with `01 01 01 00 01`, 2 at the same position with `01 01 01 01 00` / `01 01 01 00 01` | 0 | 0..=29 |
| `TING` | 0 | 0 | 0 / 1 |
| `GULP` | **213 points**, 15 polygons (2 named) | 56 points, 1 polygon (named) | 0..=213 points |
| `CAVE` | 5 (the per-map constant) | 5 | per map |

Confidence: high (parsed by the retail-exact reader).

Roles of the hub's elements (own words, from the script's use of them; see section 5):

| Role | Chunk / index | Count |
|---|---|---|
| Player-character slots (the gang; the campaign fills them) | `SCOT` 0..=49 | 50 |
| A merry man who joins as a recruit (deactivated and put off-map at load, brought in by `Hourglass`) | `TOTO` 0 | 1 |
| The trainer (AI locked / unlocked by the level, shoots at trainees in the sword-training zone) | `BORG` 0 | 1 |
| Two knight-family actors, AI-locked and teleported off-map at load (never used afterwards) | `BORG` 1, 2 | 2 |
| Archery targets (each reports an arrow hit to the level with message 0) | `BOOM` 0..=2 | 3 |
| An arrow object with empty handlers | `BOOM` 3 | 1 |
| Information scrolls, one per camp activity (drink, roast, arrows, herbs, purses, buffet, apples, nets, stones, bow training, sword training, wasps); taking one shows text 2..=13 and sets a bit of campaign word 0 | `SKRO` 0..=11 | 12 |
| Information scroll of the deployment zone (text 23) | `SKRO` 12 | 1 |
| Seven scrolls attached to the seven campaign roster slots (`n215(0..6)`); taking one shows text 16..=22 and sets a bit of campaign word 11 | `SKRO` 13..=18, 20 | 7 |
| The silver-arrow scroll (text 31, bit 14 of word 0) | `SKRO` 19 | 1 |
| The production report and its information scroll (same position) | `SKRO` 21, 22 | 2 |
| Sword-training zone (empty handlers; its location is 222) | `GULP` polygon 9 | 1 |
| Deployment zone (the team assembles here; location 227) | `GULP` polygon 14 | 1 |
| Unnamed polygons: work areas and camera / walk targets (locations 213..=226) | `GULP` polygons 0..=8, 10..=13 | 13 |
| Work spots, walk targets, camera targets (locations 0..=212) | `GULP` points | 213 |

The manual (pp. 12-14, paraphrased) describes the camp as: new recruits appear after missions; workshops
produce items between missions and a parchment near the central tree reports the output; the feast, archery
and combat workshops rest and train the men; hovering a man shows his abilities; the MAP icon selects a mission
and the team is gathered on the path out of the forest, then sent. The elements above map onto that: the
production zones (natives 199 / 200), the report scroll, the training helpers, the trainer, the deployment
zone and the recruit.

## 4. The element index space (the finding)

### 4.1 Model

The flat table addressed by native 3 is, for **every** level:

```
[ map FLIM entries ]  [ map TUPO entries ]  [ POUF ]  [ OILE ]  [ TOTO ]  [ BORG ]  [ BOOM ]  [ ZORG ]  [ SKRO ]  [ TING ]  [ SCOT ]  [ GULP polygons ? ]
   0 .. F-1             F .. F+T-1
```

with `F` = the `FLIM` count and `T` = the `TUPO` count of the map's `.rhp`. The relative order of the three
per-map / per-mission prefix parts (`FLIM`, `TUPO`, `POUF`) is not observable (their counts are constant per
map or per mission); their *sum* is what places the mission's records. The position of the script polygons is
not observable either (no polygon class references itself by index in any file); the hub's script never
addresses an index beyond its scrolls, so it does not matter for loading.

Per-map prefix `K = F + T` (since 2026-09-05 computed from the `.rhp` by `map_element_count`; the table is kept as
`known_map_element_count`, a cross-check asserted by `crates/opensherwood-script/tests/gamedata.rs`; "current value"
below is the value the engine used before):

| Map | `FLIM` | `TUPO` | `K` (new) | current value | change |
|---|---|---|---|---|---|
| Croisement01 | 13 | 6 | **19** | 14 | +5 |
| Croisement02 | 15 | 9 | **24** | 19 | +5 |
| Croisement03 | 15 | 9 | **24** | 19 | +5 |
| Derby | 13 | 7 | **20** | 19 | +1 |
| Nottingham | 48 | 11 | **59** | 58 | +1 |
| Leicester | 47 | 16 | **63** | 59 | +4 |
| Lincoln | 38 | 12 | **50** | 49 | +1 |
| York | 60 | 10 | **70** | ~67 | +3 |
| Sherwood | 20 | 0 | **20** | none | new |

The `TUPO` count is the `u16` at the head of the raw chunk (`rhp::Map::tupo`). `TUPO` records are the map
patches ("pixel_vert" / "notpatch" strings in the spec) and native 5 (patch by index) has per-map ranges that
match these counts (Lincoln `<= 11` vs 12, Leicester `<= 15` vs 16): the element table contains the patches
right after the animated elements. Confidence: **high** for `K` (nine maps, derived below), **medium** for the
identification of the second block as `TUPO` (the counts match in all nine maps and native 5's ranges agree;
no script addresses a patch through native 3, so the block is only ever *skipped*).

### 4.2 Evidence: the mission records

`harness/tools/probe/scb_elements.py` compares, for every class whose name is a mission record, the native-3
immediates used inside that class with the record's index in its chunk; the most frequent difference is the
chunk's base. The scratch probe `scot_place.py` (a copy of it in the analyst scratchpad) converts each base into
"index of the first `OILE` record minus the `POUF` count", i.e. `K` if the `SCOT` records are *not* in front,
or `K + SCOT count` if they are. Results (hits = self-references agreeing):

| Map | Missions (SCOT count): implied `K` from `BORG` / `BOOM` self-references | Consistent only if |
|---|---|---|
| Nottingham | H02 (5): 59 (2 hits); H07 (1): 59 (6); H09 (4): 59 (2); S01 (1): 59 (4) | `SCOT` not in front: with `SCOT` first the four would need `K` = 54, 58, 55, 58 |
| Lincoln | H01 (1): 50 (3 + 5); H05 (5): 50 (2) | same argument (49 vs 45) |
| Derby | H03 (4): 20 (2); S04 (4): 20 (2); Str02 (5): 20 (2) | same (16 vs 15) |
| Leicester | H04 (4): 63 (3); S02 (4): 63 (5 + 3) | either (both have 4 slots) |
| York | Str03 (5): 70 (2); H10, S05: no `BOOM` / `BORG` self-reference | `K` = 70 = 60 + 10 |
| Croisement01 | Emb01, Emb04, Emb08, Tac01 (all 5): 19 (5, 6, 4, 8) | either; 19 = 13 + 6 |
| Croisement02 | Emb05, Emb07, Emb09, S03 (4), Tac02: 24 (12, 5, 7, 2, 11) | S03 has 4 slots, the others 5: `SCOT` not in front |
| Croisement03 | Emb02, Emb03, Emb06, EmbTut, Tac03: 24 (5, 9, 8, 5, 9) | either; 24 = 15 + 9 |
| Sherwood | Outro (2): `TOTO` 20 (3 hits), `BORG` 20 (5 hits) | 20 = 20 + 0 |

Every map gives one value regardless of the mission's `SCOT` count, and that value is `FLIM + TUPO` in all nine
maps. Under the current model (`SCOT` first, `K` derived from one mission per map) eleven of the 37 loadable
missions are bound with their mission part shifted by 1..=4 entries: H02, H09, H12 (Nottingham, 5 / 4 / 5
slots against a `K` derived from 1-slot missions), H05, Str01 (Lincoln), H03, S04, Str02 (Derby), H10, S05,
Str03 (York). H01, the tutorial and the other forest missions are unaffected (same slot count as the mission
their `K` came from), which is why the first mission's walkthrough still matched. Confidence: **high**.

### 4.3 Evidence: the tail (`ZORG`, `TING`, `SCOT`)

Four files have a named `SCOT` class (a hidden player character, or the hero) that references itself by index:

| Mission | `SCOT` self-reference (index of slot 0, hits) | `K + POUF + OILE + TOTO + BORG + BOOM + SKRO` | `+ ZORG` | `+ TING` | Match |
|---|---|---|---|---|---|
| Tac01_FoA_MP | 107 (5 hits, five hidden PCs) | 19 + 23 + 45 + 18 = 105 | +2 = 107 | +0 | exact |
| Emb03_FoC_MP | 107 (4) | 24 + 18 + 34 + 24 = 100 | +6 = 106 | +1 = 107 | exact |
| Emb04_FoA_MP | 93 (5) | 19 + 17 + 30 + 16 = 82 | +10 = 92 | +1 = 93 | exact |
| SherwoodOutro | 70 (1; the hero's class also references its own marker object at 54 = `BOOM` 0) | 20 + 18 + 11 + 5 + 9 = 63 | +7 = 70 | +0 | exact |

The outro's level class confirms it independently: its `Initialize` flags elements 70 and 71 with `n180(x, 1)`
(the "initialise player characters" idiom of every mission) and its cutscene moves, animates and messages
element 70 as the hero (messages 2000..=2002, handled by the hero's class), 71 as the second slot. In H01 the
new tail puts the single `SCOT` slot at 126, which is the element whose two attributes the level's `Initialize`
zeroes (`n117(126, ..)`, previously read as "a polygon or the CAVE list"), and the block 115..=125 is exactly
its 11 `ZORG` entries. Confidence: **high** for `SCOT` after both `SKRO` and `ZORG` and after `TING` (four
exact matches over counts 2 / 6 / 10 / 7 and 0 / 1 / 1 / 0); the order *between* `SKRO` and `ZORG` is not
observable here (only their sum places `SCOT`) and is fixed by `docs/original/h01-win-path.md` section 2 as
`.. BOOM, ZORG, SKRO, TING, SCOT` (the file's chunk order; the corpus-wide scroll-state calls and the oracle
agree), so in H01 the block 100..=110 is the `ZORG` items and 111..=125 the scrolls; the `ZORG` and `TING`
entries are addressed by no class of their own (they are inert `Unmodelled` slots for the engine).

### 4.4 The hub under the model

`K` = 20, `POUF` = 0, `OILE` = 0: `TOTO` 0 = element 20, `BORG` 0..=2 = 21..=23, `BOOM` 0..=3 = 24..=27,
`SKRO` 0..=22 = 28..=50, `SCOT` 0..=49 = 51..=100, polygons 101..=115 (if they follow). The script's native-3
immediates are exactly 20..=50 (31 distinct values, every one used): 20 x6 (the recruit), **21 x16 (the
trainer: AI lock / unlock, `n101` action test, `n97` zone test, `n214`, `n229`, `n59` shoot-at)**, 22 / 23 x2
(the two knights: lock and teleport off-map), 24..=26 (the three targets, one per shooting position of the
archery-training helper), 27 (the arrow object, deactivated when a campaign bit is set), 28..=39 (the twelve
activity scrolls, initialised with campaign bits 2..=4096), 40 (the deployment-zone scroll, state 1 on the first
day), 41..=46 and 48 (the roster scrolls, `n215(0..6)`), 47 (the silver-arrow scroll, state 2 / 3), 49 / 50 (the
report scrolls, states 1 / 0 on the first day). Native 6 immediates are 0..=227 = 213 points + 15 polygons,
exact. Confidence: **high** (every index lands on an element whose role matches its use).

The outro: `OILE` 0..=17 = 20..=37, `TOTO` 0..=10 = 38..=48, `BORG` 0..=4 = 49..=53, `BOOM` 0..=8 = 54..=62,
`ZORG` 63..=69, `SCOT` 70, 71, polygon 72. Native-3 immediates 0..=71 (64 distinct), native 6 0..=55 (56
points), consistent. `Initialize` deactivates map elements 0..=16, 18, 19 (19 of the 20 animated elements of
the camp), AI-locks all 18 civilians and 5 soldiers, flags the merry men 40..=48 and the two slots as player
characters, and every named actor's class receives message 2000 (start) then 2001 / 2002 from the level's
1773-quad `ProcessMessage`, a choreography of 45 sequences (walks `n45` / `n48`, animations `n49`, expressions
`n62`, remarks `n69`, `n133` walk-to with a location, camera `n33` / `n34`) with no text.
`CheckVictoryCondition` returns 1 once a level variable is set. Confidence: high (structure), the choreography
itself was not traced.

## 5. What the hub's script does (`sherwood.scb`, 31 classes)

Level class: 26 variables (ten "last random action" counters, nine "the camp needs X" flags derived from
campaign flags, a shooting-position counter, first-day / speech-done / trainer-locked / post-map-initialised
flags, a speech id, a "the lady is here" flag), 30 functions, 4530 quads. 80 distinct natives at 926 call
sites. Element classes: the trainer (5 empty handlers), the three targets (`ActivatedByArrow`: message 0 with
`n10(self)` to the level), the arrow object (empty handlers), 23 scrolls (`IsTaken`: text + campaign bit), two
zones.

Load path (`Initialize` of the level, then of the elements, then `PostInitialize`), in order, all `[M]` unless
marked (opcode and native readings of `scb.md`; the native rows for the camp-only ids are low confidence):

- `Initialize`: lock the AI of and teleport off-map the two knights (22, 23); reset the ten counters and the
  flags; derive the nine needs from campaign progress flags `n210(k)` with k = 0, 11, 3, 4, 16, 12 | 13, 18, 15,
  10 (arrows, drink, purses, stones, nets, roast, wasps, plants, apples); `n214(trainer)` (one-shot, unknown);
  send the player characters standing in the deployment zone (location 227) to a random point between locations
  10 and 11 (`n204` / `n205` / `n213` / `n45`); if campaign word 10 is 1, deactivate the main player character
  and put it off-map (`n211`, `n113`, `n96(.., n159())`); scan the player characters (`n216` / `n217`) for one
  whose campaign id `n256` is 5 (the lady) and remember it; deactivate the recruit (20) and put it off-map; if
  wasps are needed start level sound 3 (`n150(n7(3))`). Returns 0.
- `PostInitialize`: if bit 0 of campaign word 0 is clear (`n195(0)`): first day, show text 0, a sequence
  (wait 150 ticks, page 34), set the report scrolls' states (50 -> 0, 49 -> 1) and set the bit (`n196(0, ..)`);
  then initialise the scrolls (each activity scroll gets state 3 if its bit of word 0 is set, else 1 / 0 by
  need; each roster scroll is attached to `n215(k)` when that slot is usable (`n85 == 0`), gets state 3 / 1 by a
  bit of word 11 and is moved to location 143 + i when the hero stands within 10 px of location 136 + i);
  compare the set of present roster slots with campaign word 2 and show text 24..=30 for each new one, store the
  set with `n196(2, mask)`, text 32 when all seven are present; texts 36 (`n261()`) and 35 (word 10) as
  applicable; objectives: `n26(0, 0)` (secondary), `n26(6, 1)` (primary).
- `Hourglass` (every call): move any player character in the deployment zone who is not one of the mission team
  members (`n163()` count, `n164(i)` member, `n86` identity) back to location 10; the recruit joins when it is
  not present (`n240 == 0`), `n172() == 16968` and `n255(26) == 0` (teleport to location 212, activate, `n232`
  join the party); once `n172() != 0` (a mission is selected: 16968 = `0x4248` reads as the two ASCII letters
  of the second story mission's level code, little-endian, so `n172()` is probably the selected level code
  **[L]**): objective 6 accomplished, objectives 1, (2 if the lady is here), 3, 4 added, and on the first day a
  cutscene (camera to location 211, page 23, back to the main character); then, gated per need by
  `n173() == 0`, the ten random work helpers (each picks a random player character in a work polygon, runs a
  sequence of walks / animations / waits and returns him), the archery-training helper (a random character in
  location 221 walks to a shooting position and shoots at target 24 / 25 / 26 with `n59(pc, 4, n10(target))`,
  positions rotate), the regeneration and sword-training helpers (when the trainer's action is not 54 and he
  stands in location 222: `n59(pc, 5, n10(trainer))`, wait, `n59(pc, 6, 0)`); when `n173() == 1` the main
  character's speech sequence runs once (objective 5, `n18`, `n112(0)`, expressions, a message to the level);
  the trainer's AI is unlocked when flagged; when the trainer is inside location 226 with state 2 (`n126`) he
  gets mode 1, AI lock, a `n53` action after 60 ticks and is sent to location 192; every call ends with
  `n229(trainer)`. Returns 0.
- `CheckVictoryCondition`: returns 0 always (the hub never ends by itself).
- `ProcessMessage`: 0 (param1 = element index of a hit target: attribute 0 += 1, a per-target hit counter);
  2 (a random expression 26..=28 or 29..=30 on the main character, then message 2 to `n111()` again after
  70..=130 ticks: a periodic idle animation); **1000** (for i < `n249()`: send `n250(i)` to the deployment zone:
  the mission team is sent to the exit); **1001** (initialise the production zones: `n199(k, location, capacity)`
  for k = 0..=12 with capacity 5, except 9 -> 170, 10 -> 340, 11 -> 20, then `n200(k, location)` work spots: 10
  for kind 0, 7 for kinds 6 and 12, 5 for 1..=5, 3 for 7 and 8). **No script in the 39 files sends 1000 or
  1001**: they must come from the engine (the SEND icon and the mission acceptance of the campaign screen are
  the obvious candidates) **[L]**.
- Deployment zone (polygon 14): `EnterZone` for a player character: if `n173() == 0` and `n163() >= n174()`
  or if `n172() == 0`, send him away (expression 14, walk to location 10); else `n165(actor)`, and when
  `n170() == 1` for the first time: objectives 1, 2, 3 accomplished. `ExitZone`: `n166(actor)`. So 165 / 166
  add / remove a character to / from the mission team, 163 is the team size, 164 its i-th member, 174 the team
  size limit, 170 "the team satisfies the mission's requirements", 172 the selected mission (0 = none) **[M]**;
  173 is a 0 / 1 state, not the "periodic chance" of the current row (it gates both the team limit and the
  random actions the same way) **[L]**.
- The sword-training zone (polygon 9) has empty handlers.

Natives used by the hub and by no other file (call sites): 150 (1), 163 (4), 164 (1), 165 (1), 166 (1), 170 (1),
172 (3), 173 (14), 174 (1), 199 (13), 200 (55), 214 (1), 215 (14), 239 (1), 248 (1), 249 (1), 256 (2), 258 (1),
261 (1). Engine status (`crates/opensherwood-core/src/natives.rs`): implemented 42 of the 80 ids, recorded stubs
32, **no row: 165, 166, 170, 174, 239, 249** (they would trap). None of the six is on the load path
(`scb_load_natives.py`: 35 ids reachable from `Initialize` / `PostInitialize`, all known); they are reached when a
player character enters or leaves the deployment zone (165 / 170 / 174, 166), when the deployment scroll is
taken (239) and by message 1000 (249). The outro uses 26 ids, all known (19 implemented, 7 stubs), none unique.

Campaign state the hub reads and writes (all through stubs today): progress flags `n210(k)` (k = 0, 3, 4, 10,
11, 12, 13, 15, 16, 18); campaign words `n195(k)` / `n196(k, v)` with k = 0 (activity bits 1..=16384, bit 0 =
"has been in the camp"), 2 (roster mask), 10 (a 0 / 1 state), 11 (roster-scroll bits): 195 / 196 are a
get / set pair of campaign bit words here, which refines the "availability of player action k" reading of
`scb.md` **[M]**; roster slots `n215(0..6)`; team natives 163..=166, 170, 172..=174; production 199 / 200;
`n249` / `n250` (the team to send); `n255(26)`, `n256(pc)` (campaign character id, 5 = the lady), `n261()`.

## 6. Implementation plan

1. **Done (2026-09-05).** **Replace `map_element_count`** (`crates/opensherwood-script/src/lib.rs`) by a value computed from the
   parsed `.rhp`: `flims.len() + tupo_count` where `tupo_count` is the leading `u16` of the raw `TUPO` chunk
   (add `Map::tupo_count()` or a typed count to `opensherwood-formats::rhp`; the engine already parses the
   `.rhp` in `Engine::load_map`, so pass the two counts, or the `Map`, into `mission::build_spec_checked` ->
   `translate_script`). Keep a table of the nine expected values as a data-backed test. Half a day.
2. **Done (2026-09-05; `ZORG` moved before `SKRO` the same day, `h01-win-path.md` 2).** **Reorder
   `MissionBinding::from_mission`**: `[map F+T] [POUF] [OILE] [TOTO] [BORG] [BOOM] [ZORG] [SKRO]
   [TING] [SCOT] [polygons]`. Entity numbering must stay the app's (`SCOT` entities first, then `OILE`, `TOTO`,
   `BORG` in file order): compute each group's first entity id from the counts instead of numbering in table
   order. `ZORG` and `TING` entries become `Element::Unmodelled`. Half a day including the unit tests
   (`map_element_count` tests at lib.rs ~1153, a synthetic mission with `SCOT` self-references in the tail).
3. **Done (2026-09-05).** **Tests and docs**: `harness/tests/data/test_mission.py` (39 loads, no accepted failures),
   `harness/tests/data/test_script.py` (`EXPECTED_AT_LOAD` rows for `sherwood` and `SherwoodOutro`; the
   load-time native counts of the eleven re-bound missions may change, re-record them with the same procedure);
   `docs/formats/scb.md` "Index spaces" and the H01 walkthrough (index 49 is a patch, 126 is the hero, 115..=125
   are `ZORG`; the "PC = 49" line goes), the per-mission table rows of the two Sherwood files; `docs/formats/rhm.md`
   (`SCOT` records are elements at the tail); `docs/status`. Half a day.
4. **Natives (done 2026-09-05, ruleset 13):** 165, 166, 170, 174, 239, 249 are recorded stubs in
   `natives.rs` (the taint and signature tables; arities from the corpus: 165 / 166 `(actor)` no value, 170 /
   172 / 173 / 174 / 249 `() -> int`, 239 `()` no value): 170 returns 0 until the team logic exists, 174 the
   policy limit 5 (`STUB_POLICY_VALUES`), 163 / 249 / 172 / 173 zero (172 = the selected level code, 0 = none;
   173 = the 0 / 1 state), 164 / 250 the main character; pinned by `policy_values_of_the_stub_table_are_pinned`.
   Still open: 195 / 196 (campaign word get / set), the `scb.md` rows for the six ids.
5. **The loading part is done** (2026-09-05: the hub and the outro load strictly with their scripts, the
   300-tick harness check applies to 39 of 39, and none of the six natives without a row is reached in it). The camp
   *gameplay* (campaign screen, team selection through the deployment zone, sending the team = message 1000,
   production between missions = message 1001 and natives 199 / 200, recruits, the report scroll) is campaign
   work beyond the loader; the outro's background variant (4) is open until the original is captured.

Expected effort for steps 1..4: two to three days; risk: low (the change is data-driven and verified by the
self-references above; the eleven re-bound missions should be re-run for 300 ticks to see that no new trap
appears).

## 7. Provenance

Observation only, 2026-09-05, on the GOG English data: `harness/tools/probe/rhm_inventory.py`,
`opensherwood-tools rhm` / `rhp` / `scb`, `scb_elements.py`, `scb_semantics.py --natives / --pseudo`,
`scb_load_natives.py`, `rhm_profiles.py`, `scb_xref.py`, the harness `Engine.reset` on the release binary, two
scratch probes in the analyst scratchpad (`scot_place.py`: per-mission implied `K` under both `SCOT` placements,
from `scb_elements.py`'s self-references; `native_sets.py`: native ids per file and the ids unique to the two
Sherwood files), the `TUPO` / `FLIM` counts read from the nine `.rhp` files, and the manual's Sherwood pages
(paraphrased). No executable analysis. Designer names, labels, compiled-script identifiers and game text are
not reproduced; roles are described in our own words.

Build: GOG English, executable SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`.
Reproduction without the scratch probes: `crates/opensherwood-script/tests/gamedata.rs`
(`map_element_counts_match_the_known_prefixes`, `every_retail_mission_binds_with_the_player_slots_at_the_tail`)
recomputes the prefixes and the self-reference counts from the player's files; the two scratch probes were
one-off derivations of the same numbers and are not needed to check the claims. Tests that depend on this
document: those two, `test_script.py::test_first_mission_element_table_has_the_hero_at_its_tail`,
`test_script.py::test_sherwood_camp_and_outro_load_strictly_and_run` and the 39-mission strict run.
