# Save games (`Savegame/Profile_xxx/{Continue,Restart}`, magic `GSHR`)

Status: **stub**.

`GSHR`, `u32 48` (size of a fixed header block), `u16 'HA'`, `u16 0`, `u32 48`, 16-byte hash, then
a table of 32-byte records each containing the same 16-byte hash, a `u32` index, a `u32` value
(0x0A / 0x5DC / 0x7D0 / ...) and `u16 0xD5A8` pairs: this is the campaign state (mission list with status).
`Campaign.bck` in the game root is a backup of the same table (the executable writes "Unable to create the
campaign backup file"). The `_t` files are [image blob](image-blob.md) thumbnails (160x120).

Save compatibility with the original is a milestone M5 goal: loading a retail save must reproduce the same
campaign state.

## Provenance

Observation.
