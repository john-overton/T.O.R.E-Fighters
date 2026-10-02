# Rudder and nosewheel steering

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Behavior

Implementation mode. John requested working rudder side force and the following
nosewheel schedule on 2026-10-01. These are opinionated requirements; the force,
roll and wheelbase constants are fitted agent decisions, not retail measurements.

In the legacy and researched adapters, yaw creates sideslip. A lateral force
opposes air-relative motion across the fuselage, turning the velocity toward
the nose. Bank then supplies its usual lift-driven turn. Existing rudder filtering,
slip drag, damage authority and stall/spin control reductions remain active.
The restricted native-table adapter remains separate.

Each aircraft owns a lateral damping coefficient of 0.8 per second and a roll
coupling of 0.35 radians per second per unit of lateral airspeed fraction.
Both scale by the existing low-speed aerodynamic authority. Roll coupling fades
with departure control authority. Positive rudder produces right yaw, a leftward
relative slip, rightward side force and right bank. Side force is applied across
the body, so it dissipates slip energy rather than rotating velocity instantly.
The lateral aerodynamic force is omitted while wheels carry the aircraft;
existing tire friction handles lateral ground motion.

The nosewheel takes rudder deflection while supported with gear down. Its maximum
angle is 90 degrees through 10 mph ground speed, then decreases linearly to zero
at 25 mph. At and above 25 mph only aerodynamic rudder yaw remains. Ground speed
is horizontal world velocity, independent of wind. A stationary wheel can turn
without rotating a stationary aircraft. The fitted effective wheelbase is 18 ft;
yaw rate is signed forward ground speed divided by wheelbase times the sine of
wheel angle. This bounded tire model remains finite at 90 degrees and reverses
steering when rolling backward. Tire friction turns the path with the body.

The HUD shows `NSW 100%` directly below BRAKE while gear is fully down and
the wheels carry the aircraft (John, 2026-10-02). This is available authority,
not pedal deflection: centered pedals still show 100% at low speed. The display
rounds to a whole percent and hides when that rounded value is zero, including
at and above 25 mph. It also hides in flight and with gear retracted. John asked
for the inactive readout to disappear on 2026-10-02. Presentation choices are
fitted; the sim supplies the fraction and its steering law is unchanged.

Unknown: aircraft-specific lateral stability derivatives, wheelbases and original
nosewheel geometry. Next research is bounded recovery of measured response if
closer tuning is required. These constants are documented approximations.
