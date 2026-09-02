# Architecture

OpenSherwood is a Cargo workspace. Dependencies point downwards only.

```
opensherwood-app  (binary `opensherwood`: headless RPC server, or the winit + wgpu window; modules engine.rs = session + RPC dispatch, rpc.rs = stdio transport, window.rs = presenter + input mapping)
   |
   +-- opensherwood-protocol   JSON-RPC types, canonical input events, ReplayV1, state hash schema, observation DTOs
   +-- opensherwood-render     deterministic CPU compositor -> framebuffer; PNG capture; Presenter trait (winit + wgpu impl lives in app)
   +-- opensherwood-audio      music (Ogg Vorbis) and effect (PCM WAVE) playback; presentation only, never authoritative
   +-- opensherwood-core       authoritative simulation: fixed tick, RNG streams, entities (arenas + generational ids),
   |                       orders, pathfinding, AI, stimuli, campaign state, snapshot/restore, canonical hashing;
   |                       modules vm.rs (script IR, program, run-time state, interpreter, scheduler) and natives.rs
   |                       (the engine functions scripts call by number)
   +-- opensherwood-script     SCB -> core IR translator (opcode table, calling convention, index spaces; no state, ADR-0008; later: Lua for mods)
   +-- opensherwood-assets     game directory discovery (GOG/Steam/explicit), VFS with base + language + mod overlays,
   |                       case-insensitive lookup, content fingerprint, decoded-asset caches
   +-- opensherwood-formats    bounded readers for every file format (no I/O policy, no game logic)
opensherwood-tools (binary: inspect / extract / disassemble; editor later)
harness/       Python: RPC client, pytest suites (synthetic in CI, data-backed locally), image and state diff
```

## Determinism contract

- The simulation advances in fixed ticks. Tick rate is a rational stored in every replay.
- All randomness comes from named, seeded RNG streams owned by core. `std::collections::HashMap` iteration never
  influences simulation; use `BTreeMap` / sorted vectors / arena order.
- Positions and timers are fixed-point unless the oracle proves the original used floats in a way we must mirror;
  in that case the float operations are isolated and documented.
- Rendering reads simulation state; it never writes to it.
- `snapshot()` captures every authoritative field; caches are rebuilt on `restore()`.
- `hash()` produces subsystem hashes and a total (see ADR-0004).

## Script VM (`opensherwood-core::vm`, ADR-0008)

A mission's compiled script is translated by `opensherwood-script` into a `Program` (classes with variables,
functions and one IR instruction per bytecode quad, the flat element table, the locations) that the app hands
to `World::new_mission` in the `MissionSpec`. The core owns everything that runs: class variable blocks, call
frames, the two argument stacks, mission variables, objectives, the message queue, sequences, pending texts, the
camera target, patches / attributes / states, the `script` RNG stream and diagnostic counters. All of it is in
`World::vm`, in the snapshot, validated on restore and hashed (`scripts`: program digest and script-visible
state; `scheduler`: queues, sequences, texts, frames).

Load: `Initialize` on every class (level first, then the element classes in table order), `PostInitialize` on
the level, then the first sequence elements. Each tick, before the entities move: deliver the messages queued
during the previous tick (to the class bound to the target element, else the level class), `Hourglass(tick)` on
every class that defines it, `EnterZone` / `ExitZone` for player characters crossing a zone class's polygon,
the running sequence (one at a time, FIFO; elements block on a text until the app dismisses it with
`World::vm_dismiss_text`, on a wait for its tick count, everything else completes at once), then
`CheckVictoryCondition` (1 = `mission_won`). A callback runs to completion within a per-tick instruction budget;
natives never call back into the script (they queue messages), so the frame stack is empty between ticks.
Unknown natives trap (see the ADR); the `IsTaken`, `ActivatedBy*`, `ReachPoint` and `ActionChange` callbacks
are exposed as `World::vm_*` hooks that nothing triggers yet.

## Presentation

Reference mode: the CPU framebuffer at the game's logical resolution is uploaded as a texture and drawn
nearest-neighbour. Headless mode never creates a window; `capture` writes the CPU buffer.

## Asset resolution

`opensherwood-assets` owns the notion of "the game directory". Lookup order for a logical path such as
`Data/Text/Level.res`: mod overlays (in configured order) > language overlay (`<langid>/data/...`) > base `DATA/`.
Lookups are case-insensitive on every platform. The content fingerprint (BLAKE3 over the layer list, every logical
path and size, and the full-content digest of every file under every layer's `Data`; a file that cannot be read
makes the fingerprint an error, never a partial hash) is reported by `hello` and stored in snapshots and replays
so they are never compared across different data sets. Every call walks the directories again and streams every
file: nothing is cached, because size and timestamps cannot prove bytes unchanged (a same-size edit with a
preserved modification time), and the change time is not available through the standard library on Windows.
Each file is stat'ed before and after it is read and hashed again when the two disagree, so a concurrent
replacement is never hashed as a mix of two versions. Cost on the retail installation (about 1 GiB): under a
second warm, a few seconds cold. The lookup index is built by `GameDir::open`; a file added afterwards changes
the fingerprint but resolves only after the directory is opened again.

## Original-game oracle

Lives outside the shipped crates (`oracle/` holds only public schema and procedures). See `docs/oracle.md` and
ADR-0003 for the clean-room boundary.

## Crate rules

- `opensherwood-formats`: pure functions from bytes to typed structures; every unknown field is named `unknown_*`;
  errors carry offsets; no panics on malformed input (fuzzed).
- `opensherwood-core`: no I/O, no rendering, no platform types; testable with synthetic worlds.
- `opensherwood-protocol`: serde types only; versioned; changes require a doc update in `docs/harness.md`.
- `opensherwood-app`: the only crate that may contain platform / FFI / `unsafe` code.
