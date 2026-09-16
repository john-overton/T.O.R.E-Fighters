# Native tumble diagnostic start — 2026-09-15

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


**Origin:** static native FA source recovery. **Runtime scope:** diagnostic only.
The live F18/Rafale legacy and hybrid adapters do not call this new component.
No retail execution, trajectory comparison or live tumble acceptance is claimed.

The source EXE/SMS identity is unchanged from the
[flight-response baseline](flight-response.md#source-identity). Expected branch
behavior and state offsets are in the [native contract](../formats/native-flight.md#native-tumble-continuation--2026-09-15).
The initial translation is `tore-formats::flight_model::tumble`; native movement
composition uses the existing imported-table helpers. Full coupling remains open.

## Checks

```sh
cargo test --locked -p tore-formats tumble
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-tumble/native
```

Three synthetic tests pass: trigger/duration/direction boundaries and active
cancellation; progress/deadline/ground/spin behavior; explicit zero-roll random
input and stalled fall with tumble pitch suppression. These expected outputs
come from the reviewed branch arithmetic. They are not captured retail outputs
and do not validate all arithmetic domains or complete movement composition.

The static extraction runs against inert files and writes the reviewed slices
and source hashes under ignored `.local/native-tumble/`. Imported table motion
and whole-tick trajectory acceptance are subsequent gates. No retail resource
bytes or source-derived disassembly are committed.

Linux checks passed: formatting, warnings-denied workspace Clippy, workspace
tests and locked build; 24 Python tests; repository and app/extractor binary
asset guards. The reviewed static extraction succeeded (107 symbol spans,
3,829 symbols). No live simulation/rendering code was changed in this diagnostic
continuation, so no new GPU, controller or audible acceptance is claimed.
Windows/macOS checks were not run.

The subsequent [joined native departure stage](native-departure-stage.md) adds
imported-table composition and both-aircraft diagnostic probes. It supersedes
this initial pass's pending component-composition check, while live integration
and matched retail trajectories remain open.
