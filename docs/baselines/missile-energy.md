# Missile energy calibration and validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-27. Model calibration, not retail
parity or a validated engineering flight envelope. Semantic model identity,
branch `missile-overhaul`, Linux x86-64. Input came from the isolated imported
profile for this task; no media bytes are committed. The shared offline probe
is `cargo run --locked --release -p tore-sim --example missile_energy_ranges
-- LOCAL_JT_DIRECTORY`. `--fit-class` exports fixed-budget one-anchor candidates for tools/missile_energy_fit.py; `--verify`
checks tail reach, altitude, generation ordering, finite shots and local
parameter sensitivity. Neither changes source or runs during gameplay.

This baseline contains Phase 2.5 class budgets and post-burn inert defeat. The earlier Phase 3
study is invalidated, remains labelled pre-fix in local scratch, and has not
been resumed. This pass is calibration and regression validation only.

## Conditions and limits

A2A launcher velocity is 300 m/s, target 250 m/s, wings level, no wind or
jammer, flat sea-level ground. Altitudes are MSL. Head, beam and tail mean
180, 90 and 0 degrees target heading relative to launch heading. All distances
below are km of initial separation, not missile path length. Numbers are
rounded to 0.01 km. The sweep samples ranges by factor 1.5 from 100 m to
1000 km and adds the shared anchor/loft-transition probes, then bisects the last
successful interval ten times. It can miss a
narrow disconnected island of success. It is not a mathematical proof of a
continuous no-escape envelope. Fitting and regression diagnostics
use twelve bisections instead of ten, so their last reported decimals can
differ slightly from the complete sweep without a physics change.

Physical reach uses the same 120 Hz motor, atmosphere, lift, drag, PN, loft,
cruise path and swept fuze kernel as live motion, with continuous ideal target
observations. Seeker-valid reach calls the live actor-owned guidance routine
each tick, including slew/FOV, memory, activation and terrain-Doppler rejection.
IR requires a valid mounted return at launch; target heat is military and both
signature fields are the explicit normalized 100 reference. Radar has ideal
continuous launcher support, so this column does not include a particular
aircraft radar's acquisition range or scan delay. There is no synthetic
percentage reduction from the physical result. N/A means the game profile is
held or uses the separate laser/command channel, not zero kinetic reach.
The ideal-observation trajectory is a reference, not an upper bound over all
possible guidance histories. Seeker memory and activation can change steering
and energy expenditure, so some seeker-valid maxima exceed that reference.
Together with the sampled search, this limits how literally either maximum
should be read as a continuous launch boundary.

