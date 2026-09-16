# Flight performance pass

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-13, Apple M3 / macOS / Metal, Rust 1.91.1. Tests used the imported Ukraine theater and F/A-18D, requested 1280×720 logical windows, responsive overlays bounded to 1920×1080, and no audio. Retail assets and measurements remain in ignored `.local/performance/`.

## Findings and changes

- The application scheduled its next flight frame **16 ms after finishing the current frame**. That additive delay was separate from surface presentation. Flight/viewer redraws now rely on AutoVsync backpressure, with one requested queued frame; menus retain their idle/animation scheduling. Zero-size/failed presentation does not create a continuous simulation redraw loop.
- Unoptimized CPU UI work dominated the initial debug rendering run. The app crate now builds with dev `opt-level=2`, retaining debug symbols/assertions and the existing dependency profiles. Normal `cargo run` benefits without requiring a release build.
- Hiding/showing the cockpit invalidated the single background cache. The cache now retains the scaled cockpit while hidden, regenerating when its visible dimensions change. Aircraft textures/buffers are prepared with renderer/theater initialization instead of on the first external view.
- Authoritative physics still advances at 120 Hz. Render-only interpolation supplies a common pose to the camera, aircraft and HUD, handles heading/bank wrap, and leaves discrete controls and headless state untouched. Pause/crash present authoritative state; restart resets interpolation history. This adds at most one simulation tick of visual interpolation latency, not a change to the flight response law.
- Live camera instruments formerly called the blocking screenshot readback path. They now submit at most one pending readback per camera page, poll without waiting, and keep the last completed raster. The existing 10 Hz request cadence is retained. Two cached depth targets avoid rebuilding the display and 138×114 preview attachments on every refresh. Offline smoke/capture paths still wait for a complete image.
- F2/F3 deliberately select the native look-back/up commands; the earlier prototype had assigned exterior cameras there. The model is present in F10 chase and the `--flight-view 2` oblique camera. No asset re-import or camera-binding reversal was necessary.

## Measurements

The diagnostic records CPU wall-clock times, **not GPU timestamps or verified display scanout FPS**. Submission/presentation includes VSync/compositor backpressure. First 30 frames are excluded; asset loading and initial cockpit construction are outside the measured window.

The initial 150-frame view-cycle rendering run averaged **57.51 ms** between frame starts, with UI composition averaging **38.52 ms** (p95 38.75). A matching post-change rendering run averaged **16.68 ms**, with UI composition averaging **1.96 ms** (p95 2.37). These correspond to roughly 17 versus 60 application frame starts/second, not a certified displayed-FPS comparison.

A follow-up counter exposed focus-triggered auto-pause in automated launches: the matched post-change run had 149 paused frames. The initial run did not yet count pause state. Therefore that comparison establishes rendering/pacing improvement, not active-flight performance. Explicit active benchmarks were added and run separately:

| Active case (330 frames each) | Mean interval | Median interval | p95 interval | Maximum interval | Mean UI | Completed camera readbacks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Repeated front/back/up/chase/oblique | 15.17 ms | 16.50 ms | 17.60 ms | 38.63 ms | 1.90 ms | 0 |
| Forward cockpit, six small instruments | 12.34 ms | 8.91 ms | 24.71 ms | 34.08 ms | 3.03 ms | 0 |
| View cycling with exterior camera instrument | 13.69 ms | 16.17 ms | 17.73 ms | 35.15 ms | 1.69 ms | 42 |

All three active runs report **zero paused frames**. Simulation/camera work averaged 0.09 / 0.03 / 0.19 ms respectively; its maximum with the live camera instrument was 1.19 ms. The removed CPU bottleneck and GPU waits are measurable, but compositor scheduling still produced variable intervals, including occasional 34–39 ms outliers. Do not interpret intervals below 16.67 ms as proof of a higher-refresh display or claim all stutter is eliminated. Sustained thermal, input-to-display latency, manual handling acceptance, actual scanout, and Windows/Linux performance remain unmeasured.

## Reproduce

From the repository root in macOS/Linux:

```sh
cargo build --workspace --locked
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 TORE_PERF_VIEWS=1 target/debug/tore-app --free-flight --no-audio --window-size 1280x720
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 target/debug/tore-app --free-flight --no-audio --window-size 1280x720 --instrument-layout small
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 TORE_PERF_VIEWS=1 target/debug/tore-app --free-flight --no-audio --window-size 1280x720 --instrument-page 3
```

