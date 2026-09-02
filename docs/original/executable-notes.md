# Notes on the original executable (behavioural knowledge, clean room)

This file collects what the *strings and imports* of `Robin Hood.exe` tell us. No disassembly output is recorded
here; see `docs/legal.md`. Detailed static-analysis notes, if ever needed, go into `re/` (git-ignored) and only
their conclusions are written up here in prose.

## Build

- PE32, i386, linker version 6.0 (Microsoft Visual C++ 6.0), link date 2003-01-15, 2.9 MB.
- Imports: `DDRAW.dll` (DirectDraw 7, 2D blitting), `D3D8.DLL` (Direct3D 8; probably only for the 3D-projected
  elements / shadows or device enumeration), `DINPUT.dll`, `WINMM`, `MSACM32`, `AVIFIL32` (AVI, unused by retail
  data?), `binkw32.dll` (Bink video), `fmod.dll` (FMOD 3.x audio), `VERSION`, `ole32`, `ADVAPI32`.
- Source tree was `P:\Robin Hood\` with files such as `RHScript.cpp`, `RHartificialmalignity.cpp` (AI),
  `RHBow.cpp`, `RHCampaign.cpp`. Classes carry the `RH` prefix.

## Subsystems (from class names and messages)

| Area | Classes / strings |
|---|---|
| Game & level | `RHGame`, `RHLevel`, `RHMission`, `RHMissionStat`, `RHCampaign`, `RHPlayerProfile`, `RHProfileManager`, `RHSaveGame`, `RHSaveGameManager`, `RHKeyConfig`, `RHGraphicConfig`, `RHSoundConfig` |
| Map structure | `RHSectorDoor`, `RHSectorLift`, `RHSectorBuilding`, `RHSectorArchery`, `RHSectorScript`, `RHSectorProduction`, `RHGate`, `RHPatch`, `RHGroundMark`, `RHHikingGuide`, "proto-level" (= `.rhp`), "RHD" (= level description = `.rhm`) |
| Movement | `RHPathFinder`, `RHPath`, `RHFastFindGrid`, `RHWayPoint`, `RHSeekPoint`, `RHPositionInterface`, "motion area", "layer", "sector", "bond" (edge between projection areas), "projection area" (3D-projected walkable polygon), "jump-zone" / "jump-line", "railroad", "static repulsive point", "obstacle-mask" |
| Actors | `RHElementActorHuman/PC/NPC/Soldier/Civilian`, `RHElementMobile`, `RHElementObject`, `RHElementBonus`, `RHElementArrow`, `RHElementProjectile`, `RHElementNet`, `RHElementWasp`, `RHElementWaspNest`, `RHElementPurse`, `RHElementScroll`, `RHElementTarget`, `RHElementFX`, `RHElementFXMasked`, `RHHumanStatus`, `RHPCStatus`, `RHOrder`, `RHStimulus`, `RHBow`, `RHSword`, "posture", "action state", "company" (NPC group) |
| AI | `RHArtificialMalignity`; state names `SUBSTATE-DEFAULT-PATROL-ENROUTE`, `...-ENROUTE-WAITING`, `...-ENROUTE-RUNNING`, `...-PATROL-CHIEF-RETURN-TO-PATROL`, `SUBSTATE-DEFAULT-SCRIPT-DRIVEN`, `CALL-PATROL-COORDINATE`, `EVENT-AFTER-SCRIPT-GO-ON`; waypoint commands `Bend`, `LookLeft`, `LookRight`, `CheckFor`, `CheckForSync`, `PatrolStart`, `PatrolDirection`, `PatrolStop` with rules about which actor kinds may use them (civilians cannot patrol; friendly soldiers cannot `CheckFor`) |
| Scripting | `RHScript`, `RHSectorScript`, `RHSequence`, `RHSequenceManager`, `RHSequenceElement{Movement,Generic,Damage}`, `RHMessenger`, `SCSerialize` (reads `.scb`); errors reference `EnterZone` / `ExitZone`, "script class", "SendMessage", "scroll executing script" |
| Rendering | `RHSprite`, `RHSpriteScriptor`, `RHMinimap`, `RHMenuGraphics`, `RHUIRenderer*`, `RHFontManager`, `RHTitbits`, `RHWidgetPortrait`, "sprite cache", "frame cache", "shadow polygon", "light zones", "Warning, unknown screen resolution" (fixed set: 640x480, 800x600, 1024x768) |
| Sound | `RHSound`, `RHSoundCache`, `RHSoundSource`, `RHSoundSourceManager`, `RHSoundGeometry`; "Need FMOD %.02f"; remark categories per actor kind (soldier / civilian / VIP) |
| UI / menus | `RHMenuCampaignMap`, `RHMenuDialogue`, `RHShortBriefings`, `RHReconnaissanceReport`, `RHMissionStat` |

## Behavioural facts worth reproducing

- Mission loading pairs a "proto-level" (`.rhp`) with a mission (`.rhm`) and warns when they do not match; unknown
  chunks are skipped with a warning, so the loader is tag-driven and tolerant.
- Map background and minimap are looked up by map name, directory and "ambiance" (Day / Night / Fog / Custom).
- Every level needs a startup script; scripts reference actors and waypoints by id and the loader validates them.
- Campaign: missions have a location and a priority; "obligatory" missions; accessible mission list is computed
  from campaign state; the gang (Robin's men, peasants) and ransom / ARES counters are campaign values;
  `Campaign.bck` is written as backup.
- Characters must lie on a motion area, on the right layer and sector, not inside an obstacle (loader asserts).
- Buildings have occupants (humans only); doors are DEFAULT or GATE unless they belong to buildings or lifts.
- Actors cannot be in a patch sector twice; script sectors are polygons with at least 3 points.
- Soldiers have companies, patrol chiefs, patrol paths with synchronisation between paths (`CheckForSync`).
- Music states: ambient / tactical / alarm / fight, per location (see `docs/formats/sound.md`).

## Provenance

Observation of printable strings and the PE import table of the retail executable.
