# Missile energy calibration and validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-26. Model calibration, not retail
parity or a validated engineering flight envelope. Semantic model identity,
branch `missile-overhaul`, Linux x86-64. Input came from the isolated imported
profile for this task; no media bytes are committed. The shared offline probe
is `cargo run --locked --release -p tore-sim --example missile_energy_ranges
-- LOCAL_JT_DIRECTORY`. `--fit-pair` prints constrained motor/drag/control-time candidates; `--verify`
checks tail reach, altitude, generation ordering, finite shots and local
parameter sensitivity. Neither changes source or runs during gameplay.

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
All conditional anchors are agent choices unless explicitly sourced. The
M/X and R73 high-head anchors have been restored to their earlier lower values. Physical head-on reach does not give a rear-only seeker
a head-on lock. Held records remain unavailable in gameplay.

## Complete A2A physical reach

Each cell is head / beam / tail, km. All 25 inventory records are included,
including held records measured diagnostically without enabling them.

| Store | Primary reference km | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | ---: | --- | --- | --- | --- |
| AA10.JT | 73 | 26.82 / 8.61 / 5.67 | 30.61 / 9.69 / 6.47 | 55.45 / 12.85 / 8.81 | 73.00 / 24.75 / 17.59 |
| AA11.JT | 30 | 21.64 / 9.57 / 6.14 | 22.57 / 10.90 / 7.06 | 25.00 / 14.39 / 9.49 | 30.00 / 20.41 / 14.48 |
| AA11B.JT | 30 | 21.64 / 9.57 / 6.14 | 22.57 / 10.90 / 7.06 | 25.00 / 14.39 / 9.49 | 30.00 / 20.41 / 14.48 |
| AA12.JT | 50 | 30.85 / 13.36 / 9.52 | 32.56 / 15.00 / 10.77 | 37.09 / 19.62 / 14.27 | 50.00 / 35.96 / 27.02 |
| AA2.JT | 20 | 16.15 / 2.92 / 1.91 | 16.54 / 3.30 / 2.18 | 17.52 / 4.40 / 2.99 | 20.01 / 8.01 / 5.46 |
| AA6.JT | 30 | 15.62 / 8.43 / 6.01 | 19.97 / 9.49 / 6.81 | 22.57 / 12.58 / 9.16 | 30.00 / 21.79 / 16.54 |
| AA8.JT | 10 | 7.76 / 3.52 / 2.07 | 8.07 / 3.95 / 2.38 | 8.80 / 4.95 / 3.11 | 10.01 / 6.31 / 4.32 |
| AA9.JT | 120 | 12.48 / 6.32 / 2.23 | 62.97 / 12.29 / 6.82 | 91.91 / 16.85 / 9.74 | 120.00 / 33.85 / 20.90 |
| AAML.JT | 150 | 35.31 / 20.86 / 13.71 | 66.74 / 23.43 / 15.62 | 111.78 / 30.75 / 21.05 | 150.00 / 58.59 / 40.84 |
| AEMP1.JT | 50 | 30.84 / 13.32 / 9.50 | 32.55 / 14.96 / 10.75 | 37.09 / 19.59 / 14.25 | 50.02 / 35.95 / 27.01 |
| AIM120.JT | 60 | 36.36 / 14.90 / 10.69 | 38.37 / 16.69 / 12.06 | 43.84 / 21.90 / 16.01 | 60.02 / 42.59 / 31.34 |
| AIM54C.JT | 150 | 12.34 / 6.40 / 2.38 | 75.26 / 16.93 / 9.81 | 112.50 / 23.23 / 13.96 | 150.00 / 45.05 / 28.51 |
| AIM7.JT | 70 | 11.54 / 6.55 / 2.95 | 37.62 / 9.14 / 5.79 | 55.00 / 12.30 / 7.99 | 70.00 / 23.61 / 16.08 |
| AIM7E.JT | 35 | 16.78 / 10.31 / 7.06 | 23.85 / 11.53 / 8.06 | 26.83 / 15.01 / 10.74 | 35.01 / 25.15 / 18.84 |
| AIM9B.JT | 20 | 13.16 / 4.92 / 3.21 | 13.68 / 5.61 / 3.69 | 15.05 / 7.63 / 5.05 | 18.09 / 11.62 / 8.09 |
| AIM9M.JT | 18 | 13.56 / 7.94 / 5.31 | 14.19 / 8.77 / 5.94 | 15.57 / 10.41 / 7.33 | 18.02 / 13.04 / 9.77 |
| AIM9X.JT | 18 | 13.53 / 7.95 / 5.32 | 14.15 / 8.79 / 5.94 | 15.55 / 10.44 / 7.34 | 18.02 / 13.07 / 9.81 |
| MICA.JT | 60 | 25.14 / 9.57 / 6.64 | 29.71 / 10.70 / 7.53 | 44.90 / 13.99 / 10.07 | 60.01 / 26.05 / 19.07 |
| PL10.JT | 50 | 31.09 / 6.68 / 3.50 | 32.71 / 12.53 / 8.70 | 36.91 / 16.25 / 11.55 | 50.00 / 31.91 / 22.64 |
| PL7.JT | 14 | 10.28 / 6.99 / 4.99 | 10.77 / 7.54 / 5.47 | 11.86 / 8.73 / 6.57 | 14.03 / 10.98 / 8.73 |
| R27ER.JT | 117 | 50.11 / 11.92 / 7.22 | 57.37 / 13.68 / 8.40 | 84.24 / 18.91 / 11.92 | 117.00 / 36.77 / 24.01 |
| R27ET.JT | 104 | 34.68 / 16.83 / 11.18 | 54.35 / 19.02 / 12.76 | 66.74 / 25.45 / 17.38 | 103.96 / 49.18 / 34.50 |
| R27T.JT | 63 | 29.38 / 11.62 / 8.21 | 32.54 / 13.04 / 9.31 | 40.91 / 17.02 / 12.35 | 62.99 / 31.74 / 23.61 |
| R530.JT | 40 | 24.91 / 10.13 / 7.06 | 26.23 / 11.34 / 8.00 | 29.73 / 14.82 / 10.67 | 40.01 / 27.51 / 20.35 |
| R550.JT | 15 | 10.69 / 6.98 / 4.93 | 11.24 / 7.64 / 5.48 | 12.49 / 9.04 / 6.73 | 15.02 / 11.70 / 9.25 |

