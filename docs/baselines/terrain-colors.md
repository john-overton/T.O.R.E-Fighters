# Theater ground colour regression

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-14, slice nr-desert, base `81593406`.
Source identity and palette families: [weather contract](../formats/weather.md#theater-palette-families).
Requirement: [ground detail](../spec/terrain-detail.md).

## Cause and history

A throwaway detached worktree bisected the palette selector between `4062bb83^`
and `81593406`. Each step compiled that revision's `layer_resource` function
with its condition table and asked for Persian Gulf clear weather. Before the
function existed the loader read the explicit MM layer. First bad commit:
`4062bb831de7fa91f67110cf51b740accb085e21`, "Launch the recovered weather
conditions from the creator", September 14. It introduced `Some(condition)`
selection from the map's initial letter, replacing PGU's `DAY2E.LAY` with
`DAY2.LAY`. Iraq and Spain have the same defect; TVIET selects `DAY2T.LAY`
instead of its explicit `DAY2.LAY`.

This is a bisect of palette selection, not a sequence of historical GPU renders.
The historical parent reads `environment.layer` directly. The bad selector
survives the terrain/scenery split, render quality, city fill, B12 and night
lighting changes. Egypt begins with E and is unaffected. The B12 before/after
Egypt day sheets are both sandy, consistent with this finding; those previews
also use the explicit MM layer by day, unlike a Quick Mission weather choice.
The detached worktree was removed with `git worktree remove`.

The 16 base MM files and all 59 variant MM files were read from the existing
local import. Every variant agrees with its base palette family. No retail
bytes or extracted art are included in this change.

## Regression and limits

`render-terrain-colors` checks ground palette entries 192 through 255 exactly
against an independent MM-derived reference for each of 16 theaters, six
conditions and three altitudes (288 comparisons). It also reports mean RGB
across non-water T2 cells. It exercises the runtime terrain loader; the
reference reads the source MM family instead of using the selector under test.
Unit tests cover every base family and generated layout selection.
The base build fails 58 of 288 comparisons: 15 each in IRA, PGU and SPA,
and 13 in TVIET. The remaining 230 pass. Shared white cloud/fog bands can
match even when the ground palette family is wrong.

The optional GPU capture fixture uses the same scenery, indexed terrain and live weather
palette path as flight, at near and far camera heights by day and night.
The flight map's T2 fallback reads that scenery palette. Briefing pictures
retain their own palettes; the creator's missing-picture fallback remains plain
grey and does not share this path. Retail runtime screenshot comparison is unavailable;
this validates source palette selection and host rendering, not full retail
lighting parity. Historical recordings with an explicit resolved layer keep
that recorded layer by design.

After the correction, all 288 comparisons pass. All eight Iraq/Persian Gulf
CPU captures change to the appropriate day/night colours. All eight
Ukraine/Kurile control captures are byte-identical before and after.
The before captures reproduced green ground in Iraq and Persian Gulf, with
Ukraine green and Kurile snow/artwork controls. The execution sandbox exposes
no hardware GPU. The fallback OpenGL adapter
rejects an existing depth-texture shader; no Vulkan software ICD is installed,
and the configured package mirror is unreachable from this environment.
Captures therefore use the diagnostic's CPU mode: actual scenery mesh and
indexed terrain textures, unlit orthographic views with the live day/night
palette. GPU lighting and distance-filter appearance were not validated.

Local evidence: `.local/tmp-nr-desert/` contains bisect steps, source hashes,
logs and the private profile. `.local/next-round/renders/nr-desert/` holds
before/after PNGs with their README. The slice report records final check
results and any remaining findings.


## Validation results

Formatting and workspace clippy passed, as did the locked workspace binary
build. Simulation tests: 1,940 passed, 8 ignored. Application tests excluding
three socket-dependent networking/UI modules: 1,643 passed, 24 ignored,
233 filtered out. The full application run hit socket permission failures
and two stalled connection tests; a focused socket-bind test confirmed
`Operation not permitted`. These are environment exclusions, not passes.

The changed battery selected 138 headless scenarios and all 138 passed,
including the new colour regression. Its initial Python precheck could not
allocate a UDP port, so the scenario run used the existing `--no-unit-tests`
option. The separately run full Python suite completed 469 tests with two
socket-permission errors; the other 467 passed. The single-player guard
against `guard-base-334a70e7` reported SAME 52, DIFFERENT 0, MISSING 0.
Documentation validation passed. All heavy work was throttled and sequential;
no windowed run or full battery was used.
