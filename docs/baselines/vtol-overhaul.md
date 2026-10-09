# VTOL and helicopter overhaul

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Measured evidence for the [powered-lift flight spec](../spec/powered-lift-flight.md)
(slices P1 to P10 of the VTOL and helicopter overhaul, October 2026). One record
for the whole project, taken on the final tree of the `vtol-overhaul` branch on
Linux (Ryzen 9 7900X, release build of the examples). Retail comparison is
unavailable: retail players never flew rotorcraft, and the jets' hover and
transition were never measured, so this is a record of the model against its own
targets and against published figures, never a parity claim.

## How the numbers were taken

Three examples read the user-owned PT records at runtime (nothing retail is
committed) and print the tables below. All run at sea level on a standard day
with no wind, at the PT gross weight (empty plus full internal fuel) unless a
line says otherwise.

```sh
PT=path/to/FA_2.LIB
cargo run --release --locked -p tore-sim --example powered_probe -- $PT/AH64.PT $PT/MI24.PT $PT/CH47.PT $PT/V22.PT
cargo run --release --locked -p tore-sim --example jet_probe -- $PT/AV8.PT $PT/YAK141.PT --compare $PT/F16C.PT
cargo run --release --locked -p tore-sim --example variety_flight -- $PT/AV8.PT $PT/YAK141.PT $PT/V22.PT $PT/AH64.PT $PT/MI24.PT $PT/CH47.PT
```

The acceptance cases are unit tests on synthetic aircraft carrying the same PT
numbers (`crates/tore-sim/src/flight/powered/*_tests.rs`, `jet.rs`,
`trim.rs`): H1 to H14 for the helicopters, J1 to J13 for the jets, T1 to T6
for the V-22, A1 to A4 for hover hold, E1 to E3 for the Easy cheat and S1 and S2
for the starts. They pass on this tree.

## Helicopters and the V-22

Probe output from `powered_probe`. Rated power is what reaches the rotors.
"Margin" is rated power over the power the hover needs.

| | AH-64 | Mi-24 | CH-47 | V-22 |
| --- | --- | --- | --- | --- |
| Gross weight, lb (maximum) | 20,298 (23,810) | 21,385 (28,660) | 21,385 (28,660) | 20,298 (23,810) |
| Rated power, hp | 3,668 | 3,315 | 3,001 | 5,122 |
| Hover out of ground effect: power, margin | 2,551 hp, 44 percent | 2,359 hp, 41 percent | 1,873 hp, 60 percent | 2,664 hp, 92 percent |
| Hover collective (lever) | 75 percent | 63 percent | 45 percent | 73 percent (nacelles 87) |
| Hover ceiling out of ground effect, gross / maximum weight | 9,900 / 3,900 ft | 9,700 ft / cannot hover | 15,700 / 4,600 ft | 13,900 / 8,900 ft |
| Level top speed | 151 kt | 177 kt | 165 kt | 273 kt (airplane mode) |
| Least power | 1,452 hp at 69 kt | 1,108 hp at 77 kt | 1,023 hp at 59 kt | |
| Best climb (excess power) | 3,602 ft/min | 3,406 ft/min | 3,052 ft/min | |
| Vertical climb at full collective | 2,381 ft/min, rotor 99 percent | 2,252 ft/min, rotor 89 percent | 2,925 ft/min, rotor 84 percent | |
| Engine cut in the hover, collective held: rotor below 80 percent after | 1.74 s | 1.93 s | 2.38 s | 1.57 s |
| Autorotation | 2,251 ft/min at 76 kt, rotor 98 to 105 percent | 1,578 ft/min at 76 kt, rotor 98 to 103 | 1,304 ft/min at 81 kt, rotor 96 to 102 | |
| Slowest wingborne level flight | | | | 117 kt |
| Stall warning on the downstops | | | | 110 KCAS |

Published figures for comparison (design verification table, 2026-10-08): level
speed AH-64 158 kt, Mi-24 170 to 181 kt, CH-47 170 kt, V-22 275 kt; hover ceiling
out of ground effect Mi-24 4,915 ft (weight not stated), AH-64 9,810 ft (D) to
11,500 ft (A); climb Mi-24 2,460 ft/min, AH-64 3,240 ft/min maximum. The Mi-24 at
its published 24,250 lb normal takeoff weight hovers out of ground effect to
4,900 ft with a 19 percent margin, climbs 2,791 ft/min and reaches 174 kt; at 26,455 lb it
hovers to 1,500 ft with a 6 percent margin, climbs 2,400 ft/min and reaches 168 kt.
The AH-64 at 17,650 lb hovers to 15,000 ft with a 73 percent margin, climbs
4,549 ft/min and reaches 152 kt. The PT empty weights are 61 percent over the real AH-64's and 43
and 26 percent under the real V-22's and CH-47's, so published power-to-weight
cannot be applied directly; power is set from published engines
where the PT thrust is implausible (see the spec).

