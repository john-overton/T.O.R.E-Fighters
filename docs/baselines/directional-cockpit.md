# Directional cockpit — 2026-09-13

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


The forward cockpit artwork and HUD now occupy one aircraft-fixed plane. Centered flight preserves their existing layout regardless of aircraft attitude. Turning the head projects both together, keeping HUD placement fixed to the combiner instead of the screen. Instruments and pause menus remain screen-anchored. This supersedes the fixed-screen frame and off-axis HUD hiding in earlier baselines.

## Source evidence and limits

Local inspection of `~F18H.PIC` shows a 1280×490 forward frame including side mirrors. The 117×200 left/right and 323×84 center resources are mirror masks, not separate side/rear cockpit pictures. The implementation uploads the complete frame, preserving its source transparency and wide coverage, then projects it on the GPU alongside the existing HUD raster. Forward cover-fit remains aspect-responsive.

The plane is an authored adaptation. Large turns can expose its finite edges, and it naturally leaves the frustum when looking behind or overhead. It does not supply a reconstructed curved cabin, repeat forward artwork behind the pilot, or implement live mirrors. Native view geometry, HUD callers and 360-degree interior acceptance remain open. No retail derivatives were added to Git.

## Validation

On Apple M3 / Metal, six captures were rendered and visually inspected: centered forward; yaw/pitch 8/4, 40/5 and 0/35 degrees; rear view; and a tall window at -15/10 degrees. Forward layout is preserved, small turns do not abruptly hide the frame/HUD, side art becomes visible with head rotation, and both layers move together. Rear view contains no repeated forward frame. Tall layout retains screen-anchored instruments.

Additional GPU smoke checks passed for quick mission, terrain viewer, flight Escape menu and a live exterior camera instrument. Formatting, Clippy with warnings denied, build and 69 workspace Rust tests passed. Two synthetic regression tests cover body-relative anchoring across aircraft attitudes and wide/tall forward cover-fit.

Local captures/logs are ignored under `.local/performance/directional-*`. Reproduce with the commands in [development setup](../DEVELOPMENT.md#directional-cockpit-checks).

## Frame timing

Active unpaused development build, requested 1280×720 window, first 30 frames excluded:

| Run | Mean interval | p95 interval | Maximum interval | Mean UI | Camera readbacks |
| --- | --- | --- | --- | --- | --- |
| 330 frames, cycling views | 16.46 ms | 17.77 ms | 34.50 ms | 1.61 ms | 0 |
| 180 frames, look 25/10, camera instrument | 16.68 ms | 17.43 ms | 19.13 ms | 1.73 ms | 28 |

AutoVsync remains enabled with one queued frame. These are CPU wall intervals including presentation backpressure, not GPU timestamps or proof of eliminating all stutter. The cockpit pass introduces no synchronous readback; source artwork uploads once and HUD pixels/uniforms update through the queue.
