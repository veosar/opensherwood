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
<- {"jsonrpc":"2.0","id":1,"result":{"protocol":5,"build":"0.1.0","ruleset":5,"capabilities":["synthetic","capture","mission","replay"],"content_fingerprint":null}}
-> {"jsonrpc":"2.0","id":2,"method":"reset","params":{"scenario":{"synthetic":"corridor"},"seed":42}}
-> {"jsonrpc":"2.0","id":3,"method":"step","params":{"ticks":10,"events":[{"tick_offset":0,"sequence":0,"kind":"pointer_move","x256":25600,"y256":19200},{"tick_offset":0,"sequence":1,"kind":"pointer_down","button":"left"},{"tick_offset":0,"sequence":2,"kind":"pointer_up","button":"left"}]}}
<- {"jsonrpc":"2.0","id":3,"result":{"tick":10,"hashes":{"total":"...","actors":"..."}}}
```

Coordinates are logical pixels in 24.8 fixed point (`x256 = x * 256`). Keys are `"escape"`, `"enter"`,
`"space"`, `"up"` ... for the named keys, `{"letter": "c"}`, `{"digit": 1}`, `{"function": 11}` for the others
(`opensherwood_core::input::Key`).

## Orders and movement modes

The documented rules of `docs/original/ui-flow.md` 9.4, all through canonical input: a **left click** on a
character selects him, a left click on the ground orders the selected player character to **walk** there, a
second left click within 20 ticks and 8 map pixels of the first (`DOUBLE_CLICK_TICKS`, `DOUBLE_CLICK_DISTANCE`)
makes that order a **run**; a **right click** on the selected character cancels his order, a right click anywhere
else deselects. `c` crouches the selected player character, `s` stands him up. `observe` entities carry `gait`
(`walk` / `run`: the mode of the current order, `walk` again once it ends) and `posture` (`standing` /
`crouched`). Speeds: running is `RUN_SPEED_FACTOR` (2) times the walking speed, sneaking (any order while
crouched) the walking speed over `CROUCH_SPEED_DIVISOR` (2); both are hypotheses documented in
`crates/opensherwood-core/src/world.rs` (the animation table's per-frame `advance` is a distance per frame, not a
speed). Running plays the run block (action 7), crouching the crouched idle / sneak blocks (14 / 16) of profiles
that have them; the double-click memory (`last_ground_click`), gait and posture are in the snapshot and the
`world` / `actors` hashes. A **left click on an enemy soldier** while a player character is selected is an
**attack order** (hypothesis: the manual's fist icon is not drawn yet): the character walks into reach
(`attack_target` names the victim), then delivers the knock-out blow when he stands behind the victim, else
stops facing him; a ground order or a right click cancels it. See "Stealth layer" below.

## Stealth layer

`docs/original/stealth-and-combat.md` "Engine" (`crates/opensherwood-core/src/ai.rs`; every constant a
hypothesis pinned by tests). `observe` entities carry `team` (`player` / `enemy` / `civilian`), `ai_state`
(`patrol` = normal, `noticed` (action 141), `alarm` (142), `alerted` (searching the last seen position with
140 / 143 / 151), `returning` (walking back to the post), `punching` (123, player characters), `knocked_down`
(41 / 44), `lying` (47 / 48, knocked out), `getting_up` (49), `dead`), `state_ticks` (ticks left in a timed
state), `last_seen` and `alert_origin` (map points, 24.8), `attack_target` (entity id), `action` (the sprite
action id the entity reports: a change fires the script's `ActionChange(previous, new)`), `hit_points`
(profile `p0`; 100 without a profile value), `knockout_resistance` (profile `p4`), `npc_gait` (the gait of the
NPC's program walks, script native 140) and `fell_backward`. Enemy soldiers that are alive, active, unlocked
and on their feet perceive: a player character inside their view cone (half angle 45 degrees, range 200 px,
100 px when crouched; occluders ignored) or a running player character within 150 px is a stimulus. A
knocked-out soldier is out of action for `KNOCK_OUT_BASE_TICKS` (600) scaled by `(100 - p4) / 100`; `p4` >=
100 makes the blow fail. All of it is in the snapshot, validated and hashed (`actors`).

## Replays

Replay time is the **session tick** (ADR-0004, "Replay time is the session tick"): every tick of a `step` is
one unit, whether a screen (briefing page, pause menu) consumed the tick's events or the world stepped, so the
world tick lags behind the session tick by the number of screen frames. `replay.start {checkpoint_every}`
(at session tick 0: right after `reset`, the mission's first text page may be showing) records the initial
world hashes as the tick-0 checkpoint, then every canonical event of subsequent `step` calls (screen events
included: the Enter that dismisses a page, the Escape that pauses) plus a checkpoint every N session ticks;
a checkpoint is `{tick, world_tick, hashes, session, frame}`: the world hashes, a `session` digest (BLAKE3 over
the screen kind, the `ui` state as `observe` reports it and the notice text with its remaining ticks) and the
`frame` hash (`capture.hash` at that tick), so a replay reproduces what the player saw, not only the world.
A replay must have a checkpoint at tick 0 and a terminal one at its last tick (the parser refuses one with
either deleted, before any reset). `replay.stop {path?}` appends the final checkpoint and returns
the `ReplayV1` JSON Lines (and writes it under the artifact directory). `replay.play {jsonl | path,
stop_on_divergence}` resets to the replay's scenario and seed, checks that the header equals the one the
session would record now (protocol, ruleset, hash schema, content fingerprint, scenario, `time: "session"`,
viewport, tick rate, seed, RNG streams; a mismatch names the fields), compares the tick-0 checkpoint before
applying anything, then drives the same `advance` as recording did with the recorded events, comparing every
checkpoint (hash parts, `world_tick`, `session`, `frame`) and reporting the first diverging session tick and
what differs.
The header line: `{type: "header", replay_version: 1, protocol, ruleset, hash_schema, content_fingerprint,
scenario, time: "session", viewport: [w, h], tick_rate: [num, den], seed, rng_streams: {name: {algorithm,
seed, stream}}}`. `restore` is refused while a recording is active; Restart or Quit from the pause menu
installs another world and discards the recording. The Python client wraps the three methods
(`Engine.replay_start` / `replay_stop` / `replay_play`).

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
snapshot handles (ADR-0004, "Envelope and validation"). Both are refused while a menu screen is shown or while
a notice (native 202 text over the world) is still visible (a snapshot describes the world only; step until it
expires), and `restore` while a replay is being recorded.

## Scenarios

| `reset` scenario | Needs game data | What it is |
|---|---|---|
| `{"synthetic": "corridor"}` | no | 640x480 room, a player, a patrolling guard, three obstacles, a goal |
| `{"map_view": {"map": "sherwood", "ambiance": "Day"}}` | yes | the retail background of that map with the synthetic units on it and a scrollable camera |
| `{"mission": "<name>"}` | yes | the retail mission's background, walkable geometry, occluders and actors (NPC, civilian and `TOTO` sprites from the profile table `Configuration/profile.cpf`, see `docs/formats/profile.md`; a default sprite with a logged warning when an entry is unavailable; heroes still in file order; hidden player characters start inactive); NPCs follow their rail programs (walk the rail back and forth, face, wait, glance, loop; see `docs/formats/rhm.md` "Rail programs"), NPCs without a rail stand idle; the mission script (`Data/Levels/<name>.scb`) runs in the core VM (objectives, texts, sequences, messages, zones, activation, patrols; see below); enemy soldiers perceive the player characters and can be knocked out (see "Stealth layer"); viewport 1024x768 |
| `{"menu": "main"}` | yes | the original main menu; `observe` returns a `ui` object (`screen` = `main_menu`, `briefing`, `pause_menu`, `dialog`, `debriefing`, `credits`, `load` or `save` (the list rows appear as items with `action` = `row:<name>`); `items` with actions and rectangles; `hovered`; briefing `page`; `credits` reports the scroll offset in `page[0]`) while a screen is shown; clicking Play! loads the first mission behind its briefing; Escape in a mission opens the pause menu (Continue / Restart / Quit with confirmation); menus never tick the world; left clicks on HUD widgets act on the interface (kneel / standing figures crouch / stand the selection, other widgets consume the click) and never reach the map |

## Scripts

While a mission with a script is loaded, `observe` carries a `script` object: `objectives` (`[{index, primary,
done}]` in the order the script added them), `texts` (pending text indices of the level's text list, first is
shown), `mission_won`, `mission_lost` (`CheckVictoryCondition` returned 1 / 2; both sticky), `sequence_active`,
`camera_target` (map pixels set by the last camera native),
`debriefing`, `unknown_natives` (`{id: count}` of natives without an implementation that were called),
`faulted` (an unknown native stopped a callback), `lenient` and `unknown_calls` (see below), and
`actor_elements` (the script element handle of every entity by entity index, -1 for entities the script
cannot address: the handle native 3 returns, so a test can aim at the actor a script polls). The app dismisses
the text at the front of the queue through `World::vm_dismiss_text` when the briefing parchment closes (one
dismissal per page, on Enter, Escape or a click on the page). Tests dismiss pages the same way, with canonical
input: `Engine.skip_briefing()` sends Enter once per page (one session tick each, recorded by an active
replay). `debug.vm` is inspection only and cannot dismiss a page.

`debug.vm` (counters, objectives, pending texts, scrolls with positions and activity); in a mission `F1` writes `saves/quick.json` under the artifact directory and `F5` loads it (the snapshot envelope with the content identity; refused while a screen or notice is shown or a replay is recorded), and every 3600 world ticks a rolling auto save `saves/auto-<0..4>.json` is written returns `{present, classes, elements, locations, objectives, texts, mission_won,
mission_lost, money, sequence_active, sequences, faulted, lenient, unknown_calls, pending_messages, camera_target, debriefing,
mission_vars, counters, rng_draws}` (`money` is the script's integer of natives 236 / 237); `counters` holds `instructions`, `callbacks`, `budget_aborts`, `faults`,
`traps`, `messages_delivered`, `messages_dropped`, `unknown_natives`, `stub_natives`,
`objective_done_before_added` and `out_of_action_true` (native 90 calls that reported an actor knocked out or
dead). Its one mutation, `debug.vm {"win": true}`, marks the mission won: a documented
harness shortcut used only by the end-of-mission flow test (`test_mission_won_shows_the_debriefing_then_the_menu`),
because no mission can be won yet through play; it is not a player action and no other test may use it.

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
