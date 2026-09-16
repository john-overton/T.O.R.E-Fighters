# STRIP placement conversion checkpoint

2026-09-15, following `fad7fd5`. **NE-00.1f complete as a narrow source and
nationality-conversion precursor.** Parent land producer/closure gates remain
open. [Contract](../formats/native-strip.md), [living plan](../native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-placement-source
cargo test --locked -p tore-sim native_objects
```

The unchanged reviewed EXE/SMS pair produces **132 reviewed regions**, 107 selected
symbol spans and 3829 symbols. Nine new bounded slices cover queue reset/removal/
insertion, mission type/alias/nationality/flags/speed/name and post-create fields.
The nationality jump-table bytes and targets were separately read as inert data
and checked against their dispatch instructions. Repeat extraction passes.

The synthetic nationality test covers all six accepted map-prefix cases,
unchanged/nonmatching prefixes (including leading tilde/dollar), five remap
branches, the separate high bit, the 7/8 threshold and 127/255 boundary behavior.
The selected UKR text value 137 converts to loaded byte 138. This is diagnostic
arithmetic; it does not parse the whole MM, place a runway, schedule an object or
establish alliances. No retail bytes are used in tests.

Linux formatting, warnings-denied workspace/all-target Clippy, **352 Rust tests**,
locked build, **24 Python tests**, repo/app/extractor asset guards and whitespace
checks pass. Logs: `.local/native-environment/strip-placement-checks/`.
Existing native airborne replay passes **28 cases / 33600 updates** for both
reviewed aircraft using the [foundation command](native-land-foundation.md) with
`strip-placement-source/tables`. Creator, viewer and both native cockpit startup
smokes pass on NVIDIA RTX 4070 / Vulkan / Immediate. No rendering changed and no
new runway visual, ground handling, performance or retail acceptance is claimed.
Physical controls/audio and Windows/macOS build/runtime remain unavailable/not
run; the existing evdev unreadable-device warning persists.

## Remaining work and discoveries

Full bounded placement/type loading, template field consumers/ownership and E004
drawing/LOD/palette closure remain open. Mission alias and final store happen
after T_AddObj returns; construction must include that phase. E015 scheduling
insertion/removal is sourced but not translated. Exploratory service delay review
finds conditional bound-20/bound-8 draws (E018); full predicates, callbacks and RNG
ownership remain unaccepted. No autonomous branch is implemented. E001/E002
staged query state must wait for these required ownership contracts; carrier and
unsupported event effects remain gated. No default change, AI work or push.
