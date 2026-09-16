# Momentum, vertical flight, sky and cockpit follow-up

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


2026-09-13, Apple M3/macOS/Metal, Rust 1.91.1. This follows the performance and look-around passes after the user's steep-climb screenshot exposed the 86-degree stop, sky pinching and disappearing frame.

## Diagnosis and changes

The committed pre-performance `flight.rs` at `d0c2f05` already clamped pitch to ±1.5 radians and recomputed velocity from nose pitch/yaw every tick. The performance pass added render interpolation but did not introduce those two limitations. The separate look-around change did explicitly hide the cockpit when panned; this behavior is now removed.

- `attitude.rs` integrates an orthonormal body basis with Rodrigues rotation and derives yaw/pitch/bank only as coordinates. It has no Euler-rate division by pitch cosine and no aircraft pitch clamp. Basis interpolation preserves continuity through both vertical attitudes. Canonical pitch coordinates remain within ±90 degrees while yaw/bank change across vertical; this does not constrain orientation.
- State now retains world-space velocity. Source-informed thrust and drag plus authored lift and gravity accelerate it. Position and vertical speed follow that vector. Pitch/roll response and aerodynamic alignment are authored and recorded in [aircraft notes](../formats/aircraft.md). Nose orientation no longer defines travel direction by assignment.
- The HUD flight-path marker projects actual velocity bearing and elevation, allowing lateral and vertical displacement from the nose datum.
- Head-look rotates in aircraft coordinates. The existing cached cockpit frame remains visible for internal front/back/up and panned views. It is a fixed 2D overlay, reused provisionally for rear/up, not a recovered 3D canopy or directional cockpit.
- The old sky UV mapping converged all source longitudes at the pole, creating the screenshot's radial spokes. SKY0 now uses an authored finite hemisphere disk mapping. The same original texture and palette are reused, without the azimuth seam/zenith collapse. Native weather projection is still pending.

This is an improved development adapter, not native flight-model acceptance or a full six-degree-of-freedom aerodynamic simulation. Inertia tensors, control moments, measured lift/AOA curves and stall/spin parity remain open. No TypeScript engine or new dependency was introduced.

## Simulation evidence

The imported F/A-18D loop probe starts at 450 KTAS / 5,000 feet, full throttle/afterburner and sustained pull. It terminates when the attitude has passed inverted and returned near the initial forward direction:

```sh
cargo run --locked -p tore-app -- --headless-flight 10800 --maneuver loop
```

Observed: `vertical=true inverted=true loop_completed=true`; **2,734 ticks / 22.783 seconds**, 428.724 KTAS, 4,862.882 feet, 10,855.467 lb internal fuel, no crash. This is a regression result for the authored adapter, not a retail maneuver measurement. Existing headless level/pull/roll/stall modes remain available.

67 Rust tests and five Python tests pass. New coverage exercises full attitude revolution and interpolation across vertical, a complete synthetic-profile flight loop, nose/velocity separation under maneuver, residual/decaying roll response after release, and bank-relative head-look. Existing fixed-tick render-rate determinism, pause/input isolation, fuel, devices and crash tests remain green. Formatting, Clippy with warnings denied, locked build and repository/binary asset guards pass.

## Rendering and performance evidence

Eight Metal checks passed: 90-degree zenith; 89-degree elevation at a different azimuth; tall side-look cockpit; an exterior view from below against the sky; creator; terrain viewer; live camera instrument smoke; Escape menu. The four captures were visually inspected. They retain the cockpit where requested, show the exterior aircraft, and remove the radial sky collapse near/at zenith. Wide/tall composition and panel margins are preserved.

```sh
cargo run --locked -p tore-app -- --flight-look 0,90 --window-size 1280x720 --capture-flight .local/zenith.ppm
cargo run --locked -p tore-app -- --flight-look 90,89 --window-size 1280x720 --capture-flight .local/near-zenith.ppm
cargo run --locked -p tore-app -- --flight-look 70,10 --window-size 640x900 --capture-flight .local/cockpit-side.ppm
```

An additional 330-frame active view-cycle benchmark, excluding 30 warmup frames, reported zero paused frames, mean UI composition **1.74 ms**, mean simulation/camera work **0.10 ms**, frame-start median **16.50 ms**, p95 **17.80 ms**, maximum **33.45 ms**. These CPU wall times include presentation backpressure and do not prove scanout FPS or eliminate compositor outliers. The performance changes remain in place. Sustained thermal, manual handling and Windows/Linux runtime acceptance remain unmeasured.

Local evidence under `.local/performance/`: `retail-loop.log`, `flight-fixes-checks.log`, `flight-fixes-gpu.log`, `flight-fixes-perf.log`, and `zenith`, `near-zenith`, `cockpit-side`, `orbit-sky` PPM/PNG captures. Derivatives remain ignored.

The cockpit/HUD presentation described above is superseded by the [directional forward-plane pass](directional-cockpit.md); historical checks remain recorded here.
