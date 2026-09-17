# Missile update plan

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. Feature sequencing is tracked in the
[roadmap](ROADMAP.md); the delivery stages for this feature live here.

John requested this next planning slice on 2026-09-17: four guidance types,
including initially silent active radar with per-weapon activation distances
from the last known intercept point, plus range, burn and tracking lifetime.
The same request now includes aircraft-velocity inheritance, boost/target-motion
estimates, uncued onboard-seeker launch, narrow IR acquisition, HUD cues and tone. IR remains independent of passive emitter
homing. [Draft behavior and complete candidate matrix](spec/missiles.md),
[inventory evidence](baselines/missiles.md), [source unknowns](formats/missiles.md).
This schedules game missile work without authorizing combat AI or new ground/ship
systems. Radar's remaining tuning pass stays open.

| Stage | Work | Acceptance |
| --- | --- | --- |
| 0. Inventory and specification | **Done as a first draft.** Review all 135 JT records; account for 63 missile-like candidates, current allowlists, provisional types and exceptions. Review the manual for HUD/seeker evidence and record user choices separately from agent defaults. | Every candidate has a row; unsupported classifications stay explicit; original data is not replaced by real-world expectations. |
| 1. Weapon profiles and timing | Build typed guidance profiles from the spec, including its fitted per-weapon active-on distances. Retain source motor/range values; add guidance lifetime separately from lock memory and object removal. Resolve wide-angle geometry and required first-release classifications. Add full launch-velocity inheritance and the fitted finite-boost rule; keep a compatibility weapon profile. | Synthetic timing/range boundaries pass, including removal before burnout and guidance expiry before cleanup. Launch-speed and closure examples agree with actual motion and estimates. `trackT` remains unmapped until its meaning is established. |
| 2. Seeker observations and guidance | Connect seeker-owned observations to signature, RCS/aspect, terrain and explicit emission state. Implement S/I/E support and loss rules, boresight candidate acquisition and heat quality with controlled fixtures. | No hidden-target updates, cross-channel fallback or transfer to the newly selected cockpit target. Emitter eligibility, narrow IR cone, quality/dwell thresholds and no-designation launch have tests. |
| 3. Silent flight and pitbull | Implement the velocity-aware cued intercept, supported updates and activation for A; BORESIGHT enables the onboard seeker immediately. | Each configured activation boundary, close launch, failed acquisition, support loss and two-target launches pass; pitbull is emitted only on acquisition. Uncued launch needs no fabricated target or intercept. |
| 4. HUD, seeker tone and replay | Deliver the spec's manual-supported HUD cues, mode/search cone, solution estimates and IR tone. Add the rebindable mode action, bay handling and mounted-seeker reset. Record full launch velocity, launch mode and heat state; update fingerprints and user guides. | Cone projection matches search geometry across zoom/aspect ratios; tone and HUD agree with acquisition. Pause and mute behave correctly; replay reproduces both launch modes. Unknown hit probability is not replaced with heat quality. |
| 5. Range and roster acceptance | Run the spec's launch-speed, target-motion, range and uncued-search scenarios, then manually exercise current A2A default stores. Record observed reach and remaining approximations in one feature baseline. | All applicable repository checks and display smoke pass; report platform limits. Catalog-only ordnance and held rows are not silently enabled. |

Stages 1 through 4 are implemented and covered by synthetic checks.
Stage 5 roster, rendered and full validation remain. Start with the fifteen current
allowlisted missile identities, prioritizing A2A. Existing AGM65G/AS7 integration
gets regression coverage; expanded A2G behavior remains deferred. The two emitter
candidates can be tested with explicit fixtures before adding ground systems.
John requested approximate per-weapon activation values; the numeric defaults,
unit choice, search-cone caps, heat/tone tuning, two-second memory, lifetime
fallback, boost budget and intercept estimator are agent decisions in the spec, not requests attributed to John.


The [feature evidence matrix](features.md) separates the retail-supported parts,
authored additions and implementation status. Stage 4 includes original HUD
resource/figure inspection and audio-sample identification; the planning pass
reviewed manual text only. Implementation progress and measured results are recorded in the feature baseline.
