# SCB compiled mission script (`.scb`, magic `SBSCRIPT`)

Status: **container decoded; the VM's behaviour is specified elsewhere**.
`crates/opensherwood-formats/src/scb.rs` parses all 39 retail files to the last byte (classes, variables,
function tables, instructions) and prints a raw disassembly. This file describes only the **container**: the
layout, the instruction encoding, the index spaces the operands address, and what the corpus shows about how
the shipped scripts use them.

**The VM's semantics are not here.** The instruction set, the calling convention, the native call protocol,
the 265 natives, the callback scheduler, the sequence machinery, the failure classes and the deliberate
departures are `docs/original/spec-script-vm.md` (a behaviour specification derived from the executable under
ADR-0009). The engine implements that specification directly:
`crates/opensherwood-core/src/vm.rs` (interpreter and state), `crates/opensherwood-core/src/natives.rs` (the
call table) and `crates/opensherwood-script` (the translation from this container into the core's
instructions). Where the specification and an older hypothesis in this file disagreed, the specification won;
the hypotheses have been removed rather than left to rot.

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
variable table), `10` = function-local block (`lv`), `11` = temporary block (`tv`); `00` is the program-wide
global block (VM-011), which the retail scripts never address, so in practice it marks non-references (jump
targets, indices, zero). Three-operand arithmetic uses `a`, `b` and the low 16 bits of `c` (the high 16 bits
are then 0 in every instruction).

Opcodes present in retail data (208 679 instructions), operand layout established by observation:

Every role below is the one `docs/original/spec-script-vm.md` section 3.1 states; the counts are this
corpus's.

| Opcode | Count | Operand layout (observed) | Role (`spec-script-vm.md` 3.1) | Desperados 1.0 name |
|---|---|---|---|---|
| 0x01 | 9171 | none | filler after 0x07 | NOP |
| 0x02 | 1653 | a = var | push argument of a script call | PARAM |
| 0x03 | 8176 | a, b = sizes | function prologue: `(size_of_volatile, size_of_tempor)` | InitFunction |
| 0x04 | 8176 | none | reported as an error, then the next instruction (VM-044); the compiler emits it as the last instruction of every function, after the `0x06` that actually returns | EndFunction |
| 0x05 | 951 | a, b = code address | call; the target is `a \| (b << 16)` (VM-002) | CALL |
| 0x06 | 11296 | none | return | RETURN |
| 0x07 | 5160 | a = var (temp) | return with value: control does **not** continue (VM-047) | RETURN value |
| 0x08 | 2595 | a = var (temp), c = 0/4/8/12/16 | read parameter at byte offset c | GETPARAM |
| 0x0a | 62 | a = var | read the frame's result slot (VM-050); in all 62 uses it directly follows a `0x05` | GETRETURN |
| 0x0b | 43965 | a = var | push native argument | NATIVEPARAM |
| 0x0c | 42734 | a, b = native id (0..=264, 192 distinct) | native call, id `a \| (b << 16)` | NATIVECALL |
| 0x0d | 20079 | a = var (temp) | read the native result register (always directly after `0x0c` here) | NATIVEGETRETURN |
| 0x0e | 5510 | a, b = quad index | unconditional jump; the two `a = b = 0xffff` occurrences are the sentinel of VM-070 | GOTO (Desperados keeps the target in `c`) |
| 0x0f | 4601 | a = var (temp), c = quad index | jump if a is non-zero | IF a != 0 GOTO |
| 0x11 | 983 | a, b = vars | move | MOV |
| 0x12 | 10 | a (local), b = vars | move (float) | - |
| 0x13 | 37060 | a = var, c = integer immediate (0..=100000) | load int immediate | MOV int immediate |
| 0x14 | 110 | a = var, c = f32 immediate (1.0, 2.0, 0.01 ...) | load float immediate | MOV float immediate |
| 0x15 | 626 | a, b = vars (temp) | unary minus | NEG int |
| 0x18 | 327 | a, b = vars (temp) | int to float | - |
| 0x19 - 0x1b | 238 / 247 / 212 | a, b, c16 = vars | int add, subtract, multiply | +I -I *I |
| 0x1d, 0x1e | 117 / 17 | a, b, c16 = vars | bitwise or, and | +F -F |
| 0x22, 0x24 | 9 / 8 | a, b, c16 | float multiply; signed int `<=` | <I >=I |
| 0x25, 0x26 | 169 / 100 | a, b, c16 | int `<`, `>=` | !=I ==I |
| 0x27, 0x28 | 42 / 30 | a, b, c16 | int `>`, `!=` | <=F <F |
| 0x29 | 4244 | a, b, c16 | int / handle `==` | !=F |
| 0x2b | 1 | a, b, c16 | float `<`, answering a float | ==F |

