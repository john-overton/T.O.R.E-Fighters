# Load Ordnance presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-09-16. The supplied retail screenshot guides
presentation; its executable identity is unknown. Recovered card positions and
loadout interactions remain documented in the [ordnance contract](../formats/ordnance-menu.md).
Store placement, fuel steps, launch validation, flight adapters and combat
behaviour retain their existing contracts. Catalog visibility is specified below.

## Catalog availability

John requested this opinionated visibility rule on 2026-09-23: hide weapons
whose flight behaviour is not hooked up yet, including when Cheat is enabled.
An imported definition alone does not make a weapon available. The catalog uses
the same supported-weapon list as flight launch validation.

With Cheat off, show a supported weapon only when at least one station on the
selected aircraft can load it, with an editable capacity of 1 through 32,766.
With Cheat on, show every imported supported weapon, regardless of aircraft
compatibility. Station placement still follows the existing Cheat rules,
including fixed-station restrictions. Visibility does not bypass launch weight
checks, sensor requirements or weapon firing limits.

Toggling Cheat still unloads every station. Rebuild both catalog categories,
reset their pages to the first page and clear the catalog selection. Keep the
existing display-name ordering and eight cards per page; an empty category has
no selectable cards and its page rocker stays on the first page. Unsupported
weapons return only after their flight behaviour is implemented and validated,
following the [weapon update passes](../ROADMAP.md#weapon-catalog-update-passes).
Validation is recorded in the [catalog baseline](../baselines/ordnance-catalog.md).

## Presentation

The original ORD_AIR3 background, thumbnail images, dial, rocker and button
pieces remain runtime imports. Catalog cards use two columns at x=68/187,
four rows from y=108 at 68-pixel spacing. Station headings retain x=350/469,
three rows from y=121 at 71-pixel spacing. Card names, quantities, location
labels and numeric fields use the approved 10 px Noto Sans Bold atlas from
[Quick Mission](quick-mission-menu.md). This is an agent-selected extension of
the approved font, not evidence of the original font face. Selected weapon names
are yellow and catalog mass/guidance are blue. Names must fit 111 pixels.

John requested centered thumbnail boxes on 2026-09-22. Both catalog and station
black wells are 113 pixels wide. Their left edges are x=66/185 for the catalog
and x=351/470 for stations. The 109-pixel outlines start two pixels inside each
well, at x=68/187 and x=353/472 respectively. This leaves equal two-pixel side
margins on selected, unselected and empty cards. Thumbnails are centered within
the 109 by 23 pixel outline in both directions; original 105 by 19 pixel images
have two pixels of inset on each edge, including the outline. These measured
background bounds guide fitted placement, not a retail interaction claim.

The page and fuel rockers use the retail rocker frames and behaviour of the
[debrief rocker](debrief.md#presentation): level at rest, tilted while pressed,
acting on press and springing back on release. Page turns sound the rocker;
fuel changes keep their fuel cue. Arrow keys tap the page rocker. Requested by
John on 2026-09-23.

The category dial uses DIAL13 for air-to-air and DIAL11 for air-to-surface,
at (148,393), pointing toward the corresponding category lamp. LIGHTON and LIGHTOFF overlay both background lamps at (115,394) and (115,422),
so active and inactive indicators both use the original blue outer rim. These images were visually inspected; the angle choices
and coordinates are fitted to the reference. They keep the entire dial and shadow
inside the white category frame and do not cover either label or indicator.
The existing category hit regions and keyboard shortcuts are retained.

Vehicle weights have comma-separated integer pounds and align to x=467 inside
their black fields. The available-weight value is shifted down two canvas pixels
to y=381 following John's visual correction. Fuel pounds are centered in (487,354,56,14). Fuel percentage
shows the number only in (487,375,27,14), since the background supplies the percent
sign. The page count is centered in (248,388,48,15). Text centering measures the
visible glyph pixels, excluding transparent font padding.

Select Plane and Fly share Quick Mission's button composition and label offsets.
Fly includes the original ACTDFLT striped cap joined to ACTDFT0L/M/R. Select Plane
uses ACTION0L/M/R without an added outer outline. Labels retain the approved
lower position; pointer hit regions include Fly's cap. Button actions are unchanged.

Top menu labels retain MENUFONT. Ordnance and Quick Mission labels are centered
vertically inside y=38..58, using visible glyph bounds. Ordnance Weapons and
Airbase begin at x=103 and x=178; matching hit regions do not overlap. Main-menu
bar labels are centered within their existing interactive rectangles. The exact
original font alignment and dial placement remain unknown; these are fitted rules.

### Game messages

John requested single-line ordnance messages with a background sized to the text
on 2026-09-23. This is an opinionated presentation rule. Preserve the existing
message content, pale text and dark background colours. Agent-selected layout:
anchor the strip at (30,335), with four pixels of horizontal padding and two
pixels of vertical padding around the font row. The background is only as wide
as the displayed text plus padding and only one font row tall plus padding.
Collapse whitespace to single spaces. Messages wider than 572 pixels end with
`...` within that width, so the strip never exceeds 580 pixels or wraps.
An empty message draws no strip. Other screens retain their own message layout.

## Dragging and empty stations

John requested visible weapon dragging, unloading into the catalog, station
transfers and persistent empty-card outlines on 2026-09-22. These requested
interactions are opinionated requirements; thumbnail art and quantity steps
come from the linked recovered contract.

While dragging, only the imported weapon thumbnail follows the pointer, with
its transparent pixels preserved. The system cursor is hidden during the drag.
No card border, name or quantity follows it.
Catalog drops fill a compatible station to its capacity. Station-to-station
drops move one quantity step: 1 for capacity up to 100, 10 for 101 through 300,
and 100 above 300. Transfers stop at the available source quantity and free
destination capacity. A different destination weapon is replaced. Incompatible
drops preserve both stations and display the existing compatibility notice.
Dropping onto the source station leaves its quantity unchanged.

Dropping a station weapon anywhere in the eight-card catalog area, including
unused card spaces, empties that station completely. Other releases leave the
load unchanged. Escape, focus loss or leaving the menu canvas cancels dragging.
John also requested adding one weapon by left-click on 2026-09-22. A matching
left press/release on a loaded station adds exactly one round or store when
capacity remains, regardless of a prior catalog selection. A full station is
unchanged. Clicking an empty station loads the selected catalog weapon as before;
dragging from the catalog can replace a loaded station's weapon. Right-click
decrement and keyboard quantity steps retain their recovered scaling.
Agent-selected input details: movement of 3 canvas pixels starts a drag; the
thumbnail is centered on the pointer; only catalog drops unload, so releasing
over unrelated controls cannot discard stores. These details are fitted.

Every empty station, including an empty internal gun station, keeps its red
109 by 23 pixel thumbnail outline. The weapon image, name, quantity and Empty
label are absent. The station's location heading remains above the outline.

## Sound effects

Successful loading, quantity changes, unloading and station transfers use the
retail ordnance cue once per completed edit, without the standard menu click
layered over it. Ordinary stores use `&ARMWPN.5K` (about 0.633 seconds);
ammunition-marked weapons use `&ARMBLLT.5K` (about 0.419 seconds). Classification
comes from the imported weapon's ammunition flag, not its filename or station
location. This is spec-derived behavior from the
[retail sound selection](../formats/ordnance-menu.md#sound-selection).

Use the weapon being loaded or transferred to choose the cue; for an unload,
use the weapon being removed. A full station, empty decrement, rejected drop,
same-station drop, canceled drag or pointer movement plays no ordnance cue.
The requested one-at-a-time left-click addition uses the same successful-edit
cue as other quantity changes. Catalog selection and unrelated menu actions
retain their existing button feedback. The Unload All menu item also retains
its existing button feedback; its original dedicated sound remains untraced.

Fuel edits use `&ARMDRIP.11K` (about 0.257 seconds) when the fuel amount changes.
Do not stack or restart that sample while it is already playing. At either fuel
limit, no fuel cue plays. Sound-effects Off and `--no-audio` silence these cues.
Playback uses the imported unsigned PCM8 mono samples at the existing reader's
5,512 Hz / 11,025 Hz rates. Original device volume and shell repeat cadence are
unverified; the host retains its current effects mix and discrete edit events.
