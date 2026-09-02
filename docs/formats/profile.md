# Character / level profile table (`Configuration/profile.cpf`) and player configuration

Status: `profile.cpf` container **verified** (the grammar below consumes the retail file to the exact byte;
`harness/tools/re/cpf_probe.py`); the four string tables are **decoded** and their use by the missions is
**established** (see [rhm.md](rhm.md), "Actor profile mapping"); the numeric fields are `unknown_*` with
observed values. `keyset*.cfg`, `Profiles` and `release.log` remain **stub**.

## `profile.cpf`

30,441 bytes, no magic. `cpf` is a table of *profiles*: the ten player characters, the 68 armed
non-player humans, the 63 campaign levels and the 24 civilians, plus numeric tables in front. The mission
files reference the actor tables by 0-based position (`BORG.profile`, `OILE.profile`, `TOTO.profile`); the
level table maps the two-letter level codes of `Text/RHLevel??.red` to `.rhm` files, maps and music.

Conventions: little-endian; `pstring` = `u16 length` + Latin-1 bytes; `code` = `char[4]`, NUL padded (a
voice-set code such as `SDHL` or a level code such as `HA`).

```
u32  n_a (27)          n_a x { u8[28] head; 10 x u8[32] record }      table A (unknown; see below)
u32  n_b (4)           n_b x u8[81]                                    table B (unknown; see below)
u32  n_pc (10)         n_pc x PC
u32  n_sd (68)         n_sd x SD
u32  n_lv (63)         n_lv x LEVEL
u32  n_cv (24)         n_cv x CV
```

### Actor records

All three actor record kinds start with the same three strings:

| Field | Type | Meaning |
|---|---|---|
| sprite | pstring | base name of the sprite profile: `Characters/<sprite>.rhs` (83 distinct files over the 102 entries; all exist) |
| sequence | pstring | the 32-char sequence name inside that `.rhs` (equal to it for all 102 entries; French) |
| label | pstring | designer label, French: unit type + colour for soldiers, e.g. "Lancier Bleu"; empty for PCs |

then a kind-specific block that contains the `voice` code (`char[4]`): `PC..`, `SD..`, `VP..` or `CV..`,
naming `Sounds/Exclamations/actor<code>.dat` and the `Text/actors.res` `WAVE` list whose paths are
`Expressions\X_<CC>_<XX>_E<nn>_V<vv>.wav` (see [sound.md](sound.md)). Seven civilian entries have an empty
code (no exclamations).

```
PC   = sprite, sequence, label, u8[8] unknown_pre, code voice, u8[82] unknown_post
SD   = sprite, sequence, label, u8[21] unknown_pre, code voice, u8[55] unknown_post
CV   = sprite, sequence, label, u8[8] unknown_pre, code voice
```

#### PC table (10 entries; `TOTO.profile` indexes it; the player's team is drawn from it)

| # | sprite | voice | # | sprite | voice |
|---|---|---|---|---|---|
| 0 | RobinHood | PCRH | 5 | WillScarlet | PCWS |
| 1 | RobinTown | PCRH | 6 | LadyMarian | PCLM |
| 2 | LittleJohn | PCLJ | 7 | MerryManA | PCMA |
| 3 | Friar Tuck | PCFT | 8 | MerryManB | PCMB |
| 4 | Stuteley | PCST | 9 | MerryManC | PCMC |

`unknown_pre` as `u16[4]`: `(256, 100, 20, 50)` for both Robins, `(256, 0, 30, 70)` Little John,
`(256, 0, 1, 10)` Tuck, `(256, 0, 1, 20)` Stuteley, `(256, 0, 20, 100)` Will, `(256, 20, 20, 30)` Marian,
`(0, 10, 1, 20)`, `(0, 1, 20, 20)`, `(0, 1, 1, 20)` for the merry men (ability values; which is which is
unknown). `unknown_post` starts with a `u16` id that is *not* the table position (1, 2, 3, 5, 7, 4, 6, 8, 9,
10 in table order), continues with small `u16` values, the same six `f32` as the SD records
(`-6, -3, 6, 3`, then `150, 150`) and ends with two `u16` (`100, 80` Robins; `100, 100`; `50, 100` Tuck and
Marian; `200, 200` Will).

