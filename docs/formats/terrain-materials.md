# Retail terrain materials and coverage

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and implementation contract, 2026-09-23. This supplements the [T2 contract](theater.md).
[Input identities and measured census](../baselines/retail-terrain-review.md)
identify the executable and archives used. Addresses below apply only to that
FA.EXE. All executable inspection was static. No retail module was executed.
[Player-visible requirements](../spec/terrain-detail.md) and
[delivery and remaining scope](../ROADMAP.md#retail-terrain-detail-review) are separate.

## Named texture placements

The retail parser handles both `tmap col row index rotation` and
`tmap_named name col row`. The latter is recognized at `0x482fc1`; the bounded
name is read at `0x483019`, the picture extension is applied at `0x48303a`, and
the two coordinates are read at `0x483055` and `0x483061`. The placement's
rotation word is initialized to zero at `0x483079`. The named texture shares
the placement table used by numbered tiles.

`KURILE.MM` and `~KURILE.MM` each contain 236 named placements, rather than
numbered `tmap` lines. Every referenced PIC is present in FA_1.LIB. There are
144 images of 128 x 128 and 92 of 256 x 256; all use the weather palette.
The shared placement geometry covers four T2 cells on each side. A lower
resolution image therefore covers the same ground, rather than half the area.

`Environment` now retains explicit PIC references from named placements. The
shared scene dependency walk follows them and numbered references for every
selected layout. The host supports both image dimensions, replicating each
128-square source texel exactly into a 2 x 2 storage block. All source indices
and their four-cell footprint are retained.

## Texture dictionaries

Each `tdic` contains a size followed by two 4 x 4 byte grids. The parser at
`0x483101` through `0x483175` reads that scalar and 32 entries. The texture
scanner at `0x4aa6c7` through `0x4aa771` establishes their interpretation:

- The scalar is the image width.
- The first grid starts at one and is cleared by any index-255 texel in its
  subcell: one means wholly opaque land.
- The second starts at one and is cleared by any non-255 texel: one means
  wholly cutout/water.
- A mixed shore subcell has zero in both grids.
- Image rows are reversed into the dictionary grid's north/south order.

All 1,078 dictionaries in the sixteen base layouts match these rules exactly
when recomputed from the corresponding PIC rasters. Kurile's dictionary sizes
also match its mixed image dimensions in named-placement order. These are
coverage summaries, not elevation, roughness, vegetation density, or biome
maps. The source assets preserve them. The host uses the source pixel cutouts directly,
rather than reconstructing the original coverage cache or scanning order.

## LAND resources and boundaries of the evidence

FA_1.LIB contains `LAND.PIC`, `VLAND.PIC`, and `V_LAND.PIC`, each 256-square,
palette indexed, with no index-255 cutouts in this copy. They are distinct assets.

At `0x4c5ea0` through `0x4c5f29`, the terrain loader skips leading tildes in
the map name, tries its first character followed by `land.PIC`, and falls back
to `land.PIC` if that resource is unavailable. This selects `VLAND.PIC` for
Vladivostok and `LAND.PIC` for the other base names in the supplied archives.
The resulting name is used by a draw branch at `0x4ab399`. A separate branch
at `0x460219` selects `v_land.PIC`. Do not conflate the underscored resource
with the theater fallback.

Unknown: the complete visibility gates, projection, tiling scale and interaction
of these land-plane paths with the raised terrain. This review does not establish
that either image should simply repeat on every untextured triangle. Next:
trace the inputs to `0x4ab366` and the plane-draw parameters at `0x447aa5`,
then write the observable placement and scale rule, or label an authored rule
`fitted`. No whole-renderer reconstruction is required.

`GRNDLRG`, `GRNDLRG2`, `GRNDMED`, `GRNDMED3`, and `GRNDSML` PICs are
multi-frame explosion/impact artwork, confirmed by decoding and inspecting the
rasters. Their names occur in a resource preload list at `0x4fb930` through
`0x4fb944`, loaded by `0x486010`. This is not a terrain-material mapping.

## Remaining source questions

T2 class bytes are preserved but their complete visual meaning is unresolved.
Do not assign grass, rock, sand or snow from class numbers without consumer
evidence. Heights and coastline art alone also do not establish a snowline.

The [shape guide](objects-and-shapes.md) and
[airport placement contract](airport-placements.md) remain authoritative for
object scale and references. Main-shape projection success is not proof of
complete detail levels, damage states, decals, lines or conditional geometry.
Campaign layout composition and generated terrain names remain unresolved;
neither a shared filename prefix nor a similar picture establishes replacement
semantics.

## Runtime layout and image storage

The app and extractor follow all selected layouts through the same dependency
walker. The app cache marker `TORE_TERRAIN_V2` requires a re-import of older
bundles so named artwork, variant scenery and newly projected shapes cannot be
silently absent. No image content is embedded in the executable.

Generated map references are bounded by `base_theater`: the sixteen base codes,
reviewed `~...F` layouts, the supplied numbered campaign names and the Kurile/
Vietnam aliases. Original MM/T2 identity remains intact. A variant uses its
own scenery list and its literal grid when available, otherwise its reviewed
base grid. This static selection rule is **fitted**, not campaign progression.

World artwork uses 256-square logical pages packed 4 x 4 into 1024-square GPU
layers, with at most 4,096 logical pages. Aircraft and smoke retain their own
rectangular image dimensions. Material-local metadata distinguishes the two
storage paths; palette remaps and shadow cutouts use the same logical indexing.
Large scenery pictures are split into pages without resampling. Faces crossing
a page boundary are clipped in UV space while interpolating world positions.
New edge vertices keep the nearer endpoint's discrete palette/fog attribute;
this is a fitted detail for the uncommon color-varying face. No new material
noise, lighting model or terrain displacement is part of this storage change.
