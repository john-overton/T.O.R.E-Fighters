# Sunrise and sunset glow

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. John requested a more natural glow around the rising and
setting sun on 2026-09-16. This presentation is **opinionated**; the constants
below are agent decisions, not measured original behavior.

The smooth weather mode adds a directional warm glow around the existing sun.
The stepped compatibility mode retains its current appearance. Original sun
position, apparent size, weather artwork and simulation timing remain intact.

The halo uses angular distance from the sun, so it follows camera rotation,
bank and zoom. A narrow aureole has a 7-degree Gaussian width, a wider shoulder
has an 18-degree width. Dawn/dusk also has a horizon wash with a 45-degree
azimuth width and a 14-degree elevation width centered on the sun. The opposite
hemisphere gets zero added light. The wash is strongest below 6 degrees solar
elevation and fades away by 25 degrees. A smaller aureole remains at noon.

Linear-light RGB emission is (1, 0.32, 0.075) at low sun, blending to
(1, 0.88, 0.65) by 25 degrees. Peak narrow/wide/wash strengths are 0.85,
0.24 and 0.16; daytime narrow/wide strengths are 0.30 and 0.06. Emission
approaches display white exponentially instead of clipping into a flat plate.
The existing core receives the same light. Glow fades over the first and last
120 seconds of the source sun interval to avoid a switch-on flash.

The atmospheric glow is restricted to visible sky, behind opaque clouds, terrain
and the cockpit. Clouds receive the separate lighting response specified below. It does not brighten water or terrain, change lens flare or add a
full-screen exposure effect. The source sky deck remains visible through it.

Reference: the ignored USNF-ATF `Docs/environment-plan.md` sky section describes
a directional scattering sky; `Docs/progress.md` records its highlight rolloff
and visible disc problem. These inform the visual intent only. This change does
not reuse that engine or claim a physical atmosphere model. Pre-sunrise and
post-sunset scattering outside the source sun interval remain unimplemented.

## Angular cloud lighting

John requested angular cloud-texture glow on 2026-09-16. This is an additional
**opinionated** presentation component with agent-selected constants.

Each cloud pixel uses the normalized camera-to-world-position direction, not
its texture coordinates or the cloud center. Its angle to the sun controls a
35-degree Gaussian lighting gradient. A smooth taper at direction dot products
0..0.2 makes added light zero throughout the opposite hemisphere. The same
calculation lights cloud detail painted into the sky deck, using that pixel's
ray/plane direction. Tile boundaries do not restart the lighting gradient.

Lighting rises with solar elevation using smoothstep in sine elevation from
-5 to +2 degrees. Its strength is 0.90 through 6 degrees, easing to zero at
25 degrees. The sun interval's existing 120-second fade also applies. This
makes cloud lighting emerge at sunrise and retreat at sunset, without adding
noon lighting. Color uses the atmospheric glow's warm-to-pale RGB transition.

Added emission is proportional to the original resolved texture RGB, so black
stays black and brighter cloud details catch more light. The same exponential
highlight rolloff bounds the result. For finite cloud sheets, distance haze attenuates emission by
`1 - clamp(haze, 0, 1)`. The sky backdrop receives full strength, since its
arbitrary plane distance is not cloud depth. Cutout alpha is unchanged. Sky-deck lighting is applied
before the existing atmospheric halo; separate opaque cloud sheets receive
cloud light only, and still hide the halo behind them.

This approximates directional scattering with flat original art. There is no
inferred cloud density, surface-normal map, shadowing between clouds or physical
multiple scattering. Lighting is camera-direction dependent by design, not a
claim about illumination of three-dimensional cloud volumes. Stepped mode,
night, absent sun and the opposite hemisphere receive no added cloud light.
