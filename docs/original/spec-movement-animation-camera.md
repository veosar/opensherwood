# Movement, animation and camera (behaviour specification)

Status: `draft`, **revision 6** (answers Codex re-review 44 finding by finding and resolves five of the
reviewer's outstanding boundaries; awaiting re-review). Build: GOG
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
is high unless stated. Section 4 is the clearance boundary: **4.2 lists what is cleared for implementation and
4.3 what is not**, rule by rule.

**The engine now runs on this clock.** Rebuild batch 1 (commit `ef45066`, ruleset 19) put the script VM
interpreter, the native table and the snapshot contract in place and made **one logic frame of 46.875 ms drive
the whole engine** (`ADR-0010`, and ANIM-001 to ANIM-003 here). The claims below are therefore no longer a
description only: they are the contract the next implementation batch builds from, and a rule this document
leaves in 4.3 is a rule that batch must carry as an `Assumption` rather than invent.

**Pinned siblings.** This revision is written against, and its cross-references point at:
`docs/original/spec-script-vm.md` **revision 4** (commit `afdfaec`), `docs/original/spec-ai-combat.md`
**revision 2** (commit `e966b05`), `docs/original/spec-navigation.md` **revision 2** (commit `e5a2e0c`), and the
engine decision `docs/decisions/ADR-0010-logic-frame.md`. Where one of those documents has already absorbed a
result of this one, section 12 says so instead of repeating the amendment.

**Compatibility tokens.** One identifier is required verbatim because the player's compiled mission scripts name
it: the callback name **`ActionChange`** (section 6). Nothing else in this file is a name taken from the program.

## Identity and exposure

- **Analyst**: agent session `a2f97805ff3217993` (an Opus session), 2026-09-13 to 2026-09-18, analyst role under
  ADR-0009. It has read decompiled code of the animation player, the actor action executors, the movement and
  collision entry points, the cart, the camera, the per-frame drawing and the settings. It must not implement any
  of them, and no implementer session may inherit its context, notes or tool output.
- **Delegated readers**: two subordinate sessions launched by `a2f97805ff3217993` on 2026-09-13, in the same role
  and the same workspace. Recorded separately, with the only handles this session retained:
  - *Delegate A* - task handle `afbf9e86a17f5815f`, brief "camera behaviour from the decompilation", transcript
    archived by the harness at
    `<session scratch>/tasks/afbf9e86a17f5815f.output`; contributed the first pass for 3.12.
  - *Delegate B* - task handle `a90a9ccb054b83d6b`, brief "drawing order and settings from the decompilation",
    transcript archived at `<session scratch>/tasks/a90a9ccb054b83d6b.output`; contributed the first pass for
    3.13 and 3.14.
  **Unresolved provenance**: those handles identify tasks, not sessions, and the harness did not expose a session
  identifier for either delegate; the transcripts live in a session-scoped scratch directory that is not part of
  the repository and may not survive. This is recorded as an open provenance gap rather than resolved by
  attributing their exposure to the parent - their exposure does count as this session's for the purposes of the
  wall, but their identities are not established. Their raw output never entered the repository; every statement
  they contributed was re-read at the cited addresses before it was kept, and several were corrected or withdrawn
  in revisions 2 to 4 (the camera follow threshold, the difficulty conclusion and the camera latch initialisation
  among them).
- **Reviewer**: Codex `gpt-6-astra`, both reviews archived in the repository.
  - **Review 18** (`docs/decisions/reviews/2026-09-13-codex-review-18-spec-movement-animation-camera.md`) reviewed
    **revision 1**, blob `17ff4e2c6e84326e82552e0afc0c2ef8fc3082b3`: 28 findings, verdict *redo*.
  - **Review 24** (`docs/decisions/reviews/2026-09-13-codex-review-24-spec-movement-animation-camera.md`) reviewed
    **revision 2** at commit `b0cd053`, blob `f9a8c498eee7d4359e0159de459f688574841439`: 22 findings, verdict
    *fix-then-clear*, with a list of results cleared as facts.
  - **Review 30** (`docs/decisions/reviews/2026-09-18-codex-review-30-spec-movement-animation-camera.md`) reviewed
    **revision 3** at commit `03b413e`, blob `1b65f0bafaa59f6ac4dd704fe7a0c6e4d841df03`: 14 findings, verdict
    *fix-then-clear*, clearing ANIM-329's ordered conversion, the gated player and NPC operations, the play-only
    status precedence, the existence of both failure-counter resets and ANIM-340's thresholds.
  - **Review 38** (`docs/decisions/reviews/2026-09-18-codex-review-38-spec-movement-animation-camera.md`) reviewed
    **revision 4** at commit `7328461`, blob `06d0fad5b45fe7106119dbf530e45e5338e89b16`: 9 findings, verdict
    *fix-then-clear*, with a per-area clearance table. It cleared the timer arithmetic and reset rules, mode-10
    preservation and mode-14 valid-entry bounds, the displacement, turning and immediate-completion predicate, the
    directional comparison itself with the failure-region rules, the cart integration and traversal, the
    sign-dependent small-map conversion with both cleared latches and the first-refresh reset, and the drawing
    order with the ground-mark ageing invariant.
  - **Review 44** (`docs/decisions/reviews/2026-09-18-codex-review-44-spec-movement-animation-camera.md`) reviewed
    **revision 5**: 8 findings, verdict *fix-then-clear*, accepting the expression filter and the `Assumption`
    hand-off, and judging the document "not yet cleared as an implementable whole" with a list of outstanding
    boundaries.
  This revision 6 answers all 8 findings of review 44 and closes the boundaries listed in 4.4; section 4.3 records
  what remains excluded and why.
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
reassign one of its two measurements to a different object; the script VM specification could not state when any
actor-side sequence element completes, which leaves every scripted sequence unable to advance; the navigation
specification had the movement advance and the turning factors but not the clock they are applied on; the camera
natives were guessed by the current engine. All of that lives only in the executable.

**Scope read** (about 120 functions; the reasons are the questions above):

| Functions | Why |
|---|---|
| 0050f710 (main loop), 004c6ef0 (level tick), 004d9420, 004de170, 004d23d0, 0053a4c0, 0053a0b0, 0052b260 | the frame, the pacing wait, the execution opportunities, the phase order inside a tick, the modal page loop |
| 005b86b0 (play and move), 005b8050 (play), 005b7820 (frame timer), 005b7300, 005b7720, 005b7f60, 005bd450, 005bd560, 005bd5b0, 005bdbf0, 005bdc20, 005bdcd0, 005bdcf0, 005bddb0, 005b5e00, 005b5ed0, 005b6790, 00576c30 | the animation clock, the play modes, the entry points, the displacement gate, the early-completion marker, the freeze gate, the saved state |
| 0055f0f0, 0055f140, 0055f1a0, 0055f210, 0055fc20, 0055fa70, 0055fe10, 0055f290 | facings, the four turn variants, the projection, the saved position state |
| 00464230, 00471b00, 00464b20, 00467a50, 004646e0, 0046abd0, 0046b210, 0046bcf0, 0046bd40, 0046dae0, 0048d510, 00470390, 00475bd0, 00462730, 004645f0, 0048a980 | the actor update, the action executors, the completion codes, the freeze gate on the AI side |
| 00585320, 00585970, 00587160, 005871b0, 005866a0, 00586ed0, 0058a940, 00582560, 0058bb80, 00585b70, 0058ba60 | element states, element parameters, and how a launched sequence reaches an actor |
| 00561040, 00563e90, 00563ea0, 00563ed0, 00560780, 00560460, 005fc660, 005fc700, 00608400 | the collision-aware move, the failure counter and its two resets, the published velocity, the arrival and proximity comparisons |
| 004ab720, 004ab7e0, 004abfe0, 004ac350, 004af540, 004ad5f0, 004ae470 | the cart: speed integration, the three sub-sprite behaviours, the instruction stream, bonds |
| 004c8380, 004cdfc0, 004ca410, 004cec60, 004c7f60, 004c7cf0, 004c7cd0, 004cf610, 004cfce0, 004be6d0, 004dba50, 004dbb70, 004db920, 004dbc90, 004db8e0, 004d90b0, 005758d0, 005a8650, and the natives 00571200, 00571270, 00571330, 005713f0, 00572ab0, 00572ba0, 00572c70, 00572d50, 00577f30, 00577fe0 | the camera: state, the shared location conversion, scroll, zoom, lock, key scrolling, the natives' actual writes |
| 005105d0, 004d0a10, 004d1d00, 004d1800, 00462a30, 004aa980, 005c73e0, 005c2ec0, 0051c0d0, 005e41d0, 005e5260, 0052cf80, 0052d090, 004c0040 | the per-frame drawing order, the comparator and the merge predicate, ground marks, the surfaces and the viewport |
| 00546050, 005460c0, 0055dbb0, 00438600, 00438710, 0055d1a0, 0051ba00, 005b2190, 00409d70, 005a6520 | the settings, and the difficulty value and how it is consumed |

**Stopping condition, stated honestly.** Reading stopped when the interoperability target could be met for the
rules section 4.2 clears **and the remaining gaps had been named**. It is *not* complete: the collision
resolution (sliding, pushing, stepping aside), the arrival test's unread cases, the per-kind admission tests, the
cart's container and two of its instructions, the speech duration, the scenery ordering and the mask integration,
the camera follow and zoom traces and the scroll ramp's exact entries are **not settled** and are listed in 4.3.
A reviewer should treat any rule not in 4.2 as provisional, and an implementer must carry each 4.3 row as an
`Assumption` rather than adopt this document's fallback as fact.

**Analyst authorisation.** On behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

Covered: the clocks and what advances on each execution opportunity; the animation player (action ids, the 16
facings, the frame timer, the play modes, the entry points, the completion signals); what a script's animation,
walk, turn, speech and action elements do on an actor and when they complete; per-frame displacement, turning,
the collision and arrival interfaces; the cart; the camera (state, the location conversion, scroll, zoom, lock,
the natives); the composition of one drawn frame; the settings and the difficulty value's role.

Taken from other subsystems: the level tick's script phases and the sequence machinery (`spec-script-vm.md`), the
path finder, the walk order pipeline, sectors, layers, doors and lifts (`spec-navigation.md`), the AI's choice of
gait and action, the random stream and the difficulty-dependent rules (`spec-ai-combat.md`), the sprite container
and animation table layout (`docs/formats/sprites.md`, `sprite-animations.md`), and the engine's fixed logic frame
(`ADR-0010`).

Handed to them: the completion of every actor-side sequence element, the movement speed per gait, the clock every
timer counts on, and the residual amendments of section 12.

## 2. Data model

### 2.1 Clocks, pacing and units

- **ANIM-001** (observed, 0050f710; high). The main loop performs one iteration per displayed frame. On the path
  that runs while the game window is active it samples the operating system's millisecond counter, and at the
  bottom of the iteration it busy-waits, re-reading that counter, until the **reported** elapsed time since that
  sample is at least **40 ms**, or at least **400 ms** when the slow-motion setting is on (ANIM-520). The
  scheduler never catches up: a frame whose work overruns the wait simply takes longer and **the loop does not
  run extra iterations to compensate**. (That is a statement about the main loop only; within one iteration a
  single actor can receive more than one play call and a single play call can step the animation timer twice -
  ANIM-035, ANIM-202, ANIM-203.)
- **ANIM-002** (observed for the mechanism, 0050f710; **inferred and host-dependent** for any duration; medium).
  The reference sample is taken on the active-window path, so the threshold bounds that path's interval and not
  exactly the whole iteration. Because the comparison is against a *reported* value, the realised frame length is
  quantised by the host counter's granularity and also depends on the counter's phase, on how long the frame's
  work took and on scheduling. **On the host the oracle recordings were made, the realised cadence is
  46.875 ms** (21.333 frames per second), which three independent measurements reproduce (section 9). That value
  is a well-supported reference cadence **for that host**; it is not a consequence of the code, the recordings do
  not measure the counter's granularity directly, and no claim is made about other hosts or about what cadence the
  game's authors targeted. The engine's own fixed choice is `ADR-0010` (section 8).
- **ANIM-003** (inferred, 0050f710 + 005b7820 + the data of section 9; high). **The animation clock is the timer
  step.** A play call performs **one** step in the ordinary case, and the exceptions in both directions must be
  reproduced:
  - *more than one*: movement mode 3 performs two steps in one call (ANIM-202); movement mode 6 adds one more, but
    **only when the facing differs from the target facing and a non-zero displacement has already been produced**
    (ANIM-203);
  - *none at all*: the admitting call of the **play-only** entry (ANIM-035); any call while the **freeze-all** flag
    is set (ANIM-005); any call for an action the profile does not have (status 4); and the **exact-equality
    immediate completion** of the play-and-move entry, which answers 3 without stepping (ANIM-035).
  An actor update performs one play call for its current action, **except** for the five action ids of ANIM-212:
  294 makes exactly two calls, and 296 to 299 make two unless the first answers status 3.
  An animation frame is displayed for `hold + 1` steps **only while the clip is advancing by the hold values**.
  That is the ordinary case of modes 0, 1, 3 to 9, 13 and 14, and it has exceptions of state as well as of mode:
  mode 2 ignores the hold values and advances on every step; modes 11 and 12 force an index on every step and mode
  10 does not step at all (ANIM-036); modes 4 and 5 hold their **first** frame for as long as their stochastic
  wait lasts (ANIM-040); mode 7 holds its **marker** frame for as long as its rotation counter runs (ANIM-043);
  and modes 8, 9 and 14 hold their **stopping** frame indefinitely once they reach it. In the ordinary case - one
  call per update, one step per call, a hold-honouring mode, a frame that is not one of those held frames - a
  frame lasts `hold + 1` updates, which is what the shipped timings and the
  recordings agree on (section 9), and every quoted duration in this document assumes that case. The "table tick"
  of `docs/formats/sprite-animations.md` rule 2 is one step. The program keeps **no** 64 Hz clock and no sub-frame
  accumulator; the "three clocks of about 64 Hz" of `stealth-and-combat.md` 8 is that document measuring the
  pacing of ANIM-002.
- **ANIM-004** (observed, 0050f710, 004c6ef0, 005105d0, 004c8380, 0053a4c0; high). The parts of this subsystem do
  **not** all advance together. Per main-loop iteration, independently gated:
  - *the level tick is attempted* and is skipped when the pause flag is set, when the level's modal-window object
    reports one open, or when the game state is one of the two that leave the level (`spec-script-vm.md` VM-101
    is the authority). The tick's own early exits (a set win or loss flag, a level-ending transition) can advance
    the tick counter while skipping the element phases.
  - *actor updates, the cart interaction pass, the sequence queue drain and the timer lists* advance only inside
    an executed tick.
  - *the camera* advances inside the drawing, on every iteration that reaches the drawing - **including
    iterations whose tick was skipped** (`spec-script-vm.md` VM-219 states the same). It is **not** advanced
    while a modal page or dialogue is open (ANIM-342).
  - *the drawing* is skipped while the freeze flag is set (ANIM-521); the level's blocking flag does **not**
    suppress the drawing as a whole - the camera work runs first and only the later part of the frame is skipped
    on 31 of every 32 level frames (ANIM-321).
  - *the animation player and the movement* have their own gate: while the level's **freeze-all** flag is set
    (ANIM-005) every play call returns without stepping the timer and without moving, although the actor updates
    themselves still run.
  - modal pages and dialogues are the case `spec-script-vm.md` VM-218/222 describes: the native or element that
    shows one runs its **own synchronous loop**, which redraws the page and does not advance the mission camera,
    the tick, or anything else in this document.
- **ANIM-005** (observed, 00576c30, 00577df0, 004ca410, 005b7820, 005b8050, 005b86b0, 005b7f60, 0048a980,
  004ab720; high). The level's **freeze-all** flag gates a specific, limited set of operations, and the claim must
  not be widened:
  - *gated*: every entry point of the animation player returns at once - no timer step, no player-side
    displacement, no completion signal, and the play-and-move entries answer "running"; and the NPC update skips
    its AI step. Consequently **no animation-driven element can complete while the flag is set**, and a sequence
    level waiting on one stalls until it is cleared.
  - *not gated*: the cart's own displacement and its speed integration, which keep running (the cart's
    **sub-sprite** animations do stop, because they go through a player entry point); the turn operations the
    action executors perform; the actor update itself, the tick phases, the camera and the drawing.
  - *who writes it*: native 139 writes it **immediately** within the native call. The recorded form (native 226,
    element kinds 0x11 and 0x12) writes the boolean its element carries, but **marks the element completed first
    and writes the flag afterwards**, so the sequence level can advance before the flag takes effect. Nothing here
    implies the same ordering for other recorded elements.
  The flag is part of the level's saved state.
- Units: positions, displacements and camera coordinates are **background pixels** ("map pixels") in the screen
  projection of `spec-navigation.md` NAV-001/002; a character's screen row is its world row minus its height.
  Run-time positions are IEEE single floats; the animation table's per-frame advance is a signed 16-bit integer
  number of pixels; facings are integers 0..15. "Update" below means one execution of the thing being described;
  "frame" means one main-loop iteration.

### 2.2 The animation table as the player uses it

The file layout is `docs/formats/sprites.md` and `sprite-animations.md`; this is what the player reads from it.

- **ANIM-010** (observed, 005b5e00, 005b8050, 005b86b0; high). An element plays an **action id**, never an
  animation index. The sequence provides a lookup from action id to the index of the first animation of a **block
  of 16**; an absent action is marked in that lookup, and a play call for an absent action is reported and fails
  without changing any state (status 4, ANIM-120). The animation played is `block index + facing`, with the
  facing 0..15 as `sprite-animations.md` "Direction order" (0 = screen-up, clockwise). Two such lookups exist per
  element (the normal one and a replacement one, ANIM-014); a flag chooses which is current.
- **ANIM-011** (observed, 005bdbf0, 005bdc20, 005bdcf0; high). Per animation the player needs: a per-frame
  **hold** value (unsigned 16-bit, the low half of the frame's timing word), a per-frame **advance** value
  (signed 16-bit, the high half of the same word), the **frame count** (the player derives it from the length of
  the per-frame hold data and answers 0 when that data is absent), a 16-bit **marker frame index** (ANIM-012) and
  a 16-bit **loop length** (ANIM-013).
- **ANIM-012** (observed that a marker field is read and how it is used, 005bdcd0, 005b7300, 005b7720, 005b7820;
  **unknown** which file field it is; medium). One 16-bit field per animation is read as a frame index and is used
  three times: play mode 6 starts a clip at it, play mode 7 rotates the facing while sitting on it, and the
  early-completion marker of ANIM-034 is derived from it. In the shipped profiles the value equals `frame count
  - 1` on every block checked here and in 112 608 of 148 512 animations counted by `sprite-animations.md`, which
  is consistent with `Animation::unknown_0x02`, but the identification was not carried through the sprite loader.
- **ANIM-013** (observed, 005bddb0; high that a second 16-bit field is read by action id as a repetition count;
  **unknown** which file field). This is the "length taken from the animation table" of `spec-navigation.md`
  NAV-200 for the ladder and ivy climb loops.
- **ANIM-014** (observed, 005b86b0, 005b8050, 005b5e00; high for (a), medium for (b)). Two script-driven
  redirections act on the action id **before** the block lookup, and their required results are:
  (a) natives 60 / 61 (element kinds 0xAA / 0xAB) select the replacement lookup, in which one action id resolves
  to another, and restore the normal one; the selection is per element and persists until it is restored, so it
  must be saved (ANIM-020).
  (b) an element may carry a list of **animation overrides**, searched by action id with the first match winning;
  a match supplies the id actually played and clears the "not overridden" mark, a miss sets it. **What else an
  override entry carries, what the mark is consumed for, how the list is populated and what its initial contents
  are were not read**: an implementation must treat the override list as empty until that is settled, and must
  record the divergence (4.3).

### 2.3 State that must survive between updates, and its initial values

Stated as the state's **role** and what a reload must reproduce, not as a layout. The routines that write the
saved form are 005b5ed0 (animation) and 0055fe10 (position), which is where the list comes from; the on-disk
order belongs to the save specification.

- **ANIM-020** (observed, 005b5ed0, 005b7300, 005bd5b0; high for the list, medium for the initial values). Per
  element, for animation: which animation is current (equivalently the action id together with the facing), the
  current frame index, the frame timer, the action id the frame timer was last reset for, the identity of the
  action element currently being played, the early-completion marker pair (ANIM-034), which action-id lookup is
  current (ANIM-014a), the depth sort key (ANIM-364), and three further flags whose meanings were not settled.
  **Initial values**: a fresh element starts from the placement of ANIM-221; the frame timer's value after a reset
  is ANIM-030 (`-1`, or `0` for play mode 10), the marker pair is recomputed whenever a new action element starts,
  and the lookup selection starts at the normal lookup. The three unsettled flags have **no established initial
  value** and are listed in 4.3.
- **ANIM-021** (observed, 0055fe10, 0055fa70, 0055fc20, 00563ea0; high for the list, medium for the initial
  values). Per element, for position and movement: the screen position, the world position and height, the
  position at the start of the current update (ANIM-201), the facing, the target facing, the turn delay counter
  and the turn hysteresis counter (ANIM-211), the layer and sector identity, the projection area, the ground kind,
  the movement direction with its height component, the reverse flag (which mirrors the facing by 8,
  `spec-navigation.md` NAV-002), the no-collision flag, the off-map flag, the visibility flags, and the
  failed-move counter with its collision tolerance and its **progress region, including that region's validity
  mark** (ANIM-241). **Initial values**: the
  facing and the target facing come from the placement (ANIM-221); the failed-move counter is 0 and the tolerance
  is its stored default, both re-established whenever a new action element starts or the progress region is left
  (ANIM-241); the **turn delay and hysteresis counters have no established initial value** (4.3), and an
  implementation must start them at 0 and record the assumption. The progress region must be restored with the
  counter, because it is what decides whether the counter resets.
- **ANIM-022** (observed, 00464230, 0046bcf0, 0046bd40; high). Per actor: the queue of action elements with the
  current one and each element's state, the action id last reported to the script (283 = none,
  `spec-script-vm.md` VM-107), the status of the last executor run (ANIM-120), the wait counter (ANIM-133) and the
  finish code of the last action that ended (ANIM-121).
