# Gradual spin entry and recovery

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Behavior and provenance

John requested input-driven spin dynamics, proportional elevator response and
recovery by threshold rather than elapsed time on 2026-09-16. Apply this fitted
agent-authored model only to the hybrid adapter. PT maximum spin yaw rates,
spin-disable flags and warning timers remain source-derived. Hybrid entry is
fitted for a continuous onset, as requested by John after the checkpoint. The restricted
research adapter is unchanged. No real-aircraft stability derivatives or retail
spin trajectories are asserted.

## Soft entry

While airborne and below clean stall speed, positive back-stick and rudder into
the source-selected departure direction can begin incipient rotation. Preserve
the source warning/stalled eligibility and direction selection, including the
X-31 spin-disable flag. Replace the hybrid's old 46.9%/93.8% rudder switches with
continuous inputs. The restricted research adapter retains those source gates.

For torque that drives the spin, multiply by a fitted departure factor:
smoothstep((1-speed/clean-stall)/0.25) * smoothstep(back-stick/0.5).
Each smoothstep clamps its argument to 0..1 and returns 3t²-2t³. Thus torque
starts at zero at stall speed and at neutral pitch, reaching its full scale
25% below stall with at least 50% back-stick. Rudder remains linear. For
spinEntry=1 (Rafale), additionally multiply driving torque by 0.5 as an agent
fit for reduced susceptibility. spinEntry=2 still disables entry.

At zero initial rotation and full back-stick/rudder, the A-4/F-14 driving
acceleration is about 25 degrees/s² at 95% of stall speed and 77 degrees/s² at
90%, versus about 244 and 219 before this adjustment. Deeper departure can
still build stronger rotation. No elapsed-time gate is added. Opposite-rudder
braking keeps its previous strength; the soft-onset factor only affects torque
that drives the current spin. These are fitted values, not aircraft measurements.

## Rotation and controls

Store signed residual spin yaw velocity in rad/s. Entry starts at zero velocity;
there is no imposed minimum rate or timed buildup. Limit its magnitude to the
source PT maximum spin yaw rate. Wrong rudder accelerates rotation; opposite
rudder decelerates it in proportion to input, including below clean stall speed.
Prevent the spin torque from reversing direction through zero; a subsequent
opposite spin needs a fresh entry. Ordinary rudder turning remains available.

Let f be residual spin speed divided by its maximum, clamped to 0..1. Let q be
(speed/clean-stall-speed)², clamped to 0..4. Control effectiveness is
(1-f) + f * (0.25 + 0.75*max(forward-airflow-dot,0)²)/(1+2*f²).
This reduces control response in fast rotation and poor airflow without making
it zero. All coefficients in this paragraph are fitted agent decisions.

During active spin, angular acceleration along the spin direction is
1.5*maximum-spin-rate*rudder-in-spin-direction*min(q,1)*effectiveness,
multiplied by the soft-entry departure factor when rudder drives the spin,
plus 0.2*q*(1-stability)*current-spin-rate for fitted autorotation, minus
q*(0.35+2*stability)*current-spin-rate for damping. Integrate at 120 Hz and
clamp between zero and maximum rate. Stability is the product of smoothstep
weights: zero to full over 1.1..1.5 times clean stall speed and 45..25 degrees
of nose/airflow separation. Neutral input can damp rotation, but has no fixed
recovery duration. Wrong rudder can rebuild rotation after partial recovery.

Add residual yaw to normal aircraft rotation rather than replacing it. Keep
nose-to-airflow alignment active. Reduce normal rudder and roll response by the
control effectiveness above. Blend normal pitch target with a direct elevator
pitch target of 40 degrees/s * pitch-stick * effectiveness * min(q,1), using f
as the blend weight. Retain the aircraft's normal pitch response smoothing
(0.1 seconds for A-4/F-14). This is actuator/response smoothing, not a recovery
countdown. Remove the old fixed 8.6-degree/s spin pitch rotation. Forward stick
therefore moves the nose directly, even before rotation has slowed or recovery
criteria have been met. Retain the fitted lift-command reduction 1-0.85*f.

## Recovery completion

Clear spin when residual yaw is no faster than normal full-rudder response
(model rudder_rate * min(q,1)), with airflow within 25 degrees of the nose. Rudder must no longer be driving the active spin direction. No
extra timer, throttle condition, or clean-stall+10 speed condition applies to
arresting rotation. If speed remains below clean stall, return to Stalled, not
Normal: a pilot can stop an incipient spin without having recovered wing lift.
Above clean stall, clear the departure warning. A-4/F-14 normal rudder response
is currently a fitted 0.12 rad/s (about 6.88 degrees/s).

After clearance, retain residual yaw and damp it using the same rate-dependent
damping term, without the spin-driving term. Do not discard remaining angular
velocity on the threshold tick. A subsequent stall with pro-spin inputs can
initiate a new spin. Ground contact clears rotation and departure state.

## Limits

This remains a fitted rotational model, not a rigid-body aerodynamic solver.
Ordinary stall classification uses the clean-envelope speed gate. Full
angle-of-attack stall classification, tail blanking, aircraft-specific inertias
and the source A-4 recovery lock remain unresolved. Next research is FA departure
motion evidence and aircraft-specific control authority; retail comparison is
not an acceptance blocker.
