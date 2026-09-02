# Full independent repository review — 2026-09-02

Reviewed baseline: commit `277af51877e2c26de207c7f0761ffed62ecce26c`, plus the pre-existing uncommitted working-tree changes.

Verdict: **block further merging and feature work until the P0/P1 items below are resolved.** The committed project has a promising deterministic kernel and unusually good early format research, but it currently violates its own no-assets policy, overstates several verification claims, exposes denial-of-service paths on hostile data, and lacks the oracle infrastructure needed to support “faithful” claims. The uncommitted menu integration is additionally not buildable as written.

No files were modified during this review.

## Review baseline and verification results

The working tree was already dirty:

```text
 M crates/opensherwood-app/src/main.rs
 M crates/opensherwood-app/src/ui.rs
 M crates/opensherwood-core/src/world.rs
 M crates/opensherwood-protocol/src/lib.rs
?? crates/opensherwood-app/src/ui_assets.rs
```

These changes belong to the maintainer and are treated separately from committed-history conclusions.

| Check | Result | Interpretation |
|---|---|---|
| `cargo fmt --all -- --check` | **Failed** | Formatting diffs in the uncommitted menu changes. |
| `cargo test --workspace --locked` | **Blocked by environment** | Cargo could not create `target/debug/.cargo-lock`: access denied in the enforced read-only workspace. This is not a test failure, but it means the current source was not compiled. |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | **Blocked by environment** | Same Cargo lock-file restriction. |
| `pytest harness/tests/synthetic` | **14 passed, 4 failed** | All four failures were artifact-write attempts denied by the read-only workspace: one PNG and three replay files. |
| `pytest harness/tests/data` with the supplied game directory | **6 passed, 1 failed** | The only failure was the denied map-view PNG write. All 39 missions loaded successfully. |
| Test collection | **25 tests** | 18 synthetic, 7 data-backed. |
| `golden_digest.py --check` | **Passed** | The committed synthetic digest matches. |
| `scripts/check_no_assets.py` | **Passed, but unsound** | It misses the critical textual asset leak and several classes of image artifact. |
| `scripts/sync_skills.py --check` | **Passed** | Six mirrored files are synchronized. |
| `cargo tree --workspace --locked` | **Passed** | No Git dependencies; dependency direction is acceptable. |
| Windows-target Cargo licence inventory | **No incompatible dependency identified** | See legal/licence audit below. |

Important qualification: the executable used by Python was built at `15:14:30 UTC`; the dirty source files were modified at `15:20:15 UTC`. The Python results therefore exercise an earlier binary, not the current menu worktree.

---

# 1. Complete findings

Severity meanings:

- **Critical**: legal/repository-integrity failure requiring immediate containment.
- **High**: merge blocker, hostile-input crash/OOM, determinism-contract break, or major fidelity blocker.
- **Medium**: material correctness, portability, maintainability, or verification weakness.
- **Low**: localized quality, documentation, or tooling defect.

## Critical

### C-01 — Committed game text violates the repository’s hard legal policy

The policy says the repository must contain no original text at all: [docs/legal.md:8](</C:/Users/przem/source/repos/opensherwood/docs/legal.md:8>) and [docs/legal.md:20](</C:/Users/przem/source/repos/opensherwood/docs/legal.md:20>).

Nevertheless, [docs/original/campaign-flow.md:42](</C:/Users/przem/source/repos/opensherwood/docs/original/campaign-flow.md:42>) contains three long briefing pages explicitly identified as “verbatim in the game,” continuing through line 57. Lines 36–41 and 74–77 contain further copied objectives and debriefing prose.

[docs/original/ui-flow.md:56](</C:/Users/przem/source/repos/opensherwood/docs/original/ui-flow.md:56>) through line 66 reproduces complete menu labels, lines 74–87 reproduce dialog wording and controls, and lines 91–120 reproduce profile and options text. Short functional labels may have a different copyright analysis from narrative prose, but the project’s own policy is stricter and forbids them.

The uncommitted [ui.rs:54](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/ui.rs:54>) also hardcodes observed retail labels. It must not be committed in that form; runtime text should be loaded from the player’s resource files.

Recommended containment:

1. Stop publishing or merging the affected material.
2. Replace narrative text with resource IDs, hashes, lengths, high-level paraphrases, and local extraction procedures.
3. Load UI and briefing strings from `Level.res`/`DEFAULT.RES` at runtime.
4. Ask counsel whether the already-pushed commit requires a history rewrite. The repository’s own incident procedure says to take a real asset leak offline rather than blindly delete history.
5. Treat the current repository as unable to assert “contains no game assets” until this is resolved.

## High

### H-01 — The uncommitted menu integration is statically unbuildable and changes versioned behaviour without version bumps

This applies to the current dirty tree, not committed `HEAD`.

- `Scenario::Menu` was added at [world.rs:90](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:90>), but the app’s exhaustive match at [engine.rs:421](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:421>) has no `Menu` arm.
- `ObserveResult.ui` is required at [protocol/lib.rs:196](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-protocol/src/lib.rs:196>), but construction at [engine.rs:597](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:597>) omits it.
- The default CLI scenario is now `menu` at [main.rs:34](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/main.rs:34>), but [engine.rs:305](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:305>) parses it as `Scenario::Synthetic("menu")`, which the core rejects.
- Map/mission viewport semantics changed from 640×480 to 1024×768, but `RULESET_VERSION`, `SNAPSHOT_VERSION`, and `PROTOCOL_VERSION` remain unchanged at `2`, `3`, and `2`. Old retail replays would be accepted and then diverge instead of being rejected.
- `cargo fmt --check` currently fails.

Finish or shelve the menu work before any commit. Add explicit menu/session ownership, protocol observations, reset/restore behaviour, version bumps, and end-to-end menu tests.

### H-02 — The no-assets control cannot prevent another leak

