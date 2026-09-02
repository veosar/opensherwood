# Test harness

The harness is how agents (and CI) verify that the engine works without a human looking at a screen.

## Layers

1. **Rust unit and property tests** (`cargo nextest run --workspace`): parsers, core determinism, hashing.
2. **Synthetic end-to-end tests** (`pytest harness/tests/synthetic`): start `opensherwood --rpc stdio --headless`,
   load a synthetic scenario (no game data), drive it with canonical input events, check state hashes, snapshot /
   restore invariants and framebuffer hashes. These run in CI on every platform.
3. **Data-backed tests** (`pytest harness/tests/data`): need `OPENSHERWOOD_GAME_DIR`. Load real missions, compare decoded
   backgrounds pixel-exactly, compare composed scenes with local golden screenshots (never committed), run recorded
   replays and compare hashes. Skipped when the variable is unset. Run locally and on a trusted self-hosted runner.
4. **Oracle comparisons** (`harness/oracle`): compare our tick traces with traces recorded from the original game.
   Windows only, local only. See `docs/oracle.md`.

## Protocol

JSON-RPC 2.0, one JSON object per line, request on stdin, response on stdout, logs on stderr.
Methods and schemas: see ADR-0004 and the Rust types in `crates/opensherwood-protocol/src/`. The Python client is
`harness/opensherwood_harness/rpc.py`.

Example session:

```
-> {"jsonrpc":"2.0","id":1,"method":"hello","params":{"client":"pytest"}}
<- {"jsonrpc":"2.0","id":1,"result":{"protocol":3,"build":"0.1.0","ruleset":4,"capabilities":["synthetic","capture","mission","replay"],"content_fingerprint":null}}
-> {"jsonrpc":"2.0","id":2,"method":"reset","params":{"scenario":{"synthetic":"corridor"},"seed":42}}
-> {"jsonrpc":"2.0","id":3,"method":"step","params":{"ticks":10,"events":[{"tick_offset":0,"sequence":0,"kind":"pointer_move","x256":25600,"y256":19200},{"tick_offset":0,"sequence":1,"kind":"pointer_down","button":"right"},{"tick_offset":0,"sequence":2,"kind":"pointer_up","button":"right"}]}}
<- {"jsonrpc":"2.0","id":3,"result":{"tick":10,"hashes":{"total":"...","actors":"..."}}}
```

Coordinates are logical pixels in 24.8 fixed point (`x256 = x * 256`).

## Replays

`replay.start {checkpoint_every}` (right after `reset`, at tick 0) records every canonical event of subsequent
`step` calls plus a checkpoint of all hashes every N ticks; `replay.stop {path?}` returns the `ReplayV1` JSON Lines
(and writes it under the artifact directory); `replay.play {jsonl | path, stop_on_divergence}` resets to the
replay's scenario and seed, feeds the events tick by tick and compares every checkpoint, reporting the first
diverging tick and the subsystem hashes that differ. Replays recorded with another ruleset, protocol, hash schema
or game content are rejected.

Limits. A request line is at most 16 MiB; a longer line is answered with "request too large" and the rest of it is
skipped through the reader's buffer without being stored. A replay file is at most 64 MiB, 2^20 events, 2^16
checkpoints and 2^24 ticks (`opensherwood_protocol::replay_limits`); `replay.play` checks the file size before
reading it and refuses replays that run past 1,000,000 ticks (about 4.6 hours at 60 Hz) before resetting the
session. The recorder enforces the same quotas cumulatively: a `step` that would push the active recording over
the event, checkpoint or tick quota is refused before anything moves (`replay.stop` first), and a recording that
crosses a quota outside `step` (window mode) is discarded with an error at `replay.stop` rather than written as a
file the parser would reject.

## Snapshots

`snapshot` returns a handle (32 kept, oldest dropped) and the snapshot itself, an envelope
`{version, ruleset, hash_schema, content, world}`; `content` is the `hello.content_fingerprint` for retail
scenarios and `null` for synthetic ones. `restore {id | snapshot}` refuses a snapshot whose three versions or
`content` differ from the session's, or whose world fails validation (geometry vertices within `+-2^20` map
pixels, animation state resolvable in the attached sprite catalog, every other invariant of `World::validate`),
and a refused restore or a failed `reset` leaves the session exactly as it was: world, background, screen,
snapshot handles (ADR-0004, "Envelope and validation"). Both are refused while a menu screen is shown.

## Scenarios

