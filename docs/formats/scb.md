# SCB compiled mission script (`.scb`, magic `SBSCRIPT`)

Status: **container decoded, control flow and calling convention established, opcode arithmetic and native
semantics hypothesised from data**. `crates/opensherwood-formats/src/scb.rs` parses all 39 retail files to the
last byte (classes, variables, function tables, instructions) and prints a raw disassembly. Everything in the
sections "Opcode hypotheses", "Native call table", "Index spaces" and "First mission walkthrough" is inferred from
the compiled scripts, the paired mission files and the observed behaviour of the first mission; each row carries a
confidence and its evidence. Nothing here comes from the executable beyond its printable strings.

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
`nbOfParams, sizeOfRetVal, sizeOfParams` by position; see "Calling convention" for what the data says.

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

Opcodes present in retail data (208 679 instructions), operand layout established by observation:

| Opcode | Count | Operand layout (observed) | Role (see "Opcode hypotheses" for confidence) | Desperados 1.0 name |
|---|---|---|---|---|
| 0x01 | 9171 | none | filler after 0x07 | NOP |
| 0x02 | 1653 | a = var | push argument of a script call | PARAM |
| 0x03 | 8176 | a, b = sizes | function prologue: `(size_of_volatile, size_of_tempor)` | InitFunction |
| 0x04 | 8176 | none | end of function (last instruction of every function) | EndFunction |
| 0x05 | 951 | a = function address | call function of the same class | CALL |
| 0x06 | 11296 | none | return | RETURN |
| 0x07 | 5160 | a = var (temp) | set return value | RETURN value |
| 0x08 | 2595 | a = var (temp), c = 0/4/8/12/16 | read parameter at byte offset c | GETPARAM |
| 0x0a | 62 | a = var | read return value of the preceding 0x05 | GETRETURN |
| 0x0b | 43965 | a = var | push native argument | NATIVEPARAM |
| 0x0c | 42734 | a = native id (0..=264, 192 distinct) | native (engine) call | NATIVECALL |
| 0x0d | 20079 | a = var (temp) | read native return value (always directly after 0x0c) | NATIVEGETRETURN |
| 0x0e | 5510 | a = quad index | unconditional jump (two exceptions with `a = 0xffff`) | GOTO (Desperados keeps the target in `c`) |
| 0x0f | 4601 | a = var (temp), c = quad index | jump if a is non-zero | IF a != 0 GOTO |
| 0x11 | 983 | a, b = vars | move | MOV |
| 0x12 | 10 | a (local), b = vars | move (float) | - |
| 0x13 | 37060 | a = var, c = integer immediate (0..=100000) | load int immediate | MOV int immediate |
| 0x14 | 110 | a = var, c = f32 immediate (1.0, 2.0, 0.01 ...) | load float immediate | MOV float immediate |
| 0x15 | 626 | a, b = vars (temp) | unary minus | NEG int |
| 0x18 | 327 | a, b = vars (temp) | int to float | - |
| 0x19 - 0x1b | 238 / 247 / 212 | a, b, c16 = vars | int add, subtract, multiply | +I -I *I |
| 0x1d, 0x1e | 117 / 17 | a, b, c16 = vars | bitwise or, and (hypothesis) | +F -F |
| 0x22, 0x24 | 9 / 8 | a, b, c16 | float multiply; comparison | <I >=I |
| 0x25, 0x26 | 169 / 100 | a, b, c16 | int `<`, `>=` (hypothesis) | !=I ==I |
| 0x27, 0x28 | 42 / 30 | a, b, c16 | int `>`; `!=` or `>=` (hypothesis) | <=F <F |
| 0x29 | 4244 | a, b, c16 | int / handle `==` | !=F |
| 0x2b | 1 | a, b, c16 | float comparison | ==F |

Opcodes 0x00, 0x09, 0x10, 0x16, 0x17, 0x1c, 0x1f-0x21, 0x23, 0x2a, 0x2c do not occur. The Desperados column comes
from the GPL-3 OpenDeathValley disassembler (see Provenance); the numbering clearly changed between 1.0 and 1.5
(0x22 is a float multiply here, 0x29 the dominant equality test), so that column is history only.

## Opcode hypotheses

Confidence: **high** = a structural invariant over the whole corpus that has no plausible alternative reading;
**medium** = consistent with every use found, but an alternative reading exists; **low** = a guess from a handful
of uses. Evidence cites the probes in `harness/tools/re/` (see Provenance).

