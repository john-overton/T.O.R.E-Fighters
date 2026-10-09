# HUD layout and startup modes

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, requested by John on 2026-09-21. The annotated cockpit
image establishes the requested direction; the pixel positions below are
agent-selected, opinionated layout fits. They apply to all selectable aircraft.
Original HUD font and palette resources remain in use.

## Flight display

Remove the fixed aircraft-datum bars at the center of the HUD. Keep the
kinematic flight-path marker and weapon-computed pipper as separate symbols.
The five-degree pitch ladder retains its positive/negative line styles and
bank orientation. Its visible window spans reference y=182..348 and x=250..390. This trims
the expanded 208-pixel height to 166 pixels, about 20% less area, while
retaining its width and upper edge. Other HUD elements keep their positions.
At John's further request, compress spacing between ladder marks to 75% of its
previous value. His 2026-09-21 correction requires the displayed pitch to match
actual aircraft orientation throughout a climb, dive and vertical transition.
For numbered, nonzero rungs, use the angular difference between rung elevation and aircraft pitch,
then apply the same 0.75 factor to its bank-normal screen displacement about
the aircraft's forward point (320,240). This scales spacing and motion together:
the 70-degree rung crosses that point at 70 degrees, not earlier. At bank zero
and unit zoom, a rung five degrees above pitch is about 27.28 pixels above it.

Agent geometry decision: project rung bearing in a local zero-pitch frame using
the relative elevation. This retains level-flight rung widths at steep attitudes,
rather than shrinking them to zero at the poles. Rotate the complete scale by
bank. Draw marks from -90 through +90 inclusive in five-degree steps, preserving
negative dashes and applying the same transform to labels. There is no special
vertical fallback or discontinuous change in compression. The +/-90 rung passes
through the forward point at vertical climb/dive, including through a full loop.

The zero-degree bar is a separate world-projected horizon reference, requested
by John on 2026-09-21 after reviewing level flight. Draw it with the same
uncompressed world projection as the flight-path marker. A level velocity vector
therefore lies on this bar even when positive angle of attack puts the nose
above the horizon. Bank, zoom and head-look apply consistently. Do not draw a
second compact zero bar. The gap between the true horizon bar and nearby compact
numbered marks is consequently not uniformly compressed, an agent implementation
choice that preserves both horizon alignment and accurate numbered pitch readings.

Do not anchor the numbered scale to the horizon: that previously produced false
pitch readings and a steep-flight gap. Keep its corrected calibration and +/-90
marks unchanged. The flight-path marker, target cues and weapon pipper retain
their world projection. No aircraft orientation, flight response, HUD zoom or
head-look behavior is changed.

Remove both the surrounding TAS/MSL numbers and their hash marks. Keep the
TAS/MSL labels and boxed current values. Move both boxes from reference y=223
to y=235, a fitted twelve-pixel downward adjustment that leaves clear space
above and below. Their horizontal positions stay unchanged.

Normal navigation omits AGL and vertical speed. Retain them for active ILS
approaches when weapon readouts are inactive: AGL at (402,259) beneath altitude
and V/S at (207,271) beneath airspeed, below the NAV status label. The bank scale sits just
below the ladder: its center tick is at y=358, the fixed index spans y=359..366,
and the centered numeric label sits at y=374. This raises the scale by 66
reference pixels without changing its width or bank-angle mapping. Make it a shallow circular
arc 223 reference pixels wide, showing angular offsets within plus/minus
30 degrees of the current bank. Ticks remain ten degrees apart, with numeric
labels every thirty degrees and full-roll wrapping. The fixed index is below
the arc. These are display geometry choices, not aircraft bank limits.

The scale turns with the horizon. In a right bank the horizon turns
counter-clockwise on the HUD, so the zero mark moves right of the index and the
index reads the bank angle from the marks to the left of zero; a left bank is
the mirror image. Before 2026-09-28 the scale turned against the horizon, which
the player reports in GitHub issue #1 called out.

The scale is gyro-driven, requested by John on 2026-09-28: the bank it shows
follows the aircraft's bank through a damped spring (natural frequency 10
radians per second, damping ratio 0.7) instead of copying it every frame. It
trails a steady roll by about 0.14 seconds, settles within about 0.6 seconds of
a sudden change, overshooting a 30 degree step by about 1.4 degrees, takes
the short way through 180 degrees, holds while paused and starts each flight
at the aircraft's bank. The pitch ladder, horizon and flight path marker still
follow the aircraft exactly. The constants are fitted agent choices.

At John's request, weapon information sits directly below the boxed values.
Status, ammunition, mode/estimate and readiness occupy x=207, y=259/271/283/295
below airspeed. Range, closure and aspect occupy x=402, y=259/271/283 below
altitude. These fitted positions retain a gap after the current-value boxes.
The full HUD clip extends through y=450. The target cue and gun-pipper inset
extends downward through y=380; off-HUD cues retain true three-dimensional
bearing. Zoom, window aspect and head-look use the existing HUD transform.

## Nosewheel authority

Requested by John on 2026-10-02: show `NSW 100%` in the normal HUD font and
color beneath BRAKE. The [steering spec](lateral-flight.md) defines when it
shows and the percentage; inactive steering has no label. Fitted layout: NSW at (388,173), HOOK at (388,184),
and MSL at (402,201), leaving ten-pixel glyphs clear of each other.

## Powered-lift cluster

VTOL overhaul decision 11 (John, 2026-10-08): the AV-8, Yak-141, helicopters
and V-22 get a proper HUD cluster drawn over the borrowed HUD art in the same
font and colour. Opinionated (agent-chosen positions, fitted at 640x480); the
retail manual (pp. 62, 81, 153) describes the hover display. Code:
`crates/tore-app/src/powered_hud.rs`; the readings are plain functions of the
flight state in `crates/tore-sim/src/flight/powered/readout.rs`.

