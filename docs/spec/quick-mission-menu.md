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

The selector shows 15 rows per page, black inset wells, blue diagonal markers,
a gold selected marker, a page count, a Prev/Next rocker, and beveled OK/Cancel
buttons. Arrow keys move selection, Home/End reach the ends, and Page Up/Down
move 15 entries. Pointer paging preserves the pending selection until another
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

## Mission wings

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
[fitted signed speed correction](ai.md#b43-wing-commands-and-formation-variation). Combat maneuvers can take them out of formation.

Original Quick Mission spawn geometry is **unknown**. Next research: recover
the generator's relative wing placements and situation offsets. Pending that,
the **fitted**, agent-selected placement puts friendly wings 2 and 3 at 4096 ft
left/right and 4096 ft behind the player. Enemy wing 1 starts at the selected
separation, with enemy wings 2 and 3 offset 4096 ft left/right. Enemy aircraft
face the friendly group; all start at the chosen altitude. Slots rotate with
each wing leader. The **fitted** formation steering point projects its slot
three seconds along its leader's heading, while speed regulation uses the
signed along-track error to the unprojected slot. This avoids circling a slot that is already reached.
Restart restores all six groups.

The existing combat AI remains partial. Pursuit steering, missile warning
delivery, live countermeasures, weapon realization and player wing orders have
known integration gaps. Separate wing placement and formation following do not
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
heading, forward at the selected separation interpreted as statute miles
(5,280 feet). The first is directly ahead. Successive pairs are 500 feet farther
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
