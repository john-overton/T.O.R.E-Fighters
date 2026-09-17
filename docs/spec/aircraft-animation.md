# Aircraft devices and exterior materials

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-09-16. John requested an animation pass and
orange exterior F-22 canopy tint, with a clear cockpit view. The existing
F/A-18D, Rafale, F-14D, A-4E and X-31 rigs remain supported. This pass completes
initial moving-device presentation on the seven roster additions. No AI or
flight-force changes are authorized or needed.

## Fitted animation

All angles, timing and hinge choices below are agent fits, not measured original
schedules. Source part identities are in [shape notes](../formats/objects-and-shapes.md).
Use the actual imported polygons and preserve texture coordinates when splitting
panels across a hinge. Rotate normals with the polygons. Neutral controls and
fully deployed gear must retain source geometry exactly.

Retain 3-second gear/flap/brake travel. New-aircraft gear rotates rigidly through
90 degrees around aircraft-specific nose/main attachment pivots, then hides at
full retraction. No strut shrinking. Source brake panels rotate from a fitted
closed pose to the original deployed endpoint; no instant popping during travel.
New-aircraft trailing flaps droop up to 0.4 rad, outboard ailerons deflect up to
0.2 rad, pitch surfaces up to 0.3 rad and rudders up to 0.35 rad. Opposite wing
roll deflections use opposite signs. Su-35 canards use 0.25 rad pitch. Existing
rigs retain their documented values and applicable hooks; no hook is added to
an aircraft without a reviewed carrier hook capability.

Gear pivot coordinates below use each shape's source units, in right/forward/up
order. Mirror the main pivot's x coordinate for the left side. Nose groups have
mean forward coordinate greater than 20. Nose gear rotates about x by
-90 × (1 - gear fraction) degrees; main gear rotates about forward by
side × 90 × (1 - gear fraction) degrees.

| Aircraft | Main pivot | Nose pivot | Switched brake pivot / retraction angle |
| --- | --- | --- | --- |
| MiG-29 | (17, 1, 0) | (0, 47, -2) | upper (0,-7,7), atan(9/7); lower (0,-8,-1), -atan(8/6) about x |
| Su-27 | (23, -5, 0) | (0, 60, -3) | (0, 40, 11) / atan(14/22) about x |
| MiG-21 | (20, 0, -1) | (0, 50, -7) | Fitted ventral strip, below |
| Su-25 | (10, 0, -9) | (0, 40, -9) | (±75, -8, 1) / upper atan(5/6), lower -atan(4/6) about x |
| MiG-23 | (3, -1, -3) | (0, 27, -3) | Fitted aft side strips, below |
| Su-35 | (23, 3, -1) | (0, 56, -1) | (0, 23, 8) / atan(10/21) about x |
| F-22 | (13, 0, -5) | (0, 63, -9) | (0,-17,5) / 0.7 rad about normalized (1,±2/3,±1/3) |

Switched brake retraction angles multiply (1 - brake fraction). Their source
pose is the deployed endpoint. MiG-21 has an independently fitted ventral brake
strip at source y=-8..12, hinged at (0,12,-8) about x through 0.6 rad. MiG-23
has fitted aft side strips at y=-24..-16, hinged at (±4,-16,0) about vertical
through ±0.6 rad. These split their own source skin, retaining the surrounding
fuselage and UVs; no switched brake identity or original linkage is claimed.
F-14's existing switched brake panels now rotate continuously through 45 degrees
about the normalized forward-edge direction (1, ±0.5, ∓0.5), at (±2,-11,1).

MiG-23 visual sweep is 0..40 degrees linearly over 400..700 knots TAS, multiplied
by (1 - flap fraction). Rotate the wing panels, their flaps and wingtip vapor
attachments together. This is an agent-authored visual schedule, not a new
sweep-dependent flight law.

## F-22 main weapon bays

Add a manually controlled main-bay presentation with 1-second travel and
90-degree outward-opening doors. Shift+O toggles the bays, and the input action
`bay` can be rebound and recorded. Aircraft without the reviewed F-22 bay rig
ignore the command. On F-22, the existing manual weapon service also requests
open bays while an armed, loaded guided weapon has a designated target. Clearing
the designation, disarming or exhausting the selected station releases that
automatic request; a manual open request remains independent. No launch timing,
weapon eligibility, mass, drag or damage rule changes.

Clip the imported belly panels over the two reviewed source bay rectangles,
retaining surrounding fuselage and the original material on moving doors. The
source switched belly details are visible only while open. This is a fitted
main-bay presentation, not recovered original door sequencing or side-bay parity.
`--flight-bay 0..1` provides an explicit inspection pose, rejected on other planes.

## F-22 canopy

Opinionated appearance requested by John: apply an amber/orange grade only to
reviewed exterior glazing faces. Preserve source texture shading and highlights.
Blend 75% toward amber RGB (0.95, 0.45, 0.08), scaled by 0.3 + 0.7 times source
luminance, retaining 25% of the sampled source color. Frame and surrounding
fuselage materials remain unchanged. Cockpit artwork, HUD and forward-view
world rendering stay clear. Do not alter any engine face on F-22.

John requested 75% opaque exterior glass on 2026-09-16. Keep the grade above
and alpha-composite the nearest glazing surface at opacity 0.75 over the
already rendered aircraft and world. This is actual 25% transparency, not a
reduction in orange tint strength. Agent choice: resolve nearest glass depth
before blending, so overlapping front/back glazing does not compound opacity.
No refraction or new cockpit geometry is introduced.

## Validation and unknowns

Test fixed hinge points, rigid gear lengths, paired surface direction, split
panel continuity, bay timing/reversal/interpolation, unsupported commands and
input roundtrip. Inspect closed/partial/open views and F-22 cockpit/exterior
views. Original schedules, exact mechanical linkage, side bays, original damage transitions, LOD
variants and retail comparison remain unknown. Damaged body rendering follows
the separate [damage and smoke specification](damage-smoke.md). Next research is source shape
control consumers and authored fits can ship without claiming original parity.
