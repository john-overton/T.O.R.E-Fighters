# Shared F/A-18D and Rafale C flight model

`tore-sim` is a renderer-independent, deterministic 120 Hz flight kernel. The
working **hybrid** path combines recovered aircraft data and native helper rules
with fitted continuous dynamics where the original engine contract is still
incomplete. It is a usable free-flight model, not a claim of byte-for-byte native
trajectory parity or a real-aircraft engineering model.

## Next scheduled work

The [flight response and maneuver buffet plan](flight-response-plan.md) orders
remaining G-load, roll-rate, rudder and departure work, followed by sustained
maneuver rumble and verified original audio. This slice precedes further weather
work. Existing implementations below remain partial/fitted as documented.

## Run and reproduce

Use the installed local media through the portable extraction entry point:

```sh
python3 tools/extract_assets.py --aircraft f18 --exclude-archive 'disc1/LHX/*' --out .local/aircraft/f18 --validate-flight
python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc1/LHX/*' --out .local/aircraft/rafale --validate-flight
cargo run --locked -p tore-app -- --free-flight --researched-flight
```

`--validate-flight` extracts the selected aircraft, hashes source/output files,
and runs the same headless suite against each distinct extracted PT version.
It requires a full aircraft extraction, not `--include`, preview or
`--native-flight`. A failed extraction or scenario returns a nonzero exit code.
The source-specific LHX exclusion skips unrelated bundled archives; omit it for
media without that directory. See [EXTRACTION](EXTRACTION.md) for source paths.

The app still defaults to its legacy adapter. `--researched-flight` selects the
hybrid model and persists across new free flights in that process. It also works
with `--headless-flight 7200 --maneuver loop`. Both F/A-18D and Rafale C are rendered aircraft; select Rafale with
`--aircraft rafale` or Quick Mission. Each uses its own original cockpit and
separate fitted animation rig. No F18.SH animation addresses are applied to RAF.SH.

For already-extracted files:

```sh
cargo run --locked -p tore-sim --example flight_suite -- .local/aircraft/f18/FA_2.LIB/F18.PT .local/aircraft/rafale/FA_2.LIB/RAFALE.PT
```

The suite exercises 13 scenarios per aircraft: level, loop, left/right banked
pull, stall, spin entry/recovery, approach landing, gear-up impact, braking roll,
controlled takeoff, constant wind, water impact and hard landing. It uses an
explicit flat runway, not a fabricated theater runway. Every step is replayed
from cloned initial state and checked for equality/finite motion. Additional
checks cover full loops, bank symmetry/AoA, wind advection, departure/recovery,
contact outcomes and fuel consumption. Synthetic unit tests cover pause/render
rate independence, wind, mass validation and ground behavior without retail data.

## Data and implementation boundaries

| Component | Runtime behavior and provenance |
| --- | --- |
| Aircraft selection | Reviewed F18.PT (F/A-18D) and RAFALE.PT (Rafale C), both FA type 5 / size 660. Other identities remain rejected, including RAFALEE/RAFALEF and F18C. |
| Envelopes and mass | Original G polygons, empty weight, internal fuel, military/AB thrust, consumption and drag/loading fields. Scalars resolve once when creating state; envelope intersection no longer allocates per update. |
| Attitude and momentum | Shared orthonormal basis, independent velocity and nose direction, full vertical/inverted flight. Float integration, aerodynamic alignment, trim AoA, atmosphere lapse, response and drag normalization are fitted. |
| Controls | Original roll-rate maximum in the hybrid path; G authority from aircraft envelopes/loading. Response filtering, pitch/yaw coupling and actuator travel are fitted. |
| Departure | Native warning/stall transitions and spin entry/recovery predicates with explicit clock/RNG. Initial stall classification uses a fitted below-clean-envelope gate. Spin yaw range/intensity comes from PT/native rate arithmetic; continuous spin force/attitude coupling is fitted. The two reviewed aircraft use spinExit −2. |
| Propulsion/fuel/devices | Per-aircraft military/AB thrust and fuel consumption; engine/fuel/throttle gates; gear, flaps, brake, hook and burner state. Lapse, exhaust ramp and three-second actuator travel remain fitted. |
| Ground contact | Native landing limits classify touchdown. Hybrid accepts only caller-declared landable ground with gear deployed and within limits; water and unsafe touchdowns crash. Eight-foot CG clearance, flat-runway tire scrub, rolling/brake friction, pitch support and crash severity policy are fitted. |
| Wind | Explicit world wind in ft/s via `research::Surface`; aerodynamic forces use air-relative velocity, position uses ground velocity. Synthetic advection test holds airspeed unchanged and checks 400-foot drift over ten seconds at 40 ft/s. |
| Payload | `State::set_payload` validates finite, nonnegative mass against current fuel + empty weight and MTOW. Added mass affects loading/forces. This is mass-only: UI loadout, station release, external fuel transfer and store-specific drag are not implemented. |
| Time/replay | Fixed 120 Hz simulation; authored fractional bridge to native 256-unit time; seeded native RNG helper. Same-host deterministic replay is tested. Original scheduler/global RNG draw ordering and cross-architecture bit identity are not established. |

