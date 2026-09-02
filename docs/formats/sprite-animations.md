# Sprite animation layout (`.rhs` character profiles)

Status: block structure, direction order and action-id tagging **verified** on all 117
`DATA/Characters/*.rhs`; the meaning of ~25 action ids **verified by looking** at rendered strips
(Robin, Soldier A, Child); the rest of the ids are listed with their structure only. The per-frame
"advance" and the per-animation displacement are **inferred** (consistent across all files, not checked
against the running game). Container layout: `docs/formats/sprites.md`. Helper:
`crates/opensherwood-formats/src/anim_table.rs`. Tool: `harness/tools/probe/anim_sheet.py`.

## Blocks of 16 directions

A character sequence's animation list is a sequence of **blocks of 16 animations**, one per facing
direction. Every animation of a block has the same frame count, the same durations and the same
`unknown_0x0c`; the frame indices step by a constant per direction (43 for Robin's first blocks, 34 for
Soldier A: the bank stores one direction's frames of a whole animation group contiguously). All 100
humanoid/character profiles have an animation count that is a multiple of 16 (`RobinHood` 2272 = 142
blocks, `Soldier A00` 2048 = 128, `Child` 736 = 46); the exceptions are objects: `ACCESSORIES_Net` (8),
`TG_York_flag` (4 and 2), `TG_MerryManStaff` (17 sequences of 3), `TG_Target` (3).

### Direction order

Index within the block, screen space:

| index | faces | index | faces |
|---|---|---|---|
| 0 | up (seen from behind) | 8 | down (faces the viewer) |
| 2 | up-right | 10 | down-left |
| 4 | right (profile) | 12 | left (profile) |
| 6 | down-right | 14 | up-left |

Odd indices are the 22.5 degree steps in between. The order is **clockwise starting at screen-up**.
Relative to `opensherwood-core`'s `facing256` (0 = +x = screen-right, clockwise): `sprite index =
(round(facing256 / 16) + 4) mod 16`; the core's 8-way octant `o` maps to sprite index `2 o + 4`.

Verified two ways: (1) the `Soldier A00` sheet of animations 0..32 (`anim_sheet.py --first 0 --last 32
--scale 3`) shows the back of the helmet at 0, the right profile at 4, the shield face-on at 8, the left
profile at 12, and a smooth turn in between; (2) blocks with a non-zero displacement (below) give
direction 0 the vector `(0, -19)`, direction 4 `(40, 0)`, direction 8 `(0, 19)`, direction 12 `(-40, 0)`,
i.e. up, right, down, left with a 2:1 isometric ellipse (462 blocks agree; no exceptions).

## Action ids: `Animation::unknown_0x0c`

`unknown_0x0c` is constant inside every block (9282 blocks over 117 files, zero mixed blocks) and
identifies the **action** independent of the profile: the same id has the same frame count pattern,
the same duration pattern and the same visual meaning in every file that has it. Profiles differ in
*which* ids they contain and in their order, so an engine must look animations up by id, not by index.
An id occurs at most once per file except `180`, present twice in `Blip00.rhs` and `MerryManBow.rhs`.

### Fixed prefix (all 81 humanoid profiles, blocks 0..14)

| block | anims | id | frames | frame `advance` | seen (direction 4 strips) |
|---|---|---|---|---|---|
| 0 | 0..15 | 0 | 6 (ping-pong, durations 6,2,2,15,4,4) | 0 | stand idle, breathing |
| 1 | 16..31 | 1 | 6..10 | 0 | idle fidget: looks around, scratches, shifts weight |
| 2 | 32..47 | 2 | 2 (4/6) | 0 | two-frame transition, standing (Robin: weapon lifted) |
| 3 | 48..63 | 4 | 2 (4/6) | 0 | two-frame transition, standing |
| 4 | 64..79 | 3 | 6 or 12 | 0 | second idle (Soldier: same frames as id 0; Robin: 12-frame variant) |
| 5 | 80..95 | 5 | 2 | 2 | walk start |
| 6 | 96..111 | 8 | 2 | 0 | walk stop |
| 7 | 112..127 | 6 | 22 | 2..4 | **walk** cycle, upright |
| 8 | 128..143 | 7 | 12 | 3..5 | **run** cycle, upright, long strides |
| 9 | 144..159 | 50 | 8 (all durations 0) | 0 | run frames with zero duration (blend/turn table?) |
| 10 | 160..175 | 51 | 2 | 2,4 | run start |
| 11 | 176..191 | 12 | 6..7 | 7,6,5,4,3,2 | sprint stop: decelerating skid, straightens up |
| 12 | 192..207 | 9 | 2 | 2,4 | sprint start |
| 13 | 208..223 | 11 | 3..6 | 5,4,3 | run stop (decelerating) |
| 14 | 224..239 | 10 | 32 (16 for Robin) | 5..7 | **sprint**: fast run, body leaning forward |

Robin's walk/run/sprint advances are 4/5/7, Soldier A's 2/3/5, so the hero moves faster (inferred).

### Per family

Grouping the files by their full id sequence (`families.py`, scratch script) gives:

- **Civilians** (15 sprite files: townsfolk, notables, the child, the priest, the tax collector, the wedding variant): 46 blocks. Prefix, then `34 36 35 37 39 38 40 41 47 44 48 45 49`
  (fall / lying / get-up set), `118 119 120 121 127 133` (kneel-and-cower set: kneel down, kneeling, one
  frame lying), `250 178 179 215 206 219 158 160 159 268 269 270`. Seen in `Child`: block 22 (id 41)
  falls flat, 23 (47) lies still, 27 (49) gets up, 32 (127) is curled up on the ground, 37 punches.
- **Soldiers** (Soldier A/B, Guard A/B, Archer, Crossbowman, Officer, Officier B, Guisbourne, Longchamp,
  Scatlock, Sherif, Trainer, MerryManBow, MerryManStaff, Blip00: 124..155 blocks). Prefix, then
  `34 36 35 37 39 38 40 41 47 44 48 45 49` at blocks 15..27, the melee set `52 53 54 96 55 56 57 58 59
  .. 75` at blocks 28..52, `100 152 153 154 76 78 77 79 101 .. 110`, then family-specific ids (archers:
  bow/crossbow ids 140..151 etc.).
- **Knights** (3 files, 87 blocks): prefix, then straight to `40 41 47 44 48 45 49 52 ..` (no 34..39).
- **Heroes** (RobinHood, RobinTown, WillScarlet, LittleJohn, Stuteley, Friar Tuck, LadyMarian,
  MerryManA/B/C: 101..154 blocks): prefix, then the **crouch set** `13 18 14 15 17 16` at blocks 15..20
  and `81 82 83 84` at 21..24; climbers (Robin, RobinTown, WillScarlet, Stuteley, MerryManB) add the
  **climb set** `19 22 20 24 23 21`; jumpers (Robin, RobinTown, WillScarlet, MerryManA) add `25 27 26 28
  30 29 31 33 32`; then the fall set `34 ..`, `42 43 46`, the melee set `52 ..` with extras `97 98 99`.
- **Objects**: `BONUS_*` 5 blocks `190..194`; relics, coat, clover 1 block `190`; `ACCESSORIES_{Ale,
  Apple, Coin, MoneyBag, Stone, Wasp}` 3 blocks `195 196 197`; `ACCESSORIES_Arrow` 1 block `95`;
  `Mendicant` `0 264 265 266 267`; `Longchamp Dead` starts at `47 45 106 ...` (a corpse: no idle).

### Ids identified by eye

| id | frames | advance | seen |
|---|---|---|---|
| 13 | 4..5 | 0 | crouch down from standing (heroes) |
| 14 | 6 (ping-pong 6,2,2,15,4,4) | 0 | crouched idle (heroes) |
| 15 / 17 | 2 | 0 | crouch transitions |
| 16 | 12..14 | 2..3 | **sneak**: crouched walk (heroes) |
| 18 | 4..5 | 0 | stand up from the crouch |
| 81..84 | 5..11 | mixed | crouch <-> run/stand transitions (83 = skid to a crouch) |
| 19 | 2 | 0 | pre-climb pose, displacement radius 10 |
| 22 | 9 | 0 | hangs on a ledge and pulls up |
| 20 | 12 | +3 | **climb up** a wall/ladder, arms reaching; displacement radius 45 |
| 21 | 12 | -3 | **climb down**; displacement radius 45 |
| 24 | 9 | -10 on some frames | drops off a ledge |
| 23 | 3 | 0 | landing, displacement |
| 25..33 | 1..9 | 0..15 | jump / drop set: crouch, airborne single frames (26, 29), landings (27, 28, 33) |
| 34 | 2 | 0 | pre-dive, displacement radius 10 |
| 37 | 9 | 0 (-10) | dives forward into a tucked pose |
| 35 / 38 | 12 | +3 / -3 | guarded walk forward / backward: fight stance with the weapon level (Robin), crouched behind the raised shield (soldiers); displacement radius 40..45 (corrected 2026-09-03; an earlier reading said "tucked pose") |
| 36 | 8 | 0 (10) | comes out of the guarded pose |
| 40 | 10 | 0 | **draw weapon** (Soldier: sword from the scabbard; Robin: readies the staff) |
| 41 | 8 | 6 | **knocked down forward**: hit, staggers, falls flat (ends face down; +36 px along the facing) |
| 44 | 7 | -7 | **knocked down backward**: collapses onto the back (-30 px) |
| 47, 45, 48 | 1 | 0 | **lying** poses (48 has a displacement) |
| 49 | 8..10 | 0 | **get up** from the ground |
| 42, 43, 46 | 11, 7, 7 | 0 | stance shuffle, crouch-to-lying, lying-to-fall (heroes) |
| 52 | 3..8 | 0 | first melee move: staff/sword swing from the idle stance |
| 53 | 3..8 | 0 | lowers the weapon back to the idle stance |
| 54 | 6 (3,3,12,4,4) | 0 | **fight idle**: stance with the weapon held level |
| 55 / 56, 57 / 58 | 5..6 | +3 / -3 | stance steps forward / back (two pairs) |
| 59..67 | 6 | 0 | individual strikes (thrust, overhead, sweep, ...) |
| 68 | 5..8 | 0 | weapon planted vertically (guard / parry) |
| 69..75 | 6..16 | 0 | further strikes and combos |
| 118..121 | 5,1,3,1 | 0 | kneel down, kneeling, rise, kneeling variant (civilians and soldiers) |
| 127 | 4 | -5 | curled up on the ground (cower) |
| 190..197 | 1..3 | 0 | bonus/accessory item frames (ids per item family; 48 one-frame coin anims = 3 blocks x 16 directions) |

Corrections from the 2026-09-03 pass (below): 35 / 38 are the guarded walk forward / backward (fight
stance; soldiers crouched behind the raised shield), not a slid tucked pose; 41 falls *forward* and 44
*backward*; 40 is a flinch (hit while carrying the weapon) as much as a draw.

### Combat, state and stealth ids (2026-09-03, by eye on `RobinHood`, `Soldier A00`, `ManCivilianPoor`, `Longchamp Dead`)

Tool: `harness/tools/probe/anim_actions.py` (`--table`, `--matrix`, `--families`, `--sheet`). The
behavioural reading of these ids (which state plays which) is in
`docs/original/stealth-and-combat.md`; this table records what the strips show. Frames / ticks /
advance are direction 4 of `Soldier A00` unless a hero value differs.

| id | frames | ticks | advance | seen | family presence |
|---|---|---|---|---|---|
| 40 | 10 | 20 | 0 | flinch, weapon swings up / sword drawn | all humanoids |
| 41 / 44 | 8 / 7 | 13 / 10 | +36 / -30 | knocked down forward (ends face down) / backward (ends on the back) | all humanoids |
| 47 / 48 / 45 | 1 | 1 | disp 0 / -24 / -24 | lying face down / on the back, shield on the chest / on the back, arms out | all humanoids |
| 49 | 8-10 | 15-16 | 0 | rolls over and stands up | all humanoids |
| 101 / 102 / 103 / 104 | 9 / 4 / 4-6 / 8-9 | 23 / 4 / 13 / 17-20 | 0 | stance: steps back shield up / short flinch / shield or weapon held up (block) / stumbles | heroes, soldiers, knights |
| 105 / 107, 106 / 108 / 109, 110 | as 41 / 44, 47 / 48 / 45, 49 | | | the same fall / lie / get-up set with the weapon out (fighting stance) | heroes, soldiers, knights |
| 111 .. 117 | 10 / 8 / 1 / 7 / 1 / 1 / 10 | 20 / 14 / 1 / 9 / 1 / 1 / 16 | 0 / +30 / 0 / -30 / 0 / 0 / 0 | the same set with the bow in hand | the four hero archers, swordsmen, archers, crossbowmen, merry men, trainer |
| 85 / 86 | 10 / 8 | 10 / 8 | 0 | bow off the shoulder / slung back | same |
| 87, 88, 89, 92 | 7, 5, 1, 1 | 7, 4, 0, 0 | 0 | nock and draw, aim, aim hold, aim hold | same |
| 90 / 91, 93 / 94 | 3 / 3, 4 / 4 | 1 | 0 | release (two variants each) | same |
| 118 / 119 / 120 / 121 | 5 / 1 / 3 / 1 | 8 / 1 / 6 / 1 | 0 | body lifted head-down / hangs over a shoulder / set down / stands limp | heroes, soldiers, civilians, the corpse profile |
| 122 | 14 | 32 | 0 | kneels, works on the ground with both hands | Robin, the lady, soldiers |
| 123 | 8 | 11-12 | disp 30-35 | knock-out blow: kick (Robin) / free-arm swing (soldier) | Robin, the big man, all soldier kinds |
| 124 / 125 / 126 | 6 / 7 / 11 | 12 / 15 / 18 | disp 20-22 | hands over / tosses underarm / bends and picks up | 124 Robin; 125 the six heroes who pay beggars; 126 all heroes |
| 127 | 4 | 4 | -20 | curled up, arms over the head | all humanoids |
| 128 / 129 | 6 / 7 | 14 / 13 | disp -28 | steps up onto a helper standing behind | Robin, Will, merry man A |
| 133 | 4 | 13 | 0 | on the back, limbs kicking (netted) | heroes, soldiers, civilians |
| 136 / 137 | 1 / 5 | 1 / 3 | 0 | heap of leaves / rises out of it | heroes |
| 140 | 6 | 33 | disp 30 | alert idle, weapon ready | soldiers, knights |
| 141 | 5 | 6 | 0 | straightens, hand to the helmet, peers | soldiers, knights |
| 142 | 8 | 11 | 0 | hand to the mouth, arm thrown up (alarm) | soldiers, knights |
| 143, 151 | 22, 12 | 0 | 3, 4 per frame | alert walk, alert run (twins of 6, 7 with the weapon ready) | soldiers, knights |
| 144 / 146, 145 / 149, 147 / 148, 150 | 2, 2, 3 / 7, 8 | | | alert starts, stops, turn table (twins of 5 / 8, 9 / 51, 11 / 12, 50) | soldiers, knights |
| 156 | 10 | 12 | +26, disp 35 | charging run into a strike | soldiers, knights |
| 158 / 159 / 160 | 5 / 1 / 5 | 5 / 1 / 7 | disp -14 | bends down / bent / straightens | soldiers, civilians |
| 164 | 12 | 12 | 0 | shield over the head, turning | soldiers |
| 165 | 10 | 26 | 0 | drinks (frame 8 held 15 ticks) | soldiers |
| 166 | 6 | 13 | 0 | bites and throws away | soldiers, knights |
| 169 | 8 | 17 | disp 28 | bends forward and reaches 28 px ahead | soldiers |
| 170 / 171 / 172 / 173 | 4 / 1 / 5 / 4 | 4 / 1 / 4 / 4 | disp 25 | raises the sword arm, holds, lowers | swordsmen, merry men, trainer, Will, merry man A |
| 178 / 179 | 5 / 3 | 8 / 6 | 0 | flung backwards through the air / topples forward | heroes, soldiers, civilians, the corpse profile (not the knights) |
| 189 | 8 | 9 | 0 | looks around, hands on the hips | soldiers, knights |
| 202 .. 205 | 5 each | 8 | 0 | four near-identical hand-to-ear glances | soldiers, knights |
| 206 | 10 | 16 | 0 | points with the outstretched arm (held 10 ticks) | soldiers, civilians |
| 207 / 208 / 209 | 4 / 4 / 6 | 3 / 3 / 34 | 0 | hands up / crouched shield up / crouched guard idle | soldiers |
| 215 | 5 | 0 | 0 | turn table (five zero-tick poses) | all humanoids |
| 216 | 1 | 0 | 0 | standing straight | soldiers |
| 219 | 1 | 1 | 0 | laid out on the back, arms folded, weapon on the body | soldiers, knights, civilians, the corpse profile |
| 227 .. 233 | 2 / 12 / 8 / 9 / 12 / 2 / 4 | | +36 / -36 on 228 / 231 | a second copy of the guarded walk set 34..39 | soldiers |
| 250 | 6 | 10 | 0 | hands over the face | civilians, officers |
| 268 / 269, 270 | 2 / 2, 44 | 2 / 2, 123 | 0 | two turn steps, a long idle routine (walk steps, idle, fidget, idle) | soldiers, civilians |

Hero-only sets not yet read: the big man's 180-189 / 199-201 / 249 / 254 (carrying walks of 22 frames
among them), the red-clad swordsman's 220 / 244 / 245 / 255 / 256, the friar's 238-243, the craftsman's
168 / 242 / 218 / 239 / 246 / 247 / 271, the lady's 251-253 / 217, merry man C's 184-188. Officers' and
antagonists' sets equal the halberdier set.

Everything else has not been looked at; `anim_actions.py --table` lists frame counts and advances for
every id.

## Per-frame timing word

The frame reference field parsed as `duration: u32` splits into two 16-bit halves: **low** = display
ticks (1..15 on idle and static frames); **high** = a signed value, `0` for idle and static blocks,
`2..4` on walk frames, `3..5` run, `5..7` sprint, `7,6,5,4,3,2` on the decelerating stops, `+3` on
climb-up and `-3` on climb-down frames, `+6` / `-7` on the two falls, `-9`, `-10`, `10`, `15` on jumps.
On every walk, run and sprint frame the tick half is **0** and only the high half is set (Robin walk
`0x00040000` = ticks 0, advance 4; Soldier A walk `0x00020000`), while idle frames have ticks and a
zero high half. Inferred: the high half is the **movement along the facing during the frame** (screen
pixels). Two readings of the zero tick half on moving frames (2026-09-03, `docs/original/stealth-and-combat.md`
4.2): (A) one frame per tick and the advance is the speed in pixels per tick (hero walk 4, run 5, sprint
7; NPC walk 2, run 3, sprint 5; the soldiers' alert walk 3 and alert run 4; the run-stops decelerate
7 6 5 4 3 2 with their last frames held one tick; the crouched walk 16 has both halves set, 27 px over 18
ticks); (B) the cycles are distance-timed (the frame ends after `advance` pixels) and the speed lives
elsewhere - but no profile field reads as a speed. A is the working hypothesis; the oracle plan in that
document measures it. Not verified in the engine. Values such as `131072` (`0x20000`) in the
`sprites.md` examples are this word. `anim_table::split_duration` separates the halves.

## Per-animation displacement: `unknown_0x04` / `unknown_0x08`

Per animation, `(unknown_0x04 - origin_x, unknown_0x08 - origin_y)` is `(0, 0)` for 8,300 of 9,282
blocks. Where it is not, the 16 directions of the block trace an ellipse of ratio 2:1 (e.g. radius
40 x 19, 45 x 22, 30 x 14, 10 x 4) starting straight up at direction 0 and going clockwise. It is set on
climbs, jumps/drops, the slid tucked pose, the stance steps and strikes of the melee set (55..79), a few
one-frame lying poses, and also on Soldier A's idle blocks 0 and 4 (radius 30 x 14, unexplained). For
most ids the vector points *along* the facing (direction 0 -> negative y); for ids 30 (jump landing), 45
and 48 (lying poses, 78 files), 108, 115, 128, 129, 135, 158..160, 170..177 and 268..270 it points
*against* it (direction 0 -> positive y), e.g. a fallen body extending behind the feet point. Inferred:
the position change applied when the animation completes (climb/jump destination) or the offset of
the pose from the entity position. Not verified in the engine.

## Sequence and frame placement

Frame top-left in screen space = entity position + `(anchor_x - origin_x, anchor_y - origin_y)` with the
sequence `origin` (150, 150) for characters (Robin's feet sit at the origin; the 6 x 5 coin covers
x -3..3, y -2..3 around it). The sequence `width`/`height` (90 x 108 for Robin) is smaller than the union
of all placed frames (150 x 155); what it bounds is unknown.

## Unknowns

- What blocks with all-zero durations (id 50) and the two-frame transition ids (2, 4, 5, 8, 9, 51, 15,
  17, 34, 39) are used for; probably blend/turn tables.
- Whether `run` (7) or `sprint` (10) is the double-click "run" of the game and which the AI uses. The engine
  plays 7 for the double-click run and 14 / 16 for a crouched character (hypothesis, `docs/harness.md`
  "Orders and movement modes").
- `Animation::unknown_0x02` = `frames - 1` in 112,608 of 148,512 animations; the others are smaller
  (loop start? last key frame?).
- The meaning of the displacement on Soldier A's idle blocks, and of ids not listed above.
- `FrameRef::unknown_0x0c` (non-zero on carts only) is untouched.

## Provenance

Observation only, no executable analysis. All scripts run with `OPENSHERWOOD_GAME_DIR` set on the GOG
data; no image was committed.

- `harness/tools/probe/anim_actions.py` (2026-09-03): `--table` / `--matrix` / `--families` print the
  per-block action tables (id, frames, tick and advance halves, displacement) and the family grouping
  quoted above from the files themselves; `--sheet` renders selected ids of one profile (viewed for
  `RobinHood`, `Soldier A00`, `ManCivilianPoor`, `Longchamp Dead`, `LittleJohn` in a scratch directory).
- `harness/tools/probe/anim_sheet.py`: `--first/--last` renders first frames in a 16-column grid (direction
  sheets); `--blocks A:B --dir 4` renders every frame of one direction per block (action strips);
  `--strip N` renders one animation. Sheets viewed: RobinHood blocks 0..89, Soldier A00 blocks 0..25
  and animations 0..32 at 3x, Child blocks 0..46.
- Scratch scripts (not in the repo): a grouping of consecutive animations by (frame count, unknowns)
  showing the 16-blocks; a statistics pass over all 117 files (block uniformity of `unknown_0x0c`, id
  -> slot/frame-count/advance histograms, the direction vector histogram quoted above, the anchor
  extents); a clustering of files by their id sequence (the families above).
- `cargo test -p opensherwood-formats anim_table` with `OPENSHERWOOD_GAME_DIR` checks the fixed prefix,
  the block counts, the walk-frame advances and the displacement ellipse on Robin, Soldier A00, Child.
