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
substitutions are used to fill those gaps, with one explicit exception: the
weight-scaled stall speed below, which John requested on 2026-09-29.

## Weight-scaled stall speed

`opinionated`, requested by John on 2026-09-29 after the F-22, Su-27 and Su-25
lifted off at 78, 97 and 73 knots; the reference weight and every number here
are agent decisions. This is a deliberate departure from the retail polygon data,
made because real fighters need markedly more speed to fly when loaded. Hybrid
adapter only; the legacy adapter is unchanged, and `--retail-stall-speeds`
turns the rule off for the whole run.

- The imported envelope polygon keeps its meaning as the aircraft's speed limits
  **at its reference weight**, which is its **empty weight** (no fuel, no
  stores). The reference is taken from the game's own data: the imported
  loaded-elevator percentage takes away G in proportion to fuel and stores
  divided by the empty weight, so the polygon is the aircraft with a loading of
  zero.
- The scale is `sqrt(W / W_ref)`, with `W` the aircraft's instantaneous weight
  (empty weight, fuel, and carried stores and ammunition), so fuel burn,
  jettisoned stores and expended ordnance all change it. It is finite for any
  weight (the weight is held to a quarter of the reference at the least).
  Removing the divisor instead would put the F-22's liftoff back near 80 knots:
  the divisor is what lifts the speed for 1 G above the flapped stall speed (97
  against 74 knots for the F-22 at gross weight).
- The scale multiplies the slow edges of the **low G rows**: the 0 G, 1 G and 2 G
  rows and the negative rows they mirror take all of it (these set the stall,
  liftoff and approach speeds), it fades linearly to nothing at the 4 G row, and
  the rows from 4 G up are the imported ones (`row_scale`). The slow side of a
  row's outline is the vertices from the first to the highest one; each moved
  vertex is held to the fast side at its height. The fast edge (top speed, the
  overspeed rule) does not move. Reason: the loaded-elevator divisor already
  takes weight off the G an aircraft can pull at any speed, so scaling the high
  rows as well counted the weight twice (a fuelled F/A-18D pulled 3.8 G at 450 kt
  instead of 7.6, corrected 2026-09-29): above the speed of the 4 G row (about
  twice the stall speed) the G limit is the imported one, and only the slow side
  loses G to the weight. The polygons the model
  holds are rewritten when the weight has changed by a quarter of a percent, so
  the flight model, the stall warning, the autopilot, the flight envelope
  instrument and the AI all read the same speeds.
- Full flaps still lower the 1 G reference by 25 percent, and the ramp to the next
  G row, the loaded-elevator divisor and the 1 G floor and ceiling rules are
  unchanged. The divisor stays: it limits the G the loaded aircraft can pull at
  any speed, the scaling moves where the wing runs out of lift. The two together
  make a loaded aircraft need a little more than the scaled stall speed to hold
  1 G, which is the speed the model takes off at.

Results at the default loadout, sea level, full flaps and afterburner, and against
John's figures for the three aircraft he named, are in the
[flight lane page](../testing/lane-flight.md#weight-scaled-stall-speed-2026-09-29).
One reference weight cannot bring every aircraft within 10 percent of the
published figures: the F-22 family, Su-25, MiG-21 and MiG-23 stay low because
their imported polygons are; see there.

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
