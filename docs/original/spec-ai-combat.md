# AI and combat (behaviour specification)

Status: `draft`, revision 4 (answers Codex review 27 of revision 3; revisions 2 and 3 answered reviews 16 and
22; awaiting clearance). Build: GOG English edition, `Robin Hood.exe` SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base `0x00400000`; every address
below is a virtual address in that image. Analyst: 2026-09-13 and 2026-09-18, session `a2931a3a5b130742c`
(analyst role, ADR-0009). Reviewer: Codex `gpt-6-astra`, reviewer session
`01a0b406-a088-76b0-b80e-f5d4a5abd73d` (spec-reviewer role only); reviews 16, 22 and 27 are committed as
`docs/decisions/reviews/2026-09-13-codex-review-16-spec-ai-combat.md`, `...2026-09-13-codex-review-22-...`
and `...2026-09-18-codex-review-27-spec-ai-combat.md`; review 22 examined revision 2 at commit `e966b05`
(blob `e10208494c95caf92c2b356357329a61cf014c13`), review 27 examined revision 3 at commit `e7b2cd4`
(`e7b2cd45ec1dd170101be64e68f2d5517efbdbfd`, blob `c9040a2a406e8525fbd6aae32241b81fe0c72a6c`). Publication
approval: pending (separate from factual approval).

Sibling specifications this file depends on, pinned: `spec-script-vm.md` revision 3 (commit `25b6dbe`),
`spec-navigation.md` revision 2 (commit `e5a2e0c`), `spec-movement-animation-camera.md` revision 2 (commit
`b0cd053`). The engine's tick is decided by `docs/decisions/ADR-0010-logic-frame.md` (one logic frame of
46.875 ms drives everything); this file's frame counts are unchanged by that choice (3.1, section 8).

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text, and no prescribed internal
structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer chooses
the organisation.

Claim ids are `AI-nnn` (section 4 is the registry; the prose cites them). Status is `observed` (read in the
program's code at the named address), `inferred` (the only reading that fits every branch, or concluded from
several observed facts), `unknown`. Confidence is high unless stated. Sibling specifications:
`spec-script-vm.md` (`VM-nnn`: the level tick, natives, sequences), `spec-navigation.md` (`NAV-nnn`: layers,
path finding, the walk pipeline), `spec-movement-animation-camera.md` (`ANIM-nnn`: the animation player, the
frame), `spec-campaign-camp-saves.md`. "Frame" means one logic frame (3.1); "px" map pixels; "profile" a record
of `Configuration/profile.cpf` ([../formats/profile.md](../formats/profile.md)); "the roll" one draw of the
program's random stream (2.2).

## Identity and exposure

- **Analyst**: session `a2931a3a5b130742c`, 2026-09-13, analyst role under ADR-0009. It has read decompiled
  code of the AI (perception, event handling, the state machine, decisions, the patrol interpreter), of the
  human actor's combat, stun and health code, of the bow and projectile code, of the profile loaders and of the
  clock and random-number helpers. It must not implement any of them, and no implementer session may inherit
  its context, notes or tool output.
- **Delegated readers**: six forked sub-sessions of session `a2931a3a5b130742c` (same role, same
  workspace), one per area, each of which read the decompiled functions of its area and reported to the
  analyst; their notes are `re/notes/ai/*.md` (git-ignored). **Identity record**: the forks were launched on
  2026-09-13 as six background tasks of session `a2931a3a5b130742c`; each has a task identifier and a
  transcript file in that session's task records (the harness's `tasks/` directory of the session scratchpad,
  six entries, in the launch order profile fields, perception, state machine, attacking, rails / orders /
  natives, combat), which the maintainer can recover from the session; the identifiers are the harness's
  internal ids and are **not reproduced here** (the harness marks them as not for user-facing text) — that is
  the retained identity gap. They are bound below to their exposure by area and note file: (1) profile fields
  — `profile-fields.md`, exposure: the profile loaders, the
  difficulty scaler and the stat consumers; (2) perception — `perception.md`, exposure: the cone, the scan,
  hearing, line of sight, identification; (3) the non-attacking state machine — `state-machine.md`, exposure:
  the event dispatchers, the default / wondering / seeking / sleeping / fleeing handlers, the civilian AI;
  (4) the attacking branch — `attacking.md`, `substate_ids.txt`, exposure: decisions, morale, targets, the
  sword-fight step, the figure chooser, tactics; (5) rails, orders and natives — `rails-orders-natives.md`,
  `state-ids.txt`, `natives-map.tsv`, exposure: the rail interpreter, the order pipeline, the natives and
  callbacks; (6) combat mechanics — `combat.md`, exposure: engagement, resolution, stun, energy, the bow,
  projectiles, purses, wasps. Their exposure counts as this session's; their raw output never entered the
  repository and their wording is not reproduced here.
- **Helpers exposed**: the export scripts in `scripts/ghidra/` (generic), `scripts/ghidra/peek.py` (byte reads),
  capstone disassembly of eight functions the decompiler exported incompletely (00438620, 005c1dd0, 004a18c0,
  0048d990, 0048e610, 004a1e90, 004500c0's float tail, 00475bd0's punch scaling): the disassembly stays in `re/`.
- **Spec reviewer**: Codex `gpt-6-astra`, reviewer session `01a0b406-a088-76b0-b80e-f5d4a5abd73d`
  (spec-reviewer role only), `cross-agent-review` task B: review 16 of revision 1 (28 findings, verdict redo),
  review 22 of revision 2 at `e966b05` (16 findings, fix-then-clear; it upheld revision 2's two disputes and
  cleared the layouts, stimulus meanings, category lifetimes, hearing corrections, clock distinction,
  difficulty, rail double advancement, mutual-duel gate and the recovery / stun / deflection boundaries) and
  review 27 of revision 3 at `e7b2cd4` (13 findings, fix-then-clear; it cleared the event-to-callback mapping,
  AI-100, natives 134 / 135, AI-170's out-of-combat recovery, AI-173's scaling, AI-174's residual phase and
  AI-176's geometric rules subject to the arithmetic contract). The three review documents are committed at
  the paths named in the header. A reviewer may read `re/`; its output is corrections to this file only.
- **Implementation reviewer**: pending, must be a session that has never read `re/`.
- **Publication approval**: pending, separate from factual approval.

## 0. Necessity record

- **Interoperability target.** Playing the player's own missions with the player's own data: the character
  profiles (`Configuration/profile.cpf`: hit points, skills, susceptibilities, the melee class blocks and the
  ranged records), the mission files (`.rhm`: actor placements with company, chief, patrol and alert rails,
  the rail programs, seek points, sight polygons of the proto-level), the animation tables (`.rhs`: which
  action is a strike, a glance, a fall) and the compiled scripts (`.scb`: the natives that lock, alert, query
  and re-route the AI, and the three callbacks the engine invokes on actor classes).
- **Information not otherwise available.** `docs/original/stealth-and-combat.md` section 7 listed eight
  questions that data observation could not settle (the sight cone, the detection thresholds and their delays,
  the knock-out rules, the melee figures' effects, the bow, the script state codes, the rail command semantics,
  the tick model); its section 8 and `combat-measurements.md` section 7 measured a few of them on one soldier
  and one hero and left the rest "not measured" (knock-out, the other eight figures, several guards, the bow,
  the view cone of any other profile); `h01-measurements-2.md` section 9 lists the identification radius, the
  purse amounts, the bow's range and the training-target shot as unmeasured; `docs/formats/profile.md` marked
  every numeric profile field `unknown_*` with hypotheses; `docs/formats/scb.md` gave the AI-related natives
  "low" confidence readings; the current engine (`crates/opensherwood-core/src/ai.rs`) records eleven
  `Assumption` variants for the rules of this subsystem. All of that lives only in the executable.
- **Scope read.** About 330 functions (section 10 lists them by area with the reason), in the address ranges
  0040bf00..00450c50 (the AI classes), 00463000..004c0500 (actor, projectile and object classes),
  004c1a00..004c7000 (level tick, sight range), 004d4000..004e2000 (order dispatch, noise display),
  004ebb00..004f0300 (grid, line of sight), 0050f700 (main loop), 0051c000..0051f000 (status), 00564000..
  0056d600 (profile loaders), 00570a00..0057c600 (the AI-related natives only), 005a3800 (sight obstacles),
  005b3c00 (loud-sound geometry), 005c1b00..005c4000 (weapon rows, titbits), 00642a70..00642a9f (random).
  Stopping condition: every rule an implementer needs to run the first mission's soldiers, civilians and the
  hero's melee and bow against their data was read to its data source and its consumers; section 9 lists the
  behaviours that were **not** read (menacing timers, the officer's tower-guard routine, the colleague-found
  handler, the beggar's betrayal roll, the civilian attitude branches, the body actions' effects, the gesture
  recogniser, order cancellation and queueing, the training-target shot). Those are listed as exclusions with
  their hand-off boundary (9.2), not as settled; whether any of them decides a mission's win path is not
  asserted.
- **Analyst authorisation**: on behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

Covered: the logic clock and the random stream the rules depend on; the meaning of every numeric profile field
and of the two weapon tables; perception (view cone, line of sight, hearing, identification); the AI event
system, its timers and the state machine of hostile, friendly and civilian non-player characters as far as read
(patrol, curiosity, search, reporting, attack decisions, the sword fight, fleeing, sleeping); the patrol program
interpreter (every rail opcode); combat on both sides (engagement, reach, strikes, defence, damage, stun and
knock-out, energy, health, death, the bow and other projectiles, thrown purses and wasp nests); difficulty; the
script natives and callbacks that touch these.

Inputs from other subsystems: positions, layers and heights of actors and their current sprite action
(`ANIM`, `NAV`); path searches (`NAV`, requested, not described); the sight-obstacle polygons and light
elements of the level (`docs/formats/rhp.md`); the animation tables (a strike's hit frame and the displacement
of a sequence come from them); the script VM (natives and callbacks, `VM`). Outputs: actor orders (walk, run,
turn, play action, strike), emoticons and remark kinds, the HUD bar quantities, events to the script VM.

**Mission readiness.** This specification does not by itself make a mission playable: the behaviours listed
as excluded in section 0 and in 9.2 (menacing, the tower guard, colleague-found, beggars, civilian attitude
branches, body actions, the gesture recogniser, order cancellation / queueing, the shot at a training target)
must be specified or explicitly stubbed, and the whole must be integrated with navigation, the scripts and the
victory conditions and demonstrated by a canonical-input completion trace of at least the first mission. Until
then the claim of this file is "the rules read are these", not "the mission is covered".

## 2. Data model

### 2.1 Units, clocks

- Positions are map pixels (px) as single-precision floats with a height (z) and a layer; distances used by
  the AI weight the y difference by 1.7434 (AI-064), except where stated (the hearing test is euclidean in
  3-D, AI-071).
- Time is counted in **logic frames** of the global frame counter (u32, incremented once per frame, AI-001).
  Every timer, delay and stagger below is in frames. The nominal frame is 40 ms (25 per second); on the host
  the oracle measured, the realised frame is 46.875 ms (3.1). Conversions to seconds below use 40 ms and say so.
- Percentages are integers 0..100 unless a formula says otherwise; "roll" is `r = rand()` with r in 0..32767
  (2.2) and `r % n` its remainder.
- **Arithmetic contract** (AI-008, observed on the float expressions cited, disassembled where the export
  dropped them). (a) Operands and constants: every float quantity read from memory (positions, profile-derived
  values, the constants 0.01, 1.33, 0.3, 20, 200, the stored perception value, the stored reach) is an IEEE
  single-precision number; 0.01 is 0.0099999997764825821, 0.3 is 0.30000001192092896. (b) Intermediates: an
  expression is evaluated left to right in the order the text gives, with intermediates in the x87 extended
  format (64-bit mantissa) **until a value is stored**; (c) **rounding boundaries**: a value is rounded to
  single (round to nearest, ties to even) exactly where the text says "stored" — the perception value of an
  entry (AI-068), and in the aim test the scaled y displacement, the tangent, the reach and the squared reach
  (AI-176) — and the comparison or the next operation then reads the stored single; (d) "trunc" converts an
  extended value to an integer by truncation toward zero. **What is demonstrated**: the fixtures of section 7
  (the 15 / 159 contributions, the 54 / 108 / 60 delays, the aim equality case) were computed under (a)..(d)
  and match the instructions read; general bit-equivalence of every float path with the original is **not**
  demonstrated and is not claimed. OpenSherwood's chosen arithmetic (section 8 item 4: `f32` storage at the
  boundaries of (c), `f64` intermediates) reproduces those fixtures; behaviour that depends on other rounding
  boundaries than the ones listed is withheld from clearance until a fixture settles it (9.2 item 18).

### 2.2 The random stream

One global stream (AI-005): state × 214013 + 2531011 (mod 2³²), result = (state >> 16) & 0x7fff. It is seeded
from the wall clock at program start and again at every save (the seed is written into the save and reapplied
on load, AI-006). Every draw of the AI, the actors, the sounds and the effects comes from it; the per-frame
order is the actor-list order, and within an actor the order the rules below list. Section 8 states what this
means for OpenSherwood's named streams.

### 2.3 Sides, kinds, ranks, the action enumeration

- **Side** (AI-010): hostile or friendly; a soldier's side is its profile's flag bit 0 (2.4), a civilian's its
  attitude word (0 hostile, 1 friendly), player characters are friendly. One actor list per side.
