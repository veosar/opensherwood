# ADR-0008: The script VM is authoritative core state; `opensherwood-script` only translates

Date: 2026-09-02. Status: accepted.

## Context

Missions are driven by compiled `SCB` scripts (`docs/formats/scb.md`): per-class functions with callbacks
(`Initialize`, `PostInitialize`, `Hourglass`, `ProcessMessage`, `EnterZone`, ...) calling ~230 natives that
read and mutate the mission (objectives, texts, sequences, activation, patrols, camera, timers, variables).
ADR-0001/0002 say that `opensherwood-core` owns every authoritative state and its snapshot / hash, while the
crate layout put `opensherwood-script` above core. The Codex full review (H-10) asked for the ownership to be
settled before any interpreter is written: a VM living above core cannot be snapshotted or hashed by core.

## Decision

1. **The VM lives in core.** `opensherwood-core::vm` defines the instruction set as plain data (a small
   typed IR: immediates, moves, arithmetic, comparisons, branches, calls, returns, native calls with argument
   and result slots), the program representation (classes, functions, variables), the run-time state (frames,
   temporaries, class variable storage, scheduler of pending callbacks, sequence state) and the interpreter.
   All of it is fixed point or integers, deterministic, part of `World`, of the snapshot (`scripts` and
   `scheduler` hash parts) and of `validate`.
2. **Natives are core functions.** A native call is dispatched by number to a function of `World`
   (`opensherwood-core::natives`) that acts on entities, objectives, timers, texts and camera through the same
   code paths the player's orders use. Natives with a documented effect the engine does not model yet are
   *stubs*: recorded no-ops counted per id. A native with no documented row is *unknown*, and calling one is a
   deterministic trap by default: the id is counted, the running callback stops at that instruction (its
   frames are discarded, later callbacks still run) and the script is marked `faulted`, observable through
   `observe.script` and `debug.vm`; the call sites are counted per mission at load. The permissive behaviour
   (unknown native = recorded no-op returning 0) exists only behind the explicit `lenient_natives` flag of the
   mission spec (`--lenient-natives` on the binary), and in that mode every unknown call is appended with its
   arguments to the VM state, snapshotted and hashed. Unimplemented behaviour is visible, never silent.
3. **`opensherwood-script` translates.** It depends on `opensherwood-formats` and `opensherwood-core` and
   converts an `Scb` container into the core IR (opcode mapping, calling convention, element index spaces per
   `docs/formats/scb.md`), validating jump targets, function addresses, variable slots and parameter counts.
   It contains no execution logic and no state.
4. **Semantics come from the spec.** Every opcode and native implemented cites its row in
   `docs/formats/scb.md` (confidence level included). Low-confidence readings are implemented behind a
   documented choice and a test that pins the chosen behaviour, so a later correction is a deliberate ruleset
   bump, not a silent change.
5. **Modding path.** A later Lua front end (ADR-0006) targets the same core IR / native table; the retail
   scripts stay the reference behaviour.

## Consequences

