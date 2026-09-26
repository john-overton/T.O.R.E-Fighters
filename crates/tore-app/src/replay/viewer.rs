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
use crate::replay::context_menu::{self, Action, Menu, Outcome, Pickable, RightClick, Target};
use crate::replay::drone::{Drone, Mode};
use crate::replay::overlay::{self, Control, Marker, MarkerKind, Model, Placement};
use crate::replay::panels::{self, Data, Kind, Panels, RecordedTrees};
use crate::replay::playback::Playback;
use crate::replay::sound::{self, ReplaySound};
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
use tore_replay::{
    AircraftInfo, Event, Recording, Side, TimedEvent, TreeSample, WeaponClass, vocab,
};

/// Weather snapshots built ahead of the playhead each frame, in ticks:
/// a ten-minute recording is ready in about a second.
const WEATHER_BUDGET: u64 = 1_200;
/// How long a subtitle stays up, in ticks: about four seconds.
const SUBTITLE_TICKS: u64 = 480;
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
    /// The Comms panel is open.
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

/// A debug panel or the right-click menu to open when the viewer starts,
/// from `--replay-panels`, for captures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// AI thinking of the selected aircraft.
    Thought,
    /// Telemetry of the selected aircraft.
    Telemetry,
    /// Guidance of the selected aircraft's newest missile in flight.
    Guidance,
    Comms,
    /// The right-click menu on the selected aircraft.
    Menu,
}

impl Request {
    /// The panels named in a comma-separated list: thought, telemetry,
    /// guidance, comms and menu.
    pub fn parse(list: &str) -> Result<Vec<Self>, String> {
        list.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| match part {
                "thought" => Ok(Self::Thought),
                "telemetry" => Ok(Self::Telemetry),
                "guidance" => Ok(Self::Guidance),
                "comms" => Ok(Self::Comms),
                "menu" => Ok(Self::Menu),
                other => Err(format!(
                    "unknown replay panel {other:?}: use thought, telemetry, guidance, comms or menu"
                )),
            })
            .collect()
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
    /// Debug panels to open on the first frame.
    pub panels: Vec<Request>,
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
/// the view, its size there and its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub id: u32,
    pub text: String,
    pub at: [f64; 2],
    pub size: [f64; 2],
    pub color: [u8; 3],
}

impl Label {
    /// Where it is drawn, as a right-click picks it: left, top, width and
    /// height in view pixels.
    pub fn rect(&self) -> [f64; 4] {
        [self.at[0], self.at[1], self.size[0], self.size[1]]
    }
}

/// Name labels over `poses` for a view of `size` through `camera`: each
/// aircraft's label in its side's colour, `selected` in brackets. Aircraft
/// over 100 nautical miles away, wrecks on the ground and the aircraft the
/// camera sits in have none. `who` gives an aircraft's label and side.
pub fn name_labels<'a>(
    poses: impl Iterator<Item = &'a crate::render_snapshot::AircraftPose>,
    camera: &Camera,
    size: [u32; 2],
    font: &tore_formats::font::Font,
    selected: Option<u32>,
    who: &dyn Fn(u32) -> (String, Side),
) -> Vec<Label> {
    let scale = Placement::new(size).scale();
    let eye = camera.position.map(f64::from);
    let mut out = Vec::new();
    for pose in poses {
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
        let (mut text, side) = who(pose.id);
        text = crate::replay::panels::ascii(&text);
        if Some(pose.id) == selected {
            text = format!("[{text}]");
        }
        let width = FlightCanvas::text_width(font, &text, scale);
        out.push(Label {
            id: pose.id,
            at: [x - width / 2., y - (font.height as f64 + 8.) * scale],
            size: [width, (font.height as f64 + 1.) * scale],
            text,
            color: trails::side_color(side),
        });
    }
    out
}

/// The debug panels and the right-click menu, drawn in their own 640x480
/// layer, with where each is.
struct PanelLayer<'a> {
    pixels: &'a [u8],
    panels: &'a [crate::controls_editor::Rect],
    menu: Option<crate::controls_editor::Rect>,
}

/// Everything drawn over the 3D view in one frame.
struct Interface<'a> {
    hidden: bool,
    labels: &'a [Label],
    model: &'a Model,
    panels: PanelLayer<'a>,
}

/// Draws the interface over a blank view of `size`: the labels at the
/// view's own resolution, the debug panels, the 640x480 interface layer,
/// then the right-click menu over everything. While the interface is hidden
/// the view is left clear, so only the 3D picture shows.
fn compose(
    canvas: &mut FlightCanvas,
    size: [u32; 2],
    font: &tore_formats::font::Font,
    layer: &mut [u8],
    interface: &Interface,
) {
    canvas.blank(size);
    if interface.hidden {
        return;
    }
    let scale = Placement::new(size).scale();
    for label in interface.labels {
        canvas.text(font, &label.text, label.at, scale, label.color);
    }
    let panels = &interface.panels;
    canvas.centered_rects(panels.pixels, panels.panels);
    layer.fill(0);
    overlay::draw(layer, font, interface.model);
    canvas.anchored_layer(layer);
    if let Some(menu) = panels.menu {
        canvas.centered_rects(panels.pixels, &[menu]);
    }
}

