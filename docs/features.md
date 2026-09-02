# Feature research: what the original has, what open engines add, what OpenSherwood will add

This is the backlog of modern features and quality-of-life improvements, ordered by milestone. It is informed by
what comparable projects shipped (OpenMW, Julius/Augustus, OpenRCT2/OpenLoco, DevilutionX, OpenXcom, CorsixTH,
fheroes2, VCMI) and by what the original executable already contains (developer console, hidden flags).

## Already in the original (to expose, not invent)

- Developer console with 60+ commands including debug overlays (`docs/original/console-commands.md`): becomes
  OpenSherwood's built-in console and debug overlay toggles, available to modders from day one.
- Three fixed resolutions (640x480, 800x600, 1024x768); the profile stores a float resolution pair that the
  community patched to 1920x1080, so the renderer already scales the play area: OpenSherwood renders at any resolution
  natively with correct UI layout.
- Two key sets (`keyset1/2.cfg`), difficulty and unit statistics in the profile (`profile.cpf`).
- Day / Night / Fog ambiances per map with separate prerendered backgrounds and animation sets.
- Cheats (invulnerability, win mission, reveal) that become testing and accessibility toggles.
- Command-line switches (`-NOSCRIPT`, `-SIMULATE`, `-GAMEPAD`, `-SOUNDDEVICE`) hinting at a scriptless test mode,
  a simulation-only mode and gamepad support that were never surfaced.

## Engine behaviour implemented so far (reference for what the harness can exercise)

- **NPC waypoint programs.** Every mission NPC with a rail (`BORG.rail`) executes a deterministic program
  translated from the rail's per-waypoint command programs (`docs/formats/rhm.md`, "Rail programs"): it walks the
  rail back and forth with the engine's pathfinding, and at each point runs the table for its travel direction as
  a percentage choice over the point's blocks: face a 16-way direction, wait (operand read as hundredths of a
  second), glance left / right, jump to another point of the rail (a `02(0)` on the last point makes the walk a
  loop) or stop for good. Commands whose meaning is not established are no-ops; the loader logs one line per
  mission with the translated / unknown counts. NPCs without a rail (and all civilians) stand idle in their
  placement direction. Programs and program counters are part of the snapshot and of the `actors` hash; the
  probabilistic choices draw from the gameplay RNG stream, so runs are reproducible from the seed. Not yet:
  the rails' check-for scans (0x0d), synchronisation between patrols (`CheckForSync`), scripted rails,
  carts.
- **Movement modes.** The manual's mouse rules (`docs/original/ui-flow.md` 9.4): left click selects a character
  or walks the selected one to the ground point, a double click (second press within 20 ticks and 8 px) runs
  there, a right click on the selected character cancels his order and a right click elsewhere deselects; `c`
  / `s` crouch / stand. Running uses the run block (action 7) at twice the walking speed, crouching the
  crouched idle and sneak blocks (14 / 16) at half speed; both speeds are hypotheses (the table's per-frame
  advance is a distance per frame). Gait, posture and the double-click memory are in the snapshot and hash.
  Not yet: distance-timed walk cycles, the run / sprint distinction (ids 7 / 10), crouch transitions
  (13 / 18), multi-selection.
- **Stealth layer** (`docs/original/stealth-and-combat.md` "Engine", `crates/opensherwood-core/src/ai.rs`).
  Every enemy soldier that is alive, active, unlocked and on his feet perceives the player characters: a view
  cone (half angle 45 degrees, range 200 px, halved for a crouched character; occluders ignored) and a noise
  radius (a running character within 150 px is heard whatever the soldier faces). A stimulus takes him from
  his patrol through *noticed* (action 141) and *alarm* (142) to *alerted*: he runs (151) to the last seen
  position with the weapon ready (140 / 143 idle / walk), keeps searching while he sees the character, and
  5 s after the last sighting walks back to where the alert took him from and resumes his program. Every
  action-id change of an actor with a script class fires `ActionChange(previous, new)` (the first mission's
  archery training ends when an archer notices something). A left click on an enemy with a character
  selected is an attack order: the character walks into reach (32 px) and, if his profile has the knock-out
  blow (123: Robin) and he stands behind the victim (within 67.5 degrees of straight behind), the victim goes
  down (41), lies knocked out (47) for 10 s scaled by his knock-out resistance (`profile.md` `p4`; 100 =
  immune), gets up (49) and returns to his post; from the front the character stops and faces him. Hit points
  come from the profile (`p0`), without a damage model yet. Script natives 85 / 87 / 90 / 128 / 240 read these
  states, 140 sets the gait of an NPC's program walks. Every constant is a hypothesis pinned by tests
  (`ai.rs`, `harness/tests/data/test_mission.py`). Not yet: sight blocked by walls, civilians raising the
  alarm, walking noise, the rails' check-for radius, soldiers fighting or shooting, comrades reviving a body,
  `FilterAIEvent` stimuli, the fist / action icons, the stars over a knocked-out head, damage and death.
