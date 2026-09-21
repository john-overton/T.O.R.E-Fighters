# Smoke and engine contrail validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21, Linux, locked Rust toolchain, NVIDIA RTX 4070
with Vulkan. The [behavior spec](../spec/damage-smoke.md) owns the requested
rates, dimensions and fade distances. These are opinionated presentation
changes, not a claim of retail parity.

Synthetic tests verify missile and damage-smoke emission counts and spacing,
missile radius at birth and during growth, powered-motor and damage gates,
expiration and capacity. Contrail tests verify two outlets at ten puffs per
second, constant initial opacity, the four-mile fade boundary, half opacity at
4.5 miles and removal at five miles. A turn changes the path direction between
checks, exercising accumulated travel rather than straight-line distance.
Removing sources clears residual contrails within four seconds. Onset tests
check the 30,000-35,000-foot range, stable per-aircraft values and variation
between aircraft and sorties.

All required checks passed: formatting, workspace Clippy with warnings denied,
workspace tests, workspace build, 68 Python tests,
source and both executable asset scans, and documentation headers. The required
`cargo run --locked -p tore-app -- --smoke-test` presented successfully.

Local captures used `--free-flight --no-audio --combat-probe-ticks 2400
--capture-flight PATH --flight-view 2 --flight-look 180,0 --flight-zoom 0.5`.
With `TORE_FLIGHT_AGL=40000`, the F/A-18D shows pale puffs following its engine
outlets. With the default low-altitude start, the same view has no contrail.
These captures verify rendering and the broad altitude gate, not measured
trail length or an exact threshold crossing. The distance boundaries and
onset range are checked by synthetic tests. Images and logs remain ignored in
`.local/smoke-contrails/high.*` and `.local/smoke-contrails/low.*`.

Other aircraft attachment points were not visually inspected in this pass.
Fitted fallback locations and the capacity limit are documented in the spec.
Combat-service replay excludes the new cosmetic contrail history, since existing
tapes do not record engine power. No flight adapter or autonomous policy changed.
