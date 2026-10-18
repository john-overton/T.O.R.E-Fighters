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

## Final CPU results

The final boundary-mean mask accepts 6,248 Ukraine and 31,999 France colour
segments, each 128 ft long. Baltic has no qualifying same-class colour segment;
its different-class field/forest edges stay crisp. Tree-belt segments number
89,862, 3,633 and 43,288 respectively. UKR/FRA use 9,328,800 map bytes each,
Baltic 14,696,448. Maximum local mean fit error in linear RGB is 0.0006128 for
UKR and 0.0022073 for FRA. The cache reconstructs pages exactly and all three
maps pass portable resource checks.

In six unlit source/correction strips, sampled edge RGB RMS drops from 6.93 to
3.26 in Ukraine, 11.94 to 1.72 in two France strips, and 6.11 to 0.95 in the
third. Every pixel outside the 500 ft band is byte-identical. Source grain,
exclusion gaps and run-end fades remain. These are CPU material measurements,
not GPU visual acceptance or timing.

Final formatting, all-target workspace clippy with warnings denied, locked
workspace binaries, nine seam tests, both shader suites and asset/doc checks
pass. The final full app run has 1,988 passes, 69 failures identical to earlier
sandbox socket failures, 40 ignored tests and two excluded indefinite socket
waits. Python runs 481 tests with its two known socket errors and four skips.
Three targeted CPU battery scenarios pass. The final real-map check passes
again after the boundary-colour refinement.

The requested guard completes 32 cases, including 144 roster combinations, in
795 seconds: SAME 52, DIFFERENT 0, MISSING 0 against `guard-base-36ab5211` on
`profile-guard36`. Timing is disabled. The last colour-sampling refinement
follows this guard and affects presentation precomputation only; the final
app/shader/build/map checks cover it. No simulation, terrain data, wire or
recording path changes afterward. GPU smoke/cost checks remain for the lead.
