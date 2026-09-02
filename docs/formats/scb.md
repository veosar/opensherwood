# SCB compiled mission script (`.scb`, magic `SBSCRIPT`)

Status: **container decoded, control flow and calling convention established, opcode arithmetic and native
semantics hypothesised from data**. `crates/opensherwood-formats/src/scb.rs` parses all 39 retail files to the
last byte (classes, variables, function tables, instructions) and prints a raw disassembly. Everything in the
sections "Opcode hypotheses", "Native call table", "Index spaces" and "First mission walkthrough" is inferred from
the compiled scripts, the paired mission files and the observed behaviour of the first mission; each row carries a
confidence and its evidence. Nothing here comes from the executable beyond its printable strings.

The original toolchain compiled a text script (`script.scs`, not shipped) into this bytecode. The executable's
version-check message for `.scb` files formats the expected and found versions as floats (paraphrased), which
confirms a float version field. The same VM family
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
| source_path | pstring32 | the source script's path on the designer's machine (a temporary-directory path ending in `script.scs`; the same string in every class of a file; not reproduced) |
| name | pstring32 | `StartUp` for the first (level) class (the engine looks the level class up by this name, see Cross-references); otherwise the name of a mission element: `<designer label>_<8 hex digits>` for actors, objects, scrolls and script polygons, `<label>__<n>___<8 hex digits>` for named rail points (see [rhm.md](rhm.md)) |
| variable_count | u32 | 0..=8 |
| size_of_variables | u32 | 4 x variable_count in every file |
| variables | variable[] | see below |
| function_count | u32 | 1..=17 |
| functions | function[] | see below |
| quad_count | u32 | 20..=2336 |
| quads | 9 x quad_count bytes | instructions |

Variable: `u8 type_tag` (2 = plain 4-byte value: designer-named counters, flags and timers; 7 = object
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
`EnterZone`, `ExitZone`) plus designer-written helper functions in the level class (French and English names;
not reproduced, referred to below by what their names say they do). Every class has `Initialize`; actor classes have `ActionChange`, `HandleEvent`,
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
of uses. Evidence cites the probes in `harness/tools/probe/` (see Provenance).

