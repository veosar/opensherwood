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
| | ~2.6 | mission `The Godfather` is loaded, shown green-tinted and paused, **briefing page 1** on a parchment | `m1_brief_page1.png` |
| V seal (508,552) or Enter | | briefing page 2 | `m1_brief_page2.png` |
| V seal / Enter | | briefing page 3 | `m1_brief_page3.png` |
| V seal / Enter | | parchment closes, game unpauses, camera on Robin outside the gate of Lincoln castle; HUD shows Money L100, Clover 0, Robin's portrait | `m1_brief_page1.png` (background), `pause_menu.png` |

There is **no intro video, no cutscene, no campaign map and no Sherwood camp** before the first mission; the
manual (p.9, "Planning a campaign") confirms: "The first mission is played with Robin Hood on his own. Once
it's completed, the second mission will launch automatically. When the second mission is completed, you will be
able to enter Sherwood Forest." An earlier run with the shipped profile (`Shadaia`, Progress 0 %, auto-saves
`Continue` / `Restart` present) went from Play! straight into the same mission as well ("Mission loaded!"
message in the lower middle of the screen, white outlined text at about (512,620)).

### The first mission

- Title: **"The Godfather"** = `2047/data/Text/Level.res` TEXT id **1000017** (strings: "The Godfather",
  "NOTUSED", "NOTUSED"). It is the first id in `DATA/Text/RHLevelHA.red`, so the level is **HA** =
  `DATA/Levels/H01_Lin_VL.rhm` (Lincoln, proto-level `Lincoln`), day ambiance (*inferred* from the file names and
  the first `.red` value; the mission file was not verified by loading it).
- Player characters: Robin Hood only. Start position: in front of the castle gate on a muddy yard with a pig
  sty and a barn to the left, two guards on the wall above the gate, a mendicant to the right.
- Objective shown in the pause menu (short briefing, `Level.res` 1000283 string 0): "Robin must get into the
  castle to find Godwin." The other short briefings of the mission (1000283 strings 1..5) are updated
  objectives: Edward's son awaits Robin in the village to help him leave the town; Edward's brave son will take
  Robin to a safe place as soon as he tells him he wants to leave; Master Worman has received some money from the
  Sheriff; Haldric the knight has been bribed by the Sheriff, why not rob him; the bowmen are far too clumsy - by
  hitting one of the targets with an arrow Robin could put an end to the training session.
- Briefing pages (TEXT 1000105 strings 0..2), verbatim in the game:
  0. "Young Robin of Locksley left his cherished land some years ago to follow the valiant King Richard the Lion
     Heart in the Crusades. Now, finally, he has returned, but it seems that his fighting days are far from over.
     In fact, King Richard has yet to return from the Holy Land. In his absence, power is in the hands of Prince
     John Lackland, his brother. But the vassals do not seem to respect his authority: the Sheriff of Nottingham,
     in particular, is using his position to tax the country folk so heavily that they are reduced to the most
     abject poverty."
  1. "Even worse: Robin learns that his father, Lord Locksley, died of old age while he was away. And the Sheriff
     took advantage of this to claim that Robin had also met his death - struck down by the infidels - and
     confiscate his lands in the name of the Crown! Now Robin has been disinherited, robbed of his estate by this
     loutish Sheriff's embezzling. Our hero is left with but one solution: he must go to Lincoln."
  2. "Lincoln! It is here that Robin spent his youth, with an old friend of his father's, Lord Godwin. It was
     under his guidance that Robin learnt to fight and use a bow and arrow: what better ally could he find in such
     circumstances! But as our hero enters his Godfather's castle, his face fortunately concealed by his hood, a
     painful surprise awaits him: the castle is teeming with men bearing the arms of the Sheriff of Nottingham! We
     must find Godwin... and hope that the Sheriff's men do not notice Robin!"
  Pages 1..3 show a different 120x160 character picture each (Robin in green, a man in a purple tunic, ...).
- The remaining strings of 1000105 (3..22) are the in-mission dialogue and tutorial popups: the clover charm
  (string 3), Master Worman's purses (4), the archery trainer (5, 6), Haldric the knight (7), Edward the servant
  (8..10), Edward's son (11, ends with "(To leave Lincoln, click on the top right icon)"), and the tutorial hints
  12..22: ivy climbing, the jump arrow, knocking out (icon + click, stars over the victim's head show the
  remaining time), throwing a purse, paying beggars for information, picking up a purse, Edward, the forgotten
  purse, cutting the drawbridge rope, combat (click / double click on the adversary, click on Robin to parry, hold
  the button and move to draw attacks, "draw a horizontal figure of 8" against strong enemies), and the bow
  ("click on the arrow drawn above his picture, then aim... when the cursor arrow turns green, click").
