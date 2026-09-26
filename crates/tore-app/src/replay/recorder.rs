//! The mission recorder: every simulation tick of a flight, captured into a
//! `tore-replay` recording that a background thread encodes and writes.
//!
//! Per tick: [`Recorder::start_tick`] opens it, [`Recorder::begin`] runs
//! right after `Combat::advance_render` and turns the tick's render snapshot
//! and flight data into a frame, noting what changed (launches, hits, kills,
//! crashes, departures, AI activity and so on). Hooks later in the tick add
//! events with [`Recorder::note`] and the other noting methods, and
//! [`Recorder::end`] closes the frame. A closed frame waits until the next
//! tick opens, so anything noted between ticks (a pause, a bookmark, an
//! order) lands on the tick the player was looking at.
//!
//! Live flight never waits for the disk: frames travel to the writer on a
//! bounded queue, and if it is ever full the frame is dropped and the next
//! one carries a `system.gap` event. A headless probe recording, which has
//! no frame rate to protect, waits for the writer instead
//! ([`Recorder::wait_for_writer`]), so it never has gaps however loaded the
//! machine is. Nothing here feeds back into flight;
//! every read is of state the tick already computed. Opinionated addition
//! requested by John on 2026-09-26; see docs/REPLAYS.md.
//!
//! The reasons behind decisions live in two child modules: [`why`] turns the
//! AI's and the flight model's write-only records into reason events and
//! display trees, and [`journal`] turns the AI message journal and the
//! communication journal into `comms.*` events. A third, [`devices`], turns
//! combat's write-only notes of released chaff and flares and the player's
//! decoy rolls into entries a replay flies the devices from.
mod devices;
mod journal;
mod why;

use super::convert::{self, EffectWatch, FlightData, Presentation};
use super::trees;
use crate::{
    ai_wings::AiWings,
    combat::{self, CommandNote},
    flight, flight_ui,
    render_snapshot::RenderSnapshot,
    terrain,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::thread::JoinHandle;
use tore_formats::flight_model::departure::DepartureMode;
use tore_replay::{
    self as replay, Event, Frame,
    vocab::{field, kind, outcome},
};
use tore_sim::combat::{ledger, live};

/// Frames the writer may fall behind by before frames are dropped: two
/// seconds of flight.
const QUEUE_FRAMES: usize = 240;
/// Knots per foot per second.
const KT_PER_FPS: f64 = 1. / 1.687_81;

enum Message {
    Aircraft(Box<replay::AircraftInfo>),
    Weapon(Box<replay::WeaponInfo>),
    Frame(Box<Frame>),
    Finish(Box<replay::Footer>),
}

/// The writer thread: it returns the finished recording's path.
type WriterThread = JoinHandle<Result<PathBuf, String>>;

/// Starts the writer thread. It owns the file; when the game drops the
/// recorder without finishing, the writer keeps its `.partial` file.
fn spawn(writer: replay::Writer) -> std::io::Result<(SyncSender<Message>, WriterThread)> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(QUEUE_FRAMES);
    let thread = std::thread::Builder::new()
        .name("tore-recorder".into())
        .spawn(move || {
            let mut writer = writer;
            let mut refused = 0u64;
            for message in receiver {
                let result = match message {
                    Message::Aircraft(info) => writer.register_aircraft(&info),
                    Message::Weapon(info) => writer.register_weapon(&info),
                    Message::Frame(frame) => writer.push(&frame),
                    Message::Finish(footer) => {
                        if refused > 0 {
                            log::info!("Recording: {refused} items were refused while writing");
                        }
                        return writer.finish(&footer).map_err(|e| e.to_string());
                    }
                };
                if let Err(error) = result {
                    if refused == 0 {
                        log::info!("Recording: {error}");
                    }
                    refused += 1;
                }
            }
            Err("the recording was not finished; its .partial file keeps what was written".into())
        })?;
    Ok((sender, thread))
}

/// Everything [`Recorder::begin`] reads for one tick.
pub struct Tick<'a> {
    /// The tick's picture, just taken by `Combat::advance_render`.
    pub snapshot: &'a RenderSnapshot,
    pub combat: &'a combat::Combat,
    /// The player's flight state after the tick, and before it.
    pub flight: &'a flight::State,
    pub previous: &'a flight::State,
    /// The player's controls for the tick.
    pub pilot: &'a flight::PilotInput,
    pub wings: Option<&'a AiWings>,
    pub world: &'a terrain::World,
    /// Combat's events for the tick.
    pub events: &'a [live::Event],
    /// Shot outcomes the ledger resolved during the tick.
    pub outcomes: &'a [ledger::Outcome],
    /// The AI message journal, drained for this tick (`None` without AI).
    pub journal: Option<&'a tore_sim::ai::thought::JournalBatch>,
}

/// What the recorder remembers about one aircraft between ticks.
#[derive(Clone, Debug, Default)]
struct Watch {
    seen: bool,
    departure: Option<DepartureMode>,
    on_ground: bool,
    engine: bool,
    fuel_out: bool,
    crashed: bool,
    /// Its end (a crash or the wreck's end) was already reported.
    ended: bool,
    wreck: Option<tore_sim::wreck::Phase>,
    escape: Option<tore_sim::ejection::Phase>,
    pilot_dead: bool,
    hp: i32,
    sections: [i32; replay::SECTION_COUNT],
}

/// What the recorder remembers about one projectile between ticks.
#[derive(Clone, Copy, Debug)]
struct Shot {
    owner: u32,
    weapon: u32,
    target: Option<u32>,
    status: Option<u8>,
    position: [f64; 3],
    /// Its lost track was already reported.
    lost: bool,
    /// When it lost track, for the reason its outcome gives.
    lost_at: Option<u64>,
}

/// The seeker tone the player hears, without its loudness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tone {
    radar: bool,
    ground: bool,
    locked: bool,
}

impl Tone {
    fn name(self) -> &'static str {
        use tore_replay::vocab::tone;
        match (self.radar, self.locked, self.ground) {
            (true, true, _) => tone::RADAR_LOCK,
            (true, false, _) => tone::RADAR_SEARCH,
            (false, true, _) => tone::INFRARED_LOCK,
            (false, false, true) => tone::GROUND,
            (false, false, false) => tone::INFRARED_SEARCH,
        }
    }
}

/// The smallest change in seeker tone loudness that is recorded, so a tone
/// that swells with the seeker's quality makes a few entries, not one a
/// tick (agent decision, 2026-09-26).
const TONE_STEP: f64 = 0.05;

/// The `audio.tone` entry for the player's seeker tone, `now` against what
/// was `last` recorded, each with its loudness: when it starts, stops,
/// changes, or its loudness moves by [`TONE_STEP`] or more.
fn tone_event(last: Option<(Tone, f64)>, now: Option<(Tone, f64)>) -> Option<Event> {
    let event = |tone: Tone, on: bool| {
        Event::new(kind::AUDIO_TONE)
            .with_subject(0)
            .with(field::TONE, tone.name())
            .with(field::ON, on)
    };
    match (last, now) {
        (Some((was, then)), Some((tone, strength)))
            if was == tone && (strength - then).abs() < TONE_STEP =>
        {
            None
        }
        (_, Some((tone, strength))) => Some(
            event(tone, true)
                .with(field::STRENGTH, trees::round(strength, 2))
                .with(field::SURFACE, tone.ground),
        ),
        (Some((was, _)), None) => Some(event(was, false)),
        (None, None) => None,
    }
}

/// Pause, time compression and cheats, compared each frame.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Session {
    frozen: bool,
    time_scale: f64,
    cheats: tore_sim::cheats::Cheats,
}

/// Records one flight. See the module documentation.
pub struct Recorder {
    path: PathBuf,
    sender: Option<SyncSender<Message>>,
    thread: Option<WriterThread>,
    /// Messages the queue could not take, sent before the next frame.
    backlog: Vec<Message>,
    /// The frame being built between `begin` and `end`.
    frame: Option<Frame>,
    /// The last closed frame, still open to notes made between ticks.
    held: Option<Frame>,
    /// Notes made during a tick before its frame begins.
    early: Vec<Event>,
    registered: BTreeSet<u32>,
    weapons: BTreeMap<String, u32>,
    effects: EffectWatch,
    surface: BTreeMap<u32, i32>,
    watches: BTreeMap<u32, Watch>,
    shots: BTreeMap<u32, Shot>,
    /// Dropped frames not yet reported: first and last tick, events lost.
    gap: Option<(u64, u64, usize)>,
    /// Wait for a full queue instead of dropping frames (headless runs).
    wait: bool,
    /// The seeker tone and its loudness as last recorded.
    tone: Option<(Tone, f64)>,
    stall: Option<&'static str>,
    danger: bool,
    session: Option<Session>,
    bookmarks: u32,
    frames: u64,
    last_tick: Option<u64>,
    /// Copies of the comms and audio entries written, for live flight's
    /// Comms panel, until it takes them.
    comms: Vec<replay::TimedEvent>,
    /// Live flight's debug panels are showing: the display trees written
    /// are copied for them too.
    live: bool,
    /// Copies of the display trees written while `live`, until taken.
    trees: Vec<(u64, replay::TreeSample)>,
    /// Every registered identity, for names in reasons and trees.
    infos: BTreeMap<u32, replay::AircraftInfo>,
    /// What the reason events and trees remember between ticks.
    why: why::Why,
    /// The AI aircraft's chaff and flares left as the last tick was
    /// recorded, for the releases its end collects.
    dispensers: devices::Dispensers,
}

/// Comms and audio entries kept for live flight between its looks.
const COMMS_TAP: usize = 1_024;
/// Display trees kept for live flight between its looks.
const TREES_TAP: usize = 4_096;

impl Recorder {
    /// Starts writing a recording to `path`, which must not exist yet.
    /// `roster` names every aircraft known at the start.
    pub fn start(
        path: PathBuf,
        header: &replay::Header,
        roster: &[replay::AircraftInfo],
    ) -> Result<Self, String> {
        let writer = replay::Writer::create(&path, header).map_err(|e| e.to_string())?;
        let partial = writer.partial_path().to_path_buf();
        let (sender, thread) = spawn(writer).map_err(|e| e.to_string())?;
        log::info!("Recording to {}", partial.display());
        Ok(Self::with(path, sender, Some(thread), roster))
    }

