# Agent instructions

Read [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) before changing the project. The roadmap governs sequencing; current work is the M0 development baseline leading into M1a menus. The baseline does not complete M0.

- Keep documentation in lowercase `docs/`; validation evidence in `docs/baselines/`; format coverage in `docs/formats/coverage.md` when importer work starts.
- This is a native Rust rebuild. `USNF-ATF/` is an ignored reference checkout: use recovered specifications and baselines, not its engine or TypeScript runtime.
- `gameassets/`, `USNF-ATF/`, `.local/`, and `target/` are local only. Never force-add retail media, extracted art/audio/fonts, generated retail derivatives, or reference checkout contents. Use synthetic fixtures in committed tests.
- The future importer reads user-owned media at runtime and writes decoded content to platform application data. Never embed retail bytes with `include_bytes!` or build scripts.
- Keep dependencies small and purposeful. Current platform dependencies are `winit`, `wgpu`, and `pollster`. Formats, simulation, and synthesis stay independent of the renderer; add crates when they have real work.
- Use `rust-toolchain.toml` and retain `Cargo.lock`. Validate with `--locked`; update dependencies deliberately.
- Preserve Linux, Windows, and macOS support. Avoid machine-specific paths and GPU/display requirements in unit tests. Future simulation must support deterministic headless execution.
- Run formatting, Clippy with warnings denied, tests, and a build as listed in `docs/DEVELOPMENT.md`. For rendering changes, run the window smoke test on a display-capable host. Run the asset guard when files or artifacts change. Report unavailable platform checks honestly.
- Keep README, setup docs, and baseline evidence aligned with actual behavior. Record open decisions instead of silently choosing AI behavior, menu fidelity, or import formats.
- Summarize in plain English: outcome, validation, and material limitations. Do not commit or push unless asked.
