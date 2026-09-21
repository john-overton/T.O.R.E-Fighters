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
| Import cache cleanup | ☐ | ☑ | Implemented. Successful import or startup load removes older numbered packs after validating the retained pack. | [Contract](spec/import-cache.md) |
| Choose Activity menu and dropdowns | ☑ | ☐ | Partially implemented. Menu navigation works; campaign, multiplayer and replay actions remain unavailable. | Manual pp. 11-13; [menu](baselines/main-menu.md) |
| Quick Mission setup | ☑ | ☑ | Partially implemented. Right-click cycles setup values backward. All six wings launch the selected aircraft, up to 29 plus the player, carrying side, wing, member and the wing's selected skill. Those wings use AI by default in separate delta formations; `--fixture-wings` retains straight-flight fixtures. Mission objectives remain. The original gives every member of a wing the wing's selected skill, which is what the payload does. | Manual pp. 18-20; [creator](baselines/creator-ordnance.md), [AI experience](spec/ai-experience.md) |
| Load Ordnance editing | ☑ | ☐ | Partially implemented. Compatible weapons, quantities and internal fuel work; tanks, campaign stock and airbase restrictions remain. | Manual p. 16; [loadout](baselines/creator-ordnance.md) |

## Flight models

These rows cover ordinary free flight across twelve retail aircraft and the F/A-XX concept. The
current flight model is the default; the previous model remains selectable.

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| F/A-XX concept variant | ☐ | ☑ | Implemented with hidden fins, split flap rudder animation and a concealed retractable hook. F-22 handling retained; independent drag physics remains. Experimental separate original-format aircraft exported; original FA flight and decal cleanup confirmed by John; export hook capability now enabled, awaiting a new live check; detailed control/damage checks and Kapset compatibility remain ([export](spec/fa-xx-export.md)). | [Contract](spec/fa-xx.md) |
| Heading/altitude and waypoint autopilot | ☐ | ☑ | Partially implemented. Requested USNF-ATF modes, pilot override, HUD and input recording work. Waypoint target API is ready; route selection and sequencing remain. Steering is fitted, retail parity unverified. | [Autopilot](spec/autopilot.md) |
| Expanded flight HUD and startup modes | ☐ | ☑ | Implemented across the selectable roster: expanded ladder window trimmed by 20%, weapon/ILS rows below the speed/altitude boxes, wider bank arc raised beneath the ladder, 25% tighter ladder spacing and motion calibrated to actual pitch, with readable +/-90-degree marks and a separate world-aligned zero bar, lower boxed TAS/MSL values without surrounding numbers or hash marks, no fixed aircraft datum, and AGL/VS only for active ILS. HUD symbols clip to aircraft-specific glass and render behind cockpit artwork. Ground starts use NAV; airborne starts select and arm the canonical gun. | [Layout and defaults](spec/hud-layout.md) |
| Pitch, roll and rudder control | ☑ | ☐ | Completed. All twelve aircraft have working flight controls. | Manual pp. 60-61; [aircraft coverage](baselines/aircraft-roster-expansion.md) |
| Speed- and altitude-dependent turning limits | ☑ | ☐ | Partially implemented. Aircraft limits are used; individual pitch/yaw response tuning remains. | Manual pp. 58-59; [flight model](FLIGHT-MODEL.md) |
| Throttle, afterburner and fuel use | ☑ | ☐ | Completed. Available engines and afterburners follow the selected aircraft's configuration. | Manual p. 61; [flight model](FLIGHT-MODEL.md) |
| Weapon weight affects handling | ☑ | ☐ | Partially implemented. Carried mass affects acceleration and loading; weapon-specific drag and external fuel transfer remain. | Manual p. 59; [flight model](FLIGHT-MODEL.md) |
| Stalls, spins and recovery | ☑ | ☐ | Partially implemented. Entry and recovery work; aircraft-specific handling review remains. | Manual p. 72; [flight tests](baselines/aircraft-roster-expansion.md) |
| X-31 low-speed control assistance | ☑ | ☑ | Partially implemented. Low-speed assistance works; full vector-control behavior and original animation timing remain. | Manual pp. 60, 81; [X-31 scope](spec/additional-aircraft.md) |
| Takeoff, touchdown, taxi and braking | ☑ | ☑ | Partially implemented. Base-theater runway surfaces, airport-relative ILS calculations and manual landing completion work. Hybrid flap lift/airflow drag, moderate continuous low-speed trim and load-scaled wheel release are implemented. Full wind affects takeoff airflow; MTOW-class thresholds and a 10-knot tailwind limit drive tire-grip difficulty and HUD cues. Static tire grip prevents parked wind drift ([wind rules](spec/runway-wind.md)). ILS arming requires the threshold within a 90-degree forward cone, 5 NM and 4,000 feet above airport ground. Tower text controls and reviewed clearance/completion recordings work. Speed brackets, campaign overlays, carrier landings and cross-platform runtime validation remain. | Manual pp. 63-71; [airport behavior](spec/airports.md); [takeoff acceptance](baselines/takeoff-acceptance.md) |
| Quick Mission player ground start | ☐ | ☑ | Implemented with a theater-specific airport picker, supported stationary player start, preserved loadout and restart. Other wings remain airborne. Requires the researched flight model; autonomous taxi/takeoff and cold starts are not added. | [Start specification](spec/quick-mission-menu.md#player-ground-start) |
| Smooth momentum and control response | ☐ | ☑ | Completed. Nose direction can differ from travel direction, with continuous movement through vertical and inverted flight. | [Flight response](FLIGHT-CONTROLS.md#flight-response-and-vertical-flight) |
| Select current or previous flight model | ☐ | ☑ | Completed. Both choices remain available. | [Flight options](FLIGHT-MODEL.md#run-and-reproduce) |

## Weapons

The [missile plan](missile-update-plan.md) contains the delivery stages, and the
[ordnance matrix](spec/missiles.md#first-pass-inventory-matrix) contains weapon values.
[Current-store validation and remaining tuning](baselines/missiles.md) records
the Linux range, replay and rendered acceptance.

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Aircraft damage appearance and smoke | ☑ | ☑ | Implemented across all twelve supported aircraft with six fitted impact regions. Light damage adds persistent imported-texture marks; concentrated damage grows mirrored wing or fin tears, or selects a matching reviewed A/B or C/D body and fragment at the 75% regional threshold. Unrelated light damage cannot remove the F/A-18D nose. Gun damage is one third of configured damage, while direct cockpit and high-damage central gun hits remain critical. Damaged aircraft emit 10 smoke puffs per second with cloud-matched weather lighting. Exact retail transitions remain unverified. | Manual pp. 161-163; [visual damage](spec/damage-smoke.md); [validation](baselines/damage-smoke.md) |
| Missile motor smoke | ☐ | ☑ | Completed with fitted timing and size. Original white puff art emits at 15 puffs per second during powered flight only, at half the previous size, then disperses independently with cloud-matched weather lighting. Original emission timing remains unverified. | [Smoke rules](spec/damage-smoke.md) |
| Engine contrails | ☐ | ☑ | Opinionated addition: 10 pale puffs per second per engine above a stable random 30,000-35,000-foot onset altitude, two-minute lifetime with a linear fade after one minute and cloud-matched weather lighting. Instanced rendering and a separate budget support full trails for 30 twin-engine aircraft. Some engine attachments remain fitted. | [Contrail rules](spec/damage-smoke.md); [validation](baselines/smoke-contrails.md) |
| Guns and manual weapon release | ☑ | ☐ | Partially implemented. Current supported stores are available in normal starts, custom missions and the range. Pilot-only recordings retain clean loads. All 12 imported aircraft and the F/A-XX concept share evenly spaced physical cannon rounds, one glowing tracer every three bullets and a 0.5-degree full gun dispersion cone ([rules](spec/damage-smoke.md#gun-dispersion-and-luminous-tracers)). Remaining catalog weapons and full combat missions remain. | Manual pp. 124-126; [weapon coverage](baselines/aircraft-roster-expansion.md) |
| Gun pipper and closing-range arc | ☑ | ☑ | Implemented for all selectable imported guns. Manual 1,000-foot visual reference, radar range/lead, bullet drop, ammo and range arc use a fitted ballistic solver. Exact retail equations remain unknown. | Manual p. 86; [gun sight](spec/gunsight-targeting.md); [checks](baselines/gunsight-targeting.md) |
| HUD target square and off-HUD direction | ☑ | ☑ | Implemented with full attitude projection and a requested edge chevron, including rearward targets. Presentation selection can survive sensor loss without granting weapon support. | Manual p. 83; [target cues](spec/gunsight-targeting.md#target-square-and-edge-chevron) |
| Missiles requiring continuous radar lock | ☑ | ☐ | Completed for supported default stores. Lost support stops measured guidance; the original target can be reacquired before guidance expiry. | Manual pp. 117-118; [current missile behavior](spec/missiles.md#what-exists-today-and-what-changes) |
| NAV and weapon selection | ☐ | ☑ | Implemented. Bracket keys and WEAPONS minus/plus cycle NAV and weapons; arming follows selection. The HUD status shows NAV, LCOS for the gun, or ARM for missiles. Boresight is silent without designation. WEAPONS lists grouped counts, selection, countermeasures and pages. | [Requested selection and layout](spec/weapon-navigation-selection.md) |
| NAV INFO destination modes | ☐ | ☑ | Partially implemented. Minus/plus select destinations; button 3 switches mission/airport mode. Airports are ordered by distance and filtered by landing eligibility. Mission route import remains unavailable. | [Requested navigation layout](spec/weapon-navigation-selection.md) |
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

## Combat AI

AI opponents and wingmen. Behavior comes from [the AI spec](spec/ai.md) and the
[experience spec](spec/ai-experience.md); the rules the spec leaves open run on
named fitted stand-ins listed in [behavior provenance](behavior-provenance.md).
Passing our tests is not a claim of demonstrated retail parity.

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| AI-flown Quick Mission wings | ☑ | ☐ | Partially implemented. Quick Mission flies all six wings as AI aircraft by default, each with its own sensors, stores, flight model and decision state; `--ai-probe-ticks N` runs the same thing headless. `--fixture-wings` retains the straight-flight setup. AI shots carry actor-owned imported records. Compatibility missile steering remains fitted; full AI seeker acquisition and pitbull remain. Mission objectives, fuller routes and broader orders remain. | [AI spec](spec/ai.md), [stages](ROADMAP.md#1e-ai) |
| Four AI experience levels per wing | ☑ | ☐ | Partially implemented for Quick Mission. Every member carries the wing's selected skill, which is what the original writes. Component tables drive tactical choice, warning delay, countermeasure odds, pursuit variation and the AI-only G adjustment; fresh warnings are queued and AI movement comes exclusively from commanded aircraft inputs. | Manual pp. 18-20; [experience](spec/ai-experience.md) |
| Enemy-skill override | ☑ | ☐ | Partially implemented. `--enemy-skill novice\|average` forces every enemy aircraft to that level, leaving friendly wings alone. There is no menu for it yet. The original's dialog promises the setting persists in the preferences file; that persistence is untraced, so ours is session-only. | [Experience channels](spec/ai-experience.md#experience-channels) |
| Dummy aircraft skill option | ☐ | ☑ | Implemented. Every Quick Mission wing offers Dummy (400 KTS): straight, level constant-speed targets, damageable without firing, evasion or wing-command maneuvers. Normal skills remain separate. | [Requested training mode](spec/dummy-aircraft.md) |
| AI target selection, pursuit and maneuvering | ☑ | ☐ | Partially implemented. Retention, ranking, geometry, tactical choices, pursuit offsets, speed bands and B44 rate formulas have component tests. Live pursuit follows moving targets, ranking receives same-wing attacker counts, and achieved motion comes exclusively from the aircraft flight model. Last-ditch shapes, random tactics and engagement pitch use fitted rules. | [B11 to B15](spec/ai.md#b12-approach-tactical-choice-and-pursuit) |
| AI weapon employment | ☑ | ☐ | Partially implemented. Cadence and finite ammunition have component tests; debit precedes a launch event and the human unlimited bypass is disabled. Live firing checks selected-store envelopes, support and terrain. Store scores use pointing error; actor-owned gun records remain guns. Full AI seeker lifecycle remains pending. | [B42, B45](spec/ai.md#b42-weapon-preparation-search-cadence-and-firing) |
| AI missile warnings and countermeasures | ☑ | ☐ | Partially implemented. Warning delays and class-specific dispenser rules have component tests. Live warnings wait until due; devices release individually at quarter-second intervals and apply class-specific decoy rolls. Visuals and post-decoy coasting are fitted; original lifetime shortening remains unknown. | [B47](spec/ai.md#b47-threat-warnings-countermeasures-and-reason-priority) |
| AI formation and wing orders | ☑ | ☐ | Partially implemented. Each wing has its own leader and idle delta-formation following, with physical controls, neighborhood breakout prediction, coordinated approach gates, stabilized capture and hidden flight traces. Render timing is shared with the player. Line abreast preserves echelon sides with alternating right/left slots. Routine changes use staged, coordinated repositioning; smoothed vertical wandering is limited to five feet of requested offset. Emergency breakout remains available. Actor-scoped commands, spacing/control settings, target approaches and cancellation now use receiver outcomes. Verified original command/reply recordings are imported and serialized independently of flight. Formation transitions and stalled closure produce restrained text reports. Sensor-limited friendly reacquisition, persistent protect-me policy, AI leader cooperation and broader mission orders remain. The approach point is fitted; formation-report audio remains unknown. | [B43, B46](spec/ai.md#b43-wing-commands-and-formation-variation) |
| AI fuel awareness and return to base | ☑ | ☐ | Partially implemented. Caution, bingo, critical and out-of-fuel states work and send an actor home. Takeoff and landing sequences remain, and the leader and singleton route home is fitted. | [B48](spec/ai.md#b48-routes-fuel-and-recovery) |
| Surface and other-family AI | ☑ | ☐ | Planned. SAM, AAA, vehicles, ships and the non-fighter aircraft families are specified only in part and are rejected rather than served fighter behavior. | [B20, B30](spec/ai.md#b20-other-aircraft-families) |

## Systems and controls

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Radar search/tracking modes and contact history | ☑ | ☐ | Completed for the current air-to-air scope. Pointer takeover covers the full black screen up to the bezel, including radar-off display. | Manual pp. 96-99; [radar](radar.md) |
| RCS exposure display | ☑ | ☑ | Completed. Uses the same aircraft exposure calculation as detection. | Manual pp. 94-95; [RCS display](spec/rcs.md) |
| Radar notching and generation-based jammer tuning | ☐ | ☑ | Partially implemented. Detection effects work; side-by-side aircraft tuning remains. | [Radar tuning](radar.md#deliberate-departures-and-known-approximations) |
| Target camera and target information | ☑ | ☑ | Partially implemented. 24 fps grayscale scene with darker scenery, improved depth precision and automatic framing along the player sight line within 1 nm of the target, type, damage, clock bearing with 10-degree Hi/Lo, skill/activity and three-second speed/range cycle work. Objective assignment and player-specific evade identity remain unavailable. | Manual p. 101; [target window](spec/target-window.md) |
| Persistent selection of search-only contacts | ☐ | ☑ | Completed. Selection and firing permission remain separate. Known friendly targets show a centered X in the HUD box. Selected targets retain the shared HUD square or edge chevron in NAV; display selection is separate from sensor support. | [Selection](radar.md#mouse-designation-and-missiles) |
| Destroyed aircraft remain visible to sensors | ☐ | ☑ | Completed while the wreck remains airborne. | [Destroyed aircraft](radar.md#destroyed-aircraft-remain-sensor-objects) |
| Centered HUD zoom and wide view | ☑ | ☑ | Completed. Zoom uses the HUD center; below 1x the cockpit and mirrors hide while HUD and instruments remain. | Manual p. 104 describes magnification; [requested presentation](baselines/cockpit-slide.md) |
| Rebindable modern controller profiles | ☐ | ☑ | Completed. Gamepads, sticks, throttles and pedals use the in-flight binding editor. | [Input](INPUT.md) |

## Maps, weather and atmosphere

| Feature | Retail manual | Opinionated addition | Status and remaining work | Details |
| --- | :---: | :---: | --- | --- |
| Live in-flight map with object category toggles | ☐ | ☑ | Implemented. Shift-M shows runways and detected air/surface contacts, unknown placeholders, zoom and pan. Right-side category toggles; Buildings off by default. No Escape-menu category filters. Surface detection and identification are fitted; ground-defense spawning remains outside this feature. | [Map](spec/flight-map.md) |
| Select and fly over the original theaters | ☑ | ☐ | Completed. All sixteen theaters are selectable. Signed mission wind headings load correctly in both Vietnam theaters. | Manual p. 198; [terrain](baselines/ukraine-viewer.md); [wind loading](baselines/mission-wind.md) |
| Airport and surrounding ground-object placement | ☑ | ☑ | Base-theater scenes implemented with original assets and fitted contact/rendering. Ukraine includes all 257 placements. Unsupported ground-defense shapes and campaign overlays remain. | [Validation](baselines/ukraine-airports.md#implementation-and-independent-review) |
| Weather and time-of-day presentation | ☑ | ☐ | Partially implemented. Six weather choices and day/night presentation work. Smooth surface lighting, geometry shadows and gradient-preserving glare are available; remaining cloud forms and special weather effects remain. | Manual p. 198; [weather coverage](formats/weather.md), [aircraft lighting](spec/aircraft-lighting.md) |
| Warm continuous surface lighting and geometric shadows | ☐ | ☑ | Completed within finite shadow-map coverage. Opaque terrain, aircraft, debris and weapon bodies share sunlight and cast/receive shadows; water receives shadows. Visible sun fraction scales shadow strength independently of glare, caster distance controls soft edges, close-view transitions are more diffuse, shared terrain normals avoid lighting seams, painted panels have view-dependent highlights, and land shadows suppress water sun glints. Low-sun land shade has reduced ambient fill without brighter ridges. Transparent effects and distant geometry beyond coverage remain outside the solid shadow pass. | [Surface lighting](spec/surface-lighting.md) |
| Physical turbulence and its disable option | ☑ | ☐ | Partially implemented. Low-altitude disturbance works; live neighboring-aircraft wake effects remain. | Manual appendix D, No Turbulence; [weather flight effects](baselines/wind-turbulence-vapor.md) |
| Animated ocean ripples and reflections | ☐ | ☑ | Completed. Original water colors and textures are retained. | [Ocean](spec/ocean.md) |
| Temperature-aware atmosphere and speed calculations | ☐ | ☑ | Partially implemented. Air-data calculations work; calibrated airspeed and pressure-based cockpit instruments remain. | [Air-data scope](FLIGHT-MODEL.md#independent-aircraft-models-and-future-gauges) |

[manual]: https://pdfcoffee.com/famanual-pdf-free.html
