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
  units, the noise channel's immediate charge and the entity `heard` flag).
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
  it changes what a tick does. Player and rail-program orders use their own per-order budget
  (`world::ORDER_SEARCH_WORK`). Navigation has no fail-open entry point: `World::try_ensure_nav` is the only
  way to rebuild a missing grid and every caller handles its error.
- **Trust boundary.** `Program::validate` in core is self-sufficient (functions in table order from address 0
  with their prologue, jumps inside their function, parameter reads and call arities against the table,
  arities within the stack limit, every native call site of a known id with the argument count of the one
  signature table `natives::NATIVE_SIGNATURES` (`id -> arity, has_result`, derived from the arity column of the
  spec's rows) and every native result read directly after a call that leaves one, aggregate code / vertex
  bounds, element and location coordinates within `+-2^20`); the translator's checks are earlier diagnostics.
  The dispatcher checks the signature again: a call whose argument count differs traps like an unknown native
  (`counters.arity_mismatches`), so a required argument never defaults to 0. `World::validate` also requires
  the gameplay and `script` RNG streams to derive from the world seed with their assigned ids (1 and 2), and
  the stealth layer's invariants (`Dead` and `alive` agree, timed states carry their timer, attack orders go
  from a player character to an enemy soldier, alert states belong to enemy soldiers).
- **Hypotheses and taint.** The retail scripts run over recorded stubs and over engine hypotheses (the
  view cone and the noticed / alarm sequence a sighting starts, the profile stats `p0` / `p4`, the
  25-versus-60 reading of the scripts' time unit, the campaign graph, the lenient asset fallbacks); the
  movement speeds, the animation clock and the noise channel (a running character heard from 330 px and
  more, the soldiers charging at once) are measured (`docs/original/stealth-and-combat.md` 8) and record
  nothing. Whenever a script-visible value depends on a hypothesis, the VM records an
  `vm::Assumption` in `VmState::assumptions` (a `BTreeSet`, snapshotted, hashed under `scripts`, validated:
  a `StubResult` must name a stub): `StubResult(id)` when the script consumes a stub's result
  (`GetNativeResult` after a policy-valued or zero-valued stub) or calls a never-win stub
  (`natives::NEVER_WIN_STUBS`); `Perception` / `KnockOut` when the stealth layer changed script-visible state
  (an alert action id of an actor alerted by sight, never of one alerted by a run heard (`Entity::heard`),
  or a knock-out action id delivered to `ActionChange`, native 90 reporting a knock-out, 128 refusing
  one); `ProfileStats` when a blow consulted the knock-out resistance; `TickRate` when a native-56 wait ran
  or `Hourglass` read its time (the animation clock being measured says nothing about the unit the
  scripts count in); `CampaignGraph` when the app's successor rule picked the next mission
  (`World::record_assumption`); `LenientAssets` when the app built the spec with a fallback
  (`MissionSpec::assumptions`). `mission_won` / `mission_lost` stay recorded, but `ScriptObservation::tainted`
  (the set is non-empty) marks the outcome as **not authoritative**: it proves consistency with the
  hypotheses, not the original's behaviour, until the oracle captures of `stealth-and-combat.md` section 7
  settle them. Strict mode keeps trapping unknown ids; the taint is what strict mode says about the known
  ones. In a normal run of the first mission the set holds `StubResult(235)` (the purse object's "taken"
  predicate the steward objective polls) and `TickRate` from the first tick after the briefing.
- **Action changes are delivered exactly once.** Every change of an actor's reported action id is queued in
  `VmState::pending_action_changes` (snapshotted, hashed under `scheduler`, validated) and delivered to the
  class bound to the actor within what the tick's budget left; a change whose class has no handler is
  dropped as undeliverable, one whose handler returned (or trapped: it would fail the same way again) is
  removed, one the budget cut short (`CallOutcome::Exhausted`) stays at the front and is delivered at the
  start of the next tick, after the messages and before `Hourglass`.
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
