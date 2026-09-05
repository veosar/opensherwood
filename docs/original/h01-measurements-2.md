# The first mission, second oracle session: pick-ups, the archery yard, the mini-map, the view cone

Status: **black-box observation of the running original** (analyst session 2026-09-05, ADR-0003: no
executable, disassembly, debugger or memory inspection; screen captures of the private copy, nothing
committed). Companion to `h01-win-path.md` (the work list this document feeds) and to
`combat-measurements.md` 5 and 7 (method). Every number carries its method and a confidence; game text
is paraphrased, designer names are not reproduced, elements are named by index and role (`docs/legal.md`).

Scene: `H01_Lin_VL`, Robin alone, profile of the earlier sessions (medium difficulty, 1024x768, zoom
normal). Positions are client pixels of the game window; at this zoom a screen pixel is a map pixel
(`stealth-and-combat.md` 8.5), and the camera's map offset is tracked from the start camera (1425,1000)
(Robin at screen (512,384) = map (1937,1384)) by template matching between frames after every scroll,
then re-fixed on landmarks (the four straw targets, the tutorial scrolls) where possible. Times are the
recorder's wall clock. Element indices are the **corrected** ones of `h01-win-path.md` 2 (`ZORG`
items 100..=110, scrolls 111..=125).

The session cost three deaths (two blind walk orders into guard posts, one walkway guard), so the
west-tower objectives (the steward's scroll and purse, the knight) were **not reached**; section 8 says
what was learned about them anyway.

## 1. Pick-up items and scrolls (`observed`)

### 1.1 What the pointer shows

| Under the pointer | Pointer | Method |
|---|---|---|
| open ground | arrow with a green tail (walk) | `ptr_mud` |
| a scroll (parchment) | an **open hand**, no badge; the arrow disappears entirely | `ptr_scroll3`, `r1_hover_ko`, `s111_hover` (three scrolls) |
| a `ZORG` item | the same **hand with a round black badge carrying a digit**: `1` over item 107 at (301,1226) (`unknown_a` 8, `unknown_b` 1), `5` over the arrow pile 103 at (192,1078) (kind 0, `unknown_b` 5), `2` over the arrow pile 100 at (2199,1092) (kind 0, `unknown_b` 2) | `rd_hover_purse`, `rd_hover_arrows`, `k1_hover_arrows` |
| Robin | the walk arrow (the selection ring under him) | `ptr_robin` |
| a wall face / unreachable spot | a white-and-red **cross** ("cannot go") | `a1_hover_scroll`, `a2_end` |
| an enemy while a fight is on | arrow with a red tail | `p2_end` |
| any spot with Left Alt held | a yellow curved pointer (section 6) | `alt_held` |

So the badge digit is the record's `unknown_b`, and for arrows it is also what the counter receives
(1.3): **`unknown_b` is the stack size**, shown before the pick-up (confidence high: three items, three
distinct values, one counter check). The hit area is the sprite itself (about 12 x 14 px above the record's
base point); a hotspot 8 px under the sprite still shows the walk arrow, so the base point is the
sprite's bottom edge, the picture rising above it.

### 1.2 The gesture

One **left click with the hand pointer**. Robin is given a walk order to the item (path-finding included:
from the start yard to the walkway pile 100 he walked to the ivy, climbed it and walked along the
walkway, 12.3 s in all), stops on it, plays a short stoop (bend and rise, 0.6..0.7 s), and the item
disappears as the counter changes. No context menu, no key, no second click. A click that misses the
sprite (arrow pointer) is an ordinary ground order to that point and takes nothing, even when Robin then
stands next to the item (r2: the climbing scroll; ar1: the walkway pile with Robin 10 px away) - so the
take is bound to the order, not to proximity.

Scrolls behave the same way: hand, click, walk, stop, a pause, then the page (1.5).

### 1.3 Counters

- The HUD counters sit under the portrait: the left digit under the bow icon (arrows), the right digit
  under the pouch icon (purses); both `0` at the start; the money line `Money: £100` and `Clover: 0`
  at the top-left (the profile's money, as on the main menu).
- Arrow pile 100 (`unknown_b` 2): the left counter went **0 -> 2** at t = 12.76 s after the click (the
  30 Hz colour crop of the counters changed from 0 to 320 differing pixels in one frame); nothing else
  changed (money £100, purses 0). Confidence high.
- Purses with money (kind 9) were not taken (section 8), so the amount per purse and the money line's
  reaction are **not measured**; the engine's 25 per stack unit stays a placeholder.
- Kind 8 items (the "purse" the badge counts as 1) were not taken either: whether they feed the right
  counter is `unknown`.

### 1.4 Take distance and timing

- Arrow pile (k1, 20 Hz, spot crop): Robin walks onto the item; at the frame before the stoop his feet
  are within **0..8 px** of the pile's base (he covers the sprite), the stoop lasts about 0.6 s, the
  counter changes at its end. Confidence medium (one pick-up, 50 ms frames).
- Scroll (r1, 30 Hz): click at 0.53 s, the last walking frames at 4.0..4.9 s, Robin's lower body then
  at x 764..793 (centre 778) and feet y 394 for a scroll base at (796,386): he stops **about 18 px
  short** of the base, and the page comes up at 5.71 s, 0.7..0.8 s after he stopped. The whole
  284 px order took 5.2 s = 1 s turn/start + 3.3 s at 85 px/s + the pause, consistent with 8.1 of the
  stealth document. Confidence medium (the pointer sat over the sprite during that recording).

For the engine: no radius rule is needed - the item is taken when the walk *to the item* arrives, i.e.
when the feet reach the base (arrows) or come within about 20 px (scrolls), plus the stoop delay.
Whether a walk that merely passes an item takes it: **no** (ar1, r2).

### 1.5 The item pictures

- Arrow pile: a bundle of arrows lying flat, white shafts with red fletching, drawn with a small yellow
  **sparkle** animation over every pick-up (scrolls too: a 24 x 24 px box around a scroll changes 0..95
  pixels in a repeating cycle of about 1.5 s while nothing else moves).
- The kind-8 item on the road: a small yellow-green pouch (mostly hidden under the hand in the capture).
- Scrolls: a rolled parchment about 12 px wide with a coloured band. The manual (p. 17) distinguishes
  red-ribbon scrolls that vanish once read from blue-ribbon tutorial scrolls that stay: observed, the
  three `Tut*` scrolls (flags5 `0101010101`) stayed after reading (still drawn, still on the mini-map),
  the training-start scroll 111 (flags5 `0101010001`) was gone after its page. **Hypothesis**: the last
  bit of `flags5` is "stays after reading"; two of three bits patterns seen agree.
- An item taken disappears at once (no fade seen at 20..30 Hz).

### 1.6 Pages

- A scroll's page freezes the world under a green tint; **Enter** advances to the next page and closes
  the last one (the knock-out tutorial: two pages; the climbing tutorial and the training-start
  scroll: one page each). The single seal at (512,556) is the mouse alternative (not clicked).
- After the climbing page the camera was moved by the script (the handler's camera call) to frame the
  ivy; after the training-start page the camera framed the yard with the sergeant. The camera does not
  come back by itself; a single click on the portrait re-centres it on Robin (and selects him).

## 2. Camera facts used here (`observed`)

- The camera scrolls while the pointer touches any of the four window edges, about 400..500 px/s
  (measured only as "more than 400 px in one second"); the arrow keys sent as scan codes did nothing.
- A single click on the portrait selects Robin **and** centres the camera on him (the earlier note said
  a double click; one click suffices).
- The mini-map's 15 : 1 scale and the level size follow from section 5.

## 3. The archery yard (`observed`, one run each)

Geometry, from the frame with the camera at (1631,614) (targets at screen (485,300), (578,300),
(678,318), (752,330) = map (2116,914), (2209,914), (2309,932), (2383,944), matching objects 95..=98):
the six archers and the sergeant stand in a loose line **south of the targets**, at map (2026,914),
(2091,964), (2141,969) (the sergeant), (2051,999), (2171,1014), (2261,974), facing north; the
training-start scroll 111 at (2215,1094) and the arrow pile 100 at (2199,1092) lie **on the gatehouse
walkway**, 60..110 px south of the archers and behind their backs. The walkway is reached from the start
yard by climbing the ivy east of the gate (Robin does it by himself on a walk order to the walkway).

- **Walking to the scroll unnoticed**: yes. Order to map (2213,1109) from the start position: the ivy
  climb, arrival at 6.2 s, then 20 s standing on the walkway (a2), later another 3 minutes there taking
  the pile, reading the scroll, aiming the bow (k1, s111, sh1, sh2): no archer reacted. While Robin
  climbs, the archers' black silhouettes turn into drawn soldiers (identification by proximity, at about
  120 px).
- **Where they do notice**: a walking Robin ordered straight north through the yard along x = 2070
  from (2070,1273) (p2) was noticed within 3 s of entering the yard, when he was about 40..60 px from
  the nearest archer at (2051,999) and still south of him, i.e. **behind** the northward cone of section
  6; the yard then attacked (red circle, health bars, Robin dead in 20 s). So either a short
  omnidirectional radius (about 50 px) or the archers' turns while shooting cover the south. One run;
  confidence low on the mechanism, medium on the distance.
- **What ends the training**: not observed. The archers kept shooting for the three minutes Robin was on
  the walkway; nothing ended it. The script's arrow-in-target end needs the bow (section 4); the
  noticing end (message 2) was seen as an attack in p2, not as a walk-off - in that run the yard did
  not empty. A **walkway guard** (a halberdier patrolling the wall, first seen at map (1900,900) on the
  west walkway) reaches the gatehouse about 60 s after the training-start page and attacks Robin there;
  that, not the archers, is what ends a stay on the walkway.
- The training-start scroll's page is the sergeant's remark (paraphrased: the archers get no food until
  one of them hits a bull's-eye; start again), one page, with his portrait; the camera then frames the
  yard.

## 4. The bow (`observed`, partial)

- The bow icon is the left of the three icons under the portrait (about (95,718)); a click turns it green
  and shows a "use bow" tooltip; the pointer becomes a **grey ring with four arrowheads around a red
  disc**, with the walk arrow drawn inside it (the hotspot stays the arrow's tip).
- The arrow's tail turns **green** when the tip rests on a character: over the sergeant at 126 px (his
  red ring and health bars appear). Over the straw target at 186 px (tip on the straw, six offsets
  tried), over the ground at 95 px and over an archer at 62 px (tip beside the sprite) the tail stayed
  red. A click with the red tail **fires nothing** (counter 2 before and after, no animation).
- So a straw target at 186 px from the walkway is not a valid shot; whether that is range, the height
  difference (walkway to yard) or the target's hit area is `unknown`; the flight, draw time and damage
  stay **not measured**. The manual's rule (click when green) holds for characters.

## 5. Mini-map (`observed`, two frames)

- Toggle: `;` opens and closes it; the world keeps running. Widget x 718..940, y 92..283; the map picture
  204 x 155 px at (728,112); **15 map px per picture px** (the item crosses of 5 items and 3 scrolls match
  `728 + x/15, 112 + y/15` within 2 px), so the level is about 3060 x 2325 px.
- Camera rectangle: 1 px black outline, **67 x 47** px (1024/15, 768/15).
- Markers, each a vertical **oval 2 px wide, 4 px tall** with a 1 px black outline (4 x 6 with outline):
  - red fill (255,0,0): identified enemies (the archers and guards near Robin once he was close);
  - light green fill (164,251,82): Robin;
  - grey fill (about (150..200) neutral): unidentified characters (silhouettes), the rest of the garrison
    and the civilians;
  - **crosses**, 5 x 5 px, yellow with a white centre: pick-ups - both scrolls and `ZORG` items (the two
    road items 103 / 107, the walkway pile 100 until taken, the north-east pile 101, the three tutorial
    scrolls, the pick-up tutorial scroll 119); a taken item's cross disappears.
- Confirmed against `combat-measurements.md` 5 (68 x 47 there; 67 here, one pixel of reading).

## 6. Field of vision with Left Alt (`observed`, one actor)

- Left Alt through `SendInput` (scan code 0x38, held) with Robin selected turns the pointer into the
  yellow curved shape and draws **nothing over open ground**; with the pointer resting on the sergeant
  (the officer of the yard, map (2141,969)) it draws his **view cone**: a translucent green sector with
  its apex at his feet.
- Geometry from the frame difference (green-tinted pixels, 25 200 of them): apex at screen (524,380);
  left edge to (477,194), i.e. **14 degrees west of north**; right edge to (769,264), i.e. **65 degrees
  east of north**: a **sector of about 80 degrees** (half-angle 40) whose axis points 25 degrees east of
  north - the sergeant faces the targets. Reach: **about 270 px** along the screen x axis, **about
  196 px** along y (the top of the sector at y = 184): the boundary is an ellipse with the y axis
  compressed to 0.72, not a circle. The cone is drawn for the character under the pointer only (the
  earlier session hovered a halberdier's shadow and saw nothing: the hotspot must be on the sprite).
- Confidence medium: one actor, one frame, the sprite's own pixels excluded by the tint test; the
  compression may be the game's distance metric or a projected drawing.

## 7. Engine implications (constants, ordered by the win-path work list)

1. (Work list 3, items) `ITEM_PICKUP`: the pointer over an active item is the hand with the stack digit;
   a left click orders a walk to the item; on arrival (feet within `ITEM_TAKE_RADIUS` about 8 px of the
   base for items, `SCROLL_TAKE_RADIUS` about 20 px for scrolls) a `STOOP_TICKS` pause of about 0.6..0.7 s
   (40 ticks at 60 Hz) then the take: arrows += `unknown_b`, the item removed, native 235 reads taken.
   A passing walk never takes. The engine's 24 px approach rule is replaced by the order-bound take;
   its "stack" reading of `unknown_b` is confirmed for arrows.
2. (3) HUD: arrows = left digit under the bow icon, purses = right digit; both 0 at load; the money
   line at the top-left starts from the profile's money (£100 here). The purse amount stays a
   placeholder (not measured).
3. (3) Pick-up pictures: an arrow bundle and a pouch with a sparkle loop of about 1.5 s; scrolls with a
   coloured band; `flags5` bit 0 as "stays after reading" (hypothesis) so tutorial scrolls remain.
4. (3 / 8) Pages: Enter advances and closes; the world frozen and tinted; a handler's camera call moves
   the view and nothing moves it back except the portrait click.
5. (6, bow) The icon toggles bow mode; the aim pointer; a click fires only with the green tail (tip on a
   character within range); a straw target at 186 px from the walkway is not a valid shot - do not let
   the engine's target hit test accept it from there until the reason is known.
6. (7, perception) `VIEW_CONE_HALF_ANGLE` about 40 degrees (not 22.5), `VIEW_RANGE` about 270 px along
   x with the y axis weighted by 1/0.72 (an ellipse), the cone bound to the actor's facing (the archers
   face north); a hero 60..110 px behind a training archer is not seen for minutes. Add a short
   omnidirectional radius of about 50 px (hypothesis from p2) or model the archers' turns; either way
   the engine's current "noticed on the approach" at world tick 280 is wrong for the walkway route,
   which must reach scroll 111 unnoticed (through the ivy climb: layers, work list 4).
7. (5, money / search) Not measured; keep the 300 stub on the knight.
8. Mini-map: 15 : 1, picture 204 x 155 at (728,112), camera rectangle 67 x 47, ovals 2 x 4 (+1 outline)
   in red / green / grey, 5 x 5 crosses for every pick-up (items and scrolls alike), `;` toggles, world
   keeps running.
9. Camera: edge scrolling on all four edges (>400 px/s), no arrow-key scrolling observed; the portrait
   click centres.

## 8. Not reached, and what is known instead

- **The steward's tip scroll 120 at (941,1192) and the purse 105 at (572,1388)** (objective 3): both
  blind walk orders towards the west (to map (876,1242) and to the road at (340,1262)) ended in a fight
  and Robin's death within 16..20 s, out of view; the route the script intends (doors opened area by
  area, `h01-win-path.md` 4.2) needs the door table the engine does not parse yet. The road west of the
  castle (camera at (0,1000)) does hold two active items in the open, 103 (arrows, `5`) at (192,1078)
  and 107 (kind 8, `1`) at (301,1226), with unidentified figures nearby, so the kind-8 pouch and a
  five-arrow pile can be measured there once the route is known.
- **The knight 78 at (861,1135)**: not approached; the search action on a body remains unmeasured (the
  manual lists search among the context actions; the pointer should change over a valid body as it does
  over items).
- **The money purse 108** (kind 9, `unknown_b` 2, activated by the money scroll 112 at (2070,1013)) lies
  in the archery yard among the archers' line: it is the tutorial's own demonstration and becomes safe
  only once the training ends (an arrow in a target).

## 9. Method and limits

- Tools: `harness/tools/original/rhcap.py` (launch, PrintWindow shots, scan-code keys, raw mouse deltas)
  and `oracle_input.py` (screenshot-feedback pointer), driven by scratch modules in the session
  directory (`drv.py` from the earlier session: recorder with colour crops; a new `drv2.py`: closed-loop
  camera scrolling by edge bursts measured with template matching, closed-loop item hover creeping up
  from under the sprite until the arrow template is lost, hand / badge crops). Recordings: `k1` (20 Hz,
  the arrow pile), `r1` / `r2` (30 Hz, two scrolls), `a2` (30 Hz, the walkway approach), `p2` (the yard
  approach that ended in the fight), `sh1` / `sh2` (25 Hz, bow clicks), single frames for the pointer
  catalogue, the mini-map, the Alt cone; one-second frame series for the two west runs. All under the
  scratch directory, nothing committed.
- Pointer relation on this run: in the menus the game pointer equals the OS cursor (factor 1, unlike the
  2 of the earlier session's DPI setting); in a mission a raw unit moves the pointer about 0.7..0.8 px
  and the OS cursor is useless, so item hovers were verified by the arrow template disappearing and
  aiming by crops. The machine again reported Left Ctrl and Left Shift as held; no chords were used;
  Enter, `;`, Left Alt and Escape went through `SendInput` scan codes with the game foreground.
- Deaths: three (p2 yard fight, the two west runs); restarts through the lost page's restart seal and
  once through the pause menu's Restart button (748,550).
- Not measured: the purse amounts, the search action, the bow's flight and range, the archers' reaction
  when the training ends, the noticing radius from behind (one event), the cone of any other profile.

## Provenance

- Game: GOG build, `Robin Hood.exe` SHA-256
  `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, run only from the private copy
  `C:\Users\przem\source\gamedata\robinhood_oracle` (cnc-ddraw windowed 1024x768 at the screen origin,
  `maxfps=60`), profile "Analyst" (medium), 2026-09-05; killed with `Stop-Process` at the end.
- Data cross-checks: `DATA/Levels/H01_Lin_VL.rhm` through `harness/tools/probe/rhm_full.py --json` (the
  `ZORG` and `SKRO` records: positions, `unknown_a` / `unknown_b`, `flags5`), read from the analysis copy
  `C:\Users\przem\source\gamedata\robinhood`; the manual (rendered pages, printed 16, 17, 28) read by eye
  and paraphrased.
- Who: analyst session (a Claude agent); no engine code read or written beyond the documents named
  above.
