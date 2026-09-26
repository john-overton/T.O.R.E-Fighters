// Windows release launches have no console. Diagnostics are initialized inside
// the executable before startup and fatal interactive errors use an OS dialog.
// CLI/probe output stays on stdout; startup diagnostics also go to session logs.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod additional_animation;
mod ai_wings;
mod aircraft;
mod aircraft_animation;
mod airfield_radio;
mod assets;
mod attitude;
mod audio;
mod canvas_present;
mod celestial;
mod clouds;
mod cockpit_renderer;
mod combat;
mod combat_tape;
mod comms;
mod controls_editor;
mod countermeasure_renderer;
mod crew_voice;
mod damage_art;
mod debrief;
mod diagnostics;
mod ejection_art;
mod engine_material;
mod flight;
mod flight_canvas;
mod flight_map;
mod flight_music;
mod flight_ui;
mod flight_views;
mod graphics;
mod graphics_screen;
mod hud;
mod hud_aperture;
mod input;
mod input_catalog;
mod instruments;
mod lens_flare;
mod locate;
mod look;
mod media_source;
mod menu;
mod mirrors;
mod missile_acceptance;
mod navigation;
mod ocean;
mod ordnance;
mod performance;
mod preferences;
mod quick_mission;
mod radio_calls;
mod rafale_animation;
mod render_snapshot;
mod renderer;
mod replay;
mod rocker;
mod roster_animation;
mod scope;
mod sim_renderer;
mod smoke_renderer;
mod startup;
mod static_art;
mod surface_lighting;
mod target_window;
mod terrain;
mod version;
mod weapon_hud;
mod weather;

use assets::Assets;
use menu::{Action, Menu};
use renderer::Renderer;
use std::{
    error::Error,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tore_sim::models::FlightModel;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::{CursorIcon, Fullscreen, Window, WindowId},
};
type AppResult<T> = Result<T, Box<dyn Error>>;
/// How an interactive start opens its window. Requested by John on 2026-09-22:
/// the game runs native borderless fullscreen by default on every platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowMode {
    Fullscreen,
    Windowed,
}
impl WindowMode {
    fn fullscreen(self) -> bool {
        self == WindowMode::Fullscreen
    }
}
/// The window mode shared by the locate shell and the game window, so one
/// Alt-Enter in the shell holds for the rest of the session.
struct WindowState {
    /// The mode the next window opens in.
    fullscreen: bool,
    /// What the `fullscreen` preference is saved as. Only a toggle changes it,
    /// so `--windowed` does not overwrite the player's saved choice.
    preference: bool,
}
impl WindowState {
    fn toggle(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.preference = self.fullscreen;
    }
}
/// The winit setting for a window mode. `Borderless(None)` uses whichever
/// monitor the window would otherwise have opened on.
fn fullscreen_attribute(on: bool) -> Option<Fullscreen> {
    on.then_some(Fullscreen::Borderless(None))
}
/// Decide the initial window mode. Borderless fullscreen is the default; a
/// window is used when the player asked for one, when a flag fixes the window
/// size, or when the saved preference says so.
///
/// * `windowed_flag`: `--windowed` was given.
/// * `window_size_flag`: `--window-size` was given.
/// * `fixed_size`: a capture, snapshot or `--smoke-test` run, all of which
///   depend on a known window size.
/// * `preference`: the saved `fullscreen` preference, default on.
fn initial_window_mode(
    windowed_flag: bool,
    window_size_flag: bool,
    fixed_size: bool,
    preference: bool,
) -> WindowMode {
    if windowed_flag || window_size_flag || fixed_size || !preference {
        WindowMode::Windowed
    } else {
        WindowMode::Fullscreen
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Main,
    Quick,
    Viewer,
    Flight,
}
struct App {
    preference_path: Option<PathBuf>,
    preference_saved: String,
    input: input::Input,
    focused: bool,
    input_recording: Option<std::io::BufWriter<std::fs::File>>,
    recorded_ticks: u64,
    performance: performance::Performance,
    combat: combat::Combat,
    hornet: aircraft::Airframe,
    flight: flight::State,
    researched_flight: bool,
    native_tables: Option<std::sync::Arc<tore_sim::native::Tables>>,
    previous_flight: flight::State,
    flight_clock: flight::Clock,
    /// Situation music observations; audio only, never read by the simulation.
    flight_music: flight_music::Observer,
    vapor: tore_sim::vapor::Vapor,
    turbulence: tore_sim::turbulence::Turbulence,
    turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng,
    /// Player blackout and redout, stepped with the simulation.
    g_effects: tore_sim::g_effects::GEffects,
    flight_view: u8,
    view_rig: flight_views::Rig,
    flight_canvas: flight_canvas::FlightCanvas,
    window_size: [u32; 2],
    /// Live window mode. Alt-Enter toggles it.
    fullscreen: bool,
    /// What the `fullscreen` preference is saved as. A flag such as
    /// `--windowed` chooses the mode for one run without overwriting the
    /// player's saved choice; only Alt-Enter changes this.
    fullscreen_preference: bool,
    flight_ui: flight_ui::FlightUi,
    instruments: instruments::Instruments,
    world: terrain::World,
    airport_service: tore_sim::airport::Service,
    airport_nav_mode: bool,
    airport_commands: Vec<flight_ui::Command>,
    theater_resources: std::collections::BTreeMap<String, Vec<u8>>,
    camera: terrain::Camera,
    quick: quick_mission::QuickMission,
    mission: Option<(f64, f64)>,
    /// Accepted player runway start, retained independently of editor changes.
    ground_start: Option<u32>,
    launch_creator: bool,
    /// Quick Mission uses AI by default. `--fixture-wings` retains the
    /// straight-flight compatibility setup.
    ai_wings_enabled: bool,
    /// Session-only flight-menu enemy-skill preference; see `--enemy-skill`.
    enemy_skill: Option<tore_sim::ai::experience::EnemySkillOverride>,
    ai_wings: Option<ai_wings::AiWings>,
    ai_mission: ai_wings::Preset,
    /// Radio and crew voice delivery; see docs/spec/radio-chatter.md.
    comms: comms::Comms,
    airfield_radio: airfield_radio::AirfieldRadio,
    /// Weapon, hit, kill and wing radio calls; see radio_calls.rs.
    radio: radio_calls::Radio,
    /// Imported phrase text for composing radio lines.
    phrases: comms::Phrases,
    /// The player's crew voice; see docs/spec/cockpit-voice.md.
    crew_voice: crew_voice::CrewVoice,
    screen: Screen,
    frame_time: Instant,
    instrument_time: Instant,
    target_refresh: target_window::Refresh,
    menu: Menu,
    audio: Option<audio::Audio>,
    wing_recipient: Option<u8>,
    renderer: Option<Renderer>,
    /// Graphics choices for the 3D view, applied to the renderer.
    graphics: graphics::Options,
    /// Where the Graphics screen saves them; `None` for diagnostics.
    graphics_path: Option<PathBuf>,
    pointer: Option<(f64, f64)>,
    modifiers: ModifiersState,
    smoke_test: bool,
    capture_terrain: Option<PathBuf>,
    /// Set by Pref > Re-import media: the menu frame the player was looking
    /// at, which the locate screen then draws over.
    reimport: Option<Vec<u8>>,
    /// The input configuration screen, open over the main menu or the
    /// paused flight menu. One component serves both.
    controls: Option<controls_editor::Editor>,
    /// The Graphics options screen, open over the main menu.
    graphics_screen: Option<graphics_screen::Editor>,
    /// Cursor position while the right button drags mouse look.
    mouse_look: Option<(f64, f64)>,
    /// Unused fraction of a smooth-scrolling wheel notch.
    wheel: f64,
    /// Head-tracker view offset added to the player's look angles.
    head_look: [f32; 2],
    finished: bool,
    next_frame: Option<Instant>,
    error: Option<Box<dyn Error>>,
    /// Where every flight's mission recording goes; `None` turns recording
    /// off (captures and diagnostics). See docs/REPLAYS.md.
    replay_library: Option<replay::library::Library>,
    /// The flight being recorded.
    replay_recorder: Option<replay::recorder::Recorder>,
}
/// The AI wingmen a player's wing order addresses, for the mission
/// recording: every living member of the player's wing, or the one wingman
/// Alt-4 to Alt-7 chose (1 is the first wingman), as the order rules pick
/// them.
fn wing_recipients(wings: Option<&ai_wings::AiWings>, recipient: Option<u8>) -> Vec<u32> {
    let Some(wings) = wings else {
        return Vec::new();
    };
    let mut members: Vec<(u8, u32)> = wings
        .slots()
        .iter()
        .filter(|slot| {
            slot.side == tore_sim::ai::launch::Side::Friendly
                && slot.wing_number == 1
                && wings.mission().actor(slot.id).is_some_and(|a| a.alive())
        })
        // The display number counts the player as the first member.
        .map(|slot| (slot.member_number.saturating_sub(1), slot.id))
        .filter(|(member, _)| recipient.is_none_or(|wanted| *member == wanted))
        .collect();
    members.sort_unstable();
    members.into_iter().map(|(_, id)| id).collect()
}
/// Deliver due radio and crew lines: HUD text and recordings together.
/// Returns the calls it delivered, for the mission recording.
fn deliver_radio(
    comms: &mut comms::Comms,
    flight_ui: &mut flight_ui::FlightUi,
    audio: Option<&audio::Audio>,
    now: f64,
) -> Vec<comms::Call> {
    let due = comms.due(now);
    for call in &due {
        match call.route {
            comms::Route::Radio | comms::Route::Airport => {
                flight_ui.message(call.line());
                if let Some(audio) = audio {
                    if call.route == comms::Route::Airport {
                        audio.airport_speech(&call.stems);
                    } else {
                        audio.speech(&call.stems);
                    }
                }
            }
            comms::Route::Direct => {
                if let (Some(audio), Some(stem)) = (audio, call.stems.first()) {
                    audio.direct_voice(stem);
                }
            }
        }
    }
    due
}
/// Wing vapor line segments: position then RGBA, two vertices per segment.
/// The five native colors are patterned fill types resolved through LAY
/// header remap tables we have located but not decoded, so the rendered
/// color and fade are fitted: the brightest source sky entry, thinning
/// along the trail.
fn vapor_vertices(
    vapor: &tore_sim::vapor::Vapor,
    world: &terrain::World,
    presented: &flight::State,
    attachments: Option<[[f64; 3]; 2]>,
) -> Vec<f32> {
    let hazing = world
        .weather
        .sample(presented.position[1])
        .is_some_and(|l| l.night_hazing());
    let roll_rate = presented.roll_rate.to_degrees();
    let mut out = Vec::new();
    for side in 0..2 {
        let Some(mut trail) = vapor.trail(side, presented.g, roll_rate, hazing) else {
            continue;
        };
        // The mesh is interpolated for display; pin only the drawn head to that
        // same pose. Authoritative trail samples remain unchanged.
        if let Some(points) = attachments {
            trail[0] = points[side];
        }
        for (i, pair) in trail.windows(2).enumerate() {
            for (end, point) in pair.iter().enumerate() {
                let step = (i + end) as f32 / vapor.segments() as f32;
                out.extend([
                    point[0] as f32,
                    point[1] as f32,
                    point[2] as f32,
                    1.,
                    1.,
                    1.,
                    0.55 * (1. - step),
                ]);
            }
        }
    }
    out
}

/// One tick of physical turbulence, applied to attitude and height only.
/// Velocity is untouched: the recovered routine is an angular and vertical
/// perturbation, not a three-dimensional wind field.
fn step_turbulence(
    turbulence: &mut tore_sim::turbulence::Turbulence,
    rng: &mut tore_formats::flight_model::clock_rng::NativeRng,
    flight: &mut flight::State,
    world: &terrain::World,
    enabled: bool,
) -> Option<tore_input::FeedbackEvent> {
    // The joined native service explicitly selects the source disabled branch.
    if flight.native.is_some() {
        return None;
    }
    let ground = f64::from(world.height(flight.position[0] as f32, flight.position[2] as f32));
    let agl = flight.position[1] - ground;
    let conditions = tore_sim::turbulence::Conditions {
        agl_feet: agl,
        on_ground: flight.crashed || flight.research.as_ref().is_some_and(|r| r.on_ground),
        speed_fps: flight.speed,
        seconds_of_day: world.weather.seconds_of_day(),
        percent: flight.model().configuration().turbulence_percent,
        daytime_ground: world.turbulence_reduced_surface(flight.position[0], flight.position[2]),
        enabled,
        nearby_strength: 0,
    };
    let d = turbulence
        .step(world.weather.ticks(), 2, conditions, rng)
        .ok()?;
    if d == tore_sim::turbulence::Disturbance::default() {
        return None;
    }
    flight.apply_turbulence(d);
    d.shake.then(|| tore_input::FeedbackEvent::Turbulence {
        intensity: d.severity(),
    })
}

fn airport_aircraft(
    world: &terrain::World,
    flight: &flight::State,
    nav_mode: bool,
) -> tore_sim::airport::Aircraft {
    let supported = world
        .airport_scene
        .runway_surface(flight.position[0], flight.position[2])
        .is_some_and(|(_, height)| flight.supported_at(height));
    tore_sim::airport::Aircraft {
        position: flight.position,
        forward: attitude::Basis::new(flight.yaw, flight.pitch, flight.bank).forward,
        nav_mode,
        gear_down: flight.gear_down,
        supported,
        alive: !flight.crashed,
        speed_fps: flight.speed,
    }
}

fn airport_wind(
    world: &terrain::World,
    flight: &flight::State,
    guidance: Option<&tore_sim::airport::Guidance>,
) -> Option<tore_sim::runway_wind::Assessment> {
    if flight.crashed {
        return None;
    }
    let heading = if let Some((id, height)) = world
        .airport_scene
        .runway_surface(flight.position[0], flight.position[2])
        && flight.supported_at(height)
    {
        let runway = world.airport_scene.runway(id)?;
        let near = runway.heading;
        if (flight.yaw - near).cos() >= 0. {
            near
        } else {
            near + std::f64::consts::PI
        }
    } else {
        let guidance = guidance?;
        world
            .airport_scene
            .runway(guidance.runway)?
            .approach_heading(guidance.end)
    };
    tore_sim::runway_wind::assessment(
        flight.model().configuration().mass.max_takeoff_lbs,
        world.wind(),
        heading,
    )
}

fn airport_reply(world: &terrain::World, reply: &tore_sim::airport::Reply) -> String {
    use tore_sim::airport::Reply;
    match reply {
        Reply::Selected { airport } => world
            .airport_scene
            .airports
            .iter()
            .find(|a| a.id == *airport)
            .map_or_else(
                || "Airport unavailable".into(),
                |a| format!("Selected {}", a.name),
            ),
        Reply::Cleared {
            airport,
            runway,
            end,
        } => {
            let name = world
                .airport_scene
                .airports
                .iter()
                .find(|a| a.id == *airport)
                .map_or("Airport", |a| a.name.as_str());
            let runway_name = world
                .airport_scene
                .runway(*runway)
                .map_or("runway", |r| r.name.as_str());
            let end = match end {
                tore_sim::airport::ApproachEnd::Near => "near approach",
                tore_sim::airport::ApproachEnd::Far => "far approach",
            };
            format!("{name}: cleared to land, {runway_name}, {end}")
        }
        Reply::Landed { airport, .. } => {
            let name = world
                .airport_scene
                .airports
                .iter()
                .find(|a| a.id == *airport)
                .map_or("Airport", |a| a.name.as_str());
            format!("{name}: welcome home, landing complete")
        }
        Reply::Declined { reason, .. } => {
            use tore_sim::airport::DeclineReason::*;
            let reason = match reason {
                NoAirport => "no airport selected",
                NoRunway => "no usable runway",
                Hostile => "airport is hostile",
                NeutralPermission => "permission is required",
                UnknownAllegiance => "airport allegiance is unknown",
                RunwayDisabled => "runway is unavailable",
            };
            format!("Landing request declined: {reason}")
        }
        Reply::Repeated(reply) => airport_reply(world, reply),
        Reply::Cancelled { .. } => "Approach cancelled".into(),
    }
}

fn airport_reply_audio(reply: &tore_sim::airport::Reply) -> Option<&'static str> {
    use tore_sim::airport::Reply;
    match reply {
        Reply::Cleared { .. } => Some(tore_formats::radio::AIRPORT_CLEAR_TO_LAND),
        Reply::Landed { .. } => Some(tore_formats::radio::AIRPORT_WELCOME_HOME),
        Reply::Repeated(reply) => airport_reply_audio(reply),
        Reply::Selected { .. } | Reply::Declined { .. } | Reply::Cancelled { .. } => None,
    }
}

impl App {
    /// Rebuilds the world under one recovered weather condition. The renderer
    /// owns per-world GPU resources, so it is rebuilt with it.
    fn set_condition(&mut self, index: usize) -> AppResult<()> {
        let code = self.world.layout.trim_end_matches(".MM").to_string();
        self.world = terrain::World::for_mission(&self.theater_resources, &code, Some(index))?;
        if let Some(renderer) = &mut self.renderer {
            renderer.set_world(&self.world);
            renderer.prepare_aircraft(&self.hornet);
        }
        Ok(())
    }

    /// Restart the resolved launch environment and its authored RNG policy.
    /// Simulation seconds of the current flight, from the fixed 120 Hz tick.
    fn sim_seconds(&self) -> f64 {
        self.combat.state.tick() as f64 / 120.
    }

    fn reset_weather(&mut self) {
        self.world.weather_presentation = tore_sim::environment::Presentation::seeded(1)
            .expect("fixed valid weather presentation seed");
        self.world.auxiliary_presentations =
            std::array::from_fn(|_| self.world.weather_presentation.clone());
        self.world.weather =
            tore_sim::environment::Environment::new(self.world.weather.configuration().clone());
        self.turbulence = Default::default();
        self.turbulence_rng.reseed_word(1);
    }

    /// Seed after the final launch position is set, so no trail crosses a teleport.
    fn reset_vapor(&mut self) {
        self.vapor = tore_sim::vapor::Vapor::seeded(
            self.hornet
                .streamer_points(&self.flight)
                .unwrap_or([[0.; 3]; 2]),
        );
    }

    /// Switch between borderless fullscreen and the previous windowed size.
    /// winit restores the window's pre-fullscreen size and position, so the
    /// player returns to the window they had. Works on every screen: the
    /// menus, the creator and flight.
    fn toggle_fullscreen(&mut self) {
        let Some(renderer) = self.renderer.as_ref() else {
            return;
        };
        self.fullscreen = !self.fullscreen;
        self.fullscreen_preference = self.fullscreen;
        renderer
            .window
            .set_fullscreen(fullscreen_attribute(self.fullscreen));
        renderer.window.request_redraw();
        // The letterbox and the pointer mapping both follow the new size, and
        // a press in flight must not survive the change.
        self.menu.state.cancel();
        self.quick.cancel();
        self.instruments.cancel_press();
        self.flight_ui.cancel_press();
        self.pointer = None;
        self.save_preferences();
    }

    fn save_preferences(&mut self) {
        let Some(path) = &self.preference_path else {
            return;
        };
        let text = preferences::Preferences::capture(
            &self.flight_ui,
            &self.instruments,
            &self.menu.state,
            self.fullscreen_preference,
        )
        .text();
        if text == self.preference_saved {
            return;
        }
        match preferences::write(path, &text) {
            Ok(()) => self.preference_saved = text,
            Err(e) => {
                log::warn!("Preferences: {e}");
                self.flight_ui
                    .message(format!("Could not save preferences: {e}"));
            }
        }
    }
    fn finish_recording(&mut self) {
        use std::io::Write;
        if let Some(mut recording) = self.input_recording.take()
            && let Err(error) = recording.flush()
        {
            self.error = Some(error.into());
        }
    }

    /// Starts recording the flight that was just set up. A recording that
    /// cannot start is logged and the flight goes on unrecorded.
    fn start_replay_recording(&mut self) {
        use replay::{convert, recorder};
        let Some(library) = &self.replay_library else {
            return;
        };
        // Commands from before this flight belong to no recording.
        let _ = self.combat.take_notes();
        let started = std::time::SystemTime::now();
        let snapshot = self.combat.render_snapshot();
        let presentation = convert::Presentation::of(snapshot);
        let aircraft = convert::identity_key(self.hornet.profile.id);
        let path = match library.new_path(
            started,
            self.world.layout.trim_end_matches(".MM"),
            aircraft.trim_end_matches(".PT"),
        ) {
            Ok(path) => path,
            Err(error) => {
                log::info!("Recording unavailable: {error}");
                return;
            }
        };
        let mut extra = vec![
            (
                "flight_model".to_owned(),
                if self.native_tables.is_some() {
                    "native-tables"
                } else if self.researched_flight {
                    "researched"
                } else {
                    "legacy"
                }
                .to_owned(),
            ),
            ("player.aircraft".into(), aircraft.into()),
            (
                "audio".into(),
                if self.audio.is_some() { "on" } else { "off" }.into(),
            ),
        ];
        if let Some((altitude, fuel)) = self.mission {
            extra.push((
                "mission.start".into(),
                self.ground_start
                    .map_or("airborne".to_owned(), |object| format!("runway {object}")),
            ));
            extra.push(("mission.altitude_ft".into(), altitude.to_string()));
            extra.push(("mission.fuel_lb".into(), fuel.to_string()));
            extra.push(("ai.mission".into(), self.ai_mission.to_string()));
            extra.push((
                "ai.aircraft".into(),
                self.ai_wings
                    .as_ref()
                    .map_or(0, ai_wings::AiWings::len)
                    .to_string(),
            ));
        }
        if self.combat.range {
            extra.push(("range".into(), "live fire".into()));
        }
        let cheats = recorder::cheats_on(&self.flight_ui.cheats);
        if !cheats.is_empty() {
            extra.push(("cheats".into(), cheats.join(",")));
        }
        let mission = if self.mission.is_some() {
            tore_replay::MissionKind::QuickMission
        } else {
            tore_replay::MissionKind::FreeFlight
        };
        let header = recorder::header(mission, &self.world, &presentation, extra, started);
        let roster = recorder::roster(
            snapshot,
            &self.hornet.profile.name,
            self.mission.is_some(),
            self.ai_wings.as_ref(),
            self.combat.models(),
        );
        let mut recording = match recorder::Recorder::start(path, &header, &roster) {
            Ok(recording) => recording,
            Err(error) => {
                log::info!("Recording unavailable: {error}");
                return;
            }
        };
        // The flight as it starts, before the first tick.
        recording.begin(recorder::Tick {
            snapshot: self.combat.render_snapshot(),
            combat: &self.combat,
            flight: &self.flight,
            previous: &self.flight,
            pilot: &flight::PilotInput::default(),
            wings: self.ai_wings.as_ref(),
            world: &self.world,
            events: &[],
            outcomes: &[],
        });
        recording.end(None, &mut self.combat);
        self.replay_recorder = Some(recording);
    }

    /// Finishes the flight's recording, if one is running, and applies the
    /// auto-delete settings. `reason` says why the flight ended.
    fn finish_replay_recording(&mut self, reason: &str) {
        let Some(mut recording) = self.replay_recorder.take() else {
            return;
        };
        recording.note(
            tore_replay::Event::new(tore_replay::vocab::kind::SYSTEM_END)
                .with(tore_replay::vocab::field::REASON, reason),
        );
        let report = self
            .mission
            .is_some()
            .then(|| debrief::capture(&self.combat, &self.flight, self.ai_wings.as_ref()));
        let footer = replay_footer(&self.combat, report.as_ref(), reason);
        if recording.finish(&footer).is_some()
            && let Some(library) = &self.replay_library
        {
            let settings = library.settings();
            let cleanup = library.cleanup(&settings, std::time::SystemTime::now(), &[]);
            for path in &cleanup.deleted {
                log::info!("Recording auto-deleted: {}", path.display());
            }
            for (path, error) in &cleanup.failed {
                log::info!("Recording not deleted: {}: {error}", path.display());
            }
        }
    }

    fn input_action(&mut self, action: tore_input::Action) -> Action {
        use flight_ui::Command;
        let name = match action {
            tore_input::Action::Pilot(command) => {
                if self.screen == Screen::Flight && !self.flight_ui.frozen() {
                    if matches!(
                        command,
                        tore_input::PilotCommand::Toggle(tore_input::Switch::Hook)
                            | tore_input::PilotCommand::Set(tore_input::Switch::Hook, _)
                    ) && !self.flight.hook_available()
                    {
                        self.flight_ui.message("Hook unavailable for this aircraft");
                    } else if command == tore_input::PilotCommand::Toggle(tore_input::Switch::Radar)
                        && self.instruments.channel != 0
                    {
                        // The same rule as the keyboard: returning to the radar
                        // channel comes before spending the power switch.
                        self.instruments.channel = 0;
                    } else {
                        self.input.queue(command);
                    }
                }
                return Action::None;
            }
            tore_input::Action::Axis(_) => return Action::None,
            tore_input::Action::Ui(name) => name,
        };
        let key = match name.as_str() {
            "menu-up" => Some("ArrowUp"),
            "menu-down" => Some("ArrowDown"),
            "menu-left" => Some("ArrowLeft"),
            "menu-right" => Some("ArrowRight"),
            "menu-accept" => Some("Enter"),
            "menu-back" | "menu" => Some("Escape"),
            _ => None,
        };
        // Controller menu buttons navigate the Graphics screen while it is open.
        if let Some(editor) = &mut self.graphics_screen {
            let Some(key) = key else {
                return Action::None;
            };
            let result = editor.key(key, false);
            return self.graphics_result(result);
        }
        if self.screen != Screen::Flight {
            return match (self.screen, key) {
                (Screen::Main, Some(key)) => self.menu.state.key(key, false),
                (Screen::Quick, Some(key)) => self.quick.key(key, false),
                _ => Action::None,
            };
        }
        if let Some(key) = key {
            // Menu navigation buttons have no flight semantics while the menu is closed.
            if self.flight_ui.menu || name == "menu" {
                let command =
                    self.flight_ui
                        .key(key, false, false, false, &self.hornet.flight_menu);
                return self.flight_command(command);
            }
            return Action::None;
        }
        if name == "pause" {
            let command = self
                .flight_ui
                .key("p", false, true, false, &self.hornet.flight_menu);
            return self.flight_command(command);
        }
        if self.flight_ui.frozen() {
            return Action::None;
        }
        if let Some(mut key) = name.strip_prefix("key:") {
            let ctrl = key.starts_with("Ctrl-");
            if ctrl {
                key = &key[5..];
            }
            let alt = key.starts_with("Alt-");
            if alt {
                key = &key[4..];
            }
            let shift = key.starts_with("Shift-");
            if shift {
                key = &key[6..];
            }
            let command = self
                .flight_ui
                .key(key, shift, ctrl, alt, &self.hornet.flight_menu);
            return self.flight_command(command);
        }
        let command = match name.as_str() {
            "weapon-next" => Command::NextWeapon,
            "weapon-previous" => Command::PreviousWeapon,
            "weapon-seeker-mode" => {
                Command::Combat(tore_sim::combat::live::Command::ToggleSeekerMode)
            }
            "designate" => Command::Target,
            "designate-previous" => Command::TargetPrevious,
            "designate-visual" => Command::TargetVisual,
            "clear-designation" => {
                Command::Combat(tore_sim::combat::live::Command::ClearDesignation)
            }
            "master-arm" => Command::None, // Retired binding in older profiles.
            "jettison" => Command::Combat(tore_sim::combat::live::Command::Jettison),
            "range-target" => Command::RangeReset,
            "damage-class" => Command::Combat(tore_sim::combat::live::Command::CycleClass),
            "fail-station" => Command::Combat(tore_sim::combat::live::Command::FailStation),
            "damage-report" => Command::DamageReport,
            "damage-player" => Command::Combat(tore_sim::combat::live::Command::DamagePlayer),
            "incoming" => Command::Combat(tore_sim::combat::live::Command::Incoming),
            "target-jammer" => Command::Combat(tore_sim::combat::live::Command::ToggleTargetJammer),
            "chaff" => Command::Chaff,
            "flare" => Command::Flare,
            "end-flight" => Command::End,
            "restart" => Command::Restart,
            "bookmark" => Command::Bookmark,
            "view-front" => Command::View(0),
            "view-back" => Command::View(3),
            "view-up" => Command::View(4),
            "view-external" => Command::View(1),
            "view-track" => Command::View(flight_views::TRACK),
            "view-threat" => Command::View(flight_views::THREAT),
            "view-wing" => Command::View(flight_views::WING),
            "view-target" => Command::View(flight_views::TARGET),
            "view-target-player" => Command::View(flight_views::TARGET_PLAYER),
            "view-fly-by" => Command::View(flight_views::FLY_BY),
            "view-missile" => Command::View(flight_views::MISSILE),
            "store-view" => Command::StoreView,
            "view-target-track" => {
                Command::ViewRelative(flight_views::TRACK, flight_views::Reference::Target)
            }
            "center-look" => Command::CenterLook,
            "waypoint-next" => Command::Waypoint(true),
            "waypoint-previous" => Command::Waypoint(false),
            s if s.starts_with("throttle-preset=") => match &s[16..] {
                "burner" => Command::ThrottlePreset(1., true),
                value => value
                    .parse()
                    .map_or(Command::None, |value| Command::ThrottlePreset(value, false)),
            },
            s if s.starts_with("throttle-step=") => {
                s[14..].parse().map_or(Command::None, Command::ThrottleStep)
            }
            "instrument-next" => Command::InstrumentCycle(1),
            "instrument-previous" => Command::InstrumentCycle(-1),
            "range-down" => Command::Range(-1),
            "range-up" => Command::Range(1),
            // The recovered radar-mode action is the available sensor-channel
            // cycle; the two newer names are explicit aliases for it.
            "radar-mode" | "sensor-channel" => Command::Mode,
            "sensor-infrared" => Command::SensorInfrared,
            "sensor-history" => Command::SensorHistory,
            "airport-next" => {
                let next = self
                    .world
                    .airport_scene
                    .airports
                    .iter()
                    .map(|a| a.id)
                    .find(|id| Some(*id) > self.airport_service.selected())
                    .or_else(|| self.world.airport_scene.airports.first().map(|a| a.id));
                next.map_or(Command::None, |id| {
                    Command::Airport(tore_sim::airport::Command::SelectAirport(id))
                })
            }
            "airport-request-landing" => {
                Command::Airport(tore_sim::airport::Command::RequestLanding)
            }
            "airport-repeat" => Command::Airport(tore_sim::airport::Command::RepeatReply),
            "airport-cancel" => Command::Airport(tore_sim::airport::Command::CancelApproach),
            "airport-nav" => Command::AirportNav,
            "cockpit" => {
                self.flight_ui.cockpit = !self.flight_ui.cockpit;
                Command::Click
            }
            "hud" => {
                self.flight_ui.hud = !self.flight_ui.hud;
                Command::Click
            }
            "zoom-in" => {
                self.flight_ui.zoom = (self.flight_ui.zoom * 1.1).min(4.);
                Command::None
            }
            "zoom-out" => {
                self.flight_ui.zoom = (self.flight_ui.zoom / 1.1).max(0.5);
                Command::None
            }
            s if s.starts_with("page-") => Command::Panel(s[5..].parse().unwrap_or(0)),
            s if s.starts_with("control-") => {
                Command::InstrumentControl(s[8..].parse::<usize>().unwrap_or(1) - 1)
            }
            s if s.starts_with("instrument-") => {
                if let Some((slot, button)) = s[11..].split_once("-control-") {
                    let slot = slot.parse::<usize>().unwrap_or(1) - 1;
                    let button = button.parse::<usize>().unwrap_or(1) - 1;
                    if self.instruments.control(slot, button) {
                        return Action::Click;
                    }
                    self.flight_ui.message("Instrument control unavailable");
                    return Action::None;
                }
                Command::InstrumentSelect(s[11..].parse::<usize>().unwrap_or(1) - 1)
            }
            _ => Command::None,
        };
        self.flight_command(command)
    }

