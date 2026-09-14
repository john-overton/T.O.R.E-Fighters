# Shared controller input acceptance — 2026-09-14

## Outcome and provenance

The hand-rolled input slice adds dependency-free `tore-input` and the isolated
`tore-input-native` platform boundary. `tore-sim` now takes typed pilot frames,
including fractional axes and ordered equipment/throttle commands at tick entry.
Keyboard defaults and model-owned response are preserved. The app adds shared
assignments, persistent profiles, calibration, throttle pickup, release/context
isolation, instrument selection using stock controls, and opt-in bounded rumble.
These are authored T.O.R.E behaviors, not decoded FA controller dispatch or native
whole-flight parity. No screen export, new instrument raster or sensor behavior
was added. [Contract and user setup](../INPUT.md).

## Hardware and native checks

Host: Linux x86_64, Rust 1.91.1, Wayland, NVIDIA RTX 4070/Vulkan, Immediate
presentation. The user's 8BitDo Ultimate 2 was initially powered off: a receiver
HID interface existed without a controller event node. After the user powered it
on, the new native diagnostic enumerated the controller through evdev:

- Vendor/product `2dc8:310b`, name `8BitDo Ultimate 2 Wireless Controller`.
- Eleven button inputs: 304, 305, 307, 308, 310, 311, 314–318.
- Stick axes 0/1/3/4: -32768..32767; trigger axes 2/5: 0..255.
- Directional axes 16/17: -1..1.
- FF_RUMBLE capability advertised. USB serial plus interface used for identity;
  actual serial/connection details are retained only in ignored local logs.
- The generated profile loaded in a real flight-window smoke test. Automatic
  standard Linux bindings also initialized in active flight benchmarks.

This confirms detection, capability reads, profile generation/loading, and startup
integration. The user subsequently ran the explicit pulse diagnostic and confirmed
that physical rumble works on the Ultimate 2. Axis/button flight handling and manual
disconnect behavior remain user acceptance checks.
No rumble test was triggered during automation. Keyboard consumer-control and
touch interfaces are excluded from controller handling; generic button-only
interfaces receive no automatic gamepad mapping.

Windows and macOS backend crates pass target-specific Clippy with warnings denied.
Windows uses RawGameController/current-reading polling and Gamepad vibration;
macOS uses GameController/CoreHaptics for supported gamepads and HID queues for
generic devices without haptics. These were
cross-checked from Linux, not linked/run as complete applications on those hosts.
No flight stick, throttle, pedals or MFD/button-box hardware was available.

## Automated validation

