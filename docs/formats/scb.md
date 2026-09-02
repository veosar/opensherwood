# SCB compiled mission script (`.scb`, magic `SBSCRIPT`)

Status: **container decoded, instruction set partially characterised**. `crates/opensherwood-formats/src/scb.rs`
parses all 39 retail files to the last byte (classes, variables, function tables, instructions) and prints a raw
disassembly; opcode semantics are hypotheses except where stated.

The original toolchain compiled a text script (`script.scs`, not shipped) into this bytecode. The engine error
string "File version of .scb not good expecting %f got %f" confirms a float version. The same VM family
(Spellbound "SBLib") is used by *Desperados: Wanted Dead or Alive*, whose `.scb` version 1.0 is a *text* container
with 10-byte instructions; ours is a binary container, version 1.5, with 9-byte instructions.

## Layout

All integers little-endian; `pstring32` = `u32 length` + Latin-1 bytes.

| Offset | Type | Value |
|---|---|---|
| 0 | char[8] | `SBSCRIPT` |
| 8 | f32 | version `1.5` (`00 00 C0 3F`) |
| 12 | u32 | class count (6..=70; 42 in the tutorial) |
| 16 | class[] | classes, back to back; the file ends after the last one |

### Class

| Field | Type | Notes |
|---|---|---|
| source_path | pstring32 | `C:\DOCUME~1\ECoste\LOCALS~1\Temp\\script.scs` (same in every class of a file) |
| name | pstring32 | `StartUp` for the first (level) class; otherwise the name of a mission element (`hidden_pc01_80000048`, `Lancier03_800000db`, `filet02_80000027`, `Archer01_8000012d`, `Point1__0___8000039f`) |
| variable_count | u32 | 0..=8 |
| size_of_variables | u32 | 4 x variable_count in every file |
| variables | variable[] | see below |
| function_count | u32 | 1..=17 |
| functions | function[] | see below |
| quad_count | u32 | 20..=2336 |
| quads | 9 x quad_count bytes | instructions |

Variable: `u8 type_tag` (2 = plain 4-byte value, names `iSoldat`, `bDontRunTwice`, `PlusSous`; 7 = object
reference), `u8 type_name_length` + type name (`Actor`, `Location` for tag 7; empty for tag 2), `pstring32 name`,
`u32 offset` (0, 4, 8, ... in declaration order). 675 tag-2 and 16 tag-7 variables in retail data.

Function: `pstring32 name`, `u32 address` (index of the first instruction in the class quad array; functions are
laid out in table order), `u32 unknown_0` (2: 6832, 3: 959, 4: 381, 5: 3, 6: 1), `u32 unknown_1` (0 or 4),
`u32 unknown_2` (0, 4, 8, 12, 16, 20), `u32 size_of_volatile` (0..=32, step 4), `u32 size_of_tempor` (0..=56,
step 4). The last two are verified: the instruction at `address` is always opcode `0x03` with exactly these two
values as its `u16` operands (8176 of 8176 functions). `unknown_0..2` follow the Desperados fields
`nbOfParams, sizeOfRetVal, sizeOfParams` by position, but `unknown_2 == 4 * (unknown_0 - 1)` holds only for a
part of the functions, so this is unverified.

Function names are the engine's script callbacks (listed as strings in the executable: `Initialize`,
`ActionChange`, `FilterAIEvent`, `ProcessMessage`, `ActivatedByApple/Arrow/Hand/Heal/Lever/Money/Search/Stone/Sword/
Listenable`, `Hourglass`, `CheckVictoryCondition`, `Finalize`, `PostInitialize`, `IsTaken`, `ReachPoint`,
`EnterZone`, `ExitZone`) plus designer helpers in the level class (`LockAllSoldiersAI`, `PlusSous`, `ejectage`,
`tuage`, `SetTrap`, `Magie`, ...). Every class has `Initialize`; actor classes have `ActionChange`, `HandleEvent`,
`ProcessMessage`, soldiers add `FilterAIEvent`; objects have the nine `ActivatedBy*` handlers and `IsTaken`;
script polygons have `EnterZone` / `ExitZone`; named rail points have `ReachPoint`.

### Instruction ("quad", 9 bytes)

`u8 opcode`, `u16 a`, `u16 b`, `u32 c`. A `u16` operand that references a variable carries a storage class in its
top two bits and a slot offset in the low 14 bits: `01` = class variable block (`cv<offset>`, offsets match the
variable table), `10` = function-local block (`lv`), `11` = temporary block (`tv`); `00` marks non-references
(jump targets, indices, zero). Three-operand arithmetic uses `a`, `b` and the low 16 bits of `c` (the high 16 bits
are then 0 in every instruction).

Opcodes present in retail data (208 679 instructions), with the operand layout established by observation:

| Opcode | Count | Operand layout (observed) | Established role | Desperados 1.0 name (unverified for 1.5) |
|---|---|---|---|---|
| 0x01 | 9171 | none | | NOP |
| 0x02 | 1653 | a = var | | PARAM (push argument) |
| 0x03 | 8176 | a, b = sizes | function prologue: `(size_of_volatile, size_of_tempor)` of the function table | InitFunction |
| 0x04 | 8176 | none | one per function | EndFunction |
| 0x05 | 951 | a = function address | call: every `a` is a function address of the same class | CALL |
| 0x06 | 11296 | none | | RETURN |
| 0x07 | 5160 | a = var (temp) | | RETURN value |
| 0x08 | 2595 | a = var (temp), c = 0/4/8/12/16 | | GETPARAM |
| 0x0a | 62 | a = var | | GETRETURN |
| 0x0b | 43965 | a = var | pushes preceding a native call; the number of pushes per native id is constant | NATIVEPARAM |
| 0x0c | 42734 | a = native id (0..=264, 192 distinct) | native (engine) call | NATIVECALL |
| 0x0d | 20079 | a = var (temp) | follows native calls | NATIVEGETRETURN |
| 0x0e | 5510 | a = quad index | unconditional jump (`a` < quad count; two exceptions with `a = 0xffff`) | GOTO (Desperados keeps the target in `c`) |
| 0x0f | 4601 | a = var (temp), c = quad index | conditional jump (`c` < quad count) | IF a != 0 GOTO |
| 0x11 | 983 | a, b = vars | | MOV |
| 0x12 | 10 | a (local), b = vars | | - |
| 0x13 | 37060 | a = var, c = integer immediate (0..=100000) | | MOV int immediate |
| 0x14 | 110 | a = var, c = f32 immediate (1.0, 2.0, 0.01 ...) | | MOV float immediate |
| 0x15 | 626 | a, b = vars (temp) | | NEG int |
| 0x18 | 327 | a, b = vars (temp) | | - |
| 0x19 - 0x1b | 238 / 247 / 212 | a, b, c16 = vars | three-operand | +I -I *I |
| 0x1d, 0x1e | 117 / 17 | a, b, c16 = vars | three-operand | +F -F |
| 0x22, 0x24 | 9 / 8 | a, b, c16 | three-operand | <I >=I |
| 0x25, 0x26 | 169 / 100 | a, b, c16 | three-operand | !=I ==I |
| 0x27, 0x28 | 42 / 30 | a, b, c16 | three-operand | <=F <F |
| 0x29 | 4244 | a, b, c16 | three-operand, the usual condition producer before 0x0f | !=F (implausible for its frequency) |
| 0x2b | 1 | a, b, c16 | three-operand | ==F |

Opcodes 0x00, 0x09, 0x10, 0x16, 0x17, 0x1c, 0x1f-0x21, 0x23, 0x2a, 0x2c do not occur. The Desperados column comes
from the GPL-3 OpenDeathValley disassembler (see Provenance) and is a hypothesis only: the opcode numbering may
have changed between 1.0 and 1.5 (0x29 being the dominant comparison suggests it did).

### Native calls

`0x0c` calls engine function number `a`. 192 distinct ids in 0..=264. The number of `0x0b` pushes before each call
is the same for every use of an id (arity 0..=6). The engine's function *names* are only present in the executable
as error strings (`IsActorPC`, `MakeNoise`, `GetDistance`, `ComputeLocationBetween`, `AreAllEnemiesInsideHS`,
`SetActorLocation`, ...), not as an ordered table, so ids cannot yet be named from data. The most frequent ids:
3 (9091 uses, 1 argument), 32 (3914, 0), 6 (2470, 1), 74 and 30/31 (about 1600 each, 0), 56 (1604, 1),
43 (949, 2), 111 (929, 0), 5 (896, 1), 113 (850, 1).

## Cross-references

- Class names == mission element names of the paired `.rhm` (100 % both ways, see [rhm.md](rhm.md)).
- A `StartUp` class comes first in every file ("a Level is missing its startup script" is fatal in the engine).

## Tools

`opensherwood-tools scb <file> [--class NAME] [--no-code]` prints classes, variables, the function table and the
raw disassembly (`enter`, `call`, `native`, `jump`, `jump_cond` for the established roles; `op_XX` with decoded
operands otherwise; raw fields are appended whenever the layout does not cover a non-zero field).

## Plan

Correlate opcodes with observable behaviour (oracle captures of the tutorial), name the native ids from arity and
usage context (e.g. which id is called with a `Location` variable after `EnterZone`), then specify the VM. The
community's Lua layer (Spellforge) replaces `.scb` with `.lua`, so a Lua API compatible with theirs is the modding
target; the SCB VM is needed only to run the retail campaign unchanged.

## Provenance

Observation: structural hypothesis tested over all 39 files (`harness/tools/re/scb_probe.py`,
`scb_opstats.py`: every file consumed exactly; opcode and operand histograms; prologue / call / jump target checks;
class-name join with the missions). Community knowledge: the OpenDeathValley project
(<https://github.com/OpenDeathValley/OpenDeathValley>, GPL-3.0, `components/files/odv_scb_handler.c` and
`odv_scb_disassembler.c`) documents the Desperados 1.0 text container (`fileName, className, nbOfVariables,
sizeOfVariables, nbOfFunctions, functionName address nbOfParams sizeOfRetVal sizeOfParams, sizeOfVolatile
sizeOfTempor, nbOfQuads`, 10-byte quads) and an opcode table its author marks as probably wrong; it was used as a
guide for field order and the operand storage-class bits, both re-verified on our data. No code was copied.
Executable knowledge: printable strings only (`docs/original/executable-notes.md`).