    /// A recorder whose queue the caller reads, with no file, for tests.
    #[cfg(test)]
    fn detached(
        capacity: usize,
        roster: &[replay::AircraftInfo],
    ) -> (Self, std::sync::mpsc::Receiver<Message>) {
        let (sender, receiver) = std::sync::mpsc::sync_channel(capacity);
        (Self::with(PathBuf::new(), sender, None, roster), receiver)
    }

    fn with(
        path: PathBuf,
        sender: SyncSender<Message>,
        thread: Option<WriterThread>,
        roster: &[replay::AircraftInfo],
    ) -> Self {
        let mut recorder = Self {
            path,
            sender: Some(sender),
            thread,
            backlog: Vec::new(),
            frame: None,
            held: None,
            early: Vec::new(),
            registered: BTreeSet::new(),
            weapons: BTreeMap::new(),
            effects: EffectWatch::default(),
            surface: BTreeMap::new(),
            watches: BTreeMap::new(),
            shots: BTreeMap::new(),
            gap: None,
            wait: false,
            tone: None,
            stall: None,
            danger: false,
            session: None,
            bookmarks: 0,
            frames: 0,
            last_tick: None,
            comms: Vec::new(),
            live: false,
            trees: Vec::new(),
            infos: BTreeMap::new(),
            why: why::Why::default(),
            dispensers: devices::Dispensers::new(),
        };
        for info in roster {
            recorder.register(info.clone());
        }
        recorder
    }

