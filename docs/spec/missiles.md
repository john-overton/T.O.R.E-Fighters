# Missile guidance and lifetime

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-17. **Draft for implementation, no gameplay changes yet.**
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
| S: supported radar | Aircraft holds radar lock on this missile's target throughout guided flight. For future ground/ship weapons, the equivalent support comes from their launcher. | No steering from hidden target state after support loss. Proposed 2-second memory period, straight flight, then permanent guidance loss unless support returns. No pitbull. |
| A: active radar | Cued launch: snapshot the aircraft firing solution and intercept, then activate at the matrix distance. Boresight launch: own seeker searches immediately, without aircraft designation or radar lock. | Aircraft updates are optional. Losing aircraft support freezes the last supported intercept; it does not destroy the missile or expose live target coordinates. After seeker acquisition, guide independently. |
| I: infrared | Use the weapon's own heat seeker, with either a designated target or narrow forward boresight search. Aircraft radar and installed FLIR are not required for boresight search. | Proposed 2-second straight-flight memory and reacquisition of the same target, then permanent guidance loss. No radar or emitter fallback. |
| E: passive emitter | Use the weapon's receiver with designation or forward boresight search for compatible emissions. An enabled passive receiver does not transmit radar. | Emission shutdown stops measurement immediately. Proposed 2-second straight-flight memory and same-target reacquisition, then permanent guidance loss. No heat fallback. |

