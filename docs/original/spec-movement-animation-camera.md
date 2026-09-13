# Movement, animation and camera (behaviour specification)

Status: `draft` (analyst session 2026-09-13, awaiting Codex spec review). Build: GOG English edition,
`Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base
`0x00400000`; every address below is a virtual address in that image. Analyst: 2026-09-13, this session
(analyst role, ADR-0009). Reviewer: pending. Publication approval: pending (separate from factual approval).

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text, and no prescribed internal
structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer chooses
the organisation.

Claim ids are `ANIM-nnn`. Status is `observed` (read in the program's code at the named address), `inferred`
(the only reading that fits every branch, or concluded from several observed facts), `unknown`. Confidence is
high unless stated. Sibling specifications, whose claim ids are referenced instead of repeated:
`spec-script-vm.md` (`VM-nnn`, the level tick and the sequence machinery), `spec-navigation.md` (`NAV-nnn`,
layers, sectors, doors, the path finder, the walk order pipeline), `spec-ai-combat.md` (`AI-nnn`, the clock and
the random stream, combat decisions).

## Identity and exposure

- **Analyst**: this session, 2026-09-13, analyst role under ADR-0009. It has read decompiled code of the
  animation player, the actor element executors, the movement and collision entry points, the cart, the camera,
  the drawing order and the settings. It must not implement any of them, and no implementer session may inherit
  its context, notes or tool output.
- **Delegated readers**: two assistant sessions of the same role and the same workspace contributed the camera
  findings (3.10) and the drawing-order and settings findings (3.11, 3.12). Their exposure counts as this
  session's; their raw output never entered the repository and their wording is not reproduced here.
- **Spec reviewer**: pending (Codex, `cross-agent-review` task B). A reviewer may read `re/`; its output is
  corrections to this file only.
- **Implementation reviewer**: pending, must be a session that has never read `re/`.
- **Publication approval**: pending, separate from factual approval.

## 0. Necessity record

**Interoperability target.** Playing the player's own missions with the player's own sprite files: the mission
and map files place characters, carts and objects and address their animations by *action id*; the compiled
mission scripts (`.scb`) drive them through sequence elements (walks, animations, speech, camera moves, zoom)
and wait for those elements to complete; the character profiles (`DATA/Characters/*.rhs`) carry the frame
timings and the per-frame advances that decide both how fast a frame is shown and how far a character moves.
Nothing plays correctly unless the engine advances animations on the same clock the original uses, applies the
same per-frame displacement, completes elements at the same moment and composes the frame in the same order.

**Information data observation and black-box testing could not settle.** The existing documents state it
plainly: `docs/formats/sprite-animations.md` had to *infer* the frame clock from two oracle measurements and
offered two competing readings of the zero timing half on moving frames; `docs/original/stealth-and-combat.md`
section 8 measured "about 64 Hz, three clocks per animation frame" without being able to say what the three
clocks are; `spec-script-vm.md` open question 6.1 lists the completion rule of every actor-side sequence
element (walk arrival, animation end, speech end) as unknown, which leaves every scripted sequence unable to
advance; `spec-navigation.md` NAV-150/151 had the movement advance and the turning factors but not the clock
they are applied on, nor the collision rule; the camera natives 18/19/20/21/33/34/35/39/40/42 were guessed by
the current engine (`spec-script-vm.md` difference 11); no observation could produce the drawing order or the
depth key. All of that lives only in the executable.

**Scope read** (about 95 functions; the reasons are the questions above):

| Functions | Why |
|---|---|
| 0050f710 (main loop), 004c6ef0 (level tick), 004d9420, 004de170 | the frame, the wait, the per-element update order, the pairwise cart/human test |
| 005b86b0 (move and play), 005b8050 (play), 005b7820 (frame timer), 005b7300, 005b7720, 005b7f60, 005bd450, 005bd560, 005bd5b0, 005bdbf0, 005bdc20, 005bdcd0, 005bdcf0, 005bddb0, 005b5e00, 005b5ed0 (save), 005b6790 (placement) | the animation clock, the play modes, the per-frame advance, the action-id lookup, the saved state |
| 0055f0f0, 0055f140, 0055f1a0, 0055f210, 0055fc20, 0055fa70, 0055fe10 (save), 0055f290 | facings, turning, the projection plane, the saved position record |
| 00464230, 00471b00, 00464b20, 00467a50, 004646e0, 0046abd0, 0046b210, 0046bcf0, 0046bd40, 0046dae0, 0048d510, 00470390, 00475bd0, 00462730, 004645f0 | the actor's per-frame update, the element executors, the completion codes |
| 00585320, 00585970, 00587160, 005866a0, 00586ed0, 0058a940 | element states, element parameters, the elements the executors create |
| 004ac350, 004ad5f0, 004ae470, 004af540 | the cart (mobile element): its instruction stream, speeds and bond crossing |
| 00561040 (head), 00563e90, 00560780, 00560460, 005c2ec0 (call site) | the collision-aware move, the anticollision failure, footstep effects |
| the camera and drawing functions named in sections 5 and 6 | the camera natives and state; the per-frame drawing order, the depth key, ground marks, the viewport |

**Stopping condition.** Reading stopped when (a) the animation clock, the frame-timer state machine and every
play mode were settled, (b) the completion condition of each actor-side sequence element kind of
`spec-script-vm.md` VM-230 was either settled or named as open, (c) the per-frame displacement rule was
settled against the measured walk, run and sneak speeds, (d) the camera natives and the drawing order were
described to the level the script elements need. The interior of the sliding routine (00561040, 11 kB), the
cart instruction set and the perception geometry were deliberately left at the level of "what the caller
requires" and are listed in section 9.

**Analyst authorisation.** On behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

Covered: the frame and the clocks; the animation player (action ids, the 16 facings, the frame timer, the
fourteen play modes, loops, ping-pong, freezing, completion signals); what a script's animation, walk, turn,
speech and action elements do on the actor and when they complete; per-frame displacement, turning, collision,
pushing and the cart; the camera (state, scrolling, zoom, follow, the natives); the composition of one drawn
frame (order, depth key, occluders, ground marks, effects, the viewport); the settings that change any of it.

Taken from other subsystems: the level tick and the sequence machinery (`spec-script-vm.md`), the path finder,
the walk order pipeline, sectors, layers, doors and lifts (`spec-navigation.md`), the AI's choice of gait and
action and the random stream (`spec-ai-combat.md`), the sprite container and animation table layout
(`docs/formats/sprites.md`, `docs/formats/sprite-animations.md`).

Handed to them: the completion of every actor-side sequence element (the VM's open question 6.1), the
movement speed per gait, the clock every AI timer counts on.

## 2. Data model

### 2.1 Clocks and units

- **ANIM-001** (observed, 0050f710; high). There is exactly one clock. The main loop runs one iteration per
  displayed frame; at the top of the iteration it samples the operating system's millisecond counter and stores
  it as the frame's start time; at the bottom it busy-waits, re-reading that counter, until at least **40**
  elapsed milliseconds are reported (**400** in the slow-motion mode of ANIM-520). The level tick, every
  animation step, every movement step and the drawing all happen once per such iteration. There is no
  interpolation, no catch-up and no fixed-step accumulator: a frame that takes longer than the wait simply
  lasts longer, and everything in it advances by exactly one step.
- **ANIM-002** (observed, 0050f710; high, with an inferred consequence). The wait compares against the
  *reported* elapsed time, so the realised frame length is the smallest multiple of the counter's granularity
  that is at least 40 ms. On the granularity Windows uses by default (15.625 ms) that is three counter ticks =
  **46.875 ms** (21.333 frames per second), which is what the oracle measured (`stealth-and-combat.md` 8: a
  walking frame 46.9 ms, an idle step 93.75 ms = two frames). On a host whose counter granularity divides 40 ms
  the same code would produce 40 ms. The original's speed therefore depends on the host; the value the game was
  played at, and the one every duration in the shipped data was authored against, is 46.875 ms. Two flags
  disable the wait entirely (ANIM-521), in which case the game runs as fast as the machine allows and every
  timing in this document scales with it.
- **ANIM-003** (inferred from ANIM-001/002 and 005b7820; high). **The animation clock is the frame.** One
  animation-frame timer step per element per executed frame; the "table tick" of
  `docs/formats/sprite-animations.md` rule 2 *is* one frame, and the "64 Hz clock with three clocks per frame"
  of `stealth-and-combat.md` 8 is that document measuring the 15.625 ms counter granularity of ANIM-002, not a
  clock the program keeps. Nothing in the program divides or multiplies the animation rate by anything other
  than the play mode (3.3) and the per-element float factor of ANIM-206.
- **ANIM-004** (observed, 004c6ef0, 0050f710; high). Everything in this specification advances exactly when the
  level tick runs and is suspended exactly when it is suspended (`spec-script-vm.md` VM-101: pause, an open
  modal page, a state that leaves the level): animations freeze, characters stop mid-stride, the camera stops.
  Drawing continues while suspended.
- Units: positions, displacements and camera coordinates are **background pixels** ("map pixels") in the screen
  projection of `spec-navigation.md` NAV-001/002; a character's screen row is its world row minus its height.
  Run-time positions are IEEE single floats; the animation table's per-frame advance is a signed 16-bit integer
  number of pixels; facings are integers 0..15. "Frame" below always means one executed main-loop iteration.

### 2.2 The animation table as the player uses it (per sprite sequence)

The file layout is `docs/formats/sprites.md` and `sprite-animations.md`; this is what the player reads from it.

- **ANIM-010** (observed, 005b5e00, 005b8050, 005b86b0; high). An element plays an **action id**, never an
  animation index. The sequence carries a lookup from action id to the index of the *first animation of a block
  of 16*; an entry of `-1` means the profile has no such action, which is reported as an error and the play call
  fails without changing anything. The animation actually played is `block index + facing`, with the facing
  0..15 exactly as `sprite-animations.md` "Direction order" (0 = screen-up, clockwise). Two lookups exist per
  element (the normal table and a replacement table, ANIM-014); a flag selects which one is current.
- **ANIM-011** (observed, 005bdbf0, 005bdc20, 005bdcf0; high). Per animation the player uses: a per-frame
  **hold** value (unsigned 16-bit, the low half of the frame's timing word), a per-frame **advance** value
  (signed 16-bit, the high half of the same word), the **frame count** (the player derives it from the length of
  the hold list, and answers 0 when the list is absent), one 16-bit field used as a **marker frame index**
  (ANIM-012) and one used as a **loop length** (ANIM-013). Frame indices are 0-based.
- **ANIM-012** (observed, 005bdcd0, 005b7300 mode 6, 005b7820 mode 7; medium). One 16-bit field of the
  animation record is read as a frame index: play mode 6 starts the animation at that frame instead of 0, and
  play mode 7 (the on-the-spot turn) treats reaching that frame as the moment to rotate. It is a candidate for
  `Animation::unknown_0x02` of `sprite-animations.md` ("= frames - 1 in 112 608 of 148 512 animations"); which
  field of the file record it is was not cross-checked, so an implementer must confirm that before relying on
  modes 6 and 7.
- **ANIM-013** (observed, 005bddb0; high). A second 16-bit field of the animation record is read, by action id,
  as a **repetition count for a loop**; this is the "length taken from the animation table" that
  `spec-navigation.md` NAV-200 reports for the ladder and ivy climb loops.
- **ANIM-014** (observed, 005b86b0, 005b8050, 005b5e00; high). Two independent redirections exist, both driven
  by the script: (a) natives 60/61 (`spec-script-vm.md` element kinds 0xAA/0xAB) switch the element to a second
  action-id table in which one action id has been replaced by another, and back; (b) an element may carry a list
  of *scripted animation overrides*: before playing, the requested action id is looked up in that list and, when
  found, the entry's own id is played instead. The search is linear over the list, first match wins, and a flag
  records whether the id was found. Both act on the action id, before the block lookup of ANIM-010.

### 2.3 State an element keeps for animation and movement

Described as required state, not as a layout. A save or snapshot must carry all of it; the on-disk order
belongs to the save specification (the routines that write it are 005b5ed0 for the animation part and 0055fe10
for the position part).

- **ANIM-020** (observed, 005b5ed0; high). Animation state: the **current animation index** (16-bit, i.e. block
  plus facing: redundant with the action id and the facing, but it is the value the program keeps and restores),
  the **current frame index** (16-bit), the **frame timer** (16-bit; see ANIM-030 for its initial value of minus
  one), the **current action id**, the **action id the frame timer was last reset for**, the **end-of-clip frame
  and end-of-clip timer** pair of ANIM-034, the **table-replacement flag**, the **depth sort key** (float,
  ANIM-402), a reference to the **action element** currently being played, and three further flag bytes.
- **ANIM-021** (observed, 0055fe10, 0055fa70, 0055fc20; high). Position state: the screen position, the world
  position and height, the **previous position** (the position at the start of the current frame, ANIM-201), the
  **facing** (0..15), the **target facing** (0..15), a **turn delay counter** and a **turn hysteresis counter**
  (signed 8-bit each, ANIM-210/211), the layer and sector word, the projection area, the **ground kind** byte,
  the movement direction vector with its height component, a **reverse** flag (mirrors the facing by 8,
  `spec-navigation.md` NAV-002), a **no-collision** flag, an **off-map** flag and the visibility flags.
- **ANIM-022** (observed, 00464230, 0046bcf0, 0046bd40; high). Per actor: the queue of **action elements** with
  the current one, the **action id last reported to the script** (283 = none, `spec-script-vm.md` VM-107), the
  **status of the last executor run** (ANIM-120), the countdown used by the wait action (ANIM-133) and the
  **finish code** of the last action that ended (ANIM-121).

## 3. Behaviour

### 3.1 One frame

- **ANIM-100** (observed, 0050f710, 005105d0; high). Order inside one main-loop iteration while a mission is
  being played: (1) sample the frame start time; (2) pump the operating system's message queue and read the input
  devices, accumulating one frame's worth of mouse and keyboard state and dispatching the commands it produced;
  (3) unless suspended, run the **level tick** (`spec-script-vm.md` VM-103), which contains every animation and
  movement step of this frame; (4) update the HUD widgets' state; (5) **draw the frame** (section 6), which begins
  by moving the camera (section 5); (6) update the sound; (7) busy-wait to the minimum frame length (ANIM-001);
  (8) hide the cursor and pump messages again. The in-mission game state is the one for which the loop's
  state switch has no case, so the branches of that switch (which end in the briefing, dialogue and campaign-map
  presentations) are not part of a mission frame.
- **ANIM-101** (observed, 004c6ef0; high). Inside the level tick, the steps that concern this specification, in
  this order after the script steps of VM-103: (a) the **per-element update**, a loop over the level's element
  table from index 0 upwards, calling each element's update; the element count is re-read on every iteration, so
  elements appended during the pass are updated in the same pass, and an element whose update answers "finished"
  is removed from the level immediately (which shifts the indices of those after it); (b) the sequence manager's
  queue drain (VM-215); (c) an optional global refresh; (d) the level's waiting-element list, walked from its
  last entry to its first (VM-221); (e) the pairwise cart/character test of ANIM-305.
- **ANIM-102** (observed, 00464230, 00471b00; high). One actor's update, in order: (1) copy the current position
  into the previous position and apply any pending placement; (2) if the element has a queued action element,
  make the first one current, else current = none; (3) if the current action's element id differs from the one
  recorded at the previous frame, mark the element as freshly started; (4) run the **element executor** for the
  current action element, which turns the body (3.6), steps the animation (3.2) and moves the character (3.5),
  and answers a **status** (ANIM-120); (5) act on the status: finished implies the action element is completed
  and popped and the sequence is told (`spec-script-vm.md` VM-211), refused implies the element is refused
  (VM-217); (6) if the current action id differs from the one last reported, run the script callback
  `ActionChange(current, previous)` with this actor as the current actor, and record the new id; when there is no
  current action the reported id is 283.
- **ANIM-103** (observed, 00471b00; high). Some per-actor bookkeeping is *staggered*: it runs only on the frames
  where the level's frame counter, combined with the low 5 bits of the element's own identity, is a multiple of
  64. An implementer must keep such rules on the same phase (element identity, not table position) or the
  regeneration rates change. The one read here recovers a fatigue-like counter by a tenth of a cap while the
  actor has not moved this frame; its meaning belongs to `spec-ai-combat.md`.

### 3.2 The animation frame timer

One state machine, one step per frame, driven by the current **play mode**. State: the animation index, the
frame index `f`, the frame timer `t`. `hold(f)` and `advance(f)` are ANIM-011; `n` is the frame count.

- **ANIM-030** (observed, 005b7300, 005bd5b0; high). The timer is reset when the **action id changes** (not when
  the facing changes: turning keeps the frame and the timer, so a character that turns while walking keeps its
  stride). A reset sets `t := -1` and, depending on the play mode, `f := 0` (most modes), `f := the marker frame`
  (mode 6), `f := n - 1` (modes 12 and 13), or `t := 0, f := 0` (mode 10). A separate explicit restart sets
  `f := 0, t := -1` or `f := n - 1, t := -1`. The value **-1** is what makes the first step of a freshly started
  clip land on `t = 0`, which is exactly the state a frame is in when it becomes current in the steady flow: the
  first frame therefore gets its full `hold + 1` displays like any other. Mode 10, which resets to `t = 0`, is the
  one case where the first frame is displayed once less.
- **ANIM-031** (observed, 005b7820; high). The step of the ordinary forward modes is: `t := t + 1`; if
  `hold(f) < t` (unsigned comparison) then `f := f + 1` (or `+ 2`, see the mode table) and `t := 0`; then wrap or
  clamp `f` as the mode says. A frame is therefore displayed for `hold(f) + 1` frames in the steady state, which
  is `docs/formats/sprite-animations.md` rule 2 with the table tick = one frame (ANIM-003), and a frame whose
  hold value is 0 changes every frame.
- **ANIM-032** (observed, 005b86b0; high). **The per-frame displacement is taken only on the frames where the
  frame index actually changed** (i.e. where the step reset `t` to 0). On any other frame the displacement is
  zero. Consequently: a cycle whose frames all hold 0 (every walk, run, sprint and alert twin in the shipped
  data) moves `advance` pixels every frame, and a cycle with non-zero holds moves in discrete jumps at the frame
  changes. This is what makes the measured speeds come out: the total distance of a cycle is the sum of its
  advances and its duration is the sum of `hold + 1`, so the average speed is `sum(advance) / (sum(hold + 1) x
  frame length)` - for the hero's walk 4 px per 46.875 ms = 85.3 px/s, and for the crouched walk 27 px over 32
  frames = 18.0 px/s, both matching `sprite-animations.md` rule 3.
- **ANIM-033** (observed, 005b7820; high). The **completion signal** of a looping mode fires when the last frame
  of the clip has become current *and* the timer has reached that frame's hold value, i.e. on the final frame of
  the final displayed frame of the cycle - one frame before the wrap. A last frame whose hold value is 0 signals
  as soon as it becomes current. The signal is produced once per cycle; the animation keeps looping afterwards.
  A separate path signals completion when the current frame's hold value is 0 at that point, which is the same
  case stated in the code twice.
- **ANIM-034** (observed, 005b7720, 005b8050, 005b86b0; medium). When a new action starts, the player also
  computes and stores an **end-of-clip marker**: a (frame, timer) pair, normally (last frame, 0) but adjusted to
  the second-to-last frame with its hold value when the clip is long enough, and set to an unreachable pair for a
  clip of one frame whose hold is below 2. Whenever the live (frame, timer) equals that stored pair, the play
  call answers status "at the end marker" (0) instead of "running" (2); the status is used for the completion of
  a few actions (ANIM-132). The exact adjustment rule was read but its purpose is only inferred: it lets an
  action end one frame early so that the next action can blend in.
- **ANIM-035** (observed, 005bd450; high). When a clip completes, the player releases the cached frames of the
  animation it was playing (one release call per frame of the animation). This is a cache-management effect only.

### 3.3 Play modes

The play mode is chosen by the caller (the action executor) per action; it is stored so that a change of mode
alone does not reset the timer (only a change of action id does, ANIM-030). `n` is the frame count, `m` the
marker frame of ANIM-012.

| Mode | Behaviour of one step | Completion signal |
|---|---|---|
| 0 | forward, wrap to 0 at `n`: an endless loop | once per cycle (ANIM-033) |
| 1 | forward, wrap to 0 at `n` | never |
| 2 | one frame per frame, ignoring the hold values (timer forced to 0) | when it steps past the last frame, leaving the frame index equal to `n` (one past the end) |
| 3 | forward by **two** frames per advance, wrap at `n` | when the frame index reaches `n - 2` with its hold reached |
| 4 | as mode 1, but while the clip sits at (frame 0, timer 0) it only starts with probability 131/32768 per frame, drawing one value from the global random stream each frame | never |
| 5 | as mode 4 with probability 327/32768 | never |
| 6 | as mode 0, but a reset starts at the marker frame `m` | once per cycle |
| 7 | as mode 0 while the frame index is not `m`; on `m` it holds the frame and, once per frame, rotates the facing (and the target facing) by **-2** of 16 (45 degrees counter-clockwise), up to 8 times, then moves past `m`: a full 360-degree turn in place over 8 frames | once per cycle, on the ordinary path |
| 8 | forward, stopping on the last frame (`n - 1`), which is then held for ever | never |
| 9 | forward, stopping two frames before the end | never |
| 10 | reset sets (frame 0, timer 0); no step of its own | never |
| 11 | frame index forced to 0 every step | never |
| 12 | frame index forced to `n - 1` every step | never |
| 13 | **backwards**, wrapping from 0 to `n - 1`, holds respected | when it reaches frame 0 with its hold reached |
| 14 | backwards by two frames, stopping two frames before the end | never |
| other | nothing happens (a frozen animation) | never |

- **ANIM-040** (observed, 005b7820; high). Modes 4 and 5 are the only consumers of the random stream in the
  animation player. They are the pause between repetitions of an idle cycle: the cycle plays out, comes back to
  its first frame and waits there, rolling once per frame, so the waiting time is geometric with mean 250 frames
  (mode 4, about 11.7 s) or 100 frames (mode 5, about 4.7 s). The roll is `rand()` of the C runtime's 15-bit
  generator compared with 130 and 326 respectively (`spec-ai-combat.md` AI-005: one global stream; the order of
  rolls within a frame follows the element update order of ANIM-101a).
- **ANIM-041** (inferred, 005b7820 and the shipped data; high). There is **no ping-pong mode**. The
  back-and-forth of the idle cycles is in the data: the frame list of such an animation already contains the
  frames in the order 0 1 2 3 2 1, as `sprite-animations.md` notes. Modes 13 and 14 play a clip backwards but
  never turn around.
- **ANIM-042** (observed, 005b7820 mode 2; high). Mode 2 leaves the frame index one past the last frame when it
  ends. Any code that reads the frame after a mode-2 clip has finished would read outside the animation; the
  program does not, because the completion is acted on in the same frame.

### 3.4 Actions, action elements and completion (the VM's open question 6.1)

An actor's queue holds **action elements**. Some are created by the script's sequence elements
(`spec-script-vm.md` VM-230), some by the walk order pipeline (`spec-navigation.md` NAV-130), some by the AI.
Each carries an **action id**, an element **kind**, and typed parameter slots. The executor is a dispatch on the
action id; every branch chooses an animation action id, a **play mode** (3.3) and a **movement mode** (3.5) and
answers a status.

- **ANIM-120** (observed, 005b8050, 005b86b0, 00464230; high). The statuses an executor answers and what the
  actor does with them: **0** = the animation reached the end marker of ANIM-034 (the actor marks the action
  element as "at its end"); **1** = the animation was (re)started this frame; **2** = running; **3** = the
  action is finished; **4** = the profile has no such animation (the action element is refused); **5** = the
  action refuses to run (refused). A status of 3 pops the action element: the next queued element becomes
  current in the same frame if there is one, otherwise the element's sequence is told the element is **done**
  (state 0, `spec-script-vm.md` VM-211). A refusal sets state 5 and triggers the abort cascade (VM-217).
- **ANIM-121** (observed, 0046bcf0; high). Independently of the status, an executor may report a **finish code**
  to the actor's own class through a virtual call and store it; the codes observed are small integers (1, 2, 3,
  4, 8) that the AI and the player-character code read to know how the last action ended. Their meanings belong
  to `spec-ai-combat.md`.
- **ANIM-122** (observed, 0046abd0; high). Before an action element is admitted, the actor classifies it into an
  **admission group** by kind, which decides which of its admission tests run: one group holds the walk
  (kind 0x14, whose group additionally depends on a flag bit of the element: a "direct" walk is in a different
  group), one holds the door crossing (0x13), one holds the lifts (0xac, 0xad, 0xae), and one large group holds
  every posture, animation, turn, climb and wait kind (1, 2, 10, 0x15, 0x19, 0x1a, 0x1b, 0x1c, 0x47, 0xa0,
  0xa4..0xa7, 0xa9). Every other kind falls into a default group. The tests themselves are the AI's
  (`spec-script-vm.md` VM-216 names the virtual slots).

**Completion of the script's element kinds** (the answer to `spec-script-vm.md` open question 6.1):

| Kind (native) | What the actor does | Completes when |
|---|---|---|
| 0x14 (45, 212), 0x18 | the walk order pipeline of `spec-navigation.md` NAV-130/140: a sequence of internal move actions along the path's waypoints, movement mode 5 (3.5) | the last move action reports arrival; a failure of the path search or of the straight-line test refuses the element (status 5) |
| 2 (internal, the door approach) | nothing; it is a test | at once: **done** if the actor is within `order radius + 5` px of the element's point, or if the element names a sector and the actor is in it; **cancelled** (state 6, which aborts the rest of the sequence) otherwise |
| 0x1A (48, and 59 code 1) | turn to the point or to the given facing: sets the target facing, turns one step per frame (3.6) | when the facing equals the target facing |
| 0xA4 (49, play once) | plays the given action id in **play mode 0** | at the end of the first cycle (ANIM-033) |
| 0xA5 (50, loop) | plays the given action id in **play mode 1** | **never**: the element stays running for ever, so its sequence level never completes and every later level of that sequence is blocked. Scripts use it for animations that should run until something else cancels them |
| 0xA6 (51, play and freeze) | plays in **play mode 0**; on completion it *also* creates and launches a one-element sequence of kind 0xA7 on the same actor | the 0xA6 element completes at the end of the cycle; the spawned 0xA7 element plays the same action in **play mode 12** (hold the last frame) and never completes |
| 0xAA / 0xAB (60, 61) | swap or restore the action-id table (ANIM-014) | at once (`spec-script-vm.md` VM-231) |
| 0x92 (62, 69, speak) | starts the speech and the speaking animation; the element is held while the speech plays | when the speech ends (ANIM-140) |
| 0x7F / 0x80 / 0x81 (52, 53, 243) | lock / unlock the AI, clear the highlight | at once (VM-231) |
| 0x15 (57, 70, 71, seek) | a walk that re-targets the moving target; when it ends, the attached sub-sequence of natives 70/71 is launched | when the actor reaches the target (the same arrival rule as the walk); the element also completes at once if the seeking actor *is* the target |
| 0x65 / 0x66 (63, 65, corpse) | pick up or put down the carried body: the carry animations of `sprite-animations.md` 118..121 | at the end of the animation |
| 0x13 (internal, door), 0xA9 (ladder), 0xac..0xae (lifts) | the action lists of `spec-navigation.md` NAV-171/200 | when the last action of the list ends |
| 0xA0 (internal, wait) | nothing; counts down | when the actor's wait counter reaches 0 (ANIM-133) |
| 0x2B (internal, cart hit) | inflicts the damage of ANIM-305 | at once |
| 0x26 (102), 0x1D / 0x20 / 0x22 / 0x32 / 0x33 / 0x34 / 0x3B..0x43 / 0x4D / 0x87 / 0x88 / 0xA1 / 0xB2 (59) | the postures, strikes and reactions of `spec-ai-combat.md` | each plays its animation and completes at its end, except the ones whose executor answers 2 permanently (the guards and the idles) |

- **ANIM-130** (observed, 00464b20; high). The **idle chain**: the idle action plays in mode 0; at the end of
  each cycle one value is drawn from the random stream and, with probability 1/10 (`rand()` modulo 10 equal to
  0), the actor's action id is changed to the *fidget* action and re-dispatched in the same frame; the fidget
  plays in mode 0 and at its end sets the action id back to idle. This is the only use of the random stream in
  the action dispatcher and it is why a standing character occasionally shifts his weight. (The stochastic idle
  of play modes 4 and 5, ANIM-040, is a different mechanism used by other classes.)
- **ANIM-131** (observed, 00464b20; high). The locomotion actions all use **play mode 0** (loop with a completion
  signal once per cycle). Their **movement modes** differ: the walk-start uses mode **5** with the factor 1.0,
  the walk and the run use mode **1**, the sprint uses mode **2**; the turn and posture actions use mode **0**.
  For the walk, run and sprint the float factor is not a literal: the executor asks the current action element
  for it through a virtual call, so an action element can carry a speed multiplier (the mechanism by which a
  scripted or AI-chosen gait modifier would enter; every retail use seen resolves to 1.0). All of them pass the
  *next* queued action element to the player so it can look ahead one waypoint.
- **ANIM-132** (observed, 0048d510, 00464b20; high). A few actions complete on status **0** (the end marker of
  ANIM-034) rather than 3 - the search action is one - and one completes on status **1** (the frame it starts),
  which makes it effectively instantaneous. An implementer must therefore keep the end-marker pair, not only the
  end-of-clip signal.
- **ANIM-133** (observed, 00464230; high). The wait action decrements a per-actor counter once per frame and
  reports "finished" when it reaches zero; the counter is set by whoever queued the wait (the building-entry
  waits of `spec-navigation.md` NAV-130 are 50 frames and `rand()&15 + rand()&15` frames, i.e. 2.3 s and 0..1.4 s
  at 46.875 ms).
- **ANIM-134** (observed, 00464b20, 0048d510; high). **Where the action transitions live.** The animation player
  itself makes exactly one transition: idle to fidget and back (ANIM-130). Every other chain - walk-start into
  walk, walk into walk-stop, run-start into run, sprint-start into sprint, the alert twins, crouch down into sneak
  and back up - is chosen by the order and AI layer: each of those actions is an action element that plays its clip
  to the end, reports a finish code (ANIM-121) and is popped, and the next action element in the queue decides what
  follows. An implementer must therefore not hard-code a state machine over action ids in the animation player; the
  player only needs "play this id in this mode and tell me when it ends". Which ids the AI and the click handlers
  choose is `spec-ai-combat.md` and `spec-navigation.md` 3.4 (gaits); the ids themselves and their visual roles are
  `docs/formats/sprite-animations.md`.
- **ANIM-140** (inferred, 00464b20 speech case, 004b9f40; medium). The speech element is held while the spoken
  sound plays and ends when the sound ends; with sound disabled or the line missing it ends after the text's own
  display time. The exact source of the duration was not read: it is either the sound's length or a per-line
  value of the text data. This is the one completion rule of the table above that stays **unknown** in its
  detail; the observable consequence (the sequence waits for the line) is certain.

### 3.5 Displacement per frame

The mover is one routine: given the element, its action element, the action id, a **movement mode**, a float
**factor**, a **play mode** and a restart flag, it (1) resolves the action-id overrides, (2) checks the action
exists, (3) if the action element changed, starts the new animation, (4) sets the animation index from the
action's block plus the facing, (5) steps the frame timer (3.2), (6) computes the displacement, (7) moves.

- **ANIM-200** (observed, 005b86b0; high). The displacement magnitude of a frame is
  `d = advance(new frame) x factor` where `advance` is the signed 16-bit value of the animation frame that became
  current in this frame (0 if no frame change, ANIM-032) and `factor` is the caller's float (1.0 for every
  locomotion action of ANIM-131; other callers pass a value obtained from the action element through a virtual
  call, which is how a scripted or AI-chosen speed multiplier would enter). If `d` is exactly 0 the element does
  not move at all this frame and no collision work is done.
- **ANIM-201** (observed, 005b86b0, 00464230; high). The direction is the element's movement direction vector,
  and the displacement is applied in screen space with the height taken from the projection plane
  (`spec-navigation.md` NAV-002). The position at the start of the frame is kept as the previous position and is
  what the bond crossing (NAV-160) and the drawing use.
- **ANIM-202** (observed, 005b86b0; high for the arithmetic, unknown for the user). **Movement mode 3** steps the
  frame timer a **second** time in the same frame and accumulates
  `d = ((2 x advance(first change)) + advance(second change)) x 2 x factor`, each contribution being 0 when that
  step did not change the frame. The first contribution is thus doubled twice; this is what the instructions
  compute (checked against the raw code), it is not a clean "twice the speed". No caller passing mode 3 was found
  in the human action dispatcher (walk and run pass 1, sprint 2, walk-start 5, the rest 0), so which class uses it
  is open (section 9).
- **ANIM-203** (observed, 005b86b0; high). **While the facing differs from the target facing** the displacement
  is scaled: movement mode 6 **doubles** it and steps the frame timer once more; every other mode multiplies it
  by **0.6**; afterwards, if the result is below **0.7** px it is set to 0.7 px. So a turning character keeps
  creeping forward at at least 0.7 px per frame (15 px/s) and cannot stall.
- **ANIM-204** (observed, 005b86b0, 00561040, 00563e90; high). The move itself: **movement modes 7 and 8, and an
  element whose no-collision flag is set or whose sector forbids collision, move without any collision handling**
  (the position is set, the projection plane re-evaluated, the sprite marked dirty). Every other mode goes
  through the collision-aware move, and if that move leaves the element inside geometry more than **50** times in
  a row the frame is refused with a logged "anticollision" failure and the element does not move.
- **ANIM-205** (observed, 00561040; high). The collision-aware move, in order: (a) walk the level's element table
  from index 0 upwards and, for every other element that is displayed, is not the element carried by this one,
  and is on the **same layer/sector word and the same projection area**, test the boxes and, for actor-family
  elements, the centre distance against **5.0** px; a hit calls that element's **proximity callback** (the
  virtual that makes a character step aside, stop or be pushed - not read, section 9). Target-family elements get
  the same callback when their own "is solid" answer is non-zero. (b) The move is tested with the live
  straight-line corridor test (`spec-navigation.md` NAV-112). (c) On failure it is resolved against the wall
  segments and bonds of the cells the step crosses (sliding, section 9), and the blocked-move counter of
  ANIM-204 is incremented while a decaying float is reduced (its use was not read). (d) The bonds crossed by the
  step are applied (NAV-160), which is where the height and the ground kind change.
- **ANIM-206** (observed, 00560780; high). After the move, the element publishes a **velocity vector** =
  `d x direction / (hold(current frame) + 1)`, i.e. the *average* velocity over the frames the current animation
  frame will be displayed. This is the value other subsystems ask the element for its speed (the camera follow of
  ANIM-340 uses it).
- **ANIM-207** (observed, 005b86b0; high). **Footstep effects.** When the element's ground kind is exactly **5**
  and the frame's displacement exceeds **2.0** px, a counter cycles 0, 1, 2 and on every third such frame an
  effect of kind 7 is created at the element's position on its layer. Nothing similar happens on the other eight
  ground kinds.
- **ANIM-208** (observed, 005b86b0, 00560460; high). **Arrival.** In every movement mode except 5, after the move,
  if the element moved this frame and has come within its **stopping radius** of its target point, the mover
  answers status 3 and the action - and with it the script's walk element - completes. The stopping radius is a
  field of the element; the walk order pipeline sets it per order (`spec-navigation.md` NAV-152: the effective
  test is `distance < radius + 5` px, 10 px for the door approach points).
- **ANIM-209** (observed, 005b86b0, 00560460, 005c9010; high). **Movement mode 5 follows a waypoint list**: the
  action element holds a list of waypoints with a cursor and a direction flag; each frame, when the element is
  within its stopping radius of the current waypoint, the cursor moves on (forwards or backwards according to the
  flag, and it re-seeks from the nearer end when the requested index is far from the current one), the movement
  direction is re-aimed and the step continues in the same frame. When the list is exhausted the mover answers
  status 3. The walk-start action and the cart (3.8) use this mode.

### 3.6 Turning: 16 facings

- **ANIM-210** (observed, 0055f0f0, 0055f140; high). An element keeps a facing and a target facing, both 0..15,
  0 = screen-up and increasing clockwise (`sprite-animations.md` "Direction order"). The ordinary turn step, run
  once per frame by the action executor before the animation is played, is: `delta = (target - facing) mod 16`;
  if `delta` is 0 nothing happens and the step answers "already facing"; if `delta < 8` the facing increases by
  1; otherwise it decreases by 1. A difference of exactly 8 (dead opposite) therefore turns **counter-clockwise**.
  The turn is one sixteenth (22.5 degrees) per frame, so a full about-turn takes 8 frames (0.375 s).
- **ANIM-211** (observed, 0055f140, 0055f210, 0055f1a0; high / medium). Three variants exist and the actor's
  class or a per-element flag chooses: (a) the **two-step** turn, which moves 2 of 16 per frame while the
  remaining difference is more than 1 and makes a single 1-step correction at the end (used by the human action
  dispatcher for some actions); (b) a **damped** turn, selected by a per-element flag, which requires the sign of
  the difference to persist for two consecutive frames before the facing moves (a signed counter that resets to 0
  when the sign flips) - this is what keeps a character from flickering between two facings when the target
  oscillates; (c) a **delayed** turn used by one class, which waits a caller-given number of frames between
  steps. All three use the same shorter-arc rule.
- **ANIM-212** (observed, 0055fc20; high). The **target facing** of a moving element is derived from its movement
  direction: the direction is converted to world space through the projection plane, quantised to one of 16
  directions, and mirrored (index exclusive-or 8) when the element's reverse flag is set. It is only recomputed
  when the element actually moves; a stationary element keeps its target facing. Scripts and the AI set the
  target facing directly (native 94, the turn element kind 0x1A, the AI's stares).
- **ANIM-213** (observed, 005b7820 mode 7; high). Play mode 7 is the only place where the facing is changed by
  the animation player itself: it rotates the facing *and* the target facing by -2 steps (45 degrees
  counter-clockwise) per frame, 8 times, on one marker frame of the clip - a 360-degree turn in place in 8
  frames.
- **ANIM-214** (observed, 005b86b0, 005b7300; high). Changing the facing does **not** restart the animation: the
  frame index and the frame timer survive, only the animation index changes (block plus new facing). A character
  therefore keeps its stride through a turn, and the turning penalty of ANIM-203 is the only cost.

### 3.7 Placement, teleport and off-map

- **ANIM-220** (observed, 00464230, 005b6790, 00462730; high). A pending placement is applied at the start of the
  element's update, before anything else: position, layer/sector and projection area are set, the previous
  position is set to the new position (so no bond is crossed and no step is interpolated), the depth key is
  recomputed and the sprite is marked dirty. Placing an element at another element's place copies that element's
  facing into **both** the facing and the target facing, so no turn follows.
- **ANIM-221** (observed, 005b6790; high). The mission file's placement of a character gives the initial facing
  (masked to 0..15, written to both the facing and the target facing) and, for objects, an initial action id
  which is played once at load; a placement whose initial action id is absent from the profile is reported and
  the object keeps no animation. For the two character placement kinds the program also writes the raw facing
  byte into the current *animation index* before any action is played, which is meaningless until the first play
  call overwrites it; an implementer should simply start from a valid animation.
- **ANIM-222** (observed, native 96 in `spec-script-vm.md`, 005b86b0; high). An element taken off the map keeps
  its animation state but is not drawn, not updated for movement and not a collision partner.

### 3.8 The cart (the "mobile element")

A cart moves on a completely different principle from a character: **its speed drives its animation**, not the
other way round, and it is steered by a small instruction stream that comes with the mission.

- **ANIM-300** (observed, 004ab720; high). Once per frame, if the cart is displayed: if its acceleration is
  non-zero, `speed := speed + acceleration`, and when the speed has reached or passed the target speed (sign
  aware) the acceleration is cleared and the speed is set exactly to the target. Then, if the speed is non-zero,
  the cart moves by `speed x direction` (px per frame, in the same screen space and through the same projection
  plane as a character). There is no animation-driven advance and no collision test against walls on this path.
- **ANIM-301** (observed, 004ab7e0; medium). After moving, the cart writes a value derived from the **reciprocal
  of its speed** into each of its sub-sprites (the wheels and the load are separate sprite objects kept in a list)
  and steps each sub-sprite's animation once. The effect is that the wheels turn at a rate proportional to the
  speed; which routine consumes the stored value was not settled, so an implementer should simply drive the
  sub-sprites' frame rate from the speed.
- **ANIM-302** (observed, 004abfe0, 004ac350; high). The cart runs a **program** taken from the mission data: a
  byte stream with a per-block length counter. Each frame, while the remaining length of the current block is
  non-zero, instructions are executed one after another until one of them answers "stop for this frame". When a
  block is exhausted the next block is selected: the program holds a table of alternatives, each with a weight
  byte and an offset, and the choice is made by drawing **`rand()` modulo 100, plus 1** from the global random
  stream and walking the table subtracting weights until the roll no longer exceeds the entry's weight - a
  weighted random branch, drawn once per block, from the same stream as everything else
  (`spec-ai-combat.md` AI-005). One further byte selects between two variants of the block list before the roll.
- **ANIM-303** (observed, 004ac350; high for the effects, unknown for the full set). The instructions are single
  bytes from 0x80 upwards followed by their operands, and the length counter is decremented by the instruction's
  size (3, 5 or 7 bytes). What each does: **0x80** - set an action id (16-bit) on every sub-sprite and restart
  each from its first frame; **0x81** - set the current speed (float) and mark the cart stopped when it is 0,
  moving otherwise; **0x82** - set a target speed (float; a target of 0 is replaced by **0.1**) and the index of
  the next waypoint, and compute `acceleration = (target^2 - speed^2) / (2 x distance to that waypoint)`, i.e.
  constant acceleration that arrives at the target speed exactly at the waypoint; **0x83** - set the next
  waypoint index and reset a per-sub-sprite timing value; **0x84** - "flip the ends", not implemented in this
  build (it logs a warning and does nothing); **0x85** and **0x86** - two further one-operand calls into the cart
  (a sound and a state change; not read). Any other byte is reported as an unknown command and the program stops.
  An out-of-range waypoint index is reported as a fatal error at the moment it is read.
- **ANIM-304** (observed, 004af540; high). A cart crosses bonds exactly like a character (`spec-navigation.md`
  NAV-160), which is how it changes height and projection area; duplicated bonds are logged and ignored.
- **ANIM-305** (observed, 004d9420; high). **Carts run people over.** Once per level tick, after the per-element
  updates, the program walks every unordered pair of level elements - outer index from the last element down to
  the first, inner index from the outer index minus one down to 0 - and for each pair in which one element is a
  human and the other is a cart whose "harmless" flag is clear, and whose boxes overlap on the same layer, it
  creates and launches a one-element sequence that inflicts damage on the human: **50** when a condition on the
  cart's motion holds (it is moving, inferred) and **10** otherwise. The pair order matters only for the order in
  which the damage sequences are queued.

### 3.9 Lifts, stairs and climbs; layer changes

The geometry and the action lists are `spec-navigation.md` 3.10; what belongs here:

- **ANIM-310** (observed, 005bddb0, NAV-200; high). The ladder and ivy climbs play a mount action, then a **loop
  action repeated a number of times taken from the animation record** (ANIM-013), then a dismount action; the
  repetition count is therefore a property of the player's sprite file, not of the map, and the climb's duration
  follows from the loop clip's own frame timings on the clock of ANIM-003.
- **ANIM-311** (observed, 00464b20 wait case, `spec-navigation.md` NAV-171; high). A layer change never happens
  during free movement: it happens in the door-crossing and lift action lists, between two move actions, so an
  element's layer is constant for the whole frame and every collision and drawing decision of that frame uses one
  layer.
- **ANIM-312** (observed, `spec-navigation.md` NAV-190; high). A character inside a building is given the extra
  layer index and is marked not displayed, so it is skipped by the drawing (section 6) and by the proximity test
  of ANIM-205 (which requires the same layer word), but its animation still steps: its element update runs
  normally.

### 3.10 The camera

- **ANIM-320** (observed, 004cec60, 004c7f60, 005758d0, 005a8650; high). The camera is two-dimensional and has
  **no layer and no height**. Its state is the **top-left corner of the visible world rectangle** in background
  pixels (two floats that always hold integers - every write rounds), plus the zoom, plus the scroll and follow
  state below. The visible rectangle is `screen width / zoom` by `(screen height - 80) / zoom`, the 80 px being
  the strip the HUD occupies (ANIM-380). The camera is clamped so that the rectangle stays inside
  the map: the corner is kept in `[0, map size - rectangle size]` on each axis independently. All the camera
  publishes to the drawing is that corner and the zoom.
- **ANIM-321** (observed, 004c8380, 005105d0; high). The camera is advanced **once per frame, at the start of the
  drawing**, i.e. after the level tick of the same iteration (ANIM-100). In the no-render mode of ANIM-521 it is
  advanced only on every 32nd frame.
- **ANIM-322** (observed, 004cec60; high). A camera step that would leave the map is **clipped**, the
  corresponding acceleration index is zeroed, and the step reports failure; a script scroll that hits a map border
  is **terminated** by that failure (its element completes, section 6).
- **ANIM-323** (observed, 004bef82 constructor loop, 004dba50, 004db920; high). **Scrolling by keys and by HUD
  buttons.** Four commands (scroll up, down, left, right) and two (zoom in, out) arrive as engine commands through
  one dispatch table. A scroll command sets the frame's camera delta to `+/- ramp[index] / zoom`, so the step is
  constant in *screen* pixels and therefore half or double in world pixels at zoom 2.0 or 0.5. Two ramps of 32
  entries each (one per axis) are built at construction from the same rule: entry 0 is 0, a working value starts at
  **6.0**, is rounded to an even integer, stored, and multiplied by **1.05** while it is below **31.0**; the
  result rises from 6 px per frame to a ceiling of 32 px per frame over about 21 entries. While the command keeps
  arriving the index increases by one per frame up to 31; reversing the direction resets it to 0; on a frame with
  no command the index is **decreased** and the camera keeps moving, so it coasts to a stop.
- **ANIM-324** (observed, 004d0400; high). There is **no drag panning**: the mouse-drag state drives the
  rubber-band selection rectangle, not the camera. Where the *screen-edge* hover is turned into the four scroll
  commands was not found (section 9); that it exists is certain from the key-binding entries the HUD creates for
  the screen borders.
- **ANIM-325** (observed, 004bef3b, 00571200; high). **Zoom** has exactly three values, **0.5, 1.0 and 2.0**,
  held as an index 0..2 into a three-entry table; 1.0 is the value at level start. Native 21 rejects anything
  else. Zooming out is refused when the zoom is already 0.5 or when the map would be smaller than the visible
  rectangle at the next step; zooming in is refused at 2.0.
- **ANIM-326** (observed, 004cf610, 004cfce0, 004c9119; high). A zoom change is **animated over 8 frames**: the
  screen captured before the change is blitted with an interpolated scale of `((n x 0.5) + (8 - n)) / 8` on step
  `n`, and the transition finishes on the last step. While it runs, further zoom requests are refused and the
  camera reports itself busy. At zoom 0.5 the camera corner is additionally **snapped to even pixels**.
- **ANIM-327** (observed, 005a8650; high). Zoom scales the whole drawing downstream in the draw manager; it does
  not select a different background, a different resolution or a different layer set. A change of zoom
  invalidates the cached backdrop.
- **ANIM-330** (observed, 004ca410 kind 6, 004cdfc0; high). **Scroll to a point** (script element kind 6, natives
  33 and 42): starting the element stores the target point (raw and clamped), stores the element's speed
  parameter, sets the **step length to 2.0** px per frame, resets the ramp index, registers the element as the
  camera's current element and cancels any camera lock. Per frame the camera moves by
  `normalise(target - corner) x L` where `L` is the element's speed parameter when it is non-zero and the current
  step length otherwise; when the remaining distance is shorter than `L` the step is shortened to land exactly on
  the target; the step is then truncated to whole pixels. With a speed of zero the step length is re-read every
  frame from the ramp of ANIM-323 (index capped at 31), so a default scroll starts at 2 px per frame and
  accelerates to 32. **The element completes** when the corner equals the target exactly, or when the step is
  clipped at a map border; either way the target is cleared, the step length is reset to 1.0 and the element is
  told it is done. When the level's "no cinematic camera" flag is set, kind 6 degenerates to an instant jump and
  completes at once.
- **ANIM-331** (observed, 00571270, 00571330, 004ca410, 004ceb2a; high). **This is what natives 18 and 19 write.**
  Both jump the camera to the given point; in addition native 18 stores **2.0** and native 19 stores **its float
  argument** into the *scroll step length* - the same field the scroll element sets to 2.0 - not into the zoom.
  The neutral value of that field is 1.0, and it is reset to 1.0 whenever a scroll ends or is cancelled. Because a
  running scroll with speed 0 overwrites the field from the ramp every frame, a value given by native 19 governs
  only the first frame of the next default-speed scroll. The current engine's reading of 18/19 as "set the zoom
  to 2.0 / to f" is wrong (section 10).
- **ANIM-332** (observed, 004ca410 kind 7, 00572ba0, 005713f0; high). **Jump** (element kind 7, native 34, and
  natives 18/19/20 immediately): the point is clamped, the corner is set, the cached-screen validity flag is
  cleared - which forces a full redraw next frame - any camera lock is cancelled, and the element **completes in
  the same call**.
- **ANIM-333** (observed, 004ca410 kind 8, 004cdfc0; high). **Zoom element** (kind 8, native 35): the element
  stores the wanted zoom factor as a request and registers itself as the camera's element. Each frame the camera
  posts one zoom-in or zoom-out command toward the request; if the gate of ANIM-325 refuses, the request is forced
  equal to the current zoom so that the element cannot hang. The element completes on the first frame where the
  request equals the current zoom - so a 2.0 to 0.5 change takes two 8-frame animations plus one frame.
- **ANIM-340** (observed, 004d90b0, 004cdfc0; medium-high). **Lock on an actor** (element kinds 0xD and 0xE,
  natives 39 and 40): the camera stores the actor and an enable byte. While locked it **does not re-centre**: it
  keeps the actor at the screen position the actor had when the lock was taken. Per axis it keeps a remainder and
  a velocity: when the remaining offset is at least **1.0** px the velocity is set to `sign x the actor's own
  reported speed` (ANIM-206), so the camera matches the character's gait rather than easing; otherwise it closes
  the remainder at **one thirtieth** per frame for a burst of **15** frames. Steps are floored to whole pixels and
  then clamped by ANIM-322.
- **ANIM-341** (observed, 004cdfc0, 004ca410; medium). The lock is broken by the target being removed from the
  level, by any camera element (scroll, jump, lock, unlock) and by native 20. Player scrolling does not clear the
  lock; it only clears a separate marker that says the camera is centred on the selected character.
- **ANIM-342** (observed, 004c8380, 0050f710; medium). Nothing suspends the camera while a dialogue or a text page
  is open beyond the suspension of the whole tick (ANIM-004); the camera is part of the drawing, so it keeps
  publishing its state. Entering a building does not move the camera.

### 3.11 Drawing one frame

The composition is behaviour an implementer must reproduce because it decides what is visible and what hides
what; the pass list is a required ordering, not a prescribed decomposition.

- **ANIM-360** (observed, 005105d0; high). The drawing, in order: the **camera and scene** (ANIM-361), a music
  poke, the **HUD widget tree**, frame statistics, an optional on-screen message clipped to a 160 px band at the
  bottom, the **HUD panel and its text**, the **mouse cursor**, and the **present**. The whole of it is skipped
  while the freeze flag of ANIM-521 is set.
- **ANIM-361** (observed, 004c8380; high). The scene starts with a **full opaque copy of the cached backdrop into
  the back buffer** and then runs the element passes. There are **no dirty rectangles**: every frame redraws
  everything. The only incremental trick is on the cached backdrop itself, which self-blits by the scroll delta
  and re-renders only the newly exposed band.
- **ANIM-362** (observed, 004d0a10; high). The element passes, in order: (1) a visibility and culling refresh,
  performed only when a dirty flag is set; (2) the camera transform and clip box, including the vertical offset of
  **-80** px; (3) depth-key propagation for attached effects; (4) the **backdrop elements**; (5) the terrain patch
  renderer; (6) the **selection and highlight underlays** of the actors; (7) the **shadows**; (8) the **ground
  marks**; (9) the binding of queued draw items to their actors; (10) **the sort and the merge** (ANIM-363); (11)
  the **main element pass** in the merged order, flushing the deferred queue up to each actor's depth key before
  drawing that actor; (12) a final flush of the deferred queue; (13) the stretched sprites; (14) the movement-path
  line and its trail marks, drawn only while the cursor is not over a pickable element; (15) floating text and
  labels; (16) a clearing of the per-element "drawn" flags. Everything after that is debug overlay, unreachable in
  the retail build.
- **ANIM-363** (observed, 004d1d00, 00462a30, 004aa980; high). **The order of the main pass.** The movable
  elements are sorted by the **depth key** ascending, ties broken by the element's own id ascending (a stable,
  deterministic total order). They are then **merged into the scenery list**, whose order comes from the level file
  and is treated as already correct: walking the scenery in its file order, every movable that passes the
  "is in front of this piece of scenery" test is emitted before it, and the remainder after the last piece.
  The test is the classic 2.5-dimensional **sort line**: each piece of scenery carries a polyline; the segment that
  spans the movable's screen x is found and the movable's side of that segment decides. Scenery without a polyline
  falls back to comparing the depth key.
- **ANIM-364** (observed, 00462a30, 005bd560, 0055f290; high). **The depth key is the element's world row**
  (the world y, not the screen row; an object lifted onto a roof therefore sorts at the depth of the ground under
  it). Two adjustments exist, and they are the whole of the "occluder" machinery an implementer needs besides the
  sort line: an element bound to another element takes the other's key **plus or minus 0.001**, and an effect
  attached to an owner takes the owner's key **plus or minus 0.01**; one kind of spawned effect adds **1000.1** to
  force itself in front of everything.
- **ANIM-365** (observed, 004d0a10, 005c73e0, 005c2ec0; high). Effects and decorations are not a separate pass:
  they are put into a **deferred queue** carrying their own depth value and are flushed into the main pass at the
  point where the queue's head is no longer in front of the actor about to be drawn, with a final flush at a
  sentinel depth of 1000000.
- **ANIM-366** (observed, 004d0a10, 004c0510; high / medium). There is **no separate occluder or mask pass**. A
  building hides a character because the building is an element in the merged list that is drawn after the
  character. The mask and sector data of the level file is loaded with the background; how it becomes the drawable
  pieces was not settled (section 9).
- **ANIM-367** (observed, 004d0a10, 004c0510; high for the ordering, medium for the meaning). Drawing is **not
  layer by layer**: there is one merged list for the whole view. The layer index is a field of the element and
  selects which background and mask set it belongs to; the loader accepts indices 0..layer count, one more than
  the number of layers, which is the extra index a character inside a building gets
  (`spec-navigation.md` NAV-190). Only the debug overlays iterate layers.
- **ANIM-370** (observed, 0051c0d0, 0051bfb0, 004cac00, 004d7dd0; high for the mechanism, medium for the visual).
  **Ground marks** are a linked list of small records, each a position plus an animation frame index and a layer.
  They are created when the player issues an order (the marker under the destination) and, while a path line is
  being drawn, once every 11th draw along the trail. Their animation frame advances by one on every frame whose
  global frame number is even, and a mark is destroyed when its frame index reaches **6** - that is 12 frames,
  about 560 ms at 46.875 ms. There is **no upper limit** on the number of marks; the list is unbounded. They are
  drawn in their own pass, after the shadows and before the elements.
- **ANIM-380** (observed, 0052cf80, 0052d090, 004c0040, 00677598; high). **Resolution and viewport.** The mission
  view is the full configured width by the configured **height minus 80** px, anchored at the top left; the
  accepted modes are 1024x768, 1228x768 and 1360x768 (the height is always 768, so the world view is 688 px high),
  and the default is 1024x768. An unknown width is refused with a logged warning. **There is no letterboxing and no
  border**: a wider mode simply shows more of the world. Video playback switches temporarily to 640x480.
- **ANIM-381** (observed, 005e41d0, 005e5260; high). The back buffer is a double-buffered flip chain in 15-bit
  RGB555 or 16-bit RGB565; there is no 8-bit path. Presenting is a flip when fullscreen and a blit to the primary
  surface when windowed. The source colour key of the sprite blits is pure green.

### 3.12 Settings and speed

- **ANIM-520** (observed, 005460c0, 0050f710; high). A **slow-motion** toggle selects the 400 ms frame wait
  instead of 40 ms (ANIM-001), i.e. exactly a tenth of the speed with every other rule unchanged, including the
  animation clock: it is a clean time scaling, not a separate animation rate.
- **ANIM-521** (observed, 0050f710, 005105d0; medium-high). Two flags disable the frame wait: a **freeze** flag,
  which also disables the level tick and the whole drawing (it is only ever cleared in this build, so it is
  effectively dead code), and a **level-side blocking** flag, which disables the wait, the cursor and the present
  and makes the camera advance only every 32nd frame. With either set the loop runs uncapped and every duration in
  this specification shrinks with it.
- **ANIM-522** (observed, 0055d1a0, 0051ba00, 005b2190, 00409d70, 005a6520; high). The only persisted settings are
  the player profile (a handful of values), two key-binding sets (also as two configuration files under the game's
  data directory), the sound configuration (five volumes and two bytes) and the graphics configuration (the
  resolution and four further bytes). The registry holds only the sound device, the language, the version and the
  path. The command line has no timing or difficulty switch. No setting changes the animation or movement rules.
- **ANIM-523** (observed, 00575f70, 0055d1a0, 00564580; high). **There is no difficulty setting in this build.**
  A search of the strings, the profile serialiser and the save code finds no difficulty level and no global
  multiplier on life, damage or alert times. The only per-mission tuning is the per-character AI level a mission
  script sets (`spec-ai-combat.md`) and the static tables of the shipped configuration profile. An implementer
  must not offer a difficulty that scales rules if the goal is fidelity.

## 4. Claims

Every claim of sections 2 and 3 carries its id, status, evidence address and confidence inline, in the form
**ANIM-nnn** (status, address; confidence). The ranges are: **001-004** clocks; **010-014** the animation table;
**020-022** state; **030-035** the frame timer; **040-042** the play modes; **100-103** the frame and the update
order; **120-140** actions and completion; **200-209** displacement; **210-214** turning; **220-222** placement;
**300-305** the cart; **310-312** lifts and layers; **320-342** the camera; **360-381** drawing; **520-523**
settings. Claims that stay **unknown** and therefore map to an `Assumption` variant (ADR-0008) in the
implementation: ANIM-012 (which field of the animation record is the marker frame), ANIM-140 (the source of a
speech element's duration), ANIM-202 (which class uses movement mode 3), ANIM-205c (the sliding rule and the
proximity callback), ANIM-303 (the last two cart instructions and the full instruction set), ANIM-324 (the
screen-edge scroll trigger and its border width), ANIM-366 (how the mask data becomes drawable pieces).

## 5. Constants

Individual functional facts. No table of the shipped data is reproduced here: the per-frame hold and advance
values, the marker and loop fields and the per-animation displacement are read from the player's
`DATA/Characters/*.rhs` files by the rules of `docs/formats/sprite-animations.md` (rules 1-3) and of ANIM-011.

| Name (ours) | Value | Unit | Source | Conf. |
|---|---|---|---|---|
| minimum frame length | 40 | ms | 0050f710 | high |
| slow-motion frame length | 400 | ms | 0050f710 | high |
| realised frame length on a 15.625 ms host counter | 46.875 | ms | 0050f710 + ANIM-002 | high |
| frame display length | hold value + 1 | frames | 005b7820 | high |
| extra length of the first frame after a reset | 1 | frame | 005b7300, 005bd5b0 | high |
| facings | 16 | - | 0055f0f0, 005b86b0 | high |
| turn step (ordinary / fast / mode 7) | 1 / 2 / -2 | of 16 per frame | 0055f0f0, 0055f140, 005b7820 | high |
| damped turn threshold | 2 | consecutive frames | 0055f210 | high |
| turn-in-place rotations (play mode 7) | 8 | frames of -2 steps | 005b7820 | high |
| turning advance factor | 0.6 (movement mode 6: x2) | - | 005b86b0, constant at 00679e40 | high |
| turning advance floor | 0.7 | px per frame | 005b86b0, constant at 00677db8 | high |
| arrival tolerance | order radius + 5 | px | 00467a50, constant at 0067748c | high |
| blocked-move limit before giving up | 50 | consecutive frames | 00563e90 | high |
| character proximity distance | 5.0 | px | 00561040, constant at 0067748c | high |
| footstep effect ground kind | 5 | kind code | 005b86b0 | medium |
| footstep effect displacement threshold | 2.0 | px per frame | 005b86b0, constant at 006774f4 | high |
| footstep effect period | every 3rd qualifying frame | frames | 005b86b0 | high |
| idle to fidget probability | 1 in 10 per idle cycle | - | 00464b20 | high |
| stochastic idle roll (play mode 4 / 5) | 131 / 327 out of 32768 per frame | - | 005b7820 | high |
| cart target speed replacement for zero | 0.1 | px per frame | 004ac350, constant at 00677580 | high |
| cart branch roll | 1 + rand() mod 100 | weight units | 004abfe0 | high |
| cart run-over damage | 50 (moving) / 10 | damage points | 004d9420 | high (values) / medium (condition) |
| zoom factors | 0.5, 1.0, 2.0 | - | 00571200, 004bef3b | high |
| zoom at level start | 1.0 | - | 004bef59 | high |
| zoom animation length | 8 | frames | 004c9119 | high |
| camera scroll ramp: first step, growth, ceiling, entries | 6.0, x1.05, 32 (cap 31.0), 32 | px per frame | 004bef74, 006777d0, 00678b30 | medium |
| camera scroll step scaling | ramp entry / zoom | px per frame | 004dba50, 004db920 | high |
| script scroll step at start | 2.0 | px per frame | 004ca410, 00571270 | high |
| script scroll step, neutral | 1.0 | px per frame | 004ceb2a | high |
| camera follow dead zone | 1.0 | px | 004cdfc0, constant at 00677d90 | medium |
| camera follow closing rate / burst | 1/30 per frame for 15 frames | - | 004cdfc0 | medium |
| HUD strip excluded from the world view | 80 | px | 00677598 | high |
| depth key sibling offset | 0.001 | world rows | 005bd560, constant at 006774cc | high |
| depth key attached-effect offset | 0.01 | world rows | 005c72a0, constant at 006774f0 | high |
| depth key "in front of everything" | +1000.1 | world rows | 005c2ec0, constant at 00679e58 | high |
| deferred queue flush sentinel | 1000000 | world rows | 004d0a10 | high |
| ground mark animation frames | 6 | frames | 0051c0d0 | high |
| ground mark frame period | every 2nd frame | frames | 0051c0d0 | high |
| trail mark period | every 11th draw | draws | 004d7dd0 | high |
| resolutions | 1024x768, 1228x768, 1360x768 | px | 0052cf80 | high |
| world view height | screen height - 80 (688) | px | 004c0040, 00677598 | high |
| colour depth | 15 or 16 | bpp | 005e41d0 | high |
| pairwise cart test period | every level tick | - | 004d9420 | high |
| staggered per-actor bookkeeping period | 64 | frames, phased by element identity | 00471b00 | high |

## 6. Interfaces to the script VM

Only the natives and element kinds this subsystem owns; the arity, coercion and error conventions are
`spec-script-vm.md` VM-085..VM-089 and its section 5, which this section corrects and completes.

| Id / kind | Meaning here | Completion | Claim |
|---|---|---|---|
| 18 `(loc)` | jump the camera to the point **and set the camera's scroll step length to 2.0** (not the zoom) | immediate | ANIM-331 |
| 19 `(loc, f)` | jump the camera to the point **and set the scroll step length to `f`** (not the zoom) | immediate | ANIM-331 |
| 20 `(loc)` | jump the camera to the point, cancel the actor lock, invalidate the cached screen | immediate | ANIM-332 |
| 21 `(f)` | set the zoom to 0.5, 1.0 or 2.0; anything else is an error and no change | animated over 8 frames | ANIM-325/326 |
| 33 / 42, kind 6 | scroll the camera to a point; 33 records speed 0 = "use the ramp", 42 records the speed as the step length in px per frame | when the corner reaches the point exactly, or when the step is clipped at a map border; at once when the level's "no cinematic camera" flag is set | ANIM-330 |
| 34, kind 7 | jump the camera to a point | at once | ANIM-332 |
| 35, kind 8 | zoom to 0.5 / 1.0 / 2.0 | when the current zoom equals the request, which is forced when the change is refused | ANIM-333 |
| 39 / 40, kinds 0xD / 0xE | lock the camera on an actor / release it | at once | ANIM-340 |
| 49, kind 0xA4 | play an action id once (play mode 0) | at the end of the first cycle | 3.4 |
| 50, kind 0xA5 | loop an action id (play mode 1) | **never** | 3.4 |
| 51, kind 0xA6 | play an action id and freeze on its last frame (play mode 0, then a spawned element in play mode 12) | the 0xA6 element at the end of the cycle; the spawned freeze element never | 3.4 |
| 60 / 61, kinds 0xAA / 0xAB | swap / restore the action-id table | at once | ANIM-014 |
| 45 / 212 / 46 / 47 / 64, kind 0x14 | a walk (the pipeline of `spec-navigation.md` NAV-130) | on arrival within `radius + 5` px; a path failure refuses the element | 3.4, ANIM-208 |
| 48 and 59 code 1, kind 0x1A | turn to a point or to a facing | when the facing equals the target facing (up to 8 frames) | ANIM-210 |
| 57 / 70 / 71, kind 0x15 | seek an actor | on arrival; at once when the seeker is the target | 3.4 |
| 62 / 69, kind 0x92 | speak | when the speech ends (ANIM-140, detail unknown) | 3.4 |
| 67 / 68 / 72 / 73, kinds 0x9C-0x9F | start / stop / activate / deactivate a cart | at once (`spec-script-vm.md` VM-231) | 3.8 |
| 93 / 94 | read / set the facing (`d mod 16`); setting writes the facing itself, so no turn follows | - | ANIM-212 |
| 96 / 156 / 152 | place off the map / into a building / out of a building | - | ANIM-220, ANIM-312 |
| 101 | the current action id, 283 when none | - | ANIM-102 |
| 103 | stop the actor (it ends its current movement) | - | 3.4 |
| 140 | the walking style the AI uses (0 walk, 1 run), i.e. which locomotion action id it chooses | - | ANIM-131 |
| 160 | the distance between two points, truncated to an integer | - | - |
| `ActionChange(current, previous)` | dispatched from the actor's update when the current action id changes, with that actor as the current actor; 283 means none | - | ANIM-102 |

## 7. Acceptance tests

Frame lengths below are the realised 46.875 ms of ANIM-002; a test that fixes the engine's tick to that value can
compare absolute times, a test on the nominal 40 ms must scale. "The hero" is the `RobinHood` profile, "a soldier"
the `Soldier A00` profile; every number quoted is read from the player's own files by the rules of ANIM-011, and
section 9 shows the reading.

1. **Frame display length.** A synthetic animation of three frames with hold values 0, 2, 0 played in mode 0,
   stepped once per frame from a fresh start: the frame index after successive steps must be 0, 1, 1, 1, 2, 0, 1,
   1, 1, 2, ... - frame 0 displayed once, frame 1 three times, frame 2 once, a cycle of 5 frames = the sum of
   `hold + 1`. The completion signal must fire on the step that lands on frame 2 (its hold is 0) and again one
   full cycle later. With the hold values 0, 2, 1 the signal must instead fire on the second display of frame 2,
   one step before the wrap.
2. **Walk cycle timing and speed.** The hero's walk block has 22 frames, every hold 0 and every advance 4 px:
   playing it must change the frame every frame, displace 4 px every frame, complete once every 22 frames
   (1.031 s) and average 85.33 px/s. The soldier's walk block (advance 2) must give 42.67 px/s, the hero's run
   (12 frames of 5) 106.67 px/s, the soldier's sprint (32 frames of 5) 106.67 px/s.
3. **Sneak is not uniform.** The hero's crouched walk has 14 frames with holds 2,2,2,2,1,1,1,1,1,1,1,1,1,1 and
   advances 1,2,2,2,2,2,2,2,2,2,2,2,2,2: the character must move **only on the frames where the animation frame
   changes** (10 of the 32 frames of the cycle carry no displacement), the cycle must last 32 frames (1.500 s) and
   the average speed must be 18.0 px/s. A test that moves the character every frame at the average speed passes
   the speed check and fails the per-frame positions; both must be checked.
4. **Turning.** A character facing 0 given target facing 5 must reach it in 5 frames, one step per frame; given
   target 11 it must reach it in 5 frames going the other way; given target 8 it must go **down** (15, 14, ...) and
   arrive in 8 frames. While turning, a walk whose advance is 4 px must displace `4 x 0.6 = 2.4` px per frame, and
   a walk whose advance is 1 px must displace 0.7 px (the floor), not 0.6.
5. **Turning does not restart the clip.** Walking with the frame index at 7 and the timer at 0, change the target
   facing: the frame index and the timer must be unchanged on the next frame and only the facing (and hence the
   animation within the block) may differ.
6. **Script animation elements.** A one-level sequence of native 49 on an actor whose profile has the action must
   complete after exactly the clip's length in frames; the same with native 50 must never complete (the sequence's
   next level must never start); with native 51 the element must complete after the clip and the actor must then
   hold the clip's last frame for ever.
7. **Walk element completion.** A walk order to a point 100 px away in a straight line, at walk speed 85.33 px/s,
   must complete on the first frame where the distance falls below `radius + 5` px, and the script's sequence
   level must advance on that same tick.
8. **The idle chain is stochastic.** With a seeded stream, an idle actor must switch to the fidget action at the
   end of an idle cycle exactly when the drawn value is a multiple of 10, and back to idle at the end of the
   fidget. The number of stream draws per frame must match: one per idle-cycle completion, none otherwise.
9. **Camera: key scrolling accelerates and coasts.** Holding "scroll right" from rest must move the camera 6 px on
   the first frame and reach 32 px per frame after about 21 frames, at zoom 1.0; at zoom 2.0 the same commands must
   move half as far in world pixels, at zoom 0.5 twice as far. Releasing the key must decelerate down the same
   ramp instead of stopping instantly. Reversing the direction must restart at 6 px.
10. **Camera: the script scroll.** Native 42 with speed 10 to a point 105 px away must take 11 frames (ten steps
    of 10 px and one shortened step of 5) and complete on the eleventh; with native 33 (speed 0) the first step
    must be 2 px and the following steps must follow the ramp. A scroll whose straight line leaves the map must
    complete early, at the border, not hang.
11. **Camera: 18 and 19 do not zoom.** After native 18 the zoom must be unchanged and the next default-speed
    scroll must begin with a 2.0 px step; after native 19 with 7.0 the next default scroll must begin with a 7.0 px
    step. A test that asserts a zoom change after 18 encodes the current engine's error.
12. **Camera: the zoom element.** Native 35 with 0.5 from 2.0 must complete after 17 frames (two 8-frame
    animations plus one) and must complete immediately - not hang - when the map is too small for the wider view.
13. **Depth order.** Two characters at the same world row must be drawn in ascending element id; a character
    carrying another must be drawn immediately before or after it (the 0.001 offset) with nothing in between; an
    effect attached to a character must stay adjacent to it (0.01); a character and a building whose sort line
    passes between them must be ordered by the side of that line, not by the depth key.
14. **Ground marks.** An order marker must advance one animation frame every second frame and disappear after 12
    frames; many orders in quick succession must all keep their marks (there is no cap).
15. **Determinism of the tick.** Two runs of the same replay must produce identical positions: the element update
    order is the element table's index order with the count re-read each step (ANIM-101), the cart/human pair scan
    is the fixed descending double loop (ANIM-305), and the only stream draws are the ones listed in section 8.
16. **Slow motion is a pure time scale.** With slow motion on, every test above must pass with all times
    multiplied by ten and all per-frame values unchanged.

## 8. Implementation choices, snapshot, RNG and departures

**What is the original's behaviour** (must be reproduced): everything in sections 2 to 6.

**Snapshot contract.** A snapshot or save must carry, per element: the animation index, the frame index, the frame
timer, the current action id, the action id the timer was last reset for, the end-of-clip (frame, timer) pair, the
table-replacement flag, the depth key, the facing, the target facing, the turn delay and hysteresis counters, the
position, the previous position, the layer and sector word, the projection area, the ground kind, the movement
direction and its height component, the reverse / no-collision / off-map / visibility flags, the blocked-move
counter, the action element queue with each element's state, and the per-actor action id last reported to the
script. Per cart, additionally: the program position, the remaining length of the current block, the current
speed, the target speed, the acceleration and the waypoint index. Per level: the camera corner, the zoom, the zoom
animation step, the scroll target, the scroll step length and ramp index, the scroll speed, the lock target and
its per-axis remainders and burst counter, the ground mark list. Reasons: every one of these is read by a later
frame, and the frame timer, the end-of-clip pair and the turn counters are the ones a naive snapshot omits
(ANIM-020, ANIM-021; the original itself saves them, 005b5ed0 and 0055fe10).

**RNG contract.** This subsystem draws from the single global stream of `spec-ai-combat.md` AI-005/AI-006 in
exactly these places, and the order within a tick is fixed by the element update order of ANIM-101: (a) the idle
to fidget roll, once per completed idle cycle of an actor (ANIM-130); (b) the stochastic idle of play modes 4 and
5, once per frame per element that is sitting at the start of such a clip (ANIM-040); (c) the cart program's
weighted branch, once per block (ANIM-302); (d) the building-entry waits of the walk pipeline
(`spec-navigation.md` NAV-130), two draws per wait. Nothing else in animation, movement, the camera or the drawing
consumes the stream. An implementation that uses separate named streams (our determinism device) must keep the
per-element order so that a replay is reproducible.

**Deliberate departures OpenSherwood should take, each recorded as such:**

1. **Fix the frame length at 46.875 ms** (64/3 Hz) rather than 40 ms. The original asks for 40 ms and gets 46.875
   ms on the host's default counter granularity (ANIM-002); every duration in the shipped data was authored
   against what the authors saw, and every oracle recording measures 46.875 ms. Running at 40 ms makes the whole
   game 17 % faster than the recordings. The choice must be one constant, documented, with the 40 ms value noted.
2. **Treat play mode 1 (the looping animation element, native 50) as never completing**, as the original does, and
   surface it as a diagnostic when a mission's sequence blocks on it, rather than "helpfully" completing it.
3. **Return a null handle rather than reading out of bounds** wherever the original is unchecked (the mode-2 frame
   index one past the end, ANIM-042; the cart's waypoint index), and log.
4. **Keep 16 facings everywhere**: the current engine's 8-way direction set cannot express the animation blocks or
   the turning rule and must go.
5. **Reproduce the arithmetic of ANIM-202** only if a class that uses movement mode 3 is found; until then refuse
   mode 3 with a diagnostic instead of guessing.
6. **The screen-edge scroll** (ANIM-324) is not settled; until it is, implement it with the same ramp and step
   rule as the key scroll and mark the border width as an assumption.

## 9. Validation against the data and the recordings

Checks run in this session with `harness/tools/probe/anim_actions.py --table` on
`C:\Users\przem\source\gamedata\robinhood\DATA\Characters` (read only; the tables stay in the analyst workspace):

- The hero's walk block: 22 frames, all hold values 0, all advances 4 px. Under ANIM-003 and ANIM-031 that is one
  frame change per frame, a cycle of 22 frames = **1.031 s** and **85.33 px/s**. The oracle recording of
  2026-09-05 measured 22 frame changes in **1.044 s** (`stealth-and-combat.md` 8.1), i.e. 47.5 ms per frame -
  within 1.4 % of the 46.875 ms of ANIM-002 and incompatible with 40 ms (which would give 0.88 s). **The
  frame-length verdict rests on this.**
- The hero's crouched walk: 14 frames, hold values summing to 18, advances summing to 27 px. ANIM-031 gives a
  duration of `18 + 14 = 32` frames = **1.500 s**, which is exactly the measured 1.50 s, and **18.0 px/s** against
  the measured 17.8. The same reading with "one frame per hold value, at least 1" gives 0.84 s and fails.
- The hero's run: 12 frames of 5 px, holds 0 -> 106.67 px/s (measured 101 +/- 10). The soldier's walk: 22 frames of
  2 px -> 42.67 px/s; alert walk (22 of 3) 64 px/s; alert run (12 of 4) 85.33 px/s; sprint (32 of 5) 106.67 px/s.
  All agree with `docs/formats/sprite-animations.md` rule 3, which derived them from the same files.
- The marker-frame candidate of ANIM-012 equals `frame count - 1` on every block checked here (idle, fidget, walk,
  run, sprint, crouched walk, both climbs), consistent with the 112 608 of 148 512 animations counted in
  `sprite-animations.md`. Play mode 7 rotating on the **last** frame of a clip is behaviourally sensible for the
  zero-duration turn-table blocks, which supports the reading; the field identification itself stays medium.
- The climb blocks: up and down are 12 frames with hold values summing to 16 and advances of +/- 3 px, with a
  per-block displacement of 45 px - consistent with `spec-navigation.md` NAV-200 taking the number of loop
  repetitions from the table rather than from the map, and with the climb covering 36 px per repetition.
- No contradiction was found between the code and the shipped animation tables. The one place where the data
  contradicts a *previous* document is the frame clock: the 64 Hz / three-clocks reading of
  `stealth-and-combat.md` 8 and `anim.rs` produces the same durations as ANIM-003 only because 3 x 15.625 ms =
  46.875 ms; the program has no 64 Hz clock.

## 10. Open questions

1. **The sliding rule and the proximity callback** (ANIM-205): the second half of 00561040 after the straight-line
   test fails, and 00563e90 / 00564020 - 00564390 / 00560840; the per-class virtual that a nearby element's
   callback invokes (called at the site inside 00561040) decides pushing, stepping aside and waiting, and is the
   biggest remaining gap in movement. `spec-navigation.md` open question 2 is the same gap.
2. **The speech element's duration** (ANIM-140): the speech case of 00464b20 and the sound length source
   (005a87f0 and the text page data).
3. **Which field of the animation record is the marker frame** (ANIM-012) and which is the loop length (ANIM-013):
   005bdcd0 and 005bddb0 read the in-memory record; the mapping to the file fields of `docs/formats/sprites.md`
   needs one pass over the sprite loader (005b5ea0 and the sequence reader it calls).
4. **Which class passes movement mode 3** (ANIM-202): the other callers of 005b86b0 (00475bd0, 00481150) and the
   player-character executor 00470390.
5. **The cart instruction set** (ANIM-303): the two unread instructions (004af0f0, 004af060), the program's
   container in the mission file (004ae470 reads it), and the meaning of the per-sub-sprite value of ANIM-301.
6. **The screen-edge scroll** (ANIM-324): which routine turns a mouse position near a border into the four scroll
   commands, and the border width; start at the HUD construction 0050b640 (it creates the border bindings), the
   event pump around 005105d0 and the command dispatch table at 004dcdac.
7. **How the level's mask data becomes the drawable scenery pieces** (ANIM-366) and how the scenery list is
   ordered at load: 004c0510, 00523df0, 004c2720.
8. **The footstep ground kind** (ANIM-207): the field 005b86b0 compares with 5 is the one the placement reader
   fills from the mission record's ground byte, but the same offset is written as a float by the cart code
   (004ab7e0), which was not reconciled.
9. **The end-of-clip marker's purpose** (ANIM-034): why the pair is adjusted to the second-to-last frame, and
   which actions rely on status 0 rather than status 3 (00475bd0 has more cases than were read).
10. **Whether the two wide resolutions are original** (ANIM-380): 005e3f70 and the mode enumeration near 005e3b10.
11. **The cart run-over condition** (ANIM-305): which motion test selects 50 over 10 damage (004d9420 and the
    geometric helper it calls).

## 11. Differences from the current engine

Against `crates/opensherwood-core/src/anim.rs`, `world.rs`, `crates/opensherwood-render/src/lib.rs`,
`crates/opensherwood-app/src/engine.rs`, `docs/formats/sprite-animations.md` and
`docs/original/stealth-and-combat.md` 8:

1. **There is no 64 Hz animation clock and no sub-tick unit system.** `anim.rs` runs a 60 Hz world tick with 16
   units per tick and 45 per table tick to approximate a 64 Hz clock. The original has one clock - the frame - and
   a frame of the animation table lasts `hold + 1` frames of it (ANIM-003, ANIM-031). The whole
   `CLOCK_HZ`/`UNITS_PER_TABLE_TICK` apparatus should be replaced by a per-element integer frame counter on a
   21.333 Hz tick; the durations come out identical because 3 x 15.625 ms = 46.875 ms, but the engine's rounding
   (`ceil(45 t / 16)`) drifts against the original's exact frame counting.
2. **Movement is not a constant average speed.** `AnimSet::cycle_speed` moves the entity at the cycle's average
   px/s; the original displaces the frame's own advance on the frames where the animation frame changes and
   nothing on the others (ANIM-032). For the uniform walk and run cycles the two agree; for the sneak, the climbs,
   the decelerating stops and every action with hold values they do not, and the difference is visible as
   stepping motion and as different arrival frames.
3. **Eight directions instead of sixteen.** `anim.rs` keys its animation sets by an 8-way direction
   (`direction_of`, `[u32; 8]` per action). The original indexes a block of 16 by the facing directly (ANIM-010)
   and turns one sixteenth per frame (ANIM-210); with eight directions neither the sprite selection nor the
   turning timing can be right.
4. **Turning has a cost and a rule.** The engine turns instantly. The original turns one (or two) of 16 steps per
   frame along the shorter arc, breaks ties counter-clockwise, scales the advance by 0.6 with a floor of 0.7 px
   while turning, and does **not** restart the animation (ANIM-203, ANIM-210, ANIM-214).
5. **Animation elements complete on real conditions.** The engine completes natives 49/50/51 at once (the VM
   spec's difference 11). The original completes 49 at the end of the clip, **never** completes 50, and 51
   completes and then leaves a permanent freeze element (3.4). Missions that wait on those elements behave
   completely differently.
6. **The camera is not a clamped integer pair with a fixed step.** `world.rs` scrolls by a constant 8 px per key
   frame and clamps to the full 1024x768. The original ramps 6 -> 32 px per frame with acceleration and
   deceleration, divides the step by the zoom, and clamps to `map - (screen / zoom)` with the world view **80 px
   shorter** than the screen (ANIM-320, ANIM-323, ANIM-380).
7. **Natives 18 and 19 set the scroll step length, not the zoom** (ANIM-331); the current engine's "deployment
   area" and "zoom" readings are both wrong, and the VM spec's row for 19 needs the same correction.
8. **Zoom is animated over 8 frames and has three fixed values**; the zoom element completes only when the zoom
   has arrived (ANIM-326, ANIM-333). The engine treats zoom as instant.
9. **The camera follow keeps the offset it was given**, matching the actor's own speed rather than centring or
   easing (ANIM-340); no engine equivalent exists.
10. **Occlusion is ordering, not masking.** `opensherwood-render` hides characters by clipping them against
    occluder masks with a depth line. The original has one merged draw list per frame: movables sorted by world
    row (ties by element id), merged into the level's own scenery order by a per-scenery sort line (ANIM-363,
    ANIM-364). The mask data exists, but the visible result comes from the order. The engine's depth line is the
    right idea in the wrong place; the sort key must also be the **world** row, not the screen row.
11. **Effects and decorations are depth-sorted into the same list** through a deferred queue with 0.01 / 0.001
    offsets and a 1000.1 "always in front" case (ANIM-364, ANIM-365); the engine draws them in separate layers.
12. **Ground marks exist and are a distinct pass** between shadows and elements, with a 12-frame life and no cap
    (ANIM-370); the engine has none.
13. **No dirty rectangles, no letterboxing, 688 px of world**: the original redraws the whole view every frame,
    keeps a scroll-shifted backdrop cache, and gives the world `height - 80` px at 1024, 1228 or 1360 px wide
    (ANIM-361, ANIM-380). `engine.rs` uses 1024x768 for the whole frame.
14. **The per-element update order is the element table's index order with the count re-read each step**, and a
    dead element is removed in the middle of the pass (ANIM-101); the engine iterates a stable snapshot of its
    entity list.
15. **Carts are speed-driven with a scripted program and a weighted random branch** (3.8), not animation-driven;
    they run people over through a pairwise scan at a fixed point of the tick (ANIM-305). The engine has neither.
16. **There is no difficulty setting to honour** (ANIM-523), and slow motion is a pure x10 time scale
    (ANIM-520).

## 12. Provenance

- Ghidra project `re/ghidra/robinhood` (never committed); exported decompilation `re/out/decomp_all/<address>.c`,
  function inventory `re/out/inventory.tsv`, string references `re/out/strings.tsv`, module map
  `re/notes/modules.txt`, all produced by the committed export scripts under `scripts/ghidra/` and all
  git-ignored. Data bytes read with `scripts/ghidra/peek.py`; the arithmetic of ANIM-032, ANIM-200 to ANIM-203 and
  the argument order of the play calls were confirmed against the raw instructions with a local capstone
  disassembly helper kept in the analyst workspace.
- Functions read: section 0. Notes: `re/notes/anim/` (git-ignored).
- Data checked: `harness/tools/probe/anim_actions.py --table` on the character profiles of the read-only game copy
  at `C:\Users\przem\source\gamedata\robinhood` (section 9). No oracle run in this session; the timing comparison
  uses the recordings of 2026-09-05 reported in `docs/original/stealth-and-combat.md` 8.
- Sibling specifications cross-referenced: `docs/original/spec-script-vm.md`, `docs/original/spec-navigation.md`,
  `docs/original/spec-ai-combat.md`; formats `docs/formats/sprites.md`, `docs/formats/sprite-animations.md`.
- Tests that will depend on this document: the animation and movement rebuild, the camera, the draw order and the
  acceptance cases of section 7 (roadmap item "movement and camera", ADR-0009).
- Identity and exposure: analyst = this session (2026-09-13), analyst role only; it read decompilation for the
  animation, movement, camera and drawing subsystems and must not implement them. Two delegated readers of the
  same workspace and the same role (one for the camera, one for the drawing order and the settings) contributed
  the findings of 3.10 to 3.12 and are part of this session's exposure record; their notes were reworded here and
  their raw output stays outside the repository. Spec reviewer: pending (Codex, task B). Publication approval:
  pending.
