# The first mission's win path: what the engine has, what it lacks

Status: **data-file observation plus black-box runs of the engine** (analyst session 2026-09-05, ADR-0003:
no executable, disassembly or debugger involved; the original was not run for this document, its
earlier measurements are cited from `combat-measurements.md` and `stealth-and-combat.md` 8). Every claim
carries a status: `observed` (read from a file, the manual, or the engine's `debug.vm` / `observe` in a
run), `inferred` (a reading that fits every case found), `hypothesis`, `unknown`. Game text is paraphrased,
designer names are not reproduced: elements are named by index and role (`docs/legal.md`).

Scope: `H01_Lin_VL` (Lincoln, the tutorial in the retail flow, Robin alone): (1) the mission's objectives
in order, what completes each and which native or event the script reads; (2) whether the engine (the
release binary built on 2026-09-05 from the tree of `8b64102`, reporting ruleset 13 / protocol 6 / hash
schema 15; HEAD `f430130` differs from it only in documentation) satisfies each through canonical input
today, and the missing native
or mechanic where it does not; (3) the shortest canonical input script that wins; (4) an engine work list
ordered by dependency. Section 5 records how far the current binary got through play.

Sources: `docs/formats/scb.md` (the H01 walkthrough, the native table, the stub policy),
`docs/formats/sherwood-hub.md` 4 (the element table), `docs/formats/rhm.md`, `docs/harness.md`
(`debug.vm`), the manual (printed pages 16-17, 26-28; `Manual.pdf` page = printed page + 2), the
mission's files through `opensherwood-tools rhm` / `rhp` and `harness/tools/probe/rhm_full.py --json`,
the script through `scb_semantics.py --pseudo`, and harness runs of the release binary (seed 1) with
scratch scripts kept out of the repository (Provenance).

## 1. The headline (observed)

- The script's victory test is one integer: `CheckVictoryCondition` returns mission variable 2 (`n2(2)`),
  selecting debriefing 0 when it is 1. Variable 2 is set to 1 by exactly one handler: the `IsTaken` of the
  **servant's son's scroll** (a `SKRO` record at map (253,380)), which also adds primary objective 2,
  completes objective 1 and shows text 11. Nothing gates that handler: no objective, flag or message is
  checked before it runs.
- **The engine wins the mission through play today**: three Enter presses (the briefing), one left click
  on Robin, one walk order to (253,380). The mission is won at world tick 3083 (51 s at 60 Hz): `debug.vm`
  reports `mission_won` true, `debriefing` 0, and the app shows the won debriefing page; the recorded
  replay (28 events, 7 checkpoints) plays back with no divergence (section 5).
- The outcome is **tainted** (ADR-0008): fourteen assumptions are recorded at the win (section 5.3), and
  two of them are load-bearing: the scroll pickup rule (`scroll_pickup`, 24 map px, hypothesis) and the
  fact that the keep's interior is reachable on foot because the engine has no doors or layers (natives
  186 / 191 are stubs, `rhp.md` "Layer transitions ... are still ignored").
- **A binding error hides the intended path**: the engine lays the `SKRO` (scroll) block before the `ZORG`
  block in the element table; the data says the opposite (section 2). Every H01 scroll index is off by
  11, so the level's `Initialize`, which deactivates seven `ZORG` pick-up items, deactivates seven scrolls
  instead - among them the **servant's scroll** that completes objective 0 and the two beggar-tip scrolls
  that add objectives 3 and 4. Objective 0 cannot be completed through play until this is fixed; it is
  the first item of the work list.

## 2. The element table: `ZORG` items precede the scrolls (inferred, confidence high)

`sherwood-hub.md` 4.3 placed the `SCOT` slots after `ZORG` and `TING` from four exact self-references;
the order *between* `SKRO` and `ZORG` was not observable there (only their sum places `SCOT`) and the
engine chose `.. BOOM, SKRO, ZORG, TING, SCOT`. Four independent observations fix the order as
`.. BOOM, ZORG, SKRO, TING, SCOT` (the chunk order of the `.rhm` file itself: `POUF BOYZ ZORG HIRN RAIL
SKRO TING GULP CAVE`):

