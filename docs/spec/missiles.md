# Missile guidance and lifetime

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-17. Delivery status is in the
[feature matrix](../features.md) and measured checks in the
[baseline](../baselines/missiles.md).
John requested four guidance types, pitbull, range, motor burn and tracking
lifetime. IR is independent of passive emitter homing. The activation thresholds
and launch modes are **opinionated game rules requested by John**. The manual supports delayed seeker activation in general, but does
not establish our distances or the additional launch mode.
John also requested approximate per-weapon activation distances on 2026-09-17.
John also requested launch-velocity inheritance, uncued seeker-active launches,
a HUD cone and narrow forward IR acquisition with tone. The numeric defaults,
unit choice and detailed rules below are agent proposals. Sequencing is in
[the missile update plan](../missile-update-plan.md).

## Four game guidance types

| Type | Before and after launch | Loss of support or signal |
| --- | --- | --- |
| S: supported radar | Aircraft holds radar lock on this missile's target throughout guided flight. For future ground/ship weapons, the equivalent support comes from their launcher. | No steering from hidden target state after support loss. Retain the last observed intercept and seek renewed support until guidance lifetime expires. No pitbull. |
| A: active radar | Cued launch: snapshot the aircraft firing solution and intercept, then activate at the matrix distance. Boresight launch: own seeker searches immediately, without aircraft designation or radar lock. | Aircraft updates are optional. Losing aircraft support freezes the last supported intercept; it does not destroy the missile or expose live target coordinates. After seeker acquisition, guide independently. |
| I: infrared | Use the weapon's own heat seeker, with either a designated target or narrow forward boresight search. Aircraft radar and installed FLIR are not required for boresight search. | Retain the last observed intercept and attempt same-target reacquisition until guidance lifetime expires. No radar or emitter fallback. |
| E: passive emitter | Use the surface weapon's receiver with designation for compatible emissions. An enabled passive receiver does not transmit radar. | Emission shutdown stops measurement immediately. Retain the last observed intercept and attempt same-target reacquisition until guidance lifetime expires. No heat fallback. |

John revised loss behavior on 2026-09-17: retain the known intercept and attempt
same-target reacquisition within the applicable seeker cone until guidance life
expires. The 2-second **agent-fitted** memory timer now separates MEMORY from
LOST searching; it never permanently disables reacquisition. Search can steer
toward the frozen intercept, but cannot follow hidden movement. S still requires
launcher support; IR and emitter weapons retain their own sensor types. This
supersedes the draft permanent-loss rule and does not interpret `trackT`.
Use current shared sensor support for S. A missile retains its own target ID;
changing cockpit selection never redirects an existing shot. At most one target
receives aircraft support at once. No aircraft combat AI is included.

## Manual-supported behavior

