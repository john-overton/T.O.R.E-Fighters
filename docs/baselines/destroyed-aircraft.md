# Destroyed aircraft validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Scope and result

Implementation pass, 2026-09-21, Linux/NVIDIA Vulkan host.
[Requested behavior and fitted physics](../spec/destroyed-aircraft.md).
No retail execution or new media extraction was needed. Living AI decisions
are unchanged; the bridge copies propulsion metadata for post-destruction use.

A shared wreck component now handles ownship and other aircraft. Tests cover:

- Exact polls at simulation ticks 120, 240, 360, 480, 600, 1200, 1800 and 2400;
  no time-zero/six-second roll, and exact success/failure draws at 4 and 5.
- Terrain contact wins over a scheduled airburst roll and ends future polling.
- Aerodynamic drag reduces speed, attitude changes, and the basis stays orthogonal.
  Identical initial state and ticks produce identical outcomes. Explosion RNG is
  independent of initial angular rates and the combat RNG.
- Balanced engines accelerate along the current body-forward direction. A single
  surviving engine adds yaw torque. Fuel exhaustion removes thrust. Ownship keeps
  engine power through destruction, consumes fuel, and ignores pilot input.
- Target airbursts emit once, remove the target and owner-linked fragments, create
  the explosion effect and do not increment the kill count. Ownship explosion
  emission is once-only. Player ground impacts stop motion and now produce one guaranteed explosion,
  including direct fatal collisions. Safe landings remain excluded.

## Display checks

Local GPU captures under `.local/wreck-pass/` were visually inspected.
`tumble-1.ppm` and `tumble-2.ppm` show the F/A-18D wreck at 0.5 and 2 seconds,
with visibly different roll/pitch/yaw. Both source engines remain active in this
fixture, each contributing approximately 4.432 ft/s² of forward acceleration;
the external fuel reading falls as the wreck continues flying.

`airburst.ppm` uses two preflight probe ticks to select a deterministic test run
whose first random roll succeeds. It shows the existing explosion effect and no
airframe. `gone.ppm` advances beyond the effect lifetime and shows neither model
nor fragments. Diagnostics remain at one poll and 120 wreck ticks after the
explosion, confirming no later rolls or wreck motion. This is a real seeded
5% roll, not a forced explosion branch.

```sh
target/debug/tore-app --free-flight --no-audio --flight-view 2 --damage-preview 1 --damage-preview-ticks 241 --capture-flight .local/wreck-pass/tumble-2.ppm --smoke-test
target/debug/tore-app --free-flight --no-audio --flight-view 2 --flight-probe-ticks 2 --damage-preview 1 --damage-preview-ticks 122 --capture-flight .local/wreck-pass/airburst.ppm --smoke-test
```

## Pilot death and player impact

The player pilot now dies when its nose/cockpit is lost at destruction. An incoming
synthetic gun round through the cockpit also emits pilot death without requiring
a breakup-sized regional hit. Component tests verify that death is permanent,
ground treatment cannot revive it, and surviving engine power is retained.

The camera transition test selects exterior view 1 (F10), recenters look/zoom,
closes the map, does not pause and does not repeat on later dead-pilot frames.
Actual nose-loss previews under `.local/pilot-impact/` start from requested view 0
and report `Pilot death preview: exterior view 1`.

Player wreck impact and direct ground-crash tests verify immediate pilot death,
no RNG poll on impact, stopped motion and hidden geometry. Combat tests verify
one explosion effect/event, removal of owned fragments and suppression of any
second impact or later airburst explosion. The deterministic GPU wreck reaches
the terrain at wreck tick 2,917, with eight unsuccessful airborne rolls. The
impact capture shows the explosion; the longer capture shows no aircraft after
the effect expires.

The smoke regression destroys ownship and marks the launcher/pilot inactive,
then verifies ten new aircraft puffs over 120 falling ticks. Impact and airburst
both stop new emission; those puffs remain and expire normally. A grounded wreck
without an active explosion effect also emits no new smoke. The host reports
impact/airburst before advancing smoke, avoiding a final unwanted puff on the
termination tick. The visually inspected
`.local/pilot-impact/falling-smoke.png` shows the dead-pilot wreck tumbling in
F10 view with its smoke trail; diagnostics count 20 puffs after 240 falling ticks.
The preview's paused banner is its capture mode, not the death-camera behavior.

## Validation and limits

Required formatting, warnings-denied workspace Clippy, locked workspace tests
and build, Python tool tests, source and executable asset checks, documentation
checks and the display smoke test passed. Captures and logs remain ignored.

The tumble coefficients, normalized engine offsets and yaw torque are fitted.
This is rigid aerodynamic wreck motion, not articulated fracture physics.
Per-engine asymmetry is verified in synthetic tests; the displayed GPU fixture
uses two surviving engines. Windows/macOS execution and a long live dogfight
were not run. Initial engine health/throttle are retained after death, without
new post-destruction thermal damage or fuel-leak scheduling.
