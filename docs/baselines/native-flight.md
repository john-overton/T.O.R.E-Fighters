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

## Component follow-up

Same host/toolchain and static-only research policy. The follow-up adds ten Rust
regression tests (92 workspace tests total) and one Python test (eight total).

- Formatting, Clippy with warnings denied, workspace tests and build: passed.
- Python synthetic tests: passed, including explicit reviewed-region bounds,
  outgoing edges, and absence of reviewed manifests for unknown input hashes.
- Extraction to `.local/native-flight/components-final` ran twice with identical
  outputs accepted: 107 exported spans plus 18 manually reviewed regions, 3,829
  symbols and 21 partial instance fields. The universal entry point remains
  `tools/extract_assets.py --native-flight`.
- `--native-flight-report` passes with the imported F/A-18D and now identifies
  `native_helpers_v2`, `complete_model=false`. It maps PT departure, drag, landing
  and velocity fields into typed profiles. Supplied-condition departure samples
  progress Warning → Stalled after the expected 512-unit delay.
- Mirrored one-second spin component probes (128 × two native elapsed units)
  produce intensity 6,400, movement pitch −10,240, movement roll ±32,528,
  bank offsets ∓3,328, AoA offset 10,064 and speed 79,360 (fixed8 units).
  These are branch outputs before subsequent native force/movement stages,
  not observed flight trajectories or certified real-time timing.
- Ordered velocity probes use a supplied weight/force set and the reviewed
  5,000-ft 1G maximum of 1,259 fps. Tests independently verify the temporary
  ground acceleration reduction with forces large enough to reach that limit.
- The unchanged playable adapter still completes its loop at tick 2,590:
  427.958 KT, 4,971.080 ft, no crash. Mirrored three-second banked pulls retain
  AoA 9.3986°, sideslip ±0.0239°, bank ±61.0741°, 413.851 KT and 5,492.948 ft.
- Asset guard: repository and both debug executables passed.
- No rendering/input behavior changed, so GPU captures were not repeated.
  Linux/Windows builds were not run on this Mac host.

Logs: `.local/native-flight/components-{tests.log,report.txt,loop.txt,bank-left.txt,bank-right.txt}`.
Research artifacts remain ignored. No imported executable or emulated routine ran.

New lessons: preserve drag-before-force ordering, distinguish wide service-time
multiplication from MatchF24, retain the spin recovery latch, and resolve the
forward speed limit from its setup producer instead of PT's zero placeholder.
The typed components allocate only at profile import/error creation; ordinary
component updates use scalars and fixed arrays. This is an architectural property,
not a measured end-to-end performance improvement.

Remaining acceptance is whole-tick composition: native loaded weight/thrust and
control authority, lift/gravity forces, complete rotations and wind, stall tumble,
terrain/carrier contacts, scheduler/RNG and original-game trajectory comparison.
The default flight model intentionally remains the previously validated adapter.

## Trigonometry and force follow-up

Static research continued on the same reviewed FA build; no original/emulated
code ran. New checks cover five rotation/force tests, two loading tests and one
position test: **100 Rust tests total**, with **nine Python tests**.

- Formatting, Clippy with warnings denied, workspace tests and build passed.
- Extraction to `.local/native-flight/rotations-final` repeated successfully:
  107 exported spans, 28 reviewed regions, and the 321-word sine table with hash
  metadata. Binary table output uses the same preflight conflict protection.
- `--native-flight-trig .../tables/sine-q15.bin` passed with the imported Hornet.
- Converted +90°/−90° PA values: 16387 / −16386. Imported sine/cosine results:
  (32766, −10) / (−32767, −7). These are native arithmetic/table results.
- With a supplied 5°/s pitch rate and 45° bank, transformed fixed8 rates at 80°
  and 90° pitch both read `[5123, 904, 5203]`; local transform pitch limiting
  does not change the movement-angle integrator's full-loop behavior.
- Supplied weight 30,000 at exact zero pitch/bank gives lift 7,680,000 and equal
  opposite gravity in native force units; assembled down force is zero.
- Synthetic tests verify flap/gear branch differences, lift cutoff/floor,
  thrust fuel/vector gates, overload buckets, signed shifts and separate wind.
