# Movement, animation and camera (behaviour specification)

Status: `draft`, **revision 2** (answers Codex spec review 18 finding by finding; awaiting re-review). Build: GOG
English edition, `Robin Hood.exe` SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base `0x00400000`; every address below
is a virtual address in that image.

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text, and no prescribed internal
structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer chooses
the organisation.

Claim ids are `ANIM-nnn`. Status is `observed` (read in the program's code at the named address), `inferred` (the
reading that fits every branch that was read, or a conclusion from several observed facts), `unknown`. Confidence
is high unless stated. Sibling specifications, whose claim ids are referenced instead of repeated:
`spec-script-vm.md` (`VM-nnn`), `spec-navigation.md` (`NAV-nnn`), `spec-ai-combat.md` (`AI-nnn`). Section 12
lists the amendments those documents need because of what is established here.

**Compatibility tokens.** One identifier is required verbatim because the player's compiled mission scripts name
it: the callback name **`ActionChange`** (section 6). Nothing else in this file is a name taken from the program.

## Identity and exposure

- **Analyst**: agent session `a2f97805ff3217993` (an Opus session), 2026-09-13, analyst role under ADR-0009. It
  has read decompiled code of the animation player, the actor action executors, the movement and collision entry
  points, the cart, the camera, the per-frame drawing and the settings. It must not implement any of them, and no
  implementer session may inherit its context, notes or tool output.
- **Delegated readers**: two subordinate sessions launched by `a2f97805ff3217993`, in the same role and the same
  workspace, produced first-pass findings for the camera and for the drawing order and settings. Their exposure
  counts as this session's. Their raw output never entered the repository; every statement they contributed was
  re-read at the addresses cited before it was kept, and several were corrected or withdrawn in this revision.
- **Reviewer**: Codex `gpt-6-astra`, **spec review 18**, which reviewed revision 1 (repository blob
  `17ff4e2c6e84326e82552e0afc0c2ef8fc3082b3`) and returned 28 findings with the verdict *redo*. This revision 2
  answers all 28; section 4.3 records which findings are answered, which are disputed with the reason, and which
  claims are **excluded from clearance** and must not be implemented until settled.
- **Implementation reviewer**: pending; must be a session that has never read `re/`.
- **Publication approval**: pending, separate from factual approval.

## 0. Necessity record

**Interoperability target.** Playing the player's own missions with the player's own sprite files: the mission and
map files place characters, carts and objects and address their animations by *action id*; the compiled mission
scripts (`.scb`) drive them through sequence elements (walks, animations, speech, camera moves, zoom) and wait for
those elements to complete; the character profiles (`DATA/Characters/*.rhs`) carry the per-frame timings and the
per-frame advances that decide both how long a frame is shown and how far a character moves. Nothing plays
correctly unless the engine advances animations on the same clock, applies the same per-frame displacement,
completes elements at the same moment and composes the frame in the same order.

**Information data observation and black-box testing could not settle.** `docs/formats/sprite-animations.md` had
to *infer* the frame clock from two oracle measurements and offered two competing readings of the zero timing half
on moving frames; `docs/original/stealth-and-combat.md` 8 measured "about 64 Hz, three clocks per animation frame"
without being able to say what the three clocks are, and `docs/original/combat-measurements.md` later had to
reassign one of its two measurements to a different object; `spec-script-vm.md` open question 6.1 lists the
completion rule of every actor-side sequence element as unknown, which leaves every scripted sequence unable to
advance; `spec-navigation.md` NAV-150/151 had the movement advance and the turning factors but not the clock they
are applied on; the camera natives were guessed by the current engine. All of that lives only in the executable.

**Scope read** (about 110 functions; the reasons are the questions above):

| Functions | Why |
|---|---|
| 0050f710 (main loop), 004c6ef0 (level tick), 004d9420, 004de170, 004d23d0 | the frame, the wait, the execution opportunities, the phase order inside a tick |
| 005b86b0 (play and move), 005b8050 (play), 005b7820 (frame timer), 005b7300, 005b7720, 005b7f60, 005bd450, 005bd560, 005bd5b0, 005bdbf0, 005bdc20, 005bdcd0, 005bdcf0, 005bddb0, 005b5e00, 005b5ed0, 005b6790 | the animation clock, the play modes, the entry points, the displacement gate, the early-completion marker, the saved state |
| 0055f0f0, 0055f140, 0055f1a0, 0055f210, 0055fc20, 0055fa70, 0055fe10, 0055f290 | facings, the four turn variants, the projection, the saved position state |
| 00464230, 00471b00, 00464b20, 00467a50, 004646e0, 0046abd0, 0046b210, 0046bcf0, 0046bd40, 0046dae0, 0048d510, 00470390, 00475bd0, 00462730, 004645f0 | the actor update, the action executors, the completion codes |
| 00585320, 00585970, 00587160, 005871b0, 005866a0, 00586ed0, 0058a940, 0058ba60 | element states, element parameters, the elements the executors create |
| 00561040, 00563e90, 00563ea0, 00563ed0, 00560780, 00560460 | the collision-aware move, the failure counter and its reset, the published velocity, the arrival test |
| 004ab720, 004ab7e0, 004abfe0, 004ac350, 004af540, 004ad5f0, 004ae470 | the cart: speed integration, sub-sprites, the instruction stream, bonds |
| 004c8380, 004cdfc0, 004ca410, 004cec60, 004c7f60, 004c7cf0, 004c7cd0, 004cf610, 004cfce0, 004be6d0, 004dba50, 004db920, 004d90b0, 005758d0, 005a8650, and the natives 00571200, 00571270, 00571330, 005713f0, 00572ab0, 00572ba0, 00572c70, 00572d50, 00577f30, 00577fe0 | the camera: state, scroll, zoom, lock, the natives' actual writes |
| 005105d0, 004d0a10, 004d1d00, 00462a30, 004aa980, 005c73e0, 005c2ec0, 0051c0d0, 005e41d0, 005e5260, 0052cf80, 0052d090, 004c0040 | the per-frame drawing order, the depth key, the sort line, ground marks, the surfaces and the viewport |
| 00546050, 005460c0, 0055dbb0, 00438600, 00438710, 0055d1a0, 0051ba00, 005b2190, 00409d70, 005a6520 | the settings, and the difficulty value and how it is consumed |

**Stopping condition, stated honestly.** Reading stopped when the interoperability target could be met for the
parts listed as `observed` **and the remaining gaps had been named**. It is *not* complete: the state machines of
the collision resolution (sliding, pushing, stepping aside), the arrival geometry beyond its plain case, the
per-kind admission tests, the cart's container and two of its instructions, the speech duration, the scenery
ordering and the mask integration, and the exact camera follow and zoom traces are **not settled**. Section 4.3
lists what is excluded from clearance; sections 7 and 10 say what an implementer must do until each is settled. A
reviewer should treat any claim not in section 4.3's cleared list as provisional.

**Analyst authorisation.** On behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

Covered: the clocks and what advances on each execution opportunity; the animation player (action ids, the 16
facings, the frame timer, the play modes, the entry points, the completion signals); what a script's animation,
walk, turn, speech and action elements do on an actor and when they complete; per-frame displacement, turning,
the collision and arrival interfaces; the cart; the camera (state, scroll, zoom, lock, the natives); the
composition of one drawn frame; the settings and the difficulty value's role.

Taken from other subsystems: the level tick's script phases and the sequence machinery (`spec-script-vm.md`), the
path finder, the walk order pipeline, sectors, layers, doors and lifts (`spec-navigation.md`), the AI's choice of
gait and action, the random stream and the difficulty-dependent rules (`spec-ai-combat.md`), the sprite container
and animation table layout (`docs/formats/sprites.md`, `sprite-animations.md`).

Handed to them: the completion of every actor-side sequence element (the VM's open question 6.1), the movement
speed per gait, the clock every timer counts on, and the amendments of section 12.

## 2. Data model

### 2.1 Clocks, pacing and units

- **ANIM-001** (observed, 0050f710; high). The main loop performs one iteration per displayed frame. Near the top
  of the iteration it samples the operating system's millisecond counter; at the bottom it busy-waits, re-reading
  that counter, until the **reported** elapsed time since that sample is at least **40 ms**, or at least **400 ms**
  when the slow-motion setting is on (ANIM-520). It never catches up: a frame whose work takes longer than the
  wait simply lasts longer, and nothing in the game advances by more than one step in it.
- **ANIM-002** (observed, 0050f710; high for the mechanism, **medium for any particular duration**). The reference
  sample is taken again on the path that runs while the game window is active, so the interval measured is not
  exactly "the whole iteration". Because the comparison is against a *reported* value, the realised frame length is
  quantised by the host counter's granularity and also depends on the counter's phase, on how long the frame's work
  took and on scheduling. **On the host the oracle recordings were made, the realised cadence is 46.875 ms
  (21.333 frames per second)**, which is three intervals of the 15.625 ms granularity Windows uses by default and
  which three independent measurements reproduce (section 9). That value is a **well-supported reference cadence
  for that host**, not a universal consequence of the code: a host whose counter granularity divides 40 ms would
  produce 40 ms. Nothing is claimed here about what cadence the game's authors targeted.
- **ANIM-003** (inferred, 0050f710 + 005b7820 + the data of section 9; high). **The animation clock is the frame.**
  One animation-frame timer step per element per executed update, and an animation frame is displayed for
  `hold + 1` such steps (ANIM-031). The "table tick" of `docs/formats/sprite-animations.md` rule 2 is one frame.
  The program keeps **no** 64 Hz clock and no sub-frame accumulator; the "three clocks of about 64 Hz" of
  `stealth-and-combat.md` 8 is that document measuring the host counter granularity of ANIM-002 through the frame
  pacing.
- **ANIM-004** (observed, 0050f710, 004c6ef0, 005105d0, 004c8380; high). The things this specification describes do
  **not** all advance together. Per iteration, independently gated:
  - *the level tick is attempted* only when the freeze flag is clear, when the level's transition object either
    does not exist or does not report a blocking transition, and when the game state is not one of the two that
    leave the level. `spec-script-vm.md` VM-101 lists the further conditions inside the tick.
  - *actor updates, the cart interaction pass, the sequence queue drain and the timer lists* advance only inside an
    executed tick, and the tick's own early exits (a set win or loss flag, a level-ending transition) can advance
    the tick counter while skipping the element phases.
  - *the camera* advances inside the drawing (ANIM-100 step 5), which runs whether or not the tick ran. A skipped
    tick therefore still scrolls the camera, still finishes a running script scroll and still completes camera
    elements.
  - *the drawing* runs unless the freeze flag or the level's blocking flag is set (ANIM-521).
  - modal pages and dialogues are the case `spec-script-vm.md` VM-101/222 describes: they suspend the tick from
    outside, in their own loop; they do not suspend the drawing or the camera.
  An implementation must keep these four gates separate; "everything stops when the tick stops" is wrong.
- Units: positions, displacements and camera coordinates are **background pixels** ("map pixels") in the screen
  projection of `spec-navigation.md` NAV-001/002; a character's screen row is its world row minus its height.
  Run-time positions are IEEE single floats; the animation table's per-frame advance is a signed 16-bit integer
  number of pixels; facings are integers 0..15. "Frame" below means one executed main-loop iteration and "update"
  one execution of the thing being described.

### 2.2 The animation table as the player uses it

The file layout is `docs/formats/sprites.md` and `sprite-animations.md`; this is what the player reads from it.

- **ANIM-010** (observed, 005b5e00, 005b8050, 005b86b0; high). An element plays an **action id**, never an
  animation index. The sequence provides a lookup from action id to the index of the first animation of a **block
  of 16**; an absent action is marked in that lookup, and a play call for an absent action is reported and fails
  without changing any state. The animation played is `block index + facing`, with the facing 0..15 as
  `sprite-animations.md` "Direction order" (0 = screen-up, clockwise). Two such lookups exist per element (the
  normal one and a replacement one, ANIM-014); a flag chooses which is current.
- **ANIM-011** (observed, 005bdbf0, 005bdc20, 005bdcf0; high). Per animation the player needs: a per-frame **hold**
  value (unsigned 16-bit, the low half of the frame's timing word), a per-frame **advance** value (signed 16-bit,
  the high half of the same word), the **frame count** (the player derives it from the length of the per-frame hold
  data and answers 0 when that data is absent), a 16-bit **marker frame index** (ANIM-012) and a 16-bit **loop
  length** (ANIM-013).
- **ANIM-012** (observed that a marker field is read and how it is used, 005bdcd0, 005b7300, 005b7720, 005b7820;
  **unknown** which file field it is; medium). One 16-bit field per animation is read as a frame index and is used
  three times: play mode 6 starts a clip at it, play mode 7 rotates the facing while sitting on it, and the
  early-completion marker of ANIM-034 is derived from it. In the shipped profiles the value equals `frame count
  - 1` on every block checked here and in 112 608 of 148 512 animations counted by `sprite-animations.md`, which is
  consistent with `Animation::unknown_0x02`, but the identification was not carried through the sprite loader.
  **Excluded from clearance**: an implementer must confirm the field before relying on modes 6 and 7, and must
  treat the early-completion marker's dependence on it (ANIM-034) as provisional.
- **ANIM-013** (observed, 005bddb0; high that a second 16-bit field is read by action id as a repetition count;
  **unknown** which file field). This is the "length taken from the animation table" of `spec-navigation.md`
  NAV-200 for the ladder and ivy climb loops. Same clearance caveat as ANIM-012.
- **ANIM-014** (observed, 005b86b0, 005b8050, 005b5e00; high). Two script-driven redirections act on the action id
  before the block lookup: (a) natives 60/61 (element kinds 0xAA/0xAB) switch the element to a replacement lookup
  in which one action id has been replaced by another, and back; (b) an element may carry a list of *animation
  overrides*, searched by action id, first match wins, whose matching entry supplies the id actually played, and a
  flag records whether a match was found. What else the override entries carry, and what the flag is consumed for,
  is not settled.

### 2.3 State that must survive between updates and be saved

Stated as the state's **role**, not as a layout. A save or snapshot must be able to reproduce every behaviour of
section 3 after a reload; the on-disk order belongs to the save specification (the routines that write it are
005b5ed0 for the animation part and 0055fe10 for the position part, which is where this list comes from).

- **ANIM-020** (observed, 005b5ed0; high). Per element, for animation: which animation is current (equivalently
  the action id together with the facing), the current frame index, the frame timer, the action id the frame timer
  was last reset for, the identity of the action element currently being played, the early-completion marker pair
  (ANIM-034), which action-id lookup is current (ANIM-014), the depth sort key (ANIM-364), and three further flags
  whose meanings were not settled. A reload that drops the frame timer or the marker pair changes when animations
  and therefore script elements complete.
- **ANIM-021** (observed, 0055fe10, 0055fa70, 0055fc20; high). Per element, for position and movement: the screen
  position, the world position and height, the position at the start of the current update (ANIM-201), the facing,
  the target facing, the turn delay counter and the turn hysteresis counter (ANIM-211), the layer and sector
  identity, the projection area, the ground kind, the movement direction with its height component, the reverse
  flag (which mirrors the facing by 8, `spec-navigation.md` NAV-002), the no-collision flag, the off-map flag, the
  visibility flags, the failed-move counter and the shrinking collision tolerance of ANIM-241.
- **ANIM-022** (observed, 00464230, 0046bcf0, 0046bd40; high). Per actor: the queue of action elements with the
  current one and each element's state, the action id last reported to the script (283 = none,
  `spec-script-vm.md` VM-107), the status of the last executor run (ANIM-120), the wait counter (ANIM-133) and the
  finish code of the last action that ended (ANIM-121).