- **ANIM-023** (observed, 004ab720, 004ac350, 005b86b0, 0051c0d0, 004be6d0; high for the list, medium for the
  initial values). Further state the same requirement reaches: per cart, the program position, the remaining
  length of the current block, the current speed, the target speed, the acceleration, the waypoint index and the
  **per-axis rattle accumulators** of ANIM-301 (which are clamped to the range -1 to 1 and carry across updates) -
  **none of which has an established initial value or reset rule**, because the cart's construction and the
  program's container were not read (4.3, `CartInitialState`); per element, the footstep effect phase counter of
  ANIM-207, which starts at **0**; per level, the freeze-all flag (ANIM-005), the camera
  state of ANIM-320 - whose established initial values are zoom 1.0, no scroll destination, the neutral step
  length 1.0, both ramp indices 0 and **both** direction latches clear (the constructor writes both, which is why
  the fresh-level asymmetry of ANIM-323 arises) - and the ground marks with their animation frames.

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
  the body (3.7), plays the animation (3.2) and moves the element (3.6) - and answers a **status** (ANIM-120); (e)
  the status is acted on (ANIM-120, ANIM-121); (f) if the current action id differs from the one last reported to
  the script, `ActionChange(current, previous)` runs with this actor as the current actor and the reported id is
  updated; with no current action the reported id is 283.
- **ANIM-103** (observed, 00471b00; high). Some per-actor bookkeeping is **staggered**: it runs only on updates
  where the low six bits of the level's frame counter equal the low **five** bits of the element's own identity
  (so each element has a phase, the period is 64 updates, and it can only fire in the first half of each 64-frame
  cycle), and only when the element has **no adversary bound** and its position did not change during this update.
  The quantity it recovers, and by how much, belongs to `spec-ai-combat.md` AI-004, which states the same rule. An
  implementation must key the phase on element identity, not on table position.

### 3.2 The animation frame timer

One state machine. State: the animation index, the frame index `f`, the frame timer `t`. `hold(f)` and
`advance(f)` are ANIM-011, `n` the frame count, `m` the marker frame (ANIM-012).

- **ANIM-030** (observed, 005b7300, 005bd5b0; high). The timer is reset when the **action id** changes - not when
  the facing changes, and not when a different action element requests the same action id (ANIM-035). A reset sets
  `t := -1` (as a 16-bit value) and, by play mode: `f := 0` for modes 0-5, 7-9 and 11; `f := m` for mode 6;
  `f := n - 1` for modes 12 and 13; `t := 0, f := 0` for mode 10 alone; and **for mode 14, and for any mode the
  reset does not recognise, nothing at all is written** - such a clip continues from whatever frame and timer the
  element already had. An explicit restart, which some entry points perform instead of the reset, sets
  `f := 0, t := -1` or `f := n - 1, t := -1` whatever the mode.
- **ANIM-031** (observed, 005b7820; high). One step of an ordinary forward mode: `t := t + 1`; if `hold(f) < t`
  (unsigned) then advance `f` (by one, or by two in modes 3 and 14) and `t := 0`; then wrap or clamp `f` as the mode
  says. Consequences: a frame is displayed for `hold + 1` steps **while the clip is advancing** (ANIM-003 lists the
  states in which a frame is held longer); the value `-1` used by a reset makes the first step
  land on `t = 0`, which is the same state a frame is in when it becomes current in the ordinary flow, so the first
  frame of a clip gets its full `hold + 1` steps. Mode 10 resets to `t = 0` and has no step of its own, so its first
  frame is short by one step and the clip never advances.
- **ANIM-032** (observed, 005b86b0; high; **cleared by review 24**). **The displacement gate is `t = 0` after the
  step**, not a comparison of the frame index before and after. Displacement is taken when, and only when, the timer
  is zero after stepping; on every other step it is zero. In an ordinary cycle that coincides with "the frame
  changed", which is the useful way to think about it, but the two differ in three cases that must be reproduced: on
  the first step after a reset (the timer goes from -1 to 0 and displacement **is** taken although the frame did not
  change); on a one-frame clip whose hold is 0 (every step leaves `t = 0`); and in the modes that leave the timer at
  0 without advancing.
- **ANIM-033** (observed, 005b7820; high). The **completion signal** of a looping mode fires on the step after
  which the clip's last frame is current *and* the timer equals that frame's hold value; if that frame's hold value
  is 0, it fires on the step that makes the frame current. It therefore fires on the clip's last displayed step,
  one step before the wrap, once per cycle; the clip keeps looping afterwards.
- **ANIM-034** (observed, 005b7720, 005b8050, 005b86b0; high for the rule, medium for its purpose; depends on
  ANIM-012). When a **new action element** starts, an **early-completion marker** - a (frame, timer) pair - is
  computed from the action's animation and stored. Whenever the live (frame, timer) equals the stored pair, the play
  call answers status **0** rather than "running", subject to the precedence of ANIM-120. The pair is:
  - `(m, 0)` in general;
  - a pair no state can ever reach, when the clip has exactly one frame whose hold value is below 2;
  - when `m = 0`: `(1, 0)` if the first frame's hold value is 0, otherwise `(0, 1)`;
  - when `m` is the last frame index or beyond: `(n - 2, hold(n - 2))` if `m > 1`, otherwise a pair no state can
    reach.
  Because `m` is the last frame index in the shipped data, the ordinary result is "the last displayed step of the
  second-to-last frame", i.e. **one displayed frame before the end of the clip**. That the purpose is blending is an
  inference, not observed. The pair is computed from the action's block without the facing; all 16 animations of a
  block share their frame count and hold values in the shipped data, so this makes no difference there.
- **ANIM-035** (observed, 005b8050, 005b86b0, 005b7300; high). **Entry points differ, and this changes observable
  timing.** Two entry points exist: *play only* (used by actions that do not move) and *play and move* (3.6). What
  each must produce:
  - *Play only, on the update that admits a **new** action element*: the early marker is recomputed; the timer is
    reset (ANIM-030) or explicitly restarted, depending on the caller's restart request; **the timer is not
    stepped**; the call answers **1** unless the preserved state already equals the recomputed marker, in which
    case it answers **0** (ANIM-120's precedence: the marker test runs after the started status is set and
    overrides it on this entry).
  - *Play only, on later updates with the same action element*: one timer step, then the marker test, then the
    completion test, which overrides both (ANIM-120).
  - *Play and move, on the update that admits a new action element*: the same bookkeeping, **and** a timer step and
    a move; the call answers **1**, and on this entry the started status is **preserved against** the marker test.
    Two things can still displace that 1 on the admitting update: (i) an **immediate-completion case**, which is
    narrower than "already arrived" - it requires the movement mode to be **anything but 5** and the element's
    supplied destination to be **exactly equal, component by component, to the element's current screen
    position**; a destination merely within the arrival tolerance does **not** trigger it, and the call answers
    3 **without stepping the timer**; and (ii) ordinary **arrival after the displacement** (ANIM-208), which
    answers 3 on that same update. **Movement mode 5 is excluded from (i)** - the exact-equality case cannot fire
    for it - but it is **not** thereby guaranteed to answer 1 on its admitting update: mode 5 has completion paths
    of its own (ANIM-209) that can answer 3 then or later.
  - *Play and move, on later updates*: one timer step (two in movement mode 3, and one extra while turning in
    movement mode 6), a move, then arrival (ANIM-208) or completion.
  - *Either entry, a **new** action element carrying the **same** action id as the previous one*: the marker is
    recomputed but the frame and the timer are **not** reset - the clip continues where it was - unless the caller
    requests an explicit restart, which resets them.
  - *Either entry, an action the profile does not have*: reported, status **4**, nothing changed.
  The practical consequence: a non-moving action started through the play-only entry runs **one update behind** the
  raw timer trace of ANIM-031, but only when it was admitted as a new element and no restart was requested.
- **ANIM-036** (observed, 005b7820, 005b7300; high). Modes 10 and every code the player does not recognise perform
  no timer step at all, so an element in one of them keeps **whatever frame and timer it already had**; mode 10
  leaves the element on frame 0 only when it was entered **with an action-id change**, whose reset is what writes
  (0, 0) - entered without one it preserves the frame. Mode 11 forces the frame index to 0 on every step and mode
  12 forces it to the last frame on every step; neither advances a clip.

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
| 10 | no step; a reset sets `f = 0, t = 0` | never |
| 11 | `f` is forced to 0 on every step | never |
| 12 | `f` is forced to `n - 1` on every step | never |
| 13 | **backwards** by one, holds respected, wrapping from below 0 to `n - 1` | when `f` reaches 0 and the timer reaches frame 0's hold value |
| 14 | **forward** by two while `f < n - 2`; no reset case of its own (ANIM-030) | never |
| unrecognised | no step | never |

- **ANIM-040** (observed, 005b7820; high). Modes 4 and 5 are the only consumers of the random stream in the
  animation player, and they draw **only** while `f = 0` and `t = 0`. After a reset the timer is -1, so the first
  step does not draw; from the second step on, one value is drawn per step until the clip starts. The waiting time
  is geometric with mean about 250 steps for mode 4 and about 100 for mode 5 (11.7 s and 4.7 s at the reference
  cadence). The draws are from the single global stream of `spec-ai-combat.md` AI-005.
- **ANIM-041** (inferred, 005b7820 and the shipped data; high). There is **no ping-pong mode**: the back-and-forth
  of the idle cycles is in the data, whose frame lists already contain the frames in a there-and-back order, as
  `sprite-animations.md` records. Mode 13 plays backwards and mode 14 forwards by two; neither turns around.
- **ANIM-042** (observed, 005b7820; high for the states, **unknown** for their consequences). Three ways exist to
  leave the frame index outside the clip, and one commonly supposed way does not exist.
  - **Mode 2** leaves it at `n` - one past the last frame - on the step that signals.
  - **Mode 3** steps by two and wraps only on *equality* with `n`, so on an **odd-length** clip it passes `n`
    without wrapping and never wraps again.
  - **Mode 7**, when its marker is the clip's last frame, ends its rotation by setting the frame to `marker + 1`
    and **returning without the wrap test**, which is `n`; the *next* ordinary step then reads the hold value of
    that out-of-range frame before the wrap test can bring the index back to 0 (ANIM-043).
  - **Mode 14 cannot leave a valid clip.** Its guard admits a step only while the frame index is below `n - 2`, so
    from any index that is already inside the clip - odd or even - it stays inside, stopping at `n - 2` or
    `n - 1`. Only an entry that is *already* out of range keeps it out of range, which is a different case.
  The mode-7 case has a **demonstrated** consumer: the ordinary step that follows reads the hold value of that
  out-of-range index before the wrap test can bring the index back to 0. What the read yields, and what it does to
  the timing of the following cycle, is unknown (4.3). For the mode-2 and mode-3 states no consumer was traced. An
  implementation must clamp and log rather than read out of range, and must record the divergence.
- **ANIM-043** (observed, 005b7820, 005b7300; medium). Mode 7's behaviour depends on the marker and on the entry
  state. Its rotation counter is the frame timer. Entered on the marker frame **with the timer at 0** - which is
  what happens when the clip plays forward to a marker that is not 0 - it performs **8** rotations of -2, a full
  turn, and then moves to `m + 1`. Entered on the marker frame **with the timer at -1**, which happens only when
  the marker is **0**, since a reset sets `f = 0, t = -1`, it performs **9** rotations and ends two steps short of
  a full turn. With the marker at the last frame the rotation happens at the end of the clip and the move to
  `m + 1`. In both cases the rotation-ending step writes `m + 1` and **returns without testing the wrap**, so with
  a terminal marker the element is left one past the end until the following step (ANIM-042).

### 3.4 Actions, statuses and completion

An actor's queue holds **action elements**, each with an action id, an element kind and typed parameters; the script
creates them through the sequence natives (`spec-script-vm.md` VM-230), the walk pipeline through
`spec-navigation.md` NAV-130, the AI through its own decisions. The executor dispatches on the action id and
answers a status. This section supplies the actor-side completion rules the script VM specification defers to it
(VM-231).

- **ANIM-120** (observed, 005b8050, 005b86b0, 00464230, 00467a50, 00585320; high where stated). The statuses, their
  **precedence** and their required effects:
  - **0** - the early-completion marker was reached (ANIM-034).
  - **1** - the animation was started or restarted on this update (ANIM-035).
  - **2** - running.
  - **3** - the clip completed (ANIM-033), or the move arrived after this update's displacement (ANIM-208), or
    the narrow immediate-completion case of ANIM-035 applied, or one of movement mode 5's own completion paths
    (ANIM-209) answered. Any of them can occur on the **admitting** update and then override the started status.
  - **4** - the requested animation is absent from the profile, or the move failed persistently (ANIM-241).
  Precedence differs by entry point and must be reproduced: on the **play-only** entry the marker test runs after
  the started status and **overrides** it, and the completion test runs last and overrides both; on the
  **play-and-move** entry the started status is **preserved against** the marker test, while completion and
  arrival are decided on their own paths and take precedence over "running".
  What the actor does: **3** makes the next queued action element current, and if there is none the finished
  element's **sequence** is told the element is **done** (state 0), which is what lets the script's sequence level
  advance (`spec-script-vm.md` VM-211). **4** sets the element's state to **refused** (5), which triggers the abort
  cascade (VM-217); this is the only refusal path established here. **0** marks the current action element as
  having reached its end, and a few actions treat that as their completion (ANIM-132). Element states are: 0
  finished, 2 running, 5 refused, 6 cancelled; 5 and 6 both abort the rest of the sequence, and a transition into 0
  is only honoured from a running or pending state.
  **Not established**: that any status other than 4 produces a refusal; that selecting the next queued element also
  runs its executor within the same update; and the ordering of the `ActionChange` callback (ANIM-102(f), after the
  executor) against the sequence notification (inside the executor's status handling) beyond the order ANIM-102
  fixes.
- **ANIM-121** (observed, 0046bcf0; high). Independently of the status, an executor may report a small integer
  **finish code** to the actor's own class and store it; the AI and the player-character code read it to know how
  the last action ended. The codes and their meanings belong to `spec-ai-combat.md`.
- **ANIM-122** (observed that a per-kind classification gates admission, 0046abd0, 0046b210; **the results are
  unknown**; low). Before an action element is admitted, the actor classifies it by kind and applies a different
  set of admission tests accordingly; the observable outcomes are "accepted, becomes the current action" and
  "refused, element state 5". Which tests apply to which kind, and which actor states refuse which kinds, was not
  read: `spec-script-vm.md` VM-216 remains the statement of that interface, and it is an assumption there too.
- **ANIM-123** (observed, 00464b20, 0048d510; medium). The natives that queue an animation accept targets other
  than humans (natives 49 and 51 accept actor-family elements and target objects; 50 accepts any known element).
  Only the **human** executor was read. The completion rules below are established for humans and are
  **provisional for objects and animals**.

**Completion of the script's element kinds**, for a human target:

| Kind (native) | What the actor does | Completes when |
|---|---|---|
| 0x14 (45, 212, 46, 47, 64), 0x18 | the walk pipeline of `spec-navigation.md` NAV-130/140: internal move actions along the path | when the last move action reports arrival (ANIM-208, ANIM-242); a path failure or a persistent blockage refuses the element (status 4) |
| 2 (internal, the door approach test) | nothing: it is a test | at once, and it is a two-way branch: **done** when the character is within the element's tolerance of its point by the Chebyshev rule of ANIM-242, or when the element names a sector and the character is in it; **cancelled** (state 6, aborting the rest of the sequence) otherwise |
| 0x1A (48, and 59 code 1) | sets the target facing and turns (3.7) | when the facing equals the target facing |
| 0xA4 (49, once) | plays the action id in **play mode 0** | at the end of the first cycle (ANIM-033); admitted through the play-only entry, so one update later than the raw timer trace (ANIM-035) |
| 0xA5 (50, loop) | plays the action id in **play mode 1** | **never by itself**: mode 1 emits no completion signal, so the element stays running until it is refused, cancelled by the abort cascade, or replaced by another action element on the actor. A sequence level containing one never completes on its own |
| 0xA6 (51, freeze at the end) | plays in **play mode 0**; on the completion signal it **launches** a new one-element sequence of kind 0xA7 on the same actor and then answers status 3 | the 0xA6 element completes at the end of the cycle. The freeze element is **launched**, not admitted: kind 0xA7 is not one of the kinds a launch dispatches immediately, so it enters the sequence manager's **deferred queue** and reaches the actor on the next queue drain (`spec-script-vm.md` VM-212, VM-215). The original sequence's next level therefore **can start before the freeze element is admitted**, and the freeze element is itself subject to admission, refusal and replacement. Once admitted it plays the same action in **play mode 12** and its executor answers "running" unconditionally, so it never completes; the actor holds the clip's last frame only while that element remains uncontested |
| 0xAA / 0xAB (60, 61) | switch or restore the action-id lookup | at once (VM-231) |
| 0x92 (62, 69, speak) | starts the line and its animation | when the line ends (ANIM-140: **unknown** in detail) |
| 0x7F / 0x80 / 0x81 (52, 53, 243) | lock or unlock the AI, clear the highlight | at once (VM-231) |
| 0x15 (57, 70, 71, seek) | a walk that re-targets a moving target; the attached sub-sequence of natives 70/71 is launched when it ends | on arrival; **at once** when the seeking actor is the target itself |
| 0x65 / 0x66 (63, 65) | pick up or put down a carried body | at the end of the animation |
| 0x13 (internal, door), 0xA9, 0xAC-0xAE (lifts) | the action lists of `spec-navigation.md` NAV-171/200 | when the last action of the list ends |
| 0xA0 (internal, wait) | counts down (ANIM-133) | on the check **after** the counter has reached 0 |
| 0x2B (internal, cart hit) | applies the hit of ANIM-305 | at once |
| 0x26 (102) and the posture, strike and reaction kinds of native 59 | the actions of `spec-ai-combat.md` | each plays its clip and completes at its end, except the ones whose executor answers "running" permanently (the guards and the idles) |

- **ANIM-130** (observed, 00464b20, 0054e730; high). The **idle chain**, with its timing corrected: the idle action
  plays in mode 0; when a play call answers the completion signal (status 3) **and the actor's current action
  element is not a wait element**, one value is drawn from the global random stream, and when it is a multiple of
  10 the **action element's action id is changed** to the *fidget* id and the element's identity is refreshed. The
  call then answers "running" - **the replacement clip is not played in that update**; it starts on the actor's
  **next** update, which is when the action-id change is noticed and the timer is reset (ANIM-030). The fidget
  plays in mode 0 and, on its completion signal, sets the action id back to idle the same way, again taking effect
  on the following update. The roll is therefore **not** made on every idle cycle: a cycle that completes while the
  current element is a wait element draws nothing. This is the only random draw in the action dispatcher.
- **ANIM-131** (observed, 00464b20; high). The locomotion actions all use **play mode 0**. Their **movement modes**
  differ and must be reproduced per action: the walk-start uses mode **5** with the factor 1.0 passed literally, the
  walk and the run use mode **1**, the sprint uses mode **2**, and the turn and posture actions use mode **0**. For
  the walk, run and sprint the factor is obtained from the current action element (ANIM-200). All of them pass the
  *next* queued action element to the play call so that it can look one record ahead.
- **ANIM-132** (observed, 0048d510, 00464b20; high). Which status an action treats as its completion is
  per-action: most use 3, some use **0** (the early marker; the search action is one), and at least one uses
  **1**, which makes it complete on the update it starts. An implementation must therefore keep the early marker
  (ANIM-034) and the started status, not only the end-of-clip signal.
- **ANIM-133** (observed, 00464230; high; **cleared by review 24**). The wait action's counter is tested for zero
  **before** it is decremented: a non-zero counter is decremented and the action stays running; a zero counter
  completes the action. A wait set to `k` therefore takes `k` decrements and completes on the `(k + 1)`-th check -
  for `k = 3`: three decrements on the first three checks and completion on the fourth. The waits the walk pipeline
  inserts (`spec-navigation.md` NAV-130: 50, and two values drawn from the random stream) are set in these units.
- **ANIM-140** (**unknown**, 00464b20 speech case; low). The speech element is held while its line plays and ends
  when the line ends. Neither the source of the duration (the sound's length, a value in the text data, or a fixed
  fallback) nor the behaviour with sound disabled was read.

### 3.5 Action-id transitions

- **ANIM-134** (observed, 00464b20, 0048d510; high for what is stated, **partial** in coverage). Two kinds of
  action change exist and an implementation must keep them apart.
  - **In place**: an executor may change the action id **of the element it is already running**, without
    completing it and without consuming a queued successor; the change takes effect on the actor's **next** update
    (ANIM-130). The idle chain is the established pair - idle to fidget on a selected roll, fidget back to idle on
    its completion - and it is **not the only such case**: other executor branches substitute an action id in
    place as well (one substitutes a fallback id when the requested action is missing from the profile). This
    document does not enumerate them; only the idle pair is cleared.
  - **By succession**: an action element completes (ANIM-120 status 3), is popped with a finish code (ANIM-121),
    and the **next queued action element** supplies the next action id. Walk-start to walk, walk to walk-stop,
    run-start to run, sprint-start to sprint, the alert twins and crouch down to sneak and back all work this way.
  The required result common to both: a clip whose element is still current and whose action id nothing has changed
  keeps playing its own id - a walk that reaches the end of its cycle does **not** become a walk-stop unless
  something queued one or substituted one in place. Which ids the AI and the click handlers queue is
  `spec-ai-combat.md` and `spec-navigation.md` 3.4; the ids and their visual roles are
  `docs/formats/sprite-animations.md`.

### 3.6 Displacement per update

Only the play-and-move entry point (ANIM-035) moves a character. Its required results, in the order they must be
produced:

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
  for the caller; **cleared by review 24**). **Movement mode 3** steps the timer a second time in the same call and
  accumulates, with `a = advance` from the first step and `b` from the second, each contributing only if the gate of
  ANIM-032 was open on that step: both open -> `d = (2a + b) x 2 x factor`; first only -> `d = 2a x factor`; second
  only -> `d = 2b x factor`; neither -> `d = 0`. The first contribution is thus doubled twice when both steps
  qualify. No caller passing mode 3 was found (the human dispatcher passes 1 for the walk and the run, 2 for the
  sprint, 5 for the walk-start and 0 for the rest).
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
  `d x direction / (hold(the current frame) + 1)`, i.e. the average velocity over the steps the current animation
  frame will be displayed. This is the value other subsystems ask an element for its speed; the camera lock
  (ANIM-340) uses it.
