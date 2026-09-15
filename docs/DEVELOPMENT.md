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

First launch automatically imports `gameassets/fighters-anthology/` if no valid cache exists. Use `--import <directory>` to refresh or choose other media. `--import-only` imports and exits without opening a window/audio device. Required media: reviewed `FA.EXE` alongside `FA_1.LIB` and `FA_2.LIB`; optional `FA_4B.LIB` and `FA_4D.LIB` supply flight and shell music. Missing required media produces an actionable terminal error; there is no file-picker UI yet.

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

An Omarchy x86_64 host has also passed local build, import and Wayland/Vulkan
startup checks with Rust 1.91.1; see the [Linux setup baseline](baselines/linux-setup.md).
The app requests a high-performance compatible GPU. On the tested AMD/NVIDIA
desktop this selects the RTX 4070; requesting the integrated AMD adapter produced
a blank visible window despite successful frame submission.
The renderer must be released in the event loop's exit callback, before its
display connection closes, and keep its window alive through GPU cleanup.

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

`--no-audio` silences a session. Normal playback uses the system's default output device, original PCM effects, recorded main/briefing playlists and the NORMAL score during free flight when available. Device initialization failure is reported and the menu continues silently. Main-menu M toggles music; `Pref` exposes music/effect toggles. Music follows the saved preference into flight and freezes on flight pause. In-flight Sound still toggles effects only. No MIDI or synth is used. See [audio behavior and limits](formats/music.md). Music/effects and flight display preferences are restored from `preferences-v1.conf` in the application data directory.

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

The main-menu Create Quick Mission action opens the original `QUIKMIS3.PIC` artwork with editable briefing fields and standard/custom weapons selection. The terrain inspection camera is now a CLI diagnostic. This supports an airborne patrol preview; AI and mission objectives remain open. Launch it with `--quick-mission`, or skip to the world with `--viewer`.

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

The creator and placeholder notices now use original `ARMFONT.PIC` sans-serif glyphs; the compact viewer HUD uses `SMLFONT.PIC`. Tinted glyphs preserve source shading instead of flattening every visible pixel to white. Button labels retain original FONTACT artwork. These remain legacy raster fonts scaled with the menu; they are not resolution-independent vector text. No system font or new dependency is required. Use `--snapshot-state notice --snapshot .local/notice.ppm` to inspect the placeholder message.

## Hornet free flight

Create Quick Mission supports F/A-18D and Rafale C airborne previews with accepted
weapons, ammunition and fuel. Set all additional wings to zero, clear conditions
and no ground targets/defenses. Select a custom load to open Load Ordnance; standard
uses reviewed PT defaults. Direct `--free-flight` remains clean with full fuel.
Selected creator altitude is retained exactly or rejected for terrain clearance.

```sh
cargo run --locked -p tore-app -- --free-flight --theater UKR
cargo run --locked -p tore-app -- --free-flight --theater TVIET --flight-view 1
cargo run --locked -p tore-app -- --headless-flight 1200 --maneuver pull
```

Arrows: pitch/bank (Down pulls up). Z/X: rudder. PageUp/PageDown: throttle; 1..9/0 selects 10..90%/full. Shift-B: afterburner (requires >95% throttle). E: engine. G/F/B/H: gear/flaps/airbrake/hook. R/J: radar/jammer. F1/F2/F3: front/back/up; F10: external. Shift-0..9: instrument windows (four large or six small). Backspace toggles cockpit art. Escape opens the paused in-flight menu; Ctrl-P pauses/resumes; Ctrl-Q ends flight. Focus loss pauses automatically. On Macs use Fn/Globe for function keys and Fn-Up/Down for PageUp/PageDown. See [all current controls and source status](FLIGHT-CONTROLS.md).

Instrument window numbers: 1 envelope, 2 front view, 3 other view, 4 target, 5 RWR, 6 navigation, 7 systems, 8 weapons, 9 radar. Range/mode buttons work on the scopes; unsupported readings and no-target states remain explicit. Older Hornet-less caches refresh from local media. `--viewer` remains a developer terrain diagnostic but has no creator button. Source data, authored integration and remaining parity work are distinguished in [aircraft notes](formats/aircraft.md); [baseline commands](baselines/f18-free-flight.md) cover screenshots and tests.


The full-height cockpit and live HUD can be captured with `--capture-flight .local/cockpit.ppm`. Add `--flight-menu` to capture the paused Escape menu. `--flight-view 0|1|2|3|4` selects front/chase/oblique/back/up for inspection. These flags require a display for GPU capture. See [cockpit/control validation](baselines/cockpit-controls.md).


Flight UI now adapts to drawable aspect ratio independently of menu letterboxing. Use `--window-size 1280x720` (or resize normally) to inspect widescreen behavior. `--capture-flight` preserves the current aspect and writes at the flight overlay resolution, capped proportionally at 1920×1080. The original `--capture-terrain` remains 960×720. Small instruments resample directly from their native rasters; HUD readouts have transparent backgrounds. See [responsive-flight checks](baselines/responsive-flight-ui.md).

