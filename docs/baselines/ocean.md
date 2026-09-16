# Ocean motion acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

2026-09-16, Linux x86_64, NVIDIA GeForce RTX 4070 / Vulkan, pinned Rust 1.91.1.
Research followed by implementation of [the ocean spec](../spec/ocean.md).
The initial whitecap trial below is historical. The current revision removes
whitecaps and retains retail textures/colors while refining short surface ripples.
[Source identity and remaining gaps](../formats/ocean.md).

## Initial trial reproduction

```sh
python3 tools/extract_assets.py --source gameassets/fighters-anthology --out .local/ocean-motion/assets --include 'WAVE*' --include 'OCEAN*.PIC'
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/ocean-motion/native --domain weather
TORE_OCEAN_PHASE=0 target/debug/tore-app --capture-terrain .local/ocean-motion/phase0.ppm --no-audio
TORE_OCEAN_PHASE=3 target/debug/tore-app --capture-terrain .local/ocean-motion/phase3.ppm --no-audio
TORE_WEATHER_VIEW=1070000,1500,687000,17,-35 TORE_OCEAN_PHASE=0 target/debug/tore-app --capture-terrain .local/ocean-motion/sea0.ppm --no-audio
TORE_OCEAN_MOTION=0 target/debug/tore-app --viewer --no-audio
```

The native pass includes water initialization and dispatch alongside the existing
repeat helpers. Original wave modules were separately read as bounded inert CODE
data and their embedded frame branch disassembled; no SH module was executed.
Raw inventories, extracted data, captures and performance logs stay ignored under
`.local/ocean-motion/`. The stale app cache was rejected for missing wave art and
automatically re-imported from the available local media.

## Initial trial results (superseded presentation)

- Formatting, warnings-denied workspace/all-target Clippy, locked build and
  **370 Rust tests** passed. Tests cover bounded/repeating shared phase, static
  mode, invalid phase inputs, unchanged phase for paused time, and exact wave
  resource selection across all sixteen theater profiles.
- **40 Python tests**, documentation headers, diff whitespace, repository asset
  guard and both debug executable asset guards passed. New documents also carry
  the current header; the header tool only discovers tracked paths.
- Menu/creator and viewer GPU paths pass. Wide F18 cockpit and tall Rafale cockpit
  captures pass with mirrors; active camera-panel performance runs complete
  asynchronous readbacks. Clear, cloudy, foggy, dawn, sunset and night captures
  pass. The cloudy/foggy/night source deck choices can leave no visible textured
  ocean; those fallback paths stay unchanged. Egypt and Kurile captures pass.
- The 1,500-foot sea view visibly shows whitecaps and ripple shading. Sixteen
  fixed-camera captures at 0.25-second phase intervals produce
  `.local/ocean-motion/ocean-preview.mp4`, a four-second preview sampled at the
  source whitecap frame rate, plus a looping GIF. The GIF loops only that excerpt,
  so its end-to-start ripple jump is not a runtime phase discontinuity.
- The Ukraine overview phase-0/phase-3 comparison changes **35,408 / 691,200**
  pixels. Selected sky (0,0)-(960,140) and opaque grass (0,395)-(200,430) rectangles
  are byte-identical. Original texture cutouts elsewhere also expose moving water.
  A repeated phase-0 sea capture is byte-identical to the first one. These are
  render regressions, not retail comparison evidence.

## Initial trial performance

630 frames per run, first 30 excluded, 1280x720 F18 flight, calm wind, active
simulation and page-3 camera instrument. All samples are sequential. Three
matched static/moving pairs use the same rebuilt binary:

| Mode | Mean frame intervals, ms | p95 intervals, ms |
| --- | --- | --- |
| Static, `TORE_OCEAN_MOTION=0` | 2.03 / 1.96 / 2.09 | 3.19 / 3.07 / 3.98 |
| Moving, default | 2.15 / 2.25 / 2.19 | 4.05 / 4.34 / 4.06 |

