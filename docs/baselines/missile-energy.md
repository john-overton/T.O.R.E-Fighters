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
motor shape, powered cruise arrival, finite shots and local parameter
sensitivity. Neither changes source or runs during gameplay.

This baseline contains Phase 2.7 guarded motor fits. Same-tick inert threat
removal is unchanged.
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
successful interval ten times. Empty envelopes retry with factor-1.1 probes,
which recovered narrow AA8 beam intervals. It can still miss a
narrow disconnected island of success. It is not a mathematical proof of a
continuous no-escape envelope. Fitting and regression diagnostics
use twelve bisections instead of ten, so their last reported decimals can
differ slightly from the complete sweep without a physics change. AA11 and
AA11B illustrate the boundary: their twelve-bisection H35 is 26.851 km,
inside the 26.850 km lower gate, while the ten-bisection table reads 26.846 km.
Acceptance uses the verifier precision, not the rounded display table.

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
| AA10.JT | 73 | 34.44 / 11.98 / 5.85 | 35.87 / 13.57 / 6.92 | 44.62 / 21.54 / 12.26 | 67.87 / 43.25 / 29.29 |
| AA11.JT | 30 | 13.37 / 9.24 / 6.88 | 15.04 / 10.39 / 7.79 | 20.05 / 13.78 / 10.47 | 26.85 / 21.07 / 16.85 |
| AA11B.JT | 30 | 13.37 / 9.24 / 6.88 | 15.04 / 10.39 / 7.79 | 20.05 / 13.78 / 10.47 | 26.85 / 21.07 / 16.85 |
| AA12.JT | 50 | 43.69 / 18.16 / 9.47 | 44.84 / 19.93 / 10.77 | 46.97 / 22.63 / 13.26 | 50.03 / 26.25 / 17.24 |
| AA2.JT | 20 | 6.13 / 3.27 / 2.18 | 7.06 / 3.74 / 2.51 | 9.85 / 5.14 / 3.50 | 16.69 / 9.94 / 6.69 |
| AA6.JT | 30 | 11.76 / 7.68 / 5.46 | 13.12 / 8.61 / 6.18 | 17.16 / 11.30 / 8.26 | 32.84 / 21.45 / 16.13 |
| AA8.JT | 10 | 12.10 / 3.83 / 2.09 | 12.25 / 4.01 / 2.25 | 12.57 / 4.37 / 2.56 | 12.99 / 3.58 / 2.98 |
| AA9.JT | 120 | 76.39 / 6.68 / 3.21 | 78.41 / 12.06 / 6.82 | 82.27 / 18.59 / 10.41 | 120.00 / 42.63 / 24.67 |
| AAML.JT | 150 | 36.56 / 22.16 / 13.00 | 49.33 / 26.04 / 15.62 | 67.41 / 36.84 / 23.20 | 150.00 / 76.20 / 48.73 |
| AEMP1.JT | 50 | 43.71 / 16.89 / 8.98 | 44.87 / 19.80 / 10.77 | 47.00 / 22.70 / 13.29 | 50.02 / 26.30 / 17.26 |
| AIM120.JT | 60 | 37.74 / 17.44 / 10.22 | 45.26 / 20.08 / 12.06 | 52.44 / 27.68 / 17.30 | 60.00 / 36.68 / 25.87 |
| AIM54C.JT | 150 | 12.41 / 6.28 / 2.39 | 52.09 / 21.90 / 11.00 | 65.71 / 32.39 / 18.16 | 150.00 / 70.82 / 43.00 |
| AIM7.JT | 70 | 16.11 / 9.10 / 5.88 | 18.19 / 10.45 / 6.89 | 24.27 / 14.23 / 9.71 | 58.58 / 27.82 / 19.63 |
| AIM7E.JT | 35 | 12.86 / 8.17 / 5.73 | 14.44 / 9.21 / 6.53 | 19.04 / 12.15 / 8.77 | 36.77 / 23.17 / 17.18 |
| AIM9B.JT | 20 | 8.94 / 4.74 / 3.16 | 10.36 / 5.45 / 3.67 | 14.65 / 7.54 / 5.20 | 17.99 / 11.49 / 7.99 |
| AIM9M.JT | 18 | 15.53 / 8.80 / 5.52 | 15.95 / 9.32 / 5.95 | 16.80 / 10.29 / 6.79 | 18.01 / 11.50 / 8.01 |
| AIM9X.JT | 18 | 15.21 / 8.33 / 5.21 | 15.77 / 9.10 / 5.76 | 16.96 / 10.50 / 6.96 | 18.63 / 12.24 / 8.63 |
| MICA.JT | 60 | 40.34 / 12.14 / 6.53 | 41.36 / 13.75 / 7.53 | 46.30 / 18.85 / 10.76 | 55.24 / 31.28 / 20.59 |
| PL10.JT | 50 | 38.38 / 6.63 / 3.01 | 39.24 / 15.62 / 8.70 | 45.92 / 20.49 / 11.81 | 50.08 / 26.03 / 17.23 |
| PL7.JT | 14 | 14.61 / 7.87 / 4.62 | 14.81 / 8.08 / 4.81 | 15.18 / 8.47 / 5.19 | 15.69 / 8.80 / 5.70 |
| R27ER.JT | 117 | 48.93 / 15.05 / 7.38 | 49.98 / 16.62 / 8.40 | 56.68 / 24.23 / 13.35 | 117.00 / 57.57 / 37.54 |
| R27ET.JT | 104 | 31.64 / 17.08 / 10.97 | 36.79 / 19.81 / 12.73 | 51.91 / 27.78 / 17.87 | 104.13 / 55.45 / 35.85 |
| R27T.JT | 63 | 19.03 / 11.63 / 8.17 | 21.97 / 13.36 / 9.41 | 30.78 / 18.52 / 13.11 | 61.52 / 38.11 / 26.68 |
| R530.JT | 40 | 13.13 / 8.82 / 6.35 | 14.70 / 9.90 / 7.21 | 19.44 / 12.86 / 9.54 | 43.92 / 25.64 / 18.78 |
| R550.JT | 15 | 14.86 / 8.08 / 4.86 | 15.08 / 8.34 / 5.08 | 15.51 / 8.81 / 5.51 | 16.11 / 9.32 / 6.11 |