- `RHLevelHA.red` (u32 values): `1000017, 1000007, 0, 23, 1000105, 1000103, 1000106, 1000001, 1000107, 1000083,
  1000108, 1000108, 1000109, 1000110, 1000110, 1000089, 1000111, 1000112, 1000113, 1000114, 1000115, 1000116,
  1000117, 1000118, 1000117, 1000119, 1000120, 1000121, 1, 1000349, 1, 1000350, 6, 1000283`: title, unused goal,
  0, then `23` and 23 ids (1000105 = the text list, the others are probably the `WAVE` voice entries of each
  string), `1` + 1000349 (debriefing when won), `1` + 1000350 (debriefing when lost), `6` + 1000283 (six short
  briefings). This refines the stub in `docs/formats/red.md` (*inferred* layout, consistent with all 57 files:
  every file ends with `n_won, id, n_lost, id, n_short, id`).
- Debriefings: won = 1000349 ("With the help of Edward's son, Robin managed to leave the estate of Lincoln, but
  his situation is far from brilliant: he was unable to find his godfather, and has nowhere to try and take
  refuge..."); lost = 1000350 ("Hell! Robin has been slain by the Sheriff's formidable soldiers! All we can do
  now is start another game...").
- **Tutorial**: there is no separate training mission in the flow; the first mission *is* the tutorial (popup
  hints above). `Levels/EmbTut_FoC_EC.rhm` ("Emb" = ambush, "Tut" = tutorial) exists in the data but is not
  reached from Play! with a fresh profile; when it is used is *unknown* (possibly the first ambush, see below).
  It is therefore not mandatory in the sense of a separate step.

## 2. After the first mission (*manual* p.9-14, `Level.res`, not observed)

1. Mission 2 launches automatically after the debriefing of mission 1. By the `.red` order the second story
   mission is **HB** = "Confessions of an Outlaw" (TEXT 1000018, briefing list 1000122 "Robin is back in
   Nottingham...", level `H02_Not_EC`). Mission titles of the H series in `Level.res` order: The Godfather (HA),
   Confessions of an Outlaw (HB), The Prince and the Outlaw (HC), The Evening Visitor (HD), The Godfather in
   Prison (HE), The Silver Arrow (HF), The Escape (HG), The Letter (HH), Last Challenge (HI); HQ = "Sherwood"
   (1000034, the camp, 37 texts starting "Here we are, Robin! Let's set up our headquarters here!"). S series
   (secondary): First Companions (SA), The Scarlet Night (SB), Pillaging (SC), The Lock-up and the Friar (SD), A
   Wedding and a Funeral (SE). A series (assaults): Free Lincoln (AA), The Black Castle (AB), The march on York
   (AC). D series: Defend Leicester / Lincoln / Derby / York. E series (ambushes, 1000042..): A Convoy!, The
   wealthy tradesman, A Carriage, Rookies, The Tax Collector, The Apprentice, The Treasure, The Captive Knight, The
   Sin of Greed, The Debt, Prevention is better..., The Tollgate, The Villainous Bandits, Debut, Let's go for a
   stroll in the woods..., Patrol!, Reinforcements, The Stragglers, The Scouts, Pillage, A Logistical Convoy,
   The Messenger, Scathlock's Secret, The Engineer, The Bard, The Press Gang (T series = tactical ambush variants).
   VI / VO = intro / outro (1000056 "into", 1000057 "outro").
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
   by renaming the file in a private copy).
2. What triggers `EmbTut_FoC_EC` (tutorial ambush) and the intro video (`2047/data/Cinematics/Intro.vid`,
   TEXT 1000056 "into"); is the video shown on the very first start of a fresh installation?
3. The debriefing screen after a mission (layout, statistics, buttons) and the automatic transition to mission 2.
4. The Sherwood camp screen and the campaign map: exact positions of the MAP / SEND icons, the team boxes, the
   red / blue seals; which `.red` order corresponds to the mission availability graph (`Campaign.bck`,
   `RHCampaign`).
5. Meaning of the second and third `.red` values (`1000007` = "NOTUSED" text; `0/1/2/3/4` then a variable block
   before the `23` count in the H/S files).
