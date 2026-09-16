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
Beyond five miles the unmodified ocean sample is used. The earlier
horizon-angle reflection fade is superseded by this trial. This interpretation
and contrast constants are agent-selected tuning, not source facts.

Angle-dependent reflection uses a Schlick approximation with 0.02 normal-incidence
reflectance, capped at 25% then scaled by 0.75 (18.75% maximum contribution)
to retain the ocean artwork. John requested this 25% reflection-strength reduction
on 2026-09-16. It reduces the reflection blend, not the underlying texture
brightness or palette. On 2026-09-16 John approved the five-mile appearance and
requested matching luminosity falloff to soften the distant grain. Reflection
strength now receives the same distance-opacity factor before the final
whole-effect blend, so reflected contrast falls with opacity squared. Nearby
reflection is unchanged; the original water sample is never multiplied toward
black. This is an authored interpretation of the requested luminosity fade,
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
motion and restart resets it. Only named OCEAN decks use the effect. No collision,
buoyancy or wind/sea-state model changes. Runtime no longer imports/uploads wave
atlases; the [retail whitecap research](../formats/ocean.md) is retained as evidence.

[Acceptance](../baselines/ocean.md) records visual checks, performance and platform
limits. Original-game side-by-side comparison remains unavailable.
