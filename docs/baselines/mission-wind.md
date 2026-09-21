# Signed mission wind validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-09-21, Linux. The [behavior spec](../spec/mission-wind.md)
and [format contract](../formats/weather.md#turbulence-generator-and-wind-line-contracts)
separate the accepted behavior from source evidence.

## Reproduction and cause

Before the fix, each of these commands failed with `mission wind outside source
range` while building the theater, before flight or wing population:

- `target/debug/tore-app --theater TVIET --free-flight --smoke-test`
- `target/debug/tore-app --theater VLA --free-flight --smoke-test`

The metadata reader already preserves signed integers. The simulation wind
constructor incorrectly rejected every negative heading. Local extracted
`FA_2.LIB` metadata has these input hashes:

| Input | SHA-256 |
| --- | --- |
| `TVIET.MM` | `9834e99d89fe6e6cc081e04df040ba444e137005b70a78d17a3e34c4cadc8847` |
| `VLA.MM` | `2830762409368a736fbc214d7f0bb2f0298cb2cef8bbf21a5aafd5849e0ba3ba` |

The files were inspected in ignored `.local/airport-research/layouts/FA_2.LIB/`.
No retail mission bytes are added to tests. The synthetic regression covers
metadata parsing through wind construction, signed direction and speed, explicit
calm, inclusive bounds, and invalid values without consuming random state.

## Validation scope

Both commands above now present flight successfully. The standard
`cargo run --locked -p tore-app -- --smoke-test` also passes. Formatting, Clippy
with warnings denied, locked workspace tests (988 passed), locked workspace
build, 68 Python tests, documentation headers and source/binary asset checks pass.
Before/after logs remain local in `.local/vietnam-wind-*.log` and
`.local/vla-wind-*.log`. The initial test pass found an obsolete assertion that
-1 degree was invalid; it now checks -361 degrees and accepts valid negative
headings through the complete weather configuration boundary.
The user's exact 15-aircraft-per-side, 40,000-foot, 20-mile configuration was not
needed to reproduce this loader failure. No changes to autonomous behavior are
part of this fix.
