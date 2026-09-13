# Campaign, Sherwood camp and saved games (behaviour specification)

Status: `draft` (analyst session 2026-09-13; not yet reviewed).
Build: GOG English edition, `Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`,
image base 0x00400000; every address below is a virtual address in that image. Analyst: this session
(2026-09-13, Claude analyst, `re/` workspace). Reviewer: **pending**. Publication approval: **pending**
(separate from factual approval).

This file describes what the original program does, in the analyst's own words, so that an implementer who has
never seen the program can build it. It contains no decompiler output, no transcribed pseudocode, none of the
binary's identifiers or strings, no tables copied from its data, no game text, and no prescribed internal
structure (ADR-0009, "expression filter"). It describes required results and orderings; the implementer chooses
the organisation.

Sibling specifications, read first and **not** duplicated here: `spec-script-vm.md` (opcodes, natives,
callback ordering, the won / lost flags, objectives, the level tick), `spec-navigation.md`, `spec-ai-combat.md`
(all profile stat fields are settled there — this file does not re-derive them). Existing observation-only
documents this one corrects or extends: `docs/formats/profile.md`, `docs/formats/savegame.md`,
`docs/formats/sherwood-hub.md`, `docs/original/campaign-flow.md`, `docs/original/ui-flow.md`.

---

## 0. Necessity record

- **Interoperability target.** The player's own files: `Configuration/profile.cpf` (the campaign graph lives in
  its level table), `Savegame/Profiles` (the player-profile archive, key and graphic configuration, the list of
  save slots), `Savegame/Profile_<nnn>/<name>` and `<name>_t` (the saved games and their thumbnails),
  `Campaign.bck` in the installation root, and the camp's own compiled script `Data/Levels/sherwood.scb`, whose
  natives 163-166, 170, 172-174, 195/196, 199/200, 215, 232, 236/237, 239, 249/250, 256, 261 only mean
  something once the campaign object behind them is specified. Without this the engine cannot load a retail
  save, cannot decide which mission is offered next, and traps on six natives the camp script reaches.
- **Information not otherwise available.** Four years of data observation established the *layout* of
  `profile.cpf` but not the meaning of any numeric field; `docs/formats/profile.md` marked the whole campaign
  graph "whether the executable applies exactly these rules is not verified" and mis-split three field groups.
  `docs/formats/savegame.md` was a `stub` guess ("mission list with status") derived from a hex dump.
  `docs/formats/sherwood-hub.md` section 5 had to guess 165/166/170/174/239/249 from call sites and recorded
  message 1000/1001 as "must come from the engine … the obvious candidates". Black-box play cannot separate the
  age counter from the money window from the ARES gate, because all three fire on the same campaign day; and no
  amount of observation recovers the dead bytes a save file must still contain.
- **Scope read.** About 150 functions: the campaign module (00450ec0-0045a940 plus its console report block at
  0045ad00 and 0045cda0), `RHMission` (0054c680-0054cda0), the profile manager's level-table reader
  (00564580, 00564e60, 00566570, 0056d190-0056d4c0), the player-profile archive and its configuration
  serializers (0055c3c0, 0055d1a0, 0055dea0, 0051ba00, 0051ec40, 005b2190, 005fc2a0), the save-game manager
  (0056ea20-0056ff10, 00511340, 0050f1a0-0050f3d0), the campaign hooks in the game loop and the level tick
  (0050e7d0, 0050e800, 0050ed40, 0050f710, 004e3220, 004e3260, 00514730), the camp production module
  (00580050-00581300, 00456a10, 00456a40), the campaign-map screen (00527d20, 005280d0, 00528810), the
  campaign natives (005791d0-0057bfe0), the minimap dot code (005788a0, 0046ffe0, 0048f280, 004a5c30,
  004b1eb0) and the archive primitives (005e0c90, 005e0ea0, 005e1b70, 005f52b0).
  **Stopping condition reached:** every byte of `Campaign.bck`, of `Savegame/Profiles` and of the
  campaign-and-profile part of a retail save is accounted for by a grammar that consumes those files exactly
  (checked, section 7), and every native the camp script calls has a defined effect. Deliberately *not* read:
  the in-mission part of a saved game (the level / actor / AI state written after the campaign block by the
  level serializer at 004c4570) beyond identifying where it starts; the presentation code of the camp and
  campaign-map screens beyond what decides *content*; the floating-point expression that scales one day of
  workshop output (section 9).
- **Analyst authorisation.** On behalf of the maintainer, on the maintainer's lawfully acquired copy.

---

## 1. Scope

This subsystem owns the state that survives a mission: the band of player characters, money, score, statistics,
the per-mission progress of 63 campaign nodes, the camp's workshops, and the files all of this is stored in.

**Inputs.** The level table of `profile.cpf` (63 records, read once at start-up; `docs/formats/profile.md`, with
the corrections of section 2.2 below). The outcome of a mission: the *won* / *lost* flags of the level tick
(`spec-script-vm.md` VM-103) and the per-mission statistics gathered while it ran. Player choices on the
campaign-map screen and in the camp. The player profile selected in the front end.

**Outputs.** Which mission is loaded next (a level code plus its `.rhm`), the set of player characters
instantiated in it and their health, the HUD counters, the campaign values and money the mission script reads
through natives 195/196 and 236/237, the offered-mission list the campaign map draws, and the contents of the
save files.

**Shared state.** One campaign object exists while a campaign is running (not in single-mission mode: natives
236/237 and 174 fail then, `spec-script-vm.md`). One player-profile object is current. The level object exposes
a per-mission statistics block that the campaign reads at mission end.

