# Brief: open-source reimplementation engine for "Robin Hood: The Legend of Sherwood" (2002)

You are Codex, acting as co-architect and adversarial reviewer. Claude (Claude Code) is the other agent and wrote this brief.
Both agents will work in the same public GitHub repo. The human owner wants: a legal, asset-free, clean-room, cross-platform
(Windows first; Linux/macOS/Android later) open engine in the spirit of OpenMW / OpenJK / Julius / OpenRCT2, that plays the
full original campaign from the player's own GOG/Steam copy, then adds modern features (map/mission editor, mods, custom
campaigns, QoL, hi-res, later co-op / custom modes). The engine must be developed *test-first with an autonomous
harness*: agents must be able to build, run headless, drive the game programmatically (everything a player can do),
screenshot, dump state, compare against the original game (oracle), and iterate without a human.

The game install (read-only for you) is at: `C:\GOG Games\Robin Hood - The Legend of Sherwood`. You may inspect it.

## Hard legal rules (non-negotiable)
- Zero game assets in the repo. Engine reads from the player's install (detect GOG/Steam path).
- Clean room: decompiler output never enters the repo. Only behaviour specs / format docs derived from analysis.
- GPLv3. Project name must not contain the trademark "Robin Hood – The Legend of Sherwood" / avoid "Sherwood" in the name.
- Rights holder: Microids (Média-Participations). Game is actively sold (Steam app 46560, GOG). C&D risk is real; keep it clean.

## Facts established so far (verified on disk today)

### Binaries
- `Robin Hood.exe` (2,899,968 B): real game. PE32 i386, **MSVC 6.0 linker (6.0)**, timestamp 2003-01-15. 4 sections, .text 0x275e39.
  Imports: DDRAW.dll, D3D8.DLL, DINPUT.dll, AVIFIL32, MSACM32, WINMM, binkw32.dll, fmod.dll (FMOD 3.x, UPX packed), ole32, VERSION.
  Source path strings inside: `P:\Robin Hood\RHScript.cpp`, `RHartificialmalignity.cpp`, `RHBow.cpp`, `RHCampaign.cpp`.
  ~80 class names visible via RTTI/asserts: RHGame, RHLevel, RHMission, RHCampaign, RHPathFinder, RHPath, RHFastFindGrid,
  RHElementActorPC/NPC/Soldier/Civilian/Human, RHElementArrow/Net/Wasp/WaspNest/Purse/Scroll/Bonus/Object/Projectile/Mobile/FX,
  RHSectorDoor/Lift/Building/Archery/Script/Production, RHGate, RHPatch, RHWayPoint, RHSeekPoint, RHStimulus, RHOrder,
  RHSequence*, RHSprite, RHSpriteScriptor, RHMinimap, RHFontManager, RHSoundCache/Source/Geometry, RHSaveGame(Manager),
  RHProfileManager, RHPlayerProfile, RHMenuCampaignMap, RHMenuDialogue, RHUIRenderer*, RHHikingGuide, RHGroundMark, RHKeyConfig.
  AI state strings like `SUBSTATE-DEFAULT-PATROL-ENROUTE-WAITING`, `CALL-PATROL-COORDINATE`, `EVENT-AFTER-SCRIPT-GO-ON`.
  Waypoint commands: Bend, LookLeft, LookRight, CheckFor, CheckForSync, PatrolStart, PatrolDirection, PatrolStop.
  Script API names (from error strings): IsActorCharacter/Object/Cart/PC/Animal/NPC/Soldier/Civilian, IsAnimationActive,
  SetAnimationState, UnBlip, IsUnblipped, GetDistance, ComputeLocationBetween, AreAllEnemiesInsideHS, GetActorLocation,
  GetMovementStyle, MakeNoise, properties: wasp nests, nets, plants, legs, ales, apples, stones. Script bytecode compiled from
  `script.scs` (text source, not shipped) into `.scb` ("SBSCRIPT", version float 1.5, function table + bytecode).
  Command line args: -NOSCRIPT -SIMULATE -GAMEPAD -SOUNDDEVICE -SETREG -CHECKSOUNDDATA -GENERATESKIPDATA -EXTRACTHUNK -GETMAJORVER -GETMINORVER.
  Built-in dev console (F11 in the community-patched exe) with ~60 commands: AI, EULER (pathfinder graph), MOTION (obstacles/doors),
  EINSTEIN (3D obstacles), ELEVATION, PROJECTION (3D projection areas), RAILROAD, CESTLAZONE (script zones), LIGHT (light zones),
  SEEKANDDESTROY (seek points), NOISE, SHADOW/SPHERE, STATUS PC/FRAMECACHE/SHADOW/HARDWARE, BIG BROTHER (actor infos), BABYLON, FPS,
  cheats (WIN, LOOSE, NUKE, HIGHLANDER, GOLDENEYE, FREEZE, ...), CAMPAIGN <file>, REPORT, LEVEL TEXT <DG|DB|PT|SB>.