#### SD table (68 entries; `BORG.profile` indexes it)

Six families of six palette variants each, in the order of the entries; within a family the variant order
is 00 blue, 01 yellow, 02 orange, 03 red, 04 black, 05 green (the designer labels say so, and the first
`u16` of `unknown_pre` grows by 10 per step from blue to black; green repeats orange). Early missions use
blue / yellow, the last story mission red / black: the colour is the enemy's strength tier.

| Index | sprite | label kind (French) | voice | English |
|---|---|---|---|---|
| 0-5 | Guard A00..A05 | Hallebardier | SDHL | halberdier |
| 6-11 | Soldier A00..A05 | Epee | SDSW | swordsman |
| 12-17 | Archer00..05 | Archer | SDBW | archer (bowman) |
| 18-23 | Officier B00..B04, Officer05 | Officier | SDOF | officer |
| 24-29 | Soldier B00..B05 | Chevalier | SDKN | knight (on foot) |
| 30-35 | Guard B00..B05 | Lancier | SDLN | lancer |
| 36-41 | Crossbowman00..05 | Arbaletrier | SDCB | crossbowman |
| 42 | MerryManBow | | SDMD | merry man with bow (recruit / trainee) |
| 43 | MerryManStaff | | SDMD | merry man with staff |
| 44 | Trainer | | SDTM | the Sherwood trainer |
| 45-51 | Guard A05, Soldier A05, Archer05, Officer05, Soldier B05, Guard B05, Crossbowman05 | "... Vert Mechant" | as above | hostile green variants (used only by `H04_Lei_VL`, Ranulph's Leicester) |
| 52 | Knight02 | "Ne pas utiliser1" | SDRD | unused (0 references) |
| 53-55 | Knight01, Knight02, Knight03 | Cavalier | SDRD | mounted knights (yellow, orange, red) |
| 56-58 | Knight02 | "Ne pas utiliser2..4" | SDRD | unused (0 references) |
| 59 | Guisbourne | | VPGG | Guy of Gisborne (armed) |
| 60 | Longchamp | | VPGL | Longchamp (York) |
| 61 | Scatlock | | VPSK | Scathlock (Derby) |
| 62 | sherif | | VPSN | Sheriff of Nottingham (armed) |
| 63-64 | Officer02 | "Ne pas utiliser5..6" | SDOF | unused (0 references) |
| 65-67 | Officer02, Officer03, Officer04 | "Officier Special" | SDOF | special officers (orange, red, black) |

`unknown_pre` reads as `u16[5], u8, u16[5]`: e.g. halberdiers `(80..120, 10..30, 40..60, 0, 0), 1, (0, 5..80,
10..50, 0, 0)`; the `u8` is 1 for the two pole-arm families (`Guard A`, `Guard B`) and 0 otherwise. Officers,
knights and VIPs have non-zero fourth / fifth values (`80, 35` ... `0, 100`); the VIPs are `(250, 100, 100,
0, 100)`. Hit points and combat skills is the obvious reading; unverified. `unknown_post`: `u16` (3
halberdier / lancer, 5 swordsman / archer / crossbowman / trainer, 7 officer, 10 knight / VIP, 15 mounted, 1
merry man), a flag byte (`0xf1`, `0xe1`, `0xc1`, `0xc0`, `0xd0`, `0xc6`, `0xc7`, `0xc9`), five `u16` (ranges
/ percentages, zero for the green variants), a `u16` *class id* unique per unit type (`0x0b` merry man
staff, `0x0c` halberdier, `0x0d` archer / merry man bow / trainer, `0x0e` lancer, `0x0f` swordsman, `0x10`
officer, `0x11` knight, `0x12` mounted knight, `0x13` VIP, `0x14` crossbowman, `0x15..0x1b` the hostile
green variants), 8 zero bytes, `f32[4] = -6, -3, 6, 3`, `u8 1`, `f32[2] = 150, 150` (the 300x300 sprite
canvas centre, see [sprites.md](sprites.md)), `u16` (`0x40`; 3 for mounted knights, 10 for VIPs) and two
`u32` (0..3).

#### CV table (24 entries; `OILE.profile` indexes it)

| # | sprite | voice | `unknown_pre` (u32, u32) | # | sprite | voice | `unknown_pre` |
|---|---|---|---|---|---|---|---|
| 0 | TaxeCollector | CVTC | 0, 0 | 12 | Friar Tuck | (none) | 0, 0 |
| 1 | Mendicant | CVMT | 4, 1 | 13 | LadyMarian | (none) | 1, 1 |
| 2 | Child | CVCH | 3, 1 | 14 | MarianneWedding | (none) | 1, 1 |
| 3 | ManCivilianPoor | CVPM | 0, 1 | 15 | ManCivilianOld | CVOM | 0, 1 |
| 4 | ManCivilianRich | CVRM | 0, 0 | 16 | Guisbourne | VPGG | 5, 1 |
| 5 | ManCivilianFriend | CVPM | 0, 1 | 17 | Priest | (none) | 0, 1 |
| 6 | WomanCivilianPoor | CVPW | 1, 1 | 18 | Allan | (none) | 0, 1 |
| 7 | WomanCivilianRich | CVRW | 1, 0 | 19 | Sherif | VPSN | 5, 1 |
| 8 | WomanCivilianFriend | CVPW | 1, 1 | 20 | Scatlock | VPSK | 5, 1 |
| 9 | Ranulph | VPRA | 5, 1 | 21 | Longchamp | VPGL | 5, 1 |
| 10 | Godwin | VPGD | 5, 1 | 22 | Longchamp Dead | (none) | 5, 1 |
| 11 | PrinceJohn | VPPJ | 5, 1 | 23 | Soldier A03 | (none) | 0, 1 |

The first word is a civilian *kind*: 0 man, 1 woman, 3 child, 4 beggar, 5 notable (the unarmed "civil"
versions of the story characters; 12-14 are Tuck / Marian in civilian clothes). The second word is 0 for the
tax collector, the two rich civilians and civilian Tuck, 1 otherwise (meaning unknown). Entry 23 reuses a
soldier sprite as a civilian for `H12_Not_MP` (its label says so).

### LEVEL records (63 entries)

```
LEVEL = code level_code, pstring map, pstring mission_file, pstring title, u32 unknown_a, u32 location,
        u8 unknown_c, u8 unknown_d, u16 unknown_e, u16 unknown_f, u32 unknown_g, u32 unknown_h,
        u32 n_after,  n_after  x code,
        u32 n_until,  n_until  x code,
        u16 0, u16 10000, u16 unknown_i, u16 unknown_j,
        u16 k, k x u32 unknown, u8[12] unknown_l,
        i8[3] unknown_slots, u16[7] unknown_m,
        pstring music_ambient, pstring music_alarm, pstring music_fight
```

(41 records have `k = 0`, 22 have `k = 1`; the record length is otherwise fixed apart from the two code
lists.)

- `level_code`: the two letters of `Text/RHLevel<code>.red`; all 57 `.red` files have an entry, plus `EJ..EO`,
  `TG..TI`, `TT`, `TV` (placeholders) and `EY` (`SherwoodOutro`).
- `map`: the proto-level name as in the `.rhp` / `FOOT` (`Lincoln`, `nottingham`, `Croisement01` ...);
  `mission_file`: the `.rhm` base name, or `Impossible_mission` for the 22 placeholder entries; `title`: an
  internal working title (not the `Level.res` title).
- `unknown_a`: 0 story (H, S), 1 assault (AA-AC), 3 ambush (E), 4 Sherwood, 5 defend (D), 6 tactical (T).
- `location`: 1 Croisement01, 2 Croisement02, 3 Croisement03, 4 Derby, 5 Leicester, 6 Lincoln, 7 Nottingham,
  8 Sherwood, 9 York (consistent with `map` in all 63 entries).
- `unknown_c`: 0 (`HQ`, `HA`, `SA`, `EY`) or 1. `unknown_d`: 1..6 (1 for `HA`, `SA`, `HG`, `HI`, all ambushes
  and tactical missions; 6 for most H missions). Not the `SCOT` record count. `unknown_e`: 0 or 256;
  `unknown_f`: 200 (tactical), 300 (ambush), 600..800 (story), 0 (Sherwood); `unknown_g`: 0, 1 (placeholders),
  5000..100000 (a money value?); `unknown_h`: 200000 for most, 20000..90000 for five ambushes, 0 for
  placeholders and the outro.
- `n_after` / `n_until` lists (0..2 codes each) form the campaign graph. Reading them as "available after"
  and "removed after" fits the manual's description: `HA` has no prerequisite and is removed by itself
  (one-shot); `SA` needs `HA`; `HB` needs `SA`; `HC` needs `HB` and `SB`; `HD` needs `SC`; `HE` needs `HD`;
  `HF` needs `SD`; `HG` needs `EI`; `HH` needs `HG`; `HI` needs `DD`; `SB` needs `EZ` (the tutorial ambush,
  which itself needs `SA`); `SC` needs `HC`; `SD` needs `HE` and `SC`; `SE` needs `HF`; `EA`, `EB`, `EC`,
  `EE`, `EF`, `EH` need `EZ` and last until `HH`; `ED` needs `HF`; `EG` needs `HE`; `EI` needs `SE`; `AA` needs
  `DA`; `AB` needs `DB`; `AC` needs `HH`; `DA` needs `DB`; `DB` needs `SD`; `DC` needs `AB`; `DD` needs `AC`;
  all tactical missions need `SD`; the placeholders `EJ..EO` need `EZ`, `ET` needs `EZ`, `EU` needs `ET`, `EV`
  needs `EU`. Every story mission, the tutorial and the placeholders `ET..EX`, `TW..TZ` list themselves in
  `n_until`; `EA`, `EB`, `EC`, `EE`, `EF`, `EH` list `HH`; the others have an empty list. This is the
  file-level view; whether the executable applies exactly these rules is not verified (see
  [../original/campaign-flow.md](../original/campaign-flow.md)).
- `unknown_i`: 100 (story, Sherwood), 60 (defend / assault, `EF`, `EH`, `EI`), 40 (ambush), 50 (tactical),
  0 (outro); `unknown_j`: 1..4 for story / defend / assault, 0x0a..0x13 for tactical, 0x33..0x4b for ambushes
  and the outro (a per-kind serial); `k x u32`: 0 or 1; `unknown_l`: zero except a few `01` bytes in the
  defend / assault records, `HG` and `HI`; `unknown_slots`: `-1 -1 -1` mostly, `1 -1 -1` (`HD`), `3 -1 -1`
  (`HE`), `9 -1 -1` (`HI`), `11 11 11` (outro), `2 0 0`, `4 1 1`, `6 3 3`, `8 5 5` (`DA..DD`), `3 1 1`,
  `5 3 3`, `7 5 5` (`AA..AC`); `unknown_m`: two small values (0..3, 0..6), then `(0, 0, 10, 2, 3)` for most,
  `(5, 2, 10, 2, 3)` Sherwood, `(2, 2, ...)` tactical, `(3, 0, 1500..3000, 500..2000, 3)` defend and
  `(12, 5..7, 2500..3500, 500..1500, 3)` assault.
- `music_*`: base names in `Musics/` (`Lincoln_D`, `Nottingham_NF`, `Cross_Amb`, `Cross_Tact`, `Cast_Alarm`,
  `Cast_Fight`, ...): the ambient, alarm and fight tracks of the level (see [sound.md](sound.md)).

### Tables A and B (unknown)

Table A: 27 blocks of a 28-byte head (`u16[14]`, starting `20 50 75 150` or `25 50 70 150` or `45 65 90 150`)
and ten 32-byte records (`u16[16]`, first word always 2). Ten records per block equals the PC count, so
"one parameter set per player character for 27 actions / skills" is the working hypothesis. Table B: four
81-byte records (`u16[20], u8, u16[20]`) with values 0..400 that differ only in a few words; the four
difficulty presets is the hypothesis (`Configuration` is what the community "Rhuce" stat editor patches).

## Other configuration files

- `Configuration/keyset1.cfg`, `keyset2.cfg` (76 B): a 16-byte header/hash then 30 u16 DirectInput scan codes
  (two key sets).
- `Configuration/release.log`: bzip2-compressed log.
- `Savegame/Profiles` (magic `FORP`): `u32 6`, `u32 2`, `u32 1`, then per profile: 16-byte hash, u32s, key set copy.
  The community resolution patcher documents a float pair (width, height) at profile offset `0x104` and a profile
  stride of `0xD5E` bytes.

## Provenance

Status `observed` unless marked. Build: GOG English, executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`; file `DATA/Configuration/profile.cpf`
(30,441 bytes). Method: data-file observation only (no executable analysis): printable-string scan, hex
inspection of the section boundaries, then the grammar above implemented in
`harness/tools/re/cpf_probe.py` and checked for exact consumption (`--hex` prints every numeric field,
`--rhs` verifies the `sequence` string against the `.rhs` header of every `sprite`). Cross-checks: the mission
join in `harness/tools/re/rhm_profiles.py` (see [rhm.md](rhm.md)); voice codes against the file names in
`Sounds/Exclamations/` and the `WAVE` paths in `Text/actors.res`; level codes against `Text/RHLevel??.red`;
`map` against the `FOOT` chunk of every `.rhm`. Analyst session 2026-09-02. Tests depending on this: see
"Parser".

## Parser

`opensherwood_formats::cpf` (`crates/opensherwood-formats/src/cpf.rs`): `parse` reads the six tables with
the grammar above and requires exact consumption; `looks_like_profile_table` is the structural detection
used by `detect` (`FileKind::ProfileTable`, since the file has no magic: the first three counts must chain
through the fixed-size tables A and B to a printable sprite string). Tables A and B and every numeric field
are kept as raw `unknown_*` bytes / words named as in this document. `opensherwood-tools cpf <file> [--hex]`
prints the tables; `inspect` reports the counts. The engine reads the file in
`opensherwood-app/src/mission.rs` to pick the sprite of every `BORG` / `OILE` / `TOTO` actor (the `sprite`
string, i.e. `Characters/<sprite>.rhs`; the `sequence` string is the French name inside that file and is
not what the sprite bank is keyed by).

Tests: `cpf::tests::{parses_synthetic_table, truncation_and_trailing_bytes_are_errors,
oversized_counts_are_rejected_early}` (synthetic bytes); `profile_table_parses_and_its_sprites_exist` in
`crates/opensherwood-formats/tests/gamedata.rs` (retail: counts 27 / 4 / 10 / 68 / 63 / 24, every actor
sprite is an existing `Characters/<sprite>.rhs` containing the record's `sequence`, every non-placeholder
level names an existing `.rhm` and its graph codes exist);
`mission::tests::profiles_resolve_through_the_table_and_fall_back_when_unavailable` (app);
`harness/tests/data/test_mission.py::test_npc_sprites_come_from_the_profile_table` (H01: halberdiers,
lancers and the beggar wear their table sprites).

Observation from the parser run: `EY` (`SherwoodOutro`) has `map = Sherwood` but `location = 2`, so the
"`location` consistent with `map` in all 63 entries" claim above holds for 62; and 24 records (not 22)
name `Impossible_mission`, leaving 39 records for the 39 `.rhm` files.