## Complete A2A seeker-valid reach

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | 26.82 / 8.61 / 5.67 | 30.61 / 9.69 / 6.47 | 55.34 / 12.85 / 8.81 | 72.81 / 24.75 / 17.59 |
| AA11.JT | 11.56 / 9.54 / 6.14 | 11.56 / 10.88 / 7.06 | 11.56 / 14.37 / 9.49 | 11.56 / 15.30 / 14.48 |
| AA11B.JT | 11.56 / 9.54 / 6.14 | 11.56 / 10.88 / 7.06 | 11.56 / 14.37 / 9.49 | 11.56 / 15.30 / 14.48 |
| AA12.JT | 30.85 / 13.34 / 9.52 | 32.56 / 14.97 / 10.77 | 37.09 / 19.59 / 14.27 | 50.00 / 35.92 / 27.02 |
| AA2.JT | 0.00 / 0.00 / 1.91 | 0.00 / 0.00 / 2.18 | 0.00 / 0.00 / 2.99 | 0.00 / 0.00 / 5.46 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 5.78 / 3.48 / 2.07 | 5.78 / 3.92 / 2.38 | 5.78 / 4.92 / 3.11 | 5.78 / 6.29 / 4.32 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 35.31 / 20.83 / 13.71 | 66.74 / 23.41 / 15.62 | 111.78 / 30.72 / 21.05 | 150.00 / 58.48 / 40.84 |
| AEMP1.JT | 30.84 / 13.27 / 9.50 | 32.55 / 14.90 / 10.76 | 37.09 / 19.51 / 14.25 | 50.02 / 35.86 / 27.01 |
| AIM120.JT | 36.36 / 14.88 / 10.70 | 38.37 / 16.66 / 12.06 | 43.84 / 21.86 / 16.01 | 60.02 / 42.53 / 31.34 |
| AIM54C.JT | 12.34 / 6.40 / 2.38 | 75.26 / 16.93 / 9.81 | 112.50 / 23.19 / 13.96 | 150.00 / 44.99 / 28.52 |
| AIM7.JT | 11.54 / 6.55 / 2.95 | 37.62 / 9.14 / 5.79 | 54.91 / 12.30 / 7.99 | 69.82 / 23.61 / 16.08 |
| AIM7E.JT | 16.78 / 10.31 / 7.06 | 23.85 / 11.53 / 8.06 | 26.83 / 15.01 / 10.74 | 34.97 / 25.15 / 18.84 |
| AIM9B.JT | 0.00 / 0.00 / 3.21 | 0.00 / 0.00 / 3.69 | 0.00 / 0.00 / 5.05 | 0.00 / 0.00 / 8.09 |
| AIM9M.JT | 9.64 / 7.92 / 5.31 | 9.64 / 8.76 / 5.94 | 9.64 / 10.40 / 7.33 | 9.64 / 12.75 / 9.77 |
| AIM9X.JT | 9.64 / 7.93 / 5.32 | 9.64 / 8.78 / 5.94 | 9.64 / 10.42 / 7.34 | 9.64 / 12.75 / 9.81 |
| MICA.JT | 25.14 / 9.56 / 6.64 | 29.72 / 10.69 / 7.53 | 44.89 / 13.97 / 10.07 | 60.01 / 25.99 / 19.07 |
| PL10.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| PL7.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| R27ER.JT | 50.11 / 11.92 / 7.22 | 57.32 / 13.68 / 8.40 | 83.22 / 18.91 / 11.92 | 116.64 / 36.77 / 24.01 |
| R27ET.JT | 11.56 / 15.30 / 11.19 | 11.56 / 15.30 / 12.76 | 11.56 / 15.30 / 17.38 | 11.56 / 15.30 / 18.28 |
| R27T.JT | 11.56 / 11.60 / 8.21 | 11.56 / 13.03 / 9.31 | 11.56 / 15.30 / 12.35 | 11.56 / 15.30 / 18.28 |
| R530.JT | 24.91 / 10.13 / 7.06 | 26.23 / 11.34 / 8.00 | 29.73 / 14.82 / 10.67 | 40.01 / 27.51 / 20.35 |
| R550.JT | 9.64 / 6.97 / 4.93 | 9.64 / 7.63 / 5.48 | 9.64 / 9.03 / 6.73 | 9.64 / 11.70 / 9.25 |

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
| AGM45.JT | 40 | 1500/10 | 40.00 | 40.00 |
| AGM65A.JT | 27 | 1500/10 | 27.00 | 15.17 |
| AGM65G.JT | 32.18688 | 1500/10 | 32.19 | 18.23 |
| AGM84A.JT | 185 | 1500/10 | 184.95 | 184.95 |
| AGM84E.JT | 93 | 1500/10 | 93.01 | 91.41 |
| AGM88.JT | 48.28032 | 1500/10 | 48.28 | 48.28 |
| AM39.JT | 70 | 1500/10 | 69.98 | 69.98 |
| AS14.JT | 10 | 1500/10 | 10.01 | N/A |
| AS15.JT | 18.288 retail | 1500/10 | 18.30 | 18.30 |
| AS16.JT | 150 | 1500/10 | 149.95 | 149.95 |
| AS30.JT | 11 | 1500/10 | 11.01 | N/A |
| AS7.JT | 10 | 1500/10 | 10.01 | 10.01 |
| AT12.JT | 7.9248 retail | 1500/10 | 7.94 | N/A |
| AT2.JT | 4 | 100/10 | 4.01 | N/A |
| FIM92.JT | 4 | 10/800 | 6.40 / 3.02 / 1.56 / 4.01 | 6.40 / 3.02 / 1.56 / 4.01 |
| MIS.JT | 6.5 | 10/1300 | 10.57 / 4.74 / 2.37 / 6.51 | 10.57 / 4.74 / 2.37 / 6.51 |
| R440.JT | 10 | 10/2000 | 13.53 / 9.24 / 6.51 / 10.01 | 13.53 / 9.24 / 6.51 / 10.01 |
| SA13.JT | 5 | 10/1000 | 7.51 / 4.20 / 2.48 / 5.01 | 4.75 / 4.20 / 2.48 / 5.01 |
| SA14.JT | 4.5 | 10/900 | 7.16 / 3.46 / 1.80 / 4.51 | 3.79 / 3.46 / 1.80 / 4.51 |
| SA15.JT | 12 | 10/2400 | 19.37 / 8.43 / 4.67 / 12.00 | 19.37 / 8.43 / 4.67 / 12.00 |
| SA16.JT | 5.2 | 10/1040 | 8.33 / 3.95 / 2.02 / 5.21 | 2.77 / 3.68 / 2.02 / 4.44 |
| SA19.JT | 8 | 10/1600 | 13.97 / 4.24 / 1.93 / 8.00 | 13.97 / 4.24 / 1.93 / 8.00 |
| SA2A.JT | 29 | 10/3000 | 49.73 / 15.88 / 9.80 / 29.00 | 49.73 / 15.88 / 9.80 / 29.00 |
| SA3.JT | 25 | 10/3000 | 42.36 / 13.69 / 7.94 / 25.00 | 42.32 / 13.69 / 7.94 / 25.00 |
| SA6.JT | 24 | 10/3000 | 41.15 / 12.82 / 7.48 / 24.00 | 41.09 / 12.82 / 7.48 / 24.00 |
| SA7.JT | 4.2 | 10/840 | 6.78 / 3.14 / 1.59 / 4.21 | 2.82 / 3.14 / 1.59 / 4.21 |
| SA9.JT | 4.2 | 10/840 | 6.70 / 3.20 / 1.66 / 4.21 | 3.80 / 3.20 / 1.66 / 4.21 |
| SAN11.JT | 8 | 10/1600 | 13.97 / 4.24 / 1.93 / 8.00 | 13.97 / 4.24 / 1.93 / 8.00 |
| SAN3.JT | 30 | 10/3000 | 52.06 / 16.53 / 10.55 / 29.99 | 52.13 / 16.53 / 10.55 / 29.99 |
| SAN4.JT | 10 | 10/2000 | 16.29 / 6.97 / 3.66 / 10.01 | 16.29 / 6.97 / 3.66 / 10.01 |
| SAN7.JT | 25 | 10/3000 | 43.16 / 13.50 / 8.22 / 25.00 | 43.13 / 13.50 / 8.22 / 25.00 |
| SAN9.JT | 12 | 10/2400 | 19.37 / 8.43 / 4.67 / 12.00 | 19.37 / 8.43 / 4.67 / 12.00 |
| SEA_SPAR.JT | 26 | 10/3000 | 43.30 / 15.43 / 8.89 / 26.00 | 43.29 / 15.43 / 8.89 / 26.00 |
| SSN9.JT | 150 | 10/10 | 149.95 | 149.95 |

