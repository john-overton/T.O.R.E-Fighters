# Original theaters and atmosphere: first recovery pass

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


Checkpoint: 2026-09-13, supplied Fighters Anthology installation. This is partial M1b recovery and an executable Ukraine preview, not accepted 1:1 environment parity. No USNF-ATF terrain, DEM, satellite imagery or engine code supplies this surface. The original executable was inspected as data and disassembly; it is never loaded or executed by the importer.

## Reproduce and identify the inputs

```sh
python3 tools/extract_assets.py --theater UKR --out .local/ukraine-import
mkdir -p .local/theater-research
objdump -d gameassets/fighters-anthology/FA.EXE > .local/theater-research/fa-disassembly.txt
```

Addresses below are virtual addresses in this exact `FA.EXE`, SHA-256 `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`. They are not portable to another edition. The extraction report records archive/output SHA-256, offsets and decoded sizes, plus structured terrain/mission metadata. Full inputs and derived captures remain ignored. Readers live in `crates/tore-formats/src/theater.rs`; runtime construction in `crates/tore-app/src/terrain.rs`.

## BIT2 / T2: confirmed packed layout

All 16 supplied T2 resources parse with these rules. Multi-byte numbers are little endian; names are null-terminated ASCII. Unknown header bytes remain in the extracted original file and are not assigned runtime meanings.

| File offset | Field / evidence |
| --- | --- |
| `0x00` | `BIT2` magic |
| `0x04`, 80 bytes | Theater display name |
| `0x54`, 16 bytes | Briefing map resource name |
| `0x64` through `0x78` | Legacy/unknown header fields; do not interpret as grid dimensions |
| `0x79` | u32 cells per coarse tile, 8 in this installation |
| `0x7d`, `0x81` | u32 coarse tile columns, rows |
| `0x85` | u32 coarse-grid file offset |
| `0x89`, `0x8d` | u32 fine-grid columns, rows |
| `0x91` | u32 fine-grid file offset, **149 / `0x95`** |
| Fine grid | Row-major three-byte cells: **palette color, class, elevation** |
| Coarse grid | Same cell representation; follows fine grid with no trailing byte |

The loader relocates `+0x85` at `0x4c5e85` and `+0x91` at `0x4c5e92–0x4c5e9a`. Lookup `0x4c6040` uses the fine grid below a step of `cells_per_tile`, otherwise divides coordinates by that value and uses the coarse grid. Rust implements both lookups; the initial renderer uses the full fine grid.

The terrain block builder at `0x4a9d00` copies the third byte of four corner cells into heights, and the first two bytes into material/class fields. Geometry construction at `0x4aa4f1–0x4aa56c`, world coordinate shifts around `0x4a9e32`, and elevation conversion at `0x42c091–0x42c0a4` establish **8,192 feet per horizontal cell and 256 feet per elevation unit**. Ukraine has 208 × 200 samples, 26 × 25 coarse tiles, and elevations 0–31 (0–7,936 feet). The preview uses X/east, Y/up, Z/north as a presentation convention; landmark orientation still needs native comparison.

**Lesson:** the reference reader's `0x94` starting offset shifts every triple by one byte, and its dimensions/elevation interpretation is not the retail layout. T2 contains actual height samples. A DEM is unnecessary to recover this terrain. This correction supersedes the earlier unresolved-elevation note in our progress tracker; it does not establish native tessellation or shoreline parity.

## Terrain textures and placements

`UKR.MM` contains 697 top-level `tmap col row texture rotation` lines at 695 unique coordinates and 29 `tdic 256` dictionaries. Coordinates (196,56) repeat texture 23 with rotations 1 then 3; (144,64) repeat textures 13 then 11, both rotation 3. The current Rust map/export retains the last entry; native duplicate precedence remains unverified, and all original lines remain in the extracted MM. The native parser at `0x482f69–0x482fb6` reads the four placement fields. Initialization at `0x4aa620` builds `%s%d.PIC` names, selecting `UKR0.PIC` through `UKR28.PIC` (256 × 256 each). Lookup at `0x4aa840` rounds cell coordinates down to multiples of four and searches eight-byte records by `(row << 16) + col`; texture and rotation are at `+4` and `+6`.

