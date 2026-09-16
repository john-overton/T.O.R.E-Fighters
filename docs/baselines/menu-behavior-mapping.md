# Creator and ordnance behavior mapping — 2026-09-14

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


Continues [active tables and geometry](menu-options-geometry.md) against the same
hash-verified FA.EXE/SMS pair. Research only; no native modules were executed and
no application rendering or simulation behavior changed.

## Results

- Mapped creator initialization, random versus fixed defaults, theater-driven
  nationalities, player minimum count and target/defense dependencies.
- Mapped Shift/list-versus-cycle behavior and accept/cancel state copying.
- Recorded exact player/other-wing filter masks and catalog record/filter limits.
- Added bounded ordnance action-table extraction with unknown/duplicate/truncation
  rejection and synthetic coverage.
- Mapped page/category state, fuel increments/clamps, quantity scaling and finite
  stock limits, cheat exceptions, card-name fit/sort and catalog/station hit grids.
- Extended explicitly aligned disassembly spans; corrected selector end address
  to include its final stack adjustment and return.

Specifications: [creator](../formats/quick-mission.md),
[ordnance](../formats/ordnance-menu.md). Remaining mapping work is listed in those
contracts. Full retail UI/mission parity remains open.

## Validation

```
python3 tools/extract_assets.py --native-menus --out .local/menu-behavior-mapping
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 -m unittest discover -s tools -p 'test_*.py'
python3 tools/check_assets.py
python3 tools/check_assets.py target/debug/tore-app
python3 tools/check_assets.py target/debug/tore-extract
```

All passed: 224 Rust tests, 20 Python tests and all three asset guards. Extraction
produced the five ordnance action mappings and retained all 60 creator dispatch
entries and 16 theater target tables. Local catalogs/disassembly remain ignored.
No rendering changes; no GPU or Windows/macOS runtime acceptance was performed.

## Flow follow-up

The repeatable `.local/menu-flow-mapping` extraction additionally covers the
custom-load `armplane` directive/parser, ordnance exit codes, menu availability,
pickup/keyboard input branches and half-open rectangle test. Static tracing
confirms separate pickup/drop rectangles, source and catalog drag deltas,
fixed-point fuel serialization, and multiplayer/fort menu restrictions.
See the [complete scope ledger](../research/menu-parity-matrix.md) for mapped and open work.
These findings extend static evidence; they do not close original-game acceptance.
