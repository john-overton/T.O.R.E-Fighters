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
groves with large tree silhouettes. Dead trees and acacias retain their retail
or drawn fallback because v2 does not provide those species. Tropical broadleaf
uses the jungle tree; desert rocks use the dry rock cells. The saguaro, small
cactus and prickly pear are sliced for completeness but never selected. No
Old World theater receives them. Agave/yucca and dry scrub are the experimental
Old World desert choices requested by John, not a surveyed botanical claim.

Tests: `python3 -m unittest discover -s tools -p test_slice_vegetation.py`.
Pillow is needed for the synthetic slicer test, but not the atlas integrity test
or the game. No runtime dependency was added.
