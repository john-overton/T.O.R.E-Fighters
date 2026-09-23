# Graphics options

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. John asked on 2026-09-22 for easier-to-see distant
aircraft and for rendering improvements that help with it: anti-aliasing, a
spotting outline, terrain distance filtering and render scale. Everything on
this page is **opinionated**. The features were requested by John; the methods,
constants and defaults are agent decisions, fitted by eye on captures. The
original game had none of these. All options off at 100% is the closest to its
look.

## The Graphics screen

Main menu Pref > Graphics... opens a screen in the same style as the controls
screen. It edits a draft; **Apply** puts it into effect at once and saves it,
**Defaults** loads the recommended settings into the draft, **Cancel** or
Escape throws away anything not applied. Up/Down move between rows, Left/Right
change a value, Enter or Space cycles it, Tab walks every control, and the
mouse picks a choice directly. A one-line description of the focused row is
shown below the options.

| Option | Choices | Default |
| --- | --- | --- |
| Anti-aliasing | Off, 2x, 4x, 8x | 4x |
| Render scale | 75%, 100%, 125%, 150%, 200% | 100% |
| Spotting aid | Off, Subtle, Strong | Subtle |
| Terrain filtering | Off, On | On |

Choices are saved in `graphics-v1.conf` in the application data directory,
beside `preferences-v1.conf`. A missing or unreadable file uses the defaults.
Smoke tests and captures ignore the saved file, like the other display
preferences.

## Anti-aliasing

Multisample anti-aliasing of the 3D view: aircraft, terrain, clouds, horizon,
vapor, tracers and smoke. The cockpit art and HUD are unchanged. 4x works on
every graphics card; 2x and 8x are offered only when the card supports them for
the display format, and unsupported levels are shown struck through. A saved
level the card cannot draw falls back to the next lower level. Changing the
level applies immediately, including in flight.

Cutout edges (water shorelines, airport markings) are decided once per pixel,
so anti-aliasing does not smooth them; render scale does.

## Render scale

The 3D view is drawn at the chosen percentage of the window's pixel size and
resampled to the window. Above 100% each window pixel averages up to 4 by 4
bilinear taps across its footprint (supersampling); below 100% the image is
bilinearly enlarged, trading sharpness for speed. The cockpit and HUD are always
drawn at window resolution.

## Spotting aid

Other aircraft get a thin, pixel-sharp contrasting outline so a small distant
contact can be found without turning it into a marker.

- **Who:** combat targets, dummy aircraft and AI wingmen and enemies. Not the
  player's own aircraft, missiles, tracers, flares, explosions or debris.
- **Where:** the main view and the rear mirrors. Not the instrument camera
  panels.
- **Shape:** drawn after the 3D view at window resolution, without
  anti-aliasing, so edges are always hard.
  - An aircraft 2 pixels or larger gets a 1-pixel border: its silhouette copied
    one window pixel in each of 8 directions, behind the aircraft itself.
  - Under 3 pixels, where there is no silhouette left to outline, it gets a
    solid square mark instead: 2 by 2 pixels under 1.5 pixels, 3 by 3 pixels up
    to 3 pixels.
- **Size:** each aircraft's largest dimension (56 feet for the F/A-18D)
  projected at its distance.
- **Strength:** the product of
  - full until the aircraft fills 1% of view height, fading to zero at 10%;
  - an eyesight limit, full above 4 arcminutes and zero below 1.5 arcminutes
    (about 8 and 21 nautical miles for a 56-foot aircraft);
  - the square root of how much of the aircraft's contrast survives haze and
    cloud, so a fully hazed or clouded aircraft gets nothing;
  - the setting: Subtle 0.5, Strong 0.85.
- **Colour:** strength is a step in brightness, never transparency.
  - Against sky the outline is dark: at full strength 0.2 times the estimated
    background brightness.
  - Against ground darker than 0.3 luminance it is light: 2.5 times the
    background brightness.
  - It keeps 30% of the background's hue.
  - The background is estimated from the horizon colour, or a hazed dark-ground
    value below the horizon; a dark outline can only darken a pixel and a light
    one can only brighten it.
- **Occlusion:** it reads the 3D view's depth, so terrain, cloud and nearer
  aircraft hide it, and it never covers the aircraft itself.

Known differences:

- The background is estimated, not read. Subtle can be faint against the darker
  sky-deck art.
- Hairline wings and tails at mid range do not get their own outline.
- Around 2 to 3 pixels the change from mark to outline can pop.
- Smoke, vapor and tracers write no depth, so the outline draws over them.

## Terrain filtering

Distant terrain texture is filtered by its on-screen footprint instead of
sampling single texels, which removes most of the shimmer and crawl on far
ground. The original palette colours are kept: coarse samples are resolved
through the live weather palette, fog and light rows before blending, never
averaged as indices.

- Each pixel's texture footprint is measured along its long and short axes.
  Up to 4 probes are spread along the long axis; each probe blends the two
  nearest coarse levels (bilinear within each), up to level 8.
- A coarse level L picks one fixed texel per 2^L by 2^L block, chosen by a
  hash of the block and level, so camera motion changes only blend weights and
  never swaps texels.
- Blocks stay inside their 256 by 256 tile. Cutout coverage (water) is averaged
  across the footprint and still cut at one half.
- Off, or a footprint of one texel or less, gives exactly the previous result.
- Terrain only. Airport art sits in padded layers and aircraft use a packed
  atlas, so both would bleed; ocean and sky decks and clouds are unchanged.

Measured on consecutive frames, the pixel change in distant terrain fell by 29%
to 57% depending on the view. Very distant high-contrast art keeps a fixed
grain, and thin features such as far roads fade.

## Command line

For one run, without saving: `--anti-aliasing off|2x|4x|8x`,
`--render-scale 75|100|125|150|200`, `--spotting-aid off|subtle|strong`,
`--terrain-filtering on|off`, and `--original-graphics` for everything off at
100%. Flags apply in order, so `--original-graphics --spotting-aid strong`
turns on only the outline. Pressing Apply on the Graphics screen saves whatever
the screen shows, including a flag's value.

## Cost

Measured in a release build at 1280 by 720 on an RTX 4070, median frame time;
see [flight performance](../baselines/flight-performance.md) for the method.

Free flight with view cycling (`TORE_PERF_FRAMES=630 TORE_PERF_ACTIVE=1
TORE_PERF_VIEWS=1 --free-flight`), three runs each, measured 2026-09-22:

| Settings | Median frame |
| --- | --- |
| `--original-graphics` (everything off) | 4.23 to 4.26 ms |
| Defaults with anti-aliasing off | 4.43 to 4.47 ms |
| Defaults (4x, 100%, Subtle, filtering on) | 4.65 to 4.68 ms |
| Defaults with 8x | 4.88 to 4.89 ms |
| Defaults at 150% render scale | 9.11 to 9.13 ms |
| Defaults at 200% render scale | 14.75 to 14.78 ms |

In a separate ten-aircraft formation run the spotting aid added about 0.04 ms.
Render scale is the only expensive option.

## Set aside

Sun glint (a brief sparkle when sunlight catches another aircraft) was tried and
set aside by John on 2026-09-22. The tried design was an extra pass over other
aircraft that lit a near-white flash when the reflected view ray came within 1
to 3 degrees of the sun, scaled by daylight, cloud, shadow and haze. A
`quality` uniform slot is reserved for it.
