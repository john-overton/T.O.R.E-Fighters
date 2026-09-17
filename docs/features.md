# Feature matrix

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Selected player-facing features, grouped by area. This is not a complete list
of everything in the game or its code.

- **Retail manual:** ☑ means the feature is described in the [FA manual][manual].
- **Opinionated addition:** ☑ means we deliberately add or change behavior.
- Both may be checked when we extend a manual-described feature. An unchecked
  retail box means we are not claiming manual support, not that retail lacked it.
- Checkboxes describe the feature's origin. **Status** describes our game:
  Completed, Partially implemented with remaining work, or Planned. Completed
  does not mean tested against a running retail copy.

## Menus

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Choose Activity menu and dropdowns | ☑ | ☐ | Partially implemented. Menu navigation works; campaign, multiplayer and replay actions remain unavailable. | Manual pp. 11-13; [menu](baselines/main-menu.md) |
| Quick Mission setup | ☑ | ☐ | Partially implemented. Aircraft, theater and briefing edits work; multi-aircraft missions and objectives remain. | Manual pp. 18-20; [creator](baselines/creator-ordnance.md) |
| Load Ordnance editing | ☑ | ☐ | Partially implemented. Compatible weapons, quantities and internal fuel work; tanks, campaign stock and airbase restrictions remain. | Manual p. 16; [loadout](baselines/creator-ordnance.md) |

## Flight models

