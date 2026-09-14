# Combat exporter and component baseline

Date: 2026-09-14. Subsequent [live-fire work](live-fire.md) supersedes the
no-live-weapons status below and fixes the expanded-cache reload cap. Plan commit `457c85b` was pushed to `origin/main` before this
implementation. This records partial W0/W1/W2 work, not completion of the
[weapons plan](../formats/weapons.md) or vanilla combat acceptance.

## Export evidence

```sh
python3 tools/extract_assets.py --aircraft f18 --aircraft rafale --weapons --exclude-archive 'disc1/LHX/*' --out .local/combat-implementation/catalog
# Run the identical command again: all outputs unchanged.
python3 tools/extract_assets.py --native-weapons --out .local/combat-implementation/native-final
cargo run --locked -p tore-sim --example weapon_probe -- .local/combat-implementation/catalog/FA_2.LIB/*.JT
TORE_DATA_DIR=.local/combat-implementation/app-profile cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only
```

The combined archive export writes **561 resources, zero errors**. On repeat,
**561 are unchanged**. All 135 JT, 51 SEE, 30 ECM and 4 GAS definitions pass the
checked typed readers. Other selected files include 97 SH, 161 PIC, 37 11K,
24 5K, 9 FNT, 2 PT, 2 PTS and both aircraft's cockpit/HUD dependencies.
The report retains all matching archive copies; these are resource counts, not
counts of supported/flyable weapons or unique rendered objects.

Shared mandatory roots are CRATER/SMOKE/FIRE/EXP/DEBRIS/CHAFF/FLARE/SPD/MPD/LPD.SH
and nine reviewed explosion/splash/fire/chaff/flare sound resources. Ten shape
loads are directly visible in GRAPHICInit. Sound preservation does not yet
establish effect-index/event mappings. Transitive literal discovery now includes
their available textures and sounds; explicit missing reviewed dependencies fail.

The report contains 137 native-symbol edges, 14 unresolved artwork candidates
and 2 unresolved PTS module candidates. These are edge counts, not distinct
missing files. F18.PTS/RAFALE.PTS contain ICONF18.PIC/ICONRAF.PIC strings absent
from this catalog. They are compiled modules, not decoded loadout presets.
SU35R.SEE labels itself as rear-facing and contains negative heading limits;
those bits are retained rather than normalized. Neither behavior is implemented
as an inferred replacement for native consumers.

The report records provider archives, last-provider dependency lookup, required
and candidate edges, filtered selection and successful inclusion separately.
Its `complete` flag means extraction succeeded; `native_parity` is false.
Raw modules and all generated evidence remain ignored in `.local/`.

## Native arithmetic evidence

Both source hashes gate fixed-address research artifacts:

- FA.EXE SHA-256: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
- FA.SMS SHA-256: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`

The static pass indexes 3,829 symbols, selects 119 weapon-related symbol spans,
and emits 19 bounded reviewed regions plus packed JT field references. It never
executes retail code. Regions can include untranslated branches and external
calls; a region listing is not a completed translation.

| Implemented component | Reviewed FA consumer | Remaining caller contract |
| --- | --- | --- |
| Launch speed | 0x4c1120 | Mount/launch attitude, spawn ownership and spread |
| Motor phase | 0x4c1170 | Native service scheduling and launch state |
| Lifetime and gravity fall | 0x4c11b0 movement subregions | Full movement/lock/smoke order and contact |
| Altitude speed performance | 0x477da0 | Command flags and guidance producers |
| Axial speed approach | 0x438070 | Complete object command/movement order |
| Positive-distance position | 0x4120c0 | Heading/pitch, reverse motion and exact imported trig data |
| Player repeat/rising edge | 0x416ef5 dispatch subregion | Bay/target permission and PROJFire lifecycle |
| Ammo debit | 0x4527f0 | HARDSetFlags and aircraft mass/configuration updates |
| Store weight/capacity masks | 0x452940 / 0x452980 | Availability, pairing/pods, loaded flags, release/jettison |
| Radar emission deadline | 0x4c2eb0 | Actual object emissions/targets and sensor updates |
| Partial FOV/range gates | 0x4c2860 | Coordinate, rear-facing, predicted-range and target producers |

Configuration is parsed into owned typed values once; arithmetic receives typed
values and caller-owned state. No callbacks, renderer, wall clock or hidden RNG
are used. Native position arithmetic requires a caller-supplied extracted sine
table. The host's 120 Hz clock is not relabeled native scheduling.

The probe passes all 135 JT definitions, four launch speeds each (**540 rows**),
plus lifetime and applicable ignition boundaries. This is component execution,
not trajectory, guidance, damage or original-game differential acceptance.

## Validation and limitations

- Formatting, warnings-denied workspace Clippy, locked workspace tests/build pass.
- Source and app/extractor binary asset guards pass; no retail payloads are added.
- **191 Rust tests and 14 Python tests pass**, including synthetic missing-art,
  cycles/overrides, filtered reporting, PTS unknowns, typed equipment, rear-angle
  preservation, native build gating and repeated-aircraft wrapper coverage.
- Expanded app cache imports successfully. A menu smoke test loads it and presents
  successfully on Linux / NVIDIA RTX 4070 / Vulkan. No rendering code changed.
- Windows/macOS runtime checks, original-game engagement comparisons and full
  combat performance/frame-time measurements are not available in this pass.

No live weapons are enabled. Complete projectile guidance, target/sensor state,
collision/fuzes/damage, original effect rendering/audio scheduling and loaded
mass/drag remain open. W3–W5 need those native contracts and matched original-game
observations; scalar tests alone cannot meet the user's 1:1 performance target.
