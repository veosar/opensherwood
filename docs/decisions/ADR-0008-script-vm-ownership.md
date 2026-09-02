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
  hash schema 8 (2026-09-02, Codex review 5).
- The `scripts` / `scheduler` hash parts stop being zero placeholders.
- **What is authoritative and what is not.** `VmState::counters` (instructions, callbacks, budget aborts,
  faults, traps, message and text drops, per-id native counts) and `VmState::budget` (the work left in the
  current tick) are diagnostics: they are neither serialised nor hashed (`#[serde(skip)]`), a restored world
  counts afresh and `debug.vm` reports the live values only. Everything the scripts can observe stays in the
  snapshot and the hash. Callbacks never yield, so a snapshot is *quiescent*: `validate` refuses frames,
  pushed arguments or a sequence still being collected instead of pretending to resume them.
- **One work budget per tick** (`vm::WORK_BUDGET_PER_TICK`, 2^22 units): instruction dispatch, every argument
  a call or native transfers, zone edge tests, scroll range checks, sequence elements, and the A* expansions
  and smoothing cells of the walks the script issues (`nav.rs` charges them to the same counter). An
  exhausted budget stops the tick deterministically (the running callback is aborted, later phases wait for
  the next tick, undelivered messages stay queued ahead of new ones) and is counted; it is part of the
  ruleset because it changes what a tick does. Player and rail-program orders use their own per-order budget
  (`world::ORDER_SEARCH_WORK`).
- **Trust boundary.** `Program::validate` in core is self-sufficient (functions in table order from address 0
  with their prologue, jumps inside their function, parameter reads and call arities against the table,
  arities within the stack limit, aggregate code / vertex bounds, element and location coordinates within
  `+-2^20`); the translator's checks are earlier diagnostics. `World::validate` also requires the gameplay and
  `script` RNG streams to derive from the world seed with their assigned ids (1 and 2).
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
