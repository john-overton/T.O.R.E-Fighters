# Ground textures and map detail

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-23. John selected retail textures, artwork, scenery
and map variants for all maps. Shaders and expanded landscapes are deferred.
Source detail and remaining fitted presentation rules are specified below;
retail visual comparison remains unavailable.
[Evidence](../baselines/retail-terrain-review.md),
[source contracts](../formats/terrain-materials.md), and
[delivery plan](../ROADMAP.md#retail-terrain-detail-review).

## Source landscape

The player flies over the original theater's hills, mountains, plains, islands
and coastline. The source terrain samples are 8,192 feet apart horizontally;
one height unit is 256 feet. The reviewed base maps have 200 to 256 samples
per axis and maximum sampled elevations from 4,096 to 7,936 feet. The
[theater census](../baselines/retail-terrain-review.md#base-theater-census)
gives the individual maps. These are source sample limits, not a claim about
the exact silhouette produced by retail's terrain interpolation.

Authored texture placements cover 32,768 feet per side and retain their source
location and quarter-turn orientation. A 256-square tile provides 128 feet
per texel; a 128-square tile provides 256 feet per texel at the same footprint.
Retain the original indexed artwork and resolve it through the selected
theater's live weather palette. A painted river, road or field is already
landscape detail and must not be discarded because it is not a separate object.

Kurile has 236 explicitly named texture placements, including coastline and
interior land artwork. They are part of its landscape even though the previous
host displayed only its cell colors. Both source image sizes use the same
placement footprint. Index 255 exposes water according to the existing
[shoreline behavior](terrain-shorelines.md). A land patch can sit on a cell
whose base color is water.

Provenance: source spacing, heights, placement geometry, dimensions and texture
identity are **native data**, to be implemented as **spec-derived** behavior.
The current fixed triangle surface, shared lighting normals, filtering and
cutout threshold are **fitted** or **opinionated** presentation, as recorded in
[surface lighting](surface-lighting.md) and [graphics options](graphics-options.md).
Changing them must not silently change the runway or contact surface.

## Placed scenery

The manual identifies permanent runways, roads, buildings and bridges on maps,
and lists rocks, crop fields and urban/industrial scenery in its object catalog.
The [airport specification](airports.md) applies to their source identity,
position, orientation and independent object behavior. Hills are not replaced
with enlarged rock objects. Reuse explicit source shape and picture references.

Availability in the catalog does not imply presence everywhere. A road can be
painted into a tile, a placed object, or mission-specific scenery. Do not scatter
extra rocks, trees or towns as if they were recovered placements. No autonomous
movement, targeting or ground-defense behavior is added by this visual review.

## Implemented source coverage

The host imports and offers all 75 reviewed MM layouts: sixteen base maps and
59 variants. Every placement with a main shape in those layouts projects into
the static scene. No-body controller records remain in the manifest. Referenced
scenery images retain their full dimensions and pixels through paged storage;
shape line records render as one-pixel strokes. Original flight adapters and
120 Hz simulation remain unchanged.

CHAP and SA2 use a **fitted static loaded pose**. Their reviewed shape records
provide the actual launcher and missile geometry; the host selects it without
calling the retail load-count callback. This is visual scenery only. Scenery
animation, live weapon inventory and autonomous operation are not added.

The existing destruction rule still removes a destroyed body. Complete original
damage-state appearance, distance-dependent model selection and unresolved
runtime decal slots are not claimed by this static scenery expansion.

## Unknown behavior and next evidence

| Missing rule | Next bounded research step |
| --- | --- |
| Generic land texture projection, repeat scale and visibility | Review the identified land-plane callers, then specify player-visible scale or an explicitly fitted choice |
| Meaning of T2 classes for land appearance | Follow the class consumers; do not infer biomes from the class numbers |
| Retail hill interpolation and distance detail transitions | Review representative steep cells and the terrain builder; record silhouette/transition behavior rather than its control flow |
| Full scenery shape detail and destroyed appearance | Compare reviewed shape branches with the current projection, then follow explicit state and damage references |
| Live campaign landscape changes | Static variants use the explicit fitted selection rule below; recover progression and destruction persistence separately |
| Trees implied by two empty catalog definitions | Find explicit usable shape resources and placement evidence; the referenced SH files are absent from the two reviewed gameplay archives |

## Host presentation where the source rule is incomplete

The following are **fitted agent choices**, not recovered original rules:

- Bare non-water terrain uses the selected LAND/VLAND artwork over four cells,
  32,768 feet per repeat, matching the existing authored-patch footprint. Source
  heights, mask holes and live theater weather colors remain authoritative.
  The original land-plane projection/scale remains a research question. This
  supplies retail artwork, not added procedural material detail.
- A selected retail MM variant supplies the complete scenery list for that
  view. It is not appended to the base list: absent placements remain absent.
  The exact MM name remains visible and source aliases remain intact. Reviewed
  generated T2 names select their theater's base grid when no literal resource
  exists. This is a static layout presentation, not a campaign-state engine.
  Mission-dependent destruction and campaign progression are outside it.
- Imported image pixels remain intact. GPU paging and exact nearest replication
  of 128-square terrain art into 256-square storage are host storage choices.
  Neither operation adds detail or downsamples large scenery textures.

## Future enhancements

Optional surface shaders and expanded landscape geometry/vegetation are
[deferred roadmap items](../ROADMAP.md#future-terrain-enhancements), as John
requested on 2026-09-23. No new shader material noise, snowline, terrain
movement or invented scenery is included in this implementation.
