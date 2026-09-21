# Flap drag and liftoff investigation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-21. No production simulation code, controls or aircraft files were edited.
This measures the current hybrid host, not original FA execution. Proposed
corrections and manual-supported behavior are in the
[research specification](../spec/takeoff-ground-contact.md).

## Identity and method

Git base: `9e513509c321c7d2df3eba18577e9882ef7dad23`, with the existing uncommitted
flight, runway-wind, damage and HUD changes preserved. Tested simulation library
SHA-256: `b2581826f5131c37f794f6d4a0e1699d2e9d510e9fa9187448552199dfe9811a`.
Local A4E.PT SHA-256:
`fcaf24149ca78dec9143e8ebd9873f69be17cfa28198f557b82343b5a46649e9`.
Other source/code hashes, standalone probe sources, per-case CSV trajectories
and summary data are in ignored `.local/takeoff-research/`.

The standalone probe links the existing `tore-sim` and `tore-formats` libraries.
It loads reviewed PT/default-store records, creates a hybrid runway start on an
unbounded flat surface at 1,024 ft MSL, and runs fixed 120 Hz steps. Fuel and
stores use the default loadout. Engines use full throttle and afterburner where
available. Pitch input is 0.35 unless the case explicitly says otherwise. Brakes
are fully released and the airbrake already stowed for the main comparisons.
Each run stops at first wheel release or 120 seconds. This isolates aircraft
physics from runway length and terrain, and does not establish sustained climb.

Intermediate flap fractions are held by the local diagnostic across the existing
binary actuator. Zero-drag/friction variants alter only in-memory probe
configuration. None is an implemented setting or proposed calibrated constant.

## Measured takeoff comparisons

A-4E starting weight is 22,026 lb: 10,800 empty, 4,434 internal fuel and 6,792
payload. It has no afterburner. Effective full thrust at the test elevation is
11,036 lbf. These are imported game values, not real-world performance claims.

| A-4E configuration | First wheel release | Ground roll | Airspeed at release |
| --- | ---: | ---: | ---: |
| Full flaps | 99.26 s | 12,694 ft | 131.39 kt |
| Half flaps, diagnostic only | 31.86 s | 3,771 ft | 134.07 kt |
| Quarter flaps, diagnostic only | 24.08 s | 2,847 ft | 135.45 kt |
| Flaps retracted | 19.44 s | 2,308 ft | 136.87 kt |
| Full flaps, flap drag removed in probe | 19.44 s | 2,308 ft | 136.87 kt |
| Full flaps, rolling resistance removed in probe | 72.35 s | 8,928 ft | 131.83 kt |
| Full flaps, gear drag removed in probe | 43.13 s | 5,146 ft | 133.00 kt |
| Full flaps, full aft input instead of 0.35 | 98.92 s | 12,618 ft | 131.08 kt |
| Full flaps, 20-knot headwind | 99.26 s | 12,694 ft | 131.39 kt |

Afterburning F/A-18D wheel release is 26.68 s with full flaps versus 14.13 s
retracted. Rafale C is 16.01 s versus 10.27 s. The A-4 is especially exposed to
the drag penalty; earlier startup coverage checked every aircraft parked but
reported takeoff performance only for F/A-18D and Rafale C.

The unmodified app also ran an A-4 airport-2 takeoff in calm wind:
`TORE_WIND=0,0 target/debug/tore-app --aircraft a4e --ground-start 2 --headless-flight 14400 --maneuver takeoff --no-audio`.
It eventually cleared airport ground by 100 feet after 12,409 ticks (103.41 s),
at 126.93 kt without crashing. This is not a literal permanent takeoff lock;
acceleration and rotation are excessively delayed for practical play. Runway
length was not independently measured, so this report does not claim an overrun.

## Confirmed causes

### Device drag without the flap benefit

The hybrid drag term in `flight.rs` adds `weight * coefficient * deployment / 256`
for flaps, gear and airbrake, without the speed-dependent multiplier documented
in the recovered force contract. The total drag is capped only to prevent it
reversing velocity in a single step. At the A-4's initial weight, full flaps add
6,539 lbf, about 59% of available thrust; gear adds 1,979 lbf. Rolling resistance
is another approximately 548 lbf equivalent before clean aerodynamic drag.

Full flaps do not change the hybrid lift command, clean stall lookup or envelope
selection. A-4 PT contains `flapsLift=51`, and reviewed evidence includes a
flap-dependent minimum-speed effect, but these benefits are not consumed by
this adapter. Removing only flap drag produces exactly the retracted-flap
trajectory, confirming that no other physical flap effect offsets the penalty.
The manual's printed page 64 explicitly describes both lift and drag benefits.

### Rotation and wheel release

At this altitude A-4's clean 1G envelope minimum is 109.71 kt, and the 2G band
starts near 131.12 kt. Load scaling caps the allowable lift command near 0.708 G
below that next band in the tested configuration. With 0.35 aft input, entering
it raises the command to approximately 1.146 G. More aft input scarcely changes
the full-flap takeoff time. Nose trim/alignment is not an independent wing-lift
calculation, so the aircraft can have substantial nose-up attitude while still
receiving insufficient modeled lift.

`research::contact` releases wheels only above support height plus 0.05 ft.
Otherwise it resets height to the support plane every tick, while retaining
positive vertical velocity. At 120 Hz, that demands more than about 6 ft/s of
upward motion to cross in one tick. A contact-only test with fixed upward speeds
0.5, 1, 3, 5.9 and 6 ft/s gains zero height over one second and stays supported;
6.1 ft/s gains 6.1 feet and releases. In the full-flap takeoff run, upward velocity
begins around 98.51 s but height remains clamped until 99.26 s. The delay is about
0.75 s in this case; the much longer rollout is primarily the drag/lift problem.

While contact remains set, tire scrub and rolling/brake deceleration are not
scaled by remaining wheel load. They stay at full strength until release.
There is no separate aerodynamic ground-effect term in the current hybrid law.

### Brake state and wind are separate contributors

`brake_out` controls both aerodynamic airbrake deployment and wheel braking.
Leaving it on adds the full airbrake term plus 18 ft/s² braking rather than
0.8 ft/s² rolling resistance. Both clean and flapped A-4 brake-on probes finish at zero speed. They still
creep about 16 feet over 120 seconds because position advances before tire
braking, a further contact-integration artifact. The flap failure above occurs with both brakes fully released, so it
cannot be dismissed as an unreleased-brake case.

The current runway-wind policy removes headwind from the wind vector passed to
ground-roll aerodynamic calculations. The 20-knot headwind probe exactly matches
calm. Keeping that difficulty/drift policy separate from actual airflow is needed
for a future wind-sensitive liftoff calculation. Airborne wind remains separate.

## Limits and validation

Thirty-nine controlled takeoff variants and six contact-only velocity cases were
run, plus the unmodified airport A-4 probe. The manual takeoff page was rendered
and inspected. No retail execution comparison, aerodynamic calibration, stable
intermediate flap control, sustained climb matrix or real-aircraft certification
was performed. No gameplay fix, commit or push was made.

All required non-display checks passed: formatting, warnings-denied workspace
Clippy, locked workspace tests and build, Python tests, source/binary asset
guards and documentation checks. The simulation source and linked library hashes
remain unchanged after the investigation. No new flight-rendering check was
needed because rendering was not modified.
