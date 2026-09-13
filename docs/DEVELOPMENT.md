# Development environment

## Baseline

Clone this repository and enter its root before running Cargo commands. Source code is under `crates/`, project documentation under `docs/`, and developer scripts under `tools/`. User-owned media belongs under ignored `gameassets/`; optional reference code under ignored `USNF-ATF/`; research output under ignored `.local/`. See [README](../README.md) for the short run workflow and [EXTRACTION](EXTRACTION.md) for the shared extraction script.

The initial host is an Apple Silicon MacBook Air M3 running macOS. Build natively as `aarch64-apple-darwin`; Rosetta is unnecessary. Development needs Rust, a native C linker, Git, and Python 3 for research/check tools. Building and unit tests need no retail media. Running the app needs imported Fighters Anthology menu and theater resources. There is no Node/Bun runtime, external synth, or Vulkan SDK requirement on macOS.

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

Expect a 960 × 720 logical-pixel window showing Choose Activity and a terminal message such as `Renderer: Apple M3 (Metal, IntegratedGpu)`. The original 640 × 480 canvas scales proportionally, with letterboxing in wider windows. Close the window or use `? → Exit to Desktop`; Escape dismisses menus. On macOS, Command-Q also quits.

Startup chooses randomly among all five original backgrounds; it does not run a timed slideshow. Force a variant for comparison with `--background CHOOSEV` (also accepts CHOOSEAC, CHOOSE3, CHOOSEU, CHOOSEM). The top bar moves to match each artwork's native origin. Hovering and keyboard focus are silent. An older menu-only cache requires re-import; the local default media is automatically used if available.

First launch automatically imports `gameassets/fighters-anthology/` if no valid cache exists. Use `--import <directory>` to refresh or choose other media. `--import-only` imports and exits without opening a window/audio device. Required archives: `FA_1.LIB` and `FA_2.LIB`; optional `FA_4B.LIB` supplies the music preview. Missing required media produces an actionable terminal error; there is no file-picker UI yet.

Cache locations:

| Platform | Directory |
| --- | --- |
| macOS | `~/Library/Application Support/T.O.R.E-Fighters/` |
| Linux | `$XDG_DATA_HOME/T.O.R.E-Fighters/`, falling back to `~/.local/share/T.O.R.E-Fighters/` |
| Windows | `%APPDATA%\T.O.R.E-Fighters\` |

`TORE_DATA_DIR` overrides this directory for isolated checks, e.g. `TORE_DATA_DIR=.local/test-profile cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only`. Each import creates a versioned `menu-*.pack`; the latest valid pack is loaded, with fallback to earlier valid packs if a write was interrupted. `import-report.txt` records resource names and offsets. Older generations are retained; cache cleanup is manual for now. Imported resources never go into the executable.

## Linux and Windows

Linux: install rustup and a native toolchain. On Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxrandr-dev libasound2-dev python3
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
python3 tools/check_assets.py target/debug/tore-extract
```

On Windows, append `.exe` to both executable paths. Rust tests cover malformed formats, decompression, menu hit testing and interaction, PCM resampling, letterboxing, and extraction filesystem behavior using synthetic inputs. Python tests exercise the data guard. None requires a display, audio device, or retail files.

With a working desktop session, also run:

```sh
cargo run --locked -p tore-app -- --smoke-test
```

This uses the imported menu, a real window and GPU, prints the renderer, presents one frame without audio, and exits. It is not a headless simulation test. Normal mode waits while idle and schedules frames for short hover transitions and placeholder messages.

`--no-audio` silences a session. Normal playback uses the system's default output device, original PCM effects, and a quiet looping `AIR003.11K` preview when available. Device initialization failure is reported and the menu continues silently. M toggles music; `Pref` exposes music/effect toggles. Preferences are session-only.

## Explore media and capture previews

```sh
python3 tools/explore_assets.py
mkdir -p .local/exploration
cargo run --locked -p tore-app -- --snapshot .local/exploration/menu.ppm
cargo run --locked -p tore-app -- --snapshot .local/exploration/pref.ppm --snapshot-state pref
```

The inventory records archive SHA-256 hashes, entry offsets, compression headers, and format counts without extracting everything. Snapshots render the native CPU menu canvas without a GPU/audio device; they are not window screenshots. States: `normal`, `hover`, `pressed`, `help`, `pref`, `multi`. Snapshots default to CHOOSEV for repeatability; `--background` overrides it. On macOS, convert for viewing with `sips -s format png .local/exploration/menu.ppm --out .local/exploration/menu.png`. Keep all resulting retail derivatives ignored.

