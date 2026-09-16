# Shared F/A-18D / Rafale C hybrid acceptance — 2026-09-13

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


Host: macOS arm64, Apple M3, pinned Rust 1.91.1. No imported executable was run.
This baseline covers the working **hybrid free-flight model**, not original-game
trajectory parity. See [component provenance](../FLIGHT-MODEL.md).

## Reproducible extraction and flight

Both commands passed using the local loose archives/retail discs:

```sh
python3 tools/extract_assets.py --aircraft f18 --exclude-archive 'disc1/LHX/*' --out .local/native-flight/f18-validated --validate-flight
python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc1/LHX/*' --out .local/native-flight/rafale-validated --validate-flight
```

F18 extraction: **101 resources, zero errors**. Rafale: **99 resources, zero
errors**. Each output report contains archive/output SHA-256 provenance and
named PT fields, envelope polygons, hardpoints and dependencies. Both aircraft
use the same schema/parser and simulator; no per-aircraft engine fork was added.

| Extracted fact | F18.PT | RAFALE.PT |
| --- | --- | --- |
| Retail identity | F/A-18D | Rafale C |
| Empty weight, lb | 23,050 | 17,100 |
| Internal fuel, lb | 11,220 | 9,900 |
| Military thrust, lbf | 17,687 | 24,000 |
| AB thrust, lbf | 32,000 | 32,000 |
| Native spinEntry / spinExit | 0 / −2 | 1 / −2 |

The 13-scenario suite passed for each aircraft (26 total), with per-step replay
equality and finite position/velocity. Most cases run 10,800 ticks (90 seconds);
loops stop on completion, impact cases on the expected crash. These are supplied
scenario inputs on an explicit flat runway, not observed native trajectories.

| Scenario/result | F/A-18D | Rafale C |
| --- | --- | --- |
| Loop from 15,000 ft, full AB | 3,615 ticks; 14,493.32 ft; no crash | 2,866 ticks; 14,160.17 ft; no crash |
| Spin with recovery inputs after 12 s | Entered and recovered; no crash | Entered and recovered; no crash |
| Approach landing, gear down | Settled at 8 ft CG clearance, stopped | Settled at 8 ft CG clearance, stopped |
| Controlled takeoff/climb at 90 s | 10,928.77 ft; no crash | 13,713.25 ft; no crash |
| Gear-up / water / hard impact | All rejected with crash | All rejected with crash |
| Left/right banked pulls | Symmetric lateral displacement; nonzero AoA | Symmetric lateral displacement; nonzero AoA |
| 40 ft/s constant wind | 3,600 ft extra drift; unchanged TAS | 3,600 ft extra drift; unchanged TAS |

The app's Hornet hybrid loop from its usual 5,000 ft setup also passed through
vertical and inverted attitudes: 2,590 ticks, 427.958 kt, 4,971.080 ft, no crash.
The shared core retains legacy regression tests; animation-geometry tests remain
in the app after moving state/attitude code to `tore-sim`.

## Checks and presentation

- **112 Rust tests**, **11 Python tests**, formatting, all-target Clippy with
  warnings denied, and workspace build passed.
- Synthetic hybrid tests verify 30/60/144 Hz scheduling equality, presentation
  isolation, wind advection, payload bounds/effect, and nonreversing runway roll.
- Python guards reject incomplete validation selections and unreviewed Rafale
  variant aliases before starting Cargo/extraction.
- Asset guards passed for repository files, debug app/extractor and the release
  headless suite. Generated media remains ignored. `git diff --check` passed.
- Active Metal view-cycle run: 180 frames, first 30 excluded, **zero paused
  frames**. Mean CPU frame interval 16.68 ms (p95 17.48, max 24.35); simulation/
  camera work mean 0.16 ms (p95 0.52, max 0.59). These are CPU wall times including
  compositor backpressure, not GPU timestamps or certified displayed FPS.
- Linux/Windows execution and manual real-pilot handling comparison were not
  available. No real-aircraft or exact original-game fidelity claim follows
  from these regression checks.

Evidence in ignored `.local/native-flight/`: `f18-validated.log`,
`rafale-validated.log`, `flight-suite.txt`, `shared-sim-tests.log`,
`hybrid-app-loop.txt`, `hybrid-gpu-views.log`, plus extraction reports under each
validated output directory.

Remaining parity gates: original trajectories, source airfield/collision/cache
producers, carrier/arrestor dynamics, damage and external store/fuel systems,
and exact native scheduling/RNG consumption. Rafale visual/cockpit/animation
porting remains separate. The app only declares ordinary theater height samples,
so safe runway contact is currently demonstrated through the headless surface API.

## Independent models and telemetry follow-up — 2026-09-14

F18FlightModel and RafaleCFlightModel now own separate fitted-law implementations
and per-instance tuning. The previous coefficients were retained to isolate the
architectural change from unvalidated retuning. Both extracted aircraft passed
the 26-scenario suite unchanged (`separate-models-suite.log`).

114 Rust tests passed, including independent model tuning and telemetry checks
for wind/TAS/ground speed, altitude datums, density-dependent EAS, Mach, zero-speed
AoA availability, and invalid atmosphere input. Formatting, Clippy, workspace
build and asset guards passed. Evidence: `.local/native-flight/separate-models-tests.log`.
No rendering behavior changed in this follow-up; GPU checks were not repeated.
IAS/CAS and pressure/barometric altitude remain unavailable sensor channels;
telemetry atmosphere is an engineering approximation, not native weather parity.

## Complete typed configuration follow-up — 2026-09-14

Each aircraft model now owns mass, propulsion, envelope/loading, native
limits, fitted equipment response and tuning in a validated `Configuration`.
The string scalar cache was removed. Research contains evolving state only;
flight updates and research activation cannot accept a second aircraft profile.
Immutable configuration sharing preserves cheap presentation/state clones.

117 Rust tests and 11 Python tests passed. New synthetic tests cover independent
configuration edits, invalid/missing fields, mass/payload bounds, thrust/fuel/device
response in both modes, and modified landing limits reaching contact handling.
Formatting, Clippy with warnings denied, workspace build and repository/binary
asset guards passed.

The 26 reported F18/Rafale scenario rows match `separate-models-suite.log`
exactly. Each scenario also checks deterministic replay at every tick. This is
agreement with the existing adapter baseline, not native trajectory verification.
The app's researched Hornet loop still completes in 2,590 ticks at 427.958 kt,
4,971.080 ft and 10,874.667 lb fuel. No renderer changed; GPU captures were not
repeated. Other operating systems were not exercised on this macOS host.

Evidence in `.local/native-flight/`: `typed-model-tests.log`,
`typed-model-clippy.log`, `typed-model-build.log`, `typed-model-suite.log`,
`typed-model-app.log`. Unresolved native fields/systems and external mod-file
loading remain open; see the ownership/API guide in [FLIGHT-MODEL.md](../FLIGHT-MODEL.md).
