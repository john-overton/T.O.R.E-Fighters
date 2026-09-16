# Atmospheric distance blending

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. John requested gentle horizon blending that responds to
weather and altitude on 2026-09-16. This is **opinionated** presentation; all
constants below are agent choices, not measured original behavior.

Smooth weather adds aerial perspective after the existing weather palette
remaps. Terrain, ocean, fog-enabled objects and finite cloud sheets approach
the current weather's horizontal horizon color in linear light. Render range,
simulation visibility and source weather bands remain unchanged. Stepped mode
(`TORE_WEATHER_SMOOTH=0`) retains its existing appearance.

Clear air remains free of added haze through 50 statute miles (264,000 feet),
interpreting John's 2026-09-16 request as statute miles. Beyond that, extinction
is `exp(-altitude / 18000) / 900000` per foot, analytically averaged along the
view segment. Added clear-air opacity is `1 - exp(-max(distance - 264000, 0)
* mean_extinction)`. At sea level it reaches 25 percent at 100 miles and 59
percent at 200 miles. Existing source palette haze remains in place.
Moisture still accumulates beyond 3,000 feet, independently of the clear-air
threshold, so fog and cloud layers do not inherit a 50-mile visibility floor.

Each weather band's far density divided by its far distance estimates additional
moisture. Subtract the clear-day reference `0.8 / 182283` per foot, floor at
zero, multiply by 0.20 and cap at `1 / 12000` per foot. Integrate its contribution
only over the part of the sightline inside that altitude band. Band edges use
500-foot smoothstep transitions centered on the source bounds. Overlapping
bands add their contributions. This intentionally modest extra blend supplements
the existing, much stronger cloud/fog remaps. Above a moist band, horizontal
views stay clearer while downward views still pass through its haze. Thin bands
are integrated analytically, so long rays cannot skip them between samples.

The high sky artwork fades from 50 statute miles to its 2,000,000-foot drawing
boundary with smoothstep. Distance is the larger of horizontal radial distance
and the source scanline distance, ensuring zero coverage at the drawing boundary.
Resolved RGB moves halfway toward the underlying sky by the end of the fade;
texture coverage also falls to zero. This reduces both luminosity contrast and
opacity, preserving the original cutout alpha. The underlying time-dependent
sky and sun glow show through. Ocean coverage receives the same distance fade,
removing a hard boundary between its resolved color and the horizon backdrop.
Smooth mode also replaces the source plane-transition strips with a continuous
horizon profile: palette index 240 at the horizon, easing to 229 across the
upper 130 source projection units when the upper horizon is enabled, and to
252 across the source lower extent when the lower horizon is enabled. Both
use smoothstep. This removes the backdrop color jump revealed by transparent
sky textures. Stepped mode keeps its source transition strips.
This is an artistic distance rule for an infinite backdrop, not cloud depth.

Finite cloud sheets use their actual distance and altitude. Cloud-sheet count
and placement alone do not infer moisture: source weather visibility bands drive
it. There is no new weather simulator, physical cloud volume or scattering model.
The tint follows the existing time-dependent palette, including night.

See [validation and limitations](../baselines/atmospheric-distance.md).
