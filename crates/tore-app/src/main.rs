mod assets;
mod audio;
mod menu;
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
}
struct App {
    world: terrain::World,
    theater_resources: std::collections::BTreeMap<String, Vec<u8>>,
    camera: terrain::Camera,
    quick: quick_mission::QuickMission,
    screen: Screen,
    frame_time: Instant,
    menu: Menu,
    audio: Option<audio::Audio>,
    renderer: Option<Renderer>,
    modifiers: ModifiersState,
    smoke_test: bool,
    capture_terrain: Option<PathBuf>,
    finished: bool,
    next_frame: Option<Instant>,
    error: Option<Box<dyn Error>>,
}
impl App {
    fn action(&mut self, event_loop: &ActiveEventLoop, action: Action) {
        if action == Action::Exit {
            self.finished = true;
            event_loop.exit();
            return;
        }
        match action {
            Action::Theater(index) => {
                if let Some((code, _)) = self.world.catalog.get(index) {
                    match terrain::World::for_theater(&self.theater_resources, code) {
                        Ok(world) => {
                            if let Some(renderer) = &mut self.renderer {
                                renderer.set_world(&world);
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
            Action::Viewer => {
                self.screen = Screen::Viewer;
                self.camera.keys.clear();
                self.quick.cancel();
                self.frame_time = Instant::now();
            }
            Action::Back => {
                self.screen = if self.screen == Screen::Viewer {
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
                            Screen::Main => "T.O.R.E-Fighters - Choose Activity".to_string(),
                            Screen::Quick => "T.O.R.E-Fighters - Quick Mission Creator".to_string(),
                            Screen::Viewer => format!(
                                "T.O.R.E-Fighters - {} Terrain Viewer",
                                self.world.theater.name
                            ),
                        })
                        .with_inner_size(LogicalSize::new(960.0, 720.0))
                        .with_min_inner_size(LogicalSize::new(640.0, 480.0)),
                )?,
            );
            pollster::block_on(Renderer::new(window, &self.world))
        })();
        match result {
            Ok(renderer) => {
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
                self.camera.keys.clear();
                self.modifiers = ModifiersState::empty();
                Action::None
            }
            WindowEvent::CursorMoved { position, .. } => {
                let point = renderer.viewport().point(position.x, position.y);
                match self.screen {
                    Screen::Main => self.menu.state.pointer(point),
                    Screen::Quick => {
                        self.quick.pointer(point);
                        Action::None
                    }
                    Screen::Viewer => Action::None,
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.quick.pointer(None);
                self.menu.state.pointer(None)
            }
            WindowEvent::Focused(false) => {
                self.menu.state.cancel();
                self.quick.cancel();
                self.camera.keys.clear();
                self.modifiers = ModifiersState::empty();
                Action::None
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if self.screen == Screen::Viewer {
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
                self.modifiers = modifiers.state();
                Action::None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let name = match &event.logical_key {
                    Key::Named(k) => format!("{k:?}"),
                    Key::Character(c) => c.to_ascii_lowercase(),
                    _ => String::new(),
                };
                if self.screen == Screen::Viewer && event.state == ElementState::Released {
                    self.camera.keys.remove(&name);
                    return;
                }
                if event.state != ElementState::Pressed {
                    return;
                }
                if (self.modifiers.super_key() && name.eq_ignore_ascii_case("q"))
                    || (self.modifiers.alt_key() && name == "F4")
                {
                    Action::Exit
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
                let animating = match self.screen {
                    Screen::Main => self.menu.render(),
                    Screen::Quick => {
                        self.quick.render(
                            &mut self.menu.pixels,
                            &self.menu.quick_sprites,
                            &self.world,
                        );
                        false
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
                match renderer.draw(
                    &self.menu.pixels,
                    (self.screen == Screen::Viewer).then_some((&self.camera, &self.world)),
                ) {
                    Ok(true) if self.smoke_test => {
                        if let Some(path) = &self.capture_terrain
                            && let Err(error) =
                                renderer.capture_sim(path, &self.camera, &self.world)
                        {
                            self.error = Some(error);
                        }

                        println!("Smoke test: requested screen presented successfully");
                        self.finished = true;
                        event_loop.exit();
                    }
                    Ok(_) => {}
                    Err(error) => {
                        self.error = Some(error);
                        event_loop.exit();
                    }
                }
                self.next_frame = animating.then(|| Instant::now() + Duration::from_millis(16));
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
    let mut capture_terrain = None;
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
                    "Usage: tore-app [--viewer | --quick-mission] [--theater CODE] [--capture-terrain OUTPUT.ppm] [--import MEDIA_DIR] [--import-only] [--no-audio] [--smoke-test] [--snapshot OUTPUT.ppm] [--snapshot-state STATE] [--background NAME]\n\nImports original menu and Ukraine theater assets into platform application data.\nA local gameassets/fighters-anthology directory is imported automatically on first run.\n--quick-mission opens the creator; --viewer opens the selected theater.\n--theater CODE selects one of the 16 original theater codes (default UKR).\n--capture-terrain writes a GPU-rendered 960x720 terrain PPM and exits (display required).\nViewer: arrows move; Shift speeds up; Q/E or PageDown/PageUp change altitude; A/D turn; W/S pitch; Escape returns.\n--snapshot writes a headless 640x480 menu preview and exits (supports --quick-mission).\n--snapshot-state: normal, hover, pressed, help, pref, multi, notice.\n--background: CHOOSEAC, CHOOSE3, CHOOSEU, CHOOSEM, CHOOSEV (default: random; snapshots use CHOOSEV).\n--smoke-test presents one frame without audio and exits.\nTORE_DATA_DIR overrides the application data directory.\nTab/arrows + Enter navigate; Escape dismisses; M toggles music; ? contains Exit."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if capture_terrain.is_some()
        && (snapshot.is_some() || import_only || initial_screen != Screen::Viewer)
    {
        return Err("--capture-terrain requires the viewer and cannot combine with --snapshot or --import-only".into());
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
        if initial_screen == Screen::Viewer {
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
    let mut app = App {
        theater_resources,
        world,
        camera,
        quick,
        screen: initial_screen,
        frame_time: Instant::now(),
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
