# Airborne native research flight connection

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


2026-09-15. F18.PT (F/A-18D) and RAFALE.PT (Rafale C). This is a **restricted
live research connection**, not complete native flight/system or retail parity.
It supersedes the diagnostic-only runtime status of the
[joined service checkpoint](native-flight-diagnostic.md). Its arithmetic source
and executable identities remain in [native flight research](../formats/native-flight.md).

## Run

Extract tables from user-owned media using the static extraction pass, then:

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-flight-diagnostic/native
cargo run --locked -p tore-app -- --free-flight --aircraft f18 --native-flight-tables .local/native-flight-diagnostic/native/tables --no-audio
cargo run --locked -p tore-app -- --free-flight --aircraft rafale --native-flight-tables .local/native-flight-diagnostic/native/tables --no-audio
```

The directory must contain exactly sized `sine-q15.bin` and `atan-pa.bin` tables.
Reads are bounded; native modules are never executed. Legacy remains default;
`--researched-flight` remains the existing hybrid. It cannot be combined with
this option. Native research currently requires clean free flight, without the
combat range or quick-mission lifecycle.

## Connected versus adapted

**Native translations now runtime connected:** loaded control/drag consumers,
normal G/pitch/roll/rudder response, auxiliary rates, warning/stall/spin/tumble,
force/velocity and movement/display composition. The same joined service owns
movement, body rates, departure state, contact caches and RNG between ticks.
Host position, velocity and cockpit attitude are output projections, not inputs
fed back into each subsequent native tick. Signed bank conversion is explicit.

**Host adaptations retained:** the 120 Hz remainder clock, initial airborne pose,
input quantization, separate seeded RNG, existing throttle/device animations,
engine switch and fuel-burn laws. Device fractions become native booleans at 0.5;
that threshold is an authored bridge, not recovered actuator timing. Horizontal
wind is quantized to the native whole-fps/PA interface and advected once.
Maneuver `commanded_g` exposes native stored control G on this path; achieved G
is a host acceleration projection over the actual native service interval.
Lift G is separately derived from native fixed8 lift force and weight.

Configuration resolves PT fields once within the aircraft-owned configuration.
An incomplete native subset retains an explicit construction error and cannot
activate; it does not silently default missing fields. Edits to shared source
fields which diverge from the resolved native snapshot also prevent activation.
Fitted tuning remains separately owned by the F18/Rafale model modules.

**Restricted/external:** native terrain/object/carrier producers, setup refresh
cadence, damage and engine/device/fuel lifecycles, control-disturbance request
producers, environmental turbulence and event execution. The source turbulence
bypass is forced and the existing fitted turbulence application is suppressed;
the menu reports the restriction. Departure/contact/high-G events are retained
for consumers, but this pass does not invent sound/damage/ejection dispatch.

Terrain heights only detect entry into an unsupported contact branch. They are
not promoted to native runway or slope queries. Contact within the reviewed
one-foot tolerance returns an explicit error, preserves the last valid host and
native state (including fuel/devices/clock/RNG), and pauses the window. Restart
creates fresh state. Headless runs fail with a nonzero exit code. This does not
implement takeoff, landing, ground steering or carrier operation in the live mode.

## Acceptance

- `native_live` example: 14 cases per aircraft, 1,200 ticks each; **28 cases and
  33,600 updates**, each compared with replay through the public `flight::State`
  API. Cases include level/pull/push, left/right roll/rudder with release, stall and left/right spin entry plus recovery,
  initial near-vertical up/down, crosswind and device movement. All pass; each
  case checks fresh restart and the 120 Hz-to-256-unit clock.
- Synthetic tests cover native/host coordinate mapping at 45 combinations of
  heading, pitch and bank (including inverted attitudes); late contact failure
  rolls back host equipment and native RNG/clock; presentation reads do not
  advance state; incomplete/edited native configurations reject activation.
- Legacy/hybrid `response_probe` regressions pass for both PTs. CLI checks reject
  conflicting modes and stop a low-altitude roll run at unsupported contact.
- Actual app headless full-loop probes pass: F18 completes the harness loop at
  tick **1,861**, Rafale at **1,639**, both with vertical and inverted flags set.
  These are host harness observations, not retail reference trajectories.
- Linux / NVIDIA RTX 4070 / Vulkan: both aircraft pass 120 active frames at
  1280×720 with cycling views, mirrors and camera instrument page 3. Each records
  zero paused frames, three completed camera readbacks and 60 mirror renders.
  F18 frame interval mean/p95: 2.30/11.49 ms; simulation/cameras mean 0.24 ms.
  Rafale: 2.45/11.48 ms; simulation/cameras mean 0.22 ms. These short CPU wall-time
  samples include presentation backpressure; no displayed-FPS/GPU timing claim.
- Both cockpit captures after 600 pull ticks were inspected: original cockpit,
  HUD, independent instrument overlays and mirrors remain composed. Creator and
  viewer smoke tests pass. No composition geometry changed.
- Formatting, warnings-denied workspace Clippy, 336 locked Rust tests/build, 24 Python
  tests and repo/app/extractor asset guards pass. Windows/macOS, manual controller
  handling, original audio and retail trajectory comparison were not validated.

Local logs/captures: `.local/native-flight-diagnostic/live-*`. No retail images
or generated derivatives are committed.

```sh
cargo run --locked -p tore-sim --example native_live -- .local/native-flight-diagnostic/native/tables/sine-q15.bin .local/native-flight-diagnostic/native/tables/atan-pa.bin .local/flight-response/validated-f18/FA_2.LIB/F18.PT .local/flight-response/validated-rafale/FA_2.LIB/RAFALE.PT
target/debug/tore-app --aircraft f18 --headless-flight 7200 --maneuver loop --native-flight-tables .local/native-flight-diagnostic/native/tables --no-audio
TORE_PERF_FRAMES=120 TORE_PERF_ACTIVE=1 TORE_PERF_VIEWS=1 target/debug/tore-app --free-flight --aircraft f18 --native-flight-tables .local/native-flight-diagnostic/native/tables --instrument-page 3 --window-size 1280x720 --no-audio
```

Substitute `rafale` for the second aircraft. The existing `--validate-flight`
importer suite still covers hybrid; this example and app probes separately cover
the new restricted native path.
