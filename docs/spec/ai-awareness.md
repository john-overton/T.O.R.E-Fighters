# Air-to-air awareness, memory and mission rules

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Scope and status

Development specification for M1 air-to-air awareness, memory, search, missile
defense, RWR missile presentation and mission engagement. Visual awareness,
aircraft memory and contact-loss search are implemented in the existing aircraft
runtime. Missile defense and shared AI/RWR missile information are implemented
for reviewed missile profiles. Mission roles, engagement rules and per-group
Quick Mission objective stamps are connected to both sides' aircraft. Delivery sequencing is defined in
[M1e](../ROADMAP.md#m1-air-to-air-awareness-delivery).

The requirements are **opinionated** gameplay behavior. Existing equipment,
flight limits and missile guidance contracts retain their documented provenance.
This specification does not establish original-game parity.

### Component provenance

| Component | Provenance |
| --- | --- |
| Skill-scaled 3 to 5 mile visual awareness, memory durations, Ace recall and Novice single-target restriction | Opinionated product requirements |
| Visible searching, mission-based engagement and escort priorities | Opinionated product requirements |
| Guidance-dependent warning onset, visual-only passive-missile detection, jink/notch/dive and countermeasure capability | Opinionated product requirements |
| Skill-dependent defensive commitment and re-engagement; shared AI/RWR missile information and blinking incoming threats | Opinionated product requirements |
| Nautical-mile interpretation, intermediate visual ranges, cone geometry, intermediate-skill capacity, sensor refresh and lifecycle knowledge policy | Opinionated agent-authored defaults |
| Search motion, warning delivery scheduling, threat classification and persistence, maneuver selection, burst scheduling, timing margins and escort limits | Opinionated agent-authored defaults; numeric tuning remains subject to acceptance |
| RWR snapshot allowance, visual-threat display integration, plotting and blink cadence | Opinionated agent-authored defaults |
| Missing visibility effects, missile notch rejection and passive countermeasure susceptibility | Unknown where not specified; fitted rules require documented constants before implementation acceptance |

Agent-authored defaults below define the initial development configuration. They
are not recovered original-game values.

## Integration baseline

The [AI spec](ai.md) and [experience spec](ai-experience.md) describe existing
rules. Four resolved skills exist: Novice, Average, Experienced and Ace. Use the
resolved pilot skill, after existing assignment rules, not the menu selection.

The runtime provides actor-owned sensors, weapons, flight state and controllers.
`ai::awareness` holds current observations separately from frozen aircraft memory.
`ai::mission` feeds only current observations into combat selection and supplies
lost hostile snapshots to the controller's search input. Visual attention uses
its own skill-scaled cone. Radar and infrared use the shared sensor component;
terrain blocks both sensor and visual observations. Production sensor import
failures are reported instead of using the sensorless fixture path.

`controller::Activity` exposes searching, acquiring and rejoining through the
existing Target window. SEARCHING also describes an airborne leader following
its current heading before first acquisition; wingmen retain formation activity.
Mission route import remains unavailable. Explicit protect/destroy assignments
feed `ai::engagement`; the original `MissionDefensePriority` branch remains a
research gap rather than a runtime requirement. Search returns to the assigned
protection, patrol center or existing formation/heading behavior.

The current host supplies terrain but no weather visibility limit to AI
perception. Cloud/night visibility remains a fitted clear-air assumption, not
complete environmental visibility. Initial integration and validation limits
are recorded in the [awareness baseline](../baselines/ai-awareness.md).

### Evidence references

The local 1999 EA/Jane's Fighters Anthology manual,
`.local/missile-update/manual.pdf`, SHA-256
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`. Printed
pages 94 and 110 describe radar-emission detection, up to 50 NM; page 97
separates RWS search from TWS targeting; page 101 explicitly lists SEARCHING,
ACQUIRING and PURSUING activities. Printed page 131, PDF page 135, distinguishes
enemy seeker tracking from an inbound missile. Page 131 was visually inspected;
the other cited pages were reviewed through extracted text. This establishes
manual-described behavior, not executable timing or a claim that every TWS/RWS
selection generates a warning. Executable build provenance for existing rules
remains in the [AI baseline](../baselines/ai-research.md).

## Visual awareness and memory

Use nautical miles and the existing 6,076 feet per NM convention. Visual range,
retention duration and memory capacity depend on resolved pilot skill.

| Resolved skill | Visual range, NM | Memory after last observation | Remembered aircraft capacity |
| --- | --- | --- | --- |
| Novice | 3 | 15 seconds | One selected hostile |
| Average | 3.5 | 90 seconds | All previously observed aircraft |
| Experienced | 4 | 120 seconds | All previously observed aircraft |
| Ace | 5 | Until destruction or mission end | All previously observed aircraft |

Memory capacity for Average, Experienced and Ace is bounded by the mission
aircraft count.

The visual detection volume is a circular forward cone with a 60-degree
half-angle, identical at every skill, with skill changing range only. Measure 3D
angle from the aircraft nose and spatial distance; include exact angle/range
boundaries. This is separate from B01's ahead/off-beam combat predicate. Terrain
blocks visual acquisition. Reuse available environmental visibility limits,
capped by the skill range; any missing cloud/night occlusion remains explicitly
fitted and must not be described as complete weather visibility. Do not alter
the player's sensors or imported equipment profiles. AI attention filters the
visual channel separately from radar and infrared hardware.

A remembered record stores stable identity, last observed world position,
velocity, observation tick and observation source. Relative bearing is
recomputed from that frozen position and own movement. A lost contact never
receives hidden position, heading or velocity updates. Aim search at its last
observed position without indefinite velocity extrapolation. Ace's unlimited
duration is perfect recall of an old observation, not perfect tracking. No
future target turn becomes known without a new observation.

A valid current visual, radar or infrared observation refreshes the remembered
record and timer. Keep source timestamps separately. Radar observations can
sustain awareness outside the visual cone. Scope history and a mere selected
marker do not count as current observations. At loss, preserve the last measured
record; expire it exactly at the configured age using simulation ticks: Novice
1,800, Average 10,800, Experienced 14,400. Pause does not age memory.
Reacquisition refreshes it.

Novice may perceive several current contacts but remembers only its chosen
hostile. Changing target discards the old memory. On a kill or expiry it cannot
select an old forgotten aircraft. It must freshly detect another aircraft on its
own scope, or visually inside its 3 NM cone. A wing order or warning may cue a
search but must not silently refill Novice's target memory.

Destroyed actors become ineligible immediately. Simulation lifecycle removal
clears their memory records at every skill, including an unobserved kill. This
is a bounded knowledge shortcut, exposes no killer or survivor positions, and
must not synthesize a radio kill confirmation. Do not attack detectable wrecks.
Clear all records on mission restart and never reuse an old record for a new
actor ID.

Memory authorizes investigation, never a weapon solution. Firing and missile
support still require the existing appropriate live sensor/seeker geometry,
weapon envelope, ammunition and terrain checks. Target retention inside B41's
20,000 feet cannot bypass the new awareness or mission eligibility gates.

## Searching and player-visible activity

Required states and transitions:

- SEARCHING: no current eligible observation, or investigating a remembered
  location. Fly toward that location while scanning; with no memory, follow the
  assigned patrol/intercept route or stay with the protected formation.
- ACQUIRING: a current eligible contact is chosen but weapon acquisition is
  incomplete. Maneuver and request the appropriate sensor designation.
- PURSUING and ATTACKING: retain existing tactical/weapon services, with a
  current permissible observation and a mission-eligible target.
- DEFENDING or EVADING: an actionable threat interrupts ordinary engagement;
  after it clears, reassess the mission and available observations.
- REJOINING or RETURNING: engagement is no longer permitted, memory expired,
  escort separation exceeded its limit, or existing fuel/survival rules require
  withdrawal. Never call this LANDING before an actual landing phase.

Turn toward the last observed position; within 1 NM of it, request a level orbit
through the existing steering adapter. Stop investigating on memory expiry, a
higher-priority threat, mission completion or the escort leash. For an Ace, cap
one uninterrupted investigation at 120 seconds before returning to its mission,
while retaining the record. This prevents unlimited recall from causing
unlimited abandonment of an escort.

The fitted search orbit is clockwise, level at entry altitude, with a 0.75 NM
(4,557 feet) radius. Correct radial error proportionally over 0.25 NM (1,519
feet), clamped to 45 degrees toward or away from the center. Measure orbit
radius horizontally. Correct altitude toward entry altitude over one orbit
radius of horizontal look-ahead, with pitch limited to plus or minus 45 degrees.
Reevaluate guidance each simulation tick using bounded 3-second motion requests
at corner speed through the existing flight adapter. Approach
the last observation directly outside the 1 NM entry radius. These are
agent-authored search constants; the manual's SEARCHING label does not establish
orbit geometry or scan timing.

Expose these states from the simulation to the existing Target window activity
line. Keep the existing tactical goal codes; SEARCHING is an activity, not a new
unsupported goal letter. Preserve the distinction between attacking the player
and attacking somebody else. Populate MISSION OBJECTIVE only from a real
protect/destroy assignment. The display reads state and cannot drive decisions.

Optional development visualization: cone extent, last observation point, age,
source and decision reason. Keep this in a debug overlay. Target-view activity
is required; the overlay is optional and cones are not part of normal gameplay.

## Missile awareness and defense

Radar search, contact selection and aircraft tracking do not themselves tell the
AI that a missile has been fired. Passive reception may still reveal an emitter
for ordinary awareness; it never manufactures a missile threat. Warning onset
comes from the actual missile lifecycle or an incoming missile seen by the
pilot. This warning contract replaces B47 launch-warning eligibility and delays
in the M1 AI path. The recovered [B47
behavior](ai.md#b47-threat-warnings-countermeasures-and-reason-priority) remains
the reference for the existing implementation, not the acceptance target for
this replacement.

Use the [missile specification's guidance
categories](missiles.md#four-game-guidance-types). A denotes active radar with
an initially silent midcourse phase. S denotes continuously supported radar. E
denotes passive emitter homing and is distinct from an A missile before pitbull.
Preserve the existing weapon classifications.

| Guidance | When the AI learns of the incoming missile | Initial device response |
| --- | --- | --- |
| A: active radar | No automatic launch warning during silent midcourse. Warn when its active seeker acquires this aircraft and enters PITBULL. | Chaff |
| S: supported radar | Warn immediately on a valid supported launch against this aircraft; the launcher must maintain the required track for guided flight. | Chaff |
| I: infrared | No automatic launch or seeker warning. Respond only after visually detecting the incoming missile. | Flares if its class is known; otherwise the mixed visual-threat burst below |
| E: passive emitter / other passive guidance | No automatic launch or seeker warning. Respond only after visually detecting the incoming missile. | Mixed visual-threat burst unless its susceptibility is independently known |

Immediate means the next fixed simulation tick, at most 1/120 second. Do not add
the old B47 launch-distance, flight-state or experience delays to these warning
events. An A missile already active at boresight launch can warn immediately if
it acquires this aircraft; an active seeker searching empty space does not alert
every aircraft. Use the missile's actual active/target state, not a second AI
calculation of activation range. Merely entering ACTIVE SEARCH does not
establish a missile targeting warning. The missile spec defines PITBULL as
successful active-seeker acquisition; use that event for both cued and boresight
shots. Reacquisition cannot create duplicate launch events or infinite bursts.

Visual missile detection uses the pilot's skill-scaled visual range and cone,
with terrain and available visibility limits, including exact boundaries. No
rear-hemisphere or beyond-range launch event may reveal an IR/passive missile.
Visual detection can also reveal a radar missile before its electronic warning.
Motor burnout alone does not make the missile invisible; seeing a launch flash
without seeing the incoming missile is not sufficient.

The perception service may test world geometry to produce a sighting, but the
controller receives only the observed missile position/motion, tick and source.
Classify a visually observed missile as incoming when measured motion is closing
and its current straight-line closest approach is within 1,000 feet of own
aircraft over the next 15 seconds. Use two successive visual samples to estimate
motion; a single silhouette does not reveal its intended target. Validate these
authored thresholds against guided crossing approaches before acceptance. Do not
use the hidden missile target ID to grant visual knowledge. Visual-only sighting
does not reveal seeker class or launcher identity automatically.

Keep perceived missile threats separate from aircraft target memory. A Novice's
one remembered hostile does not prevent reacting to several visible missiles,
and a missile sighting does not reveal a forgotten launcher. Retain a lost
visual threat for 2 seconds using its last observation, then clear it unless
sighted again; passive missiles cannot refresh this record remotely. Electronic
threat reports may refresh while supported guidance or an active seeker remains
directed at the aircraft. On signal loss retain the last warning for the same
2-second grace, without live geometry, then reassess. Destruction, impact and
expiry retire the missile's threat record. These bounded lifecycle shortcuts
convey no surviving aircraft positions. Aircraft memory timers above are
unchanged.

### Jink, notch, dive and countermeasures

Once the response assessment calls for defense, select a notch for an
electronically reported radar threat and a jink for a visual-only incoming
threat. Combine a dive with either only when terrain and speed safety checks
permit it. All skills can use these responses. Existing experience-dependent
flying limits still apply. Immediate warning receipt means immediate awareness,
not mandatory immediate maneuvering or dispensing.

- **Jink:** alternating lateral break requests around the heading at detection.
  Initial pattern: +45 degrees for 2 seconds, then -45 degrees
  for 2 seconds, with initial side selected deterministically. Reassess after
  each leg; use existing flight limits rather than instantaneous attitude changes.
- **Notch:** request a heading 90 degrees left or right of the known radar-source
  bearing, choosing the smaller turn and a deterministic tie break. For S use
  the supporting radar's bearing; for A after pitbull use the missile radar's
  bearing. Refresh only from received evidence. No known radar bearing means
  jink instead. A notch command is an attempt, not automatic missile defeat.
- **Dive:** combine the break with a -20-degree pitch request only when
  the predicted path over the next 5 seconds remains at least 1,000 feet above
  queried terrain and inside the aircraft speed envelope. Existing terrain
  avoidance can override it earlier. Otherwise remain level or climb as needed.
- **Devices:** request a burst when the time-based response assessment calls
  for it, while maneuvering.
  Release two chaff for known radar; two flares for known IR; two of
  each available type for an unclassified visual missile. Visual detection does not identify passive guidance or establish device
  effectiveness.
  Reuse the quarter-second device scheduler; for a mixed burst alternate chaff
  and flare. Debit actual inventory and continue with the available type if
  the other is empty. Repeat no sooner than 2 seconds after burst start, and
  only while a current sighting or electronic warning supports the threat.

The time-based release decision replaces the old 35/50/75/90 percent release
gate in the M1 AI path, not the established probability that a device
actually decoys a missile. Entering the release window requests a burst if the
matching inventory is available. Never equate dispensing with a guaranteed
escape. Device absence or exhaustion cannot prevent maneuvering. Neither an
existing attack on the launcher nor a same-side missile may suppress
self-preservation once an incoming threat is perceived. Weapons-hold orders
prohibit offensive fire, not defensive maneuvering or countermeasures.

Aggregate simultaneous threats rather than starting conflicting maneuvers or one
burst per missile every tick. Prioritize the smallest positive time to closest
approach from current observed motion; bearing-only radar warnings precede
visual threats whose approach time is unknown. Keep the current threat on a tie.
A new higher-priority threat may change the maneuver, but the per-aircraft burst
interval still applies. Represent activity as DEFENDING/EVADING, with the chosen
maneuver available for debugging. After the last perceived threat clears,
reassess mission duty and current aircraft contacts.

### Skill, time available and returning to the fight

Assess whether enough time remains to maneuver and deploy countermeasures.
Novice can maneuver at long range without dispensing. Ace uses its estimated
margin to defeat the missile and continue or resume attacking an eligible
target. This changes decision quality and commitment, not sensor hardware,
access to hidden missile state or physical turn performance. No skill guarantees
survival or a counter-kill.

Awareness, maneuver commitment and device release are separate decisions. A
defensive maneuver does not automatically trigger a countermeasure burst.

At each new observation, estimate time to closest approach from measured
relative position and motion. This is an imperfect threat-time estimate, not
knowledge of the missile's future guided trajectory. Estimate the time required
to reach the chosen defensive heading/pitch using current speed, bank, usable
turn rate, roll-in time, altitude and the selected flight model's limits. Add an
uncertainty margin. Do not assume an instantaneous 90-degree turn or use the
unseen missile's remaining energy. A stale observation increases uncertainty; a
bearing-only radar warning supplies no precise range or approach time.

Initial defensive timing parameters:

| Skill | Extra time reserved beyond estimated maneuver time | Long-range behavior with reliable observed motion |
| --- | --- | --- |
| Novice | 8 seconds | Start a simple defensive maneuver on detection even when there is ample time; withhold devices until the release window |
| Average | 5 seconds | Continue mission until estimated threat time reaches maneuver time plus margin, then defend |
| Experienced | 3 seconds | Same threshold rule, with less conservative commitment and continuous reassessment |
| Ace | 1.5 seconds | Preserve the attack while comfortably outside the threshold; use the available margin, defend, then reassess attack opportunity |

The fitted response estimate uses half the load-limited coordinated-turn rate
at current speed, bounded by the aircraft's maximum bank. Pitch authority uses
half the available normal acceleration above 1 G divided by current speed.
Roll-in allowance is the requested bank plus the magnitude of current bank,
divided by the loaded roll-rate limit, plus 0.7 seconds of control settling.
Gravity is 32.174 feet per second squared. These agent-authored conservative
estimates use loaded aircraft limits; they are not exact trajectory predictions.
A dive is speed-safe only when current speed plus five seconds of gravitational
acceleration at 20 degrees remains within the current maximum-speed envelope.

Maneuver-time estimation must be bounded against measured flight-model turns
before acceptance. If estimated available time is already too short at any
skill, act immediately with the fastest feasible safe break and available
countermeasures; do not refuse defense because the preferred notch is
unreachable. Aircraft capability can make a dive or jink preferable to a late
notch.

Device release window: start a burst when estimated approach time is at most 6
seconds, or immediately when the assessment says there is insufficient time for
the chosen maneuver. A distant threat can therefore prompt a Novice turn without
a burst. Retain the 2-second per-aircraft burst interval while a current
observation supports the threat; do not reset it when switching missiles. The
6-second window is fitted and must be checked against actual device and seeker
effects. A radar warning without usable motion/range instead triggers immediate
conservative defense and one available radar-countermeasure burst; never claim
that Ace can safely wait on a precise countdown it cannot observe. Further
bursts still follow the current-warning and cooldown rules.

Ace's smaller margin is deliberate tactical risk, not intentionally slower
warning delivery. Reassess every simulation tick using only latest permitted
measurements; terrain, low energy, several threats or lost observations can
force earlier defense. With uncertain motion, use the conservative fallback
rather than applying the optimistic Ace margin. Debug output must expose
estimated threat time, maneuver time, margin and the chosen response so the
reason can be inspected without altering Target-view gameplay.

After the perceived threat clears, resume the mission's priority selection. Ace must preserve a valid offensive solution through a defensive maneuver when
feasible, and promptly re-engage when defense permits. Own missile support is a
tactical constraint, never a reason to ignore an imminent impact. If survival
requires breaking supported guidance, do so and let the existing missile
support-loss rules apply. Attack the launcher only if independently identified,
currently observed or legitimately remembered and permitted by the mission
rules. A silent or anonymous missile cannot reveal its attacker. Escorts return
to protecting their charge instead of automatically chasing revenge. Novice must
still obey its single-target memory and fresh-reacquisition rule when its former
target is gone.

Active missile seekers use the shared Advanced notch preset: a 60 ft/s radial
speed half-width and a 0.45 center range factor at full ground clutter. Clutter
uses the target's actual height above terrain and the downward sight angle.
This is a fitted shared default, not weapon-specific recovered rejection.
Supported missiles lose measured steering when their launcher's own radar loses
support; they may reacquire before guidance expiry. A notch can shorten detection
range or break support; it never guarantees missile defeat. Passive E countermeasure
susceptibility also remains weapon-specific and unknown where not specified.
Ground/emitter homing can be tested with fixtures. A2G AI is out of scope.

### Shared RWR missile information and display

RWR supplies missile information to AI and displays missile contacts for the
player. Known incoming threats blink for the receiving aircraft. The
[manual-based RWR specification](rwr.md) defines symbols, indicators and layout.

Create one actor-owned threat-information service in `tore-sim`, consumed by
both AI and the player's RWR presentation. No AI-only omniscient missile list,
and no UI-generated threat decisions. Each record identifies the observed
missile, evidence source, observation tick, bearing, any permitted range/motion
estimate, known guidance class if available, and whether it is known to threaten
this receiver. Include freshness and loss state. Launcher identity remains
optional and must not be inferred from hidden missile ownership.

RWR information policy: once an S launch warning or A pitbull warning is
eligible, the shared RWR service supplies missile bearing, range and
relative-motion snapshots every 30 ticks (0.25 seconds), with an initial
snapshot on warning receipt. This is an explicitly opinionated gameplay
allowance for ranged RWR missile plots and skill-dependent response assessment,
not a claim about real passive receiver ranging or recovered retail precision.
Only eligible known radar threats receive it; no midcourse A, unseen I/E, hidden
launcher pose or future guidance trajectory is exposed. Controller and display
use the same snapshot and its age. Do not look up the missile's live world pose
between updates. Bearing-only observations remain valid when a ranged report is
absent; they use the conservative defense rule above. Validate this initial
exact-sample approximation in play before considering fitted estimation error.

Own launched missiles also appear as steady dots, as described by the manual.
The shared service supplies own-ordnance snapshots at the same 30-tick cadence,
an opinionated telemetry allowance that does not reveal another aircraft's
silent weapons. Unrelated active missile emitters provide bearing-only records
within 50 NM unless independently seen. Only a directed S warning or acquired
A threat supplies the authored radar threat range/motion snapshot. The player's
visual receiver uses the 5 NM cone as an agent-authored host default; AI uses
its resolved skill. RWR failure suppresses electronic reception but not vision.

Visual I/E observations feed the same threat service as visual evidence, never
as electronic detections. Show a visually acquired incoming missile on the
combined RWR threat display too, with its visual source retained internally.
This does not grant a passive missile warning before it is seen. Its
range/motion and incoming judgment come from the visual observation rules; no
hidden seeker/target metadata is added. Radar-emitter reception by itself can
identify an emitting missile only if that classification is established; it
cannot assert who the missile targets. Unknown emitter plots stay unknown.

Presentation contract:

- A known missile targeting this aircraft blinks as a missile dot. A detected
  missile not known to threaten this aircraft stays steady. Never blink solely
  because it belongs to the enemy, is selected, or exists in the world.
- S against this aircraft starts blinking with the immediate launch warning.
  A starts blinking at actual pitbull against this aircraft. I/E appear only
  after visual detection and blink only when the observed incoming test supports
  a threat to this aircraft. The visual threat judgment is an authored estimate,
  not access to the passive missile's actual target assignment.
- The same missile may blink for one aircraft and be steady or absent for
  another, according to each receiver's knowledge. No wing-wide propagation of
  missile position snapshots is implied.
- Use heading-relative bearings and the existing 5/10/20/30/50 NM display scales.
  Plot measured range only when present. Unknown-range bearings
  use a distinct rim tick, and a ranged threat outside the selected scale uses
  a clipped rim marker. Neither is drawn as a falsely ranged dot. Display scale
  and whether the RWR window is open do not change AI threat knowledge.
- Blink cadence: 60 simulation ticks on, 60 off, one full cycle
  per second at 120 Hz. Pause freezes phase; rendering rate does not affect it.
  A blink's off phase never removes the threat from AI state.
- During the 2-second lost-observation grace, show the last bearing/range as
  stale with a steady marker; cease blinking when current targeting evidence
  ends. The AI may finish its conservative defense during that grace, but it
  receives no new measurements. Remove expired, impacted or destroyed contacts
  and clear all records on restart.
- Respect RWR failure for electronic observations and the failed player panel.
  A failed receiver does not blind the pilot to visually seen missiles, and
  that visual evidence can still drive AI defense. No separate AI exemption.

Player RWR missile plots and warning onset are in scope. Human audible-warning
changes are out of scope. Any audio/visual timing mismatch must be recorded as
an integration limitation.

## Mission roles and rules of engagement

Separate role (what to accomplish), stance (when allowed to engage), and current
activity (what the aircraft is doing). Skill controls awareness and execution;
it does not rewrite the mission. Supported stances are weapons hold,
self-defense, protect-assigned-aircraft and engage-assigned-hostiles. Weapons
hold prohibits offensive firing but permits evasion and countermeasures;
self-defense permits action against a perceived immediate threat. Explicit hold
orders take precedence over automatic target selection. Keep existing survival
and fuel limits authoritative.

| M1 role | Priority among known, eligible contacts | When to stop pursuit |
| --- | --- | --- |
| Free engagement / combat air patrol | Immediate threats, then assigned hostiles, using existing distance/wing assignment ranking within a priority | Target knowledge expires, patrol boundary or survival limit |
| Intercept | Assigned hostile aircraft, with self-defense interruption | Objective destroyed, order canceled, or survival limit |
| Escort / protect | Observed attacks or perceived incoming missiles threatening the protected aircraft, then known hostile escorts obstructing that defense, then other assigned threats | Threat clears and a higher duty applies, or escort leash exceeded |
| Self-defense / disengage | Immediate threats to self; otherwise return to assigned flight | Threat clears; do not chase incidental enemies |

Before each role's ranking, handle immediate inbound threats to self using the
existing defense service. Escort response must restore protection after the immediate threat clears. Enemy escorts use the
same rules to protect their own assigned aircraft.

Mission context must identify protected aircraft, destroy objectives, escort
relationships and any patrol region. Allegiance or proximity alone does not
identify an escort or objective. For M1, supply these explicit assignments from
Quick Mission presets and synthetic scenarios, independently of the M2 campaign
importer. Preserve the existing default Quick Mission behavior until these
assignments and defaults are deliberately connected.

Escort reporting policy: protected aircraft share perceived missile threats and
observed attacks with their assigned escorts on the next tick. Share the
observed threat bearing and only identities actually known, not the hidden enemy
world pose. A missile warning alone cannot identify its launcher as an aircraft
target. Escorts must independently acquire the threat before firing. This narrow
report is separate from any future wing-wide contact sharing and does not defeat
Novice's single-target rule. Treat perceived missile/attack events as threat
evidence; do not read an opponent's private controller goal. Mission-known
escort identity is metadata, not detection.

Escort pursuit limit: 10 NM from the protected aircraft, with re-engagement
allowed inside 8 NM to avoid oscillating at one boundary. Outside the leash,
evade immediate attacks and rejoin; do not initiate another chase. Within one
priority keep the current eligible target to limit switching; a new
higher-priority threat may preempt it. Use B41 ranking within a newly selected
priority and stable actor ID for exact ties. Patrol extent comes from the
assignment; an absent region does not authorize unlimited patrol.

### Assignment delivery and observed attack reports

Explicit accepted target orders replace the aircraft's current assignment with
an intercept of that target. Hold orders set weapons hold. Protect Me creates a
persistent escort assignment for the player; Disengage changes to self-defense.
Routine break, formation and spacing commands do not rewrite the mission.
Target selection may retain a currently detected assigned aircraft outside its
weapon range so it can approach. Actual firing still requires a valid weapon
solution, including range, direction, support and terrain checks.

Attack reports expire exactly 240 ticks after their last observation. An actor
receives its own observations immediately; assigned same-side escorts receive
copies on the next simulation tick. Reports preserve their original observation
time and do not refresh themselves by forwarding. Unknown attackers remain
unknown. A world-relative bearing may direct a level search at corner speed,
using one-second bounded motion requests, but supplies no range, aircraft memory
or weapon target. This cue ends on expiry, a real target, recovery, own missile
defense or the escort leash.

A supporting radar can be associated with an attacker only when exactly one
independently observed hostile RF emitter lies within 2 degrees of the received
supporting-radar bearing. Active-missile radar bearings never identify the
launcher. A visible departing missile or tracer can identify a shooter only
during its first 30 ticks, with exactly one independently observed, visually
eligible hostile aircraft within 1,000 feet of the observed departure point.
Incoming trajectory evidence is required; hidden projectile target/owner IDs
cannot establish that association. These are fitted identification rules.

Escort distance is horizontal. Escort leaders follow the assigned live friendly
charge using the existing delta guidance; members retain their own wing leader
only when that leader has the same protection assignment. Leaders use existing
delta slots 1, 4 and 7 by wing index. Outside the 10 NM leash, rejoin the charge;
resume engagement at or inside 8 NM. Rejoin guidance uses corner speed with
pitch limited to plus or minus 20 degrees in three-second bounded requests.
Immediate missile defense and fuel recovery take precedence. CAP returns toward
its assigned center outside the patrol radius. Full route navigation and
mission success/failure scoring remain outside this slice.

### Quick Mission objective stamps

Each of the three friendly and three enemy groups has its own objective stamp.
Click the stamp to choose an objective. Right-click cycles backward. Tab and
arrow navigation include all six stamps; popup navigation uses existing keys.
Use the original creator art and font.

| Objective | Assignment |
| --- | --- |
| Use mission setting | Inherit the selected mission preset; normal default is free engagement |
| Free engagement | Engage observed eligible hostiles |
| Combat air patrol | Patrol a 10 NM horizontal circle centered on the player launch position |
| Intercept opposing group 1, 2 or 3 | Assign every aircraft in that group as a destroy objective |
| Escort another same-side group | Protect every aircraft in the chosen group |
| Self-defense | Engage only independently identified immediate attackers |
| Weapons hold | No offensive fire; defense and countermeasures remain available |

Group references are resolved to actual launch identities. Friendly group 1
includes the player. Same-side intercept and opposing-side/self-group escort
assignments are invalid and cannot manufacture targets. An inactive referenced
group resolves to an empty assignment. Its aircraft must never be substituted
from a different group. Inactive groups retain their selected stamps for later
editing. Mission restart rebuilds the same group assignments with fresh memory.
The settings are session-local; campaign/save persistence remains separate.

`--ai-mission free|cap|intercept|escort|self-defense|hold` selects the inherited
Quick Mission preset. Explicit group stamps override it. Intercept targets the
first enemy aircraft; its other aircraft protect it. Escort assigns friendly
AI to protect the player, the enemy principal to intercept the player, and
enemy escorts to protect that principal. These are authored M1 presets using
current fighter aircraft, not additional bomber or transport behavior families.
Hostile escort relationships are explicit metadata derived from assignments;
they affect priority only after observation and a perceived threat report.

Every AI aircraft inherits its group's resolved duty, on both sides. Shift-4
shows that duty in a separate objective line, such as `INTERCEPT ENEMY 1`,
`PROTECT FRIENDLY 2`, `AIR PATROL` or `HOLD FIRE`. Activity remains a separate
live field. The `MISSION OBJECTIVE` marker still means the selected aircraft
is a protect/destroy objective for the player's assignment. The player's group
stamp supplies objectives and AI-wingman orders; it does not automate human
controls or enforce player trigger discipline. Dummy aircraft retain straight
flight regardless of their stored objective.

## Acceptance criteria

Use synthetic fixtures and fixed 120 Hz headless simulation. Required cases:

- All four skills at exact range/cone boundaries, behind the cone, terrain
  occlusion and reacquisition. Skill changes no radar equipment capability.
- Memory just before/at expiry, paused time, source refresh, restart, destroyed
  IDs and Novice switching. A target turning unseen must not update its record.
- An Ace remembers several aircraft after minutes; a Novice kills its target,
  ignores a forgotten off-scope aircraft and reacquires only on fresh detection.
- Search visibly changes activity in Target view. Acquisition, attack, defense
  and rejoin transitions represent actual simulation state.
- Radar selection/tracking alone produces no missile alert. A is silent before
  actual pitbull; S warns on launch; I/E never create automatic warnings. Cover
  boresight activation/acquisition, reacquisition, destroyed missiles and launches
  by both player and AI, without duplicate events or changes to player audio.
- Visual missile range/cone/terrain boundaries, motor burnout, crossing and
  receding missiles, lost sightings and the 2-second grace. Hidden seeker class,
  target ID and launcher pose never leak into the controller's observation.
- Shared RWR/AI records agree on identity, source, freshness and warning onset.
  Verify ranged and bearing-only plots, one-second blinking, steady non-threats,
  target-specific visibility, out-of-scale markers, pause, panel closure, receiver
  failure and stale/removal behavior. Hidden I/E and pre-pitbull A never leak
  through the RWR feed. Confirm actual live display with a rendering smoke test.
- Jink legs, correct S/A notch-source bearings, unsafe-dive rejection, empty
  dispensers, mixed bursts, cooldown and simultaneous threats. Measure actual
  support/seeker loss and decoy outcomes separately from maneuver requests.
  Warning receipt and memory alone must never authorize offensive firing.
- Escort chooses a known threat to its charge over a nearer unrelated hostile,
  recognizes explicitly assigned hostile escorts only when observed, handles
  warning-only bearings, respects hold orders and returns at its leash.
- Reliable distant-threat fixtures show Novice maneuvering without devices,
  while Ace can preserve offense until its fitted margin is reached. Verify
  threshold equality, too-late fallback, bearing-only warnings, uncertain/lost
  observations, low-energy turns and safe re-engagement. Compare decisions and
  feasible trajectories, not a requirement that Ace always survives or wins.
- Mixed roles, sides and skills, simultaneous threats, empty stores, fuel
  withdrawal and preserved missile support. Full seeker activation/pitbull
  integration is required before calling radar-missile encounters accepted.
- Twelve ported aircraft by four resolved skills, mixed-aircraft encounters,
  identical-seed restart/replay, render-rate independence and a measured
  30-aircraft encounter. Preserve all flight adapters and `--fixture-wings`.

## Exclusions and unresolved contracts

Surface AI, air-to-ground tactics, new aircraft behavior families, full campaign
objectives/import and general contact-sharing doctrine are outside M1 scope.

Before implementing the affected behavior, resolve and document:

- Environmental visibility where cloud/night occlusion is incomplete.
- Weapon-specific tuning beyond the shared active-seeker notch preset.
- Passive-guidance countermeasure susceptibility where no weapon rule exists.
- Defensive maneuver-time estimates validated against the selected flight model.

Validate authored tuning through the acceptance scenarios. Unresolved original
behavior remains unknown and is not a requirement for byte-level reconstruction.
