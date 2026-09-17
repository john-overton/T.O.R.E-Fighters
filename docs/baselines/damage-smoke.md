# Aircraft damage and smoke validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-17, Linux and locked Rust toolchain. Source archive
identities, dimensions and shape observations live in the
[resource review](../formats/objects-and-shapes.md#combat-damage-and-smoke-resource-review);
[behavior and fitted constants](../spec/damage-smoke.md) are specified separately.
No retail bytes are committed. Source excerpts, inventories and screenshots
remain in ignored `.local/damage-smoke/`.

All twelve imported aircraft passed creator/loadout validation, including
normal weapon loads, 29-fixture restart, a distinct nonempty damaged body,
finite geometry, falling-wreck geometry and reset to intact appearance.
Synthetic tests cover ignition silence, emission during motor burn, coast
without new puffs, residual smoke after missile removal, aircraft health at
51% versus 50%, smoke from airborne wrecks, cessation on ground contact, no gun
smoke, puff cadence/lifetime/rise, capacity eviction and reset. Smoke state also
participates in the existing 30/60/144 Hz deterministic simulation comparison.

F/A-18D and Rafale C default-store combat smokes passed all five damage classes.
Captured F/A-18D/Rafale damaged bodies and AIM-120 powered smoke were inspected
on NVIDIA RTX 4070/Vulkan. Smoke transparency uses the source's index-255 key;
the initial opaque background found during visual review was corrected.
Aircraft smoke cadence was fitted to 60 puffs/second for a continuous plume.
Load Ordnance was captured after removing the dummy-flight description.

Remaining limits: unverified original B/D selection/trajectories, retail damage thresholds,
measured original puff schedules, wind advection of smoke, smoke sensor effects,
or damage-specific flight-force changes. Variant scale parity and Windows/macOS
runtime rendering remain unverified. Existing flight adapters and damage
amounts are unchanged; damaged straight-flight fixtures still do not maneuver.

The subsequent debris tests verify full velocity inheritance, gravity and swept
contact against sloping terrain, one fragment per damaged object, removal and
exactly one ground effect at contact, effect expiry and reset. All twelve source
aircraft are also checked for distinct, nonempty detached geometry. `GRDLRGA.PIC`
was inspected as a 256x252 ground-explosion sheet before selecting its fitted
small impact presentation. Ground objects, AI and persistent debris obstacles
are not introduced.

F/A-18D captures show the original nose section detached at tick 90 (0.75 s).
A 70-foot-AGL capture at tick 260 reports zero remaining pieces and one impact,
with the small ground animation visible below the aircraft. Evidence:
`debris-falling.png`, `debris-impact.png` and their logs in the local evidence
directory. The piece is not left resting on terrain. These are controlled
presentation fixtures, not a comparison against original execution.

## Final acceptance

All required AGENTS checks passed: formatting, warnings-denied workspace Clippy,
474 Rust tests, locked workspace build, 40 Python tests, source and both binary
asset guards, and 147 documentation headers. The required
`cargo run --locked -p tore-app -- --smoke-test` presented successfully on
NVIDIA RTX 4070/Vulkan. No required Linux check was unavailable.

All twelve aircraft completed 230 default-station/five-damage-class cases and 46
serialized combat replay checks, including AGM65G and AS7. Logs and tapes are
under `.local/damage-smoke/`; no retail derivatives were added to Git. This
validation covers fitted game behavior and source-resource use, not original
runtime parity. Windows/macOS runtime checks and retail comparison were unavailable.
