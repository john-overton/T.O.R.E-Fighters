# F/A-18D free-flight development checkpoint

Host: Apple M3/macOS, pinned Rust 1.91.1, wgpu Metal. Work started from the committed all-theater/menu baseline on 2026-09-13. No retail bytes or derivatives are tracked. This is an incremental development port; native 1:1 acceptance remains pending.

## Reproduce

```sh
python3 tools/extract_assets.py --aircraft f18 --weapons --exclude-archive 'disc1/LHX/*' --out .local/f18-import
cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only
cargo run --locked -p tore-app -- --free-flight
cargo run --locked -p tore-app -- --headless-flight 1200 --maneuver level
cargo run --locked -p tore-app -- --capture-flight .local/f18-research/cockpit.ppm
cargo run --locked -p tore-app -- --capture-flight .local/f18-research/chase.ppm --flight-view 1
cargo run --locked -p tore-app -- --panel-snapshot .local/f18-research/systems.ppm --instrument-page 7
```

Captures require pre-existing parent directories. Flight captures use an actual GPU/display and include the HUD/windows. Panel snapshots and headless flight need imported media but no window or audio device. `--instrument-page 1..9` selects a single window for capture/inspection. `--maneuver level|pull|roll|stall` selects a deterministic headless input script; these are regression probes, not native maneuver acceptance.

Research files, source hashes and exports are under ignored `.local/f18-research/` and `.local/f18-import/`. The structured extraction report supplies dataset identity. Detailed evidence and limitations are in [aircraft format notes](../formats/aircraft.md).

## Validation

Recorded checks: Native game-flight comparison, Linux/Windows runtime acceptance and audible comparison are not established by parser tests or a Metal startup frame.


- Shared extraction: **384 resources, zero errors**, including 135 JT definitions. Repeating it reported all 384 unchanged with a complete hash/provenance report.
- Structured source recovery: 14 G rows, nine hardpoints, 282 visible neutral shape faces, 256×644 atlas, WIN11 height 10. Gear/brake/hook/burner branches were independently compared with the reference neutral projector.
- Metal: all 16 theaters launched in free flight; all nine instrument pages rendered; main menu, creator and retained developer viewer passed startup checks. Camera windows were captured from actual GPU rendering. Subsequent viewport and camera-framing corrections were rechecked on Ukraine.
- Visual review: original cockpit transparency, corrected viewport, chase/oblique exterior, systems text, RWR, close Other View and creator captures inspected under `.local/f18-research/`. No native side-by-side flight acceptance is claimed.
- Headless level/pull/roll/power-off probes run twice for identical output; synthetic fixed-tick tests compare state at 30/60/144 render Hz. A review caught incorrect banked-lift projection; it was corrected and given a physical-outcome regression test.
- Rust formatting, Clippy with warnings denied, locked tests/build and Python/asset checks are required for the final working tree. Final results: **47 Rust tests and five Python tests passed**; formatting, Clippy with warnings denied and locked build passed. Repository and both executable asset guards passed; `git diff --check` was clean.

Source identities for this device mapping:

| Resource | SHA-256 |
| --- | --- |
| F18.PT | `2cd308c5b94b4560726c35c37d2f0735db179a6e9c7cd0ddef66ab70dc303abd` |
| F18.SH | `d6c876d63d10a05072c8afd8a53cedffdd9cdfbcff4c4576a90c1b6c064b8bb9` |
| F18.HUD | `ca086ce252bb7b89e98aa029658ea485ff9249ae1ffa705b3d38022d0a3829eb` |
| _F18.PIC | `9e75663623b378ef4b92093be3caf2ff793965f77af6d473f62876dfeb6f0861` |
| WIN11.FNT | `0d0692c28b3dc753d880c6f6a6d82f36dd62eae7a5d4f46f2bb448dcac261286` |

The final import also includes all three HUDSYM mode fonts. Strict-art validation exposed the earlier incorrect HUDSYM.PIC assumption; the resource family was corrected and import/render checks rerun. The matching synthetic dependency test was extended for newly mandatory cockpit/font inputs. No runtime dependency was added and no commit was created.
