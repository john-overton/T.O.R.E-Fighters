# Spin transition validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Scope and result

Implementation pass, 2026-09-16, Linux debug build. Validated the fitted
[input-driven spin specification](../spec/spin-transitions.md). Source aircraft
evidence remains in the [port baseline](aircraft-fa-expansion.md). No original
spin dynamics or real-aircraft stability derivatives are claimed.

Synthetic tests establish:

- Rudder magnitude continuously changes spin acceleration, including 0.001-axis
  increments and values straddling the old recovery-input threshold.
- Both directions respond symmetrically. Longer wrong-rudder application builds
  more rotation and takes longer to arrest than early intervention.
- Opposite rudder arrests an incipient spin below stall speed, leaving Stalled
  rather than claiming normal wing lift has recovered.
- Forward stick changes measured nose-pitch rate in the first simulation tick,
  proportionally from 0.001 to full deflection, before spin clearance.
- Faster rotation reduces control effectiveness without eliminating it.
- Recovery honors the 25-degree cone and rudder-rate threshold, retaining and
  damping residual angular velocity rather than snapping it to zero.

Soft-entry regressions verify continuous torque on both sides of the old
46.9%/93.8% rudder switches, zero torque at the speed/pitch boundary, stronger
torque with greater deficit/back-stick, and the X-31 disable flag. With full
back-stick/rudder, zero initial rotation and maximum spin rate 180 degrees/s,
initial acceleration is 25.3422 degrees/s² at 95% clean stall speed and
76.9824 degrees/s² at 90%. Opposite-rudder braking remains unscaled.

The `spin_recovery` example accepts local F14.PT/A4E.PT. For each aircraft,
54 seeded cases cover 300 mph, 350 knots, 600 mph, both directions and throttle
0/0.4/1. Cases include full spin with a nose-high downward trajectory, partial
spin with aligned airflow and neutral controls, and release of recovery inputs
after 0.5 seconds. All 108 cases recover within the 20-second observation and
match independent fixed-step replay. They are synthetic departures, not a
recording of John's exact maneuver or original-game trajectories.

All 65 standard flight-suite scenarios passed across F18, RAFALE, F14, A4E and
F31. Required formatting, workspace clippy, 390 Rust tests, workspace build,
40 Python tests, repository and binary asset checks, documentation and diff
whitespace checks passed. No renderer changes; GPU smoke was not rerun.
Physical controller playtesting and retail comparison were not performed.
The fitted torque and elevator constants still need subjective handling review.