- `Game.exe` (42 KB, MinGW GCC 13.2): community Ready2Play launcher that just launches `Robin Hood.exe`. `ddraw.dll` = cnc-ddraw
  (windowed, d3d9on12). `binkw32.dll` (15 KB MinGW) is a shim forwarding to `binkw32x.dll` (real Bink 2002).
- Tooling on this machine: Windows 11, Python 3.12 (numpy, opencv, pillow), Rust stable (x86_64-msvc; MSVC Build Tools being
  installed now via winget), clang 22 (LLVM), CMake, Ninja, gh CLI (user `veosar`), Docker Desktop, WSL2, Node. No Ghidra yet
  (can install via winget + Temurin JDK). No C++ compiler until Build Tools finish.

### Data layout (`DATA/`, 1 GB)
- `robinhood.bks` (565 MB) + `robinhood.dic` (9.3 MB): sprite bank + dictionary. `.dic` and every `.rhs` start with magic `C9 EB 03 00`.
  `.bks` starts with a table of u16 values (lots of `6d06`), i.e. probably an index/palette-ish or offsets table.
- `Characters/*.rhs` (117 files, 15 MB) and `Animations/{Day,Fog,Night}/*.rhs` (116): sprite/animation descriptors
  (magic + u16 count + 32-byte name in French e.g. "ACCESSOIRES Ale", then tables). Actual pixels presumably in `.bks`.
- `Levels/`: 9 `.rhp` map files (`MEUH` container: chunk = 4-char tag + u32 size; sub-chunks SPOK, STAT, ZO..; e.g. nottingham.rhp 1.6 MB),
  39 `.rhm` missions (`DUTY` container; sub-chunks FOOT, POUF, ...; contain actor names + ASCII tags like "Trapcr01"),
  39 `.scb` compiled scripts, `Levels/{Day,Night,Fog,Custom1..4}/*.map` (prerendered backgrounds, 1.5–9 MB each) + `*.min` (minimaps, 30–50 KB).
  Error string: "version %d does not match %d in chunk %s" confirms chunk versioning.
- `Interface/DEFAULT.RES` (6.6 MB): `SRES` archive, version 0x100, 292 entries; entry = tag ("PICC"/"TEXT"/"WAVE") + id + fields + zlib data (78 DA).
  `Text/Level.res` (UTF-16 strings, TEXT entries), `Text/actors.res` (WAVE entries with paths), `Text/RHLevel??.red` (64 B, id tables).
- `Interface/Loading.pak`, `Start.sxt`: header `00 04 00 03 02 00 00 00 <u32 size>` then **bzip2** stream (`BZh9`).
- `Interface/Fonts/*.bfn` (bitmap fonts), `*.tfn` (`SBTTFT`, 90 B TrueType font descriptor, e.g. SimSun), `dialog.fnt`, `simsun.ttc`, `manager.cfg` (text mapping).
- `Sounds/`: 539 wav, `Exclamations/actor*.dat` (`NEUF` tables), `*.sfk` (`SFPK`), `Robin Hood.fxg` / `menu.fxg` (`FXBK` name→id tables), `Musics/*.wav`.
- `Configuration/profile.cpf` (30 KB binary profile), `keyset1/2.cfg` (76 B keymaps), `release.log` (bzip2).
- `Savegame/Profiles` (`FORP`), `Savegame/Profile_001/{Continue,Restart}` (`GSHR` chunked, 175–187 KB) + `_t` thumbnails.
- `../2047/data/` contains the English overlay: `Cinematics/Intro.vid`, `Outro.vid` (**Bink**, "BIKi"), `Text/Level.res`, `Interface/Start.sxt`,
  `Slideshow_in.pak`, `Sounds/Exclamations/Expressions`, `Text/Dialogues/*.wav`. (2047 likely = language id; engine probably resolves `Data\...` via the language folder.)
