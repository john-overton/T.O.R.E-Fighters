# STRIP collision dispatch and death marking checkpoint

2026-09-15, following `9baf9fa`. **NE-00.1o completes bounded source recovery and
a diagnostic collision predicate**, not native collision damage or removal.
[Contract](../formats/native-strip.md#collision-hit-dispatch-and-death-marking--ne-001o),
[living plan](../native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-hit-source
cargo test --locked -p tore-sim collision_ratio
```

Six new aligned regions recover hit dispatch, base/delegated selectors, generic
collision hit, death marking and zero-crater entry. Fresh and repeated extraction
pass: **208 reviewed regions**, 107 selected symbol spans, 3829 symbols and
unchanged reviewed EXE/SMS identities. The death-output jump table begins after
the accepted code bound and is not decoded as instructions in that region.

The collision helper checks source type hit-point ratio, not event amount or
remaining health. Tests cover below/equal threshold, signed type words, extreme
word values and explicit zero-denominator failure. Source review establishes
victim current-object ownership, conditional notification and zero-crater
exclusion for the selected original definition. No lookup, notification, damage,
removal, runtime event or imported callback is executed by this helper.

Linux validation: **364 Rust tests**, **26 Python tests**, fmt, workspace/all-target
Clippy with warnings denied, locked build, repo/app/extractor asset guards,
whitespace and changed-document local file-target checks pass. File-target checks
do not validate anchors. Both-aircraft native live replay passes **28 cases /
33,600 updates** with the current tables, F18.PT = F/A-18D and RAFALE.PT = Rafale C.
Fresh creator smoke presents on RTX 4070 / Vulkan / Immediate. Logs:
`.local/native-environment/strip-hit-{tests,replay,gpu}.log`.

Replay/GPU checks cover unchanged airborne/creator behavior, not new crash,
runway, viewer/cockpit or performance acceptance. Physical input/audio were not
manually tested. Windows/macOS build/runtime and retail comparison remain
unavailable/not run. No AI, carrier/default activation or push.

Next: selected local-service entry/clock producers, notification consumers and
later dead-object service, then remaining comment/output/world ownership before
staged E001/E002. Parent phases remain researching.
