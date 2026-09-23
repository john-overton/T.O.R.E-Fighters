# Quick Mission menu presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-09-16. Existing field values and dependencies
come from [the recovered tables](../formats/quick-mission.md). The two original
menu screenshots supplied by John in this session guide presentation only; their
executable build identity is unknown. They do not establish filtering or input
behaviour.

Aircraft choices contain only exact supported identities with a successfully
parsed imported flight profile. Metadata-only aircraft and variant aliases are
excluded from every wing selector. This is an opinionated restriction requested
by John on 2026-09-16. Names use the supported aircraft's full variant label.

Aircraft and theater fields open a modal list on activation. Shift cycles these
fields; other scalar fields retain click-to-cycle and Shift-to-open. Direct
opening for theaters is an agent decision. Selecting a row is provisional until
OK; Cancel and Escape preserve the draft. Empty lists cannot be accepted.

John requested reverse cycling with right-click on 2026-09-21. Right-clicking an
inline option selects its previous available value, wrapping to the last value.
This also works for aircraft/theater/airport fields without opening their list;
their existing left-click behavior is unchanged. The player's wing count wraps
from one to its maximum, never through zero. Empty lists do nothing. Existing
field dependencies, including clearing defenses when ground targets are none,
continue to apply. Require a matching right press/release on the same field.
Right-click cannot activate OK, Cancel, Exit, popup rows or controls behind a
selector/help menu. Focus loss cancels a pending press. In the ordnance view,
right-click retains its existing station-quantity decrement.

