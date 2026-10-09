# Straight-flight, waypoint and hover-hold autopilot

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Player behavior

John requested the USNF-ATF modes on 2026-09-17, including waypoint plumbing
before routes exist. A toggles heading/altitude hold. Ctrl-A toggles waypoint
hold. Pressing the active mode again turns it off; switching modes preserves
the original heading and altitude capture. Re-engaging from off captures anew.
Waypoint mode continuously follows the selected horizontal target, retaining
captured altitude. With no target, or within 1 metre of it, it holds the captured
heading. No route selection, automatic sequencing or waypoint altitude is added.

Pitch, roll or rudder input strictly above 0.15 in magnitude disengages on the
same simulation tick. Throttle and equipment controls remain manual (on the
helicopters and the V-22 the collective is the autopilot's, below). Autopilot
disengages on ground contact or crash; new flights start off. It uses ordinary
control inputs at 120 Hz, without changing aircraft attitude or position directly.
The HUD shows two lines at the upper left beside the heading tape, using the
existing HUD font and color: `AUTO` above `HDG ALT` or `WP <number>`.
The number is the selected mission waypoint's number. With no target, the second
line reads `WP --`. No label appears when off. John requested this layout and
numbered waypoint label on 2026-09-17; the missing-target placeholder is an agent
choice.

## Hover hold (helicopters and the V-22)

John asked on 2026-10-08 for a hover hold on the helicopters and the V-22, as
an autopilot mode that any stick, pedal or collective input cancels, and not
on the AV-8 or Yak-141 (VTOL overhaul decision 2). Retail never let players fly
rotorcraft, so everything here is **opinionated** (requested) or **fitted**.

- **Key.** Ctrl+Alt+A (`hover-hold`) toggles it on every aircraft. It
  replaces an engaged A or Ctrl+A mode, and starts a fresh capture.
- **Engages** on the AH-64, Mi-24, CH-47 and V-22 (nacelles at 75 degrees or
  more), airborne, below 40 kt ground speed, with engine power and hydraulics.
  Otherwise it is refused with a message: `Hover hold is not available on
  this aircraft` (the jets, fixed-wing aircraft, any aircraft on the legacy
  adapter), `Hover hold needs the nacelles at 75 degrees or more`, `Hover
  hold is not available on the ground`, `Hover hold needs less than 40
  knots`, `Hover hold needs engine power`, `Hover hold needs hydraulic
  power`. Damage that makes the autopilot unavailable refuses it with the
  existing `Autopilot unavailable due to damage`.
- **Announced** `Hover hold engaged`; the HUD shows `AUTO` above `HOVER` in
  the autopilot label slot. Every way it ends says `Hover hold off`.
- **What it holds.** It first brakes the drift; once the ground speed is below
  1 kt it takes that ground point and holds it. It holds the heading at
  engagement, and the wheels' height above the ground at engagement, at
  least 10 ft (an engagement lower climbs to 10 ft). It commands at most 15
  degrees of pitch or bank and 500 ft/min of climb or sink. A wind or gust is
  flown out by leaning into it, as a pilot would.
- **Through the controls only.** It writes the cyclic, pedals and collective
  the flight model receives that tick, on the same physics, at every
  stability level and with Easy flight physics. It never moves the aircraft
  directly: replaying the same inputs by hand flies the same flight, bit for
  bit. Out of power it cannot hold the height, and the aircraft sinks.
- **Cancels**, on the tick it happens, with `Hover hold off`: stick or pedal
  past 0.15 (the rule above); any collective input (a collective or throttle
  key or step, the collective keys held, a collective or throttle lever moved
  more than 2 percent from where the hold first saw it); ground contact; a
  crash; an engine failure, shutdown or fuel exhaustion; the loss of the
  hydraulics; Ctrl+Alt+A again.
- **Trim keys do not cancel it.** Ctrl+Up / Ctrl+Down / Ctrl+Left /
  Ctrl+Right move the held point 10 ft forward, back, left or right of the
  heading per tap (a held key moves it about 50 ft a second) and leave the
  trim alone. Pedal trim and trim centre still move the trim; Trim set is
  ignored while it holds, since its latch would hide the autopilot's stick.
  There is no height nudge: the collective keys cancel.

## The A modes on the powered-lift aircraft

The heading and waypoint modes fly through each aircraft's own controls
(VTOL overhaul slice P9, agent decisions of 2026-10-09):

- **AV-8 and Yak-141** fly the fixed-wing law above. They need flying speed:
  below their 1 G stall speed A and Ctrl+A are refused (`Autopilot needs
  flying speed`), and an engaged mode lets go below 85 percent of it
  (`Autopilot off: too slow`).
- **Helicopters and the V-22** fly attitude loops through the cyclic, since
  their stick tilts a rotor disk rather than commanding a load factor. They
  need 40 kt ground speed, where hover hold stops: below it A and Ctrl+A are
  refused (`Autopilot needs 40 knots; Ctrl+Alt+A holds a hover`), and an
  engaged mode lets go below 30 kt (`Autopilot off: too slow`). The heading
  and waypoint guidance is the fixed-wing law's (heading error times 1.5,
  within 30 degrees of bank). The pedals keep the turn coordinated against
  the sideslip through the air at every stability level.
