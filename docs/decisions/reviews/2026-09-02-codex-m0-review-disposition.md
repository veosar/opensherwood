# Disposition of the Codex M0 code review (2026-09-02)

Codex reviewed commit `f79df35` adversarially (full text: `2026-09-02-codex-m0-review.md`) and returned 20
findings with the verdict "redesign". Every finding is answered below; the fixes landed in the commit that
adds this file unless stated otherwise.

| # | Finding | Disposition |
|---|---|---|
| 1 | Version identifiers not bumped | Fixed: `RULESET_VERSION` 2, `HASH_SCHEMA_VERSION` 2, `PROTOCOL_VERSION` 2, snapshot schema 3. A golden-hash unit test (`golden_hash_of_the_corridor_script`) and a cross-platform digest fixture (`harness/fixtures/synthetic_corridor.json`, checked in CI on all three OSes) fail when canonical bytes change without a deliberate update. |
| 2 | Hashing omits authoritative state | Fixed: goal, held buttons/keys, patrol vectors, entity count, animation state and explicit lengths/tags are encoded; `pathfinding` / `scripts` / `scheduler` / `campaign` hash to versioned placeholders. `every_authoritative_field_changes_some_hash` guards it. |
| 3 | Entity order erased by hashing; keys hashed in press order | Fixed: entities are hashed in slot order (which is the simulation and draw order); held buttons/keys are `BTreeSet`s. |
| 4 | Inline snapshots unvalidated, restore not transactional, background not restored | Fixed: `Snapshot` carries version/ruleset/hash schema; `World::validate` checks ids, ranges, patrols, selection, camera, RNG; restore validates before touching state; the session reloads background and catalog when the snapshot's scenario differs; snapshots are dropped on reset. Opaque server-side snapshots remain the recommended path; inline snapshots are for tests. |
| 5 | Fixed-point overflow | Fixed: all `Fixed` operations use `i64` intermediates and saturate; pointer coordinates are clamped; tick/camera/patrol arithmetic is saturating; `hostile_input_never_panics` and `extremes_never_panic_and_saturate` tests. |
| 6 | ReplayV1 incomplete / lax | Fixed: header carries `hash_schema` and named RNG streams; the parser requires header first, strictly increasing `(tick, sequence)` and checkpoint ticks, current versions, positive tick rate, a fingerprint for game-data scenarios. `reset` still takes only a seed (viewport and tick rate are constants for now; they will move into `reset` params when the mission loader lands). |
| 7 | RNG stream identity lossy | Fixed: `Rng` keeps seed and stream id, ids are bounded, draws saturate, the hash records algorithm/seed/stream/state/draws. |
| 8 | JSON-RPC conformance | Fixed: `jsonrpc` must be "2.0", ids validated, structurally invalid requests answered with `INVALID_REQUEST`, notifications produce no response. |
| 9 | Unbounded input / response amplification | Fixed: 16 MiB line cap, bounded stdin channel, limits on ticks (100k), hashed ticks (10k), events (100k), queued window input, 32 snapshot handles (FIFO), handles cleared on reset. |
| 10 | RPC-driven window mode nondeterministic | Fixed: with an RPC client attached the window never ticks on its own; window input is queued and applied by the next `step`. |
| 11 | Image allocation bombs | Fixed: dimensions capped at 8192, decoded size at 128 MiB, checked in `parse` (not only in detection). SRES aggregate budget: deferred (retail archives are 6.6 MB; noted for the asset cache design). |
| 12 | Renderer panics / long loops | Fixed: framebuffer size clamped, circles use `i64` and clipped ranges, lines are Liang-Barsky clipped before rasterising, blits verify source lengths. |
| 13 | Fingerprint ignores content | Fixed: v2 fingerprint digests every file (whole file up to 1 MiB, first and last 64 KiB beyond) in layer precedence order. |
| 14 | VFS ordering / validation | Fixed: directory entries sorted by raw name, case-insensitive collisions and non-UTF-8 names rejected, `robinhood.bks` verified through the case-insensitive index. Symlinks: followed as the OS presents them (documented). |
| 15 | Parsers accept invalid structures | Partially: image blobs validated; DIC table detection unchanged (the sprite decoder now validates every stream length against the frame size, which catches a wrong table immediately). SRES duplicate ids are tolerated on purpose (retail data may contain them). |
| 16 | Tests do not prove cross-platform determinism | Fixed: the committed digest fixture is checked on Linux, Windows and macOS in CI; malformed snapshot tests added. Fuzz targets and a 32-bit job: deferred to M1 (tracked in the roadmap). |
| 17 | Python client can hang | Fixed: stderr drained by a thread into a bounded buffer, per-call deadline, process always reaped. |
| 18 | Protocol types not enforced | Fixed: `observe` honours `entities`; button/key tags are explicit; `EntityKind` tag explicit. Physical-key set: deferred. |
| 19 | `harness/tools/re/` vs forbidden `re/` | Clarified: the forbidden path is the repository-root `/re/` (private analysis material). `harness/tools/re/` holds *observation* scripts that embed no game bytes; the directory will be renamed to `harness/tools/probe/` once the running investigations finish, and `check_no_assets.py` will reject a tracked root `re/`. |
| 20 | CI not reproducible | Fixed: toolchain pinned to 1.95.0 (matches `rust-toolchain.toml`), `--locked`, nextest pinned, Python requirements pinned, job timeouts. |

Verdict after the fixes: the M0 kernel is considered stable enough to build M1/M2 on; the remaining deferred items
(SRES budget, fuzz targets, 32-bit CI, physical-key set, `re` rename) are in the roadmap.
