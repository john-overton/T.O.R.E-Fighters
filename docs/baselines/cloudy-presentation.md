# Cloud occlusion and overcast water validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-16. Build `d7c64a4` plus working changes.
Linux, NVIDIA RTX 4070, Vulkan. Authored presentation, not retail parity.

The weather validation report confirms CLOUD1 has a dense 4,500..9,500-foot
band, full distance haze at 5,120 feet, and no named ocean or sky deck. Palette
remaps alone left land/water differences and small texture marks visible through
that band. No named ocean deck meant the ocean ripple path did not run at all.
See [occlusion rules](../spec/atmospheric-distance.md) and
[overcast water rules](../spec/ocean.md).

All required formatting, warnings-denied Clippy, locked workspace tests/build,
Python tests, source/binary asset guards and documentation checks passed. The
GPU smoke test passed. `--validate-weather` also completed successfully.

Captures use `--weather-condition 1 --capture-terrain PATH --no-audio` and
`TORE_WEATHER_VIEW=x,y,z,yaw,pitch`. Local artifacts are ignored retail-derived
images and must not be committed.

- Occlusion: `1070000,11827,590000,45,0`, plus altitude 10093/pitch 20,
  altitude 7000/pitch 0, and altitude 3000/pitch -20. Above-cloud shoreline and
  texture speckles no longer show through the thick layer. The inside-cloud
  frame is opaque. These are viewer checks, not exact reproductions of the
  user's banked cockpit poses.
- Clear-day and stepped captures at `1070000,5000,590000,45,0` each have zero
  changed pixels against the previous corresponding captures. These comparisons
  were run after the occlusion change; water reflection is separately gated to
  dense weather and smooth mode.
- Dense fog follow-up: condition 2 at `1070000,400000,590000,45,-60`,
  `1070000,11827,590000,45,-20` and `1070000,300,590000,45,-20`.
  Cloud condition 1 at altitude 11827/pitch -20 and altitude 7000/pitch 0.
  These exercise maximum viewer height, above-layer and inside-layer views.
  The final extinction rule is fully opaque after 600 weighted feet, with
  approximately 14 percent contrast remaining after 300 feet. FOG1's source
  dense band spans 0..8,000 feet, full source distance haze at 5,120 feet.
- Water: `1170000,1200,690000,45,-20`, with motion enabled and
  `TORE_OCEAN_MOTION=0`. The enabled frame has visible short ripples and diffuse
  cloud reflections; the static frame retains the palette-water background.
  Comparison: `.local/atmosphere/cloud-water-comparison.png`, static left,
  enabled right. Early candidate poses on land were not used as water evidence.

No independent city-light on/off bug was established. The small marks in the
cloud report are treated as residual surface-texture contrast, and all surface
contrast is occluded consistently. Reflection reuses original cloud art on an
approximate repeating plane; it does not mirror individual sheet placements.
Cloud volumes remain flat and lack internal texture. No performance benchmark,
continuous climb movie, Windows/macOS test or exhaustive night/fog matrix was run.
