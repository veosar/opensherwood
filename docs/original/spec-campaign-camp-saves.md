# Campaign, Sherwood camp and saved games (behaviour specification)

Status: `draft`, **revision 2** (answers Codex spec review 19 on revision 1, reviewed blob
`6601ef353cb680df86e7b23613627c0cd6b032d5`; awaiting re-review).
Build: GOG English edition, `Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`,
image base 0x00400000; every address below is a virtual address in that image.
Analyst: session `a00ebf7dd67504358` (Opus, 2026-09-13), working in the git-ignored `re/` workspace.
Reviewer: Codex `gpt-6-astra`, spec review 19 (2026-09-13), verdict *redo*.
Publication approval: **pending**, maintainer, separate from factual approval.

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data or its code, no game text, and no prescribed
internal structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer
chooses the organisation. Where a value set inside the executable would have to be reproduced to make a feature
implementable, the feature is named in **section 11, "Excluded from clearance"**, and is *not* cleared for
implementation by this revision.

Sibling specifications, read first and **not** duplicated here: `spec-script-vm.md` (opcodes, natives, callback
ordering, the won / lost flags, objectives, the level tick), `spec-navigation.md`, `spec-ai-combat.md` (every
profile stat field, and **difficulty**, are settled there; this file defers to AI-045 and AI-020 and does not
re-derive them). Existing documents this one corrects: `docs/formats/profile.md`, `docs/formats/savegame.md`,
`docs/formats/sherwood-hub.md`, `docs/original/campaign-flow.md`.

---

## 0. Necessity record

**Interoperability target.** The player's own files: `Configuration/profile.cpf`, whose level table carries the
campaign graph and whose character-profile table carries the capability slots the camp's deployment test reads;
`Savegame/Profiles`, the player-profile archive that holds the key and graphic configuration and the list of save
slots; `Savegame/Profile_<nnn>/<name>` and `<name>_t`, the saved games and their thumbnails; `Campaign.bck` in
the installation root; the `SCOT` chunk of every mission file (`docs/formats/rhm.md`), which supplies the team
size limit and the per-slot capability requirements; and the camp's own compiled script
`Data/Levels/sherwood.scb`, whose natives 163-166, 170, 172-174, 195/196, 199/200, 232, 236/237, 239, 249/250,
256 and 261 mean nothing until the campaign object behind them is specified. Without this the engine cannot read
a retail save, cannot decide which mission is offered next, and traps on six natives the camp script reaches.

**Information not otherwise available, with the earlier attempts.** Data observation has run since 2026-09-02 and
established containers but not meaning:

- `docs/formats/profile.md` (analyst sessions 2026-09-02 and 2026-09-03, observation only) decoded the level
  table's *layout* but left every numeric field `unknown_*`, mis-split three field groups (findings CAMP-041 to
  CAMP-043 below), and closed its description of the graph with "whether the executable applies exactly these
  rules is not verified".
- `docs/formats/savegame.md` was a `stub` written from a hex dump: it guessed the save's campaign section as "a
  table of 32-byte records each containing the same 16-byte hash, a u32 index, a u32 value" and `Campaign.bck` as
  "a backup of the same table".
- `docs/formats/sherwood-hub.md` (2026-09-05, observation only) recovered the camp's element index space but had
  to mark 165/166/170/174/239/249 as natives with **no row at all**, read 195/196 as "availability of player
  action k", and recorded messages 1000 and 1001 as "no script in the 39 files sends them: they must come from
  the engine … the obvious candidates" — a guess it could not close.
- `docs/original/campaign-flow.md` is `observed` only up to the start of the first mission; everything later is
  marked *manual* / *inferred*.

Black-box play cannot separate the money window from the band-size window from the age counter from the
capability gate, because all of them fire on the same campaign day and produce the same visible result (a
mission is or is not offered); it cannot recover a save field that is written and then discarded; and it cannot
tell which of two counters the "spared lives" percentage divides.

**Scope read.** About 165 functions: the campaign module (00450ec0-0045a940, its console report block at
0045ad00 and 0045cda0), the mission-slot module (0054c680-0054cda0), the profile manager's level-table reader
and its mission-file pass (00564580, 00564e60, 00566570, 0056b8d0, 0056d190-0056d4c0), the player-profile
archive and its configuration serializers (0051ba00, 0051d4e0, 0051d5d0, 0051ec40, 0055c3c0, 0055d1a0, 0055dea0,
005b2190, 005fc2a0), the save-game manager (0056ea20-0056ff10, 00511340, 0050f1a0-0050f3d0), the campaign hooks
in the game loop and the level tick (0050e7b0, 0050e7d0, 0050e800, 0050ed40, 0050f710, 004e3220, 004e3260,
00514730), the camp production module (00456a10, 00456a40, 00456a70, 00580050-00581300, 00484360), the
campaign-map screen (00527d20, 005280d0, 00528810), the campaign natives (005791d0-0057bfe0), the mini-map dot
code (005788a0, 0046ffe0, 0048f280, 004a5c30, 004b1eb0, 005ef320) and the archive primitives (005e0c90,
005e0ea0, 005e1b70, 005f52b0).

**Honest stopping statement.** The *file grammars* are closed: a grammar written from this specification consumes
`Configuration/profile.cpf` (30,441 bytes), `Campaign.bck` (2,717 bytes), `Savegame/Profiles` (430 bytes) and
the campaign part of both retail saves to the exact byte on the fixtures of this installation. The *behaviour*
is **not** closed. Nine areas remain open (section 10) and four of them are load-bearing: the arithmetic that
turns one camp day into workshop output, the delivery order of the two camp messages relative to production, the
origin of the "send the team" message, and the expression that produces a recruit count. These are named in
section 11 and are **excluded from clearance**: this revision does not authorise implementing camp production,
nor any claim about message ordering.

**Analyst authorisation.** On behalf of the maintainer, on the maintainer's lawfully acquired copy.

---

## 1. Scope

This subsystem owns what survives a mission: the band of player characters, money, score, the statistics the
profile screen shows, the per-mission state of 63 campaign nodes, the camp's workshops, and the files all of it
lives in.

**Inputs.** The level table of `profile.cpf`; the `SCOT` chunk of each mission file; the *won* / *lost* flags of
the level tick (`spec-script-vm.md` VM-103) and the statistics gathered while a mission ran; the player's choices
on the campaign-map screen and in the camp; the selected player profile.

**Outputs.** Which mission loads next, which player characters are instantiated in it and with what health, the
HUD counters, the campaign values and money that mission scripts read through natives 195/196 and 236/237, the
offered-mission list the campaign map draws, and the contents of the save files.

**Shared state.** One campaign object exists while a campaign runs; outside a campaign natives 174, 234, 236 and
237 fail and the blazon display refuses to refresh. One player profile is current.

**Terminology (ours, chosen independently).**