1. **Positions.** The eleven `ZORG` records of H01 sit next to the scroll or actor that the script
   pairs them with, when the `ZORG` block is read at 100..=110:

   | Element (corrected) | `ZORG` position | The script's use | Nearest paired element |
   |---|---|---|---|
   | 102 | (1586,270) | activated (`n114`) by the `IsTaken` of the arrows scroll | scroll 124 at (1618,280), 33 px |
   | 104, 105 | (520,1386), (572,1388) | activated by message 3 (west-tower area); 105 is polled with `n235` for the steward sub-goal | civilian 50 at (557,1388), the hidden actor message 3 activates |
   | 106 | (2445,801) | activated by the pick-up tutorial scroll | scroll 119 at (2459,770), 34 px |
   | 108 | (2029,1008) | activated by the money scroll | scroll 112 at (2070,1013), 41 px |
   | 109 | (1874,962) | activated by message 7 (the hall) | scroll 123 at (1845,1162) area; location point 12 (1836,976) |
   | 110 | (135,340) | activated by the poor man's scroll | scroll 125 at (97,384), 58 px |

   Under the engine's order the same immediates address scrolls: the level would deactivate the
   servant's scroll (102), the climbing / jump / knock-out tutorial scrolls (104..=106), the pick-up
   tutorial and both beggar-tip scrolls (108..=110), and the arrows scroll would re-activate the
   servant's scroll: none of it coherent.
2. **Scroll states.** Natives 193 / 194 (element state, "on scrolls and zones" per `scb.md`) are called
   in H01 on 111 (state 3 when the training ends), 120 / 121 (bit 2 added when the steward / knight
   sub-goal completes), 122 (state 3 in the drawbridge-tower zone) and 123 (2 / 3 by message 7). With
   `ZORG` first these are the archery-start, steward-tip, knight-tip, drawbridge-tip and hall-opening
   scrolls: each state change marks the scroll whose tip was just fulfilled. Over the corpus (18
   missions with both blocks, scratch probe `zorg_order_check.py`): **102 of 102** native-3 immediates
   passed to 193 / 194 fall in the scroll range with `ZORG` first, 27 of 102 with `SKRO` first; the
   immediates passed to 113 / 114 / 235 fall in the `ZORG` range 303 times against 107.
