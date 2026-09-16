# Airport and callback ownership source checkpoint

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


2026-09-15, following `535d227`. **NE-00.1h completes a bounded source ledger
only**; E016/E019/E020 and the parent producer remain open.
[Contract](../formats/native-strip.md#airport-ownership-and-comment-preflight--ne-001h),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-ownership-final
python3 -m unittest discover -s tools -p 'test_*.py'
```

The same reviewed EXE/SMS hashes produce **146 reviewed regions**, 107 selected
symbol spans and 3829 symbols. Eleven added aligned regions cover airport lookup,
airport service reset, three template predicates, comment selection/finish,
speech-buffer reset and separate actor-list reset/removal/registration. The
complete comment action body is deliberately not labeled reviewed. No imported
module is executed and no callback or autonomous behavior is translated.

Findings: comment suppression still clears two buffer starts; eligible actors
come from a separate ordered 60-entry ID list; airport records have mutable
service fields beyond their construction template. The source ledger records
short-circuit order, rank ties, widths, partial resets and current-object switches.
Native unsafe pointer assumptions must become explicit host validation. No
empty-list assumption follows from the exclusion of autonomous behavior.

Linux formatting, warnings-denied workspace/all-target Clippy, **360 Rust tests**,
locked workspace build, **24 Python tests**, repo/app/extractor asset guards and
whitespace checks pass. These are regression checks; no new runtime behavior or
synthetic simulation acceptance is claimed for this source-only slice. Repeat
static extraction passes. Creator smoke passes on RTX 4070 / Vulkan / Immediate.
The unchanged 28-case both-aircraft native replay passed in the immediately
preceding [placement slice](native-strip-record.md); no new replay producer was
added here. Logs for workspace/GPU regressions remain in ignored
`.local/native-environment/strip-record-checks/ownership-*.log`; source artifacts
remain in `.local/native-environment/strip-ownership-final/`.

No new runway drawing, contact, handling or performance acceptance. Physical
input/audio, Windows/macOS build/runtime and retail comparison remain unavailable
or not run. Next: remaining template callbacks/default consumers, actor/attachment/
clock/speech producers and required service bodies, plus independent E004 drawing
closure before staged E001/E002 world/query state. No push or carrier activation.
