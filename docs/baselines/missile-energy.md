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

## Conditions and limits

A2A launcher velocity is 300 m/s, target 250 m/s, wings level, no wind or
jammer, flat sea-level ground. Altitudes are MSL. Head, beam and tail mean
180, 90 and 0 degrees target heading relative to launch heading. All distances
below are km of initial separation, not missile path length. Numbers are
rounded to 0.01 km. The sweep samples ranges by factor 1.5 from 100 m to
1000 km, then bisects the last successful interval ten times. It can miss a
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

The public anchor is conditional and its provenance lives in the
[data evidence](../formats/missile-energy-data.md). Most A2A fitting uses
35,000 ft head-on, as requested. Rear-only AIM9B and R3S instead use tail shots:
B's public 4.8 km is explicitly a tail figure; the R3S tail condition is an
agent fit. Higher physical head-on reach does not grant these seekers a
head-on lock. The late IR R27 variants illustrate why physical reach and
seeker reach must be reported separately.

## Complete A2A physical reach

Each cell is head / beam / tail, km. All 25 inventory records are included,
including held records measured diagnostically without enabling them.

| Store | Public/class anchor km | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | ---: | --- | --- | --- |
| AA10.JT | 73 | 25.70 / 7.75 / 5.04 | 32.37 / 10.62 / 7.16 | 73.00 / 20.44 / 14.44 |
| AA11.JT | 30 | 13.97 / 3.93 / 2.55 | 18.00 / 5.38 / 3.62 | 29.99 / 10.31 / 7.34 |
| AA11B.JT | 30 | 13.97 / 3.93 / 2.55 | 18.00 / 5.38 / 3.62 | 29.99 / 10.31 / 7.34 |
| AA12.JT | 50 | 18.29 / 5.26 / 3.40 | 23.30 / 7.21 / 4.86 | 49.99 / 13.86 / 9.87 |
| AA2.JT | 7.6 | 14.08 / 3.96 / 2.72 | 18.03 / 5.32 / 3.74 | 30.02 / 10.34 / 7.60 |
| AA6.JT | 30 | 14.44 / 4.04 / 2.57 | 18.39 / 5.54 / 3.66 | 29.99 / 10.53 / 7.41 |
| AA8.JT | 10 | 4.51 / 0.89 / 0.57 | 5.71 / 1.40 / 0.87 | 10.00 / 2.88 / 2.01 |
| AA9.JT | 120 | 42.53 / 14.33 / 9.21 | 52.51 / 19.44 / 12.86 | 119.98 / 36.73 / 25.28 |
| AAML.JT | 150 | 56.15 / 18.68 / 12.21 | 100.50 / 25.34 / 16.97 | 149.95 / 48.94 / 33.87 |
| AEMP1.JT | 50 | 18.28 / 5.19 / 3.38 | 23.28 / 7.14 / 4.85 | 49.99 / 13.57 / 9.85 |
| AIM120.JT | 60 | 21.78 / 6.42 / 4.20 | 27.54 / 8.79 / 5.97 | 60.00 / 16.91 / 12.07 |
| AIM54C.JT | 150 | 52.79 / 18.54 / 11.98 | 101.85 / 25.01 / 16.60 | 149.95 / 46.95 / 32.36 |
| AIM7.JT | 70 | 24.38 / 7.45 / 4.76 | 45.82 / 10.20 / 6.77 | 69.98 / 19.27 / 13.54 |
| AIM7E.JT | 35 | 17.28 / 5.00 / 3.26 | 21.76 / 6.76 / 4.56 | 34.99 / 12.89 / 9.20 |
| AIM9B.JT | 4.8 | 11.78 / 0.00 / 1.65 | 15.09 / 3.31 / 2.30 | 20.34 / 6.53 / 4.80 |
| AIM9M.JT | 18 | 8.28 / 2.13 / 1.36 | 10.67 / 2.96 / 2.02 | 18.00 / 5.65 / 4.17 |
| AIM9X.JT | 18 | 8.27 / 2.12 / 1.36 | 10.66 / 2.94 / 2.01 | 18.00 / 5.65 / 4.15 |
| MICA.JT | 60 | 21.95 / 6.48 / 4.20 | 27.84 / 8.87 / 5.97 | 60.00 / 17.05 / 12.08 |
| PL10.JT | 50 | 18.30 / 5.19 / 3.26 | 22.86 / 7.17 / 4.68 | 49.99 / 13.64 / 9.50 |
| PL7.JT | 14 | 6.30 / 1.55 / 0.94 | 8.08 / 2.16 / 1.39 | 13.99 / 4.24 / 3.02 |
| R27ER.JT | 117 | 43.70 / 14.47 / 9.54 | 54.01 / 19.63 / 13.27 | 117.00 / 37.87 / 26.49 |
| R27ET.JT | 104 | 54.46 / 18.85 / 12.68 | 66.71 / 25.19 / 17.24 | 103.96 / 48.92 / 34.31 |
| R27T.JT | 63 | 31.84 / 10.06 / 6.72 | 39.70 / 13.45 / 9.18 | 62.99 / 26.24 / 18.48 |
| R530.JT | 40 | 14.58 / 4.06 / 2.59 | 18.70 / 5.60 / 3.74 | 40.00 / 10.76 / 7.65 |
| R550.JT | 15 | 6.74 / 1.69 / 1.02 | 8.67 / 2.35 / 1.52 | 15.00 / 4.59 / 3.26 |