| Opcode | Meaning | Confidence | Evidence |
|---|---|---|---|
| 0x01 | no-op | high | zero operands; 5081 of 5160 `0x07` are followed by one, otherwise it never carries information |
| 0x02 | push argument for the next 0x05 | high | number of 0x02 between calls equals the callee's parameter count (see Calling convention) |
| 0x05 | call function `a` of the same class; 0x0a afterwards reads its value | high | all 951 targets are function addresses; 0x0a occurs only directly after 0x05 (62/62) |
| 0x06 | return | high | last-but-one instruction of every function and early exits |
| 0x07 | set the return value from `a`; control continues (a 0x01 filler, then 0x06 or a jump) | high | `return_value 1` idiom in every default `ActivatedBy*` / `FilterAIEvent` handler; 5069 of 5160 read a temp loaded by 0x13 just before |
| 0x08 | `a = parameter at byte offset c` (parameter k at 4k) | high | offsets 0/4/8/12/16 only; a function reading offset 4 always has at least two parameters by the call-site count |
| 0x0f | `if (a != 0) goto c` - branch taken on **true** | high | 3784 of 4601 have the shape `0x0f cond -> L1; 0x0e -> L2; L1: body; L2:`, the form a compiler emits when its only conditional branch is "branch if true"; loops are `L: cond; 0x0f -> body; 0x0e -> exit; body ...; 0x0e -> L` (176 cases); run-once guards `flag == 0 -> body sets flag = 1` |
| 0x11 | `a = b` (int or handle) | high | copies of native results into typed class variables (`MonActeur:Actor = n111()`), locals, temps |
| 0x12 | `a = b` for a float value | medium | 10 uses; 9 store the result of 0x22 into a local, 1 copies a local |
| 0x13 / 0x14 | load int / float immediate | high | `c` is a plausible int in all 37060 / a plausible float in all 110 (1.0, 2.0, 0.5, 0.01, 30.0, 10.0) |
| 0x15 | `a = -b` | high | 626 uses, the operand is always a temp holding immediate 1; the result is passed to natives 3 and 6 (an element / location index of -1 = "none"; see the `ejectage` helper which teleports to `n6(-1)`) |
| 0x18 | `a = float(b)` | medium | operand is always an int (immediate 30, 10, or `random(100)`); result feeds 0x22 (float multiply) and native 224's float parameters |
| 0x19 / 0x1a | int add / subtract | high | `i = i + 1` loop steps (190 of 238 adds use immediate 1), `time - last`, `count - 1` |
| 0x1b | int multiply | medium | `x * 25` feeding native 56 (108 of 212 uses) and `x * 1`, `x * 2` |
| 0x1d | bitwise or | medium | 88 of 117 have an immediate operand and *all* immediates are powers of two (1..=8192); chained (`(2 op 4)`, `(a op b) op c`); results are passed as flag arguments (natives 196, 224) |
| 0x1e | bitwise and | medium | 17 uses; `(param op 1) > 0`, `(param op 2) > 0`, ... (mask tests of a flags parameter), `(n195(11) op param2)` |
| 0x22 | float multiply | medium | all 9 uses are `local = op12(0.01f op22 float(n161(100)))`: a random float in 0..1 |
| 0x24 | integer comparison (8 uses: `n2(0) op 2`, `distance op 80`, `money op 2000`, `phase op 3`) | low | direction not determinable from data; Desperados names it `>=` |
| 0x25 | `a = b < c` | medium | ascending loops `i op25 n75()` / `i op25 n216()` (over 100 copies of the same helper), `distance op25 radius` (75, 50, 10, 150), `arrows + 1 op25 initial`, `money op25 100000` |
| 0x26 | `a = b >= c` | medium | descending loops `i = n75() - 1; while (i op26 0) { ...; i = i - 1 }` (90 of 100 uses compare with 0), `random op26 threshold`; `!=` would make the loops skip index 0 |
| 0x27 | `a = b > c` | medium | `elapsed op27 15` then the timer is reset, `PC count op27 0`, `(flags & bit) op27 0`, `distance op27 distance`, `time op27 timer` |
| 0x28 | `a = b != c` or `>=` | low | `money(knight) op28 0` gates an objective that is completed when `money == 0` (so not `<=`); `state op28 2`, `flag op28 1`, `bool op28 bool` |
| 0x29 | `a = b == c` (int or handle) | high | 4230 of 4244 compare with an immediate loaded just before; message dispatch `param0 == id` in every `ProcessMessage`, `flag == 0` guards, `IsPC(actor) == 1` |
| 0x2b | comparison of two float locals (1 use) | low | operands are locals written by 0x12 |

Not decided by data: whether 0x0f treats the value as int or float (the sources are 0x25-0x29 results, native
results and immediates, all ints), the meaning of the two `0x0e` with `a = 0xffff`, and the unused opcodes.

## Calling convention

Established over all 951 script calls and 2595 parameter reads (`scb_semantics.py --ops`, `--params`):

- The caller pushes arguments with 0x02 in order, then 0x05. `unknown_2` of the callee equals
  `4 x pushes + unknown_1` in every combination seen (`(pushes, unknown_0, unknown_1, unknown_2)` =
  (0,2,0,0), (0,2,4,4), (1,2,0,4), (1,2,4,8), (2,3,0,8), (3,4,0,12), (3,4,4,16), (4,5,0,16), (5,6,0,20)). So
  `unknown_1` is the return-value size (0 or 4) and `unknown_2` the size of the parameter block *including* the
  return slot, which sits after the parameters. `unknown_0` is `max(2, pushes + 1)`: a frame slot count rather
  than the parameter count.
