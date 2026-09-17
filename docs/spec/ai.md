# Aircraft and surface AI behavior

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-17. This is the main behavior specification for AI work.
It records the behavior established so far and the inputs needed to reproduce
it. It is partial, not a claim that the complete AI is recovered or implemented.
The scope covers [all twelve ported aircraft](ai-experience.md#currently-ported-aircraft),
the wider aircraft families and surface objects. Experience values and family
bindings have one home in [AI experience](ai-experience.md).

Evidence locations are indexed by the behavior IDs below in the
[source map](../formats/ai.md#behavior-source-points). Build identities and
inspection results live in the [baseline](../baselines/ai-research.md).
Sequencing and completion criteria remain in [M1e](../ROADMAP.md#1e-ai).

## Evidence and implementation status

**Executable-confirmed** means the relevant arithmetic or condition was
inspected in the hashed FA executable. **Source-confirmed** means an active
statement in the shipped AI text establishes the rule. Source confirmation
does not claim full agreement with the compiled BI. Comments, disabled blocks
and suggestive names are not behavior evidence. Both are research evidence
for future spec-derived code, not instructions to copy original internals.

The API below is an **opinionated agent proposal**. No signatures or AI modules
have been implemented. No new fitted gameplay rule is selected in this pass.
Unknown behavior must remain visibly unresolved until researched or deliberately
specified as fitted/opinionated. Runtime hookup remains a later stage.

## The information an aircraft uses

These quantities describe an assigned target, not a rule that gives the AI
knowledge of every object. Acquisition, tracking persistence and information
sharing are separate, still incomplete contracts.

| ID | Established input meaning | Evidence and remaining limits |
| --- | --- | --- |
| B01 | Target ahead means the larger of absolute heading-to-target error and pitch-to-target error is strictly less than 90 degrees. Off-beam is that larger error, expressed in degrees. | Executable-confirmed. This is not a circular cone measured by one 3D vector angle. |
| B02 | Target facing means its absolute horizontal bearing error toward this aircraft is at most 90 degrees. | Executable-confirmed. The evaluator uses the heading output, not the target's pitch error; equality differs from B01. |
| B03 | Target distance is spatial separation; horizontal distance removes altitude separation. Own `alt` is height above the queried surface. | Executable-confirmed, with the existing fixed8 feet contract. Keep AGL and absolute altitude distinct. Terrain/object query eligibility is not fully recovered. |
| B04 | Climbing is permitted by the script predicate when current scalar speed is at least current minimum speed plus 75. Better-speed means own maximum speed exceeds the target's by at least 75. | Executable-confirmed in the source speed domain. Conversion to the host speed API and equipment/altitude dependence of the maximum-speed query need closure before calibrated acceptance. |
| B05 | Better thrust-to-weight means the own-minus-target performance evaluator is at least 10. `corner` and `cornerSpeed` return the same aircraft query. | Executable-confirmed. The thrust-to-weight scale is not established here; do not interpret 10 as a physical ratio of ten or an invented ten-percent bonus. |
| B06 | The fighter can distinguish human-controlled targets, fighter versus other aircraft, wing combat/approach state, and its own resolved experience. | Source use and executable accessors inspected. Human-control accessor reads the target's control flag. Network/control ownership and wing-state production remain open. |

Proposed host inputs must distinguish observed target pose from permitted
classification/performance metadata. FA's relative-performance evaluators read
the assigned target's aircraft state directly. The host may eventually expose
equivalent information through an explicit retail-awareness policy; this pass
does not establish how the target first became known. The current sensor service
is a reusable host component with its own documented authored detection rules.

## Fighter and strike behavior

The following rules apply to the shared family used by all twelve current
ports. They describe requested maneuvers. Achieved trajectories depend on the
aircraft's flight limits and on steering rules still being recovered.

### B10: Idle, hit and missile events

The source has separate reasons for nothing, evade, attack, radar launch,
infrared launch and hit. Nothing and hit end this fighter script invocation.
They do not prove that the aircraft stops moving, loses its route or ignores
damage. Default event/route behavior is outside those handlers.

On an infrared-launch reason, request a climb to 90-degree pitch at current
heading and maximum speed, with bank unconstrained. A 50% choice also sends a
wingman break request of +90 heading and +90 pitch. The comment calls this a
break toward the sun, but the active statements do not inspect sun position.
Do not add a sun-tracking dependency from that comment.

On a radar-launch reason, if target off-beam is at most 90 degrees, turn toward
the target bearing at the engagement pitch and corner speed. Otherwise choose
current heading plus or minus 90 degrees with equal probability. Both branches
send a wingman break request. The engagement-pitch evaluator is not simply the
line-of-sight pitch; it depends on relative altitude and aircraft limits.

These are source-confirmed choices after the engine supplies the reason.
Threat sensing, warning eligibility, the identity represented by the event's
target, and interruptions of an existing maneuver remain unknown. Device
release is a separate [experience-dependent response](ai-experience.md#other-experience-effects).

### B11: Evasion and last-ditch defense

For a non-aircraft target, the fighter source requests unchanged heading,
maximum speed and an altitude operand equal to the larger of current AGL
altitude and 20000. Do not describe this as a settled 20000-foot AGL climb:
the source mixes an AGL read with a command whose altitude frame still needs
confirmation.

Against aircraft, coordinated escape requires a target behind and facing,
wing-combat state and wing-approach state. A preliminary 70% skip leaves 30%
for this branch when all its conditions hold. The remaining choice is equally
split among cross-turn, left/right split and high/low split. Exact coordination
geometry and membership must be recovered from the wing services.

A target behind, facing, within 5000 feet and with heading difference at most
35 degrees enters last-ditch selection. Candidate actions include split-S,
horizontal scissors, reverse, overshoot, loop and horizontal/vertical jinks.
Some candidates redraw when altitude, speed or geometry makes them unsuitable.
The branch thresholds therefore are not unconditional maneuver frequencies.
At 30000 feet or more, the ordinary fallback requests flight away from the
target at maximum speed rather than jinking. Below that distance it selects a
vertical jink. All of these decisions are source-confirmed; the maneuvers'
complete shapes and interruption rules are still open.

### B12: Approach, tactical choice and pursuit

Air-to-air entry raises requested wing horizontal spacing to 5000 if it is
smaller. Spacing is in feet; B43 records the reviewed wing consumer and remaining geometry.
The source attempts wing splits only against an ahead fighter with nonzero
wing-approach value in the inclusive range 5000 through 30000. That value's
producer is unresolved; do not replace it with ordinary target distance yet.

A special approach to a human-controlled target requires separation from 5000
through 20000 feet, off-beam at most 20 degrees, heading difference at least
155 degrees and pitch difference at most 25 degrees. The `random 100 > 75`
skip means a nominal 76% entry threshold, not 75%. It then chooses offset pass
or overhead pass equally. This is source-confirmed; resulting pass geometry
requires the maneuver consumers.

After earlier approach choices, separation greater than 15000 feet selects
pursuit directly. Otherwise ahead/facing geometry feeds the
[experience-dependent tactical choices](ai-experience.md#fighter-tactical-choices).
The branch called best attack ordinarily selects pursuit. Against a fighter
behind and facing within 2000 feet, heading difference at most 35 degrees and
a 75% gate can instead choose last-ditch defense. The comment promising repeated
fast passes for skilled energy fighters does not establish such a behavior.

Pursuit requests target-relative offsets with the experience-dependent
variation already specified. Negative `homepos` speed operands select the distance-regulating rule in B15.
They encode a desired separation, not negative airspeed or a closure rate.

### B13: Basic maneuvers and command limits

Straight flight requests current heading, engagement pitch and maximum speed.
Straight climb requests +45-degree pitch, falling back to straight flight if
B04 rejects climbing. Straight dive requests -45 degrees and falls back to
straight flight below 3000 feet AGL. Left/right breaks request current heading
minus/plus 170 degrees, engagement pitch and corner speed. A turnaround chooses
one of those heading changes with equal probability. These choices are
source-confirmed, not evidence of instantaneous turns.

Executable-confirmed command input handling wraps heading into 0 through
359 degrees, bounds explicit bank to -180 through +180 degrees and supports an
unconstrained-bank mode. Motion construction bounds requested pitch to
-90 through +90 degrees. Ordinary positive speed requests are constrained by
minimum/maximum speed queries. The duration operand is clamped to 0 through
15; zero takes a geometric-completion path rather than meaning no action.
Nonzero duration is nominal simulation seconds, evaluated on a quarter-second
clock. A duration of 5 adds 20 quarter-second counts to the sampled clock;
expiration is eligible when the clock reaches that deadline. Clock phase can
make eligibility up to just under a quarter-second earlier than five seconds
after submission, and service scheduling can delay observation. This is a
simulation-time rule, not a wall-clock promise under pause or time compression.
Long-session deadline saturation remains an original edge case described in
the source notes, not a requirement to copy the timer representation.

Zero-duration motion chooses one completion axis from heading, pitch and, when
constrained, bank, using angular differences and aircraft rate queries. It does
not wait for all three axes to arrive. The ordinary selected-axis predicate
requires equality; pitch has additional early completion paths. In the reviewed
combat-state path, requested pitch above 25 degrees can finish when scalar speed
is at most minimum speed plus 25. This is separate from B04's entry permission
for a climb. Terrain avoidance and noncombat-state exceptions still need a
complete contract. Threat interruption and route resumption remain open.

### B14: Surface attack and egress

The fighter source uses the ordinary straight attack run after a 25% alternative
selection fails. The ordinary run approaches until horizontal separation is
at most 2000 feet or the target ceases to satisfy the source target predicate.
The alternative branch can choose dive-bombing when altitude and horizontal
distance are each at least `max(10000 feet, twice turn radius)`, behind a 33%
gate. Otherwise it chooses a fast-high pass one-third of the time and pop-up
two-thirds of the time.

Pop-up approaches with a target-offset vertical operand of 500 until within
10000 feet, requests +25-degree angular offset, then turns toward the target
until within 1500 feet or the target is no longer ahead/valid. The fast-high
pass uses a vertical offset operand of 5000 and exits its approach within
2000 feet or when the target is no longer ahead/valid. Its comment assumes
air-to-ground missiles, but its weapon-availability check is commented out.
Do not claim that this script guarantees a compatible weapon before choosing
that maneuver. Actual firing eligibility belongs to weapon services.

Egress uses a jinking phase while within 10000 horizontal feet, then a climbing
departure until at least `max(30000 feet, twice turn radius)` away, before
turning back. This is horizontal separation, not a 30000-foot altitude goal.
B15 establishes the shared target-offset frame. Dive-bomb rollout, jink path
and complete attack trajectories remain open.

## B15: Pursuit reference frame and speed regulation

Executable-confirmed for `homepos` with a valid target and positive duration.
Its three offset operands are distances in feet. The horizontal pair rotates
with target heading, while the vertical component remains world-vertical.
Target pitch and bank do not rotate that offset into a full aircraft body frame.
The point used for steering can first be adjusted by the selected weapon's
prediction helper for a hostile target. B44 records the recovered lead gates
and the remaining speed-estimator boundary; pure position pursuit is insufficient.

The negative speed operand's magnitude is a desired spatial separation from
the target's unoffset position. For example, `-750` requests regulation around
750 feet, independently of the offset steering point. Let `e` be actual spatial
separation minus that desired separation, and `v` be the target's scalar speed,
with negative target speed treated as zero. When both heading and pitch errors
to that target position are at most 90 degrees, choose:

| Separation error `e`, feet | Requested speed before aircraft limits |
| --- | --- |
| `e >= 5000` | Own maximum speed |
| `1000 < e < 5000` | `v + 100` |
| `250 < e <= 1000` | `v + 50` |
| `50 <= e <= 250` | `v + 25` |
| `-50 <= e < 50` | `v` |
| `-100 <= e < -50` | `v - 25` |
| `e < -100` | `v - 50` |

The speed increments above remain in the recovered scalar-speed domain;
physical conversion must be closed before the host treats them as knots or
feet/second. If either angular error is greater than 90 degrees, the request
instead uses corner speed. The ordinary regulating branch applies own maximum
and minimum speed queries, with a separate aircraft-state exemption from the
minimum. Achieved acceleration and turning remain properties of the motion
consumer, not instantaneous changes implied by this table.

The offset representation loses some precision at larger magnitudes. That
encoding is recorded in source notes; it is not a requirement to implement
packed offsets in Rust. Lateral/longitudinal sign conventions, weapon lead,
invalid-target continuation and the minimum-speed exemption need closure before
full trajectory acceptance.

## B20: Other aircraft families

These are active source behaviors, not acceptance of newly flyable aircraft.
Family binding and catalog counts remain in the experience spec.

| Family | Behavior present | Inputs and missing contract |
| --- | --- | --- |
| F-117 | Shares fighter air combat; ground pass requests a 5000 vertical offset and speed operand 300 while target remains ahead/valid | Target pose, maximum/minimum speed and attack path; offset axes and release timing remain open |
| Helicopter | Shares IR/radar response entry, distinct pursuit and pop-up attack; missile response turns toward a target with off-beam below 90 degrees, otherwise chooses a +/-90-degree break | Helicopter motion limits and target geometry; do not reuse fighter aerobatic trajectories |
| Bomber | Aircraft attack handler exits; ground attack uses a high pass; IR/radar reactions choose a +/-90-degree break | Target type, altitude, available stores, wing state and evasion limits; exit does not prove that defensive weapons cannot fire |
| AC-130 | Ground attack repeatedly requests a circle with speed halfway between corner and maximum | Target pose and orbit parameters; six circle-argument meanings and weapon firing sector remain unresolved |
| Large/airliner | Handles hit and evade; hit requests a downward pitch from 0 through -24 degrees and speed operand 2000 | Damage event and aircraft limits; positive speed clamp prevents assuming literal 2000-unit achieved flight |
| MOTH | Fighter-derived air behavior with distinct ground attack and shorter egress operands | Exact special-aircraft configuration and maneuver consumers; no ordinary aircraft alias |

## B30: Surface behavior

SARAN's hydrofoil program attacks ships only and has the approach/depart/turn
sequence in the [experience spec](ai-experience.md#wider-retail-behaviour-coverage).
Other surface behavior is primarily engine-side. `_GVProc` dispatches to a
surface event service and the shared non-player weapon service. Static objects,
carriers, deck personnel and ejectees also have distinct bindings. This proves
that copying the hydrofoil script cannot supply all surface AI.

Proposed surface inputs are mission role/order, route, speed/turn limits,
terrain or water constraints, damage, side, resolved ground experience,
observations, mount arcs, weapon inventory and supported target classes.
SAM, AAA, moving vehicles, ships and carriers need separate contracts for
target eligibility, tracking, firing and movement. Exact engagement ranges,
salvos, crew reaction delays, terrain avoidance and experience effects are
unknown in this pass. Do not apply aircraft experience tables to these objects
without a demonstrated consumer.

## B40: Weapons, orders and lifecycle boundaries

The aircraft scripts request motion and wing actions; the non-player weapon
service separately evaluates and services weapons. Target selection, emission
control, gun lead, missile release, support, countermeasure consumption and
rearming are not established by a maneuver name. An attack request may result
in movement without a legal shot.

A controller needs feedback for invalid/lost targets, unavailable weapons,
failed or completed movement, damage, fuel state and superseded orders. The
host already models several weapon inhibition reasons. Route following,
takeoff/recovery, formation rejoin and low-fuel disengagement have identified
source entry points but do not yet have complete AI contracts. No distance,
fuel percentage or priority is invented here.

## B41: Target retention, eligibility and ranking

An aircraft's weapon target selector can retain an existing valid target at
strictly less than 20000 feet spatial separation when its special retarget-policy
flag is clear. At exactly 20000 feet it proceeds to selection. This is a
retention shortcut, not a maximum sensor range or maximum missile range.
The corresponding non-aircraft actor shortcut also checks terrain blocking.
Aircraft still have a blocking check in the later firing service, so retained
target identity must not be treated as permission to shoot through terrain.
The meaning and producers of the special policy flag remain unresolved.

Candidate selection excludes self, invalid objects and disallowed object types,
then evaluates reaction policy and available seeker eligibility. Reaction
rejects same-side candidates and depends on target class, mission masks and
usable weapon inventory. This establishes equipment-dependent eligibility;
it does not establish that every candidate is a currently visible sensor contact.
The exact seeker detection, information-sharing and reacquisition rules remain
open. A separate policy route can select a mission/defense-priority candidate
without the ordinary distance ranking below.

For an ordinary eligible candidate, the aircraft selector minimizes a score
starting at spatial distance. It adds 10000 feet for each applicable condition:

- The candidate is not an aircraft.
- At least one member of the same wing is already attacking it.
- The existing wing attacker count meets or exceeds the assignment allowance.

The last two penalties can both apply. They discourage concentration of attacks;
they do not make an occupied target ineligible. The allowance helper can return
1, 2 or 100 according to policy flags, but the mission-facing meanings of those
flags still need tracing. Equal scores do not displace the current best candidate
in this pass. Do not expose original enumeration order as a gameplay requirement.
Surface actors have additional ranking terms and special zone behavior. The
three-penalty aircraft rule must not silently become their full selector.

## B42: Weapon preparation, search cadence and firing

Executable-confirmed service boundaries distinguish target search, preparation,
lock checking, firing and reload. A valid hostile target, an available compatible
station, weapon-specific lock checks and an unblocked firing path precede the
reviewed firing branch. Losing a target sends the service back toward search.
A failed lock causes a nominal one-second retry; no suitable station causes a
nominal two-second retry. Pre-firing service states can add half a second to a
chosen delay with a 10% gate. These are eligible service times, subject to the
clock and scheduling limits in B13, not a guaranteed time to the first shot.

The imported NPC fields supply search and initial preparation delays. Among the
current twelve PT records, the no-target search delay is:

| Aircraft records | Nominal no-target retry |
| --- | --- |
| F18.PT, RAFALE.PT, F14.PT, F31.PT, SU25.PT | 3 seconds |
| A4E.PT, MIG29.PT, SU27.PT, MIG21.PT, MIG23.PT, SU35.PT, F22.PT | 5 seconds |

All twelve provide nominal preparation delays of 5 seconds for the ordinary
branch and 8 seconds for the branch selected by the unready flag. The flag's
producer and interruptions remain open; these are not experience levels.
A successful lock uses the weapon's own tracking delay before the firing state.
The service also maintains a 15-second preparation/lock window in the reviewed
transitions. This is not a universal missile cooldown or target-loss timeout.

Burst size, within-burst spacing, reload and startup pacing come from the
selected projectile and NPC policy, rather than one shared fighter firing rate.
The reviewed firing service invokes the shared launch service and subsequently
checks whether the station still resolves, so depleted stores can change its
state. B45 specifies ordinary ammunition accounting and the reviewed envelope
and support gates. Per-store profiles, full gun lead, burst randomization,
in-flight support transitions and missile limits remain in research.
No single fixed gun range or unlimited-ammunition behavior is implied.

API consequence: supply a typed weapon profile, compatible-station availability,
lock/blocking results, preparation status and per-station feedback. Search and
preparation clocks belong to persistent actor/service state. Do not redraw or
restart them on every 120 Hz call. The imported timing profile remains distinct
from aircraft experience, flight capability and mission role.

## B43: Wing commands and formation variation

The script's approach command requires the sender to be the wing leader and
at least one wingman to exist. It compares the first wingman's target with the
sender's current target, requests target assignment when they differ, then sends
the approach parameters. Break and approach parameters are bounded heading-offset
and pitch requests, delivered through wing events. This establishes a command
and recipient-state dependency, not instantaneous movement or guaranteed obedience.
The receiver behavior established in this pass is specified in B46; radio
acknowledgment and broader order handling remain separate questions.

Horizontal and vertical spacing requests are clamped to 512 through 20000 feet
and the reviewed script setters act only for a wing leader. Formation motion
combines a formation-table position scaled by those spacings with small changing
offsets. Its offset draws are -15 through +14 feet in the first component and
-50 through +49 feet in the other two components before position packing. The
next variation deadline advances by a randomly chosen 1 through 10 simulation
seconds from its previous deadline. It is not a new random offset every frame,
and delayed service can leave it catching up with an old deadline.

The formation point rotates horizontally with the reference aircraft heading.
The reviewed formation request lasts nominally 3 seconds and uses its own
position-regulating speed mode. Exact formation names/table geometry, full
speed regulation for that mode, join/rejoin completion and wing breakup remain
open. Keep wing slot, leader, target assignments, spacing and variation state
explicit in the future API. A single target position and aircraft skill cannot
represent this behavior.

## B44: Steering execution and pursuit lead

Executable-confirmed: heading, flight-path pitch and body bank approach their
requested values over simulation time. They are not instantaneous attitude
assignments. The ordinary angular approach stops at the requested value instead
of overshooting it. Aircraft turn and roll capability come from the current
flight-performance lookup, with turn capability also depending on current speed.
Consequently, sharing F.BI does not give all twelve aircraft identical turns.

For the bank-dependent movement branch, heading authority decreases while the
aircraft rolls into a turn. Below seven eighths of the reference bank magnitude,
it scales with bank magnitude, with a floor of one quarter of base turn authority.
Bank in the opposing direction can suppress heading progress. Pitch authority
also depends on bank, with a quarter-rate floor. A reduced-rate command divides
heading and pitch authority by three; its roll rate is divided by three and
bounded to nominally 10 through 30 degrees per second. Other state branches
halve roll authority and cap it at 45 degrees per second. These are conditional
rules, not an experience multiplier or universal aircraft limits.

Requested flight-path pitch is bounded to approximately -90 through +90 degrees.
At the aircraft ceiling, positive requested pitch becomes level flight. Terrain
avoidance can raise the pitch request. Body pitch includes a separate offset
from flight-path pitch, so nose direction and velocity direction must remain
distinct inputs. Exact terrain clearance, special aircraft-state overrides,
performance-table selection and every steering mode are **unknown** in this
specification. The reviewed axis consumers establish these dependencies, not
complete steering closure for every maneuver. Trace those producers before
claiming a complete aircraft-specific motion profile.

The weapon-dependent lead used by B15 starts from the target position. For the
predictive weapon branch, distances of 20000 feet or more bypass prediction.
Within that boundary the calculation uses distance, a weapon/launcher speed
estimate and target scalar speed and attitude. For an aircraft launcher within
1600 feet, prediction is reduced according to the larger absolute heading or
pitch difference between the two aircraft: zero at 10 degrees or less, linear
between 10 and 35 degrees, and full at 35 degrees or more. These are differences
between aircraft attitudes, not line-of-sight errors. A non-aircraft aim point
can receive a 20-foot upward offset. The long-range early exit bypasses that
offset too. The speed estimator and short prediction-time cutoff still need
physical-unit closure; do not substitute a generic intercept solver and label
it recovered behavior.

## B45: Seeker visibility, launch eligibility and ammunition

Executable-confirmed: seeker eligibility has separate geometric, signature,
launcher-support and release-service checks. Passing any one is insufficient.
The checked envelope belongs to the selected equipment profile; there is no
single range or cone shared by the twelve aircraft or by every carried weapon.

When range checking is enabled, the seeker checks spatial range against the
profile's inclusive minimum and maximum. It also checks target altitude relative
to the observer against the profile's inclusive vertical limits. Sentinel limits
can disable individual bounds. A signature-adjusted effective range must also
fit inside the maximum. For positive final signature percentage S, that range
is physical range times 100/S, subject to overflow protection; zero signature
fails against a finite maximum range. Thus being inside the nominal range alone does
not establish visibility. Aspect, look-down and a speed-sensitive rejection
branch modify eligibility. Their complete signature producers, configuration
flags and per-store numerical profiles remain unresolved.

Angles are measured in the observer/mount frame. In the forward vertical
hemisphere, absolute horizontal and vertical errors must each fit their own
inclusive limits. For vertical errors beyond 90 degrees, the reviewed rule
accepts horizontal error within the larger of the horizontal limit and 90 degrees,
or vertical error at least 180 degrees minus the vertical limit. Both angular
limits set to their unrestricted sentinel bypass angles, not range checking.
The launch lock routine selects the second stored seeker zone. This does not
prove which zone every in-flight guidance caller uses.

A target must still exist and be usable. Equipment can additionally require a
live launcher, launcher emission, a compatible supporting seeker, or launcher
seeker visibility. Human-controlled aircraft have an additional launch-context
G check against the weapon's tracking limit; that particular gate does not apply
to ordinary AI. Support-required flags must not be flattened into a universal
one-missile support channel. The reviewed AI radar-on check requires the actor's
emission-enabled state and extends its emission-valid deadline to at least
10 seconds ahead. This is not proof of ten seconds of missile guidance after
radar shutdown. The corresponding player path checks its existing deadline.
Support loss, reacquisition and terminal autonomous guidance still require the
in-flight callers and their state transitions.

For the bomb-type branch when trajectory checking is requested, the predicted
impact must fall within the larger of 1000 feet and one eighth of current range
from the target. This is a release-solution tolerance, not guaranteed bomb
accuracy or a general missile launch radius. B42's AI service checks lock and
terrain before release. The lower-level release routine can instead clear an
invalid or terrain-blocked target and still release the weapon. Preserve the
difference between AI authorization to fire and a weapon's ability to leave
the station without a retained target.

An inhibited station refuses release. Ordinary finite ammunition is debited by
the equipment's actual-rounds-per-game-round amount. A positive remainder smaller
than that debit is allowed and is reduced to zero; zero ammunition refuses the
debit. An unlimited-store sentinel succeeds without decrementing. The global
unlimited-ammunition bypass found in this routine additionally requires human
control, so it is not an unconditional AI exemption. Pod and burst metadata can
make one release operation create multiple projectiles; projectile count is not
necessarily the ammunition debit. Exact special-store and complete burst policy
remain open.

The original debits ammunition before allocating the projectile. An allocation
failure can therefore return failure after a debit. A host implementation may
choose atomic allocation/debit as an explicit opinionated robustness policy,
but must not call that behavior recovered. Report inventory change separately
from projectile creation and target retention in the future service API.

## B46: Wing-command receiver contract

Executable-confirmed for the reviewed aircraft event receiver:

| Request | Receiver behavior |
| --- | --- |
| Break | For an eligible AI aircraft, constructs a nominal five-second motion request at corner speed, with heading relative to the receiver's own body heading, the supplied pitch, and automatic bank selection. A target is not required. |
| Approach | For an eligible AI aircraft with a target, forwards heading/pitch and a speed bounded by its own minimum/maximum. A zero speed request selects corner speed. No target means no approach motion is installed. |
| Horizontal/vertical spacing | Applies the respective formation setter. The sender-side limits remain in B43. |
| Formation selection | Applies the formation setter; the follow-up clears an active ordinary-state command in the reviewed state branch. It is not immediate relocation into a slot. |
| Wing control | Applies the control setter and resets the active command. |
| Target/order assignment | Distinguishes holding fire, restoring free selection, a class/policy request, and a concrete target. A concrete target changes the reaction/target state and establishes a nominal 20-second target-related deadline, with a time-adjustment branch. This is not yet proof that the target is forgotten at expiry. |

Break and approach do not install AI steering on a human-controlled recipient.
The common maneuver eligibility gate rejects original states 1..18 and 21..30;
accepted states 19..30 are normalized by a second helper before the new request.
The combined effect is that states 19 and 20 can transition, while 21..30 are
rejected on that path. Mission-facing names for all of these states remain
unknown, so do not relabel them as landing or refueling solely from their numbers.

The event handler's Boolean result is not an obedience or radio-acknowledgment
contract: some settings are applied while returning false, and a human break
or targetless approach can return true without installing motion. A host receiver
needs distinct applied, rejected, and no-motion outcomes. Broader player radio
orders, formation names, approach completion, sharing recipients, interruption
priority and rejoin behavior remain open. Trace these separately rather than
using the original handler result as the host command outcome.

## Proposed host API

The following is an opinionated interface design for future Rust implementation.
Names and records in this section are proposed, not existing crate exports.
Use explicit units and typed decisions rather than script sensor-name strings
or executable addresses. Keep the simulation independent of rendering.

```rust,ignore
pub fn resolve_experience(
    request: ExperienceRequest,
    policy: &ExperienceAssignment,
    random: &mut DecisionRandom,
) -> Result<ResolvedExperience, AiError>;

impl Controller {
    pub fn new(identity: ActorIdentity, profile: BehaviorProfile,
               experience: ResolvedExperience, seed: u64)
        -> Result<Self, AiError>;

    pub fn step(&mut self, frame: &DecisionFrame<'_>)
        -> Result<IntentBatch, AiError>;
}

pub fn target_geometry(own: &OwnState, target: &KnownTarget)
    -> TargetGeometry;
pub fn choose_aircraft_behavior(context: &AircraftContext<'_>,
    random: &mut DecisionRandom) -> Result<BehaviorChoice, AiError>;
pub fn choose_surface_behavior(context: &SurfaceContext<'_>,
    random: &mut DecisionRandom) -> Result<BehaviorChoice, AiError>;
pub fn plan_maneuver(choice: &BehaviorChoice, context: &MotionContext<'_>)
    -> Result<MotionIntent, AiError>;
pub fn decide_weapons(context: &WeaponContext<'_>)
    -> Result<WeaponIntent, AiError>;
```

The last five functions describe separable calculation boundaries for tests;
they need not all become public crate exports. `Controller::step` is the main
per-actor entry point. It accepts one immutable snapshot per advancing 120 Hz
simulation tick. Weapon search/preparation timings are specified in B42; other internal decision
cadences remain separate policies that are not yet fully specified. It must not redraw a tactic every tick
merely because the host calls `step`. Pause supplies no advancing ticks.

### Records and required inputs

| Record/entry point | Must look at | Must produce or preserve |
| --- | --- | --- |
| `ExperienceRequest` / `resolve_experience` | Explicit per-object level versus wing/editor setting, side/domain assignment channel, documented assignment policy | Level 0..3 plus origin; no reroll during ordinary updates, no fallback from unknown Quick Mission policy to editor jitter |
| `ActorIdentity` / `BehaviorProfile` | Stable actor identity, exact aircraft/object type, side, wing/member, family and mission role | Separate identities for family, capability and role; reject unsupported bindings rather than substitute another aircraft |
| `DecisionFrame` | Simulation tick, own state, permitted targets, threat events, orders, wing state, environment and service feedback | All views from one simulation snapshot, with observation timestamps and absent values represented explicitly |
| `OwnState` | Pose, scalar/ground-relative speed as distinct quantities, AGL and absolute altitude, fuel, damage, stores and current flight limits | No renderer-derived state or shared player globals |
| `KnownTarget` / `target_geometry` | Observation source/time, pose when known, permitted class/control metadata, validity and observed destruction | Spatial/horizontal range, separate heading/pitch errors, ahead/facing/off-beam and explicit unknown geometry |
| `AircraftContext` / `choose_aircraft_behavior` | Current reason/order, experience, geometry, performance comparisons, wing state and active maneuver | Behavior choice and reason; conditional probability draws only at documented decision points |
| `SurfaceContext` / `choose_surface_behavior` | Object class, ground experience, role, route, mobility, sensor and weapon constraints | Class-specific intent; stationary actors may request weapons without motion |
| `MotionContext` / `plan_maneuver` | Behavior choice, aircraft or surface limits, terrain, target/reference frame and ongoing maneuver progress | Desired motion with explicit altitude/offset frame and completion rule; never direct position writes |
| `WeaponContext` / `decide_weapons` | Launcher ownership, target role, inventory, legal firing solution, range/arc, sensor support and current orders | Sensor/track and weapon/device requests; separate inventory debit, projectile creation and retained target in feedback (B45) |

Proposed frame events include `ThreatReported`, `Hit`, `OrderChanged`,
`TargetUnavailable` and `ActorRemoved`, each with tick and actor references.
A threat report records its source, guidance knowledge and whether a launcher
or projectile identity is actually known. Do not manufacture warning knowledge
from a raw global missile list. Event priority and coalescing remain policy
gaps; the old interpreter's numeric reason ranking is not an accepted host rule.

Use feet and feet/second at existing simulation boundaries, radians for host
geometry and explicit conversion for recovered degree rules. Use named altitude
and offset frames rather than a bare vertical number. Performance differences
whose retail scale is unresolved must not masquerade as physical measurements.
Unresolved steering, eligibility or interruption semantics return `UnspecifiedRule` in a
research harness. An implementation can later supply a documented fitted policy
as an explicit profile, never as an invisible default.

### Intents, feedback and persistence

`IntentBatch` separates motion, sensor/target selection, weapon/device requests,
wing requests and activity. A weapon request names actor, station, target and
request identity so retry/feedback cannot fire twice accidentally. Motion has
an identity and completion rule. Feedback distinguishes completion, rejection,
cancellation, lost reference and still-in-progress, with a reason.

Store active behavior, target/order identities, maneuver progress, resolved
experience, deterministic random state and last processed tick in controller
state. Include this state in future replay/save snapshots. A removed actor
cannot remain a usable target; restart creates fresh state from the recorded
initial configuration. These are host design requirements, not recovered
retail scheduling details. Destruction knowledge and physical sensor presence
remain separate, as in the existing sensor component.

### Existing code to connect later

| Existing boundary | Planned connection and limitation |
| --- | --- |
| `sensors::Sensors::contacts`, `visual`, `observation`, `support` | Build permitted target views and track feedback; give each actor its own sensor state. `Observable` is service input, not automatic AI knowledge. |
| `sensors::Sensors::designate` and `step` | Apply validated sensor requests and advance observations; current channel/track rules stay in the shared component. |
| `combat::live::State::readiness`, `mounted_solution`, `step` | Reuse launch checks and combat simulation through a future actor adapter. Current methods are player/range-oriented, not a ready multi-actor AI API. |
| `flight::State::step_surface` and selected aircraft model | Convert motion intent into controls and step the actor's own model. `autopilot` supplies reusable steering ideas, not a claim of recovered combat steering. |
| `quick_mission::QuickMission::dummy_wings` | Replace the lossy launch payload at the later hookup stage; preserve all six wings, side/member identity and experience origin. |

## Acceptance and next research boundary

First synthetic cases should test B01/B02 at 89, 90 and 91 degrees, separating
heading and pitch; B03 with equal horizontal positions and different altitude;
B04 at minimum speed plus 74/75/76; experience choice boundaries from the linked
spec; and B12's inclusive distance/angle and 76% entry boundaries. Source-only
branch tests are provisional until relevant BI correspondence is checked.

Add B15 cases at each side of -100, -50, 50, 250, 1000 and 5000 feet of
separation error, plus off-beam rejection and own-speed limiting. Test B41
retention at 19999/20000 feet and the cumulative wing-assignment penalties.
Test each B42 aircraft timing group, delayed lock retries, pause and advancing
simulation time, and B43 spacing clamps and persistent variation deadlines.

Test climb/dive fallback, distinct IR/radar requests, an unarmed aircraft's
attack request, non-aircraft targets, absent/stale observations and removed
actors. Test the host API proposal for actor isolation, deterministic restart,
no duplicate fire and no per-tick skill rerolls once it is implemented. Do not
call these tests implemented or flown acceptance yet.

Add B44 tests for roll-in, opposing bank, rate limiting and the 1600/20000-foot
lead boundaries; B45 tests for inclusive envelope limits, zero signature, support
loss, inhibited/empty/unlimited stores and a final partial ammunition debit; B46
tests separating applied settings from motion installation and handler results.
These are specified future cases, not newly implemented tests.

Next resolve Quick Mission writer-to-loader skill handling; steering performance
producers, terrain overrides and completion exceptions; signature producers and
per-store envelopes; in-flight support-loss/reacquisition; and remaining wing
orders and approach completion. B44 through B46 close the reviewed connections
and explicitly identify the branches still preventing complete behavioral closure.
Surface engagement follows its separate source map. Broader maneuver and
family coverage remains open. This document can grow by complete behavior
sections without waiting for byte-level closure of the entire executable.