AS15 and AT12 retain runtime decoded mass/range and the uniform class diameter
because their real-world identity is unresolved. Their data-only fallback
drag fit meets that retail range anchor; this is not real-world calibration.
The SAM aspect allowance continues the previous game policy where the
original aspect field has no reviewed interpretation. In particular, this
is not a claim that every real infrared SAM is all-aspect.

## Paired calibration results

Primary and secondary conditions are altitude ft / aspect / target speed m/s /
target km. They are fitted conditions unless the data source explicitly says
otherwise. The old 15% paired-fit claim no longer applies: residuals are shown
instead of raising targets to conceal the constrained model's differences.

| Store | Primary condition | Measured km | Secondary condition | Measured km | Drag | Controlled s | Perturbation change |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: |
| AIM120.JT | 35000 / head / 250 / 60 | 60.017 | 5000 / head / 250 / 27 | 38.369 | 2 | 52.308 | 1.35% |
| AIM9M.JT | 35000 / head / 250 / 18 | 18.027 | 5000 / tail / 250 / 6 | 5.939 | 3 | 16.492 | 1.88% |
| AIM9X.JT | 35000 / head / 250 / 18 | 18.024 | 5000 / tail / 250 / 6 | 5.945 | 3 | 16.417 | 1.87% |
| AIM54C.JT | 35000 / head / 250 / 150 | 150.000 | 5000 / head / 250 / 67.5 | 75.271 | 0.75 | 244.150 | 3.40% |
| AA10.JT | 35000 / head / 250 / 73 | 73.006 | 5000 / head / 250 / 32.85 | 30.617 | 3 | 107.733 | 2.66% |
| R27ER.JT | 35000 / head / 250 / 117 | 117.000 | 5000 / head / 250 / 52.65 | 57.365 | 1.5 | 191.217 | 3.03% |
| R27T.JT | 35000 / head / 250 / 63 | 63.006 | 5000 / head / 250 / 28.35 | 32.543 | 2.5 | 82.158 | 1.62% |
| R27ET.JT | 35000 / head / 250 / 104 | 103.998 | 5000 / head / 250 / 46.8 | 54.368 | 1.5 | 145.550 | 1.57% |
| AA11.JT | 35000 / head / 250 / 30 | 30.016 | 5000 / tail / 250 / 7 | 7.057 | 1.5 | 31.058 | 1.98% |
| AA12.JT | 35000 / head / 250 / 50 | 50.011 | 5000 / head / 250 / 22.5 | 32.558 | 2 | 43.083 | 1.37% |
| AA9.JT | 35000 / head / 250 / 120 | 120.000 | 5000 / head / 250 / 54 | 62.970 | 1 | 204.483 | 4.12% |
| AGM65G.JT | 4921 / head / 0 / 32.1869 | 32.190 | 2460.5 / head / 0 / 31.5431 | 32.122 | 0.75 | 80.558 | 1.04% |
| AGM88.JT | 4921 / head / 0 / 48.2803 | 48.285 | 2460.5 / head / 0 / 47.3147 | 46.671 | 0.75 | 86.275 | 1.06% |
| AM39.JT | 4921 / head / 0 / 70 | 69.998 | 2460.5 / head / 0 / 68.6 | 70.046 | 0.5 | 179.183 | 1.04% |
| AIM9B.JT | 35000 / head / 250 / 20 | 18.090 | 5000 / tail / 250 / 1.65 | 3.693 | 3 | 20.000 | 2.07% |
| AA2.JT | 35000 / head / 250 / 20 | 20.013 | 5000 / tail / 250 / 1.65 | 2.181 | 2.5 | 29.092 | 2.31% |
| AA8.JT | 35000 / head / 250 / 10 | 10.015 | 5000 / head / 250 / 4.5 | 8.068 | 3 | 11.375 | 1.91% |
| R550.JT | 35000 / head / 250 / 15 | 15.021 | 5000 / head / 250 / 6.75 | 11.239 | 3 | 11.517 | 1.78% |
| AA11B.JT | 35000 / head / 250 / 30 | 30.016 | 5000 / tail / 250 / 7 | 7.057 | 1.5 | 31.058 | 1.98% |
| AIM7E.JT | 35000 / head / 250 / 35 | 35.016 | 5000 / head / 250 / 15.75 | 23.857 | 3 | 32.408 | 1.31% |
| AIM7.JT | 35000 / head / 250 / 70 | 70.007 | 5000 / head / 250 / 31.5 | 37.618 | 3 | 104.992 | 5.89% |
| R530.JT | 35000 / head / 250 / 40 | 40.012 | 5000 / head / 250 / 18 | 26.234 | 2.5 | 36.975 | 1.31% |
| AAML.JT | 35000 / head / 250 / 150 | 150.000 | 5000 / head / 250 / 67.5 | 66.746 | 1 | 200.917 | 2.41% |
| MICA.JT | 35000 / head / 250 / 60 | 60.010 | 5000 / head / 250 / 27 | 29.710 | 3 | 75.725 | 3.57% |
| PL7.JT | 35000 / head / 250 / 14 | 14.028 | 5000 / head / 250 / 6.3 | 10.772 | 3 | 10.583 | 1.83% |
| PL10.JT | 35000 / head / 250 / 50 | 50.011 | 5000 / head / 250 / 22.5 | 32.712 | 2.5 | 49.667 | 1.29% |
| AEMP1.JT | 35000 / head / 250 / 50 | 50.015 | 5000 / head / 250 / 22.5 | 32.554 | 2 | 43.108 | 1.36% |
| AA6.JT | 35000 / head / 250 / 30 | 30.013 | 5000 / head / 250 / 13.5 | 19.966 | 2.5 | 27.017 | 1.29% |
| SA7.JT | 2756 / stationary / 0 / 4.2 | 4.210 | 1378 / stationary / 0 / 4.284 | 4.281 | 1 | 10.275 | 2.62% |
| SA14.JT | 2953 / stationary / 0 / 4.5 | 4.511 | 1476.5 / stationary / 0 / 4.59 | 4.588 | 1 | 10.625 | 2.68% |
| SA16.JT | 3412 / stationary / 0 / 5.2 | 5.211 | 1706 / stationary / 0 / 5.304 | 5.302 | 1 | 12.508 | 2.32% |
| SA15.JT | 7874 / stationary / 0 / 12 | 12.005 | 3937 / stationary / 0 / 12.24 | 12.222 | 1 | 29.475 | 2.05% |
| SA6.JT | 9843 / stationary / 0 / 24 | 24.002 | 4921.5 / stationary / 0 / 24.48 | 24.544 | 1 | 68.783 | 1.90% |
| SA9.JT | 2756 / stationary / 0 / 4.2 | 4.211 | 1378 / stationary / 0 / 4.284 | 4.284 | 1 | 9.983 | 2.67% |
| MIS.JT | 4265 / stationary / 0 / 6.5 | 6.508 | 2132.5 / stationary / 0 / 6.63 | 6.627 | 1 | 16.267 | 2.05% |
| SEA_SPAR.JT | 9843 / stationary / 0 / 26 | 26.005 | 4921.5 / stationary / 0 / 26.52 | 26.518 | 0.75 | 69.242 | 2.07% |
| FIM92.JT | 2625 / stationary / 0 / 4 | 4.010 | 1312.5 / stationary / 0 / 4.08 | 4.080 | 1 | 9.583 | 2.70% |
| R440.JT | 6562 / stationary / 0 / 10 | 10.014 | 3281 / stationary / 0 / 10.2 | 10.135 | 1 | 14.050 | 2.08% |
| SA2A.JT | 9843 / stationary / 0 / 29 | 29.003 | 4921.5 / stationary / 0 / 29.58 | 29.528 | 1 | 83.150 | 1.72% |
| SA3.JT | 9843 / stationary / 0 / 25 | 25.002 | 4921.5 / stationary / 0 / 25.5 | 25.582 | 1 | 69.533 | 2.02% |
| SA13.JT | 3281 / stationary / 0 / 5 | 5.015 | 1640.5 / stationary / 0 / 5.1 | 5.099 | 1 | 9.992 | 2.83% |
| SA19.JT | 5249 / stationary / 0 / 8 | 8.004 | 2624.5 / stationary / 0 / 8.16 | 8.166 | 1 | 23.892 | 1.65% |
| SAN3.JT | 9843 / stationary / 0 / 30 | 30.002 | 4921.5 / stationary / 0 / 30.6 | 30.511 | 1 | 88.517 | 1.46% |
| SAN4.JT | 6562 / stationary / 0 / 10 | 10.007 | 3281 / stationary / 0 / 10.2 | 10.202 | 1 | 25.175 | 2.05% |
| SAN7.JT | 9843 / stationary / 0 / 25 | 25.002 | 4921.5 / stationary / 0 / 25.5 | 25.489 | 1 | 72.933 | 1.73% |
| SAN9.JT | 7874 / stationary / 0 / 12 | 12.005 | 3937 / stationary / 0 / 12.24 | 12.222 | 1 | 29.475 | 2.05% |
| SAN11.JT | 5249 / stationary / 0 / 8 | 8.004 | 2624.5 / stationary / 0 / 8.16 | 8.166 | 1 | 23.892 | 1.65% |
| SSN9.JT | 9843 / head / 0 / 150 | 149.990 | 9843 / tail / 15 / 135 | 142.160 | 0.4 | 528.533 | 0.83% |
| AGM45.JT | 4921 / head / 0 / 40 | 40.005 | 2460.5 / head / 0 / 39.2 | 38.644 | 0.75 | 77.483 | 1.07% |
| AGM65A.JT | 4921 / head / 0 / 27 | 27.003 | 2460.5 / head / 0 / 26.46 | 26.850 | 0.75 | 86.950 | 1.09% |
| AGM84A.JT | 4921 / head / 0 / 185 | 184.989 | 2460.5 / head / 0 / 181.3 | 184.935 | 1 | 584.883 | 2.01% |
| AGM84E.JT | 4921 / head / 0 / 93 | 93.010 | 2460.5 / head / 0 / 91.14 | 92.946 | 1 | 296.408 | 2.03% |
| AS7.JT | 4921 / head / 0 / 10 | 10.007 | 2460.5 / head / 0 / 9.8 | 9.999 | 1.5 | 20.650 | 1.44% |
| AS16.JT | 4921 / head / 0 / 150 | 149.990 | 2460.5 / head / 0 / 147 | 150.658 | 0.25 | 337.792 | 1.27% |
| AS14.JT | 4921 / head / 0 / 10 | 10.013 | 2460.5 / head / 0 / 9.8 | 10.121 | 1 | 14.183 | 2.09% |
| AS30.JT | 4921 / head / 0 / 11 | 11.014 | 2460.5 / head / 0 / 10.78 | 11.110 | 1 | 14.383 | 2.08% |
| AT2.JT | 328 / head / 0 / 4 | 4.008 | 200 / head / 0 / 3.92 | 4.001 | 1 | 10.592 | 2.32% |
| AS15.JT | 4921 / head / 0 / 18.288 | 18.301 | 2460.5 / head / 0 / 17.9222 | 18.415 | 1 | 23.850 | 1.62% |
| AT12.JT | 4921 / head / 0 / 7.9248 | 7.945 | 2460.5 / head / 0 / 7.7663 | 7.616 | 0.75 | 11.667 | 5.49% |

