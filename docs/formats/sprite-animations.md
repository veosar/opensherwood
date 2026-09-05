# Sprite animation layout (`.rhs` character profiles)

Status: block structure, direction order and action-id tagging **verified** on all 117
`DATA/Characters/*.rhs`; the roles of about a hundred action ids **verified by looking** at rendered
strips (Robin, Soldier A, a poor civilian, the child, the corpse profile), a few dozen more named from
their table pattern (twins and second copies of seen ids), the remaining ids only by their family. The
per-frame "advance" and the per-animation displacement are **inferred** (consistent
across all files, not checked against the running game). This document records ids, roles, presence
and the rules for reading timing and displacement; it does not reproduce the per-block tables (see
"Reading rules" and Provenance). Container layout: `docs/formats/sprites.md`. Helper:
`crates/opensherwood-formats/src/anim_table.rs`. Tools: `harness/tools/probe/anim_sheet.py`,
`anim_actions.py`.

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

The ids are structural identifiers the engine must use (the scripts compare `ActionChange` parameters
with them, `docs/original/stealth-and-combat.md` 2.5), so they are listed here with their roles. The
per-block numbers (frame count, tick and advance halves, displacement of every block of every file)
are *not* reproduced: the engine derives them from the player's files with the rules below, and the
generated tables stay in the analyst workspace (ADR-0003; see Provenance).

### Reading rules (what the engine derives at run time)

1. **Block of an action.** Walk the sequence's animation list in steps of 16; the block whose
   animations carry `unknown_0x0c == id` is the action, and the animation for a facing is `block start
   + sprite index` (direction order above). A profile without the id has no such block; the engine
   substitutes a documented fallback (`crates/opensherwood-core/src/anim.rs`) and never invents one.
