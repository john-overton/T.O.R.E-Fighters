# STRIP source and box-metadata checkpoint

2026-09-15, following clean `3714c72`. NE-00.1d metadata precursor complete;
parent NE-00.1 / NE-01.1 / NE-03.1 remain researching.
**Native source/diagnostic translation tested; no new runtime contact branch;
retail comparison unavailable.** [Contract](../formats/native-strip.md),
[living dependency register](../native-environment-systems-plan.md).

## Reproduction and scope

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-source-final
python3 tools/extract_assets.py --include '_RUNWAY.PIC' --exclude-archive 'disc1/LHX/*' --exclude-archive 'disc1/WB/*' --out .local/native-environment/strip-resources
cargo run --locked -p tore-formats --example native_strip -- .local/native-environment/land-discovery/FA_2.LIB/RUNWAY.SH
```

The exact reviewed EXE/SMS hashes remain unchanged. The static pass now emits
**111 reviewed regions**, 107 symbol spans and 3829 symbols. Seventeen new
aligned regions cover STRIP addition/selector, box lookup, airport registration,
point transforms, mission conversion and initial query, current-object load/store,
callback resolution and collision-list registration. Referenced branches outside
those slices remain open; emitted disassembly is not a runnable or accepted engine.
Full-image static byte reads separately checked the selector/jump tables at
0x4be678/0x4be68c, 0x4a7a1c and 0x462960 against their consumer instructions.

`native_strip` validates the required first-match IDs and reports **23 boxes**,
ten midpoint-position records and two orientation records. It does not place
an airport or execute callbacks. The existing incomplete static projector reports
**63 faces**, one encountered named texture `_RUNWAY.PIC`, and no encountered
state words. This does not establish complete reachable drawing/LOD coverage,
absence of other dependencies, visual inspection or collision acceptance.

The texture extraction selected **1 resource / 0 errors** from FA_2.LIB:
decoded size **54924 bytes**, SHA-256
`60bd9a87bde90671c05e041664b9a782d63e6542be151f45a3a4240126e4d929`.
Archive SHA-256 remains
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
The earlier four-resource root extraction is identified in the
[foundation baseline](native-land-foundation.md). These two filtered invocation
reports are not a full-media census or cumulative catalog.

## Validation

Two new synthetic tests cover signed midpoint rounding/extremes, source pair
order, duplicate IDs, uninterpreted flag retention, every truncation through a
two-record list, absent versus empty F2, unsigned relative-link bounds, the
4096-record host cap and required terminator. No retail fixture is committed.

Linux checks pass: formatting, warnings-denied workspace/all-target Clippy,
**348 Rust tests**, locked workspace build, **24 Python tests**, repository/app/
extractor asset guards and diff whitespace checks. Logs:
`.local/native-environment/strip-checks/`. Static artifacts and imported data
remain ignored under `.local/native-environment/`.

Existing native airborne replay passes **28 cases / 33600 updates** for the
reviewed F/A-18D and Rafale C. Creator, viewer and both native cockpit startup
smokes pass on NVIDIA RTX 4070 / Vulkan / Immediate. Commands match the
[foundation regression commands](native-land-foundation.md), using
`strip-source-final/tables`. These are startup regressions, not new handling,
contact, runway visual or performance acceptance. No rendering code changed.
The controller evdev warning persists; physical controls/audio, Windows/macOS
build/runtime and retail comparison remain unavailable/not run.

## Remaining gate

Initial object ground sampling precedes collision and airport registration
(new E012). Finish type resolution, template defaults, final creation/store,
failed-registration cleanup and full placement fields; preserve this order in
staged world construction. E004 drawing/LOD/palette/visual closure remains open.
Then implement E001/E002 ordered queries with late-failure state/cache/RNG rollback
and narrowly connect both aircraft. Carrier, unsupported callbacks and unknown
event effects remain gated. No AI work, default change or push.
