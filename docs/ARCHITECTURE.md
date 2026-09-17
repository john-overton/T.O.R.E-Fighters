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
| Workspace | `crates/tore-app`, `tore-formats`, `tore-extract`, `tore-sim`, `tore-input`, `tore-input-native` | Desktop shell and entry point, plus the format, extraction, simulation and input crates |
| Window/input | `winit` 0.30 | Native window lifecycle and input |
| Graphics | `wgpu` 27 | Metal on macOS; native backends for Windows/Linux |
| Startup bridge | `pollster` 0.4 | Wait for GPU initialization without a general async runtime |
| Audio device | `cpal` 0.16 | Native output for the small PCM mixer; [upstream API](https://docs.rs/cpal/0.16.0/cpal/) |
| Formats | Dependency-free `crates/tore-formats` | Bounded EALIB, raw-literal DCL, PIC/glyphs, a narrow CHOOSEAC DLG reader, BIT2, mission environment fields PL weather palettes, BRF aircraft/equipment, bounded SH projection and compiled FNT glyphs |
| Simulation | `crates/tore-sim` | Shared 120 Hz state/attitude, selectable hybrid dynamics, the shared aircraft sensor component and headless aircraft acceptance |
| Extraction | `crates/tore-extract` + `tools/extract_assets.py` | Title-independent archive discovery/extraction, safe output paths, provenance |
| Checks | Cargo, Python standard library, GitHub Actions | Local and CI checks |

These are baseline-compatible versions, not automatically the latest versions. Exact versions live in `Cargo.lock`. See upstream [wgpu 27 documentation](https://docs.rs/wgpu/27.0.1/wgpu/) and [winit 0.30 documentation](https://docs.rs/winit/0.30.13/winit/) for API contracts.

The shell creates its window on the main event loop and uploads a 640 × 480 RGBA menu canvas to a Metal/native GPU texture. The shader draws a scaled, letterboxed image using linear filtering. The same viewport calculation maps physical mouse coordinates back into retail coordinates, including Retina scaling. It handles zero-sized windows and recoverable surface loss. Fatal graphics/startup errors return a failing process status.

`assets.rs` selectively decompresses resources into a versioned local pack and validates them before import completes. `menu.rs` composites original background, button pieces, and font strips and owns UI state. `renderer.rs` owns the surface, texture, and scaling. `audio.rs` mixes at most eight PCM effects plus an optional music loop, with linear resampling into the device rate. Audio initializes independently and can fail without blocking menus. No input device or microphone is opened.

The app and general extractor share the same EALIB/DCL readers. `Archive::open` reads a directory and seeks to selected resources; it does not load whole disc archives. The menu retains a 16 MiB resource cap; the general CLI has an explicit configurable cap for larger media. The Python entry point handles portable invocation and SHA-256 report enrichment; it contains no second decompressor.

Menu-only startup randomness chooses one of the five native backgrounds independently of future simulation state. The selected background's embedded palette colors shared sprites/fonts, and native bar offsets keep controls aligned. Hover/focus notifications do not enqueue audio.

Menu drawing uses CPU composition for this small static canvas; it is not a commitment to software-rendering flight scenes. The window sleeps while idle. Hover transitions and transient placeholder messages schedule temporary redraws. The fragment shader is authored source; it contains no retail bytes.

## Boundaries for menu work

Keep `tore-formats` independent of windowing and GPU APIs. The future simulation likewise needs to run headlessly with deterministic inputs. Avoid empty placeholder crates or premature engine abstractions.

Menu rendering should consume decoded palettes, indexed images, font data, and recovered layout geometry. Recover specifications from the TypeScript reference; implement runtime behavior in Rust. Do not introduce a web shell, copy the Three.js engine, or select a modern widget toolkit before checking retail geometry requirements.

Import user-owned media at runtime into platform application data. The v1 pack is a development cache of selected decompressed resources, not a stable mod/save format. `gameassets/` is a local source-media convenience, not a runtime bundle or save-data location. See [menu formats](formats/menu.md) for the current limits and provenance.

## First simulation renderer

`terrain.rs` constructs a world from the selected retail T2/MM, numbered texture family and its DAY2 variant palette. Its camera and surface queries have no GPU/window dependency. `sim_renderer.rs` uploads geometry and a texture array, owns depth targets and draws terrain plus a fullscreen sky pass; `terrain.wgsl` supplies the initial perspective, sampling and fog. `renderer.rs` composes this scene with the transparent CPU HUD, resizing depth and surface together. This separation allows aircraft/object/weather passes and a deterministic simulation to be added without coupling format readers to wgpu.

The initial implementation uses full-resolution fixed triangles and an authored sky/fog projection. It is not the native adaptive renderer. All geometry/colors come from local source data at runtime; no retail derivatives are embedded. See [theater findings](formats/theater.md) for recovered versus authored behavior. The Hornet adapter now advances at 120 fixed ticks/second; the developer free camera still uses elapsed wall time for inspection.

The creator selects among all 16 base theaters. Scene replacement rebuilds the GPU vertex/texture buffers for that world; a variable texture-array layer count also supplies the sky shader's layer index. Only the active world mesh is built, while the bounded source bundle remains cached. Maps and fonts stay in the menu compositor. Source text shading is preserved when tinting; ARMFont/SMLFONT replace the unsuitable BODYFONT in the investigation UI and notices.

The Hornet slice adds dependency resolution and bounded BRF/SH/FNT readers to `tore-formats`. `tore-sim::flight` contains fixed-tick state/integration without wgpu/winit dependencies; the app re-exports its interface; `aircraft.rs` adapts imported geometry and camera poses. `instruments.rs` renders independent small rasters from flight/equipment state. The GPU terrain pass now accepts an aircraft vertex stream and original rectangular atlas with shared depth; front/other instrument cameras render offscreen. CLI extraction and cache import share the same dependency resolver. These adapters do not execute imported x86 modules. See [aircraft evidence and open questions](formats/aircraft.md).


`flight_ui.rs` owns desktop command dispatch, imported menu navigation, session presentation settings and pause state. `hud.rs` draws the forward-flight HUD from state and source font glyphs, projecting the ladder/path through the renderer's 60-degree camera convention. Simulation remains independent of both. The full-canvas cockpit is transparent art over the world; instrument windows are independent rasters. Menu/focus pauses stop fixed ticks and engine loops, and input transitions clear held controls. Shader zoom is shared by terrain and sky projection; camera previews restore the main camera before drawing.


`flight_canvas.rs` now composes the flight-only overlay at an aspect-responsive size (physical drawable, proportionally capped at 1920×1080). The separate GPU cockpit pass preserves uniform cover-fit in the centered forward view, and instrument layout rectangles anchor to actual edges. Native instrument rasters go directly to their destination sizes instead of passing through a reduced 640×480 composite. The original cockpit texture is uploaded once; unchanged scaled panel rasters are cached. Alpha-aware filtering prevents dark transparent borders. `renderer.rs` recreates its UI texture when dimensions change and uses the full viewport for flight; menus/viewer overlays retain their existing canvas. Pointer conversion uses the same responsive panel rectangles, while the centered pause menu retains menu coordinates. HUD metadata is 15% smaller, with projection compensation for both shrink and portrait aspect.

### Flight presentation and measurement

`flight::State` remains authoritative at 120 Hz. `main` retains the preceding tick for render-only pose interpolation (shortest-path wrapped angles); pause/crash show authoritative state and restart resets history. Camera, exterior geometry and HUD consume the same presented pose. Audio consumes authoritative state. No renderer smoothing feeds back into physics.

Active simulation views request the next redraw without a post-render timer; AutoVsync and a requested maximum frame latency of one provide presentation backpressure. Idle menu behavior is unchanged. Failed/zero-size presentation does not continually schedule simulation redraws. `performance.rs` provides opt-in bounded CPU wall-time sampling via environment variables, with warmup exclusion and view cycling.

The GPU cockpit texture survives view and size changes; projection uniforms track the current aspect. Aircraft GPU resources are prepared when the renderer/theater loads. Live camera panels submit bounded asynchronous readbacks (at most one pending per camera page), consume completed rasters on later frames, and retain their last image while pending. The simulation renderer retains two depth targets to avoid reallocating display/138×114 depth buffers at every panel refresh. Offline captures retain an explicit blocking readback so smoke evidence contains the requested image. Direct GPU panel composition, full GPU UI rendering and native terrain LOD remain future optimization work.

`look.rs` owns authored held-key classification, look limits and exterior spherical orbit. Shift/Ctrl arrows are isolated from flight input and retain their look classification until physical release, including after modifier changes. Camera motion uses elapsed presentation time while unpaused. Internal elevation is limited to the forward eye line through overhead; exterior orbit keeps a fixed radius and aims at the same interpolated aircraft pose. It does not alter simulation state or synchronously capture the GPU.

### Continuous attitude and momentum

`attitude.rs` supplies body bases, Rodrigues rotation, orthonormalization, and render interpolation. `flight::State` retains separate world velocity and pitch/roll response rates. The old pitch clamp and nose-derived position update are removed. Force integration and attitude response remain deterministic at 120 Hz; render interpolation does not feed back. `look` uses the same body basis for head rotation, while exterior orbit retains aircraft-centered inspection behavior. The HUD flight-path marker projects actual velocity bearing/elevation.

`cockpit_renderer.rs` and `cockpit.wgsl` project the complete original forward artwork and the 640×480 HUD raster through one aircraft-fixed plane. Relative eye/body axes move both layers together during head-look, independently of aircraft attitude. The pass draws after the world and before screen-anchored instruments/menus, including offline captures; camera instrument readbacks exclude it. Linear premultiplied sampling preserves transparent borders. HUD uploads and uniforms are nonblocking; resize does not rebuild the source texture. The sky shader projects SKY0 onto a finite hemisphere disk with `ray.xz / (1 + max(ray.y, 0))`, avoiding the latitude/longitude singularity and seam at zenith. Native sky mapping and full 3D cockpit geometry remain unimplemented; the projected source plane has finite coverage and cannot provide a rear/overhead interior.

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
retained controller; public HID support queries suppress duplicate raw endpoints. It alone
permits unsafe platform FFI; `libc` and `windows` are thin platform bindings,
not a third-party input policy engine. Main-loop discovery never blocks a flight
frame. Presentation-only look resolution cannot change pilot-axis ownership.
See [contracts, platform limits and profile syntax](INPUT.md).

The paused flight controls editor owns a draft `tore-input::Profile`; native
capture is isolated from menu/gameplay dispatch. Canonical serialization validates
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
`combat::missiles`, independent of rendering. The live adapter uses full release
velocity for accepted missile profiles; compatibility retains scalar source
motion. Combat tape version 4 includes world velocity, while versions 2 and 3
select compatibility rules. [Missile specification](spec/missiles.md).
