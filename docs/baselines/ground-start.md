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

Since 2026-09-23 the player's whole wing starts on the ground and the player
stands on the airport's own takeoff spot; see
[Whole-wing ground start](#whole-wing-ground-start-2026-09-23). The other wings
keep the airborne launch at the chosen altitude. Legacy and restricted native
modes reject ground start without changing adapters. Missing or obstructed
starts produce a notice instead of a partial launch.

## Weight-class runway wind

The [wind rules](../spec/runway-wind.md) use imported maximum takeoff weight
and the requested noticeable/rough/limit classes. The takeoff correction now
uses full atmospheric wind for aerodynamics and applies the difficulty fraction
to lateral tire grip instead of masking airflow. Headwind has no warning penalty
but does contribute to takeoff airspeed.

Current [takeoff/contact acceptance](takeoff-acceptance.md) covers all thirteen
selectable aircraft parked in wind with brakes on/off, position and attitude
hold, normal-load-scaled friction, signed drift, and wind-sensitive takeoff.
Threshold/decomposition tests still cover the weight boundaries and ten-knot
tailwind flag. Existing airborne 400-foot advection coverage remains valid.
Earlier `.local/runway-wind-review/` captures document the unchanged HUD labels
and class assignments; they are not proof of the current aerodynamic coupling.

Imported-resource validation records each selectable identity's MTOW and class
in `.local/runway-wind-review/creator.log`. The imported Su-25 maximum is 90,390
pounds and therefore selects 13/22/33 knots; the other twelve selectable entries
use 10/18/30. These are the imported game records, not replacement real-world
weights. No identity or source value is substituted to change the classification.

GPU captures in the same local directory show F/A-18D NOTICE, ROUGH, LIMIT,
tailwind LIMIT and ignored-headwind conditions, plus Su-25 NOTICE under the same
crosswind that gives the Hornet ROUGH. The roster wind HUD uses runway heading;
wheel coupling uses wheel heading. The required display smoke passes on RTX
4070/Vulkan. These are fitted game-behavior checks, not retail comparisons.

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
  Clippy, **941 Rust tests**, locked workspace build, **68 Python tests**,
  repository and both debug executable asset guards, documentation headers and
  diff whitespace. Two existing GPU unit tests remain ignored; explicit real
  display smoke/captures passed below.
- Synthetic tests cover idle support and acceleration, preserved fuel/payload,
  adapter rejection without mutation, rotated/inset departure poses, correct
  support-plane height, dominant paving selection, airport picker cancellation,
  theater change, and keyboard access to the new controls.
  All thirteen selectable aircraft also pass a 1,200-tick airport-2 run with
  `TORE_WIND=90,40`, ending at zero knots without crashing. Current logs are in
  `.local/gunsight-ground-review/`.
- The first airport in every one of the **16 base theaters** passed a stationary
  headless start. All **13 selectable aircraft**, including F/A-XX, held zero
  speed without crashing for 1,200 ticks at Simferopol. These are representative
  startup checks, not a validation of every aircraft/runway combination.
- Current default-load takeoff probes clear airport ground by 100 feet without
  crashing for all thirteen selectable aircraft. F/A-18D takes 12.35 seconds,
  Rafale C 9.93 seconds and A-4E 16.13 seconds using full power and fixed 0.35
  pitch input. These are fitted host probes, not retail performance measurements.
  [Full matrix and human test guidance](takeoff-acceptance.md).
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
carrier start or retail comparison was performed.

## Whole-wing ground start, 2026-09-23

Implementation mode. Behavior is specified in
[Quick Mission](../spec/quick-mission-menu.md#player-ground-start),
[AI airfield sequences](../spec/ai-airfield.md) and
[player landing priority](../spec/airports.md#wing-landing-orders-and-player-priority).
This work includes John's requests for whole-wing ground starts, landing and
bug-out commands, 200/300 nautical mile separations, and go-arounds instead of
non-catastrophic landing ejections. The extension of the ejection guard to
takeoff and the safety constants are agent decisions.

The Claude main session and nine subagent transcripts were recovered locally.
Their final validation agent stopped at an API session limit while checking
terrain clearance. Its prior review fixes were present: parked-player priority,
climb-out priority, device cleanup after cancelled approaches, bug out on the
inbound route, persistent cancellation of a joined landing, and preservation of
legacy terrain height. The branch was brought onto main at `6521723`, preserving
its radio, debrief, terrain and menu changes.

### Airport data

- **Airport points on real fields.** A throwaway probe loaded all 16 base
  theaters from the import. 231 of 311 airport entries yield a full set of
  takeoff, landing, taxi and parking points on the airport surface: every
  STRIP, STRIP1 to STRIP7 and STRIP3A field. The 22 DTSTRP vertical pads and
  the 58 STRIP5A, STRIP6A and STRIP7A entries do not: their shapes carry every
  point, but the points fall off their own paving. Those use the staggered
  runway fallback (`anchors-by-theater.log`).
- **The takeoff spot is the old runway start line.** On every one of the 231
  fields the takeoff spot (box 0x11) lies exactly on the host's near runway
  end, 0 ft away, and 100 ft behind the previous fitted player start. The host
  already used box 0x11 as the near end of its runway line, with the far end at
  the far edge of the whole airport mesh; it never used box 0x11 as the runway
  centre. Along the takeoff centerline the paving starts several hundred feet
  before the spot (about 510 ft on RNWY4, 490 ft on RNWY1). On RNWY1, RNWY3 and
  RNWY7 the landing aim point (box 0x12) is on a separate parallel centerline.
  On RNWY1 the whole mesh reaches 5,556 ft forward of the shape origin while
  the takeoff centerline's paving ends at 3,832 ft, so the host's runway line
  is longer than that paved strip. The runway geometry itself was not changed.
### Review fixes and regressions

- A hazardous steep final over the runway could continue until it crashed.
  It now requests a go-around while recoverable. Synthetic flights cover
  recoverable hazards over and short of the runway, an ordinary low final,
  and a critically damaged lander that still ejects.
- Rafale C and MiG-21 repeatedly floated past the runway and went around.
  The AI control mapping now accounts for the researched model's flap lift,
  flap-adjusted minimum speed and continuous low-speed G ceiling. Both now
  land and park. A synthetic shallow descent with flaps checks actual flight
  path and exact replay from the generated inputs.
- Correcting the control mapping exposed a fast touchdown after a ridge
  approach. The flare now starts easing according to height and speed, rather
  than waiting for a fixed 60 ft height. The synthetic ridge approach lands.
- At Ivano Frankivs'k, a wingman hit rising terrain while holding after
  takeoff. The terrain rule now covers climb-out, keeps the wings level
  during the correction, uses full military power for the climb and closes
  the speedbrake. Both wingmen complete the scenario below.

### Imported aircraft and airports

Linux, current local user-owned import in the isolated
`.local/ground-start-review/data/` profile. Logs and readable transcripts stay
under `.local/ground-start-review/`, outside Git. The shared development profile
was not written. These headless runs advance the same 120 Hz AI, flight,
terrain, combat and order services used by the app.

For the twelve-aircraft sweep, each player's wing has one AI wingman. The
scripted player takes off; the wingman receives Alt-L's order at tick 30,000
(250 seconds). All use UKR airport 2, Simferopol, 200 nm separation and the
normal imported loadout. The F-22 uses tick 15,000 instead, as explained below.
Times are simulation seconds since launch, not retail comparisons.

| Exact imported identity | Wingman liftoff | Parked |
| --- | ---: | ---: |
| F18.PT, F/A-18D | 137.9s | 1101.1s |
| RAFALE.PT, Rafale C | 120.2s | 1129.2s |
| F14.PT, F-14D | 133.0s | 1069.0s |
| A4E.PT, A-4E | 135.3s | 1098.1s |
| X31.PT, X-31 | 120.7s | 1087.7s |
| MIG29.PT, MiG-29 | 121.9s | 1137.9s |
| SU27.PT, Su-27 | 120.4s | 1127.3s |
| MIG21.PT, MiG-21 | 131.6s | 1092.8s |
| SU25.PT, Su-25 | 127.2s | 1101.7s |
| MIG23.PT, MiG-23 | 134.4s | 1053.9s |
| SU35.PT, Su-35 | 124.7s | 1104.5s |
| F22.PT, F-22A | 106.1s | 698.5s |

Every tested wingman above landed on its first approach and stayed alive.
Additional scenarios, each run for 216,000 ticks (30 simulated minutes):

| Scenario | Result |
| --- | --- |
| UKR airport 2, player plus three F/A-18D wingmen, order at tick 30,000 | All three parked by 1,715.1 s. Liftoff gaps 39.9 and 127.8 s, showing why there is no fixed takeoff interval. |
| UKR airport 12, Ivano Frankivs'k, two F/A-18D wingmen, order during takeoff at tick 18,000 | Both cleared the rising terrain and parked by 1,191.0 s. |
| UKR airport 1, two A-4E wingmen, bug out at tick 30,000 | Both returned, landed and parked by 1,158.7 s. |
| FRA airport 24, two F/A-18D wingmen, order at tick 30,000 | Unusable anchors selected the staggered runway fallback. Both parked by 1,093.3 s. |

A 300 nm start at UKR airport 12 was shortened to 292.7 nm and printed the
map-fit notice. The 200 nm Simferopol start turned enemy placement by about
32 degrees and retained the requested distance. Neither printed an off-map
start. Synthetic layout tests cover bounds, shortening, blocked slots and
fallbacks. The ordinary airborne probe and player headless flight also run. A parked
wingman stays in Waiting through 1,200 ticks while the player remains grounded;
two identical runs give checksum `3619c359cfcf68c3`.

Reproduce a full wing scenario from the repository root:

```sh
TORE_DATA_DIR=.local/ground-start-review/data target/release/tore-app --theater UKR --ground-start 2 --aircraft f18 --probe-wing-size 4 --maneuver takeoff --probe-wing-order 30000:land-selected --separation 200 --ai-probe-ticks 216000 --no-audio
```

### Final checks

All required Linux checks pass: formatting, workspace clippy with warnings
as errors, locked workspace tests and build, 75 Python tests, source and both
debug-binary asset guards, and documentation headers. The workspace tests
include 785 passing simulation tests and 447 passing app tests (three GPU
tests are ignored by the ordinary suite). The controls document was regenerated and checked.

Display smoke tests pass for the menu, creator ground start, airborne start
and `--fixture-wings` ground start; each creator smoke also passes its restart
comparison. These use the default one-aircraft player wing, so they do not
visually validate wingman taxi paths. The 1,200-tick headless player run ends
without a crash, and the ordinary 1,200-tick airborne AI probe keeps every
actor alive. The controller was detected only as hidraw on this host, so no
manual gamepad takeoff was validated.

### Scope of validation

The scripted human pilot is a simple takeoff-and-cruise harness and can hit
terrain later. It is not a validated player autopilot. In the original F-22
sweep, its wingman crashed in free-flight formation at 210.3 s, before the
250 s landing order. An order at 125 s exercised the same exact F22.PT aircraft's
successful landing sequence. Enemy X-31 and F-22 free-flight actors also hit
terrain. Those failures remain outside the airfield-sequence fixes; this is
not whole-sortie acceptance. Taxi traces sometimes leave paving briefly while
following the airport anchors, then return. No claim of collision-free taxi
paths on every field is made.

The keyboard map was checked in Chromium on all three sheets at 1920 by 1080
and 960 by 720. Alt-U and Alt-L fit their keys. The integration review also
restored main's Alt-S radio-silence binding to the map. Comms PNG exports at
1080p and 4K include the labels; ZIP and PDF code was unchanged.

Unvalidated: a creator launch with wingmen on a display, crosswind landing
coverage, multi-wing runway contention, every airport layout, Windows/macOS
runtime behavior and retail comparison. Vertical pads, STOVL and carriers
remain outside the conventional-aircraft sequence.
