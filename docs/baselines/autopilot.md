# Autopilot validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation on 2026-09-17, Linux, for the
[autopilot specification](../spec/autopilot.md). Synthetic fixtures only in tests.

- Four closed-loop cases pass: heading hold and waypoint turns on hybrid and
  legacy adapters, each simulated for 120 seconds at 120 Hz. The specified
  heading, altitude, bank and no-stall/no-crash gates pass.
- Mode switching retains capture; same-mode toggles turn off; missing, invalid
  and coincident targets fall back; north-crossing bearings turn the short way.
- All three pilot axes retain engagement at 0.15 and disengage above it.
  Ground/crash disengagement and manual throttle/gear behavior pass.
- Numbered target labels retain the supplied waypoint number (`WP 7`); missing
  targets show `WP --`. HUD placement follows the requested two-line upper-left
  layout in the specification.
- Mode input tape roundtrip and identical simulation replay pass. Keyboard,
  menu action and modifier-isolation tests pass.
- `cargo fmt --all -- --check`, workspace Clippy with warnings denied,
  workspace tests (502 passed), and workspace build pass, all Cargo dependency
  operations using `--locked`.
- Python tools tests (40 passed), source and both executable asset checks,
  documentation header check and `git diff --check` pass.
- `cargo run --locked -p tore-app -- --smoke-test` presents successfully on
  NVIDIA GeForce RTX 4070 Vulkan. The additional `--free-flight --smoke-test`
  also passes. These are rendering smoke tests, not a human
  evaluation of autopilot handling or HUD legibility.

Retail comparison, closed-loop restricted-native-table flight, and all-aircraft
handling validation were not run. Controller hardware was detected by the smoke
test but physical autopilot button operation was not exercised. No mission
waypoints exist yet; target guidance is exercised through synthetic API inputs.

## Hover hold and the powered-lift aircraft (2026-10-09)

Implementation validation of VTOL overhaul slice P9 on Linux, synthetic
aircraft carrying the AH-64, Mi-24, CH-47 and V-22 PT numbers
(`crates/tore-sim/src/flight/powered/hover_hold_tests.rs`).

- A1, engaged at 30 kt with a 15 kt crosswind at Damper: drift below 1 kt
  after 7.3 s (AH-64), 7.3 s (Mi-24), 6.8 s (CH-47), 8.7 s (V-22); then over
  60 s the largest distance from the held point 5.9, 6.2, 2.7 and 6.2 ft, the
  largest height change 1.6, 1.2, 1.7 and 1.9 ft. Gates 20 s, 20 ft, 10 ft.
- A1 across every stability level, with and without Easy flight physics, in
  calm air, 15 kt from four sides and a gusting wind (15 kt with 5 kt gusts
  every 7 s and 3 kt every 3 s), on a south-westerly heading: worst capture
  9.7 / 11.0 / 6.9 / 9.2 s, worst position 6.8 / 6.1 / 3.7 / 4.5 ft, worst
  height 2.5 / 2.7 / 3.0 / 3.0 ft, worst heading 2.3 / 1.5 / 1.1 / 0.3
  degrees (AH-64 / Mi-24 / CH-47 / V-22).
- A2 refusals, A3 cancels (each on its tick with its message; trim keys and
  a lever held still do not cancel), the 10 ft floor and the trim-key nudge
  pass. A4: 1,800 ticks of hover hold in a gusting wind, replayed by hand
  from the inputs the flight model received, are bit-identical on all four.
  A hold restored from the exact state (alone and against a baseline) and
  from a checkpoint, while braking and while holding, flies on bit for bit
  for 1,200 ticks.
- The A and Ctrl+A modes on all six powered-lift aircraft from their airborne
  starts, at every stability level, with and without the cheat, after a
  20-degree, 100 ft upset or a 45-degree waypoint turn: heading within 3
  degrees after 120 s, altitude within 30 ft from 30 s on (heading mode) and
  within 50 ft throughout the turn, speed within 10 kt. Before this slice the
  same modes engaged in a hover put every rotorcraft and both jets out of
  control (the V-22 and AV-8 crashed within 80 s), and the waypoint turn cost
  the helicopters 160 to 270 ft.
- The fixed-wing autopilot tests above are unchanged and pass; the golden
  fingerprints are unchanged. Workspace format, Clippy with warnings denied,
  tests and binary build pass. The headless battery for the change (105
  scenarios chosen against `48d19295`) and the `flight-autopilot-*`,
  `flight-waypoint-*`, `flight-variety-*`, `flight-animation-*` and
  `flight-overspeed-*` scenarios (137) pass.

Not run: the 13 windowed battery scenarios the change selects (free flight,
quick restarts, the audio and ground start ones), any windowed or human flight
of hover hold, and a retail comparison (none exists: retail players never
flew rotorcraft).
