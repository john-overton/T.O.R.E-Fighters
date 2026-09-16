# Active menu options and geometry — 2026-09-14

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


Continues [contract tooling](menu-contract-pass.md), using the same reviewed
EXE/SMS hashes. No native code was executed.

```sh
python3 tools/extract_assets.py --native-menus --out .local/menu-options-geometry
cargo run --locked -p tore-formats --example dialog_geometry -- .local/quick-mission-plan/FA_2.LIB/QUIKMISS.DLG .local/quick-mission-plan/FA_2.LIB/QUICK14.DLG .local/quick-mission-plan/FA_2.LIB/LOADORD.DLG
```

The dialogs come from the earlier creator extraction; use the general extractor
if absent. `creator-options.json` records 60 dispatch entries, 16 theater target
lists, 29 briefing geometry rows and source hashes/addresses. Three `aligned/`
consumer disassemblies avoid linear-disassembler misalignment after jump tables.
Normal symbol spans remain diagnostic, not complete code/data classification.

All 26 inspected creator/legacy-selector/ordnance dialogs parsed successfully;
`dialogs.txt` retains the output. The [specification](../formats/quick-mission.md)
summarizes active choices and source geometry without copying catalog data into code.

## Validation

- 224 Rust tests and 19 Python tests pass; formatting, warnings-denied Clippy
  and locked workspace build pass on Linux.
- Synthetic tests cover truncated dialogs, invalid imports/labels/relocations,
  duplicate draw records, bad constant-pointer grammar, missing double terminators,
  oversized/non-ASCII option lists and wrong section kinds.
- Repeated hash-gated source extraction and all retail dialog reads pass.
- Source/app/extractor asset guards and diff whitespace checks pass.
- No rendering/flight changes, GPU smokes or Windows/macOS runtime checks.

Dynamic catalogs/defaults, modifier mapping, runtime inline hit geometry and final
art placement remain separate gates. Reports are research artifacts, not new UI.
