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
| Quick Mission setup | ☑ | ☑ | Partially implemented. All six wings launch selected aircraft as straight-flying dummies, up to 29 plus the player. Combat AI and objectives remain. | Manual pp. 18-20; [creator](baselines/creator-ordnance.md) |
| Load Ordnance editing | ☑ | ☐ | Partially implemented. Compatible weapons, quantities and internal fuel work; tanks, campaign stock and airbase restrictions remain. | Manual p. 16; [loadout](baselines/creator-ordnance.md) |

## Flight models

These rows cover ordinary free flight across the twelve supported aircraft. The
current flight model is the default; the previous model remains selectable.

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Heading/altitude and waypoint autopilot | ☐ | ☑ | Partially implemented. Requested USNF-ATF modes, pilot override, HUD and input recording work. Waypoint target API is ready; route selection and sequencing remain. Steering is fitted, retail parity unverified. | [Autopilot](spec/autopilot.md) |
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
[Current-store validation and remaining tuning](baselines/missiles.md) records
the Linux range, replay and rendered acceptance.

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Aircraft damage appearance and smoke | ☑ | ☑ | Completed across all twelve supported aircraft with fitted breakup rules. Original bodies and detached pieces render at half health; damaged aircraft trail smoke. Pieces fall, disappear on ground contact and play an impact effect. Exact retail transitions remain unverified. | Manual pp. 161-163; [visual damage](spec/damage-smoke.md); [validation](baselines/damage-smoke.md) |
| Missile motor smoke | ☐ | ☑ | Completed with fitted timing and size. Original white puff art emits during powered flight only, then disperses independently. | [Smoke rules](spec/damage-smoke.md) |
| Guns and manual weapon release | ☑ | ☐ | Partially implemented. Current supported stores are available in normal starts, custom missions and the range. Pilot-only recordings retain clean loads. Remaining catalog weapons and full combat missions remain. | Manual pp. 124-126; [weapon coverage](baselines/aircraft-roster-expansion.md) |
| Missiles requiring continuous radar lock | ☑ | ☐ | Completed for supported default stores. Lost support stops measured guidance; the original target can be reacquired before guidance expiry. | Manual pp. 117-118; [current missile behavior](spec/missiles.md#what-exists-today-and-what-changes) |
| Independent infrared guidance | ☑ | ☐ | Completed for A2A stores; surface designation remains deferred. Seeker-owned IR observations, heat scoring, reacquisition, HUD and fitted tone work. Imported sample assignment is fitted. | Manual p. 119; [missile scope](spec/missiles.md) |
| Delayed active-radar acquisition | ☑ | ☐ | Completed. Cued shots fly to a supported intercept before enabling their own seeker. Acquisition is separate from activation. | Manual p. 118; [activation](spec/missiles.md#activation-and-independent-acquisition) |
| Per-weapon pitbull activation distances | ☐ | ☑ | Completed. All nine configured activation distances have boundary tests; pitbull requires acquisition. | [Activation rules](spec/missiles.md#activation-and-independent-acquisition) |
| Emitter-homing missiles | ☑ | ☐ | Completed in controlled fixtures. Explicit radar-emission eligibility and shutdown work without IR fallback. No ground systems or catalog loadouts added. | Manual pp. 117, 120; [guidance types](spec/missiles.md#four-game-guidance-types) |
| Aircraft velocity, motor boost and target-motion estimates | ☐ | ☑ | Completed. Full velocity inheritance, finite boost, limited turns and fitted maneuver losses are shared by flight and prediction. Estimated maximum range responds to launch velocity, attitude and observed target motion. Active-radar predicted reach can exceed nominal launch range; minimum range and onboard seeker limits remain enforced. | [Launch motion](spec/missiles.md#launch-velocity-and-intercept-estimates) |
| Uncued launch with the onboard seeker enabled | ☐ | ☑ | Completed. Automatic armed/no-designation bore, release-lock button, rebindable action and clickable mode label permit independent A2A seeker release without designation. Known targets inside minR inhibit release; blind shots cannot engage inside minR. Surface weapons do not use A2A bore. Radar power off disables radar bore and allows permanently unguided radar-missile release; IR bore remains independent, with selected-track priority and bore fallback on release; bay and normal release gates remain. | [Launch modes](spec/missiles.md#uncued-launch-and-narrow-ir-search) |
| Narrow IR search and heat-quality selection | ☐ | ☑ | Completed. Circular five-degree bore search, centre-weighted signal scoring and 0.25-second dwell drive acquisition, HUD and tone. | [IR rules](spec/missiles.md#fitted-heat-quality-and-tone) |
| Missile seeker diamond, range scale and lock tone | ☑ | ☐ | Completed with fitted layout, in-range radar diamond blink and imported search/lock samples at doubled default volume. Local manual figures were inspected; exact retail sound mapping and probability formula remain unknown. An explicitly fitted hit estimate is implemented. | Manual pp. 83-84, 119; [HUD delivery](spec/missiles.md#weapon-hud-delivery) |
| HUD search cone and launch-mode display | ☐ | ☑ | Completed. Rail-aligned search boundaries, mode labels, acquisition status and upper-right shot details share simulation state. Bore uses a provisional blinking diamond and range triangle; short retail labels and bare hit percentages align below speed in a smaller forward HUD; the range scale sits inside altitude, with radar R/C/A below it and ARM above the weapon. IN RNG blinks beside the percentage from predicted reach. Two horizontal bars mark the fitted favorable firing-range window. Armed missile readouts replace lower flight readouts. | [HUD additions](spec/missiles.md#weapon-hud-delivery) |
| Separate guidance lifetime and lock-loss memory | ☐ | ☑ | Completed in simulation. Guidance expiry is separate from motor and removal. Lost seekers keep their target and can reacquire until guidance expiry. | [Lifetime rules](spec/missiles.md#range-motor-and-tracking-lifetime) |

## Systems and controls

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Radar search/tracking modes and contact history | ☑ | ☐ | Completed for the current air-to-air scope. Pointer takeover covers the full black screen up to the bezel, including radar-off display. | Manual pp. 96-99; [radar](radar.md) |
| RCS exposure display | ☑ | ☑ | Completed. Uses the same aircraft exposure calculation as detection. | Manual pp. 94-95; [RCS display](spec/rcs.md) |
| Radar notching and generation-based jammer tuning | ☐ | ☑ | Partially implemented. Detection effects work; side-by-side aircraft tuning remains. | [Radar tuning](radar.md#deliberate-departures-and-known-approximations) |
| Persistent selection of search-only contacts | ☐ | ☑ | Completed. Selection and firing permission remain separate. With master arm off, a selected current radar contact still has a HUD box. | [Selection](radar.md#mouse-designation-and-missiles) |
| Destroyed aircraft remain visible to sensors | ☐ | ☑ | Completed while the wreck remains airborne. | [Destroyed aircraft](radar.md#destroyed-aircraft-remain-sensor-objects) |
| Centered HUD zoom and wide view | ☑ | ☑ | Completed. Zoom uses the HUD center; below 1x the cockpit and mirrors hide while HUD and instruments remain. | Manual p. 104 describes magnification; [requested presentation](baselines/cockpit-slide.md) |
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
