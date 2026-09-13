mod assets;
mod audio;
mod menu;
mod renderer;

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
struct App {
    menu: Menu,
    audio: Option<audio::Audio>,
    renderer: Option<Renderer>,
    modifiers: ModifiersState,
    smoke_test: bool,
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
        if let Some(audio) = &self.audio {
            audio.action(action);
        }
        if let Some(renderer) = &self.renderer {
            renderer
                .window
                .set_cursor(if self.menu.state.hover.is_some() {
                    CursorIcon::Pointer
                } else {
                    CursorIcon::Default
                });
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
                        .with_title("T.O.R.E-Fighters — Choose Activity")
                        .with_inner_size(LogicalSize::new(960.0, 720.0))
                        .with_min_inner_size(LogicalSize::new(640.0, 480.0)),
                )?,
            );
            pollster::block_on(Renderer::new(window))
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
                Action::None
            }
            WindowEvent::CursorMoved { position, .. } => self
                .menu
                .state
                .pointer(renderer.viewport().point(position.x, position.y)),
            WindowEvent::CursorLeft { .. } => self.menu.state.pointer(None),
            WindowEvent::Focused(false) => {
                self.menu.state.cancel();
                Action::None
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if state == ElementState::Pressed {
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                let name = match &event.logical_key {
                    Key::Named(k) => format!("{k:?}"),
                    Key::Character(c) => c.to_string(),
                    _ => String::new(),
                };
                if (self.modifiers.super_key() && name.eq_ignore_ascii_case("q"))
                    || (self.modifiers.alt_key() && name == "F4")
                {
                    Action::Exit
                } else {
                    self.menu.state.key(
                        if name == "Space" { " " } else { &name },
                        self.modifiers.shift_key(),
                    )
                }
            }
            WindowEvent::RedrawRequested => {
                let animating = self.menu.render();
                match renderer.draw(&self.menu.pixels) {
                    Ok(true) if self.smoke_test => {
                        println!("Smoke test: main menu presented successfully");
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
    let (mut smoke_test, mut no_audio, mut import_only) = (false, false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
                    "Usage: tore-app [--import MEDIA_DIR] [--import-only] [--no-audio] [--smoke-test] [--snapshot OUTPUT.ppm] [--snapshot-state STATE]\n\nImports original menu assets into platform application data.\nA local gameassets/fighters-anthology directory is imported automatically on first run.\n--snapshot writes a headless 640x480 menu preview and exits.\n--snapshot-state: normal, hover, pressed, help, pref, multi.\n--smoke-test presents one frame without audio and exits.\nTORE_DATA_DIR overrides the application data directory.\nTab/arrows + Enter navigate; Escape dismisses; M toggles music; ? contains Exit."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
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
    let mut menu = Menu::new(assets);
    if let Some(path) = snapshot {
        menu.preview_state(&snapshot_state)?;
        menu.save_ppm(&path)?;
        println!("Menu preview: {}", path.display());
        return Ok(());
    }
    let mut app = App {
        menu,
        audio,
        renderer: None,
        modifiers: ModifiersState::empty(),
        smoke_test,
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
