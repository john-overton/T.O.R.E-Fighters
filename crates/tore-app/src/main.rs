mod additional_animation;
mod aircraft;
mod aircraft_animation;
mod assets;
mod attitude;
mod audio;
mod celestial;
mod clouds;
mod cockpit_renderer;
mod combat;
mod combat_tape;
mod controls_editor;
mod engine_material;
mod flight;
mod flight_canvas;
mod flight_ui;
mod hud;
mod input;
mod instruments;
mod lens_flare;
mod look;
mod menu;
mod mirrors;
mod ocean;
mod ordnance;
mod performance;
mod preferences;
mod quick_mission;
mod rafale_animation;
mod renderer;
mod roster_animation;
mod scope;
mod sim_renderer;
mod terrain;
mod weather;

use assets::Assets;
use menu::{Action, Menu};
use renderer::Renderer;
use std::{
    error::Error,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tore_sim::models::FlightModel;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState},
    window::{CursorIcon, Window, WindowId},
};
type AppResult<T> = Result<T, Box<dyn Error>>;
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
    vapor: tore_sim::vapor::Vapor,
    turbulence: tore_sim::turbulence::Turbulence,
    turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng,
    flight_view: u8,
    flight_canvas: flight_canvas::FlightCanvas,
    window_size: [u32; 2],
    flight_ui: flight_ui::FlightUi,
    instruments: instruments::Instruments,
    world: terrain::World,
    theater_resources: std::collections::BTreeMap<String, Vec<u8>>,
    camera: terrain::Camera,
    quick: quick_mission::QuickMission,
    mission: Option<(f64, f64)>,
    screen: Screen,
    frame_time: Instant,
    instrument_time: Instant,
    menu: Menu,
    audio: Option<audio::Audio>,
    renderer: Option<Renderer>,
    pointer: Option<(f64, f64)>,
    modifiers: ModifiersState,
    smoke_test: bool,
    capture_terrain: Option<PathBuf>,
    finished: bool,
    next_frame: Option<Instant>,
    error: Option<Box<dyn Error>>,
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

impl App {
    /// Rebuilds the world under one recovered weather condition. The renderer
    /// owns per-world GPU resources, so it is rebuilt with it.
    fn set_condition(&mut self, index: usize) -> AppResult<()> {
        let code = self
            .world
            .environment
            .map
            .trim_end_matches(".T2")
            .to_string();
        self.world = terrain::World::for_mission(&self.theater_resources, &code, Some(index))?;
        if let Some(renderer) = &mut self.renderer {
            renderer.set_world(&self.world);
            renderer.prepare_aircraft(&self.hornet);
        }
        Ok(())
    }

    /// Restart the resolved launch environment and its authored RNG policy.
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

