# Brief for the full independent review (2026-09-02, end of day 1)

This file is the context handed to Codex for a complete, maximum-effort review of the repository so the
maintainer can verify the state of the project independently. It is also a snapshot of where day 1 ended.

## What exists (all on `main`)

- Governance: `AGENTS.md`, `CLAUDE.md`, skills in `.agents/skills`, ADRs 0001-0007, legal policy, clean-room roles,
  two previous Codex reviews and their dispositions (`docs/decisions/reviews/`).
- Formats (`crates/opensherwood-formats`, specs in `docs/formats/`): image blobs, SRES archives, sprite bank
  (RHS profiles, DIC pages + frame table, BKS VQ/span decoding: all 404,855 frames), animation layout
  (16-direction action blocks), RHP maps (occluder masks, motion area, projection areas, bonds, zones), RHM missions
  (all chunks of all 39 files), SCB script container (opcode semantics unknown), fonts (SBFONT). Data-backed tests
  cover every retail file of each kind; synthetic tests cover malformed input.
- Assets (`opensherwood-assets`): game directory discovery, case-insensitive overlay VFS (base + language dirs),
  content fingerprint (partial digests of large files), sprite bank loader with frame cache.
- Core (`opensherwood-core`): deterministic world (fixed ticks, 24.8 saturating fixed point, PCG32 named streams,
  canonical BLAKE3 subsystem hashes, validated snapshots), canonical input events, camera, selection and orders,
  walkable geometry (point-in-polygon), navigation grid + A* + path following, sprite animation state, MissionSpec.
- Render (`opensherwood-render`): CPU RGBA compositor, backgrounds, sprites with FACE occluders, bitmap-font text.
- Audio (`opensherwood-audio`): Ogg Vorbis music / PCM effects via rodio.
- Protocol (`opensherwood-protocol`): JSON-RPC 2.0 types, ReplayV1 (validated), observation DTOs.
- App (`opensherwood-app`): headless stdio RPC server; winit + wgpu window (borderless fullscreen default,
  letterboxed, F11), controlled mode when an RPC client is attached, scenarios `corridor`, `map:<name>`,
  `mission:<name>` (all 39 retail missions load their actors on the right background), replay record/play, debug.nav.
- Tools (`opensherwood-tools`): inspect / export / overlay commands for every format.
- Harness (`harness/`): Python RPC client with timeouts, 25 tests (synthetic in CI on Linux/Windows/macOS,
  data-backed locally), cross-platform determinism fixture, `drive.py`, `play_window.py` (OS-level input).

## What does not exist yet (honest list)

- No script VM (SCB opcode semantics unknown), so no objectives, dialogues, cutscenes, triggers.
- No AI (patrol programs are decoded but not executed; guards only walk rail points), no view cones, alarm, combat,
  items, quick actions, crouch/run/climb rules, doors, layers.
- No HUD, menus, profiles, options, save slots; the main menu module exists (`app/src/ui.rs`) but is not wired.
- Profile-index -> sprite table unknown (NPCs use a default soldier sprite); walk timing uses a placeholder.
- The original's path graph (STAT rest) is undecoded; paths come from our grid.
- Original save compatibility, Bink video, Linux/macOS testing of the window, Android: not started.

## What the reviewer should produce

1. A ruthless, complete review of every crate and script for correctness, determinism, memory/time safety on
   hostile input, cross-platform hazards, API design, code quality and documentation accuracy.
2. A verification of every claim in `docs/formats/*.md` against the parsers and tests (are the statuses honest?).
3. A verification of the legal / clean-room policy against what was actually committed (`git log -p` is available):
   any game bytes, any decompiler-derived content, any licence problems in dependencies (`cargo tree`).
4. An assessment of the roadmap and architecture against the goal (complete, faithful, modernised port with
   modding), including the biggest risks and a recommended order of work for the next sessions.
5. A list of concrete, prioritised fixes with file:line references.
