# Behaviour specifications

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

This directory holds **prose specifications of what a player experiences in
Fighters Anthology**. These are the parity target for the rebuild. Nothing in
this directory describes the original program's code.

Research mode writes these files. Implementation mode reads them and builds the
described behaviour idiomatically in the Rust crates, testing against the numbers
recorded here. See [AGENTS.md](../../AGENTS.md).

## What goes in a spec

The granularity test is:

> Would a player notice if this were different?

**Yes**, it belongs here, with numbers: speeds, rates, angles, times, ranges,
thresholds, what appears on screen, what the player hears, what the controls do,
what makes the difference between success and failure.

**No**, it is a source-notes footnote at the bottom of the file, or it is left
out. Call ordering, cache layout, RNG draw ordering, fixed-point rounding and
internal state machines are not player-visible and do not belong in the body of
a spec.

## Shape of a spec file

One file per feature a player would name. Suggested sections:

1. **What the player sees and does**, the behaviour in plain prose.
2. **Numbers**, a table of every value, with units and the conditions each
   applies under.
3. **Edge cases**, what happens at the boundaries, and what the player sees
   when something fails.
4. **Unknown**, what is not yet established, and the next research step.
5. **Source notes**, where the facts came from: build identity, routine or
   resource, and any branch that could not be resolved. Footnote, not the spec.

A spec describes **Fighters Anthology's** behaviour. A subsystem that is
deliberately ours, the input layer, for instance, is an opinionated design
and belongs in its own guide, not here. Record the departure from original
behaviour in that guide and label the component `opinionated`.

Link to the supporting research in [`../formats/`](../formats/) and to measured
evidence in [`../baselines/`](../baselines/) rather than restating it.

## Status

Current specifications include aircraft ports, animation, atmosphere, ocean,
terrain and menus. See [`../parity-plan.md`](../parity-plan.md) for status and the
next feature. The [radar specification](radar.md) records the twelve-plane
capability survey, the installed visual and ECM records and the recovered range
and mode rules; the shared component built against it has its own
[guide](../radar.md). [Instrument window bezels](instrument-bezel.md) records
each aircraft's instrument window frame, its colours and its geometry.
[Mission debrief](debrief.md) specifies the five result pages, their counting
rules and the Quick Mission outcome. [Ejection](ejection.md) specifies player confirmation, pilot survival and the
fitted AI recovery assessment, including the requested safety and chance rules.
[Flight music](flight-music.md) specifies which recorded score plays during
flight, its rank and when a change is immediate or waits for a phrase boundary.
[Cockpit voice](cockpit-voice.md) specifies the player's crew remarks: dogfight
coaching, G sounds, fuel calls and missile warnings, and who is labelled speaking.
Supporting research lives in [`../formats/`](../formats/), measured evidence in
[`../baselines/`](../baselines/), and frozen archives in [`../research/`](../research/).