[scripts/check_no_assets.py:15](</C:/Users/przem/source/repos/opensherwood/scripts/check_no_assets.py:15>) omits PNG, JPEG, WebP, TIFF, and other common derived-image formats. It only flags binaries larger than 512 KiB at lines 23 and 52, so a small game-derived screenshot passes.

It also:

- scans only the current `git ls-files` set, not introduced blobs or history;
- cannot detect copied game prose;
- does not implement the promised explicit rejection of a tracked root `/re/`;
- reads tracked files more than once;
- treats success as “no game assets detected,” which is disproved by C-01.

There is also a path mismatch:

- [docs/harness.md:68](</C:/Users/przem/source/repos/opensherwood/docs/harness.md:68>) says `harness/goldens/` is ignored;
- [.gitignore:35](</C:/Users/przem/source/repos/opensherwood/.gitignore:35>) ignores only repository-root `/goldens/`.

Consequently, the documented golden directory could be committed normally.

The policy check should reject all derived media by path/type, reject `harness/goldens` and root `re`, scan newly introduced Git objects, and require human review for copied text. Its success message should be scoped to what it actually checks.

### H-03 — Public sprite decoding and export tools permit multi-gigabyte allocations

[sprite_decode.rs:130](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/sprite_decode.rs:130>) accepts a public `FrameRecord` with unrestricted `u16` dimensions. It allocates:

- `width * height` `u16` pixels at lines 151 or 182 — up to about 8.6 GiB;
- an additional `pixels.len() * 4` bytes at [sprite_decode.rs:216](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/sprite_decode.rs:216>) — up to about 17.2 GiB.

`SpriteBank` limits records to 4096 pixels per dimension, but the public format API does not. The tool bypasses `SpriteBank` and allocates `rec.length` directly at [opensherwood-tools/main.rs:285](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-tools/src/main.rs:285>), allowing nearly 4 GiB before decode validation.

Rust allocation failure normally aborts rather than returning `FormatError`, contradicting [formats/lib.rs:7](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/lib.rs:7>).

Use one shared validated `DecodeLimits` policy, checked multiplication, `try_reserve`, and a decoded-byte budget in every public route.

### H-04 — SRES can amplify a small archive into hundreds of GiB

[sres.rs:120](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/sres.rs:120>) eagerly decompresses every picture. Individual image blobs allow 128 MiB, while `PICC` and cursor counts allow 4096 images at lines 135–145 and 172–182. There is no archive-wide decoded-byte or image-count budget.

A hostile archive can therefore request over 512 GiB of decoded storage even though each individual image passes its cap. Top-level entry count is also not bounded.

Make SRES lazy or enforce cumulative compressed, decoded, entry, and nesting budgets.

### H-05 — Hostile inline snapshots can overflow geometry arithmetic and panic or diverge by build mode

`World::validate` limits only the number of geometry vertices at [world.rs:558](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:558>); it does not bound coordinates.

`point_in_polygon` multiplies differences of arbitrary `i32` coordinates in `i64` at [geom.rs:48](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/geom.rs:48>). The maximum product exceeds `i64`. Navigation scan conversion performs an even larger multiplication by 256 at [nav.rs:310](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/nav.rs:310>).

Restore validates and then immediately rebuilds navigation at [world.rs:830](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:830>), so a JSON snapshot can reach the overflow. Debug builds panic; release builds wrap, creating a determinism violation.

Use `i128` or strict map-relative coordinate limits, and add extreme-coordinate snapshot tests.

### H-06 — Replay/RPC limits are per parse or per request, not end-to-end resource limits

Several independent paths allow unbounded memory or time:

- Oversized RPC input is initially capped, but the remainder is drained into an unbounded `Vec` at [rpc.rs:97](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/rpc.rs:97>).
- `replay.play` reads a whole arbitrary file before applying the 64 MiB parser cap at [engine.rs:749](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:749>).
- Recording appends events and checkpoints without cumulative limits at [engine.rs:267](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:267>). Repeated valid `step` calls bypass the per-request event cap.
- `replay.stop` then constructs the full JSONL string and returns it in a JSON response, causing further large copies.
- `MAX_TICK = 2^24` at [protocol/lib.rs:12](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-protocol/src/lib.rs:12>) permits roughly 6.5 days of 30 Hz simulation inside one RPC.
- Checkpoint interval 1 can make the recorder produce a replay its own parser later rejects.

Use bounded discard buffers, pre-read file-size checks or streaming parsing, cumulative recorder quotas, bounded serialization, and a practical synchronous playback budget.

### H-07 — The content fingerprint does not meet the determinism guarantee stated in the architecture

[assets/lib.rs:242](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-assets/src/lib.rs:242>) claims that any edited file changes the fingerprint. Files over 1 MiB hash only the first and last 64 KiB. A same-size middle edit is invisible.

Read/open/seek failures are silently represented by an empty or partial per-file hash at lines 259–279 instead of failing the fingerprint operation.

This contradicts [docs/architecture.md:39](</C:/Users/przem/source/repos/opensherwood/docs/architecture.md:39>), which says a replay is never compared across different datasets.

Use a full streaming digest with a metadata cache and return an error if any indexed file cannot be hashed.

### H-08 — The renderer cannot reproduce correct depth ordering

Entities are drawn in slot order at [render/lib.rs:435](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-render/src/lib.rs:435>), without map-y, layer, sector, or original depth ordering.

Occluders are applied after every sprite has been drawn. Restoring background pixels over a “behind” sprite can erase pixels from a different foreground sprite whose rectangle overlaps it. The method works only when relevant sprite rectangles do not overlap.

The complete function is also called twice at lines 494–499.

Before claiming map fidelity, define and test a stable depth key using map layer/sector/y information and composite actors and occluders in depth order. Add overlapping-actor fixtures.

### H-09 — The project advertises an oracle and visual comparison system that does not exist

[README.md:45](</C:/Users/przem/source/repos/opensherwood/README.md:45>) says the harness compares against the original, and lines 55–56 say nothing is merged without screenshot comparison. [docs/harness.md:14](</C:/Users/przem/source/repos/opensherwood/docs/harness.md:14>) describes `harness/oracle`; lines 68–70 describe a golden generator.