The 2-second memory is an **agent-selected fitted game rule**, not `trackT`.
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
| AA11.JT (AA-11) | I* | Live | 0.33 to 9.87 | 0 | 26 | 52 | 9.87 | 80/80 | N/A | 3/3 |
| AA11B.JT (AA-11B) | I* | Live | 0.25 to 9.87 | 0 | 26 | 52 | 9.87 | wide*/90 | N/A | 3/3 |
| AA12.JT (AA-12) | A* | Live | 1.97 to 24.69 | 0 | 66 | 132 | 24.69 | 60/60 | 5 | 10/10 |
| AA2.JT (AA-2) | I* | Live | 0.66 to 3.95 | 0 | 4 | 20 | 4.94 | 45/45 | N/A | 3/3 |
| AA6.JT (AA-6) | S* | Catalog | 1.97 to 14.81 | 1 | 39 | 80 | 14.81 | 45/45 | N/A | N/A |
| AA8.JT (AA-8) | I* | Live | 0.66 to 3.95 | 0 | 4 | 20 | 4.94 | 45/45 | N/A | 3/3 |
| AA9.JT (AA-9) | S* | Catalog | 1.97 to 41.15 | 2 | 130 | 264 | 41.15 | 45/45 | N/A | N/A |
| AAML.JT (AAM-L) | A* | Live | 4.94 to 74.06 | 0 | 130 | 270 | 74.06 | 45/45 | 8 | 10/10 |
| AEMP1.JT (AEMP-1) | A* | Catalog | 0.33 to 14.81 | 0 | 35 | 70 | 19.75 | 45/45 | 3 | 10/10 |
| AGM45.JT (AGM-45) | E* | Catalog | 0.00 to 9.87 | 0 | 10 | 40 | 9.87 | 45/45 | N/A | 10/10 |
| AGM65A.JT (AGM-65A) | I* | Catalog | 0.00 to 8.23 | 1 | 9 | 40 | 8.23 | 45/45 | N/A | 3/3 |
| AGM65G.JT (AGM-65) | I* | Live | 0.08 to 9.87 | 1 | 9 | 40 | 9.87 | 45/45 | N/A | 3/3 |
| AGM84A.JT (AGM-84A) | A* | Catalog | 0.08 to 59.25 | 2 | 118 | 120 | 59.25 | 45/45 | 8 | 10/10 |
| AGM84E.JT (AGM-84E) | I* | Catalog | 0.08 to 49.37 | 2 | 118 | 120 | 49.37 | 45/45 | N/A | 3/3 |
| AGM88.JT (AGM-88) | E* | Catalog | 0.08 to 32.92 | 0 | 25 | 40 | 32.92 | 45/45 | N/A | 10/10 |
| AIM120.JT (AIM-120) | A* | Live | 1.97 to 23.70 | 0 | 66 | 132 | 23.70 | 45/45 | 5 | 10/10 |
| AIM54C.JT (AIM-54) | A* | Live | 4.94 to 98.75 | 2 | 139 | 283 | 98.75 | 45/45 | 10 | 10/10 |
| AIM7.JT (AIM-7) | S* | Catalog | 1.32 to 19.75 | 2 | 51 | 106 | 19.75 | 45/45 | N/A | N/A |
| AIM7E.JT (AIM-7E) | S* | Catalog | 0.49 to 16.46 | 1 | 11 | 60 | 16.46 | 45/45 | N/A | N/A |
| AIM9B.JT (AIM-9B) | I* | Catalog | 0.66 to 3.95 | 0 | 3 | 20 | 8.23 | 45/45 | N/A | 3/3 |
| AIM9M.JT (AIM-9M) | I* | Live | 0.66 to 3.95 | 0 | 11 | 22 | 8.23 | 45/45 | N/A | 3/3 |
| AIM9X.JT (AIM-9X) | I* | Live | 0.49 to 3.95 | 0 | 15 | 24 | 8.23 | 75/75 | N/A | 3/3 |
| AM39.JT (AM-39) | A* | Catalog | 0.08 to 59.25 | 2 | 64 | 132 | 59.25 | 45/45 | 8 | 10/10 |
| AS14.JT (AS-14) | Hold: designator | Catalog | 0.08 to 6.58 | 1 | 9 | 30 | 8.23 | 45/45 | TBD | TBD |
| AS15.JT (AS 15) | S* | Catalog | 0.08 to 9.87 | 1 | 19 | 40 | 9.87 | 45/45 | N/A | N/A |
| AS16.JT (AS-16) | A* | Catalog | 0.08 to 6.58 | 1 | 9 | 30 | 8.23 | 45/45 | 2 | 10/10 |
| AS30.JT (AS 30L) | Hold: designator | Catalog | 0.08 to 9.87 | 1 | 12 | 26 | 9.87 | 45/45 | TBD | TBD |
| AS7.JT (AS-7) | S* | Live | 0.08 to 4.94 | 1 | 9 | 30 | 8.23 | 45/45 | N/A | N/A |
| ASROC.JT (ASROC) | Hold: radar role | Catalog | 0.08 to 12.34 | 2 | 20 | 40 | 16.46 | wide*/90 | TBD | TBD |
| AT12.JT (AT-12) | Hold: designator | Catalog | 0.16 to 4.28 | 3 | 97 | 180 | 4.28 | 45/45 | TBD | TBD |
| AT2.JT (AT-2) | Hold: no seeker | Catalog | 0.08 to 2.96 | 1 | 9 | 30 | 3.29 | 45/45 | TBD | TBD |
| FIM92.JT (FIM-92) | I* | Catalog | 1.23 to 8.89 | 0 | 4 | 20 | 16.46 | wide*/90 | N/A | 3/3 |
| HQ2J.JT (HQ-2J) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| HQ61.JT (HQ-61) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| MICA.JT (MICA) | A* | Live | 1.97 to 24.69 | 0 | 66 | 132 | 24.69 | 45/45 | 5 | 10/10 |
| MIM23.JT (MIM-23) | S* | Catalog | 0.16 to 7.41 | 1 | 9 | 20 | 15.43 | wide*/90 | N/A | N/A |
| MIS.JT (MIS) | S* | Catalog | 1.23 to 7.90 | 2 | 21 | 40 | 10.53 | wide*/90 | N/A | N/A |
| PL10.JT (PL-10) | S* | Catalog | 1.32 to 24.69 | 2 | 18 | 40 | 24.69 | 45/45 | N/A | N/A |
| PL7.JT (PL-7) | I* | Catalog | 0.66 to 3.95 | 0 | 4 | 20 | 8.23 | 45/45 | N/A | 3/3 |
| R440.JT (R-440) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| R530.JT (R-530D) | S* | Live | 1.32 to 16.46 | 0 | 53 | 106 | 19.75 | 45/45 | N/A | N/A |
| R550.JT (R-550) | I* | Live | 0.66 to 3.95 | 0 | 7 | 20 | 8.23 | 45/45 | N/A | 3/3 |
| ROLAND.JT (Roland) | S* | Catalog | 0.16 to 4.44 | 1 | 9 | 20 | 7.41 | wide*/90 | N/A | N/A |
| SA13.JT (SA-13) | I* | Catalog | 0.25 to 2.47 | 1 | 9 | 20 | 4.11 | wide*/90 | N/A | 3/3 |
| SA14.JT (SA-14) | I* | Catalog | 0.08 to 2.47 | 0 | 22 | 20 | 3.29 | wide*/90 | N/A | 3/3 |
| SA15.JT (SA-15) | S* | Catalog | 0.16 to 5.92 | 1 | 9 | 20 | 13.17 | wide*/90 | N/A | N/A |
| SA16.JT (SA-16) | I* | Catalog | 0.08 to 1.32 | 0 | 5 | 20 | 2.47 | wide*/90 | N/A | 3/3 |
| SA19.JT (SA-19) | Hold: radar role | Catalog | 0.08 to 3.95 | 0 | 5 | 20 | 9.87 | wide*/90 | TBD | TBD |
| SA2A.JT (SA-2A) | S* | Catalog | 1.23 to 15.64 | 2 | 21 | 40 | 24.69 | wide*/90 | N/A | N/A |
| SA3.JT (SA-3) | S* | Catalog | 1.23 to 8.89 | 2 | 21 | 40 | 16.46 | wide*/90 | N/A | N/A |
| SA6.JT (SA-6) | S* | Catalog | 1.48 to 12.34 | 2 | 21 | 20 | 13.17 | wide*/90 | N/A | N/A |
| SA7.JT (SA-7) | I* | Catalog | 0.08 to 1.48 | 0 | 5 | 20 | 2.47 | wide*/90 | N/A | 3/3 |
| SA9.JT (SA-9) | I* | Catalog | 0.41 to 2.96 | 0 | 10 | 20 | 3.29 | wide*/90 | N/A | 3/3 |
| SAN11.JT (SA-N-11) | Hold: radar role | Catalog | 0.25 to 3.95 | 2 | 8 | 40 | 16.46 | wide*/90 | TBD | TBD |
| SAN3.JT (SA-N-3) | S* | Catalog | 0.82 to 16.46 | 2 | 8 | 180 | 16.46 | wide*/90 | N/A | N/A |
| SAN4.JT (SA-N-4) | S* | Catalog | 0.66 to 5.76 | 2 | 8 | 40 | 16.46 | wide*/90 | N/A | N/A |
| SAN5.JT (SA-N-5) | I* | Catalog | 0.08 to 1.48 | 2 | 3 | 20 | 2.47 | wide*/90 | N/A | 3/3 |
| SAN7.JT (SA-N-7) | S* | Catalog | 0.25 to 9.87 | 2 | 8 | 40 | 16.46 | wide*/90 | N/A | N/A |
| SAN8.JT (SA-N-8) | I* | Catalog | 0.08 to 2.47 | 2 | 3 | 40 | 4.11 | wide*/90 | N/A | 3/3 |
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
`NO SOLUTION`; it must not invent a time-to-go or a hit probability.

