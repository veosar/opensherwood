# File format specifications

Each document describes one file format used by *Robin Hood: The Legend of Sherwood* (RH) as far as
we understand it. Status tags:

- `verified` - our parser reads every file of this kind in the retail GOG data set without error and
  the interpretation was cross-checked (rendered, listened to, or matched against the original game).
- `partial` - container structure known, some fields unknown.
- `stub` - only the header is known.

All integers are little-endian unless stated otherwise. Offsets are in bytes from the start of the file.
"u16/u32/i32/f32" have their usual meanings. Strings are `u16 length + bytes` ("pstring16") unless noted.

| Format | Extension / magic | Purpose | Status | Spec |
|---|---|---|---|---|
| Image blob | header `w,h,flag,size` | Generic compressed picture (used by `.map`, `.min`, `.pak`, `.sxt`, `_t` thumbnails and inside SRES) | verified (RGB565) | [image-blob.md](image-blob.md) |
| SRES | `SRES` (`.res`, `.RES`) | Resource archive: UI pictures, texts, sound lists | container verified for retail archives; widget state meanings partial; version unchecked | [sres.md](sres.md) |
| RHP | `MEUH` (`.rhp`) | Map / level geometry, sectors, motion, lights | verified: occluder masks, motion area + obstacles, projection areas, bonds, zones, sprite instances; partial: path graph, FARM/AZ/TUPO/LOUD | [rhp.md](rhp.md) |
| RHM | `DUTY` (`.rhm`) | Mission: actors, waypoints, zones, scripts binding | partial: all 10 chunks framed and consumed for all 39 files (corpus test), POUF stored raw, profile table and several field meanings unknown; parser does not enforce the chunk set | [rhm.md](rhm.md) |
| SCB | `SBSCRIPT` (`.scb`) | Compiled mission script bytecode | container verified (classes, variables, functions, 9-byte instructions); opcode semantics unknown | [scb.md](scb.md) |
| Sprite bank | `C9 EB 03 00` (`.rhs`, `.dic`, `.bks`) | Character/animation sprites and the 565 MB pixel bank | framing verified; all 404,855 frames decoded once on 2026-09-02 (dated corpus observation, the maintained test samples ~4,700 frames); shadow/alpha rendering unverified | [sprites.md](sprites.md) |
| Sprite animations | (layout of `.rhs` animation lists) | 16-direction action blocks, action ids, timing | verified (idle/walk/run...), partial (ids > 75) | [sprite-animations.md](sprite-animations.md) |
| Fonts | `SBFONT` (`.bfn`, `.fnt`), `SBTTFT` (`.tfn`) | Bitmap fonts and TrueType descriptors | pixels verified (glyph table, colour + mask strips); text layout (advance, adjustments) partial | [fonts.md](fonts.md) |
| Sound tables | `FXBK` (`.fxg`), `SFPK` (`.sfk`), `NEUF` (`actor*.dat`) | Sound effect name tables, sound packs, remark tables | stub | [sound.md](sound.md) |
| Text index | `.red` | Per-level text id tables | stub | [red.md](red.md) |
| Profile / config | `.cpf`, `keyset*.cfg`, `Profiles` (`FORP`) | Player profile, key bindings | stub | [profile.md](profile.md) |
| Save game | `GSHR` | Campaign / mission save | stub | [savegame.md](savegame.md) |
| Video | `BIKi` (`.vid`) | Bink video (RAD Game Tools) | stub (magic detection only; Bink is a third-party format) | [video.md](video.md) |

The data inventory (which files exist, sizes, counts) is in [../original/data-inventory.md](../original/data-inventory.md).
