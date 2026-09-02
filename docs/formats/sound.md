# Sound tables (`.fxg`, `.sfk`, `actor*.dat`) and audio files

Status: **stub** for the tables; audio containers **verified**. Sound effects and dialogues (`Sounds/*.wav`,
`<lang>/data/Text/Dialogues/*.wav`) are RIFF WAVE, PCM 16-bit mono 22050 Hz. The music files in `Musics/*.wav`
are **Ogg Vorbis** streams (stereo, 44100 Hz) despite the `.wav` extension (FMOD 3 played them by content);
only `The_Last_Dance.wav` is a real 8-bit PCM WAVE. The engine sniffs the container (`OggS` / `RIFF`).

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
The four-letter set code (`PC..` player characters, `SD..` soldiers, `VP..` VIPs, `CV..` civilians) is the
`voice` field of the character tables in `Configuration/profile.cpf` ([profile.md](profile.md)), which is
how a mission actor reaches its voice set; the 34 `WAVE` entries of `actors.res` (54 / 33 / 88 / 28 paths per
class; 2000024 / 2000025 are empty placeholders) use the same codes in their file names.

## Music

`Musics/<City>_D.wav` (day) and `<City>_NF.wav` (night/fog) per city, `Sherwood.wav`, `Cross_{Amb,Tact,Alarm,Fight}.wav`
(crossroads ambient / tactical / alarm / fight), `Cast_{Alarm,Fight}.wav` (castle), `Menu.wav`, `Mission_win/lost.wav`.
Which three tracks (ambient, alarm, fight) a level uses is stored per level in `Configuration/profile.cpf`
([profile.md](profile.md)). The music state machine (ambient / alarm / fight) is part of the simulation and
must be reproduced.

## Provenance

Observation.
