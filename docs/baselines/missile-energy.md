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
continuous no-escape envelope. Paired, geometry and regression diagnostics
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
[the data evidence](../formats/missile-energy-data.md#per-store-motors-and-paired-anchors).
All conditional anchors are agent choices unless explicitly sourced. The
M/X and R73 high-head anchors have been restored to their earlier lower values. Physical head-on reach does not give a rear-only seeker
a head-on lock. Held records remain unavailable in gameplay.

## Complete A2A physical reach

Each cell is head / beam / tail, km. All 25 inventory records are included,
including held records measured diagnostically without enabling them.

| Store | Primary reference km | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | ---: | --- | --- | --- | --- | --- |
| AA10.JT | 73 | 6.95 / 0.00 / 0.45 | 27.42 / 8.95 / 5.95 | 55.45 / 12.85 / 8.81 | 73.00 / 24.75 / 17.59 |
| AA11.JT | 30 | 7.87 / 0.00 / 0.95 | 22.07 / 10.26 / 6.64 | 25.00 / 14.39 / 9.49 | 30.00 / 20.41 / 14.48 |
| AA11B.JT | 30 | 7.87 / 0.00 / 0.95 | 22.07 / 10.26 / 6.64 | 25.00 / 14.39 / 9.49 | 30.00 / 20.41 / 14.48 |
| AA12.JT | 50 | 8.56 / 1.37 / 1.26 | 32.48 / 14.04 / 10.04 | 37.09 / 19.62 / 14.27 | 50.00 / 35.96 / 27.02 |
| AA2.JT | 20 | 4.43 / 0.00 / 0.26 | 16.31 / 2.96 / 2.04 | 17.52 / 4.40 / 2.99 | 20.01 / 8.01 / 5.46 |
| AA6.JT | 30 | 5.83 / 0.00 / 1.22 | 19.18 / 8.88 / 6.36 | 22.57 / 12.58 / 9.16 | 30.00 / 21.79 / 16.54 |
| AA8.JT | 10 | 4.59 / 0.00 / 0.28 | 7.94 / 3.74 / 2.25 | 8.80 / 4.95 / 3.11 | 10.01 / 6.31 / 4.32 |
| AA9.JT | 120 | 13.26 / 0.00 / 0.53 | 42.73 / 10.90 / 5.83 | 91.91 / 16.85 / 9.74 | 120.00 / 33.85 / 20.90 |
| AAML.JT | 150 | 20.52 / 4.10 / 2.24 | 64.53 / 22.27 / 14.73 | 111.78 / 30.75 / 21.05 | 150.00 / 58.59 / 40.84 |
| AEMP1.JT | 50 | 8.55 / 0.00 / 1.25 | 32.48 / 14.00 / 10.03 | 37.09 / 19.59 / 14.25 | 50.02 / 35.95 / 27.01 |
| AIM120.JT | 60 | 9.88 / 2.54 / 1.83 | 38.30 / 15.77 / 11.36 | 43.84 / 21.90 / 16.01 | 60.02 / 42.59 / 31.34 |
| AIM54C.JT | 150 | 12.95 / 0.00 / 1.25 | 72.14 / 15.04 / 8.42 | 112.50 / 23.23 / 13.96 | 150.00 / 45.05 / 28.51 |
| AIM7.JT | 70 | 18.08 / 3.87 / 4.10 | 59.49 / 43.26 / 32.56 | 63.48 / 47.79 / 37.47 | 70.00 / 54.70 / 44.53 |
| AIM7E.JT | 35 | 7.08 / 0.00 / 0.47 | 22.25 / 10.28 / 7.08 | 26.83 / 15.01 / 10.74 | 35.01 / 25.15 / 18.84 |
| AIM9B.JT | 20 | 3.33 / 0.00 / 0.32 | 13.32 / 5.12 / 3.40 | 15.05 / 7.63 / 5.05 | 18.09 / 11.62 / 8.09 |
| AIM9M.JT | 18 | 4.61 / 0.00 / 0.42 | 13.71 / 8.13 / 5.48 | 15.57 / 10.41 / 7.33 | 18.02 / 13.04 / 9.77 |
| AIM9X.JT | 18 | 5.59 / 0.00 / 0.62 | 13.84 / 8.36 / 5.63 | 15.55 / 10.44 / 7.34 | 18.02 / 13.07 / 9.81 |
| MICA.JT | 60 | 7.75 / 0.00 / 0.91 | 28.66 / 10.17 / 7.12 | 44.90 / 13.99 / 10.07 | 60.01 / 26.05 / 19.07 |
| PL10.JT | 50 | 8.55 / 0.00 / 0.81 | 32.57 / 11.23 / 7.65 | 36.91 / 16.25 / 11.55 | 50.00 / 31.91 / 22.64 |
| PL7.JT | 14 | 4.67 / 0.00 / 0.65 | 10.53 / 7.27 / 5.23 | 11.86 / 8.73 / 6.57 | 14.03 / 10.98 / 8.73 |
| R27ER.JT | 117 | 11.13 / 0.00 / 0.63 | 54.49 / 12.52 / 7.65 | 84.24 / 18.91 / 11.92 | 117.00 / 36.77 / 24.01 |
| R27ET.JT | 104 | 11.94 / 0.00 / 1.17 | 48.15 / 17.48 / 11.66 | 66.74 / 25.45 / 17.38 | 103.96 / 49.18 / 34.50 |
| R27T.JT | 63 | 7.38 / 0.00 / 0.78 | 29.45 / 12.05 / 8.55 | 40.91 / 17.02 / 12.35 | 62.99 / 31.74 / 23.61 |
| R530.JT | 40 | 8.91 / 2.17 / 1.36 | 26.18 / 10.92 / 7.67 | 29.73 / 14.82 / 10.67 | 40.01 / 27.51 / 20.35 |
| R550.JT | 15 | 4.56 / 0.00 / 0.61 | 10.96 / 7.32 / 5.20 | 12.49 / 9.04 / 6.73 | 15.02 / 11.70 / 9.25 |

## Complete A2A seeker-valid reach

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | 6.95 / 0.00 / 0.45 | 27.42 / 8.95 / 5.95 | 55.34 / 12.85 / 8.81 | 72.81 / 24.75 / 17.59 |
| AA11.JT | 7.86 / 0.00 / 0.95 | 11.56 / 10.25 / 6.64 | 11.56 / 14.37 / 9.49 | 11.56 / 15.30 / 14.48 |
| AA11B.JT | 7.86 / 0.00 / 0.95 | 11.56 / 10.25 / 6.64 | 11.56 / 14.37 / 9.49 | 11.56 / 15.30 / 14.48 |
| AA12.JT | 8.56 / 0.00 / 1.27 | 32.48 / 14.02 / 10.05 | 37.09 / 19.59 / 14.27 | 50.00 / 35.92 / 27.02 |
| AA2.JT | 0.00 / 0.00 / 0.26 | 0.00 / 0.00 / 2.04 | 0.00 / 0.00 / 2.99 | 0.00 / 0.00 / 5.46 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 4.58 / 0.00 / 0.28 | 5.78 / 3.72 / 2.25 | 5.78 / 4.92 / 3.11 | 5.78 / 6.29 / 4.32 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 20.51 / 4.02 / 2.25 | 64.53 / 22.24 / 14.74 | 111.78 / 30.72 / 21.05 | 150.00 / 58.48 / 40.84 |
| AEMP1.JT | 8.55 / 0.00 / 1.25 | 32.47 / 13.96 / 10.03 | 37.09 / 19.51 / 14.25 | 50.02 / 35.86 / 27.01 |
| AIM120.JT | 9.88 / 2.41 / 1.84 | 38.30 / 15.74 / 11.37 | 43.84 / 21.86 / 16.01 | 60.02 / 42.53 / 31.34 |
| AIM54C.JT | 12.95 / 0.00 / 1.25 | 72.15 / 15.04 / 8.42 | 112.50 / 23.19 / 13.96 | 150.00 / 44.99 / 28.52 |
| AIM7.JT | 18.08 / 3.87 / 4.10 | 59.47 / 43.16 / 32.56 | 63.46 / 47.57 / 37.47 | 70.00 / 54.06 / 44.53 |
| AIM7E.JT | 7.08 / 0.00 / 0.47 | 22.25 / 10.28 / 7.08 | 26.83 / 15.01 / 10.74 | 34.97 / 25.15 / 18.84 |
| AIM9B.JT | 0.00 / 0.00 / 0.32 | 0.00 / 0.00 / 3.40 | 0.00 / 0.00 / 5.05 | 0.00 / 0.00 / 8.09 |
| AIM9M.JT | 4.61 / 0.00 / 0.42 | 9.64 / 8.12 / 5.48 | 9.64 / 10.40 / 7.33 | 9.64 / 12.75 / 9.77 |
| AIM9X.JT | 5.58 / 0.00 / 0.63 | 9.64 / 8.34 / 5.63 | 9.64 / 10.42 / 7.34 | 9.64 / 12.75 / 9.81 |
| MICA.JT | 7.75 / 0.00 / 0.91 | 28.67 / 10.16 / 7.13 | 44.89 / 13.97 / 10.07 | 60.01 / 25.99 / 19.07 |
| PL10.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| PL7.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| R27ER.JT | 11.13 / 0.00 / 0.63 | 54.49 / 12.52 / 7.65 | 83.22 / 18.91 / 11.92 | 116.64 / 36.77 / 24.01 |
| R27ET.JT | 11.56 / 0.00 / 1.17 | 11.56 / 15.30 / 11.67 | 11.56 / 15.30 / 17.38 | 11.56 / 15.30 / 18.28 |
| R27T.JT | 7.38 / 0.00 / 0.78 | 11.56 / 12.04 / 8.55 | 11.56 / 15.30 / 12.35 | 11.56 / 15.30 / 18.28 |
| R530.JT | 8.91 / 2.17 / 1.36 | 26.18 / 10.92 / 7.67 | 29.73 / 14.82 / 10.67 | 40.01 / 27.51 / 20.35 |
| R550.JT | 4.56 / 0.00 / 0.61 | 9.64 / 7.31 / 5.21 | 9.64 / 9.03 / 6.73 | 9.64 / 11.70 / 9.25 |

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

Before and after use the same round-2 and current models as the A2A comparison
below. SAM/strike entries retain their appropriate target and launch conditions;
the four-altitude airborne head/tail grid is the A2A comparison, not an invented
air-intercept role for a ground weapon.

Four numbers on each side of a SAM cell are head / beam / tail / stationary; one number
for A-G is stationary. N/A marks a held or separate designator/command
channel, not zero physical range. Laser/command live integration is tested
separately; no seeker-valid claim is made from the physical-only rows.

| Store | Anchor km | Launcher/target m | Physical km, before → after | Seeker-valid km, before → after |
| --- | ---: | --- | --- | --- |
| AGM45.JT | 40 | 1500/10 | 40.00 → 19.32 | 40.00 → 19.32 |
| AGM65A.JT | 27 | 1500/10 | 27.00 → 11.61 | 15.17 → 11.61 |
| AGM65G.JT | 32.18688 | 1500/10 | 32.19 → 14.56 | 18.23 → 14.56 |
| AGM84A.JT | 185 | 1500/10 | 184.95 → 23.28 | 184.95 → 23.28 |
| AGM84E.JT | 93 | 1500/10 | 93.01 → 19.42 | 91.41 → 19.42 |
| AGM88.JT | 48.28032 | 1500/10 | 48.28 → 17.69 | 48.28 → 17.69 |
| AM39.JT | 70 | 1500/10 | 69.98 → 20.66 | 69.98 → 20.66 |
| AS14.JT | 10 | 1500/10 | 10.01 → 8.13 | N/A → N/A |
| AS15.JT | 18.288 retail | 1500/10 | 18.30 → 11.97 | 18.30 → 11.97 |
| AS16.JT | 150 | 1500/10 | 149.95 → 37.09 | 149.95 → 37.09 |
| AS30.JT | 11 | 1500/10 | 11.01 → 8.54 | N/A → N/A |
| AS7.JT | 10 | 1500/10 | 10.01 → 7.37 | 10.01 → 7.37 |
| AT12.JT | 7.9248 retail | 1500/10 | 7.94 → 7.11 | N/A → N/A |
| AT2.JT | 4 | 100/10 | 4.01 → 0.97 | N/A → N/A |
| FIM92.JT | 4 | 10/800 | 6.40 / 3.02 / 1.56 / 4.01 → 4.86 / 0.00 / 0.00 / 2.50 | 6.40 / 3.02 / 1.56 / 4.01 → 4.86 / 0.00 / 0.00 / 2.50 |
| MIS.JT | 6.5 | 10/1300 | 10.57 / 4.74 / 2.37 / 6.51 → 8.16 / 0.00 / 0.48 / 4.16 | 10.57 / 4.74 / 2.37 / 6.51 → 8.16 / 0.00 / 0.48 / 4.16 |
| R440.JT | 10 | 10/2000 | 13.53 / 9.24 / 6.51 / 10.01 → 9.18 / 4.87 / 3.67 / 6.17 | 13.53 / 9.24 / 6.51 / 10.01 → 9.18 / 4.87 / 3.67 / 6.17 |
| SA13.JT | 5 | 10/1000 | 7.51 / 4.20 / 2.48 / 5.01 → 5.38 / 0.14 / 0.67 / 2.96 | 4.75 / 4.20 / 2.48 / 5.01 → 4.75 / 0.14 / 0.67 / 2.96 |
| SA14.JT | 4.5 | 10/900 | 7.16 / 3.46 / 1.80 / 4.51 → 5.40 / 0.00 / 0.00 / 2.78 | 3.79 / 3.46 / 1.80 / 4.51 → 3.79 / 0.00 / 0.00 / 2.78 |
| SA15.JT | 12 | 10/2400 | 19.37 / 8.43 / 4.67 / 12.00 → 14.20 / 2.82 / 2.06 / 7.36 | 19.37 / 8.43 / 4.67 / 12.00 → 14.20 / 2.82 / 2.06 / 7.36 |
| SA16.JT | 5.2 | 10/1040 | 8.33 / 3.95 / 2.02 / 5.21 → 6.36 / 0.00 / 0.00 / 3.27 | 2.77 / 3.68 / 2.02 / 4.44 → 2.77 / 0.00 / 0.00 / 3.27 |
| SA19.JT | 8 | 10/1600 | 13.97 / 4.24 / 1.93 / 8.00 → 11.02 / 0.00 / 0.00 / 5.30 | 13.97 / 4.24 / 1.93 / 8.00 → 11.02 / 0.00 / 0.00 / 5.30 |
| SA2A.JT | 29 | 10/3000 | 49.73 / 15.88 / 9.80 / 29.00 → 22.05 / 6.26 / 5.24 / 12.82 | 49.73 / 15.88 / 9.80 / 29.00 → 22.05 / 6.26 / 5.24 / 12.82 |
| SA3.JT | 25 | 10/3000 | 42.36 / 13.69 / 7.94 / 25.00 → 20.83 / 3.02 / 4.28 / 12.20 | 42.32 / 13.69 / 7.94 / 25.00 → 20.83 / 3.02 / 4.28 / 12.20 |
| SA6.JT | 24 | 10/3000 | 41.15 / 12.82 / 7.48 / 24.00 → 19.41 / 4.45 / 3.99 / 11.55 | 41.09 / 12.82 / 7.48 / 24.00 → 19.41 / 4.45 / 3.99 / 11.55 |
| SA7.JT | 4.2 | 10/840 | 6.78 / 3.14 / 1.59 / 4.21 → 5.16 / 0.00 / 0.00 / 2.62 | 2.82 / 3.14 / 1.59 / 4.21 → 2.82 / 0.00 / 0.00 / 2.62 |
| SA9.JT | 4.2 | 10/840 | 6.70 / 3.20 / 1.66 / 4.21 → 5.07 / 0.00 / 0.00 / 2.61 | 3.80 / 3.20 / 1.66 / 4.21 → 3.80 / 0.00 / 0.00 / 2.61 |
| SAN11.JT | 8 | 10/1600 | 13.97 / 4.24 / 1.93 / 8.00 → 11.02 / 0.00 / 0.00 / 5.30 | 13.97 / 4.24 / 1.93 / 8.00 → 11.02 / 0.00 / 0.00 / 5.30 |
| SAN3.JT | 30 | 10/3000 | 52.06 / 16.53 / 10.55 / 29.99 → 22.25 / 7.28 / 5.64 / 12.96 | 52.13 / 16.53 / 10.55 / 29.99 → 22.25 / 7.28 / 5.64 / 12.96 |
| SAN4.JT | 10 | 10/2000 | 16.29 / 6.97 / 3.66 / 10.01 → 12.30 / 1.62 / 1.43 / 6.31 | 16.29 / 6.97 / 3.66 / 10.01 → 12.30 / 1.62 / 1.43 / 6.31 |
| SAN7.JT | 25 | 10/3000 | 43.16 / 13.50 / 8.22 / 25.00 → 18.94 / 5.28 / 4.34 / 11.41 | 43.13 / 13.50 / 8.22 / 25.00 → 18.94 / 5.28 / 4.34 / 11.41 |
| SAN9.JT | 12 | 10/2400 | 19.37 / 8.43 / 4.67 / 12.00 → 14.20 / 2.82 / 2.06 / 7.36 | 19.37 / 8.43 / 4.67 / 12.00 → 14.20 / 2.82 / 2.06 / 7.36 |
| SEA_SPAR.JT | 26 | 10/3000 | 43.30 / 15.43 / 8.89 / 26.00 → 19.04 / 0.00 / 3.95 / 11.38 | 43.29 / 15.43 / 8.89 / 26.00 → 19.04 / 0.00 / 3.95 / 11.38 |
| SSN9.JT | 150 | 10/10 | 149.95 → 35.02 | 149.95 → 35.02 |

AS15 and AT12 retain runtime decoded mass/range and the uniform class diameter
because their real-world identity is unresolved. Their original data-only fallback
drag fit targeted that retail range anchor before geometry drag; this is not real-world calibration.
The SAM aspect allowance continues the previous game policy where the
original aspect field has no reviewed interpretation. In particular, this
is not a claim that every real infrared SAM is all-aspect.


## Paired calibration results

Current physical reach, not a fresh per-store refit. Conditions are altitude ft /
aspect / target speed m/s / target km. All are fitted conditions unless explicitly
sourced. The geometry multiplier changes low-altitude SAM and strike results
as well as A2A. Their old primary-fit claim does not apply to this model.

The paired perturbation column changes fuel/derived thrust, drag and controlled
time by plus/minus 2%, measuring the largest change over the primary and
secondary conditions. The A2A regression table below also perturbs burn duration
at constant impulse, and samples primary plus 5k/35k tail conditions instead.
These different parameters and geometries explain the different percentages.
Neither is an uncertainty bound or a public performance claim.

| Store | Primary condition | Measured km | Secondary condition | Measured km | Drag | Controlled s | Paired perturbation change |
| --- | --- | ---: | --- | ---: | ---: | ---: | ---: |
| AIM120.JT | 35000 / head / 250 / 60 | 60.017 | 5000 / head / 250 / 27 | 38.302 | 2 | 52.308 | 1.35% |
| AIM9M.JT | 35000 / head / 250 / 18 | 18.027 | 5000 / tail / 250 / 6 | 5.476 | 3 | 16.492 | 1.86% |
| AIM9X.JT | 35000 / head / 250 / 18 | 18.024 | 5000 / tail / 250 / 6 | 5.630 | 3 | 16.417 | 1.86% |
| AIM54C.JT | 35000 / head / 250 / 150 | 150.000 | 5000 / head / 250 / 67.5 | 72.148 | 0.75 | 244.150 | 3.96% |
| AA10.JT | 35000 / head / 250 / 73 | 73.006 | 5000 / head / 250 / 32.85 | 27.421 | 3 | 107.733 | 2.70% |
| R27ER.JT | 35000 / head / 250 / 117 | 117.000 | 5000 / head / 250 / 52.65 | 54.496 | 1.5 | 191.217 | 3.55% |
| R27T.JT | 35000 / head / 250 / 63 | 63.006 | 5000 / head / 250 / 28.35 | 29.453 | 2.5 | 82.158 | 1.51% |
| R27ET.JT | 35000 / head / 250 / 104 | 103.998 | 5000 / head / 250 / 46.8 | 48.167 | 1.5 | 145.550 | 1.41% |
| AA11.JT | 35000 / head / 250 / 30 | 30.016 | 5000 / tail / 250 / 7 | 6.637 | 1.5 | 31.058 | 1.98% |
| AA12.JT | 35000 / head / 250 / 50 | 50.011 | 5000 / head / 250 / 22.5 | 32.480 | 2 | 43.083 | 1.37% |
| AA9.JT | 35000 / head / 250 / 120 | 120.000 | 5000 / head / 250 / 54 | 42.729 | 1 | 204.483 | 46.86% |
| AGM65G.JT | 4921 / head / 0 / 32.1869 | 14.565 | 2460.5 / head / 0 / 31.5431 | 12.761 | 0.75 | 80.558 | 1.55% |
| AGM88.JT | 4921 / head / 0 / 48.2803 | 17.698 | 2460.5 / head / 0 / 47.3147 | 11.055 | 0.75 | 86.275 | 1.57% |
| AM39.JT | 4921 / head / 0 / 70 | 20.666 | 2460.5 / head / 0 / 68.6 | 18.825 | 0.5 | 179.183 | 1.43% |
| AIM9B.JT | 35000 / head / 250 / 20 | 18.090 | 5000 / tail / 250 / 1.65 | 3.395 | 3 | 20.000 | 2.07% |
| AA2.JT | 35000 / head / 250 / 20 | 20.013 | 5000 / tail / 250 / 1.65 | 2.043 | 2.5 | 29.092 | 2.31% |
| AA8.JT | 35000 / head / 250 / 10 | 10.015 | 5000 / head / 250 / 4.5 | 7.939 | 3 | 11.375 | 1.91% |
| R550.JT | 35000 / head / 250 / 15 | 15.021 | 5000 / head / 250 / 6.75 | 10.965 | 3 | 11.517 | 1.78% |
| AA11B.JT | 35000 / head / 250 / 30 | 30.016 | 5000 / tail / 250 / 7 | 6.637 | 1.5 | 31.058 | 1.98% |
| AIM7E.JT | 35000 / head / 250 / 35 | 35.016 | 5000 / head / 250 / 15.75 | 22.251 | 3 | 32.408 | 1.31% |
| AIM7.JT | 35000 / head / 250 / 70 | 70.021 | 5000 / head / 250 / 31.5 | 59.501 | 0.5 | 51.550 | 1.95% |
| R530.JT | 35000 / head / 250 / 40 | 40.012 | 5000 / head / 250 / 18 | 26.182 | 2.5 | 36.975 | 1.31% |
| AAML.JT | 35000 / head / 250 / 150 | 150.000 | 5000 / head / 250 / 67.5 | 64.529 | 1 | 200.917 | 2.82% |
| MICA.JT | 35000 / head / 250 / 60 | 60.010 | 5000 / head / 250 / 27 | 28.668 | 3 | 75.725 | 3.85% |
| PL7.JT | 35000 / head / 250 / 14 | 14.028 | 5000 / head / 250 / 6.3 | 10.531 | 3 | 10.583 | 1.83% |
| PL10.JT | 35000 / head / 250 / 50 | 50.011 | 5000 / head / 250 / 22.5 | 32.567 | 2.5 | 49.667 | 1.31% |
| AEMP1.JT | 35000 / head / 250 / 50 | 50.015 | 5000 / head / 250 / 22.5 | 32.476 | 2 | 43.108 | 1.36% |
| AA6.JT | 35000 / head / 250 / 30 | 30.013 | 5000 / head / 250 / 13.5 | 19.188 | 2.5 | 27.017 | 1.29% |
| SA7.JT | 2756 / stationary / 0 / 4.2 | 2.623 | 1378 / stationary / 0 / 4.284 | 2.648 | 1 | 10.275 | 2.37% |
| SA14.JT | 2953 / stationary / 0 / 4.5 | 2.783 | 1476.5 / stationary / 0 / 4.59 | 2.796 | 1 | 10.625 | 2.38% |
| SA16.JT | 3412 / stationary / 0 / 5.2 | 3.275 | 1706 / stationary / 0 / 5.304 | 3.251 | 1 | 12.508 | 2.02% |
| SA15.JT | 7874 / stationary / 0 / 12 | 7.364 | 3937 / stationary / 0 / 12.24 | 6.614 | 1 | 29.475 | 1.21% |
| SA6.JT | 9843 / stationary / 0 / 24 | 11.549 | 4921.5 / stationary / 0 / 24.48 | 9.866 | 1 | 68.783 | 1.78% |
| SA9.JT | 2756 / stationary / 0 / 4.2 | 2.611 | 1378 / stationary / 0 / 4.284 | 2.636 | 1 | 9.983 | 2.35% |
| MIS.JT | 4265 / stationary / 0 / 6.5 | 4.165 | 2132.5 / stationary / 0 / 6.63 | 4.013 | 1 | 16.267 | 1.68% |
| SEA_SPAR.JT | 9843 / stationary / 0 / 26 | 11.378 | 4921.5 / stationary / 0 / 26.52 | 9.321 | 0.75 | 69.242 | 1.41% |
| FIM92.JT | 2625 / stationary / 0 / 4 | 2.501 | 1312.5 / stationary / 0 / 4.08 | 2.531 | 1 | 9.583 | 2.34% |
| R440.JT | 6562 / stationary / 0 / 10 | 6.166 | 3281 / stationary / 0 / 10.2 | 5.120 | 1 | 14.050 | 1.98% |
| SA2A.JT | 9843 / stationary / 0 / 29 | 12.819 | 4921.5 / stationary / 0 / 29.58 | 10.845 | 1 | 83.150 | 2.95% |
| SA3.JT | 9843 / stationary / 0 / 25 | 12.202 | 4921.5 / stationary / 0 / 25.5 | 10.738 | 1 | 69.533 | 2.73% |
| SA13.JT | 3281 / stationary / 0 / 5 | 2.961 | 1640.5 / stationary / 0 / 5.1 | 2.935 | 1 | 9.992 | 2.28% |
| SA19.JT | 5249 / stationary / 0 / 8 | 5.301 | 2624.5 / stationary / 0 / 8.16 | 4.551 | 1 | 23.892 | 1.17% |
| SAN3.JT | 9843 / stationary / 0 / 30 | 12.965 | 4921.5 / stationary / 0 / 30.6 | 10.675 | 1 | 88.517 | 2.73% |
| SAN4.JT | 6562 / stationary / 0 / 10 | 6.313 | 3281 / stationary / 0 / 10.2 | 5.778 | 1 | 25.175 | 1.35% |
| SAN7.JT | 9843 / stationary / 0 / 25 | 11.409 | 4921.5 / stationary / 0 / 25.5 | 9.371 | 1 | 72.933 | 2.23% |
| SAN9.JT | 7874 / stationary / 0 / 12 | 7.364 | 3937 / stationary / 0 / 12.24 | 6.614 | 1 | 29.475 | 1.21% |
| SAN11.JT | 5249 / stationary / 0 / 8 | 5.301 | 2624.5 / stationary / 0 / 8.16 | 4.551 | 1 | 23.892 | 1.17% |
| SSN9.JT | 9843 / head / 0 / 150 | 35.034 | 9843 / tail / 15 / 135 | 27.634 | 0.4 | 528.533 | 210.23% |
| AGM45.JT | 4921 / head / 0 / 40 | 19.326 | 2460.5 / head / 0 / 39.2 | 12.496 | 0.75 | 77.483 | 1.61% |
| AGM65A.JT | 4921 / head / 0 / 27 | 11.609 | 2460.5 / head / 0 / 26.46 | 9.534 | 0.75 | 86.950 | 1.65% |
| AGM84A.JT | 4921 / head / 0 / 185 | 23.287 | 2460.5 / head / 0 / 181.3 | 19.216 | 1 | 584.883 | 4.28% |
| AGM84E.JT | 4921 / head / 0 / 93 | 19.422 | 2460.5 / head / 0 / 91.14 | 15.407 | 1 | 296.408 | 2.51% |
| AS7.JT | 4921 / head / 0 / 10 | 7.374 | 2460.5 / head / 0 / 9.8 | 5.952 | 1.5 | 20.650 | 1.10% |
| AS16.JT | 4921 / head / 0 / 150 | 37.101 | 2460.5 / head / 0 / 147 | 35.604 | 0.25 | 337.792 | 1.56% |
| AS14.JT | 4921 / head / 0 / 10 | 8.126 | 2460.5 / head / 0 / 9.8 | 6.925 | 1 | 14.183 | 1.34% |
| AS30.JT | 4921 / head / 0 / 11 | 8.541 | 2460.5 / head / 0 / 10.78 | 7.331 | 1 | 14.383 | 1.24% |
| AT2.JT | 328 / head / 0 / 4 | 0.971 | 200 / head / 0 / 3.92 | 0.543 | 1 | 10.592 | 1.84% |
| AS15.JT | 4921 / head / 0 / 18.288 | 11.974 | 2460.5 / head / 0 / 17.9222 | 10.582 | 1 | 23.850 | 0.94% |
| AT12.JT | 4921 / head / 0 / 7.9248 | 7.110 | 2460.5 / head / 0 / 7.7663 | 6.335 | 0.75 | 11.667 | 2.50% |

## Geometry trade-off and before/after reach

The literal span/diameter formula uses the requested 5.6 anchor. A shared fade
`(1-smoothstep(h/35000))^80` was selected from exponents 1, 8, 16, 32, 64, 72,
80, 88 and 96, holding every non-Sparrow per-store parameter fixed. It is the
best tested head residual among candidates satisfying all 25 tail floors.
This is a constrained numerical choice, not a claim of a global optimum.

A broad exponent-1 fade breaks 22 of 25 5k tail floors; AIM120 head reach falls
to 10.763 km against 27 km. Exponents 16/32/64 break 11/3/1 floors. At 72,
AA2 still reaches only 1.974 km. At 80 it reaches 2.043 km, the minimum across
all entries. Exponent 88 raises that to 2.090 but worsens mean head residual.
The high-power fade is effectively gone by 10,000 ft, despite reaching exactly
one at 35,000 ft. Most 5k head-reference errors therefore remain. The geometry
rule cannot fix them within this tested shared-curve family while retaining
the requested floor. No per-missile fudge was added to conceal that conflict.

A final neighboring-anchor comparison retained exponent 80 and the final
Sparrow. No nearby anchor solves the 5k mismatch: 5.0 improves the fitted
500 ft diagnostic but leaves 5k unchanged; 6.2 gains only 0.24 percentage
points at 5k while worsening the deck diagnostic and narrowing the tail
margin. The requested 5.6 is retained as the compromise. These means are
absolute residuals over all 25 A2A records, including held entries.

| Anchor | Mean absolute head residual, 500 ft | At 5k ft | Minimum tail, 5k km |
| --- | ---: | ---: | ---: |
| 5.0 | 44.25% | 42.43% | 2.060 |
| 5.6 | 47.39% | 42.43% | 2.043 |
| 6.2 | 49.89% | 42.20% | 2.027 |

For AIM120 the altitude factors at 0/500/5000/15000/35000 ft are
5.600000 / 5.382102 / 1.048176 / 1.000000 / 1.000000. Every profile uses the same curve, scaled by its geometry.
Geometry values and field provenance belong to [the data table](../formats/missile-energy-data.md#geometry-scaled-low-altitude-drag).

Before uses the round-2 range measurement, whose physical model is unchanged
in reviewer commit c42555ed, including the old Sparrow. After includes
the Sparrow refit and geometry drag. Each row reports head reach and its
residual against a **fitted diagnostic** reference curve, plus tail reach.
That curve is primary_head_km times `0.45 + 0.55*(h-5000)/30000`.
Only its 35k endpoint and existing 5k head targets come from the prior fit;
500/15k interpolation and 5k values for profiles with tail secondary targets
are agent-authored diagnostics, not additional public evidence. There is no
published head/tail reference at every height. Tail values are measured reach,
not residuals against a head reference. All ranges are km.

| Store | Altitude ft | Head before → after | Head residual before → after | Tail before → after |
| --- | ---: | --- | --- | --- |
| AA10.JT | 500 | 26.817 → 6.949 | -0.0% → -74.1% | 5.675 → 0.448 |
| AA10.JT | 5000 | 30.613 → 27.416 | -6.8% → -16.5% | 6.473 → 5.952 |
| AA10.JT | 15000 | 55.455 → 55.455 | +19.9% → +19.9% | 8.815 → 8.815 |
| AA10.JT | 35000 | 73.000 → 73.000 | +0.0% → +0.0% | 17.585 → 17.585 |
| AA11.JT | 500 | 21.638 → 7.867 | +96.3% → -28.6% | 6.144 → 0.948 |
| AA11.JT | 5000 | 22.569 → 22.075 | +67.2% → +63.5% | 7.056 → 6.637 |
| AA11.JT | 15000 | 25.002 → 25.002 | +31.6% → +31.6% | 9.490 → 9.490 |
| AA11.JT | 35000 | 30.005 → 30.005 | +0.0% → +0.0% | 14.482 → 14.482 |
| AA11B.JT | 500 | 21.638 → 7.867 | +96.3% → -28.6% | 6.144 → 0.948 |
| AA11B.JT | 5000 | 22.569 → 22.075 | +67.2% → +63.5% | 7.056 → 6.637 |
| AA11B.JT | 15000 | 25.002 → 25.002 | +31.6% → +31.6% | 9.490 → 9.490 |
| AA11B.JT | 35000 | 30.005 → 30.005 | +0.0% → +0.0% | 14.482 → 14.482 |
| AA12.JT | 500 | 30.850 → 8.563 | +67.9% → -53.4% | 9.515 → 1.264 |
| AA12.JT | 5000 | 32.557 → 32.479 | +44.7% → +44.4% | 10.767 → 10.045 |
| AA12.JT | 15000 | 37.090 → 37.090 | +17.1% → +17.1% | 14.267 → 14.267 |
| AA12.JT | 35000 | 50.000 → 50.000 | +0.0% → +0.0% | 27.023 → 27.023 |
| AA2.JT | 500 | 16.149 → 4.426 | +119.7% → -39.8% | 1.905 → 0.259 |
| AA2.JT | 5000 | 16.541 → 16.307 | +83.8% → +81.2% | 2.180 → 2.043 |
| AA2.JT | 15000 | 17.517 → 17.517 | +38.3% → +38.3% | 2.993 → 2.993 |
| AA2.JT | 35000 | 20.013 → 20.013 | +0.1% → +0.1% | 5.464 → 5.464 |
| AA6.JT | 500 | 15.616 → 5.834 | +41.6% → -47.1% | 6.014 → 1.215 |
| AA6.JT | 5000 | 19.966 → 19.183 | +47.9% → +42.1% | 6.811 → 6.355 |
| AA6.JT | 15000 | 22.569 → 22.569 | +18.8% → +18.8% | 9.157 → 9.157 |
| AA6.JT | 35000 | 30.005 → 30.005 | +0.0% → +0.0% | 16.541 → 16.541 |
| AA8.JT | 500 | 7.763 → 4.588 | +111.2% → +24.8% | 2.072 → 0.285 |
| AA8.JT | 5000 | 8.067 → 7.937 | +79.3% → +76.4% | 2.377 → 2.247 |
| AA8.JT | 15000 | 8.798 → 8.798 | +38.9% → +38.9% | 3.106 → 3.106 |
| AA8.JT | 35000 | 10.014 → 10.014 | +0.1% → +0.1% | 4.323 → 4.323 |
| AA9.JT | 500 | 12.485 → 13.255 | -71.7% → -69.9% | 2.233 → 0.533 |
| AA9.JT | 5000 | 62.970 → 42.726 | +16.6% → -20.9% | 6.816 → 5.834 |
| AA9.JT | 15000 | 91.907 → 91.907 | +20.9% → +20.9% | 9.744 → 9.744 |
| AA9.JT | 35000 | 120.000 → 120.000 | +0.0% → +0.0% | 20.897 → 20.897 |
| AAML.JT | 500 | 35.310 → 20.517 | -35.9% → -62.8% | 13.708 → 2.243 |
| AAML.JT | 5000 | 66.739 → 64.529 | -1.1% → -4.4% | 15.620 → 14.729 |
| AAML.JT | 15000 | 111.782 → 111.782 | +17.7% → +17.7% | 21.049 → 21.049 |
| AAML.JT | 35000 | 150.000 → 150.000 | +0.0% → +0.0% | 40.841 → 40.841 |
| AEMP1.JT | 500 | 30.845 → 8.549 | +67.9% → -53.5% | 9.500 → 1.249 |
| AEMP1.JT | 5000 | 32.554 → 32.476 | +44.7% → +44.3% | 10.752 → 10.033 |
| AEMP1.JT | 15000 | 37.087 → 37.087 | +17.1% → +17.1% | 14.248 → 14.248 |
| AEMP1.JT | 35000 | 50.015 → 50.015 | +0.0% → +0.0% | 27.010 → 27.010 |
| AIM120.JT | 500 | 36.359 → 9.879 | +64.9% → -55.2% | 10.694 → 1.835 |
| AIM120.JT | 5000 | 38.369 → 38.301 | +42.1% → +41.9% | 12.058 → 11.361 |
| AIM120.JT | 15000 | 43.840 → 43.840 | +15.4% → +15.4% | 16.011 → 16.011 |
| AIM120.JT | 35000 | 60.017 → 60.017 | +0.0% → +0.0% | 31.342 → 31.342 |
| AIM54C.JT | 500 | 12.342 → 12.954 | -77.6% → -76.5% | 2.380 → 1.254 |
| AIM54C.JT | 5000 | 75.264 → 72.143 | +11.5% → +6.9% | 9.811 → 8.422 |
| AIM54C.JT | 15000 | 112.500 → 112.500 | +18.4% → +18.4% | 13.962 → 13.962 |
| AIM54C.JT | 35000 | 150.000 → 150.000 | +0.0% → +0.0% | 28.509 → 28.509 |
| AIM7.JT | 500 | 11.539 → 18.081 | -55.1% → -29.7% | 2.948 → 4.101 |
| AIM7.JT | 5000 | 37.618 → 59.491 | +19.4% → +88.9% | 5.792 → 32.561 |
| AIM7.JT | 15000 | 54.998 → 63.482 | +24.1% → +43.2% | 7.994 → 37.468 |
| AIM7.JT | 35000 | 70.000 → 70.000 | +0.0% → +0.0% | 16.081 → 44.528 |
| AIM7E.JT | 500 | 16.776 → 7.084 | +30.4% → -44.9% | 7.062 → 0.475 |
| AIM7E.JT | 5000 | 23.852 → 22.246 | +51.4% → +41.2% | 8.061 → 7.076 |
| AIM7E.JT | 15000 | 26.827 → 26.827 | +21.0% → +21.0% | 10.745 → 10.745 |
| AIM7E.JT | 35000 | 35.009 → 35.009 | +0.0% → +0.0% | 18.841 → 18.841 |
| AIM9B.JT | 500 | 13.158 → 3.334 | +79.0% → -54.6% | 3.211 → 0.322 |
| AIM9B.JT | 5000 | 13.678 → 13.323 | +52.0% → +48.0% | 3.693 → 3.395 |
| AIM9B.JT | 15000 | 15.046 → 15.046 | +18.8% → +18.8% | 5.055 → 5.055 |
| AIM9B.JT | 35000 | 18.087 → 18.087 | -9.6% → -9.6% | 8.087 → 8.087 |
| AIM9M.JT | 500 | 13.557 → 4.612 | +104.9% → -30.3% | 5.312 → 0.422 |
| AIM9M.JT | 5000 | 14.185 → 13.710 | +75.1% → +69.3% | 5.938 → 5.476 |
| AIM9M.JT | 15000 | 15.572 → 15.572 | +36.6% → +36.6% | 7.326 → 7.326 |
| AIM9M.JT | 35000 | 18.024 → 18.024 | +0.1% → +0.1% | 9.773 → 9.773 |
| AIM9X.JT | 500 | 13.526 → 5.588 | +104.5% → -15.5% | 5.316 → 0.624 |
| AIM9X.JT | 5000 | 14.153 → 13.836 | +74.7% → +70.8% | 5.944 → 5.629 |
| AIM9X.JT | 15000 | 15.553 → 15.553 | +36.4% → +36.4% | 7.340 → 7.340 |
| AIM9X.JT | 35000 | 18.024 → 18.024 | +0.1% → +0.1% | 9.811 → 9.811 |
| MICA.JT | 500 | 25.135 → 7.749 | +14.0% → -64.9% | 6.643 → 0.912 |
| MICA.JT | 5000 | 29.709 → 28.661 | +10.0% → +6.2% | 7.527 → 7.122 |
| MICA.JT | 15000 | 44.895 → 44.895 | +18.1% → +18.1% | 10.073 → 10.073 |
| MICA.JT | 35000 | 60.006 → 60.006 | +0.0% → +0.0% | 19.065 → 19.065 |
| PL10.JT | 500 | 31.094 → 8.549 | +69.2% → -53.5% | 3.502 → 0.811 |
| PL10.JT | 5000 | 32.711 → 32.566 | +45.4% → +44.7% | 8.699 → 7.647 |
| PL10.JT | 15000 | 36.914 → 36.914 | +16.6% → +16.6% | 11.549 → 11.549 |
| PL10.JT | 35000 | 50.000 → 50.000 | +0.0% → +0.0% | 22.642 → 22.642 |
| PL7.JT | 500 | 10.280 → 4.665 | +99.8% → -9.3% | 4.988 → 0.653 |
| PL7.JT | 5000 | 10.770 → 10.529 | +71.0% → +67.1% | 5.474 → 5.232 |
| PL7.JT | 15000 | 11.864 → 11.864 | +33.8% → +33.8% | 6.569 → 6.569 |
| PL7.JT | 35000 | 14.026 → 14.026 | +0.2% → +0.2% | 8.730 → 8.730 |
| R27ER.JT | 500 | 50.110 → 11.129 | +16.5% → -74.1% | 7.220 → 0.629 |
| R27ER.JT | 5000 | 57.365 → 54.492 | +9.0% → +3.5% | 8.402 → 7.652 |
| R27ER.JT | 15000 | 84.242 → 84.242 | +13.7% → +13.7% | 11.919 → 11.919 |
| R27ER.JT | 35000 | 117.000 → 117.000 | +0.0% → +0.0% | 24.014 → 24.014 |
| R27ET.JT | 500 | 34.681 → 11.940 | -9.3% → -68.8% | 11.184 → 1.171 |
| R27ET.JT | 5000 | 54.352 → 48.151 | +16.1% → +2.9% | 12.755 → 11.665 |
| R27ET.JT | 15000 | 66.742 → 66.742 | +1.3% → +1.3% | 17.384 → 17.384 |
| R27ET.JT | 35000 | 103.962 → 103.962 | -0.0% → -0.0% | 34.496 → 34.496 |
| R27T.JT | 500 | 29.378 → 7.383 | +26.9% → -68.1% | 8.205 → 0.778 |
| R27T.JT | 5000 | 32.543 → 29.450 | +14.8% → +3.9% | 9.309 → 8.548 |
| R27T.JT | 15000 | 40.910 → 40.910 | +2.5% → +2.5% | 12.354 → 12.354 |
| R27T.JT | 35000 | 62.990 → 62.990 | -0.0% → -0.0% | 23.605 → 23.605 |
| R530.JT | 500 | 24.908 → 8.908 | +69.4% → -39.4% | 7.056 → 1.359 |
| R530.JT | 5000 | 26.234 → 26.182 | +45.7% → +45.5% | 7.997 → 7.667 |
| R530.JT | 15000 | 29.730 → 29.730 | +17.4% → +17.4% | 10.674 → 10.674 |
| R530.JT | 35000 | 40.011 → 40.011 | +0.0% → +0.0% | 20.348 → 20.348 |
| R550.JT | 500 | 10.690 → 4.556 | +93.9% → -17.4% | 4.929 → 0.606 |
| R550.JT | 5000 | 11.239 → 10.964 | +66.5% → +62.4% | 5.476 → 5.203 |
| R550.JT | 15000 | 12.493 → 12.493 | +31.5% → +31.5% | 6.729 → 6.729 |
| R550.JT | 35000 | 15.021 → 15.021 | +0.1% → +0.1% | 9.254 → 9.254 |

## Range regression outcomes

All 25 A2A profiles must exceed 2 km astern at 5,000 ft, increase at 15k/35k,
and fail controlled shots at 150% of the primary head reference. M/X/R73 must
outreach B/R3S/AA8 astern at all three heights. The 12 km, 500 ft tail drag must
escape M/X/R73. Newer Sparrow > AIM7E, R27ER > R27R (AA10), and R27ET > R27T
are checked at every 500/5k/15k/35k × head/beam/tail cell. Sparrow requires
12 strict wins. Each R27 pair requires 11 strict wins: at 500 ft beam both
members are unshootable under the geometry rule (zero/zero), a documented
unreachable cell rather than an upgrade win. No other ties or reversals pass. AIM120/R77/MICA >
AIM7E/R40 (AA6)/Super530D is checked at 35k head, where the reference ordering
is clear. Cross-family age alone does not establish a universal range ordering;
burn duration, drag and control time differ. Newer Sparrow's 70 km reference
is not forced below AMRAAM's 60 km reference.

The A2A perturbation check additionally varies burn duration at constant
impulse; every sampled range change must remain below 10%. The paired column
above uses different conditions and omits that fourth perturbation.

| Store | Astern 5,000 ft km | Astern 15,000 ft km | Astern 35,000 ft km | Largest A2A perturbation change |
| --- | ---: | ---: | ---: | ---: |
| AA10.JT | 5.953 | 8.816 | 17.589 | 1.89% |
| AA11.JT | 6.637 | 9.491 | 14.486 | 1.98% |
| AA11B.JT | 6.637 | 9.491 | 14.486 | 1.98% |
| AA12.JT | 10.048 | 14.267 | 27.024 | 1.75% |
| AA2.JT | 2.043 | 2.993 | 5.466 | 2.31% |
| AA6.JT | 6.357 | 9.160 | 16.546 | 1.78% |
| AA8.JT | 2.247 | 3.107 | 4.324 | 2.48% |
| AA9.JT | 5.835 | 9.745 | 20.897 | 2.45% |
| AAML.JT | 14.733 | 21.049 | 40.841 | 1.81% |
| AEMP1.JT | 10.033 | 14.253 | 27.011 | 1.74% |
| AIM120.JT | 11.363 | 16.011 | 31.346 | 1.71% |
| AIM54C.JT | 8.422 | 13.966 | 28.516 | 2.29% |
| AIM7.JT | 32.566 | 37.473 | 44.529 | 2.25% |
| AIM7E.JT | 7.077 | 10.748 | 18.843 | 1.72% |
| AIM9B.JT | 3.395 | 5.055 | 8.089 | 2.07% |
| AIM9M.JT | 5.476 | 7.326 | 9.775 | 1.92% |
| AIM9X.JT | 5.630 | 7.340 | 9.811 | 1.93% |
| MICA.JT | 7.124 | 10.073 | 19.068 | 1.82% |
| PL10.JT | 7.648 | 11.551 | 22.646 | 2.05% |
| PL7.JT | 5.232 | 6.569 | 8.730 | 1.92% |
| R27ER.JT | 7.652 | 11.921 | 24.019 | 2.49% |
| R27ET.JT | 11.665 | 17.384 | 34.496 | 1.88% |
| R27T.JT | 8.550 | 12.356 | 23.605 | 1.83% |
| R530.JT | 7.669 | 10.676 | 20.348 | 1.79% |
| R550.JT | 5.205 | 6.730 | 9.255 | 1.82% |

## Validation status

Implementation mode, Linux x86-64, Rust 1.91.1. Final workspace tests: 7,205
passed, zero failed, 106 ignored across 59 suites. Workspace/all-target
Clippy with warnings denied, formatting, workspace build, Python tests,
source/both binary asset guards and documentation checks passed. All Cargo
commands used --locked, four build workers and four test threads at nice 15.
The memory guard checked free -g and waited below 8 GiB available before each
heavy command. Heavy runs were serialized.

All 35 battery scenarios passed, including the prior 34 and the new turning
player lane. Windows ran through tools/agent-run.sh, at most one at a time,
with nice 15 restored inside Hyprland; battery concurrency was two jobs.
The reviewer's c42555ed straight-flight envelope projection and earlier
0c2209a7 non-vacuous decoy-fuze tests remain intact.

The behavior-hash tripwire records model identity 10468354493397306314 and
quantized trace 12183010523406479596. It covers synthetic trajectories and
live contact/launcher exclusion, and detects an injected PN change without
an identity change. Rule revisions for derivation, force integration and
prediction were incremented; numeric motor, geometry and curve inputs enter
the model identity directly. This is finite regression coverage, not proof
that every possible rule change or platform difference is caught.

The player tests sample the actual launcher-view HUD consumer in the opening
20 seconds. The straight lanes require answers on 75% of all ticks and 90%
of observed ticks. Turning lanes prescribe level circles at 3/10 degrees/s
with normal enemy flying and sensors, at least 31 clients and 600 observed
ticks, and answers on 90% of observed ticks. A miss is a valid answer, distinct
from pending or absent observation. No future target pose is supplied to the
predictor. Tests also reject reversals, changed turn rates, teleports and old
answers, preserve strict AI permission, and exercise saturated admission.

| Player HUD load | Hit / miss / pending | No observation | Answers / all ticks | Answers / observed ticks |
| --- | --- | ---: | ---: | ---: |
| 4v4 | 1922 / 112 / 0 | 366 | 84.75% | 100.00% |
| 23 AI | 1920 / 47 / 0 | 433 | 81.96% | 100.00% |
| 23 AI, 3 degrees/s | 1570 / 0 / 0 | 830 | 65.42% | 100.00% |
| 23 AI, 10 degrees/s | 661 / 0 / 0 | 1739 | 27.54% | 100.00% |

Windows/macOS execution, Apple Silicon fingerprints and retail comparison
were not run. The model remains fitted, with the geometry/reference conflict
and unreachable cells stated above. Phase 3 remains stopped and its previous
study remains labelled pre-fix. This pass is calibration and regression validation.

## Prediction cost

Sequential release runs on AMD Ryzen 9 7900X, Linux x86-64, nice 15 while John
used the PC. Cold is the first 240 ticks, later samples follow those ticks.
Wall and thread CPU times include request work and prediction. Warm reads
are measured separately and perform no flyout. The shared 16,384 quarter-step
budget is unchanged; player work uses a portion of it, with the unused part
returned to the common queue. There is no synchronous fallback.

| Lane | Cold wall p50/p95 ms | Later wall p50/p95 ms | Later CPU p95 ms | Wall max ms | CPU max ms |
| --- | --- | --- | ---: | ---: | ---: |
| natural-ukr | 0.678 / 0.758 | 0.024 / 0.732 | 0.729 | 0.995 | 0.984 |
| natural-bal | 0.673 / 0.753 | 0.031 / 0.715 | 0.714 | 1.703 | 0.916 |
| natural-spa | 0.683 / 0.749 | 0.031 / 0.724 | 0.720 | 1.022 | 0.995 |
| natural-gre | 0.690 / 0.755 | 0.033 / 0.736 | 0.733 | 2.050 | 0.992 |
| natural-big | 0.760 / 0.798 | 0.731 / 0.799 | 0.796 | 1.389 | 1.012 |
| turning-3 | 0.765 / 0.804 | 0.769 / 0.806 | 0.804 | 0.989 | 0.988 |
| turning-10 | 0.761 / 0.804 | 0.771 / 0.803 | 0.801 | 1.035 | 1.033 |
| hold-frozen | 0.871 / 0.966 | 0.863 / 0.968 | 0.965 | 1.148 | 1.133 |
| hold-moving | 0.833 / 0.923 | 0.737 / 0.912 | 0.904 | 1.172 | 1.165 |
| hold-cruise | 0.694 / 0.719 | 0.708 / 0.746 | 0.744 | 0.993 | 0.991 |

The conservative sum of independent prediction and frame-reader wall maxima
is 2.050 ms. Wall maxima include scheduling delays on the shared PC.
All lanes had zero refused requests and zero admission starvation. CPU maxima
remain below 1.17 ms; the 2.05 ms wall outlier is not a strict sub-2-ms wall
guarantee on a shared machine. Admission is not proof of a usable answer:
the synthetic moving-cruise pressure lane had zero qualified cached reads in
its 20-second window. Its bounded cost is measured, not an availability pass.
The natural-fight and actual player availability results are reported separately.

| Natural fight | Spec reference launches | Energy launches | AI gate hit / miss / pending | Available gate samples |
| --- | ---: | ---: | --- | ---: |
| natural-ukr | 11 | 12 | 67690 / 206939 / 30923 | 89.88% |
| natural-bal | 12 | 13 | 65422 / 202342 / 31448 | 89.49% |
| natural-spa | 12 | 13 | 65448 / 202324 / 31440 | 89.49% |
| natural-gre | 12 | 13 | 65422 / 202354 / 31436 | 89.49% |
| natural-big | 24 | 25 | 127964 / 124957 / 106807 | 70.31% |
