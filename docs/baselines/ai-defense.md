# AI missile defense and RWR validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. This feature pass covers the [defense contract](../spec/ai-awareness.md#missile-awareness-and-defense)
and [RWR presentation](../spec/rwr.md). The source base is `5b2241f`. Committed
fixtures are synthetic; imported media and rendered derivatives remain local.

## Behavior covered

Guidance-dependent warnings, frozen threat snapshots, visual-only passive
missile detection, own-ordnance plots, skill-dependent defensive commitment,
jink/notch/dive, bounded countermeasure bursts, actor-owned radar support and
actual active-seeker acquisition are connected to the live mission and RWR.
The explicit compatibility weapon mode retains its prior steering and B47 AI
warning route.

## Validation

Validated on Linux with Rust 1.91.1 and locked dependencies.

| Check | Result |
| --- | --- |
| Formatting and workspace Clippy with warnings denied | Passed |
| `cargo test --workspace --locked` | 1,077 passed; 3 existing ignored tests |
| `cargo build --workspace --locked` | Passed |
| Python tool tests | 68 passed |
| Source and both executable asset scans | Passed |
| Documentation headers and diff whitespace | Passed |
| Imported roster probe, 3,600 ticks per aircraft/skill case | All 52 cases passed, zero dropped launches |
| Display smoke test | Passed on NVIDIA GeForce RTX 4070 using Vulkan |

Focused cases verify silent active midcourse, actual pitbull, immediate S
warnings, visual-only I/E acquisition, failed receivers, full 240-tick loss
grace, unknown/receding-threat fallback, Novice versus Ace timing, hold-fire
self-preservation, two-device debit at ticks 0 and 30, mixed visual bursts,
owner-specific radar support and no pre-pitbull chaff decoy. The existing
compatibility and deterministic replay tests also pass.

## Visual evidence and limits

The manual's printed pages 94, 95 and 131 were rendered and inspected. Runtime
capture with original art/font assets uses:

```sh
target/debug/tore-app --live-fire --aircraft f18 --weapon-slot 2 --combat-command target-distance:100000 --combat-command target-radar --combat-probe-ticks 720 --instrument-page 5 --instrument-layout small --capture-flight .local/ai-defense/rwr-emitter-missile-separated.ppm --no-audio --smoke-test
```

The inspected image shows the original frame/font, 50 NM range label, R
indicator, filled enemy emitter diamond and a distinct steady own-missile dot.
The close incoming CLI fixture overlays its missile on the center aircraft
mark after the probe setup, so those captures do not independently establish
both flash phases. Exact flash boundaries and symbol shapes have synthetic
raster tests. No running retail comparison or human encounter-balance review
was performed.

Flight response estimates, visual CPA classification, stale grace, own-ordnance
telemetry and active-seeker notch response are authored/fitted as specified.
Cloud/night visual limits and weapon-specific notch calibration remain open.
Passive unknown guidance does not acquire an invented IR label or decoy
susceptibility. Prelaunch painting/tracking emitter states have drawing support
but no fabricated sensor producer. Player audible-warning changes are out of
scope. Mission engagement policies are the next stage.
