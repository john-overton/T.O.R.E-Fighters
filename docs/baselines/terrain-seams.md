# Temperate terrain tile audit and seam presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research audit followed by implementation, 2026-10-18, `exp/vegetation` at
`63faab1256e6ffe72b4e9ce5d92224a1e034d914`. Presentation specification:
[terrain detail](../spec/terrain-detail.md#tile-seams). Source contracts:
[theater](../formats/theater.md) and
[terrain materials](../formats/terrain-materials.md).

## Source comparison

The CPU audit reads the existing isolated import and the installation's
FA_1.LIB/FA_2.LIB directly. All 203 checked payloads are byte-identical. It covers
32 layouts: ten UKR, twelve FRA and ten BAL, including their base maps. All
1,514 `tdic` dictionaries across those layouts match recomputed pixel coverage.
The audit also checks the actual terrain upload mesh's page assignment and
quarter-turn UVs, and verifies that runtime T2 cells equal the selected source
T2. Generated campaign aliases use the existing documented base-grid fallback.

Input identity, SHA-256:

| Input | Hash |
| --- | --- |
| FA.EXE, 1.02F | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| FA_1.LIB | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| FA_2.LIB | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |

The base UKR layout has 697 `tmap` lines at 695 coordinates and 29 numbered
pictures. The two duplicate coordinates retain the existing last-entry rule;
retail duplicate precedence remains unknown. FRA has 1,957 placements and 47
pictures; BAL has 3,356 and 66. All four rotations occur. None of those numbered
pictures or LAND has an embedded palette. UKR uses DAY2, FRA DAY2F and BAL the
B palette family; one Baltic variant explicitly selects CLOUD1B. No reviewed
layout carries a separate per-tile shade or seasonal palette selector.

Source colours really differ. In the noon UKR palette, mean sRGB for UKR22 is
(40.62, 96.69, 64.90), UKR23 is (40.42, 96.80, 64.94), and LAND is
(45.39, 105.08, 77.27). City/coastal art spans a wider range. The map shows this
without GPU lighting, fog or vegetation. The existing fitted LAND projection
accounts for many boundaries between authored placements and uncovered cells.
This establishes correct source capture, not retail screenshot parity.

No importer, terrain-height, simulation-class or protocol fix was justified.
The old vegetation-only brown margin rule was an unmasked 1,147 ft fade toward
T2 colour; the new presentation replaces it. Whole-tile means alone also
produced false bands at already-matched artwork edges in the CPU reference
review, so the final correction uses local edge means.

## Local evidence and validation boundary

All retail-derived output stays under `.local/tmp-exp-vegetation/`:

- `seams-audit-final/`: archive hashes, layout/keyword tables, every cell's texture,
  page, rotation, palette, T2 colour/class/height, all numbered picture means,
  and full-theater unlit colour/mean PNGs for every reviewed variant.
  These material maps sample at 1,024 ft per pixel and use diagnostic blue
  for water.
- `seams-boundary-maps/`: final accepted edge masks, measured resource/cache data,
  CPU before/after strips and exact lead camera coordinates.
- `seams-lead.sh`: separate fresh Cargo targets and frozen before/after source,
  offscreen UKR/FRA/BAL captures and 4K High signed GPU timing pairs.

CPU strips are unlit references, not GPU captures. Synthetic tests exercise
water, roads, cities, rotated airports, field boundaries, matched artwork
edges, tree-belt falloff and the exact 500 ft gradient. Shader parsing and
binding validation run without an adapter. GPU appearance, distant filtering,
canopy handoff and measured millisecond cost require the lead's script. Retail
runtime comparison remains unavailable. No window, GPU run, commit or push is
part of this sandbox pass.

## Current seam validation

The GPU review of `939cbcfa` rejected the global statistical LAND replacement.
Evidence is retained in
`.local/next-round/renders/exp-vegetation/seams-texture-20261018-104138-659993/`.
Its images show existing mottling disappearing. The cause is replacement of
the complete LAND upload page, not a shader mask extending outside 500 ft.
The earlier contrast-gap measurement calibrated LAND toward the flatter named
tiles and did not require either source interior to retain contrast.

The reviewed camera's labels were reversed in that interpretation. At UKR
x=16,384, the south side (z<32,768, tile key 0,0) is fallback LAND.PIC, upload
page 28, rotation 0. The north side (tile key 0,4) is UKR23.PIC, page 0,
rotation 2. LAND contains the broad mottled art; UKR23 is predominantly flat
at this location. The actual mesh/UV audit and the source images establish
these labels independently of visual guesses.

Measurements from the supplied GPU surface-only 20,000 ft overview use fixed
interior crops, all outside the seam band. Luminance is decoded from sRGB to
linear RGB before measurement:

| Region and crop (width x height + x + y) | Original SD | Rejected SD |
| --- | --- | --- |
| LAND, 1950x500+850+1400 | 0.02314076 | 0.00513565 |
| UKR23, 1950x400+850+350 | 0.00465445 | 0.00465445 |

The 780,000-pixel UKR23 crop is byte-identical in both GPU captures, SHA-256
`7417be0e07020881b2377f9788cddd2ab39a5b942ba4a27648bab96a109dad21`.
The rejected implementation removed about 78% of LAND's luminance contrast.
Restoring the original page restores that lost artwork. Adding new mottling
to the flat UKR23 interior would conflict with the request to keep named-tile
interiors pixel-identical; that additional scope is awaiting clarification.

The correction now retains every source page and terrain vertex. The headless
UKR/FRA/BAL audit compares them against a scene built without this feature and
checks 665,600 / 665,600 / 1,048,576 interior positions for exactly zero response.
A dedicated synthetic case also scans all interior texel centres in two tiles.
Exclusions and the hard +/-250 ft cutoff are retained. The 500 ft reference
blend, actual baked GPU input and colour-only control are measured separately.

Shader work inside accepted bands is one mask lookup and one filtered response
sample. A sparse RGBA16Float atlas stores the linear RGB correction and source
luminance, sampled at 128 ft intervals along accepted runs and nine depths on
each side. The reference uses the original pages/rotations and the irregular
boundary. Original source pixels are never overwritten. The response is scaled
by live/reference luminance before lighting. This preserves uniform brightness
changes; nonuniform weather remaps are a fitted approximation.

The extra response storage is 16,515,072 bytes for UKR, 74,317,824 for FRA and
33,030,144 for BAL. It replaces per-fragment neighbour-art and palette work
with a single filtered read. It is uploaded once and cached. No per-frame
upload or ordinary draw is added. Array sizes remain within portable limits.
The supplied rejected GPU costs were 1.542 ms UKR and 2.293 ms FRA at 100 ft.
New GPU milliseconds are not measured in this sandbox.

The local `seams-lead.sh` builds unchanged `bcffd680` and the candidate with
separate fresh Cargo targets. It retains the existing views and adds dedicated
source interiors plus a mask-only capture. `mask-projection.tsv` derives the
500 ft width from the exact camera and terrain: at the UKR 5,000/10,000 ft
overheads the expected widths are 186.481 / 93.385 pixels at 4K.
`seams-gpu-check.py` measures contrast/spatial increments in GPU crops, rejects
changed original interiors or a wrong mask width, and enforces the 0.5 ms
100/2,000 ft median cost target. These are acceptance gates for the lead's run,
not claims of a new GPU result.

Current evidence and validation are recorded in the report's Tile seams /
Texture blend review section. No importer, simulation, terrain-height/class,
wire or replay data changes accompany this correction.