## Complete A2A seeker-valid reach

| Store | 5,000 ft | 15,000 ft | 35,000 ft |
| --- | --- | --- | --- |
| AA10.JT | 25.70 / 7.75 / 5.04 | 32.37 / 10.62 / 7.16 | 70.50 / 20.44 / 14.44 |
| AA11.JT | 11.56 / 3.92 / 2.55 | 11.56 / 5.37 / 3.63 | 11.56 / 10.29 / 7.34 |
| AA11B.JT | 11.56 / 3.92 / 2.55 | 11.56 / 5.37 / 3.63 | 11.56 / 10.29 / 7.34 |
| AA12.JT | 18.29 / 5.24 / 3.40 | 23.29 / 7.19 / 4.86 | 49.99 / 13.83 / 9.87 |
| AA2.JT | 0.00 / 0.00 / 2.72 | 0.00 / 0.00 / 3.74 | 0.00 / 0.00 / 7.60 |
| AA6.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AA8.JT | 4.51 / 0.00 / 0.57 | 5.71 / 1.33 / 0.87 | 5.78 / 2.85 / 2.02 |
| AA9.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| AAML.JT | 56.15 / 18.65 / 12.21 | 100.50 / 25.30 / 16.97 | 149.95 / 48.86 / 33.87 |
| AEMP1.JT | 18.28 / 5.18 / 3.38 | 23.28 / 7.12 / 4.85 | 49.99 / 13.51 / 9.85 |
| AIM120.JT | 21.78 / 6.41 / 4.20 | 27.54 / 8.78 / 5.97 | 60.00 / 16.86 / 12.07 |
| AIM54C.JT | 52.77 / 18.54 / 11.98 | 101.85 / 24.99 / 16.60 | 149.95 / 46.91 / 32.37 |
| AIM7.JT | 24.38 / 7.45 / 4.76 | 45.73 / 10.20 / 6.77 | 69.05 / 19.27 / 13.54 |
| AIM7E.JT | 17.28 / 5.00 / 3.26 | 21.76 / 6.76 / 4.56 | 34.78 / 12.87 / 9.20 |
| AIM9B.JT | 0.00 / 0.00 / 1.65 | 0.00 / 0.00 / 2.30 | 0.00 / 0.00 / 4.80 |
| AIM9M.JT | 8.28 / 2.09 / 1.36 | 9.64 / 2.92 / 2.02 | 9.64 / 5.62 / 4.17 |
| AIM9X.JT | 8.27 / 2.08 / 1.36 | 9.64 / 2.91 / 2.01 | 9.64 / 5.63 / 4.15 |
| MICA.JT | 21.95 / 6.46 / 4.20 | 27.84 / 8.86 / 5.98 | 60.00 / 17.02 / 12.08 |
| PL10.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| PL7.JT | N/A / N/A / N/A | N/A / N/A / N/A | N/A / N/A / N/A |
| R27ER.JT | 43.70 / 14.47 / 9.54 | 54.01 / 19.63 / 13.27 | 115.61 / 37.87 / 26.49 |
| R27ET.JT | 11.56 / 15.30 / 12.68 | 11.56 / 15.30 / 17.25 | 11.56 / 15.30 / 18.28 |
| R27T.JT | 11.56 / 10.04 / 6.72 | 11.56 / 13.44 / 9.18 | 11.56 / 15.30 / 18.28 |
| R530.JT | 14.58 / 4.06 / 2.59 | 18.70 / 5.60 / 3.74 | 39.98 / 10.74 / 7.65 |
| R550.JT | 6.73 / 1.65 / 1.02 | 8.67 / 2.31 / 1.52 | 9.64 / 4.57 / 3.26 |

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
| AGM84E.JT | 93 | 1500/10 | 92.98 | 91.41 |
| AGM88.JT | 48.28032 | 1500/10 | 48.28 | 48.28 |
| AM39.JT | 70 | 1500/10 | 69.98 | 69.98 |
| AS14.JT | 10 | 1500/10 | 10.00 | N/A |
| AS15.JT | 18.288 retail | 1500/10 | 18.28 | 18.28 |
| AS16.JT | 150 | 1500/10 | 149.95 | 149.95 |
| AS30.JT | 11 | 1500/10 | 11.00 | N/A |
| AS7.JT | 10 | 1500/10 | 10.00 | 10.00 |
| AT12.JT | 7.9248 retail | 1500/10 | 7.92 | N/A |
| AT2.JT | 4 | 100/10 | 4.00 | N/A |
| FIM92.JT | 4 | 10/800 | 7.64 / 1.41 / 0.66 / 4.00 | 0.00 / 0.00 / 0.00 / 4.00 |
| MIS.JT | 6.5 | 10/1300 | 11.95 / 2.57 / 1.38 / 6.50 | 0.00 / 0.00 / 0.00 / 6.50 |
| R440.JT | 10 | 10/2000 | 16.93 / 5.06 / 3.15 / 10.00 | 1.79 / 0.00 / 0.00 / 10.00 |
| SA13.JT | 5 | 10/1000 | 9.35 / 1.84 / 0.95 / 5.00 | 4.75 / 1.83 / 0.95 / 5.00 |
| SA14.JT | 4.5 | 10/900 | 8.54 / 1.65 / 0.81 / 4.50 | 3.79 / 1.65 / 0.81 / 4.50 |
| SA15.JT | 12 | 10/2400 | 20.72 / 5.43 / 3.22 / 12.00 | 20.72 / 5.43 / 3.22 / 12.00 |
| SA16.JT | 5.2 | 10/1040 | 9.75 / 1.98 / 1.01 / 5.20 | 2.77 / 1.97 / 1.01 / 4.44 |
| SA19.JT | 8 | 10/1600 | 14.51 / 3.33 / 1.83 / 8.00 | 14.51 / 3.33 / 1.83 / 8.00 |
| SA2A.JT | 29 | 10/3000 | 50.55 / 14.68 / 9.43 / 28.99 | 0.00 / 0.00 / 0.00 / 28.99 |
| SA3.JT | 25 | 10/3000 | 42.55 / 12.87 / 8.35 / 24.99 | 0.00 / 0.00 / 0.00 / 24.99 |
| SA6.JT | 24 | 10/3000 | 41.11 / 12.13 / 7.78 / 24.00 | 12.06 / 6.21 / 3.44 / 24.00 |
| SA7.JT | 4.2 | 10/840 | 8.02 / 1.53 / 0.73 / 4.20 | 2.82 / 1.53 / 0.72 / 4.20 |
| SA9.JT | 4.2 | 10/840 | 8.00 / 1.51 / 0.72 / 4.20 | 3.80 / 1.51 / 0.72 / 4.20 |
| SAN11.JT | 8 | 10/1600 | 14.51 / 3.33 / 1.83 / 8.00 | 14.51 / 3.33 / 1.83 / 8.00 |
| SAN3.JT | 30 | 10/3000 | 52.77 / 15.02 / 9.61 / 29.99 | 53.95 / 15.02 / 9.61 / 29.99 |
| SAN4.JT | 10 | 10/2000 | 17.59 / 4.33 / 2.51 / 10.00 | 17.59 / 4.33 / 2.51 / 10.00 |
| SAN7.JT | 25 | 10/3000 | 43.26 / 12.48 / 7.95 / 24.99 | 43.32 / 12.48 / 7.95 / 24.99 |
| SAN9.JT | 12 | 10/2400 | 20.72 / 5.43 / 3.22 / 12.00 | 20.72 / 5.43 / 3.22 / 12.00 |
| SEA_SPAR.JT | 26 | 10/3000 | 44.73 / 13.22 / 8.51 / 26.00 | 44.86 / 13.22 / 8.51 / 26.00 |
| SSN9.JT | 150 | 10/10 | 149.95 | 149.95 |

