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
Derby (Der), Leicester (Lei), York (Yor/Yrk), forest areas (FoA/FoB/FoC = the three forest crossings), Sherwood camp.

Character sprites (`Characters/*.rhs`, 117 files; names are not listed here, the engine reads them from
`Configuration/profile.cpf`, see `docs/formats/profile.md`): 82 humanoid profiles (10 player characters, about
a dozen named story characters including an armed and an unarmed version of each antagonist, 12 generic
townspeople and civilians, 5 merry-man variants, 48 soldier and knight sprites (seven families in up to six
colour tiers, three mounted knights and a few extra officer variants), a trainer, a corpse), 10 accessory sprites (thrown
and carried items), 13 bonus pick-ups, 8 relics, 3 training-target objects and one blip marker. File-name
prefixes group the non-humanoid ones (`ACCESSORIES_`, `BONUS_`, `RELIC_`, `TG_`).