- **ANIM-207** (observed, 005b86b0; medium for the selector). When the element's **ground kind** is 5 and the
  update's displacement exceeds **2.0** px, a phase counter cycles 0, 1, 2 and on every third such update an effect
  of kind 7 is created at the element's position on its layer. The selector is the ground-kind value the placement
  reader fills from the mission record (ANIM-021); the cart's per-sub-sprite value of ANIM-301 belongs to a
  different object and is **not** in conflict with it (the revision-2 note claiming a conflict is withdrawn). The
  phase counter's initial value was not read (ANIM-023).
- **ANIM-208** (observed, 005b86b0, 00560460; high for the trigger, medium for the test). **Arrival** in every
  movement mode except 5: after the move, if the element moved this update and the arrival test (ANIM-242) answers
  that it has reached its target, the call answers status 3, which completes the action and with it the script's
  walk element.
- **ANIM-209** (observed, 005b86b0, 00560460; **partial**, and excluded as a contract). **Movement mode 5** advances
  through a **sequence of queued action records** rather than a single target. Two completion triggers were read and
  neither is "the records are exhausted":
  - when the arrival test answers for the current record, the element moved this update, and its movement direction
    is non-zero, the mover re-derives the position from the records - and if a **next queued action element exists
    and carries the same action id as the current one**, it answers status **3** at once. The comparison is of
    **action ids**, not of targets or destinations: it ends an element whose successor will go on playing the same
    action, so that the successor takes over without the clip being restarted (ANIM-030 resets on a change of
    action id, and there is none). With no successor, or a successor carrying a different action id, this path
    does not fire;
  - when the animation's **completion signal** was produced by this call's stepping, the mover walks the record
    list, and several of those branches answer **3**.
  Both can fire on the admitting update, so mode 5 does **not** invariably answer 1 there. The walk-start action and
  the cart use this mode. **The full completion contract is excluded** (4.3, `WaypointRecords`): what the records
  are, how they are ordered, how they are re-entered after an interruption and which of the record-walking branches
  answer 3 were not settled, and the geometric rule for "reached the current record" is the same unsettled arrival
  test. An implementation must not assume exhaustion is the only path.

### 3.7 Turning: 16 facings

- **ANIM-210** (observed, 0055f0f0; high; **cleared by review 24**). An element keeps a facing and a target facing,
  both 0..15, 0 = screen-up, increasing clockwise. The ordinary turn step is: `delta = (target - facing) mod 16`; if
  `delta` is 0 the step answers "already facing" and nothing changes; if `delta < 8` the facing increases by 1;
  otherwise it decreases by 1. A difference of exactly 8 therefore turns **counter-clockwise**. One step is 22.5
  degrees.
- **ANIM-211** (observed, 0055f140, 0055f210, 0055f1a0; high for the rules, medium for which actor uses which;
  **cleared by review 24**). Four variants exist and the caller or a per-element flag chooses: (a) the ordinary
  one-step turn; (b) a **two-step** turn that moves 2 of 16 while the remaining difference is more than 1 and makes
  a single one-step correction at the end; (c) a **damped** turn, selected by a per-element flag, which accumulates
  a signed counter and first moves the facing on the **third** call of the same sign starting from zero, keeps
  moving on every later call of that sign, and spends one call resetting the counter to zero when the sign
  reverses; (d) a **delayed** turn that moves one step and then answers "turning" for the next `k` calls, where `k`
  is the caller's argument, so its steps are `k + 1` calls apart.
- **ANIM-212** (observed, 00464b20, 005b86b0, 0055f0f0; high). **Ordering**: within one execution of an action,
  the turn operation happens **before** the animation is played and before the displacement is computed, so the
  facing the displacement of ANIM-203 is scaled against is the facing *after* this execution's turn. **Count**: the
  ordinary action performs **one** turn-and-play execution per actor update. The human dispatcher has exactly
  **five** action ids that execute twice, in two shapes:
  - **294**: **exactly two** executions, each a turn operation followed by a play-and-move call in movement mode 2;
    the second runs **even when the first answers status 3**;
  - **296, 297, 298 and 299**: **at most two**; the loop stops after the first when that call answers status 3.
    These four also, **on the update where the element is freshly started**, take their target facing from the
    sector the element stands on rather than from the movement direction, and clear the freshly-started mark.
  No id executes more than twice. A **turn operation is not a facing change**: it changes nothing when the facing
  already equals the target facing, and the damped variant (ANIM-211c) consumes calls without moving the facing
  until its counter reaches the third same-sign call. So the observable rule is "one or two turn/play executions
  per update, by action id", and the facing moves once per execution **only** with the ordinary turn variant and
  a remaining angular distance of at least one sixteenth.
- **ANIM-213** (observed, 0055fc20; high). The **target facing** of a moving element is derived from its movement
  direction: the direction is converted to world space through the projection plane, quantised to one of 16
  directions, and mirrored (index exclusive-or 8) when the element's reverse flag is set. The refresh happens when
  the element's direction state is recomputed **with the caller's update flag set** and the movement vector's length
  is at least 1.0; a shorter vector leaves the target facing alone, and so does a recomputation without that flag.
  Scripts and the AI set the target facing directly (native 94, element kind 0x1A, the AI's stares).
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
  0..15, which becomes both the facing and the target facing; an object placement additionally names an initial
  action id, which is played once and, when the profile lacks it, is reported and leaves the object without an
  animation. Until an action is played, a character placement leaves the element's current animation in a state
  that carries no meaning; the required result is that the first play call establishes it, and an implementation
  should start from a valid animation of the placed facing.
- **ANIM-222** (observed, native 96 of `spec-script-vm.md`, 005b86b0, 00561040; high). An element taken off the map
  keeps its animation state but is not drawn, does not move, and is not a proximity partner (the scan of ANIM-240
  requires a displayed element).

### 3.9 Collision and arrival

The geometric comparisons are established; the resolution that follows a failed move is not.

- **ANIM-240** (observed, 00561040, 005fc660, 005fc7c0, 0055fc20, 00608400; high for the comparison and the
  guards listed, the reaction **unknown**). Before moving, the mover looks for **partners in its path**. The guards
  it applies, in the order they are decided, are: the candidate is not the mover itself; it is **displayed**; it is
  not the element the mover carries; and it shares the mover's **layer and sector identity** (two comparisons, the
  sector as a number and as the resolved sector - there is **no** projection-area comparison here; revision 4's
  claim of one is withdrawn). The two families then diverge:
  - an **actor-family** candidate additionally passes a set of class and state tests that were read only in part
    and are **not cleared here**, a containment test of its **position** in the mover's query box with **inclusive**
    bounds on both axes, and finally the directional comparison below;
  - a **target-family** candidate is admitted by its own "is solid" answer and the same inclusive containment test,
    and **no directional comparison is applied to it**.
  The directional comparison, which applies to **character candidates only**, is **not a radial distance**: it is
  the **dot product of (partner position - mover position) with the mover's movement direction**, and the partner
  qualifies when that product is **at least 5**.
  The operand is the movement **direction**, which the position code normalises before use, so the comparison is a
  distance measured **along the direction of travel** and is **independent of the mover's speed** (the revision-3
  remark that it depends on speed is withdrawn); it selects partners **ahead** of the mover. A qualifying partner
  has its own **proximity reaction** invoked - the per-class behaviour that makes a character step aside, stop,
  wait or be pushed. **The reaction itself was not read**, including whether it can move either element within this
  update. `spec-navigation.md` NAV-150(b) describes the query geometry in more detail at its current revision; at
  the revision pinned here it does not, so the geometry of the box itself is **not cleared by this document**
  either - only the guard list and the comparison above are.
- **ANIM-241** (observed, 00561040, 00563e90, 00563ea0, 00563ed0, 00607d90, 00608400; high). The move is then
  tested and, on failure, resolved against the geometry of the cells the step crosses. Three pieces of state
  accompany the failures: a **failed-move counter**, a **collision tolerance** float, and a small **progress
  region** that carries a **validity mark** as well as bounds. The progress check runs on each failure and decides
  between two outcomes:
  - the region is **valid and contains the tested point** - containment is tested with **inclusive** bounds on both
    axes - so no progress has been made: the counter is incremented and the tolerance is reduced by **0.2** while
    it is still above 1.0, a shrinking clearance that lets a wedged character squeeze through;
  - the region is **invalid, or the point lies outside it**: a new region is established around the tested point
    with half-extents of about **0.49** px, and the counter is set to 0 and the tolerance restored to its stored
    default.
  **When the check runs**: once per update, **after** the whole deflection sequence of ANIM-244 has finished, on
  the **resulting candidate position** - not once per deflection attempt and not on the element's current position.
  Two earlier points in the same update increment the counter and shrink the tolerance the same way without
  consulting the region: when the candidate position is found not to be free ground, and when the corridor from the
  element's goal is found blocked.
  A **new action element** resets the counter and the tolerance **and invalidates the region**. The first progress
  check after that therefore takes the second branch - it establishes a region and leaves the counter at 0 - so a
  fresh action needs one failure to establish the region and only its *subsequent* in-region failures count. The
  counter measures failures **without progress**, not failures per action and not consecutive updates, and the
  region's bounds and validity mark must be saved with it or the update on which an element is refused changes.
  After the resolution the caller checks the counter and, when it exceeds **50**, reports a failure and answers
  status **4** (ANIM-120), which refuses the action element. Whether the position is rolled back on a failed
  update, and what the resolution does geometrically (sliding along walls and bonds), was not read.
- **ANIM-242** (observed, 00560460, 00467a50 case 2, 005fc660, 005fc700, 004f6c20 (call only), 004a4a10; high for
  the selectors and the comparisons, medium for the tolerance's source). Two different tests exist and must not be
  conflated.
  - The **door-approach element** (kind 2) uses the **Chebyshev distance** - the larger of the two axis
    separations - and answers "done" when it is **strictly less than** `tolerance + 5` px, with 10 px as the
    tolerance the walk pipeline sets, so the effective bound is 15 px. `spec-navigation.md` NAV-150(a) states the
    same rule.
  - The **mover's own arrival test** has **four** cases, selected by two flags and the failure counter. The first
    selector is the element's **blocked latch** (set at the end of an update whose move had to be deflected,
    cleared when a straight move succeeds); the second is an **isometric-metric flag**; in the blocked case a third
    is an **arrival-mode flag**.
    1. *Not blocked, plain metric*: the **dot product of the remaining displacement with the element's direction**
       against the stopping tolerance, arriving when it is **not greater** (inclusive). A projection, not a radius.
    2. *Not blocked, isometric metric*: the same comparison after the remaining displacement's **second component
       is multiplied by 1.7434** - the vertical correction of the 2:1 projection - so the test is a screen-space
       ellipse rather than a plane projection.
    3. *Blocked, arrival-mode flag set*: arrival is decided **entirely by a walkability test** from the element's
       position to its stored goal, using the element's sector and its owning element's world position. Distance
       does not enter.
    4. *Blocked, flag clear, failure counter non-zero* (the wedged case): a **slack** test on the **Chebyshev**
       metric, **strictly** below either `10.0` px or, when the element's order object is present and is not of one
       particular kind, `the target's nominal collision radius + the element's current collision radius + 10.0` px.
       With the counter at zero this case falls back to 1.
    The observable consequence that matters: a character that cannot reach its exact goal because it is wedged
    **still arrives** once it is within about 10 px plus the two radii, and stops cleanly instead of grinding.
  **What sets the stopping tolerance** was not found in the mover, the arrival test or the play call (4.3).
- **ANIM-244** (observed, 00561040, 00560840, 00520ce0, 00520e10, 0051fea0, 0051ff60; high for the shape and the
  ordering, medium for the two solves' exact operands). **Sliding is a deflection, not a retry.** The required
  results:
  - The step is **not shortened, not split per axis, and not retried from a list of candidate directions**. It is
    **rotated, keeping its length**, so that it runs along the blocker. A character walking into a wall at an angle
    therefore slides along it at full speed; it does not stop and does not stutter.
  - Two kinds of blocker take part: **round obstacles** and **wall segments**. Each is given a signed **clearance
    gap** - the separation along the relevant direction minus the element's own current collision radius and the
    blocker's clearance - and any blocker whose gap exceeds its own reach is **discarded**. The remainder is
    **sorted by that gap, nearest first**, in both lists.
  - The lists are then walked as a **merge**, always taking whichever head has the smaller gap, so blockers are
    resolved strictly **nearest first**. A blocker that is found relevant produces a replacement step; the step is
    replaced, a "deflected" mark is set, that blocker is **removed**, the remainder is **re-sorted with the new
    step**, and the walk restarts. Each blocker therefore sees the deflection the previous one caused. This
    ordering is observable and must be reproduced: it is what makes a doorway work as two successive jamb slides.
  - **Round obstacle**: the replacement step is the circle-circle solution - with the step length `r`, the distance
    to the obstacle `d` and the effective avoidance radius `R`, the along-component is `a = (r^2 - R^2 + d^2) / 2d`
    and the across-component is `sqrt(r^2 - a^2)`, combined as `along x direction + across x perpendicular`, the
    side chosen by the sign of the cross product of the step with the direction to the obstacle. It is **accepted
    only when `|a| <= r`**; otherwise that obstacle is skipped entirely.
  - **Wall segment**: the required normal component `n` is derived from the signed distance to the wall and the
    wanted clearance, and the replacement step is `sqrt(r^2 - n^2) x tangent + n x normal`, the tangent's sign
    chosen so that the step keeps moving forward. **Accepted only when `|n| <= r`**; an element already deeper than
    the wall's clearance plus reach gives up on that wall.
  - Two degenerate fallbacks exist and are visible to the player: a wall that cannot be solved (a head-on
    approach) slides sideways at **0.95** of the step length, and a degenerate circle case uses **0.99**.
  - A relevance test precedes each solve (a distance-and-side test for a round obstacle, a signed-distance and
    within-span test for a wall); a blocker that fails it is skipped without being solved.
- **ANIM-245** (observed, 00561040; high). **There is no rollback, because there is nothing to roll back.** The
  element's own position is not modified while the deflection of ANIM-244 runs: the resolution works on a candidate
  and **commits once**, by publishing a translated transform for the element and marking it moved. If no branch
  commits, the element **simply does not move this update** - it is never left partially moved and never left
  inside geometry by this path.
- **ANIM-246** (observed, 004f5890, 00561040; high). **The unstick is the last resort inside the move**, not only a
  step of the walk order (`spec-navigation.md` NAV-111 describes the same routine). It runs when the candidate has
  been found not to be free ground and the step has been backed off - repeatedly multiplied by **0.8** - below
  **0.1** px. Then: the resolution box is inflated by **0.2** px on every side and clamped inside the map; up to
  **50** passes are made; each pass collects the overlapping wall segments and, for every corner of the box whose
  signed distance to a segment exceeds **-0.1**, translates the box along that segment's normal by **its
  penetration plus 1.0** px; a pass that finds no overlap succeeds. On success the box's centre is committed as the
  position. The observable result: a character genuinely inside geometry is shoved straight out perpendicular to
  the offending wall, one wall at a time, and reappears at the nearest clear spot.
- **ANIM-247** (inferred from the absence of any special case, 00561040; medium). **Doors, thresholds and corners
  have no special handling in the move.** They enter as ordinary wall segments of the element's current sector, and
  their passability is decided inside the corridor test, which was not read (4.3). What makes a doorway passable is
  the generic machinery: the tolerance shrink of ANIM-241 narrows the element until it fits, and the nearest-first
  deflection of ANIM-244 handles the two jambs as two successive slides.
- **ANIM-243** (observed, 00561040; high). The bonds crossed by the step are applied after it (`spec-navigation.md`
  NAV-160), which is where the height and the ground kind change.

### 3.10 The cart (the "mobile element")

A cart is speed-driven: its speed produces its motion and drives its sub-sprites, the reverse of a character.

- **ANIM-300** (observed, 004ab720; high). Once per update, if the cart is displayed: when its acceleration is
  non-zero, `speed := speed + acceleration`, and when the speed has reached or passed the target speed (tested with
  the sign of the acceleration) the acceleration is cleared and the speed is set exactly to the target. Then, if the
  speed is non-zero, the cart moves by `speed x direction` px per update, in the same screen space and through the
  same projection plane as a character, and it crosses bonds like one.
- **ANIM-301** (observed, 004ab7e0; high for the three cases, **unknown** for what selects them). Each update the
  cart also visits each of its **sub-sprites** (the wheels and the load, which are separate sprite elements) and
  moves it with the cart. Which of three mutually exclusive treatments each sub-sprite receives is decided by two
  per-cart conditions that were not read; the required results are:
  - **case A** - the sub-sprite's **animation is stepped** once (play mode 0) and nothing else is written to it;
  - **case B** - a value derived from the **reciprocal of the cart's speed** is stored on the sub-sprite (a very
    large value when the speed is zero) and the animation is **not** stepped;
  - **case C** - the same reciprocal value is stored **and** the sub-sprite is given a **random rattle**: two values
    are drawn from the global random stream and turned into an x and a y jitter of amplitude `speed x 0.1`,
    accumulated per axis into state that carries across updates and clamped to the range -1 to 1.
  Cases A and B are alternatives, so the animation-stepping case does **not** write the reciprocal. What consumes
  the stored reciprocal was not read, so "the wheels turn faster when the cart goes faster" is an inference and not
  a required result; case C's two draws per sub-sprite per update are a required result and appear in the RNG
  contract (section 8).
- **ANIM-302** (observed, 004abfe0, 004ac350; high). A cart runs a **program** that comes with the mission: a byte
  stream organised in blocks, each with a length. On each update, while the current block still has length,
  instructions are executed one after another until one of them yields for this update. When a block is exhausted
  the next one is chosen: a byte of the program first selects between two ways of continuing, and **only on one of
  them** is a weighted choice made - a table of alternatives, each carrying a weight and a destination, walked by
  drawing **`rand()` modulo 100, plus 1** from the global stream and subtracting weights until the roll no longer
  exceeds an entry's weight. On the other way the continuation is taken directly, **with no draw**. A block change
  therefore consumes at most one value from the stream, not exactly one.
- **ANIM-303** (observed, 004ac350; high for the effects stated, **unknown** for the rest). Instructions are single
  bytes from 0x80 upwards with their operands. Reading one always advances the program position past its opcode
  byte; **most** instructions then consume their operands and reduce the block's remaining length by their whole
  size (3, 5 or 7 bytes), and the exception matters: one instruction is reported as unsupported and **advances the
  position by its opcode byte alone without reducing the remaining length**, so the length accounting and the
  program position part company from that point on. Observable effects of the rest: **0x80** sets an action id on
  every sub-sprite and restarts each from its first frame; **0x81** sets the current speed and marks the cart
  stopped when that speed is zero, moving otherwise; **0x82** sets a target speed (a target of zero is replaced by
  **0.1**) together with the index of the next waypoint and computes
  `acceleration = (target^2 - speed^2) / (2 x the distance to that waypoint)`, an acceleration that would reach the
  target speed at the waypoint in continuous time - the discrete integration of ANIM-300 only guarantees that the
  speed is **clamped** to the target when it passes it, so the arrival speed is reached at or before the waypoint,
  not exactly at it; **0x83** sets the next waypoint index, resets a per-sub-sprite value and **yields for this
  update**. Two further instructions each pass one operand into the cart and were not read. Any other byte is
  reported and the program stops. An out-of-range waypoint index is reported as fatal when it is read. The
  program's container in the mission file was not read.
- **ANIM-304** (observed, 004af540; high). A cart crosses bonds exactly like a character (`spec-navigation.md`
  NAV-160); duplicated bonds are reported and ignored.