None of these exists:

- no `harness/oracle`;
- no `harness/tools/make_goldens.py`;
- no `harness/goldens`;
- no `oracle/schema/trace-v1.md`, despite [docs/oracle.md:18](</C:/Users/przem/source/repos/opensherwood/docs/oracle.md:18>);
- no exact, masked, perceptual, or oracle image comparison in the tests.

Current data tests check parse/load invariants and determinism against OpenSherwood itself. They do not establish original-game fidelity.

This is the largest technical-program risk after the legal issue. Complete the first controlled oracle trace and a legally local-only comparison pipeline before implementing more inferred gameplay.

### H-10 — Authoritative script-state ownership is unresolved

The architecture says the core owns all authoritative state, including AI, campaign, snapshot, and hashing at [docs/architecture.md:11](</C:/Users/przem/source/repos/opensherwood/docs/architecture.md:11>), while `opensherwood-script` is a higher crate with a VM/native boundary at line 13.

`World::snapshot()` captures only `World`; script, scheduler, and campaign currently hash as fixed zero placeholders at [world.rs:983](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:983>). Once a higher-level VM owns mutable state, the current core snapshot cannot capture it without either reversing dependency direction or introducing a new authoritative composite layer.

Resolve this with an ADR before VM work. Plausible designs include:

- VM state represented by pure data types owned by core, with execution logic in script;
- a new simulation/session crate above core and script that owns the authoritative snapshot;
- a trait-driven VM state representation with canonical serialization defined below both.

## Medium and low findings by component

### `opensherwood-formats`

1. **Medium — Parser versions are often recorded but not enforced.** SRES reads the version at `sres.rs:124`; bitmap/TrueType fonts at `font.rs:146` and `234`; SCB at `scb.rs:139`; RHM root and child versions at `rhm.rs:1235–1274`. In these cases corpus tests assert the retail version, but the parser accepts incompatible versions.

2. **Medium — SRES accepts a missing trailer.** At `sres.rs:228–233`, exact EOF immediately after entries is accepted even though the documented retail format has an offset trailer.

3. **Medium — RHM validation is much weaker than its status claim.** [rhm.rs:1235](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/rhm.rs:1235>) requires only `FOOT`; other missing chunks become empty defaults. Duplicate chunks overwrite earlier values, order is not enforced, child versions are not validated, and unknown chunk bodies are discarded. The exact ten-chunk invariant is enforced only by the retail-data test at `tests/gamedata.rs:375–423`.

4. **Medium — `POUF` is not decoded.** [rhm.rs:728](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/rhm.rs:728>) scans for the next pair of printable length-prefixed strings and stores the intervening bytes raw. That is a useful corpus framing heuristic, not a decoded record format.

5. **Medium — RHP accepts ambiguous containers.** RHP correctly validates its root and known child versions at [rhp.rs:733](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/rhp.rs:733>), but `Container::child` returns only the first matching tag at `chunk.rs:43–46`; duplicate and extra chunks are ignored.

6. **Medium — DIC table discovery is heuristic.** [dic.rs:53](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/dic.rs:53>) finds a suffix whose offsets chain. The parser does not prove that page parsing ends exactly at the inferred table start or that the frame chain begins at BKS offset zero. `SpriteBank` validates bounds but not monotonicity, gaps, overlaps, or final exact end.

7. **Medium — SCB parser accepts structurally inconsistent programs.** [scb.rs:136](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/scb.rs:136>) does not enforce version 1.5. Function addresses need not be sorted or in range; variable offsets, prologue operands, opcodes, native IDs, calls, or jump targets are not validated. Those invariants exist only in `tests/gamedata.rs:507–545`.

8. **Medium — Animation displacement can overflow.** [anim_table.rs:311](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/anim_table.rs:311>) casts `u32` origins/offsets to `i32` and subtracts. The app repeats the issue for origins at `engine.rs:55–63`.

9. **Medium — Walk timing deliberately discards documented distance timing.** At `engine.rs:61`, a zero low-half duration is changed to one tick and the high half is ignored. Retail walk frames are documented as `ticks == 0 && advance > 0`. Animation speed and foot phase are therefore known placeholders, not verified behaviour.

10. **Medium — “Every frame” is not an automated regression.** The Rust data test decodes every 97th frame plus only the first 500 page-less frames at [tests/gamedata.rs:241](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/tests/gamedata.rs:241>). The all-404,855 claim rests on analyst scripts/scratch runs, not a maintained test.

11. **Medium — Data-backed Rust tests report success when absent.** The `need_data!` macro returns normally at `tests/gamedata.rs:40–47`, so Cargo reports a passed test, not a skipped test, when the environment is missing.

12. **Medium — No fuzz targets exist.** This contradicts [docs/architecture.md:51](</C:/Users/przem/source/repos/opensherwood/docs/architecture.md:51>), which says malformed input is fuzzed.

13. **Low — The DIC module header is stale.** `dic.rs:2` still says BKS pixel decoding is unknown.

### `opensherwood-assets`

1. **Medium — Language selection is not modeled.** [assets/lib.rs:172](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-assets/src/lib.rs:172>) loads every numeric language directory, sorts names lexicographically, and gives the lowest name first priority. An installation containing several languages can resolve unintended text.

2. **Medium — Mod priority reverses call order.** `push_mod` inserts at index zero at lines 213–220. Calling it in configured order makes the last call highest priority, while the docs do not define that reversal.

3. **Medium — Unicode normalization is not implemented.** Paths are lowercased only at lines 110 and 131–136, even though the collision error claims Unicode-form ambiguity is handled.

4. **Medium — Symlink traversal has no cycle/root containment check.** Recursive `walk` follows directory metadata at lines 90–127. A hostile mod tree can recurse through a cycle or index outside its declared root.

5. **Medium — Root discovery is nondeterministic in ambiguous installations.** `find_child_ci` uses unsorted `read_dir(...).flatten().find_map(...)` at lines 286–290.

