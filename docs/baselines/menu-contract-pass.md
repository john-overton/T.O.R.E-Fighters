# Creator/ordnance contract tooling — 2026-09-14

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


First implementation step after planning commit `c68b5eb`.

## Implemented

- `tools/extract_assets.py --native-menus` selects the new static research domain,
  preserving native/archive mutual exclusion, bounded input, safe output and
  conflict handling. `tools/native_menus.py` supplies hash-gated creator filter,
  ordnance weight/load/capacity subregions and bounded data-string references.
- `ui::menu_tree` exposes the shared inert MNU grammar; `flight_menu` delegates
  without changing runtime behavior. The `menu_tree` example bounds file reads
  and prints extracted hierarchy/accelerators.
- Synthetic tests cover unknown-build gating, truncated/non-data/oversize string
  ranges, CLI domain separation, anonymous menu containers, shared/cyclic child
  references, invalid markers/encoding and unterminated labels.

## Local source results

Repeated `--native-menus` extraction into `.local/menu-contract-pass` completed
without overwrite or conflicts: 14 selected symbol spans, 3,829 symbols, six
reviewed subregions, 156 candidate strings and 64 direct literal references.
EXE/SMS hashes match the reviewed pair; individual regions carry source hashes.
`menu-trees.txt` records the bounded QM_MENU and ARMPLANE reader output.

QM_MENU contains Fly all and nested Era choices. ARMPLANE contains Unload All,
the explicit unrestricted-loading cheat, next/previous aircraft with bracket
accelerators, and campaign replay/exit. Campaign visibility is not established
for quick missions. These are recovered source labels/trees, not working callbacks.

Creator filter code at `0x42ed13–0x42edd3` handles era IDs `0x203–0x206`, storing
the four choices in mask `0x30000`. Checkmark code at `0x42ee15–0x42eeaa` compares
those bits. The full catalog filtering and initialization contracts remain open.
Generic disassembly can misalign around embedded data: named SMS spans remain
research, and the six subregions do not establish whole-screen execution parity.

## Validation

- Formatting, warnings-denied workspace Clippy, locked workspace tests and build
  passed on Linux. **222 Rust tests and 17 Python tests passed.**
- Source, app and extractor asset guards passed; `git diff --check` passed.
- Repeated retail static extraction and both retail menu-tree reads passed.
- No rendering or flight behavior changed; GPU smokes were not run for this step.
  Windows/macOS runtime checks were not performed.

Still open: active option/default tables, DLG geometry and runtime population,
art dependency closure, typed mission/loadout editing, creator/ordnance screen
implementation and accepted armed launch. This is completion of the repeatable
research-tooling substep, not completion of either screen.
