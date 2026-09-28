# Systems instrument and aircraft failures

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Contract and provenance

Implementation mode, requested by John on 2026-09-21. Systems (Shift-7) shows
THR, TEMP, OIL and HYD percentages, then internal FUEL and (+ EXT ... LBS).
It does not show weapons weight as fuel. Manual printed page 89 establishes
normal temperature as 0%, normal oil/hydraulic pressure as 100%. Throttle shows
the actual lever position. The supplied retail screenshot confirms the row
order, green text, right-aligned numbers and external-fuel parentheses.
Manual identity is recorded in the [envelope spec](envelope.md).

Manual page 161 establishes reduced pitch/roll control from damaged surfaces,
rudder yaw bias, unavailable autopilot after control damage, reduced control
from hydraulic damage and frozen surfaces/gear at zero hydraulic fluid/pressure.
Engine/oil/compressor damage reduces power; failure removes it. Oil damage causes
overheating, mitigated by low throttle. Compressor damage risks failure above
25% throttle. Fire precedes destruction. High G threatens a damaged structure.
Pilot wounds allow about 15 minutes to return, less after further wounds.
Damaged avionics and weapons become unavailable.

The [reviewed event mapping](../formats/systems-damage.md) identifies the retail
fuel, propulsion, fluid, device, surface, structure, avionics, pilot and hardpoint
faults. Original modules are never executed. Existing hit selection and repeat
limits remain in use; the player-visible progression below is fitted.