6. **Medium — Explicit invalid `--game-dir` silently falls back to synthetic mode.** [main.rs:53](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/main.rs:53>) treats all discovery errors the same. If the user supplied a path explicitly, the app should fail.

7. **Low/Performance — Sprite RGBA is copied for every render lookup.** [engine.rs:33](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:33>) clones the cached RGBA vector into a new `Arc<SpriteFrame>`, defeating much of the asset cache.

### `opensherwood-core`

1. **Medium — Snapshot `hash_schema` is not validated.** It is emitted at `world.rs:823` but restore validates only snapshot and ruleset versions at lines 830–843.

2. **Medium — Snapshot animation state is not catalog-aware.** Validation accepts large bounded animation/frame values without checking that the attached profile, animation, and frame exist. After restore, rendering and animation can silently fall back or stop.

3. **Medium — Inline snapshots have no content fingerprint.** A retail snapshot can be restored against different game/mod content, even though catalog and background are not part of the snapshot.

4. **Medium — `set_geometry` exposes unchecked geometry.** [world.rs:332](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:332>) accepts arbitrary geometry and immediately builds navigation without validation.

5. **Medium — `NavGrid::line_clear` can overflow its public inputs.** `b - a` and `abs()` at `nav.rs:244–250` are unsafe for extreme `i32` coordinates.

6. **Medium/Fidelity — Civilians become guards.** [world.rs:301](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-core/src/world.rs:301>) maps both enemies and civilians to `EntityKind::Guard`. The entity model cannot yet represent teams, neutrality, civilians, VIPs, or mission objects.

7. **Positive finding.** Fixed-point arithmetic itself uses saturating operations; authoritative set iteration uses deterministic structures; hash encoding is explicit; and no unsafe code was found. The synthetic M0 kernel is the strongest part of the repository.

### `opensherwood-script`

The crate is only a six-line SCB header re-export at [script/lib.rs:1](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-script/src/lib.rs:1>). This is honest, but the phrase “dependency direction is established” is premature given H-10. There is no VM, ABI, scheduler, error model, snapshot state, resource budget, or native-call interface to review yet.

### `opensherwood-render`

1. **Medium — Glyph x adjustment is applied twice.** `BitmapFont::advance` already includes `x_adjust` at [font.rs:109](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-formats/src/font.rs:109>). `FontAtlas::measure` adds it again at `text.rs:49`, and `draw` moves the pen by it before calling `advance` at lines 70–77.

2. **Medium — Malformed public `Background` values can panic.** `apply_occluders` indexes `bg.rgba[si..si+4]` at [render/lib.rs:540](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-render/src/lib.rs:540>) without the length check used by `blit_region`.

3. **Medium — Public render structures allow overflow-prone dimensions and lines.** `Occluder::depth_y` performs `i32` subtraction before widening at lines 75–85. Internal RHP values are small, but the public API does not enforce that.

4. **Low — Occlusion is performed twice.** `render/lib.rs:494–499`.

5. **Medium/Fidelity — RGBA alpha and shadow behaviour are preview choices.** `0x001F` becomes half-transparent black, but the spec explicitly says this is not verified. A reference RGB565/integer blending path will likely be needed.

### `opensherwood-audio`

1. **Medium — Looping buffers an entire decoded track.** [audio/lib.rs:91](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-audio/src/lib.rs:91>) uses Rodio’s `repeat_infinite`, whose implementation wraps the decoder in `Buffered`; the first playback caches every decoded sample. Use Rodio’s seek/restart-capable looped decoder or a custom streaming loop.

2. **Medium/Fidelity — Missions play the menu track.** [engine.rs:203](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:203>) handles only `MapView`; every other scenario uses `Menu.wav`.

3. **Low — Failed replacement retains old music.** The existing player is stopped only after the new decoder succeeds at `audio/lib.rs:82–90`.

4. Audio remains presentation-only and does not contaminate simulation hashes, which is correct.

### `opensherwood-protocol` and app session

1. **Medium — Replay header fields are ceremonial.** Validation checks only that viewport and tick rate are nonzero and RNG streams nonempty at [protocol/lib.rs:334](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-protocol/src/lib.rs:334>). Playback ignores their actual values and resets using only scenario and seed at `engine.rs:767–775`.

2. **Medium — Tick-zero checkpoints are never compared.** Playback compares checkpoints only after stepping at `engine.rs:782–803`.

3. **Medium — Session restore is not transactional.** [engine.rs:622](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:622>) validates only `snap.world`, may reset and mutate the session, then calls `World::restore`, which can reject the envelope version/ruleset. The error leaves a changed session.

4. **Medium — Duplicate timed-event sequence values are accepted.** `engine.rs:565–578` sorts but does not reject duplicate `(tick_offset, sequence)`, leaving their order dependent on JSON input order rather than represented semantics.

5. **Medium/Security — Artifact containment is lexical only.** Checks at `engine.rs:478–491` and `659–675` reject `..` and absolute paths but do not prevent a symlink/junction beneath the artifact directory from escaping it. The substring check also rejects benign names such as `frame..png`.

6. **Medium — RHP errors are logged and ignored.** [engine.rs:342](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/engine.rs:342>) returns a background with empty geometry and occluders if the map’s RHP is absent or corrupt. A required gameplay asset should make reset fail.

7. **Medium/Fidelity — Mission ambiance is hardcoded.** `engine.rs:441–445` always chooses `Day` and ignores `FOOT.variant`.

8. **Medium/Fidelity — Mission identity is not cross-checked.** The mission’s `FOOT.map_id` is not verified against RHP `SPOK`.

9. **Medium/Fidelity — Mission import intentionally drops authoritative data.** [mission.rs:60](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-app/src/mission.rs:60>) activates hidden player characters, substitutes default profiles, treats rail points as simple patrols, ignores waypoint command programs, and drops MEOW/objects/unknown actor groups at line 121.

10. **Medium — Sprite/profile decode failures silently degrade to colored circles.** This prevents format regressions from failing mission tests.