    /// Where the finished recording will be, so cleanup can spare it.
    #[allow(dead_code)] // Read by the Replays screen.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn register(&mut self, info: replay::AircraftInfo) {
        if self.registered.insert(info.id) {
            self.infos.insert(info.id, info.clone());
            self.send(Message::Aircraft(Box::new(info)));
        }
    }

    /// An aircraft's label for reasons: `You`, `Enemy 2-1`.
    fn who(&self, id: u32) -> String {
        who(&self.infos, id)
    }

    /// Makes the recorder wait for a full queue instead of dropping frames.
    /// For headless runs only: live flight must never wait for the disk.
    pub fn wait_for_writer(&mut self) {
        self.wait = true;
    }

    /// Queues a message for the writer and says whether it went. Normally it
    /// never waits: registrations that do not fit wait in the backlog and
    /// count as sent; a frame that does not fit is not sent. After
    /// [`Recorder::wait_for_writer`] every message waits for room instead.
    fn send(&mut self, message: Message) -> bool {
        let Some(sender) = &self.sender else {
            return false;
        };
        if self.wait {
            let mut pending = std::mem::take(&mut self.backlog);
            pending.push(message);
            for message in pending {
                if sender.send(message).is_err() {
                    self.sender = None;
                    return false;
                }
            }
            return true;
        }
        while !self.backlog.is_empty() {
            match sender.try_send(self.backlog.remove(0)) {
                Ok(()) => {}
                Err(TrySendError::Full(message)) => {
                    self.backlog.insert(0, message);
                    break;
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.sender = None;
                    return false;
                }
            }
        }
        let message = if self.backlog.is_empty() {
            match sender.try_send(message) {
                Ok(()) => return true,
                Err(TrySendError::Full(message)) => message,
                Err(TrySendError::Disconnected(_)) => {
                    self.sender = None;
                    return false;
                }
            }
        } else {
            message
        };
        match message {
            Message::Frame(_) => false,
            other => {
                self.backlog.push(other);
                true
            }
        }
    }

    /// Notes the ticks dropped so far at the start of `frame`.
    fn mark_gap(&self, frame: &mut Frame) {
        if let Some((from, to, lost)) = self.gap {
            frame.events.insert(
                0,
                Event::new(kind::SYSTEM_GAP)
                    .with(field::FROM, from as i64)
                    .with(field::TO, to as i64)
                    .with("events_lost", lost as i64)
                    .with_text("the recorder fell behind and dropped these ticks"),
            );
        }
    }

    /// Hands the held frame to the writer without waiting. If the queue is
    /// full the frame is dropped and joins the gap the next frame reports.
    fn release_held(&mut self) {
        let Some(mut frame) = self.held.take() else {
            return;
        };
        let (tick, events) = (frame.tick, frame.events.len());
        // Live flight's Comms panel lists what the recording holds, even
        // when the writer falls behind.
        self.comms.extend(
            frame
                .events
                .iter()
                .filter(|e| crate::replay::panels::Channel::of(&e.kind).is_some())
                .map(|e| replay::TimedEvent {
                    tick,
                    event: e.clone(),
                }),
        );
        if self.comms.len() > COMMS_TAP {
            self.comms.drain(..self.comms.len() - COMMS_TAP);
        }
        // Its display trees too while the panels show, so a live panel
        // shows the very tree the recording keeps.
        if self.live {
            self.trees
                .extend(frame.trees.iter().map(|tree| (tick, tree.clone())));
            if self.trees.len() > TREES_TAP {
                self.trees.drain(..self.trees.len() - TREES_TAP);
            }
        }
        self.mark_gap(&mut frame);
        fit(&mut frame);
        if self.send(Message::Frame(Box::new(frame))) {
            self.gap = None;
        } else if self.sender.is_some() {
            let (from, _, lost) = self.gap.unwrap_or((tick, tick, 0));
            self.gap = Some((from, tick, lost + events));
        }
    }

    /// The comms and audio entries written since the last call, oldest
    /// first, for live flight's Comms panel. Only the newest 1,024 wait.
    pub fn take_comms(&mut self) -> Vec<replay::TimedEvent> {
        std::mem::take(&mut self.comms)
    }

    /// Whether live flight's debug panels are showing, so the display trees
    /// written are kept for them.
    pub fn set_live(&mut self, live: bool) {
        self.live = live;
        if !live {
            self.trees.clear();
        }
    }

    /// The display trees written since the last call while live, oldest
    /// first, each with its tick.
    pub fn take_trees(&mut self) -> Vec<(u64, replay::TreeSample)> {
        std::mem::take(&mut self.trees)
    }

    /// Adds an event: to the tick being recorded, or between ticks to the
    /// last one, or before the first frame to the first.
    pub fn note(&mut self, event: Event) {
        if let Some(frame) = &mut self.frame {
            frame.events.push(event);
        } else if let Some(held) = &mut self.held {
            held.events.push(event);
        } else {
            self.early.push(event);
        }
    }

    /// Opens a simulation tick: the previous frame is written, and anything
    /// noted from now on belongs to this tick. Notes that `ui` and `combat`
    /// collected between ticks go on the previous frame first.
    pub fn start_tick(
        &mut self,
        ui: Option<&mut flight_ui::FlightUi>,
        combat: &mut combat::Combat,
    ) {
        self.collect(ui, combat);
        self.release_held();
    }

    /// Cockpit messages, player commands, released chaff and flares and the
    /// player's decoy rolls collected since the last look.
    fn collect(&mut self, ui: Option<&mut flight_ui::FlightUi>, combat: &mut combat::Combat) {
        if let Some(ui) = ui {
            for (text, shown) in ui.take_notes() {
                let (result, reason) = match shown {
                    flight_ui::Shown::Now => (outcome::DELIVERED, ""),
                    flight_ui::Shown::Repeated => (
                        outcome::DELIVERED,
                        "the same message was on screen: it moved to the bottom with a fresh timer",
                    ),
                    flight_ui::Shown::PushedOff => (
                        outcome::REPLACED,
                        "seven newer lines pushed it off the screen",
                    ),
                };
                let mut event = Event::new(kind::COMMS_HUD)
                    .with(field::OUTCOME, result)
                    .with_text(text);
                if !reason.is_empty() {
                    event = event.with(field::REASON, reason);
                }
                self.note(event);
            }
        }
        let designated = combat.state.designated();
        for note in combat.take_notes() {
            let (name, text) = match note {
                CommandNote::Command(command) => (
                    crate::combat_tape::command_name(command),
                    format!("{command:?}"),
                ),
                CommandNote::Release => ("release".to_owned(), "trigger released".to_owned()),
            };
            let mut event = Event::new(kind::PLAYER_COMMAND)
                .with_subject(0)
                .with(field::COMMAND, name)
                .with_text(text);
            if let Some(target) = designated {
                event = event.with_object(target);
            }
            self.note(event);
        }
        self.countermeasures(combat);
    }

    /// Closes the tick's frame. It is written when the next tick opens, or
    /// when the recording finishes.
    pub fn end(&mut self, ui: Option<&mut flight_ui::FlightUi>, combat: &mut combat::Combat) {
        self.collect(ui, combat);
        if let Some(frame) = self.frame.take() {
            self.held = Some(frame);
        }
    }

    /// Pause, time compression and cheat changes, checked once per rendered
    /// frame so a pause is noted when it happens.
    pub fn session(&mut self, ui: &flight_ui::FlightUi) {
        let now = Session {
            frozen: ui.frozen(),
            time_scale: ui.time_scale,
            cheats: ui.cheats,
        };
        let Some(before) = self.session.replace(now) else {
            return;
        };
        if before.frozen != now.frozen {
            self.note(Event::new(if now.frozen {
                kind::SYSTEM_PAUSE
            } else {
                kind::SYSTEM_RESUME
            }));
        }
        if before.time_scale != now.time_scale {
            self.note(
                Event::new(kind::SYSTEM_TIME_SCALE)
                    .with(field::SCALE, now.time_scale)
                    .with_text(format!("time {}x", now.time_scale)),
            );
        }
        for (name, was, is) in cheat_changes(&before.cheats, &now.cheats) {
            if was != is {
                self.note(
                    Event::new(kind::SYSTEM_CHEAT)
                        .with(field::CHEAT, name)
                        .with(field::ON, is),
                );
            }
        }
        if before.cheats.enemy_ai != now.cheats.enemy_ai {
            self.note(
                Event::new(kind::SYSTEM_CHEAT)
                    .with(field::CHEAT, "enemy_ai")
                    .with(field::ON, now.cheats.enemy_ai.is_some())
                    .with_text(format!("{:?}", now.cheats.enemy_ai)),
            );
        }
    }

    /// Marks this moment; returns the bookmark's number, from 1.
    pub fn bookmark(&mut self) -> u32 {
        self.bookmarks += 1;
        let number = self.bookmarks;
        self.note(
            Event::new(kind::PLAYER_BOOKMARK)
                .with_subject(0)
                .with("index", i64::from(number))
                .with_text(format!("Bookmark {number}")),
        );
        number
    }

    /// An AI aircraft's pilot ejected; `friendly` wingmen play a cue.
    pub fn wing_ejection(&mut self, id: u32, message: &str, friendly: bool) {
        if friendly {
            self.note(
                Event::new(kind::AUDIO_EJECTION)
                    .with_subject(id)
                    .with(field::SOUND, "^PUNCH.5K")
                    .with_text(message),
            );
        }
    }

    /// Sounds the tick released: combat's emissions and the player's
    /// weapon release sounds, as the tick drained them.
    pub fn sounds(
        &mut self,
        emissions: &[tore_sim::acoustics::Emission],
        releases: &[(&str, &tore_formats::weapons::Weapon)],
    ) {
        for emission in emissions {
            let [x, y, z] = emission.position;
            let mut event = Event::new(kind::AUDIO_EFFECT)
                .with(field::SOUND, format!("{:?}", emission.kind).to_lowercase())
                .with(field::X_FT, x)
                .with(field::Y_FT, y)
                .with(field::Z_FT, z);
            if emission.own {
                event = event.with(field::OWN, true);
            }
            self.note(event);
        }
        for (sound, weapon) in releases {
            let weapon = self.weapon_id(weapon);
            self.note(
                Event::new(kind::AUDIO_RELEASE)
                    .with_subject(0)
                    .with(field::SOUND, *sound)
                    .with(field::WEAPON, replay::Value::Id(weapon)),
            );
        }
    }

    /// The registered id for a weapon type, registering it on first use.
    fn weapon_id(&mut self, weapon: &tore_formats::weapons::Weapon) -> u32 {
        if let Some(id) = self.weapons.get(&weapon.source) {
            return *id;
        }
        let id = self.weapons.len() as u32;
        self.weapons.insert(weapon.source.clone(), id);
        self.send(Message::Weapon(Box::new(convert::weapon_info(id, weapon))));
        id
    }

    /// Records the tick. See the module documentation.
    pub fn begin(&mut self, tick: Tick<'_>) {
        // A frame left open, or one held from the last tick, goes first.
        if let Some(open) = self.frame.take() {
            self.release_held();
            self.held = Some(open);
        }
        self.release_held();
        let snapshot = tick.snapshot;
        let number = snapshot.tick;
        if self.last_tick.is_some_and(|last| number <= last) {
            // A tick can be recorded once only; a repeat means the picture
            // did not advance.
            return;
        }
        self.last_tick = Some(number);
        let mut frame = Frame {
            tick: number,
            events: std::mem::take(&mut self.early),
            ..Frame::default()
        };
        let targets: BTreeMap<u32, &live::Target> = tick
            .combat
            .state
            .targets
            .iter()
            .map(|t| (t.id, t))
            .collect();
        let player_ground = ground_height(tick.world, tick.flight.position);

        // Aircraft: the player first, then every other aircraft in target
        // order. Ground objects keep only their hit points.
        let aircraft: Vec<&crate::render_snapshot::AircraftPose> =
            std::iter::once(&snapshot.player)
                .chain(snapshot.targets.iter().filter(|p| p.aircraft.is_some()))
                .collect();
        for pose in &aircraft {
            if !self.registered.contains(&pose.id) {
                self.register(default_info(pose, tick.wings));
            }
            let data = flight_data(pose, &tick, &targets, player_ground);
            frame.aircraft.push(convert::aircraft_state(pose, &data));
        }
        for target in snapshot.targets.iter().filter(|p| p.aircraft.is_none()) {
            if self.surface.insert(target.id, target.damage.hp) != Some(target.damage.hp) {
                frame.surface_hp.push((target.id, target.damage.hp));
            }
        }
        if number.is_multiple_of(replay::TICKS_PER_SECOND) {
            frame.checksum = Some(replay::state_checksum(&frame.aircraft));
        }

        // Weapons in flight.
        let live_shots: BTreeMap<u32, &live::Projectile> = tick
            .combat
            .state
            .projectiles
            .iter()
            .map(|p| (p.id, p))
            .collect();
        let config = tick.combat.state.configuration();
        let mut shots = BTreeMap::new();
        for pose in &snapshot.projectiles {
            let simulated = live_shots.get(&pose.id);
            let weapon = simulated.map(|p| p.weapon(config));
            let weapon_id = match weapon {
                Some(weapon) => self.weapon_id(weapon),
                None => self.weapons.get(&pose.weapon).copied().unwrap_or(u32::MAX),
            };
            let seeker = simulated
                .and_then(|p| p.guidance.as_ref())
                .map(convert::seeker);
            frame.projectiles.push(convert::projectile_state(
                pose,
                weapon_id,
                simulated.map_or(0, |p| p.age),
                seeker,
            ));
            shots.insert(
                pose.id,
                Shot {
                    owner: pose.owner,
                    weapon: weapon_id,
                    target: pose.target,
                    status: seeker.map(|s| s.status),
                    position: pose.position,
                    lost: self.shots.get(&pose.id).is_some_and(|shot| shot.lost),
                    lost_at: self.shots.get(&pose.id).and_then(|shot| shot.lost_at),
                },
            );
        }
        frame.debris = convert::debris_states(&snapshot.debris);
        frame.escapees = snapshot.pilots.iter().map(convert::escapee_state).collect();
        frame.new_effects = self.effects.started(&snapshot.effects);
        frame.new_puffs = new_puffs(tick.combat);

        let mut events = Vec::new();
        // Decoy rolls first: they explain this tick's spoofed outcomes.
        self.decoys(&tick, &frame, &mut events);
        self.dispensers = devices::dispensers(tick.wings);
        self.weapon_events(&tick, &frame, &mut shots, &live_shots, &mut events);
        self.aircraft_events(&tick, &frame, &mut events);
        self.cue_events(&tick, player_ground, &mut events);
        self.shots = shots;
        // The reasons behind the tick and its display trees.
        self.explain(&tick, &mut frame, &mut events);
        frame.events.extend(events);
        self.frames += 1;
        self.frame = Some(frame);
    }

    fn weapon_events(
        &mut self,
        tick: &Tick<'_>,
        frame: &Frame,
        shots: &mut BTreeMap<u32, Shot>,
        live_shots: &BTreeMap<u32, &live::Projectile>,
        events: &mut Vec<Event>,
    ) {
        let state_of = |id: u32| frame.aircraft.iter().find(|a| a.id == id);
        // What a round with no target of its own was fired at: the AI's
        // current target, or the player's designated one.
        let intended = |owner: u32| {
            if owner == 0 {
                tick.combat.state.designated()
            } else {
                tick.wings
                    .and_then(|w| w.mission().actor(owner))
                    .and_then(|a| a.controller().target())
            }
        };
        for (id, shot) in shots.iter_mut() {
            let Some(previous) = self.shots.get(id) else {
                // A launch, with its geometry at release.
                let mut event = Event::new(kind::WEAPON_LAUNCH)
                    .with_subject(shot.owner)
                    .with(field::PROJECTILE, replay::Value::Id(*id))
                    .with(field::WEAPON, replay::Value::Id(shot.weapon));
                if let Some(weapon) = live_shots
                    .get(id)
                    .map(|p| p.weapon(tick.combat.state.configuration()))
                {
                    event = event.with(
                        field::CLASS,
                        convert::weapon_info(shot.weapon, weapon).class.name(),
                    );
                }
                let mode = live_shots
                    .get(id)
                    .map_or("unguided", |p| match p.guidance.as_ref() {
                        Some(guidance)
                            if guidance.mode
                                == tore_sim::combat::missiles::LaunchMode::Boresight =>
                        {
                            "boresight"
                        }
                        Some(_) => "cued",
                        None if live::is_gun(p.weapon(tick.combat.state.configuration())) => "gun",
                        None => "unguided",
                    });
                event = event.with(field::MODE, mode);
                let aim = shot.target.or_else(|| intended(shot.owner));
                if let Some(target) = aim {
                    event = event.with_object(target);
                }
                if let (Some(shooter), Some(target)) =
                    (state_of(shot.owner), aim.and_then(state_of))
                {
                    event = geometry(event, shooter, target);
                } else if let Some(shooter) = state_of(shot.owner) {
                    event = event
                        .with(field::SHOOTER_ALT_FT, shooter.position[1])
                        .with(field::SHOOTER_SPEED_KT, speed(shooter) * KT_PER_FPS);
                }
                events.push(event);
                continue;
            };
            // The shot lost its target, once per shot: the target let go
            // while it flew on, or its seeker gave up.
            const LOST: u8 = tore_sim::combat::missiles::seeker::Status::Lost as u8;
            let let_go = previous.target.is_some() && shot.target.is_none();
            let seeker_lost = shot.status == Some(LOST) && previous.status != Some(LOST);
            if !shot.lost && (let_go || seeker_lost) {
                shot.lost = true;
                shot.lost_at = Some(frame.tick);
                let target = previous.target.or(shot.target);
                let mut event = Event::new(kind::WEAPON_TRACK_LOST)
                    .with_subject(shot.owner)
                    .with(field::PROJECTILE, replay::Value::Id(*id));
                if let Some(target) = target {
                    event = event.with_object(target);
                    if let Some(state) = state_of(target) {
                        event =
                            event.with(field::RANGE_FT, distance(shot.position, state.position));
                    }
                }
                if seeker_lost {
                    event = event.with("seeker", "lost");
                }
                let decoyed = self.why.decoyed(*id);
                let reason = if let Some(roll) = decoyed {
                    format!(
                        "decoyed by {} from {}",
                        why::decoy_name(roll.class),
                        self.who(roll.releaser)
                    )
                } else if seeker_lost {
                    "the seeker lost the target".to_owned()
                } else if target
                    .and_then(state_of)
                    .is_none_or(|state| !state.flags.alive)
                {
                    "its target was destroyed or is gone".to_owned()
                } else {
                    "it could not hold the target: seeker limits, terrain or guidance time"
                        .to_owned()
                };
                events.push(event.with(field::REASON, reason));
            }
        }
        // Seeker milestones combat reported this tick.
        for event in tick.events {
            let (kind, id) = match event {
                live::Event::SeekerActivated(id) => (kind::WEAPON_SEEKER_ACTIVE, *id),
                live::Event::Pitbull(id) => (kind::WEAPON_PITBULL, *id),
                _ => continue,
            };
            let Some(shot) = shots.get(&id) else {
                continue;
            };
            let mut out = Event::new(kind)
                .with_subject(shot.owner)
                .with(field::PROJECTILE, replay::Value::Id(id))
                .with(field::WEAPON, replay::Value::Id(shot.weapon));
            if let Some(target) = shot.target {
                out = out.with_object(target);
                if let Some(state) = state_of(target) {
                    out = out.with(field::RANGE_FT, distance(shot.position, state.position));
                }
            }
            events.push(out);
        }
        // How each shot ended.
        for resolved in tick.outcomes {
            let (result, damage) = match resolved.resolution {
                ledger::Resolution::Hit(damage) => (outcome::HIT, Some(damage)),
                ledger::Resolution::Missed => (outcome::MISSED, None),
                ledger::Resolution::Spoofed => (outcome::SPOOFED, None),
                ledger::Resolution::Jammed => (outcome::JAMMED, None),
            };
            let mut event = Event::new(kind::WEAPON_OUTCOME)
                .with_subject(resolved.key.owner)
                .with(field::PROJECTILE, replay::Value::Id(resolved.projectile))
                .with(field::RESULT, result);
            if let Some(target) = resolved.key.aim {
                event = event.with_object(target);
            }
            if let Some(damage) = damage {
                event = event.with(field::DAMAGE, i64::from(damage));
            }
            let shot = self.shots.get(&resolved.projectile);
            if let Some(weapon) = shot.map(|shot| shot.weapon) {
                event = event.with(field::WEAPON, replay::Value::Id(weapon));
            }
            if let Some((miss, _)) = self.why.closest(resolved.projectile)
                && resolved.resolution == ledger::Resolution::Missed
            {
                event = event.with(field::MISS_FT, trees::round(miss, 1));
            }
            let reason = match resolved.resolution {
                ledger::Resolution::Spoofed => self.why.decoyed(resolved.projectile).map(|roll| {
                    format!(
                        "decoyed by {} from {} ({})",
                        why::decoy_name(roll.class),
                        self.who(roll.releaser),
                        roll.draw.map_or_else(String::new, |d| trees::draw_text(&d))
                    )
                }),
                ledger::Resolution::Jammed => {
                    Some("the target's jammer or countermeasures deceived it".to_owned())
                }
                ledger::Resolution::Missed => shot
                    .and_then(|shot| shot.lost_at)
                    .map(|at| format!("it lost track of its target at {}", trees::clock(at))),
                ledger::Resolution::Hit(_) => None,
            };
            if let Some(reason) = reason {
                event = event.with(field::REASON, reason);
            }
            events.push(event);
        }
        // Rounds that hit the ground: each ground impact effect this tick,
        // matched to the nearest weapon that vanished.
        let mut vanished: Vec<(u32, Shot)> = self
            .shots
            .iter()
            .filter(|(id, _)| !shots.contains_key(id))
            .map(|(id, shot)| (*id, *shot))
            .collect();
        for effect in frame
            .new_effects
            .iter()
            .filter(|e| e.kind == replay::EffectKind::Ground)
        {
            let nearest = vanished
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    distance(a.1.1.position, effect.position)
                        .total_cmp(&distance(b.1.1.position, effect.position))
                })
                .map(|(i, _)| i);
            let [x, y, z] = effect.position;
            let mut event = Event::new(kind::COMBAT_GROUND_IMPACT)
                .with(field::X_FT, x)
                .with(field::Y_FT, y)
                .with(field::Z_FT, z);
            if let Some(i) = nearest {
                let (id, shot) = vanished.remove(i);
                event = event
                    .with_subject(shot.owner)
                    .with(field::PROJECTILE, replay::Value::Id(id))
                    .with(field::WEAPON, replay::Value::Id(shot.weapon));
            }
            events.push(event);
        }
    }

    fn aircraft_events(&mut self, tick: &Tick<'_>, frame: &Frame, events: &mut Vec<Event>) {
        let ledger = &tick.combat.state.ledger;
        let hit_by = |victim: u32| -> Option<&ledger::Outcome> {
            let hits = tick
                .outcomes
                .iter()
                .filter(|o| matches!(o.resolution, ledger::Resolution::Hit(_)));
            if victim == 0 {
                hits.clone()
                    .find(|o| o.key.aim == Some(0))
                    .or_else(|| hits.clone().next())
            } else {
                let owner = ledger.credit(victim).map(|kill| kill.owner);
                hits.clone()
                    .find(|o| Some(o.key.owner) == owner && o.key.aim == Some(victim))
                    .or_else(|| hits.clone().find(|o| Some(o.key.owner) == owner))
            }
        };
        let subsystems: Vec<&str> = tick
            .events
            .iter()
            .filter_map(|e| match e {
                live::Event::SubsystemDamaged(index) => {
                    Some(tore_sim::aircraft_systems::label(*index))
                }
                _ => None,
            })
            .collect();
        for state in &frame.aircraft {
            let id = state.id;
            let actor = tick.wings.and_then(|wings| wings.mission().actor(id));
            let flight = if id == 0 {
                Some(tick.flight)
            } else {
                actor.map(|a| a.flight())
            };
            let mut watch = self.watches.remove(&id).unwrap_or_default();
            let first = !watch.seen;
            watch.seen = true;
            let speed_kt = speed(state) * KT_PER_FPS;
            let hp_before = if first { state.hp } else { watch.hp };

            // Damage landed.
            if !first && state.hp < watch.hp {
                let hit = hit_by(id);
                let attacker = if id == 0 {
                    hit.map(|o| o.key.owner)
                } else {
                    ledger
                        .credit(id)
                        .map(|kill| kill.owner)
                        .or(hit.map(|o| o.key.owner))
                };
                let mut event = Event::new(kind::COMBAT_HIT)
                    .with_object(id)
                    .with(field::DAMAGE, i64::from(watch.hp - state.hp))
                    .with(field::HP_AFTER, i64::from(state.hp));
                if let Some(attacker) = attacker {
                    event = event.with_subject(attacker);
                }
                if let Some(hit) = hit {
                    event = event.with(field::PROJECTILE, replay::Value::Id(hit.projectile));
                    if let Some(weapon) = self.shots.get(&hit.projectile).map(|s| s.weapon) {
                        event = event.with(field::WEAPON, replay::Value::Id(weapon));
                    }
                }
                let section = (0..replay::SECTION_COUNT)
                    .filter(|i| state.sections[*i] > watch.sections[*i])
                    .max_by_key(|i| state.sections[*i] - watch.sections[*i]);
                if let Some(section) = section {
                    event = event.with(field::SECTION, section as i64);
                }
                if id == 0 && !subsystems.is_empty() {
                    event = event.with("subsystems", subsystems.join(", "));
                }
                events.push(event);
            }
            // Destroyed in combat.
            let destroyed = tick.events.iter().any(|e| match e {
                live::Event::Destroyed(victim) => *victim == id,
                live::Event::PlayerDestroyed => id == 0,
                _ => false,
            });
            if destroyed {
                let killer = if id == 0 {
                    hit_by(0).map(|o| o.key.owner)
                } else {
                    ledger.credit(id).map(|kill| kill.owner)
                };
                let mut event = Event::new(kind::COMBAT_DESTROYED).with_subject(id);
                if let Some(killer) = killer {
                    event = event.with_object(killer);
                }
                if let Some(hit) = hit_by(id) {
                    event = event.with(field::PROJECTILE, replay::Value::Id(hit.projectile));
                    if let Some(weapon) = self.shots.get(&hit.projectile).map(|s| s.weapon) {
                        event = event.with(field::WEAPON, replay::Value::Id(weapon));
                    }
                }
                events.push(event);
            }
            watch.hp = state.hp;
            watch.sections = state.sections;

            if let Some(f) = flight {
                // Departure mode, stalls and spins.
                let mode = f.maneuver.departure;
                if !first && mode != watch.departure {
                    let name = |m: Option<DepartureMode>| {
                        m.map_or("none".to_owned(), |m| format!("{m:?}").to_lowercase())
                    };
                    events.push(
                        Event::new(kind::FLIGHT_DEPARTURE)
                            .with_subject(id)
                            .with(field::FROM, name(watch.departure))
                            .with(field::TO, name(mode)),
                    );
                    for (kind, entered) in [
                        (kind::FLIGHT_STALL, DepartureMode::Stalled),
                        (kind::FLIGHT_SPIN, DepartureMode::Spinning),
                    ] {
                        let was = watch.departure == Some(entered);
                        let is = mode == Some(entered);
                        if was != is {
                            let mut event = Event::new(kind)
                                .with_subject(id)
                                .with(field::ON, is)
                                .with(field::SPEED_KT, speed_kt);
                            if kind == kind::FLIGHT_SPIN && is {
                                let yaw = f.maneuver.body_rates_rad_per_second[2];
                                event = event.with(
                                    field::DIRECTION,
                                    if yaw >= 0. { "right" } else { "left" },
                                );
                            }
                            events.push(event);
                        }
                    }
                }
                watch.departure = mode;
                let alive = !f.crashed && f.escape.is_none() && !f.systems.pilot.dead;
                // Engine and fuel.
                let engine = f.engine && f.fuel > 0.;
                if !first && watch.engine && !engine && alive && state.hp > 0 {
                    let commanded = id == 0
                        && tick.pilot.commands.iter().any(|c| {
                            matches!(
                                c,
                                flight::PilotCommand::Toggle(flight::Switch::Engine)
                                    | flight::PilotCommand::Set(flight::Switch::Engine, false)
                            )
                        });
                    let mut event = Event::new(kind::AIRCRAFT_FLAMEOUT).with_subject(id);
                    if f.fuel <= 0. {
                        event = event.with(field::REASON, "its fuel ran out");
                    } else if commanded {
                        event = event.with(field::REASON, "the pilot switched it off");
                    }
                    events.push(event);
                }
                watch.engine = engine;
                let fuel_out = f.fuel <= 0.;
                if !first && fuel_out && !watch.fuel_out {
                    events.push(Event::new(kind::AIRCRAFT_FUEL_OUT).with_subject(id));
                }
                watch.fuel_out = fuel_out;
                // Wheels on the ground.
                let on_ground = state.flags.on_ground;
                if !first && on_ground != watch.on_ground && alive {
                    events.push(
                        Event::new(if on_ground {
                            kind::AIRCRAFT_LANDED
                        } else {
                            kind::AIRCRAFT_TOOK_OFF
                        })
                        .with_subject(id)
                        .with(field::SINK_FPS, -state.velocity[1]),
                    );
                }
                watch.on_ground = on_ground;
                // Ejection and the pilot.
                let escape = f.escape.as_ref().map(|e| e.phase);
                if !first && watch.escape.is_none() && escape.is_some() {
                    let mut event = Event::new(kind::AIRCRAFT_EJECTED).with_subject(id);
                    let hazard = actor
                        .and_then(|a| a.trace().ejection)
                        .and_then(|e| e.assessment);
                    if let Some(hazard) = hazard {
                        event = event.with(field::REASON, trees::hazard_text(&hazard));
                    } else if id == 0 && tick.pilot.commands.contains(&flight::PilotCommand::Eject)
                    {
                        event = event.with(field::REASON, "you pulled the ejection handle");
                    }
                    events.push(event);
                }
                watch.escape = escape;
                let dead = f.systems.pilot.dead;
                if !first && dead && !watch.pilot_dead {
                    let mut event = Event::new(kind::AIRCRAFT_PILOT_KILLED).with_subject(id);
                    if let Some(killer) = ledger.credit(id).map(|k| k.owner) {
                        event = event.with_object(killer);
                    }
                    events.push(event);
                }
                watch.pilot_dead = dead;
                // The end of the aircraft: flying into the ground or a
                // structure while still flyable, or the wreck of a destroyed
                // one coming down or exploding.
                if !first
                    && f.crashed
                    && !watch.crashed
                    && hp_before > 0
                    && !destroyed
                    && !watch.ended
                {
                    watch.ended = true;
                    events.push(
                        Event::new(kind::AIRCRAFT_CRASHED)
                            .with_subject(id)
                            .with(field::SPEED_KT, speed_kt)
                            .with(field::REASON, "it hit the ground or a structure"),
                    );
                }
                watch.crashed = f.crashed;
            }
            let wreck = convert::wreck_phase(state.wreck_phase);
            if !first && wreck != watch.wreck && !watch.ended {
                let reason = match wreck {
                    Some(tore_sim::wreck::Phase::Grounded) => Some("the wreck hit the ground"),
                    Some(tore_sim::wreck::Phase::Exploded) => Some("the wreck exploded in the air"),
                    _ => None,
                };
                if let Some(reason) = reason {
                    watch.ended = true;
                    events.push(
                        Event::new(kind::AIRCRAFT_CRASHED)
                            .with_subject(id)
                            .with(field::SPEED_KT, speed_kt)
                            .with(field::REASON, reason),
                    );
                }
            }
            watch.wreck = wreck;

            self.watches.insert(id, watch);
        }
    }

    /// Cockpit cues the player hears: seeker tone, stall warning, device
    /// sounds and the ejection seat's warnings and effects.
    fn cue_events(&mut self, tick: &Tick<'_>, player_ground: f64, events: &mut Vec<Event>) {
        let flight = tick.flight;
        let tone = tick
            .combat
            .state
            .seeker_tone(combat::launcher(flight))
            .map(|t| {
                let tone = Tone {
                    radar: t.radar,
                    ground: t.ground,
                    locked: t.locked,
                };
                // The loudness the mixer takes.
                (tone, t.strength.clamp(0., 1.))
            });
        if let Some(event) = tone_event(self.tone, tone) {
            events.push(event);
            self.tone = tone;
        }
        let stall = crate::audio::stall_cue(flight.stall_alert(player_ground));
        if stall != self.stall {
            let mut event = Event::new(kind::AUDIO_STALL_WARNING)
                .with_subject(0)
                .with(field::ON, stall.is_some());
            if let Some(sound) = stall.or(self.stall) {
                event = event.with(field::SOUND, sound);
            }
            events.push(event);
            self.stall = stall;
        }
        let names = ["gear", "flaps", "hook", "brake"];
        for (device, sound) in names
            .into_iter()
            .zip(crate::audio::actuator_cues(tick.previous, flight))
        {
            if let Some(sound) = sound {
                events.push(
                    Event::new(kind::AUDIO_DEVICE)
                        .with_subject(0)
                        .with(field::DEVICE, device)
                        .with(field::SOUND, sound),
                );
            }
        }
        let danger = tore_sim::ejection::assess(flight, |x, z| {
            f64::from(tick.world.height(x as f32, z as f32))
        })
        .is_some();
        let ejection = |sound: &str, text: &str| {
            Event::new(kind::AUDIO_EJECTION)
                .with_subject(0)
                .with(field::SOUND, sound)
                .with_text(text)
        };
        if danger && !self.danger && flight.escape.is_none() && !flight.systems.pilot.dead {
            events.push(ejection("^EJECTX3.5K", "eject warning"));
        }
        self.danger = danger;
        if tick.previous.escape.is_none() && flight.escape.is_some() {
            events.push(ejection("^EJECTNG.5K", "ejecting"));
            events.push(ejection("&EJECT.5K", "ejection seat"));
        }
        let inflating = |f: &flight::State| {
            f.escape
                .as_ref()
                .is_some_and(|p| p.phase == tore_sim::ejection::Phase::Inflating)
        };
        if inflating(flight) && !inflating(tick.previous) {
            events.push(ejection("&CHUTE.5K", "parachute opening"));
        }
    }

    /// Writes the footer and seek index, renames the file to its final name
    /// and returns that path. Waits for the writer to drain its queue.
    pub fn finish(mut self, footer: &replay::Footer) -> Option<PathBuf> {
        if let Some(frame) = self.frame.take() {
            self.held = Some(frame);
        }
        let last = self.held.take().map(|mut frame| {
            self.mark_gap(&mut frame);
            fit(&mut frame);
            frame
        });
        let sender = self.sender.take()?;
        // Everything left waits its turn now; the flight is over.
        for message in std::mem::take(&mut self.backlog) {
            let _ = sender.send(message);
        }
        if let Some(frame) = last {
            let _ = sender.send(Message::Frame(Box::new(frame)));
        }
        let _ = sender.send(Message::Finish(Box::new(footer.clone())));
        drop(sender);
        let result = self.thread.take()?.join();
        match result {
            Ok(Ok(path)) => {
                log::info!(
                    "Recording saved: {} ({} frames)",
                    path.display(),
                    self.frames
                );
                Some(path)
            }
            Ok(Err(error)) => {
                log::info!("Recording not finished: {error}");
                None
            }
            Err(_) => {
                log::info!("Recording writer stopped unexpectedly");
                None
            }
        }
    }
}

