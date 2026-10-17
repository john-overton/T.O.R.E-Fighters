# Generated vegetation experiment

Provenance: **generated sprite pack supplied by John, 2026-10-16**.
This is John's second, cleaner pixel-art pack, `john-sprite-pack-v2.png`.
Source SHA-256: `54bcb54d641c22b2ad8619dc9765ecdfabfef9f22e18934ba4ab45f4731d9941`.
The first pack is an unused local reference and is not shipped.
No retail art or retail derivatives are stored here.

`tools/slice_vegetation.py SOURCE OUTPUT` rebuilds these assets using Pillow.
The source remains in the local pass4-refs folder. Crop coordinates are recorded
in the tool; `sprites.json` records names, cells and original aspect ratios.
The tool also writes `pack_layout.rs`, which the runtime includes.
Black below RGB maximum 24 or source alpha below 128 becomes transparent.
Remaining pixels have binary opaque alpha, matching indexed cutouts. Generator red, green and yellow
chroma is removed with explicit thresholds, followed by a 3 px morphological
opening and removal of tiny disconnected debris. Natural greens and autumn
orange are retained. Individual PNGs are trimmed; four sprites occupy each
256 px atlas page, with two transparent pixels around each 124 px cell.
Nearest-neighbour resizing preserves the pixel style. Runtime restores each
sprite's original aspect ratio. The last page contains three sprites.

`pack.rgba` is eight consecutive 256 by 256 RGBA pages. It is embedded generated
art, not an extracted game resource. PNG pages and individual PNGs are review
and authoring artifacts. The app maps RGB at 78% brightness into the theater's
palette once on scene construction. Live palette/weather lighting, local tint,
fog and shadows are the same as for retail vegetation.

`TORE_VEGETATION_SPRITES=retail|pack|mixed` selects art at scene construction.
Mixed is this branch's default: retail conifers and groves, pack broadleaf,
palms, shrubs and rocks. Pack replaces all available equivalents, including
groves with large tree silhouettes. Sahara supplies dead trees and acacias. Every missing retail species now uses a
supplied cutout, including conifers; no code-drawn fallback remains. Tropical broadleaf
uses the jungle tree; desert rocks use the dry rock cells. The saguaro, small
cactus and prickly pear are sliced for completeness but never selected. No
Old World theater receives them. Agave/yucca and dry scrub are the experimental
Old World desert choices requested by John, not a surveyed botanical claim.

Tests: `python3 -m unittest discover -s tools -p test_slice_vegetation.py`.
Pillow is needed for the synthetic slicer test, but not the atlas integrity test
or the game. No runtime dependency was added.


Pass 5 adds **generated Sahara and top-view sprite sheets supplied by John,
2026-10-17**. Source identities:

- `john-sahara-sprites.png`, 1536 by 1024, SHA-256
  `7b0516ae7489678ede80835629a30232080bba9cf5bddf7d3cdd220976c84b73`.
- `john-topdown-tileset.png`, 2172 by 724, SHA-256
  `8fa71c2dcfd24beadc7c2d6c407a80e0a9b65dd7f8c3018779fb7a3ec3949ea5`.

`tools/slice_vegetation_pass5.py SAHARA TOP OUTPUT` rebuilds the `pass5/` folder.
It requires Pillow and numpy for authoring only, adding no game dependency.
The tool records every crop. Sahara segmentation fits a robust local cubic RGB
background to low-gradient pixels, then keeps foreground residuals connected to
strong cores and textured outlines. Smooth residual blobs are rejected, including
those beside trunks; small disconnected debris is removed. It does not assume a
black background or subtract glow colours from valid bark. The top sheet's black
matte and white/grey/black outline are removed separately. In a four-source-pixel
edge band, bright neutral pixels (minimum RGB above 155, spread below 38) and
dark neutral pixels (maximum below 100, spread below 16) become transparent.
A one-pixel erosion removes the remaining mixed matte edge. Bright stone faces
and dark bark away from that band are retained. This is a conservative authoring
choice; thin leaf tips may be lost. Side sheets are unchanged by this cleanup.
All output alpha is binary;
transparent RGB is zero. Conservative edge cleanup can lose faint leaf tips.

There are 21 Sahara side sprites, their 21 matching top views, and 31 v2 top
views. `pass5/pack.rgba` contains 19 RGBA pages, with the same 124 px cells and
2 px gutters as v2. Individual PNGs, page PNGs, `review.png`, JSON metadata and
`layout.rs` are review/rebuild artifacts. The Sahara palms, acacias, bare trees,
shrubs and rocks are selected for desert equivalents. Saguaro remains excluded.
Top modes use the corresponding supplied top cell; retail side silhouettes use
a fitted equivalent from that sheet. Local scale/width variation is unchanged.
The whole contact sheet was reviewed against a contrasting background and the
synthetic glow test passed. In-game GPU appearance still requires lead review.
