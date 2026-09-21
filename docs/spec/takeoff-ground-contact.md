# Takeoff, flaps and wheel release

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, authorized by John on 2026-09-21 after the cause review.
The rules below correct the hybrid adapter. [Pre-change measurements and source
identity](../baselines/takeoff-research.md) remain the historical research evidence.
New constants and continuous host adaptations are agent-selected fits; original
control flow is not an implementation requirement.

## Established player-visible behavior

The 1999 FA manual, printed page 64 (local PDF page 68), starts takeoff with
flaps extended and states that deploying them increases both lift and drag.
The tutorial uses full throttle or available afterburner, a pull as the nose
begins to rise, then gear retraction after liftoff. Its instruction to retract
flaps after reaching 200 knots is tutorial guidance, not evidence of a universal
A-4 rotation speed or a separate takeoff-flap detent. Manual identity is recorded
in the [gunsight spec](gunsight-targeting.md#manual-supported-behavior).

Existing reviewed FA evidence establishes a flap-dependent minimum-speed change
for low-G envelopes and a flap-lift term. Their actual contracts and unresolved
branches have one home in [flight-format research](../formats/native-flight.md).
The hybrid adapter implements those player-visible benefits without reproducing
original control flow. The legacy and restricted research adapters remain distinct.

Unknown: measured retail A-4 takeoff distance/time and configuration, ground
rotation response, wheel-unloading thresholds, and the source meaning of any
intermediate flap setting. No real-aircraft flight manual values or variant
substitutions are used to fill those gaps.

## Hybrid flap and lift rules

- Full deployment lowers the clean 1G stall reference by 25%; intermediate
  actuator fractions interpolate linearly. This uses the reviewed low-G flap
  effect, not an aircraft-specific invented rotation speed.
- The imported `flapsLift` is fixed8, not a percentage. At gear-up use the raw
  coefficient; gear-down scales it by the reviewed speed-dependent drag percent,
  with half that gear-down bonus during ground support. Apply deployment fraction
  and divide by 256 for the lift multiplier. This is a continuous host adaptation
  of the reviewed device effect.
- Flap and aerodynamic airbrake drag scale with the reviewed airflow-dependent
  drag percentage. Aerodynamic gear drag is omitted while wheels are supported;
  tire rolling/braking resistance remains a separate force.
- Interpolate the positive-G ceiling continuously between the flap-adjusted 1G
  minimum and the next positive-G envelope's lower-speed boundary. Apply existing
  loading reduction afterward. Use the same low-speed interpolation in the air,
  so wheel release cannot drop lift back into the former lower discrete band.
  At and above the next boundary, retain the existing envelope bands.
- Keep the existing up/down flap control. No takeoff-flap detent is introduced.

## Low-speed rotation

John requested a further rotation correction on 2026-09-21 after observing
excessive nose-up attitude while the aircraft remained on its wheels. The old
airborne trim fit could request 20 degrees at low speed regardless of ground
support. The following alignment correction is agent-selected and fitted.

Below twice the clean 1G minimum speed, use a low-speed nose/flight-path target
of `clamp(trim_degrees + 8 * filtered_elevator, 0, 10)` degrees. Blend this target
linearly into the existing airborne trim target between the flap-adjusted 1G
minimum and twice the clean 1G minimum. Below the adjusted minimum use the
low-speed target alone; at or above the upper boundary retain the existing
airborne response exactly. Existing actuator, pitch-rate and alignment filtering
remain active. This law is shared on both sides of wheel release, so contact
state cannot abruptly change the target angle.

This changes the hybrid nose alignment, not lift capacity or the force-based
wheel-release criterion. Legacy and restricted research adapters are unchanged.
Pitch is attitude relative to the horizon; angle of attack is relative to
airflow. Sustained back pressure can still produce a climbing pitch angle above
the low-speed target after liftoff. Neither the target nor measured rollout
speeds are claimed as real-aircraft or retail performance data.

## Wheel unloading and contact

Compute remaining wheel load from the world-vertical aerodynamic and thrust
support, including the vertical drag component, divided by aircraft weight.
Clamp remaining load to 0..1. It is not the body-normal HUD G reading.

While already supported, release when remaining load is at most 2% and upward
velocity exceeds 0.1 feet/second. The former 0.05-foot-per-tick height gate no longer blocks unloaded wheels.
A gap above 0.05 feet still detects geometric support loss, such as rolling
off an elevated surface, without snapping the aircraft down. A rising support
plane is resolved before unloading can release an aircraft below that plane. Once airborne, reacquire only at or below the support plane, preserving
positive height gains and avoiding immediate reattachment. These thresholds
are fitted contact hysteresis, not measured retail constants.

Scale lateral tire scrub, rolling resistance and wheel braking by remaining
wheel load. Apply supported horizontal movement using the post-tire velocity,
so tire braking does not leave a small position advance every tick. Preserve
existing touchdown severity checks against the pre-contact incoming velocity,
gear state and landable/water classification. Stable idle parking remains valid
with brakes either applied or released. At zero ground speed, idle throttle and
neutral controls, applied attitude rotation is also held while supported so
wind cannot swivel a parked aircraft. Rolling steering and aerodynamic airflow
remain active outside that parked condition.

## Aerodynamic wind and runway policy

Use the full atmospheric wind to obtain air-relative velocity for lift, speed,
rotation and aerodynamic forces. Restore it once for world motion; do not inject
or subtract a fixed wind-velocity correction each tick.

The [MTOW runway thresholds](runway-wind.md) remain difficulty/warning inputs.
Their maximum crosswind/tailwind fraction, multiplied by the smooth 0..5-knot
rolling transition, reduces lateral tire scrub by `1 - 0.5*fraction`: 75% scrub
at rough crosswind and 50% at the class limit once rolling above 5 knots.
Longitudinal braking and rolling resistance are not weakened by that wind factor.
Headwind has no runway penalty but now contributes to actual takeoff airflow.
This replaces the earlier use of filtered wind as the aerodynamic wind itself.

A separate aerodynamic ground-effect model is not present in the hybrid force
calculation. It is a later tuning question; the confirmed flap and contact
problems should first be isolated and corrected.

## Acceptance checks

- Compare all selectable aircraft at identical fuel/store configurations, with
  flaps deployed/retracted and explicit brake states; include non-afterburning A-4.
- Verify that flap deployment changes low-speed lift/stall behavior, not just drag.
- Check continuous rotation and wheel unloading without altitude resets, excessive
  nose angle, ground jitter or spurious touchdown damage.
- Compare calm, headwind, tailwind and crosswind at matched air-relative speeds.
- Exercise brake release separately from flap operation; the current airbrake and
  wheel-brake control is shared and its intended behavior must be explicit.
- Record aircraft-specific performance targets before fitting them. Do not declare
  a universal rotation speed or retail parity from a generic flat-runway probe.
