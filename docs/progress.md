# Parity progress

Updated 2026-09-13. This is the actionable checklist for the [roadmap](ROADMAP.md), covering menus, original flight environments and aircraft. Checked items describe work in this Rust repository, not work completed in USNF-ATF. An unchecked item remains open even when a reference decoder or prototype exists. Keep format status in [coverage](formats/coverage.md) and acceptance evidence in [baselines](baselines/).

**Current scope:** Choose Activity now leads to a Quick Mission Creator mock and a Ukraine free-camera viewer. Original T2 heights, texture placements, briefing map and a fixed weather-palette/sky preview are implemented in Rust. The remaining menu system, full environment fidelity, aircraft and flight simulation remain open. See [theater recovery](formats/theater.md) and [viewer baseline](baselines/ukraine-viewer.md).

## Fidelity and evidence rules

- Recover the original game from user-owned retail assets and, where data alone is insufficient, executable analysis and observed native behavior. Reuse original pictures, fonts, textures, geometry, terrain data and audio wherever possible.
- **Do not port USNF-ATF's custom terrain system or use its DEM-based theaters as the original terrain.** Its terrain rendering, real-elevation pipeline, satellite imagery and authored shoreline system are outside this recovery effort. Consult its retail-format research as evidence, with its uncertainties preserved.
- USNF-ATF's hand-rolled flight engine contains recovered helpers alongside authored integration and assisted controls. Study and reproduce verified retail systems in Rust; do not inherit custom behavior merely because the reference app runs. Binary engine logic is reconstructed, not unpacked as an asset.
- Label findings as **retail data**, **recovered executable behavior**, **native observation**, or **authored approximation**. A matching reference implementation establishes a regression comparison, not automatically retail parity.
- Preserve source title, edition, archive/resource, hashes, decoder revision and any overrides. Mixed-title aircraft bundles and toolkit modifications must be explicit; they cannot silently define the classic baseline.
- Target retail geometry, presentation, systems and gameplay as closely as evidence permits. The roadmap does not require identical display resolution, every rendered pixel or legacy clock artifacts. Record tolerances and intentional differences before accepting each feature.
- Keep retail inputs and generated derivatives in ignored local storage/application data. Commit original documentation and synthetic test fixtures only.

## 0. Shared foundation and recovery workflow — M0 and ongoing

- [x] Establish pinned Rust workspace, native window/GPU/audio dependencies and macOS Apple M3 development instructions.
- [x] Configure macOS, Linux and Windows CI and retail-data guards. Local interactive acceptance currently covers macOS; configuration is not proof of an interactive run on the other platforms.
- [x] Provide independent, documented extraction through `tools/extract_assets.py` and shared Rust EALIB/DCL readers, preserving archive boundaries and provenance reports.
- [x] Extract the supplied five Fighters Anthology archives: 7,520 unique resources. See the [main-menu baseline](baselines/main-menu.md); resource counts do not establish decoded terrain or flyable-aircraft coverage.
- [ ] **F1 — Complete source inventory.** Inventory editions/discs and missing dependencies per title; identify base assets, optional media and overrides. Record extraction success separately from format interpretation.
- [ ] **F2 — Extend extraction when required.** Track ISO disc-image access, ESA containers and coded-literal DCL independently. These are input-format gaps, not prerequisites for using the already supported loose LIB installation. Add bounded readers, malformed-input checks and provenance before claiming support.
- [ ] **F3 — Build an evidence index.** Associate recovered fields/routines with exact source hashes, addresses where applicable, confidence and reproducible commands. Preserve unknown fields instead of assigning convenient meanings.
- [ ] **F4 — Establish native comparisons.** Capture original screen flows, theater views and controlled flights with title/version, input sequence and starting state. Define measurable tolerances; separate unavailable native evidence from passing Rust/reference comparisons.
- [ ] **F5 — Maintain acceptance records.** Each completed step links its Rust commit, input identity, commands, results, screenshots/audio or telemetry, host/platform and remaining gaps. Record real Linux/Windows window, sound and input checks as those hosts become available.

## 1. Menu parity — M1a, with later systems wired in M1d–M2

### MENU1 — Current Choose Activity slice

