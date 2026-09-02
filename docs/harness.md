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
<- {"jsonrpc":"2.0","id":1,"result":{"protocol":1,"build":"0.1.0","ruleset":1,"capabilities":["synthetic","capture"],"content_fingerprint":null}}
-> {"jsonrpc":"2.0","id":2,"method":"reset","params":{"scenario":{"synthetic":"corridor"},"seed":42}}
-> {"jsonrpc":"2.0","id":3,"method":"step","params":{"ticks":10,"events":[{"tick_offset":0,"sequence":0,"kind":"pointer_move","x256":25600,"y256":19200},{"tick_offset":0,"sequence":1,"kind":"pointer_down","button":"right"},{"tick_offset":0,"sequence":2,"kind":"pointer_up","button":"right"}]}}
<- {"jsonrpc":"2.0","id":3,"result":{"tick":10,"hashes":{"total":"...","actors":"..."}}}
```

Coordinates are logical pixels in 24.8 fixed point (`x256 = x * 256`).

## Scenarios

| `reset` scenario | Needs game data | What it is |
|---|---|---|
| `{"synthetic": "corridor"}` | no | 640x480 room, a player, a patrolling guard, three obstacles, a goal |
| `{"map_view": {"map": "sherwood", "ambiance": "Day"}}` | yes | the retail background of that map with the synthetic units on it and a scrollable camera |
| `{"mission": "<name>"}` | yes | not implemented yet (milestone M2/M4) |

`harness/tools/drive.py` runs a short scripted session (select, order, scroll, capture) in headless or window
mode and prints where the PNGs went; agents use it to look at the engine after a change.

## Artifacts

Local runs write under `harness/out/` (git-ignored). CI uploads only synthetic artifacts. Anything derived from
game data stays on the machine that produced it.

## Golden images

`harness/goldens/` (git-ignored) holds reference PNGs generated from the player's copy plus a manifest with the
content fingerprint. `harness/tools/make_goldens.py` regenerates them. Comparisons: exact hash for decoded assets,
masked perceptual metric (SSIM over the play area, UI masked) for composed scenes, thresholds in the test file.