Average means are 2.03 versus 2.20 ms, approximately +0.17 ms. Each run renders
630 mirrors, has zero paused frames, and completes 13-15 camera readbacks. The
initial pre-change sample was 2.10 ms mean / 4.23 p95; the first post-change
sample was noisier at 2.76 / 12.12 ms, prompting the matched repetitions above.
These are short CPU wall-time/presentation-inclusive observations, not GPU
queries, displayed FPS or sustained worst-case qualification. No blocking live
readback or sleep was introduced.

This pass does not establish retail animation equality, native water-effect
culling/phase parity, wave physics or Windows/macOS rendering. The recovered
whitecap timing/art and the newly authored ripple motion remain separate.


## Current revision: short ripples, original art and colors

User direction on 2026-09-16 supersedes the whitecap trial: remove whitecaps,
use tighter/shorter ripples, retain retail textures and colors, and transition
from pixelated near detail to smooth distant/high views. The image is a pattern
reference, not authorization to replace the ocean palette with turquoise.

The user-suggested WaterSurfaceRendering repository was read at commit
`416d31648ed19fb12077bd7ee20ae0f6257a9a1a`. Its separation of wave slopes/normals
from view-dependent reflection informed the design. No third-party code, assets,
FFT library, mesh system, atmospheric model or water-color constants were copied.
The reference checkout stays ignored under `.local/ocean-refinement/`.
[Current authored rules and constants](../spec/ocean.md).

Runtime whitecap drawing, atlas uploads, app cache requirements and shared-profile
wave dependencies are removed. The source investigation remains documented.
The surface now uses short procedural gradient-noise slopes, preserving original
ocean art and reflecting original sky colors. Pixel size, viewing distance and
altitude control filtering and the detail fade. No texture generation was needed.

```sh
TORE_WEATHER_VIEW=1070000,500,687000,17,-35 TORE_OCEAN_PHASE=0 target/debug/tore-app --capture-terrain .local/ocean-refinement/near0.ppm --no-audio
TORE_WEATHER_VIEW=1070000,5000,687000,17,-35 TORE_OCEAN_PHASE=0 target/debug/tore-app --capture-terrain .local/ocean-refinement/sea5000.ppm --no-audio
TORE_WEATHER_VIEW=1070000,16000,687000,17,-35 TORE_OCEAN_PHASE=0 target/debug/tore-app --capture-terrain .local/ocean-refinement/sea16000.ppm --no-audio
```

Captures, logs and the earlier binary for matched benchmarks stay in ignored
`.local/ocean-refinement/`. Current validation results are recorded below.

### Current validation

- Formatting, warnings-denied workspace/all-target Clippy, locked build,
  **369 Rust tests**, **40 Python tests**, documentation and asset guards passed.
  Removing the no-longer-applicable wave import test accounts for the test-count
  difference from the initial trial.
- Real GPU captures cover 500, 1,500, 5,000 and 16,000 feet plus a grazing horizon
  view. The phase-repeat capture is byte-identical. At 16,000 feet phase 0 and 3
  are byte-identical, confirming the detail cutoff. Near phases differ visibly.
- Menu, creator, viewer, both aircraft, mirrors, six weather choices, Egypt and
  Kurile render checks pass. Camera-panel performance runs complete asynchronous
  readbacks. No additional textures are generated or imported.
- Preview: `.local/ocean-refinement/ocean-preview.mp4`; altitude comparison:
  `.local/ocean-refinement/altitudes.png`. Captures are local retail derivatives.

Three sequential 630-frame pairs compare the saved whitecap-trial binary with
this revision, 1280x720 F18/page-3 active flight and calm wind, first 30 frames
excluded:

