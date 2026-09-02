# SCB compiled mission script (`.scb`, magic `SBSCRIPT`)

Status: **partial**. Header and function table shape known; instruction set unknown.

The original toolchain compiled a text script (`script.scs`, not shipped) into this bytecode. The engine
error string "File version of .scb not good expecting %f got %f" confirms a float version.

## Header

| Offset | Type | Value |
|---|---|---|
| 0 | char[8] | `SBSCRIPT` |
| 8 | f32 | version `1.5` (`00 00 C0 3F`) |
| 12 | u32 | count A (e.g. 23) |
| 16 | u32 | length of source path string (44) |
| 20 | bytes | source path, e.g. `C:\DOCUME~1\ECoste\LOCALS~1\Temp\\script.scs` |
| ... | | function table |

## Function table

Sequence of records: `u32 name_len`, name bytes (`StartUp`, `PlusSous`, `Choix_Popup`, `LockAllSoldiersAI`,
`UnlockAllSoldiersAI`, `KillActorsInZone`, ...), then `u32` fields (counts / offsets / types) whose meaning is still
to be determined. Every mission script starts with `StartUp` ("a Level is missing its startup script" is a fatal
error in the engine).

## Runtime API

Names of built-in functions the VM exposes were recovered from error strings in the executable
(see [../original/executable-notes.md](../original/executable-notes.md)): `IsActorCharacter`, `IsActorObject`,
`IsActorCart`, `IsActorPC`, `IsActorAnimal`, `IsActorNPC`, `IsActorSoldier`, `IsActorCivilian`, `IsAnimationActive`,
`IsActorActive`, `SetAnimationState`, `UnBlip`, `IsUnblipped`, `GetDistance`, `ComputeLocationBetween`,
`AreAllEnemiesInsideHS`, `GetActorLocation`, `SetActorLocation`, `GetMovementStyle`, `MakeNoise`, camera moves,
zoom, sequences (`Start` / `Thanx` record a cut-scene sequence), gates, patches, `SendMessage` to NPC script classes,
properties (wasp nests, nets, plants, legs, ales, apples, stones, money).

## Plan

Write a disassembler in `opensherwood-cli` that dumps the function table and raw opcode stream; correlate opcodes across the
39 scripts and with observable mission behaviour; then specify the VM. The community's Lua layer (Spellforge)
replaces `.scb` with `.lua`, so a Lua API compatible with theirs is the modding target; the SCB VM is needed only to
run the retail campaign unchanged.

## Provenance

Observation (hexdump of all 39 files; string extraction from the retail executable).