For the general extractor, use `python3 tools/extract_assets.py --dry-run` followed by `python3 tools/extract_assets.py`. It runs the standalone Rust tool in release mode without the app's window/audio dependencies. See [EXTRACTION.md](EXTRACTION.md) for filters, alternate source directories, and reports.

## Data guard

`tools/check_assets.py` scans tracked and non-ignored untracked files. It rejects Git-visible local media/reference directories, retail asset/cache extensions, plausible embedded EALIB archives, and recognizable standalone or embedded PIC headers. A format-name constant in the decoder is allowed; a plausible archive directory/sentinel is rejected. Pass explicit files/directories to inspect artifacts, including ignored outputs.

This is an initial guard, not proof that an artifact contains no retail derivatives. PIC has a structured header rather than ASCII magic. The guard does not decode compressed packages or identify converted art/audio. Extend it as packaging arrives. Run it against exact release contents before shipping; a debug executable check alone is not a release audit.

## Troubleshooting

- Wrong Rust version: source `~/.cargo/env`, check `command -v cargo`, and run `rustup show` from this repository. Homebrew Cargo does not honor rustup overrides by itself.
- Linker/SDK error: check `xcode-select -p` and complete any pending Xcode first-launch setup.
- No graphics adapter or display: run from a logged-in desktop session with working drivers. Compilation and tests do not require a window; the smoke test does.
- Missing reference folder: the Rust app does not need it. Missing media: an existing valid cache still runs; otherwise import your own media. See [REFERENCES.md](REFERENCES.md).

An editor with rust-analyzer is useful but optional. No global editor configuration is required.

## Terrain development loop

The main-menu Create Quick Mission action opens the original `QUIKMIS3.PIC` artwork with a theater selector and Terrain Viewer button. This is an authored shell, not the full quick-mission system. Launch it with `--quick-mission`, or skip to the world with `--viewer`.

```sh
cargo run --locked -p tore-app -- --quick-mission --smoke-test
cargo run --locked -p tore-app -- --viewer --smoke-test
cargo run --locked -p tore-app -- --viewer --no-audio
mkdir -p .local/theater-research
cargo run --locked -p tore-app -- --quick-mission --snapshot .local/theater-research/quick.ppm
cargo run --locked -p tore-app -- --capture-terrain .local/theater-research/terrain.ppm
```

The quick-mission snapshot is a headless CPU image. `--capture-terrain` requires a real display/GPU, renders the simulation pass into a 960 × 720 offscreen target, reads it back as PPM and exits without audio. It excludes the HUD; it is not a desktop screenshot. Convert locally with `sips` on macOS if desired. Both captures start at a repeatable camera pose. Keep derivatives ignored.

Controls: arrows move horizontally, Shift accelerates translation 8×, Q/E or PageDown/PageUp lower/raise, A/D turn, W/S pitch. Camera movement uses elapsed time with a 50 ms cap, clamps to the theater and stays at least 100 feet above the rendered surface. It is an inspection camera, not aircraft physics. Focus loss clears held keys. Escape returns to the creator, then Choose Activity. The viewer schedules frames while active; menus remain idle when no redraw is needed.

`terrain.rs` owns renderer-independent surface/camera data; `sim_renderer.rs` and `terrain.wgsl` own the depth-tested GPU scene and sky. Follow [the recovery notes](formats/theater.md) before extending source semantics; record approximations and native evidence. The initial renderer builds the whole selected theater mesh at startup, including when entering through the main menu. Streaming, LOD, object rendering and complete weather remain open.

### All-theater selection and text

All 16 creator entries now select/load a theater, rebuild its GPU resources, update its briefing map and reset the free camera. `--theater CODE` also works with `--viewer`, `--quick-mission` and `--capture-terrain`. Ukraine retains the original inspection pose; others start near the grid center at 28,000 feet with a terrain-clearance floor. Example: `cargo run --locked -p tore-app -- --viewer --theater EGY`.

Older Ukraine-only caches automatically re-import from default local media. For external media, refresh with `--import`. The selective all-theater pack is capped at 128 MiB / 2,048 resources; it is still a development cache. Source textures use the first three theater-code characters (TVI for TVIET); Kurile's base MM has no numbered texture placements and currently renders palette-colored height geometry.

The creator and placeholder notices now use original `ARMFONT.PIC` sans-serif glyphs; the compact viewer HUD uses `SMLFONT.PIC`. Tinted glyphs preserve source shading instead of flattening every visible pixel to white. Button labels retain original FONTACT artwork. These remain legacy raster fonts scaled with the menu; they are not resolution-independent vector text. No system font or new dependency is required. Use `--snapshot-state notice --snapshot .local/notice.ppm` to inspect the placeholder message.