- **Mission scripts.** Every retail mission's compiled script (`.scb`) is translated to the core VM and runs
  (ADR-0008, `docs/formats/scb.md` "Engine"): `Initialize` / `PostInitialize` at load, `Hourglass` and
  `CheckVictoryCondition` every tick, messages between classes, sequences (text pages that wait for the player,
  timed waits, camera moves, walk orders), `EnterZone` / `ExitZone` on the script polygons, objectives, mission
  variables, element activation (hidden actors appear when the script says so), patrol assignment and AI
  locks, attributes, states, patches, a `script` RNG stream. The first mission's briefing pages and its initial
  objective come from the script. Natives without an implementation are visible: stubs are counted, unknown
  ones trap (or are logged in lenient mode). `IsTaken` fires from scroll pickups and `ActionChange` from the
  actors' action-id changes. Not yet: combat and item natives, `ActivatedBy*` / `ReachPoint` triggers from
  the world, doors, animations, the text presentation of in-mission popups.

## Target for "1.0" (the maintainer's definition of done)

The complete retail game runs unchanged: every mission and cutscene, all menus, settings, profiles and saves,
with the original look and behaviour, plus: graphics through a modern cross-platform renderer (wgpu: DX12 /
Vulkan / Metal), sound through a modern mixer, **borderless fullscreen at the desktop resolution by default**,
Windows / Linux / macOS builds, and QoL that does not touch the original assets. Because the original menus are
data we only read, extra options live in two places that are ours: a **launcher / options window** (resolution
mode, scaling, audio device, QoL toggles, mods, replays) and an **in-game overlay** opened with a hotkey, drawn
with the engine's own UI drawn from the player's assets at runtime (the retail button and font resources may be
reused for a consistent look because they are read from the player's copy, never shipped).

## Platform and QoL (M2-M7)

| Feature | Notes |
|---|---|
| Any resolution, widescreen, hi-DPI, borderless, windowed | reference mode stays pixel-exact; UI re-laid out for wide screens |
| Integer / smooth scaling, optional upscaling shaders (xBR, CRT), optional AI-upscaled backgrounds as a mod | presentation modes separate from the reference compositor |
| Fast loading: decoded-asset cache, memory-mapped sprite bank, parallel decode | the original's "frame cache" replaced by a real cache |
| Autosave (mission start, every N minutes, before risky actions), rolling save slots, quick save/load with history ("rewind"), save anywhere | internal snapshot system makes rewind cheap; original save import/export in M7 |
| Rebindable keys, mouse buttons, wheel zoom, edge/drag scrolling options, gamepad, Steam Deck friendly layout | |
| Game speed control, pause with orders, planning mode (queue simultaneous actions like "Showdown Mode") | new mode, off by default, replay-recorded |
| Accessibility: colour-blind view-cone colours, larger fonts, subtitles for dialogues, UI scale | |
| Localisation: all shipped languages, community translation packs as mods, UTF-8 text overrides | |
| Audio: positional mixing, music crossfade, volume per category, modern codecs for mods | |
| Deterministic replays, replay sharing, speedrun timer, TAS-friendly frame stepping | |
| Built-in benchmark and profiler overlay | |
| Mod manager: load order, overlay directories, per-mod settings | |

## Modding (M7+)

| Feature | Notes |
|---|---|
| Override directories: any file in `mods/<name>/DATA/...` replaces the original | the VFS overlay is designed for this in M2 |
| Lua mission scripting with a Spellforge-compatible API surface | community missions portable |
| Map/mission editor: place actors, waypoints, patrols, zones, triggers, scripts on any map; import RHP geometry; export new maps | editor reuses the engine and renderer |
| New campaigns: campaign graph editor, briefing texts, custom music | |
| Characters and skins: sprite import from PNG sheets, palette swaps (the original has `CHROMA`) | needs a sprite-bank writer (M1 decoder first) |
| New backgrounds: import a rendered 16-bit image + geometry | |
| Scripting hooks for new abilities, items, AI behaviours | |
| Steam Workshop / mod portal packaging format | |

## New modes (research, after the campaign is complete)

Co-op (lockstep on the deterministic core), custom challenge modes (no-detection runs, time attack, permadeath),
randomised missions, an "ironman" campaign, and support for Desperados: Wanted Dead or Alive data.

## Performance targets

Load a mission in under two seconds from a warm cache; 60 Hz presentation at 4K with the reference compositor
on integrated graphics; memory footprint under 1 GB with the whole sprite bank resident on demand.
