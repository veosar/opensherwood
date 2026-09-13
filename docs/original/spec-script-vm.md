# Script VM, natives and callback scheduler (behaviour specification)

Status: `draft` (analyst session 2026-09-13, awaiting Codex review). Build: GOG, executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`. Analyst session: 2026-09-13, analyst role
(ADR-0009). Image base `0x00400000`; every address below is a virtual address in that image.

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, no identifiers
or strings of the binary, no tables copied from its data (ADR-0009). Claims carry a stable id (`VM-nnn`), a status
(`observed` = read in the executable's code, `inferred` = concluded from several observed facts, `unknown`), the
address of the function that supports it, and a confidence (high / medium / low).

## 0. Necessity record

Interoperability target: running the game's own compiled mission scripts (`.scb`, `docs/formats/scb.md`) and the
mission, map and text data they address, so that the campaign plays as shipped. The current engine
(`crates/opensherwood-core/src/natives.rs`, `docs/formats/scb.md`) rests on hypotheses for the opcode arithmetic,
the calling convention details, 168 natives (95 stubs) and the scheduler; the interoperability requires the exact
semantics. What was read, and why it was necessary:

| Functions (addresses) | Why |
|---|---|
| 00639370 run loop; 00634bf0 - 00638e90 (49 opcode handlers); 0063a140, 0063a1d0, 0063a1e0, 0063a1f0, 0063a250, 0063a320, 0063a370, 0063a3b0, 0063a400 (frame, block and parameter-buffer helpers, disassembled from the raw bytes where the decompiler dropped the object pointer) | exact opcode semantics, storage classes, frames, calling convention, native call protocol |
| 00639300, 00639360, 006392e0, 00639340, 00639280, 00639a30, 00639960, 006390b0, 00639170, 00639230, 006399a0, 00639a00 | how a callback is started, how its parameters and result travel, binding of a class to an element, instance state |
| 0063b2d0, 0063a940 (quad part), 0063a510, 0063a490, 00639f80 | which header fields the interpreter uses, the in-memory instruction layout, version check, function lookup |
| 004075c0 (raw bytes) and the 265 wrappers 00404a80 - 004075bf (raw bytes) | the native id table (id -> function), the arity, the argument order, bool conversions, the result convention |
| every native function body listed in section 5 (the id table's targets, 00570a30 - 0057c500 and the shared helpers named there) | the semantics of every id |
| 004c6ef0 (level tick), 0050f710 (main loop, timing), 0050e7b0, 0050e7d0, 0050e800, 004c3740, 004ba000, 004b9fa0, 004b9f40, 004ba4e0, 004ba760, 004ba5c0, 004ba7a0, 004ba7e0, 004b9f30 | the scheduler: which callbacks run when, the clock, the order, the win / loss checks, scroll callbacks |
| 004030a0, 00403190, 004032a0, 004033b0, 004034e0, 004035d0 - 00403ed0, 00404080, 00404180, 00404280, 00404380, 00404480, 00404570, 00404780, 00404860, 00404960, 00408660, 00408750, 00408900, 004089f0, 00408af0 | the callback invokers: parameters pushed, names looked up |
| 0046e7e0, 004a0f10, 004bb820, 00551bb0, 0057f8c0, 004bc3d0, 004105d0, 004abfe0, 0040dcb0, 00410620, 00464230, 0057fcc0, 0057fdb0, 00467230 (message case), 004bae60 and the undefined code 004bc1d0 - 004bc370 (raw bytes) | who calls each callback and with which arguments; the "current actor" and "current scroll" rules |
| 00570f00, 00571020, 00571100, 0057a020, 005866a0, 00584d60, 00585ea0, 00586ed0, 00587010, 00586e00, 00586fd0, 00585500, 00582220, 0058a3d0, 0058a940, 00582530, 00582560, 00582620, 005823f0, 0058a430, 0058bb80, 00585b70, 00585570, 00585320, 00585710, 0058ba60, 004ca410, 0053a1a0, 0053a0b0, 004646e0, 0046b210, 0058bbe0 | the sequence machinery: recording, levels (the barrier), launch, per-tick execution, completion, abortion, the level-side element kinds |
| 004e3d20, 004e3e30, 004e3e00, 004e3e20 | mission variables |
| 004e3220, 004e3260 | win / loss flags read by the tick |

Reading stopped when every opcode, every native id and the scheduling rules were settled; the actor-side
execution of movement, animation and speech elements (AI and animation subsystems) is out of scope and listed in
section 6.

## 1. Scope

The VM executes the per-class bytecode of a mission's `.scb` file: one interpreter instance per scripted element
(the level itself, actors, player characters, objects, scrolls, waypoints, script zones). The engine calls named
functions of a class ("callbacks") with a few integer parameters; the script calls the engine back through
numbered natives (0..264). This spec covers: the loader's use of the file, the instruction semantics, the calling
convention, the native call protocol, the scheduler (when each callback runs, on which clock), messages, the
sequence machinery (natives 30 / 31 / 32 and every element-recording native), and the semantics of every native
id. It hands to other specs: the element table's construction from the mission file (`docs/formats/rhm.md`,
`docs/formats/scb.md` "Index spaces"), the map's doors / patches / buildings / paths tables, the AI (alert states,
patrols, posts, subordinates), the movement and animation players that complete sequence elements, the HUD and
text pages, the campaign store (money, values, roster).

## 2. Data model

### 2.1 Program (per class, loaded once per file)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-001 | The file is accepted only if its 8-byte magic matches and its version float is exactly `1.5` (bitwise equal to the constant; the format is `docs/formats/scb.md`). Otherwise an error is raised and nothing is loaded. | observed | 0063b2d0 | high |
| VM-002 | Each 9-byte instruction of the file (`u8 op, u16 a, u16 b, u32 c`) is stored in a 12-byte slot: the opcode at byte 0, the 8 operand bytes at bytes 4..11 (`a` at +4, `b` at +6, `c` at +8), bytes 1..3 zero. Consequently a 32-bit read at +4 yields `a | (b << 16)`, and the third symbol of a three-operand instruction is the low 16 bits of `c`. | observed | 0063a940, 00639370 | high |
| VM-003 | At callback entry the interpreter reads only the function's *name* (exact, case-sensitive byte comparison with the requested name, first match in table order) and its *address* (index of the first instruction). The five header fields of the function record (`unknown_0..2`, `size_of_volatile`, `size_of_tempor`) are not used by the interpreter: the prologue instruction (0x03) carries the block sizes, and the parameter block grows dynamically. | observed | 00639300, 0063a510 | high |
| VM-004 | The class-variable block is allocated, zero-filled, with the class's `size_of_variables` bytes when the class is bound to an element (once per instance). Variables have no run-time types: every cell is 4 bytes; ints and floats share cells. | observed | 00639280 | high |
| VM-005 | The level class is the one named `StartUp`; a level without it is a fatal error at load. | observed | 004c0510 (string reference) | high |

### 2.2 Interpreter instance (one per scripted element)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-010 | Instance state: program counter (instruction index); a stack of frames; the class-variable block; a shared "global" block (storage class `00`, one per program, never addressed by the retail scripts); the *current parameter buffer* (a growable byte buffer); a native argument buffer of 12 cells (48 bytes) with a fill count; a native result register; a callback return-value register (initialised to 0 at construction and **never reset**: it keeps the last value written by any callback of this instance). | observed | 006390b0, 00639960, 00634c30 | high |
| VM-011 | Symbol operands are `u16`: bits 15..14 select the storage class (`00` global block, `01` class block, `10` current frame's locals, `11` current frame's temporaries), bits 13..0 are a byte offset into that block. No bounds are checked. | observed | 00634c30 (bytes) | high |
| VM-012 | A frame holds: the return program counter; a 4-byte *result slot* (uninitialised memory when the frame is created); the caller's parameter buffer (the one that received the caller's pushes); a locals block and a temporaries block (both allocated zero-filled by opcode 0x03, sized by its operands; re-executing 0x03 frees and re-allocates them). | observed | 0063a250, 00634d30 | high |
| VM-013 | Creating a frame (opcode 0x05 and callback entry) saves the current parameter buffer into the new frame and installs a fresh, empty one as current. Popping a frame (0x06 / 0x07) frees the saved buffer and the frame's blocks; the current buffer stays the callee's (now empty) one. Parameters therefore flow one way: pushes go into the current buffer, a call captures it, the callee reads it. | observed | 0063a250, 0063a320, 0063a3b0 | high |

### 2.3 Mission variables (natives 0, 1, 2)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-020 | One growable integer array per level, empty at level start. Native 0 with an index `k` beyond the current size grows the array to `k + 16` entries (new entries 0) and stores the value; with `k` inside, it stores. Native 1 stores only if `0 <= k < size`, else an error and no effect. Native 2 returns the value, or `-1` (not 0) for an index outside `0 <= k < size`. Negative `k` in native 0 is not checked (out-of-bounds write). | observed | 004e3d20, 004e3e30, 004e3e00, 004e3e20 | high |

### 2.4 Handles

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-030 | Elements, doors, patches, buildings, paths, locations and sound sources are *object pointers* in the original, `0` = none. Natives that take a handle usually validate it: "is a known element" means *the pointer is found in the level's element table* (linear search), which excludes player characters (they live in a separate table). Scripts cannot forge handles; a reimplementation may use table indices as long as `0` remains the null handle and player characters are distinguishable from table elements. | observed | 005709c0 | high |
| VM-031 | Element kinds are read from a type word of the element; the natives test it with masks. Classes used by the natives (bit tests, low byte): actor family = bits 1..0 == 0; animated map element = bits 1..0 == 1; cart = bits 1..0 == 2; "target" family (objects, scrolls, items) = bits 1..0 == 3; human = bits 2..0 == 4; animal = bits 2..0 == 0 (an actor that is not a human); non-player human (NPC) = bits 3..0 == 4; player character (PC) = bits 3..0 == 0xC; soldier = bits 4..0 == 0x14; civilian = bits 4..0 == 4; "target" object = bits 3..0 == 9; scroll = target family with sub-kind 0xC; bonus item = bits 3..0 == 3 within the target family. | observed | 00570a70 - 00570cb0, 0057b080, 005798f0 | high |

## 3. Behaviour

### 3.1 Instruction semantics

Notation: `S(x)` = the 4-byte cell named by symbol operand `x`; `a`, `b` = the two `u16` operands, `c` = the
`u32` operand, `c16` = its low 16 bits as a symbol; `pc` = program counter; `int` = 32-bit two's complement,
`float` = IEEE-754 single. Arithmetic wraps; comparisons of ints are *signed*; float comparisons are IEEE
(unordered → false). Unless stated, an instruction advances `pc` by 1.

| Op | Semantics | Id | Address | Conf. |
|---|---|---|---|---|
| 0x00 | Error (reported), then `pc := -1` and the loop continues: it reads the slot before the instruction array (undefined behaviour). Never emitted. | VM-040 | 00634bf0 | high |
| 0x01 | No operation. | VM-041 | 00634c20 | high |
| 0x02 | Append the 4 bytes of `S(a)` to the current parameter buffer (grows by 4). | VM-042 | 00634c30 | high |
| 0x03 | Function prologue: allocate the frame's locals block of `a` bytes and temporaries block of `b` bytes, both zero-filled (previous blocks freed). Every temporary and local therefore starts at 0 on each entry. | VM-043 | 00634d30 | high |
| 0x04 | Error (reported), then `pc += 1` (execution runs into the next function's prologue). Reached only if a function falls off its end; never happens with the retail files. | VM-044 | 00634d80 | high |
| 0x05 | Call: push a frame with return `pc + 1` (VM-013), `pc := a \| (b << 16)`. | VM-045 | 00634db0 | high |
| 0x06 | Return: pop the frame; `pc :=` its return pc. If that is `-1` the callback ends. | VM-046 | 00634dd0, 00639370 | high |
| 0x07 | Return with value: `v := S(a)`; the instance's return-value register `:= v`; if the frame depth is > 1, also the *caller frame's* result slot `:= v`; then exactly as 0x06. **Control does not continue** after 0x07 (the 0x01 / 0x06 that follow it in the files are dead code). | VM-047 | 00634de0 (bytes) | high |
| 0x08 | `S(a) :=` the 4 bytes at byte offset `c` of the frame's saved parameter buffer (parameter `k` is at `4k`). No bounds check. | VM-048 | 00634eb0, 0063a3b0 | high |
| 0x09 | The inverse: write `S(a)` into the saved parameter buffer at offset `c`. Never emitted. | VM-049 | 00634fa0 | high |
| 0x0A | `S(a) :=` the current frame's result slot (the value the last returning callee stored with 0x07; uninitialised memory if none did). | VM-050 | 00635090 | high |
| 0x0B | Append `S(a)` to the native argument buffer at index `count`, `count += 1`. No bounds check (12 cells; the retail maximum is 8). | VM-051 | 00635150 | high |
| 0x0C | Native call: `id := a \| (b << 16)`; call the native table entry `id` (no range check; 265 entries) with the argument buffer; the wrapper pops the native's arity from the buffer (`count -= arity`, arguments in push order) and the *result register := the value returned* (section 3.3). | VM-052 | 00635210 | high |
| 0x0D | `S(a) :=` the native result register. | VM-053 | 00635240 | high |
| 0x0E | `pc := a \| (b << 16)`. | VM-054 | 00635320 | high |
| 0x0F | If `S(a) != 0` (as a 32-bit word: `-0.0f` counts as true) then `pc := c` else `pc += 1`. | VM-055 | 00635330 | high |
| 0x10 | If `S(a) == 0` then `pc := c` else `pc += 1`. Never emitted. | VM-056 | 006353f0 | high |
| 0x11, 0x12 | `S(a) := S(b)` (4-byte copy; the two opcodes are identical). | VM-057 | 006354b0, 00635630 | high |
| 0x13, 0x14 | `S(a) := c` (4-byte immediate; identical for int and float bit patterns). | VM-058 | 006357b0, 006358a0 | high |
| 0x15 | `S(a) := -S(b)` (int, wraps at `INT_MIN`). | VM-059 | 00635990 | high |
| 0x16 | `S(a) := -S(b)` (float sign flip). Never emitted. | VM-060 | 00635b20 | high |
| 0x17 | `S(a) := (int) S(b)` (float → int, truncation toward zero; out of range gives `0x80000000`). Never emitted. | VM-061 | 00635cb0 | high |
| 0x18 | `S(a) := (float) S(b)` (int → float, round to nearest). | VM-062 | 00635e50 | high |
| 0x19 / 0x1A / 0x1B | `S(a) := S(b) + S(c16)` / `S(b) - S(c16)` / `S(b) * S(c16)` (int, wrapping). | VM-063 | 00635fd0, 00636210, 00636450 | high |
| 0x1C | `S(a) := S(b) / S(c16)` (int, signed, truncating; division by zero is a machine trap in the original). Never emitted. | VM-064 | 00636680 | high |
| 0x1D / 0x1E / 0x1F | `S(a) := S(b) \| S(c16)` / `& ` / `^` (bitwise). | VM-065 | 006368c0, 00636b00, 00636d40 | high |
| 0x20 / 0x21 / 0x22 / 0x23 | float `+`, `-`, `*`, `/` (`S(b) op S(c16)`, IEEE single result; the x87 computes the operation and stores a single). | VM-066 | 00636f80, 006371b0, 006373e0, 00637610 | high |
| 0x24 / 0x25 / 0x26 / 0x27 / 0x28 / 0x29 | int compare, result `1` or `0` in `S(a)`: `S(b) <= S(c16)`, `<`, `>=`, `>`, `!=`, `==` (signed). | VM-067 | 00637840, 00637a40, 00637c40, 00637e40, 00638040, 00638240 | high |
| 0x2A / 0x2B / 0x2C / 0x2D / 0x2E / 0x2F | float compare, result stored **as a float** `1.0f` or `0.0f` in `S(a)`: `S(b) <= S(c16)`, `<`, `>=`, `>`, `!=`, `==`. (A following 0x0F still branches correctly because `1.0f` is non-zero.) | VM-068 | 00638440, 00638650, 00638860, 00638a70, 00638c80, 00638e90 | high |
| ≥ 0x30 | Error (reported); `pc` unchanged: the original loops forever on such an instruction. | VM-069 | 00639370 | high |

Consequences worth stating for the implementer:

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-070 | The two retail instructions `0x0E` with `a = b = 0xFFFF` (H10, the two end-zone classes, `EnterZone`, after a native 202) jump to `pc = 0xFFFFFFFF`; the original then executes whatever 12 bytes precede the instruction array (heap memory). Behaviour is undefined in the original; the script's evident intent is to end the callback (its other paths end with return value 1). A reimplementation must treat a jump to `0xFFFFFFFF` as ending the callback and must record this as a deliberate divergence. | observed | 00639370, data (`H10_Yor_VL.scb` classes 35 and 36, instruction 31) | high (fact) / medium (intent) |
| VM-071 | Temporaries (`11`) and locals (`10`) are zero at every function entry (VM-043); class variables are zero at bind (VM-004); the result slot read by 0x0A is undefined unless the callee executed 0x07 (it always does in the retail files: 0x0A follows a call to a value-returning function in all 62 cases). | inferred | 00634d30, 0063a250 | high |
| VM-072 | Opcode 0x07 at callback depth 1 writes only the return-value register; the engine reads that register after the run (VM-090). A callback that ends with 0x06 leaves the register as it was (possibly a value from an earlier callback of the same instance). | observed | 00634de0, 00639360 | high |

### 3.2 Calling convention (script to script)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-080 | The caller pushes each argument with 0x02 (4 bytes each, in order), then 0x05. The callee reads argument `k` with 0x08 at offset `4k`. There is no argument count check: reading beyond the pushed bytes reads the buffer's slack (uninitialised). | observed | 00634c30, 0063a250, 0063a3b0 | high |
| VM-081 | A value is returned with 0x07 (VM-047) and read by the caller with 0x0A, which must directly follow the 0x05 (any intervening call would overwrite the slot). | observed | 00634de0, 00635090 | high |
| VM-082 | Frames nest without limit other than memory; recursion is allowed; every frame's locals / temporaries are fresh (VM-043). | observed | 0063a250 | high |

### 3.3 Native call protocol

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-085 | The native table has 265 entries, ids 0..264, all populated (no id in that range is missing; the table is built once, when the first interpreter instance is created). | observed | 004075c0 (bytes) | high |
| VM-086 | Each entry is a wrapper that (1) takes the top `arity` cells of the argument buffer (the last pushed is the last argument), decrements the fill count by `arity`, (2) calls the native with those arguments in push order, (3) converts the result: *void* natives yield `0`; *bool* natives yield the low 8 bits of the native's result (`0` or `1` for all such natives, except 206 / 207 / 208 whose bitwise result is truncated to 8 bits); *int / handle* natives yield the full 32-bit value. Some wrappers convert one argument to a bool before the call (non-zero → 1): natives 22, 26 (second), 36, 37 (third), 38 (second), 102 (third), 107, 115 (third), 130 (third), 131 (third), 134 (second), 138 (second), 139, 143 (second), 157 (second), 177 (second), 180 (second), 186 - 189 (second), 190 (third), 191 (**first**), 226, 244 (second), 254 (second), 257 (second), 264 (third). | observed | wrappers 00404a80 - 004075bf (bytes) | high |
| VM-087 | Natives with arity 0 are called with no argument (ids 23, 29, 30, 31, 32, 40, 54, 55, 74, 75, 106, 111, 119, 120, 121, 122, 147, 148, 159, 163, 167, 170, 171, 172, 173, 174, 211, 216, 234, 236, 238, 239, 245, 249, 251, 261). A script that pushes arguments for them leaves the cells in the buffer (the count is not reset); the buffer is only 12 cells, so a persistent imbalance overflows it in the original. The retail files are balanced for every id (arity column of section 5 equals the corpus arity). | observed | wrappers | high |
| VM-088 | The argument buffer is per instance and is *not* cleared between callbacks; nested callbacks (VM-095) push and pop symmetrically, so it stays balanced. | inferred | 006390b0, 00635210 | high |
| VM-089 | Errors inside natives (bad handle, bad index, wrong kind) are reported to the log and the native returns its failure value (given per id in section 5). Nothing stops the script; there is no trap or fault state. | observed | every native | high |

### 3.4 Running a callback

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-090 | To run callback `F` with parameters `p0..pn-1` on an instance: push `p0..pn-1` (4 bytes each) into the current parameter buffer; look the function up by name (VM-003; a missing function dereferences null in the original = crash, except the level's `PostInitialize`, which is only called if present); push a frame with return pc `-1` (capturing the buffer); `pc :=` the function's address; run until a 0x06 / 0x07 pops that frame. The caller then reads the return-value register (VM-072) when it wants a result. | observed | 00639300, 006392e0, 00639360, 00639340 | high |
| VM-091 | Callback parameter lists as pushed by the engine (parameter `k` = 0x08 offset `4k`): `Initialize` on the level: one parameter, always 0; `Initialize` on every other class: none; `PostInitialize`: none; `Hourglass` on the level: one parameter = the *hourglass count* `T / 25` (section 3.5); `Hourglass` on a scroll class: one parameter, always 0; `CheckVictoryCondition`: one parameter = the hourglass count; `Finalize`: one parameter, 0 or 1 (section 3.5); `ProcessMessage`: `(message, arg1, arg2)`; `ActionChange` (actor classes): `(current_action, previous_action)` where a missing action is 283; `FilterAIEvent`: `(actor_or_0, event)`; `ActivatedByApple/Arrow/Hand/Heal/Lever/Money/Search/Stone/Sword/Listenable`: `(actor)` = the acting actor; `IsTaken`: `(actor)`; `ReachPoint`: `(actor)`; `EnterZone` / `ExitZone`: `(actor)`; `HandleEvent`: never called by this build. | observed | 00404080, 004030a0, 00404570, 00404180, 00404960, 00404280, 00404480, 004033b0, 00403190, 004032a0, 004035d0 - 00403ed0, 00404860, 00408750, 004089f0, 00408af0; callers in section 0 | high |
| VM-092 | Results the engine reads: `CheckVictoryCondition` (0 running, 1 won, 2 lost; anything else = running); `FilterAIEvent` (0 = drop the AI event, non-zero = let it through); `IsTaken` (non-zero = the scroll is taken: its status becomes 2 and it disappears; 0 = it stays); `Initialize` of the level (read, then ignored); the nine `ActivatedBy*` results are **ignored** (the object's own effect always runs); `Hourglass`, `ProcessMessage`, `ActionChange`, `Finalize`, `EnterZone`, `ExitZone`, `ReachPoint` results are ignored. | observed | 004c6ef0, 00410620, 004ba4e0, 004bae60, 004c3740 | high |
| VM-093 | *Current actor* (native 74): a global handle set for the duration of `ProcessMessage`, `FilterAIEvent`, `ActionChange` (the actor whose class runs), `EnterZone` / `ExitZone` (the entering / leaving actor), and the nine `ActivatedBy*` (**the object itself**), and restored to its previous value afterwards. It is *not* set for `Initialize`, `PostInitialize`, `Hourglass`, `CheckVictoryCondition`, `IsTaken`, `ReachPoint`: there native 74 returns whatever was set last (null at level start). | observed | 00467230, 00410620, 0040dcb0, 00464230, 0057fcc0, 0057fdb0, 004bc1d0 (bytes), 00578050 | high |
| VM-094 | *Current scroll* (native 192): a global set to the scroll for the duration of a scroll class's `Initialize`, `Hourglass` and `IsTaken`, cleared afterwards; setting it while another is set is reported as a fatal error (never happens: scroll callbacks do not nest). Native 192 returns null in every other callback. | observed | 004ba760, 004ba5c0, 004b9fa0, 004b9f40, 004ba4e0, 005798c0 | high |
| VM-095 | Callbacks nest synchronously: natives 109 / 110 (send message now), 153 / 154 (zone leave / enter), and any sequence whose first level contains a message element run the target's `ProcessMessage` / `EnterZone` / `ExitZone` **inside the native call**, on the target's instance. If the target is the caller's own instance (the level sending to the level), the run nests on the same instance: the inner frame is pushed on top of the outer one and popped before the native returns; the interpreter's program counter is restored by the native-call instruction, so the outer callback continues correctly. Side effects on the shared registers (native result, return value) are overwritten by the inner run but re-written by the outer instruction that follows. | observed | 00635210 (pc saved in a local), 00578d80, 0058a3d0, 00582560, 00585b70, 00467230, 004ca410 | high |

### 3.5 The scheduler

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-100 | **Clock.** The level advances once per rendered frame ("level tick"). The main loop busy-waits so that a frame lasts at least 40 ms (400 ms in a debug slow-motion mode); a frame that takes longer is not caught up. The nominal script clock is therefore **25 ticks per second** (40 ms). Conversion to the engine's clocks: 1 level tick = 2.4 world ticks at 60 Hz = 2.56 animation clocks at 64 Hz; Hourglass period (25 ticks) = 1 s = 60 world ticks. | observed | 0050f710 (wait loop on the millisecond counter, constants 40 and 400) | high |
| VM-101 | The level tick is skipped (no script activity, no timers, no sequences advance) while the game is paused, while a modal page is open (text pages from natives 202 / 203, the map, dialogs), and in the game states that leave the level. | observed | 0050f710 (guards around the tick call) | high |
| VM-102 | **Tick counter** `T`: starts at 0 when the level starts, increments by one at a fixed point of every executed tick (after the Hourglass step, before the defeat checks), is never reset. | observed | 004c6ef0 | high |
| VM-103 | **Order within one level tick** (only the script-relevant steps): (1) if the *lost* flag is set: run the level's `Finalize(0)` (if the level has a script) and end the level with code 2; if the *won* flag is set: `Finalize(1)` and end with code 3; (2) pending one-shot updates; (3) if `T mod 25 == 0`: level `Hourglass(T / 25)`; then, if `(T / 25) mod 3 == 0` or the *force* flag (native 29) is set: clear the force flag, run `CheckVictoryCondition(T / 25)`; result 1 → set the *won* flag (its sub-flag is 0 when the campaign node is of type 3 or 6, else 1); result 2 → set the *lost* flag and run the loss bookkeeping; (4) `T += 1`; (5) defeat checks: no living, present player character → lost; a captured player character → lost; a dead civilian whose "may die" flag is clear → lost; (6) per-element updates in element-table order (each element's tick: actors dispatch `ActionChange`, scrolls count toward their `Hourglass`, timers of sequence elements, movement, AI ...); (7) drain the sequence manager's queue (VM-215); (8) the level's timer list: every waiting timer element (native 56) is decremented, those reaching 1 complete (VM-221). Hourglass therefore runs at T = 0, 25, 50, ...; CheckVictoryCondition at T = 0, 75, 150, ... unless forced. | observed | 004c6ef0 | high |
| VM-104 | **Initialisation order.** While the mission file is read, every scripted actor, object, waypoint and script zone binds its class and runs its `Initialize` immediately (in the order the records are read; the element table is incomplete at that point, so those callbacks should not address other elements — the retail ones are stubs). Player characters run `Initialize` when they are created. Then, just before the play loop starts: the level's `Initialize(0)`, then every scroll's `Initialize` (in scroll-table order, with the current scroll set). The level's `PostInitialize` runs once, on the first pass of the play loop, **after** the first level tick (i.e. after `Hourglass(0)` and `CheckVictoryCondition(0)`), if the class has it. | observed | 0046e7e0, 004bb820, 00551bb0, 0057f8c0, 004a0f10, 004c3740, 004ba000, 004b9fa0, 0050f710 | high (order of the level / scrolls / PostInitialize); medium (that the first tick precedes PostInitialize in every game state: see 6.3) |
| VM-105 | **Scroll `Hourglass`**: each active, scripted scroll counts its own element ticks; when its counter reaches 25 it runs `Hourglass(0)` and resets the counter. The count runs only while the scroll is active, so scrolls fire at 25, 50, ... ticks of activity, not on the level's grid. | observed | 004b9f40 | high |
| VM-106 | **Scroll `IsTaken`**: run when the "take scroll" sequence element (kind 0xAF, created by the pickup order) executes, with the taking actor; before the call the scroll's status is set to 3 and a sound is played; a non-zero result sets status 2 (taken) and hides it; zero leaves the scroll with status 3. | observed | 004ba4e0, 004ca410 | high |
| VM-107 | **`ActionChange`**: in an actor's per-element update, when the actor's current action id (283 = none) differs from the id stored at the previous check, `ActionChange(current, previous)` runs and the stored id is updated. Objects of the target family have their own dispatcher of the same shape (not read). | observed | 00464230 | high (actors) / medium (objects) |
| VM-108 | **`FilterAIEvent(actor_or_0, event)`**: run by the AI when an event is about to be applied to a scripted NPC; the first parameter is the actor involved for one class of events (source code 3), else 0; the second is the event code (a translation of the AI's internal codes: 1..; the file corpus compares with 0, 2, 8, 11, 13, 14, 22, 23, 31, 33, 34, 52, 102..106). A zero result drops the event. Two dispatchers exist (00410620 for the general events, 0040dcb0 for five codes 102..106). | observed | 00410620, 0040dcb0 | medium (the code mapping is not transcribed) |
| VM-109 | **`ReachPoint(actor)`**: run when a path follower reaches a waypoint that carries a script reference, and when an actor reaches the end of a patrol path node flagged for scripting. | observed | 004105d0, 004abfe0 | medium |
| VM-110 | **`EnterZone(actor)` / `ExitZone(actor)`**: run when an actor's sector membership changes into / out of a script zone (sector) that has a script; the zone keeps a member list; entering twice or leaving without entering is reported (and still dispatched). Natives 154 / 153 trigger the same paths. | observed | 0057fcc0, 0057fdb0 | high |
| VM-111 | **`ActivatedBy*`**: run when the player's "use tool on object" sequence element (kinds 0x93..0x9B: apple, arrow, hand, heal, lever, money, search, stone, sword, in this order) executes on a scripted object, with the acting actor as parameter and the *object* as current actor (VM-093); `ActivatedByListenable` runs from the object's own update when a "listenable" flag is set. Results ignored (VM-092). | observed | 004bc1d0 (bytes), 004bae60, 004bc3d0 | high |
| VM-112 | Every script callback is gated by the global "scripts enabled" flag (set once a level script is loaded); with it clear none of the callbacks run. | observed | 004c6ef0, 0046e7e0 and the other invokers | high |

### 3.6 Messages

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-120 | A message is `(message, arg1, arg2)`; the target is an actor-family element or **null**, and null means *the level class* (`ProcessMessage` of the level). A target of any other family is an error and the message is dropped. There is no message queue: delivery is a sequence element of kind 0xF, executed synchronously when its level runs (VM-095). Natives 43 / 44 *record* it into the sequence being recorded (error and no delivery when none is being recorded); natives 109 / 110 wrap it in a one-element sequence and deliver it **immediately, inside the native call**. | observed | 00578cb0, 00578e50, 00578d80, 00578f20, 004ca410, 00467230 | high |
| VM-121 | The message-with-arguments forms (44 / 110) pass `(message, a, b)`; the plain forms pass `(message, 0, 0)`. There is no delay parameter: the fourth argument of 44 is `arg2`, not a delay. | observed | 00578e50, 00578cb0 | high |
| VM-122 | Message ids are opaque integers; the engine itself sends none through this path (the AI/sequence code that sends messages uses ids ≥ 1000 supplied by scripts: native 71 requires `message >= 1000`). | observed | 00573f00 | high |

### 3.7 Sequences

A *sequence* is an ordered list of *elements*, each tagged with a *level* number; it is executed level by level:
all elements of a level start together, and the next level starts when every element of the current level has
finished. Recording (natives 30 / 31 / 32 and the recording natives) builds such a list from a script.

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-200 | **Recording state** (global, one at a time): the sequence being recorded, the *current level* (0 = not recording), and two "entered actors" lists used by natives 46 / 47 / 63. Native 30: if a recording is open → error, returns 0, no change; else creates an empty sequence, current level := 1, clears the two lists, returns 1. | observed | 00570f00 | high |
| VM-201 | Native 32 (barrier): if not recording → error, returns 0. Else, if the sequence has at least one element **and the last recorded element's level equals the current level**, current level += 1. Returns the current level (as a 16-bit value). Two consecutive barriers therefore count once. | observed | 00571100 | high |
| VM-202 | Native 31: if not recording → error, returns 0. Else recording ends (current level := 0); the "entered actors" lists are cleared; if the sequence has **no** element → error, the sequence is discarded, returns 1; else the sequence is handed to the manager and **started at once** (VM-210), returns 1. | observed | 00571020 | high |
| VM-203 | A recording native appends one element tagged with the current level to the sequence being recorded. If no recording is open the element is created, an error is reported and the element is discarded (no effect). This applies to every "record" native (33..42, 43, 44, 45, 48..57, 59..65, 67..73, 203, 212, 226, 243 — 45, 212, 46, 47, 63 build their elements through the movement builder which also checks the recording state). | observed | 0057a020 and the natives | high |
| VM-204 | Elements are created with a *kind* code (table in VM-230) and typed parameter slots (ints, bools, floats, points) filled by the recording native; the kind decides who executes the element and when it completes. | observed | 005866a0, 00584d60, 00585ea0, 00586ed0, 00587010, 00586e00, 00586fd0 | high |
| VM-210 | **Launch**: when a sequence is handed to the manager it is appended to the manager's sequence list and started: the elements of the first level (all elements sharing the level of the first one) are marked *running* and dispatched (VM-212). Sequences from natives 102, 109, 110 and from the engine (pickups, AI) use the same path with a single element at level 1. | observed | 0058a3d0, 00582530, 00582560, 0058a940 | high |
| VM-211 | **Level completion**: each sequence keeps a count of the elements of the current level that have not finished; an element finishing (state *done*) decrements it; at zero the next level starts (the run pointer advances past all elements of the finished level, the new level is the level of the next element, its elements are dispatched). After the last level nothing happens; the sequence object is deleted by the manager's housekeeping (every 256 ticks, a sequence none of whose elements is running or pending is removed). | observed | 00582620, 00582560, 005823f0, 0058a430, 004c6ef0 | high |
| VM-212 | **Dispatch of an element** at level start: (a) kinds 4, 5, 7, 0xB, 0x4F, 0x50, 0xAF are executed by the *level executor* immediately; (b) kind 0xF (message) with an actor target is executed by the actor's message path immediately, with a null target by the level executor immediately; (c) kinds 0x18, 0x7F, 0x80, 0x81, 0x92, 0x9C, 0x9D, 0x9E, 0x9F, 0xAA, 0xAB are executed by the *target actor* immediately (its element executor); (d) every other kind is appended to the manager's FIFO queue. | observed | 00582560, 0058bb80, 00585b70 | high |
| VM-215 | **Queue drain** (step 7 of the tick): each queued element in FIFO order is removed and, if still pending, given to its target: an actor target's "receive element" (VM-216), a null target → the level executor. A message handler or native that starts a sequence during the drain appends to the same queue and is drained in the same pass. | observed | 0058ba60, 00585570, 004c6ef0 | high |
| VM-216 | **Actor receives an element**: the actor's own admission tests run (its state, the element's kind and parameters; e.g. a dead or absent actor refuses); if refused, the element gets state *refused* (5). If accepted: if the actor is idle the element becomes its current action and starts; otherwise it is merged / queued according to the actor's own priority rules (AI subsystem; not read). Completion is reported by the actor when the action ends (arrival, animation end, speech end): the element gets state *done* and VM-211 applies. | observed | 004646e0, 0046b210 (structure only) | medium |
| VM-217 | **Abort cascade**: when an element becomes *refused* (5) or *cancelled* (6), the first element of the **next** level is set to the same state, and from there every following element of the sequence, whatever its level. Elements of the same level as the refused one are not touched. The refused element never reports *done*, so its level never completes: the remainder of the sequence is dropped. (Same-level siblings still finish normally.) | observed | 00585320 | high |
| VM-218 | Kinds executed by the level executor and their completion: 4 / 5 (lock / unlock player input, HUD events 0x1C / 0x1D): done at once. 7 (camera jump to point): camera set, done at once. 6 (camera scroll to point with an optional speed): the element becomes the *camera element*; done when the camera scroll ends, or at once when the level's "no cinematic camera" flag is set; a new camera element cancels (marks done) the previous one. 8 (zoom to a factor): sets the zoom target and becomes the camera element (completion by the camera code, not read). 9 (map display on/off): done at once. 0xB (timer): appended to the level's timer list (VM-221). 0xC (dialog page `k`): the dialog is shown unless the "no presentation" flag is set; done at once; HUD event 0x2E. 0xD / 0xE (camera lock on an actor / clear lock): done at once. 0xF (message): `ProcessMessage` of the level, done at once. 0x10 (popup text `k`, native 203): the page is opened unless the "no presentation" flag is set; done at once — but the page is modal, so the level tick stops (VM-101) until it is dismissed: the following level effectively waits. 0x11 / 0x12 (freeze-all flag := bool): done at once. 0x4F (action `k` available := bool for a PC; HUD events 0x1A / 0x1B), 0x50 (character available := bool; HUD events 0xB / 0xC): done at once. 0xAF (take scroll → `IsTaken`, VM-106): done at once. | observed | 004ca410, 0053a1a0, 0050f710 | high |
| VM-221 | **Timer** (native 56, kind 0xB, argument `n`): each executed level tick (step 8) every timer in the list is visited: if its counter equals 1 it is *done* and removed; otherwise the counter is decremented. A timer of `n ≥ 1` completes at the `n`-th tick after the level started counting (n = 1: the first tick), i.e. `n × 40 ms` nominal. `n ≤ 0` never completes (the counter runs negative). | observed | 004c6ef0 | high |
| VM-222 | Sequence elements are not executed while the level tick is skipped (VM-101): pages and dialogs freeze everything. | inferred | 0050f710 | high |
| VM-230 | **Element kinds recorded by natives** (kind: native → executor): 4: 54, 5: 55, 6: 33 (speed 0 = default) / 42 (speed), 7: 34, 8: 35, 9: 36, 0xB: 56, 0xC: 41, 0xD: 39, 0xE: 40, 0xF: 43 / 44 (level) / 109 / 110 (immediate), 0x10: 203, 0x11: 226, 0x14: 45 / 46 / 47 / 212 (movement, actor), 0x15: 57 / 70 / 71 (seek actor, actor), 0x1A: 48 (turn to point, actor), 0x1D / 0x20 / 0x22 / 0x32 / 0x33 / 0x34 / 0x3B..0x43 / 0x4D / 0x87 / 0x88 / 0xA1 / 0xB2: 59 (actions, actor), 0x26: 102 (inflict pain, actor), 0x4F: 37, 0x50: 38, 0x65: 63 (take corpse), 0x66: 65 (leave corpse), 0x7F: 52 (lock AI), 0x80: 53 (unlock AI), 0x81: 243 (un-blip), 0x92: 62 / 69 (speak), 0x9C / 0x9D / 0x9E / 0x9F: 67 / 68 / 72 / 73 (mobile element start / stop / activate / deactivate, target = player character `i`), 0xA4 / 0xA5 / 0xA6: 49 / 50 / 51 (animation once / loop / freeze), 0xAA / 0xAB: 60 / 61 (replace / restore animation). | observed | the natives of section 5 | high |
| VM-231 | Actor-side immediate kinds (VM-212 c) are done as soon as the actor executes them (0x7F / 0x80 lock / unlock, 0x81 un-blip, 0xAA / 0xAB animation table swap, 0x9C..0x9F mobile element flags); 0x92 (speak) and 0x18 (a movement variant used by the engine) end when the actor finishes. Queued kinds end when the actor's action ends (walks: arrival; animations: end of the clip / loop; actions: end of the action). The exact end conditions belong to the AI / animation specs (section 6). | observed (dispatch) / unknown (durations) | 00467230, 004646e0 | medium |

### 3.8 Objectives, debriefing, win and loss

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-240 | Native 26 `(k, main)` adds objective `k` (bool `main`) to the objective display; 27 `(k)` marks it accomplished. Both just forward to the objectives store (index into the level's short-briefing texts, `docs/formats/scb.md`). | observed | 005785b0, 005785d0 | high |
| VM-241 | Native 28 `(k)` stores `k` as the debriefing variant used when the level ends (read by the main loop at level end). | observed | 00570a30, 0050f710 | high |
| VM-242 | Native 29 sets the *force victory check* flag: the next Hourglass step (VM-103 step 3) runs `CheckVictoryCondition` even if `(T/25) mod 3 != 0`. It does not run it immediately. | observed | 00578c60, 004c6ef0 | high |
| VM-243 | `Finalize(1)` runs on the tick after the *won* flag was set, `Finalize(0)` after the *lost* flag (VM-103 step 1); the level then ends. Winning may also come from native 178 (banner capture reaching the required count in a type-1 campaign node). | observed | 004c6ef0, 004e3220, 005795c0 | high |

## 4. Constants

| Name (ours) | Value | Unit | Where it comes from | Confidence |
|---|---|---|---|---|
| minimum frame time (script tick) | 40 | ms | 0050f710 (busy wait) | high |
| slow-motion frame time (debug mode) | 400 | ms | 0050f710 | high |
| Hourglass period | 25 | level ticks | 004c6ef0 | high |
| victory check period | 3 | hourglass counts | 004c6ef0 | high |
| sequence housekeeping period | 256 | level ticks | 004c6ef0 (low byte of T == 0) | high |
| scroll Hourglass period | 25 | scroll ticks (while active) | 004b9f40 | high |
| native table size | 265 | ids (0..264) | 004075c0 | high |
| native argument buffer | 12 | cells of 4 bytes | 006390b0 | high |
| instruction slot in memory | 12 | bytes | 0063a940 | high |
| file version accepted | 1.5 | float, exact | 0063b2d0 | high |
| mission variable growth | k + 16 | entries | 004e3d20 | high |
| missing action id (`ActionChange`) | 283 | action id | 00464230, 00578060 | high |
| "no posture" / "no state" answer of 91 and 259 | 666 | code | 005762d0, 005766f0 | high |
| zoom factors accepted by 21 / 35 | 0.5, 1.0, 2.0 | camera zoom | 00571200, 00572d50 | high |
| speak / remark id limit (69, 264) | 120 (ids 0..119) | remark id | 00572ef0, 00573010 | high |
| door search radius of native 64 | 300 (squared 90000) | map units | 00574860 | high |
| campaign value window of 195 / 196 | k in 0..19 → slots 7..26 | index | 00579430, 00579470 | high |
| NPC custom values (197 / 198) | k in 0..9 | index | 005794b0, 00579520 | high |
| scroll status range (194) | 0..3 | status | 00579950 | high |
| minimum custom message id (71) | 1000 | message id | 00573f00 | high |
| team size limit when no mission is selected (174) | 5 | characters | 00579300 | high |
| default zoom set by 18 | 2.0 | camera zoom | 00571270 | high |

## 5. Natives by id

Columns: `Args` = the arguments in push order (`b` = converted to bool by the wrapper, VM-086); `→` = result
convention (`void` = 0, `bool` = 0/1, `int`, `handle`); `Records` = creates a sequence element (kind, VM-230) in
the open recording; addresses are the native's function (the wrapper is `00404a80 + ...`, VM-085). Confidence is
high unless marked. "Error" means: reported to the log and the stated failure value is returned; "actor family",
"human", "NPC", "PC", "soldier" as in VM-031. `Location` = a location handle (native 6 / 95 / 213): a *point* is
a location whose polygon count is zero; a *sector* (script zone) is a location of the zone kind.

| Id | Args | → | Behaviour | Address |
|---|---|---|---|---|
| 0 | `k, v` | void | Declare / set mission variable `k` := `v`, growing the array (VM-020). | 00577090 |
| 1 | `k, v` | void | Set mission variable `k` := `v`; error if `k` outside the array (no effect). | 005770b0 |
| 2 | `k` | int | Mission variable `k`; `-1` if outside the array (error). | 00577100 |
| 3 | `i` | handle | Element `i` of the level's element table (count truncated to 16 bits); `i` in `[N, N + P)` → player character `i - N`; `-1` → null (no error); other → error, null. | 00571590 |
| 4 | `i` | handle | Door `i` of the map's door table; `-1` → null; out of range → error, null. | 005716a0 |
| 5 | `i` | handle | Patch `i` of the map's patch table; same rules. | 00571700 |
| 6 | `i` | handle | Location `i` of the level's location table (`i` taken modulo 65536, no upper check); `-1` → null; an empty entry → error, null. | 005714f0 |
| 7 | `k` | handle | Sound source `k` of the level; null with an error if the level has sound and the source does not exist; null without error when the level has no sound. | 005755f0 |
| 8 | `i` | handle | Building `i` of the map's building table, **no check at all**: `-1` reads the word before the table (undefined in the original; the scripts use it as "outdoors", see 6.2). | 00571760 |
| 9 | `i` | handle | Patrol path `i` (`i` modulo 65536, no upper check); `-1` → null. | 00571780 |
| 10 | `e` | int | Index of `e` in the element table; if not there, its index **within the player-character table** (not offset by N); `-1` if in neither. | 00579bd0 |
| 11 | `door` | int | Index in the door table or `-1`. | 00579c90 |
| 12 | `patch` | int | Index in the patch table or `-1`. | 00579d00 |
| 13 | `x` | int | Index of `x` in a fourth per-map table (probably the sectors); `-1` if absent. Whether this inverts native 6 for polygon locations is open (6.2). | 00579d70 (medium-low) |
| 14 | `sound` | int | Index of the sound source or `-1` (error); `-1` without error when the level has no sound. | 00579de0 |
| 15 | `building` | int | Index in the building table or `-1`. | 00579e80 |
| 16 | `path` | int | A 16-bit property of the path (unknown meaning; 3 uses stored to locals). | 00579ef0 (unknown) |
| 17 | `x` | void | Forwards `x` to a level routine of unknown effect (1 use). | 005714a0 (unknown) |
| 18 | `loc` | bool | Camera jumps to the point `loc` and the zoom target is set to 2.0; null → error, 0. Returns 1. | 00571270 |
| 19 | `loc, f` | void | As 18 with zoom target `f`. | 00571330 |
| 20 | `loc` | void | Camera jumps to the point `loc` and the camera-follow flag is cleared; null → error. | 005713f0 |
| 21 | `f` | void | Set the zoom factor; only 0.5, 1.0, 2.0 accepted (error otherwise). | 00571200 |
| 22 | `b` | void | Show (`b` = 1) / hide the map display. | 005714b0 |
| 23 | – | void | A screen transition (HUD event 0x24 with a fade); unused. | 00576170 (medium) |
| 24 | `e, code` | void | Minimap dot of element `e`: `code` 0 / 1 = default dots; 100..102, 200..202, 300..302 = green / red / blue dot styles; 111 / 222 / 333 = the same for humans only. Any other code (the retail 444 included) → error, **no change**. | 005788a0 |
| 25 | `sector, f` | void | Force the "emergency box" of a motion-area sector (radius `f`); error if not such a sector; unused. | 00578c70 |
| 26 | `k, b` | void | Add objective `k`, primary if `b`. | 005785b0 |
| 27 | `k` | void | Objective `k` accomplished. | 005785d0 |
| 28 | `k` | void | Debriefing variant := `k` (VM-241). | 00570a30 |
| 29 | – | void | Force the next victory check (VM-242). | 00578c60 |
| 30 | – | bool | Begin recording (VM-200). | 00570f00 |
| 31 | – | bool | End recording and launch (VM-202). | 00571020 |
| 32 | – | int | Barrier: bump the level, return it (VM-201). | 00571100 |
| 33 | `loc` | bool | Records kind 6 (camera scroll to point, default speed); requires recording and a point (else error, 0). | 00572ab0 |
| 34 | `loc` | bool | Records kind 7 (camera jump to point); same checks. | 00572ba0 |
| 35 | `f` | bool | Records kind 8 (zoom to `f`, one of 0.5 / 1 / 2). | 00572d50 |
| 36 | `b` | bool | Records kind 9 (map display := `b`). | 00572e40 |
| 37 | `pc, k, b` | bool | Records kind 0x4F (action `k` available := `b` for player character `pc`); `pc` must be actor family (else error 0). | 00577d30 |
| 38 | `pc, b` | bool | Records kind 0x50 (character `pc` available := `b`); actor family required. | 00577e70 |
| 39 | `actor` | void | Records kind 0xD (camera locks on `actor`); actor family required. | 00577f30 |
| 40 | – | void | Records kind 0xE (clear the camera lock). | 00577fe0 |
| 41 | `k` | void | Records kind 0xC (dialog page `k`). | 00572a20 |
| 42 | `loc, speed` | bool | Records kind 6 with scroll speed `speed`. | 00572c70 |
| 43 | `target, msg` | void | Records kind 0xF: message `(msg, 0, 0)` to `target` (null = level; non-actor → error, dropped) (VM-120). | 00578cb0 |
| 44 | `target, msg, a, b` | void | Records kind 0xF: `(msg, a, b)`. | 00578e50 |
| 45 | `actor, loc, mode` | bool | Records a movement (kind 0x14) of `actor` (actor family) to point `loc`: mode 0 / 2 = walk, 1 / 3 = run; modes 2 / 3 additionally mark the added elements with a variant flag (6). Errors (no recording, null / non-point location, non-actor, bad mode) → 0. Completes on arrival (actor side). | 005740b0 (medium for modes 2/3) |
| 46 | `actor, loc, dir, b` | bool | "Enter the game": if `actor` is not yet in the recording's entered list, it is placed at point `loc` facing `dir` (−1 = the actor's own direction) and registered; then a movement (walk if `b` = 0, run otherwise) is recorded at the current level. Returns 0 (the wrapper's bool sees 0 even on success). | 00577650 (medium) |
| 47 | `actor, loc, dir, b` | bool | Variant of 46 (position taken from the actor when already registered; facing `(dir − 8) mod 16`); returns 1 on success. | 005779a0 (medium) |
| 48 | `actor, loc` | bool | Records kind 0x1A: `actor` turns to face point `loc`. | 00574e70 |
| 49 | `x, anim` | bool | Records kind 0xA4: play animation `anim` once on `x` (actor family or a target object). | 00573190 |
| 50 | `x, anim` | bool | Records kind 0xA5: loop animation `anim` on element `x` (any known element). | 00573280 |
| 51 | `x, anim` | bool | Records kind 0xA6: play `anim` and freeze on its last frame (actor family or target object). | 00573370 |
| 52 | `actor` | bool | Records kind 0x7F: lock the AI of `actor` (must be a known actor). | 00575120 |
| 53 | `actor` | bool | Records kind 0x80: unlock the AI. | 005751d0 |
| 54 | – | bool | Records kind 4: lock player input. | 005753d0 |
| 55 | – | bool | Records kind 5: unlock player input. | 00575470 |
| 56 | `n` | bool | Records kind 0xB: wait `n` level ticks (VM-221). | 00575350 |
| 57 | `actor, target, k, v` | bool | Records kind 0x15: `actor` seeks `target` (walk if `k` = 1 else run), parameter `v`. | 00573d10 (medium) |
| 58 | `x` | bool | Always 0 (placeholder). | 006390a0 |
| 59 | `actor, code, arg` | bool | Records an action of `actor` (known element required): code 0 → kind 0xA1; 1 → kind 0x1A with direction `arg mod 16` (turn); 2 → 0x1D; 3 → 0x22; 4 → shoot at element index `arg` (kind 0xB2; `arg` must be a valid element index); 5 → begin sword fight with element `arg` (kind 0x32); 6 → 0x33; 7 → 0x34; 8..16 → sword actions 0x3B..0x43 against element `arg`; 17 / 18 → look sideways (soldier only, kinds 0x87 / 0x88); 19 → 0x20; 20 → 0x4D. Other codes → error, 0. | 00573450 |
| 60 | `actor, a, b` | bool | Records kind 0xAA: replace animation `a` by `b` in the actor's table (actor family). | 00574f80 |
| 61 | `actor, a` | bool | Records kind 0xAB: restore animation `a`. | 00575050 |
| 62 | `pc, text, flag` | bool | Records kind 0x92 (speak) for a **player character** with text `text` and flag `flag`; non-PC → error 0. | 005730b0 |
| 63 | `actor, corpse, b` | bool | Records the "take corpse" sequence (a movement to `corpse` then kind 0x65): `actor` must be a PC with a carrying skill, `corpse` an actor. | 00574a70 (medium) |
| 64 | `actor, loc, mode` | bool | Walk into a building: among the map's doors of type 1 the one nearest to point `loc` within 300 units is chosen and native 45 is recorded to its position; no door → error, 0. | 00574860 |
| 65 | `actor` | bool | Records kind 0x66: leave the carried corpse (PC with the skill). | 00574db0 |
| 66 | `x` | void | Resets an animated map element's animation (type bits 1..0 == 1); error otherwise. Unused. | 00570dd0 (medium) |
| 67 / 68 | `i` | void | Records kind 0x9C / 0x9D (start / stop "mobile element") targeting **player character `i`** (index into the PC table, no check). | 005783b0, 00578430 |
| 69 | `actor, k` | bool | Records kind 0x92 (speak remark `k`, 0..119) for a human; `k` ≥ 120 → error 0. | 00572ef0 |
| 70 | `actor, target, k, range, reporter, msg` | void | Records kind 0x15 (seek `target`, walk if `k` = 0 else run, float `range`) with an attached one-element sub-sequence: when the seek ends, message `(msg, 0, 0)` is sent to `reporter` (null = level). `reporter` of a non-actor family → error, nothing recorded. | 00573da0 |
| 71 | `actor, target, k, range, reporter, msg, a, b` | void | As 70 with `(msg, a, b)`; `msg < 1000` → error, nothing. | 00573f00 |
| 72 / 73 | `i` | void | Records kind 0x9E / 0x9F (activate / deactivate mobile element) targeting player character `i`. | 005784b0, 00578530 |
| 74 | – | handle | The current actor (VM-093). | 00578050 |
| 75 | – | int | Number of elements in the element table (player characters excluded). | 005714d0 |
| 76 | `x` | bool | `x` is an animated map element. | 00570a70 |
| 77 | `x` | bool | `x` is of the target family (objects, scrolls, items); null → 0; unknown element → error 0. | 00570ae0 |
| 78 | `x` | bool | `x` is of the actor family (same error rule). | 00570a90 |
| 79 | `x` | bool | `x` is a player character. | 00570b80 |
| 80 | `x` | bool | `x` is an NPC (non-player human). | 00570c10 |
| 81 | `x` | bool | `x` is a soldier. | 00570c60 |
| 82 | `x` | bool | `x` is a civilian. | 00570cb0 |
| 83 | `x` | bool | `x` is an animal (actor that is not human); null → error 0. | 00570bd0 |
| 84 | `x` | bool | `x` is a cart. | 00570b30 |
| 85 | `x` | bool | `x` is null. | 00570a50 |
| 86 | `a, b` | bool | `a == b` (handle identity). | 00570a60 |
| 87 | `x` | bool | Human `x` is dead (the actor's own "dead" test); non-human → error 0. | 00579a70 |
| 88 | `x` | bool | Human `x` is knocked out (its knock-out flag). | 00579ac0 |
| 89 | `x` | bool | Human `x` is tied up (action state 18). | 00579b10 |
| 90 | `x` | bool | Human `x` is dead, tied up or knocked out. | 00579b60 |
| 91 | `x` | int | Posture code of human `x` from its movement state: 1 → 0; 3 → 2 (17 if knocked out); 4 → 4; 5 → 9; 6 → 5; 7 → 6 if life > 0 else 15; 8 → 16; 9 → 8; 10 → 10; 11 → 11; 12 → 15; other states → 666; error → −1. | 005762d0 |
| 92 | `x, k` | void | Set posture of human `x`: 0 (get up: from knock-out state 3 it stands; from state 11 (tied) untie), 2, 7 (lie down, with a knock-out for NPCs), 10, 15, 16, 17, 100 (crouch/flee variant); 4, 5, 6, 8, 9, 11 → error "cannot set"; others → error. | 005763e0 (medium) |
| 93 | `x` | int | Facing direction 0..15 of a known element. | 00571a70 |
| 94 | `x, d` | bool | Set facing `d mod 16` (carts use their own setter); for target objects also flags a redraw. | 00571ab0 |
| 95 | `x` | handle | A new point location at `x`'s position (with its sector); unknown element → error, null. | 005717a0 |
| 96 | `x, loc` | bool | `loc` null: take `x` off the map (inactive, "off-map" flag; humans stop; a present PC is removed from play; an unlocked NPC has its AI locked). Otherwise `loc` must be a point (error 0) whose sector allows standing (else error 0 after the sector was already changed); `x` is put back on the map if it was off, moved there, its movement stopped. Returns 1. | 005718d0 |
| 97 | `x, zone` | bool | `x` is inside script zone `zone` (a sector location; else error 0); an inactive target-family element is never inside. | 00571de0 |
| 98 | `x, building` | bool | `building` null → `x`'s sector is outdoors (its outdoor flag); else → `x`'s sector is `building`. No validation of `x`. | 00577cf0 |
| 99 | `x` | void | Un-blip (clear the highlight mark) of `x` if it is marked; returns 1 when it did. | 00570e70 |
| 100 | `x` | void | `x`'s movement style == 3 (a boolean; unused). | 00571550 |
| 101 | `x` | int | Current action id: target family → its stored action; actor family → the action id of its current action, 283 if none; others → error 0. | 00578060 |
| 102 | `x, amount, b` | void | Inflict `amount` damage on actor `x` (must be in the element table) with intensity flag `b` (100 if set else 0), through a one-element sequence (kind 0x26) executed on the next queue drain. | 00577500 |
| 103 | `x` | void | Stop actor `x` (mode 6); non-actor → error. | 00571b30 |
| 104 | `npc, x` | bool | NPC `npc` sees human `x`. Unused. | 00578160 |
| 105 | `npc` | void | Enable the view-cone display of `npc`. Unused. | 005786b0 |
| 106 | – | bool | A HUD flag. Unused. | 00578820 |
| 107 | `b` | void | Set that HUD flag (event 0x27 when it changes). Unused. | 00578830 |
| 108 | `x, a, b` | bool | Runs `FilterAIEvent(a, b)` directly and returns whether it was non-zero. Unused. | 00578690 (medium) |
| 109 | `target, msg` | void | Send `(msg, 0, 0)` **now** (VM-120). | 00578d80 |
| 110 | `target, msg, a, b` | void | Send `(msg, a, b)` now. | 00578f20 |
| 111 | – | handle | **Always null** in this build (a placeholder). | 00579a20 |
| 112 | `k` | bool | Selection: 31 → select all (HUD event 0xD), 0 → select none (event 0x12); other → error. Returns 1. | 00578710 |
| 113 | `x` | bool | Deactivate: cart → its own deactivation; PC → removed from play (its three HUD slots cleared); every other element → active flag := 0. Unknown element → error 0. | 00575640 |
| 114 | `x` | bool | Activate (the inverse); null or unknown → error 0. | 005756f0 |
| 115 | `pc, k, b` | bool | Action `k` (0..5) of player character `pc` available := `b` (HUD events 0x1D / 0x1C); non-PC or `k` out of range → error 0. | 00575760 |
| 116 | `pc, k` | bool | Action `k` is available for `pc` (both of its disable flags clear). | 00575870 |
| 117 | `x, prop, v` | bool | Set property: 0 arrows (human; 0 and no effect without a quiver), 1 money (NPC), 2 life points (human), 3 concussion (human), 4 purses, 5 stones, 6 apples, 7 ales, 8 legs, 9 plants, 10 nets, 11 wasp nests (PC inventory counters), 12 name preset 0 / 1 / 2 (PC). Wrong kind / unknown property → error 0. | 00572300 |
| 118 | `x, prop` | int | Get property (same numbering; 0 arrows: 0 without a quiver; 2 life as a signed 16-bit value; 3 concussion; 4..11 as 16-bit counters); errors → −1. | 00571fb0 |
| 119 | – | bool | Some civilian in the level's element list is dead. | 005761f0 |
| 120 | – | bool | Some soldier is dead. Unused. | 00576260 |
| 121 / 122 | – | int | Highest alert state (0..2) over living soldiers / over living non-soldier NPCs. Unused. | 00576a30, 00576af0 |
| 123 | `npc, s` | bool | Set alert state `s` (0, 1, 2) of an NPC; PC or other → error 0. | 00575af0 |
| 124 | `npc` | int | Alert state 0..2; non-NPC → error 0. | 00575bf0 |
| 125 | `npc, s` | bool | Set AI sub-state: 1 (idle/patrol), 3 (seek; soldiers only), 5 (mapped to the internal state 6), 7 (return to post); 0, 4, 6 cannot be set (error); others error. | 00575c80 (medium) |
| 126 | `npc` | int | AI state code: internal states 1..6 map to 1, 2, 3, 6, 4, 5; else 0. | 00575e20 |
| 127 | `a, b` | void | Obsolete: always an error. | 00575ee0 |
| 128 | `npc` | bool | The NPC's attitude code is 1 (hostile); non-NPC → 0. | 00575f00 |
| 129 | `npc, a, b` | void | No effect (returns 1 for an NPC). Unused. | 00575f70 |
| 130 | `npc, target, b` | void | `npc` stares at actor `target` (a known element) with flag `b`. | 00575fc0 |
| 131 | `npc, loc, b` | void | `npc` stares at point `loc`. | 005760b0 |
| 132 | `npc, path` | void | Assign patrol path (handle from 9). | 00576c50 |
| 133 | `npc, loc, d` | void | Set the NPC's post at point `loc` facing `d`. | 00576cc0 |
| 134 | `x, b` | void | Lock AI: NPC → its AI lock with flag `b`; animal → its lock flag := 1; PC → error. | 00576d80 |
| 135 | `x` | void | Unlock AI: NPC (error if not locked); animal → unlock and wake; PC → error. | 00576e00 |
| 136 | `soldier, k` | void | Force a battle decision: `k` 0..16 → internal codes 3..19, 99 → none; `k ≥ 100` means `k − 100` as a temporary decision. Errors otherwise. | 00576eb0 (medium) |
| 137 | `loc, k` | void | Make a noise at point `loc`: `k` 0 → noise type 11, 1 → type 12 (other `k` → error, then used as the type); null location → error. | 00571160 |
| 138 | `human, b` | void | Freeze flag := `b` (NPCs and other humans keep it in different fields). | 00576bb0 |
| 139 | `b` | void | Level "freeze all" flag := `b` (immediate form of 226). | 00576c30 |
| 140 | `npc, k` | void | Path walking style: 0 walk, 1 run (other values passed through). | 005780d0 |
| 141 | `soldier` | int | Rank. | 00579a30 |
| 142 | `x` | bool | Animated map element active flag. | 00570d00 |
| 143 | `x, b` | bool | Animated map element active := `b`; returns 1 on success. | 00570d80 |
| 144 | `patch` | bool | Patch active flag (**no null check**). | 00570e20 |
| 145 / 146 | `patch` | bool | Activate / deactivate patch; clears the camera-follow flag. Returns 1. | 00570e30, 00570e50 |
| 147 | – | bool | Stop all playing level sounds. | 00575510 (medium) |
| 148 | – | bool | Restart the level's ambient sounds around the camera. | 00575530 (medium) |
| 149 | `sound` | bool | Start sound source `sound` (once; ignored if already started). | 00575560 (medium) |
| 150 | `sound` | bool | Stop the playing instance of `sound`. | 00575590 (medium) |
| 151 | `sound` | bool | Stop and release `sound`. | 005755c0 (medium) |
| 152 | `x` | void | Take actor `x` (element-table actor) out of its building; error if outdoors. | 00577150 |
| 153 | `x, zone` | void | Remove `x` from script zone `zone` (must be a member) → runs `ExitZone(x)`. Unused. | 00577220 |
| 154 | `x, zone` | void | Add `x` to `zone` → runs `EnterZone(x)`. Unused. | 00577390 |
| 155 | `x` | void | No effect. | 00578c50 |
| 156 | `x, building` | void | Put actor `x` inside `building`: position := the building's entry point, sector := the building, default facing, hidden (active flag 0), registered inside; a PC gets its own extra handling. | 005781f0 |
| 157 | `x, b` | void | Sets a flag `b` on an object and on each entry of its list (a lift / mechanism; unread). Unused. | 005782a0 (unknown) |
| 158 | `zone` | handle | First actor inside `zone`, or null. Unused. | 00571ea0 |
| 159 | – | handle | **Always null** (same placeholder as 111). | 00579a20 |
| 160 | `a, b` | int | Distance between two points (float distance truncated); a non-point → error 0. | 00571b80 |
| 161 | `n` | int | `rand() mod n` with the C runtime's 15-bit generator; `n` = 0 traps. | 00578680 |
| 162 | `x` | void | Forwards `x` to a sound/notification routine (unknown). Unused. | 00579f10 (unknown) |
| 163 | – | int | Size of the campaign's character roster list. | 005791d0 |
| 164 | `i` | handle | Element for roster entry `i` (no check). | 005791f0 |
| 165 / 166 | `pc` | void | Add / remove `pc` to / from the team of the next mission (HUD refreshed); non-PC → error. 165 also marks the PC as present and resets two of its fields. | 00579210, 00579280 |
| 167 | – | int | Size of the selected mission's team list. | 005792d0 |
| 168 | `i` | handle | Element of team entry `i` (fatal error if out of range). | 00579350 |
| 169 | `x` | bool | `x`'s campaign character id is in the selected mission's team list. | 005793a0 |
| 170 | – | bool | The team satisfies the selected mission's requirements (campaign routine). | 005793e0 (medium) |
| 171 | – | int | First code of the campaign's "available missions" list, 0 if empty. | 005793f0 (medium) |
| 172 | – | int | Code of the selected mission (first entry of the selection list), 0 if none. | 00579410 |
| 173 | – | bool | A campaign state byte (hub helper). | 005795b0 (unknown) |
| 174 | – | int | Team size limit of the selected mission; 5 if none is selected; error 0 outside the hub (campaign node type 8). | 00579300 |
| 175 | `id, loc` | void | Place the PC whose campaign id is `id` at point `loc`. | 00579000 |
| 176 | `soldier, n` | void | Company number := `n`. | 005790b0 |
| 177 | `soldier, b` | void | Always-attentive mode := `b`. | 005790f0 |
| 178 | `banner` | void | Capture a banner (must be active, else error): deactivated; campaign counter 3 += its value; in a type-1 node, reaching the node's required count wins the mission; HUD refresh. | 005795c0 |
| 179 | `banner` | void | Lose a banner (inactive → active, counter 3 decreased). | 005796d0 |
| 180 | `human, b` | void | Invisible flag := `b`. | 00579760 |
| 181 | `human` | bool | Invisible flag. | 005797c0 |
| 182 / 183 / 184 / 185 | `door` | bool | Door state bytes A / D / C / B (four independent flags; A is the "locked" one the scripts poll). **No null check.** | 00579810 - 00579840 |
| 186 / 187 / 188 / 189 | `door, b` | void | Set door state byte A / D / C / B := `b`; A, C, B also mark the door for refresh when `b` = 0. No null check. | 00579850 - 005798a0 |
| 190 | `x, a, b` | void | Forwards `(a, b)` to a door/lift routine (unread). Unused. | 005786f0 (unknown) |
| 191 | `b, door` | void | Door's mechanism "activated" byte := `b`; null door → error. | 005787f0 |
| 192 | – | handle | The current scroll (VM-094). | 005798c0 |
| 193 | `scroll` | int | Scroll status 0..3; not a scroll → error 0. | 005798f0 |
| 194 | `scroll, s` | void | Status := `s` (0..3; else error). Status 1 and 3 make the scroll visible, 0 and 2 hide it; 3 also plays the "scroll" sound. | 00579950 |
| 195 | `k` | int | Campaign value `k + 7` for `k` in 0..19; else error 0. | 00579430 |
| 196 | `k, v` | void | Set campaign value `k + 7`. | 00579470 |
| 197 | `npc, k` | int | NPC custom value `k` (0..9); errors → −1. | 005794b0 |
| 198 | `npc, k, v` | void | Set it. | 00579520 |
| 199 | `k, loc, n` | void | Camp production zone `k` at `loc` with capacity `n`. | 005799d0 (medium) |
| 200 | `k, loc` | void | Camp production zone `k` at `loc`. | 00579a00 (medium) |
| 201 | `id` | handle | The player-character slot with campaign id `id`; `id ≥` the slot count → error null; none → null. | 00571610 |
| 202 | `k` | void | Open popup text page `k` **now** (modal; HUD event 0x2E; the level tick stops until dismissed). | 00579f30 |
| 203 | `k` | void | Records kind 0x10 (popup text `k`, VM-218). | 00579fa0 |
| 204 | `zone` | int | Number of actors inside `zone` (sector; else error 0; null → 0). | 0057a060 |
| 205 | `zone, i` | handle | `i`-th actor inside (`i <` count) else error null. | 0057a0a0 |
| 206 / 207 / 208 | `a, b` | bool | `a & b`, `a \| b`, `a ^ b`, **truncated to 8 bits**. | 0057a280 - 0057a2a0 |
| 209 | `pc, k` | bool | PC has action / skill `k` (0..29 except 22 → error). | 0057a2b0 |
| 210 | `k` | bool | The skill test for `k` against the current player character set (the loop does not index the characters: it repeats the same test). Returns 1 when the test passes. | 0057a4c0 (medium-low) |
| 211 | – | handle | The first player character flagged as leader (its own test), or null. | 0057a8e0 |
| 212 | `actor, loc, mode, d` | bool | Movement "near" point `loc` at distance `d`; modes as 45; the added elements get variant flag 7. | 005743a0 (medium) |
| 213 | `a, b, t` | handle | New point at `a + (b − a) × t` (float `t`); both points must be in the same sector (else error null). | 00571c60 |
| 214 | `soldier` | void | Declare combat trainer (two flags). | 00579160 |
| 215 | `k` | handle | The active relic / information scroll of kind `26 + k` (k 0..6) in the element list, or null. | 0057a950 |
| 216 | – | int | Number of player characters (`mod 256`). | 0057aa20 |
| 217 | `i` | handle | Player character `i mod 256` (no bound check). | 0057aa50 |
| 218 | `chief, sub` | void | Add `sub` as subordinate of `chief` (both NPCs; neither already subordinate; `sub` not a chief; not self); no-op if already listed. | 0057aa70 |
| 219 | `npc` | void | Remove all subordinates. | 0057acc0 |
| 220 | `soldier` | void | Switch to the alert path. | 0057ad30 |
| 221 | `x` | bool | Soldier `x` is a rider; null → 0; non-soldier → 0. | 0057ad90 |
| 222 | `x` | bool | `x`'s highlight/mark flag is clear (the flag 99 clears and 186 / 188 / 189 set). | 00570ec0 |
| 223 | `banner` | bool | Banner inactive (= captured); null → error, reads null. | 00579730 |
| 224 | `loc, a, b, c` | void | Create a repulsive point (trap) at point `loc` with parameters `(a, b, c)`; non-point → error. | 0057adf0 (medium) |
| 225 | `n` | void | Level parameter := `n` and refresh all elements. Unused. | 0057ae60 (unknown) |
| 226 | `b` | void | Records kind 0x11: freeze-all flag := `b` when executed. | 00577df0 |
| 227 | `x` | void | Forwards to a trap routine (unread). Unused. | 0057ae40 (unknown) |
| 228 | `npc, k, v` | void | Emoticon: `k` = 0 clears; 1..7 → emoticon `k + 1` for duration `v`; else error. | 0057aed0 |
| 229 | `human` | void | Confiscate: a non-PC human's money is added to campaign counter 1 and zeroed. | 005729a0 |
| 230 | `zone` | bool | Every player character is inside `zone` (97 for each). | 00571e30 |
| 231 | `zone` | bool | No active hostile soldier in `zone` is still able to fight (alive, not knocked out, not tied, not fleeing). | 00571ef0 |
| 232 | `pc` | void | Add `pc` to the player's band (campaign roster); errors if not a PC or already present. | 0057afe0 |
| 233 | `npc, scroll` | void | Attach information scroll `scroll` (or null to detach) to `npc`. | 005785f0 |
| 234 | – | bool | Campaign counter 3 ≥ the required banner count (0 outside a campaign). | 00579690 |
| 235 | `item` | bool | Bonus item picked-up flag; not an item → error 0. | 0057b080 |
| 236 | – | int | Campaign money (counter 1); −1 with error outside a campaign. | 0057b0f0 |
| 237 | `v` | void | Set campaign money. | 0057b120 |
| 238 | – | int | A HUD value (unread). Unused. | 0057b150 (unknown) |
| 239 | – | void | Show the debriefing screen. | 0057b160 (medium) |
| 240 | `x` | bool | Active flag of a known element. | 00570d40 |
| 241 | `k, a, b` | void | Campaign: select entry `k − 1` and set two values. Unused. | 0057b2b0 (unknown) |
| 242 | `pc, a, b` | void | Set the two experience values of `pc`. | 0057b2f0 |
| 243 | `actor` | bool | Records kind 0x81: un-blip `actor` (known actor-family element). | 00575290 |
| 244 | `x, b` | void | Active flag of the element linked from `x` := `b` (`x` non-null; a player slot's character). | 0057b350 (medium) |
| 245 | – | int | Number of player characters not dead. | 0057b370 |
| 246 | `zone` | bool | Every living player character is inside `zone`. | 0057b3d0 |
| 247 | `target` | void | Convert a "handle" target into a "take" target (flags); errors if not a target object or wrong state. | 0057b450 |
| 248 | `pc` | bool | `pc` is selected (null: any selection). | 0057b4f0 |
| 249 | – | int | Number of selected player characters. | 0057b5e0 |
| 250 | `i` | handle | `i`-th selected player character; `i ≥` count → error null. | 0057b5f0 |
| 251 | – | void | Stop a sound category. Unused. | 0057b780 (medium) |
| 252 | `pc` | void | Make `pc` crouch. | 0057b7a0 |
| 253 | `k` | bool | Some player character with skill `k` is lost: dead, or (not in the band, with a captured mark, and recorded as lost by the campaign). | 0057b800 (medium) |
| 254 | `x, b` | void | A flag byte of `x` := `b` (no check). | 0057ba50 |
| 255 | `k` | bool | Some *present* player character has skill `k`. | 0057a6d0 |
| 256 | `pc` | int | Campaign identity of `pc` by its profile name: 0 the hero, 1 John, 2 Tuck, 3 Stuteley, 4 Scarlet, 5 Marian, 6 / 7 / 8 the three generic followers; unknown → error −1. | 0057ba60 |
| 257 | `pc, b` | void | Select (`b`) / deselect `pc` (HUD events 0xD / 0x12); null → all. | 0057bdb0 |
| 258 | `pc` | bool | `pc` is selected and an action is currently selected in the HUD. | 0057bed0 |
| 259 | `human` | int | Action state 0..17, else 666; non-human → −1. | 005766f0 |
| 260 | `human, k` | void | Set action state `k` (0..17). | 00576830 |
| 261 | – | bool | A campaign flag byte. | 0057bfe0 (unknown) |
| 262 | `n` | void | Screen fade in `n` steps. Unused. | 0057bff0 (medium) |
| 263 | `target, fx` | void | Link an animated element to a target object. Unused. | 0057c500 |
| 264 | `npc, k, b` | void | Forbid remark `k` (< 120) with flag `b`. | 00573010 |

Ids never called by the retail scripts (73 of 265; the other 192 occur at least once, probe output of section 8):
11, 14, 15, 22, 23, 25, 36, 37, 57, 60, 61, 63, 65, 66, 67, 68, 71, 76, 77, 78, 83, 84, 100, 104, 105, 106, 107,
108, 115, 116, 120, 121, 122, 123, 124, 127, 129, 131, 136, 138, 141, 142, 147, 148, 151, 153, 154, 157, 158,
167, 168, 169, 171, 175, 176, 181, 183, 184, 185, 190, 201, 206, 207, 208, 225, 227, 230, 238, 242, 251, 257,
260, 263. Their rows above are read from the code alone and cannot be cross-checked against script usage.

## 6. Open questions

### 6.1 Actor-side element execution (AI / movement / animation specs)
- Completion rules of the queued kinds (walk 0x14 / 0x15 arrival tolerance, path failure; animation 0xA4..0xA6 durations; actions of native 59; speak 0x92 end; corpse 0x65 / 0x66): 004646e0 (receive), 00467a50 (the actor's element runner, 4869 bytes), 0046bd40 (element-done notification), 0046b210 (admission tests through virtual slots 0x7C / 0x80 / 0x84 / 0x88 of the actor).
- The variant flags 6 / 7 that modes 2 / 3 of natives 45 / 212 put on the added elements (00582640, 3.5 KB movement builder; 00583630).
- What natives 46 / 47 do with the two "entered actors" lists beyond the first placement (00577650, 005779a0, 0057c950).

### 6.2 Natives whose exact reading is not settled
- 13: the fourth map table at the map object's offset 0x40E4 (00579d70) — is it the sector table, and does its order match the polygon part of the location table used by 6 (005714f0, level offset 0x4200)? The retail scripts assume 13 inverts 6 for polygon locations.
- 8 with `−1`: undefined read (00571760); the scripts' intent is "outdoors" = null; recommend returning null for `−1` and stating the divergence.
- 16, 17, 157, 162, 173, 190, 225, 227, 238, 241, 261: forwarding natives whose targets were not read (unused or single-use).
- 210: the loop that ignores its index (0057a4c0) — the effective semantics may be "the first player character's skill test".
- 147 - 151: which of "start / stop / release" each is (005a87f0, 005a8810, 005a88a0, 005a8780, 005a8700).
- The property codes of 117 / 118 beyond arrows / money / life (the PC inventory routine at virtual slot 0xE4 / 0xEC of the PC).

### 6.3 Scheduler details
- Whether the first level tick can be skipped by a modal on the first loop pass (0050f710: the modal guard at the tick call) — decides whether `Hourglass(0)` precedes or follows `PostInitialize` in practice.
- The "may die" civilian flag and the captured-PC field consulted by the defeat checks (004c6ef0, offsets 0x490 and 0x4A40 of the level).
- The exact set of AI events translated for `FilterAIEvent` (00410620, 0040dcb0).
- The scroll's per-element tick source (004b9f40 is called through a virtual slot; which update loop and whether it runs when the tick is skipped).

## 7. Differences from the current engine

Against `docs/formats/scb.md` and `crates/opensherwood-core/src/natives.rs` (the hypothesis engine):

1. **Opcode 0x07 returns.** The engine treats 0x07 as "set the return value, continue"; the original pops the frame at once (VM-047). The engine's semantics happens to agree on the retail files only because a 0x01 and a 0x06 follow every 0x07.
2. **Comparison opcodes.** 0x24 is `<=` (engine: `>=` as a low-confidence policy), 0x28 is `!=` (engine's guess was right), 0x26 is `>=`, 0x27 is `>`, all signed; 0x2A..0x2F are the six float comparisons with float `1.0 / 0.0` results (engine: 0x2B as a fixed-point `<`). 0x1C is int division, 0x1F xor, 0x16 float negate, 0x17 float→int: the engine calls them unused; they exist.
3. **Operand widths.** The jump, call and native-call opcodes (0x0E, 0x05, 0x0C) read `a | (b << 16)`; the two retail `0x0E` with `a = b = 0xFFFF` jump to `0xFFFFFFFF` (undefined in the original; engine: "unresolved jump" taint). Symbols are 14-bit offsets with a 2-bit storage class including a *global* block (`00`), which the engine calls "non-reference".
4. **Native 2 returns −1** for an unknown mission variable (engine: 0); native 0 grows the array by 16; native 1 errors on an undeclared index. The engine's fixed `MISSION_VARIABLES` array and "declare = set" reading are wrong in the error cases.
5. **Element table access (native 3)** covers `N + P` handles: player characters are addressed by `N + i` through 3 but native 10 returns the *PC-table index* for them (not `N + i`). Native 75 excludes the PCs. Out-of-range indices are errors returning null, not traps.
6. **Native 111 (and 159) are always null** in this build. The engine returns "the player's character" for 111 (policy) and an off-map location for 159 (policy). Messages sent to `n111()` are therefore messages to the *level* (null target = level class), which is why the first mission's level class handles them.
7. **Messages are synchronous, not queued.** 109 / 110 run `ProcessMessage` inside the native; 43 / 44 are sequence elements run when their level starts; 44's fourth argument is `arg2`, not a delay. The engine queues messages for the next tick.
8. **The clock.** The script tick is the 40 ms frame (25 Hz): native 56 counts level ticks (`n × 40 ms`), Hourglass runs every 25 ticks (1 s) with the *hourglass count* as parameter, CheckVictoryCondition every third Hourglass (or when forced by 29), and element (scroll) Hourglass gets 0 as parameter. The engine's `TickRate` assumption (25 ticks/s for 56, Hourglass period) is confirmed for the nominal rate; the engine runs the VM at 60 Hz world ticks, so it must convert (2.4 world ticks per script tick) or run the VM on a 25 Hz sub-clock.
9. **Scheduler order and initialisation.** Element `Initialize` runs at load in file order, the level's `Initialize(0)` afterwards, then the scrolls' `Initialize`, then the first tick (`Hourglass(0)`, `CheckVictoryCondition(0)`), then `PostInitialize`. The engine runs the level first, then the elements, then `PostInitialize` before any tick. Actor classes never get `Hourglass` (only scroll classes do, on their own activity counter); `HandleEvent` is never called; the nine `ActivatedBy*` results are ignored (the engine treats a zero as "refuse").
10. **Sequences are level-parallel with an abort cascade.** All elements between two barriers run concurrently; the barrier (32) only bumps the level when something was recorded since the last bump; a refused element (dead / absent actor) aborts every later level; text pages (203) complete immediately but pause the whole level tick; 202 is the same modal page shown at once; 41 is a dialog, not a "camera mode". The engine's per-element completion tokens are close but its "camera moves are instant" and "203 holds the sequence" readings differ: 33 / 42 wait for the camera scroll, 34 is instant; 203 does not hold the sequence (the pause of the tick does).
11. **Native semantics the engine got wrong or guessed** (by id): 4 / 5 / 6 / 9 are handle lookups with null on error (not identities); 8 is unchecked; 12 / 13 are inverse lookups into map tables (13 not settled); 18 / 19 / 20 are camera jumps (18 sets zoom 2.0), not "deployment area"; 21 sets zoom; 24 with 444 is a **no-op** (the engine records an effect); 35 is a zoom, 36 the map display, 37 / 38 HUD availability, 39 / 40 camera lock, 41 a dialog page; 46 / 47 place an actor and record a walk; 49 / 50 / 51 are animation elements with actor-side completion (the engine completes them at once); 52 / 53 lock / unlock AI as elements; 54 / 55 lock / unlock player input; 57 / 70 / 71 seek an actor (70 / 71 send a message afterwards); 59's codes are actions, 4 = shoot at an element index; 62 / 69 speak (69 = remark id < 120); 64 walks to the nearest building door; 72 / 73 hide / show player character `i` (index into the PC table, not "presentation pair"); 74 is the current actor (the *object* during `ActivatedBy*`); 80 = NPC, 81 = soldier, 82 = civilian, 83 = animal, 84 = cart; 87 dead, 88 knocked out, 89 tied, 90 any of them; 92 posture; 96 with null takes the actor off the map (a null location, not an "off-map location"); 98 with null tests outdoors; 101 = current action id (283 none); 102 inflicts damage; 103 stops; 112 select all / none; 117 / 118 property ids (1 = money of an NPC, 2 = life, 3 = concussion, 4..11 inventory); 119 = a civilian died; 125 / 126 AI sub-states; 128 = hostile attitude; 130 / 131 stare; 133 = post at location; 137 = noise; 140 = walking style; 144 has no null check; 152 / 156 buildings; 160 truncates a float distance; 161 is `rand() % n` of the C runtime; 178 / 179 / 223 / 234 banners with the win condition inside 178; 182 - 189 door state bytes (four flags); 191 = mechanism byte; 192 = current scroll; 193 / 194 scroll status with visibility (1 and 3 visible); 195 / 196 campaign values `k + 7`; 197 / 198 NPC custom values; 204 / 205 zone membership; 206 - 208 are 8-bit truncated bit operations; 209 / 210 / 253 / 255 skill tests; 211 = leader; 213 = interpolation between points of the same sector; 216 / 217 modulo 256; 218 - 220 subordinates / alert path; 221 rider; 222 = mark flag clear; 226 = freeze-all element; 228 emoticon; 229 confiscate; 231 / 246 zone predicates over soldiers / living PCs; 232 band roster; 233 attach scroll; 235 item taken flag; 236 / 237 campaign money with −1 outside a campaign; 240 active flag; 243 un-blip element; 244 linked element's active flag; 245 living PCs; 248 - 250 selection; 256 identity by profile name; 264 forbid remark.
12. **Error behaviour.** The original never traps: every bad handle or index logs and returns the failure value of section 5. The engine's strict-mode faults (unknown native, arity mismatch) have no counterpart: all 265 ids exist and arity is fixed by the wrapper (a mismatch corrupts the buffer silently).
13. **Temporaries and locals are zero-initialised at every function entry**; class variables at bind; the callback return register persists across runs (a callback ending with 0x06 returns the previous value). The engine clears frames per callback; the persistence matters only for callbacks that omit the value (none in the corpus).
14. **Natives 30 / 31 / 32 return values** (1 / 1 / level) that the engine ignores; 30 while recording is an error and returns 0 (the engine nests or resets).
15. **The engine's "sequence elements" list** (`SEQUENCE_ELEMENTS`) treats 43 / 44 / 45 / 48 / 56 / 203 as elements only inside 30 / 31; the original records them only inside a recording and otherwise **drops them with an error** (the engine executes them immediately).

## 8. Validation against the data

Counts are from `harness/tools/probe/scb_opstats.py` and `scb_semantics.py --natives / --params` over the 39
retail files (208 679 instructions, 42 734 native calls, 192 distinct ids):

- Every opcode present in the files (0x01 - 0x08, 0x0A - 0x0F, 0x11 - 0x15, 0x18 - 0x1B, 0x1D, 0x1E, 0x22,
  0x24 - 0x29, 0x2B) is implemented by the interpreter (VM-040..VM-069); no file uses 0x00, 0x09, 0x10, 0x16, 0x17,
  0x1C, 0x1F - 0x21, 0x23, 0x2A, 0x2C - 0x2F.
- Arity: the corpus has one arity per id and it equals the wrapper's arity for all 192 ids used (checked by
  joining the `arity=` column of the probe with the table of VM-086; no disagreement). Result use: the corpus reads
  a result (`0x0D`) only after ids whose wrapper returns a value or a bool, except that void ids are never read.
- 0x0A occurs 62 times, always directly after 0x05 whose target function ends with 0x07 (VM-071 holds).
- 0x07 is followed by 0x01 in 5081 of 5160 cases and by a jump otherwise: dead code under VM-047, consistent.
- Comparison directions (VM-067): the 100 uses of 0x26 are descending loops `i >= 0` (skipping index 0 would
  break them under `!=`); the 169 uses of 0x25 are ascending `i < n75()` loops; the 8 uses of 0x24 (`x <= 2`,
  `distance <= 80`, `money <= 2000`) read naturally as `<=`; 0x28 as `!=` matches the "purse not empty" gate.
- Native 56 arguments are `seconds × 25` in 108 uses and immediates 10 / 15 / 25 / 40 otherwise (VM-100: 25 ticks
  per second).
- Hourglass callbacks on element classes have signature `(2, 0, 4)` and read their parameter in 6 of 235 cases;
  the parameter is always 0 (VM-091), so those 6 reads compare a constant.
- The two `0x0E` with `a = 0xFFFF` are in H10 (classes 35 and 36, both `EnterZone`, after native 202; VM-070).
- Native 24 is called with 444 in 14 of 16 uses: all no-ops in the original (section 5).
- Native 111 is called 929 times (189 of them as the target of 43, all 507 of 44): under VM-120 those messages go
  to the level class, which matches the observation of `docs/formats/scb.md` that the level handles them.
- `FilterAIEvent` compares its second parameter with 0, 2, 8, 11, 13, 14, 22, 23, 31, 33, 34, 52 (VM-108) and
  returns 1 by default (VM-092).
- Contradictions found: none in the data; the H10 jump (VM-070) and the 444 minimap code are script-side
  oddities that the original tolerates by accident.

## 9. Provenance

Ghidra project `re/ghidra/robinhood` (never committed); decompilation export `re/out/decomp_all/<address>.c`,
inventory `re/out/inventory.tsv`, strings `re/out/strings.tsv`, module map `re/notes/modules.txt` (all
git-ignored); raw-byte disassembly with `scripts/ghidra/peek.py` plus a local capstone session for the functions
the decompiler mis-typed (the wrapper table 004075c0 and its 265 wrappers, the frame helpers, the activation
dispatcher 004bc1d0). Analyst notes: `re/notes/vm/` (wrapper table, native table with arities, per-id dump,
usage statistics). Functions read: section 0. Data checks: `python harness/tools/probe/scb_opstats.py
<levels>`, `python harness/tools/probe/scb_semantics.py <levels> --natives`, `--params`, `--pseudo --class`
on the game data copy at `C:\Users\przem\source\gamedata\robinhood` (read-only). No oracle recording was used;
the 40 ms frame pacing is a code fact, the 60 Hz / 64 Hz conversions refer to `docs/original/stealth-and-combat.md`.
Tests that will depend on this spec: the script VM rebuild (roadmap item "script VM and natives", ADR-0009).