- `World` grows a `vm: Option<VmState>` (program, class variables, mission variables, objectives, message
  queue, sequences with their completion tokens, pending texts, camera target, patches, attributes, states,
  the `script` RNG stream) and entities gain `active` / `ai_locked` flags; ruleset 6, snapshot schema 9 and
  hash schema 8 (2026-09-02, Codex review 5); ruleset 7 for the budget scope and charges below (Codex review 6,
  canonical bytes unchanged); ruleset 10, snapshot schema 13 and hash schema 12 (2026-09-05, Codex review 7:
  the native signature table, the action-change queue, the assumption set and the stealth layer's cursor);
  ruleset 11, snapshot schema 14 and hash schema 13 (2026-09-05, the oracle measurements applied: speeds
  from the profiles' cycles on the measured animation clock, the animation state's `elapsed` in clock
  units, the noise channel's immediate charge and the entity `heard` flag); ruleset 12, snapshot schema
  15 and hash schema 14 (2026-09-05, Codex review 8: the dependency-closed taint registry, transactional
  action-change delivery with the queue overflow as a fault, one simulation budget with a cursor per
  phase, the native call fused with its result read); ruleset 13, snapshot schema 16 and hash schema 15
  (2026-09-05, the melee of `docs/original/combat-measurements.md`: hit points, energy, the fight state
  and poses, death, `hero_dead`, the press for the drawn figures, the assumption sources `MeleeReach`,
  `PowerfulBlowChance`, `PostBound`, `CombatActions`, `HeroDeathLoss`); ruleset 14, snapshot schema 17 and
  hash schema 16 (2026-09-05, Codex review 9, findings 1 / 3 / 4 / 5: the engine's own hypothesis sources
  recorded where the rule first mutates state (`SightCone`, `NoiseRadius`, `AlertPolicy`,
  `AttackPolicy(rule)` replacing `Perception` / `MeleeReach` / `PowerfulBlowChance` / `PostBound`, `KnockOut`
  widened), the script call fused with its result read, per-phase quotas of the simulation budget with
  cursors for the movement, the animation advance and the action-change scan, the obstacle index).
- The `scripts` / `scheduler` hash parts stop being zero placeholders.
- **What is authoritative and what is not.** `VmState::counters` (instructions, callbacks, budget aborts,
  faults, traps, message and text drops, per-id native counts) and `VmState::budget` (the work left in the
  current tick) are diagnostics: they are neither serialised nor hashed (`#[serde(skip)]`), a restored world
  counts afresh and `debug.vm` reports the live values only. Everything the scripts can observe stays in the
  snapshot and the hash. Callbacks never yield, so a snapshot is *quiescent*: `validate` refuses frames,
  pushed arguments or a sequence still being collected instead of pretending to resume them; the
  interpreter guarantees it through one teardown path (`vm::teardown`) that every callback exit takes
  (return, budget abort, fault, trap), and `Program::validate` rejects programs whose parameter / argument
  stacks are not balanced (a worklist walk per function: a call or native needs its `argc` values pushed,
  a return needs both stacks empty, join points must agree).
- **One work budget per tick** (`vm::WORK_BUDGET_PER_TICK`, 2^22 units), granted at the start of `vm_tick`
  and nowhere else: the event hooks (`IsTaken`, `ReachPoint`, `ActivatedBy*`, `ActionChange`) and
  `vm_dismiss_text`, which the app calls between ticks, draw from what the current tick left (after an
  exhausted tick a dismissal removes the page and the sequence behind it continues next tick); the load-time
  run of `attach_script` has its own `WORK_BUDGET_AT_LOAD`, whose remainder serves the dismissals of the
  briefing pages before the first tick. Charged: instruction dispatch, every argument a call or native
  transfers, every entity a zone or scroll scan looks at plus one unit per polygon edge tested, natives 97
  (edges) and 204 (entities plus edges per player character) before they scan a borrowed polygon, sequence
  elements, and every stage of the walks the script issues (`nav.rs`: search initialisation at one unit per
  64 cells charged before the arrays are allocated, A* expansions, unwound cells, line-clear cells and
  smoothed output points; `world.rs`: the conversion of the final path, allocated fallibly). An exhausted
  budget stops the tick deterministically (the running callback is aborted, later phases wait for the next
  tick, undelivered messages stay queued ahead of new ones) and is counted; it is part of the ruleset because
  it changes what a tick does. The rest of the simulation draws from **one simulation budget per tick**
  (`world::SIM_WORK_PER_TICK`, 2^24) handed out **phase by phase on deterministic quotas** (Codex review 9,
  finding 4; `world::SimBudget`): a pre-index pass charged one unit per entity (at most `MAX_ENTITIES`),
  then perception (`SIM_QUOTA_PERCEPTION`, the remainder: about 2^22 + 2^21), the state transitions, the
  attack orders and the waypoint programs (2^21 each), the movement against the obstacle index and the
  walkable geometry (2^20: one unit per mover, per index cell looked at, per obstacle candidate tested and
  per polygon edge tested), the animation advance and the action-change scan (2^20 each, above
  `MAX_ENTITIES`, so they always complete). A phase is granted its quota plus what the phases before it
  left unused, never more than the budget has left, so no phase can starve another whatever the
  snapshot holds (a `MAX_ENTITIES` world of half player characters and half soldiers, 2^30 perception
  pairs a round, still runs every timer, plans a bounded, growing number of attack and program walks and
  moves every mover on every tick: `every_phase_keeps_its_quota_in_the_largest_hostile_snapshot`).
  Each phase walks its own entity list from a persisted cursor (`World::cursors`: perception, states,
  attacks, programs, movement, animation, actions; snapshotted, hashed under `world`, validated); when
  its grant runs out the cursor stays on the entity not served so the next tick starts there, and when
  it ran out on the first entity of a walk that had the whole quota the cursor moves past it (one entity
  too expensive for a quota blocks nobody). The quotas rather than a persisted global phase cursor
  because the stimuli perception hands the transitions are not state: resuming a tick mid-sequence would
  have to persist them. Path searches the simulation issues (an alert run, a return, an attack
  approach, a program's walk) are capped per search at `world::SIM_SEARCH_WORK` (2^20, below every
  search-issuing quota): a search that fails with the full cap is unreachable under this budget (a
  definite answer: the order is dropped, the instruction skipped, the soldier patrols where he stands),
  one that fails with less is retried first next tick. Only the player's own click orders keep a
  per-order budget of `ORDER_SEARCH_WORK`, one per click. The obstacle entities are queried through a
  spatial index (`world::ObstacleIndex`: grid buckets of `OBSTACLE_CELL` = 64 px keyed by cell, a CSR
  layout, positions outside the map folded into the edge cells), derived from the entities, never
  serialised, rebuilt by the tick whose pre-index finds the obstacle boxes changed; its size is bounded
  by `MAX_OBSTACLE_INDEX_ENTRIES` (2^22), which `validate` enforces so the rebuild stays a bounded,
  uncharged refresh like the navigation grid's. Navigation has no fail-open entry point:
  `World::try_ensure_nav` is the only way to rebuild a missing grid and every caller handles its error.
- **Trust boundary.** `Program::validate` in core is self-sufficient (functions in table order from address 0
  with their prologue, jumps inside their function, parameter reads and call arities against the table,
  arities within the stack limit, every native call site of a known id with the argument count of the one
  signature table `natives::NATIVE_SIGNATURES` (`id -> arity, returns_value, read_in_corpus`: the arity and
  the `-> result` contract from the spec's rows, the corpus observation of a `0x0d` kept as its own column)
  and a result slot only on a native that leaves a value, aggregate code / vertex bounds, element and location
  coordinates within `+-2^20`); the translator's checks are earlier diagnostics. The native call and its
  result read are **one IR instruction** (`Instr::Native { id, argc, dst }`): the translator fuses a `0x0c`
  with the `0x0d` after it (the `0x0d` quad becomes a `Nop` so quad indices stay instruction indices) and
  refuses a jump whose target is a `0x0d` quad, so no control flow can reach a result read without the
  call that produces it (Codex review 8, finding 6); frames hold no native result. The script call and
  its result read are fused the same way (Codex review 9, finding 3): `Instr::Call { function, argc,
  dst }` carries the destination of the `0x0a` after it (the `0x0a` quad becomes a `Nop`), the translator
  refuses a `0x0a` without a `0x05` before it, after a call of a function without a result, and any jump
  whose target is a `0x0a` quad (direct, from a divergent predecessor, as a loop's entry);
  `Program::validate` refuses a destination on a callee without `has_result` (and a call of a function
  outside the table), and the callee's value is written to the destination when its frame returns, so a
  frame holds no call result either and no fabricated value can feed a branch.
  The dispatcher checks the signature again: a call whose argument count differs traps like an unknown native
  (`counters.arity_mismatches`), so a required argument never defaults to 0. `World::validate` also requires
  the gameplay and `script` RNG streams to derive from the world seed with their assigned ids (1 and 2), and
  the stealth layer's invariants (`Dead` and `alive` agree, timed states carry their timer, attack orders go
  from a player character to an enemy soldier, alert states belong to enemy soldiers).
- **Hypotheses and taint.** The retail scripts run over recorded stubs and over engine hypotheses; the
  movement speeds, the animation clock and the noise channel (a running character heard from 330 px and
  more, the soldiers charging at once) are measured (`docs/original/stealth-and-combat.md` 8) and record
  nothing. The taint is **dependency-closed by construction** (Codex review 8, finding 1): the sources are
  a registry, the `vm::Assumption` enum, and every place in the core that takes a hypothesis records its
  variant in `VmState::assumptions` (a `BTreeSet`, snapshotted, hashed under `scripts`, validated:
  every entry must be well formed for its source) at the point where the hypothesis is taken, whether or
  not the script reads a value there: it is conservative, not a data-flow analysis. The sources:
  `Opcode(op)` when an instruction of a low-confidence opcode executes (`vm::LOW_CONFIDENCE_OPCODES`:
  `0x24` read as `>=`, `0x28` as `!=`, `0x2b` as a fixed-point `<`, `0x14` rounded to 24.8; the translator
  keeps `0x24` apart from the medium-confidence `0x26` as `BinOp::GeLow`) and `UnresolvedJump` when a jump
  to `0xffff` leaves its function (`Instr::LeaveUnresolved`); `Policy(id)` on every call of an
  implemented native whose reading is a policy rather than an observation (`natives::NATIVE_TAINT`,
  `Taint::Policy`: 8, 44, 45, 64, 93, 94, 98, 110, 128, 133, 134, 135, 140, 159, 161, 193, 194, 196, 204,
  245, each with its choice named in the table; the `Taint::Branch` rows 111 / 211 / 250 record it only
  with more than one player character and 240 only for a non-actor element); `StubResult(id)` on every
  call of a recorded stub with an effect the engine does not model (`Taint::Effect`, the never-win stubs
  included) and whenever a stub's fabricated result is consumed (the result slot of the fused native);
  only the stubs proven presentation-only record nothing on the call (`Taint::Presentation`: 62 an
  expression, 69 a remark before a dialogue line, 149 / 150 a level sound, 243 the cutscene highlight,
  each justified in the table); `UnknownNative(id)` on every lenient unknown call; the engine's own rules record their source **where
  the rule first mutates authoritative state, independent of any callback or later consumer** (Codex
  review 9, finding 1: a hypothesis-driven position can win a mission through an observed native such as
  97 with no `ActionChange` handler in sight): `SightCone` when the view cone's geometry decided a
  sighting and a soldier's state changed on it (he noticed, or his alert was refreshed), `NoiseRadius`
  when a run was heard from beyond the measured 330 px bound (`ai::NOISE_MEASURED_RADIUS`) and within the
  engine's 350 px (`RUN_NOISE_RADIUS`: an engine choice above the bound; a run heard within the bound is
  measured and records nothing), `AlertPolicy` when the noticed -> alarm -> search sequence, the alert
  timeout, the re-plan while searching or the return to the post (after an alert or a knock-out) mutated
  a soldier's state, `AttackPolicy(rule)` for the attack policy: `Reach` when an attack order resolved
  with the victim's back to the attacker (the reach bands and the arc deciding the knock-out blow or the
  fight; the frontal fight at the measured distance records nothing), `Block` when a player character's
  automatic strike started or resolved against a soldier (it never lands: inferred from one fighter
  pair), `HitChance` when a soldier's swing was timed with the engine's jitter or a blow was resolved by
  a roll (the soldier's two in three, the powerful blow's one in three from 2 of 6 strokes), `PostBound`
  when a soldier's foe left him alive and he stood his ground (measured for the halberdier only);
  `KnockOut` when the blow's effect (the fall, the timer and its scaling, the immunity threshold)
  changed the victim's state, and when native 90 reported a knock-out, 128 refused one or a knock-out
  action id was delivered to `ActionChange`; `ProfileStats` when a blow consulted the knock-out
  resistance; `CombatActions` when a melee action id or a dead actor's fall reached an `ActionChange`
  handler (the ids are read by eye), `HeroDeathLoss` when a player character's death raised the loss
  while another one was alive (measured for a lone hero); the measured constants (the speeds, the
  animation clock, the noise channel within its bound and the immediate charge, hit points, energy,
  damage, cadence, the fighting distance, the blow's timing, the attack order, death and the lost page)
  record nothing; `TickRate` when a native-56 wait ran (in a sequence or outside one) or
  `Hourglass` read its time; `ScrollPickup` when `IsTaken` fired (the pickup radius and the
  take-on-non-zero rule); `ZoneAtLoad` when a zone callback fired on the first scan for a character
  standing inside at load; `WalkCompletion` when a barrier was released by a walk that did not arrive;
  `ActionChangeOrder` on every `ActionChange` delivery (the parameter order); `CampaignGraph` when the
  app's successor rule picked the next mission (`World::record_assumption`); `LenientAssets` when the app
  built the spec with a fallback (`MissionSpec::assumptions`). The set only grows (a rolled-back
  transaction keeps what it recorded). `mission_won` / `mission_lost` stay recorded, but
  `ScriptObservation::tainted` (the set is non-empty) marks the outcome as **not authoritative**: it
  proves consistency with the hypotheses, not the original's behaviour, until the oracle captures of
  `stealth-and-combat.md` section 7 settle them; an outcome with an empty set took no hypothesis the
  engine knows of (`a_charge_from_the_unmeasured_noise_band_taints_a_win_read_from_native_97`: the
  same charge from 320 px wins untainted, from 340 px tainted by `NoiseRadius` alone, through a JSON
  snapshot and checkpoints every 50 ticks). Strict mode keeps trapping unknown ids; the taint is what
  strict mode says about the known ones. Under this model the retail missions are tainted from their load-time callbacks on (the
  first mission's `Initialize` locks doors and hides actors through effect stubs and sets action
  availability and AI locks through policy natives), which is the honest reading until those rows are
  implemented or observed.
- **Action changes are delivered exactly once, transactionally.** Every change of an actor's reported
  action id is queued in `VmState::pending_action_changes` (snapshotted, hashed under `scheduler`,
  validated) and delivered to the class bound to the actor within what the tick's budget left; a change
  whose class has no handler is dropped as undeliverable, one whose handler returned (or trapped: it
  would fail the same way again) is removed. A queued handler runs as a transaction (`vm::Transaction`,
  Codex review 8, finding 3): before it starts, the VM's mutable state (class and mission variables,
  objectives, queues, sequences, texts, money, patches, attributes, states, the script RNG) is captured
  at one work unit per value copied, and the entities its natives touch (`World::vm_touch_entity`), the
  selection and the camera are captured as they are touched; one the budget cut short
  (`CallOutcome::Exhausted`) is rolled back to that capture and stays at the front to be delivered at
  the start of the next tick, after the messages and before `Hourglass`, running whole from the state it
  saw the first time. A capture that does not fit the budget waits like an exhausted handler. A full
  queue is a deterministic fault (`VmState::fault = ActionQueueOverflow`, sticky, hashed; `faulted` is
  now derived from `fault`, which also names an unknown native or an arity mismatch), never a silent
  drop.
- **Campaign money.** `MissionSpec::starting_money` (100 by default) is applied to `VmState::money` before
  `Initialize` runs, so a script that sets it (H10's native 237) wins and nothing overwrites it afterwards;
  the app seeds it from the player's profile at load, never at install.
- **Sequences.** Native 32 is a barrier: walks (45 / 48 / 64) and animations (49..=53, stubs) issue completion
  tokens, the barrier holds the sequence until every token issued since the previous barrier completed (a
  walk completes when the entity arrived, gave up, was ordered elsewhere, deactivated or died: hypothesis,
  `docs/formats/scb.md`, "Engine notes"). Native 203 pages hold their sequence directly; native 202 texts
  never block anything, and `VmState::pending_text_requests` / `ScriptObservation::text_requests` expose the
  flag so the app can show a 202 text without pausing (`pending_texts` stays for compatibility).
- The synthetic corridor has no script: its hashes only change through the schema bump.
- The app dismisses the script's text pages through `World::vm_dismiss_text` when the briefing parchment is
  closed by canonical input (Enter, Escape or a click on the page); `debug.vm` only inspects. Replay time is
  the session tick (ADR-0004, protocol 4), so the dismissals are recorded and replayed as the key and pointer
  events they are, the tick-0 checkpoint fixes the state after `PostInitialize` and before the first
  dismissal, and a mission replay started right after `reset` reproduces the pages, the walk and the pause
  menu (`harness/tests/data/test_script.py`, `test_mission_replay_round_trip_from_the_first_page`).
- `opensherwood-script` keeps its place in the dependency graph (above core), which stays acyclic:
  core defines the IR, script produces it, the app hands it to core at mission load.
