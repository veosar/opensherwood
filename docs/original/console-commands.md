# Developer console of the original game

The retail executable contains a developer console with a built-in help list. In the community-patched
executable (rhmods.com "Developer Console" by Gravitr) it opens with **F11** while a selected character is
standing and the mouse is over the kneeling icon. NPC selection: hold Left Alt and hover; PC selection after
`PCSIGHT`. Status per the community PDF: works / does not work / crashes.

Command names are listed because they are what a tester types into the original to use it as an oracle
(the manual, p.37, and the public community list document them); the descriptions are ours, not the
executable's help text.

The debug displays are the most valuable part for us: they visualise internal data structures of the original
engine on top of the map, which gives us ground truth for our parsers (motion obstacles, pathfinder graph,
script zones, light zones, seek points, rails, projection areas) without any disassembly.

## Debug displays (visual ground truth)

| Command | What it draws |
|---|---|
| `AI` | the AI state of the selected NPC |
| `BIG BROTHER` | per-actor information overlay |
| `BABYLON` | every NPC's current remark |
| `EULER` | the pathfinder graph |
| `MOTION` | motion obstacles and doors |
| `EINSTEIN` | 3D obstacles |
| `ELEVATION` | bonds (yellow, red when crossed), character elevation (blue), character movement (white) |
| `PROJECTION` | 3D projection areas |
| `RAILROAD` | patrol paths |
| `CESTLAZONE` | script zones |
| `LIGHT` | light zones (night maps) |
| `SEEKANDDESTROY` | seek points |
| `NOISE` | the walk-noise radius of each PC |
| `SHADOW` / `SPHERE` | shadow polygon debug views (do not work) |
| `ANIM` | animation polylines (does not work) |
| `COMPANIES` | company numbers |
| `PCSIGHT` | PC view cones |
| `FPS` | frame rate |
| `STATUS PC` / `STATUS FRAMECACHE` / `STATUS SHADOW` / `STATUS HARDWARE` | status of PCs / sprite caching / sprite cache / hardware |
| `REPORT` | a campaign state dump (campaign values, per-mission accessibility, gang, ARES, ransom) |
| `LEVEL TEXT DG|DB|PT|SB` | the level's texts: dialogues, debriefings, popup texts, short briefings |
| `HIDEINTERFACE` / `DISPLAYINTERFACE` | hide / show the UI (useful for clean screenshots) |

## State manipulation (useful for oracle scenarios)

| Command | Effect |
|---|---|
| `FREEZE` | freeze / unfreeze all NPCs |
| `ROTER ALARM` | alert all NPCs |
| `HONOLULU` / `LAST MAN STANDING` | remove the selected NPC / all but the selected one |
| `UBIQUITY` or `UNBLIP` | reveal all actors |
| `WAKEUP`, `MORPHEUS`, `HADES`, `COMA`, `LUKAS`, `MISTER SANDMAN`, `SAN PETRUS`, `NUKE`, `DIES IRAE`, `BUD SPENCER` | wake / knock out / kill / stun actors (single or all) |
| `HIGHLANDER` or `IMMUNITY`, `HIGHLANDER2`, `GOLDENEYE`, `PAMELA ANDERSON` or `PAM` | invulnerable PCs / NPCs, invisible PCs, dumb soldiers |
| `FULLHOUSE`, `BINGO`, `AMOR`, `WASP MASTER`, `EZB`, `CASH ...` (crashes), `WAPPEN`, `AMULETS` or `GOODLUCK`, `KOLKOZ` or `MERRYMAN` | ammunition, money, blazons, amulets, a new peasant |
| `WIN` or `WINNER`, `LOOSE`, `I AM THE WINNER` | win / lose the mission, win the campaign |
| `CAMPAIGN <file>` | load campaign values |
| `CHROMA a b c d e` | change the colour of a PC |
| `CALL` | call a PC's method (does not work) |
| `ALARM` | reinforcement arriving (does not work) |
| `OPTIMIZE`, `SARKOZY`, `FORGET`, `ASSERTFALSE` (crashes) | memory manager tools |
| `HELP` | list |

## Command-line arguments of `Robin Hood.exe`

`-NOSCRIPT`, `-SIMULATE`, `-GAMEPAD`, `-SOUNDDEVICE`, `-SETREG`, `-CHECKSOUNDDATA`, `-GENERATESKIPDATA`,
`-EXTRACTHUNK`, `-GETMAJORVER`, `-GETMINORVER`, `-GETBUILRHER` (switch names kept as identifiers; their
meaning is inferred from the names). The engine also accepts a character name on the command line (the
accepted identifiers are the hero names) and reports invalid arguments with a generic message.
The Linux port (RuneSoft) adds `-NOINPUTGRAB`, `-NOFULLSCREEN`, `-FULLSCREEN`.

## Provenance

Command names: the community PDF (rhmods.com/content/uploads/Console-command-list.pdf), the manual (p.37) and
the executable's own help list (names only; the help sentences are not reproduced). Works / does-not-work
status and the F11 procedure: the community PDF.
