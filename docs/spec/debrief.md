# Mission debrief

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-09-23. Retail facts come from the
[debrief research](../formats/debrief.md) of FA.EXE 1.02F and from five retail
screenshots John supplied in this session (a failed Quick Mission, flown alone
for one second, nothing fired). Validation: [debrief baseline](../baselines/debrief.md).

## What the player sees and does

Ending a Quick Mission (Escape menu **End mission**, or Ctrl-Q) opens the
**MISSION DEBRIEF** screen: a clipboard on a map background, a `?` menu on the
top bar, and a control panel with a page counter, a **PREV/NEXT** rocker, a
greyed-out **Cancel** and a blue default **OK**. The clipboard shows five pages:

1. The mission's own debrief text. For a Quick Mission this is retail `QUICK.MT`:
   **MISSION SUCCESS**, "You have successfully completed this Quick Mission.", or
   **MISSION FAILURE**, "You failed this Quick Mission."
2. **MISSION OUTCOME : SUCCESS/FAILURE**, the objective sentences, the elapsed
   time and the **PILOT STATUS** table (Status, Damage, Landing grade).
3. **KILLS**: Fighter, Bomber, Helicopter, Ship, SAM, AAA, Tank, Vehicle,
   Structure, Other, then Friendly fire.
4. **HIT PERCENTAGES**: Air-to-Air and Air-to-Ground (Launches, Hit, Failed,
   Spoofed, Jammed), Gun (Hit) and Bomb (Hit).
