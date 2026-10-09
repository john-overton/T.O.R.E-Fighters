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

The overhaul's second project, AI wingmen, will restore them: the AI will fly
these aircraft through their inputs on the same physics a player uses (VTOL
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
  from the rear (differential collective, 1.2 degrees at full stick); lateral
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
  maximum takeoff weight. The PT mass, fuel and
  stores are kept (the PT weights are 53 percent of the real aircraft's).
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
  6,150 shp (Pub) times 0.42, about 8 percent losses times the PT's maximum
  takeoff weight over the published 52,600 lb vertical maximum, so the
  aircraft, about 40 percent of the real one's weight, keeps its power
  loading: 5,122 hp. Rotor speed 100 percent (397 rpm, Pub) with the nacelles
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

## Unknown evidence

Retail hover power, altitude ceiling, nozzle travel/rate, rotor torque, lift
engine scheduling, detailed rotor physics and transition losses remain unknown.
Next research is a bounded review of the original force consumers or measured
retail behavior when available. Those gaps are filled by the explicit fits
above and do not claim original gameplay parity.
