# Retail terrain and scenery review

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research followed by implementation, 2026-09-23, Linux. John requested a review
of retail ground detail, then selected textures, artwork, scenery and map
variants for all maps, with shaders/expanded landscapes deferred. The isolated
worktree is `T.O.R.E-Fighters-retail-terrain`, branch
`research/retail-terrain-review`. Research began on local `main` at
`61eedea5851bcde75c6686167bb92d33ee7d1776`. Implementation was rebased onto
`0a59263` after the newer flight-view, debrief, rocker and audio changes landed.
[Behavior specification](../spec/terrain-detail.md),
[data contract](../formats/terrain-materials.md),
[plan and options](../ROADMAP.md#retail-terrain-detail-review).

## Input identity and review method

Sources are the user-owned installation under the main checkout's
`gameassets/fighters-anthology/` and its existing local manual PDF.

| Input | SHA-256 |
| --- | --- |
| FA.EXE | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| FA_1.LIB | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| FA_2.LIB | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `.local/missile-update/manual.pdf` | `1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3` |

The manual is the 1999 electronic edition. Printed page 201, PDF page 206,
describes permanent map objects and map display categories. Printed pages
337-338, PDF pages 342-343, list rocks, roads, bridges, crop fields and urban
and industrial structures. Those three pages were rendered and visually read.
Text on printed page 331 describes a reference-view background toggle;
printed page 332 delegates graphics settings to the Install Guide. That is
not evidence for an in-flight detail-distance constant. The separate Install
Guide was not located or reviewed.

TORE review covered `tore-formats::{theater, mission, static_object, shape}`, the
app importer, `terrain.rs`, `terrain.wgsl`, `sim_renderer.rs`, and the existing
shoreline, airport, lighting, graphics and map specs. The reference checkout's
T2 notes were also read. Their shifted cell layout and DEM conclusion are
superseded by TORE's documented executable-backed T2 contract; its custom
terrain system was not used.

FA.EXE was disassembled with `objdump -d -M intel`. Reviewed locations are
recorded in the linked data contract. A local Rust probe used the current
bounded `Archive`, `Theater`, `Environment`, `Layout`, `Definition`, `Shape`
and `Pic` readers. It read retail data without executing any imported code.

## Extraction and artwork

From the new worktree:

```sh
python3 tools/extract_assets.py \
  --source ../T.O.R.E-Fighters/gameassets/fighters-anthology \
  --theater all --exclude-archive 'disc1/LHX/*' \
  --exclude-archive 'disc1/SETUP.ESA:*' \
  --out .local/retail-terrain-review/loose-assets
```

Result: **1,618 resources, zero extraction errors**, with archive/output hashes.
FA_1 supplied 901 and FA_2 supplied 717. The additional scanned FA archives had
no selected matches. This is the current profile, which also includes static
object dependencies, not the older 1,129-resource profile.

The first invocation also scanned archives nested in SETUP.ESA. Extraction
reported 3,236 resources and zero decoder errors, but the Python provenance
wrapper failed by treating `SETUP.ESA:FA_1.LIB` as a filesystem path. The clean
loose-archive run above avoids that existing wrapper defect. Nested-installer
provenance reporting was not fixed in this documentation-only review.

The pre-change profile selected **none of Kurile's 236 named terrain PICs**. The probe read
every one directly from FA_1 and decoded all successfully: 144 at 128-square,
92 at 256-square, no private palettes. Their source index rasters total 8 MiB;
expanding all to the host's current 256-square layers would take 14.75 MiB
before weather and object layers. Texture-layer limits need checking during
implementation because those resources currently share one GPU array.

The probe recomputed the size and both coverage grids for **1,078 of 1,078**
base-layout `tdic` records with exact agreement. It also decoded the three
LAND resources and five GRND sprite sheets. A local contact sheet was inspected,
including two Kurile coast images, representative river/road tiles and land
textures. Theater-specific tile previews use the source LAY's keyframe 2;
shared previews use DAY2 keyframe 2, with cutouts shown against a diagnostic
blue. These are asset inspections, not retail screenshots or live weather renders.

## Base-theater census

This table records the pre-change gaps measured during research.

The bare-land measure uses each in-bounds quad's first T2 sample. A quad is
counted as land when its color is not 255, and as bare when the current
`Environment` has no numbered texture placement for it. It is a coverage
diagnostic, not a percentage of screen pixels or a physical land-area estimate.
Zero bare land does not establish complete class behavior or object detail.
Bodies below mean nonempty main-shape projection, not complete visual acceptance.

| Theater | Samples | Highest sample, ft | Unique numbered placements | Bare land quads | Projected bodies / placements |
| --- | --- | ---: | ---: | ---: | ---: |
| APA | 256 x 256 | 7,936 | 1,177 | 0.0% | 104/107 |
| BAL | 256 x 256 | 4,096 | 3,356 | 7.8% | 163/165 |
| CUB | 256 x 256 | 6,912 | 870 | 0.0% | 123/129 |
| EGY | 208 x 200 | 7,936 | 1,671 | 2.2% | 152/152 |
| FRA | 208 x 200 | 7,936 | 1,957 | 13.8% | 218/218 |
| GRE | 256 x 256 | 6,400 | 2,372 | 0.0% | 189/191 |
| IRA | 256 x 256 | 7,424 | 3,326 | 1.3% | 158/160 |
| KURILE | 256 x 256 | 7,936 | 0 | 100.0% | 85/85 |
| LFA | 256 x 256 | 4,608 | 296 | 0.0% | 64/64 |
| NSK | 256 x 256 | 7,168 | 2,576 | 0.0% | 119/124 |
| PGU | 256 x 256 | 6,144 | 2,725 | 2.9% | 152/154 |
| SPA | 256 x 256 | 7,936 | 2,949 | 26.2% | 131/135 |
| TVIET | 200 x 200 | 7,936 | 1,236 | 19.2% | 514/544 |
| UKR | 208 x 200 | 7,936 | 695 | 66.3% | 257/257 |
| VLA | 208 x 200 | 7,936 | 1,484 | 0.4% | 160/160 |
| WTA | 256 x 256 | 7,936 | 1,618 | 0.0% | 155/158 |

Kurile's omitted named placements cover all 2,072 quads counted as land by this
measure. Ukraine has 19,084 bare quads out of 28,771; Pakistan has 16,636 out
of 63,566. These gaps make good review scenes, but do not establish the correct
fallback texture rule by themselves.

## Object findings

Before implementation, the research probe counted **2,803 placements** across
the sixteen base layouts, with
**2,744 nonempty projected bodies**, 42 unsupported projections and 17 no-body
controllers. This independently reproduces the earlier airport census.
The 42 unsupported placements use CHAP.SH or SA2.SH and fail at the same
unsupported opcodes noted there. Restoring their appearance does not authorize
ground-defense AI. No-body controllers are not missing visible buildings.

The two gameplay archives contain 170 OT entries. All 170 definition records
parsed. Of their referenced shapes, 163 projected, CRATER.SH failed its bounded
re-entry pattern, four carrier-tower shapes produced no accepted geometry,
and TREE1.SH/TREE2.SH were absent. Those last two definitions also have empty
display/class names and no base-layout placements. This does not establish a
usable retail forest system or exclude some separate resource path.

All ten ROCK/ROCKB variants project and have 35 placements in the base maps.
The four ROAD definitions project but have zero base placements. Crop fields,
bridges, city blocks and large-city shapes also project. Industrial blocks and
some other catalog scenery are available without being placed in the base maps.
Object catalog availability and mission placement coverage must stay separate.

The original scene builder consumed faces but omitted the projector's line list
and downsampled images larger than 256 pixels. Both omissions are corrected in
the implementation below. Main-shape projection still does not prove complete
shape-state coverage. Destroyed bodies retain the existing removal behavior;
no original damaged replacement is claimed.

## Implementation validation

The final implementation constructs **75 of 75** retail MM layouts, with
**10,252 placements and 10,235 visible bodies**. The other 17 are no-body
controllers. The sixteen base maps account for 2,803 placements and 2,786 bodies:
all 42 formerly unsupported CHAP/SA2 placements now project in a fitted static
loaded pose. No autonomous behavior was added. Both the isolated importer and
`--validate-maps` passed again after the final rebase.

The final all-theater extraction selected **1,859 resources with zero errors**,
including all 236 named Kurile PICs. It used the same source and exclusions as
above, writing `final-assets/extraction-report.json` with output/archive hashes.

All 75 layouts also passed a real Vulkan viewer smoke test on the NVIDIA
GeForce RTX 4070 before the final audio/debrief rebase. The renderer changes
were unchanged by that rebase. Every base map and four representative variants
were captured. Kurile coast artwork, Ukraine city/land detail, Pakistan terrain,
Egypt desert colors and variant scenes were visually inspected. The location
picker was captured with Ukraine variant 1 selected, and its page navigation
and label fit were inspected. Subsequent matched captures, Kurile fog, Ukraine night, the standard smoke and
a Ukraine-variant smoke exercise the final combined tree. At the low Kurile fog
camera, the uniformly obscured frame is byte-identical before and after
(SHA-256 `f034d6528e54decb5626b94b39697bf614866dcecd719671c731cb41ad3387c2`).
That checks preservation of the existing fog result, not visible ground detail
through dense fog.

Synthetic tests cover named dependencies without a theater filename prefix,
missing dependencies, unsafe names, duplicate placement precedence, signed
borders, reviewed alias bounds, exact 128-square texel replication, full-sized
non-square scenery/masks, UV-page clipping area and coordinates, more than 256
logical images, loaded-pose grammar bounds, variant menu/airport identity and
unchanged inspection-camera placement across variant labels.

All nine required repository checks passed on the final tree: **1,407 Rust
tests and 75 Python tests**, formatting,
warnings-denied Clippy, locked workspace tests/build,
repository/both debug executable asset guards and documentation validation.
The three normally ignored GPU tests were run explicitly and passed: geometric
shadows, smoke/weather-palette behavior, and smooth glare. The final suite includes the camera-identity regression and geometry-expansion
budget checks.

The prescribed 1,200-tick headless flight passed, ending at 436.693 knots and
5,014.249 feet without a crash. A 1,200-tick `~UKR1` ground-start rollout passed,
retaining runway identity `1073741824`, ending at 132.380 knots and 14.088 feet
MSL without a crash. These are deterministic scripted checks, not human landing
acceptance. No new default input binding or flight adapter was introduced.

### Matched captures and frame measurements

Before/after captures use identical cameras in Kurile, Ukraine and Egypt, at
960 x 720 and 4x anti-aliasing. `captures/before-after.png` shows the recovered
Kurile coastline/artwork and the retained city/desert composition. The baseline
is an isolated source snapshot of `c3d61c3`; the implementation includes the
later `0a59263` rebase. The sole instrumentation patch in the baseline enables
the existing frame counter for the viewer, which previously counted only flight.
The implementation also makes that opt-in counter available in the viewer.

Each longer run collected 1,200 frames, excluding the first 30, with zero paused
frames and no concurrent GPU workload. Both binaries are debug builds. These
are CPU wall frame intervals including presentation backpressure, not isolated
GPU times or a general gameplay FPS claim.

| Fixed view | Before median / p95, ms | After median / p95, ms |
| --- | ---: | ---: |
| Kurile coast | 3.24 / 13.60 | 3.61 / 13.62 |
| Ukraine city/coast | 4.88 / 14.77 | 5.24 / 5.72 |
| Egypt desert | 4.68 / 14.69 | 4.78 / 14.76 |

Median intervals increased by 0.10-0.37 ms in these views. Tail intervals vary
with desktop presentation, including in the shorter exploratory runs, so the
Ukraine p95 change is not evidence of a renderer speedup. The extra source
artwork is usable on this host; broader hardware/flight load measurements and
cold-import timing remain unmeasured. Logs: `*-perf-long.log` and
`performance-long.json`. Cameras and commands are retained in `compare.py`.

### Presentation choices and limits

Original image pixels, source placements and height samples are retained.
Generic land repeat scale, the choice to display each variant as a complete
static list on its resolved grid, loaded launcher pose and line stroke width
are host choices recorded in the [specification](../spec/terrain-detail.md).
Image paging does not add a new material effect. Shaders and expanded landscape
features remain in the future roadmap.

No Windows/macOS runtime check or retail side-by-side comparison was possible
in this pass. Dynamic scenery animation, destroyed replacements, runtime decals,
exact retail distance transitions and campaign progression are not claimed.
The Install Guide and missing TREE1/TREE2 shape path remain unreviewed. Retail
comparison is unavailable and does not block usable source-detail delivery.

Ignored evidence is retained under `.local/retail-terrain-review/` in the
repository checkout:
`probe.rs`, `census.tsv`, extraction reports/logs, `fa-disassembly.txt`, manual
page renders, `retail-textures.png`, `checks.json`/`check-*.log`,
`maps-after-rebase.log`, `render-maps.json`, `gpu-tests.log`, headless flight logs,
and `captures/`. The research probe used the locked-built `tore-formats` library.
Retail resources and all derived captures remain local and are excluded from
the source commit.