- **ANIM-305** (observed, 004d9420; high for the traversal and the two parameter pairs, **unknown** for the
  condition; **cleared by review 24** for the traversal). Once per level tick, **before** the element updates
  (ANIM-101.2), every unordered pair of level elements is visited - the outer index from the last element down to
  the first, the inner index from the outer index minus one down to 0 - and for each pair in which one element is a
  human and the other is a cart whose disabling flag is clear, two geometric containment tests are applied using
  the **human's position**; when both hold, a one-element sequence is created and launched that applies a hit to the
  human. The hit action receives **two** equal parameters, either **10 and 10** or **50 and 50**; which pair is used
  depends on a geometric or directional comparison on the cart that was not identified, and calling it "the cart is
  moving" is **not supported**. What the hit action does with its two parameters belongs to `spec-ai-combat.md`.
  The traversal order fixes the order in which the hit sequences are queued.

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
  pixels (floats that hold integral values), the current zoom, a requested zoom, a zoom transition step counter and a
  busy flag, a **scroll destination** kept twice (as the caller supplied it and as the conversion of ANIM-329
  produced it) with a sentinel meaning "no scroll", a **scroll step length**, a **scroll ramp index**, a **scroll
  speed** (an unsigned 16-bit value, 0 meaning "use the step length and the ramp"), the **current camera element**
  (the script element that owns the camera, if any), the **lock target** with an enable flag and its per-axis
  correction, velocity and burst counter, the per-update camera delta, two per-axis direction latches with their own
  ramp indices, a redraw-mode value and a cached-view validity flag. The visible rectangle is
  `screen width / zoom` by `(screen height - 80) / zoom`, the 80 px being the strip the HUD occupies (ANIM-380).
  The corner is bounded per axis, but **not by an unconditional clamp to the interval from 0 to
  `map size - rectangle size`**: the lower and upper bounds are applied by the separate, conditional steps of
  ANIM-329 (for a script point) and by the step clipping of ANIM-322 (for a per-update delta), and on a map smaller
  than the view the upper-bound step can produce a **negative** corner without the fallback firing. All the camera
  publishes to the drawing is the corner and the zoom.
- **ANIM-329** (observed, 005758d0, confirmed against the raw instructions; high). **The shared location
  conversion.** Every script-supplied camera point - natives 18, 19 and 20 and the scroll and jump elements - passes
  through one conversion from a **world point** to a **camera corner**. Its required results, in this order:
  1. **Centring**: `corner = point - (screen size / (2 x zoom))` per axis, using the **configured screen width and
     height** - the full height, not the height reduced by the HUD strip - so the point ends up at the centre of the
     screen rectangle and therefore slightly below the centre of the visible world strip.
  2. **Truncation** of both components toward zero to whole pixels.
  3. **Lower bound**: a negative component becomes 0.
  4. **Upper bound**, per axis: when `map size < view size + component`, where the view size is
     `screen width / zoom` on x and `(screen height - 80) / zoom` on y, the component becomes `map size - view
     size`. **Exception**: if that axis's truncated component was **negative**, the conversion instead sets the
     **current zoom to 1.0** and answers the map origin `(0, 0)`. This is the only path on which a camera *point*
     changes the zoom, and it is reachable when the map is smaller than the view at the current zoom. The
     condition is the **sign of the truncated component**, not whether the map is undersized: on an undersized map
     a component that was **zero or positive** takes the ordinary branch and is assigned `map size - view size`,
     which is then **negative**, so the corner ends up negative and no fallback occurs. An implementation must
     reproduce that asymmetry rather than substitute a clamp.
  5. **Alignment**: at zoom 0.5 an odd component is reduced by 1, so both are even.
  Consequences an implementation must reproduce: a caller cannot set the corner directly; "the camera jumps to the
  point" means "the point is centred, truncated, bounded and aligned"; and a jump or scroll near a map edge lands
  at the bound, not at the requested point.
- **ANIM-321** (observed, 004c8380, 005105d0; high). The camera is advanced once per iteration, inside the drawing
  (ANIM-100 step 5), on every iteration that reaches the drawing - **including iterations whose level tick was
  skipped**, but not while a modal page or dialogue is open (ANIM-342). In the level's blocking mode the camera
  routine returns early on updates whose level frame number is not a multiple of 32 - but **only after** the scroll,
  zoom and lock processing of ANIM-330 to ANIM-340 has already run, so that return suppresses the later part of the
  camera and composition work, not the camera's own state advance.
- **ANIM-322** (observed, 004cec60; high). A camera step that would leave the map is **clipped** to the bound, both
  scroll ramp indices are zeroed, and the step reports that it clipped. A **script scroll whose step clips is
  terminated**: its destination is cleared, the step length is reset to 1.0, the cached view is invalidated and the
  camera element is completed (ANIM-330).
- **ANIM-323** (observed, 004be6d0, 004dba50, 004dbb70, 004db920, 004dbc90, 004db8e0; high for the rules, **the ramp
  entries unsettled**). **Scrolling by keys and HUD buttons.** Four commands (up, down, left, right) and two (zoom
  in, out) arrive as engine commands and are dispatched to per-direction handlers. Each handler is **gated**: it
  does nothing while a **script scroll destination is active**, or while a per-direction "already handled" byte is
  set. When it runs it clears the "camera is on the selected character" marker, leaves the actor lock alone, and
  produces a camera delta of `+/- ramp[index] / zoom` on its axis, so the step is constant in *screen* pixels and
  therefore halves at zoom 2.0 and doubles at zoom 0.5.
  Each axis has one **direction latch** and one ramp index, and the latch records which of the two directions was
  last commanded: the negative direction (left, up) sets it, the positive direction (right, down) clears it. A
  handler whose direction **does not match** the latch sets the index to **0** and flips the latch; a handler whose
  direction matches increases the index by one up to **31**. Because **entry 0 of the ramp is 0.0**, the first
  update after *changing* direction produces **no movement**. Starting from rest is **not** symmetric: the level's
  construction clears **both** latches, and a cleared latch is what the positive directions (right, down) match, so
  the first right or down command increments the index to 1 and moves by the first non-zero entry at once, while
  the first left or up command resets the index to 0 and produces no movement on that update. On an update where no
  command arrived for an axis
  the index is **decreased** and the camera keeps moving, so it coasts to a stop.
  **The ramp is generated, and the rule is now settled** (004bef6e to 004bf008, read as instructions). There are
  two tables, one per axis, and they are filled with **the same values** from one working value - one ramp, stored
  twice. Entry 0 of both is **0**. Then, with a **single-precision** working value `w` seeded to **6.0** and stored
  back as single precision at every step, for each of the 31 remaining entries in order:
  1. if the integer part of `w`, taken by **truncation toward zero**, is **odd**, then `w := w + 1` - the
     adjustment is written back into the working value, so it persists into the following entries;
  2. both tables' entry `i` := `w` **rounded to the nearest integer, ties to even**;
  3. if `w < 31.0` then `w := w x 1.05`.
  The two roundings are different operations and using one for both gives the wrong table. The sequence this
  produces is 0, 6, 6, 7, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 26, 28, 31 and then **32** for every
  remaining index: the working value stops growing once it passes 31 and its integer part is even, so the step
  rises from 6 to a ceiling of 32 px per update over twenty entries and stays there, and the index is clamped to
  31. An implementation should generate the table from the rule rather than store it; the rule, not the values, is
  what this specification fixes.
- **ANIM-324** (observed, 004d0400, 00517900, 005186a0; high). There is **no drag panning**: the mouse-drag state
  drives the rubber-band selection rectangle. **Screen-edge scrolling is settled**: once per input poll, immediately
  after the keyboard is read, the pointer's position is compared with the screen's edges and **the same four scroll
  commands the keys raise** are posted, into the same dispatcher, so everything ANIM-323 says about gating, latches,
  the ramp and the zoom division applies unchanged. The details that decide what the player feels:
  - **the border is one pixel**: the left edge fires only at x exactly 0 and the top only at y exactly 0, while the
    right and bottom fire at or beyond the screen dimension minus 1;
  - the four tests are **independent**, so a **corner scrolls diagonally** (two commands in the same poll);
  - there is **no edge detection and no latch**: the commands are re-posted on **every** input poll while the
    pointer rests on the edge, which is what feeds the ramp's acceleration;
  - it is gated twice: a global flag suppresses the whole poll, and a list of **exclusion rectangles** (the HUD's
    own areas) suppresses an individual command when the pointer is inside one - a different gate from the one the
    four handlers apply;
  - the same poll turns the **mouse wheel** into the two zoom commands, one per wheel step.
  Who fills the exclusion-rectangle list was not read (4.3).
- **ANIM-325** (observed, 00571200, 004bef3b, 004c7cf0, 004c7cd0; high). The zoom has exactly three values, **0.5,
  1.0 and 2.0**, held as an index into a three-entry table, and 1.0 at level start. Native 21 accepts only those
  three and otherwise reports an error and changes nothing. **Native 21 and element kind 8 write a *requested*
  zoom**, not the current one; the camera update performs the change. A zoom-out request is refused when the zoom is
  already 0.5 or when the map would be smaller than the visible rectangle at the next step; a zoom-in request is
  refused at 2.0. A fourth way to change the zoom exists and is not a request at all: the conversion of ANIM-329
  can set the current zoom to 1.0 directly.
- **ANIM-326** (observed, 004cf610, 004cfce0, 004c8380; high for the count and the gate, medium for the
  appearance). A zoom change is rendered as a transition over **8** steps, one per update, by blitting the screen
  captured before the change with an interpolated scale; **the two directions use different interpolation
  expressions**, so no single formula describes both. While the transition runs, a busy flag refuses further zoom
  commands and the camera reports itself busy. At zoom 0.5 the camera corner is kept even (ANIM-329). **At which
  step of the transition the current zoom value changes was not established**, which is why ANIM-333's completion
  count is not a settled number.
- **ANIM-327** (observed, 005a8650; high). Zoom scales the whole drawing downstream; it does not select a different
  background, a different resolution or a different layer set, and a change of zoom invalidates the cached backdrop.
- **ANIM-334** (observed, 004cdfc0; high). **Precedence inside one camera update**, which decides what actually
  happens when more than one camera activity is outstanding. The order is: (1) the **actor lock**; (2) the **zoom
  request**; (3) the **scroll destination**. The lock stage is not merely first, it can **end the update**, but not
  unconditionally:
  - the branch that recomputes the lock's correction and does **not** start a burst leaves the camera routine
    immediately;
  - the branch that applies a burst step leaves it immediately **unless** the bounding of that step (ANIM-322)
    both clipped and left a **zero** delta - in that case processing **falls through** to the zoom and scroll
    stages on the same update.
  So an enabled, active actor lock **suspends** a script scroll and a pending zoom for as long as it is producing
  movement, and stops suspending them exactly when its own step is clipped to nothing at a map bound.
  The zoom stage runs before the scroll stage. What it can do to a scroll is narrower than revision 4 said: when a
  zoom request completes it **completes and clears the installed camera element**, which may be the scroll's
  element, but it **leaves the scroll destination set and does not stop the camera from moving toward it**. The
  required consequence is therefore about *completion*, not about motion: a scroll whose element was taken away by
  a zoom keeps scrolling and still terminates on arrival or at a bound (clearing its destination), but **no element
  is completed by that arrival** - the sequence level it belonged to was already advanced by the zoom's completion.
- **ANIM-330** (observed, 004ca410 kind 6, 004cdfc0; high). **Scroll to a point.** Starting the element: any previous
  camera element is **completed** (state 0), this element becomes the camera element, the actor lock is released,
  and then either - when the level's no-presentation flag is set - the point is converted (ANIM-329), the corner is
  set to it, the element is completed at once and the camera element is cleared; or the destination is stored twice
  (the point as given and its conversion), the scroll speed is set from the element's speed parameter, the step
  length to **2.0** and the ramp index to **0**.
  Per camera update, while a destination is set: **first** it is tested whether the destination has been reached; if
  it has, the destination is cleared, the step length is reset to 1.0, the cached view is invalidated, the delta is
  zeroed and the camera element is **completed**. Otherwise the delta is `normalise(destination - corner)` times
  `L`, where `L` is the scroll **speed** when it is non-zero and the step **length** otherwise, shortened to the
  remaining distance when that is smaller, and **truncated per component** to whole pixels. If the bound did not
  clip: when the step length is exactly 1.0 the ramp index is reset to 0, otherwise it increases by one up to 31,
  and the step length becomes the ramp entry at that index (so a default-speed scroll moves 2 px on its first
  update and then follows the ramp, always using the **vertical** ramp whatever the axis). If the bound clipped,
  the scroll terminates as ANIM-322 says.
  **Arrival is tested at the start of a later camera update**, so a scroll that lands exactly on its destination
  completes on the **following** update, not on the one that lands. Because the delta is truncated per component, a
  destination that is not reachable by whole-pixel steps along the line can be approached without ever satisfying
  the equality; a test must choose an axis-aligned destination at an exact multiple of the step to observe the
  clean case.
- **ANIM-331** (observed, 00571270, 00571330, confirmed against the raw instructions; high). **What natives 18 and 19
  really do.** Neither moves the camera and neither is a zoom request. Each requires a non-null location (otherwise
  it reports an error and returns 0) and then writes the camera's **scroll destination**, both the point as given
  and its conversion by ANIM-329, and sets the **scroll step length**: native 18 to **2.0**, native 19 to **its
  float argument**. The camera's own update (ANIM-330) then scrolls there. What an implementation must reproduce:
  (a) they create **no camera element**, but they do **not clear** one either - an element that is already the
  camera element stays, and the arrival of the scroll these natives started will **complete that element**;
  (b) they do **not** set the scroll **speed**, so the scroll uses whatever speed the last scroll **element** left
  there - a non-zero leftover makes the step length, and therefore native 19's argument, irrelevant;
  (c) they do **not** reset the ramp index, so the step length continues from the ramp position the previous scroll
  left; and a native 19 argument of exactly **1.0** makes the **first** ramp refresh set the index to 0, which
  installs ramp entry 0 - a zero step length - for the next update, after which the refreshes advance the index
  normally again (ANIM-330). It shifts the ramp trace by that one reset; it does **not** reset the index on every
  later update;
  (d) they do not touch the actor lock or a pending zoom request - and by ANIM-334 that has two different
  consequences: a lock that is enabled **and producing movement** suspends the scroll's motion, but a lock whose
  own step is clipped to a **zero** delta at a map bound does not (processing falls through to the scroll on that
  update); while a zoom completion never stops the motion, but can **take the camera element away** so that
  nothing is completed when the scroll arrives;
  (e) through ANIM-329 they can, on a map smaller than the view, set the zoom to 1.0.
- **ANIM-332** (observed, 005713f0, 004ca410 kind 7, confirmed against the raw instructions; high). **Native 20 and
  element kind 7 are the jumps.** Native 20 requires a non-null location, converts its point by ANIM-329 and writes
  the result to the camera **corner**, then invalidates the cached view so that the next frame is fully redrawn. It
  touches neither the scroll destination, nor the zoom request, nor the actor lock - though the conversion itself
  can set the zoom to 1.0 on a small map. Element kind 7 does the same and additionally completes any previous
  camera element and releases the actor lock, and it **completes itself in the same call**.
- **ANIM-333** (observed, 004ca410 kind 8, 004cdfc0; high for the rule, **unknown** for the frame count). **Element
  kind 8** stores its float as the requested zoom and becomes the camera element. On each camera update, **first**:
  if the requested zoom equals the current zoom, the request is cleared and the camera element is **completed**.
  Then, if a request is pending and no transition is busy, one zoom-in or zoom-out command is raised toward it; if
  the gate of ANIM-325 refuses, the request is set equal to the current zoom, which makes the element complete on
  the **next** update rather than this one. Three cases an acceptance test must separate: a request that is
  **already equal** to the current zoom completes on the element's first camera check; a request that is **refused**
  completes on the check after the refusal; and a request that is **accepted** completes when the current zoom has
  become equal, whose update count is not settled because ANIM-326 does not say when the current zoom changes.
- **ANIM-340** (observed, 004d90b0, 004c8120, 004c7f60, 004cdfc0, 005bdcb0; high). **Lock on an actor** (element
  kinds 0xD and 0xE, natives 39 and 40).
  **Setting the lock**: a visibility rectangle is built from the camera corner - the view, its top edge raised by
  the 80 px HUD strip - and the actor's position is tested against it **inclusively**. Only when the actor is
  **outside** that rectangle is the camera moved at once, and it is then **centred** on the actor: the corner
  becomes the actor's position minus half the view, **rounded to nearest**, bounded to the map, and forced to even
  coordinates at zoom 0.5. This is a **different** conversion from the one script points take (ANIM-329), which
  does not subtract half the view, truncates instead of rounding and has the zoom-reset fallback. Setting the lock
  also clears both ramp direction indices. Afterwards the camera **remembers the offset** the actor then had -
  `(actor position - corner) x zoom` - and that remembered offset is **never recaptured** while the lock holds: the
  camera's job is to keep the actor at the screen position it had when the lock was taken, not to centre it.
  **Per update**, in order:
  1. the lock is dropped when the target reports itself gone or dead;
  2. if the **burst counter** is non-zero, the recompute is **skipped** entirely and the stored correction is
     applied (step 6);
  3. otherwise the per-axis **correction** is `(actor position - corner) x zoom - remembered offset`;
  4. a **prediction** is formed by adding the actor's own per-update movement to its position and repeating that
     computation - the actor's movement is taken only when its sprite is marked as having moved, otherwise it is
     zero;
  5. the decision:
     - if the prediction's length is **zero** - the actor will land exactly on the remembered offset by itself -
       the correction is scaled by **1/30** and the burst counter is set to **15**: the residue is bled away
       slowly;
     - otherwise the **overshoot guard** zeroes, per axis, a correction whose predicted magnitude is **smaller**
       than the current one (the actor is already closing the gap by itself);
     - the burst counter is set to **15** when either axis's correction magnitude is **at least 1.0** (inclusive);
     - an axis whose correction magnitude is **greater than 1.0** (strict) has its correction **replaced by
       `sign x the actor's own current speed`** - so the camera matches the actor's gait rather than easing. A
       correction of exactly 1.0 arms the burst but is left at 1 px;
     - both corrections are **rounded to nearest**;
     - if the burst counter is still zero the camera does **not** move this update;
  6. applying: the burst counter is decremented, the stored correction becomes this update's camera delta, and an
     axis whose remembered offset and current offset **round to the same whole pixel** has its delta zeroed;
  7. the delta is bounded (ANIM-322); the update then ends - skipping the zoom and scroll stages - **unless** the
     bounding both clipped and left a zero delta (ANIM-334).
  The **actor's own speed** is read from the actor's current animation state, so it is the speed of the clip being
  played, not a constant. Required observable traces: an actor **walking steadily** drifts until an axis reaches
  1 px and the camera then follows for 15 updates at the actor's own speed without recomputing; an actor
  **stopping** leaves the camera still, because the zero-length prediction path and the whole-pixel test between
  them produce no delta; an actor **reversing** has that axis's correction zeroed, so the camera never overshoots
  while the other axis continues; an actor **far away** is approached at its own speed per update, never in a jump;
  and at a **map bound** the camera stops while the actor walks on, the correction grows without being recaptured,
  and control falls through to the scroll stage.
- **ANIM-341** (observed, 004ca410, 004dba50; high). The actor lock is **set or replaced** by element kind 0xD, and
  **released** by element kinds 0xE, 6 and 7 (each of which calls the same setter, with the actor for 0xD and with
  nothing for the others); it is also dropped when the target is gone. **Player scrolling does not release it**: it
  clears only the separate marker that says the camera is centred on the selected character. **Native 20 does not
  release it either** - its only other write is the cached-view invalidation.
- **ANIM-342** (observed, 0050f710, 004c8380, 0053a4c0; high). A modal text page or dialogue runs its **own
  synchronous loop**, which redraws the page and does **not** advance the mission camera: a script scroll does not
  progress and cannot complete while such a page is open (`spec-script-vm.md` VM-218/222 state the same). The
  camera's independence from the tick (ANIM-004, ANIM-321) applies to iterations of the **main** loop whose tick was
  skipped - a level transition, the pause and the blocking flag - not to a modal loop. The setup of a page can
  invoke a single draw, hence at most one camera advance, before its loop begins. Entering a building does not move
  the camera.

### 3.13 Drawing one frame

The composition is required behaviour because it decides what is visible and what hides what. The pass ordering is
required; how it is organised is not.

- **ANIM-360** (observed, 005105d0; high). Required order: the **camera and the scene** (ANIM-361), then the **HUD
  widgets**, then an optional on-screen message clipped to a band at the bottom of the screen, then the **HUD panel
  and its text**, then the **mouse cursor**, then the **present**. The whole of it is skipped while the freeze flag
  of ANIM-521 is set.
- **ANIM-361** (observed, 004c8380; high for the ordinary case). In the ordinary case the scene begins with a full
  opaque copy of a **cached backdrop** into the back buffer and then runs the element passes; there are **no dirty
  rectangles**, so every frame redraws everything on top of that copy. The cached backdrop itself is maintained
  incrementally: it self-blits by the scroll delta and re-renders only the band the scroll exposed. **While a zoom
  transition is running the scene is not composed this way**: the transition blits captured surfaces (ANIM-326), so
  the ordinary pass list does not run on those updates, and nothing that depends on being drawn - the ground marks
  of ANIM-370 among them - advances on them.
