# Engine material validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-16. User-requested presentation, using John's
supplied image rather than retail texture bytes. Contract:
[engine material](../spec/engine-material.md).

## Evidence

| Asset | SHA-256 |
| --- | --- |
| engine-texture-full.png | eb060ecbf6817ddcb863b4ae1d185e0cad32bc0a66e9aea7dea0db347b22b1f7 |
| engine-texture.png | f8b63dc73b6b4c4e63810cff2ee1af6b16be07a760dbbbc2c4be86ee4ee81ed6 |
| engine-texture.rgba | 9ca91857c0d4d6be9c267e57935ec239d1c926738e5703067e68c3b4e388e3d2 |

Full source is 1254 × 1254. Runtime copy is 314 × 314, generated with Box
resampling at 25% dimensions. The full source was moved, not re-encoded.

## Results

All 382 Rust tests, 40 Python tests, workspace Clippy, formatting, locked build,
documentation checks and source/binary asset scans passed. Bounded image parsing,
zero/mid/full throttle, afterburner override, engine-off/fuel-out and A-4 exclusion
have synthetic checks. The shader is validated by GPU pipeline creation.

Linux NVIDIA RTX 4070/Vulkan smoke test passed. Rendered zero/full throttle
captures of F/A-18D, Rafale C, X-31 and F-14D passed; twin-engine mapping and
red-white versus gray endpoints were visually checked. An X-31 mid-throttle
capture and an A-4 unchanged-material capture also passed. Burner-on capture
passed with the existing plume. Logs and local images are under
`.local/engine-material-checks/`. Windows/macOS and subjective live throttle
sweeps were not run. This does not claim original FA material parity.
