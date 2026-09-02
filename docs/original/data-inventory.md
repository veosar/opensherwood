# Retail data inventory (GOG build with the 2025-04-08 Ready2Play launcher)

Root of the install (`Robin Hood - The Legend of Sherwood/`), ~1 GB:

| Path | Purpose |
|---|---|
| `Robin Hood.exe` (2.9 MB) | the game, PE32 i386, MSVC 6 link, 2003-01-15 |
| `Game.exe` (42 KB) | community launcher (MinGW) that starts `Robin Hood.exe`; GOG's play task points here |
| `ddraw.dll` + `ddraw.ini` | cnc-ddraw wrapper (windowed / d3d9on12) |
| `binkw32.dll` -> `binkw32x.dll` | Bink shim -> real Bink (2002) |
| `Fmod.dll` | FMOD 3.x (UPX packed) |
| `dxcfg.exe/.ini` | DirectX settings tool |
| `Campaign.bck` | campaign state backup |
| `Manual.pdf` | manual (mechanics reference) |
| `2047/data/` | language overlay (English): `Cinematics/*.vid`, `Interface/Slideshow_in.pak`, `Interface/Start.sxt`, `Text/Level.res`, `Text/Dialogues/*.wav`, `Sounds/Exclamations/Expressions/*.wav`. The executable references these as `Data\...`; the language directory is resolved at runtime |
| `_Backup/` | original `Game.exe` / `Robin Hood.exe`, msvcr90, SDL.dll, splash |

`DATA/`:

| Directory | Files | Size | Formats |
|---|---|---|---|
| `Animations/{Day,Fog,Night}` | 116 | 0.6 MB | `.rhs` |
| `Characters` | 117 | 15 MB | `.rhs` |
| `Configuration` | 4 | 35 KB | `.cpf`, `.cfg`, `.log` |
| `Interface` | 31 | 18 MB | `DEFAULT.RES`, `Loading.pak`, `Fonts/*` |
| `Levels` | 135 | 105 MB | 9 `.rhp`, 39 `.rhm`, 39 `.scb`, 28 `.min`, 20 `.map` |
| `Musics` | 23 | 12 MB | `.wav`, `.sfk` |
| `Savegame` | 5 | 0.4 MB | `Profiles`, `Continue`, `Restart`, thumbnails |
| `Sounds` | 579 | 52 MB | `.wav`, `Exclamations/actor*.dat`, `.sfk`, `.fxg` |
| `Text` | 58 | 133 KB | 57 `.red`, `actors.res` |
| `robinhood.bks` | 1 | 565 MB | sprite bank |
| `robinhood.dic` | 1 | 9.3 MB | sprite dictionary |

Missions (`Levels/*.rhm`): `EmbTut_FoC_EC`, `Emb01..09` (ambush, forest areas A/B/C), `H01_Lin_VL`, `H02_Not_EC`,
`H03_Der_MK`, `H04_Lei_VL`, `H05_Lin_EC`, `H07_Not_MK`, `H09_Not_VL`, `H10_Yor_VL`, `H12_Not_MP`, `S01_Not_VL`,
`S02_Lei_MP`, `S03_FoB_MP`, `S04_Der_EC`, `S05_Yrk_EC`, `Str01_Lin_EC`, `Str02_Der_MP`, `Str03_Yor_MK`,
`Tac01..06`, `Tac17..19`, `Tac21`, `Sherwood`, `SherwoodOutro`. Locations: Lincoln (Lin), Nottingham (Not),
Derby (Der), Leicester (Lei), York (Yor/Yrk), forest areas (FoA/FoB/FoC = Croisement01..03), Sherwood camp.

Characters (`Characters/*.rhs`): RobinHood, RobinTown, LittleJohn, WillScarlet, Scatlock, Stuteley, Friar Tuck,
LadyMarian, MarianneWedding, Allan, Godwin, Guisbourne, Longchamp (+Dead), PrinceJohn, Ranulph, Sherif, Priest,
TaxeCollector, Trainer, Mendicant, Child, civilians (Man/Woman x Friend/Old/Poor/Rich), MerryMan A/B/C/Bow/Staff,
Soldier A00-A05 / B00-B05, Guard A/B 00-05, Archer 00-05, Crossbowman 00-05, Knight 01-03, Officer 02-05,
Officier B00-B04, ACCESSORIES_* (ale, apple, arrow, coat, coin, moneybag, net, stone, wasp, waspsting),
BONUS_* (pickups), RELIC_* (ampulla, arrow, book, crown, sceptre, spoon, stamp, sword), Blip00, TG_* (training targets).
