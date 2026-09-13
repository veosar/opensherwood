# Script VM, natives and callback scheduler (behaviour specification)

Status: `draft`, revision 2 (answers Codex review 14; awaiting re-review). Build: GOG English edition,
`Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base
0x00400000; every address below is a virtual address in that image. Analyst: 2026-09-13, session
`a45d5359e8dec5140` (Claude agent, analyst role under ADR-0009). Reviewer: Codex gpt-6-astra, review 14 (findings
1..28, verdict fix-then-clear; this revision answers them, see section 0). Exposure: analyst — this session;
implementer — none yet. Publication approval: pending (maintainer); separate from the factual review.

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text and no prescribed internal
structure (ADR-0009, expression filter), with one marked exception: the **compatibility tokens** of VM-005 and
VM-091 — the class name `StartUp` and the callback names `Initialize`, `PostInitialize`, `Hourglass`,
`CheckVictoryCondition`, `Finalize`, `ProcessMessage`, `ActionChange`, `FilterAIEvent`, `IsTaken`, `ReachPoint`,
`EnterZone`, `ExitZone`, `HandleEvent` and the nine `ActivatedBy…` names — which are function names stored in
the player's `.scb` files and looked up by exact spelling; a compatible program must use the same spellings.

Claims carry a stable id (`VM-nnn`), a status (`observed` = read in the executable's code, `inferred` =
concluded from several observed facts, `unknown`), the supporting address(es) and a confidence (high / medium /
low). Claims that are not `observed` and every excluded native map to an `Assumption` variant (section 4).

## 0. Necessity record

- Interoperability target: running the game's own compiled mission scripts (`.scb`, `docs/formats/scb.md`) and
  the mission, map and text data they address, so that the campaign plays as shipped.
- Information not otherwise available: the opcode arithmetic, the calling and native protocols, the semantics of
  the 265 natives, the callback scheduler and its clock, the sequence machinery. The data-only analysis of
  `docs/formats/scb.md` and the hypothesis engine (`crates/opensherwood-core/src/natives.rs`) left 95 natives as
  stubs and every scheduling rule as an assumption; eleven reviews said the engine cannot become faithful that
  way.
- Scope read: the interpreter (00634bb0 - 00639a80, 0063a140 - 0063a510, 0063b2d0, 0063a940 partly, 0063b070),
  the native registration and its 265 wrappers (004075c0, 00404a80 - 004075bf, raw bytes), every native's
  function (00570a30 - 0057c500 and the helpers named in section 6), the level tick and main loop (004c6ef0,
  0050f710), the callback invokers (004030a0 - 00408af0) and their callers, the sequence machinery
  (00570f00 - 00571100, 0057a020, 00582220 - 0058bff0 partly, 004ca410, 0053a0b0 - 0053a4c0, 0052b100), the
  mission-variable store (004e3d20 - 004e3e30), the end-of-level bookkeeping (004e3220, 004e3260, 004e3cd0), the
  element-table registration (004c2720, 00570850, 004d8460, 005709c0), the camera update's completion of camera
  elements (004cdfc0, 004c8380, 005105d0). The full list is in section 10.
- Stopping statement (honest scope): **completed** — the instruction semantics, the calling convention, the
  native protocol, the arity / result convention of all 265 ids, the semantics of 254 ids, the scheduler's order
  and clock, the sequence recording / launch / completion / abortion rules, messages, mission variables, end of
  level. **Not completed** — the actor-side execution and completion of movement, animation, speech and action
  elements (delegated to the navigation and AI specifications, section 1.1, with their open items), the meaning
  of natives 13, 91, 92, 157, 173, 190, 224, 225, 227, 238, 241, 261 (excluded from clearance, section 4.2), and
  the open questions of section 9.
- Analyst authorisation: on behalf of the maintainer, on the maintainer's lawfully acquired copy.

## 1. Scope

The VM executes the per-class bytecode of a mission's `.scb` file: one interpreter instance per scripted element
(the level itself, actors, player characters, objects, scrolls, waypoints, script zones). The engine calls named
functions of a class ("callbacks") with a few integer parameters; the script calls the engine back through
numbered natives (0..264). Covered: the loader's use of the file, the instruction semantics, the calling
convention, the native call protocol, the scheduler, messages, the sequence machinery and every native id.

### 1.1 Reviewed dependencies (what this spec does not decide)

| Topic | Depends on | Status of the dependency |
|---|---|---|
| Walk elements (natives 45, 46, 47, 57, 63, 64, 70, 71, 212): the orders they push, arrival, failure | `spec-navigation.md` NAV-130 (the walk sequence), NAV-142 (asynchronous path search, empty result fails after 100 clock units), NAV-152 (arrival within radius + 5 px) | reviewed there; its open question 3 (completion frame of a move-to-point action) stays open |
| Doors, buildings, patches behind natives 4, 8, 64, 98, 152, 156, 182 - 191 | `spec-navigation.md` section 5, NAV-172 (door state changes) | reviewed there |
| AI event ids seen by `FilterAIEvent`, the pre-filter, alert states, AI states behind 123 - 126, 128, 132 - 136, 140, 177, 218 - 220, 228 | `spec-ai-combat.md` AI-041 (state set), AI-081 (event set), AI-082 (pre-filter), AI-084 (markers), AI-190 (callback ids and renumbering), section 5.1 (native rows) | reviewed there; contradictions with this spec are listed in 9.4 |
| Posture codes returned by 91 / set by 92, action states of 259 / 260 | `spec-ai-combat.md` AI-066 and its open question 4 (codes unnamed) | **open** — natives 91 / 92 excluded (4.2) |
| Completion of animation, speech and action elements (natives 49 - 51, 59 - 62, 69) | animation / AI specs (not written) | **open** — section 9.1 |
| Element admission by an actor (which elements a dead, absent, locked or busy actor refuses) | `spec-ai-combat.md` (locking, AI-082 a) and the actor's element runner 00467a50 | **partly open** — VM-216 |

## 2. Data model

### 2.1 Program (per class, loaded once per file)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-001 | The file is accepted when its 8-byte magic matches and its version float compares *equal* to 1.5 under the machine's floating-point equality (so the exact value 1.5 is accepted, every other number rejected, and a NaN version is accepted because the comparison is unordered, section 3.1); otherwise an error is raised and nothing is loaded. | observed | 0063b2d0 | high |
| VM-002 | Operand composition: for the jump (0x0E), call (0x05) and native-call (0x0C) instructions the target / id is the 32-bit value `a \| (b << 16)` formed from the file's two 16-bit operands; for the conditional jumps (0x0F, 0x10) it is `c`; for three-operand arithmetic the third symbol is the low 16 bits of `c`. No other combination of operands is read. | observed | 00639370 and the handlers of 3.1 | high |
| VM-003 | At callback entry the interpreter uses the function's *name* (exact byte comparison, first match in table order), its *address* (first instruction index) and its `size_of_volatile` field, the last only to pre-allocate the frame's locals block; the prologue instruction 0x03 then re-allocates both blocks from its own operands, so the header value has no effect when it agrees with the prologue (it does in all 8176 retail functions). The fields `unknown_0..2` and `size_of_tempor` of the header are not read by the interpreter. | observed | 00639300, 0063a510, 0063b070, 0063a250, 00634d30 | high |
| VM-004 | The class-variable block is allocated, zero-filled, with the class's `size_of_variables` bytes when the class is bound to an element (once per instance). Variables have no run-time types: every cell is 4 bytes; ints and floats share cells. | observed | 00639280 | high |
| VM-005 | The level class is the one named `StartUp` (compatibility token: the class name in the file); a level without it is a fatal error at load. | observed | 004c0510 | high |

### 2.2 Interpreter instance (one per scripted element)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-010 | Instance state: program counter (instruction index); a stack of frames; the class-variable block; a shared "global" block (storage class `00`, one per program, never addressed by the retail scripts); the *current parameter buffer* (growable); a native argument buffer of 12 cells (48 bytes) with a fill count; a native result register; a callback return-value register (0 at construction, **never reset**: it keeps the last value written by any callback of this instance). | observed | 006390b0, 00639960, 00634c30 | high |
| VM-011 | Symbol operands are `u16`: bits 15..14 select the storage class (`00` global block, `01` class block, `10` current frame's locals, `11` current frame's temporaries), bits 13..0 are a byte offset into that block. No bounds are checked. | observed | 00634c30 (bytes) | high |
| VM-012 | A frame holds: the return program counter; a 4-byte *result slot* (uninitialised when the frame is created); the caller's parameter buffer (the one that received the caller's pushes); a locals block and a temporaries block (allocated zero-filled by opcode 0x03 with its operands as sizes; re-executing 0x03 frees and re-allocates them). | observed | 0063a250, 00634d30 | high |
| VM-013 | Creating a frame (opcode 0x05 and callback entry) saves the current parameter buffer into the new frame and installs a fresh, empty one as current. Popping a frame (0x06 / 0x07) frees the saved buffer and the frame's blocks; the current buffer stays the callee's (now empty) one. Parameters flow one way: pushes go into the current buffer, a call captures it, the callee reads it. | observed | 0063a250, 0063a320, 0063a3b0 | high |
| VM-014 | Snapshot-visible state of an instance between callbacks (callbacks never yield, so no frame or pc needs saving): the class-variable block, the return-value register, the native result register (readable only by the next 0x0D, which always follows a 0x0C: not needed), and the global block. The argument buffer is empty between callbacks in every retail file (balanced pushes). | inferred | 006390b0 | high |

### 2.3 Mission variables (natives 0, 1, 2)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-020 | One growable signed 32-bit array per level, empty at level start. Native 0 with an index `k` beyond the current size grows the array to `k + 16` entries (new entries 0) and stores the value; with `k` inside, it stores. Native 1 stores only if `0 <= k < size`, else an error and no effect. Native 2 returns the value, or `-1` (not 0) for `k` outside `0 <= k < size`. Native 0 does not check a negative `k` (unchecked write). Snapshot: the whole array. | observed | 004e3d20, 004e3e30, 004e3e00, 004e3e20 | high |

### 2.4 Handles and element categories

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-030 | Elements, doors, patches, buildings, paths, locations and sound sources are opaque handles, `0` = none. The **element table** is the list of elements registered during the mission load in registration order (map elements, mission records: actors, objects, items, scrolls, waypoints — the observed order is in `docs/formats/scb.md`, "Index spaces"), followed by **one slot per campaign character id** appended in the post-load step (the slot holds the player character with that id, or null when that character is absent). The script zones are created and bound before that post-load step; whether and where they occupy table entries is neither observable from the data (no zone class addresses itself by index) nor read here (9.3). Natives 3, 10 and 75 work on this table (player characters included). A further table of *cart-family* elements (carts and their kin) is reachable through native 3 beyond the last slot and through native 10 (VM-030b). "Is a known element" in the native rows means: found in the element table by handle (linear search); player characters *are* found. | observed | 004c2720, 00570850, 004d8460, 005709c0, 00571590, 00579bd0, 005714d0 | high (construction) / medium (the order of zone registration relative to the slots, taken from the data) |
| VM-030b | Native 3 on index `i` (table count `N`, truncated to 16 bits; cart count `C`): `i = -1` → null without error; `i < N` → the table entry (may be null for an empty slot); `N <= i < N + C` → cart `i - N`; otherwise error, null. `i < -1` is not rejected: it reads before the table (unchecked). Native 10 on a handle: its index in the table; if absent, its index in the **cart table counted from 0** (not offset by `N`); if absent from both, `-1`. Native 75 returns `N`. | observed | 00571590, 00579bd0, 005714d0 | high |
| VM-031 | Element *categories* used by the natives (defined by behaviour, not by representation): **actor family** — anything that moves under its own orders: humans, animals; **human** — a player character (PC) or a non-player human (NPC); **NPC** — a human that is not a PC; **soldier** — an NPC of the soldier class; **civilian** — an NPC of the civilian class; **animal** — an actor that is not human (horses); **cart family** — carts; **animated map element** — a map animation (fires, torches …); **target family** — objects, scrolls and pick-up items; **target object** — an object of that family that can be used with tools; **scroll**; **bonus item**. Which record kinds fall into which category is fixed by the mission and map formats (`docs/formats/rhm.md`, `rhp.md`); the natives only test membership. | observed | 00570a70 - 00570cb0, 0057b080, 005798f0 | high |
| VM-032 | A *location* is a point (from native 6 for a point entry, 95, 213) or a *zone* (a script polygon / sector, from native 6). "Requires a point" and "requires a zone" in section 6 are checks on that kind; a wrong kind is an error with the row's failure value. | observed | 005714f0, 00571de0, 00571b80 | high |

## 3. Behaviour

### 3.1 Instruction semantics

Notation: `S(x)` = the 4-byte cell named by symbol operand `x`; `a`, `b` = the two `u16` operands, `c` = the
`u32` operand, `c16` = its low 16 bits as a symbol; `pc` = program counter; `int` = 32-bit two's complement;
`float` = IEEE-754 single. Integer arithmetic wraps; integer comparisons are *signed*. Floating-point
environment: the x87 with invalid-operation exceptions masked (the process default); a comparison with a NaN
operand is *unordered* and yields the **exceptional outcomes** given in VM-068. Unless stated, an instruction
advances `pc` by 1.

| Op | Semantics | Id | Address | Conf. |
|---|---|---|---|---|
| 0x00 | Error (reported), then `pc := -1` and the loop continues: it reads the instruction before the first one (unchecked). Never emitted. | VM-040 | 00634bf0 | high |
| 0x01 | No operation. | VM-041 | 00634c20 | high |
| 0x02 | Append the 4 bytes of `S(a)` to the current parameter buffer (grows by 4). | VM-042 | 00634c30 | high |
| 0x03 | Function prologue: allocate the frame's locals block of `a` bytes and temporaries block of `b` bytes, both zero-filled (previous blocks freed). Every temporary and local therefore starts at 0 on each entry. | VM-043 | 00634d30 | high |
| 0x04 | Error (reported), then `pc += 1` (execution runs into the next function's prologue). Never reached by the retail files. | VM-044 | 00634d80 | high |
| 0x05 | Call: push a frame with return `pc + 1` (VM-013), `pc := a \| (b << 16)`. | VM-045 | 00634db0 | high |
| 0x06 | Return: pop the frame; `pc :=` its return pc. If that is `-1` the callback ends. | VM-046 | 00634dd0, 00639370 | high |
| 0x07 | Return with value: `v := S(a)`; the instance's return-value register `:= v`; if the frame depth is > 1, also the *caller frame's* result slot `:= v`; then exactly as 0x06. **Control does not continue** after 0x07. | VM-047 | 00634de0 (bytes) | high |
| 0x08 | `S(a) :=` the 4 bytes at byte offset `c` of the frame's saved parameter buffer (parameter `k` is at `4k`). No bounds check. | VM-048 | 00634eb0, 0063a3b0 | high |
| 0x09 | The inverse: write `S(a)` into the saved parameter buffer at offset `c`. Never emitted. | VM-049 | 00634fa0 | high |
| 0x0A | `S(a) :=` the current frame's result slot: the value stored by the *most recent* callee of this frame that executed 0x07; it persists until the next such callee overwrites it (uninitialised if none did). | VM-050 | 00635090, 00634de0 | high |
| 0x0B | Append `S(a)` to the native argument buffer at index `count`, `count += 1`. No bounds check (12 cells; the retail maximum pushed before one call is 6). | VM-051 | 00635150 | high |
| 0x0C | Native call: `id := a \| (b << 16)`; call table entry `id` (no range check; 265 entries) with the argument buffer; the wrapper pops the native's arity (`count -= arity`, arguments in push order) and the *result register := the wrapper's result* (section 3.3). | VM-052 | 00635210 | high |
| 0x0D | `S(a) :=` the native result register. | VM-053 | 00635240 | high |
| 0x0E | `pc := a \| (b << 16)`. | VM-054 | 00635320 | high |
| 0x0F | If `S(a) != 0` (as a 32-bit word; `-0.0f` counts as true) then `pc := c` else `pc += 1`. | VM-055 | 00635330 | high |
| 0x10 | If `S(a) == 0` then `pc := c` else `pc += 1`. Never emitted. | VM-056 | 006353f0 | high |
| 0x11, 0x12 | `S(a) := S(b)` (4-byte copy; identical opcodes). | VM-057 | 006354b0, 00635630 | high |
| 0x13, 0x14 | `S(a) := c` (4-byte immediate; identical for int and float bit patterns). | VM-058 | 006357b0, 006358a0 | high |
| 0x15 | `S(a) := -S(b)` (int; `INT_MIN` stays `INT_MIN`). | VM-059 | 00635990 | high |
| 0x16 | `S(a) := -S(b)` (float sign flip). Never emitted. | VM-060 | 00635b20 | high |
| 0x17 | `S(a) :=` float → int: the float is truncated toward zero to a **64-bit** integer and the **low 32 bits** are kept (so 4294967296.0 gives 0, 2147483648.0 gives 0x80000000, −1.5 gives −1); a value outside the 64-bit range or a NaN gives the 64-bit "indefinite" whose low word is 0. Never emitted. | VM-061 | 00635cb0 (conversion helper 00642b7c) | high |
| 0x18 | `S(a) := (float) S(b)` (int → float, round to nearest even). | VM-062 | 00635e50 | high |
| 0x19 / 0x1A / 0x1B | `S(a) := S(b) + S(c16)` / `S(b) - S(c16)` / `S(b) * S(c16)` (int, wrapping). | VM-063 | 00635fd0, 00636210, 00636450 | high |
| 0x1C | `S(a) := S(b) / S(c16)` (int, signed, truncating). Division by zero and `INT_MIN / -1` are machine traps in the original. Never emitted. | VM-064 | 00636680 | high |
| 0x1D / 0x1E / 0x1F | `S(a) := S(b) \| S(c16)` / `&` / `^` (bitwise). | VM-065 | 006368c0, 00636b00, 00636d40 | high |
| 0x20 / 0x21 / 0x22 / 0x23 | float `+`, `-`, `*`, `/` (`S(b) op S(c16)`); computed in extended precision and stored as single (one rounding). | VM-066 | 00636f80, 006371b0, 006373e0, 00637610 | high |
| 0x24 / 0x25 / 0x26 / 0x27 / 0x28 / 0x29 | int compare, result `1` or `0` in `S(a)`: `S(b) <= S(c16)`, `<`, `>=`, `>`, `!=`, `==` (signed). | VM-067 | 00637840, 00637a40, 00637c40, 00637e40, 00638040, 00638240 | high |
| 0x2A / 0x2B / 0x2C / 0x2D / 0x2E / 0x2F | float compare, result stored **as a float** `1.0f` or `0.0f` in `S(a)`: `S(b) <= S(c16)`, `<`, `>=`, `>`, `!=`, `==`. **Unordered operands** (a NaN): `<=`, `<` and `==` yield `1.0f`; `>=`, `>` and `!=` yield `0.0f`. | VM-068 | 00638440, 00638650, 00638860, 00638a70, 00638c80, 00638e90 | high |
| ≥ 0x30 | Error (reported); `pc` unchanged: the original loops forever. | VM-069 | 00639370 | high |

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-070 | The two retail instructions `0x0E` with `a = b = 0xFFFF` (H10, two zone classes, `EnterZone`, after native 202) jump to `pc = 0xFFFFFFFF`; the original then executes memory before the instruction array (unchecked). The script's evident intent is to end the callback (its other paths end with return value 1). Implementation choice 8.1. | observed | 00639370; data (`H10_Yor_VL.scb`, classes 35 and 36, instruction 31) | high (fact) / medium (intent) |
| VM-071 | Temporaries and locals are zero at every function entry (VM-043); class variables are zero at bind (VM-004); the result slot read by 0x0A is undefined unless a callee of the same frame executed 0x07 (true for all 62 retail uses: each follows a call to a value-returning function). | inferred | 00634d30, 0063a250 | high |
| VM-072 | Opcode 0x07 at callback depth 1 writes only the return-value register, which the engine reads after the run (VM-090). A callback that ends with 0x06 leaves the register unchanged (a value from an earlier callback of the same instance, or 0). All 39 `Finalize` and all 39 `PostInitialize` functions, 1199 of 1448 `Initialize`, 235 of 274 `Hourglass` and all 376 `ProcessMessage` functions end this way; the engine reads none of their results (VM-092). | observed | 00634de0, 00639360; data | high |

### 3.2 Calling convention (script to script)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-080 | The caller pushes each argument with 0x02 (4 bytes, in order), then 0x05. The callee reads argument `k` with 0x08 at offset `4k`. No argument count is checked: reading beyond the pushed bytes reads the buffer's slack (uninitialised). | observed | 00634c30, 0063a250, 0063a3b0 | high |
| VM-081 | A value is returned with 0x07 (VM-047) and read by the caller with 0x0A; the slot keeps the value until the next value-returning callee of the same frame overwrites it (VM-050). In the retail files 0x0A always directly follows the 0x05 (a corpus fact, not a rule). | observed | 00634de0, 00635090 | high |
| VM-082 | Frames nest without limit other than memory; recursion is allowed; every frame's locals / temporaries are fresh (VM-043). | observed | 0063a250 | high |

### 3.3 Native call protocol

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-085 | The native table has 265 entries, ids 0..264, all populated, built once when the first interpreter instance is created. | observed | 004075c0 (bytes) | high |
| VM-086 | Each entry is a wrapper that (1) takes the top `arity` cells of the argument buffer (the last pushed is the last argument), decrements the fill count by `arity`, (2) calls the native with those arguments in push order, (3) yields the result in one of three conventions, given per id in section 6: **void** — the wrapper yields 0; **bool** — the wrapper yields the low 8 bits of the native's result (for natives whose body produces no value this is the low byte of the last internal step, see the row); **int / handle** — the full 32-bit value. Some wrappers convert one argument to a bool before the call (non-zero → 1): natives 22, 26 (second), 36, 37 (third), 38 (second), 102 (third), 107, 115 (third), 130 (third), 131 (third), 134 (second), 138 (second), 139, 143 (second), 157 (second), 177 (second), 180 (second), 186 - 189 (second), 190 (third), 191 (**first**), 226, 244 (second), 254 (second), 257 (second), 264 (third). | observed | wrappers 00404a80 - 004075bf (bytes) | high |
| VM-087 | Natives with arity 0 take no argument (23, 29, 30, 31, 32, 40, 54, 55, 74, 75, 106, 111, 119, 120, 121, 122, 147, 148, 159, 163, 167, 170, 171, 172, 173, 174, 211, 216, 234, 236, 238, 239, 245, 249, 251, 261). Pushed cells for them stay in the buffer (the count is not reset); a persistent imbalance overflows the 12-cell buffer in the original (unchecked write). The retail files are balanced for every id. | observed | wrappers | high |
| VM-088 | The argument buffer is per instance and not cleared between callbacks; nested callbacks (VM-095) push and pop symmetrically. | inferred | 006390b0, 00635210 | high |
| VM-089 | **Failure classes** of native calls (each row of section 6 names its class): **(E)** the error is reported to the log and the row's failure value is returned, the script continues; **(E+)** the error is reported but the operation *still proceeds* (rows say "continues"); **(U)** unchecked memory access — no validation, the original reads or writes whatever the bad argument addresses (natives 3 for `i < -1`, 8 for any out-of-range index, 9 beyond the table, 144 / 182 - 189 with a null handle, 217 out of range, 164, 168 below zero, 0 with a negative index); **(T)** an arithmetic trap (161 with `n = 0`, opcode 0x1C); **(F)** fatal termination through the engine's fatal-error path (native 168 with an index beyond the list, the missing-function case of VM-090, two scroll callbacks overlapping — VM-094); **(N)** non-termination (opcodes ≥ 0x30 and 0x00, VM-069 / VM-040). Only class (E) is "safe"; implementation choice 8.1 turns U / T / F / N into a deterministic fault. | observed | 00571590, 00571760, 00570e20, 00578680, 00579350, 005f8030, 00639370 | high |

### 3.4 Running a callback

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-090 | To run callback `F` with parameters `p0..pn-1` on an instance: push `p0..pn-1` (4 bytes each) into the current parameter buffer; look the function up by name (VM-003); a **missing function is a null dereference (crash) in the original**, except the level's `PostInitialize`, whose presence is tested first; push a frame with return pc `-1` (capturing the buffer); `pc :=` the function's address; run until a 0x06 / 0x07 pops that frame. The caller then reads the return-value register (VM-072) when it wants a result. | observed | 00639300, 006392e0, 00639360, 00639340 | high |
| VM-091 | Callback parameter lists as pushed by the engine (parameter `k` = 0x08 offset `4k`; the names are compatibility tokens, see the preamble): `Initialize` on the level: one parameter, always 0; `Initialize` on every other class: none; `PostInitialize`: none; `Hourglass` on the level: one parameter = the *hourglass count* `T / 25` (3.5); `Hourglass` on a scroll class: one parameter, always 0; `CheckVictoryCondition`: one parameter = the hourglass count; `Finalize`: one parameter, **0 = the mission ended successfully, 1 = it ended in failure** (VM-103b); `ProcessMessage`: `(message, arg1, arg2)`; `ActionChange` (actor classes): `(current_action, previous_action)`, a missing action being 283; `FilterAIEvent`: `(actor_or_0, event)` (VM-108); `ActivatedByApple / Arrow / Hand / Heal / Lever / Money / Search / Stone / Sword / Listenable`: `(actor)` = the acting actor; `IsTaken`: `(actor)`; `ReachPoint`: `(actor)`; `EnterZone` / `ExitZone`: `(actor)`; `HandleEvent`: never called by this build. | observed | 00404080, 004030a0, 00404570, 00404180, 00404960, 00404280, 00404480, 004033b0, 00403190, 004032a0, 004035d0 - 00403ed0, 00404860, 00408750, 004089f0, 00408af0; callers in section 10 | high |
| VM-092 | Results the engine reads: `CheckVictoryCondition` (1 = declare victory, 2 = end in failure, anything else = continue; VM-103b); `FilterAIEvent` from the general AI dispatcher (0 = drop the event, non-zero = let it through; the second dispatcher ignores it — VM-108); `IsTaken` (non-zero = the scroll is taken: its status becomes 2 and it disappears; 0 = it stays); `Initialize` of the level (read, then ignored). Ignored: the nine `ActivatedBy*` (the object's own effect always runs), `Hourglass`, `ProcessMessage`, `ActionChange`, `Finalize`, `EnterZone`, `ExitZone`, `ReachPoint`. | observed | 004c6ef0, 00410620, 0040dcb0, 004ba4e0, 004bae60, 004c3740 | high |
| VM-093 | **Current actor** (native 74) is a global handle with *dynamic scope*: it is set to the actor whose `ProcessMessage`, `FilterAIEvent` or `ActionChange` runs, to the entering / leaving actor for `EnterZone` / `ExitZone`, and to **the object itself** for the nine `ActivatedBy*`; each of these restores the previous value when it returns. A message to the level does **not** change it: the level's `ProcessMessage` sees whatever is current (the actor whose callback sent it, or null at top level). `Initialize`, `PostInitialize`, `Hourglass`, `CheckVictoryCondition`, `IsTaken`, `ReachPoint` neither set nor restore it: they observe the enclosing value (null at level start; the last set actor otherwise). | observed | 00467230, 00410620, 0040dcb0, 00464230, 0057fcc0, 0057fdb0, 004bc1d0 (bytes), 004ca410, 00578050 | high |
| VM-094 | **Current scroll** (native 192) is a global with dynamic scope: set to the scroll for the duration of a scroll class's `Initialize`, `Hourglass` and `IsTaken` and cleared (to null, not restored) afterwards; setting it while it is already set is reported as a fatal error (class F). Any callback nested inside a scroll callback (a message sent by it, a zone entered by its walk order …) observes that scroll; every callback outside one observes null. | observed | 004ba760, 004ba5c0, 004b9fa0, 004b9f40, 004ba4e0, 005798c0 | high |
| VM-095 | Callbacks nest synchronously: natives 109 / 110 (send now), 153 / 154 (zone leave / enter), and any sequence whose first level contains a message element run the target's `ProcessMessage` / `EnterZone` / `ExitZone` **inside the native call** on the target's instance. If the target is the caller's own instance the run nests on it: the inner frame is pushed on top of the outer one and popped before the native returns; the native-call instruction restores its own `pc` from a local copy, so the outer callback continues correctly; the shared registers (native result, return value) are overwritten by the inner run but re-written by the outer instruction that follows. Outer 0x02 pushes pending at that moment would be captured by the inner run (no retail file pushes across a native call). | observed | 00635210, 00578d80, 0058a3d0, 00582560, 00585b70, 00467230, 004ca410 | high |

### 3.5 The scheduler

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-100 | **Clock.** The level advances at most once per main-loop iteration ("level tick"). In the active-window branch of the loop, when the game is not paused and the level's "presentation" flag is clear, the loop busy-waits so that an iteration lasts at least 40 ms (400 ms in a debug slow-motion mode); there is no catch-up. The nominal script clock is therefore **25 ticks per second** (40 ms); an iteration that takes longer stretches the tick, and the wait is skipped while paused or during presentations. Conversion: 1 level tick = 2.4 world ticks at 60 Hz = 2.56 animation clocks at 64 Hz (`docs/original/stealth-and-combat.md`); Hourglass period (25 ticks) = 1 s nominal. | observed | 0050f710 (wait loop, constants 40 and 400, guards) | high |
| VM-101 | The tick is **attempted** on every iteration but skipped when: the game is paused; a modal window object is open; the game state is one of the two "leaving the level" states. Text pages and dialogs are *not* this modal window: they are synchronous loops inside the native / element that shows them (VM-218). While the tick is skipped the *camera update* still runs on every unpaused iteration and can complete camera elements (VM-219). | observed | 0050f710 (guards at the tick call), 005105d0, 004c8380 | high |
| VM-102 | **Tick counter** `T`: 0 when the level starts; incremented at step 4 of every executed tick (VM-103); never reset. Snapshot: `T`, the force flag, the won flag and its sub-flag, the success / failure end flags, the debriefing index. | observed | 004c6ef0 | high |
| VM-103 | **Order within one executed level tick**, with its early exits (only the script-relevant steps): **(1)** if the *success-end* flag is set: level `Finalize(0)` (if scripts are enabled), tick ends with code 2 (the level ends); else if the *abort* flag is set: end bookkeeping (VM-103b), tick ends with code 1; else if the *failure-end* flag is set: `Finalize(1)`, code 3. **(2)** if the victory sub-flag is set and no player character has left the map yet: clear the sub-flag and show the "you may leave" notice. **(3)** pending one-shot updates (four HUD refresh flags). **(4)** if `T mod 25 == 0`: level `Hourglass(T / 25)`; then, if `(T / 25) mod 3 == 0` or the *force* flag is set: clear the force flag, run `CheckVictoryCondition(T / 25)`; result 1 and the won flag clear → **declare victory** (won flag := 1, sub-flag := 1 unless the campaign node is of type 3 or 6; for those two node types an additional immediate action follows that was not read, 9.3; otherwise no immediate end); result 2 → end bookkeeping (VM-103b) and **the tick ends here** (code 1) — `T` is *not* incremented. **(5)** `T += 1`. **(6)** if any of three "cinematic / transition" flags is set → the tick ends (code 0) skipping steps 7..10. **(7)** unless the debug "no defeat" toggle is set — **defeat checks**, each ending the tick (code 1) after the end bookkeeping: no player character that is present and has not left → end; a captured player character → end; a dead civilian whose "may die" flag is clear → end. **(8)** per-element updates in element-table order (actors dispatch `ActionChange`, scrolls count toward their `Hourglass`, movement, AI …). **(9)** drain the sequence manager's queue (VM-215). **(10)** the timer pass (VM-221). Hourglass therefore runs at `T` = 0, 25, 50, …; `CheckVictoryCondition` at `T` = 0, 75, 150, … unless forced. | observed | 004c6ef0 | high |
| VM-103b | **End bookkeeping**: computes the end statistics (living / dead / neutralised hostile soldiers into campaign counters, a 1000 bonus when the node is not of type 3), then: if the won flag is set → success-end flag := 1 (so the *next* tick runs `Finalize(0)` and ends with code 2); else → failure-end flag := 1 (`Finalize(1)`, code 3). Consequently: a mission ends **successfully** when the script declared victory *and afterwards* every player character has died or left the map (the first defeat check) or `CheckVictoryCondition` returned 2 or the abort flag was raised; a `CheckVictoryCondition` result of 2 *after* a declared victory still ends successfully. Native 178 (banner capture reaching the node's requirement in a type-1 node) declares victory the same way. | observed | 004e3260, 004e3220, 004e3cd0, 004c6ef0, 005795c0 | high |
| VM-104 | **Initialisation order.** While the mission file is read, every scripted actor, object, waypoint and script zone binds its class and runs its `Initialize` immediately, in the order the records are read (the element table is incomplete then; the retail callbacks are stubs). Player characters run `Initialize` when they are created. Then, just before the play loop: the level's `Initialize(0)`, then every scroll's `Initialize` in scroll-table order (current scroll set). The level's `PostInitialize` runs once, on the first loop iteration, after the first *attempted* tick (if that tick executed, after `Hourglass(0)` and `CheckVictoryCondition(0)`) and if the class has it. | observed | 0046e7e0, 004bb820, 00551bb0, 0057f8c0, 004a0f10, 004c3740, 004ba000, 004b9fa0, 0050f710 | high |
| VM-105 | **Scroll `Hourglass`**: each active, scripted scroll counts its own element updates; when the count reaches 25 it runs `Hourglass(0)` and resets. It counts only while the scroll is active, so scrolls fire at 25, 50, … updates of activity, not on the level grid. | observed | 004b9f40 | high |
| VM-106 | **Scroll `IsTaken`**: run when the "take scroll" element (created by the pickup order, VM-212 a) executes, with the taking actor; before the call the scroll's status is set to 3 and a sound is played; a non-zero result sets status 2 (taken, hidden); zero leaves it at 3. | observed | 004ba4e0, 004ca410 | high |
| VM-107 | **`ActionChange`**: in an actor's per-element update, when the actor's current action id (283 = none) differs from the id stored at the previous check, `ActionChange(current, previous)` runs and the stored id is updated. Objects of the target family have a dispatcher of the same shape (not read). | observed | 00464230 | high (actors) / medium (objects) |
| VM-108 | **`FilterAIEvent(actor_or_0, event)`** has two sources. (a) The general AI dispatcher, before the pre-filter (`spec-ai-combat.md` AI-082): `event` is the renumbered id of AI-190; the first parameter is the involved actor for payload kind 3, else 0; a zero result **drops the event**. (b) A second dispatcher for seven **events 100..106** raised by the actor's own order machinery (0 → 100 … 6 → 106): before the callback it also sets the NPC's alert state (events 100 - 102 → alert 0; 103 and 106 → alert 1); the first parameter is an element stored with the NPC (its target for 100..103, another stored element for 104..106); the result is **ignored**; afterwards the event and its payload are stored in the NPC. The external meaning of 100..106 is open (9.2). | observed | 00410620, 0040dcb0 | high (a) / medium (b) |
| VM-109 | **`ReachPoint(actor)`**: run when a path follower reaches a waypoint that carries a script reference, and when an actor reaches the end of a patrol node flagged for scripting (AI-091). | observed | 004105d0, 004abfe0 | medium |
| VM-110 | **`EnterZone(actor)` / `ExitZone(actor)`** run from the zone's membership dispatchers: entering adds the actor to the member list (a second entry is reported but still dispatched), leaving removes it (leaving without membership is reported and still dispatched). Natives 153 / 154 call the *same dispatchers* but only when the actor **is already a member** (VM-120b). | observed | 0057fcc0, 0057fdb0, 00577220, 00577390 | high |
| VM-111 | **`ActivatedBy*`**: run when the player's "use tool on object" element executes on a scripted object — nine tool kinds, in this order: apple, arrow, hand, heal, lever, money, search, stone, sword — with the acting actor as parameter and the *object* as current actor (VM-093); `ActivatedByListenable` runs from the object's own update when its "listenable" flag is set. Results ignored (VM-092). | observed | 004bc1d0 (bytes), 004bae60, 004bc3d0 | high |
| VM-112 | Every script callback is gated by the global "scripts enabled" flag (set once a level script is loaded). | observed | 004c6ef0 and the invokers | high |
| VM-113 | **Engine-originated message**: in the campaign's hub level only (the current campaign node is of type 8), the engine sends message `(1001, 0, 0)` to the **level** (a) right before the play loop starts unless the loop is entered from the mission-selection state, and (b) each time the loop returns from the mission-selection state. Both go through the send-now path (VM-120). No other message is originated by the engine. | observed | 0050f710 (call sites 0050f8bb, 0050fd58), 0050b640 (the type-8 flag), 00578d80 | high |

### 3.6 Messages

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-120 | A message is `(message, arg1, arg2)`; the target is an actor-family element or **null**, and null means *the level class*. Any other target is an error and the message is dropped (E). There is no queue: delivery is a sequence element executed synchronously when its level runs (VM-095). Natives 43 / 44 *record* it into the sequence being recorded (error and no delivery when none is being recorded); natives 109 / 110 wrap it in a one-element sequence and deliver it **immediately, inside the native call**. | observed | 00578cb0, 00578e50, 00578d80, 00578f20, 004ca410, 00467230 | high |
| VM-120b | Natives 154 / 153 (add to / remove from a zone) fire `EnterZone` / `ExitZone` only for an actor that is **already a member** of the zone (an actor of the element table, actor family); for an absent actor they report an error and fire nothing. The direct dispatchers, by contrast, add / remove membership themselves (VM-110). | observed | 00577390, 00577220, 0057fcc0, 0057fdb0 | high |
| VM-121 | The with-arguments forms (44 / 110) pass `(message, a, b)`; the plain forms pass `(message, 0, 0)`. There is no delay parameter. | observed | 00578e50, 00578cb0 | high |
| VM-122 | Message ids are opaque integers. The engine originates only message 1001 of VM-113; native 71 requires its message id to be ≥ 1000. | observed | 00573f00, VM-113 | high |

### 3.7 Sequences

A *sequence* is an ordered list of *elements*, each tagged with a *level* number; it runs level by level: all
elements of a level start together, and the next level starts when every element of the current level has
finished. Recording (natives 30 / 31 / 32 and the recording natives of section 6) builds such a list.

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-200 | **Recording state** (global, one recording at a time): the sequence being recorded, the *current level* (a 16-bit counter; 0 = not recording) and two "entered actors" lists used by natives 46 / 47 / 63. Native 30: if a recording is open → error, returns 0, no change; else creates an empty sequence, level := 1, clears the two lists, returns 1. | observed | 00570f00 | high |
| VM-201 | Native 32 (barrier): if not recording → error, returns 0. Else, if the sequence has at least one element **and the last recorded element's level equals the current level**, level += 1 (16-bit; wrapping at 65536 is unreachable: a class has at most 2336 instructions). Returns the current level. Two consecutive barriers count once. | observed | 00571100 | high |
| VM-202 | Native 31: if not recording → error, returns 0. Else recording ends (level := 0); the entered lists are cleared; an empty sequence → error, discarded, returns 1; else the sequence is handed to the manager and **launched at once** (VM-210), returns 1. | observed | 00571020 | high |
| VM-203 | A recording native appends one element tagged with the current level to the sequence being recorded. If no recording is open, the element is created, an error is reported and the element is discarded (no effect). This holds for every recording native of section 6 (those marked *records*). | observed | 0057a020 and the natives | high |
| VM-204 | An element carries the effect requested by its native (its category: camera, timer, message, page, walk, animation, …), its target and the native's arguments; the category decides who executes it and when it completes (VM-212, VM-218, VM-231). | observed | the natives of section 6 | high |
| VM-210 | **Launch**: the sequence is appended to the manager's list; the elements of the first level (all elements sharing the level of the first one) are *dispatched* (VM-212): immediate categories run now; the others are queued as *pending* (an element becomes *running* only when an actor admits it). Sequences from natives 102, 109, 110 and from the engine (pickups, AI) use the same path with one element at level 1. | observed | 0058a3d0, 00582530, 00582560, 0058a940 | high |
| VM-211 | **Level completion**: each sequence counts the elements of the current level not yet *done*; an element reaching *done* decrements it; at zero the next level's elements are dispatched **synchronously at that point** (inside whatever step completed the last element: an actor update, the queue drain, the timer pass, a modal return). After the last level nothing happens; the manager's housekeeping (every 256 ticks) deletes a sequence none of whose elements is running or pending. | observed | 00582620, 00582560, 005823f0, 0058a430, 004c6ef0 | high |
| VM-212 | **Dispatch of an element** at level start: (a) *level-immediate* categories — lock / unlock player input (54 / 55), camera jump (34), timer (56), PC action availability (37), character availability (38), take-scroll (pickup) — are executed by the level executor at once; (b) a message element (43 / 44) with an actor target is executed by the actor's message path at once, with a null target by the level executor at once; (c) *actor-immediate* categories — lock / unlock AI (52 / 53), un-blip (243), animation table swap (60 / 61), mobile-element start / stop / activate / deactivate (67 / 68 / 72 / 73), speak (62 / 69) and an engine-internal movement variant — are handed to the target actor at once (VM-216); (d) every other category (camera scroll 33 / 42, zoom 35, map 36, dialog 41, camera lock 39 / 40, page 203, freeze-all 226, walks, seeks, turns, animations 49 - 51, actions 59, damage 102, corpse 63 / 65) is appended to the manager's FIFO queue as *pending*. | observed | 00582560, 0058bb80, 00585b70 | high |
| VM-215 | **Queue drain** (step 9 of the tick): the queue is processed FIFO until empty: each element removed and, if still pending, given to its target — an actor target's admission (VM-216), a null target → the level executor (VM-218). Elements appended during the drain (by a callback or a level completion) are drained in the same pass. | observed | 0058ba60, 00585570, 004c6ef0 | high |
| VM-216 | **Actor admission**: the actor runs its admission tests on the element (its own state and the element's parameters; `spec-ai-combat.md` on locking and out-of-action actors; the exact predicates are open, 9.1). Refused → the element gets state *refused* with the propagation flag (VM-217). Accepted: if the actor is idle it becomes the current action and starts (*running*); else it is merged or queued by the actor's own priority rules (open). The actor reports *done* when the action ends (VM-231). | observed (structure) | 004646e0, 0046b210 | medium |
| VM-217 | **Abort cascade**: a state change to *refused* (5) or *cancelled* (6) carries a propagation flag: with flag "next level" (used by the actor's refusal and cancellation) the first element of the **next** level is set to the same state with flag "chain"; with flag "chain" the next element of the sequence is set likewise, so every later element (whatever its level) ends in that state; with no flag nothing propagates. Same-level siblings of the refused element are not touched and finish normally. A refused / cancelled element never reports *done*, so its level never completes and the rest of the sequence is dropped (deleted by housekeeping). A *done* transition never propagates. | observed | 00585320, 004646e0 | high |
| VM-218 | **Level executor** (categories run by the level; completion): lock / unlock player input: HUD events, *done* at once. Camera jump to a point: *done* at once. **Camera scroll** to a point (33 / 42): becomes the *camera element*; *done* when the camera update finishes the scroll (VM-219), or at once when the level's "no cinematic camera" flag is set; a new camera element marks the previous one *done*. **Zoom** (35): sets the zoom target, becomes the camera element; *done* when the zoom reaches its target (VM-219). Map display on / off (36): *done* at once. **Timer** (56): appended to the level's timer list (VM-221). **Dialog** (41) and **page** (203): unless the "no presentation" flag is set, a **synchronous modal loop** shows the dialog / page and returns only when the player dismisses it — the whole program is suspended inside it (no ticks, no other sequences, no input other than the page's); then the element is *done* and a HUD event follows. Camera lock on an actor / clear (39 / 40): *done* at once. Message: the level's `ProcessMessage`, *done* at once. Freeze-all flag := bool (226): *done* at once. PC action availability (37) and character availability (38): HUD events, *done* at once. Take-scroll → `IsTaken` (VM-106): *done* at once. Native 202 shows the same modal page **inside the native call**: the script instruction after it runs only after dismissal. | observed | 004ca410, 0053a1a0, 0053a0b0, 0053a4c0 (modal loop), 0052b100, 00579f30 | high |
| VM-219 | **Camera update**: runs once per unpaused main-loop iteration (whether or not the tick executed), after the tick step; when the current camera scroll ends it resets the movement scale to 1.0, clears the follow flag and marks the camera element *done*; when the zoom reaches its target it clears the target and marks the camera element *done*. These completions (and the level completions they trigger, VM-211) are therefore an execution opportunity **outside the tick phases**. | observed | 004cdfc0, 004c8380, 005105d0, 0050f710 | high |
| VM-221 | **Timer pass** (step 10): the level's timer list is visited in insertion order, over the count captured at the start of the pass: an element whose counter equals 1 is *done* and removed; otherwise `counter -= 1` (32-bit wrapping: a counter of 0 or negative reaches 1 only after about 2^32 ticks — "effectively never"). A timer inserted earlier in the same tick (by the drain, a callback, a level completion) *is* visited in that tick's pass; one inserted *during* the pass (by a completion in the pass) is first visited next tick. Two timers completing in the same pass complete in list order, each running its level completion synchronously before the next is visited. A timer of `n >= 1` completes on the `n`-th visited pass. | observed | 004c6ef0, 00575350 | high |
| VM-222 | While the program is suspended in a modal loop (VM-218) nothing else advances; while the *tick* is skipped (VM-101) the camera update still completes camera elements (VM-219). | observed | 0050f710, 004cdfc0 | high |
| VM-231 | Actor-side completion: actor-immediate categories (VM-212 c) are *done* as soon as the actor executes them; speak ends when the actor finishes the line; queued categories end when the actor's action ends — walks on arrival (NAV-152: within radius + 5 px) or failure (NAV-142: an empty path result fails the order after 100 clock units), animations at the end of the clip / loop, actions at the end of the action. The exact end conditions of animation, speech and action elements are open (9.1). | observed (dispatch) / unknown (durations) | 00467230, 004646e0 | medium |

### 3.8 Objectives, debriefing, win and loss

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-240 | Native 26 `(k, main)` adds objective `k` (bool `main`) to the objective display; 27 `(k)` marks it accomplished (both forward to the objectives store; `k` indexes the level's short-briefing texts, `docs/formats/scb.md`). | observed | 005785b0, 005785d0 | high |
| VM-241 | Native 28 `(k)` stores `k` as the debriefing variant read by the main loop when the level ends. | observed | 00570a30, 0050f710 | high |
| VM-242 | Native 29 sets the *force* flag: the next step-4 Hourglass point runs `CheckVictoryCondition` even if `(T/25) mod 3 != 0`; it does not run it immediately. | observed | 00578c60, 004c6ef0 | high |
| VM-243 | `Finalize(0)` runs on the tick after the success-end flag was set, `Finalize(1)` after the failure-end flag (VM-103 step 1, VM-103b); the level then ends. | observed | 004c6ef0, 004e3260 | high |

## 4. Claims

The claim register is the union of the `VM-nnn` rows of sections 2, 3 and the per-id rows of section 6 (each
row names its address and failure class). This section adds the assumption mapping.

### 4.1 Assumption mapping for non-observed claims

| Claim / native | Status | `Assumption` variant (ADR-0008) |
|---|---|---|
| VM-014 (snapshot set) | inferred | none needed (a design fact) |
| VM-030 zone-registration order relative to the PC slots | medium | `ElementTableOrder` |
| VM-070 (intent of the `-1` jump) | medium | `UnresolvedJump` (kept) |
| VM-108 (b): meaning of events 100..106 | unknown | `AiEventCode(100..106)` |
| VM-109 (patrol-node variant) | medium | `ReachPointSource` |
| VM-216 admission predicates, VM-231 durations | unknown | `ElementAdmission`, `ElementDuration(category)` |
| natives 46 / 47 / 63 details (entered lists), 212 variant flag | medium | `Policy(id)` |
| natives 13, 91, 92, 157, 173, 190, 224, 225, 227, 238, 241, 261 | excluded from clearance (4.2) | `UnknownNative(id)` (lenient: record the call and return the row's placeholder) |

### 4.2 Natives excluded from clearance

These ids are called by the retail scripts (13 ×25, 91 ×1, 92 ×1, 173 ×14, 224 ×159, 241 ×1, 261 ×2; the
others are unused) but their semantics were not settled; they stay `unknown` and map to `UnknownNative(id)`
with the placeholder result of their row until an analyst reads the addresses of 9.2. For 224 the *effect*
(create a repulsive point) is observed but its result and parameters are not.

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| minimum iteration time (script tick) | 40 | ms | 0050f710 | high |
| slow-motion iteration time (debug) | 400 | ms | 0050f710 | high |
| Hourglass period | 25 | level ticks | 004c6ef0 | high |
| victory check period | 3 | hourglass counts | 004c6ef0 | high |
| sequence housekeeping period | 256 | level ticks | 004c6ef0 | high |
| scroll Hourglass period | 25 | scroll updates | 004b9f40 | high |
| native table size | 265 | ids | 004075c0 | high |
| native argument buffer | 12 | cells | 006390b0 | high |
| file version accepted | 1.5 | float, machine equality | 0063b2d0 | high |
| mission variable growth | k + 16 | entries | 004e3d20 | high |
| missing action id | 283 | action id | 00464230, 00578060 | high |
| "no state" answer of 91 and 259 | 666 | code | 005762d0, 005766f0 | high |
| zoom factors accepted by 21 / 35 | 0.5, 1.0, 2.0 | zoom | 00571200, 00572d50 | high |
| camera movement scale set by 18 | 2.0 (19: its argument; reset to 1.0 when the scroll ends) | scale | 00571270, 00571330, 004cdfc0 | high |
| remark id limit (69, 264) | 120 (ids 0..119) | remark id | 00572ef0, 00573010 | high |
| door search radius of 64 | 300 (squared 90000) | map px | 00574860 | high |
| campaign value window of 195 / 196 | k in 0..19 → slots 7..26 | index | 00579430, 00579470 | high |
| NPC custom values (197 / 198) | k in 0..9 | index | 005794b0, 00579520 | high |
| scroll status range (194) | 0..3 | status | 00579950 | high |
| minimum custom message id (71) | 1000 | message id | 00573f00 | high |
| engine-originated message (hub level) | 1001 | message id | 0050f710 | high |
| team size limit with no mission selected (174) | 5 | characters | 00579300 | high |

## 6. Interfaces to the script VM: natives by id

Columns: `Args` in push order (`b` = converted to bool by the wrapper); `→` = wrapper result convention
(VM-086): `void` (0), `bool` (low byte), `int`, `handle`; **records** = creates an element in the open
recording (VM-203, error and no effect otherwise); **Fail** = failure class of VM-089 with the value returned.
Confidence high unless marked. Element categories as in VM-031; "actor" = actor family unless narrowed.

| Id | Args | → | Behaviour | Fail | Address |
|---|---|---|---|---|---|
| 0 | `k, v` | void | Mission variable `k := v`, growing the array (VM-020). | U (negative k) | 00577090 |
| 1 | `k, v` | void | Mission variable `k := v`; `k` outside the array → E, no effect. | E | 005770b0 |
| 2 | `k` | int | Mission variable `k`; outside → −1. | E (−1) | 00577100 |
| 3 | `i` | handle | Element `i` (VM-030b). | E (null) / U (i < −1) | 00571590 |
| 4 | `i` | handle | Door `i` of the map's door list (NAV section 5); `-1` → null; out of range → null. | E (null) | 005716a0 |
| 5 | `i` | handle | Patch `i` of the map's patch list; same rules. | E (null) | 00571700 |
| 6 | `i` | handle | Location `i` of the level's location list (`i` modulo 65536; no upper check); `-1` → null; an empty entry → null. | E (null) / U | 005714f0 |
| 7 | `k` | handle | Sound source `k`; error and null when the level has sound and `k` does not exist; null silently when the level has no sound. | E (null) | 005755f0 |
| 8 | `i` | handle | Building `i` of the map's building list, **no check at all** (NAV section 5; `-1` reads before the list). | U | 00571760 |
| 9 | `i` | handle | Patrol path `i` (modulo 65536, no upper check); `-1` → null. | U | 00571780 |
| 10 | `e` | int | Index of `e` (VM-030b): table index, else cart index from 0, else −1. | E (−1) | 00579bd0 |
| 11 | `door` | int | Index in the door list or −1. | (−1) | 00579c90 |
| 12 | `patch` | int | Index in the patch list or −1. | (−1) | 00579d00 |
| 13 | `x` | int | **Excluded (4.2).** Index of `x` in the map's script-zone (sector) list, −1 if absent; the retail scripts use it as the inverse of native 6 for polygon locations — whether the two orders coincide is open (9.2). Placeholder: the inverse of 6. | (−1) | 00579d70 |
| 14 | `sound` | int | Index of the sound source; −1 with an error if unknown; −1 silently when the level has no sound. | E (−1) | 00579de0 |
| 15 | `building` | int | Index in the building list or −1. | (−1) | 00579e80 |
| 16 | `path` | int | Index of `path` in the patrol-path list (inverse of 9), as a 16-bit value; 65535 if absent. | (65535) | 00579ef0 |
| 17 | `k` | void | Shows dialog page `k` **now** in the synchronous modal loop of VM-218 (the native returns after dismissal). | – | 005714a0 |
| 18 | `loc` | bool | Camera scrolls to point `loc` with the movement scale set to 2.0 (VM-219 resets it to 1.0 at the end); returns 1. Null → error, 0. Not an element. | E (0) | 00571270 |
| 19 | `loc, f` | bool | As 18 with movement scale `f`. | E (0) | 00571330 |
| 20 | `loc` | bool | Camera scrolls to point `loc` (scale unchanged) and the camera-follow flag is cleared; returns 1. | E (0) | 005713f0 |
| 21 | `f` | bool | Zoom factor := `f`, only 0.5 / 1.0 / 2.0 accepted → 1; else error, 0. | E (0) | 00571200 |
| 22 | `b` | bool | Map display shown (`b`) / hidden; returns 1. | – | 005714b0 |
| 23 | – | void | A screen transition (HUD event with a fade). Unused. | – | 00576170 (medium) |
| 24 | `e, code` | void | Minimap dot of known element `e`: codes 0 / 1 = default dots; 100 - 102, 200 - 202, 300 - 302 = green / red / blue styles; 111 / 222 / 333 the same, humans only. Any other code (the retail 444 included) → error, **no change**. | E | 005788a0 |
| 25 | `zone, f` | void | Force the "emergency box" of a motion-area zone (radius `f`); not such a zone → error. Unused. | E | 00578c70 |
| 26 | `k, b` | void | Add objective `k`, primary if `b`. | – | 005785b0 |
| 27 | `k` | void | Objective `k` accomplished. | – | 005785d0 |
| 28 | `k` | void | Debriefing variant := `k` (VM-241). | – | 00570a30 |
| 29 | – | void | Force the next victory check (VM-242). | – | 00578c60 |
| 30 | – | bool | Begin recording → 1; already recording → error, 0 (VM-200). | E (0) | 00570f00 |
| 31 | – | bool | End recording and launch → 1; not recording → error, 0 (VM-202). | E (0) | 00571020 |
| 32 | – | int | Barrier: bump the level when due; returns the level; not recording → error, 0 (VM-201). | E (0) | 00571100 |
| 33 | `loc` | bool | Records a camera scroll to point `loc` (default speed) → 1; not recording / null / not a point → error, 0. | E (0) | 00572ab0 |
| 34 | `loc` | bool | Records a camera jump to point `loc` → 1; same checks. | E (0) | 00572ba0 |
| 35 | `f` | bool | Records a zoom to `f` (0.5 / 1 / 2) → 1; else error, 0. | E (0) | 00572d50 |
| 36 | `b` | bool | Records map display := `b` → 1; not recording → error, 0. | E (0) | 00572e40 |
| 37 | `pc, k, b` | bool | Records "action `k` available := `b`" for player character `pc` → 1; `pc` not an actor → error, 0. | E (0) | 00577d30 |
| 38 | `pc, b` | bool | Records "character `pc` available := `b`" → 1; not an actor → error, 0. | E (0) | 00577e70 |
| 39 | `actor` | bool | Records "camera locks on `actor`" → 1; not an actor → error, 0. | E (0) | 00577f30 |
| 40 | – | bool | Records "clear the camera lock"; the byte returned is the recording step's result (1 recorded, 0 not recording). | E (0) | 00577fe0 |
| 41 | `k` | bool | Records "dialog page `k`" (modal when executed, VM-218); byte = recording step's result. | E (0) | 00572a20 |
| 42 | `loc, speed` | bool | Records a camera scroll to point `loc` at `speed` → 1. | E (0) | 00572c70 |
| 43 | `target, msg` | void | Records message `(msg, 0, 0)` to `target` (null = level; non-actor → error, nothing) (VM-120). | E | 00578cb0 |
| 44 | `target, msg, a, b` | void | Records `(msg, a, b)`. | E | 00578e50 |
| 45 | `actor, loc, mode` | bool | Records a walk of `actor` to point `loc` following NAV-130: mode 0 / 2 walk, 1 / 3 run; modes 2 / 3 mark the elements added with a variant flag whose effect is open (4.1). Not recording / null or non-point location / non-actor / bad mode → error, 0. Completes on arrival or failure (VM-231). | E (0) | 005740b0 (medium for modes 2/3) |
| 46 | `actor, loc, dir, b` | bool | "Enter the game": if `actor` is not yet in the recording's entered list, it is placed at point `loc` facing `dir` (−1 = its own facing) and registered; then a walk (walk if `b` = 0, run otherwise) is recorded. The byte returned is unspecified (the low byte of an internal value; never read by the scripts). | E (0) | 00577650 (medium) |
| 47 | `actor, loc, dir, b` | bool | Variant of 46 (position taken from the actor when already registered; facing `(dir − 8) mod 16`) → 1. | E (0) | 005779a0 (medium) |
| 48 | `actor, loc` | bool | Records "turn to face point `loc`" → 1. | E (0) | 00574e70 |
| 49 | `x, anim` | bool | Records "play animation `anim` once" on an actor or a target object → 1; completion open (VM-231). | E (0) | 00573190 |
| 50 | `x, anim` | bool | Records "loop animation `anim`" on any known element → 1. | E (0) | 00573280 |
| 51 | `x, anim` | bool | Records "play `anim` and freeze on its last frame" (actor or target object) → 1. | E (0) | 00573370 |
| 52 | `actor` | bool | Records "lock the AI of `actor`" (known actor) → 1 (AI section 5.1 for the lock's effect). | E (0) | 00575120 |
| 53 | `actor` | bool | Records "unlock the AI" → 1. | E (0) | 005751d0 |
| 54 / 55 | – | bool | Records "lock" / "unlock player input" → 1; not recording → 0. | E (0) | 005753d0, 00575470 |
| 56 | `n` | bool | Records a timer of `n` ticks (VM-221); byte = recording step's result. | E (0) | 00575350 |
| 57 | `actor, target, k, v` | bool | Records "seek actor `target`" (walk if `k` = 1 else run) with parameter `v`; byte = recording result. | E (0) | 00573d10 (medium) |
| 58 | `x` | bool | Always 0 (placeholder). | – | 006390a0 |
| 59 | `actor, code, arg` | bool | Records an action of `actor` (known element): 0 → a default action; 1 → turn to direction `arg mod 16`; 2, 3, 6, 7 → four fixed actions; 4 → shoot at element index `arg` (must be a valid table index); 5 → begin a sword fight with element `arg`; 8..16 → nine sword figures against element `arg`; 17 / 18 → look sideways (soldier only); 19, 20 → two fixed actions. Other codes / invalid `arg` → error, 0; else 1. Completion open (VM-231). | E (0) | 00573450 |
| 60 | `actor, a, b` | bool | Records "replace animation `a` by `b`" for the actor → 1. | E (0) | 00574f80 |
| 61 | `actor, a` | bool | Records "restore animation `a`" → 1. | E (0) | 00575050 |
| 62 | `pc, text, flag` | bool | Records "speak text `text` with flag `flag`" for a **player character** → 1; non-PC → error, 0. | E (0) | 005730b0 |
| 63 | `actor, corpse, b` | bool | Records the "take corpse" walk-and-take: `actor` a PC with a carrying skill, `corpse` an actor → 1. | E (0) | 00574a70 (medium) |
| 64 | `actor, loc, mode` | bool | Walk into a building (NAV section 5): the nearest type-1 door within 300 px of point `loc` is chosen and native 45 is recorded to it; no door / not a point → error, 0. | E (0) | 00574860 |
| 65 | `actor` | bool | Records "leave the carried corpse" (PC with the skill) → 1. | E (0) | 00574db0 |
| 66 | `x` | bool | Resets an animated map element's animation → 1; other kinds → error, 0. Unused. | E (0) | 00570dd0 (medium) |
| 67 / 68 | `i` | void | Records "start" / "stop mobile element" for **player character `i`** (index into the PC list, unchecked). | U | 005783b0, 00578430 |
| 69 | `actor, k` | bool | Records "speak remark `k`" (0..119) for a human → 1; `k` ≥ 120 or non-human → error, 0. | E (0) | 00572ef0 |
| 70 | `actor, target, k, range, reporter, msg` | bool | Records "seek `target`" (walk if `k` = 0 else run, float `range`) with an attached message `(msg, 0, 0)` to `reporter` (null = level) sent when the seek ends → 1; `reporter` not an actor → error, 0. | E (0) | 00573da0 |
| 71 | `…, msg, a, b` | bool | As 70 with `(msg, a, b)`; `msg < 1000` → error, 0. Unused. | E (0) | 00573f00 |
| 72 / 73 | `i` | void | Records "activate" / "deactivate mobile element" for player character `i` (unchecked index). | U | 005784b0, 00578530 |
| 74 | – | handle | The current actor (VM-093). | – | 00578050 |
| 75 | – | int | The element table's size `N` (VM-030b). | – | 005714d0 |
| 76 | `x` | bool | `x` is an animated map element. | – | 00570a70 |
| 77 | `x` | bool | `x` is of the target family; null → 0; unknown element → error 0. | E (0) | 00570ae0 |
| 78 | `x` | bool | `x` is of the actor family (same rule). | E (0) | 00570a90 |
| 79 | `x` | bool | `x` is a player character. | E (0) | 00570b80 |
| 80 | `x` | bool | `x` is an NPC. | E (0) | 00570c10 |
| 81 | `x` | bool | `x` is a soldier. | E (0) | 00570c60 |
| 82 | `x` | bool | `x` is a civilian. | E (0) | 00570cb0 |
| 83 | `x` | bool | `x` is an animal; null → error 0. | E (0) | 00570bd0 |
| 84 | `x` | bool | `x` is a cart. | E (0) | 00570b30 |
| 85 | `x` | bool | `x` is null. | – | 00570a50 |
| 86 | `a, b` | bool | `a == b`. | – | 00570a60 |
| 87 | `x` | bool | Human `x` is dead; non-human → error 0. | E (0) | 00579a70 |
| 88 | `x` | bool | Human `x` is unconscious. | E (0) | 00579ac0 |
| 89 | `x` | bool | Human `x` is tied up (action 18). | E (0) | 00579b10 |
| 90 | `x` | bool | Human `x` is dead, tied up or unconscious. | E (0) | 00579b60 |
| 91 | `x` | int | **Excluded (4.2).** A posture code of human `x` from the set {0, 2, 4, 5, 6, 8, 9, 10, 11, 15, 16, 17} or 666 for other states; non-human / unknown → −1. The meaning of the codes is open (AI-066, its open question 4). | E (−1) | 005762d0 |
| 92 | `x, k` | void | **Excluded (4.2).** Set a posture of human `x`: accepted codes 0, 2, 7, 10, 15, 16, 17, 100; codes 4, 5, 6, 8, 9, 11 → error "cannot be set"; others → error. | E | 005763e0 |
| 93 | `x` | int | Facing direction 0..15 of a known element; unknown → 0. | E (0) | 00571a70 |
| 94 | `x, d` | bool | Facing := `d mod 16` → 1 (carts through their own setter). | E (0) | 00571ab0 |
| 95 | `x` | handle | A new point location at `x`'s position; unknown → null. | E (null) | 005717a0 |
| 96 | `x, loc` | bool | `loc` null: take `x` off the map (inactive and flagged off-map; humans stop; a present PC is removed from play; an unlocked NPC has its AI locked). Else `loc` must be a point whose ground allows standing: `x` is put back if it was off, moved there, its movement stopped → 1. Not a point → 0; ground not walkable → error, 0 **after** the zone was already changed (E+). | E / E+ | 005718d0 |
| 97 | `x, zone` | bool | `x` is inside `zone`; not a zone → error 0; an inactive target-family element is never inside. | E (0) | 00571de0 |
| 98 | `x, building` | bool | `building` null → `x`'s zone is a building interior (NAV section 5); else → `x`'s zone is `building`. `x` unvalidated. | U | 00577cf0 |
| 99 | `x` | bool | Un-blip (reveal the silhouette, AI-074) if `x` is marked → 1; else 0; unknown → error 0. | E (0) | 00570e70 |
| 100 | `x` | int | 1 if `x`'s movement style is the third style, else 0; unknown → 0. Unused. | E (0) | 00571550 |
| 101 | `x` | int | Current action id: target family → its action; actor family → its current action, 283 if none; others → error 0. | E (0) | 00578060 |
| 102 | `x, amount, b` | void | Damage `amount` on actor `x` of the element table, intensity flag 100 if `b` else 0, through a one-element sequence executed on the next drain (AI section 5.1). Not such an actor → error. | E | 00577500 |
| 103 | `x` | bool | Stop actor `x` (its "stop" with reason 6) → 1; non-actor → error 0. | E (0) | 00571b30 |
| 104 | `npc, x` | bool | NPC `npc` sees human `x` (AI-066). Unused. | E (0) | 00578160 |
| 105 | `npc` | void | Enable the view-cone display of `npc`; a non-NPC is reported **but the call proceeds** (E+). Unused. | E+ | 005786b0 |
| 106 | – | bool | A HUD flag byte. Unused. | – | 00578820 |
| 107 | `b` | void | Set that HUD flag (event when it changes). Unused. | – | 00578830 |
| 108 | `x, a, b` | bool | Runs `FilterAIEvent(a, b)` directly, result ≠ 0. Unused. | – | 00578690 (medium) |
| 109 | `target, msg` | void | Send `(msg, 0, 0)` **now** (VM-120); bad target → error. | E | 00578d80 |
| 110 | `target, msg, a, b` | void | Send `(msg, a, b)` now. | E | 00578f20 |
| 111 | – | handle | **Always null** in this build (placeholder). | – | 00579a20 |
| 112 | `k` | bool | Selection: 31 → select all, 0 → select none, else error; returns 1 always. | E (1) | 00578710 |
| 113 | `x` | bool | Deactivate `x` → 1: cart → its own deactivation; PC → removed from play; others → inactive. Unknown → error 0. | E (0) | 00575640 |
| 114 | `x` | bool | Activate (inverse) → 1; null / unknown → error 0. | E (0) | 005756f0 |
| 115 | `pc, k, b` | bool | Action `k` (0..5) of `pc` available := `b` (HUD events) → 1; non-PC or `k` out of range → error 0. | E (0) | 00575760 |
| 116 | `pc, k` | bool | Action `k` available for `pc` (both disable flags clear); non-PC → error 0. | E (0) | 00575870 |
| 117 | `x, prop, v` | bool | Set property → 1: 0 arrows (human; 0 and no effect without a quiver), 1 money (NPC), 2 life points (human), 3 concussion (human), 4 purses, 5 stones, 6 apples, 7 ales, 8 legs, 9 plants, 10 nets, 11 wasp nests (PC inventory counters), 12 name preset 0 / 1 / 2 (PC). Wrong kind / property → error 0. | E (0) | 00572300 |
| 118 | `x, prop` | int | Get property (same numbering: 0 arrows, 0 without a quiver; 2 life as signed 16-bit; 3 concussion; 4..11 counters); errors → −1. | E (−1) | 00571fb0 |
| 119 | – | bool | Some civilian in the level is dead. | – | 005761f0 |
| 120 | – | bool | Some soldier is dead. Unused. | – | 00576260 |
| 121 / 122 | – | int | Highest alert state (0..2) over living soldiers / over living non-soldier NPCs. Unused. | – | 00576a30, 00576af0 |
| 123 | `npc, s` | bool | Alert state := `s` (0, 1, 2) of an NPC → 1; PC / other / bad `s` → error 0. | E (0) | 00575af0 |
| 124 | `npc` | int | Alert state 0..2; non-NPC → error 0. | E (0) | 00575bf0 |
| 125 | `npc, s` | bool | Set the AI's state from the script: 1 → default (patrol), 3 → seeking (soldiers only; civilians → error 0), 5 → fleeing, 7 → return to the post; 0 (sleeping), 4 (menacing), 6 (attacking) cannot be set → error 0; others → error 0. Non-NPC → error 0. (State names: AI-041.) | E (0) | 00575c80 (medium) |
| 126 | `npc` | int | The NPC's top-level AI state (AI-041) as a script code: 1 default, 2 wondering, 3 seeking, 4 menacing, 5 fleeing, 6 attacking, 0 sleeping / unknown; non-NPC → error 0. (`spec-ai-combat.md` 5.1 gives 4 fleeing / 5 sleeping / 6 attacking: to reconcile, 9.4.) | E (0) | 00575e20 |
| 127 | `a, b` | bool | Obsolete: always error, 0. | E (0) | 00575ee0 |
| 128 | `npc` | int | 1 if the NPC is hostile (its attitude, AI section 5.1), else 0; non-NPC → error 0. | E (0) | 00575f00 |
| 129 | `npc, a, b` | bool | No effect; 1 for an NPC, error 0 otherwise. Unused. | E (0) | 00575f70 |
| 130 | `npc, target, b` | void | Validates `npc` (NPC) and `target` (known element), then a routine that is **empty in this build**: no effect (AI section 5.1). | E | 00575fc0, 00486f50 |
| 131 | `npc, loc, b` | void | Validates (NPC, point) then the same empty routine: no effect. Unused. | E | 005760b0 |
| 132 | `npc, path` | void | Assign the patrol path (AI section 3.4); non-NPC → error. | E | 00576c50 |
| 133 | `npc, loc, d` | void | Set the NPC's post at point `loc` facing `d`; non-NPC → error. | E | 00576cc0 |
| 134 | `x, b` | void | Lock the AI: NPC → its lock with flag `b` (AI section 5.1); animal → its lock flag := 1; PC / other → error. | E | 00576d80 |
| 135 | `x` | void | Unlock the AI: NPC (error if not locked); animal → unlock and wake; PC / other → error. | E | 00576e00 |
| 136 | `soldier, k` | void | Force a battle decision: `k` 0..16 → the seventeen decisions in order, 99 → none; `k ≥ 100` → `k − 100` as a temporary decision; other → error; non-soldier → error. | E | 00576eb0 (medium) |
| 137 | `loc, k` | void | Make a noise at point `loc`: `k` 0 → noise type 11, 1 → type 12; other `k` → error **and the value is still used as the type** (E+); null → error. | E / E+ | 00571160 |
| 138 | `human, b` | void | Freeze flag := `b`; unknown / non-human → error **but still applied** (E+). | E+ | 00576bb0 |
| 139 | `b` | void | Level "freeze all" flag := `b` (immediate form of 226). | – | 00576c30 |
| 140 | `npc, k` | void | Walking style: 0 walk, 1 run, others verbatim (AI-093); non-NPC → error. | E | 005780d0 |
| 141 | `soldier` | int | Rank; non-soldier → error 0. | E (0) | 00579a30 |
| 142 | `x` | bool | Animated map element active flag; other → error 0. | E (0) | 00570d00 |
| 143 | `x, b` | bool | Animated map element active := `b` → 1; other → error 0. | E (0) | 00570d80 |
| 144 | `patch` | bool | Patch active flag; **null unchecked**. | U | 00570e20 |
| 145 / 146 | `patch` | bool | Activate / deactivate the patch, clear the camera-follow flag → 1. | – | 00570e30, 00570e50 |
| 147 | – | bool | Stop all playing level sounds → 1. | – | 00575510 (medium) |
| 148 | – | bool | Restart the ambient level sounds around the camera → 1. | – | 00575530 (medium) |
| 149 | `sound` | bool | Start sound source `sound` (once) → 1. | – | 00575560 (medium) |
| 150 | `sound` | bool | Stop its playing instance → 1. | – | 00575590 (medium) |
| 151 | `sound` | bool | Stop and release it → the release result. | – | 005755c0 (medium) |
| 152 | `x` | bool | Take actor `x` (of the table) out of its building (NAV-191) → 1; outdoors / not such an actor → error 0. | E (0) | 00577150 |
| 153 | `x, zone` | bool | Fire `ExitZone(x)` for a **member** `x` of `zone` → 1 (VM-120b); not a zone / not an actor / not a member → error 0. Unused. | E (0) | 00577220 |
| 154 | `x, zone` | bool | Fire `EnterZone(x)` for a **member** `x` → 1; otherwise error 0. Unused. | E (0) | 00577390 |
| 155 | `x` | void | No effect. | – | 00578c50 |
| 156 | `x, building` | void | Put actor `x` inside `building` (NAV-190): entry point, zone := the building, default facing, hidden, registered; not an actor → error **but the call proceeds** (E+). | E+ | 005781f0 |
| 157 | `x, b` | void | **Excluded (4.2).** Sets a flag `b` on an object and on each entry of one of its lists. Unused. | – | 005782a0 |
| 158 | `zone` | handle | First actor inside `zone`, null if none; not a zone → error null. Unused. | E (null) | 00571ea0 |
| 159 | – | handle | **Always null** (same placeholder as 111). | – | 00579a20 |
| 160 | `a, b` | int | Distance between two points (float, truncated); a non-point → error 0. | E (0) | 00571b80 |
| 161 | `n` | int | `rand() mod n` with the C runtime generator (8.2); `n = 0` traps. | T | 00578680 |
| 162 | `x` | void | Debug output of `x` as a decimal number (no game effect). Used once. | – | 00579f10 |
| 163 | – | int | Size of the campaign's character roster list. | – | 005791d0 |
| 164 | `i` | handle | Element for roster entry `i` (unchecked). | U | 005791f0 |
| 165 / 166 | `pc` | void | Add / remove `pc` to / from the next mission's team (HUD refresh); non-PC → error. 165 also marks the PC present and resets two of its fields. | E | 00579210, 00579280 |
| 167 | – | int | Size of the selected mission's team list. | – | 005792d0 |
| 168 | `i` | handle | Element of team entry `i`; `i` beyond the list → **fatal** (F). | F | 00579350 |
| 169 | `x` | bool | `x`'s campaign character id is in the selected mission's team list. | – | 005793a0 |
| 170 | – | bool | Low byte of the campaign routine "the team satisfies the selected mission's requirements". | – | 005793e0 (medium) |
| 171 | – | int | First code of the campaign's "available missions" list, 0 if empty. | – | 005793f0 (medium) |
| 172 | – | int | Code of the selected mission, 0 if none. | – | 00579410 |
| 173 | – | bool | **Excluded (4.2).** A settings byte (the fifth entry of the options store); placeholder 0. Called 14 times, result read. | – | 005795b0 |
| 174 | – | int | Team size limit of the selected mission; 5 if none; outside the hub node (type 8) → error 0. | E (0) | 00579300 |
| 175 | `id, loc` | void | Place the PC with campaign id `id` at point `loc`; none → nothing. | – | 00579000 |
| 176 | `soldier, n` | void | Company number := `n`; non-soldier → error. | E | 005790b0 |
| 177 | `soldier, b` | void | Always-attentive := `b` (AI section 5.1); non-soldier → error. | E | 005790f0 |
| 178 | `banner` | void | Capture a banner (must be active; else error): deactivated; campaign counter 3 += its value; in a type-1 node reaching the requirement **declares victory** (VM-103b); HUD refresh. | E | 005795c0 |
| 179 | `banner` | void | Lose a banner (inactive → active, counter 3 decreased, HUD refresh); null → error, then unchecked. | E / U | 005796d0 |
| 180 | `human, b` | void | Invisible flag := `b`; non-human → error. | E | 00579760 |
| 181 | `human` | bool | Invisible flag; non-human → error 0. | E (0) | 005797c0 |
| 182 / 183 / 184 / 185 | `door` | bool | Door lock bytes: 182 player lock, 183 the lock-pick byte, 184 soldier lock, 185 civilian lock (NAV-172); **null unchecked**. | U | 00579810 - 00579840 |
| 186 / 187 / 188 / 189 | `door, b` | void | Set those bytes; 186 / 188 / 189 with `b = 0` also open the door (NAV-172); null unchecked. | U | 00579850 - 005798a0 |
| 190 | `x, a, b` | void | **Excluded (4.2).** Forwards `(a, b)` to a door / lift routine. Unused. | – | 005786f0 |
| 191 | `b, door` | void | The door leaf's click-target flag := `b` (NAV section 5); null → error. | E | 005787f0 |
| 192 | – | handle | The current scroll (VM-094). | – | 005798c0 |
| 193 | `scroll` | int | Scroll status 0..3; not a scroll → error 0. | E (0) | 005798f0 |
| 194 | `scroll, s` | void | Status := `s` (0..3; else error). 1 and 3 make the scroll visible, 0 and 2 hide it; 3 also plays the scroll sound. Not a scroll → error. | E | 00579950 |
| 195 | `k` | int | Campaign value `k + 7` for `k` in 0..19; else error 0. | E (0) | 00579430 |
| 196 | `k, v` | void | Set it; else error. | E | 00579470 |
| 197 | `npc, k` | int | NPC custom value `k` (0..9, AI section 5.1); errors → −1. | E (−1) | 005794b0 |
| 198 | `npc, k, v` | void | Set it; errors → nothing. | E | 00579520 |
| 199 | `k, loc, n` | void | Camp production zone `k` at `loc` with capacity `n`. | – | 005799d0 (medium) |
| 200 | `k, loc` | void | Camp production zone `k` at `loc`. | – | 00579a00 (medium) |
| 201 | `id` | handle | The player-character slot with campaign id `id`; `id` beyond the slot count → error null; none → null. | E (null) | 00571610 |
| 202 | `k` | void | Show popup page `k` **now** in the synchronous modal loop (VM-218). | – | 00579f30 |
| 203 | `k` | void | Records "popup page `k`" (modal when executed). | E | 00579fa0 |
| 204 | `zone` | int | Number of actors inside `zone`; not a zone → error 0; null → 0. | E (0) | 0057a060 |
| 205 | `zone, i` | handle | `i`-th actor inside (`i` < count) else error null. | E (null) | 0057a0a0 |
| 206 / 207 / 208 | `a, b` | int | `a & b`, `a \| b`, `a ^ b` (full 32-bit results). Unused. | – | 0057a280 - 0057a2a0 |
| 209 | `pc, k` | bool | PC has skill `k` (0..29 except 22 → error 0) by either of two skill tests; non-PC → error 0. | E (0) | 0057a2b0 |
| 210 | `k` | bool | Some player character (any, in PC-list order) has skill `k` by either test; bad `k` → error 0. | E (0) | 0057a4c0 |
| 211 | – | handle | The first PC flagged as the leader, or null. | – | 0057a8e0 |
| 212 | `actor, loc, mode, d` | bool | Records a walk "near" point `loc` at distance `d` (modes as 45; variant flag, 4.1) → 1. | E (0) | 005743a0 (medium) |
| 213 | `a, b, t` | handle | New point at `a + (b − a) × t` (float `t`); both points in the same zone, else error null. | E (null) | 00571c60 |
| 214 | `soldier` | void | Declare combat trainer; non-soldier → error. | E | 00579160 |
| 215 | `k` | handle | The active relic / information scroll of kind `26 + k` (k 0..6) in the level, or null. | – | 0057a950 |
| 216 | – | int | Number of player characters (PC list, `mod 256`). | – | 0057aa20 |
| 217 | `i` | handle | Player character `i mod 256` of the PC list (unchecked). | U | 0057aa50 |
| 218 | `chief, sub` | void | Add `sub` as subordinate of `chief` (AI-097): both NPCs, neither already subordinate, `sub` not a chief, not self; else error. | E | 0057aa70 |
| 219 | `npc` | void | Remove all subordinates; non-NPC → error. | E | 0057acc0 |
| 220 | `soldier` | void | Switch to the alert path (AI section 5.1); non-soldier → error. | E | 0057ad30 |
| 221 | `x` | bool | Soldier `x` is a rider; null / non-soldier → 0. | E (0) | 0057ad90 |
| 222 | `x` | bool | `x`'s highlight mark is clear (the mark 99 clears; also raised by 186 / 188 / 189 when opening). | E (0) | 00570ec0 |
| 223 | `banner` | bool | Banner inactive (= captured); null → error then unchecked. | E / U | 00579730 |
| 224 | `loc, a, b, c` | int | **Excluded (4.2).** Creates a repulsive point (trap) at point `loc` with `(a, b, c)` and returns that routine's result; not a point → error 0. Called 159 times, result unused. | E (0) | 0057adf0 |
| 225 | `n` | void | **Excluded (4.2).** A level parameter := `n` and every element refreshed. Unused. | – | 0057ae60 |
| 226 | `b` | void | Records "freeze-all flag := `b`". | E | 00577df0 |
| 227 | `x` | void | **Excluded (4.2).** Forwards to a trap routine. Unused. | – | 0057ae40 |
| 228 | `npc, k, v` | void | Head marker (AI-084): `k` 0 clears; 1..7 → markers 2..8 for `v` frames; else error; non-NPC → error. | E | 0057aed0 |
| 229 | `human` | void | Confiscate: a non-PC human's money is added to campaign counter 1 and zeroed; non-human → error. | E | 005729a0 |
| 230 | `zone` | bool | Every player character is inside `zone`. Unused. | – | 00571e30 |
| 231 | `zone` | bool | No active hostile soldier in `zone` can still fight (alive, not unconscious, not tied, not fleeing); not a zone → error 0. | E (0) | 00571ef0 |
| 232 | `pc` | void | Add `pc` to the player's band (campaign roster); errors reported but the roster update **proceeds** for a non-PC (E+). | E+ | 0057afe0 |
| 233 | `npc, scroll` | void | Attach information scroll `scroll` (null detaches) to `npc`; a non-scroll is reported **and still attached** (E+); non-NPC → error. | E / E+ | 005785f0 |
| 234 | – | bool | Campaign counter 3 ≥ the required banner count; 0 outside a campaign. | – | 00579690 |
| 235 | `item` | bool | Bonus item picked-up flag; not a bonus item → error 0. | E (0) | 0057b080 |
| 236 | – | int | Campaign money (counter 1); outside a campaign → error −1. | E (−1) | 0057b0f0 |
| 237 | `v` | void | Set campaign money; outside → error. | E | 0057b120 |
| 238 | – | int | **Excluded (4.2).** A HUD value. Unused. | – | 0057b150 |
| 239 | – | void | Show the debriefing screen. | – | 0057b160 (medium) |
| 240 | `x` | bool | Active flag of a known element; unknown → error 0. | E (0) | 00570d40 |
| 241 | `k, a, b` | void | **Excluded (4.2).** Campaign: a random-pick routine on entry `k − 1` (it also sets the byte native 261 reads), then two values set on an object not identified. Used once. | – | 0057b2b0 |
| 242 | `pc, a, b` | void | Set the two experience values of `pc`; non-PC → error. | E | 0057b2f0 |
| 243 | `actor` | bool | Records "un-blip `actor`" → 1; not a known actor → error 0. | E (0) | 00575290 |
| 244 | `x, b` | void | Active flag of the element linked from `x` := `b` (`x` non-null; a player slot's character). | – | 0057b350 (medium) |
| 245 | – | int | Number of player characters not dead. | – | 0057b370 |
| 246 | `zone` | bool | Every living player character is inside `zone`. | – | 0057b3d0 |
| 247 | `target` | void | Convert a "handle" target object into a "take" target; wrong kind / state → error. | E | 0057b450 |
| 248 | `pc` | bool | `pc` is selected (null: any selection); non-PC → error, 1. | E (1) | 0057b4f0 |
| 249 | – | int | Number of selected player characters. | – | 0057b5e0 |
| 250 | `i` | handle | `i`-th selected player character; `i` ≥ count → error null. | E (null) | 0057b5f0 |
| 251 | – | void | Stop a sound category. Unused. | – | 0057b780 (medium) |
| 252 | `pc` | void | Make `pc` crouch; non-PC → error. | E | 0057b7a0 |
| 253 | `k` | bool | Some player character with skill `k` is lost (dead, or not in the band with a captured mark and recorded lost by the campaign); bad `k` → error 0. | E (0) | 0057b800 (medium) |
| 254 | `x, b` | void | A flag byte of `x` := `b` (unchecked). | U | 0057ba50 |
| 255 | `k` | bool | Some *present* player character has skill `k`. | E (0) | 0057a6d0 |
| 256 | `pc` | int | Campaign identity of `pc`: 0 the hero, 1..5 the five named companions (John, Tuck, Stuteley, Scarlet, Marian), 6..8 the three generic followers; unknown profile → error −1; non-PC → error −1. | E (−1) | 0057ba60 |
| 257 | `pc, b` | void | Select (`b`) / deselect `pc` (HUD events); null → all; non-PC → error. | E | 0057bdb0 |
| 258 | `pc` | bool | `pc` is selected and an action is selected in the HUD; non-PC → error 0. | E (0) | 0057bed0 |
| 259 | `human` | int | Action state 0..17, else 666; non-human → −1. | E (−1) | 005766f0 |
| 260 | `human, k` | void | Set action state `k` (0..17); non-human → error. | E | 00576830 |
| 261 | – | bool | **Excluded (4.2).** A campaign byte set by the random-pick routine of 241 (also run at a successful end) and cleared at a successful end. Used twice. Placeholder 0. | – | 0057bfe0 |
| 262 | `n` | void | Screen fade in `n` steps. Used once. | – | 0057bff0 (medium) |
| 263 | `target, fx` | void | Link an animated map element to a target object; wrong kinds → error. Unused. | E | 0057c500 |
| 264 | `npc, k, b` | void | Forbid remark `k` (< 120) with flag `b`; non-NPC / bad `k` → error. | E | 00573010 |

Ids never called by the retail scripts (73 of 265): 11, 14, 15, 22, 23, 25, 36, 37, 57, 60, 61, 63, 65, 66,
67, 68, 71, 76, 77, 78, 83, 84, 100, 104, 105, 106, 107, 108, 115, 116, 120, 121, 122, 123, 124, 127, 129, 131,
136, 138, 141, 142, 147, 148, 151, 153, 154, 157, 158, 167, 168, 169, 171, 175, 176, 181, 183, 184, 185, 190, 201,
206, 207, 208, 225, 227, 230, 238, 242, 251, 257, 260, 263. Their rows are read from the code alone.

## 7. Acceptance tests

Synthetic programs (assembled from the opcode table) and expected outcomes; "tick" = an executed level tick.

1. **Return semantics (VM-047, VM-050, VM-072).** A function `f` = `0x03; 0x13 t,7; 0x07 t; 0x13 t,9; 0x07 t; 0x06`; caller: `0x05 f; 0x0A x` → `x = 7` (the second store never runs). A callback whose body is `0x03; 0x06` after another callback of the same instance returned 5 → the engine reads 5.
2. **Persistent result slot (VM-050).** Caller: `0x05 f; 0x13 y,1; 0x0A x` → `x = 7`; caller: `0x05 f; 0x05 g` (g returns 3); `0x0A x` → `x = 3`.
3. **Nested callback (VM-095).** Level `Hourglass`: `n1(0, 1); n109(null, 5); n2(0) → x`; level `ProcessMessage(5,…)`: `n1(0, 2)` → `x = 2`, and the Hourglass continues to its end.
4. **Barrier (VM-201).** `n30(); n32() → a; n32() → b; n56(5); n32() → c; n31()` → `a = 1, b = 1, c = 2`, one sequence with the timer at level 1. `n30(); n30() → r` → `r = 0`, the recording unchanged.
5. **Recording outside a recording (VM-203).** `n56(5)` alone → error, nothing scheduled; `n43(null, 1)` alone → no `ProcessMessage`; `n109(null, 1)` → `ProcessMessage(1,0,0)` runs before `n109` returns.
6. **Simultaneous timers (VM-221).** Sequence: level 1 = timers 3 and 3; level 2 = message 9 to the level. Launched from `Hourglass(0)` (step 4 of tick 0), both timers are inserted before that tick's timer pass, which already visits them: counters 3 → 2 (tick 0), 2 → 1 (tick 1), 1 → done (tick 2). Both complete in the pass of tick 2 in insertion order; `ProcessMessage(9)` runs inside that pass, before the pass visits any later timer. A timer inserted by that message is first visited in tick 3.
7. **Hourglass schedule (VM-103).** With no forcing: `Hourglass(0)` and `CheckVictoryCondition(0)` at T = 0; `Hourglass(1)` at T = 25 without a check; `CheckVictoryCondition(3)` at T = 75. `n29()` during `Hourglass(1)` → `CheckVictoryCondition(2)` at T = 50, and `(3)` still at T = 75.
8. **Victory and end (VM-103b).** `CheckVictoryCondition` returns 1 at T = 75 with two present PCs → no `Finalize`; the notice is shown once (step 2); the mission ends only when both PCs have died or left: on that tick the end bookkeeping runs; the next tick runs `Finalize(0)` and ends with code 2. Returning 2 at T = 150 without a prior victory → bookkeeping in that tick (T stays 150), `Finalize(1)` next tick, code 3.
9. **Modal suspension (VM-218).** Sequence: level 1 = page 4; level 2 = timer 1. The page opens when level 1 is dispatched; until the player dismisses it, T does not advance and no other sequence progresses; after dismissal the element is done, level 2 starts, and the timer completes at the next timer pass. `n202(4)` inside `IsTaken` returns only after dismissal.
10. **Abort propagation (VM-217).** Sequence: level 1 = walk of a dead actor, timer 2, message 7; level 2 = message 8. The message 7 runs at dispatch; the walk is refused when the queue is drained (state refused, propagated: message 8 is refused too); the timer still completes in its pass; level 1 never completes; `ProcessMessage(8)` never runs; the sequence is deleted at the next housekeeping.
11. **Message target (VM-120).** `n43(null, 1)` in a recording whose sequence launches → level `ProcessMessage(1,0,0)`; `n43(scroll, 1)` → error, nothing; `n44(actor, 2, 3, 4)` → that actor's `ProcessMessage(2,3,4)` with native 74 = that actor inside it.
12. **Context (VM-093, VM-094).** Inside an actor's `ProcessMessage` that calls `n109(null, 6)`: the level's `ProcessMessage(6)` sees `n74()` = that actor. Inside a scroll's `IsTaken` that sends `n109(actor, 6)`: the actor's handler sees `n192()` = that scroll, `n74()` = the actor; after both return, `n192()` = null.
13. **Invalid inputs (VM-089).** `n3(999)` → null with an error; `n3(-1)` → null silently; `n2(99)` → −1; `n1(99, 5)` → no effect; `n24(x, 444)` → no effect; `n161(0)` → a fault (8.1); `n8(-1)` → null (8.1, the original reads out of range); `n56(0)` → never completes; a jump to `0xFFFFFFFF` → the callback ends (8.1).
14. **Arithmetic (VM-061..VM-068).** `0x24` on (2, 2) → 1; `0x26` on (−1, 0) → 0; `0x2B` on (NaN, 1.0) → 1.0f; `0x2E` on (NaN, 1.0) → 0.0f; `0x2C` on (NaN, NaN) → 0.0f; `0x17` on 4294967296.0 → 0; `0x1C` on (INT_MIN, −1) → a fault; `0x19` on (INT_MAX, 1) → INT_MIN.
15. **Result conventions (VM-086).** `n206(0x1FF, 0x100)` → 0x100; `n128(hostile npc)` → 1; `n112(7)` → 1 with an error; `n46(...)` → 0 even when it recorded; `n2(k)` for a declared `k` holding −5 → −5.
16. **Initialisation order (VM-104).** Trace of a level whose actor `Initialize` sets mission variable 0 := 1 and whose level `Initialize` sets it := 2: the level's value wins; the scrolls' `Initialize` run after it; `PostInitialize` runs after `Hourglass(0)` when the first tick executes.

## 8. Implementation choices

### 8.1 Deliberate departures from the original

| Case | Original | OpenSherwood | Reason |
|---|---|---|---|
| jump to `0xFFFFFFFF` (VM-070) | unchecked read | the callback ends as by 0x06 | the script's intent; UB is not reproducible |
| unchecked accesses (class U: 3 with `i < -1`, 8, 9, 144, 182 - 189 with null, 164, 217, 254, 179 / 223 after their error) | reads / writes arbitrary memory | null / 0 result, `Fault::UncheckedAccess(id)` recorded (`n8(-1)` → null, the scripts' intent "outdoors") | UB |
| arithmetic traps (class T: 161 with 0, opcode 0x1C by zero or INT_MIN / −1) | process exception | deterministic fault of the running callback | crash |
| fatal paths (class F: 168, missing function, overlapping scroll callbacks) | program termination | fault of the callback; a missing function is a recorded no-op returning the register | crash |
| non-termination (class N: opcodes 0x00, ≥ 0x30) | hang | fault | hang |
| argument buffer overflow (VM-087) | unchecked write | fault | UB |
| excluded natives (4.2) | their (unread) effect | `UnknownNative(id)` recorded, placeholder result | not settled |
| version check (VM-001) | float equality (NaN accepted) | bit-exact 1.5 | no retail file is NaN; keep the check strict |

### 8.2 Random numbers

Native 161 draws from the process-wide C-runtime generator (the linear congruential `rand()` of the Microsoft
runtime, 15-bit results), shared with every other consumer in the program (the AI, `spec-ai-combat.md` 2.2).
OpenSherwood uses its named seeded stream for the script (ADR-0004); the consumption order between the script
and the AI within a tick follows VM-103 and is part of the determinism model, not of this spec's fidelity claims.

### 8.3 Snapshot and restore boundaries

Authoritative script state (all hashed): per instance VM-014; the mission-variable array (VM-020); `T`, the
force flag, the won flag and its sub-flag, the success / failure / abort end flags, the debriefing index
(VM-102); every live sequence (elements with category, target, arguments, state, level; the level counters; the
manager's FIFO queue order; the timer list with counters and order); the recording state (VM-200) is empty
between callbacks (recordings never span a native return) and need not be saved; the current actor / current
scroll are null between ticks; element flags read by natives (active, mark, scroll status, door bytes, patch
flags, banner state, PC availability, freeze flags) belong to their owners' snapshots. Restoring at a tick
boundary reproduces the next tick exactly because nothing script-visible lives in frames or buffers between
callbacks.

### 8.4 Differences from the current engine (`docs/formats/scb.md`, `natives.rs`)

1. Opcode 0x07 returns immediately (the engine continues to the dead 0x01 / 0x06).
2. Comparisons: 0x24 is `<=` (engine `>=`), 0x26 `>=`, 0x27 `>`, 0x28 `!=`, signed; 0x2A..0x2F are float compares with float results and the unordered outcomes of VM-068; 0x1C / 0x1F / 0x16 / 0x17 exist.
3. Jump / call / native-call targets are `a | (b << 16)`; the two `-1` jumps (VM-070).
4. Native 2 returns −1 for an unknown variable; 0 grows by 16; 1 errors on undeclared.
5. The element table includes one slot per campaign character (VM-030); native 10 returns cart indices from 0 for carts; native 75 counts the slots. The engine's table lacks the null slots of absent characters and offsets nothing.
6. Natives 111 and 159 are null: messages "to the player" go to the level class.
7. Messages are synchronous (VM-120); 44's fourth argument is `arg2`.
8. The clock: 25 Hz nominal, frame-locked (VM-100); Hourglass every 25 ticks with `T/25`; `CheckVictoryCondition` every third Hourglass or forced; scroll Hourglass on the scroll's own counter with parameter 0; actor classes never get Hourglass; `HandleEvent` is never called; `ActivatedBy*` results are ignored.
9. `Finalize(0)` means success and `Finalize(1)` failure (VM-103b); victory does not end the level (VM-103b); the tick has early exits (VM-103).
10. Initialisation order (VM-104): elements at load, the level, the scrolls, the first tick, then `PostInitialize`.
11. Sequences: level-parallel with a barrier that counts once (VM-201), abort cascade (VM-217), pages and dialogs are synchronous modal loops (VM-218) — 203 does hold the sequence, by suspending the program; 33 / 42 wait for the camera scroll (completed by the camera update, VM-219), 34 is instant; recording natives outside a recording drop their element (VM-203).
12. Native semantics corrected (section 6): 4 / 5 / 6 / 9 are lookups with null on error, not identities; 8 unchecked; 12 / 13 / 16 inverse lookups; 17 a modal dialog; 18 / 19 camera scrolls with a movement scale (not a zoom); 20 camera scroll without follow; 21 zoom; 24 with 444 a no-op; 35 zoom, 36 map, 37 / 38 HUD availability, 39 / 40 camera lock, 41 dialog; 46 / 47 placement + walk; 49 - 51 animation elements completed by the actor; 52 / 53 AI lock elements; 54 / 55 input lock; 57 / 70 / 71 seek (70 / 71 with a follow-up message); 59 actions; 62 / 69 speak; 64 walk to the nearest building door; 72 / 73 hide / show PC `i`; 74 the current actor (the object during `ActivatedBy*`); 80 - 84 category tests; 87 - 90 dead / unconscious / tied; 96 with null takes the actor off the map; 98 with null tests "in a building"; 101 the action id (283 none); 102 damage; 103 stop; 112 select all / none; 117 / 118 property ids; 119 a civilian died; 125 / 126 AI states (AI-041); 128 hostile; 130 / 131 no-ops; 133 post; 137 noise; 140 walking style; 152 / 156 buildings; 160 truncated float distance; 161 `rand() mod n`; 178 / 179 / 223 / 234 banners (178 declares victory); 182 - 189 door lock bytes; 191 the leaf's click flag; 192 the current scroll; 193 / 194 scroll status with visibility; 195 / 196 campaign values `k + 7`; 197 / 198 NPC custom values; 204 / 205 zone membership; 206 - 208 full-width bit operations; 209 / 210 / 253 / 255 skill tests; 211 the leader; 213 interpolation; 216 / 217 modulo 256; 218 - 220 subordinates / alert path; 221 rider; 222 mark clear; 226 freeze-all element; 228 head marker; 229 confiscate; 231 / 246 zone predicates; 232 band roster; 233 attach scroll; 235 item flag; 236 / 237 campaign money (−1 outside a campaign); 240 active; 243 un-blip element; 245 living PCs; 248 - 250 selection; 256 identity; 264 forbid remark.
13. Failure classes (VM-089) replace the engine's strict-mode traps; all 265 ids exist.
14. Temporaries / locals zero at every entry; the return register persists.
15. Natives 30 / 31 / 32 return values; 30 while recording is an error returning 0.

## 9. Open questions

### 9.1 Actor-side element execution (navigation / AI / animation)
- Completion rules of walks beyond NAV-152 / NAV-142 (the move element's own completion, NAV open question 3), animations 49 - 51, actions of 59, speak 62 / 69, corpse 63 / 65: 004646e0 (admission), 00467a50 (the actor's element runner, 4.9 KB), 0046bd40 (done notification), 0046b210 (admission predicates through the actor's virtual slots).
- The variant flags set by modes 2 / 3 of 45 and by 212 (00582640, 00583630); what 46 / 47 do with the entered lists beyond the first placement (00577650, 005779a0, 0057c950).
- The actor's priority / merge rules when it is busy (004646e0 cases 0..3, 0046ad00).

### 9.2 Excluded natives
- 13: the map's zone list (00579d70; built by 004fb8e0) versus the level's location list (005714f0, points then polygons): are the polygon locations the same objects in the same order?
- 173: the settings byte (the fifth option of the options store, set by 004b90f0): its meaning.
- 224: the repulsive-point routine 004ec910 (parameters and result); 225 (0057ae60, 0048ebe0); 227 (004eca10).
- 241 / 261: the campaign random-pick routine 004524b0 and the byte it sets; the object written by 0051d540.
- 91 / 92: posture codes (AI-066, open question 4; 005762d0, 005763e0).
- 157 (005782a0), 190 (005786f0, 0057dea0), 238 (0057b150, 0055dbb0): unused; low priority.
- Events 100..106 of the second AI dispatcher (0040dcb0; raised from the actor's order machinery 0046bcb0 / 0046bcf0 callers).

### 9.3 Scheduler details
- The three "cinematic / transition" flags of VM-103 step 6 (level offsets 0x4790, 0x4791 and 0xD0: which screens set them) and the debug "no defeat" toggle.
- The "may die" civilian flag and the captured-PC field of the defeat checks (004c6ef0).
- Whether the first tick can be skipped on the first loop pass by the modal-window guard (0050f710).
- The scroll's per-element update source (004b9f40 through its virtual slot).
- Whether script zones are registered into the element table and at which position (0057f8c0, 004fb8e0, called before the post-load step 004c2720 of the level loader 004c0510).
- The immediate action after a declared victory in campaign nodes of type 3 or 6 (0050eca0: an interface-mode switch, callers 004e3220).

### 9.4 Contradictions with the AI spec to reconcile
- Native 126's code order (this spec: from the state order of AI-041 and the case table at 00575e20; the AI spec's row: 4 fleeing, 5 sleeping, 6 attacking).
- Native 134 on a player character (this spec: error, no effect, 00576d80; the AI spec: "a player character gets a lock byte").
- Native 59 (this spec: an action element; the AI spec: "a play-animation step").

## 10. Provenance

Ghidra project `re/ghidra/robinhood` (never committed); decompilation export `re/out/decomp_all/<address>.c`,
inventory `re/out/inventory.tsv`, strings `re/out/strings.tsv`, module map `re/notes/modules.txt` (git-ignored);
raw-byte disassembly with `scripts/ghidra/peek.py` and a local capstone session for the functions the decompiler
mis-typed (the wrapper table 004075c0 and its 265 wrappers, the frame helpers, the activation dispatcher
004bc1d0, native 210's loop, the message call sites of VM-113). Analyst notes: `re/notes/vm/` (wrapper
disassembly, id → function table with arity and result convention, per-id dump, usage statistics).

Functions read (all): 00634bb0 - 00639a80 (interpreter and instance), 0063a140 - 0063a510, 0063b2d0, 0063a940,
0063b070, 004075c0 and 00404a80 - 004075bf; natives 00570a30 - 0057c500 with helpers 004e3d20 - 004e3e30,
005709c0, 00570850, 004c2720, 004d8460, 0051d0e0, 0052b070, 004524b0, 0051d540, 0053a0b0, 0053a1a0, 0053a4c0,
0052b100, 004bae60, 004ba760, 004ba5c0, 004ba7a0, 004ba7e0, 004b9f30, 005a87f0, 005a8810, 005a88a0, 005a8780,
005a8700; scheduler 004c6ef0, 0050f710, 0050e7b0, 0050e7d0, 0050e800, 004c3740, 004ba000, 004b9fa0, 004b9f40,
004ba4e0, 004e3220, 004e3260, 004e3cd0, 005105d0, 004c8380, 004cdfc0 (camera completion), 0050b640 (the hub
flag); invokers 004030a0 - 00408af0 and callers 0046e7e0, 004a0f10, 004bb820, 00551bb0, 0057f8c0, 004bc3d0,
004105d0, 004abfe0, 0040dcb0, 00410620, 00464230, 0057fcc0, 0057fdb0, 00467230, 004bc1d0 (bytes); sequences
00570f00, 00571020, 00571100, 0057a020, 005866a0, 00584d60, 00585ea0, 00586ed0, 00587010, 00586e00, 00586fd0,
00585500, 00582220, 0058a3d0, 0058a940, 00582530, 00582560, 00582620, 005823f0, 0058a430, 0058bb80, 00585b70,
00585570, 00585320, 00585710, 0058ba60, 004ca410, 004646e0, 0046b210, 0058bbe0, 0058bff0, 0058c360, 0058a9c0,
005843c0, 00584980, 005849e0, 00584b40, 00584de0, 005822a0, 0058ad80, 0058af00, 0058b110, 0058b280.

Data checks: `python harness/tools/probe/scb_opstats.py <levels>`, `scb_semantics.py <levels> --natives`,
`--params`, `--pseudo --class`, and an ad-hoc probe over the parsed quads (0x07 successors, functions without a
value return) on the game data copy at `C:\Users\przem\source\gamedata\robinhood` (read-only). Corpus facts used
in section 3: 208 679 instructions, 42 734 native calls, 192 distinct ids, maximum arity pushed before one call
6, 0x07 successors 5081 no-ops / 47 immediate loads / 27 jumps / 4 other (static adjacency, not executed flow),
value returns absent in all `Finalize` and `PostInitialize`, 1199 / 1448 `Initialize`, 235 / 274 `Hourglass`,
376 / 376 `ProcessMessage`, 68 / 312 `EnterZone`. No oracle recording was used; the 40 ms pacing is a code fact;
the 60 Hz / 64 Hz conversions refer to `docs/original/stealth-and-combat.md`. Sibling specifications referenced:
`docs/original/spec-navigation.md` (NAV-130, NAV-142, NAV-152, NAV-172, NAV-190, NAV-191, section 5),
`docs/original/spec-ai-combat.md` (AI-041, AI-066, AI-074, AI-081, AI-082, AI-084, AI-091, AI-093, AI-097,
AI-190, sections 2.2, 3.4, 5.1). Tests that will depend on this spec: section 7 (the VM rebuild, ADR-0009).