The selector shows 15 rows per page, black inset wells, blue diagonal markers,
a gold selected marker, a page count, a Prev/Next rocker, and beveled OK/Cancel
buttons. Arrow keys move selection, Home/End reach the ends, and Page Up/Down
move 15 entries. The rocker animates and pages on press like the
[debrief rocker](debrief.md#presentation); Page Up/Down tap it. Pointer paging preserves the pending selection until another
row is chosen. All 15 black wells remain visible on every page. Unused wells contain no marker
or label and have no hit region; the last page may contain fewer than 15 entries.

Presentation is fitted to the supplied references on a 640 by 480 canvas:
the existing 270 by 370 selector starts at (185,100), with 14-pixel wells spaced
18 pixels apart. Runtime original panel texture, rocker, button pieces and fonts
are reused. Diagonal markers and bevel edges are authored. Quick Mission briefing,
selector and button text uses a bundled raster atlas of Noto Sans Bold at 10 px.
It is flat light grey with antialiased edges, with no glyph shadow or bevel.
John requested a cleaner regular bold font at approximately the existing size
on 2026-09-16. Choosing Noto Sans Bold is an agent decision. Its 10 px raster size follows
John's request for a slight further reduction. The font is openly licensed, not derived from retail media; see the
[atlas provenance](../../crates/tore-app/assets/README.md).

Inline field boxes have a one-pixel raised bevel and two pixels of vertical
padding around the 11-pixel glyph cell. Each inline field reserves two extra
pixels on either side, in addition to sentence spaces, so adjacent boxes remain
separate. Ground-target fields retain sentence wrapping and spacing.
Menu magnification uses nearest sampling to avoid blur. Flight UI sampling is
unchanged. Unsupported mission settings still produce a notice at launch.

The blue OK artwork has three additional top rows compared with Cancel.
Its draw origin is shifted up three pixels so both coloured faces align.
The original `ACTDFLT.PIC` cap supplies the striped marker, outside outline and
left rim beside the blue face. It is placed directly before the imported
`ACTDFT0L/M/R` pieces at their shared top edge. No authored replacement border
is drawn over these assets. The OK hit region includes the marker.
Cancel uses only its imported button pieces, without an extra surrounding frame.
Both labels use the flat menu font, centered in the 21-pixel-high face and shifted
down two pixels to match John's supplied close-up. Label placement remains fitted.

Exact original font choice, popup texture placement and field bevel composition remain
unknown. Further research would inspect the original dialog drawing data.

The Aircraft menu label is vertically centered by visible glyph bounds within
y=38..58, matching the [ordnance menu bar](ordnance-presentation.md).

## Mission end

End mission opens the [mission debrief](debrief.md). Its OK returns to this
creator with every setting of the mission just flown, including a custom load,
and the ordnance screen closed. Requested by John on 2026-09-23. Starting the
next mission reopens the ordnance screen for a custom load as before.

## Mission wings

Guns only applies to the player and every member of all six wings, on both
launch and restart. Each aircraft retains ammunition for its own gun; every
other weapon has zero ammunition, including missiles in internal bays. Choosing
Guns and missiles with Standard load restores normal default weapons. Custom
player loads still reject non-gun weapons at launch while Guns only is selected. This scope is an
opinionated requirement requested by John on 2026-09-22. It changes loadout
initialization only. It does not change flight or combat decision rules.

Implementation mode. Normal creator launches use AI for every selected non-player
aircraft, requested by John on 2026-09-17. Friendly Wing 1 includes the player,
so a count of 5 creates four wingmen. The other five wings each launch their
full selected count, up to 29 AI aircraft plus the player. Each wing retains its
own aircraft, experience, side and leader. Friendly Wing 1 follows the human;
the other wings have their own AI leaders. No command-line option is required.
`--ai-wings` remains a shortcut to open the creator.

John requested separate delta formations on 2026-09-17. The host uses the B43
alternating trailing slots with level stacking: slots 1 and 2 are 512 ft right
and left, 512 ft behind; slots 3 and 4 are 1024 ft right and left, 1024 ft behind.
This is an opinionated formation choice; it is not a new recovered formation
name. Idle wingmen track their own leader's moving slots using the
[physical formation and rejoin procedure](ai.md#physical-departure-and-rejoin).
Combat maneuvers can take them out of formation.

Original Quick Mission spawn geometry is **unknown**. Next research: recover
the generator's relative wing placements and situation offsets. Pending that,
the **fitted**, agent-selected placement puts friendly wings 2 and 3 at 4096 ft
left/right and 4096 ft behind the player. Enemy wing 1 starts at the selected
separation, with enemy wings 2 and 3 offset 4096 ft left/right. Enemy aircraft
face the friendly group. Airborne aircraft start at the chosen altitude; a
ground start parks the player's wing instead (see
[Player ground start](#player-ground-start)). Slots rotate with each wing
leader. Close tracking projects the slot along the leader velocity; separated
aircraft use the linked rejoin procedure, with neighborhood clearance and
approach coordination.
Restart restores all six groups exactly as launched.

### Separation

Separation is in nautical miles (6,076.12 ft), as the manual states (p.19:
"between 1 and 50 nautical miles"). John chose nautical miles for every entry
on 2026-09-23. The retail list offers 1, 2, 5, 10, 20 and 50 miles; John
requested 200 and 300 miles on 2026-09-23, and the host appends them in the
retail label style ("200 miles", "300 miles"). Every label means nautical
miles. An out-of-range choice falls back to the 5 mile default instead of
failing.

### Keeping the enemy on the map

John requested on 2026-09-23 that the enemy group is aimed into the map when
the selected separation would put it off the edge. Rule (the numbers are
**fitted** agent choices):

- Every enemy aircraft, including the side wings 4096 ft left and right and
  the formation slots behind them, must start at least one terrain cell
  (8,192 ft) inside the map edge.
- Straight ahead is kept when the whole group fits there. Otherwise bearings
  are tried one degree at a time, nearest to straight ahead first, clockwise
  before counter-clockwise at equal angles. The first bearing that fits at the
  full distance is used.
- In an airborne start the whole airborne scene turns: the player's starting
  heading and the friendly wings turn with the enemy, so the enemy is still
  straight ahead. In a ground start the parked wing cannot turn, so only the
  enemy group moves around the airport and still faces it.
- If no bearing fits at the full distance, the bearing that allows the longest
  distance is used at that distance, and the flight opens with a message such
  as "Enemy forces start 142 miles away: 300 miles does not fit this theater."
  Restart keeps the same placement.

From the default airborne start near the middle of a 208 by 200 cell theater,
200 and 300 miles never fit at full distance; in the 256 by 256 cell Baltic and
Kurile theaters 200 miles fits only toward a corner.

### Home airfields

Every AI aircraft has a home runway for returning to base. The player's
wingmen in a ground start call the departure runway home. Every other aircraft
takes the nearest runway, measured from where it starts, that its side may
use: a friendly field or a neutral one that grants permission, the same test
the player's tower service applies. Allegiance is recorded from the player's
side, so an enemy aircraft's own fields are the ones marked hostile, and the
single neutral-permission flag serves both sides. Ties go to the lower runway
number. An aircraft with no usable runway keeps its start point as home. Every
imported airport is currently neutral with permission, so in practice each
aircraft takes its nearest runway. This choice is **fitted** (agent decision,
2026-09-23).

### Radar and aircraft on the ground

Manual p.208: "Grounded aircraft do not appear on enemy radar until they take
to the air." An aircraft on the ground, parked or rolling, gives no radar
return to anyone: it cannot become a radar contact, be locked, or be picked
through the radar, for the player or for AI. It becomes an ordinary radar
contact on the first moment its wheels leave the ground. Visual and infrared
sighting are unchanged, and its own radar and jammer are still received. The
rule is applied as a zero radar signature while the aircraft is on the ground
(fitted implementation detail).

The existing combat AI remains partial. Scoped [player wing commands and radio](ai.md#live-wing-command-and-radio-integration)
are connected, including recipient outcomes and cancellation. Broader mission
campaign routes and scoring remain. Persistent protection and reviewed missile
acquisition are connected through the current AI services. Separate wing placement and formation following do not
establish combat or retail parity.

## Straight-flight mission fixtures

The compatibility option `--fixture-wings` retains the straight-flight setup
originally requested by John on 2026-09-17. Every populated wing
launches its selected supported aircraft. Friendly Wing 1 includes the player;
its remaining aircraft and every aircraft in the other five wings are dummies.
The source count choices remain 0 through 5 per wing, permitting 29 dummies.
They fly straight and can be observed, designated and hit through shared combat
and sensor rules. They never maneuver, shoot, transmit radar or operate a jammer.
Nationality, skill and advantage do not control behavior or protect a dummy from
selection. These are practice fixtures, not friendly/enemy combat AI.

Agent-selected fitted placement: all dummies start at the player's altitude and
heading, forward at the selected separation in nautical miles (see
[Separation](#separation)), shortened if it does not fit the map. In an
airborne start they turn with the player; in a ground start only the player is
parked and the dummies keep the runway heading. The first is directly ahead. Successive pairs are 500 feet farther
forward and 500 feet farther to either side per pair. Speed is 300 feet/second;
engine heat uses 70% throttle without afterburner. Radius is 28 feet. Aircraft
identity supplies original geometry, texture, radar signature and hit points.
Straight paths can intersect terrain, using the existing collision rules.
Restart restores the accepted formation, stores and fuel. No avoidance is added.

Normal free flight loads the aircraft's supported PT-default weapons as requested
by John on 2026-09-17. The range flag adds diagnostic fixtures; it is no longer
needed for ammunition. The restricted native research adapter and pilot-only input recordings remain
clean to preserve their existing initial conditions.
Weapon compatibility, custom loads and the guns-only creator choice still apply.

## Player ground start

John requested on 2026-09-23 that wingmen start queued on the taxiway near the
runway. With valid airport anchors, place them backwards along the taxi-out
polyline from its last point, facing toward the runway. Fitted spacing is
200 ft, with candidate positions checked every 50 ft and at least 250 ft from
the player's takeoff spot. Every slot must pass the existing pavement and
building checks. If the full queue cannot fit, retain the documented staggered
runway fallback. Only one queued aircraft enters the runway at a time; the
player goes first. This requested placement supersedes parking-slot starts
for Quick Mission. Ordinary parking-start sequences remain supported.


John requested a ground-start choice in the Quick Mission creator on 2026-09-20.
The creator adds Start (Airborne/Ground) and a named airport/runway selector.
Airborne remains the default. Airport choices belong to the selected theater;
changing theater resets the airport choice. Popup cancel preserves the draft.
The accepted start is retained through ordnance setup and mission restart.

Ground start parks the player's whole wing, friendly wing 1, at the selected
airport. The other wings, friendly and enemy, keep the airborne launch.

**Source anchors and player placement (spec-derived).** Each airport's STRIP
shape carries a takeoff spot, taxi route and nine parking slots
([recovered roles](../formats/native-strip.md#remaining-template-callback-boundaries)).
The player starts exactly on the takeoff spot, facing down the runway. Wingmen
use the requested taxiway queue described above. Reviewed TRAIN01.M places
another grounded aircraft in parking; it does not establish Quick Mission's
new queue placement. The takeoff spot is the near end of the host's runway
line, 100 ft behind the earlier fitted single-aircraft start. The anchor
coverage and placement evidence are in the
[ground-start baseline](../baselines/ground-start.md#airport-data).

**Fallback (fitted, agent decision, 2026-09-23).** Vertical-landing pads
(DTSTRP) and the STRIP5A, STRIP6A and STRIP7A fields have no usable points,
because theirs fall off their own paving. So does an airport whose takeoff spot
is blocked or whose taxiway queue cannot fit. There the wing parks
staggered on the runway:

- The player is farthest down the runway, on the centerline. Wingmen stand
  behind, wingman 1 right of the centerline, 2 left, 3 right, 4 left, each
  250 ft farther back than the one before and 40 ft off the centerline.
- The last aircraft stands at the old single-aircraft start point, so the
  player is (wing size - 1) x 250 ft farther down the runway than before.
- Every slot must be on the airport's paving, on a landable surface, and clear
  of buildings (checked 6 ft above the surface). If one fails, the spacing
  tightens to 200, 150 and then 100 ft. If none works, the creator shows a
  notice and nothing launches.
- The 40 ft offset keeps the widest selectable wingspan, the F-14's 64 ft
  spread, within 72 ft of the centerline, inside a 150 ft wide runway.
  Neighbours stand on opposite sides, 80 ft apart across the runway and a full
  spacing apart along it; even 100 ft is more than an F-14's length, so no two
  aircraft overlap.

A player alone (wing size 1) uses the same placement: the takeoff spot, or the
old start point on a fallback field.

Every parked aircraft is stationary with the engine idling, gear and flaps
fully down, brakes applied, afterburner and autopilot off. Aircraft height
includes each aircraft's own wheel/CG clearance above the surface. The
wingmen fly the researched flight model, each with its own fixed seed (the
player 1, wingman *n* 1 + *n*), so a run repeats exactly. Nobody is dropped
from altitude, and restart restores the same layout. Existing B releases the
player's brakes and throttle controls begin the takeoff roll. How the AI
wingmen wait, taxi, take off, return and land is in the
[AI airfield rules](ai-airfield.md). No cold-start sequence is added. These
initial settings and the new UI layout are agent-selected host behavior, not
recovered retail Quick Mission rules.

Other selected wings keep the airborne launch path at the chosen altitude,
positioned relative to the player's slot, with the enemy aimed as described in
[Keeping the enemy on the map](#keeping-the-enemy-on-the-map). If any aircraft
starts in the air, the altitude must clear the airport ground by at least
100 feet; otherwise the creator shows a notice instead of launching. Parked
wingmen do not need that clearance. Ground mode ignores the altitude setting
for the parked wing. Airborne mode retains its existing behavior. Only the
default researched flight model supports ground start; legacy and restricted
native research modes report that incompatibility without silently changing
adapters. Missing or obstructed starts are rejected. Choosing ground start
selects the airport for tower commands but does not grant landing clearance or
announce that a landing has completed. The straight-flight fixtures
(`--fixture-wings`) park only the player.

Parked aircraft do not show on radar until they fly; see
[Radar and aircraft on the ground](#radar-and-aircraft-on-the-ground). The
headless `--ai-probe-ticks N --ground-start AIRPORT` probe gives the player two
wingmen so the parked-wing path is exercised.

[Ground-start validation](../baselines/ground-start.md) records creator launch,
restart, real runway support, takeoff probes and rendering checks.

[Reverse-cycling validation](../baselines/horizon-creator.md).

## Group objectives

All six groups carry objective selectors in ordinary briefing sentences, using
the same `line` layout, font and beveled inline fields as the other mission
parameters. Wing counts use 14-pixel row spacing; objective sentences occupy
rows 201, 215 and 229 in each column. Separate survival-required/optional
sentences occupy rows 249, 263 and 277. Other mission parameters begin at row
301, with the optional airport row at 399, clear of the bottom buttons.

For example, `Your primary target is enemy group 1.` identifies that group as
the player's mission objective. `Your flight will use free fire.` permits any
observed eligible hostile without assigning every enemy as a primary objective.
This objective does not authorize firing at launch. Both sides begin in neutral
formation and follow [leader authorization](ai-awareness.md#formation-and-leader-authorization).
CAP, protection, self-defense, hold and mission inheritance remain available.
Click an objective field to choose, right-click to cycle backward, or use
Tab/arrows and Enter. Survival fields toggle between required and optional.
Inactive groups preserve both settings. [Assignment and target-label rules](ai-awareness.md#quick-mission-objective-stamps)
define the semantics; [validation](../baselines/mission-objectives.md) covers
styling and two opposing-group discrimination.

## Retail map variants

John selected retail map-detail expansion on 2026-09-23. The existing location
picker retains its sixteen base theaters and appends 59 imported MM variants.
Each label contains the original map identity so variants can be distinguished.
Selecting a variant loads its scenery and airport list; choosing another map
resets the airport selection. Nationality/target menus use the variant's base
theater tables. Restart retains the selected layout. The renderer uses the
[static variant contract](terrain-detail.md), not a live campaign simulation.
