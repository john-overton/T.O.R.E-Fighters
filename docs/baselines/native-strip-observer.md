# STRIP queue routing and speech observer checkpoint

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `61eb266`. **NE-00.1n completes a bounded source ledger**,
not event/speech execution. [Contract](../formats/native-strip.md#queue-routing-and-speech-observation--ne-001n),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-observer-source
```

Thirteen independently aligned ranges establish queue reset, remaining enqueue
caller segments, sender-scoped observer/default speech, buffer append/output,
sample sequencing and mission interceptor name/selection. Fresh and repeated
extraction pass: **202 reviewed regions**, 107 selected spans, 3829 symbols and
unchanged reviewed EXE/SMS hashes. Referenced strings `code`, `.MC`, comma and
`.5K` were inspected as bounded inert data from the reviewed PE sections. No
native module was executed and no extracted retail bytes enter the repository.

New evidence identifies reverse recipient iteration, queue-full notification,
separate queue/string/timer/handle resets, unconditional emitted-speech deadline
updates and ordered sample handle feedback. E022 records the required owned
output boundary. Group expansion, transport, naming/formatting, playback and
mission lifecycle remain explicit dependencies; no source closure is claimed
for their downstream bodies. No runtime or importer behavior changed.

Linux validation: **363 Rust tests**, **26 Python tests**, fmt, workspace/all-target
Clippy with warnings denied, locked build, repo/app/extractor asset guards,
whitespace and changed-document local file-target checks pass. Link checks do
not validate anchors. Fresh creator smoke presents on RTX 4070 / Vulkan /
Immediate. Test/GPU logs: `.local/native-environment/strip-observer-{tests,gpu}.log`.
The unchanged simulation reuses the immediately preceding [movement slice's
28-case / 33,600-update both-aircraft replay](native-strip-movement.md); it was
not rerun or presented as new event-service evidence.

No new runway/handling, viewer/cockpit or performance acceptance. Physical input/
audio were not manually tested. Windows/macOS build/runtime and retail comparison
remain unavailable/not run. No AI, carrier/default activation or push.

Next: OBJEventProc damage/cleanup and selected local-service/clock producers,
then remaining comment/output ownership. The parent query/world gates remain
open; PM control stays ignored and uncommitted.
