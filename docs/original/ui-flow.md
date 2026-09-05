# Original game: UI flow (black-box observation)

Status: **observed** unless marked *inferred* / *unknown*. Coordinates are logical pixels of the game's own
framebuffer at **1024x768** (the resolution stored in the analysed profile; the engine only knows 640x480,
800x600 and 1024x768, see `executable-notes.md`). Screenshots live in `harness/captures/original/` (git-ignored,
never committed); they are referenced by file name only. Measurements from screenshots are accurate to about
+-2 px unless stated otherwise.

See also `campaign-flow.md` (what happens after "Play!") and `console-commands.md`.

## 1. Boot sequence

| t (s, from process start) | What is shown | File |
|---|---|---|
| 0.0 | window `Robin Hood - Legend Of Sherwood` appears, client 1024x768, black | `boot_000_000.1s.png` |
| ~0.3-0.7 | splash: title logo "Robin Hood - The Legend of Sherwood" over a picture of Robin's eyes, centred on black (about 470x210 px at (280,40)) | `boot_001_000.4s.png` |
| ~0.8 | main menu with the last used profile | `boot_002_000.8s.png` |

- No publisher / developer slides and no intro video are played at boot (GOG build, existing profile). The intro
  movie is only reachable through **Show movies**. Whether a first-ever start (no `DATA/Savegame/Profiles`)
  plays the intro is *unknown* (not tested, see open questions).
- Nothing was skipped by keys; the splash is too short to test.
- Timing measured on a fast machine (NVMe SSD); the splash is probably shown while the resource files load, so
  the duration is machine dependent (*inferred*).

## 2. Screen layout common to all menus