3. **Walks to scrolls.** Message 5 sends the servant (53) to element 113 with `n233`, message 12 sends
   the son (54) to 114, message 10 the persecuted civilian (52) to 125: with `ZORG` first each actor
   walks to the scroll that carries his own dialogue (the servant's scroll at (1173,836), the son's at
   (253,380), the poor man's at (97,384)); with `SKRO` first the servant walks to the arrows, the son
   to the poor man's scroll and the civilian to an inert entry.
4. **The oracle.** `combat-measurements.md` (scene) saw, right after the briefing, a ribboned scroll at
   client (796,383) and a second pickup at (812,302) = map (2221,1383) and (2237,1302) with the start
   camera at (1425,1000): the knock-out and climbing tutorial scrolls (117 / 115 corrected), which the
   engine's table deactivates at load (`debug.vm.scrolls` reports engine elements 104 and 106 inactive).

Corrected H01 table: map elements 0..=49, civilians 50..=56, soldiers 57..=94, objects 95..=99, **`ZORG`
items 100..=110, scrolls 111..=125**, the hero's slot 126, polygons 127..=138. The scroll classes are bound
by name, so `IsTaken` still fires on the right scroll today; only the index-addressed natives (113 / 114 /
193 / 194 / 233 / 235) hit the wrong rows. Eighteen missions have both blocks (every town mission, S03,
the tutorial ambush and Tac21); Sherwood has no `ZORG` and the outro no `SKRO`, so `sherwood-hub.md` 4.4
is unaffected. `ZORG` records are therefore **pick-up items** (purses, arrows: the `rhm.md` "Bonus"
hypothesis, now supported by the pairing above); their fields `unknown_a` / `unknown_b` (kind, amount?)
stay `unknown`.

Indices below are the **corrected** ones; the engine's current index is given in brackets where it differs.

## 3. The objectives (observed from the script; roles paraphrased from TEXT 1000283)

The level adds objective 0 at load; the others are added by scroll pickups, so their order in play is the
player's. `CheckVictoryCondition` reads none of them: objectives are display state, the win is variable 2.

| # | Kind | Role (paraphrased) | Added by | Completed by | Natives / events read | Engine today |
|---|---|---|---|---|---|---|
| 0 | primary | get into the castle and find the godfather | `PostInitialize`, `n26(0, 1)` | `n27(0)` in the `IsTaken` of the **servant's scroll** 113 [102] at (1173,836); the handler also adds objective 1, reveals the son (`n99(54)`, stub), sends message 14 (a three-page cutscene: pages 8..=10, camera to point 9 (276,352), the son walks there (`n45`), message 12 sends him on to his scroll 114) and makes the servant walk to Robin (`n233`) | scroll pickup (`IsTaken`), 26, 27, 99, 109, 233, 45, 133, 203 | **No.** (a) The scroll is inactive: the level's `Initialize` deactivates `ZORG` 102 (the arrows item) and the engine reads 102 as this scroll (section 2); (b) (1173,836) is not walkable geometry (`debug.nav`: `geometry_walkable` false, no nearest walkable cell): the hall interior is a `WOAW` layer the navigation does not enter. The servant himself (53, hidden at load) is activated only by message 5 (the great-hall zone, polygon 7) but is not needed for the pickup. |
| 1 | primary | the son waits in the village to help Robin leave | the servant's scroll (above) | `n27(1)` in the `IsTaken` of the **son's scroll** 114 [103] at (253,380) | scroll pickup, 26, 27, 202, 1, 233 | **Yes**, reached and taken (section 5); today it is completed *before* it is added (`counters.objective_done_before_added`), since objective 0's scroll is unreachable. |
| 2 | primary | the son leads Robin to safety once told | the son's scroll | never marked done: the same handler sets variable 2 = 1, `CheckVictoryCondition` returns 1 next tick and selects debriefing 0 | 26, 202, 1, 2, 28 | **Yes** (the observed win). |
| 3 | secondary | the steward's ill-gotten money could serve Robin | `IsTaken` of the **steward-tip scroll** 120 [109] at (941,1192), only while `n235(105) == 0`; opens a cutscene (freeze by message 13, camera to point 3 (565,1398), message 3, highlights on soldier 79 and civilian 50, page 4) | `Hourglass`: `n235(105) == 1` (the purse item at (572,1388), activated by message 3) then `n27(3)` and scroll 120's state += 2 | 235 (item taken), 26, 27, 193 / 194, 144 / 145, 243 | **No.** 235 is an *effect stub* (arity 1, returns a value, result 0 with `stub_result` recorded): the purse can never read as taken; `ZORG` items are `Unmodelled` (no position, activity or taken flag in the VM); there is no pick-up action (manual p. 28: a context action over an item; tutorial texts 15 / 17). Today the tip scroll itself is deactivated at load (section 2), so the objective is not even added. |
| 4 | secondary | the knight was bribed: rob him | `IsTaken` of the **knight-tip scroll** 121 [110] at (1288,982), only while `n118(78, 1) != 0` (opcode 0x28, low confidence: taint `opcode 0x28`); cutscene: page 7, camera to the knight, highlight | `Hourglass`: `n118(78, 1) == 0` then `n27(4)` and scroll 121's state += 2 | 117 / 118 (element attribute 1 of the knight = element 78, `BORG` 21 at (861,1135), profile 24, 105 hp in the engine), 26, 27 | **No, and wrong today**: attribute 1 starts at 0 (nothing in the script writes it), so `Hourglass` completes objective 4 on tick 1, before it exists (`objective_done_before_added` = 1 two ticks after the briefing; the only candidate, since 235 returns 0). The knight's record carries `unknown_0x23` = 300, the only value above 100 in H01 (six others in the corpus: 120..=1500; every other record 0 or 10..=100): **hypothesis** - the field is the money the actor carries, attribute 1 is initialised from it and cleared when a player character searches the knocked-out knight (the manual's search action, p. 28; the sum going to the player's money, natives 236 / 237). Needs the oracle (rob the knight, watch the money counter). The scroll's position (1288,982) is not walkable geometry either. |
| 5 | secondary | end the archers' training session (one of Robin's arrows in a target does it), which frees the way to a beggar | `IsTaken` of the **archery-start scroll** 111 [100] at (2215,1094), only while variable 1 == 0; cutscene: freeze (message 13), camera to the sergeant 71, remark 61, pages 5 and 22, message 11 (shooting timer), unfreeze | message 1 to the level, sent by a target's `ActivatedByArrow(shooter)` when `n79(shooter) == 1` (the four targets are objects 95..=98 at (2116,914), (2206,914), (2305,927), (2381,942)); the level then `n27(5)`, variable 1 = 1, groups the six archers under the sergeant (218, stub), sends them along paths 66..=71 and the sergeant along 38 with text 6. **Alternative end**: an archer's `ActionChange(_, 141)` (he noticed something) sends message 2: variable 1 = 1 and the same walk-off, *without* `n27(5)` - the objective then stays open for good | object hook `ActivatedByArrow`, 79, 27, 1, 218, 132, 202; the bow; arrows (items) | **No.** (a) No bow: the icon, the aim, the arrow counter and the projectile do not exist (`combat-measurements.md` 2: 0 arrows at start; the arrows lie further in - items of section 2, which ones is `unknown`); (b) `World::vm_activated` (the `ActivatedBy*` hook) has no caller: no object can be activated; (c) the archers' own shots (native 59) are stubs, presentation only; (d) **the engine ends the training on the approach**: in both tour runs an archer noticed the walking Robin before he reached the scroll (`perception` recorded, message 2 delivered, variable 1 = 1 at world tick ~280), so the scroll's `IsTaken` added nothing. The scroll lies 60..=100 px from the archers; the original lets the player take it during the training (the objective text presumes it), so the view-cone hypothesis (45 degrees, 250 px, occluders ignored) or a "busy" state of scripted actors is wrong here. |

