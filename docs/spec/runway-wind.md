# Runway wind limits and ground coupling

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, requested by John on 2026-09-21. These are opinionated TORE
runway-wind rules, not real-aircraft operating limits or recovered retail values.
They provide ground handling difficulty and explicit warning data. Reaching a
threshold does not crash the aircraft, lock controls, deny tower clearance or
change AI behavior.

## Maximum-takeoff-weight classes

Classification uses the selected aircraft's imported maximum takeoff weight.
It never uses empty weight, current fuel, current payload or current gross weight.
Boundary values enter the heavier class except that 200,000 pounds remains in
the 80,000 through 200,000 pound class.

| Imported maximum takeoff weight | Noticeable crosswind | Rough crosswind | Crosswind limit |
| --- | ---: | ---: | ---: |
| Below 6,000 lb | 6 kt | 10 kt | 15 kt |
| 6,000 to below 20,000 lb | 8 kt | 14 kt | 20 kt |
| 20,000 to below 80,000 lb | 10 kt | 18 kt | 30 kt |
| 80,000 through 200,000 lb | 13 kt | 22 kt | 33 kt |
| Above 200,000 lb | 15 kt | 26 kt | 38 kt |

Tailwind limit is 10 knots for every weight class. Headwind has no ground-rule
penalty. Crosswind is signed for left/right presentation, while severity uses
its absolute value. Severity is below-noticeable, noticeable, rough or limit.
The combined at-limit flag is set when absolute crosswind reaches the class limit
or tailwind reaches 10 knots.

## Wheel-contact influence

Use full atmospheric wind for aerodynamic airspeed, lift and rotation. The
thresholds below adjust remaining tire grip; they no longer scale or subtract
a wind-velocity vector. Headwind has no runway-difficulty penalty, but its
actual airflow can shorten the ground roll needed to reach liftoff airspeed.

The crosswind difficulty fraction is zero through noticeable, interpolates
linearly to 0.5 at rough, and reaches 1 at the class limit. Tailwind difficulty
is tailwind divided by ten knots, clamped to 0..1. Use the larger fraction and
multiply it by the fitted rolling transition `t*t*(3-2*t)`, where `t` is ground
speed divided by five knots, clamped to 0..1.

Lateral tire scrub is multiplied by `1 - 0.5*fraction`, retaining at least half
its base strength even at the wind limit. Multiply all tire forces separately
by remaining wheel load. Longitudinal rolling and wheel braking receive no
additional wind-difficulty reduction. The [takeoff/contact rules](takeoff-ground-contact.md)
define unloading, static hold and aerodynamic wind separation.

Airborne wind and legacy/restricted adapter behavior stay distinct. Thresholds
continue to provide the requested difficulty cues, not automatic crashes,
control locks or clearance denial.

## Runway HUD cues

The HUD shows signed crosswind and the applicable class limit in knots, plus
`CALM`, `NOTICE`, `ROUGH` or `LIMIT`. A nonzero tailwind adds its value and the
universal ten-knot limit; reaching that limit adds `LIMIT` to the tailwind cue.
These display labels and layout are agent-selected fits for the requested
operating difficulty. They do not change tower clearances or landing scoring.

On a supported runway, use the runway end closest to aircraft heading. During
ILS guidance, use the selected approach end. This makes departure and approach
warnings runway-relative, while wheel-force coupling follows the aircraft's
actual wheel heading. Positive crosswind pushes toward runway right. Away from
a supported runway or ILS guidance the runway-specific cue is hidden.
