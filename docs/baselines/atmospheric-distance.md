# Atmospheric distance validation, 2026-09-16

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode validation of the [atmospheric distance blend](../spec/atmospheric-distance.md).
Build: `08f94b4` plus this working change. Linux, NVIDIA RTX 4070, Vulkan.

Passed workspace formatting, all-target warnings-denied Clippy, locked Rust
tests and build, 40 Python tests, source and both executable asset guards,
documentation headers and the display smoke test:
`cargo run --locked -p tore-app -- --smoke-test`.

GPU captures used `target/debug/tore-app --weather-condition N --capture-terrain
.local/atmosphere/NAME.ppm --no-audio` with `TORE_WEATHER_VIEW` set to
`1070000,ALTITUDE,590000,45,0` (feet and degrees).

| Capture | Condition | Altitude | Result |
| --- | --- | --- | --- |
| after / revised | 0 | 5,000 | Matched initial/revised pass: nearer terrain clearer, sky texture dissolves into horizon without the blue backdrop strip |
| revised-cloud | 1 | 5,000 | GPU capture passed; source cloud whiteout remains |
| revised-above | 1 | 30,000 | GPU capture passed above cloud layer |
| revised-dawn | 3 | 5,000 | GPU capture passed with dawn palette |
| revised-stepped | 0 | 5,000 | TORE_WEATHER_SMOOTH=0 capture matches the previous stepped frame exactly |

Local comparison image: `.local/atmosphere/revised-comparison.png`, initial
haze left and revised haze right. Original-art captures are ignored and not
committed. Weather captures include existing source remaps and do not isolate
the added moisture contribution. Source palette haze is retained even inside
the new 50-statute-mile clear-air threshold.

This is authored tuning, not measured retail parity or a physical visibility
model. No performance benchmark, continuous climb across band boundaries,
Windows/macOS run or exhaustive weather/time/terrain matrix was performed.
Scattered cloud-sheet placement alone does not alter moisture.