Other script reactions that are not objectives but belong to the tutorial and are reachable today: the
knock-out of soldier 87 (a corridor post) makes `Hourglass` give him and civilian 51 new paths
(`test_knock_out_from_behind_puts_the_soldier_out_of_action`, passes on this binary: `out_of_action_true`
> 0, `knock_out` and `profile_stats` recorded); a courtyard lancer 75..=77 out of action sends civilian 56
along path 57; the persecution zone (polygon 11, the south-east corner) and the four soldiers 81..=84
(paths 25..=28) form the beggar sub-plot of texts 3 and 19. The drawbridge mechanism (object 99 at
(1217,1732), `ActivatedBySword`: animation 160, message 15, patches 3 / 4) needs the same object hook as
the targets.

## 4. The canonical input (positions from the mission file, element indices corrected)

### 4.1 The shortest script that wins today (observed, replayed)

Session ticks; screen coordinates are the 1024x768 viewport; the camera starts at (1425,1000) and scrolls
8 px per tick on a held arrow key (observed in the recording).

```
tick 0..2   Enter, Enter, Enter                       the three briefing pages (world does not tick)
tick 3      left click at screen (512,384)            map (1937,1384): select Robin
tick 5..159 hold Left 5 x 30 ticks                    camera to x = 225
tick 160..252 hold Up 3 x 30 ticks                    camera to y = 280
tick 253    left click at screen (28,100)             map (253,380): walk order to the son's scroll
tick 254..3096 wait                                   Robin arrives, IsTaken fires, variable 2 = 1
world tick 3083: mission_won, debriefing 0, the won page is shown
```