The legacy path remains available for comparison. Native integer matrix, force,
contact-query and scheduler research remains in `tore-formats::flight_model`;
those helpers are not falsely presented as a complete native integrator.

## What “working” covers

Both aircraft complete the same tested free-flight and ground scenarios using
their own extracted data. The suite is an engineering regression gate, not an
original-game trajectory oracle. To claim native parity still requires original
trajectory comparison, terrain/object collision geometry and cache producers,
carrier/arresting dynamics, remaining damage/equipment state, exact integer
update scheduling and RNG consumption order. Fitted behavior is localized in
`tore-sim`, separate from bounded readers and static native translations.

Current theaters supply height only through the app's existing callback; they do
not yet identify validated runway surfaces for the hybrid model. Consequently,
landing acceptance is demonstrated headlessly on explicit runways, and arbitrary
app terrain is not silently treated as a safe runway. This boundary is intentional
until source airfield/object collision mapping is ported.

See [acceptance evidence](baselines/shared-flight-model.md),
[native research](formats/native-flight.md), and [remaining work](progress.md).

## Independent aircraft models and future gauges

F/A-18D and Rafale C now own separate `models/f18.rs` and `models/rafale_c.rs`
implementations of `FlightModel`. `AircraftModel` dispatches by reviewed aircraft
identity, and each instance owns its full validated `Configuration` (including `Tuning`). Trim/AoA and thrust-lapse
laws are implemented separately; roll/pitch response, alignment, rudder and tire
coefficients are obtained from the selected model. Original PT envelopes, thrust,
mass, fuel and departure data remain aircraft-specific. Initial fitted numbers
preserve the previous tested baseline; identical initial coefficients do not
establish that the real aircraft behave alike. Changing one model's tuning does
not mutate the other. Common numerical integration, vector math and recovered
helper algorithms remain reusable components.

For code mods, copy a model module, implement `FlightModel`, add a registry/enum
entry, and review the new aircraft's PT identity/schema and extraction roots.
Use `AircraftModel::set_configuration` for complete per-instance edits or
`set_tuning` for fitted coefficients alone. Both validate before replacement. There is no arbitrary-code
plugin loader or external tuning-file format yet. Adding a new native layout
still requires bounded-reader review and its own acceptance baseline.

`telemetry::AirData::sample` provides a gauge-independent snapshot from flight
state and explicit terrain, wind, temperature and static pressure:

- TAS (3D air-relative speed), horizontal ground speed, EAS, Mach and dynamic pressure.
- Geometric MSL altitude, terrain-relative AGL, vertical speed, true heading,
  attitude, AoA, sideslip and load factor, with units in field names.
- ASL is explicitly an alias for the same mean-sea-level datum here, not another
  independently integrated height. AGL can be negative for below-terrain diagnostics.
- IAS, CAS, pressure altitude and indicated/barometric altitude are `Option`
  channels currently unavailable. They are not filled with TAS or geometric MSL.
  Pitot/static calibration, pressure setting, instrument error/lag and failures
  belong in future sensor components. AoA/sideslip are unavailable at zero airspeed.

