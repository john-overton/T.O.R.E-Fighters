# Sun glow validation, 2026-09-16

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode validation of the [sun glow specification](../spec/sun-glow.md).
Build: `d7c64a4` plus working cloud/fog, overcast-water and continuous-sun changes.
Linux, NVIDIA RTX 4070, Vulkan, 2026-09-16.

Passed formatting, warnings-denied all-target Clippy, locked workspace tests
and build, 40 Python tests, source/binary asset guards, documentation headers,
and the display smoke test. A new synthetic regression checks horizon crossings
at 07:00 and 19:00, continued positioning with the source sun flag disabled,
0.25-degree elevations a minute before/after crossings, normalized directions,
and midnight wrap continuity. Glare checks cover 35% at the horizon, 17.5%
at -0.25 degrees and zero at -0.5 degrees. The all-day synthetic fixture
initially exposed a rejected 24-hour daylight span in the visual arc; it is now
supported and the existing glare-toggle regression passes. Simulation sun-angle tests remain unchanged.

Captures use `--weather-condition 4 --capture-terrain PATH --no-audio`,
`TORE_WEATHER_VIEW=1070000,5000,590000,YAW,0` and explicit time overrides:

| Time | Yaw | Result |
| --- | --- | --- |
| 07:01 | 100 | Rising sun and warm directional glow |
| 19:01 | -100 | Sunset preset now shows the sun's remaining visible portion with a warm sky wash |
| 19:10 | -100 | Below-horizon center retains twilight scattering and a small visible edge of the enlarged original sun art |
| 00:00 | -100 | GPU capture passed with the sun below the horizon |

Local comparison: `.local/atmosphere/sunset-review.png`, sunrise, sunset preset,
then twilight. These earlier captures predate the current half-size sun/moon.
Current size validation uses `.local/atmosphere/half-sunset.ppm` and
`half-moon.ppm`. The moon projection regression now expects half the former
screen diameter and one quarter of the squared billboard edge length. The
smaller sun now uses its earlier color treatment without the trial orange rim.
Final sunset capture: `.local/atmosphere/half-sunset-no-grade.ppm`. Original-art captures remain ignored.
Different
source sky palettes make dawn and dusk differ in overall brightness even though
the angular/elevation scattering rules are symmetric. The original enlarged sun
art remains stylized; this is not a calibrated solar angular diameter.

The dense-layer cloud/fog occlusion checks are recorded in
[cloudy presentation](cloudy-presentation.md). No continuous full-day recording,
Windows/macOS validation, frame-time benchmark or exhaustive weather/altitude
matrix was run. This is authored presentation, not measured retail parity or
astronomical accuracy. Terrain is still flat and there is no refraction model.

Glare captures use 18:58 and 19:03 with yaw -100/pitch 4. These check visible
lens artifacts before sunset and their absence after the half-degree cutoff.
Files: `.local/atmosphere/glare-before.ppm`, `glare-after.ppm`. Palette-whiteout
timing is covered by unit tests; one-frame captures do not measure its settling
time. Final required checks and the display smoke test passed after these changes.