Boresight launch overrides delayed activation: the seeker is enabled before
release and remains enabled afterward. No guessed intercept is required to fire.
The matrix's active-on distance applies to cued A launches only.

Separate `MIDCOURSE`, `ACTIVE SEARCH`, `PITBULL`, `LOST` and `EXPIRED` states.
`PITBULL` requires a successful seeker acquisition, not merely crossing the
activation distance. Cued shots search for their assigned target only. Uncued
shots may acquire one eligible contact from their own cone; after acquisition,
keep that identity and use the same loss rules. Active search continues until
guidance expiry. There is no opportunistic switching after acquisition.
After acquisition, loss uses the same proposed 2-second memory rule as I.
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
and object lifetime; show `NO SOLUTION` when interception is not predicted.
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

**Opinionated behavior requested by John, 2026-09-17.** All accepted weapons with
an independent onboard seeker, A/I/E, get an explicit `BORESIGHT` mode alongside
`CUED`. Here “active internal seeker” means an enabled seeker; I and E remain
passive sensors. Supported radar S still needs launcher support. Held and
laser/designator rows gain no capability from the generic switch.

Agent-proposed control: a rebindable `weapon-seeker-mode` action and a clickable
HUD mode label switch modes; no existing key is silently reassigned. Default to
CUED. Retain the cockpit designation separately, so entering BORESIGHT ignores
it without deleting it, and returning to CUED restores its normal use. Snapshot
the mode at launch; changing modes later cannot retask missiles in flight.

