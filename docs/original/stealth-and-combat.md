# Stealth, perception and melee: what the data and the manual say

Status: **data-file observation and manual reading only** (analyst session 2026-09-03, ADR-0003: no
executable, disassembly or debugger involved; the original was not run for this document). Every
claim carries a status: `observed` (read directly from a file or from the manual), `inferred` (a
reading of observed data that fits every case found, with the evidence), `hypothesis` (fits, but an
alternative reading exists), `unknown`. Numbers quoted are counts, ranges and field values of the
player's own data files; no game text is reproduced, the manual is paraphrased and cited by its
printed page number (`Manual.pdf` page = printed page + 2).

Scope: what an implementer needs to finish the first mission (`H01_Lin_VL`: Robin alone, blue-tier
soldiers) - (1) how NPCs perceive and react, (2) which combat / knock-out states exist and which sprite
actions and timings go with them, (3) movement postures and speeds. Section 7 lists what only an oracle
trace can settle and how to capture it.

Sources: `Configuration/profile.cpf` ([../formats/profile.md](../formats/profile.md)), the `.rhs`
animation tables ([../formats/sprite-animations.md](../formats/sprite-animations.md)), the mission
files and their rail programs ([../formats/rhm.md](../formats/rhm.md)), the compiled scripts
([../formats/scb.md](../formats/scb.md)), the manual, the first mission's tutorial texts (`Level.res`
TEXT 1000105 strings 12..22, see [campaign-flow.md](campaign-flow.md)), and the console command list
([console-commands.md](console-commands.md)). Probes: `harness/tools/probe/cpf_stats.py`,
`anim_actions.py`, `scb_semantics.py --pseudo`, `rhm_full.py`.

## 1. Rules from the manual and the tutorial texts (observed, paraphrased)

Perception (manual p. 20-21, 16, 23-24; tutorial strings 14, 15, 21, 22):

- Soldiers and civilians only see what enters their **field of vision**. One character's cone can be
  displayed at a time (Eye icon top-right or hold the field-of-vision key); it is not shown while the
  character is still an unidentified black **silhouette**. A player character or an object such as a
  body that passes through the cone has a good chance of being seen and may cause the **alarm**.
- Unknown characters start as black silhouettes; identifying one needs a character close by, a beggar's
  information or a special ability (the `Blip00.rhs` profile, a black copy of the soldier animation set
  with 155 blocks, is that silhouette; scripts reveal actors with natives 99 / 197 / 198, see scb.md).
- **Emoticons** over NPC heads: question mark = noticed something, not yet understood; exclamation mark =
  alarmed; sun = saw something attractive and will go for it (a purse); storm cloud = angry (stung by a
  wasp); rain cloud = saw something uninteresting; Z = asleep (loud noises wake him); spiral = drunk
  (slower, less alert).
- Running makes a lot of noise (p. 16); crouching characters are less visible but slower (p. 16);
  hidden characters (under leaves or a cloak at mission start) are not noticed until given an order
  (p. 21). The console has `NOISE` (walk-noise radius of each PC) and `PCSIGHT` (PC view cones), so
  both a **noise radius** and **sight cones** exist as engine quantities.
- Soldier kinds (p. 23-24, paraphrased): lancers are weak conscripts with long spears (hard to close in
  on); halberdiers stand guard and rarely leave their post, sending others to look; swordsmen carry a
  large shield that also protects a comrade (an archer) from arrows; archers are weak in melee but deadly
  at range; crossbowmen are braver and mail-armoured; officers are tough, hard to knock out, prefer to
  send their men and keep them from being tempted by gold and beer; knights (two-handed sword) wait
  until their troops are dead before engaging and resist arrows; cavalrymen can only be hurt by blunt
  weapons; civilians either ignore the player or raise the alarm, and must not be killed. Colour = tier:
  blue, yellow, orange, red, black in rising strength; green = the allies-to-be of one mission.
- Stimuli per soldier kind (annex, p. 34): purse / beer / apple / whistle - lancer: purse, beer,
  whistle; halberdier: beer; swordsman: purse, apple, whistle; archer: purse, whistle; crossbowman:
  beer, apple; officer: beer, whistle; knight: apple; cavalryman: apple, whistle. Tutorial string 15:
  a thrown purse draws some soldiers (lancers) and can start a brawl among them.

Combat and states (manual p. 26-28, 18; tutorial strings 14, 21, 22):

- Every character has **life points**; a wound costs some, the amount shows briefly over the head; at
  zero the character dies. The portrait's parchment burns as health drops. **Energy** (blue gauge under
  the health bar in combat) is spent by special attacks; at zero the character is worn out, stands with
  stars over his head and can neither attack nor defend until rested - enemies rest too.
- A blow can put a character **out of action without killing** him (typical of staff and mace); the
  victim stays **knocked out** until the stars over his head disappear or someone revives him; enemies
  revive their own. The knock-out chance rises when the victim's health is low. Weapons differ in power
  and in how easily they knock out.
- Knock-out order (tutorial 14): select the fist icon, click the victim; the size of the stars shows the
  remaining unconscious time. Context actions on a body (p. 28, cursor changes over a valid target): tie
  someone up, finish off an unconscious enemy, transport (carry), revive, search; also pick up, activate
  mechanism, knock over boulders, pick locks. Which hero can do which is the table on p. 32-33 (Robin:
  fight, climb, jump, be given a leg up, pay beggars, search, activate mechanisms, archery, punch /
  knock-out, throw purse; tie-up = the friar, the craftsman, the moustached merry man; finish off = the
  red-clad swordsman and the aggressive merry man; transport = the big man and the strong merry man;
  revive = the big man, the lady, the aggressive merry man).
- Melee (p. 26-27, tutorial 21): click an enemy to fight (double-click too); click your own character
  to parry; hold the left button and draw a figure for a specific attack. Ten attacks are listed: quick
  jab (click, or a stroke backwards), slow powerful blow (stroke forwards), attack left / right (stroke
  left / right), half-circle left / right (counter-clockwise / clockwise half circle), full circle left /
  right, the finishing blow (an infinity sign: very slow, very powerful; the tutorial says to use the
  horizontal figure of eight against strong enemies); right-click = defensive stance (not held forever).
  Unselected characters fight on their own. A fighting character has a red circle instead of green.
- Bow (tutorial 22, p. 26): click the arrow icon over the portrait, aim at the target, click when the
  cursor turns green; arrows must be gathered first; Ctrl-click moves first and shoots on arrival;
  right-click on the icon drops one arrow (double right-click five).
- Death (p. 28, 18): a dead character's portrait becomes inaccessible; a dead main hero without a clover
  loses the mission; with a clover he falls wounded with stars and can be revived by a comrade via the
  clover icon; a dead minor character can be replaced from the camp.
- Principal enemies (p. 25) are immune to everything except Robin duelling them.

## 2. NPC perception and reaction: what the data adds

### 2.1 Patrol programs (`RAIL`, observed / inferred)

The rail command set (rhm.md "Rail programs") already gives face (0x03), wait (0x04 in 1/100 s, unit
unverified), jump-to-point (0x02), stop (0x07) and the two glances (0x0b / 0x0c, always as a pair after a
face + wait, "look to one side then the other" - which is left is not established). New observations
(`rail_ops`, scratch, over the 1638 rails and the `BORG.rail` assignments):

| Opcode | Who uses it (rails assigned to an actor) | Reading | Status |
|---|---|---|---|
| 0x0d `u16` | archers 35, crossbowmen 13, lancers 8, knights 5, halberdiers 5, officers 3, unassigned 28; operand 50 in 51 of 101 cases, else 25, 30, 40, 75, 100, 125, 1000 | **check-for** with a radius: the ranged units scan for enemies within `d` map pixels at the point; the operand set is the distance set the scripts use with native 160 (10..150) | hypothesis (the executable names a check-for command and says friendly soldiers cannot use it; no rail of a merry man carries it, but only three merry men have rails at all) |
| 0x05 `u16, u16` | swordsmen 26, officers 16, green officers 9, crossbowmen 9, lancers 7, halberdiers 7, archers 5, unassigned 17; e.g. (14, 75), (79, 50), (89, 50), (28, 100), (0, 250) | the synchronised check-for (a partner point or rail plus a radius) | low |
| 0x00 (none) | 126 unassigned, officers 15, swordsmen 8 | patrol start / stop marker of script-driven walks | unknown |
| 0x09, 0x0a (none) | lancers 13 / 13, swordsmen 9 / 9, archers 7 / 9, unassigned 80 / 40 | a pair like 0x0b / 0x0c (0x09 ends blocks after face + wait) | unknown |
| 0x06 `u16 x3`, 0x08 `u16`, 0x0e, 0x0f, 0x10 | rare (2..12 uses) | - | unknown |

