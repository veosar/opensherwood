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

## Scenarios

| `reset` scenario | Needs game data | What it is |
|---|---|---|
| `{"synthetic": "corridor"}` | no | 640x480 room, a player, a patrolling guard, three obstacles, a goal |
| `{"map_view": {"map": "sherwood", "ambiance": "Day"}}` | yes | the retail background of that map with the synthetic units on it and a scrollable camera |
| `{"mission": "<name>"}` | yes | the retail mission's background, walkable geometry, occluders and actors (default soldier sprite for NPCs until the profile table is decoded); NPCs follow their rail programs (walk the rail back and forth, face, wait, glance, loop; see `docs/formats/rhm.md` "Rail programs"), NPCs without a rail stand idle; no scripts or reactive AI yet; viewport 1024x768 |
| `{"menu": "main"}` | yes | the original main menu; `observe` returns a `ui` object (`screen` = `main_menu`, `briefing`, `pause_menu` or `dialog`; `items` with actions and rectangles; `hovered`; briefing `page`) while a screen is shown; clicking Play! loads the first mission behind its briefing; Escape in a mission opens the pause menu (Continue / Restart / Quit with confirmation); menus never tick the world |

`harness/tools/drive.py` runs a short scripted session (select, order, scroll, capture) in headless or window
mode and prints where the PNGs went; agents use it to look at the engine after a change.

## Determinism fixture

`harness/fixtures/synthetic_corridor.json` (asset-free) records per-tick hashes and the framebuffer hash of a fixed
corridor script. CI checks it on Linux, Windows and macOS; regenerate it deliberately with
`python harness/tools/golden_digest.py --write harness/fixtures/synthetic_corridor.json` when the ruleset changes.

## Artifacts

Local runs write under `harness/out/` (git-ignored). CI uploads only synthetic artifacts. Anything derived from
game data stays on the machine that produced it.

## Golden images (planned)

Not implemented yet. The plan: `harness/goldens/` (git-ignored) holds reference PNGs generated from the player's
copy plus a manifest with the content fingerprint; comparisons are exact hashes for decoded assets and a masked
perceptual metric for composed scenes. Until then, data-backed tests check invariants and the maintainers inspect
captures by eye (`harness/captures/original/` holds the analyst's screenshots of the original, git-ignored).
