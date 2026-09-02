# OpenSherwood

OpenSherwood is a free, open-source game engine that plays *Robin Hood: The Legend of Sherwood* (Spellbound
Entertainment, 2002) from the data files of a copy you already own. It is a clean-room reimplementation in the
tradition of OpenMW, Julius and OpenRCT2: the original game's assets are read, never shipped.

**Status: pre-alpha, milestone M2 in progress.** What works today (2026-09-02): the original main menu (pictures,
fonts and strings from the player's files), Play! loading the first mission behind its briefing pages, every
retail mission's map and actors, walking with pathfinding and foreground occlusion, animated sprites, music; a
deterministic core with snapshots, canonical hashes and replays; a headless JSON-RPC harness that plays the engine;
a borderless-fullscreen window (winit + wgpu). Not yet: HUD, AI behaviour, combat, scripts (objectives, dialogues),
options, profiles and saves, comparison against the original. See [docs/roadmap.md](docs/roadmap.md) and the
[status reports](docs/status/).

## Goals

1. Play the complete original campaign (30+ missions) with the original data, bit-for-bit faithful where it matters
   (simulation, AI, scripts), while running natively on Windows, Linux and macOS (Android later).
2. Modern comfort: any resolution, widescreen, hi-DPI, fast loading, rebindable keys, gamepad, save anywhere.
3. Modding: a mission and map editor, Lua scripting compatible with the community Spellforge API where practical,
   override directories for custom campaigns, maps, characters and translations.
4. Later: co-op and custom modes, and support for other games built on the same engine family
   (Desperados: Wanted Dead or Alive).

## You need the original game

OpenSherwood contains no game content. Buy the game on GOG or Steam and point OpenSherwood at the installation
directory (it looks for `Robin Hood.exe` and `DATA/robinhood.bks`). See [docs/legal.md](docs/legal.md).

## Building and running

See [docs/build.md](docs/build.md). Short version:

```
cargo build --release
cargo run --release -p opensherwood -- --game-dir "C:\GOG Games\Robin Hood - The Legend of Sherwood"
```

Tests that need game data read the directory from the `OPENSHERWOOD_GAME_DIR` environment variable and are skipped
when it is not set.

## Repository map

| Path | Content |
|---|---|
| `crates/` | the engine (Rust workspace); see [docs/architecture.md](docs/architecture.md) |
| `harness/` | Python test harness: drives the engine over JSON-RPC (comparison against the original is planned, see `docs/oracle.md`) |
| `docs/` | all documentation: legal policy, architecture, roadmap, file formats, notes on the original |
| `docs/decisions/` | architecture decision records |
| `.agents/skills/`, `.claude/skills/` | procedures for AI agents working on this repo (kept identical) |
| `AGENTS.md`, `CLAUDE.md` | instructions for AI coding agents; humans should read them too |
| `scripts/` | helper scripts (skill sync, game directory detection, CI helpers) |

## Development model

This project is developed largely by AI coding agents (Claude Code and OpenAI Codex) under human direction,
with a rule that nothing is merged unless the automated harness verifies it: build, run headless, play,
screenshot (comparison with the original is planned). Humans are welcome; read [CONTRIBUTING.md](CONTRIBUTING.md).

## License

GNU General Public License v3.0 or later. See [LICENSE](LICENSE).
Robin Hood: The Legend of Sherwood is a trademark of its respective owners; OpenSherwood is not affiliated with or
endorsed by Microids, Spellbound Entertainment or their successors.