- Parameter k is read with `0x08 tv, c = 4k`. A function with `unknown_1 = 4` sets its value with 0x07 and the
  caller reads it with 0x0a (or the engine reads it: `CheckVictoryCondition`, `FilterAIEvent`, `ActivatedBy*`).
- Temporaries (`tv`) hold at most one live value between a producer and its consumer; the folded listing of
  `scb_semantics.py --pseudo` substitutes them safely.

Callback signatures observed (parameter reads and what they are compared with):

| Callback | (unknown_0, unknown_1, unknown_2) | Parameters used | Observed meaning |
|---|---|---|---|
| `Initialize` | (2,0,0) elements; (2,4,4) / (2,4,8) level and some elements | none | returns 0/1 in the level class |
| `PostInitialize` | (2,0,0) | none | level only; runs the briefing sequence (H01) |
| `Hourglass` | (2,0,4) elements, (2,4,8) level | param0 = a time value: compared with stored values (`time - last > 15`, `time > timer`) | periodic tick |
| `CheckVictoryCondition` | (2,4,8) | none; returns a mission variable | level only |
| `ProcessMessage` | (4,0,12) | param0 = message id (compared with immediates), param1 = argument (element index, flag), param2 rarely | message dispatch |
| `ActionChange` | (3,0,8) | param0 or param1 compared with action ids 137 (most), 141, 136, 107, 102, 135, 280, 281 | actor changed action state |
| `HandleEvent` | (3,0,8) / (3,0,4) | param1 == 31 (2 uses) | almost always empty |
| `FilterAIEvent` | (3,4,12) | param1 compared with event ids 0, 2, 52, 22, 13, 14, 31, 8, 23, 11, 33, 34; returns 1 by default | AI event filter |
| `ActivatedBy*` | (2,4,8) | param0 read in 7 `ActivatedByArrow` (the shooter: passed to native 79 = is-PC) ; returns 1 (rarely 0) | object interaction |
| `EnterZone` / `ExitZone` | (2,4,8) | param0 = the actor (269 of 312 `EnterZone` test `n79(param0)` first) | script polygon events |
| `IsTaken` | (2,4,8) | param0 = the actor taking the scroll (camera returns to `n95(param0)`) | scroll picked up |
| `ReachPoint` | (2,4,8) | param0 = the actor reaching the named rail point | rail event |
| `Finalize` | (2,0,4) | param0 == 1 (17 files) | mission end (won?) |

## Native call table

`0x0c` calls engine function `a`; the number of 0x0b pushes per id is constant (arity), and whether the result is
read (0x0d) is constant per id as well. 192 ids occur. The table names the ids that matter for the first mission
and the most frequent ones; names are **hypotheses** chosen to describe the observed effect, not engine names
(the executable's function names - `IsActorPC`, `MakeNoise`, `GetDistance`, `ComputeLocationBetween`,
`AreAllEnemiesInsideHS`, `SetActorLocation`, ... - are only visible as error strings, without ids). Evidence
abbreviations: H01 = the first-mission walkthrough below; helper = the name of a designer helper function that
consists mostly of the call (`scb_semantics.py --handlers`); flow = argument / result data flow over the corpus
(`--natives`); bound = the immediate range matches a table of the mission file (`scb_xref.py`).