### Hover rates and the stability levels

Full stick for 2 s from a hover, degrees per second (design targets in
brackets). Damper adds rate damping and never adds rate over Off.

| | Level | Pitch | Roll | Yaw |
| --- | --- | --- | --- | --- |
| AH-64 (45 / 90 / 90) | Off | 51 | 101 | 106 |
| | Damper | 41 | 80 | 88 |
| Mi-24 (40 / 90 / 80) | Off | 43 | 108 | 101 |
| | Damper | 35 | 86 | 84 |
| CH-47 (25 / 45 / 45) | Off | 28 | 50 | 48 |
| | Damper | 23 | 41 | 42 |
| V-22 (30 / 45 / 30) | Off | 34 | 56 | 40 |
| | Damper | 28 | 48 | 33 |
| AV-8 (20 / 50 / 20) | Damper | 16 | 50 | 21 |
| Yak-141 (20 / 50 / 20) | Damper | 8 | 49 | 19 |

The Yak-141's pitch takes longer than 2 s to build (3 s: 16 deg/s, slice P4).

Torque (H12): a 30 percent collective step with the pedals fixed, yaw rate after
2 s in degrees per second (positive nose right).

| | Off | Damper |
| --- | --- | --- |
| AH-64 | +25.7 | -2.3 |
| Mi-24 | -23.5 | +1.4 |
| CH-47 | +0.4 | -0.1 |

### Conversion and the corridor (V-22)

- Conversion from a 1,000 ft hover with the keys held and the lever at 85
  percent, Damper: on the downstops in 15.9 s, 200 KCAS in 27.0 s, height from
  -14 to +124 ft, never outside the corridor, rotor speed at least 92 percent of
  its reference throughout.
- Protection with the helicopter preset asked in airplane mode: from 180 KCAS the
  nacelles stop at 41.4 degrees (185 KCAS); from 210 KCAS at 0 degrees. At 140
  KCAS and 80 degrees (edge 130) the nacelles move forward after 0.1 s and are
  inside the corridor after 0.9 s. Nacelles asked forward at 40 KCAS hold at 58.5
  degrees at 66 KCAS (the lower edge at that speed is 53.8 degrees).
- Airplane mode at sea level, gross: level top speed 273 kt; slowest wingborne level
  flight 117 kt; stall warning 110 KCAS (published stall 110 kt, top speed 275 kt).

## Vectoring jets

Probe output from `jet_probe`. The `oracle` is the same PT flown by the
conventional model with its powered lift taken away.

| | AV-8 | Yak-141 |
| --- | --- | --- |
| Weight, lb | 21,727 | 35,385 |
| Hover vertical acceleration at gross (margin) | +3.34 ft/s² (+10.4 percent) | +0.39 ft/s² (+1.2 percent) |
| Same with 1,000 lb of stores | +2.41 (+7.5 percent) | -0.50 (-1.6 percent) |
| Same with 4,000 lb of stores | -0.65 (-2.0 percent): cannot hover | -2.85 (-8.9 percent) |
| Thrust at gross: main / lift engines, lbf | 33,291 / none | 19,613 / 17,790 |
| Transition from a 500 ft hover (nozzle script of the manual) | 90 kt at 8.2 s, 150 kt at 10.5 s, lowest 450 ft | 90 kt at 27.7 s, 150 kt at 33.8 s, lowest 224 ft |
| Level top speed 1,000 ft / 10,000 ft | 519 / 515 kt (oracle 519 / 515) | 659 / 731 kt (oracle 659 / 731) |
| Slowest level speed at 10,000 ft | 147 kt (oracle 143) | 112 kt (oracle 112) |
| Peak G at 300 kt, 10,000 ft | 5.28 (oracle 4.94) | 4.66 (oracle 4.56) |
| Peak G at 450 kt, 10,000 ft | 4.47 (oracle 4.14) | 5.66 (oracle 5.49) |
| Mean pitch rate at 300 / 450 kt, deg/s | 18.0 / 9.5 (oracle 17.5 / 8.8) | 17.1 / 14.0 (oracle 17.5 / 13.0) |
| Roll rate at 300 / 450 kt, deg/s | 224 / 224 (oracle 224 / 225) | 224 / 224 (oracle 224 / 225) |
| Sustained 60 / 70 degree turn at 10,000 ft | 492 kt, 3.9 deg/s / 468 kt, 6.4 deg/s (oracle 491, 3.9 / 467, 6.5) | 652 kt, 2.7 / 537 kt, 5.6 (oracle 650, 2.7 / 534, 5.6) |

