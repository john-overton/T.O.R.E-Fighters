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
| Building distance (city fill) | 10 nm, 20 nm, 30 nm | 30 nm |
| Building detail (city fill) | 1 nm, 2 nm, 5 nm, 10 nm, 20 nm | 5 nm |
| Ground detail | Off, On, Fine | On |
| Ground detail distance | 1 nm, 2 nm, 4 nm | 2 nm |
| Vegetation distance | 2 nm, 4 nm, 6 nm, 8 nm | 4 nm |
| Vegetation detail | 1 nm, 2 nm, 3 nm, 4 nm | 2 nm |
| Vegetation density | Low, Med, High, Ultra | High |

Choices are saved in `graphics-v1.conf` in the application data directory,
beside `preferences-v1.conf`. A missing file uses the defaults quietly; an unreadable or malformed one uses them and logs a warning saying why.
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

## Building distance and building detail

Shown whenever the city fill is on, which it is by default (hidden only
under the developer switch `TORE_CITY_FILL=0`; see the
[feature matrix](../features.md)). John asked
on 2026-10-10 for both settings and their defaults; the steps are agent
choices. Building distance is how far city fill buildings are drawn.
Building detail is the distance inside which they are drawn with full
texture and detail; beyond it they are plain blocks in each building's
main colour. Over a band a quarter beyond the detail distance the block
fades out over its full detail building, so the change is gradual and the
edge does not show as a line. Buildings also fade in rather than appear:
over the last 15% of the building distance (4.5 nm of the default 30) and
while they grow from one to two pixels on screen (John, 2026-10-11, "let's
try phase in transitions"; the band widths are agent choices, fitted). The
fade is a fixed dither pattern spread over each pixel's anti-aliasing
samples, so with 4x a pixel blends in quarter steps and a still view never
shimmers; with anti-aliasing off it is a screen-door pattern. The target
display (target window, TV and gunsight views) draws every building it sees
at full detail whatever the setting, and fades them in at its own draw
distance. The values are saved
only when they differ from the defaults. Measured costs are in the city
fill notes; at 4K the 5 nm default adds about 2 to 5 ms over Kiev, 1 nm
about 1 ms. The fades cost about 0.1 to 0.4 ms more at 4K (2026-10-12).

## Vegetation texture and vegetation objects

Shown when the exploratory vegetation is on, which it is on this branch
(hidden only under the developer switch `TORE_VEGETATION=0`). John asked
on 2026-10-16 for the same kind of control the city buildings have. The
steps and the defaults are an agent choice the same day, aimed at an
RTX 4070 at 4K. Vegetation is presentation only: it is not collidable,
and it is not part of the simulation, the network session, replays or
checkpoints. What was drawn is seeded from the theater and the tile, so
every machine that uses the same settings shows the same scene.

**Ground detail** is the close-range ground detail: furrows on
fields, grass, a mottled forest floor, sand grain, ripples and dunes,
pebbles on gravel, and scree on rock. It is described in
[terrain detail](terrain-detail.md#exploratory-vegetation-and-ground-detail).
Off leaves the retail tile. On is the ordinary grain. Fine is a
tighter grain. **Ground detail distance** is how far that grain
reaches. It also fades with height, full below about 2,200 ft and gone
by about 7,500 ft, so a cruise view stays the retail tile; the desert's
broad colour fields and dune shading reach farther and fade out by
about 26,000 ft. A flare,
afterburner, fire or gun flash only adds light to it: the grain is the
same with and without one.

**Vegetation distance** is how far trees, bushes, palms and rocks are
drawn, the same idea as building distance. **Vegetation detail** sets
how far they stay at full density: trees are at full density out to
0.4 times the detail distance (about 4,900 ft at the default 2 nm) and
thin beyond it, smaller things sooner (bushes and boulders at about a
third of that, single stones at about an eighth), and the grass carpet
reaches half the detail distance at most. Tree clumps (retail grove
shapes) stand in for single trees from about a third of the full
density distance outwards. The trees, bushes, palms and rocks are the
camera-facing sprites from retail art, low-poly fallbacks or John's generated v2
pack. The developer environment switch `TORE_VEGETATION_SPRITES=retail|pack|mixed`
is read at scene startup, with `mixed` the branch default. There is no new saved
Graphics row, since changing this art requires rebuilding the scene atlas. Sprites draw with a 1.5 pixel size floor. **Vegetation density** is Low, Med, High or Ultra
(about 0.4, 0.7, 1.0 and 1.65 times the High placement), and also sets
the grass carpet:

| Density | Grass radius | Tufts per 32 ft patch near the camera | Objects kept at most |
| --- | --- | --- | --- |
| Low | 700 ft | 48 | 65,536 |
| Med | 1,100 ft | 96 | 131,072 |
| High | 1,500 ft | 160 | 262,144 |
| Ultra | 2,200 ft | 256 | 524,288 |

Each tuft has five blades. Grass thins in four rings: full within 14%
of its radius, then half, a quarter and an eighth. Where a view would
hold more objects than the density keeps (a deep forest at High would
hold several times as many), full density reaches less far, for every kind
of object alike, so the view thins sooner instead of stopping short of
the vegetation distance. With the larger budget Ultra keeps about the
same thinning distance as High in such places, so it is denser at
every distance and reaches farther. Objects also fade out with height,
full below about 7,000 ft and gone by about 15,000 ft, and nothing is
placed above 16,000 ft.

The five values are saved in `graphics-v1.conf` only when they differ
from the defaults (`vegetation-texture`, `vegetation-texture-distance`,
`vegetation-distance`, `vegetation-detail`, `vegetation-density`). An
older file that has none of them reads as the defaults. Apply puts them
into effect at once, the same way as the building settings. Original
graphics turns the texture off and leaves the object distances at the
defaults, as it does for buildings.

## Command line

For one run, without saving: `--anti-aliasing off|2x|4x|8x`,
`--render-scale 75|100|125|150|200`, `--spotting-aid off|subtle|strong`,
`--terrain-filtering on|off`, `--building-distance 10|20|30`,
`--building-detail 1|2|5|10|20`, `--vegetation-texture off|on|fine`,
`--vegetation-texture-distance 1|2|4`, `--vegetation-distance 2|4|6|8`,
`--vegetation-detail 1|2|3|4`, `--vegetation-density low|medium|high|ultra`,
and `--original-graphics` for everything off
at 100%. Flags apply in order, so `--original-graphics --spotting-aid strong`
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