| Id | Arity, result | Hypothesised effect | Confidence | Evidence |
|---|---|---|---|---|
| 0 | (k, v) | declare mission variable k with initial value v | medium | level `Initialize` and helper `InitMissionVars`; always followed later by 1 / 2 on the same k |
| 1 | (k, v) | set mission variable k | high | 192 uses in event handlers; k in 0..=40 plus designer codes (1004, 7777); v 0..=7 |
| 2 | (k) -> int | get mission variable k | high | 171 of 201 results go into `==` tests; `CheckVictoryCondition` returns it (H01) |
| 3 | (index) -> element | element by index in the level's flat element table (see Index spaces); -1 allowed | high | 9091 uses, result is the first argument of almost every element native; loops `for i < n75(): n3(i)` |
| 4 | (index) -> door | door / passage of the map (per-map index range: Lincoln <= 52, Nottingham <= 94, Derby <= 29, Leicester <= 55, York <= 122) | medium | helper `InitializeDoors`, `LockDoor`; used only with 186-189 and 191 |
| 5 | (index) -> patch | map patch (visual alteration of the background) by per-map index (Lincoln <= 11 in all three Lincoln missions, Leicester <= 15, ...) | medium | helper `InitPatches`, `RefreshPatches`; H01 drawbridge; only used with 144 / 145 |
| 6 | (index) -> location | location of the mission's `GULP` chunk: points first (0..points-1), then polygons | high | bound exact in 11 files (max index = points + polygons - 1), H01 `persecutionZone` (last polygon) is `n6(26)` = 15 points + 11 |
| 9 | (index) -> path | patrol path (`RAIL` index) | high | max index < rail count in all 26 files that use it (exact in 3); consumed by 132 |
| 10 | (element) -> index | index of an element (inverse of 3) | medium | passed as the argument of messages whose handler does `n3(param1)` (H01 msg 9; tutorial soldiers `n44(n111(), 1, n10(n74()), ...)`); native 59 takes it |
| 26 | (k, main) | add objective k = short-briefing text k; main = 1 for a primary objective, 0 for a secondary one | high | H01: `n26(0, 1)` at start = TEXT 1000283 string 0 (the observed initial objective), 1..5 added exactly when the campaign notes say those objectives appear; k < short-briefing count of the `.red` in every file |
| 27 | (k) | objective k accomplished | high | H01: `n27(0)` when objective 1 is added, `n27(3)` / `n27(4)` when the steward / knight sub-goals complete |
| 28 | (k) | select debriefing / ending variant k | medium | H01 `CheckVictoryCondition`: `if n2(2) == 1: n28(0)`; ambushes choose `n28(2)` or `n28(variable)` on a campaign flag; `Finalize(1) -> n28(1)`; k < debriefing count of the `.red` |
| 30 / 31 | () | begin / end a sequence (script-driven cutscene or timed action list) | high | balanced in all 8176 functions, never nested; exe class names `RHSequence`, `RHSequenceElement*` |
| 32 | () | sequence step: wait for the previous element to finish | high | 3914 uses, only between 30 and 31, after every element that takes time (text page, camera move, wait, animation) |
| 33 | (location) | camera moves to location (sequence element) | medium | inside sequences, argument from 6 or 95; followed by 32 |
| 34 | (location) | camera returns / jumps to location (last element of every cutscene: `n34(n95(actor))`) | medium | H01 briefing end (observed: camera on Robin after the parchment), end of all tutorial popups |
| 35 | (float) | sequence element with a duration or rate (1.0 in 79 of 85 uses; 2.0; 0.5) at the start of cutscenes | low | always `n30(); n35(1f); n32(); n54()` |
| 43 | (target, msg) | send message msg to target's `ProcessMessage` | high | H01: the archer sends msg 1 to the sergeant, whose class handles msg 1; every message id sent is handled by some class of the same file |
| 44 | (target, msg, arg, x) | send message with an argument (param1 of the handler); x in 0..=6 unknown (delay?) | high (first three) | H01 msg 9 with `n10(element)` -> handler uses `n3(param1)`; msg 13 with 1 / 0 = freeze / unfreeze NPCs |
| 45 | (actor, location, mode) | move actor to location, mode 0..=2 | medium | helper `SendToDeploymentZone`; sequence element; H01 son moves to a point |
| 48 | (actor, location) | move actor to location (sequence element) | medium | helper `RunToAlertPath`?; sergeant walks to the archer's location `n48(sergeant, n95(archer))` |
| 49 / 50 / 51 | (actor, anim) | play animation anim (51 in 418 `ActivatedByArrow`: target hit animation 210; 51 with 0 resets; 49 with 216 on the shouting sergeant) | medium | ranges 3..=270 in three natives; sequence elements |
| 52 | (actor) | sequence element on an actor (wait for it?) | low | H01 msg 9 |
| 53 | (actor) | actor-level action (ReachPoint 36, ActivatedByArrow 128 uses) | low | |
| 54 / 55 | () | enter / leave cutscene presentation (interface hidden, NPCs frozen by msg 13 around it) | medium-low | present in every popup sequence; not strictly paired per function |
| 56 | (ticks) | wait (sequence element); 25 ticks per second is the hypothesis | high | 1604 uses; immediates 10, 15, 25, 40, ...; `seconds * 25` via 0x1b in 108 uses |
| 59 | (archer, 4, target index) | archer shoots at target | low | helper `ArcherShoot`; H01 archery training |
| 64 | (actor, location, 0) | place / send actor at location | low | H01 msg 9 |
| 69 | (actor, id) | actor performs remark / gesture id (2..=96) before its dialogue text | low | sergeant before text 5, the persecuted one before text 3 |
| 74 | () -> actor | the actor this class belongs to ("self") | high | 1606 of 1622 uses in element `ProcessMessage`; fed to movement / AI natives |
| 75 | () -> int | number of elements (loop bound for 3) | high | `for i < n75(): n3(i)` in every helper that scans actors |
| 79 | (actor) -> bool | is a player character | high | gate of 269 `EnterZone` and of the target `ActivatedByArrow`; matches exe string `IsActorPC` |
| 80 / 81 | (actor) -> bool | actor kind predicates (80: NPC?; 81: soldier?) | low | helpers `LockAllSoldiersAI` (81), `UnBlipAllNPCsInside` (80) |
| 85 | (actor) -> bool | actor is unusable (dead / removed): helpers skip actors with `n85 == 1` | medium | `ActivateAllPCs`: `if n85(pc) == 0: n114(pc)`; `KillActorsInZone` skips them |
| 87, 88, 89, 90 | (actor) -> bool | status predicates or-ed by helper `IsActorNeutralized`; 90 alone means "soldier out of action" in H01 | medium | H01 waits for `n90 == 1` on the courtyard lancers |
| 95 | (actor) -> location | location of an actor | high | fed by 3 / 211, consumed by 33 / 34 / 48 / 160 |
| 96 | (actor, location) | set actor location (teleport; `n6(-1)` = off map) | medium | helper `ejectage`, `PutOutOfMap`; exe string `SetActorLocation` |
| 97 | (actor, zone) -> bool | actor is inside zone | medium | helper `IsPCSafe`, `KillActorsInZone` (loops over actors with the zone parameter) |
| 99 | (actor) | reveal actor (un-blip) | low | helpers `UnBlipAllNPCsInside`, `UnblipMarkedNPC` |
| 102 | (actor, 10, 1) | inflict damage / kill | low | helper `tuage` |
| 109 | (target, msg) | send message (second entry point; used from zones, scrolls, `ActionChange`) | high (delivers a message) | H01: `n109(archer, 1)` every 15 ticks drives the archer's msg-1 handler |
| 110 | (target, msg, a, b) | send message with arguments (variant of 44) | low | 21 uses |
| 111 | () -> actor | the player's character in the current context (H01: Robin); messages addressed to it reach the level script | medium | stored into `MonActeur:Actor`; `n34(n95(...))`-style camera code uses param0 instead; all messages sent to it in H01 and the tutorial are handled by `StartUp.ProcessMessage` (Robin has no class in H01) |
| 113 / 114 | (element) | deactivate / activate an element (hidden actors, scrolls that appear later) | high | helpers `DeactivateAllPCs` (113) / `ActivateAllPCs` (114); H01 tutorial scrolls activate each other |
| 117 / 118 | (element, attr, value) / (element, attr) -> value | set / get an element attribute (attr 1 on the knight = his purse / money; attr 0 on an archer incremented by the sergeant) | medium | H01 objective 4 completes when `n118(knight, 1) == 0` |
| 132 | (actor, path) | assign patrol path (actor follows the `RAIL`) | high | 949 uses with `n9`; helpers `NewPost`, `SwapPath`, `AlertReserveSoldier` |
| 134 / 135 | (actor, flag) / (actor) | lock / unlock the actor's AI | medium | helpers `LockAIEverybody` / `UnlockAIEverybody`, `LockNPCAI`; msg 13 freeze in H01 |
| 140 | (actor, 0/1) | patrol mode flag (run?) set right after 132 | low | |
| 144 / 145 | (patch) -> bool / (patch) | patch is active / activate patch (also used as one-shot flags) | medium | H01 drawbridge: rope cut -> `n145(n5(4)); n145(n5(3))`; zones: `if n144(n5(k)) == 0: n145(n5(k)); ...` |
| 159 | () -> location | off-map location | low | helper `PutOutOfMap` |
| 160 | (location, location) -> distance | distance between locations | high | compared with radii 10..=150; exe string `GetDistance` |
| 161 | (n) -> int | random number in 0..n | medium | `0.01 * float(n161(100))`, thresholds |
| 186 - 189 | (door, 1) | lock / unlock door variants | low | helper `LockDoor` calls 186, 187, 188, 189 |
| 191 | (state, door) | open / close door (0 at level start, 1 when the player reaches the area) | low | H01 `InitializeDoors` pattern |
| 193 / 194 | (element) -> state / (element, state) | get / set an element state 0..=3 (bit 1 toggled with `+ 2`, or set to 3) on scrolls and zones | low | H01 msg 7, `Hourglass` |
| 195 | (k) -> int | campaign / progress flag k (0..=11) | low | ambush debriefing choice `if n195(6) == 1` |
| 196 | (k, flags) | availability of player action / skill k (0..=11) | low | H01 `Initialize` disables 6, 7, 8, 9, 3; tutorials enable with or-ed flags |
| 197 / 198 | (actor, k) -> flag / (actor, a, b) | actor visibility / blip flags | low | helper `MarkToUnBlip` (198), `UnblipMarkedNPC` (197, 198) |
| 202 | (k) | show text k of the level's text list immediately (dialogue line / hint) | high | k < text count of the matching `.red` in all 30 files; H01 hints 13, 14, 15, 17, 19, 21 appear in the handlers of the matching tutorial scrolls |
| 203 | (k) | show text k as a sequence element (parchment page; waits for dismissal) | high | H01 `PostInitialize` shows 0, 1, 2 = the three observed briefing pages; popups 3..=22 inside cutscenes |
| 204 | (zone) -> int | presence / count of (player) actors in zone | low | H01 `persecutionZone.ExitZone`, helper `IsEveryEnemyInZoneHS` |
| 211 | () -> actor | the main player character | medium | `n34(n95(n211()))` = camera on Robin after the briefing (observed) |
| 216 / 217 | () -> int / (i) -> actor | number of player characters / player character i | high | helper `ActivateAllPCs` loops `for i < n216(): n217(i)` |
| 218 | (leader, member) | group members under a leader | low | H01 sergeant collects the archers after training |
| 224 | (location, 30.0, 10.0, flags) | set a trap at location | low | helpers `SetTrap`, `AddRepulsiveZoneForHole` |
| 233 | (actor, element) | actor goes to / addresses element (an actor, a scroll, a zone) | medium | H01: the servant goes to Robin, the son goes to a scroll position, the persecuted one goes to a zone |
| 235 | (element) -> bool | element taken / used (purse object) | low | H01 steward objective |
| 243 | (actor) | highlight actor during a cutscene | low | 340 uses in `IsTaken`, always inside a sequence on the actor the text talks about |

