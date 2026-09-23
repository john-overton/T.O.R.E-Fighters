# Weapon selection and NAV instrument

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Selection

Opinionated, requested by John on 2026-09-21: `[` selects the previous item and
`]` the next in a wrapping ring containing NAV and the configured weapon slots.
Selecting a weapon arms it. NAV disarms and releases the trigger. There is no
independent player master-arm control. Semicolon and U no longer select or arm.
The agent retains configured station order, including empty and failed stations,
so selection does not hide store status. Ground starts use NAV; airborne starts
retain the canonical gun, now armed. Existing headless range commands and old
recordings keep their explicit station and arm commands for compatibility.

Boresight IR audio follows the eligible target actively tracked under the bore
and its displayed percentage without requiring designation, as requested by
John on 2026-09-23.
The [audio guide](../audio.md#seeker-growl) defines availability and lock gain.
A radar missile in boresight sounds its lock tone on its bore return without
designation, and radar tones stop inside minimum range (John, 2026-09-23). Acquisition, guidance and
NAV selected-target cues are unchanged.
John also requested NAV at the HUD status position on 2026-09-21; armed guns
show LCOS there, and missiles show ARM. [HUD placement](hud-layout.md).

## NAV INFO

Opinionated, requested by John on 2026-09-21: the first two instrument buttons
are minus and plus, selecting previous and next destinations with wraparound.
The third button switches mode 1 (mission waypoints) and mode 2 (airports).
The supplied NAV INFO screenshot is presentation evidence only: three lettered
rows, bearing and distance beneath each name, selected row brighter, ETA below.
Reuse the existing instrument frame and imported font. Mode number on the third
button identifies the active source; show the source in the empty-state message.

Agent choices: list airports nearest first by horizontal distance to their
nearest usable runway center, breaking ties by airport ID. Keep selection by ID
as distances change. Eligible airports are friendly, or neutral with landing
permission, and must have a usable runway. Hostile, unknown, unpermitted neutral
and disabled airports are excluded. This denotes landing eligibility, not a
promise of freedom from nearby threats. The existing base-layout host explicitly
assigns neutral landing permission; mission allegiance is not inferred.

Show three rows per page containing the selected destination. Distances use
6,076.12 feet per nautical mile and one decimal place. Bearings are true headings,
rounded to integer degrees modulo 360. ETA is direct horizontal distance divided
by current horizontal speed, displayed as minutes:seconds; below 1 foot/second,
show `ETA --:--`. Names are clipped to the instrument's text width.

Mission route import is not implemented in this host. Mode 1 honestly displays
`NO WAYPOINTS` until a mission supplies ordered named coordinates; no route is
invented from the screenshot. Next research step: recover and specify the player
mission route data grammar before importing it. Airport selection feeds the
existing landing service, does not request clearance, and does not switch the
weapon/NAV selection. This change adds no automatic route sequencing or flight
control.

## WEAPONS instrument

Opinionated presentation requested by John on 2026-09-21, using his supplied
WEAPONS screenshot: list ammunition count and imported short weapon name, mark
the selected weapon with `>`, and show live CHAFF and FLARE counts at the bottom.
The agent groups identical source weapons and sums their remaining rounds.
NAV has no selected-weapon marker. Six rows fit each page; the third button `P`
wraps pages and does nothing on a single page. Minus/plus use the same selection
ring as the bracket keys and reveal the selected weapon's page. Detailed range
and system diagnostics remain in the existing diagnostic overlay.

[Validation results](../baselines/weapon-navigation-selection.md).
