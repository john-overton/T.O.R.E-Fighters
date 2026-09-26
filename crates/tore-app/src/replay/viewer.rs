//! The mission replay viewer: plays a recording back from any viewpoint.
//! Opinionated addition requested by John on 2026-09-26; see
//! docs/REPLAYS.md. The picture comes from the same drawing helpers and the
//! same renderer calls as live flight, fed from the recording; every
//! interface choice here is an agent design (2026-09-26).
//!
//! The viewer owns its own world and aircraft models. Entering it points
//! the renderer at them; leaving hands the renderer back to the game's.
use crate::aircraft::Airframe;
use crate::flight;
use crate::flight_canvas::FlightCanvas;
use crate::flight_views::{self, Body, Reference, Rig, Scene, Shot};
use crate::render_snapshot::{self, AircraftPose, CombatArt, RenderSnapshot};
use crate::renderer::Renderer;
use crate::replay::clock::{self, Clock, Direction};
use crate::replay::drone::{Drone, Mode};
use crate::replay::overlay::{self, Control, Marker, MarkerKind, Model, Placement};
use crate::replay::playback::Playback;
use crate::replay::tracks::{Scanner, Tracks};
use crate::replay::trails;
use crate::replay::weather::WeatherTrack;
use crate::terrain::{Camera, World};
use crate::{AppResult, attitude::Basis};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tore_formats::aircraft::AircraftId;
use tore_replay::{AircraftInfo, Recording, Side, TimedEvent, vocab};

/// Weather snapshots built ahead of the playhead each frame, in ticks:
/// a ten-minute recording is ready in about a second.
const WEATHER_BUDGET: u64 = 1_200;
/// How long a subtitle stays up, in ticks: about four seconds.
const SUBTITLE_TICKS: u64 = 480;
/// Lines the Comms list shows.
const COMMS_LINES: usize = 14;
/// Messages at the top of the view last this long.
const TOAST: Duration = Duration::from_secs(3);
/// Labels further away than this are left out, feet (100 nautical miles).
const LABEL_REACH: f64 = 607_600.;
/// The view the viewer opens in: F10, outside the aircraft.
const EXTERNAL: u8 = 1;

/// What the viewer shows besides the 3D view. Every part can be switched
/// on its own; `hidden` hides all of them at once, trails excepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ui {
    pub hidden: bool,
    pub labels: bool,
    pub timer: bool,
    pub comms: bool,
    pub subtitles: bool,
    pub trails: bool,
    /// Index into [`trails::LENGTHS`].
    pub trail_length: usize,
}

impl Ui {
    /// The interface parts named in a comma-separated list (labels, timer,
    /// trails, comms, subtitles), all others off; `none` for none.
    pub fn parse(list: &str) -> Result<Self, String> {
        let mut ui = Self {
            labels: false,
            timer: false,
            subtitles: false,
            ..Self::default()
        };
        for part in list.split(',').map(str::trim) {
            match part {
                "labels" => ui.labels = true,
                "timer" => ui.timer = true,
                "trails" => ui.trails = true,
                "comms" => ui.comms = true,
                "subtitles" => ui.subtitles = true,
                "none" | "" => {}
                other => {
                    return Err(format!(
                        "unknown replay interface part {other:?}: use labels, timer, trails, comms or subtitles"
                    ));
                }
            }
        }
        Ok(ui)
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            hidden: false,
            labels: true,
            timer: true,
            comms: false,
            subtitles: true,
            trails: false,
            trail_length: trails::DEFAULT_LENGTH,
        }
    }
}

/// How the viewer starts, from the command line or the Replays screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    /// Start paused at this tick.
    pub tick: Option<u64>,
    /// Flight view, 0 to 11 as `--flight-view` numbers them.
    pub view: Option<u8>,
    /// Aircraft to select.
    pub aircraft: Option<u32>,
    /// Start in the follow drone.
    pub drone: bool,
    /// Start playing at this speed, backwards when negative.
    pub speed: Option<f64>,
    pub ui: Ui,
}

/// How long one frame's parts took, in milliseconds, for the
/// `TORE_PERF_FRAMES` measurement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timing {
    /// The frame reached the screen.
    pub presented: bool,
    /// Rebuilding the moment and preparing the 3D view.
    pub scene: f64,
    /// Labels and the interface.
    pub interface: f64,
    /// Handing the frame to the GPU.
    pub present: f64,
}

/// What the app should do after a key or click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    None,
    /// Back to the menu.
    Leave,
    /// Save the frame as a PNG.
    Screenshot,
}

/// Names of the flight views by `--flight-view` number, with their keys.
fn view_name(view: u8) -> &'static str {
    match view {
        0 => "F1 Front",
        1 => "F10 External",
        2 => "Oblique",
        3 => "F2 Back",
        4 => "F3 Up",
        flight_views::TRACK => "F4 Track",
        flight_views::THREAT => "F5 Threat",
        flight_views::WING => "F6 Wing",
        flight_views::TARGET => "F7 Target",
        flight_views::TARGET_PLAYER => "F8 Target view",
        flight_views::FLY_BY => "F9 Fly-by",
        flight_views::MISSILE => "F12 Missile",
        _ => "View",
    }
}

/// The views the camera button steps through, in F-key order.
const VIEW_ORDER: [u8; 11] = [
    0,
    3,
    4,
    flight_views::TRACK,
    flight_views::THREAT,
    flight_views::WING,
    flight_views::TARGET,
    flight_views::TARGET_PLAYER,
    flight_views::FLY_BY,
    1,
    flight_views::MISSILE,
];

/// The terrain height under an east and north position, feet.
fn ground(world: &World) -> impl Fn(f64, f64) -> f64 + '_ {
    move |x, z| f64::from(world.height(x as f32, z as f32))
}

/// A copy of a camera: the terrain camera is not `Clone`.
fn copy(camera: &Camera) -> Camera {
    let mut out = Camera::new();
    out.weather_slot = camera.weather_slot;
    out.hidden_target = camera.hidden_target;
    out.hidden_projectile = camera.hidden_projectile;
    out.position = camera.position;
    out.yaw = camera.yaw;
    out.pitch = camera.pitch;
    out.roll = camera.roll;
    out.view_fraction = camera.view_fraction;
    out.zoom = camera.zoom;
    out.near_clip = camera.near_clip;
    out.keys = camera.keys.clone();
    out
}

/// The body roll rate between two recorded attitudes a tick apart,
/// radians per second: the part of the turn about the nose. The recording
/// keeps attitudes, not rates, and wing vapor shortens with roll rate.
pub fn roll_rate(before: [f64; 3], after: [f64; 3]) -> f64 {
    let a = Basis::new(before[0], before[1], before[2]);
    let b = Basis::new(after[0], after[1], after[2]);
    // For a small turn, each axis moves by the turn crossed with it, so
    // the sum of each axis crossed with its successor is twice the turn.
    let turn: [f64; 3] = std::array::from_fn(|i| {
        [(a.right, b.right), (a.up, b.up), (a.forward, b.forward)]
            .iter()
            .map(|(p, q)| tore_sim::attitude::cross(*p, *q)[i])
            .sum::<f64>()
            * clock::TICKS_PER_SECOND
            / 2.
    });
    // Banking right turns the aircraft negatively about its nose.
    -tore_sim::attitude::dot(turn, b.forward)
}

/// A name label over an aircraft: its text, the top left of the text on
/// the view, and its colour.
#[derive(Clone, Debug, PartialEq)]
struct Label {
    text: String,
    at: [f64; 2],
    color: [u8; 3],
}

