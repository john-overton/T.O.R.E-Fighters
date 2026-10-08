# Selectable external fuel tanks

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-10-05. John requested this alongside the
[aircraft variety batch](aircraft-variety.md) on 2026-10-05. His reported case
is the F-14 starting with external fuel while Load Ordnance offers no fuel
tanks to select or remove. The requested outcome is explicit tank selection
and separate internal and external fuel. Tank selection, removal, separate
mass/fuel accounting and accepted-load persistence are implemented. Source-backed
headless validation and synthetic checks are recorded in the
[tank baseline](../baselines/external-fuel-tanks.md). Original-game comparison
and in-flight tank jettison remain unavailable.

## Current implementation

The runtime now keeps installed tank equipment separate from weapon ammunition:

- `combat::live::Configuration::from_source` reads source-default `.GAS`
  stations and compatible tank hardpoints into explicit tank rows. Each selected
  row records its exact resource, original hardpoint and installed quantity.
- `combat::loadout::Loadout` exposes tank selection/count changes independently
  from weapon quantities. Shell mass and full starting external fuel derive from
  the selected GAS records and counts. Fixed external equipment excludes tanks.
- Load Ordnance lists compatible tanks first in the air-to-surface catalog,
  following John's retail reference. Source-numbered station rows remain visible
  in both categories and show their actual weapon or tank. No separate Tanks
  button or external-fuel/tank-shell banner is shown. Its
  fuel rocker continues to edit internal fuel only, in 500 lb steps.
- `aircraft_systems::fuel::Fuel` already stores external fuel separately and
  consumes it before internal fuel. Separation of fuel quantities exists;
  the previous missing piece was selectable tank equipment, which is now exposed.
- `tore-formats::weapons::Tank` and `combat::loading::Store::tank` already
  provide bounded tank parsing and a tank compatibility representation.