    fn flight_command(&mut self, command: flight_ui::Command) -> Action {
        use flight_ui::Command;
        if self.native_tables.is_some() && !self.flight_ui.cheats.no_turbulence {
            self.flight_ui.cheats.no_turbulence = true;
            self.flight_ui
                .message("Environmental turbulence is unavailable in native research flight");
        }
        match command {
            Command::AirportNav => {
                if self.airport_commands.len() < 32 {
                    self.airport_commands.push(Command::AirportNav);
                }
                Action::None
            }
            Command::Airport(command) => {
                if !self.flight_ui.frozen() && self.airport_commands.len() < 32 {
                    self.airport_commands.push(Command::Airport(command));
                }
                Action::None
            }
            Command::WingRecipient(recipient) => {
                if !self.flight_ui.frozen() {
                    self.wing_recipient = recipient;
                    self.flight_ui.message(recipient.map_or_else(
                        || "Orders address all wingmen".to_owned(),
                        |n| format!("Orders address wingman {n}"),
                    ));
                }
                Action::None
            }
            Command::WingFormationCycle => match &self.ai_wings {
                Some(wings) => {
                    let next = wings.next_formation(self.wing_recipient);
                    self.flight_command(Command::Wing(tore_sim::ai::wing::PlayerOrder::Formation(
                        next,
                    )))
                }
                None => {
                    self.flight_ui.message("Wing order unavailable: no AI wing");
                    Action::None
                }
            },
            Command::Wing(order) => {
                if self.flight_ui.frozen() {
                    return Action::None;
                }
                let selected = self.combat.state.designated();
                // Land at selected airport uses the airport Shift-N selected
                // for the tower.
                let site = if order == tore_sim::ai::wing::PlayerOrder::LandAtSelected {
                    match ai_wings::AiWings::landing_site(
                        &self.world.airport_scene,
                        &self.world.airfield_anchors,
                        &self.airport_service,
                    ) {
                        Ok(site) => Some(site),
                        Err(message) if self.ai_wings.is_some() => {
                            self.flight_ui.message(message);
                            return Action::None;
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };
                let recipients = wing_recipients(self.ai_wings.as_ref(), self.wing_recipient);
                let result = self.ai_wings.as_mut().map(|bridge| {
                    bridge.command_at(order, selected, self.wing_recipient, site.as_ref())
                });
                match result {
                    Some(Ok(report)) => {
                        if let Some(audio) = &self.audio {
                            audio.radio(&report.radio, true);
                        }
                        if !report.radio.is_empty() {
                            self.comms.spoken(self.sim_seconds());
                        }
                        if let Some(recording) = &mut self.replay_recorder {
                            recording.order(
                                &format!("{order:?}"),
                                recipients,
                                &report.message,
                                &report.radio,
                                None,
                            );
                        }
                        self.flight_ui.message(report.message);
                    }
                    Some(Err(error)) => {
                        if let Some(recording) = &mut self.replay_recorder {
                            recording.order(
                                &format!("{order:?}"),
                                recipients,
                                &error.to_string(),
                                &[],
                                Some(&error.to_string()),
                            );
                        }
                        self.flight_ui.message(error.to_string());
                    }
                    None => self.flight_ui.message("Wing order unavailable: no AI wing"),
                }
                Action::None
            }
            Command::Bookmark => {
                match self.replay_recorder.as_mut() {
                    Some(recording) => {
                        let number = recording.bookmark();
                        self.flight_ui.message(format!("Bookmark {number} saved"));
                    }
                    None => self
                        .flight_ui
                        .message("Bookmark not saved: this flight is not being recorded"),
                }
                Action::None
            }
            Command::RadioSilence => {
                let message = self.comms.toggle_silence();
                self.flight_ui.message(message);
                Action::None
            }
            Command::DamageReport => {
                self.flight_ui
                    .message(self.flight.systems.summary(self.flight.damage_fraction));
                for index in 1..36 {
                    if self.flight.systems.has(index) {
                        self.flight_ui
                            .message(tore_sim::aircraft_systems::label(index));
                    }
                }
                for message in self.combat.equipment_damage_report() {
                    self.flight_ui.message(message);
                }
                Action::None
            }
            Command::None => Action::None,
            Command::Click => Action::Click,
            Command::End => Action::Back,
            Command::Exit => Action::Exit,
            Command::Restart => Action::FreeFlight,
            // Retail Ctrl+V works only while flying an aircraft. The message is
            // an opinionated agent addition (2026-09-23).
            Command::Valkyries => {
                if !self.flight_ui.frozen()
                    && self.flight.escape.is_none()
                    && !self.flight.crashed
                    && let Some(on) = self.audio.as_ref().and_then(audio::Audio::toggle_valkyries)
                {
                    self.flight_ui.message(if on {
                        "Valkyries music on"
                    } else {
                        "Valkyries music off"
                    });
                }
                Action::None
            }
            Command::Eject => {
                if !self.flight_ui.frozen() {
                    self.input.queue(tore_input::PilotCommand::Eject);
                }
                Action::None
            }
            Command::Combat(command) => {
                if self.combat.range
                    || matches!(
                        command,
                        tore_sim::combat::live::Command::ToggleArm
                            | tore_sim::combat::live::Command::ClearDesignation
                            | tore_sim::combat::live::Command::ToggleSeekerMode
                    )
                {
                    self.combat.cancel();
                    self.combat.command(command, combat::launcher(&self.flight));
                    // Range commands can replace targets or launch a round now.
                    if self.combat.range {
                        self.combat
                            .refresh_render(&self.flight, self.ai_wings.as_ref());
                    }
                    if let Err(error) = self.flight.set_payload(
                        (self.combat.state.payload_lbs() - self.flight.systems.used_external_lbs())
                            .max(0.),
                    ) {
                        self.flight_ui.message(error.to_string());
                    }
                } else {
                    self.flight_ui
                        .message("Manual range command requires --live-fire");
                }
                Action::None
            }
            Command::Chaff | Command::Flare => {
                use tore_sim::combat::live::Command as Live;
                let launcher = combat::launcher(&self.flight);
                if self.flight_ui.frozen()
                    || !launcher.alive
                    || self.flight.escape.is_some()
                    || self.combat.state.player_hp <= 0
                {
                    return Action::None;
                }
                let chaff = command == Command::Chaff;
                let count = |state: &tore_sim::combat::live::State| {
                    if chaff { state.chaff } else { state.flares }
                };
                let before = count(&self.combat.state);
                self.combat.command(
                    if chaff {
                        Live::ReleaseChaff
                    } else {
                        Live::ReleaseFlare
                    },
                    launcher,
                );
                // The retail cockpit messages, FA.EXE string table.
                let after = count(&self.combat.state);
                self.flight_ui.message(match (chaff, before) {
                    (true, 0) => "Out of chaff".to_string(),
                    (false, 0) => "Out of flares".to_string(),
                    (true, _) => format!("Chaff launched, {after} left"),
                    (false, _) => format!("Flare launched, {after} left"),
                });
                Action::None
            }
            Command::NextWeapon | Command::PreviousWeapon => {
                cycle_player_weapon(
                    &mut self.combat,
                    &self.flight,
                    &mut self.instruments,
                    &mut self.airport_nav_mode,
                    command == Command::NextWeapon,
                );
                Action::None
            }
            Command::Target | Command::TargetPrevious | Command::TargetVisual => {
                use tore_sim::combat::live::Command as Live;
                self.combat.command(
                    match command {
                        Command::Target => Live::Designate,
                        Command::TargetPrevious => Live::DesignatePrevious,
                        _ => Live::DesignateVisual,
                    },
                    combat::launcher(&self.flight),
                );
                Action::None
            }
            Command::RangeReset => {
                if self.combat.range {
                    self.combat.cancel();
                    self.combat.command(
                        tore_sim::combat::live::Command::ReplaceTarget,
                        combat::launcher(&self.flight),
                    );
                    self.combat
                        .refresh_render(&self.flight, self.ai_wings.as_ref());
                } else {
                    self.flight_ui
                        .message("Target reset is available only with --live-fire");
                }
                Action::None
            }
            Command::Effects(on) => Action::Effects(on),
            Command::ControlsOpen => {
                self.flight_ui.menu = true;
                self.input.context(true, self.focused);
                self.camera.keys.clear();
                self.combat.cancel();
                self.flight_clock.remainder = 0.;
                self.open_controls("Flight paused");
                Action::Click
            }
            Command::Toggle(switch) => {
                if switch == tore_input::Switch::Hook && !self.flight.hook_available() {
                    self.flight_ui.message("Hook unavailable for this aircraft");
                    return Action::None;
                }
                // Returning to the active radar channel comes first, so the
                // radar power switch is not spent leaving the passive page.
                if switch == tore_input::Switch::Radar && self.instruments.channel != 0 {
                    self.instruments.channel = 0;
                    return Action::Click;
                }
                self.input.queue(tore_input::PilotCommand::Toggle(switch));
                Action::None
            }
            Command::InstrumentSelect(slot) => {
                if self.instruments.select(slot) {
                    self.flight_ui
                        .message(format!("Instrument {} selected", slot + 1));
                } else {
                    self.flight_ui.message("Instrument slot unavailable");
                }
                Action::None
            }
            Command::InstrumentCycle(delta) => {
                if self.instruments.cycle_selection(delta) {
                    self.flight_ui.message(format!(
                        "Instrument {} selected",
                        self.instruments.selected + 1
                    ));
                }
                Action::None
            }
            Command::InstrumentControl(button) => {
                if self.instruments.control(self.instruments.selected, button) {
                    Action::Click
                } else {
                    self.flight_ui.message("Instrument control unavailable");
                    Action::None
                }
            }
            Command::CenterLook => {
                self.flight_ui.look = [0.; 2];
                self.input.center_head();
                Action::None
            }
            Command::StoreView => {
                self.view_rig
                    .save(self.flight_view, self.flight_ui.look, self.flight_ui.zoom);
                if !self.instruments.pages.contains(&3) {
                    self.instruments.toggle(3);
                }
                self.instruments.cameras.remove(&3);
                self.flight_ui.message("Other View saved");
                Action::Click
            }
            Command::View(_) | Command::ViewRelative(_, _) => {
                let (view, reference) = match command {
                    Command::View(view) => (view, flight_views::Reference::Player),
                    Command::ViewRelative(view, reference) => (view, reference),
                    _ => unreachable!(),
                };
                let scene = flight_views::Scene::new(
                    &self.flight,
                    &self.combat,
                    self.ai_wings.as_ref(),
                    false,
                );
                self.view_rig.observe(&scene);
                let mut candidate = self.view_rig.clone();
                candidate.select(reference);
                if let Err(reason) = candidate.camera(
                    view,
                    &scene,
                    self.hornet.camera(&self.flight, view, Default::default()),
                    [0.; 2],
                    1.,
                ) {
                    self.flight_ui.message(reason);
                    return Action::None;
                }
                self.view_rig = candidate;
                self.flight_view = view;
                self.flight_ui.look = [0.; 2];
                self.input.center_head();
                self.flight_ui.zoom = 1.;
                Action::Click
            }
            Command::WindowLayout => {
                self.instruments.toggle_layout();
                self.flight_ui
                    .message(if self.instruments.layout == instruments::Layout::Large {
                        "Large instruments: four corners"
                    } else {
                        "Small instruments: two bottom groups of three"
                    });
                Action::Click
            }
            Command::Panel(page) => {
                self.instruments.cancel_press();
                self.instruments.toggle(page);
                Action::Click
            }
            Command::ThrottlePreset(value, burner) => {
                self.input.queue(tore_input::PilotCommand::Throttle(value));
                self.input.queue(tore_input::PilotCommand::Set(
                    tore_input::Switch::Burner,
                    burner,
                ));
                Action::None
            }
            Command::ThrottleStep(delta) => {
                let (throttle, burner) = self
                    .input
                    .pending_throttle(self.flight.throttle, self.flight.burner);
                let (throttle, burner) = input::fa_throttle_step(throttle, burner, delta);
                self.input
                    .queue(tore_input::PilotCommand::Throttle(throttle));
                self.input.queue(tore_input::PilotCommand::Set(
                    tore_input::Switch::Burner,
                    burner,
                ));
                Action::None
            }
            Command::Waypoint(next) => {
                self.instruments.navigation.pending.push(usize::from(next));
                Action::None
            }
            Command::Range(delta) => {
                if self.instruments.pages.last() == Some(&0) {
                    let scales = tore_sim::sensors::passive::SCALE_LADDER_NMI.len() as i32 - 1;
                    self.instruments.rcs_range =
                        (self.instruments.rcs_range as i32 + delta).clamp(0, scales) as usize;
                } else {
                    // The RWR and radar share one range setting.
                    self.instruments.step_range(delta);
                }
                Action::Click
            }
            Command::Mode => {
                // Cycles the available sensor channels. Radar search and track
                // modes follow the selected display range automatically.
                self.instruments.cycle_channel();
                if !self
                    .combat
                    .state
                    .sensors
                    .available(self.instruments.controls().channel)
                {
                    self.instruments.cycle_channel();
                    self.flight_ui.message("No infrared sensor is installed.");
                }
                Action::Click
            }
            Command::SensorHistory => {
                self.instruments.history = !self.instruments.history;
                Action::Click
            }
            Command::SensorInfrared => {
                if self
                    .combat
                    .state
                    .sensors
                    .available(tore_sim::sensors::Channel::Infrared)
                {
                    self.instruments.channel = 1;
                } else {
                    self.flight_ui.message("No infrared sensor is installed.");
                }
                Action::Click
            }
        }
    }
    fn open_controls(&mut self, context: &'static str) {
        let mut editor = controls_editor::Editor::new(
            self.input.settings_profile(),
            self.input.devices.values().cloned().collect(),
            context,
        );
        editor.head_status = self.input.head_status();
        self.controls = Some(editor);
        self.mouse_look = None;
    }
    fn controls_result(&mut self, result: controls_editor::ResultAction) -> Action {
        use controls_editor::ResultAction;
        match result {
            ResultAction::None => Action::None,
            ResultAction::Changed => Action::Click,
            ResultAction::Save => {
                if let Some(editor) = &mut self.controls {
                    match self.input.save_settings(&editor.profile) {
                        Ok(()) => {
                            editor.message = "Controls saved and applied".into();
                            editor.saved();
                        }
                        Err(e) => editor.message = e,
                    }
                }
                Action::Click
            }
            ResultAction::Close => {
                self.controls = None;
                if self.screen == Screen::Flight {
                    self.flight_ui.controls_closed();
                }
                self.menu.state.cancel();
                Action::Click
            }
        }
    }
    fn open_graphics(&mut self, context: &'static str) {
        // The renderer exists whenever the menu is shown; without one every
        // level is offered and the renderer clamps on creation.
        let supported = graphics::AntiAliasing::ALL
            .map(|level| self.renderer.as_ref().is_none_or(|r| r.supports(level)));
        let current = self
            .renderer
            .as_ref()
            .map_or(self.graphics, |r| r.graphics());
        self.graphics_screen = Some(graphics_screen::Editor::new(current, supported, context));
        self.mouse_look = None;
    }
    fn graphics_result(&mut self, result: controls_editor::ResultAction) -> Action {
        use controls_editor::ResultAction;
        match result {
            ResultAction::None => Action::None,
            ResultAction::Changed => Action::Click,
            ResultAction::Save => {
                if let Some(editor) = &mut self.graphics_screen {
                    // Applied at once; renderers created later start from
                    // `self.graphics`, and the world rebuild on entering
                    // flight keeps the renderer's copy.
                    self.graphics = editor.apply(self.graphics_path.as_deref());
                    if let Some(renderer) = &mut self.renderer {
                        renderer.set_graphics(self.graphics);
                    }
                }
                Action::Click
            }
            ResultAction::Close => {
                self.graphics_screen = None;
                self.menu.state.cancel();
                Action::Click
            }
        }
    }
    fn action(&mut self, event_loop: &ActiveEventLoop, action: Action) {
        if action == Action::Exit {
            self.finished = true;
            event_loop.exit();
            return;
        }
        match action {
            Action::Controls => self.open_controls("Main menu"),
            Action::Graphics => self.open_graphics("Main menu"),
            Action::ReimportMedia => {
                // The pack on disk is still valid here, so the menu the player
                // is looking at becomes the locate screen's background.
                self.reimport = Some(self.menu.pixels.clone());
                self.finished = true;
                event_loop.exit();
                return;
            }
            Action::Music(on) => {
                self.menu.state.music = on;
            }
            Action::Effects(on) => {
                self.menu.state.effects = on;
                self.flight_ui.effects = on;
            }
            Action::Theater(index) => {
                if let Err(e) = self.combat.finish_recording() {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if let Some((code, _)) = self.world.catalog.get(index) {
                    match terrain::World::for_theater(&self.theater_resources, code) {
                        Ok(world) => {
                            self.ground_start = None;
                            let service =
                                match tore_sim::airport::Service::new(&world.airport_scene) {
                                    Ok(service) => service,
                                    Err(error) => {
                                        self.error = Some(std::io::Error::other(error).into());
                                        event_loop.exit();
                                        return;
                                    }
                                };
                            if let Some(renderer) = &mut self.renderer {
                                renderer.set_world(&world);
                                diagnostics::stage("aircraft graphics preparation");
                                renderer.prepare_aircraft(&self.hornet);
                                diagnostics::stage_done();
                            }
                            self.camera = terrain::Camera::for_world(&world);
                            self.world = world;
                            self.airport_service = service;
                        }
                        Err(error) => {
                            self.error = Some(error);
                            event_loop.exit();
                        }
                    }
                }
            }
            Action::Aircraft(index) => {
                if let Err(e) = self.combat.finish_recording() {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if let Some(&id) = tore_formats::aircraft::AircraftId::SELECTABLE.get(index) {
                    match aircraft::Airframe::load(&self.theater_resources, id) {
                        Ok(aircraft) => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.prepare_aircraft(&aircraft);
                            }
                            self.hornet = aircraft;
                            self.reset_weather();
                            self.flight = self.hornet.start(&self.world);
                            self.reset_vapor();
                            match combat::Combat::new(
                                &self.hornet,
                                &self.theater_resources,
                                self.combat.range,
                            )
                            .and_then(|mut c| {
                                c.reset(&mut self.flight)?;
                                if c.uses_normal_startup_defaults() {
                                    c.apply_startup_weapons();
                                }
                                c.add_airport_targets(&self.world.airport_scene)?;
                                Ok(c)
                            }) {
                                Ok(c) => {
                                    self.combat = c;
                                    self.airport_nav_mode = false;
                                    self.instruments.navigation = navigation::Navigation::default();
                                }
                                Err(e) => {
                                    self.error = Some(e);
                                    event_loop.exit();
                                    return;
                                }
                            }
                            self.previous_flight = self.flight.clone();
                            self.instruments.cameras.clear();
                            self.instruments.cancel_press();
                            self.flight_canvas = flight_canvas::FlightCanvas::default();
                        }
                        Err(error) => {
                            self.error = Some(error);
                            event_loop.exit();
                        }
                    }
                }
            }
            Action::QuickMission => {
                self.screen = Screen::Quick;
                self.menu.state.cancel();
            }
            Action::Mission => {
                let Some(id) = self.quick.player() else {
                    return;
                };
                let index = tore_formats::aircraft::AircraftId::SELECTABLE
                    .iter()
                    .position(|v| *v == id)
                    .unwrap();
                self.action(event_loop, Action::Theater(self.quick.theater_index()));
                if self.error.is_some() {
                    return;
                }
                self.action(event_loop, Action::Aircraft(index));
                if self.error.is_some() {
                    return;
                }
                let result = (|| -> AppResult<()> {
                    if self.quick.draft.values[18] == 0
                        || self
                            .quick
                            .ordnance
                            .as_ref()
                            .is_none_or(|o| o.loadout.aircraft != id)
                    {
                        let load = tore_sim::combat::loadout::Loadout::new(
                            &self.hornet.profile,
                            |name| {
                                self.theater_resources.get(name).cloned().ok_or_else(|| {
                                    std::io::Error::other(format!(
                                        "missing loadout resource {name}"
                                    ))
                                })
                            },
                        )?;
                        self.quick.ordnance =
                            Some(ordnance::Ordnance::new(load, &self.theater_resources)?);
                    }
                    let guns_only = self.quick.guns_only();
                    let o = self.quick.ordnance.as_mut().unwrap();
                    if guns_only {
                        o.loadout.restrict_to_guns();
                    }
                    o.visible = true;
                    o.message = None;
                    Ok(())
                })();
                if let Err(e) = result {
                    self.quick.notice = Some(e.to_string());
                } else if self.quick.draft.values[18] == 0 {
                    self.action(event_loop, Action::MissionFly);
                }
            }
            Action::MissionFly => {
                if let Some(message) = self.quick.unsupported() {
                    self.quick.ordnance.as_mut().unwrap().message = Some(message);
                    return;
                }
                let load = &self.quick.ordnance.as_ref().unwrap().loadout;
                if self.quick.guns_only()
                    && load
                        .configuration
                        .stations
                        .iter()
                        .zip(&load.quantities)
                        .any(|(s, n)| s.weapon.source != load.aircraft.gun() && *n > 0)
                {
                    self.quick.ordnance.as_mut().unwrap().message=Some("Guns only is selected. Unload other weapons or return to setup and change the restriction.".into());
                    return;
                }
                let altitude = [5000., 10000., 20000., 40000.][self.quick.draft.values[14]];
                let selected_ground = self.quick.ground_runway();
                if selected_ground.is_some()
                    && (!self.researched_flight || self.native_tables.is_some())
                {
                    self.quick.ordnance.as_mut().unwrap().message=Some("Ground start requires the researched flight model. Choose Airborne for this adapter.".into());
                    return;
                }
                let wings = match self.quick.wing_launches(self.enemy_skill) {
                    Ok(wings) => wings,
                    Err(error) => {
                        self.quick.ordnance.as_mut().unwrap().message = Some(error.to_string());
                        return;
                    }
                };
                // A ground start parks the player's whole wing on the runway;
                // the straight-flight fixtures keep only the player there.
                let parked = if self.ai_wings_enabled {
                    self.quick.player_wing_size()
                } else {
                    1
                };
                let mut start = self.hornet.start(&self.world);
                let ground_layout = match selected_ground {
                    Some(object) => {
                        let result = start
                            .enable_research(1)
                            .map_err(|e| -> Box<dyn Error> { e.into() })
                            .and_then(|()| {
                                quick_mission::ground_layout(&self.world, object, parked)
                            })
                            .and_then(|layout| {
                                quick_mission::place_on_runway(&self.world, &mut start, &layout, 0)
                                    .map(|()| layout)
                            });
                        match result {
                            Ok(layout) => Some(layout),
                            Err(error) => {
                                self.quick.ordnance.as_mut().unwrap().message =
                                    Some(error.to_string());
                                return;
                            }
                        }
                    }
                    None => None,
                };
                let layout = quick_mission::MissionLayout::plan(
                    &self.world,
                    &start,
                    ground_layout,
                    &ai_wings::enemy_group_offsets(&wings),
                    self.quick.separation_feet(),
                );
                let ground = f64::from(
                    self.world
                        .height(start.position[0] as f32, start.position[2] as f32),
                );
                // Only aircraft that start in the air need the altitude to
                // clear the ground; parked wingmen do not.
                let airborne_wings = if self.ai_wings_enabled {
                    wings.iter().any(|wing| {
                        !wing.is_empty()
                            && (layout.ground.is_none()
                                || wing.wing.side.is_enemy()
                                || wing.wing.index != 0)
                    })
                } else {
                    self.quick.dummy_wings().iter().any(|(_, count)| *count > 0)
                };
                if (selected_ground.is_none() || airborne_wings) && altitude < ground + 100. {
                    self.quick.ordnance.as_mut().unwrap().message = Some(format!(
                        "Airborne altitude must exceed {:.0} feet here. Choose a higher altitude.",
                        ground + 100.
                    ));
                    return;
                }
                let fuel = load.fuel_lbs;
                match combat::Combat::with_loadout(&self.hornet, &self.theater_resources, load) {
                    Ok(mut c) => {
                        if let Err(error) = c.add_airport_targets(&self.world.airport_scene) {
                            self.error = Some(error);
                            event_loop.exit();
                            return;
                        }
                        let populated = if self.ai_wings_enabled {
                            c.mission_aircraft(&wings, &layout, &self.theater_resources)
                        } else {
                            c.mission_layout = Some(layout.clone());
                            c.mission_dummies(
                                &self.quick.dummy_wings(),
                                layout.enemy.distance_ft,
                                &self.theater_resources,
                            )
                        };
                        if let Err(error) = populated {
                            self.quick.ordnance.as_mut().unwrap().message = Some(error.to_string());
                            return;
                        }
                        self.combat = c;
                        self.mission = Some((altitude, fuel));
                        self.ground_start = selected_ground;
                        // Rebuild the world on the mission's own weather choice
                        // before entering flight, so palette, clock and wind
                        // all start from it.
                        if let Some(index) = quick_mission::condition(self.quick.draft.values[15])
                            && let Err(error) = self.set_condition(index)
                        {
                            self.quick.ordnance.as_mut().unwrap().message = Some(error.to_string());
                            return;
                        }
                        self.action(event_loop, Action::FreeFlight);
                    }
                    Err(e) => self.quick.ordnance.as_mut().unwrap().message = Some(e.to_string()),
                }
            }
            Action::FreeFlight => {
                // A flight still recording is being restarted.
                let restarted = self.replay_recorder.is_some();
                self.finish_replay_recording("restart");
                if let Some(audio) = &self.audio {
                    audio.restart_flight();
                }
                // A fixed seed keeps headless runs deterministic.
                self.comms.restart(1);
                self.crew_voice = crew_voice::CrewVoice::new(&self.hornet.profile);
                self.radio = Default::default();
                if self.recorded_ticks > 0 {
                    self.finish_recording();
                }
                self.input.context(true, self.focused);
                self.reset_weather();
                self.flight = self.hornet.start(&self.world);
                if let Some((altitude, fuel)) = self.mission {
                    self.flight.position[1] = altitude;
                    self.flight.fuel = fuel;
                }
                // The accepted creator layout, reused unchanged on restart.
                let layout = self
                    .mission
                    .and(self.combat.mission_layout.clone())
                    .filter(|layout| layout.ground.is_some() == self.ground_start.is_some());
                let parked = layout.as_ref().and_then(|layout| layout.ground.clone());
                self.airfield_radio.reset(parked.as_ref().map(|g| g.runway));
                if let Some(layout) = layout.as_ref().filter(|l| l.player_turn != 0.) {
                    // Airborne: the whole scene turns so the enemy ahead stays
                    // on the map.
                    self.flight.yaw += layout.player_turn;
                    let basis = attitude::Basis::new(self.flight.yaw, 0., 0.);
                    self.flight.velocity = std::array::from_fn(|i| {
                        basis.forward[i] * self.flight.speed + self.world.wind()[i]
                    });
                }
                if let Some(object) = self.ground_start {
                    let pose = match &parked {
                        Some(ground) => Ok((ground.slots[0], ground.heading)),
                        None => quick_mission::runway_pose(&self.world, object),
                    };
                    let (position, heading) = match pose {
                        Ok(pose) => pose,
                        Err(error) => {
                            self.error = Some(error);
                            event_loop.exit();
                            return;
                        }
                    };
                    self.flight.position[0] = position[0];
                    self.flight.position[2] = position[2];
                    if self.mission.is_none() {
                        self.flight.position[1] = self.flight.position[1].max(position[1] + 5000.);
                    }
                    self.flight.yaw = heading;
                    let basis = attitude::Basis::new(heading, 0., 0.);
                    self.flight.velocity = std::array::from_fn(|i| {
                        basis.forward[i] * self.flight.speed + self.world.wind()[i]
                    });
                }
                if self.researched_flight
                    && let Err(error) = self.flight.enable_research(1)
                {
                    self.error = Some(error.into());
                    event_loop.exit();
                    return;
                }
                if let Some(tables) = &self.native_tables {
                    if self.combat.range || self.mission.is_some() {
                        self.error = Some(
                            "native research flight currently requires clean free flight".into(),
                        );
                        event_loop.exit();
                        return;
                    }
                    if let Err(e) = self.flight.enable_native(tables.clone(), 1) {
                        self.error = Some(e.into());
                        event_loop.exit();
                        return;
                    }
                }
                if let Err(e) = self.combat.reset(&mut self.flight) {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if self.combat.uses_normal_startup_defaults() {
                    self.combat.apply_startup_weapons();
                }
                let ground_airport = if let Some(object) = self.ground_start {
                    let placed = match &parked {
                        Some(ground) => {
                            quick_mission::place_on_runway(&self.world, &mut self.flight, ground, 0)
                                .map(|()| ground.airport)
                        }
                        None => {
                            quick_mission::apply_ground_start(&self.world, &mut self.flight, object)
                        }
                    };
                    match placed {
                        Ok(airport) => Some(airport),
                        Err(error) => {
                            self.error = Some(error);
                            event_loop.exit();
                            return;
                        }
                    }
                } else {
                    None
                };
                if let Err(error) = self.airport_service.reset(&self.world.airport_scene) {
                    self.error = Some(std::io::Error::other(error).into());
                    event_loop.exit();
                    return;
                }
                self.airport_nav_mode = ground_airport.is_some();
                self.combat.state.armed = !self.airport_nav_mode;
                self.airport_commands.clear();
                self.instruments.navigation = navigation::Navigation::default();
                if let Some(airport) = ground_airport {
                    self.airport_service.command(
                        &self.world.airport_scene,
                        airport_aircraft(&self.world, &self.flight, self.airport_nav_mode),
                        tore_sim::airport::Command::SelectAirport(airport),
                    );
                }
                // The AI bridge is built from the targets the existing spawner
                // just placed, so the AI aircraft start exactly where the
                // straight-flight fixtures would have started.
                self.ai_wings = None;
                self.wing_recipient = None;
                self.combat.ai_poses = false;
                if self.ai_wings_enabled && self.mission.is_some() {
                    let built = self
                        .quick
                        .wing_launches(self.enemy_skill)
                        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })
                        .and_then(|wings| {
                            // Home runways for every aircraft; a ground start
                            // parks the player's wingmen behind the player.
                            let airfields = ai_wings::Airfields::from_world(
                                &self.world,
                                parked.as_ref().map(quick_mission::GroundLayout::departure),
                            );
                            ai_wings::AiWings::build_mission(
                                &wings,
                                &self.combat.state.targets,
                                self.quick.guns_only(),
                                &self.theater_resources,
                                &airfields,
                            )
                        });
                    match built {
                        Ok(mut bridge) => {
                            bridge.apply_mission_preset(self.ai_mission, self.flight.position);
                            bridge.apply_group_objectives(
                                &self.quick.group_objectives,
                                self.flight.position,
                            );
                            bridge.apply_group_survival(&self.quick.group_must_survive);
                            bridge.mirror_pose_out(&mut self.combat.state.targets);
                            self.combat.ai_poses = !bridge.is_empty();
                            if bridge.is_empty() {
                                self.flight_ui
                                    .message("AI wings: no aircraft in this setup");
                            } else {
                                self.flight_ui
                                    .message(format!("AI wings: {} aircraft", bridge.len()));
                            }
                            self.ai_wings = Some(bridge);
                        }
                        Err(error) => {
                            self.error = Some(error);
                            event_loop.exit();
                            return;
                        }
                    }
                }
                // Draw from the placed start, including the AI's own poses.
                self.combat
                    .restart_render(&self.flight, self.ai_wings.as_ref());
                // Every flight records itself from this picture on.
                self.start_replay_recording();
                if restarted && let Some(recording) = &mut self.replay_recorder {
                    recording.note(tore_replay::Event::new(
                        tore_replay::vocab::kind::SYSTEM_RESTART,
                    ));
                }
                self.flight_music = flight_music::Observer::new(flight_music::home_base(
                    &self.world,
                    ground_airport,
                ));
                self.reset_vapor();
                self.previous_flight = self.flight.clone();
                self.g_effects = Default::default();
                self.flight_clock.remainder = 0.;
                self.flight_view = 0;
                self.view_rig = Default::default();
                let saved = preferences::Preferences::capture(
                    &self.flight_ui,
                    &self.instruments,
                    &self.menu.state,
                    self.fullscreen_preference,
                );
                self.flight_ui.reset_for_flight();
                saved.apply(
                    &mut self.flight_ui,
                    &mut self.instruments,
                    &mut self.menu.state,
                );
                self.flight_ui.effects = self.menu.state.effects;
                if self.ground_start.is_some() {
                    self.flight_ui
                        .message("Ground start: B releases brakes; PageUp adds throttle.");
                }
                if let Some(notice) = layout.as_ref().and_then(|l| l.notice()) {
                    self.flight_ui.message(notice);
                }
                self.screen = Screen::Flight;
                self.camera.keys.clear();
                self.combat.cancel();
                self.quick.cancel();
                self.frame_time = Instant::now();
            }
            Action::Back => {
                if self.screen == Screen::Flight && self.mission.is_some() {
                    let report =
                        debrief::capture(&self.combat, &self.flight, self.ai_wings.as_ref());
                    match debrief::Debrief::new(report, &self.theater_resources, None) {
                        Ok(debrief) => self.quick.debrief = Some(debrief),
                        Err(error) => self.quick.notice = Some(error.to_string()),
                    }
                    if let Some(ordnance) = &mut self.quick.ordnance {
                        ordnance.visible = false;
                    }
                }
                // After the debrief, before the wings go: the footer carries
                // the same result.
                if self.screen == Screen::Flight {
                    self.finish_replay_recording(if self.mission.is_some() {
                        "end mission"
                    } else {
                        "end flight"
                    });
                }
                self.ai_wings = None;
                self.wing_recipient = None;
                self.combat.ai_poses = false;
                if let Err(e) = self.combat.finish_recording() {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if let Some(renderer) = &mut self.renderer {
                    renderer.combat(&Default::default());
                }
                if self.recorded_ticks > 0 {
                    self.finish_recording();
                }
                self.input.context(true, self.focused);
                self.screen = if matches!(self.screen, Screen::Viewer | Screen::Flight) {
                    Screen::Quick
                } else {
                    Screen::Main
                };
                self.camera.keys.clear();
                self.combat.cancel();
                self.quick.cancel();
            }
            _ => {}
        }
        if let Some(renderer) = &self.renderer {
            renderer.window.set_title(&match self.screen {
                Screen::Flight => format!(
                    "T.O.R.E-Fighters - {} Free Flight",
                    self.hornet.profile.id.label()
                ),
                Screen::Main => "T.O.R.E-Fighters - Choose Activity".to_string(),
                Screen::Quick => "T.O.R.E-Fighters - Quick Mission Creator".to_string(),
                Screen::Viewer => format!(
                    "T.O.R.E-Fighters - {} Terrain Viewer",
                    self.world.theater.name
                ),
            });
        }
        if let Some(audio) = &self.audio {
            audio.scene(match self.screen {
                Screen::Flight => audio::music::Scene::Score(0),
                Screen::Main => audio::music::Scene::Main,
                _ => audio::music::Scene::Brief,
            });
            audio.action(action);
        }
        self.save_preferences();
        if let Some(renderer) = &self.renderer {
            renderer.window.set_cursor_visible(
                !(self.screen == Screen::Quick
                    && self
                        .quick
                        .ordnance
                        .as_ref()
                        .is_some_and(|o| o.visible && o.dragging())),
            );
            renderer.window.set_cursor(
                if (self.screen == Screen::Main && self.menu.state.hover.is_some())
                    || (self.screen == Screen::Quick
                        && self
                            .quick
                            .debrief
                            .as_ref()
                            .map_or(self.quick.hover.is_some(), |d| d.hovering()))
                {
                    CursorIcon::Pointer
                } else {
                    CursorIcon::Default
                },
            );
            renderer.window.request_redraw();
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let result = (|| {
            diagnostics::stage("game window creation");
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(match self.screen {
                            Screen::Flight => format!(
                                "T.O.R.E-Fighters - {} Free Flight",
                                self.hornet.profile.id.label()
                            ),
                            Screen::Main => "T.O.R.E-Fighters - Choose Activity".to_string(),
                            Screen::Quick => "T.O.R.E-Fighters - Quick Mission Creator".to_string(),
                            Screen::Viewer => format!(
                                "T.O.R.E-Fighters - {} Terrain Viewer",
                                self.world.theater.name
                            ),
                        })
                        // Fixed-size diagnostic windows preserve requested capture aspect ratios
                        // on compositors that otherwise tile/rescale newly created windows.
                        .with_resizable(!self.smoke_test)
                        // The inner size is also the size Alt-Enter returns to.
                        .with_inner_size(LogicalSize::new(self.window_size[0], self.window_size[1]))
                        .with_min_inner_size(LogicalSize::new(640.0, 480.0))
                        .with_fullscreen(fullscreen_attribute(self.fullscreen)),
                )?,
            );
            diagnostics::stage_done();
            pollster::block_on(Renderer::new(window, &self.world, self.graphics))
        })();
        match result {
            Ok(mut renderer) => {
                diagnostics::stage("aircraft graphics preparation");
                renderer.prepare_aircraft(&self.hornet);
                diagnostics::stage_done();
                renderer.window.request_redraw();
                self.renderer = Some(renderer);
                if std::mem::take(&mut self.launch_creator) {
                    let view = self.flight_view;
                    let reference = self.view_rig.reference;
                    let look = self.flight_ui.look;
                    let zoom = self.flight_ui.zoom;
                    self.action(event_loop, Action::Mission);
                    if self.smoke_test && self.mission.is_some() && self.error.is_none() {
                        let initial = (
                            self.flight.position,
                            self.flight.yaw,
                            self.flight.speed,
                            self.flight.gear,
                            self.flight.fuel,
                            self.flight.payload_lbs,
                            self.airport_service.selected(),
                        );
                        let targets: Vec<_> = self
                            .combat
                            .state
                            .targets
                            .iter()
                            .map(|t| (t.id, t.position, t.hp))
                            .collect();
                        self.action(event_loop, Action::FreeFlight);
                        let restarted = (
                            self.flight.position,
                            self.flight.yaw,
                            self.flight.speed,
                            self.flight.gear,
                            self.flight.fuel,
                            self.flight.payload_lbs,
                            self.airport_service.selected(),
                        );
                        let restarted_targets: Vec<_> = self
                            .combat
                            .state
                            .targets
                            .iter()
                            .map(|t| (t.id, t.position, t.hp))
                            .collect();
                        if initial != restarted || targets != restarted_targets {
                            self.error = Some(
                                "Quick Mission restart did not restore the accepted start".into(),
                            );
                            event_loop.exit();
                        } else {
                            println!("Quick Mission restart: PASS");
                        }
                    }
                    self.flight_view = view;
                    self.view_rig.select(reference);
                    self.flight_ui.look = look;
                    self.flight_ui.zoom = zoom;
                    if self.mission.is_none() && self.error.is_none() {
                        self.error =
                            Some("Quick Mission could not launch the selected setup".into());
                        event_loop.exit();
                    } else if self.error.is_none() {
                        println!(
                            "Quick Mission launch: ground={:?} player_position={:?} supported={} airborne_targets={} parked_targets={} enemy_nm={:.1}",
                            self.ground_start,
                            self.flight.position,
                            self.flight.supported_at(
                                self.world
                                    .surface(self.flight.position[0], self.flight.position[2])
                                    .height
                            ),
                            self.combat
                                .state
                                .targets
                                .iter()
                                .filter(|t| t.airborne && !t.on_ground)
                                .count(),
                            self.combat
                                .state
                                .targets
                                .iter()
                                .filter(|t| t.on_ground)
                                .count(),
                            self.combat
                                .mission_layout
                                .as_ref()
                                .map_or(0., |l| l.enemy.distance_ft / quick_mission::FEET_PER_NM)
                        );
                    }
                }
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.finished {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if renderer.window.id() != id {
            return;
        }
        let action = match event {
            WindowEvent::CloseRequested => Action::Exit,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                renderer.resize();
                self.menu.state.cancel();
                self.quick.cancel();
                self.instruments.cancel_press();
                self.flight_ui.cancel_press();
                if let Some(editor) = &mut self.controls {
                    editor.cancel_capture();
                }
                if let Some(editor) = &mut self.graphics_screen {
                    editor.cancel_press();
                }
                self.mouse_look = None;
                self.pointer = None;
                self.camera.keys.clear();
                self.combat.cancel();
                self.modifiers = ModifiersState::empty();
                Action::None
            }
            WindowEvent::CursorMoved { position, .. } => {
                let point = renderer.viewport().point(position.x, position.y);
                self.pointer = Some((position.x, position.y));
                if self.controls.is_some() || self.graphics_screen.is_some() {
                    return;
                }
                match self.screen {
                    Screen::Main => self.menu.state.pointer(point),
                    Screen::Quick => {
                        self.quick.pointer(point);
                        Action::None
                    }
                    Screen::Flight => {
                        if self.flight_ui.frozen() {
                            self.mouse_look = None;
                        }
                        if let Some(last) = self.mouse_look {
                            let profile = &self.input.resolver.profile;
                            // 1.0 sensitivity turns 2 radians per 1,000 pixels.
                            let k = 0.002 * profile.mouse_sensitivity;
                            let up = if profile.mouse_invert { 1. } else { -1. };
                            look::nudge(
                                &mut self.flight_ui.look,
                                [
                                    ((position.x - last.0) * k) as f32,
                                    ((position.y - last.1) * k * up) as f32,
                                ],
                                matches!(self.flight_view, 1 | 2),
                            );
                            self.mouse_look = Some((position.x, position.y));
                            renderer.window.request_redraw();
                        }
                        Action::None
                    }
                    Screen::Viewer => Action::None,
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.wheel += match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.,
                };
                let notches = self.wheel.trunc() as i32;
                self.wheel -= f64::from(notches);
                if notches == 0 {
                    return;
                }
                if let Some(editor) = &mut self.controls {
                    let result = editor.wheel(notches);
                    self.controls_result(result)
                } else if let Some(editor) = &mut self.graphics_screen {
                    let result = editor.wheel(notches);
                    self.graphics_result(result)
                } else if self.screen == Screen::Flight && !self.flight_ui.frozen() {
                    self.input.mouse_wheel(notches);
                    Action::None
                } else {
                    return;
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                self.quick.pointer(None);
                self.menu.state.pointer(None)
            }
            WindowEvent::Focused(true) => {
                self.focused = true;
                Action::None
            }
            WindowEvent::Focused(false) => {
                self.focused = false;
                self.input.context(true, false);
                if self.screen == Screen::Flight {
                    self.flight_ui.paused = true;
                }
                self.menu.state.cancel();
                self.quick.cancel();
                self.instruments.cancel_press();
                self.flight_ui.cancel_press();
                if let Some(editor) = &mut self.controls {
                    editor.cancel_capture();
                }
                if let Some(editor) = &mut self.graphics_screen {
                    editor.cancel_press();
                }
                self.mouse_look = None;
                self.pointer = None;
                self.camera.keys.clear();
                self.combat.cancel();
                self.modifiers = ModifiersState::empty();
                Action::None
            }
            WindowEvent::MouseInput { state, button, .. } if self.controls.is_some() => {
                let point = self
                    .pointer
                    .and_then(|(x, y)| renderer.viewport().point(x, y));
                let pressed = state == ElementState::Pressed;
                let editor = self.controls.as_mut().expect("guarded");
                let result = match (button, mouse_control(button)) {
                    (MouseButton::Left, _) => editor.pointer(point, pressed),
                    (_, Some(control)) if pressed => editor.mouse(control),
                    _ => controls_editor::ResultAction::None,
                };
                self.controls_result(result)
            }
            WindowEvent::MouseInput { state, button, .. } if self.graphics_screen.is_some() => {
                let point = self
                    .pointer
                    .and_then(|(x, y)| renderer.viewport().point(x, y));
                let editor = self.graphics_screen.as_mut().expect("guarded");
                let result = if button == MouseButton::Left {
                    editor.pointer(point, state == ElementState::Pressed)
                } else {
                    controls_editor::ResultAction::None
                };
                self.graphics_result(result)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } if self.screen == Screen::Quick => self.quick.right(state == ElementState::Pressed),
            WindowEvent::MouseInput { state, button, .. }
                if self.screen == Screen::Flight && button != MouseButton::Left =>
            {
                let pressed = state == ElementState::Pressed;
                if button == MouseButton::Right && self.input.resolver.profile.mouse_look {
                    self.mouse_look = self.pointer.filter(|_| pressed && !self.flight_ui.frozen());
                } else if let Some(control) = mouse_control(button) {
                    self.input.mouse_button(control, pressed);
                }
                Action::None
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if self.screen == Screen::Flight && self.flight_ui.map.open && !self.flight_ui.menu
                {
                    self.flight_ui.map.pointer(
                        self.pointer
                            .and_then(|(x, y)| renderer.viewport().point(x, y)),
                        state == ElementState::Pressed,
                    );
                    Action::None
                } else if self.screen == Screen::Flight && self.flight_ui.menu {
                    let command = self.flight_ui.pointer(
                        &self.hornet.flight_menu,
                        self.pointer
                            .and_then(|(x, y)| renderer.viewport().point(x, y)),
                        state == ElementState::Pressed,
                    );
                    self.camera.keys.clear();
                    self.combat.cancel();
                    self.frame_time = Instant::now();
                    self.flight_command(command)
                } else if self.screen == Screen::Flight
                    && (self.view_rig.cockpit(self.flight_view)
                        && self.flight_ui.weapon_diagnostics_shown(self.flight_view))
                    && !self.flight_ui.frozen()
                    && self.pointer.is_some_and(|p| {
                        let size = [
                            f64::from(renderer.window.inner_size().width),
                            f64::from(renderer.window.inner_size().height),
                        ];
                        weapon_hud::mode_hit(p, size) || weapon_hud::release_hit(p, size)
                    })
                {
                    if state == ElementState::Pressed {
                        self.combat.command(
                            if self.pointer.is_some_and(|p| {
                                weapon_hud::release_hit(
                                    p,
                                    [
                                        f64::from(renderer.window.inner_size().width),
                                        f64::from(renderer.window.inner_size().height),
                                    ],
                                )
                            }) {
                                tore_sim::combat::live::Command::ClearDesignation
                            } else {
                                tore_sim::combat::live::Command::ToggleSeekerMode
                            },
                            combat::launcher(&self.flight),
                        );
                    }
                    Action::Click
                } else if self.screen == Screen::Flight {
                    let hit = self.instruments.screen_pointer(
                        self.pointer,
                        [
                            renderer.window.inner_size().width as f64,
                            renderer.window.inner_size().height as f64,
                        ],
                        state == ElementState::Pressed,
                    );
                    // The simulation revalidates the requested identity, so a
                    // click can never select a target it does not observe.
                    if let Some(id) = self.instruments.designation.take() {
                        self.combat.command(
                            tore_sim::combat::live::Command::DesignateTarget(id),
                            combat::launcher(&self.flight),
                        );
                    }
                    if hit { Action::Click } else { Action::None }
                } else if self.screen == Screen::Viewer {
                    Action::None
                } else if self.screen == Screen::Quick {
                    if state == ElementState::Pressed {
                        self.quick.shift = self.modifiers.shift_key();
                        self.quick.down()
                    } else {
                        self.quick.up()
                    }
                } else if state == ElementState::Pressed {
                    self.menu.state.down();
                    Action::None
                } else {
                    self.menu.state.up()
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                if !modifiers.state().is_empty() {
                    self.combat.cancel();
                }
                if self.screen == Screen::Flight {
                    look::modifiers_changed(&mut self.camera.keys, modifiers.state());
                }
                self.modifiers = modifiers.state();
                Action::None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let mut name = match &event.logical_key {
                    Key::Named(k) => format!("{k:?}"),
                    Key::Character(c) => c.to_ascii_lowercase(),
                    _ => String::new(),
                };
                if self.screen == Screen::Flight {
                    name = flight_key(event.physical_key, &name);
                }
                // Alt-Enter switches window mode on every screen, before any
                // screen claims the key. F11 is not used: it already opens the
                // flight keyboard help (docs/FLIGHT-CONTROLS.md).
                if name == "Enter"
                    && self.modifiers.alt_key()
                    && event.state == ElementState::Pressed
                {
                    if !event.repeat {
                        self.toggle_fullscreen();
                    }
                    return;
                }
                // The controls screen takes every key press while it is open,
                // with the same physical key names flight uses for capture.
                if event.state == ElementState::Pressed
                    && let Some(editor) = &mut self.controls
                {
                    if event.repeat && editor.capturing() {
                        return;
                    }
                    let name = flight_key(event.physical_key, &name);
                    let result = editor.key(
                        &name,
                        self.modifiers.shift_key(),
                        self.modifiers.control_key(),
                        self.modifiers.alt_key(),
                    );
                    let action = self.controls_result(result);
                    self.action(event_loop, action);
                    return;
                }
                if event.state == ElementState::Pressed
                    && let Some(editor) = &mut self.graphics_screen
                {
                    let result = editor.key(&name, self.modifiers.shift_key());
                    let action = self.graphics_result(result);
                    self.action(event_loop, action);
                    return;
                }
                if self.screen == Screen::Flight
                    && !(event.state == ElementState::Pressed
                        && self.flight_ui.map.open
                        && matches!(
                            name.as_str(),
                            "m" | "Escape"
                                | "+"
                                | "="
                                | "-"
                                | "_"
                                | "ArrowLeft"
                                | "ArrowRight"
                                | "ArrowUp"
                                | "ArrowDown"
                                | "Home"
                        ))
                    && (event.state == ElementState::Released
                        || (!(self.flight_ui.menu
                            && matches!(
                                name.as_str(),
                                "Escape"
                                    | "Tab"
                                    | "ArrowUp"
                                    | "ArrowDown"
                                    | "ArrowLeft"
                                    | "ArrowRight"
                                    | "Enter"
                                    | "Space"
                            ))))
                    && (if event.repeat {
                        self.input.claimed(&name)
                    } else {
                        self.input
                            .key(&name, event.state == ElementState::Pressed, self.modifiers)
                    })
                {
                    return;
                }
                if self.screen == Screen::Flight && name == "Space" {
                    let blocked =
                        self.flight_ui.frozen() || !self.focused || !self.modifiers.is_empty();
                    self.combat.input.space(
                        event.state == ElementState::Pressed,
                        event.repeat,
                        blocked,
                    );
                    if !self.flight_ui.menu {
                        return;
                    }
                }
                if matches!(self.screen, Screen::Viewer | Screen::Flight)
                    && event.state == ElementState::Released
                {
                    self.camera.keys.remove(&name);
                    self.camera.keys.remove(&format!("Look{name}"));
                    return;
                }
                if event.state != ElementState::Pressed {
                    return;
                }
                if (self.modifiers.super_key() && name.eq_ignore_ascii_case("q"))
                    || (self.modifiers.alt_key() && name == "F4")
                {
                    Action::Exit
                } else if self.screen == Screen::Flight {
                    let map_before = self.flight_ui.map.open;
                    let before = self.flight_ui.frozen();
                    let command = if event.repeat || self.modifiers.super_key() {
                        flight_ui::Command::None
                    } else {
                        self.flight_ui.key(
                            &name,
                            self.modifiers.shift_key(),
                            self.modifiers.control_key(),
                            self.modifiers.alt_key(),
                            &self.hornet.flight_menu,
                        )
                    };
                    if self.flight_ui.map.open != map_before {
                        self.flight_ui.map.cancel_press();
                        self.camera.keys.clear();
                        self.combat.cancel();
                        self.instruments.cancel_press();
                    }
                    if self.flight_ui.frozen() || before != self.flight_ui.frozen() {
                        self.camera.keys.clear();
                        self.combat.cancel();
                        self.instruments.cancel_press();
                        self.flight_clock.remainder = 0.;
                        self.previous_flight.clone_from(&self.flight);
                        self.frame_time = Instant::now();
                    } else if !self.flight_ui.map.open {
                        look::press(&mut self.camera.keys, &name, self.modifiers);
                    }
                    self.flight_command(command)
                } else if self.screen == Screen::Viewer {
                    if name == "Escape" {
                        Action::Back
                    } else {
                        self.camera.keys.insert(name);
                        Action::None
                    }
                } else if event.repeat {
                    Action::None
                } else if self.screen == Screen::Quick {
                    self.quick.key(
                        if name == "Space" { " " } else { &name },
                        self.modifiers.shift_key(),
                    )
                } else {
                    self.menu.state.key(
                        if name == "Space" { " " } else { &name },
                        self.modifiers.shift_key(),
                    )
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();
                let mut simulation_ms = 0.;
                if self.screen == Screen::Flight
                    && let Some(view) = self.performance.view()
                {
                    self.flight_view = view;
                }
                if self.screen == Screen::Flight && self.performance.active() {
                    // Explicit bounded benchmark only: desktop automation may steal focus.
                    self.flight_ui.paused = false;
                }
                let mut animating = match self.screen {
                    Screen::Main => {
                        let animating = self.menu.render();
                        if let Some(editor) = &self.controls {
                            editor.draw(&mut self.menu.pixels, &self.hornet.font);
                        }
                        if let Some(editor) = &self.graphics_screen {
                            editor.draw(&mut self.menu.pixels, &self.hornet.font);
                        }
                        animating
                    }
                    Screen::Quick => self.quick.render(
                        &mut self.menu.pixels,
                        &self.menu.quick_sprites,
                        &self.world,
                    ),
                    Screen::Flight => {
                        self.world.no_sun_whiteout = self.flight_ui.cheats.no_sun_whiteout;
                        let now = Instant::now();
                        let elapsed = (now - self.frame_time).as_secs_f64().min(0.25);
                        let steps = self.flight_ui.steps(&mut self.flight_clock, elapsed);
                        self.frame_time = now;
                        if let Some(audio) = &self.audio {
                            audio.seeker(
                                self.combat
                                    .state
                                    .seeker_tone(combat::launcher(&self.flight)),
                            );
                            audio.pause_flight(self.flight_ui.frozen());
                        }
                        // Scope channel, display range and history are player
                        // controls, applied as a simulation input so replay
                        // reproduces every change and the labels never lag.
                        self.flight.sensors = self.instruments.controls();
                        self.flight.cheats = self.flight_ui.cheats;
                        self.combat.state.cheats = self.flight_ui.cheats;
                        if let Some(wings) = &mut self.ai_wings {
                            self.combat.state.friendlies = wings.friendly_ids();
                            wings.set_enemy_skill(self.flight_ui.cheats.enemy_ai);
                            wings.set_guns_only(self.flight_ui.cheats.guns_only);
                        }
                        // Pauses, time compression and cheats, noted as they happen.
                        if let Some(recording) = &mut self.replay_recorder {
                            recording.session(&self.flight_ui);
                        }
                        for _ in 0..steps {
                            if let Some(recording) = &mut self.replay_recorder {
                                recording.start_tick(Some(&mut self.flight_ui), &mut self.combat);
                            }
                            for button in std::mem::take(&mut self.instruments.weapon_controls) {
                                cycle_player_weapon(
                                    &mut self.combat,
                                    &self.flight,
                                    &mut self.instruments,
                                    &mut self.airport_nav_mode,
                                    button == 1,
                                );
                            }
                            self.instruments.navigation.refresh(
                                &self.world.airport_scene,
                                &self.airport_service,
                                self.flight.position,
                            );
                            for button in std::mem::take(&mut self.instruments.navigation.pending) {
                                if let Some(id) = self.instruments.navigation.control(button) {
                                    self.airport_commands.push(flight_ui::Command::Airport(
                                        tore_sim::airport::Command::SelectAirport(id),
                                    ));
                                }
                            }
                            for command in std::mem::take(&mut self.airport_commands) {
                                match command {
                                    flight_ui::Command::AirportNav => {
                                        self.airport_nav_mode = !self.airport_nav_mode;
                                        self.combat.cancel();
                                        self.combat.command(
                                            if self.airport_nav_mode {
                                                tore_sim::combat::live::Command::SelectNav
                                            } else {
                                                tore_sim::combat::live::Command::NextSelection
                                            },
                                            combat::launcher(&self.flight),
                                        );
                                        if let Some(recorder) = &mut self.combat.recorder {
                                            recorder.record(
                                                if self.airport_nav_mode {
                                                    "airport-nav:1"
                                                } else {
                                                    "airport-nav:0"
                                                },
                                                combat::launcher(&self.flight),
                                            );
                                        }
                                        self.flight_ui.message(if self.airport_nav_mode {
                                            "Navigation mode selected"
                                        } else {
                                            "Navigation mode off"
                                        });
                                    }
                                    flight_ui::Command::Airport(command) => {
                                        if let Some(recorder) = &mut self.combat.recorder {
                                            recorder.record(
                                                &combat_tape::airport_command_name(command),
                                                combat::launcher(&self.flight),
                                            );
                                        }
                                        let aircraft = airport_aircraft(
                                            &self.world,
                                            &self.flight,
                                            self.airport_nav_mode,
                                        );
                                        for event in self.airport_service.command(
                                            &self.world.airport_scene,
                                            aircraft,
                                            command,
                                        ) {
                                            if let tore_sim::airport::Event::Reply(reply) = event {
                                                self.airfield_radio.reply(&reply);
                                                self.comms.cancel_airport();
                                                self.comms
                                                    .spoken(self.combat.state.tick() as f64 / 120.);
                                                if let Some(recording) = &mut self.replay_recorder {
                                                    recording.tower(
                                                        &airport_reply(&self.world, &reply),
                                                        airport_reply_audio(&reply),
                                                    );
                                                }
                                                self.flight_ui
                                                    .message(airport_reply(&self.world, &reply));
                                                if let Some(audio) = &self.audio {
                                                    if let Some(stem) = airport_reply_audio(&reply)
                                                    {
                                                        audio.airport_radio(&[stem]);
                                                    } else {
                                                        audio.cancel_airport_radio();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    _ => unreachable!("only airport commands are queued"),
                                }
                            }
                            self.previous_flight.clone_from(&self.flight);
                            let (pilot, _) =
                                self.input.frame(&self.camera.keys, self.flight.throttle);
                            if let Some(recording) = &mut self.input_recording {
                                self.recorded_ticks += 1;
                                if let Err(error) = tore_input::recording::write_frame(
                                    recording,
                                    self.recorded_ticks,
                                    &pilot,
                                ) {
                                    self.error = Some(error.into());
                                    event_loop.exit();
                                    return;
                                }
                            }
                            self.flight
                                .step_surface(&pilot, |x, z| self.world.surface(x, z));
                            if self.flight.native.is_none()
                                && self
                                    .world
                                    .solid_contact(
                                        self.previous_flight.position,
                                        self.flight.position,
                                        self.combat
                                            .state
                                            .targets
                                            .iter()
                                            .filter(|target| target.hp > 0)
                                            .map(|target| target.id),
                                    )
                                    .is_some()
                            {
                                if self.flight_ui.cheats.no_crashes {
                                    self.flight.rebound(self.previous_flight.position);
                                } else {
                                    self.flight.crashed = true;
                                }
                            }
                            if let Some(error) = self.flight.native_fault() {
                                self.flight_ui.message(error.to_owned());
                                self.flight_ui.paused = true;
                                log::warn!("{error}");
                                break;
                            }
                            // Weather shares the authoritative tick; pausing simply
                            // stops calling it, with no elapsed-time catch-up.
                            let scene = flight_views::Scene::new(
                                &self.flight,
                                &self.combat,
                                self.ai_wings.as_ref(),
                                false,
                            );
                            let weather_view = self
                                .view_rig
                                .camera(
                                    self.flight_view,
                                    &scene,
                                    self.hornet.camera(
                                        &self.flight,
                                        self.flight_view,
                                        Default::default(),
                                    ),
                                    look::combine(
                                        self.flight_ui.look,
                                        self.head_look,
                                        matches!(self.flight_view, 1 | 2),
                                    ),
                                    self.flight_ui.zoom,
                                )
                                .unwrap_or_else(|_| {
                                    self.hornet.camera(&self.flight, 0, Default::default())
                                });
                            self.world.step_weather(self.flight.speed, &weather_view);
                            self.world.step_view_weather(
                                &mirrors::camera(&self.flight),
                                self.flight.speed,
                            );
                            self.world.step_view_weather(
                                &self.hornet.panel_camera(&self.flight, 2),
                                self.flight.speed,
                            );
                            let scene = flight_views::Scene::new(
                                &self.flight,
                                &self.combat,
                                self.ai_wings.as_ref(),
                                false,
                            );
                            self.view_rig.observe(&scene);
                            if let Ok(camera) = self.view_rig.other_camera(
                                &scene,
                                self.hornet.camera(
                                    &self.flight,
                                    self.view_rig.other_view(),
                                    Default::default(),
                                ),
                            ) {
                                self.world.step_view_weather(&camera, self.flight.speed);
                            }
                            if let Some(camera) = self.combat.target_camera(&self.flight) {
                                self.world.step_view_weather(&camera, self.flight.speed);
                            }
                            let turbulence_cue = step_turbulence(
                                &mut self.turbulence,
                                &mut self.turbulence_rng,
                                &mut self.flight,
                                &self.world,
                                !self.flight_ui.cheats.no_turbulence,
                            );
                            self.g_effects.step(
                                self.flight.g,
                                !self.flight_ui.cheats.no_g_effects
                                    && !self.flight.crashed
                                    && self.flight.native.is_none(),
                            );
                            if let Some(points) = self.hornet.streamer_points(&self.flight) {
                                self.vapor.step(self.world.weather.ticks(), points);
                            }
                            if let Some(cue) = turbulence_cue {
                                self.input.feedback(cue);
                            }
                            if let Some(audio) = &self.audio {
                                audio.controls(&self.previous_flight, &self.flight);
                            }
                            self.combat.controller.space(
                                self.input.resolver.held("fire"),
                                false,
                                false,
                            );
                            if let Some(recorder) = &mut self.combat.recorder {
                                let airport = airport_aircraft(
                                    &self.world,
                                    &self.flight,
                                    self.airport_nav_mode,
                                );
                                recorder.record(
                                    &format!(
                                        "airport-state:{}:{}:{}",
                                        u8::from(airport.nav_mode),
                                        u8::from(airport.gear_down),
                                        u8::from(airport.supported)
                                    ),
                                    combat::launcher(&self.flight),
                                );
                            }
                            let events = match self.combat.step(&mut self.flight, &self.world) {
                                Ok(events) => events,
                                Err(e) => {
                                    self.error = Some(e);
                                    event_loop.exit();
                                    return;
                                }
                            };
                            if let Some(view) = self.flight_ui.pilot_death_view(
                                self.previous_flight.systems.pilot.dead
                                    || self.previous_flight.escape.is_some(),
                                self.flight.systems.pilot.dead || self.flight.escape.is_some(),
                            ) {
                                self.flight_view = view;
                                self.view_rig.select(flight_views::Reference::Player);
                            }
                            for airport_event in self.airport_service.synchronize_health(
                                self.combat
                                    .state
                                    .targets
                                    .iter()
                                    .filter(|target| {
                                        target.role
                                            == tore_sim::combat::missiles::TargetRole::Surface
                                    })
                                    .map(|target| (target.id, target.hp)),
                            ) {
                                if matches!(
                                    airport_event,
                                    tore_sim::airport::Event::ClearanceInvalidated(_)
                                ) {
                                    self.flight_ui
                                        .message("Landing clearance cancelled: runway unavailable");
                                    if let Some(audio) = &self.audio {
                                        audio.cancel_airport_radio();
                                    }
                                }
                            }
                            for event in self.airport_service.step(
                                &self.world.airport_scene,
                                airport_aircraft(&self.world, &self.flight, self.airport_nav_mode),
                            ) {
                                if matches!(event, tore_sim::airport::Event::LandingComplete { .. })
                                {
                                    self.flight_ui.message("Landing complete");
                                }
                            }
                            // Manual p.65: the player always lands first and
                            // other aircraft hold at marshal. The retail
                            // condition (gear, height, speed and range) is
                            // re-evaluated every tick, so climbing away,
                            // raising the gear, a crash or restart release it.
                            if let Some(wings) = &mut self.ai_wings {
                                let [x, _, z] = self.flight.position;
                                wings.update_player_landing(
                                    &self.world.airport_scene,
                                    &self.airport_service,
                                    &self.flight,
                                    self.world.surface(x, z).height,
                                );
                            }
                            for message in self.flight.systems.messages.drain(..) {
                                self.flight_ui.message(message);
                            }
                            let mut releases = Vec::new();
                            for event in &events {
                                use tore_sim::combat::live::Event;
                                if let Some(cue) =
                                    combat::feedback(event, self.combat.state.configuration())
                                {
                                    self.input.feedback(cue);
                                }
                                match event {
                                    Event::Jolt(jolt) => match jolt.target {
                                        None => self.flight.jolt_from(jolt.from, jolt.strength),
                                        Some(id) => {
                                            if let Some(wings) = &mut self.ai_wings {
                                                wings.jolt(id, jolt.from, jolt.strength);
                                            }
                                        }
                                    },
                                    Event::PlayerDamaged(_)
                                    | Event::Hit(_)
                                    | Event::Ground
                                    | Event::Destroyed(_)
                                    | Event::PilotKilled
                                    | Event::SubsystemDamaged(_)
                                    | Event::Defeated(_)
                                    | Event::TrackLost(_)
                                    | Event::SeekerActivated(_)
                                    | Event::Pitbull(_) => {}
                                    Event::PlayerDestroyed => {
                                        self.flight.crashed = true;
                                    }
                                    Event::Fired(i) => {
                                        if let Some(name) =
                                            self.combat.state.configuration().stations[*i]
                                                .weapon
                                                .fire_sound
                                                .as_deref()
                                        {
                                            releases.push((name.to_string(), *i));
                                        }
                                    }
                                    Event::PlayerGroundImpact => {
                                        self.flight_ui.message("Your aircraft exploded on impact");
                                    }
                                    Event::Airburst(id) => {
                                        self.flight_ui.message(if *id == 0 {
                                            "Your aircraft exploded"
                                        } else {
                                            "Destroyed aircraft exploded"
                                        });
                                    }
                                }
                            }
                            // One AI tick per combat tick, immediately after it,
                            // so the AI reads the damage combat just applied and
                            // then writes the authoritative pose back.
                            if let Some(mut bridge) = self.ai_wings.take() {
                                let stepped =
                                    bridge.step(&mut self.combat.state, &self.flight, &self.world);
                                for (id, message, friendly) in bridge.ejection_events.drain(..) {
                                    if let Some(recording) = &mut self.replay_recorder {
                                        recording.wing_ejection(id, &message, friendly);
                                    }
                                    self.flight_ui.message(message);
                                    if friendly && let Some(audio) = &self.audio {
                                        audio.wingman_ejected();
                                    }
                                }
                                let message = bridge.take_message();
                                self.ai_wings = Some(bridge);
                                if let Err(error) = stepped {
                                    self.error = Some(error);
                                    event_loop.exit();
                                    return;
                                }
                                if let Some(message) = message {
                                    self.flight_ui.message(message);
                                }
                            }
                            // The tick's picture: combat and the AI have both
                            // written their poses for it.
                            self.combat
                                .advance_render(&self.flight, self.ai_wings.as_ref());
                            // The mission recording reads the same picture,
                            // before the radio drains this tick's strikes.
                            if let Some(recording) = &mut self.replay_recorder {
                                let outcomes = self.combat.state.ledger.take_outcomes();
                                recording.begin(replay::recorder::Tick {
                                    snapshot: self.combat.render_snapshot(),
                                    combat: &self.combat,
                                    flight: &self.flight,
                                    previous: &self.previous_flight,
                                    pilot: &pilot,
                                    wings: self.ai_wings.as_ref(),
                                    world: &self.world,
                                    events: &events,
                                    outcomes: &outcomes,
                                });
                            }

                            if (self.flight.crashed
                                || self.flight.escape.is_some()
                                || self.flight.systems.pilot.dead)
                                && let Some(audio) = &self.audio
                            {
                                audio.cancel_airport_radio();
                            }
                            self.airfield_radio.step(
                                self.combat.state.tick() as f64 / 120.,
                                &self.phrases,
                                &mut self.comms,
                                &self.flight,
                                &self.world,
                                &self.airport_service,
                                self.ai_wings.as_ref(),
                            );
                            self.crew_voice.step_host(
                                &mut self.comms,
                                &self.phrases,
                                &crew_voice::Host {
                                    flight: &self.flight,
                                    combat: &self.combat.state,
                                    wings: self.ai_wings.as_ref(),
                                    world: &self.world,
                                },
                            );
                            radio_calls::step(
                                &mut self.radio,
                                &mut self.comms,
                                &self.phrases,
                                comms::crew(&self.hornet.profile),
                                &events,
                                &mut self.combat.state,
                                self.ai_wings.as_mut(),
                                &self.flight,
                            );
                            let delivered = deliver_radio(
                                &mut self.comms,
                                &mut self.flight_ui,
                                self.audio.as_ref(),
                                self.combat.state.tick() as f64 / 120.,
                            );
                            if let Some(recording) = &mut self.replay_recorder {
                                recording.radio(&delivered, comms::crew(&self.hornet.profile));
                            }

                            let danger = tore_sim::ejection::assess(&self.flight, |x, z| {
                                f64::from(self.world.height(x as f32, z as f32))
                            })
                            .is_some();
                            if let Some(audio) = &self.audio {
                                audio.ejection(&self.previous_flight, &self.flight, danger);
                            }
                            if let Some(audio) = &self.audio {
                                // The debrief's own evaluator, so the success
                                // music and the debrief always agree.
                                let succeeded = || {
                                    debrief::capture(
                                        &self.combat,
                                        &self.flight,
                                        self.ai_wings.as_ref(),
                                    )
                                    .outcome
                                        == debrief::Outcome::Success
                                };
                                let mission = (self.mission.is_some() && self.ai_wings.is_some())
                                    .then_some(&succeeded as &dyn Fn() -> bool);
                                let music = self.flight_music.step(
                                    &self.flight,
                                    &self.combat.state,
                                    &events,
                                    self.ai_wings.as_ref(),
                                    &self.world,
                                    mission,
                                );
                                audio.situation(&music.inputs, music.now);
                                // Addressed to the player's flight; the label
                                // (crew or YOU) is unresolved in retail, so this
                                // choice is fitted. The mission result comes
                                // about 2 seconds after it is decided (native).
                                let label = comms::crew(&self.hornet.profile)
                                    .map_or("YOU", comms::Crew::label);
                                for stem in music.radio {
                                    let delay = if stem == ai_wings::outcome::MISSION_ACCOMPLISHED {
                                        2.
                                    } else {
                                        0.
                                    };
                                    self.comms.send(
                                        self.combat.state.tick() as f64 / 120.,
                                        comms::Call::new(
                                            label,
                                            comms::Phrase::stem(&self.phrases, stem),
                                            comms::Kind::Important,
                                        )
                                        .after(delay),
                                    );
                                }
                            }
                            // Audio observes authoritative poses and consumes each emission once.
                            let emissions = self.combat.state.take_sound_events();
                            if let Some(audio) = &self.audio {
                                let scene = flight_views::Scene::new(
                                    &self.flight,
                                    &self.combat,
                                    self.ai_wings.as_ref(),
                                    false,
                                );
                                let listener_camera = self
                                    .view_rig
                                    .clone()
                                    .camera(
                                        self.flight_view,
                                        &scene,
                                        self.hornet.camera(
                                            &self.flight,
                                            self.flight_view,
                                            Default::default(),
                                        ),
                                        look::combine(
                                            self.flight_ui.look,
                                            self.head_look,
                                            matches!(self.flight_view, 1 | 2),
                                        ),
                                        self.flight_ui.zoom,
                                    )
                                    .unwrap_or_else(|_| {
                                        self.hornet.camera(&self.flight, 0, Default::default())
                                    });
                                let basis = tore_sim::attitude::Basis::new(
                                    f64::from(listener_camera.yaw),
                                    f64::from(listener_camera.pitch),
                                    -f64::from(listener_camera.roll),
                                );
                                audio.spatial_tick(
                                    tore_sim::acoustics::Listener {
                                        position: listener_camera.position.map(f64::from),
                                        right: basis.right,
                                        view: self.flight_view,
                                        external: !self.view_rig.cockpit(self.flight_view),
                                    },
                                    &audio::spatial_sources(&self.combat.state, &self.flight),
                                    &emissions,
                                    &releases
                                        .iter()
                                        .map(|(name, _)| name.as_str())
                                        .collect::<Vec<_>>(),
                                    self.flight.position,
                                );
                            }
                            if let Some(recording) = &mut self.replay_recorder {
                                let stations = &self.combat.state.configuration().stations;
                                let releases: Vec<(&str, &tore_formats::weapons::Weapon)> =
                                    releases
                                        .iter()
                                        .map(|(name, i)| (name.as_str(), &stations[*i].weapon))
                                        .collect();
                                recording.sounds(&emissions, &releases);
                            }

                            if self.flight.crashed
                                && !self.previous_flight.crashed
                                && self.flight.escape.is_none()
                            {
                                self.input.feedback(tore_input::FeedbackEvent::Crash);
                            }
                            if self.flight.afterburner_active()
                                && !self.previous_flight.afterburner_active()
                            {
                                self.input
                                    .feedback(tore_input::FeedbackEvent::AfterburnerEngaged);
                            }
                            self.input
                                .afterburner_feedback(self.flight.afterburner_active());
                            self.input.feedback_tick();
                            if let Some(recording) = &mut self.replay_recorder {
                                recording.end(Some(&mut self.flight_ui), &mut self.combat);
                            }
                        }
                        self.input.feedback_flush();
                        if !self.flight_ui.frozen() {
                            let analog_look = self.input.resolver.look();
                            look::step_axes(
                                &mut self.flight_ui.look,
                                analog_look,
                                elapsed,
                                matches!(self.flight_view, 1 | 2),
                            );
                            self.head_look = self.input.head_look().unwrap_or([0.; 2]);
                        }
                        self.combat.present_targets(if self.flight_ui.frozen() {
                            1.0
                        } else {
                            self.flight_clock.remainder / flight::DT
                        });
                        // Everything combat draws this frame, shared by every camera.
                        let frame = self.combat.presented();
                        let presented = if self.flight_ui.frozen() {
                            self.flight.clone()
                        } else {
                            self.flight.presented(
                                &self.previous_flight,
                                self.flight_clock.remainder / flight::DT,
                            )
                        };
                        if presented.escape.is_some() {
                            self.flight_view = 1;
                            self.view_rig.select(flight_views::Reference::Player);
                        }
                        let scene = flight_views::Scene::new(
                            &presented,
                            &self.combat,
                            self.ai_wings.as_ref(),
                            true,
                        );
                        let camera_keys = std::mem::take(&mut self.camera.keys);
                        let base =
                            self.hornet
                                .camera(&presented, self.flight_view, camera_keys.clone());
                        self.camera = match self.view_rig.camera(
                            self.flight_view,
                            &scene,
                            base,
                            look::combine(
                                self.flight_ui.look,
                                self.head_look,
                                matches!(self.flight_view, 1 | 2),
                            ),
                            self.flight_ui.zoom,
                        ) {
                            Ok(camera) => camera,
                            Err(reason) => {
                                self.flight_ui
                                    .message(format!("{reason}; returning to Forward view"));
                                self.flight_view = 0;
                                self.view_rig.select(flight_views::Reference::Player);
                                self.flight_ui.look = [0.; 2];
                                self.flight_ui.zoom = 1.;
                                self.hornet.camera(&presented, 0, camera_keys)
                            }
                        };
                        if !self.flight_ui.cheats.no_screen_shake
                            && !presented.crashed
                            && self.view_rig.cockpit(self.flight_view)
                        {
                            let seconds =
                                self.flight.ticks as f64 * flight::DT + self.flight_clock.remainder;
                            let [yaw, pitch] = tore_sim::g_effects::shake(presented.g, seconds);
                            look::apply(
                                &mut self.camera,
                                presented.view_position().map(|v| v as f32),
                                [yaw as f32, pitch as f32],
                                false,
                            );
                        }
                        self.camera.zoom = self.flight_ui.zoom;
                        // One resolved instant per frame, shared by the main view,
                        // the mirrors and the camera panels.
                        self.world
                            .resolve_palette(f64::from(self.camera.position[1]));
                        let vapor = vapor_vertices(
                            &self.vapor,
                            &self.world,
                            &presented,
                            self.hornet.streamer_points(&presented),
                        );
                        renderer.vapor(&vapor);
                        renderer.smoke(
                            &self.combat.art.smoke,
                            [&self.combat.state.smoke, &self.combat.contrails],
                            &self.combat.state.devices,
                        );
                        renderer.emitters(
                            &self.combat.state.devices,
                            &self.combat.afterburner_glows(&presented),
                        );
                        match renderer.poll_previews() {
                            Ok(previews) => {
                                self.performance.completed_previews += previews.len();
                                for (page, pixels) in previews {
                                    if page == 4 {
                                        let requested = self.instruments.target_preview.take();
                                        if requested
                                            != self.combat.state.display_target().map(|t| t.id)
                                        {
                                            continue;
                                        }
                                        self.instruments.camera_target = requested;
                                    }
                                    if page == 3
                                        && !std::mem::take(&mut self.view_rig.other_pending)
                                    {
                                        continue;
                                    }
                                    if page == 2 {
                                        self.instruments.front_shown =
                                            self.instruments.front_pending.take();
                                    }
                                    self.instruments.cameras.insert(page, pixels);
                                }
                            }
                            Err(e) => {
                                self.error = Some(e);
                                event_loop.exit();
                                return;
                            }
                        }
                        renderer.airports(
                            &self
                                .world
                                .visible_static_vertices(&self.combat.state.targets),
                            &self.world.visible_static_lines(&self.combat.state.targets),
                        );
                        let target_due = self.target_refresh.due(now) || self.smoke_test;
                        let other_due = now.duration_since(self.instrument_time).as_millis() >= 100
                            || self.smoke_test;
                        if target_due || other_due {
                            if other_due {
                                self.instrument_time = now;
                            }
                            for page in [2, 3, 4] {
                                if self.instruments.pages.contains(&page)
                                    && if page == 4 { target_due } else { other_due }
                                {
                                    let camera = if page == 4 {
                                        let Some(camera) = self.combat.framed_target_camera(
                                            &presented,
                                            &self.hornet,
                                            &self.world,
                                        ) else {
                                            self.instruments.cameras.remove(&4);
                                            self.instruments.camera_target = None;
                                            continue;
                                        };
                                        camera
                                    } else if page == 3 {
                                        let base = self.hornet.camera(
                                            &presented,
                                            self.view_rig.other_view(),
                                            Default::default(),
                                        );
                                        let Ok(camera) = self.view_rig.other_camera(&scene, base)
                                        else {
                                            self.instruments.cameras.remove(&3);
                                            continue;
                                        };
                                        camera
                                    } else {
                                        self.hornet.panel_camera(&presented, page)
                                    };
                                    let front = (page == 2).then(|| {
                                        instruments::front_view::Symbology::new(
                                            &presented,
                                            self.world.air_data(&presented).ok().as_ref(),
                                        )
                                    });
                                    renderer.dummies(render_snapshot::aircraft_batches(
                                        &frame,
                                        self.combat.models(),
                                        &camera,
                                        &self.world,
                                    ));
                                    renderer.combat(&render_snapshot::combat_geometry(
                                        &frame,
                                        &self.combat.art,
                                        &self.hornet,
                                        &presented,
                                        &camera,
                                        &self.world,
                                    ));
                                    renderer.aircraft(
                                        &self.hornet,
                                        &presented,
                                        page == 3 && self.view_rig.other_shows_player(),
                                        &camera,
                                        &self.world,
                                    );
                                    let result = if self.smoke_test {
                                        renderer
                                            .scene_pixels(&camera, &self.world, 138, 114, false)
                                            .map(|p| {
                                                self.instruments.cameras.insert(page, p);
                                                if page == 2 {
                                                    self.instruments.front_shown = front;
                                                }
                                                if page == 4 {
                                                    self.instruments.camera_target = self
                                                        .combat
                                                        .state
                                                        .display_target()
                                                        .map(|t| t.id);
                                                }
                                                true
                                            })
                                    } else {
                                        renderer
                                            .request_preview(page, &camera, &self.world)
                                            .inspect(|submitted| {
                                                if page == 3 && *submitted {
                                                    self.view_rig.other_pending = true;
                                                }
                                                if page == 2 && *submitted {
                                                    self.instruments.front_pending = front;
                                                }
                                                if page == 4 && *submitted {
                                                    self.instruments.target_preview = self
                                                        .combat
                                                        .state
                                                        .display_target()
                                                        .map(|t| t.id);
                                                }
                                            })
                                    };
                                    if let Err(e) = result {
                                        self.error = Some(e);
                                        event_loop.exit();
                                        return;
                                    }
                                }
                            }
                        }
                        if let Some(art) = &self.combat.art.escape {
                            renderer.escapees(
                                art,
                                &art.vertices_for(
                                    frame
                                        .pilots
                                        .iter()
                                        .map(|p| (p.position, p.heading, p.phase)),
                                    &self.hornet.palette,
                                    self.camera.position.map(f64::from),
                                ),
                            );
                        }
                        renderer.dummies(render_snapshot::aircraft_batches(
                            &frame,
                            self.combat.models(),
                            &self.camera,
                            &self.world,
                        ));
                        renderer.combat(&render_snapshot::combat_geometry(
                            &frame,
                            &self.combat.art,
                            &self.hornet,
                            &presented,
                            &self.camera,
                            &self.world,
                        ));
                        renderer.aircraft(
                            &self.hornet,
                            &presented,
                            !self.view_rig.cockpit(self.flight_view),
                            &self.camera,
                            &self.world,
                        );
                        simulation_ms = frame_start.elapsed().as_secs_f64() * 1000.;
                        self.instruments.combat = Some(
                            self.combat
                                .readout(&self.flight, self.instruments.rcs_scale_nmi()),
                        );
                        if let Some(target) = self
                            .instruments
                            .combat
                            .as_mut()
                            .and_then(|c| c.target.as_mut())
                            && let Some(wings) = &self.ai_wings
                        {
                            target.with_activity(wings);
                        }
                        if let (Some(readout), Some(wings)) =
                            (self.instruments.combat.as_mut(), self.ai_wings.as_ref())
                        {
                            for emitter in &mut readout.rwr.emitters {
                                if emitter.kind == scope::EmitterKind::EnemyAircraft
                                    && let Some(slot) = wings.slot(emitter.id)
                                    && slot.side == tore_sim::ai::launch::Side::Friendly
                                {
                                    emitter.kind = scope::EmitterKind::FriendlyAircraft;
                                }
                            }
                        }
                        // Hover feedback uses the same projection as the click,
                        // so the selector marks the contact a click would take.
                        let window = renderer.window.inner_size();
                        self.instruments.weapon_debug = self.view_rig.cockpit(self.flight_view)
                            && self.flight_ui.weapon_diagnostics_shown(self.flight_view);
                        self.instruments.hover(
                            self.pointer,
                            [f64::from(window.width), f64::from(window.height)],
                        );
                        renderer.window.set_cursor_visible(
                            self.instruments.crosshair.is_none()
                                || self.flight_ui.menu
                                || self.flight_ui.map.open,
                        );
                        let cockpit_palette = self.hornet.cockpit_palette(
                            &self.world,
                            self.camera.position[1] as f64,
                            self.flight_ui.brightness,
                        );
                        self.instruments.hud_color =
                            cockpit_palette[usize::from(self.hornet.hud.primary_color)];
                        self.instruments.palette = cockpit_palette;
                        self.flight_canvas.begin(
                            renderer.flight_size(),
                            &self.hornet,
                            &presented,
                            &self.instruments,
                        );
                        self.menu.pixels.fill(0);
                        if self.flight_ui.hud && self.view_rig.cockpit(self.flight_view) {
                            let airport_aircraft =
                                airport_aircraft(&self.world, &presented, self.airport_nav_mode);
                            let guidance = self
                                .airport_service
                                .guidance(&self.world.airport_scene, airport_aircraft)
                                .filter(|_| self.airport_nav_mode);
                            let ils = guidance.as_ref().and_then(|g| {
                                let airport = self
                                    .world
                                    .airport_scene
                                    .airports
                                    .iter()
                                    .find(|a| a.id == g.airport)?;
                                let runway = self.world.airport_scene.runway(g.runway)?;
                                Some((g, airport.name.as_str(), runway.name.as_str()))
                            });
                            hud::draw(
                                &mut self.menu.pixels,
                                &presented,
                                &self.hornet.hud_font,
                                self.world.height(
                                    presented.position[0] as f32,
                                    presented.position[2] as f32,
                                ) as f64,
                                self.world.air_data(&presented).ok().as_ref(),
                                self.flight_ui.ladder,
                                !self.airport_nav_mode && weapon_hud::active(&self.combat.state),
                                cockpit_palette[usize::from(self.hornet.hud.primary_color)],
                                self.flight_canvas.hud_zoom(1.),
                                ils,
                                airport_wind(&self.world, &presented, guidance.as_ref()).as_ref(),
                            );
                        }
                        let target_friendly = self.combat.state.display_target().is_some_and(|target| {
                            self.ai_wings.as_ref().and_then(|wings| wings.slot(target.id))
                                .is_some_and(|slot| slot.side == tore_sim::ai::launch::Side::Friendly)
                                || self.world.airport_scene.runway(target.id).is_some_and(|runway| {
                                    self.world.airport_scene.airports.iter().any(|airport| {
                                        airport.id == runway.airport
                                            && airport.allegiance == tore_sim::airport::Allegiance::Friendly
                                    })
                                })
                        });
                        // Easy targeting draws the square wherever the target is on
                        // screen, in place of the HUD's square or edge arrow.
                        let easy_square = (self.flight_ui.cheats.easy_targeting
                            && self.flight_ui.hud
                            && self.view_rig.cockpit(self.flight_view)
                            && !weapon_hud::target_in_hud(
                                &presented,
                                &self.combat.state,
                                f64::from(self.flight_canvas.hud_zoom(1.)),
                            ))
                        .then(|| self.combat.state.display_target())
                        .flatten()
                        .and_then(|target| {
                            self.camera
                                .project(self.flight_canvas.size, target.position)
                        });
                        if self.flight_ui.hud && self.view_rig.cockpit(self.flight_view) {
                            weapon_hud::draw(
                                &mut self.menu.pixels,
                                &presented,
                                &self.combat.state,
                                &self.hornet.hud_font,
                                cockpit_palette[usize::from(self.hornet.hud.primary_color)],
                                f64::from(self.flight_canvas.hud_zoom(1.)),
                                self.airport_nav_mode,
                                target_friendly,
                                easy_square.is_none(),
                            );
                        }
                        renderer.cockpit(
                            &presented,
                            &self.camera,
                            self.flight_ui.cockpit
                                && !presented.wreck_gone()
                                && self.view_rig.cockpit(self.flight_view),
                            self.flight_ui.hud
                                && !presented.wreck_gone()
                                && self.view_rig.cockpit(self.flight_view),
                            &self.menu.pixels,
                            &cockpit_palette,
                        );
                        self.menu.pixels.fill(0);
                        if self.view_rig.cockpit(self.flight_view)
                            && self.flight_ui.weapon_diagnostics_shown(self.flight_view)
                        {
                            weapon_hud::debug(
                                &mut self.menu.pixels,
                                &self.combat.state,
                                &presented,
                                &self.hornet.hud_font,
                                cockpit_palette[usize::from(self.hornet.hud.primary_color)],
                                self.airport_nav_mode,
                            );
                            self.flight_canvas.weapon_debug(&self.menu.pixels);
                        }
                        self.menu.pixels.fill(0);
                        if let Some(point) = easy_square {
                            self.flight_canvas.target_square(
                                point,
                                f64::from(self.flight_ui.zoom),
                                cockpit_palette[usize::from(self.hornet.hud.primary_color)],
                                target_friendly,
                            );
                        }
                        use tore_sim::g_effects::GEffects;
                        for (color, level) in [
                            ([150, 0, 0], self.g_effects.redout),
                            ([0, 0, 0], self.g_effects.blackout),
                        ] {
                            if level > 0. {
                                self.flight_canvas
                                    .veil(color, |radius| GEffects::coverage(level, radius));
                            }
                        }
                        if self.flight_ui.map.open {
                            self.flight_ui.map.draw(
                                &mut self.menu.pixels,
                                &self.world,
                                &presented,
                                &self.combat.state,
                                &self.hornet.font,
                                &self.menu.quick_sprites,
                            );
                            // Opaque letterbox prevents the cockpit leaking around the map.
                            for pixel in self.flight_canvas.pixels.chunks_exact_mut(4) {
                                pixel.copy_from_slice(&[72, 72, 72, 255]);
                            }
                            self.flight_canvas.legacy_layer(&self.menu.pixels, 1.);
                            self.menu.pixels.fill(0);
                        }
                        self.flight_ui.draw_notices(
                            &mut self.flight_canvas,
                            &self.hornet.hud_font,
                            self.instruments.hud_color,
                        );
                        self.flight_ui.draw(
                            &mut self.menu.pixels,
                            &self.hornet.font,
                            &self.hornet.flight_menu,
                        );
                        if let Some(editor) = &self.controls {
                            editor.draw(&mut self.menu.pixels, &self.hornet.font);
                        }
                        self.flight_canvas.legacy_layer(&self.menu.pixels, 1.);
                        if let Some(audio) = &self.audio {
                            audio.seeker(
                                self.combat
                                    .state
                                    .seeker_tone(combat::launcher(&self.flight)),
                            );
                            audio.pause_flight(self.flight_ui.frozen());
                            audio.flight(Some((
                                &self.hornet.profile,
                                &self.flight,
                                f64::from(self.world.height(
                                    self.flight.position[0] as f32,
                                    self.flight.position[2] as f32,
                                )),
                            )));
                        }
                        true
                    }
                    Screen::Viewer => {
                        let now = Instant::now();
                        let elapsed = (now - self.frame_time).as_secs_f64().min(0.25);
                        self.camera
                            .step(elapsed as f32, self.modifiers.shift_key(), &self.world);
                        for _ in 0..self.flight_clock.steps(elapsed) {
                            self.world.step_weather(0., &self.camera);
                        }
                        self.world
                            .resolve_palette(f64::from(self.camera.position[1]));
                        self.frame_time = now;
                        quick_mission::hud(
                            &mut self.menu.pixels,
                            &self.menu.quick_sprites,
                            &self.camera,
                            &self.world,
                        );
                        true
                    }
                };
                if self.screen != Screen::Flight {
                    renderer.window.set_cursor_visible(true);
                    renderer.aircraft(&self.hornet, &self.flight, false, &self.camera, &self.world);
                    if let Some(audio) = &self.audio {
                        audio.pause_flight(false);
                        audio.flight(None);
                    }
                }
                if matches!(self.screen, Screen::Viewer | Screen::Flight) {
                    renderer.airports(
                        &self
                            .world
                            .visible_static_vertices(&self.combat.state.targets),
                        &self.world.visible_static_lines(&self.combat.state.targets),
                    );
                }
                let compose_ms = frame_start.elapsed().as_secs_f64() * 1000. - simulation_ms;
                let present_start = Instant::now();
                match renderer.draw(
                    if self.screen == Screen::Flight {
                        &self.flight_canvas.pixels
                    } else {
                        &self.menu.pixels
                    },
                    (matches!(self.screen, Screen::Viewer | Screen::Flight))
                        .then_some((&self.camera, &self.world)),
                    (self.screen == Screen::Flight).then_some(self.flight_canvas.size),
                ) {
                    Ok(true) if self.smoke_test => {
                        if let Some(path) = &self.capture_terrain
                            && let Err(error) = renderer.capture_sim(
                                path,
                                &self.camera,
                                &self.world,
                                self.screen == Screen::Flight,
                            )
                        {
                            self.error = Some(error);
                        }

                        println!("Smoke test: requested screen presented successfully");
                        self.finished = true;
                        event_loop.exit();
                    }
                    Ok(presented) => {
                        animating &= presented;
                    }
                    Err(error) => {
                        self.error = Some(error);
                        event_loop.exit();
                    }
                }
                if matches!(self.screen, Screen::Flight | Screen::Viewer)
                    && self.performance.record(
                        frame_start,
                        simulation_ms,
                        compose_ms,
                        present_start.elapsed().as_secs_f64() * 1000.,
                        self.flight_ui.frozen(),
                    )
                {
                    println!(
                        "  rear mirror renders: {} (one per visible frame; no readback)",
                        renderer.mirror_frames
                    );
                    self.finished = true;
                    event_loop.exit();
                }
                // Simulation views are paced by presentation, not an extra post-render sleep.
                self.next_frame = animating.then(|| {
                    if matches!(self.screen, Screen::Flight | Screen::Viewer) {
                        Instant::now()
                    } else {
                        Instant::now() + Duration::from_millis(16)
                    }
                });
                return;
            }
            _ => return,
        };
        self.action(event_loop, action);
        self.input.context(
            self.screen != Screen::Flight || self.flight_ui.frozen(),
            self.focused,
        );
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Release GPU backends while the event loop's display connection is alive.
        self.input.stop();
        self.finish_replay_recording("exit");
        self.save_preferences();
        if let Some(recording) = &mut self.input_recording {
            use std::io::Write;
            if let Err(error) = recording.flush() {
                self.error = Some(error.into());
            }
        }
        if let Err(e) = self.combat.finish_recording() {
            self.error = Some(e);
        }
        self.renderer = None;
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let paused = self.screen != Screen::Flight || self.flight_ui.frozen();
        self.input.context(paused, self.focused);
        if Instant::now() >= self.input.next_poll {
            let (actions, lost, warnings) = self.input.poll();
            let mut changed = lost || !actions.is_empty();
            let mut captured = false;
            if let Some(editor) = &mut self.controls {
                let status = self.input.head_status();
                changed |= status != editor.head_status;
                editor.head_status = status;
                if !editor.capturing() {
                    editor.devices = self.input.devices.values().cloned().collect();
                } else {
                    editor
                        .devices
                        .retain(|d| self.input.devices.contains_key(&d.id));
                    for d in self.input.devices.values() {
                        if !editor.devices.iter().any(|old| old.id == d.id) {
                            editor.devices.push(d.clone());
                        }
                    }
                }
                for event in &self.input.observed {
                    captured |= editor.observe(event);
                }
                changed |= captured;
                if changed && let Some(renderer) = &self.renderer {
                    renderer.window.request_redraw();
                }
            }
            for warning in warnings {
                log::warn!("Input: {warning}");
            }
            if lost && self.screen == Screen::Flight {
                self.flight_ui.paused = true;
                self.flight_ui
                    .message("Active controller disconnected; resume explicitly");
                self.camera.keys.clear();
                self.combat.cancel();
                self.input.context(true, self.focused);
                self.flight_clock.remainder = 0.;
                self.frame_time = Instant::now();
            } else {
                for action in actions {
                    // Controller menu buttons navigate the controls screen,
                    // except in the poll that completed a capture with them.
                    if let Some(editor) = &mut self.controls {
                        let key = match &action {
                            tore_input::Action::Ui(name) => match name.as_str() {
                                "menu-up" => Some("ArrowUp"),
                                "menu-down" => Some("ArrowDown"),
                                "menu-left" => Some("ArrowLeft"),
                                "menu-right" => Some("ArrowRight"),
                                "menu-accept" => Some("Enter"),
                                "menu-back" => Some("Escape"),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(key) = key.filter(|_| !captured && !editor.capturing()) {
                            let result = editor.key(key, false, false, false);
                            let action = self.controls_result(result);
                            self.action(event_loop, action);
                        }
                        continue;
                    }
                    let was_frozen = self.flight_ui.frozen();
                    let result = self.input_action(action);
                    self.action(event_loop, result);
                    if was_frozen != self.flight_ui.frozen() {
                        self.camera.keys.clear();
                        self.combat.cancel();
                        self.flight_clock.remainder = 0.;
                        self.previous_flight.clone_from(&self.flight);
                        self.frame_time = Instant::now();
                        self.input.context(self.flight_ui.frozen(), self.focused);
                        break; // Remaining events belong to the previous context.
                    }
                }
            }
            if changed && let Some(renderer) = &self.renderer {
                renderer.window.request_redraw();
            }
        }
        if self.next_frame.is_some_and(|next| Instant::now() >= next) {
            if let Some(renderer) = &self.renderer {
                renderer.window.request_redraw();
            }
            self.next_frame = None;
        }
        let next = self
            .next_frame
            .map_or(self.input.next_poll, |n| n.min(self.input.next_poll));
        event_loop.set_control_flow(ControlFlow::WaitUntil(next));
    }
}
/// What the headless AI probe scripts for the human leader
/// (`--maneuver takeoff`, `--probe-wing-size`, `--probe-wing-order`,
/// `--probe-player-home`, `--probe-attack`). Development harness only.
#[derive(Clone, Debug, Default)]
struct ProbeScript {
    /// Take off from the ground start, climb and cruise on the autopilot.
    takeoff: bool,
    /// Aircraft in the player's wing, the player included.
    wing_size: Option<usize>,
    /// Reproduce an isolated player wing without the usual probe opponents.
    wing_only: bool,
    /// Wing orders to all wingmen at a tick.
    orders: Vec<(u64, tore_sim::ai::wing::PlayerOrder)>,
    /// From the first tick to the second the player flies gear down over the
    /// departure airfield, the configuration that gives it landing priority.
    home: Option<(u64, u64)>,
    /// Ticks between trace lines for the player's wing, 0 for none.
    trace_ticks: u64,
    /// The leader fires its own weapons from this tick.
    attack: Option<ProbeAttack>,
}

/// `--record-mission PATH` on an AI probe, with `--verify-render`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ProbeRecord {
    /// The new recording's path; the probe refuses to overwrite a file.
    path: PathBuf,
    /// After the run, rebuild every tick from the file and compare it with
    /// the live picture, printing one summary line.
    verify: bool,
}

/// A recording's footer: why the flight ended, and for a mission the
/// debrief's outcome, the player's fate and kills.
fn replay_footer(
    combat: &combat::Combat,
    report: Option<&debrief::Report>,
    reason: &str,
) -> tore_replay::Footer {
    let mut result = vec![("end".to_owned(), reason.to_owned())];
    match report {
        Some(report) => {
            result.push((
                "outcome".into(),
                format!("{:?}", report.outcome).to_lowercase(),
            ));
            result.push((
                "player".into(),
                format!("{:?}", report.player.status).to_lowercase(),
            ));
            result.push((
                "kills".into(),
                report.player.kills.iter().sum::<u32>().to_string(),
            ));
        }
        None => result.push(("kills".into(), combat.state.kills.to_string())),
    }
    tore_replay::Footer {
        end_tick: combat.state.tick(),
        result,
    }
}

/// `--probe-attack TICK[:REPEAT_SECONDS]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProbeAttack {
    /// First tick on which the leader looks for a shot.
    from: u64,
    /// Ticks from one shot to the next attack, 0 for a single attack.
    repeat: u64,
}

impl ProbeScript {
    /// `TICK:bug-out`, `TICK:land-selected`, `TICK:attack-on-contact` or
    /// `TICK:engage-my-target`.
    fn parse_order(text: &str) -> AppResult<(u64, tore_sim::ai::wing::PlayerOrder)> {
        use tore_sim::ai::wing::PlayerOrder;
        let usage = "--probe-wing-order needs TICK:bug-out, TICK:land-selected, TICK:attack-on-contact or TICK:engage-my-target";
        let (tick, order) = text.split_once(':').ok_or(usage)?;
        let order = match order {
            "bug-out" => PlayerOrder::BugOut,
            "land-selected" => PlayerOrder::LandAtSelected,
            "attack-on-contact" => PlayerOrder::AttackOnContact,
            "engage-my-target" => PlayerOrder::EngageMyTarget,
            _ => return Err(usage.into()),
        };
        Ok((tick.parse()?, order))
    }

    /// `TICK` for a single attack, or `TICK:REPEAT_SECONDS`.
    fn parse_attack(text: &str) -> AppResult<ProbeAttack> {
        let usage = "--probe-attack needs TICK or TICK:REPEAT_SECONDS";
        let (tick, repeat) = match text.split_once(':') {
            Some((tick, seconds)) => {
                let seconds: f64 = seconds.parse().map_err(|_| usage)?;
                if !(seconds > 0. && seconds <= 3600.) {
                    return Err("--probe-attack needs 0..3600 repeat seconds".into());
                }
                (tick, (seconds * 120.).round().max(1.) as u64)
            }
            None => (text, 0),
        };
        Ok(ProbeAttack {
            from: tick.parse().map_err(|_| usage)?,
            repeat,
        })
    }
}

/// The scripted human leader of the AI probe. `fitted` test harness (agent
/// decision, 2026-09-23), not game behaviour: the same full-power, 0.35
/// pitch rotation as `--maneuver takeoff` until 50 ft above the ground, then
/// gear and flaps up and a 10 degree nose-up hold to 3,000 ft above the
/// ground, where the player's own heading-and-altitude autopilot takes over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbePilot {
    Roll,
    Climb,
    Cruise,
    Home,
    Away,
}

/// Height above the ground at which the scripted leader cleans up.
const PROBE_CLEAN_AGL_FT: f64 = 50.;
/// Nose-up attitude the scripted leader holds in the climb.
const PROBE_CLIMB_PITCH_DEG: f64 = 10.;
/// Height above the ground at which the scripted leader levels off.
const PROBE_CRUISE_AGL_FT: f64 = 3_000.;

impl ProbePilot {
    fn fly(
        &mut self,
        tick: u64,
        flight: &mut flight::State,
        keys: &mut flight::PilotInput,
        world: &terrain::World,
        ground: Option<&quick_mission::GroundLayout>,
        script: &ProbeScript,
    ) {
        use flight::{PilotCommand::*, Switch};
        let [x, y, z] = flight.position;
        let agl = y - world.surface(x, z).height;
        match *self {
            Self::Roll => {
                keys.pitch = 0.35;
                if agl > PROBE_CLEAN_AGL_FT {
                    keys.commands = vec![Set(Switch::Gear, false), Set(Switch::Flaps, false)];
                    *self = Self::Climb;
                    println!("t={tick} player: airborne, gear and flaps up");
                }
            }
            Self::Climb => {
                let error = PROBE_CLIMB_PITCH_DEG - flight.pitch.to_degrees();
                keys.pitch =
                    (0.08 * error - 0.3 * flight.pitch_rate.to_degrees() / 10.).clamp(-1., 1.);
                keys.roll = (-flight.bank.to_degrees() / 30.).clamp(-1., 1.);
                if agl > PROBE_CRUISE_AGL_FT {
                    keys.pitch = 0.;
                    keys.roll = 0.;
                    keys.commands = vec![
                        Set(Switch::Burner, false),
                        Throttle(0.85),
                        Set(Switch::Autopilot, true),
                    ];
                    *self = Self::Cruise;
                    println!("t={tick} player: levelling off at {agl:.0} ft AGL, autopilot on");
                }
            }
            Self::Cruise => {
                if let (Some((from, _)), Some(ground)) = (script.home, ground)
                    && tick >= from
                {
                    let centre = ground.runway.center;
                    flight.autopilot.set_navigation_target(Some(
                        tore_sim::autopilot::NavigationTarget {
                            number: 1,
                            position: [centre[0], centre[2]],
                        },
                    ));
                    keys.commands = vec![
                        Set(Switch::Gear, true),
                        Throttle(0.6),
                        Set(Switch::WaypointAutopilot, true),
                    ];
                    *self = Self::Home;
                    println!("t={tick} player: gear down, flying over the departure airfield");
                }
            }
            Self::Home => {
                if script.home.is_some_and(|(_, until)| tick >= until) {
                    keys.commands = vec![
                        Set(Switch::Gear, false),
                        Throttle(0.85),
                        Set(Switch::Autopilot, true),
                    ];
                    *self = Self::Away;
                    println!("t={tick} player: gear up, leaving the airfield");
                }
            }
            Self::Away => {}
        }
    }
}

/// Per-actor transitions for the AI probe: airfield phase and activity for
/// the player's wing, deaths for everyone, and ground hazards (off the
/// landable surface, inside a building, two aircraft within
/// [`PROBE_CLOSE_FT`], or stopped mid-taxi for [`PROBE_STUCK_S`]).
#[derive(Default)]
struct ProbeWatch {
    last: std::collections::BTreeMap<u32, String>,
    player: Option<String>,
    priority: Option<u32>,
    hazards: std::collections::BTreeSet<(u32, u32, &'static str)>,
    stopped_since: std::collections::BTreeMap<u32, u64>,
    firsts: std::collections::BTreeMap<u32, (String, Vec<(String, u64)>)>,
    events: usize,
    /// Ticks between trace lines for the player's wing, 0 for none.
    trace: u64,
    go_arounds: std::collections::BTreeMap<u32, u32>,
}

/// Aircraft on the ground closer than this are reported as touching.
const PROBE_CLOSE_FT: f64 = 30.;
/// A taxiing aircraft stopped this long is reported as possibly stuck.
const PROBE_STUCK_S: f64 = 60.;
/// Height above the surface below which an aircraft counts as on the ground.
const PROBE_GROUND_AGL_FT: f64 = 15.;
/// Id used for the player in hazard pairs.
const PROBE_PLAYER: u32 = 0;

impl ProbeWatch {
    fn observe(
        &mut self,
        tick: u64,
        bridge: &ai_wings::AiWings,
        player: &flight::State,
        world: &terrain::World,
    ) {
        use tore_sim::ai::airfield::Phase;
        let seconds = tick as f64 / 120.;
        let line = |f: &flight::State| {
            let [x, y, z] = f.position;
            format!(
                "agl={:.0} kt={:.0} x={x:.0} z={z:.0} hdg={:.0} terrain_agl={:.0}",
                y - world.surface(x, z).height,
                f.speed / 1.68781,
                f.yaw.to_degrees().rem_euclid(360.),
                y - f64::from(world.height(x as f32, z as f32))
            )
        };
        let [px, py, pz] = player.position;
        let player_ground = world.surface(px, pz);
        let player_on_ground = player.research.as_ref().is_some_and(|r| r.on_ground);
        let key = format!(
            "on_ground={player_on_ground} gear={} crashed={}",
            player.gear_down, player.crashed
        );
        if self.player.as_ref() != Some(&key) {
            println!("t={tick} ({seconds:.1}s) player: {key} {}", line(player));
            self.player = Some(key);
        }
        let priority = bridge.mission().priority_landing();
        if priority != self.priority {
            println!("t={tick} ({seconds:.1}s) player landing priority: {priority:?}");
            self.priority = priority;
        }
        let mut grounded = Vec::new();
        if !player.crashed && py - player_ground.height < PROBE_GROUND_AGL_FT {
            grounded.push((PROBE_PLAYER, player.position));
        }
        for slot in bridge.slots() {
            let Some(actor) = bridge.mission().actor(slot.id) else {
                continue;
            };
            let f = actor.flight();
            let phase = actor.airfield_phase();
            let own_wing =
                slot.side == tore_sim::ai::launch::Side::Friendly && slot.wing_number == 1;
            if own_wing && self.trace > 0 && tick.is_multiple_of(self.trace) {
                println!(
                    "t={tick} ({seconds:.1}s) trace {}: {:?} leg={:?} {} thr={:.2} brake={}",
                    slot.label(),
                    phase,
                    actor.airfield().map(|s| s.leg()),
                    line(f),
                    f.throttle,
                    f.brake_out
                );
            }
            let key = if own_wing {
                format!(
                    "phase={} activity=\"{}\" alive={}",
                    phase.map_or("-".into(), |p| format!("{p:?}")),
                    actor.activity().label(),
                    actor.alive()
                )
            } else {
                format!("alive={}", actor.alive())
            };
            if self.last.get(&slot.id) != Some(&key) {
                println!(
                    "t={tick} ({seconds:.1}s) {} {:?}: {key} {}",
                    slot.label(),
                    slot.aircraft,
                    line(f)
                );
                self.last.insert(slot.id, key);
                self.events += 1;
                if own_wing {
                    let entry = self
                        .firsts
                        .entry(slot.id)
                        .or_insert_with(|| (slot.label(), Vec::new()));
                    let name = phase.map_or("Airborne".into(), |p| format!("{p:?}"));
                    if !entry.1.iter().any(|(n, _)| *n == name) {
                        entry.1.push((name, tick));
                    }
                }
            }
            if let Some(sequence) = actor.airfield() {
                let count = sequence.go_arounds();
                if count > *self.go_arounds.get(&slot.id).unwrap_or(&0) {
                    let point = sequence.landing_point();
                    let [x, _, z] = f.position;
                    let [vx, _, vz] = f.velocity;
                    // Signed distance past the landing point along the track.
                    let past = ((x - point[0]) * vx + (z - point[2]) * vz) / vx.hypot(vz).max(1.);
                    println!(
                        "t={tick} ({seconds:.1}s) GO-AROUND {} #{count}: {} vs={:.0} {:.0} ft past the landing point, {:.0} ft above it, landable below={}",
                        slot.label(),
                        line(f),
                        f.velocity[1],
                        past,
                        f.position[1] - point[1],
                        world.surface(x, z).landable
                    );
                }
                self.go_arounds.insert(slot.id, count);
            }
            if !actor.alive() || f.crashed {
                continue;
            }
            let [x, y, z] = f.position;
            let surface = world.surface(x, z);
            if y - surface.height >= PROBE_GROUND_AGL_FT {
                self.stopped_since.remove(&slot.id);
                continue;
            }
            grounded.push((slot.id, f.position));
            let hazard = |this: &mut Self, what: &'static str, on: bool| {
                let entry = (slot.id, slot.id, what);
                if on && this.hazards.insert(entry) {
                    println!(
                        "t={tick} ({seconds:.1}s) HAZARD {what}: {} {}",
                        slot.label(),
                        line(f)
                    );
                } else if !on && this.hazards.remove(&entry) {
                    println!("t={tick} ({seconds:.1}s) clear {what}: {}", slot.label());
                }
            };
            hazard(self, "off landable surface", !surface.landable);
            let probe = [x, surface.height + 6., z];
            let inside = world
                .solid_contact(
                    probe,
                    probe,
                    world.airport_scene.objects.iter().map(|o| o.id),
                )
                .is_some();
            hazard(self, "inside building", inside);
            let taxiing = matches!(phase, Some(Phase::Taxi | Phase::LineUp | Phase::TaxiClear));
            if taxiing && f.speed < 1. {
                let since = *self.stopped_since.entry(slot.id).or_insert(tick);
                hazard(
                    self,
                    "stopped while taxiing",
                    (tick - since) as f64 / 120. >= PROBE_STUCK_S,
                );
            } else {
                self.stopped_since.remove(&slot.id);
                hazard(self, "stopped while taxiing", false);
            }
        }
        for (i, (a, pa)) in grounded.iter().enumerate() {
            for (b, pb) in &grounded[i + 1..] {
                let close = (pa[0] - pb[0]).hypot(pa[2] - pb[2]) < PROBE_CLOSE_FT;
                let entry = (*a, *b, "aircraft within 30 ft");
                if close && self.hazards.insert(entry) {
                    println!(
                        "t={tick} ({seconds:.1}s) HAZARD aircraft within 30 ft: ids {a} and {b}"
                    );
                } else if !close {
                    self.hazards.remove(&entry);
                }
            }
        }
    }

    fn summary(&self) {
        let mut climbs = Vec::new();
        for (label, phases) in self.firsts.values() {
            let text: Vec<_> = phases
                .iter()
                .map(|(name, tick)| format!("{name}@{:.1}s", *tick as f64 / 120.))
                .collect();
            println!("AI probe phases: {label}: {}", text.join(" "));
            if let Some((_, tick)) = phases.iter().find(|(n, _)| n == "ClimbOut") {
                climbs.push(*tick);
            }
        }
        climbs.sort_unstable();
        let gaps: Vec<_> = climbs
            .windows(2)
            .map(|w| format!("{:.1}s", (w[1] - w[0]) as f64 / 120.))
            .collect();
        println!(
            "AI probe liftoff gaps: [{}] hazards_open={} transitions={}",
            gaps.join(", "),
            self.hazards.len(),
            self.events
        );
    }
}

/// Longest gun burst the scripted leader holds, in ticks.
const PROBE_BURST_TICKS: u64 = 120;
/// Ticks a selected missile may go without READY before the scripted leader
/// changes to the gun, when the gun reaches the target.
const PROBE_LOCK_TICKS: u64 = 240;
/// The scripted leader fires the gun with the target this close to the pipper.
const PROBE_PIPPER_DEG: f64 = 2.;

/// The scripted leader's own weapons for `--probe-attack`, and what the probe
/// reports about the fight that follows.
///
/// `fitted` test harness (agent decision, 2026-09-26), not game behaviour. The
/// leader uses only the player's own controls, applied between ticks as key and
/// mouse input is: a scope click designates the nearest hostile aircraft among
/// the player's current sensor contacts, `]` steps through NAV and the
/// stations to the chosen weapon, and Space fires. Rule: the weapon is the
/// longest-reaching air-to-air store (missile or gun) whose employment zone
/// holds the contact's observed range. A missile is one press once its readout
/// says READY; after [`PROBE_LOCK_TICKS`] without READY the leader changes to
/// the gun if the gun reaches. The gun fires while the target sits within
/// [`PROBE_PIPPER_DEG`] of the HUD's gun pipper, for at most
/// [`PROBE_BURST_TICKS`]. The flying is left to the rest of the probe script.
/// After each shot the next attack waits the repeat interval, choosing the
/// nearest target again; a single attack ends with its first shot.
struct ProbeAttacker {
    repeat: u64,
    /// Tick of the next attack, `None` once a single attack has fired.
    next: Option<u64>,
    /// The attack in progress has not yet chosen its target.
    fresh: bool,
    /// The attack in progress has given up on missiles.
    guns: bool,
    /// The selected missile station and the tick it began waiting for READY.
    waiting: Option<(usize, u64)>,
    /// A missile press, released on the next tick.
    pressed: bool,
    /// First tick of the gun burst in progress.
    burst: Option<u64>,
    /// The leader's scope clicks, `]` presses, trigger presses, missiles,
    /// gun bursts and gun rounds.
    clicks: u32,
    steps: u32,
    presses: u32,
    missiles: u32,
    bursts: u32,
    rounds: u32,
    /// Hits on and destructions of AI aircraft by anyone, and hits on the
    /// player, from the combat events.
    hits: u32,
    destroyed: u32,
    player_damaged: u32,
    /// Each AI aircraft's engagement gate last tick.
    neutral: std::collections::BTreeMap<u32, bool>,
    /// AI aircraft that have perceived an attack, or defended against a missile.
    attacked: std::collections::BTreeSet<u32>,
    defending: std::collections::BTreeSet<u32>,
    /// Decoys the AI aircraft carried at the start.
    decoys: u32,
    ejections: u32,
}

impl ProbeAttacker {
    fn new(attack: ProbeAttack, combat: &combat::Combat, bridge: &ai_wings::AiWings) -> Self {
        let state = &combat.state;
        let stations: Vec<_> = state
            .configuration()
            .stations
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let zone = s.weapon.seeker.zones[1];
                format!(
                    "{} x{} {}..{} ft{}",
                    s.weapon.hud_name,
                    state.rounds(i),
                    zone.minimum_range,
                    zone.maximum_range,
                    if probe_air_to_air(&s.weapon) {
                        ""
                    } else {
                        " (not air to air)"
                    }
                )
            })
            .collect();
        println!(
            "AI probe attack: from=t{} repeat={:.1}s stations=[{}]",
            attack.from,
            attack.repeat as f64 / 120.,
            stations.join(", ")
        );
        let actors = bridge.mission().actors();
        Self {
            repeat: attack.repeat,
            next: Some(attack.from),
            fresh: true,
            guns: false,
            waiting: None,
            pressed: false,
            burst: None,
            clicks: 0,
            steps: 0,
            presses: 0,
            missiles: 0,
            bursts: 0,
            rounds: 0,
            hits: 0,
            destroyed: 0,
            player_damaged: 0,
            neutral: actors.iter().map(|a| (a.id(), a.is_neutral())).collect(),
            attacked: Default::default(),
            defending: Default::default(),
            decoys: probe_decoys(bridge),
            ejections: 0,
        }
    }

    /// The player's controls for this tick, applied before it as key and
    /// mouse input is. One control per tick.
    fn aim(
        &mut self,
        tick: u64,
        combat: &mut combat::Combat,
        flight: &flight::State,
        bridge: &ai_wings::AiWings,
    ) {
        use tore_sim::combat::live::{Command, Readiness, is_gun};
        if std::mem::take(&mut self.pressed) {
            combat.input.space(false, false, false);
        }
        if self.next.is_none_or(|next| tick < next) {
            return;
        }
        let launcher = combat::launcher(flight);
        if !launcher.alive
            || flight.escape.is_some()
            || flight.systems.pilot.dead
            || combat.state.player_hp <= 0
        {
            self.end_burst(tick, combat);
            return;
        }
        let seconds = tick as f64 / 120.;
        let state = &combat.state;
        let hostile = |id: u32| {
            bridge
                .slot(id)
                .is_some_and(|s| s.side == tore_sim::ai::launch::Side::Enemy)
                && state.targets.iter().any(|t| t.id == id && t.hp > 0)
        };
        let current = state
            .designated()
            .filter(|id| hostile(*id) && state.sensors.contact(*id).is_some());
        if self.fresh || current.is_none() {
            let nearest = state
                .sensors
                .contacts()
                .iter()
                .filter(|c| !c.destroyed && hostile(c.id))
                .min_by(|a, b| {
                    a.distance_ft
                        .total_cmp(&b.distance_ft)
                        .then(a.id.cmp(&b.id))
                })
                .map(|c| (c.id, c.distance_ft));
            let Some((id, range)) = nearest else {
                self.end_burst(tick, combat);
                return;
            };
            self.fresh = false;
            if current != Some(id) {
                self.end_burst(tick, combat);
                self.guns = false;
                self.waiting = None;
                combat.command(Command::DesignateTarget(id), launcher);
                self.clicks += 1;
                println!(
                    "t={tick} ({seconds:.1}s) attack: designates {} at {range:.0} ft",
                    probe_label(bridge, id)
                );
                return;
            }
        }
        let Some((target, contact)) = combat
            .state
            .designated()
            .and_then(|id| Some((id, *combat.state.sensors.contact(id)?)))
        else {
            return;
        };
        let range = contact.distance_ft;
        let station = self
            .guns
            .then(|| probe_station(&combat.state, range, true))
            .flatten()
            .or_else(|| probe_station(&combat.state, range, false));
        let Some(station) = station else {
            self.end_burst(tick, combat);
            return;
        };
        if !combat.state.armed || combat.state.selected != station {
            self.end_burst(tick, combat);
            combat.cancel();
            combat.command(Command::NextSelection, launcher);
            self.steps += 1;
            if combat.state.armed && combat.state.selected == station {
                println!(
                    "t={tick} ({seconds:.1}s) attack: selects {} at {range:.0} ft",
                    combat.state.configuration().stations[station]
                        .weapon
                        .hud_name
                );
            }
            return;
        }
        let ready = combat.state.release_readiness == Readiness::Ready;
        if is_gun(&combat.state.configuration().stations[station].weapon) {
            let on = ready && probe_on_pipper(&combat.state, &launcher, station, &contact);
            match self.burst {
                Some(from) if !on || tick - from >= PROBE_BURST_TICKS => {
                    self.end_burst(tick, combat);
                }
                Some(_) => {}
                None if on => {
                    combat.input.space(false, false, false);
                    combat.input.space(true, false, false);
                    self.burst = Some(tick);
                    self.presses += 1;
                    println!(
                        "t={tick} ({seconds:.1}s) attack: gun at {} range={range:.0} ft",
                        probe_label(bridge, target)
                    );
                }
                None => {}
            }
        } else if ready {
            combat.input.space(false, false, false);
            combat.input.space(true, false, false);
            self.pressed = true;
            self.presses += 1;
        } else {
            let since = match self.waiting {
                Some((waiting, since)) if waiting == station => since,
                _ => {
                    self.waiting = Some((station, tick));
                    tick
                }
            };
            if tick - since >= PROBE_LOCK_TICKS
                && !self.guns
                && probe_station(&combat.state, range, true).is_some()
            {
                self.guns = true;
                println!(
                    "t={tick} ({seconds:.1}s) attack: no {} shot ({}), changes to the gun",
                    combat.state.configuration().stations[station]
                        .weapon
                        .hud_name,
                    combat.state.release_readiness.label()
                );
            }
        }
    }

    /// Release a gun burst in progress; the burst counts as this attack's shot.
    fn end_burst(&mut self, tick: u64, combat: &mut combat::Combat) {
        if self.burst.take().is_some() {
            combat.input.space(false, false, false);
            self.bursts += 1;
            self.shot(tick);
        }
    }

    fn shot(&mut self, tick: u64) {
        self.next = (self.repeat > 0).then_some(tick + self.repeat);
        self.fresh = true;
        self.guns = false;
        self.waiting = None;
    }

    /// This tick's combat events, after `Combat::step`.
    fn events(
        &mut self,
        tick: u64,
        events: &[tore_sim::combat::live::Event],
        combat: &combat::Combat,
        bridge: &ai_wings::AiWings,
    ) {
        use tore_sim::combat::live::Event;
        let seconds = tick as f64 / 120.;
        for event in events {
            match event {
                Event::Fired(station) => {
                    let weapon = &combat.state.configuration().stations[*station].weapon;
                    if tore_sim::combat::live::is_gun(weapon) {
                        self.rounds += 1;
                        continue;
                    }
                    self.missiles += 1;
                    let target = combat.state.designated();
                    println!(
                        "t={tick} ({seconds:.1}s) attack: fires {} at {} range={:.0} ft",
                        weapon.hud_name,
                        target.map_or("-".into(), |id| probe_label(bridge, id)),
                        target
                            .and_then(|id| combat.state.sensors.observation(id))
                            .map_or(0., |c| c.distance_ft)
                    );
                    self.shot(tick);
                }
                Event::Hit(id) if bridge.slot(*id).is_some() => self.hits += 1,
                Event::Destroyed(id) if bridge.slot(*id).is_some() => {
                    self.destroyed += 1;
                    println!(
                        "t={tick} ({seconds:.1}s) destroyed: {}",
                        probe_label(bridge, *id)
                    );
                }
                Event::PlayerDamaged(_) => self.player_damaged += 1,
                Event::PlayerDestroyed => {
                    println!("t={tick} ({seconds:.1}s) destroyed: player");
                }
                _ => {}
            }
        }
    }

    /// After the AI step: perceived attacks, releases, missile defence and
    /// ejections.
    fn observe(&mut self, tick: u64, bridge: &mut ai_wings::AiWings) {
        let seconds = tick as f64 / 120.;
        for slot in bridge.slots() {
            let Some(actor) = bridge.mission().actor(slot.id) else {
                continue;
            };
            if let Some(attack) = actor.perceived_attacks().first()
                && self.attacked.insert(slot.id)
            {
                println!(
                    "t={tick} ({seconds:.1}s) {} perceives an attack on {} by {}",
                    slot.label(),
                    probe_label(bridge, attack.report.defended_id),
                    attack
                        .report
                        .attacker_id
                        .map_or("an unknown attacker".into(), |id| probe_label(bridge, id))
                );
            }
            let neutral = actor.is_neutral();
            if self.neutral.insert(slot.id, neutral) == Some(true) && !neutral {
                println!(
                    "t={tick} ({seconds:.1}s) {} released to engage",
                    slot.label()
                );
            }
            if actor.defense_decision().is_some_and(|d| d.motion.is_some())
                && self.defending.insert(slot.id)
            {
                println!(
                    "t={tick} ({seconds:.1}s) {} defends against a missile",
                    slot.label()
                );
            }
        }
        for (_, message, _) in bridge.ejection_events.drain(..) {
            println!("t={tick} ({seconds:.1}s) {message}");
            self.ejections += 1;
        }
    }

    fn summary(&self, combat: &combat::Combat, flight: &flight::State, bridge: &ai_wings::AiWings) {
        use tore_sim::ai::launch::Side;
        let lost = |side: Side| {
            let slots: Vec<_> = bridge.slots().iter().filter(|s| s.side == side).collect();
            let down = slots
                .iter()
                .filter(|s| bridge.mission().actor(s.id).is_none_or(|a| !a.alive()))
                .count();
            format!("{down}/{}", slots.len())
        };
        let released = self.neutral.values().filter(|n| !**n).count();
        println!(
            "AI probe attack: clicks={} steps={} presses={} missiles={} gun_bursts={} gun_rounds={} player_hits={} player_kills={} hits={} destroyed={} lost_friendly={} lost_enemy={} player_alive={} player_damaged={} ejections={} perceived={} released={released} defending={} decoys_used={}",
            self.clicks,
            self.steps,
            self.presses,
            self.missiles,
            self.bursts,
            self.rounds,
            combat.state.hits,
            combat.state.kills,
            self.hits,
            self.destroyed,
            lost(Side::Friendly),
            lost(Side::Enemy),
            !flight.crashed && combat.state.player_hp > 0,
            self.player_damaged,
            self.ejections,
            self.attacked.len(),
            self.defending.len(),
            self.decoys.saturating_sub(probe_decoys(bridge)),
        );
    }
}

/// A missile with an air-to-air seeker profile, or a gun.
fn probe_air_to_air(weapon: &tore_formats::weapons::Weapon) -> bool {
    use tore_sim::combat::missiles::{Profile, TargetRole};
    tore_sim::combat::live::is_gun(weapon)
        || Profile::for_weapon(weapon).is_some_and(|p| p.role == TargetRole::Aircraft)
}

/// The scripted leader's weapon at this range: the longest-reaching loaded
/// air-to-air station, or gun, whose employment zone holds it.
fn probe_station(state: &tore_sim::combat::live::State, range_ft: f64, gun: bool) -> Option<usize> {
    state
        .configuration()
        .stations
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            let zone = s.weapon.seeker.zones[1];
            state.rounds(*i) > 0
                && state.ammo[*i] & 0x8000 == 0
                && probe_air_to_air(&s.weapon)
                && (!gun || tore_sim::combat::live::is_gun(&s.weapon))
                && range_ft >= f64::from(zone.minimum_range)
                && range_ft <= f64::from(zone.maximum_range)
        })
        .max_by(|(a, x), (b, y)| {
            x.weapon.seeker.zones[1]
                .maximum_range
                .cmp(&y.weapon.seeker.zones[1].maximum_range)
                .then(b.cmp(a))
        })
        .map(|(i, _)| i)
}

/// Whether the contact sits within [`PROBE_PIPPER_DEG`] of the gun pipper,
/// solved as the HUD solves it, and inside the gun's reach.
fn probe_on_pipper(
    state: &tore_sim::combat::live::State,
    launcher: &tore_sim::combat::live::Launcher,
    station: usize,
    contact: &tore_sim::sensors::Contact,
) -> bool {
    use tore_sim::{
        combat::{gunsight, missiles},
        sensors::Channel,
    };
    let station = &state.configuration().stations[station];
    let radar = (contact.channel == Channel::Radar
        && state.sensors.operating(Channel::Radar)
        && launcher.radar)
        .then_some(gunsight::TargetObservation {
            position: contact.position,
            velocity: contact.velocity,
        });
    let Ok(Some(solution)) = gunsight::solve(&station.weapon, launcher, station.mount, radar)
    else {
        return false;
    };
    let pipper = missiles::sub(solution.point, launcher.position);
    let target = missiles::sub(contact.position, launcher.position);
    let range = missiles::length(target);
    range <= solution.maximum_range_ft
        && tore_sim::attitude::dot(pipper, target) / (missiles::length(pipper) * range).max(1.)
            >= PROBE_PIPPER_DEG.to_radians().cos()
}

/// Decoys every AI aircraft still carries.
fn probe_decoys(bridge: &ai_wings::AiWings) -> u32 {
    bridge
        .mission()
        .actors()
        .iter()
        .flat_map(|a| a.dispensers())
        .map(|d| d.count)
        .sum()
}

/// "Enemy 1-2" for an AI aircraft, "player" for the human leader.
fn probe_label(bridge: &ai_wings::AiWings, id: u32) -> String {
    bridge.slot(id).map_or_else(
        || {
            if id == 0 {
                "player".into()
            } else {
                format!("id {id}")
            }
        },
        ai_wings::Slot::label,
    )
}

/// Deterministic headless AI probe (`--ai-probe-ticks`).
///
/// It builds the same chain a flown Quick Mission builds: the existing spawner
/// places the wings, `Combat::reset` puts them in the world, and the AI bridge
/// takes over from the targets it finds. Nothing about the fixture path is
/// bypassed, so a difference between this probe and a flown mission would be a
/// real difference.
///
/// `opinionated` (agent decision, 2026-09-17): the probe overwrites four setup
/// fields so both sides always have aircraft. Rule: the default draft populates
/// only enemy wing 1, which would leave the friendly-side and wingman paths
/// untested, and a probe that exercises one side is not evidence for two.
/// With `--ground-start` it also gives the player two wingmen (agent
/// decision, 2026-09-23), so the parked-wing path is exercised too.
#[allow(clippy::too_many_arguments)]
fn ai_probe_run(
    ticks: usize,
    quick: &mut quick_mission::QuickMission,
    hornet: &aircraft::Airframe,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    world: &terrain::World,
    enemy_skill: Option<tore_sim::ai::experience::EnemySkillOverride>,
    ai_mission: ai_wings::Preset,
    script: &ProbeScript,
    record: Option<&ProbeRecord>,
) -> AppResult<()> {
    quick.draft.values[7] = if script.wing_only { 0 } else { 2 };
    quick.draft.values[8] = 1;
    quick.draft.values[21] = if script.wing_only { 0 } else { 2 };
    if quick.ground_runway().is_some() {
        quick.draft.values[4] = 3;
    }
    if let Some(size) = script.wing_size {
        quick.draft.values[4] = size;
    }
    quick.draft.values[22] = 2;
    let wings = quick
        .wing_launches(enemy_skill)
        .map_err(|e| e.to_string())?;
    let mut combat = combat::Combat::new(hornet, resources, false)?;
    combat.add_airport_targets(&world.airport_scene)?;
    // The same launch layout a flown mission uses, including a ground start
    // when `--ground-start` chose a runway.
    let mut flight = hornet.start(world);
    let parked = match quick.ground_runway() {
        Some(object) => {
            flight.enable_research(1)?;
            Some(quick_mission::ground_layout(
                world,
                object,
                quick.player_wing_size(),
            )?)
        }
        None => None,
    };
    let layout = quick_mission::MissionLayout::plan(
        world,
        &flight,
        parked.clone(),
        &ai_wings::enemy_group_offsets(&wings),
        quick.separation_feet(),
    );
    combat.mission_aircraft(&wings, &layout, resources)?;
    let heading = match &parked {
        Some(ground) => {
            flight.position[0] = ground.slots[0][0];
            flight.position[2] = ground.slots[0][2];
            ground.heading
        }
        None => flight.yaw + layout.player_turn,
    };
    if heading != flight.yaw {
        flight.yaw = heading;
        let basis = attitude::Basis::new(heading, 0., 0.);
        flight.velocity =
            std::array::from_fn(|i| basis.forward[i] * flight.speed + world.wind()[i]);
    }
    combat.reset(&mut flight)?;
    if script.attack.is_some() {
        // A flown mission's weapon startup: the gun selected and armed in the
        // air, navigation mode on a ground start.
        combat.apply_startup_weapons();
        combat.state.armed = parked.is_none();
    }
    if let Some(ground) = &parked {
        quick_mission::place_on_runway(world, &mut flight, ground, 0)?;
    }
    let airfields = ai_wings::Airfields::from_world(
        world,
        parked.as_ref().map(quick_mission::GroundLayout::departure),
    );
    let mut bridge = ai_wings::AiWings::build_mission(
        &wings,
        &combat.state.targets,
        quick.guns_only(),
        resources,
        &airfields,
    )?;
    bridge.apply_mission_preset(ai_mission, flight.position);
    bridge.apply_group_objectives(&quick.group_objectives, flight.position);
    bridge.apply_group_survival(&quick.group_must_survive);
    bridge.mirror_pose_out(&mut combat.state.targets);
    println!(
        "AI probe: aircraft={} actors={} ticks={ticks} enemy_skill={enemy_skill:?} mission={ai_mission}",
        hornet.profile.name,
        bridge.len()
    );
    // Radio calls are observed, never fed back, so the probe is unchanged.
    let mut comms = comms::Comms::new(1);
    let mut radio = radio_calls::Radio::default();
    let mut airfield_radio = airfield_radio::AirfieldRadio::default();
    airfield_radio.reset(parked.as_ref().map(|g| g.runway));
    let phrases = comms::phrases(resources);
    let mut heard = Vec::new();
    println!(
        "AI probe layout: airport={:?} ground={:?} runway_ft={:?} anchored={:?} spacing_ft={:?} enemy_nm={:.1} requested_nm={:.1} enemy_turn_deg={:.0}",
        parked.as_ref().and_then(|g| world
            .airport_scene
            .airports
            .iter()
            .find(|a| a.id == g.airport)
            .map(|a| a.name.as_str())),
        parked.as_ref().map(|g| g.object),
        parked.as_ref().map(|g| g.runway.length_ft.round()),
        parked.as_ref().map(|g| g.anchored),
        parked.as_ref().and_then(|g| g.spacing_ft),
        layout.enemy.distance_ft / quick_mission::FEET_PER_NM,
        layout.enemy.requested_ft / quick_mission::FEET_PER_NM,
        layout.enemy.turn.to_degrees()
    );
    if let Some(notice) = layout.notice() {
        println!("AI probe notice: {notice}");
    }
    let bounds = quick_mission::map_bounds(world);
    for slot in bridge.slots() {
        let Some(actor) = bridge.mission().actor(slot.id) else {
            continue;
        };
        if let Some(home) = actor.home_runway()
            && slot.side == tore_sim::ai::launch::Side::Friendly
            && slot.wing_number == 1
        {
            println!(
                "AI probe home: {} runway={} airport={:?} length_ft={:.0} vertical_pad={}",
                slot.label(),
                home.object,
                world
                    .airport_scene
                    .airports
                    .iter()
                    .find(|a| a.id == home.airport)
                    .map(|a| a.name.as_str()),
                home.length_ft,
                world.airport_scene.vertical_pad(home.object)
            );
        }
        let [x, _, z] = actor.flight().position;
        if !(bounds.min[0]..=bounds.max[0]).contains(&x)
            || !(bounds.min[1]..=bounds.max[1]).contains(&z)
        {
            println!("AI probe OFF-MAP start: {} x={x:.0} z={z:.0}", slot.label());
        }
    }
    // The tower service the player's Shift-A would use, with the departure
    // airport selected, so "land at selected airport" and the landing
    // priority go through the same rules as a flown mission.
    let mut service =
        tore_sim::airport::Service::new(&world.airport_scene).map_err(std::io::Error::other)?;
    if let Some(ground) = &parked {
        service.command(
            &world.airport_scene,
            airport_aircraft(world, &flight, false),
            tore_sim::airport::Command::SelectAirport(ground.airport),
        );
    }
    let scripted = script.takeoff && parked.is_some();
    if scripted {
        flight.brake_out = false;
        flight.throttle = 1.;
        flight.burner = flight
            .model()
            .configuration()
            .propulsion
            .afterburner_thrust_lbf
            > 0.;
    }
    let mut pilot = ProbePilot::Roll;
    let mut watch = ProbeWatch {
        trace: script.trace_ticks,
        ..Default::default()
    };
    let mut attacker = script.attack.map(|attack| {
        // As a flown mission does each frame: T and Enter skip friendlies.
        combat.state.friendlies = bridge.friendly_ids();
        ProbeAttacker::new(attack, &combat, &bridge)
    });
    // A mission recording of the probe: the picture is taken the way live
    // flight takes it, and nothing it reads feeds back into the run.
    let mut recording = match record {
        Some(record) => {
            combat.restart_render(&flight, Some(&bridge));
            let mut recording = start_probe_recording(
                record, &combat, &flight, &bridge, hornet, world, ticks, ai_mission, script,
            )?;
            recording.end(None, &mut combat);
            Some(recording)
        }
        None => None,
    };
    let verify = record.is_some_and(|r| r.verify);
    let mut pictures = Vec::new();
    if verify {
        pictures.push(combat.render_snapshot().clone());
    }
    let mut noted_ejections = 0;
    for tick in 0..ticks as u64 {
        let previous = recording.as_ref().map(|_| flight.clone());
        if let Some(recording) = &mut recording {
            recording.start_tick(None, &mut combat);
        }
        let mut keys = flight::PilotInput::default();
        if scripted {
            pilot.fly(tick, &mut flight, &mut keys, world, parked.as_ref(), script);
        }
        if let Some(attacker) = &mut attacker {
            attacker.aim(tick, &mut combat, &flight, &bridge);
        }
        if parked.is_some() {
            flight.step_surface(&keys, |x, z| world.surface(x, z));
        } else {
            flight.step(&keys, |x, z| f64::from(world.height(x as f32, z as f32)));
        }
        let events = combat.step(&mut flight, world)?;
        if let Some(attacker) = &mut attacker {
            attacker.events(tick, &events, &combat, &bridge);
            // What a flown mission does with the same events.
            for event in &events {
                use tore_sim::combat::live::Event;
                match event {
                    Event::Jolt(jolt) => match jolt.target {
                        None => flight.jolt_from(jolt.from, jolt.strength),
                        Some(id) => bridge.jolt(id, jolt.from, jolt.strength),
                    },
                    Event::PlayerDestroyed => flight.crashed = true,
                    _ => {}
                }
            }
        }
        let [x, _, z] = flight.position;
        bridge.update_player_landing(
            &world.airport_scene,
            &service,
            &flight,
            world.surface(x, z).height,
        );
        for (at, order) in &script.orders {
            if *at != tick {
                continue;
            }
            let site = if *order == tore_sim::ai::wing::PlayerOrder::LandAtSelected {
                match ai_wings::AiWings::landing_site(
                    &world.airport_scene,
                    &world.airfield_anchors,
                    &service,
                ) {
                    Ok(site) => Some(site),
                    Err(message) => {
                        println!("t={tick} order={order:?} refused: {message}");
                        continue;
                    }
                }
            } else {
                None
            };
            let recipients = wing_recipients(Some(&bridge), None);
            let report =
                bridge.command_at(*order, combat.state.designated(), None, site.as_ref())?;
            println!("t={tick} order={order:?} reply={:?}", report.message);
            if let Some(recording) = &mut recording {
                recording.order(
                    &format!("{order:?}"),
                    recipients,
                    &report.message,
                    &report.radio,
                    None,
                );
            }
        }
        bridge.step(&mut combat.state, &flight, world)?;
        if let Some(recording) = &mut recording {
            for (id, message, friendly) in bridge.ejection_events.iter().skip(noted_ejections) {
                recording.wing_ejection(*id, message, *friendly);
            }
        }
        if let Some(attacker) = &mut attacker {
            attacker.observe(tick, &mut bridge);
        }
        noted_ejections = bridge.ejection_events.len();
        if let (Some(recording), Some(previous)) = (&mut recording, &previous) {
            combat.advance_render(&flight, Some(&bridge));
            let outcomes = combat.state.ledger.take_outcomes();
            recording.begin(replay::recorder::Tick {
                snapshot: combat.render_snapshot(),
                combat: &combat,
                flight: &flight,
                previous,
                pilot: &keys,
                wings: Some(&bridge),
                world,
                events: &events,
                outcomes: &outcomes,
            });
        }
        let now = combat.state.tick() as f64 / 120.;
        airfield_radio.step(
            now,
            &phrases,
            &mut comms,
            &flight,
            world,
            &service,
            Some(&bridge),
        );
        let crew = comms::crew(&hornet.profile);
        let state = &mut combat.state;
        radio_calls::step(
            &mut radio,
            &mut comms,
            &phrases,
            crew,
            &events,
            state,
            Some(&mut bridge),
            &flight,
        );
        let due = comms.due(now);
        if let Some(recording) = &mut recording {
            recording.radio(&due, crew);
            // Sounds are drained only when recording; nothing else reads them.
            let emissions = combat.state.take_sound_events();
            let stations = &combat.state.configuration().stations;
            let releases: Vec<(&str, &tore_formats::weapons::Weapon)> = events
                .iter()
                .filter_map(|event| match event {
                    tore_sim::combat::live::Event::Fired(i) => {
                        let weapon = &stations[*i].weapon;
                        Some((weapon.fire_sound.as_deref()?, weapon))
                    }
                    _ => None,
                })
                .collect();
            recording.sounds(&emissions, &releases);
            recording.end(None, &mut combat);
            if verify {
                pictures.push(combat.render_snapshot().clone());
            }
        }
        heard.extend(
            due.iter()
                .map(|c| format!("{now:.1}s {} {:?}", c.line(), c.stems)),
        );
        watch.observe(tick, &bridge, &flight, world);
    }
    watch.summary();
    println!(
        "player crashed={} gear={} x={:.1} y={:.1} z={:.1} hdg={:.1}",
        flight.crashed,
        flight.gear_down,
        flight.position[0],
        flight.position[1],
        flight.position[2],
        flight.yaw.to_degrees()
    );
    for line in bridge.probe_lines() {
        println!("{line}");
    }
    let report = debrief::capture(&combat, &flight, Some(&bridge));
    println!("AI probe debrief: {}", report.summary());
    println!("AI probe radio: calls={} heard={}", radio.made, radio.heard);
    for line in heard.iter().take(40) {
        println!("  {line}");
    }
    if let Some(attacker) = &attacker {
        attacker.summary(&combat, &flight, &bridge);
    }
    // A single number that changes if any actor's path changes, so two runs can
    // be compared without diffing every coordinate.
    let checksum = bridge
        .positions()
        .iter()
        .flatten()
        .fold(0u64, |acc, v| acc.rotate_left(7) ^ v.to_bits());
    println!(
        "AI probe totals: wings={} ticks={} shots={} dropped={} warnings={} live_projectiles={} player_hp={} target_hp={:?} checksum={checksum:016x}",
        bridge.slots().len(),
        bridge.mission().tick(),
        bridge.realised_launches,
        bridge.dropped_launches,
        bridge.threat_reports().len(),
        combat.state.projectiles.len(),
        combat.state.player_hp,
        combat
            .state
            .targets
            .iter()
            .map(|t| t.hp)
            .collect::<Vec<_>>()
    );
    if let Some(mut recording) = recording {
        recording.note(
            tore_replay::Event::new(tore_replay::vocab::kind::SYSTEM_END)
                .with(tore_replay::vocab::field::REASON, "probe finished"),
        );
        let path = recording
            .finish(&replay_footer(&combat, Some(&report), "probe finished"))
            .ok_or("the mission recording could not be finished; see the session log")?;
        if verify {
            let verification = replay::cli::verify(&path, &pictures)?;
            println!("AI probe {}", verification.line());
        }
    }
    Ok(())
}

/// Starts `--record-mission` for an AI probe, with the probe's settings and
/// what it does not simulate in the header, and records the start.
#[allow(clippy::too_many_arguments)]
fn start_probe_recording(
    record: &ProbeRecord,
    combat: &combat::Combat,
    flight: &flight::State,
    bridge: &ai_wings::AiWings,
    hornet: &aircraft::Airframe,
    world: &terrain::World,
    ticks: usize,
    ai_mission: ai_wings::Preset,
    script: &ProbeScript,
) -> AppResult<replay::recorder::Recorder> {
    use replay::{convert, recorder};
    let snapshot = combat.render_snapshot();
    let mut extra = vec![
        (
            "probe".to_owned(),
            "headless AI probe: no weather stepping, crew voice, music or cockpit messages"
                .to_owned(),
        ),
        ("probe.ticks".into(), ticks.to_string()),
        (
            "flight_model".into(),
            if flight.research.is_some() {
                "researched"
            } else {
                "legacy"
            }
            .into(),
        ),
        (
            "player.aircraft".into(),
            convert::identity_key(hornet.profile.id).into(),
        ),
        ("ai.mission".into(), ai_mission.to_string()),
        ("ai.aircraft".into(), bridge.len().to_string()),
    ];
    if script.takeoff {
        extra.push(("probe.script".into(), "takeoff".into()));
    }
    if let Some(attack) = script.attack {
        extra.push((
            "probe.attack".into(),
            format!("from tick {} repeat {} ticks", attack.from, attack.repeat),
        ));
    }
    for (tick, order) in &script.orders {
        extra.push(("probe.order".into(), format!("tick {tick}: {order:?}")));
    }
    let header = recorder::header(
        tore_replay::MissionKind::Probe,
        world,
        &convert::Presentation::of(snapshot),
        extra,
        std::time::SystemTime::now(),
    );
    let roster = recorder::roster(
        snapshot,
        &hornet.profile.name,
        true,
        Some(bridge),
        combat.models(),
    );
    let mut recording = recorder::Recorder::start(record.path.clone(), &header, &roster)
        .map_err(|error| format!("--record-mission {}: {error}", record.path.display()))?;
    recording.begin(recorder::Tick {
        snapshot,
        combat,
        flight,
        previous: flight,
        pilot: &flight::PilotInput::default(),
        wings: Some(bridge),
        world,
        events: &[],
        outcomes: &[],
    });
    Ok(recording)
}

/// What the app should do when there is no usable pack in application data.
/// Kept free of the file system so the decision itself can be tested.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    /// The pack loaded; start the game.
    Play,
    /// A source the app already knows: import it without asking.
    ImportNow(PathBuf),
    /// Show the locate screen, with this path in the field if there is one.
    Ask(Option<PathBuf>),
    /// No window and nothing to import from: report it in the terminal.
    Fail,
}

/// `known` is the remembered source or the developer checkout, whichever
/// detects first; `prefill` is the first automatically detected source.
fn next_step(
    loaded: bool,
    interactive: bool,
    known: Option<PathBuf>,
    prefill: Option<PathBuf>,
) -> Step {
    match (loaded, known, interactive) {
        (true, ..) => Step::Play,
        (false, Some(path), _) => Step::ImportNow(path),
        (false, None, true) => Step::Ask(prefill),
        (false, None, false) => Step::Fail,
    }
}

/// The path the shell imports without waiting for the player: a source the app
/// already knows, or, under `--smoke-test`, whatever the field was prefilled
/// with, so a first run can be checked end to end without clicks.
fn auto_import_path(step: &Step, prefill: Option<&Path>, smoke_test: bool) -> Option<PathBuf> {
    match step {
        Step::ImportNow(path) => Some(path.clone()),
        _ if smoke_test => prefill.map(Path::to_path_buf),
        _ => None,
    }
}

/// How a session of the game application ended.
enum Outcome {
    Done,
    /// Pref asked for another import. The frame is the menu the player was
    /// looking at, which the locate screen draws over.
    Reimport(Vec<u8>),
}

/// What a session starts from.
enum Session {
    First,
    Reimport(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellOutcome {
    Quit,
    Continue,
}

/// Progress and results from the importing worker thread. The error is already
/// a plain string, because `Box<dyn Error>` cannot cross a thread boundary.
enum ImportMessage {
    Progress(assets::Progress),
    Done(Vec<String>),
    Failed(String),
}

/// The pre-game shell: the locate screen, its own blit-only presenter, and the
/// import running on a worker thread. It is a second winit application handler
/// because every game object is built from imported assets, so the game `App`
/// cannot exist before an import does.
struct LocateShell {
    locate: locate::Locate,
    data: PathBuf,
    presenter: Option<canvas_present::CanvasPresenter>,
    font: menu::Sprite,
    small: menu::Sprite,
    background: Option<Vec<u8>>,
    pixels: Vec<u8>,
    worker: Option<std::sync::mpsc::Receiver<ImportMessage>>,
    /// When the starting bar last moved, so pointer motion cannot speed it up.
    last_tick: Instant,
    /// Last pointer position in canvas coordinates: winit reports a button
    /// press without one, so the latest motion is what a click uses.
    last_pointer: Option<(f64, f64)>,
    /// Imported as soon as the window is up, without waiting for the player.
    auto_import: Option<PathBuf>,
    /// `--smoke-test` continues as soon as the import finishes.
    auto_continue: bool,
    outcome: ShellOutcome,
    error: Option<Box<dyn Error>>,
    /// Live window mode, carried back to the game window so one Alt-Enter in
    /// the shell holds for the rest of the session.
    fullscreen: bool,
    /// Alt state, which winit reports separately from the key event.
    modifiers: ModifiersState,
}

/// winit key names the locate screen understands. Printable characters go
/// through `text_input` instead, so their case survives.
fn locate_key_name(key: &Key) -> Option<&'static str> {
    use winit::keyboard::NamedKey;
    let Key::Named(named) = key else {
        return None;
    };
    Some(match named {
        NamedKey::Enter => "Enter",
        NamedKey::Escape => "Escape",
        NamedKey::Tab => "Tab",
        NamedKey::Backspace => "Backspace",
        NamedKey::Delete => "Delete",
        NamedKey::Home => "Home",
        NamedKey::End => "End",
        NamedKey::ArrowUp => "ArrowUp",
        NamedKey::ArrowDown => "ArrowDown",
        NamedKey::ArrowLeft => "ArrowLeft",
        NamedKey::ArrowRight => "ArrowRight",
        _ => return None,
    })
}

impl LocateShell {
    fn redraw(&mut self) {
        if let Some(presenter) = &self.presenter {
            presenter.window.request_redraw();
        }
    }
    fn settle(&mut self, event_loop: &ActiveEventLoop, event: locate::Event) {
        match event {
            locate::Event::None => {}
            locate::Event::Import(path) => self.start_import(path),
            locate::Event::Quit => {
                self.outcome = ShellOutcome::Quit;
                event_loop.exit();
            }
            locate::Event::Continue => {
                self.outcome = ShellOutcome::Continue;
                event_loop.exit();
            }
        }
        self.redraw();
    }
    /// Classify the chosen path, then read it on a worker thread so the screen
    /// keeps drawing. Nothing partial is kept: a failure leaves the old pack.
    fn start_import(&mut self, path: PathBuf) {
        if self.worker.is_some() {
            return;
        }
        log::info!("Import source: {}", path.display());
        let source = match media_source::MediaSource::detect(&path) {
            Ok(source) => source,
            Err(error) => {
                log::warn!("Import source detection failed: {error}");
                self.locate.set_phase(locate::Phase::Failed {
                    reason: error.to_string(),
                });
                return;
            }
        };
        self.locate.clear_hint();
        // Say so at once: the worker reads nothing countable until it has
        // identified the build and found what to read.
        self.locate.set_phase(locate::Phase::Starting {
            step: String::from("Opening the game files"),
        });
        let (sender, receiver) = std::sync::mpsc::channel();
        let data = self.data.clone();
        std::thread::spawn(move || {
            log::info!("Import worker started");
            let reports = sender.clone();
            let result = Assets::import_with_progress(&source, &data, &mut |progress| {
                // Archive changes are bounded progress checkpoints, not per-resource logging.
                match &progress {
                    assets::Progress::Preparing(step) => log::info!("Import step: {step}"),
                    assets::Progress::Reading {
                        archive, done: 0, ..
                    } => {
                        log::info!("Import archive: {archive}")
                    }
                    assets::Progress::Reading { .. } => {}
                }
                let _ = reports.send(ImportMessage::Progress(progress));
            });
            let _ = sender.send(match result {
                // The decoded assets are dropped here; the caller reloads the
                // pack that was just written, which proves it reads back.
                Ok(outcome) => {
                    log::info!("Import worker completed successfully");
                    ImportMessage::Done(outcome.summary)
                }
                Err(error) => {
                    log::error!("Import failed: {error}");
                    ImportMessage::Failed(error.to_string())
                }
            });
        });
        self.worker = Some(receiver);
    }
    /// Drain the worker channel. Returns true when the screen changed.
    fn poll_import(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(receiver) = self.worker.take() else {
            return false;
        };
        let mut changed = false;
        let mut running = true;
        loop {
            match receiver.try_recv() {
                Ok(ImportMessage::Progress(assets::Progress::Preparing(step))) => {
                    self.locate.set_phase(locate::Phase::Starting {
                        step: step.to_owned(),
                    });
                    changed = true;
                }
                Ok(ImportMessage::Progress(assets::Progress::Reading {
                    archive,
                    done,
                    total,
                })) => {
                    self.locate.set_phase(locate::Phase::Importing {
                        archive,
                        resources_done: done,
                        resources_total: total,
                    });
                    changed = true;
                }
                Ok(ImportMessage::Done(summary)) => {
                    self.locate.set_phase(locate::Phase::Done { summary });
                    changed = true;
                    running = false;
                }
                Ok(ImportMessage::Failed(reason)) => {
                    self.locate.set_phase(locate::Phase::Failed { reason });
                    changed = true;
                    running = false;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    running = false;
                    break;
                }
            }
        }
        if running {
            self.worker = Some(receiver);
        } else if self.auto_continue && matches!(self.locate.phase(), locate::Phase::Done { .. }) {
            self.outcome = ShellOutcome::Continue;
            event_loop.exit();
        }
        changed
    }
}

impl ApplicationHandler for LocateShell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.presenter.is_some() {
            return;
        }
        let result = (|| {
            diagnostics::stage("first-run window creation");
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("T.O.R.E-Fighters - Locate Fighters Anthology")
                        .with_inner_size(LogicalSize::new(960.0, 720.0))
                        .with_min_inner_size(LogicalSize::new(640.0, 480.0))
                        .with_fullscreen(fullscreen_attribute(self.fullscreen)),
                )?,
            );
            diagnostics::stage_done();
            pollster::block_on(canvas_present::CanvasPresenter::new(window))
        })();
        match result {
            Ok(presenter) => {
                presenter.window.request_redraw();
                self.presenter = Some(presenter);
                if let Some(path) = self.auto_import.take() {
                    self.start_import(path);
                }
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(presenter) = self.presenter.as_mut() else {
            return;
        };
        if presenter.window.id() != id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.outcome = ShellOutcome::Quit;
                event_loop.exit();
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.last_pointer = None;
                presenter.resize();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::DroppedFile(path) => {
                // A dropped file or folder is classified before it reaches the
                // field, so the field always holds a folder the importer can read.
                match media_source::MediaSource::detect(&path) {
                    Ok(source) => self.locate.dropped_path(source.path),
                    Err(error) => {
                        self.locate.dropped_path(path);
                        self.locate.set_hint(error.to_string());
                    }
                }
                self.redraw();
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // Same window-mode toggle the game uses, before the field sees
                // the key, so Alt-Enter never submits the locate form.
                if self.modifiers.alt_key()
                    && event.logical_key == Key::Named(winit::keyboard::NamedKey::Enter)
                {
                    if !event.repeat {
                        self.fullscreen = !self.fullscreen;
                        presenter
                            .window
                            .set_fullscreen(fullscreen_attribute(self.fullscreen));
                        self.last_pointer = None;
                        self.redraw();
                    }
                    return;
                }
                if let Some(name) = locate_key_name(&event.logical_key) {
                    let result = self.locate.key(name);
                    self.settle(event_loop, result);
                    return;
                }
                // Space continues a finished import and otherwise types a space.
                if event.logical_key == Key::Named(winit::keyboard::NamedKey::Space) {
                    let result = self.locate.key(" ");
                    self.locate.text_input(' ');
                    self.settle(event_loop, result);
                    return;
                }
                if let Some(text) = &event.text {
                    for ch in text.chars().filter(|ch| !ch.is_control()) {
                        self.locate.text_input(ch);
                    }
                    self.redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_pointer = presenter.viewport().point(position.x, position.y);
                if let Some((x, y)) = self.last_pointer {
                    self.locate.hover(x, y);
                    self.redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some((x, y)) = self.last_pointer {
                    let result = self.locate.click(x, y);
                    self.settle(event_loop, result);
                }
            }
            WindowEvent::RedrawRequested => {
                self.poll_import(event_loop);
                self.locate.draw(
                    &mut self.pixels,
                    &self.font,
                    &self.small,
                    self.background.as_deref(),
                );
                let pixels = std::mem::take(&mut self.pixels);
                if let Some(presenter) = &mut self.presenter
                    && let Err(error) = presenter.present(&pixels)
                {
                    self.error = Some(error);
                    event_loop.exit();
                }
                self.pixels = pixels;
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let changed = self.poll_import(event_loop);
        // The starting bar steps at the poll rate below, whatever else wakes
        // the loop.
        let animating = self.worker.is_some()
            && self.last_tick.elapsed() >= Duration::from_millis(33)
            && self.locate.tick();
        if animating {
            self.last_tick = Instant::now();
        }
        if changed || animating {
            self.redraw();
        }
        event_loop.set_control_flow(if self.worker.is_some() {
            // Poll the import often enough for a smooth progress bar.
            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(33))
        } else {
            ControlFlow::Wait
        });
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Release the GPU backend while the display connection is still alive,
        // for the same reason the game renderer does; see docs/DEVELOPMENT.md.
        self.presenter = None;
    }
}

/// What the shell does without waiting for the player: the source to import as
/// soon as it opens, and whether a finished import continues into the game.
struct AutoImport {
    path: Option<PathBuf>,
    continue_when_done: bool,
}

/// Show the locate screen until the player quits or continues.
fn locate_shell(
    event_loop: &mut EventLoop<()>,
    data: &Path,
    prefill: Option<PathBuf>,
    candidates: Vec<media_source::MediaSource>,
    background: Option<Vec<u8>>,
    auto: AutoImport,
    window: &mut WindowState,
) -> AppResult<ShellOutcome> {
    let candidates: Vec<locate::Candidate> = candidates
        .into_iter()
        .map(|source| locate::Candidate {
            path: source.path,
            kind: match source.kind {
                media_source::Kind::Installed => locate::SourceKind::Installed,
                media_source::Kind::Disc => locate::SourceKind::Disc,
            },
        })
        .collect();
    match &auto.path {
        Some(path) => println!(
            "Locate Fighters Anthology: {} detected source(s), importing {}",
            candidates.len(),
            path.display()
        ),
        None => println!(
            "Locate Fighters Anthology: {} detected source(s), waiting for a choice",
            candidates.len()
        ),
    }
    let mut shell = LocateShell {
        locate: locate::Locate::new(prefill.map(|path| path.display().to_string()), candidates),
        data: data.to_path_buf(),
        presenter: None,
        font: menu::flat_font([235, 239, 243]),
        small: menu::flat_font([210, 219, 230]),
        background: background.filter(|art| art.len() == menu::WIDTH * menu::HEIGHT * 4),
        pixels: vec![0; menu::WIDTH * menu::HEIGHT * 4],
        worker: None,
        last_tick: Instant::now(),
        auto_import: auto.path,
        auto_continue: auto.continue_when_done,
        outcome: ShellOutcome::Quit,
        error: None,
        last_pointer: None,
        fullscreen: window.fullscreen,
        modifiers: ModifiersState::empty(),
    };
    event_loop.run_app_on_demand(&mut shell)?;
    if shell.fullscreen != window.fullscreen {
        window.toggle();
    }
    match shell.error {
        Some(error) => Err(error),
        None => Ok(shell.outcome),
    }
}

/// Headless locate-screen preview (`--snapshot PATH --snapshot-state locate`,
/// `locate-starting`, `locate-importing` or `locate-done`). The candidate list and the import
/// figures are fixed so the layout is reviewable on any machine, with or
/// without media. The figures are those of a disc 1 import.
fn locate_snapshot(path: &Path, state: &str) -> AppResult<()> {
    use std::io::Write;
    // The progress and completion previews show the disc being imported.
    let chosen = (state != "locate").then(|| String::from("/run/media/pilot/FA_DISC1"));
    let mut screen = locate::Locate::new(
        chosen,
        vec![
            locate::Candidate {
                path: PathBuf::from("gameassets/fighters-anthology"),
                kind: locate::SourceKind::Installed,
            },
            locate::Candidate {
                path: PathBuf::from("/run/media/pilot/FA_DISC1"),
                kind: locate::SourceKind::Disc,
            },
        ],
    );
    match state {
        "locate" => screen.set_hint(
            "Drop the mounted disc 1 folder on this window, or type the folder above.".into(),
        ),
        "locate-starting" => screen.set_phase(locate::Phase::Starting {
            step: "Finding aircraft and theaters".into(),
        }),
        "locate-importing" => screen.set_phase(locate::Phase::Importing {
            archive: "FA_2.LIB".into(),
            resources_done: 1344,
            resources_total: Some(2247),
        }),
        "locate-done" => screen.set_phase(locate::Phase::Done {
            summary: vec![
                "Build read: FA.EXE 1.0 (disc)".into(),
                "FA_1.LIB: 1996 entries read".into(),
                "FA_2.LIB: 5405 entries read".into(),
            ],
        }),
        other => return Err(format!("unknown locate snapshot state {other}").into()),
    }
    let mut pixels = vec![0u8; menu::WIDTH * menu::HEIGHT * 4];
    screen.draw(
        &mut pixels,
        &menu::flat_font([235, 239, 243]),
        &menu::flat_font([210, 219, 230]),
        None,
    );
    let mut file = std::fs::File::create(path)?;
    write!(file, "P6\n{} {}\n255\n", menu::WIDTH, menu::HEIGHT)?;
    for pixel in pixels.chunks_exact(4) {
        file.write_all(&pixel[..3])?;
    }
    println!("Locate preview: {}", path.display());
    Ok(())
}

/// The saved window-mode preference. Borderless fullscreen is the default, so
/// a missing, unreadable or older preferences file starts fullscreen.
fn saved_fullscreen() -> bool {
    let Ok(directory) = assets::data_directory() else {
        return true;
    };
    preferences::read(&directory.join("preferences-v1.conf"))
        .ok()
        .and_then(|text| preferences::Preferences::parse(&text).ok())
        .is_none_or(|saved| saved.fullscreen)
}

/// The event loop is created at most once per process and only when a window
/// is actually wanted, so headless runs still work without a display.
fn shared_event_loop(slot: &mut Option<EventLoop<()>>) -> AppResult<&mut EventLoop<()>> {
    if slot.is_none() {
        diagnostics::stage("event loop creation");
        slot.replace(EventLoop::new()?);
        diagnostics::stage_done();
    }
    Ok(slot.as_mut().expect("the event loop was just created"))
}

fn sessions(event_loop: &mut Option<EventLoop<()>>) -> AppResult<()> {
    let mut session = Session::First;
    loop {
        match run(event_loop, session)? {
            Outcome::Done => return Ok(()),
            // Pref asked for another import: the shell runs again, and on
            // Continue the game is rebuilt from the new pack.
            Outcome::Reimport(frame) => session = Session::Reimport(frame),
        }
    }
}

fn main() -> std::process::ExitCode {
    diagnostics::init();
    let interactive = startup::interactive();
    let result = std::panic::catch_unwind(|| {
        if let Some(result) = startup::self_test() {
            return result;
        }
        let mut event_loop = None;
        sessions(&mut event_loop)
    });
    match result {
        Ok(Ok(())) => {
            diagnostics::finish_success();
            std::process::ExitCode::SUCCESS
        }
        Ok(Err(error)) => {
            diagnostics::report_error(error.as_ref(), interactive);
            std::process::ExitCode::FAILURE
        }
        Err(_) => {
            diagnostics::report_panic(interactive);
            std::process::ExitCode::from(101)
        }
    }
}

fn run(event_loop: &mut Option<EventLoop<()>>, session: Session) -> AppResult<Outcome> {
    diagnostics::stage("argument parsing and startup options");
    if matches!(session, Session::First) {
        println!("{}", version::label());
    }
    let mut args = std::env::args().skip(1);
    let mut live_fire = false;
    let mut dummy_aircraft = Vec::new();
    let mut jammer_on = false;
    let mut combat_smoke = false;
    let mut missile_acceptance = false;
    let mut record_combat = None;
    let mut replay_combat = None;
    let mut combat_probe = None;
    let mut ai_wings_enabled = false;
    let mut fixture_wings = false;
    let mut ai_probe = None;
    let mut ai_roster_probe = false;
    // Mission recordings: what happened, not re-simulation tapes.
    let mut record_mission: Option<PathBuf> = None;
    let mut verify_render = false;
    let mut recording_info: Option<PathBuf> = None;
    let mut recording_log: Option<PathBuf> = None;
    let mut recording_acmi: Option<PathBuf> = None;
    let mut recording_diff: Option<(PathBuf, PathBuf)> = None;
    let mut recording_out: Option<PathBuf> = None;
    let mut recording_from: Option<f64> = None;
    let mut recording_to: Option<f64> = None;
    let mut recording_ids: Option<Vec<u32>> = None;
    let mut recording_rate: Option<f64> = None;
    let mut recording_guns = false;
    // Session only. `docs/spec/ai-experience.md` records the flight-menu
    // enemy-skill preference's persistence as untraced, so this setting is not
    // written to the preferences file and does not survive a restart.
    let mut enemy_skill = None;
    let mut ai_mission = ai_wings::Preset::Free;
    let mut combat_commands = Vec::new();
    let mut weapon_slot: Option<usize> = None;
    let mut input_profile = None;
    let mut native_input = true;
    let mut record_input = None;
    let mut replay_input = None;
    let mut input_seconds = None;
    let mut write_input_profile = None;
    let mut test_rumble = None;
    let (mut import, mut snapshot) = (None, None);
    let mut snapshot_state = String::from("normal");
    let mut background = None;
    let mut theater_code = String::from("UKR");
    let mut aircraft_id = tore_formats::aircraft::AircraftId::F18;
    let mut initial_screen = Screen::Main;
    let mut flight_view = 0;
    let mut flight_reference = flight_views::Reference::Player;
    let mut flight_look = [0f32; 2];
    let mut flight_zoom = 1f32;
    let mut flight_menu = false;
    let mut flight_map = false;
    let mut weapon_diagnostics = false;
    let mut controls_menu = false;
    let mut flight_mode_arg = None;
    let mut native_tables_path: Option<PathBuf> = None;
    let mut window_size = [960, 720];
    let mut instrument_page = None;
    let mut sensor_channel = None;
    let mut scope_range = None;
    let mut scope_history = false;
    let mut instrument_layout = instruments::Layout::Large;
    let mut capture_terrain = None;
    let mut native_flight_report = false;
    let mut native_flight_trig = None;
    let mut headless_ticks = None;
    let mut flight_probe_ticks = None;
    let mut flight_devices = None;
    let mut flight_controls = None;
    let mut flight_throttle = None;
    let mut flight_bay = None;
    let mut damage_preview = None;
    let mut ejection_preview = None;
    let mut hud_target_preview: Option<[f64; 3]> = None;
    let mut damage_preview_section = tore_sim::combat::live::DamageSection::Nose;
    let mut damage_preview_ticks = 240usize;
    let mut countermeasure_preview = None;
    let mut maneuver = String::from("level");
    let mut panel_snapshot = None;
    let mut systems_preview: Vec<usize> = Vec::new();
    let mut validate_creator = false;
    let mut sensor_summary = false;
    let mut validate_weather = false;
    let mut validate_maps = false;
    let mut weather_condition: Option<usize> = None;
    let mut airport_probe: Option<(u32, tore_sim::airport::Aircraft, Option<[f64; 2]>)> = None;
    let mut ground_start_airport: Option<u32> = None;
    let mut probe_script = ProbeScript::default();
    let mut separation_nm: Option<f64> = None;
    let mut launch_creator = false;
    let (mut smoke_test, mut no_audio, mut import_only) = (false, false, false);
    // `--windowed`, and any flag that fixes the window size, opt out of the
    // borderless fullscreen default.
    let mut windowed_flag = false;
    let mut graphics_flags: Vec<(String, String)> = Vec::new();
    let mut window_size_flag = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-controllers" => native_input = false,
            "--launch-quick-mission" => { launch_creator=true; initial_screen=Screen::Flight; },
            "--ground-start" => {
                ground_start_airport=Some(args.next().ok_or("--ground-start needs an airport number")?.parse()?);
            }
            "--separation" => {
                let nm: f64 = args.next().ok_or("--separation needs a distance in nautical miles")?.parse()?;
                if !quick_mission::SEPARATION_NM.contains(&nm) {
                    return Err(format!("--separation needs one of {:?} nautical miles", quick_mission::SEPARATION_NM).into());
                }
                separation_nm = Some(nm);
            }
            "--probe-wing-only" => probe_script.wing_only = true,
            "--probe-wing-size" => {
                let size: usize = args.next().ok_or("--probe-wing-size needs 1..5")?.parse()?;
                if !(1..=5).contains(&size) {
                    return Err("--probe-wing-size needs 1..5".into());
                }
                probe_script.wing_size = Some(size);
            }
            "--probe-wing-order" => probe_script.orders.push(ProbeScript::parse_order(
                &args.next().ok_or("--probe-wing-order needs TICK:ORDER")?,
            )?),
            "--probe-attack" => probe_script.attack = Some(ProbeScript::parse_attack(
                &args.next().ok_or("--probe-attack needs TICK or TICK:REPEAT_SECONDS")?,
            )?),
            "--probe-trace" => {
                let seconds: f64 = args.next().ok_or("--probe-trace needs seconds")?.parse()?;
                if !(seconds > 0. && seconds <= 3600.) {
                    return Err("--probe-trace needs 0..3600 seconds".into());
                }
                probe_script.trace_ticks = (seconds * 120.).round().max(1.) as u64;
            }
            "--probe-player-home" => {
                let usage = "--probe-player-home needs FROM:UNTIL ticks";
                let text = args.next().ok_or(usage)?;
                let (from, until) = text.split_once(':').ok_or(usage)?;
                probe_script.home = Some((from.parse()?, until.parse()?));
            }
            "--airport-probe" => {
                let value = args.next().ok_or("--airport-probe needs ID,X,Y,Z,NAV,GEAR[,HEADING,PITCH]")?;
                let fields: Vec<_> = value.split(',').collect();
                if !matches!(fields.len(), 6 | 8) {
                    return Err("--airport-probe needs ID,X,Y,Z,NAV,GEAR[,HEADING,PITCH]".into());
                }
                let flag = |text: &str| match text { "0" => Ok(false), "1" => Ok(true), _ => Err("airport probe flags need 0 or 1") };
                let position = [fields[1].parse()?, fields[2].parse()?, fields[3].parse()?];
                if position.iter().any(|value: &f64| !value.is_finite()) {
                    return Err("airport probe position must be finite".into());
                }
                let angles = if fields.len() == 8 {
                    let angles = [fields[6].parse::<f64>()?, fields[7].parse::<f64>()?];
                    if !angles.iter().all(|value| value.is_finite())
                        || angles[0].abs() > 360_000.
                        || !(-90. ..=90.).contains(&angles[1])
                    {
                        return Err("airport probe heading/pitch outside bounded degree range".into());
                    }
                    Some(angles)
                } else {
                    None
                };
                airport_probe = Some((fields[0].parse()?, tore_sim::airport::Aircraft {
                    position,
                    forward: [0., 0., 1.],
                    nav_mode: flag(fields[4])?,
                    gear_down: flag(fields[5])?,
                    supported: false,
                    alive: true,
                    speed_fps: 140.0,
                }, angles));
            }
            "--record-input" => {
                record_input = Some(PathBuf::from(
                    args.next().ok_or("--record-input needs a new path")?,
                ))
            }
            "--replay-input" => {
                replay_input = Some(PathBuf::from(
                    args.next().ok_or("--replay-input needs a tape path")?,
                ))
            }
            "--combat-command" => {
                let name = args.next().ok_or(
                    "--combat-command needs arm/jettison/clear/class/fail/next/target/designate/damage/incoming/target-jammer",
                )?;
                if combat_commands.len() >= 32 {
                    return Err("too many combat setup commands".into());
                }
                combat_commands
                    .push(combat_tape::command(&name).ok_or("unknown combat setup command")?);
                live_fire = true;
                initial_screen = Screen::Flight;
            }
            "--record-combat" => {
                record_combat = Some(PathBuf::from(
                    args.next().ok_or("--record-combat requires a new path")?,
                ));
                live_fire = true;
                initial_screen = Screen::Flight;
            }
            "--replay-combat" => {
                replay_combat = Some(PathBuf::from(
                    args.next().ok_or("--replay-combat requires a path")?,
                ));
            }
            "--jammer-on" => {
                jammer_on = true;
                live_fire = true;
                initial_screen = Screen::Flight;
            }
            "--hud-target-preview" => {
                let values: Vec<f64> = args.next().ok_or("--hud-target-preview needs bearing,elevation,feet")?
                    .split(',').map(str::parse).collect::<Result<_,_>>()?;
                if values.len()!=3 || values.iter().any(|v| !v.is_finite()) || values[0].abs()>180. || values[1].abs()>90. || !(100. ..=60000.).contains(&values[2]) {
                    return Err("--hud-target-preview needs bearing +/-180, elevation +/-90, range 100..60000 feet".into());
                }
                hud_target_preview = Some([values[0],values[1],values[2]]);
                live_fire = true;
            }
            "--damage-preview-ticks" => {
                damage_preview_ticks = args.next().ok_or("--damage-preview-ticks needs 1..7200")?.parse()?;
                if !(1..=7200).contains(&damage_preview_ticks) { return Err("--damage-preview-ticks needs 1..7200".into()); }
            }
            "--countermeasure-preview" => {
                let ticks: usize = args.next().ok_or("--countermeasure-preview needs 1..7200")?.parse()?;
                if !(1..=7200).contains(&ticks) { return Err("--countermeasure-preview needs 1..7200".into()); }
                countermeasure_preview = Some(ticks);
            }
            "--damage-preview" => {
                let fraction = args.next().ok_or("--damage-preview needs 0..1")?.parse::<f64>()?;
                if !fraction.is_finite() || !(0. ..=1.).contains(&fraction) { return Err("--damage-preview needs 0..1".into()); }
                damage_preview = Some(fraction);
                initial_screen = Screen::Flight;
            }
            "--damage-preview-section" => {
                damage_preview_section = match args.next().as_deref() {
                    Some("nose") => tore_sim::combat::live::DamageSection::Nose,
                    Some("cockpit") => tore_sim::combat::live::DamageSection::Cockpit,
                    Some("core") => tore_sim::combat::live::DamageSection::Core,
                    Some("left-wing") => tore_sim::combat::live::DamageSection::LeftWing,
                    Some("right-wing") => tore_sim::combat::live::DamageSection::RightWing,
                    Some("tail") => tore_sim::combat::live::DamageSection::Tail,
                    _ => return Err("--damage-preview-section needs nose|cockpit|core|left-wing|right-wing|tail".into()),
                };
            }
            "--dummy-aircraft" => {
                let value = args.next().ok_or("--dummy-aircraft needs ID,COUNT")?;
                let (id, count) = value.split_once(',').ok_or("--dummy-aircraft needs ID,COUNT")?;
                dummy_aircraft.push((tore_formats::aircraft::AircraftId::parse(id)?, count.parse::<usize>()?));
                initial_screen = Screen::Flight;
            }
            "--live-fire" => {
                live_fire = true;
                initial_screen = Screen::Flight;
            }
            "--ai-wings" => ai_wings_enabled = true,
            "--ai-mission" => {
                ai_mission = args.next().ok_or("--ai-mission needs a preset")?.parse()?;
            }
            "--fixture-wings" => fixture_wings = true,
            "--enemy-skill" => {
                enemy_skill = Some(
                    match args
                        .next()
                        .ok_or("--enemy-skill needs novice or average")?
                        .as_str()
                    {
                        "novice" => tore_sim::ai::experience::EnemySkillOverride::AllNovice,
                        "average" => tore_sim::ai::experience::EnemySkillOverride::AllAverage,
                        _ => return Err("--enemy-skill needs novice or average".into()),
                    },
                );
            }
            "--ai-probe-ticks" | "--ai-roster-probe-ticks" => {
                ai_roster_probe = arg == "--ai-roster-probe-ticks";
                let ticks: usize = args
                    .next()
                    .ok_or("--ai-probe-ticks requires 1..216000")?
                    .parse()?;
                if !(1..=216_000).contains(&ticks) {
                    return Err("AI probe tick limit exceeded".into());
                }
                ai_probe = Some(ticks);
                ai_wings_enabled = true;
            }
            "--record-mission" => {
                record_mission = Some(PathBuf::from(
                    args.next().ok_or("--record-mission needs a new path")?,
                ));
            }
            "--verify-render" => verify_render = true,
            "--recording-info" => {
                recording_info = Some(PathBuf::from(
                    args.next().ok_or("--recording-info needs a recording")?,
                ));
            }
            "--recording-log" => {
                recording_log = Some(PathBuf::from(
                    args.next().ok_or("--recording-log needs a recording")?,
                ));
            }
            "--recording-acmi" => {
                recording_acmi = Some(PathBuf::from(
                    args.next().ok_or("--recording-acmi needs a recording")?,
                ));
            }
            "--recording-diff" => {
                let usage = "--recording-diff needs two recordings";
                let a = PathBuf::from(args.next().ok_or(usage)?);
                let b = PathBuf::from(args.next().ok_or(usage)?);
                recording_diff = Some((a, b));
            }
            "--out" => {
                recording_out = Some(PathBuf::from(
                    args.next().ok_or("--out needs a path")?,
                ));
            }
            "--from" | "--to" => {
                let seconds: f64 = args
                    .next()
                    .ok_or(format!("{arg} needs seconds of mission time"))?
                    .parse()?;
                if !(seconds.is_finite() && seconds >= 0.) {
                    return Err(format!("{arg} needs seconds of mission time, 0 or more").into());
                }
                if arg == "--from" {
                    recording_from = Some(seconds);
                } else {
                    recording_to = Some(seconds);
                }
            }
            "--ids" => {
                recording_ids = Some(
                    args.next()
                        .ok_or("--ids needs aircraft ids such as 0,7")?
                        .split(',')
                        .map(str::parse)
                        .collect::<Result<_, _>>()?,
                );
            }
            "--rate" => {
                let hz: f64 = args
                    .next()
                    .ok_or("--rate needs samples per second")?
                    .parse()?;
                if !(hz.is_finite() && hz > 0. && hz <= 120.) {
                    return Err("--rate needs samples per second above 0 and at most 120".into());
                }
                recording_rate = Some(hz);
            }
            "--guns" => recording_guns = true,
            "--missile-acceptance" => missile_acceptance = true,
            "--compatibility-weapons" => combat_commands.push(tore_sim::combat::live::Command::CompatibilityWeapons),
            "--combat-smoke" => {
                combat_smoke = true;
            }
            "--weapon-slot" => {
                weapon_slot = Some(
                    args.next()
                        .ok_or("--weapon-slot requires a 1-based PT weapon slot")?
                        .parse()?,
                );
            }
            "--combat-probe-ticks" => {
                let ticks: usize = args
                    .next()
                    .ok_or("--combat-probe-ticks requires 1..7200")?
                    .parse()?;
                if !(1..=7200).contains(&ticks) {
                    return Err("combat probe tick limit exceeded".into());
                }
                combat_probe = Some(ticks);
                live_fire = true;
                initial_screen = Screen::Flight;
            }
            "--input-profile" => {
                input_profile = Some(PathBuf::from(
                    args.next().ok_or("--input-profile needs a path")?,
                ))
            }
            "--list-inputs" => input_seconds = Some(2),
            "--monitor-inputs" => {
                input_seconds = Some(
                    args.next()
                        .ok_or("--monitor-inputs needs seconds")?
                        .parse::<u64>()?,
                )
            }
            "--write-input-profile" => {
                write_input_profile = Some(PathBuf::from(
                    args.next()
                        .ok_or("--write-input-profile needs a new path")?,
                ));
                input_seconds.get_or_insert(2);
            }
            "--test-rumble" => {
                test_rumble = Some(
                    args.next()
                        .ok_or("--test-rumble needs a device id or only")?,
                );
                input_seconds.get_or_insert(2);
            }
            "--researched-flight" => flight_mode_arg = Some(true),
            "--legacy-flight" => flight_mode_arg = Some(false),
            "--native-flight-tables" => native_tables_path = Some(args.next().ok_or("--native-flight-tables needs a directory containing sine-q15.bin and atan-pa.bin")?.into()),
            "--capture-terrain" => {
                capture_terrain = Some(PathBuf::from(
                    args.next().ok_or("--capture-terrain needs a .ppm path")?,
                ));
                initial_screen = Screen::Viewer;
                smoke_test = true;
            }
            "--aircraft" => {
                aircraft_id = tore_formats::aircraft::AircraftId::parse(
                    &args.next().ok_or("--aircraft needs a supported aircraft ID (see --help)")?,
                )?;
            }
            "--theater" => {
                theater_code = args
                    .next()
                    .ok_or("--theater needs a code")?
                    .to_ascii_uppercase()
            }
            "--window-size" => {
                let size = args.next().ok_or("--window-size needs WIDTHxHEIGHT")?;
                let (w, h) = size
                    .split_once('x')
                    .ok_or("--window-size needs WIDTHxHEIGHT")?;
                window_size = [w.parse()?, h.parse()?];
                window_size_flag = true;
                if !(640..=3840).contains(&window_size[0])
                    || !(480..=2160).contains(&window_size[1])
                {
                    return Err("window size outside 640x480..3840x2160".into());
                }
            }
            "--instrument-layout" => {
                instrument_layout = match args.next().as_deref() {
                    Some("large") => instruments::Layout::Large,
                    Some("small") => instruments::Layout::Small,
                    _ => return Err("--instrument-layout needs large or small".into()),
                };
            }
            "--instrument-page" => {
                let page = args
                    .next()
                    .ok_or("--instrument-page needs 0..9")?
                    .parse::<u8>()?;
                if !(0..=9).contains(&page) {
                    return Err("--instrument-page needs 0..9".into());
                }
                instrument_page = Some(page);
            }
            "--flight-zoom" => {
                flight_zoom = args.next().ok_or("--flight-zoom needs 0.5..4")?.parse()?;
                if !flight_zoom.is_finite() || !(0.5..=4.).contains(&flight_zoom) {
                    return Err("--flight-zoom needs a finite value in 0.5..4".into());
                }
            }
            "--flight-look" => {
                let value = args
                    .next()
                    .ok_or("--flight-look needs YAW,PITCH in degrees")?;
                let (yaw, pitch) = value
                    .split_once(',')
                    .ok_or("--flight-look needs YAW,PITCH in degrees")?;
                flight_look = [yaw.parse()?, pitch.parse()?];
                if flight_look.iter().any(|v| !v.is_finite() || v.abs() > 360.) {
                    return Err(
                        "flight look angles must be finite and within -360..360 degrees".into(),
                    );
                }
            }
            "--flight-reference" => {
                flight_reference = match args.next().as_deref() {
                    Some("player") => flight_views::Reference::Player,
                    Some("target") => flight_views::Reference::Target,
                    Some("missile") => flight_views::Reference::Missile,
                    _ => return Err("--flight-reference needs player, target or missile".into()),
                };
            }
            "--flight-view" => {
                flight_view = args
                    .next()
                    .ok_or("--flight-view needs 0..11")?
                    .parse::<u8>()?;
                if flight_view > flight_views::MISSILE {
                    return Err("--flight-view needs 0..11".into());
                }
            }
            "--flight-map" => { flight_map = true; initial_screen = Screen::Flight; }
            "--weapon-diagnostics" => weapon_diagnostics = true,
            "--ejection-preview" => {
                let phase = args.next().ok_or("--ejection-preview needs seat, freefall or chute")?;
                if !matches!(phase.as_str(), "seat" | "freefall" | "chute") { return Err("--ejection-preview needs seat, freefall or chute".into()); }
                ejection_preview = Some(phase);
                initial_screen = Screen::Flight;
            }
            "--capture-flight" => {
                capture_terrain = Some(PathBuf::from(
                    args.next().ok_or("--capture-flight needs a PPM path")?,
                ));
                initial_screen = Screen::Flight;
                smoke_test = true;
            }
            "--controls-menu" => {
                controls_menu = true;
                flight_menu = true;
                initial_screen = Screen::Flight;
            }
            "--flight-menu" => {
                flight_menu = true;
                initial_screen = Screen::Flight;
            }
            "--free-flight" => initial_screen = Screen::Flight,
            "--flight-probe-ticks" => {
                let ticks = args
                    .next()
                    .ok_or("--flight-probe-ticks needs a tick count")?
                    .parse::<usize>()?;
                if ticks > 120 * 60 {
                    return Err("rendered flight probe limited to one minute".into());
                }
                flight_probe_ticks = Some(ticks);
            }
            "--flight-bay" => {
                let value: f64 = args.next().ok_or("missing bay fraction")?.parse()?;
                if !value.is_finite() || !(0. ..=1.).contains(&value) {
                    return Err("--flight-bay requires 0..1".into());
                }
                flight_bay = Some(value);
            }
            "--flight-throttle" => {
                let value: f64 = args.next().ok_or("missing flight throttle")?.parse()?;
                if !value.is_finite() || !(0. ..=1.).contains(&value) { return Err("--flight-throttle requires 0..1".into()); }
                flight_throttle = Some(value);
            }
            "--flight-devices" | "--flight-controls" => {
                let raw = args
                    .next()
                    .ok_or("animation preview needs comma-separated values")?;
                let values = raw
                    .split(',')
                    .map(str::parse::<f64>)
                    .collect::<Result<Vec<_>, _>>()?;
                let devices = arg == "--flight-devices";
                if values.len() != if devices { 5 } else { 3 }
                    || values
                        .iter()
                        .any(|v| !v.is_finite() || *v > 1. || *v < if devices { 0. } else { -1. })
                {
                    return Err("--flight-devices requires G,F,B,H,AB in 0..1; --flight-controls requires pitch,roll,rudder in -1..1".into());
                }
                if devices {
                    flight_devices = Some(values);
                } else {
                    flight_controls = Some(values);
                }
            }
            "--maneuver" => {
                maneuver = args.next().ok_or(
                    "--maneuver needs level, takeoff, pull, loop, roll, stall, spin, bank-left or bank-right",
                )?;
                if ![
                    "level",
                    "takeoff",
                    "pull",
                    "loop",
                    "roll",
                    "stall",
                    "spin",
                    "bank-left",
                    "bank-right",
                ]
                .contains(&maneuver.as_str())
                {
                    return Err("unsupported maneuver".into());
                }
            }
            "--native-flight-report" => native_flight_report = true,
            "--native-flight-trig" => {
                native_flight_trig = Some(
                    args.next()
                        .ok_or("--native-flight-trig needs extracted sine-q15.bin path")?,
                );
                native_flight_report = true;
            }
            "--headless-flight" => {
                headless_ticks = Some(
                    args.next()
                        .ok_or("--headless-flight needs tick count")?
                        .parse::<usize>()?,
                )
            }
            "--systems-preview" => {
                systems_preview = args.next().ok_or("--systems-preview needs comma-separated damage indices 1..35")?
                    .split(',').map(str::parse).collect::<Result<Vec<usize>, _>>()?;
                if systems_preview.iter().any(|i| !(1..=35).contains(i)) { return Err("--systems-preview indices must be 1..35".into()); }
            }
            "--panel-snapshot" => {
                panel_snapshot = Some(args.next().ok_or("--panel-snapshot needs output path")?)
            }
            "--viewer" => initial_screen = Screen::Viewer,
            "--quick-mission" => initial_screen = Screen::Quick,
            "--background" => {
                background = Some(args.next().ok_or("--background needs an asset name")?)
            }
            "--snapshot-state" => {
                snapshot_state = args.next().ok_or("--snapshot-state needs a state name")?
            }
            "--import" => {
                import = Some(PathBuf::from(
                    args.next()
                        .ok_or("--import needs an installed folder or a disc folder")?,
                ))
            }
            "--snapshot" => {
                snapshot = Some(PathBuf::from(
                    args.next().ok_or("--snapshot needs a .ppm output path")?,
                ))
            }
            "--smoke-test" => smoke_test = true,
            "--windowed" => windowed_flag = true,
            flag @ ("--anti-aliasing" | "--render-scale" | "--spotting-aid"
            | "--terrain-filtering") => {
                let value = args.next().ok_or(format!("{flag} needs a value"))?;
                graphics_flags.push((flag.to_owned(), value));
            }
            "--original-graphics" => graphics_flags.push((arg.clone(), String::new())),
            "--no-audio" => no_audio = true,
            "--version" | "-V" => {
                println!("T.O.R.E-Fighters v{}", version::version());
                return Ok(Outcome::Done);
            }
            "--import-only" => import_only = true,
            "--validate-creator" => validate_creator = true,
            "--sensor-summary" => sensor_summary = true,
            "--sensor-channel" => {
                sensor_channel = Some(
                    match args
                        .next()
                        .ok_or("--sensor-channel needs radar or ir")?
                        .as_str()
                    {
                        "radar" => 0,
                        "ir" => 1,
                        _ => return Err("--sensor-channel needs radar or ir".into()),
                    },
                );
            }
            "--scope-range" => {
                let nmi: f64 = args
                    .next()
                    .ok_or("--scope-range needs a recovered scope setting in nautical miles")?
                    .parse()?;
                scope_range = Some(
                    tore_sim::sensors::RANGE_LADDER_NMI
                        .iter()
                        .position(|v| *v == nmi)
                        .ok_or("--scope-range needs 5, 10, 25, 50, 100 or 150")?,
                );
            }
            "--scope-history" => scope_history = true,
            "--validate-weather" => validate_weather = true,
            "--validate-maps" => validate_maps = true,
            "--weather-condition" => {
                let value: usize = args
                    .next()
                    .ok_or("--weather-condition needs 0..5")?
                    .parse()?;
                if value >= tore_sim::environment::CONDITIONS.len() {
                    return Err("--weather-condition needs one of the six source choices".into());
                }
                weather_condition = Some(value);
            }
            "--help" | "-h" => {
                println!(
                    "Visuals: --ejection-preview seat|freefall|chute inspects imported escape poses with --capture-flight. --hud-target-preview bearing,elevation,feet inspects selected-target cues with --capture-flight. --damage-preview 0..1 with --capture-flight inspects original damage bodies and two seconds of smoke. --countermeasure-preview TICKS advances flight and combat after the setup commands, so --combat-command chaff/flare captures show the devices developing.\nCreator: --dummy-aircraft ID,COUNT adds straight-flight fixtures one mile ahead (repeat for mixed aircraft). --quick-mission opens setup; --snapshot-state ordnance opens the loadout preview; --validate-creator checks all imported loadouts and restart without a display.\nCombat: --live-fire starts an explicit PT-default range. Space fires; [ and ] cycle NAV/weapons; T designates; backslash resets target. --weapon-slot N selects a 1-based weapon slot. --combat-command NAME applies a manual setup command before the probe. Shift-K jettisons the selected external group; ; or L clears designation; Insert/Delete release chaff/flare; Use --combat-command class/fail for damage-class and station-fault fixtures. D reports ownship damage and systems in the sim log; Ctrl-Shift-I launches one incoming selected weapon; Shift-Y toggles target ECM; J toggles own ECM (--jammer-on starts powered). Select is the gamepad combat modifier; see INPUT.md. --record-combat NEW_PATH writes version-6 combat-service inputs, including the sensor controls; --replay-combat PATH replays them headlessly with matching --aircraft/--theater and assets. --combat-smoke runs all default slots and five damage classes; TORE_COMBAT_EVIDENCE=DIR also roundtrips per-slot tapes. --combat-probe-ticks 1..7200 advances a scripted firing pass before --capture-flight.\nAI wings: Quick Mission uses AI by default, with separate friendly and enemy delta formations. --ai-wings opens the creator; --fixture-wings retains the old straight-flight setup. --ai-mission free|cap|intercept|escort|self-defense|hold selects the next Quick Mission policy; free is the default. --enemy-skill novice|average forces every enemy aircraft to that level for this session only (the original's persistence of this preference is untraced). --ai-probe-ticks 1..216000 runs a headless AI mission and prints a deterministic per-actor summary; with --ground-start it also prints phase transitions and ground hazards. --maneuver takeoff flies the player off the ground start and cruises on the autopilot; --probe-wing-size 1..5 sizes the player's wing; --probe-wing-only removes all other wings for isolated probes or creator captures; --probe-wing-order TICK:bug-out|land-selected|attack-on-contact|engage-my-target orders all wingmen; --probe-player-home FROM:UNTIL flies the player gear down over the departure field; --probe-attack TICK[:SECONDS] has the scripted leader designate the nearest hostile aircraft, select a weapon and fire from that tick, attacking again SECONDS after each shot. --separation 1|2|5|10|20|50|100|150|200|300 sets the Quick Mission enemy distance in nautical miles.\nMissiles: click CUED/BORESIGHT or bind weapon-seeker-mode. --missile-acceptance runs controlled reach probes. --compatibility-weapons retains prior weapon rules independently of the flight model.\nSensors: one shared radar/infrared component serves every imported aircraft. M cycles the available channels, I selects infrared, R returns to radar, Y toggles contact history, comma/period change the scope setting and a click designates a contact. --sensor-summary prints each aircraft's imported capability; --sensor-channel radar|ir, --scope-range 5|10|25|50|100|150 and --scope-history set the scope for a headless capture. Guidance/contact/damage coupling is a development approximation, not native parity."
                );
                println!(
                    "Controllers: --no-controllers, --record-input NEW_PATH, --replay-input PATH, --list-inputs, --monitor-inputs SECONDS, --write-input-profile NEW_PATH, --input-profile PATH, --test-rumble DEVICE_ID|only, --controls-menu. See docs/INPUT.md.\nInstrument focus: Ctrl-Tab / Ctrl-Shift-Tab, Ctrl-1..6; Ctrl-Shift-1..4 operates selected instrument buttons."
                );
                println!(
                    "Mission recordings: every flight records what happened into replays/ in the data folder; Ctrl+B marks a moment (TORE_RECORD_MISSIONS=0 turns recording off for a run). These are not the --record-input/--replay-input or --record-combat/--replay-combat tapes, which store inputs and simulate them again. --recording-info FILE describes a recording. --recording-log FILE [--out DIR] [--from SECONDS] [--to SECONDS] [--ids 0,7] [--rate HZ] writes log.jsonl and summary.txt. --recording-acmi FILE [--out FILE] [--rate HZ] [--guns] writes a Tacview .txt.acmi file. --recording-diff A B compares two recordings. --ai-probe-ticks N --record-mission NEW_PATH records a headless probe without changing its output; --verify-render then checks every recorded tick redraws the picture the probe drew. See docs/REPLAYS.md."
                );
                println!(
                    "Usage: tore-app [--free-flight | --viewer | --quick-mission] [--theater CODE] [--capture-terrain OUTPUT.ppm] [--import MEDIA_DIR] [--import-only] [--no-audio] [--smoke-test] [--snapshot OUTPUT.ppm] [--snapshot-state STATE] [--background NAME]\n\nImports original menus, all theaters, F/A-18D, Rafale C, F-14D, A-4E, X-31 EFM, MiG-29, Su-27, MiG-21, Su-25, MiG-23, Su-35, F-22A and F-22N assets into platform application data.\n--import MEDIA_DIR takes an installed Fighters Anthology folder, or the folder of a mounted disc 1 holding SETUP.ESA (the container path itself is also accepted). A raw .iso is not read: mount it and choose the mounted folder.\nOn first run without --import the remembered source is used, otherwise a local gameassets/fighters-anthology directory.\n--aircraft f18|rafale|f14|a4e|x31|mig29|su27|mig21|su25|mig23|su35|f22|f22n|faxx selects the aircraft (default f18).\n--free-flight launches the selected aircraft; --headless-flight TICKS runs without a display.\n--launch-quick-mission launches the creator setup directly.\n--ground-start AIRPORT_NUMBER selects a runway start, or presets Ground in --quick-mission. The researched flight model is required.\nUse --ground-start N --headless-flight TICKS --maneuver takeoff for a deterministic rollout probe.\nFlight: Shift-arrows look/orbit, keypad 5 or Shift-/ recenter. Arrows pitch/bank, End/PageDown or Z/X rudder, 1-5 throttle idle to 100%, 6 afterburner, 7/8 throttle -/+5%, Insert/Delete chaff/flare, Shift-E twice to eject. F1 front, F2 back, F3 up, F4 track, F5 threat, F6 wing, F7 player-target, F8 target-player, F9 fly-by, F10 external, F12 missile-target. Alt/Ctrl+view references target/last missile (Alt-F4 exits). V saves Other View. Shift-0..9 instruments. Esc > Pref > Large windows? switches four-corner/six-bottom layouts. Esc flight menu, Ctrl-P pause, Backspace cockpit, F11 keyboard help. See docs/FLIGHT-CONTROLS.md.\n--quick-mission opens the creator; --viewer opens the selected theater.\n--theater CODE selects a base theater or imported layout variant, such as ~UKR1 (default UKR). --validate-maps constructs every imported map without a display.
Weather: --weather-condition 0..5 selects one of the six source choices (clear, cloudy, foggy, dawn, sunset, night); --validate-weather checks every imported module, one full simulated day and every choice without a display. TORE_WEATHER_TIME=HH:MM overrides the launch time for matched captures; TORE_VAPOR_PROBE=1 prints the resolved wing vapor trail headlessly.\n--capture-flight PATH captures flight with instruments; --flight-view 0..11 chooses front/external/oblique/back/up/track/threat/wing/player-target/target-player/fly-by/missile-target. --flight-reference player/target/missile selects the reference. --flight-menu captures the paused menu. --flight-map opens the Shift-M map. --weapon-diagnostics shows the upper-right weapon diagnostic panel (Escape > Pref > Weapon diagnostics? in flight). --flight-look YAW,PITCH sets look angles in degrees for inspection. --flight-zoom 0.5..4 sets initial zoom.\n--flight-throttle 0..1 sets initial throttle for material inspection. --flight-bay 0..1 sets an F-22 main-bay pose. O toggles bays in flight.\n--flight-devices G,F,B,H,AB sets initial fractions (0..1); --flight-controls pitch,roll,rudder sets initial deflections (-1..1). Animation captures pause at the specified pose.\n--instrument-layout large/small selects four corners or six bottom windows.\n--panel-snapshot PATH writes one instrument; --systems-preview 12,13,14 injects panel-only faults and advances --flight-probe-ticks (default 1200); --instrument-page 0..9 selects it.\n--native-flight-tables DIR enables airborne native research using extracted sine/atan tables; environmental turbulence and native contact/lifecycle producers are unavailable.\n--researched-flight explicitly selects the default hybrid flight/contact model (not native parity). --legacy-flight selects the previous compatibility model.\n--native-flight-report prints static-translated helper probes (not a native simulation). --native-flight-trig PATH additionally probes an extracted sine-q15.bin table.\n--headless-flight TICKS supports --maneuver level/pull/loop/roll/stall/spin/bank-left/bank-right. --flight-probe-ticks TICKS advances that maneuver before a rendered flight (maximum 7200 ticks).\n--capture-terrain writes a GPU-rendered 960x720 terrain PPM and exits (display required).\nGraphics for one run: --anti-aliasing off/2x/4x/8x, --render-scale 75/100/125/150/200, --spotting-aid off/subtle/strong, --terrain-filtering on/off; --original-graphics turns every addition off.\nViewer: arrows move; Shift speeds up; Q/E or PageDown/PageUp change altitude; A/D turn; W/S pitch; Escape returns.\n--snapshot writes a headless 640x480 menu preview and exits (supports --quick-mission).\n--snapshot-state: normal, hover, pressed, help, pref, multi, notice, controls, controls-keyboard, controls-mouse, controls-head, graphics, locate, locate-importing, locate-done. Quick mission: normal, aircraft, theaters, help.\n--background: CHOOSEAC, CHOOSE3, CHOOSEU, CHOOSEM, CHOOSEV (default: random; snapshots use CHOOSEV).\n--smoke-test presents one frame without audio and exits.\nThe game starts in borderless fullscreen; --windowed starts in a window, as --window-size, --smoke-test and the captures already do. Alt-Enter switches at any time and the choice is remembered.\nTORE_DATA_DIR overrides the application data directory. TORE_LOG_DIR overrides diagnostic logs; TORE_NO_ERROR_DIALOG=1 suppresses failure dialogs.\n--diagnostics-self-test[=error|panic|worker-panic|graphics|dialog] checks reporting without retail media.\nTab/arrows + Enter navigate; Escape dismisses; M toggles music; ? contains Exit."
                );
                return Ok(Outcome::Done);
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if fixture_wings
        && (ai_wings_enabled || enemy_skill.is_some() || ai_mission != ai_wings::Preset::Free)
    {
        return Err(
            "--fixture-wings cannot combine with AI wings, an AI probe or enemy skill".into(),
        );
    }
    if ai_wings_enabled
        && (live_fire
            || !dummy_aircraft.is_empty()
            || combat_probe.is_some()
            || combat_smoke
            || missile_acceptance
            || native_tables_path.is_some()
            || record_combat.is_some()
            || replay_combat.is_some()
            || record_input.is_some()
            || replay_input.is_some()
            || headless_ticks.is_some())
    {
        return Err(
            "--ai-wings flies a Quick Mission; the range, dummy fixtures, combat/input tapes, native research flight and headless flight have their own paths"
                .into(),
        );
    }
    if ai_probe.is_some() && (capture_terrain.is_some() || snapshot.is_some() || import_only) {
        return Err("--ai-probe-ticks is a headless probe and cannot capture or snapshot".into());
    }
    if probe_script.attack.is_some() && (ai_probe.is_none() || ai_roster_probe) {
        return Err("--probe-attack scripts the leader of an --ai-probe-ticks run".into());
    }
    if ai_wings_enabled && ai_probe.is_none() {
        // The bridge reads the Quick Mission setup screen, so the flag opens it.
        initial_screen = Screen::Quick;
    }
    if !dummy_aircraft.is_empty()
        && (live_fire
            || native_tables_path.is_some()
            || record_input.is_some()
            || replay_input.is_some()
            || replay_combat.is_some()
            || headless_ticks.is_some())
    {
        return Err("dummy aircraft require normal desktop flight; range/research/replay/headless-flight modes have separate fixtures".into());
    }
    if countermeasure_preview.is_some() && capture_terrain.is_none() {
        return Err("--countermeasure-preview requires --capture-flight".into());
    }
    if !systems_preview.is_empty() && panel_snapshot.is_none() {
        return Err("--systems-preview requires --panel-snapshot".into());
    }
    if damage_preview.is_some()
        && (capture_terrain.is_none()
            || native_tables_path.is_some()
            || record_input.is_some()
            || record_combat.is_some()
            || replay_combat.is_some()
            || replay_input.is_some())
    {
        return Err("--damage-preview requires --capture-flight and cannot record/replay or use native research flight".into());
    }
    if hud_target_preview.is_some()
        && (capture_terrain.is_none()
            || native_tables_path.is_some()
            || record_input.is_some()
            || record_combat.is_some()
            || replay_combat.is_some()
            || replay_input.is_some())
    {
        return Err("--hud-target-preview requires --capture-flight without recording/replay or native research flight".into());
    }
    let researched_flight = flight_mode_arg.unwrap_or(native_tables_path.is_none());
    let native_tables = if let Some(path) = native_tables_path {
        if flight_mode_arg.is_some() || live_fire || combat_probe.is_some() || combat_smoke {
            return Err(
                "native research flight cannot combine with explicit hybrid/legacy or combat modes"
                    .into(),
            );
        }
        use std::io::Read;
        let read = |name: &str, limit: u64| -> AppResult<Vec<u8>> {
            let mut bytes = Vec::new();
            std::fs::File::open(path.join(name))?
                .take(limit + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() != limit as usize {
                return Err(format!("{name}: incorrect table length").into());
            }
            Ok(bytes)
        };
        Some(std::sync::Arc::new(tore_sim::native::Tables::parse(
            &read("sine-q15.bin", 642)?,
            &read("atan-pa.bin", 1028)?,
        )?))
    } else {
        None
    };
    if capture_terrain.is_some()
        && (snapshot.is_some()
            || import_only
            || !matches!(initial_screen, Screen::Viewer | Screen::Flight))
    {
        return Err("scene capture requires flight/viewer and cannot combine with --snapshot or --import-only".into());
    }
    if snapshot.is_none() && !smoke_test && snapshot_state != "normal" {
        return Err("--snapshot-state requires --snapshot or --smoke-test".into());
    }
    // The locate screen owns no imported media, so its preview is written
    // before any pack is looked for.
    if let Some(path) = &snapshot
        && snapshot_state.starts_with("locate")
    {
        locate_snapshot(path, &snapshot_state)?;
        return Ok(Outcome::Done);
    }
    // Mission recording tools read a recording file and need no media.
    let recording_commands = [
        recording_info.is_some(),
        recording_log.is_some(),
        recording_acmi.is_some(),
        recording_diff.is_some(),
    ]
    .into_iter()
    .filter(|on| *on)
    .count();
    if recording_commands > 1 {
        return Err("use one --recording-* command at a time".into());
    }
    if recording_out.is_some() && recording_log.is_none() && recording_acmi.is_none() {
        return Err("--out goes with --recording-log or --recording-acmi".into());
    }
    if (recording_from.is_some() || recording_to.is_some() || recording_ids.is_some())
        && recording_log.is_none()
    {
        return Err("--from, --to and --ids go with --recording-log".into());
    }
    if recording_rate.is_some() && recording_log.is_none() && recording_acmi.is_none() {
        return Err("--rate goes with --recording-log or --recording-acmi".into());
    }
    if recording_guns && recording_acmi.is_none() {
        return Err("--guns goes with --recording-acmi".into());
    }
    if record_mission.is_some() && (ai_probe.is_none() || ai_roster_probe) {
        return Err("--record-mission records an --ai-probe-ticks run".into());
    }
    if verify_render && record_mission.is_none() {
        return Err("--verify-render checks a --record-mission run".into());
    }
    if let Some(path) = recording_info {
        replay::cli::info(&path, &mut std::io::stdout().lock())?;
        return Ok(Outcome::Done);
    }
    if let Some(path) = recording_log {
        let folder = replay::cli::log(
            &path,
            &replay::cli::LogOptions {
                out: recording_out,
                from_s: recording_from,
                to_s: recording_to,
                ids: recording_ids,
                rate: recording_rate,
            },
        )?;
        println!("Recording log: {}", folder.join("log.jsonl").display());
        println!(
            "Recording summary: {}",
            folder.join("summary.txt").display()
        );
        return Ok(Outcome::Done);
    }
    if let Some(path) = recording_acmi {
        let written = replay::cli::acmi(&path, recording_out, recording_rate, recording_guns)?;
        println!("Tacview file: {}", written.display());
        return Ok(Outcome::Done);
    }
    if let Some((a, b)) = recording_diff {
        replay::cli::diff(&a, &b, &mut std::io::stdout().lock())?;
        return Ok(Outcome::Done);
    }
    let probe_record = record_mission.map(|path| ProbeRecord {
        path,
        verify: verify_render,
    });
    if let Some(seconds) = input_seconds {
        input::diagnostics(
            seconds,
            write_input_profile.as_deref(),
            test_rumble.as_deref(),
        )?;
        return Ok(Outcome::Done);
    }
    if record_input.is_some()
        && (headless_ticks.is_some()
            || import_only
            || snapshot.is_some()
            || initial_screen != Screen::Flight
            || capture_terrain.is_some()
            || flight_probe_ticks.is_some()
            || flight_devices.is_some()
            || flight_controls.is_some()
            || (flight_throttle.is_some() || flight_bay.is_some()))
    {
        return Err("--record-input requires direct --free-flight without headless/capture/probe/pose overrides".into());
    }
    let replay_frames = if let Some(path) = replay_input {
        let frames =
            tore_input::recording::read(std::io::BufReader::new(std::fs::File::open(path)?))?;
        if frames.is_empty()
            || headless_ticks.is_some()
            || record_input.is_some()
            || maneuver != "level"
        {
            return Err("--replay-input requires a nonempty tape, default maneuver, and no --headless-flight/--record-input".into());
        }
        headless_ticks = Some(frames.len());
        Some(frames)
    } else {
        None
    };
    diagnostics::stage_done();
    diagnostics::stage("data directory and preferences");
    let data = assets::data_directory()?;
    log::info!("Data directory: {}", data.display());
    if ground_start_airport.is_some() {
        let ground_capture_probe = flight_probe_ticks.is_some()
            && capture_terrain.is_some()
            && initial_screen == Screen::Flight
            && matches!(maneuver.as_str(), "level" | "takeoff");
        if !researched_flight || native_tables.is_some() {
            return Err("Ground start requires the researched flight model; choose Airborne for this adapter.".into());
        }
        if airport_probe.is_some()
            || flight_devices.is_some()
            || flight_controls.is_some()
            || damage_preview.is_some()
            || (flight_probe_ticks.is_some() && !ground_capture_probe)
            || flight_throttle.is_some()
            || flight_bay.is_some()
        {
            return Err("ground start cannot combine with other pose overrides".into());
        }
        if initial_screen == Screen::Viewer {
            return Err("ground start is for flight or the Quick Mission creator".into());
        }
        if initial_screen == Screen::Main {
            initial_screen = Screen::Flight;
        }
    }
    // A window is opened only when no headless mode was selected. The locate
    // screen belongs to those runs alone; everything else keeps the terminal
    // behaviour, which package C settled.
    let windowed = !import_only
        && snapshot.is_none()
        && replay_combat.is_none()
        && panel_snapshot.is_none()
        && headless_ticks.is_none()
        && ai_probe.is_none()
        && !sensor_summary
        && !missile_acceptance
        && !combat_smoke
        && !native_flight_report
        && !validate_creator
        && !validate_weather
        && !validate_maps
        && !(airport_probe.is_some() && !(smoke_test && initial_screen == Screen::Flight))
        && std::env::var_os("TORE_ENVIRONMENT_PROBE").is_none();
    // Borderless fullscreen on the monitor the window would have opened on is
    // the default for an interactive start, and the locate shell and the game
    // window share the choice for the rest of the session.
    let preference = saved_fullscreen();
    let mut window = WindowState {
        fullscreen: initial_window_mode(
            windowed_flag,
            window_size_flag,
            smoke_test
                || capture_terrain.is_some()
                || snapshot.is_some()
                || panel_snapshot.is_some(),
            preference,
        )
        .fullscreen(),
        preference,
    };
    let reimport_background = match session {
        Session::Reimport(frame) => Some(frame),
        Session::First => None,
    };
    let reimporting = reimport_background.is_some();
    diagnostics::stage_done();
    diagnostics::stage("asset loading or import");
    let mut assets = if let Some(chosen) = import.filter(|_| !reimporting) {
        // --import accepts an installed folder, a mounted disc folder or the
        // installer container inside one; the kind is decided by content.
        Assets::import_path(&chosen, &data)?
    } else {
        // Pref asked for this screen, so the pack on disk is deliberately ignored.
        let loaded = match reimporting {
            true => Err("Re-import media".into()),
            false => Assets::load(&data),
        };
        match loaded {
            Ok(assets) => assets,
            Err(error) => {
                log::warn!("Imported assets unavailable: {error}");
                diagnostics::stage("media source discovery");
                // A remembered source is tried first, then a developer checkout.
                let known = media_source::remembered(&data)
                    .map(|(path, _)| path)
                    .into_iter()
                    .chain(std::iter::once(PathBuf::from(
                        "gameassets/fighters-anthology",
                    )))
                    .find_map(|path| media_source::MediaSource::detect(&path).ok())
                    .map(|source| source.path);
                let candidates = if windowed {
                    media_source::candidates(Duration::from_secs(2))
                } else {
                    Vec::new()
                };
                diagnostics::stage_done();
                let first = candidates.first().map(|source| source.path.clone());
                let prefill = known.clone().or_else(|| first.clone());
                let step = next_step(false, windowed, known, first);
                diagnostics::stage("media availability and import");
                match step {
                    Step::Fail => {
                        return Err(format!(
                            "{error}\nImport your own Fighters Anthology media with --import <directory>: an installed Fighters Anthology folder, or the folder of a mounted disc 1 holding SETUP.ESA."
                        )
                        .into());
                    }
                    // No window: the terminal path package C left in place.
                    Step::ImportNow(path) if !windowed => Assets::import_path(&path, &data)?,
                    Step::Play => Assets::load(&data)?,
                    step => {
                        // Re-import is a deliberate choice, so that screen waits
                        // for the player even when a source is already known.
                        let auto = if reimporting && !smoke_test {
                            None
                        } else {
                            auto_import_path(&step, prefill.as_deref(), smoke_test)
                        };
                        let outcome = locate_shell(
                            shared_event_loop(event_loop)?,
                            &data,
                            prefill,
                            candidates,
                            reimport_background,
                            AutoImport {
                                path: auto,
                                continue_when_done: smoke_test,
                            },
                            &mut window,
                        )?;
                        if outcome == ShellOutcome::Quit {
                            return Ok(Outcome::Done);
                        }
                        diagnostics::stage("loading newly imported assets");
                        Assets::load(&data)?
                    }
                }
            }
        }
    };
    diagnostics::stage_done();
    if import_only {
        return Ok(Outcome::Done);
    }
    if validate_maps {
        let catalog = tore_formats::theater::map_catalog(&assets.theater_resources)?;
        for (code, _) in &catalog {
            let world = terrain::World::for_theater(&assets.theater_resources, code)?;
            let bytes = world.texture_indices.len() + world.sky_indices.len();
            if bytes / 65536 > 4096 {
                return Err("map artwork exceeds GPU page budget".into());
            }
            println!(
                "map {code}: source={} grid={}x{} textures={} placements={} bodies={} terrain_vertices={} scenery_vertices={} artwork_bytes={}",
                world.environment.map,
                world.theater.cols,
                world.theater.rows,
                world.environment.textures.len(),
                world.static_manifest.len(),
                world.airport_scene.objects.len(),
                world.vertices.len() / 10,
                world
                    .static_vertices
                    .values()
                    .map(|v| v.len() / 10)
                    .sum::<usize>(),
                bytes
            );
        }
        println!("Validated {} retail map layouts", catalog.len());
        return Ok(Outcome::Done);
    }
    diagnostics::stage("aircraft loading");
    let hornet = aircraft::Airframe::load(&assets.theater_resources, aircraft_id)?;
    diagnostics::stage_done();
    if let Some(path) = replay_combat {
        if record_combat.is_some() {
            return Err("combat record and replay are mutually exclusive".into());
        }
        let c = combat::Combat::new(&hornet, &assets.theater_resources, true)?;
        let w = terrain::World::for_theater(&assets.theater_resources, &theater_code)?;
        combat_tape::replay(
            &path,
            &assets.theater_resources,
            c.state.configuration().clone(),
            &theater_code,
            &w,
        )?;
        return Ok(Outcome::Done);
    }
    if sensor_summary {
        // The reviewable per-aircraft capability report. Porting an aircraft
        // means reviewing this output, not writing another radar controller.
        for id in tore_formats::aircraft::AircraftId::ALL {
            match aircraft::Airframe::load(&assets.theater_resources, id) {
                Ok(airframe) => println!("{}", airframe.sensors.summary()),
                Err(error) => println!("{id:?}: unavailable, {error}"),
            }
        }
        return Ok(Outcome::Done);
    }
    if missile_acceptance {
        let config = tore_sim::combat::live::Configuration::from_source(&hornet.profile, |name| {
            assets
                .theater_resources
                .get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other("missing probe resource"))
        })?;
        missile_acceptance::run(config)?;
        return Ok(Outcome::Done);
    }
    if combat_smoke {
        combat::smoke(&hornet, &assets.theater_resources)?;
        return Ok(Outcome::Done);
    }
    if live_fire && record_input.is_some() {
        return Err("combat recording is not in the flight-only tape format".into());
    }
    if native_flight_report {
        flight::native_report(&hornet.profile)?;
        if let Some(path) = native_flight_trig {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(643)
                .read_to_end(&mut bytes)?;
            let table = tore_formats::flight_model::rotation::TrigTable::parse(&bytes)?;
            flight::native_rotation_report(&hornet.profile, &table)?;
        }
        return Ok(Outcome::Done);
    }
    let ground_object = |world: &terrain::World| -> AppResult<Option<u32>> {
        ground_start_airport
            .map(|id| {
                world
                    .airport_scene
                    .airports
                    .iter()
                    .find(|a| a.id == id)
                    .and_then(|a| a.runway_objects.first())
                    .copied()
                    .ok_or_else(|| "Selected ground-start airport is unavailable".into())
            })
            .transpose()
    };
    let setup_maneuver = |state: &mut flight::State| {
        let mut keys = flight::PilotInput::default();
        match maneuver.as_str() {
            "pull" | "loop" => {
                keys.pitch = 1.;
                if maneuver == "loop" {
                    state.throttle = 1.;
                    state.burner = true;
                }
            }
            "bank-left" | "bank-right" => {
                state.bank = if maneuver == "bank-left" {
                    -45f64
                } else {
                    45f64
                }
                .to_radians();
                state.throttle = 1.;
                state.burner = true;
                keys.pitch = 1.;
            }
            "takeoff" => {
                state.brake_out = false;
                state.throttle = 1.;
                state.burner = state
                    .model()
                    .configuration()
                    .propulsion
                    .afterburner_thrust_lbf
                    > 0.;
                keys.pitch = 0.35;
            }
            "roll" => {
                keys.roll = 1.;
            }
            "spin" => {
                state.speed = 180.;
                state.engine = false;
                state.velocity = attitude::Basis::new(state.yaw, state.pitch, state.bank)
                    .forward
                    .map(|v| v * state.speed);
                keys.pitch = 1.;
                keys.yaw = 1.;
            }
            "stall" => {
                state.engine = false;
                state.pitch = 0.2;
                state.velocity = attitude::Basis::new(state.yaw, state.pitch, state.bank)
                    .forward
                    .map(|v| v * state.speed);
            }
            _ => {}
        }
        keys
    };
    if let Some(ticks) = headless_ticks {
        if ticks > 120 * 3600 {
            return Err("headless flight limited to one hour".into());
        }
        let replay_world = if replay_frames.is_some() || ground_start_airport.is_some() {
            Some(terrain::World::for_theater(
                &assets.theater_resources,
                &theater_code,
            )?)
        } else {
            None
        };
        let mut state = if let Some(world) = &replay_world {
            hornet.start(world)
        } else {
            flight::State::new(&hornet.profile, [0., 5000., 0.])?
        };
        if researched_flight {
            state.enable_research(1)?;
        }
        println!(
            "flight_model={}",
            if native_tables.is_some() {
                "native-research-airborne"
            } else if state.research.is_some() {
                "hybrid"
            } else {
                "legacy"
            }
        );
        if let Some(world) = &replay_world
            && let Some(object) = ground_object(world)?
        {
            if replay_frames.is_none() {
                let load = combat::Combat::new(&hornet, &assets.theater_resources, false)?;
                state.set_payload(load.state.payload_lbs())?;
                state.systems = tore_sim::aircraft_systems::Systems::new(
                    load.state.configuration().engines,
                    load.state.external_fuel_lbs(),
                );
            }
            quick_mission::apply_ground_start(world, &mut state, object)?;
            println!(
                "ground_start={object} position={:?} heading={:.3} gear={} brakes={}",
                state.position,
                state.yaw.to_degrees(),
                state.gear,
                state.brake_out
            );
        }
        let keys = setup_maneuver(&mut state);
        if let Some(tables) = &native_tables {
            state.enable_native(tables.clone(), 1)?;
        }
        let initial_forward = attitude::Basis::new(state.yaw, state.pitch, state.bank).forward;
        let (mut vertical, mut inverted, mut completed) = (false, false, false);
        for tick in 0..ticks {
            let keys = replay_frames.as_ref().map_or(&keys, |frames| &frames[tick]);
            if ground_start_airport.is_some() {
                let world = replay_world.as_ref().unwrap();
                state.step_surface(keys, |x, z| world.surface(x, z));
            } else {
                state.step(keys, |x, z| {
                    replay_world
                        .as_ref()
                        .map_or(0., |world| f64::from(world.height(x as f32, z as f32)))
                });
            }
            if let Some(error) = state.native_fault() {
                return Err(error.into());
            }
            let basis = attitude::Basis::new(state.yaw, state.pitch, state.bank);
            vertical |= basis.forward[1] > 0.999;
            inverted |= basis.up[1] < -0.9;
            completed |= inverted
                && basis.up[1] > 0.9
                && attitude::dot(basis.forward, initial_forward) > 0.98;
            if maneuver == "takeoff" && ground_start_airport.is_some() {
                let world = replay_world.as_ref().unwrap();
                let object = ground_object(world)?.unwrap();
                let height = world.airport_scene.runway(object).unwrap().elevation_ft;
                if !state.crashed && state.position[1] > height + 100. {
                    println!("takeoff_complete=true airport_ground_ft={height}");
                    break;
                }
            }
            if maneuver == "loop" && completed {
                break;
            }
        }
        let body = attitude::Basis::new(state.yaw, state.pitch, state.bank);
        let forward = attitude::dot(state.velocity, body.forward);
        let side = attitude::dot(state.velocity, body.right);
        let up = attitude::dot(state.velocity, body.up);
        println!(
            "aoa_deg={:.4} sideslip_deg={:.4} bank_deg={:.4}",
            (-up).atan2(forward).to_degrees(),
            side.atan2(forward.hypot(up)).to_degrees(),
            state.bank.to_degrees()
        );
        println!("vertical={vertical} inverted={inverted} loop_completed={completed}");
        println!(
            "departure_alert={:?} spin_direction={}",
            state.stall_alert(0.),
            state.research.as_ref().map_or(0, |r| r.spinning)
        );

        println!(
            "ticks={} speed_kt={:.3} altitude_ft={:.3} fuel_lb={:.3} crashed={}",
            state.ticks,
            state.speed / 1.68781,
            state.position[1],
            state.fuel,
            state.crashed
        );
        if let Some(pilot) = &state.escape {
            println!(
                "ejection={:?} pilot_alive={} pilot_position={:?}",
                pilot.phase, !state.systems.pilot.dead, pilot.position
            );
        }
        return Ok(Outcome::Done);
    }
    if let Some(path) = panel_snapshot {
        use std::io::Write;
        let mut state = flight::State::new(&hornet.profile, [0., 5000., 0.])?;
        let mut combat = combat::Combat::new(&hornet, &assets.theater_resources, false)?;
        combat.reset(&mut state)?;
        if let Some(throttle) = flight_throttle {
            state.throttle = throttle;
        }
        for index in &systems_preview {
            state.systems.hit(*index, state.throttle);
        }
        if !systems_preview.is_empty() {
            for _ in 0..flight_probe_ticks.unwrap_or(1200) {
                state
                    .systems
                    .advance(true, state.throttle, 1., 0., false, &mut state.fuel);
                state.ticks += 1;
            }
            println!("{}", state.systems.summary(0.));
        }
        let mut panels = instruments::Instruments::default();
        panels.palette = hornet.daylight_palette();
        let r = panels.page(instrument_page.unwrap_or(7), &hornet, &state);
        let mut f = std::fs::File::create(path)?;
        write!(
            f,
            "P6\n{} {}\n255\n",
            instruments::WIDTH,
            instruments::HEIGHT
        )?;
        for p in r.pixels.chunks_exact(4) {
            f.write_all(&p[..3])?;
        }
        return Ok(Outcome::Done);
    }
    diagnostics::stage("audio initialization");
    let audio = if no_audio
        || smoke_test
        || validate_creator
        || validate_weather
        || snapshot.is_some()
        || std::env::var_os("TORE_ENVIRONMENT_PROBE").is_some()
    {
        None
    } else {
        match audio::Audio::new(
            std::mem::take(&mut assets.sounds),
            &assets.music_scores,
            &assets.theater_resources,
        ) {
            Ok(audio) => Some(audio),
            Err(error) => {
                log::warn!("Continuing without audio: {error}");
                None
            }
        }
    };
    diagnostics::stage_done();
    // Saved previews stay reproducible; normal launches randomly select all five.
    if snapshot.is_some() && background.is_none() {
        background = Some("CHOOSEV".into());
    }
    diagnostics::stage("terrain construction");
    let mut world =
        terrain::World::for_mission(&assets.theater_resources, &theater_code, weather_condition)?;
    diagnostics::stage_done();
    let ground_start = ground_object(&world)?;
    if std::env::var_os("TORE_AIRPORT_PROBE").is_some() {
        println!(
            "airport scene: theater={} airports={} runways={} objects={} targets={}",
            theater_code,
            world.airport_scene.airports.len(),
            world.airport_scene.runways.len(),
            world.airport_scene.objects.len(),
            world.airport_scene.objects.len()
        );
    }
    if validate_creator {
        ordnance::validate_sources(&assets.theater_resources, &world)?;
        return Ok(Outcome::Done);
    }
    if validate_weather {
        weather::validate_sources(&assets.theater_resources, &world.environment)?;
        return Ok(Outcome::Done);
    }
    let theater_resources = assets.theater_resources.clone();
    let creator_options = assets.creator_options.clone();
    if let Some((airport_id, mut aircraft, angles)) = airport_probe
        && !(smoke_test && initial_screen == Screen::Flight)
    {
        let mut combat = combat::Combat::new(&hornet, &theater_resources, false)?;
        combat.add_airport_targets(&world.airport_scene)?;
        let mut flight = hornet.start(&world);
        flight.position = aircraft.position;
        flight.gear_down = aircraft.gear_down;
        flight.gear = f64::from(aircraft.gear_down);
        if let Some([heading, pitch]) = angles {
            flight.yaw = heading.to_radians();
            flight.pitch = pitch.to_radians();
            flight.bank = 0.;
        }
        aircraft.forward = attitude::Basis::new(flight.yaw, flight.pitch, flight.bank).forward;
        combat.reset(&mut flight)?;
        let live_targets = combat
            .state
            .targets
            .iter()
            .filter(|target| target.role == tore_sim::combat::missiles::TargetRole::Surface)
            .count();
        let visible_vertices = world.visible_static_vertices(&combat.state.targets).len() / 10;
        let mut service =
            tore_sim::airport::Service::new(&world.airport_scene).map_err(std::io::Error::other)?;
        service.command(
            &world.airport_scene,
            aircraft,
            tore_sim::airport::Command::SelectAirport(airport_id),
        );
        let reply = service.command(
            &world.airport_scene,
            aircraft,
            tore_sim::airport::Command::RequestLanding,
        );
        let guidance = service.guidance(&world.airport_scene, aircraft);
        println!(
            "airport probe: theater={theater_code} scene_objects={} live_targets={live_targets} visible_vertices={visible_vertices} forward={:?} heading_deg={:.3} pitch_deg={:.3} selected={:?} clearance={:?} reply={reply:?} guidance={guidance:?}",
            world.airport_scene.objects.len(),
            aircraft.forward,
            flight.yaw.to_degrees(),
            flight.pitch.to_degrees(),
            service.selected(),
            service.clearance(),
        );
        return Ok(Outcome::Done);
    }
    diagnostics::stage("menu construction");
    let mut menu = Menu::new(assets, background.as_deref())?;
    diagnostics::stage_done();
    diagnostics::stage("menu and flight state setup");
    if let Some(path) = snapshot {
        if matches!(initial_screen, Screen::Viewer | Screen::Flight) {
            return Err(
                "Use --capture-terrain for a GPU terrain capture; CPU snapshots support menus only"
                    .into(),
            );
        }
        if initial_screen == Screen::Quick {
            let mut quick = quick_mission::QuickMission::new(
                aircraft_id,
                creator_options.clone(),
                &theater_resources,
            );
            quick.ai_mission = ai_mission;
            let selection = world
                .catalog
                .iter()
                .position(|(code, _)| code == &theater_code)
                .unwrap_or(0);
            quick.theater(selection);
            if let Some(object) = ground_start {
                quick.choose_ground_runway(object)?;
            }
            if matches!(
                snapshot_state.as_str(),
                "ordnance"
                    | "ordnance-empty"
                    | "ordnance-drag"
                    | "ordnance-message"
                    | "ordnance-message-long"
            ) {
                quick.ordnance = Some(ordnance::Ordnance::new(
                    tore_sim::combat::loadout::Loadout::new(&hornet.profile, |n| {
                        theater_resources
                            .get(n)
                            .cloned()
                            .ok_or_else(|| std::io::Error::other("missing loadout resource"))
                    })?,
                    &theater_resources,
                )?);
            }
            if let Some(page) = snapshot_state.strip_prefix("debrief") {
                let mut report = debrief::Report::sample();
                if page == "-success" {
                    report.outcome = debrief::Outcome::Success;
                }
                let mut debrief =
                    debrief::Debrief::new(report, &theater_resources, Some("DEBSCV.PIC"))?;
                debrief.page = page
                    .trim_start_matches('-')
                    .parse::<usize>()
                    .map_or(0, |page| page.clamp(1, 5) - 1);
                quick.debrief = Some(debrief);
            }
            quick.render(&mut menu.pixels, &menu.quick_sprites, &world);
            quick.preview_selector(&snapshot_state)?;
            quick.render(&mut menu.pixels, &menu.quick_sprites, &world);
            use std::io::Write;
            let mut f = std::fs::File::create(&path)?;
            write!(f, "P6\n640 480\n255\n")?;
            for p in menu.pixels.chunks_exact(4) {
                f.write_all(&p[..3])?;
            }
        } else if snapshot_state == "graphics" {
            // No GPU here: a fixed support list stands in for the adapter,
            // with 8x unavailable so the struck-through style is visible.
            let editor = graphics_screen::Editor::new(
                graphics::Options::default(),
                [true, true, true, false],
                "Main menu",
            );
            menu.preview_state("normal")?;
            menu.render();
            editor.draw(&mut menu.pixels, &hornet.font);
            use std::io::Write;
            let mut f = std::fs::File::create(&path)?;
            write!(f, "P6\n640 480\n255\n")?;
            for p in menu.pixels.chunks_exact(4) {
                f.write_all(&p[..3])?;
            }
        } else if let Some(tab) = snapshot_state.strip_prefix("controls") {
            // A synthetic Xbox-layout pad with its defaults, so the screen
            // can be inspected without hardware.
            let pad = controls_editor::preview_device();
            let defaults = input::gamepad_defaults(&pad);
            let profile = tore_input::Profile {
                bindings: defaults.bindings,
                modifiers: defaults.modifiers,
                gamepad_defaults: true,
                ..Default::default()
            };
            let mut editor = controls_editor::Editor::new(profile, vec![pad], "Main menu");
            editor.head_status = "Waiting for opentrack on UDP 4242".into();
            editor.preview(tab.trim_start_matches('-'))?;
            menu.preview_state("normal")?;
            menu.render();
            editor.draw(&mut menu.pixels, &hornet.font);
            use std::io::Write;
            let mut f = std::fs::File::create(&path)?;
            write!(f, "P6\n640 480\n255\n")?;
            for p in menu.pixels.chunks_exact(4) {
                f.write_all(&p[..3])?;
            }
        } else {
            menu.preview_state(&snapshot_state)?;
            menu.save_ppm(&path)?;
        }

        println!("Menu preview: {}", path.display());
        return Ok(Outcome::Done);
    }
    let turbulence_enabled = match std::env::var("TORE_TURBULENCE").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("1") => true,
        Ok("0") => false,
        _ => return Err("TORE_TURBULENCE needs 0 or 1".into()),
    };
    let mut camera = terrain::Camera::for_world(&world);
    if let Ok(pose) = std::env::var("TORE_WEATHER_VIEW") {
        let values = pose
            .split(',')
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()?;
        if !(5..=6).contains(&values.len())
            || values.iter().any(|v| !v.is_finite())
            || values[..3].iter().any(|v| v.abs() > 2_000_000.)
            || values[3].abs() > 360.
            || values[4].abs() > 90.
            || values.get(5).is_some_and(|roll| roll.abs() > 360.)
        {
            return Err("TORE_WEATHER_VIEW needs x,y,z,yaw,pitch[,roll] in feet/degrees".into());
        }
        camera.position.copy_from_slice(&values[..3]);
        camera.yaw = values[3].to_radians();
        camera.pitch = values[4].to_radians();
        camera.roll = values.get(5).copied().unwrap_or(0.).to_radians();
    }
    let selection = world
        .catalog
        .iter()
        .position(|(code, _)| code == &theater_code)
        .unwrap_or(0);
    let mut quick =
        quick_mission::QuickMission::new(aircraft_id, creator_options.clone(), &theater_resources);
    quick.ai_mission = ai_mission;
    quick.theater(selection);
    if let Some(object) = ground_start {
        quick.choose_ground_runway(object)?;
    }
    if let Some(nm) = separation_nm {
        quick.draft.values[17] = quick_mission::SEPARATION_NM
            .iter()
            .position(|choice| *choice == nm)
            .unwrap_or(quick.draft.values[17]);
    }
    if let Some(size) = probe_script.wing_size {
        quick.draft.values[4] = size;
    }
    if probe_script.wing_only {
        for field in [7, 10, 21, 24, 27] {
            quick.draft.values[field] = 0;
        }
    }
    probe_script.takeoff = maneuver == "takeoff";
    if let Some(ticks) = ai_probe {
        if ai_roster_probe {
            ai_wings::roster_probe(ticks, &theater_resources, &world)?;
            return Ok(Outcome::Done);
        }
        ai_probe_run(
            ticks,
            &mut quick,
            &hornet,
            &theater_resources,
            &world,
            enemy_skill,
            ai_mission,
            &probe_script,
            probe_record.as_ref(),
        )?;
        return Ok(Outcome::Done);
    }
    if initial_screen == Screen::Quick && snapshot_state == "ordnance" {
        quick.ordnance = Some(ordnance::Ordnance::new(
            tore_sim::combat::loadout::Loadout::new(&hornet.profile, |n| {
                theater_resources
                    .get(n)
                    .cloned()
                    .ok_or_else(|| std::io::Error::other("missing loadout resource"))
            })?,
            &theater_resources,
        )?);
    }
    let animation_capture = capture_terrain.is_some()
        && (flight_devices.is_some()
            || flight_controls.is_some()
            || (flight_throttle.is_some() || flight_bay.is_some())
            || flight_probe_ticks.is_some()
            || damage_preview.is_some()
            || countermeasure_preview.is_some());
    let mut flight = hornet.start(&world);
    if let Ok(value) = std::env::var("TORE_FLIGHT_AGL") {
        let agl = value.parse::<f64>()?;
        if !agl.is_finite() || !(10. ..=90000.).contains(&agl) {
            return Err("TORE_FLIGHT_AGL needs 10..90000 feet".into());
        }
        flight.position[1] =
            f64::from(world.height(flight.position[0] as f32, flight.position[2] as f32)) + agl;
    }
    flight.jammer = jammer_on;
    if researched_flight {
        flight.enable_research(1)?;
    }
    if let Some(tables) = &native_tables {
        flight.enable_native(tables.clone(), 1)?;
    }
    if let Some(object) = ground_start {
        let (position, heading) = quick_mission::runway_pose(&world, object)?;
        flight.position = [
            position[0],
            flight.position[1].max(position[1] + 5000.),
            position[2],
        ];
        flight.yaw = heading;
        let basis = attitude::Basis::new(heading, 0., 0.);
        flight.velocity =
            std::array::from_fn(|i| basis.forward[i] * flight.speed + world.wind()[i]);
    }
    // The probe advances weather and vapor with the flight so captures taken
    // after it show the same environment and trail history a live run would.
    let mut probe_vapor =
        tore_sim::vapor::Vapor::seeded(hornet.streamer_points(&flight).unwrap_or([[0.; 3]; 2]));
    let mut probe_turbulence = tore_sim::turbulence::Turbulence::default();
    let mut probe_turbulence_rng = tore_formats::flight_model::clock_rng::NativeRng::seeded(1)?;
    if ground_start.is_none()
        && let Some(ticks) = flight_probe_ticks
    {
        let keys = setup_maneuver(&mut flight);
        if matches!(maneuver.as_str(), "stall" | "spin") {
            for (v, wind) in flight.velocity.iter_mut().zip(world.wind()) {
                *v += wind;
            }
        }
        for _ in 0..ticks {
            flight.step_surface(&keys, |x, z| world.surface(x, z));
            if let Some(error) = flight.native_fault() {
                return Err(error.into());
            }
            let mut weather_view = hornet.camera(&flight, flight_view, Default::default());
            look::apply(
                &mut weather_view,
                flight.position.map(|v| v as f32),
                flight_look.map(f32::to_radians),
                matches!(flight_view, 1 | 2),
            );
            world.step_weather(flight.speed, &weather_view);
            world.step_view_weather(&mirrors::camera(&flight), flight.speed);
            for page in [2, 3] {
                world.step_view_weather(&hornet.panel_camera(&flight, page), flight.speed);
            }
            step_turbulence(
                &mut probe_turbulence,
                &mut probe_turbulence_rng,
                &mut flight,
                &world,
                turbulence_enabled,
            );
            if let Some(points) = hornet.streamer_points(&flight) {
                probe_vapor.step(world.weather.ticks(), points);
            }
        }
    }
    // Bounded wing-vapor probe: prints the resolved trail without a window.
    if std::env::var_os("TORE_VAPOR_PROBE").is_some() {
        if let Some(def) = &hornet.streamer {
            for side in 0..2 {
                let p = def.attachment(side, 0)?;
                let nearest = hornet.poses[0]
                    .faces
                    .iter()
                    .flat_map(|f| &f.positions)
                    .map(|v| {
                        let v = v.map(f64::from);
                        let distance =
                            ((v[0] - p[0]).powi(2) + (v[1] - p[2]).powi(2) + (v[2] - p[1]).powi(2))
                                .sqrt()
                                / 3.;
                        (distance, v)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                println!(
                    "CE side={side} right/up/forward={p:?}; nearest mesh distance_ft/vertex={nearest:?}"
                );
            }
        }
        println!(
            "wing vapor: g={:.2} roll_rate_deg={:.1} position={:?}",
            flight.g,
            flight.roll_rate.to_degrees(),
            flight.position.map(|v| v.round())
        );
        for side in 0..2 {
            let hazing = world
                .weather
                .sample(flight.position[1])
                .is_some_and(|l| l.night_hazing());
            match probe_vapor.trail(side, flight.g, flight.roll_rate.to_degrees(), hazing) {
                Some(trail) => {
                    for (i, p) in trail.iter().enumerate() {
                        println!("  side {side} point {i}: {:?}", p.map(|v| v.round()));
                    }
                }
                None => println!("  side {side}: no trail"),
            }
        }
    }
    if std::env::var_os("TORE_ENVIRONMENT_PROBE").is_some() {
        println!(
            "Environment: wind={:?} world_fps={:?} air_data={:?}",
            world.weather.configuration().wind(),
            world.wind(),
            world.air_data(&flight)
        );
        println!(
            "Turbulence: enabled={turbulence_enabled} state={probe_turbulence:?}; position={:?} attitude={:?}",
            flight.position,
            [flight.yaw, flight.pitch, flight.bank]
        );
        return Ok(Outcome::Done);
    }
    if let Some(v) = flight_devices {
        if v[4] > 0.
            && flight
                .model()
                .configuration()
                .propulsion
                .afterburner_thrust_lbf
                <= 0.
        {
            return Err(
                "the selected aircraft has no afterburner; set the fifth device fraction to 0"
                    .into(),
            );
        }
        if v[3] > 0. && !flight.hook_available() {
            return Err(
                "the selected aircraft has no hook; set the fourth device fraction to 0".into(),
            );
        }
        flight.gear = v[0];
        flight.flaps = v[1];
        flight.brake = v[2];
        flight.hook = v[3];
        flight.exhaust = v[4];
        flight.gear_down = v[0] > 0.;
        flight.flaps_down = v[1] > 0.;
        flight.brake_out = v[2] > 0.;
        flight.hook_down = v[3] > 0.;
        flight.burner = v[4] > 0.;
        if flight.burner {
            flight.throttle = 1.;
        }
    }
    if let Some(value) = flight_throttle {
        flight.throttle = value;
    }
    if let Some(v) = flight_controls {
        flight.elevator = v[0];
        flight.aileron = v[1];
        flight.rudder = v[2];
    }

    // Headless probes and captures have no instrument panel to read, so the
    // command line supplies the same sensor controls a player would set.
    flight.sensors = sensor_controls(sensor_channel, scope_range, scope_history);
    let mut combat = combat::Combat::new(&hornet, &theater_resources, live_fire)?;
    combat.add_airport_targets(&world.airport_scene)?;
    if let Some(ref path) = record_combat {
        combat.recorder = Some(combat_tape::Recorder::new(
            path,
            &theater_resources,
            combat.state.configuration(),
            &theater_code,
        )?);
    }
    combat.clean_recording = record_input.is_some();
    if combat.clean_recording {
        log::warn!(
            "Pilot-only recording keeps the existing clean-aircraft load; use combat recording for weapons."
        );
    }
    combat.mission_dummies(&dummy_aircraft, 5280., &theater_resources)?;
    combat.reset(&mut flight)?;
    let normal_startup_defaults = !live_fire
        && record_input.is_none()
        && replay_frames.is_none()
        && combat_probe.is_none()
        && record_combat.is_none();
    if normal_startup_defaults {
        combat.apply_startup_weapons();
    }
    if let Some(object) = ground_start {
        quick_mission::apply_ground_start(&world, &mut flight, object)?;
        probe_vapor =
            tore_sim::vapor::Vapor::seeded(hornet.streamer_points(&flight).unwrap_or([[0.; 3]; 2]));
        if let Some(ticks) = flight_probe_ticks {
            let keys = setup_maneuver(&mut flight);
            for _ in 0..ticks {
                flight.step_surface(&keys, |x, z| world.surface(x, z));
                if let Some(error) = flight.native_fault() {
                    return Err(error.into());
                }
                let mut weather_view = hornet.camera(&flight, flight_view, Default::default());
                look::apply(
                    &mut weather_view,
                    flight.position.map(|v| v as f32),
                    flight_look.map(f32::to_radians),
                    matches!(flight_view, 1 | 2),
                );
                world.step_weather(flight.speed, &weather_view);
                world.step_view_weather(&mirrors::camera(&flight), flight.speed);
                for page in [2, 3] {
                    world.step_view_weather(&hornet.panel_camera(&flight, page), flight.speed);
                }
                step_turbulence(
                    &mut probe_turbulence,
                    &mut probe_turbulence_rng,
                    &mut flight,
                    &world,
                    turbulence_enabled,
                );
                if let Some(points) = hornet.streamer_points(&flight) {
                    probe_vapor.step(world.weather.ticks(), points);
                }
            }
        }
    }
    if let Some((airport, mut aircraft, angles)) = airport_probe {
        flight.position = aircraft.position;
        flight.gear_down = aircraft.gear_down;
        flight.gear = f64::from(aircraft.gear_down);
        if let Some([heading, pitch]) = angles {
            flight.yaw = heading.to_radians();
            flight.pitch = pitch.to_radians();
            flight.bank = 0.;
        }
        aircraft.forward = attitude::Basis::new(flight.yaw, flight.pitch, flight.bank).forward;
        airport_probe = Some((airport, aircraft, angles));
        println!(
            "airport_probe_pose position={:?} forward={:?} heading_deg={:.3} pitch_deg={:.3}",
            aircraft.position,
            aircraft.forward,
            flight.yaw.to_degrees(),
            flight.pitch.to_degrees()
        );
    }
    if let Some(value) = flight_bay {
        if !flight.bay_available() {
            return Err("selected aircraft has no reviewed weapon bay".into());
        }
        flight.bay = value;
        flight.bay_open = value > 0.;
    }

    if let Some(weapon_slot) = weapon_slot {
        if weapon_slot == 0 || weapon_slot > combat.state.ammo.len() {
            return Err("weapon slot outside this aircraft's PT loadout".into());
        }
        while combat.state.selected != weapon_slot - 1 {
            combat.command(
                tore_sim::combat::live::Command::NextWeapon,
                combat::launcher(&flight),
            );
        }
    }
    // Scripted setup keeps the render history as live flight does: a changed
    // scene is retaken at once and each combat step ends a tick.
    if live_fire {
        combat.state.armed = true;
        combat.command(
            tore_sim::combat::live::Command::ReplaceTarget,
            combat::launcher(&flight),
        );
        combat.refresh_render(&flight, None);
        // A scripted designation needs a current observation first, exactly as
        // a player's click does.
        combat.step(&mut flight, &world)?;
        combat.advance_render(&flight, None);
    }
    for command in combat_commands {
        combat.command(command, combat::launcher(&flight));
    }
    combat.refresh_render(&flight, None);
    // Lets released chaff and flares develop before a capture.
    if let Some(ticks) = countermeasure_preview {
        for _ in 0..ticks {
            flight.step(&tore_input::PilotInput::default(), |x, z| {
                f64::from(world.height(x as f32, z as f32))
            });
            combat.step(&mut flight, &world)?;
            combat.advance_render(&flight, None);
        }
        let devices = &combat.state.devices;
        println!(
            "Countermeasure preview: ticks={ticks} flares={} burning={} puffs={} chaff={}",
            devices.flares.len(),
            devices.flares.iter().filter(|f| f.burning()).count(),
            devices.puffs().count(),
            devices.chaff.len()
        );
        for flare in &devices.flares {
            let p = flare.position;
            println!(
                "  flare at {:.0},{:.0},{:.0}, {:.0} ft above ground; aircraft {:.0},{:.0},{:.0}",
                p[0],
                p[1],
                p[2],
                p[1] - f64::from(world.height(p[0] as f32, p[2] as f32)),
                flight.position[0],
                flight.position[1],
                flight.position[2]
            );
        }
    }
    if let Some(ticks) = combat_probe {
        combat.command(
            tore_sim::combat::live::Command::Designate,
            combat::launcher(&flight),
        );
        // Half a second of tracking before the probe fires, so a radar weapon
        // has the same support a player would wait for.
        for _ in 0..tore_sim::sensors::track::ACQUISITION_STEPS {
            combat.step(&mut flight, &world)?;
            combat.advance_render(&flight, None);
        }
        combat.input.space(true, false, false);
        let mut feedback = tore_input::FeedbackMixer::default();
        let mut cues = std::collections::BTreeMap::<String, usize>::new();
        let mut pulses = 0;
        for _ in 0..ticks {
            flight.step(&flight::PilotInput::default(), |x, z| {
                f64::from(world.height(x as f32, z as f32))
            });
            for event in combat.step(&mut flight, &world)? {
                if let Some(cue) = combat::feedback(&event, combat.state.configuration()) {
                    *cues.entry(format!("{cue:?}")).or_default() += 1;
                    feedback.event(cue);
                }
            }
            combat.advance_render(&flight, None);
            if matches!(
                feedback.tick(),
                Some(tore_input::FeedbackUpdate::Pulse { .. })
            ) {
                pulses += 1;
            }
        }
        println!(
            "Combat probe feedback generation (no hardware playback): {cues:?}, pulses={pulses}; {}",
            combat.status(&flight)
        );
        combat.cancel();
        println!(
            "Combat probe: {} shots={} hits={} kills={} active={} ammo={:?}",
            hornet.profile.name,
            combat.state.shots,
            combat.state.hits,
            combat.state.kills,
            combat.state.projectiles.len(),
            combat.state.ammo
        );
    }
    if let Some([bearing, elevation, range]) = hud_target_preview {
        for _ in 0..120 {
            combat.step(&mut flight, &world)?;
            combat.advance_render(&flight, None);
        }
        let id = combat
            .state
            .targets
            .iter()
            .find(|t| t.role == tore_sim::combat::missiles::TargetRole::Aircraft)
            .ok_or("HUD preview requires range aircraft")?
            .id;
        combat.command(
            tore_sim::combat::live::Command::DesignateTarget(id),
            combat::launcher(&flight),
        );
        if combat.state.designated() != Some(id) {
            return Err("HUD preview target could not be observed before repositioning".into());
        }
        let body = tore_sim::attitude::Basis::new(flight.yaw, flight.pitch, flight.bank);
        let (bearing, elevation) = (bearing.to_radians(), elevation.to_radians());
        let target = combat
            .state
            .targets
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap();
        target.position = std::array::from_fn(|i| {
            flight.position[i]
                + range
                    * (body.forward[i] * bearing.cos() * elevation.cos()
                        + body.right[i] * bearing.sin() * elevation.cos()
                        + body.up[i] * elevation.sin())
        });
        combat.refresh_render(&flight, None);
        for _ in 0..24 {
            combat.step(&mut flight, &world)?;
            combat.advance_render(&flight, None);
        }
        println!(
            "HUD target preview: bearing={} elevation={} display={:?} sensor={:?}",
            bearing.to_degrees(),
            elevation.to_degrees(),
            combat.state.display_target().map(|t| t.id),
            combat.state.designated()
        );
    }
    if let Some(fraction) = damage_preview {
        combat
            .state
            .preview_localized_damage(damage_preview_section, fraction);
        combat.state.player_hp = (f64::from(combat.state.configuration().damage_capacity)
            * (1. - fraction))
            .round() as i32;
        for target in &mut combat.state.targets {
            target.hp = (f64::from(target.initial_hp) * (1. - fraction)).round() as i32;
        }
        combat.refresh_render(&flight, None);
        for _ in 0..damage_preview_ticks {
            flight.step(&tore_input::PilotInput::default(), |x, z| {
                f64::from(world.height(x as f32, z as f32))
            });
            combat.step(&mut flight, &world)?;
            combat.advance_render(&flight, None);
        }
        println!(
            "Damage preview: ticks={} debris={} impacts={} smoke={}",
            damage_preview_ticks,
            combat.state.debris.len(),
            combat
                .state
                .effects
                .iter()
                .filter(|e| e.kind == tore_sim::combat::live::EffectKind::DebrisImpact)
                .count(),
            combat.state.smoke.puffs.len()
        );
    }
    if damage_preview.is_some()
        && let Some(wreck) = &flight.wreck
    {
        println!(
            "Wreck preview: phase={:?} ticks={} polls={} engine_acceleration={:?} position={:?} attitude={:?}",
            wreck.phase,
            wreck.ticks,
            wreck.polls,
            wreck.power.acceleration,
            flight.position,
            [flight.yaw, flight.pitch, flight.bank]
        );
    }
    if let Some(phase) = &ejection_preview {
        if !flight.eject() {
            return Err("Ejection preview requires a living pilot and available seat".into());
        }
        let pilot = flight.escape.as_mut().unwrap();
        pilot.phase = match phase.as_str() {
            "seat" => tore_sim::ejection::Phase::Seat,
            "freefall" => tore_sim::ejection::Phase::Freefall,
            _ => tore_sim::ejection::Phase::Parachute,
        };
        // Isolate the original pilot model from the abandoned aircraft in this art fixture.
        pilot.position[0] += 300.;
        flight_view = 1;
        println!(
            "Ejection preview: {phase}, pilot_alive={}",
            !flight.systems.pilot.dead
        );
    }
    if flight.systems.pilot.dead {
        flight_view = 1;
        if damage_preview.is_some() {
            println!("Pilot death preview: exterior view {flight_view}");
        }
    }
    // The first frame draws the prepared scene, ejected pilot included.
    combat.refresh_render(&flight, None);
    if input_profile.is_none() {
        let default = assets::data_directory()?.join("input-v1.conf");
        if default.exists() {
            input_profile = Some(default);
        }
    }
    if record_input.is_some()
        && (initial_screen != Screen::Flight || animation_capture || flight_probe_ticks.is_some())
    {
        return Err(
            "--record-input requires direct --free-flight without a capture or flight probe".into(),
        );
    }
    let input_recording = if let Some(path) = record_input {
        use std::io::Write;
        let mut file = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        );
        writeln!(file, "{}", tore_input::recording::HEADER)?;
        Some(file)
    } else {
        None
    };
    let preferences_enabled = !smoke_test
        && capture_terrain.is_none()
        && !animation_capture
        && std::env::var_os("TORE_PERF_FRAMES").is_none();
    // Diagnostics ignore saved choices, like the other display preferences.
    let graphics_path = if preferences_enabled {
        Some(assets::data_directory()?.join("graphics-v1.conf"))
    } else {
        None
    };
    let mut graphics = graphics_path
        .as_deref()
        .map_or_else(graphics::Options::default, graphics::Options::load);
    graphics.apply_flags(&graphics_flags)?;
    let mut airport_service =
        tore_sim::airport::Service::new(&world.airport_scene).map_err(std::io::Error::other)?;
    let airport_nav_mode = airport_probe.map_or_else(
        || ground_start.is_some() && weapon_slot.is_none() && !live_fire,
        |(_, aircraft, _)| aircraft.nav_mode,
    );
    if airport_nav_mode {
        combat.state.command(
            tore_sim::combat::live::Command::SelectNav,
            combat::launcher(&flight),
        );
    }
    if let Some(object) = ground_start {
        let airport = world.airport_scene.runway(object).unwrap().airport;
        airport_service.command(
            &world.airport_scene,
            airport_aircraft(&world, &flight, airport_nav_mode),
            tore_sim::airport::Command::SelectAirport(airport),
        );
    }
    if let Some((airport, aircraft, _)) = airport_probe {
        airport_service.command(
            &world.airport_scene,
            aircraft,
            tore_sim::airport::Command::SelectAirport(airport),
        );
        airport_service.command(
            &world.airport_scene,
            aircraft,
            tore_sim::airport::Command::RequestLanding,
        );
    }
    diagnostics::stage_done();
    diagnostics::stage("controller and input initialization");
    let input = input::Input::new(input_profile.as_deref(), native_input)?;
    diagnostics::stage_done();
    diagnostics::stage("application state construction");
    let mut airfield_radio = airfield_radio::AirfieldRadio::default();
    airfield_radio.reset(ground_start.and_then(|id| world.runway_view(id)));
    let mut app = App {
        mission: None,
        ground_start,
        launch_creator,
        ai_wings_enabled: !fixture_wings,
        enemy_skill,
        ai_wings: None,
        ai_mission,
        comms: comms::Comms::new(1),
        airfield_radio,
        radio: Default::default(),
        phrases: comms::phrases(&theater_resources),
        crew_voice: crew_voice::CrewVoice::new(&hornet.profile),
        preference_path: if preferences_enabled {
            Some(assets::data_directory()?.join("preferences-v1.conf"))
        } else {
            None
        },
        preference_saved: String::new(),
        graphics,
        graphics_path,
        input_recording,
        recorded_ticks: 0,
        input,
        focused: true,
        performance: performance::Performance::from_env()?,
        combat,
        hornet,
        researched_flight,
        native_tables,
        previous_flight: flight.clone(),
        flight,
        flight_clock: flight::Clock { remainder: 0. },
        flight_music: Default::default(),
        vapor: probe_vapor,
        turbulence: probe_turbulence,
        turbulence_rng: probe_turbulence_rng,
        g_effects: Default::default(),
        flight_view,
        view_rig: {
            let mut rig = flight_views::Rig::default();
            rig.select(flight_reference);
            rig
        },
        flight_canvas: Default::default(),
        window_size,
        fullscreen: window.fullscreen,
        fullscreen_preference: window.preference,
        flight_ui: {
            let mut ui = flight_ui::FlightUi::default();
            ui.cheats.no_turbulence = !turbulence_enabled;
            ui.menu = flight_menu;
            ui.map.open = flight_map;
            ui.weapon_diagnostics = weapon_diagnostics;
            ui.paused = animation_capture || combat_probe.is_some() || ejection_preview.is_some();
            ui.look = flight_look.map(f32::to_radians);
            ui.zoom = flight_zoom;
            if !matches!(flight_view, 1 | 2) {
                ui.look[1] = ui.look[1].clamp(0., std::f32::consts::FRAC_PI_2);
            }
            ui
        },
        instruments: {
            let mut i = instruments::Instruments::new(instrument_layout, instrument_page);
            apply_sensor_controls(&mut i, sensor_channel, scope_range, scope_history);
            i
        },
        pointer: None,
        theater_resources,
        world,
        airport_service,
        airport_nav_mode,
        airport_commands: Vec::new(),
        camera,
        quick,
        screen: initial_screen,
        frame_time: Instant::now(),
        instrument_time: Instant::now(),
        target_refresh: target_window::Refresh::new(),
        menu,
        audio,
        wing_recipient: None,
        renderer: None,
        modifiers: ModifiersState::empty(),
        smoke_test,
        capture_terrain,
        reimport: None,
        controls: None,
        graphics_screen: None,
        mouse_look: None,
        wheel: 0.,
        head_look: [0.; 2],
        finished: false,
        next_frame: None,
        error: None,
        replay_library: match std::env::var("TORE_RECORD_MISSIONS").as_deref() {
            Err(std::env::VarError::NotPresent) => preferences_enabled,
            Ok("1") => true,
            Ok("0") => false,
            _ => return Err("TORE_RECORD_MISSIONS needs 0 or 1".into()),
        }
        .then(|| assets::data_directory().map(|data| replay::library::Library::new(&data)))
        .transpose()?,
        replay_recorder: None,
    };
    diagnostics::stage_done();
    diagnostics::stage("saved preferences");
    if let Some(path) = app.preference_path.clone() {
        match preferences::read(&path) {
            Ok(text) => match preferences::Preferences::parse(&text) {
                Ok(saved) => {
                    saved.apply(
                        &mut app.flight_ui,
                        &mut app.instruments,
                        &mut app.menu.state,
                    );
                    if std::env::args().any(|a| a == "--instrument-layout")
                        && app.instruments.layout != instrument_layout
                    {
                        app.instruments.toggle_layout();
                    }
                    if let Some(page) = instrument_page {
                        app.instruments.pages = vec![page];
                        app.instruments.selected = 0;
                    }
                    apply_sensor_controls(
                        &mut app.instruments,
                        sensor_channel,
                        scope_range,
                        scope_history,
                    );
                    if std::env::args().any(|a| a == "--flight-zoom") {
                        app.flight_ui.zoom = flight_zoom;
                    }
                    if weapon_diagnostics {
                        app.flight_ui.weapon_diagnostics = true;
                    }
                }
                Err(e) => {
                    log::warn!("Preferences not loaded: {e}; preserving original file");
                    app.preference_path = None;
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                log::warn!("Preferences not loaded: {e}; preserving original file");
                app.preference_path = None;
            }
        }
    }
    if app.native_tables.is_some() {
        app.flight_ui.cheats.no_turbulence = true;
    }
    app.preference_saved = preferences::Preferences::capture(
        &app.flight_ui,
        &app.instruments,
        &app.menu.state,
        app.fullscreen_preference,
    )
    .text();
    if let Some(audio) = &app.audio {
        audio.scene(match app.screen {
            Screen::Flight => audio::music::Scene::Score(0),
            Screen::Main => audio::music::Scene::Main,
            _ => audio::music::Scene::Brief,
        });
        audio.preferences(app.menu.state.music, app.menu.state.effects);
    }
    if controls_menu {
        app.flight_command(flight_ui::Command::ControlsOpen);
    }
    app.input.context(
        app.screen != Screen::Flight || app.flight_ui.frozen(),
        app.focused,
    );
    diagnostics::stage_done();
    shared_event_loop(event_loop)?.run_app_on_demand(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    match app.reimport {
        Some(frame) => Ok(Outcome::Reimport(frame)),
        None => Ok(Outcome::Done),
    }
}

/// Headless sensor control overrides, so a capture or probe can exercise the
/// infrared channel, another scope setting and the history trail.
fn sensor_controls(
    channel: Option<usize>,
    range: Option<usize>,
    history: bool,
) -> tore_sim::sensors::Controls {
    tore_sim::sensors::Controls {
        channel: if channel == Some(1) {
            tore_sim::sensors::Channel::Infrared
        } else {
            tore_sim::sensors::Channel::Radar
        },
        range_index: range.unwrap_or(tore_sim::sensors::DEFAULT_RANGE_INDEX),
        history,
    }
}
fn apply_sensor_controls(
    instruments: &mut instruments::Instruments,
    channel: Option<usize>,
    range: Option<usize>,
    history: bool,
) {
    if let Some(channel) = channel {
        instruments.channel = channel;
    }
    if let Some(range) = range {
        instruments.radar_range = range;
    }
    if history {
        instruments.history = true;
    }
}
/// Profile names of the mouse buttons other than the left one.
fn mouse_control(button: MouseButton) -> Option<&'static str> {
    Some(match button {
        MouseButton::Right => "button:right",
        MouseButton::Middle => "button:middle",
        MouseButton::Back => "button:back",
        MouseButton::Forward => "button:forward",
        _ => return None,
    })
}
fn flight_key(physical: winit::keyboard::PhysicalKey, fallback: &str) -> String {
    use winit::keyboard::{KeyCode, PhysicalKey};
    if let PhysicalKey::Code(code) = physical {
        let name = format!("{code:?}");
        if let Some(letter) = name.strip_prefix("Key") {
            return letter.to_ascii_lowercase();
        }
        if let Some(digit) = name.strip_prefix("Digit") {
            return digit.into();
        }
        return match code {
            KeyCode::BracketLeft => "[",
            KeyCode::BracketRight => "]",
            KeyCode::Equal => "=",
            KeyCode::Minus => "-",
            KeyCode::Comma => ",",
            KeyCode::Period => ".",
            KeyCode::Semicolon => ";",
            KeyCode::Backslash => "\\",
            KeyCode::Quote => "'",
            KeyCode::Slash => "/",
            // FA reads scan codes without the extended bit, so the keypad is
            // the navigation cluster whatever NumLock says (docs/spec/keyboard.md).
            KeyCode::Numpad0 => "Insert",
            KeyCode::NumpadDecimal => "Delete",
            KeyCode::Numpad1 => "End",
            KeyCode::Numpad2 => "ArrowDown",
            KeyCode::Numpad3 => "PageDown",
            KeyCode::Numpad4 => "ArrowLeft",
            KeyCode::Numpad5 => "Numpad5",
            KeyCode::Numpad6 => "ArrowRight",
            KeyCode::Numpad7 => "Home",
            KeyCode::Numpad8 => "ArrowUp",
            KeyCode::Numpad9 => "PageUp",
            KeyCode::NumpadAdd => "=",
            KeyCode::NumpadSubtract => "-",
            KeyCode::NumpadEnter => "Enter",
            KeyCode::NumpadDivide => "/",
            _ => fallback,
        }
        .into();
    }
    fallback.into()
}

fn cycle_player_weapon(
    combat: &mut combat::Combat,
    flight: &flight::State,
    instruments: &mut instruments::Instruments,
    nav_mode: &mut bool,
    forward: bool,
) {
    combat.cancel();
    combat.command(
        if forward {
            tore_sim::combat::live::Command::NextSelection
        } else {
            tore_sim::combat::live::Command::PreviousSelection
        },
        combat::launcher(flight),
    );
    *nav_mode = !combat.state.armed;
    let readout = combat.readout(flight, instruments.rcs_scale_nmi());
    if let Some(index) = readout.weapons.iter().position(|row| row.2) {
        instruments.weapon_page = index / 6;
    }
}

#[cfg(test)]
mod startup_tests {
    use super::*;

    fn path(text: &str) -> Option<PathBuf> {
        Some(PathBuf::from(text))
    }

    #[test]
    fn a_readable_pack_starts_the_game() {
        assert_eq!(
            next_step(true, true, path("/games/fa"), path("/mnt/disc1")),
            Step::Play
        );
        assert_eq!(next_step(true, false, None, None), Step::Play);
    }

    #[test]
    fn a_known_source_is_imported_without_asking() {
        // The remembered source, or a developer checkout, needs no click.
        assert_eq!(
            next_step(false, true, path("/games/fa"), path("/mnt/disc1")),
            Step::ImportNow(PathBuf::from("/games/fa"))
        );
        assert_eq!(
            next_step(false, false, path("/games/fa"), None),
            Step::ImportNow(PathBuf::from("/games/fa"))
        );
    }

    #[test]
    fn an_unknown_source_asks_the_player_when_there_is_a_window() {
        assert_eq!(
            next_step(false, true, None, path("/mnt/disc1")),
            Step::Ask(path("/mnt/disc1"))
        );
        assert_eq!(next_step(false, true, None, None), Step::Ask(None));
        // Headless runs keep the terminal message package C wrote.
        assert_eq!(
            next_step(false, false, None, path("/mnt/disc1")),
            Step::Fail
        );
    }

    #[test]
    fn fullscreen_is_the_default_and_the_flags_win() {
        use super::{WindowMode::*, initial_window_mode};
        // An ordinary interactive start.
        assert_eq!(initial_window_mode(false, false, false, true), Fullscreen);
        // Each opt-out on its own.
        assert_eq!(initial_window_mode(true, false, false, true), Windowed);
        assert_eq!(initial_window_mode(false, true, false, true), Windowed);
        assert_eq!(initial_window_mode(false, false, true, true), Windowed);
        assert_eq!(initial_window_mode(false, false, false, false), Windowed);
        // A saved fullscreen preference never overrides a flag.
        assert_eq!(initial_window_mode(true, false, true, true), Windowed);
        // And a windowed preference is not undone by a fixed-size run.
        assert_eq!(initial_window_mode(false, false, true, false), Windowed);
        assert!(Fullscreen.fullscreen());
        assert!(!Windowed.fullscreen());
        assert!(super::fullscreen_attribute(false).is_none());
        assert!(super::fullscreen_attribute(true).is_some());
    }
    #[test]
    fn the_smoke_test_imports_the_prefilled_source() {
        let ask = Step::Ask(path("/mnt/disc1"));
        assert_eq!(auto_import_path(&ask, None, false), None);
        // Under --smoke-test the field's own path is imported, so a first run
        // can be checked end to end without clicks.
        assert_eq!(
            auto_import_path(&ask, Some(Path::new("/mnt/disc1")), true),
            path("/mnt/disc1")
        );
        assert_eq!(auto_import_path(&Step::Ask(None), None, true), None);
        // A known source starts on its own either way.
        let known = Step::ImportNow(PathBuf::from("/games/fa"));
        assert_eq!(auto_import_path(&known, None, false), path("/games/fa"));
        assert_eq!(
            auto_import_path(&known, Some(Path::new("/mnt/disc1")), true),
            path("/games/fa")
        );
    }

    #[test]
    fn named_keys_reach_the_locate_screen_and_letters_do_not() {
        use winit::keyboard::NamedKey;
        assert_eq!(locate_key_name(&Key::Named(NamedKey::Enter)), Some("Enter"));
        assert_eq!(
            locate_key_name(&Key::Named(NamedKey::Backspace)),
            Some("Backspace")
        );
        // Printable characters go through text_input, with their case intact.
        assert_eq!(locate_key_name(&Key::Character("D".into())), None);
        assert_eq!(locate_key_name(&Key::Named(NamedKey::Space)), None);
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use winit::keyboard::{KeyCode, PhysicalKey};
    #[test]
    fn physical_keys_survive_shift_and_option_translations() {
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::Digit1), "!"), "1");
        // The keypad follows FA, whatever NumLock reports.
        for (code, logical, name) in [
            (KeyCode::Numpad0, "0", "Insert"),
            (KeyCode::NumpadDecimal, ".", "Delete"),
            (KeyCode::Numpad3, "3", "PageDown"),
            (KeyCode::Numpad5, "5", "Numpad5"),
            (KeyCode::Numpad8, "ArrowUp", "ArrowUp"),
        ] {
            assert_eq!(flight_key(PhysicalKey::Code(code), logical), name);
        }
        assert_eq!(
            flight_key(PhysicalKey::Code(KeyCode::BracketLeft), "{"),
            "["
        );
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::KeyE), "é"), "e");
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::Equal), "+"), "=");
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::Slash), "?"), "/");
    }

    #[test]
    fn airport_replies_route_verified_recordings_and_text_fallback() {
        use tore_sim::airport::{ApproachEnd, DeclineReason, Reply};
        let cleared = Reply::Cleared {
            airport: 1,
            runway: 2,
            end: ApproachEnd::Near,
        };
        assert_eq!(airport_reply_audio(&cleared), Some("^CLRLAND"));
        assert_eq!(
            airport_reply_audio(&Reply::Repeated(Box::new(cleared))),
            Some("^CLRLAND")
        );
        assert_eq!(
            airport_reply_audio(&Reply::Declined {
                airport: Some(1),
                reason: DeclineReason::RunwayDisabled,
            }),
            None
        );
        assert_eq!(
            airport_reply_audio(&Reply::Cancelled { airport: Some(1) }),
            None
        );
    }
}

