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

This baseline contains Phase 2.8 burn/sustain and powered-cruise fits. Same-tick inert threat
removal is unchanged.
Class budgets and post-burn defeat are fixed inputs to the reach search, not
optimizer parameters. Defeat therefore affects every candidate's measured reach. The earlier Phase 3
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
differ slightly from the complete sweep without a physics change. Acceptance uses the verifier precision, not rounded display values. Retained
AA11/AA11B H35 is 26.851 km at verifier precision, inside the 26.850 km lower
gate; the coarser display sweep reports 26.846 km.

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
| AA10.JT | 73 | 23.39 / 10.43 / 5.72 | 25.77 / 12.60 / 7.18 | 33.08 / 17.88 / 11.17 | 64.73 / 37.30 / 25.11 |
| AA11.JT | 30 | 13.37 / 9.24 / 6.88 | 15.04 / 10.39 / 7.79 | 20.05 / 13.78 / 10.47 | 26.85 / 21.07 / 16.85 |
| AA11B.JT | 30 | 13.37 / 9.24 / 6.88 | 15.04 / 10.39 / 7.79 | 20.05 / 13.78 / 10.47 | 26.85 / 21.07 / 16.85 |
| AA12.JT | 50 | 38.13 / 15.84 / 8.90 | 45.66 / 18.37 / 10.44 | 48.19 / 23.42 / 14.11 | 52.04 / 28.03 / 18.88 |
| AA2.JT | 20 | 6.13 / 3.27 / 2.18 | 7.06 / 3.74 / 2.51 | 9.85 / 5.14 / 3.50 | 16.69 / 9.94 / 6.69 |
| AA6.JT | 30 | 11.76 / 7.68 / 5.46 | 13.12 / 8.61 / 6.18 | 17.16 / 11.30 / 8.26 | 32.84 / 21.45 / 16.13 |
| AA8.JT | 10 | 9.45 / 2.08 / 1.89 | 11.31 / 2.89 / 2.27 | 12.76 / 4.80 / 2.75 | 13.20 / 4.67 / 3.20 |
| AA9.JT | 120 | 12.25 / 6.36 / 2.54 | 42.42 / 14.00 / 6.82 | 51.53 / 22.25 / 11.74 | 120.03 / 51.01 / 30.09 |
| AAML.JT | 150 | 38.72 / 21.55 / 10.48 | 43.85 / 25.18 / 15.64 | 58.33 / 34.54 / 22.53 | 150.00 / 69.07 / 46.08 |
| AEMP1.JT | 50 | 36.94 / 15.50 / 8.94 | 45.06 / 18.02 / 10.46 | 48.33 / 23.43 / 14.23 | 52.14 / 28.02 / 18.97 |
| AIM120.JT | 60 | 32.65 / 16.51 / 10.31 | 40.14 / 19.17 / 12.06 | 52.69 / 27.05 / 17.19 | 60.00 / 36.48 / 25.75 |
| AIM54C.JT | 150 | 12.41 / 6.28 / 2.39 | 52.09 / 21.90 / 11.00 | 65.71 / 32.39 / 18.16 | 150.00 / 70.82 / 43.00 |
| AIM7.JT | 70 | 16.11 / 9.10 / 5.88 | 18.19 / 10.45 / 6.89 | 24.27 / 14.23 / 9.71 | 58.58 / 27.82 / 19.63 |
| AIM7E.JT | 35 | 12.86 / 8.17 / 5.73 | 14.44 / 9.21 / 6.53 | 19.04 / 12.15 / 8.77 | 36.77 / 23.17 / 17.18 |
| AIM9B.JT | 20 | 8.94 / 4.74 / 3.16 | 10.36 / 5.45 / 3.67 | 14.65 / 7.54 / 5.20 | 17.99 / 11.49 / 7.99 |
| AIM9M.JT | 18 | 15.53 / 8.80 / 5.52 | 15.95 / 9.32 / 5.95 | 16.80 / 10.29 / 6.79 | 18.01 / 11.50 / 8.01 |
| AIM9X.JT | 18 | 15.53 / 8.82 / 5.52 | 15.96 / 9.34 / 5.95 | 16.81 / 10.31 / 6.80 | 18.03 / 11.51 / 8.03 |
| MICA.JT | 60 | 23.61 / 11.32 / 6.29 | 26.51 / 13.12 / 7.53 | 37.37 / 18.52 / 11.34 | 60.00 / 35.36 / 23.69 |
| PL10.JT | 50 | 29.31 / 6.63 / 3.05 | 35.62 / 14.57 / 8.70 | 46.43 / 20.61 / 12.24 | 50.09 / 26.01 / 17.34 |
| PL7.JT | 14 | 14.63 / 7.84 / 4.63 | 14.82 / 8.06 / 4.82 | 15.18 / 8.44 / 5.19 | 15.69 / 8.78 / 5.69 |
| R27ER.JT | 117 | 48.93 / 15.05 / 7.38 | 49.98 / 16.62 / 8.40 | 56.68 / 24.23 / 13.35 | 117.00 / 57.57 / 37.54 |
| R27ET.JT | 104 | 31.64 / 17.08 / 10.97 | 36.79 / 19.81 / 12.73 | 51.91 / 27.78 / 17.87 | 104.13 / 55.45 / 35.85 |
| R27T.JT | 63 | 19.03 / 11.63 / 8.17 | 21.97 / 13.36 / 9.41 | 30.78 / 18.52 / 13.11 | 61.52 / 38.11 / 26.68 |
| R530.JT | 40 | 13.13 / 8.82 / 6.35 | 14.70 / 9.90 / 7.21 | 19.44 / 12.86 / 9.54 | 43.92 / 25.64 / 18.78 |
| R550.JT | 15 | 14.86 / 8.08 / 4.86 | 15.08 / 8.34 / 5.08 | 15.51 / 8.81 / 5.51 | 16.11 / 9.32 / 6.11 |

