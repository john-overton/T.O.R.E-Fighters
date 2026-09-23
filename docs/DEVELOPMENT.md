# Development environment

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

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

Expect native borderless fullscreen on the monitor the window would have opened on, showing Choose Activity, and a terminal message such as `Renderer: Apple M3 (Metal, IntegratedGpu)`. The original 640 × 480 canvas scales proportionally and is letterboxed, so a 16:9 screen shows black bars either side. Close the window or use `? → Exit to Desktop`; Escape dismisses menus. On macOS, Command-Q also quits.

Alt-Enter switches between borderless fullscreen and the previous windowed size, on every screen: the menus, the Quick Mission creator, the locate screen and flight. F11 is not used for this, because it already opens the in-flight keyboard help. The choice is saved as `fullscreen` in `preferences-v1.conf`, which is now format version 5; a version 3 file still loads and starts fullscreen. `--windowed` starts in a 960 × 720 window for one run without changing the saved choice, and `--window-size`, `--smoke-test` and the captures keep their fixed-size windows as before.

On Windows a release build is a GUI application, so no console window appears behind the game. Nothing printed reaches a terminal there: `--version`, `--help`, `--import-only` output and import errors are silent on a Windows release build. Debug builds keep the console, so development output and the headless probes still print. A fatal startup error is also written to `last-error.txt` in the application data directory, next to `import-report.txt`, and that file is removed after the next successful start; on Windows release builds it is the only place the message appears.

Startup chooses randomly among all five original backgrounds; it does not run a timed slideshow. Force a variant for comparison with `--background CHOOSEV` (also accepts CHOOSEAC, CHOOSE3, CHOOSEU, CHOOSEM). The top bar moves to match each artwork's native origin. Hovering and keyboard focus are silent. An older menu-only cache requires re-import; the local default media is automatically used if available.

First launch imports automatically when it can find media on its own: the remembered source first, then `gameassets/fighters-anthology/`. Otherwise it opens the Locate Fighters Anthology screen, which lists detected sources, takes a typed path or a dropped folder, shows progress and reports failures in plain words; see [first-run import](spec/first-run-import.md). Pref reopens it as Re-import media. Use `--import <directory>` to refresh or choose other media from a terminal. `--import-only` imports and exits without opening a window/audio device, and a headless run with no usable source still fails with an actionable terminal error. Required media: reviewed `FA.EXE` alongside `FA_1.LIB` and `FA_2.LIB`; optional `FA_4B.LIB` and `FA_4D.LIB` supply flight and shell music. There is no native file dialog yet: drag-and-drop is being tried with players first.

Cache locations:

| Platform | Directory |
| --- | --- |
| macOS | `~/Library/Application Support/T.O.R.E-Fighters/` |
| Linux | `$XDG_DATA_HOME/T.O.R.E-Fighters/`, falling back to `~/.local/share/T.O.R.E-Fighters/` |
| Windows | `%APPDATA%\T.O.R.E-Fighters\` |

`TORE_DATA_DIR` overrides this directory for isolated checks, e.g. `TORE_DATA_DIR=.local/test-profile cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only`. Each import creates a versioned `menu-*.pack`; the latest valid pack is loaded, with fallback to earlier valid packs if a write was interrupted. `import-report.txt` records resource names and offsets. After a successful import or startup load, older numbered packs are automatically removed. An import is read back and validated before cleanup; failed imports leave earlier packs available. Cleanup leaves newer generations and unrelated files alone. See [cache retention](spec/import-cache.md). Imported resources never go into the executable.

## Linux and Windows

Linux: install rustup and a native toolchain. On Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libxrandr-dev libasound2-dev python3
```

Running the app also requires a graphical session and a working Vulkan or OpenGL/EGL driver. CI builds and tests without creating a window.

An Omarchy x86_64 host has also passed local build, import and Wayland/Vulkan
startup checks with Rust 1.91.1; see the [Linux setup baseline](baselines/linux-setup.md).
The app requests a high-performance compatible GPU. On the tested AMD/NVIDIA
desktop this selects the RTX 4070; requesting the integrated AMD adapter produced
a blank visible window despite successful frame submission.
The renderer must be released in the event loop's exit callback, before its
display connection closes, and keep its window alive through GPU cleanup.

Windows: install rustup from the official installer and Visual Studio 2022 Build Tools with **Desktop development with C++** and a Windows SDK. Use the MSVC Rust host toolchain. Install Python 3 and Git. Run the same Cargo commands in PowerShell; use `python` instead of `python3` where appropriate. The renderer can use Direct3D 12 or Vulkan.

## Fresh clone

Run this once, before anything else:

```sh
python3 tools/setup_dev.py
```

It sets `core.hooksPath` to the committed `.githooks/` directory and reports any
missing tool. From then on `git push` runs the everyday checks below and aborts
the push if any of them fail, so a broken commit never reaches the remote.
Git hooks live in `.git/hooks/`, which is not version controlled, so every clone
needs this step. A single push can skip the hook with `git push --no-verify`.

The hook runs everything the CI job runs that a single machine can run. Other
platforms stay in CI, which builds and tests four targets on every push:
Ubuntu 24.04, Windows 2022, macOS 14 on Apple Silicon, and macOS 15 on Intel.
The `macos-15-intel` image is the last x86_64 macOS runner GitHub will offer and
retires in August 2027; after that, Intel coverage means cross-compiling
`x86_64-apple-darwin` from an Apple Silicon runner.

