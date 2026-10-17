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

Weather choices retain the map's retail palette family. Egypt, Iraq, Persian
Gulf and Spain use the sand family by day and at night; North Vietnam uses its
MM's unsuffixed family. All sixteen families are listed in the
[weather contract](../formats/weather.md#theater-palette-families).
This applies to textured and bare ground, distance filtering, daylight colours
under night lights, city classification, and maps that use terrain colours as a
fallback. Briefing pictures keep their own embedded palettes. Provenance:
**spec-derived**, from the explicit retail MM layer references.

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
movement or invented scenery is included in this implementation. The
`exp/vegetation` branch is an exploratory exception (John, 2026-10-16):
close-range ground detail and non-collidable vegetation, set from the
Graphics screen
([vegetation](graphics-options.md#vegetation-texture-and-vegetation-objects)).
It is not part of the terrain-detail behaviour above.

## Exploratory vegetation and ground detail

`exp/vegetation` only. Everything in this section is **opinionated**:
John asked for vegetation on 2026-10-16 and for the second pass the
same day (retail trees, placement from the tile art, denser grass, a
better desert); the rules and numbers are agent choices, fitted by eye
on offscreen renders. The original game has none of it. Nothing is
collidable or part of the simulation, the network session, replays or
checkpoints.

### Shapes

Trees, bushes, tree clumps and palms are the retail shapes and
textures from `FA_2.LIB`, which no mission or layout places: `TREEA`
(broadleaf), `TREEB` (autumn), `TREEC` (dead), `TREED` (conifer),
`BUSH1` to `BUSH3` (dry bushes), `BUSH4` (green bush), `GROVA` to
`GROVD` and `GROVPA` to `GROVPC` (broadleaf and pine clumps), and
`PLMGA` to `PLMGC` (palm groves; single palms are cut out of them). The
import keeps them as `VEGETATION_ART`, apart from the theater
resources. Rocks are the retail `ROCK` and `ROCKA` to `ROCKE` shapes,
sunk a little into the ground and tinted toward the tile under them.
Acacias and loose boulders have no retail shape and are drawn
fallbacks, as is anything an older import lacks.

### Where things grow

Placement reads the tile art under each spot through the theater's
reference palette, with the ground's slope and height. A theater's
own greens are split by tone, so the split follows that theater's
palette rather than fixed colours:

- **Woodland** (its darker greens): dense trees, broadleaf or conifer
  by climate, with tree clumps standing in at distance.
- **Meadow** (the middle greens): grass with copses where a slow noise
  field says so.
- **Fields** (the lighter yellow-greens): clear, with hedgerows and
  the odd tree along the edges of 1,024 ft plots.
- **Hills**: more trees with slope (from about 3 to 13 degrees) and
  with height above the theater's median, up to a climate tree line.
- **Water**: river banks and shores are denser. A 512 ft block counts
  as water only when at least half its texels are water, so a blue
  roof is not a river.
- **Clear ground**: cities, streets, runways (plus about 220 ft) and
  water.
- **Deserts**: sand stays bare. Palm groves and green belts follow
  rivers and oases; dry bushes and a few trees line wadi banks;
  outcrops ringed by boulders stand on slopes and stony ground.

`--vegetation-preview OUT overlay` writes top-down classification
pictures for Ukraine and Egypt.

### Grass

A carpet of grass tufts, each five tapered blades, on meadow, field
edges, woodland floor and desert gravel (dry straw there). Its radius
and density follow the
[density setting](graphics-options.md#vegetation-texture-and-vegetation-objects).

### Desert ground

The desert texture is meant to read as sand at every height, not as
one repeating ripple:

- **Colour fields**, miles across: ochre, red and pale sands, grey
  gravel plains (serir), dark varnish on stony ground, pale silt in
  meandering wadi beds about 150 ft wide with darker banks.
- **Dunes**: in sand seas a few miles across, transverse ridges about
  1,150 and 1,750 ft apart and up to about 80 ft high, with a long lit
  windward slope, a sharp crest and a shaded slip face. Barchan
  crescents, a few hundred feet across, sit on the sea's edges. They
  are shading on the lighting, not geometry.
- **Close range**: wind streaks along the wind, and ripples 24 to 40
  ft apart in three directions, only on sand sheets and windward
  flanks, soft enough that the dune's own light still shows. Slip
  faces stay smooth. Soft grit, and pebbles about a foot across on
  gravel. Open sand grows nothing; dry tufts and bushes stay on wadi
  banks, gravel and water.

Each pattern fades out before it is smaller than a pixel. The fine
grain is full below about 2,200 ft and gone by about 7,500 ft; the
colour fields and dunes reach two to six times the texture distance
and fade out between about 9,000 and 26,000 ft.

### Lights

Flares, afterburners, fires and gun flashes add light to the ground
detail and never remove it. Measured on the Ukraine chase view (the
mean difference of each pixel from its 3 by 3 neighbourhood, lower half
of the picture), the ground detail is 1.001 times as strong with an
afterburner's light at 300 ft and 1.049 times at 40 ft, where the light
reaches the ground.
