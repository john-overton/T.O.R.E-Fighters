# Original tintable vegetation

Implementation mode. John requested replacement art on 2026-10-21.
The artwork and material fitting are agent decisions. This is not a claim of
original retail silhouettes, scale, palette or placement parity.

All 54 individual plants and rocks were generated with the built-in image_gen
tool from text only. No retail picture, retail derivative, previous sprite pack,
or image reference was supplied. Every exact prompt is in `prompts.json`.
Each generation contains one side and one matching vertical overhead view.
`sprites.json` records the source PNG SHA-256, measured proportions and cells.
Full generated PNGs remain in the task scratch directory, not the runtime pack.

There are three variants each of broadleaf, dead tree, conifer, palm, acacia,
birch, jungle tree, green bush, dry bush, fern, banana, yucca, dry scrub, stone,
boulder, outcrop, desert rock and tropical shrub. Autumn reuses the broadleaf
and birch forms with autumn tint. Groves select individual trees. Every selected
species has at least three variants, including matching conifer tops.

`python3 tools/pack_vegetation_materials.py GENERATED OUTPUT` rebuilds the pack
using Pillow, an authoring dependency only. GENERATED holds `KIND-1.png` through
`KIND-3.png` for the 18 kinds listed in the tool. The generated alpha is the only
background key. Alpha below 128 is cut out. Each half is trimmed independently,
nearest-neighbour sampled to 80 by 80, then to a 124 by 124 usable cell inside a
two-pixel transparent gutter. Measured aspect is restored in world geometry.
No tracing, retail palette copying or image-derived generation is involved.

The packer audits both final sampled views of every pair before material encoding.
Its conservative defringe rule is a **fitted agent choice**: an opaque texel must
touch transparency in its eight-neighbour ring, have every RGB channel at least
224 with a channel spread at most 24, and have a same-material opaque neighbour
whose maximum channel is at least 64 below its shading luminance. It takes the
median-by-RGB-sum shade of those darker neighbours. Alpha, bounds, aspect and
material remain unchanged. This targets isolated white matte; broad baked
backgrounds still need source review. Warm wood highlights, ordinary stone
highlights and bright interiors are retained. Add `--audit REPORT.json` to the
packer command to record bright-edge and white-matte counts for all 108 views,
before and after repair. Bright-edge counts alone are not proof of a fringe.

The generation prompts use neutral grayscale foliage/stone/straw and warm
orange bark as material identification colours. The packer classifies bark when
red exceeds 1.2 times green and red minus blue exceeds 16. Dead wood is always
bark, named rock types always stone, and dry scrub always straw. Remaining
pixels are foliage. Bark shade uses maximum RGB, other materials average RGB;
the normalized value is rounded to one of 32 levels.

`pack.materials` contains 27 consecutive 256 by 256 one-byte pages, 1,769,472
bytes. Each byte is `material * 32 + shade`: 0 foliage, 1 bark, 2 stone, 3 straw.
255 is transparent. Transparent RGB does not exist in this encoding. The shader
uses `0.25 + 1.25 * shade / 31` as a linear colour multiplier. `pack_layout.rs`
is generated metadata included by Rust. Runtime needs no image decoder or
retail shape extraction for this pack.

At a partially covered silhouette edge, the shader never decodes 255 as a
material. If the nearest texel is transparent, colour comes from the opaque
texel with the greatest bilinear weight in the same two-by-two footprint.
Fully opaque nearest samples keep their authored colour. Coverage still uses
the existing bilinear alpha and multisample rule, including depth and shadows.
This fixes the white dead-tree fringe without altering any shipped sprite bytes.

The per-instance colour input supplies local foliage or stone/straw tint;
the mesh input supplies independent theater bark. `veg_material` is the shader
material input hook. A future weather system may provide new linear RGB colours
for these materials, a wetness multiplier (1 dry, smaller values darker), and
snow coverage (0 none, 1 fully snow-coloured). Those future inputs are a design
contract only; no wetness or snow behaviour is implemented here. Current
climate/tile fitting and `TORE_VEG_TINT=theater|spring|summer|autumn|winter-dry|desert`
are specified once in the
[terrain specification](../../../../docs/spec/terrain-detail.md#sprites-and-cached-placement).
The debug override is process-wide; restart the mission process to change it.

The old retail/mixed/pack selector and retail top PNG overrides are retired.
Only `TORE_VEGETATION_SPRITES=original` is accepted when explicitly set. Existing
profile `vegetation-overrides` files are ignored and left untouched. The old
John-supplied pack is archived locally, with no old image deleted by this task.
`--vegetation-art OUT` exports the new pairs without loading any retail art.

Validation: `python3 -m unittest discover -s tools -p test_slice_vegetation.py`
and the Rust vegetation tests check IDs, shading, alpha, gutters, proportions,
three variants per species and valid top cards in every climate. Use the task
report for contact sheets, GPU captures, timing and visual limitations.
The `vegetation-material-fringe` battery case runs a synthetic GPU regression
against the production shader, including all four materials and empty cutouts.