- **ANIM-023** (observed, 004ab720, 004ac350, 005b86b0, 0051c0d0; high). Further state the same requirement
  reaches: per cart, the program position, the remaining length of the current block, the current speed, the target
  speed, the acceleration and the waypoint index, plus the per-axis rattle accumulators of ANIM-301; per element,
  the footstep effect phase counter of ANIM-207; per level, the camera state of ANIM-320 and the ground marks with
  their animation frames.

## 3. Behaviour

### 3.1 One frame and its execution opportunities

- **ANIM-100** (observed, 0050f710, 005105d0; high). Required order within one iteration while a mission is played:
  (1) sample the pacing reference; (2) pump the operating system's messages, read the input devices and dispatch the
  commands the input produced (this is where the camera commands of ANIM-323 are raised and consumed); (3) attempt
  the **level tick** under the gate of ANIM-004; (4) update the HUD widgets' state; (5) **draw the frame**
  (3.13), whose first step advances the **camera** (3.12); (6) update the sound; (7) busy-wait to the pacing
  minimum; (8) hide the cursor and pump messages again. The frame that is presented shows the state the tick of the
  same iteration produced.
- **ANIM-101** (observed, 004c6ef0; high). Required order of the phases inside one executed level tick that concern
  this specification, **after** the script phases of `spec-script-vm.md` VM-103 steps 1 to 5:
  1. the path-finder hand-off and result consumption (`spec-navigation.md` NAV-142);
  2. **the cart/character interaction pass** (ANIM-305) - it runs **before** any element is updated this tick, so
     it sees the positions the previous tick left;
  3. the **per-element update pass** (ANIM-102), over the level's element table from index 0 upwards;
  4. the sequence manager's queue drain (VM-215);
  5. a conditional global refresh;
  6. a level-owned list of timed items, visited from its last entry to its first, each asked to advance and removed
     when it answers that it is finished (what that list holds was not settled: it is **not** the script timer
     list);
  7. the **script timer list** (native 56, element kind 0xB), visited in **insertion order**, exactly as
     `spec-script-vm.md` VM-221 specifies.
- **ANIM-102** (observed, 004c6ef0, 00464230, 00471b00; high). The per-element update pass: the element count is
  re-read before every step, so elements appended during the pass are updated in the same pass. An element whose
  update answers that it is finished is removed from the level immediately, **and the pass index still advances**,
  so the element that moves into the vacated position is not updated this tick. One actor's update then proceeds:
  (a) the position at the start of the update is recorded and any pending placement is applied; (b) the first queued
  action element becomes current, or there is none; (c) an action element that was not current at the previous
  update is marked as freshly started; (d) the **action executor** runs for the current action element - it turns
  the body (3.7), steps and plays the animation (3.2) and moves the element (3.6) - and answers a **status**
  (ANIM-120); (e) the status is acted on (ANIM-120, ANIM-121); (f) if the current action id differs from the one
  last reported to the script, `ActionChange(current, previous)` runs with this actor as the current actor and the
  reported id is updated; with no current action the reported id is 283.
- **ANIM-103** (observed, 00471b00; high). Some per-actor bookkeeping is **staggered**: it runs only on updates
  where the low six bits of the level's frame counter equal the low **five** bits of the element's own identity
  (so each element has a phase and the period is 64 updates), and only when the element has no adversary bound and
  its position did not change during this update. The quantity it recovers, and by how much, belongs to
  `spec-ai-combat.md` (whose AI-004 states the wrong bit width; section 12). An implementation must key the phase
  on element identity, not on table position.

### 3.2 The animation frame timer

One state machine. State: the animation index, the frame index `f`, the frame timer `t`. `hold(f)` and
`advance(f)` are ANIM-011, `n` the frame count, `m` the marker frame (ANIM-012).

- **ANIM-030** (observed, 005b7300, 005bd5b0; high). The timer is reset when the **action id** changes - not when
  the facing changes, and not when a different action element requests the same action id (ANIM-035). A reset sets
  `t := -1` (as a 16-bit value) and, by play mode: `f := 0` for modes 0-5, 7-9 and 11; `f := m` for mode 6;
  `f := n - 1` for modes 12 and 13; and for mode 10 alone `t := 0, f := 0`. An explicit restart, which some entry
  points perform instead, sets `f := 0, t := -1` or `f := n - 1, t := -1`.
- **ANIM-031** (observed, 005b7820; high). One step of an ordinary forward mode: `t := t + 1`; if `hold(f) < t`
  (unsigned) then advance `f` (by one, or by two in modes 3 and 14) and `t := 0`; then wrap or clamp `f` as the mode
  says. Consequences: a frame is displayed for `hold + 1` steps; the value `-1` used by a reset makes the first step
  land on `t = 0`, which is the same state a frame is in when it becomes current in the ordinary flow, so the first
  frame of a clip gets its full `hold + 1` steps. Mode 10 resets to `t = 0` instead, and has no step of its own
  (ANIM-036), so it neither displays a first frame for that long nor advances at all.
- **ANIM-032** (observed, 005b86b0; high). **The displacement gate is `t = 0` after the step**, not a comparison of
  the frame index before and after. Displacement is taken when, and only when, the timer is zero after stepping; on
  every other update it is zero. In an ordinary cycle that coincides with "the frame changed", which is the useful
  way to think about it, but the two differ in three cases that must be reproduced: on the first step after a
  reset (the timer goes from -1 to 0 and displacement **is** taken although the frame did not change); on a
  one-frame clip whose hold is 0 (every step leaves `t = 0` and displacement is taken every step); and in the modes
  that freeze while leaving the timer at 0.
- **ANIM-033** (observed, 005b7820; high). The **completion signal** of a looping mode fires on the step after
  which the clip's last frame is current *and* the timer equals that frame's hold value; if that frame's hold value
  is 0, it fires on the step that makes the frame current. It therefore fires on the clip's last displayed step,
  one step before the wrap, once per cycle; the clip keeps looping afterwards.
- **ANIM-034** (observed, 005b7720, 005b8050, 005b86b0; high for the rule, medium for its purpose; depends on
  ANIM-012). When a **new action element** starts, an **early-completion marker** - a (frame, timer) pair - is
  computed from the action's animation and stored. Whenever the live (frame, timer) equals the stored pair, the play
  call answers status **0** instead of "running". The pair is:
  - `(m, 0)` in general;
  - a pair no state can ever reach, when the clip has exactly one frame whose hold value is below 2;
  - when `m = 0`: `(1, 0)` if the first frame's hold value is 0, otherwise `(0, 1)`;
  - when `m` is the last frame index or beyond: `(n - 2, hold(n - 2))` if `m > 1`, otherwise a pair no state can
    reach.
  Because `m` is the last frame index in the shipped data, the ordinary result is "the last displayed step of the
  second-to-last frame", i.e. **one displayed frame before the end of the clip**. That the purpose is blending is an
  inference, not observed. The pair is computed from the action's block without the facing; all 16 animations of a
  block share their frame count and hold values in the shipped data, so this makes no difference there.
- **ANIM-035** (observed, 005b8050, 005b86b0; high). **Entry points differ, and this changes observable timing.**
  Two entry points exist: *play only* (used by actions that do not move) and *play and move* (3.6).
  - On the update that admits a **new action element**: the play-only entry recomputes the early marker, resets or
    restarts the timer as ANIM-030 says, answers status **1**, and does **not** step the timer. The play-and-move
    entry does the same bookkeeping but **does** step the timer and move.
  - On later updates with the same action element both entries step.
  - A new action element carrying the **same action id** as the previous one recomputes the marker but does **not**
    reset the frame or the timer: the clip continues where it was.
  - When the requested action is absent from the profile, both entries report it and answer status **4** without
    changing any state.
  A consequence for section 7: the raw timer trace of ANIM-031 is the trace of a moving action; a non-moving action
  started through the play-only entry is one step behind it.
- **ANIM-036** (observed, 005b7820, 005bd450; high). Modes 10 and 11 and every code the player does not recognise
  perform no timer step, so the element holds its frame. When a clip completes, the player also releases the cached
  pixel data of the animation it was playing; this is cache housekeeping with no observable effect on behaviour and
  an implementation need not reproduce it.

### 3.3 Play modes

The play mode is chosen by the caller per action and is remembered, so that changing only the mode does not reset
the timer (ANIM-030 resets on the action id).

| Mode | One step | Completion signal |
|---|---|---|
| 0 | forward by one, wrap to 0 when `f` reaches `n`: an endless loop | once per cycle (ANIM-033) |
| 1 | forward by one, wrap to 0 when `f` reaches `n` | never |
| 2 | forward by one **ignoring the hold values** (the timer is forced to 0 each step); `f` is wrapped only when it exceeds `n` | on the step that leaves `f = n`, one past the last frame |
| 3 | forward by **two**; `f` is wrapped to 0 only on **equality** with `n` | when `f` reaches `n - 2` and the timer reaches that frame's hold value |
| 4 | as mode 1, except that while `f = 0` and `t = 0` it does not advance unless a value drawn from the global random stream is below 131 (of 32768) | never |
| 5 | as mode 4 with the bound 327 | never |
| 6 | as mode 0, except that a reset starts at `m` | once per cycle |
| 7 | as mode 0 while `f` is not `m`; on `m` it holds the frame and, once per step, rotates the facing **and** the target facing by **-2** of 16 (45 degrees counter-clockwise) until its counter passes 8, then moves to `m + 1` | once per cycle, on the ordinary path |
| 8 | forward by one while `f < n - 1`: it stops on the last frame and holds it | never |
| 9 | forward by one while `f < n - 2`: it stops two frames before the end | never |
| 10 | no step (a reset sets `f = 0, t = 0`) | never |
| 11 | `f` is forced to 0 on every step | never |
| 12 | `f` is forced to `n - 1` on every step | never |
| 13 | **backwards** by one, holds respected, wrapping from below 0 to `n - 1` | when `f` reaches 0 and the timer reaches frame 0's hold value |
| 14 | **forward** by two while `f < n - 2`: it stops two frames before the end | never |
| unrecognised | no step | never |

- **ANIM-040** (observed, 005b7820; high). Modes 4 and 5 are the only consumers of the random stream in the
  animation player, and they draw **only** while `f = 0` and `t = 0`. After a reset the timer is -1, so the first
  step does not draw; from the second step on, one value is drawn per step until the clip starts. The waiting time
  is geometric with mean about 250 steps for mode 4 and about 100 for mode 5 (11.7 s and 4.7 s at the reference
  cadence). The draws are from the single global stream of `spec-ai-combat.md` AI-005 and their order follows the
  element update order of ANIM-101.3.
- **ANIM-041** (inferred, 005b7820 and the shipped data; high). There is **no ping-pong mode**: the back-and-forth
  of the idle cycles is in the data, whose frame lists already contain the frames in a there-and-back order, as
  `sprite-animations.md` records. Modes 13 and 14 are a backward and a forward two-step mode; neither turns around.
- **ANIM-042** (observed, 005b7820; high for the state, **unknown** for its consequences). Mode 2 leaves the frame
  index one past the last frame when it signals, and modes 3 and 14 can step past `n` on an odd-length clip, where
  mode 3's equality-only wrap never fires again. Whether any consumer reads a frame index in those states was not
  established. An implementation must clamp and log rather than read out of range, and must record the divergence.
- **ANIM-043** (observed, 005b7820, 005b7300; medium). Mode 7's rotation count depends on the entry state, which is
  an observable difference: entered in the ordinary flow with the timer at 0 it performs **8** rotations of -2 (a
  full turn) before moving past the marker; entered immediately after a reset, whose timer is -1, it performs
  **9**, which leaves the facing two steps short of a full turn. A marker of 0 puts the rotation at the clip's first
  frame; a marker at the last frame puts it at the end.

### 3.4 Actions, statuses and completion (the VM's open question 6.1)

An actor's queue holds **action elements**, each with an action id, an element kind and typed parameters; the script
creates them through the sequence natives (`spec-script-vm.md` VM-230), the walk pipeline through
`spec-navigation.md` NAV-130, the AI through its own decisions. The executor dispatches on the action id and
answers a status.

