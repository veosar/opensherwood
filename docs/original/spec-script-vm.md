# Script VM, natives and callback scheduler (behaviour specification)

Status: `draft`, revision 3 (answers Codex reviews 14 and 17; awaiting re-review). Build: GOG English edition,
`Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, image base
0x00400000; every address below is a virtual address in that image.

Identity and handoff record:
- Analyst: 2026-09-13, session `a45d5359e8dec5140` (Claude agent, analyst role under ADR-0009), revisions 1..3.
- Reviews: Codex gpt-6-astra — review 14 covered **revision 1** (blob `92aedbf44c6e3e974bc44beb5be1c1d46b7b36c8`,
  28 findings, verdict fix-then-clear); review 17 covered **revision 2** (20 findings, fix-then-clear). Review
  session references: the review texts `review14.md` and `review17.md` in the analyst's scratchpad (Codex
  session ids were not supplied to the analyst; the lead holds them). This revision 3 answers both.
- Exposure: the analyst session read the decompilation of the VM, its natives, the level tick / main loop, the
  camera update, the sequence machinery, the element-table registration and the AI / actor helpers listed in
  section 10; the two reviewer sessions read the same subsystems (interpreter, wrappers, natives, tick, camera,
  sequence, actor admission) to verify the claims; no implementer session has read any of it.
- Publication approval: pending (maintainer); recorded separately from the factual review.
- Analyst authorisation: on behalf of the maintainer, on the maintainer's lawfully acquired copy.

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
low; confidence is separate from status). Every non-observed claim and every unsettled native maps to an
`Assumption` variant (section 4).

## 0. Necessity record

- Interoperability target: running the game's own compiled mission scripts (`.scb`, `docs/formats/scb.md`) and
  the mission, map and text data they address, so that the campaign plays as shipped.
- Information not otherwise available: the opcode arithmetic, the calling and native protocols, the semantics of
  the 265 natives, the callback scheduler and its clock, the sequence machinery. The data-only analysis of
  `docs/formats/scb.md` and the hypothesis engine (`crates/opensherwood-core/src/natives.rs`) left 95 natives as
  stubs and every scheduling rule as an assumption.
- Scope read: section 10.
- Stopping statement (honest scope). **Settled from the executable:** the instruction semantics; the calling
  convention; the native protocol; the arity and result convention of all 265 ids; the effect of 241 ids in full;
  the effect of 12 further ids whose *internal codes are unnamed* (45, 46, 47, 57, 59, 63, 136, 212, 254, 259, 260
  and the speak flag of 62 — section 4.3); the order of the level tick and its clock *as far as the executable
  decides it* (the realised frame length is host-dependent, VM-100); the sequence recording, launch, dispatch,
  completion and abortion rules; messages; mission variables; the end of a level. **Not settled:** 12 ids
  excluded from clearance (section 4.2); the actor-side execution of movement, animation, speech and action
  elements and the AI event meanings, which depend on the three sibling specifications of 1.1 — all still
  *drafts awaiting review* — and on their open items; the open questions of section 9. The scheduler and the
  sequence rules therefore cannot be cleared for implementation before those dependencies are reviewed.

## 1. Scope

The VM executes the per-class bytecode of a mission's `.scb` file: one interpreter instance per scripted element
(the level itself, actors, player characters, objects, scrolls, waypoints, script zones). The engine calls named
functions of a class ("callbacks") with a few integer parameters; the script calls the engine back through
numbered natives (0..264). Covered: the loader's use of the file, the instruction semantics, the calling
convention, the native call protocol, the scheduler, messages, the sequence machinery and every native id.

### 1.1 Dependencies on sibling specifications (none reviewed yet)

| Topic | Depends on | Status of the dependency (as of this revision) |
|---|---|---|
| Realised frame length of the script clock | `spec-movement-animation-camera.md` ANIM-002 (counter granularity), ANIM-521 (the two flags that disable the frame wait) | draft, awaiting review; clearance of VM-100 depends on its review |
| Walk elements (natives 45, 46, 47, 57, 63, 64, 70, 71, 212): the orders pushed, arrival, failure | `spec-navigation.md` NAV-130 (walk sequence), NAV-142 (asynchronous path search; an empty result fails the order after "100 clock units" — that spec has not yet established the unit), NAV-152 (arrival within radius + 5 px); `spec-movement-animation-camera.md` ANIM-208 (arrival) and its element table (3.4) | both drafts, awaiting review; NAV open question 3 (completion frame of a move-to-point action) open |
| Doors, buildings, patches behind natives 4, 8, 64, 98, 152, 156, 182 - 191 | `spec-navigation.md` section 5, NAV-172, NAV-190, NAV-191 | draft, awaiting review |
| Animation elements (49 / 50 / 51), speech (62 / 69), seek (57 / 70 / 71) | `spec-movement-animation-camera.md` element table 3.4 (rows 0xA4 / 0xA5 / 0xA6 / 0x92 / 0x15), ANIM-033, ANIM-131, ANIM-140 (speech end; inferred there) | draft, awaiting review |
| Camera natives 18 - 21, 33 - 35, 39, 40, 42 and the camera update | `spec-movement-animation-camera.md` ANIM-330 - ANIM-333 | draft, awaiting review; **contradiction**: its ANIM-331 says natives 18 / 19 "jump" the camera — the executable shows they write the scroll target, not the camera corner (row 18 / 19 and VM-219); that spec is being corrected by its analyst |
| AI events seen by `FilterAIEvent`, the pre-filter, alert / AI states behind 123 - 126, 128, 132 - 136, 140, 177, 218 - 220, 228 | `spec-ai-combat.md` AI-041 (state set), AI-081 (event set), AI-082 (pre-filter), AI-084 (head markers), AI-190 (callback ids and renumbering), its section 5.1 | draft, awaiting review; contradictions listed in 9.4 |
| Posture codes returned by 91 / set by 92 | `spec-ai-combat.md`: the posture factor inside AI-066 (a visibility rule that consumes the codes) and its open question 4 (the codes are unnamed there too) | **open**; natives 91 / 92 excluded (4.2) |
| Element admission by an actor (which elements a dead, absent, locked or busy actor refuses) and the actor's priority / merge rules | `spec-ai-combat.md` AI-082 (a) for the locked case; the actor's element runner (00467a50) otherwise | **open** — VM-216 |

## 2. Data model

### 2.1 Program (per class, loaded once per file)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-001 | The file is accepted when its 8-byte magic matches and its version float compares *equal* to 1.5 under the machine's floating-point equality (exactly 1.5 is accepted; a NaN version is accepted too because the comparison is unordered, VM-068); otherwise an error is raised and nothing is loaded. | observed | 0063b2d0 | high |
| VM-002 | Operand composition: for the jump (0x0E), call (0x05) and native-call (0x0C) instructions the target / id is the 32-bit value `a \| (b << 16)` formed from the file's two 16-bit operands; for the conditional jumps (0x0F, 0x10) it is `c`; for three-operand arithmetic the third symbol is the low 16 bits of `c`. | observed | 00639370 and the handlers of 3.1 | high |
| VM-003 | At callback entry the interpreter uses the function's *name* (exact byte comparison, first match in table order), its *address* and its `size_of_volatile` field (a pre-allocation of the locals block, replaced by the prologue 0x03 — no effect when the two agree, as in all 8176 retail functions). `unknown_0..2` and `size_of_tempor` of the header are not read. | observed | 00639300, 0063a510, 0063b070, 0063a250, 00634d30 | high |
| VM-004 | The class-variable block is allocated zero-filled with `size_of_variables` bytes when the class is bound to an element (once per instance). Cells are 4 bytes; ints and floats share them. | observed | 00639280 | high |
| VM-005 | The level class is the one named `StartUp` (compatibility token); a level without it is a fatal error at load. | observed | 004c0510 | high |

### 2.2 Interpreter instance (one per scripted element)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-010 | Instance state: program counter; a stack of frames; the class-variable block; a shared global block (storage class `00`, one per program, never addressed by the retail scripts); the *current parameter buffer* (growable); a native argument buffer of 12 cells (48 bytes) with a fill count; a **native result register**; a **callback return register** (0 at construction, **never reset**). | observed | 006390b0, 00639960, 00634c30 | high |
| VM-011 | Symbol operands are `u16`: bits 15..14 select the storage class (`00` global, `01` class, `10` frame locals, `11` frame temporaries), bits 13..0 a byte offset. No bounds checks. | observed | 00634c30 (bytes) | high |
| VM-012 | A frame holds: the return program counter; a 4-byte *result slot* (uninitialised at creation); the caller's parameter buffer; a locals block and a temporaries block (allocated zero-filled by 0x03; re-executing 0x03 frees and re-allocates them). | observed | 0063a250, 00634d30 | high |
| VM-013 | Creating a frame (0x05 and callback entry) saves the current parameter buffer into the new frame and installs a fresh empty one; popping (0x06 / 0x07) frees the saved buffer and the frame's blocks; the current buffer stays the callee's. | observed | 0063a250, 0063a320, 0063a3b0 | high |
| VM-014 | **Snapshot set** of an instance at a callback boundary (callbacks never yield, so frames and `pc` are empty there): the class-variable block; the callback return register; the native result register; the **argument buffer contents and fill count** (they persist across callbacks; the retail files leave them empty, but the state is not disposable in general); the global block. Restoring this set at a boundary reproduces the following callbacks exactly (acceptance case 17). | inferred | 006390b0, 00635210, 00635240 | high |

### 2.3 Mission variables (natives 0, 1, 2)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-020 | One growable signed 32-bit array per level, **empty at level start** (every index is invalid until native 0 declared it). Native 0 with `k` beyond the size grows the array to `k + 16` entries (new entries 0) and stores; inside, it stores. Native 1 stores only if `0 <= k < size`, else an error and no effect. Native 2 returns the value, or `-1` for `k` outside `0 <= k < size`. Native 0 with a negative `k` is an unchecked write. Snapshot: the whole array. | observed | 004e3d20, 004e3e30, 004e3e00, 004e3e20 | high |

### 2.4 Handles and element categories

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-030 | Elements, doors, patches, buildings, paths, locations and sound sources are opaque handles, `0` = none. The **element table** is the list of elements registered during the mission load in registration order (map elements and mission records: actors, objects, items, scrolls, waypoints — the observed order is in `docs/formats/scb.md`, "Index spaces"), followed by **one slot per campaign character id**, appended in the post-load step (each slot holds the player character with that id, or null when that character is absent). The script zones are created and bound *before* that post-load step; whether and where they occupy table entries is neither observable from the data nor read (9.3). A separate **cart-family table** (carts and their kin) is reachable through natives 3 and 10 (VM-030b). "Is a known element" in section 6 means: found in the element table by handle (linear search); player characters *are* found. | observed | 004c2720, 00570850, 004d8460, 005709c0 | high |
| VM-030b | Let `N` be the element table's size and `N16 = N mod 65536`, `C` the cart table's size. Native 3 on index `i`: `i = -1` → null without error; `i < N16` → the table entry (null for an empty slot); `N16 <= i < N16 + C` → cart `i - N16`; otherwise error, null; `i < -1` is not rejected (unchecked read before the table). Native 10 on a handle: its index in the table; if absent, its index in the cart table **counted from 0**; if in neither, −1. Native 75 returns the **full** `N`. | observed | 00571590, 00579bd0, 005714d0 | high |
| VM-031 | Element *categories* used by the natives (defined by behaviour): **actor family** — anything that moves under its own orders: humans, animals; **human** — a player character (PC) or a non-player human (NPC); **NPC** — a human that is not a PC; **soldier** — an NPC of the soldier class; **civilian** — an NPC of the civilian class; **animal** — an actor that is not human; **cart family** — carts; **animated map element**; **target family** — objects, scrolls and pick-up items; **target object** — an object that can be used with tools; **scroll**; **bonus item**. Which record kinds fall into which category is fixed by the mission and map formats (`docs/formats/rhm.md`, `rhp.md`); the natives only test membership. | observed | 00570a70 - 00570cb0, 0057b080, 005798f0 | high |
| VM-032 | A *location* is a point (native 6 for a point entry, 95, 213) or a *zone* (a script polygon / sector, native 6). "Requires a point / zone" in section 6 is a check on that kind. | observed | 005714f0, 00571de0, 00571b80 | high |

## 3. Behaviour

### 3.1 Instruction semantics

Notation: `S(x)` = the 4-byte cell named by symbol operand `x`; `a`, `b` = the two `u16` operands, `c` = the
`u32` operand, `c16` = its low 16 bits as a symbol; `pc` = program counter; `int` = 32-bit two's complement;
`float` = IEEE-754 single. Integer arithmetic wraps; integer comparisons are *signed*. Floating-point
environment: the x87 with invalid-operation exceptions masked (the process default); a comparison with a NaN
operand is *unordered* and yields the exceptional outcomes of VM-068. Unless stated, `pc` advances by 1.

| Op | Semantics | Id | Address | Conf. |
|---|---|---|---|---|
| 0x00 | Error (reported), then `pc := -1` and the loop continues with an **unchecked instruction fetch** before the first instruction (what follows depends on that memory). Never emitted. | VM-040 | 00634bf0 | high |
| 0x01 | No operation. | VM-041 | 00634c20 | high |
| 0x02 | Append the 4 bytes of `S(a)` to the current parameter buffer. | VM-042 | 00634c30 | high |
| 0x03 | Prologue: allocate the locals block of `a` bytes and the temporaries block of `b` bytes, zero-filled (previous blocks freed). | VM-043 | 00634d30 | high |
| 0x04 | Error (reported), then `pc += 1`. Never reached by the retail files. | VM-044 | 00634d80 | high |
| 0x05 | Call: push a frame with return `pc + 1` (VM-013), `pc := a \| (b << 16)`. | VM-045 | 00634db0 | high |
| 0x06 | Return: pop the frame; `pc :=` its return pc; `-1` ends the callback. | VM-046 | 00634dd0, 00639370 | high |
| 0x07 | Return with value: `v := S(a)`; the callback return register `:= v`; if the frame depth is > 1, also the *caller frame's* result slot `:= v`; then as 0x06. **Control does not continue.** | VM-047 | 00634de0 (bytes) | high |
| 0x08 | `S(a) :=` the 4 bytes at offset `c` of the frame's saved parameter buffer (parameter `k` at `4k`). No bounds check. | VM-048 | 00634eb0, 0063a3b0 | high |
| 0x09 | The inverse write. Never emitted. | VM-049 | 00634fa0 | high |
| 0x0A | `S(a) :=` the current frame's result slot: the value stored by the most recent 0x07 executed at a depth one greater than this frame's (a callee of this frame, or a nested callback running on top of it — VM-095); it persists until overwritten; uninitialised if none. | VM-050 | 00635090, 00634de0 | high |
| 0x0B | Append `S(a)` to the native argument buffer at index `count`, `count += 1`. No bounds check (12 cells; the retail maximum before one call is 6). | VM-051 | 00635150 | high |
| 0x0C | Native call: `id := a \| (b << 16)`; call table entry `id` (no range check; 265 entries) with the buffer; the wrapper pops `arity` cells (push order) and the **native result register := the wrapper's result** (3.3). | VM-052 | 00635210 | high |
| 0x0D | `S(a) :=` the native result register. | VM-053 | 00635240 | high |
| 0x0E | `pc := a \| (b << 16)`. | VM-054 | 00635320 | high |
| 0x0F | If `S(a) != 0` (32-bit word; `-0.0f` is true) then `pc := c` else `pc += 1`. | VM-055 | 00635330 | high |
| 0x10 | If `S(a) == 0` then `pc := c` else `pc += 1`. Never emitted. | VM-056 | 006353f0 | high |
| 0x11, 0x12 | `S(a) := S(b)` (identical opcodes). | VM-057 | 006354b0, 00635630 | high |
| 0x13, 0x14 | `S(a) := c` (identical). | VM-058 | 006357b0, 006358a0 | high |
| 0x15 | `S(a) := -S(b)` (int; `INT_MIN` stays). | VM-059 | 00635990 | high |
| 0x16 | float sign flip. Never emitted. | VM-060 | 00635b20 | high |
| 0x17 | float → int: truncation toward zero to a **64-bit** integer, **low 32 bits** kept (4294967296.0 → 0; 2147483648.0 → 0x80000000; −1.5 → −1); outside the 64-bit range or NaN → the 64-bit "indefinite", low word 0. Never emitted. | VM-061 | 00635cb0 (helper 00642b7c) | high |
| 0x18 | int → float (round to nearest even). | VM-062 | 00635e50 | high |
| 0x19 / 0x1A / 0x1B | int `S(b) + S(c16)`, `-`, `*` (wrapping). | VM-063 | 00635fd0, 00636210, 00636450 | high |
| 0x1C | int `S(b) / S(c16)` (signed, truncating); by zero and `INT_MIN / -1` are machine traps. Never emitted. | VM-064 | 00636680 | high |
| 0x1D / 0x1E / 0x1F | bitwise `\|`, `&`, `^`. | VM-065 | 006368c0, 00636b00, 00636d40 | high |
| 0x20 / 0x21 / 0x22 / 0x23 | float `+`, `-`, `*`, `/` (extended intermediate, stored as single). | VM-066 | 00636f80, 006371b0, 006373e0, 00637610 | high |
| 0x24 … 0x29 | int compare → `1` / `0`: `<=`, `<`, `>=`, `>`, `!=`, `==` (signed). | VM-067 | 00637840, 00637a40, 00637c40, 00637e40, 00638040, 00638240 | high |
| 0x2A … 0x2F | float compare → **float** `1.0f` / `0.0f`: `<=`, `<`, `>=`, `>`, `!=`, `==`. **Unordered** (a NaN): `<=`, `<`, `==` yield `1.0f`; `>=`, `>`, `!=` yield `0.0f`. | VM-068 | 00638440, 00638650, 00638860, 00638a70, 00638c80, 00638e90 | high |
| ≥ 0x30 | Error (reported); `pc` unchanged: the original loops forever. | VM-069 | 00639370 | high |

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-070 | The two retail `0x0E` with `a = b = 0xFFFF` (H10, two zone classes, `EnterZone`, after native 202) jump to `pc = 0xFFFFFFFF`: an unchecked fetch before the instruction array. The script's evident intent is to end the callback (its other paths end with return value 1). Implementation choice 8.1. | observed (fact) / inferred (intent) | 00639370; data | high / medium |
| VM-071 | The result slot read by 0x0A is undefined unless a 0x07 wrote it (VM-050); in all 62 retail uses the 0x0A directly follows a 0x05 whose target returns a value. | inferred | 00634d30, 0063a250; data | high |
| VM-072 | A 0x07 at depth 1 writes only the callback return register, read by the engine after the run (VM-090). A callback ending with 0x06 leaves the register unchanged (a value from an earlier callback of the instance — including a nested one, VM-095 — or 0). All 39 `Finalize` and `PostInitialize`, 1199 / 1448 `Initialize`, 235 / 274 `Hourglass`, 376 / 376 `ProcessMessage`, 68 / 312 `EnterZone` end this way; the engine reads none of their results. | observed | 00634de0, 00639360; data | high |

### 3.2 Calling convention (script to script)

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-080 | The caller pushes arguments with 0x02 (4 bytes each, in order), then 0x05; the callee reads argument `k` with 0x08 at `4k`; no count check. | observed | 00634c30, 0063a250, 0063a3b0 | high |
| VM-081 | A value is returned with 0x07 and read with 0x0A; the slot keeps the value until the next write (VM-050). | observed | 00634de0, 00635090 | high |
| VM-082 | Frames nest without a limit other than memory; recursion is allowed; locals / temporaries are fresh per entry. | observed | 0063a250 | high |

### 3.3 Native call protocol

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-085 | The native table has 265 entries, ids 0..264, all populated, built once when the first instance is created. | observed | 004075c0 (bytes) | high |
| VM-086 | Each entry is a wrapper that (1) takes the top `arity` cells (the last pushed is the last argument), decrements the fill count by `arity`, (2) calls the native with the arguments in push order, (3) yields one of three conventions (per id in section 6): **void** — 0; **bool** — the low 8 bits of the native's result (rows say what that byte is when the native's body produces no value of its own); **int / handle** — the full 32-bit value. Bool-converted arguments (non-zero → 1): 22, 26 (second), 36, 37 (third), 38 (second), 102 (third), 107, 115 (third), 130 (third), 131 (third), 134 (second), 138 (second), 139, 143 (second), 157 (second), 177 (second), 180 (second), 186 - 189 (second), 190 (third), 191 (**first**), 226, 244 (second), 254 (second), 257 (second), 264 (third). | observed | wrappers 00404a80 - 004075bf (bytes) | high |
| VM-087 | Arity-0 natives (23, 29, 30, 31, 32, 40, 54, 55, 74, 75, 106, 111, 119, 120, 121, 122, 147, 148, 159, 163, 167, 170, 171, 172, 173, 174, 211, 216, 234, 236, 238, 239, 245, 249, 251, 261) take no cell; pushed cells for them stay in the buffer; a persistent imbalance overflows the 12-cell buffer (unchecked write). The retail files are balanced for every id. | observed | wrappers | high |
| VM-088 | The argument buffer and its fill count are per instance and persist across callbacks; nested callbacks (VM-095) push and pop symmetrically. The retail files leave the buffer empty at every callback end; a program that does not must have the cells snapshotted (VM-014). | inferred | 006390b0, 00635210 | high |
| VM-089 | **Failure classes** of native calls (each row of section 6 names its class): **(E)** reported to the log, the row's failure value returned, the script continues; **(E+)** reported but the operation *still proceeds* wholly or partly (rows say which part); **(U)** unchecked memory access; **(T)** an arithmetic trap (161 with `n = 0`; opcode 0x1C); **(X)** a C++ exception thrown by a bounds-checked container (native 168: its index is compared as *unsigned* with the list size, so negative indices also take this path; the exception unwinds the native — not the engine's fatal-error routine); **(F)** the engine's fatal-error routine (two scroll callbacks overlapping, VM-094); **(C)** a null dereference crash (a callback name that the class lacks, VM-090); **(N)** non-termination (opcodes ≥ 0x30). Opcode 0x00's continuation is an unchecked fetch (VM-040), not necessarily a hang. Implementation choice 8.1 turns U / T / X / F / C / N into deterministic faults. | observed | 00571590, 00571760, 00570e20, 00578680, 00579350, 0057c710, 005f8030, 00639300, 00639370, 00634bf0 | high |

### 3.4 Running a callback

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-090 | To run callback `F` with parameters `p0..pn-1` on an instance: push them (4 bytes each) into the current parameter buffer; look the function up by name (VM-003); **a missing function is a null dereference in the original** (class C), except the level's `PostInitialize`, whose presence is tested first; push a frame with return pc `-1` (capturing the buffer); `pc :=` the address; run until that frame is popped. The engine then reads the callback return register when it wants a result. | observed | 00639300, 006392e0, 00639360, 00639340 | high |
| VM-091 | Callback parameters (compatibility tokens, see the preamble): `Initialize` on the level: one parameter, 0; `Initialize` elsewhere: none; `PostInitialize`: none; `Hourglass` on the level: `T / 25` (3.5); `Hourglass` on a scroll: 0; `CheckVictoryCondition`: `T / 25`; `Finalize`: **0 = success, 1 = failure** (VM-103b); `ProcessMessage`: `(message, arg1, arg2)`; `ActionChange` (actor classes): `(current_action, previous_action)`, 283 = none; `FilterAIEvent`: `(actor_or_0, event)` (VM-108); the nine `ActivatedBy…`: `(actor)` = the acting actor; `IsTaken`, `ReachPoint`, `EnterZone`, `ExitZone`: `(actor)`; `HandleEvent`: never called. | observed | 00404080, 004030a0, 00404570, 00404180, 00404960, 00404280, 00404480, 004033b0, 00403190, 004032a0, 004035d0 - 00403ed0, 00404860, 00408750, 004089f0, 00408af0 | high |
| VM-092 | Results read by the engine: `CheckVictoryCondition` — 1 = declare victory, 2 = **request the end bookkeeping** (whose outcome is success if a victory was declared before, failure otherwise: VM-103b), anything else = continue; `FilterAIEvent` from the general AI dispatcher — 0 drops the event (the second dispatcher ignores it, VM-108); `IsTaken` — non-zero = taken; `Initialize` of the level — read then ignored. Ignored: the nine `ActivatedBy*`, `Hourglass`, `ProcessMessage`, `ActionChange`, `Finalize`, `EnterZone`, `ExitZone`, `ReachPoint`. | observed | 004c6ef0, 00410620, 0040dcb0, 004ba4e0, 004bae60, 004c3740 | high |
| VM-093 | **Current actor** (native 74): a global handle with dynamic scope: set to the actor whose `ProcessMessage`, `FilterAIEvent` (both dispatchers) or `ActionChange` runs, to the entering / leaving actor for `EnterZone` / `ExitZone`, and to **the object itself** for the nine `ActivatedBy*`; each restores the previous value on return. A message to the level does **not** change it. `Initialize`, `PostInitialize`, `Hourglass`, `CheckVictoryCondition`, `IsTaken`, `ReachPoint` observe the enclosing value (null at level start). | observed | 00467230, 00410620, 0040dcb0, 00464230, 0057fcc0, 0057fdb0, 004bc1d0 (bytes), 004ca410, 00578050 | high |
| VM-094 | **Current scroll** (native 192): set to the scroll for the duration of a scroll class's `Initialize`, `Hourglass` and `IsTaken`, cleared to null afterwards; setting it while set is class F. Any callback nested inside a scroll callback observes that scroll; every callback outside one observes null. | observed | 004ba760, 004ba5c0, 004b9fa0, 004b9f40, 004ba4e0, 005798c0 | high |
| VM-095 | **Nested callbacks** (natives 109 / 110, 153 / 154, and message elements dispatched at a level start) run synchronously inside the native call on the target's instance. On the **same** instance: the inner frame is pushed on top of the outer one; the native-call instruction restores its own `pc` from a local copy and then overwrites the **native result register** with the wrapper's result, so the outer callback continues correctly and its next 0x0D reads the right value. Two effects **are** observable: (1) the **callback return register is not restored** — an inner callback that executed 0x07 leaves its value there, and an outer callback that then ends with 0x06 returns the inner value to the engine; (2) the inner callback's own 0x07 runs at depth ≥ 2, so it also **writes the outer frame's result slot**, which a later 0x0A of the outer frame would read (no retail file does: 0x0A always directly follows 0x05). Outer 0x02 pushes pending across the native would be captured by the inner run (none in the retail files). | observed | 00635210, 00634de0, 00635090, 00578d80, 0058a3d0, 00582560, 00585b70, 00467230, 004ca410 | high |

### 3.5 The scheduler

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-100 | **Clock — three layers.** *(a) Executable:* the level advances at most once per main-loop iteration ("level tick"); in the active-window branch, when the game is not paused and the level's presentation flag is clear (ANIM-521 names the two flags), the loop busy-waits on the millisecond counter until at least **40 ms** (400 ms in a debug slow-motion mode) have elapsed since the iteration began; there is no catch-up; the wait is skipped when either flag is set. *(b) Host-dependent inference:* the millisecond counter advances in the host's timer granularity; the movement spec's ANIM-002 concludes that on the measured host (15.625 ms granularity) the realised normal iteration is **46.875 ms** (three counter steps ≈ 21.33 iterations/s), which the oracle recordings confirm; so 25 executed ticks span **1.171875 s** and 75 span **3.515625 s** on that host, absent skipped or delayed iterations; on another host the realised length is the smallest multiple of the granularity ≥ 40 ms. *(c) OpenSherwood:* the fixed timestep is an implementation choice (8.1: 46.875 ms, following the movement spec's decision); the program has no separate 64 Hz clock. | observed (a) / inferred (b) | 0050f710 (a); ANIM-002 (b) | high (a) / medium (b) |
| VM-101 | The tick is **attempted** every iteration and skipped when: the game is paused; a modal window object is open; the game state is one of the two "leaving the level" states. Text pages and dialogs are not this window: they are synchronous loops inside the native / element that shows them (VM-218). While the tick is skipped the *camera update* still runs on every unpaused iteration (VM-219). | observed | 0050f710, 005105d0, 004c8380 | high |
| VM-102 | **Tick counter** `T`: 0 at level start; incremented at **step 5** of every executed tick (VM-103); never reset. Snapshot: `T`, the force flag, the won flag and its sub-flag, the success-end / abort / failure-end flags, the debriefing index. | observed | 004c6ef0 | high |
| VM-103 | **Order within one executed level tick** (script-relevant steps; "ends" = the tick returns and later steps are skipped): **(1)** if the victory sub-flag is set and no player character has left the map yet: clear the sub-flag and show the "you may leave" notice (before any terminal check). **(2)** if the *success-end* flag is set: level `Finalize(0)` (scripts enabled), the tick ends with code 2 and the level ends; else if the *abort* flag is set: end bookkeeping (VM-103b), ends with code 1; else if the *failure-end* flag is set: `Finalize(1)`, ends with code 3. **(3)** pending one-shot HUD updates. **(4)** if `T mod 25 == 0`: level `Hourglass(T / 25)`; then, if `(T / 25) mod 3 == 0` **or the force flag is set at that moment** (a native 29 called inside this `Hourglass` counts): clear the force flag, run `CheckVictoryCondition(T / 25)`; result 1 with the won flag clear → **declare victory** (won flag := 1; sub-flag := 1 unless the campaign node is of type 3 or 6, in which case an interface-mode switch not read here follows, 9.3; no immediate end); result 2 → end bookkeeping (VM-103b) and **the tick ends** (code 1) without incrementing `T`. **(5)** `T += 1`. **(6)** if any of three *transition flags* (set by screens that suspend play; not read, 9.3) is set → ends (code 0). **(7)** unless the debug "no defeat" toggle is set — **defeat checks**, each running the end bookkeeping and ending the tick (code 1): no player character that is present and has not left → end; a captured player character → end; a dead civilian without the "may die" mark → end. **(8)** per-element updates in element-table order (actors dispatch `ActionChange`, scrolls count toward their `Hourglass`, movement, AI …). **(9)** queue drain (VM-215). **(10)** timer pass (VM-221). | observed | 004c6ef0 | high |
| VM-103b | **End bookkeeping**: computes the end statistics into campaign counters, then: won flag set → success-end flag := 1 (next tick: `Finalize(0)`, code 2); else → failure-end flag := 1 (`Finalize(1)`, code 3). The end triggers are therefore: a `CheckVictoryCondition` result of 2, the abort flag, and the three defeat checks — each ending in success **iff** a victory had been declared before it fired (by result 1 or by native 178). A declared victory by itself ends nothing: the mission ends when one of those triggers fires (typically the first defeat check, once every player character has died or left the map). | observed | 004e3260, 004e3220, 004e3cd0, 004c6ef0, 005795c0 | high |
| VM-104 | **Initialisation order.** While the mission file is read, every scripted actor, object, waypoint and script zone binds its class and runs `Initialize` at once, in record order (the retail ones are stubs). Player characters run `Initialize` when created. Just before the play loop: the level's `Initialize(0)`, then every scroll's `Initialize` in scroll-table order (current scroll set). The level's `PostInitialize` runs once on the first loop iteration, after the first *attempted* tick, if the class has it. | observed | 0046e7e0, 004bb820, 00551bb0, 0057f8c0, 004a0f10, 004c3740, 004ba000, 004b9fa0, 0050f710 | high |
| VM-105 | **Scroll `Hourglass`**: each active scripted scroll counts its own updates; at 25 it runs `Hourglass(0)` and resets. | observed | 004b9f40 | high |
| VM-106 | **Scroll `IsTaken`**: run by the "take scroll" element (pickup order) with the taking actor; status set to 3 and a sound played before; non-zero → status 2 (taken, hidden); zero → stays at 3. | observed | 004ba4e0, 004ca410 | high |
| VM-107 | **`ActionChange`**: in an actor's update, when its current action id (283 = none) differs from the stored one, `ActionChange(current, previous)` runs and the stored id is updated. Target-family objects have a dispatcher of the same shape (not read). | observed | 00464230 | high (actors) / medium (objects) |
| VM-108 | **`FilterAIEvent(actor_or_0, event)`** has two sources. (a) The general AI dispatcher, before the pre-filter (AI-082): `event` = the renumbered id of AI-190; the first parameter is the involved actor for payload kind 3, else 0; a zero result **drops the event**. (b) A second dispatcher for seven events **100..106** raised by the actor's own order machinery: before the callback it sets the NPC's alert state (100 - 102 → 0; 103 and 106 → 1); the first parameter is **the NPC receiving the callback** (also installed as current actor) for 100..103 and another element stored with the NPC (unidentified) for 104..106; the result is **ignored**; afterwards the event and its payload are stored in the NPC. The meaning of 100..106 is open (`AiEventCode`, 4.1). | observed | 00410620, 0040dcb0 | high (a) / medium (b) |
| VM-109 | **`ReachPoint(actor)`**: run when a path follower reaches a waypoint carrying a script reference, and when an actor reaches the end of a patrol node flagged for scripting (AI-091). | observed | 004105d0, 004abfe0 | medium |
| VM-110 | **`EnterZone` / `ExitZone`**: run from the zone's membership dispatchers, which add / remove the membership themselves (a duplicate entry or a leave without membership is reported and still dispatched). Natives 153 / 154 call the same dispatchers only for an actor **already a member** (VM-120b). | observed | 0057fcc0, 0057fdb0, 00577220, 00577390 | high |
| VM-111 | **`ActivatedBy*`**: run when the player's "use tool on object" element executes on a scripted object (tools in order: apple, arrow, hand, heal, lever, money, search, stone, sword), with the acting actor as parameter and the object as current actor; `ActivatedByListenable` from the object's own update when its "listenable" flag is set; results ignored. | observed | 004bc1d0 (bytes), 004bae60, 004bc3d0 | high |
| VM-112 | Every callback is gated by the global "scripts enabled" flag. | observed | 004c6ef0 and the invokers | high |
| VM-113 | **Engine-originated message** `(1001, 0, 0)` to the **level**, hub level only (the current campaign node is of type 8), sent through the send-now path: (a) right before the play loop starts, unless the loop is entered from the mission-selection state; (b) when the loop returns from the mission-selection state **and** the selection was accepted (not cancelled) **and** the selected mission is the current node (a different mission leaves the level instead; a cancelled selection resumes play without a message). No other message is originated by the engine. | observed | 0050f710 (0050f8bb, 0050fd58), 0050b640, 00578d80 | high |

### 3.6 Messages

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-120 | A message is `(message, arg1, arg2)`; the target is an actor-family element or **null** = the level class; any other target → error, dropped (E). No queue: delivery is a sequence element executed synchronously when its level is dispatched (VM-095). Natives 43 / 44 *record* it (error and no delivery outside a recording); 109 / 110 wrap it in a one-element sequence and deliver it inside the native call. | observed | 00578cb0, 00578e50, 00578d80, 00578f20, 004ca410, 00467230 | high |
| VM-120b | Natives 154 / 153 fire `EnterZone` / `ExitZone` only for an actor of the element table that is already a member of the zone; an absent actor → error, nothing. | observed | 00577390, 00577220 | high |
| VM-121 | 44 / 110 pass `(message, a, b)`; 43 / 109 pass `(message, 0, 0)`. No delay parameter. | observed | 00578e50, 00578cb0 | high |
| VM-122 | Message ids are opaque integers; the engine originates only message 1001 (VM-113); native 71 requires `message >= 1000`. | observed | 00573f00 | high |

### 3.7 Sequences

A *sequence* is an ordered list of *elements*, each tagged with a *level*; it runs level by level: all elements
of a level are dispatched together and the next level starts when every element of the current level has
finished. Recording (natives 30 / 31 / 32 and the recording natives) builds such a list.

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-200 | **Recording state** (global, one recording at a time): the sequence being recorded (null = none), the *current level* (a 16-bit counter, 0 = not recording), and two "entered actors" lists (natives 46 / 47 / 63). Native 30: a recording already open → error, returns 0, no change; else: new empty sequence, level := 1, lists cleared, returns 1. **The recording state persists across callbacks**: native 30 returns with the recording open, and nothing closes it until native 31 (the retail files always close it within the same function, `docs/formats/scb.md`). Snapshot: the recording state belongs to the snapshot set (8.3). | observed | 00570f00 | high |
| VM-201 | Native 32: not recording → error, 0. Else, if the sequence has an element **and the last recorded element's level equals the current level**, level += 1; returns the level. Two consecutive barriers count once. **16-bit wrap**: after 65535 increments the counter becomes 0 while the recording stays open: 32 returns 0, and 31 / 32 / every recording native then behave as "not recording" (errors, nothing recorded) while 30 still refuses because a recording is open — the recording can no longer be closed. Reachable only by a script that loops over recorded elements and barriers; implementation choice 8.1 faults at the wrap. | observed | 00571100, 00570f00, 00571020, 0057a020 | high |
| VM-202 | Native 31: not recording → error, 0. Else: level := 0; the entered lists cleared; an empty sequence → error, discarded, 1; else handed to the manager and **launched at once** (VM-210), 1. | observed | 00571020 | high |
| VM-203 | A recording native appends one element tagged with the current level. Outside a recording the element is discarded with an error — but whatever the native did *before* its recording step is not undone: natives 46 / 47 place the actor and register it in the entered lists first (E+, rows 46 / 47), native 63 registers the taker first (E+); every other recording native has no effect before the recording step. | observed | 0057a020, 00577650, 005779a0, 00574a70 | high |
| VM-204 | An element carries the effect requested by its native (its category: camera, timer, message, page, walk, animation, …), its target and the native's arguments; the category decides who executes it and when it completes (VM-212, VM-218, VM-231). | observed | the natives of section 6 | high |
| VM-210 | **Launch**: the sequence is appended to the manager's list and its first level is *dispatched* (VM-212): immediate categories execute now, in element order; the others become *pending* in the FIFO queue. A pending element becomes *running* only when its executor admits it. Natives 102, 109, 110 and the engine (pickups, AI) launch one-element sequences the same way. | observed | 0058a3d0, 00582530, 00582560, 0058a940 | high |
| VM-211 | **Level completion**: a sequence counts the elements of its current level not yet *done*; an element reaching *done* decrements it; at zero the next level is dispatched **synchronously at that point**. After the last level nothing happens; housekeeping (every 256 ticks) deletes a sequence with no running or pending element. | observed | 00582620, 00582560, 005823f0, 0058a430, 004c6ef0 | high |
| VM-212 | **Dispatch of a level**: its elements are visited in order; a *cancelled* element is skipped; each other element is (a) executed at once by the level executor if its category is level-immediate — lock / unlock player input (54 / 55), camera jump (34), timer (56), PC action availability (37), character availability (38), take-scroll; (b) a message (43 / 44): executed at once by the target actor's message path, or by the level executor for a null target; (c) handed at once to the target actor if actor-immediate — lock / unlock AI (52 / 53), un-blip (243), animation table swap (60 / 61), mobile-element start / stop / activate / deactivate (67 / 68 / 72 / 73), speak (62 / 69), an engine-internal movement variant; (d) otherwise appended to the manager's FIFO queue as *pending* (camera scroll 33 / 42, zoom 35, map 36, dialog 41, camera lock 39 / 40, page 203, freeze-all 226, walks, seeks, turns, animations 49 - 51, actions 59, damage 102, corpse 63 / 65). | observed | 00582560, 0058bb80, 00585b70 | high |
| VM-213 | **Re-entrancy during dispatch**: a callback run by an immediate element (b) may launch sequences, complete elements or cancel elements. Ordering: the nested launch dispatches *its* first level completely (its immediate elements execute, its pending ones are queued) **before** the outer dispatch continues with the next sibling; queue entries keep launch order; a sibling cancelled by the callback before its turn is skipped (VM-212); a sibling already handed to an actor is unaffected until the actor processes it. Setting an element to the state it already has is a no-op (no second completion, no propagation). | observed | 00582560, 00585320, 0058a3d0 | high |
| VM-215 | **Queue drain** (step 9): FIFO until empty; each element removed and, if still pending, given to its target — an actor target's admission (VM-216), a null target → the level executor (VM-218). Elements appended during the drain (callbacks, level completions, nested launches) are drained in the same pass, after the entries already queued. | observed | 0058ba60, 00585570, 004c6ef0 | high |
| VM-216 | **Actor admission**: the actor's admission tests decide (its own state and the element's parameters; the locked case is AI-082 (a); the other predicates are **open**, 9.1). Refused → *refused* with the "next level" propagation (VM-217). Accepted and idle → current action, *running*; accepted and busy → merged / queued by the actor's own rules (**open**). The actor reports *done* when the action ends (VM-231). | observed (structure) / unknown (predicates) | 004646e0, 0046b210 | medium |
| VM-217 | **Abort cascade**: a *transition* to *refused* or *cancelled* carries one of three propagation modes: **next-level** (used by the actor's refusal and cancellation): the first element of the next level is set to the same state in **chain** mode; **chain**: the following element of the sequence is set likewise, so every later element, whatever its level, ends in that state; **none**: no propagation. Same-level siblings of the refused element are not touched and finish normally. A refused / cancelled element never reports *done*, so its level never completes and the remainder is dropped (deleted by housekeeping). A *done* transition never propagates. | observed | 00585320, 004646e0 | high |
| VM-218 | **Level executor** (completion of its categories): lock / unlock player input — HUD events, *done* at once. Camera jump — the camera corner set (VM-219), *done* at once. **Camera scroll** (33 / 42) — becomes the *camera element*; *done* when the camera update finishes the scroll (VM-219), or at once when the level's "no cinematic camera" flag is set; a new camera element marks the previous one *done*. **Zoom** (35) — sets the zoom request, becomes the camera element; *done* when the zoom reaches the request (VM-219). Map display (36) — at once. **Timer** (56) — appended to the timer list (VM-221). **Dialog** (41) and **page** (203) — unless the "no presentation" flag is set, a **synchronous modal loop** shows it and returns only when the player dismisses it (the whole program is suspended inside: no ticks, no camera update, no other sequence); then *done*, then a HUD event. Camera lock / clear (39 / 40) — at once. Message — the level's `ProcessMessage`, at once. Freeze-all := bool (226) — at once. PC action availability (37), character availability (38) — HUD events, at once. Take-scroll → `IsTaken` — at once. Native 202 runs the same modal loop **inside the native call**; native 17 likewise for a dialog. | observed | 004ca410, 0053a1a0, 0053a0b0, 0053a4c0, 0052b100, 00579f30, 005714a0 | high |
| VM-219 | **Camera update** (once per unpaused main-loop iteration, after the tick step): a running scroll advances the camera corner toward the scroll destination by the **scroll step length** (map pixels per camera update; for a default-speed scroll the step is refreshed from the camera update's ramp, ANIM-330; an explicit speed from native 42 is used as is); when the corner reaches the destination (or the step is clipped at the map border) the step length is reset to 1.0, the actor-follow lock is cleared and the camera element (if any) is *done*; when the zoom equals the zoom request the request is cleared and the camera element is *done*. These completions, and the level completions they trigger, are an execution opportunity **outside the tick phases**. Natives 18 / 19 write the scroll target, the (clamped) scroll destination and the step length; 20 writes the camera corner directly (a jump) and invalidates the cached view; 21 writes the zoom request (rows in section 6). | observed | 004cdfc0, 004c8380, 005105d0, 00571270, 00571330, 005713f0, 00571200 | high |
| VM-221 | **Timer pass** (step 10): the timer list is visited in insertion order over the count captured at the start of the pass; an element whose counter equals 1 is *done* and removed; otherwise `counter -= 1` (32-bit, wrapping). A timer inserted earlier in the same tick is visited in that tick's pass; one inserted *during* the pass is first visited next tick. Timers completing in the same pass complete in list order, each running its level completion synchronously before the next is visited. A timer recorded with counter `n` completes on visited pass number `((n − 1) mod 2^32) + 1`: `n >= 1` → the `n`-th pass; `n = 0` → the 2^32-th pass; a negative `n` → `(n mod 2^32)`-th pass (e.g. −1 → the (2^32 − 1)-th). OpenSherwood counts identically (8.1: no departure). | observed | 004c6ef0, 00575350 | high |
| VM-222 | Inside a modal loop nothing advances; while the tick is skipped the camera update still completes camera elements (VM-219). | observed | 0050f710, 004cdfc0 | high |
| VM-231 | **Actor-side completion** (per the movement spec's element table, draft): actor-immediate categories (VM-212 c) are *done* as soon as the actor executes them; **speak** (62 / 69) when the speech ends (ANIM-140, inferred there); **walks** on arrival within radius + 5 px (NAV-152 / ANIM-208) or refusal on a path failure (NAV-142); **seek** (57 / 70 / 71) on arrival, or at once when the seeker is the target; **animation 49** completes after the clip; **animation 50 (loop) never completes** — its level never finishes and every later level is blocked; **animation 51** completes after the clip and then holds the actor on the clip's last frame (through a separate one-element sequence that itself never completes); **actions of native 59**: end of the action (**open**); corpse 63 / 65 (**open**); the admission and priority rules (**open**). | inferred (from the sibling drafts) | 00467230, 004646e0, 00464b20 | medium |

### 3.8 Objectives, debriefing, win and loss

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-240 | Native 26 adds objective `k` (bool `main`) to the objective display; 27 marks it accomplished (`k` indexes the level's short-briefing texts). | observed | 005785b0, 005785d0 | high |
| VM-241 | Native 28 stores `k` as the debriefing variant read when the level ends. | observed | 00570a30, 0050f710 | high |
| VM-242 | Native 29 sets the *force* flag, consumed at the next step-4 check: if 29 is called inside the level's `Hourglass`, that same tick's check runs (VM-103 step 4); otherwise the next Hourglass tick runs it. It does not run the check immediately. | observed | 00578c60, 004c6ef0 | high |
| VM-243 | `Finalize(0)` runs on the tick after the success-end flag was set, `Finalize(1)` after the failure-end flag (VM-103 step 2, VM-103b). | observed | 004c6ef0, 004e3260 | high |

## 4. Claims

The claim register is the union of the `VM-nnn` rows of sections 2 and 3 and the per-id rows of section 6; every
row names its address and failure class. Status values are only `observed`, `inferred`, `unknown`.

### 4.1 Assumption mapping (every claim not `observed`, and every settled-effect gap)

| Claim / native | Status | `Assumption` variant (ADR-0008) |
|---|---|---|
| VM-014, VM-088 (snapshot set and buffer persistence) | inferred | `SnapshotSet` (a design fact: the set is a superset of what any retail file needs) |
| VM-030 (position of script zones in the table) | unknown | `ElementTableOrder` |
| VM-070 (intent of the `-1` jump) | inferred | `UnresolvedJump` |
| VM-071 (result slot undefined when unwritten) | inferred | none needed (the retail files never read it unwritten) |
| VM-100 (b) realised frame length | inferred (host-dependent) | `FrameLength` (8.1) |
| VM-107 (objects' `ActionChange` dispatcher) | inferred | `ObjectActionChange` |
| VM-108 (b): meaning of events 100..106 and the stored element of 104..106 | unknown | `AiEventCode(100..106)` |
| VM-109 (patrol-node variant) | inferred | `ReachPointSource` |
| VM-216 admission / priority predicates | unknown | `ElementAdmission` |
| VM-231 durations of 59 actions, 63 / 65, speech detail | unknown / inferred | `ElementDuration(category)` |
| natives 45 / 212 variant flag, 46 / 47 / 63 entered-list details, 57 / 70 / 71 `v` parameter | inferred | `Policy(id)` |
| natives with settled effect and unnamed codes (4.3) | observed effect, unknown code names | `UnnamedCode(id)` (the code is passed through verbatim) |
| natives excluded from clearance (4.2) | unknown | `UnknownNative(id)` with the placeholder of 4.2 |

### 4.2 Natives excluded from clearance (12)

Called by the retail scripts: 13 (25 calls), 91 (1), 92 (1), 173 (14, result read), 224 (159, result unused),
241 (1), 261 (1); unused: 157, 190, 225, 227, 238. Their semantics are not settled; they map to
`UnknownNative(id)` and the implementation must make the placeholder an **explicit choice** recorded as an
assumption: 13 → the inverse of native 6 (the scripts' evident intent); 91 → 0; 173 → 0; 224 → 0 (the repulsive
point *is* created, 224's effect is observed, its result is not); 238 → 0; 261 → 0; 92, 157, 190, 225, 227, 241
→ no-op. A strict mode may instead raise a deterministic "unresolved operation" fault on any of them.

### 4.3 Natives with settled effect and unnamed internal codes (12)

45 and 212 (the variant flag of modes 2 / 3), 46 / 47 (entered-list details), 57 (`v`), 59 (the action codes:
the effect is "record the action with that code"; the actions' names are not established), 62 (the speak flag),
63 (details), 136 (the seventeen battle decisions in order: names not established), 254 (the flag it sets: not
identified), 259 / 260 (the eighteen action states 0..17: names not established). Implementable as pass-through
of the code; cleared only together with the sibling spec that names the codes (`UnnamedCode(id)`).

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| minimum iteration time requested (guarded wait) | 40 | ms | 0050f710 | high |
| slow-motion iteration time (debug, guarded) | 400 | ms | 0050f710 | high |
| realised normal iteration on the measured host | 46.875 (three 15.625 ms counter steps) | ms | ANIM-002 (movement spec, draft) | medium |
| Hourglass period | 25 | executed ticks (1.171875 s realised on the measured host) | 004c6ef0 | high |
| victory check period | 3 | hourglass counts (3.515625 s realised) | 004c6ef0 | high |
| sequence housekeeping period | 256 | executed ticks | 004c6ef0 | high |
| scroll Hourglass period | 25 | scroll updates | 004b9f40 | high |
| native table size | 265 | ids | 004075c0 | high |
| native argument buffer | 12 | cells | 006390b0 | high |
| file version accepted | 1.5 | float, machine equality | 0063b2d0 | high |
| mission variable growth | k + 16 | entries | 004e3d20 | high |
| missing action id | 283 | action id | 00464230, 00578060 | high |
| "no state" answer of 91 and 259 | 666 | code | 005762d0, 005766f0 | high |
| zoom requests accepted by 21 / 35 | 0.5, 1.0, 2.0 | zoom factor | 00571200, 00572d50 | high |
| scroll step length written by 18 | 2.0 (19: its float argument) | map px per camera update | 00571270, 00571330 | high |
| remark id limit (69, 264) | 120 (ids 0..119) | remark id | 00572ef0, 00573010 | high |
| door search radius of 64 | 300 (squared 90000) | map px | 00574860 | high |
| campaign value window of 195 / 196 | k in 0..19 → slots 7..26 | index | 00579430, 00579470 | high |
| NPC custom values (197 / 198) | k in 0..9 | index | 005794b0, 00579520 | high |
| scroll status range (194) | 0..3 | status | 00579950 | high |
| minimum custom message id (71) | 1000 | message id | 00573f00 | high |
| engine-originated message (hub level) | 1001 | message id | 0050f710 | high |
| team size limit with no mission selected (174) | 5 | characters | 00579300 | high |
| barrier level counter width | 16 | bits | 00571100 | high |

## 6. Interfaces to the script VM: natives by id

Columns: `Args` in push order (`b` = converted to bool by the wrapper); `→` = result convention (VM-086);
**records** = creates an element in the open recording (VM-203); **Fail** = failure class of VM-089 with the value
returned. Confidence high unless marked. Categories as in VM-031; "actor" = actor family unless narrowed.

| Id | Args | → | Behaviour | Fail | Address |
|---|---|---|---|---|---|
| 0 | `k, v` | void | Mission variable `k := v`, growing the array (VM-020). | U (negative k) | 00577090 |
| 1 | `k, v` | void | Mission variable `k := v`; `k` undeclared → E, no effect. | E | 005770b0 |
| 2 | `k` | int | Mission variable `k`; undeclared → −1. | E (−1) | 00577100 |
| 3 | `i` | handle | Element `i` (VM-030b). | E (null) / U (i < −1) | 00571590 |
| 4 | `i` | handle | Door `i` of the map's door list (NAV section 5); `-1` → null; out of range → null. | E (null) | 005716a0 |
| 5 | `i` | handle | Patch `i` of the map's patch list; same rules. | E (null) | 00571700 |
| 6 | `i` | handle | Location `i` of the level's location list (`i` modulo 65536; no upper check); `-1` → null; an empty entry → null. | E (null) / U | 005714f0 |
| 7 | `k` | handle | Sound source `k`; error and null when the level has sound and `k` does not exist; null silently without sound. | E (null) | 005755f0 |
| 8 | `i` | handle | Building `i` of the map's building list, **no check** (NAV section 5). | U | 00571760 |
| 9 | `i` | handle | Patrol path `i` (modulo 65536, no upper check); `-1` → null. | U | 00571780 |
| 10 | `e` | int | Index of `e` (VM-030b). | (−1) | 00579bd0 |
| 11 | `door` | int | Index in the door list or −1. | (−1) | 00579c90 |
| 12 | `patch` | int | Index in the patch list or −1. | (−1) | 00579d00 |
| 13 | `x` | int | **Excluded (4.2).** Index of `x` in the map's script-zone list, −1 if absent; whether this inverts native 6 for polygon locations is open (9.2). | (−1) | 00579d70 |
| 14 | `sound` | int | Index of the sound source; −1 with an error if unknown; −1 silently without sound. | E (−1) | 00579de0 |
| 15 | `building` | int | Index in the building list or −1. | (−1) | 00579e80 |
| 16 | `path` | int | Index of `path` in the patrol-path list (inverse of 9), 16-bit; 65535 if absent. | (65535) | 00579ef0 |
| 17 | `k` | void | Shows dialog page `k` **now** in the synchronous modal loop (VM-218). | – | 005714a0 |
| 18 | `loc` | bool | **Scroll request**: scroll target := point `loc`; scroll destination := that point clamped to the map; **scroll step length := 2.0 map px per camera update**; the camera update then scrolls there (VM-219; the step is reset to 1.0 at the end). Not an element; returns 1. Null → error, 0. | E (0) | 00571270 |
| 19 | `loc, f` | bool | As 18 with step length := `f`. | E (0) | 00571330 |
| 20 | `loc` | bool | **Jump**: camera corner := point `loc` clamped to the map (immediately), the cached view invalidated, the actor-follow lock cleared; the step length is unchanged; returns 1. Null → error, 0. | E (0) | 005713f0 |
| 21 | `f` | bool | **Zoom request** := `f` (only 0.5 / 1.0 / 2.0 accepted → 1; else error, 0); the visual change follows in later camera updates (VM-219). | E (0) | 00571200 |
| 22 | `b` | bool | Map display shown (`b`) / hidden; returns 1. | – | 005714b0 |
| 23 | – | void | A screen transition (HUD event with a fade). Unused. | – | 00576170 (medium) |
| 24 | `e, code` | void | Minimap dot of known element `e`: codes 0 / 1 default; 100 - 102, 200 - 202, 300 - 302 green / red / blue styles; 111 / 222 / 333 the same, humans only; any other code (the retail 444 included) → error, **no change**. | E | 005788a0 |
| 25 | `zone, f` | void | Force the "emergency box" of a motion-area zone (radius `f`); other → error. Unused. | E | 00578c70 |
| 26 | `k, b` | void | Add objective `k`, primary if `b`. | – | 005785b0 |
| 27 | `k` | void | Objective `k` accomplished. | – | 005785d0 |
| 28 | `k` | void | Debriefing variant := `k`. | – | 00570a30 |
| 29 | – | void | Force the next victory check (VM-242). | – | 00578c60 |
| 30 | – | bool | Begin recording → 1; already recording → error, 0 (VM-200). | E (0) | 00570f00 |
| 31 | – | bool | End recording and launch → 1; not recording → error, 0 (VM-202). | E (0) | 00571020 |
| 32 | – | int | Barrier (VM-201); not recording → error, 0. | E (0) | 00571100 |
| 33 | `loc` | bool | Records a camera scroll to point `loc` (default speed) → 1; not recording / null / not a point → error, 0. | E (0) | 00572ab0 |
| 34 | `loc` | bool | Records a camera jump to point `loc` → 1; same checks. | E (0) | 00572ba0 |
| 35 | `f` | bool | Records a zoom to `f` (0.5 / 1 / 2) → 1; else error, 0. | E (0) | 00572d50 |
| 36 | `b` | bool | Records map display := `b` → 1; not recording → error, 0. | E (0) | 00572e40 |
| 37 | `pc, k, b` | bool | Records "action `k` available := `b`" for player character `pc` → 1; `pc` not an actor → error, 0. | E (0) | 00577d30 |
| 38 | `pc, b` | bool | Records "character `pc` available := `b`" → 1; not an actor → error, 0. | E (0) | 00577e70 |
| 39 | `actor` | bool | Records "camera locks on `actor`" → 1; not an actor → error, 0. | E (0) | 00577f30 |
| 40 | – | bool | Records "clear the camera lock"; the returned byte is the recording step's result (1 recorded, 0 not recording). | E (0) | 00577fe0 |
| 41 | `k` | bool | Records "dialog page `k`" (modal when executed, VM-218); byte = the recording step's result. | E (0) | 00572a20 |
| 42 | `loc, speed` | bool | Records a camera scroll to point `loc` with step length `speed` → 1. | E (0) | 00572c70 |
| 43 | `target, msg` | void | Records message `(msg, 0, 0)` to `target` (null = level; non-actor → error, nothing). | E | 00578cb0 |
| 44 | `target, msg, a, b` | void | Records `(msg, a, b)`. | E | 00578e50 |
| 45 | `actor, loc, mode` | bool | Records a walk of `actor` to point `loc` (NAV-130): mode 0 / 2 walk, 1 / 3 run; modes 2 / 3 additionally mark the added elements with a variant whose effect is unnamed (4.3). Not recording / null or non-point location / non-actor / bad mode → error, 0. Completion: VM-231. | E (0) | 005740b0 |
| 46 | `actor, loc, dir, b` | bool | "Enter the game" — **immediate effects first**: if `actor` is not in the recording's entered lists it is placed at point `loc` facing `dir` (−1 = its own facing), stopped, and added to both lists; **then** a walk (walk if `b` = 0, run otherwise) is recorded. Outside a recording the placement still happens and only the walk is dropped (E+). Not an actor / not a point / bad `b` → error before any effect. **Returns 0 always** (the low byte is explicitly cleared). | E / E+ (0) | 00577650 |
| 47 | `actor, loc, dir, b` | bool | Variant of 46 (position taken from the actor when already registered; facing `(dir − 8) mod 16`); placement first, then the walk; → 1. | E / E+ (0) | 005779a0 |
| 48 | `actor, loc` | bool | Records "turn to face point `loc`" → 1. | E (0) | 00574e70 |
| 49 | `x, anim` | bool | Records "play animation `anim` once" on an actor or target object → 1; completes after the clip (VM-231). | E (0) | 00573190 |
| 50 | `x, anim` | bool | Records "loop animation `anim`" on any known element → 1; **never completes** (VM-231). | E (0) | 00573280 |
| 51 | `x, anim` | bool | Records "play `anim` and freeze on its last frame" (actor or target object) → 1; completes after the clip, the actor then holds the last frame (VM-231). | E (0) | 00573370 |
| 52 | `actor` | bool | Records "lock the AI of `actor`" (known actor) → 1 (AI section 5.1). | E (0) | 00575120 |
| 53 | `actor` | bool | Records "unlock the AI" → 1. | E (0) | 005751d0 |
| 54 / 55 | – | bool | Records "lock" / "unlock player input" → 1; not recording → 0. | E (0) | 005753d0, 00575470 |
| 56 | `n` | bool | Records a timer with counter `n` (VM-221); byte = the recording step's result. | E (0) | 00575350 |
| 57 | `actor, target, k, v` | bool | Records "seek actor `target`" (walk if `k` = 1 else run) with a parameter `v` (unnamed, 4.3); byte = recording result. | E (0) | 00573d10 |
| 58 | `x` | bool | Always 0 (placeholder). | – | 006390a0 |
| 59 | `actor, code, arg` | bool | Records an action of `actor` (known element) by `code` 0..20 (4.3): 1 = turn to direction `arg mod 16`; 4 = shoot at element index `arg` (must be a valid table index); 5 = begin a sword fight with element `arg`; 8..16 = nine sword figures against element `arg`; 17 / 18 soldier-only looks; 0, 2, 3, 6, 7, 19, 20 fixed actions without an argument. Other codes / invalid `arg` → error, 0; else 1. | E (0) | 00573450 |
| 60 | `actor, a, b` | bool | Records "replace animation `a` by `b`" → 1. | E (0) | 00574f80 |
| 61 | `actor, a` | bool | Records "restore animation `a`" → 1. | E (0) | 00575050 |
| 62 | `pc, text, flag` | bool | Records "speak text `text`" (flag unnamed, 4.3) for a **player character** → 1; non-PC → error, 0. | E (0) | 005730b0 |
| 63 | `actor, corpse, b` | bool | Records the "take corpse" walk-and-take: `actor` a PC with a carrying skill, `corpse` an actor; the taker is registered in the entered lists first (E+ outside a recording) → 1. | E / E+ (0) | 00574a70 |
| 64 | `actor, loc, mode` | bool | Walk into a building (NAV section 5): the nearest type-1 door within 300 px of point `loc` and native 45 recorded to it; no door / not a point → error, 0. | E (0) | 00574860 |
| 65 | `actor` | bool | Records "leave the carried corpse" (PC with the skill) → 1. | E (0) | 00574db0 |
| 66 | `x` | bool | Resets an animated map element's animation → 1; other → error, 0. Unused. | E (0) | 00570dd0 (medium) |
| 67 / 68 | `i` | void | Records "start" / "stop mobile element" for **player character `i`** (unchecked PC-list index). | U | 005783b0, 00578430 |
| 69 | `actor, k` | bool | Records "speak remark `k`" (0..119) for a human → 1; else error, 0. | E (0) | 00572ef0 |
| 70 | `actor, target, k, range, reporter, msg` | bool | Records "seek `target`" (walk if `k` = 0 else run, float `range`) with an attached message `(msg, 0, 0)` to `reporter` (null = level) sent when the seek ends → 1; `reporter` not an actor → error, 0. | E (0) | 00573da0 |
| 71 | `…, msg, a, b` | bool | As 70 with `(msg, a, b)`; `msg < 1000` → error, 0. Unused. | E (0) | 00573f00 |
| 72 / 73 | `i` | void | Records "activate" / "deactivate mobile element" for player character `i` (unchecked). | U | 005784b0, 00578530 |
| 74 | – | handle | The current actor (VM-093). | – | 00578050 |
| 75 | – | int | The element table's full size `N` (VM-030b). | – | 005714d0 |
| 76 | `x` | bool | `x` is an animated map element. | – | 00570a70 |
| 77 | `x` | bool | `x` is of the target family; null → 0; unknown → error 0. | E (0) | 00570ae0 |
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
| 91 | `x` | int | **Excluded (4.2).** A posture code of human `x` from {0, 2, 4, 5, 6, 8, 9, 10, 11, 15, 16, 17}, 666 for other states; non-human / unknown → −1. Meaning open (1.1). | E (−1) | 005762d0 |
| 92 | `x, k` | void | **Excluded (4.2).** Set a posture of human `x`: codes 0, 2, 7, 10, 15, 16, 17, 100 accepted; 4, 5, 6, 8, 9, 11 → error "cannot be set"; others → error. | E | 005763e0 |
| 93 | `x` | int | Facing direction 0..15 of a known element; unknown → 0. | E (0) | 00571a70 |
| 94 | `x, d` | bool | Facing := `d mod 16` → 1 (carts through their own setter). | E (0) | 00571ab0 |
| 95 | `x` | handle | A new point location at `x`'s position; unknown → null. | E (null) | 005717a0 |
| 96 | `x, loc` | bool | `loc` null: take `x` off the map (inactive, off-map; humans stop; a present PC is removed from play; an unlocked NPC has its AI locked) → 1. Else `loc` must be a point whose ground allows standing: `x` is put back if it was off, moved there, stopped → 1. Not a point → error 0; ground not walkable → error 0 **after** the zone was already changed (E+). | E / E+ | 005718d0 |
| 97 | `x, zone` | bool | `x` is inside `zone`; not a zone → error 0; an inactive target-family element is never inside. | E (0) | 00571de0 |
| 98 | `x, building` | bool | `building` null → `x`'s zone is a building interior; else → `x`'s zone is `building`; `x` unvalidated. | U | 00577cf0 |
| 99 | `x` | bool | Un-blip (reveal the silhouette, AI-074) if `x` is marked → 1; else 0; unknown → error 0. | E (0) | 00570e70 |
| 100 | `x` | int | 1 if `x`'s movement style is the third style, else 0; unknown → 0. Unused. | E (0) | 00571550 |
| 101 | `x` | int | Current action id: target family → its action; actor family → its current action, 283 if none; others → error 0. | E (0) | 00578060 |
| 102 | `x, amount, b` | void | Damage `amount` on actor `x` of the element table, intensity flag 100 if `b` else 0, through a one-element sequence executed on the next drain (AI section 5.1). Not such an actor → error. | E | 00577500 |
| 103 | `x` | bool | **Stops** actor `x` (its current movement and order are cancelled, the stop the engine uses when a script halts an actor) → 1; non-actor / unknown → error 0. | E (0) | 00571b30 |
| 104 | `npc, x` | bool | NPC `npc` sees human `x` (AI-066). Unused. | E (0) | 00578160 |
| 105 | `npc` | void | Enable the view-cone display; a non-NPC is reported **but the call proceeds** (E+). Unused. | E+ | 005786b0 |
| 106 | – | bool | A HUD flag byte. Unused. | – | 00578820 |
| 107 | `b` | void | Set that HUD flag (event when it changes). Unused. | – | 00578830 |
| 108 | `x, a, b` | bool | Runs `FilterAIEvent(a, b)` directly, result ≠ 0. Unused. | – | 00578690 (medium) |
| 109 | `target, msg` | void | Send `(msg, 0, 0)` **now** (VM-120); bad target → error. | E | 00578d80 |
| 110 | `target, msg, a, b` | void | Send `(msg, a, b)` now. | E | 00578f20 |
| 111 | – | handle | **Always null** in this build. | – | 00579a20 |
| 112 | `k` | bool | Selection: 31 → select all, 0 → select none, else error; returns 1 always. | E (1) | 00578710 |
| 113 | `x` | bool | Deactivate `x` → 1: cart → its own deactivation; PC → removed from play; others → inactive. Unknown → error 0. | E (0) | 00575640 |
| 114 | `x` | bool | Activate (inverse) → 1; null / unknown → error 0. | E (0) | 005756f0 |
| 115 | `pc, k, b` | bool | Action `k` (0..5) of `pc` available := `b` → 1; non-PC / bad `k` → error 0. | E (0) | 00575760 |
| 116 | `pc, k` | bool | Action `k` available for `pc`; non-PC → error 0. | E (0) | 00575870 |
| 117 | `x, prop, v` | bool | Set property → 1: 0 arrows (human; 0 without a quiver), 1 money (NPC), 2 life points (human), 3 concussion (human), 4 purses, 5 stones, 6 apples, 7 ales, 8 legs, 9 plants, 10 nets, 11 wasp nests (PC counters), 12 name preset 0 / 1 / 2 (PC). Wrong kind / property → error 0. | E (0) | 00572300 |
| 118 | `x, prop` | int | Get property (same numbering; 2 life as signed 16-bit; 4..11 as counters); errors → −1. | E (−1) | 00571fb0 |
| 119 | – | bool | Some civilian in the level is dead. | – | 005761f0 |
| 120 | – | bool | Some soldier is dead. Unused. | – | 00576260 |
| 121 / 122 | – | int | Highest alert state (0..2) over living soldiers / over living non-soldier NPCs. Unused. | – | 00576a30, 00576af0 |
| 123 | `npc, s` | bool | Alert state := `s` (0, 1, 2) → 1; PC / other / bad `s` → error 0. | E (0) | 00575af0 |
| 124 | `npc` | int | Alert state 0..2; non-NPC → error 0. | E (0) | 00575bf0 |
| 125 | `npc, s` | bool | Set the AI's state (AI-041): 1 → default, 3 → seeking (soldiers only; civilians → error 0), 5 → fleeing, 7 → return to the post; 0 (sleeping), 4 (menacing), 6 (attacking) cannot be set → error 0; other → error 0. | E (0) | 00575c80 (medium) |
| 126 | `npc` | int | The NPC's top-level AI state (AI-041) as a script code: 1 default, 2 wondering, 3 seeking, 4 menacing, 5 fleeing, 6 attacking, 0 sleeping / unknown; non-NPC → error 0. (AI spec 5.1 differs: 9.4.) | E (0) | 00575e20 |
| 127 | `a, b` | bool | Obsolete: always error, 0. | E (0) | 00575ee0 |
| 128 | `npc` | int | 1 if the NPC is hostile (its attitude), else 0; non-NPC → error 0. | E (0) | 00575f00 |
| 129 | `npc, a, b` | bool | No effect; 1 for an NPC, error 0 otherwise. Unused. | E (0) | 00575f70 |
| 130 | `npc, target, b` | void | Validates `npc` (NPC) and `target` (known element); the operation itself is **empty in this build**. | E | 00575fc0 |
| 131 | `npc, loc, b` | void | Validates (NPC, point); empty operation. Unused. | E | 005760b0 |
| 132 | `npc, path` | void | Assign the patrol path (AI 3.4); non-NPC → error. | E | 00576c50 |
| 133 | `npc, loc, d` | void | Set the NPC's post at point `loc` facing `d`; non-NPC → error. | E | 00576cc0 |
| 134 | `x, b` | void | Lock the AI: NPC → its lock with flag `b`; animal → its lock flag := 1; PC / other → error. | E | 00576d80 |
| 135 | `x` | void | Unlock the AI: NPC (error if not locked); animal → unlock and wake; PC / other → error. | E | 00576e00 |
| 136 | `soldier, k` | void | Force a battle decision: `k` 0..16 → the seventeen decisions in order (unnamed, 4.3), 99 → none; `k ≥ 100` → `k − 100` as a temporary decision; other → error; non-soldier → error. | E | 00576eb0 |
| 137 | `loc, k` | void | Make a noise at point `loc`: `k` 0 → noise type 11, 1 → type 12; other `k` → error **and the value is still used as the type** (E+); null → error. | E / E+ | 00571160 |
| 138 | `human, b` | void | Freeze flag := `b`; unknown / non-human → error **but still applied** (E+). | E+ | 00576bb0 |
| 139 | `b` | void | Level "freeze all" flag := `b`. | – | 00576c30 |
| 140 | `npc, k` | void | Walking style: 0 walk, 1 run, others verbatim (AI-093); non-NPC → error. | E | 005780d0 |
| 141 | `soldier` | int | Rank; non-soldier → error 0. | E (0) | 00579a30 |
| 142 | `x` | bool | Animated map element active flag; other → error 0. | E (0) | 00570d00 |
| 143 | `x, b` | bool | Animated map element active := `b` → 1; other → error 0. | E (0) | 00570d80 |
| 144 | `patch` | bool | Patch active flag; null unchecked. | U | 00570e20 |
| 145 / 146 | `patch` | bool | Activate / deactivate the patch, clear the actor-follow lock → 1. | – | 00570e30, 00570e50 |
| 147 | – | bool | Stop all playing level sounds → 1. | – | 00575510 (medium) |
| 148 | – | bool | Restart the ambient sounds around the camera → 1. | – | 00575530 (medium) |
| 149 | `sound` | bool | Start sound source `sound` (once) → 1. | – | 00575560 (medium) |
| 150 | `sound` | bool | Stop its playing instance → 1. | – | 00575590 (medium) |
| 151 | `sound` | bool | Stop and release it → the release result. | – | 005755c0 (medium) |
| 152 | `x` | bool | Take actor `x` (of the table) out of its building (NAV-191) → 1; outdoors / not such an actor → error 0. | E (0) | 00577150 |
| 153 | `x, zone` | bool | Fire `ExitZone(x)` for a member `x` → 1 (VM-120b); otherwise error 0. Unused. | E (0) | 00577220 |
| 154 | `x, zone` | bool | Fire `EnterZone(x)` for a member `x` → 1; otherwise error 0. Unused. | E (0) | 00577390 |
| 155 | `x` | void | No effect. | – | 00578c50 |
| 156 | `x, building` | void | Put actor `x` inside `building` (NAV-190): entry point, zone := the building, default facing, hidden, registered; not an actor → error **but the call proceeds** (E+). | E+ | 005781f0 |
| 157 | `x, b` | void | **Excluded (4.2).** Sets a flag `b` on an object and on its listed parts. Unused. | – | 005782a0 |
| 158 | `zone` | handle | First actor inside `zone`, null if none; not a zone → error null. Unused. | E (null) | 00571ea0 |
| 159 | – | handle | **Always null** (as 111). | – | 00579a20 |
| 160 | `a, b` | int | Distance between two points (float, truncated); a non-point → error 0. | E (0) | 00571b80 |
| 161 | `n` | int | `rand() mod n` with the C runtime generator (8.2); `n = 0` traps. | T | 00578680 |
| 162 | `x` | void | Debug output of `x` as a decimal number (no game effect). Used once. | – | 00579f10 |
| 163 | – | int | Size of the campaign's character roster list. | – | 005791d0 |
| 164 | `i` | handle | Element for roster entry `i` (unchecked). | U | 005791f0 |
| 165 / 166 | `pc` | void | Add / remove `pc` to / from the next mission's team (HUD refresh); non-PC → error. 165 also marks the PC present and resets two of its fields. | E | 00579210, 00579280 |
| 167 | – | int | Size of the selected mission's team list. | – | 005792d0 |
| 168 | `i` | handle | Element of team entry `i`; `i` outside `0 <= i < size` (unsigned compare, so negatives too) → **exception** (X). | X | 00579350 |
| 169 | `x` | bool | `x`'s campaign character id is in the selected mission's team list. | – | 005793a0 |
| 170 | – | bool | Low byte of "the team satisfies the selected mission's requirements". | – | 005793e0 (medium) |
| 171 | – | int | First code of the "available missions" list, 0 if empty. | – | 005793f0 (medium) |
| 172 | – | int | Code of the selected mission, 0 if none. | – | 00579410 |
| 173 | – | bool | **Excluded (4.2).** A game-settings flag (which setting is not identified). Called 14 times, result read. | – | 005795b0 |
| 174 | – | int | Team size limit of the selected mission; 5 if none; outside the hub node → error 0. | E (0) | 00579300 |
| 175 | `id, loc` | void | Place the PC with campaign id `id` at point `loc`; none → nothing. | – | 00579000 |
| 176 | `soldier, n` | void | Company number := `n`; non-soldier → error. | E | 005790b0 |
| 177 | `soldier, b` | void | Always-attentive := `b` (AI 5.1); non-soldier → error. | E | 005790f0 |
| 178 | `banner` | void | Capture a banner (must be active; else error): deactivated; campaign counter 3 += its value; in a type-1 node reaching the requirement **declares victory** (VM-103b); HUD refresh. | E | 005795c0 |
| 179 | `banner` | void | Lose a banner (inactive → active, counter 3 decreased, HUD refresh); null → error, then unchecked. | E / U | 005796d0 |
| 180 | `human, b` | void | Invisible flag := `b`; non-human → error. | E | 00579760 |
| 181 | `human` | bool | Invisible flag; non-human → error 0. | E (0) | 005797c0 |
| 182 - 185 | `door` | bool | Door lock bytes: 182 player lock, 183 lock-pick byte, 184 soldier lock, 185 civilian lock (NAV-172); null unchecked. | U | 00579810 - 00579840 |
| 186 - 189 | `door, b` | void | Set those bytes; 186 / 188 / 189 with `b = 0` also open the door (NAV-172); null unchecked. | U | 00579850 - 005798a0 |
| 190 | `x, a, b` | void | **Excluded (4.2).** Forwards `(a, b)` to a door / lift operation. Unused. | – | 005786f0 |
| 191 | `b, door` | void | The door leaf's click-target flag := `b`; null → error. | E | 005787f0 |
| 192 | – | handle | The current scroll (VM-094). | – | 005798c0 |
| 193 | `scroll` | int | Scroll status 0..3; not a scroll → error 0. | E (0) | 005798f0 |
| 194 | `scroll, s` | void | Status := `s` (0..3; else error); 1 and 3 visible, 0 and 2 hidden; 3 also plays the scroll sound; not a scroll → error. | E | 00579950 |
| 195 | `k` | int | Campaign value `k + 7` for `k` in 0..19; else error 0. | E (0) | 00579430 |
| 196 | `k, v` | void | Set it; else error. | E | 00579470 |
| 197 | `npc, k` | int | NPC custom value `k` (0..9); errors → −1. | E (−1) | 005794b0 |
| 198 | `npc, k, v` | void | Set it. | E | 00579520 |
| 199 | `k, loc, n` | void | Camp production zone `k` at `loc` with capacity `n`. | – | 005799d0 (medium) |
| 200 | `k, loc` | void | Camp production zone `k` at `loc`. | – | 00579a00 (medium) |
| 201 | `id` | handle | The player-character slot with campaign id `id`; beyond the slot count → error null; none → null. | E (null) | 00571610 |
| 202 | `k` | void | Show popup page `k` **now** in the synchronous modal loop (VM-218). | – | 00579f30 |
| 203 | `k` | void | Records "popup page `k`" (modal when executed). | E | 00579fa0 |
| 204 | `zone` | int | Number of actors inside `zone`; not a zone → error 0; null → 0. | E (0) | 0057a060 |
| 205 | `zone, i` | handle | `i`-th actor inside (`i` < count) else error null. | E (null) | 0057a0a0 |
| 206 / 207 / 208 | `a, b` | int | `a & b`, `a \| b`, `a ^ b` (full 32-bit). Unused. | – | 0057a280 - 0057a2a0 |
| 209 | `pc, k` | bool | PC has skill `k` (0..29 except 22 → error 0) by either of two tests; non-PC → error 0. | E (0) | 0057a2b0 |
| 210 | `k` | bool | Some player character (any, in PC-list order) has skill `k`; bad `k` → error 0. | E (0) | 0057a4c0 |
| 211 | – | handle | The first PC flagged as the leader, or null. | – | 0057a8e0 |
| 212 | `actor, loc, mode, d` | bool | Records a walk "near" point `loc` at distance `d` (modes as 45; variant unnamed, 4.3) → 1. | E (0) | 005743a0 |
| 213 | `a, b, t` | handle | New point at `a + (b − a) × t`; both points in the same zone, else error null. | E (null) | 00571c60 |
| 214 | `soldier` | void | Declare combat trainer; non-soldier → error. | E | 00579160 |
| 215 | `k` | handle | The active relic / information scroll of kind `26 + k` (k 0..6), or null. | – | 0057a950 |
| 216 | – | int | Number of player characters (PC list, `mod 256`). | – | 0057aa20 |
| 217 | `i` | handle | Player character `i mod 256` (unchecked). | U | 0057aa50 |
| 218 | `chief, sub` | void | Add `sub` as subordinate of `chief` (AI-097): both NPCs, neither already subordinate, `sub` not a chief, not self; else error. | E | 0057aa70 |
| 219 | `npc` | void | Remove all subordinates; non-NPC → error. | E | 0057acc0 |
| 220 | `soldier` | void | Switch to the alert path; non-soldier → error. | E | 0057ad30 |
| 221 | `x` | bool | Soldier `x` is a rider; null / non-soldier → 0. | E (0) | 0057ad90 |
| 222 | `x` | bool | `x`'s highlight mark is clear (99 clears it; opening a door through 186 / 188 / 189 raises the door's). | E (0) | 00570ec0 |
| 223 | `banner` | bool | Banner inactive (= captured); null → error then unchecked. | E / U | 00579730 |
| 224 | `loc, a, b, c` | int | **Excluded (4.2).** Creates a repulsive point at point `loc` with `(a, b, c)` (effect observed) and returns that operation's result (not read); not a point → error 0. | E (0) | 0057adf0 |
| 225 | `n` | void | **Excluded (4.2).** A level parameter := `n` and every element refreshed. Unused. | – | 0057ae60 |
| 226 | `b` | void | Records "freeze-all flag := `b`". | E | 00577df0 |
| 227 | `x` | void | **Excluded (4.2).** Forwards to a trap operation. Unused. | – | 0057ae40 |
| 228 | `npc, k, v` | void | Head marker (AI-084): `k` 0 clears; 1..7 → markers 2..8 for `v` frames; else error; non-NPC → error. | E | 0057aed0 |
| 229 | `human` | void | Confiscate: a non-PC human's money → campaign counter 1, then zeroed; non-human → error. | E | 005729a0 |
| 230 | `zone` | bool | Every player character is inside `zone`. Unused. | – | 00571e30 |
| 231 | `zone` | bool | No active hostile soldier in `zone` can still fight (alive, not unconscious, not tied, not fleeing); not a zone → error 0. | E (0) | 00571ef0 |
| 232 | `pc` | void | Add `pc` to the player's band (campaign roster); a non-PC is reported **but the roster update proceeds** (E+). | E+ | 0057afe0 |
| 233 | `npc, scroll` | void | Attach information scroll `scroll` (null detaches) to `npc`; a non-scroll is reported **and still attached** (E+); non-NPC → error. | E / E+ | 005785f0 |
| 234 | – | bool | Campaign counter 3 ≥ the required banner count; 0 outside a campaign. | – | 00579690 |
| 235 | `item` | bool | Bonus item picked-up flag; not a bonus item → error 0. | E (0) | 0057b080 |
| 236 | – | int | Campaign money (counter 1); outside a campaign → error −1. | E (−1) | 0057b0f0 |
| 237 | `v` | void | Set campaign money; outside → error. | E | 0057b120 |
| 238 | – | int | **Excluded (4.2).** A HUD value. Unused. | – | 0057b150 |
| 239 | – | void | Show the debriefing screen. | – | 0057b160 (medium) |
| 240 | `x` | bool | Active flag of a known element; unknown → error 0. | E (0) | 00570d40 |
| 241 | `k, a, b` | void | **Excluded (4.2).** A campaign-level operation keyed by `k` with two further values `a`, `b`; the only established observable effect is that the flag read by native 261 becomes set; the rest is not identified. Used once. | – | 0057b2b0 |
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
| 254 | `x, b` | void | A flag of `x` := `b` (which flag: unnamed, 4.3; unchecked). | U | 0057ba50 |
| 255 | `k` | bool | Some *present* player character has skill `k`. | E (0) | 0057a6d0 |
| 256 | `pc` | int | Campaign identity of `pc`: 0 the hero, 1..5 the five named companions (John, Tuck, Stuteley, Scarlet, Marian), 6..8 the three generic followers; unknown profile / non-PC → error −1. | E (−1) | 0057ba60 |
| 257 | `pc, b` | void | Select (`b`) / deselect `pc` (HUD events); null → all; non-PC → error. | E | 0057bdb0 |
| 258 | `pc` | bool | `pc` is selected and an action is selected in the HUD; non-PC → error 0. | E (0) | 0057bed0 |
| 259 | `human` | int | Action state 0..17 (unnamed, 4.3), else 666; non-human → −1. | E (−1) | 005766f0 |
| 260 | `human, k` | void | Set action state `k` (0..17); non-human → error. | E | 00576830 |
| 261 | – | bool | **Excluded (4.2).** A campaign flag: set by native 241 (and by a successful level end), cleared at a successful level end; what it means for play is not identified. Used once. | – | 0057bfe0 |
| 262 | `n` | void | Screen fade in `n` steps. Used once. | – | 0057bff0 (medium) |
| 263 | `target, fx` | void | Link an animated map element to a target object; wrong kinds → error. Unused. | E | 0057c500 |
| 264 | `npc, k, b` | void | Forbid remark `k` (< 120) with flag `b`; non-NPC / bad `k` → error. | E | 00573010 |

Ids never called by the retail scripts (73 of 265): 11, 14, 15, 22, 23, 25, 36, 37, 57, 60, 61, 63, 65, 66,
67, 68, 71, 76, 77, 78, 83, 84, 100, 104, 105, 106, 107, 108, 115, 116, 120, 121, 122, 123, 124, 127, 129, 131,
136, 138, 141, 142, 147, 148, 151, 153, 154, 157, 158, 167, 168, 169, 171, 175, 176, 181, 183, 184, 185, 190, 201,
206, 207, 208, 225, 227, 230, 238, 242, 251, 257, 260, 263.

## 7. Acceptance tests

Synthetic programs and expected outcomes; "tick" = an executed level tick; unless stated, the level has scripts
enabled and the mission-variable array is empty at start (VM-020).

1. **Return semantics (VM-047, VM-050, VM-072).** `f` = `0x03; 0x13 t,7; 0x07 t; 0x13 t,9; 0x07 t; 0x06`; caller `0x05 f; 0x0A x` → `x = 7`. A callback `0x03; 0x06` on an instance whose previous callback returned 5 → the engine reads 5.
2. **Persistent result slot (VM-050).** `0x05 f; 0x13 y,1; 0x0A x` → `x = 7`; `0x05 f; 0x05 g` (g returns 3); `0x0A x` → `x = 3`.
3. **Nested callback (VM-095).** Level `Initialize`: `n0(0, 0)`. Level `Hourglass`: `n1(0, 1); n109(null, 5); n2(0) → x; 0x06`; level `ProcessMessage(5,…)`: `n1(0, 2); return value 9` → `x = 2`; the Hourglass continues to its end; **the engine's readable return register afterwards holds 9** (the inner 0x07 was not restored). Variant: the Hourglass then calls `g` (returns 4) and reads `0x0A` → 4 (the inner callback's write to the outer frame's slot was overwritten by the later callee); variant without `g`: an `0x0A` in the Hourglass after `n109` reads 9.
4. **Barrier (VM-201).** `n30(); n32() → a; n32() → b; n56(5); n32() → c; n31()` → `a = 1, b = 1, c = 2`; one sequence with the timer at level 1. `n30(); n30() → r` → `r = 0`, the recording unchanged. A loop of 65535 `n56(1); n32()` pairs → the level reads 0 after the last barrier (OpenSherwood: a fault at the wrap, 8.1).
5. **Outside a recording (VM-203).** `n56(5)` alone → error, nothing scheduled; `n43(null, 1)` alone → no `ProcessMessage`; `n46(actor, p, -1, 0)` alone → the actor is placed at `p` (and stays there), no walk, result 0; `n109(null, 1)` → `ProcessMessage(1,0,0)` runs before `n109` returns.
6. **Simultaneous timers (VM-221).** Level 1 = timers 3 and 3; level 2 = message 9 to the level; launched from `Hourglass(0)` (step 4 of tick 0): the pass of tick 0 already visits both (3 → 2), tick 1 (2 → 1), tick 2 (1 → done); both complete in the pass of tick 2 in insertion order; `ProcessMessage(9)` runs inside that pass, before the pass visits any later timer; a timer it inserts is first visited in tick 3. `n56(0)` completes on its 2^32-th visited pass; `n56(-1)` on its (2^32 − 1)-th.
7. **Hourglass schedule (VM-103, VM-242).** No forcing: `Hourglass(0)` and `CheckVictoryCondition(0)` at T = 0; `Hourglass(1)` at T = 25 without a check; `CheckVictoryCondition(3)` at T = 75. `n29()` inside `Hourglass(1)` → `CheckVictoryCondition(1)` **in the same tick** (T = 25), and `(3)` still at T = 75. `n29()` from a zone callback at T = 30 → `CheckVictoryCondition(2)` at T = 50.
8. **Victory and end (VM-103b).** `CheckVictoryCondition` returns 1 at T = 75 with two present PCs → no `Finalize`; the notice is shown once; nothing ends until a trigger fires: (a) both PCs dead or departed → end bookkeeping in that tick, `Finalize(0)` next tick, code 2; (b) a later `CheckVictoryCondition` result 2 → the same success end; (c) the abort flag → the same. Without a prior victory, result 2 at T = 150 → bookkeeping in that tick (T stays 150), `Finalize(1)` next tick, code 3.
9. **Modal suspension (VM-218).** Level 1 = page 4 (a queued category); level 2 = timer 1. The page opens when the element is **drained** (step 9 of the launching tick, or of the next tick if launched after step 9); until dismissal T does not advance and no other sequence or camera element progresses; after dismissal the element is done, level 2 starts, and its timer is visited by the next timer pass. `n202(4)` inside `IsTaken` returns only after dismissal.
10. **Abort propagation (VM-217).** Level 1 = walk of a dead actor, timer 2, message 7; level 2 = message 8. Message 7 runs at dispatch; the walk is refused when drained (state refused, propagated to message 8); the timer still completes; level 1 never completes; `ProcessMessage(8)` never runs; the sequence is deleted at the next housekeeping. Setting the refused element to refused again changes nothing.
11. **Re-entrancy (VM-213).** Level 1 = message A (to the level), message B (to the level); `ProcessMessage(A)` launches a sequence whose level 1 = message C and a timer → order: `ProcessMessage(A)`, `ProcessMessage(C)` (nested launch dispatched completely), then `ProcessMessage(B)`; the timer is queued after any entry already queued. If `ProcessMessage(A)` cancels message B's element → B never runs and the level completes when A's and the remaining elements are done.
12. **Message target (VM-120).** `n43(null, 1)` in a launched recording → level `ProcessMessage(1,0,0)`; `n43(scroll, 1)` → error, nothing; `n44(actor, 2, 3, 4)` → that actor's `ProcessMessage(2,3,4)` with `n74()` = that actor inside it.
13. **Context (VM-093, VM-094).** Inside an actor's `ProcessMessage` that calls `n109(null, 6)`: the level's handler sees `n74()` = that actor. Inside a scroll's `IsTaken` that sends `n109(actor, 6)`: the actor's handler sees `n192()` = that scroll, `n74()` = the actor; after both return, `n192()` = null.
14. **Invalid inputs (VM-089).** With `N = 130` and an array of 16 declared variables: `n3(999)` → null with an error; `n3(-1)` → null silently; `n2(99)` → −1; `n1(99, 5)` → no effect; `n24(x, 444)` → no effect; `n161(0)` → a fault (8.1); `n8(-1)` → null (8.1); `n168(-1)` and `n168(size)` → a fault (X); a jump to `0xFFFFFFFF` → the callback ends (8.1).
15. **Arithmetic (VM-061..068).** `0x24` on (2, 2) → 1; `0x26` on (−1, 0) → 0; `0x2B` on (NaN, 1.0) → 1.0f; `0x2E` on (NaN, 1.0) → 0.0f; `0x2C` on (NaN, NaN) → 0.0f; `0x17` on 4294967296.0 → 0; `0x1C` on (INT_MIN, −1) → a fault; `0x19` on (INT_MAX, 1) → INT_MIN.
16. **Result conventions (VM-086).** `n206(0x1FF, 0x100)` → 0x100; `n128(hostile npc)` → 1; `n112(7)` → 1 with an error; `n46(...)` → 0 even when it recorded; `n2(k)` holding −5 → −5; `n75()` = the full table size while `n3(N16)` addresses the first cart.
17. **Restore equivalence (VM-014, VM-200, 8.3).** Run a level to the end of tick 10 with an open recording left by a callback (a `n30()` without `n31()`), two pending sequences, three timers, mission variables and a nested-callback return register (case 3); snapshot; run tick 11; restore the snapshot; run tick 11 again → identical script-visible state, identical native calls in identical order, identical callback return registers.
18. **Initialisation order (VM-104).** An actor `Initialize` sets variable 0 := 1 (after declaring it) and the level `Initialize` sets it := 2: the level's value wins; the scrolls' `Initialize` run after it; `PostInitialize` runs after `Hourglass(0)` when the first tick executes.
19. **Camera natives (VM-219).** After `n18(p)` the zoom is unchanged and the camera scrolls toward `p` at 2.0 px per camera update (until the ramp / border rules of ANIM-330 apply); after `n20(p)` the camera corner equals `clamp(p)` on the next drawn frame; after `n21(2.0)` the zoom request is 2.0 and the drawn zoom changes in later updates.

## 8. Implementation choices

### 8.1 Deliberate departures from the original

| Case | Original | OpenSherwood | Reason |
|---|---|---|---|
| fixed timestep | 40 ms requested, 46.875 ms realised on the measured host (VM-100) | 46.875 ms (the movement spec's decision), recorded as `Assumption::FrameLength` until that spec is reviewed | the shipped data was authored at the realised rate |
| jump to `0xFFFFFFFF` (VM-070) | unchecked fetch | the callback ends as by 0x06 | the script's intent; UB |
| unchecked accesses (U) | arbitrary memory | null / 0 result, `Fault::UncheckedAccess(id)` (`n8(-1)` → null) | UB |
| arithmetic traps (T) | process exception | deterministic fault of the callback | crash |
| container exception (X: 168) | C++ exception unwinds the native | deterministic fault | crash-equivalent |
| fatal routine (F) and null dereference (C) | termination / crash | fault; a missing callback is a recorded no-op returning the register | crash |
| non-termination (N: ≥ 0x30) and the 0x00 fetch | hang / unchecked fetch | fault | hang / UB |
| argument buffer overflow (VM-087) | unchecked write | fault | UB |
| barrier wrap (VM-201) | stuck recording | fault at the 65536th level | unreachable in retail data; UB-like |
| excluded natives (4.2) | unread effect | `UnknownNative(id)`, placeholder of 4.2 (or a strict fault) | not settled |
| version check (VM-001) | float equality (NaN accepted) | bit-exact 1.5 | no retail file is NaN |
| timers `n <= 0` (VM-221) | wrapping counter | identical wrapping counter (no departure) | cheap and faithful |

### 8.2 Random numbers

Native 161 draws from the process-wide C-runtime generator (the linear congruential `rand()` of the Microsoft
runtime, 15-bit results), shared with every other consumer (the AI, `spec-ai-combat.md` 2.2). OpenSherwood uses
its named seeded stream for the script (ADR-0004); the consumption order between the script and the AI within
a tick follows VM-103 and belongs to the determinism model, not to this spec's fidelity claims.

### 8.3 Snapshot and restore boundaries

Snapshots are taken only at **tick boundaries outside any callback and outside any modal loop** (no frame is
live then). Authoritative script state (all hashed): per instance VM-014 (class block, both registers, the
argument buffer with its count, the global block); the mission-variable array; `T`, the force flag, the won flag
and sub-flag, the three end flags, the debriefing index; **the recording state** (the open sequence with its
elements and their levels, the level counter, the two entered lists — a recording *can* span callbacks, VM-200);
every live sequence (elements with category, target, arguments, state, level; level counters; the manager's
FIFO order; the timer list with counters and order); the current actor and current scroll are null at a tick
boundary. Element flags read by natives belong to their owners' snapshots. Restoring at a tick boundary
reproduces the following ticks exactly (case 17).

### 8.4 Differences from the current engine (`docs/formats/scb.md`, `natives.rs`)

1. Opcode 0x07 returns immediately.
2. Comparisons: 0x24 `<=`, 0x26 `>=`, 0x27 `>`, 0x28 `!=`, signed; 0x2A..0x2F float compares with float results and unordered outcomes; 0x1C / 0x1F / 0x16 / 0x17 exist.
3. Jump / call / native-call targets are `a | (b << 16)`; the two `-1` jumps.
4. Native 2 returns −1 for an undeclared variable; 0 grows by 16; 1 errors on undeclared.
5. The element table includes one slot per campaign character; 3 truncates its boundary to 16 bits while 75 does not; 10 returns cart indices from 0.
6. Natives 111 and 159 are null: messages "to the player" go to the level class.
7. Messages are synchronous; 44's fourth argument is `arg2`.
8. The clock: 40 ms requested, 46.875 ms realised (VM-100); Hourglass every 25 ticks with `T/25`; `CheckVictoryCondition` every third Hourglass or forced (same tick when forced inside Hourglass); scroll Hourglass on its own counter with 0; actor classes never get Hourglass; `HandleEvent` never; `ActivatedBy*` results ignored.
9. `Finalize(0)` = success, `(1)` = failure; a declared victory ends nothing by itself; result 2 requests the end bookkeeping; the tick has early exits.
10. Initialisation order (VM-104).
11. Sequences: level-parallel with a barrier that counts once; abort cascade; pages / dialogs are synchronous modal loops (203 does hold the sequence, by suspending the program); 33 / 42 wait for the camera update, 34 is instant; recording natives outside a recording drop their element (46 / 47 / 63 keep their placement).
12. Native semantics corrected — section 6 throughout; notably 18 / 19 scroll requests with a step length (not a zoom, not a "deployment area"), 20 a jump, 21 a zoom request; 24 with 444 a no-op; 49 / 50 / 51 completed by the actor (50 never); 72 / 73 hide / show PC `i`; 74 the current actor (the object during `ActivatedBy*`); 96 with null takes the actor off the map; 98 with null tests "in a building"; 101 the action id (283 none); 130 / 131 no-ops; 178 declares victory; 182 - 189 door lock bytes; 193 / 194 scroll status with visibility; 206 - 208 full-width; 210 any PC; 211 the leader; 236 / 237 campaign money.
13. Failure classes (VM-089) replace the engine's strict-mode traps; all 265 ids exist.
14. Temporaries / locals zero at every entry; the callback return register persists (also across nested callbacks).
15. Natives 30 / 31 / 32 return values; 30 while recording is an error returning 0.

## 9. Open questions

### 9.1 Actor-side element execution (navigation / movement / AI)
- Admission predicates and the busy actor's merge / priority rules (004646e0, 0046b210, 0046ad00); the end conditions of actions (native 59) and corpse elements (63 / 65); the speech end detail (ANIM-140); the walk element's own completion frame (NAV open question 3; 00467a50, 0046bd40).
- The variant of modes 2 / 3 of 45 and of 212 (00582640, 00583630); the entered lists of 46 / 47 / 63 beyond the first placement (00577650, 005779a0, 0057c950).

### 9.2 Excluded natives
- 13: the map's zone list (00579d70; built by 004fb8e0) versus the level's location list (005714f0): same objects in the same order?
- 173: which game setting the flag reflects (005795b0; the writer 004b90f0).
- 224: the repulsive-point operation 004ec910; 225 (0057ae60, 0048ebe0); 227 (004eca10).
- 241 / 261: the campaign operation behind 241 (004524b0) and the meaning of the flag read by 261.
- 91 / 92: posture codes (005762d0, 005763e0; AI open question 4).
- 157 (005782a0), 190 (005786f0, 0057dea0), 238 (0057b150, 0055dbb0): unused; low priority.
- The seven events 100..106 of the second AI dispatcher (0040dcb0) and the element it passes for 104..106.

### 9.3 Scheduler details
- The three transition flags of VM-103 step 6 (which screens set them) and the debug "no defeat" toggle (004c6ef0).
- The "may die" civilian mark and the captured-PC reference of the defeat checks (004c6ef0).
- Whether the first tick can be skipped on the first loop pass by the modal-window guard (0050f710).
- The scroll's update source (004b9f40 through its virtual slot).
- Whether script zones are registered into the element table and where (0057f8c0, 004fb8e0, before 004c2720 in 004c0510).
- The interface-mode switch after a declared victory in campaign nodes of type 3 or 6 (0050eca0).
- The realised frame length on other hosts (VM-100 b): confirmation of ANIM-002 by the movement spec's review.

### 9.4 Contradictions with the sibling drafts to reconcile
- Movement spec ANIM-331: natives 18 / 19 "jump the camera" — the executable writes the scroll target and destination, not the corner (rows 18 / 19; 00571270, 00571330 versus 005713f0).
- AI spec 5.1, native 126's code order (this spec: from AI-041's state order and the case table at 00575e20).
- AI spec 5.1, native 134 on a player character (this spec: error, no effect, 00576d80).
- AI spec 5.1, native 59 ("a play-animation step" versus an action element, 00573450).

## 10. Provenance

Ghidra project `re/ghidra/robinhood` (never committed); decompilation export `re/out/decomp_all/<address>.c`,
inventory `re/out/inventory.tsv`, strings `re/out/strings.tsv`, module map `re/notes/modules.txt` (git-ignored);
raw-byte disassembly with `scripts/ghidra/peek.py` and a local capstone session for the functions the decompiler
mis-typed (the wrapper table 004075c0 and its 265 wrappers, the frame helpers, the activation dispatcher
004bc1d0, native 210's loop, the message call sites of VM-113, the camera writes of natives 18 / 20). Analyst
notes: `re/notes/vm/` (wrapper disassembly, id → function table with arity and result convention, per-id dump,
usage statistics).

Functions read (all): 00634bb0 - 00639a80 (interpreter and instance), 0063a140 - 0063a510, 0063b2d0, 0063a940,
0063b070, 004075c0 and 00404a80 - 004075bf; natives 00570a30 - 0057c500 with helpers 004e3d20 - 004e3e30,
005709c0, 00570850, 004c2720, 004d8460, 0051d0e0, 0052b070, 004524b0, 0051d540, 0053a0b0, 0053a1a0, 0053a4c0,
0052b100, 004bae60, 004ba760, 004ba5c0, 004ba7a0, 004ba7e0, 004b9f30, 005a87f0, 005a8810, 005a88a0, 005a8780,
005a8700, 005758d0, 0057c710; scheduler 004c6ef0, 0050f710, 0050e7b0, 0050e7d0, 0050e800, 004c3740, 004ba000,
004b9fa0, 004b9f40, 004ba4e0, 004e3220, 004e3260, 004e3cd0, 005105d0, 004c8380, 004cdfc0, 0050b640, 0050eca0;
invokers 004030a0 - 00408af0 and callers 0046e7e0, 004a0f10, 004bb820, 00551bb0, 0057f8c0, 004bc3d0, 004105d0,
004abfe0, 0040dcb0, 00410620, 00464230, 0057fcc0, 0057fdb0, 00467230, 004bc1d0 (bytes); sequences 00570f00,
00571020, 00571100, 0057a020, 005866a0, 00584d60, 00585ea0, 00586ed0, 00587010, 00586e00, 00586fd0, 00585500,
00582220, 0058a3d0, 0058a940, 00582530, 00582560, 00582620, 005823f0, 0058a430, 0058bb80, 00585b70, 00585570,
00585320, 00585710, 0058ba60, 004ca410, 004646e0, 0046b210, 0058bbe0, 0058bff0, 0058c360, 0058a9c0, 005843c0,
00584980, 005849e0, 00584b40, 00584de0, 005822a0, 0058ad80, 0058af00, 0058b110, 0058b280.

Data checks: `python harness/tools/probe/scb_opstats.py <levels>`, `scb_semantics.py <levels> --natives`,
`--params`, `--pseudo --class`, and an ad-hoc probe over the parsed quads on the game data copy at
`C:\Users\przem\source\gamedata\robinhood` (read-only). Corpus facts used: 208 679 instructions in 39 files,
42 734 native calls, 192 distinct ids, the largest class 6948 instructions, maximum arity pushed before one call
6, 0x07 successors 5081 no-ops / 47 immediate loads / 27 jumps / 5 other (static adjacency, not executed flow),
value returns absent in all `Finalize` and `PostInitialize`, 1199 / 1448 `Initialize`, 235 / 274 `Hourglass`,
376 / 376 `ProcessMessage`, 68 / 312 `EnterZone`; natives 162, 241, 261, 262 each called once. No oracle
recording was used by this session; the realised frame length is taken from the movement spec's ANIM-002 (its
oracle evidence is cited there). Sibling specifications referenced (all drafts awaiting review):
`docs/original/spec-navigation.md` (NAV-130, NAV-142, NAV-152, NAV-172, NAV-190, NAV-191, section 5),
`docs/original/spec-movement-animation-camera.md` (ANIM-002, ANIM-033, ANIM-131, ANIM-140, ANIM-208,
ANIM-330 - ANIM-333, ANIM-521, element table 3.4), `docs/original/spec-ai-combat.md` (AI-041, AI-066, AI-074,
AI-081, AI-082, AI-084, AI-091, AI-093, AI-097, AI-190, sections 2.2, 3.4, 5.1). Tests that will depend on this
spec: section 7 (the VM rebuild, ADR-0009).