/// An aircraft's label for people: its label, else its type, else its id.
fn label_of(info: &BTreeMap<u32, AircraftInfo>, id: u32) -> String {
    info.get(&id).map_or_else(
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

/// Each missile's shooter and weapon name, from the launch events.
fn missiles(recording: &Recording) -> BTreeMap<u32, (u32, String)> {
    recording
        .events()
        .iter()
        .filter(|e| e.event.kind == vocab::kind::WEAPON_LAUNCH)
        .filter_map(|e| {
            let projectile = e.event.id(vocab::field::PROJECTILE)?;
            let shooter = e.event.subject?;
            let weapon = e
                .event
                .id(vocab::field::WEAPON)
                .and_then(|w| recording.weapon_info(w))
                .filter(|w| !w.name.is_empty())
                .map_or_else(|| "Missile".to_owned(), |w| w.name.clone());
            Some((projectile, (shooter, weapon)))
        })
        .collect()
}

/// What a replay lends the debug panels: the recording at the playhead.
struct ReplayData<'a> {
    recording: &'a Recording,
    trees: &'a mut RecordedTrees,
    info: &'a BTreeMap<u32, AircraftInfo>,
    missiles: &'a BTreeMap<u32, (u32, String)>,
    comms: &'a [TimedEvent],
    now: u64,
}

impl Data for ReplayData<'_> {
    fn now(&self) -> u64 {
        self.now
    }

    fn tree(&mut self, subject: u32, channel: &str) -> Option<(u64, TreeSample)> {
        self.trees.tree(self.recording, subject, channel, self.now)
    }

    fn name(&self, id: u32) -> String {
        label_of(self.info, id)
    }

    fn title(&self, kind: Kind, subject: u32) -> String {
        match kind {
            Kind::Guidance => match self.missiles.get(&subject) {
                Some((owner, weapon)) => format!("{weapon} from {}", self.name(*owner)),
                None => format!("missile {subject}"),
            },
            Kind::Thought | Kind::Telemetry => match self.info.get(&subject) {
                Some(a) if !a.name.is_empty() && !a.label.is_empty() => {
                    format!("{}  {}", a.label, a.name)
                }
                _ => self.name(subject),
            },
        }
    }

    fn missing(&self, kind: Kind, subject: u32) -> String {
        let who = self.name(subject);
        match kind {
            Kind::Thought if subject == 0 || self.info.get(&subject).is_some_and(|a| a.human) => {
                format!("{who} is flown by a person, so there is no AI thinking to show.")
            }
            Kind::Thought => format!("No AI thinking recorded for {who} up to this moment."),
            Kind::Telemetry => format!("No telemetry recorded for {who} up to this moment."),
            Kind::Guidance => "No guidance recorded for this missile up to this moment.".into(),
        }
    }

    fn comms(&self) -> (&[TimedEvent], usize) {
        let end = self.comms.partition_point(|e| e.tick <= self.now);
        (self.comms, end)
    }

    fn aircraft(&self) -> Vec<u32> {
        self.info.keys().copied().collect()
    }
}

/// What took the left button's press, so its release goes there too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Menu,
    Panels,
    Bar,
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