- Every menu screen is a **1024x512 background picture letter-boxed in the 1024x768 frame**: rows 0..127 and
  640..767 are black (the picture's own vignette makes rows 128..143 and 624..639 almost black). *Inferred*:
  the picture is blitted at (0,128). At 640x480 / 800x600 the layout is not observed (open question).
- Backgrounds seen (all `DATA/Interface/DEFAULT.RES`, tag `PIC `, matched by eye):
  - id 187 (1024x512): castle tower + oak tree = **main menu**.
  - id 186 (1024x512): forest with ivy pillars = **Options**, **Select player**, **Show movies**.
  - id 188 (1024x512): sunlit forest with a wrecked hut = **Graphical options**, **Sound options**.
  - id 189 (1024x512): dungeon portcullis with a vertical iron bar = **Load**, **Save**, **Shortcuts**
    (the portcullis is the list area, the bar is the scrollbar).
  - id 309 (1024x768): dark forest = **Credits** (full frame, no letterbox).
- Text is drawn with the bitmap fonts of `DATA/Interface/Fonts` (`Title.tfn` for the orange gothic screen titles,
  `MenuButtonEnabled/Disabled.tfn` for button labels, `ListDefault/Focused/Selected.tfn` for list rows,
  `EditFields.tfn` for the name field, `Debrief.tfn` for the briefing parchment, `tooltips*.tfn`,
  `InfoScroll.tfn`). Which font is which was matched by name only (*inferred*).
- **Mouse cursor**: green-tailed white arrow, matches `DEFAULT.RES` `PIC` id 284 (30x38) / `CUR` id 28 (28x34)
  (*inferred*: 284 is the arrow with a small down-triangle; 285..295 are the other arrow variants).

### 2.1 Menu button column

All menu buttons share one widget: `DEFAULT.RES` `BTTN` id 190 (168x39, 3 pictures = 3 states). Observed states:
grey/dark plate = disabled, teal plate = normal, orange plate = hovered (the label is drawn on top; disabled
labels use the "Disabled" font). The column is right-aligned:

- x = 664..831 (168 px), row pitch **41 px**, plate height 39.
- Row k (k = 0..6) has plate top y = **339 + 41*k**: 339, 380, 421, 462, 503, 544, 585 (measured on
  `menu_main.png` and `pause_menu.png` at x = 668: bright frame rows 339..342, 380..383, ..., 615..617; an
  earlier reading of 345 in this document was the label baseline area, not the plate top). Row y values quoted
  elsewhere in this file as 345 + 41*k are the earlier reading and mean the same rows. Every screen fills the column
  from the bottom (`Back`/`Cancel`/`Quit`/`OK` always on the last row, y = 591).
- Label text centred, y-centre = row y + 19.

| Screen | Rows (top to bottom, with y) |
|---|---|
| Main menu | play 345, load 386, select player 427, options 468, movies 509, credits 550, exit 591 (labels: `Level.res` TEXT, see 10) |
| Options | Graphics 468, Sounds 509, Shortcuts 550, Back 591 |
| Graphical options / Sound options | OK 550, Cancel 591 |
| Shortcuts | OK 427, Default 1 468, Default 2 509, User defined 550, Cancel 591 |
| Load (main menu and pause) | Load 509, Delete 550, Cancel 591 (Load/Delete disabled while nothing is selected) |
| Save (pause menu, from the manual p.8) | Save 509, Delete 550, Cancel 591, plus a name field at the bottom of the list |
| Select player | Select 468, New 509, Rename 550, Delete 591 |
| Show movies | OK 591 |
| Pause (Escape) menu in mission | Continue 386, Load 427, Save 468, Options 509, Restart 550, Quit 591 |

Files: `menu_main.png`, `menu_hover_play.png` (Play! hovered = orange), `options_main.png`,
`options_graphics.png`, `options_sounds.png`, `options_shortcuts.png`, `menu_load.png`, `menu_select.png`,
`menu_movies.png`, `pause_menu.png`.

### 2.2 Keyboard in menus

- **Escape** in the main menu opens the quit confirmation dialog
  (`menu_quit_dialog.png`). Escape closes: Graphical options (= Cancel), Load, Show movies, Credits.
- Escape does **not** leave Sound options, Shortcuts or Select player (their Cancel/Select button must be
  clicked). While a name is being edited (Rename / New) Escape and Enter only end the edit.
- Enter confirms nothing in the New player dialog (it just leaves the text field); the blue seal must be clicked.

### 2.3 Dialogs (parchment scrolls)

- Quit confirmation: horizontal scroll about 400x200 at (312,288) = `DEFAULT.RES` `PIC` 38 or 237 (400x200,
  *inferred*), question text centred in the top half, two wax seals below: blue **V** (yes) at (483,433) and red
  **X** (no) at (541,433), 41x44 = `BTTN` 145 (blue) / 146 (red), 3 states. The dialog is modal: the buttons
  behind it ignore clicks.
- The same seals confirm/cancel the New player dialog and the in-game leave-the-game confirmation
  (pause menu -> Quit). `BTTN` 281/282 (41x41, seal with a blue ring) are a second style not yet seen.

## 3. Main menu (`menu_main.png`)

Left of the buttons, centred at x = 432, the current profile summary in yellow-orange text:

```
<profile name>                 (title font, y = 254)
difficulty label : <level>     (y = 278)
money label: <pound sign><n>   (y = 298)
score label : <n>              (y = 318)
spared-lives label : <n> %     (y = 338)
progress label : <n> %         (y = 358)
game-length label : <mm:ss>    (y = 378)
```
(The label strings come from `Level.res` TEXT entries at run time and are not reproduced here.)

Buttons: see 2.1. **Exit to windows** asks for confirmation (same dialog as Escape). The values come from the
selected profile (`DATA/Savegame/Profiles`, see `docs/formats/savegame.md`).

## 4. Options

`options_main.png`: screen title (y = 158, x-centre 442), two info lines with the processor name and clock
(y = 254) and the memory size in MB (y = 274), buttons graphics / sounds / shortcuts / back.

### 4.1 Graphical options (`options_graphics.png`)

Screen title (TEXT 1000507 string 28). Two groups of full-width option bars (x = 227..639, 413 px wide; a bar is 26 px
high, pitch 41; orange = selected / enabled, teal = not selected):

| Group label (y) | Bars (y of bar top) | Default |
|---|---|---|
| resolution group, string 42 (233) | ratio bars, strings 43..45 (249, 290, 331) | first |
| effects group, string 46 (383) | four toggles, strings 47..50 (400, 441, 482, 523) | all enabled |

(Option labels are `Level.res` TEXT 1000507 strings; indices are given instead of the wording.)

The GOG/Ready2Play build offers **aspect ratios instead of resolutions** here (the retail game listed
640x480 / 800x600 / 1024x768 according to its strings; *inferred* that the patched build maps the ratio to a
resolution). OK (550) applies, Cancel (591) / Escape discards.

### 4.2 Sound options (`options_sounds.png`)

| Control | y | Values / default |
|---|---|---|
| output mode bars (strings 51, 53) | 220 / 261 | first selected; the 3D one greyed out (unavailable on this machine) |
| quality bars (strings 54, 55) | 320 / 361 | first selected |
| volume 1 (string 56) | label 424, slider 433..447 | 10 cells of 27 px (x = 226 + 42*i, i = 0..9), value = index of the orange cell; default 10/10 |
| volume 2 (string 57) | label 464, slider 473 | default 10/10 |
| volume 3 (string 58) | label 504, slider 513 | default 10/10 |
| volume 4 (string 59) | label 544, slider 553 | default 10/10 |
| frequency (string 60) | label 584, slider 593 | default 6/10 |

Slider widget = `DEFAULT.RES` `SLID` id 201 (6 pictures, 25x21 knobs) on a 10-cell track (*inferred*). OK / Cancel.

### 4.3 Shortcuts (`options_shortcuts.png`)

A two-column table over the dungeon background, actions at x = 226, keys right-aligned to x = 590, 15 px line
pitch starting at y = 161. Buttons: OK, default set 1, default set 2, user defined (orange = active set),
Cancel (strings 15, 21, 22, 23, 16). The manual (p.31) gives both default sets. Observed bindings of set 1,
by function (the action names are TEXT 1000507 strings and are not reproduced): zoom in/out = numpad +/-;
scroll = arrow keys; minimap = `;`; select character 1..5 = digits 1..5; select all / none = q / d; crouch /
stand = c / s; go-behind modifier = left Shift; outlines = Caps Lock; action 1/2/3 = g / h / j; move-during-
action modifier = left Ctrl; save quick action = a; start quick actions = Space; clear = Backspace; field of
vision = Alt; quick save / quick load = F1 / F5. Set 2 (manual) moves most of these to the numpad (1..5, 6 / 0,
7 / 8 / 9, *), Page Up/Down for crouch / stand, right Shift / right Ctrl for the modifiers, Return for the quick
action save and AltGr for the field of vision.

The active set is stored in `DATA/Configuration/keyset1.cfg` / `keyset2.cfg` (76 bytes each) and in the profile.

## 5. Select player (`menu_select.png`, `select_new_*.png`)

- List of **10 rows** (x = 227..639, row top y = 225 + 41*k, 23 px high). A row shows the profile name at
  x = 236 and "<difficulty> / <progress> %" right-aligned to x = 628. The selected row is orange, others grey.
- **Select** returns to the main menu with that profile. **Rename** turns the row into an edit field with a
  caret. **Delete** (not tested). **New** opens the "New player" parchment (`select_new_clicked.png`):
  vertical scroll (~496x463, `PIC` 147, *inferred*) centred, a title, a name label, a text field
  (about 400x22 at (316,290), `NPTF` 191 424x39 *inferred*) that has keyboard focus immediately, a difficulty
  label with three wax seals **easy (444,428) / medium (504,428) / hard (580,428)** (medium
  pre-selected, orange V mark), then the blue **V** (480,542) / red **X** (540,542) seals. Typing the name and
  clicking V adds the profile as a new selected row (`select_new_confirmed.png`); the main menu then shows it with
  Money L100, Score 0, Progress 0 % (`menu_main_analyst.png`).

## 6. Load / Save

`menu_load.png`: dungeon background, list area x = 227..610, y = 160..600 (the portcullis), scrollbar at
x = 620..640 with a knob at the top. Buttons Load / Delete / Cancel. With the shipped profile the list was
**empty** although `DATA/Savegame/Profile_001/` contains `Continue`, `Restart` and their `_t` thumbnails: those
are automatic saves that are not listed (see open questions). The manual (p.7-8) shows one line per save with a
name and a thumbnail in the top-left corner; quicksaves are named `QuickSave`, a second one renames the first to
`ExQuickSave`. The Save screen adds a text field for the save name at the bottom of the list and Save / Delete /
Cancel buttons (manual p.8; not captured because in-mission mouse input could not be driven, see Provenance).

## 7. Show movies (`menu_movies.png`)

Screen title; a framed thumbnail 264x134 at (300,225) (`BTTN` 297 = Intro, 3 states; `BTTN` 298 = Outro,
presumably shown after the campaign) with a still from `2047/data/Cinematics/Intro.vid`; OK at the bottom.
Clicking the thumbnail was not tested. Escape returns to the main menu.

## 8. Credits (`menu_credits.png`, `menu_credits_later.png`)

Full-frame dark forest (`PIC` 309, 1024x768), white credits scrolling upwards (studio logo, then the team
by role). The text is *probably* the long
strip `PIC` 308 (400x7659) scrolled at about 20 px/s (from the two frames 6 s apart: roughly 120 px). Escape
returns to the main menu.

## 9. In mission

### 9.1 Loading screen (`play_001_001.8s.png`)

Full-frame painting of Robin drawing his bow, a thin progress bar at y = 700 (x = 95..930), "v1.1" in the
top-right corner (x = 995, y = 8). Visible for about 1.7 s on this machine.

### 9.2 Briefing parchment (`m1_brief_page1..3.png`)

The mission is loaded and drawn **green-tinted (paused)** (measured on `m1_brief_page1.png` against the engine's
untinted frame at the same camera: each output channel is a multiple of the luminance, (r, g, b) =
lum x (0.12, 0.43, 0.29), residual 4-6 levels; the camera is centred on the hero, offset 0,0) behind a vertical parchment (~496x463 at (264,148),
`PIC` 147 *inferred*). Text in the dark-brown `Debrief` font, left-aligned at x = 318, wrapped to ~400 px, with
a 120x160 character picture at (600,205) (the `Level.res` `PIC` entries 1000007/1000010/... are 120x160 and
`DEFAULT.RES` `PICC` 252..267 are 5-frame 120x160 sets; which one is used here is *unknown*). One string per
page; a blue **V** seal at (508,552) or **Enter** goes to the next page; after the last page the game starts
(unpaused). Page 1 of the first mission = `Level.res` TEXT 1000105 string 0 (see `campaign-flow.md`).

### 9.3 HUD (`m1_brief_page1.png` behind the parchment, `_hud_crops.png` = 2x crops)

Manual p.11 numbers the elements; observed positions at 1024x768:

| # | Element | Position | Resource (matched by eye) |
|---|---|---|---|
| 1 | Game information: the money and clover counters (L100 / 0 at the start), yellow outlined text | (4,4) and (4,20) | font `tooltips.tfn` (*inferred*); clover icon `BTTN` 165 |
| 2 | Mini-map scroll, opens the mini-map; the `;` key toggles it; a right click does **not** close it (measured 2026-09-05, `combat-measurements.md` 5: the scroll at x 718..940, y 92..283, the map picture 204x155 at (728,112), a 68x47 camera rectangle, the world running on) | top-right, scroll at ~(945,25)-(1000,75) | `BTTN` 61 (61x52) *inferred* |
|   | Engine: the overlay draws the level's `.min` scroll at the measured position with the camera rectangle over the map area; the original's markers (ovals for characters, crosses for pickups) are not drawn yet. | | |
| 3 | Zoom towers = zoom levels (normal / near / distant), two small tower icons | (998,8)-(1024,60) | `BTTN` 4 (26x46) / 5 (26x54) |
| - | Robin's eyes in the foliage (top-right corner) - decoration / hidden button? | (950,0)-(1024,60) | `BTTN` 60 (74x60) |
| 4 | Hero portrait: parchment with the face, name "Robin / Hood" in two lines, action icons (bow, fist = knock out, purse = throw purse) and two counters "0" (arrows) and "0" (purses / money bags) | (70,632)-(185,765) | portrait faces `PIC` 136..155 (40x50) and `PICC` 242..244; frame *unknown* |
| 7 | Crouch / stand figures left of the portrait: a small standing Robin and a kneeling Robin | (5,660)-(45,760) | `BTTN` 3 (43x62 standing) / `BTTN` 2 (43x45 kneeling) |
| 5 / 9 | Quick-action plan scroll bottom-right; the bugle icon (start quick actions) appears next to it when a plan exists | (950,700)-(1010,755) | `BTTN` 251 (74x53) or `BTTN` 1 (43x41) |
| - | Foliage border along the bottom | y = 655..768, full width | `PIC` 50 (871x110) + 51 (734x110) + corners 46/47 |
| 6 / 8 | Field of vision (Alt) and health meter around a selected character | not captured | - |

Implementer's measurement (2026-09-02, template matching of the decoded pictures in `pause_menu.png`, correlation
in brackets): eyes `BTTN` 60 at (924,0) [0.97], towers `BTTN` 4 at (998,0) [0.99] and `BTTN` 5 at (998,46)
[0.91], map scroll `BTTN` 61 at (941,38) [0.99], standing figure `BTTN` 3 at (1,661) [0.96], kneeling figure
`BTTN` 2 at (0,721) [0.94], the bottom-right scroll is `BTTN` 1 (43x41) at (964,701) [0.998] rather than 251,
portrait face `PIC` 136 at (83,657) [0.83]. The portrait's parchment frame is not any picture of `DEFAULT.RES`
between 90 and 140 px wide (`actors.res` is the only other archive), so it is probably composed from smaller
pieces; the foliage pieces 46..51 match nowhere with confidence above 0.68, their placement is unknown.

