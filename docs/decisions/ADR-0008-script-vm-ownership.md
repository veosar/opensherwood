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
  queue, sequences, pending texts, camera target, patches, attributes, states, the `script` RNG stream) and
  entities gain `active` / `ai_locked` flags; ruleset 5, snapshot schema 7 and hash schema 6 (2026-09-02).
- The `scripts` / `scheduler` hash parts stop being zero placeholders.
- The synthetic corridor has no script: its hashes only change through the schema bump.
- The app dismisses the script's text pages through `World::vm_dismiss_text` (the briefing parchment, the
  `debug.vm` inspection method); no canonical input event exists for it yet, so a replay does not reproduce
  dismissals: an open question for the replay format.
- `opensherwood-script` keeps its place in the dependency graph (above core), which stays acyclic:
  core defines the IR, script produces it, the app hands it to core at mission load.
