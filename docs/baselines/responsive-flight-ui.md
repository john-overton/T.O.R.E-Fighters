# Responsive cockpit and instrument composition

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


2026-09-13, Apple M3/macOS, Rust 1.91.1, wgpu Metal. This supersedes the fixed 640×480 flight composition and intermediate small-window downsampling in the earlier [layout checkpoint](instrument-layouts.md). Menus retain their original proportional canvas.

## Changes

The flight overlay follows the drawable aspect, proportionally bounded to 1920×1080. The original Hornet frame uniformly covers that entire overlay; wider displays reveal more source side artwork instead of cutting off the image at a centered 4:3 boundary. Corner windows and the two small bottom groups anchor to actual screen edges. Reference sizes/margins scale by `min(width/640, height/480)`; the central gap grows with available width. Pointer coordinates use the same rectangles, including right-hand controls outside the old menu canvas.

Small instruments go directly from their 160×156 source-font raster to destination size, avoiding the old 96×94 intermediate raster and subsequent enlargement. Alpha-aware filtering retains transparent edge colors. Static cockpit images and unchanged resized panel images are cached; no new font or dependency is required. This remains raster typography, not resolution-independent text.

The HUD presentation is 15% smaller. Projection compensation preserves the camera-relative pitch ladder/flight-path mapping, including tall windows. The dark TAS/MSL number backgrounds are removed; overlapping tape labels are suppressed. No terrain, physics or aircraft asset identity changes were made.

## Evidence

58 Rust tests and five Python tests pass; formatting, Clippy with warnings denied, locked build and all asset guards pass. New checks cover wide/ultrawide/portrait panel bounds and right-hand button input, alpha filtering without dark halos, and HUD focal-scale preservation while reducing its presentation size. The previous synthetic thin-line reduction test was replaced because the intermediate downsampler is no longer used.

Eight Metal checks passed. Captures use the actual drawable aspect, not the previously fixed 960×720 flight capture:

| Capture | Output size |
| --- | --- |
| Wide, small instruments | 1920×1080 |
| Wide, large instruments | 1920×1080 |
| Ultrawide, small instruments | 1920×800 |
| Tall, large instruments | 827×1080 (window-manager-constrained size) |
| Wide flight menu | 1920×1080 |
| Wide camera instrument | 1920×1080 |
| Quick Mission Creator / terrain viewer | Window smoke checks |

The wide small, ultrawide small and tall large captures were visually inspected. Local evidence: `.local/f18-research/responsive-tests.log`, `responsive-gpu.json`, `wide-small.ppm`, `wide-large.ppm`, `ultrawide-small.ppm`, `tall-large.ppm`, `wide-menu.ppm`, `wide-camera.ppm`. Derivatives remain ignored.

```sh
mkdir -p .local/f18-research
cargo run --locked -p tore-app -- --window-size 1280x720 --instrument-layout small --capture-flight .local/f18-research/wide-small.ppm
cargo run --locked -p tore-app -- --window-size 1440x600 --instrument-layout small --capture-flight .local/f18-research/ultrawide-small.ppm
cargo run --locked -p tore-app -- --window-size 640x900 --instrument-layout large --capture-flight .local/f18-research/tall-large.ppm
```

`--window-size WIDTHxHEIGHT` requests logical dimensions; the OS may constrain them. `--capture-flight` now preserves the resulting aspect at the bounded overlay size. Terrain-only captures remain 960×720. Windows/Linux runtime, native cockpit composition parity, mirror cameras and native HUD-symbol mapping remain open. No frame-rate or manual native-game acceptance is claimed by these smoke checks.