- **ANIM-120** (observed, 005b8050, 005b86b0, 00464230, 00467a50, 00585320; high where stated). The statuses and
  their required effects:
  - **0** - the early-completion marker was reached (ANIM-034). The actor marks the current action element as
    having reached its end. A few actions treat this as their completion (ANIM-132).
  - **1** - the animation was started or restarted on this update (ANIM-035). Some actions treat this as their
    completion, which makes them effectively instantaneous.
  - **2** - running; nothing happens.
  - **3** - the action is finished. The actor makes the next queued action element current; if there is none, the
    finished element's **sequence** is told the element is **done** (state 0), which is what lets the script's
    sequence level advance (`spec-script-vm.md` VM-211).
  - **4** - the requested animation is absent from the profile, or the move failed persistently (ANIM-241). The
    actor sets the element's state to **refused** (5), which triggers the abort cascade of VM-217. This is the one
    refusal path established here.
  - Element states are: 0 finished, 2 running, 5 refused, 6 cancelled; 5 and 6 both abort the rest of the sequence,
    and a transition into 0 is only honoured from a running or pending state.
  **Not established**: that any status other than 4 produces a refusal; that selecting the next queued element also
  runs its executor in the same update (it does not follow from what was read); and the relative timing of the
  `ActionChange` callback (which happens at ANIM-102(f), after the executor) against the sequence notification
  (which happens inside the executor's status handling). An implementation must keep those two notifications
  ordered as ANIM-102 states and must not assume more.
- **ANIM-121** (observed, 0046bcf0; high). Independently of the status, an executor may report a small integer
  **finish code** to the actor's own class and store it; the AI and the player-character code read it to know how
  the last action ended. The codes and their meanings belong to `spec-ai-combat.md`.
- **ANIM-122** (observed that a per-kind classification gates admission, 0046abd0, 0046b210; **the results are
  unknown**; low). Before an action element is admitted, the actor classifies it by kind and applies a different
  set of admission tests accordingly; the observable outcomes are "accepted, becomes the current action" and
  "refused, element state 5". Which tests apply to which kind, and which actor states refuse which kinds, was not
  read and is **excluded from clearance**: `spec-script-vm.md` VM-216 is the current statement of the admission
  interface and remains an assumption.
- **ANIM-123** (observed, 00464b20, 0048d510; medium). The natives that queue an animation accept targets other
  than humans (`spec-script-vm.md` natives 49/50/51 accept actor-family elements and target objects; 50 accepts any
  known element). Only the **human** executor was read. The completion rules below are therefore established for
  humans and are **provisional for objects and animals**.

**Completion of the script's element kinds**, for a human target:

| Kind (native) | What the actor does | Completes when |
|---|---|---|
| 0x14 (45, 212, 46, 47, 64), 0x18 | the walk pipeline of `spec-navigation.md` NAV-130/140: internal move actions along the path | when the last move action reports arrival (ANIM-242); a path failure or a persistent blockage refuses the element (status 4) |
| 2 (internal, the door approach test) | nothing: it is a test | at once, and it is a two-way branch: **done** when the actor is within the element's tolerance of its point, or when the element names a sector and the actor is in it; **cancelled** (state 6, aborting the rest of the sequence) otherwise |
| 0x1A (48, and 59 code 1) | sets the target facing and turns (3.7) | when the facing equals the target facing |
| 0xA4 (49, once) | plays the action id in **play mode 0** | at the end of the first cycle (ANIM-033), one update later than the raw timer trace if it was started through the play-only entry (ANIM-035) |
| 0xA5 (50, loop) | plays the action id in **play mode 1** | **never by itself**: mode 1 emits no completion signal, so the element stays running until it is refused, cancelled by the abort cascade, or displaced by another action element. A sequence level containing it never completes on its own |
| 0xA6 (51, freeze at the end) | plays in **play mode 0**; on the completion signal it creates and launches a one-element sequence of kind 0xA7 on the same actor **before** answering status 3 | the 0xA6 element completes at the end of the cycle; because the freeze element is launched first, it is already queued on the actor when the original sequence's next level starts. The 0xA7 element plays the same action in **play mode 12** and its executor answers "running" unconditionally, so it never completes by itself |
| 0xAA / 0xAB (60, 61) | switch or restore the action-id lookup | at once (VM-231) |
| 0x92 (62, 69, speak) | starts the line and its animation | when the line ends (ANIM-140: **unknown** in detail) |
| 0x7F / 0x80 / 0x81 (52, 53, 243) | lock or unlock the AI, clear the highlight | at once (VM-231) |
| 0x15 (57, 70, 71, seek) | a walk that re-targets a moving target; the attached sub-sequence of natives 70/71 is launched when it ends | on arrival; **at once** when the seeking actor is the target itself |
| 0x65 / 0x66 (63, 65) | pick up or put down a carried body | at the end of the animation |
| 0x13 (internal, door), 0xA9, 0xAC-0xAE (lifts) | the action lists of `spec-navigation.md` NAV-171/200 | when the last action of the list ends |
| 0xA0 (internal, wait) | counts down (ANIM-133) | on the update **after** the counter has reached 0 |
| 0x2B (internal, cart hit) | applies the hit of ANIM-305 | at once |
| 0x26 (102) and the posture, strike and reaction kinds of native 59 | the actions of `spec-ai-combat.md` | each plays its clip and completes at its end, except the ones whose executor answers "running" permanently (the guards and the idles) |

- **ANIM-130** (observed, 00464b20; high). The **idle chain**: the idle action plays in mode 0; on each completion
  signal one value is drawn from the global random stream and, when it is a multiple of 10, the actor's action id
  becomes the *fidget* id and is dispatched again within the same update; the fidget plays in mode 0 and on its
  completion sets the action id back to idle. This is the only random draw in the action dispatcher.
- **ANIM-131** (observed, 00464b20; high). The locomotion actions all use **play mode 0**. Their **movement modes**
  differ and must be reproduced per action: the walk-start uses mode **5** with the factor 1.0 passed literally, the
  walk and the run use mode **1**, the sprint uses mode **2**, and the turn and posture actions use mode **0**. For the
  walk, run and sprint the factor is obtained from the current action element (ANIM-200). All of them pass the *next*
  queued action element to the play call so that it can look one record ahead.
- **ANIM-132** (observed, 0048d510, 00464b20; high). Which status an action treats as its completion is
  per-action: most use 3, some use **0** (the early marker; the search action is one), and at least one uses
  **1**, which makes it complete on the update it starts. An implementation must therefore keep the early marker
  (ANIM-034) and the started status, not only the end-of-clip signal.
- **ANIM-133** (observed, 00464230; high). The wait action's counter is tested for zero **before** it is
  decremented: a non-zero counter is decremented and the action stays running; a zero counter completes the action.
  A wait set to `k` therefore completes on the `(k + 1)`-th update after it became current. The waits the walk
  pipeline inserts (`spec-navigation.md` NAV-130: 50, and two values drawn from the random stream) are set in these
  units.
- **ANIM-140** (**unknown**, 00464b20 speech case; low). The speech element is held while its line plays and ends
  when the line ends. Neither the source of the duration (the sound's length, a value in the text data, or a fixed
  fallback) nor the behaviour with sound disabled was read. **Excluded from clearance**: an implementation must
  make the duration an explicit assumption with a documented fallback and must let a replay pin it.

### 3.5 Action-id transitions

- **ANIM-134** (observed, 00464b20, 0048d510; high). The animation player makes exactly one action transition of its
  own: idle to fidget and back (ANIM-130). Every other chain - walk-start into walk, walk into walk-stop, run-start
  into run, sprint-start into sprint, the alert twins, crouch down into sneak and back up - is produced by the order
  and AI layer: each of those actions is an action element that plays its clip, reports a finish code and is popped,
  and the next queued action element decides what follows. An implementation must not put a state machine over
  action ids inside the animation player; the player's contract is "play this id in this mode and tell me when it
  ends". Which ids the AI and the click handlers choose is `spec-ai-combat.md` and `spec-navigation.md` 3.4; the ids
  and their visual roles are `docs/formats/sprite-animations.md`.

### 3.6 Displacement per update

The play-and-move entry point (ANIM-035) is the only thing that moves a character. Its required results, in the
order they must be produced:

- **ANIM-200** (observed, 005b86b0; high). The magnitude offered to the movement step is
  `d = advance(the frame current after the step) x factor`, zero unless the displacement gate of ANIM-032 is open.
  The factor is a float supplied by the caller: the locomotion actions obtain it from the current action element
  (so an action element can carry a speed multiplier; every retail case seen resolves to 1.0), while the walk-start
  action passes 1.0 literally. When `d` is exactly zero the element does not move and no collision work is done for
  it this update.
- **ANIM-201** (observed, 005b86b0, 00464230; high). The displacement is applied along the element's movement
  direction in screen space, with the height taken from the projection plane (`spec-navigation.md` NAV-002). The
  position recorded at the start of the update is what the bond crossing (NAV-160) and the drawing use as the step's
  origin.
- **ANIM-202** (observed, 005b86b0, confirmed against the raw instructions; high for the arithmetic, **unknown**
  for the caller). **Movement mode 3** steps the timer a second time in the same update and accumulates, with
  `a = advance` from the first step and `b` from the second, each contributing only if the gate of ANIM-032 was open
  on that step: both open -> `d = (2a + b) x 2 x factor`; first only -> `d = 2a x factor`; second only ->
  `d = 2b x factor`; neither -> `d = 0`. The first contribution is thus doubled twice when both steps qualify. No
  caller passing mode 3 was found (the human dispatcher passes 1 for the walk and the run, 2 for the sprint, 5 for
  the walk-start and 0 for the rest), so mode 3 is **excluded from clearance**: refuse it with a diagnostic until a
  caller is identified.
- **ANIM-203** (observed, 005b86b0; high). **While the facing differs from the target facing**, and only when a
  non-zero `d` has already been produced, `d` is scaled: movement mode 6 **doubles** it and steps the timer once
  more; every other mode multiplies it by **0.6**; and if the result is then below **0.7** px it is set to 0.7 px.
  The floor applies only to a displacement that already entered the movement path, so a held frame with no
  displacement does not creep.
- **ANIM-204** (observed, 005b86b0; high). Movement **without** collision handling is used when the movement mode is
  7 or 8, or when the element's no-collision flag is set, or when its sector allows it: the position is set, the
  projection plane re-evaluated and the sprite marked dirty. Every other case goes through the collision-aware move
  of 3.9.
- **ANIM-206** (observed, 00560780; high). After moving, the element publishes a velocity vector of
  `d x direction / (hold(the current frame) + 1)`, i.e. the average velocity over the updates the current animation
  frame will be displayed. This is the value other subsystems ask an element for its speed; the camera lock
  (ANIM-340) uses it.
- **ANIM-207** (observed, 005b86b0; medium for the selector). When the element's **ground kind** is 5 and the
  update's displacement exceeds **2.0** px, a phase counter cycles 0, 1, 2 and on every third such update an effect
  of kind 7 is created at the element's position on its layer. The selector compared with 5 is the value the
  placement reader fills from the mission record's ground byte; the same storage is written as a float by the cart
  code (ANIM-301), which was not reconciled (section 10).
- **ANIM-208** (observed, 005b86b0, 00560460; high for the trigger, medium for the test). **Arrival** in every
  movement mode except 5: after the move, if the element moved this update and the arrival test (ANIM-242) answers
  that it has reached its target, the mover answers status 3, which completes the action and with it the script's
  walk element.
- **ANIM-209** (observed, 005b86b0, 00560460; medium). **Movement mode 5** walks a **sequence of queued action
  records** rather than a single target: when the arrival test answers for the current record the mover moves on to
  the next one, re-aims the movement direction and continues within the same update, and it answers status 3 when
  the records are exhausted. The walk-start action and the cart use this mode. How the records are ordered and
  re-entered after an interruption was not settled, and the geometric rule for "the current record's target" is the
  same unsettled arrival test; **excluded from clearance** beyond the two results stated here.

### 3.7 Turning: 16 facings

- **ANIM-210** (observed, 0055f0f0; high). An element keeps a facing and a target facing, both 0..15, 0 = screen-up,
  increasing clockwise. The ordinary turn step is: `delta = (target - facing) mod 16`; if `delta` is 0 the step
  answers "already facing" and nothing changes; if `delta < 8` the facing increases by 1; otherwise it decreases by
  1. A difference of exactly 8 therefore turns **counter-clockwise**. One step is 22.5 degrees.
- **ANIM-211** (observed, 0055f140, 0055f210, 0055f1a0; high for the rules, medium for which actor uses which).
  Four variants exist and the caller or a per-element flag chooses: (a) the ordinary one-step turn; (b) a **two-step**
  turn that moves 2 of 16 while the remaining difference is more than 1 and makes a single one-step correction at the
  end; (c) a **damped** turn, selected by a per-element flag, which accumulates a signed counter and first moves the
  facing on the **third** call of the same sign starting from zero, keeps moving on every later call of that sign,
  and spends one call resetting the counter to zero when the sign reverses; (d) a **delayed** turn that moves one
  step and then answers "turning" for the next `k` calls, where `k` is the caller's argument, so its steps are
  `k + 1` calls apart.
- **ANIM-212** (observed, 005b86b0, 0055f0f0; high for the fact, medium for the count). The turn step is performed by
  the action executor, before the animation is played. It is **not** guaranteed to be one step per update: some
  executor paths perform more than one play or move call within a single actor update, and each such call can carry a
  turn step. An implementation must therefore drive turning from the same place as the play call rather than from the
  update, and a test must count turn steps per play call, not per update.
- **ANIM-213** (observed, 0055fc20; high). The **target facing** of a moving element is derived from its movement
  direction: the direction is converted to world space through the projection plane, quantised to one of 16
  directions, and mirrored (index exclusive-or 8) when the element's reverse flag is set. The refresh happens when
  the element's direction state is recomputed **with the caller's update flag set** and the movement vector's length
  is at least 1.0; a shorter vector leaves the target facing alone. Scripts and the AI set the target facing
  directly (native 94, element kind 0x1A, the AI's stares).
- **ANIM-214** (observed, 005b7820 mode 7; high). Play mode 7 is the only place where the animation player changes
  the facing; it moves the facing and the target facing together (ANIM-043).
- **ANIM-215** (observed, 005b86b0, 005b7300; high). Changing the facing does **not** reset the frame or the timer:
  only the animation index changes (the same block, a different facing). A character keeps its stride through a
  turn; the cost of turning is ANIM-203 alone.

### 3.8 Placement, teleport and off-map

- **ANIM-220** (observed, 00464230, 005b6790, 00462730; high). A pending placement is applied at the start of the
  element's update, before anything else: position, layer and sector, and projection area are set, the position
  recorded as the start of the update is set to the new position (so no bond is crossed and no step is
  interpolated), the depth key is recomputed and the sprite is marked dirty. Placing an element at another
  element's place copies that element's facing into **both** the facing and the target facing, so no turn follows.
- **ANIM-221** (observed, 005b6790; high). A mission placement of a character supplies the initial facing, masked to
  0..15 and written to both the facing and the target facing; an object placement additionally names an initial
  action id, which is reported and ignored when the profile lacks it. For the two character placement kinds the
  loader also writes the raw facing byte into the element's current animation index, which is meaningless until the
  first play call replaces it; an implementation should start from a valid animation instead.
- **ANIM-222** (observed, native 96 of `spec-script-vm.md`, 005b86b0, 00561040; high). An element taken off the map
  keeps its animation state but is not drawn, does not move, and is not a collision partner (the proximity scan
  requires a displayed element).

### 3.9 Collision and arrival

What the caller requires is established; the resolution itself is not, and is **excluded from clearance**.

- **ANIM-240** (observed, 00561040; high for the trigger). The collision-aware move first looks for **close
  neighbours**: it walks the level's element table from index 0 upwards and, for every other element that is
  displayed, is not the element this one carries, and shares this element's layer/sector identity **and** its
  projection area, it applies geometric tests and, for actor-family elements, a centre-distance test against
  **5.0** px; a hit invokes that element's own **proximity reaction** (the per-class behaviour that makes a
  character step aside, stop, wait or be pushed). Target-family elements get the same reaction when their own
  "is solid" answer is non-zero. **The reaction itself was not read**: pushing, waiting and stepping aside are
  unknown (section 10), and so is whether the reaction can move either element within this update.
- **ANIM-241** (observed, 00561040, 00563e90, 00563ea0, 00563ed0; high for the counter, medium for the effect). The
  move is then tested and, on failure, resolved against the geometry of the cells the step crosses. Two pieces of
  state accompany the failures: a **failed-move counter**, incremented on each failure, and a **collision tolerance**
  float that is reduced by **0.2** on each failure while it is still above 1.0 - a shrinking clearance that lets a
  wedged character squeeze through. Both are **reset when a new action element starts** (the tolerance to a stored
  default), so the threshold is per action, not per element lifetime or per update run. After the resolution the
  caller checks the counter and, when it exceeds **50**, reports a failure and answers status **4** (ANIM-120), which
  refuses the action element. Whether the position is rolled back on a failed update, and what the resolution does
  geometrically (sliding along walls and bonds), was not read.
- **ANIM-242** (observed, 00560460, 00467a50 case 2; high that several cases exist, medium for each). The **arrival
  test** has more than one case. The plain case compares the distance from the element to its target with the
  element's own stopping radius and answers "arrived" when the distance is **not greater** than it (an inclusive
  comparison); other cases consult the element's movement direction and the live straight-line test, which makes
  the answer depend on obstruction. Separately, the door-approach element kind 2 uses its own tolerance of
  `radius + 5` px (`spec-navigation.md` NAV-152) with 10 px as the radius the walk pipeline sets. Those two must not
  be conflated: `radius + 5` is the **approach test's** rule, not the general arrival rule.