The player's aircraft takes these faults only with the Cheat menu's Damage
set to Realistic; Normal takes hit points only. See the
[cheats specification](cheats.md#behaviour-of-each-cheat). AI aircraft always
take them, as described under [AI aircraft](#ai-aircraft).

## Component ownership

The model is split into engine, fuel, fluids, controls, structure and pilot
components. Each owns its state and progression. A shared coordinator delivers
source faults once, couples fluid pressure to thermal behavior and combines
fatal outcomes. Instrument readings are views of those components; they never
create or advance damage. Source counters remain provenance and duplicate guards,
not a substitute for modeled component effects.

## Fitted rules chosen by the agent

These implement the manual's effects with deterministic 120 Hz state. They are
agent choices, not tuning requested by John. Healthy state is unchanged.

- Engine damage loses 25 percentage points of power per hit. Serious damage
  caps power at 25% and loses another 0.5 percentage points per second.
  Shutdown loses 1/engine-count of power, down to zero. Severed fuel lines
  remove all power. Flameout is restartable after six seconds by cycling the
  throttle to at most 25% then raising it.
- Compressor damage reduces power by 25% and fails the engine after 30 seconds
  of accumulated operation above 25% throttle per hit. This deterministic
  exposure budget replaces unknown retail random scheduling.
- Oil/hydraulic leaks remove 1 percentage point of fluid per second per hit.
  Oil pump damage halves pressure per hit. Oil pressure is fluid fraction times
  pump health. Hydraulic pressure follows remaining fluid. Engine-off healthy
  circuits retain their displayed nominal pressures as a fitted simplification.
- Oil starvation adds `(1-pressure) * (0.1 + 6*throttle)` temperature percentage
  points per second while running. Healthy oil cools by 2 points/second;
  engine-off cooling is 2 points/second. At 100% temperature the engine fails.
  Internal fuel leaks lose 4 lb/second per hit. External tank failures empty
  the actual tank. External fuel feeds engine consumption before internal fuel;
  empty tank mass remains until a future tank-jettison implementation.
- Fire raises temperature 10 percentage points per second and destroys the
  aircraft after 10 seconds; imminent explosion after 5.
  Hydraulic fire also exhausts hydraulic fluid. Fire does not auto-extinguish.
- Reduced surfaces retain half authority per hit. Bent surfaces add alternating
  signed 25% command bias, selected deterministically by axis. Damaged rudder
  also adds a 10% yaw bias. Hydraulic pressure
  multiplies authority; at zero pressure retain the last actual surface position
  and gear/device position. Control-line damage retains 30% authority. Unstable
  controls add a 0.2-amplitude, 2 Hz sinusoidal command. Stuck throttle retains
  the lever position at impact. These failures inhibit autopilot.
- Wing damage halves lift authority. Wing destruction is fatal. Structural
  weakness destroys the aircraft after two accumulated seconds above a load
  threshold `max(2, 9*(1-airframe_damage_fraction))`, considering absolute G.
- First pilot wound starts a 900-second timer. Later wounds halve time remaining.
  Expiry is fatal; landing alive on a supporting surface stops the bleeding.
  This is a fitted stand-in for medical treatment, without a campaign hospital.
  Nose/cockpit loss, a critical cockpit hit or an aircraft explosion kills the
  pilot immediately while still aboard. An [ejected pilot](ejection.md) is
  independent of later aircraft damage. Ground treatment cannot revive a dead pilot. Pilot death
  selects the F10 exterior view without stopping wreck motion.
- Flight-sensor failure suppresses flight HUD data. The first display failure
  blanks the radar page, the second the RWR page. Navigation failure replaces
  Nav contents with a fault and blocks waypoint autopilot. Equipment failures
  retain existing weapon/sensor/ECM effects and are reported in the sim log.

## Accumulated combat damage

John reported normal enemy fire leaving a flyable aircraft at 92% damage with
healthy systems on 2026-09-21. A single per-hit chance roll is insufficient for
this experience. In addition to existing chance-based faults, the following
agent-selected fitted milestones ensure degradation as total ownship damage
crosses 25%, 50%, 75% and 90%, inclusively:

| Total damage | Required affected group | Preferred new fault |
| --- | --- | --- |
| 25% | Pitch, roll, yaw or control linkage | Elevator authority halved |
| 50% | Engine or compressor | Engine power reduced by 25 percentage points |
| 75% | Hydraulic circuit | Hydraulic leak, 1 percentage point/second |
| 90% | Oil circuit | Oil pump pressure halved |

An existing fault in that group satisfies its milestone. Otherwise select the
first eligible fault in that group's fixed order, respecting the actual PT
weight, repeat limit and installed capability. Control order is elevator,
ailerons, rudder, damaged linkage, unstable linkage; engine order is ordinary,
serious, compressor, first shutdown, second shutdown; hydraulics use a leak;
oil uses pump then lines. Never invent a disabled source event. All milestones
crossed by one large hit are evaluated; dividing the same damage into many small
hits cannot bypass them. Repeated hits after a milestone do not repeat its fault.
The milestones apply to AI aircraft too, measured against their own hit
points. Faults continue to reach the normal sim log and D report.

## AI aircraft

John asked on 2026-09-28 for AI aircraft, friendly and enemy, to take the
same Realistic damage as the player, whatever the player's Damage cheat says.
Each hit on an AI aircraft rolls faults from that aircraft's own PT fault
table with the rules above: the chance fault, then the accumulated-damage
milestones. The totals are measured against the AI aircraft's hit points. An
aircraft already destroyed by the hit rolls nothing. Straight-flight fixture
aircraft and Quick Mission [dummy aircraft](dummy-aircraft.md) roll too, but
fly a fixed course, so their faults change nothing; they are lost only when
their hit points run out.

- Engine, fuel, fluid, control, structure and pilot faults act on the AI
  aircraft's flight exactly as on the player's, with the same fitted numbers.
- A station fault stops the AI firing that store, and it stays out if Air
  combat guns only is turned on and off. An external fuel tank fault empties
  the tank.
- A radar, infrared or visual fault blinds that sensor, and a failed radar
  stops transmitting. An RWR fault stops missile warnings from the RWR.
- An ECM fault loses the jammer, chaff or flares by the player's roll, drawn
  from a separate generator so it cannot shift decoy rolls.
- An AI aircraft killed by its own systems, for example a destroyed wing,
  failed structure, fire or a dead pilot, or one that crashes, is lost. It falls
  as a wreck, and the kill goes to its last attacker as the
  [debrief](debrief.md#kills) describes.

### AI pilot response to faults

Requested by John on 2026-09-28. The decisions below are agent-authored,
**fitted** responses to the existing component model. Original AI fault-response
thresholds and procedures are unknown. No hit points, fault probabilities or
failure progression change: AI milestones still use AI hit points, and the
player uses twice the aircraft hit points. Dummy and straight-flight fixtures
continue ignoring all faults in their fixed-course movement and decisions.

- An active pilot wound, fuel leak/feed failure, engine damage or shutdown,
  compressor fault, oil/hydraulic failure, damaged flight controls, jammed
  throttle, wing damage or weakened structure ends offensive engagement.
  The aircraft returns to its known home runway using the existing landing
  sequence. An existing landing remains valid. This decision stays committed
  through transient defense, later formation/attack orders and wound treatment.
  A new explicit landing destination is still accepted. Without a runway it
  heads toward the known home position; without either, it holds a level heading
  and reports that no recovery destination is known. No destination is invented.
- A damaged aircraft on the ground holds idle throttle and brakes rather than
  starting or continuing a departure. Landing rollout and taxi-clear continue
  through the existing sequence. Ordinary ground treatment still stops bleeding.
- A damaged compressor or reduced oil pressure requests at most **25% throttle**
  and no afterburner. Existing component rules determine whether this prevents
  failure or merely delays it. Low-speed flight at/below **1.35 times minimum
  speed**, a descent faster than **10 ft/s below 500 ft AGL**, and airborne
  takeoff/climb-out, final or go-around allow the flight controller's required
  military power instead. These safety exceptions can still worsen engine
  damage. A jammed lever remains jammed; the policy never writes over it.
- Other engine faults request no afterburner and return home, but do not impose
  an arbitrary low-throttle limit where the component has no throttle-dependent
  failure. A recoverable flameout holds at most **25%** through the component's
  six-second restart interval, then requests **50%** to complete its required
  throttle cycle. Permanent power loss cannot be restarted this way.
- Recovery flight uses at most **30 degrees of bank** and **2.5 G**, bounded by
  the aircraft's existing limits. Route, marshal and approach speed requests
  are capped at **1.6 times minimum speed**, with the airbrake above that request
  by **20 ft/s**. Lower final-approach speeds and full-power terrain/go-around
  commands retain their existing rules. Slowing the recovery prevents the lower
  bank limit from creating wide, repeated misses of the approach gates.
  A weakened structure additionally caps the
  requested load **0.25 G below** its existing failure threshold. These are
  requested limits, not a clamp or repair of actual motion. Defensive maneuvers,
  terrain avoidance and the existing unrecoverable-flight ejection remain.
- Fuel, engine, hydraulic or imminent-explosion fire makes a live airborne
  pilot eligible to eject after **0.25 seconds**, using the existing seat and
  escape mechanics and John's existing **70% chance per second**. The first
  opportunity is one second after danger begins; failed rolls can still be fatal. The current component model has no extinguisher and becomes
  fatal in five or ten seconds. Grounded aircraft hold instead. Healthy aircraft,
  dead pilots, unavailable seats and dummy aircraft never gain this response.
- Avionics and hardpoint faults retain their existing sensor, weapon and ECM
  gates. They do not grant missing contacts or repair failed equipment.

Replay thought records explain the fault, recovery destination, throttle request,
safety exception, jammed-lever limit and restricted maneuvering. Fault responses
use live component state, not the diagnostic record. Validation is recorded in
[AI damage-response checks](../baselines/ai-damage-response.md).

## Regional structural damage

Regional wing and control-surface damage continues to affect flight. John later
requested hiding all airframe damage visuals below 100% on 2026-09-21; that
presentation-only gate does not weaken these penalties. The following
agent-selected fitted rules connect damaged regions to ownship flight. With left/right wing damage L/R
and tail damage T, each clamped to 0..1:

- Roll authority is `1 - 0.6*max(L,R)`; pitch/yaw authority is `1 - 0.8*T`.
- Neutral roll bias is `0.35*(R-L)`, toward the damaged wing. Tail damage adds
  `0.15*T` yaw bias. These aerodynamic biases persist if hydraulics freeze.
- Lift authority is `max(0.15, 1 - 0.35*(L+R) - 0.2*T)`. An independently
  damaged-wing subsystem halves this again.
- Drag increases by `25*(L+R) + 15*T` percent.
- Any regional wing/tail damage disables autopilot. Input commands remain
  separate from actual held surface positions; severed geometry cannot regain
  effectiveness by freezing a surface or switching flight adapters.

The regional rule complements individual subsystem faults in both player and
AI flight. Dummy aircraft retain their fixed-course exception. A wing with 75% regional damage (its partial-tear visual is currently hidden)
therefore loses 45% roll authority, 26.25% lift and produces 26.25% roll bias,
before additional subsystem penalties. The exact retail coefficients are unknown.
Ownship damage reports cap at 99% while any hit points remain; 100% means
actual destruction, not rounding of 99.x%.

## Presentation

Use the existing imported font inside the aircraft's
[instrument bezel](instrument-bezel.md). In the 162 by 160 window, labels start
at x=21 and values end at x=139. Gauge rows are y=32,46,60,74; the separator is
y=90; fuel rows are y=100,114. Healthy RGB is (72,172,55), amber (235,187,60),
red (235,70,50), fitted to the supplied screenshot. Buttons remain blank.
Pressure below 75% is amber, at/below 25% red. Temperature above 25% is amber,
at/above 75% red. These thresholds are fitted. As clarified by John on
2026-09-21, all damage notifications use the existing bottom-center sim log.
Do not add fault rows, labels or new buttons to Systems. D reports overall
ownship damage percentage, the temperature/oil/hydraulic readings, available
engine power and failed systems in that log. It no longer injects a hit.
The explicit developer damage command remains available for fixtures. Messages
queue in the existing log so simultaneous faults are not silently overwritten,
with duplicates suppressed and wrapped to three lines. Generic hit notices are
limited to once per four simulation seconds; named subsystem events remain
individual. The queue is bounded to 64 notices.
No rendered retail assets or extracted strings are committed.

Unknown: exact original gauge palette, pressure transients, per-engine behavior,
thermal/leak/fire timing, stochastic failure probability, repair/refueling and
medical-treatment rules. Next research: bounded review of DAMAGEUpdate and the
Systems draw inputs with the same build. Restricted native-table research flight
keeps its existing solver; host failure effects are applied without changing the
selected adapter. The AI response above is an authored addition; original fault-response tactics
remain unknown.
