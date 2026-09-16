# Selected stationary STRIP movement checkpoint

2026-09-15, following `dab34e7`. **NE-00.1m completes selected intermediate
movement source recovery and a diagnostic angle helper.** It does not enable
object service or native contact. [Contract](../formats/native-strip.md#selected-stationary-movement-path--ne-001m),
[living plan](../native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-movement-source
cargo test --locked -p tore-sim angle_approach
```

Nine independently aligned reviewed ranges cover selected heading/pitch/bank/
speed dispatch and common-body branches plus word-angle stepping/magnitude.
Fresh and repeated extraction pass: **189 regions**, 107 selected symbol spans,
3829 symbols, unchanged reviewed EXE/SMS hashes. Dispatch tables were inspected
as inert data; surrounding autonomous command branches remain unsupported.

The existing OBJECT schema and original STRIP.OT identify zero turn/bank/minimum-
speed/maximum-altitude inputs. The metadata reader remains unchanged; this is
not full loader acceptance. Source review establishes why ground pitch/bank
targets do not turn this zero-rate object, while replacement flag 1 still updates
Y from the ground sample at zero speed. No flat-height substitution or callback
suppression is introduced.

`approach_angle` tests zero rate, exact and exceeded step, negative step,
word wrap in both directions, the signed half-turn tie, widened magnitude
32768 and dword INT_MIN/INT_MAX normalization. No retail bytes are test fixtures.
The helper does not own clock, query, command or mutable world state.

Linux validation: **363 Rust tests**, **26 Python tests**, fmt, workspace/all-target
Clippy with warnings denied, locked workspace build, repo/app/extractor asset
guards, whitespace and changed-document local file-target checks pass. Link
checks do not validate anchors. Native live replay passes **28 cases / 33,600
updates**, F18.PT = F/A-18D and RAFALE.PT = Rafale C, with the extracted tables.
Creator smoke presents on RTX 4070 / Vulkan / Immediate. Logs:
`.local/native-environment/strip-movement-{tests,replay,gpu}.log`.

Replay and GPU evidence cover unchanged airborne/creator behavior, not a new
runway or handling result. No fresh viewer/cockpit/performance claim. Physical
input/audio were not manually tested; Windows/macOS build/runtime and retail
comparison remain unavailable/not run. No AI, carrier/default changes or push.

Next: E021 queue reset/routing, speech observer and global interceptor ownership;
retain E003/E005 full typed construction and E016/E004 dependencies before staged
E001/E002. No parent phase is complete.
