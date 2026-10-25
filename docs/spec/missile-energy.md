# Energy-based powered guided weapons

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-25. **In development, experimental opt-in only, not the live default.**
John requested energy-limited guided weapons, lead guidance, greater drag low
down, flare response to target heat/aspect, and one uniform model. His Phase 2
decisions require real-world data where available and physical decoy pursuit.
This document owns the new behaviour. The existing [missile specification](missiles.md)
continues to describe the compatibility and previous Spec flight paths.

## Scope and provenance

All currently supported powered guided weapons use the same force kernel:
air-to-air, surface-to-air, ship strike, powered air-to-ground, laser and command.
Unpowered guided bombs and unguided rocket pods retain their current models.
Weapon identities, supported roles and availability do not change merely because
a real-world missile of that name has different guidance. Aircraft flight
adapters remain distinct and unchanged.

Energy flight and the countermeasure consequences below are opinionated,
requested by John on 2026-10-25. The coefficients, response times and derivation
rules are agent-fitted game approximations, not recovered retail behaviour.
Public mass/dimension/range facts must carry per-field source references in the
data documentation. A published maximum is conditional on launch geometry,
not a guaranteed range against every target. Unknown aerodynamic quantities
remain labeled fitted or estimated.

## Motion

At fixed 120 Hz, the body inherits the launcher's world velocity once, plus its
ejection vector. A drop does not erase downward velocity at ignition. Motor
phases contain duration, thrust and propellant mass, with dry mass as a hard
lower bound. Thrust ends when the phase ends. There is no velocity gain budget,
absolute speed clamp, coast-speed floor or reset to a powered speed.