| `reset` scenario | Needs game data | What it is |
|---|---|---|
| `{"synthetic": "corridor"}` | no | 640x480 room, a player, a patrolling guard, three obstacles, a goal |
| `{"map_view": {"map": "sherwood", "ambiance": "Day"}}` | yes | the retail background of that map with the synthetic units on it and a scrollable camera |
| `{"mission": "<name>"}` | yes | the retail mission's background, walkable geometry, occluders and actors (NPC, civilian and `TOTO` sprites from the profile table `Configuration/profile.cpf`, see `docs/formats/profile.md`; a default sprite with a logged warning when an entry is unavailable; heroes still in file order; hidden player characters start inactive); NPCs follow their rail programs (walk the rail back and forth, face, wait, glance, loop; see `docs/formats/rhm.md` "Rail programs"), NPCs without a rail stand idle; the mission script (`Data/Levels/<name>.scb`) runs in the core VM (objectives, texts, sequences, messages, zones, activation, patrols; see below); no reactive AI yet; viewport 1024x768 |
| `{"menu": "main"}` | yes | the original main menu; `observe` returns a `ui` object (`screen` = `main_menu`, `briefing`, `pause_menu` or `dialog`; `items` with actions and rectangles; `hovered`; briefing `page`; `credits` reports the scroll offset in `page[0]`) while a screen is shown; clicking Play! loads the first mission behind its briefing; Escape in a mission opens the pause menu (Continue / Restart / Quit with confirmation); menus never tick the world |

## Scripts

While a mission with a script is loaded, `observe` carries a `script` object: `objectives` (`[{index, primary,
done}]` in the order the script added them), `texts` (pending text indices of the level's text list, first is
shown), `mission_won`, `sequence_active`, `camera_target` (map pixels set by the last camera native),
`debriefing`, `unknown_natives` (`{id: count}` of natives without an implementation that were called),
`faulted` (an unknown native stopped a callback), `lenient` and `unknown_calls` (see below). The app dismisses
the text at the front of the queue through `World::vm_dismiss_text` when the briefing parchment closes (one
dismissal per page); the harness does the same through `debug.vm {"dismiss_text": true}`, which is an inspection
method, not a canonical input: a test that claims a player dismissed a page must do it through the screen.

`debug.vm` (counters, objectives, pending texts, scrolls with positions and activity) returns `{present, dismissed, classes, elements, locations, objectives, texts, mission_won,
sequence_active, sequences, faulted, lenient, unknown_calls, pending_messages, camera_target, debriefing,
mission_vars, counters, rng_draws}`; `counters` holds `instructions`, `callbacks`, `budget_aborts`, `faults`,
`traps`, `messages_delivered`, `messages_dropped`, `unknown_natives`, `stub_natives` and
`objective_done_before_added`.

Unknown natives (no row of `docs/formats/scb.md` with an effect) are a deterministic trap by default: the
callback stops there, `faulted` becomes true, and the id is counted. `opensherwood --lenient-natives` selects
the permissive policy for the session: such a native is a recorded no-op returning 0 and every call is logged
with its arguments in the VM state (`unknown_calls`, snapshotted and hashed). Stub natives (documented effect
not modelled yet: animations, remarks, doors, blips, ...) are recorded no-ops in both modes. The loader logs one
line per mission with the native call sites by class (implemented / stub / unknown ids).

`harness/tools/drive.py` runs a short scripted session (select, order, scroll, capture) in headless or window
mode and prints where the PNGs went; agents use it to look at the engine after a change.
`harness/tools/play_window.py --flow menu` plays the real window with OS-level mouse and keyboard input
(main menu, Play!, briefing, selection, walk order, pause menu, quit confirmation) and verifies every step
over the RPC; it is the end-to-end check of the window path (Windows only). It binds the window to the spawned
process id, re-checks geometry, overlap and foreground before every action, and exits 0 (PASS), 1 (FAIL) or 2
(DEGRADED: keys went through the RPC because Ctrl or Shift is physically held, so only the mouse path was
exercised).

## Determinism fixture

`harness/fixtures/synthetic_corridor.json` (asset-free) records per-tick hashes and the framebuffer hash of a fixed
corridor script. CI checks it on Linux, Windows and macOS; regenerate it deliberately with
`python harness/tools/golden_digest.py --write harness/fixtures/synthetic_corridor.json` when the ruleset changes.

## Artifacts

Local runs write under `harness/out/` (git-ignored). CI uploads only synthetic artifacts. Anything derived from
game data stays on the machine that produced it.

## Oracle comparison (local only)

`harness/captures/original/` (git-ignored) holds the analyst's screenshots of the original game, taken from
the player's own copy with `harness/tools/original/rhcap.py`. `opensherwood_harness.compare.compare(ours,
original, masks, diff_out)` computes the structural similarity (SSIM), the mean absolute difference and the
fraction of pixels differing by more than 32 over the frame with the given rectangles masked, and writes a diff
image. `harness/tests/data/test_oracle_menu.py` compares the engine's main menu with the original's capture
(profile text, button column and cursor masked): SSIM 0.9995 on 2026-09-02. Tests skip when the capture is
absent, so CI never needs game imagery. Scenes with a camera (missions) need an aligned capture first; the
briefing and pause screens are next.