## Flight performance

Normal `cargo run --locked -p tore-app -- --free-flight` now optimizes the app crate at level 2, retaining debug symbols/assertions. Dependencies keep their existing debug settings; Cargo may still label the overall dev profile “unoptimized.” No release build is required to benefit. Simulation remains fixed at 120 Hz; presentation interpolates its last two poses and requests uncapped Immediate presentation, then Mailbox, with FIFO only as a supported-mode fallback and one requested queued frame. There is no additional 16 ms sleep in flight/viewer mode. The display/compositor can still limit presentation frequency.

Run a bounded desktop measurement (macOS/Linux shell):

```sh
TORE_PERF_FRAMES=330 TORE_PERF_ACTIVE=1 TORE_PERF_VIEWS=1 cargo run --locked -p tore-app -- --free-flight --no-audio --window-size 1280x720
TORE_PERF_FRAMES=180 TORE_PERF_ACTIVE=1 cargo run --locked -p tore-app -- --free-flight --no-audio --instrument-page 3
```

On PowerShell, set `$env:TORE_PERF_FRAMES="330"`, `$env:TORE_PERF_ACTIVE="1"`, and `$env:TORE_PERF_VIEWS="1"`, run the same Cargo command, then remove those environment variables. `TORE_PERF_FRAMES` accepts 60–100000 frames (0/off by default), prints mean/p50/p95/max milliseconds and exits after that many flight frames. The first 30 are excluded. Presence of `TORE_PERF_VIEWS` cycles front/back/up/chase/oblique every 30 frames; omit it to measure the selected view or switch views manually. These diagnostics only count flight frames; start with `--free-flight`. Avoid concurrent GPU workloads when comparing runs. Presence of `TORE_PERF_ACTIVE` explicitly keeps the bounded flight benchmark unpaused even if automation steals focus (it does not dismiss menus). Omit it for normal pause behavior and interactive pause measurements. The report includes paused frames and completed live camera readbacks; check these before interpreting a run as active flight.

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
Click scalar text to cycle; Shift-click opens a scrollable list. Aircraft names
open the list directly. Only F/A-18D and Rafale C can fly. Unsupported mission
systems remain editable setup placeholders with launch validation.

```sh
cargo run --locked -p tore-app -- --quick-mission --aircraft rafale
cargo run --locked -p tore-app -- --free-flight --aircraft rafale --theater FRA
cargo run --locked -p tore-app -- --aircraft rafale --headless-flight 10800 --maneuver loop
cargo run --locked -p tore-app -- --quick-mission --snapshot-state aircraft --snapshot .local/aircraft-selector.ppm
cargo run --locked -p tore-app -- --quick-mission --snapshot-state theaters --snapshot .local/theater-selector.ppm
```

`--aircraft f18` remains the default. Quick-mission snapshot states are `normal`,
`aircraft`, `theaters`, `ordnance`, and `help`; they use the original 640×480 menu canvas.
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
select the hybrid Hornet model; the default remains the legacy adapter. Extract
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

Both `--combat-smoke` identities now exercise each of five default JT slots
against all five source damage entries (50 cases total), with negative launch
checks and deterministic live-state comparison. Set `TORE_COMBAT_EVIDENCE` to a
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
and ECM checks to both combat smokes, version-2 combat tapes, `--jammer-on`,
and `--combat-command damage|incoming|target-jammer`. Probe logs include haptic
event/mixer counts without playing historical pulses on hardware. Use a new
`TORE_COMBAT_EVIDENCE` directory for ten serialized slot tapes. Current native
research has 28 reviewed regions; full subsystem/ECM parity remains open.

## Creator / ordnance acceptance

```sh
cargo run --locked -p tore-app -- --validate-creator
cargo run --locked -p tore-app -- --validate-weather
TORE_WEATHER_TIME=19:06 cargo run --locked -p tore-app -- --capture-terrain .local/weather/dusk.ppm
TORE_VAPOR_PROBE=1 cargo run --locked -p tore-app -- --free-flight --maneuver pull --flight-probe-ticks 400 --smoke-test
cargo run --locked -p tore-app -- --weather-condition 1 --capture-terrain .local/weather/cloudy.ppm
cargo run --locked -p tore-app -- --quick-mission --snapshot-state ordnance --snapshot .local/ordnance.ppm
cargo run --locked -p tore-app -- --quick-mission --snapshot-state ordnance --smoke-test
```

`--validate-creator` needs imported media but no display/audio, and checks both
aircraft's supported placements, fuel, empty stations and accepted-ammo restart.
In Load Ordnance click a catalog weapon then a compatible station, or drag between
them. Tab changes selected station; +/- changes ammunition; right-click decrements.
Fuel rocker edits 500 lb at a time. Select Plane preserves the custom draft.
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
