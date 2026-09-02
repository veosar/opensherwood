# Architecture review

## Executive verdict

Proceed, with five material changes:

1. Keep Rust.
2. Keep `winit` + `wgpu`, but isolate them behind a very small platform boundary and remove the initial `softbuffer` fallback.
3. Design the replay/state protocol before implementing real gameplay.
4. Replace the horizontal “decode everything” M1 with risk spikes followed by a playable vertical slice.
5. Establish an actual clean-room separation before anyone who sees decompiler output writes engine code.

The current plan is technically credible, but its clean-room definition is insufficient, its first format milestone is too broad, and the proposed RPC surface mixes player input with privileged engine commands.

## Verified observations from this installation

I checked the installed files rather than relying only on the brief:

- All 233 inspected RHS files and `robinhood.dic` begin with the same little-endian value, `0x0003EBC9`. Because the executable contains the error “RHS file … was not generated with the current bank,” this is probably a bank-generation/build identifier, not an RHS/DIC file-type magic.
- `robinhood.bks` is 592,261,466 bytes. Every `u16` in the first 8 MiB is between 0 and 4095, and that sample contains all 4096 possible values. `0x066D` accounts for about 63% of that sample. This strongly suggests a 12-bit symbol/code stream stored as 16-bit words, not an offset table.
- The DIC sample spans the full `u16` range and contains many values resembling RGB565 colors. It is plausibly per-frame codebook/palette/decompression data, but that is not yet proven.
- `DATA/Levels/Day/sherwood.map` has a 12-byte header containing `1920 × 1088`, version 2, and a payload length exactly equal to file size minus 12. Its bzip2 payload expands to exactly `1920 × 1088 × 2` bytes.
- `sherwood.min` similarly contains `225 × 182`, version 1, a payload length, and a zlib stream expanding to exactly `225 × 182 × 2` bytes. MAP/MIN decoding is therefore already close to solved: header plus compressed RGB16 pixels.
- This `Robin Hood.exe` has SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base `0x00400000`, relocations stripped, and no ASLR. It contains both `GetTickCount` and `timeGetTime`. That makes a version-specific Frida oracle substantially easier than usual.
- There are 39 RHM/SCB pairs, but that does not mean 39 distinct campaign missions must all become the same regression gate. `EmbTut_FoC_EC` is the obvious tutorial vertical-slice candidate.

