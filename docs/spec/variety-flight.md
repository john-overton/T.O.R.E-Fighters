# Variety aircraft flight

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-10-05. This is a source-informed, fitted contract
for the [23-aircraft batch](aircraft-variety.md), not measured retail parity.
The reviewed BRF records come from the local retail FA_2.LIB catalog; the
[source registration](../formats/aircraft-variety.md#source-build-and-bounded-review)
records archive identities and dependency evidence. Exact
identity, mass, internal fuel, maximum takeoff weight, thrust, fuel flow,
G envelopes, departure/contact limits and roll controls are decoded using the
[aircraft schema](../formats/aircraft.md). Each aircraft owns its configuration.
Sharing the integration law does not erase F-4 fuel, envelope or equipment
differences. No autonomous pilot behavior is specified or changed.

## Conventional flight

The seventeen conventional aircraft use the existing hybrid envelope, lift,
drag, ground contact and loaded-mass rules. Source roll rate, acceleration and
release determine their roll response. The following initial response fits are
agent decisions: all use 0.2-second roll lag, 0.1-second pitch lag, 2-degree
trim, 1.25 degrees of extra trim per G, 70,000-foot exponential thrust lapse,
0.12-radian/second rudder response, slip drag 0.5, slip damping 0.8/second,
and slip roll 0.35 radians/second. Nose alignment is 0.7/second for fighters,
0.4 for transports and 0.3 for airliners. Stall reference weight is empty
weight times 1.0 for every new conventional aircraft, an explicit uncalibrated
fit rather than an existing fighter's reference fraction.

Actuators take 3 seconds, exhaust response 0.2 seconds, control deflection
0.1 seconds and throttle travel 0.35/second. Afterburner requires throttle above
0.95 and source afterburner thrust. Tire scrub is 8/second, rolling resistance
0.8 ft/s squared and brakes 18 ft/s squared. Ground clearance fits use the lowest deployed original gear point times its
host SH scale. AH64 and CH47 use the lowest point of their fixed-gear mesh.
These are reviewed geometry fits for the contact plane, rather than measurements
of original flight behavior. Hook availability uses the reviewed configuration,
independent of flight family.

| Aircraft | Ground clearance, feet |
| --- | --- |
| C130 | 14 |
| AC130 | 40/3 |
| E3 | 38/3 |
| IL76 | 50/3 |
| E2 | 28/3 |
| AV8 | 17/3 |
| YAK141 | 7 |
| V22 | 13 |
| AH64 | 23/3 |
| MI24 | 10 |
| CH47 | 29/3 |
| MIG17F | 14/3 |
| F4B, F4J | 20/3 |
| F4E, F4G | 7 |
| A7 | 25/3 |
| F15 | 9 |
| F16C | 7 |
| F104 | 13/3 |
| A10 | 22/3 |
| B747 | 20 |
| A310 | 14 |

Airborne starts for the new conventional aircraft and VTOL jets use 65 percent
of their own top speed at the start altitude, bounded above by 95 percent of
top speed and below by 130 percent of clean stall speed. Helicopters and V22
start level at zero speed, full engine power and collective chosen to balance
the configured mass and altitude lapse, capped at full collective. This is an
agent-authored initial condition; it does not add a hover controller. An airborne
Quick Mission recomputes this initial collective once after its selected altitude,
fuel, stores and tanks are applied. Above available lift capacity it uses full
collective and falls rather than inventing support. After the first tick, mass
changes never retrim collective automatically. Ground starts retain the existing
idle/brakes/gear setup and clear collective and lagged lift.

## Powered lift and controls

AV8 and YAK141 nozzle pitch is normalized 0 (forward) to 1 (90 degrees down),
with signed yaw producing up to 15 degrees of lateral thrust. Neutral resets
both nozzle demands to forward/center and V22 conversion to forward airplane
mode, preserving collective and engine power. V22 conversion is 0 (airplane) to
1 (90-degree helicopter). Rotorcraft collective is independent of engine power;
the ordinary throttle remains engine power. Held pitch/conversion controls
move 0.25 of travel per second, vector yaw 1 per second, and collective
0.35 per second. Absolute axes set demands directly. Actual nozzle/nacelle
pitch moves at 0.25/second and actual vector yaw at 1/second. Collective
responds at 0.7/second. Demands, actual positions and lift response persist in
fixed-tick state and exact snapshots. Irrelevant actions do nothing.

These continuous 120 Hz force laws are fitted agent choices. Rotor lift is
source military thrust times an aircraft-specific efficiency, collective,
engine power, damage power and the existing exponential altitude lapse.
Efficiencies are AH64 0.98, MI24 0.84, CH47 0.245 and V22 1.10. CH47's
tandem arrangement has no tail-rotor torque or fictitious tail-rotor failure.
The modest efficiency on its much larger source thrust avoids copying another
helicopter's performance. AV8 uses source thrust. YAK141 adds a fitted
18,000-lbf lift-engine contribution at full power and full vectoring. Source
afterburner can contribute only with less than 20 percent nozzle travel.
Lift response is 0.35 seconds AV8, 0.45 YAK141, 0.65 V22, 0.45 AH64,
0.60 MI24 and 0.85 CH47. These fits do not establish retail force behavior.

At low speed the stick commands bounded attitude, with release retaining a
level target. Pitch and bank targets are 20 and 25 degrees for AV8/YAK141,
20 and 25 for V22, 20 and 30 for AH64, 18 and 25 for MI24, and 15 and
20 for CH47. Attitude response is 2/second, bounded to 45 degrees/second.
Yaw rates at full hover support are respectively 35, 30, 30, 45, 35 and
25 degrees/second, each bounded by its own decoded powered-control axis.
AV8 and YAK141 therefore retain their source 20-degree/second yaw and pitch
rate bounds, while the fitted roll rate stays under their 50-degree/second
source roll bound. Thrust follows actual attitude, so tilting translates and
reduces vertical support. Yaw control needs engine power; attitude authority scales with lagged lift divided
by weight, capped at 1. No automatic height
or position hold exists. Neutral stick levels the aircraft but does not cancel
velocity. Low-speed horizontal damping is respectively 0.06, 0.07, 0.10,
0.14, 0.12 and 0.10/second; vertical damping is 0.35/second times hover fraction plus 0.5/second times
forward wing authority. Forward drag
adds acceleration proportional to speed squared using source top speed and
thrust. Lift is limited by fuel and payload weight; full collective cannot
hold a load exceeding the available lift. No autorotation or detailed rotor
vortex-ring model is claimed.

AV8/YAK141/V22 wings gain support continuously with forward airspeed squared
relative to the source 1-G stall speed. Hover attitude controls fade into
forward-flight pitch/roll as vectoring/conversion decreases and forward
airspeed grows. Rotorcraft have no fixed-wing stall warning. AV8, YAK141 and V22 use the
reviewed warning timers below half conversion; above half conversion the warning
clears. Detailed spins are omitted in the fitted powered-lift solver, while
low forward airspeed still reduces wing lift continuously. Ground contact
uses the aircraft's source landing limits and current gear on landable dry
surfaces. A level vertical landing with less than 5 ft/s descent is the
acceptance scenario; harder landings remain subject to each source limit.

Synthetic acceptance: all six sustain a level hover within 5 feet over
10 seconds at an explicitly calculated collective/power for their test mass;
increasing lift climbs and reducing it descends; stick tilt translates, yaw
turns, and engine-off loses lift. V22 and VTOL conversion advances gradually
with matching actual state and finite motion; vertical departure and landing
work on a flat runway. Cloned configurations remain independent. Identical
inputs and exact snapshot restoration produce identical future ticks. The
legacy adapter and restricted native research path retain their existing
behavior and limitations.

## Unknown evidence

Retail hover power, altitude ceiling, nozzle travel/rate, rotor torque, lift
engine scheduling, detailed rotor physics and transition losses remain unknown.
Next research is a bounded review of the original force consumers or measured
retail behavior when available. Those gaps are filled by the explicit fits
above and do not claim original gameplay parity.