## Complete A2A seeker-valid reach

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | 23.39 / 10.43 / 5.72 | 25.77 / 12.60 / 7.18 | 33.08 / 17.88 / 11.17 | 64.54 / 37.29 / 25.11 |
| AA11.JT | 11.56 / 9.22 / 6.88 | 11.56 / 10.38 / 7.79 | 11.56 / 13.77 / 10.47 | 11.56 / 15.30 / 16.85 |
| AA11B.JT | 11.56 / 9.22 / 6.88 | 11.56 / 10.38 / 7.79 | 11.56 / 13.77 / 10.47 | 11.56 / 15.30 / 16.85 |
| AA12.JT | 38.14 / 15.78 / 8.90 | 45.66 / 18.28 / 10.45 | 48.19 / 23.34 / 14.11 | 52.04 / 27.96 / 18.88 |
| AA2.JT | 0.00 / 0.00 / 2.18 | 0.00 / 0.00 / 2.51 | 0.00 / 0.00 / 3.50 | 0.00 / 0.00 / 6.69 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 5.78 / 0.00 / 1.89 | 5.78 / 2.56 / 2.27 | 5.78 / 4.74 / 2.75 | 5.78 / 4.51 / 3.19 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 38.72 / 21.53 / 10.72 | 43.85 / 25.16 / 15.64 | 58.33 / 34.52 / 22.53 | 150.00 / 68.96 / 46.08 |
| AEMP1.JT | 36.95 / 15.29 / 8.94 | 45.06 / 17.75 / 10.46 | 48.32 / 23.25 / 14.23 | 52.14 / 27.83 / 18.97 |
| AIM120.JT | 32.66 / 16.44 / 10.31 | 40.15 / 19.10 / 12.06 | 52.69 / 26.92 / 17.19 | 59.98 / 36.40 / 25.75 |
| AIM54C.JT | 12.41 / 6.28 / 2.39 | 52.09 / 21.87 / 11.00 | 65.71 / 32.37 / 18.16 | 150.00 / 70.76 / 43.01 |
| AIM7.JT | 16.11 / 9.10 / 5.88 | 18.19 / 10.45 / 6.89 | 24.27 / 14.23 / 9.71 | 58.45 / 27.82 / 19.63 |
| AIM7E.JT | 12.86 / 8.17 / 5.73 | 14.44 / 9.21 / 6.53 | 19.04 / 12.15 / 8.77 | 36.61 / 23.17 / 17.18 |
| AIM9B.JT | 0.00 / 0.00 / 3.16 | 0.00 / 0.00 / 3.67 | 0.00 / 0.00 / 5.20 | 0.00 / 0.00 / 7.99 |
| AIM9M.JT | 9.64 / 8.78 / 5.52 | 9.64 / 9.29 / 5.95 | 9.64 / 10.26 / 6.79 | 9.64 / 11.48 / 8.01 |
| AIM9X.JT | 9.64 / 8.79 / 5.52 | 9.64 / 9.32 / 5.95 | 9.64 / 10.29 / 6.80 | 9.64 / 11.50 / 8.03 |
| MICA.JT | 23.61 / 11.29 / 6.29 | 26.51 / 13.09 / 7.53 | 37.37 / 18.50 / 11.34 | 59.98 / 35.27 / 23.69 |
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
| AGM84E.JT | 93 | 1500/10 | 93.00 | 91.41 |
| AGM88.JT | 48.28032 | 1500/10 | 48.28 | 48.28 |
| AM39.JT | 70 | 1500/10 | 71.48 | 71.48 |
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