- **ANIM-243** (observed, 00561040; high). The bonds crossed by the step are applied after it (`spec-navigation.md`
  NAV-160), which is where the height and the ground kind change.

### 3.10 The cart (the "mobile element")

A cart is speed-driven: its speed produces its motion and its animation, the reverse of a character.

- **ANIM-300** (observed, 004ab720; high). Once per update, if the cart is displayed: when its acceleration is
  non-zero, `speed := speed + acceleration`, and when the speed has reached or passed the target speed (tested with
  the sign of the acceleration) the acceleration is cleared and the speed is set exactly to the target. Then, if the
  speed is non-zero, the cart moves by `speed x direction` px per update, in the same screen space and through the
  same projection plane as a character, and it crosses bonds like one.
- **ANIM-301** (observed, 004ab7e0; high for each branch, **unknown** for the selector). The move has two branches
  and they do different things to the cart's sub-sprites (the wheels and the load, which are separate sprite
  elements):
  - both branches store, on each sub-sprite, a value derived from the **reciprocal of the speed** (and a very large
    value when the speed is zero); what consumes it was not read, so the observable "the wheels turn faster when the
    cart goes faster" is an inference;
  - one branch **steps each sub-sprite's animation** once;
  - the other branch gives each sub-sprite a **random rattle**: per sub-sprite, **two** values are drawn from the
    global random stream and turned into an x and a y jitter whose amplitude is `speed x 0.1`, accumulated per axis
    and clamped to the range -1 to 1.
  The rattle is the largest single consumer of the random stream in this subsystem and it must appear in the RNG
  contract (section 8). Which condition selects which branch was not read.
- **ANIM-302** (observed, 004abfe0, 004ac350; high). A cart runs a **program** that comes with the mission: a byte
  stream organised in blocks, each with a length. On each update, while the current block still has length,
  instructions are executed one after another until one of them yields for this update. When a block is exhausted
  the next one is chosen from a table of alternatives, each carrying a weight and a destination, by drawing
  **`rand()` modulo 100, plus 1** from the global stream and walking the table subtracting weights until the roll no
  longer exceeds an entry's weight. One further byte selects between two variants of that table before the roll.
- **ANIM-303** (observed, 004ac350; high for the five effects stated, **unknown** for the rest). The instructions are
  single bytes from 0x80 upwards with their operands, and the block length is reduced by each instruction's size.
  Observable effects: **0x80** sets an action id on every sub-sprite and restarts each from its first frame;
  **0x81** sets the current speed and marks the cart stopped when that speed is zero, moving otherwise; **0x82** sets
  a target speed (a target of zero is replaced by **0.1**) together with the index of the next waypoint and computes
  `acceleration = (target^2 - speed^2) / (2 x the distance to that waypoint)`, an acceleration that would reach the
  target speed at the waypoint in continuous time - the discrete integration of ANIM-300 only guarantees that the
  speed is **clamped** to the target when it passes it, not that it arrives exactly at the waypoint; **0x83** sets the
  next waypoint index, resets a per-sub-sprite value and **yields for this update**; one instruction has **no effect
  in this build** and is reported when it is met. Two further instructions each pass one operand into the cart and
  were not read. Any other byte is reported and the program stops. An out-of-range waypoint index is reported as
  fatal when it is read. The program's container in the mission file was not read. **Excluded from clearance.**
- **ANIM-304** (observed, 004af540; high). A cart crosses bonds exactly like a character (`spec-navigation.md`
  NAV-160); duplicated bonds are reported and ignored.
- **ANIM-305** (observed, 004d9420; high for the traversal and the two parameter pairs, **unknown** for the
  condition). Once per level tick, **before** the element updates (ANIM-101.2), every unordered pair of level
  elements is visited - the outer index from the last element down to the first, the inner index from the outer index
  minus one down to 0 - and for each pair in which one element is a human and the other is a cart whose disabling
  flag is clear, two geometric containment tests are applied using the human's position; when both hold, a
  one-element sequence is created and launched that applies a hit to the human. The hit carries **two** equal
  parameters, either **10 and 10** or **50 and 50**; which pair is used depends on a geometric or directional
  comparison on the cart that was not identified, and calling it "the cart is moving" is **not supported**. The
  traversal order fixes the order in which the hit sequences are queued.

### 3.11 Lifts, stairs and climbs; layer changes

- **ANIM-310** (observed, 005bddb0, `spec-navigation.md` NAV-200; medium, depends on ANIM-013). The ladder and ivy
  climbs play a mount action, a loop action repeated a number of times read from the animation table by action id,
  and a dismount action; the repetition count is therefore a property of the player's sprite file, and the climb's
  duration follows from the loop clip's own timings on the clock of ANIM-003.
- **ANIM-311** (observed, `spec-navigation.md` NAV-171, 00464230; high). A layer change happens only inside the
  door-crossing and lift action lists, between two move actions; an element's layer is constant for a whole update,
  so every collision and drawing decision of that update uses one layer.
- **ANIM-312** (observed, `spec-navigation.md` NAV-190, 00561040; high). A character inside a building is given the
  extra layer index and is not displayed, so the drawing skips it and the proximity scan of ANIM-240 cannot see it
  (it requires a displayed element on the same layer); its element update, and therefore its animation, still runs.

### 3.12 The camera

- **ANIM-320** (observed, 004cec60, 004c7f60, 005758d0, 005a8650, 004c0040; high). The camera is two-dimensional and
  has **no layer and no height**. Its state is: the **top-left corner of the visible world rectangle** in background
  pixels (floats that hold integral values; every write rounds), the current zoom, a requested zoom, a zoom
  transition step counter and a busy flag, a **scroll destination** kept twice (as the script gave it and clamped to
  the map) with a sentinel meaning "no scroll", a **scroll step length**, a **scroll ramp index**, a **scroll speed**
  (an unsigned 16-bit value, 0 meaning "use the step length and the ramp"), the **current camera element** (the
  script element that owns the camera, if any), the **lock target** with an enable flag and its per-axis correction,
  velocity and burst counter, the per-update camera delta, a redraw-mode value and a cached-screen validity flag.
  The visible rectangle is `screen width / zoom` by `(screen height - 80) / zoom`, the 80 px being the strip the HUD
  occupies (ANIM-380), and the corner is clamped independently per axis to `[0, map size - rectangle size]`. All the
  camera publishes to the drawing is the corner and the zoom.
- **ANIM-321** (observed, 004c8380, 005105d0; high). The camera is advanced once per iteration, inside the drawing
  (ANIM-100 step 5), **whether or not the level tick ran** (ANIM-004). In the level's blocking mode the camera
  routine returns early on updates whose frame number is not a multiple of 32 - but **only after** the scroll, zoom
  and lock processing of ANIM-330 to ANIM-340 has already run, so that return suppresses the later part of the
  camera work, not the camera itself.
- **ANIM-322** (observed, 004cec60; high). A camera step that would leave the map is **clipped** to the border, the
  corresponding scroll ramp index is zeroed, and the step reports that it clipped. A **script scroll whose step
  clips is terminated**: its target is cleared, the step length is reset to 1.0, the cached screen is invalidated and
  the camera element is completed (ANIM-330).
