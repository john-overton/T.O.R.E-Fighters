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
AV8 includes its always-present central nose wheel at source Z=-21; using only
the switched outriggers at Z=-17 left that wheel 4/3 ft below the runway.
These are reviewed geometry fits for the contact plane, rather than measurements
of original flight behavior. Hook availability uses the reviewed configuration,
independent of flight family.

AH64 and CH47 use a fitted fixed-gear rule, agent decision 2026-10-05 from
reviewed always-present gear geometry. Their state starts with gear down and
keeps that position through commands and simulation ticks. A manual toggle
reports `Landing gear is fixed on this aircraft`; explicit extension commands
remain quiet. This prevents a normal landing being treated as gear-up while the
original fixed wheels are visible. Mi24 remains retractable. This rule does not
claim recovered retail actuator logic or add a new binding.

| Aircraft | Ground clearance, feet |
| --- | --- |
| C130 | 14 |
| AC130 | 40/3 |
| E3 | 38/3 |
| IL76 | 50/3 |
| E2 | 28/3 |
| AV8 | 7 |
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

Airborne starts for the new conventional aircraft and the powered-lift aircraft
use 65 percent of their own top speed at the start altitude, bounded above by
95 percent of top speed and below by 130 percent of clean stall speed. The
altitude is the mission's: a single-player or multiplayer airborne start picks
the speed after the selected altitude is applied, and AI aircraft pick it at
their own spawn altitude. Ported fighters keep their fixed 450 knots.

The AH-64, Mi-24, AV-8 and Yak-141 (VTOL overhaul decision 8, slice P7) start in
**trimmed forward flight** at that speed, not a hover, on every spawn path:
single player's restart, an AI actor put on the hybrid model (a multiplayer seat
is one until a human takes it) and a revival. One trim routine
(`crates/tore-sim/src/flight/powered/trim.rs`) runs after the final mass,
altitude and heading are set. A single-rotor helicopter gets the collective,
cyclic and pedals of its rotor trim with the rotor governed at 100 percent and
the body at rest. A jet gets its nozzles at 0 and its lift engines off, and a
throttle, pitch and small pitch trim found by probing the force law one tick at
a time, so that hands off it holds height within 10 feet and speed within 2
knots for ten seconds (acceptance S1). A start above a helicopter's ceiling
tries a hover and then falls back to full collective; it never invents support.
The CH-47 and the V-22 still start level at zero speed with full engine power
and the collective that balances the configured mass and altitude lapse
(capped at full collective), until their slices land. After the first tick,
mass changes never retrim anything.

Ground starts match the fixed-wing ones: stationary, engine running at idle,
gear and flaps down, brakes on, autopilot off. A helicopter has its rotor at the
governed speed with the collective down and its engines at 100 percent (the
throttle keys drive the collective, so the engine throttle is set at the start),
a jet has its nozzles at 0 and its lift engines off. There is no cold start. The
V-22's nacelles will start at the 87-degree helicopter preset with its slice.

## Powered lift and controls