Public facts and the deliberately fitted anchor conditions are separated in
[the data evidence](../formats/missile-energy-data.md#per-store-motors-and-class-control-budgets).
All conditional anchors are agent choices unless explicitly sourced. The
M/X and R73 high-head anchors have been restored to their earlier lower values. Physical head-on reach does not give a rear-only seeker
a head-on lock. Held records remain unavailable in gameplay.

## Complete A2A physical reach

Each cell is head / beam / tail, km. All 25 inventory records are included,
including held records measured diagnostically without enabling them.

| Store | Primary reference km | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | ---: | --- | --- | --- | --- |
| AA10.JT | 73 | 24.50 / 15.65 / 11.01 | 28.02 / 17.81 / 12.60 | 40.86 / 24.08 / 17.22 | 73.00 / 49.01 / 34.61 |
| AA11.JT | 30 | 21.79 / 15.74 / 11.79 | 22.84 / 16.90 / 12.85 | 25.20 / 19.41 / 15.21 | 29.75 / 24.13 / 19.74 |
| AA11B.JT | 30 | 21.79 / 15.74 / 11.79 | 22.84 / 16.90 / 12.85 | 25.20 / 19.41 / 15.21 | 29.75 / 24.13 / 19.74 |
| AA12.JT | 50 | 17.16 / 7.91 / 4.75 | 20.02 / 9.18 / 5.56 | 30.39 / 13.05 / 7.98 | 50.00 / 24.70 / 15.84 |
| AA2.JT | 20 | 12.56 / 7.11 / 4.84 | 14.54 / 8.18 / 5.60 | 17.28 / 10.71 / 7.28 | 19.99 / 13.78 / 9.99 |
| AA6.JT | 30 | 9.77 / 5.71 / 3.83 | 11.07 / 6.49 / 4.40 | 14.94 / 8.78 / 6.08 | 30.06 / 17.48 / 12.50 |
| AA8.JT | 10 | 6.10 / 2.11 / 1.35 | 7.03 / 2.45 / 1.60 | 9.93 / 3.54 / 2.34 | 14.04 / 6.53 / 4.04 |
| AA9.JT | 120 | 11.85 / 6.54 / 2.77 | 34.26 / 20.38 / 13.24 | 44.66 / 26.98 / 18.03 | 120.00 / 54.19 / 35.14 |
| AAML.JT | 150 | 35.77 / 22.68 / 14.23 | 41.94 / 25.67 / 17.11 | 56.43 / 34.47 / 23.41 | 150.00 / 69.97 / 46.83 |
| AEMP1.JT | 50 | 17.16 / 7.86 / 4.74 | 20.02 / 9.14 / 5.55 | 30.39 / 13.02 / 7.97 | 50.00 / 24.69 / 15.83 |
| AIM120.JT | 60 | 26.71 / 14.19 / 9.06 | 31.89 / 16.44 / 10.55 | 51.20 / 23.05 / 14.87 | 60.00 / 36.10 / 25.27 |
| AIM54C.JT | 150 | 11.74 / 6.60 / 2.89 | 41.60 / 25.91 / 17.43 | 54.52 / 34.19 / 23.49 | 150.00 / 69.59 / 44.86 |
| AIM7.JT | 70 | 38.09 / 15.89 / 9.63 | 46.49 / 24.23 / 16.25 | 59.38 / 33.93 / 22.43 | 70.00 / 47.22 / 34.81 |
| AIM7E.JT | 35 | 10.74 / 4.16 / 2.57 | 12.24 / 4.84 / 3.02 | 16.83 / 6.88 / 4.38 | 34.99 / 14.80 / 9.69 |
| AIM9B.JT | 20 | 13.72 / 7.67 / 5.14 | 15.90 / 8.84 / 5.95 | 17.33 / 10.84 / 7.33 | 19.65 / 13.41 / 9.65 |
| AIM9M.JT | 18 | 12.93 / 7.68 / 5.24 | 14.75 / 8.75 / 6.03 | 17.97 / 11.54 / 7.97 | 21.36 / 15.31 / 11.36 |
| AIM9X.JT | 18 | 12.88 / 7.68 / 5.24 | 14.69 / 8.76 / 6.03 | 18.00 / 11.59 / 8.00 | 21.43 / 15.39 / 11.43 |
| MICA.JT | 60 | 24.19 / 13.14 / 8.39 | 28.57 / 15.21 / 9.77 | 46.16 / 21.25 / 13.76 | 60.00 / 35.92 / 24.88 |
| PL10.JT | 50 | 12.22 / 6.45 / 2.52 | 21.19 / 9.51 / 5.72 | 31.55 / 13.43 / 8.18 | 50.00 / 25.01 / 16.04 |
| PL7.JT | 14 | 5.80 / 1.86 / 1.27 | 6.67 / 2.14 / 1.51 | 9.37 / 3.04 / 2.21 | 13.99 / 6.25 / 4.00 |
| R27ER.JT | 117 | 27.50 / 17.32 / 11.98 | 31.12 / 19.63 / 13.70 | 42.16 / 26.52 / 18.78 | 117.00 / 53.72 / 37.24 |
| R27ET.JT | 104 | 33.06 / 19.50 / 12.94 | 37.82 / 22.30 / 14.88 | 51.91 / 30.60 / 20.62 | 103.96 / 61.28 / 41.84 |
| R27T.JT | 63 | 27.16 / 15.45 / 10.08 | 31.26 / 17.76 / 11.66 | 43.46 / 24.63 / 16.28 | 62.99 / 40.84 / 28.02 |
| R530.JT | 40 | 9.82 / 4.34 / 2.56 | 11.21 / 5.01 / 3.01 | 15.59 / 7.04 / 4.39 | 40.00 / 15.43 / 9.91 |
| R550.JT | 15 | 11.33 / 4.75 / 2.87 | 13.20 / 5.55 / 3.37 | 14.03 / 6.85 / 4.02 | 15.00 / 7.88 / 5.00 |

## Complete A2A seeker-valid reach

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | 24.51 / 15.65 / 11.01 | 28.03 / 17.81 / 12.60 | 40.85 / 24.08 / 17.22 | 72.84 / 48.60 / 34.61 |
| AA11.JT | 11.56 / 15.30 / 11.79 | 11.56 / 15.30 / 12.85 | 11.56 / 15.30 / 15.21 | 11.56 / 15.30 / 18.28 |
| AA11B.JT | 11.56 / 15.30 / 11.79 | 11.56 / 15.30 / 12.85 | 11.56 / 15.30 / 15.21 | 11.56 / 15.30 / 18.28 |
| AA12.JT | 17.17 / 7.89 / 4.75 | 20.03 / 9.17 / 5.56 | 30.40 / 12.98 / 7.98 | 50.00 / 24.56 / 15.84 |
| AA2.JT | 0.00 / 0.00 / 4.85 | 0.00 / 0.00 / 5.61 | 0.00 / 0.00 / 7.28 | 0.00 / 0.00 / 9.14 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 5.78 / 2.02 / 1.35 | 5.78 / 2.38 / 1.60 | 5.78 / 3.49 / 2.34 | 5.78 / 6.48 / 4.04 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 35.77 / 22.66 / 14.46 | 41.95 / 25.64 / 17.11 | 56.43 / 34.42 / 23.41 | 150.00 / 69.85 / 46.83 |
| AEMP1.JT | 17.16 / 7.80 / 4.74 | 20.03 / 9.04 / 5.55 | 30.40 / 12.79 / 7.97 | 49.99 / 24.35 / 15.83 |
| AIM120.JT | 26.71 / 14.15 / 9.06 | 31.89 / 16.39 / 10.55 | 51.20 / 22.96 / 14.87 | 60.00 / 36.00 / 25.27 |
| AIM54C.JT | 11.74 / 6.60 / 2.89 | 41.60 / 25.89 / 17.43 | 54.54 / 34.17 / 23.50 | 150.00 / 69.50 / 44.88 |
| AIM7.JT | 38.12 / 15.89 / 9.63 | 46.44 / 24.23 / 16.25 | 59.31 / 33.93 / 22.43 | 69.95 / 46.72 / 34.81 |
| AIM7E.JT | 10.75 / 4.16 / 2.57 | 12.24 / 4.85 / 3.02 | 16.84 / 6.88 / 4.38 | 34.84 / 14.81 / 9.69 |
| AIM9B.JT | 0.00 / 0.00 / 5.14 | 0.00 / 0.00 / 5.95 | 0.00 / 0.00 / 7.33 | 0.00 / 0.00 / 9.65 |
| AIM9M.JT | 9.64 / 7.66 / 5.24 | 9.64 / 8.74 / 6.03 | 9.64 / 11.52 / 7.97 | 9.64 / 12.75 / 11.36 |
| AIM9X.JT | 9.64 / 7.68 / 5.24 | 9.64 / 8.74 / 6.03 | 9.64 / 11.58 / 8.00 | 9.64 / 12.75 / 11.43 |
| MICA.JT | 24.19 / 13.10 / 8.39 | 28.59 / 15.16 / 9.77 | 46.17 / 21.17 / 13.76 | 59.98 / 35.83 / 24.88 |
| PL10.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| PL7.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| R27ER.JT | 27.51 / 17.33 / 11.99 | 31.12 / 19.64 / 13.70 | 42.16 / 26.52 / 18.78 | 115.16 / 53.72 / 37.24 |
| R27ET.JT | 11.56 / 15.30 / 12.94 | 11.56 / 15.30 / 14.89 | 11.56 / 15.30 / 18.28 | 11.56 / 15.30 / 18.28 |
| R27T.JT | 11.56 / 15.30 / 10.09 | 11.56 / 15.30 / 11.66 | 11.56 / 15.30 / 16.28 | 11.56 / 15.30 / 18.28 |
| R530.JT | 9.82 / 4.34 / 2.56 | 11.22 / 5.01 / 3.02 | 15.59 / 7.04 / 4.39 | 39.99 / 15.43 / 9.91 |
| R550.JT | 9.64 / 4.72 / 2.88 | 9.64 / 5.52 / 3.37 | 9.64 / 6.81 / 4.02 | 9.64 / 7.84 / 5.00 |

## SAM and powered A-G calibration

SAM launcher is stationary at 10 m MSL. Calibration target is stationary at
clamp(0.2*anchor_range_m,300,3000) m, an agent-selected condition that keeps
short-range SAM benchmarks below their typical upper engagement heights.
The anchor measures missile reach to a stationary target. The additional
head/beam/tail cases use a 250 m/s target; their initial ranges differ because
of target motion. They are not asserted to be manufacturer launch envelopes.

A-G launcher is 300 m/s at 1500 m, target stationary at 10 m. Slow command
weapons use a helicopter reference of 30 m/s at 100 m. Ship strike uses
stationary launch at 10 m. These distances are horizontal stand-off, with
heights separately stated. The surface target represents a finite-height
object, not an impact point below terrain. Laser/command point predictions
use the existing 25 ft contact tolerance. Other synthetic targets use a 30 ft
sphere plus the decoded fuze, with the decoded arming delay. Aircraft launch
minimums, radar acquisition limits and weather are separate employment gates.

Four numbers in a SAM cell are head / beam / tail / stationary; one number
for A-G is stationary. N/A marks a held or separate designator/command
channel, not zero physical range. Laser/command live integration is tested
separately; no seeker-valid claim is made from the physical-only rows.

| Store | Anchor km | Launcher/target m | Physical km | Seeker-valid km |
| --- | ---: | --- | --- | --- |
| AGM45.JT | 40 | 1500/10 | 42.73 | 42.73 |
| AGM65A.JT | 27 | 1500/10 | 27.39 | 15.17 |
| AGM65G.JT | 32.18688 | 1500/10 | 33.81 | 18.23 |
| AGM84A.JT | 185 | 1500/10 | 189.79 | 189.79 |
| AGM84E.JT | 93 | 1500/10 | 173.70 | 91.41 |
| AGM88.JT | 48.28032 | 1500/10 | 49.20 | 49.20 |
| AM39.JT | 70 | 1500/10 | 78.77 | 78.77 |
| AS14.JT | 10 | 1500/10 | 33.73 | N/A |
| AS15.JT | 18.288 retail | 1500/10 | 39.04 | 39.04 |
| AS16.JT | 150 | 1500/10 | 76.24 | 76.24 |
| AS30.JT | 11 | 1500/10 | 34.61 | N/A |
| AS7.JT | 10 | 1500/10 | 22.70 | 22.70 |
| AT12.JT | 7.9248 retail | 1500/10 | 53.39 | N/A |
| AT2.JT | 4 | 100/10 | 9.10 | N/A |
| FIM92.JT | 4 | 10/800 | 12.40 / 6.08 / 3.39 / 7.90 | 12.41 / 6.08 / 3.39 / 7.90 |
| MIS.JT | 6.5 | 10/1300 | 11.44 / 4.96 / 2.56 / 7.01 | 11.44 / 4.97 / 2.56 / 7.02 |
| R440.JT | 10 | 10/2000 | 22.98 / 13.78 / 9.36 / 16.16 | 22.99 / 13.78 / 9.36 / 16.16 |
| SA13.JT | 5 | 10/1000 | 17.73 / 9.78 / 6.03 / 11.88 | 4.75 / 6.30 / 6.03 / 7.54 |
| SA14.JT | 4.5 | 10/900 | 11.77 / 5.68 / 3.12 / 7.45 | 3.79 / 5.02 / 3.12 / 6.02 |
| SA15.JT | 12 | 10/2400 | 15.91 / 7.70 / 4.38 / 10.16 | 15.92 / 7.70 / 4.38 / 10.16 |
| SA16.JT | 5.2 | 10/1040 | 11.43 / 5.24 / 2.79 / 7.11 | 2.77 / 3.68 / 2.80 / 4.44 |
| SA19.JT | 8 | 10/1600 | 9.40 / 3.67 / 1.66 / 5.61 | 9.41 / 3.67 / 1.67 / 5.61 |
| SA2A.JT | 29 | 10/3000 | 27.32 / 14.55 / 9.24 / 18.21 | 27.33 / 14.55 / 9.24 / 18.21 |
| SA3.JT | 25 | 10/3000 | 25.57 / 11.78 / 7.13 / 16.33 | 25.58 / 11.78 / 7.13 / 16.33 |
| SA6.JT | 24 | 10/3000 | 23.38 / 11.34 / 6.84 / 15.06 | 23.39 / 11.34 / 6.84 / 15.06 |
| SA7.JT | 4.2 | 10/840 | 10.72 / 5.04 / 2.70 / 6.71 | 2.82 / 3.73 / 2.70 / 4.48 |
| SA9.JT | 4.2 | 10/840 | 12.22 / 5.96 / 3.31 / 7.77 | 3.80 / 5.03 / 3.31 / 6.03 |
| SAN11.JT | 8 | 10/1600 | 9.40 / 3.67 / 1.66 / 5.61 | 9.41 / 3.67 / 1.67 / 5.61 |
| SAN3.JT | 30 | 10/3000 | 27.34 / 15.45 / 10.09 / 18.64 | 27.34 / 15.46 / 10.09 / 18.64 |
| SAN4.JT | 10 | 10/2000 | 14.13 / 6.39 / 3.47 / 8.81 | 14.14 / 6.39 / 3.47 / 8.82 |
| SAN7.JT | 25 | 10/3000 | 23.50 / 12.40 / 7.73 / 15.54 | 23.50 / 12.40 / 7.73 / 15.55 |
| SAN9.JT | 12 | 10/2400 | 15.91 / 7.70 / 4.38 / 10.16 | 15.92 / 7.70 / 4.38 / 10.16 |
| SEA_SPAR.JT | 26 | 10/3000 | 28.25 / 13.37 / 8.01 / 18.10 | 28.25 / 13.37 / 8.01 / 18.11 |
| SSN9.JT | 150 | 10/10 | 153.63 | 153.63 |

AS15 and AT12 retain runtime decoded mass/range and the uniform class diameter
because their real-world identity is unresolved. Their historical fallback
drag fit is retained; class budgets now change reach. This is game calibration.
The SAM aspect allowance continues the previous game policy where the
original aspect field has no reviewed interpretation. In particular, this
is not a claim that every real infrared SAM is all-aspect.

## One-anchor calibration and class floors

All 25 A2A records use fixed class budgets. No timer was fitted. The table
reports 35k head reach and its residual against the unchanged numerical anchor.
Fuel/drag are bounded fits; the detailed scoring and explicit class allocations
are in [the data](../formats/missile-energy-data.md#per-store-motors-and-class-control-budgets).
The sourced AIM9B motor/fuel pair and 20-second time are retained. M/X share
the retained thrust/impulse data. Their overshoot is accepted, not removed
by shortening control time.

Every selected 5k tail clears its class floor and 1.2 times the decoded
minimum. AA8 needs about 14.05 km high head to keep at least 1.5 km low tail;
the feasibility study's roughly 13 km candidate had only 0.87 km tail and
was therefore ineligible. AIM9B's 1.74% shortfall preserves modern IR tail
ordering without changing its sourced motor or budget.

| Store | Class | Fuel | Drag | Control s | Anchor km | H35 km | Residual | T5 km | Class floor km | 1.2 x decoded min km |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| AA10.JT | MediumAir | 0.401382 | 1.5 | 70 | 73 | 73.000 | +0.00% | 12.602 | 3 | 4.389 |
| AA11.JT | ShortIr | 0.500000 | 1.1 | 20 | 30 | 29.749 | -0.84% | 12.848 | 1.5 | 0.732 |
| AA11B.JT | ShortIr | 0.500000 | 1.1 | 20 | 30 | 29.749 | -0.84% | 12.848 | 1.5 | 0.549 |
| AA12.JT | MediumAir | 0.151102 | 1 | 70 | 50 | 50.000 | +0.00% | 5.558 | 3 | 4.389 |
| AA2.JT | ShortIr | 0.263265 | 2.25 | 20 | 20 | 19.994 | -0.03% | 5.603 | 1.5 | 1.463 |
| AA6.JT | MediumAir | 0.352050 | 3 | 70 | 30 | 30.062 | +0.21% | 4.398 | 3 | 4.389 |
| AA8.JT | ShortIr | 0.120000 | 1.75 | 20 | 10 | 14.045 | +40.45% | 1.596 | 1.5 | 1.463 |
| AA9.JT | LongAir | 0.379835 | 1 | 180 | 120 | 120.000 | +0.00% | 13.244 | 5 | 4.389 |
| AAML.JT | LongAir | 0.358567 | 0.75 | 180 | 150 | 150.000 | +0.00% | 17.109 | 5 | 10.973 |
| AEMP1.JT | MediumAir | 0.151195 | 1 | 70 | 50 | 50.000 | +0.00% | 5.547 | 3 | 0.732 |
| AIM120.JT | MediumAir | 0.220219 | 1 | 70 | 60 | 60.000 | +0.00% | 10.555 | 3 | 4.389 |
| AIM54C.JT | LongAir | 0.404907 | 0.75 | 180 | 150 | 150.000 | +0.00% | 17.425 | 5 | 10.973 |
| AIM7.JT | MediumAir | 0.292327 | 1 | 70 | 70 | 70.000 | +0.00% | 16.254 | 3 | 2.926 |
| AIM7E.JT | MediumAir | 0.156298 | 2 | 70 | 35 | 34.994 | -0.02% | 3.022 | 3 | 1.097 |
| AIM9B.JT | ShortIr | 0.243786 | 1.75 | 20 | 20 | 19.652 | -1.74% | 5.947 | 1.5 | 1.463 |
| AIM9M.JT | ShortIr | 0.337835 | 3 | 20 | 18 | 21.363 | +18.68% | 6.028 | 1.5 | 1.463 |
| AIM9X.JT | ShortIr | 0.341432 | 3 | 20 | 18 | 21.429 | +19.05% | 6.028 | 1.5 | 1.097 |
| MICA.JT | MediumAir | 0.229913 | 1 | 70 | 60 | 60.000 | +0.00% | 9.765 | 3 | 4.389 |
| PL10.JT | MediumAir | 0.157434 | 1 | 70 | 50 | 50.000 | +0.00% | 5.723 | 3 | 2.926 |
| PL7.JT | ShortIr | 0.122969 | 2.25 | 20 | 14 | 13.995 | -0.04% | 1.506 | 1.5 | 1.463 |
| R27ER.JT | LongAir | 0.394911 | 1.5 | 180 | 117 | 117.000 | +0.00% | 13.700 | 5 | 4.389 |
| R27ET.JT | LongAir | 0.293510 | 1 | 180 | 104 | 103.962 | -0.04% | 14.882 | 5 | 4.389 |
| R27T.JT | MediumAir | 0.255217 | 1 | 70 | 63 | 62.990 | -0.02% | 11.657 | 3 | 4.389 |
| R530.JT | MediumAir | 0.179955 | 2 | 70 | 40 | 40.004 | +0.01% | 3.013 | 3 | 2.926 |
| R550.JT | ShortIr | 0.122482 | 1 | 20 | 15 | 14.996 | -0.03% | 3.373 | 1.5 | 1.463 |

## Family, ratio and altitude checks

Family margins are the smallest relative advantage over all twelve
500/5k/15k/35k head/beam/tail cells for Sparrow and both R-27 pairs.
IR generation margins cover the three 5k/15k/35k tail cells. These measured
constraints do not assert universal cross-family generation ordering.

| Newer / older | Minimum advantage |
| --- | ---: |
| AIM7.JT/AIM7E.JT | 99.99% |
| R27ER.JT/AA10.JT | 3.16% |
| R27ET.JT/R27T.JT | 19.46% |
| AIM9M.JT/AIM9B.JT | 1.35% |
| AIM9M.JT/AA2.JT | 7.58% |
| AIM9M.JT/AA8.JT | 180.74% |
| AIM9X.JT/AIM9B.JT | 1.38% |
| AIM9X.JT/AA2.JT | 7.62% |
| AIM9X.JT/AA8.JT | 182.54% |
| AA11.JT/AIM9B.JT | 104.59% |
| AA11.JT/AA2.JT | 97.50% |
| AA11.JT/AA8.JT | 387.93% |

Every selected H5/H35 is within 0.20..0.90 and every T5/H35 within 0.05..0.60.
No altitude reversal exceeds 10%, and no physical sweep cell has zero reach.
Three 500-foot employment intervals are empty after the decoded minimum is
applied: AIM54 beam 6.60 km and tail 2.89 km versus minimum 9.144 km, and
AA9 tail 2.77 km versus 3.6576 km. These are investigated deck cases, not
a claim that every physical maximum is a permitted launch. The same heavy
stores already had deck-tail reach below their decoded minimum in the
previous baseline. Their 5k tails exceed the required minimum margins.
No launch gate was relaxed. At 10% beyond these deck edges, the diagnostic
shots strike flat ground after 5.725..6.325 seconds at Mach 1.62..1.79,
during boost, with neither control expiry nor post-burn defeat. This is the
retained drop/PN trajectory's ground-contact limitation. Increasing its
180-second budget would not prevent a ground collision six seconds after
release. No new A2A ground-avoidance rule was introduced.

The existing 12 km deck-tail escape and modern radar high-head ordering checks
remain. Perturbation checks change fuel/derived thrust, drag, control budget
and burn duration by 2% (constant impulse for burn duration), using high-head
and 5k/35k tail conditions. They are sensitivity diagnostics, not uncertainty
bounds.

| Store | Tail 5k km | Tail 15k km | Tail 35k km | Largest perturbation change |
| --- | ---: | ---: | ---: | ---: |
| AIM120.JT | 10.555 | 14.871 | 25.268 | 2.53% |
| AIM9M.JT | 6.028 | 7.975 | 11.359 | 1.96% |
| AIM9X.JT | 6.030 | 7.997 | 11.432 | 1.95% |
| AIM54C.JT | 17.426 | 23.498 | 44.875 | 2.07% |
| AA10.JT | 12.605 | 17.220 | 34.614 | 1.90% |
| R27ER.JT | 13.701 | 18.779 | 37.244 | 2.16% |
| R27T.JT | 11.660 | 16.283 | 28.022 | 2.22% |
| R27ET.JT | 14.885 | 20.621 | 41.847 | 2.05% |
| AA11.JT | 12.849 | 15.205 | 19.742 | 1.87% |
| AA12.JT | 5.558 | 7.980 | 15.840 | 2.74% |
| AA9.JT | 13.244 | 18.035 | 35.147 | 2.37% |
| AIM9B.JT | 5.948 | 7.333 | 9.650 | 2.15% |
| AA2.JT | 5.604 | 7.281 | 9.996 | 2.08% |
| AA8.JT | 1.596 | 2.337 | 4.046 | 2.87% |
| R550.JT | 3.373 | 4.023 | 4.998 | 2.79% |
| AA11B.JT | 12.849 | 15.205 | 19.742 | 1.87% |
| AIM7E.JT | 3.022 | 4.385 | 9.685 | 2.85% |
| AIM7.JT | 16.255 | 22.427 | 34.817 | 2.09% |
| R530.JT | 3.014 | 4.395 | 9.912 | 2.75% |
| AAML.JT | 17.112 | 23.408 | 46.826 | 1.94% |
| MICA.JT | 9.768 | 13.759 | 24.883 | 2.49% |
| PL7.JT | 1.506 | 2.207 | 3.996 | 2.88% |
| PL10.JT | 5.725 | 8.185 | 16.035 | 2.74% |
| AEMP1.JT | 5.547 | 7.971 | 15.835 | 2.75% |
| AA6.JT | 4.399 | 6.084 | 12.502 | 1.97% |

## SAM and A-G before/after

Motor/fuel/drag are unchanged for these roles. Class control budgets and the
post-burn air-target policy change reach. SAM rows below use their stationary
calibration targets; moving head/beam/tail results are in the full table above.
Ground seeker/support gates remain separate from these ideal physical values.

| Store | Previous km | Phase 2.5 km | Change |
| --- | ---: | ---: | ---: |
| AGM65G.JT | 32.186 | 33.811 | +5.0% |
| AGM88.JT | 48.280 | 49.199 | +1.9% |
| AM39.JT | 69.982 | 78.770 | +12.6% |
| SA7.JT | 4.208 | 6.713 | +59.5% |
| SA14.JT | 4.511 | 7.445 | +65.0% |
| SA16.JT | 5.211 | 7.112 | +36.5% |
| SA15.JT | 12.003 | 10.158 | -15.4% |
| SA6.JT | 23.995 | 15.065 | -37.2% |
| SA9.JT | 4.210 | 7.768 | +84.5% |
| MIS.JT | 6.507 | 7.014 | +7.8% |
| SEA_SPAR.JT | 26.000 | 18.100 | -30.4% |
| FIM92.JT | 4.010 | 7.898 | +97.0% |
| R440.JT | 10.014 | 16.161 | +61.4% |
| SA2A.JT | 29.003 | 18.214 | -37.2% |
| SA3.JT | 25.002 | 16.332 | -34.7% |
| SA13.JT | 5.014 | 11.881 | +137.0% |
| SA19.JT | 8.002 | 5.607 | -29.9% |
| SAN3.JT | 29.991 | 18.645 | -37.8% |
| SAN4.JT | 10.006 | 8.814 | -11.9% |
| SAN7.JT | 25.002 | 15.540 | -37.8% |
| SAN9.JT | 12.003 | 10.158 | -15.4% |
| SAN11.JT | 8.002 | 5.607 | -29.9% |
| SSN9.JT | 149.954 | 153.634 | +2.5% |
| AGM45.JT | 39.998 | 42.735 | +6.8% |
| AGM65A.JT | 26.998 | 27.387 | +1.4% |
| AGM84A.JT | 184.953 | 189.788 | +2.6% |
| AGM84E.JT | 93.010 | 173.696 | +86.7% |
| AS7.JT | 10.006 | 22.702 | +126.9% |
| AS16.JT | 149.954 | 76.236 | -49.2% |
| AS14.JT | 10.010 | 33.726 | +236.9% |
| AS30.JT | 11.011 | 34.610 | +214.3% |
| AT2.JT | 4.008 | 9.102 | +127.1% |
| AS15.JT | 18.296 | 39.043 | +113.4% |
| AT12.JT | 7.943 | 53.390 | +572.2% |

AS16's fast-ground 90-second budget no longer fits its old 150 km proxy anchor.
AT12 retains the game's cruise-path class and its 600-second budget; its real
identity remains unknown. These are explicit class consequences, not new
real-world performance claims. No A-G/SAM motor was refitted to hide them.

## Defeat and force diagnostics

The offline --diagnostics mode records available/required G, positive G
deficit, closing speed, Mach, control expiry and post-burn defeat at each
measured edge and 10% beyond it. These values do not create a G-based death
rule. Raw commanded G near contact can exceed available G; applied steering
is still limited by the shared kernel. The behaviour tripwire now includes
the low-Mach countdown, reset, defeat transition and inert contacts.

## Validation status

Linux x86-64, model identity 11178521366749080852 (9b2215f4eb316d14),
behaviour trace 14647273043871393956. All 7,212 Rust tests pass, with 106
intentionally ignored: the complete workspace run had only the expected old
golden pair failure, then the reviewed new pair passed its targeted rerun.
Workspace build, clippy with warnings denied, formatting, documentation,
507 Python tests (five skipped), and source/app/extractor asset checks pass.
All 25 A2A range regressions pass, including the retained 12 km deck escape,
class floors, decoded minimum margins, family ordering and 2% perturbations.
All 391 physical/seeker rows and 668 edge/beyond-edge diagnostics were regenerated.

All 36 battery scenarios pass: the previous 35 plus the new post-burn defeat
scenario. SA2A, SA3 and SA6 each hit vulnerable moving jets at 10k and 20k feet.
The new 32-shot case records 22 inert defeats and six hits, with no hit after
defeat and byte-identical serial/four-worker rows. The existing 240-shot
Energy comparison and eight historical Spec shots also pass. The first battery
attempt stopped on the new checker's callback signature; the checker and its
runner-interface test were corrected, the 507 Python tests reran successfully,
and the entire 36-scenario battery was rerun successfully.

Windows/macOS were not run on this Linux host. Retail comparison remains
unavailable. Phase 3 stays stopped; its earlier pre-fix study is still invalid.


## Prediction cost

Linux release examples, imported profile, same final model as the tables above.
Every heavy command checked available memory first and used nice 15,
CARGO_BUILD_JOBS=4 and four Rust test threads. Battery concurrency was two jobs
and one isolated window through tools/agent-run.sh. Heavy runs were serial.

Natural UKR/BAL/SPA/GRE fights run 14,400 ticks with eight AI aircraft.
The large fight runs 7,200 ticks with 23 AI aircraft. Turning-player and hold
lanes run 2,400 ticks; hold lanes request eight stations per aircraft.
Cold means the first 240 ticks. Steady means the remaining moving or frozen
ticks. Wall timings include scheduling delays; CPU timings measure prediction
work. These are host measurements, not a cross-platform performance guarantee.

| Lane | Cold p50 / p95 ms | Steady p50 / p95 ms | Largest CPU p95 ms | Refused / starved | Guided launches |
| --- | ---: | ---: | ---: | ---: | ---: |
| natural-ukr | 0.655 / 0.722 | 0.030 / 0.714 | 0.719 | 0 / 0 | 13 |
| natural-bal | 0.655 / 0.721 | 0.030 / 0.685 | 0.720 | 0 / 0 | 13 |
| natural-spa | 0.665 / 0.726 | 0.028 / 0.692 | 0.725 | 0 / 0 | 13 |
| natural-gre | 0.667 / 0.726 | 0.028 / 0.697 | 0.724 | 0 / 0 | 13 |
| natural-big | 0.734 / 0.769 | 0.708 / 0.767 | 0.766 | 0 / 0 | 26 |
| turning-3 | 0.760 / 0.797 | 0.761 / 0.793 | 0.797 | 0 / 0 | 14 |
| turning-10 | 0.749 / 0.785 | 0.759 / 0.790 | 0.789 | 0 / 0 | 12 |
| hold-frozen | 0.815 / 0.877 | 0.191 / 0.873 | 0.876 | 0 / 0 | 36 |
| hold-moving | 0.809 / 0.832 | 0.737 / 0.838 | 0.836 | 0 / 0 | 36 |
| hold-cruise | 0.678 / 0.721 | 0.686 / 0.727 | 0.727 | 0 / 0 | 16 |

All cold/steady p95 values are below 2 ms; the largest is 0.877418 ms wall
and 0.876166 ms CPU. The largest individual wall sample is 1.075107 ms.
No lane reports admission refusal, starvation or admission waiting ticks.
Cached-reader p95 is at most 0.014629 ms.

At 3 degrees/s the player has qualified advice on 1,570/1,570 observed ticks
(1,321 positive and 249 negative). At 10 degrees/s it is 660/661, with one
pending tick. The denominator is the ticks with an eligible observation,
not all 2,400 simulated ticks. Strict AI qualification remains separate
from turn-projected player advice.

Frozen and moving A2A hold lanes record 149,840 and 20,805 successful cached
reads. The 20-second cruise hold still records zero qualified cached reads.
It demonstrates bounded cost, not completed-cruise-answer availability.
This existing limitation is explicit; no synchronous fallback or relaxed
support gate was introduced. Ground-target live reach and support behaviour
are covered separately by the A-G battery cases and range tables.