| Store | Targets H35 / T5 | Round 2 | Phase 2.5 | Phase 2.6 | Phase 2.7 | Phase 2.8 | Residual H35 / T5 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| AA10.JT | 73.000 / 6.473 | 73.000 / 6.473 | 73.000 / 12.602 | 64.542 / 7.205 | 67.870 / 6.915 | 64.734 / 7.180 | -11.32% / +10.92% |
| AA11.JT | 30.000 / 7.056 | 30.005 / 7.056 | 29.749 / 12.848 | 27.003 / 7.758 | 26.846 / 7.794 | 26.846 / 7.794 | -10.51% / +10.46% |
| AA11B.JT | 30.000 / 7.056 | 30.005 / 7.056 | 29.749 / 12.848 | 27.003 / 7.758 | 26.846 / 7.794 | 26.846 / 7.794 | -10.51% / +10.46% |
| AA12.JT | 50.000 / 10.767 | 50.000 / 10.767 | 50.000 / 5.558 | 52.037 / 10.195 | 50.031 / 10.767 | 52.037 / 10.443 | +4.07% / -3.01% |
| AA2.JT | 20.000 / 2.180 | 20.013 / 2.180 | 19.994 / 5.603 | 16.765 / 2.532 | 16.691 / 2.509 | 16.691 / 2.509 | -16.55% / +15.09% |
| AA6.JT | 30.000 / 6.811 | 30.005 / 6.811 | 30.062 / 4.398 | 32.761 / 6.181 | 32.841 / 6.176 | 32.841 / 6.176 | +9.47% / -9.32% |
| AA8.JT | 10.000 / 2.377 | 10.014 / 2.377 | 14.045 / 1.596 | 12.829 / 2.250 | 12.987 / 2.246 | 13.198 / 2.274 | +31.98% / -4.33% |
| AA9.JT | 120.000 / 6.816 | 120.000 / 6.816 | 120.000 / 13.244 | 118.134 / 6.867 | 120.000 / 6.816 | 120.027 / 6.823 | +0.02% / +0.10% |
| AAML.JT | 150.000 / 15.620 | 150.000 / 15.620 | 150.000 / 17.109 | 149.763 / 15.580 | 150.000 / 15.620 | 150.000 / 15.637 | +0.00% / +0.11% |
| AEMP1.JT | 50.000 / 10.752 | 50.015 / 10.752 | 50.000 / 5.547 | 50.475 / 10.417 | 50.015 / 10.767 | 52.144 / 10.462 | +4.29% / -2.70% |
| AIM120.JT | 60.000 / 12.058 | 60.017 / 12.058 | 60.000 / 10.555 | 59.985 / 12.050 | 60.000 / 12.058 | 60.000 / 12.058 | +0.00% / +0.00% |
| AIM54C.JT | 150.000 / 9.811 | 150.000 / 9.811 | 150.000 / 17.425 | 144.515 / 10.999 | 150.000 / 10.999 | 150.000 / 10.999 | +0.00% / +12.11% |
| AIM7.JT | 70.000 / 5.792 | 70.000 / 5.792 | 70.000 / 16.254 | 58.577 / 6.890 | 58.577 / 6.890 | 58.577 / 6.890 | -16.32% / +18.96% |
| AIM7E.JT | 35.000 / 8.061 | 35.009 / 8.061 | 34.994 / 3.022 | 36.768 / 6.530 | 36.768 / 6.530 | 36.768 / 6.530 | +5.05% / -18.99% |
| AIM9B.JT | 20.000 / 3.693 | 18.087 / 3.693 | 19.652 / 5.947 | 17.994 / 3.673 | 17.994 / 3.673 | 17.994 / 3.673 | -10.03% / -0.54% |
| AIM9M.JT | 18.000 / 5.938 | 18.024 / 5.938 | 21.363 / 6.028 | 18.060 / 5.935 | 18.010 / 5.947 | 18.010 / 5.947 | +0.06% / +0.15% |
| AIM9X.JT | 18.000 / 5.944 | 18.024 / 5.944 | 21.429 / 6.028 | 18.004 / 5.961 | 18.631 / 5.765 | 18.029 / 5.952 | +0.16% / +0.13% |
| MICA.JT | 60.000 / 7.527 | 60.006 / 7.527 | 60.000 / 9.765 | 59.985 / 7.524 | 55.239 / 7.527 | 60.000 / 7.530 | +0.00% / +0.04% |
| PL10.JT | 50.000 / 8.699 | 50.000 / 8.699 | 50.000 / 5.723 | 49.763 / 8.706 | 50.077 / 8.699 | 50.092 / 8.699 | +0.18% / +0.00% |
| PL7.JT | 14.000 / 5.474 | 14.026 / 5.474 | 13.995 / 1.506 | 15.408 / 4.926 | 15.691 / 4.811 | 15.691 / 4.819 | +12.08% / -11.97% |
| R27ER.JT | 117.000 / 8.402 | 117.000 / 8.402 | 117.000 / 13.700 | 116.838 / 8.413 | 117.000 / 8.402 | 117.000 / 8.402 | +0.00% / +0.00% |
| R27ET.JT | 104.000 / 12.755 | 103.962 / 12.755 | 103.962 / 14.882 | 104.128 / 12.730 | 104.128 / 12.730 | 104.128 / 12.730 | +0.12% / -0.20% |
| R27T.JT | 63.000 / 9.309 | 62.990 / 9.309 | 62.990 / 11.657 | 61.523 / 9.414 | 61.523 / 9.414 | 61.523 / 9.414 | -2.34% / +1.13% |
| R530.JT | 40.000 / 7.997 | 40.011 / 7.997 | 40.004 / 3.013 | 43.916 / 7.214 | 43.916 / 7.214 | 43.916 / 7.214 | +9.79% / -9.79% |
| R550.JT | 15.000 / 5.476 | 15.021 / 5.476 | 14.996 / 3.373 | 15.876 / 5.076 | 16.111 / 5.078 | 16.111 / 5.078 | +7.41% / -7.27% |

Seven records have explicit residual exceptions: AA10, AA2, AA8, AIM54C, AIM7,
AIM7E and PL7. John approved the PL7 +/-12.5% exception during Phase 2.7
after the bounded guarded search missed the former band. In Phase 2.8 he
also approved the AA8 head-only ceiling of 132.5%, retaining its tail band. The default
verifier band is 89.5%..110.5% of the reference, with half a percentage point
for sampling tolerance. The exceptions below are bounds, not exemptions.
M/X now use the ordinary band; their former overshoot is no longer accepted.

| Store | Accepted H35/reference | Accepted T5/reference | Reason |
| --- | --- | --- | --- |
| PL7 | 0.875..1.125 | 0.875..1.125 | John-approved guarded motor compromise |
| AA10 | 0.86..1.105 | 0.895..1.13 | Best found bounded joint role/family fit |
| AA2 | 0.82..1.105 | 0.895..1.18 | Best found bounded two-condition fit |
| AA8 | 0.895..1.325 | 0.895..1.105 | John-approved Phase 2.8 head-only exception with the stricter motor guard |
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
AS16 uses BoostGlide 600 s, retaining normal drag. AM39 and AGM84E use
the shared ShortCruise 300 s class approved in Phase 2.8.
These agent assignments use the lead's explicit permission; the class times
were fixed before motor fitting. The original nine class values remain.

