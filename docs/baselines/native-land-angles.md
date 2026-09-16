# Native contact angle arithmetic checkpoint

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


2026-09-15. NE-00.1c / E008, following `66a7999`.
**Native source and diagnostic translation/tested; runtime unconnected; retail unavailable.**
[Contract](../formats/native-land-contact.md#candidate-and-requested-heading-angles--ne-001c),
[frozen plan](../research/native-environment-systems-plan.md).

Two aligned slices (`0x411a40..0x411aec`, `0x4c6c30..0x4c6d5f`) extend the
hash-gated static pass to **94 reviewed regions**. EXE/SMS identities and the
three external tables are unchanged. Candidate normal reduction, native atan/
square-root angle construction, pitch clamp and requested-heading projection are
pure typed helpers. No world cache, instance loader, native module execution or
live flight path is added.

The new synthetic test checks axis normals, negative-pitch clamp, a non-axis
normal with explicit reduced words and synthetic table expectations, and cardinal
heading projection. Existing geometry tests still cover malformed/degenerate
and exact boundary inputs. Synthetic tables are test inputs, not retail data.

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/angle-source
cargo run --locked -p tore-formats --example native_land_geometry -- .local/native-environment/angle-source/tables .local/native-environment/land-discovery/FA_2.LIB/UKR.T2 .local/native-environment/land-discovery/FA_2.LIB/RUNWAY.SH
```

The probe now takes a **table directory** and reads sine, atan and sqrt seeds.
It repeats the prior 166,400 geometry cases and **332,800 candidate/projection
cases** (headings 0 and 0x4000), identically. Flat upward normals produce zero
pitch/roll at both headings. Projected pitch spans PA -7,126 to 8,037.
This is deterministic imported-data diagnostic coverage, not independent retail
comparison, a live query trace or a validated runway placement.

Validation on Linux: formatting, warnings-denied Clippy, **346 Rust tests**,
locked workspace build, **24 Python tests**, all three asset guards and diff
whitespace check pass. The existing airborne probe passes 28 cases / 33,600
updates. Creator, viewer and F18/Rafale native cockpit startup checks pass on
NVIDIA RTX 4070 / Vulkan / Immediate. Logs:
`.local/native-environment/angle-checks/`. No new rendering/handling/performance
claim; physical input/audio, Windows/macOS build/runtime and retail comparisons
remain unavailable/not run. The evdev-controller warning persists.

E008 arithmetic is complete. E003/E005 STRIP initialization/placement/callbacks,
E004 full visual/resource closure, E001/E002 staged world queries/cache/RNG and
late-failure rollback remain prerequisites for a reviewed live land branch.
Both aircraft still stop at unsupported contact; carrier remains gated.
