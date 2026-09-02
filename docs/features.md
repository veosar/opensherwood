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