- **ANIM-362** (observed, 004d0a10; high). Required order of the element passes: (1) a visibility and culling
  refresh, only when a dirty flag is set; (2) the camera transform and the clip box, including the vertical offset
  of **-80** px; (3) depth-key propagation for attached effects; (4) the **backdrop elements**; (5) the terrain patch
  renderer; (6) the **selection and highlight underlays**; (7) the **shadows**; (8) the **ground marks**; (9) the
  binding of queued draw items to their elements; (10) **the sort and the merge** (ANIM-363); (11) the **main element
  pass** in the merged order, flushing the deferred queue up to each actor's depth key before drawing that actor;
  (12) a final flush of the deferred queue; (13) the stretched sprites; (14) the movement-path line and its trail
  marks, only while the cursor is not over a pickable element; (15) floating text and labels; (16) a clearing of the
  per-element "drawn" flags. Everything after that is debug overlay, unreachable in the retail build's default state.
- **ANIM-363** (observed, 004d1d00, 004d1800, 00462a30, 004aa980, 004d8460, 004d1f50, 004d1810, 004bb820; high).
  The movable elements are **sorted** and then **merged** into the scenery order.
  - **The movable sort** uses the comparison at 00462a30, which the sort invokes directly: **the depth key
    ascending, ties broken by the element's own identity ascending** - a total, deterministic order.
  - **The scenery order is not file order.** It is produced **once, at level start**, by sorting the scenery list by
    the **smallest screen row among the vertices of the piece's profile polyline**, ascending and **strictly**, with
    **no tiebreak** - so pieces whose polylines share a topmost row keep whatever relative order the sort leaves, and
    an implementation must choose a stable order and record that as a divergence. The key is computed while the
    piece's polyline is read.
  - **The merge**: walking the scenery in that order and consuming the sorted movables **from the front**, every
    movable the sort-line test answers **true** for is emitted **before** the piece - so the piece is drawn later and
    **covers** it - and the movables left over are emitted after the last piece. The merge does **not** stop at the
    first candidate that fails. Both sides skip an element whose **visible mark** is clear, so an invisible scenery
    piece stops occluding. With **no** scenery pieces at all, every movable is emitted in pure sort order.
  - **The sort-line test, exactly.** The movable's **screen point** is tested against the piece's profile polyline,
    whose vertices are stored **ascending in x**:
    1. no polyline: answer `movable screen row < the piece's own screen row` (**strict**);
    2. the movable's x is **left of the first vertex**: answer `movable screen row < the first vertex's row`;
    3. the movable's x is **right of the last vertex**: answer `movable screen row < the last vertex's row`;
    4. otherwise: scan **forward** from the second vertex for the first whose x is **not less than** the movable's x,
       take that vertex and its predecessor as the segment, and answer `cross(segment vector, movable - segment
       start) < 0` (**strict**), which for a left-to-right segment is "the movable's row is above the segment's
       interpolated row".
    The scan has no index bound of its own: case 3 is what keeps it inside the polyline, and the polyline must have
    at least two vertices.
  - **The equality case**: a movable lying exactly on the line answers **false**, so it is emitted **after** the
    piece and is **drawn on top of it**. All three degenerate cases use the same strict comparison.
  Cases 2 and 3 have an observable consequence: beyond its ends the polyline behaves as if extended **flat** at the
  height of its first and last vertex, so a character walking past the end of a wall is judged against that flat
  extension.
- **ANIM-368** (observed, 004d8460, 004d88e0, 004d1d00; high for the split, medium for the family encoding). **Which
  elements take part.** At registration each element is put into exactly one of three drawing roles by its family
  word and its geometry, and the roles decide its ordering for the rest of the level:
  - **movable** - everything that is not of the scenery family; it is sorted by the depth key every frame;
  - **scenery** - a scenery-family element **that has a profile polyline**; it is a merge partner and can occlude;
  - **flat** - a scenery-family element whose sprite is marked flat; it is kept in a separate list and never enters
    the merge.
  The case an implementation is most likely to get wrong: **a scenery-family element with no polyline is registered
  as a movable** and is sorted by its own depth key exactly like a character - it does not occlude by a sort line
  and takes part in the depth ordering instead. Removal mirrors every list. A second, different comparison exists in
  the program (a family-class priority key first, then the depth key, then identity) but the routine using it has no
  traced caller and is **not** part of the mission frame (4.3).
- **ANIM-364** (observed, 00462a30, 005bd560, 0055f290; high; **cleared by review 24**). **The depth key is the
  element's world row** (the world y, not the screen row: an object lifted onto a roof sorts at the depth of the
  ground under it). Two adjustments exist: an element bound to another takes the other's key plus or minus
  **0.001**, and an effect attached to an owner takes the owner's key plus or minus **0.01**; one kind of spawned
  effect adds **1000.1**. These offsets order the pair **relative to each other**; they do **not** guarantee that
  nothing else sorts between them, and the 1000.1 case cannot be described as "in front of everything" without
  knowing the range of world rows in play.
- **ANIM-365** (observed, 004d0a10, 005c73e0, 005c2ec0; high). Effects and decorations are not a separate pass: they
  are queued with their own depth value and flushed into the main pass at the point where the queue's head is no
  longer nearer than the element about to be drawn, with a final flush at a sentinel depth of 1 000 000.
- **ANIM-366** (observed, 004d0a10, 004c0510, 004fbc30, 00523df0, 004eda80, 005243a0, 005246c0; medium-high).
  **Masks are not a drawing mechanism.** There is no separate occluder or mask pass in the scene's pass list, and
  the level file's mask records do not reach the drawing at all: each one is built from its own chunk into an object
  carrying one or two polylines, a topmost row, two points, a small opaque byte blob and, for one variant, a list of
  obstacle elements; the objects are registered into a **spatial grid of 64-unit cells** and every consumer traced
  from them is **sight and obstacle** code - the same "is this point above the polyline" test the sort line uses,
  extended with a height test - reached from the AI, the ground marks and the effects, **never from the scene pass**.
  The required conclusion for an implementer: **occlusion comes entirely from the ordering of ANIM-363**, using each
  element's own profile polyline; mask records belong to the perception model (`spec-navigation.md` NAV-210), not to
  the painter. What remains unsettled is narrow: a drawable effect class exists whose serialised form carries one
  extra value that may name a mask, and whether that class's own drawing consumes a mask's byte blob was not read
  (4.3).
- **ANIM-367** (observed, 004d0a10, 004c0510; high for the ordering, medium for the meaning). Drawing is **not layer
  by layer**: there is one merged list for the whole view. The layer index is a field of the element and selects
  which background and mask set it belongs to; the loader accepts indices from 0 to the layer count inclusive, one
  more than the number of layers, which is the extra index a character inside a building gets
  (`spec-navigation.md` NAV-190). Only the debug overlays iterate layers.
- **ANIM-370** (observed, 0051c0d0, 0051bfb0, 004cac00, 004d7dd0; high for the ageing rule, medium for the visual).
  **Ground marks** are a level-owned collection, unbounded in number, each holding a position, an animation frame
  index and a layer. They are created when the player issues an order (the marker at the destination) and, while the
  movement-path line is drawn, once every 11th draw along the trail. **Ageing happens inside the drawing pass**, and
  only for a mark that passes the pass's visibility test; for such a mark the frame index advances by one **on every
  qualifying draw whose level frame counter is even**, and a mark whose index reaches **6** is destroyed at that
  draw. The required result is therefore **six eligible ageing events**, not a fixed number of frames: a mark is not
  aged while it is off-screen, while the drawing is skipped, or during a zoom transition; a level counter that
  stands still on an even value ages it on **every** draw, and one that stands still on an odd value never ages it;
  and the parity of the counter when the mark is inserted decides whether its first draw ages it. **Six eligible
  ageing events is the invariant**, and the number of draws is not fixed even in the ordinary case: with an
  uninterrupted alternating counter and continuous visibility, a mark whose first drawn update has an **even**
  counter is removed on draw **11**, and one whose first drawn update is **odd** on draw **12**.
- **ANIM-380** (observed, 0052cf80, 0052d090, 004c0040, 00677598; high). **Resolution and viewport.** The mission view
  is the full configured width by the configured **height minus 80** px, anchored at the top left; the accepted modes
  are 1024x768, 1228x768 and 1360x768 (so the world view is 688 px high), the default is 1024x768, and an unknown
  width is refused with a report. **There is no letterboxing and no border**: a wider mode shows more of the world.
  Video playback switches temporarily to 640x480. Whether the two wide modes are original or were added for this
  release was not established. Note that the location conversion of ANIM-329 centres on the **full** height while the
  bounds use the reduced one.
- **ANIM-381** (observed, 005e41d0, 005e5260; high). The back buffer is a double-buffered flip chain in 15-bit or
  16-bit colour with no 8-bit path; presenting is a flip when fullscreen and a blit to the primary surface when
  windowed; the sprite blits key on pure green.

### 3.14 Settings, speed and difficulty

- **ANIM-520** (observed, 005460c0, 0050f710; high for the selection, **unknown** for the realised ratio). A
  **slow-motion** toggle selects the 400 ms pacing minimum instead of 40 ms (ANIM-001). The **nominal** ratio is 10.
  The **realised** ratio is host-dependent for the same reason as ANIM-002 and was not measured: with a quantised
  counter the two thresholds need not scale by the same factor, so neither "exactly 10" nor any particular other
  number is established here. What is established is that nothing else changes: the animation clock, the movement
  per update and the camera rules are all per-update rules, so slow motion scales the rate at which updates happen
  and nothing inside them.
- **ANIM-521** (observed, 0050f710, 005105d0; medium-high). Two flags remove the pacing wait: a **freeze** flag,
  which also blocks the level tick and the whole drawing and which nothing in this build ever sets, and a
  level-side **blocking** flag, which removes the wait and the cursor and present steps and makes the later part of
  the camera and composition work run only on every 32nd level frame (ANIM-321) - it does **not** stop the camera's
  own state advance and does not stop the drawing as a whole. While either is set the loop runs uncapped. A frozen
  game does not "shrink durations": nothing advances at all while the freeze flag is set, whereas the blocking flag
  leaves the tick running uncapped, which does compress real time.
- **ANIM-522** (observed, 0055d1a0, 0051ba00, 005b2190, 00409d70, 005a6520; high). The persisted settings are the
  player profile, two key-binding sets (also as two configuration files under the game's data directory), the sound
  configuration and the graphics configuration (the resolution and four further bytes). The registry holds only the
  sound device, the language, the version and the path. The command line has no timing switch. None of these changes
  any rule in sections 3.1 to 3.13.
- **ANIM-523** (observed, 0055dbb0, 00438710, 00438600, 004936f0; high for the shape, deferring for the meaning).
  **A difficulty setting exists in this build**; `spec-ai-combat.md` AI-045 owns it and this document defers to it.
  What is established here about the shared scaling helper: it takes a value, two factors and a cap, and it applies
  them **only when the element it is called for answers that it is on the hostile side** - an element that does not
  is returned unchanged whatever the difficulty. For a hostile element it returns the value multiplied by the
  **first** factor at difficulty **0**, by the **second** at difficulty **2**, and unchanged at difficulty **1** or
  any other value; the product is **capped at the third argument before** the conversion to an integer, which
  truncates; the value and the cap are taken as 16-bit quantities. One caller supplies the factors 0.5 and 2.0 with
  a cap of 100, and the player-character update consults the same difficulty value. Difficulty therefore changes AI
  and combat quantities, and an implementation must reproduce it. It changes **nothing** in this specification: the
  pacing, the animation clock, the displacement rules, the turning, the camera and the drawing are independent of
  it. The revision-1 claim that no difficulty setting exists was withdrawn in revision 2; AI-045 stands.

## 4. Claims, clearance and assumptions

### 4.1 Claim index

Every claim of sections 2 and 3 carries its id, status, evidence address and confidence inline as **ANIM-nnn**
(status, address; confidence). Ranges: **001-005** clocks, execution opportunities and the freeze gate; **010-014**
the animation table; **020-023** state and its initial values; **030-036** the frame timer and the entry points;
**040-043** the play modes; **100-103** the frame, the tick phases and the update order; **120-134** actions,
statuses, completion and transitions; **200-209** displacement; **210-215** turning; **220-222** placement;
**240-243** collision and arrival; **300-305** the cart; **310-312** lifts and layers; **320-342** the camera (the
location conversion is **ANIM-329**); **360-381** drawing; **520-523** settings and difficulty. The mode table of
3.3 is covered by ANIM-030 to ANIM-043, the completion table of 3.4 by ANIM-120 to ANIM-140 together with the
native rows of section 6, and the pass list of 3.13 by ANIM-360 to ANIM-367.

### 4.2 Cleared for implementation, rule by rule

A rule is listed here only when nothing in 4.3 qualifies it. Where a claim has both settled and unsettled parts,
only the settled part appears here, with the qualification spelled out, and 4.3 names the rest. The distinction
that matters most: the **arithmetic and state transitions** of the frame timer are settled and testable with
supplied synthetic values, while the **file fields** two of them read are not (ANIM-012, ANIM-013), so a rule that
consumes a marker or a loop length is cleared **for a supplied marker** and excluded for a marker read from a
profile.

- **Pacing and clocks**: the 40 ms / 400 ms reported-time thresholds and the absence of main-loop catch-up
  (ANIM-001); the four execution gates and their independence (ANIM-004); the freeze-all gate **with the scope
  and the write ordering ANIM-005 states**; the step and call accounting of ANIM-003 **with every exception it
  lists** - movement mode 3's second step, movement mode 6's conditional extra step, the repeated-operation
  actions of ANIM-212, and the four calls that step **nothing** (the play-only admitting call, a call under
  freeze-all, a call for a missing action, and the exact-equality immediate completion) - and the restriction of
  the `hold + 1` display duration to a clip that is **advancing**, with the held-frame states ANIM-003 lists.
  *Not* the realised frame length, which is ANIM-002's host-dependent reference value and `ADR-0010`'s engine
  decision.
- **Animation table**: action-id lookup and `block + facing` (ANIM-010); the per-animation quantities the player
  needs (ANIM-011); the replacement lookup of natives 60/61 (ANIM-014a).
- **Frame timer, for supplied values**: the reset rule including the modes that have no reset case (ANIM-030);
  forward stepping and `hold + 1` (ANIM-031, **cleared by review 24**); the timer-zero displacement gate (ANIM-032,
  **cleared**); the completion signal (ANIM-033); the early marker's **four computed cases as arithmetic over a
  supplied marker value** (ANIM-034); the entry-point and precedence rules (ANIM-035, **play-only precedence
  cleared by review 30**) with the narrow immediate-completion condition and its "no timer step", **except** the
  admitting-update status of movement mode 5, which 4.3 excludes; the non-stepping modes and mode 10's
  dependence on a reset (ANIM-036); the stochastic-idle draws (ANIM-040); the absence of a ping-pong mode
  (ANIM-041); mode 14's inability to leave a valid clip (ANIM-042).
- **The mode table of 3.3**, with one qualification: modes **6 and 7** are cleared **only for a supplied marker
  frame**, because which file field the marker is remains open (ANIM-012); every other row is cleared outright.
- **Frame and tick order**: the iteration order (ANIM-100); the tick phase order including the pre-update cart pass
  and the two distinct later lists (ANIM-101, **cleared**); the element traversal with the re-read count and the
  index that advances past a removal (ANIM-102, **cleared**); the staggered phase (ANIM-103, **cleared**).
- **Actions**: the status set, its per-entry precedence and the actor's response to 3 and 4 (ANIM-120, except the
  parts 4.3 names); the finish code's existence (ANIM-121); the idle chain (ANIM-130); the per-action play and
  movement modes (ANIM-131); the per-action choice of completion status (ANIM-132); the wait's zero-before-decrement
  rule and its `k`-decrements-then-complete trace (ANIM-133, **cleared**); the **idle pair** of ANIM-130 with its
  next-update timing and its wait-element exclusion, and the distinction of ANIM-134 between an in-place action-id
  change and succession - but **not** an enumeration of the in-place cases, which 4.3 excludes; native 49 and
  native 50 completion for human targets (**cleared**); native 51's completion and the deferred launch of its
  freeze element (3.4).
- **Displacement**: the magnitude rule and the caller-supplied factor (ANIM-200); the direction and the recorded
  start position (ANIM-201); the mode-3 four-case arithmetic (ANIM-202, **cleared**); the turning scale and floor
  (ANIM-203); the collision-free modes (ANIM-204); the published velocity (ANIM-206); the footstep effect with its
  **confirmed ground-kind selector** and its phase counter (ANIM-207); arrival as the trigger for status 3
  (ANIM-208). Movement mode 3's arithmetic **and its single caller, action id 304** (ANIM-202).
- **Turning**: the shorter-arc rule with the counter-clockwise tie (ANIM-210, **cleared**); the four variants
  including the damped one's third-call rule and the delayed one's spacing (ANIM-211, **cleared**); the ordering of
  the turn operation before the play and the displacement, and the counts of ANIM-212 - one execution for an
  ordinary action, exactly two for action id 294 and at most two for 296 to 299 with the stop-on-status-3 rule,
  together with the rule that a turn operation is not necessarily a facing change; the target-facing refresh
  conditions (ANIM-213); mode 7 as the only player-side facing change
  (ANIM-214); stride preserved through a turn (ANIM-215).
- **Placement and initial state**: the placement order and the copied facing (ANIM-220); the placed facing
  (ANIM-221); off-map behaviour (ANIM-222); and the **established construction-time values** of ANIM-020, ANIM-021
  and ANIM-023 - animation index 0, frame 0, timer -1, an unreachable early-marker pair, last-reset action id 283,
  facing and target facing 0, turn delay counter 2, turn hysteresis counter 0, the damped-turn selector clear, the
  ground kind from the level default, failed-move counter 0, progress region invalid, default collision tolerance
  4.0, footstep phase 0, and the camera values ANIM-023 lists.
- **Collision and arrival**: the guard list of ANIM-240 **as corrected** (no projection-area comparison) and the
  directional projection against 5 **for character candidates only**, with the operand being the **normalised
  direction** (that list and that comparison only); the failure counter's two resets, the
  region's validity mark and inclusive containment, and the threshold that refuses the element (ANIM-241, the
  counting rule only, **both resets cleared by review 30**) together with **where the progress check runs in the
  update** and the two earlier increments; the **four** arrival cases of ANIM-242 with their selectors, metrics and
  inclusive/strict boundaries (but not what sets the stopping tolerance); the **deflection** of ANIM-244 - its
  gap-sorted, nearest-first, one-at-a-time-with-re-sort ordering, the two solves with their acceptance conditions,
  and the two degenerate fallbacks; the **absence of rollback** and the single commit (ANIM-245); the in-move
  **unstick** with its backoff, inflation, pass limit and push rule (ANIM-246); the **absence** of any door or
  corner special case (ANIM-247); bonds (ANIM-243).
- **Cart**: the speed integration (ANIM-300); the program's block structure and the **conditional** weighted draw
  (ANIM-302); the five instruction effects stated and the one instruction that advances without reducing the block
  length (ANIM-303, those only); bonds (ANIM-304); the pre-update pair traversal and the two parameter pairs
  (ANIM-305, **cleared** for the traversal).
- **Camera**: the state list and the view rectangle (ANIM-320, with the bounding qualification it states); the
  location conversion in all five steps including the conditional zoom-reset fallback (ANIM-329, **cleared by
  review 30**); the advance timing and the 32nd-frame return (ANIM-321); border clipping and scroll termination
  (ANIM-322); the key-scroll gating, latch rule, asymmetric start, coasting and zoom division (ANIM-323), **including
  the generating rule of its ramp**; the absence of drag panning (ANIM-324); the three zoom values and the request
  semantics
  (ANIM-325); the eight transition steps and the busy gate (ANIM-326, except when the current zoom changes); the
  **whole of ANIM-324**, including the
  one-pixel border, the diagonal corners, the absence of a latch and the two gates; zoom
  as a global draw scale (ANIM-327); the scroll element's per-update rule and its arrival-on-a-later-update
  completion (ANIM-330); natives 18/19 as scroll requests with all five consequences (ANIM-331); native 20 and
  kind 7 as jumps (ANIM-332); the zoom element's three completion cases (ANIM-333, except the accepted case's
  count); the lock-zoom-scroll precedence and the lock's ability to end a camera update (ANIM-334); **the whole of
  ANIM-340** - the
  visibility test that decides whether setting a lock recentres, the centring conversion it then uses, the
  remembered offset that is never recaptured, the skip while a burst runs, the prediction and its overshoot guard,
  the zero-length 1/30 path, the inclusive 1.0 burst threshold and the strict 1.0 speed replacement (**those two
  cleared by review 30**), the rounding, the whole-pixel zeroing and the end-of-update behaviour; lock
  set/replace/release (ANIM-341); the modal loop's isolation (ANIM-342).
- **Drawing**: the frame order (ANIM-360); backdrop reuse and the absence of dirty rectangles (ANIM-361); the pass
  list (ANIM-362); **the whole of ANIM-363** - the movable comparison with its identity tie-break (**cleared**),
  the load-time scenery order by the polyline's topmost row with its instability, the merge's front-consuming walk
  and its visible-mark skip, and the sort-line test with its four cases, its strict comparisons and its
  draw-on-top equality; the three drawing roles and the "scenery piece without a polyline is a movable" case
  (ANIM-368);
  the depth key as the world row with its three offsets and their non-adjacency (ANIM-364, **cleared**); the
  deferred queue (ANIM-365); the absence of a top-level mask pass and the positive finding that mask records belong to
  the perception
  model and never reach the scene pass (ANIM-366); the single merged
  list and the extra layer index (ANIM-367); ground marks' six eligible ageing events and the conditions on them
  (ANIM-370); the resolutions and the 80 px strip (ANIM-380, except whether the wide modes are original); the
  surfaces (ANIM-381).
