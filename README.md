# T.O.R.E-Fighters

Tasteful Opinionated Reverse Engineered: a native Rust rebuild of Fighters Anthology, following [the roadmap](docs/ROADMAP.md). The current slices are original menus, all 16 original theaters, and F/A-18D / Rafale C free flight with raster instrument windows.

The app launches into the original **Choose Activity** menu using artwork, button pieces, proportional fonts, and sounds imported from your own Fighters Anthology files. Buttons animate; `?`, `Pref`, and `Multi` open dropdowns. **Create Quick Mission** opens the original-style briefing: click the aircraft name in Wing 1 or the theater name in “You are flying over…” to select, then **OK** to fly. F/A-18D and Rafale C are available. Enemy fields are ghosted and inert; loadout and combat are deferred. No retail game data ships in this repository.

Each launch randomly selects one of the five original menu backgrounds. Hovering is silent; sounds play on clicks/toggles.

## Run locally

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

F/A-18D and Rafale C free flight are available through **Create Quick Mission → OK**, or `cargo run --locked -p tore-app -- --free-flight --aircraft rafale` (use `--aircraft f18` for the Hornet). Select either aircraft and any imported theater through the highlighted briefing text; other mission fields are ghosted and loadout is skipped. The cockpit adapts to the window aspect, with a compact original-font HUD and instrument windows anchored to the screen edges. Arrows fly; PageUp/PageDown adjusts throttle; Shift-B enables afterburner. F1/F2/F3 looks forward/back/up, F10 selects external view, Backspace toggles cockpit art, and Shift-0..9 toggles instruments. **Escape → Pref → Large windows?** switches between four inset corner windows and six smaller bottom windows (three per side). Escape opens the paused flight menu, Ctrl-P pauses/resumes, and F11 opens keyboard help. See the [complete current control reference](docs/FLIGHT-CONTROLS.md). See [controls/setup](docs/DEVELOPMENT.md#hornet-free-flight) and [aircraft/weapon extraction](docs/EXTRACTION.md#fa-18d-and-weapons). This is a playable development adapter; full native flight and instrument parity remain [tracked work](docs/formats/aircraft.md#next-parity-gates).

Flight performance: ordinary debug runs now optimize the app's rendering loops and use display-paced presentation with interpolated 120 Hz simulation. Use **F10** for the exterior aircraft view; **F2/F3** look back/up from the aircraft. See [performance diagnostics and measurements](docs/baselines/flight-performance.md) for repeatable frame-time checks.

Look around with **Shift + arrows** (Ctrl + arrows also works). Cockpit Down stops at the forward eye line; F10 exterior view orbits freely around the aircraft. **Shift + /** recenters the current view; **F1** returns to the cockpit. [Control details](docs/FLIGHT-CONTROLS.md#look-around-and-exterior-orbit).

Flight now carries momentum independently of nose direction and can rotate through vertical for loops. The cockpit and HUD stay anchored together to the aircraft’s forward position during look-around, and the sky projection no longer pinches at straight up. Directional viewing projects the original flat cockpit artwork; full rear/overhead interior geometry remains unavailable. See [directional cockpit validation](docs/baselines/directional-cockpit.md). [Flight-response and sky validation](docs/baselines/flight-response-sky.md).

F/A-18 exterior devices now animate continuously: **G/F/B/H** for gear/flaps/airbrake/hook, arrows and **Z/X** for fitted control surfaces. Press **0 then Shift+B** for afterburner; use **F10** to inspect. [Animation coverage and inspection commands](docs/baselines/f18-animations.md).

Native flight reverse-engineering now has a [repeatable static extraction pass and
coverage notes](docs/formats/native-flight.md). Use `cargo run --locked -p tore-app -- --native-flight-report`
for imported-Hornet helper probes. Full native dynamics remain in progress; normal
free flight still uses the authored adapter.

Native flight research now includes reusable PT profiles, stall/spin components, force/loading calculations, landing checks, extracted trigonometry, and velocity/angle/wind stages. These are diagnostic components; free flight still uses the authored adapter. See [decode coverage and component boundaries](docs/formats/native-flight.md#second-pass-departure-ground-and-integration-components).

Native flight research includes an imported-table world/cockpit composition probe
and isolated contact, equipment/control and clock/RNG components. See
[development commands](docs/DEVELOPMENT.md) and
[extraction status and limits](docs/formats/native-flight.md). These diagnostic
translations are not yet the playable flight adapter.

Rafale C uses its own retail PT, cockpit, exterior and equipment/audio dependencies. Its original canards, elevons, gear, airbrakes, rudder and exhaust now animate through a fitted presentation rig. The imported model has no hook control. Exact native animation and flight-model parity remain open. See [Rafale and creator evidence](docs/baselines/rafale-quick-mission.md).

The shared `tore-sim` kernel now supports tested hybrid flight for the F/A-18D
and extracted Rafale C data. Run the Hornet with `cargo run --locked -p tore-app --
--free-flight --researched-flight`; add `--aircraft rafale` for the Rafale C
with its own cockpit and animation rig. Use `tools/extract_assets.py
--aircraft rafale --validate-flight` with your media options to reproduce the
second-aircraft workflow. See [flight-model commands and scope](docs/FLIGHT-MODEL.md).

F/A-18D and Rafale C now own separate flight-law modules and independently editable
typed configurations for mass, thrust/fuel, aerodynamics, departure/contact, equipment
and tuning. The simulator reads these directly. A typed air-data interface supports future analog instruments; see the
[model and instrument extension guide](docs/FLIGHT-MODEL.md).