Money is the campaign money (L100 at the start). The top-right seal that ends a mission (manual p.10) appears
only when the mission is won; the tutorial text of mission 1 refers to it as an icon at the top right.

### 9.4 Camera, selection, orders (from the manual p.15-16 and 26-27; not reproduced with input, see Provenance)

- Scrolling: arrow keys and screen edges; three zoom levels (normal / near / distant) via mouse wheel or the
  Towers icon. Double-clicking a portrait centres the camera on that character.
- Left click on a character or his portrait selects him (green circle at the feet, red circle in combat). Shift +
  click on portraits or a drag box on the map selects several. Right-click on empty ground deselects / cancels
  an action.
- Left click on the ground = walk, **double-click = run** (more noise). Cursor becomes a cross where the
  character cannot go, a double vertical arrow over climbable ivy, a curved arrow with a blue trajectory line
  over a jump spot, a door symbol over doors, an "OK" hand when confirming a directed action, an outlined yellow
  cross with Left Shift held ("go behind house"). Right-click on the selected character cancels his order.
- Actions: click an icon under the portrait then the target; Ctrl + click moves first and performs the action on
  arrival; right-click on an action icon drops one ammunition object (double right-click: five).
- Combat: hold the left button and draw a figure (circle, horizontal 8, straight strokes) to choose an attack;
  the tutorial text of mission 1 (Level.res 1000105 string 21) describes the same.