## Range regression outcomes

All 25 A2A profiles exceed 2 km physical tail reach at 5,000 ft. Each increases
at 15,000 and 35,000 ft. M/X/R73 exceed B/R3S/AA8 at each of those tail heights.
Rear-only seeker restrictions remain separate: this is not a claim of universal
head-on physical range ordering. Head-on shots at 150% of each primary anchor
fail their controlled-intercept forecast. Expired live bodies remain ballistic.

The sensitivity check perturbs fuel/derived thrust, drag, control time and
burn duration by plus/minus 2%. Burn-duration perturbation preserves impulse.
It checks primary reach and 5k/35k tail reach. Every A2A profile stays below
10% relative range change; the measured maximum is2.68%. This is a
local smoothness check, not proof against every possible disconnected interval.

| Store | Astern 5,000 ft km | Astern 15,000 ft km | Astern 35,000 ft km | Largest perturbation change |
| --- | ---: | ---: | ---: | ---: |
| AA10.JT | 6.475 | 8.816 | 17.589 | 1.90% |
| AA11.JT | 7.057 | 9.491 | 14.486 | 1.98% |
| AA11B.JT | 7.057 | 9.491 | 14.486 | 1.98% |
| AA12.JT | 10.769 | 14.267 | 27.024 | 1.75% |
| AA2.JT | 2.181 | 2.993 | 5.466 | 2.31% |
| AA6.JT | 6.813 | 9.160 | 16.546 | 1.84% |
| AA8.JT | 2.377 | 3.107 | 4.324 | 2.48% |
| AA9.JT | 6.816 | 9.745 | 20.897 | 2.55% |
| AAML.JT | 15.624 | 21.049 | 40.841 | 1.85% |
| AEMP1.JT | 10.754 | 14.253 | 27.011 | 1.75% |
| AIM120.JT | 12.058 | 16.011 | 31.346 | 1.75% |
| AIM54C.JT | 9.814 | 13.966 | 28.516 | 2.48% |
| AIM7.JT | 5.794 | 7.995 | 16.083 | 2.68% |
| AIM7E.JT | 8.061 | 10.748 | 18.843 | 1.81% |
| AIM9B.JT | 3.693 | 5.055 | 8.089 | 2.07% |
| AIM9M.JT | 5.939 | 7.326 | 9.775 | 1.92% |
| AIM9X.JT | 5.945 | 7.340 | 9.811 | 1.93% |
| MICA.JT | 7.528 | 10.073 | 19.068 | 1.82% |
| PL10.JT | 8.701 | 11.551 | 22.646 | 2.05% |
| PL7.JT | 5.475 | 6.569 | 8.730 | 1.92% |
| R27ER.JT | 8.402 | 11.921 | 24.019 | 2.56% |
| R27ET.JT | 12.758 | 17.384 | 34.496 | 1.91% |
| R27T.JT | 9.310 | 12.356 | 23.605 | 1.83% |
| R530.JT | 7.997 | 10.676 | 20.348 | 1.80% |
| R550.JT | 5.477 | 6.730 | 9.255 | 1.82% |