`TORE_PERF_FRAMES` bounds the run and exits after 60–100000 flight frames. `TORE_PERF_VIEWS` cycles views every 30 frames. `TORE_PERF_ACTIVE` explicitly ignores pause state during this bounded benchmark so desktop automation cannot silently measure paused flight; normal gameplay still pauses on focus loss. Omit these variables for ordinary play. See [development instructions](../DEVELOPMENT.md#flight-performance) for PowerShell and metric definitions.

Local evidence: `before.log`, `after-matched.log`, `active-views.log`, `active-cockpit.log`, `active-camera.log`, `checks.log`, and `gpu.log` under `.local/performance/`. Additional post-change preliminary runs are retained there with `after-*` names. No graphics workload was intentionally run concurrently with the final active benchmarks.

## Regression evidence

60 Rust tests and five Python tests pass, including fixed-clock determinism, control/release isolation, wrapped-angle presentation interpolation without state mutation, and bounded profiling/warmup/view cycling. Formatting, Clippy with warnings denied, locked workspace build and asset guards pass.

Ten Metal smoke/capture cases passed: chase, oblique, back, up, wide/small cockpit, tall/large cockpit, camera instrument, Escape menu, Quick Mission Creator and terrain viewer. Chase/oblique captures were visually inspected and show the imported aircraft; wide/tall captures retain cockpit, HUD transparency and instrument margins. The live asynchronous readback path is additionally exercised by the active camera benchmark; screenshot smoke tests intentionally use synchronous capture.

Remaining optimization options: GPU-native UI/panel composition, persistent preview color/readback pools, terrain LOD/streaming, occlusion-aware background scheduling and longer profiling on multiple platforms. Original terrain and flight-system parity remain separate work.

Follow-up: [flight response, sky and retained cockpit](flight-response-sky.md) supersedes the earlier Euler interpolation, rigid nose/velocity coupling and hidden cockpit during head-look. Earlier measurements remain historical evidence.

The 2026-09-14 [live-mirror pass](mirrors.md) supersedes AutoVsync pacing with
Immediate/Mailbox where supported, retaining FIFO fallback. It removes Wayland
frame-callback pacing in uncapped modes and records current Linux measurements.

The 2026-09-14 [shared-input pass](input.md#short-frame-time-evidence) records a
short matched Linux comparison with the Ultimate 2 connected: mean interval
1.27 → 1.29 ms, p95 1.28 → 1.58 ms, zero paused frames. These remain CPU frame
intervals; the small mean difference does not establish unchanged input latency
or sustained performance. The camera-panel run still completes asynchronous
readbacks; no post-render sleep or blocking live readback was introduced.

The [manual weapons pass](manual-weapons.md#short-performance-sample) records
Linux/Vulkan clean F18, loaded live F18 and Rafale missile/camera cases: mean
CPU intervals 1.71 / 2.15 / 2.06 ms, p95 1.96 / 2.42 / 2.39 ms, zero paused
frames, 330 mirrors each and seven camera readbacks in the Rafale case.
These remain short CPU measurements with presentation backpressure; native
parity, GPU timing and sustained maximum-load performance are not established.

## Weapons/systems notifications

The [systems pass](weapons-systems.md#frame-time-investigation-and-fix) records
clean/gun/incoming/camera samples and a measured notification-composition fix.
Incoming-run CPU means dropped from 4.85/4.64 ms to 2.51/2.49 ms by reusing the
existing range HUD line; physics and event scheduling were unchanged. These are
short CPU/presentation measurements, not native-parity or displayed-FPS claims.

## Per-camera weather

The [weather camera slice](weather-cameras.md) records paired clear/fog runs
with an active Other View feed: mean CPU intervals 1.79/1.63 ms, p95 2.73/2.08 ms,
zero paused frames and 330 GPU-only mirror renders each. These are short
presentation-inclusive measurements, not GPU timings or a before/after claim.

Wind/turbulence/attachment continuation: matched calm F18 1280×720/page-3
630-frame debug runs measured mean CPU intervals 2.17 ms before and 2.16 ms
after, with simulation/cameras 0.19 ms in both. No paused frames; 14 completed
asynchronous camera readbacks each. Short-run tail variability and full
qualification are in [the acceptance record](wind-turbulence-vapor.md#frame-time-evidence).