The [retail catalog](../formats/fa-aircraft.csv) records F14.PT internal fuel
as 15,741 lb. Its default tank quantity, capacity and empty mass are now
reviewed in [F-14 source data](../formats/aircraft.md#f-14-default-external-fuel).
The catalog's station count must not be confused with individual tank count.
The previous weapon-only editor hid the default tank station. The source-backed
probe now follows zero, one and two selected tanks through accepted loadout
restoration without restoring a removed tank.

## Requested behavior and host rules

Tank selection and removal are John's requested behavior. The following
detailed rules are agent decisions, 2026-10-05, with fitted behavior where the
original contract is not recovered.

| Case | Implemented result |
| --- | --- |
| Load Ordnance | Show supported compatible tanks and their assigned stations; allow loading and removal without treating tanks as ammunition |
| Internal fuel | Keep it distinct from installed tank capacity and external fuel; removing a tank does not remove internal fuel |
| No external tanks selected | Start with exactly 0 lb of external fuel and 0 lb of tank shell mass |
| Tanks selected | Derive capacity from those exact GAS records and quantities; show internal fuel, external fuel and total mass consistently |
| Default load | Source-default tanks may remain, but must appear in the editable accepted loadout; never silently restore a removed tank at launch |
| Fuel consumption | Reduce remaining fuel and total mass consistently; burning a tank dry preserves empty shell mass, while separate tank geometry remains unresolved |
| Tank removal | Setup removal clears the selected quantity, fuel capacity and shell mass together |
| Restart and multiplayer | Preserve accepted tank selections and starting fuel across restart, lobby edits, server validation and replay; seat handoff preserves current fuel and installed tanks |

Use source station compatibility and quantity limits. Do not make every tank
fit every plane. Keep tank shell mass separate from fuel mass. Preserve the
existing internal-fuel step unless a later UI decision changes it. Reject an
overweight or incompatible load with the same clear validation as weapons.

For an exact source-default GAS type, the station's source `maxItems` is its
allowed installed quantity. This is a **fitted default-capacity rule**, chosen by
the agent to preserve reviewed default loads. A4E supplies four F150 tanks and
F104 supplies two F250 tanks on one row, while the shared full-tank weight cap
would permit only two and one respectively. The [weight follow-up](../formats/aircraft-ordnance.md#quantity-weight-and-pod-follow-up)
confirms full tank weight and no pair multiplier in the reviewed capacity
branch. Original default-load normalization remains unknown. Alternative tank
types keep the shared compatibility and weight caps, and aircraft maximum
takeoff weight always applies. Review the initial/default loader before
changing this documented default rule.

Selecting a tank loads the station's allowed quantity, initially full. Minus or
right-click removes one tank, plus or clicking a loaded row adds one up to its
capacity. An emptied row retains its last selected type for convenient reloading;
zero quantity still means zero capacity, fuel and shell mass. Selecting a tank on
a shared weapon hardpoint clears its weapon quantity, and selecting a weapon
clears the tank quantity. A remotely supplied overlap is rejected. Unload All
and toggling Cheat clear both weapon and tank quantities. Cheat uses the shared
source station rules and never bypasses takeoff-weight validation.

New accepted loadouts explicitly list their tanks, including an empty list.
The mission text writes `loadout tanks explicit` and one
`loadout tank HARDPOINT GAS QUANTITY` line per installed row. A missing tank list
in an older saved mission preserves source-default tanks. Network protocol 7
carries the same optional list and exact quantities. Restart restores the
accepted load, while runtime fuel consumption leaves empty installed shells.

John limited the 2026-10-05 follow-up to the ordnance page and weapon
availability. Existing tank shape candidates may be recorded for later jettison
work; no aircraft rendering is added.

GAS supplies mass, capacity and compatibility flags but no shape pointer. This
pass adds no separate tank mesh or tank drag rule. Exact source tank geometry
and conditional attachment behavior remain unknown; the next research step is
to identify the original shape/loadout consumer. Source aircraft shapes remain
the visual inputs. In-flight tank/fuel jettison is not implemented by this pass.
Shift+J is confirmed research evidence but has no current runtime binding.
Shift+K remains selected weapon-store jettison.

`--loadout none` removes weapons and tanks together. `--loadout guns` removes
non-gun weapons while retaining the selected tank equipment.

## Remaining decisions and research

| Topic | Next step |
| --- | --- |
| F-14 live reproduction | Headless accepted-load checks pass; inspect the complete interactive editor-to-flight flow and source tank geometry separately |
| Default tank count and capacity | Source values and hashes are [reviewed](../formats/aircraft.md#f-14-default-external-fuel); next verify that the editor, accepted loadout and flight state agree with them |
| External fill level | Selected tanks start full; a separate partially filled tank control is deferred |
| Jettison versus dumping | Review Shift+J and current runtime handling; the [keyboard evidence](keyboard.md) calls it external fuel jettison, which alone does not establish shell removal |
| Visuals and drag | Review tank shapes, mounts and any implemented drag law; label a new drag rule fitted if source behavior is unknown |
| Interactive and platform validation | Check tank UI layout, multiplayer lobby interaction and controller operation on available hosts; source and synthetic validation do not imply retail parity |

## Acceptance matrix

| Scenario | Required evidence |
| --- | --- |
| F-14 with tanks removed | 0 lb external fuel at launch; correct internal fuel and mass; no residual tank store; same accepted configuration after restart |
| F-14 with source tanks | Reviewed quantity and capacity, correct loaded mass and visible source station in the editor; internal/external readouts agree with state |
| Empty installed tank | 0 lb remaining tank fuel while shell mass remains; aircraft rendering is unchanged |
| Burn, damage and jettison | Fuel never becomes negative or exceeds installed capacity; mass follows the specified equipment outcome; in-flight tank jettison remains deferred |
| Invalid loads | Compatibility and maximum takeoff weight enforced without changing the previously accepted load |
| Multiplayer and replay | Host/client configuration agrees; restart restores starting selections and handoff retains current state |
| Aircraft switching | No F-14 tank or fuel state leaks into another type; repeat on the F-4 family and an aircraft with no default tank |

Add a reproducible F-14 regression scenario to the battery and synthetic tests
for tank loading, fuel/mass accounting and serialization. Keep retail fixtures
local. Record validation together for this feature and update the
[ordnance presentation contract](ordnance-presentation.md) and
[import guide](../aircraft-import.md) when behavior changes.