AS15 and AT12 retain runtime decoded mass/range and the uniform class diameter
because their real-world identity is unresolved. Their data-only fallback
drag fit meets that retail range anchor; this is not real-world calibration.
The SAM aspect allowance continues the previous game policy where the
original aspect field has no reviewed interpretation. In particular, this
is not a claim that every real infrared SAM is all-aspect.

## Validation status

Phase 2 implementation and regression validation completed on Linux x86-64.
`cargo fmt --all -- --check`, workspace/all-target clippy with warnings denied,
workspace tests and workspace build passed with the locked Rust 1.91.1 toolchain.
Tests: 7,181 passed, zero failed, 106 ignored across 59 suites. Python tooling:
505 run, five skipped. Source/binary asset guards and documentation checks
passed. No expected golden constants were changed.

Selected battery coverage includes bombs/rocket pods, laser and command weapons,
SAMs, ship missiles, cruise terrain following, seeker/support loss, burnout,
input tapes, replay HUD state and a four-versus-four fight. AT2's first run exposed
a command-missile control-speed cutoff; the command class now uses the documented
70 m/s rule and a helicopter reference. The anti-ship fixture now waits 180 s
for subsonic missiles instead of stopping at 90 s. Both reruns passed. The final
seven-scenario pass included serial/parallel Energy-study equality, AT2,
anti-ship, replay HUD, countermeasure display, weapon-chase replay and AI fight.

