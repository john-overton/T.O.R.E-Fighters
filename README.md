<p align="center">
  <img src="docs/images/tore-fighters-logo.png" alt="T.O.R.E Fighters squadron patch" width="320">
</p>

<h1 align="center">T.O.R.E-Fighters</h1>

<p align="center"><em>Tasteful Opinionated Reverse Engineered &mdash; a native Rust rebuild of Fighters Anthology</em></p>

<p align="center">
  <a href="https://github.com/john-overton/T.O.R.E-Fighters/actions/workflows/ci.yml"><img alt="Rust baseline build" src="https://github.com/john-overton/T.O.R.E-Fighters/actions/workflows/ci.yml/badge.svg"></a>
  <a href="rust-toolchain.toml"><img alt="Rust 1.91.1 pinned" src="https://img.shields.io/badge/rust-1.91.1-b7410e?logo=rust&logoColor=white"></a>
  <a href="docs/DEVELOPMENT.md"><img alt="Linux, Windows and macOS" src="https://img.shields.io/badge/platforms-Linux%20%7C%20Windows%20%7C%20macOS-2f6f9f"></a>
  <a href="LICENSE"><img alt="GNU General Public License v3.0" src="https://img.shields.io/badge/license-GPL--3.0-1f6feb"></a>
  <a href="docs/ROADMAP.md"><img alt="Milestone M1 in progress" src="https://img.shields.io/badge/milestone-M1%20in%20progress-orange"></a>
</p>

<p align="center">
  <a href="docs/formats/theater.md"><img alt="16 theaters" src="https://img.shields.io/badge/theaters-16-3c7a57"></a>
  <a href="docs/FLIGHT-MODEL.md"><img alt="F/A-18D and Rafale C" src="https://img.shields.io/badge/aircraft-F%2FA--18D%20%7C%20Rafale%20C-3c7a57"></a>
  <a href="docs/baselines/weapons-systems.md"><img alt="135 weapon definitions" src="https://img.shields.io/badge/weapon%20definitions-135-3c7a57"></a>
  <a href="AGENTS.md"><img alt="No retail game data in this repository" src="https://img.shields.io/badge/retail%20game%20data-none%20shipped-6b4fbb"></a>
  <a href="MODS.md"><img alt="Mods keep their own license" src="https://img.shields.io/badge/mods-your%20own%20license-6b4fbb"></a>
</p>

T.O.R.E-Fighters follows [the roadmap](docs/ROADMAP.md). The current slices are original menus, all 16 original theaters, and F/A-18D / Rafale C free flight with raster instrument windows.

