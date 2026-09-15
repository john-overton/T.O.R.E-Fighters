# Parity progress

Updated 2026-09-14. This is the actionable checklist for the [roadmap](ROADMAP.md), covering menus, original flight environments and aircraft. Checked items describe work in this Rust repository, not work completed in USNF-ATF. An unchecked item remains open even when a reference decoder or prototype exists. Keep format status in [coverage](formats/coverage.md) and acceptance evidence in [baselines](baselines/).

**Current scope:** Original Choose Activity and Quick Mission briefing lead to all-theater previews and F/A-18D/Rafale C free flight. The explicit manual range now connects the ten PT-default weapon slots, sensors, damage, stores and combat-service replay. Full native environment/flight/combat parity and the remaining menu screens stay open. AI is deferred until manual acceptance. See [current systems evidence](baselines/weapons-systems.md), [earlier manual weapons evidence](baselines/manual-weapons.md) and the dated checklists below.

## Manual systems continuation — 2026-09-14

- [x] Translate reviewed player HP/amount, weighted subsystem selection/eligibility,
  and ECM hit-probability contracts; preserve explicit adapter/native boundaries.
- [x] Connect manual incoming weapons, player damage/destruction, automatic visual,
  radar, ECM and weapon-hardpoint faults, typed source configuration and tape v2.
- [x] Connect controller fire and 14 isolated modifier bindings, controls editor/help,
  bounded confirmed-event haptics and per-device feedback error handling.
- [x] Validate 220 Rust tests, 14 Python tests, 50 outgoing and 20 incoming source
  cases, two automatic-damage sequences, ten serialized tapes and 26 flight scenarios.
- [x] Reconcile current controls, architecture, roadmap and format summaries with
  the systems baseline; retain earlier acceptance results as dated history.
- [ ] Complete native decoy/sensor/guidance contracts, unknown subsystem effects,
  native RNG/difficulty/death timing, remaining loadouts/effects and original-game
  differential acceptance. Current implementation is not full native parity.
- [ ] Physical controller/haptic acceptance and Windows/macOS runtime checks.
  AI remains deferred.

[Current evidence, captures, limitations](baselines/weapons-systems.md).


## Two-aircraft live-fire pass — 2026-09-14

- [x] Resolve exact ported identities from registry, models and assets: F/A-18D
  (F18.PT) and Rafale C (RAFALE.PT), with their own PT weapon slots.
- [x] Preserve/commit the preceding exporter/components work; implement a separate
  connected development combat adapter using source values and documented approximations.
- [x] Wire held trigger/release, source ammo debit, gun/missile spawning, partial
  sensor acquisition/guidance, swept contacts, source HP/damage and destruction.
- [x] Add explicit PT-default range targets, payload release, live ammo/target/radar
  readouts, original target/missile geometry, explosion-sheet art and PCM events.
- [x] Keep ordinary free flight externally clean; cancel firing across pause/menu,
  focus, resize, selection/restart and release. Do not add unported aircraft.
- [x] Pass both imported end-to-end suites (all 10 weapon slots), 198 Rust tests,
  14 Python tests, full lint/build/asset checks and Linux GPU/capture validation.
- [x] Fix expanded-cache reload bounds; record short active CPU frame-time evidence.
- [ ] Recover full native guidance, collision, damage, effects and scheduler contracts;
  finish store textures/racks, source-specific drag and countermeasures. AI is deferred.
- [ ] Obtain matched original-game evidence before claiming W3–W5 or 1:1 parity.

[Capabilities, exact loadouts, captures, approximations and validation](baselines/live-fire.md).
Remaining menu screens are still deferred.

## Aircraft weapons research and initial implementation — 2026-09-14

- [x] Audit FA archives, native EXE/SMS and the shared exporter against reference
  research; distinguish authored/reference behavior from FA evidence.
- [x] Extract all 135 JT definitions and 170 dependent resources; audit literal
  JT references across 145 PTs and reproduce both reviewed aircraft exports.
