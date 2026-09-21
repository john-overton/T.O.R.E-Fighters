# Target window and Dummy aircraft validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-09-21. See the [target-window spec](../spec/target-window.md)
and [Dummy aircraft spec](../spec/dummy-aircraft.md) for requested behavior,
fitted constants and unresolved data.

## Checks

The workspace formatting check, Clippy with warnings denied, locked workspace
tests and build, 68 Python tests, documentation headers and source/binary asset
checks passed. The Rust workspace passes 986 tests.

Synthetic tests cover clock directions/wraparound, three-second phase boundaries,
damage endpoints/clamping, and player-directed attack underlining. Hi/Lo tests
cover just inside, exactly at, and just outside +/-10 degrees at several ranges,
plus overhead, directly below and coincident targets. Camera tests
project a synthetic model through the render camera matrix at multiple ranges,
from above/below and overhead. They verify the camera stays on the player-target
segment within one nautical mile of the subject, retains player position for
nearby targets, handles coincident positions, preserves the full sight line,
and fills one image dimension without entering the text margins.

Dummy tests cover all six skill menus, exclusion of the human slot, immunity to
enemy-skill overrides, mixed normal/Dummy missions, and the live target mirror.
A ten-second simulation checks exact 400-knot displacement, unchanged heading,
altitude, fuel and ammunition, no launches/devices/orders, command rejection,
and no further dummy movement after destruction.

Depth tests reproduce equal depth values for surfaces 1.5 inches apart at
60,000 feet with the old one-foot near plane, then verify distinct values with
the target-fitted near plane. Other camera defaults remain one foot. Grayscale
checks verify 10% scenery darkening and unchanged foreground brightness. Preview
cadence tests request 240 updates over ten seconds at 24, 30, 60 and 144 Hz host
rates, and skip missed updates without catch-up bursts.

## Visual review

The display-capable Linux host passed `cargo run --locked -p tore-app --
--smoke-test`. These captures were visually inspected:

- One-nautical-mile camera limit: `--live-fire --hud-target-preview -90,-2,60000
  --capture-flight .local/target-one-nm-far.ppm` and the same command with
  `30,15,4000` written to `.local/target-one-nm-near.ppm`. Both targets fill the
  view. The far readout remains 9.9 NM from the player while the camera is within
  1 NM of the target; the nearer target reads 0.7 NM and uses player position.
- Player-perspective elevated target, large windows: `--live-fire
  --hud-target-preview 30,15,12000 --capture-flight
  .local/target-player-perspective.ppm`.
- Distant lower target, small windows: `--live-fire
  --hud-target-preview -90,-2,60000 --instrument-layout small
  --capture-flight .local/target-player-far.ppm`. It remains framed at 9.9 NM.
- Shallow elevated target: `--live-fire --hud-target-preview 0,5,12000
  --capture-flight .local/target-elevation-five.ppm`. Visually checked that
  12:00 has no HI suffix despite more than 1,000 feet of altitude separation.
- Background contrast: `.local/target-contrast-new.ppm`, using the shallow-target
  command above. A matched background pixel changes from RGB 204 to 184; the
  frame remains RGB 98/115/143. The target remains at its prior brightness.
- Long-range depth/contrast: `.local/target-depth-matched.ppm`, using the distant
  lower-target command above. Visually checked the aircraft surfaces and outline.
- Creator skill menu: `--quick-mission --snapshot-state field-22
  --snapshot .local/dummy-skill-menu.ppm`. Dummy (400 KTS) appears after Ace.
- Earlier captures also checked exact Rafale C identity and the speed phase at
  360 ticks, showing 178 KTS for the unchanged legacy fixture. Their old fixed-height
  Hi/Lo labels are superseded by the current angular rule.

An initial integration capture caught a weather-slot indexing error, corrected
before these passing captures. Retail-derived images remain ignored in `.local/`.

## Limits

No continuous-motion flicker capture, measured live GPU delivery-rate trace,
retail side-by-side run, Windows/macOS execution, ground-target visual capture,
or live pilot activity transition capture was performed. Dummy movement and
mixed-mode integration were exercised with synthetic profiles; the creator menu
was rendered from local retail UI assets. Existing combat AI decisions and
flight adapters remain separate from the new constant-motion training mode.
Current launch data has no objective assignments, so `OBJECTIVE ?` remains
deliberate. Player-specific evasion and impending-fire prediction are not claimed.
