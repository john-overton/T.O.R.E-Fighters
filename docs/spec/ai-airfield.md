# AI takeoff, landing and return to base

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. AI work requested by John on 2026-09-23. This page says
what a player sees AI aircraft do on and around an airfield, with the numbers.
Retail evidence is the static trace of the takeoff and landing handlers in
[AI source notes](../formats/ai.md#airfield-takeoff-and-landing-sequences) and
the airport anchor roles in
[STRIP notes](../formats/native-strip.md#remaining-template-callback-boundaries).
Nothing here was compared against a running retail copy.

Labels: **spec-derived** (retail handler or manual page), **fitted** (agent
decision, 2026-09-23, where retail is unknown or the hybrid flight model cannot
hold the retail target; the retail value is given alongside), **opinionated**
(named requester and date). Code: `tore-sim::ai::airfield` (the sequence) and
`ai::mission` (the gates between aircraft). Every number is a named constant in
`airfield.rs`.

## Airport anchors

An airport's STRIP shape supplies taxi-out points, a takeoff spot and heading,
a landing point and heading, taxi-back points and nine parking slots. With
anchors the AI uses them exactly as below. Without anchors (a runway known only
by its centre, heading and length) the AI uses the fitted runway-only fallback
described in each section.

## Takeoff from the ground

- **Hold** (spec-derived, retail state 1): stopped, brakes set, gear down,
  takeoff flaps down, idle. The gates below are re-tested on entry and then
  every 5 seconds.
- **Turn gate** (spec-derived, manual p.209: one at a time, leader first):
  every earlier member of the wing must be past its first taxiway leg. When a
  human leads the wing, AI wingmen also wait while the player is on the ground,
  so they stay parked until the player is airborne. A human leader who never
  takes off keeps them waiting.
- **Runway-free gate** (spec-derived): no other aircraft at the airport is on
  the ground within 125 ft of the takeoff spot, rolling for takeoff (the player
  counts as rolling from 7 ft/s), lining up, taxiing its first two legs, flying
  the approach gates or final, or rolling out. Climbing out, holding at marshal,
  taxiing clear and parked aircraft do not block. With anchors, an aircraft
  already within 475 ft of the takeoff spot needs only the turn gate.
- **No timed interval** (spec-derived): spacing comes only from the two gates
  and the 5 second re-test. An eligible wingman starts within
  5 seconds of the gates clearing. Taxi distance and other aircraft still
  occupying the runway can make liftoff gaps much longer.
- **Taxi out** (spec-derived): at 50 ft/s (30 kt) to taxi-out point 0, stopping
  there, then on to points 1, 2 and 3; a leg ends within 50 ft of the line
  through its point running back along the reversed runway heading, or within
  475 ft of the takeoff spot. Retail pivots a stopped aircraft toward the next
  point. The hybrid ground model cannot turn a stopped aircraft, so it creeps
  round at 12.5 ft/s instead (fitted).
- **Line up** (spec-derived): aim at the centerline point 50 ft closer than the
  current distance to the spot; 50 ft/s when within 5 degrees of that aim,
  otherwise 12.5 ft/s (retail stops and pivots at 45 degrees or more, fitted as
  above). On the centerline (within 50 ft) taxi straight to the spot and stop
  within 10 ft. More than 26,400 ft from the spot goes back to holding. Without
  anchors the spot is on the centerline 200 ft ahead of the aircraft's parking
  place (fitted), and the runway-free gate always applies.
- **Roll** (spec-derived): brakes off, full military power, runway heading,
  wings level. After reaching minimum speed it runs 5 seconds at full power,
  then raises the nose 4 degrees for 4 seconds. B44 still forbids pitching up
  below minimum speed. Fitted: an aircraft still on its wheels after that asks
  for 8 degrees of nose; an afterburning aircraft lights the burner only if it
  passes half the runway below minimum speed (retail burner use is unknown).
- **Climb-out** (spec-derived): runway heading, flight path 4 degrees, full
  military power, gear and flaps still down, until 650 ft above the terrain.
  Then gear and flaps come up and the aircraft returns to free flight; wingmen
  rejoin their leader. Fitted safety: the climb-out ends 90 seconds after
  liftoff in any case.

## Landing

Landing starts from four causes. Each ends with the aircraft parked for good:
it stays alive on the ground, never takes off again, takes no part in combat or
formation, is not an air target, and refuses wing orders.

| Cause | Trigger | Provenance |
| --- | --- | --- |
| Bingo fuel | Bingo or critical fuel (B48) with a known home runway | Spec-derived for an AI wingman of an AI leader; fitted for leaders, singletons and wingmen of a human leader |
| Bug out | The player's bug-out order (manual p.160) | Spec-derived |
| Land at selected airport | The player's order naming the tower's selected airport | Opinionated, requested by John 2026-09-23 |
| Join the leader | An AI wingman within 10,000 ft of a landing leader and 40,000 ft of its airport (B48) | Spec-derived; for a human leader the wingman's home runway is used as the leader's airport (fitted) |

Without a home runway, bingo keeps the older behaviour: the aircraft turns for
its home airport at cruise speed.

- **Route home** (spec-derived for bingo and bug out, fitted for an ordered
  landing): a private route to the airport at 5,000 plus a random 0 to 5,000 ft
  and B48 cruise speed. Within 60,000 ft of the airport the aircraft hands over
  to the marshal. This leg is free flight for missile warnings. A joining
  wingman goes straight to the marshal.
- **Marshal** (spec-derived, manual p.65: "all other aircraft will hold marshal
  while you land"): cleared when it holds a parking slot, every earlier member
  of its wing landing at the same airport is on the ground (fitted scope: other
  wing members are not waited for), and the runway is free. Otherwise it holds
  on a square with corners 52,800 ft (10 statute miles) out on each world axis
  from the landing point, flying corner to corner clockwise from above, at
  6,000 ft plus 1,000 ft per wing position (absolute; fitted floor 1,000 ft
  above the airfield) and (maximum + corner speed) / 2. The clearance and the
  corner are re-planned every 5 seconds. The human player's landing blocks the
  runway: the host reports it through `AiMission::set_priority_landing`
  ([player priority](airports.md#wing-landing-orders-and-player-priority)).
- **Approach** (spec-derived): gear down; three gates on a 6 degree path back
  along the landing heading from the landing point, 35,200, 17,600 and 8,800 ft
  out and about 3,700, 1,850 and 925 ft above it. Speed (maximum + corner) / 2
  to the first gate, then at most 366 ft/s (217 kt). A gate is done within
  250 ft horizontally; fitted addition: or once it is abeam or behind within
  5,000 ft, because the hybrid model's turn radius at these speeds is far wider
  than 250 ft. Fitted: flaps come down below 200 kt and the speedbrake opens
  when more than 50 ft/s fast in the marshal, approach and final.
- **Terrain clearance** (fitted, agent decision 2026-09-23): during climb-out, on the route
  home, at marshal, on the approach gates and during a go-around, sample the
  ground at seven equally spaced points along the next 12,000 ft of track.
  When terrain there is more than 300 ft above the landing point, stop
  descending within 1,000 ft of it and request at least a 20 degree climb
  within 500 ft. During either correction level the wings and hold the current
  heading, so a turn cannot consume the lift needed to clear the terrain.
  Use full military power and close the speedbrake during the climb. Resume
  the route once clear. This is a forward sampling rule, not terrain routing.
- **Final** (spec-derived path, fitted speed and flare): down the 6 degree path
  to the landing point, wings level below 50 ft. Retail flies at most 293 ft/s
  with the nose 17 degrees above the path and has no flare. The AI flies 1.1
  times its clean minimum speed under that 293 ft/s cap, lets the hybrid model
  set its own angle of attack, and eases its descent toward height above the wheels divided by 6 seconds,
  with a minimum 1.5 degree downward path, so a fast final has time to flare.
  Below 60 ft it reduces the speed target to the clean minimum. The speedbrake
  stays closed during the flare. These flare constants are fitted.
- **Rollout** (spec-derived end, fitted method): brakes on, idle, flaps down,
  held on the centerline until below 1 ft/s. Retail holds the nose 8 degrees up
  at 146 ft/s for 2 seconds; on the hybrid model that lifts the aircraft off
  again, so the AI holds half nose-down elevator instead.
- **Clear and park** (spec-derived with anchors): flaps up, taxi at 50 ft/s to
  the rollout end, runway exit, return taxiway and parking entry points,
  stopping at each, then to its parking slot; the throttle closes within 200 ft
  of the slot and it stops there. The runway counts as free as soon as it starts
  taxiing clear. Retail turns to the parking heading; the hybrid ground model
  cannot turn a stopped aircraft, so the AI parks on whatever heading it arrives
  (fitted). Without anchors (fitted): it taxis on toward the far end to a spot
  400 ft before it, 250 ft further back for each higher slot, alternating 40 ft right
  and left of the centerline, never less than 150 ft ahead of where it stopped.
- **Landing end without anchors** (fitted): the end with at least a knot more
  headwind, otherwise the end facing the aircraft's arrival. The aim point is a
  quarter of the way down (manual p.65-68). With anchors the airport's landing
  heading is always used (spec-derived).
- **Parking slots** (spec-derived): nine per airport. A ground-started aircraft
  sitting on a slot holds it until its climb-out ends; a lander takes the lowest free slot
  when it leaves the marshal and keeps it.
- **Go-around** (fitted safety; retail has none): within 6,000 ft of the landing
  point a final more than 300 ft off the centerline or 30 degrees off the
  landing heading, an aircraft still airborne 35% of the runway length past the
  landing point, or a bounce above 15 ft during the rollout goes back to the
  marshal and flies the gates again.
  The ejection assessment also requests a go-around for a hazardous final
  steeper than 9 degrees down, including one over the runway. This fitted
  threshold is the steepest commanded final (6 degrees plus 3 degrees of
  glide correction). The aircraft uses full military power on the landing
  heading, level below minimum speed and at an 8 degree climb above it.
  Gear rises at 200 ft above the runway and flaps above 200 kt. The climb
  ends at 1,000 ft above the runway or after 60 seconds, then it tries again.

## Orders and warnings

- **Bug out** (spec-derived): ignored while taking off, landing (from the
  marshal on) or on the ground. Otherwise the wingman leaves the wing, drops its target and flies home
  to land. From then on it refuses every wing order (manual p.160: "wingman will
  no longer respond to commands").
- **Land at selected airport** (opinionated, requested by John 2026-09-23;
  cancel rule fitted): a later disengage or formation order cancels an ordered or join-the-leader landing while the
  aircraft is still on its route, holding at marshal or flying the gates. From
  final on it is committed. Cancellation raises gear and flaps and closes the
  speedbrake. A cancelled join remains cancelled while the leader is recovering.
- **Wing abort** (spec-derived): a joining wingman on its route, at marshal or on
  the gates whose leader is neither landing nor on the ground raises gear and
  flaps and returns to free flight.
- **Missile warnings** (B47, spec-derived): ignored while taking off and from
  final on. At marshal or on the gates a warning or a defensive reaction
  abandons the approach; the aircraft defends in free flight and resumes the
  landing, from its route or the marshal, once the threat has gone (resume
  fitted). Abandoning the approach raises gear and flaps and closes the
  speedbrake. The route leg reacts like any free flight.
- **Formation** (fitted): a wingman does not formate on a human leader who is on
  the ground; it flies free until the leader is airborne. Aircraft on the ground
  are not air targets.

## Ejection during an airfield sequence

John requested on 2026-09-23 that a landing aircraft try again instead of
abandoning a recoverable aircraft. The agent also applies the guard during
takeoff roll and climb-out. Outside those phases the
[ordinary ejection assessment](ejection.md#fitted-ai-decision) is unchanged.

First the ordinary assessment must find an unrecoverable descent or destroyed
aircraft. During takeoff roll, climb-out, marshal, approach, final and rollout,
only a catastrophic assessment reaches the existing ejection chance monitor.
The fitted catastrophe thresholds, chosen by the agent on 2026-09-23, are:
structural failure, fire, wing damage, damage fraction at least 0.5, a dead
engine with the landing point farther than six times the height above ground,
a spin, bank beyond 90 degrees below 1,000 ft, or impact within one second
with no landable surface, gear below 99% deployed, or an exceeded imported
landing limit. This remains an estimate, not proof of physical inevitability.

Other assessed hazards on the approach gates request a go-around. On final
that happens if the projected touchdown is off the runway, gear is below 99%,
or the descent path is steeper than 9 degrees. An ordinary low, sinking final
over the runway continues to flare and land. Takeoff continues at full power.
The existing healthy-aircraft guard above 200 ft and the once-per-second
70% ejection chance remain in effect; a catastrophic judgement does not
promise that an ejection will happen before impact.

## Validation

Synthetic integration tests in `ai::mission::airfield_integration_tests` fly
actual input-driven departures and landings, cancellation, priority, bug out,
bingo, wing joins, missile warnings, go-arounds, catastrophic ejection and a
ridge approach. The steering regression also checks a flap-down descent and
exact physical replay. Imported-aircraft runs and their limits are recorded
in the [ground-start baseline](../baselines/ground-start.md#whole-wing-ground-start-2026-09-23).
Retail comparison, broad crosswind coverage and multi-wing airport contention
remain unmeasured.