## Complete A2A seeker-valid reach

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | 34.44 / 11.98 / 5.85 | 35.87 / 13.57 / 6.92 | 44.59 / 21.54 / 12.26 | 67.68 / 43.00 / 29.29 |
| AA11.JT | 11.56 / 9.22 / 6.88 | 11.56 / 10.38 / 7.79 | 11.56 / 13.77 / 10.47 | 11.56 / 15.30 / 16.85 |
| AA11B.JT | 11.56 / 9.22 / 6.88 | 11.56 / 10.38 / 7.79 | 11.56 / 13.77 / 10.47 | 11.56 / 15.30 / 16.85 |
| AA12.JT | 43.69 / 18.07 / 9.47 | 44.84 / 19.85 / 10.77 | 46.97 / 22.57 / 13.26 | 50.03 / 26.19 / 17.24 |
| AA2.JT | 0.00 / 0.00 / 2.18 | 0.00 / 0.00 / 2.51 | 0.00 / 0.00 / 3.50 | 0.00 / 0.00 / 6.69 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 5.78 / 3.76 / 2.09 | 5.78 / 3.93 / 2.25 | 5.78 / 4.28 / 2.56 | 5.78 / 0.00 / 2.98 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 36.56 / 22.11 / 13.00 | 49.33 / 25.99 / 15.62 | 67.41 / 36.79 / 23.20 | 150.00 / 76.07 / 48.73 |
| AEMP1.JT | 43.71 / 16.66 / 8.98 | 44.87 / 19.52 / 10.77 | 47.00 / 22.55 / 13.29 | 50.02 / 26.15 / 17.26 |
| AIM120.JT | 37.74 / 17.35 / 10.22 | 45.28 / 19.98 / 12.06 | 52.44 / 27.59 / 17.30 | 59.98 / 36.61 / 25.87 |
| AIM54C.JT | 12.41 / 6.28 / 2.39 | 52.09 / 21.87 / 11.00 | 65.71 / 32.37 / 18.16 | 150.00 / 70.76 / 43.01 |
| AIM7.JT | 16.11 / 9.10 / 5.88 | 18.19 / 10.45 / 6.89 | 24.27 / 14.23 / 9.71 | 58.45 / 27.82 / 19.63 |
| AIM7E.JT | 12.86 / 8.17 / 5.73 | 14.44 / 9.21 / 6.53 | 19.04 / 12.15 / 8.77 | 36.61 / 23.17 / 17.18 |
| AIM9B.JT | 0.00 / 0.00 / 3.16 | 0.00 / 0.00 / 3.67 | 0.00 / 0.00 / 5.20 | 0.00 / 0.00 / 7.99 |
| AIM9M.JT | 9.64 / 8.78 / 5.52 | 9.64 / 9.29 / 5.95 | 9.64 / 10.26 / 6.79 | 9.64 / 11.48 / 8.01 |
| AIM9X.JT | 9.64 / 8.30 / 5.21 | 9.64 / 9.08 / 5.76 | 9.64 / 10.48 / 6.96 | 9.64 / 12.23 / 8.63 |
| MICA.JT | 40.34 / 12.05 / 6.53 | 41.36 / 13.65 / 7.53 | 46.30 / 18.72 / 10.76 | 55.24 / 31.20 / 20.59 |
| PL10.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| PL7.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| R27ER.JT | 48.93 / 15.05 / 7.38 | 49.98 / 16.62 / 8.40 | 56.68 / 24.23 / 13.35 | 115.20 / 57.52 / 37.54 |
| R27ET.JT | 11.56 / 15.30 / 10.97 | 11.56 / 15.30 / 12.73 | 11.56 / 15.30 / 17.88 | 11.56 / 15.30 / 18.28 |
| R27T.JT | 11.56 / 11.61 / 8.17 | 11.56 / 13.35 / 9.42 | 11.56 / 15.30 / 13.11 | 11.56 / 15.30 / 18.29 |
| R530.JT | 13.13 / 8.82 / 6.35 | 14.70 / 9.90 / 7.21 | 19.44 / 12.86 / 9.54 | 43.88 / 25.64 / 18.78 |
| R550.JT | 9.64 / 8.05 / 4.86 | 9.64 / 8.31 / 5.08 | 9.64 / 8.79 / 5.51 | 9.64 / 9.30 / 6.11 |

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
| AGM84A.JT | 185 | 1500/10 | 186.11 | 186.11 |
| AGM84E.JT | 93 | 1500/10 | 99.17 | 91.41 |
| AGM88.JT | 48.28032 | 1500/10 | 48.28 | 48.28 |
| AM39.JT | 70 | 1500/10 | 76.27 | 76.27 |
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
| SSN9.JT | 150 | 10/10 | 151.12 | 151.12 |

## Two-condition A2A comparison

