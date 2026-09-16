# Ordnance planning review — 2026-09-14

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


Reviewed the [user-linked retail screenshot](https://www.old-games.com/screenshot/6252-5-jane-s-fighters-anthology.jpg)
and local F-14D ordnance photo. The linked image shows an F-22A with a two-column,
eight-card catalog, station groups, fuel/weight controls and Select Plane/Fly.
It was downloaded to ignored `.local/quick-mission-plan/ordnance-reference.jpg`.
The local photo shows the same composition with different aircraft/stations.

## Static code evidence

Same FA.EXE hash as [creator research](quick-mission-research.md). Reviewed existing
ignored disassembly in `.local/systems-pass/native/fa-disassembly.txt` and helper
regions, alongside `tore-sim::combat::loading` and app `combat.rs`.
Addresses here are virtual addresses. No native code was executed.

| Evidence | Finding |
| --- | --- |
| Symbol inventory, `0x4197d0` | `ArmPlane` entry; screen caller contract still needs full trace |
| `0x4199b7` | References LOADORD string before dialog setup call |
| `0x41aa0c` | References loaded/max-count format used by station presentation |
| `0x419d30`, `0x41c3f2` | Screen-region calls to HARDCanLoad |
| `0x41b709`, `0x41b7c3` | Screen-region calls to HARDLoad |
| `0x41b2d6–0x41b2f8` | Compares weight values and branches to overweight message; complete event/return flow remains unverified |
| `0x452c20–0x452d06` | HARDLoad resolves station, unloads, resolves requested store; zero requested count calls HARDCanLoad, then installs store/count and initializes type-specific state |
| `0x452c60–0x452c6b` | Nonzero requested count bypasses this helper's automatic capacity calculation; callers must be traced and port input validation must remain explicit |
| `0x452940`, `0x452980` | Existing StoreWeight and HARDCanLoad research/typed translations |

Adjacent EXE strings identify Wingtip/Wing/Internal Bay/Internal Gun/Fuselage/
Centerline labels; count and supply text; FNTWPNB.PIC/FNTWPNY.PIC, ARMFONT.PIC,
SMLFONT.PIC and ^NOPIC.PIC; weapon/ammunition/fuel cue references. These are leads
for complete art/label/callback resolution, not a finished display specification.

`HARDLoad` also contains native type-specific initialization and an RNG call;
this planning review does not establish translated whole-load or replay parity.
Existing `Station::allowed_count` covers fixed/default exceptions, store-type
masks and weight-class capacity, explicitly excluding full stock/year rules.

## Extraction

```sh
python3 tools/extract_assets.py --include 'ORD*.PIC' --include '*WEAP*.PIC' --include '*ORD*.MNU' --include 'LOADORD.DLG' --exclude-archive 'disc1/LHX/*' --exclude-archive 'disc1/WB/*' --out .local/ordnance-plan
```

Two resources extracted with zero errors and SHA-256 provenance. This narrow
filename search is not the complete screen dependency closure. LOADORD.DLG
contains Select Plane and imported DrawAction/DrawDial/DrawRocker symbols;
geometry and callbacks still require bounded record/handler recovery.

This pass changes planning documentation only. No new screen, loadout runtime,
native execution, GPU acceptance or full workspace test run is claimed.
Implementation and validation gates are in the [ordnance plan](../research/ordnance-plan.md).
