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
Two records are corrected before use (agent decisions on John's request of
2026-10-13, [aircraft record corrections](aircraft-records.md)): the C-130's
military thrust is the AC-130U record's 40,000 lbf instead of 108,192, and the
MiG-17F's internal fuel is 2,520 lb instead of the MiG-21's 4,534, with fuel
flows of 1 and 4 lb/s instead of 3 and 20.

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

The AH-64, Mi-24, CH-47, V-22, AV-8 and Yak-141 (VTOL overhaul decision 8) start in
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
The V-22 starts wingborne with its nacelles on the downstops (its own trim,
refined by the same probing so it holds height hands-off), or in a hover at its
87-degree helicopter preset where no airplane-mode trim exists. The CH-47
starts like the single-rotor helicopters. After the first tick, mass changes
never retrim anything.

Ground starts match the fixed-wing ones: stationary, engine running at idle,
gear and flaps down, brakes on, autopilot off. A helicopter has its rotor at the
governed speed with the collective down and its engines at 100 percent (the
throttle keys drive the collective, so the engine throttle is set at the start),
a jet has its nozzles at 0 and its lift engines off. There is no cold start. The
V-22's nacelles start at the 87-degree helicopter preset.

## Powered lift and controls

The six powered-lift aircraft (AV8, YAK141, V22, AH64, MI24 and CH47) do not
share the conventional hybrid law. Since the VTOL and helicopter overhaul
(October 2026) each flies a six-degree-of-freedom rigid body: angular rates are
state, moments come from rotors, puffer jets, nozzles and wings, and inertia sets
how fast anything responds. There are no attitude limits and no hidden velocity
damping. In short:

- **Helicopters** (AH64, MI24, CH47) fly rotors with a rotor-speed state:
  momentum-theory inflow, translational lift, ground effect, power-limited climb
  and top speed, autorotation, the vortex ring state, torque and retreating blade
  stall. The throttle controls drive the collective; the engines are governed.
  The CH47 is a tandem pair with no tail rotor.
- **The V22** flies two proprotors on nacelles that turn 0 to 97.5 degrees at 8
  degrees per second through an always-on conversion corridor, on a fly-by-wire
  mixer and a 110 kt stall wing.
- **The AV8 and YAK141** vector thrust through nozzles that travel 0 to 100
  degrees at 100 degrees per second, steer in a hover with bleed-driven puffer
  jets, and carry an angle-of-attack wing matched to the conventional model on the
  same PT. The Yak-141 adds lift engines for takeoff and landing.
- **One dial.** The stability level (Off, Damper by default, Attitude) and the
  Easy flight physics cheat are the accessibility settings; hover hold (Ctrl+Alt+A)
  is an autopilot mode for the helicopters and the V-22.
- **Starts.** Airborne starts are trimmed forward flight; ground starts match the
  fixed-wing ones.

The behaviour, the physics per type, the controls, the HUD, the data sources and
the fitted values have one home: [powered-lift flight](powered-lift-flight.md).
Measured results are in the [overhaul baseline](../baselines/vtol-overhaul.md).
The fits that predate it and still apply to the top speeds are below.

## AI wingmen

