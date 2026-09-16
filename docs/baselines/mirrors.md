# Live mirrors and uncapped presentation

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


2026-09-14, Linux/Wayland, NVIDIA RTX 4070 / Vulkan, Rust 1.91.1 dev profile
(app opt-level 2). User-owned FA media, Ukraine, clean free flight, no audio.

## Source and fitted behavior

Both `~F18H.PIC` and `~RAFH.PIC` are reviewed 1280×490 transparent cockpit art.
Original flat fills identify center/left/right mirror silhouettes. F18 seed
pixels are (640,40), (110,380), (1170,380); Rafale seeds are (640,40), (190,360),
(1090,360). These are authored identification hints from inspected art, not native
camera records. Four-connected exact-RGBA floods require opaque, non-overlapping
regions between four pixels and one eighth of the image. Dimension/size bounds
reject other inputs; a failed mask preserves the original fills. No source image,
mask bytes or other retail derivative is committed.

The ignored reference's `tools/retail/retail/cockpit.py` demonstrates flat-fill
mask identification; `engine/src/terrain/mirrors.ts` uses one 512×256 rear scene
at 10 Hz. Its engine is not used by this app.

The Rust renderer uses one persistent 768×384 color target, with cached depth,
for all three mirrors. The GPU shader samples horizontally reflected,
aspect-preserving center/side crops inside the source silhouettes. Camera axes
look opposite aircraft forward while retaining aircraft up, including bank and
vertical attitudes. Eye placement is fitted at seven feet above and ten feet
forward of model origin. The rear pass sees the original animated exterior;
the primary cockpit pass does not. The same interpolated state drives both.

Every flight frame with visible mirror art submits a rear pass. No timer, frame
skipping, CPU pixel transfer or GPU wait is used. Shared world/aircraft resources
are reused; the rear submission precedes primary camera-buffer updates so the
views cannot overwrite each other's uniforms. Mirrors share the cockpit's
source-coordinate pan, zoom and fade. Hidden/offscreen mirrors skip rendering.
Aircraft changes regenerate masks and clear stale geometry.

Optics, crop positions and eye placement remain fitted. These are rear-view
crops rather than native curved mirror geometry or independently traced mirror
planes. Source pixel resolution remains visible when enlarged. Native mirror
optics and full cockpit interior recovery remain open.

## Presentation and measurements

Select supported Immediate presentation, otherwise Mailbox, otherwise FIFO.
Immediate was available on this host. Uncapped modes omit Winit's Wayland
`pre_present_notify` frame callback, which otherwise paces redraws to compositor
refresh even with Immediate selected. No desktop configuration changes. There
is no application flight FPS cap; drivers/compositors can still apply pacing.
Simulation stays fixed at 120 Hz. Menus retain their idle scheduling.

Final runs: 6,030 frames, first 30 excluded, zero paused frames. Logical 1280×720
floating window at 1.6× desktop scale (2048×1152 drawable; overlay capped at
1920×1080). The commands below plus local `bench.py` set the actual window size;
initial `--window-size` alone may be overridden by tiling. All times are CPU wall
measurements, including queue/compositor backpressure, not GPU timestamps or
verified displayed FPS. No other intentional graphics benchmark ran concurrently.

| Case | Mean interval | Median | p95 | Max | Rear renders |
| --- | ---: | ---: | ---: | ---: | ---: |
| Rafale, mirrors disabled | 1.84 ms | 1.07 ms | 11.22 ms | 11.89 ms | 0 |
| Rafale, mirrors enabled | 1.45 ms | 1.26 ms | 1.41 ms | 11.79 ms | 6,030 |
| F18, mirrors enabled | 1.33 ms | 1.27 ms | 1.37 ms | 12.24 ms | 6,030 |

Enabled means correspond to about 690 and 750 application frame starts/second.
Rafale median rises about 0.19 ms with mirrors; the disabled run encounters more
presentation stalls, so its worse mean does not imply mirrors make rendering
faster. Short preliminary 1,530-frame runs were around 750–780 starts/second.
These are bounded desktop samples, not sustained thermal or other-platform claims.

A 330-frame Rafale run with camera instrument 3 completed six asynchronous
instrument readbacks and 330 rear renders, zero paused frames. A 330-frame
front/back/up/chase/oblique cycle performed 150 rear renders (only views with
visible mirror art), zero mirror readbacks and zero paused frames.

```sh
TORE_PERF_FRAMES=6030 TORE_PERF_ACTIVE=1 cargo run --locked -p tore-app -- --free-flight --aircraft rafale --no-audio --window-size 1280x720
TORE_MIRRORS=0 TORE_PERF_FRAMES=6030 TORE_PERF_ACTIVE=1 cargo run --locked -p tore-app -- --free-flight --aircraft rafale --no-audio --window-size 1280x720
```

`TORE_MIRRORS=0` is a startup comparison switch retaining original fills. Omit
for normal operation. Instrument camera pages keep their separate asynchronous
readback cadence; that never limits mirror rendering.

## Validation

124 Rust tests and 11 Python tests; locked build, formatting, warnings-denied
Clippy and source/debug-binary asset guards. New synthetic tests check flood
bounds, silhouette coverage, overlapping seeds, leaks and rear-camera attitude
through bank/vertical flight. Existing simulation, pause/input isolation and
cockpit pan/zoom tests pass.

GPU smoke checks: creator, terrain viewer, rear, overhead and exterior. Wide and
tall live captures inspect Rafale forward, F18 side-look and zoom-out, including
original mirror rims and Hornet twin-tail reflections. Aircraft switching is
checked through Quick Mission. Camera-preview and view-cycle runs verify shared
resources. Evidence stays in ignored `.local/mirrors/`; switch captures remain
under `.local/rafale-animation/`. Windows/macOS checks unavailable on this host.