- Quick actions: save a plan per character (key `a`), start all with Space, clear with Backspace.
- Dialogues, popups and tutorial hints: not reached (open question). Popup texts of mission 1 are Level.res
  1000105 strings 3..22 (see `campaign-flow.md`).

### 9.5 Pause menu (`pause_menu.png`)

Escape pauses: the scene is tinted green, the current objective (short briefing, `Level.res` 1000283 string 0
for mission 1) is written at (210,150) in a small white font, and the button column continue / load / save /
options / restart / quit appears (strings 9..14; rows k = 1..6). Escape again continues. Quit asks the
leave-the-game question (string 31) on a horizontal scroll with V / X seals (observed once, screenshot lost);
Options opens the same Options screen as the main menu with the forest background.

### 9.6 Win / lose

Not reached. Per manual p.10: when the objectives are met a message says the mission is won and a seal appears
at the top right of the screen to leave; a mission is lost when a main character dies. The debriefing texts of
mission 1 are Level.res 1000349 (won) and 1000350 (lost). The manual (p.37) documents the developer console
(with a standing character selected, hover the kneel icon and press F11), codes
`goodluck`, `cash`, `bingo`, `immunity`, `merryman`, `timeless`, `pam`, `unblip`, `winner` (win mission) - so the
retail build should be able to show the win screen through `winner` (untested, see `console-commands.md`).