Message ids are designer-defined per mission (1.., 100.., 1000.., 2987..3017, 4000.., 5000..); a handler compares
`param0` with immediates and every id that is compared is sent by some 43 / 44 / 109 / 110 call of the same file
in all but a few files (`scb_semantics.py --messages`), so the engine itself sends few or none of them.

## Index spaces

- **Elements (native 3)**: one flat table per level. From self-references of object classes (a target plays
  its own hit animation on `n3(k)`; `scb_elements.py`), the mission part of the table is, in `.rhm` order,
  `SCOT` (player characters), `OILE`, `TOTO`, `BORG`, `BOOM`, `SKRO`, then further entries (script polygons or
  the `CAVE` list; not resolved). The first mission index of a file is `base = POUF count + K(map)` where K is
  14 for Croisement01, 19 for Croisement02 and Croisement03 (20 forest missions, exact), 19 for Derby, 58 for
  Nottingham, 59 for Leicester, 49 for Lincoln (H01), about 67 for York. The map's `FLIM` animated elements
  occupy indices 0..FLIM-1 (confirmed in H01: indices 19..=32 are Lincoln's chimney fires, torches and candles,
  which the day mission deactivates at start and re-activates room by room); K exceeds the `FLIM` count by 1, 4,
  4, 6, 10, 12, 11 respectively, so a small proto-level table that is not identified follows the animated
  elements. `sherwood.scb` (50 PCs) does not fit this model (max index 50). For H01: map elements 0..=37,
  unresolved 38..=48, PC = 49, civilians 50..=56, soldiers 57..=94, objects 95..=99, scrolls 100..=114,
  115..=126 unresolved (script polygons or the `CAVE` list).
