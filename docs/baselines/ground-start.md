# Quick Mission ground-start validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-20. John requested a ground-start option in the
Quick Mission creator. [Behavior and fitted defaults](../spec/quick-mission-menu.md#player-ground-start).
Validation host: Linux, NVIDIA GeForce RTX 4070, Vulkan. No original executable
was run, no retail-derived assets were committed, and no flight adapter default
was changed. Earlier uncommitted airport-audio work was preserved.

## Delivered behavior

The creator exposes Start (Airborne/Ground) and a theater-specific airport list.
Popup cancellation preserves the draft; changing theater resets the airport
choice. Airborne remains the default. The accepted runway is kept through loadout
selection and restart. The player starts stationary on the shared runway plane
with the aircraft's own wheel/CG offset, engine idling, gear/flaps down and brakes
applied. Selected fuel and stores survive initialization. Existing B releases
brakes and the normal throttle/pitch inputs begin manual takeoff.

Other selected aircraft use their existing airborne spawn reference at the chosen
altitude. No autonomous ground traffic or new AI behavior was added. Legacy and
restricted native modes reject ground start without changing adapters. Missing
or obstructed runway starts produce a notice instead of a partial launch.

## Corrections found during validation

Composite runway shapes include buildings. Their overall vertical bounds cannot
supply the runway's contact-plane height. Contact now uses the plane through the
authored runway origin, including supported orientation. A synthetic regression
checks that raising attached building bounds does not raise the runway surface.

Near-ground captures also exposed paving below the mesh origin in some variants.
RNWY1's dominant horizontal layer is at source -4 feet after header scaling. A
fitted per-shape vertical correction aligns this layer with the contact plane.
Solid and textured detail passes use different depth bias, so no separate
physical texture-face lift puts the pavement above the wheels. The ILS airport
altitude reference remains unchanged. [Rendering rules](../spec/airports.md).

## Checks and results

- All required checks passed: formatting, warnings-denied workspace/all-target
  Clippy, **908 Rust tests**, locked workspace build, **68 Python tests**,
  repository and both debug executable asset guards, documentation headers and
  diff whitespace. Two existing GPU unit tests remain ignored; explicit real
  display smoke/captures passed below.
- Synthetic tests cover idle support and acceleration, preserved fuel/payload,
  adapter rejection without mutation, rotated/inset departure poses, correct
  support-plane height, dominant paving selection, airport picker cancellation,
  theater change, and keyboard access to the new controls.
- The first airport in every one of the **16 base theaters** passed a stationary
  headless start. All **13 selectable aircraft**, including F/A-XX, held zero
  speed without crashing for 1,200 ticks at Simferopol. These are representative
  startup checks, not a validation of every aircraft/runway combination.
- Default-load F/A-18D and Rafale C takeoff probes used released brakes, full
  throttle, available afterburner and a fixed 0.35 pitch input. The probes stop
  after clearing airport ground by 100 feet. F/A-18D succeeded after 3,897 ticks
  at 171.082 knots; Rafale C after 2,555 ticks at 169.314 knots. Neither crashed.
  These are fitted host probes, not retail takeoff-performance measurements.
- Simferopol initialization placed the F/A-18D at 1,032 ft MSL: airport ground
  1,024 ft plus its 8-ft wheel/CG offset. The Panama launch used 8 ft MSL over
  zero-elevation airport ground. Legacy mode and an invalid airport number were
  rejected explicitly. The ordinary airborne start still passed.
- The actual creator launch action was exercised on a real window for F/A-18D,
  Rafale C, the straight-flight wing compatibility mode, an airborne start and
  a Panama ground start. Each smoke run restarted through the normal flight
  action and compared accepted position, heading, speed, gear, fuel, payload,
  selected airport and all target initial states. All matched. Ground launches
  reported supported players and two airborne selected wing aircraft.
- The required display smoke passed. Creator normal/airport-popup snapshots,
  external ground-start captures in Ukraine and Panama, and a terrain rendering
  regression capture were inspected. Pavement is visible beneath the aircraft,
  gear is deployed, and the original controls/artwork are reused.

Evidence is ignored local output under `.local/ground-start-review/`:
`checks.json`, `probes.json`, `render-checks.json`, the per-case logs and captures.
The app's source cache was reused. These controlled probes used `TORE_WIND=0,0`
to isolate startup from the pre-existing negative mission-wind limitation.

Reproduction examples:

```sh
target/debug/tore-app --quick-mission --ground-start 2 --snapshot .local/ground-start-review/normal.ppm
target/debug/tore-app --launch-quick-mission --ground-start 2 --capture-flight .local/ground-start-review/creator-f18.ppm --flight-view 2 --smoke-test --no-audio
target/debug/tore-app --ground-start 2 --headless-flight 1200 --no-audio
target/debug/tore-app --ground-start 2 --headless-flight 7200 --maneuver takeoff --no-audio
```

The numeric diagnostic selects an airport in the current theater; the normal
creator uses names. `--launch-quick-mission` follows the same creator/loadout/flight
actions as clicking OK. With `--smoke-test`, it also checks restart before capture.
Ground pilot-input replays require the same aircraft, theater and ground-start
selection, consistent with the existing tape's matching-initial-state contract.

No manual joystick takeoff session, Windows/macOS runtime check, cold start,
carrier start or retail comparison was performed. Ground start is player-only;
AI taxi/takeoff sequencing remains outside this work. No commit or push was made.
