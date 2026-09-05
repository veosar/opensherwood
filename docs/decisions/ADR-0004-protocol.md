# ADR-0004: Harness protocol, replay and state hashing are designed at M0

Date: 2026-09-02. Status: accepted.

Versions in force (2026-09-05, Codex review 7): protocol 6 (`reset.starting_money` and `ReplayHeader.starting_money`: the mission's starting money is a canonical input recorded in the header, playback resets with it; `UiItem.selected`; `observe.persistence_error`; replay time is the session tick: header `time:
"session"`, checkpoints carry `world_tick`, a `session` digest and the `frame` hash, the tick-0 and terminal
checkpoints are required and compared; `ui` observation,
`menu` scenario, optional world fields; the `script` observation object and `debug.vm` are additive), ruleset 12
(2026-09-05, Codex review 8: the taint is dependency-closed (every low-confidence opcode, policy native, effect
stub, lenient unknown call and engine hypothesis records its `Assumption` when taken), queued `ActionChange`
handlers run transactionally (rolled back when the budget cuts them short, a full queue is a fault), one
simulation budget with a cursor per phase pays for perception, transitions, attack orders and program walks,
the native call and its result read are one instruction; the canonical input gains `Key::Semicolon`, tag
13 after Backspace); ruleset 11
(2026-09-05, the oracle measurements of `docs/original/stealth-and-combat.md` 8: every entity moves at the speed of
the cycle it plays, read from the profile's table on the measured animation clock (a frame lasts its tick half plus
one table ticks of 3 clocks at 64 Hz; hero walk 85.3 px/s, run 106.7, sneak 18.0; the fallback ratios 5 / 4 and
27 / 128 for units without a cycle), timed states last their animation on that clock, a running character is heard
from 350 px and the soldier charges at once without the noticed / alarm pause, the view range is 250 px, and an alert
reached through the measured noise channel records no `perception` assumption); ruleset 10 (Codex
review 7: native call sites must match the signature table (a wrong argument count refuses the program at load and
traps at dispatch; a required argument never defaults), action changes are queued and delivered to `ActionChange`
exactly once (an exhausted tick delivers them next tick), the stealth layer charges every entity its perception
inspects and every path search it issues to one per-tick budget walked from a round-robin cursor, `Dead` and
`alive` must agree and the five status natives read one state function, the starting money is seeded before
`Initialize` and never overwritten, and every outcome that depends on a stub value or a hypothesis records an
assumption (`observe.script.tainted`); ruleset 9 (stealth layer:
soldiers perceive the player characters through a view cone and a noise radius and cycle through the alert
states, a left click on an enemy is an attack order and the knock-out blow from behind puts the victim out of
action for a timer scaled by his resistance, natives 85 / 87 / 90 / 128 / 240 read these states and 140 sets
the gait of program walks, `ActionChange` fires on every action-id change; ruleset 8: movement modes: a
left click on the ground walks, a double click runs, `c` / `s` crouch / stand, a right click cancels / deselects;
ruleset 7: script VM: `Initialize` /
`PostInitialize` at load, `Hourglass` and `CheckVictoryCondition` every tick, sequences, messages, zone events;
hidden player characters start inactive; native 32 is a barrier over walk / animation completion tokens; one
work budget per tick, granted only at the start of the tick (the load-time run has its own; event hooks and
text dismissals draw from what the tick left) and charging instructions, argument transfers, every entity a
zone / scroll scan or native 204 looks at, every polygon edge tested (zones, natives 97 / 204), sequence
elements and every stage of the path searches the script issues (initialisation, expansions, unwinding,
smoothing, conversion); programs must have balanced parameter / argument stacks; AI locking halts an NPC's
walk, native 160 and camera centring are computed in `i64`), hash schema 14 (the four simulation cursors under
`world`, the VM's `fault` and the new assumption tags under `scripts`, the fused native's result slot in the
program digest; schema 13: the entity `heard` flag under `actors`;
the animation `elapsed` bytes now count clock units; schema 12: the VM's `assumptions` under `scripts`,
its `pending_action_changes` under `scheduler`, the `ai_cursor` under `world`; schema 11: entity `team`, `ai_state`, `state_ticks`,
`action`, `hit_points`, `knockout_resistance`, `npc_gait`, `fell_backward`, `last_seen`, `alert_origin` and
`attack_target` under `actors`; schema 10: entity `gait` / `posture` tags under `actors`,
the `last_ground_click` under `world`; schema 9: `scripts` and `scheduler` parts carry the VM state including sequence tokens and the
barrier wait; frames and stacks are no longer encoded because a snapshot must be quiescent; entity `active` /
`ai_locked` flags under `actors`, the `script` RNG stream under `rng`; schema 9 adds the player's `money`
(natives 236 / 237) and `mission_lost` (`CheckVictoryCondition` = 2) to `scripts`), snapshot schema 15 (the
world's `cursors` replace `ai_cursor`, the VM's `fault` replaces `faulted`, `Instr::Native` carries `dst`,
frames hold no native result; schema 14: entity
`heard`, animation `elapsed` in clock units; schema 13: the VM's
`assumptions` and `pending_action_changes`, the world's `ai_cursor`, mission specs' `starting_money` and
`assumptions`; schema 12: the stealth
layer's entity fields and the actor specs' `hit_points` / `knockout_resistance`; schema 11: entity `gait` /
`posture`, the world's `last_ground_click`; schema 10: `vm`
state without its diagnostic `counters` and per-tick `budget`, sequence `tokens`, entity flags, `money` and
`mission_lost`).
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

Protocol and ruleset version, content fingerprint, scenario id, time model, logical viewport, tick rate as a
rational, initial seed and named RNG stream states, ordered events keyed by `(tick, sequence)`, optional intent
annotations, checkpoint expectations. JSON Lines. Never contains OS events, timestamps, paths, pointers or
physical display coordinates.

### Replay time is the session tick

Replay time (`time: "session"`, the only model) counts the session's `advance` calls since the world was
installed by `reset`: every tick of a `step` is one unit, whether a screen consumed the tick's events or the
world stepped. Events carry the session tick they are applied at; a checkpoint at session tick `t` holds the
world's hashes and its own tick (`world_tick`, which lags while a screen is shown) after `t` advances, plus a
`session` digest of the presentation state (screen kind, `ui` state, notice text and remaining ticks) and the
`frame` hash of the rendered framebuffer, so playback proves what the player saw, not only the world. Screens
are therefore part of the timeline: dismissing the mission's first text pages, opening the pause menu with
Escape and continuing are ordinary recorded key and pointer events, and playback runs them through the same
`advance` the recording did. The checkpoint at tick 0 is the state right after `reset`, before anything is
applied; recording never drops it, the parser requires it together with a terminal checkpoint at the replay's
last tick (a replay with either deleted is refused before any reset), and playback compares it first, so a
replay that reproduces nothing cannot report no divergence.

`replay.start` is allowed only at session tick 0 (right after `reset`, even while the first page is shown; the
main menu has no world). `replay.play` resets to the replay's scenario and seed, then requires the header to
equal, field by field, the header the session would record now (protocol, ruleset, hash schema, content,
scenario, time model, viewport, tick rate, seed, RNG stream identities); a replay recorded under other
parameters is refused with the differing fields named, never played against a session that would produce
different checkpoints. `restore` is refused while a recording is active (a restore is not an input event, so
the recording could not reproduce it), and `snapshot` / `restore` are refused while a notice (native 202) is
visible: a snapshot describes the world only and a notice is session presentation it does not carry. Every
reset path (`reset`, Play!, Restart, the next mission) installs a world with no notice of the previous one.
Restart and Quit from the pause menu install another world and discard the recording.

## Canonical state hash

A manual canonical byte encoding (domain prefix, schema version, little-endian fixed-width fields, entities by
stable id, deterministic ordering, no caches / handles / clocks / audio state / diagnostics, fixed-point or
explicitly normalised numbers, RNG algorithm + state + stream + draw count, script VM state, scheduler queues,
pending stimuli) hashed with BLAKE3 per subsystem (`world`, `actors`, `orders`, `pathfinding`, `scripts`,
`scheduler`, `rng`, `campaign`) and in total, so the first divergence is diagnosable.

## Screens and the world

Menus, briefings and the pause menu are session state of the app, not of the world: they never enter world hashes,
and `snapshot` and `restore` are refused while a screen is shown (error `screen shown`), so a snapshot always
describes a directly played world. Replays are different: a screen's frames are session ticks and the events
that drive it are recorded and replayed (see "Replay time is the session tick"), so `replay.start` works while
the first briefing page is shown and `replay.play` works from any screen (it resets first). The harness
dismisses screens with the same events a player would use (`Engine.skip_briefing`: Enter per page) before
taking snapshots; `debug.*` methods never dismiss or drive a screen.

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
