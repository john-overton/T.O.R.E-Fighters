# Surface lighting and geometric shadows

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. Warm smooth lighting and geometric shadows, including
terrain, were requested by John on 2026-09-18. This is **opinionated** presentation.
All constants and rendering choices below are agent decisions, not recovered
original behavior. Original shadow and surface-light calibration remain unknown;
retail comparison is unavailable.

## Surface response

Smooth weather uses continuous diffuse lighting on all opaque world triangles:
terrain, aircraft, damaged aircraft, debris and rendered weapon bodies. Geometry
normals determine the response, without quantizing brightness to palette rows.
Original artwork and the changing weather palette remain the base color.
Aircraft retain their source facets. Smooth terrain lighting interpolates
area-weighted normals shared by coincident mesh vertices; triangle normals are
retained separately for correct geometric shadow-depth comparisons.

Sun direction and warmth follow the [sky's continuous arc and color](sun-glow.md).
The shared solar tint changes from linear RGB (1, 0.32, 0.075) below 6 degrees
to (1, 0.88, 0.65) above 25 degrees, using smoothstep of sine elevation.
John requested stronger shadows and more panel definition on 2026-09-18 after
reviewing a sunset capture. The following tuning is an agent decision.
Surface direct light blends 65 percent of the solar tint with white and adds
0.90 times the nonnegative normal/light dot product and visibility. Ambient
fill varies with the surface normal: downward faces use RGB (0.12, 0.14, 0.18),
upward faces (0.28, 0.32, 0.39), interpolated by (normal.y + 1) / 2.
Night uses 0.20 to 0.32 ambient by orientation and 0.30 moon diffuse.

John requested darker shaded land in early/late light on 2026-09-18, retaining
water and the current brightness of sunlit ridges. Terrain alone reduces its
ambient fill by up to 55 percent. The reduction is strongest at solar elevation
6 degrees and below, easing away by 25 degrees with the same sine-elevation
smoothstep as solar warmth. It applies to the unexposed portion:
`1 - smoothstep(0, 0.35, normal/light dot * light visibility)`. Fully exposed
slopes retain the preceding ambient and direct response; no extra ridge light
is added. The existing night/day blend phases out this change at night. These
constants are agent decisions. Aircraft fill, water lighting and fog remain
unchanged.


Daylight strength is the visible area fraction of the original solid sun disc,
including when its center is below the horizon. While partly visible, the light
and shadow direction use the centroid of its visible circular segment. This
keeps rays downward and shadows continuous through sunset. Without sun art,
the fitted angular radius is 0.5 degrees. The night/day ambient blend uses
smoothstep of center elevation from -6 to +2 degrees, independently of direct
sun visibility.

Opaque object panels additionally receive a broad view-dependent sky sheen:
strength 0.08, modulated by the reflected view's vertical direction and a
0.25 + 0.75 * (1 - normal/view dot)^3 grazing response. A restrained solar
highlight uses a normalized half-vector, exponent 32 and strength 0.22, gated
by direct visibility and the normal/light dot. Both follow panel normals and
preserve source colors and markings. Terrain uses diffuse response without
this painted-surface sheen. No new panel seams are invented in the artwork.

Dense weather bands attenuate sunlight along its path to the surface using the
same band classification as atmospheric occlusion, with a 600-foot transmission
scale. Fog reduces the surface-lighting contrast, then the existing aerial
perspective and dense-cloud occlusion apply. Emissive engine heat, flame sheets,
tracers, explosions, burning flares, chaff, smoke and vapor retain their own
presentation rather than receiving solid diffuse shading or casting solid shadows.
Glass receives lighting but does not cast an opaque silhouette.

Burning flares, and lit afterburner flames at an eighth of a flare's strength
([afterburner glow](engine-material.md#afterburner-glow)), add their own warm
point light to every surface this pass lights, and to water, clouds and
smoke, with no shadows. Its strength, reach,
night boost and the daylight colors it shows at night are in
[countermeasure presentation](countermeasures.md#flare-light).

## Geometry shadows

All opaque geometry submitted to the world renderer participates in the same
directional depth-map pass. Terrain casts and receives shadows; objects shadow
themselves, one another, terrain and visible water. Texture cutouts keep their
holes. The player's aircraft still casts a shadow when hidden by cockpit view.
Light direction follows the visible sun segment. Fully shadowed water retains
30 percent of its unshadowed resolved color at full sun, blended by visible-disc
strength, cloud transmission and fog. Moon shadows are not added.

Three camera-centered maps use half-widths of 256, 8,192 and 131,072 feet,
each 2,048 pixels square, with light-depth coverage of plus/minus 262,144 feet.
The innermost map containing a receiver supplies detail; its outer 15 percent
blends to the next map, and the outermost fades to unshadowed ambient/daylight.
John requested visibility-proportional shadow strength and contact-dependent
softness on 2026-09-18. One percent or 30 percent visible sun supplies one
percent or 30 percent of the current time-of-day shadow contrast; fully visible
sun supplies 100 percent, not black. Fully hidden sun supplies zero. This is
independent of the sunglare option, which affects optical glare only.

The circular-segment area supplies the horizon-visible fraction. Geometry
visibility additionally estimates the unblocked fraction of the sun at each
receiver, allowing a gradual boundary behind land. A blocker search measures
caster-to-receiver distance along the light. Penumbra radius is that distance
times the tangent of the apparent sun radius: aircraft close to a surface cast
sharp shadows; distant casters cast broader shadows. The agent-selected bounded
filter searches the center plus four directions at 1, 4, 16 and 64 texels, then
uses 32 fixed, evenly distributed disc samples with bilinear comparisons.
John requested less tile flicker and more diffuse near-view transitions on
2026-09-18. Agent-selected stabilization uses four manually weighted depth
neighbors per tap, each corrected to the receiving triangle's plane. A 0.125-foot
minimum comparison tolerance (or 0.002 shadow texels, whichever is larger)
eases occlusion over twice that tolerance. Blocker weights are continuous,
including their contribution to the inferred filter width. Occlusion and filter
width ease with blocker confidence from zero to one weighted blocker, making
the empty-search fast path continuous. A second map is sampled only inside
the cascade blend region. There is no abrupt
switch between contact and soft filtering.

The camera-distance antialias radius is 2 texels through 500 feet, easing with
smoothstep to 0.65 texels at 12,000 feet. This gives closer transitions a more
diffuse appearance and distant ones a sharper appearance. Physical caster
separation can still require a wider penumbra; total radius remains limited to
64 texels. The projection basis varies continuously throughout daylight,
without an axis switch near overhead sun. Sun direction uses the fractional
weather clock (256 source units per second, advanced by the fixed 120 Hz host),
shared with sky and glare, rather than jumping at whole seconds. No temporal
history or random jitter is used, avoiding motion trails and preserving
repeatable paused views. Receiver-plane depth correction prevents grazing
surfaces from creating their own false blockers. This is a finite-sample
approximation, not an exact solar-disc ray trace. Projections snap to shadow
texels to reduce crawling as the camera moves. A 0.20-texel grazing normal
offset supplements the continuous comparison tolerance described above. Terrain raster bias is constant 2 /
slope 3; object bias is constant 1 / slope 1 so nearby panels retain contact
shadows. These are finite-resolution compromises, not exact contact geometry.

These are finite-resolution geometric shadows, not ray tracing or recovered
retail shadow shapes. Small objects lose shadow detail at long distances;
very distant terrain outside the maps has no geometric shadow. Transparent
cloud volumes do not cast triangle silhouettes, but dense weather attenuates
sunlight. Cockpit/menu artwork is a separate overlay. Future opaque world
objects use this shared path when submitted as surface batches.

The stepped compatibility mode retains existing colors and palette lighting,
without the new diffuse response or shadow pass. Simulation and flight adapters
are unaffected. No new dependencies or retail-derived committed assets are needed.

See the [validation baseline](../baselines/aircraft-lighting.md).