## Validation status

Implementation mode, Linux x86-64, Rust 1.91.1. Final workspace tests: 7,199
passed, zero failed, 106 ignored across 59 suites. Workspace/all-target
Clippy with warnings denied, formatting, workspace build, Python tests,
source/both binary asset guards and documentation checks passed. All Cargo
commands used --locked, four build workers and four test threads at nice 15.
Free memory was checked before each heavy command; the guard waits below
8 GiB available. Heavy runs were serialized. No golden constants were changed.

The strengthened battery has 34 passing scenarios: 33 passed in the full run,
then the expanded long-shot case passed its targeted rerun. Its old 160-shot
short/medium sample contained only hits or decoys after refitting; the 240-shot
replacement includes long shots and requires a physical control-loss miss,
while keeping serial/parallel output equality. The isolated SA2 variant also
passed a targeted rerun. Coverage includes the previous
33, stricter max-range replay rejection of any unintended hit, a vulnerable
SA2 battery requiring damage with only its launcher armed and radar support retained, the 23-AI launch-count gate and the new player
HUD availability check. The reviewer's two decoy-fuze tests remain intact.
Windows ran through tools/agent-run.sh with at most one window; the launcher
restored nice 15 inside Hyprland. Battery concurrency was two jobs.

The player test samples the opening 20 seconds of natural 4v4 and 23-AI fights,
using the actual player HUD consumer, a loaded AIM120 and a live enemy sensor
track. It requires answers on at least 75% of all 2,400 ticks and 90% of ticks
with an observed cue. Missing sensor observations count against the first
threshold. Later sensor loss is not called a prediction failure.

