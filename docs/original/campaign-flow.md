# Original game: campaign flow (black-box observation)

Status: **observed** for everything up to the start of the first mission; later stages are taken from the
printed manual and the text resources and are marked *manual* / *inferred*. Coordinates: 1024x768 logical
pixels, see `ui-flow.md`. Screenshot file names refer to `harness/captures/original/` (git-ignored).

## 1. From the main menu to the first mission

Precondition: a profile selected in **Select player** (a fresh one: Medium difficulty, Money L100, Score 0,
Progress 0 %). Then:

| Step | t (s after click) | What happens | File |
|---|---|---|---|
| click **Play!** (748,364) | 0 | main menu disappears | `play_000_000.9s.png` |
| | ~0.9-2.5 | **loading screen**: painting of Robin drawing his bow, progress bar y = 700, "v1.1" top right | `play_001_001.8s.png`, `play_002_002.2s.png` |
| | ~2.6 | mission 1 (level HA) is loaded, shown green-tinted and paused, **briefing page 1** on a parchment | `m1_brief_page1.png` |
| V seal (508,552) or Enter | | briefing page 2 | `m1_brief_page2.png` |
| V seal / Enter | | briefing page 3 | `m1_brief_page3.png` |
| V seal / Enter | | parchment closes, game unpauses, camera on Robin outside the gate of Lincoln castle; HUD shows Money L100, Clover 0, Robin's portrait | `m1_brief_page1.png` (background), `pause_menu.png` |

There is **no intro video, no cutscene, no campaign map and no Sherwood camp** before the first mission; the
manual (p.9, campaign planning section) confirms it: the first mission is played with Robin alone, the second
launches automatically after it, and Sherwood Forest becomes reachable only after the second. An earlier run
with the shipped profile (Progress 0 %, auto-saves `Continue` / `Restart` present) went from Play! straight
into the same mission as well (a short "mission loaded" style message in white outlined text at about
(512,620)).

### The first mission

- Title: `2047/data/Text/Level.res` TEXT id **1000017** string 0 (strings 1 and 2 are placeholders). It is
  the first id in `DATA/Text/RHLevelHA.red`, so the level is **HA** =
  `DATA/Levels/H01_Lin_VL.rhm` (Lincoln, proto-level `Lincoln`), day ambiance (*inferred* from the file names and
  the first `.red` value; the mission file was not verified by loading it).
- Player characters: Robin Hood only. Start position: in front of the castle gate on a muddy yard with a pig
  sty and a barn to the left, two guards on the wall above the gate, a mendicant to the right.
