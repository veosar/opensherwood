# Campaign, Sherwood camp and saved games (behaviour specification)

Status: `draft`, **revision 9** (awaiting re-review). Revision 5 answered the 16 findings of Codex re-review 29
of revision 4 and added the camp's production arithmetic (3.11), derived from instruction listings that carry the
floating-point operations the decompiler drops; revision 6 answered the 13 findings of Codex re-review 34 of
revision 5; revision 7 answered the 6 findings of Codex re-review 41 of revision 6; revision 8 answers the 3
findings of Codex re-review 43 of revision 7; revision 9 answers the 11 findings of Codex re-review 46 of
revision 8. Review 46 showed that four of the eight exclusions revision 8 resolved were resolved **wrongly**;
revision 9 corrects two of them from the code, withdraws two back into exclusions, and states the three
gameplay-integration gates the reviewer required. Its rule is accuracy over count. Revision 2 answered the 29 findings of Codex spec review
19 on revision 1; revision 3 added the three interface tables of section 2.7 on the maintainer's clarification to
ADR-0009 section 5; revision 4 answers the 19 findings of Codex re-review 23 of revision 3 (verdict
*fix-then-clear*), which cleared as facts the character-record framing, the four node references' order, the town
code, the conditional-backup and unconditional-live block framing, the version 46/47 assignment word, and T1's
ten mappings with its substitutions and T2's nine mappings.

Sibling specifications this one depends on, pinned to the revision that was reviewed:
`docs/original/spec-script-vm.md` revision 4 at commit `afdfaec` (opcodes, natives, the callback scheduler, the
won and lost flags, the level tick) and `docs/original/spec-ai-combat.md` revision 2 at commit `e966b05` (every
profile stat field, and difficulty). `docs/decisions/ADR-0010-logic-frame.md` fixes the engine's single logic
frame at 46.875 ms; this document's one timing rule, the play-time counter of 3.7, is expressed in those frames.
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
  the engine ... the obvious candidates"  -  a guess it could not close.
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

**Honest stopping statement.** The *reading* side of the file grammars is closed: a grammar written from this
specification consumes `Configuration/profile.cpf` (30,441 bytes), `Campaign.bck` (2,717 bytes),
`Savegame/Profiles` (430 bytes) and the campaign part of both retail saves to the exact byte  -  on the fixtures of
this installation, which proves grammar and not semantics. The *writing* side is not closed: a file the original
will load needs sixteen tag bytes per record that this document may not carry (2.6, 8.2). The *behaviour* is not
closed either. Fourteen areas remain open (section 10) and **seventeen** subsystems are excluded from clearance (section 11).
Revision 8 claimed to settle seven exclusions; review 46 showed four of those readings were wrong. Revision 9
keeps the three that survive scrutiny - character initialisation (with the two experience fields now named), the
camp message's completion ordering, and the absence of any "send the team" message - corrects the hostile-
survivor predicate and the spawn contract from the code, and **withdraws** native 173 and the revive pass's
effect back into exclusions.
The camp's production *arithmetic* is settled, but the workshop pass around it is not: the location-membership
rebuild that runs **after each workshop's own turn**, the pass's scheduling, and the effective precision mode of
its multiplications are all excluded, so "the whole workshop pass" is **not** cleared. **This document does not yet describe a campaign that can be played from end to end, and revision 8's claim that
the remaining exclusions were "presentation or interoperability" was wrong.** Three of them change what the
player sees during ordinary play: the per-workshop location-membership rebuild and the capture pass's scheduling
decide which men are found at which workshop, and therefore the stock, training and healing of every camp day;
and the multiplications' precision mode decides the day's numbers themselves. Recruitment, spawn assignment,
native 165, native 173, the revive pass's effect and the terminal transition are likewise gameplay. Section 8.9
states the gate each of the three production-integration items must pass before an implementation may ship it;
the others need the readings section 10 names.

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