Opinionated, agent decision 2026-10-08, revised 2026-10-12 (slice ST-H, John's
decision of 2026-10-11; slice H3, John's decisions of 2026-10-12): the AI flies
every selectable aircraft, the six powered-lift aircraft (AH64, MI24, CH47, V22,
AV8, YAK141) included. Slice ST-H brought in the AH-64 and the Mi-24; slice H3 the
CH-47, the V-22, the AV-8 and the Yak-141, once the AI could hover, climb and
descend a rotorcraft (slice H1). On the legacy adapter the rotorcraft start at
zero speed, drop about 400 feet and fly like airplanes (the CH-47 pitches to 69
degrees), which is why they were kept out. A 2026-10-08 headless probe of the
AV-8 and the Yak-141 found a ground-started second wingman that never left the
taxiway. Slice H2 traced it: on the powered-lift model of that day a turning jet
on its wheels kept sliding along its old track (no tyre side force or nosewheel
steering), so the wingman circled at 13 kt turning 0.4 degrees a second where the
taxi law expects about 9, and never reached the line-up point. The continuous jet
physics of the same day (nosewheel steering and tyre forces) fixed it; on the
2026-10-12 base every AV-8 and Yak-141 wingman of a wing of three or four takes
off at seven Blue fields. Since slice H2 they take off vertically or, too heavy
to hover, by the retail short takeoff ([vertical takeoff and
landing](ai-airfield.md#vertical-takeoff-and-landing)).

- Quick Mission's five AI wing fields (friendly wings 2 and 3, enemy wings 1 to 3)
  list every aircraft the player's field lists. Friendly wing 1 shares the
  player's list. A player in a helicopter, the V-22, the AV-8 or the Yak-141 has AI
  wingmen on either start; on a ground start they take off vertically (or by the
  short takeoff) since slice H2. The multiplayer lobby's wing 1 is for people and
  keeps its size; its other wings list and accept all six (the same check,
  `AircraftId::ai_flyable`, guards a mission file).
- **The CH-47 is an AI wing choice only** (John, 2026-10-12: Quick Mission has
  escort missions already, no new escort objective). It has no weapons: it flies
  its route or the mission's, defends with the helicopter break, and never
  strikes. See [AI strike](ai-strike.md#ai-helicopters-in-quick-mission), "Who
  strikes".
- The probe options `--probe-enemy-aircraft` and `--probe-friendly-aircraft` take
  all six; `--probe-matrix` (fighters against fighters) still leaves them out as
  opponents.
- How the AI flies the helicopters (their start height, the fuel schedule, the
  strike passes and the defence) is in
  [AI strike](ai-strike.md#ai-helicopters-in-quick-mission) and
  [the AI's fuel](ai.md#b48-routes-fuel-and-recovery).
- Since slice H1 (2026-10-12) every AI helicopter and the AI V-22 fly the
  powered-lift physics from the mission start, in single player too (before, an
  airborne AI rotorcraft in single player flew the legacy model, an aeroplane's),
  through the AI's own [rotorcraft control](ai.md#rotorcraft-and-jet-lift-control):
  collective for climb, cyclic for speed, coordinated pedals, a hover to a commanded
  point and the V-22's nacelle schedule. Opinionated, John's request of 2026-10-12
  for AI helicopter and VTOL hover, takeoff and landing; the mechanism is the lead's
  decision. The AV-8 and Yak-141 keep the AI flight model's choice; their jet-lift
  law flies only the hovers the airfield sequences will ask for (slice H2).

The overhaul's second project, AI wingmen, restores the other four: the AI will
fly these aircraft through their inputs on the same physics a player uses (VTOL
overhaul decision 7). Until then this list stays, and it is the one switch to
undo.

## Top speeds

Units: PT envelope speeds are true airspeed in ft/s at each altitude; the
tables below give knots true (ft/s divided by 1.68781). The right edge of the
1 G row is the top speed: the hybrid drag reaches full thrust there, the
[envelope window](envelope.md) draws it and the [overspeed](overspeed.md) rule
uses it as the structural limit (except the four rotorcraft on the hybrid adapter,
which use their never-exceed speeds, below). The drag reaches full thrust at that edge
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

The overhaul's rotor and jet physics changed the simulated top speeds without
touching an envelope. Level top speed at sea level at the PT gross weight, from
the [overhaul baseline](../baselines/vtol-overhaul.md): AH64 151 kt (published
158), MI24 177 kt (published 170 to 181), CH47 165 kt (published 170), V22 273 kt
in airplane mode (published 275), YAK141 659 kt at 1,000 ft (published 675 kt at
sea level) and AV8 519 kt, the same as the conventional model on the same PT.
Before the overhaul the helicopters topped out at 34 to 45 kt.

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

### Helicopter power and structural speed (VTOL overhaul P2-fix, 2026-10-08)

All agent decisions. They apply to the AH64 and MI24 on the hybrid adapter,
which fly the single-rotor physics; the CH47 and the V22 have their own sections
below.

**Overspeed at Vne.** The overspeed rule uses each helicopter's never-exceed
speed, not the envelope's top speed above: AH64 197 kt (Wikipedia, AH-64
specifications), MI24 190 kt (fitted: no reliable published figure). Before
this, a dive was judged against 158 and 178 kt. Retreating blade stall starts
at the same speed, so the stall shows before the airframe is at risk.
Details in [overspeed](overspeed.md).

**MI24 rotor power.** The PT thrust (35,691 lbf) is 1.67 times the gross
weight, so deriving the rated power from it (4,798 hp) gave an aircraft that
hovered out of ground effect to 17,700 ft and climbed at 5,558 ft/min, against
a published hover ceiling out of ground effect of 4,915 ft. The Mi-24's rated
power is now the published 2 x 2,225 shp (TV3-117) times 0.745, or 3,315 hp at
the rotors and tail rotor; the PT mass, fuel and stores are kept. About 8
percent of the 0.745 is transmission, tail drive and installation loss; the
rest is what the published hover ceiling leaves, since that figure may be
quoted at a lower power rating or a warmer day. The whole share is fitted to
the 4,915 ft ceiling at the 24,250 lb normal takeoff weight. The forward
flat-plate drag area drops from 52 to 34 ft² so the lower power still reaches
the published level speed.

| Mi-24 at sea level, standard day | Before | After | Published |
| --- | --- | --- | --- |
| Rated power at the rotors, hp | 4,798 | 3,315 | 2 x 2,225 shp |
| Hover margin at the PT gross weight (21,385 lb) | 103 percent | 41 percent | |
| Hover ceiling out of ground effect at gross | 17,700 ft | 9,700 ft | |
| Same at 24,250 lb (normal takeoff weight) | 13,800 ft | 4,900 ft | 4,915 ft |
| Same at 26,455 lb (maximum takeoff weight) | 11,100 ft | 1,500 ft | |
| Best climb at 24,250 lb | 4,663 ft/min | 2,791 ft/min | 2,460 ft/min |
| Best climb at gross | 5,558 ft/min | 3,406 ft/min | |
| Level top speed at gross | 170 kt | 177 kt | 170 to 181 kt |
| Vertical climb at full collective, gross | 3,538 ft/min | 2,252 ft/min, rotor 89 percent | |

The hover ceiling is met. The climb is 13 percent above the published figure:
the excess-power estimate of the best climb is generous, and lower power would
take away the published hover. At the PT's own maximum takeoff weight of
28,660 lb the Mi-24 can no longer hover at sea level; the real maximum is 26,455
lb. Full collective in a hover at gross now droops the rotor to about 89
percent, which an overloaded helicopter does.

Sources (read 2026-10-08): the 2 x 2,225 shp, the 24,250 and 26,455 lb normal
and maximum takeoff weights, the 4,915 ft out-of-ground-effect and 7,210 ft
in-ground-effect hover ceilings, the 2,460 ft/min maximum climb and the 335
km/h level speed are from [Aerospaceweb, Mi-24
Hind](https://aerospaceweb.org/aircraft/helicopter-m/mi24) (the Mi-24D and V).
[Wikipedia, Mil Mi-24](https://en.wikipedia.org/wiki/Mil_Mi-24) gives 2 x
2,200 shp, 170 kt and a 3,000 ft/min climb; it gives no hover ceiling.
[Armed Conflicts](https://www.armedconflicts.com/-t42100) gives 2 x 2,225 hp, a
24,692 lb normal takeoff weight, a 2,461 ft/min climb and 335 km/h. None of
the sources says at which weight the hover ceiling is quoted; the 24,250 lb
normal weight is the assumption.

**AH64 power, checked and left alone.** The AH-64's rated power (3,668 hp,
from the PT thrust) is 97 percent of two -701C engines at 1,890 shp and 8
percent above two -701 at 1,696 shp. At the PT gross weight of 20,298 lb the
model hovers out of ground effect to 9,900 ft. At 17,650 lb, the AH-64A
maximum takeoff weight, it would reach 15,000 ft, against published figures
of 11,500 ft (AH-64A) and 9,810 ft (AH-64D) from [Aerospaceweb, AH-64
Apache](https://aerospaceweb.org/aircraft/helicopter-m/ah64/), neither with a
stated weight. The PT's empty weight (18,298 lb) is 61 percent above the
real 11,385 lb, so the published power-to-weight cannot be applied without
grounding the PT aircraft; the model's ceiling at the PT weights is in the
published range. Climb: 3,602 ft/min best, against 3,240 ft/min maximum and
2,500 ft/min vertical published. Level speed 151 kt against 158. Not
adjusted. The 10,200 ft at 17,650 lb quoted for the -701C could not be found
in a source.

### Tandem rotors: the CH-47 (VTOL overhaul P3, 2026-10-09)

All agent decisions; the numbers are fitted unless a line cites a source. The
CH47 flies two counter-rotating 60 ft rotors 38.9 ft apart on the overhaul's
rotor physics (the same rotor model as the AH-64 and Mi-24) and one
cross-shafted drive with one governor.

- **Controls.** Aft stick adds blade pitch to the front rotor and takes it
  from the rear (differential collective, 1.32 degrees at full stick since slice HF3, 1.2 before); lateral
  stick tilts both disks together; the pedals tilt them apart (front right,
  rear left, 0.85 of the lateral range at full pedal). The collective is
  common. There is no tail rotor.
- **Torque** cancels: the two rotors turn opposite ways, so the airframe
  feels only the difference between their powers. A 30 percent collective
  step with the pedals fixed yaws under 1 deg/s at Off and at Damper.
- **Rear rotor in the front rotor's wake.** The rear rotor's air gains 0.3
  times the front rotor's induced velocity along the front disk's normal,
  fading out by 40 kt, so the rear rotor needs more collective in a hover and
  the hover takes about 10 percent more power than without the wake.
- **Longitudinal trim.** The stability law's CH-47 schedule (none below 40 kt,
  all of it at 140 kt, Damper and Attitude only) tilts both disks forward by up
  to 2.5 degrees, so the fuselage flies about 2 degrees more level than at Off.
- **Power.** The PT thrust (135,795 lbf, 4.7 times the maximum takeoff weight)
  is implausible and set aside. The rated power is the CH-47F's two T55-GA-714A
  at 4,733 shp each ([Wikipedia, CH-47
  Chinook](https://en.wikipedia.org/wiki/Boeing_CH-47_Chinook), read
  2026-10-08), at its 54,000 lb maximum gross weight, scaled to the PT's
  28,660 lb maximum by the 3/2 power of the weight ratio (hover power follows
  the weight to the 3/2) and times 0.82 (about 8 percent transmission and
  installation loss, the rest fitted to the full-collective climb): 3,001 hp
  at the rotors. The design's first draft (1.25 times the maximum takeoff
  weight in thrust) corresponds to 3,270 hp, 9 percent more; the model's
  maximum static thrust is now what 3,001 hp hovers at, 1.17 times the PT
  maximum takeoff weight. The PT stores are kept. The PT's mass and fuel were
  the Mi-24's (53 percent of the real aircraft's); since slice HF3 the reader
  gives the CH-47F's (24,578 lb empty, 7,340 lb fuel, 54,000 lb maximum,
  [record correction](../formats/aircraft.md#rotorcraft-fuel-records)), so
  the weight ratio is 1 and the power the full 7,762 hp. The results below
  are the P3 figures at the old weights; the current ones are in
  [powered-lift flight](powered-lift-flight.md#fuel-slice-hf2).
- **Vne.** 190 kt, fitted: the published maximum speed is 170 kt but no Vne was
  found in a reliable source. The overspeed rule and the retreating blade
  stall onset both use it.
- **Results** against the PT (standard day, sea level, gross weight 21,385
  lb): hover margin 60 percent; hover ceiling out of ground effect 15,700 ft
  at gross and 4,600 ft at the 28,660 lb maximum (published service ceiling
  20,000 ft); level top speed 165 kt with the Damper's trim schedule
  (published 170 kt); least power 1,023 hp at 59 kt; vertical climb at full
  collective 2,925 ft/min with the rotor at 84 percent; engine cut in the hover,
  rotor below 80 percent after 2.4 s; autorotation 1,294 ft/min at 81 kt with
  the rotor at 96 to 102 percent; full-stick hover rates at Damper pitch 23,
  roll 41, yaw 42 deg/s (targets 25, 45, 45). Autorotation is slower than
  the single-rotor helicopters' 1,500 to 2,500 ft/min because the disk loading
  is a third of theirs.
- **Known differences.** Hands-off forward trim at the Attitude level settles
  near 130 kt (the single-rotor helicopters 60 to 100): the tandem's pitch is
  held by differential collective and its speed stability is weak. The
  retreating blade stall shows from about 155 kt at gross weight in a dive
  and pitches the rotors' thrust back, but gives no roll (the two rotors'
  roll tendencies oppose).

### V-22 tiltrotor (VTOL overhaul P5, 2026-10-08)

All agent decisions unless a figure says Pub. The V22 on the hybrid adapter no
longer flies the fitted law above; the rules are in
`crates/tore-sim/src/flight/powered/tiltrotor.rs`.

- **Rotors and drive.** Two 38 ft 1 in proprotors 46.5 ft apart (Pub,
  Derived) on nacelles, each the shared rotor model with the nacelle axis as
  its shaft, on one interconnected drive. Rated power is two AE 1107C at
  6,150 shp (Pub) less about 8 percent losses, times the record's maximum
  takeoff weight over the published 52,600 lb vertical maximum, so any record
  keeps the real aircraft's power loading. Since slice HF3 the reader gives
  the V-22 the MV-22B's weights (the PT's were the AH-64's,
  [record correction](../formats/aircraft.md#rotorcraft-fuel-records)), so the
  share is 1: 11,316 hp (5,122 hp before). Rotor speed 100 percent (397 rpm, Pub) with the nacelles
  up and 84 percent on the downstops (Pub), moving at 6 percent a second.
- **Nacelles.** 0 to 97.5 degrees at 8 degrees per second (Pub); the
  helicopter preset is 87. The conversion keys move the demand at the same
  rate; `0` asks for the downstops.
- **Conversion corridor.** Indicated airspeed limits against nacelle angle
  (design table: no minimum and 100 KCAS from 85 degrees up, 30 to 130 at
  80, 60 to 160 at 60, 75 to 180 at 45, 90 to 200 at 30, 100 to 200 at 15,
  110 to 280 on the downstops; mid-points and the 200 KCAS aft lock Pub, edges
  fitted). Protection is always on, at every stability level and with the
  Easy flight physics cheat on: past the upper edge the nacelles go forward
  at the full rate to 5 KCAS inside it; aft motion stops at the upper edge
  and above 200 KCAS; forward motion stops at the lower edge; both slow
  within 5 degrees of the edge, to a quarter of the rate. The nacelles are
  never raised for the pilot, and the pilot's demand is kept.
- **Controls.** Rotor terms scale with the sine of the nacelle angle: lateral
  stick is 2 degrees of differential collective, longitudinal stick 8 degrees
  of cyclic on both rotors, the pedals 4 degrees of differential cyclic. The
  wing's surfaces fly the pilot's command through the angle-of-attack wing
  (110 KCAS 1 G stall at the PT gross weight, Pub; 45 degrees per second of
  roll). The thrust control lever is the collective in the hover; from 75
  degrees down to 30 it becomes a power lever (full lever, full power at any
  airspeed), and the flight computers add the blade pitch that the airspeed
  through the disks needs. Midway they take pitch off when the rotor speed
  droops.
- **Download** 10 percent of the rotor thrust at 90 degrees, fading with the
  nacelle angle and with airspeed. **Drag** a 22.5 ft² forward flat plate
  (fitted to the published 275 kt) and the PT's gear, flap and G-pull terms.
- **Warnings.** Stall warning only below 35 degrees of nacelle, below the
  corridor's lower edge (110 KCAS on the downstops); GEAR SPEED with the gear
  down above 140 KCAS (Pub limit). Retreating blade stall only in edgewise
  flight (nacelles at 60 degrees and up).
- **Overspeed** at 280 KCAS, or the corridor maximum with the nacelles up
  ([overspeed](overspeed.md)).
- **Ground.** Nacelles below 60 degrees on the wheels below 10 kt of ground
  speed is a rotor strike (a crash), whatever the cheat; a rolling takeoff
  or landing with the nacelles low is not, until it slows. Dynamic rollover
  as on the helicopters.

Measured on the real PT (local probe, `examples/powered_probe.rs`): hover out
of ground effect at gross weight on 2,664 hp, 92 percent margin, lever 73
percent; hover ceiling 13,900 ft at gross, 8,900 ft at maximum weight; full
stick for 2 s from a hover at Damper 28 / 48 / 33 deg/s pitch, roll, yaw;
engine cut in the hover, rotor below 80 percent in 1.57 s; conversion with
the keys held at 85 percent lever from a 1,000 ft hover: on the downstops in
16 s, 200 KCAS in 27 s, height within 124 ft, never outside the corridor;
airplane-mode top speed 273 kt at sea level; slowest wingborne level flight
117 kt; stall warning at 110 KCAS.

## MiG-25 (M25-8, 2026-11-07)

The MiG-25 is the batch's eighteenth conventional aircraft. It flies the
hybrid adapter's own `variety` configuration, not a shared fighter's, and its
`M25.PT` (`MIG25.SH`) record supplies every value: empty 48,221 lb, internal
fuel 37,320 lb, maximum takeoff 90,830 lb, 38,800 lbf military and 49,382 lbf
afterburner thrust, G limits +5/-2 and a 68,000 ft ceiling. The shared
conventional flight, ground contact, actuator and loaded-mass rules apply
unchanged. No new fit was needed, and `fitted_envelopes` leaves the record
untouched.

The 1 G row's fast edge is 2,740 ft/s at 36,000 ft, 1,623 kt, against the
published 3,000 km/h (1,620 kt) at 13,000 m. The 0.2 percent difference is
inside the decoded envelope, so no `fitted_envelopes` entry is added: this is
the source's top speed, not a fit. The sea-level 1 G edge is 1,100 ft/s,
652 kt, and the shared loaded-drag rule makes full internal fuel settle at
90.1 percent of the edge at 1,000, 10,000, 20,000 and 30,000 ft and 90.0
percent at 36,000 ft, inside the fighters' 87 to 96 percent band. The G
limits are the source's +5/-2 rows, so the loaded 5 G row gives 3.86 G at the
default gross weight, through the shared loaded-elevator divisor (38 percent
loaded elevator, 0.863 loading).

The record's fuel flows are 2 lb/s at military power and 12 lb/s at full
afterburner. That is 6 and 2.8 times below the real R-15B-300's specific fuel
consumption (1.12 and 2.45 lb/(h lbf)), so the tank's endurance is far longer
than the real aircraft's. The M25-0 audit lists the flows for John; the tank
is left as retail, so the flow stays with the record rather than a fitted
value. Changing it would be a record correction, and the flight model only
consumes the record's flow.

The hybrid researched default is the validated path. The legacy
`--legacy-flight` adapter and the restricted `--native-flight-tables` path
stay separate and are not changed.

## F-16A (F16A-6, 2026-11-08)

The F-16A is the batch's nineteenth conventional aircraft. It flies the hybrid
adapter's own `variety` configuration, built from its own `F16.PT` (`F16.SH`)
record, not from the F-16C identity. `F16.PT` and `F16C.PT` are identical in
every flight field, so the F-16A's numbers equal the F-16C's: empty 14,567 lb,
internal fuel 6,972 lb, maximum takeoff 33,000 lb, 17,687 lbf military and
32,000 lbf afterburner thrust, 14 envelope rows from -4 to +9 G and a 60,000 ft
ceiling. The shared conventional flight, ground contact, actuator and
loaded-mass rules apply unchanged, and no new fit was needed.

The 1 G row's fast edge is 1,933 ft/s at 36,000 ft, 1,145 kt, against the
published 2,074 km/h (1,120 kt) at 36,000 ft (Aeropedia and Aero Web, in the
[F-16A published audit](../formats/aircraft-variety.md#published-audit)) and
inside the published 1,147 to 1,176 kt band at 40,000 ft the F-16C already
flies against. The 2 percent difference is inside the decoded envelope, so no
`fitted_envelopes` entry is added: this is the source's top speed, not a fit.
The sea-level 1 G edge is 794 kt, the same as the F-16C's and 0.1 percent under
the published 795 kt. The shared loaded-drag rule makes full internal fuel
settle at 93.5 percent of the edge at 1,000, 10,000, 20,000 and 40,000 ft and
92.1 percent at the 60,000 ft ceiling, inside the fighters' 87 to 96 percent
band. The G limits are the source's -4 to +9 rows, and the shared
loaded-elevator divisor reduces the usable limit with weight: at the default
gross weight the envelope limit at 1,000 ft is 7.55 G.

The only aircraft-to-aircraft difference on the ground is the default ordnance
load (the F-16A station record, [F16A-4](../formats/aircraft-ordnance.md)):
the F-16A carries 3,894 lb of stores (25,433 lb gross) against the F-16C's
4,854 lb (26,393 lb gross). Both are under the record's 33,000 lb maximum. With
an identical loadout the two identities fly identical ground probes, so the
difference is stores, not a flight field.

The hybrid researched default is the validated path. The legacy
`--legacy-flight` adapter and the restricted `--native-flight-tables` path
stay separate and are not changed.

## MiG-31 (M31-8, 2026-11-08)

The MiG-31 is the batch's twentieth conventional aircraft. It flies the hybrid
adapter's own `variety` configuration, built from its own `MIG31.PT`
(`MIG31.SH`) record. Its values: empty 48,115 lb, internal fuel 30,865 lb,
maximum takeoff 91,725 lb, 33,510 lbf military and 48,500 lbf afterburner
thrust, environment rows -2 to +4 G and a 68,000 ft ceiling. The shared
conventional flight, ground contact, actuator and loaded-mass rules apply
unchanged, and no new fit was needed.

The 1 G row's fast edge is 2,743 ft/s at 36,000 ft, 1,625 kt, against the
published 3,000 km/h (1,620 kt) at 17,500 m, 0.3 percent over. The sea-level
1 G edge is 1,370 ft/s, 812 kt, against the published 810 kt, 0.2 percent over.
Both are inside the decoded envelope, so `fitted_envelopes` leaves the record
untouched: this is the source's top speed, not a fit. The shared loaded-drag
rule makes full internal fuel settle at 0.915 to 0.916 of the edge at 1,000,
10,000, 20,000, 30,000 and 36,000 ft, inside the fighters' 87 to 96 percent
band. The G limits are the source's +4/-2 rows, so the loaded 4 G row gives
3.25 G at the default gross weight, through the shared loaded-elevator divisor
(36 percent loaded elevator, 0.735 loading). The positive limit is one G under
the real aircraft's published +5 supersonic limit; it stays retail.

The record's fuel flows are 6 lb/s at military power and 36 lb/s at full
afterburner. That is, as plain arithmetic, about 85.7 minutes of military power
or 14.3 minutes of afterburner from the internal tank, far longer than the real
D-30F6's specific fuel consumption allows. The record rule keeps the flow with
the tank, which is left as retail: this is a gap for John, not a correction.
The afterburner thrust is 29 percent under the real engines, and the aircraft
still reaches its decoded envelope: the full-afterburner climb tops 77,009 ft
before the tank runs dry, and the sprint reaches 842.3 kt at 5,000 ft, 0.940 of
the edge there.

The MiG-31 also carries a default ordnance load of four AA-9s and two AA-8s, so
its gross weight is above the clean record weight; the flight model reads the
loaded mass from the shared rules and no flight field changes with the stores.

The hybrid researched default is the validated path. The legacy
`--legacy-flight` adapter and the restricted `--native-flight-tables` path
stay separate and are not changed.

## Mirage 2000 (M2K-8, 2026-11-10)

The Mirage 2000 flies the hybrid adapter's own `variety` configuration, built
from `M2000.PT` (`M20.SH`). Its values: empty 17,100 lb, internal fuel 6,346 lb,
maximum takeoff 37,480 lb, environment rows -4 to +8 G and a 59,000 ft ceiling
as decoded, with the one-engine override of John's decision 2 (2026-11-09):
14,455 lbf military and 21,380 lbf afterburner thrust, one engine, applied where
the record is read ([aircraft record corrections](aircraft-records.md)). The
shared conventional flight, ground contact, actuator and loaded-mass rules apply
unchanged, and no new fit was needed.

The 1 G row's fast edge is 2,130 ft/s at 36,000 ft, 1,262 kt, against the
published Mach 2.2 (1,261 kn, Wikipedia, "Dassault Mirage 2000"); the sea-level
edge is 693 kt against a published 600 kn (most sources) to 729 kn
(GlobalSecurity). Both stay as decoded, so `fitted_envelopes` leaves the record
untouched. The shared loaded-drag rule settles full internal fuel at 0.948 of the
edge at every altitude (1,197 kt, Mach 2.09, at 36,000 ft). The `climb` maneuver
tops out at 64,422 ft, 1.092 of the 59,000 ft ceiling.

The hybrid drag law reaches the reference thrust at the top speed, so cutting
the thrust from the Rafale template's 32,000 lbf to the real M53-P2's 21,380
lbf leaves every top speed where the record puts it and lowers acceleration and
climb in proportion. At full internal fuel and afterburner the peak specific
excess power is 418 to 435 ft/s (25,100 to 26,100 ft/min), against 699 ft/s for
the Rafale record at the same weight, and the best sustained climb is 454 ft/s
(27,200 ft/min). Level acceleration at 1,000 ft from 363 kt takes 10 s to 460 kt
and 31 s to 590 kt; military power gives half the excess power (213 to 240 ft/s)
and settles at 551 kt at 1,000 ft and 984 kt (Mach 1.71) at 36,000 ft. The
published sea-level climb rate is 56,100 ft/min (285 m/s, test weight not
stated); the model gives about 45 percent of it, because of the drag law and the
absence of any ram thrust gain, not because of the thrust or the weight (see the
[validation](../baselines/variety-flight.md#mirage-2000-validation-2026-11-10)).
Raising the thrust to close the gap would take about 2.2 times the real thrust,
so it is reported, not changed.

The G limits are the record's -4 and +8 rows through the shared
loaded-elevator rule (40 percent): a full-stick pull peaks at 7.05 to 7.08 G on
a clean aircraft and 6.60 G with the default stores, the full-stick push at -3.5
G. The real aircraft is quoted at +9 and -3.2 G. The record's fuel flows (2 and
16 lb/s, 0.50 and 2.69 lb/lbf/h against the override's thrust) are kept as retail
with the tank: they give 52.9 minutes at military power and 6.6 minutes at
afterburner, against about 29 and 8.5 minutes at the M53-P2's published specific
fuel consumption for the same tank. The empty weight (17,100 lb, real 16,535 lb)
and the 6,346 lb tank (real about 6,970 lb) stay retail; at full fuel the clean
aircraft is within 60 lb of the real one.

The default load takes off at 130.3 kt after 1,026 ft and approaches at 145.2 kt
at 65 percent fuel (real approach 140 kt, Dassault), touches down at 132.6 kt
and stops; the record has no arrestor hook. The liftoff and approach ranges the
battery checks are fitted windows (agent decisions, 2026-11-10).

The hybrid researched default is the validated path. The legacy
`--legacy-flight` adapter and the restricted `--native-flight-tables` path
stay separate and are not changed.

## F-15E (F15E-8, 2026-11-10)

The F-15E flies the hybrid adapter's `variety` configuration built from
`F15E.PT`. Its flight words are the F-15C record's (John kept retail, decision 1,
2026-11-10): empty 28,600 lb, internal fuel 13,662 lb, maximum takeoff 68,000 lb,
two engines at 32,558 lbf military and 47,860 lbf afterburner, environment rows
-4 to +9 G and a 100,000 ft ceiling. The shared conventional flight, ground
contact, actuator and loaded-mass rules apply unchanged; no new fit was needed,
and the numbers equal the F-15C's on the same load.

The record's 1 G fast edge is 2,420 ft/s at 36,000 ft, 1,434 kt (Mach 2.5, the
Jane's text and the published figure); the sea-level edge is 806 kt (published:
about Mach 1.2, 800 kt CAS). Both stay as decoded. The shared loaded-drag rule
settles full internal fuel at 0.916 of the edge at every altitude (1,313 kt,
Mach 2.29, at 36,000 ft; 750 kt at 1,000 ft). The record's ceiling is 100,000 ft
and the model holds level flight to about 95,000 ft, against a published service
ceiling of 60,000 ft (Jane's) to 65,000 ft; the `climb` maneuver tops out at
108,921 ft. This is the retail copy, left as is.

At full internal fuel and afterburner the peak specific excess power is 588 to
594 ft/s (35,300 to 35,600 ft/min) clean and 436 to 453 ft/s with the default
load, and the best sustained climb is 36,000 ft/min against the published
50,000 ft/min or more: about 71 percent, because of the shared drag law (drag
equals the reference thrust at the top speed, no ram thrust gain), the same
cause as the Mirage but a smaller shortfall since the F-15E's thrust to weight
is 1.13 clean. Level acceleration at 1,000 ft from 363 kt takes 7 s to 460 kt
and 19 s to 590 kt; military power halves the excess power and settles at 619
kt at 1,000 ft and 1,083 kt (Mach 1.89) at 36,000 ft.

The G limits are the record's -4 and +9 rows through the shared loaded-elevator
rule (43 percent): a full-aft-stick pull at 461 kt and 1,000 ft peaks at 7.57 G
at full fuel, 8.28 at half, 8.69 at a quarter and 9.10 at 300 lb (clean); the
default stores give 6.73 to 7.91 G; the push is -2.5 to -3.0 G. A scripted
sustained turn at 461 kt and 1,000 ft reaches 5.74 G and 12.8 deg/s clean. The
record's fuel flows (4 and 24 lb/s, 0.442 and 1.805 lb/lbf/h) are kept: 56.9
minutes at military power and 9.5 minutes at afterburner, against about 35
minutes at the real F100-PW-220's 0.73 lb/lbf/h.

The default load (gross 52,202 lb) takes off at 149.1 kt after 1,155 ft and
approaches at 156.1 kt at 65 percent fuel; the clean aircraft lifts off at 127.3
kt after 673 ft and approaches at 140.4 kt (Jane's: take-off run 274 m,
approach 125 kt CAS, landing run 1,067 m, for the F-15C). The battery's liftoff
and approach windows are fitted (agent decisions, 2026-11-10; see the
[validation](../baselines/variety-flight.md#f-15e-validation-2026-11-10)).

The hybrid researched default is the validated path. The legacy
`--legacy-flight` adapter and the restricted `--native-flight-tables` path stay
separate and are not changed.

## Saab Gripen (GRI-8, 2026-11-10)

The Saab Gripen flies the hybrid adapter's own `variety` configuration, built
from `GRIPEN.PT` (`gri.SH`). Its values are the record's own, a real JAS 39A:
empty 14,599 lb, internal fuel 5,000 lb, maximum takeoff 27,560 lb, one engine at
12,140 lbf military and 18,100 lbf afterburner, environment rows -4 to +9 G and a
60,000 ft ceiling as decoded. No record correction applies, and John kept the
ceiling and the -4 limit (decision 10, 2026-11-10). The shared conventional
flight, ground contact, actuator and loaded-mass rules apply unchanged, and no new
fit was needed.

The 1 G row's fast edge is 1,145 kt at 36,000 ft (Mach 2.0), matching the
published Mach 2 (Wikipedia, "Saab JAS 39 Gripen"); the sea-level edge is 739 kt
at 1,000 ft (Mach 1.10), under the published Mach 1.15 to 1.2. Both stay as
decoded. The shared loaded-drag rule settles full internal fuel at 0.952 of the
edge (1,090 kt, Mach 1.90, at 36,000 ft; 704 kt at 1,000 ft) and level flight
holds to the 60,000 ft ceiling (933 kt); the `climb` maneuver zooms to 68,470 ft,
1.141 of the ceiling. The published ceiling is 50,000 ft.

At full internal fuel (19,599 lb, thrust to weight 0.92) and afterburner the peak
specific excess power is 402 to 445 ft/s (26,700 ft/min at sea level) and the
best sustained climb 427 to 432 ft/s (about 25,800 ft/min), with 36,000 ft in
107 to 123 s. Level acceleration at 1,000 ft from 363 kt takes 9 s to 460 kt and
27 s to 590 kt; military power gives half the excess power (about 225 ft/s). The
shape matches the Mirage 2000's (418 to 435 ft/s at 0.91), because both use the
shared drag law, in which drag equals the reference thrust at the top speed and
grows with the square of speed and no ram thrust gain is modelled. No published
sea-level climb rate exists for the JAS 39C/D, so no ratio to a published figure
is claimed; see the
[validation](../baselines/variety-flight.md#saab-gripen-validation-2026-11-10).

A full-stick pull at 461 kt and 1,000 ft peaks at 8.00 G with full fuel (7.26 G
with the default load) and 9.03 G with 300 lb of fuel, against the F-16A's
7.67 to 9.09 G and the Mirage 2000's 7.96 to 9.09 G for the same four fuel states
(the loaded-elevator rule scales the limit with the fuel and stores carried). The
full-stick push is -3.55 G clean. The sustained level turn at 461 kt and 1,000 ft
is 4.8 G and 10.4 deg/s from the scripted pilot (a lower bound), two thirds of
the F-16A record's. The real limits are +9 and -3 G. The record's fuel flows (2
and 14 lb/s) are kept: the 5,000 lb tank lasts 41.7 minutes at military power and
6.0 minutes at afterburner, against about 29 and 9.3 minutes at the RM12's
published specific fuel consumption.

The default load (23,809 lb) takes off at 132.1 kt after 1,097 ft (334 m; Saab
quotes 400 m) and approaches at 147.5 kt, touching down at 142.0 kt at 65
percent fuel and stopping 589 m later (Saab quotes 500 to 600 m); the record has no
arrestor hook, which is right for the real aircraft. The liftoff and approach
ranges the battery checks are fitted windows (agent decisions, 2026-11-10).

## Northrop F-5E (F5E-8, 2026-11-10)

The F-5E Tiger flies the hybrid adapter's own `variety` configuration, built from
`F5E.PT` (`f5e.SH`). Its values are the retail record's with John's B5-2
corrections ([aircraft record corrections](aircraft-records.md)): empty 9,558 lb,
internal fuel 4,400 lb (retail 6,240 lb), maximum takeoff 24,664 lb, two engines
at 7,000 lbf military and 10,000 lbf afterburner in all, environment rows -4 to +7
G (the retail +8 row, a repeat of the -4 row, is retired) and a 51,800 ft ceiling
(retail 42,000 ft). The shared conventional flight, ground contact, actuator and
loaded-mass rules apply unchanged, and no new fit was needed.

The 1 G row's fast edge is 1,090 kt at 36,000 ft (Mach 1.90), against the
published Mach 1.63 (939 kn, Wikipedia, "Northrop F-5"); the sea-level edge is 731
kt at 1,000 ft (no published sea-level figure was found). Both stay as decoded. The
shared loaded-drag rule settles full internal fuel at 0.937 of the edge (1,021 kt,
Mach 1.78, at 36,000 ft; 685 kt at 1,000 ft) and level flight holds to the 51,800
ft ceiling (535 kt); the `climb` maneuver zooms to 52,214 ft, 1.008 of it. The
polygon's fast edge falls about 29 kt per 1,000 ft above 39,000 ft (the B5-2
correction moved only the top edge), so the speed at 50,000 ft is 586 kt.

At full internal fuel (13,958 lb, thrust to weight 0.72) and afterburner the peak
specific excess power is 291 to 332 ft/s (19,900 ft/min at sea level) and the best
sustained climb 314 to 316 ft/s (about 18,900 ft/min), with 36,000 ft in 138 to
159 s. Level acceleration at 1,000 ft from 363 kt takes 12 s to 460 kt and 37 s to
590 kt; military power gives about 55 percent of the excess power (170 to 185
ft/s). The published sea-level climb rate is 34,500 ft/min (575 ft/s, test weight
not stated); the model gives 55 to 58 percent of it, because of the drag law and
the absence of any ram thrust gain, not because of the thrust or the weights (see
the [validation](../baselines/variety-flight.md#northrop-f-5e-validation-2026-11-10)).
It is reported, not changed.

A full-stick pull at 461 kt and 1,000 ft peaks at 5.98 G with full fuel, 6.49,
6.78 and 7.00 G with half, a quarter and 300 lb of fuel, the shared
loaded-elevator rule scaling the limit with the fuel carried (the Gripen 8.00 to
9.03 G, the F-16A 7.67 to 9.09 G, the Mirage 2000 7.96 to 9.09 G for the same four
states). The top row is +7: the pull stops at 7.00 G, never passes it and there is
no +8 row; the real limit is +7.33 G. The full-stick push is -2.55 G (full) to -2.99
G (near empty), because the -4 row starts at 570 kt at sea level. The record's G rows
fall off quickly with speed and altitude (+3 G reaches 622 kt at sea level and ends
at 28,875 ft), so a pull at 593 kt and 1,000 ft gives 2.6 G and the sustained level
turn is 4.13 G (9.6 deg/s) at 461 kt, 2.5 G at 593 kt and about 1.6 G at 30,000 ft
and above, from a scripted pilot (a lower bound). The record's fuel flows (1 and 8
lb/s) are kept: the 4,400 lb tank lasts 73.3 minutes at military power and 9.2 at
afterburner, against about 30 and 12.4 minutes at the J85-GE-21's published
specific fuel consumption.

The default load (15,365 lb) takes off at 153.4 kt after 1,763 ft (537 m; the real
ground run is about 2,000 ft at 15,745 lb) and approaches at 166.2 kt (1.3 times the
published 124 kt landing-configuration stall speed is 161 kt), touching down at
159.8 kt at 65 percent fuel and stopping 703 m later; the record has no arrestor
hook. The liftoff and approach ranges the battery checks are fitted windows (agent
decisions, 2026-11-10).

## Dassault Mirage F1 (MF1-8, 2026-11-11)

The Mirage F1 flies the hybrid adapter's own `variety` configuration, built from
`MF1.PT` (`mf1.SH`). Its values are the retail record's with John's B5-2 thrust
correction ([aircraft record corrections](aircraft-records.md)): one Atar 9K-50 at
11,020 lbf military and 15,873 lbf afterburner (retail 15,000 and 30,000). Empty
weight 15,542 lb, internal fuel 6,120 lb and maximum takeoff 30,203 lb stay the
retail (Mirage III) values by John's decision, and so do the record's own
environment rows -3 to +7 G, the 66,125 ft ceiling and the 1,345 kt top row. The
shared conventional flight, ground contact, actuator and loaded-mass rules apply
unchanged, and no new fit was needed.

The 1 G row's fast edge is 1,345 kt at 36,000 ft (Mach 2.35) and 806 kt at 1,000 ft.
The shared loaded-drag rule settles full internal fuel at 0.945 of the edge: 1,271
kt (Mach 2.22) at 36,000 ft and 762 kt (Mach 1.16) at 1,000 ft, against the game
encyclopedia's Mach 2.2 (data line 1,266 kt) and about 800 kt at low altitude.
Level flight holds to the 66,125 ft ceiling (published 65,600 ft); the scripted
climb reaches 63,914 ft before the tank runs dry.

At full internal fuel (21,662 lb clean, thrust to weight 0.73) and afterburner the
peak specific excess power is 338 to 376 ft/s (22,600 ft/min at sea level) and the
best sustained climb 360 to 375 ft/s (21,600 to 22,500 ft/min), with 36,000 ft in
121 to 135 s. Level acceleration at 1,000 ft from 363 kt takes 11 s to 460 kt and 29
s to 590 kt; military power gives about 57 percent of the excess power (194 to 215
ft/s). The published sea-level climb rate is 41,930 ft/min (the encyclopedia) or
47,800 ft/min (MIGFlug, Wikipedia); the model gives 54 and 47 percent of them
because of the drag law and the absence of any ram thrust gain, not because of the
thrust (the real 15,873 lbf) or the weights. It is reported, not changed.

A full-stick pull at 461 kt and 1,000 ft peaks at 6.14 G with full fuel and 6.59,
6.84 and 7.06 G with half, a quarter and 300 lb of fuel, the shared loaded-elevator
rule scaling the limit with the fuel carried (the Mirage 2000 7.96 to 9.09 G, the
Gripen 8.00 to 9.03 G, the F-5E 5.98 to 7.00 G and the F-16A 7.67 to 9.09 G for the
same four states). The pull passes the +7 top row by 0.06 G when nearly dry and
the real limit is +7.5 G; the full-stick push is -2.61 G (full) to -3.00 G (near
empty), against the real -3.6 G. The G rows hold at speed (6.1 G from 330 kt to Mach
1.1 at 1,000 ft). The sustained level turn is 3.63 G (8.1 deg/s) at 461 kt and
1,000 ft, 2.68 G at 593 kt and about 3.2 G at 30,000 and 36,000 ft, from a
scripted pilot (a lower bound); it fell from 5.76 G to 3.63 G with the real thrust.
The record's fuel flows (3 and 23 lb/s) are kept: the dry flow matches the real
engine's published specific fuel consumption (0.98 lb/lbf/h), but the 4.4 minutes
the 6,120 lb tank lasts at afterburner is 2.6 times the real consumption (about
11.8 minutes), because 23 lb/s was authored for the retail 30,000 lbf.

**The default load changes the weight-dependent numbers.** The record's default
stores (the DEFA gun, four belly 250 gallon tanks, two `R550.JT`, centreline empty)
weigh 8,177 lb, a gross weight of 29,839 lb (99 percent of the maximum). With them
the 461 kt pull is 5.18 G and the sustained turn 2.93 G, the afterburner speed at
36,000 ft is 1,190 kt (Mach 2.07) and at 1,000 ft 714 kt, the peak excess power
252 ft/s, and the takeoff is 135.4 kt after 1,655 ft (504 m; clean 103.9 kt after
691 ft, 211 m; the published run is 600 m). Approach is 145.6 kt (clean 115.9 kt;
published about 125 kt), touching down at 135.9 kt at 65 percent fuel and stopping
584 m later (clean 376 m; published landing run 670 m). The record has no arrestor
hook. These default-load numbers depend on John's open stations decision
(MF1-5/MF1-6); the clean ones do not. The liftoff and approach ranges the battery
checks (125 to 150 kt) are fitted windows (agent decisions, 2026-11-11).

Update (MF1-5b, 2026-11-11): the stations decision is made: the belly row holds two
`F250.GAS`, so the default load weighs 4,481 lb carried, a gross weight of 26,143 lb
(87 percent of the maximum). The scripted liftoff is now 121.0 kt after 1,144 ft
(349 m) and the approach 127.6 kt, touching down at 118.7 kt and stopping in about
447 m; the 125 to 150 kt bands stay and the recorded values moved to 121.0 and 127.6
(agent decision). The pull, turn and top-speed figures above were measured with the
heavier load and were not re-run.

## Panavia Tornado IDS (TOR-8, 2026-11-11)

The Tornado IDS flies the hybrid adapter's own `variety` configuration, built from
`TOR.PT` (`tor.SH`). Its values are the retail record's with John's B5-2 and B5-3
corrections ([aircraft record corrections](aircraft-records.md)): empty 31,065 lb,
maximum takeoff 61,620 lb, two engines at 17,300 lbf military and 32,150 lbf
afterburner in all, environment rows -3 to +7 G (the +7 row fitted from the +6 row,
real +7.5), fuel flows 3 and 22 lb/s; the 11,221 lb tank and the 60,000 ft ceiling
are the record's own. The shared conventional flight, ground contact, actuator and
loaded-mass rules apply unchanged, and no new fit was needed. **No flight law reads
the wing sweep**: TOR-3 and TOR-5 draw it (spread to 304 kt, 350 mph, since TOR-11 on 2026-11-12, John; it was 250 kt; 67 degrees at 600 kt)
and move the swivelling pylons by it, and that is all it does. The real sweep changes
lift and drag a great deal; TORE has one envelope, drag law and stall speed per
aircraft, and because the drawn sweep is a function of speed only, the Tornado's own
envelope rows already hold the speed-dependent result.

Clean (31,065 lb plus 11,221 lb of fuel, 42,286 lb) the afterburner level speed is
793 kt (Mach 1.20) at 1,000 ft and 1,321 kt (Mach 2.30) at 36,000 ft, 0.965 of the
1 G edge, against the published about Mach 1.2 at sea level and Mach 2.2 at altitude;
level flight holds to the record's 60,000 ft (the published service ceiling is
50,000 ft). Thrust to weight is 0.76 clean and 0.56 at the default gross of 56,982
lb; the peak specific excess power is 406 ft/s at sea level clean (285 ft/s with the
default load) and the afterburner climb from a 450 kt start reaches 30,000 ft in 91
to 101 s clean (120 to 134 s loaded), against the published "less than two minutes"
from brake release. A full-stick pull at 461 kt and 1,000 ft peaks at 6.17 G with
full fuel, 6.59, 6.82 and 7.04 G with half, a quarter and 300 lb of fuel (5.29 to
5.91 G with the default load), the shared loaded-elevator rule scaling the limit with
the fuel carried (the F-15E 7.57 to 9.10 G, the F-5E 5.98 to 7.00 G, the Gripen 8.00
to 9.03 G and the Mirage F1 6.14 to 7.06 G for the same four states). The real limit
is +7.5 G, 0.5 G above the whole-G top row. The sustained level turn is 4.82 G
(10.2 deg/s) at 461 kt and 1,000 ft, from a scripted pilot (a lower bound). The
record's fuel flows give 62 minutes at military power and 8.5 at afterburner from
the internal tank.

The default load (14,696 lb, which depends on the open station question) takes off
at 150.6 kt after 1,945 ft (593 m; the published run is 900 m at a typical strike
weight) and approaches at 160.1 kt (the 1.05 times loaded minimum floor), stopping
757 m after touchdown; the published landing run is 370 m with thrust reversers,
which TORE does not model (no field, panel or flight rule). The liftoff and
approach ranges the battery checks are fitted windows (agent decisions,
2026-11-11).

The hybrid researched default is the validated path. The legacy
`--legacy-flight` adapter and the restricted `--native-flight-tables` path
stay separate and are not changed.

Update (TOR-10, 2026-11-11): the +8 row and the 50,000 ft ceiling of B5-4 and the
lighter default load of TOR-5b and TOR-6b change the figures above; the re-measured
numbers are in the [Tornado acceptance record](../baselines/aircraft-tornado.md#flight-envelope).
The record's 60,000 ft ceiling and the level flight to 837 kt there no longer hold.

## Boeing B-52H (B52-8, 2026-11-15)

The B-52H flies the `variety` configuration built from `B52.PT` with the BMB-A2 values
(136,000 lbf, empty 185,000 lb, tank 312,197 lb, 20 lb/s, 488,000 lb maximum takeoff) and
the record's own envelopes and G rows. Six fitted terms are the aircraft's own; every
other aircraft keeps the shared value ([review and table](../formats/aircraft-variety.md#b-52-flight-review-b52-8-2026-11-15)):

- **Clean-drag shape.** The shared drag is the full thrust at the 1 G top-speed edge
  times the square of speed over the edge. The B-52's low-altitude edge is a placard
  (385 kt at sea level), so that law gave about three times the real drag at 300 kt.
  Its clean drag is instead 0.15 of that on the square law plus 0.85 on the twelfth power
  of speed over the edge: 1 at the edge (the top speed does not move), 0.13 of the thrust
  at 0.78 of the edge, 0.24 at 0.857 of it. Mach 0.77 at 36,000 ft burns 4.7 lb/s
  (about 8,000 nautical miles from the tank, published combat range 7,600) and the climb
  from sea level at 300 KCAS reaches 5,000 ft in 73 to 82 s (published 6,270 ft/min).
- **Thrust lapse** 32,000 ft (shared 70,000). **Rolling deceleration** 4.8 ft/s squared
  (shared 0.8): the 488,000 lb takeoff run is 7,300 to 7,700 ft, lifting off at 158.6 kt.
  **Brake deceleration** 2.6 (shared 18): a 270,000 lb landing rolls 4,100 ft.
  **Stall reference fraction** 1.09 (shared 1): approach 140 kt at 270,000 lb.
- **Nose alignment** 0.4, the transports'.

Not changed: the top-speed edges (515 kt at 36,000 ft against a published 560 to 565),
the 55,000 ft ceiling (published 50,000), the G rows and the roll limit. Engines stay
clamped to four in the live configuration (see the review).

## Rockwell B-1B Lancer (B1B-8, 2026-11-16)

The B-1B flies the `variety` configuration built from `B1.PT` with the BMB-B2 values (69,560 lbf dry, 123,120 lbf
with afterburner, empty 192,000 lb, tank 265,274 lb, 477,000 lb maximum takeoff) and the record's own G rows and
flight tuning. Four terms are the aircraft's own, all **fitted** or **opinionated** (agent decisions, 2026-11-16);
every other aircraft keeps the shared value ([review and table](../formats/aircraft-variety.md#b-1b-lancer-flight-review-b1b-8-2026-11-16)):

- **Fuel flows** 11 lb/s dry and 84 lb/s with afterburner (were 6 and 16): four F101-GE-102 at 0.562 and 2.46 lb/(lbf h).
  A full tank lasts 402 minutes at full military power and 52.6 minutes on full afterburner.
- **Sea-level top speed** Mach 0.92, 608 kt with afterburner (was Mach 1.00, 663 kt), the published low-level speed. The
  1 G edge rises in a straight line to Mach 1.25 (716 kt) at 36,000 ft, so Mach 1.00 comes at 10,000 ft.
- **Clean-drag shape**, the B-52's hook: 0.72 of the reference drag on the square law and 0.28 on the twelfth power of
  speed over the edge. 1 at the edge, so no top speed moves; Mach 0.8 at 36,000 ft holds on 0.76 throttle and burns 7.6
  lb/s (about 5,000 nautical miles from the tank, published 5,100). No other term of the B-52's hook is taken.
- **Speed brake** a fifth of the fighter airbrake (the two spoiler plates the rig opens): 0.08 to 0.17 G of extra deceleration
  at 306 to 550 kt at 1,000 ft (was 0.34 to 0.74 G).

Not changed: the 55,000 ft ceiling (published 60,000), the G rows (-2 to +5), the roll limit (60 degrees a second), the
yaw authority (the rudder cap of 0.45 radian is a drawing limit), the takeoff and landing laws. The wing sweep (spread to
280 kt, fully swept from 520 kt) is drawn only: no flight law reads it, and sweep adds no drag, lift or stall term.

## Tupolev Tu-160 (T160-8, 2026-11-16)

The Tu-160 flies the `variety` configuration built from `TU160.PT` with the BMB-B2 values (123,600 /
220,400 lbf, empty 242,500 lb, tank 326,285 lb, 606,260 lb maximum takeoff) and the record's own G rows. The
top-speed edge and the fuel flows are corrected in the record, and five fitted terms are the aircraft's own;
every other aircraft keeps the shared value ([review and table](../formats/aircraft-variety.md#tu-160-flight-review-t160-8-2026-11-16)):

- **Top speed.** The 1 G edge is 556 kt at sea level (the published 640 mph) and rises on a constant
  calibrated airspeed (556 kt, 595 kt at 40,000 ft) to 1,200 kt (2,025 ft/s) at 40,000 ft; above that the
  record's edge stays. The record's 1,203 kt at sea level and 1,270 kt at 36,000 ft are gone. Level flight in
  afterburner settles at 94 percent of the edge: 524 kt at sea level, 1,020 kt at 36,000 ft, 1,130 kt at
  40,000 ft. Above the edge the overspeed rule applies as for every aircraft.
- **Fuel flow.** 25 lb/s at military power and 104 lb/s with afterburner (the NK-32's 0.72 to 0.73 and 1.70
  specific consumption): 218 minutes dry, 52 minutes with afterburner from the 326,285 lb tank.
- **Clean-drag shape**, the B-52's form with 0.40 of the reference drag on the square law and the rest on the
  12th power: 0.27 of the afterburner thrust at 430 kt at sea level (the climb), 0.07 at Mach 0.77 at 36,000
  ft (the range). The peak climb at sea level is 14,600 ft/min at 450,000 lb (published 13,780) and 10,800
  ft/min at the full tank; Mach 0.9 from the full tank covers 7,100 to 7,400 nautical miles (published 6,640).
- **Stall reference fraction** 2.3 (shared 1): the weight-scaled stall speed carries 558,000 lb on the record's
  speeds. At the 600,532 lb default load the flapped stall is 113 kt, rotation 146 kt and liftoff 168 kt after
  4,072 ft (it was 252 kt after 8,721 ft); the scripted approach flies 143.6 kt at 486,000 lb and 110.8 kt at
  the 341,710 lb landing weight (it crashed at 207 kt before). **Brake deceleration** 4.5 ft/s squared
  (shared 18): the landing roll from touchdown is 4,100 ft at 486,000 lb and 2,813 ft at the landing weight.
  **Nose alignment** 0.4, the transports'.

Not changed: the 50,000 ft ceiling (published 49,200 to 52,495), the G rows (a 2.1 G full-stick pull at a
typical load, 1.8 G at the full tank; Jane's "g limit: 2"), the thrust lapse (70,000 ft), the rolling
deceleration (the takeoff run is 1,241 m against a published 2,200 m of unknown weight), the sweep schedule
(35 to 48 degrees at the 524 to 556 kt sea-level top speed) and the roll limit. The AI heavy-class flaps law
no longer applies to it (flaps-up minimum 151 kt at the takeoff weight, under 190.5 kt).

## Unknown evidence

Retail hover power, altitude ceiling, nozzle travel/rate, rotor torque, lift
engine scheduling, detailed rotor physics and transition losses remain unknown.
Next research is a bounded review of the original force consumers or measured
retail behavior when available. Those gaps are filled by the explicit fits
above and do not claim original gameplay parity.
