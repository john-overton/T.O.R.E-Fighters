# Fighters Anthology Rebuild: Roadmap

T.O.R.E-Fighters in the Repo - Tasteful Opinionated Reverse Engineered

Development baseline: see [DEVELOPMENT.md](DEVELOPMENT.md), [ARCHITECTURE.md](ARCHITECTURE.md), and [recorded validation](baselines/environment.md). The first M1a [main-menu slice](baselines/main-menu.md) now imports original menu assets and runs natively. M0 research and the remaining M1a screens/audio work remain in progress.

Track concrete steps, substeps and acceptance gates in [progress.md](progress.md). Further menu screens are deferred until explicitly scheduled. Original terrain and environment systems will be recovered from retail assets and native behavior; USNF-ATF's custom terrain system and DEM-based theaters are not being ported.

This is the sequencing document for the ground-up rebuild in Rust.  The existing TypeScript repo /USNF-ATF is the guide, not the gospel: its format docs, decoders, recovered geometry, audio recovery, and baselines are the reference material.  Its engine is not being ported.

Current user priority (2026-09-15): recover **native** F18/Rafale departure/tumble
and complete control/force/movement coupling, with source-derived expectations
and scoped retail comparison. Existing [adapter response work](baselines/flight-response.md)
does not complete native flight parity. Follow [behavior provenance](behavior-provenance.md).
Then resume maneuver audio/rumble and final acceptance in the
[flight-response plan](flight-response-plan.md), add F-14, A-4E and X-31 through
the [aircraft import gates](aircraft-import.md), and resume remaining weather.
This does not schedule broader AI or menus.

The preceding execution order (2026-09-14) remains a broader gate: finish manual weapons, sensors and damage
acceptance for F/A-18D and Rafale C before AI work. The only AI authorized for the
later weapon-testing phase is a basic fly-forward target. The broader future AI
milestone below is not authorization to implement combat AI now. See
[current systems evidence and remaining gates](baselines/weapons-systems.md).
The manual range covers both aircraft’s ten PT-default weapon slots, partial
ECM/player-damage integration and controller feedback. This does not close the
M1d/M1f native acceptance gates or authorize additional aircraft/loadouts.

## Principles

1. **Faithful first, opinionated second.**  Milestone 1 and 2 reproduce the retail game.  Every expansion, remaster, and quality-of-life change lands as a layer on top that can be switched off, so a "classic" mode always exists and always matches retail.
2. **Bring your own copy.**  The repo ships no retail bytes.  The importer reads the user's own Fighters Anthology media at runtime and writes to app data.  A signature scan for EALIB, PIC, and other retail markers stays a release gate.
3. **Hand-rolled where it counts.**  External dependencies are kept to a minimum.  Formats, synth, terrain, and sim are ours.
4. **Importer grows with the game.**  There is no "import everything" phase.  Each step decodes exactly the formats the next playable piece needs.  Breadth is tracked in a coverage table, not a milestone.
5. **Cross-platform from day one.**  Linux, Windows, and macOS build and run at every milestone.  No platform is "deferred" this time.
6. **Baselines are recorded, not remembered.**  Every milestone writes its measurements and acceptance evidence to `docs/baselines/`.
7. **Deterministic and headless from the start.**  Massive battles and live campaigns are the reason for the rewrite.  The sim runs without a renderer and produces identical results from identical inputs from M1 onward.  This is a constraint, not a feature.

## What "1:1" means

In scope for parity:
- Aircraft flight envelopes from retail PT data
- Weapon, sensor, and object stats from retail OT, JT, and NT data
- AI behavior and pursuit tactics
- Mission logic, triggers, scoring, and campaign progression
- Menus, screen geometry, art, fonts, and audio
- Original theater terrain from retail T2 data

Out of scope for parity:
- Pixel-accurate rendering
- Original resolution, frame timing, or integer clock artifacts
- Bugs that are not load-bearing for gameplay

Open decision (see bottom): whether the retail AI VM is reimplemented from the recovered bytecode or the behaviors are recreated from observation.  This is decided in M0 and shapes M1e.

---

## Milestone 0: Spec and salvage

