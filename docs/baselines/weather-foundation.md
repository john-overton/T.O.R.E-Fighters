# Weather foundation implementation — 2026-09-15

First bounded implementation slice of dependency step 1. Steps 1–3 are not
complete. Existing uncommitted review changes were preserved; nothing was
committed or pushed. Source identity is the reviewed FA.EXE/SMS pair in
[weather research](weather-research.md).

## Changes

- LAY callbacks resolve through bounded CODE aliases and import records to
  typed Rust behavior. Unsupported symbols, libraries, ordinals and aliases
  fail explicitly. Imported native code is never loaded or executed.
- Environment state owns mutable record copies and a validated, separately
  seeded RNG. Matching fog records mutate before selection/blending and retain
  their changes. Selection uses one second after a callback, ten otherwise,
  with early reselection on leaving the first active interval. Sampling cannot
  consume randomness or change time. Seed 1 and the dedicated RNG stream are
  authored defaults, not retail shared-RNG or scheduler parity.
- Altitude haze uses the source tint RGB at `+0xfb`. The previous implementation
  incorrectly used the remap shade at `+0x36`.
- Pure palette helpers preserve the reviewed selective ranges, signed rounding,
  odd-strength truncation, capped instrument range and smoothing overshoot.
  **These helpers are diagnostic: rendered palettes still ignore tint scalar.**
  View-dependent reduction, complete palette ordering/cadence and native remaps
  must be recovered and integrated before visible fog tint is complete.
- Static extraction includes altitude haze, tint helpers and celestial dispatch.
  `--validate-weather` now reports callback/tint fields and shape coverage.

## Validation

- Formatting, warnings-denied workspace/all-target Clippy, locked workspace
  tests and build passed: **261 Rust tests**, **24 Python tests**.
- Asset guards passed for source and debug app/extractor binaries.
- Synthetic checks cover callback aliases/truncations, persistent fog mutation,
  immutable configuration, camera-query purity, selection intervals and early
  interval exit, haze color source, and palette range/rounding boundaries.
- Imported validation parsed all **24 LAY modules**. Full-day probes for DAY2
  and FOG1 passed; all six condition launch/altitude checks passed. Six FOG
  variants reference the fog callback; other supplied records have null callbacks.
- Linux Vulkan / RTX 4070: creator and fog viewer smoke tests passed. F/A-18D
  fog at 1280×720 after 240 probe ticks and Rafale dusk at 720×960 captured
  successfully and were visually inspected. These confirm composition and
  existing weather rendering, not the unintegrated tint helpers or retail parity.
- Bounded active fog run: 330 frames, first 30 excluded, zero paused frames,
  frame interval mean **1.27 ms**, p95 **1.40 ms**; simulation/cameras mean
  **0.09 ms**; 150 mirror renders, zero completed readbacks. CPU wall intervals
  include presentation backpressure. No matched before-run was made; these are
  neither GPU timings nor proof of a performance improvement.

Captures, provenance and GPU logs are under ignored `.local/weather-foundation/`.
Build/test logs use `/tmp/weather-foundation-*.log` on the reviewed host.
The hash-gated static pass is reproducible with:

```sh
python3 tools/extract_native_flight.py --domain weather --source gameassets/fighters-anthology --out .local/weather-foundation/native
target/debug/tore-app --weather-condition 2 --validate-weather
target/debug/tore-app --viewer --weather-condition 2 --no-audio --smoke-test
target/debug/tore-app --quick-mission --no-audio --smoke-test
```

The initial selective extraction encountered the unrelated WB installer LIB;
rerunning with both `disc1/LHX/*` and `disc1/WB/*` archive exclusions avoids
treating those unrelated formats as EALIB resources.

## Remaining dependency work

1. Finish view-dependent tint reduction/state, palette ordering/remaps and native
   sky/horizon projection. Keep fixed-tick state independent of camera queries.
2. Decode and render original celestial primitives. Current static projection
   stops at SUN opcode `0x13` / VA `0x1034`; MOON and STARS yield no accepted
   geometry. Source dispatch flags/angles are traced, but full clipping,
   materials and rendered movement remain open.
3. Decode cloud primitives and recover distribution/deck composition.
   CLOUD1 yields two static faces; CLOUDS yields no accepted geometry. Neither
   result establishes native transparency, placement or deck crossings.

No matched retail capture, Windows runtime or macOS runtime check was available
in this pass. None of the numbered weather parity gates is marked complete.