The wingborne numbers stay within 10 percent of the conventional model, which is
the acceptance (J4). A dive pushed to a 60-degree nose-down attitude then released
keeps the flight path within 2 degrees of the nose and descends at 650 to 700 ft/s
(AV-8 at 374 kt) as a jet should, where the earlier fitted law descended at 100
ft/s. The AV-8's manual transition (J3) reaches 150 kt in 10.5 s with a 50 ft
height loss. The Yak-141 takes 34 s and loses 276 ft with the same script: its
afterburner is blocked above 20 percent nozzle travel, so the dry engine and
lift engines have little margin to accelerate on; this is recorded as a known
difference, not tuned away (J3 is written for the AV-8 and the Yak-141 is
covered by J13).

## Hover hold

From the [autopilot baseline](autopilot.md#hover-hold-and-the-powered-lift-aircraft-2026-10-09)
(tests A1 to A4 in `hover_hold_tests.rs`, passing on this tree): engaged at 30
kt with a 15 kt crosswind at Damper, drift below 1 kt after 7.3 s (AH-64), 7.3 s
(Mi-24), 6.8 s (CH-47) and 8.7 s (V-22); then over 60 s the largest distance
from the held point is 5.9, 6.2, 2.7 and 6.2 ft and the largest height change
1.6, 1.2, 1.7 and 1.9 ft (gates 20 s, 20 ft, 10 ft). Across every stability
level, the Easy cheat, five winds and gusts the worst case is 11.0 s, 6.8 ft, 3.0
ft and 2.3 degrees of heading. A4: 1,800 gusty ticks replayed by hand from the
inputs the flight model received are bit-identical.

## Easy flight physics

From slices P8 and P8b (tests in `easy_physics_tests.rs`). With the cheat on:

| Test | Without the cheat | With the cheat |
| --- | --- | --- |
| H10 sink arrested at one hover induced velocity, full collective | AH-64 and Mi-24 still sinking at 3 s; CH-47 3.2 s; V-22 4.9 s | AH-64 5.9 s, Mi-24 1.9 s, CH-47 0.6 s, V-22 2.4 s |
| H12 collective step, yaw rate after 2 s at Off | AH-64 26 deg/s, Mi-24 43 | AH-64 0.5, Mi-24 1.2, CH-47 0.003 |
| H11 CH-47 dived to 215 kt, pitch in 2 s | +23 degrees | +7 degrees (vibration cue stays) |
| H9b rotor speed after an engine cut with the lever held | below 70 percent, no recovery | exactly 85 percent (84 for the V-22 on the downstops) |
| H14 rollover at 20 degrees of bank under thrust | crash on tick 1 | no crash |
| J11 AV-8 at Off, 10 degrees of sideslip at 40 kt | 33.6 degrees of bank in 3 s | 0.09 degrees |
| Hands-off forward trim of 10 percent, height held | | AH-64 about 90 kt; CH-47 157 kt at Damper, 129 kt at Off, steady |

H3 (top speed) and the climb rate (H6) stay within 2 percent with the cheat on
(CH-47 and V-22 top speeds identical), and a loaded jet still cannot hover.

## Starts

S1: every airborne start of the six types hands off at Damper and Off, at
3,000 and 6,000 ft, empty and loaded, holds height within 10 ft and speed within
2 kt for 10 s (the AH-64 about 100 kt, the V-22 about 177 kt wingborne with the
nacelles on the downstops, the jets wingborne). S2: a ground start is stationary
with the engine idling, the rotor at its governed speed within 1.5 percent for
5 s, the collective down and the brakes on (V-22 nacelles at 87 degrees).

## Moving parts and replay

Rotors turn at the simulated rotor speed; disk tilt, nacelle angle and nozzle
angle are the simulated ones. A replay records the rotor speed (chunk section 8)
and the disk tilt (section 9, to 1/256 rad, within half a step); tests cover a
round trip across chunks with aircraft entering and leaving, the writer's
refusals, and playback forward, backward and between ticks. Files from before
either section read as a stopped rotor and level disks.

## Changes at the end of the project (slice P10)

- The collective lever moves at its intended 2 per second. The old law's actuator
  step (0.7 per second) used to move it as well, so the lever ran at 2.7 per
  second; the H, T and tandem suites needed no refit. A pinned test checks a
  quarter second moves half the travel.
- `PoweredLift::efficiency`, the last field of the old fitted law, is gone: an
  aircraft built without a start takes its collective and lift from the
  rotorcraft's own hover trim.

## Sources

Public figures checked on 2026-10-08 (Wikipedia figures are the Specifications
section of each article).

| Key | Source |
| --- | --- |
| W-AH64 | [Boeing AH-64 Apache, Wikipedia](https://en.wikipedia.org/wiki/Boeing_AH-64_Apache) |
| TM4201 | NASA TM 4201 (AVSCOM TM 90-B-015), Aerodynamic performance of a 0.27-scale model of an AH-64 helicopter, 1990 (solidity 0.0928, hover tip speed 727 ft/s) |
| W-Mi24, AW-Mi24 | [Mil Mi-24, Wikipedia](https://en.wikipedia.org/wiki/Mil_Mi-24); [Aerospaceweb Mi-24 Hind](https://aerospaceweb.org/aircraft/helicopter-m/mi24) (17.30 m, 240 rpm, 2 x 2,225 shp, 335 km/h, ceilings) |
| AW-rot | [Aerospaceweb, helicopter rotation conventions](https://aerospaceweb.org/question/helicopters/q0212b.shtml) |
| W-CH47 | [Boeing CH-47 Chinook, Wikipedia](https://en.wikipedia.org/wiki/Boeing_CH-47_Chinook) (CH-47F) |
| W-V22 | [Bell Boeing V-22 Osprey, Wikipedia](https://en.wikipedia.org/wiki/Bell_Boeing_V-22_Osprey) (MV-22B; 97.5 degrees; 12 s conversion) |
| VM-V22 | [Flying the V-22, Vertical Magazine](https://www.verticalmag.com/features/20112-flying-the-v-22-html/) (8 deg/s nacelles, corridor reference points, 200 KCAS aft lock, 84 percent cruise rotor speed, 280 KCAS) |
| AOPA-V22 | [Flying schizophrenic, AOPA Pilot, November 2009](https://www.aopa.org/news-and-media/all-news/2009/november/pilot/flying-schizophrenic) (333 and 397 rpm) |
| Acree | C. W. Acree, JVX proprotor performance, AHS 2008, [NASA PDF](https://rotorcraft.arc.nasa.gov/Publications/files/Acree_AHS-SF2008.pdf) |
| Patent-6644588 | [US 6,644,588, multi-mode tiltrotor nacelle control with envelope protection](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/6644588) |
| W-AV8B | [McDonnell Douglas AV-8B Harrier II, Wikipedia](https://en.wikipedia.org/wiki/McDonnell_Douglas_AV-8B_Harrier_II) |
| AE-noz | [Powerplant: nozzle actuation system, Aircraft Engineering and Aerospace Technology, 1970](https://emeraldinsight.com/insight/content/doi/10.1108/eb034599/full/html) (98.5 degrees) |
| W-Yak, GS-Yak | [Yakovlev Yak-141, Wikipedia](https://en.wikipedia.org/wiki/Yakovlev_Yak-141); [GlobalSecurity Yak-141](https://www.globalsecurity.org/military/world/russia/yak-141.htm) (95 degrees) |
| TM88360 | M. G. Ballin, UH-60A real-time simulation, NASA TM 88360, 1987 (inertias) |
| NESC-F16 | [NASA NESC Academy flight simulation check cases](https://nescacademy.nasa.gov/flightsim/2015/bodies) (F-16 inertias) |
| CB | Cheeseman and Bennett, The effect of the ground on a helicopter rotor in forward flight, ARC R&M 3021, 1955 |
| CG | Castles and Gray, NACA TN-2474, 1951 (the shape of the vortex ring inflow curve; not read) |

What could not be verified is fitted and labelled so: the CH-47's rotor speed,
solidity and thrust, the Mi-24's solidity, the tail rotor arms, the CH-47, Mi-24
and V-22 inertias, the Harrier's nozzle rate, the V-22's wing download, the
vortex ring curve coefficients and the ground effect coefficient. The Yak-141's
afterburner in the hover has no source, so it stays blocked above 20 percent
nozzle travel.

## Open items for John

- The AH-64's power is the PT's own, which hovers it to 9,900 ft at gross and
  leaves it stronger than the real aircraft at the real aircraft's weights; its PT
  empty weight is 61 percent over the real 11,385 lb. Decide whether it should be
  weaker.
- The V-22's power share (0.92 x 23,810 / 52,600 of two AE 1107C) is generous: a
  92 percent hover margin and a 13,900 ft hover ceiling. A lower share would
  also shrink its forward flat plate.
- The Mi-24 cannot hover at the PT's own 28,660 lb maximum weight (the real
  maximum is 26,455 lb).
- The Easy flight physics numbers await John's tuning.
- If European-layout players report accidental hover hold (AltGr reports as
  Ctrl+Alt), move it to Ctrl+Shift+H.

## Battery and repository checks

Recorded in the final section below after the headless pass.