The commercial/legal caution is warranted: the title remains actively sold by [Steam](https://store.steampowered.com/app/46560/Robin_Hood_The_Legend_of_Sherwood) and [GOG](https://www.gog.com/en/game/robin_hood), and GOG has recently updated its preserved build.

## 1. Rust versus C++20

Decision: use Rust.

The strongest reasons are not abstract memory-safety claims; they are specific to this project:

- Most early work consists of parsers for malformed or only partially understood binary formats. Bounds-checked slices, explicit integer conversions, fuzzing, and structured errors materially reduce iteration time.
- Cargo provides one reproducible dependency, build, test, benchmark, and fuzz ecosystem. That is especially valuable when autonomous agents are expected to iterate without repairing CMake/package-manager drift.
- The engine does not need ABI or class-layout compatibility with the original. Matching an MSVC6 object model would only matter inside the separate oracle.
- Deterministic simulation benefits from Rust’s ability to restrict mutable state and make nondeterministic container use conspicuous.

C++ has advantages in SDL, FFmpeg, Frida examples, mature profilers, and direct correspondence with old engine idioms. Those advantages belong mostly in tooling and the oracle, not the clean implementation.

Concrete constraints:

- Pin the toolchain in `rust-toolchain.toml` and commit `Cargo.lock`.
- Keep unsafe code out of formats, simulation, scripts, and replay code. Confine it to platform/FFI modules.
- Do not adopt a large Rust game engine or general ECS.
- Use explicit bounded binary readers rather than clever parser abstractions that obscure offsets.
- Use `cargo-nextest`, fuzz targets, and `sccache` early.
- Treat Android and web as later validation targets, not M0 acceptance gates.

One change to the dependency plan: do not run LuaJIT on desktop and Lua 5.1 elsewhere. That creates platform-dependent mod behavior. Use one Lua 5.1-compatible interpreter everywhere for the compatibility tier. An optional JIT can be an explicitly non-authoritative performance feature later.

Likewise, do not “port FFmpeg’s `bink.c`” casually. Either integrate a separately licensed decoder after a license audit or invoke/link FFmpeg under a documented distribution model. Video is not on the critical path.

## 2. Presentation stack

Decision: retain `winit` + `wgpu`.

SDL3 is entirely viable. It supports Android and Emscripten, exposes absolute and relative mouse motion, and has both simple rendering and a modern GPU API. Its Emscripten documentation also points out that hundreds of megabytes of bundled assets are problematic in browsers, which applies directly to this game’s approximately 1 GiB data set. [SDL platform support](https://wiki.libsdl.org/SDL3/README-platforms), [SDL mouse events](https://wiki.libsdl.org/SDL3/SDL_MouseMotionEvent), [SDL Emscripten notes](https://wiki.libsdl.org/SDL3/README-emscripten).

Nevertheless, `winit` + `wgpu` better preserves the proposed single-Cargo workflow. Winit supports both Android lifecycle integration and `wasm32-unknown-unknown`; wgpu covers DX12, Vulkan/Android, Metal, WebGPU, and WebGL fallback. [Winit Android](https://docs.rs/winit/latest/winit/platform/android/), [Winit web](https://docs.rs/winit/latest/winit/platform/web/), [wgpu backends](https://docs.rs/wgpu/latest/wasm32-unknown-unknown/wgpu/struct.Backends.html).

The important design is:

```text
deterministic CPU renderer
        |
        +-- capture/hash directly from CPU framebuffer
        |
        +-- narrow presentation trait
                |
                +-- winit + wgpu texture upload
```

Rules for the reference presentation mode:

- CPU framebuffer is authoritative.
- Upload as non-sRGB RGBA8.
- Nearest-neighbor sampling.
- No filtering, color correction, or shader effects.
- Screenshots come from the CPU buffer, never GPU readback.
- Keep a reference RGB565/integer-blending path if oracle results show the original compositor operates that way.
- Add widescreen, shaders, and modern color handling only as separate presentation modes.

Do not add `softbuffer` initially. It doubles platform paths before there is evidence that wgpu cannot present one streaming texture adequately.

Input fidelity is not decided by SDL versus winit. The engine must preserve:

- absolute pointer trajectory;
- button transitions and ordering;
- wheel events;
- physical key identity;
- high-DPI/window-to-logical-coordinate transforms;
- multiple motion events within one simulation tick.

This matters because mouse gestures and drag paths may be gameplay inputs, not just cursor positions.

## 3. Workspace split

The proposed split is too granular for M0 and is missing the most important boundary: asset resolution.

I recommend:

```text
crates/
  opensherwood-formats       bounded readers and format models
  opensherwood-assets        VFS, install discovery, overlays, fingerprints, caches
  opensherwood-core          authoritative world, scheduler, AI, orders, pathfinding
  opensherwood-script        SCB parser/VM and script-native boundary
  opensherwood-render        deterministic CPU compositor
  opensherwood-protocol      RPC, replay, observation, snapshot and hash schemas
  opensherwood-app           headless and interactive binaries
  opensherwood-tools         inspectors/exporters; editor added later
harness/
  Python RPC client, tests, image/state comparisons
oracle/
  public trace schema/controller documentation only
testdata/
  explicitly synthetic fixtures with provenance
```

Specific changes:

- Add `opensherwood-assets`. It must implement explicit path override, GOG/Steam discovery, case-insensitive lookup on case-sensitive systems, language/edition overlays, mod overlays, and a content fingerprint. The `2047/data` overlay behavior belongs here, not in individual parsers.
- Keep all formats as modules inside one crate until compile time or dependency direction proves a split necessary.
- Split SCB execution from Lua modding. Lua is not needed until the engine has a stable native gameplay API.
- Delay `opensherwood-audio` and `opensherwood-editor`; empty crates are architectural ceremony.
- Do not create a shipping `opensherwood-oracle` crate. Frida/Ghidra integration is Windows-only research tooling. Put the shared trace types in `opensherwood-protocol`; keep the version-specific oracle adapter on the analysis side of the clean-room boundary.
- Add a synthetic test-world module or test-support crate only when multiple crates need it.

Do not use a general ECS initially. Use stable generational entity IDs, typed component stores/arenas, explicit systems, and deterministic iteration order. A general ECS makes serialization, state hashing, and behavior tracing harder, while this game has a modest and well-defined entity taxonomy.

## 4. Harness, replays, hashing, and savestate fuzzing

### Transport and minimal RPC

Use JSON-RPC 2.0 over newline-delimited stdin/stdout for headless tests:

- no fixed-port collisions;
- clean subprocess ownership;
- logs can go exclusively to stderr;
- CI does not expose a listening socket.

An optional loopback socket can support an interactive game later.

The minimal surface should be:

| Method | Purpose |
|---|---|
| `hello` | Protocol version, capabilities, build/ruleset version, content fingerprint |
| `reset` | Load a synthetic scenario or mission with seed/configuration |
| `step` | Atomically enqueue canonical input events and advance exactly N ticks |
| `observe` | Return filtered structured state/UI/objectives and component hashes |
| `snapshot` | Create an authoritative internal checkpoint |
| `restore` | Restore a checkpoint or supplied snapshot |
| `capture` | Return framebuffer hash and optionally write under a fixed artifact directory |
| `shutdown` | Clean termination |

`step` is the key primitive:

```json
{
  "ticks": 12,
  "events": [
    {"tick_offset": 0, "sequence": 1, "kind": "pointer_move", "x256": 82240, "y256": 46080},
    {"tick_offset": 1, "sequence": 0, "kind": "pointer_down", "button": "right"},
    {"tick_offset": 1, "sequence": 1, "kind": "pointer_up", "button": "right"}
  ]
}
```

`select_unit`, `order`, `attack`, and `console` may exist as debug extensions, but they must not define the canonical player/replay interface. A campaign automation test that invokes `order(move)` directly does not prove selection, picking, UI, mouse gestures, or command translation.

### Replay schema

Define `ReplayV1` at M0:

- protocol and ruleset version;
- content fingerprint;
- mission/scenario identifier;
- canonical logical viewport;
- tick-rate rational;
- initial seed and named RNG-stream states;
- ordered events identified by `(tick, phase/sequence)`;
- optional non-authoritative intent annotations;
- optional checkpoint expectations.

Never serialize winit events, OS timestamps, physical paths, pointers, or physical display coordinates.

Start with JSON Lines for inspectability. Compress later without changing the logical schema.

### State hashing

Do not hash dumped JSON or arbitrary Serde output. Define a manual canonical byte encoding:

- domain prefix and hash-schema version;
- little-endian fixed-width fields;
- entities sorted by stable ID;
- deterministic map/set ordering;
- no caches, GPU state, handles, wall clock, audio mixer state, or diagnostic counters;
- fixed-point authoritative positions/time where practical;
- explicit normalization if floats remain;
- RNG algorithm, state, stream ID, and draw count;
- script VM stacks, globals, suspended calls, scheduler queues, and pending stimuli.

Return both an overall BLAKE3 hash and subsystem hashes:

```text
world
actors
orders
pathfinding
scripts
scheduler
rng
campaign
total
```

That makes the first divergence diagnosable.

### Savestate fuzzing

Internal snapshots and original-compatible saves are separate features.

From M0 onward:

1. Run a replay prefix.
2. Snapshot at a chosen tick.
3. Run the suffix and record hashes.
4. Restore the snapshot.
5. Run the identical suffix.
6. Compare every per-tick subsystem hash and selected framebuffer hashes.

Fuzz:

- snapshot points at every tick around state transitions;
- repeated save/load cycles;
- truncated/corrupt/oversized snapshot input;
- unknown version/tag handling;
- replay event splicing;
- synthetic worlds generated with property tests.

A restored snapshot must reconstruct authoritative state but may rebuild caches. Original save compatibility should not constrain the internal snapshot schema.

## 5. Oracle strategy

Use the three techniques in this order:

1. Frida hooks for authoritative tick and selected state.
2. Memory scanning only to discover/validate candidate layouts.
3. The built-in console for visual/semantic cross-checks.

Raw memory scanning is too fragile for a long-lived oracle. Console screenshots are excellent for geometry, zones, path graphs, and human verification, but poor for machine-readable tick traces.

MSVC6 `thiscall` is manageable: on x86, capture `ECX` at function entry as `this`, and read stack arguments according to the analyst’s specification. This installed executable is especially friendly because it is fixed at `0x00400000`, has stripped relocations, and lacks ASLR. Every address profile must be keyed by the complete executable hash.

### First concrete experiment

Use `EmbTut_FoC_EC` and trace one right-click movement:

1. Fingerprint the executable and data manifest.
2. On the analyst side, identify exactly one central simulation-update function and the player actor’s stable identification/position fields.
3. Hook `timeGetTime` and `GetTickCount`, both present in this executable, so all calls within one oracle step see a controlled time value.
4. Pause at the central tick boundary and advance it through a harness-controlled semaphore/message.
5. Capture at tick exit:
   - tick number and controlled time;
   - actor identity by mission ID/name or creation ordinal, never heap pointer;
   - position/elevation/facing;
   - current order and movement state;
   - animation/frame identifier if available;
   - RNG call count or results if a narrow RNG hook is found.
6. Inject one fixed mouse move/right-click sequence in a fixed-size window.
7. Record 200 steps.
8. Validate the observed destination/position against the game’s own actor overlay and screen projection.

The first goal is not full parity. It is a trustworthy trajectory with a proven clock boundary and two independent validations of the actor fields.

Do not assume original “frame” equals new-engine “tick.” First measure whether the original uses fixed, clamped, or variable delta time. Exact tick alignment may be impossible across MSVC6 x87 and modern x86-64/ARM arithmetic. Compare semantic transitions and quantized values until the time model is established.

## 6. Clean-room reverse-engineering workflow

The proposed `re/` directory ignored by Git is not a clean room. It prevents ordinary commits; it does not prevent implementers or agents from seeing the material, nor `git add -f`.

If “clean room” is truly non-negotiable, use separate roles and storage:

### Analyst side

May access:

- original binaries/assets;
- Ghidra projects and decompiler output;
- debugger/Frida sessions;
- mutation experiments on a private owned copy;
- private address maps and structure layouts.

May produce only:

- behavioral specifications;
- format-field specifications;
- small factual traces/test vectors;
- reproduction procedures;
- confidence/provenance records.

### Implementation side

May access:

- committed specifications;
- synthetic fixtures;
- public trace schema;
- normalized oracle results;
- the original game only through an approved black-box runner, if counsel accepts that arrangement.

It must not access decompiler output, annotated disassembly, private address maps, or copied algorithms.

If the same agents read Ghidra output and then implement the corresponding subsystem, call the process interoperability reverse engineering—not clean-room engineering. A context reset or `.gitignore` alone is not credible separation. Obtain actual counsel for the intended jurisdictions.

Each committed claim should carry:

```text
claim_id
status: observed | inferred | unknown
game_build_sha256
edition/language
source file and offset/chunk
observation method
reproduction steps
confidence
analyst/date
tests depending on claim
supersedes/superseded_by
```

Use neutral field names such as `unknown_0x24` until semantics are demonstrated. Never commit pseudocode reconstructed from the executable, large hex dumps, lookup tables copied from assets, Ghidra databases, or screenshots containing game art.

Also add repository policy for:

- no game data in commits, releases, CI artifacts, issue attachments, or test snapshots;
- trusted self-hosted data tests never running on untrusted pull requests;
- proprietary screenshots and traces remaining local;
- synthetic fixtures carrying an origin/provenance manifest;
- contributor attestation that no leaked source or decompiler-derived implementation was submitted.

The storefront identifies Microids as publisher, but a legal document should not treat that alone as conclusive proof of the entire rights chain. Have counsel verify the relevant companies and title marks.

## 7. Sprite-bank analysis

The initial premise should be revised:

> BKS is probably a `u16` stream of 12-bit symbols. DIC is likely decoding metadata/codebooks. RHS appears to associate named animation profiles with bank-generation and frame references.

Recommended sequence:

1. **Inventory and cluster RHS files.** Record build ID, the word at offset 4, name field, sizes, candidate counts, and repeated record widths. Group by the apparent RHS variants rather than forcing one schema.
2. **Correlate Day/Fog/Night counterparts.** Same-name profiles can distinguish geometry/frame references from ambience-specific dictionary/palette information.
3. **Analyze BKS as symbols.** Measure run lengths, transitions, entropy, symbol frequency by region, and likely frame boundaries. Test whether `0x066D` is transparent-fill, repeat, end-of-line, or simply a common dictionary entry.
4. **Analyze DIC as variable records.** Test hypotheses involving 4096-symbol codebooks, per-frame palettes, Huffman tables, run commands, and block/tile dictionaries. Its size is not an exact sequence of 4096-entry `u16` tables, so do not assume a global palette.
5. **Start with simple assets.** Arrow, coin, ale, purse, blip, and single/static objects give known small silhouettes and repeated frames.
6. **Trace file I/O dynamically.** Hook open/read/seek calls for BKS, DIC, and one RHS while the original loads or displays a chosen sprite. This directly reveals which byte ranges correspond to one frame without understanding the decompressor.
7. **Capture post-decode surfaces.** Intercept the DirectDraw/D3D lock/upload path or a narrow decoded-frame function and dump dimensions, pitch, pixel format, and framebuffer bytes privately.
8. **Run differential mutations on a private copy.** Change one BKS symbol or a small DIC region, display the same isolated frame, and observe the affected pixel/run/tile. Never mutate the installed copy or commit the results as assets.
9. **Test hypotheses mechanically.** Begin with 12-bit LZW-like codes, RLE/control tokens, palette indices, delta frames, and vector-quantized blocks. Reject each with explicit invariants.
10. **Only then write the decoder.** Make it streaming or memory-mapped; do not load 565 MiB into memory. Validate bounds and output dimensions before allocation.

The first exit criterion should be “one independently verified frame,” then “all frames of one profile,” then “cross-ambience profiles”—not “extract the entire sprite bank.”

## 8. AGENTS.md, CLAUDE.md, and skills

Use `AGENTS.md` as the sole policy source of truth. It should contain:

- project purpose and non-goals;
- legal/asset/clean-room boundary;
- deterministic-state rules;
- repository layout and dependency direction;
- approved build/test commands;
- specification/provenance requirements;
- definition of done;
- forbidden files and CI artifact policy.

`CLAUDE.md` should be minimal:

```markdown
@AGENTS.md

# Claude adapter

`.claude/skills` is generated from `.agents/skills`.
Never edit generated skill files directly.
```

Use `.agents/skills/<name>/` as canonical. Mirror the complete skill directory—not only `SKILL.md`—into `.claude/skills/<name>/`, preserving relative references and assets.

`scripts/sync_skills.py` should support:

```text
--write   replace the generated mirror
--check   compare file sets and SHA-256 of every file
```

CI should run:

```text
python scripts/sync_skills.py --check
python scripts/check_no_assets.py
cargo fmt --check
cargo clippy --workspace --all-targets
cargo nextest run --workspace
pytest harness/tests/synthetic
git diff --exit-code
```

The mirror should contain a generated marker, and CI must fail on extra as well as missing files. Avoid project rules duplicated in individual skills; skills should describe bounded workflows such as format investigation, deterministic replay triage, or provenance recording.

## 9. Project name

“OpenSherwood” does not reproduce the title mark, but I would not choose it as the final public name.

Problems:

- extremely generic and weak in search results;
- prior game-sector use, including the historical OpenSherwood Entertainment name;
- likely collisions across repositories, packages, companies, and domains;
- `opensherwood-*` crate prefixes are also generic.

Use it only as an internal codename until a proper GitHub, crates.io, domain, EUIPO/WIPO/USPTO, and general commercial search is complete.

A more distinctive working candidate is **Yewglass** with `yewglass-*` crate names. A preliminary web search found no obvious game/engine collision, but that is not trademark clearance. Avoid names such as Loxley, Merry Men, Nottingham, or anything containing the prohibited title terms.

The public README should use the original game title only in a compatibility statement and include a clear independent/unaffiliated disclaimer, reviewed by counsel.

## 10. Roadmap changes and biggest risk

Replace the horizontal roadmap with this:

| Milestone | Exit criterion |
|---|---|
| M0: Governance and deterministic kernel | Clean-room roles documented; asset audit active; synthetic world can replay, snapshot, restore, hash, and render identically in CI |
| M1: Feasibility gates | Exact MAP/MIN decode; one verified sprite frame; SCB disassembler consumes the tutorial script with explicit unknowns; first controlled oracle trace |
| M2: Scene vertical slice | VFS resolves base/language overlays; tutorial map, static entities, camera, picking, selection, and one actor render correctly |
| M3: Movement slice | Motion geometry/path graph parsed; player reaches a target using canonical pointer input; replay and restore remain deterministic |
| M4: Tutorial slice | Required SCB instructions, objectives, stimuli, AI, interactions, win/lose, and input-only automation complete `EmbTut_FoC_EC` |
| M5: Representative campaign | At least one mission of each major archetype passes; campaign graph, internal saves, menus, audio state, and regression traces work |
| M6: Full campaign | Every reachable campaign mission has deterministic scripted coverage; campaign can be completed using player-level input |
| M7+: Compatibility/features | Original-save import/export, editor, Lua mods, high resolution, QoL, Android, co-op experiments |

Changes embedded in this ordering:

- Decode only formats required by the next vertical slice.
- Move sprite and SCB feasibility ahead of broad implementation.
- Implement read-only parsers first; round-trip writers are an editor milestone.
- For MAP/MIN source decoding, demand exact pixel equality, not SSIM 0.98.
- Use perceptual/masked comparisons only for complete rendered scenes.
- Move original save compatibility out of the full-campaign critical path.
- Make campaign automation act through canonical player input. Privileged observations are acceptable for planning; privileged world commands are not.
- Do not upload local golden images or proprietary failure artifacts from self-hosted runners.

The single biggest risk is the clean-room boundary. The current plan lets the same agents inspect Ghidra output and implement the engine, while calling the result clean-room. That is a process failure with potentially existential legal consequences. Establish analyst/implementer separation and provenance before the first behavior-derived engine code is committed.

The largest technical risk after that is SCB-plus-gameplay semantics, not BKS. A sprite decoder can be attacked with I/O traces and decoded-surface captures; completing a campaign requires hundreds of interacting script natives, AI transitions, timing rules, and mission-specific edge cases. The mitigation is the tutorial-first vertical slice, subsystem hashes, controlled oracle traces, and incremental mission-archetype coverage.

# Concrete disagreements with Claude’s plan

- An ignored private `re/` directory is not a clean room.
- M1 is too broad; “extract every file type” delays the first falsifiable gameplay result.
- The shared RHS/DIC prefix is probably a bank-generation ID, not format magic.
- BKS looks like a 12-bit symbol stream, not an offset table.
- MAP/MIN are easy, exact RGB16 decode targets and should be pulled forward.
- Direct `select_unit`/`order` RPC methods cannot be the canonical player interface.
- Stdio RPC is better than a fixed TCP port for autonomous tests.
- State hashing must be explicitly canonical and componentized from M0.
- Original tick-to-new tick parity must be measured, not assumed.
- A general ECS is unnecessary and hazardous to deterministic ordering.
- `opensherwood-assets` is missing and more important than early audio/editor crates.
- A shipping `opensherwood-oracle` crate is the wrong boundary.
- `softbuffer` is premature duplication.
- LuaJIT desktop plus Lua elsewhere creates avoidable behavioral divergence.
- Round-trip writers are not an early requirement for read-only game-data support.
- SSIM is the wrong criterion for a directly decoded background.
- Original-save compatibility should not gate campaign completion.
- A general campaign-playing bot is unnecessary; deterministic per-mission input scripts are sufficient.
- OpenSherwood is acceptable as a codename but weak as a public brand.