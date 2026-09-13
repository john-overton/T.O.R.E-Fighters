# Development environment

## Baseline

The initial host is an Apple Silicon MacBook Air M3 running macOS. Build natively as `aarch64-apple-darwin`; Rosetta is unnecessary. The baseline needs Rust, a native C linker, Git, and Python 3 for the asset guard. It needs no Node/Bun, retail media, external synth, or Vulkan SDK on macOS.

Rust **1.91.1** is intentionally pinned to match the compiler already present on the initial host. It is a reproducible starting version, not a claim to be the newest release. `rust-toolchain.toml` selects the minimal profile plus rustfmt and Clippy; `Cargo.lock` fixes resolved dependencies. Upgrade both deliberately and validate all three platforms.

## macOS setup

Install Apple's command line tools if Xcode or its tools are not already installed:

```sh
xcode-select --install
```

Install rustup using the [official installer](https://www.rust-lang.org/tools/install):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh
sh /tmp/rustup-init.sh -y --profile minimal --default-toolchain 1.91.1 --component rustfmt --component clippy
source "$HOME/.cargo/env"
```

If Homebrew Rust is already installed, the installer may report the existing installation. To keep it alongside rustup, prefix the `sh` installer command with `RUSTUP_INIT_SKIP_PATH_CHECK=yes`. Put `source "$HOME/.cargo/env"` after Homebrew's PATH setup in `~/.zshrc`. The initial host has an equivalent line configured; no Homebrew packages were removed.

Check tools from this repository:

```sh
command -v cargo
rustup show
rustc -vV
xcode-select -p
python3 --version
```

Cargo should resolve to `~/.cargo/bin/cargo` and the compiler host to `aarch64-apple-darwin`. Python 3.10+ is sufficient; if absent, `brew install python` supplies it. Python needs no third-party packages.

Build and run:

```sh
cargo build --workspace --locked
cargo run --locked -p tore-app
```

Expect a 960 × 720 logical-pixel dark window and a terminal message such as `Renderer: Apple M3 (Metal, IntegratedGpu)`. There is no menu art yet. Resize the window; close it or press Escape to exit.

## Linux and Windows

Linux: install rustup and a native toolchain. On Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxrandr-dev python3
```

Running the app also requires a graphical session and a working Vulkan or OpenGL/EGL driver. CI builds and tests without creating a window.

Windows: install rustup from the official installer and Visual Studio 2022 Build Tools with **Desktop development with C++** and a Windows SDK. Use the MSVC Rust host toolchain. Install Python 3 and Git. Run the same Cargo commands in PowerShell; use `python` instead of `python3` where appropriate. The renderer can use Direct3D 12 or Vulkan.

## Everyday checks

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 -m unittest discover -s tools -p 'test_*.py'
python3 tools/check_assets.py
python3 tools/check_assets.py target/debug/tore-app
```

On Windows, the executable is `target/debug/tore-app.exe`. There are no Rust unit tests yet; `cargo test` currently validates compilation of the test target. Python tests exercise the data guard with synthetic inputs.

With a working desktop session, also run:

```sh
cargo run --locked -p tore-app -- --smoke-test
```

This uses a real window and GPU, prints the renderer, presents one frame, and exits. It is not a headless simulation test. Normal mode waits for window events instead of rendering continuously.

## Data guard

`tools/check_assets.py` scans tracked and non-ignored untracked files. It rejects Git-visible local media/reference directories, common retail asset extensions, binary EALIB markers, and recognizable PIC headers. Text documentation may name formats. Pass explicit files/directories to inspect artifacts, including ignored outputs.

This is an initial guard, not proof that an artifact contains no retail derivatives. PIC has a structured header rather than ASCII magic. The check recognizes standalone PIC headers and embedded EALIB markers; it does not decode compressed packages, detect embedded PIC at arbitrary offsets, or identify converted art/audio. Extend it as importers and packaging arrive. Run it against exact release contents before shipping; a debug executable check alone is not a release audit.

## Troubleshooting

- Wrong Rust version: source `~/.cargo/env`, check `command -v cargo`, and run `rustup show` from this repository. Homebrew Cargo does not honor rustup overrides by itself.
- Linker/SDK error: check `xcode-select -p` and complete any pending Xcode first-launch setup.
- No graphics adapter or display: run from a logged-in desktop session with working drivers. Compilation and tests do not require a window; the smoke test does.
- Missing reference/media folders: the shell still runs. See [REFERENCES.md](REFERENCES.md) before importer work.

An editor with rust-analyzer is useful but optional. No global editor configuration is required.
