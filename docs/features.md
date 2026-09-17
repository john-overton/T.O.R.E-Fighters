# Feature evidence and authored additions

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

This matrix separates what evidence supports from what we deliberately add or
approximate. **Origin and completion are separate.** A retail record establishes
its values, not every rule using them. A manual describes intended behavior, not
a verified match to our imported executable. No retail comparison is claimed.

Read the [provenance definitions](behavior-provenance.md) for `spec-derived`,
`native`, `fitted`, `opinionated` and `unknown`. A behavior specification can
contain both retail-supported components and explicit authored additions;
implementing that spec does not turn every component into recovered retail fact.
This is a reader's index, not a second roadmap or a revision log. Exact rules,
constants and tests live in the linked specifications and guides.

## Implemented components

| Component | Retail support | Our contribution or limitation | State and source |
| --- | --- | --- | --- |
| Menu art, fonts and sounds | Imported original resources and recovered layout evidence | Native assets with spec-derived layout; host navigation and presentation are separate | Implemented; [menu evidence](baselines/main-menu.md) |
| Aircraft equipment and signatures | Reviewed PT/SEE/ECM records | Spec-derived profiles; aircraft labels do not justify substituting equipment | Implemented for twelve aircraft; [radar spec](spec/radar.md) |
| Radar contact detection | Source ranges and some reviewed conditions | Opinionated detection, notch, jammer and era tuning; not retail measurements | Implemented; [decisions and limits](radar.md#deliberate-departures-and-known-approximations) |
| RCS display and aircraft exposure | Documented panel purpose and partial source calculations | Opinionated shared aspect/contour model; agent constants are identified separately from John's feature request | Implemented; [RCS spec](spec/rcs.md), [component guide](radar.md#rcs-instrument-and-shared-aspect-model) |
| Destroyed aircraft on sensors | Remaining original behavior is not fully established | Opinionated persistence requested by John; fitted fall and target eligibility remain documented | Implemented; [scope](radar.md#destroyed-aircraft-remain-sensor-objects) |
| Flight response | Imported PT values and reviewed component behavior | Hybrid flight combines native components with fitted laws; provenance is not whole-aircraft parity | Implemented with limits; [flight model](FLIGHT-MODEL.md), [component origins](behavior-provenance.md#current-flight-examples) |
| Ground contact and landing | Full retail contract unresolved | Opinionated contact accepted by John on 2026-09-15; authored rules are not waiting for native provenance | Implemented with limits; [landing baseline](baselines/native-land-foundation.md) |
| Input and device profiles | Original game controls are references, not evidence for modern device support | Opinionated modern binding layer and controller feedback | Implemented; [input guide](INPUT.md), [validation](baselines/input.md) |
| Current missile motion | Imported envelopes, source launch-speed/motor helpers and weapon-specific support checks | Fitted timing and pursuit; scalar launch-speed inheritance exists, full velocity inheritance does not | Development range implemented; [current behavior](spec/missiles.md#what-exists-today-and-what-changes), [record interpretation](formats/missiles.md) |

## Planned missile additions

All rows below are **planned, not implemented**. John requested the feature
additions on 2026-09-17. Agent-selected constants are not attributed to him.
The [missile plan](missile-update-plan.md) owns delivery stages; the
[missile matrix](spec/missiles.md#first-pass-inventory-matrix) owns per-weapon values.

| Component | Retail-supported portion | Deliberate addition or unresolved part | Origin of the planned change |
| --- | --- | --- | --- |
| Active-radar activation | General concept has manual support, as recorded in the spec | Per-weapon last-intercept thresholds and explicit search/acquired states; exact original transitions unknown | Opinionated requested feature, fitted agent distances; [activation](spec/missiles.md#activation-and-independent-acquisition) |
| Launch motion and prediction | Existing source helper incorporates aircraft scalar speed | Full vector inheritance, finite additive boost and matching motion/intercept estimates | Opinionated user request; fitted agent boost budget; [velocity rules](spec/missiles.md#launch-velocity-and-intercept-estimates) |
| Seeker-active uncued release | General no-designation release is not established by the reviewed evidence | A/I/E BORESIGHT launch with immediate own-seeker search; S retains support requirements | Opinionated user request; [launch modes](spec/missiles.md#uncued-launch-and-narrow-ir-search) |
| Narrow forward IR acquisition | Imported IR signature and seeker volume | Smaller uncued cone, ranked heat quality, dwell and aircraft engine/aspect modifiers | Opinionated user request; fitted agent cone and quality constants; [heat rules](spec/missiles.md#fitted-heat-quality-and-tone) |
| Weapon HUD and seeker sound | Manual-supported cues are listed in the spec | Projected search cone, mode labels and estimates; exact original pixels/samples and probability model unresolved | Mixed manual-supported presentation and opinionated additions; fitted tone envelope; [HUD scope](spec/missiles.md#weapon-hud-delivery) |
| Guidance lifetime and reacquisition | Motor and cleanup fields exist; `trackT` meaning remains unknown | Independent guidance lifetime, memory window and explicit fallback values | Requested lifetime feature, fitted agent rules; [timers](spec/missiles.md#range-motor-and-tracking-lifetime) |
| Passive emitter behavior | Candidate source records and manual context | Exact radar/jammer eligibility still needs per-weapon evidence; no invented heat fallback | Requested separate E category; unresolved eligibility; [guidance types](spec/missiles.md#four-game-guidance-types) |

A fitted value is allowed to ship when its behavior is documented and validated.
It does not become a retail fact because a test passes. Planned features remain
planned until their implementation and acceptance evidence are linked here.
