# Ukraine viewer baseline

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


Date: 2026-09-13. Host: Apple Silicon MacBook Air M3, macOS, pinned Rust 1.91.1, `aarch64-apple-darwin`. This records the first implementation working tree; no commit was created for this slice. Source identity and native addresses are in [theater recovery](../formats/theater.md).

## Working slice

Choose Activity → Create Quick Mission → Terrain Viewer renders Ukraine from retail T2 height samples and UKR texture placements. The creator reuses original FA artwork, font strips, button pieces and the Ukraine briefing map. Its mission fields and dropdown layout are authored stubs. Cancel/Escape return; the creator's `?` provides back/exit. Hover remains silent and activation uses the existing click cue.

The free camera supports arrows, Shift 8× translation, Q/E or PageDown/PageUp altitude, A/D yaw and W/S pitch. Initial position is `(1070000, 28000, 590000)` feet, yaw 0.3 radians, pitch -0.32. Surface height queries match the fixed triangle mesh; the camera stays 100 feet above it. This is not flight simulation.

The catalog reads 16 source T2 names: Panama, The Baltics, Cuba, Egypt, France, Greece, Iraq, Kuril Islands, Falkland Islands, North/South Korea, Persian Gulf, Pakistan, North Vietnam, Ukraine, Vladivostok and Taiwan. Only Ukraine is enabled. A parsed height grid does not mean that theater's dependency bundle or runtime has been ported.

## Extraction evidence

```sh
python3 tools/extract_assets.py --theater UKR --out .local/ukraine-import
```

Result: **213 resources, zero errors**, 51 from FA_1.LIB and 162 from FA_2.LIB. Repeat run: all 213 `unchanged`, zero errors. Hashes, offsets, raw outputs and decoded metadata are in ignored `.local/ukraine-import/extraction-report.json`.

Included: all 16 T2 grids, UKR maps/mission/campaign resources and 29 terrain textures, all LAY modules, sky textures 0–8, SUN/MOON/STARS shapes, CLOUD1/CLOUDS shapes and their `_MOON.PIC`, `_CLOUD1.PIC`, `CLOUDS.PIC` dependencies. The profile does not yet recursively import every ground-object or campaign-generated terrain dependency.

Ukraine has 208 × 200 fine samples, a 26 × 25 coarse grid and height bytes 0–31. UKR.MM has 697 tmap lines at 695 unique coordinates. The current runtime/export map retains the last placement per coordinate; original lines remain in the extracted MM. Duplicate precedence still needs native validation.

## Checks performed

- `cargo fmt --all -- --check`, Clippy for all targets with warnings denied, workspace tests and locked workspace build passed.
- **29 Rust tests passed**, including synthetic packed T2 boundaries/truncation, native palette ramp ordering and RVA bounds, mission metadata, camera speed/ground clamp, creator activation/cancel, silent hover, and extraction dependency selection/error reporting. No committed retail fixtures.
- **5 Python tests passed**. Repository and both debug executable asset guards passed.
- Real window smoke tests passed for main menu, `--quick-mission`, and `--viewer`, reporting **Apple M3 (Metal, IntegratedGpu)**. These present one frame; they do not certify every interactive input or audio device behavior.
- GPU terrain readback and headless creator capture succeeded and were visually inspected. Terrain capture shows source city tiles, green relief, coast/water and a sky/fog preview. Creator capture shows the original panel artwork, Ukraine map and temporary controls.

```sh
cargo run --locked -p tore-app -- --smoke-test
cargo run --locked -p tore-app -- --quick-mission --smoke-test
cargo run --locked -p tore-app -- --viewer --smoke-test
cargo run --locked -p tore-app -- --capture-terrain .local/theater-research/terrain.ppm
cargo run --locked -p tore-app -- --quick-mission --snapshot .local/theater-research/quick.ppm
```

