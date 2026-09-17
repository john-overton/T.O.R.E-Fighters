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