| Version | Mean frame intervals, ms | Median intervals, ms | p95 intervals, ms |
| --- | --- | --- | --- |
| Previous whitecap trial | 2.18 / 1.99 / 2.20 | 1.49 / 1.47 / 1.47 | 11.62 / 3.76 / 11.56 |
| Short-ripple revision | 2.23 / 3.04 / 2.59 | 1.50 / 1.56 / 1.54 | 11.69 / 11.84 / 11.77 |

Average means are 2.12 versus 2.62 ms. This is a measured increase, with variable
presentation/backpressure tails; the new appearance is not performance-neutral.
An earlier revision that still evaluated invisible slopes averaged 2.80 ms.
The final shader skips unresolved detail, with the same visual fade. These are
CPU wall intervals, not GPU timings or verified displayed FPS. The 500-foot
water-heavy view was visually checked but not separately benchmarked. Windows
and macOS rendering remain untested; no retail parity claim applies to this
user-directed appearance change.

### Reflection reduction and fade diagnosis

On 2026-09-16 John requested 25% weaker reflections. The reflection blend now
scales by 0.75 after the existing cap; retail texture/palette values are unchanged.
Formatting, warnings-denied Clippy, 369 Rust tests, 40 Python tests, locked build,
asset guards, Linux Vulkan creator/viewer smoke tests and a 500-foot ocean capture
passed. Logs and capture are in `.local/ocean-dimming/`. Windows/macOS checks
were not run for this adjustment; prior performance results above predate it.

Code inspection distinguishes the terrain far clip (2,200,000 feet), sky/ocean
deck forward-depth limit (2,000,000 feet), and the much nearer ocean detail fade.
The full theater mesh is submitted, rather than a plane-centered terrain radius.
At 500 feet above water, 720 pixels high and zoom 1, the 12-24 feet-per-pixel
filter fades remaining ocean detail over approximately 1,934-2,736 feet of
viewing distance. This is calculated from the shader's projected footprint,
not a measured visual boundary. It likely explains the reported nearby band;
this diagnosis motivated the broader transition below. The sky uses a separate
horizon gradient.

### Broader nearby transition

John approved this implementation adjustment on 2026-09-16. Fine/coarse ripples
fade earlier using square-root footprint interpolation, while smooth reflection
persists beyond the resolved ripples. Constants are in the [spec](../spec/ocean.md).
The 25% reflection reduction remains active. At 500 feet and 720p/zoom 1, the
reflection filter now spans approximately 790-5,471 feet of viewing distance,
rather than 1,934-2,736 feet. The unresolved slopes still reach zero at the old
sampling limits, avoiding extending fine waves into undersampled distances.

Matched 500/1,500/5,000/16,000-foot captures and Linux Vulkan creator/viewer smoke
tests passed. The 500-foot before/after comparison was visually reviewed: the
nearby bright-to-flat band is more gradual. At 16,000 feet the captures remain
byte-identical. Local evidence: `.local/ocean-fade/`, including `comparison.png`
(before left, after right). Formatting, warnings-denied Clippy, 369 Rust tests,
40 Python tests, locked build, documentation and asset guards passed.

Three sequential matched 630-frame F18/page-3 calm-wind runs at 1280x720, excluding
the first 30 frames, produced mean CPU frame intervals of 3.19/2.85/2.83 ms before
and 3.44/2.30/2.24 ms after. All had zero paused frames and completed asynchronous
camera readbacks. Presentation variability is substantial; these short runs do
not establish a speedup or GPU timing. Windows/macOS and manual moving-camera
acceptance remain untested for this adjustment.

### Horizon-reaching shading, superseding the nearby fade

John rejected the previous broadened fade as still too close on 2026-09-16.
The current implementation removes the footprint/distance cutoff from reflection,
adds three subtle longer ripple bands, and blends reflection into the horizon by
view angle and weather haze. See [current constants](../spec/ocean.md). Earlier
captures and the statement that high-altitude output is unchanged describe
previous revisions: high-altitude motion still stops, but smooth reflection now
remains active there.