- **ANIM-323** (observed, 004be6d0, 004dba50, 004db920; high for the rules, medium for the ramp's exact entries).
  **Scrolling by keys and HUD buttons.** Four commands (up, down, left, right) and two (zoom in, out) arrive as engine
  commands and are dispatched to per-direction handlers. A scroll command produces a camera delta of
  `+/- ramp[index] / zoom` on its axis, so the step is constant in *screen* pixels and therefore halves at zoom 2.0
  and doubles at zoom 0.5, and it clears the "camera is on the selected character" marker while leaving the actor
  lock alone. Each axis has its own direction latch and ramp index: when the command arrives with the latch not yet
  set for this direction the index is set to **0** and the latch is set, otherwise the index increases by one up to
  **31**. Because **entry 0 of the ramp is 0.0**, the first update after starting or reversing a scroll produces
  **no movement**, and the motion begins on the next update. On an update where no command arrived for an axis the
  index is **decreased** and the camera keeps moving, so it coasts to a stop. The two ramps (one per axis) are built
  once from the same rule: entry 0 is 0, then a fractional working value that starts at 6.0 is adjusted to an even
  integer, stored, and multiplied by **1.05** while it is below **31.0**, for 31 entries. The stored sequence
  therefore rises from 6 to a ceiling of 32 px per update over roughly twenty entries; the exact entries depend on
  the rounding the adjustment performs, which was not pinned down, so an implementation must treat the individual
  entries as an assumption and only the endpoints and the monotonic shape as established.
- **ANIM-324** (observed, 004d0400; high for the negative, **unknown** for the trigger). There is **no drag
  panning**: the mouse-drag state drives the rubber-band selection rectangle. Where a mouse position near a screen
  border is turned into the four scroll commands was not found, and the border width is unknown. **Excluded from
  clearance**; until it is settled, implement edge scrolling with the same commands and ramp as the keys and record
  the border width as an assumption.
- **ANIM-325** (observed, 00571200, 004bef3b, 004c7cf0, 004c7cd0; high). The zoom has exactly three values, **0.5,
  1.0 and 2.0**, held as an index into a three-entry table, and 1.0 at level start. Native 21 accepts only those
  three and otherwise reports an error and changes nothing. **Native 21 and element kind 8 write a *requested*
  zoom**, not the current one; the camera update performs the change. A zoom-out request is refused when the zoom is
  already 0.5 or when the map would be smaller than the visible rectangle at the next step; a zoom-in request is
  refused at 2.0.
- **ANIM-326** (observed, 004cf610, 004cfce0, 004c8380; high for the count and the gate, medium for the appearance).
  A zoom change is rendered as a transition over **8** steps, one per update, by blitting the screen captured before
  the change with an interpolated scale; the two directions use different interpolation expressions, so no single
  formula describes both. While the transition runs, a busy flag refuses further zoom commands and the camera reports
  itself busy. At zoom 0.5 the camera corner is snapped to even pixels. **At which step of the transition the current
  zoom value changes was not established**, which is why ANIM-333's completion count is not a settled number.
- **ANIM-327** (observed, 005a8650; high). Zoom scales the whole drawing downstream; it does not select a different
  background, a different resolution or a different layer set, and a change of zoom invalidates the cached backdrop.
- **ANIM-330** (observed, 004ca410 kind 6, 004cdfc0; high). **Scroll to a point.** Starting the element: any previous
  camera element is **completed** (state 0), this element becomes the camera element, the actor lock is released, and
  then either - when the level's no-presentation flag is set - the point is clamped, the camera corner is set to it,
  the element is completed at once and the camera element is cleared; or the raw destination is set to the element's
  point, the active destination to its clamped copy, the scroll speed to the element's speed parameter, the step
  length to **2.0** and the ramp index to **0**.
  Per camera update, while a destination is set: **first** it is tested whether the destination has been reached; if
  it has, the destination is cleared, the step length is reset to 1.0, the cached screen is invalidated, the delta is
  zeroed and the camera element is **completed**. Otherwise the delta is `normalise(destination - corner)` times
  `L`, where `L` is the scroll **speed** when it is non-zero and the step **length** otherwise, shortened to the
  remaining distance when that is smaller, and truncated to whole pixels. If the clamp did not clip: when the step
  length is exactly 1.0 the ramp index is reset to 0, otherwise it increases by one up to 31, and the step length
  becomes the ramp entry at that index (so a default-speed scroll moves 2 px on its first update and then follows the
  ramp, always using the **vertical** ramp whatever the axis). If the clamp clipped, the scroll terminates as
  ANIM-322 says.
  **Because arrival is tested at the start of the next camera update, a scroll that lands exactly on its destination
  completes one update later**, not on the update that lands.
- **ANIM-331** (observed, 00571270, 00571330, confirmed against the raw instructions; high). **What natives 18 and 19
  really do.** Neither moves the camera and neither is a zoom. Each requires a non-null location (otherwise it
  reports an error and returns 0) and then writes the camera's **scroll destination**, both the raw and the
  clamped-to-map copy, from that location's point, and sets the **scroll step length**: native 18 to **2.0**, native
  19 to **its float argument**. The camera's own update (ANIM-330) then scrolls there over the following updates.
  Three consequences an implementation must reproduce: (a) these natives create **no camera element**, so no script
  sequence waits for the scroll and nothing is completed when it arrives; (b) they do **not** set the scroll
  **speed**, so the scroll uses whatever speed the last scroll **element** left there - a non-zero leftover speed
  makes the step length, and therefore native 19's argument, irrelevant; (c) they do **not** reset the ramp index, so
  after the first update the step length continues from the ramp position the previous scroll left. Native 19's
  argument therefore governs the first update of the scroll it starts, and only when the leftover speed is zero.
- **ANIM-332** (observed, 005713f0, 004ca410 kind 7, confirmed against the raw instructions; high). **Native 20 and
  element kind 7 are the jumps.** Native 20 requires a non-null location, clamps its point and writes it to the
  camera **corner**, then invalidates the cached screen so that the next frame is fully redrawn. It touches neither
  the scroll destination, nor the zoom, nor the actor lock. Element kind 7 does the same and additionally completes
  any previous camera element and releases the actor lock, and it **completes itself in the same call**.
- **ANIM-333** (observed, 004ca410 kind 8, 004cdfc0; high for the rule, **unknown** for the frame count). **Element
  kind 8** stores its float as the requested zoom and becomes the camera element. On each camera update, **first**:
  if the requested zoom equals the current zoom, the request is cleared and the camera element is **completed**.
  Then, if a request is pending and no transition is busy, one zoom-in or zoom-out command is raised toward it; if
  the gate of ANIM-325 refuses, the request is set equal to the current zoom, which makes the element complete on the
  **next** update rather than immediately. Because the point in the transition at which the current zoom changes was
  not established (ANIM-326), the number of updates a two-step zoom takes is **not** settled; an implementation must
  distinguish the native's return, the logical zoom, the displayed scale and the element's completion, and must pin
  the count with a replay.
- **ANIM-340** (observed, 004d90b0, 004cdfc0; medium). **Lock on an actor** (element kinds 0xD and 0xE, natives 39
  and 40). Setting a lock stores the actor and enables the lock, and it **can move the camera immediately**: the
  setter positions the camera when the actor is not within the checked view, so "the camera never re-centres" is
  wrong. While the lock is enabled: the lock is dropped when the target reports itself gone or dead; otherwise, when
  the burst counter is zero, a per-axis correction is recomputed from the actor's position, the remembered offset and
  the zoom, with an overshoot guard that zeroes an axis whose new correction is smaller in magnitude than the stored
  one, and the burst counter is set to **15**; an axis whose correction magnitude is above the comparison value
  (which is **0.0**, so any non-zero correction) has its velocity set to `sign x the actor's own published speed`
  (ANIM-206), so the camera matches the followed character's gait; corrections are rounded to whole pixels. While the
  burst counter is non-zero the stored per-axis velocity is applied, the counter is decremented, an axis whose
  correction has been consumed is zeroed, and the step is clamped like any other. A separate closing rate for small
  corrections exists in the part of this routine whose decompilation is folded and was not confirmed. **Excluded from
  clearance**: the exact traces for an already visible actor, an off-screen actor, a stopping or reversing actor and
  a border clip must be pinned by recordings before this is implemented as fact.
- **ANIM-341** (observed, 004ca410, 004dba50; high). The lock is released by element kinds 6, 7, 0xD and 0xE (each of
  which calls the same setter, with the actor for 0xD and with nothing for the others), and it is dropped when the
  target is gone. **Player scrolling does not release it**: it clears only the separate marker that says the camera is
  centred on the selected character. **Native 20 does not release it either** - its only other write is the
  cached-screen flag.
- **ANIM-342** (observed, 0050f710, 004c8380; high). Nothing suspends the camera while a dialogue or a text page is
  open: those suspend the level tick from their own loop, and the camera lives in the drawing (ANIM-004). A running
  script scroll therefore continues, and can complete, while the tick is suspended. Entering a building does not move
  the camera.

### 3.13 Drawing one frame

The composition is required behaviour because it decides what is visible and what hides what. The pass ordering is
required; how it is organised is not.

- **ANIM-360** (observed, 005105d0; high). Required order: the **camera and the scene** (ANIM-361), then the **HUD
  widgets**, then an optional on-screen message clipped to a band at the bottom of the screen, then the **HUD panel
  and its text**, then the **mouse cursor**, then the **present**. The whole of it is skipped under the gate of
  ANIM-521.
- **ANIM-361** (observed, 004c8380; high for the ordinary case). In the ordinary case the scene begins with a full
  opaque copy of a **cached backdrop** into the back buffer and then runs the element passes; there are **no dirty
  rectangles**, so every frame redraws everything on top of that copy. The cached backdrop itself is maintained
  incrementally: it self-blits by the scroll delta and re-renders only the band the scroll exposed. **While a zoom
  transition is running the scene is not composed this way**: the transition blits captured surfaces (ANIM-326), so
  the ordinary pass list does not run on those updates.
- **ANIM-362** (observed, 004d0a10; high). Required order of the element passes: (1) a visibility and culling
  refresh, only when a dirty flag is set; (2) the camera transform and the clip box, including the vertical offset
  of **-80** px; (3) depth-key propagation for attached effects; (4) the **backdrop elements**; (5) the terrain patch
  renderer; (6) the **selection and highlight underlays**; (7) the **shadows**; (8) the **ground marks**; (9) the
  binding of queued draw items to their elements; (10) **the sort and the merge** (ANIM-363); (11) the **main element
  pass** in the merged order, flushing the deferred queue up to each actor's depth key before drawing that actor;
  (12) a final flush of the deferred queue; (13) the stretched sprites; (14) the movement-path line and its trail
  marks, only while the cursor is not over a pickable element; (15) floating text and labels; (16) a clearing of the
  per-element "drawn" flags. Everything after that is debug overlay, unreachable in the retail build's default state.
- **ANIM-363** (observed, 004d1d00, 00462a30, 004aa980; high for the actor comparison, medium for the rest). The
  movable elements are **sorted** and then **merged** into the scenery order: the scenery's order comes from the level
  file and is treated as already correct; walking it in that order, and consuming the sorted movables from the front,
  every movable that the scenery test places on the **far** side of the current piece - behind it, so that the piece
  must cover it - is emitted **before** it in the painter's order, and the movables left over are emitted after the
  last piece. The scenery test is the classic 2.5-dimensional **sort line**: the piece
  carries a polyline, the segment spanning the movable's screen x is found, and the movable's side of that segment
  decides; a piece with no polyline falls back to comparing positions by world row. The comparison the sort applies to
  a pair of actors is: **the depth key ascending, ties broken by the element's own identity ascending** - a total,
  deterministic order. **Not established**: that every sortable family uses that same comparison (the sort is reached
  through an intermediate whose body was not read), and the endpoint and equality results of the sort line.
- **ANIM-364** (observed, 00462a30, 005bd560, 0055f290; high). **The depth key is the element's world row** (the world
  y, not the screen row: an object lifted onto a roof sorts at the depth of the ground under it). Two adjustments
  exist: an element bound to another takes the other's key plus or minus **0.001**, and an effect attached to an owner
  takes the owner's key plus or minus **0.01**; one kind of spawned effect adds **1000.1**. These offsets order the
  pair **relative to each other**; they do **not** guarantee that nothing else sorts between them, and the 1000.1
  case cannot be described as "in front of everything" without knowing the range of world rows in play.
- **ANIM-365** (observed, 004d0a10, 005c73e0, 005c2ec0; high). Effects and decorations are not a separate pass: they
  are queued with their own depth value and flushed into the main pass at the point where the queue's head is no
  longer nearer than the element about to be drawn, with a final flush at a sentinel depth of 1 000 000.
- **ANIM-366** (observed, 004d0a10; high for the negative at the top level, **unknown** for the rest). There is **no
  separate occluder or mask pass** in the scene's pass list: a building hides a character because it is an element in
  the merged list drawn after the character. That is not proof that no element's own drawing uses mask data: the level
  file's mask and sector data is loaded with the background and **how it becomes the drawable pieces was not read**.
  **Excluded from clearance.**
- **ANIM-367** (observed, 004d0a10, 004c0510; high for the ordering, medium for the meaning). Drawing is **not layer by
  layer**: there is one merged list for the whole view. The layer index is a field of the element and selects which
  background and mask set it belongs to; the loader accepts indices from 0 to the layer count inclusive, one more than
  the number of layers, which is the extra index a character inside a building gets (`spec-navigation.md` NAV-190).
  Only the debug overlays iterate layers.
- **ANIM-370** (observed, 0051c0d0, 0051bfb0, 004cac00, 004d7dd0; high for the ageing rule, medium for the visual).
  **Ground marks** are a level-owned collection, unbounded in number, each holding a position, an animation frame index
  and a layer. They are created when the player issues an order (the marker at the destination) and, while the
  movement-path line is drawn, once every 11th draw along the trail. Their ageing happens **inside the drawing pass**
  and only for a mark that passes the pass's visibility test: for such a mark the frame index advances by one on the
  updates whose level frame number is even, and a mark whose index reaches **6** is destroyed. A mark's life is
  therefore 12 *visible, drawn* updates - it does not age while it is off-screen, while the drawing is skipped, or
  during a zoom transition.
- **ANIM-380** (observed, 0052cf80, 0052d090, 004c0040, 00677598; high). **Resolution and viewport.** The mission view
  is the full configured width by the configured **height minus 80** px, anchored at the top left; the accepted modes
  are 1024x768, 1228x768 and 1360x768 (so the world view is 688 px high), the default is 1024x768, and an unknown
  width is refused with a report. **There is no letterboxing and no border**: a wider mode shows more of the world.
  Video playback switches temporarily to 640x480. Whether the two wide modes are original or were added for this
  release was not established.
- **ANIM-381** (observed, 005e41d0, 005e5260; high). The back buffer is a double-buffered flip chain in 15-bit or
  16-bit colour with no 8-bit path; presenting is a flip when fullscreen and a blit to the primary surface when
  windowed; the sprite blits key on pure green.

### 3.14 Settings, speed and difficulty

- **ANIM-520** (observed, 005460c0, 0050f710; high for the selection, medium for the realised ratio). A **slow-motion**
  toggle selects the 400 ms pacing minimum instead of 40 ms (ANIM-001). The **nominal** ratio is 10. The **realised**
  ratio is not 10 and is host-dependent for the same reason as ANIM-002: on a host with the 15.625 ms granularity the
  measured cadence would be 26 intervals (406.25 ms) against three (46.875 ms), a ratio of about 8.7. Nothing else
  changes: the animation clock, the movement per update and the camera rules are all per-update rules, so slow motion
  scales time and nothing else.
- **ANIM-521** (observed, 0050f710, 005105d0; medium-high). Two flags remove the pacing wait: a **freeze** flag, which
  also blocks the level tick and the whole drawing and which nothing in this build ever sets, and a level-side
  **blocking** flag, which removes the wait, the cursor and the present and makes the later part of the camera work
  run only on every 32nd update (ANIM-321). While either is set the loop runs uncapped. A frozen game does not
  "shrink durations": nothing advances at all while the freeze flag is set, whereas the blocking flag leaves the tick
  running uncapped, which does compress real time.