| Opcode | Meaning | Confidence | Evidence |
|---|---|---|---|
| 0x01 | no-op | high | zero operands; 5081 of 5160 `0x07` are followed by one, otherwise it never carries information |
| 0x02 | push argument for the next 0x05 | high | number of 0x02 between calls equals the callee's parameter count (see Calling convention) |
| 0x05 | call function `a` of the same class; 0x0a afterwards reads its value | high | all 951 targets are function addresses; 0x0a occurs only directly after 0x05 (62/62) |
| 0x06 | return | high | last-but-one instruction of every function and early exits |
| 0x07 | set the return value from `a`; control continues (a 0x01 filler, then 0x06 or a jump) | high | `return_value 1` idiom in every default `ActivatedBy*` / `FilterAIEvent` handler; 5069 of 5160 read a temp loaded by 0x13 just before |
| 0x08 | `a = parameter at byte offset c` (parameter k at 4k) | high | offsets 0/4/8/12/16 only; a function reading offset 4 always has at least two parameters by the call-site count |
| 0x0f | `if (a != 0) goto c` - branch taken on **true** | high | 3784 of 4601 have the shape `0x0f cond -> L1; 0x0e -> L2; L1: body; L2:`, the form a compiler emits when its only conditional branch is "branch if true"; loops are `L: cond; 0x0f -> body; 0x0e -> exit; body ...; 0x0e -> L` (176 cases); run-once guards `flag == 0 -> body sets flag = 1` |
| 0x11 | `a = b` (int or handle) | high | copies of native results into typed class variables (an `Actor` variable = `n111()`), locals, temps |
| 0x12 | `a = b` for a float value | medium | 10 uses; 9 store the result of 0x22 into a local, 1 copies a local |
| 0x13 / 0x14 | load int / float immediate | high | `c` is a plausible int in all 37060 / a plausible float in all 110 (1.0, 2.0, 0.5, 0.01, 30.0, 10.0) |
| 0x15 | `a = -b` | high | 626 uses, the operand is always a temp holding immediate 1; the result is passed to natives 3 and 6 (an element / location index of -1 = "none"; see the "eject" helper which teleports to `n6(-1)`) |
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
| `CheckVictoryCondition` | (2,4,8) | none; returns 0 (running), 1 (won) or 2 (lost: H02 and Tac21 select a debriefing first, `n28(k); return_value 2`); H01 returns a mission variable | level only, read by the engine every tick |
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
(the executable's own native names are visible only in error strings, without ids, and are not reproduced).
Evidence abbreviations: H01 = the first-mission walkthrough below; helper = a designer helper function that
consists mostly of the call, described by what its name says it does (`scb_semantics.py --handlers`); flow = argument / result data flow over the corpus
(`--natives`); bound = the immediate range matches a table of the mission file (`scb_xref.py`).

| Id | Arity, result | Hypothesised effect | Confidence | Evidence |
|---|---|---|---|---|
| 0 | (k, v) | declare mission variable k with initial value v | medium | level `Initialize` and an "init mission variables" helper; always followed later by 1 / 2 on the same k |
| 1 | (k, v) | set mission variable k | high | 192 uses in event handlers; k in 0..=40 plus designer codes (1004, 7777); v 0..=7 |
| 2 | (k) -> int | get mission variable k | high | 171 of 201 results go into `==` tests; `CheckVictoryCondition` returns it (H01) |
| 3 | (index) -> element | element by index in the level's flat element table (see Index spaces); -1 allowed | high | 9091 uses, result is the first argument of almost every element native; loops `for i < n75(): n3(i)` |
| 4 | (index) -> door | door / passage of the map (per-map index range: Lincoln <= 52, Nottingham <= 94, Derby <= 29, Leicester <= 55, York <= 122) | medium | door-initialisation and door-locking helpers; used only with 186-189 and 191 |
| 5 | (index) -> patch | map patch (visual alteration of the background) by per-map index (Lincoln <= 11 in all three Lincoln missions, Leicester <= 15, ...) | medium | patch-initialisation / refresh helpers; H01 drawbridge; only used with 144 / 145 |
| 6 | (index) -> location | location of the mission's `GULP` chunk: points first (0..points-1), then polygons | high | bound exact in 11 files (max index = points + polygons - 1), H01: the persecution zone (last polygon) is `n6(26)` = 15 points + 11 |
| 7 / 149 / 150 | (k) -> sound; (sound); (sound) | sound resource k (3..=19) of the level: 149 plays it once from a message handler (a "record crowd cheers" helper follows one such call), 150 starts it at level start (sherwood) | low | flow: 7's result is consumed only by 149 / 150 |
| 8 / 98 / 156 / 152 | (index) -> building; (actor, building) -> bool; (actor, building); (actor) | building (interior) index of a per-map table (Nottingham <= 41, York <= 73, Leicester <= 15, Derby <= 7; -1 = outdoors, via 0x15); 98 = actor is inside building; 156 = put actor inside building (a "put actor in building" helper; the `Initialize` of the four town missions with interiors); 152 = take the actor out of its building (always right before a 156 or an off-map teleport) | medium (index space, 98, 156), low (152) | bound: ranges consistent across the missions of one map (`scb_xref.py ... 8`); flow: 8's result feeds only 156[1] and 98[1]; the "all enemies in the castle out of action" helper counts an enemy only while `n98(x, n8(-1)) == 0`; `n98(pc, n8(4)) == 1` in an "is a PC at the place" helper |
| 9 | (index) -> path | patrol path (`RAIL` index) | high | max index < rail count in all 26 files that use it (exact in 3); consumed by 132 |
| 10 | (element) -> index | index of an element (inverse of 3) | medium | passed as the argument of messages whose handler does `n3(param1)` (H01 msg 9; tutorial soldiers `n44(n111(), 1, n10(n74()), ...)`); native 59 takes it |
| 12 / 13 | (patch) -> index; (location) -> index | index of a patch / of a location (inverses of 5 / 6, as 10 is of 3) | high | results are passed as message arguments (`n44(n111(), 1, n12(n5(k)), 0)`, `n44(..., n13(n6(k)), n10(n3(e)))`) and the handlers of the same files do `n145(n5(param1))` / `n45(n3(param1), n6(param2), 1)` |
| 18 / 112 | (location); (0 / 31) | presentation setup before a cutscene (18 at a location; 112 with a constant: 31 around duel sequences, 0 before a speech) | low | 2 / 4 uses |
| 20 | (location) | set the mission's deployment / start area (where the player characters are placed and the view begins) | low | 18 files, exactly one call per file, among the last statements of the level `Initialize`; only the forest missions (which start with a deployment phase) and five town missions; argument from 6 or `n95(actor)` |
| 24 | (actor, v) | set a per-actor value: 444 on one or two key non-player actors at level start (14 uses), 0 at start and 100 when that actor is freed (S02) | low | `Initialize` (10) and one handler; 444 occurs nowhere else (not an attribute value of 117) |
| 26 | (k, main) | add objective k = short-briefing text k; main = 1 for a primary objective, 0 for a secondary one | high | H01: `n26(0, 1)` at start = TEXT 1000283 string 0 (the observed initial objective), 1..5 added at the points where the campaign notes say those objectives appear (consistent with the notes, not traced); k < short-briefing count of the `.red` in every file |
| 27 | (k) | objective k accomplished | high | H01: `n27(0)` when objective 1 is added, `n27(3)` / `n27(4)` when the steward / knight sub-goals complete |
| 28 | (k) | select debriefing / ending variant k | medium | H01 `CheckVictoryCondition`: `if n2(2) == 1: n28(0)`; ambushes choose `n28(2)` or `n28(variable)` on a campaign flag; `Finalize(1) -> n28(1)`; k < debriefing count of the `.red` |
| 29 | () | notification after an objective / capture change (`n178(x); n29()`; `n29(); var = var + 1` when a PC enters the exit zone) | low | 5 uses |
| 30 / 31 | () | begin / end a sequence (script-driven cutscene or timed action list) | high | balanced in all 8176 functions, never nested; the executable has sequence / sequence-element classes (`docs/original/executable-notes.md`) |
| 32 | () | sequence step: wait for the previous element to finish | high | 3914 uses, only between 30 and 31, after every element that takes time (text page, camera move, wait, animation) |
| 33 | (location) | camera moves to location (sequence element) | medium | inside sequences, argument from 6 or 95; followed by 32 |
| 34 | (location) | camera returns / jumps to location (last element of every cutscene: `n34(n95(actor))`) | medium | H01 briefing end (observed: camera on Robin after the parchment), end of all tutorial popups |
| 35 | (float) | sequence element with a duration or rate (1.0 in 79 of 85 uses; 2.0; 0.5) at the start of cutscenes | low | always `n30(); n35(1f); n32(); n54()` |
| 38 | (actor, 0 / 1) | sequence element toggling an actor presentation state (1 on the hero at the start of H04's briefing sequence; 0 on every PC when a zone is entered in H10, 1 again later) | low | 5 uses inside 30 / 31, followed by 32 |
| 39 | (actor) | sequence element on an actor after a camera move or a teleport (appearance effect?) | low | 7 uses, followed by 32 or a walk |
| 41 | (0 / 1 / 2) | sequence element without a target (screen transition / camera mode k): 0 and 1 in pairs around presentation blocks (`n41(1); n32(); n55()`), 2 before a duel shot | low | 24 uses |
| 42 | (location, n) | sequence element at a location with a small count or duration n (2..=30), at the start of the ambush missions after a wait | low | 20 uses, always followed by 32 |
| 43 | (target, msg) | send message msg to target's `ProcessMessage` | high | H01: the archer sends msg 1 to the sergeant, whose class handles msg 1; every message id sent is handled by some class of the same file |
| 44 | (target, msg, arg, x) | send message with an argument (param1 of the handler); x in 0..=6 unknown (delay?) | high (first three) | H01 msg 9 with `n10(element)` -> handler uses `n3(param1)`; msg 13 with 1 / 0 = freeze / unfreeze NPCs |
| 45 | (actor, location, mode) | move actor to location, mode 0..=2 | medium | a "send to deployment zone" helper; sequence element; H01 son moves to a point |
| 46 / 47 | (actor, location, k, flag) | actor performs action k at location (46: k 1..=11, 47: k 2..=12; flag 0 / 1); sequence elements followed by 32 and often by 53 on the same actor | low | 19 / 15 uses |
| 48 | (actor, location) | move actor to location (sequence element) | medium | a "run to alert path" helper?; sergeant walks to the archer's location `n48(sergeant, n95(archer))` |
| 49 / 50 / 51 | (actor, anim) | play animation anim (51 in 418 `ActivatedByArrow`: target hit animation 210; 51 with 0 resets; 49 with 216 on the shouting sergeant) | medium | ranges 3..=270 in three natives; sequence elements |
| 52 | (actor) | sequence element on an actor (wait for it?) | low | H01 msg 9 |
| 53 | (actor) | actor-level action (ReachPoint 36, ActivatedByArrow 128 uses) | low | |
| 54 / 55 | () | enter / leave cutscene presentation (interface hidden, NPCs frozen by msg 13 around it) | medium-low | present in every popup sequence; not strictly paired per function |
| 56 | (ticks) | wait (sequence element); 25 ticks per second is the hypothesis | high | 1604 uses; immediates 10, 15, 25, 40, ...; `seconds * 25` via 0x1b in 108 uses |
| 59 | (archer, 4, target index) | archer shoots at target | low | an "archer shoots" helper; H01 archery training |
| 62 | (actor, k, flag) | actor shows expression k (1..=20; a class variable named after an expression feeds it in H10), flag 0 / 1 or random | low | 10 uses; sequence element |
| 64 | (actor, location, 0) | place / send actor at location | low | H01 msg 9 |
| 69 | (actor, id) | actor performs remark / gesture id (2..=96) before its dialogue text | low | sergeant before text 5, the persecuted one before text 3 |
| 70 | (actor, target, 1, range, reporter, msg) | actor hunts / duels target within range (float 40..=75); when it ends, message msg goes to reporter (555, 1001, 1002 are message ids handled in the same files) | low | 4 uses; the "hunt" helper of H12 |
| 72 / 73 | (0) | presentation pair: 73 opens, 72 closes (variant 0), 7..=10 s apart at the start of every forest mission (`n30(); n73(0); n32(); n56(173..255); n32(); n72(0); n31()`); 73 alone in a sequence at the end of two `Initialize` | low | 10 / 11 uses, level class, forest missions only |
| 74 | () -> actor | the actor this class belongs to ("self") | high | 1606 of 1622 uses in element `ProcessMessage`; fed to movement / AI natives |
| 75 | () -> int | number of elements (loop bound for 3) | high | `for i < n75(): n3(i)` in every helper that scans actors |
| 79 | (actor) -> bool | is a player character | high | gate of 269 `EnterZone` and of the target `ActivatedByArrow`; an executable error string names an is-actor-PC native |
| 80 / 81 | (actor) -> bool | actor kind predicates (80: NPC?; 81: soldier?) | low | a "lock all soldiers' AI" helper (81), an "un-blip all NPCs inside" helper (80) |
| 85 | (actor) -> bool | actor is unusable (dead / removed): helpers skip actors with `n85 == 1` | medium | the "activate all PCs" helper: `if n85(pc) == 0: n114(pc)`; the "kill actors in zone" helper skips them |
| 86 | (actor, actor) -> bool | the two handles are the same actor (identity) | medium | 116 uses, every result compared with 0 / 1; a "kick everybody but Robin out of the duel place" helper skips an actor unless `n86(x, robin) == 0`, `n86(x, a) == 0`, `n86(x, b) == 0`; zone gate `n86(param0, n3(k)) == 1` = "the entering actor is k" |
| 87, 88, 89, 90 | (actor) -> bool | status predicates or-ed by an "is actor neutralised" helper; 90 alone means "soldier out of action" in H01 | medium | H01 waits for `n90 == 1` on the courtyard lancers |
| 92 | (actor, 100) | set an actor value to 100 (one PC at level start, after 180) | low | 1 use |
| 93 / 94 / 133 | (element) -> dir; (actor, dir); (actor, location, dir) | facing direction 0..=15 (sixteen directions): 93 = direction of an element, 94 = set it, 133 = place actor at location facing dir (`n133(self, n95(self), n93(self))` = turn in place) | medium | 133's third argument is an immediate 0..=15 or `n93(...)`; 94 takes 2..=14 or `n93(...)`; 93 is compared with 2..=14 |
| 95 | (actor) -> location | location of an actor | high | fed by 3 / 211, consumed by 33 / 34 / 48 / 160 |
| 96 | (actor, location) | set actor location (teleport; `n6(-1)` = off map) | medium | the "eject" and "put out of map" helpers; an executable error string names a set-actor-location native |
| 97 | (actor, zone) -> bool | actor is inside zone | medium | "is PC safe" and "kill actors in zone" helpers (loop over actors with the zone parameter) |
| 99 | (actor) | reveal actor (un-blip) | low | the "un-blip" helpers |
| 101 | (actor) -> action | current action id of the actor (compared with 52..=57 by an "is sword-fighting" helper) | low | 3 uses |
| 102 | (actor, 10, 1) | inflict damage / kill | low | a "kill" helper |
| 103 | (actor) | send the actor away (the kick-out helper calls it on every other actor found in the duel building; H01 on the actor entering the servant's zone; H10 on the main PC) | low | 8 uses |
| 109 | (target, msg) | send message (second entry point; used from zones, scrolls, `ActionChange`) | high (delivers a message) | H01: `n109(archer, 1)` every 15 ticks drives the archer's msg-1 handler |
| 110 | (target, msg, a, b) | send message with arguments (variant of 44) | low | 21 uses |
| 111 | () -> actor | the player's character in the current context (H01: Robin); messages addressed to it reach the level script | medium | stored into an `Actor` class variable; `n34(n95(...))`-style camera code uses param0 instead; all messages sent to it in H01 and the tutorial are handled by the level class's `ProcessMessage` (Robin has no class in H01) |
| 113 / 114 | (element) | deactivate / activate an element (hidden actors, scrolls that appear later) | high | the "deactivate all PCs" (113) / "activate all PCs" (114) helpers; H01 tutorial scrolls activate each other |
| 117 / 118 | (element, attr, value) / (element, attr) -> value | set / get an element attribute (attr 1 on the knight = his purse / money; attr 0 on an archer incremented by the sergeant) | medium | H01 objective 4 completes when `n118(knight, 1) == 0` |
| 119 | () -> bool | victory predicate of H04 (`== 1` -> won) | low | 1 use |
| 125 | (actor, 1 / 3 / 7) | actor AI mode: 1 right after a new patrol path (132), 7 when a rail point is reached (then an animation), 3 once | low | 65 uses |
| 126 | (actor) -> state | actor state code compared with 1 and 2 (`== 1`, `op28 1`, `op28 2`); polled in `Hourglass`, tested on the actor entering a zone | low | 18 uses |
| 128 | (actor) -> bool | actor is able to act (alive / conscious): required `== 1` by every zone that reacts to an actor, by the "eject" helper, and by the "all enemies out of action" helpers for an enemy to still count | medium-low | 103 uses, all `== 1` |
| 132 | (actor, path) | assign patrol path (actor follows the `RAIL`) | high | 949 uses with `n9`; "new post", "swap path" and "alert reserve soldier" helpers |
| 134 / 135 | (actor, flag) / (actor) | lock / unlock the actor's AI | medium | "lock / unlock everybody's AI" and "lock NPC AI" helpers; msg 13 freeze in H01 |
| 140 | (actor, 0/1) | patrol mode flag (run?) set right after 132 | low | |
| 143 | (element, 0 / 1) | animated element off / on (an "initialise animations" helper switches 14 map elements off at start, handlers switch them on) | low | 20 uses; the 113 / 114 pattern of the H01 lights |
| 144 / 145 | (patch) -> bool / (patch) | patch is active / activate patch (also used as one-shot flags) | medium | H01 drawbridge: rope cut -> `n145(n5(4)); n145(n5(3))`; zones: `if n144(n5(k)) == 0: n145(n5(k)); ...` |
| 159 | () -> location | off-map location | low | the "put out of map" helper |
| 160 | (location, location) -> distance | distance between locations | high | compared with radii 10..=150; an executable error string names a get-distance native |
| 161 | (n) -> int | random number in 0..n | medium | `0.01 * float(n161(100))`, thresholds |
| 163 / 164 / 172 / 173 | () -> int; (i) -> actor; () -> int; () -> bool | sherwood camp: 163 = a count (compared with `n204(zone)` and `n174()`), 164 = its i-th member, 172 = a code (compared with 16968 and 0), 173 = periodic chance (`== 0` -> a random camp action) | low | sherwood only |
| 177 | (actor, 0 / 1) | actor flag: 1 when an enemy arrives at its post or is initialised (`n133(...); n134(x, 0); n177(x, 1)`), 0 when it is parked (before a rail-point turn) | low | 381 uses, 352 with 1, mostly `ProcessMessage` of enemy classes |
| 178 / 223 / 234 | (element); (element) -> bool; () -> bool | capture flags on banner elements of the tactical missions: 178 = mark captured (once the guards near it are out of action), 223 = is captured (victory `n223(banner) == 1`), 234 = all captured (a "refresh banners" helper) | medium | Tac01 `Hourglass`: soldiers 57 / 58 out of action and `n223(105) == 0` -> `n178(105)`; its `CheckVictoryCondition` returns 1 when 105 or 106 is marked |
| 180 | (actor, 0 / 1) | actor flag: 1 on the player characters at start (an "initialise PCs" helper), 0 on the actor caught when a trap object fires (the trap's `ActionChange`: patch on, `n180(actor, 0)`) | low | 41 uses |
| 182 | (door) -> bool | door state predicate (`== 0` -> once: a patch, an actor moves, `n191(0, door)`) | low | 3 uses in `Hourglass` |
| 186 - 189 | (door, 1) | lock / unlock door variants | low | the door-locking helper calls 186, 187, 188, 189 |
| 191 | (state, door) | open / close door (0 at level start, 1 when the player reaches the area) | low | H01 door-initialisation pattern |
| 192 | () -> element | this class's element for non-actor classes (scrolls, objects, zones): the counterpart of 74 | medium | 139 uses in element `Hourglass` / `IsTaken`, fed to 193 / 194 (state), 113, 95, 233 |
| 193 / 194 | (element) -> state / (element, state) | get / set an element state 0..=3 (bit 1 toggled with `+ 2`, or set to 3) on scrolls and zones | low | H01 msg 7, `Hourglass` |
| 195 | (k) -> int | campaign / progress flag k (0..=11) | low | ambush debriefing choice `if n195(6) == 1` |
| 196 | (k, flags) | availability of player action / skill k (0..=11) | low | H01 `Initialize` disables 6, 7, 8, 9, 3; tutorials enable with or-ed flags |
| 197 / 198 | (actor, k) -> flag / (actor, a, b) | actor visibility / blip flags | low | "mark to un-blip" (198) and "un-blip marked NPC" (197, 198) helpers |
| 199 / 200 | (k, location, n); (k, location) | production zones of the camp (sherwood): kind k 0..=12 at a location; 199 adds a capacity (5, 20, 170, 340) | low | the sherwood "initialise production zones" helper only |
| 202 | (k) | show text k of the level's text list immediately (dialogue line / hint) | high | k < text count of the matching `.red` in all 30 files; H01 hints 13, 14, 15, 17, 19, 21 appear in the handlers of the matching tutorial scrolls |
| 203 | (k) | show text k as a sequence element (parchment page; waits for dismissal) | high | H01 `PostInitialize` shows 0, 1, 2 = the three observed briefing pages; popups 3..=22 inside cutscenes |
| 204 | (zone) -> int | presence / count of (player) actors in zone | low | H01 persecution zone `ExitZone`, an "is every enemy in zone" helper |
| 205 | (zone, i) -> actor | i-th actor inside zone (loops `for i < n204(zone)`) | medium | 26 uses; result fed to 80 / 81 / 82 / 99 / 243 |
| 210 | (k) -> bool | campaign progress flag k (0..=25): sherwood's `Initialize` derives the camp's needs from ten of them, H02's `CheckVictoryCondition` tests 25 | low | 11 uses, all `== 0 / 1` |
| 211 | () -> actor | the main player character | medium | `n34(n95(n211()))` = camera on Robin after the briefing (observed) |
| 212 | (actor, location, 1, d) | actor moves away from a location by d (25..=50 or 100 + random) | low | 7 uses; sequence element |
| 213 | (location, location, t) -> location | point between the two locations at fraction t (t = 0.01 x random(100), or 0.1) | medium | 14 uses, arguments from 6 / 95 and a random float; result to 45 or a nested 213 |
| 214 / 247 / 248 / 258 / 261 | (element); (element); (x) -> bool; (x) -> bool; () -> bool | one-shot calls of sherwood and H07 (`Initialize`, `PostInitialize`, random-training helpers) | low | 1..=2 uses each |
| 215 | (k) -> element | campaign roster slot k (0..=6): the camp attaches an information scroll to each slot present (`n85(n215(k)) == 0`) | low | 14 uses |
| 216 / 217 | () -> int / (i) -> actor | number of player characters / player character i | high | the "activate all PCs" helper loops `for i < n216(): n217(i)` |
| 218 | (leader, member) | group members under a leader | low | H01 sergeant collects the archers after training |
| 219 / 220 | (actor) | actor orders: 219 right before an "alert soldier" helper (wake / alert), 220 after `n177(x, 1); n228(...)` and in `ReachPoint` (stop / clear orders) | low | 18 / 51 uses |
| 221 | (actor) -> bool | actor is mounted (a "save riders" helper keeps those with 1; zones and the eject helper require 0) | low | 89 uses |
| 222 | (element) -> bool | object was used / taken (polled in element `Hourglass`, sets a "used" flag) | low | 94 uses, all `== 1` |
| 224 | (location, 30.0, 10.0, flags) | set a trap at location | low | "set trap" and "add repulsive zone for hole" helpers |
| 226 | (0 / 1) | cutscene flag paired with 54 / 55 (`n226(1); n54()` ... `n226(0); n55()`) | low | 73 uses |
| 228 | (actor, k, ticks) | timed actor state k (1 in 140 uses; 0, 2, 4, 5, 6) for n ticks (`seconds x 25`, 10..=100), right after 177 | low | 170 uses |
| 229 | (actor) | actor drops out (right before a kill 102 or an off-map teleport 96 in the "kill" / "eject" helpers; PCs are deactivated instead) | low | 49 uses |
| 231 / 246 | (zone) -> bool | group presence in a zone: 231 = a group is inside (civilians in a house, an army position), 246 = the player characters are all inside (victory `n246(exit zone) == 1` in H03, H04, S04) | low | 5 / 3 uses |
| 232 | (actor) | actor joins the player's party (freed prisoners: `n114(x); n232(x)`; S03 at start) | medium | 10 uses |
| 233 | (actor, element) | actor goes to / addresses element (an actor, a scroll, a zone) | medium | H01: the servant goes to Robin, the son goes to a scroll position, the persecuted one goes to a zone |
| 235 | (element) -> bool | element taken / used (purse object) | low | H01 steward objective |
| 236 / 237 | () -> int; (v) | get / set the player's money (`n237(n236() + 25)`, `n236() < 100000`, `n237(n236() - 2000)`) | high | 8 / 5 uses |
| 240 | (actor) -> bool | actor is present (active on the map): the "all enemies out of action" helpers require `== 1`; `== 0` or-ed with 89 declares the messenger lost in Tac21 | medium-low | 22 uses |
| 243 | (actor) | highlight actor during a cutscene | low | 340 uses in `IsTaken`, always inside a sequence on the actor the text talks about |
| 244 | (patch, 0) | clear a patch flag (the beam-me spots of absent player characters: `if n85(pc) == 1: n244(patch, 0)`) | low | 87 uses, second argument always 0 |
| 245 | () -> int | number of player characters (compared with mission variable 3 = `n216()` at start, incremented per PC entering the exit zone) | medium | 2 uses |
| 250 | (0) -> actor | player character by campaign id (0 = the main character; `n34(n95(n250(0)))` replaces `n211()` in the forest missions) | medium | 13 uses, always 0; result to 95 / 10 / an `Actor` variable |
| 253 / 255 | (k) -> bool | campaign character k is alive / present (253: k 25..=28, `== 0` -> mission lost with `n28(k); return 2`; 255: k 1, 2, 23, 26, all `== 0` -> "no heroes") | medium-low | 14 uses, all in `CheckVictoryCondition` |
| 254 | (patch, 0 / 1) | patch flag (1 at start, 0 when its door opens and its actors are activated) | low | 3 uses |
| 256 | (actor) -> id | campaign character id of an actor (compared with 1..=5) | low | sherwood |
| 264 | (actor, k, 1) | actor start pose / idle k (61, 74, 114: none is an animation id of 49 - 51) with flag 1, after locking the AI | low | 12 uses, `Initialize` only |

Message ids are designer-defined per mission (1.., 100.., 1000.., 2987..3017, 4000.., 5000..); a handler compares
`param0` with immediates and every id that is compared is sent by some 43 / 44 / 109 / 110 call of the same file
in all but a few files (`scb_semantics.py --messages`), so the engine itself sends few or none of them.

### Natives at load per mission

The engine runs `Initialize` on every class (level first, then the elements in table order), then the level's
`PostInitialize`, then the first elements of the sequences those callbacks opened; messages go out on the next
tick. So the natives reached at load are those of the two callbacks and of the helper functions they call.
`scb_load_natives.py` computes that closure statically (every branch taken) against the engine's implemented /
stub lists (`natives.rs`); a run of the engine with `--lenient-natives` (2026-09-02, seed 1, all missions,
500 ticks each) confirmed which of them are hit, which follow at tick 1 (`Hourglass` of every class,
`CheckVictoryCondition`), and which appear within the first 500 ticks. A strict VM stops a callback at the
first unknown id ("first trap"; a second trap is the level's `PostInitialize`), so the whole load column must
be handled before a mission gets past `Initialize`. 244 is reached statically in every forest mission but hit
only when a player-character slot is empty (`n85(pc) == 1`), i.e. in the five files that mark it.

| Mission | Unknown at load (hit) | First strict trap | Tick 1 | Within 500 ticks |
|---|---|---|---|---|
| Emb01_FoA_EC | 20, 42, 72, 73, 250 | 20; 73 | - | 177, 228 |
| Emb02_FoC_MK | 42, 46, 244 | 244 | - | - |
| Emb03_FoC_MP | 42, 72, 73, 250 | 250 | - | - |
| Emb04_FoA_MP | 42, 72, 73, 250 | 73 | - | 133, 177, 220, 228 |
| Emb05_FoB_MP | 12, 20, 72, 73, 244, 250 | 244; 250 | - | 177, 228 |
| Emb06_FoC_EC | 13, 20, 42, 72, 73, 250 | 20; 73 | - | - |
| Emb07_FoB_JMS | - | - | 255 | - |
| Emb08_FoA_JMS | 180, 228, 244 | 228 | 255 | - |
| Emb09_FoB_JMS | 12, 72, 73, 244 | 244; 12 | - | 177, 228 |
| EmbTut_FoC_EC | 20, 42, 72, 73 | 20; 73 | - | 228 |
| H01_Lin_VL | - | - | - | - |
| H02_Not_EC | 24, 264 | 24 | 8, 98, 192, 222, 240, 253 | - |
| H03_Der_MK | 42 | 42 | 222, 240 | - |
| H04_Lei_VL | 38, 254 | 254; 38 | 93, 119, 126, 133, 192, 222 | - |
| H05_Lin_EC | 24, 177 | 24 | 182, 192, 222, 240 | - |
| H07_Not_MK | 20, 92, 177, 180, 205, 244, 247, 264 | 20 | 8, 98, 231, 240 | - |
| H09_Not_VL | - | - | 126, 192, 222 | - |
| H10_Yor_VL | 24, 236, 237 | 236 | 192, 222, 253 | - |
| H12_Not_MP | 8, 20, 156, 177 | 177 | 46, 98 | - |
| S01_Not_VL | 24 | 24 | 192 | - |
| S02_Lei_MP | 8, 20, 24, 156, 177, 254, 264 | 264 | 182, 253 | - |
| S03_FoB_MP | 8, 20, 38, 70, 125, 156, 177, 232, 250 | 232; 250 | 47, 93, 98, 133 | - |
| S04_Der_EC | - | - | 192, 222, 240, 246 | - |
| S05_Yrk_EC | 8, 20, 24, 156, 264 | 264 | 98, 222, 240, 245 | - |
| Str01_Lin_EC | - | - | 182, 222 | - |
| Str02_Der_MP | 20 | 20 | - | - |
| Str03_Yor_MK | 143 | 143 | 223, 231, 234 | - |
| Tac01_FoA_MP | 20, 250 | 20; 250 | 223 | - |
| Tac02_FoB_EC | 20 | 20 | 178 | - |
| Tac03_FoC_MP | 39, 250 | 250 | 223 | - |
| Tac04_FoA_EC | 20, 42, 250 | 250; 42 | 178 | - |
| Tac05_FoC_MP | 177 | 177 | 178 | - |
| Tac06_FoB_EC | 20 | 20 | 178 | - |
| Tac17_FoC_EC | 20, 42, 72, 73, 250 | 20; 73 | 178 | - |
| Tac18_FoA_EC | 20, 42, 72, 73, 244 | 244; 73 | 178 | 133, 177, 220, 228 |
| Tac19_FoB_EC | 12, 20, 72, 73 | 12; 20 | 178 | 177, 228 |
| Tac21_FoB_EC | 20, 39, 250 | 20; 39 | 29, 178, 240 | - |
| sherwood | (static) 7, 150, 205, 210, 213, 214, 215, 256, 261 | refused: no element index space (Index spaces) | 86, 101, 112, 125, 126, 163, 164, 172, 173, 205, 213, 229, 232, 240, 248, 255, 256, 258 (static) | - |
| SherwoodOutro | (static) 180 | refused (same) | - | - |

Union of the load column over the 37 loadable files: 8, 12, 13, 20, 24, 38, 39, 42, 46, 70, 72, 73, 92, 125, 143,
156, 177, 180, 205, 228, 232, 236, 237, 244, 247, 250, 254, 264 (28 ids; 20 blocks 18 files, 250 eleven, 42 ten,
72 / 73 ten). Tick 1 adds 8, 29, 46, 47, 93, 98, 119, 126, 133, 178, 182, 192, 222, 223, 231, 234, 240, 245, 246,
253, 255 (21 ids); the first 500 ticks add 133, 177, 220, 228 (ambush messages of seven forest missions). The
lenient run reached tick 500 in every loadable mission without a fault, so with the load ids handled every
mission runs past `Initialize` / `PostInitialize`; with the tick-1 ids at the values below the early game
traps nowhere. Unknown ids that are never reached in those 500 ticks (by frequency): 221 (89), 226 (73),
200 (55), 229 (49), 41 (24), 219 (18), 62 (10), 94 (10), 21 (8), 40 (7), 179 (7), 212 (7), and 30 more with
five uses or fewer.

**Stub policy.** A *recorded stub* is a no-op returning 0 (`STUB_NATIVES`). Whether that is safe depends on
whether the script branches on the result; the table gives the value that keeps the scripts sane where 0 does
not, and what a first implementation would be where the row is confident enough.

| Id | 0-stub safe? | Notes |
|---|---|---|
| 7, 149, 150; 18, 112; 20; 24, 92; 29; 103; 125; 143; 152, 156; 177; 180; 214, 247; 219, 220; 226; 228; 229; 244; 254; 264 | yes | no result, or the result is never branched on; 20 is safe because the engine already places the player characters from the mission file |
| 38, 39, 41, 42, 46, 47, 62, 72, 73, 212 | yes, as sequence stubs | they sit before a 32 barrier: the completion token must complete at once (the 49 - 53 treatment) |
| 8 | yes at load | the result reaches only 156 / 98; implement as the index itself (-1 = outdoors) together with 98 |
| 98 | yes (never-win only) | branches every tick in six town missions; with 0 the "enemies in the castle" helpers never succeed; sane: 1 iff the building argument is the outdoors handle (-1), the engine has no interiors |
| 12, 13 | no | the handler would act on patch / location 0; implement as the index (high) |
| 70 | yes at load | the reporter message (555 / 1001 / 1002) never arrives, so the hunt flows of H12 / S02 / S03 stall later |
| 86 | yes | zones keyed on one actor never fire and the kick-out helper kicks nobody; implement as handle equality (medium) |
| 93, 94, 133 | 93, 94 yes; 133 loses placements | ambush enemies are placed with 133 after tick 100: implement 133 as 96 (teleport) ignoring the direction |
| 101, 126 | yes | branches compare with 52..=57 / 1 / 2 |
| 119 | yes | 1 would win H04 at tick 1 |
| 128 | **no** | zones reacting to actors never fire and every "all enemies out of action" helper returns 1 at once: return 1 |
| 178, 223, 234 | yes (never-win) | tactical missions cannot be won until 223 reads a per-element flag set by 178 |
| 182 | either | a run-once flag guards the branch; 1 avoids a spurious door close at tick 1 |
| 192 | **no** | 0 addresses element 0 (a map element) with 193 / 194 / 113: return the class's own element (as 74 does for actors) |
| 205 | return -1 | 0 would be a map element handed to 80 / 81 / 99 / 243 |
| 210, 215, 256, 261, 163, 164, 172, 173, 199, 200, 248, 258 | yes | sherwood only (refused today); 210 = 0 means nothing accomplished |
| 213 | yes (wrong walks) | 0 is location 0; implement the interpolation (medium) |
| 221, 222 | yes | 0 = not mounted / not used: the zones fire as for unmounted actors; objects count as unused until implemented |
| 231, 246 | yes | 1 would win H03 / H04 / S04 at tick 1 |
| 232 | yes at load | the actor does not join the party (S03's companion stays uncontrollable) |
| 236, 237 | yes | implement as one VM integer (H10 sets 100000 at load, S05 subtracts 2000) |
| 240 | **no** | with 0, Tac21 is won at tick 1 (its messenger counts as lost) and the "all enemies out of action" helpers succeed: return 1 |
| 245 | yes (never-win) | H02 / S05 win when mission variable 3 reaches it; implement as the number of live player characters |
| 250 | no | 0 would be element 0: return the main player character (211's value) |
| 253, 255 | **no** | every `CheckVictoryCondition` that tests 253 returns 2 (lost) at tick 1 (H02, H05, H10, S02, Tac21) and 255 = 0 sets "no heroes" (Emb07, Emb08): return 1 |

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

Read with `scb_semantics.py <file> --pseudo`. Element indices are given with the *roles* of the `.rhm` records
they resolve to under the element table above; texts are referred to by their index in TEXT 1000105 (see
`docs/original/campaign-flow.md` for what each says) and objectives by their index in TEXT 1000283. Mission
element classes are referred to by role, never by their designer names (those are game data).

**Confidence marks.** Every statement carries the *minimum* confidence of the opcode, native and index-space
hypotheses it depends on: **[H]** all high, **[M]** at least one medium, **[L]** at least one low. The
element-index resolution (index -> `.rhm` record) is itself **medium** for the mission part of the table
(placed by object self-references) and **medium** for the map part (only the `FLIM` light check supports it),
so no statement that names a specific actor, scroll or zone by index is better than [M]. Only the statements
marked *observed* were checked against the original (`docs/original/campaign-flow.md`: the three briefing
pages, the initial objective, the camera on Robin afterwards). Everything else is *consistent with the
observed flow* at best; nothing in this section is "exact" until an oracle trace confirms it.

**Level class** (8 variables: a shooting timer, a run-once flag, two "sub-goal done" flags, a loop counter
and bound, an `Actor` scratch variable, a "going away" flag).

- `Initialize` [L]: disables five player actions (`n196(6..9, 0)`, `n196(3, 0)`; 196 is low); deactivates
  map elements 19..=32 [M] - in `lincoln.rhp` these `FLIM` entries are the two chimney fires, five torches and
  seven candles, i.e. the interior lights of a day mission (the room-entry messages below switch them on
  again room by room) - and several scrolls (102, 104..=106, 108..=110 = the scrolls that appear only after
  earlier steps) [M]; zeroes the variables [H]; stores the player character (`n111()`) [M]; declares mission
  variables 1, 2, 3 [M]; locks doors 20 and 28 and closes doors 8, 20, 21, 37, 25, 23 [L: natives 4, 186,
  191]; deactivates, AI-locks and hides four actors (50, 79, 53, 92: the girl, a soldier, the servant, a
  soldier) that are activated by later events [L: 113 high, 134 medium, 197 / 198 low]; sets two attributes
  of element 126 to 0 [M: 117]; returns 0 [H].
- `PostInitialize` [M]: adds the primary objective 0 [H], then one sequence [H: 30 / 32 / 31]: text pages 0, 1,
  2 [H: 203; *observed*: the three briefing pages], then the camera goes to the main PC (`n34(n95(n211()))`)
  [M: 34, 211; *observed*: camera on Robin after the parchment]. This is consistent with the observed
  Play! -> briefing -> camera-on-Robin flow; the tick at which the camera move completes and whether the
  sequence blocks input are not verified.
- `Hourglass(time)` [M overall]: every 15 ticks [M: the tick unit of the time parameter is a hypothesis]
  while mission variable 1 (training over) is 0 [H], sends message 1 to the first archer (element 70) [H:
  109], which runs one shot of the training loop. When soldier 87 is out of action (`n90`) [M], once: soldier
  87 and civilian 51 (the girl) get patrol paths 65 / 58 [H: 132] and the girl runs (native 140) [L]. When
  mission variable 3 is 0 and the four soldiers 81..=84 are all out of action [M], message 1 goes to the
  persecuted civilian (52) [H]. Steward sub-goal: when the steward's purse (105) is taken (`n235`) [L],
  objective 3 is accomplished [H] and scroll 120's state gets bit 1 (193 / 194) [L]. Knight sub-goal: when the
  knight (78) has no money (`n118(78, 1) == 0`) [M], objective 4 is accomplished [H]. Exit: when any of the
  courtyard lancers 75..=77 is out of action [M], soldier 56 gets path 57 [H].
- `CheckVictoryCondition` [M]: returns mission variable 2 [H] (set when Robin tells the servant's son he
  wants to leave); selects debriefing 0 first (`n28`) [M].
- `ProcessMessage(msg, arg)` [H for the dispatch, per branch as marked]: 1 = a target was hit (training over:
  objective 5 accomplished [H], variable 1 = 1 [H], the sergeant groups the six archers (218) [L] and
  everybody walks to the mess along paths 66..=71 / 38 [H], the sergeant speaks text 6 [H: 202]); 2 = same
  without the sergeant's line (an archer changed action 141) [H]; 3 / 5 / 6 / 7 = area entered the first
  time (patch flag 1 / 10, 11 / 8 / 7) [M: 144 / 145]: activate the actors, light effects and scrolls of that
  area [H], open its doors [L], give paths [H]; 8 / 9 = give element `arg` a path [M: 10] (9: also reveal it
  [L: 99] and play a sequence [L: 52, 64]); 10 = the persecuted civilian goes to the servant's zone [M: 233];
  11 = shift the shooting timer [H]; 12 = the son goes to location 9 then to scroll 114 [M: 45, 233]; 13 =
  freeze (`arg = 1`) or unfreeze all NPCs (loop over all elements with 80 / 134 / 135 / 197) [L]; 14 = the
  servant's cutscene (texts 8, 9, 10 [H], camera to location 9 [M], the son moves [M], message 12 [H], camera
  back [M]); 15 = after the drawbridge rope is cut, `n137(location 14, 1)` [L: 137 is not in the table].
- `Finalize`: empty [H].

**Actor classes** (archers, the sergeant, the target soldiers, the lancers, a shield-bearer, the persecuted
civilian): `Initialize`, `HandleEvent`, `FilterAIEvent` (returns 1) are stubs [H]. `ActionChange(_, 141)`
sends message 2 to the level [H]. The first archer's `ProcessMessage(1)` [M]: sequence - message 1 to the
sergeant (he walks to location 1 [M: 48]), wait 12 [H: 56], shoot at target 96 (`n59`) [L], reset the target
animation [M: 51], wait 12, message 2 to the sergeant (he increments attribute 0 of the archer [M: 117 / 118],
walks to him [M], plays animation 216 [M: 49]). The persecuted civilian's `ProcessMessage(1)` [M]: the four
soldiers get paths 25..=28 [H], variable 3 = 1 [H], then a sequence moving the civilian to locations 7 and 8
[M: 45] and message 10 to the level [H]. The shield-bearer's `ActionChange(_, 141)`: gives soldier 87 and the
girl their paths [H].

**Object classes** (the four archery targets, the drawbridge mechanism): all `ActivatedBy*` return 1 [H].
`ActivatedByArrow` of a target [M]: if not yet hit and the shooter is a PC (`n79`) [H], mark hit, sequence -
play animation 210 on itself [M: 51], wait 10 [H], message 1 to the level [H]. `ActivatedBySword` of the
mechanism [M]: once - animation 160 on itself [M], message 15 to the level [H], activate patches 4 and 3 (the
lowered drawbridge) [M: 145].

**Rail point classes**: the archers' officer rail point [M]: a sequence sending message 9 with each archer's
index every 12 ticks [M: 10, 44] (the group leaves one by one). The two girl-path rail points'
`ReachPoint(actor)` [L]: hand over paths between the girl and the soldier following her [H], then
`n130(soldier, girl, 1)` [L: 130 is not in the table].

**Scroll classes** (`IsTaken(actor)`, return 1 or 0): the archery-start scroll [L]: if training not over -
secondary objective 5 [H], cutscene (freeze via message 13 [L], camera to the sergeant [M], remark 61 [L: 69],
texts 5 and 22 [H], message 11 [H], unfreeze [L]). The money scroll [H]: if training over show text 19,
activate scroll 108. The servant's scroll [M]: primary objective 1, objective 0 accomplished [H], reveal the
son [L: 99], message 14 to the level [H], the servant goes to Robin [M: 233]. The servant's son's scroll [M]:
primary objective 2, objective 1 accomplished, text 11, mission variable 2 = 1 (victory) [H], the son goes to
Robin [M]. The climbing tutorial scroll [M]: cutscene - camera to location 11 [M], text 12 [H]. The jump
tutorial scroll: text 13 [H]. The knock-out tutorial scroll: texts 14 and 21 [H]. The beggar tutorial scroll
[L]: cutscene - camera to location 0 [M], highlight the beggar (55) [L: 243], text 16 [H]. The pick-up
tutorial scroll [H]: text 15, first time also text 17 and activate scroll 106. The steward-tip scroll [L]: if
the steward's purse is not taken [L: 235] - secondary objective 3 [H], cutscene (camera to location 3 [M],
message 3 [H], highlight 79 and 50 [L], text 4 [H]). The knight-tip scroll [L]: if the knight still has money
[M] - secondary objective 4 [H], cutscene (text 7 [H], camera to the knight [M], highlight him [L]). The
drawbridge-tip scroll [M]: first time activate patch 5 [M] and element 99 (the mechanism) [H]; cutscene with
text 20 [H]. The hall-opening tutorial scroll [M]: unless patch 7 is active [M] - cutscene (camera to location
12 [M], message 7 [H]). The arrows scroll [H]: activate scroll 102. The poor man's scroll [L]: activate scroll
110 [H], the persecuted civilian goes to Robin [M], remark 86 [L] and text 3 [H].

**Zone classes** (`EnterZone(actor)`): all first test `n79(actor) == 1` (a PC) [H]. The two zone-6 polygons
[M]: message 6 while patch 8 is inactive; the west-tower zone [M]: message 3 while patch 1 is inactive; the
drawbridge-tower zone [L]: activate patch 5 and the mechanism [M], set state 3 on element 122 [L: 194]; the
lower central zone [L]: patch 6 [M], activate soldier 23 [H], doors 37 / 6 [L]; the two zone-7 polygons [M]:
message 7 while patch 7 inactive; the great-hall zone [M]: message 5 while patch 11 inactive; the two
staircase zones [M]: patch 10, activate 24 and 25; the servant's zone [L]: first time - `n103(actor)` [L: 103
is not in the table], cutscene with camera to location 4 [M] and text 18 [H] (the servant's introduction);
the persecution zone's `ExitZone` [L]: if PCs are in zone 26 (`n204`) [L] and variable 3 is 0 [H] - variable
3 = 1 [H], message 1 to the persecuted civilian [H].

What a first VM needs for this mission: the calling convention above; natives 0-3, 6, 9, 10, 26-28, 30-35,
43-45, 48-52, 56, 59, 64, 69, 74, 75, 79, 80, 85, 90, 95, 96, 99, 103, 109, 111, 113, 114, 117, 118, 130,
132-135, 137, 140, 144, 145, 186, 191, 193, 194, 196, 197, 198, 202-204, 211, 216-218, 233, 235, 243, 4 and
5 as at least stubs; message delivery to classes by element; the periodic `Hourglass`; the sequence model
(30 / 32 / 31 with blocking elements). Natives 130 and 137 are used by this mission but have no table
row yet (single uses; effect unknown); 103 has one (low).

## Engine notes

Sequence scheduling and scroll pickup (engine hypotheses, 2026-09-02): every sequence advances independently
each tick (one per element, like the original's sequence manager); running them one after another queued a
scroll's popup behind the archery-training sequences of the first mission. A player character within 24 map
pixels of an active scroll bound to a class triggers `IsTaken` once per approach; a non-zero result takes the
scroll (it becomes inactive). Observed in the engine: the third-nearest scroll of the first mission shows a
text page, the two nearer ones activate their areas without text. Natives implemented and stubbed are listed
in `crates/opensherwood-core/src/natives.rs` (`IMPLEMENTED_NATIVES`, `STUB_NATIVES`).

Barrier and completion tokens (engine model of native 32, 2026-09-02): inside a sequence every element that
takes time issues a completion token; native 32 is a barrier that holds the sequence until every token issued
since the previous barrier has completed, then clears them. A walk (45 / 48 / 64) completes when the actor is
no longer walking to that point: it arrived, the path failed, it was ordered elsewhere, deactivated or died
(hypothesis, medium for the arrival, low for the failure cases: the original presumably waits for the arrival
only; treating failure as completion keeps a blocked cutscene from stalling a mission). Animations (49..=53)
are stubs whose token completes at once (the engine has no animation model yet). Text pages (203) and waits
(56) hold the sequence directly; camera moves (33 / 34) are instant. Native 202 texts never block anything:
they are queued with `blocking = false` and the app may show them without pausing. Hypothesis to verify with
the original: whether 32 also waits for a camera pan and how long an animation element takes.

AI locking (natives 134 / 135, engine hypothesis, confidence **low**): locking halts the actor's current walk
(the rail program stays on its instruction and re-issues the walk when unlocked; a barrier waiting for that
walk completes) and stops the rail program from issuing new orders; a player character's orders are the
player's and are not touched. The original's "freeze" (message 13 in the first mission) may instead let a
walk in progress finish; the engine's choice is pinned by
`locking_mid_walk_stops_the_ai_walk_and_completes_the_barrier` so a correction is a deliberate ruleset bump.

Work budget (2026-09-02, ruleset 7): everything the VM does in one tick is charged to one deterministic
budget (`vm::WORK_BUDGET_PER_TICK`) granted at the start of the tick and nowhere else (the load-time run has
`vm::WORK_BUDGET_AT_LOAD`; event hooks and text dismissals draw from the tick's remainder): instructions,
arguments transferred by calls and natives, every entity a zone / scroll scan looks at, every polygon edge
tested (zones, natives 97 and 204), sequence elements, and every stage of the path searches of the walks the
script issues (initialisation, expansions, unwinding, smoothing, conversion). When it is spent the tick stops
(the running callback is aborted, the remaining phases wait for the next tick, undelivered messages stay
queued) and `counters.budget_aborts` counts it; the retail scripts use a small fraction of it. Every callback
exit (return, abort, fault, trap) clears the frames, both stacks and a sequence being collected, and a program
whose parameter / argument stacks are not balanced in some function is refused at load.

Natives after the stub policy (2026-09-03, hash schema 9 / snapshot schema 10): every id with a row of the
native call table is now implemented or a recorded stub (`natives.rs`: 69 implemented, 99 stubs; an id without
a row, e.g. 21, 40, 179, still traps in strict mode). Implemented from the policy table: 8 / 12 / 13 as the
index itself (8: -1 = outdoors), 86 as handle equality, 98 = 1 iff the building argument is -1 (the engine has
no interiors), 192 = the calling class's own element (74 for non-actor classes), 250 (0) = 211's value (the
main player character), 236 / 237 as one integer `VmState::money` (hashed, snapshotted, `debug.vm.money`; the
HUD does not read it yet), 245 = the number of live player characters (S05 starts mission variable 3 at 0 and
wins when it equals 245, so a 0 stub would win at tick 1; H02 gates the same test behind variable 2), 133 as 96
plus the facing, 93 / 94 as the facing of an actor. The sixteen script directions are mapped onto the entities'
256-unit facing as `direction x 16` with direction 0 = facing 0 (the `+x` axis): which direction the original
calls 0 is not established (confidence **low**), pinned by `facing_natives_map_sixteen_directions_onto_facing256`.
Recorded stubs with a policy value (`STUB_POLICY_VALUES`, pinned by `policy_values_of_the_stub_table_are_pinned`):
128 / 240 / 253 / 255 return 1, 205 returns -1; every other stub returns 0, including 119 / 231 / 246 (a 1
would win H03 / H04 / S04 at tick 1) and the never-win group 178 / 223 / 234 / 222 / 182 / 213 / 70 / 232.
The sequence stubs 38, 39, 41, 42, 46, 47, 62, 72, 73, 212 are collected as sequence elements (recorded when
the sequence reaches them) and issue no completion token, so the barrier that follows them does not wait.
`CheckVictoryCondition` = 2 sets `VmState::mission_lost` (sticky like `mission_won`, hashed, in the `script`
observation and `debug.vm`); the app does not react to it yet. Strict run of 2026-09-03 (seed 1, no page
dismissed): all 37 loadable missions run `Initialize` / `PostInitialize` and 1000 ticks without a trap, a
run-time fault or a budget abort, none is won or lost; H10 holds 100000 of money after its `Initialize`.
The harness pins the load-time state of every script (`EXPECTED_AT_LOAD`) and the first 300 strict ticks
(`harness/tests/data/test_script.py`).

Stealth layer (2026-09-03, ruleset 9, `crates/opensherwood-core/src/ai.rs`,
`docs/original/stealth-and-combat.md` "Engine"): 87 (dead), 90 (out of action: knocked down, lying knocked
out or dead; a soldier getting up is back, hypothesis), 128 (alive, active and on his feet; non-actor
elements always can act) and 240 (the entity's `active` flag; other elements present unless deactivated)
read the entities' states instead of the policy values (73 implemented, 95 stubs; `STUB_POLICY_VALUES` keeps
205 / 253 / 255); 88 / 89 stay stubs at 0 (no tied / netted state exists). 140 (actor, 0 / 1 / 2) sets the
gait of the actor's rail walks (0 walk, else run: the hypothesis of the stealth spec, section 2.5; a walk under
way keeps its gait). `ActionChange(previous, new)` fires on the class bound to an actor whenever the actor's
reported sprite action id changes (`ai::action_id`: 0 / 6 / 7 / 14 / 16 for the normal posture, 141 / 142 /
140 / 143 / 151 for the alert states, 123 / 41 / 44 / 47 / 48 / 49 for the knock-out); the parameter order
is a hypothesis from the actor classes comparing the second parameter with 141 and the object classes the
first with 137 (objects never fire it yet), pinned by `action_changes_reach_the_actors_class`. `FilterAIEvent`
is not called. `debug.vm.counters.out_of_action_true` counts the calls of 90 that reported 1 (diagnostic);
the `script` observation's `actor_elements` lists the element handle of every entity.

## Cross-references

- Class names == mission element names of the paired `.rhm` (100 % both ways, see [rhm.md](rhm.md)).
- A class named `StartUp` comes first in every file; the executable treats a level without its startup script as
  a fatal error (paraphrased message), so the engine may look the level class up by this name.
- Native argument ranges against `.rhm` chunk counts and `.red` text counts: see Index spaces.

## Tools

`opensherwood-tools scb <file> [--class NAME] [--no-code]` prints classes, variables, the function table and the
raw disassembly (`enter`, `call`, `native`, `jump`, `jump_cond` for the established roles; `op_XX` with decoded
operands otherwise; raw fields are appended whenever the layout does not cover a non-zero field).

Probes (`harness/tools/probe/`, observation only, no game bytes embedded): `scb_probe.py` (container walk,
histograms), `scb_opstats.py` (per-opcode operand statistics), `scb_semantics.py` (`--ops` opcode contexts and
jump shapes, `--natives` argument / result data flow per native id, `--imm` immediate ranges per file,
`--handlers` natives per callback, `--params` callback parameter usage, `--messages` message ids sent versus
handled, `--find` opcode contexts, `--pseudo` folded expression listing of one file), `scb_xref.py` (native
immediate ranges against `.rhm` counts and `.red` text counts), `scb_elements.py` (element index bases from
object self-references), `scb_load_natives.py` (natives reachable from the load-time callbacks through helper calls, per
mission and per id, against the engine's known set read from `natives.rs`: `--missions`, `--ids`, `--all-unknown`,
`--context --id N`; any callback set with `--callbacks`).

## Plan

Verify the hypotheses with the original as an oracle (console `LEVEL TEXT`, the tutorial mission): which text
appears when a given scroll is taken, tick rate of `Hourglass` and native 56, the direction of 0x24 / 0x28, the
unresolved element-table base. Then specify the VM (`docs/architecture.md` script section) and the native
interface for the first mission. The community's Lua layer (Spellforge) replaces `.scb` with `.lua`, so a Lua API
compatible with theirs is the modding target; the SCB VM is needed only to run the retail campaign unchanged.

## Provenance

Container and operand encoding: structural hypothesis tested over all 39 files (`harness/tools/probe/scb_probe.py`,
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
objectives of TEXT 1000105 / 1000283). Commands: `python harness/tools/probe/scb_semantics.py <gamedir>/DATA/Levels
--ops`, `... --natives`, `... <gamedir>/DATA/Levels/H01_Lin_VL.scb --pseudo`, `python harness/tools/probe/scb_xref.py
<gamedir>/DATA/Levels <gamedir>/DATA/Text target/release/opensherwood-tools.exe`, `python
harness/tools/probe/scb_elements.py <gamedir>/DATA/Levels target/release/opensherwood-tools.exe`. Game build:
GOG English, executable SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`; data copy
`C:\Users\przem\source\gamedata\robinhood`. Confidence is stated per row; nothing in these sections is `observed`
in the ADR-0003 sense except the counts and the briefing / objective correspondence. Text sweep 2026-09-02
(review 4, findings 1 and 15): executable messages and paths, designer helper / variable / element names
removed or paraphrased; walkthrough statements annotated with their minimum dependency confidence.

Natives at load per mission and the second batch of native rows (2026-09-02, analyst session, data files only):
`python harness/tools/probe/scb_load_natives.py <gamedir>/DATA/Levels --known-rs
crates/opensherwood-core/src/natives.rs --missions --ids --all-unknown`, the same with `--callbacks
Hourglass,CheckVictoryCondition`, `--context --id ...` for the call shapes, `scb_semantics.py --natives` for the
arities and result consumers, `scb_xref.py ... 8 7 24 210 215 264 42 46 133 228 125` for the immediate ranges
against the mission tables, folded listings (`--pseudo --fn`) of the helpers named in the rows (described by
role), and a black-box run of the engine over all missions with `--lenient-natives` (500 ticks, seed 1) for the
hit / tick-1 / 500-tick columns. Helper and variable names of the scripts are paraphrased; no game text is
reproduced.
