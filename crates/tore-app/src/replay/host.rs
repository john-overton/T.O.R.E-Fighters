//! The replay viewer inside the app: opening it from an action or the
//! command line, routing window events to it, drawing it each frame, and
//! handing the renderer back to the game when it closes. Kept apart from
//! `main.rs` so the screen's plumbing lives in one place.
use crate::replay::viewer::{Command, Options, Viewer};
use crate::{App, Screen, menu::Action, renderer::Renderer};
use std::path::{Path, PathBuf};
use std::time::Instant;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, PhysicalKey};

/// A `--capture-replay` request: the file to write. A PPM shows the
/// interface as the viewer shows it, so a hidden interface leaves only the
/// 3D view; a PNG is the clean view P saves, through the same writer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub path: PathBuf,
}

/// The open viewer, and a capture to take once its first frame shows.
pub struct Replay {
    pub viewer: Viewer,
    pub capture: Option<Capture>,
}

/// The name the viewer reads for a key: letters by their physical key in
/// lower case, `` ` `` for the key left of 1, otherwise as the app names
/// keys.
fn key_name(event: &KeyEvent) -> String {
    if event.physical_key == PhysicalKey::Code(KeyCode::Backquote) {
        return "`".into();
    }
    let name = match &event.logical_key {
        Key::Named(key) => format!("{key:?}"),
        Key::Character(c) => c.to_ascii_lowercase(),
        _ => String::new(),
    };
    crate::flight_key(event.physical_key, &name)
}

/// A window point in the view's own pixels, and the view's size.
fn view_point(renderer: &Renderer, [x, y]: [f64; 2]) -> ([f64; 2], [u32; 2]) {
    let window = renderer.window.inner_size();
    let size = renderer.flight_size();
    (
        [
            x * f64::from(size[0]) / f64::from(window.width.max(1)),
            y * f64::from(size[1]) / f64::from(window.height.max(1)),
        ],
        size,
    )
}

impl App {
    /// `Action::WatchReplay`: opens the viewer, or explains on the menu why
    /// it could not.
    pub(crate) fn watch_replay(&mut self, path: &Path) {
        match Viewer::open(path, &self.theater_resources, &Options::default()) {
            Ok(viewer) => self.start_replay(viewer, None),
            Err(error) => {
                log::warn!("Replay {}: {error}", path.display());
                self.menu.state.toast = Some((
                    format!("Could not open the replay: {error}"),
                    Instant::now(),
                ));
            }
        }
    }

    /// Shows `viewer`. The renderer switches to its world on the first
    /// frame drawn.
    pub(crate) fn start_replay(&mut self, viewer: Viewer, capture: Option<Capture>) {
        self.replay = Some(Box::new(Replay { viewer, capture }));
        self.screen = Screen::Replay;
        self.mouse_look = None;
        self.menu.state.cancel();
        // Replay sound comes later; until then the replay is silent.
        if let Some(audio) = &self.audio {
            audio.restart_flight();
            audio.flight(None);
        }
        if let (Some(renderer), Some(replay)) = (&self.renderer, &self.replay) {
            renderer.window.set_title(&replay.viewer.title());
            renderer.window.request_redraw();
        }
    }

    /// Closes the viewer and gives the renderer back the game's own world
    /// and aircraft.
    pub(crate) fn leave_replay(&mut self) {
        let entered = self.replay.take().is_some_and(|r| r.viewer.entered());
        if let Some(renderer) = &mut self.renderer {
            if entered {
                renderer.set_world(&self.world);
                renderer.prepare_aircraft(&self.hornet);
            }
            renderer.window.set_cursor_visible(true);
        }
        self.screen = Screen::Main;
        // Back to the recordings list, re-read, even when the viewer was
        // started from the command line.
        self.return_to_replays();
    }

    fn replay_command(&mut self, event_loop: &ActiveEventLoop, command: Command) {
        match command {
            Command::None => {}
            Command::Leave => {
                self.leave_replay();
                self.action(event_loop, Action::Click);
            }
            Command::Screenshot => {
                let (Some(replay), Some(renderer)) = (self.replay.as_mut(), self.renderer.as_mut())
                else {
                    return;
                };
                let saved = crate::assets::data_directory().and_then(|folder| {
                    replay
                        .viewer
                        .screenshot(renderer, &folder.join("screenshots"))
                });
                match saved {
                    Ok(path) => log::info!("Replay screenshot: {}", path.display()),
                    Err(error) => replay.viewer.message(format!("Screenshot failed: {error}")),
                }
            }
        }
    }

