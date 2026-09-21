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
rates, dimensions and fade timing. These are opinionated presentation
changes, not a claim of retail parity.

Synthetic tests verify missile and damage-smoke emission counts and spacing,
missile radius at birth and during growth, powered-motor and damage gates,
expiration and capacity. Contrail tests verify two outlets at ten puffs per
second, full initial opacity through one minute, half opacity at 1:30 and
removal at 2:00. Puffs emitted at different speeds retain equal opacity at equal
ages. Removing their sources leaves the full lifetime intact; world positions
remain fixed. Capacity tests retain a fading history beyond the old 8,192-puff
limit and check the 72,000-puff budget for 60 outlets at ten puffs/s for 120
seconds. Camera tests retain overlapping edge puffs, reject offscreen/behind
puffs and verify the combined instance capacity. Onset tests
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
trail length or an exact threshold crossing. The lifespan/fade boundaries and
onset range are checked by synthetic tests. Images and logs remain ignored in
`.local/smoke-contrails/high.*` and `.local/smoke-contrails/low.*`.

## Cloud-matched lighting

John's dusk screenshots `screenshot-2026-09-21_15-19-21.png` and
`screenshot-2026-09-21_15-19-49.png` were inspected from his Pictures directory.
The old pass baked `SMOKE.PIC` through the aircraft palette at load time and only
reduced opacity with distance. The reviewed local sheet has no embedded palette.
The current pass retains indices and uses the live weather palette/remaps plus
the same cloud lighting function for all three smoke families.

The explicitly run GPU regression is:
`cargo test --locked -p tore-app
 gpu_all_smoke_families_follow_live_palette_without_lighting_cutouts -- --ignored`.
It passes using synthetic artwork. Each smoke family is rendered through the
production shader while the bound palette changes from daylight to dusk to
night and back. Pixel checks verify the expected linear-light color, unchanged
coverage and zero RGB/alpha in transparent sprite corners. The contrail case
renders at age 1:30 to check the half-opacity fade through the production shader.
No sprite reload is needed for a lighting change. The test uses the instanced
vertex path, which uploads 24 bytes per puff rather than six 32-byte vertices.
This is an eightfold reduction per visible puff; the full simulation history is
kept when a camera culls it. No full-mission CPU/GPU performance measurement is
claimed.

In-game captures at the sunset preset (19:01) and with `TORE_WEATHER_TIME=19:20`
were inspected for all three families. Contrails use the high-altitude command
above; aircraft smoke uses `--weather-condition 4 --damage-preview 0.6
--flight-view 2 --capture-flight PATH`; missile smoke uses `--weather-condition 4
--live-fire --weapon-slot 2 --combat-command seeker-mode --combat-probe-ticks 100
--flight-view 2 --capture-flight PATH`. Captures are ignored at
`.local/smoke-lighting-{contrail,damage,missile}-{dusk,night}.ppm`. The late captures
show darker puffs rather than the previous bright white night trails. Their
lighting changes together with the scene without changing the emission or
lifespan rules in the spec.
These are static visual checks, not a retail comparison or continuous-motion test.

The instanced dusk capture `.local/contrail-instanced-dusk.ppm` matches the earlier
20-second dusk capture exactly: ImageMagick reports zero differing pixels. This
checks that instancing and view culling preserve the existing visible result
before the new fade begins. The default rendering smoke test also passes.

The two-minute fade is checked numerically and through the GPU test, not by
waiting two minutes in an interactive flight. Other aircraft attachment points
were not visually inspected in this pass.
Fitted fallback locations and the capacity limit are documented in the spec.
Combat-service replay excludes the new cosmetic contrail history, since existing
tapes do not record engine power. No flight adapter or autonomous policy changed.
