# All-theater extraction and added discs

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature; see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


Checkpoint: 2026-09-13, macOS Apple M3, Rust 1.91.1. This extends extraction, not theater renderer acceptance.

```sh
python3 tools/extract_assets.py --theater all --exclude-archive 'disc1/LHX/*' --out .local/all-theaters
```

Result: **1,129 resources, zero errors**; repeat run reused all 1,129 as `unchanged`. FA_1 supplies 852 selected resources and FA_2 supplies 277. All 16 defined T2 files and all 75 MM layouts were parsed. Shared sky/celestial/cloud resources are included. Full archive/output hashes and metadata remain in ignored `.local/all-theaters/extraction-report.json`.

All-theater testing exposed valid signed border texture placements in PGU/SPA layouts. The parser now retains them. It also exposed unsupported compression flags in the bundled LHX game's EALIB variants; the explicit path exclusion avoids claiming support for those archives. MPlayer's non-EALIB `_SETUP.LIB` is printed and skipped during directory-based theater scans. Generic extraction and explicit archive inputs remain strict.

## Added disc inventory

| Disc / archive | Directory entries | Observed resource types |
| --- | ---: | --- |
| disc1 / FA_4C | 91 | 44 PCM, 4 CB8, 43 PIC |
| disc1 / FA_7 | 815 | 105 PCM, 355 FBC, 355 VDO |
| disc2 / FA_3 | 1,091 | 269 INF, 822 PIC |
| disc2 / FA_10 | 22 | 11 PCM, 11 CB8 |
| disc2 / FA_10B | 20 | 10 PCM, 10 CB8 |
| disc2 / FA_11 | 20 | 10 PCM, 10 CB8 |
| disc2 / FA_11B | 16 | 8 PCM, 8 CB8 |

Inventory reports with hashes/offsets: `.local/exploration/disc1.json` and `disc2.json`, produced by `tools/explore_assets.py` against each disc root. These are directory inventories, not successful video/audio decoding claims. There were no theater-profile matches in these seven additional archives. They are useful inputs for later reference/media work. Disc 1 also includes SETUP.ESA and bundled legacy software; package/disc completeness and installer decoding remain unverified.

## Validation

Formatting, warnings-denied Clippy, locked build and **30 Rust tests** passed. Tests cover all-profile aliases, single-profile isolation, archive exclusions, signed border coordinates and retained generic strictness. **5 Python tests** and repository/both debug executable asset guards passed. Ukraine still passed a real Metal viewer smoke test. No other theater was enabled or visually accepted, and no Linux/Windows runtime was tested here.

## Runtime and typography follow-up

All 16 base theaters are now imported into the app cache and enabled in the creator. Each was built and presented in a real Metal window with `--theater CODE --viewer --smoke-test`; all exited successfully. Creator CPU captures were generated for all 16. Egypt's actual GPU capture and the corrected creator/notice images were visually inspected under `.local/font-audit/`. These confirm functional previews, not native environment parity.

The renderer sizes its texture array per theater, uses that theater's DAY2 variant palette and updates the sky layer binding. Kurile has no base-MM numbered placements, so its current surface uses source palette colors. Selection updates the source briefing map and resets the camera; mouse/keyboard selection covers all 16 entries in a synthetic test. Full weather, native shoreline behavior and ground objects remain open.

TVIET uses **TVI0–41.PIC**, which were absent from the earlier filename profile. The corrected all-profile extraction now contains **1,171 resources with zero errors**. The app imports these directly from user media and rejects old caches lacking the new layouts, Vietnam textures or font strips.

The creator/notice font changed from BODYFONT to the original ARMFont sans-serif; the HUD uses SMLFONT. The tint renderer now preserves source dark/edge pixels instead of filling them white. The original FONTACT button lettering is retained. Creator and notice captures show the thinner, clearer text. Fonts still share the native 640×480 raster canvas; no scalable font engine or new font dependency was added.

Final checks: **32 Rust tests**, **5 Python tests**, formatting, warnings-denied Clippy, locked build, repository and both debug executable asset guards passed. Native window smoke checks covered every theater and the creator. Source/derivative assets remain ignored.
