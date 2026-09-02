# Sound tables (`.fxg`, `.sfk`, `actor*.dat`) and audio files

Status: **stub**. Audio itself is plain RIFF WAVE (`Sounds/*.wav`, `Musics/*.wav`, `<lang>/data/Text/Dialogues/*.wav`)
played through FMOD 3.x in the original.

## `.fxg` (magic `FXBK`, "effects bank")

`Sounds/Robin Hood.fxg` (game) and `Sounds/Menu/menu.fxg`. Header: `FXBK`, `u32 16`, `u32 1`, `u32 count`, then
records: `u32 id`, `u32`, `u16 0`, `pstring16 name` ("fx_0182", ...). Maps logical effect ids to `fx_NNNN.wav`.

## `.sfk` (magic `SFPK`, "sound pack")

`fx_0017.sfk`, `fx_0019.sfk`, `snd_055.sfk`, `Musics/Mission_win.sfk`, `Musics/Sherwood.sfk`. Header `SFPK`, `u32 1`,
`u32 0x40` (header size), then 16-bit values; a companion to the `.wav` of the same name.
Possibly precomputed "skip data" (the executable accepts `-GENERATESKIPDATA`).

## `Sounds/Exclamations/actor*.dat` (magic `NEUF`)

One per actor voice set (`actorCVCH` = civilian child, `actorPCLJ` = PC Little John, ...). Header `NEUF`, `u32 1`,
`u32 id` (0x1E849C.. = 2000028.., which are `WAVE` entry ids in `Text/actors.res`), `u32 count = 28`, then
`count` pairs of `u32` (index, variant count). Maps remark categories (E00..E27) to the wav list in `actors.res`.

## Music

`Musics/<City>_D.wav` (day) and `<City>_NF.wav` (night/fog) per city, `Sherwood.wav`, `Cross_{Amb,Tact,Alarm,Fight}.wav`
(crossroads ambient / tactical / alarm / fight), `Cast_{Alarm,Fight}.wav` (castle), `Menu.wav`, `Mission_win/lost.wav`.
The music state machine (ambient / alarm / fight) is part of the simulation and must be reproduced.

## Provenance

Observation.