**Goal:** know what is being rebuilt before writing engine code.

Work:
- Inventory the TS repo and mark every artifact as *spec* (format docs, byte layouts, recovered DLG geometry, MUS scripts, PT field maps, baselines) or *implementation* (engine code, React shell, Three.js render).  Spec carries forward.  Implementation is reference only.
- Inventory the full Fighters Anthology disc layout: USNF '97, ATF Gold, NATO Fighters, Marine Fighters, and the Pro Mission Creator.  Produce a format-by-title census.
- Write the 1:1 definition above into `docs/` and get it settled.
- Decide the AI VM question.
- Set up the Rust workspace, three-platform build, and the retail signature scan.

Deliverable: an app on all three platforms that opens a window, prints its renderer, and passes the scan.

Exit: census committed, 1:1 definition committed, AI VM decision recorded.

---

## Milestone 1: Faithful quick fight

This is the first playable milestone.  It reads: original menus, original terrain, four aircraft with basic systems, guns and missiles quick fight against real AI.

### 1a. Shell and menus

Work:
- Importer: ESA, EALIB, DCL, PAL, PIC, FNT, DLG, MNU, LAY.
- Main menu, quick fight setup, aircraft selection, loadout screen at recovered geometry with retail art, fonts, and title music.
- Pause bar, settings, volume mixer, briefing and debrief screens as shells.
- Original recorded PCM music with bounded MUS scheduling. User decision 2026-09-14: use FA recordings; MIDI conversion/synthesis is excluded from this slice.

Deliverable: every retail menu screen navigable with the correct art and audio.

Exit: side-by-side comparison against retail screenshots recorded in the baseline.  Title and menu music plays from recorded game data with no synth.

### 1b. Original terrain

Work:
- Importer: T2 and its dependents.  This is the first real reverse-engineering blocker of the rebuild.
- Terrain renderer for retail theaters as shipped.  No real-elevation pipeline in this milestone.
- Sky, horizon, and time of day at retail fidelity.

Deliverable: free camera over Ukraine and one other retail theater.

Exit: theater layout matches retail mission geography.  Frame time recorded per platform.

### 1c. Free flight

Aircraft: F-14, A-4E, X-31, and F/A-18.  The F/A-18 is included so radar and air-to-ground systems are exercised before the AI step needs ground targets.

Work:
- Importer: PT, SH, and the cockpit and HUD assets for the four aircraft.
- Fixed-rate sim loop, decoupled from render, headless-capable.
- Flight model from PT.  Engine, gear, flaps, hook, brakes, throttle, afterburner.
- Cockpit view, HUD, external and chase cameras, control surface animation.
- Navigation display and waypoints.
- Keyboard, gamepad, and joystick input.
- Engine, actuator, stall, and environment audio from retail samples.
- Maneuver harness: level flight, sustained turn, loop, stall, runnable headless in one command.

Deliverable: take off, fly, and land any of the four aircraft on any imported theater.

Exit: harness green on all three platforms with numbers recorded.  Someone who has played the original says it feels right.

### 1d. Quick fight loop

Work:
- Quick fight setup wired end to end: aircraft, loadout, altitude, range, encounter orientation, airborne and runway starts.
- Bandits fly fixed profiles.  No AI yet.
- Visual target acquisition, target cycling, guns, hit detection, airframe damage, destruction, debris, explosions.
- Combat audio and situation music from recovered MUS scripts.
- Debrief with results.

Deliverable: a complete quick fight from menu to debrief against scripted opponents.

Exit: the loop can be run a hundred times headless with a fixed seed and produce identical results.  This is the harness for 1e and 1f.

### 1e. AI

Work:
- Importer: OT, JT, and NT.
- Enemy aircraft AI at retail behavior (per the M0 decision).
- Wingmen and wingman commands.
- AI liveness probe: every bot moves, engages, and fires.

Deliverable: a dogfight where every aircraft is thinking.

Exit: liveness probe green.  Behavior compared against retail recordings and recorded in the baseline.

### 1f. Sensors and weapons

