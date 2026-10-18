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

## Current seam and flat-tile validation

Implementation at `0cd9f492d3100d525d586c1de4fb7c7fc9f70bef`, 2026-10-18.
John approved enriching flat named-tile interiors while keeping already mottled
interiors unchanged. The reviewed UKR edge at x=16,384, z=32,768 has mottled
LAND.PIC south of it, upload page 28, and flat UKR23.PIC north of it, page 0,
rotation 2. These labels come from the actual mesh and source resources.
The earlier whole-LAND replacement flattened the wrong side and is removed.

The supplied lead run is
`.local/next-round/renders/exp-vegetation/seams-texture-review-20261018-113730-715135/`.
Its comparison images retain LAND's original mottling but show the flat UKR23
area clearly. Its median 100 ft 4K High seam increments are 0.128512 ms UKR and
0.125248 ms FRA. Those numbers describe the base, not the new flat-tile work.

Runtime detection compares green-ground low-pass luminance deviations at
256, 512, 1,024 and 2,048 ft against mottled reference art. Statistical thresholds,
world-space noise fitting, protected-feature rules and fades are specified in
[terrain detail](../spec/terrain-detail.md#tile-seams). Source art, palette bytes,
mesh vertices and tile rotations are never replaced. A separate correction
adds broad modulation only to eligible flat placements. The existing noisy
500 ft texture blend composes the enriched materials and remains bounded.

CPU evidence is local, under `.local/tmp-exp-vegetation/flat-tiles-final/`.
The audit checks one deterministic representative of each flat source page,
including the reviewed UKR23 placement. It measures the actual half-float data
and interpolation supplied to the GPU, not an idealized noise generator.

| Theater | Flat source pages | Enriched placements | 512 ft SD after / reference |
| --- | ---: | ---: | ---: |
| UKR | 12 | 504 | 0.9654 to 1.1057 |
| FRA | 17 | 1,318 | 0.9872 to 1.1652 |
| BAL | 13 | 1,635 | 0.8947 to 1.0728 |

These counts include neutralized ground pages. Examples are UKR23, FRA34 and
BAL17. The UKR23 representative rises from 0.00033557 to 0.01869859 at 512 ft,
against LAND's 0.01844048. All 42 representatives meet the 25% contrast target
on eligible natural ground. Valid block counts accompany every scale; some
Baltic 2,048 ft samples are insufficient after feature guards and cannot
establish a large-scale match. Before/after/reference PNGs show the modulation
and retained fine grain. Separate mask images make the measured area visible.

Road, water and other protected source samples remain exact: 91,587 UKR,
66,899 FRA and 90,993 BAL samples. Unclassified placements have zero flat
modulation. Original source bytes and every terrain vertex match the disabled
scene. Across all placements, 665,600 / 665,600 / 1,048,576 interior probes have
zero seam response. Synthetic checks cover unchanged mottled interiors, fine
texture scaling, runway/water/road protection, coherent fields and stable noise
without a tile-spacing repeat. Cache reconstruction is exact and resource
limits pass for all three theaters.

The actual baked UKR edge changes the source step across -1/+1 ft from 12 RGB
levels to 0.075768. The independent colour component retains its exact 500 ft
ramp; texture endpoints equal their enriched source values at +/-250 ft.
Original source colours outside the band stay unchanged on mottled tiles.
Flat interior changes are separate and explicitly authorized.

The cache holds one 96-square world-space modulation patch per active flat
placement, appended to the seam response texture. Response storage totals
64 MiB UKR, 184 MiB FRA and 168 MiB BAL, versus 15.75 / 70.875 / 31.5 MiB on the
base. No shader noise, additional source-art fetch, ordinary draw call or
per-frame upload is added. A page flag skips modulation on mottled pages;
active flat fragments add one record load and one filtered response sample.
This is a memory-for-bandwidth tradeoff. Nonuniform weather palette remaps
remain a fitted approximation requiring visual review.

The local `seams-lead.sh` compares `0cd9f492` against a frozen candidate, with
separate fresh Cargo targets and private profiles. It retains the 2,000 ft
seam views, 5,000/10,000 ft UKR23/LAND overheads and 20,000 ft overview, plus
FRA/BAL controls. A recorded observer-only patch adds unlit material captures
to the baseline. Full-tile probes and a candidate eligibility mask let
`flat-gpu-check.py` measure the same eligible natural ground before and after,
require contrast gain within 25% of its mottled reference, and require exact
mottled-control equality. Whole-crop contrast and eligible coverage are also
reported, so protection margins cannot be hidden by the acceptance mask.
The shader's seam mask still has the independently projected 500 ft width.

The cost job alternates off/on with one scene, fixed populations and identical
cameras, disabling both flat modulation and seams in the off state. It reports
three batches of signed GPU timing pairs and full-frame times, including 100
and 2,000 ft at 4K High. The acceptance target is at most 0.5 ms total increment.
**New GPU contrast, ordinary appearance and cost are not measured here.**
They require the lead's run. CPU statistics do not establish GPU acceptance.

Validation: 19 seam tests and both shader/binding suites pass, as do workspace
clippy with warnings denied, locked workspace bins, docs and asset checks.
Full `tore-app` has 1,998 passes, the same 69 socket failures as the base review,
40 ignored and two known indefinite socket waits excluded. Python has 481
tests, the same two sandbox socket errors and four skips. Three targeted
vegetation battery cases pass. The final diagnostic-mask addition also passed
focused shader, seam, lint, build and real-map checks. Logs and full validation
results stay in scratch and the report.

No importer, simulation, terrain-height/class, AI, wire, replay or checkpoint
change. No GPU/window run, new guard, commit or push. The historical SAME 52
simulation guard result is not a new measurement for this presentation change.
