# External fuel tank selection validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation-mode validation, 2026-10-05, on Linux x86_64. Workspace base
`7d25975b8bde296442ecb873d256cc51864a2a4a` with the uncommitted variety and tank
changes. The [tank specification](../spec/external-fuel-tanks.md) owns the host
rules; [F-14 source data](../formats/aircraft.md#f-14-default-external-fuel)
owns the record values and resource hashes. No retail gameplay comparison ran.

## Source-backed F-14 result

`TORE_DATA_DIR=.local/dev-profile cargo run --locked -p tore-app --
--validate-tanks --aircraft f14 --no-audio` passed. The probe constructs the
source default load, changes installed tank quantity, accepts that load, then
restores it twice from the standard source load. Internal fuel stays distinct.

| Installed tanks | Internal fuel lb | External fuel lb | Shell mass lb | Loaded mass lb |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 15741 | 0 | 0 | 60815 |
| 1 | 15741 | 1650 | 198 | 62663 |
| 2 | 15741 | 3300 | 396 | 64511 |

Each accepted count survived both restores. Consuming all 3300 lb of external
fuel preserved internal fuel and the 396 lb of empty shells. An older loadout
without an explicit tank list preserved the source-default two-tank load.
The local independent reader/loadout probe produced the same rows.

The source F-14 closure extracted 147 resources with zero errors into
`.local/tank-f14/`; its report retains archive/resource SHA-256 values. The
complete variety source report lives separately in
`.local/aircraft-variety/extraction-report.json`.

## Regression coverage

`menus-tanks-f14` is the isolated display-free battery scenario. It checks all
three exact fuel/shell rows and the accepted-load restart marker. Synthetic tests
cover tank selection/mass, separate empty shells, UI removal/reload/Unload All,
mission-text source-default versus explicit-empty lists, and network loadout
round trips. All five scoped tests passed: separate tank shell/fuel accounting, UI
removal/reload/Unload All, mission-text old defaults versus explicit empty,
network loadout round trips, and sustained gun-pod firing with retained empty
hardware and explicit jettison. The source-backed creator loadout stage passed
all 37 selectable identities, including all 23 new aircraft. It checks accepted
loads, store removal, restart, applicable gun sights/tracers and finite damage
geometry. Rotorcraft wing geometry is reported as partial where no fitted wing
surface applies, rather than counted as a distinct damage-body success.
The final source-backed F4J probe held the trigger for 120 ticks, confirmed
continued fire from its 600-unit pod budget, then confirmed that zero ammunition
retained the hardware mass. The complete 37-identity loadout-stage log is local
at `.local/variety-creator-loadouts-final.log`.

A CPU snapshot at `.local/tanks-f14.png` was visually reviewed. The new Tanks
button uses the original pieces and the catalog uses original tank icons. The
internal/external/shell/total readouts match the two-tank row above, with no
clipping or overlap. The image remains local-only.

## Material limits

No separate tank mesh, fitted tank drag law, partial-fill editor or in-flight
tank/fuel jettison was added. GAS has no shape pointer; source shape/attachment
behavior needs a separate review. This pass validates selected hardware, mass,
fuel and persistence, not original visual placement or retail flight parity.
Controller operation, a multi-computer lobby session and Windows/macOS were not
run by this validation pass.
