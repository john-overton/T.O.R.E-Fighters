# Wind, turbulence and vapor attachment acceptance

2026-09-15, Linux / NVIDIA RTX 4070 / Vulkan. Local logs, source-derived probes
and captures are ignored under `.local/wind-turbulence/`. No retail bytes are
committed.

## Outcome

Wind defaults now draw heading then speed (7..28 fps), retaining binary heading
and origin. Explicit calm remains calm. Launch uses an isolated seed of 1;
this is reproducible authored integration, not native global RNG scheduling.
Both adapters receive the same wind as live AirData; initial ground velocity
includes wind to preserve starting TAS. AirData explicitly samples terrain and
standard atmosphere; IAS/CAS/indicated and pressure altitude remain unavailable.

Turbulence now samples the recovered daytime flat-terrain class-1 gate. Bounded
wake strength accepts explicit nearby-aircraft geometry; the app supplies zero
because there are no qualifying flying neighbors. Body-axis rotation preserves
independent velocity through both vertical attitudes. **No turbulence?** disables
physical events and persists across flight restart.

CE attachments use right/up/forward, unlike mesh right/forward/up. Own-shape
neutral-vertex checks give:

| Aircraft | CE point | Mesh point | Distance |
| --- | --- | --- | --- |
| F18 | (-54, 1, -17) | (-54, -17, 1) | 0 ft |
| F18 | (55, 0, -16) | (55, -16, 0) | 0 ft |
| Rafale | (-54, -1, -30) | (-54, -30, -1) | 0 ft |
| Rafale | (54, -1, -30) | (54, -30, -1) | 0 ft |

The existing one-third-foot scale is retained. Heading hinges rotate right/forward
and preserve up. Rendered trail heads follow the interpolated aircraft pose;
authoritative trail history stays on fixed ticks. No F18C coordinates were used.

## Validation

- Formatting, Clippy with warnings denied, 298 Rust tests, locked workspace build,
  24 Python tests and repository/both-binary asset guards passed.
- Both aircraft passed the shared extraction `--validate-flight` suite.
- Synthetic tests cover wind draw order/calm, single wind advection in both
  adapters, AirData TAS, class-1 selection, wake range/speed/rear cone,
  daytime/night scaling, complete attitude loops, attachment attitude transforms,
  heading hinge axis and cheat/restart preservation.
- Full environment probes: 600 ticks at initial 100 ft AGL, each aircraft with
  and without `--researched-flight`, explicit calm versus `90,20`, turbulence
  disabled. Position differences were 99.999993 ft east and 0.038350 ft north,
  with unchanged altitude. The small north component reflects source degree
  conversion (182 binary units per degree) and the floating trig adapter.
- Enabling turbulence in matched calm probes changed final height by +1.282522 ft
  for F18 and -0.425203 ft for Rafale. This checks the live service, not retail
  response parity. The generated-wind probe resolved heading -13400, speed 8 fps.
- Creator and viewer window smoke tests passed. Both-aircraft before/after
  400-tick pull captures include the oblique exterior view and live Other View
  panel: F18 1280×720 and Rafale 640×900. Inspected corrected images show
  attached wing trails in main and panel views. GPU snapshots do not independently
  establish native trail lifetime, rasterization or sub-tick motion parity.

Reproduce integration and attachment checks with the commands in
[development](../DEVELOPMENT.md#wind-turbulence-and-attachment-probes). Flight
suites used `tools/extract_assets.py --source .local/weather-cameras/source
--aircraft f18|rafale --out .local/wind-turbulence/flight-IDENTITY
--validate-flight` with one literal aircraft per run. That local source contains
user-owned FA archives, not the unrelated unsupported LHX archive.

## Static source evidence

`python3 tools/extract_native_flight.py --domain weather --source
gameassets/fighters-anthology --out .local/wind-turbulence/native` passed both
source gates and emitted 85 symbol spans / 3829 symbols. Added reviewed regions
cover wind defaults/override/position and surface query/class production/store.
EXE SHA-256: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
SMS SHA-256: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`.
Imported native modules were not executed.

## Remaining limits

Native complete movement/display turbulence coupling, integer wake geometry,
cached/object/carrier collision producers, wind audio dispatch, serialized
environment replay and retail response comparison remain open. Surface class 1
is not a validated runway or a newly asserted water class. Exterior-store
placement is a separate open issue. Windows/macOS runtime checks were unavailable.

## Frame-time evidence

Matched debug F18 runs used explicit calm, 1280×720, instrument page 3,
`TORE_PERF_ACTIVE=1` and `TORE_PERF_FRAMES=330`, then 630 in reverse order.
Both exclude the first 30 frames. The before executable was copied before this
wind/turbulence/attachment slice, after the camera-weather slice.

| Frames | Build | Mean / p50 / p95 / max interval, ms |
| --- | --- | --- |
| 330 | Before | 1.51 / 1.38 / 1.55 / 11.67 |
| 330 | After | 1.67 / 1.37 / 2.62 / 11.76 |
| 630 | Before | 2.17 / 1.45 / 11.53 / 11.90 |
| 630 | After | 2.16 / 1.44 / 11.52 / 12.13 |

No paused frames; 6 completed asynchronous camera readbacks per short run and
14 per long run. Long-run simulation/camera mean was 0.19 ms in both builds.
The short-run tail difference prompted the longer reverse-order comparison;
these samples establish no clear sustained regression. These are CPU wall
intervals including presentation backpressure, not GPU timing or displayed FPS.