2. **Duration** of an action = the sum over its frames of the low half of the timing word ("Per-frame
   timing word"), in ticks of that word. A frame whose low half is 0 inside an otherwise timed
   animation is held one tick minimum (`hypothesis`). The moving cycles (walk, run, sprint and their
   alert twins) have a zero low half on every frame and are paced by the advance instead (reading A
   below).
3. **Advance** of an action = the sum of the signed high halves, in screen pixels along the facing
   (positive = forward). **Displacement** of a block = `(unknown_0x04 - origin_x, unknown_0x08 -
   origin_y)` of the animation, a 2:1 ellipse over the 16 directions ("Per-animation displacement"):
   the far point of the action (a climb's destination, the punch victim's spot, a lying body's extent).
4. **Presence** is a property of the sprite file, not of the mission actor: the `.rhs` the profile
   table names decides which ids exist (matrix below).

### Fixed prefix (all 81 humanoid profiles, blocks 0..14)

Every humanoid profile starts with the same fifteen ids in the same order (`observed`; the
`anim_table` test checks it on three profiles):

| block | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| id | 0 | 1 | 2 | 4 | 3 | 5 | 8 | 6 | 7 | 50 | 51 | 12 | 9 | 11 | 10 |
| role | idle | fidget | transition | transition | idle 2 | walk start | walk stop | **walk** | **run** | turn table | run start | sprint stop | sprint start | run stop | **sprint** |

Illustrative values (direction 4): the idle 0 is six frames played ping-pong over 33..35 ticks; the
walk 6 has 22 frames, the run 7 twelve, the sprint 10 sixteen (Robin) or 32 (soldiers); the frames of
these cycles carry no ticks and an advance of 4 / 5 / 7 px for the hero against 2 / 3 / 5 for every NPC
(the soldiers' alert twins 143 / 151: 3 / 4), so the hero moves faster (`inferred`); the run-stops 11 /
12 decelerate (advances from 7 down to 2, the last frames held one tick); the two-frame transitions
and the zero-duration id 50 are probably blend / turn tables (unknown).

### Presence per family

Grouping the 117 files by their id sequence (`anim_actions.py --families`) gives five families; inside
a family the block *order* also agrees, but the engine must not rely on it (rule 1). Files:
**civilians** 15 (townsfolk, notables, the child, the priest, the tax collector, the wedding variant;
46 blocks); **soldiers** (the seven soldier kinds, officers, the antagonists, the trainer, the two
merry-man recruits, the silhouette profile; 124..155 blocks); **knights** 3 (87 blocks); **heroes** 10
(101..154 blocks); **objects** (bonus items: five blocks of ids 190..194; relics, coat and clover: one
block 190; the thrown accessories: three blocks 195..197; the arrow: one block 95; the mendicant `0 264
265 266 267`; the corpse profile starts at 47 with no idle).

| set | ids | heroes | soldiers | knights | civilians | corpse |
|---|---|---|---|---|---|---|
| fixed prefix | 0..12, 50, 51 | yes | yes | yes | yes | no |
| crouch | 13 14 15 16 17 18, 81..84 | yes | no | no | no | no |
| climb | 19..24 | five heroes | no | no | no | no |
| jump / drop | 25..33 | four heroes | no | no | no | no |
| guarded walk (fight stance) | 34..39 | yes | yes (a second copy 227..233) | no | yes | no |
| hit / fall / lie / get up, weapon carried | 40 41 44 45 47 48 49 | yes | yes | yes | yes | 47 45 |
| stance shuffle, crouch-to-lying | 42 43 46 | yes | no | no | no | no |
| melee | 52..79 | yes (+97 98 99) | yes (+100 152 153 154) | yes | no | no |
| hit / fall / lie / get up, fighting stance | 101..110 | yes | yes | yes | no | 106 109 |
| bow | 85..94 | the four archers of the manual | swordsmen, archers, crossbowmen, merry men, trainer (not halberdiers, lancers, officers, antagonists) | no | no | no |
| hit / fall / lie / get up, bow in hand | 111..117 | the same four | the same kinds | no | no | 113 116 |
| carried body | 118..121 | yes (the big man: 120 121 only) | yes | no | yes | yes |
| search, pay, pick up, leg up | 122 282; 124; 125; 126 248; 128 129 | per hero (below) | 122 | no | no | no |
| knock-out blow | 123 | Robin and the big man only | all soldier kinds, officers, antagonists | no | no | no |
| cower / netted / laid out | 127; 133; 219 | 127 133 | 127 133 219 | 127 219 | 127 133 219 | 219 |
| hidden in leaves | 136 137 | yes | no | no | no | no |
| alert set | 140..151, 156 | no | yes | yes | no | no |
| stimuli reactions | 164 165 166 169 | no | yes | 166 | no | no |
| bend / pick up, panic | 158 159 160; 250 | no | 158..160 (250 officers) | no | yes | no |
| flung | 178 179 | yes | yes | no | yes | yes |
| signal, glances, point, idle routine | 170..173; 189; 202..205; 206; 268..270 | 170..173 (two heroes) | yes | 189, 202..205 | 206, 268..270 | no |

Hero-only sets not yet read: the big man's 180..189 / 199..201 / 249 / 254 (carrying walks among
them), the red-clad swordsman's 220 / 244 / 245 / 255 / 256, the friar's 238..243, the craftsman's
168 / 242 / 218 / 239 / 246 / 247 / 271, the lady's 251..253 / 217, merry man C's 184..188. Officers'
and antagonists' sets equal the halberdier set.

### Ids identified by eye

Roles of the locomotion, crouch, climb, jump, fall and melee ids (direction 4 strips of `RobinHood`,
`Soldier A00`, `Child`; `observed` unless marked). Frame counts and timings are read from the files
(rules 2 and 3); a few are quoted under "Illustrative durations and displacements".

| id | role |
|---|---|
| 13 / 18 | crouch down from standing / stand up from the crouch (heroes) |
| 14 | crouched idle (six frames ping-pong, like 0) |
| 15 / 17 | crouch transitions (two frames) |
| 16 | **sneak**: the crouched walk (heroes; the only crouched cycle) |
| 81..84 | crouch <-> run / stand transitions (83 = skid to a crouch) |
| 19 / 22 / 23 | pre-climb pose / hangs on a ledge and pulls up / landing |
| 20 / 21 | **climb up** / **climb down** a wall or ladder (advance +-3 per frame, block displacement radius 45) |
| 24 | drops off a ledge |
| 25..33 | jump / drop set: crouch, airborne single frames (26, 29), landings (27, 28, 33) |
| 34 / 36 / 37 / 39 | transitions into and out of the guarded walk (34 pre-dive, 37 dives forward into a tucked pose, 36 comes out of the guarded pose) |
| 35 / 38 | **guarded walk** forward / backward: fight stance with the weapon level (Robin), crouched behind the raised shield (soldiers); corrected 2026-09-03 from an earlier "tucked pose" reading |
| 40 | **draw weapon / flinch** (Soldier: sword from the scabbard; Robin: readies the staff; the same animation plays when hit while carrying the weapon) |
| 41 / 44 | **knocked down forward** (hit from behind, ends face down) / **backward** (hit from the front, ends on the back) |
| 47 / 48 / 45 | **lying** poses: face down / on the back, shield on the chest / on the back, arms out (one frame each; 48 and 45 carry a displacement) |
| 49 | **get up** from the ground (rolls over, pushes up, stands) |
| 42 / 43 / 46 | stance shuffle, crouch-to-lying, lying-to-fall (heroes) |
| 52 / 53 | enter / leave the fighting stance (weapon from carried to level and back) |
| 54 / 96 | **fight idle**: stance with the weapon held level, breathing / shifting |
| 55 / 56, 57 / 58 | stance steps forward / back (two pairs) |
| 59..67 | quick strikes (thrust, overhead, sweep, ...) |
| 68 | weapon planted vertically: **guard / parry** |
| 69..75 | further strikes and combos; 71..74 sweeping swings (the half-circle and circle attacks), 75 the over-the-head finishing blow |
| 118..121 | **carried body**: lifted by the hips until it hangs head-down / hangs over a shoulder / set down feet first / stands limp (corrected 2026-09-03 from an earlier "kneel" reading; civilians and soldiers have the set too, so bodies keep their own sprite while carried) |
| 127 | curled up on the ground, arms over the head (cower / duck; tumble for soldiers) |
| 190..197 | bonus / accessory item frames (one to three frames per item family; the coin's 48 one-frame animations are 3 blocks x 16 directions) |

### Combat, state and stealth ids (2026-09-03, by eye on `RobinHood`, `Soldier A00`, `ManCivilianPoor`, `Longchamp Dead`)

Tool: `harness/tools/probe/anim_actions.py --sheet`. The behavioural reading of these ids (which state
plays which) is in `docs/original/stealth-and-combat.md` 2.4 and 3.2; this table records what the
strips show. Presence per family is in the matrix above.

| id | role |
|---|---|
| 101 / 102 / 103 / 104 | fighting-stance reactions: steps back shield up / short flinch / shield or weapon held up (**block**) / stumbles back a step (**hit in the stance**) |
| 105 / 107, 106 / 108 / 109, 110 | the fall / lie / get-up set of 41 / 44, 47 / 48 / 45, 49 with the weapon out (fighting stance) |
| 111 .. 117 | the same set with the bow in hand (111 hit, 112 / 114 fall forward / backward, 113 / 115 / 116 lying, 117 get up) |
| 85 / 86 | bow off the shoulder / slung back (draw / put away the bow) |
| 87 | nocks and draws (ready the shot) |
| 88 / 89 / 92 | **aim**: bow drawn / hold frame (held until the click) / hold variant |
| 90 / 91, 93 / 94 | release (two variants each): **shoot** |
| 122 / 282 | kneels and works on the ground with both hands: **search** (a body, a chest) |
| 123 | **knock-out blow** (the fist icon): a high kick (Robin) or a free-arm swing (soldier); the block displacement, 30..35 px ahead, is the victim's spot |
| 124 / 125 / 126, 248 | hands something over (**throw purse**) / tosses underarm (**pay the beggar**) / bends and picks up, crouched reach (**pick up**) |
| 128 / 129 | steps up onto a helper standing 28 px behind (**be given a leg up**) |
| 133 | on the back, limbs kicking (netted / struggling) |
| 136 / 137 | a heap of leaves / rises out of it (**hidden under leaves**) |
| 140 | **alert idle**: crouched forward, weapon and shield up |
| 141 | **noticed something**: straightens, hand to the helmet, peers |
| 142 | **raises the alarm**: hand cupped to the mouth, then the arm thrown up |
| 143 / 151 | **alert walk / alert run**: twins of 6 / 7 with the weapon ready and a larger advance per frame |
| 144 / 146, 145 / 149, 147 / 148, 150 | alert starts, stops and turn table: twins of 5 / 8, 9 / 51, 11 / 12, 50 |
| 156 | charging run into a strike |
| 158 / 159 / 160 | bends down / stays bent / straightens (pick up / search; the displacement points behind) |
| 164 | shield raised over the head, turning (protects from arrows) |
| 165 / 166 / 169 | drinks (beer) / bites and throws the rest away (apple) / bends forward and reaches for the ground ahead (picks up the purse) |
| 170 / 171 / 172 / 173 | raises the sword arm, holds it up, lowers it (a signal to the company) |
| 178 / 179 | flung backwards through the air / topples forward (a trap, a boulder, dropped) |
| 189 | looks around, hands on the hips (a check) |
| 202 .. 205 | four near-identical hand-to-ear glances (listens) |
| 206 | points with the outstretched arm and holds it (scripts play it after an archer shoots) |
| 207 / 208 / 209 | hands up / crouched shield up / crouched guard idle (a second alert idle) |
| 215 | turn table: five zero-tick poses (every humanoid profile) |
| 216 | standing straight |
| 219 | laid out on the back, arms folded, weapon on the body (a body after being carried / tied; the corpse profile and the knights have it) |
| 227 .. 233 | a second copy of the guarded walk set 34..39 (soldiers) |
| 250 | hands over the face (civilian panic) |
| 268 / 269, 270 | two turn steps, a long idle routine (walk steps, idle, fidget, idle) |

Everything else has not been looked at; `anim_actions.py --table` lists frame counts, ticks, advances
and displacements for every id of a profile.

#### Illustrative durations and displacements

Unit: the low half of the timing word, summed over the frames (rule 2); one world tick per unit is the
working reading (A below), 25 per second the `scb.md` hypothesis. The values are those of `Soldier
A00`, direction 4, unless a hero value is given; the engine reads them from the profile and keeps only
the first five as fallback constants for a profile without the block (`crates/opensherwood-core/src/ai.rs`).

| id | ticks | id | ticks |
|---|---|---|---|
| 141 noticed | 6 | 142 alarm | 11 |
| 41 knocked down forward | 13 | 49 get up | 15..16 |
| 123 knock-out blow | 11..12 | 44 knocked down backward | 10 |
| 0 idle, 54 fight idle, 140 alert idle | 32..35 | 47 / 48 / 45 lying | 1 (held by the state machine) |
| 59..66 quick strikes | 8 | 75 finishing blow | 30 (Robin), 18 (soldier) |
| 88 aim | 4, then a 0-tick hold frame (89) | 16 sneak | 18 over 12..14 frames, both halves set |

Displacements: 41 advances +36 px and 44 -30 px along the facing (the body ends there); 48 and 45
lie 24 px *behind* the feet; 123's block displacement is 30..35 px ahead (the victim's spot); 128 /
129 point 28 px behind (the helper); 20 / 21 climb with a block radius of 45; 100 + 152..154 (shield
charge) 50 px ahead; 169 (purse) 28 px ahead.

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
blocks. Where it is not, the 16 directions of the block trace an ellipse of ratio 2:1 (radii from 10 x 4
to 45 x 22) starting straight up at direction 0 and going clockwise. It is set on climbs, jumps/drops,
the guarded walk, the stance steps and strikes of the melee set (55..79), the knock-out blow, a few
one-frame lying poses, and also on Soldier A's idle blocks 0 and 4 (radius 30 x 14, unexplained). For
most ids the vector points *along* the facing (direction 0 -> negative y); for the lying poses 45 and 48
(78 files), the leg-up 128 / 129, the bends 158..160 and a few others it points *against* it (direction
0 -> positive y), e.g. a fallen body extending behind the feet point. The engine reads the sign from the
vector itself; no per-id list is needed. Inferred: the position change applied when the animation
completes (climb/jump destination) or the offset of the pose from the entity position. Not verified in
the engine.

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
  per-block action tables (id, frames, tick and advance halves, displacement of every block) and the
  family grouping; `--sheet` renders selected ids of one profile (viewed for `RobinHood`, `Soldier A00`,
  `ManCivilianPoor`, `Longchamp Dead`, `LittleJohn` in a scratch directory). **The full per-block and
  per-family tables live only in the ignored analyst workspace** (`re/`, `harness/out/`) and are
  reproduced by running those probes on the player's files; this document keeps the ids, their roles,
  the presence matrix and a few illustrative values (ADR-0003: no lookup tables copied from assets;
  the quoted tables were removed after review 7, finding 8, on 2026-09-05).
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