Work:
- Radar modes, RWR, IFF, and the retail sensor model.
- Missiles, bombs, and gun stats from retail data.  Stores affect weight and flight.
- Per-system damage.
- SAM sites and ships as targets and threats.
- Loadout compatibility mask decoded so stations offer real options.

Deliverable: full quick fight with retail weapons against air and ground threats.

Exit: **Milestone 1 tagged.**  A stranger can install it on any of the three platforms, import their own disc, and fly a quick fight.

---

## Milestone 2: Missions and campaigns

This is the base game.  Tagged as 1.0.

Work:
- Importer: M, MT, and campaign structures across all Anthology titles, plus Pro Mission Creator files.
- Mission loader, triggers, objectives, scoring, and events.
- Briefing, map, and debrief at retail fidelity.
- Campaign progression, pilot record, saves.
- Carrier operations: launch, recovery, deck.
- Remaining aircraft imported and validated in batches.  Coverage table shows what is flyable per title.
- Encyclopedia and video playback where media is complete.

Deliverable: every retail campaign playable start to finish.

Exit: a full campaign completed per title, evidence recorded.  Importer coverage table shows no red cells for gameplay-critical formats.

---

## Milestone 3: Tools

Work:
- Aircraft and asset tools: import, inspect, validate, and export flight profiles, shapes, and textures.
- Mission editor as the successor to Pro Mission Creator, reading and writing its format.
- Audio composer plugin: users choose instrument banks and sound sets per category, audition, and save as a profile.
- Mod loader with manifests, load order, and a "classic" profile that disables everything.

Deliverable: a user can build and share a mission and a sound profile without touching code.

---

## Milestone 4: Remaster layer

Everything here ships as toggleable layers over classic.

Work:
- Menus: modern layout option while keeping retail geometry available.
- Rebaked maps and atmosphere: real-elevation terrain pipeline returns here as an optional theater source.
- Cockpits and instruments: higher fidelity while keeping retail cockpit art selectable.
- Systems expansion: HARM and other rudimentary systems brought up to plausible depth, gameplay stats otherwise unchanged.
- Lighting, shadows, particles, vegetation, and basic cities, all at a light flight-sim feel.

Exit: classic mode still matches the M2 baseline with every layer off.

---

## Milestone 5: Live campaigns and multiplayer

Work:
- Online lobby and quick fight multiplayer.
- Authoritative server for a persistent combined-arms dynamic campaign with daily role selection: SAR, resupply, CAS, CAP, interdiction, strike, and escort.
- Ground-to-ground warfare as a campaign layer.
- Massive battle scaling, measured and recorded.

Entry condition: the sim has been deterministic and headless since M1.  If that is not true, this milestone becomes a second rewrite.

---

## Milestone 6: Custom maps and community theaters

Work:
- Terrain pipeline productized so users can build theaters from public elevation sources.
- Theater manifests, sharing, and validation.
- Asset expansion: community aircraft and objects through the M3 tools.

---

## Importer coverage table

Maintained in `docs/formats/coverage.md` and updated every milestone.  Rows are formats.  Columns are titles.  Cells are: not started, partial, decoded, validated in game.

Formats: ESA, LIB, DCL, PAL, PIC, FNT, DLG, MNU, LAY, XMI, MUS, 5K, 11K, T2, PT, SH, OT, JT, NT, HUD, M, MT, CB8, VDO, FBC, INF, campaign and save structures, Pro Mission Creator.

---

## Funding

- Donations only, for development tools and hosting.  Hard cap of $600 per month.
- Do not open until Milestone 1c is flyable by a stranger.
- Publish a ledger so the cap reads as discipline.
- GitHub Sponsors first, Patreon if the audience asks for it.  Money stays attached to the engine, never to the EA property.

## Open decisions

| Decision | Where it is made |
| --- | --- |
| Retail AI VM reimplemented or behaviors recreated | M0 |
| Working title | Whenever, before M1 tag |
| TS repo stays runnable as reference or is archived at M0 | M0 |
| Multiplayer in 1.0 or after | Before M2 tag |
| Fourth aircraft is F/A-18 or something else | M1c |
| Save format and mod manifest schema | M3 |
