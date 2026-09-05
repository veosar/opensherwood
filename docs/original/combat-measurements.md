# Combat measurements: the first mission's melee, bars, death and lost flow

Status: **black-box observation of the running original** (analyst session 2026-09-05, ADR-0003: no
disassembly, debugger or memory inspection; screen recordings of the private oracle copy, nothing
committed). Every claim carries a status: `observed` (measured on the recordings), `inferred`
(a reading that fits every measurement), `hypothesis`, `not measured`. Positions are client pixels of
the 1024x768 game window (= map pixels at the normal zoom, see stealth-and-combat.md 8.5); times are
the recorder's wall clock. Game text is never reproduced: pages, tooltips and numbers are described by
their layout and value. The method and its limits are in section 7; the sequence of the four questions
of the brief is kept.

Scene: `H01_Lin_VL` right after the briefing, hero Robin alone, medium difficulty (the preset the
New-player dialog preselects; the profile of the 2026-09-05 session), camera as the game places it.
The one soldier a walking hero can reach from the start position is a **halberdier** in mail standing
at the arch of the right-hand wall, feet at (932,347); every fight below is Robin against him. (The
two "idle courtyard lancers" of stealth-and-combat.md 8.4 are not soldiers: they are sparkle-animated
pickups - a scroll with a ribbon at (796,383) and a purse at (812,302) - whose animation steps every
93.75 ms; the walking soldiers of that scene are on the walls. The 93.75 ms idle step therefore
belongs to a pickup sparkle, not to a soldier idle. Correction for section 8.4.)

## 1. Melee (question 1)

### 1.1 Drawing the sword, the attack order (`observed`)

- There is no sword icon and no key: the portrait's action row holds three icons only (bow at
  (100,715), fist at (135,715), purse at (165,715), counters below them). The sword is drawn by the
  engine when the hero enters a fight.
- A left click on the enemy sprite with Robin selected is the attack order (the pointer over the
  enemy keeps its arrow shape with a red tail instead of the green one seen over open ground). A
  click on the ground next to the soldier (30 px short) is a plain walk order, and the soldier does
  not react to Robin standing 36 px beside him (three separate approaches; he faces the courtyard).
- Hovering the enemy already shows his two bars (5.2 s of recording e3, before the click at 5.4 s).
- Sequence after the click (recording e3, click at 5.44 s, Robin 140 px away): Robin walks up, stops
  about 52 px from the halberdier's feet, both circles turn red and both bars appear (Robin's at
  7.3 s), the halberdier shows the exclamation emoticon (8.3 s), the HUD portrait switches to the
  combat state (a red band with crossed swords over the name area, rows about 665..700). The same
  order given through the **fist icon** (icon click, then the enemy) produced exactly the same
  sequence: Robin walked up from the front, no punch was seen, the halberdier raised "!" as Robin
  arrived, and the sword fight began (recording e5). A knock-out from behind was not possible in this
  scene (no unaware soldier reachable), so the "tie-up / carry" prompt was not observed.

### 1.2 Health and energy bars (`observed`, confidence high)

Both fighters get two bars under the feet while in combat or hovered: a **red health row** and, 4 px
lower, a **blue energy row**, each **20 px wide** on a black background, 3 px tall. Pixel values:
health (255,0,0) when the sprite is under the pointer, (123,0,0) otherwise; energy (0,~200,255) /
(0,101,123); the spent part is (0,0,0). Robin's bars at x 872..891, rows 360..362 / 364..366 while his
feet were at (882,~352); the halberdier's at 924..943, rows 354..356 / 358..360.

- **One halberdier hit = "5" over Robin's head and 1 px off Robin's bar** (36 hits over three
  recordings, every one 1 px): Robin's bar is **100 hp at 5 hp per pixel**.
- The powerful blow that landed on the halberdier showed **"50"** and took his bar from 20 to **7 px**:
  20 px = his full health and 50 hp = 13 px, i.e. **4 hp per pixel = 80 hp**, the `pre[0]` value of a
  blue halberdier in profile.cpf (stealth-and-combat.md 2.3) - a direct confirmation of `pre[0]` =
  hit points (`inferred`, one data point; 12.5 px rounded to 13 removed, 7 shown).
- Health never regenerates: the halberdier stayed at 7 px for the remaining 92 s of e7 and Robin's bar
  only fell.
