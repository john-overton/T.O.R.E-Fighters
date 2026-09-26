# Initial architecture

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The M0 environment supports the M1a menu slice, the M1b renderer across all 16 theaters, and M1c free flight in twelve aircraft (see the [roster guide](aircraft-import.md)) plus a development weapons range. M0's full title census, salvage inventory, parity specification, and AI VM decision remain open.

| Component | Choice | Purpose |
| --- | --- | --- |
| Language | Rust 2024, compiler 1.91.1 | Reproducible native builds |
| Workspace | `crates/tore-app`, `tore-formats`, `tore-extract`, `tore-sim`, `tore-input`, `tore-input-native`, `tore-diagnostics-native`, `tore-replay` | Desktop shell and entry point, plus the format, extraction, simulation, input and mission recording crates |
| Window/input | `winit` 0.30 | Native window lifecycle and input |
| Graphics | `wgpu` 27 | Metal on macOS; native backends for Windows/Linux |
| Diagnostic facade | Existing `log` 0.4 and `tracing` 0.1 | Bounded app/backend logs without a logging framework |
| Startup bridge | `pollster` 0.4 | Wait for GPU initialization without a general async runtime |
| Audio device | `cpal` 0.16 | Native output for the small PCM mixer; [upstream API](https://docs.rs/cpal/0.16.0/cpal/) |
| Formats | Dependency-free `crates/tore-formats` | Bounded EALIB, raw-literal DCL, PIC/glyphs, a narrow CHOOSEAC DLG reader, BIT2, mission environment fields PL weather palettes, BRF aircraft/equipment, bounded SH projection and compiled FNT glyphs |
| Simulation | `crates/tore-sim` | Shared 120 Hz state/attitude, selectable hybrid dynamics, the shared aircraft sensor component and headless aircraft acceptance |
| Extraction | `crates/tore-extract` + `tools/extract_assets.py` | Title-independent archive discovery/extraction, safe output paths, provenance |
| Checks | Cargo, Python standard library, GitHub Actions | Local and CI checks |

These are baseline-compatible versions, not automatically the latest versions. Exact versions live in `Cargo.lock`. See upstream [wgpu 27 documentation](https://docs.rs/wgpu/27.0.1/wgpu/) and [winit 0.30 documentation](https://docs.rs/winit/0.30.13/winit/) for API contracts.

The shell creates its window on the main event loop and uploads a 640 × 480 RGBA menu canvas to a Metal/native GPU texture. The shader draws a scaled, letterboxed image using linear filtering. The same viewport calculation maps physical mouse coordinates back into retail coordinates, including Retina scaling. It handles zero-sized windows and recoverable surface loss. Fatal graphics/startup errors return a failing process status.

`media_source.rs` decides what a chosen folder is by its contents: an installed game folder with loose archives, or a disc folder holding the Electronic Arts installer container (`SETUP.ESA`), whose stored archives are read in place by offset rather than copied. `tore-formats::executable` maps the executable's hash to one of the two reviewed builds and supplies that build's table addresses, so the disc's 1.0 build and the 1.02F patch decode the same content; an unknown build is refused before any archive is opened. `assets.rs` selectively decompresses resources into a versioned local pack and validates them before import completes, reporting progress by archive and resource count. `menu.rs` composites original background, button pieces, and font strips and owns UI state. `renderer.rs` owns the surface, texture, and scaling. Every game object is built from imported assets before the window opens and the renderer needs a terrain world, so a first run cannot happen inside the game application: `main.rs` runs a pre-game shell, a second winit application handler holding `locate.rs` and the blit-only presenter in `canvas_present.rs`, on the same event loop through `run_app_on_demand`. Re-import from Pref ends the game application and runs the shell again; the event loop is created once. `audio.rs` mixes bounded local PCM, recorded music, and up to sixteen spatial voices with linear resampling into the device rate. `tore-sim::acoustics` advances sound wavefronts and passing-object detection at 120 Hz. Combat publishes bounded sound emissions with physical positions; the app uses the main camera as listener and the device mixer supplies stereo direction, distance gain and treble fade. An emission released by the player's own aircraft plays centered and without distance while the listener is in that cockpit. See [audio behavior](audio.md). Audio initializes independently and can fail without blocking menus. No input device or microphone is opened.

The app and general extractor share the same EALIB/DCL readers. `Archive::open` reads a directory and seeks to selected resources; it does not load whole disc archives. The menu retains a 16 MiB resource cap; the general CLI has an explicit configurable cap for larger media. The Python entry point handles portable invocation and SHA-256 report enrichment; it contains no second decompressor.

Menu-only startup randomness chooses one of the five native backgrounds independently of future simulation state. The selected background's embedded palette colors shared sprites/fonts, and native bar offsets keep controls aligned. Hover/focus notifications do not enqueue audio.

Menu drawing uses CPU composition for this small static canvas; it is not a commitment to software-rendering flight scenes. The window sleeps while idle. Hover transitions and transient placeholder messages schedule temporary redraws. The fragment shader is authored source; it contains no retail bytes.


`assets.rs` retains prior cache generations until a new import is synced and
validated from disk. Successful import and startup load remove older numbered
packs from that cache directory. Cleanup is best effort and does not touch
source media or extracted assets. [Retention contract](spec/import-cache.md).

## Startup diagnostics

`diagnostics.rs` installs a bounded `log` sink and panic hook at the start of
`main`, before preferences, assets and the event loop. Existing wgpu logging
and winit tracing feed the same sink. Startup stages and first presentations
are explicit checkpoints; simulation behavior and adapter choice are unchanged.
`startup.rs` owns unattended-dialog policy and media-free success, failure,
panic and synthetic graphics checks. Main-thread unwinding produces a fatal
report and a nonzero exit rather than resuming gameplay.

`tore-diagnostics-native` is a second narrow audited OS boundary. Its safe API
shows Windows MessageBox/AppKit errors without creating the game renderer and
writes Windows Application events. Linux notifications use an optional bounded
`notify-send` process. The app and simulation retain their unsafe-code ban.
The MSI registers the source against the executable's embedded message table.
The existing platform bindings and `log`/`tracing` dependencies are reused;
there is no additional GUI or logging framework.

File paths, retention, fallbacks and failure limits have one home in the
[startup diagnostics contract](spec/startup-diagnostics.md). Release packages
exercise the actual staged and extracted executable; desktop installation
acceptance remains separate.

## Boundaries for menu work

Keep `tore-formats` independent of windowing and GPU APIs. The future simulation likewise needs to run headlessly with deterministic inputs. Avoid empty placeholder crates or premature engine abstractions.

Menu rendering should consume decoded palettes, indexed images, font data, and recovered layout geometry. Recover specifications from the TypeScript reference; implement runtime behavior in Rust. Do not introduce a web shell, copy the Three.js engine, or select a modern widget toolkit before checking retail geometry requirements.

Import user-owned media at runtime into platform application data. The v1 pack is a development cache of selected decompressed resources, not a stable mod/save format. `gameassets/` is a local source-media convenience, not a runtime bundle or save-data location. See [menu formats](formats/menu.md) for the current limits and provenance.

## First simulation renderer

World image pages share a bounded 1024-square array, with sixteen logical pages
per layer. `static_art.rs` retains large scenery pictures at source resolution
and clips face geometry at page boundaries. Material-local storage metadata
keeps rectangular aircraft/smoke images and paged world/weather art distinct.
All sample, palette-remap and shadow paths use the corresponding addressing.
[Source coverage and static variant composition](spec/terrain-detail.md).

`terrain.rs` constructs a world from the selected retail MM and its resolved T2, named or numbered texture references and DAY2 variant palette. Its camera and surface queries have no GPU/window dependency. `sim_renderer.rs` uploads geometry and a texture array, owns depth targets and draws terrain plus a fullscreen sky pass; `terrain.wgsl` supplies the initial perspective, sampling and fog. `renderer.rs` composes this scene with the transparent CPU HUD, resizing depth and surface together. This separation allows aircraft/object/weather passes and a deterministic simulation to be added without coupling format readers to wgpu.

The initial implementation uses full-resolution fixed triangles and an authored sky/fog projection. It is not the native adaptive renderer. All geometry/colors come from local source data at runtime; no retail derivatives are embedded. See [theater findings](formats/theater.md) for recovered versus authored behavior. The Hornet adapter now advances at 120 fixed ticks/second; the developer free camera still uses elapsed wall time for inspection.

The creator selects among all 16 base theaters. Scene replacement rebuilds the GPU vertex/texture buffers for that world; a variable texture-array layer count also supplies the sky shader's layer index. Only the active world mesh is built, while the bounded source bundle remains cached. Maps and fonts stay in the menu compositor. Source text shading is preserved when tinting; ARMFont/SMLFONT replace the unsuitable BODYFONT in the investigation UI and notices.

The Hornet slice adds dependency resolution and bounded BRF/SH/FNT readers to `tore-formats`. `tore-sim::flight` contains fixed-tick state/integration without wgpu/winit dependencies; the app re-exports its interface; `aircraft.rs` adapts imported geometry and camera poses. `instruments.rs` renders independent small rasters from flight/equipment state, each framed by the aircraft's own original instrument window picture (named by its HUD) and coloured every frame through the live cockpit palette, as the cockpit art is; page content draws in coordinates relative to the 138×114 screen ([bezel spec](spec/instrument-bezel.md)). The GPU terrain pass now accepts an aircraft vertex stream and original rectangular atlas with shared depth; front/other instrument cameras render offscreen. CLI extraction and cache import share the same dependency resolver. These adapters do not execute imported x86 modules. See [aircraft evidence and open questions](formats/aircraft.md).

`surface_lighting.rs` owns three geometric shadow maps and the shared light
uniform. `surface_lighting.wgsl` supplies continuous diffuse response, solar
warmth, orientation-dependent fill, painted-panel highlights, cloud transmission
and shadow sampling to terrain, aircraft and weapon
surfaces; sky/cloud shaders share its solar tint helper. Opaque batches enter
the shadow pass before the world pass. Partial sunrise/sunset uses the solid
disc's visible area and segment centroid for direct light and shadow direction.
Manually filtered depth neighbors each use receiver-plane correction and
continuous blocker weights. The bounded penumbra filter combines caster
distance with smooth camera-distance antialiasing. Terrain has a separate
stream of area-weighted shared normals for lighting; shadow depth still uses
actual triangles. Smooth sky, glare and shadow directions share fractional
weather-clock time. Sun-disc strength and geometric visibility are independent
of the sunglare toggle. Terrain alone reduces low-sun ambient fill on unexposed
slopes; exposed ridges, aircraft and water retain their existing response. Smooth aircraft geometry is complete,
including faces hidden from the camera, and hiding the player in cockpit view
does not remove its shadow caster. Material layer -6 identifies emissive combat
effects and -7 identifies textured flame sheets. Glass and flame sheets do not
cast solid shadows; ordinary cutout surfaces cast only their opaque texels. Stepped
mode retains palette lighting and camera face rejection. See the
[surface specification](spec/surface-lighting.md) for fitted limits.



`flight_views.rs` resolves manual-described camera relations from read-only flight,
combat and wing snapshots. The camera rig owns only presentation state: the
reference, last player missile, fixed fly-by position and saved Other View.
Weather, spatial audio, main rendering and the Other View panel use the same
camera rules. Remote aircraft/missile interior cameras hide the reference body without removing
it from simulation. A replay builds the same scene from recorded poses, and any
aircraft can be the reference. [Behavior and fitted constants](spec/flight-views.md).

`flight_ui.rs` owns desktop command dispatch, imported menu navigation, session presentation settings and pause state. `hud.rs` draws the forward-flight HUD from state and source font glyphs, projecting the ladder/path through the renderer's 60-degree camera convention. Simulation remains independent of both. The full-canvas cockpit is transparent art over the world; instrument windows are independent rasters. Menu/focus pauses stop fixed ticks and engine loops, and input transitions clear held controls. Shader zoom is shared by terrain and sky projection; camera previews restore the main camera before drawing.


`flight_canvas.rs` now composes the flight-only overlay at an aspect-responsive size (physical drawable, proportionally capped at 1920×1080). The separate GPU cockpit pass preserves uniform cover-fit in the centered forward view, and instrument layout rectangles anchor to actual edges. Native instrument rasters go directly to their destination sizes instead of passing through a reduced 640×480 composite. The original cockpit texture is uploaded once; unchanged scaled panel rasters are cached. Alpha-aware filtering prevents dark transparent borders. `renderer.rs` recreates its UI texture when dimensions change and uses the full viewport for flight; menus/viewer overlays retain their existing canvas. Pointer conversion uses the same responsive panel rectangles, while the centered pause menu retains menu coordinates. HUD metadata uses a 0.7225 layout scale, including the requested additional 15% reduction. Projection compensation preserves angular cues through resizing and portrait aspect.

`tore_sim::autopilot` owns captured heading/altitude, mode and an optional
world-space navigation target. `State::step_surface` consumes mode switches,
applies pilot override and generates control deflections before the selected
flight adapter runs. The HUD reads this state. See the
[autopilot specification](spec/autopilot.md).

### Flight presentation and measurement

`flight::State` remains authoritative at 120 Hz. `main` retains the preceding tick for render-only pose interpolation (shortest-path wrapped angles); pause/crash show authoritative state and restart resets history. Camera, exterior geometry and HUD consume the same presented pose. Everything else combat draws is captured once per tick, after the AI step, as a plain-data `RenderSnapshot` (`render_snapshot.rs`): other aircraft with their devices, damage and wreck state, fixtures, weapons, effects, debris and ejected pilots, plus the player's own pose. Combat keeps the last two snapshots, and the main view, mirrors and camera panels draw their blend at the same fraction through the shared `aircraft_batches` and `combat_geometry` helpers, which a mission replay uses to draw a recording, so both show the same picture. Chaff and flares are not part of the snapshot: the countermeasure renderer draws them from the combat state, and afterburner flame lights sit at the presented poses. `replay/convert.rs` turns each snapshot, plus flight data it does not carry, into a `tore-replay` frame and a decoded frame back into a snapshot; a synthetic round-trip test draws both through the shared helpers and requires every vertex within the format's 1/64 ft position precision. Reset clears that history. AI gear, flap, hook, brake, bay, exhaust and control-surface samples use that
same render fraction; an aircraft whose AI stopped flying holds its last
devices, and fixtures retain their fixed devices. Audio consumes
authoritative state. No renderer smoothing feeds back into physics.

World camera depth uses `Depth32Float`, storing the near plane at depth 1 and
the far plane at depth 0. Using the near and far distances, the vertex shader computes clip Z as
`near * (far - view_z) / (far - near)`, then perspective division yields depth.
Depth clears to zero and nearer fragments compare greater. The existing near
plane and 2,200,000-foot far limit remain. Aircraft, terrain, static objects,
clouds, smoke, vapor, chaff, flares, flare glare and the spotting aid use
this same mapping. The separate
orthographic shadow maps keep their existing depth convention. This is an
agent-selected host correction for distant surface flicker; see
[NVIDIA's depth-precision analysis](https://developer.nvidia.com/blog/visualizing-depth-precision/).

Active simulation views request the next redraw without a post-render timer; AutoVsync and a requested maximum frame latency of one provide presentation backpressure. Idle menu behavior is unchanged. Failed/zero-size presentation does not continually schedule simulation redraws. `performance.rs` provides opt-in bounded CPU wall-time sampling via environment variables, with warmup exclusion and view cycling.

The GPU cockpit texture survives view and size changes; projection uniforms track the current aspect. Aircraft GPU resources are prepared when the renderer/theater loads. Live camera panels submit bounded asynchronous readbacks (at most one pending per camera page), consume completed rasters on later frames, and retain their last image while pending. The simulation renderer caches world-pass attachments (depth, multisampled colour and the render-scale image) for up to five output sizes, so the display, mirrors and 138×114 panels do not reallocate at every refresh. World pipelines live together in `sim_renderer::Pipelines` and are rebuilt when the anti-aliasing sample count changes. After the world pass, an optional resample pass scales the render-scale image to the output, and the spotting-aid pass draws single-sampled outlines using the world depth; `graphics.rs` holds the options and the [graphics options](spec/graphics-options.md) page specifies them. Offline captures retain an explicit blocking readback so smoke evidence contains the requested image. Direct GPU panel composition, full GPU UI rendering and native terrain LOD remain future optimization work.

`look.rs` owns authored held-key classification, look limits and exterior spherical orbit. Shift arrows are isolated from flight input and retain their look classification until physical release, including after modifier changes; Ctrl arrows, FA's thrust vectoring, do nothing yet. Camera motion uses elapsed presentation time while unpaused. Internal elevation is limited to the forward eye line through overhead; exterior orbit keeps a fixed radius and aims at the same interpolated aircraft pose. It does not alter simulation state or synchronously capture the GPU.

### Continuous attitude and momentum

`attitude.rs` supplies body bases, Rodrigues rotation, orthonormalization, and render interpolation. `flight::State` retains separate world velocity and pitch/roll response rates. The old pitch clamp and nose-derived position update are removed. Force integration and attitude response remain deterministic at 120 Hz; render interpolation does not feed back. `look` uses the same body basis for head rotation, while exterior orbit retains aircraft-centered inspection behavior. The HUD flight-path marker projects actual velocity bearing/elevation.

`cockpit_renderer.rs` and `cockpit.wgsl` project the complete original forward artwork and the 640×480 HUD raster through one aircraft-fixed plane. Relative eye/body axes move both layers together during head-look, independently of aircraft attitude. The pass draws after the world and before screen-anchored instruments/menus, including offline captures; camera instrument readbacks exclude it. A cached source-size glass mask follows the cockpit transform and clips the HUD when cockpit art is visible. The shader composites masked HUD behind cockpit art and mirror contents; cockpit-off/wide view bypasses the glass mask. Linear premultiplied sampling preserves transparent borders. HUD uploads and uniforms are nonblocking; resize does not rebuild the source texture. The sky shader projects SKY0 onto a finite hemisphere disk with `ray.xz / (1 + max(ray.y, 0))`, avoiding the latitude/longitude singularity and seam at zenith. Native sky mapping and full 3D cockpit geometry remain unimplemented; the projected source plane has finite coverage and cannot provide a rear/overhead interior.

The shared hybrid adapter and its recovered-versus-fitted boundaries are specified in [FLIGHT-MODEL.md](FLIGHT-MODEL.md). Scalars resolve once when constructing the aircraft-owned typed configuration; immutable model data are shared by presentation snapshots and envelope queries allocate no temporary hit vectors. Renderer-specific Hornet animation tests remain in the app.

Aircraft-specific fitted laws live in `tore-sim::models::{f18,rafale_c}`, selected
through `AircraftModel` and the `FlightModel` interface. Each owns independent
validated configuration covering mass, propulsion, envelopes, recovered departure/contact limits, equipment response and tuning. `State` holds the selected model; `Research` holds only evolving departure/contact/clock state. Updates take no raw aircraft argument. Integration and recovered algorithms are shared components.
`tore-sim::telemetry` exposes gauge-independent air/ground/altitude channels with
explicit units and unavailable sensor readings; analog gauge presentation must
remain downstream of this interface. See [model extension guide](FLIGHT-MODEL.md).

## Shared physical input

`tore-input` is a dependency-free safe Rust crate for physical control bindings,
calibration, per-source contribution/ownership, context release rules, typed pilot
frames and bounded input tapes. `tore-sim` consumes those frames at 120 Hz and no
longer interprets keyboard names. Aircraft configuration and control response stay
in the respective model modules. The app translates keyboard/menu actions and
routes instrument focus to the existing stock controls.

`tore-input-native` owns a dedicated bounded device worker: Linux evdev/rumble,
Windows raw-controller readings/Gamepad vibration, and macOS GameController/
CoreHaptics plus generic HID queues. Apple gamepad input and haptics share the same
retained controller; public HID support queries suppress duplicate raw endpoints. It and the diagnostics boundary
permit audited unsafe platform FFI; `libc` and `windows` are thin platform bindings,
not a third-party input policy engine. Main-loop discovery never blocks a flight
frame. Presentation-only look resolution cannot change pilot-axis ownership.
It also runs the head-tracker receiver: a safe `std::net` loopback UDP socket on
its own thread that reads opentrack poses. Head and mouse look add to the
presented look angle only; they never touch pilot axes or the simulation.
See [contracts, platform limits and profile syntax](INPUT.md).

The app's input configuration screen (`controls_editor`) is one component opened
from the main menu and the paused flight menu. It owns a draft
`tore-input::Profile`; native capture is isolated from menu/gameplay dispatch.
`input_catalog` is the single table of listed actions and stock keyboard/mouse
assignments; it drives the screen's rows, stock-key remapping (`disable`
directives) and the generated [controls master list](CONTROLS.md). Canonical serialization validates
before file replacement and live rebaselining. General display/instrument/sound
preferences use a separate bounded versioned file and persist independently of
flight/aircraft state. Smoke/capture/performance diagnostics bypass those preferences.
Afterburner feedback combines a renewable finite low rumble with the engagement
impulse; context loss cancels both, without changing authoritative flight state.

## Manual combat and systems

`tore-sim::combat::live` owns the deterministic manual range at 120 Hz. Typed
configuration resolves each supported PT’s loadout, SEE/ECM equipment and damage
table once; mutable ammo, projectiles, player HP, subsystem counts and
adapter RNG remain in state. Contacts are no longer its own: it holds a
`tore-sim::sensors` live state, feeds it ownship pose, equipment state and the
observable targets each tick, and asks it whether a specific target is supported
before a radar weapon launches. Physical airborne presence is separate from
destroyed combat state, so hit points reaching zero does not erase a return.
`combat::systems` contains bounded translations of
reviewed ECM probability and damage-selection helpers. Unknown native subsystem
side effects remain explicit gaps; the service reproduces combat behaviour rather
than reconstructing the original executable's combat tick.

`combat::ledger` records every projectile from its first step to its outcome
(hit with damage, missed, spoofed by a decoy, jammed), keyed by shooter,
intended target and retail weapon class, plus credited kills and each target's
last attacker. Nothing in flight reads it. The app's `debrief.rs` turns it into
the post-mission pages; `ai_wings.rs` supplies the aim of AI gun rounds and
reports decoyed missiles. See the [debrief spec](spec/debrief.md).

`combat::gunsight` supplies a renderer-independent fixed-step gun solution using
live projectile speed/drop helpers and current radar observations. `weapon_hud`
draws its pipper/range arc and projects a selected target into a square or edge
chevron. Combat retains a separate display-only target identity through sensor
loss; this never substitutes for `sensors` launch support or radar observations.
[Behavior and evidence](spec/gunsight-targeting.md).

The app merges independent keyboard/controller trigger holds, dispatches explicit
commands and consumes confirmed events for instruments, graphics, audio and the
bounded feedback mixer. Two-control modifier bindings consume their base controls
and require neutral release across layer/context changes. Native feedback errors
disable that device’s feedback until reconnect; finite leases and context stops
bound rumble. Combat notices reuse the existing HUD line and expire in simulation
ticks, so pause does not age them or expand overlay composition work.

Version-3 combat tapes record service inputs including jammer state, the player
sensor controls (channel, display range and history) and explicit fixture
commands; a designation is recorded by stable target identity, never by screen
coordinate. Replays validate identity/assets and reproduce combat state,
including adapter RNG and subsystem failures. Version-2 tapes still replay with
the default sensor controls; version 1 rejects explicitly. This
is combat-service determinism: it reproduces combat state, not a full application
replay. See [contracts, validation and limitations](baselines/weapons-systems.md).

## Additional aircraft

The aircraft registry includes F-14D, A-4E, X-31 EFM and the seven
[roster additions](spec/roster-aircraft.md). Each owns a typed
model configuration; presentation rigs remain in tore-app. Shared combat resolves
the selected identity's sensors by parsed record channel, never by aircraft name,
and reads its PT stations. Audio switching clears old
aircraft voices. See [aircraft behavior](spec/additional-aircraft.md).

The optional user-supplied [engine material](spec/engine-material.md) replaces reviewed burner
face materials at runtime. Its throttle glow is separate from the retail atlas
and does not change A-4E presentation or flame geometry.

Researched flight is now the default; `--legacy-flight` preserves the previous
model. HUD and audio share the [stall warning signal](spec/stall-warnings.md),
including the original imported warning samples.

Aircraft-owned exterior fits for the seven roster additions live in
`roster_animation.rs`, with reviewed source branch membership in
`additional_animation.rs`. The [animation contract](spec/aircraft-animation.md)
defines their presentation. F-22 bay fraction advances at the shared 120 Hz;
the renderer interpolates it, and manual combat can request it without changing
launch eligibility. Exterior canopy grading is a mesh material, independent
of cockpit artwork and world-view rendering. Its nearest surface is resolved
in a depth-only pass, then blended at 75% opacity over the opaque scene.

## Shared sensor boundary

`tore-sim::sensors` is one component serving all twelve imported aircraft. There
is no aircraft-specific radar code: `profile` normalizes each PT's own SEE and
ECM records into typed capability profiles resolved by parsed signature channel,
`signature` holds the single observer-relative aspect function, `detection` holds
the authored range model, `track` holds the live state (current observations, one
selected target, at most one acquired fire-control track across radar and
infrared, bounded history and received interference), and `passive` collects
received emitters for the exposure instrument. An unreviewed radar or ECM record
is an import error rather than a silent substitution.

The component decides what is observable and whether a specific target is
supported for a radar weapon. `tore-app::scope` only reprojects those shared
observations for drawing and picking, so the page cannot make a hidden target
selectable, and one projection serves both drawing and the mouse pick.
`instruments.rs` draws pages 9 and 0 from that input. Player controls (channel,
display range, history) travel as a per-tick input, which is why the combat tape
reproduces them. The flight adapters and renderer independence are unchanged.
[What is modelled, what is authored tuning and what is deferred](radar.md);
[what was validated](baselines/radar.md).

Missile profiles and the fitted finite-boost motion predictor live in
`combat::missiles`, independent of rendering. Its 120 Hz prediction shares live
lead steering, turn limits, maneuver losses and propulsion. The live adapter
refreshes the bounded maximum-range search at 2 Hz per selected observed target. The live adapter uses full release
velocity for accepted missile profiles; compatibility retains scalar source
motion. Explicit target role is separate from damage category. Launch-origin
minimum-range qualification persists through terminal closure, as specified in
[engagement rules](spec/missiles.md#minimum-engagement-and-target-role). Combat tape version 4 includes world velocity and bay permission, while versions 2 and 3
select compatibility rules. [Missile specification](spec/missiles.md).

Combat tape version 5 records radar power separately from transmission, preserving
passive-channel behavior and radar-off unguided releases. Versions before 5 imply
power on for the new guidance gate. Version 4 remains the missile rule-version boundary. Seeker mode and
controlled target heat/emission changes are recorded commands; seeker observations
and acquisition are reproduced from those inputs, the matching asset fingerprint
and terrain. `--combat-command compatibility-weapons` explicitly selects the old
weapon adapter. Flight adapter selection is independent. Fitted seeker synthesis
consumes the mounted-seeker amplitude and never controls acquisition.

Quick Mission launches AI wings by default; `--fixture-wings` retains the
straight-flight compatibility path. `Combat::mission_aircraft` retains the six
wing groups and their fitted spawn poses for restart. `AiWings` builds actors
after reset. AI emits aircraft inputs and the flight model alone advances
its pose; no steering pose overwrite follows physics. The bridge mirrors results into `combat::live::Target` for sensors,
rendering and damage. Each wing follows its own leader from the shared world
snapshot; the human leader remains outside the AI actor list.
`ai::awareness` owns timestamped current observations and frozen aircraft memory
for each actor. Only current observations enter target selection, weapon geometry
and firing; lost hostile records enter a separate controller search input. The
AI visual cone is skill-filtered independently of imported player sensor
profiles, and the mission terrain query masks visual and radar/infrared sensing.
Production sensor loading fails explicitly rather than enabling the sensorless
synthetic-fixture path. SEARCHING/ACQUIRING/REJOINING are simulation activities
read by Target view. [Behavior and limitations](spec/ai-awareness.md).
`combat::threats` owns bounded missile observations for every receiver, including
the player RWR. Controllers consume copied threat records through `ai::defense`;
they cannot inspect live missiles or private launcher targets. `ActorSupport`
snapshots bind each guided projectile to its own launcher, and the shared
seeker lifecycle supplies actual pitbull and support-loss state. Countermeasure
bursts are scheduled/debited by the mission before the bridge applies decoy
rolls. RWR drawing reads the same records and never drives the decision clock.
`ai::engagement` gates current target selection by role and stance before B41
ranking. Quick Mission initializes a separate neutral engagement gate for every
actor. Accepted combat orders release it; formation and disengage commands
recall it without rewriting mission objectives. Recall suppresses repeated
offensive reactions to known projectile IDs while retaining missile evasion.
AI leaders release their own wings only after a perceived attack, with command
delivery after all same-tick decisions. `AiMission` delivers perception-only attack reports to assigned escorts and wing leaders
on the next tick, with a fixed expiry; bearings never become synthetic targets.
Escorts also assess detected aircraft against each assigned friendly's protection
zone using observed relative motion. Confirmed attackers outrank prospective
threats. Frozen contacts can guide investigation but still cannot authorize fire.
Quick Mission group objectives resolve through stable side/wing/member metadata
into per-actor assignments. Whole-group survival requirements are stored separately from combat orders.
The Target window combines player assignments, these requirements and allegiance
to select a typed Survive/Destroy label, with no label for other contacts. Source missions and
campaign outcomes remain outside the M1 assignment adapter.
`ai::formation` owns routine repositioning, trailing, breakout, intercept, stabilization and
capture guidance. Traffic and arrival states are snapshotted before any actor
advances, so iteration order cannot grant approach priority. Its trace hook is
read-only; optional host CSV logging is described in
[development diagnostics](DEVELOPMENT.md#formation-flight-traces). The formation
choice and placement are specified in [mission wings](spec/quick-mission-menu.md#mission-wings).
Rendering groups targets by imported airframe, with a cached
texture binding per identity and vertex buffers that grow to fit the formation.
The player's atlas is never substituted for another type. Fixed-step timing and
the three player flight adapters are unchanged. Normal flight loads supported default stores;
restricted native research flight keeps its clean configuration.

`combat::smoke` owns bounded, fixed-step puff histories independently of rendering
and guidance. The app supplies engine outlet positions once per combat tick;
contrails retain two minutes of history in a separate 72,000-puff budget.
[Smoke and contrail rules](spec/damage-smoke.md) define rates, size and fade.
The app's smoke pass retains original indexed smoke artwork and resolves it
through each camera's current weather palette and haze remaps. Clouds and smoke
share directional sunset lighting and air/cloud occlusion. It sorts keyed
billboards for each camera, blends them without depth writes, and depth-tests
against the world. It culls offscreen puffs without deleting their history,
then uploads one 24-byte instance per visible puff; the vertex shader builds
its six billboard vertices. Age and expiry remain fixed-step CPU state.
Coverage is premultiplied only after lighting to preserve transparent edges.
Gun release dispersion is sampled once in the shared fixed-step projectile
path using a stable projectile-identity hash. Cannons release individual physical
bullets at a fixed-step cadence; every third bullet draws one tracer ribbon,
without a duplicate projectile mesh. Ribbons follow the actual swept segments. A separate additive material pass supplies self-lit cores and halos
without depth writes or shadow casting; solid depth and atmospheric attenuation
still obscure them. [Gun behavior](spec/damage-smoke.md#gun-dispersion-and-luminous-tracers).
Aircraft damage keeps six aircraft-local accumulators beside the existing hit
points. Gun segments intersect fitted nose, cockpit, core, wing and tail volumes;
surface objects remain on their ordinary object damage path. `damage_art` packs
intact and damaged PICs losslessly into one runtime atlas per identity. It adds
persistent imported-texture marks at light regional damage, then selects a
reviewed A/C body only when its missing region matches the hit. Other regions
use fitted, mirrored face clipping. Damaged shapes bypass intact-model animation
address maps.

`combat::debris` computes both fitted A/B and C/D attachments from inert shape bounds and advances
pieces at 120 Hz with inherited velocity, gravity and tumble. First swept terrain
contact retires a piece and creates a short visual ground impact. The runtime
atlas also contains the matching fragment textures. Fragment state belongs to
combat simulation, so replay and rendering consume the same breakup lifecycle.

Quick Mission's `ai_wings` bridge mirrors live actor poses and damage, while
combat retains movement ownership of wrecks. Each AI projectile carries its
own weapon record; rendering resolves its shape by resource name, independent
of the player's station numbering. The simulation owns delayed warnings,
individual dispenser releases and scoped wing requests. The app realizes
those events in the existing projectile and effect services. See the
[AI integration contract](spec/ai.md#live-integration-and-authored-boundaries).

Wing commands use `AiMission::order_wing_report` for scoped per-recipient
outcomes. The player bridge resolves orders and applies B43 control effects;
AI requests use the same B46 receiver after all actors decide. `ai_wings/orders`
and `reports` keep delivery and advisory text separate from physical steering.
`audio::Mixer` owns a bounded serial radio queue, independent of simulation
execution. `tore-formats::radio` reads reviewed inert phrase records during
import; the existing archive and PCM readers load original recordings.
Radio chatter is observation only: `ai_wings/chatter` turns mission output
into events, combat's strike list attributes projectile damage, and
`radio_calls` words them and applies the listener rule before `comms` delivers
them. [Radio chatter](spec/radio-chatter.md#implementation-in-tore),
[command behavior](spec/ai.md#live-wing-command-and-radio-integration),
[radio data](formats/radio.md), [controls](INPUT.md#player-wing-orders).

Routine formation transitions share previous-tick velocity intentions through
the immutable traffic snapshot. Each actor owns its staged route and yielding
position; no coordinator moves aircraft directly. Live random slot offsets are
smoothed in `Controller`, with vertical amplitude reduced to five feet. The
[transition specification](spec/ai.md#normal-formation-variation-and-transitions)
owns the fitted parameters and limits.

Smooth lens flare is a continuous optical-light composite over the completed
world, so the sky gradient is not quantized inside flare circles. Stepped mode
keeps indexed remapping. Water's solar glint tests geometric light visibility
before adding reflected sunlight; ordinary water shadow tint is a separate
operation. See [glare](spec/sun-glow.md#continuous-lens-flare-composition) and
[water reflection](spec/ocean.md#separate-sun-and-environment-reflection-trial).

## Airport scenes

Airport scenes are immutable imported data owned by `terrain::World`. Static
GPU geometry is batched by placement and filtered each frame from combat-owned
target HP, so destroyed objects disappear consistently in main and mirror
views; a replay filters by its recorded destroyed objects instead. The airport
service derives availability from those combat targets and
owns player selection, clearance, landing progress, and typed replies.
Weather-only reconstruction preserves service and combat state. Theater changes
and flight restart rebuild both from the imported scene.

`tore-sim::ai::airfield` owns AI takeoff and landing sequences. The app resolves
STRIP anchors and home runways, then passes terrain and landable surfaces to
`AiMission::step_with_surface`. Aircraft motion remains driven by pilot inputs;
the renderer has no role in traffic gates or landing decisions.
[Behavior and fitted safety rules](spec/ai-airfield.md).
`airfield_radio` observes those states and player runway context without changing
flight. It coalesces status by actor and uses the shared `comms` channel, with
separate airport audio ownership for cancellation. Quick Mission resolves a
validated queue along the final taxiway legs before any actor is created.

The creator stores its accepted ground-start runway identity separately from the
editable draft. It constructs the existing airborne wing launch reference first,
then initializes the player's whole wing on the shared runway surface. Restart
reuses the accepted launch layout, airport, fuel and stores. Building height
inside a composite runway shape never supplies the support-plane elevation.
Terrain render triangles are split at airport footprint edges and recessed
below the fixed support plane, with perimeter walls. Source terrain and physics
queries are unchanged. Static solid and texture passes use ordered equal-depth
tests without depth bias, so they cannot pull pavement in front of aircraft. A per-shape vertical normalization
aligns the dominant horizontal paving layer with the placement's runway plane;
building height does not move that plane.

`runway_wind` classifies imported MTOW and decomposes wind relative to a supplied
heading. Hybrid aerodynamics consume full wind; wheel physics consumes the bounded
crosswind/tailwind difficulty fraction as a tire-grip adjustment;
HUD departure/ILS cues consume its unscaled assessment using the active runway
end. It leaves airborne wind, flight-adapter selection and clearance logic
independent. [Rules](spec/runway-wind.md).

Hybrid takeoff combines an airflow-dependent device-drag factor, imported flap
lift, and continuous low-speed lift allowance. Contact receives remaining wheel
load from world-vertical support forces, classifies touchdown before changing
velocity, then resolves post-tire movement or wheel release. This keeps parked
stability separate from aerodynamic airflow and avoids discarding small climbs.
[Takeoff/contact behavior](spec/takeoff-ground-contact.md).

The shared [HUD layout and startup rules](spec/hud-layout.md) keep projected
flight and targeting geometry separate from fixed readouts. NAV presentation
hides weapon-specific symbols while retaining the selected target cue. Player
startup applies gun/SAFE selection after loadout reset, with ground NAV chosen
by the application; explicit diagnostic overrides are resolved afterward.

Airport guidance receives the aircraft body-forward vector from live flight or
recorded launcher pose. It filters threshold candidates through the shared
90-degree forward cone and range/airport-altitude band before returning either
armed or active ILS data. Head-look is not an input, and replay needs no new
wire field. [Arming contract](spec/airports.md#ils-arming-envelope).

`flight_map.rs` draws the Shift-M map as an opaque flight overlay. Simulation
map observations combine the active sensor and visual returns, including surface
objects, without adding surface targets to air-to-air selection. The renderer
receives observed positions and identification flags. Right-side map buttons
filter only presentation. An explicit structural-resource allowlist hides
buildings by default, including unidentified returns, while preserving defenses
and other surface objects. Original MCICONS artwork
is optional for older caches. [Display rules](spec/flight-map.md).

The [target window](spec/target-window.md) builds read-only presentation data in
`tore-app::target_window`. It uses the same retained selection as the HUD,
existing aircraft activity, and an independent weather slot. Its asynchronous
camera results carry the requested target identity so a changed selection cannot
reuse another target's picture. No display state feeds combat decisions.

Quick Mission [Dummy aircraft](spec/dummy-aircraft.md) carry a separate launch
mode. The mission actor moves them at a fixed 400 knots instead of invoking its
combat controller or aerodynamic flight. They retain the existing world snapshot,
damage mirror and target identities alongside normal AI aircraft.

Target-camera readbacks preserve a scenery coverage mask in alpha for the 10%
background darkening, then restore opaque panel pixels. Camera-specific near
clipping improves magnified subject depth precision. Target preview requests use
an independent 24 Hz phase clock; the other camera panels retain their existing
refresh interval.

## Ownship systems state

`tore-sim::aircraft_systems` coordinates separate engine, fluids, fuel, controls,
structure and pilot components. Each owns its fault state and fixed-tick
progression. The coordinator couples oil pressure to engine temperature and
combines fatal outcomes; it does not own the components' timer arithmetic.
Regional structural effects supply one shared set of flight penalties for both
flight adapters, using the same regional fractions as the damage renderer.
The renderer currently hides all airframe marks and tears below 100% damage;
this gate never changes component state or aerodynamic penalties. The app transfers newly selected combat faults exactly once, resolves
hardpoint identities and forwards notifications to the existing sim log. Flight
applies power, controls and actuator limits, and consumes source tank fuel. The
Systems raster only reads this state. D is a report action and never applies a
hit. See [the behavior contract](spec/systems-damage.md).

## Wreck lifecycle

`tore-sim::wreck` owns fixed-tick aerodynamic wreck motion, captured per-engine
thrust and an independent explosion RNG. Ownship flight and combat targets both
use it after destruction; living AI decisions are untouched. The AI bridge only
copies live propulsion metadata for later use by the wreck. Airburst events
trigger existing audio/effects and hide the whole airframe and detached pieces.
[Behavior contract](spec/destroyed-aircraft.md).

## Pilot escape ownership

`tore-sim::ejection` owns seat/chute motion, survival and the fitted recovery
assessment. Player commands are recorded in the existing pilot tape; AI decides
before weapon releases. Combat continues to own AI wreck motion, while the
mission retains the detached pilot. The app imports original indexed art into
a separate texture batch and schedules voice/effect transitions. See the
[ejection specification](spec/ejection.md) and [source notes](formats/ejection.md).

## Mission recordings

`crates/tore-replay` is dependency free and knows nothing of the simulation
or the renderer: the recording model, the chunked writer, the bounded reader
and the exports (debug log, summary, anomaly flags, Tacview, comparison).
`tore-app/src/replay/` is the only place the app's types meet it.

The capture boundary is the per-tick `RenderSnapshot`. Each tick, right after
the AI step, `replay/recorder.rs` reads the snapshot live flight draws plus
flight data the snapshot lacks, and diffs both against the previous tick to
find launches, hits, crashes, departures and AI activity changes.
`replay/convert.rs` turns a snapshot into frame pieces and a decoded frame
back into a snapshot; the draw rules that hold for a whole flight (loaded
models, which ids draw with them) travel in the header. Everything the
recorder reads was already computed by the tick. The few outputs it needed
are write-only and bounded, drained by the host and never read by flight: the
combat ledger's list of shot outcomes, the cockpit message requests in
`FlightUi` and the player commands in `Combat`; `deliver_radio` returns the
lines it delivered. Frames go to a writer thread through a queue two seconds
deep; a full queue drops the frame and the next reports a gap, so the flight
never waits for the disk. Probes with recording on and off print
byte-identical output.

`replay/library.rs` owns the `replays/` folder: names, `replays-v1.conf`
auto-delete settings, listing from each file's header, seek index and footer
(`Recording::peek`), and a cleanup that deletes only proven, unkept, inactive
recordings. `terrain::World::identity` captures the resolved world for the
header and `World::for_identity` rebuilds it without environment variables.
`replay/cli.rs` holds the `--recording-*` commands and the tick-by-tick
render check. See [mission replays](REPLAYS.md).