- Asset guards passed for the repository and both debug executables.
- No renderer/playable-input changes; GPU smoke tests were not repeated.
  Linux/Windows validation remains unavailable on this host.

Evidence: `.local/native-flight/rotations-{tests.log,report.txt}` and
`rotations-final/{reviewed-components.json,tables/inventory.json}`. These are
translated-component checks, not complete native trajectories. The playable
adapter remains unchanged; the previous loop/banked-pull baselines still describe
it. Main remaining work is complete matrix/display composition, loaded control
and equipment state, contact handling, and clock/RNG integration.

## Fourth pass validation — 2026-09-13

- macOS arm64, pinned Rust 1.91.1: formatting, Clippy (all targets, warnings
  denied), workspace build and **107 Rust tests** passed; **9 Python tests** passed.
- Asset guards passed for repository files, both debug app/extractor binaries and
  the new `native_composition` example. `git diff --check` passed.
- Static extraction to `.local/native-flight/composition-final` ran twice with
  identical outputs accepted: 43 reviewed regions, 107 SMS-selected spans,
  3,829 symbols, bounded 642-byte sine and 1,028-byte atan tables.
- Imported-table example passed. Supplied forward/side/down fixed8 velocities
  `(128000,2560,1280)`, 30-degree movement bank, zero heading, 2-degree slip
  and 5-degree AoA produce the following diagnostic outputs:

| Movement pitch | World velocity fixed8 XYZ | Display heading/pitch/roll PA |
| --- | --- | --- |
| 0° | 1576, −2389, 127996 | 771, 606, −5493 |
| 30° | 1576, 61928, 112026 | 920, 6060, −5953 |
| 60° | 1576, 109658, 66033 | 1704, 11465, −6971 |
| 90° | 1576, 127988, 2349 | 23325, 15401, −28772 |

- Synthetic tests cover exact contact tolerance/surface thresholds, hold-timer
  crossing, touchdown settling and flag transitions; equipment sentinels/tank
  fuel and control reduction order; matrix saturation and bounded tables;
  seeded RNG versus an independent wide modular reference, replay and no-draw
  branches; pause, timer clamps/word wrap and 120-step fractional accounting.
- The example's four seeded bound-256 draws are 136, 107, 13, 71. Its authored
  fixed-clock bridge sums to 256 native units in 120 steps. These are translated
  diagnostic outputs, **not** a comparison against running the original game.
- No renderer or playable flight response changes in this pass. GPU smoke tests
  were not repeated; Linux/Windows checks were unavailable on this host.

Evidence: `.local/native-flight/composition-{tests.log,report.txt}` and
`composition-final/{reviewed-components.json,tables/inventory.json}`. Open gates:
terrain/carrier query production, touchdown event effects, remaining loaded
field producers/semantics, seed and RNG consumption order, native scheduling,
overflow edges and full-trajectory validation. Contact arithmetic is more complete;
the complete contact system is not yet ported.

## Fifth pass validation — 2026-09-13

- macOS arm64: 110 Rust tests and 9 Python tests passed. Formatting, all-target
  Clippy with warnings denied, workspace build and asset guards passed.
- Static extraction to `.local/native-flight/queries-final` ran twice with
  identical output accepted: 52 reviewed regions, 107 selected spans and 3,829
  symbols. The reseed entry-reference index reports the four reviewed callers.
- Synthetic tests cover preferred/fallback object lookup, horizontal approximate
  distance, reverse-order ties, empty inventories, request/object query flags,
  signed reseed extremes including zero and −32768, unconditional chance draw
  consumption, and unsigned due-time comparisons across word wrap.
- Python fixture verifies incoming entry references separately from outgoing
  edges. Source media remains hash-gated; no imported code was executed.
- Evidence: `.local/native-flight/queries-tests.log` and
  `queries-final/reviewed-components.json`. Playable flight/rendering unchanged;
  no GPU smoke repeated and no Linux/Windows host validation performed.

This establishes additional helper contracts, not a complete terrain collision
engine or native scheduled replay. Remaining work is listed in the fifth-pass
format notes and progress checklist.