5. **ENEMY HIT PERCENTAGES**, fire aimed **ON** each pilot: AAM and SAM
   (Launches, Hit, Failed, Jammed, Spoofed), Gun (Hit) and AAA (Hit). Aircraft
   fire fills AAM and Gun; the fire of a mission's SAM sites, guns and ships
   fills SAM and AAA ([below](#weapon-classes)).

Every table has a PLAYER and a WINGMAN column. The rocker, Page Up/Down and the
arrow keys turn pages; Home and End jump to the first and last page. Paging
stops at either end. Left-clicking the clipboard turns to the next page and
right-clicking it turns back, each with a matching tap of the rocker. Requested
by John on 2026-09-23 (`opinionated`). OK, Enter, Space or Escape close the debrief. Cancel is
never available. `?` offers Exit to Desktop, like the Quick Mission creator.

A networked flight's debrief (multiplayer) changes only the first page and the
outcome line, and only when nobody's objectives decided the mission: the page
reads **MISSION ENDED** and says who ended it, and page 2 reads **MISSION
OUTCOME : INCOMPLETE**, retail's word for a multiplayer mission cut short
(`agent decision`, EF-F; [details](../ARCHITECTURE.md#smoke-test-fixes-ef-f)).
Single player never shows it.

When a SAM site, gun or ship shot the pilot's aircraft down, page 2's pilot
status table gains a **Shot down by** row with the unit's short name (SA-6,
ZSU-23-4); the flight shows the same name in a message ("Shot down by SA-6")
when its aircraft is destroyed. Neither appears for an aircraft's kill or when
the pilot is flying (`opinionated`, agent decision, 2026-10-10; the networked
debrief does not carry the name yet).

Closing the debrief returns to the **Quick Mission creator with every setting
of the mission just flown**, not to the ordnance screen. Requested by John on
2026-09-23 (`opinionated`). Retail appears to return to the main menu after a
Quick Mission (medium confidence, see the research).

## Numbers

### Presentation

| Item | Value | Provenance |
| --- | --- | --- |
| Background | One of DEBSCR, DEBSC3, DEBSCU, DEBSCV, equal chance, chosen each time the debrief opens. DEBSCR's 60 blank rows below 480 are not shown | retail |
| Panel controls | BRIEFSCR.DLG origin (48,363): Cancel at (145,377) width 70, OK at (145,410) width 60, rocker at (96,402) | retail |
| Page counter | `N of  M` (two spaces before the total), PANLFNT2, centred in (78,381,50,17) over the background's own black box | retail string and font; placement fitted |
| PREV / NEXT | PANELFNT at (68,404) and (68,426) | fitted to screenshots |
| Rocker hit halves | (96,402,18,16) previous, (96,418,18,16) next | retail |
| Rocker frames | Level is ROCKER02. Pressing the top half steps 01 then 00 and holds; the bottom half steps 03 then 04. Releasing steps back to 02. The page turns on press | retail |
| Rocker frame time | 40 ms per frame | fitted; retail waits one screen update |
| Other menus | The Quick Mission selector and ordnance page and fuel rockers share this rocker. Requested by John on 2026-09-23 | retail frames |
| Keys and clipboard clicks | Tap the rocker: it steps to the held frame and straight back | agent decision |
| Clipboard click area | The brown board: (248,66,333,404) on DEBSCV/DEBSCU, (278,66,303,404) on DEBSCR/DEBSC3; the panel controls stay on top | fitted |
| Page fonts | BODYFONT body, BOLDFONT for `.bold`, HEADFONT for `.header`; imported colours on the background palette | retail fonts, face match to screenshots |
| Text column | Left x=294, centred text on x=418, top line at y=162 | fitted to screenshots |
| First-page result text | Both the heading and success/failure sentence are centred on x=418, including body lines marked `.left` in QUICK.MT. Other pages retain their column alignment | opinionated correction requested by John on 2026-09-23 |
| Line height | 12 px; header lines 15 px | fitted to screenshots |
| Tab stops | x=403 and x=478; a tab past the last stop does not move | retail |
| Underline | 1 px, two rows below the capital baseline, under the underlined text only | fitted |
| OK and Cancel labels | Quick Mission's approved flat font, Cancel in grey | `opinionated`, matches the creator buttons |
| Sounds | Rocker press `&ROCKDN.11K`, release `&ROCKUP.11K`, either half; a tap plays the press sound only. OK and `?` use the menu button cue | retail |

### Cells

| Cell | Rule | Provenance |
| --- | --- | --- |
| Counts (kills, launches) | `-` for zero, otherwise the number | retail |
| Percentages | `-` when nothing was launched; otherwise part × 100 ÷ whole, rounded down, part capped at the whole | retail |
| Hit | Percentage, then ` (N)` with the damage points those hits did when N > 0, for example `50% (340)` | retail |
| Failed | Launches minus hits, spoofs and jams, as a percentage of launches. A missile still flying at mission end counts as failed | retail |
| Spoofed, Jammed | Their own counts as a percentage of launches. A missile resolves once | retail |
| Status | Alive, Ejected or Dead. A dead pilot is Dead even after ejecting | retail |
| Damage | Airframe damage as a whole percentage, rounded down. Dead or ejected shows 100% | retail |
| Landing grade | Average landing score as `NN%`, `-` with no landings | retail format; scoring fitted, below |
| Elapsed time | Simulated seconds as `M:SS`; minutes do not wrap | retail |
| WINGMAN column | One aircraft: the lowest-numbered AI member of Friendly Wing 1. All `-` when the player flies alone | retail |
| PLAYER and WINGMAN columns in multiplayer | Each seat has its own debrief. Its PLAYER column is the plane that seat flies and its WINGMAN column is the first other member of that plane's wing, human or AI. Alone in its wing, the column is all `-`. With one seat this is the row above | agent decision, 2026-09-29 |

### Weapon classes

A store counts by its type flags, as retail does: guided (0x1) with the air flag
(0x10000) is Air-to-Air; guided with the surface flag (0x20000) is
Air-to-Ground; otherwise the bomb flag (0x10) is Bomb and the gun flag (0x80)
is Gun. Unguided rockets fall in none of these and are not listed. Fire from an
enemy aircraft at a pilot counts as Gun when it carries the gun flag and as AAM
otherwise. Fire from an enemy surface unit (a SAM site, an anti-aircraft gun,
a tank, a ship, a battery launcher) counts the same way with the retail
names: a gun round is **AAA** and anything else is **SAM**, so a SCUD
launcher's SA-9 counts as a SAM and a tank's shell as AAA. A friendly or
neutral unit's fire is not counted. Every round or missile is one launch; a
battery's missile belongs to the launcher that fired it (`spec-derived`
from the retail rule, [format](../formats/debrief.md#page-contents)).

Every gun projectile is one round. Whether retail counts rounds or bursts is
unknown.

### Kills

Only the player's and the wingman's kills are counted. Destroying an aircraft
on your own side counts as Friendly fire. Every other victim counts in the first
matching row of its object class word: 0x8000 Fighter, 0x4000 Bomber,
0x2000 Ship, 0x1000 SAM, 0x800 AAA, 0x400 Tank, 0x200 Vehicle, 0x100 Structure,
0x40 Other. Retail puts helicopters (a PT flag) first; no supported aircraft is
a helicopter yet, so that row stays empty.

**One record.** A kill is only ever credited to a shooter whose hit, with
damage, the hit tally also holds. The debrief's hit counts and the credit for a
kill come from the same strike, so a pilot's kills never exceed its recorded
hits. A hit that did no damage is counted as a hit, but it does not make its
shooter the last attacker.

- **Direct kill.** The kill goes to the shooter whose hit destroyed the
  aircraft.
- **Indirect kill.** An aircraft that is lost another way after being damaged,
  for example its pilot ejects, it crashes or it hits the ground, goes to the
  last shooter whose hit damaged it (retail credits the attributed last
  attacker). A mid-air collision is the same: if the aircraft was damaged
  earlier, its last attacker is credited. This holds however long ago the hit
  was.
- **No credit.** An aircraft lost to the map edge, overspeed or belly scrape
  wear credits nobody, whoever hit it earlier. An aircraft nobody hit credits
  nobody.
- **A missile that was decoyed.** A missile resolves once. If a flare or chaff
  spoofs it, it counts as Spoofed, and if it then flies on and damages an
  aircraft after all, that strike is its hit (Spoofed is withdrawn, Hit and its
  damage are counted, and the missile is still one launch). Without this an
  aircraft could lose its hit points and its shooter be credited with the kill
  while the hit table showed none. A decoyed missile that only touches a wreck,
  explodes on the ground, runs out or is jammed stays Spoofed. A mission
  recording keeps both moments: the spoof when the decoy wins, then the hit,
  marked as replacing it, and naming the chaff or flare that fooled the missile
  by its release number ([recorded events](../REPLAYS.md#recording)).
- **Same rule everywhere.** Multiplayer scores (kills, damage) and the PvP
  kill limit credit a lost plane by this same record (`Ledger::credit`), so a
  plane's credited kill, its debrief and its score agree. A kill before the
  pilot ejects still counts twice in a multiplayer score, as the manual says.
  Agent decision, 2026-10-09: the manual does not describe indirect kills,
  decoyed missiles or hits that did no damage.

Airport and scene objects count as not friendly. Surface units are objects
with a side, and their kills fall in the same rows by their class word: a SAM
launcher in SAM, a gun in AAA, a tank in Tank, a ship in Ship, a supply truck
and a Straight Flush or HAWK radar element in Vehicle, a GCI radar in
Structure, a parked aircraft in Fighter or Bomber by its type's class word
(each counted in the first row of its class bits). A kill by splash or
collateral damage credits the shooter like any other
([splash](missiles.md#splash-damage)).

**Friendly ground units.** Destroying a surface unit of the player's own side
that is not an objective counts as Friendly fire, like an aircraft of its own
side (retail: a same-side object not flagged as a target). That includes a
base-layout SAM, radar or supply truck and a battery's added radar or truck,
which are never targets. A unit with no side (scenery) is not friendly
(`spec-derived`; John decided that it fails the mission, 2026-10-10).

### Outcome and objectives

| Rule | Provenance |
| --- | --- |
| Any friendly-fire kill by the player fails the mission. In a debrief for another seat, the kills of that seat's plane count, not the kills of the other seats | retail; multiplayer reading is an agent decision, 2026-09-29 |
| Any target still alive fails it; a target counts as destroyed when it is shot down, crashed or its pilot has ejected | retail |
| Any friendly objective lost fails it | retail |
| Otherwise the mission succeeds | retail |
| Targets are the enemy group the player's flight is assigned to destroy. When the player's flight has no target group (free fire, CAP, protection, self-defence, hold), every enemy aircraft is a target, as in retail Quick Missions | agent decision, 2026-09-23; retail makes every enemy aircraft a target |
| Friendly objectives are the aircraft the player's flight protects plus every member of a group whose survival is required, including the player when your own group's survival is required; a Redfor player's also include the mission's ground target | agent decision; retail Quick Missions have none |
| "Destroyed the target." / "Failed to destroy the target." for one target; "Destroyed the N targets." when all are down; otherwise "Destroyed N of M targets." Protected sentences follow the same pattern. A sentence appears only when its list is not empty | retail |

**A mission's ground target** adds its objects to the same lists
([surface objectives](surface-defenses.md#objectives-scoring-and-debrief)).
Every object of the template flagged 0x80, parked aircraft included, is a
target of a friendly (Blue) plane, destroyed when its hit points are gone,
and joins the air targets in **one combined Destroy line** ("Destroyed 2 of 5
targets."). In a multiplayer game a plane of the enemy (Redfor) side defends
the target: the same objects are its **Protect** objective, protected while
they stand. A surface object the result cannot find is undecided: neither
destroyed nor lost (an aircraft it cannot find counts as gone, as before).
Added radars and supply trucks are never targets. Surface ids have no
lineage. `opinionated` (John, 2026-10-10: one combined line, Redfor defends,
a friendly ground unit destroyed fails the mission).

Retail also requires a counter to reach 300 before a protect-only mission can
succeed; what it counts is unknown, so this rule is not applied.

### Objectives in a game with respawns

A multiplayer game can bring a lost aircraft back: a player's revival, or the
AI's respawn of its own aircraft. Each aircraft the mission started with is
the root of a *lineage*, and every aircraft a revival or a respawn adds
continues one (docs/ARCHITECTURE.md, "Death, revival and lives"). Retail has
no respawns, so every rule here is an agent decision of the lobby pass's
follow-up F1, 2026-10-09, **pending John**; he asked only that a respawned
objective aircraft "should still say objective".

| Rule | Provenance |
| --- | --- |
| An objective names a lineage. Every aircraft of an objective lineage is an objective wherever the original is: the target window's "Obj: Destroy" or "Obj: Survive", the AI's own target and escort lists (an intercepting AI wingman goes after the respawn too), and the network readout a client is sent | John asked for it, 2026-10-09; the rule is an agent decision |
| A destroy objective counts each lineage once. It is destroyed the first time any aircraft of it is lost, so the objective can be met although the lineage flies again; shooting the respawn down later is credited as a kill as usual and changes no objective count | agent decision, pending John |
| A protect objective fails on the first loss of a protected lineage, as it would without respawns; the respawned aircraft still shows "Obj: Survive" | agent decision, pending John |
| A player's revived aircraft is asked what its lineage was asked: the same targets and friendly objectives | agent decision |
| With no target group, the targets are every aircraft of the other side the mission started with, human-flown ones too, one lineage each. Before this, a networked flight counted only the AI's aircraft | agent decision; the table's retail rule, applied to every enemy aircraft |
| A networked mission gives every aircraft its objectives when it is built (the group objective of its wing, the groups that must survive), so a player who takes one in flight is asked what the mission asks of that aircraft. Before this a player who took an aircraft by handoff had no objectives: the target window never read "Obj:", and the debrief listed no friendly objective | agent decision (a fault found by follow-up F1) |
| A Redfor player's objectives are its own side's: friendly means its own side, whichever side it flies for | agent decision (a fault found by follow-up F1) |

Single player never adds an aircraft, so none of this changes it.

### Landing grade

Each touchdown on a landable surface after at least 5 seconds airborne counts
as one landing. Spawning on the runway and bounces do not count. A touchdown
with descent at or below half the aircraft's landing-limit descent rate and
bank at or below half its roll limit scores 100 (Good); any other touchdown
within the limits scores 50 (Fair). A touchdown outside the limits is a crash,
so the retail Unsafe score of 0 cannot occur. `fitted`: retail scores Good,
Fair and Unsafe as 100, 50 and 0, but its runway grading windows are
unrecovered. Only the default researched flight model records landings. AI
aircraft do not land, so the wingman's grade is `-`.

## Edge cases

- Restart during flight begins a new record; only the last attempt is shown.
- Free flight started with `--free-flight` has no mission and shows no debrief.
- An import made before the debrief lacks its art, fonts and text and is
  rejected with a re-import message. If a resource still fails to load when a
  mission ends, the creator shows the error instead of the debrief.

## Unknown

- Retail runway landing grade windows and carrier trap units. Next step: trace
  the writes of the landing grade byte (see research).
- The protect-only success counter. Next step: find the writers of its word.
- Whether a retail gun "launch" is one round.
- Retail menu after a Quick Mission debrief; this build returns to the creator
  by request regardless.
