# Envelope instrument validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Scope and result

Implementation pass, 2026-09-21, Linux display-capable host. Behavior contract:
[envelope instrument](../spec/envelope.md). No simulation, flight adapter or
autonomous behavior was changed. No retail executable was run.

Reviewed the local retail manual's printed pages 90-92 and the three supplied
screenshots. Manual identity and unresolved presentation values are recorded in
the spec, not duplicated here.

All 13 imported profiles produced headless panel captures: F/A-18D, Rafale C,
F-14D, A-4E, X-31 EFM, MiG-29, Su-27, MiG-21, Su-25, MiG-23, Su-35, F-22A
and the separately authored F/A-XX. Visually reviewed the contact sheet at
`.local/envelope/roster.png`: filled bands and the marker remain inside each
plot, and altitude, G and knots readouts are legible. Each profile uses its own
bounds. This is a presentation check, not a claim of retail flight parity.

Example capture command:

```sh
target/debug/tore-app --aircraft f18 --panel-snapshot .local/envelope/f18.ppm --instrument-page 1
cargo run --locked -p tore-app -- --free-flight --no-audio --instrument-layout small --instrument-page 1 --capture-flight .local/envelope/small.ppm --smoke-test
```

The small-window GPU capture also passed visual review. Its cyan marker and the
headless yellow marker confirm distinct phases in real output. Synthetic tests
check the three-color tick cycle, current-G rounding, filled-band membership,
shared comparison coordinates, target-advantage shading, scaling and plot/frame
clipping. The full color cadence and comparison against an acquired live target
were not manually exercised in an interactive flight.

## Checks

Passed: `cargo fmt --all -- --check`, workspace Clippy with all targets and
warnings denied, workspace tests, workspace build, Python tool tests, source
asset check, both executable asset checks, and documentation checks. Cargo
commands used `--locked` where supported. Both the standard GPU smoke test and
the free-flight small-window smoke/capture completed successfully.

All local captures remain ignored. No retail image, font or audio was added to
the repository. Exact retail color indices, marker timing and scale rules remain
fitted as documented in the spec.