Passed:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 -m unittest discover -s tools -p 'test_*.py'
python3 tools/check_assets.py
python3 tools/check_assets.py target/debug/tore-app
python3 tools/check_assets.py target/debug/tore-extract
cargo clippy -p tore-input-native --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
cargo clippy -p tore-input-native --all-targets --locked --target aarch64-apple-darwin -- -D warnings
```

150 Rust tests and 11 Python tests pass. Synthetic input coverage includes short
press/release ordering, repeat suppression, source-independent holds, on-change
versus following switches, disconnect/reconnect baselines, pause/focus rearming,
neutral controls on first movement after resume, priority/stable axis ownership,
paired triggers, throttle crossing/preset pickup, encoder counts, discrete
positions, large button boxes, aliases, malformed/nonfinite input, bounded queues,
and modifier release ownership. App bridge tests disable native hardware access.
Stock instrument operations and unavailable controls are exercised without assets
or a GPU.

Pilot tapes round-trip fractional values and ordered commands. A varying 360-tick
synthetic tape yields identical flight states at 30, 60 and 144 Hz render schedules.
An 81-frame pilot tape recorded from a real window also loaded and ran through the
headless theater replay path. Tapes record inputs, not initial-state/mission saves;
reproduction requires matching aircraft, theater, model, assets and configuration.

The same shared flight-suite implementation used by `--validate-flight` passed
all 26 cases across the reviewed F18.PT and RAFALE.PT profiles: level, loop, banked
turns, stall/spin recovery, landing, gear-up contact, taxi, takeoff, wind, water and
hard landing. This remains adapter regression evidence, not native parity.

Six real window smoke tests passed: menu, Quick Mission creator, terrain viewer,
F18 flight, Rafale flight and flight with the generated Ultimate 2 profile. The
active camera-panel benchmark completed five asynchronous camera readbacks.

## Short frame-time evidence

Matched command, before at `4e5d5aa` and after this working-tree change:

```sh
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 TORE_PERF_VIEWS=1 target/debug/tore-app --free-flight --no-audio --window-size 1280x720
```

The before executable was built from an isolated temporary checkout, which was
removed after measurement. The after run had the Ultimate 2 connected with the
standard mappings. Runs were sequential; each excludes 30 warmup frames and
reports zero paused frames and 150 mirror renders.

| Case | Mean interval | Median | p95 | Maximum | Mean simulation/cameras |
| --- | ---: | ---: | ---: | ---: | ---: |
| Before | 1.27 ms | 1.02 ms | 1.28 ms | 11.38 ms | 0.07 ms |
| After, matched | 1.29 ms | 1.07 ms | 1.58 ms | 12.41 ms | 0.08 ms |
| After, additionally recording pilot inputs | 1.22 ms | 1.03 ms | 1.26 ms | 11.69 ms | 0.08 ms |
| After, camera instrument page 3 | 1.27 ms | 0.94 ms | 1.58 ms | 11.42 ms | 0.10 ms |

These are short CPU wall-clock intervals including presentation backpressure,
not GPU timings, displayed FPS or input-to-display latency. Mean interval stayed
close, but the matched run's p95/max increased; this is not evidence that latency
or all stutter is unchanged. Longer platform measurements and human handling
acceptance remain open. No post-render sleep or blocking live readback was added.
The native worker's bounded wait is separate from active simulation/rendering.

## Remaining acceptance

- User Ultimate 2 flight test: signs, partial travel, trigger cancellation,
  shoulder throttle, keyboard override, menu isolation, instrument selection,
  pause/focus recovery and unplug/reconnect. The explicit Linux rumble pulse passed
  by user report; cancellation and repeated in-flight effects still need testing.
- Windows/macOS full app/runtime and actual hardware tests, including simultaneous
  controllers, Apple duplicate suppression, haptic locality fallback, interruption
  and restart after native haptic errors. Generic HID rumble on macOS and directional
  flight-stick forces remain unimplemented.
- Physical HOTAS, pedals, switches, encoders and multiple identical button boxes.
  Synthetic coverage does not certify vendor firmware/driver behavior.
- Automatic mappings beyond standard Linux gamepads, radial dead zones, a
  calibration wizard and reviewed multi-contact selector composition.
- Native controller mapping parity, wall-clock capture scheduling through long
  stalls, cross-CPU flight determinism and whole-mission replay.

Ignored evidence is under `.local/input-validation/`: device/profile output,
checks, both aircraft suite results, smoke logs, before/after/camera timing logs,
a pilot tape and headless replay output. No retail derivatives or user device
identities were added to tracked fixtures. No commit or push was performed.

## Rumble follow-up

The user explicitly confirmed a felt Linux test pulse on their Ultimate 2. No
additional hardware pulse was triggered by automation in this follow-up.
Windows retains native two-motor Gamepad vibration and now also zeros motors on
endpoint removal/read failure. macOS 11+ adds Apple GameController/CoreHaptics with
retained per-controller endpoints, finite effects, two-handle/default-locality
routing and stop/error cleanup. Raw HID is retained for generic equipment.
Apple gamepad identities are session-only; wildcard named-control profiles are
explicitly shared, not persistent physical identity. `--test-rumble only` rejects
zero or multiple capable devices and waits for native acceptance and pulse expiry.
Synthetic selection coverage verifies ambiguity rejection and exact targeting.

The follow-up reruns formatting, workspace Clippy/tests/build, Python tests and
asset guards, plus both Windows and Apple Silicon native-backend Clippy targets.
Logs use `.local/input-validation/rumble-*.log`. These are Linux regression and
cross-compilation checks, not macOS/Windows linking or tactile acceptance. No
rendering code changed in the follow-up, so prior GPU evidence is unchanged.

## Event impulse follow-up

Added an authored, fixed-eight-slot `tore-input::feedback` mixer and typed events
for gun, missile, bomb, rocket, turbulence, afterburner, damage and crash. Only
actual afterburner activation and crash currently have live producers. Weapon,
damage and turbulence cues are hooks; no input-only firing or invented turbulence
was added. The mixer advances at 120 Hz, emits at most 20 updates per simulated second with catch-up coalesced,
uses per-motor maxima and bounded finite durations, and clears on interruption.
Assigned capable controllers receive cues at rest; UI-only/unassigned devices do
not. A newly connected controller is not added to an already-playing event.

Six added synthetic tests cover overlap caps/expiry, repeated-fire bounds, event
cooldowns, finite turbulence scaling, interruption, opt-in and idle-device routing.
Workspace formatting/Clippy/tests/build, native Windows/macOS cross-target Clippy,
Python tests and asset guards are rerun; logs use `impulses-*.log` under ignored
`.local/input-validation/`. Current total: 156 Rust tests and 11 Python tests.
No rendering or flight-response laws changed. Actual afterburner tactile strength
and Windows/macOS hardware behavior remain acceptance work.

## Controls editor, preferences and continuous afterburner follow-up

The flight Control root now opens the authored paused mapping/rumble editor.
Bindings can be captured, selected, calibrated, added/removed and saved to the
active input profile (or default app-data profile). Compatible modes and shared
assignments remain explicit. Save validation precedes file replacement/live
application; tests verify reload and preservation after invalid edits. Automatic
new-gamepad defaults survive rumble-only changes when enabled.

Normal user preferences retain both instrument page sets/layouts, selection,
range/mode, cockpit/HUD/zoom and sound choices. These survive aircraft changes and
flight restarts. Preference parsing is bounded; malformed files are reported and
preserved. Restore does not play toggle sounds. Smoke/capture/performance probes
ignore preferences and do not overwrite them. Their windows use fixed requested
sizes; ordinary app windows retain interactive resizing.

Afterburner now adds a quiet continuous low-frequency bed (3.5% strong / 1% weak)
under the engagement impulse. Each native lease lasts 750 ms and renews every
500 ms of simulated flight. Disengagement clears both afterburner cues; pause/focus
loss clears mixer/native state. Synthetic sustained-effect tests verify renewal,
stopping and interruption. This is provisional tactile tuning; no new physical
rumble was triggered by automation.

Workspace formatting/Clippy/tests/build, Python tests, source/binary asset guards
and native Windows/macOS cross-target Clippy are rerun. Logs use `settings-*.log`
under `.local/input-validation/`. Real GPU editor captures are `controls-wide.ppm`
and `controls-tall.ppm`; creator/viewer/menu smoke logs are retained alongside them.
No retail screenshots or personal profiles are added to tracked fixtures.

Final follow-up results: 165 Rust tests and 11 Python tests passed, along with
formatting, workspace Clippy (warnings denied), build, both native cross-target
Clippy checks and all asset guards. Editor captures were verified at **1280×720**
and **720×1000**; creator, viewer and main-menu GPU smoke checks passed. The first
capture frame can precede native enumeration, so saved device bindings appear as
disconnected in those images; captures validate geometry, not device handling.
Windows/macOS linked runtime, tactile tuning and physical capture still require
user/hardware acceptance.
