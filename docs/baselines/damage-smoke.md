# Aircraft damage and smoke validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-20, Linux with the locked Rust toolchain. Source
archive identities, dimensions and shape observations live in the
[resource review](../formats/objects-and-shapes.md#combat-damage-and-smoke-resource-review).
[Behavior and fitted constants](../spec/damage-smoke.md) are specified separately.
No retail bytes are committed. Current screenshots and logs are local to
`.local/damage-profile-review/`; earlier resource and smoke evidence remains in
`.local/damage-smoke/`.

## Localized appearance

F/A-18D captures on NVIDIA RTX 4070/Vulkan inspect left-wing damage fractions
0.10, 0.40 and 0.80, right-wing and tail damage at 0.80, and nose damage at 0.10.
The capture fixture uses one simulation tick and the oblique view at 2x zoom.
Light damage retains the nose. The left wing gains persistent marks and then
loses area; the reviewed original C/D body and fragment appear at the structural
threshold. Right-wing damage leaves the left wing intact. Tail damage visibly
shortens the vertical stabilizers and adds damage patches. The local comparison
is `damage-grid.png`; full unmodified captures are the corresponding PPM files.
These are controlled presentation fixtures, not original-runtime comparisons.

Synthetic renderer tests check that wing damage preserves the opposite wing,
that light marks retain intact surface geometry, that cuts interpolate texture
coordinates, and that fin loss does not cut the wing or nose. Reviewed original
body selection is tested separately from fitted surface tears.

## Smoke and debris

Smoke retains the tested source artwork, ignition and burnout gating, airborne
wreck emission, ground-contact cessation, independent puff expiry, fixed-tick
cadence, capacity limits and reset behavior. Gun rounds do not emit missile smoke.
The original shared smoke and ground-impact texture inspections remain valid.
The current smoke rates are in the behavior spec, not inferred from captures.

Debris tests cover full velocity inheritance, gravity, swept terrain contact,
one ground effect per contact, expiry and reset. Structural region selection now
chooses the matching reviewed fragment and attachment offset. Sections without
a reviewed original breakup pair use fitted surface tears without substituting
an unrelated detached part. The previous global half-health body swap is no
longer the acceptance rule.

## Gun dispersion and tracer glow

The 0.5-degree full cone is checked over 20,000 deterministic samples for unit
length, maximum angle, lateral and vertical symmetry, and uniform solid-angle
distribution. A live gun-release test verifies that dispersion is applied only
once. Non-gun direction and replay tests continue to pass.

Daylight and night F/A-18D captures at twelve combat ticks show the same bright
warm tracer core with a soft amber halo. Full frames and enlarged details are
in `.local/tracer-review/`. These were inspected on NVIDIA RTX 4070/Vulkan;
the separate required display smoke also passed. Geometry tests cover both
side-on ribbons and nondegenerate end-on glows. Two optional GPU unit tests
remain ignored in the normal suite; captures validate the running renderer.
These checks establish the fitted presentation, not original tracer appearance.

Roster coverage also checks each identity's canonical gun mapping through the
actual simulation release path for reduced damage, normalized cone direction,
and deterministic results. The pass exposed two appearance omissions: light
marks skipped flat-colored surfaces such as Rafale wings, and address sampling
could omit the Su-27's few large wing faces. Those omissions are corrected and covered by synthetic tests and the
imported-resource validator. Roster captures additionally exposed the Su-35's
forward-set fins falling outside the longitudinal tail mask; elevated aft
surfaces are now included without adding fins to the F/A-XX. Their fitted rules are in
the behavior spec.

## Cannon stream review

The 2026-09-21 correction replaces simultaneous gun groups with physical bullets
at the fitted cadence in the [spec](../spec/damage-smoke.md#individual-cannon-rounds).
Source records for all six canonical guns yield 32 bullets per second. Tests
check one bullet per release, 3/4-tick gaps, 32 ammunition units and eleven
tracers in one second, fractional rates, release/repress boundaries and summed
target-class damage shares. Zero damage shares preserve the documented cockpit
critical-contact rule without inventing regional damage.

Root review checked the player firing path, actor-owned release bridge and
renderer. Corrections retain cadence/tracer ordinals across trigger pulls and
actor release groups, stop queued emissions from dead actors, and preserve each
queued group's target. These changes affect weapon mechanics only.

All thirteen selectable identities passed 80-tick application firing captures.
The F/A-18D emitted 22 bullets and consumed 22 ammunition units. GPU review
includes a daylight rear-view F-22 case matching the user's duplicate-tracer
report, plus night firing. Visible rounds use a single luminous ribbon instead
of stacked projectile art. Local evidence is `.local/rotation-cadence-review/`.
Audio cue generation follows each physical shot; hardware playback was not
auditioned. Runtime perception of the cadence remains a human testing item.

## Limits

Region boundaries, critical-hit rules, damage-patch placement, polygon cuts and
breakup thresholds are fitted. Gun strength is the requested opinionated tuning.
Retail damage transitions, trajectories and timing remain unverified. Local
rendering does not establish Windows or macOS runtime rendering. Damage-specific
flight forces, persistent wreck obstacles and wind-driven smoke are not added.

## Repository validation

Formatting, warnings-denied workspace Clippy, locked workspace tests and build,
68 Python tests, source and both executable asset guards, and all 170 document
headers passed. The Rust suite passed 946 tests with two optional GPU tests
ignored. The separate required `--smoke-test` presented successfully on NVIDIA
RTX 4070/Vulkan. The creator validator passed all twelve imported aircraft plus the selectable
F/A-XX concept. Each identity fires its imported gun and produces luminous
tracer geometry. All 117 regional appearance cases pass: left wing, right wing
and tail at 10%, 40% and 80% damage for every aircraft. Supported stores, mixed
fixtures and restart also pass. Roster evidence is in
`.local/roster-effects-review/`. Twenty-six GPU captures cover a firing
gun and severe tail damage for each selectable identity; the two roster
montages were visually inspected.

The F/A-18D gun probe passed all five object damage classes. Regional
collision tests cover translated and rotated aircraft, multi-tick cockpit
contact, swept relative movement and the exclusion of surface targets from
aircraft critical-hit rules. A pilot kill preserves the round's physical damage
amount instead of inventing enough structural damage to detach the nose.

The additional full F/A-18D `--combat-smoke` probe is not green: it passes all
five gun and AIM-120 damage-class cases, then its AGM guidance fixture expects
a launch against an ineligible aircraft target. Its older targetless assertion
also conflicted with existing automatic boresight launch behavior; that assertion
now checks the actual release gate. Missile gameplay was not changed. The AGM
fixture remains outside this damage pass and is not reported as validated.
