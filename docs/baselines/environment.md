# Development baseline — 2026-09-13

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


## Host

| Item | Observed value |
| --- | --- |
| Machine | MacBook Air M3 (user identified); Apple M3 graphics adapter confirmed |
| OS | macOS 26.6.2, build 25G83 |
| Architecture | arm64 / `aarch64-apple-darwin` |
| Xcode developer directory | `/Applications/Xcode.app/Contents/Developer` |
| Rust | rustup-managed 1.91.1, rustfmt and Clippy installed |
| Python | 3.14.6 |
| Renderer | `Apple M3 (Metal, IntegratedGpu)` |

Homebrew Rust 1.91.1 was already present. Installed rustup and the matching toolchain alongside it, and appended `. "$HOME/.cargo/env"` to `~/.zshrc`. A new interactive zsh resolves Cargo through `~/.cargo/bin/cargo` and honors the repository toolchain pin. No Homebrew packages or Xcode settings were changed.

## Validation

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed; currently zero Rust unit tests |
| `cargo build --workspace --locked` | Passed |
| `cargo run --locked -p tore-app -- --smoke-test` | Passed; real Metal surface initialized and one frame presented, exit 0 |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | Passed; four synthetic asset-guard tests |
| Source asset guard | Passed |
| Debug executable asset guard | Passed |
| Local media, reference checkout, build output ignored | Confirmed with `git check-ignore` |
| Linux and Windows builds | Configured in GitHub Actions; not executed in this session |

The smoke test verifies GPU startup and frame presentation, not visual parity, interactive resize/keyboard behavior, or sustained frame timing. No retail assets are loaded. No screenshots or performance claims are recorded at this stage.

## Remaining roadmap work

This establishes the local environment and initial cross-platform build configuration. It does not complete M0: full salvage classification, title/format census, settled parity specification, and the AI VM decision remain open. Linux/Windows build results and runtime evidence remain to be collected.

Next game-facing work is M1a: validate the local archive layout, implement the minimum container/image/palette/font/layout readers, and render the first faithful menu. See [local references](../REFERENCES.md). Expand the initial data guard as real import and release formats become known.