| Element | Shown on | When | Where and what |
| --- | --- | --- | --- |
| Nozzle angle | AV-8, Yak-141 | Nozzles not at 0 | `NOZ 60` in whole degrees at (388,274), a 60-pixel gauge for 0 to 100 degrees under it with a mark at 90 and the nozzle's pointer, and a caret under the gauge at the demand while the nozzles slew. The retail `VCTR` cue is in the borrowed art |
| Lift engines | Yak-141 | Running | `LIFT` at (430,274) |
| Hover display | Jets below their stall speed; helicopters and the V-22 below 40 kt ground speed | Navigation HUD only (not the weapon HUD) | Cross hairs and a 22-pixel circle centred at (320,296): the circle moves with the ground velocity (2.2 pixels a knot, forward up), so its radius is 10 knots and its forward edge over the hairs means drifting back at 10 knots. Vertical bars at x=376 (80 pixels, 1.6 pixels per ft/s): a tick rides with the climb or sink, the centre marks are zero sink, the long cap at the bottom is the lower edge |
| Rotor speed | Helicopters, V-22 | Always | `NR 100` at (211,292); flashes more than 10 points below or 5 above its governed reference (below 90 and above 105 percent, or 74 and 89 around the V-22's 84 on the downstops) |
| Torque | Helicopters, V-22 | Always | `TQ 72` at (211,304), percent of rated power; flashes above 100 |
| Collective | Helicopters, V-22 | Always | `COL 81` in the throttle readout's place, (235,178) |
| Radar height | All six | Below 1,000 ft above ground | `R 450` at (388,262) |
| Stability level | All six | What really acts is not the Damper (Off without hydraulics) | `SAS OFF` or `SAS ATT` at (211,316); `SAS EZ DMP` (a jet at Off damped by the Easy flight physics cheat) or `SAS EZ ATT` (a rotorcraft given the cheat's weak attitude retention, even at Damper) while the cheat supplies it |
| Nacelle | V-22 | Always | `NAC 75` at (388,274), a vertical tape at x=446 from 0 degrees (bottom) to 97.5 (60 pixels) with a pointer at the nacelle and a caret at the demand, and `CONV` at (388,286) while the conversion protection moves or holds the nacelles. Bars beside the tape bracket the nacelle angles the conversion corridor allows at the current indicated airspeed |
| Autopilot | Helicopters, V-22 | Hover hold engaged | `AUTO` above `HOVER` in the existing autopilot label slot: the hover hold mode only has to give its autopilot label the word `HOVER` |

Flashing rows are on for 30 ticks and off for 30 (a quarter second). The
lower edge of the vertical bars is the same scale on every aircraft; the
manual's "stall rather than sink" mark for the jets is fitted to it rather
than computed. The cluster is a list of marks, a pure function of the flight
state, so each row is tested by what it puts where; `--hud-snapshot PATH
[--hud-snapshot-state forward|hover]` draws the HUD of `--aircraft` headless.

## Cockpit glass and layer order

Requested by John after the layout checkpoint was committed. When cockpit
artwork is visible, confine the forward HUD to the aircraft's reviewed glass
aperture in source-art coordinates. Use the cockpit's existing translation,
cover-fit scale and zoom for the aperture; it must follow the glass during
head-look and resizing rather than remain at a fixed screen rectangle.
The aperture is a fitted polygon based on visual inspection of the user's
imported cockpit image, not recovered original clipping behavior.

Composite the world first, then the masked HUD, then the premultiplied cockpit
art and live mirror contents. Opaque frame pixels must cover HUD symbols;
transparent glass must reveal them. Preserve smooth artwork edges, shared
cockpit/HUD fading, palette lighting and independent instrument/menu overlays.
Apply this to flight symbols, weapon cues and target markers alike. Clipping
can hide lower rows on smaller glass apertures; this pass does not reflow them.

When the cockpit is switched off or hidden by the existing below-1x wide-view
mode, retain the independent HUD without the invisible glass mask. Turning off
the HUD must leave cockpit art and mirrors intact. Cache the source-size mask
per prepared aircraft; do not rebuild or read back it every frame. Only the
reviewed cockpit dimensions are accepted. An unreviewed source uses no cockpit
HUD aperture until its glass is reviewed, rather than leaking across the frame.
No imported picture or generated mask is committed.

## Startup and navigation mode

Normal ground starts select NAV, with weapons disarmed. Normal airborne starts
select and arm the canonical gun. New flights and restarts, including Quick
Mission, use these defaults. NAV suppresses weapon-specific HUD symbols and
retains selected-target cues. The shared status position (207,259) shows NAV in
navigation mode, LCOS for the armed gun, and ARM for missiles, as requested by
John on 2026-09-21. Active ILS vertical speed sits one row below NAV to avoid overlap.
Bracket keys cycle through NAV and weapon slots;
arming follows selection. See [selection and instrument rules](weapon-navigation-selection.md).

Explicit command-line weapon selection overrides the default gun. An explicit
weapon slot or live-fire range suppresses the implicit ground NAV default;
an explicit airport-probe NAV flag takes precedence. Recording/replay and
combat probes retain their established diagnostic setup, and live-fire remains
armed. These exceptions are developer facilities, not alternate player defaults.

[Validation and visual evidence](../baselines/hud-cleanup.md#expanded-hud-and-startup-review).

[Horizon reference validation](../baselines/horizon-creator.md).