- **Locations (native 6)**: `GULP` points then polygons (exact bound in 11 files).
- **Paths (native 9)**: `RAIL` index.
- **Doors (native 4) and patches (native 5)**: per-map tables of the proto-level (index ranges are consistent
  across the missions of one map).
- **Texts (natives 202 / 203)**: index into the level's text list (`.red` layout in `campaign-flow.md`: the
  `count, list id` pair). The mapping mission file -> `RHLevel??.red` used by `scb_xref.py` is the campaign-flow
  order and holds for every file except `EmbTut_FoC_EC`, whose text indices (<= 7) exceed `RHLevelET.red`
  (3 texts): the only E/T index with enough texts is `RHLevelEZ.red` (8), so the tutorial ambush is probably EZ.
- **Objectives (natives 26 / 27)**: index into the level's short-briefing list (`.red` last pair).
- **Debriefings (native 28)**: index into the won / lost debriefing lists.

## First mission script walkthrough (`H01_Lin_VL.scb`, 47 classes)

Read with `scb_semantics.py <file> --pseudo`. Element indices are given with the names of the `.rhm` records
they resolve to under the table above; texts are referred to by their index in TEXT 1000105 (see
`docs/original/campaign-flow.md` for what each says) and objectives by their index in TEXT 1000283.

**Level class `StartUp`** (8 variables: a shooting timer, a run-once flag, two "sub-goal done" flags, a loop
counter and bound, an `Actor` scratch variable, a "going away" flag).

- `Initialize`: disables five player actions (`n196(6..9, 0)`, `n196(3, 0)`); deactivates map elements
  19..=32 - in `lincoln.rhp` these `FLIM` entries are the two chimney fires, five torches and seven candles,
  i.e. the interior lights of a day mission (the room-entry messages below switch them on again room by room) -
  and several scrolls (102, 104..=106, 108..=110 = the scrolls that appear only after earlier steps); zeroes the
  variables; stores the player character (`n111()`); declares mission variables 1, 2, 3; locks doors 20 and 28
  and closes doors 8, 20, 21, 37, 25, 23; deactivates, AI-locks and hides four actors (50, 79, 53, 92: the
  girl, a soldier, Edward the servant, a soldier) that are activated by later events; sets two attributes of
  element 126 to 0; returns 0.
- `PostInitialize`: adds the primary objective 0, then one sequence: text pages 0, 1, 2 (the three observed
  briefing pages), then the camera goes to the main PC (`n34(n95(n211()))`). This matches the observed
  Play! -> briefing -> camera-on-Robin flow exactly.
- `Hourglass(time)`: every 15 ticks while mission variable 1 (training over) is 0, sends message 1 to the
  first archer (element 70), which runs one shot of the training loop. When soldier 87 is out of action, once:
  soldier 87 and civilian 51 (the girl) get patrol paths 65 / 58 and the girl runs (msg to 140). When mission
  variable 3 is 0 and the four soldiers 81..=84 are all out of action, message 1 goes to the persecuted one
  (52). Steward sub-goal: when the steward's purse (105) is taken, objective 3 is accomplished and scroll 120's
  state gets bit 1. Knight sub-goal: when the knight (78) has no money (`n118(78, 1) == 0`), objective 4 is
  accomplished. Exit: when any of the courtyard lancers 75..=77 is out of action, soldier 56 gets path 57.