## 10. Resource ids seen in the UI (`DATA/Interface/DEFAULT.RES`)

| Id | Kind | Size | Used for (by eye) |
|---|---|---|---|
| 186 / 187 / 188 / 189 | PIC | 1024x512 | menu backgrounds: forest / castle (main) / sunlit forest (graphics, sound) / dungeon (load, save, shortcuts) |
| 309 | PIC | 1024x768 | credits background |
| 308 | PIC | 400x7659 | credits text strip (*inferred*) |
| 190 | BTTN | 168x39 x3 | menu button (disabled / normal / hovered) |
| 144 | BTTN | 200x50 x3 | text field frames (rename / save name) *inferred* |
| 191 | NPTF | 424x39 x6 | name input field (New player) *inferred* |
| 201 | SLID | 25x21 x6 | volume slider knob |
| 145 / 146 | BTTN | 41x44 x3 | blue V / red X seals (dialogs) |
| 281 / 282 | BTTN | 41x41 x3 | V / X seals with a ring (not seen yet) |
| 38, 237 | PIC | 400x200 | horizontal dialog scroll |
| 147 | PIC | 496x463 | vertical parchment (New player, briefing) |
| 162 | PIC | 496x200 | wide scroll (unknown use) |
| 133 | PIC | 220x100 | small scroll (tooltip / experience parchment?) |
| 297 / 298 | BTTN | 264x134 x3 | movie thumbnails Intro / Outro |
| 123 | PIC | 629x480 | campaign map (York, Lincoln, Sherwood, Derby, Nottingham, Leicester) |
| 125..129 | BTTN | ~120x120 x3 | town icons on the campaign map |
| 130..132 | BTTN | 30x41 x3 | map markers |
| 227..236 | PIC | various | troop-movement arrows on the campaign map |
| 225, 238, 239 | PIC | 50x21, 16x21, 32x42 | red ribbon, small / large heraldic shield |
| 60 | BTTN | 74x60 | Robin's eyes in leaves (HUD top-right) |
| 61 | BTTN | 61x52 | map scroll (HUD) |
| 251 | BTTN | 74x53 | plan / minimap scroll (HUD bottom-right) |
| 1 | BTTN | 43x41 | small scroll icon |
| 2 / 3 | BTTN | 43x45 / 43x62 | kneeling / standing figure (crouch, stand up) |
| 4 / 5 | BTTN | 26x46 / 26x54 | towers (zoom) |
| 165 | BTTN | 33x39 | clover |
| 48 / 49, 50 / 51, 46 / 47 | PIC | 320x165, 871x110 / 734x110, 46x63 | HUD foliage borders |
| 42, 71..75, 113..115 | BTTN | 112x50 x3 | portrait + coloured bar (Sherwood team boxes?) |
| 299..307 | PIC | 112x50 | portrait + crossed swords (Sherwood / mission stats?) |
| 240 | BTTN | 128x72 | group picture of the Merry Men |
| 157..160 | BTTN | 88x41 | purse / archer / cart / tower -> shield (Sherwood or campaign actions) |
| 184 / 185, 277 / 278 | BTTN | 31x72, 44x46 | list arrows, gold "<< >>" coins (dialogue paging?) |
| 284 (+285..295) | PIC | 30x38 | mouse cursors (plain arrow = 284) |
| 161 | PIC | 640x480 | CD picture (the disc-check screen, not seen) |
| 164 | PIC | 160x100 | "?" placeholder (movie thumbnail?) |
| 37 | TEXT | 34 strings | UI strings (the first is the mission-failed message) |