The app launches into the original **Choose Activity** menu using artwork, button pieces, proportional fonts, and sounds imported from your own Fighters Anthology files. Buttons animate; `?`, `Pref`, and `Multi` open dropdowns. **Create Quick Mission** opens the original-style briefing: click the aircraft name in Wing 1 or the theater name in “You are flying over…” to select, then **OK** to fly. F/A-18D and Rafale C are available. All briefing fields are editable; unsupported mission systems are validated before launch. Custom weapons opens the original-art Load Ordnance screen with compatible weapon and fuel edits. Set enemy Wing 1 to zero for the supported single-aircraft preview. [Testing steps and remaining parity](docs/baselines/creator-ordnance.md). The explicit `--live-fire` range supports manual weapon testing, incoming fixtures, ECM contact resolution and partial automatic subsystem damage. Controller combat bindings and bounded haptics are integrated; [native-parity limits and evidence](docs/baselines/weapons-systems.md) remain explicit. For hands-on testing, see the [keyboard controls](docs/FLIGHT-CONTROLS.md#weapons-and-systems-continuation) and [controller combat layer](docs/INPUT.md#manual-combat-layer--2026-09-14). Physical vibration acceptance remains open. No retail game data ships in this repository.

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
- [Mods](MODS.md): what the license means for mission, theater, art and sound content.
- [Third-party notices](THIRD_PARTY_NOTICES.md): upstream attributions for engine code.

`crates/tore-app/` contains the native shell, `crates/tore-formats/` the shared readers, and `crates/tore-extract/` the headless extractor. `tools/` contains portable extraction/research scripts and the asset guard. GitHub Actions is configured to build and check macOS, Linux, and Windows.

Your game files belong in ignored `gameassets/fighters-anthology/`. The ignored `USNF-ATF/` checkout supplies reference specifications; it is not needed by the Rust importer or runtime. Music uses original recorded menu/briefing playlists and the NORMAL free-flight score. Optional `FA_4B.LIB` and `FA_4D.LIB` supply the recordings; MIDI and synthesis are not required. See [audio recovery](docs/formats/music.md). Run with `--no-audio` for a silent session.

F/A-18D and Rafale C free flight are available through **Create Quick Mission → OK**, or `cargo run --locked -p tore-app -- --free-flight --aircraft rafale` (use `--aircraft f18` for the Hornet). Select either aircraft and any imported theater through the highlighted briefing text; set additional wings to zero, choose clear conditions and no ground target/defenses. Select a custom weapons load to edit ordnance before Fly; standard currently uses PT defaults. The cockpit adapts to the window aspect, with a compact original-font HUD and instrument windows anchored to the screen edges. Arrows fly; PageUp/PageDown adjusts throttle; Shift-B enables afterburner. F1/F2/F3 looks forward/back/up, F10 selects external view, Backspace toggles cockpit art, and Shift-0..9 toggles instruments. **Escape → Pref → Large windows?** switches between four inset corner windows and six smaller bottom windows (three per side). Escape opens the paused flight menu, Ctrl-P pauses/resumes, and F11 opens keyboard help. See the [complete current control reference](docs/FLIGHT-CONTROLS.md). See [controls/setup](docs/DEVELOPMENT.md#hornet-free-flight) and [aircraft/weapon extraction](docs/EXTRACTION.md#fa-18d-and-weapons). This is a playable development adapter; full native flight and instrument parity remain [tracked work](docs/formats/aircraft.md#next-parity-gates).

Flight performance: ordinary debug runs now optimize the app's rendering loops and use display-paced presentation with interpolated 120 Hz simulation. Use **F10** for the exterior aircraft view; **F2/F3** look back/up from the aircraft. See [performance diagnostics and measurements](docs/baselines/flight-performance.md) for repeatable frame-time checks.

Look around with **Shift + arrows** (Ctrl + arrows also works). Cockpit Down stops at the forward eye line; F10 exterior view orbits freely around the aircraft. **Shift + /** recenters the current view; **F1** returns to the cockpit. [Control details](docs/FLIGHT-CONTROLS.md#look-around-and-exterior-orbit).

Weather now uses the original day/night palettes, horizon and sky/ocean planes,
sun/moon/stars, cloud sheets, aircraft light/fog maps and cockpit/HUD palette.
The moon's world orientation stays independent of aircraft bank. Retail visual
acceptance and the remaining wind/turbulence/vapor work are tracked in the
[weather plan](docs/weather-plan.md); Linux captures alone do not establish 1:1 parity.

Flight now carries momentum independently of nose direction and can rotate through vertical for loops. The cockpit and HUD stay anchored together to the aircraft’s forward position during look-around, and the sky projection no longer pinches at straight up. Directional viewing projects the original flat cockpit artwork; full rear/overhead interior geometry remains unavailable. See [directional cockpit validation](docs/baselines/directional-cockpit.md). [Flight-response and sky validation](docs/baselines/flight-response-sky.md).

F/A-18 exterior devices now animate continuously: **G/F/B/H** for gear/flaps/airbrake/hook, arrows and **Z/X** for fitted control surfaces. Press **0 then Shift+B** for afterburner; use **F10** to inspect. [Animation coverage and inspection commands](docs/baselines/f18-animations.md).

Native flight reverse-engineering now has a [repeatable static extraction pass and
coverage notes](docs/formats/native-flight.md). Use `cargo run --locked -p tore-app -- --native-flight-report`
for imported-Hornet helper probes. Full native dynamics remain in progress; normal
free flight still uses the authored adapter.

Native flight research now includes reusable PT profiles, stall/spin components, force/loading calculations, landing checks, extracted trigonometry, and velocity/angle/wind stages. These components also power an explicit airborne native research option; legacy remains the default. See [decode coverage and component boundaries](docs/formats/native-flight.md#second-pass-departure-ground-and-integration-components).

Native flight research includes an imported-table world/cockpit composition probe
and isolated contact, equipment/control and clock/RNG components. See
[development commands](docs/DEVELOPMENT.md) and
[extraction status and limits](docs/formats/native-flight.md). Their unrestricted runtime/lifecycle acceptance remains open; see the airborne
research option below.

Rafale C uses its own retail PT, cockpit, exterior and equipment/audio dependencies. Its original canards, elevons, gear, airbrakes, rudder and exhaust now animate through a fitted presentation rig. The imported model has no hook control. Exact native animation and flight-model parity remain open. See [Rafale and creator evidence](docs/baselines/rafale-quick-mission.md).

The shared `tore-sim` kernel now supports tested hybrid flight for the F/A-18D
and extracted Rafale C data. Run the Hornet with `cargo run --locked -p tore-app --
--free-flight --researched-flight`; add `--aircraft rafale` for the Rafale C
with its own cockpit and animation rig. Use `tools/extract_assets.py
--aircraft rafale --validate-flight` with your media options to reproduce the
second-aircraft workflow. Supported G/rate telemetry, rudder response and hybrid
stall/spin recovery now have [both-adapter evidence](docs/baselines/flight-response.md);
maneuver audio/rumble and full native parity remain open. See
[flight-model commands and scope](docs/FLIGHT-MODEL.md).

F/A-18D and Rafale C now own separate flight-law modules and independently editable
typed configurations for mass, thrust/fuel, aerodynamics, departure/contact, equipment
and tuning. The simulator reads these directly. A typed air-data interface supports future analog instruments; see the
[model and instrument extension guide](docs/FLIGHT-MODEL.md).

Cockpit mirrors now render live rear views every visible frame for both aircraft,
including the airframe. Rendering requests uncapped presentation; simulation
remains fixed at 120 Hz. Mirror optics are fitted to the original artwork.
See [measurements and limitations](docs/baselines/mirrors.md).

Controller input is available through a hand-rolled shared binding layer.
Standard Linux gamepads have default flight/menu bindings; sticks, throttles,
pedals and button boxes can use explicit profiles. Run
`cargo run --locked -p tore-app -- --list-inputs` to inspect hardware without
loading retail media. See [controller setup, bindings and instrument focus](docs/INPUT.md)
and [platform/hardware acceptance](docs/baselines/input.md).

Controller rumble: `cargo run --locked -p tore-app -- --test-rumble only` tests
exactly one capable connected controller. Linux Ultimate 2 pulse response is
user-confirmed; Windows and macOS 11+ haptics have cross-compile checks but still
need hardware acceptance. See [controller setup and platform limits](docs/INPUT.md).

In flight, **Escape → Control** now edits bindings and enables rumble through
**Save & apply**. Instrument layouts/pages, scope settings, cockpit/HUD/zoom and
sound preferences persist between normal sessions. Afterburner provides a quiet
continuous rumble beneath its engagement pulse while active. [Settings guide](docs/INPUT.md).

Aircraft armament import now includes all 135 FA weapon definitions, sensors,
ECM, tanks and reviewed shared effects. Export both supported aircraft with
`--aircraft f18 --aircraft rafale --weapons`. A connected development range is now available:
`cargo run --locked -p tore-app -- --live-fire --aircraft f18` (or `rafale`).
Space fires, semicolon selects a weapon, backslash resets range, and T designates.
The range supports both source guns and PT-default missiles, with documented
guidance/contact/damage approximations. Ordinary free flight remains externally
clean. [Exact capabilities, screenshots and validation](docs/baselines/live-fire.md).

The two-aircraft development range now supports manual arm/safe, sensor/range
inhibits, five damage-class fixtures, station failure, external-group jettison,
carried weapon bodies and optional combat-service recording/replay. See
[manual weapon testing](docs/baselines/manual-weapons.md) for controls, all-slot
checks and remaining native-parity gaps. Combat AI is deferred.

## License

The engine, tools and documentation are licensed under the
[GNU General Public License v3.0](LICENSE). Mods are data the engine loads, not
derivative works of it, so your content stays under whatever license you choose;
[MODS.md](MODS.md) draws that line and explains the retail-asset rule. Playing
requires a legally owned copy of Jane's Fighters Anthology; no retail game data
ships in this repository.


For the restricted native flight connection, use `--free-flight --aircraft f18
--native-flight-tables DIR` (or `--aircraft rafale`). DIR contains statically
extracted sine/atan tables. This mode runs native control/departure/force/movement
translations, retains explicit host clock/device/fuel inputs, disables environmental
turbulence and stops at unsupported terrain contact.
[Setup, validation and remaining limits](docs/baselines/native-live-flight.md).