- `CheckVictoryCondition`: returns mission variable 2 (set when Robin tells the servant's son he wants to
  leave); selects debriefing 0 first.
- `ProcessMessage(msg, arg)`: 1 = a target was hit (training over: objective 5 accomplished, variable 1 = 1,
  the sergeant groups the six archers and everybody walks to the mess along paths 66..=71 / 38, the sergeant
  speaks text 6); 2 = same without the sergeant's line (an archer changed action 141); 3 / 5 / 6 / 7 = area
  entered the first time (patch flag 1 / 10, 11 / 8 / 7): activate the actors, light effects and scrolls of that
  area, open its doors, give paths; 8 / 9 = give element `arg` a path (9: also reveal it and play a sequence); 10 = the
  persecuted one goes to the servant's zone; 11 = shift the shooting timer; 12 = the son goes to location 9
  then to scroll 114; 13 = freeze (`arg = 1`) or unfreeze all NPCs (loop over all elements with 80 / 134 /
  135 / 197); 14 = the servant's cutscene (texts 8, 9, 10, camera to location 9, the son moves, message 12,
  camera back); 15 = after the drawbridge rope is cut, `n137(location 14, 1)`.
- `Finalize`: empty.

**Actor classes** (archers, sergeant, target soldiers, lancers, `BouclierXT01`, `PoorWeepingOne`): `Initialize`,
`HandleEvent`, `FilterAIEvent` (returns 1) are stubs. `ActionChange(_, 141)` sends message 2 to the level.
`Acher01.ProcessMessage(1)`: sequence - message 1 to the sergeant (he walks to location 1), wait 12, shoot at
target 96 (`n59`), reset the target animation, wait 12, message 2 to the sergeant (he increments attribute 0 of
the archer, walks to him, plays animation 216). `PoorWeepingOne.ProcessMessage(1)`: the four soldiers get paths
25..=28, variable 3 = 1, then a sequence moving her to locations 7 and 8 and message 10 to the level.
`BouclierXT01.ActionChange(_, 141)`: gives soldier 87 and the girl their paths.

**Object classes** (`Cible1..4`, the drawbridge mechanism): all `ActivatedBy*` return 1. `ActivatedByArrow`
of a target: if not yet hit and the shooter is a PC, mark hit, sequence - play animation 210 on itself, wait 10,
message 1 to the level. `ActivatedBySword` of the mechanism: once - animation 160 on itself, message 15 to the
level, activate patches 4 and 3 (the lowered drawbridge).

**Rail point classes**: `OfficierdesArchers__6`: a sequence sending message 9 with each archer's index every
12 ticks (the group leaves one by one). `GirlWay01/02.ReachPoint(actor)`: hand over paths between the girl and
the soldier following her, then `n130(soldier, girl, 1)`.

**Scroll classes** (`IsTaken(actor)`, return 1 or 0): `ParchArcherDebut`: if training not over - secondary
objective 5, cutscene (freeze via message 13, camera to the sergeant, remark 61, texts 5 and 22, message 11,
unfreeze). `ParchArgent`: if training over show text 19, activate scroll 108. `ParchEdwardSpeak`: primary
objective 1, objective 0 accomplished, reveal the son, message 14 to the level, the servant goes to Robin.
`ParchFilsEdward`: primary objective 2, objective 1 accomplished, text 11, mission variable 2 = 1 (victory),
the son goes to Robin. `TutGrimper`: cutscene - camera to location 11, text 12. `TutJump`: text 13. `TutKO`:
texts 14 and 21. `TutBeggar`: cutscene - camera to location 0, highlight the beggar (55), text 16.
`TutGetObject`: text 15, first time also text 17 and activate scroll 106. `BeggarWorman`: if the steward's
purse is not taken - secondary objective 3, cutscene (camera to location 3, message 3, highlight 79 and 50,
text 4). `BeggarHaldric`: if the knight still has money - secondary objective 4, cutscene (text 7, camera to
the knight, highlight him). `Beggarpatch01`: first time activate patch 5 and element 99 (the mechanism);
cutscene with text 20. `TutOpenSalle`: unless patch 7 is active - cutscene (camera to location 12, message 7).
`ArrowsAtEnd`: activate scroll 102. `PoorManScroll`: activate scroll 110, the persecuted one goes to Robin,
remark 86 and text 3.