- **ANIM-522** (observed, 0055d1a0, 0051ba00, 005b2190, 00409d70, 005a6520; high). The persisted settings are the
  player profile, two key-binding sets (also as two configuration files under the game's data directory), the sound
  configuration and the graphics configuration (the resolution and four further bytes). The registry holds only the
  sound device, the language, the version and the path. The command line has no timing switch. None of these changes
  any rule in sections 3.1 to 3.13.
- **ANIM-523** (observed, 0055dbb0, 00438710, 00438600, 004936f0; high). **A difficulty setting exists in this build**
  and this specification defers to `spec-ai-combat.md` AI-045 for it. What is established here: a difficulty value
  lives in the profile-side state and is reached through a single accessor; a scaling helper takes a value together
  with **two caller-supplied factors and a cap** and returns the value scaled by the first factor at difficulty **0**,
  by the second at difficulty **2**, and unchanged at difficulty **1**, truncated to an integer; one caller applies the
  factors **0.5** and **2.0** with a cap of **100** to a per-character quantity, and the player-character update
  consults the same value. Difficulty therefore changes AI and combat quantities, and an implementation must
  reproduce it. It changes **nothing** in this specification: the pacing, the animation clock, the displacement rules,
  the turning, the camera and the drawing are all independent of it. The revision-1 claim that no difficulty setting
  exists is **withdrawn**; AI-045 stands.

## 4. Claims, clearance and assumptions

### 4.1 Claim index

Every claim of sections 2 and 3 carries its id, status, evidence address and confidence inline as **ANIM-nnn**
(status, address; confidence). Ranges: **001-004** clocks and execution opportunities; **010-014** the animation
table; **020-023** state and the snapshot; **030-036** the frame timer and the entry points; **040-043** the play
modes; **100-103** the frame, the tick phases and the update order; **120-134** actions, statuses, completion and
transitions; **200-209** displacement; **210-215** turning; **220-222** placement; **240-243** collision and
arrival; **300-305** the cart; **310-312** lifts and layers; **320-342** the camera; **360-381** drawing;
**520-523** settings and difficulty. The mode table of 3.3 is covered by ANIM-030 to ANIM-043, the completion table
of 3.4 by ANIM-120 to ANIM-140 together with the native rows of section 6, and the pass list of 3.13 by ANIM-360 to
ANIM-367.

### 4.2 Cleared for implementation

The clocks and pacing as stated in ANIM-001 to ANIM-004 (with the host-dependence of ANIM-002 carried through); the
animation table's use (ANIM-010, ANIM-011, ANIM-014); the state list of ANIM-020 to ANIM-023; the frame timer and
the entry points (ANIM-030 to ANIM-036); the play modes and their completion signals (3.3, ANIM-040 to ANIM-043);
the frame and tick ordering (ANIM-100 to ANIM-103); the statuses and their effects as far as ANIM-120 states them,
the idle chain (ANIM-130), the per-action completion status choice (ANIM-132), the wait boundary (ANIM-133) and the
transition responsibility (ANIM-134); displacement (ANIM-200, ANIM-201, ANIM-203, ANIM-204, ANIM-206 to ANIM-208);
turning (ANIM-210 to ANIM-215); placement (ANIM-220 to ANIM-222); the collision trigger and the failure counter
(ANIM-240, ANIM-241); bonds (ANIM-243); the cart's speed integration and program structure (ANIM-300, ANIM-302,
ANIM-304) and the traversal of ANIM-305; the camera state, clipping, key scrolling, zoom values, scroll and jump
semantics and the natives (ANIM-320 to ANIM-323, ANIM-325, ANIM-327, ANIM-330 to ANIM-332, ANIM-341, ANIM-342);
the drawing order and the depth key (ANIM-360 to ANIM-365, ANIM-367, ANIM-370, ANIM-380, ANIM-381); the settings
and difficulty statements (ANIM-520 to ANIM-523).

### 4.3 Excluded from clearance (and the `Assumption` variant each becomes)

| Claim | What is missing | Until then |
|---|---|---|
| ANIM-012, ANIM-013 | which animation-record field is the marker and which the loop length | `AnimMarkerField`: read the field the sprite loader fills; do not rely on play modes 6 and 7, and treat ANIM-034's marker source as provisional |
| ANIM-034 | the marker's purpose, and which actions depend on the early status in profiles other than the ones read | `AnimEarlyMarker` |
| ANIM-042 | whether any consumer reads the out-of-range frame states of modes 2, 3 and 14 | `AnimOutOfRangeFrame`: clamp and log |
| ANIM-043 | mode 7's entry-state dependence beyond the two traces given | `AnimTurnTable` |
| ANIM-120 (the parts marked not established), ANIM-122 | the per-kind admission tests and their refusal results; whether any status but 4 refuses | `ActorAdmission` (already carried by `spec-script-vm.md` VM-216) |
| ANIM-123 | the executors of non-human targets of natives 49/50/51 | `AnimTargetFamily` |
| ANIM-140 | the speech duration's source and its fallback | `SpeechDuration` |
| ANIM-202 | which class passes movement mode 3 | `MoveMode3`: refuse with a diagnostic |
| ANIM-207 | which stored value the ground-kind selector is | `FootstepGroundKind` |
| ANIM-209 | the queued-record traversal and re-entry after an interruption | `WaypointRecords` |
| ANIM-240 | the proximity reaction: pushing, waiting, stepping aside | `CharacterPush` (also `spec-navigation.md` open question 2) |
| ANIM-241 | the resolution geometry (sliding) and whether a failed update rolls the position back | `CollisionSlide` |
| ANIM-242 | the arrival test's non-plain cases | `ArrivalGeometry` |
| ANIM-301, ANIM-303 | the cart branch selector, the two unread instructions, the sub-sprite value's consumer, the program's container in the mission file | `CartProgram` |
| ANIM-305 | which comparison selects the larger hit parameters | `CartHitSeverity` |
| ANIM-324 | the screen-edge scroll trigger and the border width | `EdgeScroll` |
| ANIM-326, ANIM-333 | when the current zoom changes during a transition, and hence the zoom element's exact duration | `ZoomTransition` |
| ANIM-340 | the follow controller's exact traces | `CameraFollow` |
| ANIM-363 | that every sortable family uses the same comparison; the sort line's endpoint and equality results | `DrawOrderFamilies` |
| ANIM-366 | how the level's mask data becomes drawable pieces | `MaskIntegration` |
| ANIM-380 | whether the two wide resolutions are original | `WideModes` |

## 5. Constants

Individual functional facts. No table of the shipped data is reproduced: the per-frame hold and advance values, the
marker and loop fields and the per-animation displacement are read from the player's `DATA/Characters/*.rhs` files by
the rules of `docs/formats/sprite-animations.md` (rules 1-3) and ANIM-011.

| Name (ours) | Value | Unit | Source | Conf. |
|---|---|---|---|---|
| pacing minimum, nominal | 40 | ms of reported elapsed time | 0050f710 | high |
| pacing minimum, slow motion | 400 | ms of reported elapsed time | 0050f710 | high |
| reference cadence on the measured host | 46.875 | ms per frame | 0050f710 + section 9 | medium (host-dependent) |
| frame display length | hold value + 1 | updates | 005b7820 | high |
| timer value a reset installs | -1 (mode 10: 0) | timer units | 005b7300 | high |
| facings | 16 | - | 0055f0f0, 005b86b0 | high |
| turn step (ordinary / fast / mode 7) | 1 / 2 / -2 | of 16 per call | 0055f0f0, 0055f140, 005b7820 | high |
| damped turn: first move | on the 3rd same-sign call from zero | calls | 0055f210 | high |
| delayed turn: step spacing | the caller's value + 1 | calls | 0055f1a0 | high |
| mode 7 rotations (ordinary / after a reset) | 8 / 9 | steps of -2 | 005b7820 | medium |
| turning advance factor | 0.6 (movement mode 6: x2) | - | 005b86b0, constant at 00679e40 | high |
| turning advance floor | 0.7 | px per update | 005b86b0, constant at 00677db8 | high |
| stochastic idle bound (play mode 4 / 5) | 131 / 327 out of 32768 | per drawing call | 005b7820 | high |
| idle to fidget probability | 1 in 10 per idle cycle | - | 00464b20 | high |
| door-approach tolerance | the order's radius + 5 | px | 00467a50 case 2, constant at 0067748c | high |
| failed-move limit within one action | 50 (the 51st fails) | failures | 00563e90 | high |
| collision tolerance decrement per failure | 0.2, while above 1.0 | tolerance units | 00563ed0, constant at 00677b3c | high |
| proximity distance (actor family) | 5.0 | px | 00561040, constant at 0067748c | high |
| footstep effect selector value | 5 | ground kind | 005b86b0 | medium |
| footstep displacement threshold | 2.0 | px per update | 005b86b0, constant at 006774f4 | high |
| footstep effect period | every 3rd qualifying update | updates | 005b86b0 | high |
| cart target speed replacement for zero | 0.1 | px per update | 004ac350, constant at 00677580 | high |
| cart rattle amplitude | speed x 0.1, clamped to -1..1 | px | 004ab7e0, constant at 00677580 | high |
| cart branch roll | 1 + rand() mod 100 | weight units | 004abfe0 | high |
| cart hit parameters | (10, 10) or (50, 50) | as the hit action defines | 004d9420 | high (values) / unknown (selector) |
| zoom values | 0.5, 1.0, 2.0 | factor | 00571200, 004bef3b | high |
| zoom at level start | 1.0 | factor | 004bef59 | high |
| zoom transition steps | 8 | updates of rendering | 004c9119 | high |
| camera scroll ramp: entry 0, first moving value, growth, ceiling, entries | 0.0, 6.0, x1.05, 32 (bound 31.0), 32 | px per update | 004bef74, 006777d0, 00678b30 | medium |
| camera scroll step scaling | ramp entry / zoom | px per update | 004dba50, 004db920 | high |
| script scroll step length at element start and from native 18 | 2.0 | px per update | 004ca410, 00571270 | high |
| script scroll step length, neutral | 1.0 | px per update | 004cdfc0 | high |
| script scroll speed parameter | unsigned 16-bit, 0 = use the ramp | px per update | 004cdfc0 | high |
| camera lock: correction comparison value | 0.0 | px | 004cdfc0, constant at 00677d90 | high |
| camera lock: burst length | 15 | updates | 004cdfc0 | medium |
| HUD strip excluded from the world view | 80 | px | 00677598 | high |
| depth key sibling offset | 0.001 | world rows | 005bd560, constant at 006774cc | high |
| depth key attached-effect offset | 0.01 | world rows | 005c72a0, constant at 006774f0 | high |
| depth key spawned-effect offset | 1000.1 | world rows | 005c2ec0, constant at 00679e58 | high |
| deferred queue flush sentinel | 1000000 | world rows | 004d0a10 | high |
| ground mark animation frames | 6 | frames | 0051c0d0 | high |
| ground mark ageing period | every 2nd level frame, while visible and drawn | - | 0051c0d0 | high |
| trail mark period | every 11th draw | draws | 004d7dd0 | high |
| resolutions | 1024x768, 1228x768, 1360x768 | px | 0052cf80 | high |
| world view height | screen height - 80 (688) | px | 004c0040, 00677598 | high |
| colour depth | 15 or 16 | bpp | 005e41d0 | high |
| staggered bookkeeping period / phase | 64 / the element identity's low 5 bits | updates | 00471b00 | high |
| difficulty scaling | factor A at level 0, factor B at level 2, unchanged at 1, truncated | - | 00438710 | high |

## 6. Interfaces to the script VM

The arity, coercion and error conventions are `spec-script-vm.md` VM-085 to VM-089. `ActionChange` is a
compatibility token (the scripts name it).

| Id / kind | What it really does | Completion | Claim |
|---|---|---|---|
| 18 `(loc)` | requires a non-null location; sets the camera's **scroll destination** (raw and clamped) and the **scroll step length to 2.0**. No jump, no zoom, no element, no effect on the scroll speed or the ramp index | nothing waits for it | ANIM-331 |
| 19 `(loc, f)` | as 18 with the step length := `f`. Effective only on the scroll's first update, and only when the leftover scroll speed is 0 | nothing waits for it | ANIM-331 |
| 20 `(loc)` | sets the camera **corner** to the clamped point - an immediate jump - and invalidates the cached screen. Does not touch the scroll destination, the zoom or the actor lock | immediate | ANIM-332 |
| 21 `(f)` | sets the **requested** zoom; only 0.5, 1.0, 2.0 accepted, else an error and no change | the camera update performs it | ANIM-325, ANIM-326 |
| 33 / 42, kind 6 | scroll the camera to a point; 33 records speed 0 ("use the step length and the ramp"), 42 records an unsigned 16-bit speed in px per update. Starting it completes any previous camera element, releases the actor lock, sets the destination, the speed, step length 2.0 and ramp index 0 | when the destination is found to be reached at the **start of a later** camera update, or when a step clips at a map border; at once when the level's no-presentation flag is set | ANIM-330, ANIM-322 |
| 34, kind 7 | jump the camera to a point, completing any previous camera element and releasing the actor lock | at once | ANIM-332 |
| 35, kind 8 | set the **requested** zoom | on a camera update where the requested zoom equals the current one; a refused request is made equal so the element completes on the next update; the exact count for an accepted change is unsettled | ANIM-333 |
| 39 / 40, kinds 0xD / 0xE | set or release the actor lock; setting can move the camera at once | at once | ANIM-340 |
| 49, kind 0xA4 | play an action id in play mode 0 | at the end of the first cycle | 3.4 |
| 50, kind 0xA5 | play an action id in play mode 1 | never by itself | 3.4 |
| 51, kind 0xA6 | play in mode 0 and, on the completion signal, launch a freeze element (kind 0xA7, play mode 12) before reporting | the 0xA6 element at the end of the cycle, with the freeze element already queued; the freeze element never | 3.4 |
| 60 / 61, kinds 0xAA / 0xAB | switch or restore the action-id lookup | at once | ANIM-014 |
| 45 / 212 / 46 / 47 / 64, kind 0x14 | a walk (`spec-navigation.md` NAV-130) | on arrival; a path failure or a persistent blockage refuses it | 3.4, ANIM-208 |
| 48 and 59 code 1, kind 0x1A | turn to a point or a facing | when the facing equals the target facing | ANIM-210 |
| 57 / 70 / 71, kind 0x15 | seek an actor | on arrival; at once when the seeker is the target | 3.4 |
| 62 / 69, kind 0x92 | speak | when the line ends (unsettled) | ANIM-140 |
| 67 / 68 / 72 / 73, kinds 0x9C-0x9F | start / stop / activate / deactivate a cart | at once | 3.10 |
| 93 / 94 | read / set the facing (`d mod 16`); setting writes the facing itself, so no turn follows | - | ANIM-213 |
| 96 / 156 / 152 | place off the map / into a building / out of one | - | ANIM-220, ANIM-312 |
| 101 | the current action id, 283 when none | - | ANIM-102 |
| 103 | stop the actor | - | 3.4 |
| 140 | the walking style the AI uses, i.e. which locomotion action id it chooses | - | ANIM-131 note in ANIM-134 |
| `ActionChange(current, previous)` | runs from the actor's update when the current action id changes, with that actor as the current actor; 283 means none | - | ANIM-102 |

