# Fighters Anthology Rebuild: Roadmap

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

T.O.R.E-Fighters in the Repo - Tasteful Opinionated Reverse Engineered

## Contents

- [Current scope](#current-scope)
- [Principles](#principles)
- [What "1:1" means](#what-11-means)
- [Milestone 0: Spec and salvage](#milestone-0-spec-and-salvage)
- [Milestone 1: Faithful quick fight](#milestone-1-faithful-quick-fight)
  - [1a. Shell and menus](#1a-shell-and-menus)
  - [1b. Original terrain](#1b-original-terrain)
  - [1c. Free flight](#1c-free-flight)
  - [1d. Quick fight loop](#1d-quick-fight-loop)
  - [1e. AI](#1e-ai)
  - [1f. Sensors and weapons](#1f-sensors-and-weapons)
  - [1g. Installer and first-run import](#1g-installer-and-first-run-import)
- [Milestone 2: Missions and campaigns](#milestone-2-missions-and-campaigns)
- [Milestone 3: Tools](#milestone-3-tools)
- [Milestone 4: Remaster layer](#milestone-4-remaster-layer)
- [Milestone 5: Live campaigns and multiplayer](#milestone-5-live-campaigns-and-multiplayer)
- [Milestone 6: Custom maps and community theaters](#milestone-6-custom-maps-and-community-theaters)
- [Importer coverage table](#importer-coverage-table)
- [Funding](#funding)
- [Open decisions](#open-decisions)

## Current scope

Development baseline: see [DEVELOPMENT.md](DEVELOPMENT.md), [ARCHITECTURE.md](ARCHITECTURE.md), and [recorded validation](baselines/environment.md). The first M1a [main-menu slice](baselines/main-menu.md) now imports original menu assets and runs natively. M0 research and the remaining M1a screens/audio work remain in progress.

This is the sequencing document for the ground-up rebuild in Rust. Current status
and the next feature are tracked on one page in [the parity plan](parity-plan.md).
The behaviour being rebuilt is described in prose in [docs/spec/](spec/); how
agents work from those specs is in [AGENTS.md](../AGENTS.md).

The existing TypeScript repo /USNF-ATF is the guide, not the gospel: its format docs, decoders, recovered geometry, audio recovery, and baselines are the reference material. Its engine is not being ported. Original terrain and environment systems are rebuilt from retail assets and from the game's observed behaviour; USNF-ATF's custom terrain system and DEM-based theaters are not being ported. Further menu screens are deferred until explicitly scheduled.

Current state (2026-09-17): original menus, the quick-mission creator and
ordnance screen, all 16 theaters, and twelve aircraft in free flight with
cockpit, HUD, instrument windows, weather and controller support. The
[ported roster](spec/ai-experience.md#currently-ported-aircraft) identifies all
twelve and their AI family bindings. A development
weapons range supports manual weapon testing. Ground contact and landing are
authored behaviour ([opinionated](behavior-provenance.md)); combat AI has
spec-derived components and a partial Quick Mission hookup. John requested aircraft and surface AI research
and planning on 2026-09-17; the 2026-09-22 M1 air-to-air plan now prioritizes
awareness, search and mission engagement. Current stages are in M1e below.

John scheduled the shoreline correction and ocean-motion trial on 2026-09-16.
See [ocean behavior and visual scope](spec/ocean.md). This bounded visual
work precedes the following order without authorizing additional systems work.

John requested initial F-14, A-4E and X-31 ports on 2026-09-16, then specified
Fighters Anthology sources throughout. F-14D, A-4E and X-31 EFM now have initial
ports through the [aircraft import workflow](aircraft-import.md). See
[acceptance and remaining limitations](baselines/aircraft-fa-expansion.md).
USNF-ATF is a research guide only. Continue maneuver audio/rumble, remaining
flight-response and weather work in the [parity plan](parity-plan.md).
No default adapter change or AI scope is included.

The preceding execution order (2026-09-14) required manual weapons, sensors and
damage acceptance for F/A-18D and Rafale C before AI work. John's 2026-09-17
request scheduled M1e research, followed by partial live aircraft hookup. The
2026-09-22 request prioritizes its air-to-air gaps; existing straight-flight
weapon fixtures remain available. See
[current systems evidence and remaining gates](baselines/weapons-systems.md).
The manual range covers both aircraft's ten PT-default weapon slots, partial
ECM/player-damage integration and controller feedback. This does not close the
M1d/M1f acceptance gates. The later aircraft ports are part of the current
twelve-aircraft scope; their individual acceptance limits remain documented.

## Principles

1. **Faithful first, opinionated second.**  Milestone 1 and 2 reproduce the retail game.  Every expansion, remaster, and quality-of-life change lands as a layer on top that can be switched off, so a "classic" mode always exists and always matches retail.
2. **Bring your own copy.**  The repo ships no retail bytes.  The importer reads the user's own Fighters Anthology media at runtime and writes to app data.  A signature scan for EALIB, PIC, and other retail markers stays a release gate.
3. **Hand-rolled where it counts.**  External dependencies are kept to a minimum.  Formats, synth, terrain, and sim are ours.
4. **Importer grows with the game.**  There is no "import everything" phase.  Each step decodes exactly the formats the next playable piece needs.  Breadth is tracked in a coverage table, not a milestone.
5. **Cross-platform from day one.**  Linux, Windows, and macOS build and run at every milestone.  No platform is "deferred" this time.
6. **Baselines are recorded, not remembered.**  Every milestone writes its measurements and acceptance evidence to `docs/baselines/`.
7. **Deterministic and headless from the start.**  Massive battles and live campaigns are the reason for the rewrite.  The sim runs without a renderer and produces identical results from identical inputs from M1 onward.  This is a constraint, not a feature.

## What "1:1" means

Parity is measured **by expression of feature**: the player must experience what
they experience in Fighters Anthology. It is not a recreation of the original
program's code, control flow or internal structure. The test for whether
something belongs in a behaviour spec is "would a player notice if this were
different?"

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
- The original program's control flow, call ordering, caches and RNG ordering

AI behavior is recreated from a behaviour spec, like every other feature; the
retail AI bytecode VM is not reimplemented. This follows from parity by
expression of feature and shapes M1e.

---

## Milestone 0: Spec and salvage

**Goal:** know what is being rebuilt before writing engine code.

Work:
- Inventory the TS repo and mark every artifact as *spec* (format docs, byte layouts, recovered DLG geometry, MUS scripts, PT field maps, baselines) or *implementation* (engine code, React shell, Three.js render).  Spec carries forward.  Implementation is reference only.
- Inventory the full Fighters Anthology disc layout: USNF '97, ATF Gold, NATO Fighters, Marine Fighters, and the Pro Mission Creator.  Produce a format-by-title census.
- Write the 1:1 definition above into `docs/` and get it settled. **Done:** see
  "What 1:1 means" above and [AGENTS.md](../AGENTS.md).
- Decide the AI VM question. **Settled 2026-09-15:** behaviors are recreated from
  a spec, not reimplemented from the recovered bytecode.
- Set up the Rust workspace, three-platform build, and the retail signature scan.

Deliverable: an app on all three platforms that opens a window, prints its renderer, and passes the scan.

Exit: census committed, 1:1 definition committed, AI VM decision recorded.

---

## Milestone 1: Faithful quick fight

This is the first playable milestone: original menus, original terrain, the
current twelve-aircraft roster with basic systems, and guns and missiles quick
fight against AI. The original four-aircraft target has expanded with the ports.

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

### Airport and ground-object expansion

**Implementation plan, requested by John on 2026-09-20.** The deliverable is a
playable airport world in TORE: source-positioned runways and surrounding objects
for every map, individually targetable ground structures, and player landing/tower
radio commands with landing guidance. Ukraine is the first complete slice.
Research serves implementation decisions. It is not a requirement to reconstruct
the original engine, scheduling or every callback before shipping.

[Behavior and fitted decisions](spec/airports.md),
[placement/resource contracts](formats/airport-placements.md),
[measured input coverage](baselines/ukraine-airports.md).
Current implementation: base-layout import, static rendering/targets, runway
surfaces, player tower commands, ILS and replay are connected. Independent review
fixed target reset, source classes/signatures, altitude reference and rendering.
The base-theater slice is implemented; campaign overlay/generated terrain handling,
unsupported shape programs, additional tower speech, speed brackets and target
camera imagery remain open. See the linked baseline for validation and limitations.
Ground start is now connected to the creator's player start and airport selectors;
other wings remain airborne and restart preserves
the accepted start. [Behavior](spec/quick-mission-menu.md#player-ground-start).

These linked documents own constants and evidence; this section owns execution.
All proposed type/file names below are agent design choices, not existing APIs.

#### What we can build on

| Existing TORE component | Reuse | Required extension |
| --- | --- | --- |
| `tore-formats::theater::Environment` | Bounded top-level M/MM environment parsing | Separate object-block reader; side tables and both nationality forms |
| `tore-formats::aircraft::Brf` and OBJECT schema | Inert OT token/field parsing | General static object definition, explicit resources and reviewed scalar fields |
| `tore-formats::strip`, SH contact boxes | Reviewed STRIP identity and runway anchors | Review all thirteen airport definitions and their actual shape contracts |
| `tore-app/src/assets.rs`, `tore-extract` | Archive provenance, runtime imports and caches | One shared transitive object dependency resolver; import report/cache version |
| `terrain::World` and `World::surface` | Source terrain, weather, grounding queries | Scene construction and runway surface query integration |
| `tore-sim::combat::live::Target` | Stable IDs, ground roles, hit points, sensors and damage classes | Per-object configuration and building/runway contact geometry |
| `combat.rs`, `sim_renderer.rs`, HUD/instruments | Current target display, geometry submission, palette and shadows | Static meshes, ground-object identity, ILS readout and airport selection |
| `audio.rs`, `tore-formats::radio` | Serial recorded speech playback and reviewed phrase mappings | Airport response events; only independently reviewed tower phrases |
| Pilot input and `combat_tape.rs` | Fixed-tick commands, replay and resource fingerprints | Airport selection/commands, scene identity and damage/clearance replay |

The aircraft renderer currently uses an aircraft-specific scale. Do not reuse it
for ground objects without measurement. Combat presentation also contains ID-to-
aircraft-index assumptions; remove those assumptions at the shared scene boundary
before adding ground targets. Current projectile tests use swept spheres; long
runways and buildings need dedicated geometry rather than inflated target radii.

#### Runtime shape and ownership

Create an immutable imported scene containing definitions, placements, dependency
identities and reviewed airport geometry. Keep GPU meshes/textures in the app.
Keep mutable target health in the existing combat state, with a stable mapping
from source placement to target ID. Keep airport selection, availability and player
clearance in a renderer-independent `tore-sim` airport service. Do not create two
independently writable health copies; derive airport availability from combat
changes. A destroyed building remains identifiable for mission goals and replay.

Use source layout identity plus record ordinal as the stable import key. Retain
signed source aliases separately; they are not globally unique object IDs. Allocate
runtime IDs deterministically through the existing ID owner, disjoint from player
and aircraft IDs. Ground objects do not consume aircraft roster indices.

Per fixed tick: apply recorded player commands, advance existing flight/combat
with shared world-contact inputs, deliver damage/contact events to the airport
service, and publish a readout plus radio events. Rendering interpolates poses
and reads that snapshot. Audio playback never changes clearance timing. Build a
new scene completely before switching theaters; failed loads leave the old scene
intact. Flight reset reconstructs target and airport state from the same scene.

#### Slice A: bounded importer and Kiev visible world

**Prerequisites:** existing Ukraine T2/MM and reviewed initial assets are available.

- Add object parsing alongside Environment rather than mixing mutable world state
  into terrain metadata. Preserve position, orientation, name delimiters, flags,
  speed, aliases, side tables, source order and unknown fields. Enforce existing
  mission size limits plus explicit per-record/token bounds. Report malformed
  records with resource and line context. Never execute mission scripts.
- Add a general static OT reader using the existing schema. Expose explicit main,
  damaged/shadow references only where their meanings are established. Unknown
  callback names remain inert metadata. Keep the narrow STRIP diagnostic intact.
- Share dependency discovery between CLI and app import. Traverse explicit OT/SH
  references with cycle detection and bounds; retain exact archive provenance.
  Missing resources name both the missing resource and referring object. Reimport
  old caches when needed, with a clear diagnostic rather than invisible buildings.
- Implement reusable static shape mesh/texture loading without aircraft rigs.
  Apply one reviewed scale and placement transform to visual and contact geometry.
  Batch shared type geometry and cull by camera bounds; avoid duplicating a mesh
  for every instance. Reuse source palettes, lighting, cutouts and shadows.
- Render Kiev's runway, four hangars and tower at their source coordinates. Inspect
  low-altitude views from both runway ends and an overhead view. Compare numerical
  transforms to the source, not modern geography or a nearby terrain-texture image.

**Exit:** six expected instances, correct names/types/transforms, no unresolved
required visual resources, no runway buried in terrain, no ID coupling to aircraft.
Synthetic tests cover truncation, signed angles/aliases, unknown tokens, duplicate
fields, resource cycles and transform consistency. This is a visible scene slice;
landing and target operation arrive in subsequent slices.

#### Slice B: complete Ukraine placement and every-map import

**Depends on A.** Expand Ukraine to all fourteen runway records and the 99 objects
in airport-labeled sections. Preserve the other 144 source objects in the scene
manifest even when outside this airport-focused rendering scope. Keep association
as metadata; it must not cause collective damage or ownership changes.

The census already extracted all 75 MM layouts and thirteen airport definitions.
Review each definition's shape geometry, scale, collision records and dependencies.
Treat STRIP3A/5A/6A/7A as independent placed instances, not damaged versions or
additional airports inferred from their names. Review their relationship to the
other runway pieces before exposing airport groups in the UI.

Implement `sides`/`sides2` and `nationality`/`nationality2` using their documented
conversion. Inventory airport-associated objects in every base layout. Prefer
explicit relations, then source authoring groups; ambiguous records remain
unassigned and appear in the audit. Retain/render ambiguous static placements in
the full scene rather than silently excluding a possible airport building. If
that requires non-airport static types, process them through the same loader.
Mobile/armed objects may have static placement/presentation, with no new behavior.

Run a coverage pass over all sixteen base theaters, including all thirteen runway
types. Resolve mission overlays separately: prove whether a source supplies a full
layout or patches it, how signed aliases replace/delete records, and how generated
`~` terrain names resolve. Never append two full layouts or redirect a campaign
alias to a base terrain without evidence. Unsupported overlays get explicit errors.
Campaign resolution remains in the final every-map scope, even if base maps ship
first; do not call base-only support complete.

**Exit:** every expected airport/associated object accounted for by source key;
per-layout counts and dependency manifests reconcile; no unresolved required
placements on accepted maps. Inspect every Ukraine airport and one view per other
base theater plus each runway type. Numerically validate every placement even
when visual checks are representative. Repeated import is unchanged and missing
resources/ambiguous overlay operations are actionable errors.

#### Slice C: runway surfaces and individual ground targets

**Depends on A; finish Ukraine first, extend through B's shared definitions.**

Add an airport surface query using reviewed extents/anchors and the fitted support
policy in the spec. Wire it through all applicable World surface consumers so
flight, target grounding and presentation agree. Building contact uses a separate
solid-object query, not roof height returned as terrain. Verify both runway ends,
edge transitions, taxi exits, gear contact and off-runway terrain. Preserve legacy,
hybrid-default and restricted native-table adapter contracts and report which
surface interactions each supports.

Decode each OT's health, class and sensor-relevant fields. Construct zero-velocity
ground targets with their own configuration. Use the current designation/sensor/
weapon eligibility pipeline and target window. A control tower, hangar and runway
must be distinct targets. Keep target identity after vector compaction/removal.

Extend projectile contact with segment-versus-oriented-box or reviewed contact
volumes, preserving earliest impact against terrain/objects and distinct fuze
radius handling. Do not change aircraft hit volumes in this slice. Reuse damage
classes, hit records and effects; connect destruction to airport availability by
one event. Add a target-destroyed notification usable by future mission goals,
without implementing the mission scripting engine here.

**Exit:** designated tower is the object hit; adjacent structures remain unchanged;
long runway geometry cannot intercept shots far outside its footprint; one
threshold-crossing destruction event is emitted; destroyed objects do not respawn
on camera changes. Synthetic deterministic tests cover high-speed tunneling,
nearest impact, collision/terrain ties, repeated hits, reset, IDs and replay.
Conduct a manual ground-attack and runway-contact pass on Ukraine.

#### Slice D: player landing guidance and tower radio

**Depends on C for runway availability; UI can develop against synthetic scenes.**

Implement typed `SelectAirport`, `RequestLanding`, `RepeatReply` and
`CancelApproach` actions, with one player clearance record and explicit response
reasons. Resolve input bindings against `tore-input`/app shortcuts before assigning
keys; expose configurable controls without taking an existing binding silently.
Use the existing HUD/font/menu pieces for airport choice and radio text.

Follow the spec's command and ILS rules. John's requested activation height is
4,000 feet above airport ground level, including the threshold. Recovered retail
evidence can refine other fitted rules without overriding this user choice. Requesting clearance sets guidance, not autopilot. Existing
manual flight and autopilot controls retain their behavior. Land completion comes
from actual contact and the fitted completion condition, not proximity alone.
The service invalidates clearance if its runway becomes unusable. Changing airport,
repeating a request, canceling and starting a new flight have explicit transitions.

Publish threshold/end, bearing/range, clearance status, localizer/glide deviation
and optional aircraft-specific speed brackets in a HUD readout. Test the published
values independently of pixel rendering. Feed typed replies to existing serial
radio playback and subtitles. Extend phrase extraction only for proven text/sample
pairs. Do not repurpose wing formation phrases as tower dialogue.

**Exit:** select an airport, request and receive an appropriate reply, fly an
indicated approach, touch down and taxi manually. Repeat/cancel/reset and runway
loss behave deterministically. Tests exercise ILS distance/altitude boundaries,
gear/NAV gates, both ends, off-axis/behind-threshold cases and zero-distance math.
No audio device or missing sample prevents the same clearance result. Replay
reproduces selection, replies, target damage, guidance and landing completion.

#### Research tasks bounded by implementation decisions

Do these within the owning slice. Stop once a prose rule can be written. If the
consumer remains unresolved, ship the documented fitted rule where available and
keep the evidence gap visible.

| Decision | Retail evidence to inspect | Implementation action |
| --- | --- | --- |
| Shape scale and runway extents | SH transform/header consumers and contact records for all thirteen types | Measure per type; inspect rendered/contact overlap before acceptance |
| Compound airport layouts | MM records, explicit aliases and STRIP variant geometry | Preserve all records; group only confirmed relationships |
| Allegiance/overlay semantics | Mission parser branches and actual base/campaign layouts | Typed conversion/patch rules; reject unresolved operations rather than guess |
| ILS activation and symbols | HUD consumers near airport selection; manual pages 67/87 disagree | Keep John's 4,000-ft airport-relative choice; research remaining gates/symbols; test the boundary |
| Radio request/reply repertoire | Player menu/input producers, APCommentProc, speech tables and sample mappings | Use the opinionated minimal command set with text fallback |
| Targeting and damage | Relevant OT fields, sensor eligibility and damage-class consumers | Reuse current combat model with per-object inputs and labeled contact approximation |
| Runway/tower destruction effects | Airport availability queries after object damage | Use the spec's independent service policy until stronger evidence exists |

The nine slots in the earlier STRIP template are evidence for that template, not
a universal capacity requirement. NPC traffic, holding patterns, autonomous taxi,
automatic player landing, capture, repair/rearm/refuel and a mission editor are
not required to deliver this plan. Existing AI must keep working alongside ground
objects, but this plan adds no autonomous behavior.

#### Release and verification

Ship coherent increments A, Ukraine B/C, D, then complete B's every-map expansion
and rerun C/D against all airport types. The every-map task remains open until the
coverage report closes base and supported retail mission/campaign layouts.
Do not label unsupported layouts as empty or successful. Record visual, interactive
and headless acceptance separately. Keep all retail-derived meshes/captures local.

Run the repository's required formatting, Clippy, locked tests/build, Python tests,
asset guards and documentation checks for each completed implementation change.
Rendering changes also require the real display smoke test and inspected captures.
Exercise Linux now; record Windows/macOS runtime validation as performed or pending.
Measure startup time, static mesh memory and frame time with airport scenes before
and after on the same host; optimize measured regressions without reducing object
coverage silently. Do not impose a speculative performance budget in advance.

Update affected feature-matrix rows and guides for extraction, objects/shapes,
theater, architecture, input and flight controls as each behavior lands. New replay
records get an explicit version and compatible old-tape handling. Milestones are
reported in the session. No commit, push or default-adapter change is authorized
by creation of this plan.

### 1c. Free flight

Aircraft: all twelve in the [ported roster](spec/ai-experience.md#currently-ported-aircraft).
F-14D, A-4E, X-31 EFM and F/A-18D were the original target; Rafale C,
MiG-29, Su-27, MiG-21, Su-25, MiG-23, Su-35 and F-22A are also in the port.

Work:
- Importer: PT, SH, and the cockpit and HUD assets for every ported aircraft.
- Fixed-rate sim loop, decoupled from render, headless-capable.
- Flight model from PT.  Engine, gear, flaps, hook, brakes, throttle, afterburner.
- Cockpit view, HUD, external and chase cameras, control surface animation.
- Navigation display and waypoints.
- Keyboard, gamepad, and joystick input.
- Engine, actuator, stall, and environment audio from retail samples.
- Maneuver harness: level flight, sustained turn, loop, stall, runnable headless in one command.

Deliverable: take off, fly, and land any of the twelve ported aircraft on any imported theater.

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

**M1 scope: air-to-air awareness and engagement.** The
[development specification](spec/ai-awareness.md) covers visual cones,
skill-scaled memory, search, missile defense, shared AI/RWR threat information
and mission rules. Observation/memory and search/Target-view activity are
implemented. Missile defense and shared RWR information are implemented;
mission rules remain pending. Surface AI
and additional behavior families remain in the broader backlog, outside this scope.

The [main AI behavior specification](spec/ai.md) now covers established fighter
choices, other family differences, surface boundaries and proposed API inputs.
The [experience specification](spec/ai-experience.md), [source map](formats/ai.md)
and [research baseline](baselines/ai-research.md) provide its companion evidence.
AI-0 has an installed-archive census; loose overrides and mission bindings remain.
AI-1 is closed: Quick Mission skill is uniform per wing, saved per-object skill
is final, the enemy-skill flight-menu override and the human-only G exemption
are specified. AI-2 covers fighter decisions, timing, pursuit, targeting,
weapon-service cadence, steering with performance and terrain rules, seeker
gates, ammunition, wing orders with formation geometry, threat warnings,
countermeasures, reason priority, routes and fuel. AI-3 has a partial runtime integration: the components are joined by a per-actor
`ai::controller` that sequences them, with `ai::fitted` supplying one named
fitted rule per unresolved branch so a live actor cannot stall. AI-5 is
partially integrated for aircraft: `ai::mission` gives every actor its own sensors,
stores, flight model and decision state, debits ammunition before emitting a
launch event, and does not duplicate missile physics. AI-6 is partially integrated for
Quick Mission: `ai::launch` carries side, wing, member, type and resolved
experience. Quick Mission enables the live hookup by default, with separate
wing groups and idle delta-formation following. `--fixture-wings` keeps the
straight-flight setup. Reviewed runtime defects are covered by the
[repair baseline](baselines/ai-research.md); AI seeker lifecycle and broader
mission integration remain partial. AI-4, surface behavior, remains pending.
The delivery sequence below prioritizes this scope; the older backlog retains
remaining research and integration work.

#### M1 air-to-air awareness delivery

Behavior, provenance, initial tuning constants and acceptance cases have one home in
[air-to-air awareness](spec/ai-awareness.md). Deliver these slices in order:

| Slice | Concrete work | Exit evidence |
| --- | --- | --- |
| A: Observations and memory, implemented | Add actor-owned timestamped observation records between sensors and decisions; separate live observations, memories and bearing-only warnings; skill-filter visual acquisition | Cone/range boundaries, expiry, hidden-turn and Novice kill/reacquisition tests pass; no hidden world pose refresh |
| B: Search and Target view, implemented for current mission context | Connect remembered-position investigation, acquiring and return/rejoin to steering; expose real activity through the existing Target window | Lost-contact scenario visibly searches and reacquires or returns; deterministic headless transitions and display smoke |
| C: Missile awareness and defense, implemented for reviewed profiles | Connect actual A pitbull, S supported launch and I/E visual sightings through shared actor-owned RWR threat records; show known missiles in player RWR with incoming threats blinking; add skill-based time-to-defend assessment, jink/notch/dive selection, timed inventory-backed bursts and re-engagement; specify missing missile notch response | Silent midcourse and unseen passive shots provoke no response; immediate supported-launch warning; matching AI/RWR knowledge, receiver-specific blinking, safe maneuvers, effective sensor/support coupling, bounded device use and no hidden launcher knowledge |
| D: Mission roles and stances | Explicit protect/destroy/escort assignments and stance inputs, narrow protected-aircraft threat reports, priority selection and leash; connect minimal Quick Mission assignments | Escort protects its charge instead of chasing unrelated enemies; hostile escorts follow symmetric rules; objective label uses assignments |
| E: Integrated combat acceptance | Finish required AI seeker acquisition/activation/pitbull hookup; run role, sensor, weapon and survival scenarios together | Twelve aircraft by four skills, mixed roles, Novice multi-kill limits, repeated-seed replay and measured 30-aircraft fixture; full repository checks and display smoke |

Implementation should extend `tore-sim::ai::mission`, `controller`, `targeting`
and threat services, with the shared sensor component supplying observations.
Keep simulation state independent of `tore-app`; the app passes assignments and
renders activity. Existing tactics and weapon services remain reusable. Each
slice needs its own specified behavior and tests before being described as
complete. No new runtime dependency or flight-adapter default change is planned.
Search-orbit constants and current route/visibility limits are documented in the
[specification](spec/ai-awareness.md); [stage validation](baselines/ai-awareness.md).
Slice C depends on real missile lifecycle events, so bring the required seeker
activation/support adapters forward from slice E. Specify missing missile notch
rejection before accepting defensive effectiveness; attempted maneuvers alone
do not close that gap.
Report slice milestones in the session for Jeeves until the PM file is restored.

#### AI backlog (2026-09-17)

Established rules with tests are listed in the
[implementation status table](spec/ai.md#implementation-status). Everything
else falls into one of four kinds.

Missing evidence (research, in priority order):

1. Remaining signature branches and seeker state producers beyond the reviewed
   B45 launch envelopes, target-class selection and support rules.
2. Remaining tactics: last-ditch candidate suitability, the random-tactic menu
   contents, engagement-pitch rule, jink and circle shapes, what a script
   restart does to a maneuver in flight (B11, B12, B13, B47).
3. Surface classes: event mask meanings and command operands in the surface
   event handler, then SAM, AAA, vehicle, ship and carrier contracts (B30).
4. Wing remainder: approach steering point, mode 9 negative-band entry,
   loose-versus-medium self-engagement, 20 second target deadline expiry,
   bug-out helpers, reply voicing (B43, B46).
5. Recovery and survival: takeoff and landing sequences, leader and singleton
   return to base, damage-triggered disengagement, attack-state producers (B48).
6. Small units: B05 thrust-to-weight scale, pursuit offset signs, lead speed
   estimator, minimum-speed exemption producer, bank bound second term, burst
   policy after a shot, decoyed-missile time shortening, template ground skill
   values, prefs persistence of the enemy-skill override.

Implementation work (spec established, not yet coded): family variants for
F-117, helicopter, bomber, AC-130, large and MOTH behaviors (B20), which
`Controller::new` currently rejects rather than serving fighter behavior; the
hydrofoil program (B30); surface actors, which `ai::targeting` still reports as
an unresolved selector.

Fitted rules now standing in for unresolved branches, each named in
`ai::fitted` and listed in [behavior provenance](behavior-provenance.md):
engagement pitch, base pitch rate, zero-duration completion axis, last-ditch
candidate suitability, random-tactic menu contents, remaining tactics,
lead speed estimator, burst and reload pacing, store hit chance, and leader
and singleton return to base. Each becomes spec-derived when its research item
above closes; none is a claim about retail behavior.

Validation work: synthetic scenarios per aircraft and experience level (48
combinations) that run the components together headless with a fixed seed;
determinism and restart tests for the live controller; a review of the
fitted steering curves against any flight-model turn data already measured.

Integration work remaining after the 2026-09-17 hookup: surface actors in the
same runtime; mission routes and orders beyond the fuel and waypoint rules
already wired; full AI seeker acquisition, activation and pitbull beyond imported launch
envelopes; and the wing-approach value producer, without
which the B12 wing-split branch stays untried rather than being fed ordinary
target distance.

Initial AI delivery covers every aircraft in the
[ported roster](spec/ai-experience.md#currently-ported-aircraft). All twelve bind
to the fighter/strike family in FA, so the first family implementation must
serve all twelve using their own capabilities. Acceptance covers 48 combinations
of aircraft and experience level, plus mixed-aircraft encounters. Wider retail
families remain in the research plan without delaying this roster behind new
aircraft imports.

| Stage | Work and deliverable | Exit evidence |
| --- | --- | --- |
| AI-0: FA inventory | Extend the initial FA_2.LIB census to archive precedence and mission bindings; retain exact aircraft identities and separate static scenery from autonomous objects | Every referenced behavior family has a build/source identity, evidence category and explicit gap list; no unnamed fallback controller |
| AI-1: Experience | Trace the six wing selections, four side/domain assignment channels, per-object mission values and all type-appropriate skill consumers | Specify Quick Mission distributions, saved-skill precedence, tactical percentages, G exemption and device reactions; synthetic boundary cases for 0..3 and invalid input |
| AI-2: Aircraft behavior specifications | Complete fighter/strike and defensive behavior first, then F-117, helicopters, bombers/AC-130, transports/airliners and special families; include formation, orders, navigation, fuel and damage responses | For each maneuver and decision, prose gives trigger, target geometry, units, limits, duration/completion and interruption rules; source/BI disagreements and unsupported aircraft motion are explicit |
| AI-3: Isolated Rust components (runtime gaps remain) | Implement specified behavior slices in renderer-independent `tore-sim::ai`; add only needed bounded data readers to dependency-free `tore-formats` | Deterministic headless scenarios exercise each family's decisions and maneuvers at all applicable experience levels; known approximations have named rules/constants and provenance |
| AI-4: Surface behavior | Trace and specify static defenses, SAM, AAA, mobile ground units, ordinary ships, hydrofoil and carrier behavior separately, then implement isolated components | At least one representative fixture per supported class validates detection/eligibility, movement where applicable, fire control and experience; scenery never acquires an invented combat brain |
| AI-5: Simulation service adapters (partial for aircraft) | Generalize actor ownership for sensors, weapons, missile support, damage, fuel and movement; feed isolated controllers through those services | Multiple actors own independent contacts, stores and targets; no free ammunition, omniscient targeting by accident, duplicated missile physics or player-state contamination |
| AI-6: Later game hookup (partial for Quick Mission) | Replace the lossy dummy-wing launch payload with side, wing, member, type, loadout, experience source and resolved level; connect mission routes/orders and activity display | Six mixed-skill wings retain identity end to end; replay and headless/live results agree; player and straight-flight fixture paths remain available |

Research can advance by family and spec section; completing all executable
routines is not a prerequisite to implementing a specified slice. Surface
research can proceed once shared actor/weapon contracts are understood, without
waiting for every aircraft family. Surface firing implementation depends on
surface sensing/designation and launcher ownership, which are not supplied by
the current aircraft range. Helicopter, bomber and ship decisions can be tested
in isolation before their required movement/import coverage exists; that does
not count as flown acceptance.

Proposed interface: a controller receives its mission/order context, own state,
equipment, resolved experience, permitted observations and threat events. It
returns desired motion, sensor/target requests, weapon/device requests and a
player-readable activity. It does not directly move bodies or apply damage.
Aircraft motion requests go through a steering adapter and the selected flight
model; surface motion uses its own bounded rules. Retail awareness shortcuts,
if established, must be explicit spec-derived inputs rather than hidden access
to all entity positions. No default flight adapter changes are included.

Keep behavior family, mission role, equipment and experience as independent
inputs. A fighter family does not imply that a particular aircraft carries a
radar missile; Ace does not imply better radar hardware. Use one controller
implementation with experience-dependent rules where supported. Keep actors'
decision state and deterministic random streams in simulation state. This is a
proposed host design; the original RNG ordering and command buffers are not
acceptance targets. Fixed 120 Hz simulation stays independent of rendering.

Acceptance scenarios must include:

- Head-on merge, pursuit, overshoot, defensive break and simultaneous threats at
  each level, with maneuver envelopes and durations checked against the spec.
- Conditional tactical probabilities tested independently of encounter win
  rates; explicit draws at each threshold, plus aggregate checks with stated
  sampling tolerances. Higher experience need not win every random fight.
- Lost/occluded contacts, wrong-side and destroyed targets, missile-support
  loss, empty stores, low fuel, damage and terrain avoidance. Fitted rules are
  tested as fitted rules, not claimed as measured retail behavior.
- Wing orders, formation split/rejoin and interrupted route resumption, then
  SAM/AAA engagement, a moving vehicle and a ship attack in separate fixtures.
- Same seed and inputs across repeated runs and render schedules; stable actor
  identity through removal and restart. Check a 30-aircraft fixture matching
  current creator capacity and record cost, without inventing a retail timing
  requirement or a performance promise before measurement.

Final deliverable: aircraft and supported surface opponents with behavior tied
to the correct experience input, integrated only after isolated acceptance.
Liveness means an eligible combatant can act and fire when appropriate, not that
unarmed transports or scenery must engage. Retail recordings may add evidence
if available later; their absence is not a blocker and passing our tests is not
a claim of demonstrated retail parity.

### 1f. Sensors and weapons

Current authorized slice: shared aircraft radar, requested 2026-09-16, independent
of the AI milestone. [Behaviour and roster stats](spec/radar.md),
[standard component guide](radar.md), [validation](baselines/radar.md).
Research established the imported radar profiles and the normal range-mode rule;
the component then shipped on the same day. Implementation milestones:

1. **Done.** Shared radar/IR A2A profiles and simulation-owned observations with
   one selected target and at most one fire-control track, using PT radar/IR
   signatures and the authored look-down, era, notch and jammer model.
   Destroyed-aircraft combat state is separate from physical sensor presence.
2. **Done.** Scope range/mode correction, directional jammer noise, the RCS
   exposure panel, persistent mouse selection and Y history using stable target
   IDs. Aircraft orientation feeds the same effective signature used by the RCS
   contour and detection. Radar/jammer generation matchups are explicit profiles.
3. **Done.** Radar-guided launch and maintained-support transitions with
   deterministic tests, including version-3 combat tapes.

Remaining in this slice: the tuning pass. All twelve aircraft produce the
capability summary and pass their combat smokes, but the twelve results have not
been reviewed side by side, so the presets and jammer matchups are unplayed
against each other. See the
[delivery table](radar.md#delivery-and-acceptance) for what each stage delivered.

Mode scope is TWS/RWS and installed infrared air-to-air, with single-target
tracking only. Gamified IFF stays in the target view. A2G/HARM awaits ground weapons
and object systems; no realistic IFF, multi-track or AI work is authorized.
Unresolved retail details use documented fitted rules; complete source-code
closure is not a prerequisite.

The [missile update plan](missile-update-plan.md) records the completed current-store
implementation: four guidance types, pitbull activation, launch velocity and range,
motor burn, guidance lifetime, uncued seeker search and HUD/tone feedback, with
delivery stages, [measured acceptance and remaining limits](baselines/missiles.md).

Work:
- Radar modes, RWR, IFF, and the retail sensor model.
- Missiles, bombs, and gun stats from retail data.  Stores affect weight and flight.
- Per-system damage.
- SAM sites and ships as targets and threats.
- Loadout compatibility mask decoded so stations offer real options.

Deliverable: full quick fight with retail weapons against air and ground threats.

Exit: **Milestone 1 tagged.**  A stranger can install it on any of the three platforms, import their own disc, and fly a quick fight.

---

### 1g. Installer and first-run import

Planned 2026-09-21 at John's request. A player installs T.O.R.E on Windows,
macOS or Linux, points it at their own Fighters Anthology, and reaches the main
menu without a terminal. Behaviour: [first-run import](spec/first-run-import.md).
Container research: [SETUP.ESA notes](formats/esa-installer.md).

What the research settled:
- The importer needs five files (`FA_1.LIB`, `FA_2.LIB`, `FA.EXE`, optional
  `FA_4B.LIB`, `FA_4D.LIB`). On the retail discs they live inside
  `disc1/SETUP.ESA`, a flat container whose compressed entries use the DCL mode
  the LIB reader already decodes. Disc 2 is not needed.
- The disc's 1.0 executable carries the same creator, cloud, flare and radio
  tables as the reviewed 1.02F build at shifted addresses; content is identical.
  The patch changes no gameplay resource the app reads.

Slices, in order:

1. **Media sources.** `tore-formats::esa` reader with synthetic tests. A
   `MediaSource` in `tore-app` that yields the five files from an installed
   folder or a disc folder, detected by content. Second executable fingerprint
   with its address set in the four table readers. Import report names the
   build. CLI: `--import` accepts either kind; `tore-extract` gains
   `--source` support for a disc folder.
2. **First-run screen.** Locate screen in original menu art, path field,
   drag-and-drop through winit's file-drop event, automatic detection of
   volumes and conventional folders, progress, error text, Pref re-import,
   remembered source. No new runtime dependency; a native file dialog is
   deferred until drag-and-drop has been tried by players.
3. **Packages.** CI job producing MSI, DMG and AppImage plus tar.gz from one
   tag, unsigned, each scanned by `tools/check_assets.py` before upload.
   Signing is a later, separate change.

Decisions recorded 2026-09-21 (John): mounted disc or copied folder, no raw
ISO reader; proper installers rather than portable archives; unsigned first
builds.

#### Implementation plan (2026-09-22)

Execution plan for the three slices, written after a code survey. Behaviour
stays in the [spec](spec/first-run-import.md); this section owns sequencing,
ownership and the agent decisions that the survey forced. Every type and file
name below is an agent design choice, not an existing API.

What the survey established:

- The importer's only file lookups are one case-insensitive `archive()` helper
  and two inline `FA.EXE` scans in `tore-app/src/assets.rs`. One `MediaSource`
  replaces all three.
- `Archive` opens a whole file from offset zero. The four LIB archives are
  stored uncompressed inside `SETUP.ESA`, so a base-offset constructor lets the
  existing EALIB reader serve a disc without copying 108 MB.
- Three table readers gate on the 1.02F hash; `radio::phrases` has no gate and
  would read wrong addresses from the 1.0 build. All four move to one build
  table with two address sets.
- Every game object in `tore-app/src/main.rs` is built from imported assets
  before the window opens, and the renderer needs a terrain world. A first-run
  screen therefore cannot live inside the game `App`.
- There is no text field, no file-drop handler, no clipboard and no release or
  packaging job anywhere in the repository.

Agent decisions, recorded as agent decisions:

- **Pre-game shell.** The locate screen is a separate winit application
  handler run with `run_app_on_demand` before the game app on the same event
  loop, presenting a 640 × 480 CPU canvas through a minimal blit-only wgpu
  path. Re-import from Pref ends the game app with a re-import outcome and the
  shell runs again. `EventLoop::new` is never called twice.
- **Art before import.** Before any import exists there is no retail art on
  disk, so the locate screen uses the bundled menu font and flat panels from
  `menu.rs`. When a pack already exists (re-import, or a stale cache) the same
  screen draws over the retail Choose Activity background. The spec's "original
  menu art" is amended to say this.
- **Text entry without a clipboard.** The path field accepts typed characters,
  Backspace, Delete, Home, End and Enter. There is no paste; drag-and-drop and
  automatic detection cover the common cases. A clipboard dependency is not
  added.
- **Import progress.** `Assets::import` gains a progress callback and runs on a
  worker thread; the shell polls a channel and redraws. The import result and
  report text are unchanged apart from the build line.
- **Build identity.** `tore-formats::executable::identify` maps a hash to a
  `Build` with per-build addresses. Unknown hashes are refused before any
  archive is read. The report line becomes `FA.EXE: 1.02F` or `FA.EXE: 1.0
  (disc)` followed by the hash.
- **Packages.** A tag-triggered release workflow builds `--release` on the four
  CI images, stages a bundle directory per platform, runs
  `tools/check_assets.py` on the staged directory and on the finished package,
  then uploads unsigned MSI (WiX from the runner image), DMG (`hdiutil`),
  AppImage (`appimagetool` pinned by hash) and tar.gz. No Rust dependency and no
  signing.

Work packages and file ownership:

| Package | Owns | Depends on |
| --- | --- | --- |
| A: ESA reader and CLI | `tore-formats/src/esa.rs`, `Archive` base-offset constructor in `tore-formats/src/lib.rs`, `tore-extract/src/main.rs`, `docs/EXTRACTION.md`, `docs/formats/coverage.md` | none |
| B: executable builds | `tore-formats/src/executable.rs`, `ui/creator.rs`, `ui/fingerprint.rs`, `weather/clouds.rs`, `weather/flare.rs`, `radio.rs`, `docs/formats/esa-installer.md` implementation notes | none |
| C: media source and importer | `tore-app/src/media_source.rs`, `tore-app/src/assets.rs`, `--import` handling in `main.rs` startup only | A, B |
| D: packaging | `.github/workflows/release.yml`, `tools/package/*`, `docs/DEVELOPMENT.md` packaging section | none |
| E: locate screen state and drawing | `tore-app/src/locate.rs` (state, key handling, layout, headless snapshot test), Pref entry in `tore-app/src/menu.rs` | none, integrates in F |
| F: shell integration | `main.rs` (pre-game shell, file drop, re-import outcome, remembered source), `tore-app/src/canvas_present.rs`, `docs/spec/first-run-import.md`, `docs/features.md`, `docs/ARCHITECTURE.md` | C, E |

A, B and D run together; C and E follow; F closes. Each package runs the
repository checks on its own files before handing back; F runs the full set
plus the display smoke test and a real first run against the local disc 1
folder and the installed folder, with `TORE_DATA_DIR` pointed at an empty
directory. Windows and macOS runtime validation is recorded as pending until a
package is installed on each.

Current (2026-09-22): slices 1 to 3 are implemented. The media source, the
ESA container reader, both executable builds, the locate screen, the pre-game
shell with drag-and-drop and Pref re-import, and the four release packages are
in the tree and validated on Linux. The release workflow has built the MSI,
both DMGs, the AppImage and the tar.gz on a `release-test` branch, and every
package passed the asset guard. Installing and running them on Windows and
macOS is pending.

Deliverable: on each platform, install, choose a mounted disc 1 or an
installed folder, and fly the README free-flight check without using a
terminal.

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

## Cannon cadence correction

Requested by John on 2026-09-21 and implemented with individual physical rounds
at the fitted source-derived host rate, plus one luminous tracer every three
bullets. Ammunition, target damage budgets, trigger boundaries and rendering are
validated together. Actor-owned release mechanics use the same shot spacing
without changing autonomous decisions. [Rules and known damage-rounding difference](spec/damage-smoke.md#individual-cannon-rounds);
[validation](baselines/damage-smoke.md).

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
| ~~Retail AI VM reimplemented or behaviors recreated~~ | Settled 2026-09-15: behaviors recreated from a spec |
| Working title | Whenever, before M1 tag |
| TS repo stays runnable as reference or is archived at M0 | M0 |
| Multiplayer in 1.0 or after | Before M2 tag |
| Initial aircraft scope | Expanded to the twelve ported aircraft listed in M1c |
| Save format and mod manifest schema | M3 |
| Installer scope: mounted disc or folder, proper installers, unsigned first builds | Settled 2026-09-21, see M1g |