- **Rank** (AI-011): 0 soldier, 1 officer, 2 knight (the profile's rank field). **Civilian kind**: 0 man,
  1 woman, 2 old man, 3 child, 4 beggar, 5 notable.
- **Character actions** (AI-012), the enumeration the profiles use for HUD icons and abilities: 0 none, 1 bow,
  2 punch, 3 hard punch, 4 purse, 5 sling stone, 6 shield, 7 big shield, 8 strangle, 9 lever, 10 help to climb,
  11 apple, 12 ale, 13 eat, 14 guzzle, 15 listen, 16 heal, 17 net, 18 pay the beggar, 19 wasp nest, 20 whistle,
  21 climb, 22 jump, 23 search, 24 resuscitate, 25 carry (strong), 26 carry (farmer), 27 tie up, 28 lock-pick,
  29 execute (kill on the ground), 30 test.

### 2.4 Profile records: layout and meaning

Byte offsets are decimal, relative to the start of the record's numeric block as profile.md names them
(`unknown_pre` starts at 0 right after the three strings; `unknown_post` starts at 0 right after the voice
code). Every field is little-endian. Consumers are the functions that read the field; the meaning follows from
what they do with it.

**Soldier record (SD, 68 entries)** — AI-020, AI-023, observed (0056a7d0 for the layout; consumers as listed).

| Bytes | Type | profile.md name | Meaning | Consumers |
|---|---|---|---|---|
| pre 0..1 | u16 | `p0` | **maximum hit points**; difficulty-scaled for hostile actors (2.11) | 004a1d00, 00435c80, 00432d30 |
| pre 2..3 | u16 | `p1` | **initiative**: shortens reaction delays (3.5.1); > 29 unlocks the alarm-run and attack decisions; > 30 / > 70 raise the attack-choice score; difficulty-scaled where scaled | 00438620, 0043a220, 00440cf0 |
| pre 4..5 | u16 | `p2` | **courage**: the retreat threshold and roll (3.7.2); 0.045 per ally in the "enough of us engaged" test; a battle cry only if > 39 | 00432d30, 004303e0, 0042ef80 |
| pre 6..7 | u16 | `p3` | **delegation** (officers): ≥ 50 "sends men rather than searches", < 50 "searches himself" (3.5.4, 3.7.1) | 00438cb0, 0043ccc0 |
| pre 8..9 | u16 | `p4` | **seniority**: `p4 + 100` per ally in the strength estimate; non-zero selects the long look-around interval; compared between two soldiers to decide who defers | 00432d30, 0041a420, 004303e0 |
| pre 10 | u8 | `pole` | pole weapon (the class block carries the same flag) | 00440470 |
| pre 11..12 | u16 | `q0` | **ranged skill** (the roll of 3.12.2); difficulty-scaled (the consumer was read; the copy at creation is inferred) | 004a5ba0, 0047bb60 |
| pre 13..14 | u16 | `q1` | **melee experience**: gates the AI's figures (3.8.2), many AI thresholds, and the damage factor of a rank-0 soldier's blows (3.9.3); difficulty-scaled | 00485240, 005c1dd0, 004a1cc0 |
| pre 15..16 | u16 | `q2` | **energy recovery**: `q2 ÷ 10` units per recovery period (3.10) | 00471b00 |
| pre 17..20 | u32 | `q3`, `q4` | **rank** (2.3): one 32-bit field, not two words | 0056a7d0, many |
| post 0..1 | u16 | `w0` | **rating**: melee experience gained for a kill = 20 + max(0, victim rating − attacker rating) | 0047e7d0 |
| post 2 | u8 | `flags` | the whole byte is loaded: bit 0 hostile side; bits 1 and 2 mounted kinds; bit 3 principal enemy (immune; special selection colour); bit 4 **post-bound** (never leaves the post to investigate; halves one alarm delay); bit 5 `unknown_flag5` (read by 0049b510, meaning not settled); bits 6 and 7 `unknown_flags67` (set on every retail record, no consumer read) | 004a5d30, 00438cb0, 0043ccc0 |
| post 3..4, 5..6, 7..8, 9..10 | u16 ×4 | `s0..s3` | stimulus susceptibilities in **file order purse, apple, beer, whistle** (percent): the purse word is the guard of the money reaction and enters the brawl-continuation formula; the apple word ÷ 2 is the chase count; the beer word guards the ale reaction; the whistle word guards the whistle reaction (> 1 for cover) and sets the knight's search radius `(whistle − 2) × 400 ÷ 98` px. See 9.1 for the review's contrary order and the evidence | 00438cb0, 004468a0, 0044cb40, 00424d50 |
| post 11..14 | u32 | `class` | 1-based index of the melee class block (2.5) | 0047ab00 |
| post 15..18 | u32 | `ranged_kind` | 1-based index of the ranged record (2.6), 0 none | 0047ab00 |
| post 19 | u8 | | `unknown_post19` (no consumer read) | |
| post 20..35, 36, 37..44 | f32[4], u8, f32[2] | | bounding box, its valid flag, the canvas centre (sprites.md) | |
| post 45..46 | u16 | `t` | **stun decay period** (3.11): 0 disables decay; otherwise one point per `t + 1` frames (64 regular kinds, 3 mounted, 10 antagonists) | 00471b00, 00471c00 |
| post 47..50, 51..54 | u32, u32 | `a`, `b` | weapon material (0 wood, 1 steel, 2 cast iron, 3 steel and wood), armour material (0 leather, 1 chain mail, 2 plate): **sounds only**, never a combat modifier | 0056c290, 0056c360, 00472070 |

profile.md's readings "p4 = knock-out resistance", "q1 / q2 = parry / counter chances" are superseded; its
"s = stimulus susceptibility (purse, apple, beer, whistle)" is confirmed. Data check on all 68 records: `p3`
non-zero only for officers; `p4` only for officers, knights, mounted knights, the trainer and the antagonists;
every stimulus word ≤ 100; every class within 1..27; every ranged kind within 0..4 (AI-023).

**Player-character record (PC, 10 entries)** — AI-021, observed (00565490, 0048fa30, 0055bf00, 0055bc90).

| Bytes | Type | Meaning | Consumers |
|---|---|---|---|
| pre 0 | u8 | alternate-sprite flag (1 iff the third string names one; 0 in retail) | 0048fa30 |
| pre 1 | u8 | **story hero** (the "256" of profile.md's first word): looked up among the seven story names; a lethal hit with a clover makes him fall into a coma instead of dying | 0055bf00, 0049fa20 |
| pre 2..3 | u16 | **bow skill** (the ranged roll) | 0047bb60 |
| pre 4..5 | u16 | **melee experience** (the twin of `q1` for the AI's thresholds; not a damage factor) | 0047ea20, 00485240 |
| pre 6..7 | u16 | **energy recovery**: `value ÷ 10` units per period (Robin 50 → 5) | 00471b00 |
| post 0..3 | u32 | melee class (1..10) | 0048fa30 |
| post 4..7 | u32 | ranged kind (1..4, 0 none) | 0048fa30 |
| post 8..11 / 12..13, 14..17 / 18..19, 20..23 / 24..25 | u32 action, u16 stock ×3 | the three HUD icons (2.3) with their starting stock; the stock is difficulty-adjusted at creation (2.11) | 0048fa30, 0055bc90, 0049d5e0 |
| post 26..41 | u32 ×4 | four context abilities (2.3) | 0049d560, 0049d520 |
| post 42 | u8 | a per-level portrait / colour selector | 0048fa30 |
| post 43..58, 59, 60..67 | f32[4], u8, f32[2] | bounding box, valid flag, centre | |
| post 68..69 | u16 | stun decay period for this character (as SD `t`) | 00471b00 |
| post 70..73, 74..77 | u32, u32 | weapon / armour material (sounds) | |
| post 78..79 / 80..81 | u16, u16 | **conspicuousness** in the forest outfit / in the town outfit: multiplies how fast an enemy's awareness of this character accumulates (3.2.5); the forest word is used when the mission's outfit flag is set, the town word otherwise (Robin 100 / 80) | 00488f60 |

The PC's maximum hit points are the constant 100 (0048f910). The team-order rank (10 − position) is computed at
load, not a file value. Revision 1's "post 80..83" layout was wrong (the block is 82 bytes).

**Civilian record (CV, 24 entries)** — AI-022 (observed for the loader, 00564ea0, 0056c490, 0056c410; the
record's consumers beyond the loader were not located: medium): pre 0..3 kind (2.3); pre 4..7 attitude
(**1 friendly, 0 hostile**; the tax collector, the two rich townspeople and the unarmed friar are hostile).

### 2.5 The melee class table (profile.md "table A", 27 blocks)

- **AI-030.** Block `class − 1` is bound to every human at creation; it is the hand-to-hand weapon class.
  Head (14 words): words 0..3 four distances in px — 0 the default minimum reach of the quick strikes, 1 the
  distance the AI closes to, 2 the default maximum reach and the "too far, close in" threshold, 3 the engagement
  distance (150 in every block); **words 4..8 the defence chance in percent by the attacker's approach** — 4
  when the attacker stands ≥ 20 px higher, 5 from the front, 6 from the left, 7 from behind, 8 from the right
  (3.9.3); **word 9 the stun resistance** (percent); **word 10 the arrow penetration threshold** (3.12.3);
  word 11 low byte pole weapon, high byte shield bearer; words 12..13 shield geometry offsets (non-zero for
  shield classes only).
- **AI-031.** Ten rows of 16 words, one per figure in the manual's order (0 quick jab, 1 slow blow, 2 finishing
  blow, 3 attack left, 4 attack right, 5 half circle left, 6 half circle right, 7 circle left, 8 circle right,
  9 the block). Per row: words 0..1 thrust target (always 2, chest); **word 2 stun value**; **word 3 hit-point
  damage**; word 4 minimum reach; word 5 maximum reach (px); words 6..7 thrust kind (0 straight, 1 lateral,
  2 push aside, 3 true half circle, 4 true circle, 5 false half circle, 6 false circle); words 8..9 side (0
  left, 1 right, 2 not applicable); words 10..12 three angles in degrees (start offset, sweep, per-step
  rotation of the sweeping figures); word 13 a push / step distance; word 14 `unknown_row14` (no consumer
  read); **word 15 energy cost**. profile.md's "2/3 = damage / effect, 5 = hit chance, 6 = figure class" is
  corrected to 2 = stun, 3 = damage, 5 = maximum reach, 6 = thrust kind.
- **AI-032** (observed on the data). The hero's two outfits are two records with two different classes: the
  forest outfit's rows mostly stun and rarely damage (only the finishing blow and the sweeping figures carry
  damage); the town outfit's rows never stun and always damage. The first mission uses the town outfit.

### 2.6 The ranged weapon table (profile.md "table B", 4 records of 81 bytes)

- **AI-035** (observed, 00569f40, 00450040, 004502e0, 004500c0). Record `ranged_kind − 1` (1 hero bow,
  2 outlaw bow, 3 soldier bow, 4 crossbow). Serialised layout: bytes 0..1 range A (px); 2..37 grid A; 38..39
  damage A; 40 the reach flag (when set, range B is the weapon's aiming reach instead of range A); 41..42
  range B; 43..78 grid B; 79..80 damage B. A grid is 6 distance steps × 3 skill rows of u16 percentages stored
  **distance-major** (for each distance step the values at skill 0, 50 and 100). The two damage words are the
  missile's damage (3.12.3); table B is **not** a difficulty table.

### 2.7 Per-human combat state (what a save must carry)

AI-040 (observed, 00471b00, 00471c00, 0047c600, 0047dc00):

| Quantity | Width / range | Initial | Notes |
|---|---|---|---|
| hit points | u16 | the maximum (2.4, 2.11) | never rises except by 2.11's easy rule and the flagged recovery (3.10); 0 = dead |
| fatigue | u16 | 0 | energy shown = 100 − fatigue; a figure adds its energy cost (no cap is enforced at the addition); recovers per 3.10 |
| stun level | u16, clamped 0..300 | 0 | ≥ 70 unconscious, < 30 awake; decays per 3.11; floored at 30 while tied, carried or (a player) in a coma |
| stun decay counter | u16 | 0 | loaded with the profile's period when the actor becomes unconscious **and the counter is 0** (a residual value is kept); during decay a counter at 0 triggers the decrement and the reload (3.11) |
| unconscious, tied, immune | flags | false | immune = principal enemy: its hit points are never lowered, stun is ignored |
| adversaries | ordered list | empty | several attackers per actor; the first entry is "the" adversary |
| initiative, grace, hand-over chance | flag, flag, u16 % | | the turn-taking of a mutual duel (3.9.1) |
| per-figure usage penalties | u16 × 9 | 0 | +50 when a figure is chosen; −10 (floor 0) each time a scoring pass is entered (not on a chooser call whose attack gate fails) (3.8.2); they decide later choices, so a snapshot carries them |
| melee experience, bow experience | (whole 0..100, hundredths) ×2 | the profile words | raised by kills and strikes, capped at 100 |
| ammunition | u16 per item kind | the profile stock | player characters: per kind with a maximum; soldiers: one count |
| door-stuck counter | u16 | 0 | incremented per frame while waiting at a door in order state 4; a reset at 26 (3.1) |

Everything in this table, in 2.8 and in 2.9 is authoritative simulation state: a snapshot carries it and the
canonical hash covers it (the HUD titbits do not).

### 2.8 Per-NPC AI state (what a save must carry)

AI-041 (observed from the serialisation order, 00414be0, and the consumers): the top-level state and the
sub-state (3.5); the target, the rival, the body, the object and the caller / officer elements; a remembered
position (x, y, height, layer) with its **priority** (1 a heard noise, 2 a body to loot, 3 a body seen, 4 a
missing colleague, 5 a sighting / arrow / look-there call: a lower priority never overwrites a higher one); a
second, officer-given position; the known elements; the company (chief, members, company number); the
colleague to check on; the "seen clearly" flag; the search flags; the walk-to and return spots; the flee
counter; the frame of the last alert; the walking flags (3.4.4); the hesitation byte (0 for hostile soldiers,
larger for civilian-class AIs); the drunkenness byte; the head marker (emoticon) with its expiry; the patrol
rail and the alert rail; the two timer deadlines and their armed flags with the sub-state the AI timer was
launched in (3.3.3); the emoticon expiry; **the cached program roll: its value and, separately, whether it is
still available** (AI-007: drawn once, consumed by the next program table); the check-for look accumulator and
interval; the search budget and the "alerting" flag; the level's seek-point cooling frames (3.5.4, level state
shared by all NPCs). Further authoritative quantities with explicit ownership (finding 8 of review 27):
- **the pending event queue** of the NPC — the deferred events in delivery order, each with its kind, payload
  (element or position) and parameters; the **queue-while-locked** flag and the events it queued (AI-082 a);
  the **locked** and **"out"** flags (AI-003);
- **the rail execution position**: the current point index and direction (AI-090), the program block being
  executed and the offset of the next command in it, the remaining length, and the suspension kind (none /
  waiting for a turn or glance to finish / the rail-wait timer / registered as a sync waiter with the partner
  and the waypoint index) (3.4);
- **the creation frame** of the NPC (the stagger key of AI-004 and AI-050);
- **the ten custom native values** of the NPC (natives 197 / 198), loaded from the record or the save;
- of the perception state (2.9): the mode, the **collapse increment** and the current base range while
  collapsing or recovering, the **tracked element** or the given direction, the head angle, the wobble phases.
No quantity above has a demonstrated reconstruction rule; each is snapshot-owned. **Integrated snapshot /
replay clearance is withheld** until the restore fixtures of 9.2 item 19 exist.

### 2.9 Per-NPC perception state (what a save must carry)

AI-042 (observed, 004863f0, 00486fa0): a perception *mode* (blind — dead or collapsed; normal; glancing left;
glancing right; wide; collapsing; tracking an element; looking in a given 16th of a turn; recovering); the base
range (px, from the level, 3.2.1); the current base range (ramps while collapsing / recovering); a range factor
(1.0; never changed by the code read); the effective range and half-angle; the body facing (16 directions); the
head angle (radians, relative to the body), its limit and turn speed; four wobble phases (drunkenness); six
**candidate lists** ("categories" 0 awareness, 1 bodies, 2 objects, 3 soldiers, 4 the watched element, 5
beggars) of entries {element, reported, shadow-reported, last value (single-precision), visible-now, heard-now}
with a per-category accumulator (u16), the maximum contribution over categories 0..2 and the lowest category
that contributed this frame; the wide-mode flag; a deafness value (px) with its last update frame; a "needs
rescan" flag; the per-frame line-of-sight cache is transient (rebuilt every frame) and is not snapshot state.
Each player character carries a walk-noise record {position, height, facing, zone, kind, radius}.

### 2.10 Mission per-actor fields (`BORG`, rhm.md)

AI-043 (observed, 004a1e90 disassembled, 0048b5d0, 00449170, 0044cae0): `unknown_0x1a` → the **patrol
chief** flag; `unknown_0x1b` → the **company number** (native 176 writes the same value); `unknown_0x23` → a
loot threshold read only by the purse-looting behaviour (inferred, low); `members` → the company member list;
`rail` → the patrol rail; `unknown_i16` → the **alert rail** (native 220 switches to it when it is not −1).

### 2.11 Difficulty

- **AI-045** (observed, 0055dbb0, 0050b640, 00438710, 00438600). Difficulty is 0 easy, 1 medium, 2 hard,
  chosen in the menu and stored in the settings. **This contradicts `spec-movement-animation-camera.md`
  ANIM-523 ("no difficulty setting in this build"); the consumers below exist and ANIM-523 is being amended.**
  A scaler applies to **hostile-side** actors only and is the identity on medium: hit points × 0.5 easy /
  × 1.5 hard (cap 10000, truncated); initiative, melee experience and ranged skill × 0.5 easy / × 2 hard (cap
  100). Further consumers: the player's starting stocks 6 → 8 easy / 4 hard, 12 → 15 / 9 (0055bc90, 0049d5e0);
  on easy a player character below 100 hit points regains 1 every 100 frames while not incapacitated
  (004936f0); the punch's stun amount × 1.5 on hard (00475bd0); the arrow's target guard (3.12.3, 004859b0); the
  identification radius × 1.3 easy / × 0.7 hard (004a0a30); the reaction-delay multiplier 2 on easy and hard
  (3.5.1, 00438620); a per-actor count 13 − q1 ÷ 10 easy, 10 − q1 ÷ 10 medium, 0 hard (0044d6c0, meaning not
  read); a scroll flag (004ba550) and an extra level element (004d9700) on easy.
- **AI-046** (observed, 00482150, 00471c00; medium). A "no player death" option byte in the options block,
  together with a launcher flag, prevents a player character's hit points from being set below their current
  value; its writer was not found (9.2).

## 3. Behaviour

### 3.1 The frame and the per-frame order

- **AI-001** (observed, 0050f710, 004c6ef0; ANIM-001..003 and VM-100 describe the same frame). The
  simulation advances one logic frame per main-loop iteration; the global frame counter is incremented once per
  frame. Unless the game is paused or the loop exits early, the loop waits at the end of each frame until the
  OS's reported millisecond time has advanced at least 40 ms since the frame's start (a debug slow-motion flag
  makes it 400 ms); nothing raises the OS timer resolution. **Nominal rule: 25 frames per second. Realised on
  the host the oracle measured: 46.875 ms per frame** (three 15.625 ms ticks of the reported time), which is
  the "64 Hz, three clocks per animation frame" of stealth-and-combat.md 8.4. The frame length is therefore host
  dependent; the OpenSherwood fixed-tick decision is in section 8, and no claim is made about the timing the
  data was authored against.
- **AI-002** (observed, 004c6ef0, 00404180). Every 25th frame the script's per-second callback runs with
  frame ÷ 25 as its argument (VM-100).
- **AI-050** (observed, 004c6ef0, 0048a980, 00487d00). Two pause mechanisms exist. (a) **The main-loop
  pause** (the menu, a page): no level tick runs, the frame counter does not advance, nothing below happens.
  (b) **The level-local pause** (a level flag read during each actor's tick): the actor's tick still runs its
  animation and movement steps, but the perception work (the rescan, the cone update, the scan, the deafness
  update) and the AI work (the think hook, the timer checks, the event queue) are skipped and the AI deadlines
  advance (AI-003). Each frame, for every non-player human in the engine's actor-list order (the mission's
  element order): (1) **unless level-paused**, the AI pre-tick hook; (2) the human base tick: stun decay
  (3.11), the animation and movement steps, and — player characters only — the refresh of the walk-noise
  record (a base tick that reports "not active" ends the actor's tick here); (3) unless level-paused: the
  rescan when flagged (every other actor enters category 1 unless listed; within a 700 px max-norm box when
  the AI is restricted), the cone geometry update (3.2.1), the perception scan (3.2.5) with its events
  delivered to this observer's own AI in list order at the end of the scan, the AI tick (the state logic), and
  the deafness update; (4) **after those**, the door-wait check: an actor in order state 4 whose action is
  one of the two door-wait animations, not "out" and not in a no-sight zone, counts frames and resets the AI
  at 26 (the counter clears otherwise); (5) **for civilians only, and regardless of the level pause, the lock
  and the "out" flag**: the civilian periodic routine keyed by (frame − creation frame + 156), which may draw
  random numbers (a 1-in-3 chatter roll and a variant roll) and play a remark; (6) the phases of AI-003 (the
  think hook, the timer checks, the emoticon expiry, the event queue), only when not level-paused, not locked
  and not "out". So an actor-local pause stops perception, the state logic and the timers, but not the
  animation, the movement, the door-wait check or the civilian chatter. No random number is consumed by
  perception (AI-007).
- **AI-004** (observed, 0048a980, 00471b00, 00487d00). Staggers keyed by the element id: identification when
  (id + frame) mod 16 = 0; the walk-noise hearing test when (id + frame) mod 3 = 0; energy recovery when
  frame mod 64 = id mod 32 (3.10).

### 3.2 Perception

#### 3.2.1 The view cone

- **AI-060** (observed, 004c1ab0, 0048ebe0, 0057ae60). The **base range** is a level constant: 400 px when the
  `FOOT` lighting word is 1, 8, 16, 32, 64 or 128; 300 px when it is 2 or 4; a script native can overwrite the
  global. Every NPC copies it at creation (the first mission: 400).
- **AI-061** (observed, 00486fa0). Every frame: if the body facing changed, the head angle is pre-compensated
  by −(change in 16ths) × 0.3927 rad, clamped to ±π/2, so the head keeps pointing where it was. Target head
  angle by mode: normal / recovering 0; glancing left −π/4, right +π/4 (these revert to normal unless the actor
  plays one of the glance animations 202..205 or action 1); tracking → the direction to the tracked element (y
  compressed); given → that direction. The head turns toward the target at 0.3927 rad per frame, limited to
  ±0.8 rad from the body (±1.3 rad tracking / given). **Half-angle 0.5 rad** normally; 1.5208 rad in the wide
  mode (set while the actor's order state is 19). Tracking or given: effective range × 1.4 (truncated) and
  half-angle 0.35 rad. One further condition of the NPC (not identified, 9.2) multiplies the range by 1.4 again.
- **AI-062** (observed, 00486fa0). Collapse (on losing consciousness): each frame the current base range
  shrinks by an increment that grows by 5 per frame (5, 10, 15, …) until it would go negative, then the mode is
  blind. Recovery: the current base range starts at 5 and grows by 8 per frame up to the base range, then normal.
- **AI-063** (observed, 00486fa0). Drunk wobble (drunkenness d ≠ 0): four phases advance by 0.1, 0.07634,
  0.12321 and 0.04546 rad per frame (mod 2π); range' = trunc(range × (1 − 0.01 d (0.1 (cos p0 + sin p1) +
  0.6))); half-angle' = half-angle × (1 − 0.01 d (0.1 (sin p2 + cos p3) + 0.2)), capped at 0.95 rad.
- **AI-064** (observed, 00486fa0, 0058e7f0, 00487b90). Axis = body facing rotated by the head angle; edges =
  the axis rotated by ± the effective half-angle; **the y component of every edge and distance vector is scaled
  by 0.5736** (its inverse 1.7434 in every distance test): the metric is an ellipse 400 wide and 229 tall for
  range 400. The Alt-key cone drawing uses the same record.

Validation (h01-measurements-2.md 6, independent evidence): the measured on-screen sector of ~80° becomes,
after undoing the 0.5736 projection, a world sector of 59° (half-angle 29.5°) whose axis lies on the
16-direction facing 67.5°: **agrees** with 0.5 rad. The measured reach (270 px along x, 196 along y, ratio
0.72) is 78..86 % of the predicted 310 / 229 (ratio 0.74). Hypothesis: the drawing fades with the graded value
(AI-066) and the capture's tint threshold cut the faint rim; alternative: an effective range near 320. Procedure
to separate them: capture the cone with a lower tint threshold, or read the range from the console's AI
display (9.2).

#### 3.2.2 Line of sight

- **AI-065** (observed, 004eff80, 004eef30, 004f0230, 004ef340, 005a3810). Two points see each other unless
  (a) one is above ground level (height > 0) and the other below (< 0), or (b) the segment crosses a
  sight-blocking polygon below that polygon's surface. The polygons are the proto-level's projection areas
  (`.rhp`, rhp.md): planar polygons with vertex heights whose second flag byte means "blocks sight" (the first
  "solid"), bound to a layer whose motion-area flag bits 0 and 1 are set. The 2-D segment is intersected with
  every edge of every candidate polygon in the spatial-grid cells the segment's box covers; at a crossing the
  segment's interpolated height is compared with the plane's height there. Results are cached per frame in a
  2000-slot table keyed by a hash of the two endpoints; **a hash collision within one frame returns the stale
  answer** (an original quirk; section 8). The mission file's own obstacle chunk (empty in retail) and mobile
  elements (carts) can add obstacles.

#### 3.2.3 Graded visibility of one element

- **AI-066** (observed, 00489680, 00489eb0, 00489d00, 0048db80). The value is 0 when the seen element's order
  state is 21..23 (hidden), the observer's mode is blind or collapsing, a global "players invisible" flag is
  set, or the element is inactive. An observer inside a **no-sight zone** (bit 13 of the zone's flag word:
  interiors) sees only elements in the same zone that are alive and conscious, with the fixed value 0.5 and no
  geometry. Otherwise, with v = target eye point − observer eye point and v.y × 1.7434:
  1. squared elliptical distance ≤ range², and the target in the front half-plane (axis · v ≥ 0) or within
     60 px (squared ≤ 3600);
  2. within 20 px (squared < 400): value 1.0;
  3. the **light-limited reach** L: with dz the height difference, L = √(range² − dz²) (0 if |dz| ≥ range).
     In levels of lighting 2 or 4 only: the light elements within a 400 px box are sampled at three points
     (observer + L·axis and the two edge points, each needing line of sight to the light); a light at squared
     distance s gives level 1.0 if s < 10, 0.5 if s > 110, else 1.05 − 0.005 s; the maximum over the lights is
     clamped to [0.5, 1]; f = 2·level − 1; L' = f × 400 + (1 − f) × L. Cached per NPC per frame;
  4. squared distance ≤ L'² and the angular test: within 60 px the target's direction in 16ths relative to the
     **body** facing must be 0..4 or 12..15 (the front 9/16 of the circle, ±101°); beyond 60 px the target must
     lie between the two edge vectors; then line of sight (3.2.2);
  5. the graded value g: ≤ 60 px → 1.0; else with r = |v| ÷ range: r < 0.3 → 1 − r ÷ 6; 0.3 ≤ r < 0.7 → 0.95 −
     1.75 (r − 0.3); r ≥ 0.7 → 0.25 − (r − 0.7) ÷ 3, clamped at 0 (r = 1 gives 0.15);
  6. the **posture factor** of the seen actor: order state 10 → × 0.5; else by its posture code: 2, 8, 14 →
     × 3; 3, 9 → × 20; 4, 5, 6 → × 2; 7, 10, 11 → × 1.5; others × 1 (the codes' names were not mapped, 9.2).
  The operands (positions, the range, the constants) are singles and the intermediates extended (AI-008); the
  value g leaves this computation unrounded and is rounded only when the entry's value is stored (AI-068).
- **AI-067** (observed, 00489680; medium). A friendly-side observer, when the mission's outfit flag is set, uses
  a simpler in-range / in-front / line-of-sight test giving 1.0 or 0.

#### 3.2.4 Per-category values and their staggering

- **AI-068** (observed, 00488f60, 00489b30, 00487d00). Let k = element id + frame. A **hostile** observer
  evaluates: category 0 — a soldier target every 16 frames, value 16 g; a player character or civilian every 2
  frames, value W × 2 g × 0.01 where W is the target's conspicuousness word (2.4; a target playing sprite
  action 242 gives 0; actions 246 / 247 set a flag whose use was not read); category 1 (bodies) — the target
  must be down and the observer not in a no-sight zone, every 8 frames, 24 g; category 2 (objects) — 0 in a
  no-sight zone, every 4 frames, 4 g' with g' the omnidirectional variant (range and line of sight, no cone);
  category 3 (soldiers) — every 8 frames, 8 g, the target must be a valid target; categories 4 (the watched
  element) and 5 (beggar) — target alive and conscious, every 8 frames, 8 g. A non-hostile observer evaluates
  only category 0 (16 g every 16 frames; dead targets 0). "Every k frames" means the frames on which
  (element id + frame) mod k = 0 (the observer's own id). Between evaluations the entry's **last value is
  reused**. **Rounding stages** (AI-008): the graded value g is an extended-precision result of
  single-precision inputs; for a player or civilian target the value is (2 × g) × W × 0.01 in that order,
  then **stored as a single-precision number** in the entry; each frame the contribution is trunc(stored value
  × 20) — trunc(stored value × 200) while the observer is in the wide mode — as an integer, whether the value
  was recomputed this frame or cached; "visible now" = contribution ≠ 0; the maximum contribution over
  categories 0..2 is kept for the AI. Example: W = 80, g = 0.5 exactly: (1.0 × 80) × 0.0099999997764825821 =
  0.79999998211860657, whose nearest single is 0.79999995231628418 (a tie, rounded to even); the contribution
  is trunc(15.999999046) = **15** per frame, **159** in the wide mode (not 16 / 160).

#### 3.2.5 Accumulation, the shadow ("?") and the sighting events

- **AI-069** (observed, 00487d00, 00488e00, 00488b80, 004887a0). Per category c each frame, entries are
  processed in list order: for each entry the contribution is computed (AI-068); **before it is added**, the
  shadow test runs: if the category is 0, 1 or 2, the target is a player character not yet engaged, the
  contribution is non-zero and acc[c] ≥ 100 already, the entry's shadow flag becomes set, and on its rising edge
  (was clear) one **shadow event** (3.3.2) with the target's position is queued; the flag is cleared when the
  conditions fail. Then the contribution of an entry that is not yet reported is added to S; after all entries,
  acc[c] += S (u16). **Emission** happens when acc[c] ≥ 1000, or when the category's rule says "immediate" and
  S ≠ 0: for a hostile observer categories 3, 4, 5 are always immediate, 0 and 1 are immediate once the AI's
  top-level state is beyond wondering, 2 once wondering or beyond; for a friendly observer 0, 3, 4, 5 are
  immediate, 1 beyond wondering, 2 wondering or beyond. On emission acc[c] := 0 and every entry is visited: a
  visible, unreported entry produces its event and — **category 0: is marked reported and kept; categories
  1..5: is removed from the list**; a category-0 entry that is reported but not visible produces out-of-view
  and is marked unreported. Without emission, the same out-of-view rule runs for category 0. The events:
  category 0 → view; 1 → sees body; 2 → sees object; 3 → sees soldier; **4 → sees the watched element (event
  18, the check-for detection)**; 5 → sees beggar. While S = 0 and acc[c] > 0, acc[c] decreases by 1 every 20
  frames. Entries are dropped when their target is dead (category 0), inactive (2) or no longer valid (5).
  There is no upper guard on the shadow test other than the category rule; a category whose accumulator passes
  1000 emits on that frame.
- **AI-070** (observed, 0048ebe0, 0048c700, 0041a300, 0041a490, 0043c960, 0048c620; the watched element's
  membership in an area search: inferred, medium). Candidate lists are filled explicitly: a hostile AI's
  initialisation puts every player character and enemy soldier into category 0; a friendly soldier's category 0
  holds the hostile soldiers; a civilian's the player characters; category 1 is filled at rescans and when
  someone falls (an actor playing the "become a body" action is pushed into every actor's category 1);
  category 2 by the AI when a purse or ale is thrown; category 3 when an officer is looked for; **category 4
  holds the element a check-for command registers (the colleague to check on); an area search clears it**
  (what the search then registers as "known" was not settled, 9.2); category 5 the beggars while a civilian's
  sub-state is a beggar one. Consequently an unalerted soldier notices a player character through category 0's
  accumulation (a "?" at 100, a sighting at 1000) unless the player has been made immediate by an alerted state.

#### 3.2.6 Hearing

- **AI-071** (observed, 0048cca0, 0048cc20, 0048cec0). A one-shot noise has a kind, a position with height
  and a radius R by kind: 0 → 300 (a projectile landing), 1 and 7 → 70, 2 → 50 (a projectile landing on a lying
  actor), 4 and 12 → 500, 5 / 6 / 8 / 9 → 200 (9 = a purse landing; 8 = the shout of a netted actor every 31
  frames; 4 / 5 / 6 = a mobile element's gait changes), 10 and 11 → 400, 14 → 0; kinds 3 (ordinary movement) and
  13 (a fighting source) take the emitting actor's walk-noise radius; a script native can emit any kind at a
  point. For every civilian or hostile actor: precheck max(|dx|, |dy|, |dz|) ≤ R; **a listener exactly at the
  source's position hears nothing**; otherwise margin = trunc(R − euclidean 3-D distance) − deafness; **heard
  iff margin > 0**; the hear event carries the noise record and the margin.
- **AI-072** (observed, 0047ef70, 00487d00, 00488a40). A player character's **walk-noise radius** is refreshed
  every frame from his sprite action and the ground material class m under his feet (0..10): idle and fidget
  actions (0, 1, 3, 14), the pick-up actions (158..160, 292, 293) and the bow set (85..94) → 15 px; the
  walking group (2, 4..8, 13, 15..24, 34..39, 49, 50, 81, 82, 255, 256, 295, 303) → 20 (m 0, 2, 7), 40 (m 1, 3,
  6), 10 (m 4), 200 (m 5); the loud group (9..12, 51, 83, 84, 294, 296, 297, 304) → 70 (m 0), 150 (m 1, 3, 6),
  75 (m 2, 7), 50 (m 4), 400 (m 5); action 30 → 80; **actions 27, 33 and 98 → 50 regardless of material**;
  actions 127 and 188 → 50 (m 0, 2, 7, 10), 100 (m 1, 3, 6), 30 (m 4), 300 (m 5); the melee actions 59..79, 96
  and 100 → 200 regardless of material (emitted as the fighting kind); actions not in any group → 0; **a
  material class not listed for the group leaves the previous radius unchanged**; 0 inside a no-sight zone or
  when inactive. Every third frame (staggered) each NPC inside the noise's bounding box runs AI-071's test
  against every player character in its category 0; on the rising edge (not heard last time, heard now, not yet
  reported) it posts hear with the target's position and the margin.
- **AI-073** (observed, 0048a830, 005b3c40). Deafness decays per frame by 10 while < 301, else by
  50 × trunc(v ÷ 300), then is raised to the coverage of the level's loud-sound geometry at the actor's
  position (per source a polyline: value = strength − trunc(distance to the polyline), 0 beyond). No explosion
  writer exists in this build.

Validation (stealth-and-combat.md 8.6, independent): a walk at 290 px was not detected (20..40 px: agrees). A
run detected at ≥ 330 px is **not** attributed: the loud group reaches 330 px only on material class 5, and a
soldier facing the runner sees him at once in category 4 / after accumulation in category 0. Procedure: read the
courtyard's material class under the run's path from the map data, and repeat the run with the console's noise
display on (9.2).

#### 3.2.7 Identification (silhouettes, the mini-map)

- **AI-074** (observed, 00487d00, 004a0a30, 00570e70). An NPC still shown as a silhouette that is a civilian
  or a soldier checks every 16 frames: a friendly soldier is revealed at once; otherwise it is revealed when
  any active, alive, conscious player character has it within G × f, G the base range, f = 1.95 if that
  player's order state is 16 else 1.5, × 1.3 on easy / × 0.7 on hard, the distance elliptical (y × 1.7434) and
  3-D unless the NPC is lower than the player (then the range is extended by the height difference and tested
  in 2-D), with line of sight. The script's reveal native runs the same routine. A hostile that sees a not yet
  revealed civilian of kind 4 in its category 0 reveals it. The mini-map colours follow the silhouette flag and
  the side (the drawing itself was not read). Validation: the oracle's "identified at about 120 px while
  climbing" disagrees with 600 px; hypothesis: the walkway's wall polygons blocked the line of sight; procedure:
  one approach on open ground with the silhouette's reveal frame recorded.

#### 3.2.8 Hidden and asleep

- **AI-075** (observed, 00489680, 00488f60; medium). A player character playing sprite action 242 or in order
  states 21..23 is invisible to the cone; an actor inside a no-sight zone is invisible from outside it. The
  mission-start cloak and "hidden in leaves" were not matched to these ids (9.2).
- **AI-076** (observed, 00410620, 00433830). A napping NPC does not process sightings but does process
  noises (3.5.2) and hits.

### 3.3 The AI event system

#### 3.3.1 Events

- **AI-080** (observed, 00424bc0, 00410620, 0040d980). Everything the AI does is a reaction to an event that
  carries its kind, a source element or position and a few parameters. Sources: perception (3.2), other NPCs
  ("calls", delivered synchronously, the caller learning whether the call was accepted), the actor (reached
  point, could not reach, done = an animation or sequence finished, got hit, lose consciousness, fit again,
  timer), the script, and the AI itself. The return-to-duty event resets the AI (3.5.1).
- **AI-081** (observed, 00410620, 0041f540). The events are named here by what happens; the only numbers an
  implementer needs are the **script-visible values** the filter callback receives (6.2), given in brackets.
  *Perception*: an enemy comes into the cone [0]; a reported enemy leaves it [1]; a noise is heard [2]; a body
  is seen [8]; an object (purse, ale) is seen [9]; a soldier is seen [10]; a friend in trouble is seen [11]; the
  watched element (the checked colleague) is seen [17]; a beggar is seen [30]; a shadow (the "?") is seen [52];
  a brawl is seen [32]; an arrow lands nearby [31]; a stone lands [54]. *Actor*: reached the point [3]; could
  not reach it [4]; the animation or sequence is done [5]; the order was impossible [6]; the AI timer fired
  [7]; got hit [13]; loses consciousness [14]; fit again (the stun fell below the waking level) [12]; the
  checked colleague is missing [15]; the object went away [16]; a synchronisation partner arrived [18]; the
  gallop loop ended [51]. *Script and control*: continue after the script [19]; return to duty [20]; stop
  [−2]. *Fights*: a player shot at me [−2]; enters a sword fight [22]; quits it [23]; an incoming sword strike
  [24]; the adversary is weak [55]; injured after the fight [56]; a good strike / a lethal strike [−2]; an
  enemy is near [−2]; an arrow was launched [53]; a door combat [50]. *Stimuli*: panic [21]; wasp / wasp gone
  [25 / 26]; apple [27]; net / net gone [28 / 29]; an apple chase comes near [49]. *Calls between NPCs*
  (delivered synchronously, the caller learns whether they were accepted): alert [33], combat alert [34], hey
  [35], hint [36], instruction [37], look there [38], coordinate [39], report [40], go to the officer [41],
  officer I am back [42], the colleague is back [43], patrol coordinate [44], tower-guard alert [45], the tower
  guard calls me [46], finish the brawl [47], you just wait [48], clean up after the brawl [−2]. *Dialogue
  beats* between two NPCs (the "my line" / "your line" steps of a reported conversation, four each) [−2]. The
  values not listed above are not produced. Every event's behaviour is in section 3 under the name used here.

#### 3.3.2 Pre-filter

- **AI-082** (observed, 00410620). Before any state logic: (a) a script-locked AI drops every event, except
  that while a "queue while locked" flag is set reached-point and done are queued and replayed in order when
  "continue after script" arrives; (b) in the wasp reaction only lose-consciousness and wasp-gone pass; under a
  net only lose-consciousness and net-gone; a merry man leaving the map only reached-point; (c) a lying actor
  passes only lose-consciousness and fit-again; (d) a timer event is dropped when the sub-state it was launched
  in differs from the current sub-state (a stale timer); (e) an actor out of action (dead, neutralised) drops
  everything; (f) an **unconscious** actor drops every event except fit-again and lose-consciousness, and drops
  fit-again too **while its order state is 7** (an order state, the same domain as native 89's 18 and the
  hidden states 21..23 — not a sprite action id); (g) three events are converted here regardless of state:
  **lose consciousness** clears the order and the head marker, removes the NPC from any brawl, enters sleeping /
  unconscious, puts the stars on the actor and silences him; **wasp** clears the order, sets the storm cloud and
  the wasp reaction; **net** enters the under-the-net reaction. For actors flagged for scripting the event is
  first offered to the script's filter callback (6.2); a zero return drops it.
- **AI-083** (observed, 00434aa0). Hostile per-16-frame poll: in walking sub-states, if the actor's animation
  is a walk (ids 55..58 or 161) but he has not moved for four consecutive polls, the move order is reissued, or
  could-not-reach is posted to self when there is no target; while idle in a non-moving sub-state an idle
  remark plays with probability 1/12 per poll (one roll); a boredom counter plays a "bored" remark past 20.
- **AI-084** (observed, 00419fb0, 00419fd0, 0057aed0, 00486f30, 005c3fc0). The **head marker** (emoticon) has
  an optional expiry in frames: 2 question mark, 3 exclamation mark, 4 Z (asleep), 5 rain cloud (stimulus
  refused), 6 sun (going for a purse or ale), 7 storm cloud (wasp, brawl), 8 spiral (drunk), 0 none; native
  228 sets it. A separate actor marker draws the stars while unconscious (count min(4, stun ÷ 50) + 1), "fit"
  (1) and a timed spiral after a wasp or net.

#### 3.3.3 Timers

- **AI-003** (observed, 00416710, 0048b450, 0048b4e0, 0048a980, 00422c40). Each NPC has two single-shot
  timers: the **AI timer** (a request of 0 frames becomes 1; the sub-state at launch is remembered for AI-082
  d) and the **rail-wait timer** (a request of 0 stays 0). Arming sets deadline = (the frame counter at the
  moment of arming) + length. **Phases within one actor's tick** (after step 7 of AI-050), taken only when the
  AI is not locked, the actor not "out" and the level not paused: (i) the think hook, when (frame − creation
  frame + 156) mod 16 = 0 — it may arm timers; (ii) the AI-timer check: if armed and (frame ≥ deadline or the
  deadline lies more than 1,000,000 frames ahead), disarm and deliver the timer event; (iii) the rail-wait
  check: if armed and frame ≥ deadline, disarm and resume the program when the sub-state is "in a program";
  (iv) the emoticon expiry; (v) the deferred event queue, drained in order while not locked / out (events posted
  during the drain are handled in the same drain). A timer armed on frame f with length n ≥ 1 therefore fires
  in phase (ii) of frame f + n whether it was armed in phase (i) or (v) of frame f. When the AI is locked, the
  actor "out" or the level paused during an actor's tick, phases (i)..(v) are skipped and **all three
  deadlines** (AI timer, rail wait, emoticon) are incremented by one — the frames counted are the actor's own
  ticks on which that condition held, a half-open interval [first skipped tick, first non-skipped tick).

### 3.4 Patrol programs (the rail interpreter)

- **AI-090** (observed, 005505c0, 00550f60). A rail is walked **back and forth**: the cursor holds the point
  index and a forward flag; one *advance* moves forward by one, except at the last point where it moves to the
  previous point and flips the flag, and backward likewise except at index 0 where it moves to index 1 and flips.
  A one-point rail never advances (the actor stands on post). A rail assignment starts at index 0, forward.
- **AI-091** (observed, 00410de0, 00550660, 004121a0, 00419cc0, 00408750). **At a point**: if the point has no
  program, advance and walk to the next point. If the point carries the script flag, the script's reach-point
  callback runs and, unless the script took control (the "script driven" sub-state), the AI posts itself
  "continue after script". Otherwise the program table is chosen by direction: table id 1 only when arriving
  forward, id 2 only when arriving backward, id 0 always. The cached roll r (1..100, AI-007) picks the first
  block whose weight ≥ r, subtracting each skipped block's weight from r; **no block picked → walk on**. Before
  walking to the next point the engine peeks at the block the same roll would pick there: if it contains no
  blocking command (face, wait, check-for, check-for-sync, stop, look, bend) the walk is flagged "do not stop at
  the waypoint" so the actor passes through without halting.
- **AI-092** (observed, 00410010). **Opcodes** (operands little-endian):

| Op | Operands | Behaviour |
|---|---|---|
| 0x00 | – | flip the travel direction, continue with the next command |
| 0x01 | – | explicit end: **advance the cursor once, then the end-of-program path advances it again** (two advances, each with AI-090's endpoint rule); the actor walks to the resulting point. On a two-point rail this returns to the point just left; on a longer rail it skips one point (at an end the reversal applies at each step) |
| unknown / end of block | – | the rest of the block is discarded; the end-of-program path advances the cursor **once** and the actor walks to the next point |
| 0x02 | u16 p | set the current point to p (previous := current) and walk there **without** advancing (an error is logged when p is the current point) |
| 0x03 | u16 dir | turn in place to the 16-way facing dir; blocks until the turn finishes (at once when already facing it) |
| 0x04 | u16 t | wait t **frames** (the rail-wait timer; 100 = 4 s nominal; rhm.md's "1/100 s" is wrong) |
| 0x05 | u16 guy, u16 N | check-for (AI-094) |
| 0x06 | u16 guy, u16 N, u16 wp | check-for with synchronisation (AI-095) |
| 0x07 | – | stop: leave the rail, remember the current position as the post, stand there |
| 0x08 | u16 rail | switch to rail `rail` (an error is logged if out of range), clear the program, restart the default state |
| 0x09 | – | set the run bit of the walking flags (AI-093), then execute the next command at once |
| 0x0a | – | clear the run bit, then execute the next command at once |
| 0x0b | – | glance left (perception mode 2, the glance animation), wait for it to finish |
| 0x0c | – | glance right, likewise |
| 0x0d | u16 t | **bend** (the stoop animation) and wait t frames (rhm.md's "check-for with a radius" is wrong) |
| 0x0e | – | patrol-stop flag := 1; a marked profile plays a remark; continue |
| 0x0f | u16 k | re-link company member k in the follow order (mechanics observed, purpose inferred) |
| 0x10 | – | patrol-start: flag := 0, a remark for marked profiles, re-sort the company by distance; continue |

  The bend, the glances, the patrol commands (civilians) and the two check-fors (civilians and friendly
  soldiers) are **diagnosed as illegal for those actors but still executed**; an implementer must not rely on
  them being skipped.
- **AI-093** (observed, 00412240, 0041bd20, 00576e00). The **walking flags** govern every rail move: bit 0 =
  **run**: the actor plays the *sprint* cycle (action 10) instead of the walk (6) (other bits force the walk or
  select cycle 290); 0x400 no stop at the waypoint; bit 2 stop exactly on the point (cleared when the target is
  on another layer); 0x10 check reachability first and report could-not-reach; 0x20 turn to the point's
  direction after arriving and play a follow-up action; 0x80 accept arrival within a radius. Native 140 writes
  this word for styles **0 (walk) and 1 (run)** and re-issues the current leg with the new flags if the actor
  is on a rail; **for any other style the word written is not the supplied value but bits derived from the
  actor's memory address** (an original defect, 005780d0 / 0041bd20): only styles 0 and 1 are cleared, and
  OpenSherwood treats any other style as an error with no change (a deliberate deviation, section 8 item 7).
- **AI-094** (observed, 00439080, 00424d50, 004444a0). **Check-for(guy, N)** (hostile AI): guy indexes the
  mission's NPC table (must be an NPC other than self). Skipped when guy is in the actor's own company or when
  the actor's last alert is less than 3000 frames old. Otherwise (warnings only) at least one waypoint of guy's
  path or his post should be visible from here; guy becomes the **watched element** (category 4, AI-070); the
  look count = N ÷ 10 + 1 and the look interval = 1000 ÷ count frames; a random left or right glance starts (one
  roll). Loop: glance done → "looking for the colleague" with a 10-frame timer; on each timer a new glance starts
  with probability (acc + 10) ÷ 5000 (one roll, then one for the side), acc grows by the interval every 10
  frames; once acc exceeds 1000 the actor gives up and goes **seeking**: a remark, then a search route built from
  guy's post or his waypoints starting at the nearest (3.5.4). Seeing guy meanwhile (the watched-element
  sighting) ends the look. N
  is a look budget, not a radius.
- **AI-095** (observed, 00439080, 00410de0). **Check-for-sync(guy, N, wp)**: with N = 0 a pure
  synchronisation: wp < 500 is an absolute waypoint index of guy's path, wp ≥ 1000 means "my current index +
  (wp − 1000)". If guy is in the default state and already at or beyond that waypoint (direction-aware), the
  program continues; otherwise the actor registers with guy and waits (polling every 20 frames) until guy
  reaches a point whose index equals the stored one and posts sync to every waiter. With N > 0 the check-for
  look loop runs first and the synchronisation follows.
- **AI-096** (observed, 00418600, 00424d50, 0044cae0; medium). **Resuming**: after an alert the default
  state's re-entry walks the actor to the *nearest* waypoint of his rail and continues from there; an
  interrupted program resumes its pending command; native 220 replaces the rail by the alert rail (2.10).
- **AI-097** (observed, 0041a960, 0041ac50, 00412d60, 00449170; medium: the slot geometry was not read).
  **Company**: members follow the chief: on "go to the chief" a member walks to a slot relative to the chief
  (members sorted by distance and paired left / right), then waits 200 frames; the chief on a patrol leg turns
  to the stored facing on arrival, waits 200 frames, and moves on only while his followers are in the default
  or wondering state. Natives 218 / 219 add / remove subordinates.

### 3.5 The state machine outside attacking

Top-level states: **sleeping**, **default** (post, route, patrol, script-driven, in a program, looking for a
colleague, synchronising), **wondering** (curiosity and stimuli), **seeking** (search and reporting),
**attacking** (3.7), **menacing** (standing over a knocked-out player), **fleeing**. "T = n" arms the AI timer
with n frames; "×m" is the reaction delay of AI-100; "reset" is return to duty (AI-101). Rolls name their draws.

#### 3.5.1 Reaction delays, reset, placement

- **AI-100** (observed, 00438620 disassembled, 0055dbb0). A delay marked ×m is trunc(((100 − initiative) ×
  0.01) × base × m + 1) frames, evaluated in that order in extended precision with the single-precision
  constant 0.01 (AI-008); initiative = SD `p1` unscaled; m = **1.0 on medium, 2.0 on easy and hard** for
  hostile-side actors, 1.0 for others. Separately, a friendly-side NPC that is not mounted uses a flat 3 frames
  when the mission's outfit flag is set. (Revision 1's "uninitialised multiplier on medium" was wrong: the
  multiplier is initialised to 1.0.) Examples: base 60, initiative 10: 90 × 0.0099999997764825821 =
  0.89999997988; × 60 = 53.99999879; + 1 → trunc = **54** on medium, **108** on easy and hard (107.99999758 +
  1); initiative 0 on medium: 100 × 0.00999999977 × 60 + 1 = 60.99999866 → **60**; initiative 100 → 1 on
  every difficulty. The integer-boundary cases fall to the lower integer because 0.01 is stored slightly
  below 1/100.
- **AI-101** (inferred, medium; the event's handling is observed in 00424d50, the body of the hostile reset
  was not isolated). **Return to duty** (the return-to-duty event, the unlock native, the end of a search, the finish of a
  brawl): the AI leaves its current state for the default state and the actor goes to his post, to the nearest
  point of his route or resumes the patrol (AI-096); what else the reset clears (the target list, the post
  facing) is inferred from the behaviour that follows and is a hand-off boundary for the implementer (9.2).
- **AI-102** (observed, 00435c80, 00414550). **Creation / placement**: at the AI's initialisation a hostile
  soldier's current hit points are put through the difficulty scale (principal enemies excluded) and the
  initial state follows the placed action: action 48 → unconscious with the **stun level set to 300**; 45 or 47
  → dead for good; 162 → napping with the Z marker; 159 or 270 → on post with a flag; 0 / 1 / 3 → on post with
  T = the look-around interval; an actor placed inside a house sector → the inside-house sub-state.
- **AI-103** (observed, 0041a420). The **look-around interval on post**: officers 200 + rand%600; profiles
  with `p4` ≠ 0 400 + rand%800; everyone else 70 + rand%70 frames (one roll each).

#### 3.5.2 Perception events → reactions (default, wondering, seeking)

- **AI-104** (observed, 0044d750). **Company relay**: a hear, sees-body or sees-object event reaching a
  member of a company in the default or wondering state is re-sent to every soldier of the company and its
  chief, and counts as handled; a receiver with an able chief lets the chief handle it instead.
- **AI-105** (observed, 00433060, 00439cb0). **Sighting of an enemy (view)** in default / wondering /
  seeking: ignored if the seen element is hidden or a player already held by another NPC; ignored (hostile side
  only) if the target stands more than 100 px higher than the viewer, unless script-marked. Then the alert frame
  is recorded; "seen clearly" = same layer or at most 50 px below, or, if higher, flat distance² ≤ dz²; the
  position is remembered with priority 5; a viewer inside a house / door sector goes to the door-fight logic
  (3.7.3). Otherwise the target list is cleared, a **look-there call is shouted to every same-side human within
  100 px** (accepted only by receivers idle, wondering or just-watching), and: a rider → attacking /
  reaction-running toward the target, T = 10; anyone else stops, plays the sighting remark (the "!"), sets the
  target and: distance < 30 px → the reaction sub-state and an immediate decision; seen clearly → the reaction
  sub-state with T = 5; else reaction-turning, a turn to the target, T = 20.
- **AI-106** (observed, 00433830). **Noise (hear)** in the sleeping, default, wondering or seeking states, by
  kind (AI-071): kinds 1, 2, 9 (quiet) only in default → question mark, wondering / watching, face the spot,
  T = 50 (kind 2 plays a remark). Kind 8 is ignored by friendly NPCs and when already watching, else like 3.
  Kinds 3, 7, 13 (steps, running, a fighting source): unless busy with money or a brawl → remember the spot
  with priority 1, and (a) if not seeking, or in "got stop", or an officer → seeking / heard-steps
  pre-reaction, question mark, remark (not for kind 7), T = 1 if already in default else 60 ×m; (b) already
  seeking → heard-steps reaction, face the spot, T = 1. Kind 10 (a whistle): if nobody of the company is already
  reacting to a whistle and the profile's whistle word is non-zero → seeking / just-watching, question mark,
  face, remark, T = 60; else remember (priority 1) and, if not seeking or an officer → wondering /
  heard-whistle, question mark, T = 50 ×m, else the heard-steps reaction. Kinds 11, 12: in default → watching
  toward the spot, T = 70 + rand%60 (one roll).
- **AI-107** (observed, 00434320). **Sees a body**: the body is filed and remembered with priority 2; a
  down-but-alive player joins the "prey" list. In a seek-point sub-state or while seeking a colleague → go to it
  as a found body; in a body sub-state for another body → queued; otherwise a remark (a different kind if the
  body still carries money), the 100 px look-there shout, question mark, seeking / body reaction, face,
  T = 80 ×m.
- **AI-108** (observed, 00434710). **Sees an object**: object kind 3 (ale): in an ale sub-state → queued; else
  clear the order, remark, face, question mark, wondering / ale reaction, T = 60 ×m. Kinds 7 and 8 (purses):
  in a money or brawl sub-state → queued; else remark, face, question mark, wondering / money reaction, T = 60
  for officers, 30 for others.
- **AI-109** (observed, 00433790). **Sees a shadow** (the "?" of AI-069): unless in a house sector or hiding →
  default / looking at the shadow, face the spot, T = 10; each timer returns to duty if the shadow counter is
  zero, else waits 10 more.
- **AI-110** (observed, 00433f90). **An arrow landed**: remember with priority 5; already seeking and not an
  officer → seeking / arrow reaction toward a point near the arrow, face, T = 1; officers → arrow just-watching,
  face, the look-there shout within 200 px, T = 60; others → arrow reaction with T = 100 ×m; the reaction's
  timer → a remark, run to the arrow, the 100 px shout, T = 200; on arrival or timeout → an area search from
  self (radius 0, thorough for rank 0).
- **AI-111** (observed, 00439e70, 00439f60, 0043a080, 004457c0). **Look-there call** → question mark for 10
  frames (hostile only), wondering / watching, face the spot, T = 100. **Tower-guard alert call**: knights →
  seeking / knight watching the tower guard (its timer → an area search of radius 300 around the remembered
  spot), others → wondering / watching the tower guard; question mark 10, remember (priority 5), face, T = 100.
  **Tower guard calls me**: rank 0 → call the nearest officer (AI-121), rank 1 → the officer's tower-guard
  routine (not read, 9.2). **Stone**: wondering / apple reaction with a remark, face, question mark, T = 50.
  **Stop** (from a script): in a sword fight ignored; else seeking / got-stop, stop moving, marker cleared; its
  timer → restart the patrol path and look around.

#### 3.5.3 Wondering (curiosity, stimuli, the brawl)

- **AI-112** (observed, 00424d50). **Watching → looking around**: watching and each "look n sideways" on
  timer → the next sub-state with a random glance (one roll for the side); each "look n" on done → the next,
  the facing turned by +5 sixteenths, T = 30 + rand&7 (one roll); after the third look → reset. About 35 frames
  per leg, three legs.
- **AI-113** (observed, 00424d50, 0043d890). The **glance**: one side, the other, both orders (the head-turn
  animations 134 / 135 / 136 on soldier profiles) or the bend; it ends with done.
- **AI-114** (observed, 00424d50, 00445d40, 004460f0, 004468a0, 0044b000, 00446400, 004469e0). **Purse /
  money**: money reaction, timer → the purse word non-zero and a purse reachable: the nearest seen purse (+300 px
  penalty for another layer), remark, approaching-money, sun for 20, walk (arrival 20 px), T = 5; not
  susceptible → rain cloud for 50, remark, purse list cleared, reset. Approaching: on timer, if a rival soldier
  also heads for it → running for it, and if the company's officer can see me he receives sees-brawl. On
  arrival: purse still there and within 25 px → stop, play the pick-up (84) on it, tell every same-side soldier
  in a money sub-state the object is gone, taking money; farther → watching for more money with a glance.
  Taking money → the next purse, else watching for more money. **Brawl**: an object-gone for a purse I wanted:
  if the continuation rule (below) says so and I am approaching → face the taker, storm cloud, brawl reaction
  with the taker as rival, T = 30 ×m; in a brawl already → the taker joins the rival list; else go for another
  purse. Brawl reaction, timer → every officer of the side in default or money-reaction within 200 px, or within
  350 px with sight of me, receives sees-brawl; brawl approaching, storm cloud, remark, walk to the rival
  (arrival 30), T = 1. Arrival: rival awake and within 33 px → stop, play the punch (86) at him, brawl hitting;
  farther → re-walk; rival asleep → drop him. Hitting, done → panic to every civilian who can see me; officers
  alerted again; a down rival is removed; **continue** iff share × 100 ÷ (share + brawlers + 1) < the
  profile's **purse** word, where share is the count of same-side soldiers already going for the purse (always
  continue when the word is 100 or the AI is civilian-class; the exact operands: medium): the rival still
  standing → approach again (T = 30); another rival (the nearest soldier in a money sub-state) → approach him;
  none → the nearest purse (running) or watching for more money. Got hit in a brawl → recovering (the get-up
  animation 71), officers alerted, storm cloud → the next rival or the purse. Watching for more money → clear the
  brawl list; an unclaimed purse → approaching loot (arrival 20) → within 100 px: a lying body instead of a
  purse → remember (priority 2) and go to the body; else loot (animation 85) → purse count grew → sun for 20 +
  remark, else rain cloud for 20 + remark; more purses → next, else reset. **Officer and the brawl**:
  sees-brawl → walk to the brawl (arrival 100) → the finish-brawl call to every brawling soldier of the side + a
  remark, storm cloud, T = 200 → the purse lists cleared, every brawler but the addressed one gets return to
  duty; a soldier receiving finish-brawl stops, faces the officer, marker cleared, "looking at the officer",
  T = 300 + rand%32.
- **AI-115** (observed, 00424d50, 00449df0). **Ale**: ale reaction, timer → the beer word non-zero →
  approaching the ale, sun for 20, remark, walk (arrival 20), the return spot stored, T = 20; else rain cloud
  50, remark, reset. Arrival: the ale gone → face, storm cloud, "ale gone", T = 30; else play the drinking
  animation (141) on it, drinking; drinking done / ale-gone timer → the next ale, else reset. (Drunkenness is
  set by the ale object's code, not read.)
- **AI-116** (observed, 00424d50, 0044cb40, 0040e7e0). **Apple**: apple reaction, timer → the apple word
  non-zero and a child found → the apple chase (running to the child; apple ÷ 2 legs: on each timer, count 0 →
  end, face the child, T = 30; else count −1, run to the child, T = 10; the talk events play the dialogue); else
  reset. The child flees (3.5.6). Apple in the visor → the hit-by-apple reaction then the stone reaction. Wasp in
  the armour: events blocked until wasp-gone → marker cleared, question mark, look around (T = 30). Under the
  net: until net-gone → "fit" marker, question mark, look around.
- **AI-117** (observed, 00424d50, 00443d10, 00443af0). **Whistle**: heard-whistle, timer → face the spot,
  watching after the whistle, T = 60 → whistle word > 1 and the "never leaves the post" value ≠ 100: rank 0 →
  an officer reachable near the spot → walk to him, "looking to the officer for advice", question mark,
  T = 100; else if `p3` < 50 or ≥ 1 soldier is known → the officer-looking behaviour; rank 1 → the officer
  looking for soldiers; rank 2 → an area search around the spot with radius (whistle − 2) × 400 ÷ 98 px; not
  susceptible → reset. A civilian child approaches the whistler (walk to 50 px, remark) and stands 100 frames.

#### 3.5.4 Seeking (the search)

- **AI-118** (observed, 00424d50, 00438cb0). **Heard steps**: pre-reaction timer → ranks 0 and 2: if the
  actor may leave his post (flag bit 4 clear and the "never leaves" value ≠ 100) → heard-steps reaction,
  question mark, face, T = 60; else just-watching (a glance sequence ending in reset for rank 0, or in the
  officer's soldier-look for rank 1), question mark, face, T = 60; rank 1: ≥ 1 known soldier or the spot more
  than 100 px away → just-watching, else the reaction. Reaction timer → rank 0 with an officer reachable near
  the spot → walk to him, "looking to the officer", T = 100; else walk to the spot (T = 200) and, on arrival or
  timeout, an **area search** from the actor's own position, radius 0, flags "run, thorough".
- **AI-119** (observed, 0043a220, 0043d0c0, 0043d530, 00522330, 00416800, 00438600). **Area search** (centre,
  radius, flags, optional direction): friendly-side NPCs and NPCs whose "never leaves" value is 100 return to
  duty instead. The order is cancelled (unless a flag says otherwise), a pending body takes precedence, the
  watched element is cleared (AI-070). If radius > 0: every **seek point** of the level (a global list loaded
  from the mission; which chunk was not traced, 9.2) gets a cost = distance² to the centre + 100 per layer step;
  points within radius² count; with a direction, points in a 15-sector fan around it are preferred (nearest
  ahead, then to the sides); points with cost < 10⁶ are inserted in cost order. If the first point's
  **freshness** < 90 the "thorough" flag is cleared; every other same-side soldier within 500 px already
  searching multiplies a budget by 1.2 and clears my thorough flag. The number of points to visit = the count in
  radius (more when thorough); points are drawn in cost order, **each accepted with probability freshness %
  (one roll: rand%100 < freshness) and inserted at a random rank (one roll)**; a visited point is *cooled*:
  freshness = 100 − remaining ÷ 100 %, cool-down 5000 frames per visit, never more than 10000 frames ahead. A
  flag adds the centre itself as a point; another, or an empty list, adds the NPC's own position as the last
  point; a flag selects running. Then the next point is taken (or, inside a house, the sideways watch).
- **AI-120** (observed, 0043d530, 00424d50). **Next seek point**: for each remaining point in order, with
  probability freshness % (one roll) → cool it, "going to the seek point", question mark, walk or run; a
  rejected point is skipped; a beggar seen during the search takes precedence (approach to 50 px, a question, a
  second question, T = 30 / 50 / 100; a betraying beggar points at the player, who becomes the target); when
  nothing is left → reset, and if the best remembered priority ≤ 1 and the flags lack the "found" bits → the
  "nothing found" remark. At a seek point: choose a random free direction from the point's perimeter list,
  excluding directions within 1/16 of the current facing (reservoir choice, one roll per candidate); watch it
  (T = 20), a sideways glance (one roll), the next direction, and so on; passed-ambush-point sub-states
  re-approach the point with a glance.
- **AI-121** (observed, 0043c960, 00424d50). **Reporting to an officer** (rank 0): the nearest awake officer
  of the side in the default state, cost = distance + 100 per layer; if any soldier of the side is already
  reporting, nobody else goes; walk to 70 px of him, T = 50. On arrival, if the officer is available → give the
  alerting report (the remark by what was seen: priority 0 / 1 a noise, 2 / 4 a body, 3 a missing colleague, 5 a
  sighting), T = 150 → by report kind and how many men the officer has already sent (noises need 0 sent, bodies
  fewer than 2, sightings fewer than 5) → the report call, the talk, a point at the spot, T = 100 → end, T = 30
  → reset. Officer side: he **calls his soldier** ("hey", accepted only by rank-0 soldiers in default /
  wondering or a few seeking sub-states: they stop, face, question mark for 20, T = 20), waits (T = 20 polls),
  instructs (the talk, a point at the spot), then waits for the instructed soldier with a patience of 101 polls
  × 30 frames after which he alerts the tower guard or searches himself (radius 300); an instructed soldier
  copies the officer's target spot and searches from it (radius 0), then returns to report (T = 100).
  **Groups**: an officer calls his whole company (the instruction carries a spot per member; bodies are spread
  over the members), waits for them (pruning to members still coming), and prunes to members still searching;
  an officer inside a house first leaves it. **Civilians report** (3.5.7) through the alert call; the soldier
  waits for the report (T = 20 polls, question mark 20), takes it (T = 30) and then: rank 0 with `p3` ≥ 50 →
  search 300 px, else call an officer or search; rank 1 → search or the tower guard; rank 2 → search 300.
- **AI-122** (observed, 0044a760, 00424d50, 0043ccc0, 0043cec0, 0044cf00, 0044a830). **Bodies**: body
  reaction timer → friendly viewer → search 300 px around self; hostile: a remark once; rank 0: an officer near
  the body → walk to him (T = 100); else by `p3`; rank 1: body > 150 px away and `p3` → the officer look; else
  **go to the body** (exclamation mark, arrival 30, T = 10). At the body (within 60 px): a same-side body →
  "looking at the dead body", the body handed to the actor, T = 50 → hostile: the next body, or the
  missing-colleague alarm (a remark, remember own position with priority 3, rank 0 calls the nearest officer,
  rank 1 alerts the tower guard, others search 400 px around self — 200 if the colleague has a patrol; the
  reaction delay is halved for post-bound units); an enemy body not out of action → reset; else **wake the
  sleeper**: the revive animation (75) on the body, T = 50 → the next body or the alarm. A netted comrade → cut
  the net (five times animation 85 then 84) and help him up.
- **AI-123** (observed, 00424d50). **Combat alert call** (from a fight): rank-0 receivers in default /
  wondering / seeking: question mark 10, face the fight, T = 50 ×m → run to the spot (arrival 50) → search 300
  px around it; attacking receivers accept silently; others refuse.
- **AI-124** (observed, 00424d50). **Could not reach**: at a seek point → the next point; going to a body →
  the next body or a search 300 around self; in panic → the movement handler; else a blocking door is opened or
  flagged and the event re-posted; in sleeping / default / wondering / menacing / fleeing → reset; seeking →
  search 300 around self. **Impossible** → done is posted to self (except while killing a sleeper).

#### 3.5.5 Sleeping

- **AI-125** (observed, 00424d50, 00471c00, 00414550). **Unconscious**: only fit-again and lose-consciousness
  pass (AI-082 f); fit-again (raised by the stun level falling below 30, 3.11) → awakening (the "fit" marker,
  T = 100) → if the actor has a patrol path not yet restarted, restart it; question mark, look around (T = 30)
  → reset. A "wake now" flag skips the awakening. **Napping** (placed asleep): no timer; a noise per AI-106 or
  a hit wakes him. **Dead for good**: nothing passes. A comrade's revive animation (75) with the body as partner
  precedes the wake-up, which is the stun path (AI-174).

#### 3.5.6 Fleeing

- **AI-126** (observed, 00424d50, 0040e7e0, 00413240). Entries: a civilian's panic (a brawl punch or a fight
  seen), "you just wait" (a soldier chasing a child), "apple chase near" (a friend chased), the archers' retire /
  run for arrows / run to alert soldiers, and the run-away decision (3.7.1). A chased child picks a flee spot:
  15 directions at 300 px, the radius shrinking by 10 per ring until reachable, a random start direction
  (rand%5 + 14 plus the chaser's bearing, one roll, sides alternating); runs at speed × 1.2 while the chaser is
  more than 150 px away, else × 1.0; a few more legs after the chaser gives up (at most 7), then reset. A merry
  man leaving the map runs to the exit and, within 10 px, fades and is removed. Run to alert soldiers → the
  tower-guard alert, else run to the door. Retire from combat → turn to face, then back to the attack logic. Run
  for arrow reserves → on arrival the actor receives 20 arrows, then an area search 300.

#### 3.5.7 Civilians (the friendly AI)

- **AI-127** (observed, 0040bfd0, 0040c140, 0040cc60, 0040d150, 0040e290; the attitude branches 0040d440 /
  0040d690 were skimmed: medium). Same pre-filter and event set, no attacking. A sighting in default /
  wondering: a friendly-attitude civilian "admires the hero", a hostile one reacts as to an enemy; in seeking
  the position is remembered (priority 5); in fleeing another flee leg with a remark, at most 7. A body → the
  civilian body reaction. Both reactions, on timer → the nearest idle same-side soldier (cost distance + 250
  per layer, path-checked) → run to him (arrival 70, remark); none → flee. At the soldier: idle and within 70 px
  → the alert call; accepted → the report (a start, a point at the spot with the position, an end, T = 10 / 30)
  then flee; refused or busy → another soldier or reset. Panic → run away from the source; a stop from a script
  → got-stop, T = 100. A whistle → a child approaches the whistler. Civilian-class AIs bypass the purse / ale /
  whistle susceptibility guards. Civilians are untargetable by the player's strikes (3.9.2); a loss rule for a
  killed civilian, if any, lives in the scripts.

### 3.6 How an alarm spreads

- **AI-130** (observed, all of 3.5). There is **no mission-wide alarm broadcast** in the AI code (the console's
  "alert every NPC" handler was not located). An alert spreads by: (1) the 100 px look-there shout on a
  sighting, a body or an arrow (200 px for an officer's arrow shout), which only makes idle receivers watch the
  spot for 100 frames; (2) the company relay (AI-104); (3) a soldier's report to the nearest officer, who
  instructs his group (AI-121); (4) the combat-alert call sent from a fight (AI-123, 3.7.1); (5) the tower
  guard's alert call (AI-111); (6) civilians reporting (AI-127); (7) brawls: sees-brawl to officers within
  200 px (350 with sight), panic to civilians who see a punch. The "!" is the head marker 3 or the sighting
  remark; the "?" is marker 2.

### 3.7 Attacking: decisions and tactics

#### 3.7.1 The decision and its outcomes

- **AI-140** (observed, 004303e0, 004207f0, 00432d30, 00438cb0). The decision runs at the end of most
  attacking timers (reaction, overview, reserve overview, observe, bow observing, after a step back, when a
  partner is lost) and produces one of: fight, observe, reserve, last reserve, alert soldiers, run and alert
  soldiers, look for help, run away, too proud to attack, tower-guard alert, tower-guard observe, and for
  archers shoot, step back, run to the archery point, cover behind a shield bearer, run for new arrows.
  **Required inputs**: the enemies (the other side's living, targetable actors), the friends (own side, alive,
  targetable, within 500 px), for every enemy the number of friends currently engaging him (in a melee
  sub-state with him as target, or in another sub-state but closer to him than I am), E the total of those, n
  the number of enemies, dmin the nearest enemy's distance (AI-142's metric), whether a player is among the
  enemies, whether a friend of lower rank exists, whether a plain soldier is among the friends; sleeping
  enemies (unconscious, not netted) are a separate list. **Priority of outcomes**: (1) no enemy → return to
  duty; (2) a stored forced decision (an officer's instruction, a script) wins; (3) a **post-bound** actor
  observes; (4) the morale check (AI-141) selects the fight or the retreat branch; (5) fight branch, melee
  actors: an officer with a rank-0 friend present and not already alerting → alert soldiers; else if E < n or
  dmin < 150 px: the "never leaves" value 100 and dmin ≥ 150 → last reserve; else if no lower-ranked friend
  exists or the too-proud test (AI-145) fails: on the hostile side, when (courage × 0.045 + 1) × n ≤ E →
  observe, else fight; else too proud; else (E ≥ n and dmin ≥ 150) → reserve. Tower guards: alert if not yet,
  else fight when dmin < 150 else observe. Archers (an actor with a ranged weapon — ranged kind ≠ 0 — whose
  hesitation value is 0): the line blocked by a friend → step back; an archery path assigned → shoot; no
  shield bearer: reserves exist → run to the archery
  point, else cover when a bearer is found, else shoot; with a bearer: cover when farther than 25 px from the
  cover spot, else shoot. (6) Retreat branch: an archer without arrows → run for new arrows; rank 0 with a
  visible player and not already alerting → look for help; rank 1 → run and alert soldiers (same guards) else
  run away; rank 2 → run away. **Permissions** consulted from seeking and some transitions: fight / run away
  need flag bit 4 clear and the "never leaves" value ≠ 100; reserve the beer word ≠ 0; alert soldiers and
  menace the purse word ≠ 0; run-and-alert the apple word ≠ 0; shoot / step back `p3` ≥ 50; look for help
  `p3` < 50 or a group; cover the whistle word > 1; observe the profile's `q2` > 50; a civilian-class AI may
  only run away, reserve, alert, run-and-alert or menace.
- **AI-141** (observed, 00432d30). **Morale**: strength = Σ over friends (a player 100, an NPC 100 + `p4`);
  r = 100 × strength ÷ (100 × n + 1); s = r < 100 ? max(0, (r − 20) × 50 ÷ 80) : min(100, 50 + (r − 100) ×
  50 ÷ 200); if hp < max: s = s × hp ÷ max (integer); a rank-0 soldier with an officer among the friends: s ×
  30. **Retreat iff s < 50 − courage ÷ 2 and rand%100 > courage** (one roll; courage 100 never retreats).
  Retreat unconditionally when fleeing, an archer without arrows, or an archer caught in a sword fight.
- **AI-142** (observed, 0043d780, 0042fe00, 0043ea70). **Target choice**: the nearest valid enemy with the
  distance √(dx² + (1.7434 dy)²); the join from observe adds 10000 per attacker already on that enemy (prefer
  the unengaged), reserve and step-back add 100 per attacker, shields skip the validity test. "Enemy near"
  while in reaction / observe makes that actor the target at once. Got hit outside a sword fight: a
  brawling hitter → the brawl logic; otherwise the hitter becomes the target. If my first opponent already
  fights ≥ 2 of us and a crowded friend exists, my target may be reassigned.
- **AI-143** (observed, 00442470, 004303e0). **How many attack one player**: nothing caps it but the observe
  rule (with courage 40 — a blue halberdier — 2.8 attackers per enemy before newcomers observe), the target
  choice's penalties, and the observer's join test (join when my target's first opponent has ≤ 2 partners or I
  am within 30 px, and no other friend is already running to it; otherwise keep 100..200 px). Several soldiers
  do attack one hero at once.

#### 3.7.2 Execution of the outcomes

- **AI-144** (observed, 004303e0, 00432f80, 0042ef80, 0042fe00, 00445210, 0043c960). **Fight** → pick the
  target, then run / walk / charge to it by distance and the class (a shield charge when the level allows it,
  the distance > 99 px and the actor is not mounted; a battle cry on a charge if courage > 39), T = 10; on
  arrival at reach → the sword fight (3.8), T = 20, a remark, the adversary links set (3.9.1). **Observe** →
  walk to 100..200 px of the target, a 50-frame overview, decide again. **Reserve** → stand, look, and every
  timer send the coordinate call to the other reserve soldiers; on receiving one → a 20-frame overview then
  decide. **Step back** → a free spot 50..250 px behind. **Look for help** → set the alerting flag; a path to an
  officer → a remark and run; else run away. **Cover** → run behind the shield bearer. **Too proud** → walk to
  100..200 px and stand, overview T = 150, retire. **Tower guard** → the alarm animation / observe from the
  tower. **Archer observe** → a look-around unless in a bow state. **Run to the archery point** → run along the
  archery path. **Last reserve** → a remark and a position check. **Run away** → fleeing. Reaction time: from
  seeing / hearing; after done / timer, if the enemy is nearer than 30 px → immediate (T = 1), else T = 30 ×m or
  10.

#### 3.7.3 Per-kind tactics

- **AI-145** (observed, 0044a090, 00424d50). **Officers** alert soldiers when a rank-0 friend exists (stand
  with the arm raised, T = 20, decide again); rank-0 soldiers near an officer get × 30 morale. **Too proud**
  (rank ≠ 0, hesitation value 0): skip fighting while a lower-ranked friend is already engaging or observing the
  same target. **Knights** run away in the retreat branch; their too-proud sub-states approach to 100..200 px,
  overview 150 frames, retire.
- **AI-146** (observed, 00424d50, 00445510, 00445180, 0044b4e0). **Archers**: loading → aiming with T = (110 −
  ranged skill) ÷ 2 frames → the line blocked by a friend → decide, else shoot (3.12) → observing T = 50 →
  decide; retire from combat (turn to face, decide); arrow reserves are level-defined piles; archery paths are
  rails walked with a final sprint; morale: no arrows or caught in melee → retreat. **Crossbowmen** use the same
  paths.
- **AI-147** (observed, 00447e50, 0044dd70, 00447070; medium). **Shield bearers / phalanx**: a phalanx needs
  at least two partners already in it; the formation point comes from the leader's facing; archers run behind
  the bearer; "protecting with the shield" looks around at random and, with a 1-in-4 chance per timer (one
  roll), picks a new target and advances; "advancing with the shield" adopts the partner's target.
- **AI-148** (observed, 0044c850, 0044ca90, 00424d50; medium). **Riders**: charging — a free point 500 px
  ahead within ±1 sixteenth (radius shrinking by 10), gallop through, on "gallop loop end" with nothing left →
  run and engage; returning / getting distance / passing sub-states.
- **AI-149** (observed, 0044b940, 0044bc70, 00424d50; medium). **Door fights**: waiting at a door 150 frames,
  turning, leaving; **ladders**: wait for the enemy (T = 100 / 30). **Sleeping enemies**: approach within 20 px
  → the execute animation → a remark → return to duty; a player in a coma is **menaced** instead (its timers
  were not read, 9.2).

### 3.8 The NPC in a sword fight

- **AI-150** (observed, 0043db20, 00441650, 00442190). The **sword-fight step** runs on the timer (re-armed
  at 20 frames while in the sword fight), on done and on reached-point, and on the adversary-weak and
  after-combat-injury events. Required outcomes in order: (1) no adversary left → leave the fight; the target
  dead or incapacitated → quitting (T = 3); the target := the first adversary; a lost player → an area search
  (radius 300), else return to duty. (2) The actor must face the target within ±1 sixteenth, else wait for the
  next timer. (3) Two rolls rand%100 ≤ hesitation (0 for hostile soldiers → 1 % each) → skip this step. (4) On
  an adversary-weak step: distance > head word 2 and experience > 59 → move to head word 1. (5) If not
  post-bound, and not exactly one friend against one enemy, and rand%3 = 0 (one roll) → a manoeuvre (approach a
  new enemy or move around the old one) instead of striking. (6) Distance > head word 2 (90 px for pole arms,
  70 for swords) → move within head word 1 (65 / 50); a post-bound fighter instead returns to his post when
  more than 20 px from it. (7) **Strike**: only if the target is not in a hit or fall sequence and its action
  state is a fighting one; the figure choice (AI-151) returns a figure → the special-strike sub-state, the
  attack aimed at the target (a battle cry before the finishing blow and the sweeping figures); when its
  animation is done or the timer fires → back to the sword fight with T = 20.
- **AI-151** (observed, 00485240, 00485590, 00482240, 00482fd0, 00440cf0; the exact thresholds are engine
  constants and are **excluded from this file**, 9.3). **Figure choice**: skill = melee experience (`q1`
  scaled, or the player's word); threshold = max(skill, 50); one roll: rand%100 ≥ threshold → the **attack
  gate fails**: outside reaction mode → no attack this step (a low-skill soldier attacks on half of his steps,
  a skill-80 one on 80 %); in reaction mode a **non-NPC** actor (a player) then draws a **second roll** and
  returns nothing if rand%100 < skill, otherwise (and an NPC always) only the block remains possible (below).
  No adversary → nothing. When the gate passes, each of the nine figures is *admissible* only if the actor's
  experience reaches a per-figure minimum and the actor's hesitation does not exceed a per-figure maximum —
  fixed pairs the program carries as content (9.3): the quick jab has no experience requirement, the laterals
  a low one, the slow blow a moderate one, the half circles and circles high ones, and the finishing blow a
  near-maximal one; **the two circles are exceptional: an actor with hesitation 0 (a hostile soldier) needs
  the circles' experience minimum and gets no bonus, while an actor with a non-zero hesitation may use them
  regardless of experience and they then earn a +500 score bonus** (revision 2 had this reversed). A figure is
  scored only if the target is idle or the figure's displacement + 2 < the target's remaining displacement;
  **the sweeping figures (half circles and circles) need at least two opponents inside their arc and reach
  band**, the others one; score = Σ over those opponents of (expected value + 30), expected value = damage ×
  (100 − the victim's defence) ÷ 100 + stun × (100 − the victim's stun resistance) ÷ 100, −1 if a friend is in
  the arc, **minus three times the figure's usage penalty** — except that a circle admitted through the
  non-zero-hesitation exception scores its sum **plus 500 without the penalty subtraction**. **When the
  scoring pass is entered** (the attack gate passed and an adversary exists) every penalty first decays by 10
  (floor 0); a chooser call that stops at the gate decays nothing. The figure with the highest score wins;
  **on equal scores the earlier row wins** (a later row must strictly exceed); the winner's penalty rises by
  50. No admissible figure → "step back" when the adversary is too far. In **reaction mode**
  (AI-152) the block is returned when the block animation's displacement is shorter than the incoming strike's
  remaining displacement — after a failed gate (the NPC case, or the player's second roll not below skill) the
  block is the only candidate; after a passed gate with no scored figure it is the fallback.
- **AI-152** (observed, 00442190). **Reaction to a player's strike** (the incoming-strike event while in the sword fight,
  special strike, approaching or moving around): the incoming figure is classified; the chooser runs in reaction
  mode; then with experience > 49, a circle-type incoming thrust and a free spot at reach + 10..20 px → **step
  back**; else the **parade** (the block animation, T = incoming displacement + 10); if the chooser returned an
  attack → a **counter-strike** instead. Good / lethal strike events only play remarks. Quit-swordfight or
  object-gone in a melee sub-state → quitting (T = 3) → decide again.
- **AI-153** (inferred, medium-low). **Cadence**: there is no swing timer; a strike attempt is the product of
  the 20-frame step, the facing and target-state gates, the hesitation and manoeuvre rolls, the attack roll, the
  admissibility and the parades. The oracle's ~5.3 s between halberd raises (combat-measurements.md 1.5) is
  **not derived** from these rules (the interaction with the animation lengths was not traced); the "about
  three steps plus animations" estimate of revision 1 is withdrawn. Procedure: trace the sub-state transitions
  of one halberdier with the AI display during a 60 s fight (9.2).

### 3.9 Melee resolution (both sides)

#### 3.9.1 Engagement, turn taking, disengagement

- **AI-160** (observed, 0047c600, 0047c4c0, 0047f520, 004801d0, 004808f0, 00472070). **Engagement**: A adds
  B as an adversary if B is attackable (alive; a civilian is never attackable by the player's side; the
  immune are not attackable), the height difference ≤ 40 px or both on the same layer, the distance ≤ head
  word 3 (150 px) and the line of sight is clear; with the "exclusive" flag the request is refused when either
  already fights someone else with more than one adversary. **The victim of a landed or attempted strike
  receives an attack order** on the attacker when he is not a civilian and the sides differ — so the defender
  enters the stance and fights back, a player character included (his auto-fight). The player's attack order
  (a left click on an enemy) walks him within 150 px (a path search if farther), then enters the stance (action
  52) and starts the auto-fight; the shield posture charges instead (action 154). The fist order from the front
  is the same order with a stun amount (3.11); there is no punch from the front.
- **AI-161** (observed, 0047dc00, 0047e1e0, 0047f420; not measured). **The auto-fight decision** (a player in
  the fight idle, fatigue < 100), each frame: with more than one adversary, rand%3 = 0 (one roll) switches to
  another adversary. Then, if my first adversary's first adversary is **not me** (a non-mutual opponent): one
  roll, rand%100 > 9 → nothing this frame (a 10 % chance to act). If it **is** me (a mutual duel): not holding
  the initiative → the defensive / repositioning behaviour and nothing else; holding it with the grace flag
  clear → one roll, **rand%100 ≤ hand-over chance** (inclusive: a chance of 0 still hands over on roll 0, one
  in 100) or a manoeuvre condition → hand the initiative to the opponent, setting his initiative and his grace
  flag; holding it with the grace flag set → clear the grace flag, no roll. After that, distance > head word 2
  → approach, else a random quick strike (3.9.2).
- **AI-162** (observed, 0047d270, 0047ca50). **Disengagement**: an adversary is dropped when he dies, is
  knocked out, moves more than 150 px away, goes out of sight, or changes layer / height by more than 40 px; an
  actor with no adversary left sends himself "quit swordfight" and leaves the stance (the fight idle is action
  54); soldiers report the quit-swordfight and after-combat-injury events to their side.
- **AI-163** (observed, 0047dc00, 0043db20, 0043ea70). **Several attackers**: an actor keeps the list; his
  adversary is the first; NPCs balance targets (AI-142). Hits from several attackers are independent messages;
  nothing serialises them.

#### 3.9.2 Who is struck

- **AI-164** (observed, 00475bd0, 00482fd0, 00482e50, 004801d0, 00482600, 004828b0, 00482390, 00480e90,
  00482b90, 00483070, 00481ef0). When a strike's animation reaches its hit frame the attacker collects targets
  by the figure's thrust kind: **straight** → only the current adversary, if minimum ≤ distance ≤ maximum reach
  of the row (re-checked in 3-D); **lateral, half circle, circle** → an arc from the attacker's facing: start =
  facing ± angle word 10, end = start ± angle word 11 (half circle) or ± 180° (circles), advancing by angle word
  12 per animation step; **every attackable actor within reach and inside the swept sector is struck**
  (civilians excluded, alive, conscious, within 150 px, line of sight); **push aside** strikes actors within
  reach whose bearing is within half of the row's word 13 of the facing and displaces them. The **quick strikes**
  (the click attacks, actions 59..66, orders that carry "no figure") use the head's default reach (words 0..2)
  and **no row**: they deal neither damage nor stun (AI-166). Each struck target receives a strike message
  carrying the attacker, the attacker's class block and the figure.
- **AI-165** (observed, 00475bd0, 004820e0). Rows ↔ sprite actions: row 0 = action 67, 1 = 68, 2 = 75, 3 =
  70, 4 = 69, 5 = 74, 6 = 73, 7 = 72, 8 = 71, row 9 the block (76; 127 for the held stance); the quick strikes
  are actions 59..66 (the high / low variant by the target's height difference).

#### 3.9.3 Resolution on the victim

- **AI-166** (observed, 0047e7d0, 005c1b70, 005c1dd0 disassembled, 005c1db0, 00471ed0, 00471e50, 00472070).
  The victim resolves the message in this order, drawing from the global stream:
  1. A lying victim (posture 10 / 11) is affected only by the two finishing-off figures (result "finished"); a
     victim without a class block (a civilian) takes nothing.
  2. **Defence roll**: b = the bearing **from the victim toward the attacker** in sixteenths; for a left attack
     b + 4, for a right attack b − 4, for straight and circling figures b itself; d = (b' − victim facing) mod
     16: d ∈ {0, 1, 15} → head word 5 (front), 2..5 → word 8 (right), 6..10 → word 7 (behind), 11..14 → word 6
     (left); if the attacker stands ≥ 20 px higher than the victim → word 4 instead. One roll r = rand%99 + 1;
     **the blow lands iff r > word**.
  3. If it lands: **damage** = trunc(row word 3 × f), f = 1 + 0.01f × (the attacker's melee experience,
     difficulty-scaled) **only when the attacker is a rank-0 soldier**; otherwise f = 1. The finishing-off
     figures deal 1. Neither armour nor weapon material enters. The victim's hit points drop by it (the rising
     number over the head is old − new); if the victim dies the attacker gains melee experience 20 + max(0,
     victim rating − attacker rating) (no roll).
  4. **Stun roll**, whether or not the blow landed: one roll r2 = rand%99 + 1; if r2 > the victim's head word 9
     (stun resistance) and the row's stun value ≠ 0 and **the victim's current hit points are not 0**: stun
     level += stun × 100 ÷ current hit points (integer division).
  5. **Outcomes and draws.** The resolution reports two *attempt* indications, not state changes: the
     **damage indication** (the defence roll passed and the row's damage was applied to the hit points — the
     immune's hit points do not fall, the indication is still set) and the **stun indication** (the stun roll
     passed and the row's stun value is non-zero — set even when a zero hit-point count or immunity prevented
     any increase of the level). Three special outcomes carry no indications: a **lying** victim (no defence
     and no stun draw; a finishing-off figure reports "finished", any other figure reports nothing); a victim
     **without a class block** (a civilian: no defence and no stun draw, and it reports **both indications
     set**, exactly as a damage-and-stun result — there is no separate "rejected" outcome); and "nothing"
     (a standing victim with both indications clear). The strike handler then: on *nothing* exits (the
     displacement reactions that precede this exit still happen; no further roll); on *finished* plays the
     reaction and exits without the experience roll; otherwise plays the reaction (step 6) and, **when the
     figure is one of the ten row figures** (not a quick strike), draws **one more roll** rand%100 and compares
     it with the attacker's experience × 0.2 — the experience-gain message applies to player attackers only,
     but that roll is consumed for every attacker in this case. **Decision draws per strike**: a standing
     victim with a class block: 2 (defence, stun) + 1 if either indication is set and the figure is a row
     figure; a quick strike on such a victim: 2; a victim without a class block: 1 for a row figure (the
     experience roll only), 0 for a quick strike; a lying victim: 0. Draws made by the reactions themselves
     (sounds, remarks) are accounted separately and are not decision draws.
  6. The reaction animation (00474d60): damage only → the flinch 44 (stance 102, bow 114); stunned or
     unconscious → the fall 44 / 107 / 114 then, if still conscious and level > 40, the stagger 103; dead → the
     forward fall 41 when the row has a stun value, else the backward fall 44 (stance / bow twins 105 / 107 and
     112 / 114).
- **AI-167** (observed, 00481d40, 00472070). **The block** (the right-click defensive stance, action 127):
  while it runs, every frame the actor's fatigue is reduced by 5 (floor 0); once fatigue is 0 the block ends
  **at the next animation boundary**. Blocking is resting; it cannot be held for ever. Its effect on incoming
  strikes is the ordinary defence roll (the block row has no words the resolution reads). Soldiers report it to
  their side.

Validation (combat-measurements.md, independent): the hero's 100 hit points, the halberdier's 80 (`p0` × 1 on
medium), the 5-hp hits (halberdier jab damage 5 × 1.05 → 5), the click attacks never hurting him (no row), the
**town-outfit slow blow doing 50 for 2 energy pixels** (row 1: damage 50, energy 10 of 100), the fighting
distance 52 (the hero closes to head word 1 = 50): **agree**. Not explained and kept as measured: the 25-hp loss
over 1.5 s (an aggregate; hypothesis: his finishing blow 20 × 1.05 = 21 plus a jab, or two hits); "two of three
swings landing" (the hero's defence words are all 0, so every *resolved* soldier strike lands; hypothesis: the
third raise was a step the gates skipped or a parade — not proven). Procedure: record the halberdier's sub-state
transitions and the hero's bar at 20 Hz for 60 s.

### 3.10 Energy, health, death

- **AI-170** (observed, 00475bd0, 00471b00, 004a1cf0, 0048f9d0). **Energy** = 100 − fatigue (the bar's 20 px
  are 5 units each). A figure adds its row's energy cost when its animation finishes (parries cost nothing; no
  cap is enforced at the addition). **Recovery**: on the frames where frame mod 64 = id mod 32, while standing
  still (position equals the target position) with no adversary: fatigue −= min(fatigue, rate ÷ 10) with rate
  = the player's energy word (Robin 50 → 5 units = one bar pixel per 64 frames = 2.56 s nominal, 3.0 s realised)
  or the soldier's `q2` (blue halberdier 10 → 1 unit; one bar pixel = 320 frames = 12.8 s nominal, 15 s
  realised). Completing a walk to a new position also recovers rate ÷ 10 once (00475bd0). At fatigue ≥ 100
  the auto-fight stops. **Discrepancies kept**: the oracle measured one hero pixel per 0.8..1.0 s and one
  soldier pixel per ~4 s; neither matches the 64-frame rule; the walk-completion recovery is a *possible*
  additional source for the hero (he moved between strokes), not a demonstrated one. Procedure: a fight where
  neither fighter moves, the bar read at 20 Hz (9.2).
- **AI-171** (observed, 004d5a90, 00482150, 005c2ec0). The HUD rows: red = hit points ÷ maximum (capped at
  1), blue = (100 − fatigue) ÷ 100; the rising number = the hit-point loss of one strike; the stun draws only
  the stars.
- **AI-172** (observed, 00482150, 0049fa20, 00474d60, 00485e90, 004748f0, 004936f0). **Death**: hit points
  0 → the die handler with the killing amount; the actor's state becomes dead; the fall per AI-166 step 6 (an
  arrow: 40 / 41, bow 111 / 112, stance 104 / 105); the "down" states make the body enter every actor's category
  1. A story hero with a clover falls into a coma instead. **Passive regeneration** exists on easy only (2.11);
  a separate **flagged recovery** (a per-player flag whose writer was not identified) raises hit points to 75 at
  once and then by 1 per frame to 100, regardless of difficulty.

### 3.11 Stun, knock-out and bodies

- **AI-173** (observed, 00475bd0, 00472070, 00471e50, 00471ed0, 00471c00, 00463da0). **The punch** (the
  fist order → action 123): a player without the hard-punch ability sends stun 80 (with it 150; × 1.5 on
  hard); an NPC's punch sends 40; the AI's finishing of a sleeper sends 3. The victim applies it **without any
  roll and without his stun resistance**: level += amount × 100 ÷ current hit points (a blue halberdier at 80:
  +100 → unconscious at once; a 250-hp antagonist: +32). Immune actors ignore it. The "from behind" condition
  is not in the resolution: the order side decides whether a punch or a fight starts (AI-160).
- **AI-174** (observed, 00471c00, 00471b00, 0048e420). **Stun level**: clamped to 0..300; > 69 → unconscious:
  all orders dropped, the stars (AI-084), the adversaries dropped, the AI event "lose consciousness" (3.3.2),
  civilians' AI told; < 30 → wakes: "fit again", and for a player the enemies' entries for him are reset so he
  is re-noticed from scratch. **Decay**: with period t from the profile (2.4): **t = 0 disables decay**;
  otherwise, each frame while the level is non-zero: if the counter is 0 the level drops by 1 and the counter is
  loaded with t, else the counter is decremented — one point per t + 1 frames once running (65 for regular
  soldiers; every 4 frames mounted; every 11 antagonists; the player's own word). **Initial phase**: when the
  actor becomes unconscious the counter is loaded with t **only if it is 0**; a residual counter is kept, so
  the first decrement comes after (residual + 1) frames. The console knock-out sets the level to 100. Tied,
  carried (order state 18) or a player in a coma → floored at 30. A comrade's revive subtracts an amount chosen
  by the AI (not read).
- **AI-175** (observed, 00475bd0, 00472070, 0056c700; the effect functions were not read: medium). Body
  actions are player orders gated by the character's abilities (2.4): search (action 122, or 282 when the
  searcher is in posture 10), tie up, carry (two variants), resuscitate, execute; their effects (money taken,
  the tied flag, the carried state) are excluded (9.2).

### 3.12 The bow and other projectiles

#### 3.12.1 Aim validity

- **AI-176** (observed, 0047b760, 0047b8a0 disassembled, 004df070, 004500a0). A player shooter with no
  arrows: invalid (result "no arrows"). Let dx, dy be the target minus the shooter in map px, dz = shooter
  height − target height, D² = dx² + (1.33 × dy)² (**the y displacement is scaled by 1.33**, a single-precision
  constant; not the 1.7434 of the AI's metric), R = the ranged record's aiming reach (range A, or range B when
  the record's flag is set), and *long* = the long mode (a **non-NPC** shooter when the mission's outfit flag
  is set; NPC shooters never). If dz ≤ 0 (the shooter not above the target): reach = R, doubled if long; valid
  iff **D² < reach²** (strict). If dz > 0 (the shooter above): reach = R + tan(0.3 rad) × dz (tan of the
  single-precision 0.30000001192092896 = 0.30933626), **doubled if long — the extension included**; valid iff
  D² < reach² (strict). Shooting downhill extends the reach; no uphill reduction exists. A valid shot is
  classified by the 3-D distance into a shot kind, forced to kind 2 when the shooter's order state is 19, and,
  outside the long mode, to kind 1 when the line of sight (mode 3) is blocked. The pointer's green tail
  additionally requires the element under the pointer to pass the order handler's target test (human targets
  are excluded when they are civilians or arrow-immune; non-human targets can pass). The straw-target shot
  that failed at 186 px from the walkway (h01-measurements-2.md 4) is **not explained** by this rule alone
  (range, height and the target's hit area remain open, 9.2).

#### 3.12.2 Release and the hit roll

- **AI-177** (observed, 0047bb60, 004500c0, 004b4260). The arrow element is created bound to the shooter's
  ranged record with an elevation constant 0.1 (flat) or 0.9 (lobbed) by the shot posture; the initial speed is
  solved for the target distance; a soldier with a signal animation tells his side (the arrow-launched event). **Hit chance**:
  the grid is A when the distance ≤ range A, else B (if the distance exceeds the selected range the chance is
  0); p = trunc(100 × distance ÷ range) (an integer percentage); the band is p < 20 → steps 0..1, < 40 → 1..2,
  < 60 → 2..3, < 80 → 3..4, else 4..5; the lower skill row is 0 if skill ≤ 49 else 1, the upper the next; for
  each of the two rows the value is interpolated linearly within the band by p; then the two row values are
  blended by **skill ÷ 100** (not a fraction within the 0..50 or 50..100 interval); the result is truncated.
  **rand%100 + 1 > chance → miss**: three further rolls jitter the aim point by −2..+2 in x, y and z, and the
  horizontal speed is scaled by 1 − (a record word × a constant). One arrow is removed from the shooter.

#### 3.12.3 Flight, penetration, damage

- **AI-178** (observed, 004b82e0, 004b7b30, 004859b0, 004a18c0 disassembled, 004a6b80). A precomputed list of
  segments; each frame the arrow moves and tests actors on its path (within 15 px of the body point, ignoring
  the shooter and lying / dead actors); the stone (projectile kind 5) can hit a crouched target with the low
  box. **Target guard**: an NPC's arrow never hits a civilian nor a same-side actor; a player's arrow, on easy
  and medium, hits only a non-civilian of the other side; **on hard a player's arrow hits any actor** (allies
  and civilians included). On landing without a hit the arrow becomes a ground pickup and emits a kind-0 noise
  (300 px). **Three target cases** once the guard admits the target: (a) a **civilian**: no penetration roll,
  the damage applies (only reachable on hard, or from an NPC never); (b) a **soldier or player** (an actor with
  a class block): one roll rand%101 and the arrow **penetrates iff roll > the target's class head word 10**,
  else it is deflected (a word of 100 always deflects; a word of 0 deflects on roll 0, one in 101); the knight
  classes carry 100; (c) an **immune** (principal-enemy) NPC target is recognised **before** the penetration
  test: the arrow does nothing to it and **no penetration draw is made** (zero draws on this path; the guard of
  the shooter kind still runs first). A deflected arrow bounces
  and re-launches; a penetrating one deals **damage = the record's damage word** (A or B by the arrow's flag;
  100 / 75 / 10 / 20 for records 1..4's A words), no stun, no defence roll; a kill gives the shooter 20 bow
  experience; civilians hit tell their AI. Gravities: −5.6 / −6.4 / −4.0 for the arrow kinds, −0.8 / −7.2 for
  the stone.
- **AI-179** (observed, 0049f520, 0049ec50, 004a74f0). **Ammunition**: per player a counter per item kind with
  a maximum; at 0 the icon is disabled and a remark plays; soldiers keep one count; arrow piles add their stack.

### 3.13 Purses and wasp nests

- **AI-180** (observed, 004b9b70, 004b9220, 0048cca0). A thrown **purse** flies on the projectile code; on
  landing it bursts into coin pickups scattered at random bearings (16 sectors) 10..41 px away (up to 7 tries
  per coin for a reachable spot; rolls per coin) and emits a **kind-9 noise (200 px)**; the sees-object / hear
  path (3.5.3) makes susceptible soldiers go for it and brawl.
- **AI-181** (observed, 004be0e0, 004be1a0, 004bc660). A **wasp nest** spawns 20 wasps on landing, each flying
  at 5 px per frame with random jitter toward its target actor, chasing within 25 px (75 px while the target's
  alert timer runs); a sting raises the wasp event (3.3.2); wasps vanish when the nest's counter runs out.

### 3.14 The player's orders

- **AI-185** (observed, 0058a3d0, 0058bbe0, 0046bcb0, 00412240; medium). A new movement order first
  **stops** what the actor is doing (its running actions are discarded) and then queues its own steps: a
  stand-up when the actor is lying or kneeling, the move (a path search across layers), an optional turn and
  follow-up action. The stop reasons seen: script / AI, a cancelled special mode, AI locked. **Excluded**
  (9.2, hand-off boundary: the implementer must not infer them from this file): the right-click cancel, the
  Ctrl queueing of a second order, and the dispatch of the left click by pointer mode to the context actions.

## 4. Claims

One row per claim id cited above (the prose is the full statement). Evidence is a function address unless it
names a data probe or a recording.

| Id | Claim (one sentence) | Status | Evidence | Confidence | Notes |
|---|---|---|---|---|---|
| AI-001 | one logic frame per main-loop iteration; a 40 ms wait on reported OS time (400 ms in debug slow motion), realised as 46.875 ms on the measured host | observed | 0050f710, 004c6ef0; stealth-and-combat.md 8.4 | high | host-dependent; see section 8 |
| AI-002 | the script's per-second callback runs every 25 frames | observed | 004c6ef0, 00404180 | high | VM-100 |
| AI-003 | two per-NPC timers; AI 0 → 1, rail wait keeps 0; expiry at deadline or > 10⁶ frames ahead; frozen with the emoticon expiry while locked / out / paused | observed | 00416710, 0048b450, 0048b4e0, 0048a980 | high | |
| AI-004 | id-keyed staggers: think 16, identification 16, hearing 3, energy frame mod 64 = id mod 32 | observed | 0048a980, 00471b00, 00487d00 | high | |
| AI-005 | one global LCG stream 214013 / 2531011, bits 16..30 | observed | 00642a7d | high | |
| AI-006 | seeded from the wall clock at start and at every save; the seed is saved and reapplied on load | observed | 0040a230, 004148b0 | high | |
| AI-007 | perception consumes no random numbers; rail programs use a cached 1..100 roll | observed | 00487d00, 00419c50, 00419c80 | high | |
| AI-010 | side = SD flag bit 0 / civilian attitude / player | observed | 004a5d30, 0056c410 | high | |
| AI-011 | rank 0 / 1 / 2; civilian kinds 0..5 | observed | 0056aee0, 0056c490 | high | |
| AI-012 | the character-action enumeration 0..30 | observed | 0056c700, 0048fa30 | high | |
| AI-020 | the SD numeric fields mean what 2.4 says (layout from the serialiser; meanings from consumers) | observed | 0056a7d0 and the listed consumers | high | stimulus order disputed by review 16 (9.1) |
| AI-021 | the PC numeric fields mean what 2.4 says | observed | 00565490, 0048fa30, 0055bf00, 00488f60 | high | |
| AI-022 | CV kind and attitude words | observed (loader) | 00564ea0, 0056c490, 0056c410 | medium | consumers not located |
| AI-023 | the data checks of 2.4 hold on all records | observed | `cpf_stats.py`, `cpf_probe.py` on the player's file | high | |
| AI-030 | the class head words: reach bands, directional defence, stun resistance, arrow penetration, pole / shield flags | observed | 00568e00, 0047ab00, 005c1b50..005c1fb0, 005c1b70, 004a18c0 | high | |
| AI-031 | the row layout: target, stun, damage, reach, kind, side, angles, step, energy | observed | 00568e00, 005c1db0, 005c1dd0, 00475bd0 | high | |
| AI-032 | the hero's two outfits use two classes; the town class damages, the forest class stuns | observed (data) | the class blocks of the two hero records | high | |
| AI-035 | the ranged record layout and the distance-major grids; damage words consumed | observed | 00569f40, 004502e0, 004500c0 | high | |
| AI-040 | per-human combat state of 2.7 | observed | 00471b00, 00471c00, 0047c600, 0047dc00 | high | |
| AI-041 | per-NPC AI state of 2.8 | observed | 00414be0 and consumers | high | |
| AI-042 | per-NPC perception state of 2.9 | observed | 004863f0, 00486fa0 | high | |
| AI-043 | the `BORG` fields' targets | observed | 004a1e90, 0048b5d0, 00449170, 0044cae0 | high | loot threshold low |
| AI-045 | difficulty 0..2 exists with the listed consumers; the hostile-only scaler | observed | 0055dbb0, 0050b640, 00438710, 00438600 and the consumers | high | contradicts ANIM-523 |
| AI-046 | a "no player death" option byte exists; writer unknown | observed | 00482150, 00471c00 | medium | |
| AI-050 | the per-frame order of 3.1 | observed | 004c6ef0, 0048a980, 00487d00 | high | |
| AI-060 | base sight range 400 / 300 by level lighting | observed | 004c1ab0, 0048ebe0 | high | |
| AI-061 | head turning, mode angles, 0.5 rad half-angle, focused ×1.4 / 0.35 | observed | 00486fa0 | high | one ×1.4 condition unidentified |
| AI-062 | collapse and recovery ramps | observed | 00486fa0 | high | |
| AI-063 | drunk wobble formulas | observed | 00486fa0 | high | |
| AI-064 | the 0.5736 / 1.7434 y metric; the drawn cone is the perception cone | observed | 00486fa0, 0058e7f0 | high | oracle agrees on the angle; reach 78..86 % (hypothesis) |
| AI-065 | line of sight through the `.rhp` sight polygons with the height test; 2000-slot per-frame cache | observed | 004eff80, 004eef30, 004f0230, 004ef340, 005a3810 | high | collision quirk |
| AI-066 | the graded visibility steps 1..6 | observed | 00489680, 00489eb0, 00489d00, 0048db80 | high | posture codes unnamed |
| AI-067 | friendly observers' simple test with the outfit flag | observed | 00489680 | medium | |
| AI-068 | per-category values, periods, ×20 / ×200 truncated contribution, cached values contribute | observed | 00488f60, 00489b30, 00487d00 | high | |
| AI-069 | shadow at acc ≥ 100 before adding; emission at ≥ 1000 or immediate; category 0 kept as reported, 1..5 removed; out-of-view; decay 1 per 20 frames | observed | 00487d00, 00488e00, 00488b80, 004887a0 | high | |
| AI-070 | how the candidate lists are filled; category 4 = the watched element | observed / inferred | 0048ebe0, 0048c700, 0041a300, 0043a220, 0048c620 | medium | search membership open |
| AI-071 | one-shot noise radii by kind; the margin test; coincident positions hear nothing | observed | 0048cca0, 0048cc20, 0048cec0 | high | |
| AI-072 | the walk-noise radius by action and material; unlisted materials keep the previous radius; hearing every 3rd frame | observed | 0047ef70, 00487d00, 00488a40 | high | |
| AI-073 | deafness decay and loud-sound geometry | observed | 0048a830, 005b3c40 | high | |
| AI-074 | identification at 1.5 / 1.95 × range with difficulty and line of sight | observed | 00487d00, 004a0a30, 00570e70 | high | oracle 120 px unexplained |
| AI-075 | hidden states by action 242 and order states 21..23 | observed | 00489680, 00488f60 | medium | |
| AI-076 | nappers process noises, not sightings | observed | 00410620, 00433830 | high | |
| AI-080 | the event-driven model and its sources | observed | 00424bc0, 00410620, 0040d980 | high | |
| AI-081 | the event kinds 0..70 | observed | 0041f540 | high | |
| AI-082 | the pre-filter rules a..g | observed | 00410620 | high | |
| AI-083 | the hostile 16-frame poll | observed | 00434aa0 | high | |
| AI-084 | the head marker values and the stars count | observed | 00419fb0, 00419fd0, 0057aed0, 00486f30, 005c3fc0 | high | |
| AI-090 | the back-and-forth rail cursor | observed | 005505c0, 00550f60 | high | |
| AI-091 | table selection by direction; the cached roll; the pass-through pre-check | observed | 00410de0, 00550660, 004121a0, 00419cc0 | high | |
| AI-092 | the opcode table; 0x01 advances twice; illegal uses execute anyway | observed | 00410010, 005505c0 | high | |
| AI-093 | the walking flags; run = the sprint cycle | observed | 00412240, 0041bd20, 00576e00 | high | |
| AI-094 | check-for: the look loop and the give-up | observed | 00439080, 00424d50, 004444a0 | high | |
| AI-095 | check-for-sync | observed | 00439080, 00410de0 | high | |
| AI-096 | resuming at the nearest waypoint; the alert rail | observed | 00418600, 00424d50, 0044cae0 | medium | |
| AI-097 | company following | observed | 0041a960, 0041ac50, 00449170 | medium | slot geometry unread |
| AI-100 | the reaction delay formula with m = 1 medium / 2 easy, hard; the flat 3 for friendly NPCs | observed | 00438620 (disassembled), 0055dbb0 | high | revision 1 corrected |
| AI-008 | the arithmetic contract (single storage, extended intermediates, truncation) | observed | 00488f60, 00487d00, 00438620, 0047b8a0 (disassembled) | high | |
| AI-101 | return to duty leaves the current state for the default state and the actor resumes post / route / patrol | inferred | 00424d50 | medium | the reset body not isolated; hand-off boundary in 9.2 |
| AI-102 | creation-time scaling and the placed-action initial states; action 48 = stun 300 | observed | 00435c80, 00414550 | high | |
| AI-103 | look-around intervals | observed | 0041a420 | high | |
| AI-104 | company relay | observed | 0044d750 | high | |
| AI-105 | the sighting reaction (elevation limit, seen clearly, the shout, the three timers) | observed | 00433060, 00439cb0 | high | |
| AI-106 | the noise reactions by kind | observed | 00433830 | high | |
| AI-107 | the body reaction | observed | 00434320 | high | |
| AI-108 | the object reactions (ale, purses) | observed | 00434710 | high | |
| AI-109 | the shadow reaction | observed | 00433790 | high | |
| AI-110 | the arrow reaction | observed | 00433f90 | high | |
| AI-111 | look-there, tower-guard calls, stone, stop | observed (the officer's tower-guard routine excluded) | 00439e70, 00439f60, 0043a080, 004457c0 | high | |
| AI-112 | looking around (three legs) | observed | 00424d50 | high | |
| AI-113 | the glance kinds | observed | 00424d50, 0043d890 | high | |
| AI-114 | purse, brawl and looting; the continuation rule | observed (the continuation operands inferred) | 00424d50, 00445d40, 004460f0, 004468a0, 0044b000, 00446400, 004469e0 | high / medium | |
| AI-115 | ale | observed | 00424d50, 00449df0 | high | |
| AI-116 | apple, wasp, net | observed | 00424d50, 0044cb40, 0040e7e0 | high | |
| AI-117 | whistle | observed | 00424d50, 00443d10, 00443af0 | high | |
| AI-118 | heard steps | observed | 00424d50, 00438cb0 | high | |
| AI-119 | the area search (costs, freshness, cooling, rolls) | observed | 0043a220, 0043d0c0, 0043d530, 00522330, 00416800, 00438600 | high | seek-point origin open |
| AI-120 | the next seek point, beggars | observed (the betrayal roll excluded) | 0043d530, 00424d50 | high | |
| AI-121 | reporting to an officer; groups; civilians' reports | observed | 0043c960, 00424d50 | high | |
| AI-122 | bodies, waking sleepers, nets | observed | 0044a760, 00424d50, 0043ccc0, 0043cec0, 0044cf00, 0044a830 | high | |
| AI-123 | the combat-alert call | observed | 00424d50 | high | |
| AI-124 | could not reach; impossible | observed | 00424d50 | high | |
| AI-125 | sleeping rules | observed | 00424d50, 00471c00, 00414550 | high | |
| AI-126 | fleeing rules | observed | 00424d50, 0040e7e0, 00413240 | high | |
| AI-127 | civilian AI | observed | 0040bfd0, 0040c140, 0040cc60, 0040d150, 0040e290 | medium | attitude branches skimmed |
| AI-130 | no global alarm; the seven propagation paths | observed | 3.5's functions | high | |
| AI-140 | the decision's inputs, priority and outcomes | observed | 004303e0, 004207f0, 00438cb0 | high | |
| AI-141 | the morale formula | observed | 00432d30 | high | |
| AI-142 | target choice and its penalties | observed | 0043d780, 0042fe00, 0043ea70 | high | |
| AI-143 | no cap on attackers but the observe / join rules | observed | 00442470, 004303e0 | high | |
| AI-144 | execution of the outcomes | observed | 004303e0, 00432f80, 0042ef80, 0042fe00, 00445210, 0043c960 | high | |
| AI-145 | officers, too proud, knights | observed | 0044a090, 00424d50 | high | |
| AI-146 | archers and crossbowmen | observed | 00424d50, 00445510, 00445180, 0044b4e0 | high | |
| AI-147 | shield bearers and the phalanx | observed | 00447e50, 0044dd70, 00447070 | medium | |
| AI-148 | riders | observed | 0044c850, 0044ca90, 00424d50 | medium | |
| AI-149 | door fights, ladders, sleeping enemies; menacing entered | observed (menacing's timers unknown) | 0044b940, 0044bc70, 00424d50 | medium | |
| AI-150 | the sword-fight step's ordered outcomes | observed | 0043db20, 00441650 | high | |
| AI-151 | figure choice: the attack roll, admissibility by experience / hesitation, ≥ 2 opponents for sweeping figures, the scoring | observed | 00485240, 00485590, 00482240, 00482fd0, 00440cf0 | high | thresholds excluded (9.3) |
| AI-152 | the reaction to a strike | observed | 00442190 | high | |
| AI-153 | no swing timer; the 5.3 s not derived | inferred | 0043db20, 00485240 | medium-low | |
| AI-160 | engagement rules; the victim auto-engages; no punch from the front | observed | 0047c600, 0047c4c0, 0047f520, 004801d0, 004808f0, 00472070 | high | |
| AI-161 | the auto-fight decision: the 10 % gate on a non-mutual opponent, the inclusive hand-over roll, the grace flag | observed | 0047dc00, 0047e1e0, 0047f420 | high | not measured |
| AI-162 | disengagement | observed | 0047d270, 0047ca50 | high | |
| AI-163 | several attackers | observed | 0047dc00, 0043db20, 0043ea70 | high | |
| AI-164 | who is struck by kind; quick strikes carry no row | observed | 00475bd0, 00482fd0, 00482e50, 004801d0, 00482600, 004828b0, 00482390, 00480e90, 00482b90, 00483070, 00481ef0 | high | |
| AI-165 | rows ↔ sprite actions | observed | 00475bd0, 004820e0 | high | |
| AI-166 | the resolution order: defence roll from the victim-to-attacker bearing, damage with the rank-0 factor, the independent stun roll with the zero-health guard, the trailing experience roll | observed | 0047e7d0, 005c1b70, 005c1dd0, 005c1db0, 00471ed0, 00471e50, 00472070 | high | |
| AI-167 | the block rests and ends at an animation boundary after fatigue 0 | observed | 00481d40, 00472070 | high | |
| AI-170 | energy 100; recovery rate ÷ 10 on frame mod 64 = id mod 32 while still; walk-completion recovery; no cap | observed | 00475bd0, 00471b00, 004a1cf0, 0048f9d0 | high | oracle discrepancies kept |
| AI-171 | the HUD quantities | observed | 004d5a90, 00482150 | high | |
| AI-172 | death, the falls, easy-only passive regeneration, the flagged recovery | observed | 00482150, 0049fa20, 00474d60, 00485e90, 004748f0, 004936f0 | high | flag writer unknown |
| AI-173 | the punch's stun amounts (80 / 150 / 40 / 3; × 1.5 on hard, truncated), applied without a roll | observed | 00475bd0 (disassembled at the difficulty branch), 00472070, 00471e50, 00471ed0, 00463da0 | high | |
| AI-174 | stun thresholds, t = 0 disables decay, t + 1 frames per point, floors, wake | observed | 00471c00, 00471b00, 0048e420 | high | |
| AI-175 | body actions exist as gated orders | observed | 00475bd0, 00472070, 0056c700 | medium | effects excluded |
| AI-176 | aim validity: no arrows invalid; downhill extends the reach; long mode for non-NPC shooters with the outfit flag; classification | observed | 0047b760, 0047b8a0, 004df070, 004500a0 | high | straw target open |
| AI-177 | the hit roll: truncated percentage bands, blend by skill ÷ 100, three jitter rolls on a miss | observed | 0047bb60, 004500c0 | high | |
| AI-178 | flight; the target guard by shooter kind, side and difficulty; penetration iff roll > word 10; damage by record | observed | 004b82e0, 004b7b30, 004859b0, 004a18c0, 004a6b80 | high | |
| AI-179 | ammunition | observed | 0049f520, 0049ec50, 004a74f0 | high | |
| AI-180 | purses | observed | 004b9b70, 004b9220, 0048cca0 | high | |
| AI-181 | wasp nests | observed | 004be0e0, 004be1a0, 004bc660 | high | |
| AI-185 | a new movement order stops the running actions and queues its steps | observed | 0058a3d0, 0058bbe0, 0046bcb0, 00412240 | medium | cancel, Ctrl queueing, context dispatch excluded (9.2) |
| AI-190 | the three callbacks, the source payload, the renumbering with −2 for unmapped ids | observed | 00403190, 004032a0, 00408750, 00410620, 00464230 | high | |

## 5. Constants

Individual functional facts; curated tables are not reproduced (the per-figure admissibility thresholds of
AI-151 are excluded, 9.3).

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| FRAME_WAIT_NOMINAL | 40 (400 debug slow motion) | ms of reported time | 0050f710 | high |
| FRAME_REALISED (oracle host) | 46.875 | ms | stealth-and-combat.md 8.4 | high (host-dependent) |
| SCRIPT_SECOND | 25 | frames | 004c6ef0 | high |
| RNG | 214013 / 2531011, bits 16..30 | – | 00642a7d | high |
| SIGHT_BASE_RANGE | 400 / 300 by lighting word | px | 004c1ab0 | high |
| CONE_HALF_ANGLE | 0.5 (wide 1.5208; focused 0.35) | rad | 00486fa0 | high |
| HEAD_TURN_SPEED / LIMIT | 0.3927 / 0.8 (1.3 focused) | rad per frame / rad | 00486fa0 | high |
| FOCUSED_RANGE_FACTOR | 1.4 | – | 00486fa0 | high |
| Y_COMPRESSION | 0.5736 (inverse 1.7434) | – | 00486fa0, 0043d780 | high |
| NEAR_OMNI_RADIUS | 60 (the front 9/16 of the circle) | px | 00489eb0 | high |
| POINT_BLANK | 20 | px | 00489680 | high |
| FADE | 1 − r/6; 0.95 − 1.75 (r − 0.3); 0.25 − (r − 0.7)/3 | – | 00489d00 | high |
| POSTURE_FACTORS | ×3 (codes 2, 8, 14), ×20 (3, 9), ×2 (4..6), ×1.5 (7, 10, 11), ×0.5 (order state 10) | – | 00489680 | high |
| CATEGORY_PERIODS / WEIGHTS | 2 (W·2g·0.01), 4 (4g'), 8 (8g; 24g bodies), 16 (16g) | frames | 00488f60 | high |
| CONTRIBUTION_SCALE | 20 (200 in the wide mode), applied to the single-precision stored value, truncated | – | 00487d00 | high |
| PERCEPTION_VALUE | (2g) × W × 0.01 (single-precision 0.01), stored as single | – | 00488f60 | high |
| SHADOW_THRESHOLD / EMIT_THRESHOLD | 100 / 1000 | accumulator | 00488e00, 00487d00 | high |
| FORGET_RATE | −1 per 20 frames | – | 00487d00 | high |
| NOISE_RADII | 300 / 70 / 50 / 500 / 200 / 400 / 0 by kind (AI-071) | px | 0048cc20 | high |
| WALK_NOISE | AI-072 | px | 0047ef70 | high |
| DEAFNESS_DECAY | 10 per frame (< 301), else 50 × trunc(v/300) | px | 0048a830 | high |
| IDENTIFY_FACTOR | 1.5 (1.95 in order state 16) × 1.3 easy / 0.7 hard | × base range | 004a0a30 | high |
| LOS_CACHE | 2000 slots per frame | – | 004eff80 | high |
| STAGGERS | think 16, identification 16, hearing 3, energy frame mod 64 = id mod 32 | frames | 0048a980, 00471b00 | high |
| REACTION_DELAY | trunc((100 − p1) × 0.01f × base × m + 1); m = 1 medium, 2 easy / hard (hostile); friendly flat 3 with the outfit flag | frames | 00438620 | high |
| LOOK_INTERVAL | 70 + rand%70; officers 200 + rand%600; p4 ≠ 0 400 + rand%800 | frames | 0041a420 | high |
| SHOUT_RADIUS | 100 (an officer's arrow 200) | px | 00439cb0 | high |
| ELEVATION_LIMIT | 100 | px | 00433060 | high |
| SEEN_CLEARLY_DEPTH | 50 | px | 00433060 | high |
| SIGHT_REACTION | 5 seen clearly / 20 turning / 10 rider; < 30 px immediate | frames / px | 00433060 | high |
| NOISE_REACTIONS | 50 quiet; 60 ×m steps; 60 whistle watch; 50 ×m heard-whistle; 70 + rand%60 kinds 11 / 12 | frames | 00433830 | high |
| BODY / OBJECT / ARROW REACTIONS | 80 ×m; 60 ×m ale; 30 purse (officer 60); 100 ×m arrow (officer 60) | frames | 00434320, 00434710, 00433f90 | high |
| LOOK_THERE_WATCH | 100 | frames | 00439e70 | high |
| LOOK_AROUND_LEG | 30 + rand&7, turn +5/16 | frames | 00424d50 | high |
| SEEK_WATCH / WALK_LEG | 20 per direction / 200 | frames | 00424d50 | high |
| SEEK_COOLDOWN | 5000 per visit, cap 10000 ahead | frames | 00522330 | high |
| SEARCH_RADII | 300 usual; 400 missing colleague (200 with a patrol); 500 neighbour test | px | 0043a220, 0043cec0 | high |
| REPORT_DISTANCE / OFFICER_COST | 70 px / distance + 100 per layer (civilians 250) | px | 0043c960, 0040e290 | high |
| OFFICER_PATIENCE | 101 polls × 30 | frames | 00424d50 | high |
| BRAWL_FINISH / STARE | 200 / 300 + rand%32 | frames | 00424d50 | high |
| CHECK_FOR | skip if the alert < 3000 frames old; looks = N/10 + 1; interval 1000/looks; give up at acc > 1000 | – | 00439080 | high |
| PATROL_WAIT | 200 | frames | 00424d50 | high |
| ENGAGEMENT_DISTANCE | 150 (head word 3) | px | 0047c600 | high |
| HEIGHT_TOLERANCE | 40 | px | 0047c600 | high |
| FIGHT_STEP | 20 | frames | 0043db20 | high |
| MANOEUVRE_CHANCE | 1/3 | – | 0043db20 | high |
| ATTACK_CHANCE | max(skill, 50) % | – | 00485240 | high |
| SWEEP_MIN_OPPONENTS | 2 | – | 00485240 | high |
| OBSERVE_RULE | (courage × 0.045 + 1) × n ≤ E | – | 004303e0 | high |
| MORALE | AI-141 | – | 00432d30 | high |
| NON_MUTUAL_ACT_CHANCE | 10 % per frame | – | 0047dc00 | high |
| DEFENCE_ROLL / STUN_ROLL | rand%99 + 1 > word | – | 0047e7d0 | high |
| DAMAGE_FACTOR | 1 + 0.01f × experience (rank-0 soldiers only) | – | 005c1dd0 | high |
| STUN_GAIN | stun × 100 ÷ current hp (hp ≠ 0) | – | 00471ed0 | high |
| STUN_THRESHOLDS | ≥ 70 out, < 30 awake, cap 300, floor 30 tied / carried / coma | – | 00471c00 | high |
| STUN_DECAY | 1 per (t + 1) frames; t = 0 none | – | 00471b00 | high |
| PUNCH_STUN | 80 / 150 hard punch / 40 NPC / 3 execution; × 1.5 on hard, truncated | – | 00475bd0 | high |
| AIM_Y_SCALE / AIM_ANGLE | 1.33 / 0.3 rad (tan 0.30933626), single-precision constants; strict reach comparison | – | 0047b8a0 | high |
| ENERGY_MAX / RECOVERY | 100 / rate ÷ 10 per 64 frames while still | units | 00471b00 | high |
| BLOCK_REST | −5 fatigue per frame | – | 00481d40 | high |
| STARS | min(4, stun ÷ 50) + 1 | – | 005c3fc0 | high |
| BOW_HIT | AI-177; rand%100 + 1 > chance | – | 004500c0, 0047bb60 | high |
| ARROW_HITBOX / PENETRATION | 15 px / rand%101 > word 10 | – | 004b7b30, 004a18c0 | high |
| AIM_DELAY (archers) | (110 − ranged skill) ÷ 2 | frames | 00424d50 | high |
| PURSE_SCATTER | 10..41 px, 16 bearings, 7 tries | – | 004b9220 | high |
| WASPS | 20 per nest, 5 px/frame, chase 25 (75) | – | 004be0e0, 004bc660 | high |
| DIFFICULTY | AI-045 | – | 00438710 and consumers | high |

## 6. Interfaces to the script VM

### 6.1 Natives (ids as in scb.md)

Common contract (observed on every body read: 00575e20, 00579b10, 00576d80, 00576e00, 005794b0, 00579520,
00577500): arguments are VM integers; an actor argument is an element handle that is first validated (an
element of the level that is a character); **on any failure the native logs a script error, has no side
effect and returns 0** unless the row says otherwise; none blocks or yields; no native below consumes a
random number. "NPC" means a non-player human (soldier or civilian); a player character is rejected where the
row says so.

| Id | Arguments → result | Contract | Status / source |
|---|---|---|---|
| 85 | (x) → bool | 1 iff x is null (no element) | observed, rails reader |
| 87 | (actor) → bool | 1 iff dead | observed (virtual predicate) |
| 88 | (actor) → bool | 1 iff unconscious (AI-174's flag) | observed |
| 89 | (human) → bool | accepts any human (player characters included — a broader type test than 134 / 135); 1 iff the actor's **order state** is 18 (the tied / carried state; which of the two is 9.2); a non-human or invalid handle → error, 0 | observed, 00579b10 |
| 90 | (actor) → bool | dead or unconscious or (order state 18) | observed |
| 99 | (actor) | reveal the silhouette (AI-074); returns nothing | observed, 00570e70 |
| 102 | (actor, amount, flag) → bool | the actor must be one of the current sequence's declared actors and a character, else error and 0; queues a hit step of `amount` with a second parameter 100 when flag ≠ 0 else 0 (its meaning 9.2); returns 1 | observed, 00577500 |
| 126 | (npc) → int | the top-level state for scripts: **0 asleep** (unconscious, napping, dead for good), 1 on duty, 2 curious, 3 searching, **4 menacing, 5 fleeing, 6 attacking** (review 22 upheld this reading); a non-NPC → error, 0 | observed, 00575e20 |
| 128 | (npc) → bool | 1 iff hostile (the profile's side), not "can act" | observed |
| 130 | (npc, target, flag) | no effect in this build | observed, 00486f50 |
| 134 | (npc, flag) | lock the NPC's AI: the locked flag set, the "queue while locked" flag := flag, the actor **stopped unless its current action is the block stance (127)** (stop reason "AI locked"), the program cleared; an object element (category 0) gets a byte set instead; **a player character → error, no effect** | observed, 00576d80, 00418b80 |
| 135 | (npc) | unlock: only when the NPC's "out" flag is set (else error, no effect): enemies are told he is fit, the locked flag cleared, and unless the actor is unconscious, has no AI target, or one of its pending sequence steps is a move, return-to-duty is posted; an object element clears its byte; a player → error | observed, 00576e00, 00418bc0 |
| 140 | (npc, style) | NPC only (a player → error, no effect); style 0 → walking flags := walk, 1 → run, then the current rail leg is re-issued; **any other style writes address-derived bits** (AI-093: only 0 / 1 cleared; OpenSherwood: error, no change) | observed, 005780d0, 0041bd20 |
| 160 | (loc a, loc b) → int | both arguments must be non-null **location** handles (a null or a non-location → error, 0); result = trunc(the plain euclidean distance between the two locations' points, in px, no y scaling) | observed, 00571b80 |
| 176 | (soldier, n) | **soldiers only** (a civilian, a player or an object → error, no effect); stores n as a 16-bit company number (2.10) | observed, 005790b0 |
| 177 | (soldier, flag) | **soldiers only** (else error); "always attentive": when set and no target, perception is re-run at once | observed, 005790f0, 00444440 |
| 197 | (npc, k) → int | the NPC's custom value k, k in 0..9; **k outside 0..9, a null / non-character handle or a non-NPC → error and −1** | observed, 005794b0 |
| 198 | (npc, k, v) | write custom value k := v, k in 0..9; invalid k / handle / non-NPC → error, **nothing written** | observed, 00579520 |
| 218 | (chief, npc) | add npc as a subordinate of chief; rejected (error, no effect) when either handle is not an NPC, when npc already has a chief, when npc has subordinates of its own, when chief is on a patrol, or when chief and npc are the same actor | observed, 0057aa70 |
| 219 | (chief) | remove all subordinates of chief; rejected when the handle is not an NPC | observed |
| 220 | (soldier) | **soldiers only** (else error); if the alert rail (2.10) is not −1 it becomes the rail; **in every case** the actor is reset to the default state (an actor already on duty is reset again even without an alert rail) | observed, 0057ad30, 0044cae0 |
| 228 | (npc, id, frames) | set the emoticon: 0 clears; 1..7 → the markers 2..8 of AI-084 for `frames` frames | observed, 0057aed0 |
| 235 | (item) → bool | picked up (unchanged from scb.md) | observed |
| 240 | (actor) → bool | active | observed |
| 59 | (actor, step kind, parameter) → bool | **excluded from this file**: it records a step of the given kind into the cinematic sequence being declared (error and 0 when no declaration is open or the actor is invalid); it is a sequence native of `spec-script-vm.md`, not an AI native, and not "archer shoots" | observed as far as stated, 00573450 |

scb.md's readings of 128, 140, 177, 197 / 198, 219 / 220 and 228 are superseded. **Excluded from clearance**
(the meaning is observed but the accepted types, coercions and failure effects were not read for them): 85,
87, 88, 90, 99, 128, 130, 228, 235, 240 — an implementer applies the common contract and records an
`Assumption` for their failure behaviour. The complete id → function map stays in the analyst workspace.

### 6.2 Callbacks (required compatibility tokens)

- **AI-190** (observed, 00403190, 004032a0, 00408750, 00410620, 00464230). The engine invokes three script
  callbacks by name on actor classes; the names `ActionChange`, `ReachPoint` and `FilterAIEvent` are
  **required compatibility tokens** (the compiled scripts bind to them). `ActionChange(new, previous)` is
  invoked with the actor as the VM's implicit context whenever an actor's action id changes, with the new id
  first and the previous id second (a "none" id when no action runs); `ReachPoint()` when a script-flagged
  waypoint is reached (AI-091); `FilterAIEvent(source, event)` for actors flagged for scripting (and only
  while the launcher's script flag is set), before the pre-filter, with the receiving actor as the implicit
  context, `source` = the event's source element when the event's payload is an element, otherwise null, and
  `event` = the **script-visible value** bracketed in AI-081 for that event, or **−2 for every event AI-081
  marks −2** (the shot-at-by-a-player event, the strike-quality events, enemy near, stop, the brawl clean-up
  call and the dialogue beats) — the callback is still invoked for them. A zero return drops the event. Hence
  the ids the retail scripts compare — 0, 2, 8, 11, 13, 14, 22, 23, 31, 33, 34, 52 — mean: 0 an enemy comes
  into the cone, 2 a noise heard, 8 a body seen, 11 a friend in trouble seen, 13 got hit, 14 loses
  consciousness, 22 enters a sword fight, 23 quits a sword fight, 31 an arrow landed, 33 the alert call, 34 the
  combat-alert call, 52 a shadow seen (and, for the record, a soldier seen is 10, an incoming strike 24).

## 7. Acceptance tests

Each test is a synthetic input with the expected result; widths and rounding as in section 2. "Frame f" counts
from 0 at the actor's creation unless stated.

1. **Accumulation and the shadow** (AI-068, AI-069, AI-008). One hostile soldier with element id 0 created
   on frame 0, normal mode, base range 400, in the default state, category 0 holding one town-outfit player
   (W = 80) whose graded value is exactly g = 0.5 on every evaluation (the test fixes g; the geometry that
   yields it is the tester's choice) and who is not engaged; the accumulator starts at 0. The entry is evaluated
   on the frames where (0 + frame) mod 2 = 0, i.e. frame 0, 2, 4, …; the stored single is 0.79999995231628418
   and every frame (evaluated or cached) contributes trunc(0.79999995 × 20) = 15. After frame k the accumulator
   is 15 (k + 1). The shadow test on frame 7 finds acc = 105 ≥ 100 before the add, the contribution non-zero
   and the flag clear → **one shadow event on frame 7**, none later while the conditions hold. acc ≥ 1000
   first holds after the add of **frame 66** (1005) → the view event on frame 66, acc := 0, the entry marked
   reported and kept. Wide mode (contribution 159): shadow on frame 1 (acc 159 before the add), view on frame 6
   (1113). Negative: the same player playing action 242 contributes 0 and nothing fires; the player in a
   no-sight zone the soldier is not in contributes 0. **Observer-id variant**: a soldier with id 1, the entry
   newly added on frame 0 (its cached value starts at 0, no forced re-evaluation): the first evaluation is on
   frame 1, frame 0 contributes 0, so the accumulator after frame k is 15 k; shadow on **frame 8** (acc 105
   before the add), sighting on **frame 67** (1005); with the cache pre-populated to the stored single before
   frame 0 the frames are those of the id-0 case.
2. **Category lifecycle** (AI-069). A body entry (category 1) visible to an alerted (seeking) hostile: its
   first non-zero contribution frame emits sees-body and removes the entry; a category-0 entry that was
   reported and is no longer visible emits out-of-view and becomes unreported, and is not removed.
3. **Hearing boundary** (AI-071, AI-072). A purse lands (kind 9, R = 200) at distance 199.4 px (3-D) from a
   deafness-0 hostile: margin = trunc(0.6) − 0 = 0 → not heard; at 198.5 px margin 1 → heard with margin 1; a
   listener exactly at the source hears nothing; deafness 5 makes the second case not heard. A player playing
   the sprint (action 10) on material class 1 has radius 150; on class 5, 400; playing action 33 on any material
   50; playing action 127 on class 9 keeps the radius of the previous frame.
4. **Timers, pause and lock** (AI-003, AI-050, AI-082). An AI timer armed with 0 during frame 10's think
   phase has deadline 11 and fires in frame 11's check phase; a rail wait of 0 armed on frame 10 has deadline
   10 and fires in frame 10's check phase if armed before it, else frame 11's. A timer armed with 50 on frame
   10 (deadline 60) with the AI locked on the actor's ticks of frames 20..39 inclusive (the half-open interval
   [20, 40): 20 skipped ticks) has deadline 80 and fires in frame 80's check phase; the same with the level
   flagged paused on those 20 ticks; with the **main loop** paused instead, no frame advances and the timer
   fires on the 60th frame that actually runs. A timer launched in sub-state X delivered while the sub-state
   is Y → dropped; a deadline set to frame + 2,000,000 fires at the next check.
5. **Reaction delay across difficulties** (AI-100, AI-008). Hostile, base 60, initiative 10: easy → 108;
   medium → 54; hard → 108; initiative 0 on medium → 60; initiative 100 → 1 on every difficulty; a friendly,
   non-mounted NPC with the outfit flag set → 3 regardless; a friendly NPC without that flag on hard, base 60,
   initiative 10 → 54 (m = 1 for the friendly side).
6. **Difficulty scaling** (AI-045). A hostile soldier with `p0` = 80, `q1` = 60: hit points 40 / 80 / 120 and
   experience 30 / 60 / 100 (capped) on easy / medium / hard; a friendly soldier keeps 80 / 60 on every
   difficulty; a player's stock of 12 becomes 15 / 12 / 9.
7. **Rail endpoints** (AI-090, AI-092). A three-point rail, cursor at index 2 forward: an ordinary advance →
   index 1 backward; opcode 0x01 there → advance to 1 backward, then again to 0 backward → the actor walks to
   point 0 (the flag stays backward); at index 0 backward a plain advance → index 1 forward. A two-point rail at
   index 1 forward with 0x01 → 0 backward then 1 forward: the actor returns to point 1.
8. **Directional defence** (AI-166). Victim facing 4 (east in sixteenths), attacker due east of him: bearing b
   = 4, straight figure → d = 0 → head word 5 (front); a right attack → b − 4 = 0 → d = 12 → word 6 (left); a
   left attack → b + 4 = 8 → d = 4 → word 8 (right); attacker due west (b = 12), straight → d = 8 → word 7
   (behind); the attacker 20 px higher → word 4 whatever the bearing. With word 10: roll 10 → not landed, roll
   11 → landed.
9. **RNG decision draws of one strike** (AI-166, AI-007). A row-figure strike on a standing victim with a
   class block: the damage indication set (stun value 0 or the stun roll failed) → three draws (defence, stun,
   experience); both indications set → three; the stun indication only → three; **both clear → two (defence,
   stun), no experience roll**; the same with an immune victim or a victim at 0 hit points → the indications
   are set as attempts and the counts are the same as for a normal victim; a quick strike (no row) → two
   (defence, stun), never the experience roll; a row figure on a victim without a class block → **one**
   (experience only); a quick strike on such a victim → zero; any figure on a lying victim → zero; the
   reactions' own draws (sounds, remarks) are accounted separately.
10. **Recovery phase** (AI-170). An actor with id 5 standing still, no adversary, rate 50, fatigue 12: fatigue
    becomes 7 on the first frame with frame mod 64 = 5 and 2 on the next such frame, then 0; id 37 recovers on
    the same frames as id 5 (37 mod 32 = 5).
11. **Stun boundaries** (AI-173, AI-174). Hit points 80, stun counter 0, level 0: a player punch (80) → level
    100 → unconscious at once (stars 3) and, with `t` = 64, the counter is loaded with 64; the first decrement
    comes 65 frames later and one every 65 frames after it, so the level reaches 29 after 71 × 65 = **4615
    frames** → fit-again; with a residual counter of 10 at the moment of the punch, the first decrement comes
    after 11 frames and the level reaches 29 after 11 + 70 × 65 = 4561 frames; `t` = 0 → never; a tied actor
    decays to 30 and stops; level 69 → still conscious, 70 → unconscious; a strike with stun 50 on a victim at
    0 hit points adds nothing; a player punch on hard → trunc(80 × 1.5) = 120 → level 150.
12. **Bow extremes** (AI-177, AI-178). Skill 100 at distance 0 → chance = the grid's step-0 / skill-100 value;
    skill 0 at exactly range A → p = 100, band 4..5, the step-5 / skill-0 value; skill 30 at 30 % of range A →
    p = 30, band 1..2 with weight 0.5, rows 0 and 1 blended 0.7 / 0.3; a miss consumes three jitter draws.
    Penetration: a soldier target with word 100 → deflected on every roll; word 0 → deflected only on roll 0; a
    civilian admitted by the guard (a player's arrow on hard) → no roll, full damage; an immune NPC target →
    **no roll**, no effect (zero draws); an NPC's arrow never affects a civilian nor a same-side
    actor; a player's arrow affects a same-side actor on hard only.
13. **Aim validity** (AI-176). Range A 250, no flag, same height: a target 249 px due east → D² = 62001 <
    62500 → valid; 250 px due east → not valid (strict); a target 200 px due north → D² = (1.33 × 200)² =
    70756 → not valid, while 187 px due north (D² = 61857) is valid. The shooter 100 px above the target →
    reach = 250 + 0.30933626 × 100 = 280.93; 280 px due east valid, 281 not; in the long mode (a non-NPC
    shooter with the outfit flag) the same case gives reach 561.87; the shooter below the target → 250 (500
    long). A player with 0 arrows → invalid before any geometry; an NPC shooter never uses the long mode.
    **Rounding-boundary fixture** (AI-008 c): reach 204, same height, dx 0, dy 153.38345336914062: the scaled
    y 1.33 × dy = 203.99999956 in extended precision is **stored as the single 204.0**, so D² = 41616 =
    reach² and the strict comparison rejects the shot; an implementation that keeps the extended value would
    accept it — the stored-single boundary is mandatory.
14. **Oracle procedures** (tolerances): (a) the cone reach: capture the Alt cone of one soldier with a tint
    threshold that keeps pixels ≥ 5 % green and expect 310 × 229 px (± 10 %); (b) the run heard at 330 px: read
    the material class under the run's path and repeat with the console's noise display; expect the drawn white
    circle to match AI-072's radius for that class; (c) the swing cadence: log the halberdier's sub-state
    transitions for 60 s and count strike attempts, parades and skipped steps; (d) energy, **out of combat**:
    a hero (rate 50) standing still with no adversary after spending 10 units, the blue bar read at 20 Hz:
    expect one pixel (5 units) back per 64 frames (± 1 frame); a soldier (`q2` = 10) in the same condition:
    one pixel per 320 frames; a *stationary duel* recording is exploratory only — passive recovery requires no
    adversary, so the measured in-fight recovery (AI-170) remains unexplained evidence, not an expectation; (e)
    identification: approach a silhouette on open ground and expect the reveal at 600 px (± 20) with line of
    sight.
15. **Callbacks** (AI-190). An actor flagged for scripting receives `FilterAIEvent` for a view event with an
    element payload → (that element, 0); for a hear event whose payload is a position → (null, 2); for
    the shot-at-by-a-player event → (null, −2) and the callback is still invoked; for a dialogue beat → (null, −2);
    the receiving actor is the implicit context, not an argument; a return of 0 drops the event before the
    pre-filter, a non-zero return lets it through; an actor without the script flag never receives the
    callback. `ActionChange` on an actor whose action changes from 6 to 141 receives (141, 6).

## 8. Implementation choices

What is the original's behaviour is sections 2, 3 and 6. The following are OpenSherwood decisions or open
decisions, not facts about the original:

1. **The world tick.** The original's frame is a host-dependent realisation of a 40 ms wait on reported time
   (AI-001; the clock facts are cleared independently of this choice). **ADR-0010 decides**: one logic frame
   of 46.875 ms drives everything; the timers, staggers and the script second of this specification count
   that frame one to one; the *speeds* and animation holds (`ANIM`) are per frame and follow it.
2. **The random stream — OpenSherwood policy (proposed by the analyst, to be ratified by the maintainer).**
   The original uses one global stream shared with sounds and effects, drawn in actor-list order with the
   per-rule orders of section 3 (AI-007). *Equivalence target*: **distribution-level**, not draw-for-draw —
   an original trace cannot be reproduced draw for draw anyway because the shared stream's sound and effect
   draws are not part of this engine. *Stream ownership*: one named stream for this subsystem ("AI and
   combat"), drawn by every rule of section 3 wherever it executes — including **transitively through the
   natives** (native 140 re-issues a rail leg, whose pass-through pre-check may generate the cached program
   roll; the unlock native may post return-to-duty whose handling rolls; native 102's hit resolves with the
   strike rolls) — and by nothing else. *Draw ordering* (complete for the entry points that exist): (i) the
   level tick's phases are those of `spec-script-vm.md` (VM-103): the script scheduler runs its natives at its
   phase, and a native's draws occur inside that native's execution; (ii) within the actor phase, actors in
   actor-list order (AI-050) and, within an actor's tick, the order of AI-050's steps then AI-003's phases;
   (iii) a synchronous cross-actor call (a shout, a call between NPCs, an engagement's attack order) is
   handled **inside the caller's step** — its draws nest at that point and complete before the caller's next
   draw; (iv) a player character's combat draws (the auto-fight decision, the strike resolution on the victim)
   occur in the actor tick of the actor whose rule executes (the striking actor's hit frame for the sweep, the
   victim's tick-independent message handling nested in the striker's step); (v) projectiles draw in the
   projectile phase of the level tick, in projectile-list order, and a hit's target rolls nest inside the
   projectile's step; (vi) within one rule the order of its numbered steps. *Withheld until ratified*:
   **integrated seeded behaviour, replay reproduction and canonical-hash expectations for this subsystem are
   not cleared** — the acceptance tests of section 7 that count draws (9, 12) are stated per decision, not
   per stream position, and no test asserts a stream position.
3. **Seeding.** The original reseeds from the wall clock at every save (AI-006); OpenSherwood keeps its
   snapshot-carried seeds — a deliberate deviation that makes replays reproducible across saves.
4. **Arithmetic.** The original's float arithmetic is AI-008; OpenSherwood implements it with `f32` storage
   and `f64` intermediates in the stated order, which reproduces every integer of section 7 (the x87's 64-bit
   mantissa and `f64`'s 53 bits agree on all the products here; where they might not, the test states the
   expected integer).
5. **The line-of-sight cache collision** (AI-065) is an original defect; reproducing it is optional and, if
   not reproduced, must be recorded as a deviation.
7. **Native 140's out-of-range styles** (AI-093) write address-derived bits in the original; OpenSherwood
   treats styles other than 0 and 1 as a script error with no change — a deliberate, recorded deviation.
6. **The current engine** (`crates/opensherwood-core/src/ai.rs`, stealth-and-combat.md "Engine") models a
   sector cone with a rear radius, a single run-noise radius, a patrol → noticed → alarm → alerted → returning
   chain with a fixed timeout, one-at-a-time fights that soldiers never start, fixed soldier hits (5 hp at 2 in
   3 every ~318 ticks) and a fixed 50-hp forward stroke at 1 in 3, a 20-unit energy bar, a knock-out timer
   scaled by `p4`, and stubs for the bow, reviving, civilian perception and occlusion — all documented there as
   hypotheses or missing. Every one of those differs from sections 3.2, 3.5..3.12 above (the perception
   accumulator and line of sight, the seven-state machine without a global alarm, the class rows and the
   directional defence roll, the stun level, the 100-unit energy, the walking-flag sprint, the rail opcodes, the
   natives 126 / 128 / 140 / 177 / 197 / 198 / 219 / 220 / 228 / 59). This file states no replacement order and
   no schedule; those belong to the roadmap once this specification is cleared and its exclusions (9.2) are
   closed.

## 9. Open questions

### 9.1 Review history: disputes and their outcome

- Revision 2 disputed two findings of review 16: the stimulus order (purse, apple, beer, whistle) and
  native 126's values (menacing 4, fleeing 5, sleeping 0). **Review 22 upheld both**; the text keeps them.
- Revision 4 disputes **no finding of review 27**. Its 13 findings are answered in the text: (1) the rounding
  boundaries of AI-008, AI-066 reconciled, the aim equality fixture, general equivalence not claimed; (2) test
  1's id-1 variant (first evaluation frame 1, shadow 8, sighting 67); (3) 3× penalty, the circle bonus without
  subtraction, decay on entering the scoring pass, the earlier row on ties; (4) attempt indications versus
  state changes, the class-less victim's outcome and single draw, "finished" restated, the displacement
  reactions before the exit; (5) order state 7 in AI-082 f; (6) zero penetration draws on the immunity path;
  (7) the pre-tick hook under the local pause, the door-wait check after perception / AI / deafness, the
  civilian periodic routine outside the guards; (8) the snapshot inventory extended with explicit ownership and
  integrated clearance withheld; (9) the natives' transitive draws, the complete entry-point ordering, the
  explicit withholding; (10) natives 89 / 160 / 176 / 177 / 218 / 220 corrected, the unfinished ones excluded;
  (11) native 140's styles 0 / 1 only, the deviation; (12) AI-081 rewritten as behavioural events with the
  script-visible values only, the internal numbers removed from the prose; (13) the identity block.
- Revision 3 disputed **no finding of review 22**. Every one of its 16 findings is answered in the text: (1)
  the rounding stages and the 15 / 159 / frame-66 example; (2) the extended-precision reaction arithmetic and
  the 54 / 108 / 60 examples; (3) the circles' hesitation exception and the reactive second roll; (4) the
  conditional experience roll and the per-result draw counts; (5) the −2 mapping and the callback tests; (6)
  order states versus action ids in the fit-again guard and native 89, natives 134 / 135 rejecting players,
  the block stance preserved by the lock; (7) the aim geometry (1.33, strict, long-mode doubling of the
  extended reach, 0.3 rad); (8) the three arrow target cases; (9) the out-of-combat recovery test; (10) the
  stun counter's initial phase; (11) the two pause mechanisms and the timer phases; (12) the snapshot
  inventory, the arithmetic contract and the RNG policy; (13) the native contracts; (14) the split registry,
  AI-185 / AI-173 / AI-101 reconciled with 9.2, the win-path assertion removed; (15) option (c) removed from
  9.3 and the archer condition restated as a capability; (16) the identity block.

The analyst's readings that review 22 confirmed as facts are not restated here; where this revision changed
a number, the change is visible in the claims and the tests.

### 9.2 Behaviours not read (exclusions until specified)

Each item is a **hand-off boundary**: the implementer records an `Assumption` (ADR-0008) where the behaviour
is first needed and does not infer it from this file. Items 15..17 were added by revision 3.

1. The menacing state's timers and exits (the case for its sub-state in 00424d50).
2. The officer's tower-guard alert routine (0043b1b0, 0043c1a0) and the colleague-found handler (00444bb0).
3. The beggar's betrayal roll (0042c9a0) and the civilian attitude branches (0040d440, 0040d690).
4. The body actions' effects: money taken by a search, the tied flag and the carried state (the cases for
   actions 122 / 282 / 169 in 00475bd0; the player status's money field); the reviver's stun subtraction.
5. The pointer-mode → order mapping of the context actions (004e1a40, 004df070..004e1630), the right-click
   cancel (callers of 0046bcb0 in 004cd400 / 004de840 / 004defa0), Ctrl queueing, and the **gesture
   recogniser** (a stroke → one of the nine figure orders; not in the actor code; the input layer near
   004dd410 / 004de290).
6. The shot at a training target (the target element's own activation, 004bb820) and why the 186 px walkway
   shot was refused (range, height, hit area).
7. The seek-point list's mission chunk (the global list read by 0043a220); 0043d0c0, 00438600, 00416800,
   00419970; what an area search registers as "known" (AI-070).
8. The mission "outfit" flag's origin (read at 004c38d0): which level field sets it; it selects the hero's
   record and class, the conspicuousness word, the long bow mode and the friendly-side rules.
9. The posture codes 2..14 (setter 0046bd10; callers 00475bd0, 0047ac90), sprite actions 242 / 246 / 247 versus
   the documented hidden-in-leaves ids 136 / 137, the second ×1.4 range condition of AI-061, and the order
   states 10, 16, 19, 21..23 used as gates.
10. The swing cadence (AI-153) and the energy recovery discrepancies (AI-170); the cone's drawn fade
    (0058e7f0); the run heard at 330 px; the identification at 120 px.
11. The ranged record used by each player (0048fa30 → 0047ab00's third argument) and the arrow "sharp" flag;
    the distance normalisation's exact float path in 004500c0 (the truncated percentage is observed; the
    rounding of p at band boundaries is not).
12. Native 102's second parameter (005861c0 and its executor); the writer of the "no player death" option
    byte (AI-046) and of the flagged recovery (AI-172).
13. SD `w0` beyond the experience formula, `unknown_post19`, flag bit 5 (0049b510), row word 14, the "wake
    now" flag (AI-125), the console's "alert all NPCs".
14. The hostile reset body (AI-101): what return-to-duty clears beyond the state change; the stun counter's
    phase when the level is set by a script rather than a blow.
15. Whether native 89's order state 18 is "tied" or "carried" (both are described as that state by different
    consumers).
18. Float paths whose rounding boundaries were not fixed by a fixture (AI-008): the graded-value formula's
    own stages (00489d00), the light-limited reach (0048db80), the morale formula's integer divisions
    (00432d30), the bow's interpolation (004500c0): cleared as formulas, withheld as bit-exact behaviour.
19. Snapshot restore fixtures for the inventory of 2.7..2.9 (a pending event queue restored in order, a
    suspended rail wait, a sync waiter, a collapsing cone, a cached roll that is still available versus
    consumed): to be written by the implementer before integrated snapshot / replay clearance.
16. The order pipeline beyond AI-185: the right-click cancel, Ctrl queueing, the left click's dispatch to the
    context actions (previously listed under item 5; restated here as the boundary of AI-185).
17. What an area search registers as "known" after clearing the watched element (AI-070), and the second
    ×1.4 range condition (AI-061).

### 9.3 Material excluded from this file (expression filter)

Under ADR-0009 section 5 as clarified on 2026-09-13, a mapping of data-file fields or ids to their meaning is
an allowed interface fact, while a table of tuned thresholds the program carries as content is not, unless the
generating rule is described or the table is loaded from a data file. The per-figure admissibility thresholds
of AI-151 (nine experience minima and nine hesitation maxima) are **content the program carries**: they are
not loaded from any data file, no rule generates them, and they are not a mapping of ids to meanings.
**Decision: they are excluded**, and they are not admitted as individual constants either (that would
reconstruct the table). The implementer records an `Assumption` for the figure choice's admissibility and
implements the qualitative ordering of AI-151. The only permitted observational alternative is to record
*outcomes* of the running original as oracle facts — e.g. "a blue halberdier in the first mission used only the
jab and the laterals in a 10-minute fight" — never the thresholds themselves. The values stay in `re/notes/ai/`.

## 10. Provenance

Ghidra project `re/ghidra/robinhood` (never committed); full decompilation exported to `re/out/decomp_all/` by
the generic scripts in `scripts/ghidra/`; strings and inventory exports; byte reads with
`scripts/ghidra/peek.py`; capstone disassembly of the eight functions named in the identity block (kept in
`re/`). Analyst session `a2931a3a5b130742c`, 2026-09-13, with six forked sub-sessions whose notes are
`re/notes/ai/{profile-fields, perception, state-machine, attacking, rails-orders-natives, combat,
clock-and-rng}.md` and the id maps `state-ids.txt`, `substate_ids.txt`, `natives-map.tsv`. Data checks on the
player's `profile.cpf` with `harness/tools/probe/cpf_stats.py` and `cpf_probe.py` (all 68 SD, 10 PC, 24 CV
records, 27 class blocks, 4 ranged records). Oracle recordings compared (no new recording): `combat-
measurements.md`, `h01-measurements-2.md`, `stealth-and-combat.md` 8 (2026-09-05). Reviews 16 (of revision 1),
22 (of revision 2 at `e966b05`) and 27 (of revision 3 at `e7b2cd4`, blob `c9040a2a406e8525fbd6aae32241b81fe0c72a6c`)
by Codex `gpt-6-astra` (reviewer session `01a0b406-a088-76b0-b80e-f5d4a5abd73d`), committed under
`docs/decisions/reviews/`, drove revisions 2, 3 and 4. Revisions 3 and 4 were written on 2026-09-18 after an
interruption on 2026-09-13; the functions re-read for them are named in 9.1's answers and in section 8 of the
review documents. Tests that will depend on this spec: none yet (section 7 is the list to implement).

Functions read, by area (why): **clock / RNG** 004c6ef0 0050f710 00404180 00642a7d 00642a70 004148b0 0040a230
(the frame, the wait, the seeding). **Profile loading and fields** 00564580 00564e60 00564bf0 00564ea0 00565490
0056a7d0 00568e00 00569f40 00565100 00565c10 0056aee0 00569570 005672d0 0056c700 0056c290 0056c360 0056c5b0
0056c100 0056cab0 0056c410 0056c490 00436500 005644c0 0056d5d0 0056d2a0 0048f280 0048fa30 0047ab00 005c1b20
00450040 004502e0 00438710 00438600 00438cb0 0041a420 004468a0 0044cb40 004a1cc0 004a1cf0 004a1d00 004a5ba0
004a5d30 0048f910 0048f9b0 0048f9c0 0048f9d0 0049d5e0 0049d560 0049d520 0055bc90 0055bf00 0055dbb0 0055dea0
0050b640 0049b510 (layouts, consumers, difficulty). **Perception** 004863f0 00486fa0 00486f30 00486f60 00486f70
00487aa0 00487ad0 00487b70 00487b90 00487d00 00488f60 00488870 00488e00 00488b80 004887a0 00488a40 00489680
00489eb0 00489d00 00489b30 0048db80 0048a130 0048a260 0048a350 0048a830 0048a980 0048c700 0048c8e0 0048c4d0
0048c5b0 0048c620 0048f110 0048e420 0048cca0 0048cc20 0048cec0 0046f290 0046ec50 0047ef70 004a0a30 00570e70
004c1ab0 0048ebe0 0057ae60 004eff80 004eef30 004f0230 004ef340 005a3810 0058e7f0 005b3c40 005b3c20 00471b00
00471c00 004936f0 0045db80 0045dce0 0045e6f0 0045e7e0 004d4300 004d5320 0041a300 (cone, values, events,
hearing, identification). **Event system and the state machine** 00424bc0 00424d50 0042c9c0 0042e450 00422c40
00422b70 00410620 0040d980 00416710 0048b450 0048b4e0 00419fb0 00419fd0 00419970 0057aed0 00434aa0 0041c130
0041f540 004207f0 00420d40 00418c40 00414550 00414be0 00424540 00435c80 00430300 00438620 0044d750 00433060
00439cb0 00433830 00434320 00434710 00433790 00433f90 00439e70 00439f60 0043a080 004457c0 0043d890 00445d40
004460f0 0044b000 00446400 004469e0 004468a0 00446d20 00449df0 0044cb40 0040e7e0 00443d10 00443af0 00443c40
004436e0 00443850 0043a220 0043d0c0 0043d530 00522330 00416800 00438600 0043c960 0043ccc0 0043cec0 0044cf00
0044a760 0044a830 004444a0 00444bb0 0044dc80 00449170 00413240 0040bfd0 0040c140 0040cc60 0040d150 0040d440
0040d690 0040e150 0040e290 0040f090 00416b30 (states, transitions, timers, calls, stimuli, civilians).
**Attacking** 004303e0 00432d30 00438cb0 0043d780 0043db20 0042ef80 0042fe00 00432f80 00430020 00441650
00442190 00442470 00447070 00447e50 0044dd70 0044ca90 0044c850 0044a090 00445210 0044b4e0 00445510 00445180
004474e0 00447490 00447640 0043ea70 00440cf0 00441020 00485240 00485590 00482240 00482fd0 00482e50 004820e0
0044b940 0044bc70 00449310 0044aa70 0044d400 0044d6c0 004a55b0 004a5610 00487aa0 004a5460 (decisions, morale,
targets, the fight step, figure choice, tactics). **Rails, orders, natives** 00410010 00410de0 004121a0
00419cc0 00419c50 00419c80 005505b0 005505c0 00550640 00550660 00550f60 00412c80 00412cf0 00412240 00412f70
00418600 00410600 0041a960 0041ac50 0041b0e0 00412d60 00439080 0041bd20 00444440 0044cae0 00486f50 004a1e90
0048b5d0 004c1df0 0054e750 0054e9a0 005866a0 00584d60 0058a3d0 0058a940 0058bbe0 00582530 0046bcb0 004de290
004de1e0 004dce20 004dcfd0 00403190 004032a0 00408750 00464230 and the native bodies 00575e20 00575f00 00575fc0
00576d80 00576e00 005780d0 005790b0 005790f0 0057acc0 0057ad30 0057aa70 00577500 00579a70 00579b60 00570a50
00570d40 00570a70 00579bd0 00579470 005794b0 00579520 00571b30 00418b80 00418bc0 (opcodes, control flow,
company, sequences, natives, callbacks). **Combat** 0047c600 0047c4c0 0047d270 0047ca50 0047dc00 0047e1e0
0047f420 0047e7d0 005c1b70 005c1dd0 005c1db0 005c1d70 005c1b50 005c1d50 005c1e80 005c1ea0 005c1ed0 005c1f10
005c1f50 005c1f90 005c1fb0 00471ed0 00471e50 00471c00 00471b00 00482150 004d5a90 005c3fc0 005c2ec0 00472070
00475bd0 0047f520 004801d0 004808f0 00480e90 00481050 00481ef0 00482600 004828b0 00482b90 00482390 00483070
00474d60 004748f0 00474ad0 00483c90 00483d90 00481d40 00463da0 0048d990 0048e610 004a5a00 0047e9a0 0049fa20
00485e90 004e39b0 0051d5d0 0051d4e0 0051e8e0 0047b760 0047b8a0 004df070 0047bb60 004500c0 004500a0 00450070
00450300 004504b0 004b82e0 004b7b30 004b7040 004b8230 004b44b0 004a18c0 004a68e0 004859b0 004a6b80 004a66f0
004b9220 004b97c0 004b9b70 004be0e0 004be1a0 004bc660 004a74f0 0049f520 0049ec50 0045e2e0 (engagement,
resolution, energy, death, stun, bow, projectiles, purses, wasps, console cheats).