## 7. Acceptance tests

All fixtures are **synthetic**: an animation is described by its frame count, its per-frame hold values and its
per-frame advances, chosen here for the boundary they exercise. Where a test needs the player's data it names the
file and the field instead of quoting values: the hold and advance halves of the frame timing word and the frame
count of a named action id of a named profile, read by the rules of `docs/formats/sprite-animations.md` and
ANIM-011. Times use the reference cadence of ANIM-002; a test that fixes the engine's update rate elsewhere must
scale them, and no test may assert an absolute duration without stating the cadence it assumed.

**Timer and modes**

1. *Frame display length.* Synthetic clip A: 3 frames, holds (0, 2, 0), advances (0, 0, 0), play mode 0, driven
   through the **play-and-move** entry with a fresh action id. The frame index after successive updates must be
   0, 1, 1, 1, 2, 0, 1, 1, 1, 2, ...: frame 0 once, frame 1 three times, frame 2 once, a cycle of 5 updates equal to
   the sum of `hold + 1`. The completion signal must fire on the update that lands on frame 2 and again one cycle
   later.
2. *Entry points differ.* The same clip A started through the **play-only** entry must answer status 1 on the
   admitting update and must not step: its frame trace is the trace of test 1 shifted by one update. A second action
   element carrying the **same** action id must answer status 1, recompute the early marker, and **not** reset the
   frame or the timer.
3. *Early marker.* Clip B: 4 frames, holds (0, 0, 3, 0), marker field = 3. The stored pair must be
   `(2, 3)` and status 0 must be answered on the update where the timer reaches 3 on frame 2 - one displayed frame
   before the clip's end. Clip C: 1 frame, hold 1: the stored pair must be unreachable and status 0 must never be
   answered. Clip D: marker 0, first hold 0: the pair must be `(1, 0)`; with the first hold non-zero it must be
   `(0, 1)`.
4. *Displacement gate.* Clip E: 2 frames, holds (1, 0), advances (5, 3). Driven from a fresh action id, the
   displacement per update must be 5 (the reset step, where the timer reaches 0 without the frame changing), 0, 3,
   5, 0, 3, ... A test that gates on "the frame index changed" produces 0 on the first update and fails.
5. *One-frame clip.* Clip F: 1 frame, hold 0, advance 2, mode 0: the displacement must be 2 on **every** update.
6. *Mode boundaries.* Mode 14 on a 6-frame clip must stop with the frame index at 4 - forward by two, stopping two
   before the end - and must never signal. Mode 9 must stop at index `n - 2`, mode 8 at `n - 1`. Mode 3 on a
   **5**-frame clip must be refused or clamped by the implementation with a diagnostic (the original's wrap tests
   equality and never fires on an odd length); on a 6-frame clip it must signal at index 4. Mode 2 must signal on
   the update that leaves the index at `n`, and the implementation must clamp that state rather than read it.
7. *Stochastic idle.* Mode 4 from a fresh reset must draw **no** random value on its first update and exactly one
   per update afterwards while it sits at frame 0 with the timer at 0; with a seeded stream the clip must start on
   the first draw below 131. Mode 5 with the bound 327.
8. *Turn table.* Mode 7 with the marker at the last frame, entered in the ordinary flow, must rotate the facing by
   -2 eight times on successive updates and then move past the marker; entered on the update after a reset it must
   rotate nine times. Both must move the target facing with the facing.

**Actions and completion**

9. *Walk cycle timing and speed.* Using `RobinHood`'s walk action id, read its frame count and its per-frame halves
   from the file: with every hold 0 the animation must change frame every update and displace that frame's advance
   every update, and the cycle's average speed must equal `sum(advance) / (sum(hold + 1) x cadence)`. The same
   formula must be checked for the same profile's run and crouched-walk action ids and for `Soldier A00`'s walk,
   alert walk and sprint. The crouched walk is the important case: its holds are not all zero, so the test must
   assert the **per-update** positions, not only the average.
10. *Wait boundary.* A wait action set to 3 must stay running on the updates where its counter reads 3, 2, 1 and 0
    and must complete on the update after the counter reached 0 - four decrements, completion on the fifth update.
11. *Script animation elements.* Native 49 on a human whose profile has the action must complete at the end of the
    clip (one update later than test 1's raw trace if the element was admitted through the play-only entry). Native
    50 must never complete, and the test must assert that the sequence's next level never starts and that the
    element can still be refused or cancelled. Native 51 must complete at the end of the clip **and** the freeze
    element must already be queued on the actor when the next level starts; the actor must then hold the clip's last
    frame indefinitely. All three tests must be marked as established for human targets only (ANIM-123).
12. *Refusal.* A walk order whose path search fails, and a walk that fails its move resolution 51 times within one
    action element, must both refuse the element (state 5) and abort the rest of the sequence. The failure counter
    must reset when the next action element starts.
13. *Turning.* From facing 0: target 5 must take 5 calls, target 11 must take 5 calls the other way, target 8 must
    go 15, 14, ... and take 8 calls. The damped variant must first move on the third same-sign call and must spend
    one call resetting after a reversal. The delayed variant with argument 2 must move every third call. Tests must
    count **play calls**, not updates (ANIM-212).
14. *Turning cost and stride.* While the facing differs from the target, a displacement of 4 px must become 2.4 px,
    and one of 1 px must become 0.7 px; a zero displacement must stay zero. Changing the target facing mid-walk must
    not reset the frame index or the timer: the next update must show the ordinary next step of the timer with the
    new facing's animation.

**Camera**

15. *Key scrolling.* From rest, "scroll right" must produce **no** movement on the first update (ramp entry 0) and
    then rise through the ramp to the ceiling; at zoom 2.0 the same commands must move half as far in world pixels
    and at zoom 0.5 twice as far. Releasing must walk the ramp index back down while still moving. Reversing must
    set the index to 0, so the update after the reversal must also produce no movement.
16. *Script scroll.* Native 42 with speed 10 to a point 105 px away must move ten steps of 10 px and one shortened
    step of 5 px, and must complete on the **camera update after** the one that lands - the twelfth. Native 33
    (speed 0) must move 2 px on its first update and then follow the ramp. A scroll whose line leaves the map must
    complete at the border instead of hanging.
17. *Natives 18, 19 and 20.* After native 18 the camera must not move in that call, the zoom must be unchanged, and
    a scroll must begin on the following updates with a first step of 2 px, with **nothing waiting** for it. After
    native 19 with 7.0 the first step must be 7 px - but only when the scroll speed left by the last scroll element
    is 0; with a leftover speed of 10 the first step must be 10 px and the argument must have no effect. After
    native 20 the camera corner must equal the clamped point immediately and the next frame must be fully redrawn,
    with the scroll destination, the zoom and any actor lock untouched.
18. *Zoom.* Native 35 from 2.0 to 0.5 must reach 0.5 and complete; the test must pin the number of updates with a
    recording rather than assert one, and must assert that a refused request (already at the limit, or a map too
    small) completes on the next update, not the same one.
19. *Camera under a suspended tick.* With the level tick suspended by an open text page, a running script scroll
    must continue and complete, and key scrolling must still work.

**Drawing**

20. *Depth order.* Two characters on the same world row must be drawn in ascending element identity. A character
    carrying another must be ordered relative to it by the 0.001 offset, and an attached effect relative to its
    owner by 0.01 - the test must assert the **relative** order of the pair and must not assert that nothing sorts
    between them. A character and a building whose sort line passes between them must be ordered by the side of that
    line, not by the depth key.
21. *Ground marks.* A mark must age one frame on every second level frame **while it is drawn and visible**, and
    must disappear on reaching frame 6; a mark scrolled off the screen must not age, and must resume ageing when it
    comes back. Many marks must all survive (there is no cap).
22. *Slow motion.* With slow motion on, every per-update value above must be unchanged and every duration must scale
    by the pacing ratio the test's host produces, which the test must measure rather than assume to be 10.

## 8. Implementation choices, snapshot, RNG and departures

**What is the original's behaviour**: everything in sections 2, 3 and 6 that section 4.2 clears. Everything in 4.3
is an `Assumption` until it is settled.

**Snapshot contract.** ANIM-020 to ANIM-023 list the state a snapshot or save must be able to reproduce. The items a
naive snapshot omits and that change observable behaviour: the frame timer (a reload that zeroes it shifts every
animation and therefore every element completion), the early-completion marker pair, the action id the timer was
last reset for, the turn delay and hysteresis counters, the failed-move counter and the collision tolerance, the
footstep phase counter, the animation-override selection, the cart's program position, block remainder, speed,
target speed, acceleration and rattle accumulators, and the camera's ramp index, leftover scroll speed, lock burst
counter and per-axis corrections. Initialisation values are the ones ANIM-030, ANIM-241 and ANIM-323 state; the
values a fresh level installs for the camera are ANIM-325 (zoom 1.0) and "no scroll, neutral step length".

**RNG contract.** This subsystem draws from the single global stream of `spec-ai-combat.md` AI-005/AI-006, in the
order the element update order of ANIM-101.3 imposes, in exactly these places:

1. the idle-to-fidget roll, one draw per completed idle cycle of an actor (ANIM-130);
2. the stochastic idle of play modes 4 and 5, one draw per update per element sitting at the start of such a clip,
   and none on the update after a reset (ANIM-040);
3. the cart's rattle: **two draws per sub-sprite per update** for every cart in the branch of ANIM-301 that rattles -
   quantitatively the largest consumer here;
4. the cart program's weighted branch, one draw per block (ANIM-302);
5. the building-entry waits of the walk pipeline, two draws per wait (`spec-navigation.md` NAV-130).

Any claim of trace equivalence with the original must account for all five. An implementation that uses separate
named streams must keep the per-element order so that a replay is reproducible, and must state that the absolute
sequence then differs from the original's.

**Deliberate departures, each to be recorded as such:**

1. **Fix the engine's update rate at 46.875 ms (64/3 Hz).** This is an **OpenSherwood decision**, not recovered
   authorial intent: the program asks for 40 ms of reported elapsed time (ANIM-001) and the host that produced every
   oracle recording delivered 46.875 ms (ANIM-002, section 9). Choosing 46.875 ms makes our replays comparable with
   those recordings; choosing 40 ms would make the game about 17 % faster than them. The choice must be one
   documented constant, with the nominal 40 ms recorded beside it and the host-dependence stated.
2. **Treat the looping animation element (native 50) as never completing**, as the original does, and raise a
   diagnostic when a mission's sequence level blocks on one, rather than completing it helpfully.
3. **Clamp and log instead of reading out of range** wherever the original is unchecked: the frame states of
   ANIM-042, the cart's waypoint index, and the camera's leftover scroll speed.
4. **Keep 16 facings everywhere**; the current engine's 8-way direction set cannot express the animation blocks or
   the turning rule.
5. **Refuse movement mode 3 with a diagnostic** until a caller is identified (ANIM-202).
6. **Implement edge scrolling with the key scroll's commands and ramp** and record the border width as an assumption
   (ANIM-324).
7. **Drive the cart's sub-sprite animation rate from the speed** and record it as an inference (ANIM-301).
8. **Reproduce difficulty scaling** through `spec-ai-combat.md` AI-045; do not omit it, and do not let it touch
   anything in this specification (ANIM-523).

## 9. Validation against the data and the recordings

Checks run in this session with `harness/tools/probe/anim_actions.py --table` on the read-only copy at
`C:\Users\przem\source\gamedata\robinhood\DATA\Characters`. Only aggregates are quoted; the per-frame tables stay in
the analyst workspace.

- **Bonus items (the decisive per-frame check).** Every bonus-item profile checked (`BONUS_Ale`, `BONUS_Arrows`) has
  five blocks whose frames all carry the hold value 1 and a frame count of 16. Under ANIM-031 each frame is displayed
  for exactly 2 updates, so the cycle is 32 updates and there are 16 frame changes per cycle. The oracle measured a
  **uniform 93.75 ms** between changes and 16 changes per **1.500 s** (`stealth-and-combat.md` 8.4 as corrected by
  `combat-measurements.md`, which reassigns that measurement from "a soldier idle" to a **pickup sparkle** - the
  correction is carried here). `2 x 46.875 = 93.75` and `32 x 46.875 = 1500` exactly. This is a direct per-frame
  confirmation of the `hold + 1` rule and of the reference cadence, not an aggregate one.
- **The hero's crouched walk.** Its holds sum to 18 over 14 frames and its advances sum to 27 px, so ANIM-031 gives a
  cycle of `18 + 14 = 32` updates = 1.500 s and an average of 18.0 px/s; the recording measured 1.50 s and
  17.8 px/s. The competing "one update per hold value, at least one" reading gives 0.84 s and fails.
- **The hero's walk.** 22 frames, every hold 0, every advance 4 px: 22 updates per cycle = 1.031 s and 85.33 px/s.
  The recording gives a **stride period of 1.044 s** and 85.3 px/s. This is a *visual stride period*, not a count of
  logged engine updates, so it supports the cadence to within its own uncertainty (about 1.4 %) and excludes 40 ms
  (which would give 0.88 s); it does not by itself measure the counter granularity.
- Further profiles agree with `docs/formats/sprite-animations.md` rule 3, which derived the same speeds from the same
  files: the hero's run (12 frames of 5 px) 106.67 px/s; `Soldier A00`'s walk (22 of 2) 42.67 px/s, alert walk
  (22 of 3) 64 px/s, alert run (12 of 4) 85.33 px/s, sprint (32 of 5) 106.67 px/s.
- The marker-field candidate of ANIM-012 equals `frame count - 1` on every block checked here, consistent with the
  112 608 of 148 512 animations counted by `sprite-animations.md`; the identification remains unconfirmed.
- The climb blocks are 12 frames with holds summing to 16 and advances of plus or minus 3 px with a per-block
  displacement of 45 px. **The revision-1 acceptance case that quoted "45 versus 36 px" as if both were
  displacements was wrong and is withdrawn**: the 45 px is the block displacement field, the 36 px is the sum of the
  advances of one repetition of the loop, and the two are different quantities. No test asserts a relation between
  them.
- What these measurements do **not** establish: an exact acceleration trace for the camera ramp, the zoom
  transition's duration, the follow controller's behaviour, or any individual displacement event. Those need new
  recordings (section 10).
- No contradiction was found between the code that was read and the shipped animation tables.

## 10. Open questions

Each one names what to read next; each corresponds to a row of section 4.3.

1. **The proximity reaction and the collision resolution** (ANIM-240, ANIM-241): the per-class reaction invoked from
   the scan inside 00561040, and the second half of that routine after the straight-line test fails (00563e90,
   00564020 to 00564390, 00560840). Whether a failed update rolls the position back. This is the same gap as
   `spec-navigation.md` open question 2 and the largest one remaining in movement.
2. **The arrival test's non-plain cases** (ANIM-242): 00560460's branches that consult the direction and the live
   straight-line test, and 004a4a10.
3. **The speech duration** (ANIM-140): the speech case of 00464b20, the sound length source around 005a87f0, and the
   fallback with sound disabled.
4. **Which animation-record fields the marker and the loop length are** (ANIM-012, ANIM-013): 005bdcd0 and 005bddb0
   read the in-memory record; the mapping to the file fields of `docs/formats/sprites.md` needs one pass over the
   sequence reader reached from 005b5ea0.
5. **Which class passes movement mode 3** (ANIM-202): the other callers of 005b86b0 (00475bd0, 00481150) and the
   player-character executor 00470390.
6. **The per-kind admission tests** (ANIM-122): 0046abd0's classification feeds tests reached through 0046b210;
   read those and state the refusal results per kind.
7. **The cart** (ANIM-301, ANIM-303): the branch selector in 004ab7e0, the two unread instructions (004af0f0,
   004af060), the consumer of the per-sub-sprite value, and the program's container in the mission file (004ae470
   reads it).