- [x] Import all five original backgrounds, button pieces, proportional PIC font strips and CHOOSEAC dialog labels/positions.
- [x] Choose a background randomly at startup, respecting its palette and menu-bar position. Native setup evidence supports this selection; a timed background slideshow and the original random sequence are not implemented.
- [x] Render enabled/disabled actions and provide mouse/keyboard interaction, authored hover/press animations and placeholder responses.
- [x] Stub `?`, `Pref` and `Multi`; implement exit and session-only music/effects toggles. Hover/focus is silent; activation/toggles play sounds.
- [x] Provide deterministic background/state snapshots and record [menu recovery](formats/menu.md) and [validation](baselines/main-menu.md).
- [ ] Recover original hover/pressed/disabled state semantics and timing. Current brightness, press displacement, focus outline, hit-area assumptions and placeholder messages are authored.
- [ ] Decode the actual MNU tree and general DLG controls, including separators, accelerators, nested menus, modal behavior and enabled-state rules. Current dropdown entries/chrome are not a recovered complete tree.
- [ ] Confirm menu music selection, transitions and cue mapping. `AIR003.11K` is a preview with unconfirmed activity-menu association; PIC fonts do not establish FNT support.

### MENU1b — Quick Mission Creator investigation shell

- [x] Reuse original FA `QUIKMIS3.PIC`, PIC fonts and button pieces; provide authored hover/press states with silent hover and click-only effects.
- [x] Connect Create Quick Mission, temporary Terrain Viewer, Cancel and `?` back/exit actions; stub Aircraft and mission fields.
- [x] Show the original Ukraine briefing map and a selector catalog from all 16 T2 names. All 16 base theaters are enabled; object/campaign dependencies remain incomplete.
- [ ] Recover actual quick-mission DLG/MNU controls, selections and native state behavior; the temporary theater/viewer layout is explicitly authorized investigation UI.
- [ ] Wire mission generation, aircraft/loadout, opponents, start conditions and launch/debrief after the dependent systems exist.

### MENU2 — Inventory and shared controls, deferred

- [ ] Enumerate screens and transitions from DLG/MNU assets, sequence references and native captures per supported title. Record each screen's art, palette, fonts, controls, audio, dependencies and return path.
- [ ] Recover generic dialog primitives: lists, scrolling, selection, text entry, sliders, tabs and confirmation/error dialogs as actually encountered. Do not assume LAY is a UI format.
- [ ] Support compiled FNT and remaining PIC/palette variants when required; preserve glyph metrics, masking, alignment and original control artwork.
- [ ] Recover settings persistence, keyboard/controller bindings, mixer behavior and reset/default rules. Validate focus, cancellation and modal input across window sizes and display scales.
- [ ] Recover XMI, instrument banks, MUS scheduling and sequence/audio references; implement internal synthesis and original transitions. Track absent samples/video separately from decoder gaps.

### MENU3 — Screen and navigation backlog, deferred

These are flow categories to inventory, not a claim that every title has identically named screens. First build faithful shells, then mark behavior complete only when the dependent system works.

| Flow | Steps remaining | Dependency / completion target |
| --- | --- | --- |
| Help / about / exit | Recover actual entries, panels, confirmations and return paths | M1a shell; help/reference content where available |
| Preferences | Graphics, sound/mixer, input controls and persistent settings | M1a shell, device/settings integration |
| Single mission | Mission list/filter, briefing/map, aircraft/loadout, launch and return | M1a shells; M2 mission execution |
| Quick mission | Aircraft, loadout, theater, start conditions, opponents, launch, debrief/retry | M1a shells; M1d loop; M1e–f combat |
| Pro mission | Recover original entry flow and required dialogs | M2 format compatibility; M3 editor tooling |
| Replay last mission | Recover availability, replay/retry semantics and navigation | Saved mission/input state; do not infer semantics from label alone |
| Reference | Categories, aircraft/object entries, pictures, text and available media | M2 encyclopedia/media support |
| New / continue campaign | Campaign and pilot selection, saves, briefing, progression, results | M2 campaigns and persistence |
| Pilot records | Selection, record details, awards/statistics and native edit/reset rules | M2 scoring and saves |
| In-flight menus | Pause, settings, briefing/map access, abort/exit confirmations and return | M1a shells, M1c–M2 runtime integration |
| Multiplayer | Recover authentic Multi entries and connection/setup dialogs | Shell inventory first; networking remains subject to roadmap M5/open decision |

