# F/A-18D animation pass — 2026-09-13

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


## Implemented presentation

The exterior now uses continuous device fractions instead of switching between endpoint shapes at 50% travel. Source textures, UVs and fully deployed device geometry remain in use. Animation runs from fixed 120 Hz state and interpolates for presentation; it does not feed fitted visual deflections back into flight dynamics.

| Part | Source evidence | Current fitted motion |
| --- | --- | --- |
| Gear | 0x7912 adds 24 polygons, including doors and nose/main assemblies | Three-second travel; separate nose/main folding axes; doors open during first quarter of extension and close during last quarter of retraction |
| Airbrake | 0x790c adds two dorsal faces at 0x5059/0x5080 | Rotates about forward edge toward the exact source deployed pose |
| Hook | 0x791e adds two faces at 0x4a03/0x4a22 | Rotates about its upper attachment toward source deployed pose |
| Flaps | Neutral faces 0x5154/0x517b and 0x525b/0x5282 | Original upper/lower inboard panels rotate together around fitted forward edges, about 30 degrees down |
| Stabilators | Eight original aft horizontal-tail faces | Fitted pitch and differential roll deflection, approximately 17/11 degrees maximum contributions |
| Rudders | Fin pairs 0x5467/0x548e and 0x54d4/0x54fb | Split existing faces along fitted trailing-rudder hinges, interpolate UVs at split, leave forward fins fixed; about 20 degrees deflection |
| Afterburner | 0x7900 adds eight crossed flame faces | Original flame art extends/retracts from nozzle over 0.2 seconds; source endpoint retained |
| Nozzle core | Four rear atlas faces | Authored dark material when exhaust effect is fully off; original hot art otherwise |

Positions use source X/right, Y/forward, Z/up. Stored SH normals use X/right, Y/up, Z/forward; moving normals are converted to source axes, rotated with their faces and converted back before visibility tests. The rig validates expected polygon counts for each reviewed group in addition to existing module layout/state-word checks.

The remaining guards (0x7906, 0x7924, 0x792a) produced no geometry delta with the current bounded static reader. That does not prove they have no native effect: native arithmetic/self-patching angles are not executed. No new SH opcode semantics or full native animation recovery is claimed.

## Controls and inspection

- G: gear; F: flaps; B: airbrake; H: arresting hook.
- Arrow keys: pitch/roll surfaces; Z/X: rudders. Visual control deflection settles over 0.1 seconds after press/release.
- **0, then Shift+B**: full throttle and afterburner. Engine must be running, fuel available and throttle above 95%. HUD/audio use the same activation check; visual exhaust finishes its short transition after disengagement.
- F10: exterior view. Shift/Ctrl+arrows: orbit; Shift+/: recenter.

Repeatable captures (display/GPU required):

```sh
cargo run --locked -p tore-app -- --flight-view 2 --flight-look 0,-25 --flight-devices 0.5,0.5,0.5,0.5,0.5 --capture-flight .local/half-devices.ppm
cargo run --locked -p tore-app -- --flight-view 2 --flight-look 30,20 --flight-devices 0,1,1,0,0 --flight-controls 1,1,1 --capture-flight .local/control-surfaces.ppm
```

`--flight-devices` supplies gear, flaps, airbrake, hook and exhaust fractions in 0..1. `--flight-controls` supplies pitch, roll and rudder in -1..1. Captures with these flags pause at the requested pose; interactive launches set initial values and then run normal simulation. These are inspection options, not a new saved aircraft configuration.

## Validation and limits

Local Metal captures under ignored `.local/animation/` cover retracted, halfway, deployed and deflected controls, using side/below and top-oblique views. Original flame art and keyed gear textures are reused. The source is low-polygon artwork: this does not create volumetric wheels, wells or a new detailed aircraft mesh.

Regression tests cover actuator reversal and render interpolation, exhaust gating/settling, control release, unchanged deployed geometry/UVs, rigid rotation and rudder split attributes. Formatting, Clippy, build and 74 Rust tests passed; Python asset checks and Metal menu/viewer/flight smoke checks are also required by the repository workflow.

A 240-frame active Metal development run with all devices deployed and live exterior camera instrument completed 38 asynchronous camera readbacks. Excluding the first 30 frames: mean interval 16.75 ms, p95 17.31 ms, max 33.12 ms; mean simulation/cameras 0.49 ms and UI 1.35 ms. These are CPU wall intervals including VSync backpressure, not GPU timestamps. This run does not establish elimination of every stutter.

Hinges, door sequence, deflection limits, exhaust transition, dark nozzle material and control mixing are authored adaptations. Native continuous animation schedules, separate outboard ailerons/leading-edge flaps, nozzle petal articulation, wheel spin/steering, landing suspension, canopy operation, damage and store release remain open. Rudder partition is fitted, not recovered native moving-part topology. No new retail assets or converted reference-engine geometry are committed; the existing shared aircraft extraction profile already supplies the original SH/PIC resources used here.
