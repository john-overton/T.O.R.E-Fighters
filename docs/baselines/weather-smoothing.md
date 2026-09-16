# Weather smoothing and celestial scale, 2026-09-15

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


## Requested presentation change

The user requested smooth spatial shades and gradual time-of-day colors, while
retaining original art, and supplied default-zoom retail sun and moon captures.
This pass is an **authored presentation enhancement**, not a translation of the
native indexed rasterizer. `TORE_WEATHER_SMOOTH=1` is the default; `0` retains the
stepped weather color/fog path for comparisons. Simulation remains fixed at
120 Hz. Pausing freezes the weather clock; this is mission time, not wall time.

- Pure visual queries resolve source time overlaps at fractional simulation
  time, retaining float six-bit color components until the eight-bit upload.
  They do not advance callbacks, RNG or native record selection. Native texture
  selection/draw flags retain their schedule independently of these colors.
- Terrain/sky altitude haze blends continuously instead of in 256-foot steps.
  GPU horizon/deck shades interpolate adjacent palette colors in linear light.
  Fog interpolates original remap results, preserving target-then-view order
  and source cutout indices. Original sky/cloud/moon texels remain point sampled.
- Tint and sun whitening have separate continuous presentation strengths at
  fixed ticks. Cockpit private colors share those effects, after original HUD
  brightness. Source effects and callback cadence remain available unchanged.
- This does not interpolate different textures, remove source-art detail, or
  blend every discrete native decision. Visibility flags, nearest imported
  remap-bank selection and integer ramp metadata remain discrete. Dedicated
  instrument-window palettes are still a separate integration issue.

## Celestial scale

The user confirmed Fighters Anthology running through dgVoodoo at default zoom;
clear day, probably Egypt for the sun. The moon's mission/theater and exact
camera poses/time are not established. The images show approximately 80–90 px
sun outer glow, 55–60 px bright core and 35–40 px moon diameter when normalized
to a 480-pixel-high view. These are estimates, not exact matching scenarios.

The imported sun has diameters 19 through 13 at depth 400. The moon has a 4×4
billboard at depth 160; its 41×41 texture has a transparent border. The previous
60-degree GPU projection made both much too small. A common **fitted factor 4**
now scales their source geometry. At the view center and zoom 1, the sun's
outer diameter is about 79 px and its core 54 px at height 480. The moon's quad
is 42 px, with its visible disk smaller. Off-center perspective affects measured
bounds and accounts for part of the reference differences.

This factor is **not claimed as recovered native math**. FA 0x4d1836..0x4d1874
still establishes projected sun diameter followed by half-radius rounding;
0x4d54fe..0x4d5565 includes billboard scale/projection globals. Their complete
projection setup relative to the host camera needs a matched retail pose.
Original relative sun rings and moon art are preserved. Geometry scales with
view height and camera zoom, without fixed pixel sizing; equal aspect-correct
X/Y projection preserves circular geometry across wide/tall windows. Moon axes
remain fixed to celestial coordinates and independent of aircraft bank.

## Validation

Ignored artifacts: `.local/weather-smoothing/`. User captures remain local;
no retail images or generated derivatives are committed.

- `--validate-weather` reads all 24 imported LAY modules and retains the full-day
  clock/selection probe. New 60-second dawn probes at 07:06/5,000 ft report six
  palette changes in the native DAY1 path (largest channel step 13/255), versus
  1,744 changed ticks in smooth mode (largest step 1/255). Egypt DAY1E reports
  2,157 changed ticks and the same 1/255 maximum. DAY2's native maximum is 9/255;
  its smooth maximum is also 1/255. Constant cloudy records remain constant.
- Synthetic tests cover fractional time, midnight wrapping, query purity,
  altitude continuity, private palette effect ordering, untouched cutout color,
  moon bank independence and projected diameter across wide/tall sizes and zoom.
- Creator/viewer smoke checks, sun, moon, banked moon, cloud crossing and eight
  flight captures (F18/Rafale, cockpit/exterior, 1280×720/720×960) passed on the
  Linux Vulkan RTX 4070 host. The before/after dawn capture removes the visible
  horizon bands. Sun/moon and flight contact sheets were visually inspected.
- Repeatable active-flight diagnostics: four alternating 3,030-frame runs
  (first 30 excluded), F18 cockpit 1280×720, same host/settings. Stepped means
  1.57/1.78 ms (p95 1.68/1.97); smooth means 1.46/1.75 ms (p95 1.60/1.96).
  All report zero paused frames and completed camera readbacks. Short runs also
  showed variation; these CPU/presentation intervals do not establish GPU time
  or displayed FPS, and do not justify claiming a performance improvement.
- Formatting, warnings-denied workspace/all-target Clippy, 287 locked Rust tests/build,
  24 Python tests and source/app/extractor asset guards passed.

These checks establish implementation behavior on this host, not full retail
or platform parity. Windows **retail** now runs and the supplied captures replace
the earlier unavailable-runtime assumption; Windows/macOS **rebuild** execution
is still unverified. Exact celestial sizing acceptance needs matched pose/time,
and additional cloud/fog and theater comparisons remain in the weather plan.