In BORESIGHT, an armed, operational, loaded weapon can fire without designation,
aircraft radar, installed FLIR or seeker lock. Its own seeker searches forward on
the rail and after release. With a lock, retain that target; without one, fly on
and search until acquisition or guidance expiry. Keep normal station, bay, trigger
and ammunition gates. Missing target data cannot produce a fake range inhibit:
show `BORESIGHT READY`, not `IN RNG`. Known too-close/out-of-envelope cues warn but
do not prevent this explicitly uncued release; seeker and fuze gates still apply.
Internal-bay opening must work without a cockpit designation.

The matrix's fitted boresight search caps are **3 degrees horizontal and vertical
for IR**, and **10 degrees each for A/E**, limited by the imported seeker volume.
These are half-angles, making the IR search six degrees wide. The center follows
the weapon's rail axis before launch and missile nose afterward, not the cockpit
camera or aircraft velocity vector. It can be narrower than the subsequent
tracking envelope. Once acquired, use the weapon's imported tracking limits.
No source cone value is overwritten. Keep normal terrain, range, target-class
and signature/emission eligibility checks, including for unknown uncued contacts.

Uncued acquisition is a weapon-seeker rule authorized here, not aircraft combat
AI. Evaluate only contacts observed by that seeker. For IR, select the strongest
eligible heat-quality score; ties use smallest off-axis angle, shortest range,
then stable target ID. For A/E, prefer smallest off-axis angle among eligible
seeker returns, then range and stable ID. Both selection rules are agent-fitted.
Require 0.25 seconds of continuous eligibility to lock.
Switching a candidate restarts that dwell; once locked, never jump to a stronger
contact. During loss memory, retain the identity but show `MEMORY` instead of a
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
`clamp(heat / (1 + (distance / nominal_range)^2), 0, 1)`; otherwise it is zero.
Require quality at least 0.25 for the 0.25-second acquisition dwell, and at least
0.20 for retention. These are game thresholds, not probabilities of hitting.
Preserve independently specified rear-aspect eligibility and target classes;
a high heat score cannot bypass them. Do not infer such eligibility by filename.

Example: a full-power, reference-signature tail target at half nominal range
scores 0.8, while its head-on score is 0.2 and cannot acquire at that distance.
Afterburner raises the same head-on score to 0.3, allowing acquisition after dwell
if the weapon's aspect eligibility permits it. A masked target scores zero.

For the selected mounted IR weapon, play a low search growl at 15% of configured
seeker volume. While a candidate is observed, use `15% + 55% * quality`; after
lock use `40% + 60% * quality`. Fade over 0.1 seconds to avoid clicks. The HUD
changes to `IR LOCK` on acquisition; tone amplitude expresses quality, not hit
chance. Use a separate lock timbre where a profile specifies A2G IR. Exact retail
sample mapping is unknown: inspect imported audio resources and verify by
listening, or label a temporary authored cue fitted. Never infer a sound mapping
from its filename alone or commit retail audio.

Only the selected mounted seeker produces this tone. Silence it on safe, empty,
failed station, pause, leaving flight or switching away from IR; pause must also
freeze acquisition timers. After firing, stop the spent seeker's tone. A newly
selected remaining round begins its own search and dwell. Do not keep sounding
lock from an airborne missile as though the next round had acquired it.

## Weapon HUD delivery

Implement the manual-supported cues above and the following **authored additions**
from the same simulation-owned weapon state used by firing and seeker logic:

