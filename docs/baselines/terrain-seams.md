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

The initial GPU review found degree-valued cameras and an excluded LAND edge.
The lead's rerun of the fixes at `bcffd680`, retained under
`.local/next-round/renders/exp-vegetation/seams-review-20261018-085122-572570/`,
confirmed centred cameras and the working colour ramp. Its comparison images
still show a texture-pattern boundary. The current implementation adds
statistical LAND synthesis and a texture residual cross-fade, retaining the
500 ft mean-colour ramp. These are opinionated presentation changes, not an
importer correction or a retail parity claim.

CPU material evidence is in `.local/tmp-exp-vegetation/seams-texture-proof/`.
It evaluates the actual RG32Uint upload records and indexed texture pages.
Synthetic tests additionally cover all four neighbour rotations with reversed
page order, source-class exclusions, unchanged outer endpoints and the exact
500 ft colour ramp. All 65 serialized capture/cost cameras pass the real reader,
projection and terrain ray; maximum centre error is below 0.0001 pixel at 4K.

The LAND fit uses only natural green pixels in distinct source pictures that
adjoin accepted fallback edges. It records mean RGB, luminance standard
deviation, dark/light patch colours, and RMS luminance increments at 256,
512, 1,024, 2,048, 4,096 and 8,192 ft. Histograms and these statistics determine
a synthesized page; no source spatial layout is copied. The source LAND has
30 neutral flecks among 65,536 pixels, so the generic-fill gate requires at
least 99% green and no transparent pixels. Protected-feature masks still use
the original source art. Per-theater results:

| Theater | Distinct source pages | Noise cells / small octave share | Largest spatial-statistic error | Luminance contrast error |
| --- | --- | --- | --- | --- |
| UKR | 6 | 4,096 ft / 0.5 | 18.083% | 0.008% |
| FRA | 4 | 2,048 ft / 0.25 | 22.544% | below 0.001% |
| BAL | 16 | 1,024 ft / 0.5 | 8.144% | below 0.001% |

Mean and dark/light RGB also match within 0.01%. The headless audit requires
spatial errors within 25% and the other statistics within 1%. The finite
32,768 ft procedural repetition and the use of one fit per theater remain
approximations requiring visual review.

The texture-band probe samples parallel strips one foot either side of a
known accepted seam. Its colour-only control uses the synthesized LAND page
and the retained mean ramp, isolating the extra texture contribution:

| Theater | Colour-only RMS discontinuity | Texture-blended RMS discontinuity | Colour-only contrast gap | Texture-blended contrast gap |
| --- | --- | --- | --- | --- |
| UKR | 0.000802306 | 0.000011960 | 39.75% | 1.22% |
| FRA | 0.001987366 | 0.000024485 | 30.97% | 0.16% |
| BAL | 0.002623923 | 0.000084551 | 16.66% | 0.52% |

RMS values are linear luminance. Contrast is strip luminance standard deviation;
the gap is the absolute difference divided by the larger value. UKR/FRA have
512 paired positions, BAL has 189 after feature exclusions. The CSVs also cover
-250, -125, -32, +32, +125 and +250 ft; outer endpoints remain unchanged.
The mean-ramp probe chooses a nonzero sample on the reviewed UKR edge because
synthesis can already match the exact original camera-centre colour.

GPU input uses 10,682,368 bytes for UKR/FRA and 16,809,984 for BAL, an extra
32 KiB over the colour-only baseline for page/rotation metadata. Boundary noise
occupies an existing spare byte and needs no per-pixel hash. LAND synthesis
replaces one existing upload page without extra shader fetches or GPU memory.
Accepted bands add one neighbour texture sample and one metadata read; existing
footprint filtering remains. There is no new binding, draw or per-frame upload.

The updated local `seams-lead.sh` builds unchanged `bcffd680` and the candidate
with separate fresh target directories. It renders identical cameras, including
5,000 and 10,000 ft overhead UKR LAND-edge views, and records 24 alternating
4K High timing pairs at 100 and 2,000 ft plus the existing higher views. It also
compares full on-state GPU times to include changes in the ordinary material
path. New GPU appearance and milliseconds are unmeasured in this sandbox.

Validation and the unchanged simulation/guard boundary are recorded in the
report's Tile seams / Texture blend section. No importer, terrain-height,
simulation-class, wire or replay data changes accompany this work.
