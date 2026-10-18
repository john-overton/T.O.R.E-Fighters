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

## Lead review and revised CPU results

The first GPU review, retained in
`.local/next-round/renders/exp-vegetation/seams-20261018-075956-525988/`, rejected
the initial result. Camera files wrote degrees into a radian reader. The
20,000 ft overview retained a LAND/PIC colour edge. The existing mask called
LAND's green fill Field and the authored tile Forest, so it selected only a
tree belt. The earlier CPU camera search examined accepted segments and missed
this excluded boundary. The GPU timing job used in-memory radians and its
cost was real: about 1 ms at 100 ft and 0.3 to 0.4 ms in several higher views.
Those are measurements of the initial shader, not the revised one.

Implementation-mode review fixes start at `3207123815bbf11d729531ae71f607bc66329852`.
The revised camera writer uses radians and checks the exact serialized camera
through the actual reader, projection and a terrain ray. All 51 capture/cost
cameras pass; maximum target-centre error is below 0.0001 pixel at 4K.

The pinned UKR boundary at x=16,384, z=32,768 joins LAND palette index 193,
sRGB (45,105,77), to UKR23 index 194, (40,97,65). Both base T2 cells have colour
212 and class 2. Treating the generic fallback shade as a crop field was the
wrong exclusion. Green fallback can now blend into natural authored ground
when those T2 values agree, while authored fields and all protected features
retain their exclusions.

The new numeric probe uses unlit bilinear source colour and the actual RG32Uint
upload bytes. Across -1/+1 ft the largest channel step falls from 12 to
0.071686 RGB levels. Samples every 5 ft match the 500 ft analytic smoothstep
within 0.000000015 in linear RGB; endpoints and samples outside +/-250 ft
remain unchanged. Source-derived CSV, strips and camera checks are in
`.local/tmp-exp-vegetation/seams-review-maps/`.

The packed shader reads one or two edge records instead of individual bytes,
and skips redundant palette resolutions. Constant edges resolve two palette
indices; interpolated varying mixtures can still require eight. The GPU input
uses 10,649,600 bytes for UKR/FRA and 16,777,216 for BAL, replacing the old
shared-art seam pages. It uploads once and adds no draw call. Revised GPU
appearance and milliseconds remain unmeasured in this sandbox. The updated
lead script measures both implementations from the same verified cameras.

Validation and current guard context are recorded in the session report's
Tile seams / Review fixes section. No simulation, imported terrain, wire or
recording format changes accompany these presentation fixes.
