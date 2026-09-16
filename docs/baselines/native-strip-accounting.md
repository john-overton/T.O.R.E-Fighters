# STRIP trailing events and death accounting checkpoint

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


2026-09-15, following `5fd23d7`. **NE-00.1q completes bounded source recovery
only**, not a runtime event, score or damage implementation.
[Contract](../formats/native-strip.md#trailing-events-and-death-accounting--ne-001q),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-accounting-source
```

Five aligned regions cover the two post-merge event consumers, the selected
STRIP scoring exclusion, death-statistics caller and damage-attribution prefix.
The subtype selector/target tables are read as inert data, outside reviewed
code bounds. Fresh and repeated extraction pass: **218 reviewed regions**,
107 selected symbol spans, 3829 symbols; reviewed EXE/SMS hashes unchanged.

The source distinguishes unscheduled-event delivery, speech observation before
subtype dispatch, and callback scratch versus stored-object tests. It excludes
the scoring body for the selected kind-0/controller-clear STRIP while preserving
separate credited-ID counters and notification dependencies. Attribution +0x76
is distinct from placement alias +0x74. No imported callback is executed and no
new helper or runtime activation is introduced.

Fresh Linux validation: **366 Rust tests**, **26 Python tests**, fmt,
workspace/all-target Clippy with warnings denied, locked build, repo/app/extractor
asset guards, whitespace and changed-document local file-target checks pass.
File-target checks do not validate anchors. Fresh creator smoke presents on
RTX 4070 / Vulkan / Immediate; logs are ignored at
`.local/native-environment/strip-accounting-{tests,gpu}.log`.

The immediately preceding `5fd23d7` both-aircraft native live replay (28 cases /
33,600 updates) covers the unchanged runtime producer; it was not rerun for this
source-only slice. No new runway, collision, damage, viewer/cockpit, handling or
performance acceptance is claimed. Physical input/audio were not manually
tested; Windows/macOS build/runtime and retail comparison remain unavailable/
not run. No AI, carrier/default activation or push.

Next: downstream 0x471400 notification, later dead-object service/full removal,
required effects/resources and comment/world ownership before staged E001/E002.
Parent phases remain researching.