Recorded as a `ReplayV1` file (28 events, checkpoints every 600 ticks; header ruleset 13 / hash schema 15,
seed 1) and played back: 7 checkpoints equal, `first_divergence` null. The route (Robin's position every
300 ticks): (1937,1384) -> (1850,1407) -> (1439,1518) -> (1027,1630) -> (616,1741) -> (205,1853) ->
(112,1517) -> (269,1123) -> (606,903) -> (728,638) -> (341,460): out of the yard to the west, north along
the west edge, then east and back west through the keep's upper court - a 3750 px walk (469 grid cells)
that crosses no zone polygon with a handler (no message 3 / 5 / 6 / 7 was delivered; the 573 messages
delivered are the training loop's). Four soldiers of the keep (entities 36, 40..=42) were `alerted` on
the way (they charge and stand by the hero: the engine's soldiers do not start fights), no page other
than the debriefing appeared, the son (entity 5) was walking to Robin when the page came up.

### 4.2 The intended path once the gaps are closed (inferred from the script and the zone polygons)

The keep is gated in the original by doors the script closes at load (`n191(0, ..)` on doors 8, 20, 21,
23, 25, 37; locks on 20 and 28) and opens area by area (message 3 opens 20, message 6 opens 25, message 7
opens 23); which door stands where is `unknown` (the map's `FARM` chunk, `rhp.md` "Raw chunks", is not
parsed). The zone polygons (`GULP`, script polygons 0..=11 = elements 127..=138) and points are known:

```
1. Enter x3; select Robin at (1937,1384).
2. Walk into the west-tower zone (polygon 2: (834..918, 1210..1274)) -> message 3: door 20 opens,
   civilian 50 and soldier 79 appear, purse items 104 / 105 activate, patch 1.
3. (Objective 3) Take the steward-tip scroll 120 at (941,1192) -> cutscene, objective 3 added; pick up
   the purse item 105 at (572,1388) -> Hourglass: objective 3 done.
4. Walk into the great-hall zone (polygon 7: (1250..1450, 628..728)) -> message 5: the hall's lights,
   the servant 53 appears at (1266,776) and walks to his scroll 113.
5. Enter the servant's zone (polygon 10: (1196..1275, 703..763)) -> n103 (stub) and page 18 (his introduction).
6. Take the servant's scroll 113 at (1173,836) -> objective 0 done, 1 added, message 14: pages 8..=10,
   the son walks to point 9 (276,352) and on to his scroll 114 (message 12).
7. (Objective 4) Take the knight-tip scroll 121 at (1288,982) -> objective 4 added; knock out the knight
   78 at (861,1135) from behind and search him -> attribute 1 = 0 -> objective 4 done.
8. Walk to the son's scroll 114 at (253,380) -> objective 1 done, 2 added, text 11, variable 2 = 1 ->
   CheckVictoryCondition = 1, debriefing 0.
```

Objective 5 (the archery yard, north-east of the start: scroll 111 at (2215,1094), targets 95..=98,
archers 68..=74 with the sergeant 71) is independent of the path above and needs the bow.

## 5. What the current binary did through play (observed, seed 1)

### 5.1 The win run

`debug.vm` right after the briefing (world tick 2): objectives `[0]`, texts `[]`, variables 1..=3 `[0,0,0]`,
`objective_done_before_added` 1 (objective 4, section 3), stubs `{186: 2, 191: 6, 198: 5, 235: 2}`,
assumptions `stub_result 186 / 191 / 198 / 235, policy 134 / 193 / 194 / 196, tick_rate,
action_change_order`. At the win (world tick 3083): objectives `[0 (open), 2 (open)]`, variables
`[0,1,0]`, `mission_won` true, `mission_lost` false, `debriefing` 0, `objective_done_before_added` 2,
`out_of_action_true` 0, no fault, no trap, no budget abort; stubs `{235: 3083, 49: 188, 51: 190, 59: 190,
186: 2, 191: 6, 198: 5}` (235 is polled once per tick by `Hourglass`; 49 / 51 / 59 are the training loop's
animations and shots); scrolls: engine 103 (the son's) taken, engine 102 (the servant's) inactive since
load, 113 / 114 active.

### 5.2 The tour run (secondary objectives)

- Archery-start scroll (engine 100 at (2215,1094)): arrived at world tick 285; variable 1 was already 1
  (`perception`: an archer's action 141 reached its `ActionChange`, message 2 delivered at ~tick 280),
  so `IsTaken` added nothing; the beggar tutorial scroll on the way showed its page (highlight 243,
  camera, page 16).
- Knock-out from behind: the repository's data test on soldier 87 passes on this binary (states `patrol ->
  knocked_down -> lying`, action 47, `out_of_action_true` > 0, the script gives paths 65 / 58 and the running
  gait); an attempt on a *patrolling* courtyard lancer (77) failed because he had walked on between the
  observation and the click (the click became a ground order): an attack order needs the sprite under the
  pointer at release, as in the original.
- Powerful blows: `test_two_powerful_blows_kill_the_soldier_the_script_polls` passes on this binary.

### 5.3 Assumptions at the win and what they mean

`stub_result 49 / 51 / 59` (training-loop animations and shots: presentation), `186 / 191` (door locks
and states: *load-bearing* - the keep is open because doors do not exist), `198` (blip flags), `235`
(the purse predicate: objective 3 never completes), `policy 134` (AI locks halt walks), `193 / 194`
(scroll states stored blindly), `196` (action availability ignored), `tick_rate` (the `Hourglass` unit),
`scroll_pickup` (*load-bearing*: the 24 px approach rule that fired `IsTaken`), `walk_completion` (a
cutscene barrier released by a walk that did not arrive: the training loop's sergeant walks),
`action_change_order` (the parameter order of `ActionChange`). Not recorded: `perception` (no soldier of
the route alerted by sight reached a handler), `knock_out`, `melee_*`. So the win is reproducible and
deterministic but not authoritative until doors / layers exist and the pickup rule is measured.

## 6. Engine work list (ordered by dependency)

| # | Item | Depends on | Effort | Unblocks |
|---|---|---|---|---|
| 1 | **Element table: `ZORG` before `SKRO`** in `MissionBinding::from_mission` (`crates/opensherwood-script/src/lib.rs`); update `scb.md` "Index spaces" and the H01 walkthrough's scroll indices (+11), `sherwood-hub.md` 4.1 / 4.3, `rhm.md` (`ZORG` = pick-up items); re-pin `EXPECTED_AT_LOAD` (`test_script.py`), `gamedata.rs`, the 300-tick strict run (load-time stub counts change: seven `Unmodelled` entries receive 113 instead of seven scrolls; the hub is unaffected) | - | 0.5 day | 3, 5, 6; objective 0's scroll becomes active at load |
| 2 | **A real win test**: `test_first_mission_is_won_through_play` from section 4.1 (select, walk to (253,380), expect `mission_won`, debriefing 0, the debriefing screen), tainted as recorded; `docs/harness.md` "Scripts" no longer says no mission can be won through play; the `debug.vm {win}` shortcut is removed (the flows are driven through play in `test_win.py`) | - (re-check after 1: the son's scroll becomes 114, still active) | 2 hours | the roadmap's "first mission won" checkbox, honestly worded |
| 3 | **Done (2026-09-05, ruleset 15; `rhm.md` "`ZORG`", `scb.md` "Pick-up items": kind 0 arrows / 9 purse, the stack from `unknown_b`, a click on the item orders the walk, taken within the scroll radius, 235 reads the taken flag, HUD counters, placeholder discs until the `BONUS_*` banks are drawn; `test_taking_the_stewards_purse_completes_the_third_objective` completes objective 3 through play, tainted `item_pickup`).** **Pick-up items** (`ZORG` as `Element::Item {x, y, kind, active, taken}`): 113 / 114 on them, 235 reads `taken` (arity 1, returns int), a pickup action (the manual's context action over an item: click, or an approach rule to measure alongside the scroll rule), the HUD counters (arrows, purses) fed by `kind`, taint `item_pickup` until measured | 1 | 1-2 days (+ 0.5 day oracle: the pickup gesture and radius) | objective 3 (with a walk to (572,1388) after message 3), the arrows for 6 |
| 4 | **Layers and doors**: enter the `WOAW` layers (Lincoln: 14 layers, 466 areas) through their transitions so interior points such as (1173,836), (1288,982), (1618,280) are reachable; parse the `FARM` door table for native 4 and make 191 (open / close) and 186..=189 (locks) real so the keep is gated as the script gates it; `debug.nav` reports the layer | - | 1-2 weeks (format work on `FARM` / ` AZ ` plus navigation) | objectives 0 and 4 through the intended path; every town mission |
| 5 | **Per-actor money**: attribute 1 initialised from `BORG` `unknown_0x23` (hypothesis: oracle first - rob the knight in the original, watch the money counter and attribute polling), a **search** context action on a knocked-out or dead actor moving the sum to 236 / 237 and zeroing attribute 1; until then a stub value of 300 on the knight would at least stop objective 4 completing on tick 1 | 1, 4 (the tip scroll's position); the knock-out exists | 1 day + 0.5 day oracle | objective 4 |
| 6 | **Bow and object activation**: arrows as items (3), the bow order (icon under the portrait, aim, click when the pointer turns green, Ctrl-move; `ui-flow.md`), a projectile with a hit test against the object polygons (`BOOM` anchors), the `ActivatedByArrow(shooter)` hook through the existing `World::vm_activated`, the target's animation 210 (presentation); the sword on the drawbridge mechanism (`ActivatedBySword`) rides on the same hook | 1, 3 | 1 week (+ oracle: draw time, flight, damage) | objective 5, the drawbridge, every `ActivatedBy*` handler of the campaign (418 arrow handlers) |
| 7 | **Perception at the archery yard**: measure the cone (stealth 7.2) and whether scripted actors in a sequence (the training loop) perceive at all; the engine must let Robin walk to (2215,1094) unnoticed or objective 5 can never be added | oracle capture | 1 day oracle + 1 day engine | objective 5 |
| 8 | Cutscene fidelity for the path's handlers: 99 (reveal), 103, 218, 130, 137, 49..=53 animations, 54 / 55 / 226 presentation, 62 / 69 / 243 | - | as they come | presentation only; not needed for the win |

Items 1 and 2 give an honest, reproducible win within a day; items 3..=7 are what "won as designed" (all
three primary objectives completed in order, the secondary ones possible) requires, about three to four
weeks of engine work plus three oracle sessions.

## Provenance

- Data: `C:\Users\przem\source\gamedata\robinhood` (GOG English build, executable SHA-256
  `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`), `DATA/Levels/H01_Lin_VL.{rhm,scb}`,
  `lincoln.rhp`, `2047/data/Text/Level.res` (TEXT 1000283 read with `harness/tools/original/sres_text.py`
  and paraphrased), `Manual.pdf` rendered with PyMuPDF and read by eye (printed pages 14-18, 24-28).
- Tools: `opensherwood-tools rhm` / `rhp` (release build of commit `76e7fc1`), `harness/tools/probe/
  scb_semantics.py --pseudo`, `rhm_full.py --json`; scratch scripts in the session scratchpad (not
  committed): a `ZORG` / `SKRO` order tally over the 18 missions with both blocks, five harness probes
  (`debug.vm`, `debug.nav`, `observe`, `replay.start` / `stop` / `play`, `capture`) driving
  `target/release/opensherwood.exe` with `--rpc stdio --headless`, seed 1, artifacts under the scratchpad.
- Engine runs: the win run and its replay, the tour run, and `pytest harness/tests/data/test_mission.py -k
  "knock_out_from_behind or two_powerful_blows"` (2 passed, 21.5 s) on 2026-09-05.
- Who: analyst session (a Claude agent); no engine code read beyond `natives.rs` tables, the
  `MissionBinding` doc comment and the `vm_activated` signature, none written.