CI has no display, no GPU, and no retail media, so it proves the code builds and
the tests pass on each platform. It proves nothing about what appears on screen.
Run the window smoke test on a display-capable host for rendering changes.

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
python3 tools/check_docs.py
```

On Windows, append `.exe` to both executable paths. `check_docs.py` verifies that
every Markdown file under `docs/` carries the current T.O.R.E header; `--fix`
writes missing ones. The pre-push hook and CI run all nine of these. Rust tests cover malformed formats, decompression, menu hit testing and interaction, PCM resampling, letterboxing, and extraction filesystem behavior using synthetic inputs. Python tests exercise the data guard. None requires a display, audio device, or retail files.

With a working desktop session, also run:

```sh
cargo run --locked -p tore-app -- --smoke-test
```

This uses the imported menu, a real 960 × 720 window (never fullscreen, so the presented frame keeps a known size) and GPU, prints the renderer, presents one frame without audio, and exits. It is not a headless simulation test. Normal mode waits while idle and schedules frames for short hover transitions and placeholder messages.

`--no-audio` silences a session. Normal playback uses the system's default output device, original PCM effects, recorded main/briefing playlists and the retail [situation scores](spec/flight-music.md) during flight when available. Device initialization failure is reported and the menu continues silently. Main-menu M toggles music; `Pref` exposes music/effect toggles. Music follows the saved preference into flight and freezes on flight pause. In-flight Sound still toggles effects only. No MIDI or synth is used. See [music behavior and limits](formats/music.md) and [combat audio](audio.md). Music/effects and flight display preferences are restored from `preferences-v1.conf` in the application data directory.

Profiles without saved preferences start with Music On. Saved Music On/Off still
overrides that default; imported sample ownership or a silent diagnostic run does
not choose the setting. If an older profile saved Music Off, use M or Pref to
enable it. See the [startup regression](baselines/menu-music-startup.md).

## Explore media and capture previews

```sh
python3 tools/explore_assets.py
mkdir -p .local/exploration
cargo run --locked -p tore-app -- --snapshot .local/exploration/menu.ppm
cargo run --locked -p tore-app -- --snapshot .local/exploration/pref.ppm --snapshot-state pref
```

The inventory records archive SHA-256 hashes, entry offsets, compression headers, and format counts without extracting everything. Snapshots render the native CPU menu canvas without a GPU/audio device; they are not window screenshots. States: `normal`, `hover`, `pressed`, `help`, `pref`, `multi`, and `controls`, `controls-keyboard`, `controls-mouse` and `controls-head` for the input configuration screen with a synthetic Xbox-layout gamepad, and `graphics` for the Graphics options screen with the default settings (with no GPU to ask, it shows Off, 2x and 4x anti-aliasing as available and 8x as unavailable). Snapshots default to CHOOSEV for repeatability; `--background` overrides it. On macOS, convert for viewing with `sips -s format png .local/exploration/menu.ppm --out .local/exploration/menu.png`. Keep all resulting retail derivatives ignored.

For the general extractor, use `python3 tools/extract_assets.py --dry-run` followed by `python3 tools/extract_assets.py`. It runs the standalone Rust tool in release mode without the app's window/audio dependencies. See [EXTRACTION.md](EXTRACTION.md) for filters, alternate source directories, and reports.

## Data guard

`tools/check_assets.py` scans tracked and non-ignored untracked files. It rejects Git-visible local media/reference directories, retail asset/cache extensions, plausible embedded EALIB archives, and recognizable standalone or embedded PIC headers. A format-name constant in the decoder is allowed; a plausible archive directory/sentinel is rejected. Pass explicit files/directories to inspect artifacts, including ignored outputs.

This is an initial guard, not proof that an artifact contains no retail derivatives. PIC has a structured header rather than ASCII magic. A `.tar.gz`, `.tgz` or `.tar` argument is opened and scanned member by member; MSI, DMG and AppImage are scanned as raw bytes only, which sees an uncompressed signature in the container but not a compressed payload. The guard does not identify converted art/audio. Run it against exact release contents before shipping; a debug executable check alone is not a release audit. The staged directory scan described under [Packaging](#packaging) is the real gate.

## Packaging

The release packages are built by `.github/workflows/release.yml` when a `v*`
tag is pushed, and by the same three scripts when a developer runs them by
hand. A push to any branch named `release-test/...` runs the build half only,
so the packaging path can be exercised without publishing anything; download
the four `packages-*` artifacts from that run with `gh run download`.
`workflow_dispatch` does the same once the workflow exists on `main`.

Every script takes the version from `--version`, then from the tag the workflow
is running for, then from `git describe`, then falls back to `0.0.0-dev`. A
leading `v` is stripped. Packages are written to ignored `dist/`, and the
staged bundle each package is built from stays in `dist/stage/`.

The app carries its own version. The main menu shows `T.O.R.E - vX.Y.Z` in the
lower left corner and `tore-app --version` prints it. A release build reads
`TORE_BUILD_VERSION` at compile time; the workflow sets it to the tag (or to
`git describe` on a test branch) and refuses a tag that does not match the
version in `crates/tore-app/Cargo.toml`. A build without the variable reports
the crate version. Bump `Cargo.toml` before tagging.

Build the release binaries first, stamping the same version the package will
carry:

```sh
TORE_BUILD_VERSION=0.1.0 cargo build --release --locked -p tore-app -p tore-extract
```

| Platform | Command | Produces |
| --- | --- | --- |
| Linux | `tools/package/package-linux.sh` | `dist/*.tar.gz` and `dist/*.AppImage` |
| macOS | `tools/package/package-macos.sh` | `dist/*.dmg` holding `T.O.R.E-Fighters.app` |
| Windows | `pwsh tools/package/package-windows.ps1` | `dist/*.msi` |

What each package contains:

- **tar.gz**: `tore-app`, `tore-extract`, `LICENSE`, `THIRD_PARTY_NOTICES.md`,
  `README.md`, the desktop entry and `tore-fighters.png`, which is the 256 px
  icon. Unpack anywhere and run.
- **AppImage**: `tore-app` only, with the desktop entry and the 256 px icon at
  the AppDir root under the name the entry's `Icon=` line asks for, plus a
  hicolor copy, so a desktop integrates it. `tore-extract` is a developer tool
  and stays in the tar.gz.
- **DMG**: `T.O.R.E-Fighters.app` plus a link to `/Applications`. The bundle
  carries `tore-app`, `tore-extract`, `tore.icns` and the three text files
  under `Contents/Resources`. `CFBundleIconFile` is `tore`.
  `CFBundleIdentifier` is `org.tore-fighters.app` and `LSMinimumSystemVersion`
  is 11.0, which is what the pinned toolchain targets on both architectures.
- **MSI**: a per-machine install into `%ProgramFiles%\T.O.R.E-Fighters`
  carrying both executables, `tore.ico` and the three text files, built with
  WiX v3 from `tools/package/tore.wxs`. Its `UpgradeCode` is permanent and
  `MajorUpgrade` replaces an older install rather than installing beside it.
  The installer's pages and its shortcut choices are described below.

None of these contain retail media. The app imports the player's own Fighters
Anthology copy at runtime; see [first-run import](spec/first-run-import.md).

### Application icon

The icon is the project logo, `docs/images/tore-fighters-logo.png`: a 1254 px
render of a round embroidered patch on a transparent ground. It is our own
artwork and carries no retail content.

`tools/package/build_icons.py` downscales it into the committed set under
`crates/tore-app/assets/icon/`. Regenerate it only when the logo changes:

```sh
python3 tools/package/build_icons.py
```

That needs ImageMagick 7 (`magick`) on `PATH`. Nothing else does: every build
and every packaging script reads the committed files, and CI never generates
icons. `--check` reports the committed sizes without writing, and
`--verify-ico` prints the entries of a finished `.ico`.

| File | Used by |
| --- | --- |
| `tore-16.png` … `tore-64.png` | macOS `.iconset`, and the DIB entries of `tore.ico` |
| `tore-128.png`, `tore-512.png` | macOS `.iconset` |
| `tore-256.png` | Linux tar.gz and AppImage, macOS `.iconset`, the PNG entry of `tore.ico` |
| `tore.ico` | the Windows executable, the MSI, both Windows shortcuts |

Downscaling uses a Lanczos filter, with a light unsharp pass at 64 px and below
so the small sizes stay legible. Two size decisions keep the committed set
under its 600 KB budget: 1024 px is not committed at all, because the source is
photographic and a lossless 1024 px PNG costs about 2 MB, and 512 px is
quantized to 255 colours, because it is only ever shown as a macOS Retina
512 pt icon. Every smaller size is full colour. The budget is enforced by the
script itself.

`tore.ico` holds 16, 32, 48 and 64 px as uncompressed 32-bit DIBs and 256 px as
a PNG, which is the layout Windows documents. The 256 px entry is the bytes of
`tore-256.png`, so the two cannot drift apart.

`tools/package/tore-fighters.desktop` is the Linux desktop entry, used by both
the tar.gz and the AppImage.

### The icon inside tore-app.exe

Explorer, the taskbar and Alt-Tab read an executable's icon from an embedded
`RT_GROUP_ICON` resource, so it has to be linked into the binary. The usual
answer is a crate such as `winres`, but that is outside the dependency budget
in [AGENTS.md](../AGENTS.md), so `crates/tore-app/build.rs` writes the resource
object itself.

The script runs on every target and does nothing unless
`CARGO_CFG_TARGET_OS` is `windows` and `CARGO_CFG_TARGET_ENV` is `msvc`. On
that target it reads `assets/icon/tore.ico`, writes a Win32 `.res` file into
`OUT_DIR` holding one `RT_ICON` per image plus the `RT_GROUP_ICON` that names
them, and emits `cargo:rustc-link-arg-bins`. `link.exe` accepts a `.res` on its
command line exactly as if `rc.exe` had produced it. Linux and macOS builds are
unaffected; an unreadable or malformed `.ico` is a warning and the build
continues without an icon.

The byte layout was checked without a Windows host by running the same writer
over the committed `.ico`, parsing the result with
`python3 tools/package/build_icons.py --verify-res PATH`, and rebuilding an
`.ico` from the parsed resources: it came back byte for byte identical. That
the linker accepts the file is proven by the Windows job in
`release.yml`; that the icon then appears in Explorer is a manual check on an
installed build.

### The Windows installer

The MSI uses the WiX `WixUI_FeatureTree` dialog set: welcome, the licence, a
feature page with an install-folder Browse button, a confirmation page, then
the finish page.

The feature page offers three entries. **T.O.R.E-Fighters** is the program and
cannot be deselected. **Start menu shortcut** and **Desktop shortcut** both
start ticked and can be turned off. Each shortcut lives in its own component
keyed on a value under `HKCU\Software\T.O.R.E-Fighters`, which is what ICE38
and ICE43 require of a component holding a non-advertised shortcut. The
shortcuts are not advertised because an advertised shortcut cannot carry its
own icon.

The finish page offers a **Launch T.O.R.E-Fighters** checkbox, which runs the
installed executable through `WixShellExec` from `WixUtilExtension`, as the
installing user rather than as the elevated installer. `candle` and `light` are
both given `-ext WixUtilExtension` alongside `-ext WixUIExtension`; both ship
with WiX 3.14 on the `windows-2022` runner.

`ICE61` is the only suppressed validation check. It rejects
`AllowSameVersionUpgrades`, which we want so that reinstalling the same version
replaces the existing install instead of stacking a second copy.

### appimagetool

`appimagetool` is published only under a moving `continuous` tag, so the script
downloads it and checks it against a SHA-256 pinned in `package-linux.sh`. When
upstream rebuilds the asset the hash stops matching and the script refuses to
run it; review the new build and update the pin deliberately. If the tool is
already on `PATH` that copy is used instead. Locally, no AppImage is a warning
and the tar.gz still succeeds; in CI it is a failure.

### Unsigned builds and first launch

Nothing is signed or notarized. Signing is a later change that does not alter
the package layout.

- **macOS**: double-clicking an unsigned app is refused. The first launch is
  right-click (or Control-click) on `T.O.R.E-Fighters.app`, choose **Open**,
  then **Open** again in the dialog. After that it launches normally.
- **Windows**: the executable links the C runtime statically
  (`.cargo/config.toml`), so no Visual C++ Redistributable is needed.
  SmartScreen warns about an unrecognized publisher. Choose
  **More info**, then **Run anyway**. This applies to the MSI itself.
- **Linux**: mark the AppImage executable (`chmod +x`) if the browser cleared
  the bit.

### What CI validates, and what it does not

The release workflow runs `cargo fmt`, Clippy, the workspace tests, the Python
tool tests and the documentation header check on all four images, builds the
release binaries, scans them, then runs the platform script. Each script scans
its staged directory before building and scans the finished package after. The
release job scans every downloaded package once more before uploading it to the
GitHub release, which is created as a pre-release if it does not exist.

That proves the packages build and carry no detectable retail data. It does not
prove they install. Installing the MSI on Windows, opening the DMG and running
the app from `/Applications` on both macOS architectures, and running the
AppImage on a machine that is not the build host are manual checks, and they
stay recorded as pending until someone performs them on a tag. The same is
true of everything the icon is for: the Windows job proves `link.exe` accepts
the generated `.res`, but only an installed build shows whether Explorer, the
Start menu, the desktop shortcut and the macOS Dock draw the icon.

## Troubleshooting

- Wrong Rust version: source `~/.cargo/env`, check `command -v cargo`, and run `rustup show` from this repository. Homebrew Cargo does not honor rustup overrides by itself.
- Linker/SDK error: check `xcode-select -p` and complete any pending Xcode first-launch setup.
- No graphics adapter or display: run from a logged-in desktop session with working drivers. Compilation and tests do not require a window; the smoke test does.
- Missing reference folder: the Rust app does not need it. Missing media: an existing valid cache still runs; otherwise import your own media. See [REFERENCES.md](REFERENCES.md).

An editor with rust-analyzer is useful but optional. No global editor configuration is required.

## Terrain development loop

The main-menu Create Quick Mission action opens the original `QUIKMIS3.PIC` artwork with editable briefing fields and standard/custom weapons selection. The terrain inspection camera is now a CLI diagnostic. This supports airborne missions with straight-flying practice aircraft; combat AI and mission objectives remain open. Launch it with `--quick-mission`, or skip to the world with `--viewer`.

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

Older Ukraine-only caches automatically re-import from default local media. For external media, refresh with `--import`. The creator/all-theater pack is capped at 256 MiB / 4,096 resources; it is still a development cache. Source textures use the first three theater-code characters (TVI for TVIET); Kurile's base MM has no numbered texture placements and currently renders palette-colored height geometry.

Quick Mission and Load Ordnance use a bundled, open-licensed Noto Sans Bold raster atlas for clean flat text, with beveled field boxes. See the [menu presentation spec](spec/quick-mission-menu.md) and [font provenance](../crates/tore-app/assets/README.md). Top menu bars retain MENUFONT and center labels using visible glyph bounds. Other menus retain their original imported fonts. No system font or new runtime dependency is required. Use `--quick-mission --snapshot .local/quick.ppm` to inspect the page.

## Hornet free flight

Create Quick Mission supports all imported aircraft with accepted weapons, ammunition
and fuel, and up to 29 straight-flying dummies from the six wing selectors. Select
no ground targets/defenses. Select a custom load to open Load Ordnance; standard
uses reviewed PT defaults. Direct `--free-flight` loads supported default weapons
with full fuel; restricted native research flight remains clean.
Selected creator altitude is retained exactly or rejected for terrain clearance.

```sh
cargo run --locked -p tore-app -- --free-flight --theater UKR
cargo run --locked -p tore-app -- --free-flight --theater TVIET --flight-view 1
cargo run --locked -p tore-app -- --headless-flight 1200 --maneuver pull
```

Arrows: pitch/bank (Down pulls up). Z/X: rudder. PageUp/PageDown: throttle; 1..9/0 selects 10..90%/full. Shift-B: afterburner (requires >95% throttle). E: engine. G/F/B/H: gear/flaps/airbrake/hook. J: jammer. R: radar power, or return to the radar channel when infrared is selected. M/O: cycle the available sensor channels; I: infrared; Y: scope contact history; comma/period: scope setting. F1/F2/F3: front/back/up; F10: external. Shift-0..9: instrument windows (four large or six small). Backspace toggles cockpit art. Escape opens the paused in-flight menu; Ctrl-P pauses/resumes; Ctrl-Q ends flight. Focus loss pauses automatically. On Macs use Fn/Globe for function keys and Fn-Up/Down for PageUp/PageDown. See [all current controls and source status](FLIGHT-CONTROLS.md).

Instrument window numbers: 0 radar cross section, 1 envelope, 2 front view, 3 other view, 4 target, 5 RWR, 6 navigation, 7 systems, 8 weapons, 9 radar. Page 0 draws the exposure contour and received emitters with `-`/`+` scale buttons; page 9 has `-`, `+`, `M` for the sensor channel and `Y` for history, and a click on a contact designates it. Unsupported readings and no-target states remain explicit. Older Hornet-less caches refresh from local media. `--viewer` remains a developer terrain diagnostic but has no creator button. Source data, authored integration and remaining parity work are distinguished in [aircraft notes](formats/aircraft.md); [baseline commands](baselines/f18-free-flight.md) cover screenshots and tests.


The full-height cockpit and live HUD can be captured with `--capture-flight .local/cockpit.ppm`. Add `--flight-menu` to capture the paused Escape menu. `--flight-view 0|1|2|3|4` selects front/chase/oblique/back/up for inspection. These flags require a display for GPU capture. See [cockpit/control validation](baselines/cockpit-controls.md).


Flight UI now adapts to drawable aspect ratio independently of menu letterboxing. Use `--window-size 1280x720` (or resize normally) to inspect widescreen behavior. `--capture-flight` preserves the current aspect and writes at the flight overlay resolution, capped proportionally at 1920×1080. The original `--capture-terrain` remains 960×720. Small instruments resample directly from their native rasters; HUD readouts have transparent backgrounds. See [responsive-flight checks](baselines/responsive-flight-ui.md).

## Headless development

Run from the repository root with the pinned Rust toolchain and `--locked`.
`cargo run` uses the dev profile by default, retaining debug symbols and
assertions; no `--dev` flag is needed. Keep runtime data separate from normal
play by setting `TORE_DATA_DIR` to an ignored directory under `.local/`.

On Linux/macOS shells, prepare an isolated profile and output directory:

```sh
mkdir -p .local/headless
export TORE_DATA_DIR="$PWD/.local/dev-profile"
cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only --no-audio
```

The import command reads local user-owned media, creates the cache and exits
without opening a window or audio device. Run it once for a new profile; later
commands reuse that cache. See [cache locations and refresh](#macos-setup).

For a deterministic flight-model probe, run 1,200 fixed simulation ticks
(10 simulated seconds):

```sh
cargo run --locked -p tore-app -- --headless-flight 1200 --aircraft f18 --maneuver level --no-audio
```

This prints flight state and exits before window/audio initialization. It is
the isolated flight-model probe, not a complete rendered mission. Keep the
default researched flight adapter unless the task explicitly concerns another
adapter. Use `--aircraft rafale` for the exact Rafale C identity.

For a CPU-rendered Ordnance screen capture:

```sh
cargo run --locked -p tore-app -- --quick-mission --snapshot-state ordnance --snapshot .local/headless/ordnance.ppm --no-audio
```

The snapshot command also exits without a display or audio device. Other menu
states, including `ordnance-empty` and `ordnance-drag`, use the same command.
Keep captures, logs and imported media under ignored directories. When using
`target/debug/tore-app` directly, first rebuild with
`cargo build --locked -p tore-app` so the binary matches the source.

`--no-audio` only disables sound. A plain app launch, `--free-flight`,
`--flight-probe-ticks` by itself, GPU `--capture-flight`/`--capture-terrain`, and
`--smoke-test` still need a display. Do not use those as generic headless
commands. The environment and other feature probes have their own exit paths;
follow their documented commands elsewhere in this guide.

In PowerShell, create `.local/headless` with `New-Item -ItemType Directory -Force .local/headless`,
set `$env:TORE_DATA_DIR` to the absolute `.local/dev-profile` path, and run the
same Cargo commands. Remove the environment override after the session with
`Remove-Item Env:TORE_DATA_DIR`; on Linux/macOS use `unset TORE_DATA_DIR`.

## Graphics options

Pref → Graphics... sets anti-aliasing, render scale, the spotting aid and terrain filtering; see [graphics options](spec/graphics-options.md). For one run, without saving, use `--anti-aliasing off|2x|4x|8x`, `--render-scale 75|100|125|150|200`, `--spotting-aid off|subtle|strong` and `--terrain-filtering on|off`; `--original-graphics` turns every addition off at 100%. Flags apply in order. Captures and smoke tests ignore the saved `graphics-v1.conf` and use the defaults plus any flags, so compare `--original-graphics` against no flag for matched before/after images.

## Flight performance

Normal `cargo run --locked -p tore-app -- --free-flight` now optimizes the app crate at level 2, retaining debug symbols/assertions. Dependencies keep their existing debug settings; Cargo may still label the overall dev profile “unoptimized.” No release build is required to benefit. Simulation remains fixed at 120 Hz; presentation interpolates its last two poses and requests uncapped Immediate presentation, then Mailbox, with FIFO only as a supported-mode fallback and one requested queued frame. There is no additional 16 ms sleep in flight/viewer mode. The display/compositor can still limit presentation frequency.

Run a bounded desktop measurement (macOS/Linux shell):

```sh
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 TORE_PERF_VIEWS=1 cargo run --locked -p tore-app -- --free-flight --no-audio --window-size 1280x720
TORE_PERF_FRAMES=180 TORE_PERF_ACTIVE=1 cargo run --locked -p tore-app -- --free-flight --no-audio --instrument-page 3
```

On PowerShell, set `$env:TORE_PERF_FRAMES="330"`, `$env:TORE_PERF_ACTIVE="1"`, and `$env:TORE_PERF_VIEWS="1"`, run the same Cargo command, then remove those environment variables. `TORE_PERF_FRAMES` accepts 60–100000 frames (0/off by default), prints mean/p50/p95/max milliseconds and exits after that many flight or terrain-viewer frames. The first 30 are excluded. Presence of `TORE_PERF_VIEWS` cycles front/back/up/chase/oblique every 30 frames; omit it to measure the selected view or switch views manually. Use `--free-flight` for aircraft measurements or `--viewer` with a fixed `TORE_WEATHER_VIEW` for terrain measurements. Avoid concurrent GPU workloads when comparing runs. Presence of `TORE_PERF_ACTIVE` explicitly keeps the bounded flight benchmark unpaused even if automation steals focus (it does not dismiss menus). Omit it for normal pause behavior and interactive pause measurements. The report includes paused frames and completed live camera readbacks; check these before interpreting a run as active flight.

The report separates frame-start intervals, simulation/camera work, UI composition, and submission/presentation. These are CPU wall-clock measurements: presentation includes VSync/backpressure, and frame intervals are not verified display scanout times or GPU timestamps. A short run does not establish sustained thermal performance. See [measured baseline and remaining work](baselines/flight-performance.md).

F2/F3 are the recovered look-back/look-up bindings, **not exterior cameras**. Use F10 for the aircraft chase view, or `--flight-view 2` for the diagnostic oblique view. F1 returns to the cockpit. On macOS, use Fn/Globe if function keys invoke system actions.

### Look-around checks

Shift/Ctrl + arrows look around in the cockpit or orbit around the aircraft externally. Cockpit Down stops at the forward eye line; exterior orbit is unrestricted in both axes. Shift-/ recenters; F1 returns to the cockpit. For repeatable captures, `--flight-look YAW,PITCH` supplies degrees in -360..360 (finite values only); internal pitch clamps to 0..90. Example: `cargo run --locked -p tore-app -- --flight-view 1 --flight-look 120,-65 --capture-flight .local/orbit.ppm`. This flag sets orientation only; use `--free-flight` or a flight capture to enter flight. See [controls and limitations](FLIGHT-CONTROLS.md#look-around-and-exterior-orbit).

### Loop regression probe

`cargo run --locked -p tore-app -- --headless-flight 10800 --maneuver loop` starts the F/A-18D at 450 KTAS / 5,000 feet with full throttle and afterburner, then holds pull until it completes a loop or reaches the tick budget. It reports vertical/inverted/completed flags and the final state. This is an authored-adapter regression probe, not native flight-model acceptance. Use `--flight-look 0,90 --capture-flight .local/zenith.ppm` to inspect the sky directly overhead; the forward cockpit plane moves out of view naturally. [Baseline](baselines/flight-response-sky.md).

## Directional cockpit checks

The original forward cockpit and HUD now share a flat GPU overlay that translates opposite head-look around the aircraft-forward datum, with a side/up fade. Inspect with `--free-flight --flight-look 8,4`, `--flight-look 40,5`, and `--flight-look 0,35`; add `--capture-flight .local/directional.ppm` for a repeatable GPU capture. Check both `--window-size 1280x720` and `--window-size 640x900`. F1 restores the centered frame; F2 should not repeat forward art behind the pilot. Instrument windows and the Escape menu remain screen-anchored. See [asset limits and measurements](baselines/directional-cockpit.md).

## Aircraft animation inspection

Use `--flight-devices G,F,B,H,AB` for initial fractions (0..1) and `--flight-controls pitch,roll,rudder` for initial visual deflections (-1..1). Combine with `--capture-flight` to pause at an exact pose. Without a capture, normal actuator and control response resumes after startup. See [examples and evidence](baselines/f18-animations.md).

## Banked-pull probes

`--headless-flight 360 --maneuver bank-left` (or `bank-right`) tests three seconds of pull from a 45-degree bank and reports AoA/sideslip. `--maneuver bank-right --flight-probe-ticks 120 --capture-flight .local/bank.ppm` captures the same maneuver after one second; rendered probes are limited to one minute. See [flight-model evidence](baselines/banked-pull-aoa.md).

## Native flight helper probes

`cargo run --locked -p tore-app -- --native-flight-report` prints deterministic
static-translated helper probes using the imported Hornet profile, without a
window. This does not select a different playable flight model. Recreate the local
executable/symbol inventory with the commands in [native flight research](formats/native-flight.md).

The native flight report now prints a typed PT profile, supplied-condition stall/spin probes and ordered velocity-step probes. Its forward limit uses the reviewed 1G-envelope update at a supplied 5,000-ft altitude. It is not a native flight trajectory. No display is required; the normal free-flight adapter remains unchanged.

Use `--native-flight-trig .local/native-flight/rotations-final/tables/sine-q15.bin` after static extraction to include imported-table rotation and force probes. The flag enables the headless native report. It validates exactly 642 bytes and does not alter free-flight behavior.

Fourth-pass static composition probe (no window, audio or retail-code execution):

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-flight/composition-final
cargo run --locked -p tore-formats --example native_composition -- .local/native-flight/composition-final/tables/sine-q15.bin .local/native-flight/composition-final/tables/atan-pa.bin
```

The example validates table lengths, prints supplied world/cockpit angle probes,
and checks that 120 authored fixed-clock steps account for 256 native time units.
Its seeded RNG draws and sample inputs are diagnostics, not recorded native
trajectories. It does not change the app's flight model.

## Aircraft selection and briefing selectors

Quick Mission uses all recovered active scalar tables and imported aircraft names.
Click scalar text to cycle; Shift-click opens a paged list. Aircraft and theater
fields open the list directly. Aircraft choices are restricted to imported flight
profiles for F/A-18D, Rafale C, F-14D, A-4E and X-31 EFM.
See the [menu presentation spec](spec/quick-mission-menu.md). Unsupported mission
systems remain editable setup placeholders with launch validation.

```sh
cargo run --locked -p tore-app -- --quick-mission --aircraft rafale
cargo run --locked -p tore-app -- --free-flight --aircraft rafale --theater FRA
cargo run --locked -p tore-app -- --aircraft rafale --headless-flight 10800 --maneuver loop
cargo run --locked -p tore-app -- --quick-mission --snapshot-state aircraft --snapshot .local/aircraft-selector.ppm
cargo run --locked -p tore-app -- --quick-mission --snapshot-state theaters --snapshot .local/theater-selector.ppm
```

`--aircraft f18` remains the default. Quick-mission snapshot states are `normal`,
`aircraft`, `theaters`, `ordnance`, `ordnance-empty`, `ordnance-drag`, `help`,
and `debrief-1` through `debrief-5` (the retail reference result on DEBSCV);
they use the original 640×480 menu canvas. The empty/drag ordnance fixtures
support CPU snapshots for inspecting card outlines and the carried thumbnail.
`ordnance-message` previews the Cheat notice; `ordnance-message-long` previews
the single-line notice's ellipsis and fitted background width.
Old caches re-import when local media is present. Aircraft switching refreshes
GPU atlas/cockpit resources, camera previews and instruments before launch.
See [validation and remaining parity](baselines/rafale-quick-mission.md).

Rafale animation inspection: use `--aircraft rafale --flight-view 2
--flight-devices 1,1,1,0,1 --flight-controls 1,0,1 --capture-flight
.local/rafale-deployed.ppm` (on one command line). The fourth fraction must be
zero: the imported model has no hook. [Animation and cockpit-switch evidence](baselines/rafale-animations.md).

## Shared flight-model development

`tore-sim` has no renderer/audio/platform dependency. It owns flight state and
attitude math shared by the app and headless tools. Use `--researched-flight` to
explicitly select the default hybrid model; use `--legacy-flight` for the old adapter. Extract
and validate either reviewed aircraft with `tools/extract_assets.py --aircraft
f18|rafale --validate-flight` (choose one literal identity).
See [FLIGHT-MODEL.md](FLIGHT-MODEL.md) for complete commands, surface inputs,
acceptance scenarios, and the explicit fitted/native boundary.

Use `--flight-zoom 0.5..4` for repeatable initial zoom, including `--capture-flight`. See [cockpit sliding/zoom evidence](baselines/cockpit-slide.md).

## Live cockpit mirrors

F/A-18D and Rafale C now render all three original mirror regions from a shared
768×384 rear GPU view every visible flight frame. There is no mirror refresh
rate cap or CPU readback. The aircraft is visible in the rear feed, while its
exterior stays hidden from the forward cockpit camera. Mirrors share cockpit
pan, zoom and fade. Source silhouettes are flood-filled at runtime from reviewed
opaque fills; optics and viewpoint are fitted. See [mirror evidence](baselines/mirrors.md).

Presentation requests Immediate, then Mailbox if Immediate is unavailable, then
FIFO as the portable fallback. Uncapped modes omit Wayland frame callbacks that
otherwise throttle redraw requests to compositor refresh. No compositor settings
are changed. Platforms/drivers may still pace presentation; the selected mode is
printed at startup. Physics remains fixed at 120 Hz.

For a matched diagnostic without mirrors, set `TORE_MIRRORS=0` at launch; original
flat fills remain. Omit it for normal live mirrors. Combine with
`TORE_PERF_FRAMES=6030 TORE_PERF_ACTIVE=1` and the same aircraft/window size.
The report includes rear-render counts. Instrument camera pages retain their
separate asynchronous readback cadence; that does not pace cockpit mirrors.

## Controller development

See [INPUT.md](INPUT.md) for the standard Linux gamepad mapping, custom profile
syntax, instrument focus, input tapes, disconnect semantics and feedback limits.

```sh
cargo run --locked -p tore-app -- --list-inputs
cargo run --locked -p tore-app -- --monitor-inputs 30
cargo run --locked -p tore-app -- --write-input-profile my-input.conf
cargo run --locked -p tore-app -- --input-profile my-input.conf --free-flight
```

Device diagnostics require neither retail media nor a display. Generated profiles
are create-new. Store a selected profile as `input-v1.conf` in the application data
directory for automatic loading. No desktop/udev permissions or drivers are
modified. `--no-controllers` disables native device access. Windows/macOS raw
controller mappings require an explicit profile; their backend cross-checks are
not hardware/runtime acceptance. macOS 11+ supported gamepads use GameController
and CoreHaptics; generic HID feedback and directional flight-stick forces remain
open. Apple gamepad IDs are session-only; see INPUT.md for shared profiles. Test
a single capable controller with `cargo run --locked -p tore-app -- --test-rumble only`.

`tore-input` owns safe, dependency-free binding policy and typed pilot input;
`tore-input-native` isolates the native platform boundary. Continue the everyday
workspace checks above. On a Linux development host with rustup targets installed:

```sh
cargo clippy -p tore-input-native --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
cargo clippy -p tore-input-native --all-targets --locked --target aarch64-apple-darwin -- -D warnings
```

These check native backend Rust/FFI declarations but do not link or run the complete
app on those operating systems. [Current evidence](baselines/input.md).

### In-game controls and preferences

**Escape → Control** opens the binding/rumble editor; use **Save & apply** to persist
changes. Normal sessions automatically save instrument layouts/pages and scope
settings, cockpit/HUD/zoom and sound choices. Aircraft changes and restarts retain
these choices. [Editing, file behavior and platform limits](INPUT.md).

`--controls-menu` opens the paused editor directly for inspection. Reproducible
wide/tall captures use `--controls-menu --window-size 1280x720|720x1000
--capture-flight PATH` with one literal size. Diagnostic windows are non-resizable
to preserve requested dimensions on tiling compositors; normal windows remain
resizable. Smoke/capture/performance diagnostics ignore saved display preferences.

## Combat component research

`tools/extract_assets.py --aircraft f18 --aircraft rafale --weapons` selects both
reviewed aircraft and the armament catalog. Exclude unrelated demo archives as
shown in [extraction instructions](EXTRACTION.md#fa-18d-and-weapons).
`--native-weapons` performs the separate hash-gated static code pass.
`cargo run --locked -p tore-sim --example weapon_probe -- PATH.JT` exercises
recovered scalar components from an extracted definition. These research commands do not
prove native lifecycle parity. The separate `--live-fire --aircraft f18|rafale`
app mode runs the development range. `--combat-smoke --aircraft f18|rafale` runs
its imported end-to-end suite headlessly; use one literal identity per invocation.
`--combat-probe-ticks N` with `--capture-flight PATH` captures a scripted live pass.
[Live-fire controls and validation](baselines/live-fire.md). [Evidence and remaining gates](baselines/combat-components.md).

### Manual weapon acceptance

`--combat-smoke` exercises the selected aircraft's default JT slots against all
five source damage entries, with negative launch checks and deterministic
live-state comparison. All twelve registered identities pass, and scripted probes
now wait the same half second for a fire-control track that a player does. Set `TORE_COMBAT_EVIDENCE` to a
new ignored directory to additionally write and replay per-slot combat tapes,
comparing complete state before and after manual commands/reset:

```sh
TORE_COMBAT_EVIDENCE=.local/manual-check TORE_DATA_DIR=.local/combat-implementation/app-profile cargo run --locked -p tore-app -- --combat-smoke --aircraft f18
TORE_COMBAT_EVIDENCE=.local/manual-check TORE_DATA_DIR=.local/combat-implementation/app-profile cargo run --locked -p tore-app -- --combat-smoke --aircraft rafale
```

Files use create-new semantics; choose a fresh output directory on subsequent
runs. The application-data override is optional and must point to an imported
profile or permit import from local media. Runtime `--record-combat NEW_PATH`,
headless `--replay-combat PATH`, and `--combat-command NAME` capture setup are
specified in [manual weapons acceptance](baselines/manual-weapons.md).

The [systems continuation](baselines/weapons-systems.md) adds incoming/player-damage
and ECM checks to the combat smokes, combat tapes, `--jammer-on`,
and `--combat-command damage|incoming|target-jammer`. Tapes are now version 3:
each record also carries the player's sensor controls, and a designation is
stored as `designate-id:N` rather than a screen coordinate. Version-2 tapes still
replay with the default sensor controls. Probe logs include haptic
event/mixer counts without playing historical pulses on hardware. Use a new
`TORE_COMBAT_EVIDENCE` directory for ten serialized slot tapes. Current native
research has 28 reviewed regions; full subsystem/ECM parity remains open.

## Creator / ordnance acceptance

```sh
cargo run -q --locked -p tore-app -- --sensor-summary
cargo run --locked -p tore-app -- --validate-creator
cargo run --locked -p tore-app -- --validate-weather
TORE_WEATHER_TIME=19:06 cargo run --locked -p tore-app -- --capture-terrain .local/weather/dusk.ppm
TORE_VAPOR_PROBE=1 cargo run --locked -p tore-app -- --free-flight --maneuver pull --flight-probe-ticks 400 --smoke-test
cargo run --locked -p tore-app -- --weather-condition 1 --capture-terrain .local/weather/cloudy.ppm
cargo run --locked -p tore-app -- --quick-mission --snapshot-state ordnance --snapshot .local/ordnance.ppm
cargo run --locked -p tore-app -- --quick-mission --snapshot-state ordnance --smoke-test
```

`--sensor-summary` needs imported media but no display or audio. It prints one
line per registered aircraft: the installed radar record with its search and
track volumes, look-down coefficient and assigned preset; the infrared and visual
records; the ECM record with its assigned generation and strength; and the PT
radar and infrared signatures. Reviewing that output is the expected cost of
porting an aircraft's sensors. For repeatable headless captures,
`--sensor-channel radar|ir`, `--scope-range 5|10|25|50|100|150` and
`--scope-history` set the scope before the capture. See
[the component guide](radar.md) and [its validation](baselines/radar.md).

`--validate-creator` needs imported media but no display/audio, and checks all
imported aircraft's supported placements, fuel, empty stations and accepted-ammo restart.
It first checks guns-only launch/restart across all six wings and the ordnance drag
paths for the full selectable roster. See the [current results and unrelated damage
assertion](baselines/ordnance-presentation.md).

Load Ordnance shows only imported weapons with connected flight support. Normal
loading also requires compatibility with at least one aircraft station. Weapons
→ Cheat unloads all stations and shows every supported imported weapon; toggling
it off unloads again and restores the normal catalog. Both category pages and
the selected catalog card reset on each toggle. See
[catalog availability](spec/ordnance-presentation.md#catalog-availability) for
placement limits and [future weapon passes](ROADMAP.md#weapon-catalog-update-passes).

In Load Ordnance click a catalog weapon then a compatible station, or drag between
them. The weapon thumbnail follows the pointer. Drag from station to station
to transfer ammunition, or from a station into the catalog to empty it.
Left-click a loaded station to add one, or an empty station to load the selected
catalog weapon. Tab changes selected station; +/- changes ammunition;
right-click decrements.
Fuel rocker edits 500 lb at a time. Successful edits play the original weapon,
ammunition or fuel sound; canceled and unchanged edits are silent. Older caches
refresh these samples from local media through the existing import path.
Select Plane preserves the custom draft.
[Evidence, hands-on steps and material limits](baselines/creator-ordnance.md).

### Weather implementation audit

The [2026-09-15 review](baselines/weather-review.md) records corrections to the
initial weather implementation and remaining parity gaps. The creator exposes
six weather rows; overcast is omitted as a duplicate of cloudy. CLI source
indices remain `0..5` (clear, cloudy, foggy, dawn, sunset, night).
`tools/inspect_shape_effects.py EXTRACTED.SH` emits a static import/re-entry
candidate inventory; optional `--disassembly NEW_LOCAL_FILE` requires GNU
objdump. It never executes imported shape code and does not prove effect absence.

### Weather inspection cameras

`TORE_WEATHER_VIEW=x,y,z,yaw,pitch[,roll]` sets the terrain viewer pose in feet and
absolute degrees for reproducible sky/deck captures. It does not change the
flight camera. Optional roll permits reproducible celestial bank checks. For example:

```sh
TORE_WEATHER_VIEW=1070000,5000,590000,45,20 cargo run --locked -p tore-app -- --viewer --weather-condition 5 --capture-terrain .local/moon.ppm --no-audio
```

`TORE_CLOUD_ALTITUDE=0..400000` overrides the cloud-sheet altitude in feet for
crossing captures. Generated weather choices use the recovered scattered-cloud
chance; normal MM launches preserve their `clouds` field. Caches predating the
inert cloud layout automatically re-import when local reviewed media is present.

`TORE_SUN_GLARE=0|1` disables/enables the recovered glare and palette whitening
(default on in this host adapter). In flight, **Escape → Cheat → No sun
whiteout? → On** suppresses both whitening and lens flare immediately, including
while paused; the sun itself remains visible. The cheat lasts for the session
and across restarts, without changing saved preferences. `TORE_SUN_GLARE=0`
keeps glare disabled regardless of the cheat. Camera-specific weather sampling
and validation are described in [weather cameras](baselines/weather-cameras.md).

`TORE_CLOUD_DETAIL=0|1|2` selects recovered cloud candidate placement (default 2).
Levels 0/1 use the base period; level 2 uses the 4x4 repeat. Native coordinate
range and view-sector gates can make the visible results identical in forward
views. Use a downward view to inspect copies around the aircraft. This override
is cloud-specific; the lower-detail terrain/sky raster is not selected by it.


Weather captures use `--capture-terrain` for the viewer and `--capture-flight`
for flight with overlays. `--capture-terrain` selects the viewer even when
`--free-flight` occurs earlier; use the flight flag for aircraft/cockpit evidence.
The latest ignored `.local/weather-continuation/final/manifest.json` records
commands, explicit weather poses and result codes for the continuation captures.
Weather sky/ocean, moon and cloud textures now use source point-index samples;
float GPU projection remains qualified against native coverage/behavior.

`TORE_WEATHER_SMOOTH=0|1` selects stepped or smooth weather presentation (default
1). Smooth mode blends source time/altitude colors and neighboring horizon/fog
shades. It uses fractional mission time, freezes on pause and does not change
simulation/callback scheduling. `--validate-weather` also reports native/smooth
palette change counts and maximum channel steps over a minute of dawn.
[Evidence and celestial sizing qualifications](baselines/weather-smoothing.md).
Smooth weather also adds an opinionated directional sun halo, dawn/dusk wash,
and per-pixel angular lighting on sky-deck textures and cloud sheets. Its visual
sun arc runs continuously through the day, with symmetric dawn/dusk twilight
gradients rather than a 19:00 drawing cutoff. The Sunset preset remains 19:01.
The sun and moon use half their previous apparent diameter (shared scale 2).
The trial orange-rim grade is removed; the earlier atmospheric glow remains. Smooth-mode glare
is gentler near the horizon and drops rapidly to zero by 0.5 degrees below it.
See [sun glow specification](spec/sun-glow.md).

Smooth weather also adds gentle distance and altitude-dependent horizon blending,
with extra haze along sightlines through moist weather bands. See the
[atmospheric distance specification](spec/atmospheric-distance.md).
Dense cloud layers now suppress residual surface colors, and cloudy weather
without an ocean deck receives original-art ripple reflections. See
[cloudy presentation checks](baselines/cloudy-presentation.md).

### Wind, turbulence and attachment probes

`TORE_WIND=heading,speed` supplies whole degrees (0..360) and feet/second
(0..200); `0,0` is explicit calm. Without an override or mission line, native-order
heading/speed draws use an isolated launch seed of 1. Both adapters start with
the wind added to ground velocity to preserve starting TAS.
`TORE_TURBULENCE=0|1` sets the initial session preference; the flight Cheat menu
can change it and restart preserves it. `TORE_FLIGHT_AGL=10..90000` sets
diagnostic starting height above terrain, not a validated runway.

```sh
TORE_ENVIRONMENT_PROBE=1 TORE_FLIGHT_AGL=100 TORE_WIND=90,20 target/debug/tore-app --free-flight --aircraft f18 --flight-probe-ticks 600 --no-audio
TORE_VAPOR_PROBE=1 TORE_WIND=0,0 target/debug/tore-app --free-flight --aircraft rafale --maneuver pull --flight-probe-ticks 400 --smoke-test --no-audio
```

The environment probe exits before window creation and prints resolved wind,
live AirData, turbulence state and final pose after the full fixed-tick service.
It differs from the isolated `--headless-flight` dynamics probe. AirData uses
a declared standard atmosphere; unavailable IAS/CAS/indicated altitude remain
unavailable. The vapor probe includes each raw CE point and nearest neutral
mesh vertex. [Acceptance](baselines/wind-turbulence-vapor.md).

### Flight-response diagnostics

The renderer-independent `response_probe` example covers both adapters for each
provided extracted PT, including applied G/rates, roll/rudder release, stall/spin
and recovery, low/high speed, devices, payload and full loops:

```sh
cargo run --locked -p tore-sim --example response_probe -- .local/aircraft/f18/FA_2.LIB/F18.PT .local/aircraft/rafale/FA_2.LIB/RAFALE.PT
```

Set `TORE_RESPONSE_TRACE=.local/response-traces` to record per-tick state. Traces
are source-derived local artifacts; do not commit them. See [conditions/results
and remaining gates](baselines/flight-response.md). This complements the hybrid
`--validate-flight` extraction suite; it is not a retail trajectory oracle.

### Native departure research

Use `cargo test --locked -p tore-formats tumble` for initial synthetic
native tumble/fall branch checks. The existing static extraction command now
includes their scheduling/movement slices. This diagnostic component is not
called by either live adapter. [Evidence and open gates](baselines/native-tumble.md).
Apply [behavior provenance](behavior-provenance.md) when interpreting results.
In research mode, identifying the source, testing the translation and writing the
spec are separate steps. "Runtime connection" is retired as a completion column,
and retail comparison is unavailable.

The `native_departure` example joins warning/stall/spin/tumble branches with
native movement composition. Pass extracted sine table, atan table and one or
more reviewed PT files; see [commands and scope](baselines/native-departure-stage.md).
It uses explicit scripted native-time inputs, not the full flight scheduler or
normal force update. It adds no live flight mode.

The departure example also evaluates separate native force/velocity snapshots
from each departure output, using explicit empty/fuel-off inputs. These are
component probes: they do not feed velocity back into a full flight trajectory.

`cargo test --locked -p tore-formats normal_control` and
`cargo test --locked -p tore-formats movement_stage` check the diagnostic primary
control and movement/contact contracts. `native_departure` now evaluates paired
force→movement snapshots with both PTs; it still does not run a full native flight.
[Scope and results](baselines/native-movement-control.md).


`native_flight` now joins loaded controls, departure, forces, movement and contact
into recurrent diagnostic updates for F18 and Rafale. Unlike `native_departure`,
it feeds position, velocity and control state back into subsequent services.
Pass the same sine/atan/PT arguments. It requires native environmental turbulence
disabled and explicit caller samples; it adds no live adapter or terrain producer.
[Reproduction, scenario coverage and remaining gates](baselines/native-flight-diagnostic.md).


### Airborne native research flight

`--native-flight-tables DIR` connects the joined service to live free flight;
DIR supplies bounded `sine-q15.bin` and `atan-pa.bin` files from the static native
extraction pass. Use it with either `--aircraft f18` or `--aircraft rafale`, and
with `--headless-flight`/`--flight-probe-ticks` for repeatable checks. It is mutually
exclusive with hybrid and combat modes. Two limits belong to this restricted
research path only: contact stops the run, and environmental turbulence is
unavailable here. Existing device/fuel/clock adaptation remains explicit.
[Commands, restart/failure semantics and acceptance](baselines/native-live-flight.md).
`cargo run --locked -p tore-sim --example native_live -- SINE ATAN PT [PT]` runs
both-aircraft live-API replay checks independently of the importer hybrid suite.

## Ocean motion inspection

Ocean decks use the original ocean and sky textures and weather palette, with
short procedural ripples and angle-dependent reflection. Close detail is
pixelated and blends to smooth sampling with distance and altitude. Smooth
reflection and fixed-size ripples blend back into the original water sample
from 2,700 feet to five statute miles of horizontal distance. Reflection strength
also follows that fade, reducing distant highlight contrast. Whitecaps
are removed. `TORE_OCEAN_MOTION=0` restores static ocean sampling for comparison;
the default is `1`. `TORE_OCEAN_PHASE=SECONDS` freezes motion at a finite phase in
`0..120` for captures. Ordinary playback uses shared simulation time, including
pause/restart semantics, in the viewer, flight, mirrors and camera instruments.
No additional texture import is required.
[Behavior and provenance](spec/ocean.md), [checks and commands](baselines/ocean.md).

Smooth water reflections separate an 85% sun peak from the sky/cloud peak.
`TORE_WATER_ENV_REFLECTION=0..1` overrides the selected sky/cloud default of 0.3; viewing angle, distance and weather reduce actual contributions. See [reflection validation](baselines/reflection-luminosity.md).

The sun reflection has independent long-range angular scatter and follows the
visible fraction of the sun disc. Its reach no longer uses the five-mile
sky/cloud reflection fade; dense weather still obscures it.

Missile reach probes use `--missile-acceptance --aircraft NAME` and print measured
hits, acquisition inhibits and expiry misses. These are controlled fixtures,
not guaranteed effective ranges. `--compatibility-weapons` with `--live-fire`
selects the prior weapon adapter independently of the flight model. The
`seeker-mode`, `target-heat:0..4`, `target-radar`, `target-distance:FEET` and
`empty-range` combat commands support deterministic fixture capture and replay.
Heat codes are unknown, off, idle, dry and afterburner respectively. Distance is
bounded to 1..1,000,000 feet. Version-4 tapes record these commands, full world
velocity and bay permission. Use fresh tape paths because recording never
overwrites an existing file. [Missile acceptance](baselines/missiles.md).

For a rendered formation check, use `--dummy-aircraft f18,5 --dummy-aircraft rafale,5`
with `--capture-flight .local/formation.ppm`. This repeatable diagnostic uses the
same dummy creation/rendering as the creator, one mile ahead. It accepts a count
without the creator's five-per-wing selector limit. Range, research and replay
modes have their own fixtures and cannot be combined with this diagnostic.
`--validate-creator` now also checks normal stores and 29-dummy reset/model geometry
for each supported identity. Custom mission recording remains unavailable.

Damage/smoke captures use `--damage-preview 0.6 --flight-view 2 --capture-flight
.local/damaged.ppm` on one command line. This explicit diagnostic sets player and
fixture health and advances two seconds before capture (override with
`--damage-preview-ticks 1..7200`); it cannot record/replay
or use native research flight. For motor smoke use `--live-fire --weapon-slot 2
--combat-command seeker-mode --combat-probe-ticks 100 --flight-view 2
--capture-flight .local/motor-smoke.ppm`. Runtime smoke comes from `SMOKE.PIC`;
`SMOKE.SH` remains unexecuted. [Fitted rules](spec/damage-smoke.md).

## AI regression probes

`cargo run --locked -p tore-app -- --ai-roster-probe-ticks 3600 --no-audio`
runs imported aircraft at all four experience levels, using all twelve exact
identities. It checks finite motion and exact replay of aircraft inputs, measures achieved
heading/bank rates, and
reports sensor fit, stores, launches and dropped launches. This is separate
from synthetic tests and is not a retail comparison or visual acceptance.
The shorter `--ai-probe-ticks N` retains the Quick Mission bridge probe.
For ground operations, combine it with `--ground-start AIRPORT`,
`--probe-wing-size 1..5` and `--maneuver takeoff`. Add `--probe-wing-only`
to omit the other wings. Those two wing options also apply to
`--launch-quick-mission` for matching creator captures. Schedule an order with
`--probe-wing-order TICK:land-selected` or `TICK:bug-out`; `--probe-trace SECONDS`
prints each wingman's airfield phase and position. `--probe-player-home FROM:UNTIL`
flies the scripted leader gear down toward the field during that tick range.
`--separation 200` or `300` also exercises the expanded enemy-distance choices.
The scripted leader is only a test harness and can hit terrain on a long cruise.
[Reproduction and limits](baselines/ground-start.md#whole-wing-ground-start-2026-09-23).


## Formation flight traces

Normal flight has no formation diagnostics on screen. To record a live rejoin,
set `TORE_FORMATION_TRACE` to a local CSV path before starting the application:

```sh
TORE_FORMATION_TRACE=.local/formation-flight.csv cargo run --locked -p tore-app
```

The parent directory must exist. The bridge appends a header at each mission
start and samples every 12 simulation ticks (10 Hz), flushing every second.
Rows contain the decision phase, its duration, slot error, closure, altitude error, predicted
minimum separation, yielding actor, steering target, achieved position/speed/
bank/G, and commanded axes plus actual throttle/afterburner. Decision quantities
precede that tick's physics; achieved quantities follow it. A write failure
reports to stderr and disables logging without stopping flight. With the
variable unset, no file is opened. The simulation inspection hook is
`Controller::formation_trace`; it never changes aircraft movement.

For airport visibility and distance precision, run the production-pipeline
checks on a GPU-capable host:

```sh
cargo test --locked -p tore-app gpu_airport_ -- --ignored --nocapture --test-threads=1
```

These check aircraft above pavement and moving distant views with 1x/4x samples.
[Measured failures and results](baselines/airfield-radio.md#ground-visibility).

`cargo test --locked -p tore-sim formation_diving_reversal -- --nocapture`
runs the synthetic four-wingman diving-reversal regression. This is a safety
and physical-input replay check, not a target rejoin-time requirement.

## Surface lighting and shadow validation

The [surface lighting spec](spec/surface-lighting.md) describes the smooth renderer.
`TORE_WEATHER_SMOOTH=0` retains stepped palette lighting without geometric shadows.
On a GPU-capable host, run the synthetic cross-surface shadow and warmth test:

```sh
cargo test --locked -p tore-app gpu_geometry_shadows -- --ignored --nocapture
cargo test --locked -p tore-app gpu_smooth_glare -- --ignored --nocapture
```

The test renders terrain/object receivers and occluders through the production
pipelines, reads pixels back, checks both sun directions and stepped mode,
and checks a warm surface's continuous response, partial-disc shadows and
view-dependent panel highlights, blocked water glints and sharper near-contact
shadows versus distant casters, and a five-second low-sun stability sequence
with small camera movements and sunglare disabled. The glare test checks
every channel of a synthetic gradient under two flare circles against continuous
optical composition. Both tests use no retail media.

## Systems instrument fixtures

`--panel-snapshot PATH --instrument-page 7` renders one 162×160 window, framed and coloured with the aircraft's stored daytime cockpit palette, from the aircraft's actual default
loadout, including external tanks. `--systems-preview 12,13,14` applies explicit
panel-only oil pump, oil leak and hydraulic leak faults. Advance with
`--flight-probe-ticks N` (default 1,200 in this fixture), and use
`--flight-throttle 0..1` for thermal comparisons. The preview requires a panel
snapshot and cannot inject damage into an interactive sortie. Indices 1..35
are listed in the [damage event map](formats/systems-damage.md). Keep captures ignored.

## AI mission and objective checks

`--ai-mission free|cap|intercept|escort|self-defense|hold` sets the inherited
Quick Mission policy. Normal default remains free engagement. Group stamps in
the creator override the preset and remain on mission restart. For example:

```sh
cargo run --locked -p tore-app -- --ai-probe-ticks 1200 --ai-mission escort --no-audio
cargo run --locked -p tore-app -- --launch-quick-mission --ai-mission intercept
cargo run --locked -p tore-app -- --quick-mission --snapshot-state objective-1 --snapshot .local/objective-popup.ppm
```

Objective popup snapshots accept `objective-1` through `objective-6`, friendly
then enemy groups. `--snapshot-state objectives` shows a primary-group/free-fire
example with a required-survival friendly group. These diagnostics do not replace campaign mission loading.

## Ejection inspection

After reimporting into an isolated profile, use `--ejection-preview seat`,
`--ejection-preview freefall` or `--ejection-preview chute` with
`--capture-flight .local/ejection/chute.ppm --no-audio`. These pause a presentation
fixture with the pilot separated from the aircraft so its original art is visible.
Pilot tapes accept `eject` as a one-shot command. Two entries within the documented
confirmation interval exercise the complete headless escape path; replay prints
phase, pilot survival and position. See [validation](baselines/ejection.md).

## Flight view inspection

`--flight-view 0..11` preserves 0 front, 1 external, 2 oblique, 3 back and 4 up.
The additions are 5 tracking, 6 inbound threat, 7 wingman, 8 player-to-target,
9 target-to-player, 10 fly-by and 11 missile-to-target. These are capture indices,
not the function-key numbers. `--flight-reference player|target|missile` selects
the reference for capture. Missing subjects return to Forward with feedback.
Use an isolated profile with `--capture-flight .local/retail-views/view.ppm`.
The [view validation](baselines/flight-views.md) records fixtures and limitations.

## Retail map detail validation

The location picker includes the sixteen base maps and 59 source variants.
`--theater '~UKR1'` selects an exact variant for the viewer, free flight,
Quick Mission or headless flight. Shell quotes preserve the tilde. Existing
flight adapters remain independent of map selection.

```sh
TORE_DATA_DIR=.local/dev-profile cargo run --locked -p tore-app -- --validate-maps --no-audio
TORE_DATA_DIR=.local/dev-profile cargo run --locked -p tore-app -- --theater KURILE --viewer --smoke-test
TORE_DATA_DIR=.local/dev-profile cargo run --locked -p tore-app -- --theater '~UKR1' --viewer --capture-terrain .local/ukr1.ppm
```

`--validate-maps` needs imported media but no display. It constructs every
imported layout, reports source identity, placement/body counts, geometry and
indexed artwork size, and exits with an error on construction failure. The
other two commands need a display. Older caches require re-import for the
`TORE_TERRAIN_V2` dependency set. Use the existing isolated import workflow
above; never write extracted retail resources into tracked assets.

Variants are static source scenery lists, with the
[documented fitted grid/composition rule](spec/terrain-detail.md#host-presentation-where-the-source-rule-is-incomplete).
They do not start a campaign or add autonomous ground behavior.