    fn save_preferences(&mut self) {
        let Some(path) = &self.preference_path else {
            return;
        };
        let text =
            preferences::Preferences::capture(&self.flight_ui, &self.instruments, &self.menu.state)
                .text();
        if text == self.preference_saved {
            return;
        }
        match preferences::write(path, &text) {
            Ok(()) => self.preference_saved = text,
            Err(e) => {
                eprintln!("Preferences: {e}");
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
            "designate" => Command::Target,
            "clear-designation" => {
                Command::Combat(tore_sim::combat::live::Command::ClearDesignation)
            }
            "master-arm" => Command::Combat(tore_sim::combat::live::Command::ToggleArm),
            "jettison" => Command::Combat(tore_sim::combat::live::Command::Jettison),
            "range-target" => Command::RangeReset,
            "damage-class" => Command::Combat(tore_sim::combat::live::Command::CycleClass),
            "fail-station" => Command::Combat(tore_sim::combat::live::Command::FailStation),
            "damage-player" => Command::Combat(tore_sim::combat::live::Command::DamagePlayer),
            "incoming" => Command::Combat(tore_sim::combat::live::Command::Incoming),
            "target-jammer" => Command::Combat(tore_sim::combat::live::Command::ToggleTargetJammer),
            "end-flight" => Command::End,
            "restart" => Command::Restart,
            "view-front" => Command::View(0),
            "view-back" => Command::View(3),
            "view-up" => Command::View(4),
            "view-external" => Command::View(1),
            "center-look" => Command::CenterLook,
            "instrument-next" => Command::InstrumentCycle(1),
            "instrument-previous" => Command::InstrumentCycle(-1),
            "range-down" => Command::Range(-1),
            "range-up" => Command::Range(1),
            // The recovered radar-mode action is the available sensor-channel
            // cycle; the two newer names are explicit aliases for it.
            "radar-mode" | "sensor-channel" => Command::Mode,
            "sensor-infrared" => Command::SensorInfrared,
            "sensor-history" => Command::SensorHistory,
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
        if self.native_tables.is_some() && !self.flight_ui.no_turbulence {
            self.flight_ui.no_turbulence = true;
            self.flight_ui
                .message("Environmental turbulence is unavailable in native research flight");
        }
        match command {
            Command::None => Action::None,
            Command::Click => Action::Click,
            Command::End => Action::Back,
            Command::Exit => Action::Exit,
            Command::Restart => Action::FreeFlight,
            Command::Combat(command) => {
                if self.combat.range
                    || matches!(
                        command,
                        tore_sim::combat::live::Command::ToggleArm
                            | tore_sim::combat::live::Command::ClearDesignation
                    )
                {
                    self.combat.cancel();
                    self.combat.command(command, combat::launcher(&self.flight));
                    if let Err(error) = self.flight.set_payload(self.combat.state.payload_lbs()) {
                        self.flight_ui.message(error.to_string());
                    }
                } else {
                    self.flight_ui
                        .message("Manual range command requires --live-fire");
                }
                Action::None
            }
            Command::NextWeapon => {
                self.combat.cancel();
                self.combat.command(
                    tore_sim::combat::live::Command::NextWeapon,
                    combat::launcher(&self.flight),
                );
                Action::None
            }
            Command::Target => {
                self.combat.command(
                    tore_sim::combat::live::Command::Designate,
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
                self.flight_ui.controls_editor = Some(controls_editor::Editor::new(
                    self.input.settings_profile(),
                    self.input.devices.values().cloned().collect(),
                ));
                Action::Click
            }
            Command::ControlsSave => {
                if let Some(editor) = &mut self.flight_ui.controls_editor {
                    editor.message = match self.input.save_settings(&editor.profile) {
                        Ok(()) => "Controls saved and applied".into(),
                        Err(e) => e,
                    };
                }
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
                Action::None
            }
            Command::View(view) => {
                self.flight_view = view;
                self.flight_ui.look = [0.; 2];
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
            Command::Throttle(value) => {
                self.input.queue(tore_input::PilotCommand::Throttle(value));
                Action::None
            }
            Command::Range(delta) => {
                let last = match self.instruments.pages.last() {
                    Some(5) => {
                        self.instruments.rwr_range =
                            (self.instruments.rwr_range as i32 + delta).clamp(0, 4) as usize;
                        return Action::Click;
                    }
                    Some(0) => {
                        let scales = tore_sim::sensors::passive::SCALE_LADDER_NMI.len() as i32 - 1;
                        self.instruments.rcs_range =
                            (self.instruments.rcs_range as i32 + delta).clamp(0, scales) as usize;
                        return Action::Click;
                    }
                    _ => tore_sim::sensors::RANGE_LADDER_NMI.len() as i32 - 1,
                };
                self.instruments.radar_range =
                    (self.instruments.radar_range as i32 + delta).clamp(0, last) as usize;
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
    fn action(&mut self, event_loop: &ActiveEventLoop, action: Action) {
        if action == Action::Exit {
            self.finished = true;
            event_loop.exit();
            return;
        }
        match action {
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
                            if let Some(renderer) = &mut self.renderer {
                                renderer.set_world(&world);
                                renderer.prepare_aircraft(&self.hornet);
                            }
                            self.camera = terrain::Camera::for_world(&world);
                            self.world = world;
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
                if let Some(&id) = tore_formats::aircraft::AircraftId::ALL.get(index) {
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
                                Ok(c)
                            }) {
                                Ok(c) => self.combat = c,
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
                let index = tore_formats::aircraft::AircraftId::ALL
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
                    let o = self.quick.ordnance.as_mut().unwrap();
                    if self.quick.draft.values[19] == 0 {
                        for (s, n) in o
                            .loadout
                            .configuration
                            .stations
                            .iter()
                            .zip(&mut o.loadout.quantities)
                        {
                            if !s.internal {
                                *n = 0;
                            }
                        }
                    }
                    o.visible = true;
                    o.message=Some("Airborne patrol preview: no enemy AI or mission objectives yet. Review your load, then Fly.".into());
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
                if self.quick.draft.values[19] == 0
                    && load
                        .configuration
                        .stations
                        .iter()
                        .zip(&load.quantities)
                        .any(|(s, n)| !s.internal && *n > 0)
                {
                    self.quick.ordnance.as_mut().unwrap().message=Some("Guns only is selected. Unload external weapons or return to setup and change the restriction.".into());
                    return;
                }
                let altitude = [5000., 10000., 20000., 40000.][self.quick.draft.values[14]];
                let start = self.hornet.start(&self.world);
                let ground = f64::from(
                    self.world
                        .height(start.position[0] as f32, start.position[2] as f32),
                );
                if altitude < ground + 100. {
                    self.quick.ordnance.as_mut().unwrap().message = Some(format!(
                        "Selected altitude is below safe terrain clearance ({:.0} feet). Select a higher altitude.",
                        ground + 100.
                    ));
                    return;
                }
                let fuel = load.fuel_lbs;
                match combat::Combat::with_loadout(&self.hornet, &self.theater_resources, load) {
                    Ok(c) => {
                        self.combat = c;
                        self.mission = Some((altitude, fuel));
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
                if let Some(audio) = &self.audio {
                    audio.restart_flight();
                }
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
                self.reset_vapor();
                self.previous_flight = self.flight.clone();
                self.flight_clock.remainder = 0.;
                self.flight_view = 0;
                let saved = preferences::Preferences::capture(
                    &self.flight_ui,
                    &self.instruments,
                    &self.menu.state,
                );
                self.flight_ui.reset_for_flight();
                saved.apply(
                    &mut self.flight_ui,
                    &mut self.instruments,
                    &mut self.menu.state,
                );
                self.flight_ui.effects = self.menu.state.effects;
                self.screen = Screen::Flight;
                self.camera.keys.clear();
                self.combat.cancel();
                self.quick.cancel();
                self.frame_time = Instant::now();
            }
            Action::Back => {
                if let Err(e) = self.combat.finish_recording() {
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
                if let Some(renderer) = &mut self.renderer {
                    renderer.combat(&[]);
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
            renderer.window.set_cursor(
                if (self.screen == Screen::Main && self.menu.state.hover.is_some())
                    || (self.screen == Screen::Quick && self.quick.hover.is_some())
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
                        .with_inner_size(LogicalSize::new(self.window_size[0], self.window_size[1]))
                        .with_min_inner_size(LogicalSize::new(640.0, 480.0)),
                )?,
            );
            pollster::block_on(Renderer::new(window, &self.world))
        })();
        match result {
            Ok(mut renderer) => {
                renderer.prepare_aircraft(&self.hornet);
                renderer.window.request_redraw();
                self.renderer = Some(renderer);
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
                self.pointer = None;
                self.camera.keys.clear();
                self.combat.cancel();
                self.modifiers = ModifiersState::empty();
                Action::None
            }
            WindowEvent::CursorMoved { position, .. } => {
                let point = renderer.viewport().point(position.x, position.y);
                self.pointer = Some((position.x, position.y));
                match self.screen {
                    Screen::Main => self.menu.state.pointer(point),
                    Screen::Quick => {
                        self.quick.pointer(point);
                        Action::None
                    }
                    Screen::Viewer | Screen::Flight => Action::None,
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
                self.pointer = None;
                self.camera.keys.clear();
                self.combat.cancel();
                self.modifiers = ModifiersState::empty();
                Action::None
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } if self.screen == Screen::Quick => self
                .quick
                .ordnance
                .as_mut()
                .filter(|o| o.visible)
                .map_or(Action::None, |o| o.right(state == ElementState::Pressed)),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if self.screen == Screen::Flight && self.flight_ui.menu {
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
                        self.quick.down();
                        Action::None
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
                if self.screen == Screen::Flight
                    && (event.state == ElementState::Released
                        || (self.flight_ui.controls_editor.is_none()
                            && !(self.flight_ui.menu
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
                    if self.flight_ui.frozen() || before != self.flight_ui.frozen() {
                        self.camera.keys.clear();
                        self.combat.cancel();
                        self.instruments.cancel_press();
                        self.flight_clock.remainder = 0.;
                        self.previous_flight.clone_from(&self.flight);
                        self.frame_time = Instant::now();
                    } else {
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
                    Screen::Main => self.menu.render(),
                    Screen::Quick => {
                        self.quick.render(
                            &mut self.menu.pixels,
                            &self.menu.quick_sprites,
                            &self.world,
                        );
                        false
                    }
                    Screen::Flight => {
                        self.world.no_sun_whiteout = self.flight_ui.no_sun_whiteout;
                        let now = Instant::now();
                        let elapsed = (now - self.frame_time).as_secs_f64().min(0.25);
                        let steps = self.flight_ui.steps(&mut self.flight_clock, elapsed);
                        self.frame_time = now;
                        if let Some(audio) = &self.audio {
                            audio.pause_flight(self.flight_ui.frozen());
                        }
                        // Scope channel, display range and history are player
                        // controls, applied as a simulation input so replay
                        // reproduces every change and the labels never lag.
                        self.flight.sensors = self.instruments.controls();
                        for _ in 0..steps {
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
                            if let Some(error) = self.flight.native_fault() {
                                self.flight_ui.message(error.to_owned());
                                self.flight_ui.paused = true;
                                eprintln!("{error}");
                                break;
                            }
                            // Weather shares the authoritative tick; pausing simply
                            // stops calling it, with no elapsed-time catch-up.
                            let mut weather_view = self.hornet.camera(
                                &self.flight,
                                self.flight_view,
                                Default::default(),
                            );
                            look::apply(
                                &mut weather_view,
                                self.flight.position.map(|v| v as f32),
                                self.flight_ui.look,
                                matches!(self.flight_view, 1 | 2),
                            );
                            self.world.step_weather(self.flight.speed, &weather_view);
                            self.world.step_view_weather(
                                &mirrors::camera(&self.flight),
                                self.flight.speed,
                            );
                            for page in [2, 3] {
                                self.world.step_view_weather(
                                    &self.hornet.panel_camera(&self.flight, page),
                                    self.flight.speed,
                                );
                            }
                            let turbulence_cue = step_turbulence(
                                &mut self.turbulence,
                                &mut self.turbulence_rng,
                                &mut self.flight,
                                &self.world,
                                !self.flight_ui.no_turbulence,
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
                            let events = match self.combat.step(&mut self.flight, &self.world) {
                                Ok(events) => events,
                                Err(e) => {
                                    self.error = Some(e);
                                    event_loop.exit();
                                    return;
                                }
                            };
                            let mut sounds = std::collections::BTreeSet::new();
                            for event in &events {
                                use tore_sim::combat::live::Event;
                                if let Some(cue) =
                                    combat::feedback(event, self.combat.state.configuration())
                                {
                                    self.input.feedback(cue);
                                }
                                match event {
                                    Event::PlayerDamaged(_) => {
                                        sounds.insert("&EXPL3.5K");
                                    }
                                    Event::SubsystemDamaged(_)
                                    | Event::Defeated(_)
                                    | Event::TrackLost(_)
                                    | Event::SeekerActivated(_)
                                    | Event::Pitbull(_) => {}
                                    Event::PlayerDestroyed => {
                                        sounds.insert("&EXPL12.5K");
                                        self.flight.crashed = true;
                                    }
                                    Event::Fired(i) => {
                                        if let Some(name) =
                                            self.combat.state.configuration().stations[*i]
                                                .weapon
                                                .fire_sound
                                                .as_deref()
                                        {
                                            sounds.insert(name);
                                        }
                                    }
                                    Event::Hit(_) | Event::Ground => {
                                        sounds.insert("&EXPL3.5K");
                                    }
                                    Event::Destroyed(_) => {
                                        sounds.insert("&EXPL12.5K");
                                    }
                                }
                            }
                            if let Some(audio) = &self.audio {
                                audio.combat(&sounds.into_iter().collect::<Vec<_>>());
                            }

                            if self.flight.crashed && !self.previous_flight.crashed {
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
                        }
                        let presented = if self.flight_ui.frozen() {
                            self.flight.clone()
                        } else {
                            self.flight.presented(
                                &self.previous_flight,
                                self.flight_clock.remainder / flight::DT,
                            )
                        };
                        self.camera = self.hornet.camera(
                            &presented,
                            self.flight_view,
                            std::mem::take(&mut self.camera.keys),
                        );
                        look::apply(
                            &mut self.camera,
                            presented.position.map(|v| v as f32),
                            self.flight_ui.look,
                            matches!(self.flight_view, 1 | 2),
                        );
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
                        match renderer.poll_previews() {
                            Ok(previews) => {
                                self.performance.completed_previews += previews.len();
                                for (page, pixels) in previews {
                                    self.instruments.cameras.insert(page, pixels);
                                }
                            }
                            Err(e) => {
                                self.error = Some(e);
                                event_loop.exit();
                                return;
                            }
                        }
                        renderer.combat(&self.combat.vertices(
                            &self.hornet,
                            &presented,
                            &self.camera,
                            &self.world,
                        ));
                        if now.duration_since(self.instrument_time).as_millis() >= 100
                            || self.smoke_test
                        {
                            self.instrument_time = now;
                            for page in [2, 3] {
                                if self.instruments.pages.contains(&page) {
                                    let camera = self.hornet.panel_camera(&presented, page);
                                    renderer.aircraft(
                                        &self.hornet,
                                        &presented,
                                        page == 3,
                                        &camera,
                                        &self.world,
                                    );
                                    let result = if self.smoke_test {
                                        renderer
                                            .scene_pixels(&camera, &self.world, 138, 114, false)
                                            .map(|p| {
                                                self.instruments.cameras.insert(page, p);
                                            })
                                    } else {
                                        renderer.request_preview(page, &camera, &self.world)
                                    };
                                    if let Err(e) = result {
                                        self.error = Some(e);
                                        event_loop.exit();
                                        return;
                                    }
                                }
                            }
                        }
                        renderer.aircraft(
                            &self.hornet,
                            &presented,
                            matches!(self.flight_view, 1 | 2),
                            &self.camera,
                            &self.world,
                        );
                        simulation_ms = frame_start.elapsed().as_secs_f64() * 1000.;
                        self.instruments.combat = Some(
                            self.combat
                                .readout(&self.flight, self.instruments.rcs_scale_nmi()),
                        );
                        // Hover feedback uses the same projection as the click,
                        // so the selector marks the contact a click would take.
                        let window = renderer.window.inner_size();
                        self.instruments.hover(
                            self.pointer,
                            [f64::from(window.width), f64::from(window.height)],
                        );
                        self.flight_canvas.begin(
                            renderer.flight_size(),
                            &self.hornet,
                            &presented,
                            &self.instruments,
                        );
                        let cockpit_palette = self.hornet.cockpit_palette(
                            &self.world,
                            self.camera.position[1] as f64,
                            self.flight_ui.brightness,
                        );
                        self.menu.pixels.fill(0);
                        if self.flight_ui.hud && matches!(self.flight_view, 0 | 3 | 4) {
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
                                cockpit_palette[usize::from(self.hornet.hud.primary_color)],
                                self.flight_canvas.hud_zoom(1.),
                            );
                        }
                        renderer.cockpit(
                            &presented,
                            &self.camera,
                            self.flight_ui.cockpit && matches!(self.flight_view, 0 | 3 | 4),
                            self.flight_ui.hud && matches!(self.flight_view, 0 | 3 | 4),
                            &self.menu.pixels,
                            &cockpit_palette,
                        );
                        self.menu.pixels.fill(0);
                        self.flight_ui.draw(
                            &mut self.menu.pixels,
                            &self.hornet.font,
                            &self.hornet.flight_menu,
                        );
                        self.flight_canvas.legacy_layer(&self.menu.pixels, 1.);
                        if let Some(audio) = &self.audio {
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
                    renderer.aircraft(&self.hornet, &self.flight, false, &self.camera, &self.world);
                    if let Some(audio) = &self.audio {
                        audio.pause_flight(false);
                        audio.flight(None);
                    }
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
                if self.screen == Screen::Flight
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
            if let Some(editor) = &mut self.flight_ui.controls_editor {
                if !editor.capture {
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
                    changed |= editor.observe(event);
                }
                if changed && let Some(renderer) = &self.renderer {
                    renderer.window.request_redraw();
                }
            }
            for warning in warnings {
                eprintln!("Input: {warning}");
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
                    if self.flight_ui.controls_editor.is_some() {
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
fn main() -> AppResult<()> {
    let mut args = std::env::args().skip(1);
    let mut live_fire = false;
    let mut jammer_on = false;
    let mut combat_smoke = false;
    let mut record_combat = None;
    let mut replay_combat = None;
    let mut combat_probe = None;
    let mut combat_commands = Vec::new();
    let mut weapon_slot = 1usize;
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
    let mut flight_look = [0f32; 2];
    let mut flight_zoom = 1f32;
    let mut flight_menu = false;
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
    let mut maneuver = String::from("level");
    let mut panel_snapshot = None;
    let mut validate_creator = false;
    let mut sensor_summary = false;
    let mut validate_weather = false;
    let mut weather_condition: Option<usize> = None;
    let (mut smoke_test, mut no_audio, mut import_only) = (false, false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-controllers" => native_input = false,
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
            "--live-fire" => {
                live_fire = true;
                initial_screen = Screen::Flight;
            }
            "--combat-smoke" => {
                combat_smoke = true;
            }
            "--weapon-slot" => {
                weapon_slot = args
                    .next()
                    .ok_or("--weapon-slot requires a 1-based PT weapon slot")?
                    .parse()?;
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
            "--flight-view" => {
                flight_view = args
                    .next()
                    .ok_or("--flight-view needs 0, 1, 2, 3 or 4")?
                    .parse::<u8>()?;
                if flight_view > 4 {
                    return Err("--flight-view needs 0, 1, 2, 3 or 4".into());
                }
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
                    "--maneuver needs level, pull, loop, roll, stall, spin, bank-left or bank-right",
                )?;
                if ![
                    "level",
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
                    args.next().ok_or("--import needs a media directory")?,
                ))
            }
            "--snapshot" => {
                snapshot = Some(PathBuf::from(
                    args.next().ok_or("--snapshot needs a .ppm output path")?,
                ))
            }
            "--smoke-test" => smoke_test = true,
            "--no-audio" => no_audio = true,
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
                    "Creator: --quick-mission opens setup; --snapshot-state ordnance opens the loadout preview; --validate-creator checks all imported loadouts and restart without a display.\nCombat: --live-fire starts an explicit PT-default range. Space fires; semicolon cycles weapons; T designates; backslash resets target. --weapon-slot N selects a 1-based weapon slot. --combat-command NAME applies a manual setup command before the probe. U arm/safe; K jettison selected external group; L clears designation; ] cycles damage-class fixture; [ fails selected station (restart repairs). D injects a gun-strength player hit; Shift-I launches one incoming selected weapon; Shift-Y toggles target ECM; J toggles own ECM (--jammer-on starts powered). Select is the gamepad combat modifier; see INPUT.md. --record-combat NEW_PATH writes version-3 combat-service inputs, including the sensor controls; --replay-combat PATH replays them headlessly with matching --aircraft/--theater and assets. --combat-smoke runs all default slots and five damage classes; TORE_COMBAT_EVIDENCE=DIR also roundtrips per-slot tapes. --combat-probe-ticks 1..7200 advances a scripted firing pass before --capture-flight.\nSensors: one shared radar/infrared component serves every imported aircraft. M or O cycles the available channels, I selects infrared, R returns to radar, Y toggles contact history, comma/period change the scope setting and a click designates a contact. --sensor-summary prints each aircraft's imported capability; --sensor-channel radar|ir, --scope-range 5|10|25|50|100|150 and --scope-history set the scope for a headless capture. Guidance/contact/damage coupling is a development approximation, not native parity."
                );
                println!(
                    "Controllers: --no-controllers, --record-input NEW_PATH, --replay-input PATH, --list-inputs, --monitor-inputs SECONDS, --write-input-profile NEW_PATH, --input-profile PATH, --test-rumble DEVICE_ID|only, --controls-menu. See docs/INPUT.md.\nInstrument focus: Ctrl-Tab / Ctrl-Shift-Tab, Ctrl-1..6; Ctrl-Shift-1..4 operates selected instrument buttons."
                );
                println!(
                    "Usage: tore-app [--free-flight | --viewer | --quick-mission] [--theater CODE] [--capture-terrain OUTPUT.ppm] [--import MEDIA_DIR] [--import-only] [--no-audio] [--smoke-test] [--snapshot OUTPUT.ppm] [--snapshot-state STATE] [--background NAME]\n\nImports original menus, all theaters, F/A-18D, Rafale C, F-14D, A-4E, X-31 EFM, MiG-29, Su-27, MiG-21, Su-25, MiG-23, Su-35 and F-22A assets into platform application data.\nA local gameassets/fighters-anthology directory is imported automatically on first run.\n--aircraft f18|rafale|f14|a4e|x31|mig29|su27|mig21|su25|mig23|su35|f22 selects the aircraft (default f18).\n--free-flight launches the selected aircraft; --headless-flight TICKS runs without a display.\nFlight: Shift/Ctrl-arrows look/orbit, Shift-/ recenter. Arrows pitch/bank, Z/X rudder, PageUp/Down throttle, Shift-B burner. F1 front, F2 back, F3 up, F10 external. Shift-0..9 instruments. Esc > Pref > Large windows? switches four-corner/six-bottom layouts. Esc flight menu, Ctrl-P pause, Backspace cockpit, F11 keyboard help. See docs/FLIGHT-CONTROLS.md.\n--quick-mission opens the creator; --viewer opens the selected theater.\n--theater CODE selects one of the 16 original theater codes (default UKR).
Weather: --weather-condition 0..5 selects one of the six source choices (clear, cloudy, foggy, dawn, sunset, night); --validate-weather checks every imported module, one full simulated day and every choice without a display. TORE_WEATHER_TIME=HH:MM overrides the launch time for matched captures; TORE_VAPOR_PROBE=1 prints the resolved wing vapor trail headlessly.\n--capture-flight PATH captures flight with instruments; --flight-view 0/1/2/3/4 chooses cockpit/chase/oblique/back/up. --flight-menu captures the paused menu. --flight-look YAW,PITCH sets look angles in degrees for inspection. --flight-zoom 0.5..4 sets initial zoom.\n--flight-throttle 0..1 sets initial throttle for material inspection. --flight-bay 0..1 sets an F-22 main-bay pose. Shift-O toggles bays in flight.\n--flight-devices G,F,B,H,AB sets initial fractions (0..1); --flight-controls pitch,roll,rudder sets initial deflections (-1..1). Animation captures pause at the specified pose.\n--instrument-layout large/small selects four corners or six bottom windows.\n--panel-snapshot PATH writes one instrument; --instrument-page 0..9 selects it.\n--native-flight-tables DIR enables airborne native research using extracted sine/atan tables; environmental turbulence and native contact/lifecycle producers are unavailable.\n--researched-flight explicitly selects the default hybrid flight/contact model (not native parity). --legacy-flight selects the previous compatibility model.\n--native-flight-report prints static-translated helper probes (not a native simulation). --native-flight-trig PATH additionally probes an extracted sine-q15.bin table.\n--headless-flight TICKS supports --maneuver level/pull/loop/roll/stall/spin/bank-left/bank-right. --flight-probe-ticks TICKS advances that maneuver before a rendered flight (maximum 7200 ticks).\n--capture-terrain writes a GPU-rendered 960x720 terrain PPM and exits (display required).\nViewer: arrows move; Shift speeds up; Q/E or PageDown/PageUp change altitude; A/D turn; W/S pitch; Escape returns.\n--snapshot writes a headless 640x480 menu preview and exits (supports --quick-mission).\n--snapshot-state: normal, hover, pressed, help, pref, multi, notice. Quick mission: normal, aircraft, theaters, help.\n--background: CHOOSEAC, CHOOSE3, CHOOSEU, CHOOSEM, CHOOSEV (default: random; snapshots use CHOOSEV).\n--smoke-test presents one frame without audio and exits.\nTORE_DATA_DIR overrides the application data directory.\nTab/arrows + Enter navigate; Escape dismisses; M toggles music; ? contains Exit."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
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
    if let Some(seconds) = input_seconds {
        return input::diagnostics(
            seconds,
            write_input_profile.as_deref(),
            test_rumble.as_deref(),
        );
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
    let data = assets::data_directory()?;
    let mut assets = if let Some(source) = import {
        Assets::import(&source, &data)?
    } else {
        match Assets::load(&data) {
            Ok(assets) => assets,
            Err(error) => {
                let local = PathBuf::from("gameassets/fighters-anthology");
                if local.is_dir() {
                    Assets::import(&local, &data)?
                } else {
                    return Err(format!("{error}\nImport your own Fighters Anthology media with --import <directory>.").into());
                }
            }
        }
    };
    if import_only {
        return Ok(());
    }
    let hornet = aircraft::Airframe::load(&assets.theater_resources, aircraft_id)?;
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
        return Ok(());
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
        return Ok(());
    }
    if combat_smoke {
        return combat::smoke(&hornet, &assets.theater_resources);
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
        return Ok(());
    }
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
        let replay_world = replay_frames
            .as_ref()
            .map(|_| terrain::World::for_theater(&assets.theater_resources, &theater_code))
            .transpose()?;
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
        let keys = setup_maneuver(&mut state);
        if let Some(tables) = &native_tables {
            state.enable_native(tables.clone(), 1)?;
        }
        let initial_forward = attitude::Basis::new(state.yaw, state.pitch, state.bank).forward;
        let (mut vertical, mut inverted, mut completed) = (false, false, false);
        for tick in 0..ticks {
            let keys = replay_frames.as_ref().map_or(&keys, |frames| &frames[tick]);
            state.step(keys, |x, z| {
                if let Some(world) = &replay_world {
                    world.height(x as f32, z as f32) as f64
                } else {
                    0.
                }
            });
            if let Some(error) = state.native_fault() {
                return Err(error.into());
            }
            let basis = attitude::Basis::new(state.yaw, state.pitch, state.bank);
            vertical |= basis.forward[1] > 0.999;
            inverted |= basis.up[1] < -0.9;
            completed |= inverted
                && basis.up[1] > 0.9
                && attitude::dot(basis.forward, initial_forward) > 0.98;
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
        return Ok(());
    }
    if let Some(path) = panel_snapshot {
        use std::io::Write;
        let state = flight::State::new(&hornet.profile, [0., 5000., 0.])?;
        let r =
            instruments::Instruments::default().page(instrument_page.unwrap_or(7), &hornet, &state);
        let mut f = std::fs::File::create(path)?;
        write!(f, "P6\n160 156\n255\n")?;
        for p in r.pixels.chunks_exact(4) {
            f.write_all(&p[..3])?;
        }
        return Ok(());
    }
    let audio = if no_audio
        || smoke_test
        || validate_creator
        || validate_weather
        || snapshot.is_some()
        || std::env::var_os("TORE_ENVIRONMENT_PROBE").is_some()
    {
        None
    } else {
        match audio::Audio::new(std::mem::take(&mut assets.sounds), &assets.music_scores) {
            Ok(audio) => Some(audio),
            Err(error) => {
                eprintln!("Continuing without audio: {error}");
                None
            }
        }
    };
    // Saved previews stay reproducible; normal launches randomly select all five.
    if snapshot.is_some() && background.is_none() {
        background = Some("CHOOSEV".into());
    }
    let mut world =
        terrain::World::for_mission(&assets.theater_resources, &theater_code, weather_condition)?;
    if validate_creator {
        return ordnance::validate_sources(&assets.theater_resources, &world);
    }
    if validate_weather {
        return weather::validate_sources(&assets.theater_resources, &world.environment);
    }
    let theater_resources = assets.theater_resources.clone();
    let creator_options = assets.creator_options.clone();
    let mut menu = Menu::new(assets, background.as_deref())?;
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
            let selection = world
                .catalog
                .iter()
                .position(|(code, _)| code == &theater_code)
                .unwrap_or(0);
            quick.theater(selection);
            if snapshot_state == "ordnance" {
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
            quick.render(&mut menu.pixels, &menu.quick_sprites, &world);
            quick.preview_selector(&snapshot_state)?;
            quick.render(&mut menu.pixels, &menu.quick_sprites, &world);
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
        return Ok(());
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
    quick.theater(selection);
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
            || flight_probe_ticks.is_some());
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
    // The probe advances weather and vapor with the flight so captures taken
    // after it show the same environment and trail history a live run would.
    let mut probe_vapor =
        tore_sim::vapor::Vapor::seeded(hornet.streamer_points(&flight).unwrap_or([[0.; 3]; 2]));
    let mut probe_turbulence = tore_sim::turbulence::Turbulence::default();
    let mut probe_turbulence_rng = tore_formats::flight_model::clock_rng::NativeRng::seeded(1)?;
    if let Some(ticks) = flight_probe_ticks {
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
        return Ok(());
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
    if let Some(path) = record_combat {
        combat.recorder = Some(combat_tape::Recorder::new(
            &path,
            &theater_resources,
            combat.state.configuration(),
            &theater_code,
        )?);
    }
    combat.reset(&mut flight)?;
    if let Some(value) = flight_bay {
        if !flight.bay_available() {
            return Err("selected aircraft has no reviewed weapon bay".into());
        }
        flight.bay = value;
        flight.bay_open = value > 0.;
    }

    if weapon_slot == 0 || weapon_slot > combat.state.ammo.len() {
        return Err("weapon slot outside this aircraft's PT loadout".into());
    }
    for _ in 1..weapon_slot {
        combat.command(
            tore_sim::combat::live::Command::NextWeapon,
            combat::launcher(&flight),
        );
    }
    if live_fire {
        combat.command(
            tore_sim::combat::live::Command::ReplaceTarget,
            combat::launcher(&flight),
        );
        // A scripted designation needs a current observation first, exactly as
        // a player's click does.
        combat.step(&mut flight, &world)?;
    }
    for command in combat_commands {
        combat.command(command, combat::launcher(&flight));
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
    let mut app = App {
        mission: None,
        preference_path: if preferences_enabled {
            Some(assets::data_directory()?.join("preferences-v1.conf"))
        } else {
            None
        },
        preference_saved: String::new(),
        input_recording,
        recorded_ticks: 0,
        input: input::Input::new(input_profile.as_deref(), native_input)?,
        focused: true,
        performance: performance::Performance::from_env()?,
        combat,
        hornet,
        researched_flight,
        native_tables,
        previous_flight: flight.clone(),
        flight,
        flight_clock: flight::Clock { remainder: 0. },
        vapor: probe_vapor,
        turbulence: probe_turbulence,
        turbulence_rng: probe_turbulence_rng,
        flight_view,
        flight_canvas: Default::default(),
        window_size,
        flight_ui: {
            let mut ui = flight_ui::FlightUi::default();
            ui.no_turbulence = !turbulence_enabled;
            ui.menu = flight_menu;
            ui.paused = animation_capture || combat_probe.is_some();
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
        camera,
        quick,
        screen: initial_screen,
        frame_time: Instant::now(),
        instrument_time: Instant::now(),
        menu,
        audio,
        renderer: None,
        modifiers: ModifiersState::empty(),
        smoke_test,
        capture_terrain,
        finished: false,
        next_frame: None,
        error: None,
    };
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
                }
                Err(e) => {
                    eprintln!("Preferences not loaded: {e}; preserving original file");
                    app.preference_path = None;
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!("Preferences not loaded: {e}; preserving original file");
                app.preference_path = None;
            }
        }
    }
    if app.native_tables.is_some() {
        app.flight_ui.no_turbulence = true;
    }
    app.preference_saved =
        preferences::Preferences::capture(&app.flight_ui, &app.instruments, &app.menu.state).text();
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
    EventLoop::new()?.run_app(&mut app)?;
    match app.error {
        Some(error) => Err(error),
        None => Ok(()),
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
            _ => fallback,
        }
        .into();
    }
    fallback.into()
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use winit::keyboard::{KeyCode, PhysicalKey};
    #[test]
    fn physical_keys_survive_shift_and_option_translations() {
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::Digit1), "!"), "1");
        assert_eq!(
            flight_key(PhysicalKey::Code(KeyCode::BracketLeft), "{"),
            "["
        );
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::KeyE), "é"), "e");
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::Equal), "+"), "=");
        assert_eq!(flight_key(PhysicalKey::Code(KeyCode::Slash), "?"), "/");
    }
}