- `Campaign.bck`, `Manual.pdf` (game manual, useful mechanics reference).

### Community resources
- rhmods.com: Spellforge Editor (dinput.dll hook, LuaJIT 2.1 mission scripting, `api.lua`, Blender .rhp importer), Developer Console (patched exe + PDF),
  Rhuce (campaign editor), Asset Editor (.RES, also works for Desperados 1 .RES), Profile Tool, Ready2Play launcher. Discord discord.gg/SWPCCgvmFP.
  Downloads are hosted on ModDB (403 for bots). None of these tools are open source as far as we know.
- github.com/phiresky/RobinHood-TheLegendOfSherwood-Resolution-Patcher (save-file resolution floats at 0x104 + n*0xd5e; Linux flags -NOINPUTGRAB, -NOFULLSCREEN).
- Desperados: Wanted Dead or Alive (2001, same studio) shares the engine core (same .RES tooling). A multi-game engine is a stretch goal.

## Claude's proposed plan (please attack it)

### Language / stack
**Rust (edition 2024), cargo workspace.** Rationale: agents are the primary developers; cargo gives one build+test+deps system on
Win/Linux/macOS/Android/wasm; memory safety cuts a whole class of debugging; egui for editor tooling; wgpu for presentation.
The human's research draft suggested C++20 + SDL3. Counter-arguments welcome, but be concrete about agent iteration speed and cross-platform.
Proposed crates: `winit` + `wgpu` (present a software-rendered RGBA framebuffer as a texture; enables shaders/upscaling later; runs on
Android and wasm), `softbuffer` fallback, `cpal`/`kira` audio, `mlua` (LuaJIT on desktop, Lua 5.1 fallback) for the mod scripting layer,
`bzip2`/`flate2` for pak/res, `image`/`png`, `serde_json` + JSON-RPC for the harness, `egui` for tools/editor, `tracing`.
Bink video: port a decoder (FFmpeg's bink.c is LGPL → compatible) or use NihAV's Rust Bink decoder; low priority (intro/outro only).

### Workspace layout
```
locksley/                     (working name; neutral, "the locksley" of the public-domain ballads)
  AGENTS.md                    canonical agent instructions (Codex reads it)
  CLAUDE.md                    "@AGENTS.md" import + Claude-specific notes
  .agents/skills/<name>/SKILL.md   canonical skills (Codex reads .agents/skills)
  .claude/skills/<name>/SKILL.md   mirror kept in sync by scripts/sync_skills.py (CI checks equality)
  docs/                        formats/*.md (specs), architecture.md, roadmap.md, legal.md, decisions/ADR-*.md, oracle.md
  crates/locksley-formats            parsers: sres, pak(bz2), rhs/dic/bks sprites, rhp, rhm, scb, map/min, bfn/tfn, sfk/fxg/neuf, cpf, savegame
  crates/locksley-sim                deterministic simulation (fixed tick, own RNG, entities, pathfinding, AI, stimulus, orders) — no I/O
  crates/locksley-script             SCB bytecode VM + Lua API (Spellforge-compatible names where sensible)
  crates/locksley-render             software compositor → RGBA framebuffer (backgrounds, sprites, shadows, light zones, fog)
  crates/locksley-audio              mixer, FMOD-ish behaviour (positional, music states D/NF/Alarm/Fight)
  crates/locksley-app                the game binary: window, input, dev console, JSON-RPC server (--headless, --rpc PORT, --script file)
  crates/locksley-cli                asset inspector/extractor (dump chunks, export sprites to PNG for local inspection only), replay tools
  crates/locksley-editor             later: egui map/mission editor
  harness/                     Python: rpc client, pytest suites (smoke, replay, golden), image diff (SSIM), oracle (frida scripts)
  scripts/                     sync_skills.py, find_game_dir.py, ci helpers
  .github/workflows/ci.yml     build+test matrix; tests needing game data are skipped in CI unless GW_GAME_DIR is provided
```

### Test/verification loop (the core requirement)
1. `locksley-app --headless --rpc 4711 --game-dir <path>`; JSON-RPC methods: load_mission, tick(n), input events (mouse/keys at engine-level),
   select_unit, order (move/attack/queue action), dump_state (JSON), screenshot(png path), checksum_state, save/load, console(cmd).
2. Determinism: fixed timestep, seeded RNG, replay files (.gwr) with input log; test = same replay → same state hash; save/load mid-replay
   must not change outcome.
3. Golden images generated locally from the player's copy (never committed); CI stores only hashes/SSIM thresholds + uses synthetic fixtures.
4. Oracle against the original: Frida scripts attach to `Robin Hood.exe`, hook functions found via Ghidra (e.g. RHElementActor position
   update, tick, RNG), dump JSON per tick; compare tick-by-tick with our sim. Also use the original's console (F11) for visual references
   (EULER graph, MOTION obstacles, CESTLAZONE zones) captured as screenshots for pixel/structure comparison of our parsers.
5. Smoke gates in CI: boots headless 100 ticks; loads mission 1 (with game data on a self-hosted runner or locally); unit moves; mission completable by scripted bot.

### Roadmap (milestones with exit criteria)
- M0 (now): repo, legal docs, AGENTS/CLAUDE, skills, workspace skeleton, locksley-app stub with RPC + headless + screenshot, harness test_boots green in CI.
- M1: formats. SRES, PAK/SXT, RED, fonts, RHP/RHM chunk maps, SCB disassembler, MAP/MIN images, sprite bank (RHS/DIC/BKS: the hard one, custom compression).
  Exit: `locksley-cli` extracts every file type; round-trip tests; docs/formats/*.md written.
- M2: render one map with static sprites at correct positions/depth; pixel-diff vs original screenshot (SSIM ≥ 0.98 target for background layer).
- M3: selection, camera, movement, pathfinding on the RHP motion graph, view cones. Exit: walk Robin across Sherwood in headless replay.
- M4: AI (patrols, stimuli, alarm), combat, items, SCB VM executing mission 1 scripts, win/lose. Exit: bot completes mission 1 (tutorial).
- M5: campaign flow, menus, saves compatible with original, all 30+ missions completable by bot. Exit: full campaign regression matrix nightly.
- M6+: editor, mod loader (Lua, override dirs), hi-res/widescreen, QoL, co-op experiments, Desperados support.

## Questions for you (answer all, be adversarial and concrete)
1. Rust vs C++20 for this project given agent-driven development and the cross-platform targets. Decide and justify.
2. Presentation stack: winit+wgpu (software compositor uploaded as texture) vs SDL3. Consider Android/wasm, and input fidelity for a click-heavy RTT.
3. Is the workspace split right? What would you merge/split? What is missing (e.g. a `locksley-oracle` crate, a tick-hash design, an ECS or not)?
4. Harness design: what JSON-RPC surface is *minimal but sufficient* for an agent to play the game? How should replays, state hashing and
   savestate fuzzing be structured so they work from M0 and don't need rewrites at M4?
5. Oracle strategy: Frida hooks on MSVC6 thiscall methods vs memory-scanning known structs vs using the game's own console output.
   What is the cheapest path to a trustworthy tick-by-tick comparison? Propose a concrete first oracle experiment.
6. Reverse engineering workflow that stays clean-room: what goes where (private `re/` dir ignored by git vs committed specs),
   how do we document provenance so a contributor can trust the specs?
7. Sprite bank (`.bks` 565 MB + `.dic` + `.rhs`): propose an analysis approach for an unknown custom sprite compression (the 2002
   press notes say "a new system of compression for sprites" was introduced vs Desperados). What would you try first?
8. AGENTS.md / CLAUDE.md / skills format that both agents read. Codex reads `AGENTS.md` and `.agents/skills`; Claude reads `CLAUDE.md`
   (supports `@file` imports) and `.claude/skills`. Propose the concrete convention and the CI check.
9. Project name: "Locksley" is proposed. Any trademark or collision concern? Alternatives?
10. Anything in the roadmap ordering you would change? What is the single biggest risk you see, and the mitigation?

Output: a structured markdown review with numbered answers, a list of concrete disagreements with Claude's plan, and your proposed
final plan changes. Keep it decisive. You may read files under the game directory to verify claims (read-only).