- On the **helicopters, and the V-22 with its nacelles at 75 degrees or
  more**, the altitude is held on the collective (10 s time constant, within
  1,000 ft/min) and the speed on the cyclic: the ground speed along the
  heading at engagement, at least 40 kt. Ctrl+Up / Ctrl+Down change the held
  speed by 2 kt a tap. Because the mode flies the collective, the same inputs
  that cancel hover hold (collective, engine, hydraulics) let it go too.
  Trim set is ignored while any mode flies a helicopter or the V-22.
- On the **V-22 with its nacelles below 75 degrees** the altitude is held on
  the pitch, as on a fixed-wing aircraft, within 15 degrees, and the power
  lever stays the pilot's, as the throttle does elsewhere.

## Numbers and provenance

The interaction and 30 degree commanded-bank / 20 m/s commanded-climb limits
are spec-derived from the reference checkout's prose. These are command limits,
not hard clamps on aircraft motion. Retail equivalence remains unknown.
The 1 metre arrival fallback is fitted. HUD line positions (211,133) and
(211,145) at 640×480 are fitted to the requested screenshot layout.

Controller tuning is fitted for this host: heading error requests bank at a
factor of 1.5; bank error requests roll rate with a 1 second time constant,
normalized by aircraft roll authority. Altitude error requests vertical speed
with a 10 second time constant, limited to 20 m/s. Vertical-speed error requests
vertical acceleration with a 3 second time constant. Bank compensation converts
that acceleration to normal load, using a minimum bank cosine of 0.5, then the
aircraft's available load envelope converts it to stick deflection. No autothrottle,
terrain avoidance, stall recovery or guaranteed hold outside the flight envelope
is implied. Mode state and target live in the renderer-independent simulation;
input tapes retain mode commands. Future navigation sets an optional world X/Z
target in feet with its waypoint number through `Autopilot::set_navigation_target`; invalid coordinates
clear the target. Target selection will need recording when navigation is added.

Hover hold and the powered-lift A modes (fitted, agent decisions of
2026-10-09): the attitude loops command a rate of 3 per second of attitude
error, at most half the aircraft's full-stick hover rate, as stick normalized
by that rate, with an integrator within 0.15 rad of error; the Attitude
level's and the Easy flight physics cheat's attitude retention is added back
so the loops fly alike at every level. Hover hold commands a ground velocity
of 0.15 per second per foot from the point (at most 10 ft/s), and a tilt of
0.8 per second of velocity error over g, with an integrator that only runs
while the tilt is inside its limit; a climb of 0.25 per second of height
error, flown as 0.08 collective per ft/s of climb error with an integrator; a
yaw rate of 2 per second of heading error. The 40 kt boundary, the 75-degree
nacelle condition, the 10 ft floor, the 15-degree and 500 ft/min command
limits and the cancel rules are the design's (VTOL overhaul section 5.4);
the 30 kt release, the 85 percent jet release, the 2 kt speed nudge and the
1,000 ft/min rotorcraft climb limit are agent choices. Retail had no
equivalent.

## Host acceptance

With the synthetic F/A-18D fixture at 10,000 ft, 450 knots and 70% throttle,
engage then disturb bank by 20 degrees and altitude by minus 100 ft. After
120 seconds, heading error must be below 3 degrees, altitude error below 100 ft,
and bank below 5 degrees, without a stall or crash. Apply this to heading hold
and a target at X/Z (100,000, 100,000) ft, in hybrid and legacy modes. These are
fitted host acceptance tolerances, not recovered retail performance numbers.

Hover hold (design tests A1 to A4), on synthetic aircraft carrying the AH-64,
Mi-24, CH-47 and V-22 PT numbers: engaged at 30 kt in a 15 kt wind it brakes
the drift below 1 kt within 20 s, then holds the point within 20 ft, the
height within 10 ft and the heading within 5 degrees for 60 s, at every
stability level, with and without Easy flight physics, in calm air, a steady
wind from four sides and a gusting one. Each refusal and cancel above acts on
its tick with its message, and the trim keys do not cancel. The inputs it
flew, replayed by hand, give a bit-identical flight, and a hold restored
mid-flight from the exact state or a checkpoint flies on bit for bit for
1,200 ticks. The A modes on all six powered-lift aircraft, from their
airborne starts, meet the fixed-wing gates above (heading within 3 degrees
after 120 s) with altitude within 30 ft (heading mode, after 30 s) and 50 ft
(waypoint turn), at every stability level, with and without the cheat.
[Validation results](../baselines/autopilot.md) record the executed checks.

## Evidence and unknowns

Reference identity: USNF-ATF commit
`2d818054ff51db9f3353d0548dbd0e469b275a1a`,
`Docs/progress.md`, section “2026-09-09: compass direction, cloud silhouettes,
sun highlight and autopilot”, autopilot description and validation results.
That baseline reports a 30 second recovery to 178 degrees / 2866 m from an
upset at 180 degrees / 2914 m and six closed-loop hold tests. This is evidence
for the reference rebuild, not a retail measurement. Its engine is not reused.
Original FA gains, arrival radius and damage-related disengagement are unknown;
future research should recover the original autopilot behavior specification.