8. **The cart hit's severity selector** (ANIM-305): the comparison in 004d9420 and the helper it calls, and what the
   hit action does with its two parameters.
9. **The screen-edge scroll** (ANIM-324): which routine turns a mouse position near a border into the four scroll
   commands and what the border width is; start at the HUD construction 0050b640, the input pump around 005105d0 and
   the command dispatch table at 004dcdac.
10. **The zoom transition** (ANIM-326, ANIM-333): at which step of 004cf610 / 004cfce0 the current zoom value
    changes, and therefore how many updates a zoom element takes.
11. **The camera follow** (ANIM-340): the folded part of 004cdfc0 between the lock test and the burst application,
    and 004c8120, which is what makes setting a lock able to move the camera.
12. **The scenery order and the mask integration** (ANIM-363, ANIM-366): how the scenery list is ordered at load
    (004c0510, 004c2720), the comparison the sort reaches through 004d1800, the sort line's endpoint and equality
    results in 004aa980, and how the level's mask data becomes drawable pieces (00523df0).
13. **The footstep selector** (ANIM-207): reconcile the stored value 005b86b0 compares with 5 against the float the
    cart code writes at the same place (004ab7e0).
14. **The level-owned timed list** of ANIM-101.6: what it holds and what "finished" means for its entries.
15. **The two wide resolutions** (ANIM-380): 005e3f70 and the mode enumeration near 005e3b10.
16. **Recordings needed**: a camera key-scroll trace (to pin the ramp entries), a zoom element trace, a follow trace
    for a visible and an off-screen actor, and a cart run to pin the rattle draws' effect on later rolls.

## 11. Differences from the current engine

Against `crates/opensherwood-core/src/anim.rs`, `world.rs`, `crates/opensherwood-render/src/lib.rs`,
`crates/opensherwood-app/src/engine.rs`, `docs/formats/sprite-animations.md` and
`docs/original/stealth-and-combat.md` 8. A different internal organisation is not itself a difference; each item
below is a different observable result.

1. **There is no 64 Hz animation clock.** `anim.rs` runs a 60 Hz world tick with 16 units per tick and 45 per table
   tick to approximate one. The original has one clock - the update - and a frame lasts `hold + 1` of them
   (ANIM-003, ANIM-031). The engine's scheme keeps its remainder, so it does not lose fractional durations, but it
   quantises every frame change to its 60 Hz presentation and cannot express "one frame per update", and its
   `world_ticks` rounding moves individual frame changes by up to one tick against the original's exact counting.
2. **Movement is not a constant average speed.** `AnimSet::cycle_speed` moves the entity at the cycle's average;
   the original displaces the current frame's own advance on the updates where the timer is zero after stepping and
   nothing on the others (ANIM-032). The two agree for the uniform walk and run cycles and disagree for the sneak,
   the climbs, the decelerating stops and every action with non-zero holds - visibly, as stepping motion, and
   observably, as different arrival updates.
3. **Eight directions instead of sixteen** (`direction_of`, the per-action `[u32; 8]` sets). The original indexes a
   block of 16 by the facing (ANIM-010) and turns one sixteenth per call (ANIM-210); with eight directions neither
   the sprite choice nor the turning timing can be right.
4. **Turning has a rule and a cost**: the shorter arc, ties counter-clockwise, four variants including a damped one,
   the 0.6 factor with the 0.7 px floor, and no restart of the clip (ANIM-203, ANIM-210 to ANIM-215). The engine
   turns instantly.
5. **Animation elements complete on real conditions**: 49 at the end of the clip, 50 **never**, 51 at the end plus a
   permanent freeze element (3.4). The engine completes all three at once, so missions that wait on them behave
   differently.
6. **The camera's scroll step is a ramp, not a constant.** `world.rs` scrolls by a constant 8 px per key update; the
   original starts at 0 (ramp entry 0), rises to 32 px per update, coasts back down, divides the step by the zoom
   and clamps to `map - (screen / zoom)` with the world view **80 px shorter** than the screen (ANIM-320, ANIM-323,
   ANIM-380). `world.rs` clamps against its own configurable viewport, which is the right shape but the wrong
   rectangle: the 80 px HUD strip is missing.
7. **Natives 18 and 19 start a scroll and set its step length; native 20 is the jump** (ANIM-331, ANIM-332). The
   engine's "deployment area" reading and the zoom reading are both wrong, and nothing waits for an 18/19 scroll.
8. **Zoom has three values, is requested rather than set, and is rendered over eight steps** (ANIM-325, ANIM-326);
   the zoom element completes on a later update (ANIM-333). The engine treats zoom as instant.
9. **The camera lock matches the followed actor's own speed in bursts and can move the camera when it is set**
   (ANIM-340); the engine has no equivalent.
10. **Occlusion is ordering.** `opensherwood-render` hides characters by clipping them against occluder masks with a
    depth line. The original produces one merged list per frame: movables sorted by **world row** with an identity
    tie-break, merged into the level's scenery order by a per-scenery sort line (ANIM-363, ANIM-364). The engine's
    depth line is the right idea in the wrong place, and its key must become the world row, not the screen row.
11. **Effects and decorations are ordered into the same list** through a deferred queue with 0.001 / 0.01 / 1000.1
    offsets (ANIM-364, ANIM-365); the engine draws them in separate layers.
12. **Animated destination and trail marks are missing.** The engine already draws target lines and selection
    markers; what it does not have is the original's **animated** ground marks with their 6-frame life, their
    every-second-frame ageing while visible, and their unbounded count (ANIM-370).
13. **No dirty rectangles, 688 px of world, no letterboxing** (ANIM-361, ANIM-380), and a zoom transition that does
    not run the ordinary pass list at all.
14. **The per-element update order is the element table's index order with the count re-read each step, and a removal
    skips the element that shifts into place** (ANIM-102); the engine iterates a stable snapshot.
15. **Carts are speed-driven with a scripted program, a weighted random branch and a per-sub-sprite random rattle**
    (3.10), and they injure people through a pass that runs **before** the element updates (ANIM-305). The engine has
    neither.
16. **Difficulty exists and must be honoured** (ANIM-523, deferring to `spec-ai-combat.md` AI-045). Slow motion
    selects a different pacing minimum whose realised ratio is host-dependent and is not exactly 10 (ANIM-520).

## 12. Amendments the sibling specifications need

These follow from what is established above and should be applied by the owner of each document, not by this one.

**`spec-script-vm.md`:**
- **VM-100**: distinguish the nominal pacing (40 ms of reported elapsed time, 400 ms in slow motion, no catch-up),
  the measured reference cadence on the recording host (46.875 ms, host-dependent), and OpenSherwood's chosen
  deterministic update rate. The conversions to 60 Hz and "64 Hz" should be dropped: there is no 64 Hz clock.
- **VM-103**: insert the cart/character interaction pass **before** the per-element update pass, and keep the two
  later lists distinct (a level-owned timed list visited last-to-first, then the script timer list in insertion
  order). Note that the tick's early exits can advance the tick counter while skipping the element phases.
- **VM-101/221/222**: unchanged and correct; this document defers to them, and adds that the camera and the drawing
  are **not** suspended when the tick is (ANIM-004, ANIM-342).
- **VM-218/219**: for kind 6 state that completion happens on a later camera update (or at a border clip), for kind
  7 that it is immediate, and for kind 8 that completion compares the requested and current zoom and is therefore
  not tied to the visual transition's end. Distinguish the native's return, the logical zoom, the displayed scale
  and the element's completion.
- **Native rows 18, 19, 20, 21, 33, 35, 42**: replace with ANIM-331, ANIM-332, ANIM-325 and the section 6 rows. In
  particular 18/19 are **scroll requests with a step length**, not jumps and not zooms; 20 is the jump; 21 sets a
  *requested* zoom.
- **VM-231 and native rows 49, 50, 51**: replace the "durations unknown" entry with the human-target rules of 3.4,
  keeping ANIM-123's restriction (non-human targets unread) and ANIM-035's startup timing.
- **VM-216**: keep as an assumption; ANIM-122 could not settle the admission tests.

**`spec-ai-combat.md`:**
- **AI-001**: 400 ms is the slow-motion selection, not an inactive-window rule; qualify the realised 46.875 ms as
  host-dependent (ANIM-002) and drop any 64 Hz clock.
- **AI-004**: the staggered phase compares the frame counter's low **six** bits with the element identity's low
  **five** bits, and the step also requires that the element has no adversary and did not move (ANIM-103).
- **AI-005/AI-006**: add the cart rattle's two draws per sub-sprite per update and the cart program's branch draw to
  any statement about trace-equivalent ordering (section 8).
- **AI-045**: stands; this document defers to it. ANIM-523 adds only the scaling helper's shape (two caller-supplied
  factors and a cap; level 0 takes the first, level 2 the second, level 1 unchanged, truncated).
- **AI-150/AI-170**: keep the unresolved swing-cadence and energy-recovery discrepancies; the clock established here
  does not resolve them.
- **AI-190**: `ActionChange` takes the current and previous action ids, with the actor dynamically scoped, as
  `spec-script-vm.md` VM-091/107 already say.

**`spec-navigation.md`:**
- **NAV-150**: the advance is applied on the updates where the animation timer is zero after stepping, not once per
  frame unconditionally; the turning factors and the floor are ANIM-203; the movement modes per action are ANIM-131.
- **NAV-151**: the proximity trigger is ANIM-240 (same layer and same projection area, 5 px for actors) and the
  failure counter is ANIM-241; the reaction and the sliding stay open in both documents.
- **NAV-152**: `radius + 5` is the door-approach element's rule; the general arrival test is ANIM-242 and is not
  settled.
- **NAV-200**: the climb loop's repetition count comes from the animation record field of ANIM-013, whose
  identification is unconfirmed.

## 13. Provenance

- Ghidra project `re/ghidra/robinhood` (never committed); exported decompilation `re/out/decomp_all/<address>.c`,
  function inventory `re/out/inventory.tsv`, string references `re/out/strings.tsv`, module map
  `re/notes/modules.txt`, all produced by the committed export scripts under `scripts/ghidra/` and all git-ignored.
  Data bytes read with `scripts/ghidra/peek.py`. The displacement gate (ANIM-032), the mode-3 arithmetic
  (ANIM-202), the destination of every write natives 18, 19 and 20 perform (ANIM-331, ANIM-332) and the scroll-ramp
  reversal (ANIM-323) were confirmed against the raw instructions with a local capstone disassembly helper kept in
  the analyst workspace.
- Functions read: section 0. Notes: `re/notes/anim/` (git-ignored).
- Data checked: `harness/tools/probe/anim_actions.py --table` on the character profiles of the read-only game copy
  at `C:\Users\przem\source\gamedata\robinhood` (section 9). No oracle run in this session; the timing comparisons
  use the recordings of 2026-09-05 as reported in `docs/original/stealth-and-combat.md` 8 **with** the correction in
  `docs/original/combat-measurements.md`.
- Siblings cross-referenced: `docs/original/spec-script-vm.md`, `spec-navigation.md`, `spec-ai-combat.md`; formats
  `docs/formats/sprites.md`, `sprite-animations.md`.
- Review history: revision 1 (blob `17ff4e2c6e84326e82552e0afc0c2ef8fc3082b3`) was reviewed by Codex `gpt-6-astra`
  as spec review 18 (28 findings, verdict *redo*). This revision 2 answers all 28; the disputed points and their
  reasons are in section 4.3 and in the report that accompanies this revision.
- Tests that will depend on this document: the animation and movement rebuild, the camera, the draw order and the
  acceptance cases of section 7 (roadmap item "movement and camera", ADR-0009).