| Term | Meaning |
|---|---|
| *node* | one record of the level table of `profile.cpf`; 63 of them, addressed by index 0..62 |
| *slot* | the campaign's mutable state for one node (age, blazon price, outcome) |
| *band* | the player characters the player owns |
| *team* | the subset of the band that goes on the next mission |
| *kind* | the node's mission kind: 0 story, 1 assault, 3 ambush, 4 camp, 5 defend, 6 tactical |
| *location* | the node's place: 1..3 the three forest crossings, 4..7 and 9 the five towns, 8 the camp |
| *blazon* | the counter the manual calls a coat of arms |
| *siege state* | a single signed byte of campaign state that gates the defend / assault branch (the program's own diagnostics use an internal name of their own for it, which is not reproduced) |
| *campaign day* | one pass camp -> mission -> camp |

---

## 2. Data model

Little-endian throughout. Widths and signedness are given per field, and refer to the **file**: no in-memory
layout is prescribed anywhere in this document.

### 2.1 The campaign object

**Counters** — 27 signed 32-bit values, index 0..26, all zero on a new campaign except index 1 (CAMP-010,
CAMP-011):

| # | Name (ours) | Unit | Meaning |
|---|---|---|---|
| 0 | `clovers` | count | the second HUD counter (the clover of `ui-flow.md` 9.3, icon `BTTN` 165). Read by the HUD and by one actor-action gate that refuses its action once the value exceeds 9. **No code path in the scope read writes it** (CAMP-016) |
| 1 | `money` | currency | the money the HUD shows and the availability test compares. Writing it plays a sound when it grows and refreshes the HUD |
| 2 | `score` | points | the score of the profile screen and the campaign-map status line |
| 3 | `blazons` | count | blazons held; section 3.6 |
| 4 | `spared` | actors | cumulative count of qualifying **hostile** actors left alive at the end of a won mission (CAMP-013) |
| 5 | `killed` | actors | cumulative count of the same actors found dead |
| 6 | `playtime` | seconds | accumulated play time |
| 7..26 | `script[0..19]` | free | the twenty values natives 195/196 expose as script indices 0..19 |

The script window is a plain offset: native 195(k) reads counter k+7 and 196(k,v) writes it, for k in 0..19; the
range check is applied to the shifted index (CAMP-012). Counter 1 is natives 236/237. Counter 3 is what native
178 adds a banner's value to and what native 234 compares.

**Scalars.**

| Name (ours) | Width | Meaning |
|---|---|---|
| `siege_state` | i8 | −1 on a new campaign (CAMP-030) |
| `recruit_flag` | u8 | set when a man returns from the recovery list; native 261 returns it; cleared in the post-victory bookkeeping |
| `previous`, `current`, `selected`, `blazon_node` | slot references | the mission played before, the mission being played (the camp is a node like any other), the mission the player picked in the camp, and the node the blazon counter belongs to |
| `town_result` | i32 | 0, or 1 / 2 = the last defend node resolved as won / lost, pending display |
| `town_code` | 4 bytes | the two-letter level code of that node — a **code, not an index** (CAMP-003) |

**Slots.** One per node, in level-table order, 63 of them (CAMP-020):

| Field | Width | Unit | Meaning |
|---|---|---|---|
| `node` | i32 | index | which level-table record this slot belongs to; −1 = none. In the retail files slot *i* points at node *i* |
| `age` | u16 | campaign days | how long the slot has been offered; reset to 0 whenever it leaves the offered list |
| `blazon_price` | u16 | currency | what one blazon costs for this node now; initialised from the node's *first blazon price*, raised by its *price step* after every purchase |
| `outcome` | u32 | — | 0 not played, 1 won, 2 lost |

**Lists.** All ordered; the order is part of the save (CAMP-021).

| Name (ours) | Elements | Meaning |
|---|---|---|
| `people` | character records | every player character the campaign has owned; the other people-lists hold positions in this one |
| `band` | positions in `people` | the men owned now; its length is the "gang size" of the availability test |
| `recovering` | positions in `people` | men out of action, from whom a recruit may return |
| `team` | positions in `people` | who goes on the next mission |
| `offered` | slot references | the missions the campaign map offers |
| `deferred` | slot references | **tactical** candidates held back while a blazon node is active (CAMP-260) |
| `workshops` | production records | exactly 13, one per workshop kind 0..12 |
| `spare_refs` | positions in `people` | empty in the retail fixtures; purpose unknown (section 10) |
| `labels` | wide strings | empty in the retail fixtures; purpose unknown (section 10) |
| `recent` | slot references | the last at most three missions played, oldest first |

### 2.2 One node: the level-table record of `profile.cpf`

`docs/formats/profile.md` documents the container. **Three corrections to its grammar**, each verified by
re-parsing the retail file to exact consumption (section 7, A1):

1. The group that document reads as `u8, u8, u16, u16` after the location is really `u8, u16, u8, u16`
   (CAMP-041). The mis-split is invisible in the retail values because the byte pattern coincides.
2. The count in front of the per-record index list is a **u32**, not a u16 (CAMP-042).
3. Consequently the trailing byte block is **10** bytes, not 12, followed by three signed bytes and seven u16
   (CAMP-043).

Field roles (names ours):

| Field (ours) | Width | Meaning |
|---|---|---|
| `code` | 4 bytes | the node's two-letter code; the key of the graph lists and of `Text/RHLevel??.red` |
| `map`, `mission_file`, `title` | strings | as in `profile.md` |
| `kind` | u32 | 0 story, 1 assault, 3 ambush, 4 camp, 5 defend, 6 tactical (CAMP-044). This is the value `spec-script-vm.md` calls the campaign node type; its VM-103 sub-flag is 0 for kinds 3 and 6 |
| `needs_camp` | u8 | 0 = may be started without going through the camp; 1 = chosen in the camp (CAMP-045) |
| `location` | u32 | 1..9; 8 = the camp (CAMP-046) |
| `min_money`, `max_money` | u32, u32 | the money window; compared **unsigned**; the value 200000 in `max_money` means "no upper limit" (CAMP-050) |
| `min_gang`, `max_gang` | u16, u16 | the band-size window, compared unsigned |
| `max_age` | u16 | the node's lifetime in campaign days |
| `obligatory` | u8 | non-zero = while this node is accessible it is the only offer (CAMP-063) |
| `prob` | u16 | a percentage used by the two random filters (3.3.2) |
| `priority` | u16 | lower wins when two candidates share a location (CAMP-065) |
| `required_pcs` | u32 count + u32 each | indices into the character-profile table of the same file: characters that must be in the team |
| `siege_gate[0..9]` | u8 × 10 | `siege_gate[0]` is a flag: not 1 means the node is not gated. When it is 1 the node is allowed only while `siege_gate[1 + siege_state]` is non-zero; because `siege_state` is −1 on a new campaign the expression reads `siege_gate[0]` itself, which is 1, so a gated node is allowed at the start (CAMP-031, CAMP-032) |
| `siege_on_win`, `siege_on_loss`, `siege_on_expiry` | i8 × 3 | the new `siege_state` after winning / losing this node, and after selecting it over-age; −1 = leave it alone (CAMP-033) |
| `m0`, `m1` | u16, u16 | no reader found in the scope read (section 10) |
| `blazons_needed`, `blazons_held` | u16, u16 | section 3.6 |
| `first_blazon_price`, `blazon_price_step` | u16, u16 | section 3.6 |
| `men_per_blazon` | u16 | how many men one blazon is worth; a zero value is diagnosed and the routine returns without dividing (CAMP-263) |
| `music_*` | strings | as in `profile.md` |

Two further per-node values are **not** in `profile.cpf`. They are read from the node's own mission file when the
level table is loaded (CAMP-047, 0056b8d0): the mission file is opened, its `SCOT` chunk located
(`docs/formats/rhm.md`) and

- `team_limit` := the number of `SCOT` records, i.e. the mission's player-character slot count. If the mission
  file cannot be opened, or has no `SCOT` chunk, a warning is reported and `team_limit := 5`.
- `capability_reqs` := a list built from the `SCOT` records: ten single-byte flags at offset 22 of each record
  are read in order and each set flag appends one capability requirement to the node's list. Duplicates are not
  removed, so a requirement appears once per slot that asks for it. After the ten flags comes one byte that,
  when non-zero, is followed by the slot's name as a byte string, and then one further byte. *The correspondence
  between flag position and capability id is a ten-entry mapping inside the executable and is excluded from this
  revision* (section 11). Codex's read-only inspection found non-zero requirement flags in **12 of the 39
  mission files, covering 16 slots** (CAMP-048), so this list is not empty in practice — revision 1's claim that
  it is empty for every node was wrong.

### 2.3 One person: the character record

What the file carries, in order (CAMP-070; the framing consumes the retail 98-byte record exactly):

| Field (ours) | Width | Meaning |
|---|---|---|
| block tag | 16 bytes | see 2.6 |
| `exp[0].level`, `exp[0].points`, `exp[1].level`, `exp[1].points` | u32 × 4 | two experience slots, each a level and a point remainder; the accumulator of 3.11 carries points into levels in hundreds and clamps the level at 100. The retail hero has levels 20 and 100 with no points (CAMP-072) |
| block tag | 16 bytes | see 2.6 |
| `health` | u16 | 0..100 |
| `unknown_a` | u8 | — |
| `unknown_b` .. `unknown_j` | u16 × 9 | — |
| `camp_slot` | u16 | the camp element the man is attached to; 0xFFFF = none |
| `name` | u16 count + that many 16-bit characters | the display name |
| `profile` | i32 | index into the character-profile table of `profile.cpf`; −1 = none |
| `deployed` | u8 | set when the man was instantiated in the current mission |

There are **no unattributed bytes**: the four 32-bit values come from a two-field loop that runs twice
(0051d5d0). Revision 1's instruction to manufacture an opaque base block from captured bytes is withdrawn.

**Name generation** (CAMP-071). A man whose character profile is marked a named hero takes his name from a text
resource: the profile's own designer name is matched against a fixed seven-name list inside the executable and
the matching position selects text id 0x90 + position; when it matches none, a fixed internal fallback string is
used and the condition is reported. A man who is not a named hero gets a generated name: one text id drawn from
100..121 and one from 122..143, joined by a single space. The generation runs inside a loop bounded at ten
iterations; the loop's exit condition and its behaviour when every attempt collides were **not** established,
and the fallback path means "the colliding name is kept" (revision 1) is not supported (section 10).

### 2.4 One workshop: the production record

Exactly 13, kind 0..12, in kind order. What the file carries (CAMP-090):

| Field (ours) | Width | Meaning |
|---|---|---|
| block tag | 16 bytes | see 2.6 |
| `kind` | u32 | 0..12 |
| `capacity` | u16 | the third argument of native 199 |
| `stock` | u16 | how much of the workshop's item the camp holds |
| `last_output` | u16 | what the last computed day produced |
| `full` | u8 | a derived capacity flag, recomputed as `5 × (number of work spots) <= stock` (CAMP-322) |
| `assignments` | u32 count + entries | per entry: an i32 position in `people`, two 32-bit coordinates, and a u16 sector reference (0xFFFF = none). From archive version 47 the u16 is the stored value; at version 46 a u16 is present, **read and discarded**, and the field is set to 0xFFFF; below 46 no u16 is present and the field is set to 0xFFFF (CAMP-093) |

The work *spots* registered by native 200 are **not** in the file; the camp script re-registers them on every
load (CAMP-091), and any importer must therefore reconstruct them by running the camp script's load path before
the assignments mean anything on the map.

### 2.5 The player profile

Per profile the engine keeps two key-configuration sets, a sound configuration, a graphic configuration, one
further coordinate pair, the player's name, the list of save slots, and six summary numbers plus the difficulty
(CAMP-100):

| Field (ours) | Source |
|---|---|
| `score` | counter 2 |
| `money` | counter 1 |
| `spared_pct` | 0 when counters 4 and 5 are both 0, otherwise a percentage of counter 4 over their sum. The updater converts a floating-point value; **the expression is absent from the export**, so it is not proved identical to the campaign map's integer form (CAMP-101) |
| `playtime` | `playtime += counter 6` |
| `progress` | the percentage of 3.7 |
| `unknown_x` | a 32-bit value, 1 in the retail fixture; no reader found |
| `difficulty` | the last of the seven serialized values: 0 easy, 1 medium, 2 hard, per `spec-ai-combat.md` AI-045, which also establishes that table B of `profile.cpf` is the ranged-weapon table and **not** a difficulty table (CAMP-280). The retail fixture holds 1, consistent with `campaign-flow.md`'s "Medium" |

### 2.6 The files

All three files use one stream format: fields in a fixed order, each record preceded by a **16-byte block tag**.
On reading, the tag is compared with a value the program derives internally; a mismatch produces a warning and
the load continues, and nothing downstream depends on it (CAMP-002). The tag values are content of the
executable and are not reproduced here.

**Compatibility requirement, resolved** (this replaces revision 1's self-contradictory policy): a reader must
accept *any* sixteen bytes in a tag position, and a writer may emit sixteen zero bytes — the original will load
such a file and log a warning per record. What matters is only that sixteen bytes appear in each tag position.
No cache of captured tag values is needed and none should be built.

Two string conventions: a byte string is a u16 count and that many bytes; a wide string is a u16 count and that
many 16-bit characters — **except** the player's name, which uses a u32 count (CAMP-004).

#### 2.6.1 `Campaign.bck` (installation root)

`u32 archive_version` followed by the campaign block of 2.6.2. The retail fixture is version 48, 2,717 bytes. It
is rewritten from the live campaign when the game loop starts a level in campaign mode (CAMP-120). Failure to
create it is reported and otherwise ignored.

#### 2.6.2 The campaign block

Field order; `V` = the archive version:

1. block tag.
2. `recruit_flag` — u8. Only when `V > 27`.
3. the 27 counters — 27 × i32.
4. `siege_state` — i8.
5. the slots: `u32 count`, then `count` slots, each: block tag, `u16 age`, `u16 blazon_price`, `u32 outcome`,
   **four bytes that are read and discarded**, `i32 node`.
6. `offered`: `u32 count`, then `count` × u16 slot index (0xFFFF = none).
7. `deferred`: the same.
8. `people`: `u32 count`, then `count` character records (2.3).
9. `band`: `u32 count`, then `count` × i32 position in `people`.
10. `recovering`: the same.
11. `team`: the same.
12. `workshops`: `u32 count`, then `count` production records (2.4).
13. `spare_refs`: `u32 count`, then `count` × i32.
14. `labels`: `u32 count`, then `count` wide strings.
15. `previous`, `current`, `selected`, `blazon_node` — four u16 slot indices, 0xFFFF = none, **in that order**
    (CAMP-023). The retail fixture reads (0xFFFF, 21, 0xFFFF, 0xFFFF): no previous mission, current mission
    slot 21, nothing selected, no blazon node.
16. only when `V > 29`: `recent`: `u32 count`, then `count` × u16 slot index.
17. only when `V >= 41`: `town_result` — i32.
18. only when `V >= 48`: `town_code` — 4 bytes (a level code).

On the four discarded bytes (CAMP-022): the writer emits whatever the four bytes of one stack location hold at
that moment and the reader throws them away. The two campaign blocks of each retail save differ there, which
shows the value is not a function of the campaign state, but the export does **not** establish what the bytes
are; revision 1's "uninitialised memory … whatever happened to be in a register" is withdrawn as unsupported.
An implementation may write zeros.

#### 2.6.3 A saved game (`Savegame/Profile_<nnn>/<name>`)

| Offset | Content |
|---|---|
| 0 | the four bytes `GSHR` |
| 4 | `u32 archive_version` (48 in the retail build) |
| 8 | `u32` holding the four bytes of the level code of the mission the save belongs to |
| 12 | `u32 archive_version` again |
| 16 | the **backup** campaign block — present only when `V > 38` |
| … | the **live** campaign block — always present |
| … | the level and actor state, written by a separate serializer (004c4570); not specified here |

Only the backup block is conditional (CAMP-124). On writing, the backup block is produced by streaming
`Campaign.bck` (minus its version dword) straight into the save; on reading it is deserialized like any campaign
block and then overwritten by the live block, so the live block wins. Practically the backup block is the
campaign as it was when the mission started, which is what makes a restart possible.

Header validation (CAMP-125): the magic is compared and a mismatch is reported, but a version dword differing
from the expected value **overwrites the rejection with success**, so a file with a wrong magic and a wrong
version is accepted. An importer must not copy that; OpenSherwood refuses any file whose magic is not `GSHR`
(section 8).

On the retail fixtures, with `V` = 48, the backup block begins at offset 16, the live block at 2,729 and the
level state at 5,442. Those offsets are properties of *these* fixtures, not of the format: the campaign block's
length varies with every count in it.

#### 2.6.4 The thumbnail

A second file whose name is the save's data-file name with `_t` appended, written immediately after the save, in
the format of `docs/formats/image-blob.md`; `docs/formats/savegame.md` records 160x120. It is a capture of the
screen and is not needed to load a save (CAMP-126).

#### 2.6.5 `Savegame/Profiles`

| Field | Width | Meaning |
|---|---|---|
| magic | 4 bytes | `FORP` |
| version | u32 | must be 6; otherwise the archive is discarded and default profiles are created (CAMP-110) |
| `next_dir` | u32 | the number the next profile directory gets (`Profile_%03i`) |
| count | u32 | number of profiles |
| profiles | — | `count` records as below |
| current | i32 | position of the selected profile; negative = none |

One profile record:

1. block tag.
2. seven u32 in this order: `unknown_x`, `score`, `money`, `spared_pct`, `playtime`, `progress`, `difficulty`
   (CAMP-111).
3. key set 1, key set 2: each a block tag, `u16 set_id`, then **29** u16 key codes.
   `Configuration/keyset1.cfg` and `keyset2.cfg` are exactly those 76 bytes and carry set ids 2 and 3
   (CAMP-112). This corrects `docs/formats/profile.md`, which read the 76 bytes as a header plus 30 codes.
4. sound configuration: a block tag, two u8, then five u16.
5. graphic configuration: a block tag, four single-byte settings whose meanings were not established, then two
   32-bit floats giving the screen width and height (CAMP-113 — the float pair the community resolution patcher
   edits at profile offset 0x104).
6. one further coordinate pair: two 32-bit floats.
7. the player's name: `u32 count`, then `count` 16-bit characters (at most 31).
8. the save-slot manager: a block tag, `u32 next_number` (the counter behind the numbered slot names),
   `u32 count`, then `count` slots, each a block tag, a byte-string *data file name*, a byte-string *thumbnail
   file name* and a wide-string *label shown to the player* (CAMP-114).

---

## 3. Behaviour

### 3.1 Start-up and a new campaign

The profile archive is read at start-up; a broken or missing archive is replaced by default profiles. A new
campaign is (CAMP-010, CAMP-200):

1. All 27 counters 0, then `money := 100`; `siege_state := -1`; the four slot references null; every list empty.
2. One slot per level-table record, in table order, with `age := 0`, `outcome := 0`,
   `blazon_price := ` the node's `first_blazon_price`.
3. The band: one man built from character-profile index **1** (the program asserts that the hero's two
   appearances are at indices 0 and 1 and refuses to run otherwise), not deployed. A command-line string can
   replace this with a longer band, each letter selecting one character profile by its designer name.
4. `team := band`.

**Level-table record 0 is the camp**; the accessibility scan starts at record 1, so the camp is never a candidate
(CAMP-201). Going to the camp means making node 0 the current mission.

### 3.2 When is a node accessible

For a slot `s` of node `n`, with `money` = counter 1 and `gang` = the length of `band`, all of the following must
hold, evaluated in this order (0054c800; the diagnostic twin 0054c8a0 names the first failure) (CAMP-060):

1. `money >= n.min_money`, unsigned
2. `money <= n.max_money`, unsigned, **or** `n.max_money == 200000`
3. `gang >= n.min_gang`, unsigned
4. `gang <= n.max_gang`, unsigned
5. `s.age < n.max_age`
6. `n.siege_gate[0] != 1` **or** `n.siege_gate[1 + siege_state] != 0`
7. every code in `n.after` names a slot whose `outcome != 0` — every prerequisite has been **played**, won *or*
   lost (CAMP-061)
8. every code in `n.until` names a slot whose `outcome == 0` — no blocker has been played (CAMP-062)

A code naming no node makes the lookup return the table's first slot; the retail data contains no such code.
Two consequences worth stating: a lost prerequisite satisfies the *after* list; and because every story node
lists itself in its own *until* list, a story node is gone for good once played.

### 3.3 Rebuilding the offered list

Rebuilding happens whenever the campaign needs the next mission **and also** when a defend node is resolved from
the camp (3.6), so it is not once per campaign day (CAMP-202). The rebuild (00451a90):

1. For every slot from index 1 to the end, in slot order: if the slot is accessible (3.2) and not already in
   `offered`, append it and set its `outcome` to 0.
2. Reduce `offered` (3.3.1); then, if `deferred` is non-empty, reduce `deferred` the same way except that steps
   c and d are skipped.

#### 3.3.1 The reduction

The reduction runs on a working copy taken from the source list, and **every outer attempt re-takes that copy**;
mutations to slot state (ages reset, outcomes set) made by a failed attempt persist (CAMP-203). At most ten
attempts:

| Step | Precondition | Effect |
|---|---|---|
| a | more than one candidate | **deadline / expiry.** Walk the list. If a candidate's node is a story node and `s.age == n.max_age - 1`, stop: that candidate becomes the whole list and every other candidate is removed with `age := 0`. Otherwise, if `s.age == n.max_age`, remove the candidate with `age := 0`, and if its node is a defend or assault node whose `outcome` is still 0, record it as **lost** (3.5) (CAMP-064) |
| b | more than one candidate | **obligatory.** Sort so that candidates with a non-zero `obligatory` come first. If the first candidate is obligatory and the second is not, the list becomes just the first and the rest are removed with `age := 0`. Two obligatory candidates at once is diagnosed and then ignored, leaving both (CAMP-063) |
| c | `offered` only, more than one candidate | the **assault / defend** random filter (3.3.2) |
| d | `offered` only | **blazon node and tactical deferral** (3.6) |
| e | more than one candidate | **recency.** Visit `recent` from newest to oldest. For each history entry, remove every candidate that is that mission, with `age := 0`; **stop as soon as fewer than two candidates remain.** A recent mission can therefore survive as the sole offer (CAMP-211) |
| f | more than one candidate | the **non-assault / defend** random filter (3.3.2) |
| g | more than one candidate | **one per location.** Sort by (map name, `priority`) ascending, then walk the sorted list keeping, per run of equal `location`, the candidate with the lowest `priority` and removing the rest with `age := 0`. Two candidates with the same location and the same priority are diagnosed and both retained. Because the sort key is the map name while the elimination compares the location identifier, this step does not guarantee one candidate per location for inputs where map and location disagree (CAMP-065) |

If the working list is non-empty the reduction is done and becomes the new source list. If it is empty, another
attempt starts from the (possibly mutated) source list. After ten attempts the program reports that it cannot
determine an accessible mission, prints the campaign state, reports that it is forcing a mission, restores the
working list from the source list, re-runs step d, and then: if the working list has fewer than two entries it is
used as it stands — **which may be empty** — otherwise one entry is drawn uniformly at random, every other slot
of the source list gets `age := 0`, and the drawn slot becomes the whole list (CAMP-204). The forced path
therefore cannot produce a mission from an empty source.

#### 3.3.2 The two random filters

Each filter is "remove those that match", over a copy of the list, followed by **restoring the copy if the list
became empty** (CAMP-212). A candidate matches when

- its node's kind is 1 or 5 (the first filter) or is neither 1 nor 5 (the second filter), **and**
- its `age` is 0, **and**
- `n.prob < rand() % 101`.

The draw is taken **only** for candidates that pass the kind and age tests, once each, in list order; a matching
candidate also gets `age := 0`. Determinism: the original draws from the C run-time generator; ours draws from a
named stream (section 8).

### 3.4 Choosing the next mission and the camp

When the campaign is asked for the next mission (004539f0) (CAMP-220):

1. If `selected` is set: `previous := current`; `current := selected`; `selected := null`. **No team reset.**
2. Otherwise: rebuild the offered list (3.3); then, for every offered candidate whose node is a defend node with
   `blazons_needed == 0`, record it as **won** (3.5) (CAMP-221); then `team := band` (CAMP-222 — the reset
   happens only on this path); then `previous := current` and
   - if `offered` holds **exactly one** candidate and that node's `needs_camp` is 0, `current :=` it;
   - otherwise `current :=` slot 0, the camp.
3. If `current` is not the camp: remove it from `offered` with `age := 0`, and append it to `recent`; if `recent`
   then holds more than three entries, drop the oldest.

This is the whole "Sherwood becomes reachable only after the second mission" rule: the first two story nodes have
`needs_camp == 0` and are each the only accessible node at their time, so they start directly; everything else
funnels through node 0 (CAMP-223, consistent with `campaign-flow.md` section 1).

**Selecting a mission in the camp** (004560d0) (CAMP-230). For each candidate in `offered`, in order:

- if it is the candidate the player picked: `selected :=` it, its `age := 0`, and it is removed from `offered`;
- otherwise: if the node's `max_age < 1000` **or** the candidate's `age` is 0, `age := age + 1`; then, if the
  *picked* candidate's `age` now exceeds the picked node's `max_age`, apply the picked node's `siege_on_expiry`
  (CAMP-231). This comparison reads the picked candidate, not the one being walked, and fires only while the
  picked candidate has not yet been reached; it is preserved here because it is what the program does.

The walk does not advance over the removed candidate, so candidates before and after the picked one both age, and
the picked one does not.

### 3.5 Recording an outcome

Recording an outcome on a slot `s` of node `n` (00456540) (CAMP-240):

| Outcome | Effect |
|---|---|
| won | `siege_state := n.siege_on_win` unless that byte is −1; `s.outcome := 1` |
| lost | `siege_state := n.siege_on_loss` unless that byte is −1; `s.outcome := 2`; **and if `s` is `blazon_node`, `blazons := 0`** |

In both cases, if `s` is `blazon_node` and `n.kind == 5`, the one-shot town notification is armed:
`town_code := n.code`, `town_result := s.outcome`. The campaign-map screen reads `town_result` and clears it
(CAMP-241).

**End of a mission** (004e3260, run by the level tick once the won / lost flag is set, `spec-script-vm.md`
VM-103), in this order (CAMP-250):

1. Record the outcome on `current` (3.5) — which, on a loss, can clear the blazon counter. So "nothing changes
   on a loss" is false.
2. Walk the level's non-player actor list in element order. Consider only actors of the soldier family on the
   hostile side (two tests: an element-kind test and a virtual side/family test that must answer 1). For each:
   if its health is at most 0 it counts as **killed**, otherwise it counts as **spared**, a per-mission spared
   counter on the level is bumped, and if the actor's profile carries one particular class value or the actor
   carries one particular flag it additionally contributes 70 score points. **These are enemies, not the
   player's men** (CAMP-013).
3. Walk the level's item list and collect what the rules there allow (the loot pass; not specified here). Steps
   1 to 3 run whatever the outcome.
4. If the mission was won: `spared += the spared count`; `killed += the killed count`;
   `score += the 70-point sum`; if `current`'s kind is not 3 (ambush), `score += 1000`; `recruit_flag := 0`;
   then add *k* men to the band (3.8) and record *k* on the level for the debriefing; if `blazon_node` is set,
   refresh the blazon display.
5. If the mission was lost: the level records that no recruits arrived, and nothing in step 4 happens.

**Score during a mission** (CAMP-251): `score += 100` when a player character's **experience level** for one slot
increases as a whole value (0049fc80 compares the level before and after the increase); `score += 50` when a
hostile actor transitions into death as the result of damage (004a5a00: damage is applied, then a virtual
side/family test must answer 1 and a virtual death test must be true). Only the *delta* form of a counter update
propagates into the level's mission-statistics block; a direct set does not (CAMP-252), so the statistics the
debriefing shows can differ from the counters.

### 3.6 Blazons

`blazons` belongs to one node at a time. Step d of the reduction (00455f00) (CAMP-260):

- **More than one candidate.** Clear `deferred`. `blazon_node` := the first candidate whose node kind is 1 or 5,
  or null. Then walk the candidates:
  - a candidate whose node kind is 1 or 5 and which is not `blazon_node` is removed with `age := 0`;
  - a candidate whose node kind is 6 (tactical) is, when `blazon_node` is null, removed with `age := 0`, and
    otherwise removed and appended to `deferred`;
  - every other candidate, and `blazon_node` itself, stays.
- **Exactly one candidate.** `blazon_node` := that candidate if its node kind is 1 or 5, else null. Nothing is
  removed.
- **No candidate.** `blazon_node` := null.

So `deferred` holds the tactical missions held back while an assault or defend node owns the blazon counter —
not, as revision 1 said, the other assault/defend candidates.

Four ways the counter moves (CAMP-261):

| Where | Effect |
|---|---|
| native 178, a banner captured in a mission | `blazons += the banner's value`. If the current node's kind is 1 and `blazons >= blazons_needed`, the mission is won at once |
| a tactical mission (kind 6) | the counter is capped at `blazons_needed - blazons_held` and the excess handed to a separate routine; when the mission is ending and `blazons >= blazons_needed`, `blazons := 0`, the blazon node is recorded as won and removed from `offered` |
| the campaign map, buying a blazon | `money -= blazon_price` of the blazon node's slot; `blazons += 1`; `blazon_price += n.blazon_price_step` |
| the camp or campaign map, spending blazons on a defend node | if `blazons >= blazons_needed`: `blazons -= blazons_needed`, the node is recorded as won, removed from `offered`, `selected := null`, and the offered list is **rebuilt** (3.3) |

The requirement the interface reports is `blazons_needed - blazons_held` of `blazon_node` when that differs from
`current`, and `blazons_needed` of `current` otherwise; native 234 answers "blazons >= that" (CAMP-262).

`men_per_blazon` converts men into blazons (00455a70): the routine first checks `men_per_blazon`, and when it is
zero it reports the condition and returns **without dividing**. Otherwise it computes
`men_per_blazon * min(gang_or_team_size / men_per_blazon, (u16)(blazons_needed - blazons_held) - blazons)` where
the `min` compares the two operands as 16-bit unsigned values and **there is no clamp to zero**: when the
requirement is already met or exceeded, the subtraction wraps and the result is large (CAMP-263).

### 3.7 Progress, score, spared percentage, game length

**Progress** in percent (00456db0) (CAMP-270). Count nodes whose kind is neither 3 nor 6 — ambushes and tactical
missions do not count, placeholders do — as `total`, and how many of those have `outcome == 1` as `won`. Then: if
any won node's code is one particular story code, the answer is **100**; else if any won node's code is one
particular defend code, the answer is **95**; else the answer is `won * 100 / total` as an **unsigned** division
**with no zero-denominator guard** — `total == 0` divides by zero. The two codes are four-byte constants
compiled into the executable; they are compatibility tokens, marked individually, and they are the two-letter
codes of the last story mission and of the last defend mission.

**Score** is counter 2 (3.5). **Spared percentage**: the campaign-map status line computes
`counter4 * 100 / (counter4 + counter5)` as an unsigned division, guarded to 0 when both are 0 (CAMP-101); the
profile updater takes the same guard and then converts a floating-point value whose expression the export does
not contain, so the two are not proved to agree. Because counters 4 and 5 count hostile actors (3.5), this
percentage is the share of enemies **left alive**, which is what makes it the game's "spared lives" statistic.

**Game length** is counter 6 in seconds. It accrues at save and transition checkpoints, not at level end
(CAMP-271): a start marker is set when a level begins or resumes, and whenever a save is written the campaign
adds `(tick now - start marker) / 1000` and **clears the marker**; the marker is then set again. One display path
adds the *unscaled* millisecond difference to the same value, which mixes units; that path feeds a version /
diagnostic line only (CAMP-272).

**Difficulty** is `spec-ai-combat.md` AI-045's: 0 easy, 1 medium, 2 hard, stored as the last of the profile's
seven summary values. No campaign routine in the scope read branches on it (CAMP-280).

### 3.8 Recruits

Adding a man (004524b0) (CAMP-290):

- If `recovering` is **empty**, no random draw is taken at all and a brand-new man is built from the
  character-profile table, considering only profiles that are not marked named heroes; the exact selection draw
  is absent from the export (section 10).
- Otherwise one draw is taken from the C run-time generator and its low bit examined. On one value the
  new-man path above runs. On the other, one entry of `recovering` is drawn uniformly and **recovered**: the
  recovery routine adjusts the man's experience state, clears his carried-item fields, sets `camp_slot` to
  0xFFFF, writes his health from a value whose source expression is absent (clamped to 100 — "re-rolled" is not
  supported), moves him into `band`, and sets `recruit_flag := 1` (CAMP-292).

The number of men added after a won mission is a value the level object supplies and truncates to an integer;
**the expression that produces it is absent from the export** and it is not established as a simple accessor
(CAMP-291, section 10). Men are added one at a time, so *k* recruits make *k* independent passes.

Men leave the band by being moved to `recovering` or by being dropped outright; both are driven by the HUD and
mission-end paths of 005145b0, which were not read.

### 3.9 The camp between missions

Entering the camp is entering node 0 as an ordinary mission on the `sherwood` map with its own script
(`docs/formats/sherwood-hub.md`). Around it the game loop does (0050f710) (CAMP-300):

1. Write `Campaign.bck` from the live campaign.
2. Create the *restart* auto-save — only when the level being started is **not** the camp (3.10).
3. If the level is the camp — the game object's "in the camp" flag, set from `current`'s `location == 8` — and
   the level is not in one particular state: submit **message 1001** to the camp level's script, then start the
   workshop pass (3.11).
4. Enter the frame loop.

Message 1001 is engine-originated and is submitted once, at camp start (CAMP-301) — this closes
`sherwood-hub.md`'s open question about where it comes from. **Its execution order relative to the workshop pass
and to the first tick is not established** (CAMP-303): the helper that submits it builds a script invocation and
hands it to the sequence scheduler, and the export dropped the call's arguments, so whether the handler completes
before the workshop pass, or is queued and drained during the first tick, is unknown. No normative statement in
this document depends on that order, and an implementer must treat it as unsettled (section 11).

The origin of **message 1000** ("send the team out") was not found (CAMP-302): the game loop does not submit it,
and native 239 (which opens the debriefing screen) does not establish it either. It is unknown, and no normative
statement here assumes otherwise.

**Team deployment.** The team is `team`; it is reset to `band` only on the unselected-next-mission path (3.4).

| Native | Effect |
|---|---|
| 163 | the length of `team` |
| 164 (`i`) | the element of `team[i]`; no bound check |
| 165 (`pc`) | refuses a non-player-character with a script error. Otherwise: append the character's record to `team` if it is not already there; refresh the HUD; then write three fields **of the actor**, one of which is set to the value 2. Those actor fields were not identified and are `unknown`; in particular 165 does **not** write the saved `deployed` flag (CAMP-310) |
| 166 (`pc`) | remove the character's record from `team`; refresh the HUD. Same refusal |
| 170 | 1 when the selected node's requirements are satisfied (below) |
| 172 | the selected node's four-byte code, 0 when nothing is selected — what the camp script compares to notice a pick |
| 174 | the selected node's `team_limit`, or 5 when nothing is selected; reports an error and answers 0 unless the **current** node's location is 8 (CAMP-311) |
| 249 | the number of **HUD-selected** player characters. This is the interface selection and must not be conflated with `team` (CAMP-314) |
| 250 (`i`) | the i-th HUD-selected player character; error and null at or above the count |

**Native 170's test** (00453cd0) (CAMP-312), in this order:

1. For every entry of the node's `required_pcs`: some member of `team` must have that character profile
   (identity comparison of the profile the member points at).
2. For every entry of the node's `capability_reqs`: some member of `team` must have that capability. A member
   has capability *c* when *c* appears in either of two capability arrays of his character profile — one of
   three entries and one of four — or when one of three substitutions applies: a requirement for capability 2 is
   also satisfied by 3 in the three-entry array, a requirement for 13 by 14 in the three-entry array, and a
   requirement for 25 by 26 in the four-entry array (CAMP-315).

Both loops stop at the first unsatisfied entry.

**Instantiating the team.** When a mission starts, each team member gets a player-character actor, his record is
marked `deployed`, and his `camp_slot` keeps a value only while the current node's location is 8, otherwise it is
set to 0xFFFF (CAMP-313). A node whose `required_pcs` cannot be matched to a spawn point produces a warning and
the spawn point stays empty.

### 3.10 Save slots and the auto-saves

The executable knows five fixed slot names — `Continue`, `Restart`, `QuickSave`, `ExQuickSave`, `Sherwood` — plus
numbered slots named from a `Savegame_%03i` pattern whose counter is the profile's `next_number` (CAMP-130).
Labels shown to the player come from text resources (0xF6 and 0xF7 for the *continue* and *restart* slots), so a
slot's file name and its label differ.

| Slot | Written when |
|---|---|
| `Restart` | at the start of every level that is not the camp (CAMP-131) |
| `Sherwood` | at the start of the camp level (CAMP-132) |
| `Continue` | after every successful save, the file just written is copied to `Continue` — unless two tests (which were not read individually) say the file *is* the continue or restart slot (CAMP-133) |
| `QuickSave`, `ExQuickSave` | the quick-save key; the older quick save moves to `ExQuickSave` |
| numbered | the save menu |

Writing a save (00511340): open the data file for writing; write the header; add the elapsed interval to
counter 6 and clear the start marker; when `V > 38` write the backup block by streaming `Campaign.bck` minus its
version dword; write the live campaign block; write the level state; set the start marker again; write the
thumbnail file; mirror into the continue slot; show a message from a text resource. Loading is the same routine
with the direction reversed: the backup block (when present) is deserialized, then the live block, then the level
state (CAMP-134). "Restart" and "Continue" are therefore not separate mechanisms but ordinary saves under fixed
names.

### 3.11 The workshop pass

The 13 workshops are addressed by kind and are what natives 199 and 200 configure (CAMP-320):

- **199 (k, location, capacity)**: workshop `k` takes that location as its zone and that capacity, and the
  location is back-linked to the workshop kind.
- **200 (k, location)**: a work spot is appended to workshop `k`'s non-persistent spot list, carrying the
  location's coordinates, one of its identifiers, and the index of its sector or 0xFFFF when it has none.

The pass runs over all 13 workshops in kind order and is handed `previous`. **Victory gates only the output
calculation, not the pass** (CAMP-321): the routines that compute a day's output for kinds 0..11 each begin by
requiring `previous` to be non-null with `outcome == 1` and do nothing otherwise, while item placement (kinds
0..8), kind 12's routine and the placement of assigned characters run regardless.

| Kind | What the pass does |
|---|---|
| 0..8 | *gated by victory:* compute the day's output, `stock += output`, `last_output := output`. *Ungated:* walk the work spots and place items in the camp, up to 5 per spot, drawing down a **temporary** quantity initialised from `stock`; `stock` itself is not reduced by placement. Then recompute `full := 5 * (number of work spots) <= stock` (CAMP-322) |
| 9, 10 | *gated by victory:* compute the day's output and add it to **each assigned character's** experience — slot 0 for kind 10, slot 1 for kind 9, and kind 9 additionally requires that character's profile to carry one particular capability in its three-entry capability array (CAMP-323) |
| 11 | *gated by victory:* compute the day's output and **restore health** by that amount to every assigned character, capped at 100. Kind 11 is the rest workshop, not a training workshop (CAMP-324) |
| 12 | a separate routine, ungated, not read (CAMP-329) |

Then, for every workshop, each assigned character is placed at his recorded spot in the camp level (CAMP-328).

The experience accumulator (CAMP-072, 0051d4e0): `points[slot] += amount`; then, while `level[slot] < 100` and
`points[slot] >= 100`, `level[slot] += points[slot] / 100`, `points[slot] %= 100`, and `level[slot]` is clamped
to 100.

A **separate campaign pass** (00456a70) captures the camp back into the campaign: for kinds 0..8 it recounts
`stock` from the camp level — summing the quantity of every **active** pick-up element whose item id matches the
workshop's, plus, for one particular item id, certain actors — and for every kind it rebuilds the `assignments`
list by scanning the camp level's player characters that pass an eligibility test, recording each one's record,
position and sector (CAMP-326). This pass has no direct caller in the export; when it runs was not established
(section 10).

**The day's output itself is not specified.** Each of the four output routines truncates a floating-point value
to an integer, and the expression that produces it — including the part played by the routine that locates the
workshop's supervising character, whose per-kind identity mapping is itself withheld (section 11) — is absent
from the export. Section 11 excludes camp production from
clearance.

### 3.12 The campaign-map screen

Ten location slots, indices 0..9; index 0 and index 8 (the camp) are skipped, and for indices 1..3 (the forest
crossings) one of the per-location widgets is skipped (CAMP-330). Two further widget groups are driven by the
current `siege_state`. A location's miniature shows the offer for that location; by construction of step g there
is normally at most one. An empty offered list produces a text and a reported warning (CAMP-331). A status line
composed from text resources 0xF3, 0xF4 and 0x40 carries `money`, `score` and the spared percentage (CAMP-332).
The buttons are the purse, recruit, cart and tower/shield actions of `ui-flow.md` (`BTTN` 71-74 here, 157-160 in
the camp). The pending town notification is read here and cleared (CAMP-241).

### 3.13 HUD and mini-map

- The two top-left counters are `money` and `clovers`, each formatted with its own text resource (0xF4 and 0xF5)
  (CAMP-340).
- The blazon display refresh reports an error and does nothing when there is no blazon node, which includes every
  non-campaign game (CAMP-341).
- **Mini-map colours.** Each element carries a dot style code and several colour entries. A player character's
  entries are set to light green, RGB (165, 255, 82). A hostile human's are set to red, RGB (255, 0, 0) — or, when
  one flag on his controller is set, to violet, RGB (115, 40, 203) and nothing else — and in the red case two
  further entries are set to yellow, RGB (255, 255, 0), and blue, RGB (0, 120, 255); **five entries in total,
  more than revision 1 claimed**, and which entry is used in which actor state was not established. One further
  element class is cyan, RGB (0, 190, 255), and another pale yellow, RGB (255, 247, 90) (CAMP-350).
- **Native 24** sets the style code. Accepted codes: 0, 1, 100, 101, 102, 111, 200, 201, 202, 222, 300, 301, 302,
  333, 500 and 666; anything else is an error that changes nothing. Codes 111, 222 and 333 additionally require
  the element to belong to the human family and report an error otherwise. Of the accepted codes, 100/101/102 and
  111 set light green, 200/201/202 and 222 red, 300/301/302 and 333 cyan; **0, 1, 500 and 666 change the style
  code only and leave the colour entries as they are, so they do not restore a colour a previous call
  overrode** (CAMP-351).
- Only the calls of one colour-conversion helper were examined. Among those calls no grey value appears; that is
  the whole of the evidence, and it does **not** establish that no grey dot can be drawn. State-dependent
  rendering — which of an element's colour entries is used, and whether an un-blipped element is drawn at all —
  is unspecified (CAMP-352).
- Portrait states, action-icon availability and tooltips were **not** read; `ui-flow.md` has the geometry. The
  name under a portrait is the `name` field of 2.3.

---

## 4. Claims

Rows whose id carries **(R19-n)** answer finding *n* of spec review 19.

| Id | Claim | Status | Evidence | Confidence | Notes |
|---|---|---|---|---|---|
| CAMP-002 | Each record is preceded by a 16-byte block tag that is compared on reading; a mismatch only logs and the load continues | observed | 005e1b70, 0060f1c0, 0060f1e0 | high | **(R19-27)** |
| CAMP-003 | The town notification stores the node's four-byte level code, not an index | observed | 00456540, 00452f20, 005280d0 | high | **(R19-3)** |
| CAMP-004 | Byte and wide strings use a u16 count; the player's name uses a u32 count | observed | 005e0c90, 005e0ea0, 0055d1a0 | high | validated on the profile archive |
| CAMP-010 | A new campaign has all 27 counters 0 except money = 100 | observed | 00450ec0, retail backup file | high | |
| CAMP-011 | The counters are 27 × i32 | observed | 00452030, 00452f20 | high | |
| CAMP-012 | Natives 195/196 address counters 7..26 as script indices 0..19 | observed | 00579430, 00579470 | high | |
| CAMP-013 | Counters 4 and 5 count qualifying **hostile soldier-family** actors left alive and found dead, not the player's men; the 70-point bonus likewise concerns qualifying surviving hostiles | observed | 004e3260 (element-kind test plus a virtual side/family test), 004a1bc0 | medium-high | **(R19-18)**; the two virtual tests were not followed, so "qualifying" is not fully characterised |
| CAMP-016 | Counter 0 is the clover HUD counter and no writer appears in the scope read | observed | 004c8140, 004a78c0 | medium | how it becomes non-zero is open |
| CAMP-020 | A slot carries age, blazon price, outcome and a node reference | observed | 0054cc40, 00452f20 | high | |
| CAMP-021 | Every campaign list is a u32 count followed by its elements in order | observed | 00452f20, 00453630, 00458be0, 00458e70, 00459160, 004593f0 | high | |
| CAMP-022 | Each slot record contains four bytes that are written from one stack location and discarded on reading | observed | 0054cc40; the two campaign blocks of each retail save differ there | high for the framing, low for the origin | **(R19-5)**; no claim about what the bytes are |
| CAMP-023 | The four slot references are written previous, current, selected, blazon node | observed | 00452f20 (order), 004539f0 (previous is assigned from current) | high | **(R19-1)**; revision 1 reversed the first two |
| CAMP-030 | The siege state is one signed byte, −1 on a new campaign | observed | 00456e40, 00456e60, retail file | high | |
| CAMP-031 | A node is gated only when the first byte of its ten-byte gate array is 1 | observed | 0054c800 | high | |
| CAMP-032 | The gate reads `gate[1 + state]`, so state −1 reads the enable flag itself and passes | observed | 0054c800 | high | |
| CAMP-033 | Win, loss and over-age selection each set the state from one signed byte, −1 meaning no change | observed | 00456540, 004560d0, 00456e40 | high | |
| CAMP-041 | The level record's group after the location is u8, u16, u8, u16 | observed | 00566570 | high | corrects `profile.md` |
| CAMP-042 | The required-character list is counted by a u32 | observed | 00566570 | high | corrects `profile.md` |
| CAMP-043 | The trailing block is 10 bytes, then 3 signed bytes, then 7 u16 | observed | 00566570 | high | |
| CAMP-044 | The node kind is 0 story, 1 assault, 3 ambush, 4 camp, 5 defend, 6 tactical | observed | 005795c0, 00514730, 0054cbf0, 0054cc20, 00456db0 | high | |
| CAMP-045 | A node whose `needs_camp` byte is 0 may start without the camp | observed | 004539f0 | high | |
| CAMP-046 | Location 8 is the camp and several routines gate on it | observed | 00579300, 0050b640, 004552f0 | high | |
| CAMP-047 | `team_limit` is the number of `SCOT` records of the node's mission file, or 5 when the file or chunk is missing (with a warning) | observed | 0056b8d0 | high | **(R19-6)** |
| CAMP-048 | `capability_reqs` is built from ten flag bytes at offset 22 of each `SCOT` record; each set flag appends one capability requirement, duplicates included | observed | 0056b8d0 | high | **(R19-6)**; 12 of 39 mission files carry them, covering 16 slots (Codex's read-only inspection). The flag-to-capability correspondence is excluded (section 11) |
| CAMP-050 | 200000 in the maximum-money field means "no maximum"; the money and gang comparisons are unsigned | observed | 0054c800, 0054c8a0 | high | **(R19-10)** |
| CAMP-060 | The availability test is the eight conditions of 3.2, in that order | observed | 0054c800, 0054c8a0 | high | |
| CAMP-061 | The *after* list requires each prerequisite to have been played, won or lost | observed | 0054c6f0 | high | contradicts `profile.md` |
| CAMP-062 | The *until* list requires each blocker to be unplayed | observed | 0054c6f0 | high | |
| CAMP-063 | Obligatory filtering needs more than one candidate; obligatory means a non-zero flag; two obligatory candidates are diagnosed and both retained | observed | 00452e70 | high | **(R19-10)** |
| CAMP-064 | A story candidate at `max_age - 1` becomes the sole offer; at `max_age` a candidate is removed, and a removed defend/assault candidate with outcome 0 is recorded lost | observed | 004526f0 | high | requires more than one candidate |
| CAMP-065 | The one-per-location step sorts by (map name, priority) and eliminates by comparing the location identifier, so it guarantees one per location only when map and location agree; equal location and priority are diagnosed and both retained | observed | 00452d20, 0054c780 | high | **(R19-10)** |
| CAMP-070 | The character record is: tag, four u32 (two experience level/point pairs), tag, health, one u8, nine u16, camp slot, wide name, profile index, deployed flag — with no unattributed bytes | observed | 0051d5d0 (a two-field loop running twice), 0055c3c0, 0055bb30; exact re-parse of the retail 98-byte record | high | **(R19-2)** |
| CAMP-071 | Generated names draw one text id from 100..121 and one from 122..143, joined by a space, inside a loop bounded at ten; a named hero's name is text 0x90 + his position in a seven-name list, with a fixed fallback when he matches none | observed | 0055bf00, 0055bdf0 | medium | **(R19-23)**; the loop's exit condition is open |
| CAMP-072 | An experience slot is a (level, points) pair; adding points carries into the level in hundreds and clamps the level at 100 | observed | 0051d4e0 | high | |
| CAMP-090 | A workshop record carries kind, capacity, stock, last output, a capacity flag and an assignment list | observed | 00580970, 00580ab0 | high | |
| CAMP-091 | Work spots are not saved; the camp script re-registers them each load | observed | 00580220, 00580970 | high | |
| CAMP-093 | An assignment's trailing u16 is stored from archive version 47; at version 46 a u16 is read and discarded and the field becomes 0xFFFF; below 46 there is none | observed | 00580ab0 | high | **(R19-17)** |
| CAMP-100 | The profile's serialized summary is six numbers plus the difficulty | observed | 0055d370, 0055d1a0, retail archive | high | |
| CAMP-101 | The campaign-map spared percentage is an unsigned integer division of counter 4 by the sum, guarded to 0 when both are 0; the profile updater applies the same guard and then a floating-point conversion whose expression is absent | observed | 00528810, 0055d370 | high for the map, medium for the profile | **(R19-21)** |
| CAMP-110 | The profile archive begins with `FORP` and a u32 version that must be 6 | observed | 0055dea0, retail file | high | |
| CAMP-111 | The profile's seven u32 are unknown, score, money, spared, play time, progress, difficulty | observed | 0055d1a0, exact re-parse | high | |
| CAMP-112 | A key configuration is a tag, a u16 set id and 29 u16 codes; the two `.cfg` files are exactly that, with ids 2 and 3 | observed | 0051ec40, both files | high | corrects `profile.md` |
| CAMP-113 | The graphic configuration is a tag, four single-byte settings and two floats giving width and height | observed | 0051ba00, 005fc2a0, retail file | high | **(R19-26)**; no in-memory order stated |
| CAMP-114 | A save slot is a tag, a data-file name, a thumbnail name and a wide label | observed | 0056ea20, 0056ff10, retail file | high | |
| CAMP-120 | `Campaign.bck` is a u32 version plus one campaign block, rewritten at every level start in campaign mode | observed | 00457560, 0050f710 | high | |
| CAMP-124 | Only the backup block is conditional on `V > 38`; the live block is unconditional. On writing, the backup block is streamed from `Campaign.bck`; on the retail fixtures the blocks begin at 16 and 2,729 and the level state at 5,442 | observed | 00511340, 004576f0; byte comparison of both retail saves against the backup file | high | **(R19-4)**; the offsets are fixture properties |
| CAMP-125 | The magic is compared, but a version dword differing from the expected value overwrites the rejection with success | observed | 0056ea70 | medium-high | **(R19-4)**; OpenSherwood does not copy this |
| CAMP-126 | The thumbnail is a separate `<name>_t` file in the image-blob format | observed | 00511340, 0056e6b0, retail files | high | |
| CAMP-130 | Five fixed slot names plus numbered slots from a pattern and a counter in the profile | observed | 0056ef30, 0056f090, 0056f1f0, 0056f350, 0056f4b0, 0056fa30 | high | |
| CAMP-131 | The restart slot is written at the start of every non-camp level | observed | 0050f2d0, 0050f710 | high | |
| CAMP-132 | The Sherwood slot is written at the start of the camp level | observed | 0050f3d0 | high | |
| CAMP-133 | Every successful save is mirrored into the continue slot unless two unread tests say it already is one of the two auto-slots | observed | 0050f1a0, 00511340 | medium | |
| CAMP-134 | Save and load are one routine with a direction flag; on load the backup block (when present) is applied first, then the live block | observed | 00511340 | high | |
| CAMP-200 | A new campaign's band is one man from character-profile index 1; a command-line string can replace it | observed | 00450ec0, 0048f280 | high | |
| CAMP-201 | Level-table record 0 is the camp; the scan starts at record 1 | observed | 00451a90, 004539f0 | high | |
| CAMP-202 | Rebuilding is not once per day: resolving a defend node from the camp rebuilds within the camp. A candidate newly appended has its outcome reset to 0 | observed | 00451a90, 00514730 | high | **(R19-11)** |
| CAMP-203 | The reduction runs on a copy re-taken from the source at every attempt, up to ten attempts, steps a..g; slot mutations of a failed attempt persist | observed | 00451b70 | high | **(R19-7)** |
| CAMP-204 | After ten attempts the campaign diagnoses, restores the working list, re-runs step d, and either uses a working list of fewer than two entries as it stands — possibly empty — or draws one entry uniformly | observed | 00451b70, 00456e60, 00457a30 | high | **(R19-7)**; it cannot guarantee a mission |
| CAMP-211 | Recency visits history newest to oldest and stops once fewer than two candidates remain, so a recent mission can survive as the sole offer | observed | 00452c90 | high | **(R19-8)** |
| CAMP-212 | Each random filter removes matching candidates over a copy and restores the copy when the list becomes empty; the draw is taken only for candidates that pass the kind and age tests, once each, in list order; a removed candidate also gets age 0 | observed | 00452040, 00452090, 00452810, 00452a50 | high | **(R19-7)** |
| CAMP-220 | The next mission is the selected one, else the single offer whose node does not need the camp, else the camp | observed | 004539f0 | high | |
| CAMP-221 | Before choosing, every offered defend candidate whose blazon requirement is 0 is recorded won | observed | 0054cda0 | medium | no retail defend node has a 0 requirement |
| CAMP-222 | The team is reset to the band only on the unselected-next-mission path | observed | 004539f0, 00455650, 004559f0 | high | **(R19-11)** |
| CAMP-223 | The first two story nodes start without the camp because each is the only accessible node and has `needs_camp` 0 | inferred from CAMP-045, CAMP-060 and the retail data | 004539f0 | high | matches `campaign-flow.md` |
| CAMP-230 | Selecting in the camp ages every other offered candidate by one and zeroes the picked one's age | observed | 004560d0 | high | |
| CAMP-231 | A candidate whose node's maximum age is 1000 or more only ages from 0 to 1; the over-age expiry comparison reads the picked candidate | observed | 004560d0 | high | preserved as observed |
| CAMP-240 | A win sets outcome 1 and the win byte; a loss sets outcome 2, the loss byte, and zeroes the blazon counter when the slot is the blazon node | observed | 00456540 | high | |
| CAMP-241 | A resolved defend blazon node arms a one-shot notification (code plus result) that the campaign-map screen reads and clears | observed | 00456540, 005280d0, 004577a0, 004577b0 | high | |
| CAMP-250 | Mission-end bookkeeping records the outcome and runs the actor and loot passes whatever the outcome; only the counter and recruit work is gated on victory. A loss therefore can still clear the blazon counter | observed | 004e3260, 00456540 | high | **(R19-20)** |
| CAMP-251 | Score gains 1000 for a won non-ambush mission, 70 per qualifying surviving hostile, 100 when a player character's experience level increases as a whole value, and 50 when a hostile dies from damage | observed | 004e3260, 0049fc80, 00485200, 004a5a00, 0047e9a0 | medium-high | **(R19-19)** |
| CAMP-252 | Only delta updates of money and score propagate into the level's statistics block; direct sets do not | observed | 00450b60, 004520e0, 004e3cc0 | high | **(R19-19)** |
| CAMP-260 | Step d: the blazon node is the first candidate of kind 1 or 5; other kind-1/5 candidates are removed; **tactical** candidates move to the deferred list when a blazon node exists and are removed otherwise; with exactly one candidate nothing is removed | observed | 00455f00, 0054cbf0, 0054cc20 | high | **(R19-9)**; revision 1 misidentified the deferred list |
| CAMP-261 | The four ways the blazon counter moves (3.6) | observed | 005795c0, 00514730, 00525040 | high | |
| CAMP-262 | The reported requirement is needed − held for the blazon node and needed for the current node | observed | 00455af0, 00579690 | high | |
| CAMP-263 | A zero `men_per_blazon` is diagnosed and the routine returns before dividing; otherwise the conversion compares 16-bit unsigned operands and has no clamp to zero, so an already-met requirement wraps | observed | 00455a70 | high | **(R19-22)** |
| CAMP-270 | Progress is 100 or 95 on two particular won level codes, else won × 100 / total over nodes of kind other than 3 and 6, unsigned and without a zero-denominator guard | observed | 00456db0 | high | **(R19-21)** |
| CAMP-271 | Play time accrues at save and transition checkpoints from an active interval, clearing the start marker | observed | 0050e7b0, 0050e800, 00511340, 00512310 | high | **(R19-21)** |
| CAMP-272 | One diagnostic display path adds unscaled milliseconds to the seconds value | observed | 0050e7d0 | high | |
| CAMP-280 | Difficulty is the profile's last serialized summary value, 0 easy / 1 medium / 2 hard, per `spec-ai-combat.md` AI-045; no campaign routine in the scope read branches on it | observed (deferred) | AI-045; 0055d1a0 | high | **(R19-20)**; table B is the ranged-weapon table (AI-020) |
| CAMP-290 | With an empty recovery list no draw is taken and a new man is generated; otherwise one draw's low bit selects between generating and recovering | observed | 004524b0 | high | **(R19-23)** |
| CAMP-291 | The number of men added after a victory comes from the level and is truncated to an integer; the expression is absent from the export | observed for the truncation, unknown for the expression | 004e3260 | low | **(R19-20)** |
| CAMP-292 | Recovery adjusts experience, clears carried-item fields, sets the camp slot to 0xFFFF, writes health from an absent expression clamped to 100, moves the man to the band and sets the recruit flag | observed except the health source | 00455bf0, 0055c390 | medium | **(R19-23)** |
| CAMP-300 | At every level start in campaign mode: write the backup file, make the restart auto-save when the level is not the camp, and in the camp submit message 1001 and run the workshop pass | observed | 0050f710 | medium-high | the campaign-mode test itself is folded (section 10) |
| CAMP-301 | Message 1001 is engine-originated and submitted once at camp start | observed | 0050f710, 00578d80 | high | closes `sherwood-hub.md`'s question |
| CAMP-302 | The origin of message 1000 is unknown | unknown | 0050f710 does not submit it; the camp handler exists | — | **(R19-13)** |
| CAMP-303 | The execution order of message 1001 relative to the workshop pass and the first tick is unknown: the submitting helper builds a script invocation and hands it to the sequence scheduler, and its arguments are absent | unknown | 00578d80, 0058a940 | — | **(R19-13)** |
| CAMP-310 | Native 165 appends the record without duplication and refreshes the HUD, then writes three unidentified actor fields, one of them to 2; it does not write the saved deployed flag. 166 removes and refreshes | observed | 00579210, 00579280, 00455400, 004555f0 | high for the list and HUD, unknown for the actor fields | **(R19-12)** |
| CAMP-311 | Native 174 errors and answers 0 unless the current node's location is 8 | observed | 00579300 | high | |
| CAMP-312 | Native 170 requires every required character profile and every capability requirement to be met by some team member, stopping at the first failure | observed | 00453cd0, 00457480, 004574b0 | high | |
| CAMP-313 | A team member's camp slot survives only while the current node's location is 8 | observed | 004552f0 | medium | |
| CAMP-314 | Native 249 counts HUD-selected characters, which is not the team | observed | 0057b5e0, 0057b5f0 | high | **(R19-13)** |
| CAMP-315 | A capability is satisfied by a three-entry or a four-entry capability array of the character profile, with three substitutions: 2 by 3 and 13 by 14 in the three-entry array, 25 by 26 in the four-entry array | observed | 004574b0 | high | **(R19-6)**; three individual facts |
| CAMP-320 | Native 199 sets a workshop's zone and capacity and back-links the location; native 200 appends a work spot with coordinates and a sector reference | observed | 005799d0, 00579a00, 00580330, 00580220, 00456a10 | high | |
| CAMP-321 | Victory gates only the output calculation for kinds 0..11; item placement, kind 12 and character placement run regardless | observed | 00456a40, 00580d20, 00581020, 005810c0, 00581180, 00580b70, 00580e80, 00580dc0 | high | **(R19-14)** |
| CAMP-322 | Placement draws down a temporary quantity and does not reduce `stock`; afterwards the capacity flag is recomputed as `5 × spots <= stock` | observed | 00580b70 | high | **(R19-15)** |
| CAMP-323 | Kinds 9 and 10 add their output to each assigned character's experience, slot 1 and slot 0; kind 9 additionally requires one capability in the character profile's three-entry array | observed | 005810c0, 0051d4e0, 0055bb00 | high | **(R19-16)** |
| CAMP-324 | Kind 11 restores health to each assigned character, capped at 100 | observed | 00581180, 00484360 | high | **(R19-16)** |
| CAMP-326 | A separate pass recounts `stock` for kinds 0..8 from **active** matching pick-up elements (plus certain actors for one item id) and rebuilds every workshop's assignment list from the camp's player characters | observed | 00456a70, 005804e0, 00580430, 00580520 | high for the content, unknown for when it runs | **(R19-15)** |
| CAMP-328 | After the pass each assigned character is placed at his recorded spot | observed | 00580dc0 | high | |
| CAMP-329 | Kind 12's routine was not read | unknown | 00580e80 | — | **(R19-17)** |
| CAMP-330 | The campaign map lays out ten location slots, skipping 0 and 8, with one widget fewer for locations 1..3 | observed | 00527d20 | medium | |
| CAMP-331 | An empty offered list on the campaign map yields a text and a reported warning | observed | 00527d20 | high | |
| CAMP-332 | The status line carries money, score and the spared percentage from text resources 0xF3, 0xF4 and 0x40 | observed | 00528810 | high | |
| CAMP-340 | The two HUD counters are money and clovers, each with its own text resource | observed | 004c8140 | high | |
| CAMP-341 | The blazon refresh errors and does nothing without a blazon node | observed | 00514730 | high | |
| CAMP-350 | A hostile human is given five colour entries (red or violet, plus yellow and blue in the red case); a player character light green; two further classes cyan and pale yellow. Which entry a state uses was not established | observed for the values | 0048f280, 004a5c30, 0046ffe0, 004b1eb0, 005ef320 | high for the values, unknown for the mapping | **(R19-24)** |
| CAMP-351 | Native 24's accepted codes and their colours; 111/222/333 require a human element; 0, 1, 500 and 666 change only the style code and leave colours untouched | observed | 005788a0 | high | **(R19-24)** |
| CAMP-352 | No grey value appears among the examined calls of one colour helper; this does not establish that no grey dot can be drawn, and state-dependent rendering is unspecified | observed (limited) | 005ef320 call sites | low | **(R19-24)** |

---

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| save magic | the four bytes `GSHR` | compatibility token, marked | 0056ea70 | high |
| profile archive magic | the four bytes `FORP` | compatibility token, marked | 0055dea0 | high |
| profile archive version | 6 | — | 0055dea0 | high |
| archive version of the retail build | 48 | — | 0056ea70, retail files | high |
| version above which the backup campaign block appears | 38 | — | 00511340 | high |
| version above which the recent list appears | 29 | — | 00452f20 | high |
| version from which the town result appears | 41 | — | 00452f20 | high |
| version from which the town code appears | 48 | — | 00452f20 | high |
| version above which the recruit flag appears | 27 | — | 00452f20 | high |
| version at which a workshop assignment's u16 is discarded | 46 | — | 00580ab0 | high |
| version from which that u16 is stored | 47 | — | 00580ab0 | high |
| block tag length | 16 | bytes | 005e1b70 | high |
| campaign counters | 27 | i32 | 00452f20 | high |
| script campaign value window | counters 7..26 as script indices 0..19 | — | 00579430 | high |
| new-campaign money | 100 | currency | 00450ec0, retail file | high |
| character-profile index of the starting man | 1 | index | 00450ec0 | high |
| no-maximum sentinel in the money window | 200000 | currency | 0054c800 | high |
| team size limit when nothing is selected, and on a missing mission file or `SCOT` chunk | 5 | men | 00579300, 0056b8d0 | high |
| capability flags per mission slot | 10 | flags at offset 22 of a `SCOT` record | 0056b8d0 | high |
| capability array sizes of a character profile | 3 and 4 | entries | 004574b0 | high |
| capability substitutions | 2 by 3, 13 by 14 (three-entry array); 25 by 26 (four-entry array) | — | 004574b0 | high |
| workshops | 13 | records | 00456a40, retail file | high |
| items placed per work spot | 5 | items | 00580b70 | high |
| capacity-flag rule | `5 × spots <= stock` | — | 00580b70 | high |
| experience carry | 100 points per level, level clamped at 100 | — | 0051d4e0 | high |
| health cap | 100 | points | 00484360, 00455bf0 | high |
| recent-mission memory | 3 | missions | 004539f0 | high |
| reduction attempts before forcing | 10 | attempts | 00451b70 | high |
| random filter modulus | 101 | — | 00452040, 00452090 | high |
| age-increment threshold above which a candidate only ages 0 -> 1 | 1000 | campaign days | 004560d0 | high |
| score for a won non-ambush mission | 1000 | points | 004e3260 | high |
| score per qualifying surviving hostile | 70 | points | 004e3260 | high |
| score for an experience-level increase | 100 | points | 0049fc80 | high |
| score for a hostile's death from damage | 50 | points | 004a5a00 | high |
| progress when the last story node is won | 100 | percent | 00456db0 | high |
| progress when the last defend node is won | 95 | percent | 00456db0 | high |
| clover ceiling for one actor action | 9 | count | 004a78c0 | medium |
| name generation text id ranges | 100..121 and 122..143 | resource ids | 0055bf00 | high |
| named-hero name text ids | 0x90 + position, positions 0..6 | resource ids | 0055bf00 | high |
| campaign-map status line text ids | 0xF3, 0xF4, 0x40 | resource ids | 00528810 | high |
| HUD counter text ids | 0xF4 money, 0xF5 clover | resource ids | 004c8140 | high |
| auto-slot label text ids | 0xF6, 0xF7 | resource ids | 0050f1a0, 0050f2d0 | high |
| game-length label text id | 0x50 | resource id | 0054d550 | high |
| player-character dot colour | RGB (165, 255, 82) | colour | 0048f280 | high |
| hostile-human dot colour | RGB (255, 0, 0) | colour | 004a5c30 | high |
| hostile-human colour under one controller flag | RGB (115, 40, 203) | colour | 004a5c30 | high |
| hostile-human further entries | RGB (255, 255, 0) and RGB (0, 120, 255) | colour | 004a5c30 | high |
| cyan element class | RGB (0, 190, 255) | colour | 0046ffe0, 005788a0 | high |
| pale-yellow element class | RGB (255, 247, 90) | colour | 004b1eb0 | high |
| slot names | `Continue`, `Restart`, `QuickSave`, `ExQuickSave`, `Sherwood`; numbered slots `Savegame_%03i` | compatibility tokens, marked individually | 0056ef30, 0056f090, 0056f1f0, 0056f350, 0056f4b0, 0056fa30 | high |
| thumbnail name suffix | `_t` | compatibility token, marked | 0056e6b0, retail files | high |
| campaign backup file name | `Campaign.bck` in the installation root | compatibility token, marked | 00457560 | high |
| profile directory pattern | `Profile_%03i` | compatibility token, marked | 0055dea0 area | high |
| progress special-case level codes | the two-letter codes of the last story mission and the last defend mission | compatibility tokens, marked individually | 00456db0 | high |

---

## 6. Interfaces to the script VM

Only the natives this subsystem owns. Arity, coercion and the call protocol are `spec-script-vm.md`'s.

| Id | Args | Returns | Meaning and side effects | Failure |
|---|---|---|---|---|
| 163 | – | int | the length of `team` | 0 outside a campaign |
| 164 | `i` | handle | the element of `team[i]`; no bound check | – |
| 165 | `pc` | – | append to `team` if absent; refresh the HUD; write three unidentified actor fields, one to 2 | script error, no change, for a non-player-character |
| 166 | `pc` | – | remove from `team`; refresh the HUD | as 165 |
| 170 | – | bool | the requirement test of 3.9 | reads through a null selection |
| 172 | – | int | the selected node's four-byte code, 0 when none | – |
| 173 | – | bool | one byte of the game object, not of the campaign; its transitions were not read | – |
| 174 | – | int | the selected node's `team_limit`, 5 when none | script error and 0 unless the current node's location is 8 |
| 178 | `banner` | – | deactivate the banner; `blazons +=` its value; in an assault node win once `blazons >= blazons_needed`; refresh the HUD | error for a null or already inactive banner |
| 195 | `k` | int | counter `k + 7`, `k` in 0..19 | script error and 0 outside the range |
| 196 | `k, v` | – | write counter `k + 7` | as 195 |
| 199 | `k, loc, n` | – | workshop `k`: zone and capacity; back-link the location | no range check on `k` |
| 200 | `k, loc` | – | append a work spot to workshop `k` | – |
| 232 | `pc` | – | add to the band | error when not a player character or already present |
| 234 | – | bool | `blazons >=` the reported requirement | 0 outside a campaign |
| 236 | – | int | counter 1 | −1 with an error outside a campaign |
| 237 | `v` | – | write counter 1; a growing value plays a sound; refresh the HUD | as 236 |
| 239 | – | – | open the debriefing screen | – |
| 249 | – | int | the number of HUD-selected player characters | – |
| 250 | `i` | handle | the i-th HUD-selected character | error and null at or above the count |
| 256 | `pc` | int | the campaign identity of `pc`, matched from his character profile's designer name; the id assignment is `spec-script-vm.md`'s row for this native and is not restated here | −1 with an error otherwise |
| 261 | – | bool | the recruit flag | – |

**Engine-originated messages to the camp level.**

| Message | When | Meaning | Ordering |
|---|---|---|---|
| 1001 | once, at camp level start, in campaign mode | "configure your production zones"; the script answers with natives 199 and 200 | **unknown** (CAMP-303) |
| 1000 | unknown | "send the team out"; the script walks the HUD-selected characters to the exit | unknown (CAMP-302) |

---

## 7. Acceptance tests

### 7.1 Format observations on this installation's fixtures

A1 is a property of the retail data file and is a genuine acceptance test. **A2 to A5 are observations of
mutable local fixtures** — a save, a backup and a profile archive that any play session rewrites — so they are
regression fixtures, not universal expectations, and they prove grammar, not semantics.

| Id | Input | Expected |
|---|---|---|
| A1 | `Configuration/profile.cpf` parsed with 2.2's grammar | consumed to the exact byte, all 30,441 of them; 63 level records; record 0 has kind 4 and location 8 |
| A2 | `Campaign.bck` (fixture) | `u32 48` + one campaign block, consumed exactly (2,717 bytes); counters all 0 but index 1 = 100; siege state −1; 63 slots whose node indices are 0..62 in order, all outcomes 0; the four slot references read (none, 21, none, none); one character record with health 100, profile index 1, experience levels 20 and 100; band = [0]; team = [0]; recovering empty; 13 workshop records with kinds 0..12 and empty assignment lists; recent = [21]; town result 0 |
| A3 | the two fixture saves | header `GSHR`, version 48, a level code, version 48; the 2,713 bytes at offset 16 equal the backup file minus its first four bytes; a second campaign block begins at 2,729 and differs from the first only in counter 6 and in the four discarded bytes of every slot; the level state begins at 5,442 |
| A4 | `Savegame/Profiles` (fixture) | `FORP`, version 6, one profile, consumed exactly (all 430 bytes); money 100, score 0, progress 0, play time 12 s, difficulty 1; two key sets of 29 codes; resolution 1024 x 768; two save slots |
| A5 | `Configuration/keyset1.cfg`, `keyset2.cfg` | 76 bytes each: a 16-byte tag, set id 2 and 3, 29 u16 codes |
| A6 | every mission file's `SCOT` chunk | each node's `team_limit` equals its mission file's `SCOT` record count; 12 of the 39 mission files carry non-zero capability flags, over 16 slots in total |

### 7.2 Behaviour tests (synthetic, no game data)

| Id | Input | Expected |
|---|---|---|
| B1 | `min_money` 5000, money 4999 | not accessible; the diagnostic names the minimum-money condition |
| B2 | `max_money` 200000, money 4 × 10^9 (unsigned) | accessible |
| B3 | an *after* entry whose slot has outcome 2 | accessible |
| B4 | a node listing itself in *until*, played | never accessible again |
| B5 | siege state −1, gate byte 0 = 1, all other gate bytes 0 | accessible |
| B6 | siege state 0, the same node | not accessible |
| B7 | three candidates sharing one location and one map, priorities 1, 2, 2 | only the priority-1 candidate survives; the others' ages are 0 |
| B8 | two candidates at the same location and priority | both retained and one diagnostic recorded |
| B9 | one obligatory and two ordinary candidates | only the obligatory candidate survives |
| B10 | one obligatory candidate alone | step b does not run; the candidate survives unchanged |
| B11 | a story candidate with `max_age` 6 and age 5, plus one other | the story candidate alone survives |
| B12 | a defend candidate with `max_age` 3, age 3, outcome 0, plus one other | it is removed, recorded lost, and its loss byte applied |
| B13 | three candidates of which the two most recent are in `recent`, newest first | the newest recent one is removed; the walk then stops with two candidates, so the second recent one **survives** |
| B14 | one candidate that is in `recent` | it survives (step e needs more than one candidate) |
| B15 | a random filter that would remove every candidate | the list is restored unchanged, and exactly one draw was taken per candidate that passed the kind and age tests |
| B16 | a random filter with three eligible candidates and one ineligible | three draws, in list order; the ineligible candidate takes none |
| B17 | an empty source list | the reduction reports failure after ten attempts and yields an empty list; no mission is forced |
| B18 | a source list of one candidate that every step keeps | the forced path is not reached; that candidate is the offer |
| B19 | exactly one offer whose node has `needs_camp` 0 | it becomes current; the camp is not entered; `team` is reset to `band` |
| B20 | exactly one offer whose node has `needs_camp` 1 | node 0 becomes current |
| B21 | `selected` set | it becomes current and `team` is **not** reset |
| B22 | picking an offer in the camp with three offers | the picked candidate's age is 0 and it leaves the list; the other two aged by one |
| B23 | a won kind-0 mission with 3 hostile survivors (one qualifying) and 1 hostile dead | spared += 3, killed += 1, score += 1000 + 70 |
| B24 | the same mission lost, the slot being the blazon node | the outcome is 2, blazons become 0, the loot pass still runs, and no counter from step 4 changes |
| B25 | spared 3, killed 1 | the map's spared percentage is 75 |
| B26 | spared 0, killed 0 | the spared percentage is 0 |
| B27 | 20 qualifying nodes, 7 won, neither special code | progress 35 |
| B28 | 0 qualifying nodes | the original divides by zero; OpenSherwood answers 0 and records a diagnostic |
| B29 | the special defend code won | progress 95 |
| B30 | the special story code won | progress 100 |
| B31 | buying a blazon at price 1500, step 500, twice | money falls 1500 then 2000; blazons 2; price 2500 |
| B32 | an assault node needing 12, banners summing to 12 | won on the banner that crosses the threshold |
| B33 | a defend blazon node needing 3, blazons 3, resolved from the camp | blazons 0, the node won and out of `offered`, the notification armed with the node's code and result 1, and the offered list rebuilt |
| B34 | `men_per_blazon` 3, team 8, needed − held 2, blazons 0 | the convertible count is 6 |
| B35 | `men_per_blazon` 3, needed − held 2, blazons 5 | the original's unsigned subtraction wraps and the result is large; OpenSherwood clamps to 0 and records a departure |
| B36 | `men_per_blazon` 0 | nothing is computed and a diagnostic is recorded; no division occurs |
| B37 | four candidates: two of kind 5, one of kind 6, one of kind 0 | the first kind-5 candidate becomes the blazon node, the second kind-5 candidate is removed, the kind-6 candidate moves to `deferred`, the kind-0 candidate stays |
| B38 | the same set with no kind-1/5 candidate | the blazon node is null and the kind-6 candidate is removed rather than deferred |
| B39 | one candidate of kind 5 | it becomes the blazon node and is not removed |
| B40 | native 174 with the current node's location not 8 | 0 and a recorded script error |
| B41 | a node whose capability requirement is satisfied only by the substitution 2-by-3 | native 170 answers 1 |
| B42 | a node with a required character profile absent from the team | native 170 answers 0 and stops at that entry |
| B43 | a workshop pass with `previous` lost | no output is computed for any kind; item placement, kind 12 and character placement still run; the capacity flag is still recomputed |
| B44 | a kind-0 workshop, stock 7, output 0 (no victory), two spots | stock stays 7; at most 5 items are placed per spot; the capacity flag becomes `10 <= 7` = false |
| B45 | a kind-9 workshop, two assigned characters, one lacking the required capability | only the other one's experience slot 1 changes |
| B46 | a kind-11 workshop, an assigned character at health 95, output 20 | his health becomes 100 |
| B47 | an experience slot at level 3, points 80, plus 150 | level 5, points 30 |
| B48 | an experience slot at level 100, points 0, plus 500 | level 100, points 500 |
| B49 | a save written and reloaded | the campaign compares equal field by field, the four discarded bytes of every slot ignored |
| B50 | a save whose backup block differs from its live block | after loading, the campaign equals the live block |
| B51 | a save with a bad magic and a bad version | the original accepts it; OpenSherwood refuses it (a recorded departure) |
| B52 | a save at archive version 46 | each workshop assignment's trailing u16 is read and the field becomes 0xFFFF |
| B53 | recruiting with an empty recovery list | no draw is taken from the recruit stream |
| B54 | recruiting with a non-empty recovery list | exactly one draw is taken, and on the recovery branch exactly one more for the choice of man |
| B55 | native 24 with code 100 then code 0 | the style code becomes 0 and the colours stay light green |
| B56 | native 24 with code 111 on a non-human element | an error and no change |
| B57 | native 24 with code 999 | an error and no change |

**Oracle procedures.** The auto-save write points (CAMP-131, CAMP-132) and the play-time accrual (CAMP-271) are
checkable against a recording from `C:\Users\przem\source\gamedata\robinhood_oracle`: start a mission and
confirm the restart slot and its thumbnail appear before the first frame; enter the camp and confirm the Sherwood
slot appears; save twice with a measured interval between and confirm the profile's play time grew by that
interval, to the second. Tolerance 1 s.

---

## 8. Implementation choices, snapshot and departure contract

Everything in sections 2, 3, 5 and 6 is the original's behaviour unless it appears below.

**OpenSherwood decisions and deliberate deviations.**

1. *Block tags.* We write sixteen zero bytes and accept any sixteen bytes on reading (2.6). No captured or
   generated tag values are kept anywhere, and there is no first-run problem.
2. *The four discarded bytes per slot.* We write zeros, for reproducible saves. Round-trip comparisons mask those
   four bytes per slot.
3. *Header validation.* We refuse any save whose magic is not `GSHR`, and we refuse an archive version we do not
   support, instead of the original's behaviour where a wrong version can mask a wrong magic (CAMP-125).
4. *Progress with no qualifying node* divides by zero in the original; we answer 0 and record a diagnostic
   (B28).
5. *The blazon conversion* has no clamp in the original and its subtraction wraps; we clamp to 0 and record a
   departure (B35). The zero-divisor case needs no deviation: the original already returns without dividing, and
   so do we.
6. *The unit mix in the live play-time display* (CAMP-272) is not reproduced; we show seconds.
7. *Randomness.* The original draws from the C run-time generator. We use **four** named seeded streams,
   consumed in exactly the orders section 3 gives, and do not claim to match the original's sequence:
   `campaign.offer` (the two random filters, one draw per eligible candidate per pass),
   `campaign.force` (the forced-mission draw), `campaign.recruit` (the recovery coin flip and the choice of man
   to recover) and `campaign.name` (the two name draws per generated name, and the bounded retry loop). There is
   no fifth stream; revision 1's snapshot said five and its list gave four, and four is correct.
8. *Diagnostics.* Where the original reports a condition and continues — two obligatory candidates, equal
   location and priority, a zero men-per-blazon, an empty offered list, a failed reduction, a missing mission
   file or `SCOT` chunk — we record a diagnostic and take the same continuation; we do not abort.

**Snapshot contract.** Authoritative, and therefore in `snapshot()` and under the canonical hash: the 27
counters; `siege_state`; `recruit_flag`; the four slot references; every slot's node, age, blazon price and
outcome; `people` in order with every field of 2.3; `band`, `recovering`, `team`, `offered`, `deferred`, `recent`
as ordered lists; the 13 workshop records with kind, capacity, stock, last output, capacity flag and
`assignments` in order; `town_result` and `town_code`; the identity of the current player profile and its save
slot list; and the positions of the four named RNG streams.

Not authoritative: the reduction's working copy; the work-spot lists of 2.4 (they are rebuilt by running the camp
script's load path — an importer that does not run it has assignments whose coordinates point nowhere, which is
a load-order requirement, not a state field); the level's mission-statistics block while no mission runs; and
every derived display value (progress, the spared percentage, the reported blazon requirement, `full`).

**Departure contract.** A ruleset bump is required by a change to: the availability test (3.2); the reduction
order, its retry and restore behaviour, or the RNG consumption order (3.3); the choice and aging rules (3.4);
the outcome and mission-end rules (3.5); the blazon rules (3.6); the score, progress and spared formulas (3.7);
the recruit rule (3.8); the deployment requirement test (3.9); or the workshop pass once it is specified (3.11).
A change to the file grammars of 2.6 is a format-version bump and must keep reading every archive version listed
in section 5.

---

## 9. Differences from the current engine

Against `crates/opensherwood-app/src/{engine.rs,ui.rs}`, `docs/formats/profile.md`, `docs/formats/savegame.md`,
`docs/formats/sherwood-hub.md` and `docs/original/campaign-flow.md` as they stand. This is a list of gaps, not a
claim that this revision closes them: section 11 names what it does not.

1. **The successor rule is wrong.** The engine's rule is a successor relation over profiles; the original's is
   the eight-condition test of 3.2, the seven-step reduction with its retries and restores, and the age counter.
   The engine has no money window, band-size window, age limit, siege gate, priority, recency filter or random
   filter.
2. **The *after* list means "played", not "won":** a lost prerequisite opens its successors.
3. **`profile.md`'s level-record grammar is wrong in three places** (CAMP-041 to CAMP-043). The existing parser
   consumes the retail file anyway because the errors cancel, but a reader that trusts the field names
   mis-reads three groups.
4. **Deployment requirements are a real feature the engine lacks entirely:** a per-node list of required
   character profiles *and* a per-node list of capability requirements built from each mission file's `SCOT`
   flags, matched against the character profile's capability arrays with three substitutions, plus a per-node
   team size limit equal to the mission's slot count.
5. **`savegame.md` is a wrong guess.** The 32-byte records are slots with age, blazon price and outcome; the 16
   bytes are a block tag; a save carries the campaign block **twice** when the version is above 38, the first
   copy streamed from `Campaign.bck`; and `Campaign.bck` is not a backup of a table but the campaign as of the
   last level start.
6. **The blazon economy is absent.** Blazons are earned in assault and tactical missions, bought at a price that
   rises per purchase, spent to resolve defend missions and cleared when the blazon node is lost. Natives 178,
   223 and 234 are stubs over a counter nothing moves.
7. **Six natives gain rows** (165, 166, 170, 174, 239, 249) and two stub policy values are wrong: 174 must be the
   selected node's slot count (5 only when nothing is selected), and 249 is the HUD selection count, which
   `STUB_POLICY_VALUES` pins to 0 while also pinning 174 to 5 unconditionally.
8. **Message 1001 is engine-originated at camp start**, so the engine's camp never configures its 13 production
   zones. (Its delivery order, and the origin of 1000, are open — section 11.)
9. **The workshop pass is absent,** including the parts that run without a victory: item placement, character
   placement and the capacity flag.
10. **Score, progress and the spared percentage are unlike anything in the engine.** Progress excludes ambush and
    tactical nodes, includes placeholders, and two level codes short-circuit to 95 and 100. The spared percentage
    is the share of **enemies left alive**, from two cumulative counters over hostile actors — not a count of the
    player's men, which is what a naive implementation would build.

Two further gaps: `profiles.json` is not `Savegame/Profiles` (different container, no key or graphic
configuration, no slot list, no play time, spared percentage, blazons, siege state or difficulty), so a profile
the engine writes cannot round-trip through the original; and the engine has no auto-save, whereas the original
writes a restart slot at every mission start, a Sherwood slot at every camp entry, and mirrors every save into
the continue slot.

---

## 10. Open questions

1. **The one-day output of a workshop, and kind 12.** Four routines truncate a floating-point value whose
   expression the export does not contain, and the part played by the routine that locates the workshop's
   supervising character is unknown. Read the raw instructions of 00581020, 005810c0, 00581180, 005806c0's tail
   and 00580e80 (kind 12).
2. **The delivery order of message 1001** relative to the workshop pass and the first tick (CAMP-303): recover
   the arguments of 00578d80's call in 0050f710 and the scheduler's admission rules (0058a940, and
   `spec-script-vm.md` VM-210/215).
3. **The origin of message 1000** (CAMP-302). Search the interface code for the camp's send action
   (`ui-flow.md` `BTTN` 160) and for submitters of 00578d80 outside the natives.
4. **The recruit count** after a victory (CAMP-291): the expression is absent from 004e3260's export.
5. **When the camp-capture pass runs** (CAMP-326, 00456a70 has no direct caller in the export) and how its
   rebuilt assignments relate to the work spots the script registers.
6. **How the clover counter becomes non-zero** (CAMP-016). Follow 004a78c0's callers and the folded
   campaign-counter call in 0050b640.
7. **The three actor fields native 165 writes** (CAMP-310), one of them set to 2.
8. **Which colour entry each actor state uses**, and whether an un-blipped element is drawn (CAMP-350,
   CAMP-352); plus portrait states, action-icon availability and tooltips (005145b0, 00514670).
9. **Smaller items:** the two unused level-record u16 (`m0`, `m1`); the purpose of `spare_refs` and `labels`;
   native 173's state transitions; the two tests that exclude the auto-slots from continue-mirroring
   (CAMP-133); the generated-name loop's exit condition (CAMP-071); the source of a recovered man's health
   (CAMP-292); what makes a surviving hostile "qualifying" (CAMP-013, CAMP-251); the campaign-mode test at the
   head of the game loop, which the export renders as a test on the siege byte and which is more plausibly "a
   campaign exists" (CAMP-300); and the behaviour of save writing when the file cannot be created.

---

## 11. Excluded from clearance

The following are **not** cleared for implementation by this revision. Each maps to an `Assumption` variant
(ADR-0008) that must stay in the registry until an amendment settles it.

| Area | Why | Assumption |
|---|---|---|
| **Camp production output** (3.11's gated half for kinds 0..11, and kind 12 entirely) | The output arithmetic, rounding and overflow are not recoverable from the export (open question 1). The routine that picks the workshop's supervising character maps each workshop kind to one campaign identity; that eleven-entry mapping is withheld on the same grounds as the other two, and the contribution it makes to the output is unknown anyway. The ungated half — item placement, character placement, the capacity flag, the recount — *is* specified and may be implemented | `CampProductionOutput` |
| **The workshop-kind to item-kind correspondence** | A nine-entry mapping inside the executable. Reproducing it is the kind of curated table ADR-0009 refuses; the reviewer refused it in revision 1. The mechanism is specified (each of kinds 0..8 has exactly one item kind; kinds above 8 have none) but the mapping itself is withheld pending a maintainer and reviewer decision | `WorkshopItemKinds` |
| **The `SCOT` flag to capability-id correspondence** | A ten-entry mapping inside the executable, of the same character. The mechanism, the flag positions, the matcher and its three substitutions are specified; the correspondence is withheld on the same grounds. Without it native 170's capability half cannot be implemented | `SlotCapabilityIds` |
| **Message ordering in the camp** | CAMP-303. No implementation may assume 1001's handler completes before the workshop pass or before the first tick | `CampMessageOrder` |
| **The origin of message 1000** | CAMP-302 | `CampSendAction` |
| **The recruit count** | CAMP-291 | `RecruitCount` |
| **Mini-map state-dependent rendering** | CAMP-350, CAMP-352: the values are known, the state-to-entry mapping is not, and no claim is made about grey dots | `MinimapDotState` |

Everything else in sections 2 to 8 is offered for clearance.

---

## 12. Implementer handoff requirements

A handoff of this subsystem is complete only when all of the following hold. Items marked *(blocked)* need an
amendment first and define the scope an implementer must leave alone.

1. The implementer session is fresh: no inherited analyst context, notes, tool output or memory, and no access
   to `re/` — including through helper scripts (ADR-0009 §2).
2. The scope handed over excludes everything in section 11. Concretely, the implementer may build: the file
   grammars of 2.6 (read and write, every archive version in section 5), the campaign object and its counters,
   the availability test, the whole reduction including retries and restores, choice and aging, outcome and
   mission-end bookkeeping, the blazon rules, the score/progress/spared/play-time formulas, the recruit
   mechanism except its count, deployment except the capability half of native 170, the workshop pass except the
   output arithmetic, the save slots and auto-saves, and the HUD counters. *(blocked)*: production output,
   kind 12, the two withheld mappings, message ordering, the send action, the recruit count and the mini-map
   state mapping.
3. The importer runs the camp script's load path before treating workshop assignments as positions (2.4,
   section 8).
4. The four named RNG streams of section 8.7 exist with the stated consumption order, and their positions are in
   the snapshot.
5. The snapshot and departure contracts of section 8 are implemented and the canonical hash covers exactly the
   authoritative set.
6. The acceptance tests of section 7 exist, with A2 to A5 marked as fixture regressions rather than universal
   expectations, and with a documented procedure for refreshing them when the fixtures change.
7. Each entry of section 11 is an `Assumption` variant in the registry, and no winning path depends on one.
8. Save failure behaviour (a file that cannot be created, a version that is not supported, a truncated block) is
   defined by OpenSherwood, since the original's behaviour is partly unknown (open question 9) — refuse and
   report, never partially apply a campaign block.

---

## 13. Identity and exposure

- **Analyst:** session `a00ebf7dd67504358` (Opus, 2026-09-13). Exposure: full decompilation of the ~165
  functions of section 0, plus the function inventory, the string cross-reference and the module map.
- **Reviewer:** Codex `gpt-6-astra`, spec review 19 on blob
  `6601ef353cb680df86e7b23613627c0cd6b032d5`, verdict *redo*, 29 findings. The reviewer is a **spec reviewer**
  and read `re/`; its exposure covers the same subsystem plus the mission-file inspection behind CAMP-048. It
  therefore may not implement this subsystem either, and its output was corrections to this document only.
- **Wall (ADR-0009 §2).** Sessions exposed for this subsystem: the analyst above and the reviewer above. Neither
  may implement campaign flow, the Sherwood camp, camp production, saved games, the player profile and
  configuration files, the campaign-map screen or the mini-map dot colours.
- **Publication approval:** pending, maintainer, separate from factual approval.
- **Gate:** `python scripts/check_no_assets.py --paths docs/original/spec-campaign-camp-saves.md` run before
  saving this revision. The gate is a heuristic and does not clear the expression filter; section 11 records
  what the reviewer withheld.

---

## 14. Provenance

Ghidra project `re/ghidra/robinhood` (never committed); full-function decompilation under `re/out/decomp_all/`
exported by `scripts/ghidra/DecompileAll.java`; the function inventory and string cross-reference from
`scripts/ghidra/ExportInventory.java` and `ExportStrings.java`; the module map in `re/notes/modules.txt`, derived
from assertion strings. Byte-level checks used `scripts/ghidra/peek.py` and three throw-away parsers in
`re/notes/campaign/` (a campaign-block reader, a level-table reader and a profile-archive reader); those parsers
are one-off derivations of the numbers in section 7 and are not needed to check the claims — the acceptance
tests re-derive them from the player's files.

Files read on disk, read-only, under `C:/Users/przem/source/gamedata/robinhood`: `Campaign.bck`,
`DATA/Configuration/profile.cpf`, `DATA/Configuration/keyset1.cfg`, `keyset2.cfg`, `Data/Savegame/Profiles`,
`Data/Savegame/Profile_001/{Continue,Continue_t,Restart,Restart_t}`. The mission-file inspection behind CAMP-048
and A6 was the reviewer's, on the same installation.

No oracle recording was made; the claims that would need one are listed in section 7.

Function addresses appear inline in sections 2 to 6 and are collected in section 0. Instruction bytes,
disassembly, recovered symbols and the comprehensive address map stay in `re/`.

Build: GOG English edition, `Robin Hood.exe` SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`.

Documents to update once this revision is cleared: `docs/formats/savegame.md` (replace the stub),
`docs/formats/profile.md` (the three grammar corrections, the field meanings, the key-set layout),
`docs/formats/rhm.md` (the `SCOT` record's capability flags and the slot count's role),
`docs/formats/sherwood-hub.md` (sections 5 and 6), `docs/original/campaign-flow.md` (the successor rule),
`docs/roadmap.md` and the `Assumption` registry (section 11).
