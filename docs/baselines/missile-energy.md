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
-- LOCAL_JT_DIRECTORY`. `--fit-class` exports fixed-budget two-condition candidates; the expanded
`--fit-motors` and `--fit-staged` searches also fit per-store motor values.
`--verify` checks explicit reference bands, role/family ordering, tail floors,
finite shots and local parameter sensitivity. Neither changes source or runs during gameplay.

This baseline contains Phase 2.6 balance fits and same-tick inert threat removal.
Class budgets and post-burn inert defeat remain separate from motor fitting. The earlier Phase 3
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
| AA10.JT | 73 | 26.10 / 11.18 / 5.98 | 27.35 / 12.92 / 7.21 | 35.15 / 18.75 / 11.52 | 64.54 / 38.05 / 25.40 |
| AA11.JT | 30 | 13.06 / 9.17 / 6.88 | 14.61 / 10.27 / 7.76 | 19.22 / 13.46 / 10.30 | 27.00 / 21.22 / 17.00 |
| AA11B.JT | 30 | 13.06 / 9.17 / 6.88 | 14.61 / 10.27 / 7.76 | 19.22 / 13.46 / 10.30 | 27.00 / 21.22 / 17.00 |
| AA12.JT | 50 | 35.41 / 15.06 / 8.73 | 43.44 / 17.49 / 10.20 | 48.14 / 23.18 / 14.03 | 52.04 / 27.85 / 18.86 |
| AA2.JT | 20 | 6.14 / 3.29 / 2.20 | 7.07 / 3.77 / 2.53 | 9.88 / 5.17 / 3.53 | 16.77 / 10.02 / 6.76 |
| AA6.JT | 30 | 11.53 / 7.61 / 5.46 | 12.90 / 8.53 / 6.18 | 16.98 / 11.24 / 8.29 | 32.76 / 21.49 / 16.25 |
| AA8.JT | 10 | 12.13 / 4.21 / 2.13 | 12.25 / 4.33 / 2.25 | 12.49 / 4.49 / 2.49 | 12.83 / 3.38 / 2.84 |
| AA9.JT | 120 | 13.44 / 5.79 / 1.46 | 55.41 / 14.51 / 6.87 | 58.53 / 19.57 / 9.78 | 118.13 / 43.68 / 24.30 |
| AAML.JT | 150 | 38.11 / 22.18 / 11.86 | 46.40 / 25.65 / 15.58 | 63.57 / 36.29 / 23.23 | 149.76 / 71.76 / 46.80 |
| AEMP1.JT | 50 | 41.63 / 16.33 / 8.60 | 45.15 / 18.92 / 10.42 | 47.31 / 22.84 / 13.48 | 50.48 / 26.63 / 17.62 |
| AIM120.JT | 60 | 33.42 / 16.72 / 10.28 | 40.92 / 19.41 / 12.05 | 52.70 / 27.35 / 17.28 | 59.98 / 36.49 / 25.72 |
| AIM54C.JT | 150 | 13.08 / 6.02 / 1.73 | 56.13 / 21.04 / 11.00 | 65.21 / 29.95 / 16.83 | 144.51 / 68.74 / 40.77 |
| AIM7.JT | 70 | 16.11 / 9.10 / 5.88 | 18.19 / 10.45 / 6.89 | 24.27 / 14.23 / 9.71 | 58.58 / 27.82 / 19.63 |
| AIM7E.JT | 35 | 12.86 / 8.17 / 5.73 | 14.44 / 9.21 / 6.53 | 19.04 / 12.15 / 8.77 | 36.77 / 23.17 / 17.18 |
| AIM9B.JT | 20 | 8.94 / 4.74 / 3.16 | 10.36 / 5.45 / 3.67 | 14.65 / 7.54 / 5.20 | 17.99 / 11.49 / 7.99 |
| AIM9M.JT | 18 | 15.50 / 8.93 / 5.50 | 15.94 / 9.42 / 5.93 | 16.78 / 10.34 / 6.78 | 18.06 / 11.55 / 8.06 |
| AIM9X.JT | 18 | 15.54 / 8.90 / 5.54 | 15.96 / 9.40 / 5.96 | 16.80 / 10.33 / 6.79 | 18.00 / 11.50 / 8.01 |
| MICA.JT | 60 | 28.63 / 11.75 / 6.40 | 29.82 / 13.41 / 7.52 | 38.18 / 18.59 / 11.22 | 59.98 / 35.43 / 23.76 |
| PL10.JT | 50 | 30.05 / 6.43 / 2.45 | 36.41 / 14.77 / 8.71 | 46.26 / 20.83 / 12.26 | 49.76 / 25.67 / 17.07 |
| PL7.JT | 14 | 14.81 / 8.08 / 4.82 | 14.91 / 8.17 / 4.93 | 15.12 / 8.31 / 5.13 | 15.41 / 8.18 / 5.43 |
| R27ER.JT | 117 | 32.90 / 13.61 / 7.01 | 36.83 / 15.79 / 8.41 | 49.74 / 22.21 / 12.38 | 116.84 / 44.47 / 26.31 |
| R27ET.JT | 104 | 31.64 / 17.08 / 10.97 | 36.79 / 19.81 / 12.73 | 51.91 / 27.78 / 17.87 | 104.13 / 55.45 / 35.85 |
| R27T.JT | 63 | 19.03 / 11.63 / 8.17 | 21.97 / 13.36 / 9.41 | 30.78 / 18.52 / 13.11 | 61.52 / 38.11 / 26.68 |
| R530.JT | 40 | 13.13 / 8.82 / 6.35 | 14.70 / 9.90 / 7.21 | 19.44 / 12.86 / 9.54 | 43.92 / 25.64 / 18.78 |
| R550.JT | 15 | 14.90 / 8.23 / 4.90 | 15.07 / 8.42 / 5.08 | 15.42 / 8.76 / 5.42 | 15.88 / 9.04 / 5.88 |

## Complete A2A seeker-valid reach

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | 26.10 / 11.18 / 5.98 | 27.35 / 12.92 / 7.21 | 35.15 / 18.75 / 11.52 | 64.35 / 38.02 / 25.40 |
| AA11.JT | 11.56 / 9.16 / 6.89 | 11.56 / 10.26 / 7.76 | 11.56 / 13.45 / 10.30 | 11.56 / 15.30 / 17.01 |
| AA11B.JT | 11.56 / 9.16 / 6.89 | 11.56 / 10.26 / 7.76 | 11.56 / 13.45 / 10.30 | 11.56 / 15.30 / 17.01 |
| AA12.JT | 35.42 / 14.99 / 8.73 | 43.45 / 17.38 / 10.20 | 48.14 / 23.09 / 14.03 | 52.04 / 27.77 / 18.86 |
| AA2.JT | 0.00 / 0.00 / 2.20 | 0.00 / 0.00 / 2.53 | 0.00 / 0.00 / 3.53 | 0.00 / 0.00 / 6.76 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 5.78 / 4.13 / 2.13 | 5.78 / 4.26 / 2.25 | 5.78 / 4.41 / 2.49 | 5.78 / 0.00 / 2.83 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 38.11 / 22.14 / 12.11 | 46.40 / 25.61 / 15.59 | 63.57 / 36.25 / 23.23 | 149.76 / 71.64 / 46.80 |
| AEMP1.JT | 41.64 / 16.10 / 8.61 | 45.15 / 18.66 / 10.42 | 47.31 / 22.68 / 13.48 | 50.48 / 26.46 / 17.61 |
| AIM120.JT | 33.42 / 16.66 / 10.28 | 40.92 / 19.33 / 12.05 | 52.70 / 27.23 / 17.28 | 59.98 / 36.41 / 25.72 |
| AIM54C.JT | 13.08 / 6.02 / 1.73 | 56.13 / 21.00 / 11.00 | 65.21 / 29.89 / 16.83 | 144.51 / 68.66 / 40.78 |
| AIM7.JT | 16.11 / 9.10 / 5.88 | 18.19 / 10.45 / 6.89 | 24.27 / 14.23 / 9.71 | 58.45 / 27.82 / 19.63 |
| AIM7E.JT | 12.86 / 8.17 / 5.73 | 14.44 / 9.21 / 6.53 | 19.04 / 12.15 / 8.77 | 36.61 / 23.17 / 17.18 |
| AIM9B.JT | 0.00 / 0.00 / 3.16 | 0.00 / 0.00 / 3.67 | 0.00 / 0.00 / 5.20 | 0.00 / 0.00 / 7.99 |
| AIM9M.JT | 9.64 / 8.91 / 5.50 | 9.64 / 9.40 / 5.93 | 9.64 / 10.32 / 6.78 | 9.64 / 11.54 / 8.06 |
| AIM9X.JT | 9.64 / 8.88 / 5.54 | 9.64 / 9.38 / 5.96 | 9.64 / 10.31 / 6.79 | 9.64 / 11.48 / 8.00 |
| MICA.JT | 28.63 / 11.70 / 6.40 | 29.82 / 13.35 / 7.53 | 38.18 / 18.51 / 11.22 | 59.98 / 35.33 / 23.76 |
| PL10.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| PL7.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| R27ER.JT | 32.90 / 13.61 / 7.01 | 36.83 / 15.79 / 8.41 | 49.74 / 22.21 / 12.38 | 115.07 / 44.47 / 26.31 |
| R27ET.JT | 11.56 / 15.30 / 10.97 | 11.56 / 15.30 / 12.73 | 11.56 / 15.30 / 17.88 | 11.56 / 15.30 / 18.28 |
| R27T.JT | 11.56 / 11.61 / 8.17 | 11.56 / 13.35 / 9.42 | 11.56 / 15.30 / 13.11 | 11.56 / 15.30 / 18.29 |
| R530.JT | 13.13 / 8.82 / 6.35 | 14.70 / 9.90 / 7.21 | 19.44 / 12.86 / 9.54 | 43.88 / 25.64 / 18.78 |
| R550.JT | 9.64 / 8.21 / 4.90 | 9.64 / 8.40 / 5.08 | 9.64 / 8.74 / 5.42 | 9.64 / 9.02 / 5.88 |

## SAM and powered A-G calibration

SAM launcher is stationary at 10 m MSL. The stationary air reference target
is at clamp(0.2*anchor_range_m,300,3000) m, an agent-selected condition.
The additional head/beam/tail cases use a 250 m/s airborne target. A-G launch
is 300 m/s at 1,500 m, except the slow command reference at 30 m/s and 100 m.
Ship strike launch is stationary at 10 m. Ground targets are stationary at
10 m. Distances are horizontal separation. Laser/command point predictions
use the existing 25 ft contact tolerance. Other targets use a 30 ft sphere
plus decoded fuze radius and arming delay. Aircraft launch minimums, radar
acquisition limits and weather remain separate gates.

Four numbers in a SAM cell are head / beam / tail / stationary; A-G has one
stationary value. N/A denotes a held or separate laser/command channel.
Physical-only rows make no seeker-valid claim. AS15/AT12 retain decoded mass
and range references because their real-world identities remain unresolved.
SAM aspect allowance is existing game policy, not a real-world claim.

| Store | Reference km | Launcher/target m | Physical km | Seeker-valid km |
| --- | ---: | --- | --- | --- |
| AGM45.JT | 40 | 1500/10 | 40.00 | 40.00 |
| AGM65A.JT | 27 | 1500/10 | 27.00 | 15.16 |
| AGM65G.JT | 32.18688 | 1500/10 | 32.19 | 18.23 |
| AGM84A.JT | 185 | 1500/10 | 183.07 | 183.07 |
| AGM84E.JT | 93 | 1500/10 | 93.01 | 91.41 |
| AGM88.JT | 48.28032 | 1500/10 | 48.28 | 48.28 |
| AM39.JT | 70 | 1500/10 | 70.00 | 70.00 |
| AS14.JT | 10 | 1500/10 | 10.00 | N/A |
| AS15.JT | 18.288 | 1500/10 | 18.29 | 18.29 |
| AS16.JT | 150 | 1500/10 | 150.00 | 150.00 |
| AS30.JT | 11 | 1500/10 | 11.00 | N/A |
| AS7.JT | 10 | 1500/10 | 10.00 | 10.00 |
| AT12.JT | 7.9248 | 1500/10 | 7.92 | N/A |
| AT2.JT | 4 | 100/10 | 4.26 | N/A |
| FIM92.JT | 4 | 10/800 | 7.19 / 1.69 / 0.58 / 4.00 | 7.19 / 1.68 / 0.58 / 4.00 |
| MIS.JT | 6.5 | 10/1300 | 10.77 / 4.39 / 2.17 / 6.50 | 10.77 / 4.39 / 2.17 / 6.50 |
| R440.JT | 10 | 10/2000 | 14.55 / 8.71 / 5.91 / 10.20 | 14.55 / 8.71 / 5.91 / 10.20 |
| SA13.JT | 5 | 10/1000 | 8.84 / 2.21 / 0.92 / 5.00 | 4.75 / 2.20 / 0.92 / 5.00 |
| SA14.JT | 4.5 | 10/900 | 7.96 / 2.21 / 0.85 / 4.50 | 3.79 / 2.21 / 0.85 / 4.50 |
| SA15.JT | 12 | 10/2400 | 18.27 / 9.60 / 5.83 / 12.00 | 18.27 / 9.60 / 5.83 / 12.00 |
| SA16.JT | 5.2 | 10/1040 | 8.95 / 3.02 / 1.33 / 5.20 | 2.77 / 3.02 / 1.33 / 4.44 |
| SA19.JT | 8 | 10/1600 | 12.38 / 6.37 / 3.66 / 8.00 | 12.38 / 6.37 / 3.66 / 8.00 |
| SA2A.JT | 29 | 10/3000 | 41.96 / 24.33 / 16.05 / 29.01 | 41.96 / 24.33 / 16.05 / 29.01 |
| SA3.JT | 25 | 10/3000 | 36.98 / 20.19 / 13.06 / 25.00 | 36.83 / 20.19 / 13.06 / 25.00 |
| SA6.JT | 24 | 10/3000 | 34.59 / 20.19 / 13.46 / 24.00 | 34.38 / 20.19 / 13.46 / 24.00 |
| SA7.JT | 4.2 | 10/840 | 7.49 / 1.99 / 0.72 / 4.20 | 2.81 / 1.99 / 0.72 / 4.20 |
| SA9.JT | 4.2 | 10/840 | 7.57 / 1.73 / 0.57 / 4.20 | 3.80 / 1.73 / 0.57 / 4.20 |
| SAN11.JT | 8 | 10/1600 | 12.38 / 6.37 / 3.66 / 8.00 | 12.38 / 6.37 / 3.66 / 8.00 |
| SAN3.JT | 30 | 10/3000 | 42.02 / 26.07 / 17.91 / 30.00 | 42.01 / 26.07 / 17.91 / 30.00 |
| SAN4.JT | 10 | 10/2000 | 15.67 / 7.63 / 4.37 / 10.00 | 15.67 / 7.63 / 4.37 / 10.00 |
| SAN7.JT | 25 | 10/3000 | 36.23 / 21.01 / 13.84 / 25.00 | 36.11 / 21.01 / 13.84 / 25.00 |
| SAN9.JT | 12 | 10/2400 | 18.27 / 9.60 / 5.83 / 12.00 | 18.27 / 9.60 / 5.83 / 12.00 |
| SEA_SPAR.JT | 26 | 10/3000 | 37.20 / 22.03 / 14.83 / 26.01 | 37.17 / 22.03 / 14.83 / 26.01 |
| SSN9.JT | 150 | 10/10 | 144.27 | 144.27 |

## Two-condition A2A comparison

H35 is 35,000 ft head-on; T5 is 5,000 ft astern, at the speeds above.
Each pair is H35 / T5, km. The first target is the existing primary reference;
the second is the measured round-2 T5. Per-store fuel, thrust and burn durations
are explicitly fitted game coefficients. Class time and the defeat rule were
not optimized. [All fitted parameters and provenance](../formats/missile-energy-data.md#per-store-motors-and-class-control-budgets)
have one home in the data document.

| Store | Targets H35 / T5 | Round 2 | Phase 2.5 | Phase 2.6 | Residual H35 / T5 |
| --- | ---: | ---: | ---: | ---: | ---: |
| AA10.JT | 73.000 / 6.473 | 73.000 / 6.473 | 73.000 / 12.602 | 64.542 / 7.205 | -11.59% / +11.31% |
| AA11.JT | 30.000 / 7.056 | 30.005 / 7.056 | 29.749 / 12.848 | 27.003 / 7.758 | -9.99% / +9.95% |
| AA11B.JT | 30.000 / 7.056 | 30.005 / 7.056 | 29.749 / 12.848 | 27.003 / 7.758 | -9.99% / +9.95% |
| AA12.JT | 50.000 / 10.767 | 50.000 / 10.767 | 50.000 / 5.558 | 52.037 / 10.195 | +4.07% / -5.31% |
| AA2.JT | 20.000 / 2.180 | 20.013 / 2.180 | 19.994 / 5.603 | 16.765 / 2.532 | -16.17% / +16.15% |
| AA6.JT | 30.000 / 6.811 | 30.005 / 6.811 | 30.062 / 4.398 | 32.761 / 6.181 | +9.20% / -9.25% |
| AA8.JT | 10.000 / 2.377 | 10.014 / 2.377 | 14.045 / 1.596 | 12.829 / 2.250 | +28.29% / -5.34% |
| AA9.JT | 120.000 / 6.816 | 120.000 / 6.816 | 120.000 / 13.244 | 118.134 / 6.867 | -1.55% / +0.75% |
| AAML.JT | 150.000 / 15.620 | 150.000 / 15.620 | 150.000 / 17.109 | 149.763 / 15.580 | -0.16% / -0.26% |
| AEMP1.JT | 50.000 / 10.752 | 50.015 / 10.752 | 50.000 / 5.547 | 50.475 / 10.417 | +0.95% / -3.12% |
| AIM120.JT | 60.000 / 12.058 | 60.017 / 12.058 | 60.000 / 10.555 | 59.985 / 12.050 | -0.02% / -0.07% |
| AIM54C.JT | 150.000 / 9.811 | 150.000 / 9.811 | 150.000 / 17.425 | 144.515 / 10.999 | -3.66% / +12.11% |
| AIM7.JT | 70.000 / 5.792 | 70.000 / 5.792 | 70.000 / 16.254 | 58.577 / 6.890 | -16.32% / +18.96% |
| AIM7E.JT | 35.000 / 8.061 | 35.009 / 8.061 | 34.994 / 3.022 | 36.768 / 6.530 | +5.05% / -18.99% |
| AIM9B.JT | 20.000 / 3.693 | 18.087 / 3.693 | 19.652 / 5.947 | 17.994 / 3.673 | -10.03% / -0.54% |
| AIM9M.JT | 18.000 / 5.938 | 18.024 / 5.938 | 21.363 / 6.028 | 18.060 / 5.935 | +0.33% / -0.05% |
| AIM9X.JT | 18.000 / 5.944 | 18.024 / 5.944 | 21.429 / 6.028 | 18.004 / 5.961 | +0.02% / +0.29% |
| MICA.JT | 60.000 / 7.527 | 60.006 / 7.527 | 60.000 / 9.765 | 59.985 / 7.524 | -0.02% / -0.04% |
| PL10.JT | 50.000 / 8.699 | 50.000 / 8.699 | 50.000 / 5.723 | 49.763 / 8.706 | -0.47% / +0.08% |
| PL7.JT | 14.000 / 5.474 | 14.026 / 5.474 | 13.995 / 1.506 | 15.408 / 4.926 | +10.06% / -10.01% |
| R27ER.JT | 117.000 / 8.402 | 117.000 / 8.402 | 117.000 / 13.700 | 116.838 / 8.413 | -0.14% / +0.13% |
| R27ET.JT | 104.000 / 12.755 | 103.962 / 12.755 | 103.962 / 14.882 | 104.128 / 12.730 | +0.12% / -0.20% |
| R27T.JT | 63.000 / 9.309 | 62.990 / 9.309 | 62.990 / 11.657 | 61.523 / 9.414 | -2.34% / +1.13% |
| R530.JT | 40.000 / 7.997 | 40.011 / 7.997 | 40.004 / 3.013 | 43.916 / 7.214 | +9.79% / -9.79% |
| R550.JT | 15.000 / 5.476 | 15.021 / 5.476 | 14.996 / 3.373 | 15.876 / 5.076 | +5.84% / -7.30% |

Six records need explicit residual exceptions: AA10, AA2, AA8, AIM54C, AIM7
and AIM7E. PL7 is approximately 10% at the rounded boundary. The default
verifier band is 89.5%..110.5% of the reference, with half a percentage point
for sampling tolerance. The exceptions below are bounds, not exemptions.
M/X now use the ordinary band; their former overshoot is no longer accepted.

| Store | Accepted H35/reference | Accepted T5/reference | Reason |
| --- | --- | --- | --- |
| AA10 | 0.86..1.105 | 0.895..1.13 | Best found bounded joint role/family fit |
| AA2 | 0.82..1.105 | 0.895..1.18 | Best found bounded two-condition fit |
| AA8 | 0.895..1.30 | 0.895..1.105 | Best found bounded fit with low tail floor |
| AIM54C | 0.895..1.105 | 0.895..1.14 | 1.2 times decoded minimum is 10.9728 km, above old 9.811 km tail |
| AIM7 | 0.82..1.105 | 0.895..1.21 | Conflicting tail targets plus joint ordering |
| AIM7E | 0.895..1.105 | 0.79..1.105 | Conflicting tail targets plus joint ordering |

Round-2 Sparrow tails were 5.792 km for AIM7 and 8.061 km for AIM7E.
Even their 10% bands do not overlap in generation order. Equal relative
error meets around 6.74 km before the other constraints. John's conditional
permission for a real-world-supported reversal does not establish that these
game-fit targets are real-world equivalents, so the generation check remains.
The other search residuals are best found within the documented search bounds,
not proofs that no better fit exists. No per-store timer or geometry drag
factor was introduced to hide a residual.

## SAM and A-G before/after

The reference condition for each row is the stationary target and stated
heights in the preceding SAM/A-G table. All 34 ground-role records are shown.
Class assignment is explicit data. AS7/AS14/AS30/AT12 share ShortGround 30 s;
AS16 uses the existing 600 s long-ground budget, retaining normal drag.
These agent assignments use the lead's explicit permission; the class times
were fixed before motor fitting. The original nine class values remain.

| Store | Class / control s | Reference km | Round 2 km | Phase 2.5 km | Phase 2.6 km | Residual |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| AGM45.JT | FastGround / 90 | 40.000 | 39.998 | 42.735 | 40.000 | +0.00% |
| AGM65A.JT | FastGround / 90 | 27.000 | 26.998 | 27.387 | 27.000 | +0.00% |
| AGM65G.JT | FastGround / 90 | 32.187 | 32.186 | 33.811 | 32.187 | +0.00% |
| AGM84A.JT | Cruise / 600 | 185.000 | 184.953 | 189.788 | 183.074 | -1.04% |
| AGM84E.JT | Cruise / 600 | 93.000 | 93.010 | 173.696 | 93.011 | +0.01% |
| AGM88.JT | FastGround / 90 | 48.280 | 48.280 | 49.199 | 48.280 | -0.00% |
| AM39.JT | Cruise / 600 | 70.000 | 69.982 | 78.770 | 70.000 | +0.00% |
| AS14.JT | ShortGround / 30 | 10.000 | 10.010 | 33.726 | 10.000 | +0.00% |
| AS15.JT | FastGround / 90 | 18.288 | 18.296 | 39.043 | 18.290 | +0.01% |
| AS16.JT | Cruise / 600 | 150.000 | 149.954 | 76.236 | 150.000 | +0.00% |
| AS30.JT | ShortGround / 30 | 11.000 | 11.011 | 34.610 | 10.998 | -0.02% |
| AS7.JT | ShortGround / 30 | 10.000 | 10.006 | 22.702 | 10.000 | +0.00% |
| AT12.JT | ShortGround / 30 | 7.925 | 7.943 | 53.390 | 7.923 | -0.02% |
| AT2.JT | Command / 30 | 4.000 | 4.008 | 9.102 | 4.260 | +6.50% |
| FIM92.JT | SurfaceIr / 25 | 4.000 | 4.010 | 7.898 | 4.000 | +0.00% |
| MIS.JT | SurfaceRadar / 70 | 6.500 | 6.507 | 7.014 | 6.500 | +0.00% |
| R440.JT | SurfaceRadar / 70 | 10.000 | 10.014 | 16.161 | 10.198 | +1.98% |
| SA13.JT | SurfaceIr / 25 | 5.000 | 5.014 | 11.881 | 5.000 | +0.00% |
| SA14.JT | SurfaceIr / 25 | 4.500 | 4.511 | 7.445 | 4.500 | +0.00% |
| SA15.JT | SurfaceRadar / 70 | 12.000 | 12.003 | 10.158 | 12.000 | +0.00% |
| SA16.JT | SurfaceIr / 25 | 5.200 | 5.211 | 7.112 | 5.201 | +0.02% |
| SA19.JT | SurfaceRadar / 70 | 8.000 | 8.002 | 5.607 | 8.001 | +0.01% |
| SA2A.JT | SurfaceRadarLong / 120 | 29.000 | 29.003 | 18.214 | 29.006 | +0.02% |
| SA3.JT | SurfaceRadarLong / 120 | 25.000 | 25.002 | 16.332 | 25.000 | +0.00% |
| SA6.JT | SurfaceRadar / 70 | 24.000 | 23.995 | 15.065 | 24.000 | +0.00% |
| SA7.JT | SurfaceIr / 25 | 4.200 | 4.208 | 6.713 | 4.202 | +0.05% |
| SA9.JT | SurfaceIr / 25 | 4.200 | 4.210 | 7.768 | 4.200 | +0.00% |
| SAN11.JT | SurfaceRadar / 70 | 8.000 | 8.002 | 5.607 | 8.001 | +0.01% |
| SAN3.JT | SurfaceRadarLong / 120 | 30.000 | 29.991 | 18.645 | 30.000 | +0.00% |
| SAN4.JT | SurfaceRadar / 70 | 10.000 | 10.006 | 8.814 | 10.000 | +0.00% |
| SAN7.JT | SurfaceRadarLong / 120 | 25.000 | 25.002 | 15.540 | 25.004 | +0.02% |
| SAN9.JT | SurfaceRadar / 70 | 12.000 | 12.003 | 10.158 | 12.000 | +0.00% |
| SEA_SPAR.JT | SurfaceRadarLong / 120 | 26.000 | 26.000 | 18.100 | 26.006 | +0.02% |
| SSN9.JT | Cruise / 600 | 150.000 | 149.954 | 153.634 | 144.274 | -3.82% |

## Family, role and altitude checks

Family margins use all twelve altitude/aspect cells. Short-IR comparisons
use the three 5k/15k/35k tail cells. Low-altitude radar role comparisons use
5k head and tail. These checks do not assert universal real-world ordering.

| Newer / older | Minimum measured advantage |
| --- | ---: |
| AIM7.JT / AIM7E.JT | 2.529% |
| R27ER.JT / AA10.JT | 3.594% |
| R27ET.JT / R27T.JT | 34.280% |
| AIM9M.JT / AIM9B.JT | 0.838% |
| AIM9M.JT / AA2.JT | 19.193% |
| AIM9M.JT / AA8.JT | 163.778% |
| AIM9X.JT / AIM9B.JT | 0.138% |
| AIM9X.JT / AA2.JT | 18.365% |
| AIM9X.JT / AA8.JT | 164.933% |
| AA11.JT / AIM9B.JT | 98.287% |
| AA11.JT / AA2.JT | 151.427% |
| AA11.JT / AA8.JT | 244.800% |

| Active radar / older group | Smallest 5k head margin km | Smallest 5k tail margin km |
| --- | ---: | ---: |
| AIM120.JT | 13.567 | 4.836 |
| AA12.JT | 16.090 | 2.981 |
| MICA.JT | 2.474 | 0.310 |

| Store | H5/H35 | T5/H35 | T5 class floor km | 1.2 x decoded min km |
| --- | ---: | ---: | ---: | ---: |
| AA10.JT | 0.424 | 0.112 | 3 | 4.389 |
| AA11.JT | 0.541 | 0.287 | 1.5 | 0.732 |
| AA11B.JT | 0.541 | 0.287 | 1.5 | 0.549 |
| AA12.JT | 0.835 | 0.196 | 3 | 4.389 |
| AA2.JT | 0.422 | 0.151 | 1.5 | 1.463 |
| AA6.JT | 0.394 | 0.189 | 3 | 4.389 |
| AA8.JT | 0.955 | 0.175 | 1.5 | 1.463 |
| AA9.JT | 0.469 | 0.058 | 5 | 4.389 |
| AAML.JT | 0.310 | 0.104 | 5 | 10.973 |
| AEMP1.JT | 0.894 | 0.206 | 3 | 0.732 |
| AIM120.JT | 0.682 | 0.201 | 3 | 4.389 |
| AIM54C.JT | 0.388 | 0.076 | 5 | 10.973 |
| AIM7.JT | 0.310 | 0.118 | 3 | 2.926 |
| AIM7E.JT | 0.393 | 0.178 | 3 | 1.097 |
| AIM9B.JT | 0.576 | 0.204 | 1.5 | 1.463 |
| AIM9M.JT | 0.883 | 0.329 | 1.5 | 1.463 |
| AIM9X.JT | 0.887 | 0.331 | 1.5 | 1.097 |
| MICA.JT | 0.497 | 0.125 | 3 | 4.389 |
| PL10.JT | 0.732 | 0.175 | 3 | 2.926 |
| PL7.JT | 0.968 | 0.320 | 1.5 | 1.463 |
| R27ER.JT | 0.315 | 0.072 | 5 | 4.389 |
| R27ET.JT | 0.353 | 0.122 | 5 | 4.389 |
| R27T.JT | 0.357 | 0.153 | 3 | 4.389 |
| R530.JT | 0.335 | 0.164 | 3 | 2.926 |
| R550.JT | 0.949 | 0.320 | 1.5 | 1.463 |

The decoded minimum removes these nominal physical employment intervals:

- AA9.JT 500 ft tail: 1.455 km <= minimum 3.658 km
- AIM54C.JT 500 ft beam: 6.017 km <= minimum 9.144 km
- AIM54C.JT 500 ft tail: 1.729 km <= minimum 9.144 km

These are range-policy exclusions, not zero kinetic reach. The floor check
uses 5k tail, not the 500 ft deck. The one altitude reversal above 10% is
AA8 beam: 4.493 km at 15k versus 3.375 km at 35k. Both boundary trials reach
the 20-second control deadline. Diagnostic available G at the edge falls
from 18.49 to 8.37, while maximum requested G deficit rises from 21.30 to
29.93. This is consistent with a high-altitude turning limitation under the
fixed deadline. The live-seeker sweep finds no valid AA8 35k beam intercept;
that zero is retained as a measured limitation, not replaced with physical
reach. AIM9B/AA2 head and beam zeros are their rear-only seeker restrictions.
PL7 and R550 H5/H35 ratios are 0.968 and 0.949, above the earlier 0.90 warning
band. Their long fitted burns consume most of the fixed 20-second budget,
so these cells show little extra range with height. The target residuals and
full cells remain visible above; warning ratios are not reference-fit gates.

## Defeat and force diagnostics

The offline `--diagnostics` mode opts into available/required G, positive
G deficit, closing speed, Mach, control expiry and post-burn defeat at each
measured edge and 10% beyond. These values do not create a G-based death rule.
Raw commanded G can exceed available G; applied steering stays bounded.
Ordinary motion and prediction skip diagnostic-only calculations.
The behaviour tripwire includes low-Mach countdown, reset, defeat transition
and inert contacts. Dedicated tests cover the separate warning publication rule.

## Validation status

Source change `256fd274`, Linux x86-64. Model identity 1976937061122229260, behaviour trace
10493902561014387883. The final complete workspace run passes 7,219 tests,
with 106 intentionally ignored. The separate example overshoot-gate test
also passes. Workspace build, clippy with warnings denied, formatting,
documentation, 518 Python tests (five skipped), and source/app/extractor asset
checks pass. Initial new-fixture failures and the old golden pair were corrected
before this clean final run.

All 59 reference checks pass their declared bands, including the six named
A2A compromises. Tail floors, decoded-minimum margins, low-altitude radar
roles, family orderings and 12 km modern-IR deck escape checks pass. The largest
reach change under a 2% parameter perturbation is 4.265%.
All 391 physical/seeker rows and 668 edge/beyond diagnostics were regenerated.
The offline seeker probe now rejects contact on the defeat tick, matching the
live inert-damage rule; this changed 96 sampled rows. Diagnostic CSV
headers now include both final ground/life flags.

All 36 scenario checks pass across the full battery and one targeted rerun.
The full run passed 35/36; UKR's 15 launches exceeded the old symmetric
pre-overhaul upper limit of 14.3. John then approved using the reviewed Phase
2.5 baseline and retaining a separate one-sided Spec floor. The revised
five-fight check passes its targeted rerun; the other 35 passed unchanged.
This includes SA2A/SA3/SA6 moving-jet hits at both 10k and 20k ft, the extended
inert near-pass case, the 240-shot Energy/worker comparison and eight historical
Spec shots. The 32 long defeat shots record
6 defeats and 10 hits, with no hit after defeat.
Both added near-pass rows are repeated serially and with four workers; 2
pass within their target's fuze/contact radius with zero inert damage.
The near-pass seed 0 reaches 0.240 ft versus radius 128.000 ft, with defeat at 50.058 s.
The near-pass seed 1 reaches 0.240 ft versus radius 128.000 ft, with defeat at 50.058 s.

The same-tick threat tests establish a live warning first, then clear it on
kinetic defeat and passed-decoy state while keeping the body. AI defense ends
without a stale-record grace period; render/readout and RWR-tone tests cover
cue removal. Historical Spec/Compatibility remain distinct.

Windows/macOS were not run on this Linux host. Retail comparison remains
unavailable. The six residual exceptions, AA8 high-altitude beam limitation
and deck minimum exclusions above remain explicit approximations. Phase 3
stays stopped; its earlier pre-fix study is still invalid.

## Natural launch counts against both baselines

John's Phase 2.6 decision retains a one-sided 70% floor against pre-overhaul
Spec, plus a symmetric 70%..130% band against the reviewed Phase 2.5 model.
The earlier symmetric Spec ceiling is no longer an acceptance gate. No AI
firing policy or runtime admission limit changed to satisfy these counts.
The table uses the independent sequential cost lanes; the targeted battery
rerun reports identical counts and zero refused/starved requests in all five.

| Fight | Pre-overhaul Spec | Phase 2.5 | Phase 2.6 | Change vs Spec | Change vs 2.5 | Spec floor | 2.5 band |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| UKR | 11 | 13 | 15 | +36.36% | +15.38% | 7.7 | 9.1..16.9 |
| BAL | 12 | 13 | 15 | +25.00% | +15.38% | 8.4 | 9.1..16.9 |
| SPA | 12 | 13 | 15 | +25.00% | +15.38% | 8.4 | 9.1..16.9 |
| GRE | 12 | 13 | 15 | +25.00% | +15.38% | 8.4 | 9.1..16.9 |
| 23-AI UKR | 24 | 26 | 30 | +25.00% | +15.38% | 16.8 | 18.2..33.8 |

## Prediction cost

Release examples, imported profile, final model above. Every heavy run checks
available memory first, uses nice 15 and CARGO_BUILD_JOBS=4, and runs serially
with other heavy commands. Rust tests use four threads. The battery uses two
jobs and one isolated window through tools/agent-run.sh; its wrapper also
renices the application to 15.

Natural UKR/BAL/SPA/GRE fights run 14,400 ticks with eight AI aircraft; the
large fight runs 7,200 ticks with 23 AI aircraft. Turning-player and hold lanes
run 2,400 ticks; hold lanes request eight stations per aircraft. Cold is the
first 240 ticks. Steady is the remaining moving/frozen ticks. Wall timings
include scheduling delay; CPU timings measure prediction work. These are
host measurements, not cross-platform guarantees.

| Lane | Cold p50 / p95 ms | Steady p50 / p95 ms | Largest CPU p95 ms | Refused / starved | Guided launches |
| --- | ---: | ---: | ---: | ---: | ---: |
| natural-ukr | 0.811 / 0.928 | 0.024 / 0.894 | 0.924 | 0 / 0 | 15 |
| natural-bal | 0.817 / 0.934 | 0.033 / 0.882 | 0.932 | 0 / 0 | 15 |
| natural-spa | 0.821 / 0.926 | 0.034 / 0.894 | 0.925 | 0 / 0 | 15 |
| natural-gre | 0.824 / 0.931 | 0.032 / 0.884 | 0.928 | 0 / 0 | 15 |
| natural-big | 0.893 / 0.941 | 0.859 / 0.948 | 0.945 | 0 / 0 | 30 |
| turning-3 | 0.889 / 0.950 | 0.901 / 0.943 | 0.949 | 0 / 0 | 14 |
| turning-10 | 0.890 / 0.931 | 0.896 / 0.930 | 0.929 | 0 / 0 | 5 |
| hold-frozen | 1.023 / 1.095 | 0.080 / 1.075 | 1.091 | 0 / 0 | 33 |
| hold-moving | 1.008 / 1.020 | 0.892 / 1.021 | 1.018 | 0 / 0 | 33 |
| hold-cruise | 0.778 / 0.835 | 0.798 / 0.870 | 0.867 | 0 / 0 | 16 |

Largest cold/steady p95 is 1.094577 ms wall and 1.091497 ms CPU, below
the 2 ms gate. Largest single wall sample is 1.266068 ms.
No lane reports admission refusal or starvation. Maximum admission wait is
0 ticks. Cached-reader p95 is at most 0.015730 ms.

turning-3: player advice is qualified on 1,570/1,570 eligible-observation ticks (1,262 positive, 308 negative, 0 pending).

turning-10: player advice is qualified on 660/661 eligible-observation ticks (660 positive, 0 negative, 1 pending).

The denominator counts eligible observations, not all simulated ticks. Strict
AI qualification remains separate from projected player advice.

hold-frozen: 149,360 successful qualified cached reads.

hold-moving: 16,048 successful qualified cached reads.

hold-cruise: 0 successful qualified cached reads.

The 20-second cruise hold demonstrates bounded cost, not completed-answer
availability. This retained limitation is explicit; no synchronous fallback
or relaxed support gate was introduced. Ground-target live reach/support is
covered separately by A-G battery cases and the range tables.
