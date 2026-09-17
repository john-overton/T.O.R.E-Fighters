# Creator and ordnance implementation, 2026-09-14

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature; see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


## Delivered behavior

All 30 briefing fields are editable from the fingerprinted active FA selector
contract. Scalar clicks cycle; Shift-click opens the list. Aircraft and theater clicks open
the paged selector. Aircraft choices now follow the
[imported-only menu specification](../spec/quick-mission-menu.md). List selection commits with OK/Enter; Cancel/Escape
preserves the previous field. Theater selection maps all 16 source entries by
identity and resets nationality/target dependencies. Friendly Wing 1 cannot be zero.

Creator OK validates supported imported aircraft in every populated wing, no
ground targets/defenses, and one of the six supported weather conditions.
All six wings launch straight-flight fixtures according to the
[mission specification](../spec/quick-mission-menu.md#straight-flight-mission-fixtures).
Selected 5,000/10,000/20,000/40,000-foot altitude is passed unchanged; insufficient
local terrain clearance rejects launch. Separation sets starting distance. Nationality, skill and situation
remain setup data with no AI/objective effects. BARCAP mission generation is not
implemented; unsupported configurations are explained by launch notices.

Load Ordnance no longer shows the straight-flight dummy description; its message
area is reserved for loading feedback and validation errors.

Custom weapons opens Load Ordnance with the original background/palette, fonts,
weapon thumbnails, station headings, dial and rocker art. Catalog categories retain
separate pages. Click a weapon then a compatible station, or drag it there, to load.
Tab selects the next station; +/- adjusts its quantity; right-click decrements.
Fuel rocker changes internal fuel by 500 lb with source-capacity bounds. Weapons
provides Unload All; Airbase/Cheat operations report their unavailable status.
Select Plane returns to the retained creator draft; custom loads survive that return
for the same aircraft. Standard load currently uses reviewed PT defaults, then flies;
it is not acceptance of retail mission-specific standard-load assignment.

Fly validates supported weapon identity, compatibility, ammunition, fuel and mass.
Accepted stores feed combat, geometry, instruments and flight payload without a
practice target. Restart restores accepted ammunition, fuel and selected altitude.
Direct free flight now carries supported default stores. The explicit live-fire
range remains available; native research flight retains its clean restriction.

Current visual validation is recorded in the
[ordnance presentation pass](ordnance-presentation.md).

## Validation

Linux, Rust 1.91.1, locked dependencies:

- Formatting, warnings-denied Clippy, workspace build: passed.
- 228 Rust tests and 20 Python tests: passed. Synthetic tests cover selector cache
  truncation/contracts, fingerprint hashing, draft acceptance/cancel/dependencies,
  loadout compatibility, quantities, fuel and weight boundaries.
- Source and both debug-binary asset guards: passed. No retail derivatives committed.
- Fresh import with bounded executable read, resource-conflict checks and shared
  creator profile: passed; cache limit is 256 MiB / 4,096 entries.
- `tore-app --validate-creator`: both identities passed six compatible supported
  store/placement cases each, edited fuel, empty stations and ammunition reset.
  This is a headless state probe, not automated pointer-to-flight acceptance.
- Shared extractor `--creator --aircraft f18 --aircraft rafale --validate-flight`:
  passed all 26 deterministic flight scenarios with source/output provenance.
- Both existing `--combat-smoke` identities passed, including default-slot damage
  cases and systems checks. Custom-load combat tape serialization is not implemented.
- CPU snapshots of creator and both aircraft ordnance screens were inspected.
  Local files: `.local/creator-final.png`, `.local/ordnance-final.png`,
  `.local/ordnance-rafale-final.ppm`. Derivatives remain ignored.

GPU smokes passed on NVIDIA RTX 4070 / Vulkan / Immediate for creator, ordnance,
viewer and both aircraft. Ordnance also presented at 1280×720 (Rafale) and 640×900
(Hornet). These prove presentation, not manual hit alignment. The shell initially
lacked display variables; checks used the existing user's Wayland socket. The
ordnance smoke exposed an overly strict snapshot-state guard, fixed to permit
`--smoke-test` as well as CPU snapshots.

Commands and logs are under ignored `.local/creator-*.log`. Windows/macOS runtime
acceptance is unavailable on this host.

## Remaining parity and hands-on gate

The recovered scalar tables are exact for the reviewed executable; complete 1:1
screen/mission behavior is not claimed. Open: dynamic aircraft era/eligibility
filters, auxiliary/tank editing, year/stock/airbase/cheat lifecycle, complete native
mass/drag/visual pairing, dial states and hold-repeat timing, sentence wrapping,
popup art/geometry/gesture comparison against original execution, custom replay,
combat AI/ground targets/defenses/objectives and other weather/start behavior.

Human testing should cover custom → edit → Select Plane → OK → Fly → restart →
return for each aircraft, full catalog browsing and resize/pointer alignment. Reject
unsupported setups explicitly; do not interpret editable setup options as implemented
mission systems. Full plans remain open at those acceptance gates.

## Straight-flight fixture validation, 2026-09-17

Implementation mode. Synthetic tests exercise all six populated wings, exact
selected identities, invalid populated-wing rejection and 29 distinct straight
trajectories over 120 ticks. The imported `--validate-creator` pass succeeds for
all twelve aircraft: normal default ammunition, reset after depletion, clean pilot-only recording, 29 dummy
geometries using the selected model, restoration after damage and movement, and
existing custom/empty loadout checks. Local log:
`.local/creator-dummies-validation.log`. Placement and heat remain fitted, and no
retail execution comparison or aircraft combat AI is claimed.

A mixed 15-aircraft formation (F/A-18D, Rafale C and F-22A) rendered successfully
on NVIDIA RTX 4070/Vulkan. The captured cockpit and radar scope were inspected
in `.local/formation.png`; the shared scope displayed the formation contacts.
App/simulation tests passed (106/178), as did warnings-denied Clippy and the
144-file documentation header check for this stage.