Opcodes 0x00, 0x09, 0x10, 0x16, 0x17, 0x1c, 0x1f-0x21, 0x23, 0x2a, 0x2c do not occur. The Desperados column comes
from the GPL-3 OpenDeathValley disassembler (see Provenance); the numbering clearly changed between 1.0 and 1.5
(0x22 is a float multiply here, 0x29 the dominant equality test), so that column is history only.

## Opcodes, calling convention and natives

See `docs/original/spec-script-vm.md`:

- **section 3.1** - the semantics of every opcode `0x00`..`0x2F` and of anything beyond, the operand
  composition (`a | (b << 16)` for the jump, call and native-call targets; `c` for the conditional jumps; the
  low 16 bits of `c` for the third symbol of a three-operand instruction), the signed integer comparisons, the
  float comparisons with their unordered outcomes, the division traps and the float-to-int conversion;
- **section 3.2** - the calling convention (`0x02` pushes, `0x05` calls, `0x08` reads a parameter by byte
  offset, `0x07` returns a value *at once*, `0x0A` reads the frame's persistent result slot);
- **section 3.3** - the native call protocol: the 265-entry table, the per-id arity, which arguments the
  wrapper converts to a bool, and the three result conventions (void, the low 8 bits, the full word);
- **section 6** - every native id with its arguments, result convention, behaviour and failure class;
- **sections 4.2 and 4.3** - the twelve ids excluded from clearance and the eighteen whose effect is settled
  only up to a code, flag or consumer;
- **section 8.1** - the deliberate departures (what an unchecked access, a trap, a missing callback, the
  sentinel jump `0xFFFFFFFF` and a barrier wrap do in OpenSherwood).

What this container contributes to those readings is only the corpus statistics: 208,679 instructions in 39
files, 42,734 native calls over 192 distinct ids, the largest class 6,948 instructions, at most six arguments
pushed before one call, and the opcodes that never occur (`0x00`, `0x09`, `0x10`, `0x16`, `0x17`, `0x1C`,
`0x1F`-`0x21`, `0x23`, `0x2A`, `0x2C`). The probes that produce them are in `harness/tools/probe/`.

## Index spaces

- **Elements (native 3)**: one flat table per level (established 2026-09-05, `sherwood-hub.md` section 4,
  from the self-references of object, actor and player-character classes over all 39 files; implemented in
  `crates/opensherwood-script`, `MissionBinding::from_mission`):

  ```
  [ map FLIM ] [ map TUPO ] [ POUF ] [ OILE ] [ TOTO ] [ BORG ] [ BOOM ] [ ZORG ] [ SKRO ] [ TING ] [ SCOT ] [ GULP polygons ]
  ```

  The map's `FLIM` animated elements occupy indices 0..FLIM-1 (confirmed in H01: indices 19..=32 are Lincoln's
  chimney fires, torches and candles, which the day mission deactivates at start and re-activates room by
  room), followed by the map's `TUPO` patches (medium: the counts fit all nine maps and native 5's per-map
  ranges agree; no script addresses a patch through native 3). The per-map prefix `K = FLIM + TUPO` is
  computed from the parsed `.rhp` (`map_element_count`): 19 Croisement01, 24 Croisement02 and Croisement03,
  20 Derby, 59 Nottingham, 63 Leicester, 50 Lincoln, 70 York, 20 Sherwood (`known_map_element_count` keeps
  the nine values as a cross-check, `crates/opensherwood-script/tests/gamedata.rs`). The mission's records
  follow in the chunk order above (the `ZORG` pick-up items before the `SKRO` scrolls, the file's own chunk
  order: `docs/original/h01-win-path.md` section 2, high: all 102 scroll-state calls 193 / 194 of the 18
  missions with both blocks land in the scroll range only with `ZORG` first, 27 otherwise, and the oracle
  shows two tutorial scrolls active at H01's start that the other order deactivated); the player-character
  slots (`SCOT`) sit at the *tail*, after the inert `TING` entries: four named `SCOT` classes address their own slot (Tac01 107, Emb03 107, Emb04 93,
  the outro 70, exact) and no mission's self-references fit `SCOT` in front (high). The position of the
  script polygons is not observable (no polygon class references itself by index); the engine puts them
  last. Under the earlier model (`SCOT` first, `K` read off one mission per map) eleven missions were bound
  with their records shifted by 1..=4: H02, H09, H12, H05, Str01, H03, S04, Str02, H10, S05, Str03. For H01:
  map elements 0..=49 (38 animated, 12 patches), civilians 50..=56, soldiers 57..=94, objects 95..=99, `ZORG`
  items 100..=110 (pick-up items, `Element::Item`: `rhm.md` "`ZORG`"), scrolls 111..=125, the hero's slot 126 (the element whose two attributes the level's
  `Initialize` zeroes), polygons 127..=138.
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
element-index resolution (index -> `.rhm` record) is **high** for the mission part of the table since
2026-09-05 (Index spaces: placed by the self-references of all 39 files) and **medium** for the map part
(the `FLIM` light check and the `TUPO` count fit),
so no statement that names a specific actor, scroll or zone by index is better than [M]. Only the statements
marked *observed* were checked against the original (`docs/original/campaign-flow.md`: the three briefing
pages, the initial objective, the camera on Robin afterwards). Everything else is *consistent with the
observed flow* at best; nothing in this section is "exact" until an oracle trace confirms it.

