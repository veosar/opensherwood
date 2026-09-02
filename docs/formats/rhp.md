# RHP map file (`.rhp`, magic `MEUH`)

Status: **partial**. Chunk list known, chunk contents not yet decoded.

One `.rhp` per location: `sherwood`, `nottingham`, `lincoln`, `york`, `derby`, `leicester`, `Croisement01..03`
(forest crossroads used by ambush missions). The prerendered background is *not* inside; it is in
`Levels/<Variant>/<map>.map`.

## Chunk container

The file is one chunk: `tag char[4]`, `size u32` (= file size - 8), then a body that starts with `version u32`
followed by child chunks with the same `tag,size,body` shape. Each child body also starts with a `u32` version.
The engine checks versions ("version %d does not match %d in chunk %s").

## Children of `MEUH` (version 2), in file order

| Tag | Version | Size (Croisement01 / sherwood) | Guess |
|---|---|---|---|
| `SPOK` | 3 | 13 / 13 | header: `u32 = 100 / 92` (map id? scale?), `u32 = 1`, `u8 = 0` |
| `STAT` | 2 | 108919 / 70129 | static geometry: after version `u16 count` then records; contains sub-tags like `ZO` |
| `TEXT` | 2 | 118 / 672 | small table of `u16` triplets (text anchor positions?) |
| `WOAW` | 3 | 9314 / 15186 | `u32 count` then `u16` index list then packed data; possibly walkable areas / motion graph |
| `007 ` | 2 | 412 / 370 | `u16 count` then `u16` coordinate pairs (seek points? spy points?) |
| `FACE` | 2 | 220070 / 425491 | the biggest chunk: `u32 count` then records with `u16` coordinates; polygon faces (3D projection / elevation) |
| `FLIM` | 2 | 691 / 904 | `u16 count` then named entries (`pstring16` names like "Treecr01", "Croisement01 - ..."): animated elements |
| `FARM` | 4 | 117 / 271 | small records with flags; buildings / sectors? |
| ` AZ ` | 2 | 6 / 364 | `u16 count` (0 / 4) then records: possibly "A to Z" jump lines |
| `DARK` | 2 | 6 / 64 | `u16 count` (0 / 1) then a polygon: dark/shadow zones |
| `TUPO` | 3 | 678 / 6 | `u16 count` then named entries ("pixel_vert", ...): typed positions/patches |
| `LOUD` | 2 | 54 / 246 | `u16 count` then records with flags and coordinates: sound zones / geometry |
| `PPPP` | 4 | 872 / 155 | `u16 count` then records of `u16` points: paths / polylines |

## Provenance

Observation (chunk walker over all 9 files). Guesses are labelled as such and must be replaced by verified content
before the Rust parser exposes typed data. The community Blender importer for `.rhp` (Spellforge Editor) proves the
geometry is recoverable; it is closed source and was not consulted.