The force calculation uses SI units, converting feet with exactly 0.3048 m/ft.
Dry standard air uses temperature and pressure layers at geopotential heights
0, 11, 20, 32, 47, 51 and 71 km, lapse rates -6.5, 0, 1, 2.8, 0, -2.8 and
-2 K/km. Sea level is 288.15 K and 101325 Pa, air gas constant 287.05287,
gamma 1.4 and gravity 9.80665 m/s². Convert geometric height using earth radius
6,356,766 m. Density is p/(RT), sound speed sqrt(1.4RT). The evaluated atmosphere
is bounded to geometric heights -1..85 km. Altitude means MSL, not terrain
clearance. [NASA standard atmosphere](https://ntrs.nasa.gov/api/citations/20050207438/downloads/20050207438.pdf?attachment=true).

With air-relative speed V and frontal reference area S=pi*d²/4, dynamic
pressure q=rho*V²/2. Zero-lift drag is q*S*Cd. Cd interpolates linearly at
Mach 0/0.8/1/1.2/2/3/5 through 0.30/0.30/0.65/0.55/0.36/0.30/0.26.
Cruise-class coefficients are 0.20/0.24/0.65/0.60/0.45/0.38/0.34. Endpoint
values hold outside the curve. Per-profile fitted scale is explicit data.
[Drag equation and reference-area convention](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/drag-equation/).

Lateral acceleration cannot exceed min(structural_G*g, q*S*CLmax/m). The initial
effective body-area CLmax is 12, induced-drag coefficient k is 0.03, and
induced drag is k*lift_force²/(q*S). Gravity compensation and loft demands use
this same limit and consume energy. These are fitted effective coefficients,
not wind-tunnel measurements. The [induced-drag equation](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/induced-drag-coefficient/)
motivates the law; it does not establish the selected k.

The lateral update rotates air-relative velocity without adding speed. Axial
thrust and drag update speed, then gravity changes world velocity; position
uses the average of old/new world velocities. Drag cannot reverse velocity in
one numerical step. This first-order force split with midpoint position is a
fitted integration choice. Coasting total mechanical energy decreases in still
air; a dive may increase kinetic energy at the expense of altitude.

Rocket classes initially lose control below 120 m/s, slow cruise/command
classes below 70 m/s, after 30 consecutive ticks. A powered launch may accelerate
through that threshold before control first becomes available. After losing
control the body is ballistic and cannot regain guidance by accelerating in a
dive. Ground/water collision and lifetime remove it. Ordinary seeker memory
loss remains separate and retains its specified reacquisition policy.

## Guidance

Proportional navigation uses only observed position and velocity. With relative
position r, velocity vr, LOS rotation omega=(r cross vr)/|r|², closing speed
Vc=-dot(vr,unit(r)) and flight direction f, command N*max(Vc,0)*(omega cross f),
projected perpendicular to the flight direction, plus gravity compensation.
N is 3, 3.5 or 4 by data class. Commands respond with a 0.08 s time constant and
are limited again by current available lift. [Homing guidance principles](https://secwww.jhuapl.edu/techdigest/content/techdigest/pdf/V29-N01/29-01-Palumbo_Principles_Rev2018.pdf).

Opening targets or heading error above 30 degrees use a bounded capture command
V*heading_error/0.5 s toward the observed LOS. There is a stable reversal axis
for opposite headings. Capture and PN share the same physical lift and drag.
Long-range radar profiles may request a loft waypoint; climb slope is limited
to 0.36, about 20 degrees. Profile data specify loft height and entry distance;
terminal guidance resumes at 0.4 of that distance. No A2A ground avoidance or
forced miss occurs. A sufficiently late turn/dive can cause physical ground
impact, overshoot or energy exhaustion; a short-range shot can still hit.

Seeker gimbal, instantaneous field of view, rate, acquisition dwell, support,
active-seeker activation and memory are separate from physical steering. SARH
requires its own launcher's support. Active seekers cannot see hidden target
coordinates before activation. Laser/command loss and HARM retention policies
remain as specified by the existing A-G guidance contracts.

## Countermeasure contract

John's Phase 2 decision supersedes the proposed inert one-second cleanup:
successful decoy transfers guidance to a physical flare/chaff object. Its
position and motion are deterministic simulation state and are checkpointed.
Flares eject and fall under gravity/drag until burnout; chaff slows toward the
air mass. Rendered-device capacity cannot delete a guiding missile's decoy.

The missile uses the same finite-energy guidance against that device. It cannot
fuze on a device. It can still fuze on a real aircraft while pursuing the decoy,
including a late flare beside the original target. After passing the decoy it
permanently ends guidance and coasts/falls until ground collision or lifetime,
without reacquiring any aircraft or device. Motor thrust is not replenished.

One roll per missile/releaser/device-class in a two-second episode. Extra
cartridges in that episode consume ammunition and appear physically but do not
compound the odds. IR odds use target heat, exhaust aspect, received flare-to-
target signal ratio and seeker generation. Imaging seekers strongly resist
flares. A beam turn reduces exhaust view; it is not an IR Doppler notch.
Radar uses target radial separation from terrain clutter for both SARH and
active seekers. Chaff and look-down notch improve seduction; active radar may
also lose a return in clutter without chaff. Clamp the full probability product,
preserving raw susceptibility values 150 and 200. The experimental evaluator uses raw susceptibility times dispenser effectiveness,
then generation and signal factors, flooring the final probability to a whole
percent. IR factors are 1.0 for rear-aspect seekers, 0.6 for conventional
all-aspect and 0.08 for imaging, with ratio capped at 3 and probability at
95% (10% for imaging). Radar uses 0.35 for active seekers, 1.0 for records
whose susceptibility exceeds 100, otherwise 0.7, multiplied by
0.25 + 2.75*clutter_notch. This susceptibility-based early-SARH grouping is
fitted and awaits explicit per-profile review.

The physical lure leaves 15 ft behind and 2 ft below the aircraft, with its
velocity plus 80 ft/s right and 15 ft/s down for a flare. This deterministic
representative of the visible pair is fitted. Flare terminal fall speed is
100 ft/s, chaff velocity decays toward wind plus 4 ft/s down with a 0.12 s
time constant. First-second devices may seduce; captured devices continue
independently of the scene registry. Flare/chaff burnout is 30/20 seconds.

Seeker slew is 15 degrees/s with a 4-degree instantaneous half-angle for
rear-only IR, 90/10 for imaging, 35/5 otherwise. Imported mechanical angles
remain. Look-down clutter scales from zero to full at five degrees depression.
Doppler separation within 60 ft/s produces notch strength; 24 ticks at strength
at least 0.75 loses the return, and 30 ticks below 0.5 restores eligibility.
These are fitted classes. Mounted cues and all estimates still need their
new-model integration before the mode becomes the default.

## Estimates and acceptance

Flight, range, TTI and AI launch decisions use the same model or a bounded
cache built from it. No obsolete straight-speed/coast-floor shortcut may
authorize a new-model intercept. Keep observed support/seeker gates separate
from physical reach. Player range and lock remain advisory at release.
The existing AI defensive schedule and flying are owned by a separate task.

Acceptance requires physical and seeker-valid A2A ranges at 5k/15k/35k ft for
head-on/beam/tail, SAM and A-G calibration, and the extended missile study.
Measured estimate overhead must stay about 2 ms per world tick with eight AI
aircraft and sixteen missiles, reporting cold/warm p50/p95. If the gate cannot
be met, report options instead of enabling a synchronous fallback.

Keep explicit old weapon modes and old combat input-tape resimulation.
Mission recordings replay their recorded projectile poses. New motor display
state must not reinterpret old recording ages through new motor tables.
The lead owns all protocol/simulation/tape/format bumps. No unmeasured new
fingerprints or real-world parity claims are acceptable.
