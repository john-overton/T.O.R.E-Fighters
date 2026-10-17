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
on offscreen renders for passes 1 and 2. Pass 3 was reviewed by John; pass 4 awaits GPU visual and timing validation.
The new placement is not a claim about retail placement. Nothing is
collidable or part of the simulation, the network session, replays or
checkpoints.

### Sprites and cached placement

John requested sprites and caching, then denser ecological placement and a generated sprite option on 2026-10-16. The following
numbers are **opinionated agent decisions**, pending offscreen visual review.
The retail asset evidence and unresolved original distance rules are in
[objects and shapes](../formats/objects-and-shapes.md#vegetation-card-assets).

All trees, bushes, clumps, palms and rocks draw as upright camera-facing
impostors at all ranges, two triangles each. At scene startup a CPU rasterizer
bakes four horizontal views, 90 degrees apart, into indexed 256 by 256 pages.
This retains the imported PIC transparency and live weather palette. Runtime
art stays in memory. Nothing retail is embedded or written into the repository.
The optional generated v2 pack uses eight 256 px indexed atlas pages with four
128 px cells per page and transparent gutters. It is supplied by John, not retail
art. `TORE_VEGETATION_SPRITES=retail|pack|mixed` is read at scene construction;
this exploratory branch defaults to `mixed`. Mixed keeps retail conifers and
groves and uses the pack for the other available kinds. Pack mode uses large
single-tree silhouettes for groves. Dead trees and acacias keep the existing
fallbacks because v2 has no equivalent. Tropical broadleaf uses the jungle tree,
and desert rocks use dry rock variants. Saguaro and prickly pear are never
selected. See [generated art provenance](../../crates/tore-app/assets/vegetation/README.md).
Generated RGB is reduced to 78% before matching the theater palette; live weather
palette rows, local tint and scene lighting apply to both art sources.
Missing retail cutouts reject that source and use a fallback, never a solid
black polygon. Baked rock brightness is normalized against the visible sprite's
mean linear colour, including drawn fallback rocks. Rock cards use the ground
slope normal for lighting, avoiding dark silhouettes from the shared quad's
vertical normal and double multiplication of ground tint. The source meshes are not drawn each frame, including at close range. This
agent choice avoids a second geometry path; close views need visual review.

The source art is `TREEA` through `TREED`, `BUSH1` through `BUSH4`, `GROVA`
through `GROVD`, `GROVPA` through `GROVPC`, `PLMGA` through `PLMGC`, `ROCK`,
`ROCKA`, `ROCKB`, `ROCKC` and `ROCKE`, with the textures they name. Single palms
are cut from palm groves. Missing art, acacias and loose boulders use generated
low-poly sources, baked in the same startup pass. Individual fallback plants
use a few dozen triangles; fallback groves combine several plants. There are
no vegetation shadow passes. Grass remains its separate existing layer.

A 2,048 ft terrain patch caches full candidates independent of camera distance.
Only newly entering patches build; departing patches are evicted. Density changes
invalidate candidates. Detail distance changes select from the cache without
rebuilding it. GPU culling selects view, distance, density rank and pixel size.
One indirect instanced draw covers the indexed atlas. Sprites fade from three
pixels down to the 1.5 pixel rejection floor, and over the outer 12% of the
configured distance. Existing altitude fades and grass rings remain.

### Where things grow

Placement still reads the theater's own palette, ground slope and water map.
The following High settings scale candidate count with density:

- Trees use a 42 ft High candidate grid, bushes 44 ft and small rocks 52 ft.
  Woodland probability is 0.70 plus 0.25 times the local woodland share.
  Meadow retains a 0.001 background plus 0.045 times woodland proximity and
  patchy copses, with probability up to 0.18 in the existing 1,400 ft noise.
  Hills add up to 0.48 and relief up to 0.18, below the tree line. Slopes fade
  from 32 to 48 degrees to keep cliffs bare.
- Field interiors stay clear. Procedural 1,024 ft square plots, independent of
  retail tile boundaries, carry selected rows at 32 ft spacing. A boundary
  qualifies when both sides are field, or one is field and the other meadow.
  Three quarters of row plants are narrow 42 to 60 ft trees, one quarter bushes.
  A 96 ft mapped-road probe selects rows on a regular 64 ft candidate grid at
  probability 0.80. Village margins carry orchard-like clumps at probability
  0.55 in the next 350 ft outside the existing exclusion. These are fitted
  approximations, not recovered roads or orchard maps.
- Water, streets, town centers and runway exclusions remain clear. Every sprite
  is grounded through `tore_world::grounding`, with scale applied from the base.
- Desert open sand has no trees and retains 0.012 dry-bush probability on the
  44 ft grid. Near water, tree probability is 0.72 on sand or 0.80 on green
  terrain, times squared water proximity. Proximity is full within 300 ft and
  zero at 2,600 ft. The 512 ft water grid accepts one eighth wet coverage so
  narrow mapped delta branches survive. This also creates coastal belts and
  can overstate small water features; river identity is not geographically
  recovered. Wadi banks add 0.10 and beds 0.025 tree probability. Wet-bank
  bushes are green; remote shrubs remain brown and olive. Palms, acacias and
  broadleaf represent the belt; there is no dedicated reed or tamarisk asset.
- Tropical forest uses the 1.15 climate multiplier, capped at probability 1,
  taller broadleaf and palms near water and low coasts. Arctic uses 0.45.
  Tree-line full/zero heights are 4,500/7,000 ft for temperate broadleaf,
  3,800/6,000 ft for temperate conifers, 900/2,400 ft arctic, and 7,500/10,500 ft
  tropical. Bush cover increases near the tree line and fades to zero by
  700 ft above it. High cliffs are bare.
- Instance sizes use independent deterministic placement hashes. Two uniform
  samples make a triangular scale distribution: trees 0.6 to 1.4, shrubs and
  rocks 0.35 to 1.65, with most near 1.0. Woodland-edge trees multiply by 0.78,
  edge shrubs/rocks by 0.8, woodland-interior trees by 1.08 and wet-bank trees
  by 1.12. Width/height ratio varies independently from 0.88 to 1.12. Existing
  species base heights remain. These constants are agent choices implementing
  John's size-variation request of 2026-10-16.
- Temperate brown tile margins blend up to 85% toward continuous terrain vertex
  colour over 3.5% of a tile. Procedural plot colour and furrows fade across
  3.5% of a plot edge. Both shaded colour and flare albedo receive the same
  correction. This softens seams without changing terrain or simulation data.

`--vegetation-preview OUT overlay` writes classification pictures. These are
placement heuristics, not claims of original scenery or geographic river data.

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
  gravel. Open sand carries sparse dry scrub; dry grass stays on wadi
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