- [ ] Record per-screen status: inventoried → assets decoded → shell navigable → behavior wired → native comparison accepted. Keep unsupported actions visibly identified until implemented.
- [ ] Verify every transition, cancel/back path, disabled state and missing-media case. Capture normal, focused, pressed and modal states with original audio behavior.
- [ ] **Menu acceptance:** complete the screen inventory for each supported title, compare with native captures and exercise full flows. The present main menu is partial M1a, not menu-system parity.

## 2. Original terrain and flight environments — M1b–M2

### ENV1 — Recover theater data before designing the renderer

- [x] Extend extraction to all 16 defined profiles, source aliases and shared atmosphere assets: 1,129 resources / 75 MM layouts; added retail discs inventoried. See [validation](baselines/all-theaters.md) and [extraction guide](EXTRACTION.md#all-defined-theaters-and-the-retail-discs). This does not enable the other theaters in the renderer.

- [x] Parse all 16 supplied T2 grids with bounded Rust readers; export dimensions, elevation range and resource names through the shared Ukraine extraction profile.
- [x] Verify packed header offsets, color/class/elevation triples, 8,192-foot cell spacing, 256-foot height steps and fine/coarse lookup against FA.EXE. See [addresses and corrections](formats/theater.md).
- [x] Recover UKR.MM's 697 texture placements and UKR0–28 texture naming/quarter-turn mapping; preserve raw `tdic` and object data for further work.

- [ ] Inventory all local T2 resources and their dependencies: briefing maps, tile/material data, palettes/textures, object shapes and mission/layout references. Record duplicate/variant names and verify alias resolution per title.
- [ ] Implement bounded Rust T2 parsing, preserving unknown header, cell and tile-table data. Compare decoded structures with independently inspected bytes and the reference reader.
- [ ] Resolve elevation classes, vertical scale, tile selection and any supporting geometry/tables through asset cross-references and native executable analysis. Determine how the game builds the actual surface.
- [ ] Resolve land/water classification and shoreline construction, including title/theater exceptions. The reference T2 notes explicitly leave the Baltics water interpretation unresolved.
- [ ] Calibrate axes, handedness, origin, horizontal/vertical units and mission placement against native landmarks. Verify across titles instead of assuming the reference coordinate conversion applies universally.
- [ ] Write a retail terrain specification with confirmed rules, unknowns and fixtures. **Gate:** do not substitute real-world elevation or USNF-ATF terrain where recovery is incomplete.

The reference [T2 notes](../USNF-ATF/Docs/formats/t2.md) used a misaligned cell offset and left elevation unresolved. Native executable analysis now establishes the packed layout and real height samples; our [corrected specification](formats/theater.md) supersedes that interpretation. Remaining research concerns exact adaptive geometry, classification, shorelines, variants and native comparisons.

### ENV2 — Reconstruct and render original theaters

- [x] Enable all 16 base theaters in the creator and direct CLI, including per-theater palettes, texture-array sizes, briefing maps and camera reset. Each passed a Metal smoke test; this does not close native parity acceptance.
- [x] Correct TVIET's TVI texture alias and preserve signed border placements; all-profile extraction now contains 1,171 resources.

- [x] Build a Ukraine GPU mesh directly from source heights and texture placements, with matching triangle-based ground-height queries.
- [x] Add an extensible depth-tested sim renderer and original SKY0 preview, separate from format/world data and menu composition.
- [x] Provide repeatable initial camera, GPU capture, arrow translation, Shift acceleration, altitude/look controls and return navigation; test camera boundaries/height and held-key clearing.

- [ ] Build runtime theater data from confirmed retail surface rules and dependencies; retain traceability from generated terrain back to source cells/tiles/resources.
- [ ] Render original surface geometry, material/palette selection, texture orientation/repetition, water and shoreline behavior. Recover native detail/visibility rules before choosing equivalent Rust rendering techniques.
- [ ] Recover runway/airfield, roads, buildings, vegetation and other placement rules where present; distinguish terrain-owned content from mission-owned objects.
- [ ] Make terrain height/contact queries and rendering agree on the physical surface, including boundaries, water and runways. Test seams, winding, out-of-bounds queries and coordinate conversions.
- [ ] Add bounded loading/caching, culling, precision handling and rendering detail appropriate to recovered data. These are new Rust implementation choices, not a port of the reference terrain system.
- [ ] Add recorded camera routes and landmark overlays beyond the current repeatable startup pose; the temporary viewer control has now been replaced by Free Flight, while `--viewer` remains a CLI diagnostic.
- [ ] **M1b acceptance:** fly a free camera over retail Ukraine and one other recovered theater; compare coasts, relief, airfields, landmarks and mission positions with native evidence. Record actual frame times, memory and screenshots on all three platforms.

### ENV3 — Atmosphere and environment systems

- [x] Extract all LAY modules, SKY0–8, SUN/MOON/STARS SH, CLOUD1/CLOUDS SH and their named PIC dependencies with provenance.
- [x] Parse top-level mission layer/cloud/wind/time fields without assigning absent values or unverified wind units.
- [x] Recover PL palette RVAs, 352-byte records and native palette ramp destinations; render an explicit DAY2 midday keyframe. Raw weather modules are preserved without execution.
- [ ] Decode celestial/cloud shapes and native placement/animation; they are extracted but not rendered yet.
- [ ] Port native weather keyframe selection/interpolation and fog updates; replace authored spherical sky/distance fog after recovering native presentation.

- [ ] Inventory environment settings in mission/theater assets and trace their consumers: sky, horizon, visibility/fog, time of day, weather, wind and lighting where supported.
- [ ] Reconstruct original sky/horizon/palette and visibility transitions. Label missing behavior as unknown rather than borrowing the reference's authored atmosphere.
- [ ] Recover wind/weather effects on flight, instruments and audio separately from their visual presentation; supply deterministic environment state to the simulation.
- [ ] Validate known time/weather scenarios against native captures and telemetry; verify horizon, water and ground visibility at representative altitudes.
- [ ] Extend accepted theater coverage across titles and campaign variants; validate mission navigation, start locations and object placement in each. Carrier decks, launch and recovery are later M2 integration, not proof supplied by terrain rendering alone.

## 3. Aircraft and original simulation systems — M1c–M2

Use the local [aircraft-porting guide](../USNF-ATF/Docs/aircraft-porting.md) and [worksheet](../USNF-ATF/Docs/templates/aircraft-port.md) as research starting points. Its helper exports reviewed bundles; successful export is explicitly not full native flight acceptance. Bun/Electron commands in that guide are reference-project commands, not commands for this Rust app.

The checklist below tracks full roster/native parity. The implemented F/A-18D subset and its evidence are itemized at the end of this document; partial progress does not check off full-parity rows.

### AIR1 — Intake and identity, repeated for every aircraft/variant

- [ ] Create an original-work aircraft evidence sheet under `docs/aircraft/` when its port starts. Record title, archives/resource hashes, PT/SH identity, cockpit/HUD references, audio, weapon/sensor dependencies and known gaps.
- [ ] Compare actual model variants and source overrides before selecting a baseline. Prefer the supplied FA resources; document any mixed-title research bundle without presenting it as an unchanged retail aircraft.
- [ ] Inspect `swpatch.lib` provenance before applying it. The reference guide identifies its reviewed installation as toolkit content, not an established official patch. Its F-14 rig must not be applied automatically to base FA geometry.
- [ ] Define per-aircraft capabilities: engine count, afterburner, sweep, hook, gear/flaps/brakes, vectoring, internal gun and hardpoints. Unsupported devices must not inherit F-14 defaults.

### AIR2 — Shapes, textures and articulation

- [ ] Implement required SH commands, calls, state branches, vertex/texture state and detail variants with bounded traversal. Preserve missing/unknown commands as explicit gaps.
- [ ] Recover complete neutral geometry before authoring moving surfaces. The reference guide's relative-call correction restored previously omitted flap panels; do not repeat the incomplete-wing workaround.
- [ ] Preserve face winding, original triangulation, UVs, texture-page selection, palettes, transparency and material roles. Inspect neutral silhouettes and both sides of textured surfaces.
- [ ] Recover native dimensions/units where possible; label any presentation calibration. Apply one consistent mesh/pivot/camera scale and verify span, length, height and ground clearance.
- [ ] Recover device groups and animation laws from source state/executable behavior. Where a rig must be authored, document hinges, axes, hierarchy, mixing and deviations per exact model hash.
- [ ] Inspect neutral/intermediate/full gear, flap, brake, sweep, hook and control poses. Check for omitted faces, doubled fixed/moving surfaces, texture sliding and incorrect engine/exhaust effects.

### AIR3 — Flight dynamics and deterministic engine

- [ ] Decode PT variants with units, raw values, confidence and identity guards: mass/fuel, thrust, performance envelopes, device effects and loading coefficients.
- [ ] Recover applicable native G-limit, power, drag, loading, fuel and ground-handling calculations. Validate recovered arithmetic against executable/oracle evidence for the actual title and caller assumptions.
- [ ] Implement a renderer-independent fixed-step Rust simulation with explicit state, seeded randomness and recorded input. Choose/document tick rate, numerical behavior and cross-platform determinism requirements before acceptance.
- [ ] Reconstruct controls, attitude response, lift, stall/recovery, damping, propulsion, fuel use and device effects from retail evidence. The reference's assisted controller, fitted polars and authored ground support are not accepted native behavior by default.
- [ ] Handle non-afterburning aircraft consistently in thrust fitting, input, visuals and audio; preserve raw zero afterburner thrust. Apply loading and rounding rules without copying F-14-specific metadata into other profiles.
- [ ] Integrate ground contact, taxi, brakes, rotation, landing and aircraft/terrain collision against the recovered environment. Hook animation alone does not implement carrier arrestment.
- [ ] Support keyboard, joystick and gamepad mapping, calibration and pause/reset independently of rendering; exercise identical recorded inputs at different render rates.

### AIR4 — Cockpit, instruments, audio and combat systems

- [ ] Decode cockpit/HUD assets and actual PT-selected dependencies. For example, the reference guide identifies A-4E's F4.HUD mapping; basename matching is insufficient.
- [ ] Recover view placement, masks, mirrors, gauges, units, update rules, warnings, navigation and external/chase cameras. Optional replacement cockpit images remain outside classic acceptance.
- [ ] Import actual aircraft PCM references and recover start/stop, loop, throttle/burner, actuator, stall and warning behavior. Verify sound audibly, not just successful decoding.
- [ ] Decode internal gun and loadout dependencies (PT/JT and relevant GAS/SEE/ECM data); separate raw retail fields from authored cadence, ballistics or attachment guesses.
- [ ] Recover hardpoint compatibility, station placement, ammunition, fuel/payload mass and drag, release behavior and selected-weapon presentation.
- [ ] Integrate radar, RWR/IFF, sensors, missiles/bombs/guns, hit detection and system damage against OT/JT/NT and executable evidence. Practice gun effects alone do not establish combat parity.
- [ ] Connect aircraft selection, loadout and launch/return when those deferred screens are scheduled. Validate missing/corrupt imports, switching aircraft and rejecting mismatched profiles.

### AIR5 — Acceptance for each aircraft

- [ ] Run ground clearance, taxi, takeoff, approach/landing and device tests; capture actual exterior/cockpit frames and sound alongside telemetry.
- [ ] Measure sustained cruise and final speed trend, military/afterburner operation as applicable, low/high altitude, fuel/payload variation and fuel exhaustion.
- [ ] Measure G pulls/pushovers, sustained turns, loops, stall and recovery at documented speeds/altitudes. Distinguish instantaneous, sustained and structural limits. Do not copy F-14 thresholds onto other aircraft.
- [ ] Separate native arithmetic checks, reference-engine regression comparisons and complete native-flight evidence. A zero-fuel envelope fixture is not a playable fuel-state test.
- [ ] Run deterministic headless maneuvers and repeat identical inputs across render rates/platforms; record tolerances and unresolved differences. M1d adds the roadmap's 100 identical seeded quick-fight runs.
- [ ] Accept an aircraft only with model, devices, handling, cockpit, audio and applicable systems evidence, plus an explicit remaining-gap list. Expand to the remaining roster in M2 batches.

### Aircraft coverage at this checkpoint

| Aircraft | Reference starting point | Rust status / next gate |
| --- | --- | --- |
| F-14 | Reviewed recipe; recovered native helpers; model/rig differs by source and override | Not started; choose and document actual FA variant before reusing findings |
| A-4E | Reviewed FA model recipe; reference flight integration retains authored behavior | Not started; intake, SH/PT and capability checks |
| X-31 | Reviewed recipe; vectoring/control-law fidelity remains a specific concern | Not started; intake and device/flight evidence |
| F/A-18D | Supplied FA F18.PT / F18.SH | Development free-flight slice implemented; full native parity open; see [evidence](formats/aircraft.md) |
| Remaining aircraft | Inventory per title, model and profile variant | Not started; M2 batches after shared pipeline acceptance |

## 4. Sequencing and open gates

1. Preserve the working menu baseline; schedule further screens explicitly from MENU2–3.
2. Extend ENV1 recovery beyond the confirmed T2/Ukraine subset. Complete dependency inventory, classification, native adaptive geometry and shoreline semantics.
3. Validate and extend ENV2's implemented Ukraine free-camera slice, then environment fidelity in ENV3. Aircraft format investigation can proceed independently; flight acceptance needs a validated ground/environment contract.
4. Develop AIR1–5 incrementally for the four M1 aircraft. Use developer harness entry points while deferred menu flows are unavailable.
5. Wire the quick-fight flow and combat/AI systems in roadmap order, then missions, campaigns, remaining screens/theaters/aircraft in M2.

Open gates include native adaptive terrain/tile coverage/water rules, per-title source completeness, native animation/music mapping, applicable flight-oracle coverage, the roadmap AI VM-versus-observation decision and full cross-platform runtime evidence. Do not mark these resolved by copying reference behavior.

## Reference index

Local links below require the ignored `USNF-ATF/` checkout; it is not a build/runtime dependency. Research checkpoint: `2d818054ff51db9f3353d0548dbd0e469b275a1a`. Recheck revisions when using its evolving findings.

| Area | Starting sources | Boundary |
| --- | --- | --- |
| Menu | [Rust menu findings](formats/menu.md), [baseline](baselines/main-menu.md), [reference MNU](../USNF-ATF/Docs/formats/mnu.md) | Retail art/native behavior prevail over custom reference controls |
| Terrain | [T2](../USNF-ATF/Docs/formats/t2.md), [mission formats](../USNF-ATF/Docs/formats/mission.md) | Partial recovery; custom terrain implementation is excluded |
| Aircraft conversion | [Port guide](../USNF-ATF/Docs/aircraft-porting.md), [SH](../USNF-ATF/Docs/formats/sh.md), [PT](../USNF-ATF/Docs/formats/pt.md) | Reviewed conversion is not complete aircraft parity |
| Flight systems | [Flight dynamics](../USNF-ATF/Docs/formats/flight-dynamics.md), [native flight](../USNF-ATF/Docs/formats/native-flight-code.md), [native performance](../USNF-ATF/Docs/formats/native-performance.md) | Preserve verified title/helper scope and authored-integration gaps |
| Cockpit/combat/audio | [HUD](../USNF-ATF/Docs/formats/hud.md), [native guns](../USNF-ATF/Docs/formats/native-guns.md), [audio](../USNF-ATF/Docs/formats/audio.md), [music](../USNF-ATF/Docs/formats/music.md) | Recover dependency mappings and runtime behavior separately |

When updating this tracker, check only the completed substep, link its evidence and update format coverage if applicable. A milestone remains open until its acceptance gate is met; no percentage estimate substitutes for evidence.

## F/A-18D free flight and instrument-window slice (2026-09-13)

- [x] Identify FA F18.PT as F/A-18D, distinct from F18C; document source facts and unsupported variants.
- [x] Shared aircraft dependency closure through `tools/extract_assets.py`; optional full weapon library; preserve original boundaries/hashes.
- [x] Bounded BRF PT/JT/SEE/ECM fields, G envelope points, hardpoint/sound references and raw unknowns.
- [x] Static nearest-detail SH geometry, source 256×644 skin and observed gear/brake/hook/burner endpoint poses.
- [x] Native cockpit artwork and bounded compiled bitmap-font recovery.
- [x] Fixed-tick Rust free-flight adapter, clean fit and headless input probes; native force/control parity remains unchecked.
- [x] Creator Free Flight action, theater selection, dotted deferred selectors and direct launch without loadout.
- [x] Small raster instrument windows; live fuel/throttle/navigation/envelope, radar power/range, imported equipment inventory and actual GPU front/other views.
- [ ] Exact native instrument layout, font dispatch, draw rounding, window controls and all page modes.
- [ ] Temperature/oil/hydraulic/system-health simulation and original gauge/failure mapping (`---` until recovered).
- [ ] Native radar/RWR/seeker contacts, RCS outline, target acquisition/tracking, waypoint planning and camera labels.
- [ ] Weapon execution, loadout/compatibility, payload/fuel partition, expendables and damage.
- [ ] Native flight helpers, continuous SH articulation, control surfaces, scale acceptance, ground support, takeoff/landing, mirror rendering and full cockpit/HUD behavior.
- [ ] Native side-by-side flight/instrument and audible acceptance; Linux/Windows runtime checks.

See [format evidence and per-page coverage](formats/aircraft.md) and [validation baseline](baselines/f18-free-flight.md). Imported data and a runnable flight do not close the full M1c/1f parity gates.


### Cockpit, HUD and desktop controls follow-up

- [x] Replace rejected half-height cockpit world with a full-canvas world and uniformly scaled source frame; retain independent instrument overlays.
- [x] Import HUD11 and all HUD mode fonts through the shared aircraft extraction profile.
- [x] Read the FA FMENUD menu hierarchy/accelerators as bounded data, including nested items and Shift-0 RCS.
- [x] Add keyboard/mouse flight menu, pause/resume/end/restart, focus-loss pause, help and explicit unavailable-command feedback.
- [x] Correct F1/F2/F3/F10, cockpit toggle, pause, menu/desktop exit and window shortcuts; preserve modifier separation. Document provisional flight bindings.
- [x] Draw source-font HUD with heading, TAS/MSL/AGL, vertical speed, G, throttle, device state, projected pitch ladder and flight-path marker.
- [ ] Recover the complete FA non-menu keyboard dispatch, controller/joystick bindings, native camera transition and zoom/pan behavior.
- [ ] Implement remaining menu handlers (preferences mixer, cheats, multiplayer, map/position), RCS and combat/navigation/radio commands.
- [ ] Recover native F18 HUD caller/symbol layout, mirrors, ILS/weapon/corner-speed cues and full cockpit panel composition; compare against retail flight.
- [ ] Validate Windows/Linux input, rendering and audio; perform manual native-game parity acceptance.

Current behavior and binding provenance: [FLIGHT-CONTROLS.md](FLIGHT-CONTROLS.md). Follow-up evidence: [cockpit/control baseline](baselines/cockpit-controls.md).


### Instrument layout follow-up

- [x] Large mode: four 160×156 corner windows with margins, using the supplied F-14 catapult capture as the placement reference.
- [x] Small mode: six 96×94 bottom windows, grouped three left and three right, with inter-window spacing and a center gutter.
- [x] Wire original Pref → Large windows? to switch layouts and preserve each layout's session selections; add `--instrument-layout large|small` for captures.
- [x] Share layout rectangles between rendering and inverse-scaled button hit testing; cancel pending clicks when pages/layouts change.
- [x] Test capacity/selection restoration, non-overlap/margins and matching button release at both sizes; validate both layouts on Metal.

Exact native placement and independent high-resolution instrument typography remain open. Small mode currently scales the existing source-font raster. See [layout validation](baselines/instrument-layouts.md).


### Responsive flight UI follow-up

- [x] Separate the flight overlay from the fixed 4:3 menu canvas; reveal source cockpit side art on wider screens and preserve uniform scaling.
- [x] Anchor corner/bottom groups to the actual display bounds, with matching pointer conversion and responsive margins.
- [x] Draw small windows directly from native instrument rasters, eliminating the intermediate downscale; cache unchanged imagery.
- [x] Shrink HUD presentation 15%, preserve attitude projection, remove opaque TAS/MSL readout backings and avoid overlapping tape labels.
- [x] Add repeatable `--window-size WIDTHxHEIGHT` captures and aspect/alpha/pointer regression coverage.

See [responsive-flight validation](baselines/responsive-flight-ui.md). Native HUD symbol mapping, mirror rendering, dynamic native instrument typography and cross-platform acceptance remain open.

## 2026-09-13: In-game performance follow-up

- [x] Measure desktop frame intervals and simulation/camera, UI, presentation wall times with an opt-in bounded diagnostic.
- [x] Remove the extra 16 ms post-render wait; use display-paced simulation presentation.
- [x] Optimize app dev-build pixel loops while retaining debugging support.
- [x] Preserve cockpit cache across view changes and prepare aircraft GPU resources before the first external view.
- [x] Interpolate render-only poses between deterministic 120 Hz ticks, including wrapped headings/bank.
- [x] Make live camera instrument readbacks asynchronous and retain display/preview depth targets.
- [x] Confirm exterior aircraft renders in chase/oblique views; explain F2/F3 native look-back/up versus F10 exterior bindings.
- [x] Record local Metal comparisons and regression evidence in [flight-performance baseline](baselines/flight-performance.md).
- [ ] Sustained thermal/battery and high-refresh-display profiling; Windows/Linux runtime measurements.
- [ ] Full GPU UI/instrument composition, terrain LOD/streaming, native flight-response acceptance and input-latency measurement.

## 2026-09-13: Look-around controls

- [x] Support Shift+arrows alongside existing Ctrl+arrows; prevent look-arrow repeats from becoming pitch/bank after modifier release.
- [x] Limit cockpit elevation to forward/upward; Down returns to the forward eye line.
- [x] Implement constant-radius, aircraft-centered exterior orbit through horizontal/vertical revolutions.
- [x] Add Shift-/ recenter and retain F1 forward/reset; document evidence and add repeatable look-angle captures.
- [ ] Recover FA-specific non-menu pan bindings and map side/rear/up cockpit artwork; full 3D cockpit remains deferred.

Evidence and validation: [look-around baseline](baselines/look-around.md).

## 2026-09-13: Flight response, vertical sky and retained cockpit

- [x] Confirm the ±1.5-radian flight clamp and nose-derived velocity existed before the performance pass.
- [x] Remove the attitude clamp; integrate/interpolate body bases through vertical and inverted flight.
- [x] Separate world velocity from attitude, add finite pitch/roll response and project actual velocity on the HUD.
- [x] Complete a headless loop with the imported F/A-18 profile and retain fixed-rate determinism checks.
- [x] Replace sky pole-pinching with a finite hemisphere texture projection.
- [x] Retain the cockpit frame during head-look; rotate look in aircraft coordinates.
- [ ] Native force/control law, stall/spin recovery, full 3D cockpit geometry and native weather projection remain open.

See [flight-response and sky evidence](baselines/flight-response-sky.md). That fixed-screen cockpit placeholder is superseded by the directional projection below; rear/up geometry is still not recovered.

## 2026-09-13: Directional forward cockpit and HUD

- [x] Inspect the wide F18 frame and side/center mirror masks; distinguish available artwork from missing interior geometry.
- [x] Project the full forward frame and HUD together in aircraft coordinates; retain centered layout and screen-anchored instruments.
- [x] Remove abrupt off-axis HUD hiding and repeated front-frame rear/up placeholders.
- [x] Check centered, small-turn, side, up, rear and tall-window captures on Metal; verify live camera panels and frame timing.
- [ ] Recover full side/rear/overhead geometry, native view mapping and working mirrors; compare against retail flight.

See [directional cockpit evidence](baselines/directional-cockpit.md). This completes the authored directional projection of available forward art, not native 360-degree cockpit parity.
