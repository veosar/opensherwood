# Script VM, natives and callback scheduler (behaviour specification)

Status: `implemented (partial: interpreter core, messages, settled natives) ruleset 19`, revision 10
(revision 9 was cleared by Codex review 39, 2026-09-18: **cleared for implementation** — the interpreter core
including the sentinel departure, the scheduler-independent individually settled native effects, the message
contracts and the snapshot / fault contract, all **for scheduler-independent use**). Waiting (excluded from that
clearance): the scheduler integration (3.5, 3.7 as a whole), deferred execution (the 8.1 deferred-fault policy
beyond its stated boundary), the camera integration (VM-219's conversion and interference), and every
sibling-gated effect (1.1), the 12 exclusions of 4.2 and the 18 unresolved effects of 4.3. 235 remains an
accounting total, not a blanket clearance. Publication approval is separate (identity block).
Implementer batch 1 (2026-09-18, ruleset 19) built the cleared parts in
`crates/opensherwood-core/src/vm.rs`, `crates/opensherwood-core/src/natives.rs` and
`crates/opensherwood-script`; what is still the engine's own reading, and the questions the build raised, are
in section 11. **Revision 10** answers those seven questions and amends the claims they touch — VM-010, VM-011,
VM-066, VM-087, row 118, the 8.1 row on storage class `00`, and two rows of 4.1 — which are **awaiting
re-review** (each marked "rev. 10"); the cleared scope above is otherwise unchanged.

Identity and handoff record:
- Analyst: 2026-09-13 and 2026-09-18, session `a45d5359e8dec5140` (Claude agent, analyst role under ADR-0009),
  revisions 1..10 (revisions 5 to 10 on 2026-09-18; revision 10 = the implementer answers of section 11).
- Reviews (reviewer Codex gpt-6-astra, spec-reviewer role with access to `re/`; each review is committed under
  `docs/decisions/reviews/` and is the unique reference for its reviewer session):
  - review 14, `2026-09-13-codex-review-14-spec-script-vm.md`: **revision 1**, blob
    `92aedbf44c6e3e974bc44beb5be1c1d46b7b36c8` (commit `ccaf5df`), 28 findings, fix-then-clear;
  - review 17, `2026-09-13-codex-review-17-spec-script-vm.md`: **revision 2**, blob
    `32d5b49a7f4b2ef2a98c8f412692bd31ba9e12e9` (commit `011a956`), 20 findings, fix-then-clear;
  - review 21, `2026-09-13-codex-review-21-spec-script-vm.md`: **revision 3**, blob
    `cb47e2b4d4e6169b01765096202bfc692c095839` (commit `25b6dbe8aee71ebca4d78fb5e2382b4ff37b22a9`), 12 findings,
    fix-then-clear with a first partial clearance;
  - review 25, `2026-09-18-codex-review-25-spec-script-vm.md`: **revision 4**, blob
    `04ae60aa5d4dd88c5e3b0687165d9ed3dc3d5a79` (commit `afdfaec666b3a1d8eee8f4d927ead008628b1516`), 11 findings,
    fix-then-clear with a second partial clearance;
  - review 28, `2026-09-18-codex-review-28-spec-script-vm.md`: **revision 5**, blob
    `de8e583fa937c8e3bbeb31cd0cb93589ad4b143c` (commit `22a1e33ccb8a2c033e96c49e6da69b048caacf64`), 7 findings,
    fix-then-clear with a third partial clearance;
  - review 31, `2026-09-18-codex-review-31-spec-script-vm.md`: **revision 6**, blob
    `35346f60c89d02c342ec8240ffbead8d5d8e92ba` (commit `392e4a3`; the review text names the intermediate commit
    `c2c578a7…`, which carries the same blob), 4 findings, fix-then-clear with a fourth partial clearance;
  - review 35, `2026-09-18-codex-review-35-spec-script-vm.md`: **revision 7**, blob
    `2ec3f7743145b63acd2c3fdd99d4243dff61f2e8` (commit `5187321b7bcb683f9914c72d657c90c33db10bdc`), 3 findings,
    fix-then-clear with a fifth partial clearance;
  - review 36, `2026-09-18-codex-review-36-spec-script-vm.md`: **revision 8**, blob
    `a745790b539c6656a2719f08cbe754f53e9db847` (commit `7328461`; the review text names the intermediate commit
    `f35eebb1…`, which carries the same blob), 1 finding, fix-then-clear with a sixth partial clearance;
  - review 39, `2026-09-18-codex-review-39-spec-script-vm.md`: **revision 9**, blob
    `9eb4284ca91c3315a7c94f8805e1b070a0d8975f` (commit `5349b4f`; the review text names the intermediate commit
    `a014e38c…`, which carries the same blob), no findings, **cleared for implementation** with the scope stated in
    the status line. Revision 9 answered all eight reviews.
  - revision 10 (no review yet): answers to the implementer questions of section 11, written after batch 1 landed
    (commit `ef45066`, ruleset 19); the amended claims are listed in the status line.
- Reviewer-session identities: the Codex session behind each review is recorded in the **maintainer-held review
  log** (the lead's mapping from review number to Codex session; not supplied to the analyst and not reproduced
  here). Each committed review file names its review number, the reviewed blob and commit; that pair plus the
  maintainer's mapping is the retrievable session reference for that review.
- Exposure record, per session:
  - analyst `a45d5359e8dec5140`: the decompilation of the VM, its natives, the level tick / main loop, the camera
    update, the sequence machinery, the element-table registration and the AI / actor helpers listed in 10;
  - reviewer session of review 14: every opcode handler, all 265 native registrations and wrappers, the 39
    scripts, and a reproducible random sample of native bodies (its own statement);
  - reviewer session of review 17: the interpreter, the wrappers and native bodies, the level tick, the camera
    update, the sequence machinery and the actor admission (the subsystems its findings cite);
  - reviewer session of review 21: the VM, the native wrappers and bodies, the scheduler, the sequences, the
    camera, the actor / AI helpers and the campaign-container helpers (its own statement);
  - reviewer session of review 25: the VM and frame helpers, the natives and wrappers, the recording and dispatch
    machinery, the camera, the actor / animation helpers and the campaign helpers (its own statement);
  - reviewer session of review 28: the VM / frame, native, recording / dispatch, camera, movement / actor and
    animation evidence (its own statement);
  - reviewer session of review 31: the decompilation of the interpreter, the natives, the scroll and take
    routines and the camera update (its own statement: "inspected decompilation");
  - reviewer session of review 35: the interpreter's return and callback-entry paths, the tick, the dispatch /
    drain / state-transition routines and the scroll take routines (the addresses its findings cite);
  - reviewer session of review 36: the recording open / close / append routines (the addresses its finding cites);
  - reviewer session of review 39: the recording routines and the decompilation relevant to case 26 (its own
    statement: "the relevant decompilation").
  **None of these reviewer contexts may implement those subsystems.** No implementer session has read any of it.
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
  convention; the native protocol; the arity and result convention of all 265 ids; the effect **at the native
  boundary** of 235 ids (265 minus the 12 exclusions of 4.2 and the 18 ids of 4.3) — for the ids that hand work
  to an actor or to the camera this settles what is requested, while the execution and completion of the request
  belong to the sibling specifications of 1.1; the effect of 18 further ids only up to an unresolved code, flag or
  consumer (section 4.3); the order of the level tick and its clock *as far as the executable decides it* (the
  realised frame length is host-dependent, VM-100); the sequence recording, launch, dispatch, completion and
  abortion rules; messages; mission variables; the end of a level. **Not settled:** the 12 exclusions; the 18
  unresolved effects of 4.3; the actor-side execution of movement, animation, speech and action elements and the AI
  event meanings, which depend on the pinned sibling revisions of 1.1 — reviewed with the dispositions recorded
  there (navigation under *redo*, the others *fix-then-clear* with partial clearances) — and on their open items;
  the open questions of section 9. The snapshot / fault contract, the scheduler and the sequence
  rules are therefore not yet cleared for implementation (review 21's partial clearance is stated above).

## 1. Scope

The VM executes the per-class bytecode of a mission's `.scb` file: one interpreter instance per scripted element
(the level itself, actors, player characters, objects, scrolls, waypoints, script zones). The engine calls named
functions of a class ("callbacks") with a few integer parameters; the script calls the engine back through
numbered natives (0..264). Covered: the loader's use of the file, the instruction semantics, the calling
convention, the native call protocol, the scheduler, messages, the sequence machinery and every native id.

### 1.1 Dependencies on sibling specifications (pinned revisions and their review dispositions)

Every dependency below is pinned to the committed revision it was checked against, with the disposition of the
Codex review that covered that revision (all under `docs/decisions/reviews/`). All four siblings are `draft`; a
later revision of a sibling re-opens the rows that cite it. Pins:

| Sibling | Pinned revision | Commit | Blob | Review of that revision and disposition |
|---|---|---|---|---|
| `docs/original/spec-navigation.md` | revision 2 | `e5a2e0c` | `a1718e313bf841807d1d457c6bae771a1eb42328` | review 20: **redo** (expression-filter blockers and unread requirements); nothing of it is cleared — every navigation row below is a gate |
| `docs/original/spec-ai-combat.md` | revision 2 | `e966b05` | `e10208494c95caf92c2b356357329a61cf014c13` | review 22: fix-then-clear; cleared as individual facts: the serialized layouts, stimulus meanings, notification lifetimes, the nominal-versus-measured clock distinction, difficulty existence, the core recovery / stun / deflection boundaries; **native 126's order (4 menacing, 5 fleeing, 0 asleep) upheld** |
| `docs/original/spec-movement-animation-camera.md` | revision 2 | `b0cd053` | `f9a8c498eee7d4359e0159de459f688574841439` | review 24: fix-then-clear; factually cleared: timer stepping, turning, the **human 49 / 50 completion behaviour**, and the direct native distinction "18 / 19 request scrolling and preserve leftover speed; 20 jumps and does not clear the actor lock" *subject to the shared coordinate conversion*, which is not cleared |
| `docs/original/spec-campaign-camp-saves.md` | revision 3 | `2cb9c2a` | `04e00f567bd79c0a0968ee82a47eef9907365db9` | review 23: fix-then-clear; cleared: record framing and the interface tables; CAMP-012 (the twenty script values of 195 / 196) and CAMP-310 (native 165's three unidentified per-character writes) are the claims this spec depends on |

| Topic | Depends on | Status of the dependency (at the pinned revision) |
|---|---|---|
| Realised frame length of the script clock | movement ANIM-002 (counter granularity; "reference cadence for that host"), ANIM-521 (the two flags that disable the frame wait); `docs/decisions/ADR-0010-logic-frame.md` for the OpenSherwood choice | draft; VM-100 (b) is exactly ANIM-002's conclusion and carries its confidence |
| Walk elements (natives 45, 46, 47, 63, 64, 212) and seeks (57, 70, 71): the orders pushed, arrival, failure | navigation NAV-130 (the walk sequence and its added elements), NAV-146 (consuming a search result: one move action per waypoint; an **empty result fails the element 100 ticks after it arrived** — ticks, i.e. 4.6875 s at the reference cadence; whether the request is re-run meanwhile is unread there), NAV-150 (a) (the **approach** element completes on `max(|dx|, |dy|) < tolerance + 5` px, a Chebyshev test distinct from move-action arrival); movement ANIM-208 (ordinary move-action arrival: a directional projection rule, not read here) and its element table 3.4 (kind 0x14 row) | navigation under **redo** (review 20): every walk-completion rule is a gate; NAV open question 3 (completion frame of a move-to-point action) open |
| Doors, buildings, patches behind natives 4, 8, 64, 98, 152, 156, 182 - 191 | navigation section 5, NAV-172, NAV-190, NAV-191 | draft |
| Animation elements (49 / 50 / 51), speech (62 / 69), seek (57 / 70 / 71) | movement element table 3.4 (rows 0xA4 / 0xA5 / 0xA6 / 0x92 / 0x15), ANIM-033, ANIM-131, ANIM-140 (speech end: **unknown** there, low confidence, `SpeechDuration`) | draft; speech completion is an implementation gate |
| Camera natives 18 - 21, 33 - 35, 39, 40, 42 and the camera update | movement ANIM-320 (camera state), ANIM-322 (border clip), ANIM-325 (zoom values), ANIM-326 / ANIM-333 (zoom transition; count **unknown**), ANIM-330 (scroll element and per-update rule), ANIM-331 (natives 18 / 19), ANIM-332 (native 20 and the jump element), ANIM-340 (actor lock) | the "jump" contradiction on 18 / 19 is resolved (ANIM-331 agrees with rows 18 / 19; review 24 cleared the direct native distinction); **not agreed / not cleared**: the shared destination conversion (VM-219 states it from the executable; the movement pin does not), the zoom-reset fallback (absent there), the interference of 18 / 19 with a running camera element, and the zoom element's duration (`ZoomTransition`) |
| AI events seen by `FilterAIEvent`, the pre-filter, alert / AI states behind 123 - 126, 128, 132 - 136, 140, 177, 218 - 220, 228 | AI AI-041 (state set), AI-081 (event set), AI-082 (pre-filter), AI-084 (head markers), AI-190 (callback ids and renumbering), its native summary **§6.1** | resolved: native 126's order (review 22 upheld the reading both specs share); **unresolved** (9.4): 134 on a player character, 59; the meanings of events 100..106 are an implementation gate |
| Posture codes returned by 91 / set by 92 | AI: the posture factor inside AI-066 (a visibility rule that consumes the codes) and its open question 4 (the codes are unnamed there too) | **open**; natives 91 / 92 excluded (4.2) |
| Element admission by an actor (which elements a dead, absent, locked or busy actor refuses) and the actor's priority / merge rules | AI AI-082 (a) for the locked case; the actor's element runner (00467a50) otherwise — not covered by any pinned claim | **open** — VM-216; implementation gate |
| Action codes of native 59, battle decisions of 136, action states of 259 / 260 | AI §6.1 (59 as "a play-animation step": contradiction, 9.4); no pinned claim names the decisions or the states | **open** — 4.3 |
| Campaign values of 195 / 196; the per-character writes of 165 | campaign CAMP-012, CAMP-310 (pinned above) | draft; CAMP-310 leaves the three writes unidentified — 165 stays in 4.3 |

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
| VM-010 | Instance state: program counter; a stack of frames; the class-variable block; a reference to the program-wide **global block descriptor** (storage class `00`) — a static (pointer, size) pair that is **never allocated by the program**: it stays (null, 0) for the process's life, its only writer being the release at shutdown, so there is no global block at all and a class-`00` reference is an unchecked access through a null base (VM-011; rev. 10); the *current parameter buffer* (growable); a native argument buffer of 12 cells (48 bytes) with a fill count; a **native result register**; a **callback return register** (0 at construction, **never reset**). | observed | 006390b0, 00639960, 00639a00, 00634c30, 0063a1e0 (bytes) | high |
| VM-011 | Symbol operands are `u16`: bits 15..14 select the storage class (`00` global, `01` class, `10` frame locals, `11` frame temporaries), bits 13..0 a byte offset. Resolution is `base of the selected block + offset` with **no bounds check** against the block's size for any class; for class `00` the base is null (VM-010), so every class-`00` reference is class U (a read returns whatever lies at that low address in the original; OpenSherwood: 8.1, rev. 10). The retail files contain no class-`00` symbol. | observed | 00634c30 (bytes), 0063a1e0 (bytes) | high |
| VM-012 | A frame holds: the return program counter; a 4-byte *result slot* (uninitialised at creation); the caller's parameter buffer; a locals block and a temporaries block (allocated zero-filled by 0x03; re-executing 0x03 frees and re-allocates them). | observed | 0063a250, 00634d30 | high |
| VM-013 | Creating a frame (0x05 and callback entry) saves the current parameter buffer into the new frame and installs a fresh empty one; popping (0x06 / 0x07) frees the saved buffer and the frame's blocks; the current buffer stays the callee's. | observed | 0063a250, 0063a320, 0063a3b0 | high |
| VM-014 | **Snapshot set** of an instance at a callback boundary (callbacks never yield, so frames and `pc` are empty there): the class-variable block; the callback return register; the native result register; the **native argument buffer** (contents and fill count); the **current script-parameter buffer** (contents and logical length — it survives a callback return with unconsumed contents, VM-013, and the next callback's parameters are *appended* to it before the frame captures it, VM-090); the global block. None of the retail files leaves either buffer non-empty at a callback end, but the state is not disposable in general. Restoring this set at a boundary reproduces the following callbacks exactly (acceptance case 17). | inferred | 006390b0, 00634c30, 006392e0, 0063a250, 0063a320, 00635210, 00635240 | high |

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
| 0x20 / 0x21 / 0x22 / 0x23 | float `+`, `-`, `*`, `/`: computed on the x87 with the process's precision control (the runtime's default 53-bit, or 64-bit; the program never narrows it below 53 — no control-word import and no such store in the game code) and stored as single. **The stored single equals the correctly rounded single of the exact result** for every normal result: for these four operations on single operands an intermediate of at least 2 × 24 + 2 = 50 bits makes the second rounding innocuous, and both 53 and 64 qualify (a product of two 24-bit significands is even exact in the intermediate). The one theoretical exception is a result in the single **subnormal** range, which no retail operand pair can produce (their float operands are small integers converted by 0x18 and literal constants). An implementation computing directly in `f32` is therefore bit-identical (rev. 10). | VM-066 | 00636f80, 006371b0, 006373e0, 00637610; 00642b7c (the only control-word change in the game code: the truncating conversion, restored on exit) | high |
| 0x24 … 0x29 | int compare → `1` / `0`: `<=`, `<`, `>=`, `>`, `!=`, `==` (signed). | VM-067 | 00637840, 00637a40, 00637c40, 00637e40, 00638040, 00638240 | high |
| 0x2A … 0x2F | float compare → **float** `1.0f` / `0.0f`: `<=`, `<`, `>=`, `>`, `!=`, `==`. **Unordered** (a NaN): `<=`, `<`, `==` yield `1.0f`; `>=`, `>`, `!=` yield `0.0f`. | VM-068 | 00638440, 00638650, 00638860, 00638a70, 00638c80, 00638e90 | high |
| ≥ 0x30 | Error (reported); `pc` unchanged: the original loops forever. | VM-069 | 00639370 | high |

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-070 | The two retail `0x0E` with `a = b = 0xFFFF` (H10, two zone classes, `EnterZone`, after native 202) jump to `pc = 0xFFFFFFFF`: an unchecked fetch before the instruction array. The script's evident intent is to end the callback (its other paths end with return value 1). Implementation choice 8.1: the current frame is popped as by 0x06, and what follows is decided by the **return address saved in the popped frame** — a script-call frame (0x05) holds its caller's return pc, an **engine callback frame holds the sentinel −1** whether it is the instance's only frame or a nested callback pushed on top of an outer callback's frames (VM-095); popping a sentinel frame ends that interpreter invocation, after which the enclosing native-call instruction (if any) resumes the outer callback. | observed (fact) / inferred (intent) | 00639370, 00639300, 0063a250, 00634dd0, 0063a320; data | high / medium |
| VM-071 | The result slot read by 0x0A holds whatever the frame's allocation left there unless a 0x07 wrote it (VM-050): the original's value is **undefined**; in all 62 retail uses the 0x0A directly follows a 0x05 whose target returns a value. **Unwritten-read policy** (departure, 8.1): OpenSherwood creates every frame with the slot equal to 0 and reads 0; a strict mode may fault instead. Mapped as `Assumption::UnwrittenResultSlot`. | inferred | 00634d30, 0063a250; data | high |
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
| VM-087 | Arity-0 natives — 37 ids: 23, 29, 30, 31, 32, 40, 54, 55, 74, 75, 106, 111, 119, 120, 121, 122, 147, 148, 159, 163, 167, 170, 171, 172, 173, 174, **192**, 211, 216, 234, 236, 238, 239, 245, 249, 251, 261 (192 was missing from the list before rev. 10; its wrapper is a plain tail-jump to the native and pops nothing) — take no cell; pushed cells for them stay in the buffer; a persistent imbalance overflows the 12-cell buffer (unchecked write). The retail files are balanced for every id. | observed | wrappers | high |
| VM-088 | Two buffers persist across callbacks and are never cleared by the engine: the **native argument buffer** (cells and fill count; a wrapper removes exactly its arity, so the buffer stays balanced only when the program is) and the **current script-parameter buffer** (VM-013: a frame creation captures the current buffer into the frame and installs a fresh one; a pop *discards* the captured buffer — nothing is restored — and the callee's buffer remains current). Consequently a residue left by a callback is still the current buffer when the engine starts the next callback on that instance, and the engine's parameters are appended after it (VM-090): the residue becomes parameter 0 and the engine's parameters are shifted. A script call made from inside that callback captures the callee's buffer and starts fresh, so it is not shifted. A nested engine callback (VM-095) behaves the same way on its own instance. The retail files leave both buffers empty at every callback end; a program that does not must have them snapshotted (VM-014), and their residue consumed as here (case 17). | observed | 006390b0, 00634c30, 006392e0, 00635210, 0063a250, 0063a320, 0063a400, 00634eb0 | high |
| VM-089 | **Failure classes** of native calls (each row of section 6 names its class): **(E)** reported to the log, the row's failure value returned, the script continues; **(E+)** reported but the operation *still proceeds* wholly or partly (rows say which part); **(U)** unchecked memory access; **(T)** an arithmetic trap (161 with `n = 0`; opcode 0x1C); **(X)** a C++ exception thrown by a bounds-checked container (native 168: its index is compared as *unsigned* with the list size, so negative indices also take this path; the exception unwinds the native — not the engine's fatal-error routine); **(F)** the engine's fatal-error routine (two scroll callbacks overlapping, VM-094); **(C)** a null dereference crash (a callback name that the class lacks, VM-090); **(N)** non-termination (opcodes ≥ 0x30). Opcode 0x00's continuation is an unchecked fetch (VM-040), not necessarily a hang. Implementation choice 8.1 gives each of U / T / X / F / C / N a deterministic, recorded outcome and says per class whether the callback continues or terminates. | observed | 00571590, 00571760, 00570e20, 00578680, 00579350, 0057c710, 005f8030, 00639300, 00639370, 00634bf0 | high |

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
| VM-100 | **Clock — three layers.** *(a) Executable:* the level advances at most once per main-loop iteration ("level tick"); in the active-window branch, when the game is not paused and the level's presentation flag is clear (ANIM-521 names the two flags), the loop busy-waits until the host's *reported* millisecond counter shows at least **40 ms** (400 ms in a debug slow-motion mode) past a reference sample — a sample that is taken again on the active-window path, so the threshold bounds that path's interval, not exactly the whole iteration; there is no catch-up; the wait is skipped when either flag is set. The executable fixes the **thresholds**, not the frame length. *(b) Measured reference cadence:* on the host of the oracle recordings the realised cadence is **46.875 ms** per iteration (21.333 per second) — the movement spec's ANIM-002, which also states why this is a well-supported value for that host and not a consequence of the code (counter granularity, phase, work per frame and scheduling all enter). At that cadence 25 executed ticks span **1.171875 s** and 75 span **3.515625 s**, absent skipped or delayed iterations. No claim is made for other hosts. *(c) OpenSherwood:* one logic frame of **46.875 ms** for everything counted in ticks, by decision `ADR-0010` (8.1); the program keeps no separate animation clock. | observed (a) / inferred (b) | 0050f710 (a); ANIM-002 (b); ADR-0010 (c) | high (a) / medium (b) |
| VM-101 | The tick is **attempted** every iteration and skipped when: the game is paused; a modal window object is open; the game state is one of the two "leaving the level" states. Text pages and dialogs are not this window: they are synchronous loops inside the native / element that shows them (VM-218). While the tick is skipped the *camera update* still runs on every unpaused iteration (VM-219). | observed | 0050f710, 005105d0, 004c8380 | high |
| VM-102 | **Tick counter** `T`: 0 at level start; incremented at **step 5** of every executed tick (VM-103); never reset. Snapshot: `T`, the force flag, the won flag and its sub-flag, the success-end / abort / failure-end flags, the debriefing index. | observed | 004c6ef0 | high |
| VM-103 | **Order within one executed level tick** (script-relevant steps; "ends" = the tick returns and later steps are skipped): **(1)** if the victory sub-flag is set and no player character has left the map yet: clear the sub-flag and show the "you may leave" notice (before any terminal check). **(2)** if the *success-end* flag is set: level `Finalize(0)` (scripts enabled), the tick ends with code 2 and the level ends; else if the *abort* flag is set: end bookkeeping (VM-103b), ends with code 1; else if the *failure-end* flag is set: `Finalize(1)`, ends with code 3. **(3)** pending one-shot HUD updates. **(4)** if `T mod 25 == 0`: level `Hourglass(T / 25)`; then, if `(T / 25) mod 3 == 0` **or the force flag is set at that moment** (a native 29 called inside this `Hourglass` counts): clear the force flag, run `CheckVictoryCondition(T / 25)`; result 1 with the won flag clear → **declare victory** (won flag := 1; sub-flag := 1 unless the campaign node is of type 3 or 6, in which case an interface-mode switch not read here follows, 9.3; no immediate end); result 2 → end bookkeeping (VM-103b) and **the tick ends** (code 1) without incrementing `T`. **(5)** `T += 1`. **(6)** if any of three *transition flags* (set by screens that suspend play; not read, 9.3) is set → ends (code 0). **(7)** unless the debug "no defeat" toggle is set — **defeat checks**, each running the end bookkeeping and ending the tick (code 1): no player character that is present and has not left → end; a captured player character → end; a dead civilian without the "may die" mark → end. **(8)** per-element updates in element-table order (actors dispatch `ActionChange`, scrolls count toward their `Hourglass`, movement, AI …). **(9)** queue drain (VM-215). **(10)** timer pass (VM-221). | observed | 004c6ef0 | high |
| VM-103b | **End bookkeeping**: computes the end statistics into campaign counters, then: won flag set → success-end flag := 1 (next tick: `Finalize(0)`, code 2); else → failure-end flag := 1 (`Finalize(1)`, code 3). The end triggers are therefore: a `CheckVictoryCondition` result of 2, the abort flag, and the three defeat checks — each ending in success **iff** a victory had been declared before it fired (by result 1 or by native 178). A declared victory by itself ends nothing: the mission ends when one of those triggers fires (typically the first defeat check, once every player character has died or left the map). | observed | 004e3260, 004e3220, 004e3cd0, 004c6ef0, 005795c0 | high |
| VM-104 | **Initialisation order.** While the mission file is read, every scripted actor, object, waypoint and script zone binds its class and runs `Initialize` at once, in record order (the retail ones are stubs). Player characters run `Initialize` when created. Just before the play loop: the level's `Initialize(0)`, then every scroll's `Initialize` in scroll-table order (current scroll set). The level's `PostInitialize` runs once on the first loop iteration, after the first *attempted* tick, if the class has it. | observed | 0046e7e0, 004bb820, 00551bb0, 0057f8c0, 004a0f10, 004c3740, 004ba000, 004b9fa0, 0050f710 | high |
| VM-105 | **Scroll `Hourglass`**: each active scripted scroll counts its own updates; at 25 it runs `Hourglass(0)` and resets. | observed | 004b9f40 | high |
| VM-106 | **Scroll `IsTaken`**: the "take scroll" element (an engine-originated pickup order, executed by the level executor at dispatch) calls the take routine with the taking actor and then marks itself *done* **unconditionally** — its completion never depends on the callback. Inside the take routine, in order: status := 3; the pickup sound; then, only for a scripted scroll: the current scroll is set (VM-094 — the overlap check is here, *after* the status and the sound), `IsTaken(actor)` runs, the current scroll is cleared; a non-zero result → status := 2 and the visibility refresh hides the scroll; zero → status stays 3 and nothing else happens. | observed | 004ba4e0, 004ca410, 004ba760, 004ba5c0, 004ba7e0 | high |
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
| VM-120 | A message is `(message, arg1, arg2)`; the target is an actor-family element or **null** = the level class; any other target → error, dropped (E). No queue: a recorded message is delivered synchronously when its level is dispatched (VM-095, VM-212 b). Natives 43 / 44 *record* it (error and no delivery outside a recording); **109 / 110 deliver it synchronously inside the native call** — the target's `ProcessMessage` has returned before the native returns, and the delivery has no lifetime of its own afterwards. | observed | 00578cb0, 00578e50, 00578d80, 00578f20, 004ca410, 00467230 | high |
| VM-120b | Natives 154 / 153 fire `EnterZone` / `ExitZone` only for an actor of the element table that is already a member of the zone; an absent actor → error, nothing. | observed | 00577390, 00577220 | high |
| VM-121 | 44 / 110 pass `(message, a, b)`; 43 / 109 pass `(message, 0, 0)`. No delay parameter. | observed | 00578e50, 00578cb0 | high |
| VM-122 | Message ids are opaque integers; the engine originates only message 1001 (VM-113); native 71 requires `message >= 1000`. | observed | 00573f00 | high |

### 3.7 Sequences

A *sequence* is an ordered list of *elements*, each tagged with a *level*; it runs level by level: all elements
of a level are dispatched together and the next level starts when every element of the current level has
finished. Recording (natives 30 / 31 / 32 and the recording natives) builds such a list.

| Id | Claim | Status | Address | Conf. |
|---|---|---|---|---|
| VM-200 | **Recording state** (global, one recording at a time): the sequence being recorded (null = none), the *current level* (a 16-bit counter, 0 = not recording), and two "entered actors" lists (natives 46 / 47 / 63); OpenSherwood adds the provenance triple of 8.3 (no counterpart in the original). Native 30: a recording already open → error, returns 0, no change; else: new empty sequence, level := 1, lists cleared, returns 1. **The recording state persists across callbacks**: native 30 returns with the recording open, and nothing closes it until native 31 (the retail files always close it within the same function, `docs/formats/scb.md`). Snapshot: the recording state belongs to the snapshot set (8.3). | observed | 00570f00 | high |
| VM-201 | Native 32: not recording (level counter 0) → error, 0. Else, if the sequence has an element **and the last recorded element's level equals the current level**, level += 1; returns the level. Two consecutive barriers count once. **16-bit wrap** (original): starting at level 1, the **65,535th** increment turns the counter to 0 while the recording object stays open. Three guards then matter, and they differ per native (every recorder checked individually): **30** tests the recording object → refuses (error, 0); **31 / 32** test the level counter → "not recording" (error, 0); **with a preliminary level-counter guard** (error, nothing built at level 0): 33, 34, 35, 36, 42, 45, 49, 50, 51, 54, 55, 59, 61, 62, 63, 69, 212, and 64 through 45; **without one** (only the common append step, which tests the recording object and therefore **appends at level 0**): 37, 38, 39, 40, 41, 43, 44, 48, 52, 53, 56, 57, 60, 65, 67, 68, 70, 71, 72, 73, 203, 226, 243. The counter is also advanced **inside the compound walk recorder** (VM-203), so a wrap can be produced by a long walk as well as by native 32; the original tags the following elements with level 0 in both cases. Whether a level-0 tail can ever be launched is not established (31 refuses); the state is reachable only by a script that loops over recorded elements and barriers. **OpenSherwood** (departure, 8.1): a deterministic fault on any attempted transition of the counter from 65,535 to 0, whether by native 32 or by the compound recorder. | observed | 00571100, 00570f00, 00571020, 0057a020, 00575350, 00578cb0, 00582640, and the recorders' addresses in section 6 | high |
| VM-202 | Native 31: not recording → error, 0. Else: level := 0; the entered lists cleared; an empty sequence → error, discarded, 1; else handed to the manager and **launched at once** (VM-210), 1. | observed | 00571020 | high |
| VM-203 | A recording native builds one or more elements, each tagged with the current level, and appends them to the open recording. The **compound walk recorders** — 45, 212, 64 (through 45), 47 and 63 — go through the walk recorder that adds door passages, waits and turns as the navigation spec's NAV-130 describes: a same-zone path yields **one** walk element at the current level; a longer construction yields **several** elements and **advances the recording's level counter between its groups**, so those elements occupy successive levels and the recording continues at the last of them (a following native 32 counts from there). **46 records exactly one walk element**; 57 / 70 / 71 record one seek element each; every other recording native records one element. Outside a recording the common append step reports an error and discards the element (E) — but effects a native performs *before* its append are not undone, and three natives differ: **46** registers the actor in the entered lists and places it first, then uses the checked append (E+: placement kept, walk dropped, result 0); **47** registers the actor in the entered lists first, then records its walk through a path that appends **without** checking the recording (U: an append through a null recording); **63** tests the level counter *before* anything else (E: no registration, nothing). | observed | 0057a020, 00577650, 005779a0, 00574a70, 00582640, 00585500, 00573d10 | high |
| VM-204 | An element carries the effect requested by its native (its category: camera, timer, message, page, walk, animation, …), its target and the native's arguments; the category decides who executes it and when it completes (VM-212, VM-218, VM-231). | observed | the natives of section 6 | high |
| VM-210 | **Launch**: the sequence is appended to the manager's list and its first level is *dispatched* (VM-212): immediate categories execute now, in element order; the others become *pending* in the FIFO queue. A pending element becomes *running* only when its executor admits it. Effects that the engine itself originates — the damage of native 102, the messages of 109 / 110, pickups, AI-issued orders — are dispatched as **independent elements with their own lifetime** (not part of any script recording) under the same immediate-versus-queued rule: a message runs at once (VM-120), the damage of 102 is *queued* and applied at the next drain in FIFO order with the other queued elements. | observed | 0058a3d0, 00582530, 00582560, 0058a940 | high |
| VM-211 | **Level completion**: a sequence counts the elements of its current level not yet *done*; an element reaching *done* decrements it; at zero the next level is dispatched **synchronously at that point**. After the last level nothing happens; housekeeping (every 256 ticks) deletes a sequence with no running or pending element. | observed | 00582620, 00582560, 005823f0, 0058a430, 004c6ef0 | high |
| VM-212 | **Dispatch of a level**: its elements are visited in order; a *cancelled* element is skipped; each other element is (a) executed at once by the level executor if its category is level-immediate — lock / unlock player input (54 / 55), camera jump (34), timer (56), PC action availability (37), character availability (38), take-scroll; (b) a message (43 / 44): executed at once by the target actor's message path, or by the level executor for a null target; (c) handed at once to the target actor if actor-immediate — lock / unlock AI (52 / 53), un-blip (243), animation table swap (60 / 61), mobile-element start / stop / activate / deactivate (67 / 68 / 72 / 73), speak (62 / 69), an engine-internal movement variant; (d) otherwise appended to the manager's FIFO queue as *pending* (camera scroll 33 / 42, zoom 35, map 36, dialog 41, camera lock 39 / 40, page 203, freeze-all 226, walks, seeks, turns, animations 49 - 51, actions 59, damage 102, corpse 63 / 65). | observed | 00582560, 0058bb80, 00585b70 | high |
| VM-213 | **Re-entrancy during dispatch**: a callback run by an immediate element (b) may launch sequences, complete elements or cancel elements. Ordering: the nested launch dispatches *its* first level completely (its level-immediate elements execute — a timer enters the timer list, a message runs — and its queued-category elements are appended to the FIFO) **before** the outer dispatch continues with the next sibling; FIFO entries are ordered by the moment they were appended, so the nested launch's queued elements precede the outer level's later siblings; a sibling cancelled by the callback before its turn is **skipped** (VM-212) — but the level's completion count was already charged for it when the level was dispatched, and cancellation never decrements that count, so **the level cannot complete** (VM-217); a cancelled or refused element notifies its executor synchronously if it has one (an actor already holding it drops it as the actor's rules say — sibling dependency). Setting an element to the state it already has is a no-op (no second completion, no propagation, no notification). | observed | 00582560, 00582620, 00585320, 0058a3d0, 00585b70 | high |
| VM-215 | **Queue drain** (step 9): FIFO until empty; each element removed and, if still pending, given to its target — an actor target's admission (VM-216), a null target → the level executor (VM-218). Elements appended during the drain (callbacks, level completions, nested launches) are drained in the same pass, after the entries already queued. | observed | 0058ba60, 00585570, 004c6ef0 | high |
| VM-216 | **Actor admission**: the actor's admission tests decide (its own state and the element's parameters; the locked case is AI-082 (a); the other predicates are **open**, 9.1). Refused → *refused* with the "next level" propagation (VM-217). Accepted and idle → current action, *running*; accepted and busy → merged / queued by the actor's own rules (**open**). The actor reports *done* when the action ends (VM-231). | observed (structure) / unknown (predicates) | 004646e0, 0046b210 | medium |
| VM-217 | **Abort cascade**: a *transition* to *refused* or *cancelled* carries one of three propagation modes: **next-level** (used by the actor's refusal and cancellation): the first element of the next level is set to the same state in **chain** mode; **chain**: the following element of the sequence is set likewise, so every later element, whatever its level, ends in that state; **none**: no propagation. Same-level siblings of the refused element are not touched and finish normally. Only a transition to *done* from a dispatched state decrements the level's completion count; a refused / cancelled element never does, so its level never completes and the remainder is dropped (deleted by housekeeping). A transition to *refused* or *cancelled* also cancels the element's sub-elements and notifies its executor synchronously; a *done* transition never propagates. | observed | 00585320, 00582620, 004646e0 | high |
| VM-218 | **Level executor** (completion of its categories): lock / unlock player input — HUD events, *done* at once. Camera jump — the camera corner set (VM-219), *done* at once. **Camera scroll** (33 / 42) — becomes the *camera element*; *done* when the camera update finishes the scroll (VM-219), or at once when the level's "no cinematic camera" flag is set; a new camera element marks the previous one *done*. **Zoom** (35) — sets the zoom request, becomes the camera element; *done* when the zoom reaches the request (VM-219). Map display (36) — at once. **Timer** (56) — appended to the timer list (VM-221). **Dialog** (41) and **page** (203) — unless the "no presentation" flag is set, a **synchronous modal loop** shows it and returns only when the player dismisses it (the whole program is suspended inside: no ticks, no camera update, no other sequence); then *done*, then a HUD event. Camera lock / clear (39 / 40) — at once. Message — the level's `ProcessMessage`, at once. Freeze-all := bool (226) — at once. PC action availability (37), character availability (38) — HUD events, at once. Take-scroll → `IsTaken` — at once. Native 202 runs the same modal loop **inside the native call**; native 17 likewise for a dialog. | observed | 004ca410, 0053a1a0, 0053a0b0, 0053a4c0, 0052b100, 00579f30, 005714a0 | high |
| VM-219 | **Camera update** (once per unpaused main-loop iteration, after the tick step; the contract is the movement spec's ANIM-320 / 322 / 330 / 333 at the pinned revision — this row states only what the VM depends on). While a scroll destination is set, each update **first** tests arrival: reached → the destination is cleared, the step length reset to 1.0, the **cached view invalidated**, and the camera element (if any) is *done* — so a scroll that lands exactly completes on the **following** update; otherwise the corner moves toward the destination by `L` = the scroll **speed** when non-zero (native 42's value) else the scroll **step length**, which is then refreshed from the ramp (ANIM-330); a step clipped at the map border terminates the scroll the same way (ANIM-322). Zoom: **first** if the requested zoom equals the current one the request is cleared and the camera element is *done*; else one zoom step is raised (ANIM-333; the number of updates an accepted change takes is **unknown** there). These completions, and the level completions they trigger, are an execution opportunity **outside the tick phases**. What the natives write directly (rows 18 - 21, 145 / 146): 18 / 19 the raw scroll target, the converted destination and the step length — never the corner, never the speed or the ramp position, and they leave a **running camera element** in place (a scroll element of 33 / 42 that is still the camera element is completed when the arrival test succeeds for the destination 18 / 19 substituted); 20 the corner and the cached-view invalidation — not the actor lock, not the destination; 21 the requested zoom; 145 / 146 invalidate the cached view. **Shared destination conversion** (used by 18, 19, 20 and the scroll / jump elements; the movement pin does not state it, so it is this spec's claim, confirmed against the raw instructions). *Centring dimensions* and *clipping dimensions* differ. Centring: `x0 = trunc(point.x − screen width / (2 · zoom))`, `y0 = trunc(point.y − screen height / (2 · zoom))` — the **full** screen height, truncated toward zero to whole pixels (at 640 × 480, zoom 1, the point (1000, 1000) gives (680, 760)). Clipping: the viewport `W = screen width / zoom` by `H = (screen height − 80) / zoom` (ANIM-320). Per axis, x first: the sign of the **truncated** value is noted and a negative value is raised to 0; then, if `value + viewport > map size` on that axis (`W` for x, `H` for y): when the truncated value was negative the conversion **falls back** — current zoom := 1.0, destination := the map origin — and the rest of the conversion is skipped (after the x fallback the y axis is not processed; after the y fallback the alignment step is skipped); otherwise the value is clamped to `map size − viewport`. Finally, at zoom 0.5 only, an odd value on either axis is reduced by one. The fallback is therefore the one path on which 18, 19 **and 20** change the zoom. Later effects: a step length of exactly 1.0 (native 19) resets the ramp index on every update in which it is in force (ANIM-330), so 19's influence is not confined to the first displacement. | observed | 004cdfc0, 004c8380, 005105d0, 005758d0, 00571270, 00571330, 005713f0, 00571200, 00570e30, 00570e50 | high |
| VM-221 | **Timer pass** (step 10): the timer list is visited in insertion order over the count captured at the start of the pass; an element whose counter equals 1 is *done* and removed; otherwise `counter -= 1` (32-bit, wrapping). A timer inserted earlier in the same tick is visited in that tick's pass; one inserted *during* the pass is first visited next tick. Timers completing in the same pass complete in list order, each running its level completion synchronously before the next is visited. A timer recorded with counter `n` completes on visited pass number `((n − 1) mod 2^32) + 1`: `n >= 1` → the `n`-th pass; `n = 0` → the 2^32-th pass; a negative `n` → `(n mod 2^32)`-th pass (e.g. −1 → the (2^32 − 1)-th). OpenSherwood counts identically (8.1: no departure). | observed | 004c6ef0, 00575350 | high |
| VM-222 | Inside a modal loop nothing advances; while the tick is skipped the camera update still completes camera elements (VM-219). | observed | 0050f710, 004cdfc0 | high |
| VM-231 | **Actor-side completion** (per the movement spec's element table, draft): actor-immediate categories (VM-212 c) are *done* as soon as the actor executes them; **speak** (62 / 69) when the speech ends (ANIM-140, inferred there); **walks**: a move action completes on the ordinary arrival rule (ANIM-208, a directional projection — not read here), the **approach** element of NAV-130 on the Chebyshev test `max(|dx|, |dy|) < tolerance + 5` px (NAV-150 a), and an empty search result fails the element 100 ticks after it arrived (NAV-146) — all three gated by the navigation redo; **seek** (57 / 70 / 71) on arrival, or at once when the seeker is the target; **animation 49** completes after the clip; **animation 50 (loop) never completes** — its level never finishes and every later level is blocked; **animation 51**: when the clip ends the actor **first** issues a hold-last-frame request as a *queued* (deferred) element and **then** reports the animation element *done*; the sequence's next level is therefore dispatched **before** the hold is admitted (the hold waits for the next drain, VM-215), so an element that next level hands to the same actor can be admitted ahead of it, and the hold itself is subject to the actor's admission (VM-216); the actor is observed holding the last frame **only if** the hold is admitted and nothing replaces it — the hold blocks no sequence; **actions of native 59**: end of the action (**open**); corpse 63 / 65 (**open**); the admission and priority rules (**open**). | inferred (from the sibling drafts) | 00467230, 004646e0, 00464b20 | medium |

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
| VM-071 (result slot undefined when unwritten) | inferred | `UnwrittenResultSlot` (OpenSherwood reads 0; never exercised by the retail files) |
| VM-100 (b) measured reference cadence | inferred (host-dependent) | none: the OpenSherwood frame is the decision of ADR-0010, not an assumption (8.1) |
| VM-103 step 4 / VM-105 / VM-107 — the engine's interim scheduling while 3.5 is uncleared (`Hourglass` on every class every frame with the frame counter, instead of the level's every 25 ticks with `T / 25` and the scrolls' own counter) | observed here; departed from by batch 1 | `SchedulerCadence` (rev. 10): one named assumption covering every scheduling rule of 3.5 the engine has not yet implemented; removed by the batch that implements 3.5 |
| natives 4 / 5 / 8 / 9 / 11 / 12 / 15 / 16 — handles of doors, patches, buildings and patrol paths represented as their positions in the map's flat lists, the inverse natives as the identity | inferred (observationally equivalent iff the lists are the file-order flat lists of `docs/formats/rhp.md` / navigation section 5 and every other source of such handles — 64's door search, 152 / 156 / 182 - 191 — draws from the same lists) | `Policy(id)` until the navigation redo pins the list definitions (NAV section 5, NAV-172); then a dependency, not an assumption (rev. 10) |
| VM-107 (objects' `ActionChange` dispatcher) | inferred | `ObjectActionChange` |
| VM-108 (b): meaning of events 100..106 and the stored element of 104..106 | unknown | `AiEventCode(100..106)` |
| VM-109 (patrol-node variant) | inferred | `ReachPointSource` |
| VM-216 admission / priority predicates | unknown | `ElementAdmission` |
| VM-231 durations of 59 actions, 63 / 65, speech detail | unknown / inferred | `ElementDuration(category)` |
| natives with an unresolved effect, flag or consumer (4.3, 18 ids) | observed write / unknown effect | `UnresolvedEffect(id)` — the value is passed to the consumer verbatim; fidelity is claimed only once the consumer's spec names it |
| natives excluded from clearance (4.2) | unknown | `UnknownNative(id)` with the placeholder of 4.2 (an explicit choice, not a fidelity claim) |

### 4.2 Natives excluded from clearance (12)

Called by the retail scripts: 13 (25 calls), 91 (1), 92 (1), 173 (14, result read), 224 (159, result unused),
241 (1), 261 (1); unused: 157, 190, 225, 227, 238. Their semantics are not settled; they map to
`UnknownNative(id)` and the implementation must make the placeholder an **explicit choice** recorded as an
assumption: 13 → the inverse of native 6 (the scripts' evident intent); 91 → 0; 173 → 0; 224 → 0 (the repulsive
point *is* created, 224's effect is observed, its result is not); 238 → 0; 261 → 0; 92, 157, 190, 225, 227, 241
→ no-op. A strict mode may instead raise a deterministic "unresolved operation" fault on any of them — as an
**opt-in for tests and analysis only**: the retail scripts call 224 at every forest mission's start and 173 in
fourteen missions, so the shipped campaign is playable only with the placeholders (rev. 10; question 7).

### 4.3 Natives whose effect is settled only up to an unresolved code, flag or consumer (18)

Observing a write does not settle its effect. Each row states the **cleared request boundary** — what the native
verifiably validates, computes and hands over, which an implementation may build now — and what stays unresolved
and would be settled by which consumer claim. Nothing in this table is executable semantics for the consumer.

| Id | Cleared request boundary (what the native does) | Unresolved | Consumer that would settle it |
|---|---|---|---|
| 45, 212 | validation; the walk elements of NAV-130 recorded; modes 2 / 3 additionally set a variant on those elements | the variant's effect on the walk | the actor's walk runner (navigation; NAV-130 does not name it) |
| 46 | validation; list registration with `loc` as entry; one walk element; the position taken from the entry geometry of (`loc`, `dir`) | the entry-geometry rule; which further actor fields the placement writes | the map's entry-placement rule (navigation) |
| 47 | validation; list registration (supplied location as entry, current position as origin); the compound walk; the final walk element | the entry geometry; the compound recorder's added elements | navigation NAV-130 |
| 57 | **no validation** of `actor` or `target`; builds one seek element from (`actor`, `target`, gait from `k`, `v`) and appends it through the checked append | the parameter `v` (the fourth argument, stored on the seek element) | the actor's seek runner (movement element table, kind 0x15; the row there does not name it) |
| 70, 71 | validation of the reporter only (null or actor family); one seek element from (`actor`, `target`, gait from `k`, the fourth argument) with the attached message; 71 additionally requires `msg >= 1000` | the fourth argument: the **same seek parameter** as 57's `v` (stored in the same field of the element); "range" was this spec's guess, not an established meaning | the actor's seek runner |
| 59 | validation of code and argument as in its row; one action element with (code, arg) | the action each code requests; its completion | the actor's action runner (AI §6.1 contradicts, 9.4) |
| 62 | validation (PC); one speak element with text id and flag | the flag | the speech element (movement ANIM-140) |
| 63 | the level guard; validation; list registration; the walk; one take element | the take element's completion | the actor's take runner |
| 100 | validation (known element); returns whether the element's movement-style value equals the third of its values | which style that is | the movement style consumer (movement / AI: AI-093 names walk 0 and run 1 only) |
| 106, 107 | 106 returns a HUD state byte; 107 sets it and raises a HUD event when it changes | which HUD state the byte is | the HUD / campaign spec |
| 136 | validation as in its row; stores (code, permanent-or-temporary) on the soldier | the decision each code selects | the AI's battle decision (`spec-ai-combat.md`) |
| 165 | validation (PC); team membership appended without duplication; HUD refresh; marks the character present | the two further per-character writes (campaign CAMP-310 lists three unidentified writes including the present mark) | the campaign / character state (`spec-campaign-camp-saves.md`) |
| 254 | a per-element flag := `b` | the flag | not identified |
| 259, 260 | 259 returns the action-state value or 666; 260 validates 0..17 and stores it | the eighteen states' meaning | the actor's action state (AI / movement) |

Cleared only together with the sibling claim that names the consumer (`UnresolvedEffect(id)`).

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| guarded-wait threshold on the reported counter | 40 | ms | 0050f710 | high |
| slow-motion threshold (debug) | 400 | ms | 0050f710 | high |
| measured reference cadence (oracle host) | 46.875 (three 15.625 ms counter steps) | ms per iteration | ANIM-002 (movement spec `b0cd053`, draft); OpenSherwood's logic frame by ADR-0010 | medium |
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
| scroll step length written by 18 | 2.0 (19: its float argument); reset to 1.0 at scroll end | map px per camera update (used only while the scroll speed is 0) | 00571270, 00571330, 004cdfc0 | high |
| remark id limit (69, 264) | 120 (ids 0..119) | remark id | 00572ef0, 00573010 | high |
| door search radius of 64 | 300 (squared 90000) | map px | 00574860 | high |
| script-visible campaign values (195 / 196) | 20 (k in 0..19) | values | 00579430, 00579470 | high |
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
| 18 | `loc` | bool | **Scroll request** (ANIM-331): raw scroll target := point `loc`; scroll destination := the converted point (VM-219: viewport centred on it, truncated, bounded and aligned for the current zoom, with the zoom-reset fallback); **scroll step length := 2.0 map px per camera update**. Nothing else is written: the camera does not move in the call, no element is created, the scroll **speed** left by the last scroll element, the **ramp position** and the **current camera element** are preserved — so the 2.0 governs the first update only when that speed is 0, afterwards the ramp rule of ANIM-330 applies, and a still-running scroll element of 33 / 42 completes when the camera arrives at the *new* destination. The zoom changes only through the conversion's fallback (VM-219). Returns 1. Null → error, 0. | E (0) | 00571270 |
| 19 | `loc, f` | bool | As 18 with step length := `f` (same caveats; `f` = 1.0 additionally resets the ramp index on each update it is in force, VM-219). | E (0) | 00571330 |
| 20 | `loc` | bool | **Jump** (ANIM-332): camera corner := point `loc` converted by the shared conversion of VM-219 (immediately, before the next drawn frame); the cached view is invalidated so the next frame is fully redrawn. Touches neither the scroll destination, the step length nor the **actor lock**; the zoom changes only through the conversion's fallback. Returns 1. Null → error, 0. | E (0) | 005713f0 |
| 21 | `f` | bool | **Requested zoom** := `f` (only 0.5 / 1.0 / 2.0 accepted → 1; else error, no change, 0); the logical and displayed zoom change through later camera updates (VM-219, ANIM-325 / 326). | E (0) | 00571200 |
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
| 42 | `loc, speed` | bool | Records a camera scroll to point `loc` with an explicit **scroll speed** (unsigned 16-bit, px per camera update, used instead of the step length and the ramp while non-zero; ANIM-330) → 1. | E (0) | 00572c70 |
| 43 | `target, msg` | void | Records message `(msg, 0, 0)` to `target` (null = level; non-actor → error, nothing). | E | 00578cb0 |
| 44 | `target, msg, a, b` | void | Records `(msg, a, b)`. | E | 00578e50 |
| 45 | `actor, loc, mode` | bool | Records a walk of `actor` to point `loc` (NAV-130): mode 0 / 2 walk, 1 / 3 run; modes 2 / 3 additionally mark the added elements with a variant whose effect is unnamed (4.3). Not recording / null or non-point location / non-actor / bad mode → error, 0. Completion: VM-231. | E (0) | 005740b0 |
| 46 | `actor, loc, dir, b` | bool | "Enter the game" — **immediate effects first**: if `actor` is not yet in the recording's entered lists, it is added to both lists with `loc` as its entry, and its position is set from the **entry geometry** derived from point `loc` and the direction `dir` (−1 = the actor's own facing): the map's entry-placement rule (navigation dependency, 4.3) yields the position, and the position-dependent fields are refreshed; **which further actor fields the placement writes (facing, movement state) is not established** (4.3). If the actor is already listed, its stored entry is updated to `loc` and nothing is placed. **Then** exactly one walk element (walk if `b` = 0, run otherwise) is built and appended through the checked append. Outside a recording the list / placement effects still happen and only the walk is dropped (E+). Not an actor / not a point / bad `b` → error before any effect. **Returns 0 always** (the low byte is explicitly cleared). | E / E+ (0) | 00577650 |
| 47 | `actor, loc, dir, b` | bool | "Enter the game" walking from where the actor is: if the actor is already in the entered lists, its **stored entry** is the walk origin; otherwise the actor is added to both lists with the **supplied location** `loc` as its stored entry, while its **current position** is the walk origin (the two values differ: the entry is what a later 46 / 47 / 63 reads back, the origin is where the recorded walk starts). Two destinations: the compound walk (walk / run by `b`) is recorded **toward the supplied location `loc`** (one or more elements, VM-203); then a **final walk element toward the entry geometry** derived from `loc` and the facing `(dir − 8) mod 16` (`dir` = −1 → from the actor) is appended **without** the recording check. → 1. Outside a recording: lists modified, then an unchecked append (U). Not an actor / not a point / bad `b` → error, 0, no effect. | E (0) / U | 005779a0 |
| 48 | `actor, loc` | bool | Records "turn to face point `loc`" → 1. | E (0) | 00574e70 |
| 49 | `x, anim` | bool | Records "play animation `anim` once" on an actor or target object → 1; completes after the clip (VM-231). | E (0) | 00573190 |
| 50 | `x, anim` | bool | Records "loop animation `anim`" on any known element → 1; **never completes** (VM-231). | E (0) | 00573280 |
| 51 | `x, anim` | bool | Records "play `anim`, then request a hold on its last frame" (actor or target object) → 1. Completion: after the clip; the hold is a **request** the actor queues before reporting completion, honoured only if the actor admits it and nothing handed to the actor in between replaces it (VM-231). | E (0) | 00573370 |
| 52 | `actor` | bool | Records "lock the AI of `actor`" (known actor) → 1 (AI §6.1). | E (0) | 00575120 |
| 53 | `actor` | bool | Records "unlock the AI" → 1. | E (0) | 005751d0 |
| 54 / 55 | – | bool | Records "lock" / "unlock player input" → 1; not recording → 0. | E (0) | 005753d0, 00575470 |
| 56 | `n` | bool | Records a timer with counter `n` (VM-221); byte = the recording step's result. | E (0) | 00575350 |
| 57 | `actor, target, k, v` | bool | Records "seek actor `target`" (walk if `k` = 1 else run) with the seek parameter `v` (unresolved, 4.3); **no check** of `actor` or `target` before the element is built — a wrong `actor` is a *deferred* fault at hand-over (8.1), never a result of this native; byte = recording result. | E (0) / deferred | 00573d10 |
| 58 | `x` | bool | Always 0 (placeholder). | – | 006390a0 |
| 59 | `actor, code, arg` | bool | Records an action element of `actor` (known element) by `code` 0..20: the native validates the code and, for 4, that `arg` is a valid table index, for 5 and 8..16 that `arg` is an element; 17 / 18 require a soldier; 1 passes `arg mod 16` as a direction. Which action each code requests, and its duration, is **not settled here** (4.3: the consumer is the actor's action runner; AI §6.1 reads 59 differently, 9.4). Other codes / invalid `arg` → error, 0; else 1. | E (0) | 00573450 |
| 60 | `actor, a, b` | bool | Records "replace animation `a` by `b`" → 1. | E (0) | 00574f80 |
| 61 | `actor, a` | bool | Records "restore animation `a`" → 1. | E (0) | 00575050 |
| 62 | `pc, text, flag` | bool | Records "speak text `text`" (flag unnamed, 4.3) for a **player character** → 1; non-PC → error, 0. | E (0) | 005730b0 |
| 63 | `actor, corpse, b` | bool | Records the "take corpse" walk-and-take: **first** the level counter is tested (not recording → error, 0, nothing else); then `actor` must be a PC with one of the two carrying skills and `corpse` an actor (else error, 0); the taker is registered in the entered lists, a walk to the corpse (walk / run by `b`) is recorded, then the take element; → the append's result (1). | E (0) | 00574a70 |
| 64 | `actor, loc, mode` | bool | Walk into a building (NAV section 5): the nearest type-1 door within 300 px of point `loc` and native 45 recorded to it; no door / not a point → error, 0. | E (0) | 00574860 |
| 65 | `actor` | bool | Records "leave the carried corpse" (PC with the skill) → 1. | E (0) | 00574db0 |
| 66 | `x` | bool | Resets an animated map element's animation → 1; other → error, 0. Unused. | E (0) | 00570dd0 (medium) |
| 67 / 68 | `i` | void | Records "start" / "stop mobile element" for **player character `i`** (unchecked PC-list index). | U | 005783b0, 00578430 |
| 69 | `actor, k` | bool | Records "speak remark `k`" (0..119) for a human → 1; else error, 0. | E (0) | 00572ef0 |
| 70 | `actor, target, k, v, reporter, msg` | bool | Records "seek `target`" (walk if `k` = 0 else run) with the seek parameter `v` (the same unresolved parameter as 57's, 4.3) and an attached message `(msg, 0, 0)` to `reporter` (null = level) sent when the seek ends → 1; `reporter` not an actor → error, 0; `actor` / `target` unchecked as in 57 (deferred fault, 8.1: the attached message is then not delivered). | E (0) / deferred | 00573da0 |
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
| 100 | `x` | int | 1 if the movement-style value of known element `x` equals the third of its values, else 0 (which style that is: unresolved, 4.3); unknown → error 0. Unused. | E (0) | 00571550 |
| 101 | `x` | int | Current action id: target family → its action; actor family → its current action, 283 if none; others → error 0. | E (0) | 00578060 |
| 102 | `x, amount, b` | void | Damage `amount` on actor `x` of the element table, intensity flag 100 if `b` else 0; not applied in the call — **queued**, applied at the next drain (step 9) in FIFO order with the other queued elements (AI §6.1 for the damage itself). Not such an actor → error. | E | 00577500 |
| 103 | `x` | bool | **Stops** actor `x` (its current movement and order are cancelled, the stop the engine uses when a script halts an actor) → 1; non-actor / unknown → error 0. | E (0) | 00571b30 |
| 104 | `npc, x` | bool | NPC `npc` sees human `x` (AI-066). Unused. | E (0) | 00578160 |
| 105 | `npc` | void | Enable the view-cone display; a non-NPC is reported **but the call proceeds** (E+). Unused. | E+ | 005786b0 |
| 106 | – | bool | A HUD state byte (which state: unresolved, 4.3). Unused. | – | 00578820 |
| 107 | `b` | void | Set that HUD state byte := `b`, raising a HUD event when it changes (4.3). Unused. | – | 00578830 |
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
| 118 | `x, prop` | int | Get property (same numbering; 2 life as signed 16-bit; 4..11 as counters); errors → −1. The properties are **live fields of the element, initialised by the loaders, never "unwritten"** (rev. 10): **1 (money)** is loaded from the mission record — a soldier's from its `BORG` record's u32 at offset 0x23 (the field the AI spec's AI-043 reads as a loot threshold: same storage), a civilian's from its `OILE` record's u32 that follows the u16 after the profile index (`rhm.md` documents those six bytes as two i16; the loader reads u16 + u32) — and afterwards changed only by natives 117 / 229, by looting between NPCs and by the NPC's own spending; **0 (arrows)** is the quiver's count (0 without a quiver); **2 (life)** and **3 (concussion)** are the AI spec's health / stun state; **4 - 11** the player character's item counters and **12** its name preset (campaign spec). Evidence: Emb02's element 40 is its single civilian (16 objects precede it) with 4000 in that field, and its victory check tests that value for 0. | E (−1) | 00571fb0, 004a1e90 (bytes), 004705f3 (bytes), 004863f0 |
| 119 | – | bool | Some civilian in the level is dead. | – | 005761f0 |
| 120 | – | bool | Some soldier is dead. Unused. | – | 00576260 |
| 121 / 122 | – | int | Highest alert state (0..2) over living soldiers / over living non-soldier NPCs. Unused. | – | 00576a30, 00576af0 |
| 123 | `npc, s` | bool | Alert state := `s` (0, 1, 2) → 1; PC / other / bad `s` → error 0. | E (0) | 00575af0 |
| 124 | `npc` | int | Alert state 0..2; non-NPC → error 0. | E (0) | 00575bf0 |
| 125 | `npc, s` | bool | Set the AI's state (AI-041): 1 → default, 3 → seeking (soldiers only; civilians → error 0), 5 → fleeing, 7 → return to the post; 0 (sleeping), 4 (menacing), 6 (attacking) cannot be set → error 0; other → error 0. | E (0) | 00575c80 (medium) |
| 126 | `npc` | int | The NPC's top-level AI state (AI-041) as a script code: 1 default, 2 wondering, 3 seeking, 4 menacing, 5 fleeing, 6 attacking, 0 sleeping / unknown; non-NPC → error 0. (AI §6.1 agrees; review 22 upheld the order.) | E (0) | 00575e20 |
| 127 | `a, b` | bool | Obsolete: always error, 0. | E (0) | 00575ee0 |
| 128 | `npc` | int | 1 if the NPC is hostile (its attitude), else 0; non-NPC → error 0. | E (0) | 00575f00 |
| 129 | `npc, a, b` | bool | No effect; 1 for an NPC, error 0 otherwise. Unused. | E (0) | 00575f70 |
| 130 | `npc, target, b` | void | Validates `npc` (NPC) and `target` (known element); the operation itself is **empty in this build**. | E | 00575fc0 |
| 131 | `npc, loc, b` | void | Validates (NPC, point); empty operation. Unused. | E | 005760b0 |
| 132 | `npc, path` | void | Assign the patrol path (AI 3.4); non-NPC → error. | E | 00576c50 |
| 133 | `npc, loc, d` | void | Set the NPC's post at point `loc` facing `d`; non-NPC → error. | E | 00576cc0 |
| 134 | `x, b` | void | Lock the AI: NPC → its lock with flag `b`; animal → its lock flag := 1; PC / other → error. | E | 00576d80 |
| 135 | `x` | void | Unlock the AI: NPC (error if not locked); animal → unlock and wake; PC / other → error. | E | 00576e00 |
| 136 | `soldier, k` | void | Force a battle decision: `k < 100` → a *permanent* forcing, `k ≥ 100` → a *temporary* one with `k − 100` as the code; the code (after that subtraction) must be 0..16 (a decision) or 99 (none) — so the accepted set is exactly {0..16, 99, 100..116, 199}; anything else → error, no change; non-soldier → error. Which decision each code selects is **not settled** (4.3: the consumer is the AI's battle decision, `spec-ai-combat.md`). | E | 00576eb0 |
| 137 | `loc, k` | void | Make a noise at point `loc`: `k` 0 → noise type 11, 1 → type 12; other `k` → error **and the value is still used as the type** (E+); null → error. | E / E+ | 00571160 |
| 138 | `human, b` | void | Freeze flag := `b`; unknown / non-human → error **but still applied** (E+). | E+ | 00576bb0 |
| 139 | `b` | void | Level "freeze all" flag := `b`. | – | 00576c30 |
| 140 | `npc, k` | void | Walking style: 0 walk, 1 run, others verbatim (AI-093); non-NPC → error. | E | 005780d0 |
| 141 | `soldier` | int | Rank; non-soldier → error 0. | E (0) | 00579a30 |
| 142 | `x` | bool | Animated map element active flag; other → error 0. | E (0) | 00570d00 |
| 143 | `x, b` | bool | Animated map element active := `b` → 1; other → error 0. | E (0) | 00570d80 |
| 144 | `patch` | bool | Patch active flag; null unchecked. | U | 00570e20 |
| 145 / 146 | `patch` | bool | Activate / deactivate the patch and **invalidate the cached view** (a full redraw follows; the actor lock is untouched) → 1. | – | 00570e30, 00570e50 |
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
| 165 / 166 | `pc` | void | Add / remove `pc` to / from the next mission's team (HUD refresh); non-PC → error. 165 also marks the PC present and writes two further per-character values (one cleared, one set to a constant) whose consumer is **not identified** (4.3). | E | 00579210, 00579280 |
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
| 177 | `soldier, b` | void | Always-attentive := `b` (AI §6.1); non-soldier → error. | E | 005790f0 |
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
| 195 | `k` | int | Script-visible campaign value `k`, `k` in 0..19 (twenty values kept by the campaign across missions; their meaning and their place in the save file are the campaign spec's); else error 0. | E (0) | 00579430 |
| 196 | `k, v` | void | Set campaign value `k` := `v`; `k` outside 0..19 → error. | E | 00579470 |
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
| 223 | `banner` | bool | Banner inactive (= captured); null → error, then an unchecked **read** (U; 8.1: 0). | E / U | 00579730 |
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
| 254 | `x, b` | void | A per-element flag of `x` := `b`; the flag's consumer is **not identified** (4.3), so its effect on play is unsettled; `x` unchecked. | U | 0057ba50 |
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
5. **Outside a recording (VM-203).** `n56(5)` alone → error, nothing scheduled; `n43(null, 1)` alone → no `ProcessMessage`; `n46(actor, p, -1, 0)` alone, with `actor` idle at `q` → the actor is in both entered lists with entry `p` (a following `n46` for the same actor takes the "already listed" branch and leaves its position alone), no walk is recorded, result 0, and the actor's position has changed to the entry geometry of `p` — **the exact position is deferred** to the navigation spec's entry-placement rule (not asserted here); `n63(pc, corpse, 0)` alone → error, 0, the PC not registered, position unchanged; `n47(...)` alone → OpenSherwood fault (U); `n109(null, 1)` → `ProcessMessage(1,0,0)` runs before `n109` returns.
6. **Simultaneous timers (VM-221).** Level 1 = timers 3 and 3; level 2 = message 9 to the level; launched from `Hourglass(0)` (step 4 of tick 0): the pass of tick 0 already visits both (3 → 2), tick 1 (2 → 1), tick 2 (1 → done); both complete in the pass of tick 2 in insertion order; `ProcessMessage(9)` runs inside that pass, before the pass visits any later timer; a timer it inserts is first visited in tick 3. `n56(0)` completes on its 2^32-th visited pass; `n56(-1)` on its (2^32 − 1)-th.
7. **Hourglass schedule (VM-103, VM-242).** No forcing: `Hourglass(0)` and `CheckVictoryCondition(0)` at T = 0; `Hourglass(1)` at T = 25 without a check; `CheckVictoryCondition(3)` at T = 75. `n29()` inside `Hourglass(1)` → `CheckVictoryCondition(1)` **in the same tick** (T = 25), and `(3)` still at T = 75. `n29()` from a zone callback at T = 30 → `CheckVictoryCondition(2)` at T = 50.
8. **Victory and end (VM-103b).** `CheckVictoryCondition` returns 1 at T = 75 with two present PCs → no `Finalize`; the notice is shown once; nothing ends until a trigger fires: (a) both PCs dead or departed → end bookkeeping in that tick, `Finalize(0)` next tick, code 2; (b) a later `CheckVictoryCondition` result 2 → the same success end; (c) the abort flag → the same. Without a prior victory, result 2 at T = 150 → bookkeeping in that tick (T stays 150), `Finalize(1)` next tick, code 3.
9. **Modal suspension (VM-218).** Level 1 = page 4 (a queued category); level 2 = timer 1. The page opens when the element is **drained** (step 9 of the launching tick, or of the next tick if launched after step 9); until dismissal T does not advance and no other sequence or camera element progresses; after dismissal the element is done, level 2 starts, and its timer is visited by the next timer pass. `n202(4)` inside `IsTaken` returns only after dismissal.
10. **Abort propagation (VM-217).** Level 1 = walk of a dead actor, timer 2, message 7; level 2 = message 8. Message 7 runs at dispatch; the walk is refused when drained (state refused, propagated to message 8); the timer still completes; level 1 never completes; `ProcessMessage(8)` never runs; the sequence is deleted at the next housekeeping. Setting the refused element to refused again changes nothing.
11. **Re-entrancy (VM-213).** Level 1 = message A (to the level), message B (to the level), page 2 (queued category); `ProcessMessage(A)` launches a sequence S whose level 1 = message C, a timer of 1 and page 1 → order: `ProcessMessage(A)`, `ProcessMessage(C)` (nested launch dispatched completely: its timer is in the timer list at once, page 1 appended to the FIFO), then `ProcessMessage(B)`, then page 2 appended; the drain shows page 1 before page 2; S's timer completes in this tick's timer pass. Variant: `ProcessMessage(A)` cancels message B's element → B never runs, the level's count stays charged for B, so level 1 **never completes** even after page 2 is done, and the sequence is deleted by housekeeping; cancelling B a second time changes nothing.
12. **Message target (VM-120).** `n43(null, 1)` in a launched recording → level `ProcessMessage(1,0,0)`; `n43(scroll, 1)` → error, nothing; `n44(actor, 2, 3, 4)` → that actor's `ProcessMessage(2,3,4)` with `n74()` = that actor inside it.
13. **Context (VM-093, VM-094).** Inside an actor's `ProcessMessage` that calls `n109(null, 6)`: the level's handler sees `n74()` = that actor. Inside a scroll's `IsTaken` that sends `n109(actor, 6)`: the actor's handler sees `n192()` = that scroll, `n74()` = the actor; after both return, `n192()` = null.
14. **Invalid inputs (VM-089).** Fixture: `N = 130` elements, **no carts** (`C = 0`), an array of 16 declared variables: `n3(999)` → null with an error; `n3(130)` → null with an error (no cart table entry); with a second fixture of `C = 2`, `n3(130)` → the first cart, `n3(132)` → null with an error; `n3(-1)` → null silently; `n2(99)` → −1; `n1(99, 5)` → no effect; `n24(x, 444)` → no effect; `n161(0)` → a fault (8.1); `n8(-1)` → null (8.1); `n168(-1)` and `n168(size)` → a fault (X); a jump to `0xFFFFFFFF` → the callback ends (8.1).
15. **Arithmetic (VM-061..068).** `0x24` on (2, 2) → 1; `0x26` on (−1, 0) → 0; `0x2B` on (NaN, 1.0) → 1.0f; `0x2E` on (NaN, 1.0) → 0.0f; `0x2C` on (NaN, NaN) → 0.0f; `0x17` on 4294967296.0 → 0; `0x1C` on (INT_MIN, −1) → a fault; `0x19` on (INT_MAX, 1) → INT_MIN.
16. **Result conventions (VM-086).** `n206(0x1FF, 0x100)` → 0x100; `n128(hostile npc)` → 1; `n112(7)` → 1 with an error; `n46(...)` → 0 even when it recorded; `n2(k)` holding −5 → −5; `n75()` = the full table size; with `C >= 1`, `n3(N16)` addresses the first cart and `n10` of that cart returns 0; with `C = 0`, `n3(N16)` → null with an error.
17. **Restore equivalence (VM-014, VM-088, VM-200, 8.3).** Run a level to the end of tick 10 in a state that exercises every persistent item: an open recording left by `Hourglass(0)` (`n30(); n56(3)` without `n31()`), two pending sequences, three timers, mission variables, the nested-callback return register of case 3, a native argument buffer holding one residual cell (a callback that executed `0x0B` without a following `0x0C`), a script-parameter buffer holding one residual cell (a callback that executed `0x02` without a `0x05`), and a native result register set by the last `0x0C`. Snapshot; run tick 11, in which a message element delivers `ProcessMessage(7, 8, 9)` to that instance: the callback reads its parameters with `0x08` at offsets 0, 4, 8, 12 and observes **the residue at offset 0 and `7, 8, 9` at offsets 4, 8, 12** (VM-088: the engine appended after the residue); it reads the native result register with `0x0D` and observes the saved value; it pushes **one** cell and calls a native of arity 2, which receives the residual native cell as its first argument and the pushed cell as its second; it then pushes one parameter and calls a script function with `0x05`, whose `0x08` at offset 0 reads that parameter **unshifted** (a fresh buffer); finally `n32()` and `n31()` close the saved recording (the timer is at level 1, the barrier returned 2). Restore the snapshot; run tick 11 again → identical script-visible state, identical native calls with identical arguments in identical order, identical callback return registers, the same sequence launched with the same levels.
18. **Initialisation order (VM-104).** An actor `Initialize` sets variable 0 := 1 (after declaring it) and the level `Initialize` sets it := 2: the level's value wins; the scrolls' `Initialize` run after it; `PostInitialize` runs after `Hourglass(0)` when the first tick executes.
19. **Camera natives (VM-219).** Screen 640 × 480 throughout. *Conversion fixtures (no scroll pending, no camera element, no lock):* zoom 1.0, map 2000 × 2000: `n20((1000, 1000))` → corner (680, 760) immediately after the call, **before any camera processing** (assert on the state, not on a drawn frame; the zoom is 1.0). Zoom 1.0, map 2000 × 500: `n20((1000, 490))` → y0 = 250, `250 + 400 > 500` with y0 non-negative → clamped, corner (680, 100). Zoom 2.0 (W = 320, H = 200), map 300 × 2000: `n18((10, 1000))` → x0 = trunc(10 − 160) = −150 negative and `0 + 320 > 300` → fallback: zoom 1.0, destination (0, 0), y not processed. Zoom 0.5 (centring offsets 640 and 480), map 4000 × 4000: `n20((1301, 1200))` → x0 = 661 odd → 660, y0 = 720 → corner (660, 720). *Uncontested scroll fixture:* zoom 1.0, map 4000 × 4000, scroll speed 0, ramp index 0, the corner at (500, 500); `p` = (1020, 740) → `d` = (700, 500), axis-aligned with the corner and unclipped. After `n18(p)`: the corner is unchanged in the call, the zoom stays 1.0, no camera element exists; the next camera update moves the corner exactly 2 px along x; later updates follow the ramp (ANIM-330). When the corner lands exactly on `d` in update `k`, update `k + 1` clears the destination and resets the step length to 1.0. *Active-camera fixture:* same map, the corner at (500, 500), a native-42 scroll element with speed 3 running toward `d1` = (500, 900); `n19((1018, 740), 7.0)` → the element stays the camera element, the destination becomes `d2` = (698, 500) (axis-aligned, unclipped, 198 px away — divisible by 3): the next 66 updates each move the corner exactly 3 px along x (not 7), and the 67th update finds the corner on `d2` and completes the element. (Rule for a distance not divisible by the speed: each update moves `min(speed, remaining)`, so 200 px at speed 3 is 66 moves of 3 px, one move of 2 px, then the arrival update.) `n20(p)` with a destination pending and an actor lock set by 39: immediately after the call the corner equals `d`, the destination is still pending and the lock is still set. After `n21(2.0)` the requested zoom is 2.0 and the drawn zoom changes in later updates (count unsettled, ANIM-333); `n21(1.5)` → error, request unchanged.
20. **Nested-fault restore (8.1, 8.3).** Level `Hourglass(1)` at T = 25 sends `n109(actor, 5)`; the actor's `ProcessMessage(5,…)` executes `n161(0)` (a trap). Expected: the actor's callback terminates with `Fault::Trap` (its frames popped, its return register unchanged), `n109` returns normally to the Hourglass with the native result register 0, `n74()` afterwards is the value it had before `n109` (null in the level's Hourglass), the Hourglass continues to its end and the fault log holds one entry at T = 25. Snapshot at the end of tick 25 and at the end of tick 24; restore the tick-24 snapshot and run tick 25 again → the same single fault entry, the same states; restore the tick-25 snapshot and run tick 26 → the log still holds exactly one entry.
21. **Missing-callback restore (8.1, 8.3).** A zone class without `ExitZone`; an actor leaves the zone at T = 30 and again at T = 40. Expected: at T = 30 the engine's call is a no-op, the parameters it appended are removed again, `Fault::MissingCallback(class, ExitZone)` is recorded once; at T = 40 nothing new is recorded. Restore the snapshot taken at the end of tick 35 and run to T = 40 → still one entry (the suppression set was restored); restore the snapshot taken at the end of tick 29 and run to T = 30 → the entry is recorded again exactly once.
22. **Sentinel jump in a script-call frame (VM-070).** A zone `EnterZone` calls `f` with 0x05; `f` executes `0x0E a=b=0xFFFF` → `f`'s frame is popped, `EnterZone` continues at the instruction after its 0x05 (an `0x13 x, 4; 0x07 x` there makes the engine read 4), `Fault::SentinelJump(class, addr)` recorded once; a second entry records nothing new.
23. **Sentinel jump in a same-instance nested callback (VM-070, VM-095).** Script trace on the level instance — `Hourglass`: `0x03; 0x05 h; 0x0B t(null); 0x0B t(6); 0x0C 109; 0x13 y, 1; 0x06` where `h` = `0x03; 0x13 r, 9; 0x07 r` (a helper that returns 9: its 0x07 at depth 2 writes 9 to the callback return register and to the Hourglass frame's result slot, then returns to the Hourglass, which goes on to the native call); `ProcessMessage`: `0x03; 0x0E 0xFFFF, 0xFFFF` (the sentinel jump at the nested callback's own depth). Expected: the register holds 9 when the nested `ProcessMessage(6, 0, 0)` starts on the same instance (depth 2 above the Hourglass frame); its sentinel pops the engine callback frame (saved return −1) → only the nested invocation ends, the register still holds 9 (the nested invocation never wrote it); `n109` returns, `0x13 y, 1` executes, the Hourglass ends with 0x06 and the engine reads **9**. Variant: `ProcessMessage` = `0x03; 0x05 g; 0x13 z, 2; 0x06` with `g` = `0x03; 0x0E 0xFFFF, 0xFFFF` → the sentinel inside `g` pops a script-call frame (case 22's rule), `ProcessMessage` continues with `0x13 z, 2` and ends with its own 0x06; the engine still reads 9 afterwards. Alternative seeding: the register may instead be left at 9 by a *previous completed* callback of the instance (e.g. `CheckVictoryCondition` returning 9 on an earlier tick — the engine treats 9 as "continue"), since the register is never reset (VM-010).
24. **Scroll overlap (VM-094, VM-106, 8.1).** Scroll A's class `Hourglass` returned 5 from an earlier run and scroll B's class instance holds 7 in its return register; while A's `Hourglass` is running (current scroll = A), the harness injects a take-scroll element for B (harness hook: the engine's pickup order) and dispatches its level. Expected: B's status becomes 3 and the sound plays; `IsTaken` of B is **not** run; B's register still holds 7 and is not consulted; B's status stays 3 and B stays visible; the take element is *done* at once; `n192()` inside the rest of A's `Hourglass` is still A, and null after it ends; `Fault::ScrollOverlap(B)` recorded once; a later ordinary take of B (outside any scroll callback) runs `IsTaken` normally.
25. **Timer before a deferred fault (8.1, VM-215, VM-217, VM-221).** Level 1 = timer 2; level 2 = `n57(h, actor, 1, 0)` recorded with `h` a handle that is not an actor-family element (a door handle from native 4); level 3 = message 9 to the level; launched from `Hourglass(1)` at T = 25 (step 4). Expected: the timer is visited in the passes of T = 25 (2 → 1) and T = 26 (1 → done); level 2 is dispatched inside the pass of tick 26 (step 10) and the seek is appended to the FIFO; it is handed over in the drain of **tick 27** (step 9), where it is set *refused*, `Fault::DeferredTarget(57, h)` is recorded at T = 27 with the sequence's provenance (the level class, `Hourglass`, T = 25), message 9's element is set *refused* in chain (never delivered), and the sequence is deleted by the next housekeeping. A snapshot at the end of tick 26 (seek pending in the FIFO) restored and run through tick 27 reproduces the same fault entry and attribution (case 26).
26. **Provenance and suppression restore (8.3)** — three separate runs, because there is only one global recording (VM-200) and a second `n30()` while it is open is refused. **(a) Open recording spanning callbacks.** `PostInitialize` (T = 0) executes `n30()` and records nothing; no other callback records anything until a zone `EnterZone` at T = 10 appends `n56(3)`; a level `Hourglass` at T = 25 executes `n30()` → refused (0, provenance unchanged); snapshot at the end of tick 12 (recording still open, one timer at level 1); run to T = 30, where a zone `ExitZone` closes it with `n31()` → the launched sequence's provenance is (level class, `PostInitialize`, T = 0), its timer completes at T = 32; restore the tick-12 snapshot and run to T = 32 again → identical provenance, identical timer completion tick. **(b) Fault entry and attribution.** Run case 25 alone to the end of tick 26 (the seek pending in the FIFO, the sequence's provenance triple (level class, `Hourglass`, T = 25)); snapshot; run tick 27; restore; run tick 27 again → the same single `Fault::DeferredTarget(57, h)` entry at T = 27 with the same provenance in both runs. **(c) Scroll-overlap suppression.** Inject case 24's overlap for scroll B at T = 20 (`Fault::ScrollOverlap(B)` recorded once); snapshot at the end of tick 22; inject the same overlap at T = 23 → nothing new is recorded; restore the tick-22 snapshot and inject at T = 23 again → still nothing new (the suppression set was restored); restore a snapshot taken at the end of tick 18 and inject at T = 20 → the entry is recorded again exactly once.

## 8. Implementation choices

### 8.1 Deliberate departures from the original

| Case | Original | OpenSherwood | Reason |
|---|---|---|---|
| fixed timestep | 40 / 400 ms reported-counter thresholds; 46.875 ms measured reference cadence on the oracle host (VM-100) | one logic frame of 46.875 ms for every tick-counted quantity — `docs/decisions/ADR-0010-logic-frame.md` (the nominal 40 ms is documented, not used) | ADR-0010: reproduces the measured original on the reference host; a fixed timestep is the only deterministic choice |
| jump to `0xFFFFFFFF` (VM-070) | unchecked fetch | **not a termination**: the *current frame* is popped exactly as by 0x06, and the outcome is the popped frame's saved return address: a **script-call frame** → control continues at its caller's return pc (the callback goes on); an **engine callback frame** (saved return −1, at any depth — a nested same-instance callback too) → that interpreter invocation ends with the return register unchanged, and the enclosing native-call instruction, if any, resumes the outer callback; pending 0x02 / 0x0B cells stay in their buffers as after any 0x06; recorded as `Fault::SentinelJump` (once per class and address). Cases 22, 23. | the script's intent; UB |
| unchecked accesses (U) — native reads (3 with `i < -1`, 6, 9, 8, 98, 144, 164, 182 - 185, 217 out of range, 223 null) | arbitrary memory | the native returns **null / 0** (223: 0), the callback **continues**, the wrapper still pops its arity, and a `Fault::UncheckedAccess(id)` is recorded (a strict mode terminates the callback instead) | UB |
| unchecked access (U) — instruction 0x08 beyond the frame's parameters | arbitrary memory | `S(a) := 0`, `pc += 1`, no register changed, the callback **continues**; `Fault::UncheckedAccess(0x08)` recorded (strict mode: terminates) | UB |
| storage class `00` reference (VM-010 / VM-011; rev. 10) | null-based unchecked access (no global block exists) | any instruction naming a class-`00` symbol: a read yields 0 and the callback **continues**; a write **terminates** the callback; both record `Fault::UncheckedAccess(0x00)` — OpenSherwood keeps **no** global block (a 1024-cell block, as batch 1 built, is a harmless but unfounded model and must not be relied on) | UB; never emitted |
| **deferred execution fault** — a seek element of 57 / 70 / 71 whose `actor` is not an actor-family element (no check at recording time, rows 57 / 70) | unchecked access by the actor side, long after the native returned | **no native-return outcome exists**: the recording native returned 1 and its callback may have ended. The fault is observable at the moment the element is **handed over** — the first queue drain (VM-215, step 9) that removes it from the FIFO: the drain of the tick in which its level was dispatched if the dispatch happened before that tick's step 9 (a launch from a callback of steps 4 or 8, a level completion during the drain itself), otherwise the **next** tick's drain (a level dispatched by a timer completion in step 10, or by a camera-update completion after the tick, is drained one tick later; case 25). At hand-over the element is set **refused** with the next-level propagation of VM-217 — the same state propagates: its level never completes and the first element of every later level is **refused** in chain (not cancelled); the attached message of 70 / 71 is **not** delivered; `Fault::DeferredTarget(id, handle)` is recorded at the hand-over tick, attributed to the element's sequence provenance (8.3: the recording's provenance triple, or the engine origin). Nothing already returned is changed and no callback is unwound. A seek whose `actor` is valid but whose `target` is not is **not settled** here: what the seek runner does with it is behind the actor gate (VM-216 / 4.3); OpenSherwood applies the same refusal as policy until that gate opens. | UB; the outcome had to be chosen |
| unchecked accesses (U) — writes (0 with `k < 0`, 67 / 68 / 72 / 73 / 254 out of range, 179 / 186 - 189 null, 47 outside a recording, 0x0B overflow) | arbitrary memory | the callback **terminates** with `Fault::UncheckedAccess(id)`; no partial write | UB |
| arithmetic traps (T: 161 with 0, 0x1C) | process exception | the callback **terminates** with `Fault::Trap`; the return register is unchanged | crash |
| container exception (X: 168) | C++ exception unwinds the native | the callback **terminates** with `Fault::Range(168)` | crash-equivalent |
| fatal routine (F: scroll callback overlap) | termination of the process, detected **before** the attempted callback's frame is pushed (status 3 and the sound have already happened) | **not a termination of any script frame**. Take outcome: the effects that precede the check stand (status 3, the sound); the attempted `IsTaken` is **not run**, no frame exists, and **no saved callback return register is consulted** — the take proceeds exactly as after a zero result: status stays 3, the scroll is not hidden, the current scroll stays the *enclosing* scroll (cleared when *its* callback ends, as usual), the take element completes at once as always, and the enclosing callback continues; recorded as `Fault::ScrollOverlap(scroll)` once per scroll. For the `Initialize` / `Hourglass` of a scroll attempted inside another scroll's callback the same rule applies: not run, register untouched, enclosing scroll retained. Whether any retail path reaches the overlap is not established (no native records a take-scroll element; the engine's pickup element is dispatched from the tick); case 24 injects it. | crash |
| null dereference (C: missing callback) | crash | the engine's call is a **no-op**: the parameters the engine appended for this call are removed again (the script-parameter buffer is left exactly as before the call, residue included), both registers are unchanged, the engine reads the return register (0 unless an earlier callback set it); recorded once per class and name as `Fault::MissingCallback` | crash; the retail data never lacks a called name |
| non-termination (N: ≥ 0x30) and the 0x00 fetch | hang / unchecked fetch | the callback **terminates** with `Fault::BadOpcode` | hang / UB |
| unwritten result slot (VM-071) | undefined value | reads 0 (`UnwrittenResultSlot`) | UB |
| level-counter wrap (VM-201) | level-0 appends, unclosable recording | the callback **terminates** with `Fault::BarrierOverflow` when the counter would turn from 65,535 to 0 — by native 32 (its 65,535th increment from level 1) or by an increment inside the compound walk recorder | unreachable in retail data; UB-like |
| excluded natives (4.2) | unread effect | `UnknownNative(id)`, placeholder of 4.2 (or a strict fault) | not settled |
| version check (VM-001) | float equality (NaN accepted) | bit-exact 1.5 | no retail file is NaN |
| timers `n <= 0` (VM-221) | wrapping counter | identical wrapping counter (no departure) | cheap and faithful |

**Termination contract** (every row that says "terminates"): (1) *unwind boundary* — all script frames of the
faulting callback on that instance are popped (each pop discards its captured parameter buffer, VM-013), down to
and including the frame the engine pushed; the callback's own current parameter buffer stays current (it is not
cleared — persistence as in VM-088); (2) *native argument buffer* — the faulting native's wrapper has already
removed its arity when the fault is raised inside the native; a fault raised by the instruction itself (0x0B
overflow, bad opcode, 0x08 out of range) removes nothing; (3) *registers* — neither the native result register
nor the callback return register is written by the fault; the engine reads the return register as after a 0x06;
(4) *nesting* — a fault inside a nested callback (VM-095) terminates **only the nested callback**: the native that
invoked it (109 / 110 / 153 / 154, or the level-start message dispatch) returns normally with its usual result and
the enclosing callback continues; the invoker restores the current actor / current scroll exactly as on a normal
return (VM-093, VM-094); (5) *engine-level state* — an open recording, the mission variables, launched sequences
and every element state are left as the fault found them (nothing is rolled back); (6) the engine continues its
tick after the terminated call. **Fault state** (snapshot, 8.3): the fault log — the ordered list of recorded
faults (kind, id / address / handle, tick `T`, instance or sequence provenance) — and the three suppression sets
that make "recorded once" reproducible — (class, name) pairs for `Fault::MissingCallback`, (class, address) pairs
for `Fault::SentinelJump`, scroll handles for `Fault::ScrollOverlap` — are authoritative state, hashed and
restored with the rest. Every fault is deterministic and part of the replay hash.

### 8.2 Random numbers

Native 161 draws from the process-wide C-runtime generator (the linear congruential `rand()` of the Microsoft
runtime, 15-bit results), shared with every other consumer (the AI, `spec-ai-combat.md` 2.2). OpenSherwood uses
its named seeded stream for the script (ADR-0004); the consumption order between the script and the AI within
a tick follows VM-103 and belongs to the determinism model, not to this spec's fidelity claims.

### 8.3 Snapshot and restore boundaries

Snapshots are taken only at **tick boundaries outside any callback and outside any modal loop** (no frame is
live then). Authoritative script state (all hashed): per instance VM-014 (the class block, the callback return
register, the native result register, the **native argument buffer** with its fill count, the **current
script-parameter buffer** with its logical length, the global block); the mission-variable array; `T`, the force flag, the won flag
and sub-flag, the three end flags, the debriefing index; **the recording state** (the open sequence with its
elements and their levels, the level counter, the two entered lists — a recording *can* span callbacks, VM-200);
every live sequence (elements with category, target, arguments, state, level; level counters; the manager's
FIFO order; the timer list with counters and order); the current actor and current scroll are null at a tick
boundary; **the fault log and the three suppression sets** of 8.1; and the **sequence provenance** defined next.
**Provenance** (OpenSherwood bookkeeping, needed only for fault attribution; the original keeps none): a
recording's provenance is the triple (class of the instance whose callback executed native **30**, name of that
callback, tick `T` at which native 30 ran) — fixed when the recording is opened, so a recording that spans
several callbacks (VM-200) is attributed to its *opener*, not to the callback that recorded a given element or
closed it with native 31; native 31 copies the triple onto the launched sequence, and every element retains its
sequence's triple while pending or running. Engine-originated sequences carry (engine, kind of the originating
order, tick). The open recording's triple and every live sequence's triple are part of the snapshot and of the
hash. Element flags read by natives belong to their owners' snapshots. Restoring at a tick boundary reproduces
the following ticks exactly, including the faults recorded afterwards and their attribution (cases 17, 20, 21, 26).

### 8.4 Differences from the current engine (`docs/formats/scb.md`, `natives.rs`)

1. Opcode 0x07 returns immediately.
2. Comparisons: 0x24 `<=`, 0x26 `>=`, 0x27 `>`, 0x28 `!=`, signed; 0x2A..0x2F float compares with float results and unordered outcomes; 0x1C / 0x1F / 0x16 / 0x17 exist.
3. Jump / call / native-call targets are `a | (b << 16)`; the two `-1` jumps.
4. Native 2 returns −1 for an undeclared variable; 0 grows by 16; 1 errors on undeclared.
5. The element table includes one slot per campaign character; 3 truncates its boundary to 16 bits while 75 does not; 10 returns cart indices from 0.
6. Natives 111 and 159 are null: messages "to the player" go to the level class.
7. Messages are synchronous; 44's fourth argument is `arg2`.
8. The clock: 40 ms threshold, 46.875 ms measured reference and ADR-0010 logic frame (VM-100); Hourglass every 25 ticks with `T/25`; `CheckVictoryCondition` every third Hourglass or forced (same tick when forced inside Hourglass); scroll Hourglass on its own counter with 0; actor classes never get Hourglass; `HandleEvent` never; `ActivatedBy*` results ignored.
9. `Finalize(0)` = success, `(1)` = failure; a declared victory ends nothing by itself; result 2 requests the end bookkeeping; the tick has early exits.
10. Initialisation order (VM-104).
11. Sequences: level-parallel with a barrier that counts once; abort cascade; pages / dialogs are synchronous modal loops (203 does hold the sequence, by suspending the program); 33 / 42 wait for the camera update, 34 is instant; recording natives outside a recording drop their element (46 keeps its placement; 47 is an unchecked append; 63 does nothing).
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
- The measured reference cadence (VM-100 b): confirmation of ANIM-002 by the movement spec's review (no claim is made for other hosts).

### 9.4 Contradictions with the sibling drafts to reconcile
- Movement pin `b0cd053`: resolved — the "jump" reading of 18 / 19 (ANIM-331 / ANIM-332 agree with rows 18 - 20); **not agreed** — the shared destination conversion, its zoom-reset fallback and the interference of 18 / 19 with a running camera element, which VM-219 states from the executable and the pin does not state (1.1).
- (Resolved: native 126's order — the AI pin's §6.1 and this spec agree; review 22 upheld it.)
- AI spec §6.1, native 134 on a player character ("gets a lock byte" there; this spec: error, no effect, 00576d80) — **unresolved**.
- AI spec §6.1, native 59 ("a play-animation step" there versus an action element, 00573450) — **unresolved**.
- Navigation: no contradiction, but the pinned revision is under redo (review 20); nothing it states is cleared.

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
oracle evidence is cited there). Sibling specifications referenced, at the pinned revisions of 1.1 (all `draft`; reviewed with the dispositions
recorded in 1.1 — navigation under redo, the others fix-then-clear with partial clearances):
`docs/original/spec-navigation.md` `e5a2e0c` (NAV-130, NAV-146, NAV-150, NAV-172, NAV-190, NAV-191, section 5),
`docs/original/spec-movement-animation-camera.md` `b0cd053` (ANIM-002, ANIM-033, ANIM-131, ANIM-140, ANIM-208,
ANIM-320 - ANIM-326, ANIM-330 - ANIM-333, ANIM-340, ANIM-521, element table 3.4), `docs/original/spec-ai-combat.md`
`e966b05` (AI-041, AI-066, AI-074, AI-081, AI-082, AI-084, AI-091, AI-093, AI-097, AI-190, sections 2.2, 3.4, 6.1),
`docs/original/spec-campaign-camp-saves.md` `2cb9c2a` (CAMP-012, CAMP-310); decision
`docs/decisions/ADR-0010-logic-frame.md`; reviews `docs/decisions/reviews/2026-09-13-codex-review-14-spec-script-vm.md`,
`2026-09-13-codex-review-17-spec-script-vm.md`, `2026-09-13-codex-review-21-spec-script-vm.md`,
`2026-09-18-codex-review-25-spec-script-vm.md` (identity block), and the sibling reviews 20, 22, 23, 24 cited in 1.1. Tests that will depend on this
spec: section 7 (the VM rebuild, ADR-0009).
## 11. Implementer questions (batch 1, 2026-09-18)

Written by the implementer session that built the cleared parts (ADR-0009: an implementer may add to this
section and must never guess silently). Every item names what the implementation chose meanwhile.

1. **Native 192's arity.** Row 192 takes no argument (`Args` column: none), but the arity-0 list of VM-087
   does not name it, while it names the other 36 arity-0 ids. Is the list simply missing 192, or does its
   wrapper pop a cell? The implementation follows the row (arity 0) and pins the list, 192 included, in
   `the_call_table_follows_the_specification`.
   **Answer (revision 10):** the list was missing 192. Its wrapper (the 193rd entry of the table) is a plain
   tail-jump to the native, pops nothing and returns the handle in full width; the arity-0 set has **37** ids
   and VM-087 now names 192. The implementation is right; the pin stands.
2. **The size of the global block** (storage class `00`, VM-010 / VM-011). The specification says a shared
   global block exists and that the retail files never address it, but not how large it is. The engine
   allocates 1024 cells (`GLOBAL_CELLS`) and treats anything beyond as an unchecked access.
   **Answer (revision 10):** there is **no global block**. The instance factory hands every instance the same
   static descriptor (a pointer / size pair in uninitialised data), and nothing in the program ever allocates
   it — its only writer is the shutdown release. Symbol resolution is `base + offset` with no size check for
   any class, so a class-`00` symbol dereferences a null base (VM-010 / VM-011, 8.1). The 1024 cells are a
   model with no counterpart: keep them only if they cost nothing, and treat any class-`00` reference as
   `Fault::UncheckedAccess(0x00)` (read → 0 and continue, write → terminate) rather than as a valid cell. No
   retail file emits one.
3. **Float rounding of `0x20` - `0x23`.** VM-066 says "extended intermediate, stored as single". The engine
   computes each operation directly in `f32`, which is correctly rounded once; the x87 rounds twice (80-bit
   then single) and can differ in the last bit for a pathological pair. Is a double-rounded result ever
   observable in the shipped data (the corpus has 9 float multiplies and 1 float comparison), or is the
   single-rounding reading safe to keep?
   **Answer (revision 10):** safe, and provably so — not merely for the corpus. The x87 runs at the runtime's
   default 53-bit precision (or 64-bit; the game code never lowers it: the only control-word change is the
   truncating float-to-int helper, which restores it), and for `+ − × ÷` on single operands an intermediate of
   ≥ 50 bits makes the second rounding innocuous, so the stored single is the correctly rounded result — the
   same value `f32` arithmetic gives (VM-066). The only theoretical exception, a subnormal single result, is
   unreachable from the retail operands. The 9 multiplies and 1 comparison of the corpus are therefore
   bit-identical under either reading.
4. **The `Hourglass` cadence while the scheduler is uncleared.** VM-103 step 4 runs the level's `Hourglass`
   every 25 frames with `T / 25`; the engine still runs `Hourglass` on **every** class every frame with the
   frame counter, because 3.5 is not cleared. No `Assumption` variant of 4.1 covers that departure - should
   the scheduler's departures get one (say `SchedulerCadence`) until 3.5 is cleared, or is the batch that
   implements 3.5 close enough that the gap can stay unnamed?
   **Answer (revision 10):** name it. ADR-0008 wants every departure from an observed rule mapped, and the
   cadence *is* observed (VM-103 step 4, VM-105: the level's `Hourglass` every 25 ticks with `T / 25`, the
   scrolls' with 0 on their own counter, the actor classes never). Section 4.1 now carries one row,
   `SchedulerCadence`, covering every rule of 3.5 the engine has not implemented; the batch that implements
   3.5 deletes it. Until then the level scripts that count `Hourglass` arguments (the ones comparing `T / 25`
   against thresholds) run off-schedule, which the assumption makes visible.
5. **Natives 4 / 5 / 8 / 11 / 12 / 15 over lists this engine does not keep.** The rows describe indices into
   the map's door, patch and building lists; the engine has no such lists, so the handle *is* the index and
   the inverse natives are the identity. Every one of them records `Policy(id)`. Is that the reading the
   navigation specification will settle (NAV section 5), or will the handles become opaque?
   **Answer (revision 10):** handle = position is observationally equivalent, and can stay, under three
   conditions that the navigation spec (under redo, review 20) has to pin rather than this one: the lists are
   the **file-order flat lists** of the map (doors, patches, buildings; patrol paths for 9 / 16) that NAV
   section 5 / NAV-172 describe; every other native that yields or consumes such a handle (64's nearest-door
   search, 152, 156, 182 - 191, 144 - 146) uses the same lists; and scripts do nothing with the handles but pass
   them back, test them for null and compare them (86) — which is all the corpus does. The original's handles
   are objects and its inverse natives search the list for the object, returning −1 (16: 65535) when absent,
   which the identity reproduces only for in-range inputs — so the out-of-range rows (4 / 5 null with an error,
   8 / 9 unchecked, 16 modulo 65536) still have to be honoured on top of the identity. Section 4.1 now records
   this as one row that turns from `Policy(id)` into a navigation dependency when the redo lands.
6. **Native 118's unmodelled properties.** Row 118 lists twelve properties; the engine keeps them in one
   hashed table and answers 0 for a property nothing has written. `Emb02_FoC_MK` wins on its first tick
   because its `CheckVictoryCondition` tests property 1 (an NPC's money) and reads 0; with `0x07` returning
   at once (8.4 item 1) that reaches the `return_value 1`. Should an unwritten property answer something
   other than 0, or does the money have to come from the mission record before that mission behaves?
   **Answer (revision 10):** the money comes from the mission record; there is no "unwritten" property. The
   twelve properties are live fields of the element (row 118). Property 1 is loaded by two loaders: a soldier's
   from its `BORG` record's u32 at offset 0x23 (the thirteenth field the loader reads; `rhm.md` calls it
   `unknown_0x23`, the AI spec reads the same storage as a loot threshold), a civilian's from its `OILE` record's
   u32 that follows the u16 after the profile index (`rhm.md` shows those six bytes as two i16 — a correction for
   the format doc, not for this file). Emb02's element 40 is its single civilian (the map's 24 elements and 16
   objects precede it), whose record carries **4000**; its victory check asks whether that value has reached 0,
   which only looting (native 229 or the NPC-to-NPC transfer) can bring about. No script writes property 1;
   six missions read it. With the record value loaded the first check fails as it must. The other properties:
   0 = the quiver's arrow count (0 without a quiver), 2 / 3 = health and stun (AI spec), 4 - 11 = the player
   character's item counters and 12 its name preset (campaign spec) — model them from those owners' state, not
   from a table of script writes.
7. **The strict mode of 4.2.** The specification allows a strict mode that faults on an excluded id instead
   of answering the placeholder. The engine always answers the placeholder (the retail scripts call 224 at
   load, so a strict fault would stop every forest mission); `MissionSpec::lenient_natives` now only decides
   whether the call is also logged with its arguments. Is that the intended default?
   **Answer (revision 10):** yes. The placeholder is the default and the strict fault is an opt-in for tests
   and analysis; 4.2 now says so. Two consequences to keep visible: 224's *effect* (the repulsive point) is
   navigation-gated, so its placeholder currently drops an effect the original has — record that under
   `UnknownNative(224)` as a known gap until navigation lands; and 173's result (read in fourteen missions)
   answers 0, which selects the branch those scripts take when the setting is off — a difference a player can
   notice only once the setting is identified (9.2). Logging the arguments is the right lenient behaviour.