Steam gauges can consume this snapshot without reading aircraft-specific structs.
The HUD now consumes the shared sample for TAS, AGL and vertical speed using
explicit wind, terrain and standard atmosphere; other panel channels retain
existing integration. This is not a fully modeled pitot/static system. Gauge animation/filtering must not modify authoritative flight state.

The optional engineering atmosphere helper follows the published
[NASA Glenn three-zone atmosphere approximation](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/earth-atmosphere-equation-metric/).
Mach uses temperature-dependent sound speed, following
[NASA's speed-of-sound relation](https://www.grc.nasa.gov/WWW/k-12/VirtualAero/BottleRocket/airplane/sound.html).
This telemetry atmosphere is separate from aircraft-specific fitted engine lapse;
it does not silently replace retail weather or claim precision ISA altimetry.

## Complete typed model ownership — 2026-09-14

`models/f18.rs` and `models/rafale_c.rs` each construct and own a validated,
immutable `Arc<Configuration>` from their own reviewed PT. The shared type and
import mapping live in `models/config.rs`; sharing a schema does not share aircraft
values. No retail values are embedded in code. Missing required fields fail import
instead of silently becoming zero. There is no longer a string-indexed parameter
cache or a second copy of aircraft limits in `Research`.

| Configuration group | Contents / runtime use |
| --- | --- |
| `mass` | Empty weight, internal fuel capacity, maximum takeoff weight; initial fuel, force acceleration, loading and payload validation |
| `propulsion` | Military/afterburner thrust and fuel rates; force and consumption calculations |
| `aerodynamics` | Altitude/speed/G polygons, loaded drag/elevator factors, G drag, source roll limit |
| `native` | Departure/spin parameters, landing limits, device drag, velocity limits and extended-warning flag |
| `equipment` | Fitted gear/flap/brake/hook travel, exhaust and control response, throttle response/AB threshold, ground clearance |
| `tuning` | Aircraft-owned fitted trim, thrust lapse, roll/pitch response, alignment, rudder and tire/braking coefficients |

Both legacy and research modes consume this configuration. Research mode adds
recovered departure/contact behavior; legacy mode retains its configured 1.8 rad/s fitted roll cap
and legacy contact/drag treatment. Existing fitted coefficients and source data
were retained, preserving the validation baseline.

Configure before creating a flight (this Rust API assumes a parsed `Aircraft`):

```rust
use tore_sim::{flight::State, models::{AircraftModel, FlightModel}};

let mut model = AircraftModel::for_aircraft(&aircraft)?;
let mut config = model.configuration().clone();
config.mass.max_takeoff_lbs += 100.; // Example code mod, not a source-data correction.
model.set_configuration(config)?;
let mut flight = State::from_model(model, [0., 5000., 0.]);
flight.enable_research(1)?; // Optional; seed must satisfy NativeRng's contract.
flight.step(&keys, |_, _| 0.);
```

`State::new(&aircraft, position)` is the fallible import-and-start convenience API.
The app validates its Hornet model at asset load and clones it for each free flight.
Updates no longer accept another `Aircraft`; research activation also uses the
already selected model. Read it through `flight.model().configuration()`.
Configurations are readonly once attached to a flight. Clone/edit/validate a model
before starting a new flight, avoiding inconsistent mid-flight fuel/load limits.
Cloning models and presentation states shares immutable data without copying
polygons; replacement gives only the edited model a new allocation.

Live fuel, payload, actuator positions, velocity, attitude, stall/spin progression,
clock and RNG belong to `State`/`Research`, not configuration. Environment inputs
remain explicit. Telemetry and future gauges keep their independent typed API.

“Complete” here means the configuration used by the current adapter plus the
already recovered native profile. This refactor adds no new native decoding.
Native velocity limits and rudder/bay/wheel drag are retained for diagnostic
helpers; the continuous adapter still uses fitted force/contact coupling. Other
unresolved PT fields remain in raw imported `Aircraft` data for research, including
unimplemented equipment, damage and loadout behavior. It is not whole-game or
whole-tick parity. External mod-file serialization/loading remains future work.

## Aircraft import and expansion

The [aircraft import and acceptance guide](aircraft-import.md) joins extraction,
existing flight/presentation/systems coverage and all per-aircraft acceptance
gates. F-14, A-4E and X-31 are scheduled after the flight-response slice; they
are not supported identities yet.