### Window/presentation

1. **Medium — EOF leaves controlled mode permanently frozen.** The stdin reader exits on EOF at `rpc.rs:145–163`, but `App.rpc` remains `Some`, so `tick_if_due` at `window.rs:365–373` never resumes autonomous ticking.

2. **Medium — Focus loss does not synthesize releases.** There is no `WindowEvent::Focused(false)` handling, so held keys/buttons can remain authoritative.

3. **Medium — Resize/fullscreen does not remap a stationary pointer.** Mapping is updated only on `CursorMoved`.

4. **Low — F11 press is consumed, but release becomes `KeyUp(Function(11))`.** See `window.rs:543–563`.

5. **Medium — Window scale multiplication can overflow.** `viewport * scale` at `window.rs:469–474` accepts an arbitrary `u32` CLI scale.

6. **Medium/Cross-platform — Surface color space is implicit.** `get_default_config` is used without explicitly selecting and testing an sRGB format. Backend-dependent presentation differences remain possible.

7. **Low/Performance — `ControlFlow::Poll` plus unconditional redraw requests creates a permanent busy loop.**

### `opensherwood-tools`

1. **Medium — Most commands read entire inputs.** [tools/main.rs:465](</C:/Users/przem/source/repos/opensherwood/crates/opensherwood-tools/src/main.rs:465>) uses `std::fs::read`. Inspecting a 565 MiB BKS or another hostile file requires the whole file in memory.

2. **Medium — Font-sheet CLI dimensions are unchecked.** User-controlled `columns` participates in unchecked multiplication at lines 325–338; `Canvas::new` then performs unchecked size arithmetic at lines 484–490.

3. **Medium — “Every format” is overstated.** `inspect` handles SRES, chunk containers, SCB header, RHS/DIC, and image blobs at lines 528–603. Sound tables, RED, profiles, saves, and Bink are only magic-detected or not semantically parsed.

4. **Low — Output paths can overwrite arbitrary user files.** This is normal CLI behavior but should be documented where tools produce game-derived local assets.

### Python harness and analyst tools

1. **Medium — RPC write/flush is outside the deadline.** [rpc.py:151](</C:/Users/przem/source/repos/opensherwood/harness/opensherwood_harness/rpc.py:151>) starts the timed wait only after blocking `stdin.write` and `flush`. A child that stops reading can hang the harness indefinitely.

2. **Medium — Stdout buffering is unbounded.** `_Reader(..., keep=None)` at line 142 stores unlimited unsolicited or unmatched output.

3. **Medium — Malformed JSON or response-ID mismatch does not kill/resynchronize the child.** Lines 170–175 raise while leaving the transport in an uncertain state.

4. **Medium — The committed digest is sparse.** [golden_digest.py:4](</C:/Users/przem/source/repos/opensherwood/harness/tools/golden_digest.py:4>) says per-tick hashes are recorded, but only every fiftieth is committed at line 40. A transient divergence that later reconverges is invisible.

5. **Low — The “restore fuzz” test is deterministic enumeration, not fuzzing.** `test_determinism.py:76–92`.

6. **Medium — The occluder test never checks occlusion.** [test_mission.py:59](</C:/Users/przem/source/repos/opensherwood/harness/tests/data/test_mission.py:59>) names occluders but never captures or examines a framebuffer.

7. **Medium — Map-view tests do not prove sprites decoded.** They check animation metadata and background color diversity, but fallback circles would still pass.

8. **Medium — Mission tests are shallow.** They prove all 39 files load, counts are plausible, movement is deterministic, and one path completes. They do not test ambiance, profiles, hidden actors, objects, scripts, occlusion pixels, music, or original behavior.

9. **Medium — Analyst-tool dependencies are undocumented.** `rhcap.py` needs `pyautogui` and `pywin32`, neither present in `harness/requirements.txt` nor documented in `docs/build.md`.

10. **Medium/Safety — `rhcap.py kill` kills every process named `Robin Hood.exe`.** See `harness/tools/original/rhcap.py:273–274`; it should target the captured PID.

11. **Low/Portability — `rhp_chunks.py:11` contains a maintainer-specific absolute fallback path.**

12. The numerous `harness/tools/probe/*.py` scripts contain no embedded game byte arrays or decompiler output that I found. They are exploratory observation scripts, however, not hardened parsers: many assume the retail corpus, read whole files, use assertions/unchecked slicing, and write derived PNG/JSON when directed. Rename the directory as already promised and keep outputs under ignored roots.

### Repository scripts and CI

1. **High/Process safety — The “read-only review” script automatically commits and pushes.** [codex_full_review.ps1:1](</C:/Users/przem/source/repos/opensherwood/scripts/codex_full_review.ps1:1>) describes a read-only review, but lines 25–29 write a tracked report, run `git commit`, and push. It may also include unrelated already-staged files. Review generation and human-approved publication must be separate operations.

2. **Medium/Supply chain — CI actions are moving tags.** [.github/workflows/ci.yml:18](</C:/Users/przem/source/repos/opensherwood/.github/workflows/ci.yml:18>) uses action major-version tags, and line 37 uses `dtolnay/rust-toolchain@master`.

3. **Medium — Python dependencies are top-level pinned but not hash-locked.** `harness/requirements.txt:1–5` does not lock transitives or hashes.

4. **Medium — No licence, vulnerability, or source-origin policy is automated.** Add `cargo-deny`/`cargo-about`, Python dependency locking, and advisory checks.

5. **Medium — No fuzz or 32-bit CI job exists.** These are acknowledged in the roadmap but remain important given the parser arithmetic findings.

6. **Low — Roadmap says CI should use nextest, but workflow uses `cargo test`.** The M0 disposition also says nextest is pinned even though it is not present.

7. **Positive finding.** The normal CI matrix covers Linux, Windows, and macOS; uses the lockfile; runs fmt, clippy, tests, release build, synthetic pytest, deterministic digest, skill sync, and policy checks.

