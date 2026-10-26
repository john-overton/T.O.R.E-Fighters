# Missile energy calibration and validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-25. Model calibration, not retail
parity or a validated engineering flight envelope. Kernel/data version 1,
branch `missile-overhaul`, Linux x86-64. Input came from the isolated imported
profile for this task; no media bytes are committed. The shared offline probe
is `cargo run --locked --release -p tore-sim --example missile_energy_ranges
-- LOCAL_JT_DIRECTORY`. `--fit` searches the drag coefficient and prints a
candidate table; it never changes source or runs during gameplay.

This baseline contains the Phase 2 review corrections. The earlier Phase 3
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
continuous no-escape envelope.

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
[the data evidence](../formats/missile-energy-data.md#per-store-motors-and-paired-anchors).
In particular, the revised IR high-head anchors are agent choices, not new
manufacturer claims. Physical head-on reach does not give a rear-only seeker
a head-on lock. Held records remain unavailable in gameplay.

## Complete A2A physical reach

Each cell is head / beam / tail, km. All 25 inventory records are included,
including held records measured diagnostically without enabling them.

| Store | Primary reference km | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | ---: | --- | --- | --- | --- | --- |
| AA10.JT | 73 | 29.11 / 5.61 / 3.28 | 31.75 / 6.41 / 3.81 | 49.34 / 8.78 / 5.41 | 73.00 / 17.86 / 11.56 |
| AA11.JT | 35 | 14.70 / 4.44 / 2.89 | 16.36 / 5.02 / 3.32 | 20.94 / 6.67 / 4.56 | 34.99 / 12.82 / 9.17 |
| AA11B.JT | 35 | 14.70 / 4.44 / 2.89 | 16.36 / 5.02 / 3.32 | 20.94 / 6.67 / 4.56 | 34.99 / 12.82 / 9.17 |
| AA12.JT | 50 | 19.54 / 3.23 / 1.83 | 21.45 / 3.73 / 2.16 | 34.43 / 5.09 / 3.13 | 50.00 / 10.52 / 7.02 |
| AA2.JT | 20 | 8.11 / 2.12 / 1.44 | 9.07 / 2.40 / 1.65 | 11.70 / 3.21 / 2.27 | 19.99 / 6.21 / 4.64 |
| AA6.JT | 30 | 12.56 / 4.66 / 3.20 | 13.93 / 5.19 / 3.61 | 17.71 / 6.74 / 4.81 | 29.99 / 12.52 / 9.31 |
| AA8.JT | 10 | 4.42 / 0.88 / 0.43 | 4.64 / 0.91 / 0.46 | 5.71 / 1.41 / 0.82 | 9.63 / 3.17 / 2.12 |
| AA9.JT | 120 | 12.48 / 6.32 / 2.23 | 50.85 / 12.37 / 6.87 | 84.80 / 16.96 / 9.81 | 120.00 / 34.04 / 21.02 |
| AAML.JT | 150 | 37.91 / 15.15 / 8.83 | 63.18 / 17.36 / 10.31 | 104.24 / 24.11 / 14.87 | 150.00 / 47.25 / 30.51 |
| AEMP1.JT | 50 | 19.53 / 3.19 / 1.81 | 21.43 / 3.68 / 2.15 | 34.43 / 5.03 / 3.11 | 50.00 / 10.42 / 7.00 |
| AIM120.JT | 60 | 23.84 / 4.30 / 2.50 | 26.04 / 4.90 / 2.91 | 41.08 / 6.70 / 4.16 | 60.00 / 13.71 / 9.09 |
| AIM54C.JT | 150 | 12.42 / 6.38 / 2.32 | 62.82 / 16.71 / 9.55 | 106.68 / 22.83 / 13.51 | 150.00 / 44.58 / 27.79 |
| AIM7.JT | 70 | 12.08 / 5.57 / 2.53 | 30.85 / 6.38 / 3.84 | 49.24 / 8.76 / 5.43 | 70.00 / 17.59 / 11.46 |
| AIM7E.JT | 35 | 15.20 / 4.71 / 2.85 | 16.76 / 5.42 / 3.39 | 21.11 / 7.41 / 4.89 | 33.77 / 13.65 / 9.72 |
| AIM9B.JT | 20 | 9.57 / 0.00 / 1.27 | 10.71 / 0.00 / 1.47 | 13.75 / 2.96 / 2.06 | 22.78 / 5.83 / 4.31 |
| AIM9M.JT | 35 | 15.29 / 4.16 / 2.72 | 16.97 / 4.72 / 3.15 | 21.43 / 6.34 / 4.35 | 34.99 / 12.25 / 8.89 |
| AIM9X.JT | 35 | 15.25 / 4.17 / 2.74 | 16.92 / 4.74 / 3.16 | 21.41 / 6.35 / 4.37 | 34.99 / 12.26 / 8.92 |
| MICA.JT | 60 | 24.45 / 4.01 / 2.27 | 26.69 / 4.58 / 2.65 | 41.12 / 6.24 / 3.78 | 60.00 / 12.77 / 8.27 |
| PL10.JT | 50 | 12.42 / 0.00 / 1.70 | 22.29 / 2.90 / 2.00 | 35.66 / 4.54 / 2.88 | 50.00 / 9.65 / 6.36 |
| PL7.JT | 14 | 5.90 / 1.44 / 0.78 | 6.48 / 1.69 / 0.96 | 8.19 / 2.38 / 1.47 | 14.00 / 4.67 / 3.26 |
| R27ER.JT | 117 | 36.68 / 12.14 / 7.46 | 49.75 / 13.97 / 8.71 | 81.24 / 19.16 / 12.24 | 117.00 / 37.26 / 24.66 |
| R27ET.JT | 104 | 33.17 / 18.16 / 12.89 | 48.79 / 20.41 / 14.62 | 60.62 / 26.64 / 19.37 | 96.70 / 49.63 / 36.86 |
| R27T.JT | 63 | 26.71 / 10.53 / 7.38 | 29.56 / 11.79 / 8.36 | 37.25 / 15.39 / 11.14 | 60.66 / 28.50 / 21.18 |
| R530.JT | 40 | 15.68 / 2.30 / 1.25 | 17.12 / 2.62 / 1.49 | 27.89 / 3.73 / 2.24 | 40.00 / 7.67 / 5.17 |
| R550.JT | 15 | 6.27 / 1.61 / 0.89 | 6.90 / 1.87 / 1.07 | 8.77 / 2.61 / 1.63 | 15.00 / 5.05 / 3.53 |

## Complete A2A seeker-valid reach

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | 29.11 / 5.61 / 3.28 | 31.75 / 6.41 / 3.81 | 49.23 / 8.78 / 5.41 | 71.98 / 17.86 / 11.56 |
| AA11.JT | 11.56 / 4.43 / 2.89 | 11.56 / 5.01 / 3.32 | 11.56 / 6.66 / 4.56 | 11.56 / 12.81 / 9.17 |
| AA11B.JT | 11.56 / 4.43 / 2.89 | 11.56 / 5.01 / 3.32 | 11.56 / 6.66 / 4.56 | 11.56 / 12.81 / 9.17 |
| AA12.JT | 19.54 / 3.17 / 1.83 | 21.45 / 3.69 / 2.16 | 34.43 / 5.07 / 3.13 | 50.00 / 10.50 / 7.02 |
| AA2.JT | 0.00 / 0.00 / 1.44 | 0.00 / 0.00 / 1.65 | 0.00 / 0.00 / 2.27 | 0.00 / 0.00 / 4.64 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 4.42 / 0.00 / 0.43 | 4.64 / 0.00 / 0.46 | 5.71 / 1.29 / 0.82 | 5.78 / 3.15 / 2.12 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 37.91 / 15.13 / 8.83 | 63.18 / 17.33 / 10.31 | 104.24 / 24.05 / 14.88 | 150.00 / 47.15 / 30.51 |
| AEMP1.JT | 19.53 / 3.13 / 1.81 | 21.43 / 3.64 / 2.15 | 34.43 / 5.01 / 3.11 | 50.00 / 10.21 / 7.00 |
| AIM120.JT | 23.84 / 4.26 / 2.50 | 26.04 / 4.87 / 2.91 | 41.08 / 6.68 / 4.16 | 60.00 / 13.62 / 9.09 |
| AIM54C.JT | 12.42 / 6.38 / 2.32 | 62.82 / 16.71 / 9.55 | 106.68 / 22.79 / 13.51 | 150.00 / 44.49 / 27.80 |
| AIM7.JT | 12.08 / 5.57 / 2.53 | 30.85 / 6.38 / 3.84 | 49.14 / 8.76 / 5.43 | 69.14 / 17.59 / 11.46 |
| AIM7E.JT | 15.20 / 4.71 / 2.85 | 16.76 / 5.42 / 3.39 | 21.11 / 7.41 / 4.89 | 33.61 / 13.63 / 9.72 |
| AIM9B.JT | 0.00 / 0.00 / 1.27 | 0.00 / 0.00 / 1.47 | 0.00 / 0.00 / 2.06 | 0.00 / 0.00 / 4.31 |
| AIM9M.JT | 9.64 / 4.14 / 2.72 | 9.64 / 4.71 / 3.15 | 9.64 / 6.32 / 4.35 | 9.64 / 12.23 / 8.89 |
| AIM9X.JT | 9.64 / 4.15 / 2.74 | 9.64 / 4.72 / 3.16 | 9.64 / 6.34 / 4.37 | 9.64 / 12.26 / 8.92 |
| MICA.JT | 24.45 / 3.96 / 2.27 | 26.69 / 4.54 / 2.65 | 41.12 / 6.21 / 3.78 | 60.00 / 12.69 / 8.28 |
| PL10.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| PL7.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| R27ER.JT | 36.68 / 12.14 / 7.46 | 49.75 / 13.97 / 8.71 | 80.96 / 19.16 / 12.24 | 115.74 / 37.26 / 24.66 |
| R27ET.JT | 11.56 / 15.30 / 12.89 | 11.56 / 15.30 / 14.62 | 11.56 / 15.30 / 18.28 | 11.56 / 15.30 / 18.28 |
| R27T.JT | 11.56 / 10.51 / 7.38 | 11.56 / 11.78 / 8.36 | 11.56 / 15.30 / 11.14 | 11.56 / 15.30 / 18.28 |
| R530.JT | 15.68 / 2.30 / 1.25 | 17.12 / 2.62 / 1.49 | 27.89 / 3.73 / 2.24 | 39.98 / 7.67 / 5.17 |
| R550.JT | 6.26 / 1.55 / 0.89 | 6.89 / 1.83 / 1.07 | 8.77 / 2.58 / 1.63 | 9.64 / 5.03 / 3.53 |

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
| --- | ---: | --- | --- | --- | --- | --- |
| AGM45.JT | 40 | 1500/10 | 41.77 | 41.77 |
| AGM65A.JT | 27 | 1500/10 | 27.00 | 15.17 |
| AGM65G.JT | 32.18688 | 1500/10 | 32.19 | 18.23 |
| AGM84A.JT | 185 | 1500/10 | 184.95 | 184.95 |
| AGM84E.JT | 93 | 1500/10 | 92.98 | 91.41 |
| AGM88.JT | 48.28032 | 1500/10 | 50.40 | 50.40 |
| AM39.JT | 70 | 1500/10 | 69.98 | 69.98 |
| AS14.JT | 10 | 1500/10 | 10.00 | N/A |
| AS15.JT | 18.288 retail | 1500/10 | 18.28 | 18.28 |
| AS16.JT | 150 | 1500/10 | 149.95 | 149.95 |
| AS30.JT | 11 | 1500/10 | 11.00 | N/A |
| AS7.JT | 10 | 1500/10 | 10.00 | 10.00 |
| AT12.JT | 7.9248 retail | 1500/10 | 8.21 | N/A |
| AT2.JT | 4 | 100/10 | 4.00 | N/A |
| FIM92.JT | 4 | 10/800 | 8.05 / 0.00 / 0.00 / 4.00 | 8.05 / 0.00 / 0.00 / 4.00 |
| MIS.JT | 6.5 | 10/1300 | 11.70 / 3.30 / 1.65 / 6.50 | 11.70 / 3.30 / 1.65 / 6.50 |
| R440.JT | 10 | 10/2000 | 16.93 / 5.06 / 3.15 / 10.00 | 16.93 / 5.06 / 3.15 / 10.00 |
| SA13.JT | 5 | 10/1000 | 9.42 / 1.93 / 0.75 / 5.00 | 4.75 / 1.92 / 0.75 / 5.00 |
| SA14.JT | 4.5 | 10/900 | 8.71 / 1.39 / 0.39 / 4.50 | 3.79 / 1.38 / 0.39 / 4.50 |
| SA15.JT | 12 | 10/2400 | 19.60 / 7.30 / 4.58 / 12.00 | 19.60 / 7.30 / 4.58 / 12.00 |
| SA16.JT | 5.2 | 10/1040 | 9.76 / 2.13 / 0.86 / 5.20 | 2.77 / 2.13 / 0.86 / 4.44 |
| SA19.JT | 8 | 10/1600 | 13.96 / 4.43 / 2.42 / 8.00 | 13.96 / 4.43 / 2.42 / 8.00 |
| SA2A.JT | 29 | 10/3000 | 46.87 / 18.21 / 12.61 / 28.99 | 46.85 / 18.21 / 12.61 / 28.99 |
| SA3.JT | 25 | 10/3000 | 39.67 / 15.86 / 11.02 / 24.99 | 39.54 / 15.86 / 11.02 / 24.99 |
| SA6.JT | 24 | 10/3000 | 38.24 / 15.10 / 10.42 / 24.00 | 37.93 / 15.10 / 10.42 / 24.00 |
| SA7.JT | 4.2 | 10/840 | 8.34 / 0.80 / 0.12 / 4.20 | 2.82 / 0.79 / 0.12 / 4.20 |
| SA9.JT | 4.2 | 10/840 | 8.33 / 0.74 / 0.11 / 4.20 | 3.80 / 0.73 / 0.11 / 4.20 |
| SAN11.JT | 8 | 10/1600 | 13.96 / 4.43 / 2.42 / 8.00 | 13.96 / 4.43 / 2.42 / 8.00 |
| SAN3.JT | 30 | 10/3000 | 48.84 / 18.71 / 12.94 / 29.99 | 49.37 / 18.71 / 12.94 / 29.99 |
| SAN4.JT | 10 | 10/2000 | 16.78 / 5.84 / 3.50 / 10.00 | 16.78 / 5.84 / 3.50 / 10.00 |
| SAN7.JT | 25 | 10/3000 | 40.14 / 15.63 / 10.77 / 24.99 | 40.05 / 15.63 / 10.77 / 24.99 |
| SAN9.JT | 12 | 10/2400 | 19.60 / 7.30 / 4.58 / 12.00 | 19.60 / 7.30 / 4.58 / 12.00 |
| SEA_SPAR.JT | 26 | 10/3000 | 41.58 / 16.40 / 11.35 / 26.00 | 41.55 / 16.40 / 11.35 / 26.00 |
| SSN9.JT | 150 | 10/10 | 145.91 | 145.91 |

AS15 and AT12 retain runtime decoded mass/range and the uniform class diameter
because their real-world identity is unresolved. Their data-only fallback
drag fit meets that retail range anchor; this is not real-world calibration.
The SAM aspect allowance continues the previous game policy where the
original aspect field has no reviewed interpretation. In particular, this
is not a claim that every real infrared SAM is all-aspect.

## Paired calibration results

Each condition is altitude ft / aspect / target speed m/s / target km.
Launcher 300 m/s for A2A/strike, 30 m/s for AT2, zero for SAM/ship launch.
SAM altitude is target height; strike altitude is launcher height. Surface
launch remains at 10 m. All conditions and the 15% tolerance are agent-fitted
unless the data reference explicitly says otherwise. Largest residuals are
AIM9B, about 14% high-head and 11% low-tail. Every row is within 15% of both
conditions. This measures the requested fit, not independent real-world
validation. Drag exceptions are listed in the data document.

| Store | Primary condition | Measured km | Secondary condition | Measured km | Drag scale |
| --- | --- | ---: | --- | ---: | ---: |
| AIM120.JT | 35000 / head / 250 / 60 | 60.000 | 5000 / head / 250 / 27 | 26.038 | 3.22817 |
| AIM9M.JT | 35000 / head / 250 / 35 | 35.002 | 5000 / tail / 250 / 3.2 | 3.145 | 6.06570 |
| AIM9X.JT | 35000 / head / 250 / 35 | 35.002 | 5000 / tail / 250 / 3.2 | 3.157 | 6.04686 |
| AIM54C.JT | 35000 / head / 250 / 150 | 150.000 | 5000 / head / 250 / 67.5 | 62.830 | 0.69623 |
| AA10.JT | 35000 / head / 250 / 73 | 73.000 | 5000 / head / 250 / 32.85 | 31.760 | 2.41886 |
| R27ER.JT | 35000 / head / 250 / 117 | 117.000 | 5000 / head / 250 / 52.65 | 49.751 | 1.59612 |
| R27T.JT | 35000 / head / 250 / 63 | 60.659 | 5000 / head / 250 / 28.35 | 29.564 | 2.82012 |
| R27ET.JT | 35000 / head / 250 / 104 | 96.706 | 5000 / head / 250 / 46.8 | 48.798 | 1.72533 |
| AA11.JT | 35000 / head / 250 / 35 | 35.002 | 5000 / tail / 250 / 3.4 | 3.322 | 4.40229 |
| AA12.JT | 35000 / head / 250 / 50 | 50.000 | 5000 / head / 250 / 22.5 | 21.452 | 3.68315 |
| AA9.JT | 35000 / head / 250 / 120 | 120.000 | 5000 / head / 250 / 54 | 50.862 | 0.99363 |
| AGM65G.JT | 4921 / head / 0 / 32.1869 | 32.186 | 2460 / head / 0 / 31.5431 | 31.748 | 0.69513 |
| AGM88.JT | 4921 / head / 0 / 48.2803 | 50.402 | 2460 / head / 0 / 47.3147 | 47.168 | 0.99148 |
| AM39.JT | 4921 / head / 0 / 70 | 69.998 | 2460 / head / 0 / 68.6 | 69.942 | 0.51121 |
| AIM9B.JT | 35000 / head / 250 / 20 | 22.783 | 5000 / tail / 250 / 1.65 | 1.473 | 7.86786 |
| AA2.JT | 35000 / head / 250 / 20 | 19.999 | 5000 / tail / 250 / 1.65 | 1.653 | 10.65584 |
| AA8.JT | 35000 / head / 250 / 10 | 9.636 | 5000 / head / 250 / 4.5 | 4.643 | 19.23411 |
| R550.JT | 35000 / head / 250 / 15 | 15.002 | 5000 / head / 250 / 6.75 | 6.897 | 12.27334 |
| AA11B.JT | 35000 / head / 250 / 35 | 35.002 | 5000 / tail / 250 / 3.4 | 3.322 | 4.40229 |
| AIM7E.JT | 35000 / head / 250 / 35 | 33.772 | 5000 / head / 250 / 15.75 | 16.765 | 6.12119 |
| AIM7.JT | 35000 / head / 250 / 70 | 70.000 | 5000 / head / 250 / 31.5 | 30.849 | 2.71680 |
| R530.JT | 35000 / head / 250 / 40 | 40.000 | 5000 / head / 250 / 18 | 17.121 | 4.40469 |
| AAML.JT | 35000 / head / 250 / 150 | 150.000 | 5000 / head / 250 / 67.5 | 63.182 | 0.86965 |
| MICA.JT | 35000 / head / 250 / 60 | 60.000 | 5000 / head / 250 / 27 | 26.696 | 2.73614 |
| PL7.JT | 35000 / head / 250 / 14 | 14.001 | 5000 / head / 250 / 6.3 | 6.482 | 13.86697 |
| PL10.JT | 35000 / head / 250 / 50 | 50.000 | 5000 / head / 250 / 22.5 | 22.295 | 3.35995 |
| AEMP1.JT | 35000 / head / 250 / 50 | 50.000 | 5000 / head / 250 / 22.5 | 21.433 | 3.68315 |
| AA6.JT | 35000 / head / 250 / 30 | 29.998 | 5000 / head / 250 / 13.5 | 13.925 | 5.12116 |
| SA7.JT | 2756 / stationary / 0 / 4.2 | 4.200 | 1378 / stationary / 0 / 4.284 | 4.285 | 5.61790 |
| SA14.JT | 2953 / stationary / 0 / 4.5 | 4.501 | 1476 / stationary / 0 / 4.59 | 4.590 | 5.63079 |
| SA16.JT | 3412 / stationary / 0 / 5.2 | 5.200 | 1706 / stationary / 0 / 5.304 | 5.314 | 4.52410 |
| SA15.JT | 7874 / stationary / 0 / 12 | 12.000 | 3937 / stationary / 0 / 12.24 | 12.375 | 2.40556 |
| SA6.JT | 9843 / stationary / 0 / 24 | 24.000 | 4922 / stationary / 0 / 24.48 | 24.800 | 1.85883 |
| SA9.JT | 2756 / stationary / 0 / 4.2 | 4.200 | 1378 / stationary / 0 / 4.284 | 4.285 | 6.62326 |
| MIS.JT | 4265 / stationary / 0 / 6.5 | 6.500 | 2132 / stationary / 0 / 6.63 | 6.650 | 3.61888 |
| SEA_SPAR.JT | 9843 / stationary / 0 / 26 | 26.000 | 4922 / stationary / 0 / 26.52 | 26.862 | 1.78529 |
| FIM92.JT | 2625 / stationary / 0 / 4 | 4.001 | 1312 / stationary / 0 / 4.08 | 4.081 | 7.40483 |
| R440.JT | 6562 / stationary / 0 / 10 | 9.999 | 3281 / stationary / 0 / 10.2 | 10.642 | 2.81162 |
| SA2A.JT | 9843 / stationary / 0 / 29 | 28.998 | 4922 / stationary / 0 / 29.58 | 30.020 | 1.44888 |
| SA3.JT | 9843 / stationary / 0 / 25 | 25.000 | 4922 / stationary / 0 / 25.5 | 25.765 | 2.22815 |
| SA13.JT | 3281 / stationary / 0 / 5 | 5.000 | 1640 / stationary / 0 / 5.1 | 5.105 | 6.46996 |
| SA19.JT | 5249 / stationary / 0 / 8 | 8.000 | 2624 / stationary / 0 / 8.16 | 8.216 | 2.22044 |
| SAN3.JT | 9843 / stationary / 0 / 30 | 29.998 | 4922 / stationary / 0 / 30.6 | 31.128 | 1.21837 |
| SAN4.JT | 6562 / stationary / 0 / 10 | 9.999 | 3281 / stationary / 0 / 10.2 | 10.297 | 2.82971 |
| SAN7.JT | 9843 / stationary / 0 / 25 | 25.000 | 4922 / stationary / 0 / 25.5 | 25.900 | 1.45427 |
| SAN9.JT | 7874 / stationary / 0 / 12 | 12.000 | 3937 / stationary / 0 / 12.24 | 12.375 | 2.40556 |
| SAN11.JT | 5249 / stationary / 0 / 8 | 8.000 | 2624 / stationary / 0 / 8.16 | 8.216 | 2.22044 |
| SSN9.JT | 9843 / head / 0 / 150 | 145.949 | 9843 / tail / 15 / 135 | 137.939 | 0.47686 |
| AGM45.JT | 4921 / head / 0 / 40 | 41.772 | 2460 / head / 0 / 39.2 | 38.946 | 0.96515 |
| AGM65A.JT | 4921 / head / 0 / 27 | 26.998 | 2460 / head / 0 / 26.46 | 26.449 | 0.64743 |
| AGM84A.JT | 4921 / head / 0 / 185 | 184.989 | 2460 / head / 0 / 181.3 | 184.592 | 6.14250 |
| AGM84E.JT | 4921 / head / 0 / 93 | 92.994 | 2460 / head / 0 / 91.14 | 92.545 | 6.11791 |
| AS7.JT | 4921 / head / 0 / 10 | 10.000 | 2460 / head / 0 / 9.8 | 9.548 | 4.13647 |
| AS16.JT | 4921 / head / 0 / 150 | 149.990 | 2460 / head / 0 / 147 | 150.495 | 0.27410 |
| AS14.JT | 4921 / head / 0 / 10 | 10.000 | 2460 / head / 0 / 9.8 | 9.802 | 5.62542 |
| AS30.JT | 4921 / head / 0 / 11 | 11.000 | 2460 / head / 0 / 10.78 | 10.785 | 4.84078 |
| AT2.JT | 328 / head / 0 / 4 | 4.000 | 200 / head / 0 / 3.92 | 3.957 | 4.75125 |
| AS15.JT | 4921 / head / 0 / 18.288 | 18.287 | 2460 / head / 0 / 17.9222 | 17.910 | 2.23668 |
| AT12.JT | 4921 / head / 0 / 7.9248 | 8.206 | 2460 / head / 0 / 7.7663 | 7.756 | 10.66591 |

## Range regression outcomes

AIM9M, AIM9X and R73 (AA11) exceed both AIM9B and R3S (AA2) in every
measured 500/5,000/15,000/35,000 ft head/beam/tail cell, for physical and
seeker-valid reach. For all 25 A2A profiles, physical reach increases with
altitude wherever an intercept exists. AIM9B beam has no intercept at either
500 or 5,000 ft, so those two cells are both zero, not a reversed envelope.
Seeker-valid reach can also plateau at a decoded acquisition limit. The strict
altitude ordering applies to nonzero physical reach, not those capped cues.

Modern IR tail maxima at 5,000 ft are 3.15 km (M), 3.16 km (X) and 3.32 km
(R73); at 15,000 ft they are 4.35, 4.37 and 4.56 km. A deterministic synthetic
regression requires all three to catch a straight 250 m/s fighter from 2.7 km
astern at 5,000 and 15,000 ft with a 300 m/s launcher, and to miss a 12 km
500-foot tail chase. Imported live-guidance maxima appear above. This is not
a statistical claim for maneuvering targets or countermeasure use.

The SAM seeker altitude floor is evaluated as a launch employment limit,
not relative to the climbing missile. The regenerated moving-target rows
replace the earlier all-zero SA2/SA3 results. Live vulnerable-jet scenarios
at 10,000 and 20,000 ft test SA2A, SA3 and SA6 separately.

## Validation status

Required workspace checks and the review battery are in progress. Results
will be recorded here before this correction pass is finished. No Phase 3
study acceptance is claimed. Windows/macOS, Apple Silicon fingerprints and
retail comparison were not run in this Linux pass.

## Prediction cost

The corrected release example supports natural 4v4 and 23+ AI fights with
real launch-gate clients, plus the eight-aircraft injected-missile stress
lanes. It measures prediction p50/p95/max, queue refusals and admission
starvation. An admission wait over 1,200 ticks counts as starvation;
completion latency is separate. The saturation unit test also requires every
one of 320 continuously requesting clients to receive an answer. Results of
the corrected performance runs are pending.