| Store | Class / control s | Reference km | Round 2 km | Phase 2.5 km | Phase 2.6 km | Phase 2.7 km | Phase 2.8 km | Residual |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| AGM45.JT | FastGround / 90 | 40.000 | 39.998 | 42.735 | 40.000 | 40.000 | 40.000 | +0.00% |
| AGM65A.JT | FastGround / 90 | 27.000 | 26.998 | 27.387 | 27.000 | 27.000 | 27.000 | +0.00% |
| AGM65G.JT | FastGround / 90 | 32.187 | 32.186 | 33.811 | 32.187 | 32.187 | 32.187 | +0.00% |
| AGM84A.JT | Cruise / 600 | 185.000 | 184.953 | 189.788 | 183.074 | 186.111 | 186.111 | +0.60% |
| AGM84E.JT | ShortCruise / 300 | 93.000 | 93.010 | 173.696 | 93.011 | 99.166 | 93.000 | +0.00% |
| AGM88.JT | FastGround / 90 | 48.280 | 48.280 | 49.199 | 48.280 | 48.280 | 48.280 | -0.00% |
| AM39.JT | ShortCruise / 300 | 70.000 | 69.982 | 78.770 | 70.000 | 76.268 | 71.476 | +2.11% |
| AS14.JT | ShortGround / 30 | 10.000 | 10.010 | 33.726 | 10.000 | 10.000 | 10.000 | +0.00% |
| AS15.JT | FastGround / 90 | 18.288 | 18.296 | 39.043 | 18.290 | 18.290 | 18.290 | +0.01% |
| AS16.JT | BoostGlide / 600 | 150.000 | 149.954 | 76.236 | 150.000 | 150.000 | 150.000 | +0.00% |
| AS30.JT | ShortGround / 30 | 11.000 | 11.011 | 34.610 | 10.998 | 10.998 | 10.998 | -0.02% |
| AS7.JT | ShortGround / 30 | 10.000 | 10.006 | 22.702 | 10.000 | 10.000 | 10.000 | +0.00% |
| AT12.JT | ShortGround / 30 | 7.925 | 7.943 | 53.390 | 7.923 | 7.923 | 7.923 | -0.02% |
| AT2.JT | Command / 30 | 4.000 | 4.008 | 9.102 | 4.260 | 4.260 | 4.260 | +6.50% |
| FIM92.JT | SurfaceIr / 25 | 4.000 | 4.010 | 7.898 | 4.000 | 4.000 | 4.000 | +0.00% |
| MIS.JT | SurfaceRadar / 70 | 6.500 | 6.507 | 7.014 | 6.500 | 6.500 | 6.500 | +0.00% |
| R440.JT | SurfaceRadar / 70 | 10.000 | 10.014 | 16.161 | 10.198 | 10.198 | 10.198 | +1.98% |
| SA13.JT | SurfaceIr / 25 | 5.000 | 5.014 | 11.881 | 5.000 | 5.000 | 5.000 | +0.00% |
| SA14.JT | SurfaceIr / 25 | 4.500 | 4.511 | 7.445 | 4.500 | 4.500 | 4.500 | +0.00% |
| SA15.JT | SurfaceRadar / 70 | 12.000 | 12.003 | 10.158 | 12.000 | 12.000 | 12.000 | +0.00% |
| SA16.JT | SurfaceIr / 25 | 5.200 | 5.211 | 7.112 | 5.201 | 5.201 | 5.201 | +0.02% |
| SA19.JT | SurfaceRadar / 70 | 8.000 | 8.002 | 5.607 | 8.001 | 8.001 | 8.001 | +0.01% |
| SA2A.JT | SurfaceRadarLong / 120 | 29.000 | 29.003 | 18.214 | 29.006 | 29.006 | 29.006 | +0.02% |
| SA3.JT | SurfaceRadarLong / 120 | 25.000 | 25.002 | 16.332 | 25.000 | 25.000 | 25.000 | +0.00% |
| SA6.JT | SurfaceRadar / 70 | 24.000 | 23.995 | 15.065 | 24.000 | 24.000 | 24.000 | +0.00% |
| SA7.JT | SurfaceIr / 25 | 4.200 | 4.208 | 6.713 | 4.202 | 4.202 | 4.202 | +0.05% |
| SA9.JT | SurfaceIr / 25 | 4.200 | 4.210 | 7.768 | 4.200 | 4.200 | 4.200 | +0.00% |
| SAN11.JT | SurfaceRadar / 70 | 8.000 | 8.002 | 5.607 | 8.001 | 8.001 | 8.001 | +0.01% |
| SAN3.JT | SurfaceRadarLong / 120 | 30.000 | 29.991 | 18.645 | 30.000 | 30.000 | 30.000 | +0.00% |
| SAN4.JT | SurfaceRadar / 70 | 10.000 | 10.006 | 8.814 | 10.000 | 10.000 | 10.000 | +0.00% |
| SAN7.JT | SurfaceRadarLong / 120 | 25.000 | 25.002 | 15.540 | 25.004 | 25.004 | 25.004 | +0.02% |
| SAN9.JT | SurfaceRadar / 70 | 12.000 | 12.003 | 10.158 | 12.000 | 12.000 | 12.000 | +0.00% |
| SEA_SPAR.JT | SurfaceRadarLong / 120 | 26.000 | 26.000 | 18.100 | 26.006 | 26.006 | 26.006 | +0.02% |
| SSN9.JT | Cruise / 600 | 150.000 | 149.954 | 153.634 | 144.274 | 151.120 | 151.120 | +0.75% |

## Family, role and altitude checks

Family margins use all twelve altitude/aspect cells. Short-IR comparisons
use the three 5k/15k/35k tail cells. Low-altitude radar role comparisons use
5k head and tail. These checks do not assert universal real-world ordering.

| Newer / older | Minimum measured advantage |
| --- | ---: |
| AIM7.JT / AIM7E.JT | 2.529% |
| R27ER.JT / AA10.JT | 17.019% |
| R27ET.JT / R27T.JT | 34.280% |
| AIM9M.JT / AIM9B.JT | 0.175% |
| AIM9M.JT / AA2.JT | 19.647% |
| AIM9M.JT / AA8.JT | 146.676% |
| AIM9X.JT / AIM9B.JT | 0.388% |
| AIM9X.JT / AA2.JT | 19.901% |
| AIM9X.JT / AA8.JT | 147.112% |
| AA11.JT / AIM9B.JT | 101.540% |
| AA11.JT / AA2.JT | 151.785% |
| AA11.JT / AA8.JT | 242.744% |

| Active radar / older group | Smallest 5k head margin km | Smallest 5k tail margin km |
| --- | ---: | ---: |
| AIM120.JT | 14.366 | 4.844 |
| AA12.JT | 19.885 | 3.229 |
| MICA.JT | 0.741 | 0.316 |