### Documentation

1. **Medium — Legal policy contradicts the accepted clean-room ADR.** [docs/legal.md:25](</C:/Users/przem/source/repos/opensherwood/docs/legal.md:25>) still describes a private `re/` directory and lines 47–50 permit implementation after static analysis “in the same sitting.” ADR-0003 requires a separate context/session at [ADR-0003:24](</C:/Users/przem/source/repos/opensherwood/docs/decisions/ADR-0003-clean-room-roles.md:24>).

2. **Medium — Provenance sections do not meet ADR-0003’s own schema.** The ADR requires build, edition/language, exact file/offset, method, reproduction, confidence, author/date, and dependent tests at lines 31–37. Most format specs provide only “Observation” plus a generic script list.

3. **Medium — The M0 review audit trail is incomplete.** `2026-09-02-codex-m0-review-disposition.md:3` references `2026-09-02-codex-m0-review.md`, but that file is absent from the working tree and all Git refs.

4. **Medium — `docs/harness.md` is stale.** The example reports protocol/ruleset 1 at line 27, while current values are 2/2. Mission documentation at line 50 says geometry is absent even though it is loaded.

5. **Medium — `docs/build.md:59` references nonexistent `scripts/ci_local.sh` and `.ps1`.**

6. **Medium — README status is stale.** [README.md:7](</C:/Users/przem/source/repos/opensherwood/README.md:7>) says missions do not exist, while mission scenarios and tests do. It also describes original-game comparison that does not exist.

7. **Low — Mojibake appears in legal/ADR text**, including the publisher parent-company name and section symbols. Legal documents should not contain encoding damage.

---

# 2. Verification of every format-spec status

The index defines “verified” as both parsing every retail file and cross-checking the interpretation by rendering, listening, or matching the original at [docs/formats/README.md:4](</C:/Users/przem/source/repos/opensherwood/docs/formats/README.md:4>). Several statuses do not satisfy that definition.

| Spec | Current claim | Evidence in parser/tests | Review verdict |
|---|---|---|---|
| `README.md` index | Summary of all statuses | Several entries collapse “container verified” and “semantics verified.” | **Needs correction.** Use separate columns for framing, parser coverage, interpretation, original-game validation, and automated regression. |
| `image-blob.md` | Verified container and RGB565 | Bounded parser, exact decompressed length, all image-like files iterated by the data test; visual checks documented. | **Reasonable, with caveat.** Container and RGB565 are supported. Original renderer blending/channel behavior is not automated. |
| `sres.md` | Container verified for retail archives | Parser consumes known tags and validates observed trailer offsets; test recursively parses `.res`. Version is unchecked, trailer optional, aggregate decode unbounded, widget-state meanings unknown. | **Corpus framing verified; semantics partial; hostile parser not hardened.** |
| `rhp.md` | Mostly decoded; every chunk framed | Parser validates known root/child versions and consumes nine files. Several chunks remain raw; gameplay meanings and original motion behavior remain unknown. | **Status mostly honest.** The index should say “record framing verified, gameplay semantics partial,” not broadly “verified.” |
| `rhm.md` | Decoded; every chunk consumed | Tests establish ten chunks/order over 39 files, but parser does not enforce them; POUF remains heuristic/raw; many semantics unknown. | **Overstated. Downgrade to partial.** “All retail files framed and selected record types parsed” is accurate. |
| `scb.md` | Container decoded, instruction set partial | All 39 scripts are parsed in the data test; tests verify calls/jumps/prologues. Parser itself accepts inconsistent structures. Opcode/native semantics remain largely unknown. | **Honest at the format-knowledge level.** Add parser-contract caveats. |
| `sprites.md` | All RHS/DIC/BKS verified; all 404,855 streams consumed | RHS references and DIC inventory are tested. Current Rust regression samples ~4,174 page frames and 500 page-less frames. Analyst scripts document a prior all-stream pass and recognizable renders. | **Observationally supported, not continuously verified.** Label the all-frame result as a dated corpus observation and add a full local regression mode. |
| `sprite-animations.md` | Block/order/action tagging verified; timing/displacement partial | Unit tests cover synthetic tables; data-backed test covers only Robin, one soldier, and Child. Visual action identification comes from selected sheets; timing is knowingly not implemented. | **Broadly honest in the spec, but index wording is too strong.** Keep “partial,” especially for meanings and timing. |
| `fonts.md` | Container, glyph table, layers, TFN/config verified | Data test parses all fonts and checks extents/layers. Exact advance/layout is explicitly inferred and renderer currently double-applies `x_adjust`. | **Container/pixels verified; text layout partial.** Split the status accordingly. |
| `sound.md` | Tables stub; audio containers verified | Audio code only sniffs and decodes Ogg/WAVE through Rodio. No parser for FXG/SFK/NEUF and no data-backed audio regression. | **Stub is honest.** “Audio containers observed/decoder-supported” is more precise than “verified.” |
| `red.md` | Stub | No Rust parser; campaign note infers a tail layout over 57 files. | **Honest as stub.** Move new claims into a parser/test before upgrading. |
| `profile.md` | Stub | Magic detection only; no parser/test. | **Honest.** |
| `savegame.md` | Stub | Magic detection only; no parser/test. | **Honest.** |
| `video.md` | Descriptive Bink note, no explicit status | Magic detection only; no decoder or structural parser. | **Add explicit `stub` status.** |

Common corrective action for every spec:

- Add a machine-readable or standardized provenance block with build hash, edition/language, corpus count, observation method, reproducible command, confidence, author/date, and exact test names.
- Separate four concepts: “framing known,” “all observed files parse,” “semantics interpreted,” and “matched original behavior.”
- Do not use “verified” for manual scratch output that cannot be reproduced from committed tooling.

---

# 3. Legal, clean-room, history, and licence audit

## Committed history

I examined all 23 commits with `git log --stat`, targeted full patches where provenance mattered, initial-file contents, and all refs.

Timeline:

- `6d7c53e`: legal policy, format specifications, and original-game notes.
- `c6e77c1`: governance, ADRs, clean-room policy, CI, and safety scripts.
- `8bd8cb0`: first Rust implementation.
- Later commits progressively added formats, renderer, audio, mission loading, pathfinding, reviews, and black-box UI/campaign observations.
- `277af518`: introduced the critical copied text.

Positive result: the clean-room ADR was committed before engine implementation began. The initial research predates the formal ADR, but no engine code existed at that point.

## Binary/media asset history

An all-ref object scan found no proprietary executable, archive, image, audio, video, map, or game-format blob. The largest committed blob is `Cargo.lock` at 85,435 bytes; Rust source is next. No large historical object or known game magic was found.

This does **not** yield a clean audit because the narrative text in C-01 is game data under the project’s policy.

## Decompiler/static-analysis contamination

I found:

- no decompiler output;
- no disassembly listings;
- no Ghidra database;
- no address map;
- no reconstructed proprietary pseudocode;
- no original executable bytes;
- no evidence that proprietary source was copied.

Most format work is explicitly based on file observation, hexdumps, statistics, decompression, rendering, and printable strings. That is allowed by ADR-0003.

However:

- provenance generally lacks the detailed attestations required by ADR-0003;
- the entire history was produced in one day by one human, usually with the same AI co-author/session;
- no per-subsystem analyst/implementer attestation exists;
- `docs/legal.md` contains an obsolete, weaker role-separation rule.

Therefore the defensible conclusion is: **no evidence of decompiler-derived implementation was found, but the repository cannot positively prove role separation for every claim.**

## Community-source provenance

[docs/formats/scb.md:97](</C:/Users/przem/source/repos/opensherwood/docs/formats/scb.md:97>) and its provenance cite the GPL-3 OpenDeathValley disassembler for field-order and opcode hypotheses and say no code was copied. GPLv3 is compatible with this project, and the hypotheses were reportedly revalidated against Robin Hood data.

Retain precise attribution and distinguish copied facts, independently observed facts, and hypotheses. If any code is later adapted, preserve licence notices and source history explicitly.

## Legal-policy inconsistencies

- `docs/legal.md` must be reconciled with ADR-0003.
- C-01 must be remediated before making any public “no assets” representation.
- The policy checker is a safety net, not proof.
- The missing original M0 review prevents independent verification of its disposition.
- Exact retail UI strings should be runtime-loaded, not embedded in Rust or documentation.
- Reference captures and decoded sheets must remain local and ignored; their hashes/metrics may be committed.

## Cargo dependency licences

The Windows-resolved dependency graph contains only crates.io and workspace dependencies—no Git dependencies.

Observed licence families:

- MIT, Apache-2.0, MIT/Apache dual licence;
- BSD-3-Clause, ISC, Zlib, CC0, Unlicense/MIT;
- Unicode-3.0;
- MPL-2.0 for Symphonia and its subcrates.

No dependency licence incompatible with GPL-3.0-or-later was identified. MPL-2.0 is file-level copyleft and can be included in a GPL larger work, but its notices and source-availability obligations still apply to modified MPL files.

Limitations:

- The offline cache did not contain every non-Windows target package, so the licence inventory was complete only for `x86_64-pc-windows-msvc`.
- There is no automated allowlist or notice generation.
- System libraries used on Linux/macOS were not independently inventoried.
- Python top-level packages are permissively licensed, but their full transitive graph is not hash-locked or audited.
- Analyst tools also depend on undeclared `pyautogui` and `pywin32`.

Add all-target `cargo-deny` and `cargo-about` jobs, generate a third-party notice, and lock/audit the Python environment.

---

# 4. Architecture and roadmap assessment

## What is architecturally strong

- The crate boundaries are sensible and no upward dependency violation was found.
- `opensherwood-formats` is I/O-free and mostly uses explicit bounded readers.
- `opensherwood-core` avoids wall-clock and platform APIs.
- Fixed-point operations, named PCG state, explicit hash encoding, sorted authoritative sets, and snapshot tests form a credible M0 determinism base.
- Rendering and audio are non-authoritative.
- The app supports the same protocol in headless and window modes.
- The three-platform CI matrix is a good foundation.
- Real-data tests remain local, consistent with the legal policy.

## Largest architectural risks

1. **Legal cleanliness and provenance.** A clean-room project cannot survive casual copied-text commits or unverifiable provenance.
2. **No original-game oracle.** Without measured timing, state traces, and image comparisons, work will optimize internally consistent guesses rather than fidelity.
3. **Script/native interface.** The campaign is driven by SCB and native calls; neither semantics nor authoritative ownership is designed.
4. **Motion/depth model.** Current navigation is a new grid system, not the original EULER/path/layer/door model. Rendering lacks actor depth order. Both affect nearly every mission.
5. **Mission semantic loss.** Profiles, hidden actors, objects, teams, rails, commands, variants, and scripting bindings are being flattened before their meaning is established.
6. **Snapshot composition.** Script, scheduler, campaign, and future mod state must enter one versioned snapshot/hash contract.
7. **Modding abstractions.** Raw retail numeric IDs and parser structures should not become the long-term public mod API.
8. **Verification scaling.** Manual scratch scripts and broad status prose will not sustain hundreds of parser/gameplay changes.

## Roadmap accuracy

- M0 correctly remains “in progress,” but the CI checkbox at `docs/roadmap.md:17` is stale: CI exists, though it uses `cargo test`, not nextest.
- The “Codex M0 review incorporated” checkbox should not imply every issue is closed; many are explicitly deferred and several fixes remain incomplete.
- M1’s RHM checkbox overstates “all chunks decoded”; POUF remains raw and semantics are incomplete.
- The font checkbox should distinguish decoding from correct layout.
- The mission checkbox is technically true only as “loads selected actors over a background,” not as mission behavior.
- The first oracle trace remains unchecked and should gate further behavioral implementation.
- M2’s VFS checkbox is unchecked despite an implementation existing; that is appropriate because language/mod/Unicode/fingerprint behavior is not yet production-ready.
- README’s “missions not yet” statement is stale.
- Menu/campaign work has started before the oracle and legal-text-loading foundations are complete.

