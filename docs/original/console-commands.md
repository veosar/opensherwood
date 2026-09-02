# Developer console of the original game

The retail executable contains a developer console ("Robin Hood Console Help File"). In the community-patched
executable (rhmods.com "Developer Console" by Gravitr) it opens with **F11** while a selected character is
standing and the mouse is over the kneeling icon. NPC selection: hold Left Alt and hover; PC selection after
`PCSIGHT`. Status per the community PDF: works / does not work / crashes.

The debug displays are the most valuable part for us: they visualise internal data structures of the original
engine on top of the map, which gives us ground truth for our parsers (motion obstacles, pathfinder graph,
script zones, light zones, seek points, rails, projection areas) without any disassembly.

## Debug displays (visual ground truth)

| Command | Shows |
|---|---|
| `AI` | AI information of the selected NPC |
| `BIG BROTHER` | actor infos |
| `BABYLON` | all NPC remarks on screen |
| `EULER` | the pathfinder graph |
| `MOTION` | all motion obstacles and doors |
| `EINSTEIN` | all 3D obstacles |
| `ELEVATION` | bonds (yellow, red when crossed), character elevation (blue), character movement (white) |
| `PROJECTION` | all 3D projection areas |
| `RAILROAD` | railroads |
| `CESTLAZONE` | script zones |
| `LIGHT` | light zones (night maps) |
| `SEEKANDDESTROY` | all seek points |
| `NOISE` | ranges of walk noise of the PCs |
| `SHADOW` / `SPHERE` | free shadow polygon / shadow polygon sphere (does not work) |
| `ANIM` | all animation polylines (does not work) |
| `COMPANIES` | company numbers |
| `PCSIGHT` | enable PC view cones |
| `FPS` | frame rate |
| `STATUS PC` / `STATUS FRAMECACHE` / `STATUS SHADOW` / `STATUS HARDWARE` | status of PCs / sprite caching system / sprite cache / hardware |
| `REPORT` | complete campaign state report (prints campaign, missions with ACCESSIBLE / NOT ACCESSIBLE / AVAILABLE, gang, ARES, ransom) |
| `LEVEL TEXT DG|DB|PT|SB` | all texts of the level: dialogues, debriefings, popup texts, short briefings |
| `HIDEINTERFACE` / `DISPLAYINTERFACE` | hide / show the UI (useful for clean screenshots) |

## State manipulation (useful for oracle scenarios)

| Command | Effect |
|---|---|
| `FREEZE` | freeze / unfreeze all NPCs |
| `ROTER ALARM` | alert all NPCs |
| `HONOLULU` / `LAST MAN STANDING` | remove selected NPC / all but selected |
| `UBIQUITY` or `UNBLIP` | reveal all actors |
| `WAKEUP`, `MORPHEUS`, `HADES`, `COMA`, `LUKAS`, `MISTER SANDMAN`, `SAN PETRUS`, `NUKE`, `DIES IRAE`, `BUD SPENCER` | wake / knock out / kill / stun actors |
| `HIGHLANDER` or `IMMUNITY`, `HIGHLANDER2`, `GOLDENEYE`, `PAMELA ANDERSON` or `PAM` | invulnerable PCs / NPCs, invisible PCs, dumb soldiers |
| `FULLHOUSE`, `BINGO`, `AMOR`, `WASP MASTER`, `EZB`, `CASH ...` (crashes), `WAPPEN`, `AMULETS` or `GOODLUCK`, `KOLKOZ` or `MERRYMAN` | ammunition, money, blazons, amulets, new peasant |
| `WIN` or `WINNER`, `LOOSE`, `I AM THE WINNER` | win / lose mission, win campaign |
| `CAMPAIGN <file>` | load campaign values |
| `CHROMA a b c d e` | change the colour of a PC |
| `CALL` | call a PC's method (does not work) |
| `ALARM` | reinforcement arriving (does not work) |
| `OPTIMIZE`, `SARKOZY`, `FORGET`, `ASSERTFALSE` (crashes) | memory manager tools |
| `HELP` | list |

## Command-line arguments of `Robin Hood.exe`

`-NOSCRIPT`, `-SIMULATE`, `-GAMEPAD`, `-SOUNDDEVICE`, `-SETREG`, `-CHECKSOUNDDATA`, `-GENERATESKIPDATA`,
`-EXTRACTHUNK`, `-GETMAJORVER`, `-GETMINORVER`, `-GETBUILRHER`. The engine also accepts a character name on the
command line ("Unknown character specified on the command line": Robin des bois, Petit Jean, Frere Tuck, Stutely,
Will Ecarlate, Lady Marianne, Paysan A/B/C) and reports "Invalid arguments %s on command line".
The Linux port (RuneSoft) adds `-NOINPUTGRAB`, `-NOFULLSCREEN`, `-FULLSCREEN`.

## Provenance

String extraction from the retail executable (names and help texts), community PDF
(rhmods.com/content/uploads/Console-command-list.pdf) for the F11 procedure and the works / does-not-work status.