| Store | H5/H35 | T5/H35 | T5 class floor km | 1.2 x decoded min km |
| --- | ---: | ---: | ---: | ---: |
| AA10.JT | 0.398 | 0.111 | 3 | 4.389 |
| AA11.JT | 0.560 | 0.290 | 1.5 | 0.732 |
| AA11B.JT | 0.560 | 0.290 | 1.5 | 0.549 |
| AA12.JT | 0.877 | 0.201 | 3 | 4.389 |
| AA2.JT | 0.423 | 0.150 | 1.5 | 1.463 |
| AA6.JT | 0.400 | 0.188 | 3 | 4.389 |
| AA8.JT | 0.857 | 0.172 | 1.5 | 1.463 |
| AA9.JT | 0.353 | 0.057 | 5 | 4.389 |
| AAML.JT | 0.292 | 0.104 | 5 | 10.973 |
| AEMP1.JT | 0.864 | 0.201 | 3 | 0.732 |
| AIM120.JT | 0.669 | 0.201 | 3 | 4.389 |
| AIM54C.JT | 0.347 | 0.073 | 5 | 10.973 |
| AIM7.JT | 0.310 | 0.118 | 3 | 2.926 |
| AIM7E.JT | 0.393 | 0.178 | 3 | 1.097 |
| AIM9B.JT | 0.576 | 0.204 | 1.5 | 1.463 |
| AIM9M.JT | 0.886 | 0.330 | 1.5 | 1.463 |
| AIM9X.JT | 0.885 | 0.330 | 1.5 | 1.097 |
| MICA.JT | 0.442 | 0.126 | 3 | 4.389 |
| PL10.JT | 0.711 | 0.174 | 3 | 2.926 |
| PL7.JT | 0.944 | 0.307 | 1.5 | 1.463 |
| R27ER.JT | 0.427 | 0.072 | 5 | 4.389 |
| R27ET.JT | 0.353 | 0.122 | 5 | 4.389 |
| R27T.JT | 0.357 | 0.153 | 3 | 4.389 |
| R530.JT | 0.335 | 0.164 | 3 | 2.926 |
| R550.JT | 0.936 | 0.315 | 1.5 | 1.463 |

The decoded minimum removes these nominal physical employment intervals:

- AA9.JT 500 ft tail: 2.540 km <= minimum 3.658 km
- AIM54C.JT 500 ft beam: 6.276 km <= minimum 9.144 km
- AIM54C.JT 500 ft tail: 2.393 km <= minimum 9.144 km

These are range-policy exclusions, not zero kinetic reach. The floor check
uses 5k tail, not the 500 ft deck. Altitude reversals above 10%: none.

## Defeat and force diagnostics

The offline `--diagnostics` mode opts into available/required G, positive
G deficit, closing speed, Mach, control expiry and post-burn defeat at each
measured edge, 0.1% beyond and 10% beyond. These values do not create a G-based death rule.
Raw commanded G can exceed available G; applied steering stays bounded.
Ordinary motion and prediction skip diagnostic-only calculations.
The behaviour tripwire includes low-Mach countdown, reset, defeat transition
and inert contacts. Dedicated tests cover the separate warning publication rule.

## Motor-shape audit