/// A cockpit message the HUD showed or queued, from the HUD's own record:
/// not the communication journal's copy of an AI text line, which names its
/// source, nor a message the HUD never showed.
fn hud_shown(event: &Event) -> bool {
    use vocab::{field, outcome};
    event.get(field::SOURCE).is_none()
        && event
            .string(field::OUTCOME)
            .is_none_or(|o| o == outcome::DELIVERED || o == outcome::QUEUED)
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
    /// Voices, tones and effects at normal speed.
    sound: ReplaySound,
    pub clock: Clock,
    info: BTreeMap<u32, AircraftInfo>,
    markers: Vec<Marker>,
    /// Unique marker ticks, for PageUp and PageDown.
    marker_ticks: Vec<u64>,
    targets: BTreeMap<u32, Vec<(u64, Option<u32>)>>,
    /// Every comms and audio entry, in time order, for the Comms panel.
    comms: Vec<TimedEvent>,
    /// Each missile's shooter and weapon name.
    missiles: BTreeMap<u32, (u32, String)>,
    panels: Panels,
    trees: RecordedTrees,
    menu: Option<Menu>,
    /// A right-button press being watched for a click, and where it went
    /// down in view pixels.
    right_click: Option<(RightClick, [f64; 2])>,
    /// Where the pointer is in the panel layer, while the interface shows.
    point: Option<(f64, f64)>,
    left_owner: Option<Owner>,
    /// What the last frame drew that a right-click can pick.
    pickables: Vec<Pickable>,
    /// The debug panels' and menu's own layer.
    panel_layer: Vec<u8>,
    /// Panels asked for at start, opened on the first frame.
    requests: Vec<Request>,
    /// The view's size in the last frame.
    size: [u32; 2],
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
            .filter(|e| panels::Channel::of(&e.event.kind).is_some())
            .cloned()
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
            sound: ReplaySound::new(
                Arc::clone(&recording),
                std::iter::once(&ownship.profile).chain(models.iter().map(|m| &m.profile)),
            ),
            targets: targets(events),
            missiles: missiles(&recording),
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
            panels: Panels::default(),
            trees: RecordedTrees::default(),
            menu: None,
            right_click: None,
            point: None,
            left_owner: None,
            pickables: Vec::new(),
            panel_layer: vec![0; overlay::WIDTH * overlay::HEIGHT * 4],
            requests: options.panels.clone(),
            size: [overlay::WIDTH as u32, overlay::HEIGHT as u32],
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
        if options.ui.comms {
            viewer.panels.open_comms(None);
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
        label_of(&self.info, id)
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
            self.panels.follow(id);
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
        // An open right-click menu takes its keys first, held ones too.
        if let Some(menu) = &mut self.menu {
            match menu.key(name, shift) {
                Outcome::Ignored => {}
                Outcome::Handled => return Command::None,
                Outcome::Close => {
                    self.menu = None;
                    return Command::None;
                }
                Outcome::Chosen(action) => {
                    self.menu = None;
                    self.perform(action);
                    return Command::None;
                }
            }
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
            "h" => {
                self.ui.hidden = !self.ui.hidden;
                self.menu = None;
            }
            "n" => self.ui.labels = !self.ui.labels,
            "t" => self.ui.timer = !self.ui.timer,
            "c" => {
                self.panels.toggle_comms();
                self.ui.comms = self.panels.comms_open();
            }
            "i" => self.toggle_panel(Kind::Thought, self.selected),
            "f" => self.toggle_panel(Kind::Telemetry, self.selected),
            "g" => match self.newest_missile(self.selected) {
                Some(id) => self.toggle_panel(Kind::Guidance, id),
                None => {
                    let who = self.label(self.selected);
                    self.toast(format!("{who} has no missile in flight"));
                }
            },
            "x" => {
                self.panels.close_all();
                self.ui.comms = false;
            }
            "m" => self.menu_on_selected(),
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
        self.right_click = None;
        self.point = None;
        self.panels.hover = None;
    }

    /// Lets go of every held key and drag, when the window loses focus.
    pub fn release(&mut self) {
        self.held.clear();
        self.dragging = None;
        self.right_click = None;
        self.left_owner = None;
        self.bar = overlay::Pointer::default();
    }

    /// The layer point under a view point, when the interface shows.
    fn layer_point(&self, point: Option<[f64; 2]>, size: [u32; 2]) -> Option<(f64, f64)> {
        point
            .filter(|_| !self.ui.hidden)
            .and_then(|p| Placement::new(size).layer(p))
    }

    /// The point in the panels' layer under a view point, when the
    /// interface shows.
    fn panel_point(&self, point: Option<[f64; 2]>, size: [u32; 2]) -> Option<(f64, f64)> {
        point
            .filter(|_| !self.ui.hidden)
            .map(|p| Placement::new(size).centered(p))
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
        if let Some((click, _)) = &mut self.right_click {
            click.moved(window);
        }
        let layer = self.layer_point(point, size);
        let at = self.panel_point(point, size);
        self.point = at;
        let over_menu = match &mut self.menu {
            Some(menu) => {
                menu.pointer(at);
                at.is_some_and(|at| menu.contains(at))
            }
            None => false,
        };
        self.panels
            .pointer(panels::REPLAY, at.filter(|_| !over_menu));
        self.bar.moved(
            layer.filter(|_| !over_menu || self.bar.scrubbing),
            &mut self.clock,
        );
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

    /// The left button went down or up at `point` in view pixels: the
    /// right-click menu takes it while open (a press outside closes it),
    /// then the debug panels, then the transport bar.
    pub fn left(&mut self, pressed: bool, point: Option<[f64; 2]>, size: [u32; 2]) {
        let layer = self.layer_point(point, size);
        let at = self.panel_point(point, size);
        if pressed {
            let owner = if let Some(menu) = &mut self.menu {
                if !menu.down(at) {
                    self.menu = None;
                }
                Owner::Menu
            } else if self.panels.down(panels::REPLAY, at) {
                Owner::Panels
            } else {
                self.bar.down(layer, &mut self.clock);
                Owner::Bar
            };
            self.left_owner = Some(owner);
            return;
        }
        match self.left_owner.take() {
            Some(Owner::Menu) => {
                let chosen = self.menu.as_mut().map(|menu| menu.up(at));
                if let Some(Outcome::Chosen(action)) = chosen {
                    self.menu = None;
                    self.perform(action);
                }
                return;
            }
            Some(Owner::Panels) => {
                let data = ReplayData {
                    recording: &self.recording,
                    trees: &mut self.trees,
                    info: &self.info,
                    missiles: &self.missiles,
                    comms: &self.comms,
                    now: self.clock.tick(),
                };
                self.panels.up(panels::REPLAY, at, &data);
                self.ui.comms = self.panels.comms_open();
                return;
            }
            Some(Owner::Bar) | None => {}
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

    /// The right button went down or up at `window`, in window pixels, and
    /// at `point` in the view's own pixels on a view of `size`: dragging
    /// with it turns the view, and a click (released within `slop` window
    /// pixels, never having strayed further) opens the right-click menu.
    pub fn right(
        &mut self,
        pressed: bool,
        window: [f64; 2],
        point: Option<[f64; 2]>,
        size: [u32; 2],
        slop: f64,
    ) {
        if pressed {
            self.dragging = Some(window);
            self.right_click = point.map(|p| (RightClick::press(window, slop), p));
            return;
        }
        self.dragging = None;
        if let Some((click, at)) = self.right_click.take()
            && click.release(window)
        {
            self.open_menu(at, size);
        }
    }

    /// Mouse wheel notches: a menu or panel under the pointer scrolls,
    /// otherwise the drone's speed, or zoom in a flight view.
    pub fn wheel(&mut self, notches: i32) {
        if let Some(menu) = &mut self.menu
            && self.point.is_some_and(|at| menu.contains(at))
        {
            menu.wheel(notches);
            return;
        }
        let data = ReplayData {
            recording: &self.recording,
            trees: &mut self.trees,
            info: &self.info,
            missiles: &self.missiles,
            comms: &self.comms,
            now: self.clock.tick(),
        };
        if self
            .panels
            .wheel(panels::REPLAY, self.point, notches, &data)
        {
            return;
        }
        match &mut self.drone {
            Some(drone) => {
                drone.wheel(notches);
                let speed = drone.speed();
                self.toast(format!("Drone speed {speed:.0} ft/s"));
            }
            None => self.zoom = (self.zoom * 1.1f32.powi(notches)).clamp(0.5, 4.),
        }
    }

    fn open_panel(&mut self, kind: Kind, subject: u32) {
        if let Err(reason) = self.panels.open(kind, subject) {
            self.toast(reason);
        }
    }

    fn toggle_panel(&mut self, kind: Kind, subject: u32) {
        if let Err(reason) = self.panels.toggle(kind, subject) {
            self.toast(reason);
        }
    }

    /// The newest missile `owner` has in flight at the playhead.
    fn newest_missile(&mut self, owner: u32) -> Option<u32> {
        let (frames, at) = self.playback.frame(self.clock.tick())?;
        frames[at]
            .projectiles
            .iter()
            .filter(|p| {
                p.owner == owner
                    && self
                        .recording
                        .weapon_info(p.weapon)
                        .is_none_or(|w| w.class == WeaponClass::Missile)
            })
            .min_by_key(|p| p.age)
            .map(|p| p.id)
    }

    /// An aircraft as the menus list it.
    fn menu_entry(&self, id: u32) -> context_menu::Aircraft {
        let info = self.info.get(&id);
        context_menu::Aircraft {
            id,
            label: self.label(id),
            name: info.map(|a| a.name.clone()).unwrap_or_default(),
            side: self.side(id),
            wing: info.map_or(0, |a| a.wing),
            member: info.map_or(0, |a| a.member),
            ai: id != 0 && info.is_some_and(|a| !a.human),
        }
    }

    /// The right-click menu for `target`, opened at layer point `at`.
    fn build_menu(&mut self, target: Target, at: (f64, f64)) -> Menu {
        let options = context_menu::Options {
            live: false,
            labels: self.ui.labels,
            trails: self.ui.trails,
        };
        let (title, items) = match target {
            Target::Aircraft(id) => {
                let entry = self.menu_entry(id);
                (entry.title(), context_menu::aircraft_items(&entry, options))
            }
            Target::Missile(id) => {
                let (owner, title) = match self.missiles.get(&id) {
                    Some((owner, weapon)) => (
                        Some(*owner),
                        format!("{weapon} from {}", self.label(*owner)),
                    ),
                    None => (None, format!("Missile {id}")),
                };
                let owner = owner.map(|o| self.menu_entry(o));
                (
                    title,
                    context_menu::missile_items(id, owner.as_ref(), options),
                )
            }
            Target::Nothing => {
                let entries: Vec<_> = self
                    .present()
                    .into_iter()
                    .map(|id| self.menu_entry(id))
                    .collect();
                (
                    "Jump to an aircraft".to_owned(),
                    context_menu::jump_items(&entries, options),
                )
            }
        };
        Menu::new(target, title, items, at, &self.ownship.font)
    }

    /// A right-click at `at` in view pixels on a view of `size`: the menu
    /// for a panel's aircraft or missile when on a panel, otherwise for the
    /// aircraft or missile drawn nearest the pointer, otherwise the list of
    /// aircraft. Nothing opens while the interface is hidden.
    fn open_menu(&mut self, at: [f64; 2], size: [u32; 2]) {
        if self.ui.hidden {
            return;
        }
        let placement = Placement::new(size);
        let layer = placement.centered(at);
        let on_panel = match self.panels.hit(panels::REPLAY, layer) {
            Some(panels::Hit::Body(side) | panels::Hit::Pin(side) | panels::Hit::Close(side)) => {
                self.panels.slot(side).map(|p| match p.kind {
                    Kind::Guidance => Target::Missile(p.subject),
                    Kind::Thought | Kind::Telemetry => Target::Aircraft(p.subject),
                })
            }
            _ => None,
        };
        let target = on_panel.unwrap_or_else(|| {
            context_menu::pick(
                at,
                size,
                &self.camera,
                &self.pickables,
                context_menu::PICK_RADIUS * placement.scale(),
            )
        });
        self.menu = Some(self.build_menu(target, layer));
    }

    /// M: the right-click menu on the selected aircraft, where the last
    /// frame drew it or else in the middle of the view.
    fn menu_on_selected(&mut self) {
        let camera = copy(&self.camera);
        self.menu_at_selected(&camera);
    }

    /// The right-click menu on the selected aircraft, where `camera` shows
    /// it or else in the middle of the view.
    fn menu_at_selected(&mut self, camera: &Camera) {
        if self.ui.hidden {
            return;
        }
        let size = self.size;
        let at = self
            .pickables
            .iter()
            .find(|p| p.target == Target::Aircraft(self.selected))
            .and_then(|p| camera.project(size, p.position))
            .unwrap_or([f64::from(size[0]) / 2., f64::from(size[1]) / 2.]);
        let layer = Placement::new(size).centered(at);
        self.menu = Some(self.build_menu(Target::Aircraft(self.selected), layer));
    }

    /// Does what a menu item says.
    fn perform(&mut self, action: Action) {
        match action {
            Action::Follow(id) => {
                self.select(id);
                self.set_view(EXTERNAL);
            }
            Action::Cockpit(id) => {
                self.select(id);
                self.set_view(0);
            }
            Action::Drone(id) => {
                self.select(id);
                match self.playback.aircraft(self.clock.tick(), id) {
                    Some(a) => {
                        self.drone = Some(Drone::beside(Mode::Follow, a.position, a.attitude[0]));
                        self.camera_error = None;
                    }
                    None => self.toast("That aircraft is not in the recording now"),
                }
            }
            Action::DroneMissile(id) => {
                let found = self
                    .playback
                    .frame(self.clock.tick())
                    .and_then(|(frames, at)| {
                        frames[at]
                            .projectiles
                            .iter()
                            .find(|p| p.id == id)
                            .map(|p| (p.position, p.direction[0].atan2(p.direction[2])))
                    });
                match found {
                    Some((position, heading)) => {
                        self.drone = Some(Drone::beside(Mode::Free, position, heading));
                        self.camera_error = None;
                    }
                    None => self.toast("That missile is not in the recording now"),
                }
            }
            Action::Thought(id) => self.open_panel(Kind::Thought, id),
            Action::Telemetry(id) => self.open_panel(Kind::Telemetry, id),
            Action::Guidance(id) => self.open_panel(Kind::Guidance, id),
            Action::Comms(id) => self.panels.open_comms(Some(id)),
            Action::Labels => self.ui.labels = !self.ui.labels,
            Action::Trails => self.ui.trails = !self.ui.trails,
            Action::Jump(id) => self.select(id),
        }
        self.ui.comms = self.panels.comms_open();
    }

    /// What a right-click can pick in `picture`: every recorded aircraft,
    /// with its name label when one is drawn, and every weapon but gun
    /// rounds.
    fn pickables_for(picture: &RenderSnapshot, labels: &[Label]) -> Vec<Pickable> {
        let labels: Vec<(u32, [f64; 4])> = labels.iter().map(|l| (l.id, l.rect())).collect();
        context_menu::pickables(picture, &labels, |_| true)
    }

    /// The panels and menu asked for at start, once the first picture is
    /// known and `camera` shows it.
    fn open_requests(&mut self, camera: &Camera) {
        for request in std::mem::take(&mut self.requests) {
            match request {
                Request::Thought => self.open_panel(Kind::Thought, self.selected),
                Request::Telemetry => self.open_panel(Kind::Telemetry, self.selected),
                Request::Guidance => match self.newest_missile(self.selected) {
                    Some(id) => self.open_panel(Kind::Guidance, id),
                    None => {
                        let who = self.label(self.selected);
                        self.toast(format!("{who} has no missile in flight"));
                    }
                },
                Request::Comms => self.panels.open_comms(None),
                Request::Menu => self.menu_at_selected(camera),
            }
        }
        self.ui.comms = self.panels.comms_open();
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
        name_labels(
            std::iter::once(&picture.player).chain(&picture.targets),
            camera,
            size,
            &self.ownship.font,
            Some(self.selected),
            &|id| (self.label(id), self.side(id)),
        )
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
                // Once per line: at its delivery, not when it was queued
                // or cut off.
                let heard = vocab::heard(event);
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
                    vocab::kind::COMMS_HUD if hud_shown(event) => Some(event.text.clone()),
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
            // The Comms panel lists every line, where the subtitles would
            // sit over it.
            subtitles: if self.ui.subtitles && !self.panels.comms_open() {
                self.subtitles(tick)
            } else {
                Vec::new()
            },
        }
    }

    /// Advances playback by the real time since the last frame and draws
    /// it: the 3D view through the same renderer calls live flight makes,
    /// then the interface. The stretch played sounds on `audio`.
    pub fn frame(
        &mut self,
        renderer: &mut Renderer,
        canvas: &mut FlightCanvas,
        shift: bool,
        audio: Option<&crate::audio::Audio>,
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
        let from = self.clock.position();
        if !self.bar.scrubbing {
            self.clock.advance(seconds);
        }
        let tick = self.clock.tick();
        let picture = self.playback.picture(tick, self.clock.alpha());
        self.weather.seek(&mut self.world, &self.tracks, tick);
        let player = self.player_state(&picture, tick);
        let camera = self.frame_camera(&picture, tick, seconds, shift);
        let moment = sound::Moment {
            from,
            clock: &self.clock,
            scrubbing: self.bar.scrubbing,
            camera: &camera,
            view: self.drone.is_none().then_some(self.view),
            selected: self.selected,
        };
        self.sound.frame(audio, &moment);
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

    /// Draws the interface over the view: labels, the debug panels, the
    /// layer and the right-click menu, or nothing while it is hidden. Notes
    /// what a right-click can pick in this picture.
    fn overlay(
        &mut self,
        picture: &RenderSnapshot,
        tick: u64,
        camera: &Camera,
        size: [u32; 2],
        canvas: &mut FlightCanvas,
    ) {
        let labels = if self.ui.labels && !self.ui.hidden {
            self.labels(picture, camera, size)
        } else {
            Vec::new()
        };
        self.size = size;
        self.pickables = Self::pickables_for(picture, &labels);
        if !self.requests.is_empty() {
            self.open_requests(camera);
        }
        let mut panel_rects = Vec::new();
        let mut menu_rect = None;
        let model = if self.ui.hidden {
            Model::default()
        } else {
            self.panel_layer.fill(0);
            let font = &self.ownship.font;
            let mut data = ReplayData {
                recording: &self.recording,
                trees: &mut self.trees,
                info: &self.info,
                missiles: &self.missiles,
                comms: &self.comms,
                now: tick,
            };
            panel_rects = self
                .panels
                .draw(&mut self.panel_layer, font, panels::REPLAY, &mut data);
            if let Some(menu) = &self.menu {
                menu.draw(&mut self.panel_layer, font);
                menu_rect = Some(menu.rect());
            }
            self.ui.comms = self.panels.comms_open();
            self.model(tick)
        };
        compose(
            canvas,
            size,
            &self.ownship.font,
            &mut self.layer,
            &Interface {
                hidden: self.ui.hidden,
                labels: &labels,
                model: &model,
                panels: PanelLayer {
                    pixels: &self.panel_layer,
                    panels: &panel_rects,
                    menu: menu_rect,
                },
            },
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
            id: 0,
            text: "[YOU]".into(),
            at: [800., 300.],
            size: [20., 6.],
            color: [110, 170, 255],
        }];
        let model = Model {
            last: 1_000,
            speed: "1x".into(),
            timer: Some("00:00.0".into()),
            subtitles: vec!["You: 'Fox two'".into()],
            ..Default::default()
        };
        // A panel on the left and a menu over the bar's corner.
        let mut panel_pixels = vec![0; overlay::WIDTH * overlay::HEIGHT * 4];
        let panel = (6, 28, 254, 398);
        let menu = (500, 400, 100, 60);
        for (x, y, w, h) in [panel, menu] {
            for yy in y..y + h {
                for xx in x..x + w {
                    panel_pixels[(yy * 640 + xx) as usize * 4..][..4]
                        .copy_from_slice(&[1, 2, 3, 255]);
                }
            }
        }
        let mut interface = Interface {
            hidden: false,
            labels: &labels,
            model: &model,
            panels: PanelLayer {
                pixels: &panel_pixels,
                panels: &[panel],
                menu: Some(menu),
            },
        };
        let mut canvas = FlightCanvas::default();
        let mut layer = vec![0; overlay::WIDTH * overlay::HEIGHT * 4];
        compose(&mut canvas, [1280, 720], &font, &mut layer, &interface);
        let covered = |c: &FlightCanvas| c.pixels.chunks_exact(4).filter(|p| p[3] != 0).count();
        assert!(covered(&canvas) > 10_000);
        let at =
            |c: &FlightCanvas, x: usize, y: usize| c.pixels[(y * 1280 + x) * 4..][..4].to_vec();
        assert_eq!(at(&canvas, 800, 300), [110, 170, 255, 255]);
        // The layer is 960 wide on this view, 160 in from the left: the
        // panel, and the menu drawn over the transport bar.
        assert_eq!(at(&canvas, 160 + 100, 300), [1, 2, 3, 255]);
        assert_eq!(at(&canvas, 160 + 825, 670), [1, 2, 3, 255]);
        // Hidden, even with stale layers, nothing is drawn.
        interface.hidden = true;
        compose(&mut canvas, [1280, 720], &font, &mut layer, &interface);
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
        let mut ownship = crate::combat::render_hash_tests::hornet_airframe(true);
        // Menus measure their text, so the font needs its glyphs.
        ownship.font = crate::replay::panels::tests::font();
        Viewer::assemble(
            Path::new("/x/test.tore-replay"),
            recording,
            crate::terrain::tests::world(),
            ownship,
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
        // Right-drag turns the view, and opens no menu.
        v.ui.hidden = false;
        v.right(true, [100., 100.], Some([100., 100.]), size, 4.);
        v.pointer(None, [150., 80.], size, [0.01, 0.01]);
        assert!((v.look[0] - 0.5).abs() < 1e-6 && (v.look[1] - 0.2).abs() < 1e-6);
        v.right(false, [150., 80.], Some([150., 80.]), size, 4.);
        v.pointer(None, [400., 80.], size, [0.01, 0.01]);
        assert!((v.look[0] - 0.5).abs() < 1e-6);
        assert!(v.menu.is_none());
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

    /// The panels' view of the recording at `now`.
    fn data(v: &mut Viewer, now: u64) -> ReplayData<'_> {
        ReplayData {
            recording: &v.recording,
            trees: &mut v.trees,
            info: &v.info,
            missiles: &v.missiles,
            comms: &v.comms,
            now,
        }
    }

    /// The Comms panel's rows at `now`, unfiltered, as time, kind and words.
    fn comms_rows(v: &mut Viewer, now: u64) -> Vec<String> {
        let data = data(v, now);
        let (events, end) = data.comms();
        let rows = panels::rows_before(events, end, &panels::CommsPanel::default(), 20);
        rows.iter()
            .map(|row| {
                let e = panels::entry(events, row, &|id| data.name(id));
                let outcome = e
                    .outcome
                    .map(|(o, _)| format!(" [{o}]"))
                    .unwrap_or_default();
                format!("{} {} {}{outcome}", e.time, e.kind, e.main)
            })
            .collect()
    }

    #[test]
    fn subtitles_and_the_comms_panel_follow_the_playhead() {
        let dir = TempDir::new("viewer-comms");
        let mut v = viewer(&dir, &Options::default());
        assert_eq!(v.subtitles(f::HEARD + 20), ["Enemy 1-1: Fox two"]);
        // The unheard call makes no subtitle; the heard one lasts four seconds.
        assert_eq!(v.subtitles(f::UNHEARD + 5), ["Enemy 1-1: Fox two"]);
        assert!(v.subtitles(f::HEARD + SUBTITLE_TICKS).is_empty());
        assert!(v.subtitles(f::HEARD - 1).is_empty());
        assert_eq!(
            comms_rows(&mut v, f::LAST),
            [
                "00:00.4 ORDER You: Engage my target",
                "00:01.1 RADIO Enemy 1-1: Fox two [heard by you]",
                "00:01.2 RADIO Enemy 1-2: Contact [not heard]",
            ]
        );
        // Playing backwards shows exactly what playing forwards showed.
        assert_eq!(comms_rows(&mut v, f::HEARD).len(), 2);
        assert!(comms_rows(&mut v, f::ORDER - 1).is_empty());
        let model = v.model(f::LAUNCH);
        assert_eq!(model.active, Some(Control::Play));
        assert_eq!(model.markers.len(), 4);
        assert_eq!(model.aircraft, "You F/A-18D");
        assert_eq!(model.camera, "F10 External");
        // C opens the Comms panel, which takes the subtitles' place.
        press(&mut v, "c");
        assert!(v.ui.comms && v.panels.comms_open());
        assert!(v.model(f::HEARD + 20).subtitles.is_empty());
        press(&mut v, "c");
        assert!(!v.ui.comms && !v.panels.comms_open());
        assert_eq!(v.model(f::HEARD + 20).subtitles.len(), 1);
    }

    #[test]
    fn panels_read_the_trees_at_the_playhead_in_either_direction() {
        let dir = TempDir::new("viewer-trees");
        let mut v = viewer(&dir, &Options::default());
        let channel = vocab::channel::AI_THOUGHT;
        for tick in [200, 900, 199, 181, 179, f::GAP.0 + 20, 30] {
            let (at, tree) = data(&mut v, tick).tree(1, channel).unwrap();
            // Inside the gap the last sample before it answers.
            let last = if (f::GAP.0..=f::GAP.1).contains(&tick) {
                f::GAP.0 - 1
            } else {
                tick
            };
            let expected = last / f::THOUGHT_EVERY * f::THOUGHT_EVERY;
            assert_eq!(at, expected, "{tick}");
            assert_eq!(tree.nodes, f::thought(expected));
        }
        assert!(data(&mut v, f::FIRST).tree(1, channel).is_none());
        let d = data(&mut v, 300);
        assert_eq!(d.title(Kind::Thought, 1), "Enemy 1-1  MiG-29");
        assert_eq!(
            d.title(Kind::Guidance, f::MISSILE),
            "Missile from Enemy 1-1"
        );
        assert!(d.missing(Kind::Thought, 0).contains("flown by a person"));
        assert!(
            d.missing(Kind::Thought, 2)
                .starts_with("No AI thinking recorded")
        );
    }

    /// A camera 3,000 feet south of and level with aircraft `id` at `tick`,
    /// looking at it.
    fn camera_on(id: u32, tick: u64) -> Camera {
        let at = f::position(id, tick);
        Drone::looking(Mode::Free, [at[0], at[1], at[2] - 3_000.], at, None).camera(None, |_, _| 0.)
    }

    /// The last frame showed `tick` through `camera` on a view of `size`.
    fn shown(v: &mut Viewer, tick: u64, camera: Camera, size: [u32; 2]) {
        v.clock.seek(tick as f64);
        let picture = v.playback.picture(tick, 1.);
        v.pickables = Viewer::pickables_for(&picture, &[]);
        v.camera = camera;
        v.size = size;
    }

    fn right_click(v: &mut Viewer, at: [f64; 2], size: [u32; 2]) {
        v.right(true, at, Some(at), size, 4.);
        v.right(false, [at[0] + 2., at[1] - 1.], Some(at), size, 4.);
    }

    #[test]
    fn right_clicks_pick_what_is_drawn_and_the_menu_acts_on_it() {
        let dir = TempDir::new("viewer-menu");
        let mut v = viewer(&dir, &Options::default());
        v.clock.pause();
        for size in [[1280, 960], [1920, 1080], [800, 1200]] {
            shown(&mut v, 300, camera_on(1, 300), size);
            let middle = [f64::from(size[0]) / 2., f64::from(size[1]) / 2.];
            right_click(&mut v, [middle[0] + 4., middle[1] - 3.], size);
            let menu = v.menu.take().unwrap();
            assert_eq!(menu.target, Target::Aircraft(1), "{size:?}");
            assert_eq!(menu.title, "Enemy 1-1  MiG-29");
            // The menu opens beside the click in the centred panel layer.
            let layer = Placement::new(size).centered(middle);
            let (x, y, _, _) = menu.rect();
            assert!((f64::from(x) - layer.0).abs() < 12. && (f64::from(y) - layer.1).abs() < 12.);
        }
        let size = [1280, 960];
        shown(&mut v, 300, camera_on(1, 300), size);
        // A drag opens nothing.
        v.right(true, [640., 480.], Some([640., 480.]), size, 4.);
        v.pointer(Some([700., 480.]), [700., 480.], size, [0.; 2]);
        v.right(false, [700., 480.], Some([640., 480.]), size, 4.);
        assert!(v.menu.is_none());
        // Keys move through the menu and choose: AI thinking.
        right_click(&mut v, [640., 480.], size);
        for _ in 0..3 {
            assert_eq!(press(&mut v, "ArrowDown"), Command::None);
        }
        assert_eq!(press(&mut v, "Enter"), Command::None);
        assert!(v.menu.is_none());
        let panel = v.panels.slot(panels::Side::Left).unwrap();
        assert_eq!((panel.kind, panel.subject), (Kind::Thought, 1));
        // Esc closes the menu, not the viewer.
        right_click(&mut v, [640., 480.], size);
        assert_eq!(press(&mut v, "Escape"), Command::None);
        assert!(v.menu.is_none());
        // Follow, cockpit and drone switch the camera to the aircraft.
        for (action, check) in [
            (Action::Follow(1), (1, EXTERNAL, false)),
            (Action::Cockpit(2), (2, 0, false)),
            (Action::Drone(1), (1, 0, true)),
        ] {
            v.perform(action);
            assert_eq!((v.selected, v.view, v.drone.is_some()), check, "{action:?}");
        }
        // Empty space lists the aircraft recorded now; a pick jumps to one.
        v.drone = None;
        right_click(&mut v, [20., 20.], size);
        let menu = v.menu.as_ref().unwrap();
        assert_eq!(menu.target, Target::Nothing);
        let jumps: Vec<_> = menu.items.iter().filter_map(|i| i.action).collect();
        assert!(jumps.contains(&Action::Jump(3)) && jumps.contains(&Action::Jump(0)));
        press(&mut v, "Home");
        press(&mut v, "Enter");
        assert_eq!(v.selected, 0);
        // A left click outside the menu closes it and does nothing else.
        right_click(&mut v, [20., 20.], size);
        let paused = v.clock.paused();
        let play = overlay::rect(Control::Play);
        let on_play = Some([f64::from(play.0 * 2 + 4), f64::from(play.1 * 2 + 4)]);
        v.left(true, on_play, size);
        v.left(false, on_play, size);
        assert!(v.menu.is_none());
        assert_eq!(v.clock.paused(), paused);
        // A missile in flight: its guidance.
        let tick = f::LAUNCH + 40;
        let missile = f::position(1, f::LAUNCH);
        let eye = [missile[0] + 400., missile[1] + 100., missile[2] - 3_000.];
        let at = v.playback.picture(tick, 1.).projectiles[0].position;
        let camera = Drone::looking(Mode::Free, eye, at, None).camera(None, |_, _| 0.);
        shown(&mut v, tick, camera, size);
        right_click(&mut v, [640., 480.], size);
        let menu = v.menu.as_ref().unwrap();
        assert_eq!(menu.target, Target::Missile(f::MISSILE));
        assert_eq!(menu.title, "Missile from Enemy 1-1");
        press(&mut v, "Enter");
        assert!(v.panels.find(Kind::Guidance, f::MISSILE).is_some());
        // Hidden, a right-click opens nothing.
        press(&mut v, "h");
        right_click(&mut v, [640., 480.], size);
        assert!(v.menu.is_none());
    }

    #[test]
    fn panel_keys_open_follow_and_close() {
        let dir = TempDir::new("viewer-panel-keys");
        let mut v = viewer(&dir, &Options::default());
        v.clock.pause();
        v.clock.seek((f::LAUNCH + 40) as f64);
        press(&mut v, "i");
        press(&mut v, "f");
        let kinds = |v: &Viewer| {
            [panels::Side::Left, panels::Side::Right]
                .map(|side| v.panels.slot(side).map(|p| (p.kind, p.subject)))
        };
        assert_eq!(
            kinds(&v),
            [Some((Kind::Thought, 0)), Some((Kind::Telemetry, 0))]
        );
        // Unpinned panels follow the selected aircraft.
        press(&mut v, "Tab");
        assert_eq!(
            kinds(&v),
            [Some((Kind::Thought, 1)), Some((Kind::Telemetry, 1))]
        );
        // Pressing the key again closes that panel.
        press(&mut v, "f");
        assert_eq!(kinds(&v)[1], None);
        // G: the selected aircraft's missile in flight, or why not.
        press(&mut v, "g");
        assert_eq!(kinds(&v)[1], Some((Kind::Guidance, f::MISSILE)));
        press(&mut v, "Tab");
        press(&mut v, "g");
        assert!(
            v.toast
                .as_ref()
                .unwrap()
                .0
                .contains("has no missile in flight")
        );
        // The wheel over a panel scrolls it rather than zooming.
        let size = [1280, 960];
        v.pointer(Some([100., 400.]), [100., 400.], size, [0.; 2]);
        let zoom = v.zoom;
        v.wheel(-1);
        assert_eq!(v.zoom, zoom);
        v.pointer(Some([640., 400.]), [640., 400.], size, [0.; 2]);
        v.wheel(-1);
        assert!(v.zoom < zoom);
        // M opens the menu on the selected aircraft; X closes every panel.
        press(&mut v, "m");
        assert_eq!(v.menu.as_ref().unwrap().target, Target::Aircraft(2));
        press(&mut v, "Escape");
        press(&mut v, "c");
        press(&mut v, "x");
        assert!(v.panels.is_empty() && !v.ui.comms);
        // Clicking a panel's close button closes it.
        press(&mut v, "i");
        let placement = Placement::new(size);
        let close = [
            (6. + 254. - 9.) * placement.scale(),
            (28. + 6.) * placement.scale(),
        ];
        v.left(true, Some(close), size);
        v.left(false, Some(close), size);
        assert!(v.panels.is_empty());
    }

    #[test]
    fn hud_subtitles_are_what_the_hud_showed() {
        use vocab::{field, kind, outcome, source};
        let hud = |result: &str| Event::new(kind::COMMS_HUD).with(field::OUTCOME, result);
        assert!(hud_shown(&hud(outcome::DELIVERED)));
        assert!(hud_shown(&hud(outcome::QUEUED)));
        assert!(hud_shown(&Event::new(kind::COMMS_HUD)));
        assert!(!hud_shown(&hud(outcome::SUPPRESSED)));
        assert!(!hud_shown(&hud(outcome::DROPPED)));
        // The journal's copy of an AI text line waiting for the HUD.
        assert!(!hud_shown(
            &hud(outcome::QUEUED).with(field::SOURCE, source::HUD)
        ));
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
            panels: Request::parse("thought, telemetry,menu").unwrap(),
        };
        let mut v = viewer(&dir, &options);
        assert!(v.panels.comms_open());
        v.open_requests(&camera_on(2, 300));
        assert_eq!(v.panels.find(Kind::Thought, 2), Some(panels::Side::Left));
        assert_eq!(v.panels.find(Kind::Telemetry, 2), Some(panels::Side::Right));
        assert_eq!(v.menu.as_ref().unwrap().target, Target::Aircraft(2));
        assert!(Request::parse("thought,sparkles").is_err());
        assert_eq!(Request::parse("").unwrap(), []);
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
