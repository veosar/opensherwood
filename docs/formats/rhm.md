# RHM mission file (`.rhm`, magic `DUTY`)

Status: **partial**. Chunk list known, chunk contents not decoded.

39 files, one per mission (`H01_Lin_VL.rhm` = Hood mission 1 in Lincoln; `Emb01_FoA_EC` = ambush in forest area A;
`S0x` = Sherwood/story; `Str0x` = street; `Tac0x` = tactical; `EmbTut` = tutorial; `Sherwood.rhm` and
`SherwoodOutro.rhm` = camp). Each mission also has a `.scb` script with the same base name (except `Sherwood.rhm`
which uses `sherwood.scb`). The two-letter designer initials at the end of the name (EC, MK, MP, VL, JMS) match the
source path `C:\DOCUME~1\ECoste\...\script.scs` embedded in the compiled scripts.

## Container

Same as [RHP](rhp.md): `DUTY size version=2` then child chunks `tag size version ...`.

## Children of `DUTY`, in file order (Emb01_FoA_EC / Sherwood)

| Tag | Version | Size | Guess |
|---|---|---|---|
| `FOOT` | 4 | 30 / 26 | header: `u32 100/92`, `u32 1`, `pstring16` map name ("Croisement01" / "Sherwood"), `u32` |
| `POUF` | 3 | 1687 | `u16 count` then named entries: `pstring16 short name` ("Trapcr01"), `pstring16 long name` ("Croisement01 - piege01"), record: mission triggers / traps |
| `BOYZ` | 3 | 3001 | `u16 count` then per-actor blocks starting with a 4-char class tag (`MEOW`, `SCOT`, ...): the actors |
| `ZORG` | 2 | 72 | `u16 count` small records: zones? |
| `HIRN` | 2 | 442 | `u16 count` then records tagged `HOLE`: AI brains (Hirn = brain): patrol paths / waypoint lists |
| `RAIL` | 3 | 3255 | `u16 count` then coordinate records: railroads (the original console has a RAILROAD display) = fixed movement rails |
| `SKRO` | 4 | 6 | `u16 count` (0): scrolls (in-game parchment messages) |
| `TING` | 3 | 132 | `u16 count` then records tagged `FLIM` with names: animation triggers (things) |
| `GULP` | 2 | 491 | `u16 count` then `u16` coordinates: jump zones? items? |
| `CAVE` | 3 | 6 | `u16 count` (0) |

## Provenance

Observation (chunk walker over all 39 files). Chunk names are the developers' jokes (German/French);
the semantics above are guesses from the data shape and the console command names.