The [1999 EA/Jane's FA manual](https://pdfcoffee.com/famanual-pdf-free.html),
pp. 83-84, describes weapon/count, target range, signed closure, aspect,
hit probability, IN RNG, a fixed reticle, seeker diamond, target box and vertical
minimum/maximum range scale. Lock does not guarantee a good shot. Pages 118-119
describe delayed active-radar acquisition, designated IR launch and stronger
A2A IR growl with better lock; A2G IR uses a different tone. Page 137 relates
closure to reach. These passages do not establish uncued launch or our numerical
rules. Manual text was reviewed, not pixel geometry or a matching retail run.

## First-pass inventory matrix

All 135 imported JT definitions were reviewed for inventory inclusion. The table
contains **63 missile or missile-like candidates**, including ground/ship and
unresolved special roles, not 63 accepted playable missiles. [Inventory method,
source identity and validation](../baselines/missiles.md).

An asterisk means a **proposed game classification from record fields**, not a
confirmed retail guidance contract or a real-world missile identification.
`sig=2` suggests I; `sig=3` with support flag `0x200` suggests S, otherwise A;
`sig=4` suggests E. [Evidence limits and exceptions](../formats/missiles.md).
Hold rows remain inventoried and outside implementation until their role is
specified. Do not force laser/designator or command-guided cases into these four.

**Live** means included in the current twelve-aircraft default-store allowlists,
not that all guidance behavior has passed acceptance. **Catalog** means inventory
only. Missile presence does not authorize SAM, ship, ground-target or AI work.

Ranges are the source launch envelope converted using **6,076 feet per nmi**,
rounded to two decimals. They are not a guaranteed intercept range. Times use
the **existing fitted host conversion of 4 source timer units per second**.
Ignition and removal are launch-relative; burn is `(fuelT - igniteT) / 4`.
These are nominal compatibility numbers, not real-world motor specifications or measured
retail seconds. Exact source values, not rounded table text, drive implementation.
The old host clock quantizes launch age to quarter seconds; the new profile
should use launch-relative 120 Hz timers. Removal can precede burnout, as noted
below. Tracking/battery life remains a
separate proposed profile setting, never inferred from these motor numbers.

| Record (source label) | Type | Availability | Launch envelope nmi | Ignition s | Burn s | Removal s | Seeker max nmi | Seeker H/V half-angle deg | Active-on distance nmi (fitted) | Uncued search H/V deg (fitted) |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | --- |
| AA10.JT (AA-10T) | S* | Catalog | 1.97 to 19.75 | 0 | 53 | 106 | 19.75 | 45/45 | N/A | N/A |
| AA11.JT (AA-11) | I* | Live | 0.33 to 9.87 | 0 | 26 | 52 | 9.87 | 80/80 | N/A | 5/5 |
| AA11B.JT (AA-11B) | I* | Live | 0.25 to 9.87 | 0 | 26 | 52 | 9.87 | wide*/90 | N/A | 5/5 |
| AA12.JT (AA-12) | A* | Live | 1.97 to 24.69 | 0 | 66 | 132 | 24.69 | 60/60 | 5 | 5/5 |
| AA2.JT (AA-2) | I* | Live | 0.66 to 3.95 | 0 | 4 | 20 | 4.94 | 45/45 | N/A | 5/5 |
| AA6.JT (AA-6) | S* | Catalog | 1.97 to 14.81 | 1 | 39 | 80 | 14.81 | 45/45 | N/A | N/A |
| AA8.JT (AA-8) | I* | Live | 0.66 to 3.95 | 0 | 4 | 20 | 4.94 | 45/45 | N/A | 5/5 |
| AA9.JT (AA-9) | S* | Catalog | 1.97 to 41.15 | 2 | 130 | 264 | 41.15 | 45/45 | N/A | N/A |
| AAML.JT (AAM-L) | A* | Live | 4.94 to 74.06 | 0 | 130 | 270 | 74.06 | 45/45 | 8 | 5/5 |
| AEMP1.JT (AEMP-1) | A* | Catalog | 0.33 to 14.81 | 0 | 35 | 70 | 19.75 | 45/45 | 3 | 5/5 |
| AGM45.JT (AGM-45) | E* | Catalog | 0.00 to 9.87 | 0 | 10 | 40 | 9.87 | 45/45 | N/A | N/A |
| AGM65A.JT (AGM-65A) | I* | Catalog | 0.00 to 8.23 | 1 | 9 | 40 | 8.23 | 45/45 | N/A | 5/5 |
| AGM65G.JT (AGM-65) | I* | Live | 0.08 to 9.87 | 1 | 9 | 40 | 9.87 | 45/45 | N/A | N/A |
| AGM84A.JT (AGM-84A) | A* | Catalog | 0.08 to 59.25 | 2 | 118 | 120 | 59.25 | 45/45 | 8 | N/A |
| AGM84E.JT (AGM-84E) | I* | Catalog | 0.08 to 49.37 | 2 | 118 | 120 | 49.37 | 45/45 | N/A | 5/5 |
| AGM88.JT (AGM-88) | E* | Catalog | 0.08 to 32.92 | 0 | 25 | 40 | 32.92 | 45/45 | N/A | N/A |
| AIM120.JT (AIM-120) | A* | Live | 1.97 to 23.70 | 0 | 66 | 132 | 23.70 | 45/45 | 5 | 5/5 |
| AIM54C.JT (AIM-54) | A* | Live | 4.94 to 98.75 | 2 | 139 | 283 | 98.75 | 45/45 | 10 | 5/5 |
| AIM7.JT (AIM-7) | S* | Catalog | 1.32 to 19.75 | 2 | 51 | 106 | 19.75 | 45/45 | N/A | N/A |
| AIM7E.JT (AIM-7E) | S* | Catalog | 0.49 to 16.46 | 1 | 11 | 60 | 16.46 | 45/45 | N/A | N/A |
| AIM9B.JT (AIM-9B) | I* | Catalog | 0.66 to 3.95 | 0 | 3 | 20 | 8.23 | 45/45 | N/A | 5/5 |
| AIM9M.JT (AIM-9M) | I* | Live | 0.66 to 3.95 | 0 | 11 | 22 | 8.23 | 45/45 | N/A | 5/5 |
| AIM9X.JT (AIM-9X) | I* | Live | 0.49 to 3.95 | 0 | 15 | 24 | 8.23 | 75/75 | N/A | 5/5 |
| AM39.JT (AM-39) | A* | Catalog | 0.08 to 59.25 | 2 | 64 | 132 | 59.25 | 45/45 | 8 | N/A |
| AS14.JT (AS-14) | Hold: designator | Catalog | 0.08 to 6.58 | 1 | 9 | 30 | 8.23 | 45/45 | TBD | TBD |
| AS15.JT (AS 15) | S* | Catalog | 0.08 to 9.87 | 1 | 19 | 40 | 9.87 | 45/45 | N/A | N/A |
| AS16.JT (AS-16) | A* | Catalog | 0.08 to 6.58 | 1 | 9 | 30 | 8.23 | 45/45 | 2 | N/A |
| AS30.JT (AS 30L) | Hold: designator | Catalog | 0.08 to 9.87 | 1 | 12 | 26 | 9.87 | 45/45 | TBD | TBD |
| AS7.JT (AS-7) | S* | Live | 0.08 to 4.94 | 1 | 9 | 30 | 8.23 | 45/45 | N/A | N/A |
| ASROC.JT (ASROC) | Hold: radar role | Catalog | 0.08 to 12.34 | 2 | 20 | 40 | 16.46 | wide*/90 | TBD | TBD |
| AT12.JT (AT-12) | Hold: designator | Catalog | 0.16 to 4.28 | 3 | 97 | 180 | 4.28 | 45/45 | TBD | TBD |
| AT2.JT (AT-2) | Hold: no seeker | Catalog | 0.08 to 2.96 | 1 | 9 | 30 | 3.29 | 45/45 | TBD | TBD |
| FIM92.JT (FIM-92) | I* | Catalog | 1.23 to 8.89 | 0 | 4 | 20 | 16.46 | wide*/90 | N/A | 5/5 |
| HQ2J.JT (HQ-2J) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| HQ61.JT (HQ-61) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| MICA.JT (MICA) | A* | Live | 1.97 to 24.69 | 0 | 66 | 132 | 24.69 | 45/45 | 5 | 5/5 |
| MIM23.JT (MIM-23) | S* | Catalog | 0.16 to 7.41 | 1 | 9 | 20 | 15.43 | wide*/90 | N/A | N/A |
| MIS.JT (MIS) | S* | Catalog | 1.23 to 7.90 | 2 | 21 | 40 | 10.53 | wide*/90 | N/A | N/A |
| PL10.JT (PL-10) | S* | Catalog | 1.32 to 24.69 | 2 | 18 | 40 | 24.69 | 45/45 | N/A | N/A |
| PL7.JT (PL-7) | I* | Catalog | 0.66 to 3.95 | 0 | 4 | 20 | 8.23 | 45/45 | N/A | 5/5 |
| R440.JT (R-440) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| R530.JT (R-530D) | S* | Live | 1.32 to 16.46 | 0 | 53 | 106 | 19.75 | 45/45 | N/A | N/A |
| R550.JT (R-550) | I* | Live | 0.66 to 3.95 | 0 | 7 | 20 | 8.23 | 45/45 | N/A | 5/5 |
| ROLAND.JT (Roland) | S* | Catalog | 0.16 to 4.44 | 1 | 9 | 20 | 7.41 | wide*/90 | N/A | N/A |
| SA13.JT (SA-13) | I* | Catalog | 0.25 to 2.47 | 1 | 9 | 20 | 4.11 | wide*/90 | N/A | 5/5 |
| SA14.JT (SA-14) | I* | Catalog | 0.08 to 2.47 | 0 | 22 | 20 | 3.29 | wide*/90 | N/A | 5/5 |
| SA15.JT (SA-15) | S* | Catalog | 0.16 to 5.92 | 1 | 9 | 20 | 13.17 | wide*/90 | N/A | N/A |
| SA16.JT (SA-16) | I* | Catalog | 0.08 to 1.32 | 0 | 5 | 20 | 2.47 | wide*/90 | N/A | 5/5 |
| SA19.JT (SA-19) | Hold: radar role | Catalog | 0.08 to 3.95 | 0 | 5 | 20 | 9.87 | wide*/90 | TBD | TBD |
| SA2A.JT (SA-2A) | S* | Catalog | 1.23 to 15.64 | 2 | 21 | 40 | 24.69 | wide*/90 | N/A | N/A |
| SA3.JT (SA-3) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| SA6.JT (SA-6) | S* | Catalog | 1.48 to 12.34 | 2 | 21 | 20 | 13.17 | wide*/90 | N/A | N/A |
| SA7.JT (SA-7) | I* | Catalog | 0.08 to 1.48 | 0 | 5 | 20 | 2.47 | wide*/90 | N/A | 5/5 |
| SA9.JT (SA-9) | I* | Catalog | 0.41 to 2.96 | 0 | 10 | 20 | 3.29 | wide*/90 | N/A | 5/5 |
| SAN11.JT (SA-N-11) | Hold: radar role | Catalog | 0.25 to 3.95 | 2 | 8 | 40 | 16.46 | wide*/90 | TBD | TBD |
| SAN3.JT (SA-N-3) | S* | Catalog | 0.82 to 16.46 | 2 | 8 | 180 | 16.46 | wide*/90 | N/A | N/A |
| SAN4.JT (SA-N-4) | S* | Catalog | 0.66 to 5.76 | 2 | 8 | 40 | 16.46 | wide*/90 | N/A | N/A |
| SAN5.JT (SA-N-5) | I* | Catalog | 0.08 to 1.48 | 2 | 3 | 20 | 2.47 | wide*/90 | N/A | 5/5 |
| SAN7.JT (SA-N-7) | S* | Catalog | 0.25 to 9.87 | 2 | 8 | 40 | 16.46 | wide*/90 | N/A | N/A |
| SAN8.JT (SA-N-8) | I* | Catalog | 0.08 to 2.47 | 2 | 3 | 40 | 4.11 | wide*/90 | N/A | 5/5 |
| SAN9.JT (SA-N-9) | S* | Catalog | 0.16 to 5.92 | 2 | 8 | 40 | 13.17 | wide*/90 | N/A | N/A |
| SEA_SPAR.JT (AIM-7) | S* | Catalog | 0.49 to 19.75 | 0 | 10 | 40 | 19.75 | wide*/90 | N/A | N/A |
| SSN9.JT (SS-N-9) | S* | Catalog | 0.49 to 19.75 | 0 | 100 | 360 | 41.15 | wide*/90 | N/A | N/A |

Seeker maximum uses source `zone0`; its minimum and relative-altitude limits
also remain binding. Half-angles use 182 source units per degree. `wide*` denotes
the 0x7fff source value, whose wide-angle handling needs review, not a normal
180-degree cone. These columns describe imported geometry, not proven detection
range or completed seeker behavior. The uncued-search column contains agent-fitted
half-angle caps, separately from the imported seeker envelope. Each axis uses
the smaller of its fitted cap and its usable source limit.

## Activation and independent acquisition

The matrix's **Active-on distance** is the proposed profile attribute
`active_seeker_activation_nmi`. Measure straight-line distance in three dimensions
from the missile to its **last known intercept point**. Activate at or below that
weapon's threshold, using 6,076 feet per nmi. This is not distance traveled,
distance from the launcher or necessarily current distance to the target.
Once activated, the seeker stays on. A launch already inside the threshold starts
searching immediately. Aircraft updates can move the remembered intercept while
support exists; losing support freezes it, including for the activation test.

These are **fitted game defaults selected by the agent**, as requested by John,
not recovered FA activation distances or estimates of real-world seeker hardware.
They replace the universal five-mile draft rule. The first-pass tuning rule uses
the imported launch maximum to give shorter-range game weapons a later search
phase and longer-range weapons more search distance:

- Below 10 nmi launch maximum: activate at 2 nmi.
- From 10 to below 20 nmi: activate at 3 nmi.
- From 20 to below 40 nmi: activate at 5 nmi.
- From 40 to below 80 nmi: activate at 8 nmi.
- At least 80 nmi: activate at 10 nmi.

Use exact source range for the bands, not the rounded display value. These bands
explain this draft's choices; the explicit per-weapon matrix value is the profile
setting and can be tuned independently later. No historical era or hardware
capability is inferred. Activation distance does not replace or expand seeker
range, cone, signature or terrain checks, and does not guarantee acquisition.

Only A rows receive numeric values. S/I/E have `N/A`, meaning no active-radar
transition, not activation at zero range. Held rows have `TBD`; do not create an
activation profile until their guidance type is resolved. Validate any configured
activation distance as finite and positive. The numeric values remain conditional
on the starred guidance classifications being accepted.

For cued launch, calculate the intercept using the launch and target motion
contract below. Update it only from new observations while the same target has
valid aircraft support. Never update from hidden target movement. A failed
intercept estimate falls back to the last observed target position and shows
`EST --` in the debug window and `0%` in the HUD; it must not
invent a time-to-go.

Boresight launch overrides delayed activation: the seeker is enabled before
release and remains enabled afterward. No guessed intercept is required to fire.
The matrix's active-on distance applies to cued A launches only.

Separate `MIDCOURSE`, `ACTIVE SEARCH`, `PITBULL`, `LOST` and `EXPIRED` states.
`PITBULL` requires a successful seeker acquisition, not merely crossing the
activation distance. Cued shots search for their assigned target only. Uncued
shots may acquire one eligible contact from their own cone; after acquisition,
keep that identity and use the same loss rules. Active search continues until
guidance expiry. There is no opportunistic switching after acquisition.
After acquisition, loss uses the same MEMORY/LOST indications as I, with
same-target reacquisition allowed until guidance expiry.
Guidance expiry has priority over activation or reacquisition on the same tick.

Use each weapon's imported seeker range and horizontal/vertical half-angles,
with explicit handling for wide-angle/sentinel records. The current live cone
collapses horizontal and vertical limits; correct that before claiming the
matrix's seekers work as specified. Do not borrow the launching aircraft's cone.
Seeker observation must account for terrain masking and the relevant target
signature: shared radar aspect/RCS for A, IR signature for I, actual enabled
emissions for E. An emitter switch-off cannot be replaced by its reflective RCS.
Radar noise alone must not reveal hidden target position or identity.

Era-based seeker presets are optional **fitted** fallbacks for missing detection
or countermeasure parameters, not replacements for imported range/angle values.
No era constants or per-weapon era assignments are established in this pass.
Implement the imported geometry first; document and test numeric presets before
adding any. Radar and jammer homing eligibility must be explicit per E profile;
AGM45/AGM88 are candidates, not evidence that both can home on every jammer.

## Launch velocity and intercept estimates

**Opinionated behavior requested by John, 2026-09-17:** aircraft velocity at
release contributes to missile motion. Motor boost develops over the burn, and
observed target speed and direction affect the calculated intercept and closing
speed. This applies to actual game motion as well as HUD estimates.

Capture aircraft **world velocity as a vector**, not just speed along the nose.
A climbing or slipping aircraft must pass its upward/sideways motion to the
missile. Snapshot this once at release; later aircraft acceleration cannot change
a missile already in flight. The seeker initially points along the launch rail,
which can differ from the inherited flight direction.

The first-pass authored profile uses 100% velocity inheritance and zero additional
rail-ejection velocity. Those are agent-selected fitted values. Keep the existing
source launch-speed helper in the compatibility profile: it selects and clamps
a scalar using `launchRetard` and `initialSpeed`; it does not implement this
vector-additive rule. Do not relabel the new interpretation as recovered FA code.

For a bounded game boost estimate, use source acceleration over elapsed powered
time, capped by a fitted motor velocity-gain budget:
`max(0, altitude-adjusted source maximum speed - source initial speed)`.
The budget is a game reuse of imported values, not their recovered meaning.
Apply no gain before ignition; stop adding at burnout. Accumulate the gain once,
in the current thrust direction, while existing turn limits constrain steering.
For an unsteered estimate, missile velocity is the launch aircraft vector plus
this accumulated forward boost vector. Steering must not increase speed by
itself. After burnout, use source deceleration toward source coast speed, without
instant speed jumps or continued thrust. Keep the old absolute powered-speed
command in the compatibility profile rather than applying both laws together.

Use the same motion predictor for launch suitability, intercept, time-to-go and
in-flight movement. Predict against the latest **observed** target position and
velocity in the same coordinate frame. Account for approaching, receding,
crossing, climbing and descending targets. Bound prediction by remaining guidance
and object lifetime; show debug `EST --` and HUD `0%` when
interception is not predicted.
A target's motion changes the intercept; it never adds propulsion to the missile.

For the target directly ahead, closing speed is missile speed toward the target
minus target speed away along that line. More generally use the projection of
`missile_velocity - target_velocity` onto the missile-to-target sight line.
Cockpit aircraft-to-target closure is a separate readout using aircraft velocity.
Do not add that aircraft closure again after already inheriting aircraft speed.

Synthetic example, not a catalog weapon: aircraft speed 600 ft/s and accumulated
forward motor gain 1,000 ft/s give a straight-flight missile speed of 1,600 ft/s.
A target approaching at 300 ft/s gives 1,900 ft/s closure; receding at 300 ft/s
gives 1,300 ft/s. A perpendicular 300 ft/s target requires lateral lead, not
1,900 ft/s missile speed. Starting with an aircraft at 300 ft/s instead gives
1,300 ft/s missile speed at the same accumulated boost. A side-slip case must
retain the launch's sideways component rather than replacing it with nose heading.

## Uncued launch and narrow IR search

**Opinionated behavior requested by John, 2026-09-17.** Accepted air-to-air weapons with
an independent onboard seeker, A/I, get an explicit `BORESIGHT` mode alongside
`CUED`. Here “active internal seeker” means an enabled seeker; I and E remain
passive sensors. Supported radar S still needs launcher support. Held and
laser/designator rows gain no capability from the generic switch.

Implemented control: a rebindable `weapon-seeker-mode` action and a clickable
upper-right diagnostic mode label switch modes; no existing key is silently reassigned.
Arming an independent air-to-air missile with no designation automatically enters
BORESIGHT when guidance is enabled. IR does not require radar power.
Designating a contact returns to CUED. The explicit mode switch remains available
with a retained designation for radar missiles; selected-track priority prevents
that override for IR missiles. L and the upper-right RELEASE LOCK button clear the
aircraft designation and mounted seeker, without redirecting airborne shots.
The FA manual p. 112 targeting list does not identify a clear-designation key;
L is an existing host choice, not a recovered retail binding. Snapshot
the mode at launch; changing modes later cannot retask missiles in flight.

In BORESIGHT, an armed, operational, loaded weapon can fire without designation,
installed FLIR or seeker lock, provided radar power is on. IR searches forward on the rail
and after release. Active radar guidance searches only after release, enabling
its seeker immediately and requiring acquisition before reporting PITBULL.
A separate prelaunch estimate can identify a provisional bore contact. With a lock, retain that target; without one, fly on
and search until acquisition or guidance expiry. Keep normal station, bay, trigger
and ammunition gates. Missing target data cannot produce a fake range inhibit:
omit `IN RNG` for an uncued release. A known bore candidate inside imported `zone1.minimum_range` inhibits release
with MIN RANGE. Other envelope cues still warn without blocking uncued release.
Internal-bay opening must work without a cockpit designation.

The fitted bore is a **5-degree circular half-angle** for independent seekers,
limited by the imported seeker volume. John requested a smaller bore on 2026-09-17; the latest "5%" wording is
interpreted as five degrees. The half-angle interpretation preserves the existing
meaning of the setting. It follows the rail axis
before launch and missile nose afterward, independent of camera look and zoom.
Mounted bore IR stays inside the circle even after acquisition. Released seekers
use imported tracking limits after acquisition.
No source cone value is overwritten. Keep normal terrain, range, target-class
and signature/emission eligibility checks, including for unknown uncued contacts.

Uncued acquisition is a weapon-seeker rule authorized here, not aircraft combat
AI. Evaluate only contacts observed by that seeker. For IR, select the strongest
eligible heat-quality score; ties use smallest off-axis angle, shortest range,
then stable target ID. Active radar chooses the strongest directional radar
signature divided by `100 * (1 + (range / nominal_range)^2)`, then the same ties.
For IR and active radar, multiply selection score by
`1 - 0.75 * clamp(off_axis / bore_half_angle, 0, 1)^2`.
This agent-fitted centre preference gives the edge one quarter of the centre's
weight without making a much stronger edge return disappear.
Emitter seekers retain angle, range and ID ordering. These rules are agent-fitted.
Require 0.25 seconds of continuous eligibility to lock.
Switching a candidate restarts that dwell. Mounted bore IR can switch to a
stronger eligible return; a released missile keeps its acquired identity. During loss memory, retain the identity but show `MEMORY` instead of a
confirmed lock; suppress lock tone until reacquisition completes. Existing fitted 2-second memory applies after loss. No team label can
make a physically visible return disappear; known-friendly warnings may use
existing IFF, but unknown contacts must not gain hidden identity information.

### Fitted heat quality and tone

These initial constants are agent choices for the game, not retail measurements.
Reuse the imported IR signature, independently of radar RCS. Multiply its ratio
to reference signature 100 by an aspect factor and engine factor. Use aspect
factor 1.0 when viewing the tail, 0.5 from the side and 0.25 head-on, interpolating
between them. Engine factor is 0.1 when off, 0.5 at idle, 1.0 at full dry power
and 1.5 with afterburner; interpolate idle-to-dry with normalized throttle.
Call the result `heat`. Unknown engine state uses factor 1.0, labelled fitted;
do not substitute radar emission state. Add these observables to fixtures and
combat tapes so playback and live search agree.

Use effective IR range `nominal_range * min(1, sqrt(max(0, heat)))`.
Inside that range and the cone, quality is
`max(heat / (1 + (distance / nominal_range)^2), 0)`; otherwise it is zero.
Require quality at least 0.25 for the 0.25-second acquisition dwell, and at least
0.20 for retention. These are game thresholds, not probabilities of hitting.
Preserve independently specified rear-aspect eligibility and target classes;
a high heat score cannot bypass them. Do not infer such eligibility by filename.

Example: a full-power, reference-signature tail target at half nominal range
scores 0.8, while its head-on score is 0.2 and cannot acquire at that distance.
Afterburner raises the same head-on score to 0.3, allowing acquisition after dwell
if the weapon's aspect eligibility permits it. A masked target scores zero.

For selected, armed IR or radar missiles, the cue plays only while the seeker is
actively tracking a return; with nothing in the seeker it is silent (John,
2026-09-23). While a candidate is observed, use `15% + 55% * quality`; after
lock use `40% + 60% * quality`, with tone quality clamped to 0..1. A radar
missile in boresight plays its lock cue at `40% + 60% * quality` on its bore
return, with or without a designated target. Any radar missile falls silent
while its tracked target is inside the weapon's minimum range.
Selection scores remain unclamped so stronger returns stay distinguishable. Fade amplitude over
0.1 seconds. Playback uses user-imported `&IRTRY.5K` / `&IRLOCK.5K` and
`&RDRTRY.5K` / `&RDRLOCK.5K` for search / lock respectively. The louder lock cue
replaces the search cue. This sample assignment and looping are **agent-fitted**:
resource existence and manual tone descriptions do not establish original
playback rules. A2G IR currently shares the IR pair; its original distinct tone
is unresolved. If samples are unavailable, the existing fitted oscillator is
retained as a fallback. Safe, empty, failed stations, death, effects mute and
pause suppress the cue. Default seeker volume is 0.30, doubled for radar and IR search/lock cues at John's request on 2026-09-17.
[Sample evidence and validation](../baselines/hud-cleanup.md).


Only the selected mounted seeker produces this tone. Silence it on safe, empty,
failed station, pause, leaving flight or switching away from IR/radar missiles; pause must also
freeze acquisition timers. After firing, stop the spent seeker's tone. A newly
selected remaining round begins its own search and dwell. Do not keep sounding
lock from an airborne missile as though the next round had acquired it.

## Weapon HUD delivery

**Opinionated cleanup requested by John, 2026-09-17.** Armed missile selection
shows short weapon/count, readiness warnings and a bare `n%` hit estimate.
The later [HUD layout](hud-layout.md) moves these rows below the expanded
ladder and limits AGL/vertical speed to non-weapon ILS guidance.
John's annotated layout request reduces the HUD text and fixed layout by 15%
from the previous size (scale 0.85 to 0.7225). Angular cues retain their actual
world alignment and five-degree bore geometry. Weapon/count and percentage
start at reference x=207, aligned with the speed box's left edge. The range scale
is 52 reference pixels tall at x=390, y=230..282, inside the altitude tape.
The altitude box's left border is x=401, two pixels beyond the former tick endpoint
x=399, matching the speed-side gap. Its text starts at x=405. Tape extent and lower readout coordinates
follow the expanded [HUD layout](hud-layout.md). TARGET DESTROYED is omitted
from the HUD; its simulation release inhibit and debug status remain.
Suppress the redundant BORE READY
message, but retain release-inhibiting warnings and CUED IN RNG. Use the imported first `si_names` string for the HUD,
even when loadout menus use the second, longer description. Manual pp. 83-84
establish the readout meanings; our probability rule and layout are fitted.
Stall, engine-off and crash warnings remain visible. Safe retains flight
readouts for missile selection, with AGL/VS restricted to active ILS, and hides missile range, probability, diamond
and ARM cues. The selected target's square or off-HUD chevron is independent of
master arm, selected weapon and current radar observation; its presentation-only
selection does not supply missile support. See the shared
[gun and target-cue specification](gunsight-targeting.md). Safe guns retain their
ammunition/SAFE text but hide the firing pipper.

All weapon symbology is drawn in the same aircraft-forward HUD layer as flight
symbology. Circle, labels, estimate and range scale translate and fade together
when looking away, and zoom together. Clip in forward HUD coordinates before
applying head-look, never against a stationary screen-centred HUD rectangle.

BORE shows a blinking diamond on one provisional contact, without creating a
target box for that provisional return. A separately selected HUD target may
still have its own square or edge chevron.
It blinks at 2 Hz with 50 percent duty, using simulation ticks so pause freezes
it. This is an estimate, not an aircraft designation or seeker lock. Active radar
launches still have no preassigned target and acquire independently after release.
IR's real dwell and tone remain independent of this immediate provisional cue.
Choose the provisional contact using the same centre-weighted signal score and
stable tie rules as acquisition, within the actual bore, range and terrain gates.
No candidate means `0%` and no diamond or range scale.

When a candidate exists, show the imported minimum and engagement-dependent estimated maximum on the weapon range scale.
Its triangular target marker blinks with the provisional diamond, clamped to
an endpoint for targets outside the launch envelope. A cued, acquired radar
seeker diamond still blinks when launch-ready. Inferred bore cues never grant
launch permission or aircraft sensor support.

### Fitted estimated hit calculator

The original probability calculation is unknown. John requested a working
estimate on 2026-09-17; this is an agent-authored heuristic, not a calibrated
retail percentage or a guarantee. It scores current observation quality,
centring, launch envelope and simulated intercept margin:

- No current observation, an observation outside the launch envelope, or no
  intercept before guidance/removal expiry gives zero percent.
- `signal = clamp(observation_quality, 0, 1)`.
- In bore, `centring = 1 - 0.75 * clamp(off_axis / bore_half_angle, 0, 1)^2`.
  CUED uses 1 because its seeker is slaved to designation.
- `r = clamp((range - minimum) / (maximum - minimum), 0, 1)`;
  `envelope = 1 - 0.75 * r^2`.
- `life` is the smaller of guidance lifetime and removal time, in seconds.
  `margin = clamp(1 - 0.6 * intercept_seconds / life, 0, 1)`.
- Round `95 * signal * centring * envelope * margin` to an integer percentage.
  Cap at 95. These numeric weights are agent choices.

The intercept uses current observed velocity and the existing propulsion model,
so target closure affects the estimate. Future evasive manoeuvres, target intent,
weather and future countermeasures are not predicted. Recompute against the
same provisional contact used by the diamond and range scale. Do not alter the
missile's physics or random damage outcomes to force agreement with this cue.

Mode, actual seeker state, release button, `R`, signed `C`, `EST` flight time,
target aspect angle (`ASP`, in degrees),
and three recent shot diagnostics stay in the upper-right debug window.
Unavailable numeric debug values use `--`. A large upper-right instrument moves
down 104 reference pixels to clear that window. The forward HUD omits BORE READY and retains CUED IN RNG; estimated chance
cannot confer a lock.

Use original HUD styling and fonts. Add seeker-state readouts and deterministic
sound controls to the app boundary; do not let the renderer or audio thread choose
targets or determine lock. Update the input, flight-controls, radar and architecture
guides when these controls and behaviors are implemented, not as shipped features
in advance. No new artwork or sound assets are required for this planning pass.

## Minimum engagement and target role

John requested minimum-range enforcement and separate surface-weapon behavior on
2026-09-17. The following boundary rules are **agent-fitted**, not measured retail
behavior. Use imported `zone1.minimum_range` in feet, inclusive at equality.
A known designated or inferred bore target below that distance inhibits release.
An uncued shot with no observed candidate remains possible. Its seeker cannot
acquire, and its proximity/direct-hit test cannot damage, a target inside that
radius measured from the launch origin. A target accepted outside the boundary
keeps its eligibility while closing, including terminal flight below minR.
This is an engagement qualification, not a mandatory straight-flight distance or
an added fuze timer. Imported fuze arming time remains a separate gate.

Weapon role is explicit, separate from seeker type and target damage category.
AGM65G, AGM45, AGM88, AGM84A, AM39 and AS16 require surface targets; AS7 does too.
Other reviewed implemented profiles require airborne aircraft. Surface weapons
have no air-combat BORESIGHT mode and require designation. Aircraft that land or
are assigned another damage-test category do not become surface targets.
The manual's air-to-ground and IR sections support Maverick's surface role and
its separate lock tone. Surface IR uses imported thermal signature without
fighter exhaust aspect/throttle multipliers, an **agent-fitted** contrast model.
No surface target scene or surface designation channel is supplied by this fix;
AGM-65 remains unavailable against the practice aircraft rather than borrowing
its A2A targeting workflow. Surface seeker behavior is tested with synthetic
fixtures. Active surface support/mission integration remains deferred.

The imported AGM65G minimum is 500 feet, whereas the manual table states 0.8 nmi.
This unresolved source discrepancy needs record/unit research. Keep the reviewed
imported value; do not replace it with an invented real-world value.

## Range, motor and tracking lifetime

Keep four independent concepts:

1. **Launch envelope:** imported minimum slant range and altitude/angle limits,
   plus a predicted intercept, control cued firing permission. The imported
   nominal launch maximum does not cap the engagement estimate. BORESIGHT explicitly permits uncued
   release as specified above. Being in range does not promise a hit.
2. **Seeker envelope:** onboard acquisition/retention limits govern observations
   after launch. Losing aircraft lock does not extend them.
3. **Motor:** ignition delay, powered burn and coast are separate phases. Burnout
   changes speed/turn behavior; it does not automatically remove the missile or
   stop its seeker. The authored boost/inheritance rule above reuses imported
   acceleration and altitude values; retain powered/unpowered turn parameters.
   Keep the original speed interpretation in the compatibility profile. The fitted maneuver loss below supplements coast deceleration; it is not a
   physical drag model or a source of real-world range values.
4. **Guidance lifetime:** a separate `guidance_lifetime_s` starts at launch and
   ends steering and reacquisition permanently. Signal loss alone does not. Call this guidance lifetime in
   the UI, not battery life, since the source does not establish battery meaning.
   Agent-selected fitted fallback: equal to the source-derived removal lifetime.
   Profiles may explicitly shorten it; do not lengthen object life implicitly.
   An expired missile continues unguided until collision or cleanup. A shorter
   synthetic profile must prove that these are genuinely separate timers.

The source `trackT` is parsed but unused in live combat. Its units and purpose
are unknown here. Do not convert it into tracking duration or battery capacity.
A lock-loss memory timer is also separate from total guidance lifetime.

Do not kill a missile when its traveled distance reaches the launch maximum.
Actual reach emerges from motion, target movement, guidance and lifetime. Add
repeatable headless reach probes at launch altitude 10,000 feet and launcher
speeds 300, 600 and 900 ft/s against stationary targets and approaching, receding
and crossing fixtures at 600 ft/s. Add matched climb and side-slip launches.
Sample 25%, 50%, 75% and 100% of each weapon's launch maximum, respecting minimum
range. Report hits, misses and expiry, not a fabricated universal effective range.

For SA14 and SA6, imported removal precedes motor cutoff. Preserve that
compatibility behavior and report it; do not silently fix the source data.
The long burn values in the table are likewise the current game's timing choice.

## What exists today and what changes

At reviewed commit `60f2863`, live combat already applies launch/seeker range
gates, motor phases, coast speed, turn limits and removal. It uses fitted pursuit.
AIM120/MICA guide independently immediately after launch; R530 requires support.
There is no explicit midcourse/activation state, no total guidance timer, and
loss currently clears the target permanently without reacquisition. Onboard
steering gates do not yet use the full shared RCS detection model. The launcher
currently supplies scalar speed and attitude, not its world velocity vector;
guided release requires a designated target. That paragraph records the pre-update baseline. The current implementation
adds vector release, finite boost, seeker-owned observations, activation,
guidance lifetime and reacquisition, plus the mounted seeker, HUD and tone.
See the feature baseline for validation and remaining approximations.

The plan adds the A activation phase, seeker-owned observations, explicit
loss/reacquisition rules, a guidance timer and readable state events. Preserve
the existing weapons profile as an explicit compatibility option when adding
the opinionated activation profile. This is a proposed weapons choice, not a
change to any flight adapter. Update combat recording configuration/fingerprints
so replays cannot silently mix the old and new rules. Record launcher velocity,
launch mode, seeker observations and heat configuration needed for deterministic
playback; old tapes retain explicit compatibility semantics.

## Acceptance contract

Synthetic tests must cover all four types, per-missile target ownership, two
simultaneous shots at different targets, support loss before/after activation,
radar/IR/emitter channel separation and signal shutdown. Test activation just
outside, exactly at and just inside each configured activation distance, converted
with 6,076 feet per nmi. Cover all nine A rows, including thresholds other than
five nmi, to catch a hardcoded five-mile implementation. Activation with no visible
target must remain `ACTIVE SEARCH`. A lost aircraft track must not feed target
updates; moving the hidden target must not move the activation point. S/I/E and
held rows must not receive an active-radar transition from a default value.
Boresight A launches activate immediately even outside their cued threshold;
I/E enable their own passive seekers without emitting radar.

Check exact range/angle edges, ignition and burnout boundaries, guidance expiry,
2-second memory expiry and cleanup separately. Test reacquisition without
resetting guidance age, terrain masking, lower RCS, jammer-only emissions and
emitter filtering. Repeat at 30/60/144 render frames per second with the same
120 Hz simulation, pause/resume and record/replay. Verify missile timers beyond
the old 16-bit clock wrap without reproducing that implementation artifact.

Also validate the synthetic launch/closure examples above against actual motion
and the predictor. Compare faster/slower aircraft, head-on/tail/crossing targets,
climb, side-slip, finite burn, coast and impossible intercepts. Launcher velocity
must be inherited exactly once; target closure must never increase missile speed.

For uncued launch, test no designation, radar/FLIR absent, no target in cone,
contact entry after launch, competing heat sources, circular 5-degree bore edges,
0.25-second dwell, loss/retention thresholds and the 2-second memory boundary.
Check radar/jammer power cannot substitute for IR heat. A below-threshold or terrain-masked contact
must never acquire or produce a lock tone. Test safe/empty/failed/bay gates,
post-shot mounted-seeker reset, mode switching and the two distinct search cones.

HUD captures must show cone geometry aligned with simulated contacts at multiple
aspect ratios and zoom settings, including offscreen/clipped cones. Verify audio
by deterministic state/envelope checks and listening: muted or absent audio must
leave acquisition and HUD behavior unchanged. Include original-style cues,
estimated probability, target-free readouts and multiple shots in the visual pass.

First playable acceptance covers current A2A defaults; S uses R530 and A uses
AIM120/MICA plus the roster's other active candidates. I uses the existing A2A
stores. E uses synthetic explicitly controlled emitter fixtures until ground
weapon support is separately implemented. Keep guns, bombs, compatibility flight
paths and existing loadouts unchanged. No retail comparison is available and no
retail parity claim follows from these checks.

## Implementation decisions

Agent decisions, 2026-09-17: AGM45/AGM88 fixture profiles accept radar
transmissions only; jammer homing requires an explicit profile opt-in. This is a
fitted conservative receiver policy, not recovered band evidence. Wide 0x7fff
angles impose no limit on that axis; other axes use independent spherical angles.
Seeker radar range uses the shared RCS/aspect square-root law with reference 100;
active-seeker notch rejection uses the shared Advanced preset with a 60 ft/s
radial half-width and a 0.45 center range factor at full terrain-relative clutter.
Weapon-specific notch tuning and jammer rejection remain unspecified. The bounded intercept estimate simulates the shared propulsion, limited turns
and fitted maneuver losses at 120 Hz. Lead updates every 0.1 seconds. See
[engagement-dependent range](#engagement-dependent-maximum-range).

## Radar cursor screen boundary

**Opinionated, requested by John on 2026-09-17.** The radar crosshair replaces
the OS pointer across the entire black instrument screen, not just the inset
contact plot. In the 160 by 156 instrument raster, this is x in [11, 149) and
y in [21, 135). Lines stop at the screen edges and retain a four-pixel central
gap. The bezel and bottom buttons keep the normal pointer. This applies when
the radar is off or sensor data is unavailable too. Contact projection and
selection tolerance do not change. Scaling uses the displayed instrument bounds.

## Radar-off unguided release

**Opinionated, requested by John on 2026-09-17.** In the spec weapon rules,
radar power OFF disables radar-missile acquisition, boresight, seeker tones
and target-derived weapon cues. IR guidance remains independent of radar power. Arming follows weapon/NAV selection.
An armed radar missile may release without designation while radar power is off, with
normal safe, bay, ammunition and station gates. The HUD omits DUMB and IN RNG for that unguided release. Such a shot never acquires or steers, even if radar power returns later.
It retains normal propulsion, inherited velocity, fuze timing, minimum-engagement
and target-role damage gates. Radar power returning enables acquisition for the
next shot. Already airborne guided shots retain their own guidance rules; power
off does not retroactively turn an independent shot dumb. Compatibility rules
remain unchanged. This is a requested game rule, not a claimed retail behavior.

## IR independence and retail-reference readouts

**Opinionated revision requested by John on 2026-09-17.** Armed A2A IR missiles
use the selected cockpit track in CUED mode when one is designated. This overrides
bore candidate selection, including a stronger return in the bore circle. A radar
track supplies the identity to the IR seeker; it does not grant an IR lock or
bypass the weapon's heat, seeker-volume, terrain or launch-envelope checks.
Clearing designation returns the mounted IR seeker to BORESIGHT on the next
120 Hz simulation tick. With no designation, IR bore works even with radar power
off. The mode toggle cannot override an existing designation for IR missiles.
Released missiles retain their own target identity.
Surface missiles retain their separate designation rules. Interpret the requested
"5%" reduction as a five-degree circular half-angle, an agent interpretation.
IR bore candidates must satisfy the imported launch envelope and have a finite
intercept prediction before displaying the blinking inferred diamond. Radar bore
candidates are additionally limited by the selected scope range and the aircraft
radar's imported tracking maximum. These are prelaunch limits, not restrictions
on an already released independent seeker. Missing aircraft radar gives no radar
bore candidates. Radar-off radar missiles still release unguided, but no DUMB
label appears in the lower HUD. IR guidance and tones are independent of power.

Use the supplied retail screenshots as presentation references, not proof of
retail probability or timing formulas. ARM sits above count and short weapon
name. The percentage shares its row with IN RNG, blinking at the existing fitted
2 Hz only for an observed, release-ready contact with a positive intercept
estimate. Place the range scale just inside the altitude tape. For radar-guided
weapons with a TWS observation or inferred bore candidate, show R (nmi), C
(signed knots) and A (aspect degrees, L/R from observed motion; -- if unavailable)
below altitude. Debug retains its detailed values. The supplied images establish the desired placement; numeric geometry and blink
timing remain fitted.

## Engagement-dependent maximum range

Implementation revision requested by John on 2026-09-17. The displayed maximum
is an **agent-fitted kinematic estimate**. The imported nominal launch maximum
is not a cap. Active radar shots can be cued outside onboard seeker range and
acquire later; other guidance types remain limited by their prelaunch seeker
range. Use launcher world velocity, rail
attitude and altitude, observed target velocity, and the target's bearing and
relative altitude. Target heading affects heat/signature; target velocity supplies
its approach, crossing and recession geometry. Future maneuvers are unknown.

Predict at 120 Hz using the same motor budget, ignition/burn/coast timing, turn
limits and steering velocity rotation as live flight. Steering uses a bounded
constant-speed lead solution recomputed every 12 ticks; if no positive solution
exists, steer toward the last measured target position. Use exact limited angular
rotation, including opposite headings. The requested nose heading is the unit
vector `nose + desired_flight_path - current_flight_path`; below 1 ft/s use the
desired flight path directly. This fitted correction handles inherited slip or
climb instead of assuming nose direction equals missile velocity. Both live
flight and prediction use it. A fitted turn-energy factor of
`exp(-0.03 * angle_radians^2 / dt)` multiplies missile velocity each turn. There
is no extra maneuver loss in straight flight. This coefficient is an agent choice,
not a measured aerodynamic or retail value. Source coast deceleration remains.

The prediction succeeds when the swept relative path passes within 25 feet of
the constant-velocity target before guidance/removal expiry. This conservative
fitted tolerance is independent of actual target size and fuze radius. For max
range, keep observed bearing and target velocity fixed. Bound the search by
`(launch_speed + motor_gain_budget + target_speed) * lifetime_seconds + 25 feet`,
where lifetime ends at the earlier guidance/removal time. This conservative
travel ceiling cannot be displayed directly as reach. For non-active-radar
profiles, also cap the search at prelaunch seeker maximum. Sample 16 distances
from imported minimum to the search ceiling, then refine the outermost
successful interval with 10 bisections. Show zero if none succeeds. Refresh the
range estimate every 60 simulation ticks or immediately when contact/station
changes. Hit estimate uses that maximum and the predicted time; IN RNG requires
an actual predicted intercept. Imported minimum range, altitude and angle limits remain release constraints.
CUED MAX RANGE comes from failure to predict an intercept, not the old nominal
maximum. Actual seeker acquisition/retention volumes remain unchanged. A radar
bore search still needs a prelaunch observation inside the selected scope and
radar tracking limits.

Prediction assumes immediate usable guidance and constant target motion, and
omits terrain along the future path, future seeker loss, lofting and wind/gravity
not present in the current missile model. It does not promise a kill. Compatibility
motion stays unchanged. The 25-foot prediction tolerance and turning-loss rule
must be validated against live synthetic trajectories; do not label them retail.

## Dynamic HUD range guidance

Implementation mode, requested by John on 2026-09-17. One simulation-owned
range cue supplies imported minimum, estimated maximum, observed target range
and in-range status to the HUD scale, target marker, radar-ready diamond and
IN RNG label. In-range requires release readiness, a predicted intercept and a
target between the range endpoints. A rounded hit percentage does not determine
physical range validity. A cued target outside range retains its range marker at
the corresponding end of the scale. No target or safe state supplies no range
cue; an impossible maximum displays `--`, not a misleading zero-mile envelope.

### Favorable firing-range bars

John clarified that the ideal firing box is a highlighted band on the vertical
range scale, not a nose-steering cue. He specified two horizontal endpoint bars. Its selection rule is **agent-fitted**:
sample 17 equally spaced distances from `min + 0.10 * (estimated_max - min)` to
estimated maximum. At each distance, retain current observed bearing, velocity,
signal quality and centring. Require minimum/altitude/angle launch geometry, a simulated
intercept, and at least 70 on the existing fitted hit-estimate scale. Highlight
the longest contiguous qualifying interval, omitting isolated samples or no
qualifying interval. This reserves space above minR and below marginal reach.
It is a favorable recommendation, not a calibrated probability or no-escape zone.

Recompute with estimated maximum at 2 Hz and immediately on target/station/mode
change. Draw two six-pixel horizontal bars just inside the range axis at the upper
and lower bounds of that interval, with no connecting outline. Keep the target triangle visible, with bore blinking unchanged. No valid
observation, SAFE, radar-off radar release or impossible prediction produces no
band. The band does not change missile physics or firing permission.

[Weapon/NAV selection and no-designation bore silence](weapon-navigation-selection.md)
apply to the player controls and mounted seeker audio.

## Actor-owned launch integration

Reviewed AI missiles use the same seeker activation, acquisition, propulsion,
last-intercept memory and expiry rules as player missiles. Each supported shot
reads only its firing actor's current fire-control observation. Player support
cannot support an AI shot. AI seekers can acquire the player as target ID 0;
player seekers cannot acquire their own launcher. The explicit compatibility
weapon path retains its prior steering.

The [shared threat service](ai-awareness.md#missile-awareness-and-defense) reads
actual missile state for warning onset. Countermeasures can decoy a spec-guided
missile only while its own enabled seeker has an acquired observation of the
releasing aircraft. An inactive active-radar seeker is not decoyed by a chaff
burst. Successful decoy preserves the physical coasting body and retires its
lock; the body can remain visible.