## Provenance

- Game: GOG build with the 2025-04-08 Ready2Play launcher, `Robin Hood.exe` SHA-256
  `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, English (`2047/data`), run from the private
  copy `C:\Users\przem\source\gamedata\robinhood_oracle` (identical files, own `ddraw.ini` and savegames; the
  running game locks `DATA\robinhood.bks`, so the copy used by the engine's tests must not be the one played).
  In-game version string "v1.1" on the loading screen.
- cnc-ddraw `ddraw.ini`: `windowed=true`, `fullscreen=false`, `devmode=true`, `maxfps=60`, `savesettings=0`,
  `renderer=direct3d9on12`, `width=height=0` (window = game resolution). Profile resolution 1024x768.
- Method: black-box play, 2026-09-02. Screenshots taken with `harness/tools/original/rhcap.py` (PrintWindow
  with PW_RENDERFULLCONTENT on the client area, so overlapping windows do not matter). Input: the game reads the
  mouse through DirectInput as relative deltas, so `SetCursorPos` does not move its cursor; `rhcap.py` clamps
  the in-game cursor to the top-left corner with a large negative raw delta and then moves by `x*0.775,
  y*0.775` (the engine scales raw deltas by about 1.29 at this resolution). This worked in all menu screens
  (`tour_menus.py`, `tour_menus2.py`); **in mission the in-game cursor stopped following any injected input**
  (only Escape and Enter were accepted), and a human was using the machine concurrently, so the in-mission
  parts marked "not reproduced" come from the printed manual (`Manual.pdf`, pages 7-11, 15-16, 26-31, 37 of
  the printed numbering) and from the mission texts in `Level.res`.
- Resource ids: `cargo run -p opensherwood-tools -- sres <gamedir>/DATA/Interface/DEFAULT.RES` and
  `export-sres` of the candidate ids, compared visually with the screenshots.
- Level.res texts: `harness/tools/original/sres_text.py` (reader written from `docs/formats/sres.md`).

## Open questions

1. First-ever start (no `Profiles` file): is the intro video played, is the New player dialog forced?
2. Layout at 640x480 and 800x600: are the menu pictures scaled or are there other backgrounds (the archive has
   only 1024x512 menu pictures, so the smaller modes probably scale or crop them)?
3. Exact widget frame pictures of the hero portrait, the objective text font, the tooltip fonts.
4. Load list: why are the profile's `Continue` / `Restart` saves not listed; what a listed save looks like
   (thumbnail `*_t` files are 24 KB, probably 160x100 or similar RGB565 blobs - see `docs/formats/savegame.md`).
5. Dialogue presentation (position of the text parchment, portraits, paging with the gold << >> coins), popup
   tutorial texts, the mini-map, the win / lose screens and the mission statistics screen.
6. Behaviour of Delete / Rename in Select player, of the movie thumbnails, of the Outro entry.
7. Which BTTN picture index is which state for 4-state buttons (`states` = 15).
8. Why injected mouse input stops working once a mission is running (DirectInput exclusive mode? cursor clipped
   by the game?) - needed before oracle capture of gameplay can be automated.
