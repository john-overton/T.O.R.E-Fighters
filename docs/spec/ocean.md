# Ocean surface appearance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

John requested ocean motion on 2026-09-16, then directed removal of the trial
whitecaps, shorter/tighter ripples, nearby pixelation and smoother distant/high
views. The supplied water image is a pattern reference. His explicit follow-up
retains the original retail textures and colors, so it is not a recoloring target.

The visual change is **user-directed opinionated**. Agent-selected tuning and
implementation below are authored approximations, not recovered FA behavior.

## Surface and filtering

The original ocean PIC and live weather palette supply water color. A moving
procedural slope field controls short surface ripples and the angle of reflected
retail sky artwork. No replacement bitmap, teal tint or whitecap sprites are used.
The optional static mode retains the earlier unanimated sampling.

The two stretched pattern scales are 100 by 40 feet and 52 by 25 feet, moving
with bounded 12/8-second phases. Slopes drive a six-foot-scaled sampling offset
and a normalized surface normal. Nearby normals use four-foot world cells,
blending to smooth evaluation across 400-2,200 feet altitude and 1-4 feet per
projected pixel. The current five-mile trial keeps those two wavelength bands
throughout its range; the previous three larger bands are removed. To soften
undersampled contrast without a hard cutoff, coarse/fine slopes are multiplied
by `1/sqrt(1 + footprint/80)` and `1/sqrt(1 + footprint/40)`. This is authored
contrast attenuation, not complete spatial or temporal antialiasing. Motion
still fades over 3,500-16,000 feet altitude.

John proposed a five-mile transparency fade on 2026-09-16. This trial interprets
miles as statute miles and measures horizontal distance from the camera to the
water. The complete shaded result, including texture distortion and reflection,
blends back into the unmodified retail ocean sample with opacity
`1 - smoothstep(2700, 26400, ground_distance_feet)`. The start approximates the
former short-ripple cutoff at 500 feet/720p/zoom 1; it is intentionally a fixed
radius rather than changing with resolution. Water itself remains opaque.
Beyond five miles the unmodified ocean sample is used beneath the independent
long-range sun reflection. The earlier
horizon-angle reflection fade is superseded by this trial. This interpretation
and contrast constants are agent-selected tuning, not source facts.

John selected an 85% sun peak and a 30% sky/cloud peak on 2026-09-16. Peak contribution means
`water * (1 - peak) + reflected_source * peak` before distance and weather
attenuation. It is not a fixed 85% brightness applied to the whole water surface.

In smooth mode, angle-dependent reflection uses a Schlick approximation with
0.02 normal-incidence reflectance, capped at 0.25 and scaled by `environment_peak / 0.25`.
The peak is therefore the selected environment value, with 8% of that peak at normal incidence. Stepped compatibility
mode retains its previous `min(fresnel, 0.25) * 0.75` response. These angular
curve choices are agent-selected tuning; the requested comparison peaks are user-directed.
Reflection strength receives the same distance-opacity factor before the final
whole-effect blend, so reflected contrast falls with opacity squared. Haze
further reduces the contribution. The original water sample is never multiplied
toward black. This is an authored interpretation of the requested luminosity,
not a complete antialiasing solution. Reflection samples the original sky deck or palette entry 240,
with weather haze reducing its influence. The reflected sky texture blends into
its palette fallback for reflected-ray Y components 0.45 down to 0.15, suppressing
high-frequency sampling near grazing angles. No third-party water colors, atmosphere
or underwater terrain are introduced. Colors can mix through filtering/reflection,
as they already do through weather presentation; the source palette stays intact.

