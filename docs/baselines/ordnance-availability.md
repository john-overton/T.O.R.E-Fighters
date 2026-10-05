# Aircraft ordnance availability validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-10-05, on the local `import-variety` branch.
The source identities, logical station matrix and compatibility evidence live
in the [source review](../formats/aircraft-ordnance.md). Player-facing rules live
in the [ordnance specification](../spec/ordnance-presentation.md). Retail runtime
comparison is unavailable; these checks validate imported data and host behavior.

## Scope

Recover nine weapon-capable source stations previously omitted by the default-JT
filter. Preserve their empty/default-tank state until selected, preserve existing
weapon indices, and read older saved loads without filling the new rows. Keep
fixed device mounts occupied and unavailable to tank editing. Use shared,
source-numbered editor rows for weapons and tanks, with original thumbnails and
compatible replacement. Tanks lead the air-to-surface catalog; the extra Tanks
button and external-fuel/tank-shell banner are removed per the supplied retail
reference. Aircraft geometry and rendering are unchanged.

## Reproducible checks

With the development profile imported from user-owned media:

```sh
TORE_DATA_DIR=.local/dev-profile target/debug/tore-app --validate-ordnance --no-audio
TORE_DATA_DIR=.local/dev-profile target/debug/tore-app --validate-tanks --aircraft f14 --no-audio
```

`menus-ordnance-availability` runs the first command in the battery. It checks
all 36 retail identities, all imported supported weapon definitions against
source station rules, installed and optional tank placements, source-default
quantities, legacy station-prefix loads, accepted-load and mission-text round
trips, shared-mount exclusion, live configuration construction, and lightweight
multiplayer load validation. Full-capacity loads may exceed takeoff weight, so
placement/persistence checks precede a separate one-unit-per-station, zero-
internal-fuel validation. This does not assert every full-capacity combination
can take off.

The F-14 probe separately covers zero, one and two F250 tanks, external fuel,
shell mass, total mass, consumption order and accepted-load restoration.

## Results

The final workspace formatting, all-target clippy, build and tests passed:
2,952 Rust tests passed, with 42 ignored. All 230 Python tool tests passed.
Source and app/extractor binary asset guards passed, as did the documentation
check. The wrapped Linux GPU smoke test presented its requested screen.

The source audit passed for all 36 retail aircraft: nine recovered weapon rows,
1,464 compatible supported-weapon placements and 146 tank placements. It also
checks explicit rejection of overlapping tank/weapon loads, genuinely older
loads without tank records, unarmed aircraft and restoring an empty row with
plus. The F-14 zero/one/two-tank probe passed. All 36 scenarios in the final
ordnance battery passed; results are in the ignored local directory
`.local/battery/20261005-131933-ordnance-final/`.

All 22 focused editor tests passed, covering shared source rows, category-stable
occupants, mixed tank/weapon paging, tank-first surface ordering, cross-kind
replacement, tank transfer/unload, retained tank kind, rejected edits and future
six-card station pagination. The existing creator drag probe now resolves UI
positions through the source-row union and explicitly loads optional empty rows
before testing their unload/refill gestures.

Fresh F-14 air-to-air and air-to-surface captures are retained locally as
`.local/ordnance-editor-audit/f14-retail-aa.png` and `f14-retail-ag.png`.
Visual inspection confirms tanks first on the surface page, five stable loaded
station rows and no clipping. Both removed areas exactly match the imported
ORD_AIR3 background in both categories: button region `(278,35,60,24)` and banner
region `(343,69,295,20)`, with zero differing bytes. Earlier source-row captures
also cover A-4E, MiG-17, F-4E, AC-130 and unarmed C-130.

The broad quick-check battery completed 162 other scenarios. Its initial creator
run exposed the old compressed-index gesture assumption, corrected above. The
unrelated exhaustive AI matrix was stopped after about 20 minutes; no pass is
claimed for it. The final full creator run is still in progress. The required workspace checks
and focused ordnance acceptance above are complete; no full creator result is
claimed yet.

Validation logs are under `.local/ordnance-audit/retail-layout-*`. Selective
extractor checks also passed with no errors: original tank thumbnails are
included across FA archives, and aircraft-only MiG-17/MiG-23 profiles include
their explicitly documented empty-station seed definitions.

## Limits

The weapon catalog still requires implemented flight behavior. Importing an
unsupported JT does not make it usable. Source compatibility is broader than
reviewed gun-pod support; SUU16 remains restricted to its source-installed F-4J
mount. Fitted source-default tank capacity and gun-pod ammunition rules remain
as documented in their feature specifications. Campaign stock, era/airbase
restrictions, sensor-pod replacement and in-flight fuel-tank jettison remain
outside this pass. The supplied F-14 reference also exposes an existing
[675 lb internal-ammunition mass gap](../formats/aircraft-ordnance.md#internal-ammunition-weight-follow-up);
this layout change preserves the shared host mass rule pending a consistent
loadout/flight correction. No Windows or macOS interactive run is claimed.
