# AI and combat (behaviour specification)

Status: `draft` (awaiting Codex review). Build: GOG, executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`. Analyst session: 2026-09-13, analyst
(a Claude agent) under ADR-0009, with six parallel reading sub-sessions (profile fields, perception, alert
state machine, attacking tactics, rails / orders / natives, combat mechanics) whose notes stay in `re/notes/ai/`.

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, no identifiers
or strings of the binary, no tables copied from its data (ADR-0009). Image base 0x400000; every address is a
virtual address of that image. Every claim carries an id (`AI-nnn`), a status (`observed` = read directly in the
program and unambiguous; `inferred` = a reading consistent with everything read; `unknown`), the function
address(es) that support it and a confidence (high / medium / low). "Frame" means one logic frame of the
program's clock (section 2.1); "px" means map pixels; "profile" means a record of `Configuration/profile.cpf`
([../formats/profile.md](../formats/profile.md)).

## 0. Necessity record

Interoperability target: playing the game's own missions with their actors' data (`Configuration/profile.cpf`,
the `.rhm` actor and rail records, the `.rhs` animation tables) and their compiled scripts in a program of our
own. The rules that decide what a soldier perceives, when he raises the alarm, how a fight resolves, what the
patrol commands do and what the profile fields mean exist only in the executable: no data file or manual states
them, and eleven reviews found the hypothesis-built layer unfaithful. Section 8 lists every function read and
why; the reading stopped where the behaviour was settled and section 6 lists what is not.

## 1. Scope

Covered: the clock and the random stream the rules depend on; the meaning of every numeric profile field and of
the two weapon tables; perception (view cone, line of sight, hearing, identification); the AI event system, its
timers and the alert state machine of hostile, friendly and civilian non-player characters (patrol, curiosity,
search, reporting, attack decisions, fleeing, sleeping); the patrol program interpreter (every rail opcode);
the order / action-sequence system as far as the AI and the script natives use it; combat on both sides
(engagement, reach, strikes, defence, damage, stun and knock-out, energy, health, death, the bow and other
projectiles, thrown purses and wasp nests); difficulty; the script natives and callbacks that touch these.

Inputs from other subsystems: positions, layers and heights of actors (movement), the path graph (path
searches are requested, their algorithm is not described here), the sight-obstacle polygons and light elements
of the level (formats), the animation tables (a strike's hit frame and displacements come from them), the
script VM (natives and callbacks). Outputs: actor orders (walk, run, turn, play action, strike), the emoticons
and remarks, the HUD bars, events to the script VM.

Not covered: the pointer-gesture recogniser that turns a mouse stroke into one of the nine figure orders (not
found, section 6), the player's context actions on bodies beyond their preconditions (tie, carry, revive: the
effect functions were not read), the campaign-level selection of the hero's outfit record.

## 2. Data model

### 2.1 Clocks

- **AI-001** (observed, 004c6ef0, 0050f710; high). The simulation advances one *logic frame* per main-loop
  iteration and a global frame counter is incremented once per frame. The main loop waits until at least 40 ms
  have passed since the frame started (400 ms while the window is inactive), so the nominal rate is 25 frames
  per second. The program never raises the OS timer resolution; on the default 15.625 ms tick counter the wait
  ends after three ticks, so a frame lasts 46.875 ms in practice (21.33 per second). This is the "64 Hz clock,
  three clocks per frame" the oracle measured (walking frame 46.9 ms, idle step 93.75 ms = two frames). Our
  world tick is one logic frame; seconds below use the nominal 40 ms unless stated.
- **AI-002** (observed, 004c6ef0, 00404180; high). Every 25th frame the script's per-second callback runs with
  frame ÷ 25 as its argument: the scripts' "seconds" are 25 frames, and native 228's durations are frames.
- **AI-003** (observed, 00422c40, 00416710, 0048a980; high). AI timers are launched with a length in frames
  (0 is treated as 1) and expire when the frame counter reaches launch + length; one AI timer per actor, single
  shot, re-armed by each state; a second per-actor timer drives the patrol program's waits. While an actor's AI
  is locked by the script or the actor is "out", both deadlines are pushed forward one frame per frame: **timers
  freeze while locked**.
- **AI-004** (observed, 0048a980, 00471b00; high). Several per-actor periodic jobs are staggered by the actor's
  element id: the AI "think" hook runs when (frame − creation frame + 156) mod 16 = 0; the energy recovery when
  the low six bits of the frame equal the actor id's low six bits (once per 64 frames); the identification check
  when (id + frame) mod 16 = 0; the walk-noise hearing test when (id + frame) mod 3 = 0.

### 2.2 Random numbers

- **AI-005** (observed, 00642a7d; high). One global linear congruential stream (the C run-time generator:
  state × 214013 + 2531011, result bits 16..30, range 0..32767) serves every random decision of the AI, the
  actors, the sound and the effects. There are no separate streams.
- **AI-006** (observed, 0040a230, 004148b0; high). The stream is seeded from the wall clock at program start,
  and again at every save: the saving code stores the current wall-clock time in the save as the seed and reseeds
  with it; loading reads the stored seed and reseeds. A replay is reproducible from a save, not across a
  session. Consumption order within a frame follows the actor-list order (section 3.1) and, inside one actor,
  the order the rules below list their rolls in.
- **AI-007** (observed, 00419c50, 00419c80; high). Patrol programs use a *cached* roll: an integer 1..100 drawn
  once and kept until a program table consumes it, so the pre-check of the next point's table (section 3.4) and
  the selection at the point use the same number.

### 2.3 Sides, kinds and ranks

- **AI-010** (observed, 004a5d30 read as bytes, 0056c410; high). Every human has a *side*: hostile (the SD
  record's flag bit 0 set, or a civilian whose attitude word is 0) or friendly (player characters, soldiers with
  the bit clear, civilians with attitude 1). The engine keeps one actor list per side; an AI's "friends" are its
  side's list and its "enemies" the other side's.
- **AI-011** (observed, 0056aee0, 0056c490; high). Soldier *rank*: 0 soldier, 1 officer, 2 knight (the SD word
  at record offset +0x28 of the numeric block, see 2.4). Civilian *kind*: 0 man, 1 woman, 2 old man, 3 child, 4
  beggar, 5 notable.
- **AI-012** (observed, 0056c700, 0048fa30; high). A *character action* enumeration names abilities and HUD
  icons: 0 none, 1 bow, 2 punch, 3 hard punch, 4 purse, 5 sling stone, 6 shield, 7 big shield, 8 strangle,
  9 lever, 10 help to climb, 11 apple, 12 ale, 13 eat, 14 guzzle, 15 listen, 16 heal, 17 net, 18 beggar (pay),
  19 wasp nest, 20 whistle, 21 climb, 22 jump, 23 search, 24 resuscitate, 25 carry (strong), 26 carry (farmer),
  27 tie up, 28 lock-pick, 29 execute (kill on the ground), 30 test.

### 2.4 Profile records: what the numeric fields mean

Offsets are bytes from the start of the record's numeric block (profile.md's `unknown_pre` starts the block;
its `unknown_post` follows the voice code). Consumers are the functions that read the field.

**Soldier record (SD, 68 entries)** — AI-020 (observed unless marked; 0056a7d0 for the layout, consumers as
listed; high):

| Bytes | profile.md name | Meaning | Consumers / rule |
|---|---|---|---|
| 0..1 | `p0` | **Maximum hit points**; difficulty-scaled for hostile actors (2.11) | 004a1d00, 00435c80, 00432d30 |
| 2..3 | `p1` | **Initiative**: shortens reaction delays (3.5.1) and gates alarm decisions (> 29) and attack-choice bonuses (> 30, > 70); difficulty-scaled | 00438620, 0043a220, 00440cf0 |
| 4..5 | `p2` | **Courage**: retreat threshold and roll (3.7.2); weight 0.045 per ally in the "enough of us engaged" test; a battle cry only if > 39 | 00432d30, 004303e0, 0042ef80 |
| 6..7 | `p3` | **Delegation** (officers): > 49 "sends men rather than searches", < 50 "searches himself" | 00438cb0 queries 10..12, 0043ccc0 |
| 8..9 | `p4` | **Seniority**: adds `p4 + 100` per ally to the strength estimate; non-zero selects the long look-around interval on post; compared between two soldiers to decide who defers | 00432d30, 0041a420, 004303e0 |
| 10 | `pole` | pole weapon flag (also present in the class block head) | 00440470 |
| 11..12 | `q0` | **Ranged skill** (the roll of 3.12); difficulty-scaled (inferred: the initialisation was not seen, the consumer was) | 004a5ba0, 0047bb60 |
| 13..14 | `q1` | **Melee experience**: gates which figures the AI may use, many AI thresholds, and the damage factor of a rank-0 soldier's blows (3.9.3); difficulty-scaled | 00485240, 005c1dd0, 004a1cc0 |
| 15..16 | `q2` | **Energy recovery**: `q2 / 10` units per recovery period (3.10) | 00471b00 |
| 17..18 | `q3` | not consumed by any code read (rank lives elsewhere) | unknown |
| 19..20 | `q4` | not consumed | unknown |
| post 0..1 | `w0` | **Rating**: melee experience gained for a kill = 20 + max(0, victim rating − attacker rating) | 0047e7d0 |
| post 2 | `flags` | bit 0 hostile side; bits 1 and 2 mounted kinds; bit 3 principal enemy (immune, special selection colour); bit 4 **post-bound** (never leaves the post to investigate; halves the body-alarm delay); bit 5 unknown; **bits 6..7 are never written by the loader** (uninitialised memory in the file: ignore) | 004a5d30, 00438cb0, 0043ccc0 |
| post 3..10 | `s0..s3` | stimulus susceptibilities in file order **purse, apple, beer, whistle** (percent): purse / apple / whistle are tested as non-zero (whistle > 1 for the cover decision), beer enters the brawl-continuation formula (3.5.3), whistle enters the knight's search radius `(whistle − 2) × 400 / 98` px, apple ÷ 2 is a chase count | 00438cb0, 004468a0, 0044cb40, 00424d50 |
| post 11..12 | `class` | 1-based index of the melee class block (2.5) | 0047ab00 |
| post 15..16 | `ranged_kind` | 1-based index of the ranged weapon record (2.6), 0 none | 0047ab00 |
| post 38..39 | `t` | **stun decay period**: one stun point is removed every `t + 1` frames (64 regular kinds, 3 mounted, 10 antagonists) | 00471b00, 00471c00 |
| post 40..43 / 44..47 | `a` / `b` | weapon material (0 wood, 1 steel, 2 cast iron, 3 steel and wood) / armour material (0 leather, 1 chain mail, 2 plate): **sounds only**, never a combat modifier | 0056c290, 0056c360, 00472070 |

profile.md's readings "p4 = knock-out resistance", "q1/q2 = parry / counter chances", "s = stimulus
susceptibility" are corrected or confirmed accordingly; the knock-out resistance is in the class block (2.5).
Data check (all 68 records): `p3` non-zero only for officers, `p4` only for officers, knights, mounted knights,
the trainer and the antagonists, every stimulus word ≤ 100, every class within 1..27, every ranged kind within
0..4, bits 6..7 set on every record (confirming they are junk).

**Player-character record (PC, 10 entries)** — AI-021 (observed, 00565490, 0048fa30, 0055bf00; high):

| Bytes | profile.md name | Meaning | Consumers |
|---|---|---|---|
| 0..1 | first word (256 / 0) | **story hero** flag (byte at +5 = 1): looked up among the seven story names; the hero's death with a clover falls instead of dying | 0055bf00, 0049fa20 |
| 2..3 | `bow` | **bow skill** (the ranged roll) | 0047bb60 |
| 4..5 | `x` | **melee experience** (the PC twin of `q1` for thresholds; not a damage factor) | 0047ea20, 00485240 |
| 6..7 | `sword` | **energy recovery**: `value / 10` units per period (Robin 50 → 5) | 00471b00 |
| post 0..3 | id | melee class (1..10) | 0048fa30 |
| post 4..7 | | ranged kind (1..4, 0 none) | 0048fa30 |
| post 8..11, 20..21; 12..15, 22..23; 16..19, 24..25 | | the three HUD action icons (2.3 enumeration) with their starting stock; the stock is difficulty-adjusted at creation: 6 → 8 easy / 4 hard, 12 → 15 / 9 | 0048fa30, 0055bc90, 0049d5e0 |
| post 28..43 | | four context abilities (the same enumeration) | 0049d560, 0049d520 |
| post 44 | | a per-level portrait / colour selector | 0048fa30 |
| post 78..79 | | stun decay period for this character (as SD `t`) | 00471b00 |
| post 80..81 / 82..83 | the two trailing words | **conspicuousness** in the town outfit (+80) / forest outfit (+82): multiplies how fast an enemy's awareness of this character accumulates (3.2.5); the forest word is used when the mission's outfit byte is set (Robin 100 forest / 80 town, the red-clad swordsman 200 / 200) | 00488f60 |

The PC's maximum hit points are the constant 100 (0048f910, read as bytes). The team-order rank is computed at
load (10 − position), not a file value.

**Civilian record (CV, 24 entries)** — AI-022 (observed, 00564ea0, 0056c490, 0056c410; medium: the consumers
of the civilian record were not located beyond the loader): bytes 0..3 kind (2.3), bytes 4..7 attitude
(**1 friendly, 0 hostile**; the tax collector, the two rich townspeople and the unarmed friar are hostile).

### 2.5 The melee class table (profile.md "table A", 27 blocks)

- **AI-030** (observed, 00568e00, 0047ab00, 005c1b20, 005c1b50..005c1fb0; high). Block `class − 1` is bound
  to every human at creation. It is the **hand-to-hand weapon class**. Head (14 words): words 0..3 four
  distances in px — 0 the default minimum reach of the quick strikes, 1 the distance the AI closes to, 2 the
  default maximum reach and the "too far, close in" threshold, 3 the engagement distance (150 in every block);
  **words 4..8 the defence chance in percent by the attacker's approach** — 4 when the attacker stands ≥ 20 px
  higher, 5 from the front, 6 from the left side, 7 from behind, 8 from the right side (3.9.2); **word 9 the
  stun resistance** (percent); word 10 the **arrow deflection** chance (percent); word 11 low byte = pole
  weapon, high byte = shield bearer; words 12..13 shield geometry offsets (non-zero for shield classes only).
- **AI-031** (observed, same; high). Ten rows of 16 words, one per figure in the manual's order (0 quick jab,
  1 slow blow, 2 finishing blow, 3 attack left, 4 attack right, 5 half circle left, 6 half circle right, 7 circle
  left, 8 circle right, 9 the block). Per row: words 0..1 thrust target (always 2 = chest), **word 2 stun
  value**, **word 3 hit-point damage**, word 4 minimum reach, word 5 maximum reach (px), words 6..7 thrust
  kind (0 straight, 1 lateral, 2 push aside, 3 true half circle, 4 true circle, 5 false half circle, 6 false
  circle), words 8..9 side (0 left, 1 right, 2 not applicable), words 10..12 three angles in degrees (start
  offset, sweep, per-step rotation of the sweeping figures), word 13 a push / step distance, word 14 unused by
  the code read, **word 15 energy cost**. profile.md's "words 2/3 = damage / effect, 5 = hit chance, 6 =
  figure class" are corrected: 2 = stun, 3 = damage, 5 = maximum reach, 6 = thrust kind.
- **AI-032** (observed on the data; high). The hero's two outfits are two records with two different classes:
  the forest outfit's rows mostly stun and rarely damage (only the finishing blow and the sweeping figures
  carry damage); the **town outfit's rows never stun and always damage** (e.g. its slow blow 50, its
  finishing blow 100; the file holds the rest). The first mission uses the town outfit, which is why its
  forward stroke did 50 (3.9.3).

### 2.6 The ranged weapon table (profile.md "table B", 4 records)

- **AI-035** (observed, 00569f40, 00450040, 004500c0, 0047bb60; high). Record `ranged_kind − 1` (1 hero bow,
  2 outlaw bow, 3 soldier bow, 4 crossbow). Layout: u16 range 1 (px); a 6 × 3 grid A; u16 (100 / 75 / 10 / 20:
  the missile's damage, not consumed by the code read — inferred); … u16 range 2; grid B; a flag byte "use
  range 2 as the reach"; u16 (100 / 100 / 10 / 10, unconsumed). Grids: for each of six distance steps the hit
  chance at skill 0 / 50 / 100 (3.12.2). Table B is **not** the difficulty table.

### 2.7 Per-human combat state

AI-040 (observed, 00471b00, 00471c00, 0047c600, 0047dc00; high):

| Quantity | Type / range | Initial | Notes |
|---|---|---|---|
| hit points | u16 | maximum (2.4) | never regenerates except on easy (2.11); 0 = dead |
| fatigue | u16, 0..100 | 0 | the energy bar shows 100 − fatigue; a figure adds its energy cost; recovers per 3.10 |
| stun level | u16, 0..300 | 0 | ≥ 70 makes the actor unconscious, < 30 wakes him; decays one point per `t + 1` frames; floored at 30 while tied, carried or (a player character) in a coma |
| unconscious, tied, immune flags | bool | false | immune = principal enemy: ignores damage, stun and hit-point changes |
| adversary list | list | empty | several attackers per actor; the first entry is "the" adversary |
| initiative flag, hand-over chance | bool, u8 % | | the turn-taking of a duel (3.8.2) |
| melee experience | two (whole, hundredths) pairs | profile | raised by kills and good strikes, capped at 100 |
| melee class handle, ranged handle | pointers | from the profile | 2.5, 2.6 |

### 2.8 Per-NPC AI state

AI-041 (observed, 00414be0 serialisation order, 00424540, 00435c80; high). A top-level state (sleeping,
default, wondering, seeking, attacking, menacing, fleeing) and a sub-state; a target element and a rival; a
body element, an object element (purse / ale), the officer or caller element; a remembered position (x, y,
height, layer) with a **priority** (1 heard a noise, 2 a body to loot, 3 a body seen, 4 a missing colleague,
5 a sighting / arrow / look-there call: a lower priority never overwrites a higher one); a second, officer-given
position; a list of known elements; the company (chief pointer, member list, company number); the colleague to
check on (from the patrol command); a "seen clearly" flag; a search-flags word; the walk-to and return spots;
a flee counter; the frame of the last sighting; the walking-flags word (3.4.4); a hesitation byte (0 for
hostile soldiers, larger for civilian-class AIs); a drunkenness byte; the head marker (emoticon) with an
expiry; the patrol rail and the alert rail indices; the timer deadlines of 2.1.

### 2.9 Per-NPC perception state

AI-042 (observed, 004863f0, 00486fa0; high). A perception *mode* (0 blind — dead or collapsed; 1 normal;
2 glancing left; 3 glancing right; 4 wide; 5 collapsing; 6 tracking an element; 7 looking in a given 16th of
a turn; 8 recovering); the base range (px, from the level, 3.2.1); the current base range (ramps in modes 5
and 8); a range factor (1.0, never changed by the code read); the effective range and half-angle; the body
facing (16 directions); the head angle (radians, relative to the body), its limit and turn speed; four wobble
phases (drunkenness); six *candidate lists* ("categories" 0 awareness, 1 bodies, 2 objects, 3 soldiers, 4 the
player character, 5 beggars) of entries {element, reported flag, shadow-reported flag, last value, visible-now,
heard-now} with a per-category accumulator (u16); a deafness value (px) with its last update frame; a "needs
rescan" flag. Each player character carries a walk-noise record {position, height, facing, zone, kind, radius}.

### 2.10 Mission per-actor fields (`BORG`, rhm.md)

AI-043 (observed, 004a1e90 read as bytes, 0048b5d0, 00449170, 0044cae0; high): `unknown_0x1a` → the **patrol
chief** flag; `unknown_0x1b` → the **company number** (native 176 writes the same word); `unknown_0x23` → a
loot threshold read only by the purse-looting behaviour (inferred, low); `members` → the company member list;
`rail` → the patrol rail; `unknown_i16` → the **alert rail** (native 220 switches to it when it is not −1).

### 2.11 Difficulty

- **AI-045** (observed, 0055dbb0 (+0x1c of the settings), 0050b640, 00438710, 00438600; high). Difficulty is
  0 easy, 1 medium, 2 hard, chosen in the menu. A scaler applies only to **hostile-side** actors and is the
  identity on medium: hit points × 0.5 easy / × 1.5 hard (cap 10000); initiative, melee experience and ranged
  skill × 0.5 easy / × 2 hard (cap 100). The player's starting stocks: 6 → 8 easy / 4 hard, 12 → 15 / 9. On
  easy a player character below 100 hit points regains 1 every 100 frames while not fighting (004936f0);
  "resting" raises him to 75 at once then 1 per frame. The punch's stun amount is × 1.5 on hard. Arrows hit
  allies on hard only. The identification radius factor is × 1.3 easy / × 0.7 hard (3.2.7). The reaction-delay
  multiplier is 2 on easy and hard and effectively 0 on medium (3.5.1, an original defect).
- **AI-046** (observed, 00482150, 00471c00; medium). A "no player death" option byte in the options block,
  together with a launcher flag, prevents a player character's hit points from being set below their current
  value; its writer was not found (open).

## 3. Behaviour

### 3.1 Per-frame order (determinism)

- **AI-050** (observed, 004c6ef0, 0048a980, 00487d00; high). Each frame, for every non-player human in the
  engine's actor-list order (the mission's element order, unchanged by the AI): (1) the AI pre-tick hook;
  (2) the human base tick: stun decay, animation and movement, and — for player characters only — the refresh
  of the walk-noise record; (3) if the "needs rescan" flag is set, every other actor is entered in category 1
  (bodies) unless listed (within a 700 px max-norm box when the AI is restricted); (4) the cone geometry
  update (3.2.1); (5) the perception scan (3.2.5) — identification, walk-noise hearing, sight — with its
  sightings dispatched to the observer's own AI at the end of that observer's scan, in a stable order by list
  position; (6) the AI tick (the substate logic of 3.5..3.7) and then the deafness update; (7) outside the
  pause check, the "think" hook (every 16 frames, staggered), the timer event when the deadline is reached, and
  the deferred event queue. No random number is consumed by perception; the AI's rolls are consumed in the
  order the rules list them, one actor after another.

### 3.2 Perception

#### 3.2.1 The view cone

- **AI-060** (observed, 004c1ab0, 0048ebe0, 0057ae60; high). The **base range** is a level constant: 400 px
  for a level whose `FOOT` lighting word is 1, 8, 16, 32, 64 or 128 and 300 px for lighting 2 or 4; a script
  native can overwrite the global. Every NPC copies it at creation (the first mission: 400).
- **AI-061** (observed, 00486fa0; high). Every frame: if the body facing changed, the head angle is
  pre-compensated by −(change in 16ths) × 0.3927 rad so the head keeps pointing where it was (clamped to
  ±π/2). The target head angle by mode: normal and recovering → 0; glancing left → −π/4, right → +π/4 (these two
  revert to normal unless the actor plays one of the glance animations 202..205 or action 1); tracking → the
  direction to the tracked element (y compressed); given direction → that direction. The head turns toward the
  target at 0.3927 rad (22.5°) per frame, limited to ±0.8 rad from the body (±1.3 rad when tracking / given).
  **Half-angle 0.5 rad (28.6°)** in modes normal / glancing / recovering; 1.5208 rad in the wide mode (set while
  the actor's order state is 19). When tracking or looking in a given direction the effective range is × 1.4
  (truncated) and the effective half-angle 0.35 rad (a narrower, longer cone). One further virtual condition
  of the NPC (not identified) multiplies the range by 1.4 again.
- **AI-062** (observed, 00486fa0; high). Collapse (on losing consciousness): every frame the current base
  range shrinks by an increment growing by 5 per frame (5, 10, 15, …) until negative, then the mode is blind.
  Recovery (after waking or a reset): the current base range starts at 5 and grows by 8 per frame up to the base
  range, then normal.
- **AI-063** (observed, 00486fa0; high). Drunk wobble (drunkenness byte d ≠ 0): four phases advance by 0.1,
  0.07634, 0.12321 and 0.04546 rad per frame (mod 2π); range' = trunc(range × (1 − 0.01 d (0.1 (cos p0 + sin p1)
  + 0.6))); half-angle' = half-angle × (1 − 0.01 d (0.1 (sin p2 + cos p3) + 0.2)), capped at 0.95 rad. At
  d = 100 the range oscillates between 20 % and 60 %.
- **AI-064** (observed, 00486fa0, 0058e7f0, 00487b90; high). The cone's axis is the body facing rotated by the
  head angle; the two edges are the axis rotated by ± the effective half-angle; **the y component of every
  edge / distance vector is scaled by 0.5736** (its inverse 1.7434 is used in every distance test: the metric
  is an ellipse 400 wide and 229 tall for range 400). The Alt-key cone drawing uses this record, so the drawn
  cone is the perception cone.

Validation against the oracle (h01-measurements-2.md 6): the measured sector of ~80° on screen becomes, after
undoing the 0.5736 projection, a world sector of 59° (half-angle 29.5°) whose axis sits on the 16-direction
facing 67.5°: **agrees with 0.5 rad**. The measured reach (270 px along x, 196 along y, ratio 0.72) is 78..86 %
of the predicted 310 / 229 with the same ratio 0.74: the graded fade of 3.2.3 (the outer 30 % of the range at
≤ 25 % intensity) would lose its faint rim to the capture's tint threshold; the alternative (an effective range
near 320) has no support in the code. One capture with a lower threshold settles it (section 6).

#### 3.2.2 Line of sight

- **AI-065** (observed, 004eff80, 004eef30, 004f0230, 004ef340, 005a3810; high). Two points see each other
  unless (a) one is above ground level (height > 0) and the other below (< 0), or (b) the segment crosses a
  sight-blocking polygon below that polygon's surface. The polygons are the proto-level's projection areas
  (`.rhp`, [../formats/rhp.md](../formats/rhp.md)): planar polygons with vertex heights whose second flag byte
  means "blocks sight" (the first "solid"), bound to a layer whose motion-area flag bits 0 and 1 are set. The
  test intersects the 2-D segment with every edge of every candidate polygon in the spatial grid cells the
  segment's box covers; at a crossing the segment's interpolated height is compared with the plane's height
  there. Results are cached per frame in a 2000-slot table keyed by a hash of the two endpoints — **a hash
  collision within one frame returns the stale answer** (an original quirk an implementer may ignore or
  reproduce). The mission file's own obstacle chunk (empty in the retail data) and mobile elements (carts) can
  add obstacles.

#### 3.2.3 Graded visibility of one element

- **AI-066** (observed, 00489680, 00489eb0, 00489d00, 0048db80; high). Returns 0 when: the seen element's
  order state is 21..23 (hidden), the observer's mode is blind or collapsing, a global "players invisible"
  flag is set, or the element is inactive. An observer standing in a **no-sight zone** (bit 13 of the zone's
  flag word: interiors) sees only elements in the same zone that are alive and conscious, with the fixed value
  0.5 and no geometry. Otherwise, with v = target eye point − observer eye point and v.y × 1.7434:
  1. the squared elliptical distance must be ≤ range², and the target in the front half-plane (axis · v ≥ 0)
     or within 60 px (squared ≤ 3600);
  2. within 20 px (squared < 400): value 1.0;
  3. the **light-limited reach** L: with dz the height difference, L = √(range² − dz²) (0 if |dz| ≥ range).
     In levels of lighting 2 or 4 only: the light elements within a 400 px box are sampled at three points
     (observer + L·axis and the two edge points, each needing line of sight to the light); a light at squared
     distance s gives level 1.0 if s < 10, 0.5 if s > 110, else 1.05 − 0.005 s; the maximum over the lights is
     clamped to [0.5, 1]; f = 2·level − 1; L' = f × 400 + (1 − f) × L. So in dark levels the reach is the short
     base in darkness and 400 in full light. Cached per NPC per frame;
  4. squared distance ≤ L'² and the angular test: within 60 px the target's direction in 16ths relative to the
     body facing must be 0..4 or 12..15 (**the front 9/16 of the circle, ±101°, without the head angle**);
     beyond 60 px the target must lie between the two edge vectors; then line of sight (3.2.2);
  5. the graded value g: ≤ 60 px → 1.0; else with r = |v| / range: r < 0.3 → 1 − r/6; 0.3 ≤ r < 0.7 →
     0.95 − 1.75 (r − 0.3); r ≥ 0.7 → 0.25 − (r − 0.7)/3, clamped at 0 (r = 1 gives 0.15);
  6. the **posture factor**: order state 10 → × 0.5; else by the seen actor's posture code: 2, 8, 14 → × 3;
     3, 9 → × 20; 4, 5, 6 → × 2; 7, 10, 11 → × 1.5; others × 1. (Which code is running, sprinting, fighting or
     crouched was not mapped: section 6; "3/9 = fighting or sprinting, 2/8/14 = running" is the analyst's guess.)
- **AI-067** (observed, 00489680; medium). Friendly NPCs, when the mission's outfit byte is set, use a simpler
  in-range / in-front / line-of-sight test giving 1.0 or 0.

#### 3.2.4 Category values and staggering

- **AI-068** (observed, 00488f60, 00489b30; high). With k = element id + frame, a **hostile** observer
  evaluates: category 0 (awareness) — a soldier target every 16 frames, value 16 g; a player character or
  civilian every 2 frames, value W × 2 × g × 0.01 where W is the target's conspicuousness word (2.4; a
  target playing sprite action 242 gives 0; actions 246 / 247 set an AI flag whose meaning was not read);
  category 1 (bodies) — the target must be down (its state is one of the two "down" states) and the observer
  not in a no-sight zone, every 8 frames, 24 g; category 2 (objects) — 0 inside a no-sight zone, every 4
  frames, 4 g' with g' the omnidirectional variant (range and line of sight, no cone); category 3 (soldiers) —
  every 8 frames, 8 g, the target must be a valid target; categories 4 (the player character) and 5 (beggar) —
  target alive and conscious, every 8 frames, 8 g. Between evaluations the previous value is kept. Non-hostile
  observers evaluate only category 0 (16 g every 16 frames; dead targets 0). **Every value is truncated to an
  integer per entry**; "visible now" = integer ≠ 0. Consequence: categories 1..5 are effectively boolean, and
  in category 0 a standing player character with W = 80 (the town outfit) counts 1 per evaluation only while
  g ≥ 0.625 (about half the range) — running, fighting or a larger W makes him count from farther.

#### 3.2.5 Accumulation, the "?" and the sighting events

- **AI-069** (observed, 00487d00, 00488e00, 00488b80, 004887a0; high). Per category c and frame: S = the sum of
  the integer values of the entries not yet reported; acc[c] += S. When 100 ≤ acc[c] < 1000, an entry that is
  a player character and not yet engaged produces one **shadow** event (the "?" reaction, 3.5.2) with the
  target's position. The category **emits** when acc[c] ≥ 1000, or at once when the rule allows: for a hostile
  observer categories 3, 4 and 5 are always immediate, categories 0 and 1 are immediate once the AI's
  top-level state is beyond wondering (already alerted), category 2 once wondering or beyond; for friendly
  observers categories 0, 3, 4, 5 are immediate and 1 once alerted. On emission acc[c] is reset and each
  visible entry produces its event (0 → view, 1 → sees body, 2 → sees object, 3 → sees soldier, 4 → sees the
  player character, 5 → sees beggar) and is marked reported; a reported entry no longer visible produces
  out-of-view (category 0 only). While S = 0 and acc[c] > 0, acc[c] decreases by 1 every 20 frames (slow
  forgetting). Entries are dropped when their target is dead (category 0), inactive (2) or no longer valid (5).
- **AI-070** (observed, 0048ebe0, 0048c700, 0041a300, 0041a490, 0043c960, 0043a220, 0048c620; high). Candidate
  lists are filled explicitly, not by proximity: a hostile AI's initialisation puts every player character and
  enemy soldier into category 0; a friendly soldier's category 0 holds the hostile soldiers; a civilian's the
  player characters; category 1 is filled at rescans and when someone falls (an actor playing the "become a
  body" action is pushed into every actor's category 1); categories 2..5 are filled by AI decisions (objects
  when a purse / ale is thrown, soldiers when looking for an officer, **the player character into category 4
  when a check-for command or an area search starts**, beggars when the civilian's substate is a beggar one).
  So an unalerted soldier notices a player character through the slow accumulation of category 0 (a "?" then,
  much later or never, a sighting) unless a noise, a call or a search has put the player into the immediate
  category 4.

#### 3.2.6 Hearing

- **AI-071** (observed, 0048cca0, 0048cc20, 0048cec0; high). A one-shot noise has a kind, a position with
  height and a radius R by kind: 0 → 300 (a projectile landing), 1 and 7 → 70, 2 → 50 (a projectile landing on
  a lying actor), 4 and 12 → 500, 5 / 6 / 8 / 9 → 200 (9 = a purse landing; 8 = the shout of a netted actor every
  31 frames; 4 / 5 / 6 = a mobile element's gait changes), 10 and 11 → 400, 14 → 0; kinds 3 and 13 take the
  emitting actor's radius; a script native can emit any kind at a point. For every civilian or hostile actor:
  precheck max(|dx|, |dy|, |dz|) ≤ R, then margin = trunc(R − euclidean distance) − deafness; **heard iff
  margin > 0**; the hear event is posted at once with the noise record and the margin. (This is the console's
  "black circle entirely inside the white circle" rule: the black circle is the deafness.)
- **AI-072** (observed, 0047ef70, 00487d00, 00488a40; high). A player character's **walk noise** is refreshed
  every frame from his current sprite action and the ground material class m (0..10 under his feet): idle and
  fidget actions (0, 1, 3, 14), the pick-up actions (158..160, 292, 293) and the bow set (85..94) → 15 px;
  the walking group (2, 4..8, 13, 15..24 — walk 6, run 7, sneak 16, climb 20 / 21 — 34..39, 49, 50, 81, 82, 255,
  256, 295, 303) → 20 (m 0, 2, 7), 40 (m 1, 3, 6), 10 (m 4), **200 (m 5)**; the loud group (9..12 — run start,
  sprint, run stops — 51, 83, 84, 294, 296, 297, 304) → 70 (m 0), 150 (m 1, 3, 6), 75 (m 2, 7), 50 (m 4),
  **400 (m 5)**; action 30 → 80; actions 27, 33, 98, 127, 188 → 50 / 100 / 30 / 300 by material group (m 0, 2, 7,
  10 / 1, 3, 6 / 4 / 5); the melee actions 59..79, 96 and 100 → 200 regardless of material; everything else 0;
  0 inside a no-sight zone or when inactive. Every third frame (staggered by id) each NPC inside the noise's
  bounding box runs the hearing test above against every player character in its category 0; on the rising
  edge (not heard last time, heard now, not yet reported) it posts hear with the target's position and margin.
  Validation: the oracle heard no walk at 290 px (20..40 px: agrees); a run detected at ≥ 330 px fits only the
  material-5 radii (400 px for the run start / sprint group, 200 for the walk group) or a sighting by a soldier
  facing him: the courtyard's ground material decides (section 6).
- **AI-073** (observed, 0048a830, 005b3c40; high). Deafness decays per frame by 10 while < 301, else by
  50 × trunc(v / 300), then is raised to the coverage of the level's loud-sound geometry at the actor's position
  (per sound source a polyline: value = strength − trunc(distance to the polyline), 0 beyond). No explosion
  writer exists in this build.

#### 3.2.7 Identification (silhouettes, the mini-map)

- **AI-074** (observed, 00487d00, 004a0a30, 00570e70; high). An NPC that is still a silhouette and is a
  civilian or soldier checks every 16 frames: a friendly soldier is revealed at once; otherwise it is revealed
  when any active, alive, conscious player character has it within G × f, G the base range, f = 1.95 if that
  player's order state is 16 else 1.5, × 1.3 on easy / × 0.7 on hard, the distance elliptical (y × 1.7434) and
  3-D unless the NPC is lower than the player (then the range is extended by the height difference and tested in
  2-D), with line of sight. The script's reveal native calls the same routine. A hostile that sees a not yet
  revealed civilian of kind 4 in its category 0 reveals it too. The mini-map colours follow the silhouette flag
  and the side (grey silhouette, red revealed hostile, green player) — the drawing itself was not read.
  Validation: the oracle's identification "at about 120 px while climbing" disagrees with 600 px in the open
  unless the walkway's wall polygons blocked the line of sight (plausible; one check on open ground settles it).

#### 3.2.8 Hidden and asleep

- **AI-075** (observed, 00489680, 00488f60; medium). A player character playing sprite action 242 or in order
  states 21..23 is invisible to the cone; an actor inside a no-sight zone is invisible from outside it. The
  mission-start cloak and "hidden in leaves" were not matched to these ids (section 6).
- **AI-076** (observed, 00410620, 00433830; high). A sleeping (napping) NPC does not process sightings (the
  view event is not handled in the sleeping state) but does process noises: the noise table of 3.5.2 applies and
  wakes him into seeking or watching.

### 3.3 The AI event system

- **AI-080** (observed, 00424bc0, 00410620, 0040d980; high). Everything the AI does is a reaction to an
  *event record* (seven payload words, a payload kind, the event id) delivered to the NPC's receive routine.
  Sources: perception (3.2), other NPCs ("calls", delivered synchronously by invoking the receiver's routine;
  the return value tells the caller whether the call was accepted), the actor (reached point, could not reach,
  done = an animation or sequence finished, got hit, lose consciousness, fit again, timer), the script, and the
  AI itself (posting to its own queue). "Return to duty" resets the AI (3.5.1).
- **AI-081** (observed, 0041f540; high). The event set, in the analyst's words with the ids the script's
  filter callback sees (5.3): 0 view, 1 out of view, 2 hear, 3 reached point, 4 could not reach point, 5 done,
  6 impossible, 7 timer, 8 shot at by a player, 9 sees body, 10 sees object, 11 sees soldier, 12 sees friend in
  trouble, 13 fit again, 14 got hit, 15 loses consciousness, 16 misses the checked colleague, 17 object gone,
  18 sees the checked colleague, 19 sync, 20 continue after script, 21 return to duty, 22 panic, 23 enters a
  sword fight, 24 quits a sword fight, 25 incoming sword strike, 26 wasp, 27 wasp gone, 28 apple, 29 net, 30 net
  gone, 31 sees beggar, 32 an arrow landed, 33 sees brawl, 34..49 the calls (alert, combat alert, hey, hint,
  instruction, look there, coordinate, report, go to the officer, officer I am back, the colleague is back,
  patrol coordinate, tower-guard alert, tower guard calls me, finish the brawl, you just wait), 50 apple chase
  near, 51 door combat, 52 gallop loop end, 53 sees shadow, 54 arrow launched, 55 stone, 56 adversary weak,
  57 after-combat injury, 58 clean up after the brawl, 59..61 my talk 1..3, 62..64 your talk 1..3, 65 good
  strike, 66 lethal strike, 67 enemy near, 68 my talk 0, 69 your talk 0, 70 stop.
- **AI-082** (observed, 00410620; high). **Pre-filter**, before any state logic: (a) a script-locked AI drops
  every event, except that while a "queue while locked" flag is set reached-point and done are queued and
  replayed in order when "continue after script" arrives; (b) in the wasp reaction only lose-consciousness and
  wasp-gone pass; under a net only lose-consciousness and net-gone; a merry man leaving the map only
  reached-point; (c) a lying actor passes only lose-consciousness and fit-again (the latter only while his
  action id is 7); (d) a timer whose generation stamp mismatches the current substate is dropped (stale);
  (e) an actor out of action (dead, neutralised) drops everything; (f) unconscious passes only fit-again, and
  fit-again is accepted only from the unconscious / napping substates; (g) three events are converted here
  regardless of state: **lose consciousness** clears the order and the head marker, removes the NPC from any
  brawl, sets sleeping / unconscious, puts the stars marker on the actor and silences him; **wasp** clears the
  order, sets the storm-cloud marker and the wasp reaction substate; **net** sets the under-the-net substate.
  For actors flagged for scripting the event id is first offered to the script's filter callback (5.3); a zero
  return drops it. (A console "stupid soldiers" toggle exists; it disables NPC strikes and the stuck-order
  reissue, not the pre-filter.)
- **AI-083** (observed, 00434aa0; high). **Hostile per-16-frame poll**: in walking substates, if the actor's
  animation is a walk (ids 55..58 or 161) but he has not moved for four consecutive polls, the move order is
  reissued, or could-not-reach is posted to self when there is no target; while idle in a non-moving substate
  an idle remark plays with probability 1/12 per poll (rolls one random number), and a boredom byte plays a
  "bored" remark when it exceeds 20.
- **AI-084** (observed, 00419fb0, 00419fd0, 0057aed0, 00486f30; high). The **head marker** (emoticon) is one
  field with an optional expiry in frames: 2 question mark, 3 exclamation mark, 4 Z (asleep), 5 rain cloud
  (stimulus refused), 6 sun (going for a purse or ale), 7 storm cloud (wasp, brawl), 8 spiral (drunk), 0 none.
  Native 228 sets it from the script (5.2). A separate actor marker draws the stars (unconscious; their count
  is min(4, stun ÷ 50) + 1, 005c3fc0), "fit" (1) and a timed spiral after a wasp or net.

### 3.4 Patrol programs (the rail interpreter)

- **AI-090** (observed, 005505c0, 00550f60; high). A rail is walked **back and forth**: the cursor holds the
  point index and a forward flag; advancing forward at the last point steps back to the previous point and
  flips the flag, advancing backward at the first point steps to index 1 and flips it. A one-point rail never
  advances (the actor stands on post). A rail assignment starts at index 0, forward.
- **AI-091** (observed, 00410de0, 00550660, 004121a0, 00419cc0; high). **At a point**: if the point has no
  program, advance and walk to the next point. If the point carries the script flag, the script's reach-point
  callback runs and, unless the script took control (the "script driven" substate), the AI posts itself
  "continue after script". Otherwise the program table is chosen by direction: table id 1 only when arriving
  forward, id 2 only when arriving backward, id 0 always (as rhm.md inferred). A table is a list of blocks
  with a weight; the cached roll r (1..100, AI-007) picks the first block whose weight ≥ r, subtracting each
  skipped block's weight from r; **no block picked → walk on**. Before walking to the next point the engine
  peeks at the block the same roll would pick there: if it contains no blocking command (face, wait, check-for,
  check-for-sync, stop, look, bend) the walk is flagged "do not stop at the waypoint" so the actor passes
  through without halting.
- **AI-092** (observed, 00410010; high). **Opcodes** (operands little-endian; "frames" = the clock of 2.1):

| Op | Operands | Behaviour |
|---|---|---|
| 0x00 | – | flip the travel direction, continue |
| 0x01 | – | end of program (the rest of the block is discarded; unknown opcodes do the same) |
| 0x02 | u16 p | set the current point to p (previous := current) and walk there **without** advancing; an error is logged when p is the current point |
| 0x03 | u16 dir | turn in place to the 16-way facing dir; blocks until the turn finishes (at once when already facing it) |
| 0x04 | u16 t | wait t **frames** (100 = 4 s nominal; the "1/100 s" reading of rhm.md is wrong) |
| 0x05 | u16 guy, u16 N | check-for: look for a colleague (AI-094) |
| 0x06 | u16 guy, u16 N, u16 wp | check-for with synchronisation (AI-095) |
| 0x07 | – | stop: leave the rail, remember the current position as the post, stand there |
| 0x08 | u16 rail | switch to rail `rail` (error if out of range), clear the program, restart the default state |
| 0x09 | – | set the run bit of the walking flags (AI-093), then execute the next command at once |
| 0x0a | – | clear the run bit, then execute the next command at once |
| 0x0b | – | glance left (perception mode 2, the glance animation), wait for it to finish; soldiers only |
| 0x0c | – | glance right, likewise |
| 0x0d | u16 t | **bend** (the stoop animation) and wait t frames; soldiers only (rhm.md's "check-for with a radius" is wrong) |
| 0x0e | – | patrol-stop flag := 1; a marked profile plays a remark; continue |
| 0x0f | u16 k | re-link company member k in the follow order (mechanics observed, purpose inferred) |
| 0x10 | – | patrol-start: flag := 0, remark for marked profiles, re-sort the company by distance; continue |

  The commands that are illegal for civilians (bend, the glances, check-for, patrol start / direction / stop)
  and for friendly soldiers (the two check-fors) are logged and skipped.
- **AI-093** (observed, 00412240, 0041bd20, 00576e00; high). The **walking-flags word** governs every rail
  move: bit 0 = **run** — the actor plays the *sprint* cycle (action 10) instead of the walk (6) (a further bit
  forces the walk, another selects cycle 290); 0x400 no stop at the waypoint; bit 2 stop exactly on the point
  (cleared when the target is on another layer); 0x10 check reachability first and report could-not-reach;
  0x20 turn to the point's direction after arriving and play a follow-up action; 0x80 accept arrival within a
  radius. Native 140 writes this word (0 walk, 1 run, other values verbatim) and re-issues the current leg with
  the new flags if the actor is on a rail.
- **AI-094** (observed, 00439080, 00424d50, 004444a0; high). **Check-for(guy, N)** (hostile AI only): guy
  indexes the mission's NPC table (must be an NPC other than self). Skipped when guy is in the actor's own
  company or when the actor's last alert is less than 3000 frames (120 s) old. Otherwise (warnings only) at
  least one waypoint of guy's path or his post should be visible from here; the actor puts the player
  characters into his immediate category 4 (AI-070), sets a look count = N ÷ 10 + 1 and a look interval =
  1000 ÷ count frames, and starts a random left or right glance (one roll). The loop: glance done → "looking
  for the colleague" with a 10-frame timer; on each timer a new glance starts with probability
  (acc + 10) / 5000 (one roll, then one for the side), acc grows by the interval every 10 frames; once acc
  exceeds 1000 the actor gives up and goes **seeking**: a remark, then a search route built from guy's post or
  guy's waypoints starting at the nearest (3.5.4). Seeing the colleague meanwhile ends the look (the
  colleague-detected substate: the "checked" flag is set; an officer with known soldiers glances once more,
  others return to duty). N is a look budget, not a radius.
- **AI-095** (observed, 00439080, 00410de0; high). **Check-for-sync(guy, N, wp)**: with N = 0 a pure
  synchronisation: wp < 500 is an absolute waypoint index of guy's path, wp ≥ 1000 means "my current index +
  (wp − 1000)". If guy is in the default state and already at or beyond that waypoint (direction-aware), the
  program continues; otherwise the actor registers in guy's waiter list and waits (polling every 20 frames)
  until guy reaches a point whose index equals the stored one and posts the sync event to every waiter. With
  N > 0 the check-for look loop runs first and the synchronisation follows.
- **AI-096** (observed, 00418600, 00424d50, 0044cae0; medium). **Resuming**: after an alert the default
  state's re-entry walks the actor to the *nearest* waypoint of his rail and continues from there ("return to
  route"); an interrupted program resumes its pending command; native 220 replaces the rail by the alert rail
  (2.10) and resets the default state.
- **AI-097** (observed, 0041a960, 0041ac50, 00412d60, 00449170; medium: the slot geometry was not read).
  **Company**: members follow the chief: on "go to the chief" a member walks to a slot relative to the chief
  (members sorted by distance and paired left / right) then waits 200 frames; the chief on a patrol leg turns
  to the stored facing on arrival, waits 200 frames, and moves on only while his followers are in the default
  or wondering state. Natives 218 / 219 add / remove subordinates (error if the chief is on a patrol or the
  subordinate already has one).

### 3.5 The alert state machine (non-attacking states)

Top-level states: **sleeping** (dead for good / unconscious / napping / awakening), **default** (post, route,
patrol, script-driven, in a program, looking for a colleague, synchronising), **wondering** (curiosity and
stimuli), **seeking** (search and reporting), **attacking** (3.7), **menacing** (standing over a knocked-out
player), **fleeing**. "T = n" below means the AI timer is armed with n frames; "×m" means multiplied by the
reaction multiplier of AI-100; "reset" means return to duty (AI-101). Rolls name their random draws.

#### 3.5.1 Reaction delays, reset, initial state

- **AI-100** (observed, 00438620 disassembled, 0055dbb0; high). Every "reaction" delay marked ×m is
  computed as trunc((100 − initiative) ÷ 100 × base × m + 1) frames, initiative = SD `p1` (unscaled), m = 2 on
  **easy and hard**; on **medium** the multiplier is an uninitialised stack slot (it holds the object pointer's
  bits reinterpreted as a float, a value around 10⁻³⁸), so the product vanishes and **every scaled reaction
  delay is 1 frame on medium** — an original defect that the oracle (medium) saw as instant reactions. For a
  friendly-side NPC that is not mounted, when the mission's outfit byte is set, the delay is a flat 3 frames.
- **AI-101** (observed, 00435c80, 00430300; high). **Return to duty** (the reset): the current facing becomes
  the post facing, the target list is cleared, the patrol rail is restored (or "no patrol" noted); if the actor
  is awake and not script-locked the default state's en-route substate is entered and the base logic decides
  between go-to-post, go-to-route (AI-096) and the patrol. On a hostile actor the hit points are also put
  through the difficulty scale at this point (principal enemies excluded).
- **AI-102** (observed, 00414550; high). **Initial state from the placed action**: action 48 → unconscious
  with a knock-out of 300 frames requested from the actor; 45 or 47 → dead for good; 162 → napping with the Z
  marker; 159 or 270 → on post with a flag; 0 / 1 / 3 → on post with T = the look-around interval; an actor
  placed inside a house sector → the inside-house substate and the global "inside houses" list.
- **AI-103** (observed, 0041a420; high). The **look-around interval on post**: officers 200 + rand%600;
  profiles with `p4` ≠ 0 400 + rand%800; everyone else 70 + rand%70 frames (one roll each).

#### 3.5.2 Perception events → reactions (default, wondering, seeking)

- **AI-104** (observed, 0044d750; high). **Company relay**: a hear, sees-body or sees-object event reaching a
  member of a company (the chief's member list is non-empty) in the default or wondering state is re-sent to
  every soldier of the company and its chief, and the receiver reports it handled; a receiver with an able
  chief lets the chief's routine handle it instead. So a company hears together.
- **AI-105** (observed, 00433060, 00439cb0; high). **Sighting of an enemy (view)** in default / wondering /
  seeking: ignored if the seen element is hidden or a player already held by another NPC; ignored (hostile
  side only) if the target stands more than 100 px higher than the viewer, unless script-marked. Then the
  sighting frame is recorded; "seen clearly" = same layer or at most 50 px below, or, if higher, flat distance²
  ≤ dz²; the position is remembered with priority 5; a viewer standing in a house / door sector goes to the
  door-fight logic (3.7.7). Otherwise the target list is cleared, a **look-there call is shouted to every
  same-side human within 100 px** (accepted only by receivers idle, wondering or just-watching), and: a rider →
  attacking / reaction-running toward the target, T = 10; anyone else stops, plays the sighting remark (the
  "!"), sets the target and: distance < 30 px → the reaction substate and an immediate decision; seen clearly
  → the reaction substate with T = 5; else the reaction-turning substate, a turn to the target, T = 20. (These
  three are fixed, not ×m.)
- **AI-106** (observed, 00433830; high). **Noise (hear)** in the sleeping, default, wondering or seeking
  states, by noise kind (AI-071's kinds): kinds 1, 2, 9 (quiet) only in default → question mark, wondering /
  watching, face the spot, T = 50 (kind 2 plays a remark). Kind 8 is ignored by friendly NPCs and when already
  watching, else like 3. Kinds 3, 7, 13 (steps, running, a heavy noise): unless busy with money or a brawl →
  remember the spot with priority 1, and (a) if not seeking, or in "got stop", or an officer → seeking /
  heard-steps pre-reaction, question mark, remark (not for kind 7), T = 1 if already in default else 60 ×m;
  (b) already seeking → heard-steps reaction, face the spot, T = 1. Kind 10 (a whistle): if nobody of the
  company is already reacting to a whistle and the profile's whistle word is non-zero → seeking /
  just-watching, question mark, face, remark, T = 60; else remember (priority 1) and, if not seeking or an
  officer → wondering / heard-whistle, question mark, T = 50 ×m, else the heard-steps reaction. Kinds 11, 12:
  in default → watching toward the spot, T = 70 + rand%60 (one roll).
- **AI-107** (observed, 00434320; high). **Sees a body**: the body is filed and remembered with priority 2; a
  down-but-alive player character joins the "prey" list. In a seek-point substate or while seeking a colleague
  → go to it as a found body; in a body substate for another body → queued; otherwise a remark (a different
  kind if the body still carries money), the 100 px look-there shout, question mark, seeking / body reaction,
  face, T = 80 ×m.
- **AI-108** (observed, 00434710; high). **Sees an object**: object kind 3 (ale): in an ale substate → queued;
  else clear the order, remark, face, question mark, wondering / ale reaction, T = 60 ×m. Kinds 7 and 8
  (purses): in a money or brawl substate → queued; else remark, face, question mark, wondering / money
  reaction, T = 60 for officers, 30 for others.
- **AI-109** (observed, 00433790; high). **Sees a shadow** (the "?" of AI-069): unless in a house sector or
  hiding → default / looking at the shadow, face the spot, T = 10; on each timer the actor returns to duty if
  his shadow counter is zero, else waits 10 more.
- **AI-110** (observed, 00433f90; high). **An arrow landed**: remember with priority 5; already seeking and not
  an officer → seeking / arrow reaction toward a point near the arrow, face, T = 1; officers → arrow
  just-watching, face, the look-there shout within 200 px, T = 60; others → arrow reaction with T = 100 ×m.
  The arrow reaction's timer → a remark, run to the arrow, the 100 px shout, T = 200; on arrival or timeout →
  an area search from self (radius 0, thorough for rank 0).
- **AI-111** (observed, 00439e70, 00439f60, 0043a080, 004457c0; high). **Look-there call** → question mark for
  10 frames (hostile only), wondering / watching, face the reported spot, T = 100. **Tower-guard alert call**:
  knights → seeking / knight watching the tower guard (its timer → an area search of radius 300 around the
  remembered spot), others → wondering / watching the tower guard; question mark 10, remember (priority 5),
  face, T = 100. **Tower guard calls me**: rank 0 → call the nearest officer (AI-121), rank 1 → the officer's
  tower-guard routine (not read, section 6). **Stone**: wondering / apple reaction with a remark, face,
  question mark, T = 50. **Stop** (from a script): in a sword fight ignored; else seeking / got-stop, stop
  moving, marker cleared; its timer → restart the patrol path and look around.

#### 3.5.3 Wondering (curiosity, stimuli, the brawl)

- **AI-112** (observed, 00424d50; high). **Watching → looking around**: watching and each "look n sideways"
  on timer → the next substate with a random glance (one roll for the side); each "look n" on done → the next,
  the facing turned by +5 sixteenths (112.5°), T = 30 + rand&7 (one roll); after the third look → reset. So a
  curious guard turns three times with glances, about 35 frames each, then returns to duty (about 4..5 s).
- **AI-113** (observed, 00424d50, 0043d890; high). The **glance**: kind 0 one side, 1 the other, 2 / 3 both
  orders (the sprite's head-turn animations 134 / 135 / 136 on soldier profiles), 4 the bend; it ends with the
  done event.
- **AI-114** (observed, 00424d50, 00445d40, 004460f0, 004468a0, 0044b000, 00446400, 004469e0; high). **Purse /
  money**: money reaction, timer → if the purse word is non-zero and a purse is reachable: pick the nearest
  seen purse (+300 px penalty for another layer), remark, approaching-money, sun marker for 20, walk (arrival
  20 px), T = 5; not susceptible → rain cloud for 50, remark, purse list cleared, reset. Approaching: on timer,
  if a rival soldier also heads for it → running for it (the run flag) and, if the company's officer can see
  me, he receives sees-brawl. On arrival: purse still there and within 25 px → stop, play the pick-up
  animation (84) on it, notify every same-side soldier in a money substate that the object is gone, taking
  money; farther → watching for more money with a glance. Taking money → the next purse in the list, else
  watching for more money. **Brawl**: an object-gone for a purse I wanted: if the beer rule (below) says so and I
  am approaching → face the taker, storm cloud, brawl reaction with the taker as rival, T = 30 ×m; in a brawl
  already → the taker joins the rival list; else go for another purse. Brawl reaction, timer → every officer of
  the side in default or money-reaction within 200 px, or within 350 px with sight of me, receives sees-brawl;
  brawl approaching, storm cloud, remark, walk to the rival (arrival 30), T = 1. Approaching, arrival: rival
  awake and within 33 px → stop, play the punch (86) at him, brawl hitting; farther → re-walk; rival asleep →
  drop him. Hitting, done → panic to every civilian who can see me; officers alerted again; a down rival is
  removed; **continue** iff (drunk soldiers of my side × 100 ÷ (brawlers + drunk)) < the profile's **beer**
  word (always when it is 100 or the AI is civilian-class): the rival still standing → approach again (T = 30),
  another rival (the nearest soldier in a money substate) → approach him, none → the nearest purse (running) or
  watching for more money. Got hit in a brawl → recovering (the get-up animation 71), officers alerted, storm
  cloud; recovering → the next rival or the purse. Watching for more money → clear the brawl list; an
  unclaimed purse → approaching loot (arrival 20) → within 100 px: a lying body instead of a purse → remember
  (priority 2) and go to the body; else loot (animation 85) → purse count grew → sun for 20 + remark, else rain
  cloud for 20 + remark; more purses → next, else reset. **Officer and the brawl**: sees-brawl → walk to the
  brawl (arrival 100) → finishing the brawl: the finish-brawl call to every brawling soldier of the side + a
  remark, storm cloud, T = 200 → clear the purse lists, every brawler but the addressed one gets return to duty;
  a soldier receiving finish-brawl stops, faces the officer, marker cleared, "looking at the officer",
  T = 300 + rand%32.
- **AI-115** (observed, 00424d50, 00449df0; high). **Ale**: ale reaction, timer → beer word non-zero →
  approaching the ale, sun for 20, remark, walk (arrival 20), the return spot stored, T = 20; else rain cloud
  50, remark, reset. Arrival: the ale gone → face, storm cloud, "ale gone", T = 30; else play the drinking
  animation (141) on it, drinking; drinking done / ale gone timer → the next ale in the list, else reset. (The
  spiral / drunkenness after drinking is set by the ale object's code, not read.)
- **AI-116** (observed, 00424d50, 0044cb40, 0040e7e0; high). **Apple**: apple reaction, timer → apple word
  non-zero and a child found → apple chase (running to the child; count = apple ÷ 2 legs: on each timer if the
  count is 0 → end, face the child, T = 30; else count −1, run to the child, T = 10; the dialogue events play
  the talk); else reset. The child flees (3.5.6). Apple in the visor → the hit-by-apple reaction then the stone
  reaction. Wasp in the armour: events blocked until wasp-gone → marker cleared, question mark, look around
  (T = 30). Under the net: until net-gone → "fit" marker, question mark, look around.
- **AI-117** (observed, 00424d50, 00443d10, 00443af0; high). **Whistle**: heard-whistle, timer → face the spot,
  watching after the whistle, T = 60 → whistle word > 1 and the "never leaves the post" value ≠ 100: rank 0 →
  an officer reachable near the spot → walk to him, "looking to the officer for advice", question mark,
  T = 100; else if `p3` < 50 or ≥ 1 soldier is known → the officer-looking behaviour; rank 1 → the officer
  looking for soldiers; rank 2 → an area search around the spot with radius (whistle − 2) × 400 ÷ 98 px; not
  susceptible → reset. A civilian child approaches the whistler (walk to 50 px, remark) and stands there 100
  frames.

#### 3.5.4 Seeking (the search)

- **AI-118** (observed, 00424d50, 00438cb0; high). **Heard steps**: pre-reaction timer → ranks 0 and 2: if the
  actor may leave his post (flag bit 4 clear and the "never leaves" value ≠ 100) → heard-steps reaction,
  question mark, face, T = 60; else just-watching (a glance sequence that ends in reset for rank 0, or in the
  officer's soldier-look for rank 1), question mark, face, T = 60; rank 1: ≥ 1 known soldier or the spot more
  than 100 px away → just-watching, else the reaction. Reaction timer → rank 0 with an officer reachable near
  the spot → walk to him, "looking to the officer", T = 100; else walk to the spot (T = 200) and, on arrival or
  timeout, an **area search** from the actor's own position, radius 0, flags "run, thorough".
- **AI-119** (observed, 0043a220, 0043d0c0, 0043d530, 00522330, 00416800, 00438600; high). **Area search**
  (centre, radius, flags, optional direction): friendly-side NPCs and NPCs whose "never leaves" value is 100
  return to duty instead. The order is cancelled (unless a flag says otherwise), a pending body takes
  precedence, the player characters are put into category 4 when the search budget allows. If radius > 0:
  every **seek point** of the level (a global list loaded from the mission; which chunk was not traced,
  section 6) gets a cost = distance² to the centre + 100 per layer step; points within radius² count; with a
  direction, points in a 15-sector fan around it are preferred (nearest ahead, then to the sides); points with
  cost < 10⁶ are inserted in cost order. If the first point's **freshness** < 90 the "thorough" flag is cleared;
  every other same-side soldier within 500 px already searching multiplies a budget by 1.2 and clears my
  thorough flag. The number of points to visit = the count in radius (more when thorough); points are then
  **drawn in cost order, each accepted with probability freshness % (one roll: rand%100 < freshness) and
  inserted at a random rank (one roll)**; a visited point is *cooled*: freshness = 100 − remaining ÷ 100 %,
  cool-down 5000 frames per visit, never more than 10000 frames ahead. Flag bit 1 adds the centre itself as a
  point; bit 0, or an empty list, adds the NPC's own position as the last point ("come back here"); a flag
  selects running. Then the next point is taken (or, inside a house, the sideways watch).
- **AI-120** (observed, 0043d530, 00424d50; high). **Next seek point**: for each remaining point in order, with
  probability freshness % (one roll) → cool it, "going to the seek point", question mark, walk or run; a
  rejected point is skipped; a beggar seen during the search takes precedence (approach to 50 px, question,
  a second question, T = 30 / 50 / 100, a betraying beggar points at the player: the pointed player becomes the
  target and the archer logic fires); when nothing is left → reset, and if the best remembered priority ≤ 1
  and the flags lack the "found" bits → the "nothing found" remark. At a seek point: choose a random free
  direction from the point's perimeter list, excluding directions within 1/16 of the current facing (reservoir
  choice, one roll per candidate); watch it (T = 20), a sideways glance (one roll), the next direction, and so
  on; passed-ambush-point substates re-approach the point with a glance.
- **AI-121** (observed, 0043c960, 00424d50; high). **Reporting to an officer** (rank 0): the nearest awake
  officer of the side in the default state, cost = distance + 100 per layer; if any soldier of the side is
  already reporting, nobody else goes; walk to 70 px of him, T = 50. On arrival, if the officer is available →
  give the alerting report (the remark by what was seen: priority 0 / 1 a noise, 2 / 4 a body, 3 a missing
  colleague, 5 a sighting), T = 150 → by report kind and how many men the officer has already sent (noises
  need 0 sent, bodies fewer than 2, sightings fewer than 5) → the report call to the officer, the talk, a point
  at the spot, T = 100 → end, T = 30 → reset. Officer side: **calls his soldier** ("hey", accepted only by
  rank-0 soldiers in default / wondering or a few seeking substates: they stop, face, question mark for 20,
  T = 20), waits (T = 20 polls), instructs (talk events, a point at the spot), then waits for the instructed
  soldier with a patience counter of 101 polls × 30 frames (about 2 minutes) after which he alerts the tower
  guard or searches himself (radius 300); a soldier instructed copies the officer's target spot and searches
  from it (radius 0), then returns to report (T = 100). **Groups**: an officer calls his whole company (the
  instruction call carries a spot per member; bodies are spread over the members), waits for them (pruning to
  members still coming), and prunes to members still searching; an officer inside a house first leaves it.
  **Civilians report** (3.5.7) through the alert call; the soldier waits for the report (T = 20 polls,
  question mark 20), takes it (T = 30) and then: rank 0 with `p3` ≥ 50 → search 300 px, else call an officer
  or search; rank 1 → search or the tower guard; rank 2 → search 300.
- **AI-122** (observed, 0044a760, 00424d50, 0043ccc0, 0043cec0, 0044cf00, 0044a830; high). **Bodies**: body
  reaction timer → friendly viewer → search 300 px around self; hostile: a remark once; rank 0: an officer near
  the body → walk to him (T = 100); else by `p3`; rank 1: body > 150 px away and `p3` → the officer look; else
  **go to the body** (exclamation mark, arrival 30, T = 10). At the body (within 60 px): a same-side body →
  "looking at the dead body", the body handed to the actor, T = 50 → hostile: the next body, or the
  missing-colleague alarm (a remark, remember own position with priority 3, rank 0 calls the nearest officer,
  rank 1 alerts the tower guard, others search 400 px around self — 200 if the colleague has a patrol; the
  radius is halved for post-bound units); an enemy body not out of action → reset; else **wake the sleeper**:
  the revive animation (75) on the body, T = 50 → the next body or the alarm. A netted comrade → cut the net
  (five times animation 85 then 84) and help him up.
- **AI-123** (observed, 00424d50; high). **Combat alert call** (from a fight, 3.7): rank-0 receivers in
  default / wondering / seeking: question mark 10, face the fight, T = 50 ×m → run to the spot (arrival 50) →
  search 300 px around it; attacking receivers accept silently; others refuse.
- **AI-124** (observed, 00424d50; high). **Could not reach**: at a seek point → the next point; going to a body
  → the next body or a search 300 around self; in panic → the movement handler; else a blocking door is opened
  or flagged and the event re-posted; in sleeping / default / wondering / menacing / fleeing → reset; seeking →
  search 300 around self. **Impossible** → done is posted to self (except while killing a sleeper).

#### 3.5.5 Sleeping

- **AI-125** (observed, 00424d50, 00471c00, 00414550; high). **Unconscious**: nothing but fit-again passes;
  fit-again (raised by the stun level falling below 30, 3.11) → awakening (the "fit" marker, T = 100) → if the
  actor has a patrol path not yet restarted, restart it; question mark, look around (T = 30) → reset. A
  "wake now" flag skips the awakening. **Napping** (placed asleep): no timer; a noise per AI-106 or a hit wakes
  him. **Dead for good**: nothing passes. Waking by a comrade: the reviver's animation (75) with the body as
  partner; the actual wake-up is the stun path (AI-153).

#### 3.5.6 Fleeing

- **AI-126** (observed, 00424d50, 0040e7e0, 00413240; high). Entries: a civilian's panic (a brawl punch or a
  fight seen), "you just wait" (a soldier chasing a child), "apple chase near" (a friend chased), the archers'
  retire / run for arrows / run to alert soldiers, and the run-away decision (3.7.1). A chased child picks a
  flee spot: 15 directions at 300 px, the radius shrinking by 10 per ring until reachable, a random start
  direction (rand%5 + 14 plus the chaser's bearing, one roll, sides alternating); runs at speed × 1.2 while the
  chaser is more than 150 px away, else × 1.0; a few more legs after the chaser gives up (at most 7), then
  reset. A merry man leaving the map runs to the exit and, within 10 px, fades and is removed. Run to alert
  soldiers → the tower-guard alert, else run to the door. Retire from combat → turn to face, then back to the
  attack logic. Run for arrow reserves → on arrival the actor receives 20 arrows, then an area search 300.

#### 3.5.7 Civilians (the friendly AI)

- **AI-127** (observed, 0040bfd0, 0040c140, 0040cc60, 0040d150, 0040d440, 0040d690, 0040e290; medium-high:
  the attitude branches were skimmed). Same pre-filter and event set, no attacking. A sighting in default /
  wondering: a friendly-attitude civilian "admires the hero", a hostile one reacts as to an enemy; in seeking
  the position is remembered (priority 5); in fleeing another flee leg with a remark, at most 7. A body →
  the civilian body reaction. Both reactions, on timer → the nearest idle same-side soldier (cost distance +
  250 per layer, path-checked) → run to him (arrival 70, remark); none → flee. At the soldier: idle and within
  70 px → the alert call; accepted → the report (a start, a point at the spot with the position, an end,
  T = 10 / 30) then flee; refused or busy → another soldier or reset. Panic → run away from the source; a stop
  from a script → got-stop, T = 100. A whistle → a child approaches the whistler. Civilian-class AIs bypass the
  purse / ale / whistle susceptibility guards. Civilians must never be killed by the player's side (the manual);
  the code makes them untargetable by strikes (3.9.1) rather than ending the mission — the loss rule, if any,
  lives in the scripts.

### 3.6 Calls between NPCs and how an alarm spreads

- **AI-130** (observed, all of 3.5; high). There is **no mission-wide alarm broadcast** in the AI code (the
  console's "alert every NPC" handler was not located). An alert spreads by: (1) the 100 px look-there shout on
  a sighting, a body or an arrow (200 px for an officer's arrow shout), which only makes idle receivers watch
  the spot for 100 frames; (2) the company relay (AI-104); (3) a soldier's report to the nearest officer, who
  instructs his group (AI-121); (4) the combat-alert call sent from a fight (AI-123, 3.7.1); (5) the tower
  guard's alert call (AI-111); (6) civilians reporting (AI-127); (7) brawls: sees-brawl to officers within
  200 px (350 with sight), panic to civilians who see a punch. The "!" is the head marker 3 (or the sighting
  remark); the "?" is marker 2.

### 3.7 Attacking: decisions and tactics

#### 3.7.1 The decision procedure

- **AI-140** (observed, 004303e0, 004207f0, 00432d30, 00438cb0; high). Run at the end of most attacking
  timers (reaction, overview, reserve overview, observe, bow observing, after a step back, when a partner is
  lost). (1) Reset every known enemy's attacker count; rebuild the friends (own side, alive, targetable,
  within 500 px) and enemies lists; for each friend NPC in a melee substate (running / walking / charging to the
  enemy, sword fight, special strike, parade, approaching a new enemy, moving around an old one, stepping back)
  increment its target's attacker count and the engaged count E; a friend in another substate but closer to my
  target than I am also counts. (2) n = number of enemies, dmin the minimum enemy distance, whether a player is
  among them, whether a lower-ranked friend exists, whether a plain soldier is among the friends; sleeping
  (unconscious, not netted) enemies go to a separate list. (3) No enemy left → return to duty. A forced
  decision (an officer's instruction, a script) is used if stored. A **post-bound** actor → observe. Else the
  **morale check** (AI-141) gives fight or retreat. (4) Fight, melee actors: an officer with a rank-0 friend
  present and not already alerting → **alert soldiers**; else if E < n or dmin < 150 px: the "never leaves"
  value 100 and dmin ≥ 150 → **last reserve**; else if no lower-ranked friend exists or the too-proud test
  fails: on the hostile side, when (courage × 0.045 + 1) × n ≤ E → **observe** (enough of us engaged) else
  **fight**; else → **too proud to attack**; else (E ≥ n and dmin ≥ 150) → **reserve**. Tower guards: alert if
  not yet, else fight when dmin < 150 else observe from the tower. Archers (a ranged handle and a hostile-kind
  AI): the shooting line blocked by a friend → **step back**; an archery path assigned → **shoot**; no shield
  bearer: arrow reserves exist → **run to the archery point**, else **cover behind a shield bearer** when one is
  found, else shoot; with a shield bearer: cover when farther than 25 px from the cover spot, else shoot. (5)
  Retreat: an archer without arrows → **run for new arrows**; rank 0 with a visible player and not already
  alerting → **look for help**; rank 1 → **run and alert soldiers** (same guards) else **run away**; rank 2 →
  run away. Permissions consulted from seeking and some transitions: fight / run away need flag bit 4 clear and
  the "never leaves" value ≠ 100; reserve needs the beer word ≠ 0; alert soldiers and menace the purse word ≠ 0;
  run-and-alert the apple word ≠ 0; shoot / step back `p3` > 49; look for help `p3` < 50 or a group; cover
  the whistle word > 1; observe the profile's `q2` > 50; a civilian-class AI may only run away, reserve, alert,
  run-and-alert or menace.
- **AI-141** (observed, 00432d30; high). **Morale**: strength = Σ over friends (a player 100, an NPC 100 +
  `p4`); r = 100 × strength ÷ (100 × n + 1); s = r < 100 ? max(0, (r − 20) × 50 ÷ 80) : min(100, 50 + (r − 100)
  × 50 ÷ 200); if hp < max: s = s × hp ÷ max (integer); a rank-0 soldier with an officer among the friends:
  s × 30. **Retreat iff s < 50 − courage ÷ 2 and rand%100 > courage** (one roll; courage 100 never retreats).
  Retreat unconditionally when fleeing, or an archer without arrows, or an archer caught in a sword fight.
- **AI-142** (observed, 0043d780, 0042fe00, 0043ea70; high). **Target choice**: the nearest valid enemy with
  the distance √(dx² + (1.7434 dy)²); a flag adds 100 per attacker already on that enemy (prefer the less
  engaged), another adds 10000 (prefer the unengaged), a third skips the validity test; the join from observe
  uses the second, reserve and step-back the first, shields the third. "Enemy near" (event 67) while in
  reaction / observe makes that actor the target at once. Got hit outside a sword fight: a brawling hitter →
  the brawl logic; otherwise the hitter becomes the target. Partner switching: if my first opponent already
  fights ≥ 2 of us and a crowded friend exists, my target may be reassigned.
- **AI-143** (observed, 00442470, 004303e0; high). **How many attack one player**: nothing caps it but the
  observe rule (with courage 40 — a blue halberdier — 2.8 attackers per enemy before newcomers observe), the
  target picker's penalties, and the observer's join test (join when my target's first opponent has ≤ 2
  partners or I am within 30 px, and no other friend is already running to it; otherwise keep 100..200 px).
  Several soldiers do attack one hero at once (the oracle's five agree).

#### 3.7.2 Execution of the decisions

- **AI-144** (observed, 004303e0, 00432f80, 0042ef80, 0042fe00, 00445210, 0043c960; high). **Fight** → pick
  the target, then run / walk / charge to it by distance and the class head (a shield charge when the level's
  word allows, the distance > 99 px and the actor is not mounted; a battle cry on a charge if courage > 39),
  T = 10; on arrival at reach → the **sword fight** substate, T = 20, a remark, the adversary links set (3.8.1).
  **Observe** → walk to 100..200 px of the target, then a 50-frame overview and decide again. **Reserve** →
  stand, look, and every timer send the coordinate call to the other reserve soldiers; on receiving one → a
  20-frame overview then decide. **Step back** → a free spot 50..250 px behind, walk there. **Look for help** →
  set the alerting flag; a path to an officer → a remark and run; else run away. **Cover** → run behind the
  shield bearer. **Too proud** → walk to 100..200 px and stand, overview T = 150, retire. **Tower guard** →
  the alarm animation / observe from the tower. **Archer observe** → a look-around unless already in a bow
  state. **Run to the archery point** → run along the archery path. **Last reserve** → a remark and a position
  check. **Run away** → fleeing (3.5.6). Reaction time: from seeing / hearing; after done / timer, if the enemy
  is nearer than 30 px → immediate (T = 1), else T = 30 ×m or 10.

#### 3.7.3 Per-kind tactics

- **AI-145** (observed, 0044a090, 00424d50; high). **Officers** (rank 1) alert soldiers when a rank-0 friend
  exists (stand with the arm raised, T = 20, decide again); rank-0 soldiers near an officer get × 30 morale.
  **Too proud** (officers and knights, rank ≠ 0, hostile-kind AI): skip fighting while a lower-ranked friend is
  already engaging or observing the same target — officers and knights let the lower ranks fight first.
  **Knights** (rank 2) run away in the retreat branch; their too-proud substates approach to 100..200 px,
  overview 150 frames, retire.
- **AI-146** (observed, 00424d50, 00445510, 00445180, 0044b4e0; high). **Archers**: loading → aiming with
  T = (110 − ranged skill) ÷ 2 frames → if a friend blocks the line → decide, else shoot (3.12) → observing
  T = 50 → decide; retire from combat (turn to face, decide); arrow reserves are level-defined piles; archery
  paths are rails walked with a final sprint; morale: no arrows or caught in melee → retreat.
  **Crossbowmen** use the same paths (no distinct branch).
- **AI-147** (observed, 00447e50, 0044dd70, 00447070; medium). **Shield bearers / phalanx**: a phalanx needs
  at least two partners already in it; the formation point comes from the leader's facing; archers run behind
  the shield bearer; the "protecting with the shield" substate looks around at random and, with a 1-in-4 chance
  per timer (one roll), picks a new target and advances; "advancing with the shield" adopts the partner's
  target.
- **AI-148** (observed, 0044c850, 0044ca90, 00424d50; medium). **Riders** (flag bits 1 / 2): charging — pick
  a free point 500 px ahead within ±1 sixteenth (radius shrinking by 10), gallop through, on "gallop loop end"
  with nothing left → run and engage; returning / getting distance / passing substates.
- **AI-149** (observed, 0044b940, 0044bc70, 00424d50; medium). **Door fights**: waiting at a door 150 frames,
  turning, leaving; **ladders**: wait at the ladder for the enemy (T = 100 / 30). **Sleeping enemies**:
  approach within 20 px → the execute animation → a remark → return to duty; a player in a coma is
  **menaced** instead (the menacing state; got hit there makes the hitter the target; another enemy seen →
  the normal sighting; its timers were not read, section 6).

### 3.8 The NPC in a sword fight

- **AI-150** (observed, 0043db20, 00441650, 00442190; high). The **sword-fight step** runs on the timer
  (re-armed at 20 frames whenever the substate is the sword fight), on done and on reached-point, and on the
  adversary-weak and after-combat-injury events, in this order: (1) no adversary left → leave the fight; the
  target dead or incapacitated → quitting (T = 3); target := the first adversary; a lost player → an area
  search (radius 300), else return to duty. (2) The actor must face the target within ±1 sixteenth, else wait
  for the next timer. (3) Rebuild the friends (own side in a sword fight within 500 px) and enemies lists. (4)
  Two rolls rand%100 ≤ hesitation (the AI's hesitation byte; 0 for hostile soldiers → 1 % each) → skip this
  step. (5) On an adversary-weak step: if the distance > head word 2 and experience > 59 → move to head word 1.
  (6) If not post-bound, and not exactly one friend against one enemy, and rand%3 = 0 (one roll) → choose a new
  position / target (approach a new enemy or move around the old one) instead of striking. (7) Distance > head
  word 2 (90 px for pole arms, 70 for swords) → move within head word 1 (65 / 50); a post-bound fighter instead
  returns to his post when more than 20 px from it. (8) **Strike**: only if the target is not in a hit or fall
  sequence and its action state is one of the fighting states (7..11); the figure chooser (AI-151) returns a
  figure → the special-strike substate, the attack sequence aimed at the target (a battle cry before the
  finishing blow and the sweeping figures); when its animation is done or the timer fires → back to the sword
  fight with T = 20.
- **AI-151** (observed, 00485240, 00485590, 00482240, 00482fd0, 00440cf0; high). **Figure choice**: skill =
  melee experience (`q1` scaled, or the player's word); threshold = max(skill, 50); rand%100 ≥ threshold →
  **no attack this step** (one roll; a skill-5 soldier attacks on half of his steps, a skill-80 one on 80 %).
  Otherwise for figures 0..8: allowed iff skill ≥ {0, 40, 95, 20, 20, 70, 70, 80, 80} and the hesitation byte
  ≤ {80, 50, 0, 20, 20, 0, 0, 80, 80}; the circles get +500 when several enemies are near; a figure is scored
  only if the target is idle or the figure's displacement + 2 < the target's remaining displacement; score = Σ
  over opponents inside the figure's arc and reach band (row words 4..5, the angles) of (expected value + 30),
  expected value = damage × (100 − the victim's defence) ÷ 100 + stun × (100 − the victim's stun resistance) ÷
  100, −1 if a friend is in the arc, minus a per-figure fatigue that grows by 50 when a figure is used and decays
  10 per choice. The best score with ≥ 1 opponent wins; "step back" when the adversary is too far. The block
  (row 9) is returned only in reaction mode (AI-152).
- **AI-152** (observed, 00442190; high). **Reaction to a player's strike** (event 25 while in the sword fight,
  special strike, approaching or moving around): the incoming figure is classified; the chooser runs in
  reaction mode and returns the **parade** when the parade animation's displacement is shorter than the striker's
  remaining displacement; then with experience > 49, a circle-type incoming thrust and a free spot at reach +
  10..20 px → **step back**; else the **parade** substate (the block animation, T = incoming displacement + 10);
  if the chooser returned an attack → a **counter-strike** instead. Good / lethal strike events only play
  remarks. Quit-swordfight or object-gone in a melee substate → quitting (T = 3) → decide again. Fit-again in
  a fight → the awakening path.
- **AI-153** (inferred from AI-150..152 and the oracle; medium). **Cadence**: there is no swing timer. A
  strike attempt happens at every 20-frame step that passes the gates (facing, target in a fighting state and
  not mid-flinch, the 1 % hesitation rolls, the 1-in-3 manoeuvre roll, the max(50, skill) % attack roll), then
  the strike's own animation and the parades the player's swings provoke interleave. For a skill-5 halberdier
  the expected interval is about three steps plus animations, 3..5 s nominal; the oracle's 5.3 s between
  halberd raises is not contradicted but not derived exactly (section 6).

### 3.9 Melee resolution (both sides)

#### 3.9.1 Engagement, turn taking, disengagement

- **AI-160** (observed, 0047c600, 0047c4c0, 0047f520, 004801d0, 004808f0, 00472070; high). **Engagement**: A
  adds B as an adversary if B is attackable (alive, not a civilian for a player's side, not immune to the
  attacker), the height difference ≤ 40 px or both on the same layer, the distance ≤ head word 3 (150 px) and
  the line of sight is clear; with the "exclusive" flag the request is refused when either already fights
  someone else with more than one adversary. **The victim of a landed or attempted strike receives an attack
  order** on the attacker when he is not a civilian and the sides differ — so the defender enters the stance and
  fights back, a player character included (his auto-fight). The player's attack order (a left click on an
  enemy) walks him within 150 px (a path search if farther), then enters the stance (action 52) and starts the
  auto-fight; the shield posture charges instead (action 154). The fist order from the front is the same
  order with a stun amount (3.11): there is no punch from the front, which is what the oracle saw.
- **AI-161** (observed, 0047dc00; high, not measured). **Turn taking** in a duel: two mutual adversaries share
  an *initiative* flag; the holder attacks; each frame the non-holder acts only with a 10 % chance (one roll);
  the holder hands the turn over with probability (hand-over chance) % per frame (one roll) or when a manoeuvre
  says so.
- **AI-162** (observed, 0047d270, 0047ca50; high). **Disengagement**: an adversary is dropped when he dies,
  is knocked out, moves more than 150 px away, goes out of sight, or changes layer / height by more than 40 px;
  an actor with no adversary left sends himself "quit swordfight" and leaves the stance (the fight idle is
  action 54); soldiers report the AI events 24 / 57 to their side.
- **AI-163** (observed, 0047dc00, 0043db20, 0043ea70; high). **Several attackers**: an actor keeps the list;
  his adversary is the first; a player with more than one adversary switches to another with a 1-in-3 chance
  per decision frame (one roll); NPCs balance targets (AI-142). Hits from several attackers are independent
  messages; nothing serialises them.

#### 3.9.2 Who is struck

- **AI-164** (observed, 00475bd0, 00482fd0, 00482e50, 004801d0, 00482600, 004828b0, 00482390, 00480e90,
  00482b90, 00483070, 00481ef0; high). When a strike's animation reaches its hit frame the attacker collects
  targets by the figure's thrust kind: **straight** (kind 0) → only the current adversary, if minimum ≤ distance
  ≤ maximum reach of the row (re-checked in 3-D); **lateral, half circle, circle** → an arc from the attacker's
  facing: start = facing ± angle word 10, end = start ± angle word 11 (half circle) or ± 180° (circles),
  advancing by angle word 12 per animation step; **every attackable actor within reach and inside the swept
  sector is struck** (civilians excluded, alive, conscious, within 150 px, line of sight) — the sweeping
  figures hit several enemies; **push aside** (kind 2) strikes actors within reach whose bearing is within half
  of the row's word 13 of the facing and displaces them. The **quick strikes** (the click attacks, actions
  59..66, orders that carry "no figure") use the head's default reach (words 0..2) and **no row**: they deal
  neither damage nor stun (AI-166). Each struck target receives a strike message carrying the attacker, the
  attacker's class block and the figure.
- **AI-165** (observed, 00475bd0, 004820e0; high). Rows ↔ sprite actions ↔ orders: row 0 = action 67, 1 = 68,
  2 = 75, 3 = 70, 4 = 69, 5 = 74, 6 = 73, 7 = 72, 8 = 71, row 9 the block (76 / 127 for the held stance); the
  quick strikes are actions 59..66 (the high / low variant by the target's height difference); the figure
  orders are nine consecutive order ids after the four quick-strike orders.

#### 3.9.3 Resolution on the victim

- **AI-166** (observed, 0047e7d0, 005c1b70, 005c1dd0 disassembled, 005c1db0, 00471ed0, 00471e50; high). The
  victim resolves the message in this order, drawing from the global stream:
  1. A lying victim (posture 10 / 11) is affected only by the two finishing-off figures (the AI's execution of a
     sleeper), result "finished"; a victim without a class block (a civilian) takes nothing.
  2. **Defence roll**: chance = the victim's head word by the approach — the approach direction is the victim's
     own facing +4 sixteenths for a left attack, −4 for a right attack, or the actual bearing attacker → victim
     for straight and circling figures; d = (approach − victim facing) mod 16: d ∈ {0, 1, 15} → word 5 (front),
     2..5 → word 8 (right), 6..10 → word 7 (behind), 11..14 → word 6 (left); if the attacker stands ≥ 20 px
     higher → word 4. Roll r = rand%99 + 1; **the blow lands iff r > chance**.
  3. **Damage** = trunc(row word 3 × f), f = 1 + 0.01 × (the attacker's melee experience, difficulty-scaled)
     **only when the attacker is a rank-0 soldier**; otherwise f = 1 (players, officers, knights, civilians).
     The two finishing-off figures deal 1. Neither armour nor weapon material enters. The victim's hit points
     drop by it (the rising number over the head is old − new hit points); at 0 he dies (3.10.3). The attacker
     gains melee experience 20 + max(0, victim rating − attacker rating) for a kill, and after every landed
     figure a player with skill ≤ 100 rolls rand%100 < skill × 0.2 for an experience gain message.
  4. **Stun roll**, independent of the damage: r2 = rand%99 + 1; if r2 > the victim's head word 9 (stun
     resistance) and the row's stun value ≠ 0: stun level += stun × 100 ÷ current hit points (integer
     division) — "the knock-out chance rises when the victim's health is low" is exactly this.
  5. The reaction animation (00474d60): damage only → the flinch 44 (stance 102, bow 114); stunned or
     unconscious → the fall 44 / 107 / 114 then, if still conscious and level > 40, the stagger 103; dead → the
     forward fall 41 when the row has a stun value, else the backward fall 44 (stance / bow twins 105 / 107 and
     112 / 114).

Validation (combat-measurements.md): the hero's 100 hit points, the halberdier's 80 (`p0` × 1 on medium), the
5-hp hits (halberdier jab damage 5 × 1.05 → 5), the occasional larger hit (his finishing blow 20 × 1.05 = 21,
the oracle's 25 was a 5-px bar reading), the click attacks never hurting him (no row), the **town-outfit slow
blow doing 50 for 2 energy pixels (row 1: damage 50, energy 10 of 100)**, the fighting distance 52 (the hero
closes to head word 1 = 50): all agree. "Two of three landing": the hero's defence words are all 0, so every
resolved soldier strike lands; the third that did not was a step the gates skipped or a parade — consistent,
not measured.

- **AI-167** (observed, 00481d40, 00472070; high). **The block** (the right-click defensive stance, action
  127): held while the block animation loops; every frame the actor's fatigue is reduced by 5 (floor 0) and
  **the block ends when fatigue reaches 0** — blocking is resting and cannot be held for ever because it ends
  once the energy is full. Soldiers report it to their side. Its defensive effect on incoming strikes is the
  ordinary defence roll (the block row has no words the resolution reads).

### 3.10 Energy, health, death

- **AI-170** (observed, 00475bd0, 00471b00, 004a1cf0, 0048f9d0; high). **Energy** = 100 − fatigue (the bar's
  20 px are 5 units each). A figure adds its row's energy cost when its animation finishes (parries cost
  nothing). **Recovery**: once per 64 frames (staggered by id, AI-004), while standing still with no adversary,
  fatigue −= min(fatigue, rate ÷ 10) with rate = the player's energy-recovery word (Robin 50 → 5 units, one
  bar pixel per 64 frames = 2.56 s nominal / 3.0 s measured) or the soldier's `q2` (blue halberdier 10 → 1
  unit; 5 units in 320 frames = 12.8 s nominal). Walking to a new position also recovers rate ÷ 10 once. At
  fatigue ≥ 100 the auto-fight stops (the "worn out" state); figures are otherwise never forbidden by energy.
  Validation: the oracle's 1 px per 0.8..1.0 s for the hero disagrees with 64 frames per pixel (3.0 s) unless
  the walk-recovery fired (the oracle's hero moved between strokes) — open (section 6); the soldier's "1 px
  back in ~4 s" agrees with 64 frames (3.0 s) to within the reading.
- **AI-171** (observed, 004d5a90, 00482150, 005c2ec0; high). The HUD rows: red = hit points ÷ maximum (capped
  at 1), blue = (100 − fatigue) ÷ 100; the rising number = the hit-point loss of one strike; stun draws
  nothing but the stars.
- **AI-172** (observed, 00482150, 0049fa20, 00474d60, 00485e90, 004748f0; high). **Death**: hit points 0 →
  the die handler with the killing amount; the actor's state becomes dead; the fall animation per AI-166
  step 5 (an arrow: 40 / 41, bow 111 / 112, stance 104 / 105); the "down" states flip the "body on the ground"
  flag so the body enters every actor's category 1. A story hero with a clover falls into a coma instead (the
  stun floor of 2.7). Health never regenerates on medium and hard (AI-045).

### 3.11 Stun, knock-out and bodies

- **AI-173** (observed, 00475bd0, 00472070, 00471e50, 00471ed0, 00471c00, 00463da0; high). **The punch** (the
  fist order → action 123): a player without the hard-punch ability sends stun 80 (with it 150; × 1.5 on
  hard); an NPC's punch sends 40; the AI's finishing of a sleeper sends 3. The victim applies it **without any
  roll and without consulting his stun resistance**: level += amount × 100 ÷ current hit points (a blue
  halberdier at 80: +100 → unconscious at once; a 250-hp antagonist: +32, several needed). Immune actors ignore
  it. The "from behind" condition is not in the resolution: the order side decides whether a punch or a fight
  starts (AI-160); a punch order on an aware enemy degenerates into the sword fight.
- **AI-174** (observed, 00471c00, 00471b00, 0048e420, 0048d990 read as bytes; high). **Stun level**: clamped
  to 0..300; > 69 → unconscious: all orders dropped, the stars marker (count min(4, level ÷ 50) + 1), the
  adversaries dropped, the AI event "lose consciousness" (3.3), civilians' AI told; < 30 → wakes: the "fit
  again" event, and enemies of the other side are told a fit-again for a player (he is re-noticed from
  scratch: their "visible now" flags for him are reset). **Decay**: −1 per (`t` + 1) frames (65 for regular
  soldiers: from 100 to 29 in about 4600 frames = 3 min nominal; 3 mounted; 10 antagonists; the PC's own word).
  Tied, carried (order state 18) or a player in a coma → floored at 30 (stays out). The console knock-out sets
  the level to 100. A comrade's revive animation subtracts an amount chosen by the AI (not read).
- **AI-175** (observed, 00475bd0, 00472070, 0056c700; medium: the effect functions were not read). Body
  actions are player orders gated by the character's abilities (2.4): search (action 122, or 282 when the
  searcher is in posture 10), tie up, carry (two variants), resuscitate, execute; their preconditions are the
  pointer modes on a valid body (section 6 for the money taken and the tied / carried state transitions).

### 3.12 The bow and other projectiles

- **AI-176** (observed, 0047b760, 0047b8a0, 004df070, 004500a0; high). **Aim validity** (the pointer's green
  tail): the shooter needs ≥ 1 arrow; the target point is the target's body point (lower when crouched); valid
  iff horizontal distance² < range² with range = the ranged record's range 1 (or range 2 when its flag byte
  says so: 250 / 400 for the two bows, 400 for the soldier bow and the crossbow), doubled for the long mode NPC
  archers use when the mission's outfit byte is set; a target higher than the shooter reduces the allowed
  distance by tan(a fixed angle) × height difference; the test also classifies the shot (0 / 1 / 2: below the
  near threshold / beyond / crouched target) and a blocked line of sight (mode 3) forces class 1. The green tail
  requires an actor under the pointer that is not a civilian and not arrow-immune. Validation: the oracle's
  straw target at 186 px from the walkway was refused — targets are not actors (the target element's own
  activation is separate) and the height difference reduces the range: consistent.
- **AI-177** (observed, 0047bb60, 004500c0, 004b4260; high). **Release**: the arrow element is created bound
  to the shooter's ranged record with an elevation constant 0.1 (flat) or 0.9 (lobbed) by the shot posture;
  the initial speed is solved for the target distance; a soldier with a signal animation tells his side
  (event 54). **Hit roll**: the grid (2.6) is A when distance ≤ range 1 else B; the distance becomes a fraction
  of the range in 20 % bands and the chance is the linear interpolation between the two band endpoints of the
  skill row (row 0 if skill ≤ 49 else row 1) and the next row, then over skill 0..100; **rand%100 + 1 > chance
  → miss**: the aim point is jittered by −2..+2 units per axis (two rolls) and the horizontal speed scaled by
  1 − (a record word × a constant). One arrow is removed from the shooter.
- **AI-178** (observed, 004b82e0, 004b7b30, 004859b0, 004a68e0, 004a6b80; high). **Flight and hit**: a
  precomputed list of segments; each frame the arrow moves and tests actors on its path (within 15 px of the
  body point, ignoring the shooter, lying / dead actors, and same-side or player-vs-player hits except on hard);
  the stone (projectile kind 5) can hit a crouched target with the low box. On landing without a hit the arrow
  becomes a ground pickup and emits a kind-0 noise (300 px). On a hit: knights (their flag) always deflect;
  otherwise the target deflects iff rand%101 > its head word 10; a deflected arrow bounces and re-launches;
  else **damage = the ranged record's word 2 (100 / 75 / 10 / 20 by record; the other half's word when the
  arrow's "sharp" flag is clear)**, no stun, no defence roll; a kill gives the shooter 20 bow experience;
  civilians hit tell their AI. Gravities: −5.6 / −6.4 / −4.0 for the arrow kinds, −0.8 / −7.2 for the stone.
- **AI-179** (observed, 0049f520, 0049ec50, 004a74f0; high). **Ammunition**: per player character a counter
  per item kind with a per-kind maximum; at 0 the icon is disabled and a remark plays; soldiers keep one count;
  arrow piles add their stack (the pick-up rule of h01-measurements-2.md).

### 3.13 Purses and wasp nests

- **AI-180** (observed, 004b9b70, 004b9220, 0048cca0; high). A thrown **purse** flies on the projectile code
  (kind 11 sub-kinds); on landing it bursts into coin pickups scattered at random bearings (16 sectors) 10..41
  px away (up to 7 tries per coin for a reachable spot; rolls per coin) and emits a **kind-9 noise (200 px)** at
  the spot; the sees-object / hear path of 3.5.3 makes susceptible soldiers go for it and brawl.
- **AI-181** (observed, 004be0e0, 004be1a0, 004bc660; high). A **wasp nest** spawns 20 wasps on landing, each
  flying at 5 px per frame with random jitter toward its target actor, chasing within 25 px (75 px while the
  target's alert timer runs); a sting raises the wasp event (3.3: the storm cloud, the wasp-in-the-armour
  reaction that blocks everything until wasp-gone); wasps vanish when the nest's counter runs out.

### 3.14 The player's orders and the action queue

- **AI-185** (observed, 0058a3d0, 0058a940, 0058bbe0, 0046bcb0, 00412240, 004de290, 004dce20; medium-high).
  Everything an actor does is a *sequence* of elements (move on a layer, path-move across layers, turn, play
  action, hit / pain, wait) owned by a global manager. A new order first **stops** what runs (the current
  sequence is told to stop; every running sequence of that actor is removed) and appends its own sequence,
  with a stand-up preamble when the actor is lying or kneeling, the move element(s), and an optional turn and
  follow-up action. Queued (Ctrl) orders are elements appended to the same sequence. The player's left click
  is dispatched by the pointer mode (walk / run, the three special-action targets, bow, and further context
  handlers whose mapping to tie / carry / search / revive / finish / pick up / climb / leg-up / pay / mechanism
  was not settled, section 6). The stop reasons seen: 6 script / AI, 7 a cancelled special mode, 8 AI locked.

## 4. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| FRAME_NOMINAL | 40 (46.875 measured) | ms | 0050f710 | high |
| SCRIPT_SECOND | 25 | frames | 004c6ef0 | high |
| RNG | 214013 / 2531011, bits 16..30 | – | 00642a7d | high |
| SIGHT_BASE_RANGE | 400 (lighting 1/8/16/32/64/128), 300 (2/4) | px | 004c1ab0 | high |
| CONE_HALF_ANGLE | 0.5 (wide 1.5208; focused 0.35) | rad | 00486fa0 | high |
| HEAD_TURN_SPEED / LIMIT | 0.3927 / 0.8 (1.3 tracking) | rad per frame / rad | 00486fa0 | high |
| Y_COMPRESSION | 0.5736 (inverse 1.7434) | – | 00486fa0, 0043d780 | high |
| NEAR_OMNI_RADIUS | 60 (front 9/16 of the circle) | px | 00489eb0 | high |
| POINT_BLANK | 20 | px | 00489680 | high |
| FADE | 1 − r/6; 0.95 − 1.75 (r − 0.3); 0.25 − (r − 0.7)/3 | – | 00489d00 | high |
| POSTURE_FACTORS | ×3 (codes 2, 8, 14), ×20 (3, 9), ×2 (4..6), ×1.5 (7, 10, 11), ×0.5 (order state 10) | – | 00489680 | high (codes unnamed) |
| CATEGORY_PERIODS / WEIGHTS | 2 (W·2·g·0.01), 4 (4g'), 8 (8g / 24g bodies), 16 (16g) | frames | 00488f60 | high |
| SHADOW_THRESHOLD / EMIT_THRESHOLD | 100 / 1000 | accumulator | 00488b80 | high |
| FORGET_RATE | −1 per 20 frames | – | 00487d00 | high |
| NOISE_RADII | 300 / 70 / 50 / 500 / 200 / 400 by kind (AI-071) | px | 0048cc20 | high |
| WALK_NOISE | 15 / 20..40 (200 on material 5) / 70..150 (400) / 200 melee (AI-072) | px | 0047ef70 | high |
| DEAFNESS_DECAY | 10 per frame (< 301), else 50 × trunc(v/300) | px | 0048a830 | high |
| IDENTIFY_FACTOR | 1.5 (1.95 in order state 16) × 1.3 easy / 0.7 hard | × base range | 004a0a30 | high |
| LOS_CACHE | 2000 slots per frame | – | 004eff80 | high |
| STAGGERS | think 16, identification 16, hearing 3, energy 64 | frames | 0048a980, 00471b00 | high |
| REACTION_DELAY | trunc((100 − p1)/100 × base × m + 1), m = 2 easy/hard, ≈0 medium; friendly 3 | frames | 00438620 | high |
| LOOK_INTERVAL | 70 + rand%70; officers 200 + rand%600; p4 ≠ 0 400 + rand%800 | frames | 0041a420 | high |
| SHOUT_RADIUS | 100 (officer's arrow 200) | px | 00439cb0 | high |
| ELEVATION_LIMIT | 100 | px | 00433060 | high |
| SEEN_CLEARLY_DEPTH | 50 | px | 00433060 | high |
| SIGHT_REACTION | 5 seen clearly / 20 turning / 10 rider; < 30 px immediate | frames / px | 00433060 | high |
| NOISE_REACTIONS | 50 quiet; 60 ×m steps; 60 whistle watch, 50 ×m; 70 + rand%60 kinds 11/12 | frames | 00433830 | high |
| BODY / OBJECT / ARROW REACTIONS | 80 ×m; 60 ×m ale, 30 purse (60 officer); 100 ×m (officer 60) | frames | 00434320, 00434710, 00433f90 | high |
| LOOK_THERE_WATCH | 100 | frames | 00439e70 | high |
| LOOK_AROUND_LEG | 30 + rand&7, turn +5/16 | frames | 00424d50 | high |
| SEEK_WATCH / WALK_LEG | 20 per direction / 200 | frames | 00424d50 | high |
| SEEK_COOLDOWN | 5000 per visit, cap 10000 ahead | frames | 00522330 | high |
| SEARCH_RADII | 300 usual, 400 missing colleague (200 with a patrol), 500 neighbour test | px | 0043a220, 0043cec0 | high |
| REPORT_DISTANCE / OFFICER_COST | 70 px / distance + 100 per layer (civilians 250) | px | 0043c960, 0040e290 | high |
| OFFICER_PATIENCE | 101 polls × 30 | frames | 00424d50 | high |
| BRAWL_FINISH / STARE | 200 / 300 + rand%32 | frames | 00424d50 | high |
| CHECK_FOR | skip if alert < 3000 frames old; looks = N/10 + 1; interval 1000/looks; give up at acc > 1000 | – | 00439080 | high |
| PATROL_WAIT | 200 (chief and followers) | frames | 00424d50 | high |
| ENGAGEMENT_DISTANCE | 150 (head word 3) | px | 0047c600 | high |
| HEIGHT_TOLERANCE | 40 | px | 0047c600 | high |
| FIGHT_STEP | 20 | frames | 0043db20 | high |
| MANOEUVRE_CHANCE | 1/3 | – | 0043db20 | high |
| ATTACK_CHANCE | max(skill, 50) % | – | 00485240 | high |
| FIGURE_SKILL_MIN | 0, 40, 95, 20, 20, 70, 70, 80, 80 | – | 00485240 | high |
| OBSERVE_RULE | (courage × 0.045 + 1) × n ≤ E | – | 004303e0 | high |
| MORALE | AI-141 | – | 00432d30 | high |
| INITIATIVE_CHANCE / TURN | 10 % per frame | – | 0047dc00 | high |
| DEFENCE_ROLL / STUN_ROLL | rand%99 + 1 > word | – | 0047e7d0 | high |
| DAMAGE_FACTOR | 1 + 0.01 × experience (rank-0 soldiers) | – | 005c1dd0 | high |
| STUN_GAIN | stun × 100 / current hp | – | 00471ed0 | high |
| STUN_THRESHOLDS | ≥ 70 out, < 30 awake, cap 300, floor 30 tied / carried / coma | – | 00471c00 | high |
| STUN_DECAY | 1 per (t + 1) frames | – | 00471b00 | high |
| PUNCH_STUN | 80 / 150 hard punch / 40 NPC / 3 execution; × 1.5 hard | – | 00475bd0 | high |
| ENERGY_MAX / RECOVERY | 100 / rate ÷ 10 per 64 frames | units | 00471b00 | high |
| BLOCK_REST | −5 fatigue per frame, ends at 0 | – | 00481d40 | high |
| STARS | min(4, stun ÷ 50) + 1 | – | 005c3fc0 | high |
| BOW_HIT | grid interpolation, rand%100 + 1 > chance | – | 004500c0, 0047bb60 | high |
| ARROW_HITBOX / DEFLECT | 15 px / rand%101 > word 10 | – | 004b7b30, 004a68e0 | high |
| AIM_DELAY (archers) | (110 − ranged skill) ÷ 2 | frames | 00424d50 | high |
| PURSE_SCATTER | 10..41 px, 16 bearings, 7 tries | – | 004b9220 | high |
| WASPS | 20 per nest, 5 px/frame, chase 25 (75) | – | 004be0e0, 004bc660 | high |
| DIFFICULTY | AI-045 | – | 00438710 | high |

## 5. Interfaces to the script VM

### 5.1 Natives (ids as in scb.md; semantics observed unless marked)

| Id | Arguments → result | Semantics | Source |
|---|---|---|---|
| 85 | (x) → bool | x is null (no actor) | natives table |
| 87 | (actor) → bool | dead | virtual |
| 88 | (actor) → bool | unconscious | 00471c00's flag |
| 89 | (actor) → bool | tied (current action id 18) | |
| 90 | (actor) → bool | dead or unconscious or tied | |
| 99 | (actor) | reveal the silhouette (AI-074) | 00570e70 |
| 102 | (actor, amount, flag) | append a hit element with `amount` (and a second parameter 100 when flag ≠ 0, else 0; its meaning not settled) | 005861c0 unread |
| 126 | (npc) → int | the top-level state renumbered for scripts: 1 default, 2 wondering, 3 seeking, **4 fleeing, 5 sleeping, 6 attacking** | 00575e20 |
| 128 | (npc) → bool | **is hostile** (the profile's attitude / side), not "can act" | |
| 130 | (npc, target, flag) | a no-op in this build (empty body) | 00486f50 |
| 134 / 135 | (actor, flag) / (actor) | lock the AI: locked, the actor stopped (reason 8) unless idle, the program cleared; a player character gets a lock byte. Unlock: only when the "out" flag is set (error otherwise); clears the lock and, unless knocked out or in a company sequence, posts return-to-duty | |
| 140 | (npc, k) | writes the walking-flags word (AI-093): 0 walk, 1 run, others verbatim; re-issues the current rail leg | 0041bd20 |
| 160 | (a, b) → px | distance | |
| 176 | (npc, n) | set the company number (2.10) | |
| 177 | (npc, flag) | "always attentive": when set and no target, perception is re-run at once | 00444440 |
| 197 / 198 | (npc, k) → v / (npc, k, v) | get / set one of ten custom values per NPC (loaded from the record / save), **not** visibility | |
| 218 / 219 | (chief, npc) / (chief) | add a subordinate / remove all subordinates (AI-097) | |
| 220 | (npc) | switch to the alert rail (2.10) and reset the default state | 0044cae0 |
| 228 | (npc, id, frames) | set the emoticon: 0 clears; 1..7 → the markers 2..8 of AI-084 for `frames` frames | 0057aed0 |
| 235 | (item) → bool | picked up (unchanged) | |
| 240 | (actor) → bool | active | |
| 59 | … | records a play-animation step in a cinematic sequence, **not** "archer shoots" | |

scb.md's readings of 128 ("able to act"), 140 ("patrol mode"), 177 ("at post"), 197 / 198 ("visibility"),
219 / 220 ("alert / clear") and 228 ("timed guard state") are superseded by the above. The complete id →
function map is in the analyst workspace (`re/notes/ai/natives-map.tsv`).

### 5.2 Callbacks

- **AI-190** (observed, 00403190, 004032a0, 00408750, 00410620; high). `ActionChange(actor, id)` on every
  change of an actor's action id; `ReachPoint(actor)` when a script-flagged waypoint is reached (AI-091);
  `FilterAIEvent(actor, event)` for actors flagged for scripting, before the pre-filter, with the event id
  **renumbered**: internal 0..7 unchanged; internal 9..35 → id − 1; 36 → 36; 37..45 → id + 2; 46 → 37; 47 → 38;
  48 → 35; 49..53 → id − 1; internal 8 ("shot at by a player") is never offered. A zero return drops the event.
  (So the ids the scripts compare — 0, 2, 8, 11, 13, 14, 22, 23, 31, 33, 34, 52 — are: 0 view, 2 hear, 8 sees
  body, 11 sees soldier, 13 got hit, 14 loses consciousness, 22 quits a sword fight, 23 an incoming strike,
  31 arrow landed, 33 alert call, 34 combat-alert call, 52 sees shadow.)

## 6. Open questions

1. The swing cadence: which of AI-150's gates dominates the measured 5.3 s (trace 0043db20 / 00485240 with the
   transition log 00422b70; the 30 px reaction constant at 0x6774e0 and the target-state gate 7..11).
2. The energy recovery of the hero (64 frames per unit-pixel versus 0.8..1.0 s measured): the walk-recovery
   branch of 00471b00 (cases 0xb / 0xc of 00475bd0) and whether a stroke's walk-up triggers it.
3. The mission's "outfit" byte at engine +0x4aa8 (read at 004c38d0): which level field sets it; it selects the
   hero's record (and class), the conspicuousness word, the long bow mode and friendly-side rules.
4. The posture codes 2..14 of AI-066 (setter 0046bd10, callers in 00475bd0 and 0047ac90) — which is running,
   sprinting, fighting, crouched; sprite actions 242 / 246 / 247 versus the documented hidden-in-leaves ids
   136 / 137 (00488f60); the range × 1.4 virtual (slot 0xf4 of the NPC vtable) and the order states 10, 16, 19,
   21..23 used as gates.
5. Whether the drawn cone fades with the graded value (0058e7f0), closing the 78..86 % reach discrepancy; the
   courtyard's ground material (the walk-noise radius that explains the run heard at ≥ 330 px).
6. The seek-point list's origin (the mission chunk feeding the global list read by 0043a220); 0043d0c0
   (visit bookkeeping), 00438600 / 00416800 (the search budget), 00419970 (remark timer).
7. The officer's tower-guard alert (0043b1b0, 4 KB) and 0043c1a0; 00444bb0 (colleague found); the menacing
   state's timers (case 0xd5 of 00424d50); 0042c9a0 (the beggar's betrayal roll); the civilian attitude
   branches 0040d440 / 0040d690.
8. The body actions' effects: money taken by a search, the tied flag and the carried state (00475bd0 cases for
   actions 122 / 282 / 169; the player status's money field); the reviver's stun subtraction.
9. The pointer-mode → order mapping of the context actions (004e1a40 and the handlers 004df070..004e1630),
   the right-click cancel (callers of 0046bcb0 in 004cd400 / 004de840 / 004defa0), the Ctrl queueing, and the
   **gesture recogniser** (a stroke → one of the nine figure orders; not in the actor code; look in the input
   layer near 004dd410 / 004de290).
10. The ranged record used by each player character (0048fa30 → 0047ab00's third argument) and the arrow
    "sharp" flag / the record's flag byte; the lost float in 004500c0 (the normalised distance); the hard
    punch's scaling expression in 00475bd0.
11. Native 102's second parameter (005861c0 and its executor); the writer of the "no player death" option byte.
12. SD `w0` beyond the experience formula, `q3`, `q4`, byte +0x5d, flag bit 5 (0049b510), class head words 10
    (deflection is read: confirm) and 14, ranged words 2 / 0x2a: consumers not found (unresolved virtual slots
    of the vtables at 0x677ff4 / 0x677dfc).
13. The "wake now" flag, the initial 300-frame knock-out request (00414550), and the console's "alert all NPCs".

## 7. Differences from the current engine (`crates/opensherwood-core/src/ai.rs`, stealth-and-combat.md)

1. **Clock**: the engine's world tick is 1/60 s with animation frames of 3 clocks at 64 Hz; the original's
   logic frame *is* the 46.875 ms frame (25 nominal); every AI timer, the script second (25 frames) and the
   sprite tick half count these frames. Rebase the tick.
2. **Perception model**: the engine tests a sector of half-angle 39.4° with reach 270 × 0.72 and a rear
   radius of 50 px, deterministically per tick. The original: half-angle 28.6° world (0.5 rad) with the y
   metric × 1.7434, base range 400 / 300 by level lighting, a head that turns independently (glances, tracking),
   a **graded value with posture factors**, the 60 px front-9/16 rule (not a rear radius), light-limited reach
   in dark levels, **line of sight through the level's sight polygons**, and an **accumulator**: an unalerted
   soldier gets a "?" at 100 and a sighting at 1000 (a standing hero at half range never accumulates), while a
   soldier who has the player in the immediate category (after a noise, a search or a check-for) sees him at
   once. `Assumption::SightCone` is replaced by this rule.
3. **Hearing**: the engine hears a run at 350 px from anyone; the original has per-action, per-ground-material
   radii (walk 20..40, run start / sprint 70..150, 400 on material 5, melee 200) plus one-shot noises by kind,
   a deafness value, and hearing staggered every 3 frames; civilians hear too.
4. **Alert states**: the engine's patrol → noticed (141) → alarm (142) → alerted → returning is replaced by
   seven top states with sub-states, reaction delays from the profile's initiative (1 frame on medium), the
   look-around legs, the seek-point search with freshness and cooling, the officer / group / civilian
   reporting, the calls with their radii, the stimuli (purse, ale, apple, whistle, wasp, net) and the brawl.
   There is **no mission-wide alarm and no 300-tick alert timeout**; a soldier returns to duty when his search
   list is exhausted.
5. **Patrol rails**: the engine reads 0x04 as 1/100 s and 0x0d as a check-for with a radius; the original
   waits in frames, 0x0d is a bend, 0x05 / 0x06 are the check-fors (a look budget, the colleague's path), the
   rail is walked back and forth, tables are direction-selected with a cached roll, 0x09 / 0x0a select the
   sprint cycle 10 (not the run 7).
6. **Combat start and count**: the engine fights one player at a time and never lets soldiers start a fight;
   the original's soldiers attack on their own decisions, several at once (the observe rule), with the morale
   check deciding retreat, and the victim of any strike auto-engages.
7. **Damage rule**: the engine's fixed 5-hp soldier hit at 2/3 and the 50-hp forward stroke at 1/3 become the
   class rows: damage word × (1 + experience/100 for rank-0 soldiers), a defence roll against the victim's
   directional words (the hero's are 0: every soldier blow lands), an independent stun roll, sweeping figures
   hitting several targets, quick click attacks dealing nothing, and **the hero's rows depending on the outfit
   record** (the town outfit's slow blow: 50 damage, 10 energy).
8. **Energy and health**: 100 units (not 20) recovered rate ÷ 10 per 64 frames while standing still (not a
   per-side fixed period); figure costs from the rows; the block is resting; hit points regenerate only on
   easy; the maximum from `p0` scaled by difficulty.
9. **Knock-out**: the engine's 600-tick timer scaled by `p4` becomes the stun level (punch 80 / 150 / 40 ×
   100 ÷ hp, no resistance roll for the punch; strikes roll against class head word 9), unconscious ≥ 70,
   awake < 30, decay one point per `t + 1` frames, stars by level ÷ 50; `p4` is seniority, not resistance;
   tie / carry / coma floor the level at 30; native 90 = dead or unconscious or tied.
10. **Natives and profile fields**: 126 / 128 / 140 / 177 / 197 / 198 / 219 / 220 / 228 / 59 mean what section 5
    says; `p1`..`p4`, `q0`..`q2`, the four stimulus words, `t`, the class rows and the ranged records mean what
    section 2 says; table B is the ranged weapon table, not difficulty; the flags' bits 6..7 are junk.
11. Also: the RNG is one global stream seeded by the wall clock and re-seeded at every save (the engine's
    named streams are our own determinism device, which is fine, but the *order* of rolls per actor per frame
    in sections 3.5..3.12 must be honoured to match a trace); identification is 600 px with line of sight, not
    120; the bow's validity, hit grid, damage by record and deflection replace the unimplemented bow.

## 8. Provenance

Ghidra project `re/ghidra/robinhood` (never committed), full decompilation exported to `re/out/decomp_all/`
by the generic scripts in `scripts/ghidra/`, strings and inventory exports, byte reads with
`scripts/ghidra/peek.py`, and capstone disassembly of six functions the decompiler left undefined or whose
float bodies it dropped (005c1dd0, 00438620, 0048d990, 0048e610, 004a1e90, and vtable getters). Analyst
session 2026-09-13 with six parallel sub-sessions whose notes are `re/notes/ai/{profile-fields, perception,
state-machine, attacking, rails-orders-natives, combat, clock-and-rng}.md` and the id maps
`state-ids.txt`, `substate_ids.txt`, `natives-map.tsv`. Data checks on the player's `profile.cpf` with
`harness/tools/probe/cpf_stats.py` and `cpf_probe.py` (all 68 SD, 10 PC, 24 CV records, all 27 class blocks,
the 4 ranged records). Oracle recordings compared: `combat-measurements.md`, `h01-measurements-2.md`,
`stealth-and-combat.md` 8 (2026-09-05 sessions; no new recording was made). Tests that will depend on this
spec: the AI / combat rulesets rebuilt from it (none yet).

Functions read, by area (why): **clock / RNG** 004c6ef0 0050f710 00404180 00642a7d 00642a70 004148b0 0040a230
(frame counter, pacing, seeding). **Profile loading and fields** 00564580 00564e60 00564bf0 00564ea0 00565490
0056a7d0 00568e00 00569f40 00565100 00565c10 0056aee0 00569570 005672d0 0056c700 0056c290 0056c360 0056c5b0
0056c100 0056cab0 0056c410 0056c490 00436500 005644c0 0056d5d0 0056d2a0 0048f280 0048fa30 0047ab00 005c1b20
00450040 00438710 00438600 00438cb0 0041a420 004468a0 0044cb40 004a1cc0 004a1cf0 004a1d00 004a5ba0 004a5d30
0048f910 0048f9b0 0048f9c0 0048f9d0 0049d5e0 0049d560 0049d520 0055bc90 0055bf00 0055dbb0 0055dea0 0050b640
(layouts, consumers, difficulty). **Perception** 004863f0 00486fa0 00486f30 00486f60 00486f70 00487aa0
00487ad0 00487b70 00487b90 00487d00 00488f60 00488870 00488e00 00488b80 004887a0 00488a40 00489680 00489eb0
00489d00 00489b30 0048db80 0048a130 0048a260 0048a350 0048a830 0048a980 0048c700 0048c8e0 0048c4d0 0048c5b0
0048c620 0048f110 0048e420 0048cca0 0048cc20 0048cec0 0046f290 0046ec50 0047ef70 004a0a30 00570e70 004c1ab0
0048ebe0 0057ae60 004eff80 004eef30 004f0230 004ef340 005a3810 0058e7f0 005b3c40 005b3c20 00471b00 00471c00
004936f0 0045db80 0045dce0 0045e6f0 0045e7e0 004d4300 004d5320 (cone, values, events, hearing, identification).
**Event system and the state machine** 00424bc0 00424d50 0042c9c0 0042e450 00422c40 00422b70 00410620
0040d980 00416710 0048b450 0048b4e0 00419fb0 00419fd0 00419970 0057aed0 00434aa0 0041c130 0041f540 004207f0
00420d40 00418c40 00414550 00414be0 00424540 00435c80 00430300 00438620 0044d750 00433060 00439cb0 00433830
00434320 00434710 00433790 00433f90 00439e70 00439f60 0043a080 004457c0 0043d890 00445d40 004460f0 0044b000
00446400 004469e0 004468a0 00446d20 00449df0 0044cb40 0040e7e0 00443d10 00443af0 00443c40 004436e0 00443850
0043a220 0043d0c0 0043d530 00522330 00416800 00438600 0043c960 0043ccc0 0043cec0 0044cf00 0044a760 0044a830
004444a0 00444bb0 0044dc80 00449170 00413240 0040bfd0 0040c140 0040cc60 0040d150 0040d440 0040d690 0040e150
0040e290 0040f090 00416b30 (states, transitions, timers, calls, stimuli, civilians). **Attacking** 004303e0
00432d30 00438cb0 0043d780 0043db20 0042ef80 0042fe00 00432f80 00430020 00441650 00442190 00442470 00447070
00447e50 0044dd70 0044ca90 0044c850 0044a090 00445210 0044b4e0 00445510 00445180 004474e0 00447490 00447640
0043ea70 00440cf0 00441020 00485240 00485590 00482240 00482fd0 00482e50 004820e0 0044b940 0044bc70 00449310
0044aa70 0044d400 0044d6c0 004a55b0 004a5610 00487aa0 004a5460 (decisions, morale, targets, the fight step,
figure choice, tactics). **Rails, orders, natives** 00410010 00410de0 004121a0 00419cc0 00419c50 00419c80
005505b0 005505c0 00550640 00550660 00550f60 00412c80 00412cf0 00412240 00412f70 00418600 00410600 0041a960
0041ac50 0041b0e0 00412d60 00439080 0041bd20 00444440 0044cae0 00486f50 004a1e90 0048b5d0 004c1df0 0054e750
0054e9a0 005866a0 00584d60 0058a3d0 0058a940 0058bbe0 00582530 0046bcb0 004de290 004de1e0 004dce20 004dcfd0
00403190 004032a0 00408750 and the native bodies 00575e20 00575f00 00575fc0 00576d80 00576e00 005780d0
005790b0 005790f0 0057acc0 0057ad30 0057aa70 00577500 00579a70 00579b60 00570a50 00570d40 00570a70 00579bd0
00579470 005794b0 00579520 00571b30 00418b80 00418bc0 (opcodes, control flow, company, sequences, natives).
**Combat** 0047c600 0047c4c0 0047d270 0047ca50 0047dc00 0047e1e0 0047e7d0 005c1b70 005c1dd0 005c1db0
005c1d70 005c1b50 005c1d50 005c1e80 005c1ea0 005c1ed0 005c1f10 005c1f50 005c1f90 005c1fb0 00471ed0 00471e50
00471c00 00471b00 00482150 004d5a90 005c3fc0 005c2ec0 00472070 00475bd0 0047f520 004801d0 004808f0 00480e90
00481050 00481ef0 00482600 004828b0 00482b90 00482390 00483070 00474d60 004748f0 00474ad0 00483c90 00483d90
00481d40 00463da0 0048d990 0048e610 004a5a00 0047e9a0 0049fa20 00485e90 004e39b0 0051d5d0 0051d4e0 0051e8e0
0047b760 0047b8a0 004df070 0047bb60 004500c0 004500a0 00450070 004502e0 004b82e0 004b7b30 004b7040 004b8230
004b44b0 004a68e0 004a18c0 004859b0 004a6b80 004a66f0 004b9220 004b97c0 004b9b70 004be0e0 004be1a0 004bc660
004a74f0 0049f520 0049ec50 0045e2e0 (engagement, resolution, energy, death, stun, bow, projectiles, purses,
wasps, console cheats).