impl Drop for Recorder {
    /// A recorder dropped without `finish` (a panic, for one) stops the
    /// writer, which keeps its `.partial` file.
    fn drop(&mut self) {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Keeps a frame within the format's limits, so the writer never refuses a
/// whole tick: over-long lists keep their first items, text is cut to the
/// longest string the format stores, and a `system.note` says what was left
/// out. Live flight stays far inside the limits; this is insurance.
fn fit(frame: &mut Frame) {
    use tore_replay::limits::*;
    fn cut<T>(list: &mut Vec<T>, limit: usize, what: &str, notes: &mut Vec<String>) {
        if list.len() > limit {
            notes.push(format!("{} {what}", list.len() - limit));
            list.truncate(limit);
        }
    }
    fn short(text: &mut String) {
        if text.len() > MAX_STRING_BYTES {
            let mut end = MAX_STRING_BYTES;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
    }
    let mut notes = Vec::new();
    cut(&mut frame.aircraft, MAX_AIRCRAFT, "aircraft", &mut notes);
    cut(
        &mut frame.projectiles,
        MAX_PROJECTILES,
        "projectiles",
        &mut notes,
    );
    cut(&mut frame.debris, MAX_DEBRIS, "debris pieces", &mut notes);
    cut(
        &mut frame.escapees,
        MAX_ESCAPEES,
        "ejected pilots",
        &mut notes,
    );
    cut(
        &mut frame.new_effects,
        MAX_EFFECTS_PER_TICK,
        "effects",
        &mut notes,
    );
    cut(
        &mut frame.new_puffs,
        MAX_PUFFS_PER_TICK,
        "smoke puffs",
        &mut notes,
    );
    cut(
        &mut frame.surface_hp,
        MAX_SURFACE_CHANGES_PER_TICK,
        "surface changes",
        &mut notes,
    );
    for event in &mut frame.events {
        short(&mut event.text);
        event.fields.truncate(MAX_FIELDS_PER_EVENT);
        for (_, value) in &mut event.fields {
            match value {
                replay::Value::Text(text) => short(text),
                replay::Value::Ids(ids) => ids.truncate(MAX_IDS_PER_VALUE),
                _ => {}
            }
        }
    }
    cut(
        &mut frame.events,
        MAX_EVENTS_PER_TICK - 1,
        "events",
        &mut notes,
    );
    cut(
        &mut frame.trees,
        MAX_TREES_PER_TICK,
        "display trees",
        &mut notes,
    );
    for tree in &mut frame.trees {
        if tree.nodes.len() > MAX_TREE_NODES {
            notes.push(format!(
                "{} nodes of a {} tree",
                tree.nodes.len() - MAX_TREE_NODES,
                tree.channel
            ));
            tree.nodes.truncate(MAX_TREE_NODES);
        }
        for node in &mut tree.nodes {
            node.depth = node.depth.min(MAX_TREE_DEPTH - 1);
            short(&mut node.label);
            short(&mut node.note);
            short(&mut node.unit);
            match &mut node.value {
                replay::Value::Text(text) => short(text),
                replay::Value::Ids(ids) => ids.truncate(MAX_IDS_PER_VALUE),
                _ => {}
            }
        }
    }
    if !notes.is_empty() {
        frame
            .events
            .push(Event::new(kind::SYSTEM_NOTE).with_text(format!(
                "over the format's limits, left out: {}",
                notes.join(", ")
            )));
    }
}

/// Each on-off cheat's name and state before and after.
fn cheat_changes(
    before: &tore_sim::cheats::Cheats,
    after: &tore_sim::cheats::Cheats,
) -> [(&'static str, bool, bool); 15] {
    type Switch = fn(&tore_sim::cheats::Cheats) -> bool;
    let list: [(&'static str, Switch); 15] = [
        ("invulnerable", |c| c.invulnerable),
        ("unlimited_ammo", |c| c.unlimited_ammo),
        ("unlimited_fuel", |c| c.unlimited_fuel),
        ("no_spins", |c| c.no_spins),
        ("no_turbulence", |c| c.no_turbulence),
        ("extra_g", |c| c.extra_g),
        ("ignore_weapon_weights", |c| c.ignore_weapon_weights),
        ("no_sun_whiteout", |c| c.no_sun_whiteout),
        ("no_g_effects", |c| c.no_g_effects),
        ("no_screen_shake", |c| c.no_screen_shake),
        ("no_crashes", |c| c.no_crashes),
        ("easy_aiming", |c| c.easy_aiming),
        ("ignore_midair_collisions", |c| c.ignore_midair_collisions),
        ("easy_targeting", |c| c.easy_targeting),
        ("guns_only", |c| c.guns_only),
    ];
    list.map(|(name, switch)| (name, switch(before), switch(after)))
}

/// The cheats switched on, for a recording's header.
pub fn cheats_on(cheats: &tore_sim::cheats::Cheats) -> Vec<String> {
    let mut on: Vec<String> = cheat_changes(cheats, cheats)
        .into_iter()
        .filter(|(_, was, _)| *was)
        .map(|(name, _, _)| name.to_owned())
        .collect();
    if let Some(level) = cheats.enemy_ai {
        on.push(format!("enemy_ai={level:?}"));
    }
    on
}

fn ground_height(world: &terrain::World, position: [f64; 3]) -> f64 {
    world.surface(position[0], position[2]).height
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

/// An aircraft's registered label, or `aircraft 7` for one never registered.
fn who(infos: &BTreeMap<u32, replay::AircraftInfo>, id: u32) -> String {
    match infos.get(&id) {
        Some(info) if !info.label.is_empty() => info.label.clone(),
        Some(info) if !info.name.is_empty() => format!("{} {id}", info.name),
        _ => format!("aircraft {id}"),
    }
}

/// Airspeed as recorded, or the ground speed when none was.
fn speed(state: &replay::AircraftState) -> f64 {
    if state.airspeed > 0. {
        state.airspeed
    } else {
        state.ground_speed()
    }
}

/// Launch geometry between a shooter and its target.
fn geometry(
    event: Event,
    shooter: &replay::AircraftState,
    target: &replay::AircraftState,
) -> Event {
    let line: [f64; 3] = std::array::from_fn(|i| target.position[i] - shooter.position[i]);
    let range = distance(target.position, shooter.position);
    let angle = |a: [f64; 3], b: [f64; 3]| {
        let dot: f64 = (0..3).map(|i| a[i] * b[i]).sum();
        (dot / range.max(1e-9)).clamp(-1., 1.).acos().to_degrees()
    };
    // Aspect: 0 degrees is dead astern of the target, 180 head on.
    let aspect = angle(target.forward(), line);
    let off_boresight = angle(shooter.forward(), line);
    let relative: [f64; 3] = std::array::from_fn(|i| target.velocity[i] - shooter.velocity[i]);
    let closure = -(0..3).map(|i| relative[i] * line[i]).sum::<f64>() / range.max(1e-9);
    event
        .with(field::RANGE_FT, range)
        .with(field::ASPECT_DEG, aspect)
        .with(field::OFF_BORESIGHT_DEG, off_boresight)
        .with(field::CLOSURE_KT, closure * KT_PER_FPS)
        .with(field::SHOOTER_ALT_FT, shooter.position[1])
        .with(field::TARGET_ALT_FT, target.position[1])
        .with(field::SHOOTER_SPEED_KT, speed(shooter) * KT_PER_FPS)
        .with(field::TARGET_SPEED_KT, speed(target) * KT_PER_FPS)
}

/// Flight data for one drawn aircraft: the player's own state, an AI
/// aircraft's, or what a straight-flight fixture's combat target holds.
fn flight_data(
    pose: &crate::render_snapshot::AircraftPose,
    tick: &Tick<'_>,
    targets: &BTreeMap<u32, &live::Target>,
    player_ground: f64,
) -> FlightData {
    let controls = |f: &flight::State, input: &flight::PilotInput| {
        [input.pitch, input.roll, input.yaw, f.throttle]
    };
    if pose.id == 0 {
        let f = tick.flight;
        return FlightData {
            airspeed: f.speed,
            g: f.g,
            fuel_lb: f.fuel,
            controls: controls(f, tick.pilot),
            on_ground: f.supported_at(player_ground),
            alive: !f.crashed
                && tick.combat.state.player_hp > 0
                && !f.systems.pilot.dead
                && f.escape.is_none(),
            ejected: f.escape.is_some(),
            wreck_gone: f.wreck_gone(),
        };
    }
    let target = targets.get(&pose.id);
    if let Some(actor) = tick.wings.and_then(|w| w.mission().actor(pose.id)) {
        let f = actor.flight();
        return FlightData {
            airspeed: f.speed,
            g: f.g,
            fuel_lb: f.fuel,
            controls: controls(f, actor.last_input()),
            on_ground: target.is_some_and(|t| t.on_ground),
            alive: actor.alive() && f.escape.is_none(),
            ejected: f.escape.is_some(),
            wreck_gone: f.wreck_gone() || target.is_some_and(|t| !t.airborne && t.hp <= 0),
        };
    }
    let [x, y, z] = pose.velocity;
    FlightData {
        airspeed: (x * x + y * y + z * z).sqrt(),
        g: 1.,
        fuel_lb: 0.,
        controls: [0.; 4],
        on_ground: target.is_some_and(|t| t.on_ground),
        alive: pose.damage.hp > 0,
        ejected: false,
        wreck_gone: !pose.airborne && pose.damage.hp <= 0,
    }
}

/// Smoke and contrail puffs released this tick: the newest puffs, which
/// have not aged yet.
fn new_puffs(combat: &combat::Combat) -> Vec<replay::PuffSpawn> {
    use tore_sim::combat::smoke::Kind;
    let mut out = Vec::new();
    for (layer, smoke) in [
        (replay::LAYER_SMOKE, &combat.state.smoke),
        (replay::LAYER_CONTRAILS, &combat.contrails),
    ] {
        let start = out.len();
        for puff in smoke.puffs.iter().rev().take_while(|p| p.age == 0) {
            out.push(replay::PuffSpawn {
                layer,
                kind: match puff.kind {
                    Kind::Missile => replay::PuffKind::Missile,
                    Kind::Aircraft => replay::PuffKind::Aircraft,
                    Kind::Contrail => replay::PuffKind::Contrail,
                },
                position: puff.position,
            });
        }
        // Release order, oldest first.
        out[start..].reverse();
    }
    out.truncate(tore_replay::limits::MAX_PUFFS_PER_TICK);
    out
}

/// An identity for an aircraft that appeared after the recording started.
fn default_info(
    pose: &crate::render_snapshot::AircraftPose,
    wings: Option<&AiWings>,
) -> replay::AircraftInfo {
    let slot = wings.and_then(|w| w.slot(pose.id));
    replay::AircraftInfo {
        id: pose.id,
        pt: pose
            .aircraft
            .map(|a| convert::identity_key(a).to_owned())
            .unwrap_or_default(),
        name: pose
            .aircraft
            .map(|a| a.label().to_owned())
            .unwrap_or_default(),
        label: slot.map_or_else(|| format!("Aircraft {}", pose.id), |s| s.label()),
        side: slot.map_or(replay::Side::Unknown, |s| side(s.side)),
        wing: slot.map_or(0, |s| u16::from(s.wing_number)),
        member: slot.map_or(0, |s| u16::from(s.member_number)),
        skill: String::new(),
        human: false,
    }
}

fn side(side: tore_sim::ai::launch::Side) -> replay::Side {
    match side {
        tore_sim::ai::launch::Side::Friendly => replay::Side::Friendly,
        tore_sim::ai::launch::Side::Enemy => replay::Side::Enemy,
    }
}

/// Every aircraft at the start of a flight: the player, then each other
/// aircraft the snapshot draws, named as the setup screen names them.
pub fn roster(
    snapshot: &RenderSnapshot,
    player_name: &str,
    player_wing: bool,
    wings: Option<&AiWings>,
    models: &[crate::aircraft::Airframe],
) -> Vec<replay::AircraftInfo> {
    let name_of = |id: tore_formats::aircraft::AircraftId| {
        models
            .iter()
            .find(|m| m.profile.id == id)
            .map_or_else(|| id.label().to_owned(), |m| m.profile.name.clone())
    };
    let mut out = vec![replay::AircraftInfo {
        id: 0,
        pt: snapshot
            .player
            .aircraft
            .map(|a| convert::identity_key(a).to_owned())
            .unwrap_or_default(),
        name: player_name.to_owned(),
        label: "You".into(),
        side: replay::Side::Friendly,
        wing: u16::from(player_wing),
        member: u16::from(player_wing),
        skill: "Human".into(),
        human: true,
    }];
    for pose in snapshot.targets.iter().filter(|p| p.aircraft.is_some()) {
        let mut info = default_info(pose, wings);
        if let Some(id) = pose.aircraft {
            info.name = name_of(id);
        }
        if let Some(actor) = wings.and_then(|w| w.mission().actor(pose.id)) {
            info.skill = format!("{:?}", actor.experience().level);
        }
        out.push(info);
    }
    out
}

/// A recording's header. `extra` holds the flight's settings, in order.
pub fn header(
    mission: replay::MissionKind,
    world: &terrain::World,
    presentation: &Presentation,
    mut extra: Vec<(String, String)>,
    recorded_at: std::time::SystemTime,
) -> replay::Header {
    extra.extend(presentation.extras());
    extra.push(("platform".into(), crate::version::target().into()));
    replay::Header {
        game_version: crate::version::version().into(),
        game_commit: crate::version::commit().into(),
        recorded_at: super::library::utc_text(recorded_at),
        mission,
        world: world.identity(),
        extra,
        ..replay::Header::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::render_hash_tests as fixture;
    use std::sync::mpsc::Receiver;

    /// Frames waiting in the queue, oldest first; registrations are skipped.
    fn frames(receiver: &Receiver<Message>) -> Vec<Frame> {
        receiver
            .try_iter()
            .filter_map(|message| match message {
                Message::Frame(frame) => Some(*frame),
                _ => None,
            })
            .collect()
    }

    /// Records `snapshot` as tick `number`, the way the tick loop does.
    fn tick(
        recorder: &mut Recorder,
        combat: &mut combat::Combat,
        snapshot: &RenderSnapshot,
        number: u64,
        ui: &mut flight_ui::FlightUi,
    ) {
        let flight = fixture::player();
        let world = terrain::tests::world();
        let mut snapshot = snapshot.clone();
        snapshot.tick = number;
        recorder.start_tick(Some(ui), combat);
        recorder.begin(Tick {
            snapshot: &snapshot,
            combat,
            flight: &flight,
            previous: &flight,
            pilot: &flight::PilotInput::default(),
            wings: None,
            world: &world,
            events: &[],
            outcomes: &[],
            journal: None,
        });
        recorder.end(Some(ui), combat);
    }

    #[test]
    fn an_oversized_frame_is_trimmed_to_fit_rather_than_refused() {
        use tore_replay::limits::{MAX_EVENTS_PER_TICK, MAX_PROJECTILES, MAX_STRING_BYTES};
        let mut frame = Frame {
            tick: 5,
            projectiles: (0..1100)
                .map(|id| replay::ProjectileState {
                    id,
                    direction: [0., 0., 1.],
                    ..Default::default()
                })
                .collect(),
            events: (0..1100)
                .map(|_| Event::new(kind::COMMS_HUD).with_text("é".repeat(700)))
                .collect(),
            ..Frame::default()
        };
        fit(&mut frame);
        assert_eq!(frame.projectiles.len(), MAX_PROJECTILES);
        assert_eq!(frame.events.len(), MAX_EVENTS_PER_TICK);
        assert!(
            frame
                .events
                .iter()
                .all(|e| e.text.len() <= MAX_STRING_BYTES)
        );
        let note = frame.events.last().unwrap();
        assert_eq!(note.kind, kind::SYSTEM_NOTE);
        assert!(note.text.contains("76 projectiles") && note.text.contains("77 events"));
        // The writer takes it.
        let dir = crate::replay::tests::TempDir::new("fit");
        let mut writer = replay::Writer::create(
            dir.path().join("fit.tore-replay"),
            &replay::Header::default(),
        )
        .unwrap();
        writer.push(&frame).unwrap();
    }

    #[test]
    fn written_comms_and_audio_entries_are_handed_to_live_flight() {
        let (mut recorder, _receiver) = Recorder::detached(64, &[]);
        recorder.note(Event::new(kind::COMMS_RADIO).with_text("Fox two"));
        recorder.note(Event::new(kind::WEAPON_LAUNCH));
        recorder.note(Event::new(kind::AUDIO_TONE));
        recorder.release_held();
        assert!(recorder.take_comms().is_empty());
        recorder.held = Some(Frame {
            tick: 7,
            events: std::mem::take(&mut recorder.early),
            ..Default::default()
        });
        recorder.release_held();
        let taken = recorder.take_comms();
        let kinds: Vec<_> = taken
            .iter()
            .map(|e| (e.tick, e.event.kind.as_str()))
            .collect();
        assert_eq!(kinds, [(7, kind::COMMS_RADIO), (7, kind::AUDIO_TONE)]);
        assert!(recorder.take_comms().is_empty());
        // Only the newest wait when nobody looks.
        for tick in 0..(COMMS_TAP as u64 + 5) {
            recorder.held = Some(Frame {
                tick,
                events: vec![Event::new(kind::COMMS_HUD)],
                ..Default::default()
            });
            recorder.release_held();
        }
        let taken = recorder.take_comms();
        assert_eq!(taken.len(), COMMS_TAP);
        assert_eq!(taken[0].tick, 5);
    }

    #[test]
    fn display_trees_are_handed_to_live_flight_only_while_it_shows_them() {
        let (mut recorder, _receiver) = Recorder::detached(64, &[]);
        let tree = |subject| replay::TreeSample {
            subject,
            channel: tore_replay::vocab::channel::AI_THOUGHT.into(),
            nodes: vec![replay::Node::new(0, "Activity", "ATTACKING")],
        };
        let frame = |tick, subject| Frame {
            tick,
            trees: vec![tree(subject)],
            ..Default::default()
        };
        recorder.held = Some(frame(1, 3));
        recorder.release_held();
        assert!(recorder.take_trees().is_empty());
        recorder.set_live(true);
        recorder.held = Some(frame(2, 4));
        recorder.release_held();
        assert_eq!(recorder.take_trees(), [(2, tree(4))]);
        recorder.held = Some(frame(3, 5));
        recorder.release_held();
        recorder.set_live(false);
        assert!(recorder.take_trees().is_empty());
    }

    #[test]
    fn the_seeker_tone_is_recorded_with_its_loudness_and_surface() {
        use tore_replay::vocab::tone as named;
        let lock = Tone {
            radar: false,
            ground: true,
            locked: true,
        };
        let search = Tone {
            locked: false,
            ..lock
        };
        let started = tone_event(None, Some((lock, 0.912))).unwrap();
        assert_eq!(started.string(field::TONE), Some(named::INFRARED_LOCK));
        assert_eq!(started.flag(field::ON), Some(true));
        assert_eq!(started.num(field::STRENGTH), Some(0.91));
        assert_eq!(started.flag(field::SURFACE), Some(true));
        // Small swells are not recorded; one of a step or more is.
        assert!(tone_event(Some((lock, 0.912)), Some((lock, 0.95))).is_none());
        let louder = tone_event(Some((lock, 0.912)), Some((lock, 0.97))).unwrap();
        assert_eq!(louder.num(field::STRENGTH), Some(0.97));
        // A new tone at the same loudness, and the tone stopping.
        let changed = tone_event(Some((lock, 0.5)), Some((search, 0.5))).unwrap();
        assert_eq!(changed.string(field::TONE), Some(named::GROUND));
        let stopped = tone_event(Some((search, 0.5)), None).unwrap();
        assert_eq!(stopped.flag(field::ON), Some(false));
        assert_eq!(stopped.string(field::TONE), Some(named::GROUND));
        assert!(tone_event(None, None).is_none());
    }

    #[test]
    fn a_full_queue_drops_frames_and_the_next_frame_reports_the_gap() {
        let (mut recorder, receiver) = Recorder::detached(3, &[]);
        let mut combat = fixture::combat(Vec::new(), Vec::new());
        let player = fixture::player();
        combat.restart_render(&player, None);
        let snapshot = combat.render_snapshot().clone();
        let mut ui = flight_ui::FlightUi::default();
        for number in 0..8 {
            tick(&mut recorder, &mut combat, &snapshot, number, &mut ui);
        }
        let first = frames(&receiver);
        assert!(!first.is_empty() && first.len() < 7, "{}", first.len());
        // The disk catches up: the next frame says what was lost.
        tick(&mut recorder, &mut combat, &snapshot, 8, &mut ui);
        let next = frames(&receiver);
        assert_eq!(next.len(), 1);
        let gap = &next[0].events[0];
        assert_eq!(gap.kind, kind::SYSTEM_GAP);
        let (from, to) = (
            gap.get(field::FROM)
                .and_then(replay::Value::as_i64)
                .unwrap(),
            gap.get(field::TO).and_then(replay::Value::as_i64).unwrap(),
        );
        let last_sent = first.last().unwrap().tick as i64;
        assert_eq!((from, to), (last_sent + 1, 6));
        assert_eq!(next[0].tick, 7);
        // Nothing waits forever: the recorder never blocked.
    }

    #[test]
    fn waiting_for_the_writer_never_drops_a_frame() {
        let (mut recorder, receiver) = Recorder::detached(3, &[]);
        recorder.wait_for_writer();
        // A slow writer on its own thread, as in a busy headless run.
        let writer = std::thread::spawn(move || {
            let mut ticks = Vec::new();
            for message in receiver {
                if let Message::Frame(frame) = message {
                    assert!(
                        frame.events.iter().all(|e| e.kind != kind::SYSTEM_GAP),
                        "a waiting recorder reported a gap"
                    );
                    ticks.push(frame.tick);
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
            ticks
        });
        let mut combat = fixture::combat(Vec::new(), Vec::new());
        let player = fixture::player();
        combat.restart_render(&player, None);
        let snapshot = combat.render_snapshot().clone();
        let mut ui = flight_ui::FlightUi::default();
        for number in 0..12 {
            tick(&mut recorder, &mut combat, &snapshot, number, &mut ui);
        }
        // The last frame is still held for notes; every earlier one arrived.
        drop(recorder);
        let ticks = writer.join().unwrap();
        assert_eq!(ticks, (0..11).collect::<Vec<u64>>());
    }

    #[test]
    fn notes_between_ticks_land_on_the_tick_on_screen() {
        let (mut recorder, receiver) = Recorder::detached(64, &[]);
        let mut combat = fixture::combat(Vec::new(), Vec::new());
        let player = fixture::player();
        combat.restart_render(&player, None);
        let snapshot = combat.render_snapshot().clone();
        let mut ui = flight_ui::FlightUi::default();
        recorder.session(&ui);
        tick(&mut recorder, &mut combat, &snapshot, 0, &mut ui);
        // Between ticks: a bookmark, a pause, a cockpit message and a
        // command, all while tick 0 is on screen.
        assert_eq!(recorder.bookmark(), 1);
        ui.paused = true;
        recorder.session(&ui);
        ui.message("Radio silence");
        ui.message("Radio silence");
        combat.command(live::Command::ClearDesignation, combat::launcher(&player));
        tick(&mut recorder, &mut combat, &snapshot, 1, &mut ui);
        tick(&mut recorder, &mut combat, &snapshot, 2, &mut ui);
        let frames = frames(&receiver);
        assert_eq!(frames.iter().map(|f| f.tick).collect::<Vec<_>>(), [0, 1]);
        // Cockpit sounds depend on the fixture's aircraft; leave them out.
        let kinds: Vec<&str> = frames[0]
            .events
            .iter()
            .map(|e| e.kind.as_str())
            .filter(|kind| !kind.starts_with("audio."))
            .collect();
        assert_eq!(
            kinds,
            [
                kind::PLAYER_BOOKMARK,
                kind::SYSTEM_PAUSE,
                kind::COMMS_HUD,
                kind::COMMS_HUD,
                kind::PLAYER_COMMAND,
            ]
        );
        // The repeat moves the line on screen to the bottom with a new timer.
        let hud: Vec<(Option<&str>, bool)> = frames[0]
            .events
            .iter()
            .filter(|e| e.kind == kind::COMMS_HUD)
            .map(|e| (e.string(field::OUTCOME), e.get(field::REASON).is_some()))
            .collect();
        assert_eq!(
            hud,
            [
                (Some(outcome::DELIVERED), false),
                (Some(outcome::DELIVERED), true)
            ]
        );
        let bookmark = frames[0]
            .events
            .iter()
            .find(|e| e.kind == kind::PLAYER_BOOKMARK)
            .unwrap();
        assert_eq!(bookmark.text, "Bookmark 1");
        assert!(
            frames[1]
                .events
                .iter()
                .all(|e| e.kind.starts_with("audio."))
        );
        // Tick 0 carries the once-a-second checksum; tick 1 does not.
        assert!(frames[0].checksum.is_some() && frames[1].checksum.is_none());
    }

    #[test]
    fn released_chaff_and_flares_are_recorded_exactly_on_their_ticks() {
        use tore_sim::combat::countermeasures::Release;
        let (mut recorder, receiver) = Recorder::detached(64, &[]);
        let mut combat = fixture::combat(Vec::new(), Vec::new());
        let player = fixture::player();
        combat.restart_render(&player, None);
        let snapshot = combat.render_snapshot().clone();
        let mut ui = flight_ui::FlightUi::default();
        (combat.state.chaff, combat.state.flares) = (2, 2);
        tick(&mut recorder, &mut combat, &snapshot, 0, &mut ui);
        // Between ticks the player releases chaff: its entries land on the
        // tick on screen, the sound with the player's own flag.
        let launcher = combat::launcher(&player);
        combat.command(live::Command::ReleaseChaff, launcher);
        let emissions = combat.state.take_sound_events();
        recorder.sounds(&emissions, &[]);
        tick(&mut recorder, &mut combat, &snapshot, 1, &mut ui);
        // An AI aircraft's flare leaves during tick 2, and a range reset
        // clears the devices during tick 3.
        let flare = Release {
            position: [100.5, 2_000.25, -300.125],
            velocity: [1. / 3., -2., 700.],
            basis: tore_sim::attitude::Basis::new(0.3, -0.1, 0.7),
        };
        for (number, change) in [(2u64, true), (3, false)] {
            recorder.start_tick(Some(&mut ui), &mut combat);
            if change {
                combat
                    .state
                    .device_released(flare, live::EffectKind::Flare, 7);
            } else {
                combat.state.range_target(launcher);
            }
            let mut snapshot = snapshot.clone();
            snapshot.tick = number;
            let (world, flight) = (terrain::tests::world(), fixture::player());
            recorder.begin(Tick {
                snapshot: &snapshot,
                combat: &combat,
                flight: &flight,
                previous: &flight,
                pilot: &flight::PilotInput::default(),
                wings: None,
                world: &world,
                events: &[],
                outcomes: &[],
                journal: None,
            });
            recorder.end(Some(&mut ui), &mut combat);
        }
        tick(&mut recorder, &mut combat, &snapshot, 4, &mut ui);
        let frames = frames(&receiver);
        let entries = |tick: u64, kind: &str| -> Vec<Event> {
            frames
                .iter()
                .filter(|f| f.tick == tick)
                .flat_map(|f| f.events.iter())
                .filter(|e| e.kind == kind)
                .cloned()
                .collect()
        };
        let chaff = entries(0, kind::COMBAT_COUNTERMEASURE);
        assert_eq!(chaff.len(), 1);
        assert_eq!(
            convert::device_release(&chaff[0], 0),
            Some(convert::DeviceRelease {
                owner: 0,
                kind: live::EffectKind::Chaff,
                release: Release {
                    position: launcher.position,
                    velocity: launcher.velocity,
                    basis: launcher.basis,
                },
                number: 1,
                // The fixture's combat never stepped.
                tick: combat.state.tick(),
            })
        );
        assert_eq!(chaff[0].num(field::LEFT), Some(1.));
        let sounds = entries(0, kind::AUDIO_EFFECT);
        assert_eq!(sounds.len(), 1);
        assert_eq!(
            (sounds[0].string(field::SOUND), sounds[0].flag(field::OWN)),
            (Some("chaff"), Some(true))
        );
        let command = entries(0, kind::PLAYER_COMMAND);
        assert_eq!(command[0].string(field::COMMAND), Some("chaff"));
        // The AI's flare: every number back bit for bit; its dispensers are
        // unknown without an AI bridge, so no count is kept.
        let ai = entries(2, kind::COMBAT_COUNTERMEASURE);
        let back = convert::device_release(&ai[0], 2).unwrap();
        assert_eq!(
            (back.owner, back.kind, back.number),
            (7, live::EffectKind::Flare, 2)
        );
        for (a, b) in [
            (back.release.position, flare.position),
            (back.release.velocity, flare.velocity),
            (back.release.basis.right, flare.basis.right),
            (back.release.basis.up, flare.basis.up),
            (back.release.basis.forward, flare.basis.forward),
        ] {
            assert_eq!(a.map(f64::to_bits), b.map(f64::to_bits));
        }
        assert!(ai[0].get(field::LEFT).is_none());
        assert_eq!(entries(3, kind::COMBAT_COUNTERMEASURES_CLEARED).len(), 1);
    }

    #[test]
    fn launches_effects_and_aircraft_are_recorded_from_the_picture() {
        let (mut recorder, receiver) = Recorder::detached(64, &[]);
        let mut combat = fixture::combat(
            fixture::models(),
            (0..7).map(|i| (i % 3, [0.; 3])).collect(),
        );
        let player = fixture::player();
        let scene = fixture::scene(combat.state.configuration());
        let [previous, current] = fixture::snapshots(&mut combat, &scene, true, &player);
        let mut ui = flight_ui::FlightUi::default();
        tick(&mut recorder, &mut combat, &previous, 10, &mut ui);
        tick(&mut recorder, &mut combat, &current, 11, &mut ui);
        tick(&mut recorder, &mut combat, &current, 12, &mut ui);
        let mut registered = BTreeSet::new();
        let mut weapons = BTreeSet::new();
        let mut frames = Vec::new();
        for message in receiver.try_iter() {
            match message {
                Message::Aircraft(info) => assert!(registered.insert(info.id)),
                Message::Weapon(info) => assert!(weapons.insert(info.source.clone())),
                Message::Frame(frame) => frames.push(*frame),
                Message::Finish(_) => unreachable!(),
            }
        }
        // The player and every aircraft of either tick, but not the two
        // ground objects.
        let aircraft = |snapshot: &RenderSnapshot| -> BTreeSet<u32> {
            std::iter::once(0)
                .chain(
                    snapshot
                        .targets
                        .iter()
                        .filter(|t| t.aircraft.is_some())
                        .map(|t| t.id),
                )
                .collect()
        };
        assert_eq!(registered, &aircraft(&previous) | &aircraft(&current));
        assert_eq!(frames[1].aircraft.len(), aircraft(&current).len());
        assert_eq!(frames[1].surface_hp.len(), 0, "unchanged since tick 10");
        assert_eq!(frames[0].surface_hp.len(), 2);
        // Every weapon in flight launched once, each type registered once.
        let launches = |frame: &Frame| {
            frame
                .events
                .iter()
                .filter(|e| e.kind == kind::WEAPON_LAUNCH)
                .count()
        };
        assert_eq!(launches(&frames[0]), 0);
        assert_eq!(launches(&frames[1]), current.projectiles.len());
        let sources: BTreeSet<String> = current
            .projectiles
            .iter()
            .map(|p| p.weapon.clone())
            .collect();
        assert_eq!(weapons, sources);
        // Effects start once, lasting the ticks they have left.
        assert_eq!(frames[1].new_effects.len(), current.effects.len());
        assert_eq!(
            frames[1].new_effects[1].duration_ticks,
            u32::from(current.effects[1].ticks)
        );
    }
}