- **Settings**: slow motion as a pacing selection that changes nothing else (ANIM-520, except its realised ratio);
  the two wait-removing flags (ANIM-521); the persisted settings (ANIM-522); the difficulty helper's shape with its
  hostile-side guard and its capped truncation, deferring the meaning to AI-045 (ANIM-523, **existence cleared**).

### 4.3 Excluded from clearance

Each row is an `Assumption` variant (ADR-0008) for the implementation. **A fallback suggested here is a way to
keep going, not a cleared requirement**: an implementation must carry the assumption, expose it, and pin it with a
recording or a further read before treating it as fact.

| Rule | What is missing | Assumption |
|---|---|---|
| ANIM-012, ANIM-013: which animation-record field is the marker and which the loop length | the mapping to the file fields; everything that consumes a marker or a loop length read from a profile - play modes 6 and 7, the early marker of ANIM-034 as computed from real data, the climb loop count | `AnimMarkerField` - the arithmetic is cleared for supplied values only |
| ANIM-014(b): the animation override list | its contents, population, the mark's consumer | `AnimOverrideList` - treat the list as empty |
| ANIM-134: the in-place action-id substitutions | which executor branches change their element's action id in place, besides the idle pair and the missing-action fallback | `ActionIdSubstitution` - implement the idle pair only and log any other need |
| ANIM-020: three animation flag bytes | their meaning and their construction-time value (everything else in ANIM-020 to ANIM-023 is now established) | `AnimUnknownFlags` - leave them out of the model until a consumer is found; the action-id lookup selection starts at the normal lookup |
| ANIM-023: the cart's initial and reset state | the cart's construction and its program container were not read, so the program position, block remainder, speed, target speed, acceleration, waypoint index and rattle accumulators have no established defaults and no established reset on activation | `CartInitialState` - zero them on activation and expose it |
| ANIM-034: the early marker's purpose | why it is pulled back, and which actions outside the profiles read depend on it | `AnimEarlyMarker` |
| ANIM-042: frames outside a clip | for mode 7's terminal-marker case the consumer is **demonstrated** (the next step reads that index's hold value): what is unknown is the value read and its effect on the following cycle. For the mode-2 and mode-3 states no consumer was traced | `AnimOutOfRangeFrame` - clamp and log |
| ANIM-043: mode 7 beyond its two traces | marker values other than 0 and the last frame, in profiles not read | `AnimTurnTable` |
| ANIM-120 (unestablished parts), ANIM-122 | the per-kind admission tests and their refusals; whether any status but 4 refuses; whether the next element also executes in the same update | `ActorAdmission` (also `spec-script-vm.md` VM-216) |
| ANIM-123: non-human targets of natives 49/50/51 | those executors | `AnimTargetFamily` |
| ANIM-140: the speech duration | its source and its fallback | `SpeechDuration` |
| ANIM-209: movement mode 5's completion contract | what the records are, their order, re-entry after an interruption, which record-walking branches answer 3, and hence the admitting-update status of a mode-5 element | `WaypointRecords` - two triggers are described, exhaustion is **not** the contract |
| ANIM-240: the proximity partner's eligibility and query box | the class and state tests that were read only in part, and the geometry of the query box itself; the target-family branch's "is solid" answer | `ProximityEligibility` |
| ANIM-240: the proximity reaction | pushing, waiting, stepping aside; whether it moves either element | `CharacterPush` (also `spec-navigation.md`) |
| ANIM-242: the stopping tolerance | what writes the element's stopping tolerance (it is not written by the mover, the arrival test or the play call) | `ArrivalTolerance` - take it from the walk order's radius and expose it |
| ANIM-244, ANIM-247: the corridor test | the passability rule the deflection and the arrival test both call, including whether doors are special inside it | `CorridorRule` - the single largest routine still unread |
| ANIM-301: the cart's sub-sprites | which condition selects case A, B or C; what consumes the reciprocal | `CartSubSprites` |
| ANIM-303: the cart program | two unread instructions, the container in the mission file | `CartProgram` |
| ANIM-305: the cart hit | which comparison selects the larger parameters | `CartHitSeverity` |
| ANIM-324: the edge-scroll exclusion rectangles | who fills the list of rectangles that suppress an individual edge command (the HUD's own areas) | `EdgeExclusion` - suppress edge scrolling over the HUD panel and expose it |
| ANIM-326, ANIM-333: the zoom transition | when the current zoom changes, hence an accepted request's duration | `ZoomTransition` |
| ANIM-340: the follow's screen-size inputs | the visibility rectangle and the centring use the configured screen size, read from a display-configuration object whose fields were identified by use, not by their writer | `CameraScreenSize` - take them from the configured resolution |
| ANIM-363, ANIM-368: the family word | the encoding of the family bits was read only as far as the three-way split needs, and the alternate comparison's routine has no traced caller | `DrawFamilyBits` - implement the split by role and log an element that fits none |
| ANIM-366: the masked effect class | whether the one drawable effect class whose serialised form carries an extra value consumes a mask's byte blob when it draws | `MaskedEffect` - draw it as an ordinary element |
| ANIM-380: the wide resolutions | whether they are original | `WideModes` |
| ANIM-520: slow motion's realised ratio | the realised ratio on any host | `SlowMotionRatio` - the mode need not be implemented (`ADR-0010`) |

### 4.4 Boundaries closed in revision 6, and what is left

Review 44 judged revision 5 "not yet cleared as an implementable whole" and listed the boundaries in the way of
that. This section records what revision 6 did with each, in the priority order the movement batch needs.

| Boundary (review 44) | Revision 6 |
|---|---|
| **Arrival geometry** | **Closed.** ANIM-242 now gives all **four** cases of the mover's test with their selectors, metrics and inclusive/strict boundaries, separately from the door-approach element's Chebyshev rule. What is left is narrow and named: what writes the stopping tolerance (`ArrivalTolerance`). |
| **Collision sliding** | **Closed.** ANIM-244: a deflection that keeps the step's length, blockers gap-sorted nearest first, resolved one at a time with a re-sort between, a circle solve and a plane solve with explicit acceptance conditions, and two degenerate fallbacks. The one dependency left is the corridor rule it calls (`CorridorRule`). |
| **Rollback** | **Closed.** ANIM-245: there is no rollback because the position is not mutated during resolution; a single commit, or the element does not move. ANIM-246 adds the in-move unstick. |
| **The mode-3 caller** | **Closed.** ANIM-202: movement mode 3 is selected by **one** action id (304) on the human executor's second dispatcher; no other caller passes it. |
| **Execution accounting** | **Closed.** ANIM-003 lists every exception in both directions (extra steps, and the four calls that step nothing); ANIM-212 gives the five repeated-execution action ids in their two shapes, and states that a turn operation is not necessarily a facing change. |
| **The initial turning counters and the frame-timer initial state** | **Closed.** ANIM-020, ANIM-021 and ANIM-023 now give the construction-time values: animation index 0, frame 0, timer -1, an unreachable early-marker pair, last-reset action id 283, facing and target facing 0, turn delay 2, turn hysteresis 0, the damped-turn selector clear, ground kind from the level default, failed-move counter 0, region invalid, default tolerance 4.0, footstep phase 0. Three animation flag bytes remain (`AnimUnknownFlags`). |
| **The footstep selector** | **Closed.** ANIM-207: the selector is the ground kind of `spec-navigation.md` NAV-003, initialised from the level default and re-established from the area or the material. |
| **The drawing order's scenery boundaries** | **Closed.** ANIM-363 gives the sort line's four cases, its strict comparisons, its draw-on-top equality and its flat extension beyond the polyline's ends; the load-time scenery order by the polyline's topmost row, unstable; and the merge's front-consuming walk with its visible-mark skip. ANIM-368 gives the three drawing roles. |
| **Mask integration** | **Closed as a negative.** ANIM-366: mask records are perception geometry, registered into a spatial grid and consumed by sight and obstacle code; they never reach the scene pass. Occlusion is the ordering alone. One narrow question remains (`MaskedEffect`). |
| **Proximity eligibility and query geometry** | **Partly closed.** ANIM-240's guard list is corrected and the comparison is cleared; the class and state tests and the query box's geometry stay excluded (`ProximityEligibility`), as does the reaction (`CharacterPush`). |
| **The camera ramp** | **Closed.** ANIM-323 now gives the generating rule - a working value from 6.0, its integer part bumped to even, floored into both tables, multiplied by 1.05 while below 31 - and the sequence it produces, rising from 6 to a ceiling of 32 px per update. |
| **Edge scrolling** | **Closed.** ANIM-324: the pointer is polled once per input poll against a **one-pixel** border on each edge, the four independent tests let a corner scroll diagonally, there is no latch so the commands repeat every poll, and the same poll turns the wheel into the two zoom commands. What is left is who fills the HUD exclusion rectangles (`EdgeExclusion`). |
| **The follow behaviour** | **Closed.** ANIM-340 now gives the whole controller: the visibility test that decides whether setting a lock recentres and the centring conversion it uses, the remembered offset that is never recaptured, the burst skip, the prediction with its overshoot guard, the zero-length 1/30 bleed, the inclusive and strict 1.0 thresholds, the speed replacement from the actor's current animation, the rounding and the whole-pixel zeroing, with the five required traces. What is left is only the source of the configured screen size (`CameraScreenSize`). |
| **Accepted zoom completion timing** | **Open** (`ZoomTransition`): it needs a recording, and section 7's zoom test says so. |
| **Frame-timer marker mapping, overrides, out-of-range consequences, the other in-place action-id substitutions, admission, speech** | **Open**, each with its row in 4.3 and its address in section 10. |
| **Cart initial state, program and sub-sprites** | **Withheld deliberately** (`CartInitialState`, `CartProgram`, `CartSubSprites`). Review 44 allows this; the observable contract the implementer needs is in 3.10: a cart is speed-driven, it runs a program with a conditional weighted branch, it rattles its sub-sprites with two draws each per update, and it runs people over through a pass that precedes the element updates. |

## 5. Constants

Individual functional facts. No table of the shipped data is reproduced: the per-frame hold and advance values, the
marker and loop fields and the per-animation displacement are read from the player's `DATA/Characters/*.rhs` files
by the rules of `docs/formats/sprite-animations.md` (rules 1-3) and ANIM-011. A value marked *(4.3)* belongs to an
excluded rule and must be carried as an assumption.

| Name (ours) | Value | Unit | Source | Conf. |
|---|---|---|---|---|
| pacing minimum, nominal | 40 | ms of reported elapsed time | 0050f710 | high |
| pacing minimum, slow motion | 400 | ms of reported elapsed time | 0050f710 | high |
| reference cadence on the measured host | 46.875 | ms per frame | 0050f710 + section 9 | medium (host-dependent) |
| engine logic frame (our decision) | 46.875 | ms per frame | `ADR-0010` | n/a (decision) |
| frame display length | hold value + 1 | timer steps | 005b7820 | high |
| timer value a reset installs | -1 (mode 10: 0; modes 14 and unrecognised: unchanged) | timer units | 005b7300 | high |
| facings | 16 | - | 0055f0f0, 005b86b0 | high |
| turn step (ordinary / fast / mode 7) | 1 / 2 / -2 | of 16 per call | 0055f0f0, 0055f140, 005b7820 | high |
| damped turn: first move | on the 3rd same-sign call from zero | calls | 0055f210 | high |
| delayed turn: step spacing | the caller's value + 1 | calls | 0055f1a0 | high |
| mode 7 rotations (timer 0 / timer -1 entry) | 8 / 9 | steps of -2 | 005b7820 | medium |
| turning advance factor | 0.6 (movement mode 6: x2) | - | 005b86b0, constant at 00679e40 | high |
| turning advance floor | 0.7 | px per update | 005b86b0, constant at 00677db8 | high |
| stochastic idle bound (play mode 4 / 5) | 131 / 327 out of 32768 | per drawing call | 005b7820 | high |
| idle to fidget probability | 1 in 10 per idle cycle | - | 00464b20 | high |
| proximity qualification | directional product at least 5 | px times the movement vector | 00561040, 005fc660, constant at 0067748c | high |
| plain arrival | directional product not greater than the element's tolerance | same | 00560460, 005fc660 | high |
| door-approach tolerance | Chebyshev distance strictly below tolerance + 5 (tolerance 10 in the walk pipeline) | px | 00467a50 case 2, 005fc700 | high |
| failed-move limit without progress | 50 (the 51st fails) | failures | 00563e90 | high |
| collision tolerance: default, decrement per failure, floor | 4.0, 0.2, 1.0 | px | 0055eee0, 00563ed0 (00677b3c), 0067749c | high |
| deflection fallback fraction, wall / round | 0.95 (and its negative) / 0.99 | of the step length; 8-byte constants | 00679100, 00679108, 00679118 | medium |
| step backoff factor before the unstick | 0.8 | of the step length | 00679724 | medium |
| minimum step length before the unstick | 0.1 | px | 00677580 | medium |
| unstick box inflation / passes / corner threshold / push | 0.2 / 50 / -0.1 / penetration + 1.0 | px, passes | 004f5890, 006787f8 | high |
| wedged arrival slack (Chebyshev, strict) | 10.0, plus both collision radii when an order object applies | px | 00560460, 00677438 | medium |
| isometric arrival correction | 1.7434 | factor on the second component | 00560460, 006774ec | medium |
| turn delay counter at construction | 2 | calls | 0055eee0 | high |
| frame timer at construction | -1 | timer units | 005b5890 | high |
| last-reset action id at construction | 283 | action id | 005b5890 | high |
| progress region half-size | about 0.49 | px | 00563ed0 | medium |
| footstep effect selector value | 5 | ground kind | 005b86b0 | medium (4.3) |
| footstep displacement threshold | 2.0 | px per update | 005b86b0, constant at 006774f4 | high |
| footstep effect period | every 3rd qualifying update | updates | 005b86b0 | high |
| cart target speed replacement for zero | 0.1 | px per update | 004ac350, constant at 00677580 | high |
| cart rattle amplitude | speed x 0.1, accumulated and clamped to -1..1 | px | 004ab7e0, constant at 00677580 | high |
| cart branch roll (when the weighted path is taken) | 1 + rand() mod 100 | weight units | 004abfe0 | high |
| cart hit parameters | (10, 10) or (50, 50) | as the hit action defines | 004d9420 | high (values) / unknown (selector) |
| zoom values | 0.5, 1.0, 2.0 | factor | 00571200, 004bef3b | high |
| zoom at level start | 1.0 | factor | 004bef59 | high |
| zoom transition steps | 8 | updates of rendering | 004c9119 | high |
| location conversion: centring | point minus screen size / (2 x zoom), full screen height | px | 005758d0 | high |
| location conversion: bounds | 0 and map size minus view size (view = width / zoom by (height - 80) / zoom) | px | 005758d0 | high |
| location conversion: fallback | current zoom := 1.0, corner := (0, 0) | - | 005758d0 | high |
| location conversion: alignment at zoom 0.5 | both components even | px | 005758d0 | high |
| camera scroll ramp: entry 0, working start, parity rule, store, growth, bound, entries | 0, 6.0 (single precision), bump to an even truncated integer part, round to nearest (ties to even), x1.05, 31.0, 32 | px per update | 004bef6e-004bf008, 0067749c, 006777d0, 00678b30 | high |
| camera scroll step scaling | ramp entry / zoom | px per update | 004dba50, 004db920 | high |
| script scroll step length at element start and from native 18 | 2.0 | px per update | 004ca410, 00571270 | high |
| script scroll step length, neutral (also the value that resets the ramp index) | 1.0 | px per update | 004cdfc0 | high |
| script scroll speed parameter | unsigned 16-bit, 0 = use the step length and the ramp | px per update | 004cdfc0 | high |
| camera lock: burst threshold (inclusive) and speed-replacement threshold (strict) | 1.0 | px | 004cdfc0, 8-byte constant at 00677d90 | high |
| camera lock: burst length | 15 | updates | 004cdfc0 | high |
| camera lock: small-correction bleed | 1/30 | per update | 004cdfc0 | high |
| edge-scroll border | 1 | px on each edge | 00517900 | high |
| HUD strip excluded from the world view | 80 | px | 00677598 | high |
| depth key sibling offset | 0.001 | world rows | 005bd560, constant at 006774cc | high |
| depth key attached-effect offset | 0.01 | world rows | 005c72a0, constant at 006774f0 | high |
| depth key spawned-effect offset | 1000.1 | world rows | 005c2ec0, constant at 00679e58 | high |
| deferred queue flush sentinel | 1000000 | world rows | 004d0a10 | high |
| ground mark ageing events before removal | 6 | qualifying even-counter draws | 0051c0d0 | high |
| trail mark period | every 11th draw | draws | 004d7dd0 | high |
| resolutions | 1024x768, 1228x768, 1360x768 | px | 0052cf80 | high |
| world view height | screen height - 80 (688) | px | 004c0040, 00677598 | high |
| colour depth | 15 or 16 | bpp | 005e41d0 | high |
| staggered bookkeeping period / phase | 64 / the element identity's low 5 bits | updates | 00471b00 | high |
| difficulty scaling (hostile side only) | min(value x factor, cap), truncated; factor A at level 0, factor B at level 2, unchanged otherwise | - | 00438710 | high |

## 6. Interfaces to the script VM

The arity, coercion and error conventions are `spec-script-vm.md` (revision 4) section 6. `ActionChange` is a
compatibility token. Every camera point named below passes through the conversion of **ANIM-329** before it is
used; "the point" in this table always means its conversion.

| Id / kind | What it really does | Completion | Claim |
|---|---|---|---|
| 18 `(loc)` | requires a non-null location; sets the camera's **scroll destination** (as given and converted) and the **scroll step length to 2.0**. No jump, no zoom request, no new camera element, no effect on the scroll speed or the ramp index - but an existing camera element is **kept**, and the scroll's arrival completes it. The first step is 2 px **only when the leftover scroll speed is 0**; otherwise that speed is used instead | nothing new waits for it; an actor lock that is enabled and producing movement **suspends the motion** (ANIM-334) unless its own step is clipped to a zero delta, while a zoom that completes first removes the camera element so that the arrival completes nothing | ANIM-331, ANIM-334 |
| 19 `(loc, f)` | as 18 with the step length := `f`. Effective on the scroll's first update only when the leftover scroll speed is 0; the value **1.0** makes the **first** ramp refresh set the index to 0, so the **ramp length** for the next update is the ramp's zero entry - which stops the camera only when the leftover scroll speed is also 0, since a non-zero retained speed is used instead of the length - after which the refreshes advance normally | as 18 | ANIM-331, ANIM-334 |
| 20 `(loc)` | sets the camera **corner** to the converted point - an immediate jump - and invalidates the cached view. Does not touch the scroll destination, the zoom request or the actor lock | immediate | ANIM-332, ANIM-329 |
| 21 `(f)` | sets the **requested** zoom; only 0.5, 1.0, 2.0 accepted, else an error and no change | the camera update performs it | ANIM-325, ANIM-326 |
| 33 / 42, kind 6 | scroll the camera to a point; 33 records speed 0 ("use the step length and the ramp"), 42 records an unsigned 16-bit speed in px per update. Starting it completes any previous camera element, releases the actor lock, sets the destination, the speed, step length 2.0 and ramp index 0 | when the destination is found to be reached at the **start of a later** camera update, or when a step clips at a map bound; at once when the level's no-presentation flag is set | ANIM-330, ANIM-322 |
| 34, kind 7 | jump the camera to a point, completing any previous camera element and releasing the actor lock | at once | ANIM-332 |
| 35, kind 8 | set the **requested** zoom | already-equal: on the element's first camera check; refused: on the check after the refusal; accepted: when the current zoom has become equal, count unsettled | ANIM-333 |
| 39, kind 0xD | **set or replace** the actor lock; setting can move the camera at once | at once | ANIM-340, ANIM-341 |
| 40, kind 0xE | release the actor lock | at once | ANIM-341 |
| 49, kind 0xA4 | play an action id in play mode 0 | at the end of the first cycle | 3.4 |
| 50, kind 0xA5 | play an action id in play mode 1 | never by itself | 3.4 |
| 51, kind 0xA6 | play in mode 0 and, on the completion signal, **launch** a freeze sequence (kind 0xA7, play mode 12) before reporting | the 0xA6 element at the end of the cycle; the freeze element reaches the actor through the deferred queue, so the next sequence level can start first, and the freeze element itself never completes once admitted | 3.4 |
| 60 / 61, kinds 0xAA / 0xAB | switch or restore the action-id lookup | at once | ANIM-014a |
| 45 / 212 / 46 / 47 / 64, kind 0x14 | a walk (`spec-navigation.md` NAV-130) | on arrival; a path failure or a persistent blockage refuses it | 3.4, ANIM-208 |
| 48 and 59 code 1, kind 0x1A | turn to a point or a facing | when the facing equals the target facing | ANIM-210 |
| 57 / 70 / 71, kind 0x15 | seek an actor | on arrival; at once when the seeker is the target | 3.4 |
| 62 / 69, kind 0x92 | speak | when the line ends (unsettled) | ANIM-140 |
| 67 / 68 / 72 / 73, kinds 0x9C-0x9F | start / stop / activate / deactivate a cart | at once | 3.10 |
| 139 immediately, 226 / kinds 0x11 and 0x12 as elements | set or clear the level's **freeze-all** flag, which stops the animation player's timer steps, its displacement and its completion signals, and the NPC update's AI step - but not cart displacement, executor-side turning, the tick phases, the camera or the drawing (ANIM-005) | native 139 writes the flag inside the call; the element is **marked completed before** its flag write, so the sequence level can advance first | ANIM-005 |
| 93 / 94 | read / set the facing (`d mod 16`); setting writes the facing itself, so no turn follows | - | ANIM-213 |
| 96 / 156 / 152 | place off the map / into a building / out of one | - | ANIM-220, ANIM-312 |
| 101 | the current action id, 283 when none | - | ANIM-102 |
| 103 | stop the actor | - | 3.4 |
| 140 | the walking style the AI uses, i.e. which locomotion action id it chooses | - | ANIM-131 |
| `ActionChange(current, previous)` | runs from the actor's update when the current action id changes, with that actor as the current actor; 283 means none | - | ANIM-102 |

## 7. Acceptance tests

Two kinds of case appear below and they are labelled. **[S]** is a **synthetic** fixture: an animation described by
its frame count, per-frame hold values and per-frame advances, chosen here for the boundary it exercises, or a
synthetic level state. **[D]** is **data-backed**: it reads the player's own files by file and field - the hold and
advance halves of the frame timing word and the frame count of a named action id of a named profile, by
`docs/formats/sprite-animations.md` and ANIM-011 - and asserts a relation, not a literal. Times use the engine's
logic frame (`ADR-0010`); a test that asserts an absolute duration must state the cadence it assumed. No test may
assert a rule listed in 4.3 as fact; where one appears below it is marked *(assumption)*.

**Timer and modes**

1. **[S]** *Frame display length.* Clip A: 3 frames, holds (0, 2, 0), advances (0, 0, 0), play mode 0, driven
   through the **play-and-move** entry with a fresh action id. The frame index after successive updates must be
   0, 1, 1, 1, 2, 0, 1, 1, 1, 2, ... - frame 0 once, frame 1 three times, frame 2 once, a cycle of 5 updates equal
   to the sum of `hold + 1`. The completion signal must fire on the update that lands on frame 2 and again one
   cycle later.
2. **[S]** *Entry points and precedence.* Clip A started through the **play-only** entry must answer status 1 on
   the admitting update and must not step, so its frame trace is test 1's shifted by one update. A second action
   element carrying the **same** action id must recompute the early marker and **not** reset the frame or the
   timer; with an explicit restart requested it must reset them. A play-only admitting update whose preserved state
   already equals the recomputed marker must answer **0**, not 1. On the play-and-move entry the same state must
   answer **1**. Three admitting-update cases on the play-and-move entry must be separated: (a) a destination
   **exactly equal** to the element's current screen position, with a movement mode other than 5, must answer
   **3** without moving **and without stepping the timer**; (b) a destination **within the arrival tolerance but
   not equal** must **not** take that path - it must step, move, and may then answer 3 through ordinary arrival;
   (c) movement mode 5 must **not** take path (a), but the test must **not** require it to answer 1 either: its own
   completion paths can answer 3 on the admitting update *(assumption, ANIM-209)*.
3. **[S]** *Early marker.* Clip B: 4 frames, holds (0, 0, 3, 0), marker 3. The stored pair must be `(2, 3)` and
   status 0 must be answered when the timer reaches 3 on frame 2 - one displayed frame before the clip's end.
   Clip C: 1 frame, hold 1 - the pair must be unreachable and status 0 never answered. Clip D: marker 0, first hold
   0 - the pair must be `(1, 0)`; with the first hold non-zero, `(0, 1)`.
4. **[S]** *Displacement gate.* Clip E: 2 frames, holds (1, 0), advances (5, 3). From a fresh action id the
   displacement per update must be 5 (the reset step, where the timer reaches 0 without the frame changing), 0, 3,
   5, 0, 3, ... A test that gates on "the frame index changed" produces 0 on the first update and fails.
5. **[S]** *One-frame clip.* Clip F: 1 frame, hold 0, advance 2, mode 0: the displacement must be 2 on **every**
   update.
6. **[S]** *Mode boundaries.* Mode 14 on a 6-frame clip must stop with the index at 4 (`n - 2`) and on a 5-frame
   clip at 4 (`n - 1`), and must never signal; entering mode 14 from **any index inside the clip, odd or even**,
   must leave it inside the clip, and entering mode 14 must not reset the frame or the timer. Mode 9 must stop at
   `n - 2`, mode 8 at `n - 1`, mode 11 must force the index to 0 on every step, and mode 10 must never advance -
   and mode 10 entered **without** an action-id change must leave the frame where it was, not at 0. Mode 3 on a
   **5**-frame clip must be refused or clamped by the implementation with a diagnostic *(assumption, ANIM-042)*;
   on a 6-frame clip it must signal at index 4. Mode 2 must signal on the step that leaves the index at `n`, and
   the implementation must clamp that state.
7. **[S]** *Stochastic idle.* Mode 4 from a fresh reset must draw **no** random value on its first update and
   exactly one per update afterwards while it sits at frame 0 with the timer 0; with a seeded stream the clip must
   start on the first draw below 131. Mode 5 with the bound 327.
8. **[S]** *Turn table.* With a **supplied** marker *(assumption, ANIM-012)*: mode 7 with a non-zero marker that
   is not the last frame, played forward into it, must rotate -2 eight times and then move to `m + 1`; mode 7 with
   marker 0, entered by a reset, must rotate nine times; both must move the target facing with the facing. With
   the marker at the **last** frame the rotation-ending step must leave the index at `n` - one past the end -
   without wrapping, and the implementation must clamp and log what the following step would otherwise read
   *(assumption, ANIM-042)*.
9. **[S]** *Freeze-all scope and ordering.* With the level's freeze-all flag set, every play call must return
   without stepping the timer, without displacement and without a completion signal, an animation element must not
   complete, and the NPC AI step must be skipped; clearing the flag must resume the clip from exactly where it
   stood. In the same state a **cart must keep moving** and an executor's **turn operation must still run**, while
   the cart's sub-sprite animations stop. A recorded freeze element must mark itself completed **before** the flag
   is written, so a sequence whose next level is dispatched in the same drain observes the flag already set; native
   139 must set it inside the call. The consequence for the next sequence level must be tested in **both**
   directions, and the distinction is **synchronous execution before the write versus queued execution after it**,
   not "this drain versus a later drain": completing the freeze element advances its sequence level, and a
   successor of a kind that the level start dispatches **synchronously** runs inside that advance, **before** the
   flag is written, so it observes the flag at its **previous** value; a successor that is **queued** runs when the
   queue is drained, which may be later in the **same** drain, and observes the written value. Both fixtures must
   state the flag's value before the element ran: assert "observes it clear" only with the flag initially clear and
   the element setting it, and "observes it set" only with the flag initially set and the element clearing it - or
   the reverse, so long as the initial value is fixed. A test that asserts the next level always sees the flag set
   is wrong.

**Actions and completion**

10. **[D]** *Cycle timing and speed.* For `RobinHood`'s walk, run and crouched-walk action ids and `Soldier A00`'s
    walk, alert walk and sprint, read the frame count and the per-frame halves from the profile and assert
    `average speed = sum(advance) / (sum(hold + 1) x cadence)` and, per update, the displacement of ANIM-032. The
    crouched walk is the important case: its holds are not all zero, so the test must assert the **per-update**
    positions, not only the average.
11. **[S]** *Wait boundary.* A wait action set to 3 must be decremented on the first three checks (counter 3, 2, 1)
    and must complete on the **fourth** check, when the counter reads 0. There is no fourth decrement.
12. **[S]** *Script animation elements.* Native 49 on a human whose profile has the action must complete at the end
    of the clip, one update later than test 1's raw trace because it is admitted through the play-only entry.
    Native 50 must never complete by itself; the test must assert that the sequence's next level never starts and
    that the element can still be refused, cancelled or replaced. Native 51 must complete at the end of the clip,
    and the freeze element must reach the actor **through the sequence queue drain**, so the test must assert that
    the original sequence's next level may start before the freeze element is admitted, and must not assert that
    the actor holds the last frame unless the freeze element was admitted and is uncontested. All three are
    established for **human** targets only *(assumption, ANIM-123)*.
13. **[S]** *Refusal and the failure counter.* A walk order whose path search fails must refuse the element (state
    5) and abort the rest of the sequence. For a blocked walk the trace must be exact: the **first** failure after
    a new action element finds the region **invalid**, so it establishes a region around the tested point and
    leaves the counter at **0**; each later failure whose tested point lies inside that region - inclusive bounds,
    half-extents about 0.49 px - increments the counter and reduces the tolerance by 0.2 while it is above 1.0; a
    failure whose point lies **outside** re-establishes the region and resets the counter and the tolerance; and
    the element is refused when the counter exceeds 50. Blocking a character for 51 updates is **not** sufficient
    to refuse it, and a restore that drops the region's validity mark must change the refusing update - assert
    that it does.
13a. **[S]** *Sliding.* A synthetic level with one straight wall. A character stepping 4 px at 45 degrees into the
    wall must end the update **4 px away from where it started** - the step keeps its length - displaced **along**
    the wall, not stopped and not shortened. Stepping **head-on** into the same wall it must move **0.95 x 4 px**
    sideways. With a round obstacle of radius `R` at distance `d` on its path, the resulting step must satisfy the
    circle solve of ANIM-244 and must be rejected - the obstacle skipped, the step unchanged by it - when the
    along-component would exceed the step length. With **two** walls forming a doorway narrower than twice the
    step, the character must resolve the **nearer** jamb first, then the second with the already-deflected step,
    and must pass through; reversing the resolution order must give a different, wrong result, which the test
    asserts is not what the engine produces.
13b. **[S]** *No rollback, and the unstick.* A character whose every deflection is rejected must end the update at
    **exactly** its starting position - not partially moved, not inside the wall. A character **placed inside** a
    wall must be pushed out perpendicular to it within the pass limit and must reappear outside; with two walls
    meeting at a corner it must take more than one pass and must still end outside both.
13c. **[S]** *The four arrival cases.* Build one fixture per case of ANIM-242: not blocked with the plain metric
    (arrival when the projection equals the tolerance exactly - inclusive); not blocked with the isometric flag
    (a destination 10 px away on the second axis must **not** arrive at a tolerance of 10, because the component
    is scaled by 1.7434); blocked with the arrival-mode flag (arrival decided by walkability to the stored goal,
    at any distance); and blocked with a non-zero failure counter (arrival at a Chebyshev distance strictly below
    10, and strictly below the radii-plus-10 bound when an order object applies). The door-approach element's
    Chebyshev bound must stay separate from all four.
14. **[S]** *Turning.* From facing 0: target 5 in 5 calls, target 11 in 5 calls the other way, target 8 downward
    (15, 14, ...) in 8 calls. The damped variant must first move on the third same-sign call and spend one call
    resetting after a reversal. The delayed variant with argument 2 must move every third call. Counts are **per
    execution of the action**, not per update, and the repeated cases have fixtures of their own. Every fixture
    that asserts a **facing change** must use the **ordinary** turn variant (the damped one consumes calls without
    moving) and must leave at least one sixteenth of angular distance per asserted change, because a turn
    operation on an already-aligned element changes nothing. With that set up: action id **294** must perform
    **two** turn/play executions even when the first answers status 3; action ids **296 to 299** must perform two
    when neither completes the action and **one** when the first answers status 3. The turn must be applied
    **before** each execution's play and displacement, so the displacement's turning scale uses the post-turn
    facing (ANIM-212).
15. **[S]** *Turning cost and stride.* While the facing differs from the target, a displacement of 4 px must become
    2.4 px and one of 1 px must become 0.7 px, and a zero displacement must stay zero. Changing the target facing
    mid-walk must not reset the frame index or the timer: the next update must show the ordinary next step with the
    new facing's animation.

**Camera**

16. **[S]** *The location conversion.* For a point `p`, a zoom `z`, a configured screen `W x H` and a map `M x N`,
    the corner must be `trunc(p - (W/(2z), H/(2z)))`, then the **conditional** bounds of ANIM-329, then made even
    on both axes at `z = 0.5`. Required cases, each tested **separately on each axis** with a **negative**, a
    **zero** and a **positive** truncated component: on a map large enough for the view, a far-inside point must
    land exactly there, a negative component must land at 0 and an over-large component must land at
    `M - W/z` (or `N - (H-80)/z`). On a map **smaller** than the view: a **negative** truncated component must
    leave the camera at `(0, 0)` **with the zoom set to 1.0**, while a **zero or positive** one must take the
    ordinary branch and produce the **negative** result `M - W/z` with the zoom unchanged. A test that substitutes
    a clamp to a non-negative interval fails the second case. Natives 18, 19, 20 and elements 6 and 7 must all show
    the same conversion.
17. **[S]** *Key scrolling.* From a fresh level, the first **right** command must move by the ramp's first non-zero
    entry and the first **left** command must move **zero** and only then begin to ramp; the same asymmetry for
    down and up. Reversing must set that axis's index to 0, so the update after a reversal must move zero. At zoom
    2.0 the same commands must move half as far in world pixels and at zoom 0.5 twice as far. Releasing must walk
    the index back down while still moving. A command must do nothing at all while a script scroll destination is
    active. The individual ramp values are an *(assumption, ANIM-323)*: assert the zero entry, the first non-zero
    entry, monotonicity and the ceiling, not the whole table.
17a. **[S]** *The ramp rule.* Generate the ramp from ANIM-323's three steps and assert the sequence 0, 6, 6, 7, 7,
    8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 26, 28, 31 and 32 thereafter. Assert that using one rounding
    for both steps - truncation for the store, or round-to-nearest for the parity test - produces a different
    sequence, so the test pins the distinction.
17b. **[S]** *Edge scrolling.* The pointer at exactly x = 0 must raise the left command and nothing else; at
    x = width - 1 the right command; at exactly (0, 0) **both** left and up in the same poll. One pixel inside any
    edge must raise nothing. Holding the pointer on an edge must raise the command on **every** poll, so the ramp
    accelerates exactly as it does for a held key. A pointer inside an exclusion rectangle must raise nothing
    *(assumption, ANIM-324)*.
17c. **[S]** *The follow.* Lock onto an actor **inside** the view: the camera must not move, and the offset the
    actor then has must be preserved for the whole lock. Lock onto an actor **outside** the view: the camera must
    centre on it, rounded, bounded and even-aligned at zoom 0.5. Then assert the four traces: a steadily walking
    actor produces bursts of 15 updates at the actor's own animation speed with no recompute in between; a stopped
    actor produces no camera motion; a reversing actor has that axis frozen while the other continues; and an
    actor beyond a map bound leaves the camera at the bound with the scroll stage still reachable.
18. **[S]** *Script scroll.* Choose an **axis-aligned**, integral destination an exact multiple of the step away,
    reachable without clipping, with **no actor lock enabled, no zoom request pending**, no camera element
    contention and a leftover scroll speed of 0. With native 42 speed 10 and a separation of 100 px the camera must
    move ten steps of 10 px and complete on the **camera update after** the one that lands - the eleventh. With
    native 33 the first step must be 2 px and the rest must follow the ramp. A scroll whose line leaves the map
    must complete at the bound instead of hanging. A non-axis-aligned or non-integral destination must not be used
    to assert an exact update count, because the per-component truncation can prevent the equality. **Contested
    cases** must be asserted separately: with an actor lock enabled and producing movement the scroll must make
    **no** progress; with that same lock's step **clipped to a zero delta** at a map bound the scroll stage must
    **run** on that update - and to assert that the camera actually **moves** then, the fixture must also give the
    scroll a non-zero effective displacement that is itself unclipped, since the fall-through establishes only
    that the stage runs; and with a zoom request outstanding that completes first, the scroll must keep
    **moving** and must terminate at its destination, but its element must already have been completed by the zoom,
    so the arrival must complete nothing.
19. **[S]** *Natives 18, 19 and 20.* Each trace below requires an isolated camera: **no actor lock enabled, no zoom
    request pending, no clipping on the steps asserted, and the destination not already reached**, so that the
    scroll stage is actually reached (ANIM-334).
    Every asserted step size additionally requires a destination that is **axis-aligned** and **far enough away
    that the remaining distance never limits the step**, because the mover shortens the last step to the remaining
    distance and truncates each component to whole pixels (ANIM-330): a 2, 7 or 10 px step is only observable while
    the remaining distance exceeds it on the asserted axis.
    After native 18 the camera must not move in that call, the zoom must be unchanged unless the conversion's
    fallback applied, and a scroll must begin with a first step of **2 px only when the leftover scroll speed is
    0**; with a leftover speed of 10 the first step must be 10 px. Nothing new waits for the scroll - but if a
    camera element was already installed, that element must complete when the scroll arrives.
    After native 19 with 7.0 the first step must be 7 px, again **only** when the leftover scroll speed is 0.
    After native 19 with **1.0** the first ramp refresh must set the index to 0, so the ramp **length** for the next
    update is the ramp's zero entry, and the refreshes after it must advance the index normally; a test that
    requires a reset on every later update is wrong. Whether the camera actually **stands still** on that update
    must be asserted only with a leftover scroll speed of 0 - a non-zero leftover speed is used instead of the
    length and keeps the camera moving.
    After native 20 the corner must equal the converted point immediately and the next frame must be fully redrawn,
    with the scroll destination, the zoom request and any actor lock untouched.
20. **[S]** *Zoom.* A request equal to the current zoom must complete on the element's first camera check; a
    refused request (already at a limit, or a map too small) must complete on the **next** check; an accepted
    request must reach the target zoom and complete, with the number of updates pinned by a recording rather than
    asserted *(assumption, ANIM-326/333)*.
21. **[S]** *Camera under a skipped tick and under a modal page.* With the level tick skipped by a transition and
    **no script destination set**, key scrolling must work. With a script destination set, the same key commands
    must do nothing at all (they are gated), while the scroll itself must continue and may complete. With a
    **modal text page open**, neither must happen: the scroll must not progress or complete and the camera must not
    move, whatever the tick state.

**Drawing**

22. **[S]** *Depth order.* Two characters on the same world row must be drawn in ascending element identity. A
    character carrying another must be ordered relative to it by the 0.001 offset and an attached effect relative
    to its owner by 0.01 - assert the **relative** order of the pair only, never that nothing sorts between them. A
    character and a building whose sort line passes between them must be ordered by the side of that line; a
    building **without** a polyline must be ordered against the character by their **screen** rows. A movable that
    fails one piece's test must not prevent later movables from being emitted before that piece.
22a. **[S]** *The sort line's cases.* A scenery piece with a three-vertex polyline rising left to right, and a
    character stepped across it. Assert each case separately: **left of** the first vertex the character is judged
    against a flat extension at the first vertex's row; **right of** the last vertex, against a flat extension at
    the last vertex's row; **between** them, against the interpolated row of the spanning segment; **exactly on**
    the line the character is drawn **in front of** the piece. A piece with **no** polyline must judge by its own
    screen row. Clearing the piece's visible mark must stop it occluding altogether.
22b. **[S]** *Roles and the scenery order.* A scenery-family element **without** a polyline must be ordered by its
    depth key like a character, not by a sort line. Two scenery pieces must be drawn in ascending order of the
    topmost row of their polylines, decided once at level start and not re-decided per frame; two pieces sharing a
    topmost row may be drawn in either order, and the test must accept both (the original's sort is unstable, and
    the engine's choice is a recorded divergence). With no scenery at all, every movable must be emitted in pure
    sort order.
23. **[S]** *Ground marks.* A mark must age by one on each drawn, visible update whose level frame counter is even,
    and must be destroyed on the **sixth** such event - that invariant, not a fixed number of draws, is what the
    test asserts. Assert also: a mark held off-screen does not age; a counter that stays even ages it on every
    draw; a counter that stays odd never ages it; and with an alternating counter and continuous visibility the
    removal falls on draw **11** when the first drawn update is even and on draw **12** when it is odd.
24. **[S]** *Snapshot and restore.* Save mid-clip (frame index 1, timer 1 of a hold-2 frame), mid-turn (a damped
    counter at -1), mid-walk (a failed-move counter of 3 **with its progress region's bounds and validity mark**),
    mid-cart (a program position inside a block, a non-zero acceleration and non-zero rattle accumulators) and
    mid-scroll (a destination, a step length of 6 and a ramp index of 4); restore and assert that the next update
    produces exactly the value it would have produced without the save, and that a restore which drops the region's
    validity mark changes the update on which the walk is refused. Then assert the cross-phase order of one tick: the
    cart pass observes the
    previous tick's positions, the element pass observes the cart pass's results, the queue drain observes the
    element pass's launches, and the script timer list is visited in insertion order.
25. **[S]** *Slow motion.* If slow motion is implemented at all (`ADR-0010` does not implement it), every
    per-update value must be unchanged and only the rate of updates may differ; the realised ratio must be measured,
    never asserted *(assumption, ANIM-520)*.

## 8. Implementation choices, snapshot, RNG and departures

**What is the original's behaviour**: the rules section 4.2 clears. Everything in 4.3 is an `Assumption`; the
fallbacks this document suggests are ways to keep going, not cleared requirements, and each must be exposed in the
assumption registry and pinned before it is relied on.

**Snapshot contract.** ANIM-020 to ANIM-023 list the state a snapshot or save must reproduce, and the items a naive
snapshot omits are: the frame timer (a reload that zeroes it shifts every animation and therefore every element
completion), the early-completion marker pair, the action id the timer was last reset for, the turn delay and
hysteresis counters, the failed-move counter **together with its collision tolerance and its progress region,
including that region's validity mark** (the region and its validity decide whether the counter resets, so
restoring the counter alone changes the update on which an element is refused), the footstep phase counter, the
current action-id lookup, the cart's program position, block remainder, speed, target speed, acceleration and
**rattle accumulators**, the level's freeze-all flag, and the camera's ramp indices, direction latches, leftover
scroll speed, lock burst counter and per-axis corrections. Established initial values are in ANIM-020 to
ANIM-023; the ones that are **not** established are two rows of 4.3 (`AnimInitialState` for the three animation
flags, the turn counters and the footstep phase; `CartInitialState` for every cart field, which has neither a
default nor an established reset on activation).

**RNG contract.** This subsystem draws from the single global stream of `spec-ai-combat.md` AI-005/AI-006 in
exactly these places:

1. the idle-to-fidget roll, one draw per idle-cycle completion signal of an actor **except** when that actor's
   current action element is a wait element, which draws nothing (ANIM-130);
2. the stochastic idle of play modes 4 and 5, one draw per step per element sitting at the start of such a clip,
   and none on the step after a reset (ANIM-040);
3. the cart's rattle, **two draws per sub-sprite per update** for every cart in case C of ANIM-301 - quantitatively
   the largest consumer here;
4. the cart program's weighted branch, **at most** one draw per block change, and none on the unweighted
   continuation (ANIM-302);
5. the building-entry waits of the walk pipeline, two draws per wait (`spec-navigation.md` NAV-130).

**The order is not given by the element update order alone.** Items 1 to 3 occur inside the per-element update
pass and follow its index order (ANIM-102); item 4 occurs there too; but item 5 occurs when a **walk sequence is
built**, which happens on a player click or inside a native call, i.e. at points outside that pass and even outside
the tick. Any claim of trace equivalence with the original must account for all five and for that ordering.

**Deliberate departures, each to be recorded as such:**

1. **A fixed logic frame of 46.875 ms** for everything counted in updates. This is the engine decision of
   `ADR-0010`, not recovered authorial intent: the program asks for 40 ms of reported elapsed time (ANIM-001) and
   the host that produced the oracle recordings delivered 46.875 ms (ANIM-002). The nominal 40 ms is documented
   and not used.
2. **Treat the looping animation element (native 50) as never completing**, as the original does, and raise a
   diagnostic when a sequence level blocks on one.
3. **Clamp and log instead of reading out of range** wherever the original is unchecked: the frame states of
   ANIM-042, the cart's waypoint index, the leftover scroll speed.
4. **Keep 16 facings everywhere.**
5. **Refuse movement mode 3 with a diagnostic** until a caller is identified (ANIM-202).
6. **Implement edge scrolling with the key scroll's commands, gating and ramp** and record the border width as an
   assumption (ANIM-324).
7. **Reproduce difficulty scaling** through `spec-ai-combat.md` AI-045, including its hostile-side guard; do not
   omit it, and do not let it touch anything in this specification (ANIM-523).
8. **Slow motion is not implemented** (`ADR-0010`), so ANIM-520's unsettled ratio never arises in the engine.

## 9. Validation against the data and the recordings

Checks run with `harness/tools/probe/anim_actions.py --table` on the read-only copy at
`C:\Users\przem\source\gamedata\robinhood\DATA\Characters`. Only aggregates and relations are quoted; the
per-frame tables stay in the analyst workspace.

- **Bonus items - the direct per-frame check.** Every bonus-item profile checked (`BONUS_Ale`, `BONUS_Arrows`) has
  five blocks of 16 frames whose hold values are all 1. Under ANIM-031 each frame is displayed for exactly 2 timer
  steps, so a cycle is 32 steps with 16 frame changes. The oracle measured a **uniform 93.75 ms** between changes
  and 16 changes per **1.500 s** (`stealth-and-combat.md` 8.4 as corrected by `combat-measurements.md`, which
  reassigns that measurement from "a soldier idle" to a **pickup sparkle**; the correction is carried here).
  `2 x 46.875 = 93.75` and `32 x 46.875 = 1500` exactly. This is a per-frame confirmation of the `hold + 1` rule
  and of the reference cadence.
- **The hero's crouched walk.** Its holds sum to 18 over 14 frames and its advances sum to 27 px, so ANIM-031 gives
  a cycle of `18 + 14 = 32` steps = 1.500 s and an average of 18.0 px/s; the recording measured 1.50 s and
  17.8 px/s. The competing "one step per hold value, at least one" reading gives 0.84 s and fails.
- **The hero's walk.** 22 frames, every hold 0, every advance 4 px: 22 steps per cycle = 1.031 s and 85.33 px/s.
  The recording gives a **stride period of 1.044 s** and 85.3 px/s. That is a *visual stride period*, not a count
  of logged engine updates, so it supports the cadence to within its own uncertainty (about 1.4 %) and excludes
  40 ms (which would give 0.88 s); it does not measure the counter's granularity.
- Further profiles agree with `docs/formats/sprite-animations.md` rule 3, which derived the same speeds from the
  same files: the hero's run (12 frames of 5 px) 106.67 px/s; `Soldier A00`'s walk (22 of 2) 42.67 px/s, alert walk
  (22 of 3) 64 px/s, alert run (12 of 4) 85.33 px/s, sprint (32 of 5) 106.67 px/s.
- The marker-field candidate of ANIM-012 equals `frame count - 1` on every block checked here, consistent with the
  112 608 of 148 512 animations counted by `sprite-animations.md`; the identification remains unconfirmed (4.3).
- The climb blocks are 12 frames with holds summing to 16 and advances of plus or minus 3 px, with a per-block
  displacement of 45 px. The revision-1 acceptance case that quoted "45 versus 36 px" as if both were displacements
  was wrong and was withdrawn in revision 2: the 45 px is the block displacement field, the 36 px is the sum of the
  advances of one repetition, and no test asserts a relation between them.
- **What these measurements do not establish**: the counter granularity itself; the slow-motion ratio; an exact
  acceleration trace for the camera ramp; the zoom transition's duration; the follow controller's behaviour; and
  any individual displacement event. Those need new recordings (section 10).
- No contradiction was found between the code that was read and the shipped animation tables.

## 10. Open questions

Each names what to read next and corresponds to a row of 4.3.

1. **The corridor rule** (ANIM-244, ANIM-247, ANIM-242 case 3): 004f6c20, the passability test the deflection, the
   arrival test and the walk pipeline all call. It is now the largest routine still unread in this subsystem, and
   it is where any door-specific rule would live. The deflection itself, the absence of rollback, the unstick and
   the four arrival cases are settled (ANIM-244 to ANIM-247).
2. **The stopping tolerance** (ANIM-242): what writes the element's tolerance. It is not written by the mover, the
   arrival test or the play call; try the walk order construction (00582640) and 0055fc20.
2a. **The proximity reaction** (ANIM-240): the per-class behaviour a qualifying partner's callback performs -
   pushing, waiting, stepping aside - and whether it can move either element within the update.
3. **The speech duration** (ANIM-140): the speech case of 00464b20, the sound length source around 005a87f0, and
   the fallback with sound disabled.
4. **The marker and loop fields** (ANIM-012, ANIM-013): the mapping from the in-memory record read by 005bdcd0 and
   005bddb0 to the file fields, through the sequence reader reached from 005b5ea0.
5. **The animation override list** (ANIM-014b): what an entry carries and who populates it (005c8e40 and the
   element's script-side list).
7. **The per-kind admission tests** (ANIM-122): 0046abd0's classification feeds tests reached through 0046b210.
8. **The cart** (ANIM-301, ANIM-303): the two conditions that select the sub-sprite cases in 004ab7e0, the two
   unread instructions (004af0f0, 004af060), the consumer of the per-sub-sprite reciprocal, and the program's
   container in the mission file (004ae470 reads it).
9. **The cart hit's severity selector** (ANIM-305): the comparison in 004d9420 and the helper it calls.
10. **The edge-scroll exclusion rectangles** (ANIM-324): who fills the list of rectangles that suppress an
    individual edge command; start at the owner of the input poll reached from 0050f710.
11. **The zoom transition** (ANIM-326, ANIM-333): at which step of 004cf610 / 004cfce0 the current zoom changes.
12. **The configured screen size** (ANIM-340, ANIM-380): the display-configuration fields the visibility
    rectangle and the centring read were identified by use; find their writer (0051ba00).
13. **The family word and the masked effect class** (ANIM-363, ANIM-366, ANIM-368): the encoding of the element
    family bits beyond the three-way drawing split; whether the drawable effect class whose serialised form carries
    one extra value consumes a mask's byte blob when it draws (004aaf80, then the effect draw path); and whether
    the second, family-priority comparison at 00549ae0 is reachable at all (0054b1f0 has no traced caller). The
    sort line, the load-time scenery order and the mask records' role are settled (ANIM-363, ANIM-366).
16. **The level-owned timed list** of ANIM-101.6: what it holds and what "finished" means for its entries.
17. **The remaining initial values**: the meaning and the construction-time value of the three animation flag
    bytes of ANIM-020 (005b5890 does not write them); and every cart field of ANIM-023, whose construction and
    activation were not read (004ad5f0, 004ae470, the cart natives 67 / 72). Everything else in ANIM-020 to
    ANIM-023 is now established at 005b5890 and 0055eee0.
18. **The two wide resolutions** (ANIM-380): 005e3f70 and the mode enumeration near 005e3b10.
19. **Recordings needed**: a zoom element trace, a follow trace
    for a visible and an off-screen actor, a cart run (to pin the rattle draws' effect on later rolls), and a
    ground-mark trace across a scroll that takes a mark off-screen.

## 11. Differences from the current engine

Against `crates/opensherwood-core/src/anim.rs`, `world.rs`, `crates/opensherwood-render/src/lib.rs`,
`crates/opensherwood-app/src/engine.rs`, `docs/formats/sprite-animations.md` and
`docs/original/stealth-and-combat.md` 8, as they stand **after rebuild batch 1** (`ef45066`, ruleset 19). A
different internal organisation is not itself a difference; each item is a different observable result.

1. **The clock is no longer a difference** (historical). Before batch 1, `anim.rs` ran a 60 Hz world tick with 16
   units per tick and 45 per table tick to approximate a 64 Hz animation clock, and kept a fractional remainder.
   `ADR-0010` and batch 1 replaced that with the single 46.875 ms logic frame this document describes, so the
   three-clock model and its conversions are gone. What remains for the movement batch is to count **timer steps
   per play call and play calls per update** as ANIM-003 and ANIM-212 state them, rather than one step per frame.
2. **Movement is not a constant average speed.** `AnimSet::cycle_speed` moves the entity at the cycle's average;
   the original displaces the current frame's own advance on the steps where the timer is zero afterwards and
   nothing on the others (ANIM-032). The two agree for the uniform walk and run cycles and disagree for the sneak,
   the climbs, the decelerating stops and every action with non-zero holds - visibly, as stepping motion, and
   observably, as different arrival updates.
3. **Eight directions instead of sixteen** (`direction_of`, the per-action `[u32; 8]` sets). The original indexes a
   block of 16 by the facing (ANIM-010) and turns one sixteenth per call (ANIM-210).
4. **Turning has a rule and a cost**: the shorter arc, ties counter-clockwise, four variants including a damped
   one, the 0.6 factor with the 0.7 px floor, and no restart of the clip (ANIM-203, ANIM-210 to ANIM-215). The
   engine turns instantly.
5. **Animation elements complete on real conditions**: 49 at the end of the clip, 50 **never**, 51 at the end plus
   a freeze element that arrives through the sequence queue (3.4). The engine completes all three at once.
6. **A camera point is centred, truncated, bounded and aligned** (ANIM-329) - the engine has no such conversion, so
   every scripted camera position lands somewhere else; and on a map smaller than the view the conversion changes
   the zoom.
7. **The camera's scroll step is a ramp with a zero first entry and an asymmetric start** (ANIM-323); `world.rs`
   scrolls by a constant 8 px per key update. `world.rs` clamps against its own configurable viewport, which is
   the right shape but the wrong rectangle: the 80 px HUD strip is missing (ANIM-380).
8. **Natives 18 and 19 request a scroll and set its step length; native 20 is the jump** (ANIM-331, ANIM-332), and
   18/19 leave a running camera element in place for the arrival to complete.
9. **Zoom has three values, is requested rather than set, and is rendered over eight steps** (ANIM-325, ANIM-326);
   the zoom element completes on a later update with three distinct cases (ANIM-333). The engine treats zoom as
   instant.
10. **The camera lock matches the followed actor's own speed in 15-update bursts and can move the camera when it is
    set** (ANIM-340); the engine has no equivalent.
11. **Occlusion is ordering.** `opensherwood-render` hides characters by clipping them against occluder masks with a
    depth line. The original produces one merged list per frame: movables sorted by **world row** with an identity
    tie-break, merged into the level's scenery order by a per-scenery sort line whose absent-polyline fallback
    compares **screen rows** (ANIM-363, ANIM-364). The engine's depth line is the right idea in the wrong place.
12. **Effects and decorations are ordered into the same list** through a deferred queue with 0.001 / 0.01 / 1000.1
    offsets (ANIM-364, ANIM-365); the engine draws them in separate layers.
13. **Animated destination and trail marks are missing.** The engine already draws target lines and selection
    markers; what it lacks is the original's **animated** ground marks with their six ageing events, their
    even-counter condition and their unbounded count (ANIM-370).
14. **No dirty rectangles, 688 px of world, no letterboxing** (ANIM-361, ANIM-380), and a zoom transition that does
    not run the ordinary pass list at all.
15. **The per-element update order is the element table's index order with the count re-read each step, and a
    removal skips the element that shifts into place** (ANIM-102); the engine iterates a stable snapshot.
16. **Carts are speed-driven with a scripted program, a conditional weighted branch and a per-sub-sprite random
    rattle** (3.10), and they injure people through a pass that runs **before** the element updates (ANIM-305).
17. **A freeze-all flag stops the animation player's stepping, displacement and completion signals and the NPC
    AI step, and nothing else** - carts keep rolling, executors keep turning, the tick, camera and drawing keep
    running (ANIM-005); the engine has no such gate, and without it a scripted freeze either does nothing or stops
    too much. Its recorded form also completes its element **before** writing the flag.
18. **Difficulty exists, is hostile-side only and is capped** (ANIM-523, deferring to `spec-ai-combat.md` AI-045).

## 12. Sibling specifications: outstanding amendments

Nothing here is marked "absorbed". Every item below was checked against the **commits pinned in the header**; where
a working copy already carries a correction, that is not recorded as absorption, because the pinned revision is
what a reader of this document will fetch. The owner of each document decides whether to apply the amendment or to
re-pin a revision that already contains it.

**`spec-script-vm.md` (pinned revision 4, `afdfaec`):**

- **VM-231** states that a walk arrives within `radius + 5`. That is the **door-approach element's** rule and it is
  a **Chebyshev** comparison, strictly less than `tolerance + 5` (ANIM-242); the ordinary move action arrives on a
  **directional projection** against the element's own tolerance, inclusive. The two must be separated.
- **VM-231** does not state native 51's **deferred** freeze admission: the freeze element is launched before the
  animation element reports completion, but it reaches the actor through the sequence queue drain, so the original
  sequence's next level can start first and the freeze is itself subject to admission and replacement (3.4).
- **VM-219**'s small-map fallback omits its condition. The fallback fires only when that axis's **truncated
  component was negative**; a zero or positive component on an undersized map takes the ordinary branch and yields
  a negative corner with no fallback (ANIM-329).
- **VM-218**'s freeze-all row says only "at once". It should point at ANIM-005 for what the flag then gates, and
  record that the recorded element is **marked completed before the flag is written**, while native 139 writes it
  immediately.

**`spec-ai-combat.md` (pinned revision 2, `e966b05`):**

- **AI-045**'s note that it "contradicts ANIM-523" is stale: revision 2 of this document withdrew the denial and
  revisions 3 and 4 state the helper's shape in AI-045's own terms.
- Add the **freeze-all gate** (ANIM-005) where the AI's own update is described, with its limits: the NPC update
  skips its AI step and no animation advances, but cart displacement, executor-side turning and the tick phases do
  not stop.
- The **RNG ordering** statement should carry the two draws per cart sub-sprite per update (only in the rattling
  case), the **at most** one draw per cart block change, and the fact that the walk pipeline's waits are drawn
  outside the element-update pass (section 8 here).

**`spec-navigation.md` (pinned revision 2, `e5a2e0c`):**

- **NAV-150** describes the proximity query as intersecting boxes and cites movement claim ids that this document
  no longer uses. The comparison is a **directional projection** of the partner displacement onto the mover's
  **normalised** movement direction, at least 5, and it applies to **character candidates only** - a target-family
  candidate is admitted by its own "is solid" answer and the containment test alone. The guard list is: not the
  mover, displayed, not the carried element, **same layer and sector** (there is no projection-area comparison),
  then an **inclusive** containment test of the partner's position in the mover's query box (ANIM-240). The current
  claim ids are ANIM-200/201/203/204, ANIM-208, ANIM-240 to ANIM-243 and ANIM-311.
- NAV-150 may also cite ANIM-241's **two** resets and the progress region's validity mark, and ANIM-242 for the
  mover's unread arrival branches.

**`spec-navigation.md`, additionally**: NAV-210 may now record that the level's **mask records are the perception
geometry and never reach the drawing** (ANIM-366), and NAV-111 that the same unstick routine is also the move's
last resort, with the backoff and inflation ANIM-246 states.

**All three**: this document's camera claim ids are unchanged since revision 3 except that the interaction
precedence is the new **ANIM-334**; the location conversion remains **ANIM-329**, and citations of ANIM-320, 322,
325, 326, 330 to 333 and 340 remain valid.

## 13. Provenance

- Ghidra project `re/ghidra/robinhood` (never committed); exported decompilation `re/out/decomp_all/<address>.c`,
  function inventory `re/out/inventory.tsv`, string references `re/out/strings.tsv`, module map
  `re/notes/modules.txt`, all produced by the committed export scripts under `scripts/ghidra/` and all git-ignored.
  Data bytes read with `scripts/ghidra/peek.py`. Confirmed against the raw instructions with a local capstone
  disassembly helper kept in the analyst workspace: the displacement gate (ANIM-032), the mode-3 arithmetic
  (ANIM-202), the location conversion's centring, truncation, bounds and alignment (ANIM-329), the destination of
  every write natives 18, 19 and 20 perform (ANIM-331, ANIM-332), the key-scroll latch polarity (ANIM-323), the
  difficulty helper's hostile guard and cap (ANIM-523) and the proximity comparison's operands (ANIM-240). The
  camera lock's threshold (ANIM-340) was corrected in revision 3 by reading the constant as the 8-byte value the
  instructions use; revision 2's "0.0" came from reading its first four bytes only. Revision 4 added, by reading
  the routines named in each claim: the immediate-completion condition's exact-equality predicate (ANIM-035), the
  normalisation of the direction the proximity product uses (ANIM-240), the progress region's validity mark,
  inclusive containment and invalidation on a new action (ANIM-241), the freeze element's completion-before-write
  ordering and the operations the flag does **not** gate (ANIM-005), the camera routine's lock-zoom-scroll
  precedence and early exits (ANIM-334), and the constructor's clearing of **both** direction latches, which
  withdrew revision 3's claim that the x latch was uninitialised (ANIM-023, ANIM-323). Revision 5 added, the same
  way: the idle transition's next-update timing and its wait-element exclusion (ANIM-130), the two-iteration
  turn-and-play loop of action ids 296 to 299 with its stop-on-completion rule (ANIM-212), movement mode 5's two
  observed completion triggers (ANIM-209), the withdrawal of the projection-area guard and the restriction of the
  directional product to character candidates (ANIM-240), and the fall-through from the camera's lock stage when a
  clipped burst step leaves a zero delta (ANIM-334).
- Functions read: section 0. Notes: `re/notes/anim/` (git-ignored).
- Data checked: `harness/tools/probe/anim_actions.py --table` on the character profiles of the read-only game copy
  at `C:\Users\przem\source\gamedata\robinhood` (section 9). No oracle run in this session; the timing comparisons
  use the recordings of 2026-09-05 as reported in `docs/original/stealth-and-combat.md` 8 **with** the correction in
  `docs/original/combat-measurements.md`.
- Pinned siblings and the engine decision: the header. Formats: `docs/formats/sprites.md`,
  `docs/formats/sprite-animations.md`.
- Review history: revision 1 (blob `17ff4e2c6e84326e82552e0afc0c2ef8fc3082b3`) - Codex review 18, 28 findings,
  *redo*. Revision 2 (commit `b0cd053`, blob `f9a8c498eee7d4359e0159de459f688574841439`) - Codex review 24, 22
  findings, *fix-then-clear*. Revision 3 (commit `03b413e`, blob
  `1b65f0bafaa59f6ac4dd704fe7a0c6e4d841df03`) - Codex review 30, 14 findings, *fix-then-clear*, which cleared
  ANIM-329's ordered conversion, the gated player and NPC operations, the play-only status precedence, the
  existence of both failure-counter resets and ANIM-340's thresholds. This revision 4 answers all 14; the
  clearances are marked in 4.2, the exclusions are in 4.3, and the disputed points are in the report that
  accompanies it. Revision 4 (commit `7328461`, blob `06d0fad5b45fe7106119dbf530e45e5338e89b16`) - Codex review 38,
  9 findings, *fix-then-clear* with a per-area clearance table; this revision 5 answers all 9.
- Implementation status: rebuild batch 1 (commit `ef45066`, ruleset 19) runs the engine on one logic frame of
  46.875 ms (`ADR-0010`) with the script VM built from its own cleared specification. The claims here are the
  contract for the movement, animation and camera batch that follows; every 4.3 row must reach that batch as an
  `Assumption`.
- Tests that will depend on this document: the animation and movement rebuild, the camera, the draw order and the
  acceptance cases of section 7 (roadmap item "movement and camera", ADR-0009).
