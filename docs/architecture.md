# Architecture

OpenSherwood is a Cargo workspace. Dependencies point downwards only.

```
opensherwood-app  (binary `opensherwood`: headless RPC server, or the winit + wgpu window; modules engine.rs = session + RPC dispatch, rpc.rs = stdio transport, window.rs = presenter + input mapping)
   |
   +-- opensherwood-protocol   JSON-RPC types, canonical input events, ReplayV1, state hash schema, observation DTOs
   +-- opensherwood-render     deterministic CPU compositor -> framebuffer; PNG capture; Presenter trait (winit + wgpu impl lives in app)
   +-- opensherwood-core       authoritative simulation: fixed tick, RNG streams, entities (arenas + generational ids),
   |                       orders, pathfinding, AI, stimuli, campaign state, snapshot/restore, canonical hashing
   +-- opensherwood-script     SCB parser + VM; native function boundary to core (later: Lua for mods)
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

## Presentation

Reference mode: the CPU framebuffer at the game's logical resolution is uploaded as a texture and drawn
nearest-neighbour. Headless mode never creates a window; `capture` writes the CPU buffer.

## Asset resolution

`opensherwood-assets` owns the notion of "the game directory". Lookup order for a logical path such as
`Data/Text/Level.res`: mod overlays (in configured order) > language overlay (`<langid>/data/...`) > base `DATA/`.
Lookups are case-insensitive on every platform. The content fingerprint (hash of file list + sizes + a few file
hashes) is reported by `hello` and stored in replays so a replay is never compared across different data sets.

## Original-game oracle

Lives outside the shipped crates (`oracle/` holds only public schema and procedures). See `docs/oracle.md` and
ADR-0003 for the clean-room boundary.

## Crate rules

- `opensherwood-formats`: pure functions from bytes to typed structures; every unknown field is named `unknown_*`;
  errors carry offsets; no panics on malformed input (fuzzed).
- `opensherwood-core`: no I/O, no rendering, no platform types; testable with synthetic worlds.
- `opensherwood-protocol`: serde types only; versioned; changes require a doc update in `docs/harness.md`.
- `opensherwood-app`: the only crate that may contain platform / FFI / `unsafe` code.
