# Banked pull and angle of attack — 2026-09-13

The reported maneuver is banking while pulling up. The previous adapter drove the nose toward the velocity vector with a zero-AoA alignment target. Its pitch-rate calculation included gravity in aircraft-up but omitted the complementary gravity term in aircraft-right/body-yaw during a banked turn. A sustained pull could therefore lose its nose/flight-path separation while retaining uncommanded sideslip.

The HUD projection was checked independently against dot products with the aircraft's right/up/forward axes. It already includes bank and horizontal velocity. No artificial sideways offset or screen-space lag was added. In a settled coordinated turn, positive AoA projects below the nose; during roll-in/pull, lateral lag can occur and must change sign between left/right maneuvers.

## Correction and limits

- Add body-yaw transport `-body.right.y * gravity / speed` alongside the existing pitch response. The speed denominator remains bounded below by 60 ft/s. This is an authored kinematic turn correction, not recovered native rudder control.
- Replace zero-AoA alignment with a nose target above the flight path in the lift plane. The fitted target, in degrees, is `(2 + 1.25 * (G - 1)) * (450 KTAS / speed)^2`, bounded to -12..20 degrees with a 150 ft/s denominator floor. The existing 0.7/s alignment response and fixed-tick force integration remain.
- Keep velocity independent from attitude. The target changes nose response; it does not rotate velocity to the nose, fabricate HUD movement or impose a pitch limit.

The target is deliberately identified as an adapter fit. Lift is still commanded through PT envelope/load-factor response rather than derived from a recovered aerodynamic coefficient table. This is not complete native FA aerodynamics or a validated real-world Hornet model. There is no wind field yet, so air-relative AoA currently uses world velocity. At high AoA the actual flight-path marker may leave the existing HUD clipping region; it is not silently clamped to a false direction.

## Source fields still awaiting native interpretation

The supplied F18.PT already contains and the importer already preserves:

| Field | F18 raw value | Remaining work |
| --- | --- | --- |
| gpullAOA | 20 | Recover native scaling and load-dependent use |
| lowAOASpeed / lowAOAPitch | 70 / 15 | Recover threshold units and low-speed pitch law |
| rudderSlip / rudderDrag / rudderBank | 10 / 128 / 5 | Recover slip, drag and roll coupling |
| rudderYaw min/max/acc/dacc | -4 / 4 / 4 / 9 | Recover rate units and update timing |
| stallWarningDelay / stallDelay | 512 / 512 | Recover native timing and entry conditions |
| stallSeverity / stallPitchDown | 256 / 30 | Recover stall force/attitude behavior |

Body rotational limits, loading penalties and spin fields are also preserved. The reference research explicitly lists the AoA consumers as unmapped (`USNF-ATF/Docs/formats/pt.md` and `native-gear-pitch.md`). Raw field availability does not establish executable behavior. Full native force/control helpers, stall/spin transitions, mass/inertia and aerodynamic moment behavior remain parity work; the new fit does not reinterpret these unknown raw fields as degrees or rates.

## Reproduction and evidence

```sh
cargo run --locked -p tore-app -- --headless-flight 360 --maneuver bank-left
cargo run --locked -p tore-app -- --headless-flight 360 --maneuver bank-right
cargo run --locked -p tore-app -- --headless-flight 10800 --maneuver loop
cargo run --locked -p tore-app -- --maneuver bank-right --flight-probe-ticks 120 --capture-flight .local/banked-pull.ppm
```

The bank probes start at ±45 degrees bank, 450 KTAS, 5,000 feet, full throttle/afterburner, then hold pull for the requested ticks. Headless output includes actual body-relative AoA and sideslip. `--flight-probe-ticks` applies the same maneuver before rendered flight, bounded to 7,200 ticks; captures pause at that resulting state. Interactive use also needs `--free-flight`. The world terrain may change the rendered starting altitude/ground contact compared with the headless flat-ground probe.

F18 retail-profile headless results after 360 ticks (three seconds):

| Right-bank pull | Before | After |
| --- | --- | --- |
| AoA | 0.0677° | 9.3986° |
| Sideslip | 2.2411° | 0.0239° |
| Speed | 414.284 KTAS | 413.851 KTAS |
| Altitude | 5,487.803 ft | 5,492.948 ft |

Left-bank results mirror sideslip/bank and retain the same AoA, speed and altitude. These are adapter regression measurements, not original-game acceptance. The full loop still completes: 2,590 ticks, 427.958 KTAS, 4,971.080 feet, no crash, both vertical and inverted flags observed.

Synthetic regressions check mirrored sustained banked pulls, nonzero lateral lag during simultaneous roll/pull, positive load-related AoA, direct HUD/body-axis projection, determinism and complete loops. All 77 Rust tests pass, along with formatting, Clippy and build. Local Metal captures cover left/right banked cockpit pulls and exterior/live camera instrument; menu and viewer smoke checks remain part of validation. Logs/captures stay ignored in `.local/aoa/`.