These rows cover ordinary free flight across the twelve supported aircraft. The
current flight model is the default; the previous model remains selectable.

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Pitch, roll and rudder control | ☑ | ☐ | Completed. All twelve aircraft have working flight controls. | Manual pp. 60-61; [aircraft coverage](baselines/aircraft-roster-expansion.md) |
| Speed- and altitude-dependent turning limits | ☑ | ☐ | Partially implemented. Aircraft limits are used; individual pitch/yaw response tuning remains. | Manual pp. 58-59; [flight model](FLIGHT-MODEL.md) |
| Throttle, afterburner and fuel use | ☑ | ☐ | Completed. Available engines and afterburners follow the selected aircraft's configuration. | Manual p. 61; [flight model](FLIGHT-MODEL.md) |
| Weapon weight affects handling | ☑ | ☐ | Partially implemented. Carried mass affects acceleration and loading; weapon-specific drag and external fuel transfer remain. | Manual p. 59; [flight model](FLIGHT-MODEL.md) |
| Stalls, spins and recovery | ☑ | ☐ | Partially implemented. Entry and recovery work; aircraft-specific handling review remains. | Manual p. 72; [flight tests](baselines/aircraft-roster-expansion.md) |
| X-31 low-speed control assistance | ☑ | ☑ | Partially implemented. Low-speed assistance works; full vector-control behavior and original animation timing remain. | Manual pp. 60, 81; [X-31 scope](spec/additional-aircraft.md) |
| Takeoff, touchdown, taxi and braking | ☑ | ☑ | Partially implemented. Test runways support these actions; validated theater runways and carrier landings remain. | Manual pp. 63-71; [landing limits](FLIGHT-MODEL.md#what-working-covers) |
| Smooth momentum and control response | ☐ | ☑ | Completed. Nose direction can differ from travel direction, with continuous movement through vertical and inverted flight. | [Flight response](FLIGHT-CONTROLS.md#flight-response-and-vertical-flight) |
| Select current or previous flight model | ☐ | ☑ | Completed. Both choices remain available. | [Flight options](FLIGHT-MODEL.md#run-and-reproduce) |

## Weapons

The [missile plan](missile-update-plan.md) contains the delivery stages, and the
[ordnance matrix](spec/missiles.md#first-pass-inventory-matrix) contains weapon values.

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Guns and manual weapon release | ☑ | ☐ | Partially implemented. Current aircraft stores fire in the range; remaining catalog weapons and full combat missions remain. | Manual pp. 124-126; [weapon coverage](baselines/aircraft-roster-expansion.md) |
| Missiles requiring continuous radar lock | ☑ | ☐ | Completed for supported default stores. | Manual pp. 117-118; [current missile behavior](spec/missiles.md#what-exists-today-and-what-changes) |
| Independent infrared guidance | ☑ | ☐ | Partially implemented. Seeker-owned IR observations, heat scoring and reacquisition work. HUD and tone delivery remain. | Manual p. 119; [missile scope](spec/missiles.md) |
| Delayed active-radar acquisition | ☑ | ☐ | Planned. Current active-radar missiles guide independently immediately after launch. | Manual p. 118; [activation](spec/missiles.md#activation-and-independent-acquisition) |
| Per-weapon pitbull activation distances | ☐ | ☑ | Planned. Initial distances are recorded in the matrix. | [Activation rules](spec/missiles.md#activation-and-independent-acquisition) |
| Emitter-homing missiles | ☑ | ☐ | Completed in controlled fixtures. Explicit radar-emission eligibility and shutdown work without IR fallback. No ground systems or catalog loadouts added. | Manual pp. 117, 120; [guidance types](spec/missiles.md#four-game-guidance-types) |
| Aircraft velocity, motor boost and target-motion estimates | ☐ | ☑ | Partially implemented. Current missile profiles inherit full velocity and finite boost; the matching predictor exists. Seeker lead and HUD integration remain. | [Launch motion](spec/missiles.md#launch-velocity-and-intercept-estimates) |
| Uncued launch with the onboard seeker enabled | ☐ | ☑ | Planned. Includes radar, IR and emitter seekers; supported-radar weapons still need lock. | [Launch modes](spec/missiles.md#uncued-launch-and-narrow-ir-search) |
| Narrow IR search and heat-quality selection | ☐ | ☑ | Completed in simulation. Three-degree search, heat scoring and 0.25-second dwell have synthetic tests. Presentation remains. | [IR rules](spec/missiles.md#fitted-heat-quality-and-tone) |
| Missile seeker diamond, range scale and lock tone | ☑ | ☐ | Planned. Weapon HUD and sound integration remain. | Manual pp. 83-84, 119; [HUD delivery](spec/missiles.md#weapon-hud-delivery) |
| HUD search cone and launch-mode display | ☐ | ☑ | Planned. Cone projection and mounted-seeker feedback remain. | [HUD additions](spec/missiles.md#weapon-hud-delivery) |
| Separate guidance lifetime and lock-loss memory | ☐ | ☑ | Completed in simulation. Guidance expiry is separate from motor and removal. Lost seekers keep their target and can reacquire until guidance expiry. | [Lifetime rules](spec/missiles.md#range-motor-and-tracking-lifetime) |

## Systems and controls

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Radar search/tracking modes and contact history | ☑ | ☐ | Completed for the current air-to-air scope. | Manual pp. 96-99; [radar](radar.md) |
| RCS exposure display | ☑ | ☑ | Completed. Uses the same aircraft exposure calculation as detection. | Manual pp. 94-95; [RCS display](spec/rcs.md) |
| Radar notching and generation-based jammer tuning | ☐ | ☑ | Partially implemented. Detection effects work; side-by-side aircraft tuning remains. | [Radar tuning](radar.md#deliberate-departures-and-known-approximations) |
| Persistent selection of search-only contacts | ☐ | ☑ | Completed. Selection and firing permission remain separate. | [Selection](radar.md#mouse-designation-and-missiles) |
| Destroyed aircraft remain visible to sensors | ☐ | ☑ | Completed while the wreck remains airborne. | [Destroyed aircraft](radar.md#destroyed-aircraft-remain-sensor-objects) |
| Rebindable modern controller profiles | ☐ | ☑ | Completed. Gamepads, sticks, throttles and pedals use the in-flight binding editor. | [Input](INPUT.md) |

## Maps, weather and atmosphere

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Select and fly over the original theaters | ☑ | ☐ | Completed. All sixteen theaters are selectable. | Manual p. 198; [terrain](baselines/ukraine-viewer.md) |
| Weather and time-of-day presentation | ☑ | ☐ | Partially implemented. Six weather choices and day/night presentation work; remaining cloud forms and special weather effects remain. | Manual p. 198; [weather coverage](formats/weather.md) |
| Physical turbulence and its disable option | ☑ | ☐ | Partially implemented. Low-altitude disturbance works; live neighboring-aircraft wake effects remain. | Manual appendix D, No Turbulence; [weather flight effects](baselines/wind-turbulence-vapor.md) |
| Animated ocean ripples and reflections | ☐ | ☑ | Completed. Original water colors and textures are retained. | [Ocean](spec/ocean.md) |
| Temperature-aware atmosphere and speed calculations | ☐ | ☑ | Partially implemented. Air-data calculations work; calibrated airspeed and pressure-based cockpit instruments remain. | [Air-data scope](FLIGHT-MODEL.md#independent-aircraft-models-and-future-gauges) |

[manual]: https://pdfcoffee.com/famanual-pdf-free.html