**AV8 and YAK141 since the VTOL overhaul's slice P4 (2026-10-08).** The two
jets no longer fly the fitted law below. Nozzle pitch 0 to 1 is now 0 (aft) to
the PT's 100-degree braking stop, slewed at the PT's 100 degrees per second;
there is no vector yaw. Thrust follows the nozzles through a spooling engine
and a vertical efficiency (AV-8 0.75, Yak-141 0.92 with its 18,000 lbf lift
engines for takeoff and landing only), puffer jets give bleed-driven moments
under the stability levels, intake momentum drag and a jet-induced dihedral
make the low-speed roll-off, suck-down takes up to 6 percent near the ground,
and an angle-of-attack wing matched to the conventional model on the same PT
carries the aircraft in forward flight. The overhaul's final pass rewrites this
section; until then the paragraphs below describe the V22 and the helicopters
(the jets' parts of them are history), and the jets' rules are in
`crates/tore-sim/src/flight/powered/jet.rs` and `aero.rs`.

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
0.14, 0.12 and 0.10/second, times hover fraction, divided by 1 plus airspeed
over 10 ft/s, so its force stops growing above a few knots (agent decision,
2026-10-08; before that it acted at every speed and held the helicopters near
40 kt). Vertical damping is 0.35/second times hover fraction plus 0.5/second times
forward wing authority. Forward drag grows with the square of airspeed and
equals a reference force at the envelope's top speed at that altitude, times
the source loaded-drag factor. For AV8, YAK141 and V22 the reference is their
rated thrust, afterburner included when the PT has one, as in the fixed-wing
adapter; before 2026-10-08 it was military thrust only, so YAK141 at full
burner flew past its own top speed. For helicopters it is their weight times
the tangent of their full pitch target, less the saturated damping force, so
full forward stick in level flight settles just under the top speed (agent
decision, 2026-10-08). Lift is limited by fuel and payload weight; full collective cannot
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

## AI wingmen

Opinionated, agent decision 2026-10-08: the AI does not fly the helicopters
(AH-64, Mi-24, CH-47), the V-22, the AV-8 or the Yak-141 yet. On the legacy
adapter the rotorcraft start at zero speed, drop about 400 feet and fly like
airplanes (the CH-47 pitches to 69 degrees). A headless probe of the AV-8 and
the Yak-141 shows they fly an airborne fight and land, but on a ground start the
second wingman never leaves the taxiway (333 seconds checked, against a takeoff
at 66 seconds for the F/A-18D and 54 seconds for the F-15). All six stay
player-flyable.

- Quick Mission's five AI wing fields (friendly wings 2 and 3, enemy wings 1 to 3)
  do not list them. Friendly wing 1 shares the player's list, so a player who
  picks one flies with no AI wingmen: the wing count drops to one, with a notice.
  The multiplayer lobby's wing 1 is for people and keeps its size.
- A mission file or lobby spec that puts one in a wing other than friendly
  wing 1 is refused (`AircraftId::ai_flyable`).
- The probe options `--probe-enemy-aircraft` and `--probe-friendly-aircraft`
  refuse them, and `--probe-matrix` skips them as opponents.

A planned VTOL and helicopter overhaul will restore them as AI wingmen; this list
is the one switch to undo then.

## Top speeds

Units: PT envelope speeds are true airspeed in ft/s at each altitude; the
tables below give knots true (ft/s divided by 1.68781). The right edge of the
1 G row is the top speed: the hybrid drag reaches full thrust there, the
[envelope window](envelope.md) draws it and the [overspeed](overspeed.md) rule
uses it as the structural limit. The drag reaches full thrust at that edge
times a level-speed fraction, so level flight at full power settles there
divided by the square root of 1 plus loading times the source loaded-drag
percent. The fraction is 1 for every aircraft except the transports and
airliners below; fighters, with full internal fuel, settle at 87 to 96 percent
of the edge.

**Heavy level-speed fraction (fitted, 2026-10-08).** C130, AC130, E3, IL76,
E2, B747 and A310 have a source loaded drag of 0, so at full power they flew
right at the edge, inside the overspeed shake band (from 95 percent). John
asked on 2026-10-08 for a little drag so they top out a few percent under it.
Their fraction is 0.96, an agent choice: full power in level flight settles
at 96 percent of the top speed at any fuel load, just out of the shake band.
It reuses the drag normalization every aircraft already has rather than a
fuel-dependent loaded-drag value, which would return them to the edge as fuel
burns off.

The variety numbers are fitted contract values. On 2026-10-08 an agent
compared the decoded 1 G top speed, the simulated level top speed (headless
probe, full power, full internal fuel) and published figures for all 23
aircraft; the measurements are in the
[validation record](../baselines/variety-flight.md#top-speed-pass-2026-10-08).
Six decoded envelopes are corrected below: five were clearly wrong, and the
E3 is capped like the airliners at John's request (2026-10-08). These are
agent decisions, not retail behavior. The decoded PT values stay in the
imported data and its reports; only the flight model's copy, which the
overspeed rule and envelope window also read, is corrected.

| Aircraft | Decoded 1 G top, kt | Fitted 1 G top, kt | Rule | Reason and source |
| --- | --- | --- | --- | --- |
| AC130 | 338 at sea level, 334 at 20,000 ft | 261, 258 | Fast-side speeds of every row times 261/338 | USAF AC-130U fact sheet: 300 mph (261 kt) at sea level. The decoded envelope is the C-130's. |
| V22 | 130 at sea level; ceiling 7,000 ft | 276 at sea level, 256 at 20,000 ft; ceiling 25,000 ft | Fast-side speeds times 275/130, all altitudes times 25,000/7,000 | Decoded as a copy of the AH-64 envelope. Published V-22: 275 kt at sea level, 25,000 ft service ceiling (Wikipedia citing Aviation Week; Naval History and Heritage Command). |
| AH64 | 130 at sea level | 158 | Fast-side speeds times 158/130 | Published AH-64 maximum level speed 158 kt (Vne 197 kt). |
| B747 | 492 at every altitude | 375 at sea level, 429 at 10,000 ft, 492 from about 18,000 ft | Capped at VMO 375 KCAS and MMO 0.92, standard atmosphere | EASA TCDS IM.A.196, 747-400. The decoded edge was right at cruise altitude but 31 percent over VMO at sea level. |
| A310 | 456 at sea level, 479 at 20,000 ft | 360, 412 at 10,000 ft, 475 at 20,000 ft, unchanged above | Capped at VMO 360 KIAS and MMO 0.84 | EASA TCDS EASA.A.172, A310-300 basic VMO. |
| E3 | 462 to 20,000 ft, 438 at 30,000 ft | 375 at sea level, 436 at 10,000 ft, 462 from about 13,000 ft | Capped at the VMO schedule 375 KIAS at sea level, 381 at 10,000 ft, 385 at 15,000, 390 at 20,000, 394 at 23,000, and MMO 0.887 | No E-3 limit is public. Analogue: FAA TCDS 4A26 revision 11, part III, 707-300B series. The E-3 is a 707-320B airframe, and its TF33 engines are the military JT3D that the 707-300B uses. |

The capped rows: the 1 G row's fast-side vertices take the calibrated-speed
cap, with vertices added every 5,000 ft up to 35,000 ft so the edge follows
the curve. Every other row's fast side shrinks by the same ratio at each
altitude and never reaches past the 1 G edge. Scaled rows keep the top vertex,
and fast-side points never move left of it. Slow sides are untouched apart
from the V22 altitude scale, so stall speeds stay decoded.

The model fixes above (helicopter damping and drag, YAK141 afterburner drag)
change the simulated top speed without touching an envelope: AH64 from about
37 kt to 154 kt at 1,000 ft, MI24 from 39 to 168 kt (published 173 to
181 kt), CH47 from 39 to 164 kt (published 170 kt), and YAK141 from a descent
past its edge to 659 kt at 1,000 ft (published 675 kt at sea level).

Left as decoded, within about 8 percent of the published figure at the
altitude it applies to, or with no reliable figure to fit to:

- C130: 334 kt at 20,000 ft against the C-130H's 320 kt there (4 percent).
- IL76: 462 kt against 459 kt at 11,000 m.
- E2: 314 to 322 kt against 325 to 350 kt in conflicting sources.
- AV8, MIG17, F-4 family, A7, F15, F16C, F104 and A10: the decoded edge is
  within about 8 percent of the published sea-level or altitude figure. The
  simulated level speed is lower at full internal fuel because of the shared
  loaded-drag rule, as for the original roster.

With the fitted edges no aircraft exceeds its top speed in level flight at
full power, so overspeed still needs a dive, and with the level-speed fraction
the transports and airliners cruise flat out about 4 percent under it, out of
the shake. Past the edge the
[fast-side hold](../FLIGHT-MODEL.md#envelope-limits-and-loading) keeps their
pull, so they can climb back under it within the five safe seconds.

Not changed and recorded for later: decoded ceilings that differ from
published ones (helicopters 7,000 ft, A10 22,812 ft, F-4 42,000 ft, IL76
51,000 ft, E3 30,000 ft, at which its envelope has no row above 1 G), and the
V22's mass, fuel and thrust, which are also the AH-64's.

## Unknown evidence

Retail hover power, altitude ceiling, nozzle travel/rate, rotor torque, lift
engine scheduling, detailed rotor physics and transition losses remain unknown.
Next research is a bounded review of the original force consumers or measured
retail behavior when available. Those gaps are filled by the explicit fits
above and do not claim original gameplay parity.