| Player HUD load | Hit / miss / pending | No observation | Answers / all ticks | Answers / observed ticks |
| --- | --- | ---: | ---: | ---: |
| 4v4 | 1924 / 110 / 0 | 366 | 84.75% | 100.00% |
| 23 AI | 1901 / 60 / 3 | 436 | 81.71% | 99.85% |

Windows/macOS execution, Apple Silicon fingerprints and retail comparison
were not run. The model remains fitted in the places identified by the data
and residual tables. Phase 3 remains stopped; its old study is invalidated
and labelled pre-fix. This pass is calibration and regression validation.

## Prediction cost

Sequential release runs on AMD Ryzen 9 7900X, Linux x86-64, nice 15 while John
used the PC. Cold means the first 240 ticks. Moving means later ticks.
Wall times include request construction, submission and the prediction work;
CPU timings distinguish computation from scheduling delays. No synchronous
fallback was added and the shared 16,384 quarter-step budget is unchanged.

| Lane | Cold wall p50/p95 ms | Later wall p50/p95 ms | Later CPU p95 ms | Wall max ms | CPU max ms |
| --- | --- | --- | ---: | ---: | ---: |
| natural-ukr | 0.639 / 0.708 | 0.029 / 0.695 | 0.693 | 1.311 | 1.323 |
| natural-bal | 0.639 / 0.705 | 0.029 / 0.678 | 0.676 | 0.863 | 0.858 |
| natural-spa | 0.651 / 0.711 | 0.029 / 0.688 | 0.687 | 1.266 | 1.264 |
| natural-gre | 0.656 / 0.706 | 0.030 / 0.690 | 0.688 | 0.908 | 0.853 |
| natural-big | 0.733 / 0.792 | 0.699 / 0.769 | 0.767 | 1.285 | 1.266 |
| hold-frozen | 0.798 / 0.872 | 0.805 / 0.918 | 0.917 | 0.946 | 0.949 |
| hold-moving | 0.785 / 0.812 | 0.782 / 0.883 | 0.882 | 1.068 | 1.061 |
| hold-cruise | 0.675 / 0.763 | 0.674 / 0.725 | 0.723 | 1.649 | 1.644 |

The conservative sum of each lane's independent prediction and frame-reader
maxima is at most 1.855 ms, within the approximately 2 ms budget.
All lanes had zero refusals or admission starvation. Point availability is
measured separately below; admission alone is not evidence of a usable answer.
Optional range-band searches can remain pending under motion/load. Retained
range displays require matching geometry and expire after ten seconds.

| Natural fight | Spec reference launches | Energy launches | AI gate hit / miss / pending | Available gate samples |
| --- | ---: | ---: | --- | ---: |
| natural-ukr | 11 | 13 | 100185 / 208500 / 30527 | 91.00% |
| natural-bal | 12 | 13 | 121206 / 198712 / 34810 | 90.19% |
| natural-spa | 12 | 13 | 104479 / 198141 / 35084 | 89.61% |
| natural-gre | 12 | 13 | 117287 / 198302 / 35339 | 89.93% |
| natural-big | 24 | 28 | 136317 / 135911 / 106196 | 71.94% |
