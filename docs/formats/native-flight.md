# Fighters Anthology native flight research

This is a static reverse-engineering pass against the local retail **FA** executable,
not a port of the reference application's custom flight engine. The Rust helper
translations are available in `tore-formats::flight_model`; the playable simulation
still uses the authored 120 Hz adapter. A full native flight tick is **not decoded**.
No imported executable, native module, or emulated native routine is run.

## Reproduce

From the repository root, with Python 3.10+, the pinned Rust toolchain and LLVM
`objdump` (included in macOS Command Line Tools):

```sh
python3 tools/extract_assets.py --native-flight --source gameassets/fighters-anthology --out .local/native-flight/repro --dry-run
python3 tools/extract_assets.py --native-flight --source gameassets/fighters-anthology --out .local/native-flight/repro
cargo run --locked -p tore-app -- --native-flight-report
```

The first two commands read `FA.EXE` and `FA.SMS` from the specified directory.
This research mode is separate from archive extraction; do not combine it with
asset selection switches. Ordinary menu/theater/aircraft extraction is unchanged.
The Rust report uses the already imported Hornet PT (normal first-run import rules
apply), needs no display, and prints deterministic helper results rather than a
simulated flight. `--help` lists it.

The script writes hashes, section bounds, 3,829 symbols, 107 selected symbol spans,
direct call addresses, and direct PT field references. Selection is a keyword
inventory, not proof that every flight dependency has been found. Spans end at the
next exported address and can contain unnamed helpers. Indirect calls, pointer
aliases and indexed field accesses need manual analysis. Absolute-address matches
can include non-memory constants; inspect the associated instruction.

Outputs are local research artifacts, not distributable assets. Repeating the same
command is accepted when contents match. Differing files require a new output
folder or `--overwrite`; all files are preflighted before writes. `objdump` version
or input-path changes can change textual output. Dry-run checks metadata and
prints provenance without invoking a disassembler or writing files. The standalone
`tools/extract_native_flight.py` exposes the same pass. Other PE32/i386 builds can
be inventoried, but build-specific PT annotations require both reviewed hashes:

- FA.EXE: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
- FA.SMS: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`

## State and units

`_cp` at `0x50ce80` and `_cpt` at `0x50d268` are global instance/type buffers,
not pointers. PT field offsets are derived from the shared packed Rust schema.
`_serviceTicks` at `0x546ba0` is read as a signed word; helper updates multiply
by it and arithmetic-shift eight places. Probe time uses the inferred fixed8
seconds contract; the FA timer scheduling path still needs a separate audit.
Do not equate a render frame or a 120 Hz tick with one native service tick.

The movement code keeps movement roll/pitch/heading separate from displayed body
angles. It adds load-related AoA, low-speed pitch, rudder slip, and turbulence in
different places. Native scalar speed/force and angle arithmetic do not constitute
a conventional lift/drag-coefficient aerodynamic polar. Applying a recovered AoA
number directly to the current velocity-alignment spring would combine different
models and is not a faithful integration.

## Translated helpers

All translations below are static instruction readings with synthetic regression
checks, **not differential execution against the original**. Arithmetic retains
signed truncation, fixed8 scales and explicit wrapping operations where reviewed.
Division faults and malformed envelope intersections become Rust errors.

| FA address | Rust helper / recovered behavior | Boundaries still outside helper |
| --- | --- | --- |
| `0x4119a0` | `match_f24`: rate-limited state approach | Native tick scheduling |
| `0x476950` | `stick_input`: asymmetric ranges, neutral return, reversal acceleration | Device input and damage-adjusted limits |
| `0x476880` | `low_speed_limit`: reduce excursion below twice clean stall speed | Selection of clean stall envelope |
| `0x476aa0` | `g_to_turn`: signed G/speed conversion, 125 fps floor, ±40°/s cap | Composition with gravity and attitude |
| `0x47c18c–0x47c1e5` | `pull_aoa`: load offset and slew | Stall/spin branches and movement/display composition |
| `0x476daf–0x476e58` | `low_speed_pitch`: fade low-speed movement offset | Ground state and final velocity rotation |
| `0x49d2d0`, `0x49d440` | `envelope_limits`, `interpolate`: authored ascending/descending chains, structural interpolation | G-envelope selection, header indices and damaged/load-adjusted limits |
| `0x49d230` | `envelope_class`: minimum/performance/structural speed classification | Native caller flags |
| `0x412780`, `0x47ab60` | `sound_speed`, `drag_percent`: altitude and transonic correction | Full force accumulation |
| `0x47a8c0` | `scalar_thrust`: throttle/scale/speed, zero thrust-vector angle | Engine/fuel gates and selected loaded thrust |
| `0x47a970` | `clean_drag`: clean force term with caller-supplied idle floor | Stores, G, rudder, devices, turbulence, ground drag |
| `0x451e50` | `fuel_rate`: military/afterburner threshold | BurnFuel tank selection and scheduling |

Pull AoA uses `gpullAOA × load / 9`, with fixed8 G. For positive G, load is
`max(G−1G, 0)`; for negative G it is G itself. The offset approaches a nonzero
target at 10°/s; return rate is at least 4°/s. The imported Hornet coefficient is
20. The separate low-speed offset uses `lowAOASpeed=70` fps and
`lowAOAPitch=15` degrees, fading above the clean stall boundary and toward vertical
movement. These are now linked to consumers, superseding the earlier note that
their units/consumers were entirely unresolved. The previous banked-pull adapter's
fitted AoA curve remains explicitly authored.

Envelope lookup uses the last highest-altitude point, then the authored chains;
it is not a generic closed-polygon intersection. Flaps lower the minimum speed
by one quarter for |G| ≤ 1. Structural speed interpolates between PT endpoints
through 36,000 ft. Above the envelope ceiling both performance bounds use the top
point's speed. Malformed/no-intersection cases are rejected, not approximated.

Fuel rate switches to the afterburner value above command 100; this alone does not
establish the actual burn timing or prove the UI's normalized throttle is the
native command. The drag consumer also exposes a missing factor in any simplified
weight-only device-drag approximation: the added coefficient sum is multiplied
by a weight term scaled by the speed-dependent drag percentage.

## Full model coverage and remaining steps

| Area | Static evidence | Remaining implementation/acceptance |
| --- | --- | --- |
| Main update | `_FMFlight` `0x47b020` located and normal-control/AoA sections traced | Translate complete branch/state graph and all callback contracts |
| Movement | `_MovePlane` `0x476ae0`; movement angles, gravity, display offsets, velocity and position stages identified | Translate fixed-point rotations, signed angle conversions, wind and collision; loops through both vertical attitudes |
| Setup/load | `_FMAircraftSetup` `0x47a690`, `_FMUpdatePlaneFields` `0x452140`, `_FMGetWeight` `0x4516b0` located | Trace stores/fuel/damage into weight, authority, thrust and drag |
| Power | `_FMGetAcc` `0x47a770` traced as a save/modify/query/restore routine | Complete live force integrator at `0x47c860`; do not call the query the live tick |
| Rudder | Normal branch `0x47c419–0x47c682` relates turn rate, authority, slip and bank | Translate all scaling/caller ranges and coupled turn acceptance |
| Stall/spin | PT fields and `_FMFlight` state branches identified | Warning/delay/severity, entry/exit, spin AoA/yaw/bank and recovery |
| Ground | `_GetGround` `0x47af20`, collision helper `0x477240` located | Contact, brakes, landing, crash, carrier/catapult and hook behavior |
| Turbulence | `_FMTurbulence` `0x477590` located | RNG/state contract, weather coupling and replay determinism |
| Devices | Gear/flap/brake/vector/fuel update symbols inventoried | Native actuator schedules and drag/lift coupling; visual fitted hinges remain separate |
| Integration | Reviewed pure helpers exposed through headless report | Native state struct, clock conversion, full update order and gameplay integration |
| Acceptance | Synthetic arithmetic and imported-data probes | Captured original-game trajectories; level, banked pull, negative G, stall/recovery, device transients, full loops |

The ignored reference checkout's `Docs/formats/native-flight-code.md`,
`native-power.md`, `native-performance.md`, and `flight-dynamics.md` helped locate
questions and candidate routines. Its USNF97 addresses and oracle claims are not
FA verification. Reproduce research from FA media and record mismatches explicitly.