The user-suggested [WaterSurfaceRendering project](https://github.com/kentril0/WaterSurfaceRendering)
provided concepts: separate surface slopes/normals from reflection and use
view-angle-dependent reflectivity. We use an independently authored WGSL slope
field, not its FFT, displaced mesh, code, textures, Preetham sky or absorption
color model. Reviewed commit: `416d31648ed19fb12077bd7ee20ae0f6257a9a1a`.

## Shared behavior and scope

Beach outlines stay fixed. All views share one simulation clock; pause freezes
motion and restart resets it. Named OCEAN decks use the textured effect; dense cloudy weather without a named
ocean deck uses the palette-water reflection described below. No collision,
buoyancy or wind/sea-state model changes. Runtime no longer imports/uploads wave
atlases; the [retail whitecap research](../formats/ocean.md) is retained as evidence.

[Acceptance](../baselines/ocean.md) records visual checks, performance and platform
limits. Original-game side-by-side comparison remains unavailable.

## Overcast water without an ocean deck

Implementation mode, requested on 2026-09-16. CLOUD1 has no named ocean deck,
so the earlier ripple shader did not run on its water background. Smooth mode
now shades the exposed sea-level background in dense cloudy weather using the
same short ripple slopes, five-mile fade and altitude attenuation. The original
background remains the base color and terrain still covers land. Motion-off
and stepped compatibility modes bypass this addition.

Reflection uses original `_CLOUD1.PIC` artwork projected onto an authored
32,768-foot repeating plane at the dense band's lower edge plus 250 feet.
It is an approximate cloud environment, not a mirrored copy of individual cloud
sheet placements. Cloud cutouts reveal a palette 240-to-229 sky gradient.
Reflection weight is `(0.08 + 0.47 * (1 - facing)^3) * (environment_peak / 0.55)`,
normalizing the existing angular curve to the selected environment peak with the existing
distance opacity applied twice and haze reducing visibility. Nearby ripple
normals therefore remain visible under diffuse overcast lighting. Sampling fades
from reflected elevation 0.02 to 0.15 to avoid grazing-angle texture noise.
All constants are agent-selected opinionated tuning. No new bitmap or daylight
city-light behavior is introduced.

## Separate sun and environment reflection trial

John requested comparison captures with sun reflection at 85% and sky/cloud
reflection at 30% or 50%. These are peak linear-light source contributions,
reduced by the existing distance fade and haze. `TORE_WATER_ENV_REFLECTION`
accepts 0..1 and defaults to the selected 0.3. Stepped mode keeps its
prior reflection behavior. The sky/cloud angular curves retain their shapes,
normalized to the selected environment peak.

A separate smooth-mode sun glint uses the original solid sun radius and palette
color, with a maximum contribution of 0.85. John requested on 2026-09-16 that
this extend across visible water independently of the five-mile detail fade.
The sun reflection is applied after ordinary water shading and haze, with only
the added atmospheric extinction attenuating it; the palette fog ramp and
sky/cloud reflection fade do not truncate it. Dense weather still occludes it,
and source plane coverage fades at the existing maximum render boundary.
Palette-only water also keeps the sun path when dusk removes the named ocean
deck, tapering over 1,800,000..2,000,000 feet. Motion-off mode still bypasses it.

Visible sun fraction uses the circular-disc area above the flat horizon:
`(acos(-q) + q * sqrt(1-q*q)) / pi`, where q is center elevation divided by
angular radius and clamped to -1..1. Thus a half-set sun supplies half peak
energy, and reflection vanishes only when the disc has fully set.

Resolved ripple normals supply the nearby specular pattern. As pixel footprint
grows from 40 to 800 feet, blend toward an angular scatter envelope around the
flat-water reflection. The envelope has Gaussian widths of disc radius plus
1.5 degrees in azimuth and disc radius plus 6 degrees in elevation. Its peak
is 0.55 of the direct reflection peak, avoiding an overly bright solid stripe.
These are agent-selected approximation constants, not a physical scattering
solution. The disc visibility fraction describes the flat horizon. The shared
[geometric shadow pass](surface-lighting.md) additionally attenuates visible
water under terrain and object shadows. At John's request on 2026-09-18,
the direct sun-reflection contribution is also multiplied by geometric light
visibility at the water point and cloud transmission before composition. A
fully blocked point receives zero solar glint, including during partial sunset.
Partial geometric visibility uses the shared distance-dependent penumbra filter,
so land-shadow boundaries fade rather than cutting a hard stripe across water.
The ordinary water color remains visible. This shares the finite shadow-map
coverage and resolution limits; terrain outside that coverage is not traced.
Cloud sheet silhouettes are not traced.
Sky/cloud reflection remains at the selected 30% peak with its existing fade.

See the [water-occlusion GPU checks](../baselines/aircraft-lighting.md).
