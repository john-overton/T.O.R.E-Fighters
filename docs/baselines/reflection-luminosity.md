# Water reflection luminosity validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-16. Build `020b6a7` plus this shader tuning.
Linux, NVIDIA RTX 4070, Vulkan. See [behavior and constants](../spec/ocean.md).

The current trial separates sun reflection (85%) from sky/cloud reflection
(30% versus 50%), per the user's clarification. These are peak linear-light
source contributions. Environment angular curves keep their shapes, normalized
to the selected peak. The separate ripple-following sun glint uses original sun
color/size and is suppressed when the whole disc is below the horizon or by dense weather.
The sun path is independent of the five-mile environment-reflection fade.
Long-range sunset captures use time 19:01 at
`1170000,2217,690000,-100,-4`, frozen phase 0 and lens glare off.
`.local/reflection-options/reach-sunset.png` shows the distant scatter path.
The horizon-visible disc fraction, rather than center elevation alone, controls
its energy; distant ripple detail transitions to a smooth angular approximation.
Matched 19:06 and 19:15 captures confirm the reflection weakens with the last
visible sliver and disappears once the disc is fully set. Comparison:
`.local/reflection-options/reach-times.png` (19:01, 19:06, 19:15).
The complete required check suite and GPU smoke test passed for the final shader.

Validation commands include all required formatting, warnings-denied Clippy,
locked workspace tests/build, Python tests, source/binary asset guards, docs
and display smoke checks, all passed. Clear and cloudy captures show increased
reflection contrast; dense fog still hides the surface. Results are in `.local/reflections85/checks.log`.

Final comparison captures use `TORE_WEATHER_VIEW=1170000,1200,690000,YAW,PITCH`,
`TORE_OCEAN_PHASE=0`, `TORE_SUN_GLARE=0` and environment peaks 0.3/0.5:

| Scene | Condition | Time | Yaw, pitch |
| --- | --- | --- | --- |
| Day | 0 | 12:00 | 45,-20 |
| Sunset | 0 | 18:30 | -100,-12 |
| Overcast | 1 | 12:00 | 45,-20 |

Images under `.local/reflection-options/` carry labels with the scene, time,
sun/environment peaks, altitude, phase and disabled lens glare. Lens glare is
explicitly off to isolate water highlights; reflected sun rendering remains on.
John selected 30% environment reflection with the sun remaining at 85%.
The runtime default is 0.3; the 30% comparison captures show this selection.
Retail-derived screenshots remain ignored and uncommitted.

This is an environment reflection approximation using original sky/cloud art,
not an exact mirror of every rendered cloud. The peak values are shader ceilings,
not a measured screen-pixel brightness; viewing angle, source color, haze and
distance determine the observed result. No performance benchmark or Windows/
macOS validation was performed. No new simulation behavior or dependencies.