- [x] Confirm missing shared smoke/fire/crater/debris/chaff/flare graphics roots;
  inspect FA launch-speed arithmetic and ammunition-field offsets.
- [x] Plan ordnance movement, sensor/guidance/ECM coupling, loadouts, graphics,
  damage and vanilla acceptance: [weapons plan](formats/weapons.md).
- [x] Commit and push the plan (`457c85b`), then implement shared combat roots,
  repeated aircraft selection, dependency/provider reports and cache invalidation.
- [x] Parse all 135 JT, 51 SEE, 30 ECM and 4 GAS configurations; preserve rear
  sensor values and unresolved compiled PTS icon references explicitly.
- [x] Export both aircraft and full equipment twice: 561 resources, zero errors,
  all unchanged on repeat. Add hash-gated static weapon research (19 regions).
- [x] Implement diagnostic launch/motor/fall/speed/lifetime/trigger/ammunition,
  loading-mask/capacity and partial sensor arithmetic in renderer-independent Rust.
- [ ] Complete native weapon/compatibility/effect-table contracts, generated
  dependency mappings and complete lifecycle/update producers.
- [ ] Implement deterministic gun/rocket/bomb/missile lifecycles, sensor logic,
  original effects and matched retail acceptance before claiming 1:1 performance.

Evidence and limitations: [initial research](baselines/weapons-research.md) and
[combat implementation](baselines/combat-components.md).
Further menu screens remain deferred; clean external free flight is preserved.

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
- [x] Configure macOS, Linux and Windows CI and retail-data guards. Local interactive acceptance covers macOS; Linux now has real Wayland/Vulkan startup and shutdown evidence for the menu, creator, viewer and flight, but manual sound/input acceptance and Windows runtime checks remain open. See [Linux setup](baselines/linux-setup.md).
- [x] Set up the Linux development host, copy and checksum-verify user-owned media, import the runtime cache, and fix renderer/window cleanup ordering before the event loop releases its display connection. See [acceptance evidence](baselines/linux-setup.md).
- [x] Correct Linux desktop GPU selection: prefer the high-performance compatible adapter, and verify a visible menu on the RTX 4070. The initial AMD frame-submission smoke tests did not detect the blank on-screen window; see [follow-up evidence](baselines/linux-setup.md#visible-window-follow-up).
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
- [ ] Complete audible acceptance of menu music transitions and remaining cue mappings. Native main/briefing playlists now replace the AIR003 preview; context resets/gains remain authored. See [audio evidence](baselines/audio.md).

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
- [ ] Complete music host transitions and sequence/audio references. PCM/MUS playback supersedes MIDI/synthesis per the user decision on 2026-09-14; missing samples/video remain distinct from decoder gaps.

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

## 2026-09-13: F/A-18 exterior animation follow-up

- [x] Audit source device deltas, flap panels, tail surfaces, fin faces and exhaust artwork.
- [x] Replace halfway endpoint switching with continuous gear/door, brake and hook motion using fitted hinges.
- [x] Animate source flap panels, stabilators and fitted trailing-rudder partitions; retain UVs and rotate visibility normals.
- [x] Add exhaust transition, cold nozzle presentation and shared afterburner activation checks.
- [x] Add repeatable pose captures and regression tests for reversal, interpolation, control release and source endpoint preservation.
- [ ] Recover native schedules and remaining leading-edge/outboard surfaces, nozzle mechanics, ground devices, canopy and damage/store animation.

See [animation evidence and limitations](baselines/f18-animations.md). This is original geometry with fitted motion, not complete native animation parity.

## 2026-09-13: Banked pull / AoA correction

- [x] Reproduce combined bank-and-pull with mirrored retail-profile headless probes.
- [x] Verify HUD velocity projection against aircraft axes; preserve actual velocity rather than adding display offsets.
- [x] Correct missing body-yaw transport and replace zero-AoA alignment with a documented load/speed-dependent fit.
- [x] Retain lateral transient lag, deterministic ticks and full loops; add probe telemetry and rendered capture support.
- [ ] Recover native gpullAOA/lowAOA consumers, force and rotational response, wind-relative air data, and stall/spin behavior.

See [measurements, source fields and limitations](baselines/banked-pull-aoa.md).

## Native flight model decoding pass

- [x] Inventory FA.EXE/FA.SMS statically with hashes, bounded metadata and symbol spans.
- [x] Add repeatable `tools/extract_assets.py --native-flight` research entry point.
- [x] Derive direct PT field references from the shared packed schema for the reviewed build.
- [x] Trace native movement/display separation and load/low-speed AoA consumers.
- [x] Translate pure control, AoA, envelope, power and fuel helper arithmetic into Rust.
- [x] Add synthetic regression checks and imported-Hornet `--native-flight-report` probes.
- [ ] Finish native instance layout, timer conversion and movement/force integrator.
- [ ] Decode load/damage/store-adjusted limits and thrust/drag consumers end to end.
- [ ] Port stall/spin, turbulence, ground/contact and device schedules.
- [ ] Integrate a complete native model; replace fitted adapter only after trajectory acceptance.
- [ ] Compare original-game level flight, banked pulls, loops, stalls/recovery and device transients.

See [addresses, lessons and coverage](formats/native-flight.md) and
[validation record](baselines/native-flight.md). Pure-helper tests are not proof
of full native flight parity.

### Native flight follow-up: reusable components

- [x] Extract explicitly reviewed unnamed departure/contact/integrator regions, hashes, direct edges and partial instance offsets through the universal script.
- [x] Map reviewed PT fields once into lightweight departure/drag/landing/velocity profiles; require an explicit loaded forward-speed maximum.
- [x] Translate warning/stall timer transitions, severity and control/lift attenuation; spin entry, directional state, motion targets, recovery timer and lock.
- [x] Translate drag assembly including native device flags and wheel drag, ordered scalar velocity updates and the movement angle stage through vertical crossings.
- [x] Translate landing-limit classification and ground pitch settling independently of terrain/carrier callbacks.
- [x] Add synthetic boundary, mirrored-spin, wide-arithmetic, drag-order and profile validation tests; extend imported Hornet diagnostics.
- [ ] Finish native stall tumble/pitch fall, initial envelope/difficulty predicates and complete dispatch/events.
- [ ] Recover loaded cp limits, complete lift/gravity/thrust assembly, native rotations and wind/position.
- [ ] Decode touchdown/contact state, terrain/carrier/catapult/hook callbacks and damage decisions.
- [ ] Audit timer scheduling and RNG ownership; implement deterministic whole-tick probes before enabling native dynamics in free flight.
- [ ] Compare original-game trajectories and approve any explicitly fitted substitutes per aircraft.

Evidence and component boundaries: [format research](formats/native-flight.md#second-pass-departure-ground-and-integration-components), [baseline](baselines/native-flight.md#component-follow-up).

### Native flight: table, force and movement follow-up

- [x] Extract the native sine/cosine interpolation table with reviewed-build gating and hashes.
- [x] Translate angle conversions, body-rate transform and Rotate2 with native integer ordering.
- [x] Translate lift/gravity/vector-thrust assembly and loading arithmetic.
- [x] Translate world-position/wind stage; identify world vertical speed feeding landing checks.
- [x] Add imported-table headless probes and synthetic rotation/force/loading/position checks.
- [ ] Complete world matrix and cockpit-angle composition, loaded control/equipment state, contact and clock/RNG contracts before enabling native dynamics.

[Research](formats/native-flight.md#third-pass-extracted-trigonometry-forces-and-loading) and [validation](baselines/native-flight.md#trigonometry-and-force-follow-up).

### Native flight fourth research pass

- [x] Translate world matrix and cockpit basis/angle composition with extracted sine and arctangent tables.
- [x] Translate contact retention, surface classification, touchdown settling and landing latch with explicit query inputs.
- [x] Translate resolved equipment mass and loaded control-limit arithmetic.
- [x] Isolate seeded shuffled RNG, native counter/frame arithmetic and an authored fractional 120 Hz time bridge.
- [x] Add repeatable table extraction and a headless imported-table composition probe.
- [ ] Decode terrain/carrier query producers and touchdown event effects; complete contact-system parity remains open.
- [ ] Resolve remaining loaded field semantics/producers and equipment pointer-to-profile mapping.
- [ ] Establish original scheduling, RNG seed/consumption order and full-tick trajectories before enabling a native flight adapter.

Details and limitations: [native format research](formats/native-flight.md).

### Native flight fifth research pass

- [x] Decode landing surface lookup as preferred/fallback nearest eligible object selection, including reverse-order ties and approximate horizontal distance.
- [x] Translate ground query-mask construction; identify cached-height and vertical collision-query branches.
- [x] Trace touchdown event gate/state helper without claiming carrier dynamics parity.
- [x] Translate signed-word RNG reseeding, unconditional percentage draws, and unsigned object-due comparisons.
- [x] Add incoming entry references and nine reviewed routines to repeatable extraction (52 total).
- [ ] Decode collision dispatcher geometry/cache production, remaining seed sources and queue rescheduling, and event consumers.
- [ ] Connect remaining loaded-state producers and verify whole-tick trajectories before enabling native flight.

See [fifth-pass findings](formats/native-flight.md) and [validation](baselines/native-flight.md).


## Rafale C and original-style Quick Mission follow-up — 2026-09-14

- [x] Add the reviewed RAFALE.PT identity and shared CLI/app dependency profile,
  preserving source-specific shape, cockpit, equipment, gun and audio metadata.
- [x] Fly Rafale C with its own original exterior/cockpit and PT inputs through the
  existing 120 Hz adapter; refresh render/instrument resources when switching.
- [x] Restore the photo's Friendly/Enemy briefing layout, with inline aircraft and
  theater text selectors, original blue OK/green Cancel pieces, and ghosted inert
  opponents/unsupported fields. This explicitly scheduled creator revision does
  not open the other deferred menu screens.
- [x] Verify extraction/provenance, synthetic identity/dependency/selector tests,
  Rafale/Hornet loops, and Rafale exterior/cockpit plus wide/tall layouts. See
  [acceptance evidence](baselines/rafale-quick-mission.md).
- [x] Fix cockpit texture replacement on aircraft selection and add an independent
  Rafale presentation rig for original moving parts; gate the unsupported hook.
  See [follow-up evidence](baselines/rafale-animations.md).
- [ ] Translate exact Rafale animation hinges/schedules, continuous gear-well
  topology and native HUD callers; current motion is explicitly fitted.
- [ ] Complete native flight tick/contact/scheduler acceptance, mission generation,
  opponents, combat, loadout and full systems. Import success does not close these.

### Shared working flight model and second-aircraft extraction

- [x] Review RAFALE.PT as the distinct Rafale C, with its own 660-byte data layout, envelopes, engines and equipment.
- [x] Add `--aircraft rafale` to shared dependency extraction and portable scripting; preserve output/source provenance.
- [x] Move flight/attitude simulation into dependency-light `tore-sim`; retain renderer-specific animation tests in the app.
- [x] Resolve scalar parameters at state construction and remove per-tick envelope intersection allocation.
- [x] Implement selectable hybrid departure/spin, runway contact, braking/takeoff, wind and payload behavior with explicit fitted provenance.
- [x] Exercise both aircraft with the same headless scenario/replay suite; connect `--validate-flight` to extraction.
- [x] Expose Hornet hybrid flight through `--researched-flight`, preserving the legacy default for comparison.
- [ ] Original-game trajectory parity, collision/airfield/carrier producers, damage/loadout/fuel-transfer systems and exact native scheduling remain separate gates.
- [x] Integrate the separate Rafale cockpit, instruments, rendering and fitted
  animation rig with the shared kernel; native visual parity remains open.

Contracts and commands: [shared flight model](FLIGHT-MODEL.md).

### Separate aircraft laws and instrument data — 2026-09-14

- [x] Separate F/A-18D and Rafale C flight-law modules and per-instance validated tuning behind `FlightModel`.
- [x] Route selected model response/ground tuning into simulation; preserve existing baseline coefficients without inventing new aircraft calibration.
- [x] Add typed air/ground speed, atmosphere, altitude-datum and attitude telemetry for future instruments.
- [x] Keep unavailable IAS/CAS and barometric/pressure-altitude channels explicit rather than aliasing TAS/MSL.
- [ ] Add pitot/static and altimeter sensor/calibration/lag models and connect future analog gauge rendering.
- [ ] External mod-file schema/loading and independent native/real-aircraft calibration remain open.

### Complete typed flight configuration — 2026-09-14

- [x] Move mass, propulsion, envelopes, loading, native limits and fitted equipment/tuning into each aircraft model's validated configuration.
- [x] Remove the string scalar cache and duplicated research configuration; resolve PT fields once and fail on missing required data.
- [x] Wire startup, fuel/forces, payload checks, devices, departure/spin and ground contact to the selected model; remove alternate-aircraft update arguments.
- [x] Add validated configuration replacement before flight, preserving independent model instances and cheap presentation clones.
- [x] Preserve both aircraft's 26-scenario baseline; test configuration effects and rejected invalid edits. See [evidence](baselines/shared-flight-model.md).
- [ ] Native parity, unresolved PT fields and an external mod-file loader remain open; this is an ownership refactor, not additional native decoding.

### Integrate parallel flight-model and Rafale visual work — 2026-09-14

- [x] Combine the shared `tore-sim`/typed-configuration update with selectable
  Rafale rendering, original cockpit replacement and separate animation rigs.
- [x] Preserve `--researched-flight` across mission launches and aircraft changes,
  alongside the default legacy adapter; keep unsupported hook gating in shared state.
- [x] Retain typed multi-aircraft extraction and the portable `--validate-flight`
  workflow. Both extracted identities pass all 13 scenarios each.
- [x] Validate the combined tree: 122 Rust tests, 11 Python tests, formatting,
  warnings-denied Clippy, locked build, source/debug-binary guards and GPU checks.
  See [integration evidence](baselines/rafale-animations.md#shared-simulation-integration).

## Cockpit sliding and zoom follow-up — 2026-09-14

- Replaced perspective-tilted forward artwork with a flat overlay fixed to the
  aircraft-forward datum: head-look translates it in the opposite direction
  without clamping movement to the image margins.
- HUD and cockpit now scale with +/-; instrument windows remain screen-anchored.
- Reference yaw/pitch fade thresholds are fitted presentation, not native
  parity. Rear/overhead interior remains unavailable.
- Added bounded `--flight-zoom` and wide/tall, zoom, side/up validation in
  [the baseline](baselines/cockpit-slide.md).

## Live cockpit mirrors and uncapped rendering — 2026-09-14

- Extract three mirror silhouettes at runtime from each aircraft's original art.
- Render one shared reflected rear view, including ownship, every visible frame;
  no mirror timer or GPU-to-CPU transfer. Reuse world/aircraft GPU resources.
- Preserve cockpit pan/zoom/fade and screen-anchored instruments; omit hidden
  mirror passes and retain source fills when masks cannot be safely identified.
- Select Immediate/Mailbox presentation where supported and remove Wayland
  refresh callbacks in those modes; retain portable FIFO fallback and 120 Hz physics.
- Validate synthetic mask/camera tests, wide/tall GPU captures, camera previews,
  view cycling and measured mirror render counts. [Evidence](baselines/mirrors.md).
- Remaining parity: native mirror optics/eye location, curved reflection and
  complete interior geometry. No contacts or targets are fabricated.

### Shared controller input — 2026-09-14

- [x] Add safe, dependency-free `tore-input` physical binding policy and typed pilot frames; remove keyboard-name interpretation from `tore-sim` and migrate both aircraft's headless suites.
- [x] Apply ordered equipment/throttle commands at 120 Hz tick entry, with existing model-owned response and actual-state actuator audio. Preserve modifier/release and pause behavior.
- [x] Add calibrated axes, shared button/trigger contributions, explicit priorities, stable axis ownership, throttle pickup, maintained-switch policies and bounded input/profile/tape handling.
- [x] Isolate native calls in `tore-input-native`: Linux evdev/FF_RUMBLE, Windows RawGameController/Gamepad vibration and macOS HID queues. Keep unsafe forbidden elsewhere; cross-check both non-host modules.
- [x] Add media-free device diagnostics, create-new persistent profile generation, custom keyboard/controller bindings, standard Linux gamepad defaults and keyboard-only operation.
- [x] Detect the user's powered-on Ultimate 2, inspect its axes/buttons/rumble capability, derive serial/interface identity and load its generated profile in a real window.
- [x] Add instrument focus/direct button actions through existing stock scope controls; no MFD screen manipulation, new raster content or fabricated systems.
- [x] Add opt-in bounded authored crash rumble, explicit rumble diagnostic, per-device expiry and pause/focus/overflow/shutdown cancellation.
- [x] Validate 149 Rust / 11 Python tests, all 26 F18/Rafale flight-suite cases, six real window smoke tests, live camera readback, input-tape replay, asset guards and short matched frame-time evidence. See [input acceptance](baselines/input.md) and [setup/design](INPUT.md).
- [ ] User physical Ultimate 2 flight handling and unplug/reconnect acceptance; real HOTAS/pedals/button boxes and Windows/macOS runtime checks.
- [ ] Generic macOS HID rumble and hardware-tested directional flight-stick forces; further device defaults, radial calibration and reviewed multi-contact switch composition.
- [ ] Retail controller dispatch parity, long-stall wall-clock sampling guarantees and whole-mission replay remain separate gates. Further menu screens remain deferred.

### Controller rumble follow-up — 2026-09-14

- [x] Record user-confirmed Linux Ultimate 2 tactile pulse acceptance.
- [x] Stop Windows Gamepad vibration on endpoint removal/read failure, in addition to context/expiry/shutdown stops.
- [x] Add macOS 11+ GameController/CoreHaptics using exact retained input/haptic endpoints, explicit two-handle/default-locality routing and finite/cancellable effects; keep generic equipment on HID.
- [x] Document Apple gamepad session identity/shared-binding limits; add `--test-rumble only` with ambiguity rejection and native completion wait.
- [x] Revalidate workspace and both native cross-targets; see [input evidence](baselines/input.md).
- [ ] Windows/macOS linked app and hardware acceptance, Apple multi-controller/disconnect/haptic recovery and a persistent per-player assignment flow. Generic HID feedback and directional stick forces remain separate work.

### Event-driven rumble impulses — 2026-09-14

- [x] Add typed gun, missile, bomb, rocket, turbulence, afterburner, damage and crash cues with a fixed-slot, 120 Hz mixer; cap overlapping amplitudes, repeat rates and native submissions.
- [x] Wire actual afterburner activation and crash transitions; route to assigned capable devices even at rest, respecting opt-in/context and clearing pending effects on interruption.
- [x] Cover repeated fire, overlap/expiry, invalid turbulence severity, pause clearing and idle-device eligibility with synthetic tests. See [feedback contract](INPUT.md).
- [ ] Connect weapon/turbulence/damage producers when those systems exist. Space remains unavailable; no effects imply a successful shot or invented weather response.
- [ ] User afterburner tactile tuning and Windows/macOS hardware validation of overlapping/repeated effects.

### In-game settings and sustained afterburner feedback — 2026-09-14

- [x] Replace the in-flight Control device-selection stub with a paused binding editor: rumble, actions, device/control selection, key/button/axis capture, compatible behaviors, calibration, inversion, priority, add/remove and Save & apply.
- [x] Validate/canonically serialize profiles and replace files through a synced temporary file plus rename; leave invalid edits/live profiles unchanged. Preserve automatic mappings for newly connected gamepads when desired.
- [x] Persist normal-session instrument page sets for both layouts, selection/ranges/mode, cockpit/HUD/ladder/brightness/zoom and music/effects; retain them across aircraft changes and flight restarts. Keep visual diagnostics independent of user preferences.
- [x] Add a low continuous afterburner rumble beneath the ignition impulse, with finite renewable leases and explicit disengagement/context cancellation.
- [x] Validate synthetic capture/cancellation, profile save/reload/failure, preference round-trips/layout restoration and sustained-effect expiry; inspect wide/tall editor captures and creator/viewer/menu smoke checks. [Evidence](baselines/input.md).
- [ ] User tactile tuning and full Windows/macOS app/hardware tests; persistent per-player Apple assignment, radial/wizard calibration and physical HOTAS/MFD/button-box validation remain open.


## Recorded audio pass — 2026-09-14

- [x] Select original recorded PCM without MIDI conversion, soundfonts or a synth dependency.
- [x] Add shared app/CLI `--music` resource selection and optional `--wav-previews`; preserve archive boundaries, conflicts, bounds and SHA-256 provenance. Recover 99 recordings and nine scripts; report four unresolved PCM references separately from extraction success.
- [x] Parse the reviewed FA MUS data grammar with bounded CFG validation and phrase execution. Prepare all nine scores; use complete NORMAL (43/43 phrases) in both aircraft's free flight.
- [x] Replace the arbitrary AIR003 menu loop with the recovered main/briefing tables. Preserve repeated table entries; document authored creator/viewer mapping, context resets, gains and audio-only RNG.
- [x] Wire brake deploy/release cues on actual 120 Hz state changes for F18 and Rafale; include the explicit-contact SQUEAL dependency. Clear old aircraft loops on flight restart/exit.
- [x] Keep music/effects independent, pause without playhead catch-up, and preserve paused UI clicks in a separate bounded pool. Refresh the local app cache for the new music profile.
- [ ] Bind AIR/DANGER/DECK/LAUNCH/HOME/EJECT/SUCC/VALK only as their real systems arrive; complete native host priority, trigger, missing-media and audible transition acceptance.
- [ ] Finish hook/flap cue-polarity audit and native wheel-brake behavior; verify actual sound output/listening parity on Windows and macOS.

Validation and material limits: [audio baseline](baselines/audio.md). Further menu screens remain deferred. No retail/generated audio is committed.

## Two-aircraft manual weapons integration — 2026-09-14

- [x] Retain F18.PT / F/A-18D and RAFALE.PT / Rafale C only; exercise all ten
  default JT stations and all five source damage entries.
- [x] Separate master-arm/station/ammo/capacity readiness from actual sensor lock,
  with source range/FOV gates, imported visual/radar cycling, terrain visibility
  approximation and distinct launcher-dependent versus autonomous tracking.
- [x] Translate the native object-category damage switch; record bounded nominal,
  applied and cumulative hit results; keep automatic subsystem/RNG contracts open.
- [x] Wire source failed-station high bit, explicit fault fixtures, external-group
  jettison and mass/geometry updates; preserve internal guns and auxiliary mass.
- [x] Retire replacement targets atomically with fresh IDs; connect track-loss
  feedback and preserve original successful-fire/hit/destruction effects/audio.
- [x] Add bounded optional combat-service recording and matching-asset headless
  replay, including release and reset; preserve pause and modifier isolation.
- [x] Extend repeatable static extraction with damage category/amount/station
  spans, and validate both aircraft through source-cache runtime and GPU checks.
- [ ] Full alternative compatible loadouts/PTS presets, non-default bombs/rockets
  and special seeker branches; native sensor/RNG/subsystem/collision/effect parity,
  weapon textures/racks/drag, auxiliary fuel handling and player combat damage.
- [ ] Original-game differential acceptance and Windows/macOS runtime/hardware
  checks. Combat AI remains explicitly deferred until full manual acceptance.

Exact validation, captures, limitations and commands: [manual weapons baseline](baselines/manual-weapons.md).