Each placement covers four cells per side, or 32,768 feet. Quarter-turn UV selection starts at `0x4aa9ac`; texture scanning around `0x4aa72d` reverses source rows. Rust applies these rotations and a V flip. Index 255 is tested as cutout/water coverage at `0x4aa739`. The runtime now treats those texels as holes exposing the shared ocean/horizon pass; untextured color-255 cells also leave that pass visible. The former land-color fill and palette-223 water fallback caused rectangular green strips beyond beach artwork and have been removed. Bilinear coverage uses a fitted 0.5 cutoff, not a recovered native raster threshold. See [shoreline behavior](../spec/terrain-shorelines.md) and [validation](../baselines/ukraine-viewer.md#shoreline-correction-2026-09-16).

Named tiles and fitted LAND/VLAND presentation are implemented. Source-aware terrain detail transitions and class-dependent material behavior remain open; the host uses pixel coverage directly rather than the original `tdic` cache. The [terrain material review](terrain-materials.md) establishes the dictionary contract and named Kurile artwork, while distinguishing the remaining land-plane questions. Exact retail shore/water geometry and lighting remain unverified; current lighting is described below. Fixed triangles currently join each four-sample quad; the height query uses those same triangles. `UKR.MM` contains 257 object placements, now imported into the static scene with
original shapes/textures, target identity and contact geometry. The sixteen base
theaters share this path. [Airport placement contract](airport-placements.md)
records unsupported shape and campaign boundaries.

The ocean now uses user-requested short ripples and distance/altitude filtering,
retaining the original textures and weather colors. Whitecaps are removed. [Source contract and limits](ocean.md),
[behavior](../spec/ocean.md), [acceptance](../baselines/ocean.md).

## Mission environment and weather modules

Follow-up: [native clock and turbulence research](weather.md) confirms continuous
time and LAY time-window selection. The dynamic palette implementation and its
remaining approximations are documented in [the full weather review](../baselines/weather-review.md);
it supersedes the historical midday-only runtime descriptions below.

The bounded `textFormat` reader exports top-level map, layer, layer parameter, clouds, wind, time and texture placements. The separate bounded mission reader now parses the indented object fields for
static scene construction. `UKR.MM` specifies `UKR.T2`, `DAY2.LAY 0`, clouds 0 and time 12:00; wind is absent and remains null. For example, `UKR01.M` specifies layer parameter 4, wind `160 7` and time 17:40. Wind units and the layer parameter's full semantics remain unverified.

Campaign missions reference names such as `~UKR6.T2`. Preserve these names; do not silently redirect them to `UKR.T2`. The parser accepts `~` and `$` resource-name characters. Resolving generated campaign terrain aliases is future work.

LAY files here are **PL modules containing weather data and native imports**, not menu layouts. The reader resolves data RVAs inside the bounded CODE section without executing code. The CODE root has the base 768-byte six-bit palette RVA at `+0x70`, and the weather-record table RVA at `+0x74`. Records are 352 bytes (`0x160`); flags bit 0 marks the sentinel.

Native palette copying at `0x4b4592–0x4b45c5` and palette writes at `0x4b3631–0x4b3661` establish:

| Record bytes | Destination palette entries |
| --- | --- |
| `+0x3e .. +0x9b` (93 bytes) | 224–254 (31 RGB colors) |
| `+0x9b .. +0xfb` (96 bytes) | 192–223 (32 RGB colors) |

Rust expands six-bit RGB to eight-bit and uses **explicit DAY2 keyframe 2** for a midday preview. Native time/altitude keyframe selection and fixed-point interpolation (`0x4b3820`, helpers `0x4b3b60`, `0x4b3b80`) remain to port. The two palette ramps are not a single contiguous destination range: reversing them produces incorrect terrain and sky colors.

DAY modules import `_T_HorizonProc` from `main.dll`; this build's export at `0x4aace0` is a return stub. FOG also imports `_WRFogLayerUpdate`; `0x4b4320` adjusts a field at `+0xfe` by a random -25…25 and clamps it to 217…235. This callback, its caller timing and seeded weather evolution are not implemented.

## Sky, sun, moon, stars and clouds

The shared extraction profile includes all of the following, preserving archive boundaries:

| Resources | Recovered dependency / current use |
| --- | --- |
| `SKY0.PIC` … `SKY8.PIC` | Nine 256 × 256 indexed sky textures; SKY0 is rendered |
| `SUN.SH` | Original sun shape, no literal PIC reference found |
| `MOON.SH`, `_MOON.PIC` | Shape references `_moon.PIC`; both preserved |
| `STARS.SH` | Original star shape, no literal PIC reference found |
| `CLOUD1.SH`, `_CLOUD1.PIC` | Shape references `_cloud1.PIC`; both preserved |
| `CLOUDS.SH`, `CLOUDS.PIC` | Shape references `clouds.PIC`; both preserved |
| All `.LAY`, `PALETTE.PAL`, `GRND*.PIC`, LAND/VLAND variants | Preserved for weather, palette and terrain follow-up |

SUN/MOON/STARS resource-name references occur in FA.EXE at `0x50c42c`, `0x50c434`, `0x50c43c`. Sun and stars appear to use shape commands rather than separately named PICs; this is an inference from literal references, not complete SH interpretation.

The renderer now selects source sky/ocean decks and uses world-anchored plane projection. Original sun circles/glow remap, moon billboard and point stars render with live weather palettes and source clock/angle gates. Cloud geometry and special horizon/ray-fog behavior remain under implementation; see [weather foundation](../baselines/weather-foundation.md) for current evidence and limitations.

## Next recovery steps

The [retail terrain detail plan](../ROADMAP.md#retail-terrain-detail-review)
owns current terrain and scenery sequencing. The named-tile and coverage
contracts are in [terrain materials](terrain-materials.md). Weather and
celestial work is tracked in the linked weather guides. All 75 static layouts are selectable. Live campaign composition and further
state-dependent source-detail behavior remain open. Retail visual comparison is unavailable.

## Broader extraction checkpoint

All 16 named profiles now share one definition table; `--theater all` selects their union. The supplied disc additions introduce no extra matching terrain resources, but supply reference media and audio/video archives. See [extraction commands and scope](../EXTRACTION.md#all-defined-theaters-and-the-retail-discs).

All 75 MM layouts parse. PGU.MM contains border placements (-4,244), (-4,248), (-4,252); SPA.MM contains (48,-4), (52,-4), (56,-4), repeated in their campaign layouts. Texture placement coordinates therefore use signed integers. The initial Ukraine renderer still validates its own placements against its grid; parsing the other theaters does not assert their rendering parity.

## Runtime expansion to the 16 base theaters

The viewer uses the selected base or variant MM, its referenced terrain and DAY2 palette, and paged indexed artwork. The numbered texture naming convention uses the first three code characters: TVIET therefore needs TVI0–41.PIC. The shared profile now includes TVI. Kurile has no numbered tmap placements; its 236 named placements now render with their original artwork. [Named-tile contract](terrain-materials.md#named-texture-placements). Other theaters have 29–68 numbered texture layers in this preview.

Signed out-of-grid border placements remain preserved; the mesh only queries patches intersecting actual fine-grid quads. Camera starts and fixed triangles are authored investigation behavior. All 16 passed Metal startup/render checks; native landmark/shoreline/atmosphere parity remains open. See [runtime validation](../baselines/all-theaters.md#runtime-and-typography-follow-up).

## Diagnostic native contact geometry, 2026-09-15

The vertical cell diagnostic now preserves the native diagonal, word normals,
integer intersection and imported square-root table rounding. It does not change
the preview renderer or its height sampling. Native fine lookup returns a zero-
elevation/class-1 fallback for out-of-grid corners; it does not clamp to the last
sample. [Contract](native-land-contact.md#vertical-terrain-arithmetic-ne-001b),
[measured decoder/replay coverage](../baselines/native-land-geometry.md).

Smooth presentation adds continuous sunlight and geometric shadows to the
existing terrain triangles, with shared area-weighted vertex normals for
continuous lighting across their edges. Shadow positions and height queries
still use the source triangles. This is an authored renderer, not newly recovered
terrain behavior. See the [shared surface spec](../spec/surface-lighting.md).

The in-flight map fits the original briefing image to the T2 grid extents,
with positive world Z at image north. T2 cell colors provide a fallback. This
is fitted cartography; the exact source projection has not been recovered. See the [map specification](../spec/flight-map.md).