Phase 2.8 closes the weak-sustain loophole with authored data and offline guards.
A2A and surface air-target stores require at least 1 g sustain thrust on mass at
sustain entry, where that ratio is lowest, or no sustain phase. Their combined
burn time is at most half the fixed class budget. Ignition is separate. Ground
stores are not subject to those two air-target limits because they do not use
post-burn Mach defeat. Full parameters have one home in
[the fitted data table](../formats/missile-energy-data.md#per-store-motors-and-class-control-budgets).

Every store is listed below, including unchanged records. A dash in the cap
column means the air-target burn cap does not apply. Initial acceleration is
still checked by the existing class floors. The Mach-one probe is the same
250 m/s, 5,000 ft launch for A2A and SAM records; it does not represent a
stationary SAM launch. No crossing within its 20 seconds is shown as None.

| Store | Class | Burn s | Control s | Burn/control | Air burn cap s | Sustain-entry g | Mach-one s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| AA10.JT | MediumAir | 34.558 | 70 | 49.37% | 35 | 2.603 | 0.358 |
| AA11.JT | ShortIr | 4.733 | 20 | 23.67% | 10 | 34.563 | 0.292 |
| AA11B.JT | ShortIr | 4.733 | 20 | 23.67% | 10 | 34.563 | 0.292 |
| AA12.JT | MediumAir | 26.433 | 70 | 37.76% | 35 | 1.044 | 1.917 |
| AA2.JT | ShortIr | 1.758 | 20 | 8.79% | 10 | 33.702 | 0.350 |
| AA6.JT | MediumAir | 4.742 | 70 | 6.77% | 35 | 33.034 | 1.383 |
| AA8.JT | ShortIr | 7.092 | 20 | 35.46% | 10 | 1.282 | 2.917 |
| AA9.JT | LongAir | 69.000 | 180 | 38.33% | 90 | 1.089 | 2.958 |
| AAML.JT | LongAir | 37.792 | 180 | 21.00% | 90 | 2.661 | 2.558 |
| AEMP1.JT | MediumAir | 17.458 | 70 | 24.94% | 35 | 1.629 | 2.758 |
| AGM45.JT | FastGround | 8.000 | 90 | 8.89% | - | 6.342 | 0.450 |
| AGM65A.JT | FastGround | 8.000 | 90 | 8.89% | - | 5.659 | 1.533 |
| AGM65G.JT | FastGround | 8.000 | 90 | 8.89% | - | 5.428 | 1.550 |
| AGM84A.JT | Cruise | 962.000 | 600 | 160.33% | - | 0.623 | None |
| AGM84E.JT | ShortCruise | 298.883 | 300 | 99.63% | - | 0.466 | None |
| AGM88.JT | FastGround | 8.000 | 90 | 8.89% | - | 7.560 | 0.392 |
| AIM120.JT | MediumAir | 15.225 | 70 | 21.75% | 35 | 2.105 | 0.817 |
| AIM54C.JT | LongAir | 77.092 | 180 | 42.83% | 90 | 1.540 | 2.650 |
| AIM7.JT | MediumAir | 16.000 | 70 | 22.86% | 35 | 2.941 | 1.142 |
| AIM7E.JT | MediumAir | 5.950 | 70 | 8.50% | 35 | 14.037 | 1.350 |
| AIM9B.JT | ShortIr | 2.200 | 20 | 11.00% | 10 | 0.000 | 0.367 |
| AIM9M.JT | ShortIr | 5.892 | 20 | 29.46% | 10 | 7.655 | 0.858 |
| AIM9X.JT | ShortIr | 6.008 | 20 | 30.04% | 10 | 4.859 | 0.942 |
| AM39.JT | ShortCruise | 299.000 | 300 | 99.67% | - | 0.363 | None |
| AS14.JT | ShortGround | 7.342 | 30 | 24.47% | - | 4.518 | 1.808 |
| AS15.JT | FastGround | 8.000 | 90 | 8.89% | - | 2.507 | 2.250 |
| AS16.JT | BoostGlide | 113.883 | 600 | 18.98% | - | 0.924 | 3.400 |
| AS30.JT | ShortGround | 20.975 | 30 | 69.92% | - | 1.275 | 3.767 |
| AS7.JT | ShortGround | 6.125 | 30 | 20.42% | - | 4.019 | 1.850 |
| AT12.JT | ShortGround | 14.525 | 30 | 48.42% | - | 0.426 | None |
| AT2.JT | Command | 8.000 | 30 | 26.67% | - | 2.162 | None |
| FIM92.JT | SurfaceIr | 11.000 | 25 | 44.00% | 12.5 | 3.352 | 0.817 |
| MICA.JT | MediumAir | 27.975 | 70 | 39.96% | 35 | 2.395 | 1.992 |
| MIS.JT | SurfaceRadar | 11.000 | 70 | 15.71% | 35 | 3.887 | 0.700 |
| PL10.JT | MediumAir | 11.158 | 70 | 15.94% | 35 | 1.107 | 2.775 |
| PL7.JT | ShortIr | 10.000 | 20 | 50.00% | 10 | 2.818 | 1.617 |
| R27ER.JT | LongAir | 82.808 | 180 | 46.00% | 90 | 1.399 | 0.600 |
| R27ET.JT | LongAir | 6.150 | 180 | 3.42% | 90 | 4.370 | 0.608 |
| R27T.JT | MediumAir | 3.167 | 70 | 4.52% | 35 | 16.906 | 0.225 |
| R440.JT | SurfaceRadar | 4.050 | 70 | 5.79% | 35 | 27.645 | 0.350 |
| R530.JT | MediumAir | 7.692 | 70 | 10.99% | 35 | 13.744 | 0.350 |
| R550.JT | ShortIr | 4.492 | 20 | 22.46% | 10 | 6.280 | 1.000 |
| SA13.JT | SurfaceIr | 11.000 | 25 | 44.00% | 12.5 | 3.019 | 0.867 |
| SA14.JT | SurfaceIr | 11.000 | 25 | 44.00% | 12.5 | 3.323 | 0.808 |
| SA15.JT | SurfaceRadar | 11.000 | 70 | 15.71% | 35 | 5.019 | 0.558 |
| SA16.JT | SurfaceIr | 11.000 | 25 | 44.00% | 12.5 | 3.540 | 0.767 |
| SA19.JT | SurfaceRadar | 11.000 | 70 | 15.71% | 35 | 5.540 | 0.525 |
| SA2A.JT | SurfaceRadarLong | 11.000 | 120 | 9.17% | 60 | 5.582 | 0.500 |
| SA3.JT | SurfaceRadarLong | 11.000 | 120 | 9.17% | 60 | 4.889 | 0.558 |
| SA6.JT | SurfaceRadar | 11.000 | 70 | 15.71% | 35 | 5.851 | 0.483 |
| SA7.JT | SurfaceIr | 11.000 | 25 | 44.00% | 12.5 | 3.384 | 0.808 |
| SA9.JT | SurfaceIr | 11.000 | 25 | 44.00% | 12.5 | 3.119 | 0.858 |
| SAN11.JT | SurfaceRadar | 11.000 | 70 | 15.71% | 35 | 5.540 | 0.525 |
| SAN3.JT | SurfaceRadarLong | 11.000 | 120 | 9.17% | 60 | 6.952 | 0.425 |
| SAN4.JT | SurfaceRadar | 11.000 | 70 | 15.71% | 35 | 4.483 | 0.608 |
| SAN7.JT | SurfaceRadarLong | 11.000 | 120 | 9.17% | 60 | 5.640 | 0.500 |
| SAN9.JT | SurfaceRadar | 11.000 | 70 | 15.71% | 35 | 5.019 | 0.558 |
| SEA_SPAR.JT | SurfaceRadarLong | 11.000 | 120 | 9.17% | 60 | 6.081 | 0.475 |
| SSN9.JT | Cruise | 600.000 | 600 | 100.00% | - | 0.314 | 0.742 |

### Changed air-target motors

The comparison is Phase 2.7 to Phase 2.8. The Mach-one probe and reference
conditions are identical. Per-store fuel, thrust and duration remain fitted
game coefficients. Class times and the defeat threshold/dwell are fixed
inputs, but defeat does enter the optimizer through the shared reach kernel.
These are not engineering performance or retail-parity claims.

| Store | Burn s before / after | H35 km before / after | Mach-one s before / after | Boost kN / s after | Sustain kN / s after |
| --- | ---: | ---: | ---: | --- | --- |
| AA10.JT | 56.833 / 34.558 | 67.872 / 64.734 | 1.133 / 0.358 | 63.091 / 1.083 | 5.951 / 33.475 |
| AA12.JT | 69.992 / 26.433 | 50.034 / 52.045 | 1.025 / 1.917 | 13.999 / 1.008 | 1.713 / 25.425 |
| AA8.JT | 19.967 / 7.092 | 12.987 / 13.199 | 1.033 / 2.917 | 3.894 / 0.767 | 0.521 / 6.325 |
| AA9.JT | 140.325 / 69.000 | 120.007 / 120.034 | 2.808 / 2.958 | 51.245 / 1.475 | 5.011 / 67.525 |
| AAML.JT | 60.658 / 37.792 | 150.018 / 150.000 | 1.083 / 2.558 | 46.019 / 0.550 | 12.913 / 37.242 |
| AEMP1.JT | 49.600 / 17.458 | 50.027 / 52.148 | 1.092 / 2.758 | 16.134 / 0.650 | 2.732 / 16.808 |
| AIM120.JT | 50.842 / 15.225 | 60.000 / 60.000 | 0.967 / 0.817 | 16.918 / 2.008 | 2.938 / 13.217 |
| AIM9X.JT | 12.900 / 6.008 | 18.631 / 18.029 | 0.650 / 0.942 | 7.972 / 3.650 | 3.501 / 2.358 |
| MICA.JT | 67.233 / 27.975 | 55.239 / 60.000 | 1.108 / 1.992 | 8.954 / 0.992 | 2.553 / 26.983 |
| PL10.JT | 59.858 / 11.158 | 50.088 / 50.096 | 3.000 / 2.775 | 20.738 / 1.600 | 1.744 / 9.558 |
| PL7.JT | 16.267 / 10.000 | 15.695 / 15.691 | 3.000 / 1.617 | 7.000 / 0.858 | 2.396 / 9.142 |

### What limits the A2A edges

Each cell is head / beam / tail. The code describes the result of a shot
0.1% beyond the sampled physical edge: D is kinetic defeat, C is control
expiry, G is ground contact, L is removal lifetime, O is another stop, and H
means the probe still hits. Combined flags are shown together. This identifies
the observed limiting event near the edge, not a proof of a continuous range
boundary. The additional 10%-beyond probe remains in the raw diagnostics.
The last successful shot can be briefly subsonic during the 30-tick defeat
dwell; the rule does not defeat a store on its first subsonic tick.

| Store | 500 ft | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- | --- |
| AA10.JT | D / D / D | D / D / D | D / D / D | C / D / D |
| AA11.JT | D / D / D | D / D / D | D / D / D | C / C / C |
| AA11B.JT | D / D / D | D / D / D | D / D / D | C / C / C |
| AA12.JT | D / D / D | C / D / D | C / C / C | C / C / C |
| AA2.JT | D / D / D | D / D / D | D / D / D | C / C / C |
| AA6.JT | D / D / D | D / D / D | D / D / D | D / D / D |
| AA8.JT | D / H / D | D / D / D | C / C / C | C / C / C |
| AA9.JT | G / G / G | D / D / D | D / D / D | D / D / D |
| AAML.JT | D / D / G | D / D / D | D / D / D | C / D / D |
| AEMP1.JT | D / D / D | D / D / D | C / C / C | C / C / C |
| AIM120.JT | D / D / D | D / D / D | C / D / D | C / C / C |
| AIM54C.JT | G / G / G | D / D / D | D / D / D | D / D / D |
| AIM7.JT | D / D / D | D / D / D | D / D / D | C / D / D |
| AIM7E.JT | D / D / D | D / D / D | D / D / D | D / D / D |
| AIM9B.JT | D / D / D | D / D / D | D / D / D | C / C / C |
| AIM9M.JT | C / C / C | C / C / C | C / C / C | C / C / C |
| AIM9X.JT | C / C / C | C / C / C | C / C / C | C / C / C |
| MICA.JT | D / D / D | D / D / D | D / D / D | C / C / C |
| PL10.JT | D / G / G | D / D / D | C / D / D | C / C / C |
| PL7.JT | C / C / C | C / C / C | C / C / C | C / C / C |
| R27ER.JT | D / D / D | D / D / D | D / D / D | D / D / D |
| R27ET.JT | D / D / D | D / D / D | D / D / D | D / D / D |
| R27T.JT | D / D / D | D / D / D | D / D / D | C / C / C |
| R530.JT | D / D / D | D / D / D | D / D / D | D / D / D |
| R550.JT | C / C / C | C / C / C | C / C / C | C / C / C |

Across the 300 near-edge A2A shots: C 96, D 194, G 9, H 1. The H case is
AA8 at 500 ft, beam: the 2.077583 km shot still hits, about 2 m beyond the
sampled edge. At 10% beyond it is defeated at 9.992 s. This is a documented
sampling limitation; the report does not claim a precisely closed boundary
for that cell. The wider probe establishes kinetic defeat, not control expiry.

### Powered cruise limit and speed

John approved shared shorter cruise classes in Phase 2.8. Before fitting, the
agent assigned AM39 and AGM84E to ShortCruise 300 seconds. AGM84A and SSN9
retain Cruise 600 seconds. No timer is fitted per store. The guard requires
powered controlled hits at the reference, half and three quarters of it, and
three quarters of the measured edge and the edge itself, all at Mach 0.70..0.95.
The powered limit is independently searched using the powered-hit predicate.
All distances below are initial separation, not distance travelled.

| Store | Reference km | Phase 2.7 powered limit / edge km | Current powered limit / edge km | Half-reference Mach | Reference Mach | Edge Mach | Burnout / edge flight s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| AGM84A.JT | 185 | 186.130 / 186.128 | 186.128 / 186.128 | 0.915 | 0.929 | 0.929 | 964.000 / 600.008 |
| AGM84E.JT | 93 | 93.138 / 99.166 | 93.000 / 93.000 | 0.897 | 0.910 | 0.910 | 300.883 / 300.008 |
| AM39.JT | 70 | 70.009 / 76.289 | 71.497 / 71.497 | 0.701 | 0.705 | 0.705 | 301.000 / 300.008 |
| SSN9.JT | 150 | 151.165 / 151.155 | 151.155 / 151.155 | 0.820 | 0.822 | 0.822 | 600.000 / 599.958 |

The prior powered limits are the review7 measurements. Current limits use the
same physical kernel and standardized range conditions. Search resolution
can differ by a few meters; the exact measured edge is also checked directly
for a powered hit. The speed checks are samples, not continuous-flight bounds.

AS16 is explicitly BoostGlide 600 seconds, with unchanged motor, normal drag
and control duration. It is a boost-glide ground rocket, exempt from powered
cruise requirements. Ground glide completion remains an intentional behavior.

### Accepted boundaries and remaining approximations

AT12 retains its 550 m/s impulse coefficient, an accepted endpoint from the
old search and below the current staged rocket search's 600 m/s lower bound.
It still satisfies the common 400..6600 m/s verification guard. Its identity
is unresolved; this is an authored game value, not a sourced exhaust velocity.
The earlier claim that no coefficient had an exception was too broad.

The bounded range exceptions remain explicit in the reference table above.
John approved an AA8 head-only ceiling of 132.5% in Phase 2.8 after the guarded
search missed its former 130% ceiling. Its tail band is unchanged. John's
PL7 +/-12.5% decision and every other reference band are unchanged.
The air-target burn cap and sustain floor have no per-store exceptions.
Retained drag ceilings and fitted coefficients remain calibration choices;
all exact values and phase provenance are in the data table. AA11/AA11B's
retained 53.490 dry-mass g is close to the staged 53.5 g ceiling, an accepted
agent fit choice inside the 54 g guard and 10.85% below the old 60 g limit.
AGM84A retains a declared 25000 m/s coefficient with an explicit engine,
whose actual impulse/fuel coefficient is 11757.778 m/s. Both are inside the
sustain-only engine guard. Long cruise burn beyond control expiry is allowed;
no such exception applies to the air-target half-time cap.

Current search endpoints are accepted fitted choices within the common guard:

- AA11.JT: drag 2.999764.
- AA11B.JT: drag 2.999764.
- AA12.JT: drag 0.500154.
- AA2.JT: fuel 0.499927.
- AA6.JT: fuel 0.499395, drag 2.999740.
- AA8.JT: fuel 0.120000.
- AEMP1.JT: drag 0.500000.
- AGM84A.JT: fuel 0.500000.
- AIM7.JT: drag 3.000000.
- AIM7E.JT: fuel 0.500000.
- AIM9B.JT: fuel 0.120000, drag 3.000000.
- AIM9M.JT: fuel 0.120425.
- AS16.JT: drag 0.500000.
- AT12.JT: drag 3.000000, coefficient 550.000000.
- AT2.JT: fuel 0.120000, drag 3.000000.
- PL7.JT: fuel 0.120034.
- R440.JT: fuel 0.500000.

## Validation status

Phase 2.8, Linux x86-64, based on Phase 2.7 commit 138c94bc. Earlier
reviewer commit efcd54db and its observer-picture fix are preserved.
Model identity 2060610122284599500, behavior trace 3894057758776089527. The new motor data changes
the model identity automatically; runtime rule revisions are unchanged.
The complete workspace passes 7,219 tests with 106 intentionally
ignored. Four separate example tests cover inverted/weak motors, coefficient
and dry-mass guards, slow acceleration including SAMs, sustain-only cruise,
current-mass sustain floors, the half-time burn cap and bounded reference
exceptions. The class-data and behavior-identity tests also pass.
Workspace build, clippy with warnings denied, formatting, documentation,
518 Python tests (five skipped) and source/app/extractor asset checks pass.

All 59 reference checks pass their declared bands, motor-shape and applicable
cruise-arrival checks. Tail floors, decoded-minimum margins, family and radar
role ordering, and modern-IR deck escape checks pass. Maximum reach change
under a 2% perturbation is 4.570%. The complete 391-row range sweep,
1002-row edge/near-edge/beyond diagnostic sweep and 59-row burn/control audit
were regenerated. The behavior golden pair was updated after reviewing the
data-only change. The accepted ShortCruise assignments keep AM39 and AGM84E
within their reference bands while powered through the full edge. AS16 retains its motor
and 600-second budget under the explicit BoostGlide class.

All 36 battery scenarios pass in one complete run. This includes the moving
SAM targets, Energy/worker comparison, historical Spec shots, inert near-pass
checks, player range-cue scenarios and five-fight launch-count regression.
The reviewer observer-picture fix is retained and the workspace tests cover it.
No runtime physics, guidance, AI firing policy or defeat rule changed.
The two ShortCruise class assignments are the approved control-budget change.

Windows/macOS were not run on this Linux host. Retail comparison is unavailable.
The named range residuals, sampled-edge uncertainty and fitted motor values
above remain approximations. The invalid Phase 3 study stays stopped and labelled pre-fix. This round does not merge.

## Natural launch counts against both baselines

The accepted gate remains a one-sided 70% floor against pre-overhaul Spec and
a symmetric 70%..130% band against reviewed Phase 2.5. There is no Spec upper
ceiling. The table uses the sequential cost lanes; the battery independently
checks the same five fights. No firing or admission policy was adjusted.

| Fight | Pre-overhaul Spec | Phase 2.5 | Phase 2.8 | Change vs Spec | Change vs 2.5 | Spec floor | 2.5 band |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| UKR | 11 | 13 | 15 | +36.36% | +15.38% | 7.7 | 9.1..16.9 |
| BAL | 12 | 13 | 14 | +16.67% | +7.69% | 8.4 | 9.1..16.9 |
| SPA | 12 | 13 | 14 | +16.67% | +7.69% | 8.4 | 9.1..16.9 |
| GRE | 12 | 13 | 14 | +16.67% | +7.69% | 8.4 | 9.1..16.9 |
| 23-AI UKR | 24 | 26 | 28 | +16.67% | +7.69% | 16.8 | 18.2..33.8 |

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
| natural-ukr | 0.829 / 1.035 | 0.706 / 0.891 | 0.982 | 0 / 0 | 15 |
| natural-bal | 0.819 / 0.941 | 0.036 / 0.897 | 0.938 | 0 / 0 | 14 |
| natural-spa | 0.830 / 0.935 | 0.034 / 0.897 | 0.933 | 0 / 0 | 14 |
| natural-gre | 0.843 / 0.972 | 0.037 / 0.906 | 0.971 | 0 / 0 | 14 |
| natural-big | 0.905 / 0.974 | 0.868 / 0.986 | 0.983 | 0 / 0 | 28 |
| turning-3 | 0.918 / 1.017 | 0.916 / 1.009 | 1.012 | 0 / 0 | 13 |
| turning-10 | 0.907 / 0.956 | 0.920 / 1.035 | 1.021 | 0 / 0 | 9 |
| hold-frozen | 1.034 / 1.127 | 0.374 / 1.088 | 1.115 | 0 / 0 | 44 |
| hold-moving | 1.036 / 1.214 | 0.904 / 1.067 | 1.212 | 0 / 0 | 44 |
| hold-cruise | 0.798 / 0.915 | 0.820 / 0.933 | 0.928 | 0 / 0 | 16 |

Worst wall p95 is 1.214 ms and worst CPU p95 is 1.212 ms, below the
2 ms gate. All ten lanes record zero refused and zero starved requests.
