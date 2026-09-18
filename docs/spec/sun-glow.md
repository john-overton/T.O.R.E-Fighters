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
The stepped compatibility mode retains its current appearance. Weather artwork and simulation timing remain intact. Apparent size follows
the shared scale described below.
Smooth-mode sun positioning follows the continuous presentation arc below.

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
The existing core receives the same light. Glow follows solar elevation, fading smoothly from full strength at the
horizon to zero at 6 degrees below it, symmetrically at sunrise and sunset.

The atmospheric glow is restricted to visible sky, behind opaque clouds, terrain
and the cockpit. Clouds receive the separate lighting response specified below.
Opaque surfaces now share its solar tint through the [surface lighting pass](surface-lighting.md).
The atmospheric glow itself does not brighten water or terrain, change lens flare or add a
full-screen exposure effect. The source sky deck remains visible through it.

Reference: the ignored USNF-ATF `Docs/environment-plan.md` sky section describes
a directional scattering sky; `Docs/progress.md` records its highlight rolloff
and visible disc problem. These inform the visual intent only. This change does
not reuse that engine or claim a physical atmosphere model. Pre-sunrise and post-sunset scattering now continue into twilight.

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
25 degrees. The same elevation-based twilight fade also applies. This
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

## Continuous sun and twilight

Implementation mode, requested on 2026-09-16. Smooth presentation uses an
opinionated full-day arc. Source sunrise and sunset times are horizon crossings,
not drawing switches. Across the daylight interval phase advances 0..pi; across
the remaining night it advances pi..2pi. Elevation is asin(sin(phase)). The
source morning/evening azimuths are selected on either side of zenith/nadir,
where horizontal direction vanishes, keeping the direction continuous.
The arc ignores the source sun-enable flag in smooth mode, leaving cloud/fog
occlusion to the renderer. The smooth sun, glare and shadow direction share fractional weather-clock time,
including updates within each second. The stepped path and simulation lighting stay intact.

For DAY2, sunrise is 07:00, sunset 19:00, and the unchanged quick-mission Sunset
preset is 19:01. Its center is then about 0.25 degrees below horizontal; the
visible portion of the original disc/rings and the warm twilight wash remain.
Sky pixels below the flat world's horizontal horizon hide the sun; terrain and
dense weather can also cover it. No Earth-curvature or refraction model is added.
The original art's apparent sun size remains an approximation. Twilight glow
uses smoothstep in sine elevation between -6 and 0 degrees and has the same
angular/color gradients at both ends of the day. Sun position is visual only;
this does not alter gameplay lighting tables or the mission's clock.

## Apparent sun and moon size

John requested half the previous apparent size on 2026-09-16. Both sun-circle
radii and moon-billboard dimensions now use a shared scale of 2 instead of 4.
This halves projected diameter/width/height at a fixed camera and zoom, including
in stepped mode. It does not halve the broader atmospheric glow or lens-flare
sprites. The source art remains unchanged. This is opinionated presentation,
not a claim of calibrated astronomical or retail size.

The trial orange-rim disc grading is removed at John's request. The sun uses
its preceding color treatment, retaining the earlier atmospheric glow and
continuous visual arc at the smaller size.

## Horizon glare

John requested gentler sunset glare with a rapid drop below the horizon on
2026-09-16. Smooth-mode lens flare and palette whiteout follow the continuous
visual sun direction. Their strength is 35 percent at the horizon and through
3 degrees elevation, easing to full daytime strength by 15 degrees. Below the
horizon it drops with smoothstep to zero at -0.5 degrees (about two minutes on
DAY2). Existing view-alignment response, glare toggle and stepped mode remain.
The existing roughly one-second whiteout smoothing uses the visual target;
lens-flare strength follows elevation directly. Dense source cloud/fog bands
between the camera and the sun suppress glare, using the 600-foot visibility
limit. The twilight sky glow is independent and can remain after glare vanishes. These strengths are authored tuning.

## Continuous lens-flare composition

John requested removing the sky-gradient artifacts inside flare circles on
2026-09-18. Smooth mode retains original circle positions, radii and the existing
glare-strength envelope, but uses opinionated continuous optical emission.
Two fills use linear RGB (1, 0.55, 0.22) at strength 0.12 and (1, 0.32, 0.18)
at strength 0.08. These colors and strengths are agent decisions. Coverage
feathers over the outer 1.5 pixels. Overlapping circles add emission; the result
is `background + (1 - background) * (1 - exp(-emission))`, with alpha preserved.
The underlying sky remains continuous through each circle, without palette
lookup boundaries or wedges. Stepped mode retains original palette remapping.

See the [gradient-composition GPU check](../baselines/aircraft-lighting.md).