**Counters**  -  27 signed 32-bit values, index 0..26, all zero on a new campaign except index 1 (CAMP-010,
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
| `siege_state` | i8 | -1 on a new campaign (CAMP-030) |
| `recruit_flag` | u8 | set when a man returns from the recovery list; native 261 returns it; cleared in the post-victory bookkeeping |
| `previous`, `current`, `selected`, `blazon_node` | slot references | the mission played before, the mission being played (the camp is a node like any other), the mission the player picked in the camp, and the node the blazon counter belongs to |
| `town_result` | i32 | 0, or 1 / 2 = the last defend node resolved as won / lost, pending display |
| `town_code` | 4 bytes | the two-letter level code of that node  -  a **code, not an index** (CAMP-003) |

**Slots.** One per node, in level-table order, 63 of them (CAMP-020):

| Field | Width | Unit | Meaning |
|---|---|---|---|
| `node` | i32 | index | which level-table record this slot belongs to; -1 = none. In the retail files slot *i* points at node *i* |
| `age` | u16 | campaign days | how long the slot has been offered; reset to 0 whenever it leaves the offered list |
| `blazon_price` | u16 | currency | what one blazon costs for this node now; initialised from the node's *first blazon price*, raised by its *price step* after every purchase |
| `outcome` | u32 |  -  | 0 not played, 1 won, 2 lost |

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
| `camp_extras` | 32-bit values | the queue workshop kind 12 draws from, one value per work spot (3.11). Empty in the retail fixtures; nothing in the scope read fills it (section 10). Revision 4 called it `spare_refs` and recorded it as "purpose unknown" |
| `labels` | wide strings | the history of names already generated for recruits; a new name is accepted only when it is absent from this list and is then appended (3.8, CAMP-295). Empty in the retail fixtures |
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
| `production_yield` | u16 | the multiplier every camp workshop applies to a day's output after this node is played (3.11). `docs/formats/profile.md` calls it `unknown_f`; revisions 1 to 4 of this document omitted it from the field table altogether (CAMP-334) |
| `prob` | u16 | a percentage used by the two random filters (3.3.2) |
| `priority` | u16 | lower wins when two candidates share a location (CAMP-065) |
| `required_pcs` | u32 count + u32 each | indices into the character-profile table of the same file: characters that must be in the team |
| `siege_gate[0..9]` | u8 x 10 | `siege_gate[0]` is a flag: not 1 means the node is not gated. When it is 1 the node is allowed only while `siege_gate[1 + siege_state]` is non-zero; because `siege_state` is -1 on a new campaign the expression reads `siege_gate[0]` itself, which is 1, so a gated node is allowed at the start (CAMP-031, CAMP-032) |
| `siege_on_win`, `siege_on_loss`, `siege_on_expiry` | i8 x 3 | the new `siege_state` after winning / losing this node, and after selecting it over-age; -1 = leave it alone (CAMP-033) |
| `m0`, `m1` | u16, u16 | no reader found in the scope read (section 10) |
| `blazons_needed`, `blazons_held` | u16, u16 | section 3.6 |
| `first_blazon_price`, `blazon_price_step` | u16, u16 | section 3.6 |
| `men_per_blazon` | u16 | how many men one blazon is worth; a zero value **terminates the original process** before any division (CAMP-005, CAMP-263). Answering 0 instead is OpenSherwood's declared deviation (8.1), not the original's behaviour (**R34-10**) |
| `music_*` | strings | as in `profile.md` |

Two further per-node values are **not** in `profile.cpf`. They are read from the node's own mission file when the
level table is loaded (CAMP-047, 0056b8d0): the mission file is opened, its `SCOT` chunk located
(`docs/formats/rhm.md`) and

- `team_limit` := the number of `SCOT` records, i.e. the mission's player-character slot count. The field is
  **zero when the record is created**, and three failure paths are distinct (CAMP-047):
  - the mission file cannot be opened: nothing is read, the field **stays 0**, and the failure **is** reported,
    non-fatally;
  - the file opens but the chunk walk finds no `SCOT` chunk: the field **stays 0** and nothing is reported;
  - a chunk the walk does find fails its validation (the outer container chunk, or the `SCOT` chunk itself): a
    non-fatal condition is reported and the field is set to **5**.

  Native 174 can therefore legitimately answer 0, and an implementation must not substitute 5 for a missing or
  chunkless file.
- `capability_reqs` := a list built from the `SCOT` records: ten single-byte flags at offset 22 of each record
  are read in order and each set flag appends one capability requirement to the node's list. Duplicates are not
  removed, so a requirement appears once per slot that asks for it. After the ten flags comes one byte that,
  when non-zero, is followed by the slot's name as a byte string, and then one further byte. The correspondence
  between flag position and capability id is **interface table T1** (2.7). Codex's read-only inspection found non-zero requirement flags in **12 of the 39
  mission files, covering 16 slots** (CAMP-048), so this list is not empty in practice  -  revision 1's claim that
  it is empty for every node was wrong.

### 2.3 One person: the character record

What the file carries, in order (CAMP-070; the framing consumes the retail 98-byte record exactly):

| Field (ours) | Width | Meaning |
|---|---|---|
| block tag | 16 bytes | see 2.6 |
| `exp[0].level`, `exp[0].points`, `exp[1].level`, `exp[1].points` | u32 x 4 | two experience slots, each a level and a point remainder; the accumulator of 3.11 carries points into levels in hundreds and clamps the level at 100. The retail hero has levels 20 and 100 with no points (CAMP-072) |
| block tag | 16 bytes | see 2.6 |
| `health` | u16 | 0..100 |
| `revive_flag` | u8 | non-zero marks the character for the end-of-mission revive pass of 3.5, which clears it |
| `stock[9]` | u16 x 9 | the carried stock of the nine capability kinds, one slot each. Which slot a capability id addresses is interface table **T4** (2.7); those ids are the same space as T1's requirements and T2's workshop items, so a workshop of kind *k* produces into the slot T4 gives for T2's item kind of *k* (CAMP-073) |
| `camp_slot` | u16 | the camp element the man is attached to; 0xFFFF = none |
| `name` | u16 count + that many 16-bit characters | the display name |
| `profile` | i32 | index into the character-profile table of `profile.cpf`; -1 = none |
| `deployed` | u8 | set when the man was instantiated in the current mission |

There are **no unattributed bytes**: the four 32-bit values are the two experience pairs, level before points
within each pair (0051d5d0). Revision 1's instruction to manufacture an opaque base block from captured bytes is
withdrawn.

**Name generation** (CAMP-071). A man whose character profile is marked a named hero takes his name from a text
resource: the profile's own designer name is matched against a fixed seven-name list inside the executable and
the matching position selects text id 0x90 + position; when it matches none, a fixed internal fallback string is
used and the condition is reported. A man who is not a named hero gets a generated name, and the rule is settled
(CAMP-295): each attempt draws one text id from 100..121 and one from 122..143 and joins the two texts with a
single space; the attempt **succeeds** when the result is absent from the campaign's `labels` history, and the
name is then appended to `labels` and used. At most ten attempts are made; if all ten collide the condition is
reported non-fatally and a fixed fallback name compiled into the executable is used **without** being appended
to `labels`. Revisions 1 to 5 left this exit unresolved; it is resolved here.

### 2.4 One workshop: the production record

Exactly 13, kind 0..12, in kind order. What the file carries (CAMP-090):

| Field (ours) | Width | Meaning |
|---|---|---|
| block tag | 16 bytes | see 2.6 |
| `kind` | u32 | 0..12 |
| `capacity` | u16 | the third argument of native 199 |
| `stock` | u16 | how much of the workshop's item the camp holds |
| `last_output` | u16 | what the last computed day produced |
| `full` | u8 | a capacity flag the workshop pass recomputes as `5 x (number of work spots) <= stock` (CAMP-322). It is a stored field of the record, so it is authoritative state (8) even though it is derived |
| `assignments` | u32 count + entries | per entry: an i32 position in `people`, two 32-bit coordinates, and a u16 sector reference (0xFFFF = none). From archive version 47 the u16 is the stored value; at version 46 a u16 is present, **read and discarded**, and the field is set to 0xFFFF; below 46 no u16 is present and the field is set to 0xFFFF (CAMP-093) |

Two further pieces of workshop state are **not** in the file and yet decide later behaviour, so they are
authoritative engine state (section 8): the **zone** a workshop was given by native 199  -  the assignment-capture
pass of 3.11 walks that zone's member list and does nothing at all when the workshop has no zone  -  and the
**ordered list of work spots** registered by native 200, whose length decides both the capacity flag and how
many items a day places. The camp script re-registers both on every camp entry (CAMP-091); section 8.3 defines
how an imported original save obtains them and what "restored" means when it cannot.

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
On reading, the tag is compared with a value the program derives internally, and a mismatch invokes the
program's **fatal** diagnostic path: it prints a fatal-exception line, writes a crash dump file, calls a
registered handler and exits the process (CAMP-002). Revision 3's "a writer may emit sixteen zero bytes  -  the
original will load such a file" was **wrong** and is withdrawn, together with the premise of review 19 behind it.

The tag values are content of the executable and are not reproduced here, so:

- **Reading the player's files** needs nothing: their tags are already right. OpenSherwood's reader is
  deliberately **permissive** and accepts any sixteen bytes, which is a departure from a fatal original and is
  recorded as one in section 8.
- **Writing a file the original can load** needs the exact sixteen bytes per record type. OpenSherwood obtains
  them at run time from the player's own files and never from a table in this repository  -  the *tag donor* of
  section 8.2. Until that mechanism is itself reviewed, producing files the original can load is **excluded from
  clearance** (section 11, `OriginalReadableWrites`). Files OpenSherwood writes for its own use carry its own
  marker and its own reader accepts them.

Two string conventions: a byte string is a u16 count and that many bytes; a wide string is a u16 count and that
many 16-bit characters  -  **except** the player's name, which uses a u32 count (CAMP-004).

#### 2.6.1 `Campaign.bck` (installation root)

`u32 archive_version` followed by the campaign block of 2.6.2. The retail fixture is version 48, 2,717 bytes. It
is rewritten from the live campaign when the game loop starts a level in campaign mode (CAMP-120). Failure to
create it is reported and otherwise ignored.

#### 2.6.2 The campaign block

Field order; `V` = the archive version:

1. block tag.
2. `recruit_flag`  -  u8. Only when `V > 27`.
3. the 27 counters  -  27 x i32.
4. `siege_state`  -  i8.
5. the slots: `u32 count`, then `count` slots, each: block tag, `u16 age`, `u16 blazon_price`, `u32 outcome`,
   **four bytes that are read and discarded**, `i32 node`.
6. `offered`: `u32 count`, then `count` x u16 slot index (0xFFFF = none).
7. `deferred`: the same.
8. `people`: `u32 count`, then `count` character records (2.3).
9. `band`: `u32 count`, then `count` x i32 position in `people`.
10. `recovering`: the same.
11. `team`: the same.
12. `workshops`: `u32 count`, then `count` production records (2.4).
13. `camp_extras`: `u32 count`, then `count` x 32-bit values (raw, not references).
14. `labels`: `u32 count`, then `count` wide strings.
15. `previous`, `current`, `selected`, `blazon_node`  -  four u16 slot indices, 0xFFFF = none, **in that order**
    (CAMP-023). The retail fixture reads (0xFFFF, 21, 0xFFFF, 0xFFFF): no previous mission, current mission
    slot 21, nothing selected, no blazon node.
16. only when `V > 29`: `recent`: `u32 count`, then `count` x u16 slot index.
17. only when `V >= 41`: `town_result`  -  i32.
18. only when `V >= 48`: `town_code`  -  4 bytes (a level code).

On the four discarded bytes (CAMP-022): four bytes stand between the outcome and the node reference, and the
reader consumes them without keeping them. The two campaign blocks of each retail save differ there, so the
value is not a function of the campaign state; what the writer puts there is **not established** and no claim is
made about it. An implementation may write zeros.

#### 2.6.3 A saved game (`Savegame/Profile_<nnn>/<name>`)

| Offset | Content |
|---|---|
| 0 | the four bytes `GSHR` |
| 4 | `u32 archive_version` (48 in the retail build) |
| 8 | `u32` holding the four bytes of the level code of the mission the save belongs to |
| 12 | `u32 archive_version` again |
| 16 | the **backup** campaign block  -  present only when `V > 38` |
| ... | the **live** campaign block  -  always present |
| ... | the level and actor state, written by a separate serializer (004c4570); not specified here |

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
| profiles |  -  | `count` records as below |
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
   32-bit floats giving the screen width and height (CAMP-113  -  the float pair the community resolution patcher
   edits at profile offset 0x104).
6. one further coordinate pair: two 32-bit floats.
7. the player's name: `u32 count`, then `count` 16-bit characters. The reader allocates from the count it read,
   so **no wire limit is established here** (CAMP-116) and revision 3's "at most 31" is withdrawn; the value 31
   appears only in the reader's string-capacity handling and was not traced to an input or interface limit. Any
   limit OpenSherwood puts on entering a name is its own choice.
8. the save-slot manager: a block tag, `u32 next_number` (the counter behind the numbered slot names),
   `u32 count`, then `count` slots, each a block tag, a byte-string *data file name*, a byte-string *thumbnail
   file name* and a wide-string *label shown to the player* (CAMP-114).

### 2.7 Interface tables

Three fixed correspondences inside the executable interpret fields of the player's own files. Under ADR-0009
section 5, clarification of 2026-09-13: a mapping from a data-file field, flag or id to its meaning is an
**interface fact** the player's files require and is allowed entry by entry with its data-file provenance,
exactly like the native table, while a table of tuned values the program carries as content stays refused.
Accordingly each table below is given entry by entry with the address that establishes it and the data-file field
it interprets. None of them is a table of tuned values: every row names one datum the player's files carry.

**T1  -  the ten capability flags of a mission slot** (CAMP-048, CAMP-315). Field interpreted: the ten single-byte
flags at offset 22 of each `SCOT` record of a mission `.rhm` (`docs/formats/rhm.md`, previously unnamed bytes of
that record). Established at 0056b8d0, which reads the ten bytes in file order and appends the capability id of
every non-zero one to the node's requirement list. Consumed by native 170 through the matcher at 004574b0, which
looks the id up in the character profile's capability arrays (2.2, CAMP-315).

| Flag, in file order | Capability id it requires |
|---|---|
| 1st | 21 |
| 2nd | 22 |
| 3rd | 28 |
| 4th | 1 |
| 5th | 25 |
| 6th | 27 |
| 7th | 2 |
| 8th | 9 |
| 9th | 13 |
| 10th | 23 |

These are the same id space as the entries of the two capability arrays of a character-profile record of
`Configuration/profile.cpf` (`docs/formats/profile.md`: the small values of the player-character record's
trailing block), which is what makes the match possible. The three substitutions of CAMP-315 apply on top: a
requirement for 2 is also met by 3 and one for 13 by 14, in the three-entry array, and one for 25 by 26 in the
four-entry array.

**T2  -  the item kind a workshop produces** (CAMP-325). Field interpreted: the item-kind id carried by a pick-up
element of a level. Established at 00580350; consumed by the stock recount at 00580430, which sums the quantity
of every active pick-up element whose item-kind id matches, and by the placement routine at 00580b70, which
creates elements of that kind. The ids live in the same space as the pick-up item field of the `ZORG` chunk
(`docs/formats/rhm.md`) and as the inventory property ids of natives 117 and 118 (`spec-script-vm.md`).

| Workshop kind | Item kind it produces and counts |
|---|---|
| 0 | 1 |
| 1 | 4 |
| 2 | 5 |
| 3 | 11 |
| 4 | 12 |
| 5 | 13 |
| 6 | 16 |
| 7 | 17 |
| 8 | 19 |
| 9..12 | none; the recount is skipped for these kinds |

**T4  -  the carried-stock slot of a capability** (CAMP-073). Field interpreted: the nine 16-bit stock counters of
a character record (2.3) and the capability ids that address them, which are also T1's requirement ids and T2's
workshop item kinds. Established at 0055c2c0, the single routine that writes a stock slot, with the wire order of the nine slots
fixed by the character record's serializer at 0055c3c0; consumed by
character initialisation (3.8) and by the workshop pass, and read by the HUD action icons
(`spec-ai-combat.md`, the three action icons and their stock).

| Capability id | Stock slot, in the wire order of 2.3 |
|---|---|
| 12 | 1st |
| 11 | 2nd |
| 1 | 3rd |
| 17 | 4th |
| 16 | 5th |
| 4 | 6th |
| 13 and 14 | 7th (both ids address the same slot, which is why a requirement for 13 is satisfied by 14, CAMP-315) |
| 5 | 8th |
| 19 | 9th |

Any other id addresses no slot and is discarded.

**T3  -  the supervising character of a workshop** (CAMP-327). Field interpreted: the **second string** of a
character-profile record of `Configuration/profile.cpf`  -  the field `docs/formats/profile.md` calls `sequence`.
Established at 005806c0, which walks the workshop's assignment list and answers with the first assigned
character whose profile's `sequence` equals a fixed name, or with nothing. The identities below are native 256's
(`spec-script-vm.md`), given as the stable way to name a character; note that native 256 itself compares a
*different* field, the record's **first** string, the one `profile.md` calls `sprite` (0057ba60).

| Workshop kind | Supervising character, by campaign identity (native 256) |
|---|---|
| 0 | 0 |
| 1 | 0 |
| 2 | 4 |
| 3 | 3 |
| 4 | 2 |
| 5 | 2 |
| 6 | 5 |
| 7 | 3 |
| 8 | 2 |
| 9 | 0 |
| 10 | 1 |
| 11 | 2 |
| 12 | none |

Because the two tests read different fields, **both** of the hero's appearances qualify as the supervisor of
kinds 0, 1 and 9 (CAMP-333): native 256 has a separate case for each of the two records' `sprite` values and
answers 0 for both, while the two records share one `sequence`, which is the field T3's test compares. Checked
on the retail file: the ten player-character records hold ten distinct `sprite` values and nine distinct
`sequence` values, the single collision being the hero pair. This also corrects `docs/formats/profile.md`, which
records `sequence` as "equal to it [the sprite] for all 102 entries": for the ten player-character records the
two strings are never equal. Revision 3's caveat that only one hero record might qualify is withdrawn.

What the supervisor contributes to a day's output is settled in 3.11: his presence multiplies the day's output
by a factor fixed per workshop kind (1.5 for kinds 0..8 and 11, 2.0 for kinds 9 and 10) and his absence by 1.0.
Revision 5 still called that contribution unknown.

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
7. every code in `n.after` names a slot whose `outcome != 0`  -  every prerequisite has been **played**, won *or*
   lost (CAMP-061)
8. every code in `n.until` names a slot whose `outcome == 0`  -  no blocker has been played (CAMP-062)

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
| b | more than one candidate | **obligatory.** Sort so that candidates with a non-zero `obligatory` come first. If the first candidate is obligatory and the second is not, the list becomes just the first and the rest are removed with `age := 0`. Two obligatory candidates at once **terminates the original** (CAMP-005, CAMP-063); OpenSherwood instead records a diagnostic and leaves both, which is a deviation (8.1) |
| c | `offered` only, more than one candidate | the **assault / defend** random filter (3.3.2) |
| d | `offered` only | **blazon node and tactical deferral** (3.6). In the **multi-candidate** case its first act is to clear `deferred`, which also sets every slot that was in it back to `age := 0`; with zero or one candidate `deferred` and its members' ages are left untouched (CAMP-264) |
| e | more than one candidate | **recency.** Visit `recent` from newest to oldest. For each history entry, remove every candidate that is that mission, with `age := 0`; **stop as soon as fewer than two candidates remain.** A recent mission can therefore survive as the sole offer (CAMP-211) |
| f | more than one candidate | the **non-assault / defend** random filter (3.3.2) |
| g | more than one candidate | **one per location.** Sort by (map name, `priority`) ascending, then walk the sorted list keeping, per run of equal `location`, the candidate with the lowest `priority` and removing the rest with `age := 0`. Two candidates with the same location and the same priority **terminate the original** (CAMP-005, CAMP-065); OpenSherwood records a diagnostic and retains both, a deviation (8.1). Because the sort key is the map name while the elimination compares the location identifier, this step does not guarantee one candidate per location for inputs where map and location disagree |

If the working list is non-empty the reduction is done and becomes the new source list. If it is empty, another
attempt starts from the (possibly mutated) source list. After ten attempts the program reports  -  non-fatally  - 
that it cannot determine an accessible mission, prints the campaign state, reports that it is forcing a mission,
restores the working list from the source list, and then (CAMP-204):

- **for the primary list only**, step d is re-run on the restored working list; for the secondary list it is
  **not** (step d belongs to the primary branch, as does the age reset below);
- if the working list then has fewer than two entries it is used as it stands  -  **which may be empty**;
- otherwise one entry is drawn uniformly at random and becomes the whole list, and, **on the primary branch
  only**, every slot of the source list other than the drawn one gets `age := 0`.

The forced path therefore cannot produce a mission from an empty source, and the two branches are not
interchangeable.

#### 3.3.2 The two random filters

Each filter is "remove those that match", over a copy of the list, followed by **restoring the copy if the list
became empty** (CAMP-212). A candidate matches when

- its node's kind is 1 or 5 (the first filter) or is neither 1 nor 5 (the second filter), **and**
- its `age` is 0, **and**
- `n.prob < rand() % 101`.

The draw is taken **only** for candidates that pass the kind and age tests, once each, in list order; a matching
candidate also gets `age := 0`. Determinism: the original draws from the C run-time generator; ours draws from a
named stream (section 8).

### 3.3.3 From a mission ending to the next offer

The sequence an implementation must reproduce between one mission finishing and the next being offered
(CAMP-224), assembled from 3.5, 3.3 and 3.4:

1. The level tick notices the won or lost flag and runs the mission-end bookkeeping of 3.5 - outcome, the
   hostile-survivor pass, the revive pass, and on a victory the counters, the recruits and the blazon refresh.
2. The level ends and the debriefing is shown; the campaign is untouched by it.
3. The campaign is asked for the next mission (3.4). With nothing selected it rebuilds the offered list (3.3);
   then, **before** the team reset, it resolves every offered defend node whose blazon requirement is zero as
   won (CAMP-221); then it resets `team` to `band`, sets `previous` to the mission that just ended, and sets
   `current` to either the single camp-free offer or the camp.
4. **`recent` receives the newly chosen `current`**, not the mission that just finished, and only when that
   choice is not the camp - entering the camp appends nothing (CAMP-224). Revision 8 had this backwards.
5. The chosen level is loaded. In campaign mode the game loop then writes the campaign backup file and, when the
   level is not the camp, the restart auto-save; these are **automatic checkpoints**, not player saves, and no
   player-initiated save is involved anywhere in this sequence. In the camp, and only when the loop is not being
   entered from mission selection, it submits message 1001, whose handler completes at once (3.9), and then runs
   the workshop pass (3.11).
6. In the camp the player sees the day's production placed, his men at their work spots, the campaign-map screen
   with one offer per location, and the town notification if one is pending. Choosing an offer ages the other
   offers (3.4) and sets `selected`; the next pass through step 3 takes that branch instead.

**This is the ordinary path only.** What the campaign does when it ends - the node whose win gives 100 per cent
progress, and the outro node - was not traced, so the terminal transition is not specified here and an
implementation must not assume the loop above simply repeats for ever (section 11, `TerminalTransition`).

### 3.4 Choosing the next mission and the camp

When the campaign is asked for the next mission (004539f0) (CAMP-220):

1. If `selected` is set: `previous := current`; `current := selected`; `selected := null`. **No team reset.**
2. Otherwise: rebuild the offered list (3.3); then, for every offered candidate whose node is a defend node with
   `blazons_needed == 0`, record it as **won** (3.5) (CAMP-221); then `team := band` (CAMP-222  -  the reset
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
| won | `siege_state := n.siege_on_win` unless that byte is -1; `s.outcome := 1` |
| lost | `siege_state := n.siege_on_loss` unless that byte is -1; `s.outcome := 2`; **and if `s` is `blazon_node`, `blazons := 0`** |

In both cases, if `s` is `blazon_node` and `n.kind == 5`, the one-shot town notification is armed:
`town_code := n.code`, `town_result := s.outcome`. The campaign-map screen reads `town_result` and clears it
(CAMP-241).

**End of a mission** (004e3260, run by the level tick once the won / lost flag is set, `spec-script-vm.md`
VM-103), in this order (CAMP-250):

1. Record the outcome on `current` (3.5)  -  which, on a loss, can clear the blazon counter. So "nothing changes
   on a loss" is false.
2. Walk the level's non-player actor list in element order. Two tests select the actors that count
   (CAMP-014): the element's kind must be the **soldier family** - narrower than "non-player human", since
   civilian-family actors are not selected - and the actor's **side** accessor must answer 1, the hostile side
   (`spec-ai-combat.md` AI-010 owns the side notion). For each selected actor: if its health is at most 0 it
   counts as **killed**, otherwise it counts as **spared** and a per-mission spared counter on the level is
   bumped. A spared actor additionally contributes **70 score points** when it satisfies the predicate native
   **89** exposes - the actor is **tied** - **or** when it carries one particular flag whose meaning was not
   traced. So the bonus rewards leaving an enemy alive and bound, which is what the game asks the player to do.
   Revision 8 called the test a profile combat class; it is an actor state, and the correction comes from the
   native that tests the same state. **These are enemies, not the player's men.**
3. Walk the level's **player-character** list in element order. For each character whose `revive_flag` (2.3) is
   non-zero, in this order: clear the flag; perform **two** operations on the actor whose meaning was not
   established; set his health to **50**; then **give the actor one further action, dispatched immediately
   rather than queued behind whatever it is doing** (CAMP-254). What that action makes the man do, and the
   posture it leaves him in, were not read, so the pass's effect on the actor stays excluded
   (`RevivePassEffects`, section 11): an implementation knows *when* the transition happens - at the end of
   the mission, before the campaign's own bookkeeping continues - but not *what* it is. This is a revive pass
   over the player's own men, not a loot pass. Steps 1 to 3 run whatever the outcome.
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

- **More than one candidate.** Clear `deferred`, which also resets the age of every slot that was in it. Then
  `blazon_node` := the first candidate whose node kind is 1 or 5,
  or null. Then walk the candidates:
  - a candidate whose node kind is 1 or 5 and which is not `blazon_node` is removed with `age := 0`;
  - a candidate whose node kind is 6 (tactical) is, when `blazon_node` is null, removed with `age := 0`, and
    otherwise removed and appended to `deferred`;
  - every other candidate, and `blazon_node` itself, stays.
- **Exactly one candidate.** `blazon_node` := that candidate if its node kind is 1 or 5, else null. Nothing is
  removed, and `deferred` is **not** cleared.
- **No candidate.** `blazon_node` := null, and `deferred` is **not** cleared.

So `deferred` holds the tactical missions held back while an assault or defend node owns the blazon counter  - 
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

`men_per_blazon` converts men into blazons (00455a70). Every field it reads comes from the **selected** node
(`selected`, not `current` and not `blazon_node`), and the head count is the length of **`team`** (CAMP-263):

- if the selected node's `men_per_blazon` is zero the original **terminates** (CAMP-005) before dividing;
  OpenSherwood records a diagnostic, answers 0 and performs no division, which is a deviation (8.1);
- otherwise `groups := team_size / men_per_blazon` (unsigned) and
  `outstanding := (u16)(blazons_needed - blazons_held) - blazons`, both of the selected node;
- the answer is `men_per_blazon * min(groups, outstanding)`, where the `min` compares its two operands as
  **16-bit unsigned** values.

There is no clamp to zero. An exactly met requirement gives `outstanding = 0` and the answer 0. Only an
*exceeded* requirement underflows, and even then the answer need not be large, because the team-derived bound can
still win the comparison: with a team of 8, `men_per_blazon` 3, an outstanding requirement of 2 and 5 blazons
already held, `groups` is 2, `outstanding` underflows to 65533, the `min` is 2, and the low word of the answer is
**6** (B35).

### 3.7 Progress, score, spared percentage, game length

**Progress** in percent (00456db0) (CAMP-270). Count nodes whose kind is neither 3 nor 6  -  ambushes and tactical
missions do not count, placeholders do  -  as `total`, and how many of those have `outcome == 1` as `won`. Two
level codes short-circuit, and the 100 case takes precedence when both are won:

| Won node's level code | Progress |
|---|---|
| `HI` | 100 |
| `DD` | 95 |

Otherwise the answer is `won * 100 / total`, an **unsigned** division **with no zero-denominator guard**:
`total == 0` divides by zero. The two codes are individual compatibility tokens, marked as such; the field they
interpret is the two-letter `code` of a level-table record of `Configuration/profile.cpf` (2.2), which is why
they are named here rather than described as "the last story mission"  -  that description does not define the
behaviour for reordered or modified data (CAMP-273).

**Score** is counter 2 (3.5). **Spared percentage**: the campaign-map status line computes
`counter4 * 100 / (counter4 + counter5)` as an unsigned division, guarded to 0 when both are 0 (CAMP-101); the
profile updater takes the same guard and then converts a floating-point value whose expression the export does
not contain, so the two are not proved to agree. Because counters 4 and 5 count hostile actors (3.5), this
percentage is the share of enemies **left alive**, which is what makes it the game's "spared lives" statistic.

**Game length** is counter 6 in seconds. It accrues at **checkpoints**, of which the program has three kinds
(CAMP-271): a *transition* checkpoint when a level begins, ends or is left; a *suspension* checkpoint when play
pauses without the level ending  -  the in-game menu accrues on entry and reopens the interval on leaving; and a
*continuing-save* checkpoint when a save is written while play goes on. Revision 6's "not at level end" was too
narrow (**R41-1**): a level ending is one of the transition checkpoints. a start marker is set when a level begins or resumes; whenever a save is written the campaign adds
`(system millisecond counter now - start marker) / 1000`, truncating, and **clears the marker**, which is then
set again. One display path adds the *unscaled* millisecond difference to the same value, which mixes units;
that path feeds a diagnostic line only (CAMP-272). The truncation loses the sub-second remainder of every
interval, so the original's counter drifts downward relative to real time.

Counter 6 is hashed state, so OpenSherwood does not read a wall clock for it (section 8.4): the interval is
counted in logic frames (ADR-0010) and the remainder is kept, so that repeated sub-second saves neither lose
time nor desynchronise.

**Difficulty** is `spec-ai-combat.md` AI-045's: 0 easy, 1 medium, 2 hard, stored as the last of the profile's
seven summary values. No campaign routine in the scope read branches on it (CAMP-280).

### 3.8 Recruits

**Creating a character record** (0055bc90) (CAMP-074). A record is built from a character profile and a flag
saying whether the profile is a named hero, and every field starts at a defined value:

- `health` := 100; `revive_flag` := 0; `camp_slot` := 0xFFFF; the name as 2.3 describes;
- `deployed` := false;
- the two experience pairs: `exp[0].level` is copied from the player-character profile's **melee experience**
  field, the third and fourth bytes of the record's eight-byte stat block in the wire order of
  `docs/formats/profile.md` (its `pre[4..5]`), and `exp[1].level` from the **bow skill**, that block's first two
  value bytes (`pre[2..3]`); both point remainders start at 0. `spec-ai-combat.md` owns what those two stats
  mean; this document only says which one feeds which slot;
- the nine stock slots: **a generic man gets 0** in each of the three slots his profile's three-entry capability
  array names, and nothing anywhere else, so he starts empty. **A named hero** gets the three starting stocks
  stored in his own profile, assigned through T4 to the slots his three capability ids name, after the
  difficulty adjustment `spec-ai-combat.md` AI-045 specifies (on easy a stock of 6 becomes 8 and one of 12
  becomes 15; on hard 6 becomes 4 and 12 becomes 9; medium leaves them alone). The adjustment is applied to the
  copied values, not to the profile.

This is the initialisation revisions 1 to 7 left excluded; nothing about a new record is undefined any more.

Adding a man (004524b0) (CAMP-290). The list `recovering` decides which branch runs, and each branch has its own
draw; revision 3's "no random draw is taken at all" was wrong (CAMP-290):

- If `recovering` is **empty**, the coin flip is **skipped** (the test short-circuits) and the new-man branch
  runs. The new-man branch builds a candidate list from the character-profile table, keeping only profiles not
  marked named heroes, and then **takes one draw** to choose among them; the expression that turns the draw into
  a choice is absent from the export (section 10).
- Otherwise **one draw** is taken and its low bit examined. On one value the new-man branch above runs, with its
  own further draw. On the other, **one draw** picks an entry of `recovering` uniformly and that man is
  recovered: his experience state is adjusted, his carried-item fields are cleared, `camp_slot` becomes 0xFFFF,
  his health is written from a value whose source expression is absent and then clamped to 100 ("re-rolled" is
  not supported), he moves into `band`, and `recruit_flag := 1` (CAMP-292).

So the draw count per added man is: one on an empty recovery list; two otherwise.

Two of recruitment's quantities are **not** settled and neither may be replaced by a chosen default
(`RecruitArithmetic`, section 11; revision 8's "documented default" is withdrawn, because inventing a number
would put an assumption on the winning path, which section 12 forbids):

- **how many men a victory adds** - a value the level computes and the campaign truncates to an integer, the
  expression absent from the export;
- **which profile the new-character draw selects** - the draw is taken and the candidate list is built from the
  non-hero profiles, but the mapping from one to the other is absent as well.

What an implementation may rely on meanwhile is only the *shape*: after a won mission, zero or more men join the
band, each built by the branch rules above and initialised as CAMP-074 specifies; the debriefing shows that
number and the band holds it when the camp is entered; after a lost mission nobody joins. Recruitment as a whole
is therefore incomplete, and a campaign built on this document cannot yet claim to reproduce the original's band
growth.

**Generated names** (CAMP-071) consume the same care. The campaign keeps a list of names already generated  - 
this is the `labels` list of 2.1, whose purpose revision 3 left unknown (CAMP-295). Building a name repeats at
most ten times: each attempt takes **two** draws, one text id from 100..121 and one from 122..143, and joins the
two texts with a single space; the attempt succeeds when the result is not already in `labels`, and the name is
then **appended to `labels`** and used. If all ten attempts collide, the condition is reported non-fatally and a
fixed fallback name compiled into the executable is used instead and is **not** appended.

The number of men added after a won mission is a value the level object supplies and truncates to an integer;
**the expression that produces it is absent from the export** and it is not established as a simple accessor
(CAMP-291, section 10). Men are added one at a time, so *k* recruits make *k* independent passes.

Men leave the band by being moved to `recovering` or by being dropped outright; both are driven by the HUD and
mission-end paths of 005145b0, which were not read.

### 3.9 The camp between missions

Entering the camp is entering node 0 as an ordinary mission on the `sherwood` map with its own script
(`docs/formats/sherwood-hub.md`). Around it the game loop does (0050f710) (CAMP-300):

1. Write `Campaign.bck` from the live campaign.
2. Create the *restart* auto-save  -  only when the level being started is **not** the camp (3.10).
3. If the level is the camp  -  the game object's "in the camp" flag, set from `current`'s `location == 8`  -  and
   the play loop is **not** being entered from the mission-selection state: submit **message 1001** to the camp
   level's script, then start the workshop pass (3.11). Returning from an accepted selection of the current node
   submits the message a second time (VM-113) but does **not** run a second workshop pass.
4. Enter the frame loop.

Message 1001 is engine-originated and goes to the **hub level only**. The pinned `spec-script-vm.md` settles
its occurrence conditions in VM-113 and this document adopts them rather than restating a narrower rule: it is
submitted (a) right before the play loop starts, unless the loop is being entered from the mission-selection
state, and (b) when the loop returns from mission selection **and** the selection was accepted **and** the
selected mission is the current node. There are therefore **two** submission sites, not the single camp-start
one revision 4 described (CAMP-301). This closes `sherwood-hub.md`'s open question about where the message comes
from.

**The handler runs to completion inside the submission** (CAMP-303). The submitting helper builds a
single-element message sequence with the **level** as its target and hands it to the sequence manager; the
pinned `spec-script-vm.md` settles what happens next, in VM-210 (a sequence handed to the manager is started at
once) and VM-212 (a message element whose target is null is executed by the level executor immediately, not
queued). So the camp script's answer to 1001 - its calls to natives 199 and 200 - completes before the
submitting code continues, and therefore **before the workshop pass and before the first tick**. Revisions 4 to
7 recorded this order as unknown and excluded it; the pinned dependency settles it and the exclusion is lifted.
The consequence is the one that matters in practice: every workshop has its zone and its full, freshly appended
spot list before the first day's production runs.

**Message 1000 is never sent in the retail build** (CAMP-302). The engine originates exactly one message and it
is 1001: the only routine that submits an engine message has a single caller, the game loop, and it submits 1001
there (`spec-script-vm.md` VM-122 states the same restriction from the other side). No script of the 39 retail
files sends 1000 either (`docs/formats/sherwood-hub.md` section 5). The camp script's handler for 1000 is
therefore **unreachable in the retail data**, and an implementation needs no send path to play the campaign: the
team leaves the camp by the ordinary route of 3.9 - the player picks a mission on the map, the members he put in
the deployment zone are the team, and the mission starts. Revisions 4 to 7 recorded the origin as unknown and
excluded it; it is not unknown, there is none.

**Team deployment.** The team is `team`; it is reset to `band` only on the unselected-next-mission path (3.4).

| Native | Effect |
|---|---|
| 163 | the length of `team` |
| 164 (`i`) | the element of `team[i]`; no bound check |
| 165 (`pc`) | refuses a non-player-character with a script error. Otherwise: append the character's record to `team` if it is not already there, and refresh the HUD. It then writes three fields **of the actor**, one of them to the value 2; those fields were not identified, they are not the saved `deployed` flag, and their effect is **excluded from clearance** (section 11, `Native165ActorEffects`). An implementation that omits them changes observable camp behaviour by an unknown amount (CAMP-310) |
| 166 (`pc`) | remove the character's record from `team`; refresh the HUD. Same refusal |
| 170 | 1 when the selected node's requirements are satisfied (below) |
| 172 | the selected node's four-byte code, 0 when nothing is selected  -  what the camp script compares to notice a pick |
| 174 | the selected node's `team_limit`, or 5 when nothing is selected; reports an error and answers 0 unless the **current** node's location is 8 (CAMP-311) |
| 249 | the number of **HUD-selected** player characters. This is the interface selection and must not be conflated with `team` (CAMP-314) |
| 250 (`i`) | the i-th HUD-selected player character; error and null at or above the count |
| 173 | a byte of the game object, called the **menu-choice state** here. What is established (CAMP-317): exactly one interface routine writes it, and exactly two actions call that routine. **Both** actions accept a mission - each stores the chosen node as `selected` and closes its dialog - but one sets the byte and the other clears it, so the value **survives** the dialog and distinguishes the two ways of accepting rather than "a screen being open". Its gameplay meaning, its initial value and when it is cleared other than by the second action were **not** traced, so it stays excluded (section 11, `Native173State`). Revision 8 called it "the mission-selection screen is open"; that was wrong |

**Native 170's test** (00453cd0) (CAMP-312), in this order:

1. For every entry of the node's `required_pcs`: some member of `team` must have that character profile
   (identity comparison of the profile the member points at).
2. For every entry of the node's `capability_reqs` (built from the slot flags through interface table T1, 2.7):
   some member of `team` must have that capability. A member
   has capability *c* when *c* appears in either of two capability arrays of his character profile  -  one of
   three entries and one of four  -  or when one of three substitutions applies: a requirement for capability 2 is
   also satisfied by 3 in the three-entry array, a requirement for 13 by 14 in the three-entry array, and a
   requirement for 25 by 26 in the four-entry array (CAMP-315).

Both loops stop at the first unsatisfied entry.

**Instantiating the team.** When a mission starts each team member gets a player-character actor, his record is
marked `deployed`, and his `camp_slot` keeps a value only while the current node's location is 8, otherwise it is
set to 0xFFFF (CAMP-313). A node whose `required_pcs` cannot be matched to a spawn point produces a non-fatal
report and the spawn point stays empty.

**Which member goes to which spawn point** is decided by the spawn routine (00453e60), not by native 170, which
answers only whether the team *contains* what the node demands. The rule (CAMP-316):

What is established (CAMP-316), in the order the routine performs it:

1. **Required placement.** The mission file's spawn points are walked in file order. Each point carries a
   **numeric selector**; a non-zero selector names one required character *identity*, and the routine maps that
   number to a character-profile identity and looks for a team member whose profile matches. It is a numeric
   selector matched to a profile identity, **not** a comparison of the point's name (revision 8 said otherwise).
   A member that matches is placed at that point, given a player-character actor and marked `deployed`, and the
   point is marked used.
2. **Remembered camp slots**, and only when the current node's location is 8: after the required placement, each
   team member whose `camp_slot` is not 0xFFFF is placed at that remembered slot, which is marked used and
   removes one point from the remaining count. So camp slots **follow** the required placement rather than
   preceding it.
3. **Randomisation of the camp's points.** Still in the camp, the remaining points are shuffled: a fixed number
   of rounds - one hundred - each draw two point indices and, when they differ, exchange those two points and
   their used flags. Two draws per round, every round, so the routine consumes draws **repeatedly**, not twice
   (revision 8 said twice).
4. **The remaining allocation** fills the points left over. It takes each point's own capability requirements
   into account and prefers candidates that match exactly one point before relaxing to ambiguous matches.
5. **Camp initialisation clears `team`** as part of this path.

**What is not settled**: the exact ordering inside step 4, its limits, and the resulting assignment when several
candidates match equally. Two things revision 8 promised are therefore **withdrawn**: that a named point which
cannot be filled always stays empty, and that every remaining point is filled. What an implementation may rely
on is only what steps 1 to 3 state, plus the non-fatal report when a required character is missing from the team.
Broader spawn clearance stays withheld (`SpawnAssignment`, section 11).

### 3.10 Save slots and the auto-saves

The executable knows five fixed slot names  -  `Continue`, `Restart`, `QuickSave`, `ExQuickSave`, `Sherwood`  -  plus
numbered slots named from a `Savegame_%03i` pattern whose counter is the profile's `next_number` (CAMP-130).
Labels shown to the player come from text resources (0xF6 and 0xF7 for the *continue* and *restart* slots), so a
slot's file name and its label differ.

| Slot | Written when |
|---|---|
| `Restart` | at the start of every level that is not the camp (CAMP-131) |
| `Sherwood` | at the start of the camp level (CAMP-132) |
| `Continue` | after every successful save, the file just written is copied to `Continue`  -  unless two tests (which were not read individually) say the file *is* the continue or restart slot (CAMP-133) |
| `QuickSave`, `ExQuickSave` | the quick-save key; the older quick save moves to `ExQuickSave` |
| numbered | the save menu |

Writing a save (00511340): open the data file for writing; write the header; add the elapsed interval to
counter 6 and clear the start marker; when `V > 38` write the backup block by streaming `Campaign.bck` minus its
version dword; write the live campaign block; write the level state; set the start marker again; write the
thumbnail file; mirror into the continue slot; show a message from a text resource.

Loading consumes the same archive order  -  header, backup block when the version has one, live block, level
state  -  and applies it in that order, so the live block's values are the ones that survive. Two steps are
write-only (producing the thumbnail file, and mirroring into the continue slot) and one is load-only (the
diagnostic when the magic does not match). Nothing here prescribes how an implementation organises the two
directions (CAMP-134). "Restart" and "Continue" are therefore not separate mechanisms but ordinary saves under fixed
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
| 0..8 | *gated by victory:* compute the day's output, `stock += output`, `last_output := output`. *Ungated:* walk the work spots and place items of the workshop's item kind (interface table T2, 2.7) in the camp, up to 5 per spot, drawing down a **temporary** quantity initialised from `stock`; `stock` itself is not reduced by placement. Then recompute `full := 5 * (number of work spots) <= stock` (CAMP-322) |
| 9, 10 | *gated by victory:* compute the day's output and add it to **each assigned character's** experience  -  slot 0 for kind 10, slot 1 for kind 9, and kind 9 additionally requires that character's profile to carry **capability id 1** in its three-entry capability array, the same id space as T1 (CAMP-323) |
| 11 | *gated by victory:* compute the day's output and **restore health** by that amount to every assigned character. The cap is applied by comparing the **16-bit signed** sum of the current health and the amount against 100: within the ordinary domain this caps at 100, but an amount large enough to make that sum negative bypasses the cap and the setter receives the unclamped value (CAMP-324, B69a). Kind 11 is the rest workshop, not a training workshop |
| 12 | a separate routine, ungated, with no output at all; see "Kind 12" below (CAMP-338) |

Then, for every workshop, each assigned character is placed at his recorded spot in the camp level (CAMP-328).

The experience accumulator (CAMP-072, 0051d4e0): `points[slot] += amount`; then, while `level[slot] < 100` and
`points[slot] >= 100`, `level[slot] += points[slot] / 100`, `points[slot] %= 100`, and `level[slot]` is clamped
to 100.

**Per workshop**, immediately after that workshop's own per-kind work and its character placement and before
the next workshop is touched, two further steps run on the **level's location membership**  -  the record of which
actors are inside which location  -  clearing it and rebuilding it (CAMP-333a). So the rebuild happens thirteen
times per pass, not once at the end, and each workshop's placements are visible to the rebuild that follows
them. That membership is exactly what assignment capture later walks (the zone's member list), so the two steps
are part of the workshop pass's observable effect, not housekeeping. Neither step was traced: what membership
results, in what order, and under which predicates is **excluded from clearance** (section 11,
`LocationMembershipRebuild`), and with it the claim that the workshop pass as a whole is specified.

Two placements run, over two different lists, and revision 4 conflated them (CAMP-328):

- **Item placement** (kinds 0..8) walks the workshop's **work spots in registration order**, placing up to five
  items per spot.
- **Character placement** walks the workshop's **assignments in assignment order**. An assignment whose
  character has no actor in the level is **skipped**; for the others, after placing the character the routine
  records the workshop's kind on his actor, which is how the camp knows who works where.

A **separate campaign pass** (00456a70) captures the camp back into the campaign, per workshop, in kind order:

- **Stock recount**, kinds 0..8 only (00580430). Every element of the camp level is examined. An **active**
  element whose element-kind nibble is 3 (a pick-up) and whose item-kind id equals the workshop's (T2, 2.7)
  contributes its own quantity field. Additionally  -  and only for the workshop whose item kind is 1  -  an
  **active** element whose element-kind nibble is 7 and whose sub-kind field equals 5 contributes exactly 1 each;
  that element class and sub-kind were not named, so this is stated as the predicate the program applies, not as
  an identification (CAMP-326). Everything else contributes nothing. The recount **replaces** `stock`.
- **Assignment capture**, every kind (00580520). The pass walks the **member list of the workshop's zone**  -  the
  location native 199 gave it  -  and **not** the camp's player characters at large. When the workshop has no zone
  the whole capture is skipped and the previous assignment list stands. A member is captured when its
  element-kind nibble is 0xC (a player character) **and** it passes the per-kind eligibility test of 00580940:
  kind 12 accepts nobody; kind 9 accepts a character only when he carries capability id 1; every other kind
  accepts any player character. Each captured member contributes his character record, his position and his
  sector index (0xFFFF when he has none), in the order the zone lists him (CAMP-331a).

- **Teardown**, every kind, immediately after the capture: the workshop's **zone association is cleared** and
  its **work-spot list is emptied** (CAMP-336). The zone and the spots are therefore live only between the camp
  script's answer to message 1001 and this pass; a workshop that is captured twice without an intervening
  message 1001 has no zone the second time, so its assignments stand unchanged.

Capture **replaces** the assignment list whenever the workshop has a zone  -  including a zone with no members,
which leaves the list empty  -  and leaves the previous list alone only when there is no zone at all.

This pass has no direct caller in the export, so **when it runs is not established**. Because it also performs
the teardown above, its scheduling is not a detail an implementation can choose freely, and it is excluded from
clearance (section 11, `CaptureScheduling`); the assignments it produces are what the next camp day's output and
placement use.

#### A day's output

Read from the program's own instruction listings for 00581020, 005810c0, 00581180 and 00580e80, which carry the
floating-point operations the decompiler drops. Three inputs and one factor:

- **`yield`**  -  the `production_yield` (2.2) of the node the campaign has in `previous`, that is of the mission
  just played. It is loaded as a signed 32-bit integer from the record's unsigned 16-bit field, so it is always
  positive (CAMP-334).
- **`capacity`**  -  the workshop's own capacity, native 199's third argument, likewise widened from unsigned 16
  bits.
- **`assignments`**  -  the number of entries in the workshop's assignment list (2.4). Used by kinds 0..8 only.
- **`sup`**  -  the supervisor factor: the lookup of interface table T3 answers the first assigned character whose
  profile matches the workshop's kind, or nothing; `sup` is the larger value when it answers a character and
  **1.0** when it answers nothing.

The shape is the same for every kind that computes an output (CAMP-335):

```
scale  = f32( capacity x S )   // S is a double-width operand; the product is narrowed to binary32 (R43-1)
output = trunc_toward_zero( P x sup x scale )
```

where `P` is `yield x assignments` for kinds 0..8 and plain `yield` for kinds 9, 10 and 11, and:

| Kinds | `S` | `sup` with a supervisor | `sup` without |
|---|---|---|---|
| 0..8 | 0.001 | 1.5 | 1.0 |
| 9, 10 | 0.01 | 2.0 | 1.0 |
| 11 | 0.01 | 1.5 | 1.0 |

Evaluation order and precision are observable and part of the rule (CAMP-339):

0. **What is verified, and what is conditional (R41-2).** Verified individually and cleared: the operands (`yield`,
   `capacity`, `assignments`, the five constants), the factor order, the store of the scale to a 32-bit float,
   the truncating conversion into a signed 64-bit integer, and the bits each caller retains. **Conditional on
   `ProductionPrecisionMode`** (section 11): every *exact output value* this section computes, because the
   precision the multiplications run in is not established. An implementation may not treat a worked example
   below as a fixed expectation until an amendment settles the mode, proves equivalence over a stated input
   domain, or declares a deviation.
1. `capacity x S` is formed from the integer capacity and a **double-width constant**, and the result is then
   stored to a **32-bit float**. The narrowing is verified and is not incidental: the last multiplication reads
   the narrowed value, so an implementation that keeps the wider value produces different integers at the
   boundaries (B66a). What is **not** established is the effective precision and rounding mode of the
   multiplications themselves: the operand is double-width, but the unit's control word is set by start-up code
   this analysis did not read, and the helper of step 4 changes only the rounding mode and restores it. An
   implementation should therefore either establish that mode or demonstrate that its own arithmetic agrees over
   the supported input domain (section 11, `ProductionPrecisionMode`).
2. `P` is formed by an integer multiply inside the floating-point unit (`yield`, then `x assignments` for kinds
   0..8). Whether that intermediate is exact depends on the same unestablished mode, so its exactness is
   asserted only for magnitudes that fit the narrowest candidate significand.
3. The factors are then multiplied in the order `P`, `sup`, `scale`, in the unit's extended registers.
4. The product is converted by the C run-time's float-to-integer helper, which sets the unit's rounding mode to
   **truncate toward zero** and stores a **signed 64-bit** integer; callers then keep the low bits they need.
   The distinction matters outside the signed 32-bit range, where a 32-bit conversion would saturate or trap
   while this one does not (CAMP-339).
5. Kinds 0..8 keep the **low 16 bits** of that integer: `stock += (u16) output` and `last_output := (u16)
   output`. Kinds 9 and 10 mask the amount to 16 bits before spending it. Kind 11 passes the whole 32-bit value
   on, and the health update compares the sum as a 16-bit signed value.

Worked example (B66), **not conditional**: a workshop of kind 0 with capacity 5 and three assignments, after a
mission whose `production_yield` is 700, produces `trunc(700 x 3 x 1.0 x f32(0.005))` = **10** and
`trunc(700 x 3 x 1.5 x f32(0.005))` = **15**. The reviewer evaluated this expression over every combination of
24-, 53- and 64-bit significand with each rounding direction and the two integers are the same throughout, so
this case does not depend on the unresolved mode. Other inputs do; B66a is one of them.

Where the output goes is 3.11's table above: stock for kinds 0..8, an experience slot for kinds 9 and 10, health
for kind 11.

#### Kind 12

Kind 12 computes nothing. It has **no victory gate, no supervisor and no arithmetic** (CAMP-338): it walks the
workshop's **work spots** in registration order and pairs spot *i* with entry *i* of the campaign's
`camp_extras` list (2.1), creating one element from each value and placing it at that spot so that the element
ends up at the spot's position, facing as the spot records and belonging to the spot's sector, or to no sector
when the spot has none.

The loop stops as soon as **either** list is exhausted, so it places exactly `min(work spots, camp_extras)`
elements. An empty `camp_extras` places nothing and **reports nothing**: the check happens at the top of the
loop, before any placement. The defensive bounds check inside the body cannot fire while that loop condition
holds, and it would *throw* rather than report and continue, so revision 5's "reports a bounds condition and
then reads past the end" is withdrawn on both counts (CAMP-338).

The list is empty in the fixtures of this installation; that is an observation about those fixtures and does
**not** establish that a stock campaign's list is always empty. What fills the list, and which element class each
value becomes, were not established (open question 12, `CampExtrasSource`).

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
- **Mini-map colours.** Each element carries a dot style code and a set of colour entries, both filled when the
  element is created. The *number* of entries differs by element class: a player character and two further
  element classes get one colour each; a hostile human gets **five**, one of which replaces the other four when
  a single flag on his controller is set, the remaining pair standing for two further states. Which entry is
  used in which actor state was not established. **The colour values themselves are a palette and are excluded
  from this document** (section 11, `MinimapPalette`): ADR-0009 section 5's clarification admits data-file
  field-to-meaning mappings but expressly keeps refusing colour lists, and splitting one palette into individual
  facts does not clear it (CAMP-350).
- **Native 24** sets the style code. Accepted codes: 0, 1, 100, 101, 102, 111, 200, 201, 202, 222, 300, 301, 302,
  333, 500 and 666; anything else is an error that changes nothing. Codes 111, 222 and 333 additionally require
  the element to belong to the human family and report an error otherwise. Twelve of the accepted codes fall
  into three groups that each install one colour over the element's entries; the remaining four  -  0, 1, 500 and
  666  -  **change the style code only and leave the colour entries as they are**, so they do not restore a colour
  an earlier call overrode. Which colour each group installs is part of the excluded palette (section 11,
  `MinimapPalette`) (CAMP-351).
- State-dependent rendering  -  which of an element's colour entries is used, and whether an un-blipped element is
  drawn at all  -  is unspecified and excluded with the palette (CAMP-352). Revision 3's "no grey value appears in
  the image" is withdrawn: the evidence was the call sites of one colour-conversion helper, which cannot
  establish a negative about the whole program.
- Portrait states, action-icon availability and tooltips were **not** read; `ui-flow.md` has the geometry. The
  name under a portrait is the `name` field of 2.3.

---

## 4. Claims

Rows whose id carries **(R19-n)** answer finding *n* of spec review 19.

| Id | Claim | Status | Evidence | Confidence | Notes |
|---|---|---|---|---|---|
| CAMP-002 | Each record is preceded by a 16-byte block tag; on reading a mismatch takes the program's **fatal** diagnostic path, which reports, writes a crash dump and exits the process | observed | 005e1b70, 0060f1c0, 005f8030, 00643f50 | high | **(R23-1)**; revision 3's warning-only reading is withdrawn |
| CAMP-005 | The shared diagnostic helper takes a severity as its first argument: a non-zero severity prints a fatal-exception line, writes a crash dump, calls a registered handler and exits; a zero severity reports and returns. Fatal sites in this subsystem: the block-tag mismatch, two obligatory candidates, two candidates with equal location and priority, and a zero `men_per_blazon`. Non-fatal sites: the failed reduction and its forced mission, an empty offered list on the campaign map, the mission-file and chunk-validation reports, the profile-archive and save-header complaints, the ten-collision name fallback, and every script error | observed | 005f8030, 00643f50; 00452e70, 00452d20, 00455a70, 005e1b70 (fatal); 00451b70, 00527d20, 0056b8d0, 0055dea0, 0056ea70, 0055bf00 (non-fatal) | high | **(R23-1, R23-2)** |
| CAMP-003 | The town notification stores the node's four-byte level code, not an index | observed | 00456540, 00452f20, 005280d0 | high | **(R19-3)** |
| CAMP-004 | Byte and wide strings use a u16 count; the player's name uses a u32 count | observed | 005e0c90, 005e0ea0, 0055d1a0 | high | validated on the profile archive |
| CAMP-010 | A new campaign has all 27 counters 0 except money = 100 | observed | 00450ec0, retail backup file | high | |
| CAMP-011 | The counters are 27 x i32 | observed | 00452030, 00452f20 | high | |
| CAMP-012 | Natives 195/196 address counters 7..26 as script indices 0..19 | observed | 00579430, 00579470 | high | |
| CAMP-013 | Counters 4 and 5 count qualifying **hostile soldier-family** actors left alive and found dead, not the player's men; the 70-point bonus likewise concerns qualifying surviving hostiles | observed | 004e3260 (element-kind test plus a virtual side/family test), 004a1bc0 | medium-high | **(R19-18)**; the two virtual tests were not followed, so "qualifying" is not fully characterised |
| CAMP-014 | The mission-end pass selects an actor by two tests: the element kind is the **non-player human** kind (the human family of native 24's test, with the bit that separates it from a player character) and the actor's side accessor answers 1, the hostile side. A selected survivor earns 70 score points when its character profile's combat class id is **18** or when the actor carries one particular unnamed flag | observed | 004e3260; `spec-ai-combat.md` AI-010 for the side notion | high for the predicate, unknown for the flag | **(R29-15 residue)**; settles the hostile qualification revisions 4 to 7 excluded |
| CAMP-016 | Counter 0 is the clover HUD counter and no writer appears in the scope read | observed | 004c8140, 004a78c0 | medium | how it becomes non-zero is open |
| CAMP-020 | A slot carries age, blazon price, outcome and a node reference | observed | 0054cc40, 00452f20 | high | |
| CAMP-021 | Every campaign list is a u32 count followed by its elements in order | observed | 00452f20, 00453630, 00458be0, 00458e70, 00459160, 004593f0 | high | |
| CAMP-022 | Each slot record contains four bytes that are written and then discarded on reading; what the writer puts there is not established | observed for the framing | 0054cc40; the two campaign blocks of each retail save differ there | high for the framing | **(R19-5, R23-18)**; no claim about the origin |
| CAMP-023 | The four slot references are written previous, current, selected, blazon node | observed | 00452f20 (order), 004539f0 (previous is assigned from current) | high | **(R19-1)**; revision 1 reversed the first two |
| CAMP-030 | The siege state is one signed byte, -1 on a new campaign | observed | 00456e40, 00456e60, retail file | high | |
| CAMP-031 | A node is gated only when the first byte of its ten-byte gate array is 1 | observed | 0054c800 | high | |
| CAMP-032 | The gate reads `gate[1 + state]`, so state -1 reads the enable flag itself and passes | observed | 0054c800 | high | |
| CAMP-033 | Win, loss and over-age selection each set the state from one signed byte, -1 meaning no change | observed | 00456540, 004560d0, 00456e40 | high | |
| CAMP-041 | The level record's group after the location is u8, u16, u8, u16 | observed | 00566570 | high | corrects `profile.md` |
| CAMP-042 | The required-character list is counted by a u32 | observed | 00566570 | high | corrects `profile.md` |
| CAMP-043 | The trailing block is 10 bytes, then 3 signed bytes, then 7 u16 | observed | 00566570 | high | |
| CAMP-044 | The node kind is 0 story, 1 assault, 3 ambush, 4 camp, 5 defend, 6 tactical | observed | 005795c0, 00514730, 0054cbf0, 0054cc20, 00456db0 | high | |
| CAMP-045 | A node whose `needs_camp` byte is 0 may start without the camp | observed | 004539f0 | high | |
| CAMP-046 | Location 8 is the camp and several routines gate on it | observed | 00579300, 0050b640, 004552f0 | high | |
| CAMP-047 | `team_limit` is zero-initialised and set to the `SCOT` record count of the node's mission file. A file that cannot be opened leaves it at 0 **and is reported** non-fatally; a file that opens but has no `SCOT` chunk leaves it at 0 and is not reported; a chunk that fails validation is reported non-fatally and sets 5 | observed | 0056b8d0 | high | **(R23-5, R29-10)** |
| CAMP-048 | `capability_reqs` is built from ten flag bytes at offset 22 of each `SCOT` record; each set flag appends one capability requirement, duplicates included; the flag-to-capability correspondence is interface table T1 (2.7) | observed | 0056b8d0 | high | **(R19-6)**; 12 of 39 mission files carry them, covering 16 slots (Codex's read-only inspection) |
| CAMP-050 | 200000 in the maximum-money field means "no maximum"; the money and gang comparisons are unsigned | observed | 0054c800, 0054c8a0 | high | **(R19-10)** |
| CAMP-060 | The availability test is the eight conditions of 3.2, in that order | observed | 0054c800, 0054c8a0 | high | |
| CAMP-061 | The *after* list requires each prerequisite to have been played, won or lost | observed | 0054c6f0 | high | contradicts `profile.md` |
| CAMP-062 | The *until* list requires each blocker to be unplayed | observed | 0054c6f0 | high | |
| CAMP-063 | Obligatory filtering needs more than one candidate and obligatory means a non-zero flag; two obligatory candidates take the **fatal** diagnostic path, so the subsequent retention of both is not reachable in the original | observed | 00452e70, 005f8030 | high | **(R19-10, R23-2)** |
| CAMP-064 | A story candidate at `max_age - 1` becomes the sole offer; at `max_age` a candidate is removed, and a removed defend/assault candidate with outcome 0 is recorded lost | observed | 004526f0 | high | requires more than one candidate |
| CAMP-065 | The one-per-location step sorts by (map name, priority) and eliminates by comparing the location identifier, so it guarantees one per location only when map and location agree; two candidates with the same location and priority take the **fatal** diagnostic path | observed | 00452d20, 0054c780, 005f8030 | high | **(R19-10, R23-2)** |
| CAMP-070 | The character record is: tag, four u32 (two experience pairs, level before points), tag, health, one u8, nine u16, camp slot, wide name, profile index, deployed flag  -  with no unattributed bytes | observed | 0051d5d0, 0055c3c0, 0055bb30; exact re-parse of the retail 98-byte record | high | **(R19-2)**; cleared as fact by review 23 |
| CAMP-071 | Generated names draw one text id from 100..121 and one from 122..143, joined by a space, at most ten times, taking the first result absent from the `labels` history (CAMP-295); a named hero's name is text 0x90 + his position in a seven-name list, with a fixed fallback when he matches none | observed | 0055bf00, 0055bdf0, 004565c0 | high | **(R19-23, R34-9)**; the exit condition is no longer open |
| CAMP-072 | An experience slot is a (level, points) pair; adding points carries into the level in hundreds and clamps the level at 100 | observed | 0051d4e0 | high | |
| CAMP-073 | The nine 16-bit fields of a character record are the carried stock of nine capability kinds, addressed by capability id through interface table T4; ids 13 and 14 share one slot and any other id addresses none | observed | 0055c2c0 | high | the ids are T1's requirement space and T2's item space, so a workshop stocks the slot its item kind names |
| CAMP-074 | A new character record starts at health 100, `revive_flag` 0, `camp_slot` 0xFFFF, both experience levels copied from two stat fields of the character profile with zero point remainders, and its stock slots at 0 for a generic man or at the profile's three stored starting stocks for a named hero, difficulty-adjusted as `spec-ai-combat.md` AI-045 specifies | observed | 0055bc90, 0051d480, 0055c2c0; AI-045 for the adjustment | high | settles the persistent-character initialisation revisions 4 to 7 excluded |
| CAMP-090 | A workshop record carries kind, capacity, stock, last output, a capacity flag and an assignment list | observed | 00580970, 00580ab0 | high | |
| CAMP-091 | Work spots are not saved; the camp script re-registers them each load | observed | 00580220, 00580970 | high | |
| CAMP-093 | An assignment's trailing u16 is stored from archive version 47; at version 46 a u16 is read and discarded and the field becomes 0xFFFF; below 46 there is none | observed | 00580ab0 | high | **(R19-17)** |
| CAMP-100 | The profile's serialized summary is six numbers plus the difficulty | observed | 0055d370, 0055d1a0, retail archive | high | |
| CAMP-102 | The routine that copies the campaign counters into the profile summary is called from three places in the game loop; what makes each call happen was not traced, so **when** score, money, progress and cumulative profile play time update is not established. An ordinary save is not one of those places | observed for the call sites, unknown for the triggers | 0050f710, 0055d370 | high / unknown | **(R29-14)**; revision 4 inferred wrongly from an empty caller inventory |
| CAMP-101 | The campaign-map spared percentage is an unsigned integer division of counter 4 by the sum, guarded to 0 when both are 0; the profile updater applies the same guard and then a floating-point conversion whose expression is absent | observed | 00528810, 0055d370 | high for the map, medium for the profile | **(R19-21)** |
| CAMP-110 | The profile archive begins with `FORP` and a u32 version that must be 6 | observed | 0055dea0, retail file | high | |
| CAMP-111 | The profile's seven u32 are unknown, score, money, spared, play time, progress, difficulty | observed | 0055d1a0, exact re-parse | high | |
| CAMP-116 | The player's name is a u32 count and that many 16-bit characters; the reader allocates from the count it read, so no wire limit on the name is established | observed | 0055d1a0 | high | **(R23-16)**; revision 3's "at most 31" is withdrawn |
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
| CAMP-134 | Both directions consume one archive order  -  header, backup block when the version has one, live block, level state  -  and loading applies it in that order, so the live block's values survive. Producing the thumbnail and mirroring into the continue slot are write-only effects; the magic diagnostic is load-only | observed | 00511340 | high | **(R23-18, R29-6)**; no claim about how the two directions are organised |
| CAMP-200 | A new campaign's band is one man from character-profile index 1; a command-line string can replace it | observed | 00450ec0, 0048f280 | high | |
| CAMP-201 | Level-table record 0 is the camp; the scan starts at record 1 | observed | 00451a90, 004539f0 | high | |
| CAMP-202 | Rebuilding is not once per day: resolving a defend node from the camp rebuilds within the camp. A candidate newly appended has its outcome reset to 0 | observed | 00451a90, 00514730 | high | **(R19-11)** |
| CAMP-203 | The reduction runs on a copy re-taken from the source at every attempt, up to ten attempts, steps a..g; slot mutations of a failed attempt persist. An attempt can only end empty through a step that does **not** restore its own working copy  -  step a's expiry does, the two random filters do not (CAMP-212) | observed | 00451b70 | high | **(R19-7, R34-6)** |
| CAMP-204 | After ten attempts the campaign reports non-fatally, restores the working list, and then  -  **on the primary list only**  -  re-runs step d and, when it draws one entry from two or more, resets the age of every other source slot; a working list of fewer than two entries is used as it stands and may be empty | observed | 00451b70, 00456e60, 00457a30 | high | **(R19-7, R23-6)**; it cannot guarantee a mission |
| CAMP-264 | Clearing the deferred list also sets every slot that was in it back to age 0  -  but the clear happens **only** in step d's multi-candidate case; with zero or one candidate the deferred list and its members' ages are untouched | observed | 004564e0, 00455f00 | high | **(R23-6, R29-9)** |
| CAMP-211 | Recency visits history newest to oldest, removing every candidate equal to the history entry, and stops once **fewer than two** candidates remain  -  a count of exactly two continues the walk (**R34-7**). Removing one recent mission from three candidates therefore leaves two and the walk continues; a recent mission survives only when it is the last candidate left. The history list is stored oldest-first and visited newest-first | observed | 00452c90 | high | **(R19-8, R23-7)**; revision 3's test B13 asserted the wrong outcome |
| CAMP-212 | Each random filter removes matching candidates over a copy and restores the copy when the list becomes empty; the draw is taken only for candidates that pass the kind and age tests, once each, in list order; a removed candidate also gets age 0 | observed | 00452040, 00452090, 00452810, 00452a50 | high | **(R19-7)** |
| CAMP-220 | The next mission is the selected one, else the single offer whose node does not need the camp, else the camp | observed | 004539f0 | high | |
| CAMP-221 | Before choosing, every offered defend candidate whose blazon requirement is 0 is recorded won | observed | 0054cda0 | medium | no retail defend node has a 0 requirement |
| CAMP-222 | The team is reset to the band only on the unselected-next-mission path | observed | 004539f0, 00455650, 004559f0 | high | **(R19-11)** |
| CAMP-223 | The first two story nodes start without the camp because each is the only accessible node and has `needs_camp` 0 | inferred from CAMP-045, CAMP-060 and the retail data | 004539f0 | high | matches `campaign-flow.md` |
| CAMP-224 | The ordinary path from a mission ending to the next offer is: mission-end bookkeeping, the debriefing, the rebuild, the zero-blazon defend resolutions, the team reset, the choice of `current`, the append of the **newly chosen non-camp `current`** to `recent` (nothing on camp entry), the level load, and then in campaign mode the automatic backup and restart checkpoints and, in the camp, message 1001 followed by the workshop pass. No player-initiated save and no further engine message is involved; the terminal transition is not covered | observed | 004c6ef0, 004e3260, 004539f0, 0050f710, 0040a230 | high for the ordinary path; not traced for the terminal one | **(R46-8)**; revision 8 appended the finished mission and omitted the zero-blazon step |
| CAMP-230 | Selecting in the camp ages every other offered candidate by one and zeroes the picked one's age | observed | 004560d0 | high | |
| CAMP-231 | A candidate whose node's maximum age is 1000 or more only ages from 0 to 1; the over-age expiry comparison reads the picked candidate | observed | 004560d0 | high | preserved as observed |
| CAMP-240 | A win sets outcome 1 and the win byte; a loss sets outcome 2, the loss byte, and zeroes the blazon counter when the slot is the blazon node | observed | 00456540 | high | |
| CAMP-241 | A resolved defend blazon node arms a one-shot notification (code plus result) that the campaign-map screen reads and clears | observed | 00456540, 005280d0, 004577a0, 004577b0 | high | |
| CAMP-250 | Mission-end bookkeeping records the outcome and runs the hostile-actor pass and the revive pass whatever the outcome; only the counter and recruit work is gated on victory. A loss therefore can still clear the blazon counter | observed | 004e3260, 00456540 | high | **(R19-20, R23-9)** |
| CAMP-253 | The second end-of-mission pass visits the level's **player characters**, not an item list. For each whose `revive_flag` is non-zero, in order: the flag is cleared, **two** unidentified operations run on the actor, health is set to 50, and one further operation schedules additional actor activity. The three unidentified steps keep the pass's complete effect excluded | observed for the objects, the flag, the health and the ordering; unknown for the three operations | 004e3260, 0049f510, 004a05a0, 00464630 | high for the framing and order, unknown for the effects | **(R23-9, R29-4, R34-11)**; revision 5 put the health write before all three |
| CAMP-254 | The revive pass's last step gives the actor one further action and dispatches it immediately rather than queueing it behind the actor's current one; what the action does and the posture it leaves is not established | observed for the immediacy; unknown for the action | 004a05a0, 00464630 | high / unknown | **(R46-6)**; revision 8's construction details are withdrawn as internal machinery |
| CAMP-251 | Score gains 1000 for a won non-ambush mission, 70 per qualifying surviving hostile, 100 when a player character's experience level increases as a whole value, and 50 when a hostile dies from damage | observed | 004e3260, 0049fc80, 00485200, 004a5a00, 0047e9a0 | medium-high | **(R19-19)** |
| CAMP-252 | Only delta updates of money and score propagate into the level's statistics block; direct sets do not | observed | 00450b60, 004520e0, 004e3cc0 | high | **(R19-19)** |
| CAMP-260 | Step d: the blazon node is the first candidate of kind 1 or 5; other kind-1/5 candidates are removed; **tactical** candidates move to the deferred list when a blazon node exists and are removed otherwise; with exactly one candidate nothing is removed | observed | 00455f00, 0054cbf0, 0054cc20 | high | **(R19-9)**; revision 1 misidentified the deferred list |
| CAMP-261 | The four ways the blazon counter moves (3.6) | observed | 005795c0, 00514730, 00525040 | high | |
| CAMP-262 | The reported requirement is needed - held for the blazon node and needed for the current node | observed | 00455af0, 00579690 | high | |
| CAMP-263 | The men-to-blazons conversion reads the **selected** node and the length of **`team`**; a zero `men_per_blazon` takes the **fatal** path before dividing; otherwise it answers `men_per_blazon * min(team_size / men_per_blazon, (u16)(needed - held) - blazons)` with the `min` over 16-bit unsigned values and no clamp, so an exactly met requirement gives 0 and only an exceeded one underflows | observed | 00455a70, 005f8030 | high | **(R19-22, R23-2, R23-8)** |
| CAMP-270 | Progress is 100 when the node with level code `HI` is won, otherwise 95 when `DD` is won, otherwise won x 100 / total over nodes of kind other than 3 and 6, unsigned and without a zero-denominator guard | observed | 00456db0 | high | **(R19-21, R23-12)** |
| CAMP-273 | The two short-circuit codes are the level codes `HI` and `DD`, compared against the `code` field of a level-table record of `profile.cpf`; they are individual compatibility tokens and the `HI` case takes precedence | observed | 00456db0 | high | **(R23-12)**; naming them is required because "the last story mission" does not define behaviour for modified data |
| CAMP-271 | Play time accrues at save and transition checkpoints from an active interval, truncating the interval to whole seconds, discarding the remainder and clearing the start marker | observed | 0050e7b0, 0050e800, 00511340, 00512310 | high | **(R19-21, R23-14)** |
| CAMP-272 | One diagnostic display path adds unscaled milliseconds to the seconds value | observed | 0050e7d0 | high | |
| CAMP-280 | Difficulty is the profile's last serialized summary value, 0 easy / 1 medium / 2 hard, per `spec-ai-combat.md` AI-045; no campaign routine in the scope read branches on it | observed (deferred) | AI-045; 0055d1a0 | high | **(R19-20)**; table B is the ranged-weapon table (AI-020) |
| CAMP-290 | An empty recovery list skips the **coin flip** only; the new-character branch takes a draw of its own. The coin's polarity is: **even selects the new-character branch, odd selects recovery**. One added man therefore costs one draw from the recruit stream on an empty recovery list and two otherwise  -  name generation draws from a different stream and is not counted here | observed | 004524b0 | high | **(R19-23, R23-10, R29-11)** |
| CAMP-291 | The number of men added after a victory comes from the level and is truncated to an integer; the expression is absent from the export | observed for the truncation, unknown for the expression | 004e3260 | low | **(R19-20)** |
| CAMP-292 | Recovery adjusts experience, clears carried-item fields, sets the camp slot to 0xFFFF, writes health from an absent expression clamped to 100, moves the man to the band and sets the recruit flag | observed except the health and experience expressions | 00455bf0, 0055c390 | medium | **(R19-23)**; the absent expressions keep the recruit mechanism out of clearance |
| CAMP-295 | The `labels` list is the campaign's history of generated names. Each attempt costs two draws; the attempt succeeds when the result is absent from the history, and the name is then appended to it and used. At most ten attempts run; ten collisions end in a non-fatal report and a fixed fallback name that is **not** appended. This is the retry exit revisions 1 to 5 left open | observed | 0055bf00, 0055bdf0, 004565c0, 00456670 | high | **(R23-10, R34-9)** |
| CAMP-300 | Before the play loop starts in campaign mode: write the backup file, make the restart auto-save when the level is not the camp, and, when the level is the camp **and the loop is not being entered from the mission-selection state**, submit message 1001 and run the workshop pass. The second submission site  -  returning from an accepted selection of the current node  -  submits the message **without** a second workshop pass | observed | 0050f710 | medium-high | **(R41-5)**; revision 6 coupled camp start with both unconditionally. The campaign-mode test itself is folded (section 10) |
| CAMP-301 | Message 1001 is engine-originated, goes to the hub level only, and has **two** submission sites, whose conditions this document adopts from `spec-script-vm.md` VM-113 rather than restating: before the play loop starts unless it is entered from mission selection, and on returning from an accepted selection of the current node | observed | 0050f710, 00578d80; `spec-script-vm.md` VM-113 | high | **(R29-13)**; revision 4's "once, at camp start" is withdrawn |
| CAMP-302 | **Message 1000 is never sent in the retail build.** The only routine that submits an engine message has one caller, the game loop, and submits 1001 there; `spec-script-vm.md` VM-122 states the same restriction; and no script of the 39 retail files sends 1000. The camp script's handler for it is unreachable | observed | 00578d80 (single caller), 0050f710; VM-122; `docs/formats/sherwood-hub.md` section 5 | high | revisions 4 to 7 recorded the origin as unknown; there is none |
| CAMP-303 | The 1001 handler runs to completion **inside** the submission: the submitted element is a message with the level as target, and the pinned `spec-script-vm.md` VM-210 and VM-212 make such an element execute immediately rather than queue. The handler therefore finishes before the workshop pass and before the first tick | observed, by reference to the pinned dependency | 00578d80, 0058a940; VM-210, VM-212 | high | **(R43 scope)**; the exclusion is lifted |
| CAMP-310 | Native 165 appends the record without duplication and refreshes the HUD, then writes three unidentified actor fields, one of them to 2; it does not write the saved deployed flag, and the actor effects are excluded from clearance | observed for the list and HUD, unknown for the actor fields | 00579210, 00579280, 00455400, 004555f0 | high / unknown | **(R19-12, R23-15)** |
| CAMP-311 | Native 174 errors and answers 0 unless the current node's location is 8 | observed | 00579300 | high | |
| CAMP-312 | Native 170 requires every required character profile and every capability requirement to be met by some team member, stopping at the first failure | observed | 00453cd0, 00457480, 004574b0 | high | |
| CAMP-313 | A team member's camp slot survives only while the current node's location is 8 | observed | 004552f0 | medium | |
| CAMP-316 | Spawn assignment walks the mission's spawn points in file order; a point's **numeric selector** names a required character identity, which is matched to a team member's character profile. Remembered camp slots are applied **after** that, in the camp only. The camp's remaining points are then shuffled over one hundred rounds of two draws each. The remaining allocation weighs each point's capability requirements and prefers uniquely matching candidates before relaxing. Camp initialisation clears `team` | observed for steps 1 to 3 and the team clear; unknown for the remaining allocation's order, limits and result | 00453e60, 00454c00, 004552f0, 004559f0 | high / unknown | **(R46-4)**; revision 8's point-name comparison, two draws and "every remaining point is filled" are withdrawn |
| CAMP-317 | Native 173 reports a byte of the game object written by exactly one routine, which exactly two actions call; both actions accept a mission and close their dialog, one leaving the byte set and the other clearing it, so the value survives dialog closure. The byte is one input to a camp widget, which has another construction route as well. Its meaning, initial value and other transitions were not traced | observed for the writers and the persistence; unknown for the meaning | 005795b0, 0050ed10, 005358d0, 00535910, 0050ed40, 00528560 | high / unknown | **(R46-3)**; revision 8's "mission-selection screen open" is withdrawn |
| CAMP-314 | Native 249 counts HUD-selected characters, which is not the team | observed | 0057b5e0, 0057b5f0 | high | **(R19-13)** |
| CAMP-315 | A capability is satisfied by a three-entry or a four-entry capability array of the character profile, with three substitutions: 2 by 3 and 13 by 14 in the three-entry array, 25 by 26 in the four-entry array | observed | 004574b0 | high | **(R19-6)**; three individual facts |
| CAMP-320 | Native 199 sets a workshop's zone and capacity and back-links the location; native 200 appends a work spot with coordinates and a sector reference | observed | 005799d0, 00579a00, 00580330, 00580220, 00456a10 | high | |
| CAMP-321 | Victory gates only the output calculation for kinds 0..11; item placement, kind 12 and character placement run regardless, and each workshop's own turn ends by clearing and rebuilding the level's location membership | observed | 00456a40, 00580d20, 00581020, 005810c0, 00581180, 00580b70, 00580e80, 00580dc0, 004fc590, 004fc660 | high | **(R19-14, R34-5)** |
| CAMP-333a | The level's location membership is cleared and rebuilt **once per workshop**, after that workshop's per-kind work and character placement and before the next workshop, so thirteen times per pass. It is the source of the zone member lists assignment capture later walks. Neither step was traced | observed that the two steps run, when, and what they operate on; unknown for their results, member order and predicates | 00580d20, 004fc590, 004fc660 | high / unknown | **(R34-5, R41-3)**; revision 6 placed it once at the end of the pass |
| CAMP-322 | Placement draws down a temporary quantity and does not reduce `stock`; afterwards the capacity flag is recomputed as `5 x spots <= stock` | observed | 00580b70 | high | **(R19-15)** |
| CAMP-323 | Kinds 9 and 10 add their output to each assigned character's experience, slot 1 and slot 0; kind 9 additionally requires **capability id 1** in the character profile's three-entry capability array | observed | 005810c0, 0051d4e0, 0055bb00 | high | **(R19-16, R23-11)** |
| CAMP-324 | Kind 11 restores health to each assigned character; the cap compares the **16-bit signed** sum of health and amount against 100, so a sufficiently large amount makes that sum negative and the cap does not fire | observed | 00581180, 00484360 | high | **(R19-16, R23-11, R34-12)**; revision 5's unconditional "capped at 100" was too strong |
| CAMP-325 | Each workshop kind 0..8 produces and counts exactly one item kind, per interface table T2; kinds 9..12 have none | observed | 00580350, 00580430, 00580b70 | high | interpreted field: the item-kind id of a pick-up element |
| CAMP-327 | Each workshop kind 0..11 has one supervising character, per interface table T3; only kind 12 has none | observed | 005806c0 | high | **(R23-3)**; revision 3 wrongly listed kind 11 as having none |
| CAMP-326 | The capture pass recounts `stock` for kinds 0..8 by replacing it with the sum over active pick-up elements (nibble 3) whose item kind matches, plus, for the workshop whose item kind is 1, one per active element of nibble 7 whose sub-kind field is 5 | observed | 00456a70, 005804e0, 00580430 | high for the predicate, unknown for the element class it names | **(R19-15, R23-11)** |
| CAMP-331a | Assignment capture walks the **member list of the workshop's zone**, not the camp's player characters at large, and is skipped entirely when the workshop has no zone. A member is captured when its element-kind nibble is 0xC and it passes the per-kind test: kind 12 accepts nobody, kind 9 accepts only a character carrying capability id 1, every other kind accepts any player character | observed | 00580520, 00580940 | high | **(R23-11)** |
| CAMP-328 | Two placements run over two different lists: item placement walks the **work spots in registration order**, character placement walks the **assignments in assignment order**, skipping an assignment whose character has no actor and recording the workshop's kind on each placed character's actor | observed | 00580b70 (items), 00580dc0 (characters) | high | **(R23-11, R29-12)**; revision 4 gave both the work-spot order |
| CAMP-336 | Immediately after capturing a workshop the campaign pass **clears that workshop's zone association and empties its work-spot list**; capture replaces the assignment list whenever a zone exists, including a zone with no members, and leaves it alone only when there is no zone | observed | 00456a70, 005803c0, 00580520 | high | **(R29-12)**; an authoritative state transition revision 4 omitted |
| CAMP-334 | The level record's u16 that `profile.md` calls `unknown_f` is the mission's production yield: every camp workshop multiplies a day's output by the yield of the node in `previous` | observed | 00581020, 005810c0, 00581180 (instruction listings) | high | the field was missing from this document's own field table until now |
| CAMP-335 | A day's output is `trunc_toward_zero(P x sup x f32(capacity x S))`, with `P` = yield x assignments for kinds 0..8 and yield alone for kinds 9, 10 and 11; `S` is 0.001 for kinds 0..8 and 0.01 for kinds 9, 10 and 11; `sup` is 1.5 (kinds 0..8 and 11), 2.0 (kinds 9 and 10) with a supervisor and 1.0 without | observed | 00581020, 005810c0, 00581180 (instruction listings); the four constants read out of the image at 00679cc8, 00677578, 006774d8, 00677670, 00677d90 | high | settles open question 1 for kinds 0..11 |
| CAMP-339 | The scale factor is formed with a double-width constant and **narrowed to a 32-bit float** before the final multiply; the remaining factors are multiplied in the order P, sup, scale; the product is converted by the run-time helper, which truncates toward zero into a **signed 64-bit** integer, of which callers keep the low bits they need  -  kinds 0..8 and 9/10 the low 16, kind 11 the low 32. The effective precision and rounding mode of the multiplications themselves is **not** established | observed for the narrowing, the order, the 64-bit conversion and the masks; unknown for the multiplication mode | 00581020, 005810c0, 00581180 (instruction listings), 00642b7c | high / unknown | **(R34-3)** |
| CAMP-338 | Kind 12 has no victory gate, no supervisor and no arithmetic: it pairs each of the workshop's work spots, in registration order, with the next entry of the campaign's `camp_extras` list, creates one element per value and places it at that spot. The loop stops as soon as **either** list is exhausted, placing `min(spots, extras)` elements and **reporting nothing**; the defensive check inside the body cannot fire under that loop condition and would throw rather than continue | observed | 00580e80 (instruction listing, the loop head), 005cb6f0, 00581220, 004a71d0 | high | **(R34-4)**; revision 5's bounds-report reading is withdrawn |
| CAMP-337 | The list the campaign serializes after `team` and the workshop records holds raw 32-bit values, not references into `people`; it is what kind 12 consumes | observed | 00452f20, 00456aa0, 00580e80 | high | revision 4 called it `spare_refs` and read its elements as references |
| CAMP-330 | The campaign map lays out ten location slots, skipping 0 and 8, with one widget fewer for locations 1..3 | observed | 00527d20 | medium | |
| CAMP-331 | An empty offered list on the campaign map yields a text and a reported warning | observed | 00527d20 | high | |
| CAMP-332 | The status line carries money, score and the spared percentage from text resources 0xF3, 0xF4 and 0x40 | observed | 00528810 | high | |
| CAMP-333 | Native 256 compares a character profile's **first** string and the supervisor test its **second**; the two hero records differ in the first and agree in the second, so both appearances qualify as the supervisor of kinds 0, 1 and 9 | observed | 0057ba60, 005806c0; re-parse of the ten player-character records (ten distinct first strings, nine distinct second strings) | high | **(R23-4)**; revision 3's caveat is withdrawn, and this contradicts `profile.md`'s "sequence equal to the sprite for all 102 entries" |
| CAMP-340 | The two HUD counters are money and clovers, each with its own text resource | observed | 004c8140 | high | |
| CAMP-341 | The blazon refresh errors and does nothing without a blazon node | observed | 00514730 | high | |
| CAMP-350 | Each element gets a dot style code and a set of colour entries at creation; the count differs by class (one for a player character and for two further classes, five for a hostile human, one of which replaces the other four under a controller flag). The colour values are a palette and are excluded from this document | observed for the structure, excluded for the values | 0048f280, 004a5c30, 0046ffe0, 004b1eb0, 005ef320 | high for the structure | **(R23-17)** |
| CAMP-351 | Native 24's accepted codes are 0, 1, 100..102, 111, 200..202, 222, 300..302, 333, 500 and 666; 111, 222 and 333 require a human element; 0, 1, 500 and 666 set the style code and leave the colour entries untouched, so they do not restore a colour a previous call overrode; anything else is an error with no change | observed | 005788a0 | high | **(R19-24)**; the colours the other codes install are part of the excluded palette |
| CAMP-352 | Which colour entry an actor state uses, and whether an un-blipped element is drawn at all, is not established | unknown | 005788a0, 004a5c30 |  -  | **(R23-17)**; revision 3's "no grey value appears in the image" is withdrawn as unprovable from one helper's call sites |

---

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| save magic | the four bytes `GSHR` | compatibility token, marked | 0056ea70 | high |
| profile archive magic | the four bytes `FORP` | compatibility token, marked | 0055dea0 | high |
| profile archive version | 6 |  -  | 0055dea0 | high |
| archive version of the retail build | 48 |  -  | 0056ea70, retail files | high |
| version above which the backup campaign block appears | 38 |  -  | 00511340 | high |
| version above which the recent list appears | 29 |  -  | 00452f20 | high |
| version from which the town result appears | 41 |  -  | 00452f20 | high |
| version from which the town code appears | 48 |  -  | 00452f20 | high |
| version above which the recruit flag appears | 27 |  -  | 00452f20 | high |
| version at which a workshop assignment's u16 is discarded | 46 |  -  | 00580ab0 | high |
| version from which that u16 is stored | 47 |  -  | 00580ab0 | high |
| block tag length | 16 | bytes | 005e1b70 | high |
| diagnostic severity that terminates the process | any non-zero first argument |  -  | 005f8030, 00643f50 | high |
| campaign counters | 27 | i32 | 00452f20 | high |
| script campaign value window | counters 7..26 as script indices 0..19 |  -  | 00579430 | high |
| new-campaign money | 100 | currency | 00450ec0, retail file | high |
| character-profile index of the starting man | 1 | index | 00450ec0 | high |
| no-maximum sentinel in the money window | 200000 | currency | 0054c800 | high |
| team size limit when nothing is selected | 5 | men | 00579300 | high |
| team size limit after a failed chunk validation | 5 | men | 0056b8d0 | high |
| team size limit of a record whose mission file is missing or has no `SCOT` chunk | 0 | men | 0056b8d0 | high |
| capability flags per mission slot | 10 | flags at offset 22 of a `SCOT` record | 0056b8d0 | high |
| capability array sizes of a character profile | 3 and 4 | entries | 004574b0 | high |
| capability substitutions | 2 by 3, 13 by 14 (three-entry array); 25 by 26 (four-entry array) |  -  | 004574b0 | high |
| interface tables | T1 slot flag -> capability id, T2 workshop kind -> item kind, T3 workshop kind -> supervising identity | see 2.7 | 0056b8d0, 00580350, 005806c0 | high |
| workshops | 13 | records | 00456a40, retail file | high |
| items placed per work spot | 5 | items | 00580b70 | high |
| output scale factor, workshop kinds 0..8 | 0.001 |  -  | 00581020, the double at 00679cc8 | high |
| output scale factor, workshop kinds 9, 10, 11 | 0.01 |  -  | 005810c0, 00581180, the double at 00677578 | high |
| supervisor factor, workshop kinds 0..8 and 11 | 1.5 |  -  | 00581020, 00581180, the double at 006774d8 | high |
| supervisor factor, workshop kinds 9 and 10 | 2.0 |  -  | 005810c0, the double at 00677670 | high |
| supervisor factor when no supervisor is assigned | 1.0 |  -  | the double at 00677d90 | high |
| float-to-integer conversion | truncate toward zero into a signed 64-bit integer; callers keep the low bits |  -  | 00642b7c | high |
| capacity-flag rule | `5 x spots <= stock` |  -  | 00580b70 | high |
| experience carry | 100 points per level, level clamped at 100 |  -  | 0051d4e0 | high |
| health cap | 100 | points | 00484360, 00455bf0 | high |
| recent-mission memory | 3 | missions | 004539f0 | high |
| reduction attempts before forcing | 10 | attempts | 00451b70 | high |
| random filter modulus | 101 |  -  | 00452040, 00452090 | high |
| age-increment threshold above which a candidate only ages 0 -> 1 | 1000 | campaign days | 004560d0 | high |
| score for a won non-ambush mission | 1000 | points | 004e3260 | high |
| score per qualifying surviving hostile | 70 | points | 004e3260 | high |
| score for an experience-level increase | 100 | points | 0049fc80 | high |
| score for a hostile's death from damage | 50 | points | 004a5a00 | high |
| progress when the node with level code `HI` is won (takes precedence) | 100 | percent | 00456db0 | high |
| progress when the node with level code `DD` is won | 95 | percent | 00456db0 | high |
| health a revived character is set to | 50 | points | 004a05a0 | high |
| clover ceiling for one actor action | 9 | count | 004a78c0 | medium |
| name generation text id ranges | 100..121 and 122..143 | resource ids | 0055bf00 | high |
| named-hero name text ids | 0x90 + position, positions 0..6 | resource ids | 0055bf00 | high |
| campaign-map status line text ids | 0xF3, 0xF4, 0x40 | resource ids | 00528810 | high |
| HUD counter text ids | 0xF4 money, 0xF5 clover | resource ids | 004c8140 | high |
| auto-slot label text ids | 0xF6, 0xF7 | resource ids | 0050f1a0, 0050f2d0 | high |
| game-length label text id | 0x50 | resource id | 0054d550 | high |
| mini-map colour entries per element | 1 for a player character and for two further classes, 5 for a hostile human | entries | 0048f280, 004a5c30, 0046ffe0, 004b1eb0 | high |
| slot names | `Continue`, `Restart`, `QuickSave`, `ExQuickSave`, `Sherwood`; numbered slots `Savegame_%03i` | compatibility tokens, marked individually | 0056ef30, 0056f090, 0056f1f0, 0056f350, 0056f4b0, 0056fa30 | high |
| thumbnail name suffix | `_t` | compatibility token, marked | 0056e6b0, retail files | high |
| campaign backup file name | `Campaign.bck` in the installation root | compatibility token, marked | 00457560 | high |
| profile directory pattern | `Profile_%03i` | compatibility token, marked | 0055dea0 area | high |
| progress special-case level codes | `HI` and `DD`, compared against the `code` field of a level-table record | compatibility tokens, marked individually | 00456db0 | high |

---

## 6. Interfaces to the script VM

Only the natives this subsystem owns. Arity, coercion and the call protocol are `spec-script-vm.md`'s.

| Id | Args | Returns | Meaning and side effects | Failure |
|---|---|---|---|---|
| 163 | - | int | the length of `team` | 0 outside a campaign |
| 164 | `i` | handle | the element of `team[i]`; no bound check | - |
| 165 | `pc` | - | append to `team` if absent; refresh the HUD; write three unidentified actor fields, one to 2 (excluded, section 11) | script error, no change, for a non-player-character |
| 166 | `pc` | - | remove from `team`; refresh the HUD | as 165 |
| 170 | - | bool | the requirement test of 3.9 | reads through a null selection |
| 172 | - | int | the selected node's four-byte code, 0 when none | - |
| 173 | - | bool | one byte of the game object, not of the campaign; nothing in the scope read writes it and its transitions are excluded (section 11) | - |
| 174 | - | int | the selected node's `team_limit`, 5 when none is selected; a node whose mission file is missing or has no `SCOT` chunk answers 0 | script error and 0 unless the current node's location is 8 |
| 178 | `banner` | - | deactivate the banner; `blazons +=` its value; in an assault node win once `blazons >= blazons_needed`; refresh the HUD | error for a null or already inactive banner |
| 195 | `k` | int | counter `k + 7`, `k` in 0..19 | script error and 0 outside the range |
| 196 | `k, v` | - | write counter `k + 7` | as 195 |
| 199 | `k, loc, n` | - | workshop `k`: zone and capacity; back-link the location | no range check on `k` |
| 200 | `k, loc` | - | append a work spot to workshop `k` | - |
| 232 | `pc` | - | add to the band | error when not a player character or already present |
| 234 | - | bool | `blazons >=` the reported requirement | 0 outside a campaign |
| 236 | - | int | counter 1 | -1 with an error outside a campaign |
| 237 | `v` | - | write counter 1; a growing value plays a sound; refresh the HUD | as 236 |
| 239 | - | - | open the debriefing screen | - |
| 249 | - | int | the number of HUD-selected player characters | - |
| 250 | `i` | handle | the i-th HUD-selected character | error and null at or above the count |
| 256 | `pc` | int | the campaign identity of `pc`, matched from his character profile's designer name; the id assignment is `spec-script-vm.md`'s row for this native and is not restated here | -1 with an error otherwise |
| 261 | - | bool | the recruit flag | - |

**Engine-originated messages to the camp level.**

| Message | When | Meaning | Ordering |
|---|---|---|---|
| 1001 | to the hub level only, at two sites, on the conditions `spec-script-vm.md` VM-113 states: before the play loop starts unless it is entered from mission selection, and on returning from an accepted selection of the current node (CAMP-301) | "configure your production zones"; the script answers with natives 199 and 200, which **append** registrations, so the number of submissions is consequential | the handler runs to **completion inside the submission**, before the workshop pass and the first tick (CAMP-303) |
| 1000 | **never sent in the retail build** (CAMP-302): the engine originates only 1001 and no retail script sends 1000, so the camp's handler for it is unreachable | "send the team out"; the script would walk the HUD-selected characters to the exit | not applicable |

---

## 7. Acceptance tests

### 7.1 Format observations on this installation's fixtures

A1 is a property of the retail data file and is a genuine acceptance test. **A2 to A5 are observations of
mutable local fixtures**  -  a save, a backup and a profile archive that any play session rewrites  -  so they are
regression fixtures, not universal expectations, and they prove grammar, not semantics.

| Id | Input | Expected |
|---|---|---|
| A1 | `Configuration/profile.cpf` parsed with 2.2's grammar | consumed to the exact byte, all 30,441 of them; 63 level records; record 0 has kind 4 and location 8 |
| A2 | `Campaign.bck` (fixture) | `u32 48` + one campaign block, consumed exactly (2,717 bytes); counters all 0 but index 1 = 100; siege state -1; 63 slots whose node indices are 0..62 in order, all outcomes 0; the four slot references read (none, 21, none, none); one character record with health 100, profile index 1, experience levels 20 and 100; band = [0]; team = [0]; recovering empty; 13 workshop records with kinds 0..12 and empty assignment lists; recent = [21]; town result 0 |
| A3 | the two fixture saves | header `GSHR`, version 48, a level code, version 48; the 2,713 bytes at offset 16 equal the backup file minus its first four bytes; a second campaign block begins at 2,729 and differs from the first only in counter 6 and in the four discarded bytes of every slot; the level state begins at 5,442 |
| A4 | `Savegame/Profiles` (fixture) | `FORP`, version 6, one profile, consumed exactly (all 430 bytes); money 100, score 0, progress 0, play time 12 s, difficulty 1; two key sets of 29 codes; resolution 1024 x 768; two save slots |
| A5 | `Configuration/keyset1.cfg`, `keyset2.cfg` | 76 bytes each: a 16-byte tag, set id 2 and 3, 29 u16 codes |
| A6 | every mission file's `SCOT` chunk | each node's `team_limit` equals its mission file's `SCOT` record count; 12 of the 39 mission files carry non-zero capability flags, over 16 slots in total |

### 7.2 Behaviour tests (synthetic, no game data)

Review 23 finding 19 drove the additions below: entry-by-entry checks of the three interface tables (B58, B58a,
B59, B59a), both hero appearances (B59b), failed-attempt state persistence (B63), deferred-list clearing (B64),
the secondary list's forced path (B65), snapshot continuation (B60), sub-second saves (B61) and the supported
archive versions with their defaults (B62).

| Id | Input | Expected |
|---|---|---|
| B1 | `min_money` 5000, money 4999 | not accessible; the diagnostic names the minimum-money condition |
| B2 | `max_money` 200000, money 4 x 10^9 (unsigned) | accessible |
| B3 | an *after* entry whose slot has outcome 2 | accessible |
| B4 | a node listing itself in *until*, played | never accessible again |
| B5 | siege state -1, gate byte 0 = 1, all other gate bytes 0 | accessible |
| B6 | siege state 0, the same node | not accessible |
| B7 | three candidates sharing one location and one map, priorities 1, 2, 2 | only the priority-1 candidate survives; the others' ages are 0 |
| B8 | two candidates at the same location and priority | the original terminates; OpenSherwood records a diagnostic, retains both, and the departure is asserted |
| B9 | one obligatory and two ordinary candidates | only the obligatory candidate survives |
| B10 | one obligatory candidate alone | step b does not run; the candidate survives unchanged |
| B11 | a story candidate with `max_age` 6 and age 5, plus one other | the story candidate alone survives |
| B12 | a defend candidate with `max_age` 3, age 3, outcome 0, plus one other | it is removed, recorded lost, and its loss byte applied |
| B13 | three candidates, two of them in `recent` (stored oldest-first, visited newest-first) | removing the newest recent leaves **two**, which is not fewer than two, so the walk continues and removes the other recent one; one candidate remains. The stop condition is "fewer than two", checked after each history entry |
| B13a | two candidates, both in `recent` | the newest recent is removed, leaving one; the walk stops and the second recent candidate **survives** as the sole offer |
| B14 | one candidate that is in `recent` | it survives (step e needs more than one candidate) |
| B14a | `recent` holding three entries and a candidate list equal to them plus two others (five candidates) | the walk removes all three recent candidates, because the count never falls below two, and **two** candidates survive; the stop condition never fires. Revision 4's expectation of a single survivor was arithmetically impossible |
| B14b | four candidates of which three are recent | all three are removed and **one** survives: the check is "fewer than two", so a count of two continues the walk. Revision 5's expectation of two survivors here was wrong |
| B15 | a random filter that would remove every candidate | the list is restored unchanged, and exactly one draw was taken per candidate that passed the kind and age tests |
| B16 | a random filter with three eligible candidates and one ineligible | three draws, in list order; the ineligible candidate takes none |
| B17 | an empty source list | the reduction reports failure after ten attempts and yields an empty list; no mission is forced |
| B18 | a source list of one candidate that every step keeps | the forced path is not reached; that candidate is the offer |
| B19 | exactly one offer whose node has `needs_camp` 0 | it becomes current; the camp is not entered; `team` is reset to `band` |
| B20 | exactly one offer whose node has `needs_camp` 1 | node 0 becomes current |
| B21 | `selected` set | it becomes current and `team` is **not** reset |
| B22 | picking an offer in the camp with three offers, every node's `max_age` below 1000 and every candidate's age already non-zero | the picked candidate's age is 0 and it leaves the list; the other two aged by one. (With `max_age` at or above 1000 a candidate whose age is non-zero does not age at all, so the preconditions matter) |
| B23 | a won kind-0 mission with 3 hostile survivors (one qualifying) and 1 hostile dead | spared += 3, killed += 1, score += 1000 + 70 |
| B24 | the same mission lost, the slot being the blazon node, one player character with `revive_flag` set | the outcome is 2, blazons become 0, the revive pass still runs and that character's flag is cleared and his health becomes 50, and no counter from step 4 changes. No item collection is asserted |
| B25 | spared 3, killed 1 | the map's spared percentage is 75 |
| B26 | spared 0, killed 0 | the spared percentage is 0 |
| B27 | 20 qualifying nodes, 7 won, neither special code | progress 35 |
| B28 | 0 qualifying nodes | the original divides by zero; OpenSherwood answers 0 and records a diagnostic |
| B29 | the special defend code won | progress 95 |
| B30 | the special story code won | progress 100 |
| B31 | buying a blazon at price 1500, step 500, twice | money falls 1500 then 2000; blazons 2; price 2500 |
| B32 | an assault node needing 12, banners summing to 12 | won on the banner that crosses the threshold |
| B33 | a defend blazon node needing 3, blazons 3, resolved from the camp | blazons 0, the node won and out of `offered`, the notification armed with the node's code and result 1, and the offered list rebuilt |
| B34 | `men_per_blazon` 3, team 8, needed - held 2, blazons 0 | the convertible count is 6 |
| B35 | selected node with `men_per_blazon` 3, `blazons_needed - blazons_held` 2, blazons 5, team size 8 | the original's outstanding count underflows to 65533, `groups` is 2, the `min` is 2 and the low word of the answer is **6**; OpenSherwood clamps the outstanding count to 0, answers 0, and the departure is asserted |
| B35a | the same with blazons exactly 2 | outstanding is 0, the answer is 0 in both, and no underflow occurs |
| B35b | the same with team size 2 | `groups` is 0 and the answer is 0 whatever the outstanding count |
| B36 | selected node with `men_per_blazon` 0 | the original terminates before dividing; OpenSherwood records a diagnostic, answers 0 and performs no division, and the departure is asserted |
| B37 | four candidates: two of kind 5, one of kind 6, one of kind 0 | the first kind-5 candidate becomes the blazon node, the second kind-5 candidate is removed, the kind-6 candidate moves to `deferred`, the kind-0 candidate stays |
| B38 | the same set with no kind-1/5 candidate | the blazon node is null and the kind-6 candidate is removed rather than deferred |
| B39 | one candidate of kind 5 | it becomes the blazon node and is not removed |
| B40 | native 174 with the current node's location not 8 | 0 and a recorded script error |
| B41 | a node whose capability requirement is satisfied only by the substitution 2-by-3 | native 170 answers 1 |
| B42 | a node with a required character profile absent from the team | native 170 answers 0 and stops at that entry |
| B43 | a workshop pass with `previous` lost | **no output is computed for kinds 0..11**; kind 12's routine, item placement, character placement and the capacity-flag recomputation still run |
| B44 | a kind-0 workshop, stock 7, output 0 (no victory), two spots | stock stays 7; at most 5 items are placed per spot; the capacity flag becomes `10 <= 7` = false |
| B45 | a kind-9 workshop, two assigned characters, one lacking the required capability | only the other one's experience slot 1 changes |
| B46 | a kind-11 workshop, an assigned character at health 95, output 20 | his health becomes 100 |
| B47 | an experience slot at level 3, points 80, plus 150 | level 5, points 30 |
| B48 | an experience slot at level 100, points 0, plus 500 | level 100, points 500 |
| B49 | a save written and reloaded | the campaign compares equal field by field, the four discarded bytes of every slot ignored |
| B50 | a save whose backup block differs from its live block | after loading, the campaign equals the live block |
| B51 | a save with a bad magic and a bad version | the original accepts it; OpenSherwood refuses it (a recorded departure) |
| B52 | a save at archive version 46 | each workshop assignment's trailing u16 is read and the field becomes 0xFFFF |
| B53 | recruiting with an empty recovery list | the coin flip is skipped and **one** draw is taken from the recruit stream, for the new-character choice |
| B54 | recruiting with a non-empty recovery list and an even coin | two draws from the recruit stream: the coin, then the new-character choice; a man is generated, not recovered |
| B54a | the same with an odd coin | two draws from the recruit stream: the coin, then the choice of which man to recover; that man is recovered |
| B54b | a generated name whose first attempt collides with an entry of `labels` | two draws from the name stream per attempt; the second attempt's name is used and appended to `labels`; the recruit stream is untouched by name generation |
| B54c | ten consecutive collisions | twenty draws from the name stream, a non-fatal report, the fixed fallback name is used and `labels` is **not** appended to |
| B55 | an element whose colour entries have been set to synthetic values of the test's own choosing, then native 24 with a colour-installing code, then code 0 | the colour-installing code overwrites the entries; code 0 then changes the style code and leaves the overwritten entries in place. The test never names an original colour |
| B56 | native 24 with code 111 on a non-human element | an error and no change |
| B57 | native 24 with code 999 | an error and no change |
| B58 | for each of T1's ten flag positions in turn, a mission file with only that flag set on one slot | the node's requirement list holds exactly the capability id T1 gives for that position, once |
| B58a | a slot with all ten flags set, and two slots each with the same single flag | ten requirements in file order for the first; two identical requirements for the pair, because duplicates are not removed |
| B59 | for each of T2's nine workshop kinds, a camp holding one active pick-up of that kind's item with quantity 3 | the recount sets that workshop's stock to 3 and every other workshop's to 0 |
| B59a | for each of T3's twelve rows, an assignment list holding one character of the named identity | the supervisor lookup answers that character for the matching kind and nothing for kind 12 |
| B59b | workshop kinds 0, 1 and 9 with an assigned character built from the hero's **first** profile record, then from his **second** | the supervisor lookup answers him in both cases (CAMP-333) |
| B60 | **canonical continuation.** Run to a checkpoint boundary, snapshot, then either continue uninterrupted or restore the snapshot into a fresh engine; feed both the same canonical inputs for N frames | the two runs agree frame by frame on the canonical hash, on every RNG stream position, and on the observable consequences that depend on them  -  generated names, the offered list, and each workshop's output, stock and assignments. Serialized equality of the two snapshots is asserted as well, but it is not the test |
| B60a | a snapshot requested away from any checkpoint the program itself takes | **not refused**: the request creates a checkpoint, so the accrual step runs and then the state is serialised. Counter 6, the residue and the interval start in the snapshot are the accrued values, and a run restored from it continues equivalently to the uninterrupted run (B60). Revision 6 still said such a snapshot is refused, contradicting 8.4 |
| B60b | a suspension checkpoint (the in-game menu) entered and left between two continuing-save checkpoints | the interval closes on entry with an accrual, a fresh interval opens on leaving, and the time spent in the menu is **not** added to counter 6 |
| B61 | ten checkpoints in a row, each exactly **6 logic frames** after the previous one | each checkpoint adds 18 sixty-fourths to the residue; counter 6 grows by exactly **2** over the ten and the residue ends at exactly **52** sixty-fourths. No intermediate checkpoint loses a fraction, and two runs of the same input agree exactly |
| B61a | one checkpoint after 22 frames | the residue gains 66 sixty-fourths, counter 6 grows by 1 and the residue is left at exactly 2 sixty-fourths |
| B61b | a restore between two checkpoints | the restored run's counter 6, residue and interval start equal the uninterrupted run's at every later checkpoint |
| B62 | a campaign block at every documented archive version, 28 to 48, and at each boundary version 27, 28, 29, 30, 38, 39, 40, 41, 45, 46, 47, 48, 49 | 28 to 48 load with the defaults of 8.6 at each boundary; 27 and 49 are refused with a diagnostic, the latter because an undocumented future version is not decoded on speculation |
| B62a | a save whose outer and inner version words disagree | the inner archive version controls decoding; the mismatch is reported, and OpenSherwood refuses the file rather than decoding to a version it was not told |
| B62b | loading an older-version campaign block over an already populated campaign | every field the older version omits takes its 8.6 default rather than the value already in memory |
| B63 | two **story** candidates at different locations, each with `max_age` 2 and `age` 2, neither obligatory, neither recent, `prob` 100 | step a removes both for expiry and resets both ages to 0, so the first attempt ends **empty**. The second attempt starts from the source list and observes those resets: both candidates are now within their age limit and accessible again. Revision 5's version could not fail at all, because the random filter restores the list it empties |
| B64 | step d with **two or more** candidates and `deferred` already holding two slots | `deferred` is emptied and both former members' ages are 0 before the new deferral is computed |
| B64a | step d with exactly **one** candidate and `deferred` holding two slots | `deferred` is unchanged and both members keep their ages; only `blazon_node` is set |
| B64b | step d with **no** candidates and `deferred` holding two slots | `deferred` is unchanged, the ages are unchanged, and `blazon_node` becomes null |
| B65 | the forced path on the secondary list with two candidates | one is drawn, step d is **not** re-run, and no source slot's age is reset |
| B66 (**R43-2**) | kind 0, capacity 5, three assignments, `previous` won with yield 700 | output **10** without the supervisor and **15** with him; `stock` grows by that and `last_output` equals it. These two integers are **unconditional**: the reviewer evaluated the expression over every combination of 24-, 53- and 64-bit significand with each rounding direction and the result is 10 and 15 throughout, so this case does not depend on `ProductionPrecisionMode` |
| B66a | kind 0, capacity 5, yield 200, **one** assignment, no supervisor | the case that distinguishes the narrowing, and its expected integer depends on **both** the significand width and the rounding direction, which must be stated together: 53-bit with round-to-nearest or toward zero gives about 0.99999998 and output **0**; 53-bit rounding upward gives about 1.00000007078 and output **1**; 24-bit rounding toward zero gives about 0.99999994 and output **0**; 24-bit rounding to nearest gives 1.0 and output **1**. The helper's truncation setting applies only to the final conversion and does not fix the multiplications' direction. The test therefore asserts the 32-bit store and records the assumed width **and** direction; it becomes a fixed integer expectation only once `ProductionPrecisionMode` is settled |
| B67 | kind 9, capacity 170, yield 300, two assignments of which one lacks capability id 1 | the amount is `trunc(300 x sup x f32(1.7))`, with `sup` 2.0 or 1.0; only the qualifying character's experience slot 1 changes, and the assignment count does **not** scale the amount |
| B68 | kind 10, capacity 340, yield 200 | the amount is `trunc(200 x sup x f32(3.4))` and every assigned character's experience slot 0 grows by it |
| B69 | kind 11, capacity 20, yield 600, an assigned character at health 95 | the amount is `trunc(600 x sup x f32(0.2))` and his health becomes 100 |
| B70 | any kind 0..11 with `previous` lost | no output is computed and nothing the output feeds changes |
| B71 | kind 12 with three work spots and a `camp_extras` list of two values | exactly two elements are created and placed, at the first two spots, and the loop stops; **nothing is reported** |
| B71a | kind 12 with two work spots and a list of five values | exactly two elements are placed, at the two spots; the remaining three values are untouched |
| B72 | kind 12 with an empty `camp_extras` list | nothing is placed and **nothing is reported** |
| B73 | a kind 0..8 workshop with `stock` 60000 and an output whose truncated value is 70000 | `last_output` becomes the low 16 bits of the output (4464) and `stock` **adds** that masked value, wrapping in 16 bits to 64464  -  stock is added to, never replaced |
| B73a | the same with `stock` 60000 and a masked output of 10000 | `stock` wraps to 4464 |
| B69a | kind 11 with an assigned character at health 95 and an output large enough that the 16-bit signed sum is negative | the original's cap does not fire and the health setter receives the unclamped sum; OpenSherwood clamps to 100 and the departure is asserted (8.1) |
| B74 | a block tag that does not match | the original terminates; OpenSherwood accepts the file, records a diagnostic and the departure is asserted |
| B75 | two candidates both with a non-zero `obligatory` | the original terminates; OpenSherwood records a diagnostic, retains both, and the departure is asserted |
| B76 | three nodes: one whose mission file is absent, one whose file has no `SCOT` chunk, one whose chunk fails validation | team limits 0, 0 and 5; the first and third are reported, the second is not |
| B77 | a campaign in which both `HI` and `DD` are won | progress is 100, not 95 |
| B78 | a camp holding, for the workshop whose item kind is 1, one active element matching the exceptional predicate and one inactive one | the recount adds exactly 1 |
| B79 | capture of a workshop with a zone that has no members, and of a workshop with no zone at all | the first workshop's assignment list becomes empty, the second's is unchanged; after each capture both workshops have no zone and no work spots |

**Oracle procedures.** Against a recording from `C:\Users\przem\source\gamedata\robinhood_oracle`:

1. *Auto-save write points* (CAMP-131, CAMP-132): start a mission and confirm the restart slot and its thumbnail
   appear before the first frame; enter the camp and confirm the Sherwood slot appears.
2. *Play-time accrual* (CAMP-271): save, wait a measured interval, save again, and read **counter 6 out of the
   live campaign block of the second save**  -  an ordinary save updates that counter and does **not** run the
   profile-summary updater, so the profile's number is the wrong thing to look at. Expect the truncated whole
   seconds of the interval. Tolerance 1 s.
3. *Profile summary* (CAMP-100, CAMP-101): drive whatever makes the profile updater run, then compare the
   profile's stored score, money, progress and play time with the campaign counters. Which action triggers the
   updater was not established (section 10), so this procedure is a probe, not yet a test.

---

## 8. Implementation choices, snapshot and departure contract

Everything in sections 2, 3, 5 and 6 is the original's behaviour unless it appears below.

### 8.1 Deliberate deviations

1. *Block-tag reading.* The original terminates on a mismatch (CAMP-002). Our reader accepts any sixteen bytes
   and records a diagnostic. Departure.
2. *The four discarded bytes per slot.* We write zeros, for reproducible saves. Round-trip comparisons mask
   those four bytes per slot.
3. *Header validation.* We refuse any save whose magic is not `GSHR`, and refuse an archive version we do not
   support, instead of the original's behaviour where a wrong version can mask a wrong magic (CAMP-125).
4. *Progress with no qualifying node* divides by zero in the original; we answer 0 and record a diagnostic
   (B28).
5. *The blazon conversion* has no clamp in the original and its subtraction underflows; we clamp the outstanding
   requirement to 0 and record a departure (B35).
6. *The fatal conditions.* Four conditions end the original's process (CAMP-005): a block-tag mismatch, two
   obligatory candidates, two candidates with equal location and priority, and a zero `men_per_blazon`. Our
   engine records a diagnostic and continues, with the continuation the original's code would have taken had it
   returned. This is a **departure**, not a reproduction, and revision 3's description of those sites as normal
   continuation was wrong. None of the four is reachable from unmodified retail data; all four are reachable
   from modified data, which is why the continuation is specified at all.
7. *Kind 11's health cap* does not fire in the original when the 16-bit signed sum of health and amount turns
   negative (CAMP-324). We clamp to 100 unconditionally and record a departure (B69a).
8. *The unit mix in the live play-time display* (CAMP-272) is not reproduced; we show seconds.
9. *Randomness.* The original draws from the C run-time generator. We use **four** named seeded streams,
   consumed in exactly the orders section 3 gives, and do not claim to match the original's sequence:
   `campaign.offer` (the two random filters, one draw per eligible candidate per pass), `campaign.force` (the
   forced-mission draw), `campaign.recruit` (the coin flip when the recovery list is non-empty, the choice of a
   man to recover, and the new-character choice  -  CAMP-290) and `campaign.name` (two draws per name attempt, at
   most ten attempts  -  CAMP-295).

### 8.1a Saved games: an OpenSherwood decision

Three of the remaining exclusions - resuming an imported original save, reconstructing a campaign from an
extracted one, and writing files the original can load - are **not on the path to the project's goal**, and the
maintainer's decision is that they need not be. Playing the original campaign from the player's own data needs
the *data*: `profile.cpf`, the mission files, the scripts and the texts. It does not need the original's save
format, because OpenSherwood writes its **own** save format, which its own reader restores exactly (8.5, B60).
The original's save files are supported for **extraction only** - reading a campaign state out of them as input
to inspection or conversion, and nothing more. Extraction does **not** let a player carry progress in: 8.3
excludes both resuming such a file and seeding a playable campaign from what was extracted, and this decision
does not change that or complete any authoritative-state contract. This is an OpenSherwood
decision, not a fact about the original, and it is the reason those three exclusions may stand indefinitely
without blocking the milestone.

### 8.9 Gameplay integration gates

Three excluded rules affect ordinary play rather than presentation, so an implementation may not simply omit
them and ship. Each is either read to a claim or passed through the gate below; the gates are the maintainer's
to approve.

**Gate 1 - the per-workshop location-membership rebuild** (`LocationMembershipRebuild`, CAMP-333a). The rebuild
decides which men the next day's assignment capture finds inside each workshop zone, hence the whole camp
economy. *To ship without reading it:* record a camp day with the original from
`C:\Users\przem\source\gamedata\robinhood_oracle` with a known band, note for each workshop which men the
camp script's own work helpers pick up on the following day (the helpers test zone membership through natives
204 and 205, so the recording shows the membership indirectly), and show that the engine's own membership -
recomputed from actor positions at the same point of the pass - selects the same men for every workshop over
two consecutive camp days. If any workshop differs, the rule must be read (004fc590, 004fc660) before shipping.

**Gate 2 - the capture pass's scheduling** (`CaptureScheduling`, CAMP-326, CAMP-331a, CAMP-336). The pass both
rebuilds assignments and tears the zone and work spots down, so where it runs decides whether a day's
assignments survive into the next day. *To ship without reading it:* demonstrate on the same recording that
entering the camp twice with one mission between leaves each man at the spot he had, and that the second day's
stock growth equals one day's output for every workshop - not zero and not double. A deviation is acceptable if
the lead approves it explicitly and it is recorded in 8.1: the natural candidate is "capture runs once, when the
camp level is left".

**Gate 3 - the production precision mode** (`ProductionPrecisionMode`, CAMP-339). *Proposed OpenSherwood
decision, for the maintainer to approve:* compute `capacity x S` as an IEEE binary64 product with
round-to-nearest, store it to binary32 with round-to-nearest, perform the remaining multiplications in binary64
with round-to-nearest, and truncate the result toward zero - that is, exactly what an ordinary compiled `double`
product followed by a `float` store gives. Under that decision every output is fixed, B66 stays 10 and 15, and
**B66a is 0**. Taking the decision converts `ProductionPrecisionMode` from an unknown into a recorded deviation
in 8.1; leaving it open keeps every output except the mode-independent ones unusable.

### 8.2 The tag donor

Writing a file the original can load needs the exact sixteen tag bytes per record type (2.6), and those bytes
are content of the executable, so they are never committed, logged or printed. OpenSherwood obtains them from
the player's own installation, under rules tight enough that a wrong tag is never mistaken for a right one:

- **Donor eligibility.** A donor is a file that parses **completely** under the grammar of 2.6  -  every field
  consumed, nothing left over  -  and that carries no OpenSherwood marker. A file OpenSherwood wrote is never a
  donor, whatever it parses as. A file that parses only because our reader is permissive (2.6) is not thereby a
  valid source: permissive parsing tells us nothing about a tag's correctness.
- **Grammar-based extraction.** Tags are taken only from the byte positions the grammar fixes for each record
  type, never by scanning for sixteen-byte patterns.
- **Build association.** Each extracted tag is stored against the SHA-256 of the executable the donor belongs
  to, and is offered only when writing for that same build. Tags from one build are never used for another.
- **Conflict handling.** If two eligible donors give different bytes for one record type, neither is used and no
  original-compatible export is offered for that record type; the conflict is reported.
- **When a donor is missing or untrusted** the engine does not claim original compatibility at all. It writes
  its own file with its own marker, which its own reader accepts, and tells the user which record type had no
  donor.
- **What such an export may claim.** At most the campaign prefix and the profile archive  -  never a complete
  saved game, whose level payload this document does not specify (8.6). The claim is only made after a
  donor-backed export has actually been loaded by the original and the result checked.
- It is a design of ours rather than a fact about the original, so producing original-readable files stays
  excluded from clearance until the mechanism is reviewed (section 11, `OriginalReadableWrites`). Revision 4's
  "a stock installation carries every tag this subsystem needs" is withdrawn: it rested on mutable local
  fixtures and is not established.

### 8.3 Ownership and reconstruction of the camp's zone and spot state

The workshop **zone** and the ordered **work spots** (2.4) decide later behaviour but are not in the original's
file. They are owned by the engine and are authoritative:

- In a campaign the engine runs, they are produced by the camp script's answer to message 1001 (3.9) and are
  cleared again by the capture pass (3.11, CAMP-336); both transitions are inside the snapshot, so a snapshot
  taken anywhere restores them directly, with no script re-run.
- An **imported original save** carries neither, and revision 4's answer  -  re-enter the camp, "which is the same
  path the original itself takes"  -  was **wrong**. The original does not re-enter: it restores the level payload
  and distinguishes loading from entering. Re-entering with a won `previous` would run the workshop pass a
  second time over a campaign block that already has the day's production, training and healing in it, applying
  them twice.
- Therefore **playable restoration of an imported original save is excluded from clearance** (section 11,
  `OriginalSaveResume`). What this document supports for such a file is **extraction**: reading the campaign
  state of 2.1 out of it, for inspection or conversion. Seeding a *new* campaign from an extracted state is a
  further step that this document does not authorise either, because it would need the same unresolved camp
  reconstruction: an extracted state may name a camp node, workshops with assignments, and a `previous` whose
  outcome is a win, and nothing here says what a fresh engine should do with that. Resuming or restarting from
  such a file needs the level payload, the camp ordering of CAMP-303 and the spawn contract of CAMP-316, none of
  which is settled.
- Restore equivalence *within* our own engine is a different matter and is tested by B60; the lossy import is
  tested separately and is not expected to round-trip.

### 8.4 Time (review 34 finding 2, review 41 finding 1)

Counter 6 is hashed state, so it must not depend on a wall clock. The original reads the system millisecond
counter at checkpoints and truncates each interval to whole seconds, discarding the remainder (CAMP-271).
OpenSherwood keeps the same shape with a deterministic source:

- the authoritative time source is the **logic frame** of ADR-0010: 64 frames are exactly 3 seconds, so one
  frame is exactly **3 sixty-fourths of a second**;
- the residue is therefore counted in **sixty-fourths of a second**, an integer, which represents every
  reachable remainder exactly  -  22 frames are 66 sixty-fourths, one whole second plus a residue of exactly 2;
- the campaign keeps three fields in the snapshot: `playtime_seconds` (counter 6), `playtime_residue_64ths` (an
  integer in 0..63) and `interval_start_frame` (a frame number), together with `interval_active`;
- **initialisation.** A new campaign has `playtime_seconds := 0`, `playtime_residue_64ths := 0`,
  `interval_start_frame := 0` and `interval_active := false`. `interval_start_frame` is initialised explicitly
  because it is hashed even while the interval is inactive, so leaving it undefined would make two equivalent
  campaigns hash differently. An interval opens when a level begins or resumes: `interval_start_frame :=
  current_frame`, `interval_active := true`. Opening an interval that is already open does nothing;
- **the accrual step**, run at every checkpoint the original takes, and only when an interval is open:
  `r := playtime_residue_64ths + 3 * (current_frame - interval_start_frame)`;
  `playtime_seconds += r / 64`; `playtime_residue_64ths := r mod 64`;
  `interval_start_frame := current_frame`. The interval stays open;
- **an interval closes** with an accrual step first, then `interval_active := false`. OpenSherwood keeps all
  three checkpoint kinds of 3.7: a *transition* checkpoint accrues and then opens or closes the interval; a
  *suspension* checkpoint accrues and closes it, and the matching resumption opens a fresh one at the frame play
  resumes; a *continuing-save* checkpoint accrues and leaves the interval open. Suspension is what the in-game
  menu does, and it is why the interval must be able to close without the level ending;
- **a snapshot request creates a checkpoint.** Taking a snapshot performs the accrual step first and then
  serialises, so a snapshot is always written at a checkpoint boundary and nothing elapsed is ever pending in
  one. Revision 5 said both that snapshots are only *taken at* a boundary and that one away from a boundary is
  *refused*; the rule is the first, and nothing is refused (B60a). Restoring reinstates all four fields and,
  when `interval_active` is true, sets `interval_start_frame := current_frame`, which is correct precisely
  because the accrual before the write left nothing outstanding.

Keeping the residue is a deviation from the original, which truncates each interval and discards the remainder;
it is what makes repeated short checkpoints neither lose time nor desynchronise two runs (B61, B61a, B61b).

### 8.5 Snapshot contract (review 23 finding 13, review 34 finding 2)

Authoritative, and therefore in `snapshot()` and under the canonical hash:

- the 27 counters, including counter 6, plus the other three time fields of 8.4  -  `playtime_residue_64ths`,
  `interval_start_frame` and `interval_active`;
- `siege_state`, `recruit_flag`, the four slot references;
- every slot's node, age, blazon price and outcome;
- `people` in order with every field of 2.3;
- `band`, `recovering`, `team`, `offered`, `deferred`, `recent` and `camp_extras` as ordered lists  - 
  `camp_extras` because the file preserves it and workshop kind 12 consumes it in order;
- `labels` in order, because it decides which generated names are still available (CAMP-295);
- the 13 workshop records with kind, capacity, stock, last output, the capacity flag `full` and `assignments` in
  order, **and** each workshop's zone and its ordered work spots (8.3);
- `town_result` and `town_code`;
- the identity of the current player profile, its `difficulty` (behaviour-relevant per `spec-ai-combat.md`
  AI-045) and its save-slot list;
- the **menu-choice state** native 173 reports (CAMP-317). It is a byte of the game object rather than of the
  campaign, but it survives the dialog that writes it and the camp script gates its idle work on it, so it is
  authoritative session state and belongs in the snapshot; if a sibling specification claims the game object,
  that contract owns it instead and this one defers;
- the positions of the four named RNG streams.

Not authoritative: the reduction's working copy; the level's mission-statistics block while no mission runs; and
the values that are recomputed on demand rather than stored  -  progress, the spared percentage and the reported
blazon requirement. Revision 3 listed the capacity flag both as authoritative and, under the name `full`, as
derived; it is one stored field and it is authoritative.

Because the zone and work-spot state has two transitions inside the engine  -  installed by message 1001's
handler, cleared by the capture pass (CAMP-336)  -  the snapshot must carry whichever side of those transitions
the campaign is on, not merely a rebuilt copy.

### 8.6 Departure contract

A ruleset bump is required by a change to: the availability test (3.2); the reduction order, its retry and
restore behaviour, or the RNG consumption order (3.3); the choice and aging rules (3.4); the outcome,
mission-end and revive rules (3.5); the blazon rules (3.6); the score, progress and spared formulas (3.7); the
time rule (8.4); the recruit and name rules (3.8); the deployment requirement test (3.9); or any part of the
workshop pass (3.11). A change to the file grammars of 2.6 is a format-version bump and must keep reading every
archive version listed in section 5.

Supported archive versions and their defaults are part of the contract. The reader accepts versions **28 to 48
inclusive**  -  the range this document documents  -  and refuses anything outside it with a diagnostic, including
versions above 48: an undocumented future format is not decoded on speculation (revision 4's "28 and above" is
withdrawn). Within the range: below 30 there is no recent list (it restores empty), below 41 no town result (0),
below 48 no town code (none), below 39 no backup block, at 46 the workshop assignment's trailing word is read and
discarded and the field becomes 0xFFFF, below 46 there is no such word and the field becomes 0xFFFF, and at 28
and 29 the recruit flag is present (it appears above version 27).

A saved game carries the archive version **twice** (2.6.3). The original decodes by the **inner** word, the one
that follows the level code; the outer word is only compared. When the two disagree OpenSherwood reports the
mismatch and refuses the file rather than decoding to a version it was not told (B62a).

**Campaign prefix versus full save interoperability.** This document specifies the header and the campaign block
or blocks of a save, and nothing after them: the level and actor payload is another subsystem's. An importer
built from this document therefore supports **extraction only**  -  reading the campaign state of 2.1 out of such
a file. It does **not** support resuming one, whatever the file's level code: resuming a camp save by
re-entering the camp would apply a camp day twice (8.3) and resuming a mission save needs the level payload.
Revision 5's permission to resume a camp save by re-entering is withdrawn (**R34-1**); it contradicted 8.3 and
`OriginalSaveResume`.

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
   flags through interface table T1, matched against the character profile's capability arrays with three
   substitutions, plus a per-node team size limit equal to the mission's slot count. All of it is specified
   here.
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
8. **Message 1001 is engine-originated**, at the two sites VM-113 names, and its handler completes inside the
   submission, so the engine's camp never configures its 13 production zones. Message 1000 is never sent at all,
   so no send path is missing.
9. **The workshop pass is absent,** all of it: the per-day output of every kind, item placement of the
   workshop's own item kind (T2), character placement, the capacity flag, the stock recount, the experience and
   health effects of kinds 9 to 11, and kind 12's placement queue. The output *expression* is now specified
   (3.11); its exact integers stay conditional on `ProductionPrecisionMode`, and the per-workshop membership
   rebuild and the pass's scheduling stay excluded, so this gap is not closed by implementing the formula
   alone.
10. **Score, progress and the spared percentage are unlike anything in the engine.** Progress excludes ambush and
    tactical nodes, includes placeholders, and two level codes short-circuit to 95 and 100. The spared percentage
    is the share of **enemies left alive**, from two cumulative counters over hostile actors  -  not a count of the
    player's men, which is what a naive implementation would build.

Two further gaps: `profiles.json` is not `Savegame/Profiles` (different container, no key or graphic
configuration, no slot list, no play time, spared percentage, blazons, siege state or difficulty), so a profile
the engine writes cannot round-trip through the original; and the engine has no auto-save, whereas the original
writes a restart slot at every mission start, a Sherwood slot at every camp entry, and mirrors every save into
the continue slot.

---

## 10. Open questions

1. **The effective precision and rounding mode of the production multiplications** (CAMP-339). The listings of
   00581020, 005810c0, 00581180, 005806c0 and 00580e80 settled the operands, the factor order, the binary32
   narrowing, the truncating conversion and the retained bits (3.11), but not the mode the multiplications run
   in, which the start-up code sets. Until it is read, only outputs shown to be mode-independent (B66) are
   fixed. A proposed OpenSherwood decision that removes the dependency is in 8.9; question 12 covers the
   remaining kind-12 gap.
2. **The recruit arithmetic** (CAMP-290, CAMP-291, CAMP-292): the expression that turns the new-character draw
   into a choice, the expression that produces the number of men a victory adds, and the health and experience
   values a recovered man is given. All three are absent from the export; each has an observable contract in
   3.8. A listing of 004e3260 and 00455bf0 in the form the production routines were given would close them.
3. **When the camp-capture pass runs** (CAMP-326, CAMP-331a, CAMP-336): 00456a70 has no direct caller in the
   export, and the pass both rebuilds assignments and tears down the zone and work spots.
4. **How the clover counter becomes non-zero** (CAMP-016). Follow 004a78c0's callers and the folded
   campaign-counter call in 0050b640.
5. **The three actor fields native 165 writes** (CAMP-310), one of them set to 2.
6. **The order of spawn assignment's relaxation passes and its two draws** (CAMP-316); the rest of the routine
   is settled.
7. **The mini-map's state-dependent rendering** (CAMP-352): which colour entry an actor state uses and whether
   an un-blipped element is drawn.
8. **Two operations of the revive pass** (CAMP-253), between the flag and the health; the sequence that follows
    is settled (CAMP-254). Excluded as `RevivePassEffects`.
9. **The actor flag that also earns the 70-point bonus** (CAMP-014); the selecting predicate and the class id
    are settled. Also the two virtual tests behind the 50-point event (004a5a00).
10. **The stored spared percentage** (CAMP-101): the floating-point expression behind it.
11. **The mapping from the new-character draw to a profile choice** (CAMP-291): the draw is taken, the
    candidate list is built from the non-hero profiles, and the expression that turns one into the other is
    absent from the export.
12. **What fills `camp_extras`, and which element class each of its values becomes** (CAMP-338): the consumer is
    settled, the producer is not, and the element constructor kind 12 calls was not read.
13. **When the profile summary updates** (CAMP-102): three call sites in the game loop, triggers not traced.
14. **Smaller items:** the two unused level-record u16 (`m0`, `m1`); the two tests
    that exclude the auto-slots from continue-mirroring (CAMP-133); the campaign-mode test at the head of the
    game loop, which the export renders as a test on the siege byte and which is more plausibly "a campaign
    exists" (CAMP-300); and what the original does when a save file cannot be created.

---

## 11. Excluded from clearance (review 34 finding 13, review 41 finding 6)

The following are **not** cleared for implementation by this revision. Each maps to an `Assumption` variant
(ADR-0008) that must stay in the registry until an amendment settles it. Revision 3's blanket sentence
"everything else in sections 2 to 8 is offered for clearance" is withdrawn: the list below is what that sentence
wrongly swept in.

| Area | Why | Assumption |
|---|---|---|
| **What fills `camp_extras`** and which element class workshop kind 12 creates from each of its values | Open question 12. Kind 12's *behaviour* is specified (3.11), including the empty-list case, and may be implemented; the list is empty in the inspected fixtures, which says nothing about whether a stock campaign keeps it empty (**R41-4**) | `CampExtrasSource` |
| **The per-workshop location-membership rebuild** (**R43-3**) | CAMP-333a: the two steps run after each workshop's own turn and what they operate on is known; the membership they produce, its order and their predicates are not, and that membership is what assignment capture reads | `LocationMembershipRebuild` |
| **The effective precision and rounding mode of the production multiplications** | CAMP-339: the single-precision narrowing, the factor order, the 64-bit truncating conversion and the masks are verified; the mode the multiplications themselves run in is set by start-up code this analysis did not read | `ProductionPrecisionMode` |
| **Kind 11's health cap outside its ordinary domain** | CAMP-324: the cap is decided on a 16-bit signed sum, so a large enough output bypasses it and the setter receives the unclamped value. OpenSherwood clamps instead (8.1), which is a deviation; what the original then does was not traced | `HealthCapOverflow` |
| **The revive pass's effect on the actor** | CAMP-253, CAMP-254: the flag, the health, the ordering and the fact that a further action is dispatched immediately are settled; the two operations before the health write and what the dispatched action does are not | `RevivePassEffects` |
| **When the camp-capture pass runs** | CAMP-326, CAMP-331a, CAMP-336: the pass rebuilds assignments *and* tears down the zone and work spots, so its scheduling changes later behaviour and is not a free choice | `CaptureScheduling` |
| **The allocation half of spawn assignment** | CAMP-316: required placement by numeric selector, the camp-slot pass, the shuffle and the team clear are settled; how the remaining points are filled - the capability weighing, the preference for uniquely matching candidates, its limits and its result - is not, so neither "every named point that cannot be filled stays empty" nor "every remaining point is filled" may be assumed | `SpawnAssignment` |
| **Recruitment's arithmetic** | Open question 2 and question 11: how many men a victory adds, which profile the new-character draw selects, and the health and experience a recovery writes. No default may stand in for any of them (section 12) | `RecruitArithmetic` |
| **Native 173's meaning** | CAMP-317: its writers and its persistence across the dialog are settled; what the state means for play, its initial value and its other transitions are not | `Native173State` |
| **The terminal campaign transition** | CAMP-224: what the campaign does when it ends was not traced, so an implementation cannot yet reach the end of the story | `TerminalTransition` |
| **When the profile summary updates** | CAMP-102: the updater is called from three places in the game loop and what makes each call happen was not traced, so when score, money, progress and cumulative profile play time move is unknown | `ProfileUpdateScheduling` |
| **Resuming an imported original save** | 8.3: the original restores a level payload instead of re-entering, and re-entering would apply a camp day twice. Extraction of the campaign state from such a file is in scope; playing on from it is not | `OriginalSaveResume` |
| **Native 165's actor effects** | CAMP-310: three unidentified actor fields, one set to 2. Omitting them changes camp behaviour by an unknown amount | `Native165ActorEffects` |
| **The meaning of the actor flag that also earns the 70-point bonus** | CAMP-014: the selecting predicate and the class id are settled; one alternative flag on the actor is not named | `BonusActorFlag` |
| **The stored spared percentage** | CAMP-101: the campaign map's integer form is specified; the profile's floating-point form is absent and is not proved identical | `ProfileSparedExpression` |
| **The mini-map palette and its state mapping** | CAMP-350, CAMP-352: colour values are a palette, which ADR-0009 section 5 keeps refusing, and which entry a state uses is unknown. Native 24's codes, its human-family requirement and its override-preservation *are* specified | `MinimapPalette` |
| **Writing files the original can load** | The tag donor (8.2) is a design of ours, not a fact about the original, and needs its own review | `OriginalReadableWrites` |

Everything in sections 2 to 8 that is not named above, and the four interface tables of 2.7, are offered for
clearance **as individual rules**. They are expressly **not** offered as an implementable whole: with
recruitment, spawn allocation, native 173, the revive effect, the terminal transition and the three items of 8.9
outstanding, a campaign built from this document cannot yet be played from the first mission to the last. The `CampProductionOutput` exclusion of revision 4 is **lifted for the arithmetic**: the per-day
output of every workshop kind  -  its inputs, its factor order, its single-precision narrowing, its truncating
conversion, its retained bits and the addition and wrapping of stock  -  is settled in 3.11 from the program's own
instruction listings, item by item. The **exact integers** it produces are not: they stay conditional on
`ProductionPrecisionMode` below. It is **not** lifted
for the workshop pass as a whole: the per-workshop location-membership rebuild, the pass's scheduling and the
multiplications' precision mode stay excluded above, so revision 5's "the whole workshop pass ... is now in
scope" is withdrawn.

---

## 12. Implementer handoff requirements

A handoff is complete only when all of the following hold.

1. The implementer session is fresh: no inherited analyst context, notes, tool output or memory, and no access
   to `re/`, including through helper scripts (ADR-0009 section 2).
2. **In scope:** the file grammars of 2.6 for reading, at every archive version of 8.6, with the defaults of
   8.6; the three interface tables of 2.7; the campaign object and its counters; the availability test; the
   whole reduction including its retries, restores, the deferred clearing and the per-branch forced path; choice
   and aging; outcome, mission-end and revive bookkeeping; the blazon rules; the score, progress, spared and
   time rules of 3.7 with the time contract of 8.4; the recruit and name *structure* of 3.8; deployment
   membership of 3.9 including the capability half of native 170; the workshop pass's **arithmetic** and its
   settled effects  -  the output *expression* for every kind with its verified operands, order, narrowing,
   conversion and retained bits, the victory gates, item and character placement, the capacity flag, the stock
   recount with its addition and wrapping, assignment capture and its teardown, and kind 12's placement queue
    -  **but not** the exact integers the expression yields except where a case is shown to be
   precision-independent (B66), the pass's scheduling, or the per-workshop location-membership rebuild; the save slots and auto-saves as far as writing our own files goes; and the HUD counters.
   **Out of scope:** every row of section 11  -  in particular the revive pass's further effects, the capture
   pass's scheduling, the profile-summary update scheduling and resuming an imported original save. Revision 4
   listed "revive bookkeeping" as in scope; only its settled flag-and-health part is.
3. Camp zone and work-spot state is owned and snapshotted as 8.3 specifies, on whichever side of the
   install/teardown transitions the campaign is. An imported original save is **extracted**, not resumed; no
   script is re-run out of order to rebuild state, and no camp entry is synthesised to stand in for a load.
4. The four named RNG streams of 8.1 exist with the stated consumption order, including the new-character draw,
   and their positions are in the snapshot.
5. The snapshot and departure contracts of 8.5 and 8.6 are implemented and the canonical hash covers exactly the
   authoritative set  -  including `labels`, `camp_extras`, the capacity flag, each workshop's zone and work spots,
   the play-time residue and interval start, and the selected profile's difficulty.
6. No wall clock reaches hashed state (8.4).
7. The acceptance tests of section 7 exist, with A2 to A6 marked as fixture regressions rather than universal
   expectations and with a documented procedure for refreshing them when the fixtures change.
8. Each row of section 11 is an `Assumption` variant in the registry, and no winning path depends on one. In
   particular no default may be invented for recruitment's quantities (CAMP-291), and the three gates of 8.9
   must be passed or their deviations approved before the camp economy is shipped.
9. Save failure behaviour is defined by OpenSherwood, since the original's is unknown (open question 12):
   refuse and report, never partially apply a campaign block.
10. The distinction of 8.6 between campaign-prefix support and full save interoperability is reflected in what
    the engine claims to the user.

---

## 13. Identity and exposure

- **Analyst:** session `a00ebf7dd67504358` (Opus; revisions 1 to 3 on 2026-09-13, revisions 4 to 9 on
  2026-09-18), working in the git-ignored `re/` workspace. Exposure: full decompilation of the functions listed
  in section 0; the function inventory, the string cross-reference and the module map; **instruction listings**
  of the eight production routines (section 14), which show the machine operations directly; and the run-time
  helper at 00642b7c, read as instruction bytes to establish its conversion. The listings and the helper's bytes
  stay in `re/`.
- **Reviewer:** Codex `gpt-6-astra`, seven reviews of this document, all archived in the repository:
  - spec review 19 on revision 1, verdict *redo*, 29 findings  - 
    `docs/decisions/reviews/2026-09-13-codex-review-19-spec-campaign-camp-saves.md`;
  - spec re-review 23 on revision 3 (commit `2cb9c2a`), verdict *fix-then-clear*, 19 findings  - 
    `docs/decisions/reviews/2026-09-13-codex-review-23-spec-campaign-camp-saves.md`;
  - spec re-review 29 on revision 4 (commit `c2c578a`), verdict *fix-then-clear*, 16 findings  - 
    `docs/decisions/reviews/2026-09-18-codex-review-29-spec-campaign-camp-saves.md`;
  - spec re-review 34 on revision 5 (commit `50d52ec`), verdict *fix-then-clear*, 13 findings with a
    per-component clearance table  - 
    `docs/decisions/reviews/2026-09-18-codex-review-34-spec-campaign-camp-saves.md`.

  - spec re-review 41 on revision 6 (commit `3d21293`), verdict *fix-then-clear*, 6 findings  - 
    `docs/decisions/reviews/2026-09-18-codex-review-41-spec-campaign-camp-saves.md`;
  - spec re-review 43 on revision 7, verdict *fix-then-clear*, 3 findings with a per-component clearance table  - 
    `docs/decisions/reviews/2026-09-18-codex-review-43-spec-campaign-camp-saves.md`;
  - spec re-review 46 on revision 8 (commit `e5b56e6`), verdict *fix-then-clear*, 11 findings  - 
    `docs/decisions/reviews/2026-09-18-codex-review-46-spec-campaign-camp-saves.md`.

  Review 46's clearance table is the last reviewed statement of what may be built, and its verdict is that these
  components are **not** cleared as an implementable whole. Revision 9 answers its eleven findings, correcting or
  withdrawing the readings it showed to be wrong; section 11 lists the seventeen exclusions that remain and 8.9
  states the three gates.

  Review 29 cleared, within the limits its findings name, the campaign graph and offers, the campaign-prefix and
  profile grammar for reading, camp deployment's membership and capability checks with T1, T2 and the corrected
  T3, and the statistics identities and arithmetic.

  Review 23 cleared as facts the character-record framing, the four node references' order, the town-code
  representation, the conditional-backup and unconditional-live block framing, the version 46/47 assignment
  word, and T1's ten mappings with its substitutions and T2's nine mappings.
- **Reviewer exposure.** The reviewer is a *spec reviewer* and read `re/`; its exposure covers this subsystem's
  decompilation, the **instruction listings** of the production routines and the bytes of the run-time
  conversion helper (the same material section 14 records for the analyst), plus the read-only mission-file
  inspection behind CAMP-048 and the parsing checks behind section 7. It may not
  implement this subsystem either, and its output was corrections to this document only.
- **Wall (ADR-0009 section 2).** Sessions exposed for this subsystem: the analyst and the reviewer above.
  Neither may implement campaign flow, the Sherwood camp, camp production, saved games, the player profile and
  configuration files, the campaign-map screen or the mini-map.
- **Publication approval:** pending, maintainer, separate from factual approval.
- **Gate:** `python scripts/check_no_assets.py --paths docs/original/spec-campaign-camp-saves.md` run before
  saving this revision. The gate is a heuristic and does not clear the expression filter; section 11 records
  what the reviewer withheld, and section 2.7 records what the maintainer admitted.

---

## 14. Provenance

Ghidra project `re/ghidra/robinhood` (never committed); full-function decompilation under `re/out/decomp_all/`
exported by `scripts/ghidra/DecompileAll.java`; the function inventory and string cross-reference from
`scripts/ghidra/ExportInventory.java` and `ExportStrings.java`; the module map in `re/notes/modules.txt`, derived
from assertion strings. Byte-level checks used `scripts/ghidra/peek.py` and four throw-away parsers in
`re/notes/campaign/` (a campaign-block reader, a level-table reader, a profile-archive reader and a
character-profile string reader); those parsers are one-off derivations of the numbers in section 7 and are not
needed to check the claims  -  the acceptance tests re-derive them from the player's files.

The production arithmetic of 3.11 was read from **instruction listings** exported to `re/out/listing/` for
00581020, 005810c0, 00581180, 005806c0, 00580e80, 00580970, 00580ab0 and 005818c0, with a fresh decompilation in
`re/out/decomp_decompile/`. Those listings are the program's own instructions rather than a reconstruction, which
is why the floating-point operations the earlier export dropped are visible in them; the five double constants
they name were read out of the image with `scripts/ghidra/peek.py`. Neither listings nor constants leave `re/`:
what this document carries is the resulting rule.

Revision 5 re-read, for the disputed findings of review 29: the diagnostic's exit path once more (005f8030,
00643f50); the mission-file open failure (0056b8d0); the workshop teardown (005803c0) and the two placement
loops (00580b70, 00580dc0); the recruit branch polarity (004524b0); the deferred clearing's precondition
(00455f00, 004564e0); the two submission sites of the camp message and the three profile-updater call sites
(0050f710), reconciled against `spec-script-vm.md` VM-113; and the element serializer of the list this revision
renames `camp_extras` (00456aa0).

Revision 4 re-read, for the disputed findings of review 23: the diagnostic helper and its exit path (005f8030,
00643f50) and the block-tag check (005e1b70); the two fatal filter diagnostics (00452e70, 00452d20); the
supervisor lookup's full case list (005806c0) and native 256's compared field (0057ba60); the mission-file pass
(0056b8d0) and the level-record creation in 00566570; the deferred-list clearing (004564e0) and the forced path
(00451b70); the recency walk (00452c90); the blazon conversion (00455a70); the second end-of-mission pass and
its two helpers (004e3260, 0049f510, 004a05a0); the recruit branches (004524b0) and the name history (0055bf00,
0055bdf0, 004565c0, 00456670); the workshop helpers (005810c0, 0055bb00, 00580430, 00580520, 00580940,
00580b70, 00580dc0); and the progress routine (00456db0).

Files read on disk, read-only, under `C:/Users/przem/source/gamedata/robinhood`: `Campaign.bck`,
`DATA/Configuration/profile.cpf`, `DATA/Configuration/keyset1.cfg`, `keyset2.cfg`, `Data/Savegame/Profiles`,
`Data/Savegame/Profile_001/{Continue,Continue_t,Restart,Restart_t}`. The mission-file inspection behind CAMP-048
and A6 was the reviewer's, on the same installation.

Sibling specifications are pinned in the header: `spec-script-vm.md` revision 4 at `afdfaec` and
`spec-ai-combat.md` revision 2 at `e966b05`. `docs/decisions/ADR-0010-logic-frame.md` supplies the logic frame
this document's time rule counts in. The three interface tables of 2.7 are admitted by the clarification to
ADR-0009 section 5 (2026-09-13, the maintainer's decision after review 19); each row carries the address that
establishes it and the data-file field it interprets.

No oracle recording was made; the procedures that would need one are in section 7.

Function addresses appear inline in sections 2 to 6 and are collected in section 0. Instruction bytes,
disassembly, recovered symbols and the comprehensive address map stay in `re/`.

Build: GOG English edition, `Robin Hood.exe` SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`.

Documents to update once this revision is cleared: `docs/formats/savegame.md` (replace the stub);
`docs/formats/profile.md` (the three level-record grammar corrections, the field meanings, the key-set layout,
and the `sequence` claim that CAMP-333 contradicts); `docs/formats/rhm.md` (the `SCOT` record's ten capability
flag bytes at offset 22, interface table T1, and the slot count's role as the team size limit);
`docs/formats/sherwood-hub.md` (sections 5 and 6); `docs/original/campaign-flow.md` (the successor rule);
`docs/roadmap.md`; and the `Assumption` registry (section 11).
