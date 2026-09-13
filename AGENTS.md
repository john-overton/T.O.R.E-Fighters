# Agent instructions

Read [docs/ROADMAP.md](docs/ROADMAP.md), [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md), and [docs/formats/menu.md](docs/formats/menu.md) before changing the project. Current work includes the first M1a menu slice and partial M1b Ukraine viewer; M0 research and the rest of M1a remain open.

- Keep documentation in lowercase `docs/`; validation evidence in `docs/baselines/`; format coverage in `docs/formats/coverage.md` when importer work starts.
- Maintain [docs/progress.md](docs/progress.md) with completed substeps, remaining parity work and acceptance evidence. The remaining menu screens are deferred until explicitly scheduled.
- Read [docs/formats/theater.md](docs/formats/theater.md) before terrain/weather changes; its native T2 byte layout supersedes the reference reader. Keep the shared theater definition table, extraction profiles and sky/celestial dependencies aligned between app and CLI. Validate both creator and viewer smoke tests for rendering changes.
- Recover terrain and environment systems from retail assets and verified native behavior. Do not port USNF-ATF's custom terrain system or substitute its DEM-based theaters for original terrain; its partial T2 research is reference evidence only.
- This is a native Rust rebuild. `USNF-ATF/` is an ignored reference checkout: use recovered specifications and baselines, not its engine or TypeScript runtime.
- `gameassets/`, `USNF-ATF/`, `.local/`, and `target/` are local only. Never force-add retail media, extracted art/audio/fonts, generated retail derivatives, or reference checkout contents. Use synthetic fixtures in committed tests.
- The importer reads user-owned media at runtime and writes selected decompressed resources to platform application data. Never embed retail bytes with `include_bytes!` or build scripts. Keep snapshots, inventories, and comparison outputs in ignored `.local/`.
- Preserve the user's fidelity requirement: reuse original art, button pieces, and fonts. The USNF-ATF menu has custom controls and is not the visual specification. Prefer Fighters Anthology media plus the user's reference photos. Document authored hover/press behavior and submenu stubs instead of calling them decoded retail behavior.
- Keep dependencies small and purposeful. Platform dependencies are `winit`, `wgpu`, `pollster`, and `cpal` for audio output. `tore-formats` has no dependencies and owns bounded binary readers. Formats, simulation, and synthesis stay independent of the renderer.
- `tools/extract_assets.py` is the cross-platform extraction entry point; it invokes `tore-extract`, which shares `tore-formats` with the app. Keep extraction independent of the reference checkout and title-specific filenames. Preserve archive boundaries, safe paths, size limits, conflict checks, and provenance reports. See `docs/EXTRACTION.md`.
- Menu startup selects randomly among the five original backgrounds; respect their different palettes and menu-bar origins. Hover/focus changes are silent. Sounds are for actual clicks/toggles. Use `--background` and `--snapshot-state` for reproducible visual checks.
- Use `rust-toolchain.toml` and retain `Cargo.lock`. Validate with `--locked`; update dependencies deliberately.
- Preserve Linux, Windows, and macOS support. Avoid machine-specific paths and GPU/display requirements in unit tests. Future simulation must support deterministic headless execution.
- Run formatting, Clippy with warnings denied, tests, and a build as listed in `docs/DEVELOPMENT.md`. For rendering changes, run the window smoke test on a display-capable host. Run the asset guard when files or artifacts change. Report unavailable platform checks honestly.
- Keep README, setup docs, and baseline evidence aligned with actual behavior. Record open decisions instead of silently choosing AI behavior, menu fidelity, or import formats.
- Summarize in plain English: outcome, validation, and material limitations. Do not commit or push unless asked.
