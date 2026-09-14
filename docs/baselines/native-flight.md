# Native flight research baseline — 2026-09-13

Host: macOS, Apple M3, pinned Rust 1.91.1. This baseline records static analysis
and translated-helper validation; no original executable was run or emulated.
See [source provenance, coverage and commands](../formats/native-flight.md).

## Checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 82 tests passed (including five new helper tests).
- `cargo build --workspace --locked`: passed.
- `python3 -m unittest discover -s tools -p 'test_*.py'`: seven tests passed.
  New synthetic PE/SMS fixtures cover malformed bounds, no-disassembly dry run,
  identical reruns, conflict preservation, output/source separation and no
  build-specific annotations on unreviewed input.
- Asset guard: repository, `target/debug/tore-app`, `target/debug/tore-extract` passed.
- Menu, viewer and free-flight `--no-audio --smoke-test`: each presented on Apple M3 Metal.
- Static extraction ran twice to `.local/native-flight/repro` with identical
  outputs accepted: 3,829 symbols, 107 selected spans. No reference checkout is
  required by the script.
- `--native-flight-report`: passed with imported F/A-18D; logs under
  `.local/native-flight/`. Output identifies `complete_model=false`.

## Selected imported-data probe results

These are outputs of Rust translations, not original-game comparison captures.

| Probe | Result |
| --- | --- |
| PT load/low-speed AoA fields | 20 / 70 fps / 15° |
| Pull offset after one inferred second at 3G | 4.441406° |
| Pull offset after one inferred second at 6G | 10° (slew-limited) |
| Pull offset after one inferred second at −3G | −6.664062° |
| 1G envelope at 5,000 ft | 213–1,259 fps; structural 1,269 fps |
| Fuel-rate fixed8, commands 50 / 100 / 101 | 256 / 512 / 4,096 |

The default authored adapter was not switched to the partial native helpers.
Its retained three-second left/right banked-pull probes report AoA 9.3986°,
mirrored sideslip ±0.0239°, speed 413.851 KT, altitude 5,492.948 ft. The full-loop
probe passes vertical/inverted/completed at tick 2,590 without a crash. These
checks establish continuity of the existing adapter, not native parity.

## Lessons and next acceptance gate

- FA's symbol map materially improves traceability; USNF97 addresses must not be
  reused as FA addresses.
- AoA is split across movement and display offsets. Replacing only a coefficient
  in the authored velocity spring would not reproduce native movement.
- Symbol spans and direct field references are research aids, not a complete
  control-flow graph. Unnamed helpers and indirect accesses remain manual work.
- Preserving native integer truncation matters even for small angle offsets.
- Complete the native state layout, clock, force/movement order, stall/spin,
  load/damage and ground branches before selecting native dynamics in free flight.