The hidden-workspace GPU smoke presented successfully. Countermeasure counts
and weapon-chase capture checks passed; the countermeasure fixture's image
contains its crashed live-fire setup, so it is not a clean visual demonstration
of flare separation. Physical/visible release agreement is covered by the
synthetic deterministic test. A 32-shot actual-AI study smoke completed.
Windows/macOS runtime and Apple Silicon fingerprints were not measured here.
The full Phase 3 statistical study follows separately.

## Prediction cost

Release build on this Linux x86-64 host, nice 10, eight AI aircraft and exactly
sixteen replenished zero-damage missiles in a real UKR world. Eight additional
launch/envelope clients exercise the same service. `missile_energy_cost`
measures request/read work plus the entire prediction-service advance.
The first 240 ticks are cold; subsequent ticks include refresh and cache work.
The stable-query lanes freeze only the eight probe geometries, while the world
and its missiles continue moving. Each row is one sequential run.

| Lane | Ticks | Cold p50 / p95 ms | Subsequent p50 / p95 ms | Maximum ms | Cached reads p95 ms |
| --- | ---: | --- | --- | ---: | ---: |
| AIM120 stable queries | 12000 | 0.684 / 0.807 | 0.003 / 0.652 | 0.841 | 0.00148 |
| AIM120 moving queries | 2400 | 0.673 / 0.723 | 0.647 / 0.748 | 0.864 | 0.00160 |
| AGM84A cruise-query stress | 4800 | 0.597 / 0.639 | 0.627 / 0.657 | 0.916 | 0.00212 |

Linux thread CPU timing independently confirms cold/subsequent p95 of
0.806/0.652, 0.725/0.747 and 0.637/0.656 ms, respectively. This optional
developer probe uses a Linux-only dev dependency already in the lockfile;
neither the runtime library nor deterministic work scheduling uses host CPU
clocks. The fixed budget is 16,384 quarter-step units, at most 4096 integration
steps, with terrain samples also charged. There is no synchronous fallback.

Latency is separate from CPU cost. Eight stable AIM120 envelopes completed;
moving envelopes stayed pending as geometry changed. Eight cold cruise range
envelopes were still pending after 40 simulated seconds under this load,
although 37,652 cached point reads succeeded. IN RNG and missile TTI consume
point forecasts without waiting for the complete range scale. A pending cue
is shown as unknown and cannot authorize an AI shot. This bounded-latency
limitation remains, especially for very long cruise trajectories.
