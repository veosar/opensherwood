# Notes on the original executable (behavioural knowledge, clean room)

This file collects what the *strings and imports* of `Robin Hood.exe` tell us, rewritten in our own words.
No disassembly output is recorded here, and no message text, class name or path from the executable is
quoted verbatim (see `docs/legal.md`; the 2026-09-02 text sweep after review 4 removed the earlier catalogs).
Detailed static-analysis notes, if ever needed, go into `re/` (git-ignored) and only their conclusions are
written up here in prose.

## Build

- PE32, i386, linker version 6.0 (Microsoft Visual C++ 6.0), link date 2003-01-15, 2.9 MB.
- Imports: `DDRAW.dll` (DirectDraw 7, 2D blitting), `D3D8.DLL` (Direct3D 8; probably only for the 3D-projected
  elements / shadows or device enumeration), `DINPUT.dll`, `WINMM`, `MSACM32`, `AVIFIL32` (AVI, unused by retail
  data?), `binkw32.dll` (Bink video), `fmod.dll` (FMOD 3.x audio), `VERSION`, `ole32`, `ADVAPI32`.
- The embedded source paths point at a single-drive source tree with one `.cpp` per subsystem (script, AI,
  bow, campaign, ...). Class names carry a two-letter game prefix; the script VM and serialisation layer carry
  a different two-letter prefix and are shared with other games of the same studio (see `docs/formats/scb.md`).

## Subsystems (from class names and messages, paraphrased)

| Area | What the strings show |
|---|---|
| Game & level | classes for the game, the level, the mission and its statistics, the campaign, the player profile and its manager, save games and their manager, key / graphics / sound configuration |
| Map structure | sector classes for doors, lifts, buildings, archery ranges, script zones and production sites; gates; patches (map alterations); ground marks; a "hiking guide" (navigation helper); the terms *proto-level* (= `.rhp`) and *level description* (= `.rhm`) |
| Movement | a path finder, paths, a fast-find grid, waypoints, seek points, a position interface; the terms *motion area*, *layer*, *sector*, *bond* (edge between projection areas), *projection area* (3D-projected walkable polygon), *jump zone* / *jump line*, *railroad* (patrol path), *static repulsive point*, *obstacle mask* |
| Actors | one element class per kind: human (with PC / NPC / soldier / civilian specialisations), mobile, object, bonus, arrow, projectile, net, wasp, wasp nest, purse, scroll, target, FX (plain and masked); status classes for humans and PCs; orders, stimuli, bow and sword; the terms *posture*, *action state*, *company* (NPC group) |
| AI | one AI class; a set of named sub-states for the default behaviour (patrol en route, waiting, running, patrol chief returning to the patrol, script driven) and events (patrol coordination call, resume after script); eight waypoint commands (a bend, a glance to each side, two check-for variants of which one is synchronised, patrol start / direction / stop) with rules about which actor kinds may use them (civilians cannot patrol; friendly soldiers cannot use the check-for command) |
| Scripting | a script class, a script-sector class, sequences with a manager and three element kinds (movement, generic, damage), a messenger, a serialiser that reads `.scb`; error messages reference the zone-enter / zone-exit callbacks, script classes, message sending and scrolls that execute scripts |
| Rendering | sprite and sprite-script classes, a minimap, menu graphics, UI renderers, a font manager, tooltips, a portrait widget; the terms *sprite cache*, *frame cache*, *shadow polygon*, *light zones*; a warning about unknown screen resolutions (fixed set: 640x480, 800x600, 1024x768) |
| Sound | sound, sound cache, sound source and source manager, sound geometry; an FMOD version check; remark categories per actor kind (soldier / civilian / VIP) |
| UI / menus | campaign map menu, dialogue menu, short briefings, a reconnaissance report, mission statistics |

## Behavioural facts worth reproducing

- Mission loading pairs a proto-level (`.rhp`) with a mission (`.rhm`) and warns when they do not match; unknown
  chunks are skipped with a warning, so the loader is tag-driven and tolerant. The version-check messages name
  each chunk with a plain English word (header, tenant, actor, bonus, tactic, path, scroll, mobile, script, jump
  for the mission; building, lift, material, sound, patch, bond, miscellaneous, sight, animation, motion, mask,
  light for the proto-level); the mapping to chunk tags is inferred in `docs/formats/rhm.md` / `rhp.md`.
- Map background and minimap are looked up by map name, directory and "ambiance" (Day / Night / Fog / Custom).
- Every level needs a startup script (a fatal error otherwise); scripts reference actors and waypoints by id and
  the loader validates them. The `.scb` version check compares a floating-point version number.
- Campaign: missions have a location and a priority; some missions are obligatory; the accessible mission list is
  computed from campaign state; the gang (Robin's men, peasants) and ransom / ARES counters are campaign values;
  `Campaign.bck` is written as backup.
- Characters must lie on a motion area, on the right layer and sector, not inside an obstacle (loader asserts).
- Buildings have occupants (humans only); doors are of a default kind or gates unless they belong to buildings
  or lifts.
- Actors cannot be in a patch sector twice; script sectors are polygons with at least 3 points.
- Soldiers have companies, patrol chiefs, patrol paths with synchronisation between paths.
- Music states: ambient / tactical / alarm / fight, per location (see `docs/formats/sound.md`).
- Native script functions have names in the executable (visible only in error strings, without their numeric
  ids); they are not reproduced. The native call table in `docs/formats/scb.md` is built from the data files.

## Provenance

Observation of printable strings and the PE import table of the retail executable, paraphrased.
