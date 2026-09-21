# Takeoff and ground-contact acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. Sol agents implemented the shared hybrid flap,
lift and contact changes; the root agent reviewed the code and ran independent
source-aircraft and application checks. [Rules and fitted constants](../spec/takeoff-ground-contact.md)
and [pre-change investigation](takeoff-research.md) are separate. No retail bytes
are committed. Evidence stays in `.local/takeoff-implementation/` for the flap/contact pass and
`.local/rotation-cadence-review/` for the subsequent rotation and cannon review.

## Review corrections

The review caught and corrected ungated aerodynamic changes that would have
altered legacy flight, a proposed per-tick wind-velocity correction, a proposed
five-knot idle freeze, and missing support-loss handling when a runway drops
away. A rising support plane is also resolved before unloading can release an
aircraft below the surface. Later source-aircraft testing caught wind-induced
yaw rotation while parked; idle/neutral static support now holds attitude too.

The retained implementation uses genuine world-vertical wheel load. A proposal
to fake wheel support when the stick is neutral was rejected; it would have
hidden landing float by restoring ground sticking. The wind-limit tire factor
retains at least 50% lateral grip independently of the remaining wheel load.

## Controlled takeoff matrix

All thirteen selectable identities ran full-flap calm, clean calm, 20-knot
headwind, 10-knot tailwind and 20-knot crosswind cases: 65 cases total. They use
the imported default fuel/stores on a flat 1,024-foot runway, full power and
available afterburner, with 0.35 aft input. Gear retracts after ten feet of
clearance. Each case is observed through at least five seconds after first
release and a 100-foot climb. All cases climb without crashing or reattaching.
The fixed held pitch input is a test stimulus, not an instruction to keep pulling
through the initial climb.

| Aircraft | Full-flap release | Ground roll | Release airspeed | Release pitch | Clean release |
| --- | ---: | ---: | ---: | ---: | ---: |
| F/A-18D | 8.31 s | 802 ft | 113.34 kt | 3.32° | 9.89 s |
| Rafale C | 6.27 s | 602 ft | 113.08 kt | 2.71° | 7.44 s |
| F-14D | 8.06 s | 640 ft | 93.76 kt | 2.91° | 9.85 s |
| A-4E | 11.76 s | 1043 ft | 103.79 kt | 3.86° | 13.52 s |
| X-31 | 5.96 s | 617 ft | 122.51 kt | 2.53° | 7.17 s |
| MiG-29 | 5.40 s | 516 ft | 112.41 kt | 2.48° | 6.51 s |
| Su-27 | 5.27 s | 440 ft | 98.40 kt | 2.29° | 6.50 s |
| MiG-21 | 7.81 s | 677 ft | 102.10 kt | 2.50° | 9.81 s |
| Su-25 | 8.32 s | 523 ft | 74.01 kt | 3.25° | 9.86 s |
| MiG-23 | 9.03 s | 774 ft | 101.10 kt | 3.38° | 10.84 s |
| Su-35 | 6.43 s | 540 ft | 98.86 kt | 2.60° | 7.88 s |
| F-22 | 3.23 s | 220 ft | 80.53 kt | 2.40° | 3.76 s |
| F/A-XX | 3.23 s | 220 ft | 80.53 kt | 2.40° | 3.76 s |

The previous flap/contact pass still allowed 14.29 degrees of A-4 pitch at
release. Low-speed trim correction reduces that to 3.86 degrees with the same
input and configuration, without an artificial wheel hold or release-speed
switch. The matrix above includes the rotation correction. Easing the A-4 stick at release
keeps pitch below 6.25 degrees and angle of attack below 5.05 degrees during
the next two seconds. Holding 0.35 aft input instead reaches 13.74 degrees of
climbing pitch and 11.21 degrees of angle of attack. These are measured
game-model results, not real-aircraft or retail performance claims. The short
F-22 rollout remains a source-profile tuning limitation for human assessment.

## Parking, landing and adapter checks

All thirteen aircraft hold exactly zero horizontal displacement and unchanged
heading/pitch for ten seconds in a 40/-25 ft/s wind, with brakes both applied and
released. Moving rudder steering still works. Contact tests cover partial wheel
loads, gentle upward release, no reattachment above the surface, surface drops,
rising surfaces, brake-stop position, and safe/gear-up/water/hard touchdowns.

A separate 118-knot, full-internal-fuel, no-external-payload landing probe checks
light aircraft without changing their identities. Slight forward input settles
all thirteen without a bounce or crash. Neutral-input approaches often float
once before settling; F-22/F/A-XX remain about 25 feet above the surface after
15 seconds. This is a known modeled overspeed/trim response for human feedback,
not evidence of real-world behavior. No artificial ground hold was added to hide it.

Independent legacy tests with flaps, gear and wind, plus clean high-speed hybrid
probes, produce exactly the pre-change numeric output for A-4, F/A-18D and Rafale.
The original baseline and current `regression.csv` match exactly.
All thirteen actual application airport-2 takeoff probes also clear 100 feet
without crashing. A-4 does so after 2,096 ticks (17.47 seconds), versus the
pre-flap-correction application's 103.41 seconds. This application milestone differs
from the table's first-wheel-release criterion.

## Required checks and visual review

All required repository checks pass: 946 Rust tests, 68 Python tests, formatting,
warnings-denied workspace/all-target Clippy, locked workspace build, source and
both executable asset guards, documentation checks and diff whitespace. Two
optional GPU unit tests remain ignored. Explicit display smoke and GPU captures
passed on NVIDIA RTX 4070/Vulkan.

The root inspected updated A-4 parking, rolling, airborne and headwind takeoff captures,
plus an airborne F/A-18D comparison. Ground capture probes now run after the actual
runway/loadout reset and advance physics/weather before rendering. Only level or
takeoff ground captures are allowed; other ground pose overrides remain rejected.
Windows/macOS runtime behavior and retail takeoff comparison were not tested.

## Human test focus

Use Quick Mission, A-4E, default stores/fuel, Ground start and a calm runway first.
Leave flaps deployed, press B to release wheel/air brakes, use 0 for full throttle,
and apply gentle back pressure as speed builds. Ease the pull after liftoff;
retract gear once clear. The A-4 has no afterburner. Compare an afterburning
F/A-18D or Rafale, then repeat with headwind/crosswind and exercise landing flare,
wheel settling, flap/gear transitions and taxi steering. There is no new flap detent.

Cannon cadence and intermittent tracers are validated separately in the
[damage/combat baseline](damage-smoke.md).
