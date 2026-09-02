# Roadmap

Milestones have exit criteria that the harness can verify. A milestone is done when its checklist is green in CI
(synthetic tests) and on a machine with game data (data tests). Dates are not promised.

## M0: Governance and deterministic kernel (in progress)

- [x] Repository, GPLv3, legal policy, provenance rules, ADRs
- [x] Format specs for everything observed so far (`docs/formats/`)
- [x] `AGENTS.md`, `CLAUDE.md`, skills mirrored and CI-checked
- [x] Cargo workspace: formats, assets, core, script, render, protocol, app, tools
- [x] Headless app speaking JSON-RPC over stdio: `hello`, `reset`, `step`, `observe`, `snapshot`, `restore`, `capture`, `shutdown`
- [x] Synthetic world (no game data) that replays, snapshots, restores, hashes and renders identically on every platform
- [x] `ReplayV1` reader/writer (protocol crate); canonical state hash with subsystem hashes
- [x] Replay playback and recording through the app (`replay.start` / `replay.stop` / `replay.play`, checkpoints, first-divergence report)
- [x] Python harness: RPC client, synthetic pytest suite, snapshot/restore fuzz
- [ ] CI on Linux/Windows/macOS: fmt, clippy, nextest, pytest synthetic, skill sync check, no-assets check
- [x] `opensherwood-tools inspect` for SRES / image blobs / RHS / chunk containers (already specified)

- [x] Codex M0 review incorporated (versions, full canonical hashing, snapshot validation, protocol limits, controlled window mode, content fingerprint)
- [ ] Deferred from the review: cargo-fuzz targets for parsers, a 32-bit CI job, SRES decode budget, physical-key code set, rename `harness/tools/re` to `probe`

## M1: Feasibility gates

- [x] Exact decode of `.map` / `.min` / `.pak` / `.sxt` containers to 16-bit pixels (all 48+ files)
- [x] Channel order of the 16-bit pixels verified (RGB565; decoded minimap and slides look right)
- [x] SRES pictures decoded and viewable locally (`opensherwood-tools export-sres`)
- [x] Sprite bank: both encodings decoded, every one of the 404,855 frames consumed exactly, frames rendered and checked (`docs/formats/sprites.md`)
- [ ] SCB disassembler consumes the tutorial script with every unknown explicitly listed
- [ ] RHP: motion geometry / path graph chunk decoded well enough to draw it and compare with the original's `EULER` / `MOTION` overlays
- [ ] RHM: actor list (`BOYZ`) decoded for the tutorial
- [ ] First controlled oracle trace of the original (see `docs/oracle.md`)

## M2: Scene vertical slice

- [x] Interactive window (winit + wgpu presenter of the CPU framebuffer), letterboxed, driven by the same canonical input events
- [x] Map view scenario: retail background + camera (keys, edge scrolling) + picking in map coordinates; RPC works in window mode too

- [ ] VFS resolves base + language overlay (`2047/data`) + mod overlays; content fingerprint
- [ ] Tutorial map renders with static entities, camera, picking, selection, one animated actor
- [ ] Pixel comparison against a local screenshot of the original (masked, perceptual threshold)

## M3: Movement slice

- [ ] Path graph, layers, sectors, doors; pointer-driven movement with the original's rules (walk/run, crouch)
- [ ] Replay and restore remain deterministic through movement

## M4: Tutorial slice

- [ ] SCB instructions needed by `EmbTut_FoC_EC`, objectives, stimuli (sight cones, noise), AI patrols, alarm, combat basics, items, win/lose
- [ ] An input-only replay completes the tutorial

## M5: Representative campaign

- [ ] One mission of each archetype (ambush, town infiltration, castle, street, tactical, Sherwood camp) completable by replay
- [ ] Campaign graph, internal saves, menus, briefing/debriefing, music states
- [ ] Nightly regression matrix over recorded replays

## M6: Full campaign

- [ ] Every reachable mission has a deterministic replay; the campaign can be completed with player-level input only

## M7+: Compatibility and features

- [ ] Original save import/export, arbitrary resolution and widescreen, QoL, rebindable input, gamepad
- [ ] Mission/map editor, Lua mods (Spellforge-compatible API), custom campaigns
- [ ] Android, co-op experiments, Desperados: Wanted Dead or Alive data support