Linux RTX 4070/Vulkan captures at 500/1,500/5,000/16,000 feet and creator/viewer
smoke tests passed. The 500-foot before/after and all four final altitude views
were visually reviewed. Smooth shading continues toward the horizon instead of
ending at the old nearby band. Identical phase-0 captures match byte for byte;
phase 1 differs. Local captures/logs are in `.local/ocean-horizon/`, with
`comparison.png` (before left, after right) and `altitudes.png` (500/1,500 feet
above 5,000/16,000 feet). Formatting, warnings-denied Clippy, locked build,
369 Rust tests, 40 Python tests, documentation and asset guards passed.

Three sequential 630-frame matched F18/page-3 calm-wind runs at 1280x720,
excluding 30 warmup frames, measured mean CPU frame intervals of 2.13/1.95/1.97 ms
before and 2.06/1.93/2.08 ms after (averages 2.02 versus 2.02 ms). All report zero
paused frames and 13-14 asynchronous camera readbacks. These short samples include
presentation backpressure; they do not establish GPU cost or worst-case low-ocean
performance. Windows/macOS and manual moving-camera acceptance remain untested.

### Five-statute-mile transparency trial

John proposed keeping fixed-size ripples farther out and gradually reducing the
shader's opacity on 2026-09-16. The current local trial supersedes the preceding
horizon/larger-band approach: whole-effect opacity fades from 2,700 to 26,400 feet
of horizontal distance, blending with the unmodified retail water sample. Water
coverage remains opaque. [Current rules and interpretation](../spec/ocean.md).

Matched 500/1,500/5,000-foot captures, 12 animation phases at 8 fps, and Linux
RTX 4070/Vulkan creator/viewer smoke tests passed. Local evidence is in
`.local/ocean-five-miles/`: `comparison.png` has the preceding horizon version on
the left and the trial on the right; `preview.mp4` contains the trial animation.
Visual inspection confirms the fixed-size ripples extend much farther. Distant
undersampled ripples are visibly grainy; the authored contrast reduction is not
a complete antialiasing solution and temporal shimmer remains a limitation to
review. This is a visual trial, not a claim of final visual acceptance.

Formatting, warnings-denied Clippy, locked build, 369 Rust tests, 40 Python tests,
documentation and asset guards passed. Three sequential matched 630-frame
F18/page-3 calm-wind 1280x720 runs, first 30 frames excluded, measured mean CPU
frame intervals 2.32/2.28/2.57 ms before and 2.39/2.33/2.49 ms after (averages
2.39 versus 2.40 ms). All report zero paused frames and 15-17 asynchronous camera
readbacks. These are short CPU/presentation measurements, not GPU timings or a
worst-case water-heavy benchmark. Windows/macOS rendering was not tested.

### Distance-dependent highlight reduction

John preferred the five-mile trial and requested matching luminosity falloff on
2026-09-16. Reflection strength now receives the same distance-opacity factor
before whole-effect blending. Its contribution therefore falls with opacity
squared, preserving nearby brightness and the original base water colors.
The change lowers distant reflected contrast; it does not remove aliasing.

Matched 500/1,500/5,000-foot captures and Linux RTX 4070/Vulkan creator/viewer
smoke tests passed. The 500-foot comparison was visually reviewed; its difference
is subtle, with near ripples preserved and distant shading reduced. Evidence is
in `.local/ocean-highlight-fade/`, including `comparison.png` (before left,
after right). Formatting, warnings-denied Clippy, locked build, 369 Rust tests,
40 Python tests, documentation and asset guards passed. Prior performance data
predates this scalar adjustment; no new timing claim is made. Windows/macOS and
moving-camera shimmer acceptance remain untested for this adjustment.

### User acceptance

After testing the final five-mile ripple and highlight fade in game, John
reported that it looks good and approved keeping this version on 2026-09-16.
This closes the requested visual tuning review; it does not establish retail
parity, complete antialiasing, or Windows/macOS rendering acceptance.