#[cfg(test)]
mod probe_tests {
    use super::*;
    use tore_sim::ai::wing::PlayerOrder;

    #[test]
    fn probe_attack_takes_a_tick_and_an_optional_repeat() {
        assert_eq!(
            ProbeScript::parse_attack("600").unwrap(),
            ProbeAttack {
                from: 600,
                repeat: 0
            }
        );
        assert_eq!(
            ProbeScript::parse_attack("600:10").unwrap(),
            ProbeAttack {
                from: 600,
                repeat: 1200
            }
        );
        assert_eq!(ProbeScript::parse_attack("0:0.5").unwrap().repeat, 60);
        for bad in ["", "x", "600:", "600:0", "600:-1", "600:3601", "-1:10"] {
            assert!(ProbeScript::parse_attack(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn probe_wing_orders_include_the_attack_orders() {
        assert_eq!(
            ProbeScript::parse_order("600:attack-on-contact").unwrap(),
            (600, PlayerOrder::AttackOnContact)
        );
        assert_eq!(
            ProbeScript::parse_order("601:engage-my-target").unwrap(),
            (601, PlayerOrder::EngageMyTarget)
        );
        assert_eq!(
            ProbeScript::parse_order("9000:bug-out").unwrap(),
            (9000, PlayerOrder::BugOut)
        );
        assert!(ProbeScript::parse_order("600:attack").is_err());
    }
}