- Objective shown in the pause menu: `Level.res` 1000283 string 0 (Robin has to get into the castle and find
  his godfather). Strings 1..5 are the updated objectives that follow (leaving the town with the servant's son,
  the steward's money, the bribed knight, the archery training). Text is loaded from the player's files at run
  time; it is not reproduced here (`docs/legal.md`).
- Briefing pages: TEXT 1000105 strings 0..2 (three paragraphs of back story: Robin's return from the Crusades,
  the loss of his estate, his arrival at his godfather's castle in Lincoln). Pages 1..3 show a different 120x160
  character picture each (Robin in green, a man in a purple tunic, ...). String lengths 0..2: see
  `sres_text.py --lengths` on a local copy.
- The remaining strings of 1000105 (3..22) are the in-mission dialogue and tutorial popups: the clover charm
  (string 3), the steward's purses (4), the archery trainer (5, 6), the knight (7), the servant (8..10), the
  servant's son (11, ends with the instruction to leave the map through the top right icon), and the tutorial
  hints 12..22: ivy climbing, the jump arrow, knocking out (icon + click, stars over the victim's head show the
  remaining time), throwing a purse, paying beggars for information, picking up a purse, the forgotten purse,
  cutting the drawbridge rope, combat (click / double click on the adversary, click on Robin to parry, hold the
  button and move to draw attacks, a horizontal figure of eight against strong enemies), and the bow (click the
  arrow above the portrait, aim, click when the cursor turns green).
- `RHLevelHA.red` (u32 values): `1000017, 1000007, 0, 23, 1000105, 1000103, 1000106, 1000001, 1000107, 1000083,
  1000108, 1000108, 1000109, 1000110, 1000110, 1000089, 1000111, 1000112, 1000113, 1000114, 1000115, 1000116,
  1000117, 1000118, 1000117, 1000119, 1000120, 1000121, 1, 1000349, 1, 1000350, 6, 1000283`: title, unused goal,
  0, then `23` and 23 ids (1000105 = the text list, the others are probably the `WAVE` voice entries of each
  string), `1` + 1000349 (debriefing when won), `1` + 1000350 (debriefing when lost), `6` + 1000283 (six short
  briefings). This refines the stub in `docs/formats/red.md` (*inferred* layout, consistent with all 57 files:
  every file ends with `n_won, id, n_lost, id, n_short, id`).
- Debriefings: won = 1000349 (Robin leaves Lincoln with the servant's son but without finding his godfather),
  lost = 1000350 (Robin was slain; start another game).
- **Tutorial**: there is no separate training mission in the flow; the first mission *is* the tutorial (popup
  hints above). `Levels/EmbTut_FoC_EC.rhm` ("Emb" = ambush, "Tut" = tutorial) exists in the data but is not
  reached from Play! with a fresh profile; when it is used is *unknown* (possibly the first ambush, see below).
  It is therefore not mandatory in the sense of a separate step.

## 2. After the first mission (*manual* p.9-14, `Level.res`, not observed)

1. Mission 2 launches automatically after the debriefing of mission 1. By the `.red` order the second story
   mission is **HB** (title TEXT 1000018, briefing list 1000122, level `H02_Not_EC`, Nottingham). The H series
   (story missions HA..HI) titles are TEXT 1000017..1000025 in `.red` order; HQ = the Sherwood camp (title
   1000034, 37 texts). S series (secondary, 5 missions), A series (assaults, 3), D series (defences, 4), E series
   (ambushes, 1000042.., about 26 with T = tactical variants), VI / VO = intro / outro (1000056, 1000057). The
   titles themselves are not reproduced here; read them from `Level.res` with `harness/tools/original/sres_text.py`.
2. After mission 2 the **Sherwood Forest camp** (level `Sherwood.rhm`) becomes the hub: new recruits appear,
   workshops (arrows, feast, archery training, combat training) produce items between missions, hovering a
   character shows a small parchment with his sword / bow ability icons.
3. **Campaign map**: the "MAP" icon at the top right of the Sherwood screen opens a map of Sherwood and the
   surrounding towns (`DEFAULT.RES` PIC 123, 629x480, with town buttons 125..129 and troop arrows 227..236).
   Hovering a location shows a one-line description, clicking opens a detailed parchment (briefing + accept /
   close seals). After accepting, a red seal and a row of boxes (required team size, required abilities, forced
   portraits) appear at the top right of the Sherwood screen; the player gathers the team on the path leading out
   of the forest (top right), or clicks the "SEND" icon, then clicks the blue seal to start.
4. Missions: up to 5 characters; a message shows the objectives at the start and whenever they change (the
   Escape menu repeats the current ones); a message announces victory, after which a seal at the top right leaves
   the mission (secondary objectives can still be pursued); a mission is lost when a main character is killed; in
   ambush missions the seal is present from the start and clicking it retreats (mission lost, men survive).
5. After a mission the team returns to Sherwood; the campaign statistics (money, score, lives saved) appear on
   the map parchment ("Money: ... Score: ... Lives saved: ..." per the manual's screenshot on p.10).

## 3. The level table in `Configuration/profile.cpf` (*data*, 2026-09-02)

`profile.cpf` ([../formats/profile.md](../formats/profile.md)) has one record per level code with the
`.rhm` file, the map, the location index, the three music tracks and two lists of level codes read here as
"available after" and "removed after":

- `HA` = `H01_Lin_VL` on Lincoln (confirms the file mapping inferred above from the `.red` order; still not
  confirmed by loading), no prerequisite, removed by itself.
- `SA` = `S01_Not_VL` (Nottingham, "First Companions" in `Level.res`) needs `HA`; `HB` = `H02_Not_EC` needs
  `SA`. So the mission that "launches automatically" after the first one is **SA**, not HB, if the lists are
  prerequisites; `S01_Not_VL` has a single `SCOT` slot (Robin alone), consistent with the manual. Unverified.
- Story chain: `HC` needs `HB` + `SB`; `SC` needs `HC`; `HD` needs `SC`; `HE` needs `HD`; `SD` needs `HE` + `SC`;
  `HF` needs `SD`; `SE` needs `HF`; `EI` needs `SE`; `HG` needs `EI`; `HH` needs `HG`; `AC` needs `HH`; `DD`
  needs `AC`; `HI` needs `DD`. `SB` needs the tutorial ambush `EZ` = `EmbTut_FoC_EC`, which needs `SA`
  (answers open question 2: the tutorial ambush is reachable after `SA`). Ambushes `EA`, `EB`, `EC`, `EE`,
  `EF`, `EH` need `EZ` and are removed after `HH`; `ED` needs `HF`; `EG` needs `HE`. Tactical missions need
  `SD`. Defend / assault: `DB` needs `SD`, `DA` needs `DB`, `AA` needs `DA`, `AB` needs `DB`, `DC` needs `AB`,
  `DD` needs `AC`.
- Location index 1..9 = Croisement01..03, Derby, Leicester, Lincoln, Nottingham, Sherwood, York (the campaign
  map buttons, presumably).

## Provenance

Same session as `ui-flow.md` (build SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`,
GOG English, 1024x768, private copy `C:\Users\przem\source\gamedata\robinhood_oracle`, 2026-09-02). Observed:
the Play! -> loading -> briefing -> mission sequence with a fresh profile (`Analyst`, Medium) and with the shipped
profile. Texts: `python harness/tools/original/sres_text.py <gamedir>/2047/data/Text/Level.res 1000017 1000105
1000283 1000349 1000350`; `.red` values dumped as little-endian u32 with a one-line Python script. Manual:
`Manual.pdf` shipped with the GOG build (pages 9-14 of the printed numbering). Level-to-file mapping is inferred
from the `RHLevel??.red` names and the `H01_Lin_VL` naming scheme; it has not been confirmed by loading the file.

## Open questions

1. Confirm that `RHLevelHA.red` / `H01_Lin_VL.rhm` is what Play! loads (e.g. with the console `REPORT` command or
   by renaming the file in a private copy). The `profile.cpf` level table (section 3) maps `HA` to
   `H01_Lin_VL`, which makes this very likely but is still not a loading test.
2. What triggers `EmbTut_FoC_EC` (tutorial ambush) and the intro video (`2047/data/Cinematics/Intro.vid`,
   TEXT 1000056 "into"); is the video shown on the very first start of a fresh installation? Data: the
   tutorial ambush (`EZ`) lists `SA` as its prerequisite (section 3).
3. The debriefing screen after a mission (layout, statistics, buttons) and the automatic transition to mission 2.
4. The Sherwood camp screen and the campaign map: exact positions of the MAP / SEND icons, the team boxes, the
   red / blue seals; whether the availability graph is the `profile.cpf` code lists of section 3 (and what
   `Campaign.bck`, `RHCampaign` add to it).
5. Meaning of the second and third `.red` values (`1000007` = "NOTUSED" text; `0/1/2/3/4` then a variable block
   before the `23` count in the H/S files).
