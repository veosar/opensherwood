# ADR-0004: Harness protocol, replay and state hashing are designed at M0

Date: 2026-09-02. Status: accepted.

Versions in force (2026-09-02 evening): protocol 2, ruleset 2, hash schema 3 (walkable geometry is hashed under
`pathfinding`), snapshot schema 3.
Any change to canonical bytes bumps the ruleset or hash schema and regenerates
`harness/fixtures/synthetic_corridor.json` (see `docs/decisions/reviews/2026-09-02-codex-m0-review-disposition.md`).

## Transport

JSON-RPC 2.0, newline-delimited, over the engine process's stdin / stdout when started with `--rpc stdio`.
Logs go to stderr only. No fixed TCP port in tests (an optional loopback socket may serve interactive tooling).

## Canonical methods

| Method | Purpose |
|---|---|
| `hello` | protocol version, capabilities, build and ruleset version, content fingerprint |
| `reset` | load a synthetic scenario or a mission with seed and configuration |
| `step` | atomically enqueue canonical input events and advance exactly N ticks |
| `observe` | filtered structured state, UI state, objectives, subsystem hashes |
| `snapshot` / `restore` | authoritative internal checkpoint |
| `capture` | framebuffer hash; optionally write a PNG under the artifact directory |
| `shutdown` | clean termination |

Limits: 16 MiB per request line, 100k ticks or events per `step`, 10k ticks with per-tick hashes, 32 snapshot
handles. Notifications (requests without `id`) get no response. In window mode with `--rpc stdio` the simulation
advances only through `step` (controlled mode); window input is queued into the next step.

Player actions are expressed only as canonical input events (`pointer_move`, `pointer_down`, `pointer_up`,
`wheel`, `key_down`, `key_up`) with fixed-point logical coordinates and an explicit `(tick_offset, sequence)`.
Debug methods (`debug.select`, `debug.order`, `debug.console`, ...) exist for planning and inspection, but any
test that claims "the player can do X" must do X through input events.

## ReplayV1

Protocol and ruleset version, content fingerprint, scenario id, logical viewport, tick rate as a rational,
initial seed and named RNG stream states, ordered events keyed by `(tick, sequence)`, optional intent annotations,
optional checkpoint expectations. JSON Lines. Never contains OS events, timestamps, paths, pointers or physical
display coordinates.

## Canonical state hash

A manual canonical byte encoding (domain prefix, schema version, little-endian fixed-width fields, entities by
stable id, deterministic ordering, no caches / handles / clocks / audio state / diagnostics, fixed-point or
explicitly normalised numbers, RNG algorithm + state + stream + draw count, script VM state, scheduler queues,
pending stimuli) hashed with BLAKE3 per subsystem (`world`, `actors`, `orders`, `pathfinding`, `scripts`,
`scheduler`, `rng`, `campaign`) and in total, so the first divergence is diagnosable.

## Snapshot / restore invariant

From M0: replay prefix, snapshot at tick T, run suffix and record hashes, restore, run the identical suffix,
compare every per-tick subsystem hash and selected framebuffer hashes. Fuzz snapshot points, repeated cycles,
corrupt input and unknown versions. Internal snapshots are independent from original-save compatibility.
