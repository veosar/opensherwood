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
- [ ] Hardening batch from the full review (P1 10-17: shared sprite decode limits, SRES cumulative budgets, bounded RPC drain, replay quotas and file-size checks, geometry coordinate bounds, full content digest, transactional restore, snapshot schema/content/catalog validation), then P2-P4 (`docs/decisions/reviews/2026-09-02-codex-full-review-disposition.md`)
- [ ] Deferred from the reviews: cargo-fuzz targets for parsers, a 32-bit CI job, SRES decode budget, physical-key code set, rename `harness/tools/re` to `probe`, oversized-line drain, transactional session restore, full-content fingerprint with cache, replay header enforcement, window focus/resize input hygiene, sRGB surface selection, streamed looped music, per-tick digest coverage, timed Python writes, SHA-pinned CI actions (see `docs/decisions/reviews/2026-09-02-codex-review-2-disposition.md`)

## M1: Feasibility gates

- [x] Exact decode of `.map` / `.min` / `.pak` / `.sxt` containers to 16-bit pixels (all 48+ files)
- [x] Channel order of the 16-bit pixels verified (RGB565; decoded minimap and slides look right)
- [x] SRES pictures decoded and viewable locally (`opensherwood-tools export-sres`)
- [x] Sprite bank: both encodings decoded, every one of the 404,855 frames consumed exactly, frames rendered and checked (`docs/formats/sprites.md`)
- [x] SCB container decoded (classes, variables, functions, 9-byte instructions) with a raw disassembler; opcode semantics still unknown
- [x] RHP: occluder masks, motion boundary + obstacles, projection areas, bonds, zones decoded and overlaid on the backgrounds (path graph still raw)
- [x] RHM: all chunks decoded for all 39 missions (actors, rails/patrol programs, waypoints, beam points, zones, scrolls, carts)
- [x] Fonts decoded (SBFONT glyph strips) and text rendering in the engine
- [x] `mission:<name>` scenario: every retail mission loads its actors at their positions on the right background
- [ ] First controlled oracle trace of the original (see `docs/oracle.md`)

## M2: Scene vertical slice

- [x] Interactive window (winit + wgpu presenter of the CPU framebuffer), letterboxed, driven by the same canonical input events
- [x] Map view scenario: retail background + camera (keys, edge scrolling) + picking in map coordinates; RPC works in window mode too
- [x] Characters drawn from the sprite bank with the documented idle/walk animation blocks and canvas origins
- [x] Audio: Ogg Vorbis music per map / menu theme, PCM effects channel (`opensherwood-audio`)
- [x] Borderless fullscreen by default, F11 toggle, letterboxed logical viewport
- [x] Main menu from the player's files (background, plate buttons, fonts, strings; geometry from `docs/original/ui-flow.md`), Play! loads the first mission (`H01_Lin_VL`, Lincoln) behind its briefing pages, camera on the hero; menus and briefings are driven by canonical input and observable over RPC (`ui`)
- [x] Pause menu (Escape: continue, restart, quit with confirmation), quit confirmation in the main menu, HUD frame (foliage, portrait scroll, money / clover) from the player's files
- [ ] HUD interactions: action icons, minimap, crouch, counters; briefing character picture; verified pause tint
- [x] Credits (background, scrolling strip at the observed 20 px/s, Escape returns)
- [ ] Options, profiles (select/new/rename/delete), load/save screens, movies

- [ ] VFS resolves base + language overlay (`2047/data`) + mod overlays; content fingerprint
- [ ] Tutorial map renders with static entities, camera, picking, selection, one animated actor
- [ ] Pixel comparison against a local screenshot of the original (masked, perceptual threshold)

## M3: Movement slice

- [x] Walkable geometry from RHP (boundary + obstacle polygons) blocks movement; occluder masks (FACE) draw the background in front of sprites behind trees, rocks and walls
- [x] Pathfinding around obstacles: 8-px navigation grid rasterised from the walkable geometry (eroded by one cell), A* with deterministic tie-breaking, string-pulled paths, orders on unreachable ground walk to the closest reachable cell (the original's EULER graph is still undecoded)
- [x] Projection areas (`WOAW`) count as walkable ground, so the town maps (Lincoln yard of mission 1) can be walked; `debug.nav` reports them
- [ ] Path graph, layers, sectors, doors; pointer-driven movement with the original's rules (walk/run, crouch)
- [ ] Replay and restore remain deterministic through movement

## M4: Tutorial slice

- [ ] SCB instructions needed by the first mission (`H01_Lin_VL`, which is the tutorial in the retail flow; `EmbTut_FoC_EC` is not reached from Play!), objectives, stimuli (sight cones, noise), AI patrols, alarm, combat basics, items, win/lose
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
