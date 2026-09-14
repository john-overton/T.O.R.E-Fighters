mod aircraft;
mod assets;
mod attitude;
mod audio;
mod cockpit_renderer;
mod flight;
mod flight_canvas;
mod flight_ui;
mod hud;
mod instruments;
mod look;
mod menu;
mod performance;
mod quick_mission;
mod renderer;
mod sim_renderer;
mod terrain;

use assets::Assets;
use menu::{Action, Menu};
use renderer::Renderer;
use std::{
    error::Error,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
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
    performance: performance::Performance,
    hornet: aircraft::Hornet,
    flight: flight::State,
    previous_flight: flight::State,
    flight_clock: flight::Clock,
    flight_view: u8,
    flight_canvas: flight_canvas::FlightCanvas,
    window_size: [u32; 2],
    flight_ui: flight_ui::FlightUi,
    instruments: instruments::Instruments,
    world: terrain::World,
    theater_resources: std::collections::BTreeMap<String, Vec<u8>>,
    camera: terrain::Camera,
    quick: quick_mission::QuickMission,
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
impl App {
    fn flight_command(&mut self, command: flight_ui::Command) -> Action {
        use flight_ui::Command;
        match command {
            Command::None => Action::None,
            Command::Click => Action::Click,
            Command::End => Action::Back,
            Command::Exit => Action::Exit,
            Command::Restart => Action::FreeFlight,
            Command::Effects(on) => Action::Effects(on),
            Command::Toggle(key) => {
                self.flight.toggle(key);
                if let Some(audio) = &self.audio {
                    audio.control(key, &self.flight);
                }
                Action::None
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
                self.instruments.pressed = None;
                self.instruments.toggle(page);
                Action::Click
            }
            Command::Throttle(value) => {
                self.flight.throttle = value;
                Action::None
            }
            Command::Range(delta) => {
                if self.instruments.pages.last() == Some(&5) {
                    self.instruments.rwr_range =
                        (self.instruments.rwr_range as i32 + delta).clamp(0, 4) as usize;
                } else {
                    self.instruments.radar_range =
                        (self.instruments.radar_range as i32 + delta).clamp(0, 4) as usize;
                }
                Action::Click
            }
            Command::Mode => {
                self.instruments.mode = (self.instruments.mode + 1) % 3;
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
            Action::Effects(on) => {
                self.menu.state.effects = on;
                self.flight_ui.effects = on;
            }
            Action::Theater(index) => {
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
            Action::QuickMission => {
                self.screen = Screen::Quick;
                self.menu.state.cancel();
            }
            Action::FreeFlight => {
                self.flight = self.hornet.start(&self.world);
                self.previous_flight = self.flight.clone();
                self.flight_clock.remainder = 0.;
                self.flight_view = 0;
                self.flight_ui = flight_ui::FlightUi::default();
                self.flight_ui.effects = self.menu.state.effects;
                self.screen = Screen::Flight;
                self.camera.keys.clear();
                self.quick.cancel();
                self.frame_time = Instant::now();
            }
            Action::Back => {
                self.screen = if matches!(self.screen, Screen::Viewer | Screen::Flight) {
                    Screen::Quick
                } else {
                    Screen::Main
                };
                self.camera.keys.clear();
                self.quick.cancel();
            }
            _ => {}
        }
        if let Some(renderer) = &self.renderer {
            renderer.window.set_title(&match self.screen {
                Screen::Flight => "T.O.R.E-Fighters - F/A-18D Free Flight".to_string(),
                Screen::Main => "T.O.R.E-Fighters - Choose Activity".to_string(),
                Screen::Quick => "T.O.R.E-Fighters - Quick Mission Creator".to_string(),
                Screen::Viewer => format!(
                    "T.O.R.E-Fighters - {} Terrain Viewer",
                    self.world.theater.name
                ),
            });
        }
        if let Some(audio) = &self.audio {
            audio.action(action);
        }
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
                            Screen::Flight => "T.O.R.E-Fighters - F/A-18D Free Flight".to_string(),
                            Screen::Main => "T.O.R.E-Fighters - Choose Activity".to_string(),
                            Screen::Quick => "T.O.R.E-Fighters - Quick Mission Creator".to_string(),
                            Screen::Viewer => format!(
                                "T.O.R.E-Fighters - {} Terrain Viewer",
                                self.world.theater.name
                            ),
                        })
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
                self.instruments.pressed = None;
                self.flight_ui.cancel_press();
                self.pointer = None;
                self.camera.keys.clear();
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
            WindowEvent::Focused(false) => {
                if self.screen == Screen::Flight {
                    self.flight_ui.paused = true;
                }
                self.menu.state.cancel();
                self.quick.cancel();
                self.instruments.pressed = None;
                self.flight_ui.cancel_press();
                self.pointer = None;
                self.camera.keys.clear();
                self.modifiers = ModifiersState::empty();
                Action::None
            }
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
                    self.frame_time = Instant::now();
                    self.flight_command(command)
                } else if self.screen == Screen::Flight {
                    if self.instruments.screen_pointer(
                        self.pointer,
                        [
                            renderer.window.inner_size().width as f64,
                            renderer.window.inner_size().height as f64,
                        ],
                        state == ElementState::Pressed,
                    ) {
                        Action::Click
                    } else {
                        Action::None
                    }
                } else if self.screen == Screen::Viewer {
                    Action::None
                } else if self.screen == Screen::Quick {
                    if state == ElementState::Pressed {
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
                        self.instruments.pressed = None;
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
                        let now = Instant::now();
                        let elapsed = (now - self.frame_time).as_secs_f64().min(0.25);
                        let steps = self.flight_ui.steps(&mut self.flight_clock, elapsed);
                        self.frame_time = now;
                        for _ in 0..steps {
                            self.previous_flight.clone_from(&self.flight);
                            self.flight
                                .step(&self.hornet.profile, &self.camera.keys, |x, z| {
                                    self.world.height(x as f32, z as f32) as f64
                                });
                        }
                        if !self.flight_ui.frozen() {
                            look::step(
                                &mut self.flight_ui.look,
                                &self.camera.keys,
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
                        if now.duration_since(self.instrument_time).as_millis() >= 100
                            || self.smoke_test
                        {
                            self.instrument_time = now;
                            for page in [2, 3] {
                                if self.instruments.pages.contains(&page) {
                                    let mut camera = self.hornet.camera(
                                        &presented,
                                        if page == 2 { 0 } else { 2 },
                                        Default::default(),
                                    );
                                    camera.view_fraction = 1.;
                                    if page == 3 {
                                        for i in 0..3 {
                                            camera.position[i] = presented.position[i] as f32
                                                + (camera.position[i]
                                                    - presented.position[i] as f32)
                                                    * 0.5;
                                        }
                                        camera.pitch = -(30f32 / 65.).atan();
                                    }
                                    renderer.aircraft(&self.hornet, &presented, page == 3, &camera);
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
                        );
                        simulation_ms = frame_start.elapsed().as_secs_f64() * 1000.;
                        self.flight_canvas.begin(
                            renderer.flight_size(),
                            &self.hornet,
                            &presented,
                            &self.instruments,
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
                                self.flight_ui.ladder,
                                self.flight_ui.brightness,
                                self.flight_canvas.hud_zoom(self.flight_ui.zoom),
                            );
                        }
                        renderer.cockpit(
                            &presented,
                            &self.camera,
                            self.flight_ui.cockpit && matches!(self.flight_view, 0 | 3 | 4),
                            self.flight_ui.hud && matches!(self.flight_view, 0 | 3 | 4),
                            &self.menu.pixels,
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
                            audio.flight(Some((&self.hornet.profile, &self.flight)));
                        }
                        true
                    }
                    Screen::Viewer => {
                        let now = Instant::now();
                        self.camera.step(
                            (now - self.frame_time).as_secs_f32(),
                            self.modifiers.shift_key(),
                            &self.world,
                        );
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
                    renderer.aircraft(&self.hornet, &self.flight, false, &self.camera);
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
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(next) = self.next_frame {
            if Instant::now() >= next {
                if let Some(renderer) = &self.renderer {
                    renderer.window.request_redraw();
                }
                self.next_frame = None;
                event_loop.set_control_flow(ControlFlow::Wait);
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(next));
            }
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
fn main() -> AppResult<()> {
    let mut args = std::env::args().skip(1);
    let (mut import, mut snapshot) = (None, None);
    let mut snapshot_state = String::from("normal");
    let mut background = None;
    let mut theater_code = String::from("UKR");
    let mut initial_screen = Screen::Main;
    let mut flight_view = 0;
    let mut flight_look = [0f32; 2];
    let mut flight_menu = false;
    let mut window_size = [960, 720];
    let mut instrument_page = None;
    let mut instrument_layout = instruments::Layout::Large;
    let mut capture_terrain = None;
    let mut headless_ticks = None;
    let mut maneuver = String::from("level");
    let mut panel_snapshot = None;
    let (mut smoke_test, mut no_audio, mut import_only) = (false, false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--capture-terrain" => {
                capture_terrain = Some(PathBuf::from(
                    args.next().ok_or("--capture-terrain needs a .ppm path")?,
                ));
                initial_screen = Screen::Viewer;
                smoke_test = true;
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
            "--flight-menu" => {
                flight_menu = true;
                initial_screen = Screen::Flight;
            }
            "--free-flight" => initial_screen = Screen::Flight,
            "--maneuver" => {
                maneuver = args
                    .next()
                    .ok_or("--maneuver needs level, pull, loop, roll or stall")?;
                if !["level", "pull", "loop", "roll", "stall"].contains(&maneuver.as_str()) {
                    return Err("unsupported maneuver".into());
                }
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
            "--help" | "-h" => {
                println!(
                    "Usage: tore-app [--free-flight | --viewer | --quick-mission] [--theater CODE] [--capture-terrain OUTPUT.ppm] [--import MEDIA_DIR] [--import-only] [--no-audio] [--smoke-test] [--snapshot OUTPUT.ppm] [--snapshot-state STATE] [--background NAME]\n\nImports original menus, all theaters and F/A-18D assets into platform application data.\nA local gameassets/fighters-anthology directory is imported automatically on first run.\n--free-flight launches the Hornet; --headless-flight TICKS runs without a display.\nFlight: Shift/Ctrl-arrows look/orbit, Shift-/ recenter. Arrows pitch/bank, Z/X rudder, PageUp/Down throttle, Shift-B burner. F1 front, F2 back, F3 up, F10 external. Shift-0..9 instruments. Esc > Pref > Large windows? switches four-corner/six-bottom layouts. Esc flight menu, Ctrl-P pause, Backspace cockpit, F11 keyboard help. See docs/FLIGHT-CONTROLS.md.\n--quick-mission opens the creator; --viewer opens the selected theater.\n--theater CODE selects one of the 16 original theater codes (default UKR).\n--capture-flight PATH captures flight with instruments; --flight-view 0/1/2/3/4 chooses cockpit/chase/oblique/back/up. --flight-menu captures the paused menu. --flight-look YAW,PITCH sets look angles in degrees for inspection.\n--instrument-layout large/small selects four corners or six bottom windows.\n--panel-snapshot PATH writes one instrument; --instrument-page 0..9 selects it.\n--headless-flight TICKS supports --maneuver level/pull/loop/roll/stall.\n--capture-terrain writes a GPU-rendered 960x720 terrain PPM and exits (display required).\nViewer: arrows move; Shift speeds up; Q/E or PageDown/PageUp change altitude; A/D turn; W/S pitch; Escape returns.\n--snapshot writes a headless 640x480 menu preview and exits (supports --quick-mission).\n--snapshot-state: normal, hover, pressed, help, pref, multi, notice.\n--background: CHOOSEAC, CHOOSE3, CHOOSEU, CHOOSEM, CHOOSEV (default: random; snapshots use CHOOSEV).\n--smoke-test presents one frame without audio and exits.\nTORE_DATA_DIR overrides the application data directory.\nTab/arrows + Enter navigate; Escape dismisses; M toggles music; ? contains Exit."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if capture_terrain.is_some()
        && (snapshot.is_some()
            || import_only
            || !matches!(initial_screen, Screen::Viewer | Screen::Flight))
    {
        return Err("scene capture requires flight/viewer and cannot combine with --snapshot or --import-only".into());
    }
    if snapshot.is_none() && snapshot_state != "normal" {
        return Err("--snapshot-state requires --snapshot".into());
    }
    let data = assets::data_directory()?;
    let assets = if let Some(source) = import {
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
    let hornet = aircraft::Hornet::load(&assets.theater_resources)?;
    if let Some(ticks) = headless_ticks {
        if ticks > 120 * 3600 {
            return Err("headless flight limited to one hour".into());
        }
        let mut state = flight::State::new(&hornet.profile, [0., 5000., 0.]);
        let mut keys = std::collections::BTreeSet::new();
        match maneuver.as_str() {
            "pull" | "loop" => {
                keys.insert("ArrowDown".to_string());
                if maneuver == "loop" {
                    state.throttle = 1.;
                    state.burner = true;
                }
            }
            "roll" => {
                keys.insert("ArrowRight".to_string());
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
        let initial_forward = attitude::Basis::new(state.yaw, state.pitch, state.bank).forward;
        let (mut vertical, mut inverted, mut completed) = (false, false, false);
        for _ in 0..ticks {
            state.step(&hornet.profile, &keys, |_, _| 0.);
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
        println!("vertical={vertical} inverted={inverted} loop_completed={completed}");

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
        let state = flight::State::new(&hornet.profile, [0., 5000., 0.]);
        let r =
            instruments::Instruments::default().page(instrument_page.unwrap_or(7), &hornet, &state);
        let mut f = std::fs::File::create(path)?;
        write!(f, "P6\n160 156\n255\n")?;
        for p in r.pixels.chunks_exact(4) {
            f.write_all(&p[..3])?;
        }
        return Ok(());
    }
    let audio = if no_audio || smoke_test || snapshot.is_some() {
        None
    } else {
        match audio::Audio::new(&assets.sounds) {
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
    let world = terrain::World::for_theater(&assets.theater_resources, &theater_code)?;
    let theater_resources = assets.theater_resources.clone();
    let mut menu = Menu::new(assets, background.as_deref())?;
    if let Some(path) = snapshot {
        if matches!(initial_screen, Screen::Viewer | Screen::Flight) {
            return Err(
                "Use --capture-terrain for a GPU terrain capture; CPU snapshots support menus only"
                    .into(),
            );
        }
        if initial_screen == Screen::Quick {
            quick_mission::QuickMission::default().render(
                &mut menu.pixels,
                &menu.quick_sprites,
                &world,
            );
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
    let camera = terrain::Camera::for_world(&world);
    let selection = world
        .catalog
        .iter()
        .position(|(code, _)| code == &theater_code)
        .unwrap_or(0);
    let mut quick = quick_mission::QuickMission::default();
    quick.selection = selection;
    let flight = hornet.start(&world);
    let mut app = App {
        performance: performance::Performance::from_env()?,
        hornet,
        previous_flight: flight.clone(),
        flight,
        flight_clock: flight::Clock { remainder: 0. },
        flight_view,
        flight_canvas: Default::default(),
        window_size,
        flight_ui: {
            let mut ui = flight_ui::FlightUi::default();
            ui.menu = flight_menu;
            ui.look = flight_look.map(f32::to_radians);
            if !matches!(flight_view, 1 | 2) {
                ui.look[1] = ui.look[1].clamp(0., std::f32::consts::FRAC_PI_2);
            }
            ui
        },
        instruments: instruments::Instruments::new(instrument_layout, instrument_page),
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
    EventLoop::new()?.run_app(&mut app)?;
    match app.error {
        Some(error) => Err(error),
        None => Ok(()),
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
