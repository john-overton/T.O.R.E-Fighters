# Visual contact and reaction to incoming fire

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Status and provenance

Research mode, 2026-09-28. **Proposed behavior, not implemented.** John requested
investigation and a repair plan for F-22 visual detection, including the Su-27
and all aircraft variants, and aircraft reactions to air and ground fire.
The [validation pass](../baselines/visual-awareness-under-fire.md) measures the
current rebuild. It does not establish original Fighters Anthology behavior.

The required outcome is that a visible hostile can prompt an appropriate
mission response, and perceived incoming fire can prompt self-preservation
without an identified attacker. The rules and new constants below are
**opinionated agent proposals**, not values requested by John or recovered from
the executable. Original lookout timing and gunfire evasion thresholds remain
**unknown**. Next research step, if needed for parity: recover the visual
attention and hit/near-miss behavior into a prose contract, using the build
provenance in the [AI research baseline](../baselines/ai-research.md).

The existing [awareness specification](ai-awareness.md) remains the description
of shipped behavior. This proposal explicitly changes its nose-only lookout
and attack-only automatic leader release. It does not silently redefine them.
Delivery order lives in the [roadmap](../ROADMAP.md#visual-contact-and-incoming-fire-repair).

## Seeing an aircraft and choosing to engage

Radar stealth must not reduce pilot visual range. Keep the existing skill
ranges, environmental/terrain limits, live-observation rules and memory
durations from [visual awareness](ai-awareness.md#visual-awareness-and-memory).
Apply the same visual rules to every aircraft, including the exact F22.PT,
F22N.PT and the opinionated F/A-XX identity. Do not tune radar or infrared
signatures to compensate for a visual or engagement defect.

Add a pilot lookout independent of the aircraft's nose direction. Proposed
acceptance bound: in clear, unobstructed flight, a continuously visible aircraft
inside the skill range is noticed within **2 seconds**, including side and rear
approaches. A scan must actually sample the relevant direction; do not grant
continuous tracking of unseen objects. An acquired threat receives attention,
while lost contacts retain only their last measured state. Scan phase and
refresh use simulation ticks. Aircraft directly above/below, sustained inverted
flight and cockpit blind sectors need explicit fixtures before accepting the
lookout implementation. Complete cloud/night visibility remains a separate gap.

Keep neutral formation startup. Proposed change to the automatic AI leader:
a currently observed, positively hostile aircraft may trigger an engagement
order when the assigned duty permits it. Free fire, intercept, CAP and escort
must still apply their own eligibility, patrol and protection rules. Detection
does not automatically make every contact an enemy. Self-defense and weapons
hold retain their restrictions. Human-led wingmen still require an accepted
order. Explicit recall remains effective until a new order or newly perceived
attack; do not immediately undo a recall just because its old contact is visible.

This extends the earlier agent-authored attack-only leader interpretation in
[formation and leader authorization](ai-awareness.md#formation-and-leader-authorization).
It is a mission-policy change, not an F-22 sensor repair. Before implementation,
update that contract and its tests together rather than changing only the UI or
claiming the current neutral policy is a sensor fault.

## Perceiving incoming fire

Separate **danger to this aircraft** from **identity of the shooter**. Air or
surface origin does not change the victim's right to defend itself. A threat
with no identified source cannot supply a target identity, position, radar lock
or permission to fire at a guessed attacker.

Proposed initial gunfire rules:

- A visible tracer with two measured positions is threatening when its relative
  trajectory predicts closest approach within **250 feet in the next 2 seconds**.
  Observe a tracer throughout its life, not just its departure. Receding and
  harmless crossing rounds do not sustain an incoming threat.
- A round actually passing within **100 feet** supplies an anonymous proximity
  cue even outside the visual scan. This is a fitted awareness allowance for a
  close pass, not recovered sound or cockpit visibility behavior. It gives no
  shooter position and expires like other fire cues.
- A combat damage event supplies an immediate anonymous hit cue, including from
  behind or below. Distinguish weapon damage from terrain contact and ordinary
  flight damage where the producer knows the cause.
- Keep a fire cue for **2 seconds** after its last observation or close pass.
  Coalesce a burst into one defensive episode. New valid evidence refreshes it;
  forwarding, hidden projectile motion and a remembered shooter do not.

Keep departure-based shooter identification separate, using the existing
[identification contract](ai-awareness.md#assignment-delivery-and-observed-attack-reports).
Its short departure window may restrict identifying a launcher; it must not
restrict recognizing danger. Supporting-radar, active-radar and passive missile
warning rules remain guidance-dependent. A hidden shooter or silent launch
must not become a global warning broadcast.

## Breaking and returning to the mission

A perceived gun threat or weapon hit can start defense with **no offensive
target**, while neutral, under weapons hold, or during a return to formation.
Request a safe break within **30 ticks, 0.25 seconds**, of accepting an urgent
gun/hit cue. This bounds the response request, not the airframe's achieved turn.

Reuse the existing [jink and dive requests](ai-awareness.md#jink-notch-dive-and-countermeasures)
and each airframe's flight limits. Choose a break from measured incoming
direction when available, otherwise use a deterministic side choice. Preserve
the existing missile notch and skill-dependent missile timing. Gun-only danger
does not dispense chaff or flares. Terrain avoidance and controllable flight
remain mandatory; a low-altitude aircraft must not dive into the ground.

Arbitrate simultaneous threats into one maneuver using observed time to danger,
with conservative treatment of an anonymous hit. A burst must not restart the
turn every round. After the cue expires, resume the permitted mission or search
from legitimately remembered information. Do not require finding or killing the
shooter before ending defense. No maneuver guarantees survival.

## Observable acceptance

Expose separate replay reasons for contact source, lookout direction, denied
visual acquisition, mission permission, incoming-fire evidence, shooter
identification, chosen break and return to duty. A blank offensive target is
not proof that an aircraft has seen nothing. Record the anonymous hit/proximity
cue so a review can distinguish it from fabricated shooter knowledge.

Required acceptance includes every current selectable opponent against F-22A,
F-22N and F/A-XX, a non-stealth control, all four resolved skills, front/side/rear
geometry, visual range/angle boundaries, terrain masking, recall/hold and
contact loss. Exercise air cannon, anonymous incoming rounds, controlled AAA
trajectories, supported SAM warnings and passive/active air missiles. No new
surface targeting AI is needed to exercise a victim's response to controlled
ground-source projectiles.

Check the 250-foot/2-second predicted-threat and 100-foot proximity boundaries,
zero-target damage, harmless and receding fire, burst coalescing, simultaneous
missile/gun threats, low altitude, low speed, depleted devices and cue expiry.
Verify actual roll/heading/G response as well as activity labels, and compare
matched defended and non-defended trajectories without requiring every shot to
miss. Repeat seeds and verify recording reconstruction. Run both legacy and
researched flight paths explicitly; keep the native-table research path separate.
