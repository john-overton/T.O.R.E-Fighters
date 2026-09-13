# T.O.R.E-Fighters

Tasteful Opinionated Reverse Engineered: a native Rust rebuild of Fighters Anthology, following [the roadmap](docs/ROADMAP.md). The current slices are original menus, all 16 original theaters, and F/A-18D free flight with raster instrument windows.

The app launches into the original **Choose Activity** menu using artwork, button pieces, proportional fonts, and sounds imported from your own Fighters Anthology files. Buttons animate; `?`, `Pref`, and `Multi` open dropdowns. **Create Quick Mission → Free Flight** launches the imported F/A-18D over the selected theater. Other mission selectors are placeholders; loadout and combat are deferred. No retail game data ships in this repository.

Each launch randomly selects one of the five original menu backgrounds. Hovering is silent; sounds play on clicks/toggles.

## Run on this Mac

Rust 1.91.1 is pinned through rustup. From the repository root:

```sh
source "$HOME/.cargo/env"
cargo run --locked -p tore-app
```

On first run, local `gameassets/fighters-anthology/` media is imported into platform application data. Later launches use that cache. To import another location or refresh the menu, theater and aircraft assets:

```sh
cargo run --locked -p tore-app -- --import /path/to/fighters-anthology
```

Use **? → Exit to Desktop** or close the window to quit. Escape dismisses a dropdown, Tab/arrows and Enter navigate, and M toggles music. `Pref` also toggles music and effects. Replay/continue are disabled until those systems exist.

To check startup, render one frame, and exit without audio:

```sh
cargo run --locked -p tore-app -- --smoke-test
```

See [development setup](docs/DEVELOPMENT.md) for fresh-machine setup, Linux/Windows prerequisites, checks, and troubleshooting.

## Explore the theaters

The free-camera terrain diagnostic remains available from the command line:

```sh
cargo run --locked -p tore-app -- --viewer
```

Arrow keys move, **Shift** moves 8× faster, **Q/E** or **PageDown/PageUp** lower/raise altitude, **A/D** turn and **W/S** look up/down. **Escape** returns to the creator, then the main menu. All 16 theaters are selectable. Launch a particular theater directly with `--viewer --theater TVIET` (Vietnam), for example. Weather uses a fixed midday preview; sun, moon, stars and cloud shapes are extracted for further recovery but are not yet rendered. See [recovery findings](docs/formats/theater.md) and [viewer validation](docs/baselines/ukraine-viewer.md).

## Extract assets for research

```sh
python3 tools/extract_assets.py --dry-run
python3 tools/extract_assets.py
# All 16 defined theaters and shared environment resources:
python3 tools/extract_assets.py --theater all --exclude-archive 'disc1/LHX/*' --out .local/all-theaters
```

This discovers and unpacks all supported archives into ignored `.local/extracted/`, preserving archive boundaries and writing a report with hashes. Use `--source` for other media and `--include "*.PIC"` for filtering. The app does not require this full extraction. See [the extraction guide](docs/EXTRACTION.md) for Windows commands, limits, and repeat-run behavior.

## Project guide

- [Roadmap](docs/ROADMAP.md): milestones and parity goals.
- [Parity progress](docs/progress.md): completed work and remaining menu, original-terrain, simulation and aircraft steps.
- [Development](docs/DEVELOPMENT.md): environment and everyday commands.
- [Extraction](docs/EXTRACTION.md): shared script, filtering, output layout, and supported containers.
- [Architecture](docs/ARCHITECTURE.md): baseline choices and boundaries.
- [Local references](docs/REFERENCES.md): media and TypeScript reference locations, menu starting points.
- [Menu extraction](docs/formats/menu.md): recovered assets, geometry, fonts, and fidelity boundaries.
- [Menu baseline](docs/baselines/main-menu.md): validation, screenshots, archive census, and remaining work.
- [Baseline evidence](docs/baselines/environment.md): what has actually been verified.
- [Agent instructions](AGENTS.md): automated contributor conventions.

`crates/tore-app/` contains the native shell, `crates/tore-formats/` the shared readers, and `crates/tore-extract/` the headless extractor. `tools/` contains portable extraction/research scripts and the asset guard. GitHub Actions is configured to build and check macOS, Linux, and Windows.

Your game files belong in ignored `gameassets/fighters-anthology/`. The ignored `USNF-ATF/` checkout supplies reference specifications; it is not needed by the Rust importer or runtime. Music currently previews recovered `AIR003.11K`; its original activity-menu mapping is not confirmed. Run with `--no-audio` for a silent session.

F/A-18D free flight is available through **Create Quick Mission → Free Flight**, or `cargo run --locked -p tore-app -- --free-flight`. Choose any imported theater; other mission selectors remain dotted placeholders and the loadout page is skipped. The cockpit adapts to the window aspect, with a compact original-font HUD and instrument windows anchored to the screen edges. Arrows fly; PageUp/PageDown adjusts throttle; Shift-B enables afterburner. F1/F2/F3 looks forward/back/up, F10 selects external view, Backspace toggles cockpit art, and Shift-0..9 toggles instruments. **Escape → Pref → Large windows?** switches between four inset corner windows and six smaller bottom windows (three per side). Escape opens the paused flight menu, Ctrl-P pauses/resumes, and F11 opens keyboard help. See the [complete current control reference](docs/FLIGHT-CONTROLS.md). See [controls/setup](docs/DEVELOPMENT.md#hornet-free-flight) and [aircraft/weapon extraction](docs/EXTRACTION.md#fa-18d-and-weapons). This is a playable development adapter; full native flight and instrument parity remain [tracked work](docs/formats/aircraft.md#next-parity-gates).