**Zone classes** (`EnterZone(actor)`): all first test `n79(actor) == 1` (a PC). `ZP06Sud/Est`: message 6 while
patch 8 is inactive; `ZP03TourOuest`: message 3 while patch 1 is inactive; `ZP01TourPtLevis`: activate patch 5
and the mechanism, set state 3 on element 122; `ZP04CentralBas`: patch 6, activate soldier 23, doors 37 / 6;
`ZP07Bas/Haut`: message 7 while patch 7 inactive; `ZP05GdSalle`: message 5 while patch 11 inactive;
`ZP09EscalierBas/Haut`: patch 10, activate 24 and 25; `EdwardZone`: first time - `n103(actor)`, cutscene with
camera to location 4 and text 18 (the servant's introduction); `persecutionZone.ExitZone`: if PCs are in zone
26 and variable 3 is 0 - variable 3 = 1, message 1 to the persecuted one.

What a first VM needs for this mission: the calling convention above; natives 0-3, 6, 9, 10, 26-28, 30-35,
43-45, 48-52, 56, 59, 64, 69, 74, 75, 79, 80, 85, 90, 95, 96, 99, 103, 109, 111, 113, 114, 117, 118, 130,
132-135, 137, 140, 144, 145, 186, 191, 193, 194, 196, 197, 198, 202-204, 211, 216-218, 233, 235, 243, 4 and
5 as at least stubs; message delivery to classes by element; the periodic `Hourglass`; the sequence model
(30 / 32 / 31 with blocking elements).

## Cross-references

- Class names == mission element names of the paired `.rhm` (100 % both ways, see [rhm.md](rhm.md)).
- A `StartUp` class comes first in every file ("a Level is missing its startup script" is fatal in the engine).
- Native argument ranges against `.rhm` chunk counts and `.red` text counts: see Index spaces.

## Tools

`opensherwood-tools scb <file> [--class NAME] [--no-code]` prints classes, variables, the function table and the
raw disassembly (`enter`, `call`, `native`, `jump`, `jump_cond` for the established roles; `op_XX` with decoded
operands otherwise; raw fields are appended whenever the layout does not cover a non-zero field).

Probes (`harness/tools/re/`, observation only, no game bytes embedded): `scb_probe.py` (container walk,
histograms), `scb_opstats.py` (per-opcode operand statistics), `scb_semantics.py` (`--ops` opcode contexts and
jump shapes, `--natives` argument / result data flow per native id, `--imm` immediate ranges per file,
`--handlers` natives per callback, `--params` callback parameter usage, `--messages` message ids sent versus
handled, `--find` opcode contexts, `--pseudo` folded expression listing of one file), `scb_xref.py` (native
immediate ranges against `.rhm` counts and `.red` text counts), `scb_elements.py` (element index bases from
object self-references).

## Plan

Verify the hypotheses with the original as an oracle (console `LEVEL TEXT`, the tutorial mission): which text
appears when a given scroll is taken, tick rate of `Hourglass` and native 56, the direction of 0x24 / 0x28, the
unresolved element-table base. Then specify the VM (`docs/architecture.md` script section) and the native
interface for the first mission. The community's Lua layer (Spellforge) replaces `.scb` with `.lua`, so a Lua API
compatible with theirs is the modding target; the SCB VM is needed only to run the retail campaign unchanged.

## Provenance

Container and operand encoding: structural hypothesis tested over all 39 files (`harness/tools/re/scb_probe.py`,
`scb_opstats.py`: every file consumed exactly; opcode and operand histograms; prologue / call / jump target checks;
class-name join with the missions). Community knowledge: the OpenDeathValley project
(<https://github.com/OpenDeathValley/OpenDeathValley>, GPL-3.0, `components/files/odv_scb_handler.c` and
`odv_scb_disassembler.c`) documents the Desperados 1.0 text container (`fileName, className, nbOfVariables,
sizeOfVariables, nbOfFunctions, functionName address nbOfParams sizeOfRetVal sizeOfParams, sizeOfVolatile
sizeOfTempor, nbOfQuads`, 10-byte quads) and an opcode table its author marks as probably wrong; it was used as a
guide for field order and the operand storage-class bits, both re-verified on our data. No code was copied.
Executable knowledge: printable strings only (`docs/original/executable-notes.md`).

Opcode hypotheses, calling convention, native table, index spaces, walkthrough (2026-09-02, analyst session,
data files only - no executable, disassembly or debugger involved): corpus statistics and data-flow analysis over
the 39 retail scripts (`scb_semantics.py --ops/--natives/--imm/--handlers/--params/--messages/--find`), cross
references against the `rhm` tool output of the paired mission files and the `RHLevel??.red` u32 lists
(`scb_xref.py`, `scb_elements.py`), the `rhp` tool output of the nine maps, and the observed behaviour of the
first mission (`docs/original/campaign-flow.md`: briefing pages, initial objective, the list of popup texts and
objectives of TEXT 1000105 / 1000283). Commands: `python harness/tools/re/scb_semantics.py <gamedir>/DATA/Levels
--ops`, `... --natives`, `... <gamedir>/DATA/Levels/H01_Lin_VL.scb --pseudo`, `python harness/tools/re/scb_xref.py
<gamedir>/DATA/Levels <gamedir>/DATA/Text target/release/opensherwood-tools.exe`, `python
harness/tools/re/scb_elements.py <gamedir>/DATA/Levels target/release/opensherwood-tools.exe`. Game build:
GOG English, executable SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`; data copy
`C:\Users\przem\source\gamedata\robinhood`. Confidence is stated per row; nothing in these sections is `observed`
in the ADR-0003 sense except the counts and the briefing / objective correspondence.
