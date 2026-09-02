# ADR-0004: Harness protocol, replay and state hashing are designed at M0

Date: 2026-09-02. Status: accepted.

Versions in force (2026-09-02, script VM): protocol 3 (`ui` observation, `menu` scenario, optional world
fields; the `script` observation object and `debug.vm` are additive), ruleset 5 (script VM: `Initialize` /
`PostInitialize` at load, `Hourglass` and `CheckVictoryCondition` every tick, sequences, messages, zone events;
hidden player characters start inactive), hash schema 6 (`scripts` and `scheduler` parts carry the VM state,
entity `active` / `ai_locked` flags under `actors`, the `script` RNG stream under `rng`), snapshot schema 7
(`vm` state, entity flags).
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
| `step` | atomically enqueue canonical input events and advance exactly N ticks; while a screen (menu, briefing, pause) is shown the events drive the screen and the world does not tick, so `tick_offset` counts frames of the screen, not world ticks |
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

## Screens and the world

Menus, briefings and the pause menu are session state of the app, not of the world: they never enter world hashes,
and `snapshot`, `restore`, `replay.start` and `replay.play` are refused while a screen is shown (error
`screen shown`), so a snapshot always describes a directly played world. The harness dismisses screens with the
same events a player would use before taking snapshots.

## Snapshot / restore invariant

From M0: replay prefix, snapshot at tick T, run suffix and record hashes, restore, run the identical suffix,
compare every per-tick subsystem hash and selected framebuffer hashes. Fuzz snapshot points, repeated cycles,
corrupt input and unknown versions. Internal snapshots are independent from original-save compatibility.

### Envelope and validation

A snapshot is `{version, ruleset, hash_schema, content, world}`. `content` is the game directory fingerprint
(`hello.content_fingerprint`) for retail scenarios and `null` for synthetic ones: the sprite catalog and the
background are not part of the snapshot, so a restore must run on the content the snapshot was taken from.
`restore` checks, in this order and before anything of the session changes: all three versions equal this
build's; `content` equals the fingerprint the session would rebuild the scenario from (a mismatch, a missing
fingerprint for a retail scenario or a fingerprint on a synthetic one are all refused); every world invariant
(`World::validate`), including geometry vertices within `+-2^20` map pixels and, when a sprite catalog is
attached, every animation state naming an existing profile with animation and frame indices in range and
`elapsed` below the frame duration. Nothing falls back silently. The world is built in a temporary when the
scenario changes; a failed `restore` (or `reset`) leaves the previous world, background, screen and snapshot
handles untouched. Geometry arithmetic (point-in-polygon, scan conversion, line tests) is `i128`/`i64` so any
`i32` input gives the same answer in debug and release builds.
