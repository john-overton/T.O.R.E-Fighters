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

## Live palette and deck foundation — 2026-09-15

- Bounded root `+0x6c` reader decodes 48-byte shade headers and up to ten
  256-entry index remaps. FA `0x4b3ad0` chooses the first minimum Manhattan RGB
  distance; `0x4b3410` quantizes density and saturates the final level.
- Tint reduction at `0x4b36ae` is gated by layer overlap. The selected object's
  `+0x34` is signed 24.8 speed, confirmed by the object/context copy into
  `0x50ceb4`. Fields `+0x12e/+0x132` supply maximum reduction/speed cap.
- The palette worker at `0x486e80` invokes the palette pass every fourth 15 ms
  iteration. Host presentation owns smoothing/reduction/RNG and uses nominal
  60 ms fixed-tick passes; pause behavior, startup phase and independent seeded
  streams are authored scheduling, not native replay parity.
- Imported terrain remaps run before palette lookup/filtering. Aircraft retain
  their own palette and fitted RGB haze. Native cross-altitude ray composition
  (`0x4b31f0`) remains open; GPU distance/filtering are not the integer rasterizer.
- Named sky/ocean decks use the plane altitude, `2^exponent` feet tile size and
  reversed Z texture coordinate verified in `0x447aa5` and `0x448400`. Wildcards
  resolve once in record/deck order. CLI/app extraction now includes OCEAN PICs
  and rejects old caches missing them. The native special horizon fill and
  above-sky branches remain open; horizon minification is visibly aliased.
- Validation: 263 Rust tests, Clippy, build and 24 Python tests passed. Linux
  viewer and F18 capture passed; `.local/weather-foundation/planes.png` was
  visually inspected. These establish a working GPU path, not retail equality.
- Follow-up creator smoke and hash-gated static extraction passed. Bounded active
  fog sample: 330 frames, 300 measured, zero paused frames; mean 1.35 ms, p95
  1.53 ms, simulation/cameras 0.13 ms. CPU intervals include presentation;
  this is not a matched before/after performance claim. Artifact guards passed.

## Celestial slice — 2026-09-15

- Separate bounded, straight-line weather SH reader: source vertex slots/axes,
  fill changes, circles, point stars, UVs and textured billboards. It rejects
  executable opcodes; the sun's projected-point publication is recorded without
  writing imported pointers. The aircraft shape reader is unchanged.
- Import validation now requires successful weather primitive decoding: SUN
  seven concentric circles, MOON one billboard, STARS 94 points, CLOUDS sixteen
  billboards. CLOUD1 retains two polygon faces. The moon uses `_MOON.PIC` (41²).
- Sun placement translates the source inclusive time/flag gate, integer arc and
  signed-WORD reflection. Moon uses LAY angles; stars retain source directions.
  Rendering is independent of camera translation. The GPU uses floating-point
  projection, one-pixel star quads and horizontal horizon clipping; native
  screen rounding/horizon dip remain comparison work.
- Sun fill 267 resolves through LAY root `+0x50`. Its six outer circles repeatedly
  remap the background index (native `0x497836`); the solid inner source fill
  remains indexed. No replacement sun texture or fitted glow color is used.
  Glare through the separately published `_sunPoint` remains unimplemented.
- Linux captures: `celestial-sun.png`, `celestial-night.png`, `moon-viewer.png`
  inspected under `.local/weather-foundation/`. The first moon cockpit poses
  placed it behind the canopy frame; the unobstructed viewer confirms the
  original textured disc and stars. This is not a retail side-by-side gate.
- Celestial checkpoint checks: 265 Rust tests, Clippy with warnings denied,
  locked build, 24 Python tests, creator/viewer/flight GPU checks and asset
  guards passed. Night active sample: 330 frames/300 measured, zero paused;
  mean 1.38 ms, p95 1.57 ms. No matched retail or Windows/macOS runtime check.
