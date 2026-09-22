# Destroyed aircraft motion and airbursts

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Requested behavior

Implementation mode. John requested aerodynamic ragdoll motion and random airborne
explosions on 2026-09-21. A destroyed aircraft keeps its position, orientation and
momentum at destruction, then falls and tumbles under gravity and air resistance.
It cannot remain frozen or fly under pilot/AI control. This applies to ownship
and other airborne aircraft, without changing living AI decisions.

While the wreck remains airborne, roll a 5% explosion chance at elapsed seconds
1, 2, 3, 4 and 5, then at 10, 15, 20 and every five seconds thereafter. There is
no roll at time zero or after ground impact. Ground contact takes priority over
a scheduled roll on the same simulation tick. Each poll is independent.

A successful roll creates the existing aircraft explosion effect and sound at
the wreck's location, removes the entire aircraft model and associated detached
parts, and stops further wreck motion and rolls. It does not award another kill
or damage unrelated aircraft. A failed roll leaves the wreck falling. Ground
impact stops the wreck and all polling. For the player aircraft, ground impact
also causes a guaranteed explosion, removing the model and its detached pieces.
This applies both to a falling wreck and a direct fatal ground collision, never
a safe landing. An earlier airburst prevents a second impact explosion.
The player wreck continues emitting aircraft damage smoke while falling, even
with a dead pilot. Ground impact or an airburst stops emission; existing puffs
finish fading normally. Normal restart clears this state.

As further requested by John on 2026-09-21, losing the player aircraft's nose or
cockpit kills its pilot immediately. Timed wound death, a critical cockpit hit,
an airburst and fatal ground impact also mark the pilot dead. A dead pilot cannot
be revived by reaching the ground. Pilot death does not shut down surviving
engines in an airborne wreck.

On the alive-to-dead pilot transition, select the F10 exterior chase view, reset
look and zoom, and close the navigation map so the falling aircraft is visible.
Do not pause simulation or repeatedly force the camera on subsequent frames.

These are opinionated requested behaviors, not recovered retail behavior.

## Fitted aerodynamic component

The shared renderer-independent wreck component advances at 120 Hz. It retains
an orthonormal attitude basis and starts with a deterministic random tumble bias;
ownship also inherits its current pitch/roll rates. Gravity is 32.174 ft/s².
Per-axis quadratic drag coefficients in body right/up/forward directions are
0.0018, 0.0024 and 0.00012 per foot. Each drag velocity decrement is capped at
half the component speed per tick to keep integration stable.

Tumble torque is driven by airflow across the body and a fixed random asymmetry.
Airflow strength is airspeed/400 ft/s, capped at 2. Angular damping is 0.6/second;
angular rates are capped at 3 radians/second. Bias acceleration is at most
0.9 rad/s² per axis at unit airflow strength. Pilot control and intact-aircraft
lift cease after destruction. This is a rigid wreck approximation rather than
articulated joints or a new material-fracture model.

As John additionally requested on 2026-09-21, surviving engines keep pushing the
wreck. Capture throttle, afterburner, available engine power and fuel at death.
Thrust accelerates along the tumbling body's forward axis, not its old flight
path. Actual source thrust and current mass determine forward acceleration.
Ownship retains altitude lapse and individual shutdown state; fuel flow uses the
source rate scaled by the remaining engine-power fraction.
a twin-engine aircraft can retain one engine with off-center thrust. Available
partial power is shared across the surviving engines. Flight snapshots provide
the same information for other simulated aircraft. Straight-flight fixtures use
source military thrust at their existing 70% throttle and initial loaded mass.

Engine lateral positions are a fitted normalized span from -1 to +1 (a single
engine is centered). Each contributes yaw acceleration of `-offset * forward
acceleration * 0.03` rad/s². Balanced engines cancel this torque; one surviving
engine does not. Remaining fuel and its captured consumption rate limit thrust
lifetime. Ground impact or an airburst ends thrust. No extra random engine-failure
roll or engine restart is invented after destruction. Engine health is captured
at death; detailed post-destruction thermal/leak changes remain outside this pass.

A wreck owns separate motion and explosion random streams seeded from its stable
object ID and wreck initialization tick. Each explosion draw is in 0..99, succeeding below
5. Neither rendering, pause, other wrecks nor combat RNG consumption changes its
poll schedule. Motion initialization never consumes its explosion stream.

## Presentation and integration

Destroyed target orientation comes from the wreck basis, including straight-flight
fixtures. Ownship continues fixed-tick motion after its destroyed flag is set.
Airbursts hide ownship exterior, cockpit and mirrors, but leave the existing
instruments/log available. Destroyed targets remain observable only until they
hit the ground or explode. Explosions use the existing retail effect assets
loaded at runtime. Damage before destruction and the current visual-damage gate
remain as specified in [airframe damage](damage-smoke.md).
