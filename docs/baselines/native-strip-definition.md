# Bounded STRIP definition checkpoint

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `4447e90`. **NE-01.1a metadata gate complete**; full
NE-01.1 resource/placement closure and NE-03.1 live contact remain open.
[Contract](../formats/native-strip.md), [frozen plan](../research/native-environment-systems-plan.md).

```sh
cargo test --locked -p tore-formats strip::
cargo run --locked -p tore-formats --example native_strip -- .local/native-environment/land-discovery/FA_2.LIB/RUNWAY.SH .local/native-environment/land-discovery/FA_2.LIB/STRIP.OT
```

The reviewed original definition parses as shape **RUNWAY.SH**, flags **0x208021**.
Its existing [archive/resource hashes](native-land-foundation.md) remain the source
identity; matching the basename alone does not identify a media edition. The
shape diagnostic still finds **23 boxes**, all twelve required lookup IDs, and
the incomplete projector's **63 faces / _RUNWAY.PIC**. No new drawing, palette,
visual or collision acceptance follows from that output.

Two synthetic tests cover alternate pointer labels, retained unknown scaling
markers/raw values, high-bit flags, wrong sizes/classes/identities/selectors,
additional shape dependencies, absent/unresolved references, path/name limits,
multiple shape strings, missing terminator and the 1 MiB input cap. Fixtures are
synthetic; no retail data or executable code is committed. The reader does not
extend extraction profiles or initialize simulation state.

Linux formatting, warnings-denied workspace/all-target Clippy, **354 Rust tests**,
locked build, **24 Python tests**, repo/app/extractor asset guards and whitespace
checks pass. Logs: `.local/native-environment/strip-definition-checks/`.
Existing native airborne replay passes **28 cases / 33600 updates** for both
reviewed PTs using the [foundation command](native-land-foundation.md) and
`strip-placement-source/tables`. Creator, viewer and both native cockpit startup
smokes pass on NVIDIA RTX 4070 / Vulkan / Immediate. No rendering changes or
new handling/performance claims. Physical input/audio, Windows/macOS build/runtime
and retail comparison remain unavailable/not run; evdev warnings persist.

Remaining: full type-load/placement semantics, template consumers/ownership,
scheduling/service RNG and E004 drawing closure; then staged E001/E002 queries
and late-failure rollback. No new runtime aircraft/contact eligibility, AI or
carrier activation. John authorized pushing the preceding tested work; origin/main
advanced through `4447e90` before this slice.
