# Radar warning receiver presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Scope

This specification defines the cockpit RWR presentation. The simulation owns
emitter reception, missile observation and threat classification. The
instrument displays one actor-owned snapshot and cannot create or alter threat
knowledge.

The RWR retains its original frame, crosshair, two range rings, own-aircraft
center mark and `JAM` label. The top of the scope is directly ahead, the bottom
is behind, the right is 90 degrees and the left is 270 degrees. Maximum
reception remains subject to installed equipment and simulation rules.

## Range

The RWR and the radar scope share one range setting, stepped along the radar
ladder of 5, 10, 25, 50, 100 and 150 nautical miles described in the
[radar specification](radar.md#scope-range-and-targeting-modes). The RWR scale
is that setting capped at 50 nautical miles: it reads 5, 10, 25 or 50 in step
with the radar, and stays at 50 while the radar is at 100 or 150. The
upper-right label, the plotting scale and the out-of-scale markers all use this
capped value. The default setting is 10, so with no saved preference both
instruments read 10 at flight start. Normal sessions save and restore the
shared setting with the other display preferences.

The RWR window's `-` and `+` buttons, the radar window's range buttons and the
keyboard scope range keys (unless the RCS page is the most recently opened
instrument) all step the shared setting one radar ladder position, stopping at
5 and 150. The RWR buttons therefore also change the radar range. Above 50 a
press moves only the radar: `+` at 50 selects 100 with the RWR still at 50, and
`-` at 150 selects 100 with no visible RWR change.

## Symbols and warnings

Draw a ground emitter as a square. Draw a friendly aircraft as an outlined
diamond and an enemy aircraft as a filled diamond. Draw a missile with a dot.
An emitter painting this aircraft is bright. An emitter known to be tracking or
firing at this aircraft flashes. These states require supplied evidence. A
passively received emitter with no lock evidence remains in its ordinary state.
A current supported-radar incoming warning marks its source as firing only when
exactly one independently identified emitter lies within 2 degrees of the
supporting-radar bearing. Ambiguous and unidentified sources remain steady.
Prelaunch painting/tracking states have no fabricated producer.

Show `R` in the lower-right corner for radar warning state and `I` beside it for
infrared warning state. An ordinary detection is dim, a seeker tracking this
aircraft is bright, and an incoming missile warning flashes. `R` describes
radar guidance and `I` describes infrared guidance. Unknown or passive guidance
does not receive a false guidance label.

A missile known from this receiver's evidence to threaten this aircraft
flashes. Other detected missiles remain steady, including friendly missiles and
enemy missiles whose target is unknown. Threat state is receiver-specific. A
missile may therefore flash for one aircraft and remain steady or absent for
another.

The flash phase uses simulation time. Symbols are visible for 60 ticks and
hidden for 60 ticks at 120 Hz, producing one complete cycle per second. Pause
freezes the phase. Render frequency does not affect it.

## Plotting and lifecycle

Plot each contact at its heading-relative bearing. When measured range is
available and falls within the selected scale, radial position is proportional
to that range. A bearing-only observation appears as a short radial tick at the
outer ring. A ranged contact beyond the selected scale appears as a forked
clipped marker at the outer ring. These markers are deliberately distinct and
neither implies a false in-range position.

During the two-second lost-observation grace, retain the last permitted bearing
and range as a dim, steady marker. Stop flashing as soon as current targeting
evidence ends. Remove the marker when the shared threat record expires or the
missile is destroyed, impacts or otherwise leaves its lifecycle.

The RWR window being closed or set to a shorter scale does not affect simulation
knowledge. A failed RWR panel draws no electronic presentation. Receiver failure
does not erase a missile independently seen by a pilot or AI, although the
failed panel cannot present that observation. Audible warning changes are
outside this specification.

## Provenance

The 1999 EA/Jane's *Fighters Anthology* manual at
`.local/missile-update/manual.pdf`, SHA-256
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`,
is the source for the scope orientation, range label, 50 NM maximum, `JAM`,
square, diamond and missile-dot symbols, bright tracking state, flashing firing
or missile state, and the `R` and `I` warning meanings. These appear on printed
pages 94 and 95, PDF pages 98 and 99. Printed page 131, PDF page 135,
distinguishes seeker tracking from an inbound missile and repeats that an
incoming radar missile flashes `R` and its dot, while an incoming infrared
missile flashes `I`.

The shared range with its 50-mile RWR cap is opinionated, requested by John on
2026-09-23, and matches retail screenshots he supplied in which both the RWR
and the radar read 10 at flight start. Stepping the full radar ladder from the
RWR buttons, so that presses above 50 change only the radar, is an agent
decision: it keeps one control rule for both windows instead of a separate RWR
step list.

The exact 60-tick visible and 60-tick hidden cadence, actor-owned threat feed,
bearing-only tick, forked out-of-scale marker, stale appearance and treatment of
unknown guidance are opinionated development rules shared with the
[air-to-air awareness specification](ai-awareness.md#shared-rwr-missile-information-and-display).
The manual establishes flashing but does not establish its exact cadence or
these missing-data presentations.

## Acceptance

Synthetic presentation tests cover the RWR scale at every radar setting,
including the 50-mile cap at 100 and 150, the RWR buttons stepping the shared
range, the 10-mile start, cardinal bearings, ranged, bearing-only and
out-of-scale plots, manual emitter shapes, steady and flashing
missiles, both phases at their exact tick boundaries, stale contacts, `R` and
`I` states, jammer state and receiver failure. A display smoke test must also
confirm the assembled instrument with original runtime art and font assets.