Captures remain ignored at `.local/theater-research/terrain.ppm` and `quick.ppm`, with local PNG conversions. Terrain capture uses the actual sim GPU pass without the HUD; creator capture is CPU-only. No Linux/Windows graphical runtime or original-game side-by-side flight comparison was performed. No new audible acceptance of weather/flight sounds is claimed.

## Historical parity limits and next gate (2026-09-13)

This paragraph records the initial implementation, not current coverage. The water fallback is superseded by the shoreline correction below; current weather/flight coverage is in [the parity plan](../parity-plan.md). The fixed full-resolution mesh, water palette fallback, spherical SKY0 projection and distance fog were initial rendering choices. Native adaptive subdivision, shoreline coverage, fallback terrain materials, 3D objects and native lighting remain open. DAY2 keyframe 2 supplies source colors, but time/altitude interpolation and weather simulation remain unimplemented. Sun/moon/star/cloud shapes are **extracted, not rendered**. No aircraft physics or mission generation is present.

Next: recover sky/cloud SH commands and native weather scheduling, native terrain coverage/LOD, ground-object dependencies and a second theater; compare identifiable original landmarks and controlled weather scenarios. Track these separately in [progress](../research/progress.md); this baseline does not close M1b.

## Shoreline correction, 2026-09-16

Implementation mode, responding to the green strips beyond beaches reported in
Ukraine quick mission at 5,000 feet. Linux, NVIDIA GeForce RTX 4070, Vulkan.
The default Ukraine viewer pose reproduced the same artifact before the change.

The source terrain texture already marked the water correctly. The shared
surface shader turned index-255 texels into zero alpha, then mixed them with the
T2 base land color and wrote an opaque pixel/depth. Untextured color-255 cells
also wrote a flat palette-223 placeholder. Those two paths produced the
rectangular green fringe and flat open water. This was a renderer composition
bug, not evidence of corrupt theater extraction.

Terrain now has its own fragment entry point: water coverage discards without
writing color/depth, revealing the existing ocean/horizon pass. Untextured water
emits no opaque geometry. Aircraft retain the previous material path. Source
texture placements, UV rotations, T2 heights and simulation queries are unchanged.
[Provenance, exact coverage rule and remaining scope](../spec/terrain-shorelines.md).

Matched local captures: `.local/shoreline/before.ppm` and `after.ppm` at the
unchanged default Ukraine viewer pose. Visual inspection confirms the green
strips are gone and the ocean reaches the beach. Retail-derived images remain
ignored. Additional captures and check logs are in `.local/shoreline/`.

Reproduction:

```sh
target/debug/tore-app --theater UKR --capture-terrain .local/shoreline/after.ppm --no-audio
TORE_WEATHER_VIEW=1070000,5000,590000,17.1887,-10 target/debug/tore-app --capture-terrain .local/shoreline/low.ppm --no-audio
target/debug/tore-app --capture-flight .local/shoreline/flight.ppm --no-audio
```

Validation:

- Formatting, warnings-denied workspace/all-target Clippy, locked workspace build
  and **367 Rust tests** passed. The synthetic shoreline regression verifies
  open-water omission, preservation of textured beach geometry in all four
  rotations, untextured land and unchanged height queries.
- **40 Python tests**, documentation checks, diff whitespace check, and repository
  plus both debug executable asset guards passed.
- Real window/GPU checks passed for menu and creator; Ukraine terrain at 28,000
  and 5,000 feet and cockpit flight with mirrors rendered successfully.
- All **16 theater startup captures** succeeded. APA/BAL/CUB/EGY used their
  mission defaults. France's default stopped before rendering with the existing
  `mission wind outside source range` error; FRA and the remaining theaters
  used `TORE_WIND=0,0` to isolate rendering. Contact sheets are
  `.local/shoreline/theaters-a.png` and `theaters-b.png`. These checks cover the
  starting views, not every shoreline in every theater.

macOS/Windows runtime checks and a retail comparison are unavailable on this host.
No change to simulation or claim of retail raster parity is implied.
