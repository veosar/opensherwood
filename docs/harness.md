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
<- {"jsonrpc":"2.0","id":1,"result":{"protocol":6,"build":"0.1.0","ruleset":14,"capabilities":["synthetic","capture","snapshot","map_view","mission","replay","menu","script"],"content_fingerprint":null}}
-> {"jsonrpc":"2.0","id":2,"method":"reset","params":{"scenario":{"synthetic":"corridor"},"seed":42}}
-> {"jsonrpc":"2.0","id":3,"method":"step","params":{"ticks":10,"events":[{"tick_offset":0,"sequence":0,"kind":"pointer_move","x256":25600,"y256":19200},{"tick_offset":0,"sequence":1,"kind":"pointer_down","button":"left"},{"tick_offset":0,"sequence":2,"kind":"pointer_up","button":"left"}]}}
<- {"jsonrpc":"2.0","id":3,"result":{"tick":10,"hashes":{"total":"...","actors":"..."}}}
```

Coordinates are logical pixels in 24.8 fixed point (`x256 = x * 256`). Keys are `"escape"`, `"enter"`,
`"space"`, `"up"` ... `"backspace"`, `"semicolon"` (the original's mini-map shortcut) for the named keys,
`{"letter": "c"}`, `{"digit": 1}`, `{"function": 11}` for the others (`opensherwood_core::input::Key`).

## Orders and movement modes

The documented rules of `docs/original/ui-flow.md` 9.4, all through canonical input: a **left click** on a
character selects him, a left click on the ground orders the selected player character to **walk** there, a
second left click within 20 ticks and 8 map pixels of the first (`DOUBLE_CLICK_TICKS`, `DOUBLE_CLICK_DISTANCE`)
makes that order a **run**; a **right click** on the selected character cancels his order, a right click anywhere
else deselects. A left click within 12 map px of an active **pick-up item** (`docs/formats/rhm.md` "`ZORG`": the
placeholder disc the engine draws for it, gold for a purse, brown for arrows, grey for an unknown kind) orders the
selected player character to walk to it (`observe` entities carry `pickup` = the item's element handle while the
order stands); within 24 map px of it (the scroll pickup radius) the item is taken: arrows add their stack to the
entity's `arrows`, a purse adds 25 per stack unit to the script's money and one to the entity's `purses`, an
unknown kind only disappears; the item deactivates and native 235 reads it as taken. The gesture, the radius and
the amounts are hypotheses (`item_pickup` in `observe.script.assumptions`). The HUD's portrait draws the selected
character's `arrows` and `purses` under the bow and purse icon positions (`ui-flow.md` 9.3 element 4). `c` crouches the selected player character, `s` stands him up. `observe` entities carry `gait`
(`walk` / `run`: the mode of the current order, `walk` again once it ends) and `posture` (`standing` /
`crouched`). Speeds: every entity moves at the speed of the animation cycle it plays, read from its profile's
table on the measured animation clock (`docs/original/stealth-and-combat.md` 8, `docs/formats/sprite-animations.md`
"Reading rules"): the hero walks at 85.3 px/s (1.42 px per tick), runs at 106.7 and sneaks at 18.0; soldiers walk at
42.7, run at 64 and use 64 / 85.3 for their alert walk / run. Units without a cycle (synthetic worlds) use
`Entity::speed` times the fallback ratios of `crates/opensherwood-core/src/world.rs` (run 5 / 4, sneak 27 / 128).
Running plays the run block (action 7), crouching the crouched idle / sneak blocks (14 / 16) of profiles
that have them; a frame lasts its tick half plus one table ticks of 46.875 ms (`observe` reports the animation's
`elapsed` in clock units, 16 per world tick and 45 per table tick); the double-click memory (`last_ground_click`),
gait and posture are in the snapshot and the `world` / `actors` hashes. A **left click on an enemy soldier** while a player character is selected is an
**attack order** (measured, `docs/original/combat-measurements.md` 1.1: no icon, no key): the character walks
into reach (`attack_target` names the victim), then delivers the knock-out blow when he arrives unseen behind
the victim, else stops 52 px short facing him and the fight begins; a ground order or a right click cancels
it. See "Stealth layer" and "Melee" below.

## Stealth layer

`docs/original/stealth-and-combat.md` "Engine" (`crates/opensherwood-core/src/ai.rs`; the noise channel and the
timings are measured, the view cone and the timers hypotheses, every value pinned by tests). `observe` entities
carry `team` (`player` / `enemy` / `civilian`), `ai_state`
(`patrol` = normal, `noticed` (action 141), `alarm` (142), `alerted` (searching the last seen position with
140 / 143 / 151), `returning` (walking back to the post), `punching` (123, player characters), `knocked_down`
(41 / 44), `lying` (47 / 48, knocked out), `getting_up` (49), `dead`), `state_ticks` (ticks left in a timed
state), `last_seen` and `alert_origin` (map points, 24.8), `attack_target` (entity id), `action` (the sprite
action id the entity reports: a change fires the script's `ActionChange(previous, new)`), `hp` / `hp_max`
(hit points left / full: the hero's measured 100, a soldier's profile `pre[0]`, 100 without a profile value),
`energy` / `energy_ticks` (0..20 and the ticks to the next regained unit), `foe`, `pose`, `pose_ticks`,
`swing_ticks`, `figure` and `in_combat` (see "Melee"), `knockout_resistance` (profile `p4`), `npc_gait` (the
gait of the NPC's program walks, script native 140), `fell_backward` and `heard` (the current alert came
from a run heard, the measured channel, rather than from the view cone). The `ai_state` values `fighting`
(in a melee), `dying` (killed, the fall playing) and `dead` (lying for good) belong to the melee. Enemy soldiers that are alive, active, unlocked
and on their feet perceive: a player character inside their view cone (half angle 45 degrees, range 250 px,
125 px when crouched; occluders ignored) starts the noticed -> alarm -> alerted sequence; a running player
character within 350 px is heard whatever the soldier faces and he charges at once (`alerted`, the alert run).
A knocked-out soldier is out of action for `KNOCK_OUT_BASE_TICKS` (600) scaled by `(100 - p4) / 100`; `p4` >=
100 makes the blow fail. All of it is in the snapshot, validated and hashed (`actors`); `validate` holds the
layer's invariants (`dead` only with `alive` false, a timer exactly in the timed states, attack orders from a
player character to an enemy soldier, alert states on enemy soldiers only). The whole simulation besides the
script shares one per-tick work budget (`world::SIM_WORK_PER_TICK`, 2^24) handed out phase by phase on
deterministic quotas (ADR-0008; Codex review 9): a pre-index pass (one unit per entity), then perception
(one per soldier inspected and per soldier / player character pair tested; the remainder of the budget,
about 2^22 + 2^21), the state transitions (one per human; 2^21), the attack orders (one per attacker, the
victim found in the index; 2^21), the guards' waypoint programs (one per idle guard; 2^21), the movement
(one per mover, per obstacle-index cell looked at, per obstacle candidate and per polygon edge of the
walkable geometry tested; 2^20), the animation advance and the action-change scan (one per entity; 2^20
each). A phase gets its quota plus what the phases before it left, so a hostile snapshot cannot starve a
phase; every path search a phase issues draws from its grant, capped per search at `world::SIM_SEARCH_WORK`
(2^20): a search that fails with the full cap is unreachable under this budget (the order dropped, the
instruction skipped), one cut short with less is retried first next tick. Each phase walks its entities
from its own cursor (the snapshot's `cursors: {perception, states, attacks, programs, movement, animation,
actions}`, in the `world` hash); when its grant runs out the cursor marks where the next tick resumes (past
an entity that alone exhausted a whole quota). Obstacle entities are queried through a 64 px grid index
derived from the snapshot (`validate` refuses more than 2^22 cell entries).

## Melee

`docs/original/combat-measurements.md` and the "Engine" section of `docs/original/stealth-and-combat.md`
(measured 2026-09-05 unless marked). The **left button acts on its release**: a press and a release within 32
map px is the click of "Orders and movement modes", a longer stroke a drawn figure; the world's `press`
remembers the press (snapshotted, `world` hash). A left click on an enemy soldier is the **attack order**: the
character walks up and, unless he arrives unseen from behind (the knock-out above), stops 52 px from the
victim's feet and the **fight** begins: both are `ai_state` = `fighting` with `foe` naming the other and
`in_combat` true in `observe`; the soldier turns to the attacker and fights where he stands. The soldier
swings every ~5.3 s (318 ticks with the gameplay RNG's jitter, `swing_ticks`), two swings in three land for
5 hp (`hp` falls, never regenerates) and cost him one unit of `energy` (0..20, regained after ~4 s); the
hero's automatic strikes never land against a soldier (hypothesis: the pole arm's reach or a block, recorded
as the `{"attack_policy": "block"}` assumption). The **forward stroke** (`pointer_down` on the ground, `pointer_move` at
least 32 px to the right within 45 degrees, `pointer_up`: the manual's figure, drawn 80 px right and 20 px up
in the measurements) locks onto the nearest enemy soldier (`figure` = `forward_stroke` until delivered) and,
in the fight, plays the **powerful blow** (`pose` = `powerful_blow`, action 75, `pose_ticks` = 57 to its
resolution): two units of energy (regained one per 0.9 s), 50 hp when it lands (one in three: the
`{"attack_policy": "hit_chance"}` assumption, recorded on every roll and on the soldier's jittered
swing timing too). `pose` is otherwise `idle` (the stance, action 54), `strike` (59) or
`flinch` (104, when hit). A ground order or a right click leaves the fight; the soldier stands his ground and
walks back to his post (the `{"attack_policy": "post_bound"}` assumption for every kind but the halberdier). At 0 hp the entity is
dead: `alive` false and `ai_state` = `dying` (the fall, 44 from the front / 41 from behind) then `dead`
(48 / 47 for good, the body stays drawn); natives 85 / 87 / 90 report it from the tick of the blow. A player
character's death sets **`observe.hero_dead`** (sticky, hashed) and the app shows the lost page on that
tick whether or not the script's `CheckVictoryCondition` reports the loss. `capture` draws, for every
fighter and for the actor under the pointer, a 20 x 3 px red health row 8 px below the feet (10 px left of
them; (255,0,0) hovered, (123,0,0) otherwise; 1 px per 5 hp of the hero) and a blue energy row 4 px lower
((0,200,255) / (0,101,123); 1 px per unit), the spent part black, and cream damage numbers rising 50 px in
1.5 s over the victim's head (`observe` does not list them: presentation only, snapshotted but not hashed).

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

`reset` takes `scenario`, `seed` and, for missions, an optional `starting_money` (the selected profile's money by default; replays record the value used in their header and play back with it). `observe` carries `persistence_error` while the last profile / settings write failed.

| `reset` scenario | Needs game data | What it is |
|---|---|---|
| `{"synthetic": "corridor"}` | no | 640x480 room, a player, a patrolling guard, three obstacles, a goal |
| `{"map_view": {"map": "sherwood", "ambiance": "Day"}}` | yes | the retail background of that map with the synthetic units on it and a scrollable camera |
| `{"mission": "<name>"}` | yes | the retail mission's background, walkable geometry, occluders and actors (NPC, civilian and `TOTO` sprites from the profile table `Configuration/profile.cpf`, see `docs/formats/profile.md`; a default sprite with a logged warning when an entry is unavailable; heroes still in file order; hidden player characters start inactive); NPCs follow their rail programs (walk the rail back and forth, face, wait, glance, loop; see `docs/formats/rhm.md` "Rail programs"), NPCs without a rail stand idle; the mission script (`Data/Levels/<name>.scb`) runs in the core VM (objectives, texts, sequences, messages, zones, activation, patrols; see below); enemy soldiers perceive the player characters and can be knocked out (see "Stealth layer"); viewport 1024x768 |
| `{"menu": "main"}` | yes | the original main menu; `observe` returns a `ui` object (`screen` = `main_menu`, `briefing`, `pause_menu`, `dialog`, `debriefing`, `credits`, `load`, `save`, `options`, `options_graphics`, `options_sounds`, `options_shortcuts`, `select_player`, `new_player`, `rename_player`, `lost` (the lost page's `restart`, `load` and `ok` seals) or `minimap` (the overlay, toggled by the map scroll; the world runs on) (list rows appear as items with `action` = `row:<index>`, all enabled, the current one with `selected` = true; a rename edits the selected row inline, Enter commits and Escape cancels; option bars as `bar:<n>` with `enabled` = selected, sliders as `slider:<n>` with the value in the label, the position left of the first cell selecting 0; of the options only the effects and music volumes act, the others (aspect, effect toggles, sound mode / quality, dialogue and comment volumes, comment frequency, shortcut set) are stored in `settings.json` until their subsystems exist); `items` with actions and rectangles; `hovered`; briefing `page`; `credits` reports the scroll offset in `page[0]`) while a screen is shown; clicking Play! loads the first mission behind its briefing; Escape in a mission opens the pause menu (Continue / Restart / Quit with confirmation); menus never tick the world; left clicks on HUD widgets act on the interface (kneel / standing figures crouch / stand the selection, other widgets consume the click) and never reach the map |

## Scripts

While a mission with a script is loaded, `observe` carries a `script` object: `objectives` (`[{index, primary,
done}]` in the order the script added them), `texts` (pending text indices of the level's text list, first is
shown), `mission_won`, `mission_lost` (`CheckVictoryCondition` returned 1 / 2; both sticky), `sequence_active`,
`camera_target` (map pixels set by the last camera native),
`debriefing`, `unknown_natives` (`{id: count}` of natives without an implementation that were called),
`faulted` (an unknown native stopped a callback), `lenient` and `unknown_calls` (see below),
`actor_elements` (the script element handle of every entity by entity index, -1 for entities the script
cannot address: the handle native 3 returns, so a test can aim at the actor a script polls), `items` (the
pick-up items of the element table: `[{element, kind, stack, x, y, active, taken}]` with `kind` = `"arrows"`,
`"purse"` or `{"unknown_a": n}`; `active` = shown and pickable, `taken` = picked up by a player character,
what native 235 reads), and the taint of
ADR-0008 ("Hypotheses and taint"): `tainted` (the script executed over a hypothesis source, so `mission_won`
/ `mission_lost` are not authoritative) with `assumptions`, the recorded entries in canonical order
(snapshotted and hashed). The set is dependency-closed by construction (Codex review 8): every source is a
variant of the registry `vm::Assumption`, recorded where the hypothesis is taken whether or not the script
reads a value there, so `tainted: false` means no known hypothesis was taken. The entries:
`{"stub_result": id}` (a stub with an unmodelled effect was called, or a stub's fabricated result consumed;
the presentation-only stubs 62 / 69 / 149 / 150 / 243 record nothing on the call), `{"policy": id}` (an
implemented native whose reading is a policy, `natives::NATIVE_TAINT`), `{"opcode": op}` (an instruction of
a low-confidence opcode executed: 0x14, 0x24, 0x28, 0x2b), `"unresolved_jump"`, `{"unknown_native": id}`
(lenient mode), and the engine's own rules, each recorded where it first changes authoritative state
whether or not a script handler exists (Codex review 9): `"sight_cone"` (the view cone decided a sighting
that changed a soldier's state), `"noise_radius"` (a run heard from beyond the measured 330 px bound and
within the engine's 350 px; within the bound the noise channel is measured and records nothing),
`"alert_policy"` (the noticed -> alarm -> search sequence, the alert timeout, the re-plan, the return to
the post), `{"attack_policy": "reach" | "block" | "hit_chance" | "post_bound"}` (the attack order
resolved from behind; the hero's strikes never landing; a chance rolled or a soldier's swing timed with
the engine's jitter; a soldier standing his ground), `"knock_out"` (the blow felled or failed to fell a
victim, native 90 / 128 reported it, or its action id reached a handler), `"profile_stats"`,
`"tick_rate"`, `"scroll_pickup"`, `"item_pickup"` (a pick-up item was taken: the click gesture, the radius,
the kind / stack reading and the purse amount), `"zone_at_load"`, `"walk_completion"`, `"action_change_order"`,
`"campaign_graph"`, `"lenient_assets"`. Every fight is therefore tainted from its first swing and every
sighting from the tick it changed a state; a win reached over measured rules only (a run heard within the
bound, the immediate charge) keeps `tainted: false`
(`a_charge_from_the_unmeasured_noise_band_taints_a_win_read_from_native_97`).
Under this model every retail mission is tainted from its load-time callbacks on (the first mission's
`Initialize` calls effect stubs and policy natives: `test_first_mission_briefing_sequence_then_camera_on_the_hero`
pins the exact set at load and after 600 ticks). The app dismisses
the text at the front of the queue through `World::vm_dismiss_text` when the briefing parchment closes (one
dismissal per page, on Enter, Escape or a click on the page). Tests dismiss pages the same way, with canonical
input: `Engine.skip_briefing()` sends Enter once per page (one session tick each, recorded by an active
replay). `debug.vm` is inspection only and cannot dismiss a page.

`debug.vm` (counters, objectives, pending texts, scrolls with positions and activity, `items` as in `observe`); in a mission `F1` writes `saves/quick.json` under the artifact directory and `F5` loads it (the snapshot envelope with the content identity; refused while a screen or notice is shown or a replay is recorded), and every 3600 world ticks a rolling auto save `saves/auto-<0..4>.json` is written returns `{present, classes, elements, locations, objectives, texts, mission_won,
mission_lost, tainted, assumptions, money, sequence_active, sequences, faulted, fault, lenient, unknown_calls, pending_messages, camera_target, debriefing,
mission_vars, counters, rng_draws, element}` (`element` answers `{"element": i}`: the entry `i` of the script's element
table as `{kind: map | unmodelled | actor | object | scroll | item | polygon, ...}` (an item carries `item_kind` and `stack`), `null` beyond the table; `money` is the
script's integer of natives 236 / 237; `fault` is the
sticky reason behind `faulted`: `{"unknown_native": id}`, `{"arity_mismatch": id}` or
`"action_queue_overflow"`, `null` while the script runs as written); `counters` holds `instructions`, `callbacks`, `budget_aborts`, `faults`,
`traps`, `messages_delivered`, `messages_dropped`, `unknown_natives`, `stub_natives`,
`objective_done_before_added`, `out_of_action_true` (native 90 calls that reported an actor knocked out or
dead), `arity_mismatches` (`{id: count}` of native calls trapped because their argument count differed from the
signature table) and `transactions_rolled_back` (queued `ActionChange` handlers the budget cut short, rolled
back and retried whole next tick; a full queue is the `action_queue_overflow` fault, never a drop). Its only mutations, `debug.vm {"win": true}` and `{"lose": true}`, mark the mission won or lost: a documented
harness shortcut used only by the end-of-mission flow tests (`test_mission_won_shows_the_debriefing_then_the_menu`, `test_mission_lost_shows_the_lost_debriefing_then_the_menu`),
because the flow tests must not depend on the long walk of `test_win.py` (the first mission is won through play there, tainted: `docs/original/h01-win-path.md`); it is not a player action and no other test may use it.

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