    /// Handles a window event while the viewer is open. Events the app
    /// still owns (resizing, focus, Alt-Enter, quitting) come back to it.
    pub(crate) fn replay_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> Option<WindowEvent> {
        if self.replay.is_none() {
            return Some(event);
        }
        match &event {
            WindowEvent::RedrawRequested => {
                self.replay_redraw(event_loop);
                None
            }
            WindowEvent::KeyboardInput { event: key, .. } => {
                let name = key_name(key);
                let pressed = key.state == ElementState::Pressed;
                let (alt, command, control) = (
                    self.modifiers.alt_key(),
                    self.modifiers.super_key(),
                    self.modifiers.control_key(),
                );
                if pressed
                    && ((alt && matches!(name.as_str(), "Enter" | "F4"))
                        || (command && name == "q"))
                {
                    return Some(event);
                }
                // The viewer's keys take no Control, Alt or Command.
                if pressed && (alt || command || control) {
                    return None;
                }
                let result = self.replay.as_mut().map_or(Command::None, |replay| {
                    replay
                        .viewer
                        .key(&name, pressed, key.repeat, self.modifiers.shift_key())
                });
                self.replay_command(event_loop, result);
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = Some((position.x, position.y));
                let window = [position.x, position.y];
                if let (Some(replay), Some(renderer)) =
                    (self.replay.as_mut(), self.renderer.as_ref())
                {
                    let (point, size) = view_point(renderer, window);
                    let profile = &self.input.resolver.profile;
                    // The same turn per pixel as mouse look in flight.
                    let k = 0.002 * profile.mouse_sensitivity;
                    let up = if profile.mouse_invert { -k } else { k };
                    replay.viewer.pointer(Some(point), window, size, [k, up]);
                }
                None
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer = None;
                if let Some(replay) = self.replay.as_mut() {
                    replay.viewer.pointer_left();
                }
                None
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = *state == ElementState::Pressed;
                let window = self.pointer.map(|(x, y)| [x, y]);
                if let (Some(replay), Some(renderer)) =
                    (self.replay.as_mut(), self.renderer.as_ref())
                {
                    match button {
                        MouseButton::Left => {
                            let (point, size) = window
                                .map(|w| view_point(renderer, w))
                                .map_or((None, renderer.flight_size()), |(p, s)| (Some(p), s));
                            replay.viewer.left(pressed, point, size);
                        }
                        MouseButton::Right => {
                            if let Some(window) = window {
                                replay.viewer.right(pressed, window);
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.wheel += match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(*y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.,
                };
                let notches = self.wheel.trunc() as i32;
                self.wheel -= f64::from(notches);
                if notches != 0
                    && let Some(replay) = self.replay.as_mut()
                {
                    replay.viewer.wheel(notches);
                }
                None
            }
            WindowEvent::Focused(false)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(replay) = self.replay.as_mut() {
                    replay.viewer.release();
                }
                Some(event)
            }
            _ => Some(event),
        }
    }

    /// Draws one replay frame and, for `--capture-replay`, saves it and
    /// exits once it has been presented.
    fn replay_redraw(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(replay), Some(renderer)) = (self.replay.as_mut(), self.renderer.as_mut()) else {
            return;
        };
        let Replay { viewer, capture } = &mut **replay;
        let start = Instant::now();
        let result = viewer.frame(
            renderer,
            &mut self.flight_canvas,
            self.modifiers.shift_key(),
        );
        renderer.window.set_cursor_visible(viewer.pointer_visible());
        match result {
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
                return;
            }
            Ok(timing) if timing.presented => {
                if self.performance.record(
                    start,
                    timing.scene,
                    timing.interface,
                    timing.present,
                    viewer.clock.paused(),
                ) {
                    self.finished = true;
                    event_loop.exit();
                    return;
                }
                if let Some(request) = capture.take() {
                    let png = request
                        .path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
                    let written = if png {
                        viewer.save_png(renderer, &request.path).map(|()| {
                            println!("Replay screenshot: {}", request.path.display());
                        })
                    } else {
                        renderer.capture_sim(&request.path, viewer.camera(), &viewer.world, true)
                    };
                    if let Err(error) = written {
                        self.error = Some(error);
                    }
                    self.finished = true;
                    event_loop.exit();
                    return;
                }
            }
            Ok(_) => {}
        }
        // Playback is paced by presentation, like flight.
        self.next_frame = Some(Instant::now());
    }
}
