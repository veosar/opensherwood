# Player profile and configuration

Status: **stub**.

- `Configuration/profile.cpf` (30 KB): binary, no magic. Starts `1B 00 00 00 14 00 32 00 4B 00 96 00 ...`: tables of
  small u16 values (difficulty / unit stats: 20, 50, 75, 150 ...). The community "Rhuce" tool edits unit and weapon
  statistics, which are probably stored here.
- `Configuration/keyset1.cfg`, `keyset2.cfg` (76 B): a 16-byte header/hash then 30 u16 DirectInput scan codes
  (two key sets).
- `Configuration/release.log`: bzip2-compressed log.
- `Savegame/Profiles` (magic `FORP`): `u32 6`, `u32 2`, `u32 1`, then per profile: 16-byte hash, u32s, key set copy.
  The community resolution patcher documents a float pair (width, height) at profile offset `0x104` and a profile
  stride of `0xD5E` bytes.

## Provenance

Observation; community documentation (github.com/phiresky/RobinHood-TheLegendOfSherwood-Resolution-Patcher).
