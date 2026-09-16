# Behaviour specifications

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

This directory holds **prose specifications of what a player experiences in
Fighters Anthology**. These are the parity target for the rebuild. Nothing in
this directory describes the original program's code.

Research mode writes these files. Implementation mode reads them and builds the
described behaviour idiomatically in the Rust crates, testing against the numbers
recorded here. See [AGENTS.md](../../AGENTS.md).

## What goes in a spec

The granularity test is:

> Would a player notice if this were different?

**Yes** — it belongs here, with numbers: speeds, rates, angles, times, ranges,
thresholds, what appears on screen, what the player hears, what the controls do,
what makes the difference between success and failure.

**No** — it is a source-notes footnote at the bottom of the file, or it is left
out. Call ordering, cache layout, RNG draw ordering, fixed-point rounding and
internal state machines are not player-visible and do not belong in the body of
a spec.

## Shape of a spec file

One file per feature a player would name. Suggested sections:

1. **What the player sees and does** — the behaviour in plain prose.
2. **Numbers** — a table of every value, with units and the conditions each
   applies under.
3. **Edge cases** — what happens at the boundaries, and what the player sees
   when something fails.
4. **Unknown** — what is not yet established, and the next research step.
5. **Source notes** — where the facts came from: build identity, routine or
   resource, and any branch that could not be resolved. Footnote, not the spec.

A spec describes **Fighters Anthology's** behaviour. A subsystem that is
deliberately ours — the input layer, for instance — is an opinionated design
and belongs in its own guide, not here. Record the departure from original
behaviour in that guide and label the component `opinionated`.

Link to the supporting research in [`../formats/`](../formats/) and to measured
evidence in [`../baselines/`](../baselines/) rather than restating it.

## Status

No specs are written yet. The research material they will be built from is in
[`../formats/`](../formats/) and in the frozen archives under
[`../research/`](../research/). The next feature scheduled for a research pass is
recorded in [`../parity-plan.md`](../parity-plan.md).
