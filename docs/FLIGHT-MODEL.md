# Shared aircraft flight model

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The [F/A-XX concept](spec/fa-xx.md) shares the F-22 flight response in all
three adapters. Its split flap rudder animation represents the existing yaw
authority; separate differential drag and fin removal effects are not simulated.

`tore-sim` is a renderer-independent, deterministic 120 Hz flight kernel. The
working **hybrid** path combines recovered aircraft data and native helper rules
with fitted continuous dynamics where the original engine contract is still
incomplete. It is a usable free-flight model, not a reproduction of the original
program's internal trajectory arithmetic or a real-aircraft engineering model.

The opt-in **airborne native research** path now connects the joined native
service to the live loop for both aircraft. Inside that research path only, the
clock, device and fuel boundaries are explicitly authored and the flight stops at
unsupported contact; ordinary free flight has working authored ground contact.
This is separate from legacy and hybrid.
[Acceptance and limits](baselines/native-live-flight.md).

## Next scheduled work

Sequencing lives in [the parity plan](parity-plan.md). The
[environment/systems plan](research/native-environment-systems-plan.md) is a
frozen archive as of 2026-09-15: use it for its recovered contact, asset,
lifecycle/event and environmental research, not for sequencing. AI movement has
an [input-only controller](spec/ai.md#input-only-aircraft-control). The first
[cache/preference checkpoint](baselines/native-land-foundation.md) and
[vertical geometry checkpoint](baselines/native-land-geometry.md) are diagnostic
only; the restricted research path still stops at unsupported contact.
Maneuver audio/rumble and final
[flight-response acceptance](research/flight-response-plan.md) follow that continuation.
[Provenance policy](behavior-provenance.md) records where each component came
from, spec-derived, native, fitted or opinionated, and keeps user-directed
changes distinct. Those labels describe origin; none of them is an acceptance
gate. Lifecycle and contact work inside the research path continues.

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

The app defaults to the researched hybrid adapter, requested by John on
2026-09-16. `--researched-flight` remains an explicit alias; `--legacy-flight`
selects the previous compatibility model. Selection persists across new free
flights in that process. It also works
with `--headless-flight 7200 --maneuver loop`. Twelve aircraft have rendered initial ports; the
[roster guide](aircraft-import.md) lists their identities. Select Rafale with
`--aircraft rafale` or Quick Mission. Each uses its source-referenced cockpit family and
its own reviewed exterior device mapping. No F18.SH animation addresses are applied to another shape. The additional
models own their configurations and use the [documented shared fit](spec/additional-aircraft.md).
Select them with `--aircraft f14|a4e|x31`; see [validation](baselines/aircraft-fa-expansion.md).
The seven [roster additions](spec/roster-aircraft.md) own separate configurations
with the same shared fit and their own source roll rates. See their
[validation](baselines/aircraft-roster-expansion.md).

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
| Departure | Source warning/stall transitions and spin entry, with connected severity/control/lift attenuation. Initial stall classification uses a fitted below-clean-envelope gate. Hybrid spin torque, damping, surface response and threshold recovery are fitted; PT maximum yaw rate bounds angular velocity. The original two reviewed aircraft use spinExit −2; X-31 additionally disables spin entry. |
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
original-game trajectory oracle. Behavior a player would still find missing
includes carrier and arresting-gear dynamics and the remaining damage/equipment
state. Terrain and object contact is authored rather than recovered, and exact
integer update scheduling and RNG consumption order are original implementation
details rather than parity targets. Fitted behavior is localized in
`tore-sim`, separate from bounded readers and static native translations.

Current theaters supply height only through the app's existing callback; they do
not yet identify validated runway surfaces for the hybrid model. Consequently,
landing acceptance is demonstrated headlessly on explicit runways, and arbitrary
app terrain is not silently treated as a safe runway. That boundary is a
deliberate design choice (opinionated), not a hold waiting on recovered source
airfield/object collision mapping; ground, terrain and object contact was
reclassified opinionated on 2026-09-15.

See [acceptance evidence](baselines/shared-flight-model.md),
[native research](formats/native-flight.md), and [remaining work](research/progress.md).

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

## Complete typed model ownership, 2026-09-14

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
recovered departure/contact behavior. Hornet/Rafale legacy mode retains its
configured 1.8 rad/s fitted roll cap and legacy contact/drag treatment. The three
new ports use their PT roll response and low-speed auxiliary rotation in both
host adapters, as specified in [additional aircraft](spec/additional-aircraft.md).
Auxiliary rotation is separate from aerodynamic control and remains available
below stall speed when powered. It does not add lift or redirect thrust.

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
gates. F-14D, A-4E and X-31 now have initial FA-only ports; see
[acceptance](baselines/aircraft-fa-expansion.md).

## Autopilot

The [autopilot specification](spec/autopilot.md) defines heading/altitude hold
and waypoint guidance through ordinary stick commands at 120 Hz. Steering gains
are fitted, throttle remains manual, and mode changes are recorded in pilot
input tapes. The optional navigation target accepts a waypoint number and world X/Z in feet; routes
and target-selection recording remain future work. Flight adapters stay distinct.

## Flight response contracts, 2026-09-15

The adapter response pass has [component/regression evidence](baselines/flight-response.md);
native steps 2–3 remain open.
`State::g` / `AirData::load_factor_g` report aerodynamic specific force projected
on body-up, excluding gravity/contact. Internal filtered lift remains separate.
`State::maneuver` is the last authoritative fixed-tick snapshot: commanded G,
attenuated lift G, achieved G, applied body roll/pitch/yaw rates in rad/s, rudder
command/filtered deflection/effective control, optional departure mode and severity.
It is not interpolated by presentation; consumers must not combine it with a
render-interpolated AirData sample as if both represented the same tick.
The existing `roll_rate`/`pitch_rate` fields are control-response state; use the
snapshot for actual rotation, including alignment, ground rotation and blended spin motion.

Rudder yaw now consumes the fitted filtered deflection, preserving a smooth
release. Each aircraft's own tuning includes `sideslip_drag=0.5`: drag/weight is
that coefficient times squared lateral airspeed fraction times low-speed authority.
This symmetric continuous loss is authored, not the native display-slip drag law.
Roll retains its single response filter and existing source/hybrid versus fitted/
legacy cap. No new native rudder-to-roll law is asserted.

Hybrid spin entry retains source warning eligibility, direction selection and
the X-31 disable flag, with continuous fitted torque onset replacing the old
rudder/pitch switches. Speed deficit and back-stick smoothly increase driving
torque; opposite-rudder braking remains unchanged. Spin
motion and recovery use [input-driven angular dynamics](spec/spin-transitions.md):
continuous rudder torque, rotation/airflow-dependent control response, direct
proportional elevator pitch and aerodynamic damping. There is no timed spin
buildup or recovery ramp. Wrong rudder can increase rotation; early opposite
rudder can arrest it even below clean stall speed. In that case the spin clears
but the aircraft remains stalled. Normal flight requires sufficient speed,
airflow inside the 25-degree cone and residual yaw within normal rudder authority.
Small remaining angular velocity is retained and damped after clearance.

Analog input remains continuous through torque and elevator response, including
small deflections. Source direction selection still uses its integer input domain. Source recovery predicates remain unchanged in the restricted
research adapter. Hybrid normal stall classification remains a fitted
clean-envelope speed gate; severity and source warning timing are unchanged.

Native pitch/roll fall, tumble, full current-G/difficulty/device classification,
original spin movement/display composition and scheduling remain open. Legacy
still has fitted low-speed lift loss with no native warning/spin state machine;
its maneuver departure channel is `None`. This does not change adapter selection.

Run `cargo run --locked -p tore-sim --example response_probe -- PATH/F18.PT
PATH/RAFALE.PT` for both-adapter response/loop checks; set `TORE_RESPONSE_TRACE`
to an ignored local directory for per-tick evidence. The existing extraction
`--validate-flight` suite continues to cover both identities in hybrid mode.

### Provenance of the response pass

**Native:** source timer, attenuation and spin predicate arithmetic, with the
specific connections listed above. **Fitted:** response time constants, trim/
alignment, clean-envelope stall gate, severity reference speed, continuous spin
motion and `sideslip_drag=0.5`. These fitted choices were authored by the
implementation; they are not source facts or user-directed flight-law changes.
**Diagnostic instrumentation:** achieved-G and applied-body-rate snapshots.
**User-directed addition:** planned sustained rumble; native audio dispatch and
its future haptic mapping remain separate work.

Native tumble/fall/spin now has a joined diagnostic stage in
`tore-formats::flight_model::departure_stage`, with explicit envelope roles and
source movement composition. Both reviewed PTs pass its imported-table probes.
Neither live adapter calls it. The native
movement-state and whole-tick connection research steps remain open; see the
[source continuation](formats/native-flight.md#native-tumble-continuation-2026-09-15).

The [departure-stage evidence](baselines/native-departure-stage.md) and
[primary-control/movement evidence](baselines/native-movement-control.md) record
component checkpoints. The current `native_flight` diagnostic now feeds native
loaded controls, departure, force/velocity and movement/contact results back
into the next service for both PTs. It includes rudder/steering, auxiliary rates,
passive fall, sampled loading/damage/device effects and returned native events.
[Current acceptance and commands](baselines/native-flight-diagnostic.md).

The next continuation now provides restricted airborne live activation with
`--native-flight-tables DIR`, where DIR contains extracted sine/atan tables.
The translated control/departure/force/movement service is authoritative; the
existing host clock, input, device and fuel producers remain explicit adaptations.
Terrain contact stops this restricted research path and environmental turbulence
is disabled inside it; ordinary free flight has working authored ground contact.
Native query producers, engine/device/fuel/damage lifecycles, setup refresh cadence
and event execution remain open research items; scheduler and RNG ordering are
original implementation details, not parity targets. Legacy and hybrid retain
their existing behavior. [Live commands and validation](baselines/native-live-flight.md).
Retail comparison remains an unavailable evidence item, not an implementation
prerequisite. Audio/rumble follows the scheduled native work.

A-4 hybrid roll now uses the [90% reported Skyhawk peak target](spec/additional-aircraft.md#a-4-roll-tuning).
The evidence, variant limits, fitted acceleration and low-speed scaling are
specified there. Legacy A-4 retains the FA control values.

AI aircraft command stick, throttle and afterburner inputs into their own flight model.
Formation departures and rejoins use the [physical rejoin procedure](spec/ai.md#physical-departure-and-rejoin);
its clearance and braking predictions never override achieved motion.
That model alone advances attitude, velocity, position, fuel and telemetry;
there is no post-step AI movement override. Imported external stores contribute
payload mass and releases reduce it. The controller and its fitted limits are
specified in [input-only AI control](spec/ai.md#input-only-aircraft-control).
The three player adapters stay distinct. Exact input-replay validation is
recorded in the [AI baseline](baselines/ai-research.md).

## Creator ground initialization

The player ground-start path requires the existing researched adapter. It retains
selected fuel and payload, resolves a source runway departure point, and adds the
aircraft model's wheel/CG clearance to the shared runway surface height. All
velocity components and rotation rates start at zero; gear/flaps are fully deployed,
brakes applied and engine at idle. The adapter starts supported, so initialization
is not misclassified as a gear-up or hard touchdown. Legacy/native adapters reject
this setup rather than changing mode. Ground motion and takeoff then use the same
existing simulation. [UI and fitted defaults](spec/quick-mission-menu.md#player-ground-start).