## Recommended order of work

1. **Legal containment and governance repair.**
   Remove copied game text, fix ignore/checker coverage, reconcile legal/ADR rules, restore missing review provenance, and introduce contribution attestations.

2. **Return the repository to a reproducibly green state.**
   Complete or shelve the dirty menu work, bump versions, run fmt/clippy/all Rust tests, run all Python tests with writable artifact storage, and confirm CI on all platforms.

3. **Harden hostile-input boundaries.**
   Shared decode budgets, SRES aggregate limits, geometry bounds/i128, RPC bounded drains, replay recording quotas, streaming replay input, artifact containment, and parser version validation.

4. **Build the real oracle gate.**
   Implement the trace schema, capture one controlled original run, measure the original time model, establish local-only image comparison, and record provenance without committing assets.

5. **Write the authoritative-state/VM ADR.**
   Decide who owns VM, scheduler, campaign, native-call, and mod-script state and how all of it snapshots and hashes.

6. **Resolve motion and depth before broad gameplay.**
   Decode or behaviorally specify EULER/path graph, layers, sectors, doors, actor depth ordering, occluder composition, walk/run timing, and animation displacement.

7. **Preserve full mission semantics during import.**
   Represent teams, hidden/inactive actors, objects, profile IDs, rail command programs, ambiance, mission/map identity, and script bindings without premature fallback mapping.

8. **Implement the minimum SCB/native subset for the tutorial.**
   Every opcode and native call should come from a provenance-backed spec and deterministic test.

9. **Create one input-only tutorial vertical slice.**
   Loading, briefing, navigation, AI/stimuli, objectives, success/failure, debriefing, and replay verification.

10. **Only then expand campaign UI, saves, and mod APIs.**

---

# 5. Prioritized fix list

## P0 — Immediate containment

1. Remove or take offline copied game prose in `docs/original/campaign-flow.md:36–57,74–77`.
2. Reduce exact copied UI strings in `docs/original/ui-flow.md` to IDs, geometry, state names, hashes, and paraphrased behavior.
3. Do not commit the hardcoded retail labels in `crates/opensherwood-app/src/ui.rs:54–60,304–309`; load them from player data.
4. Obtain a decision on history rewrite for commit `277af518`.
5. Add `/harness/goldens/`, derived-image extensions, and root `re` enforcement to `.gitignore` and `check_no_assets.py`.
6. Change the policy checker’s success wording so it does not claim to prove the absence of all assets.

## P1 — Restore build and security integrity

7. Complete the `Scenario::Menu` reset/parse path and add `ui` to every `ObserveResult`.
8. Bump ruleset/snapshot/protocol versions as required by the new scenario and viewport semantics.
9. Make current `cargo fmt`, clippy, Rust tests, synthetic tests, and data tests green from a fresh build.
10. Add shared sprite decode limits and route `export-frame` through them.
11. Add SRES cumulative decode budgets or lazy images.
12. Bound oversized-line draining without allocating the discarded line.
13. Enforce replay file size before reading, cumulative recording quotas, and practical playback time limits.
14. Bound geometry coordinates or move cross-products to `i128`.
15. Replace partial content digests with full cached streaming hashes and propagate I/O failure.
16. Make session restore transactional.
17. Validate snapshot `hash_schema`, content identity, and catalog-relative animation state.

## P2 — Verification and format honesty

18. Implement `oracle/schema/trace-v1.md` and the first controlled original trace.
19. Implement the documented local-only golden/image comparison pipeline with correct ignore rules.
20. Add all-frame sprite decoding as an explicit slow/local data test.
21. Add fuzz targets for image blob, SRES, chunk, RHP, RHM, RHS/DIC/BKS, fonts, SCB, replay, and RPC input.
22. Enforce versions, required chunk sets, duplicate policy, and cross-reference validation inside parsers, not only corpus tests.
23. Relabel RHM as partial and split every status into framing/corpus/semantics/oracle/regression dimensions.
24. Replace silent Rust “skip by return” with explicit ignored/configured data tests.
25. Bring every provenance section up to ADR-0003’s required schema.

## P3 — Fidelity foundation

26. Fix font `x_adjust` double counting and add layout fixtures.
27. Replace slot-order rendering with a documented stable depth model.
28. Correct overlapping sprite/occluder composition and remove the duplicate pass.
29. Make required RHP/profile/sprite failures fail scenario reset rather than silently downgrade.
30. Model ambiance, map ID checks, hidden PCs, teams, objects, profiles, and rail command programs.
31. Implement distance-based animation timing and per-animation displacement from verified behavior.
32. Use scenario-appropriate mission music and a streaming loop mechanism.
33. Decide the authoritative VM/scheduler/campaign ownership in an ADR before writing the interpreter.

## P4 — Portability, supply chain, and maintenance

34. Add explicit language selection and defined mod precedence.
35. Normalize Unicode paths, sort case-insensitive root discovery, and contain/cycle-check symlinks.
36. Pin CI actions by commit SHA.
37. Add all-target `cargo-deny`, `cargo-about`, vulnerability checks, and third-party notices.
38. Produce a hash-locked Python environment including optional analyst-tool dependencies.
39. Put Python writes under a real deadline and bound stdout.
40. Fix controlled-window EOF, focus releases, pointer remapping, sRGB selection, scale limits, and busy polling.
41. Split review generation from commit/push in `scripts/codex_full_review.ps1`.
42. Add the missing local CI scripts or remove their documentation.
43. Restore or explicitly account for the missing M0 review document.
44. Update README, harness, architecture, roadmap, and encoding-damaged legal text.

The immediate conclusion is straightforward: the deterministic prototype is worth preserving, but the repository should not advance to more UI or campaign implementation until the legal leak, current build breakage, hostile-input boundaries, and missing oracle gate are resolved.