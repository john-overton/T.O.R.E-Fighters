# AI gun employment and weapon-service recovery

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Status and provenance

Research mode, 2026-09-28. **Proposed correction, not implemented.** John asked
why enemies appear never to use their guns. The
[investigation](../baselines/ai-gun-employment.md) confirms sparse two-round
releases followed by a permanent weapon-service timeout in his recorded fight.
The current [B42 service](ai.md#b42-weapon-preparation-search-cadence-and-firing)
and [B45 store eligibility](ai.md#b45-seeker-visibility-launch-eligibility-and-ammunition)
remain the shipped implementation.

The rules below are **opinionated agent proposals**. The existing imported
weapon envelopes and fitted physical bullet cadence retain their provenance.
Original expiry recovery, gun-specific preparation, burst duty cycle and lead
policy are **unknown**. The next research step, if original parity is needed,
is a bounded behavior contract for those specific decisions using the build
identity in the [AI research baseline](../baselines/ai-research.md). Missing
retail comparison is not a reason to preserve a permanent inability to fire.

## Recovery and readiness

A living aircraft with a current target and ammunition must remain capable of
firing after a previous attempt expired. Expiry must not require forgetting its
target, losing sight, changing aircraft or restarting the mission.

For the shared weapon service, the initial proposal is to replace permanent
expiry with the existing bounded no-station retry, **2 seconds plus the existing
optional 0.5-second delay**. Re-evaluate current eligibility after that delay.
Keep target identity, observations, ammunition, orders and missile-support state.
An expired attempt cannot authorize a shot while its firing conditions fail.
Missile-specific preparation, acquisition and support remain distinct.

A loaded gun has its own ready/burst/recovery state. Once a permitted live gun
solution exists and its recovery is complete, evaluate release at the next
120 Hz tick. Do not run an entire missile-style search/preparation/tracking
sequence for every gun group. Losing the solution interrupts fire but does not
make the gun permanently unavailable or require a target-memory reset.

## Physical bursts and aiming

Initial authored burst policy: **0.5 seconds of permitted fire followed by
0.5 seconds of recovery**, while a valid solution persists. This gives 16 bullets
per complete burst for the reviewed guns at the existing fitted 32 bullets/s.
Other profiles use their own imported cadence. These values define a starting
development policy, not original-game timing or real cannon rates.

Use the existing [individual-round contract](damage-smoke.md#individual-cannon-rounds)
for spacing, tracer ordinals, finite ammunition and per-bullet damage. Debit
actual emitted rounds consistently. Preserve cadence across interruptions, and
stop pending fire on accepted hold/recall, target loss, lost firing solution,
death or empty ammunition. Do not refund or consume ammunition twice across the
controller/host boundary. Full accounting must be tested before replacing the
older prepaid representative-group policy.

The barrel is fixed to the aircraft's gun mount. Steer toward a predicted
interception point using only current observed target motion and the shared
projectile model. A broad store acquisition/employment envelope alone is not a
barrel-alignment test. Authorize fire only when the predicted physical gun path
intersects the target's collision volume under the normal terrain/range checks;
emit along the actual barrel direction with the shared gun dispersion rule.
Bullets remain unguided. Frozen memory and a hidden target position cannot supply
a firing solution. Do not compensate for poor alignment by rotating each bullet
directly toward the target or weakening F-22 stealth.

Original alignment tolerance and tracking policy remain unknown. Using the
existing target collision volume and projectile model for the predicted
intersection is an agent-authored starting rule; validate it on approaching,
receding and crossing targets before choosing any additional aim tolerance.

## Acceptance

- Restore valid station/lock/path after expiry without losing the target; repeat
  across all 14 profiles and with a different still-visible target. Firing must
  resume. Also verify no shot while range, terrain, orders or ammunition forbid it.
- Measure complete bursts, exact physical cadence, reload timing, tracer
  continuity, finite stores and interruption/accounting boundaries at 120 Hz.
- Prove fixed-barrel launch and lead on head-on, tail-chase and crossing targets.
  Compare predicted and flown projectile paths instead of accepting an activity
  label or a nominal store score as evidence of a gun solution.
- Run sustained close fights beyond the old timeout at all four skills, using
  exact aircraft identities, an F-22 and a non-stealth control, and both flight
  adapters. Preserve missile selection/support and all compatibility modes.
- Expose gun readiness, solution failure, burst/recovery timing and ammunition
  in replay explanations. A permanently expired service must not say merely
  "not yet". Repeat seeds and validate actual bullet releases and damage.
