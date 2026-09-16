# Sun glow validation, 2026-09-16

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode validation of the [opinionated sun glow](../spec/sun-glow.md).
Build: `02c7711` plus this working change, Linux, NVIDIA RTX 4070, Vulkan.

Passed formatting, warnings-denied workspace/all-target Clippy, locked workspace
build and all 369 Rust tests, 40 Python tests, source and both binary asset
guards, documentation headers, and `git diff --check`. The required
`cargo run --locked -p tore-app -- --smoke-test` passed on the display.
An initial shader validation failure from an incorrectly placed edit was fixed
before the successful GPU captures and final checks.

Ignored captures live in `.local/sun-glow/`. Each used `TORE_SUN_GLARE=0`,
`--free-flight --smoke-test --no-audio --capture-flight PATH`, and:

| Capture | TORE_WEATHER_TIME | --flight-look |
| --- | --- | --- |
| dawn.ppm | 07:30 | 100,5 |
| dusk-1830.ppm | 18:30 | -100,5 |
| opposite.ppm | 07:30 | -80,5 |
| dusk.ppm | 19:00 | -100,5 |

All four GPU captures passed. Visual inspection found warm light around the
visible dawn and 18:30 sunset discs, and a cooler opposite-facing sky. At 19:00
the disc was not visible, so that capture does not validate its halo.
These are authored presentation checks, not retail comparisons. The original
sun rings remain visibly stepped inside the soft halo. Other theaters, fog,
individual cloud-sheet occlusion, noon, bank/zoom combinations and Windows/macOS
execution were not visually checked in this pass. Twilight outside the source sun interval
remains outside this implementation's scope.

## Angular cloud-lighting validation

The same working build and host passed all checks listed above after adding
per-pixel cloud lighting. Ignored artifacts are in `.local/cloud-glow/`; all
captures used the same base flags as above. `before.ppm` uses the sun-halo-only
build; `after.ppm` uses angular cloud lighting, both at 07:30 with look 100,15.
`comparison.png` places before on the left and after on the right. Inspection
shows a broader warm gradient while retaining the original texture detail.

`early.ppm` uses 07:05 and look 100,15; `sunset.ppm` uses 18:30 and look -100,15.
Both rendered successfully and were inspected. The early sky has no visible
cloud texture in this source weather record, so it establishes early-sun
rendering, not cloud lighting at that time. The sunset cloud texture shows the
same directional warm gradient. The source's discrete texture schedule is
unchanged; this change does not blend between different weather textures.

`opposite.ppm` uses 07:30 and look -80,5. ImageMagick absolute-error comparison
against `.local/sun-glow/opposite.ppm` reports zero changed pixels. All captures
used disabled lens flare/whiteout so those effects cannot explain the glow.
Finite cloud-sheet lighting and distance-haze attenuation are implemented, but
these captures visually establish sky-deck lighting only. No full-day animation
or frame-time benchmark was run.
