# Gun pipper and target-cue validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. Linux with the locked Rust toolchain, NVIDIA
RTX 4070 and Vulkan. [Behavior, manual provenance and fitted constants](../spec/gunsight-targeting.md)
are specified separately. Local evidence is under `.local/gunsight-ground-review/`.
No retail document, screenshot or extracted resource is committed.

## Manual and simulation checks

The local FA manual's printed pages 83 and 86 were rendered and inspected,
including the designator diagram and gun range-arc figures. The gun fallback
is 1,000 feet; older USNF-remake meter-based notes are not the FA specification.

Synthetic tests validate gravity/drop, imported scalar launch-speed dependence,
moving-target radar lead, no invented vector velocity inheritance, deterministic
results, exact range-arc anchors, and all selectable gun identities. The imported
creator validator independently loads all twelve aircraft plus F/A-XX, fires
each gun and verifies a finite 1,000-foot solution and usable source maximum range.

Projection tests cover front/right/left/above/below/behind, edge transitions,
full bank and steep pitch, invalid/coincident positions, and zoom. A live test
loses sensor selection while retaining the display target, proves that no weapon
observation is supplied by that display selection, and verifies destruction and
explicit clear remove the cue.

## Display checks

GPU captures inspect F/A-18D base mode, radar-ranging mode, right/up/rearward
chevrons, SAFE, Rafale radar mode, and a tall window. The first SAFE capture
exposed flight text overlapping ammunition; safe guns now reserve the weapon
readout area and the corrected capture was inspected. Existing missile HUD and
a banked target-cue capture are also checked. The required `--smoke-test` presents
successfully. Captures establish fitted layout and integration, not exact retail
pixels, equations or Windows/macOS rendering.

## Ground-wind checks

The [ground-start baseline](ground-start.md) records the tire-grip rule. Synthetic
checks hold exact horizontal position for ten seconds with brakes applied and
released and retain existing airborne wind-advection coverage. Current
weight-class thresholds and rollout checks are in the
[ground-start baseline](ground-start.md). All thirteen selectable aircraft also run 1,200 headless
ticks at airport 2 with `TORE_WIND=90,40`: every run finishes at zero knots,
runway height, without a crash.

## Required checks

Formatting, warnings-denied workspace Clippy, locked workspace tests and build,
68 Python tests, source and both binary asset guards, documentation headers,
and diff whitespace checks pass. The Rust suite passes 928 tests; two optional
GPU unit tests remain ignored, with explicit display smoke and captures run.
The earlier full combat-smoke AGM fixture limitation remains documented in the
[damage baseline](damage-smoke.md); no missile gameplay was changed here.