- **Energy**: a soldier's landed hit costs him 1 px of energy (20 -> 19) that comes back after about
  4 s; Robin's forward-stroke blow costs **2 px** (20 -> 18, first visible 1.2 s after the stroke)
  and regenerates **1 px per 0.8-1.0 s** (18 -> 19 -> 20 in 1.7 s, five times). Auto-fighting costs
  Robin no visible energy.
- Damage numbers: a cream-coloured (about (238,210,140)) digit string, drawn at the victim's head
  (Robin: x 880..895, y 285..300) and **rising** over about 1.5 s (the "50" climbed from y 300 to
  y 250 in three 0.5 s frames).

### 1.3 Robin's automatic attacks never hurt the halberdier (`observed`)

In 85 s (e3) + 40 s (e5) + 100 s (e7) of automatic fighting (Robin in stance, thrusting and cutting
at 52 px; sprites 55..66 by their look) the halberdier's bar never moved. Reading (`inferred`): the
pole arm's reach band (table A head, the two pole classes have the widest distances) keeps the sword
out of range, or the halberdier blocks everything a click attack throws; the manual's advice to use
the figure attacks against strong enemies is what the numbers say.

### 1.4 The drawn figures (`observed`, small sample)

Method: hold the left button, move the pointer 80 px right and 20 px up (the manual's "stroke
forwards" = slow powerful blow), release; the start point on open mud 50..100 px left-below Robin.
While the button is held the target soldier is drawn with a **yellow outline** (twice a blue one -
meaning unknown), so the engine locks the figure onto the nearest enemy.

| stroke | recording | stroke end -> bar drop | result |
|---|---|---|---|
| forward 1 | e6 | 1.03 s | 50 hp, bar 20 -> 7 |
| forward 1 | e7 | 0.87 s | 50 hp, bar 20 -> 7 |
| forward 2..5 | e7 | - (energy spent each time) | no damage; the halberdier answered with his own hit within 1.5 s three times |

So the powerful blow **does 50 damage when it lands** (a figure-eight, backward stroke and circle were
drawn in e6 only after Robin's death, on the page: untested) and landed 2 of 6 times against a
halberdier: **two landed blows kill an 80 hp halberdier** (`inferred`; he was never killed - the
five strokes of e7 used the whole recording). Swing length of the blow: the rising "50" and the bar
drop come about 0.9-1.0 s after the button release; Robin's sprite region shows a change burst of
0.6-0.9 s (animation frames stepping every ~0.1 s) after each stroke. Confidence: medium (6 strokes).

### 1.5 The soldier's hits on Robin (`observed`, confidence high for the cadence)

Landed 5 hp hits, times from the bar (0.5 s and 0.055 s resolution):

- e3 (auto-fight, 90 s): 11.5, 17.7, 30.1, 40.5, 50.8, 59.1, 64.3, 70.5, 81.8 s; intervals 6.2, 12.4,
  10.4, 10.3, 8.3, 5.2, 6.2, 11.3 s.
- e7 (with strokes, 120 s): 16.0, 24.0, 31.9, 39.4, 54.8, 61.8, 69.4, 75.5, 82.9, 95.4, 104.9, 111.6,
  118.4 s; intervals 8.0, 7.9, 7.5, 15.4, 7.0, 7.6, 6.1, 7.4, 12.5, 9.5, 6.7, 6.8 s.
- e5: 31.1, 38.4, 48.0, 62.3 s.

**Median 7.7 s between landed hits, range 5.2..15.4 s** (28 intervals). The halberd blade rising
above his helmet (a 0.27-0.31 s bright event, 12 in 64 s of e3, one per 5.3 s) is his swing; about
two swings in three land. Larger blows exist: Robin's bar fell 5 px (25 hp) between 89.0 and 90.5 s
of e3, and 13 px in the unrecorded 92 s between e5 and e6 (`observed` totals, the blows not seen).
Full health to death took **about 3 min** in the recorded 1-on-1 (100 hp at e5 29 s to 0 hp at e6
21.4 s = 187 s wall clock), and 100 -> 30 hp in 90 s in e3.

### 1.6 Geometry and animation rate (`observed`)

Fighting distance 52 px between the feet (Robin west of the halberdier; the halberdier never left his
post through 5 min of fighting). The fight animations of both sprites step at about **10 frames per
second** (the change signal alternates at 9-10 Hz, i.e. the 93.75 ms step of section 8.4, not the
46.9 ms walking frame).

## 2. Bow (question 2)

`observed`: Robin has the bow icon (100,715) with the arrow counter at 0 at mission start; hovering /
clicking the icon shows a one-line label under it, but with 0 arrows a click on the icon followed by
a move over open ground leaves the ordinary arrow pointer (no aim mode). The manual and the tutorial
texts say arrows must be gathered first (they lie further into the mission). Draw time, damage and
range: **not measured** (no arrows in the start area).

## 3. Guard behaviour (question 3)

- The halberdier stands guard and does not chase: he fought where he stood and was still at
  (934,352) after three restarts' worth of fighting - the manual's "halberdiers rarely leave their
  post" (`observed`).
- No other soldier joined a 5 min fight at the arch (the gate guards and a swordsman patrolling
  beyond the north wall stayed put), so a lone fight is possible in the tutorial courtyard; the mission-
  wide response of stealth-and-combat.md 8.6 was to a *running* hero in the open.
- Attack cadence: section 1.5 (median 7.7 s between landed hits, a swing every ~5 s). Simultaneous
  attacks by several guards: `not measured` here (8.6 saw five soldiers around Robin).
- Closing speed: `not measured` (nobody ran in these scenes; 8.4 / 8.8 give the run speeds).

## 4. Death and the lost flow (question 4)

`observed` (recording e6, 17 Hz):

- Robin's bar 1 px -> gone at **21.45 s**; the last recorded fight frame at 21.55 s still shows the
  courtyard, the frame at 21.80 s shows the page: **death -> lost page in 0.1..0.35 s**, no visible
  death animation before the page (whatever plays is under the green tint and was not analysed).
- The lost page: the world green-tinted (paused) with the HUD still drawn (money / clover counters,
  the portrait dark), a vertical parchment with rolls spanning x 264..760, y 152..614 (flat body
  300..726 x 157..610), a title row at y 205..226 centred on x 512, two text lines at y 244..276 from
  x 318 (the lost debriefing, `Level.res` 1000350 per campaign-flow.md), and three seals on the bottom
  edge: **restart** (gold, double chevron) centred (333,556), **load** (gold, folder) (388,556),
  **OK** (blue V) centred (517,547), bbox 497..530 x 537..569.
- The camera has moved when the page is up: the backdrop shows a different part of the courtyard
  than the fight camera (the north wall walkway with its patrol at the top). Whether it centres on
  the body is `not established`.
- **Restart seal -> briefing page 1 in about 7 s** (loading), three Enter presses -> mission running
  from scratch (the halberdier at full health, Robin at (513,392)); the same in three restarts.

## 5. Other HUD facts seen on the way (`observed`)

- **Mini-map**: the `;` key toggles a scroll at x 718..940, y 92..283 (map picture 204 x 155 px at
  (728,112)) drawn over the top-right of the scene; a 68 x 47 px rectangle marks the camera (at
  (825,181) with the start camera, so the whole level is about 15 screen px per map px, roughly
  3070 x 2330 px); markers are ovals (grey = unidentified, red = identified enemies, green = Robin)
  and small crosses for pickups; the world keeps running underneath. A right click does not close it
  (contrary to ui-flow.md 9.3); the key closes it; a left click on the scroll widget at (950,60)
  opens it.
- **Field of vision**: holding Left Alt with Robin selected only turned the pointer into a yellow
  curved shape (over ground and over the halberdier); no cone was drawn. Either Alt is not the key
  of key set 1 or the machine's stuck modifiers (Provenance) interfere: `not measured`.
- **Portrait burn**: at 30 hp the top ~30 px of the portrait scroll are charred dark; at 100 hp the
  scroll is intact (qualitative; the foliage border defeats a pixel count).
- Pointer shapes: arrow with a green tail (walk), with a red tail (attack / in combat), a hand over a
  pickup, a yellow curved pointer with Alt held.

## 6. Engine implications (constants for the implementer)

1. `HERO_HIT_POINTS` = 100; soldier hit points = `pre[0]` (80 for the blue halberdier confirmed).
   Bars: 20 px, 1 px = hp / 20, red row + blue row 4 px lower, 3 px tall, under the feet; drawn while
   in combat or hovered; health never regenerates.
2. Energy: 20 px; a landed soldier blow costs 1 px (regained in ~4 s); the hero's powerful blow costs
   2 px, regained at 1 px per ~0.9 s (`ENERGY_REGEN_MS` about 900); click attacks cost nothing visible.
3. Damage: soldier basic hit 5 (with occasional 25; the halberdier's row of table A); hero forward
   stroke 50 (Robin's row 1 of table A, `hypothesis`); numbers drawn cream, rising ~50 px in 1.5 s.
4. Cadence: soldier swing every ~5 s, landing about 2 of 3 (median 7.7 s between hits); a hero's
   click attacks against a pole arm never land at 52 px (reach bands of table A: make the sword's
   shorter than the halberd's, or give the halberdier a block).
5. Figure attack: lock onto the nearest enemy while the button is held (yellow outline), resolve
   ~0.9-1.0 s after release (the blow's animation), hit chance well below 1 against a halberdier
   (2 of 6).
6. Fight animations step at 93.75 ms; the halberdier stands his ground (post-bound AI, no chase).
7. Death: hero hp 0 -> lost page within 0.35 s, world paused and tinted, three seals at (333,556),
   (388,556), (517,547); restart -> briefing.
8. Mini-map toggled by `;`, 204 x 155 px picture, camera rectangle 68 x 47, closes only by key.
9. Attack order = left click on the enemy; the fist order on an alert-capable soldier from the front
   degenerates into the sword fight (no punch); the sword needs no icon or key.

## 7. Method and limits

- Recorder: `harness/tools/original/frame_rec.py`-style mss screen-DC capture wrapped in a session
  script (scratch, not committed) that keeps per frame a timestamp, a grey crop of the fight area
  (560,180)-(1024,540), colour crops of the bar band (800..1010, 320..400), the digit band (800..1000,
  220..330) and the portrait (60..200, 630..768), plus a colour PNG every 0.5 s; 25 Hz for grey-only
  recordings, 17-20 Hz with the colour crops. Bars were read by scanning each colour frame for a
  20 px run of pure red (or blue) followed by pure black; hits are the frames where the run shortens.
  Digits were located by colour in the crops and read by eye on zoomed sheets.
- Input: `oracle_input.py`'s screenshot-feedback pointer (`mmove` / `mclick`) for open ground and HUD,
  plus an open-loop 1 px per event step (about 1.1 raw units per screen px in mission) to put the
  pointer onto a sprite; figure attacks as a held button with 1 px steps; keys by scan code only while
  the game was the foreground window. Restarts through the lost page's restart seal.
- Recordings: e3 (sword attack, 90 s), e5 (fist order, 70 s), e6 (figures to death, 260 s), e7
  (five forward strokes, 120 s); single-frame captures for the mini-map, Alt, bow and lost page.
  Unrecorded gaps (about 4 min after e3 and 92 s between e5 and e6) hide Robin's first death and
  13 px of his bar.
- Not measured: knock-out from behind and the stars, the tie-up / carry prompt, the bow, the other
  eight figures, the death animation under the tint, several guards at once, guard closing speed, the
  view cone. Sample sizes are one fighter pair and 2..36 events per number; treat the cadence and hit
  chances as medium confidence and the bar / hp readings as high.

## Provenance

- Game: GOG build, `Robin Hood.exe` SHA-256
  `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, run only from the private copy
  `C:\Users\przem\source\gamedata\robinhood_oracle` (cnc-ddraw windowed 1024x768 at the screen origin,
  `maxfps=60`, `devmode=true`), profile of the 2026-09-05 session (medium), 2026-09-05, killed with
  `taskkill` at the end.
- Tools: `harness/tools/original/rhcap.py` (launch, PrintWindow shots, scan-code keys),
  `oracle_input.py` (pointer control), a scratch driver on top of them (recorder with colour crops,
  target click, drag figures); analysis with numpy / PIL / OpenCV in the session scratch directory.
  Recordings and screenshots stay in the scratch directory, nothing committed.
- Machine: Windows reported Left Ctrl and Left Shift held all session (`GetAsyncKeyState`), so no
  chords were used and Alt was the only modifier tried.
- Who: analyst session (a Claude agent), no engine code read or written.
