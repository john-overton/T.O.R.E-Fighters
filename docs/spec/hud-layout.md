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
previous value. After perspective projection, scale only the component normal
to the banked rungs about HUD center (320,240). Apply the same transform to
rung labels. Rung width and five-degree labels stay unchanged. This is a compact
attitude scale, so ladder marks no longer line up directly with world angles;
the flight-path marker, target cues and weapon pipper retain their existing
world projection.

Remove both the surrounding TAS/MSL numbers and their hash marks. Keep the
TAS/MSL labels and boxed current values. Move both boxes from reference y=223
to y=235, a fitted twelve-pixel downward adjustment that leaves clear space
above and below. Their horizontal positions stay unchanged.

Normal navigation omits AGL and vertical speed. Retain them for active ILS
approaches when weapon readouts are inactive, at reference y=426. The bank
scale remains the non-weapon display's bottom cue. Make it a shallow circular
arc 223 reference pixels wide, showing angular offsets within plus/minus
30 degrees of the current bank. Ticks remain ten degrees apart, with numeric
labels every thirty degrees and full-roll wrapping. The fixed index is below
the arc. These are display geometry choices, not aircraft bank limits.

Weapon status, ammunition and mode/estimate rows move down by 116 reference
pixels, beginning at y=395. Radar range/closure/aspect move down with them.
The full HUD clip extends through y=450. The target cue and gun-pipper inset
extends downward through y=380; off-HUD cues retain true three-dimensional
bearing. Zoom, window aspect and head-look use the existing HUD transform.

## Startup and navigation mode

Normal ground starts select NAV and master arm SAFE. Normal airborne starts
select the aircraft's canonical gun with master arm SAFE. Apply these defaults
to a new flight and restart, including Quick Mission. A safe selected gun shows
its ammunition and SAFE label without a firing pipper. The status overlay
identifies NAV and GUN explicitly instead of showing a missile seeker mode.

NAV suppresses weapon-specific HUD symbology and retains the selected target
cue. Explicit weapon cycling leaves NAV. Master-arm changes do not themselves
change the selected navigation mode. NAV remains the existing navigation/display
mode; it is not an additional weapon-release interlock. Master arm controls
release safety.

Explicit command-line weapon selection overrides the default gun. An explicit
weapon slot or live-fire range suppresses the implicit ground NAV default;
an explicit airport-probe NAV flag takes precedence. Recording/replay and
combat probes retain their established diagnostic setup, and live-fire remains
armed. These exceptions are developer facilities, not alternate player defaults.

[Validation and visual evidence](../baselines/hud-cleanup.md#expanded-hud-and-startup-review).