H35 is 35,000 ft head-on; T5 is 5,000 ft astern, at the speeds above.
Each pair is H35 / T5, km. The first target is the existing primary reference;
the second is the measured round-2 T5. Per-store fuel, thrust and burn durations
are explicitly fitted game coefficients. Class time and the defeat rule were
not optimized. [All fitted parameters and provenance](../formats/missile-energy-data.md#per-store-motors-and-class-control-budgets)
have one home in the data document.

| Store | Targets H35 / T5 | Round 2 | Phase 2.5 | Phase 2.6 | Phase 2.7 | Residual H35 / T5 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| AA10.JT | 73.000 / 6.473 | 73.000 / 6.473 | 73.000 / 12.602 | 64.542 / 7.205 | 67.870 / 6.915 | -7.03% / +6.83% |
| AA11.JT | 30.000 / 7.056 | 30.005 / 7.056 | 29.749 / 12.848 | 27.003 / 7.758 | 26.846 / 7.794 | -10.51% / +10.46% |
| AA11B.JT | 30.000 / 7.056 | 30.005 / 7.056 | 29.749 / 12.848 | 27.003 / 7.758 | 26.846 / 7.794 | -10.51% / +10.46% |
| AA12.JT | 50.000 / 10.767 | 50.000 / 10.767 | 50.000 / 5.558 | 52.037 / 10.195 | 50.031 / 10.767 | +0.06% / +0.00% |
| AA2.JT | 20.000 / 2.180 | 20.013 / 2.180 | 19.994 / 5.603 | 16.765 / 2.532 | 16.691 / 2.509 | -16.55% / +15.09% |
| AA6.JT | 30.000 / 6.811 | 30.005 / 6.811 | 30.062 / 4.398 | 32.761 / 6.181 | 32.841 / 6.176 | +9.47% / -9.32% |
| AA8.JT | 10.000 / 2.377 | 10.014 / 2.377 | 14.045 / 1.596 | 12.829 / 2.250 | 12.987 / 2.246 | +29.87% / -5.51% |
| AA9.JT | 120.000 / 6.816 | 120.000 / 6.816 | 120.000 / 13.244 | 118.134 / 6.867 | 120.000 / 6.816 | +0.00% / +0.00% |
| AAML.JT | 150.000 / 15.620 | 150.000 / 15.620 | 150.000 / 17.109 | 149.763 / 15.580 | 150.000 / 15.620 | +0.00% / +0.00% |
| AEMP1.JT | 50.000 / 10.752 | 50.015 / 10.752 | 50.000 / 5.547 | 50.475 / 10.417 | 50.015 / 10.767 | +0.03% / +0.14% |
| AIM120.JT | 60.000 / 12.058 | 60.017 / 12.058 | 60.000 / 10.555 | 59.985 / 12.050 | 60.000 / 12.058 | +0.00% / +0.00% |
| AIM54C.JT | 150.000 / 9.811 | 150.000 / 9.811 | 150.000 / 17.425 | 144.515 / 10.999 | 150.000 / 10.999 | +0.00% / +12.11% |
| AIM7.JT | 70.000 / 5.792 | 70.000 / 5.792 | 70.000 / 16.254 | 58.577 / 6.890 | 58.577 / 6.890 | -16.32% / +18.96% |
| AIM7E.JT | 35.000 / 8.061 | 35.009 / 8.061 | 34.994 / 3.022 | 36.768 / 6.530 | 36.768 / 6.530 | +5.05% / -18.99% |
| AIM9B.JT | 20.000 / 3.693 | 18.087 / 3.693 | 19.652 / 5.947 | 17.994 / 3.673 | 17.994 / 3.673 | -10.03% / -0.54% |
| AIM9M.JT | 18.000 / 5.938 | 18.024 / 5.938 | 21.363 / 6.028 | 18.060 / 5.935 | 18.010 / 5.947 | +0.06% / +0.15% |
| AIM9X.JT | 18.000 / 5.944 | 18.024 / 5.944 | 21.429 / 6.028 | 18.004 / 5.961 | 18.631 / 5.765 | +3.51% / -3.01% |
| MICA.JT | 60.000 / 7.527 | 60.006 / 7.527 | 60.000 / 9.765 | 59.985 / 7.524 | 55.239 / 7.527 | -7.94% / +0.00% |
| PL10.JT | 50.000 / 8.699 | 50.000 / 8.699 | 50.000 / 5.723 | 49.763 / 8.706 | 50.077 / 8.699 | +0.15% / +0.00% |
| PL7.JT | 14.000 / 5.474 | 14.026 / 5.474 | 13.995 / 1.506 | 15.408 / 4.926 | 15.691 / 4.811 | +12.08% / -12.11% |
| R27ER.JT | 117.000 / 8.402 | 117.000 / 8.402 | 117.000 / 13.700 | 116.838 / 8.413 | 117.000 / 8.402 | +0.00% / +0.00% |
| R27ET.JT | 104.000 / 12.755 | 103.962 / 12.755 | 103.962 / 14.882 | 104.128 / 12.730 | 104.128 / 12.730 | +0.12% / -0.20% |
| R27T.JT | 63.000 / 9.309 | 62.990 / 9.309 | 62.990 / 11.657 | 61.523 / 9.414 | 61.523 / 9.414 | -2.34% / +1.13% |
| R530.JT | 40.000 / 7.997 | 40.011 / 7.997 | 40.004 / 3.013 | 43.916 / 7.214 | 43.916 / 7.214 | +9.79% / -9.79% |
| R550.JT | 15.000 / 5.476 | 15.021 / 5.476 | 14.996 / 3.373 | 15.876 / 5.076 | 16.111 / 5.078 | +7.41% / -7.27% |

Seven records have explicit residual exceptions: AA10, AA2, AA8, AIM54C, AIM7,
AIM7E and PL7. John approved the PL7 +/-12.5% exception during Phase 2.7
after the bounded guarded search missed the former band. The default
verifier band is 89.5%..110.5% of the reference, with half a percentage point
for sampling tolerance. The exceptions below are bounds, not exemptions.
M/X now use the ordinary band; their former overshoot is no longer accepted.

| Store | Accepted H35/reference | Accepted T5/reference | Reason |
| --- | --- | --- | --- |
| PL7 | 0.875..1.125 | 0.875..1.125 | John-approved guarded motor compromise |
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

| Store | Class / control s | Reference km | Round 2 km | Phase 2.5 km | Phase 2.6 km | Phase 2.7 km | Residual |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| AGM45.JT | FastGround / 90 | 40.000 | 39.998 | 42.735 | 40.000 | 40.000 | +0.00% |
| AGM65A.JT | FastGround / 90 | 27.000 | 26.998 | 27.387 | 27.000 | 27.000 | +0.00% |
| AGM65G.JT | FastGround / 90 | 32.187 | 32.186 | 33.811 | 32.187 | 32.187 | +0.00% |
| AGM84A.JT | Cruise / 600 | 185.000 | 184.953 | 189.788 | 183.074 | 186.111 | +0.60% |
| AGM84E.JT | Cruise / 600 | 93.000 | 93.010 | 173.696 | 93.011 | 99.166 | +6.63% |
| AGM88.JT | FastGround / 90 | 48.280 | 48.280 | 49.199 | 48.280 | 48.280 | -0.00% |
| AM39.JT | Cruise / 600 | 70.000 | 69.982 | 78.770 | 70.000 | 76.268 | +8.95% |
| AS14.JT | ShortGround / 30 | 10.000 | 10.010 | 33.726 | 10.000 | 10.000 | +0.00% |
| AS15.JT | FastGround / 90 | 18.288 | 18.296 | 39.043 | 18.290 | 18.290 | +0.01% |
| AS16.JT | Cruise / 600 | 150.000 | 149.954 | 76.236 | 150.000 | 150.000 | +0.00% |
| AS30.JT | ShortGround / 30 | 11.000 | 11.011 | 34.610 | 10.998 | 10.998 | -0.02% |
| AS7.JT | ShortGround / 30 | 10.000 | 10.006 | 22.702 | 10.000 | 10.000 | +0.00% |
| AT12.JT | ShortGround / 30 | 7.925 | 7.943 | 53.390 | 7.923 | 7.923 | -0.02% |
| AT2.JT | Command / 30 | 4.000 | 4.008 | 9.102 | 4.260 | 4.260 | +6.50% |
| FIM92.JT | SurfaceIr / 25 | 4.000 | 4.010 | 7.898 | 4.000 | 4.000 | +0.00% |
| MIS.JT | SurfaceRadar / 70 | 6.500 | 6.507 | 7.014 | 6.500 | 6.500 | +0.00% |
| R440.JT | SurfaceRadar / 70 | 10.000 | 10.014 | 16.161 | 10.198 | 10.198 | +1.98% |
| SA13.JT | SurfaceIr / 25 | 5.000 | 5.014 | 11.881 | 5.000 | 5.000 | +0.00% |
| SA14.JT | SurfaceIr / 25 | 4.500 | 4.511 | 7.445 | 4.500 | 4.500 | +0.00% |
| SA15.JT | SurfaceRadar / 70 | 12.000 | 12.003 | 10.158 | 12.000 | 12.000 | +0.00% |
| SA16.JT | SurfaceIr / 25 | 5.200 | 5.211 | 7.112 | 5.201 | 5.201 | +0.02% |
| SA19.JT | SurfaceRadar / 70 | 8.000 | 8.002 | 5.607 | 8.001 | 8.001 | +0.01% |
| SA2A.JT | SurfaceRadarLong / 120 | 29.000 | 29.003 | 18.214 | 29.006 | 29.006 | +0.02% |
| SA3.JT | SurfaceRadarLong / 120 | 25.000 | 25.002 | 16.332 | 25.000 | 25.000 | +0.00% |
| SA6.JT | SurfaceRadar / 70 | 24.000 | 23.995 | 15.065 | 24.000 | 24.000 | +0.00% |
| SA7.JT | SurfaceIr / 25 | 4.200 | 4.208 | 6.713 | 4.202 | 4.202 | +0.05% |
| SA9.JT | SurfaceIr / 25 | 4.200 | 4.210 | 7.768 | 4.200 | 4.200 | +0.00% |
| SAN11.JT | SurfaceRadar / 70 | 8.000 | 8.002 | 5.607 | 8.001 | 8.001 | +0.01% |
| SAN3.JT | SurfaceRadarLong / 120 | 30.000 | 29.991 | 18.645 | 30.000 | 30.000 | +0.00% |
| SAN4.JT | SurfaceRadar / 70 | 10.000 | 10.006 | 8.814 | 10.000 | 10.000 | +0.00% |
| SAN7.JT | SurfaceRadarLong / 120 | 25.000 | 25.002 | 15.540 | 25.004 | 25.004 | +0.02% |
| SAN9.JT | SurfaceRadar / 70 | 12.000 | 12.003 | 10.158 | 12.000 | 12.000 | +0.00% |
| SEA_SPAR.JT | SurfaceRadarLong / 120 | 26.000 | 26.000 | 18.100 | 26.006 | 26.006 | +0.02% |
| SSN9.JT | Cruise / 600 | 150.000 | 149.954 | 153.634 | 144.274 | 151.120 | +0.75% |

## Family, role and altitude checks

Family margins use all twelve altitude/aspect cells. Short-IR comparisons
use the three 5k/15k/35k tail cells. Low-altitude radar role comparisons use
5k head and tail. These checks do not assert universal real-world ordering.

| Newer / older | Minimum measured advantage |
| --- | ---: |
| AIM7.JT / AIM7E.JT | 2.529% |
| R27ER.JT / AA10.JT | 8.871% |
| R27ET.JT / R27T.JT | 34.280% |
| AIM9M.JT / AIM9B.JT | 0.175% |
| AIM9M.JT / AA2.JT | 19.647% |
| AIM9M.JT / AA8.JT | 164.782% |
| AIM9X.JT / AIM9B.JT | 7.918% |
| AIM9X.JT / AA2.JT | 28.896% |
| AIM9X.JT / AA8.JT | 156.679% |
| AA11.JT / AIM9B.JT | 101.540% |
| AA11.JT / AA2.JT | 151.785% |
| AA11.JT / AA8.JT | 247.017% |

| Active radar / older group | Smallest 5k head margin km | Smallest 5k tail margin km |
| --- | ---: | ---: |
| AIM120.JT | 9.399 | 4.844 |
| AA12.JT | 8.974 | 3.553 |
| MICA.JT | 5.496 | 0.313 |

| Store | H5/H35 | T5/H35 | T5 class floor km | 1.2 x decoded min km |
| --- | ---: | ---: | ---: | ---: |
| AA10.JT | 0.528 | 0.102 | 3 | 4.389 |
| AA11.JT | 0.560 | 0.290 | 1.5 | 0.732 |
| AA11B.JT | 0.560 | 0.290 | 1.5 | 0.549 |
| AA12.JT | 0.896 | 0.215 | 3 | 4.389 |
| AA2.JT | 0.423 | 0.150 | 1.5 | 1.463 |
| AA6.JT | 0.400 | 0.188 | 3 | 4.389 |
| AA8.JT | 0.943 | 0.173 | 1.5 | 1.463 |
| AA9.JT | 0.653 | 0.057 | 5 | 4.389 |
| AAML.JT | 0.329 | 0.104 | 5 | 10.973 |
| AEMP1.JT | 0.897 | 0.215 | 3 | 0.732 |
| AIM120.JT | 0.754 | 0.201 | 3 | 4.389 |
| AIM54C.JT | 0.347 | 0.073 | 5 | 10.973 |
| AIM7.JT | 0.310 | 0.118 | 3 | 2.926 |
| AIM7E.JT | 0.393 | 0.178 | 3 | 1.097 |
| AIM9B.JT | 0.576 | 0.204 | 1.5 | 1.463 |
| AIM9M.JT | 0.886 | 0.330 | 1.5 | 1.463 |
| AIM9X.JT | 0.846 | 0.309 | 1.5 | 1.097 |
| MICA.JT | 0.749 | 0.136 | 3 | 4.389 |
| PL10.JT | 0.784 | 0.174 | 3 | 2.926 |
| PL7.JT | 0.944 | 0.307 | 1.5 | 1.463 |
| R27ER.JT | 0.427 | 0.072 | 5 | 4.389 |
| R27ET.JT | 0.353 | 0.122 | 5 | 4.389 |
| R27T.JT | 0.357 | 0.153 | 3 | 4.389 |
| R530.JT | 0.335 | 0.164 | 3 | 2.926 |
| R550.JT | 0.936 | 0.315 | 1.5 | 1.463 |

The decoded minimum removes these nominal physical employment intervals:

- AA9.JT 500 ft tail: 3.206 km <= minimum 3.658 km
- AIM54C.JT 500 ft beam: 6.276 km <= minimum 9.144 km
- AIM54C.JT 500 ft tail: 2.393 km <= minimum 9.144 km

These are range-policy exclusions, not zero kinetic reach. The floor check
uses 5k tail, not the 500 ft deck. Altitude reversals above 10%: AA8.JT beam 15000/35000.

## Defeat and force diagnostics

The offline `--diagnostics` mode opts into available/required G, positive
G deficit, closing speed, Mach, control expiry and post-burn defeat at each
measured edge and 10% beyond. These values do not create a G-based death rule.
Raw commanded G can exceed available G; applied steering stays bounded.
Ordinary motion and prediction skip diagnostic-only calculations.
The behaviour tripwire includes low-Mach countdown, reset, defeat transition
and inert contacts. Dedicated tests cover the separate warning publication rule.

## Motor-shape audit

Phase 2.7 compares the reviewed Phase 2.6 data at efcd54db with the current
fits. This is implementation-mode game calibration. All rows use identical
120 Hz conditions and the current offline search. Reach is the primary
reference condition, with the A2A two-condition comparison above giving tails.
The Mach-one probe starts at 250 m/s and 5,000 ft for every store; it is
separate from the 300 m/s A2A reach launch and from a stationary SAM launch.
“None” means no crossing within the 20-second probe, not a failed ground-store
requirement. Edge speed is measured at the last sampled successful range.
A reference miss is labelled explicitly rather than reporting its final speed
as an arrival. Peak Mach refers to the primary edge trajectory only.

| Changed store | Reach km before / after | Mach-one s before / after | Edge arrival Mach before / after | Peak Mach before / after |
| --- | ---: | ---: | ---: | ---: |
| AA10.JT | 64.542 / 67.872 | 2.733 / 1.133 | 1.555 / 2.048 | 2.819 / 3.152 |
| AA11.JT | 27.007 / 26.851 | 0.300 / 0.292 | 2.115 / 2.260 | 6.691 / 6.279 |
| AA11B.JT | 27.007 / 26.851 | 0.300 / 0.292 | 2.115 / 2.260 | 6.691 / 6.279 |
| AA12.JT | 52.049 / 50.034 | 3.133 / 1.025 | 1.634 / 1.911 | 1.944 / 1.911 |
| AA2.JT | 16.765 / 16.693 | 0.408 / 0.350 | 1.329 / 1.326 | 2.909 / 2.868 |
| AA6.JT | 32.771 / 32.845 | 1.325 / 1.383 | 0.988 / 0.988 | 5.712 / 5.510 |
| AA8.JT | 12.832 / 12.987 | 9.725 / 1.033 | 1.461 / 1.320 | 1.509 / 1.377 |
| AA9.JT | 118.139 / 120.007 | 12.642 / 2.808 | 0.999 / 1.058 | 1.718 / 2.054 |
| AAML.JT | 149.763 / 150.018 | 2.608 / 1.083 | 1.146 / 1.283 | 2.926 / 2.586 |
| AEMP1.JT | 50.486 / 50.027 | 7.558 / 1.092 | 1.703 / 1.762 | 1.794 / 1.782 |
| AGM84A.JT | 183.101 / 186.128 | None / None | 0.915 / 0.929 | 0.921 / 0.934 |
| AGM84E.JT | 93.012 / 99.166 | None / None | 0.265 / 0.268 | 0.900 / 0.897 |
| AIM120.JT | 59.989 / 60.000 | 2.208 / 0.967 | 1.915 / 2.018 | 2.491 / 2.141 |
| AIM54C.JT | 144.541 / 150.053 | 6.817 / 2.650 | 1.165 / 0.997 | 2.336 / 3.080 |
| AIM9M.JT | 18.060 / 18.011 | 2.300 / 0.858 | 2.698 / 2.155 | 2.933 / 2.544 |
| AIM9X.JT | 18.005 / 18.631 | 1.442 / 0.650 | 2.224 / 2.232 | 2.615 / 2.518 |
| AM39.JT | 70.000 / 76.289 | 2.692 / None | 0.218 / 0.173 | 2.356 / 0.951 |
| AS16.JT | 150.000 / 150.000 | 1.192 / 3.400 | 0.233 / 0.200 | 6.352 / 3.514 |
| MICA.JT | 59.989 / 55.239 | 3.342 / 1.108 | 1.691 / 1.825 | 2.603 / 2.037 |
| PL10.JT | 49.765 / 50.088 | 4.083 / 3.000 | 1.508 / 1.590 | 1.858 / 1.758 |
| PL7.JT | 15.411 / 15.695 | 11.675 / 3.000 | 2.778 / 2.251 | 2.955 / 2.316 |
| R27ER.JT | 116.847 / 117.008 | 7.958 / 0.600 | 0.999 / 0.995 | 1.822 / 2.655 |
| R550.JT | 15.878 / 16.112 | 3.367 / 1.000 | 2.249 / 1.866 | 2.321 / 2.042 |
| SSN9.JT | 144.283 / 151.155 | 1.017 / 0.742 | 0.175 / 0.822 | 0.932 / 0.822 |

Motor shape is thrust in kN / phase seconds. Boost is listed before sustain;
a zero booster denotes a sustain-only engine. The parameter table in
[the data document](../formats/missile-energy-data.md#per-store-motors-and-class-control-budgets)
contains fuel, drag, phase split and coefficients for every store.

| Changed store | Before boost; sustain | After boost; sustain | Initial g before / after | Dry-mass peak g before / after |
| --- | --- | --- | ---: | ---: |
| AA10.JT | 10.652 / 16.000; 3.824 / 24.000 | 21.892 / 3.117; 4.744 / 53.717 | 4.293 / 8.824 | 6.274 / 15.996 |
| AA11.JT | 31.198 / 5.125; 0.000 / 0.000 | 32.298 / 1.058; 32.298 / 3.675 | 30.298 / 31.366 | 59.903 / 53.490 |
| AA11B.JT | 31.198 / 5.125; 0.000 / 0.000 | 32.298 / 1.058; 32.298 / 3.675 | 30.298 / 31.366 | 59.903 / 53.490 |
| AA12.JT | 5.147 / 6.967; 1.846 / 10.458 | 14.933 / 1.575; 0.665 / 68.417 | 2.999 / 8.701 | 3.459 / 10.012 |
| AA2.JT | 9.562 / 0.208; 22.931 / 1.358 | 18.763 / 1.708; 13.151 / 0.050 | 12.501 / 24.529 | 59.951 / 49.051 |
| AA6.JT | 138.178 / 4.075; 0.000 / 0.000 | 116.336 / 2.317; 116.333 / 2.425 | 29.664 / 24.975 | 59.286 / 49.889 |
| AA8.JT | 0.501 / 15.250; 0.000 / 0.000 | 3.599 / 1.292; 0.071 / 18.675 | 1.162 / 8.341 | 1.320 / 9.479 |
| AA9.JT | 6.403 / 21.833; 1.571 / 72.792 | 57.695 / 2.683; 0.903 / 137.642 | 1.333 / 12.007 | 1.934 / 15.970 |
| AAML.JT | 18.072 / 11.700; 4.436 / 39.000 | 40.356 / 4.450; 2.904 / 56.208 | 3.686 / 8.230 | 5.706 / 12.163 |
| AEMP1.JT | 2.402 / 16.000; 0.862 / 24.000 | 14.041 / 1.158; 0.943 / 48.442 | 1.400 / 8.182 | 1.594 / 9.388 |
| AGM84A.JT | 0.000 / 0.000; 3.000 / 962.000 | 0.000 / 0.000; 3.300 / 962.000 | 0.567 / 0.623 | 1.133 / 1.246 |
| AGM84E.JT | 0.000 / 0.000; 2.413 / 253.375 | 0.000 / 0.000; 1.860 / 461.100 | 0.392 / 0.303 | 0.462 / 0.344 |
| AIM120.JT | 6.642 / 7.475; 2.385 / 11.208 | 14.443 / 3.667; 0.572 / 47.175 | 4.291 / 9.331 | 5.306 / 11.872 |
| AIM54C.JT | 11.239 / 20.283; 2.758 / 67.617 | 68.122 / 0.775; 6.839 / 76.317 | 2.475 / 15.003 | 3.070 / 19.790 |
| AIM9M.JT | 3.659 / 14.925; 0.000 / 0.000 | 9.904 / 0.600; 6.353 / 5.292 | 4.330 / 11.718 | 5.222 / 13.322 |
| AIM9X.JT | 5.260 / 7.292; 0.000 / 0.000 | 11.510 / 2.708; 1.300 / 10.192 | 6.290 / 13.764 | 8.412 / 17.507 |
| AM39.JT | 92.641 / 2.000; 25.266 / 6.000 | 35.093 / 0.925; 1.766 / 336.208 | 14.100 / 5.341 | 18.277 / 10.511 |
| AS16.JT | 550.373 / 2.308; 150.283 / 6.917 | 49.428 / 23.683; 8.002 / 90.200 | 46.769 / 4.200 | 59.863 / 7.325 |
| MICA.JT | 3.645 / 17.233; 0.588 / 26.808 | 9.136 / 3.883; 0.390 / 63.350 | 3.319 / 8.318 | 4.462 / 10.437 |
| PL10.JT | 8.007 / 4.000; 2.874 / 6.000 | 16.335 / 2.583; 0.283 / 57.275 | 4.665 / 9.518 | 5.350 / 10.876 |
| PL7.JT | 0.194 / 11.475; 34.380 / 1.233 | 7.259 / 0.475; 1.801 / 15.792 | 0.223 / 8.317 | 58.082 / 12.639 |
| R27ER.JT | 4.821 / 17.183; 1.298 / 34.367 | 53.622 / 2.225; 4.364 / 80.583 | 1.404 / 15.623 | 1.951 / 24.386 |
| R550.JT | 2.437 / 14.733; 0.000 / 0.000 | 7.623 / 1.183; 5.233 / 3.308 | 2.792 / 8.734 | 3.179 / 10.063 |
| SSN9.JT | 303.160 / 3.000; 7.600 / 360.000 | 426.821 / 1.267; 9.466 / 598.733 | 9.721 / 13.687 | 19.443 / 21.574 |

The old PL7 motor was inverted: a weak 11.475-second boost followed by a
1.233-second, 34.38 kN sustain. It was not simply a long-burn motor. The new
motor has a short stronger boost and a weaker sustain. AA8 now has a short
strong booster followed by a weak long sustain; the shared 20-second control
deadline remains independent. AA2's brief inverted sustain is removed.
AS16's coefficient and dry-mass acceleration are now inside the guard, with
its peak Mach reported above rather than treated as a real-world claim.
PL7's approved +/-12.5% exception keeps the existing drag bounds and motor
guard. Its residual is about +12.1% head and -12.1% tail. No other reference
band changed in this round.

### Powered cruise at the exact reference

All four checked cruise stores now hit while powered and arrive within the
agent-selected Mach 0.60..0.95 band. Before values labelled “miss” are the
end-of-trial condition. AGM84A's small thrust increase prevents control expiry
just short of its reference; its long 962-second engine is retained. SSN9 uses
the candidate with less booster impulse when range scores tie, an agent choice
to avoid unnecessary initial acceleration. AS16 is not a sea-skimmer: it keeps
normal drag with the long-ground control budget.

| Store | Reference km | Before: hit / powered / Mach / seconds | After: hit / powered / Mach / seconds |
| --- | ---: | --- | --- |
| AGM84A.JT | 185 | miss / true / 0.892 / 600.01 | hit / true / 0.929 / 596.40 |
| AGM84E.JT | 93 | hit / false / 0.265 / 359.71 | hit / true / 0.619 / 462.38 |
| AM39.JT | 70 | hit / false / 0.218 / 243.12 | hit / true / 0.618 / 339.08 |
| SSN9.JT | 150 | miss / false / 0.184 / 551.83 | hit / true / 0.822 / 595.69 |

### Accepted fitted boundaries and long burns

Fuel and drag remain authored calibration values, not measured aerodynamics.
The five highlighted Phase 2.6 drag-ceiling stores are listed below. AIM7
and AIM9B already passed the new guard and are unchanged. AA6, AA11 and AA2
needed shape changes; their near-ceiling drag remains accepted. No coefficient
or dry-mass load is exempted from the interior motor guard.

| Store | Before drag / total burn s | After drag / total burn s | Disposition |
| --- | ---: | ---: | --- |
| AA6.JT | 3.000000 / 4.075 | 2.999740 / 4.742 | Guard-required refit |
| AIM7.JT | 3.000000 / 16.000 | 3.000000 / 16.000 | Retained, guard passes |
| AIM9B.JT | 3.000000 / 2.200 | 3.000000 / 2.200 | Retained, guard passes |
| AA11.JT | 2.998453 / 5.125 | 2.999764 / 4.733 | Guard-required refit |
| AA2.JT | 2.998714 / 1.567 | 2.992430 / 1.758 | Guard-required refit |
| AIM54C.JT | 0.952529 / 87.900 | 1.281605 / 77.092 | Guard-required refit |
| AA9.JT | 0.790769 / 94.625 | 0.845315 / 140.325 | Guard-required refit |
| AA10.JT | 2.823302 / 40.000 | 2.990471 / 56.833 | Guard-required refit |

The retained-boundary policy also accepts the following fuel/drag endpoints
(within 0.001 of a search edge); the full precise values are in the data table:

- AA11.JT: drag 2.999764.
- AA11B.JT: drag 2.999764.
- AA12.JT: drag 0.500000.
- AA2.JT: fuel 0.499927.
- AA6.JT: fuel 0.499395, drag 2.999740.
- AA8.JT: fuel 0.120000.
- AEMP1.JT: drag 0.500000.
- AGM84A.JT: fuel 0.500000.
- AGM84E.JT: fuel 0.120030.
- AIM7.JT: drag 3.000000.
- AIM7E.JT: fuel 0.500000.
- AIM9B.JT: fuel 0.120000, drag 3.000000.
- AIM9M.JT: fuel 0.120425.
- AM39.JT: drag 2.999940.
- AS16.JT: drag 0.500000.
- AT12.JT: drag 3.000000.
- AT2.JT: fuel 0.120000, drag 3.000000.
- R440.JT: fuel 0.500000.

These bounded game fits remain approximations. The guards constrain shape,
not engineering fidelity; long sustains and residuals are still explicit.
PL7 and PL10 reach Mach 1 at exactly 3.000 s in the standardized probe,
on that acceptance threshold. AA11 and AA11B reach 53.490 dry-mass g,
near the staged search's 53.5 g ceiling. This is an accepted agent fit choice,
10.85% below the old 60 g limit; neither bypasses the shared 54 g guard. AGM84A retains a declared coefficient of 25000 m/s
but its explicit 3300 N engine gives actual impulse/fuel of 11757.778 m/s;
both pass the sustain-only engine guard. The other changed stores use
matching declared and actual coefficients, within output rounding.
AGM84E and AM39 are powered at their exact references, then coast to their
farther measured edges; the edge columns do not imply powered flight there.

## Validation status

Phase 2.7, Linux x86-64, based on reviewer commit efcd54db, which is preserved.
Model identity 14394240088631571064, behavior trace 4002119430299407734. The new motor data changes
the model identity automatically; runtime rule revisions are unchanged.
The complete workspace passes 7,219 tests with 106 intentionally
ignored. Three separate example tests cover inverted/weak motors, coefficient
and dry-mass guards, slow acceleration, sustain-only cruise and bounded
reference exceptions. The class-data and behavior-identity tests also pass.
Workspace build, clippy with warnings denied, formatting, documentation,
518 Python tests (five skipped) and source/app/extractor asset checks pass.

All 59 reference checks pass their declared bands, motor-shape and applicable
cruise-arrival checks. Tail floors, decoded-minimum margins, family and radar
role ordering, and modern-IR deck escape checks pass. Maximum reach change
under a 2% perturbation is 4.570%. The complete 391-row range sweep,
668-row edge/beyond diagnostic sweep and 59-row before/after motor audits
were regenerated. The behavior golden pair was updated after reviewing the
data-only change. The initial cruise check exposed AGM84A's shortfall; its
final 3300 N sustain fit passes without changing its control budget.

All 36 battery scenarios pass in one complete run. This includes the moving
SAM targets, Energy/worker comparison, historical Spec shots, inert near-pass
checks, player range-cue scenarios and five-fight launch-count regression.
The reviewer observer-picture fix is retained and the workspace tests cover it.
No physics, guidance, AI firing policy, control timers or defeat rule changed.

Windows/macOS were not run on this Linux host. Retail comparison is unavailable.
The named range residuals, tight AA11/B band margin, AA8 high-altitude beam
limitation and fitted motor values above remain approximations. The invalid
Phase 3 study stays stopped and labelled pre-fix. This round does not merge.

## Natural launch counts against both baselines

The accepted gate remains a one-sided 70% floor against pre-overhaul Spec and
a symmetric 70%..130% band against reviewed Phase 2.5. There is no Spec upper
ceiling. The table uses the sequential cost lanes; the battery independently
checks the same five fights. No firing or admission policy was adjusted.

| Fight | Pre-overhaul Spec | Phase 2.5 | Phase 2.7 | Change vs Spec | Change vs 2.5 | Spec floor | 2.5 band |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| UKR | 11 | 13 | 12 | +9.09% | -7.69% | 7.7 | 9.1..16.9 |
| BAL | 12 | 13 | 13 | +8.33% | +0.00% | 8.4 | 9.1..16.9 |
| SPA | 12 | 13 | 13 | +8.33% | +0.00% | 8.4 | 9.1..16.9 |
| GRE | 12 | 13 | 13 | +8.33% | +0.00% | 8.4 | 9.1..16.9 |
| 23-AI UKR | 24 | 26 | 25 | +4.17% | -3.85% | 16.8 | 18.2..33.8 |

## Prediction cost

Release examples on the isolated imported profile. Every heavy command checks
free memory first, waits below 8 GB available, runs at nice 15 with four Cargo
jobs, and is serialized with other heavy commands. Rust tests use four threads;
the battery uses two jobs and one window, wrapped through tools/agent-run.sh.
No window was launched on John's active workspace.

Natural UKR/BAL/SPA/GRE fights run 14,400 ticks with eight AI aircraft; the
23-AI UKR fight runs 7,200 ticks. Turning-player and held/frozen/moving/cruise
lanes run 2,400 ticks. Held lanes use eight stations and sixteen missiles.
Times below are prediction-service milliseconds per world tick. Cold and
steady show wall p50 / p95; CPU p95 is the larger of their CPU values.
The cruise hold measures cost, not guaranteed cue availability.

| Lane | Cold p50 / p95 ms | Steady p50 / p95 ms | Max CPU p95 ms | Refused / starved | Guided launches |
| --- | ---: | ---: | ---: | ---: | ---: |
| natural-ukr | 0.822 / 0.928 | 0.777 / 0.907 | 0.925 | 0 / 0 | 12 |
| natural-bal | 0.823 / 0.944 | 0.723 / 0.914 | 0.941 | 0 / 0 | 13 |
| natural-spa | 0.835 / 0.936 | 0.651 / 0.925 | 0.933 | 0 / 0 | 13 |
| natural-gre | 0.834 / 0.947 | 0.721 / 0.919 | 0.942 | 0 / 0 | 13 |
| natural-big | 0.911 / 0.957 | 0.871 / 0.965 | 0.961 | 0 / 0 | 25 |
| turning-3 | 0.897 / 0.959 | 0.908 / 0.962 | 0.957 | 0 / 0 | 13 |
| turning-10 | 0.907 / 0.956 | 0.912 / 0.955 | 0.953 | 0 / 0 | 9 |
| hold-frozen | 1.028 / 1.084 | 0.643 / 1.083 | 1.080 | 0 / 0 | 44 |
| hold-moving | 1.021 / 1.035 | 0.876 / 1.096 | 1.091 | 0 / 0 | 44 |
| hold-cruise | 0.777 / 0.832 | 0.798 / 0.871 | 0.869 | 0 / 0 | 16 |

Worst wall p95 is 1.096 ms and worst CPU p95 is 1.091 ms, below the
2 ms gate. All ten lanes record zero refused and zero starved requests.
