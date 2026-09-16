# Cockpit look-around and exterior orbit

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature; see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-13, Apple M3/macOS/Metal, Rust 1.91.1. Builds on the uncommitted performance pass; it does not change the fixed flight integration or introduce GPU readback waits.

## Behavior and evidence

Shift+arrows is now an alias for Ctrl+arrows. `look.rs` classifies held keys, advances view angles by elapsed time (one radian/second), and applies camera transforms. A look arrow remains claimed until release even when Shift/Ctrl is released first; repeats cannot turn it into pitch/bank. Modifier press can also convert an already-held flight arrow to look. Existing focus/pause paths clear held controls.

Cockpit elevation ranges from the forward eye line to overhead (0..90 degrees relative to forward pitch). Down can return from an upward look but cannot go below that line. Horizontal look wraps continuously. Exterior views orbit at constant distance around the interpolated aircraft position, through full horizontal and vertical revolutions; crossing a pole can invert the camera. This replaces rotating the exterior camera away from the aircraft. Recenter uses the exact initial orbit elevation rather than the previous fitted -0.3-radian aim, keeping the aircraft centered. Shift-/ recenters look without changing selected view/zoom; F1 selects and recenters the cockpit.

The local `USNF-ATF/Docs/reference/JANES_US_NAVY_FIGHTERS_djvu.txt`, lines 2264–2275, describes Ctrl+arrows for keyboard-flight panning and Right Shift plus joystick. `USNF-ATF/Docs/phase-4-cockpit-guns.md` documents that app's Shift+arrow convention. The supplied FA readme did not establish Anthology's non-menu dispatch. Therefore Ctrl has USNF manual evidence, while Shift and the recenter shortcut are convenience mappings, not asserted native FA recovery. No reference app engine code was imported.

Forward artwork/HUD are still hidden while panned; instruments stay visible. Side/rear/up cockpit artwork, native FA pan rates/limits, 3D cockpit geometry, orbit terrain collision and joystick head-look remain unimplemented. The user's cockpit restriction is implemented directly without claiming recovered native geometry.

## Validation

63 Rust tests and five Python tests pass; formatting, Clippy with warnings denied, locked workspace build and repository/binary asset guards pass. Tests cover modifier-to-look transitions, modifier release/repeat isolation, Alt/Super isolation, cockpit floor/ceiling, elapsed-time motion at 30/60/144 Hz, below-aircraft and over-pole centered orbit, and physical slash/recenter handling. Existing pause and fixed-step tests remain green.

Six Metal checks pass: wide upward/right cockpit view; tall cockpit with a requested negative pitch clamped to forward; exterior orbit below the aircraft; exterior orbit over the pole; creator and terrain viewer. The four captures were visually inspected; exterior captures show the aircraft centered and cockpit floor capture retains forward artwork/HUD. No human handling or native-game parity acceptance is claimed.

```sh
cargo run --locked -p tore-app -- --flight-view 0 --flight-look 45,35 --window-size 1280x720 --capture-flight .local/look-up.ppm
cargo run --locked -p tore-app -- --flight-view 0 --flight-look 0,-45 --window-size 640x900 --capture-flight .local/look-floor.ppm
cargo run --locked -p tore-app -- --flight-view 1 --flight-look 120,-65 --capture-flight .local/orbit-below.ppm
cargo run --locked -p tore-app -- --flight-view 1 --flight-look 45,140 --capture-flight .local/orbit-over.ppm
```

`--flight-look` accepts finite degrees in -360..360; internal elevation clamps to 0..90. Local logs/captures are in `.local/performance/look-checks.log`, `look-gpu.log`, `look-up.*`, `look-floor.*`, `orbit-below.*` and `orbit-over.*`. Keep derivatives ignored.

Follow-up: [flight response, sky and retained cockpit](flight-response-sky.md) supersedes the earlier Euler interpolation, rigid nose/velocity coupling and hidden cockpit during head-look. Earlier measurements remain historical evidence.

The cockpit/HUD presentation described above is superseded by the [directional forward-plane pass](directional-cockpit.md); historical checks remain recorded here.