| Cue | Planned rule |
| --- | --- |
| CUED / BORESIGHT | Always identify launch mode for an eligible selected weapon; changing it never changes an existing missile. |
| Search-cone outline | Project the actual mode's angular limits around the rail axis onto the HUD. BORESIGHT uses the matrix caps. Clip to the HUD viewport; camera FOV or zoom must not change the physical search volume. The fixed aiming reticle alone is not a cone boundary. |
| SEARCH / ACQUIRING / IR LOCK | Give non-audio feedback for the same acquisition state. Never draw a lock onto an unobserved contact. IR LOCK remains distinct from a good launch solution. |
| Reach estimate | Keep source min/max envelope cues separate from the fitted motion-based reach and time-to-go. Label the latter `EST`; omit unavailable estimates rather than drawing a plausible number. |
| Missile state | Use stable shot identity for MIDCOURSE, ACTIVE SEARCH, PITBULL, LOST and EXPIRED. Include motor phase and remaining guidance time in the weapon detail display. Multiple shots must not overwrite the mounted seeker's state. |

Treat the manual's probability cue as a separately sourced task: its complete
calculation is unknown. Do not display heat quality or a range ratio as hit
probability. Until a meaningful game probability model is specified and validated,
show `P HIT --` and the explicit solution/lock cues. Keep friendly warnings separate
from seeker physics. Validate original symbol geometry against manual figures
and available HUD resources during implementation; this pass reviewed text only.

Use original HUD styling and fonts. Add seeker-state readouts and deterministic
sound controls to the app boundary; do not let the renderer or audio thread choose
targets or determine lock. Update the input, flight-controls, radar and architecture
guides when these controls and behaviors are implemented, not as shipped features
in advance. No new artwork or sound assets are required for this planning pass.

## Range, motor and tracking lifetime

Keep four independent concepts:

1. **Launch envelope:** imported minimum/maximum slant range and altitude/angle
   limits control cued firing permission. BORESIGHT explicitly permits uncued
   release as specified above. Being in range does not promise a hit.
2. **Seeker envelope:** onboard acquisition/retention limits govern observations
   after launch. Losing aircraft lock does not extend them.
3. **Motor:** ignition delay, powered burn and coast are separate phases. Burnout
   changes speed/turn behavior; it does not automatically remove the missile or
   stop its seeker. The authored boost/inheritance rule above reuses imported
   acceleration and altitude values; retain powered/unpowered turn parameters.
   Keep the original speed interpretation in the compatibility profile. No new
   physical drag model or real-world range values are part of this plan.
4. **Guidance lifetime:** a separate `guidance_lifetime_s` starts at launch and
   ends steering and reacquisition permanently. Call this guidance lifetime in
   the UI, not battery life, since the source does not establish battery meaning.
   Agent-proposed fitted fallback: equal to the source-derived removal lifetime.
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
guided release requires a designated target. These are implementation gaps for
the launch and boresight rules above, not completed features.

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
contact entry after launch, competing heat sources, exact 3-degree IR edges,
0.25-second dwell, loss/retention thresholds and the 2-second memory boundary.
Check radar/jammer power cannot substitute for IR heat. A below-threshold or terrain-masked contact
must never acquire or produce a lock tone. Test safe/empty/failed/bay gates,
post-shot mounted-seeker reset, mode switching and the two distinct search cones.

HUD captures must show cone geometry aligned with simulated contacts at multiple
aspect ratios and zoom settings, including offscreen/clipped cones. Verify audio
by deterministic state/envelope checks and listening: muted or absent audio must
leave acquisition and HUD behavior unchanged. Include original-style cues,
unknown probability, target-free readouts and multiple shots in the visual pass.

First playable acceptance covers current A2A defaults; S uses R530 and A uses
AIM120/MICA plus the roster's other active candidates. I uses the existing A2A
stores. E uses synthetic explicitly controlled emitter fixtures until ground
weapon support is separately implemented. Keep guns, bombs, compatibility flight
paths and existing loadouts unchanged. No retail comparison is available and no
retail parity claim follows from these checks.
