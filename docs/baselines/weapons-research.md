# Aircraft weapons research baseline — 2026-09-14

Extraction/static-code audit, not weapon runtime acceptance.
[Plan and findings](../formats/weapons.md).

## Inputs and method

Local installation plus recursive disc archive discovery, excluding unrelated LHX.
No retail code executed. Reports, resources and assembly remain ignored under
`.local/weapons-research/`. Source hashes:

- FA.EXE: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
- FA.SMS: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`

Inspected `aircraft.rs`, `aircraft_schema.rs`, extractor filtering/analysis,
Python wrapper, app asset import and aircraft/native-flight documentation.
Compared ignored reference `Docs/formats/{jt,native-guns,damage}.md` and
`tools/retail/retail/gun.py` as leads, retaining their hypothesis/authored labels.

## Reproduce

```sh
python3 tools/explore_assets.py --out .local/weapons-research/inventory.json
python3 tools/extract_assets.py --weapons --exclude-archive 'disc1/LHX/*' --out .local/weapons-research/all-weapons
python3 tools/extract_assets.py --aircraft f18 --exclude-archive 'disc1/LHX/*' --out .local/weapons-research/f18
python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc1/LHX/*' --out .local/weapons-research/rafale
python3 tools/extract_assets.py --native-flight --out .local/weapons-research/native
python3 tools/extract_assets.py --include '*.PT' --include '*.PTS' --include '*.GAS' --include '*.SEE' --include '*.ECM' --include 'SMOKE.SH' --include 'FIRE.SH' --include 'FLARE.SH' --include 'CHAFF.SH' --include 'DEBRIS.SH' --include 'CRATER.SH' --exclude-archive 'disc1/LHX/*' --exclude-archive 'disc1/WB/MPLAYER/_SETUP.LIB' --out .local/weapons-research/supplement
```

Native-flight mode provides full disassembly and 3,829 SMS entries; its selected
107 spans/52 reviewed regions are flight research, not a complete weapons pass.
Weapon symbols were separately filtered from that inventory.

## Results

| Check | Result |
| --- | --- |
| Loose catalog | 145 PT, 37 PTS, 135 JT, 51 SEE, 30 ECM, four GAS; 84 NT and 170 OT |
| Whole weapons extraction | 305 resources, zero errors; all 135 JT have named analyses |
| Breakdown | 135 JT, 75 SH, 61 PIC, 26 11K, seven 5K, one PAL |
| Archive contribution | Two resources from FA_1, 303 from FA_2; no extra matches from discs/swpatch |
| F18 / Rafale profiles | 102 / 100 resources, zero errors |
| Supplemental extraction | 273 resources, zero errors with exclusions above |
| PT literal JT references | 145 PTs scanned; 70 unique JT names; none missing from catalog/weapons export |
| Raw JT signature values | 0: 57; 1: eight; 2: 23; 3: 45; 4: two; consumer semantics not established by counts |

The initial generic supplemental extraction wrote 273 resources but reported one
non-EALIB MPlayer `_SETUP.LIB` error. Explicitly excluding that installer archive
and repeating reused the resources and passed. Profile discovery already skips
non-EALIB installers. This was not a weapon parser failure.

Literal PT references cover defaults/references, not all compatible loadouts.
PTS semantics and other PT identities are not validated by raw preservation.
Likewise 75 extracted SH files do not establish 75 rendered weapon models.

## Native evidence

FA GRAPHICInit pushes the following strings before calls to 0x4a6ae0:

| File | String VA | Push instruction VA |
| --- | --- | --- |
| CRATER.SH | 0x4f4e70 | 0x442c36 |
| SMOKE.SH | 0x4f4e64 | 0x442c4a |
| FIRE.SH | 0x4f4e5c | 0x442c72 |
| DEBRIS.SH | 0x4f4e48 | 0x442d08 |
| CHAFF.SH | 0x4f4e3c | 0x442d2b |
| FLARE.SH | 0x4f4e30 | 0x442d53 |

All six exist in FA_2; none is selected by `--weapons`. Module strings identify
SMOKE.PIC, FIREA.PIC, FLARE.PIC and CRATERS.PIC. All exist and are also absent.
This confirms missing shared graphics roots beyond token-scan limitations.

PROJSpeed was inspected instruction-by-instruction from 0x4c1120 through its
return at 0x4c1163. Local packed schema offsets agree with reads at
+0x115/+0xfb/+0x67/+0x6b. PROJFire reads ammunition debit at 0x4c2248 from
+0xf0 before calling HARDUnload at 0x4527f0. The full firing lifecycle and clock
remain open; [the plan](../formats/weapons.md) distinguishes these boundaries.

## Validation limits

This pass changes documentation only. No original-game trajectory/capture/audio
acceptance or Windows/macOS runtime check was performed. No GPU checks were
rerun for this documentation-only change; they cannot certify combat that does
not execute yet.

Validation passed on the Linux host: `cargo fmt --all -- --check`, workspace
Clippy with `--all-targets --locked -- -D warnings`, 174 Rust tests with
`--workspace --locked`, workspace locked build, all 11 Python tests, and asset
guards for the source tree plus both debug executables. `git diff --check`
passed. Retail outputs remain ignored; no commit or push was made.