Consequence for the first mission: four 0x0d and four 0x05 commands sit on the 18 actor-assigned rails
of `H01` (the engine's load coverage note in rhm.md), so "scan for the player in radius d while standing
at the point" is part of the patrol behaviour the level expects; a plain walk-and-wait patrol reproduces
the visible movement but not the detection at the check points.

### 2.2 Per-actor mission fields (`BORG`, observed, meanings unknown)

`unknown_0x1a` (0 / 1 on 127 records; "patrol chief"), `unknown_0x1b` (0 or 1..20, 50, 99, 100),
`unknown_0x23` (0 or 10..100, a percentage), `unknown_i16` (-1 or 7..22) and `members` (the actor's
group) are the per-placement AI parameters. The executable's strings name companies, patrol chiefs and
synchronised paths (executable-notes.md), so `members` + `unknown_0x1a` = the company and its chief is the
working reading. Which field, if any, is a per-actor alertness or sight range is `unknown`; the
percentage `unknown_0x23` is a candidate.

### 2.3 Profile fields that could be perception parameters (`profile.cpf` SD records)

Full field analysis in profile.md "Stat field hypotheses" (added 2026-09-03; readings as intervals with
example values - the per-record columns stay in the analyst workspace). Summary of what varies with
what, over the 7 families x 5 tiers (`cpf_stats.py --tiers`):

- **Rises with the tier, differs per family**: `pre[0]` (30..250: 80 for a blue halberdier, 30 for a
  blue archer, 130 for a blue officer, 250 for the four antagonists; +10 per tier), `pre[1]`, `pre[2]`
  (0..100, +5 per tier), `q1`, `q2` (0..100, +10..20 per tier for the melee families, constant 30 / 10
  for archers and crossbowmen), `ranged` (30..100 for the units with a bow or crossbow set only, 0
  elsewhere), `p4` (35..100 for officers, knights and mounted knights, 95 the trainer, 100 the
  antagonists, 0 elsewhere).
- **Falls with the tier**: the four post words `(purse, apple, beer, whistle)` - see section 5 - and
  `p3` (officers only, 80 down to 0).
- **Constant per family**: `pole` (1 for the two pole-arm families), `rank` (0 soldiers, 1 officers,
  2 knights, antagonists, merry men, trainer), `w0` (1..15, rising with the unit's strength), the flags
  byte, the class id, the two trailing words (`weapon` 0 none / 1 sword / 2 two-handed / 3 pole;
  `armour` 0 / 1 / 2).

**No field reads as a sight range or a hearing radius**: nothing is in the 100..500 px range a cone
would need, and no column is constant across tiers but different for archers (long sight) versus
lancers. The reading that fits is that sight, hearing and alert timers are engine constants (or table B
values, section 5.3) modulated by the tier through `q1` / `q2` or not at all. Status: `inferred` (from
absence); the oracle measurement in section 7 decides.

### 2.4 The alert animation set (sprite data, observed by eye)

All soldier and knight profiles carry a second locomotion set that mirrors the patrol set (walk / run /
sprint with their starts, stops and turn tables) but with the weapon held ready and larger strides; the
frame counts, tick patterns and per-frame advances are read from the player's `.rhs` at run time
(`docs/formats/sprite-animations.md`, "Reading rules"), only the roles are recorded here:

| id | role (`Soldier A00`, direction 4) | patrol twin |
|---|---|---|
| 140 | alert idle: crouched forward, sword and shield up | 0 |
| 141 | **noticed something**: straightens, hand to the helmet, peers | - |
| 142 | **raises the alarm**: hand cupped to the mouth, then the arm thrown up | - |
| 143 | alert walk, weapon ready (faster than the patrol walk) | 6 |
| 144 / 146 | alert walk start / stop | 5 / 8 |
| 151 | alert run (faster than the patrol run) | 7 |
| 145 / 149 / 147 / 148 / 150 | run starts and stops, turn table | 9 / 51 / 11 / 12 / 50 |
| 156 | charge: runs the last steps into a strike | - |
| 170 .. 173 | raises the sword arm and holds it up, lowers it (a signal to the company; swordsmen, the merry men, the trainer and two heroes only) | - |
| 189 | looks around, hands on hips (a check) | - |
| 202 .. 205 | four near-identical short listens / glances (hand to the ear, small turn) | - |
| 206 | points with the arm outstretched, holds, lowers (scripts play it after an archer shoots) | - |
| 209 | second alert idle, crouched behind the shield | 0 |

Reading (`inferred`): the engine has at least two AI postures for soldiers - the patrol posture (0, 6, 7,
10) and the **alerted / fighting posture** (140, 143, 151) that moves faster, with 141 and 142 as the
transition animations that match the manual's question-mark and exclamation-mark emoticons. The scripts
confirm 141 as a state change the engine reports to the script: every archer class of `H01` sends
message 2 to the level from `ActionChange(_, 141)` (scb.md), which ends the archery training, i.e. "an
archer noticed something" ends the tutorial scene. Status of the 141 = noticed reading: `inferred`
(animation content + script use); the exact trigger (sight, noise, both) is `unknown`.

### 2.5 Script natives that touch perception and AI state (from scb.md, with the new uses)

| Native | Use in the scripts | Reading | Status |
|---|---|---|---|
| 134 / 135 | lock / unlock the actor's AI (726 / 347 uses); every enemy is unlocked when it reaches its post | the AI does not perceive or react while locked | medium |
| 140 `(actor, 0/1/2)` | 141 uses with 1, 15 with 0, 1 with 2; 1 right after a new patrol path, 0 when a girl is handed back to walking | movement gait: 0 walk, 1 run, 2 sprint - the three prefix cycles 6 / 7 / 10 | hypothesis (three values, three cycles) |
| 126 `(actor) -> int` | compared with 1 (`== 1` required by zones for the entering PC; `!= 1` makes an archer get a new path) and 2 | an actor status code with 1 = normal / patrolling; 2 and other values = alerted or fighting or out of action | low |
| 128 `(actor) -> bool` | required `== 1` by every zone that reacts to an actor and for an enemy to still count | able to act (not knocked out, tied, dead) | medium-low |
| 177 `(actor, 0/1)` | 1 when an enemy reaches its post / is initialised, 0 when parked | "at post" flag (the AI patrol resumes) | low |
| 228 `(actor, k, ticks)` | k = 1 for 10..100 ticks right after 177 | a timed guard state (stand and watch for n ticks?) | low |
| 219 / 220 `(actor)` | 219 right before an "alert" helper (wake / alert), 220 after 177 and in `ReachPoint` (stop / clear orders) | 219 = alert the soldier by script, 220 = clear its current order | low |
| 130 `(actor, target, 1)` | 3 uses: a soldier and the girl he escorts in `H01`, a "check chase" helper | actor follows / chases target | low |
| `FilterAIEvent(_, event)` | returns 1 by default; classes test event ids 0, 2, 8, 11, 13, 14, 22, 23, 31, 33, 34, 52; crossbowmen relay event 31 to the level as a message; a mechanism object suppresses event 8; two soldiers of a town mission react to event 14 by taking a path | AI stimulus filter: the engine asks the script whether an event (a sighting, a noise, a hit ...) is to be processed; returning 0 suppresses it | medium for the mechanism, ids unknown |
| `ActionChange(_, id)` | ids compared: 137 (objects, most), 141 (archers), 136, 107, 102, 135, 31, 280, 281 | the engine reports the actor's new **action id = sprite action id** (141 noticed, 107 fell backwards, 102 flinched, 136 hidden in leaves, 135 stood up; 137 on objects is not a sprite id) | inferred (the ids match the animation table) |

`AI` and `BIG BROTHER` on the console draw the AI state and per-actor data of the original: the
cheapest way to learn the state names and the cone geometry without the executable (section 7).

## 3. Combat and neutralised states

### 3.1 States the scripts distinguish (observed uses, inferred meanings)

| Native | Uses | Evidence | Reading | Status |
|---|---|---|---|---|
| 90 `(actor) -> bool` | 159, almost all `== 1` on enemies in `Hourglass` | the designers' variable names for "enemy out of service" flags are set from it; the "all enemies out of action" helpers count an enemy while `n128 == 1 and n240 == 1 and n90 == 0` | **out of action** (knocked out, tied or dead) | medium |
| 87 `(actor) -> bool` | 25 | an "is any green soldier dead" helper sets its flag when `n81 == 1` (soldier) and `n87 == 1`; an "officer dead" flag is set on `n90 or n87`; victory / objective code tests it on named enemies | **dead** | medium |
| 89 `(actor) -> bool` | 3 | a courier counts as lost when `n89 == 1` or absent; an objective in a town mission needs `n89(x) == 1` on a hidden actor inside a zone | a further neutralised state (tied up or netted / captured) | low |
| 88 `(actor) -> bool` | 1 | only in the "is neutralised" helper, which returns `n88 or n87 or n89 or n90` | the fourth neutralised state | unknown |
| 85 `(actor) -> bool` | 225 | helpers skip actors with `n85 == 1`; PCs absent from the team | unusable / removed / absent | medium |
| 240 `(actor) -> bool` | 22 | with 90 above | present on the map | medium-low |
| 102 `(actor, 10, 1)` | 39, in the "kill" and "kill everything in the zone" helpers, PCs are deactivated (113) instead; `(x, 100, 1)` twice; 229 (drop out) precedes it | inflict damage of the given size (10 kills any soldier only if the unit is not hit points - so the second argument is more likely a damage *kind* or a percentage) | low |
| 59 `(actor, k, target)` | 26: `k` = 1 (11), 4, 2, 5, 6, 17, 18, 0; target = element index via native 10 or an immediate 0..15 (a direction when there is no target) | archer shoots: `k` arrows or a shot kind, at an element or in a direction | low |

So the engine has at least the states **normal, out of action, dead** plus two more neutralised
states (candidates: knocked out vs. tied up vs. caught in a net; the manual has all three) and the
transient ones the manual describes (worn out with stars, asleep, drunk, hidden, carried). The
tutorial's "stars show the remaining time" gives the knocked-out state a **timer**; the manual says
enemies revive their own (an NPC action on a body) and tying up makes the state permanent.

### 3.2 Sprite action ids of the combat and state animations

The ids, their roles and the per-family presence matrix are in sprite-animations.md ("Presence per
family", "Ids identified by eye", "Combat, state and stealth ids"); frame counts, ticks and advances
are read from the files by the rules given there and are not repeated here. What the presence pattern
says about the game's rules (`anim_actions.py --families`, matched against the manual's ability table
p. 32-33; `inferred` unless marked):

- **Knock-out blow 123**: of the ten heroes only Robin and the big man have it, and the manual gives
  the punch to exactly those two; all soldier kinds, officers and antagonists have it too (they knock
  out as well); knights and civilians do not. Its block displacement puts the victim's spot 30..35 px
  ahead (`observed`).
- **Bow set 85..94** and its hit / fall set 111..117: the four hero archers of the manual (Robin in
  both outfits, the lady, the moustached merry man); among the soldiers: swordsmen, archers,
  crossbowmen, the two merry-man recruits and the trainer, never halberdiers, lancers, officers or
  antagonists (a superset of the kinds with a non-zero ranged skill in section 2.3: the swordsman and
  the staff recruit carry the bow animations without the skill).
- **Search 122 / 282**: Robin and the lady, the only heroes the manual lets search; soldiers have 122
  as well (their use of it is unknown).
- **Pay the beggar 125**: exactly the six heroes the manual allows (not the merry men); **throw purse
  124**: Robin only; **be given a leg up 128 / 129**: Robin, the red-clad swordsman and the moustached
  merry man, exactly the manual's three (the helper stands 28 px behind); **pick up 126 / 248**: every
  hero.
- **Crouch 13 / 14 / 18 / 16**: heroes only; no soldier or civilian crouches (`observed`).
- **Alert set 140..151, 156** and the **stimuli reactions 164 / 165 / 166 / 169**: soldiers and
  knights only (the knights: 166 only); no hero or civilian has an alert posture.
- **Carried body 118..121** and the laid-out pose 219 exist in the corpse profile and in the civilian
  and soldier sets, so a carried or laid-out body keeps its own sprite.
- **Hidden in leaves 136 / 137**: heroes only.

The state transitions the ids imply (what each id looks like is in sprite-animations.md):

- hit while standing, weapon carried: 40 (a flinch; the same animation draws the weapon); in the
  fighting stance: 104 (stumbles back a step), 102 (short flinch); bow in hand: 111;
- knocked down: 41 forward (struck from behind; the body ends 36 px ahead) / 44 backward (struck from
  the front; 30 px back), with the stance twins 105 / 107 and the bow twins 112 / 114;
- lying: 47 (face down) / 48 (on the back, shield on the chest) / 45 (on the back, arms out), one frame
  each, held by the state machine; stance twins 106 / 108 / 109, bow twins 113 / 115 / 116; 219 laid
  out stiff (after being carried or tied; a corpse);
- get up: 49 (stance 110, bow 117);
- melee: 52 / 53 enter and leave the stance, 54 / 96 fight idle, 55..58 stance steps, 35 / 38 guarded
  advance / retreat (34, 36, 37, 39 its transitions), 59..66 quick strikes, 67 / 69 / 70 / 79 wider
  cuts, 71..74 the sweeping half-circle and circle attacks (two directions x two amplitudes), 75 the
  finishing blow (over the head, ends crouched), 68 and 103 parry / block, 100 + 152..154 the
  soldiers' shield charge (50 px), 97 / 98 the hero's lunges;
- bow: 85 draw, 87 nock, 88 aim, 89 / 92 hold (until the click), 90 / 91 / 93 / 94 release, 86 put away;
- other: 133 struggling on the back (netted), 127 cower / duck, 178 / 179 flung, 250 civilian panic,
  158..160 bend and pick up, 165 / 166 / 169 the beer / apple / purse reactions.

Illustrative timings (the tick half of the timing word summed over the frames; `Soldier A00` unless
stated; sprite-animations.md "Illustrative durations and displacements"): a quick strike 8 ticks, the
finishing blow 30 (Robin), a knock-down 13 (forward) / 10 (backward) plus the one-frame lying pose the
state machine holds, getting up 16, the knock-out blow 12, the standing hit 40 about 20, the fight
idle loop 32. If the tick half counts simulation ticks at the rate native 56 uses (25 per second is the
scb.md hypothesis), a quick strike is 0.3 s and the finishing blow 1.2 s. Frames with a zero tick half
inside timed animations (e.g. the first frame of 49) are read as "no hold" (one tick minimum) -
`hypothesis`.

### 3.3 The melee parameter tables of `profile.cpf`

**Table A** (27 blocks of a 14-word head and ten 16-word records; the column layout with example
values is in profile.md "Stat field hypotheses") is indexed by the **combat class id**: the PC records
carry a `u16` id 1..10 (a permutation of the table positions) and the SD records a class id
0x0b..0x1b; together they are exactly 1..27 = the 27 blocks (`inferred`, exact fit). The head's first
four words are rising distances (the fourth 150 in every block), and the two pole-arm classes are the
only blocks whose first three are the widest: **melee reach bands**, longer for the pole arms. The ten
records match the manual's list of **nine attacks plus the block** in the manual's order
(`hypothesis`, structural): rows 3 / 4, 5 / 6 and 7 / 8 are pairs identical except for a 0 / 1 mirror
flag (attack left / right, half-circle left / right, circle left / right), rows 0 / 1 / 2 are the three
single attacks, row 9 is all zeros with the flag set (the block). Per row the columns read as damage
and a second effect (0..200, e.g. a knock-out chance; the finishing blow has 100 / 100 for Robin), a
hit chance (45..100), the figure class, the mirror flag, three angles in degrees (the swing's arc), a
wind-up and the **energy cost** (0..50, rising with the manual's slow-and-powerful ordering; the block
costs nothing). The head's seven weights after the distances differ per class (all equal for
halberdiers, high for knights, mostly zero for officers): AI attack-choice weights is the reading. All
`hypothesis`.

**Table B** (four records: a word, six triplets, a word, a byte, a word, six triplets, a word) holds
two 6 x 3 percentage grids per record whose rows fall from all-100 to a last row of small values and
whose columns rise left to right; the four records differ in five words only. Six rows by three
columns reads as a percentage by (six tiers or six health bands) x (three difficulty levels); four
records = four difficulty presets or four weapon kinds. `unknown`; listed because "knock-out chance
rises with low health" (manual) needs exactly such a table.

**Knock-out resistance**: `p4` (0 for most kinds; 35..100 for officers, knights and mounted knights,
95 the trainer, 100 the antagonists) is the one field that singles out the kinds the manual calls hard
or impossible to knock out; `p3` (officers only, 80 falling to 0) matches "officers prefer to send
their men" fading with rank. Both `hypothesis`.

## 4. Movement

### 4.1 Postures and cycles (observed tables)

| Posture | idle | walk cycle | run cycle | sprint cycle | who |
|---|---|---|---|---|---|
| upright, weapon carried | 0 (6 frames, 33-35 ticks), 1 / 3 fidgets | 6 (22 frames) | 7 (12) | 10 (16 Robin / 32 soldier) | everyone |
| crouched (sneak) | 14 (6 frames, 33 ticks) | 16 (12-14 frames, 18 ticks) | - | - | heroes only |
| fighting stance | 54 (6, 32) | 35 forward / 38 backward (12 frames) | 156 charge (soldiers) | - | heroes, soldiers |
| alerted (soldiers) | 140 (6, 33) | 143 (22) | 151 (12) | - | soldiers, knights |
| climb | 19 / 22 / 23 | 20 up / 21 down (12 frames, 16 ticks, +-3 per frame, block displacement 45) | | | climbers |

Crouch key / icon (manual p. 16, 31): `c` crouch, `s` stand (set 1). The tutorial and the manual give no
crouched run; the tables have none (16 is the only crouched cycle). Soldiers and civilians have no crouch
set at all, so "crouching" is a player-character posture only (`observed`).

### 4.2 Advance per frame (observed) and what it means for speed (hypothesis)

The per-frame advance of the moving cycles is the same in every NPC profile (soldiers of every kind,
knights, civilians) and larger in the hero profiles, so two vectors describe all 117 files
(`observed`):

| cycle | hero (`RobinHood`) | NPC (`Soldier A00` and every other) |
|---|---|---|
| walk 6 | 4 px per frame x 22 frames = 88 per cycle | 2 x 22 = 44 |
| run 7 | 5 x 12 = 60 | 3 x 12 = 36 |
| sprint 10 | 7 x 16 = 112 | 5 x 32 = 160 |
| alert walk 143 / run 151 | - | 3 x 22 / 4 x 12 |
| sneak 16 | 27 px over 12..14 frames and 18 ticks | - |
| fight step 55, guarded walk 35 | 3 per frame | 3 per frame |
| climb 20 / 21 | +-3 per frame | - |
| run-stops 11 / 12 | decelerating, e.g. 7 6 5 4 3 2 | decelerating, e.g. 6 6 5 4 3 2 0 |

Every walking, running and sprinting frame has a **zero tick half** and only the advance half set; the
sneak cycle has both (ticks 2 2 2 2 1 1 ... and advances 1 2 2 ...); the run-stops have decreasing
advances with ticks 0 0 0 1 1 1. Two readings of the moving frames:

- **A. one frame per tick, advance = pixels per tick**: walk speed = 4 px/tick for the hero and
  2 px/tick for every NPC (100 and 50 px/s at 25 ticks/s), run 5 / 3, sprint 7 / 5, alert walk 3, alert
  run 4, sneak 27 / 18 = 1.5 px/tick, climb 3 px/tick, guarded walk 3 px/tick. The decelerating stops
  (7 6 5 4 3 2 with the last frames held 1 tick) and the sneak cycle (explicit ticks and advances)
  read naturally under A, and "crouching is slower" and "the hero outpaces patrolling guards" come out
  right. Speed then lives in the sprite data, not in the profile.
- **B. distance-timed frames**: the entity moves at a speed set elsewhere and the frame changes after
  `advance` pixels; the profile would then hold the speed. No SD field is a plausible speed (section
  2.3); the PC records end in two words 100 / 80 (Robin), 100 / 100, 50 / 100 (the friar, the lady),
  200 / 200 (the red-clad swordsman) that could be percentages of a base speed (`unknown`).

Reading A was the working hypothesis (the earlier "distance-timed" note in sprite-animations.md is B);
section 7 item 1 measured it. **Measured 2026-09-05 (section 8): the advance is pixels per displayed frame, a walking
frame lasts 46.9 ms (hero walk 85.3 px/s), the crouched cycle runs slower than one frame per walking tick
(17.8 px/s): the frame clock of sprite-animations.md "Reading rules" (a frame lasts its tick half plus one
table ticks of 3 clocks at 64 Hz) gives every measured value.** Either way the *ratios* are data: hero :
soldier walk = 2 : 1, run = 5 : 3, alert walk : patrol walk = 3 : 2.

### 4.3 Orders (manual p. 15-16, 26; ui-flow.md 9.4)

Click = walk, double-click = run (noisy); right-click on the character cancels; Ctrl-click = move then
act; a cross cursor = unreachable; Shift = target behind an obstacle. Native 140's three values (section
2.5) and the three upright cycles suggest walk / run / sprint are the engine's gaits, the double-click
run being either 7 or 10 (`unknown`; the engine currently plays 7, sprite-animations.md).

## 5. Stimuli and the four falling post words

The four `u16` after the flags byte of every SD record (profile.md `unknown_post`) fall by 5 or 10 per
tier from blue to black and are zero for the green spare entries. Matching their non-zero pattern per
family against the manual's stimulus table (section 1) gives:

| family | word 1 | word 2 | word 3 | word 4 | manual: purse / apple / beer / whistle |
|---|---|---|---|---|---|
| halberdier | 0 | 0 | 75..55 | 25..5 | - / - / yes / - |
| swordsman | 50..10 | 100..80 | 0 | 75..55 | yes / yes / - / yes |
| archer | 100..60 | 0 | 0 | 50..30 | yes / - / - / yes |
| officer | 0 | 80..60 | 25..5 | 100..80 | - / - / yes / yes |
| knight on foot | 0 | 75..55 | 0 | 0 | - / yes / - / - |
| lancer | 75..35 | 0 | 100..80 | 0 | yes / - / yes / yes |
| crossbowman | 0 | 50..30 | 50..30 | 0 | - / yes / yes / - |
| mounted knight | 0 | 25..5 | 0 | 25..5 | - / yes / - / yes |
| trainer | 100 | 100 | 100 | 100 | (training dummy) |
| merry men, antagonists | 0 | 0 | 0 | 0 | - |

Word 1 = **purse** and word 3 = **beer** agree with the manual for all eight kinds; word 2 = **apple**
disagrees once (officers) and word 4 = **whistle** twice (the lancer's and the halberdier's whistle
cells are swapped between the data and the manual): 29 of 32 cells. Reading: the four words are the
percentage chance that the unit reacts to the purse / apple / beer / whistle stimulus, falling with
experience - which is what the manual says about officers keeping their men from gold and beer.
Status: `inferred` (medium-high). These matter for the first mission only through the tutorial purse
(string 15 says lancers go and look).

Sprite reactions: 169 (goes and picks up the purse), 165 (drinks), 166 (eats), the sun / storm emoticons.

## 6. What the first mission needs (implementer's checklist)

1. Soldier AI with two postures (patrol: 0 / 6 / 7; alerted: 140 / 143 / 151) and the transitions 141
   (noticed) and 142 (alarm); patrol rails with face / wait / glance / check-for (radius `d`) / stop.
2. A perception model: a view cone plus a noise radius (constants to be measured, section 7), lower
   visibility when crouched (16 / 14), silhouettes until identified.
3. States per human: normal, alerted, fighting, hit (40 / 104 / 111), knocked down (41 / 44 and the
   fighting-stance and bow variants), lying (47 / 48 / 45 ...), knocked out with a timer (stars), getting
   up (49 / 110 / 117), dead (a lying pose kept for good; 219 when laid out), tied (needs a hero who can
   tie: not in `H01`), carried (118-121, not in `H01`), plus the script predicates 90 / 87 / 128 / 240 / 85.
4. The knock-out blow 123 (hero) with its 30-35 px reach; the melee stance 52 / 54 / 55-58; strikes 59-75
   with table A rows as the ten attacks; parry 68 / 103; the bow 85 / 87 / 88 / 89 / 93 / 86 with the
   click-when-green aim; hit points (`pre[0]`: 80 for a blue halberdier or swordsman, 30 for a blue
   archer, 45 for a blue lancer) and Robin's PC values (profile.md).
5. Speeds from section 4.2 on the measured animation clock (section 8.4, sprite-animations.md "Reading
   rules").

## Engine (implemented 2026-09-03; `crates/opensherwood-core/src/ai.rs`, ruleset 9; measured values of section 8 applied 2026-09-05, ruleset 11)

What of section 6 exists, with the constants chosen and their status. Everything is fixed point, part of
every entity (`team`, `ai_state`, `state_ticks`, `last_seen`, `alert_origin`, `attack_target`, `action`,
`hit_points`, `knockout_resistance`, `npc_gait`, `fell_backward`, `heard`), snapshotted, validated and hashed; the
harness reads it through `observe` (`docs/harness.md`, "Stealth layer").

**Measured versus assumed** (after section 8): the movement speeds (walk / run / sneak of the hero;
the soldiers' walk, run, alert walk and alert run derived from the same clock), the animation clock
(a table tick = 3 clocks of 1/64 s, a frame lasts its tick half plus one such ticks), the noise
channel (a running character is heard from at least 330 px by soldiers not facing him, and they
charge at once) and the silence of a walk at 290 px are **measured**; the view cone (angle, range,
the crouch divisor), the noticed -> alarm sequence a sighting starts, the alert timeout, the return,
the knock-out timer, the punch's arc and the profile stats stay **hypotheses**. The script VM
records `Assumption::Perception` only for the hypothesised part (an alert an actor reached by sight),
never for a run heard (ADR-0008, "Hypotheses and taint").

- **Speeds** (item 5). Every moving entity covers per world tick the speed of the cycle it plays
  (`Entity::effective_speed`, `AnimSet::cycle_speed`): the cycle's summed `advance` over its duration
  on the animation clock (`crates/opensherwood-core/src/anim.rs`), i.e. `advance per frame x 64 / 3`
  px/s for the one-tick frames of the walk / run cycles: hero walk 85.3 px/s (1.42 px per tick,
  measured 85.3), run 106.7 (measured 101 +- 10; the double click plays action 7, `inferred`), sneak
  18.0 (27 px per 32 table ticks; measured 17.8); soldier walk 42.7, run 64, alert walk 64, alert run
  85.3 (derived). The entity moves at the constant cycle average, not in per-frame steps (`observed`
  residual 2.5 px rms in 8.1 fits both). An entity whose profile has no forward-moving cycle for its
  state (synthetic units, profiles without a table) walks at `Entity::speed` (364 / 256 px per tick
  for a synthetic player, 182 / 256 for a synthetic guard: the hero's and the soldier's measured
  values) times the hero table's ratios `FALLBACK_RUN_SPEED_RATIO` = 5 / 4 and
  `FALLBACK_SNEAK_SPEED_RATIO` = 27 / 128. Pinned by `cycle_speeds_match_the_measured_hero_values`
  (core) and `test_walk_run_and_sneak_speeds_match_the_measurements` (H01, within 5 % over 120 ticks).
- **Perception** (item 2). Every enemy soldier (`BORG` actor) that is alive, active, not AI-locked (natives
  134 / 135: a locked AI perceives nothing, section 2.5) and on his feet tests every player character each
  tick, in entity order from a round-robin cursor (`World::ai_cursor`, snapshotted) and within the layer's
  per-tick work budget (`AI_WORK_PER_TICK` = 2^24, shared with the path searches the alert states issue: one
  unit per entity pre-indexed, one per entity inspected, one per soldier / player character pair tested; a
  scan the budget cuts short resumes next tick where it stopped): a **view cone** of half angle
  `VIEW_CONE_HALF_ANGLE_256` = 32 (45 degrees) and range `VIEW_RANGE` = 250 map px, the range over
  `CROUCH_VIEW_DIVISOR` = 2 for a crouched character; and a **noise radius** `RUN_NOISE_RADIUS` = 350 px
  around the soldier within which a running character is heard whatever he faces. Occluders and walls do not
  block sight; walking and sneaking make no noise; civilians perceive nothing. The noise radius is
  `measured` as a lower bound (8.6: heard at >= 330 px; 350 is the engine's choice above it), the
  silence of a walk at 290 px is `observed`; the cone's angle, its range (below 290 px for a soldier not
  facing the character, 8.6; 250 keeps it beyond the rails' check-for radii) and the crouch divisor are
  `hypothesis` (item 7.2 measures them); the geometry (a sector test on a 4096-scaled sine table,
  `ai::in_view_cone`) is pinned by `view_cone_geometry`.
- **Alert states** (item 1 / 3). Two entries. **Heard** (`measured`, 8.6): a soldier who hears a running
  character charges at once, `patrol` -> `alerted` on the same tick with the alert run 151 to the position
  of the noise, no reaction animation (the entity's `heard` flag marks the alert as the measured channel;
  whether the whole courtyard joined because each soldier heard the run or because the alarm propagates
  is not separated by the measurement: the engine has no propagation, every soldier within the radius
  hears for himself). **Seen** (`hypothesis`, the cone is not measured): `patrol` (the rail program, actions
  0 / 6 / 7) -> `noticed` (141, plays for the animation's length: 11 table ticks = 31 world ticks on the
  soldier profiles; the soldier stops and remembers where he perceived the character and where he stood)
  -> `alarm` (142, 19 table ticks = 54 world ticks) -> `alerted`. Both then: `alerted` runs to the last
  seen position with 151, stands with 140, walks with 143; every new sighting or noise refreshes the
  position and the `ALERT_TIMEOUT_TICKS` = 300 timer (a hypothesis) -> `returning` (walks back to the
  origin with 143) -> `patrol` (the program continues where it stood). The animation ids come from the
  soldier / knight profiles (section 2.4); profiles without them fall back to idle / walk / run
  (`anim::AnimSet`). Every change of an actor's action id reaches its script class as
  `ActionChange(previous, new)` (the parameter order is a hypothesis: the H01 archer classes compare the
  second parameter with 141; pinned by `action_changes_reach_the_actors_class`); a run near the
  archery-training archers no longer ends the training through 141 (they charge instead), which is what
  8.6 shows for the courtyard (`test_running_near_a_soldier_alerts_him_at_once_from_afar`).

- **Perception** (item 2). Every enemy soldier (`BORG` actor) that is alive, active, not AI-locked (natives
  134 / 135: a locked AI perceives nothing, section 2.5) and on his feet tests every player character each
  tick, in entity order from a round-robin cursor (`World::ai_cursor`, snapshotted) and within the layer's
  per-tick work budget (`AI_WORK_PER_TICK` = 2^24, shared with the path searches the alert states issue: one
  unit per entity pre-indexed, one per entity inspected, one per soldier / player character pair tested; a
  scan the budget cuts short resumes next tick where it stopped): a **view cone** of half angle
  `VIEW_CONE_HALF_ANGLE_256` = 32 (45 degrees) and range `VIEW_RANGE` = 200 map px, the range over
  `CROUCH_VIEW_DIVISOR` = 2 for a crouched character; and a **noise radius** `RUN_NOISE_RADIUS` = 150 px
  around the soldier within which a running character is heard whatever he faces. Occluders and walls do not
  block sight; walking and sneaking make no noise; civilians perceive nothing. All four numbers are
  `hypothesis` (section 2.3 found no such field; item 7.2 / 7.3 measure them); the geometry (a sector test on a
  4096-scaled sine table, `ai::in_view_cone`) is pinned by `view_cone_geometry`.
- **Alert states** (item 1 / 3). `patrol` (the rail program, actions 0 / 6 / 7) -> `noticed` (141, plays
  for the animation's length: 6 ticks on the soldier profiles; the soldier stops and remembers where he
  perceived the character and where he stood) -> `alarm` (142, 11 ticks) -> `alerted` (runs to the last seen
  position with 151, stands with 140, walks with 143; every new sighting refreshes the position and the
  `ALERT_TIMEOUT_TICKS` = 300 timer, a hypothesis) -> `returning` (walks back to the origin with 143) ->
  `patrol` (the program continues where it stood). The animation ids come from the soldier / knight profiles
  (section 2.4); profiles without them fall back to idle / walk / run (`anim::AnimSet`). The states are the
  engine's reading of section 2.4 (`inferred`); the transition order noticed -> alarm and the return are
  `hypothesis`. Every change of an actor's action id reaches its script class as `ActionChange(previous,
  new)` (the parameter order is a hypothesis: the H01 archer classes compare the second parameter with 141;
  pinned by `action_changes_reach_the_actors_class`), so the archery training of the first mission ends when
  an archer notices something (`test_running_past_a_soldier_is_noticed_then_the_alarm`).
- **Knock-out** (item 4, the blow only). A left click on an enemy with a player character selected is an
  attack order (hypothesis for the manual's fist icon, section 1): the character walks into
  `PUNCH_REACH` = 32 px (the 30-35 px displacement of action 123, `observed`), then, if his profile has 123
  (Robin and the big man, `observed`) and he stands within `BACK_ARC_HALF_ANGLE_256` = 48 (67.5 degrees) of
  straight behind the victim (`hypothesis`), plays 123 (`punching`, 12 ticks) and the victim goes
  `knocked_down` (41 forward, or 44 backward if struck from the front - unreachable today, since the blow is
  only delivered from behind, 13 / 10 ticks) -> `lying` (47 / 48) for `KNOCK_OUT_BASE_TICKS` = 600 ticks
  scaled by `(100 - p4) / 100` (`p4` = the profile's knock-out resistance, section 3.3, `hypothesis`; `p4` >=
  100 makes the blow fail and the victim notices the attacker) -> `getting_up` (49, 16 ticks) -> `returning` /
  `patrol`. From the front the character stops and faces the victim. The knock-out chance of the manual and
  the "stars" are not modelled; no comrade revives him; the victim keeps his position (the fall's
  displacement is ignored). Pinned by `knock_out_from_behind_and_a_stop_from_the_front`,
  `immune_victims_notice_the_blow_and_resistance_shortens_the_sleep`, `a_profile_without_the_punch_cannot_strike`
  and, on the first mission, `test_knock_out_from_behind_puts_the_soldier_out_of_action` (the corridor post
  the level script polls with native 90).
- **Hit points**: `p0` of the SD record per entity (`hypothesis`, section 2.3); 100 for player characters and
  civilians (no field read yet). No damage model: nothing loses them yet.
- **Script predicates** (item 3; all five read one state function, `ai::ActorStatus`, and `validate` holds
  the layer's invariants: `Dead` exactly when `alive` is false, a timer exactly in the timed states, alert
  states on enemy soldiers only, attack orders from a player character to an enemy soldier): 85 = dead or
  deactivated, 87 = dead, 90 = knocked down / lying / dead
  (getting up counts as back in action: hypothesis), 128 = alive, active and on his feet, 240 = active;
  88 / 89 stay stubs returning 0 (no tied / netted state exists); 140 (actor, 0 / 1 / 2) sets the gait of the
  actor's program walks (0 walk, else run; section 2.5, `hypothesis`); `FilterAIEvent` is never called.
- **Timing**: every timed state lasts one loop of its animation on the measured animation clock
  (`anim::world_ticks`: a frame lasts its tick half plus one table ticks of 3 clocks at 64 Hz, 2.8125
  world ticks per table tick at 60 Hz; the animation player accumulates the clock in
  `AnimState::elapsed`, 16 units per world tick against 45 per table tick, so the frame rate is exact in
  the long run and a state ends on the tick its loop completes); without the block (or a catalog) the
  soldier profiles' counts apply (`NOTICED_TICKS` 31 world ticks from 11 table ticks, `ALARM_TICKS` 54
  from 19, `KNOCKED_DOWN_TICKS` 60 from 21, `GET_UP_TICKS` 68 from 24, `PUNCH_TICKS` 54 from the hero's
  19). The `+ 1` rule is `inferred` from the sneak cycle alone (8.4 note); no reaction animation was
  timed.

- **Taint** (ADR-0008, "Hypotheses and taint"): the VM records an assumption whenever a *hypothesised*
  part of the layer changes script-visible state: `perception` when an alert action id of an actor alerted
  by sight reaches an `ActionChange` handler (an actor alerted by a run heard, `heard`, records nothing:
  that channel is measured), `knock_out` when native 90 reports a knocked-out actor, 128 refuses one or a
  knock-out action id reaches a handler, `profile_stats` when a blow consults `p4`; `observe.script.tainted`
  then marks the mission's outcome as not authoritative until the oracle captures of section 7 settle the
  values.

Not implemented from section 6: the rails' check-for scans, silhouettes, the fighting posture, strikes and
parries, the bow, damage and death, tying / carrying / reviving, the stimuli of section 5.

## 7. Open questions and the oracle capture plan

Only a trace of the original settles these. Capture with `harness/tools/original/rhcap.py` (windowed
cnc-ddraw build, 1024x768, `boot`-style periodic screenshots at a fixed `dt`; the in-mission cursor
problem of ui-flow.md open question 8 must be solved first or the scene driven by a human while
`rhcap.py boot <prefix> <secs> <dt>` records). `HIDEINTERFACE` gives clean frames; `timeless` (manual
p. 37) stops the clock for measuring static geometry; the first mission is the scene for everything
below unless stated.

1. **Speed and tick model** (decides section 4.2 A vs B). Scene: Robin on the open ground below the
   gate; order a walk of ~400 px along the screen x axis, then a double-click run, then the same
   crouched (`c`). Record at `dt = 0.1 s` for 15 s each. Measure: feet position per frame (the
   selection circle centre) -> px/s for walk, run, crouched walk; the animation frame rate (count
   stride cycles: 22 frames per walk cycle). Then a blue soldier walking a rail (the courtyard lancers):
   px/s of the patrol walk, and of the alert run after being noticed. Expected under A at 25 ticks/s:
   hero 100 / 125 / 37.5 px/s, soldier 50 / 75 (alert 100). Also whether the double-click run is 7 or 10
   (stride length 60 vs 112 px per cycle).
2. **Sight cone**: select Robin, hold Alt and hover a halberdier on the wall and a lancer in the yard:
   screenshot the cone (`HIDEINTERFACE` first). Measure: opening angle, length in map px, whether it is
   a sector or a fan with a near / far part, and whether it turns with the glance commands (record a
   guard executing a `0b`/`0c` pair: face, wait, glance left, glance right - timing per glance). Repeat
   for an archer (long sight?) and a civilian; compare with `PCSIGHT` for Robin's own cone. This decides
   whether the cone depends on the family / tier (section 2.3 says the profile has no such field).
3. **Detection thresholds**: walk Robin into a cone from the side at walk / run / crouch; record
   the emoticon sequence (question mark -> exclamation), the distance at which each appears, and the
   delay (frames) between entering the cone and 141 / 142; then out of sight: how long until the guard
   returns to patrol (alert timer). With `NOISE` on, measure the noise radius for walk / run / crouch and
   whether a guard facing away reacts to a run inside it.
4. **Knock-out**: from behind, knock out a lone lancer (fist icon, tutorial string 14). Record: the
   sequence of action ids (123 on Robin; 41 or 44 on the victim - forward or backward relative to the
   blow), the stars' duration (count frames until 49 starts) for a blue lancer and a blue halberdier at
   full health and after one sword hit, whether a knocked-out guard revives by himself, whether a
   comrade revives him and with which animation (158-160?), what `BIG BROTHER` prints for the victim
   (state names, the KO timer). Then the same from the front: does the knock-out succeed at all
   (the manual implies it does, with a lower chance)?
5. **Melee**: fight one blue swordsman: for each of the ten figures draw it and record the action id
   played (59..75), its duration, the damage number shown over the enemy's head (matches table A `[2]`?),
   the energy drop (table A `[15]`?), the parry animation (68 / 103) on right-click and how long it holds;
   record `BIG BROTHER` on both fighters (hit points: 80 for a blue swordsman if `pre[0]` is right).
   Note which animation plays when the enemy dies (44 / 107?) versus when he is knocked out.
6. **Bow**: with arrows gathered, shoot one target of the archery range: ids 85 / 87 / 88 / 89 / 93 / 86 in
   order with their durations; the aim time before the cursor turns green; the arrow's flight speed.
7. **Script states**: with `AI` on a courtyard lancer, note the state names displayed while patrolling,
   after noticing, in combat, knocked out; correlate with natives 126 / 128 / 90 (the tutorial mission's
   `Hourglass` polls 90 on the lancers: the exit soldier gets his path the tick 90 becomes 1).
8. **Rail semantics**: film one wall guard through a whole rail cycle and time each command: 0x04
   operand versus seconds (is 100 = 1 s?), the glance angle and duration, 0x0d (does the guard scan /
   turn / stand longer?).

Everything measured goes to `docs/oracle/` as normalised numbers (no frames committed); the fields
above then move from `hypothesis` to `observed`.

## 8. Oracle measurements (2026-09-05)

Black-box observation of the running original (analyst session, ADR-0003: no disassembly, debugger or
memory inspection; screen recordings of the private copy, nothing committed). Scene: the first mission
(`H01_Lin_VL`) directly after the briefing, camera as the game places it (Robin's start is at client
(500..513, 392..399) with the camera centred on him), profile resolution 1024x768, zoom normal. Every
position is a client pixel of the game window (= a map pixel at this zoom, see 8.5); every time is the
recorder's wall clock (`time.time()`, resolution about 1 ms; sampling at 30 Hz or about 70 Hz as noted).

### 8.1 Walk speed (`observed`, confidence high)

Method: Robin selected through his portrait, one left click on open mud at (180,580); full-frame
recording at 30 Hz for 8 s (`frame_rec.py`, median-background blob tracking of the feet = bottom
centre of the sprite's blob). Raw: click at t = 0.50 s; the feet stay at (511,392) until t = 1.45 s
(turn in place and walk start), then move in a straight line to (174,589), reached at t = 6.2 s, then
idle. Least-squares fit of the feet over t = 1.8..6.0 s (125 samples): vx = -73.2, vy = +43.8,
**|v| = 85.3 px/s**, residual of x around the line 2.5 px rms. Autocorrelation of the blob's width
over the same window: stride period **1.044 s** (peak 0.56); 22 frames x 4 px = 88 px per cycle at
85.3 px/s gives 1.03 s, consistent. Uncertainty: +-1.5 px/s (fit), +-0.03 s (period).

### 8.2 Crouched walk (`observed`, confidence high)

Method: key `c` (crouch), then one left click at (180,580) from the start position; 30 Hz for 13 s.
Raw: click at 0.50 s, feet from (496,405) at 1.10 s to (316,508) at 12.96 s (still moving when the
recording ended). Fit over 1.2..12.9 s (348 samples): vx = -15.5, vy = +8.7, **|v| = 17.8 px/s**
(residual 3.6 px rms); shape periodicity (height and width autocorrelation, peaks 0.66-0.70)
**1.48-1.52 s per cycle**. 27 px per cycle (the sneak table's summed advances, 4.2) at 17.8 px/s is
1.52 s per cycle, consistent. Ratio crouched : upright walk = **0.21** (the 4.2 hypothesis of
1.5 / 4 = 0.375 is wrong: the sneak cycle is not played one frame per walking tick, see 8.4).

### 8.3 Run (double-click) (`observed`, confidence medium: one second of clean motion)

Method: from (174,589) a double left click (two clicks 120 ms apart) on (513,392), a cropped recording
(x 140..560, y 320..610) at about 70 Hz. Raw: clicks at 0.50 s; the feet leave (181,595) at 1.08 s;
feet (222,592) at 1.15 s, (249,586) at 1.40 s, (262,576) at 1.63 s, (278,566) at 1.78 s; from 1.9 s
on other sprites (soldiers, see 8.6) enter the crop and the track is lost. Fit over 1.10..1.85 s
(57 samples): **|v| = 101 px/s** (0.75..1.85 s including the start: 114 px/s). Predicted by 8.4 for
the run cycle 7 (5 px per frame): 106.7 px/s; the 16-frame sprint 10 (7 px per frame) would give
149 px/s. So the double-click gait is **run 7** (`inferred`, one short sample; the mix of accelerating
start frames and the loss of the track after 0.8 s bound the value to about 95..115 px/s). Ratio
run : walk about 1.2 (1.25 predicted).

### 8.4 Animation clock (`inferred` from 8.1-8.3 and the idle guards, confidence medium-high)

Idle courtyard lancers (two soldiers at feet (812,302) and (796,389), never moving in this scene): a
cropped recording at about 70 Hz (mean sample interval 14.2 ms) for 6 s shows the picture of each
soldier changing at a **uniform 93.75 ms** (16 changes per 1.500 s; the sequence of changed-pixel
counts repeats exactly every 16 changes, so the cycle is 1.50 +- 0.01 s and no change is skipped).
Robin's walk: 22 frames per 1.044 s = **47.5 ms per frame**, twice as fast, and 4 px per frame =
85.3 px/s measured (4 x 64/3 = 85.33). The reading that fits all three numbers: an animation clock of
about **64 Hz** where a walking / running frame lasts 3 clocks (46.9 ms) and an idle frame step 6
clocks (93.75 ms); a 21.3 Hz clock with one walking frame per tick fits the walk equally and the idle
step as two ticks per frame. What is settled: the sprite tables' advance values are pixels per
displayed frame, and a displayed walking frame lasts 46.9 +- 1 ms - not 40 ms (25 Hz) and not
16.7 ms (60 Hz). Speeds under this clock: hero walk 85.3, run 106.7, sprint 149.3 px/s; soldier walk
42.7, run 64, alert walk 64, alert run 85.3 px/s (4.2 advances x 64/3; only the hero walk, crouch and
run were measured). The crouched cycle (14 frames, 1.50 s) runs at the idle step rate (6-7 clocks per
frame), not the walking one.

Note on the idle: the spec's "6 frames ping-pong" for action 0 would give 10 changes per cycle, the
lancers show 16 per 1.5 s; either these guards play a different standing action (rail wait state) or
the table reading is off. Open. Checked against the tables by the implementer (2026-09-05): the idle
block 0 lists its six frames *with* the ping-pong (frame indices 0 1 2 3 2 1), so one loop is 6
changes of uneven length (tick halves 6 3 3 15 4 4 on `Soldier A00`, 6 2 2 15 4 4 on the hero and
on the alert idle 140), 35 table ticks = 1.64 s on the walking clock or 41 = 1.92 s with the
`+ 1` rule below; no static block of `Guard A00` (127 blocks) or of the other soldier profiles has
16 uniform changes per 1.5 s under either reading (the nearest, the crossbowman's bow set 87, is 16
uneven frames of 34 ticks). Neither reading reproduces the observation; the courtyard lancers'
standing action is not identified (the console's `AI` display, item 7.7, would name it). What
*is* settled by the sneak measurement: the crouched cycle's 14 frames carry tick halves summing
to 18 and last 1.50 s together, which is `18 + 14 = 32` table ticks of 46.875 ms exactly, so a
frame lasts its **tick half plus one** table ticks (the walking frames' 0 = one tick, as
already read); the plain "tick half, at least 1" reading would give 0.84 s and 32 px/s for the
sneak, refuted by 8.2. The engine uses the `+ 1` rule for every frame (`inferred`, one
measurement; it lengthens the timed reaction animations, e.g. 141 from 6 to 11 table ticks, which
no measurement checks yet).

### 8.5 Screen versus map pixels (`observed`)

Both walks were along the screen diagonal dx : dy = -1.67 : 1 (down-left) and the speed matched the
per-frame advance exactly in screen pixels, so at the normal zoom **1 map px = 1 screen px** with no
direction-dependent foreshortening of the advance. A purely horizontal or vertical walk was not
recorded (the courtyard offers no 300 px straight line free of obstacles from the start position).

### 8.6 Alert reaction to running (`observed`, qualitative)

Walking the diagonal (513,392) -> (174,589) at 85 px/s, 290..450 px from the two courtyard lancers,
provoked nothing in 8 s. The double-click run back along the same line (8.3) did: within 1.5 s of
the first running frames soldiers were converging on Robin (sprites entering the recording crop from
t = 1.9 s); at t = 8 s five soldiers (the two courtyard lancers, one from the gate, two more from the
right) stood around him in melee (red circles, health bars, a damage number "10" over Robin, the
portrait showing the crossed-swords combat state); Robin was dead about 40 s later ("mission lost"
parchment with restart / load / OK seals at (333,556), (388,556), (512,556); the restart seal goes
straight to the briefing). Robin was at least 330 px from the nearest lancer when the run started.
Conclusion: a running player character is detected at **>= 330 px** by standing soldiers (sight or
noise is not separated here; the lancers were not facing him, so noise is the likelier channel), which
rules out `RUN_NOISE_RADIUS` = 150 together with `VIEW_RANGE` = 200 as the only detection channels.
The reaction is not the "noticed / alarm" pause of section 2.4 followed by a walk: the soldiers ran to
him at once and the whole courtyard joined (a mission-wide alarm, `inferred`).

### 8.7 Not measured (and why)

- View-cone geometry (Alt-hover, `PCSIGHT`), noise radius (`NOISE`), knock-out duration and punch
  reach, alert timeout, guard patrol speed, bow and melee tables: the 60-minute budget went into
  making the in-mission pointer controllable (see Provenance) and into the speeds; no soldier
  patrolled in the start view during the session (all eight visible soldiers stood in rail wait
  states), and the only alert run ended in Robin's death. The console (F11 with the kneel icon hovered)
  was not opened.
- Briefing: three parchment pages, each advanced by Enter; no timer observed.
- Repeatability: each speed is one run; the walk start was seen twice (0.9-1.0 s from click to the
  first moving frame in both directions).

### 8.8 Numbers for the implementer

| Quantity | Use | Status |
|---|---|---|
| hero walk | 85.3 px/s (4 px per 46.9 ms frame) | observed |
| hero crouched walk | 17.8 px/s (27 px per 1.50 s cycle) | observed |
| hero run (double-click) | 107 px/s predicted, 101 +- 10 measured; gait = action 7 | observed / inferred |
| hero sprint (action 10) | 149 px/s | inferred (clock) |
| soldier walk / run / alert walk / alert run | 42.7 / 64 / 64 / 85.3 px/s | inferred (clock) |
| displayed frame of a walking cycle | 46.9 ms (about 2.8 engine ticks at 60 Hz; the table's "1 tick" = 3 clocks of 1/64 s; a frame lasts its tick half plus one such ticks, from the sneak cycle) | inferred |
| idle frame step | 93.75 ms | observed |
| click -> first moving frame | 0.9-1.0 s (turn + walk-start animation) | observed (2 samples) |
| running detected by standing soldiers | at >= 330 px; walking at 290 px not detected | observed |
| `ALERT_TIMEOUT_TICKS`, `KNOCK_OUT_BASE_TICKS`, `PUNCH_REACH`, `VIEW_CONE_HALF_ANGLE_256` | unchanged hypotheses | not measured |

### Provenance (section 8)

- Game: GOG build, `Robin Hood.exe` SHA-256 `1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`,
  English, run from the private copy `C:\Users\przem\source\gamedata\robinhood_oracle` (cnc-ddraw
  windowed 1024x768 at the screen origin, `maxfps=60`, `devmode=true`, `renderer=direct3d9on12`),
  profile "Analyst", medium difficulty, 2026-09-05.
- Tools (all generic, `harness/tools/original/`): `rhcap.py` (launch, PrintWindow screenshots,
  scan-code keys), `oracle_input.py` (`mmove` / `mclick`: closed-loop in-mission pointer control with
  screenshot feedback; now matches several pointer shapes - the arrow and the "cannot go" cross - from
  templates cropped out of earlier screenshots, because the pointer changes shape over characters and
  blocked ground and the earlier single template lost it), `frame_rec.py` (new: mss screen-DC recording
  of grey frames with timestamps into one .npz, median-background blob tracking, per-region change
  timelines, speed fits). Recordings and screenshots under `harness/captures/original/` and the session
  scratch directory, not committed.
- Input findings: the game reads relative mouse deltas (DirectInput style) and in a mission recentres
  the OS cursor, so only screenshot-feedback control works; the pointer at a window edge scrolls the
  camera (one stray move scrolled to the village outside the walls; a double click on the hero's
  portrait at (125,690) brings the camera back and selects him). Windows reported Left Ctrl and Left
  Shift as held during the whole session (`GetAsyncKeyState`), so no chords were used; single keys
  (`c`, `s`, Enter, Escape) and clicks worked. The game stops rendering when it loses the foreground;
  `rhcap.focus` (Alt tap + `SetForegroundWindow`) recovers it. A screen-DC recording taken while another
  window covers the game records that window: check the foreground before recording.
- Analysis: `frame_rec.py track` (largest blob in a region against the per-recording median; feet =
  median x of the lowest 5 rows), `numpy.polyfit` for the speeds, autocorrelation of the blob's width /
  height / pixel count for the stride period, `frame_rec.py changes` for the idle frame steps.
- Who: analyst session (a Claude agent run separately from the implementer; no engine code written in
  it).

## Provenance

Data files of the GOG English build (executable SHA-256
`1d64cf088f1202e67045759fe23aaa879434ea662a922e93cff537a839da12b5`, read-only copy
`C:\Users\przem\source\gamedata\robinhood`), 2026-09-03, analyst session, observation only:

- `profile.cpf`: `harness/tools/probe/cpf_stats.py --tiers --tables` (SD / PC numeric fields as columns,
  per-family tier deltas, tables A and B); the layout correction of the SD `unknown_post` block
  (a `u16`, a flags byte, four `u16`, the class `u16`, `u16 0`, `u16 ranged kind`, `u16 0`, `u8 0`, the
  floats) is in profile.md.
- `.rhs`: `harness/tools/probe/anim_actions.py --families / --table / --matrix / --sheet` on
  `RobinHood`, `Soldier A00`, `Guard A00`, `Archer00`, `ManCivilianPoor`, `Child`, `Longchamp Dead`,
  `LittleJohn` (sheets viewed in a scratch directory, not committed; frame counts, tick and advance halves
  from the tables of all 117 files).
- `.rhm`: rail opcode counts per assigned actor family (scratch script over `rhm_full.parse_file`,
  all 39 files).
- `.scb`: `scb_semantics.py --natives --id 87 88 89 90 102 59 130 140 134 135 101 126 128 228 219 220
  177 196 197 198` and `--pseudo` listings of all 39 files, grepped for the natives and callbacks named
  above; helper and variable names are paraphrased ("is neutralised", "out of service" flags).
- Manual: `Manual.pdf` printed pages 11, 15-18, 20-28, 31-34, 37, read from page renders; paraphrased.
- Tutorial texts: `Level.res` TEXT 1000105 strings 12..22 via `harness/tools/original/sres_text.py`;
  paraphrased.

Nothing here was checked against the running original; the confidence column is the analyst's.