**Terminology used below** (ours, chosen independently of the program's):

| Term | Meaning |
|---|---|
| *node* | one record of the level table of `profile.cpf`; 63 of them, addressed by index 0..62 |
| *mission slot* | the campaign's mutable state for one node (age, blazon price, outcome) |
| *band* | the player characters the player owns |
| *team* | the subset of the band that goes on the next mission |
| *kind* | the node's mission kind: 0 story, 1 assault, 3 ambush, 4 camp, 5 defend, 6 tactical |
| *location* | the node's place: 1..3 the three forest crossings, 4..7 and 9 the five towns, 8 the camp |
| *blazon* | the counter the manual calls a coat of arms; the console calls it both blazon and amulet |
| *ARES state* | a single signed byte of campaign state that gates the defend / assault branch |
| *campaign day* | one pass through camp -> mission -> camp; the age counter advances once per day |

---

## 2. Data model

All integers are little-endian. "u16", "i32" etc. give width and signedness. Units are stated per field.

### 2.1 The campaign object

Three groups: 27 counters, the node state, and the people.

**Counters** — 27 signed 32-bit values, index 0..26, all zero on a new campaign except index 1
(CAMP-010, CAMP-011):

| # | Name (ours) | Unit | Meaning |
|---|---|---|---|
| 0 | `clovers` | count | the second HUD counter (the clover, `ui-flow.md` 9.3 row 1, icon `BTTN` 165). Read by the HUD (004c8140) and by one actor-action gate (004a78c0, which refuses the action once the value exceeds 9); **no code path in the image writes it through the counter accessors** (CAMP-016) |
| 1 | `money` | currency | the money the HUD shows and the availability test compares; the debug report calls it the ransom. Writing it plays a sound when it grows and refreshes the HUD |
| 2 | `score` | points | the profile screen's score |
| 3 | `blazons` | count | blazons held; see 3.6 |
| 4 | `returned` | men | cumulative count of team members who came back alive from a won mission |
| 5 | `lost` | men | cumulative count of team members who did not |
| 6 | `playtime` | seconds | accumulated play time |
| 7..26 | `script[0..19]` | free | the twenty values natives 195/196 expose to mission scripts as indices 0..19 |

The script window is a plain offset: native 195(k) returns counter k+7 and 196(k,v) writes it, for k in 0..19
(CAMP-012; the range check is on the shifted index, 00579430/00579470). Counter 1 is also natives 236/237.
Counter 3 is what native 178 adds a captured banner's value to and what native 234 compares.

**Single scalars.**

| Name (ours) | Width | Meaning |
|---|---|---|
| `ares` | i8 | the ARES state, −1 on a new campaign (CAMP-030) |
| `recruit_flag` | u8 | set when a man was moved back from the recovery list into the band; native 261 returns it; cleared at the start of the post-mission bookkeeping |
| `current` | node ref | the mission being played (the camp is a node like any other) |
| `previous` | node ref | the mission played before it |
| `selected` | node ref | the mission the player picked in the camp, or null |
| `blazon_node` | node ref | the defend / assault node the blazon counter currently belongs to, or null |
| `town_result` | i32 | 0, or 1 / 2 = the last defend node was won / lost, pending display |
| `town_node` | node ref (raw) | the level record of that node |

**The node state.** One *mission slot* per node, in level-table order, 63 of them (CAMP-020):

| Field | Width | Unit | Meaning |
|---|---|---|---|
| `node` | index | — | which level-table record this slot belongs to; the retail files have slot i pointing at node i |
| `age` | u16 | campaign days | how many days the slot has been in the offered list; reset to 0 whenever it leaves the list |
| `blazon_price` | u16 | currency | what one blazon costs for this node right now; initialised from the node's *first blazon price* field, raised by the node's *price step* after every purchase |
| `outcome` | u32 | — | 0 not played, 1 won, 2 lost |

**Lists.** All of these are ordered; the order is part of the save (CAMP-021).

| Name (ours) | Elements | Meaning |
|---|---|---|
| `people` | character-status records | every player character the campaign has ever owned; the other lists hold positions in this one |
| `band` | positions in `people` | the men the player owns now; its length is the "gang size" of the availability test |
| `recovering` | positions in `people` | men out of action; a man may be moved back into `band` later |
| `team` | positions in `people` | who goes on the next mission |
| `offered` | mission slots | the missions the campaign map offers |
| `pending` | mission slots | a second, parallel list of slots filtered the same way (section 3.3) |
| `workshops` | production records | exactly 13, one per workshop kind 0..12 |
| `spare_refs` | positions in `people` | a list that is empty in the retail data; see section 9 |
| `labels` | wide strings | a list that is empty in the retail data; see section 9 |
| `recent` | mission slots | the last at most three missions played, oldest first |

### 2.2 One node: the level-table record of `profile.cpf`

`docs/formats/profile.md` documents the container. The record is 244 bytes in memory and its fields are read in
a fixed order from the file. **Three corrections to that document** (each verified by re-parsing the retail file
to exact consumption, section 7):

1. The group the document reads as `u8 unknown_c, u8 unknown_d, u16 unknown_e, u16 unknown_f` is really
   `u8, u16, u8, u16` (CAMP-041). The old reading happens to total the same 14 bytes because the retail values
   make the mis-split invisible.
2. The count in front of the per-record index list is a **u32**, not a u16 (CAMP-042). That list is the node's
   *required player characters*: each entry is a 32-bit index into the character-profile table of the same file.
3. Consequently the trailing byte block is **10** bytes, not 12, followed by the three signed bytes and the
   seven u16 (CAMP-043).

Field roles (names ours; "file field" refers to `docs/formats/profile.md`'s grammar as corrected above):

| Field (ours) | Width | Meaning established here |
|---|---|---|
| `code` | 4 chars | the node's two-letter code, the key of the graph lists and of `Text/RHLevel??.red` |
| `map`, `mission_file`, `title` | strings | unchanged from `profile.md` |
| `kind` | u32 | 0 story, 1 assault, 3 ambush, 4 camp, 5 defend, 6 tactical (CAMP-044). This is the value `spec-script-vm.md` calls the campaign node type: its VM-103 sub-flag is 0 for kinds 3 and 6 |
| `needs_camp` | u8 | 0 = the node may be started without going through the camp; 1 = it is chosen in the camp (CAMP-045) |
| `location` | u32 | 1..9 as in the table above; 8 = the camp. Native 174 refuses to answer unless the *current* node's location is 8 (CAMP-046) |
| `min_money`, `max_money` | u32, u32 | the money window the availability test applies; the value 200000 in `max_money` means "no upper limit" (CAMP-050) |
| `min_gang`, `max_gang` | u16, u16 | the band-size window of the availability test |
| `max_age` | u16 | the node's lifetime in campaign days |
| `obligatory` | u8 | 1 = while this node is accessible it is the only offer (CAMP-063) |
| `prob` | u16 | a percentage used by the two random filters of section 3.3 |
| `priority` | u16 | lower wins when two nodes share a location (CAMP-065) |
| `required_pcs` | u32 count + u32 each | indices into the character-profile table; every one of them must be in the team before the mission may be sent (native 170) |
| `ares_gate[0..9]` | u8 × 10 | `ares_gate[0]` is a flag: 0 = the node is not ARES-gated. If it is 1, the node is allowed only while `ares_gate[1 + ares]` is non-zero. Because `ares` is −1 on a new campaign the expression reads `ares_gate[0]` itself, which is 1, so a gated node is allowed at the start (CAMP-031, CAMP-032) |
| `ares_on_win`, `ares_on_loss`, `ares_on_expiry` | i8 × 3 | the new ARES state after winning / losing this node, and after selecting it over-age; −1 = leave the state alone (CAMP-033) |
| `m0`, `m1` | u16, u16 | not read by any code this analysis reached (section 9) |
| `blazons_needed` | u16 | how many blazons this node needs (see 3.6) |
| `blazons_held` | u16 | how many of them the player is treated as already holding |
| `first_blazon_price` | u16 | the initial value of a slot's `blazon_price` |
| `blazon_price_step` | u16 | what a purchase adds to it |
| `men_per_blazon` | u16 | how many men one blazon is worth; the program asserts loudly if it is 0 because it divides by it |
| `music_*` | strings | unchanged |
| `team_limit` | u16 | **not stored in the file**: a run-time field of the record that native 174 returns as the team size limit of the selected node (CAMP-047). Where it is filled from was not found (section 9) |

The record also carries two run-time vectors that the file does not contain: the *after* and *until* code lists
(filled from the file) and a second requirement list that native 170 also walks; in the retail data the latter
is empty for every node (CAMP-048).

### 2.3 One person: the character-status record

Held in `people`; 72 bytes in memory. What a save carries (CAMP-070):

| Field (ours) | Width | Meaning |
|---|---|---|
| base block | 16 bytes | two 32-bit values written by a shared base serializer (0051d5d0) plus 8 bytes this analysis could not attribute; the block also contains the value 100 in a fresh profile. Treat as `unknown_base` and copy it through (section 9) |
| `health` | u16 | 0..100; a recovered man's health is re-rolled and clamped to 100 |
| `unknown_a` | u8 | — |
| `unknown_b..unknown_i` | u16 × 8 | — |
| `camp_slot` | u16 | the camp element the man is attached to, 0xFFFF = none |
| `name` | u16 count + count wide chars | the man's display name |
| `profile` | i32 | index into the character-profile table of `profile.cpf`, −1 = none |
| `present` | u8 | 1 = the man was instantiated in the current mission |

**Name generation** (CAMP-071). A man whose character profile is marked as a named hero takes his name from a
text resource: the profile's designer name is matched against a fixed list of seven names inside the executable
and the matching position selects text id 0x90 + position. A man who is not a named hero gets a generated name:
one text id drawn uniformly from 100..121 and one from 122..143, joined by a single space; the draw is retried
up to ten times to avoid a name already in use, and the name is kept even if all ten attempts collide.
(The seven designer names are data of the file, not reproduced.)

### 2.4 One workshop: the production record

Exactly 13, kind 0..12, in kind order. 52 bytes in memory; what a save carries (CAMP-090):

| Field (ours) | Width | Meaning |
|---|---|---|
| `kind` | u32 | 0..12 |
| `capacity` | u16 | the third argument of native 199 |
| `stock` | u16 | how much of the workshop's item is lying in the camp |
| `last_output` | u16 | what the last day produced |
| `unknown_flag` | u8 | — |
| `assignments` | u32 count + entries | who works where: per entry an i32 position in `people`, two 32-bit coordinates, and a u16 sector reference (0xFFFF = none) |

The work *spots* registered by native 200 live in a second, **not** serialized list: the camp script re-registers
them on every load (CAMP-091).

### 2.5 The player profile

One record of `Savegame/Profiles`; see 2.6 for the file. The engine keeps, per profile: two key-configuration
sets, a sound configuration, a graphic configuration, one further coordinate pair, the player's name as a wide
string of at most 31 characters, the list of save slots, and six summary numbers that the campaign refreshes
(CAMP-100):

| Field (ours) | Source |
|---|---|
| `score` | counter 2 |
| `money` | counter 1 |
| `spared` | 0 when counters 4 and 5 are both 0, else `100 * counter4 / (counter4 + counter5)` with integer division (CAMP-101) |
| `playtime` | `playtime += counter 6` |
| `progress` | the percentage of section 3.7 |
| `unknown_x`, `unknown_y` | two further 32-bit values, both 1 in a fresh retail profile; one of them is the difficulty (four presets exist, matching the four records of table B of `profile.cpf`), but which one was not established (section 9) |

### 2.6 The files

All three are the same serializer: a stream of fields with a 16-byte signature in front of each class. The
signature is a digest of the class name; it is *checked* on reading and a mismatch only produces a warning, so
an implementation may write any 16 bytes it likes as long as it writes the right number of them — but writing
the original's values is what lets the original read our files, so the sixteen bytes must be preserved
byte-for-byte from the player's own files or hard-coded (CAMP-002). They are content of the binary and are not
reproduced here; an implementation reads them out of the player's `Campaign.bck` / `Profiles` once and reuses
them, or keeps them in a private table generated from the player's files at build time.

Two string conventions appear: a byte string is a u16 count followed by that many bytes; a wide string is a u16
count followed by that many 16-bit characters — except the player's name, which uses a **u32** count
(CAMP-003).

#### 2.6.1 `Campaign.bck` (installation root)

`u32 archive_version` followed by the campaign block of 2.6.2. The retail file is version 48 and 2717 bytes.
It is rewritten from the live campaign every time the game loop starts a level in campaign mode (CAMP-120), and
it is read back verbatim when a save is written (CAMP-123). Failure to create it is reported and ignored.

#### 2.6.2 The campaign block

Field order (this is the order an implementation must write; `V` = the archive version):

1. `signature(RHCampaign)` — 16 bytes.
2. `recruit_flag` — u8. Only present when `V > 27`.
3. the 27 counters — 27 × i32 = 108 bytes.
4. `ares` — i8.
5. the node state: `u32 count`, then `count` mission slots, each: `signature(RHMission)` 16 bytes, `u16 age`,
   `u16 blazon_price`, `u32 outcome`, **two u16 of uninitialised memory**, `i32 node` (−1 = none). 32 bytes
   each. The two u16 are read and discarded; the original writes whatever happened to be in a register, and the
   two campaign blocks of one save file differ in exactly these bytes (CAMP-022). An implementation may write
   zeros.
6. `offered`: `u32 count`, then `count` × u16 slot index (0xFFFF = none).
7. `pending`: the same.
8. `people`: `u32 count`, then `count` character-status records (2.3).
9. `band`: `u32 count`, then `count` × i32 position in `people`.
10. `recovering`: the same.
11. `team`: the same.
12. `workshops`: `u32 count`, then `count` production records (2.4).
13. `spare_refs`: `u32 count`, then `count` × i32.
14. `labels`: `u32 count`, then `count` wide strings.
15. `current`, `previous`, `selected`, `blazon_node` — four u16 slot indices, 0xFFFF = none, **in that order**
    (CAMP-023).
16. only when `V > 29`: `recent`: `u32 count`, then `count` × u16 slot index.
17. only when `V >= 41`: `town_result` — i32.
18. only when `V >= 48`: `town_node` — i32 (a node index).

#### 2.6.3 A saved game (`Savegame/Profile_<nnn>/<name>`)

| Offset | Content |
|---|---|
| 0 | the four characters `GSHR` |
| 4 | `u32 archive_version` (48 in the retail build) |
| 8 | `u32` the four characters of the level code of the mission the save belongs to |
| 12 | `u32 archive_version` again |
| 16 | **campaign block A** |
| … | **campaign block B** |
| … | the level and actor state, written by the level serializer (004c4570); not specified here |

Block A is byte-identical to the payload of `Campaign.bck` (CAMP-124); block B is the live campaign state at the
moment of saving. Both blocks are present only when `V > 38`; on loading, both are deserialized in order, so
block B wins. Practically: block A is the campaign as it was when the mission started (that is what makes
"Restart" possible), block B is the campaign as it is now. Only the first three dwords and the second version
dword are validated, and only the magic actually causes a refusal; a wrong version merely mis-parses
(CAMP-125).

The header's magic check is the only integrity test. There is no length field, no checksum over the payload and
no compression.

#### 2.6.4 The thumbnail

A second file whose name is the save's data name with `_t` appended, written immediately after the save, in the
picture format of `docs/formats/image-blob.md`; `docs/formats/savegame.md` records 160x120. It is a capture of
the screen at the moment of saving and is not needed to load a save (CAMP-126).

#### 2.6.5 `Savegame/Profiles`

| Field | Width | Meaning |
|---|---|---|
| magic | 4 bytes | the four characters `FORP` |
| version | u32 | must be 6, otherwise the archive is discarded and default profiles are created (CAMP-110) |
| `next_dir` | u32 | the number the next profile directory gets (`Profile_%03i`) |
| count | u32 | number of profiles |
| profiles | — | `count` records, each as below |
| current | i32 | position of the selected profile, negative = none |

One profile record:

1. `signature(RHPlayerProfile)`.
2. six u32 in this order: `unknown_x`, `score`, `money`, `spared`, `playtime`, `progress`, then `unknown_y`
   (seven values in total) (CAMP-111).
3. key set 1, key set 2: each `signature(RHKeyConfig)`, `u16 set_id`, then 58 bytes = **29** u16 key codes.
   `Configuration/keyset1.cfg` and `keyset2.cfg` are exactly these 76 bytes and carry set ids 2 and 3
   (CAMP-112; this corrects `docs/formats/profile.md`, which read the 76 bytes as a 16-byte header plus 30
   codes).
4. sound configuration: `signature(RHSoundConfig)`, `u8`, `u8`, then five u16 (12 bytes of payload).
5. graphic configuration: `signature(RHGraphicConfig)`, four u8 written in the order *first, second, fourth,
   third* of the in-memory block, then two 32-bit floats = the screen width and height (CAMP-113 — this is the
   float pair the community resolution patcher edits at profile offset 0x104).
6. one further coordinate pair: two 32-bit floats.
7. the player's name: `u32 count`, then `count` 16-bit characters.
8. the save-slot manager: `signature(RHSaveGameManager)`, `u32 next_number` (the counter behind the numbered
   slot names), `u32 count`, then `count` slots, each `signature(RHSaveGame)`, byte string *data file name*,
   byte string *thumbnail file name*, wide string *label shown to the player* (CAMP-114).

---

## 3. Behaviour

### 3.1 Start-up and a new campaign

At start-up the profile archive is read (2.6.5); a broken or missing archive is replaced by default profiles.
A new campaign is built as (CAMP-010, CAMP-200):

1. All 27 counters 0, then `money := 100`; `ares := -1`; all four node references null; every list empty.
2. One mission slot per level-table record, in table order, with `age := 0`, `outcome := 0` and
   `blazon_price := ` the record's `first_blazon_price`.
3. The band: one man, created from the character-profile entry **at index 1** of `profile.cpf` (the program
   asserts that index 0 and index 1 are the hero's two appearances and refuses to run otherwise), with
   `present := 0`. The command line can replace this with a longer band: a string of letters, each letter
   selecting one character profile by its designer name. The default is the single man.
4. `team := band`.

**Node 0 of the level table is the camp** and is never a candidate: the accessibility scan starts at slot 1
(CAMP-201). Sending the player "to the camp" means making node 0 the current mission.

### 3.2 When is a node accessible

For a mission slot `s` of node `n`, with `money` = counter 1 and `gang` = the length of `band`, all of the
following must hold (evaluated in this order; the debug report at 0054c8a0 names the first failure)
(CAMP-060):

1. `money >= n.min_money`
2. `money <= n.max_money` **or** `n.max_money == 200000`
3. `gang >= n.min_gang`
4. `gang <= n.max_gang`
5. `s.age < n.max_age`
6. `n.ares_gate[0] != 1` **or** `n.ares_gate[1 + ares] != 0` (with `ares == -1` this reads `n.ares_gate[0]`,
   which is 1, so the test passes)
7. every code in `n.after` names a slot whose `outcome != 0` — that is, every prerequisite has been **played**,
   won *or* lost (CAMP-061)
8. every code in `n.until` names a slot whose `outcome == 0` — no blocker has been played yet (CAMP-062)

A code that names no node makes the lookup return the first slot of the table; the retail data contains no such
code.

Two consequences worth stating because they are easy to get wrong: (a) the *after* list is satisfied by a lost
prerequisite as well as a won one; (b) because every story node lists **itself** in its own *until* list, a
story node is permanently gone once played, which is what keeps it out of the list after its slot's `outcome`
is reset in step 1 of 3.3.

### 3.3 Rebuilding the offered list

Run when the campaign needs the next mission (00451a90), once per campaign day:

1. For every slot from index 1 to the end, in slot order: if the slot is accessible (3.2) and is not already in
   `offered`, append it to `offered` and **set its `outcome` to 0** (CAMP-202).
2. Reduce `offered` (3.3.1), then reduce `pending` the same way.

#### 3.3.1 The reduction

Work on a copy of the list. Repeat at most ten times (CAMP-203):

| Step | Applies when | Effect |
|---|---|---|
| a | size > 1 | **story deadline / expiry filter.** Walk the list. If a slot's node is a story node and `s.age == n.max_age - 1`, stop: that slot becomes the entire list (everything else is dropped with its `age` reset to 0). Otherwise, if `s.age == n.max_age`, drop the slot with `age := 0`; and if its node is a defend or assault node whose `outcome` is still 0, **mark it lost** (3.5) (CAMP-064, CAMP-210) |
| b | size > 1 | **obligatory filter.** Sort so that obligatory nodes come first. If the first slot's node is obligatory and the second's is not, the list becomes just the first slot (the rest get `age := 0`). Two obligatory nodes at once is an error the program reports and then ignores (CAMP-063) |
| c | `offered` only, size > 1 | **assault / defend random filter**, using the node's `prob` (3.3.2) |
| d | `offered` only | **blazon-node selection** (3.6) |
| e | size > 1 | **recency filter.** Drop every slot that is in `recent` (the last at most three missions played), with `age := 0` (CAMP-211) |
| f | size > 1 | **non-assault / defend random filter**, using `prob` (3.3.2) |
| g | size > 1 | **one per location.** Sort by (map name, `priority`) ascending and keep, for each location, only the slot with the lowest `priority`; the others are dropped with `age := 0`. Two candidates with the same location *and* the same priority is an error the program reports and then ignores (CAMP-065) |

If the list is non-empty the reduction is done. If it is empty, repeat from step a. After ten empty rounds the
program reports that it cannot determine an accessible mission, prints the whole campaign state, says it is
forcing a mission, and then: if the working list has fewer than two entries it is used as is; otherwise one
entry is drawn uniformly at random, every other slot of the source list gets `age := 0`, and the drawn slot
becomes the whole list (CAMP-204).

#### 3.3.2 The two random filters

Both examine `rand() % 101` against the node's `prob` and act when `prob < rand() % 101` (CAMP-212). One filter
considers only nodes of kind 1 or 5 (assault, defend), the other only nodes that are neither. Both also require
the slot's `age` to be 0. The precise membership decision of these two passes (keep or drop) could not be
separated from the surrounding container code the decompiler folded away; see section 9. Determinism note: both
draw from the C run-time generator, in list order.

### 3.4 Choosing the next mission and the camp

When the campaign is asked for the next mission (004539f0) (CAMP-220):

1. If `selected` is set (the player picked a mission in the camp, or the command line forced one):
   `previous := current`; `current := selected`; `selected := null`.
2. Otherwise: rebuild the offered list (3.3). Then, for every offered slot, if its node is a defend node whose
   `blazons_needed` is 0, **mark it won** (3.5) (CAMP-221). Then reset `team := band` and set
   `previous := current` and:
   - if `offered` holds **exactly one** slot and that node's `needs_camp` is 0, `current :=` that slot;
   - otherwise `current :=` slot 0, the camp.
3. If `current` is not the camp: remove it from `offered` (with `age := 0`), and append it to `recent`; if
   `recent` then holds more than three entries, drop the oldest.

This is the whole "Sherwood becomes reachable only after the second mission" rule: the first two story nodes
have `needs_camp == 0` and are the only accessible node at their time, so they start directly; everything else
funnels through node 0 (CAMP-222, and it matches `campaign-flow.md` section 1).

**Selecting a mission in the camp** (004560d0) advances the campaign day (CAMP-230). For each slot in
`offered`, in order:

- if it is the slot the player picked: `selected :=` it, its `age := 0`, and it is removed from `offered`;
- otherwise: if the node's `max_age < 1000` **or** the slot's `age` is 0, `age := age + 1`; then, if the
  *picked* slot's `age` now exceeds its own node's `max_age`, apply the picked node's `ares_on_expiry`
  (CAMP-231).

Because the picked slot is removed without advancing the walk, slots before it in the list age and slots after
it age too; the picked slot itself does not.

### 3.5 Recording an outcome

Applying an outcome to a slot `s` of node `n` (00456540) (CAMP-240):

| Outcome | Effect |
|---|---|
| won | `ares := n.ares_on_win` unless that byte is −1; `s.outcome := 1` |
| lost | `ares := n.ares_on_loss` unless that byte is −1; `s.outcome := 2`; and if `s` is `blazon_node`, `blazons := 0` |

In both cases, if `s` is `blazon_node` and `n.kind == 5` (defend), the pending town notification is armed:
`town_node := n`, `town_result := s.outcome`. The campaign-map screen reads `town_result` and clears it
(CAMP-241).

**End of a mission** (004e3260, run by the level tick after the won / lost flag is set, `spec-script-vm.md`
VM-103) (CAMP-250), in this order:

1. Apply the outcome to `current`.
2. Walk the level's player-character list in element order. For each man the level still knows and who belongs
   to the campaign: if his health is at most 0 he counts as *lost*, otherwise he counts as *returned*, the
   level's own survivor counter is bumped, and if his character profile carries a particular class marker or
   his actor carries a particular flag he additionally contributes 70 score points.
3. Walk the level's item list and collect what the rules there allow (the loot pass; not specified here).
4. If the mission was won: `returned += survivors`; `lost += casualties`; `score += the 70-point sum`; if
   `current`'s kind is not 3 (ambush), `score += 1000`; `recruit_flag := 0`; then add *k* new men to the band,
   where *k* is a number the level object provides (3.8); record *k* on the level for the debriefing; if
   `blazon_node` is set, refresh the blazon HUD.
5. If the mission was lost: no counter changes at all; the level records that no recruits arrived.

**Score during a mission** (CAMP-251): `score += 50` for one player-character act (004a5a00, gated on two
virtual tests of the actor), `score += 100` when a player character's inventory count for a slot changes
(0049fc80, i.e. picking something up). Every `score` change is also added to the mission-statistics block the
debriefing screen shows; every `money` change likewise (CAMP-252).

### 3.6 Blazons

`blazons` (counter 3) belongs to one node at a time, `blazon_node`, chosen in step d of the reduction: the first
slot of the working list whose node is of kind 1 or 5 (assault or defend). If there is none, `blazon_node` is
null. While `blazon_node` is set, the other assault / defend slots are taken out of `offered` and moved to
`pending`; if `blazon_node` is null they are dropped instead (CAMP-260).

Four ways the counter moves (CAMP-261):

| Where | Effect |
|---|---|
| native 178, a banner captured in a mission | `blazons += the banner's value`. If the current node's kind is 1 (assault) and `blazons >= blazons_needed`, the mission is **won** at once |
| a tactical mission (kind 6) | the counter is capped at `blazons_needed − blazons_held`; the excess is handed to a separate routine. If the mission is ending and `blazons >= blazons_needed`, `blazons := 0`, the blazon node is marked **won** and removed from `offered` |
| the campaign map, buying a blazon (00525040) | `money -= the slot's blazon_price`; `blazons += 1`; `blazon_price += the node's blazon_price_step` |
| the camp / campaign map, spending blazons on a defend node | if `blazons >= blazons_needed`: `blazons -= blazons_needed`, the node is marked **won**, removed from `offered`, `selected := null`, and the offered list is rebuilt |

The number of blazons the player still needs, as the interface reports it, is `blazons_needed − blazons_held` of
the node `blazon_node` when that differs from `current`, and `blazons_needed` of `current` otherwise
(00455af0); native 234 answers "blazons >= that number" (CAMP-262).

`men_per_blazon` converts men into blazons: the number of men that may be spent is
`men_per_blazon * min(team_size / men_per_blazon, blazons_needed − blazons_held − blazons)`, with unsigned
integer division and clamping at 0 (00455a70). The program asserts when `men_per_blazon` is 0 (CAMP-263).

### 3.7 Progress, score, spared lives, game length, difficulty

**Progress** in percent (00456db0) (CAMP-270). Count nodes whose kind is neither 3 nor 6 (so ambushes and
tactical missions do not count, placeholders do); call that `total`, and count how many of those have
`outcome == 1`; call that `won`. Then:

- if any of the won nodes has the code `HI`, the answer is **100**;
- otherwise if any has the code `DD`, the answer is **95**;
- otherwise the answer is `won * 100 / total`, integer division.

`HI` and `DD` are two four-byte constants compiled into the executable; they are compatibility tokens, marked
individually, and are needed because the campaign's end is recognised by code, not by data.

**Score** is counter 2 (3.5). **Spared lives** is `100 * returned / (returned + lost)`, integer division, 0
when both are 0 (CAMP-101); the campaign-map status line (00528810) and the player profile use the same
expression. **Game length** is counter 6 in seconds: the game loop adds `(now − level_start) / 1000` at the end
of a level, with `now` from the system tick counter in milliseconds (CAMP-271). One display path (0050e7d0)
adds the *unscaled* millisecond difference to the same value, which mixes units; that path feeds a version /
debug line only (CAMP-272).

**Difficulty.** Four presets exist (table B of `profile.cpf` has four records) and a new profile is created at
the middle setting. The profile carries two 32-bit values that are both 1 in a fresh retail profile and one of
them is the difficulty; which one, and how the preset reaches the combat rules, was not settled (section 9).
No difficulty-dependent branch was found in the campaign code itself (CAMP-280).

### 3.8 Recruits

A new man is added by 004524b0 (CAMP-290). With probability 1/2 (one draw of the C generator, low bit) and only
if `recovering` is non-empty: one entry of `recovering` is drawn uniformly, his health is re-rolled and clamped
to 100, his `camp_slot` is set to 0xFFFF, he is moved into `band`, and `recruit_flag := 1` — a man comes back
from the sick list. Otherwise a brand-new man is built from the character-profile table, considering only
profiles that are not marked as named heroes (the exact draw is one the decompiler folded away, section 9).

The number of men added after a won mission is a value the level object carries; the level fills it from the
mission's own bookkeeping (004e3260 reads it through a level accessor and truncates it to an integer). Men are
added one at a time, so a run of *k* recruits makes *k* independent draws (CAMP-291).

Men leave the band by being moved to `recovering` (00455b80) or by being dropped outright (00455b20); both are
driven by the HUD / mission-end paths of 005145b0.

### 3.9 The camp between missions

Entering the camp is entering node 0 as an ordinary mission on the `sherwood` map with its own script
(`docs/formats/sherwood-hub.md`). What the engine does around it (CAMP-300, from the game loop at 0050f710):

1. Write `Campaign.bck` from the live campaign (2.6.1).
2. Create the *Restart* auto-save — but only when the level being started is **not** the camp (3.10).
3. If the level is the camp (the game object's "in the camp" flag, which is set from `current`'s
   `location == 8`) and the level is not in one particular state: **send message 1001 to the level script**, then
   **run the production of all 13 workshops** (3.11).
4. Enter the frame loop.

Message 1001 is therefore an engine-originated message delivered once, when the camp level starts, before the
first tick; the camp script answers it by configuring its 13 production zones with natives 199 and 200
(CAMP-301). This confirms and replaces the guess in `docs/formats/sherwood-hub.md` section 5.

Message 1000 ("send the team out") is delivered by the interface when the player uses the send action; the
script answers it by walking the selected characters to the exit. This analysis located the message path
(00578d80) and the camp-side handler but not the button that raises 1000; see section 9 (CAMP-302).

**Team deployment.** The team is `team`. `team := band` whenever the campaign picks a new mission (3.4 step 2).
Natives 165 and 166 add and remove one man (they refuse a non-player-character with a script error): 165
appends the man's status record to `team` if it is not already there, marks him present, clears two of his
fields and refreshes the HUD; 166 removes him and refreshes the HUD (CAMP-310). Native 163 is the length of
`team`, 164 its i-th member's element. Native 174 is the selected node's `team_limit`, or 5 when no node is
selected, and it reports an error and answers 0 unless the current node's location is 8 (CAMP-311). Native 170
answers "the team satisfies the node's requirements": every entry of the node's `required_pcs` list must be the
character profile of some member of `team`, and the same for the node's second (in retail: empty) requirement
list (CAMP-312). Native 172 is the selected node's four-character code, 0 when nothing is selected — this is
what the camp script compares against a level code to notice that a mission has been picked.

**Instantiating the team.** When a mission starts, each team member gets a player-character actor; his status
record is marked present, and his `camp_slot` keeps a value only while the current node's location is 8, else it
is set to 0xFFFF (004552f0) (CAMP-313). A node whose `required_pcs` cannot be matched to a spawn point produces
a warning and the spawn point stays empty.

### 3.10 The five named save slots and the auto-saves

The executable knows five fixed slot names — `Continue`, `Restart`, `QuickSave`, `ExQuickSave`, `Sherwood` —
plus numbered slots named by a `Savegame_%03i` pattern whose counter is the save manager's `next_number`
(CAMP-130). Slot labels shown to the player come from text resources (0xF6 for *Continue*, 0xF7 for *Restart*),
so the data file name and the label differ.

| Slot | Written when |
|---|---|
| `Restart` | at the start of every level that is not the camp (CAMP-131) |
| `Sherwood` | at the start of the camp level (CAMP-132) |
| `Continue` | after **every** successful save: the file just written is copied to `Continue`, unless the save that was written is itself `Continue` or `Restart` (CAMP-133) |
| `QuickSave`, `ExQuickSave` | the quick-save key; `ExQuickSave` holds the previous quick save |
| numbered | the save menu |

Writing a save (00511340): open the data file for writing, write the header, commit the elapsed play time into
counter 6, write campaign block A by streaming `Campaign.bck` (minus its version dword) into the archive, write
campaign block B from the live campaign, write the level state; then write the thumbnail file; then mirror into
`Continue`; then show a message from a text resource. Loading is the same function with the direction reversed:
both campaign blocks are deserialized in order, then the level state (CAMP-134).

"Restart" therefore restores the campaign as it was when the mission began and reloads the mission from its
start; "Continue" restores the last save of any kind. Neither is a separate mechanism: both are ordinary saves
under fixed names.

### 3.11 Production between missions

The 13 workshops are addressed by kind, `campaign.workshops[k]`, and are the objects natives 199 and 200
configure (CAMP-320):

- **native 199 (k, location, capacity)**: workshop `k` gets that location as its zone and that capacity, and
  the location is back-linked to the workshop kind.
- **native 200 (k, location)**: a work spot is appended to workshop `k`'s (non-persistent) spot list, carrying
  the location's coordinates, one of its identifiers and the index of its sector, or 0xFFFF when the location
  has no sector.

Production runs once per camp entry, over all 13 workshops in kind order, and **only if the previous mission
was won** — the routine is handed `previous` and does nothing unless its `outcome` is 1 (CAMP-321). Per
workshop kind:

| Kind | What one day does |
|---|---|
| 0..8 | compute the day's output; `stock += output`; `last_output := output`; then place up to 5 units of the workshop's item per work spot in the camp, drawing down `stock` (CAMP-322) |
| 9, 10 | compute the day's output and hand it to a global player-wide store, at index 1 for kind 9 (only when a further test passes) and index 0 for kind 10 (CAMP-323) |
| 11 | compute the day's output and apply it to **every** man assigned to the workshop — this is the training workshop (CAMP-324) |
| 12 | a separate routine (00580e80), not read |

Each kind maps to one item id: 0->1, 1->4, 2->5, 3->11, 4->12, 5->13, 6->16, 7->17, 8->19; the others have no
item (CAMP-325). Existing stock is recounted from the camp level before production: every pick-up item in the
level whose item id matches contributes its quantity to `stock`, and for item id 1 certain actors count too
(CAMP-326).

Each kind also has a **supervising hero**, identified by campaign identity (the ids of native 256): kinds 0, 1
and 9 the hero; kind 2 Scarlet; kinds 3 and 7 Stutely; kinds 4, 5 and 8 Tuck; kind 6 Marian; kind 10 John;
kinds 11 and 12 none (CAMP-327). The day's output is computed from the workshop and from whether its supervising
hero is among the men assigned to it; **the arithmetic is a floating-point expression the decompiler dropped**
and is an open question (section 9). What is certain: the result is truncated towards zero to an integer, and a
workshop with no assigned men and no supervisor still runs.

After production each assigned man is placed at his work spot in the camp level (CAMP-328), which is what makes
the camp look busy.

### 3.12 The campaign-map screen

Reached from the camp. It shows ten location slots, indices 0..9; index 0 and index 8 (the camp) are skipped,
and for indices 1..3 (the three forest crossings) one of the per-location widgets is skipped as well
(CAMP-330). Two further widget groups are driven by the current `ares` value. The miniature of a location shows
the offered mission for that location — by construction of step g of 3.3.1 there is at most one. If the offered
list is empty the screen shows a text and the program reports a warning (CAMP-331). A status line is composed
from text resources 0xF3, 0xF4, 0x40 and carries `money`, `score` and the spared-lives percentage (CAMP-332).
The buttons along the bottom are the purse, recruit, cart and tower/shield actions of `ui-flow.md` 9.x
(`BTTN` 71-74 on this screen, 157-160 in the camp). The pending town notification (`town_result`, `town_node`)
is read here and cleared (CAMP-241).

### 3.13 HUD and mini-map

What this analysis settles (the rest of the HUD is `ui-flow.md`, measured from captures):

- The two top-left counters are `money` (counter 1) and `clovers` (counter 0), each formatted with its own text
  resource (0xF4 and 0xF5) (CAMP-340).
- The blazon display is refreshed whenever `blazons` changes and only exists in campaign mode; outside a
  campaign the refresh reports an error and does nothing (CAMP-341).
- **Mini-map dot colours.** Each element carries a dot style code and a three-entry colour set. The default
  colour set is fixed per element class: a player character is light green, RGB (165, 255, 82); a
  non-player human is red, RGB (255, 0, 0), unless one flag on his controller is set, in which case he is
  violet, RGB (115, 40, 203); a non-player human additionally carries a yellow, RGB (255, 255, 0), and a blue,
  RGB (0, 120, 255), entry for two further states; one further element class is cyan, RGB (0, 190, 255), and
  another pale yellow, RGB (255, 247, 90) (CAMP-350).
- **Native 24** overrides the style: codes 0 and 1 are the class default; 100/101/102 and 111 set light green,
  200/201/202 and 222 red, 300/301/302 and 333 cyan; the codes 111, 222 and 333 additionally require the
  element to belong to the human family and report an error otherwise; 500 and 666 are accepted but match no
  colour case, so they leave the colour set unchanged; every other code is an error that changes nothing
  (CAMP-351). This refines the entry in
  `spec-script-vm.md` with the actual colours.
- No grey colour is set anywhere in the image. A grey dot in a capture is therefore either a dot that is not
  drawn at all (an element the script un-blipped, native 243) or the map's own dark background, RGB (0, 30, 60)
  (CAMP-352).
- The portrait counters, the action-icon availability and the tooltips were **not** read (section 9); the
  character name shown under a portrait is the `name` field of 2.3 and the portrait faces are the resources
  `ui-flow.md` 9.3 lists.

---

## 4. Claims

| Id | Claim (one sentence) | Status | Evidence | Confidence | Notes |
|---|---|---|---|---|---|
| CAMP-002 | Each serialized class is preceded by a 16-byte digest of its class name; a mismatch only warns | observed | 005e1b70, 0060f1c0 | high | reading is tolerant, so the digest need not be recomputed, only reproduced |
| CAMP-003 | Byte strings and wide strings use a u16 count; the player's name uses a u32 count | observed | 005e0c90, 005e0ea0, 0055d1a0 | high | validated on `Profiles` |
| CAMP-010 | A new campaign has all 27 counters 0 except money = 100 | observed | 00450ec0, `Campaign.bck` | high | |
| CAMP-011 | The 27 counters are i32 and the whole block is 108 bytes | observed | 00452030, 00452f20 | high | |
| CAMP-012 | Natives 195/196 address counters 7..26 as script indices 0..19 | observed | 00579430, 00579470 | high | agrees with `spec-script-vm.md` |
| CAMP-013 | Counter 1 is money, 2 score, 3 blazons, 4 men returned, 5 men lost, 6 play time in seconds | observed | 0055d370, 00528810, 004e3260, 0050e800, 005795c0 | high | the debug report names money, gang size and ARES |
| CAMP-016 | Counter 0 is the clover HUD counter and no code path writes it through the counter accessors | observed | 004c8140, 004a78c0 | medium | a variable-index write via native 196 cannot reach it; how it ever becomes non-zero is open |
| CAMP-020 | The node state is one 12-byte slot per level-table record: age u16, blazon price u16, outcome u32, plus a node reference | observed | 0054cc40, 00452f20 | high | |
| CAMP-021 | Every campaign list is serialized as a u32 count followed by its elements in order | observed | 00452f20, 00453630, 00458be0, 00458e70, 00459160, 004593f0 | high | |
| CAMP-022 | Each mission slot record contains two u16 of uninitialised memory that are read and discarded | observed | 0054cc40; the two campaign blocks of the retail `Continue` differ in exactly those bytes | high | write anything |
| CAMP-023 | The four node references are written in the order current, previous, selected, blazon node, each as a u16 slot index with 0xFFFF for null | observed | 00452f20, 00453940; retail `Campaign.bck` reads 0xFFFF, 21, 0xFFFF, 0xFFFF | high | 21 is the slot of the first story node |
| CAMP-030 | ARES is one signed byte, −1 on a new campaign | observed | 00456e40, 00456e60, retail file | high | |
| CAMP-031 | A node is ARES-gated only when the first byte of its 10-byte gate array is 1 | observed | 0054c800 | high | |
| CAMP-032 | The gate is `gate[1 + ares] != 0`, so `ares == -1` reads the enable flag itself and always passes | observed | 0054c800 | high | a deliberate or accidental aliasing; either way it is the behaviour |
| CAMP-033 | Winning, losing and over-age selection each set ARES from one signed byte of the node, −1 meaning no change | observed | 00456540, 004560d0, 00456e40 | high | |
| CAMP-041 | The level record's field group after the location is u8, u16, u8, u16 and not u8, u8, u16, u16 | observed | 00566570 | high | corrects `docs/formats/profile.md` |
| CAMP-042 | The required-player-character list of a level record is counted by a u32 | observed | 00566570 | high | corrects `profile.md`; re-parsed to exact consumption |
| CAMP-043 | The trailing byte block of a level record is 10 bytes, then 3 signed bytes, then 7 u16 | observed | 00566570, exact re-parse | high | |
| CAMP-044 | The node's kind is the u32 the old document called `unknown_a`, with 0 story, 1 assault, 3 ambush, 4 camp, 5 defend, 6 tactical | observed | 005795c0, 00514730, 0054cbf0, 0054cc20, 00456db0 | high | |
| CAMP-045 | A node whose `needs_camp` byte is 0 may start without visiting the camp | observed | 004539f0 | high | explains the first two missions |
| CAMP-046 | Location 8 is the camp and several routines gate on it | observed | 00579300, 0050b640, 004552f0 | high | |
| CAMP-047 | Native 174 answers the selected node's run-time `team_limit` u16, 5 when nothing is selected | observed | 00579300 | high | the field is not in `profile.cpf` |
| CAMP-048 | Native 170 also walks a second, in retail empty, requirement list of the node | observed | 00453cd0 | medium | never non-empty in the retail data |
| CAMP-050 | 200000 in the maximum-money field means "no maximum" | observed | 0054c800, 0054c8a0 | high | a single sentinel constant |
| CAMP-060 | The availability test is the eight conditions of 3.2, in that order | observed | 0054c800, 0054c8a0 | high | the debug variant names each failure |
| CAMP-061 | The *after* list requires each prerequisite to have been played, won or lost | observed | 0054c6f0 | high | contradicts the "available after winning" reading in `profile.md` |
| CAMP-062 | The *until* list requires each blocker to be unplayed | observed | 0054c6f0 | high | |
| CAMP-063 | An obligatory node, when accessible, is the only offer; two at once is a reported error | observed | 00452e70 | high | |
| CAMP-064 | A story node whose slot age reaches `max_age - 1` becomes the only offer; at `max_age` a slot is dropped, and a dropped defend / assault slot that was never played is marked lost | observed | 004526f0 | high | ignoring a defend mission loses it |
| CAMP-065 | At most one mission per location is offered, the one with the lowest priority value | observed | 00452d20, 0054c780 | high | |
| CAMP-070 | The character-status record carries health, a camp slot, a wide name, a character-profile index and a present flag around an unattributed 16-byte base block | observed | 0055c3c0, 0055bb30, 0051d5d0, exact re-parse of the retail record (98 bytes) | high for the framing, medium for the base block | |
| CAMP-071 | A generated man's name is one text id from 100..121 and one from 122..143 joined by a space, retried up to ten times for uniqueness; a named hero's name is text 0x90 + his position in a seven-name list | observed | 0055bf00, 0055bdf0 | high | the seven names are data of the binary and not reproduced |
| CAMP-090 | A workshop record carries kind, capacity, stock, last output, a flag and a list of assignments | observed | 00580970, 00580ab0 | high | 13 records in the retail file, payload 15 bytes each |
| CAMP-091 | The work spots of native 200 are not saved; the camp script re-registers them each load | observed | 00580220, 00580970 | high | |
| CAMP-100 | The profile's six summary numbers are score, money, spared percent, accumulated play time and progress percent | observed | 0055d370, 0055d1a0, retail `Profiles` | high | |
| CAMP-101 | Spared lives = 100 * returned / (returned + lost), integer, 0 when both are 0 | observed | 0055d370, 00528810 | high | the float expression itself was dropped; the guard and the campaign-map copy fix it |
| CAMP-110 | `Savegame/Profiles` starts with the four characters `FORP` and a u32 version that must be 6 | observed | 0055dea0, retail file | high | |
| CAMP-111 | A profile record's seven u32 are written in the order unknown, score, money, spared, play time, progress, unknown | observed | 0055d1a0, exact re-parse | high | |
| CAMP-112 | A key configuration is a 16-byte signature, a u16 set id and 29 u16 key codes; `keyset1.cfg` / `keyset2.cfg` are exactly that and carry ids 2 and 3 | observed | 0051ec40, both files | high | corrects `profile.md`'s "30 u16" |
| CAMP-113 | The graphic configuration is four u8 written in the order 1, 2, 4, 3 followed by two floats giving the screen width and height | observed | 0051ba00, 005fc2a0, retail file (1024, 768) | high | matches the community resolution patch offset |
| CAMP-114 | A save slot is a signature, a byte-string data file name, a byte-string thumbnail name and a wide label | observed | 0056ea20, 0056ff10, retail file | high | |
| CAMP-120 | `Campaign.bck` is a u32 version plus the campaign block and is rewritten at every level start in campaign mode | observed | 00457560, 0050f710 | high | |
| CAMP-123 | On writing a save the campaign block A is streamed verbatim out of `Campaign.bck` | observed | 004576f0, 00511340 | high | |
| CAMP-124 | A retail save contains the campaign block twice; block A equals the `Campaign.bck` payload byte for byte and block B is the live state | observed | 00511340; byte comparison of the retail `Continue`, `Restart` and `Campaign.bck` | high | both start at fixed offsets 16 and 2729 in the retail files |
| CAMP-125 | Only the `GSHR` magic is enforced; the version dwords are read but a mismatch does not refuse the file | observed | 0056ea70 | medium | the refusal expression is partly folded |
| CAMP-126 | The thumbnail is a separate file named `<data name>_t` in the image-blob format | observed | 00511340, 0056e6b0, retail files | high | |
| CAMP-130 | Five fixed slot names exist plus numbered slots from a `Savegame_%03i` pattern and a counter in the profile | observed | 0056ef30, 0056f090, 0056f1f0, 0056f350, 0056f4b0, 0056fa30 | high | |
| CAMP-131 | *Restart* is written at the start of every non-camp level | observed | 0050f2d0, 0050f710 | high | |
| CAMP-132 | *Sherwood* is written at the start of the camp level | observed | 0050f3d0 | high | |
| CAMP-133 | Every successful save is mirrored into *Continue* unless it is itself *Continue* or *Restart* | observed | 0050f1a0, 00511340 | medium-high | the two exclusion tests were not read individually |
| CAMP-134 | Save and load are one routine with a direction flag; on load both campaign blocks are applied in order, so block B wins | observed | 00511340 | high | |
| CAMP-200 | A new campaign's band is one man built from character profile index 1; a command line string can replace it | observed | 00450ec0, 0048f280 | high | the program asserts the two hero profiles are at 0 and 1 |
| CAMP-201 | Level-table record 0 is the camp and the accessibility scan starts at record 1 | observed | 00451a90, 004539f0 | high | the retail record 0 is the camp node |
| CAMP-202 | A slot newly appended to the offered list has its outcome reset to 0 | observed | 00451a90 | high | harmless because story nodes block themselves |
| CAMP-203 | The reduction is applied to a copy, up to ten times, in the order a..g | observed | 00451b70 | high | |
| CAMP-204 | After ten empty rounds the campaign reports the failure, prints its state and forces one mission, drawn uniformly when more than one candidate is left | observed | 00451b70, 00456e60 | high | |
| CAMP-210 | Step a is the only place a slot is dropped for age, and the only place an ignored defend / assault node is marked lost | observed | 004526f0 | high | |
| CAMP-211 | The last at most three played missions are excluded from the offered list | observed | 00452c90, 004539f0 | high | the list is saved from version 30 on |
| CAMP-212 | The two random filters compare the node's percentage field with `rand() % 101` and act when the field is smaller | observed | 00452040, 00452090 | medium | which side is kept was not separated; see section 9 |
| CAMP-220 | The next mission is the selected one, else the single offer whose node does not need the camp, else the camp | observed | 004539f0 | high | |
| CAMP-221 | Before choosing, every offered defend node whose blazon requirement is 0 is marked won | observed | 0054cda0 | medium | no retail defend node has a 0 requirement |
| CAMP-222 | The team is reset to the whole band whenever the campaign picks a new mission | observed | 00455650, 004559f0 | high | |
| CAMP-230 | Selecting a mission in the camp ages every other offered slot by one and resets the picked slot's age | observed | 004560d0 | high | |
| CAMP-231 | A slot whose node's maximum age is 1000 or more only ever ages from 0 to 1 | observed | 004560d0 | high | no retail node has such a value |
| CAMP-240 | Recording a win sets outcome 1 and the win ARES byte; a loss sets outcome 2, the loss ARES byte, and zeroes the blazon counter when the node is the blazon node | observed | 00456540 | high | |
| CAMP-241 | Losing or winning the blazon node when it is a defend node arms a one-shot notification the campaign-map screen reads and clears | observed | 00456540, 005280d0, 004577a0, 004577b0 | high | saved from version 41 (result) and 48 (node) |
| CAMP-250 | Mission-end bookkeeping is the six steps of 3.5 and changes no counter at all when the mission was lost | observed | 004e3260 | high | |
| CAMP-251 | Score gains 1000 for a won non-ambush mission, 70 per surviving special team member, 50 for one player-character act and 100 for an inventory pickup | observed | 004e3260, 004a5a00, 0049fc80 | high for the values, medium for what the 70 and the 50 are earned by | the two virtual tests were not followed |
| CAMP-252 | Every money and score change is also added to the level's mission-statistics block | observed | 00450b60, 004e3cc0 | high | that block feeds the debriefing |
| CAMP-260 | The blazon node is the first offered assault or defend node; the others move to the second list, or are dropped when there is none | observed | 00455f00, 0054cbf0 | medium-high | |
| CAMP-261 | The four ways the blazon counter moves, as tabulated in 3.6 | observed | 005795c0, 00514730, 00525040 | high | |
| CAMP-262 | The reported blazon requirement is needed − held for the blazon node and needed for the current node | observed | 00455af0, 00579690 | high | |
| CAMP-263 | Men convert to blazons in groups of `men_per_blazon`, capped by the outstanding requirement; a zero divisor is an asserted error | observed | 00455a70 | high | |
| CAMP-270 | Progress is 100 with one particular story node won, else 95 with one particular defend node won, else won × 100 / total over nodes of kind other than 3 and 6 | observed | 00456db0 | high | the two codes are compatibility tokens |
| CAMP-271 | Play time accumulates as (tick now − tick at level start) / 1000 seconds at the end of each level | observed | 0050e800 | high | |
| CAMP-272 | One display path adds unscaled milliseconds to the same seconds value | observed | 0050e7d0 | high | a unit bug in the original; a version / debug line only |
| CAMP-280 | No campaign routine branches on difficulty | observed | scope read | medium | the presets act elsewhere |
| CAMP-290 | A recruit is, with probability one half and if anyone is recovering, a recovered man with re-rolled health, else a newly generated man from a non-hero profile | observed | 004524b0, 00455bf0 | medium | the generation draw is folded |
| CAMP-291 | After a won mission the campaign adds as many men as the level's recruit count, one draw each | observed | 004e3260 | high | |
| CAMP-300 | At every level start in campaign mode the engine writes the campaign backup, makes the *Restart* auto-save, and, in the camp, sends message 1001 and runs production | observed | 0050f710 | medium-high | the campaign-mode test itself is folded (section 9) |
| CAMP-301 | Message 1001 is engine-originated, delivered once at camp start, before the first tick | observed | 0050f710, 00578d80 | high | settles the open question of `sherwood-hub.md` 5 |
| CAMP-302 | Message 1000 is delivered by the interface's send action | inferred | 0050f710 does not send it; the camp handler exists | low | the raising site was not found |
| CAMP-310 | Native 165 appends the man to the team if absent, marks him present and clears two of his fields; 166 removes him; both refuse non-player-characters and refresh the HUD | observed | 00579210, 00579280, 00455400, 004555f0 | high | fills the six missing native rows |
| CAMP-311 | Native 174 errors and answers 0 unless the current node's location is 8 | observed | 00579300 | high | |
| CAMP-312 | Native 170 requires every required character profile of the node to be the profile of a team member | observed | 00453cd0, 00457480, 004574b0 | medium-high | the two per-entry helpers were not read in full |
| CAMP-313 | A team member's camp slot survives only while the current node's location is 8 | observed | 004552f0 | medium | |
| CAMP-320 | Native 199 sets a workshop's zone and capacity and back-links the location; native 200 appends a work spot with coordinates and a sector reference | observed | 005799d0, 00579a00, 00580330, 00580220, 00456a10 | high | |
| CAMP-321 | Production runs once per camp entry over all 13 workshops in kind order and only when the previous mission was won | observed | 00456a40, 00580d20 | high | |
| CAMP-322 | Kinds 0..8 add their output to stock and then place up to five units per work spot | observed | 00581020, 00580b70 | medium-high | |
| CAMP-323 | Kinds 9 and 10 add their output to a global store at index 1 and 0 | observed | 005810c0, 0051d4e0 | medium | what the store is was not read |
| CAMP-324 | Kind 11 applies its output to every assigned man (training) | observed | 00581180, 00484360 | medium | |
| CAMP-325 | The item id per workshop kind is 1, 4, 5, 11, 12, 13, 16, 17, 19 for kinds 0..8 and none above | observed | 00580350 | high | nine individual facts, not a data table |
| CAMP-326 | Stock is recounted from the camp level before production by summing the quantities of matching pick-up items | observed | 00580430 | high | |
| CAMP-327 | Each workshop kind has a supervising hero by campaign identity: 0/1/9 hero, 2 Scarlet, 3/7 Stutely, 4/5/8 Tuck, 6 Marian, 10 John, 11/12 none | observed | 005806c0 | high | matched by designer name inside the binary; the names are not reproduced |
| CAMP-328 | After production every assigned man is placed at his work spot | observed | 00580dc0 | high | |
| CAMP-330 | The campaign map lays out ten location slots, skipping 0 and 8, with one widget fewer for locations 1..3 | observed | 00527d20 | medium | presentation detail |
| CAMP-331 | An empty offered list on the campaign map produces a text and a reported warning | observed | 00527d20 | high | |
| CAMP-332 | The campaign-map status line carries money, score and spared percent from text resources 0xF3, 0xF4 and 0x40 | observed | 00528810 | high | |
| CAMP-340 | The two HUD counters are money and the clover counter, each with its own text resource | observed | 004c8140 | high | agrees with `ui-flow.md` 9.3 |
| CAMP-341 | The blazon display refresh reports an error and does nothing outside a campaign | observed | 00514730 | high | |
| CAMP-350 | The default mini-map colour is per element class: player character light green, hostile human red (violet under one controller flag), plus a yellow and a blue state colour, and two further classes cyan and pale yellow | observed | 0048f280, 004a5c30, 0046ffe0, 004b1eb0, 005ef320 | high for the values, medium for which state uses which | |
| CAMP-351 | Native 24 accepts 0, 1, 100-102, 111, 200-202, 222, 300-302, 333, 500 and 666; 111, 222 and 333 require a human element; 500 and 666 change no colour; anything else is an error with no change | observed | 005788a0 | high | refines `spec-script-vm.md` |
| CAMP-352 | No grey dot colour exists in the image | observed | every call site of 005ef320 | high | an apparent grey dot is an undrawn dot or the map background |

---

## 5. Constants

| Name (ours) | Value | Unit | Source | Confidence |
|---|---|---|---|---|
| save magic | the four characters `GSHR` | — | 0056ea70 | high |
| profile archive magic | the four characters `FORP` | — | 0055dea0 | high |
| profile archive version | 6 | — | 0055dea0 | high |
| archive version of the retail build | 48 | — | 0056ea70, retail files | high |
| archive version at which the second campaign block appears | 39 (the test is "greater than 38") | — | 00511340 | high |
| archive version at which the recent-mission list appears | 30 (the test is "greater than 29") | — | 00452f20 | high |
| archive version at which the town result appears | 41 | — | 00452f20 | high |
| archive version at which the town node appears | 48 | — | 00452f20 | high |
| archive version at which the recruit flag appears | 28 (the test is "greater than 27") | — | 00452f20 | high |
| archive version at which a workshop assignment gains its u16 | 47 | — | 00580ab0 | medium |
| class signature length | 16 | bytes | 005e1b70, 0060f1e0 | high |
| campaign counters | 27 | i32 slots | 00452f20 | high |
| script campaign value window | counters 7..26, script indices 0..19 | — | 00579430 | high |
| new-campaign money | 100 | currency | 00450ec0, retail `Campaign.bck` | high |
| character profile index of the starting man | 1 | index | 00450ec0 | high |
| no-maximum sentinel in the money window | 200000 | currency | 0054c800 | high |
| default team size limit when nothing is selected | 5 | men | 00579300 | high |
| workshops | 13 | records | 00456a40, retail file | high |
| units placed per work spot per day | 5 | items | 00580b70 | medium-high |
| recent-mission memory | 3 | missions | 004539f0 | high |
| reduction attempts before forcing | 10 | rounds | 00451b70 | high |
| random filter modulus | 101 | — | 00452040, 00452090 | high |
| score for a won non-ambush mission | 1000 | points | 004e3260 | high |
| score per surviving special team member | 70 | points | 004e3260 | high |
| score for one player-character act | 50 | points | 004a5a00 | high |
| score for one inventory pickup | 100 | points | 0049fc80 | high |
| progress when the final story node is won | 100 | percent | 00456db0 | high |
| progress when the last defend node is won | 95 | percent | 00456db0 | high |
| clover ceiling for one actor action | 9 (the action is refused above it) | count | 004a78c0 | medium |
| health clamp for a recovered man | 100 | points | 00455bf0 | high |
| name generation: first-name text ids | 100..121 | resource id | 0055bf00 | high |
| name generation: second-name text ids | 122..143 | resource id | 0055bf00 | high |
| named-hero name text ids | 0x90 + position, positions 0..6 | resource id | 0055bf00 | high |
| campaign-map status line text ids | 0xF3, 0xF4, 0x40 | resource id | 00528810 | high |
| HUD counter text ids | 0xF4 (money), 0xF5 (clover) | resource id | 004c8140 | high |
| *Continue* / *Restart* label text ids | 0xF6, 0xF7 | resource id | 0050f1a0, 0050f2d0 | high |
| game-length label text id | 0x50 | resource id | 0054d550 | high |
| player-character mini-map colour | RGB (165, 255, 82) | colour | 0048f280 | high |
| hostile-human mini-map colour | RGB (255, 0, 0) | colour | 004a5c30 | high |
| hostile-human alternate colour under one controller flag | RGB (115, 40, 203) | colour | 004a5c30 | high |
| hostile-human second and third state colours | RGB (255, 255, 0) and RGB (0, 120, 255) | colour | 004a5c30 | high |
| mini-map colour of the cyan element class | RGB (0, 190, 255) | colour | 0046ffe0, 005788a0 | high |
| mini-map colour of the pale-yellow element class | RGB (255, 247, 90) | colour | 004b1eb0 | high |
| mini-map background | RGB (0, 30, 60) | colour | 005ef320 call sites | medium |
| workshop item ids for kinds 0..8 | 1, 4, 5, 11, 12, 13, 16, 17, 19 | item id | 00580350 | high |
| slot names | `Continue`, `Restart`, `QuickSave`, `ExQuickSave`, `Sherwood`; numbered slots `Savegame_%03i` | compatibility tokens, marked individually | 0056ef30, 0056f090, 0056f1f0, 0056f350, 0056f4b0, 0056fa30 | high |
| thumbnail name suffix | `_t` | compatibility token, marked | 0056e6b0, retail files | high |
| campaign backup file name | `Campaign.bck` in the installation root | compatibility token, marked | 00457560 | high |
| profile directory pattern | `Profile_%03i` under the save directory | compatibility token, marked | 0055d1a0 area | high |
| progress special-case level codes | the two-letter codes `HI` and `DD` | compatibility tokens, marked individually | 00456db0 | high |

---

## 6. Interfaces to the script VM

Only the natives this subsystem owns. Arity, coercion and the call protocol are `spec-script-vm.md`'s; what is
new here is the meaning. Rows marked **new** had no row in the engine before.

| Id | Args | Returns | Meaning and side effects | Failure |
|---|---|---|---|---|
| 163 | – | int | length of `team` | 0 outside a campaign |
| 164 | `i` | handle | the element of `team[i]`; no bound check | – |
| 165 **new** | `pc` | – | if `pc` is not already in `team`, append his status record; mark him present, clear two of his fields, refresh the HUD | script error and no change when the argument is not a player character |
| 166 **new** | `pc` | – | remove him from `team`; refresh the HUD | as 165 |
| 170 **new** | – | bool | 1 when every required character profile of the *selected* node is the profile of some team member (and the node's second requirement list is likewise satisfied) | reads through a null selected node |
| 172 | – | int | the four-character code of the selected node, 0 when none | – |
| 173 | – | bool | one byte of the game object, not of the campaign | – |
| 174 **new** | – | int | the selected node's team limit, 5 when none is selected | script error and 0 unless the current node's location is 8 |
| 178 | `banner` | – | deactivate the banner; `blazons += its value`; in an assault node, win the mission once `blazons >= blazons_needed`; refresh the HUD | error when the banner is null or already inactive |
| 195 | `k` | int | counter `k + 7` for `k` in 0..19 | script error and 0 outside the range |
| 196 | `k, v` | – | write counter `k + 7` | as 195 |
| 199 | `k, loc, n` | – | workshop `k`: zone `loc`, capacity `n`; back-link the location to the kind | no range check on `k` |
| 200 | `k, loc` | – | append a work spot to workshop `k` from the location's coordinates, one identifier and its sector index (0xFFFF when it has none) | – |
| 215 | `k` | handle | as `spec-script-vm.md` | – |
| 232 | `pc` | – | add `pc` to the band | error when not a player character or already present |
| 234 | – | bool | `blazons >=` the reported requirement of 3.6 | 0 outside a campaign |
| 236 | – | int | counter 1 | −1 with an error outside a campaign |
| 237 | `v` | – | write counter 1; plays a sound when the value grows and refreshes the HUD | as 236 |
| 239 **new** | – | – | open the debriefing screen (a named interface resource) | – |
| 249 **new** | – | int | the number of currently *selected* player characters — a HUD selection count, **not** a campaign field | – |
| 250 | `i` | handle | the i-th selected player character | error and null when `i` is at or above the count |
| 256 | `pc` | int | the campaign identity of `pc` by his profile's designer name: 0 hero, 1 John, 2 Tuck, 3 Stutely, 4 Scarlet, 5 Marian, 6..8 the three generic followers | −1 with an error for anything else |
| 261 | – | bool | the campaign's recruit flag | – |

**Engine-originated messages to the camp level.**

| Message | When | Meaning |
|---|---|---|
| 1001 | once, at camp level start, before the first tick, only in campaign mode | "configure your production zones"; the script answers with natives 199 and 200 |
| 1000 | when the player uses the send action in the camp | "send the team out"; the script walks the selected characters to the exit |

---

## 7. Acceptance tests

**Format tests, all runnable against the player's own installation** (all four were run in this session and
pass; the probes live in the analyst workspace and are one-off derivations, so the tests below re-derive the
numbers from the files):

| Id | Input | Expected |
|---|---|---|
| A1 | `Configuration/profile.cpf` parsed with the level-record grammar of 2.2 | the file is consumed to the exact byte; 63 level records; record 0 has kind 4 and location 8 |
| A2 | `Campaign.bck` | parses as `u32 48` + the campaign block; 27 counters with only counter 1 non-zero and equal to 100; ARES = −1; 63 mission slots whose node indices are 0..62 in order; all outcomes 0; the four node references are none, slot 21, none, none; one man in `people` with health 100, profile index 1 and the name of the hero; `band` = [0]; `team` = [0]; `recovering` empty; 13 workshops with kinds 0..12 and empty assignment lists; `recent` = [21]; town result 0 |
| A3 | `Savegame/Profile_001/Continue` and `Restart` | header `GSHR`, version 48, level code of the first story node, version 48; the 2713 bytes at offset 16 are byte-identical to `Campaign.bck` minus its first four bytes; a second campaign block starts at offset 2729 and differs from the first only in counter 6 and in the two discarded u16 of every mission slot |
| A4 | `Savegame/Profiles` | magic `FORP`, version 6, one profile, consumed to the exact byte; money 100, score 0, progress 0, play time 12 s; two key sets of 29 codes; graphic resolution 1024 x 768; two save slots named `Restart` / `Restart_t` and `Continue` / `Continue_t` |
| A5 | `Configuration/keyset1.cfg`, `keyset2.cfg` | 76 bytes each: 16-byte signature, set id 2 and 3, 29 u16 codes |

**Behaviour tests** (synthetic, no game data needed):

| Id | Input | Expected |
|---|---|---|
| B1 | a node with `min_money` 5000 and money 4999 | not accessible; the reported reason is the minimum-money one |
| B2 | a node with `max_money` 200000 and money 10^9 | accessible (the sentinel) |
| B3 | a node whose `after` list names a node with outcome 2 (lost) | accessible — a lost prerequisite satisfies the list |
| B4 | a node whose `until` list names itself, played | never accessible again |
| B5 | ARES −1 and a node with gate byte 0 = 1 and all other gate bytes 0 | accessible |
| B6 | ARES 0 and the same node | not accessible |
| B7 | two accessible nodes with the same location, priorities 1 and 2 | only the priority-1 node is offered; the other's age is reset to 0 |
| B8 | one accessible obligatory node and two others | only the obligatory node is offered |
| B9 | a story node with `max_age` 6 and age 5 | it alone is offered |
| B10 | a defend node with `max_age` 3, age 3, outcome 0 | dropped from the list **and** marked lost; its ARES loss byte is applied |
| B11 | the last three played missions all still accessible | none of them is offered |
| B12 | the reduction leaves the list empty ten times | the forced-mission path runs and produces exactly one mission |
| B13 | exactly one offer whose node has `needs_camp` 0 | that node becomes current; the camp is not entered |
| B14 | exactly one offer whose node has `needs_camp` 1 | node 0 (the camp) becomes current |
| B15 | picking an offer in the camp | the picked slot's age is 0 and it leaves the offered list; every other offered slot's age grew by one |
| B16 | a won mission of kind 0 with 3 survivors and 1 casualty, one of the survivors special | returned += 3, lost += 1, score += 1000 + 70 |
| B17 | the same mission lost | no counter changes at all |
| B18 | returned 3, lost 1 | spared lives = 75 |
| B19 | returned 0, lost 0 | spared lives = 0 |
| B20 | 20 non-ambush, non-tactical nodes, 7 won, none of them the two special codes | progress = 35 |
| B21 | the special defend node won and nothing else | progress = 95 |
| B22 | the final story node won | progress = 100 |
| B23 | buying a blazon for a node with price 1500 and step 500, twice | money falls by 1500 then 2000; blazons = 2; the slot's price is 2500 |
| B24 | an assault node with `blazons_needed` 12, banner values summing to 12 | the mission is won on the banner that crosses the threshold |
| B25 | a defend node with `blazons_needed` 3 and blazons 3, resolved from the camp | blazons = 0, the node is won and leaves the offered list, and the town notification is armed with result 1 |
| B26 | `men_per_blazon` 3, team 8, outstanding requirement 2 | the convertible number of men is 6 |
| B27 | `men_per_blazon` 0 | the implementation must refuse rather than divide |
| B28 | a save written and reloaded | the campaign compares equal field by field, with the two discarded u16 of every mission slot ignored |
| B29 | a save whose block A differs from block B | after loading, the campaign equals block B |
| B30 | native 174 with the current node's location not 8 | 0 and a recorded script error |
| B31 | production with the previous mission lost | nothing changes in any workshop |
| B32 | production of a kind-0..8 workshop with output 7 and two work spots | stock rises by 7 and at most 5 units are placed per spot |

**Oracle procedures.** The play-time accumulation (CAMP-271) and the *Restart* / *Sherwood* write points
(CAMP-131, CAMP-132) are checkable against a recording made from
`C:\Users\przem\source\gamedata\robinhood_oracle`: start a mission, note that `Restart` and `Restart_t` appear
before the first frame; enter the camp, note that `Sherwood` and `Sherwood_t` appear; quit and compare the
profile's play time with the wall-clock time to the second. Tolerance: 1 s.

---

## 8. Implementation choices, snapshot and departure contract

**What is the original's behaviour** — everything in sections 2, 3, 5 and 6 unless listed below.

**OpenSherwood decisions.**

1. *The 16-byte class signatures.* Ours are not computed; they are read from the player's own `Campaign.bck` and
   `Profiles` on first run and cached, or supplied by the format layer. We never publish them. A save we write
   must carry the same bytes in the same places for the original to accept it; a save we read must not be
   refused for a signature mismatch, because the original only warns (CAMP-002).
2. *The two discarded u16 per mission slot.* We write zeros. This is a deliberate deviation from the original,
   which writes uninitialised memory; it makes our saves reproducible and byte-stable, which the harness needs.
   Round-trip comparisons must mask those four bytes per slot (CAMP-022).
3. *The unit bug in the live play-time display* (CAMP-272) is not reproduced; we show seconds.
4. *The unattributed 16-byte base block of a character-status record* (CAMP-070) is carried through verbatim as
   `unknown_base` and is part of the snapshot. We neither interpret nor regenerate it; a new man gets the bytes
   the original writes for a new man, which an implementer must capture once from a retail save or default to
   zeros with the health field set.
5. *Randomness.* The original uses the C run-time generator for the two offer filters, the forced-mission draw,
   the recruit coin flip and the generated names. We use named seeded streams (`campaign.offer`,
   `campaign.force`, `campaign.recruit`, `campaign.name`), consumed in exactly the orders section 3 gives, so
   that replays are deterministic. We do not claim to match the original's sequence.
6. *Errors.* Where the original reports a condition and then continues (two obligatory nodes, two nodes with the
   same location and priority, a zero `men_per_blazon`, an empty offered list, a failed reduction), we record a
   diagnostic and take the same continuation; we do not abort.

**Snapshot contract.** Everything in 2.1 is authoritative and belongs in `snapshot()`: the 27 counters, `ares`,
`recruit_flag`, the four node references as slot indices, every mission slot (node, age, blazon price, outcome),
the `people` list in order with every field of 2.3, `band`, `recovering`, `team`, `offered`, `pending`, `recent`
as ordered index lists, the 13 workshop records with their assignment lists in order, `town_result` and
`town_node`. Also authoritative, though not in the original's campaign block: the identity of the current
player profile and the save-slot list, because a save the player can reload depends on them. The RNG stream
positions of the five named streams are authoritative. Not authoritative: the offered list's *reduction* working
copy, the work-spot lists of 2.4, the level's mission-statistics block while no mission is running, and every
derived display value (progress, spared lives, the blazon requirement).

**Departure contract.** The canonical hash covers the snapshot above. A change to any of the following is a
ruleset bump: the availability test of 3.2, the reduction order of 3.3.1, the aging rule of 3.4, the outcome and
mission-end rules of 3.5, the blazon rules of 3.6, the score, progress and spared-lives formulas of 3.7, the
recruit rule of 3.8, or the RNG consumption order anywhere in section 3. A change to the file grammars of 2.6 is
a format-version bump and must keep reading every earlier archive version listed in section 5.

---

## 9. Open questions

1. **How does the clover counter (counter 0) ever become non-zero?** No write through the counter accessors
   exists, and native 196 cannot reach index 0. Look at 004a78c0's caller chain and at the pick-up handling of
   the item whose action id is 14, and at 0050b640 near the campaign-counter call whose arguments the
   decompiler dropped (CAMP-016).
2. **The one-day output of a workshop.** 00581020, 005810c0 and 00581180 each truncate a floating-point value to
   an integer; the expression that produces it, and the part 005806c0's supervising-hero lookup plays in it, were
   folded away by the decompiler. Read the raw instructions of 00581020 (0x94 bytes), 005810c0 and 005806c0's
   tail; also 00580e80 for workshop kind 12 and 0051d4e0 for the global store of kinds 9 and 10.
3. **Which of the two 32-bit profile values is the difficulty**, and how a preset reaches the combat rules.
   Look at readers of the player profile at +0x18 and +0x1c and at the four-record table of `profile.cpf`.
4. **Where `team_limit` comes from.** Native 174 answers a level-record field that `profile.cpf` does not
   contain. Candidates: the `SCOT` count of the node's `.rhm`, or a field of the mission file copied in at load.
   Look at writers of the level record at +0x72 and at 00568ba0 (the forced-mission-profile path).
5. **The two random offer filters** (CAMP-212): which side of the comparison is kept. The container code around
   00452810 and 00452a50 was folded; read their raw instructions.
6. **Who raises message 1000** (CAMP-302). Search the interface code for the send action of the camp
   (`ui-flow.md`'s `BTTN` 160) and for callers of the message path 00578d80 outside the script natives.
7. **The eight unattributed bytes of a character-status record** (CAMP-070). The base serializer at 0051d5d0
   writes only two 32-bit values, yet sixteen bytes separate the two class signatures. Re-read 0055c3c0's head
   and 0051d5d0's raw instructions.
8. **What the two unused u16 of a level record (`m0`, `m1`) do.** No reader was found in the scope read. They
   vary per story node in the retail data, which suggests a briefing or difficulty use; search for readers of
   the level record at +0x64 and +0x66 outside the profile manager.
9. **What earns the 70-point bonus and the 50-point act** (CAMP-251): two virtual calls were not followed
   (004e3260's two indirect tests, 004a5a00's virtual slots 0x130 and 0x48).
10. **The campaign-mode test at the head of the game loop** (0050f710): the decompiler renders it as a test on
    the campaign's ARES byte, which would make the normal path depend on a value that is −1 at the start; the
    plausible reading is "a campaign object exists". This must be settled before the loop order of 3.9 is
    implemented.
11. **The exact semantics of the second offered list (`pending`)** and of the two lists that are empty in the
    retail data (`spare_refs`, `labels`). They are saved, so an importer must read them, but nothing in the
    retail data exercises them.
12. **Portrait states, action-icon availability and tooltips** were not read; `ui-flow.md` has the geometry.
    Look at 005145b0, 00514670 and the portrait widget of the HUD module.

---

## 10. Differences from the current engine

Concrete, against `crates/opensherwood-app/src/{engine.rs,ui.rs}`, `docs/formats/profile.md`,
`docs/formats/savegame.md`, `docs/formats/sherwood-hub.md` and `docs/original/campaign-flow.md` as they stand:

1. **The successor rule is wrong.** The engine's rule is a successor relation over profiles; the original's is
   the eight-condition availability test of 3.2 plus the seven-step reduction of 3.3.1 plus the age counter.
   Nothing in the engine has a money window, a band-size window, an age limit, an ARES gate, a per-location
   priority, a recency filter or the two random filters.
2. **The *after* list is not "won", it is "played".** A lost prerequisite opens its successors. The engine's
   reading (and `profile.md`'s) is stricter than the original.
3. **`profile.md`'s level-record grammar is wrong in three places** (CAMP-041, CAMP-042, CAMP-043): a field
   group is mis-split, a count is u16 instead of u32, and a byte block is 12 instead of 10. The parser in
   `opensherwood-formats::cpf` consumes the retail file anyway because the errors cancel; a strict reader that
   trusts the field names will mis-read `unknown_e`, the required-character list and `unknown_l`.
4. **The required-character list is a real feature.** It is a list of character-profile indices and it gates
   sending the team (native 170). The engine has no notion of it.
5. **`savegame.md` is a wrong guess.** There is no "table of 32-byte records each containing the same 16-byte
   hash, a u32 index and a u32 value" in the sense described: the 32-byte records are mission slots with age,
   blazon price and outcome, the 16 bytes are a class signature, and the file contains the campaign block
   **twice**, the first copy byte-identical to `Campaign.bck`. `Campaign.bck` is not "a backup of the same
   table"; it is the campaign as of the last level start and is the source of the save's first block.
6. **Nothing in the engine implements the blazon economy.** Blazons are earned in assault and tactical
   missions, bought for money at a price that rises per purchase, spent to resolve defend missions, and reset
   when the blazon node is lost. Natives 178, 223 and 234 are stubs against a counter that no rule moves.
7. **The six natives `sherwood-hub.md` lists as "no row" now have rows** (165, 166, 170, 174, 239, 249), and
   two of the stub policy values are wrong: 174 must be the selected node's team limit (5 only when nothing is
   selected) and 249 is the *HUD selection* count, not a campaign number — `STUB_POLICY_VALUES` pins 174 to 5
   and 249 to 0 unconditionally.
8. **Message 1001 is engine-originated at camp start and message 1000 comes from the send action**; the engine
   sends neither, so the camp's 13 production zones are never configured and no workshop ever produces.
9. **Production only runs after a won mission**, over 13 workshops in kind order, with a per-kind item id and a
   per-kind supervising hero. The engine has no production at all.
10. **The score, progress and spared-lives formulas differ from anything in the engine.** Progress is not
    "missions done / missions total": ambushes and tactical nodes are excluded, placeholders are included, and
    two particular level codes short-circuit to 95 and 100. Spared lives is a ratio of two cumulative counters,
    not a count. `campaign-flow.md`'s "Progress 0 %" for a fresh profile is consistent, but the engine's
    `profiles.json` has no field for play time, spared lives, blazons or the ARES state, so a profile written
    by the engine cannot round-trip through the original.

Two further differences worth naming: the engine's `profiles.json` is not the original's `Savegame/Profiles`
(different container, different fields, no key or graphic configuration, no save-slot list), and the engine has
no auto-save at all, whereas the original writes *Restart* at every mission start, *Sherwood* at every camp
entry and mirrors every save into *Continue*.

---

## 11. Identity block

- **Analyst:** this session (Claude analyst agent, 2026-09-13), working in the git-ignored `re/` workspace on the
  maintainer's lawfully acquired copy. Exposure: full decompilation of the functions listed in section 0
  ("scope read"), plus `re/out/inventory.tsv`, `re/out/strings.tsv` and `re/notes/modules.txt`.
- **Exposure record for the wall (ADR-0009 §2).** This session has read decompilation for: campaign flow, the
  Sherwood camp, camp production, saved games, the player profile and configuration files, the campaign-map
  screen, and the mini-map dot colours. It must not implement any of them, and no implementer session may
  inherit its context, notes or tool output.
- **Spec reviewer:** pending (Codex, task B of `cross-agent-review`).
- **Publication approval:** pending, maintainer, separate from factual approval.
- **Gate:** `python scripts/check_no_assets.py --paths docs/original/spec-campaign-camp-saves.md` run before
  saving.

---

## 12. Provenance

Ghidra project `re/ghidra/robinhood` (never committed), full-function decompilation under `re/out/decomp_all/`
exported by `scripts/ghidra/DecompileAll.java`, the function inventory and string cross-reference by
`scripts/ghidra/ExportInventory.java` and `ExportStrings.java`, and the module map in `re/notes/modules.txt`
derived from assertion strings. Byte-level checks used `scripts/ghidra/peek.py` and three throw-away parsers in
`re/notes/campaign/` (a `Campaign.bck` / save reader, a level-table reader and a `Savegame/Profiles` reader);
those parsers are one-off derivations of the numbers in section 7 and are not needed to check the claims — the
acceptance tests A1..A5 re-derive them from the player's files.

Files read on disk (read-only, `C:/Users/przem/source/gamedata/robinhood`): `Campaign.bck`,
`DATA/Configuration/profile.cpf`, `DATA/Configuration/keyset1.cfg`, `keyset2.cfg`,
`Data/Savegame/Profiles`, `Data/Savegame/Profile_001/{Continue,Continue_t,Restart,Restart_t}`.

No oracle recording was made for this specification; the two timing claims that would need one are named in
section 7.

Function addresses are given inline throughout sections 2 to 6 and collected in section 0. Instruction bytes,
disassembly, recovered symbols and the comprehensive address map stay in `re/`.

Build: GOG English edition, `Robin Hood.exe` SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`.

Documents that must be updated once this specification is reviewed: `docs/formats/savegame.md` (replace the
stub), `docs/formats/profile.md` (the three grammar corrections and the field meanings),
`docs/formats/sherwood-hub.md` (sections 5 and 6: the six natives, messages 1000/1001, production),
`docs/original/campaign-flow.md` (the successor rule), `docs/roadmap.md` and the `Assumption` registry.