**Level class** (8 variables: a shooting timer, a run-once flag, two "sub-goal done" flags, a loop counter
and bound, an `Actor` scratch variable, a "going away" flag).

- `Initialize` [L]: disables five player actions (`n196(6..9, 0)`, `n196(3, 0)`; 196 is low); deactivates
  map elements 19..=32 [M] - in `lincoln.rhp` these `FLIM` entries are the two chimney fires, five torches and
  seven candles, i.e. the interior lights of a day mission (the room-entry messages below switch them on
  again room by room) - and seven `ZORG` pick-up items (102, 104..=106, 108..=110: the purses and arrows the
  tutorial scrolls hand out later; `docs/original/h01-win-path.md` section 2) [M]; zeroes the variables [H]; stores the player character (`n111()`) [M]; declares mission
  variables 1, 2, 3 [M]; locks doors 20 and 28 and closes doors 8, 20, 21, 37, 25, 23 [L: natives 4, 186,
  191]; deactivates, AI-locks and hides four actors (50, 79, 53, 92: the girl, a soldier, the servant, a
  soldier) that are activated by later events [L: 113 high, 134 medium, 197 / 198 low]; sets two attributes
  of element 126, the hero's own slot (Index spaces), to 0 [M: 117]; returns 0 [H].
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
  back [M]); 15 = after the drawbridge rope is cut, `n137(location 14, 1)` [L: 137 has an arity-only row].
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
(30 / 32 / 31 with blocking elements). Natives 130 and 137 are used by this mission and have arity-only
rows (effect unknown); 103 has one (low).

## Engine notes

The scheduler, the sequence machinery and the natives' effects are `docs/original/spec-script-vm.md` sections
3.5 to 3.8 and 6. Two rules of this engine are **not** the specification's and are recorded as assumptions
(ADR-0008) until the sections that cover them are cleared:

- the scheduler runs `Hourglass` on every class every logic frame with the frame counter and
  `CheckVictoryCondition` after it, instead of the specification's 25-frame cadence with `T / 25` (VM-103);
- a sequence runs element by element with completion tokens and a barrier, instead of the specification's
  levels with their abort cascade (VM-210 - VM-217); every decision the original's actors would make records
  `Assumption::ElementAdmission` or `Assumption::ElementDuration`.

Measured behaviour that belongs to the world rather than to the VM stays here: a scroll is read by an order,
never by proximity (`docs/original/h01-measurements-2.md` 1): a left click on the scroll's sprite walks the
selected player character to about 18 px short of it, a pause of 15 logic frames follows the arrival, then
`IsTaken(actor)` runs on the first class bound to the scroll (`World::vm_read_scroll`, from
`World::resolve_pickups`); a non-zero result takes the scroll (a hypothesis, `Assumption::ScrollPickup`). A
pick-up item is taken after a stoop of 14 logic frames and is then marked taken and deactivated
(`Assumption::ItemPickup`); no class is bound to an item. A passing walk takes nothing. Pinned by
`items_are_taken_on_a_click_and_native_235_reads_it`.

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

The container is done. What remains for the VM is in `docs/original/spec-script-vm.md`: the scheduler (3.5)
and the sequence machinery (3.7) are not cleared yet, and the ids of its 4.2 / 4.3 wait for the sibling
specifications that name their consumers. The community's Lua layer (Spellforge) replaces `.scb` with `.lua`,
so a Lua API compatible with theirs is the modding target; the SCB VM is needed only to run the retail
campaign unchanged.

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