/// Draws the interface over a blank view of `size`: the labels at the
/// view's own resolution, then the 640x480 layer. While the interface is
/// hidden the view is left clear, so only the 3D picture shows.
fn compose(
    canvas: &mut FlightCanvas,
    size: [u32; 2],
    hidden: bool,
    labels: &[Label],
    model: &Model,
    font: &tore_formats::font::Font,
    layer: &mut [u8],
) {
    canvas.blank(size);
    if hidden {
        return;
    }
    let scale = Placement::new(size).scale();
    for label in labels {
        canvas.text(font, &label.text, label.at, scale, label.color);
    }
    layer.fill(0);
    overlay::draw(layer, font, model);
    canvas.anchored_layer(layer);
}

/// Where a screenshot of `recording` at `tick` goes in `folder`: the
/// recording's name and the tick, numbered on when that file exists.
fn screenshot_path(folder: &Path, recording: &Path, tick: u64) -> PathBuf {
    let name = recording
        .file_name()
        .map_or_else(|| "replay".to_owned(), |n| n.to_string_lossy().into_owned());
    let stem = name
        .trim_end_matches(".partial")
        .trim_end_matches(".tore-replay");
    let mut path = folder.join(format!("{stem}-tick{tick:07}.png"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{stem}-tick{tick:07}-{n}.png"));
        n += 1;
    }
    path
}

/// The events the timeline marks: launches, kills, orders and bookmarks.
fn markers(events: &[TimedEvent]) -> Vec<Marker> {
    events
        .iter()
        .filter_map(|e| {
            let kind = match e.event.kind.as_str() {
                vocab::kind::WEAPON_LAUNCH => MarkerKind::Launch,
                vocab::kind::COMBAT_DESTROYED => MarkerKind::Kill,
                vocab::kind::COMMS_ORDER => MarkerKind::Order,
                vocab::kind::PLAYER_BOOKMARK => MarkerKind::Bookmark,
                _ => return None,
            };
            Some(Marker { tick: e.tick, kind })
        })
        .collect()
}

/// Each aircraft's recorded target over time: the tick and the target,
/// `None` once it has none. AI aircraft have their target changes; the
/// player's designation is noted with every command the player gives.
fn targets(events: &[TimedEvent]) -> BTreeMap<u32, Vec<(u64, Option<u32>)>> {
    let mut out: BTreeMap<u32, Vec<(u64, Option<u32>)>> = BTreeMap::new();
    for e in events {
        let target = match e.event.kind.as_str() {
            vocab::kind::AI_TARGET => e.event.object.or_else(|| e.event.id(vocab::field::TO)),
            vocab::kind::PLAYER_COMMAND => e.event.object,
            _ => continue,
        };
        if let Some(subject) = e.event.subject {
            out.entry(subject).or_default().push((e.tick, target));
        }
    }
    out
}

pub struct Viewer {
    pub path: PathBuf,
    recording: Arc<Recording>,
    pub world: World,
    /// The recorded player's aircraft, drawn in the renderer's ownship slot.
    pub ownship: Airframe,
    /// Models of the other aircraft, in the recording's draw order.
    models: Vec<Airframe>,
    art: CombatArt,
    /// A start state for the player's airframe; every drawn player state
    /// is rebuilt over it.
    template: flight::State,
    scratch: flight::State,
    playback: Playback,
    tracks: Tracks,
    scanner: Scanner,
    weather: WeatherTrack,
    pub clock: Clock,
    info: BTreeMap<u32, AircraftInfo>,
    markers: Vec<Marker>,
    /// Unique marker ticks, for PageUp and PageDown.
    marker_ticks: Vec<u64>,
    targets: BTreeMap<u32, Vec<(u64, Option<u32>)>>,
    /// Indices into the recording's events of every comms entry.
    comms: Vec<usize>,
    view: u8,
    drone: Option<Drone>,
    /// Where the selected aircraft was last drawn, so a follow drone holds
    /// its place while the aircraft is missing from the recording.
    anchor: Option<[f64; 3]>,
    rig: Rig,
    selected: u32,
    look: [f32; 2],
    zoom: f32,
    camera: Camera,
    /// A frame has been drawn, so `camera` is one the viewer chose.
    shown: bool,
    camera_error: Option<&'static str>,
    pub ui: Ui,
    /// Held drone movement keys.
    held: BTreeSet<String>,
    bar: overlay::Pointer,
    /// Where a right-drag last was, in window pixels.
    dragging: Option<[f64; 2]>,
    toast: Option<(String, Instant)>,
    layer: Vec<u8>,
    airports: Option<(BTreeSet<u32>, Vec<f32>, Vec<f32>)>,
    /// The renderer holds this viewer's world and models.
    entered: bool,
    last_frame: Option<Instant>,
}

impl Viewer {
    /// Opens a recording and loads what drawing it needs: its world, the
    /// player's airframe, the other aircraft models and the weapon art.
    pub fn open(
        path: &Path,
        resources: &BTreeMap<String, Vec<u8>>,
        options: &Options,
    ) -> AppResult<Self> {
        let recording =
            Arc::new(Recording::open(path).map_err(|e| format!("{}: {e}", path.display()))?);
        for problem in recording.problems() {
            log::warn!("Replay {}: {problem}", path.display());
        }
        if recording.first_tick().is_none() {
            return Err(format!("{}: the recording holds no frames", path.display()).into());
        }
        let world = World::for_identity(resources, &recording.header().world)?;
        let identities = crate::replay::convert::Identities::of(&recording);
        let presentation = crate::replay::convert::Presentation::from_header(recording.header());
        let player = match identities.aircraft.get(&0) {
            Some(id) => *id,
            None => {
                log::warn!("Replay: the recording has no player aircraft; drawing an F/A-18D");
                AircraftId::F18
            }
        };
        let ownship = Airframe::load(resources, player)?;
        let mut models: Vec<Airframe> = Vec::new();
        for id in &presentation.models {
            if !models.iter().any(|m| m.profile.id == *id) {
                models.push(Airframe::load(resources, *id)?);
            }
        }
        let mut art = CombatArt::load(resources, &ownship.palette)?;
        art.add_shapes(
            recording.weapons().filter_map(|w| w.shape.as_deref()),
            resources,
        );
        Self::assemble(path, recording, world, ownship, models, art, options)
    }

    /// A viewer from loaded parts.
    fn assemble(
        path: &Path,
        recording: Arc<Recording>,
        world: World,
        ownship: Airframe,
        models: Vec<Airframe>,
        art: CombatArt,
        options: &Options,
    ) -> AppResult<Self> {
        let (Some(first), Some(last)) = (recording.first_tick(), recording.last_tick()) else {
            return Err(format!("{}: the recording holds no frames", path.display()).into());
        };
        let playback = Playback::new(Arc::clone(&recording));
        let template = ownship.start(&world);
        let events = recording.events();
        let markers = markers(events);
        let mut marker_ticks: Vec<u64> = markers.iter().map(|m| m.tick).collect();
        marker_ticks.dedup();
        let comms = events
            .iter()
            .enumerate()
            .filter(|(_, e)| e.event.kind.starts_with("comms."))
            .map(|(i, _)| i)
            .collect();
        let mut clock = Clock::new(first, last);
        if let Some(speed) = options.speed {
            if speed < 0. {
                clock.end();
            }
            if !clock.set_speed(speed) {
                return Err(format!(
                    "{speed} is not a replay speed: use 0.125, 0.25, 0.5, 0.75, 1, 2, 4, 8 or 16, negative for reverse"
                )
                .into());
            }
        }
        if let Some(tick) = options.tick {
            clock.seek(tick as f64);
            if options.speed.is_none() {
                clock.pause();
            }
        }
        let info: BTreeMap<u32, AircraftInfo> =
            recording.aircraft().map(|a| (a.id, a.clone())).collect();
        let selected = options.aircraft.unwrap_or(0);
        if options.aircraft.is_some() && !info.contains_key(&selected) {
            return Err(format!("the recording has no aircraft {selected}").into());
        }
        let mut rig = Rig::default();
        rig.select(Reference::Aircraft(selected));
        let mut viewer = Self {
            path: path.to_path_buf(),
            scanner: Scanner::start(Arc::clone(&recording)),
            tracks: Tracks::new(first),
            // A headless probe never steps the weather, so its recording
            // keeps the launch sky throughout.
            weather: WeatherTrack::new(
                &world,
                if recording.header().mission == tore_replay::MissionKind::Probe {
                    0
                } else {
                    last
                },
            ),
            targets: targets(events),
            recording,
            world,
            scratch: template.clone(),
            template,
            ownship,
            models,
            art,
            playback,
            clock,
            info,
            markers,
            marker_ticks,
            comms,
            view: options.view.unwrap_or(EXTERNAL),
            drone: None,
            anchor: None,
            rig,
            selected,
            look: [0.; 2],
            zoom: 1.,
            camera: Camera::new(),
            shown: false,
            camera_error: None,
            ui: options.ui,
            held: BTreeSet::new(),
            bar: overlay::Pointer::default(),
            dragging: None,
            toast: None,
            layer: vec![0; overlay::WIDTH * overlay::HEIGHT * 4],
            airports: None,
            entered: false,
            last_frame: None,
        };
        if options.drone {
            viewer.drone_mode(Some(Mode::Follow));
        }
        Ok(viewer)
    }

    /// The window title.
    pub fn title(&self) -> String {
        let name = self
            .path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        format!("T.O.R.E-Fighters - Replay {name}")
    }

    /// Points the renderer at this viewer's world and the player's airframe.
    /// The world rebuild discards the renderer's aircraft, so the ownship
    /// slot is prepared after it.
    pub fn enter(&mut self, renderer: &mut Renderer) {
        if !self.entered {
            renderer.set_world(&self.world);
            renderer.prepare_aircraft(&self.ownship);
            self.entered = true;
            self.airports = None;
        }
    }

    /// The renderer holds this viewer's world and aircraft.
    pub fn entered(&self) -> bool {
        self.entered
    }

    /// Waits for the background pass over the recording, so trails, fallen
    /// buildings and the weather are complete: for captures.
    pub fn finish_tracks(&mut self) {
        self.scanner.wait(&mut self.tracks);
    }

    fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    /// Shows a message along the top of the view for a few seconds.
    pub fn message(&mut self, text: impl Into<String>) {
        self.toast(text);
    }

    fn label(&self, id: u32) -> String {
        self.info.get(&id).map_or_else(
            || format!("Aircraft {id}"),
            |a| {
                if !a.label.is_empty() {
                    a.label.clone()
                } else if !a.name.is_empty() {
                    a.name.clone()
                } else {
                    format!("Aircraft {id}")
                }
            },
        )
    }

    fn side(&self, id: u32) -> Side {
        if id == 0 {
            return self.info.get(&0).map_or(Side::Friendly, |a| match a.side {
                Side::Unknown => Side::Friendly,
                side => side,
            });
        }
        self.info.get(&id).map_or(Side::Unknown, |a| a.side)
    }

    /// Aircraft recorded on the current tick, in id order.
    fn present(&mut self) -> Vec<u32> {
        let tick = self.clock.tick();
        self.playback
            .frame(tick)
            .map(|(frames, at)| frames[at].aircraft.iter().map(|a| a.id).collect())
            .unwrap_or_default()
    }

    /// Tab and Shift+Tab: the next or previous aircraft on the current tick.
    pub fn cycle_aircraft(&mut self, forward: bool) {
        let ids = self.present();
        if ids.is_empty() {
            return;
        }
        let next = if forward {
            ids.iter()
                .copied()
                .find(|id| *id > self.selected)
                .unwrap_or(ids[0])
        } else {
            ids.iter()
                .copied()
                .rev()
                .find(|id| *id < self.selected)
                .unwrap_or(ids[ids.len() - 1])
        };
        self.select(next);
    }

    fn select(&mut self, id: u32) {
        if id != self.selected {
            self.selected = id;
            self.anchor = None;
            self.rig.select(Reference::Aircraft(id));
            self.camera_error = None;
            self.look = [0.; 2];
        }
    }

    /// F1 to F12: a flight view on the selected aircraft, leaving the drone.
    pub fn set_view(&mut self, view: u8) {
        self.drone = None;
        self.view = view;
        self.look = [0.; 2];
        self.zoom = 1.;
        self.camera_error = None;
        // The fly-by point is chosen afresh each time the view is picked.
        self.rig.select(Reference::Aircraft(self.selected));
    }

    /// Where the selected aircraft is drawn in `picture`.
    fn anchor(&self, picture: &RenderSnapshot) -> Option<[f64; 3]> {
        if self.selected == 0 {
            return Some(picture.player.position);
        }
        picture.target(self.selected).map(|t| t.position)
    }

    /// Backquote: from a flight view to the follow drone, then the free
    /// drone, then back. `Some(mode)` switches straight to that drone.
    pub fn drone_mode(&mut self, mode: Option<Mode>) {
        let tick = self.clock.tick();
        let recorded = self.playback.aircraft(tick, self.selected);
        let anchor = recorded.as_ref().map(|a| a.position);
        let heading = recorded.as_ref().map_or(0., |a| a.attitude[0]);
        let next = match (mode, &self.drone) {
            (Some(mode), _) => Some(mode),
            (None, None) => Some(Mode::Follow),
            (None, Some(drone)) if drone.mode == Mode::Follow => Some(Mode::Free),
            (None, Some(_)) => None,
        };
        match next {
            None => self.drone = None,
            Some(mode) => match &mut self.drone {
                Some(drone) => drone.set_mode(mode, anchor),
                None => {
                    self.drone = Some(Drone::from_camera(mode, &self.camera, anchor, heading));
                }
            },
        }
        self.camera_error = None;
    }

    fn camera_label(&self) -> String {
        match &self.drone {
            Some(drone) if drone.mode == Mode::Follow => "Drone follow".into(),
            Some(_) => "Drone free".into(),
            None => view_name(self.view).to_owned(),
        }
    }

    /// The camera button: the next flight view in F-key order, then the
    /// two drones, then the first view again.
    fn next_camera(&mut self) {
        match &self.drone {
            Some(drone) if drone.mode == Mode::Follow => self.drone_mode(Some(Mode::Free)),
            Some(_) => self.set_view(VIEW_ORDER[0]),
            None => {
                let at = VIEW_ORDER.iter().position(|v| *v == self.view);
                match at.map(|i| i + 1).filter(|i| *i < VIEW_ORDER.len()) {
                    Some(i) => self.set_view(VIEW_ORDER[i]),
                    None => self.drone_mode(Some(Mode::Follow)),
                }
            }
        }
    }

    /// A key pressed or released. `name` is the key as the app names it
    /// (letters lower case, `` ` `` for the backquote key).
    pub fn key(&mut self, name: &str, pressed: bool, repeat: bool, shift: bool) -> Command {
        if matches!(name, "w" | "a" | "s" | "d" | "e" | "q") {
            if pressed {
                self.held.insert(name.to_owned());
            } else {
                self.held.remove(name);
            }
            return Command::None;
        }
        if !pressed {
            return Command::None;
        }
        // Stepping and jumping repeat while held; switches do not.
        match name {
            "ArrowLeft" | "ArrowRight" => {
                let sign = if name == "ArrowLeft" { -1. } else { 1. };
                if shift {
                    self.clock.jump(sign * 30.);
                } else if self.clock.paused() {
                    self.clock.step(sign as i64);
                } else {
                    self.clock.jump(sign * 5.);
                }
                return Command::None;
            }
            "ArrowUp" if !repeat => self.clock.faster(),
            "ArrowDown" if !repeat => self.clock.slower(),
            _ if repeat => return Command::None,
            "Space" => self.clock.toggle(),
            "j" => self.clock.reverse(),
            "k" => self.clock.pause(),
            "l" => self.clock.forward(),
            "Home" => self.clock.start(),
            "End" => self.clock.end(),
            "PageUp" | "PageDown" => {
                if !self.clock.marker(&self.marker_ticks, name == "PageDown") {
                    self.toast("No more markers that way");
                }
            }
            "Tab" => self.cycle_aircraft(!shift),
            "`" => self.drone_mode(None),
            "h" => self.ui.hidden = !self.ui.hidden,
            "n" => self.ui.labels = !self.ui.labels,
            "t" => self.ui.timer = !self.ui.timer,
            "c" => self.ui.comms = !self.ui.comms,
            "r" if shift => {
                self.ui.trails = true;
                self.ui.trail_length = (self.ui.trail_length + 1) % trails::LENGTHS.len();
                let seconds = trails::LENGTHS[self.ui.trail_length];
                self.toast(format!("Trails: {seconds} seconds"));
            }
            "r" => {
                self.ui.trails = !self.ui.trails;
                self.toast(if self.ui.trails {
                    "Trails on (Shift+R for length)"
                } else {
                    "Trails off"
                });
            }
            "p" => return Command::Screenshot,
            "Escape" => {
                if self.ui.hidden {
                    self.ui.hidden = false;
                } else {
                    return Command::Leave;
                }
            }
            key => {
                if let Some(view) = flight_views::key(key) {
                    self.set_view(view);
                }
            }
        }
        Command::None
    }

    /// The pointer left the window.
    pub fn pointer_left(&mut self) {
        self.bar.hover = None;
        self.dragging = None;
    }

    /// Lets go of every held key and drag, when the window loses focus.
    pub fn release(&mut self) {
        self.held.clear();
        self.dragging = None;
        self.bar = overlay::Pointer::default();
    }

    /// The layer point under a view point, when the interface shows.
    fn layer_point(&self, point: Option<[f64; 2]>, size: [u32; 2]) -> Option<(f64, f64)> {
        point
            .filter(|_| !self.ui.hidden)
            .and_then(|p| Placement::new(size).layer(p))
    }

    /// The pointer moved to `point` in view pixels (`None` off the window);
    /// `window` is the same point in window pixels, for looking around, and
    /// `radians` how far one pixel turns the view.
    pub fn pointer(
        &mut self,
        point: Option<[f64; 2]>,
        window: [f64; 2],
        size: [u32; 2],
        radians: [f64; 2],
    ) {
        let layer = self.layer_point(point, size);
        self.bar.moved(layer, &mut self.clock);
        if let Some(last) = self.dragging {
            let delta = [
                (window[0] - last[0]) * radians[0],
                (window[1] - last[1]) * radians[1],
            ];
            self.dragging = Some(window);
            match &mut self.drone {
                Some(drone) => drone.look([delta[0], -delta[1]]),
                None => {
                    crate::look::nudge(&mut self.look, [delta[0] as f32, -delta[1] as f32], true)
                }
            }
        }
    }

    /// The left button went down or up at `point` in view pixels.
    pub fn left(&mut self, pressed: bool, point: Option<[f64; 2]>, size: [u32; 2]) {
        let layer = self.layer_point(point, size);
        if pressed {
            self.bar.down(layer, &mut self.clock);
            return;
        }
        let Some(control) = self.bar.up(layer) else {
            return;
        };
        if !overlay::transport(control, &mut self.clock) {
            match control {
                Control::Camera => self.next_camera(),
                Control::Aircraft => self.cycle_aircraft(true),
                Control::HideUi => self.ui.hidden = true,
                _ => {}
            }
        }
    }

    /// The right button went down or up at `window`, in window pixels:
    /// dragging with it turns the view.
    pub fn right(&mut self, pressed: bool, window: [f64; 2]) {
        self.dragging = pressed.then_some(window);
    }

    /// Mouse wheel notches: the drone's speed, or zoom in a flight view.
    pub fn wheel(&mut self, notches: i32) {
        match &mut self.drone {
            Some(drone) => {
                drone.wheel(notches);
                let speed = drone.speed();
                self.toast(format!("Drone speed {speed:.0} ft/s"));
            }
            None => self.zoom = (self.zoom * 1.1f32.powi(notches)).clamp(0.5, 4.),
        }
    }

    /// The UI covers the view: the pointer shows.
    pub fn pointer_visible(&self) -> bool {
        !self.ui.hidden
    }

    /// The target of `id` at `tick` as the recording knows it.
    fn target_of(&self, id: u32, tick: u64) -> Option<Option<u32>> {
        let list = self.targets.get(&id)?;
        let at = list.partition_point(|(t, _)| *t <= tick);
        at.checked_sub(1).map(|i| list[i].1)
    }

    /// The scene the flight views read, built from the picture: the
    /// selected aircraft's target when the recording knows it, otherwise
    /// the player's.
    fn scene(&self, picture: &RenderSnapshot, tick: u64) -> Scene {
        let body = |pose: &AircraftPose| {
            let [yaw, pitch, bank] = pose.attitude;
            Body::new(
                pose.id,
                pose.position,
                pose.velocity,
                Basis::new(yaw, pitch, bank),
            )
        };
        let target = self
            .target_of(self.selected, tick)
            .or_else(|| self.target_of(0, tick))
            .flatten();
        let flying = |pose: &&AircraftPose| pose.airborne && pose.damage.hp > 0;
        let wings = picture
            .targets
            .iter()
            .filter(flying)
            .filter_map(|pose| {
                let info = self.info.get(&pose.id)?;
                (info.wing > 0).then(|| {
                    (
                        pose.id,
                        info.side == Side::Friendly,
                        u8::try_from(info.wing).unwrap_or(u8::MAX),
                        u8::try_from(info.member).unwrap_or(u8::MAX),
                    )
                })
            })
            .collect();
        let missiles = picture
            .projectiles
            .iter()
            .filter(|p| !p.gun)
            .map(|p| {
                let d = p.direction;
                let speed = f64::from(p.speed_f8) / 256.;
                let basis = Basis::new(d[0].atan2(d[2]), d[1].atan2(d[0].hypot(d[2])), 0.);
                Shot::new(
                    Body::missile(p.id, p.position, d.map(|v| v * speed), basis),
                    p.owner,
                    p.target,
                    p.incoming,
                )
            })
            .collect();
        Scene::from_parts(
            body(&picture.player),
            target,
            picture
                .targets
                .iter()
                .filter(|t| t.airborne || t.damage.hp > 0)
                .map(body)
                .collect(),
            wings,
            missiles,
        )
    }

    /// The camera for this frame: the drone, or the selected flight view.
    /// A view that cannot be shown (no target, no wingman) says why once and
    /// shows the aircraft from outside; an aircraft no longer recorded keeps
    /// the last camera.
    fn frame_camera(
        &mut self,
        picture: &RenderSnapshot,
        tick: u64,
        seconds: f64,
        shift: bool,
    ) -> Camera {
        let anchor = self.anchor(picture).or(self.anchor);
        self.anchor = anchor;
        if let Some(drone) = &mut self.drone {
            drone.step(seconds, &self.held, shift, anchor, ground(&self.world));
            return drone.camera(anchor, ground(&self.world));
        }
        let scene = self.scene(picture, tick);
        match self
            .rig
            .camera(self.view, &scene, Camera::new(), self.look, self.zoom)
        {
            Ok(camera) => {
                self.camera_error = None;
                camera
            }
            Err(reason) => {
                if self.camera_error != Some(reason) {
                    self.camera_error = Some(reason);
                    self.toast(reason);
                }
                // The aircraft from outside; if it has left the scene, the
                // last camera shown, or at first the player from outside.
                let mut outside = self.rig.clone();
                let mut player = Rig::default();
                player.select(Reference::Aircraft(0));
                outside
                    .camera(EXTERNAL, &scene, Camera::new(), self.look, self.zoom)
                    .or_else(|error| {
                        if self.shown {
                            Err(error)
                        } else {
                            player.camera(EXTERNAL, &scene, Camera::new(), [0.; 2], 1.)
                        }
                    })
                    .unwrap_or_else(|_| copy(&self.camera))
            }
        }
    }

    /// The player as drawn: its pose over the airframe's start state, with
    /// the recorded load factor and the roll rate its attitudes imply.
    fn player_state(&mut self, picture: &RenderSnapshot, tick: u64) -> flight::State {
        let mut state = render_snapshot::pose_state(&self.template, &picture.player);
        if let Some(now) = self.playback.aircraft(tick, 0) {
            state.g = now.g;
            state.roll_rate = tick
                .checked_sub(1)
                .and_then(|t| self.playback.aircraft(t, 0))
                .map_or(0., |before| roll_rate(before.attitude, now.attitude));
        }
        state
    }

    /// Trail ribbons for every aircraft and guided weapon in the picture.
    fn trails(
        &self,
        picture: &RenderSnapshot,
        tick: u64,
        camera: &Camera,
        height: f64,
        out: &mut Vec<f32>,
    ) {
        let seconds = trails::LENGTHS[self.ui.trail_length];
        for pose in std::iter::once(&picture.player).chain(&picture.targets) {
            let Some(samples) = self.tracks.aircraft.get(&pose.id) else {
                continue;
            };
            let points = trails::path(samples, tick, seconds, pose.position);
            trails::ribbon(
                out,
                &points,
                trails::side_color(self.side(pose.id)),
                camera,
                height,
            );
        }
        for projectile in picture.projectiles.iter().filter(|p| !p.gun) {
            let Some(path) = self.tracks.missiles.get(&projectile.id) else {
                continue;
            };
            let points = trails::path(&path.samples, tick, seconds, projectile.position);
            let color = trails::weapon_color(self.side(path.owner));
            trails::ribbon(out, &points, color, camera, height);
        }
    }

    /// Name labels over the aircraft in the picture, in their side's colour,
    /// placed for a view of `size` pixels. The selected aircraft's label is
    /// bracketed; an aircraft the camera sits inside has none.
    fn labels(&self, picture: &RenderSnapshot, camera: &Camera, size: [u32; 2]) -> Vec<Label> {
        let font = &self.ownship.font;
        let scale = Placement::new(size).scale();
        let eye = camera.position.map(f64::from);
        let mut out = Vec::new();
        for pose in std::iter::once(&picture.player).chain(&picture.targets) {
            if Some(pose.id) == camera.hidden_target || (!pose.airborne && pose.crashed) {
                continue;
            }
            let distance = (0..3)
                .map(|i| (pose.position[i] - eye[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            if distance > LABEL_REACH {
                continue;
            }
            let Some([x, y]) = camera.project(size, pose.position) else {
                continue;
            };
            let mut text = self.label(pose.id);
            if pose.id == self.selected {
                text = format!("[{text}]");
            }
            let width = FlightCanvas::text_width(font, &text, scale);
            out.push(Label {
                at: [x - width / 2., y - (font.height as f64 + 8.) * scale],
                text,
                color: trails::side_color(self.side(pose.id)),
            });
        }
        out
    }

    /// Radio, tower, crew and cockpit lines heard in the last four seconds.
    fn subtitles(&self, tick: u64) -> Vec<String> {
        let events = self
            .recording
            .events_between(tick.saturating_sub(SUBTITLE_TICKS - 1), tick);
        events
            .iter()
            .filter_map(|e| {
                let event = &e.event;
                let heard = event.flag(vocab::field::HEARD) != Some(false);
                let speaker = event
                    .string(vocab::field::SPEAKER)
                    .map(str::to_owned)
                    .or_else(|| event.subject.map(|id| self.label(id)));
                match event.kind.as_str() {
                    vocab::kind::COMMS_RADIO | vocab::kind::COMMS_TOWER if heard => {
                        Some(match speaker {
                            Some(who) => format!("{who}: {}", event.text),
                            None => event.text.clone(),
                        })
                    }
                    vocab::kind::COMMS_CREW if heard => Some(format!(
                        "{}: {}",
                        event.string(vocab::field::SPEAKER).unwrap_or("Crew"),
                        event.text
                    )),
                    vocab::kind::COMMS_HUD => Some(event.text.clone()),
                    _ => None,
                }
            })
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// The Comms list: the latest recorded comms entries up to `tick`. The
    /// full, filterable Comms panel comes with the debug panels.
    fn comms_lines(&self, tick: u64) -> Vec<String> {
        let events = self.recording.events();
        let end = self.comms.partition_point(|&i| events[i].tick <= tick);
        self.comms[end.saturating_sub(COMMS_LINES)..end]
            .iter()
            .map(|&i| {
                let TimedEvent { tick, event } = &events[i];
                let kind = event
                    .kind
                    .strip_prefix("comms.")
                    .unwrap_or(&event.kind)
                    .to_ascii_uppercase();
                let who = event
                    .string(vocab::field::SPEAKER)
                    .map(str::to_owned)
                    .or_else(|| event.subject.map(|id| self.label(id)))
                    .unwrap_or_default();
                let what = if !event.text.is_empty() {
                    event.text.clone()
                } else {
                    [vocab::field::ORDER, vocab::field::OUTCOME]
                        .iter()
                        .find_map(|f| event.string(f))
                        .unwrap_or("")
                        .to_owned()
                };
                let unheard = if event.flag(vocab::field::HEARD) == Some(false) {
                    " (not heard)"
                } else {
                    ""
                };
                format!(
                    "{} {kind} {who}: {what}{unheard}",
                    clock::timestamp(*tick as f64)
                )
            })
            .collect()
    }

    /// The interface model for this frame.
    fn model(&self, tick: u64) -> Model {
        let active = if self.clock.paused() {
            Control::Pause
        } else {
            match self.clock.direction() {
                Direction::Reverse => Control::Reverse,
                Direction::Forward if self.clock.speed() > 1. => Control::FastForward,
                Direction::Forward => Control::Play,
            }
        };
        let aircraft = match self.info.get(&self.selected) {
            Some(info) if !info.name.is_empty() => {
                format!("{} {}", self.label(self.selected), info.name)
            }
            _ => self.label(self.selected),
        };
        Model {
            first: self.clock.first(),
            last: self.clock.last(),
            position: self.clock.position(),
            markers: self.markers.clone(),
            active: Some(active),
            pressed: self.bar.pressed,
            hover: self.bar.hover,
            speed: self.clock.label(),
            time: format!(
                "{} / {}",
                clock::timestamp(self.clock.position()),
                clock::timestamp(self.clock.last() as f64)
            ),
            camera: self.camera_label(),
            aircraft,
            timer: self.ui.timer.then(|| {
                format!(
                    "{}   tick {}",
                    clock::timestamp(self.clock.position()),
                    clock::grouped(tick)
                )
            }),
            toast: self
                .toast
                .as_ref()
                .filter(|(_, at)| at.elapsed() < TOAST)
                .map(|(text, _)| text.clone()),
            subtitles: if self.ui.subtitles {
                self.subtitles(tick)
            } else {
                Vec::new()
            },
            comms: self.ui.comms.then(|| self.comms_lines(tick)),
        }
    }

    /// Advances playback by the real time since the last frame and draws
    /// it: the 3D view through the same renderer calls live flight makes,
    /// then the interface.
    pub fn frame(
        &mut self,
        renderer: &mut Renderer,
        canvas: &mut FlightCanvas,
        shift: bool,
    ) -> AppResult<Timing> {
        self.enter(renderer);
        let now = Instant::now();
        let seconds = self
            .last_frame
            .map_or(0., |last| (now - last).as_secs_f64().min(0.25));
        self.last_frame = Some(now);
        self.scanner.poll(&mut self.tracks);
        self.weather
            .build(&mut self.world, &self.tracks, WEATHER_BUDGET);
        if !self.bar.scrubbing {
            self.clock.advance(seconds);
        }
        let tick = self.clock.tick();
        let picture = self.playback.picture(tick, self.clock.alpha());
        self.weather.seek(&mut self.world, &self.tracks, tick);
        let player = self.player_state(&picture, tick);
        let camera = self.frame_camera(&picture, tick, seconds, shift);
        self.world.resolve_palette(f64::from(camera.position[1]));
        let vapor = match self.playback.vapor(tick, &self.ownship, &mut self.scratch) {
            Some(vapor) => crate::vapor_vertices(
                &vapor,
                &self.world,
                &player,
                self.ownship.streamer_points(&player),
            ),
            None => Vec::new(),
        };
        renderer.vapor(&vapor);
        let [smoke, contrails] = self.playback.smoke(tick);
        // Recordings do not carry released chaff and flares yet, so none are
        // drawn and nothing lights the scene.
        let devices = tore_sim::combat::countermeasures::Devices::default();
        renderer.smoke(&self.art.smoke, [smoke, contrails], &devices);
        renderer.emitters(&devices, &[]);
        let destroyed = self.tracks.destroyed(tick);
        if self
            .airports
            .as_ref()
            .is_none_or(|(set, ..)| *set != destroyed)
        {
            let vertices = self.world.visible_static_vertices_where(&destroyed);
            let lines = self.world.visible_static_lines_where(&destroyed);
            self.airports = Some((destroyed, vertices, lines));
        }
        if let Some((_, vertices, lines)) = &self.airports {
            renderer.airports(vertices, lines);
        }
        if let Some(art) = &self.art.escape {
            renderer.escapees(
                art,
                &art.vertices_for(
                    picture
                        .pilots
                        .iter()
                        .map(|p| (p.position, p.heading, p.phase)),
                    &self.ownship.palette,
                    camera.position.map(f64::from),
                ),
            );
        }
        renderer.dummies(render_snapshot::aircraft_batches(
            &picture,
            &self.models,
            &camera,
            &self.world,
        ));
        let mut combat = render_snapshot::combat_geometry(
            &picture,
            &self.art,
            &self.ownship,
            &player,
            &camera,
            &self.world,
        );
        let size = renderer.flight_size();
        if self.ui.trails {
            self.trails(
                &picture,
                tick,
                &camera,
                f64::from(size[1]),
                &mut combat.vertices,
            );
        }
        renderer.combat(&combat);
        renderer.aircraft(
            &self.ownship,
            &player,
            camera.hidden_target != Some(0),
            &camera,
            &self.world,
        );
        // No cockpit, HUD or mirrors in a replay; the cockpit pass keeps its
        // switches between frames, so they are turned off every frame.
        renderer.cockpit(&player, &camera, false, false, &[], &self.world.palette);
        let composed = Instant::now();
        self.overlay(&picture, tick, &camera, size, canvas);
        let drawn = Instant::now();
        let presented = renderer.draw(
            &canvas.pixels,
            Some((&camera, &self.world)),
            Some(canvas.size),
        )?;
        self.camera = camera;
        self.shown = true;
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        Ok(Timing {
            presented,
            scene: ms(composed - now),
            interface: ms(drawn - composed),
            present: ms(drawn.elapsed()),
        })
    }

    /// Draws the interface over the view: labels and the layer, or nothing
    /// while it is hidden.
    fn overlay(
        &mut self,
        picture: &RenderSnapshot,
        tick: u64,
        camera: &Camera,
        size: [u32; 2],
        canvas: &mut FlightCanvas,
    ) {
        let (labels, model) = if self.ui.hidden {
            (Vec::new(), Model::default())
        } else {
            let labels = if self.ui.labels {
                self.labels(picture, camera, size)
            } else {
                Vec::new()
            };
            (labels, self.model(tick))
        };
        compose(
            canvas,
            size,
            self.ui.hidden,
            &labels,
            &model,
            &self.ownship.font,
            &mut self.layer,
        );
    }

    /// Saves the last frame's 3D view, without the interface, as a PNG in
    /// `folder`, named after the recording and the tick. Returns the file
    /// written.
    pub fn screenshot(&mut self, renderer: &mut Renderer, folder: &Path) -> AppResult<PathBuf> {
        std::fs::create_dir_all(folder)?;
        let path = screenshot_path(folder, &self.path, self.clock.tick());
        self.save_png(renderer, &path)?;
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.toast(format!("Saved screenshots/{name}"));
        Ok(path)
    }

    /// Writes the last frame's 3D view, without the interface, to `path` as
    /// a PNG at the view's size (at most 1920x1080).
    pub fn save_png(&self, renderer: &mut Renderer, path: &Path) -> AppResult<()> {
        let [width, height] = renderer.flight_size();
        let pixels = renderer.scene_pixels(&self.camera, &self.world, width, height, false)?;
        std::fs::write(
            path,
            crate::replay::png::encode_rgba(width, height, &pixels)?,
        )?;
        Ok(())
    }

    /// The camera of the last frame drawn.
    pub fn camera(&self) -> &Camera {
        &self.camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiding_the_interface_leaves_nothing_over_the_view() {
        let font = tore_formats::font::Font {
            height: 5,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 4,
                    pixels: vec![(0, 0), (1, 1), (2, 2)],
                })
                .collect(),
        };
        let labels = [Label {
            text: "[YOU]".into(),
            at: [400., 300.],
            color: [110, 170, 255],
        }];
        let model = Model {
            last: 1_000,
            speed: "1x".into(),
            timer: Some("00:00.0".into()),
            subtitles: vec!["You: 'Fox two'".into()],
            ..Default::default()
        };
        let mut canvas = FlightCanvas::default();
        let mut layer = vec![0; overlay::WIDTH * overlay::HEIGHT * 4];
        compose(
            &mut canvas,
            [1280, 720],
            false,
            &labels,
            &model,
            &font,
            &mut layer,
        );
        let covered = |c: &FlightCanvas| c.pixels.chunks_exact(4).filter(|p| p[3] != 0).count();
        assert!(covered(&canvas) > 10_000);
        let at =
            |c: &FlightCanvas, x: usize, y: usize| c.pixels[(y * 1280 + x) * 4..][..4].to_vec();
        assert_eq!(at(&canvas, 400, 300), [110, 170, 255, 255]);
        // Hidden, even with a stale layer, nothing is drawn.
        compose(
            &mut canvas,
            [1280, 720],
            true,
            &labels,
            &model,
            &font,
            &mut layer,
        );
        assert_eq!(canvas.size, [1280, 720]);
        assert_eq!(covered(&canvas), 0);
    }

    use crate::replay::fixture as f;
    use crate::replay::tests::TempDir;

    /// A viewer over the synthetic recording, with synthetic art and world.
    fn viewer(dir: &TempDir, options: &Options) -> Viewer {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let recording = Arc::new(f::recording(dir.path(), &format!("viewer-{n}")));
        let art = CombatArt::synthetic(BTreeMap::new(), vec![Vec::new(); 12], vec![Vec::new(); 12]);
        Viewer::assemble(
            Path::new("/x/test.tore-replay"),
            recording,
            crate::terrain::tests::world(),
            crate::combat::render_hash_tests::hornet_airframe(true),
            Vec::new(),
            art,
            options,
        )
        .unwrap()
    }

    fn press(viewer: &mut Viewer, key: &str) -> Command {
        viewer.key(key, true, false, false)
    }

    #[test]
    fn keys_drive_playback() {
        let dir = TempDir::new("viewer-keys");
        let mut v = viewer(&dir, &Options::default());
        assert!(!v.clock.paused());
        press(&mut v, "Space");
        assert!(v.clock.paused());
        // Held keys do not toggle again.
        v.key("Space", true, true, false);
        assert!(v.clock.paused());
        // Paused, the arrows step a tick; with Shift they jump 30 seconds.
        press(&mut v, "ArrowRight");
        assert_eq!(v.clock.position(), (f::FIRST + 1) as f64);
        v.key("ArrowRight", true, true, false);
        assert_eq!(v.clock.position(), (f::FIRST + 2) as f64);
        press(&mut v, "ArrowLeft");
        v.key("ArrowRight", true, false, true);
        assert_eq!(v.clock.position(), f::LAST as f64);
        press(&mut v, "Home");
        assert_eq!(v.clock.position(), f::FIRST as f64);
        // Playing, they jump 5 seconds.
        press(&mut v, "l");
        press(&mut v, "ArrowRight");
        assert_eq!(v.clock.position(), (f::FIRST + 600) as f64);
        press(&mut v, "l");
        assert_eq!(v.clock.speed(), 2.);
        press(&mut v, "ArrowUp");
        assert_eq!(v.clock.speed(), 4.);
        press(&mut v, "ArrowDown");
        press(&mut v, "ArrowDown");
        press(&mut v, "ArrowDown");
        assert_eq!(v.clock.speed(), 0.75);
        press(&mut v, "j");
        assert_eq!(
            (v.clock.direction(), v.clock.speed()),
            (Direction::Reverse, 1.)
        );
        press(&mut v, "k");
        assert!(v.clock.paused());
        press(&mut v, "End");
        assert_eq!(v.clock.position(), f::LAST as f64);
        // Markers: order, launch, bookmark and kill.
        press(&mut v, "Home");
        for expected in [f::ORDER, f::LAUNCH, f::BOOKMARK, f::KILL] {
            press(&mut v, "PageDown");
            assert_eq!(v.clock.position(), expected as f64);
        }
        press(&mut v, "PageDown");
        assert!(v.toast.is_some());
        press(&mut v, "PageUp");
        assert_eq!(v.clock.position(), f::BOOKMARK as f64);
    }

    #[test]
    fn keys_switch_views_aircraft_and_the_interface() {
        let dir = TempDir::new("viewer-views");
        let mut v = viewer(&dir, &Options::default());
        assert_eq!((v.view, v.selected), (EXTERNAL, 0));
        press(&mut v, "F2");
        assert_eq!(v.view, 3);
        press(&mut v, "F12");
        assert_eq!(v.view, flight_views::MISSILE);
        press(&mut v, "F11");
        assert_eq!(v.view, flight_views::MISSILE);
        // Tab walks the aircraft recorded now; the late one is not there yet.
        for expected in [1, 2, 0, 1] {
            press(&mut v, "Tab");
            assert_eq!(v.selected, expected);
        }
        v.key("Tab", true, false, true);
        assert_eq!(v.selected, 0);
        v.clock.seek(f::LATE_FROM as f64);
        v.key("Tab", true, false, true);
        assert_eq!(v.selected, f::LATE);
        // Backquote: follow drone, free drone, back to the view.
        press(&mut v, "`");
        assert_eq!(v.drone.as_ref().map(|d| d.mode), Some(Mode::Follow));
        press(&mut v, "`");
        assert_eq!(v.drone.as_ref().map(|d| d.mode), Some(Mode::Free));
        press(&mut v, "`");
        assert!(v.drone.is_none());
        assert_eq!(v.view, flight_views::MISSILE);
        // Drone keys are held while pressed.
        press(&mut v, "w");
        assert!(v.held.contains("w"));
        v.key("w", false, false, false);
        assert!(v.held.is_empty());
        press(&mut v, "q");
        v.release();
        assert!(v.held.is_empty());
        // The interface parts.
        let before = v.ui;
        for key in ["n", "t", "c", "r"] {
            press(&mut v, key);
        }
        assert_eq!(
            (v.ui.labels, v.ui.timer, v.ui.comms, v.ui.trails),
            (!before.labels, !before.timer, !before.comms, !before.trails)
        );
        v.ui.trails = false;
        v.key("r", true, false, true);
        assert!(v.ui.trails);
        assert_eq!(trails::LENGTHS[v.ui.trail_length], 60);
        assert_eq!(press(&mut v, "p"), Command::Screenshot);
        // H hides everything; Esc first shows it again, then leaves.
        press(&mut v, "h");
        assert!(v.ui.hidden && !v.pointer_visible());
        assert_eq!(press(&mut v, "Escape"), Command::None);
        assert!(!v.ui.hidden);
        assert_eq!(press(&mut v, "Escape"), Command::Leave);
    }

    #[test]
    fn clicks_on_the_bar_play_scrub_and_switch() {
        let dir = TempDir::new("viewer-clicks");
        let mut v = viewer(&dir, &Options::default());
        // A 4:3 view at twice the layer's size.
        let size = [1280, 960];
        let at = |control: Control| {
            let (x, y, w, h) = overlay::rect(control);
            Some([f64::from(x * 2 + w), f64::from(y * 2 + h)])
        };
        let click = |v: &mut Viewer, control| {
            v.left(true, at(control), size);
            v.left(false, at(control), size);
        };
        click(&mut v, Control::Pause);
        assert!(v.clock.paused());
        click(&mut v, Control::Play);
        assert!(!v.clock.paused());
        click(&mut v, Control::End);
        assert_eq!(v.clock.position(), f::LAST as f64);
        click(&mut v, Control::StepBack);
        assert_eq!(v.clock.position(), (f::LAST - 1) as f64);
        // Press on the timeline's middle, drag to its start, let go.
        let y = at(Control::Timeline).unwrap()[1];
        v.left(true, Some([640., y]), size);
        let middle = (f::FIRST + f::LAST) as f64 / 2.;
        assert!((v.clock.position() - middle).abs() < 1.);
        v.pointer(Some([16., y]), [0.; 2], size, [0.; 2]);
        assert_eq!(v.clock.position(), f::FIRST as f64);
        v.left(false, Some([16., y]), size);
        v.pointer(Some([640., y]), [0.; 2], size, [0.; 2]);
        assert_eq!(v.clock.position(), f::FIRST as f64);
        // The camera button steps through the views, then the drones.
        click(&mut v, Control::Camera);
        assert_eq!(v.view, flight_views::MISSILE);
        click(&mut v, Control::Camera);
        assert_eq!(v.drone.as_ref().map(|d| d.mode), Some(Mode::Follow));
        click(&mut v, Control::Camera);
        assert_eq!(v.drone.as_ref().map(|d| d.mode), Some(Mode::Free));
        click(&mut v, Control::Camera);
        assert_eq!((v.drone.is_none(), v.view), (true, 0));
        click(&mut v, Control::Aircraft);
        assert_eq!(v.selected, 1);
        // Hidden, the bar takes no clicks.
        click(&mut v, Control::HideUi);
        assert!(v.ui.hidden);
        let paused = v.clock.paused();
        click(&mut v, Control::Play);
        click(&mut v, Control::Pause);
        assert_eq!(v.clock.paused(), paused);
        // Right-drag turns the view.
        v.ui.hidden = false;
        v.right(true, [100., 100.]);
        v.pointer(None, [150., 80.], size, [0.01, 0.01]);
        assert!((v.look[0] - 0.5).abs() < 1e-6 && (v.look[1] - 0.2).abs() < 1e-6);
        v.right(false, [150., 80.]);
        v.pointer(None, [400., 80.], size, [0.01, 0.01]);
        assert!((v.look[0] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn views_follow_recorded_targets_and_fall_back_outside() {
        let dir = TempDir::new("viewer-cameras");
        let mut v = viewer(&dir, &Options::default());
        let camera = |v: &mut Viewer, tick: u64| {
            let picture = v.playback.picture(tick, 1.);
            v.frame_camera(&picture, tick, 0., false)
        };
        // Before the player has a target the target view says so once and
        // shows the player from outside.
        press(&mut v, "F7");
        let outside = camera(&mut v, 50);
        assert_eq!(v.camera_error, Some("No current target for this view"));
        assert!(v.toast.is_some());
        let external = f::position(0, 50);
        let offset: f64 = (0..3)
            .map(|i| (f64::from(outside.position[i]) - external[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(offset > 100. && offset < 250., "{offset}");
        // From tick 100 the player targets aircraft 2: the view looks from
        // the player towards it.
        let toward = camera(&mut v, 200);
        assert_eq!(v.camera_error, None);
        let forward = Basis::new(f64::from(toward.yaw), f64::from(toward.pitch), 0.).forward;
        let [a, b] = [f::position(0, 200), f::position(2, 200)];
        let d: [f64; 3] = std::array::from_fn(|i| b[i] - a[i]);
        let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!(tore_sim::attitude::dot(forward, d.map(|x| x / length)) > 0.99);
        // Aircraft 1's own target is the player; its missile view works
        // while its missile flies.
        press(&mut v, "Tab");
        press(&mut v, "F12");
        camera(&mut v, f::LAUNCH + 20);
        assert_eq!(v.camera_error, None);
        camera(&mut v, f::IMPACT + 20);
        assert_eq!(v.camera_error, Some("That aircraft has no live missile"));
        // The front view sits in the aircraft and hides it.
        press(&mut v, "F1");
        let inside = camera(&mut v, 300);
        assert_eq!(inside.hidden_target, Some(1));
        let picture = v.playback.picture(300, 1.);
        let labels = v.labels(&picture, &inside, [1280, 960]);
        assert!(labels.iter().all(|l| !l.text.contains("1-1")));
        // Outside, the selected aircraft's label is bracketed and coloured
        // for its side.
        press(&mut v, "F10");
        let outside = camera(&mut v, 300);
        let labels = v.labels(&picture, &outside, [1280, 960]);
        let own = labels.iter().find(|l| l.text == "[Enemy 1-1]").unwrap();
        assert_eq!(own.color, trails::side_color(Side::Enemy));
    }

    #[test]
    fn subtitles_and_the_comms_list_follow_the_playhead() {
        let dir = TempDir::new("viewer-comms");
        let v = viewer(&dir, &Options::default());
        assert_eq!(v.subtitles(f::HEARD + 20), ["Enemy 1-1: Fox two"]);
        // The unheard call makes no subtitle; the heard one lasts four seconds.
        assert_eq!(v.subtitles(f::UNHEARD + 5), ["Enemy 1-1: Fox two"]);
        assert!(v.subtitles(f::HEARD + SUBTITLE_TICKS).is_empty());
        assert!(v.subtitles(f::HEARD - 1).is_empty());
        let lines = v.comms_lines(f::LAST);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "00:00.4 ORDER You: Engage my target");
        assert!(lines[2].ends_with("Enemy 1-2: Contact (not heard)"));
        assert_eq!(v.comms_lines(f::ORDER - 1), Vec::<String>::new());
        let model = v.model(f::LAUNCH);
        assert_eq!(model.active, Some(Control::Play));
        assert_eq!(model.markers.len(), 4);
        assert_eq!(model.aircraft, "You F/A-18D");
        assert_eq!(model.camera, "F10 External");
    }

    #[test]
    fn command_line_options_set_the_start() {
        let dir = TempDir::new("viewer-options");
        let options = Options {
            tick: Some(300),
            view: Some(3),
            aircraft: Some(2),
            drone: true,
            speed: Some(-4.),
            ui: Ui::parse("trails,comms").unwrap(),
        };
        let v = viewer(&dir, &options);
        assert_eq!(v.clock.position(), 300.);
        assert_eq!(
            (v.clock.direction(), v.clock.speed(), v.clock.paused()),
            (Direction::Reverse, 4., false)
        );
        assert_eq!((v.view, v.selected), (3, 2));
        assert_eq!(v.drone.as_ref().map(|d| d.mode), Some(Mode::Follow));
        assert!(v.ui.trails && v.ui.comms && !v.ui.labels && !v.ui.timer);
        // A tick alone starts paused there.
        let v = viewer(
            &dir,
            &Options {
                tick: Some(10),
                ..Default::default()
            },
        );
        assert!(v.clock.paused());
        assert!(Ui::parse("labels,sparkles").is_err());
        let bad = Options {
            speed: Some(3.),
            ..Default::default()
        };
        let recording = Arc::new(f::recording(dir.path(), "bad-speed"));
        let art = CombatArt::synthetic(BTreeMap::new(), Vec::new(), Vec::new());
        assert!(
            Viewer::assemble(
                Path::new("x"),
                recording,
                crate::terrain::tests::world(),
                crate::combat::render_hash_tests::hornet_airframe(true),
                Vec::new(),
                art,
                &bad,
            )
            .is_err()
        );
    }

    #[test]
    fn screenshots_are_named_after_the_recording_and_tick() {
        let dir = crate::replay::tests::TempDir::new("viewer-shots");
        let recording = Path::new("/x/2026-09-26_1540_UKR_F18.tore-replay");
        let first = screenshot_path(dir.path(), recording, 86_808);
        assert_eq!(
            first.file_name().unwrap(),
            "2026-09-26_1540_UKR_F18-tick0086808.png"
        );
        std::fs::write(&first, b"x").unwrap();
        let second = screenshot_path(dir.path(), recording, 86_808);
        assert_eq!(
            second.file_name().unwrap(),
            "2026-09-26_1540_UKR_F18-tick0086808-2.png"
        );
        let partial = Path::new("/x/flight.tore-replay.partial");
        assert_eq!(
            screenshot_path(dir.path(), partial, 5).file_name().unwrap(),
            "flight-tick0000005.png"
        );
    }

    #[test]
    fn roll_rate_is_the_turn_about_the_nose() {
        let rate = 90f64.to_radians();
        for [yaw, pitch] in [[0., 0.], [1., 0.3], [-2., -0.5]] {
            let before = [yaw, pitch, 0.2];
            let after = [yaw, pitch, 0.2 + rate / 120.];
            assert!(
                (roll_rate(before, after) - rate).abs() < 1e-3,
                "{yaw} {pitch}"
            );
            assert!((roll_rate(after, before) + rate).abs() < 1e-3);
        }
        // A level turn is not a roll.
        assert!(roll_rate([0., 0., 0.], [0.01, 0., 0.]).abs() < 1e-9);
    }

    #[test]
    fn markers_and_targets_come_from_events() {
        let dir = crate::replay::tests::TempDir::new("viewer-events");
        let recording = crate::replay::fixture::recording(dir.path(), "events");
        use crate::replay::fixture as f;
        let marks = markers(recording.events());
        assert_eq!(
            marks,
            [
                Marker {
                    tick: f::ORDER,
                    kind: MarkerKind::Order
                },
                Marker {
                    tick: f::LAUNCH,
                    kind: MarkerKind::Launch
                },
                Marker {
                    tick: f::BOOKMARK,
                    kind: MarkerKind::Bookmark
                },
                Marker {
                    tick: f::KILL,
                    kind: MarkerKind::Kill
                },
            ]
        );
        let found = targets(recording.events());
        assert_eq!(found[&1], [(20, Some(0))]);
        assert_eq!(found[&0], [(100, Some(2))]);
        // The player's designation rides on its commands.
        let command = |tick, target: Option<u32>| TimedEvent {
            tick,
            event: {
                let event = tore_replay::Event::new(vocab::kind::PLAYER_COMMAND).with_subject(0);
                match target {
                    Some(id) => event.with_object(id),
                    None => event,
                }
            },
        };
        let player = targets(&[command(5, Some(3)), command(9, None)]);
        assert_eq!(player[&0], [(5, Some(3)), (9, None)]);
    }
}
