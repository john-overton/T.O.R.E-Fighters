//! The observer screen (stage F phase 2, slice F2-O2; docs/ARCHITECTURE.md,
//! "The observer view").
//!
//! A game with no plane watches the flying mission through the replay
//! viewer's live mode:
//!
//! ```mermaid
//! flowchart LR
//!   client["Client::observer_frame<br/>(drawn picture, real plane ids)"] --> feeder["Feeder<br/>one replay frame per host tick"]
//!   feeder --> store["Store thread<br/>writes the growing recording,<br/>reads it again"]
//!   store --> viewer["Viewer::grow<br/>(the playhead follows the newest frame)"]
//! ```
//!
//! - [`Feeder`] turns each observer frame into replay frames, one for every
//!   host tick the picture has passed (a tick between two drawn pictures is
//!   blended as flight blends its ticks), with the registry entries the
//!   frames need, the weapons the projectiles are, the effects, craters and
//!   fires that started, the ground objects that fell, and a launch or a loss
//!   as an event for the timeline's markers.
//! - [`Store`] writes them with the replay writer to a file of its own in the
//!   game's data folder (`observer/`, not the system's temporary folder,
//!   which can be memory) and, a quarter of a second after new frames (longer
//!   for a long recording), reads the file again and hands the viewer the new
//!   read. The file keeps the whole mission (about 16 MB for ten minutes of
//!   thirty aircraft); the viewer lets the last ten minutes be scrubbed. It
//!   is removed when the watch ends, and one left by a game that crashed a
//!   day ago is removed when the next watch starts.
//! - [`Observing`] is the game's one watch: the feeder and the store, and
//!   `impl App` starts the screen when the first frames have arrived, feeds
//!   it each frame, sends the camera's subject to the host, and returns to
//!   the lobby when the watch ends (the player's Stop Watching, the
//!   mission's end, the session's).
//!
//! The recording has no player (`draw.player` is [`NO_PLAYER`]): every
//! aircraft is another, in its real plane id, so the viewer's cameras,
//! views, labels and panels work as on any replay. What the host's observer
//! stream does not carry (the cockpit readout, radar, the AI's thinking,
//! radio lines) is not shown; smoke, contrails and gun rounds are not yet
//! regenerated (a known limit shared with converted captures).
use crate::replay::convert::{self, FlightData, NO_PLAYER, Presentation};
use crate::replay::viewer::{Options, Viewer};
use crate::snapshot::{self, AircraftPose, ProjectilePose, RenderSnapshot};
use crate::{App, Screen};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use tore_formats::aircraft::AircraftId;
use tore_replay::{self as replay, vocab};
use tore_session::client::observe::ObserverFrame;
use tore_session::wire::messages::{Roster, RosterPlane};
use tore_sim::combat::blast;
use tore_world::world::World;

/// A gap of more than this many ticks between two observer frames (two
/// seconds, as the capture conversion's) is not bridged: the recording has
/// a gap there.
pub const BRIDGE_TICKS: u64 = 240;
/// Frames in one chunk of the recording: a fifth of a second, so the newest
/// frames reach the viewer soon after they are made.
pub const CHUNK_TICKS: u32 = 24;
/// How soon after new frames the recording is read again, at least. A long
/// recording takes longer to read; the store waits three times as long as
/// the last read took.
pub const REFRESH: Duration = Duration::from_millis(250);
/// The aircraft ids up to this draw with their own model (`draw.slots`):
/// planes a revival adds later are above any roster's, so none is hidden.
pub const SLOTS: u32 = 65_535;

/// What some observer frames add to the recording.
#[derive(Debug, Default)]
pub struct Out {
    /// Aircraft the frames name for the first time.
    pub aircraft: Vec<replay::AircraftInfo>,
    /// Weapons the frames name for the first time.
    pub weapons: Vec<replay::WeaponInfo>,
    /// The frames, in tick order.
    pub frames: Vec<replay::Frame>,
}

/// An effect or a mark as a picture shows it, to see when one is new.
type Key = (String, [u64; 3]);

fn key(what: impl std::fmt::Debug, position: [f64; 3]) -> Key {
    (format!("{what:?}"), position.map(f64::to_bits))
}

/// What the feeder reads besides the frame: who flies and what the mission
/// is made of.
#[derive(Clone, Copy, Default)]
pub struct Context<'a> {
    pub roster: Option<&'a Roster>,
    pub world: Option<&'a World>,
}

/// Turns observer frames into replay frames.
#[derive(Default)]
pub struct Feeder {
    /// The tick the next frame has.
    next: Option<u64>,
    /// The picture the last frame stretch came from, and the host tick it
    /// showed.
    previous: Option<(f64, RenderSnapshot)>,
    /// Aircraft the writer has been told of.
    registered: BTreeSet<u32>,
    /// Weapons the writer has been told of, by name.
    weapons: BTreeMap<String, u32>,
    /// The first tick each projectile flying was seen.
    launched: BTreeMap<u32, u64>,
    /// Effects playing in the last picture.
    effects: BTreeSet<Key>,
    /// Craters and fires started.
    marks: BTreeSet<Key>,
    /// Ground objects already recorded as fallen.
    fallen: BTreeSet<u32>,
    /// Aircraft seen flying, and those already recorded as lost.
    flying: BTreeSet<u32>,
    lost: BTreeSet<u32>,
}

impl Feeder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The replay frames for the ticks `frame`'s picture has reached since
    /// the last call: none while it is still inside the same tick, one
    /// usually, more when drawing is slower than the host's ticks.
    pub fn push(&mut self, frame: &ObserverFrame, around: Context<'_>) -> Out {
        let mut out = Out::default();
        let render = frame.render_tick;
        if !render.is_finite() || render < 0. {
            return out;
        }
        let newest = render.floor() as u64;
        let first = match self.next {
            None => newest,
            Some(next) if next > newest => return out,
            Some(next) if newest - next >= BRIDGE_TICKS => newest,
            Some(next) => next,
        };
        let planes: BTreeSet<u32> = around
            .roster
            .into_iter()
            .flat_map(|r| r.planes.iter().map(|p| p.id))
            .collect();
        for tick in first..=newest {
            let picture = match &self.previous {
                Some((before, previous)) if render > *before => snapshot::interpolate(
                    Some(previous),
                    &frame.picture,
                    (tick as f64 - before) / (render - before),
                ),
                _ => frame.picture.clone(),
            };
            // What started goes into the frame of the newest tick, from the
            // picture the client drew.
            let drawn = (tick == newest).then_some(&frame.picture);
            self.frame(tick, &picture, drawn, &planes, around, &mut out);
        }
        self.register(frame, around.roster, around.world, &mut out);
        self.next = Some(newest + 1);
        self.previous = Some((render, frame.picture.clone()));
        out
    }

    /// The registry entries the new frames need, for aircraft not yet told.
    fn register(
        &mut self,
        frame: &ObserverFrame,
        roster: Option<&Roster>,
        world: Option<&World>,
        out: &mut Out,
    ) {
        let planes: BTreeMap<u32, &RosterPlane> = roster
            .into_iter()
            .flat_map(|r| r.planes.iter().map(|p| (p.id, p)))
            .collect();
        let mut ids: BTreeSet<u32> = out
            .frames
            .iter()
            .flat_map(|f| f.aircraft.iter().map(|a| a.id))
            .collect();
        ids.retain(|id| !self.registered.contains(id));
        for id in ids {
            self.registered.insert(id);
            out.aircraft.push(match planes.get(&id) {
                Some(plane) => tore_session::client::convert::roster_info(plane, NO_PLAYER, world),
                None => {
                    let kind = frame
                        .picture
                        .targets
                        .iter()
                        .find(|pose| pose.id == id)
                        .and_then(|pose| pose.aircraft);
                    replay::AircraftInfo {
                        id,
                        pt: kind
                            .map(|k| k.selection_key().to_owned())
                            .unwrap_or_default(),
                        name: kind.map(|k| k.label().to_owned()).unwrap_or_default(),
                        label: format!("Aircraft {id}"),
                        ..replay::AircraftInfo::default()
                    }
                }
            });
        }
    }

    /// The weapon id of `pose`'s weapon, registering it the first time: the
    /// mission's station weapon of that name, else a plain missile (a guided
    /// one) or a gun round.
    fn weapon(&mut self, pose: &ProjectilePose, world: Option<&World>, out: &mut Out) -> u32 {
        if let Some(id) = self.weapons.get(&pose.weapon) {
            return *id;
        }
        let id = self.weapons.len() as u32;
        let known = world.and_then(|w| {
            w.combat
                .dummy_configurations()
                .iter()
                .flat_map(|c| c.stations.iter().map(|s| &s.weapon))
                .find(|w| w.source == pose.weapon)
        });
        out.weapons.push(match known {
            Some(weapon) => convert::weapon_info(id, weapon),
            None => replay::WeaponInfo {
                id,
                source: pose.weapon.clone(),
                shape: pose.shape.clone(),
                name: pose.weapon.clone(),
                class: if pose.gun {
                    replay::WeaponClass::Gun
                } else if pose.target.is_some() {
                    replay::WeaponClass::Missile
                } else {
                    replay::WeaponClass::Other
                },
            },
        });
        self.weapons.insert(pose.weapon.clone(), id);
        id
    }

    /// One frame: the aircraft, weapons, debris and pilots of `picture` at
    /// `tick`, and, when `drawn` is the picture the client drew, what
    /// started in it.
    fn frame(
        &mut self,
        tick: u64,
        picture: &RenderSnapshot,
        drawn: Option<&RenderSnapshot>,
        planes: &BTreeSet<u32>,
        around: Context<'_>,
        out: &mut Out,
    ) {
        let mut frame = replay::Frame {
            tick,
            ..replay::Frame::default()
        };
        let is_aircraft =
            |pose: &AircraftPose| planes.contains(&pose.id) || pose.aircraft.is_some();
        for pose in picture.targets.iter().filter(|pose| is_aircraft(pose)) {
            let ejected = picture.pilots.iter().any(|p| p.owner == pose.id);
            let speed = pose.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
            let device = pose.devices.map_or(0., |d| d[snapshot::DEVICES - 2]);
            let lost = pose.damage.hp <= 0 || pose.crashed;
            frame.aircraft.push(convert::aircraft_state(
                pose,
                &FlightData {
                    airspeed: if device > 0. { device } else { speed },
                    g: 1.,
                    fuel_lb: 0.,
                    controls: [0.; 4],
                    on_ground: !pose.airborne && !pose.crashed,
                    alive: !lost && !ejected,
                    ejected,
                    wreck_gone: matches!(pose.wreck, Some(tore_sim::wreck::Phase::Exploded)),
                },
            ));
            // A loss is marked once, for an aircraft seen flying first (one
            // that was already down when the watch began is not a loss of
            // this watch).
            if !lost {
                self.flying.insert(pose.id);
            } else if self.flying.contains(&pose.id) && self.lost.insert(pose.id) {
                frame
                    .events
                    .push(replay::Event::new(vocab::kind::COMBAT_DESTROYED).with_subject(pose.id));
            }
        }
        frame.aircraft.sort_by_key(|a| a.id);
        frame.aircraft.truncate(replay::limits::MAX_AIRCRAFT);
        for pose in &picture.projectiles {
            let weapon = self.weapon(pose, around.world, out);
            let new = !self.launched.contains_key(&pose.id);
            let first = *self.launched.entry(pose.id).or_insert(tick);
            frame.projectiles.push(convert::projectile_state(
                pose,
                weapon,
                tick.saturating_sub(first),
                None,
            ));
            if new && !pose.gun {
                let mut launch = replay::Event::new(vocab::kind::WEAPON_LAUNCH)
                    .with_subject(pose.owner)
                    .with(vocab::field::PROJECTILE, replay::Value::Id(pose.id))
                    .with(vocab::field::WEAPON, replay::Value::Id(weapon));
                if let Some(target) = pose.target {
                    launch = launch.with_object(target);
                }
                frame.events.push(launch);
            }
        }
        frame.projectiles.truncate(replay::limits::MAX_PROJECTILES);
        let flying: BTreeSet<u32> = picture.projectiles.iter().map(|p| p.id).collect();
        self.launched.retain(|id, _| flying.contains(id));
        frame.debris = convert::debris_states(&picture.debris);
        frame.debris.truncate(replay::limits::MAX_DEBRIS);
        frame.escapees = picture.pilots.iter().map(convert::escapee_state).collect();
        frame.escapees.truncate(replay::limits::MAX_ESCAPEES);
        if let Some(drawn) = drawn {
            self.started(drawn, planes, &mut frame);
        }
        out.frames.push(frame);
    }

    /// What started in the picture the client drew: effects, craters and
    /// fires, and ground objects that fell.
    fn started(
        &mut self,
        drawn: &RenderSnapshot,
        planes: &BTreeSet<u32>,
        frame: &mut replay::Frame,
    ) {
        let mut playing = BTreeSet::new();
        for effect in &drawn.effects {
            let seen = key((effect.kind, effect.blast), effect.position);
            if !self.effects.contains(&seen) {
                frame.new_effects.push(replay::EffectSpawn {
                    kind: convert::effect_kind(effect.kind, effect.blast),
                    position: effect.position,
                    duration_ticks: u32::from(effect.ticks),
                });
            }
            playing.insert(seen);
        }
        self.effects = playing;
        for mark in &drawn.marks {
            if self.marks.insert(key(mark.kind, mark.position)) {
                frame.new_effects.push(replay::EffectSpawn {
                    kind: match mark.kind {
                        blast::MarkKind::Crater(size) => replay::EffectKind::Crater(size),
                        blast::MarkKind::Fire => replay::EffectKind::Fire,
                    },
                    position: mark.position,
                    duration_ticks: blast::FOREVER,
                });
            }
        }
        frame
            .new_effects
            .truncate(replay::limits::MAX_EFFECTS_PER_TICK);
        for pose in &drawn.targets {
            let ground = pose.aircraft.is_none() && !planes.contains(&pose.id);
            if ground && (pose.damage.hp <= 0 || pose.crashed) && self.fallen.insert(pose.id) {
                frame.surface_hp.push((pose.id, 0));
            }
        }
    }
}

/// The header of an observer's recording: the world as the mission has it,
/// and how to draw the aircraft (the mission's models, no player).
pub fn header(terrain: &crate::terrain::Terrain, models: &[AircraftId]) -> replay::Header {
    let presentation = Presentation {
        models: models.to_vec(),
        slots: SLOTS,
        player: NO_PLAYER,
    };
    let mut extra = presentation.extras();
    extra.push((
        replay::model::FUEL_KEY.into(),
        replay::model::FUEL_WITH_EXTERNAL.into(),
    ));
    replay::Header {
        game_version: crate::version::version().into(),
        game_commit: crate::version::commit().into(),
        mission: replay::MissionKind::Other("Observing".into()),
        world: crate::replay::identity::of(terrain),
        extra,
        ..replay::Header::default()
    }
}

enum Message {
    Out(Out),
    Stop,
}

/// The growing recording on disk, written and read again on a thread of
/// its own.
pub struct Store {
    sender: mpsc::Sender<Message>,
    reads: mpsc::Receiver<Arc<replay::Recording>>,
    thread: Option<std::thread::JoinHandle<()>>,
    path: PathBuf,
}

static MADE: AtomicU32 = AtomicU32::new(0);

impl Store {
    /// Starts a recording with `header` in `folder` (made if need be).
    pub fn start(folder: &Path, header: &replay::Header) -> Result<Self, String> {
        std::fs::create_dir_all(folder)
            .map_err(|error| format!("{}: {error}", folder.display()))?;
        let name = format!(
            "observer-{}-{}-{}.tore-replay",
            std::process::id(),
            MADE.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_millis())
        );
        let path = folder.join(name);
        let writer = replay::Writer::create_with(
            &path,
            header,
            replay::WriterOptions {
                chunk_ticks: CHUNK_TICKS,
                // The file is not kept: no need to sync it to disk.
                sync_ticks: u64::MAX / 2,
            },
        )
        .map_err(|error| format!("{}: {error}", path.display()))?;
        let (sender, messages) = mpsc::channel();
        let (reads_sender, reads) = mpsc::channel();
        let partial = replay::partial_path(&path);
        let thread = std::thread::Builder::new()
            .name("observer recording".into())
            .spawn(move || run(writer, &partial, &messages, &reads_sender))
            .map_err(|error| format!("cannot start the recording's thread: {error}"))?;
        Ok(Self {
            sender,
            reads,
            thread: Some(thread),
            path,
        })
    }

    /// Hands the thread frames to write.
    pub fn write(&self, out: Out) {
        let _ = self.sender.send(Message::Out(out));
    }

    /// The newest read of the recording that has arrived since the last
    /// call.
    pub fn newest(&self) -> Option<Arc<replay::Recording>> {
        self.reads.try_iter().last()
    }

    /// Waits for a read that reaches at least tick `tick` (tests).
    #[cfg(test)]
    pub fn wait_for(&self, tick: u64) -> Arc<replay::Recording> {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(read) = self.newest()
                && read.last_tick().is_some_and(|last| last >= tick)
            {
                return read;
            }
            assert!(
                Instant::now() < end,
                "the recording never reached tick {tick}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Where the recording is while it grows (tests).
    #[cfg(test)]
    pub fn partial(&self) -> PathBuf {
        replay::partial_path(&self.path)
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let partial = replay::partial_path(&self.path);
        let _ = std::fs::remove_file(&partial);
        // The folder goes too when nothing else is in it.
        if let Some(folder) = self.path.parent() {
            let _ = std::fs::remove_dir(folder);
        }
    }
}

/// The recording's thread: writes what arrives and, when there is more than
/// the viewer has read, reads the file again and sends the read.
fn run(
    mut writer: replay::Writer,
    partial: &Path,
    messages: &mpsc::Receiver<Message>,
    reads: &mpsc::Sender<Arc<replay::Recording>>,
) {
    let mut dirty = false;
    let mut failed = false;
    let mut every = REFRESH;
    let mut last_read = Instant::now();
    loop {
        let wait = if dirty {
            every.saturating_sub(last_read.elapsed())
        } else {
            Duration::from_secs(1)
        };
        match messages.recv_timeout(wait) {
            Ok(Message::Out(out)) => {
                if !failed && let Err(error) = write(&mut writer, out) {
                    log::warn!("Observer recording: {error}; it stops here");
                    failed = true;
                }
                dirty = true;
            }
            Ok(Message::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if dirty && last_read.elapsed() >= every {
            let started = Instant::now();
            match replay::Recording::open(partial) {
                // Nothing to show before the first chunk is written.
                Ok(read) if read.first_tick().is_none() => {}
                Ok(read) => {
                    dirty = false;
                    if reads.send(Arc::new(read)).is_err() {
                        break;
                    }
                }
                Err(error) => log::warn!("Observer recording: cannot read it: {error}"),
            }
            every = REFRESH.max(started.elapsed() * 3);
            last_read = Instant::now();
        }
    }
}

fn write(writer: &mut replay::Writer, out: Out) -> Result<(), replay::Error> {
    for info in &out.aircraft {
        writer.register_aircraft(info)?;
    }
    for info in &out.weapons {
        writer.register_weapon(info)?;
    }
    for frame in &out.frames {
        writer.push(frame)?;
    }
    Ok(())
}

/// The game's one watch of a flying mission, from the host's start of the
/// observer flight until it ends: the frames turned into a growing
/// recording.
pub struct Observing {
    feeder: Feeder,
    store: Option<Store>,
    /// Where the recording's file goes.
    folder: PathBuf,
    /// Why the recording could not start: the watch cannot be shown.
    failure: Option<String>,
}

impl Observing {
    pub fn new(folder: PathBuf) -> Self {
        Self::remove_stale(&folder);
        Self {
            feeder: Feeder::new(),
            store: None,
            folder,
            failure: None,
        }
    }

    /// The folder a game's observer recordings go in: `observer/` in the data
    /// folder, or the system's temporary folder when there is none.
    pub fn folder() -> PathBuf {
        crate::assets::data_directory()
            .map(|data| data.join("observer"))
            .unwrap_or_else(|_| std::env::temp_dir().join("tore-observer"))
    }

    /// Removes what a game that crashed left in `folder` a day or more ago.
    fn remove_stale(folder: &Path) {
        let Ok(entries) = std::fs::read_dir(folder) else {
            return;
        };
        let day = Duration::from_secs(24 * 60 * 60);
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|made| made.elapsed().ok())
                .is_some_and(|age| age >= day);
            if old && entry.file_name().to_string_lossy().starts_with("observer-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// One more frame of the picture. The first one starts the recording,
    /// with the mission's world and models.
    pub fn feed(&mut self, frame: &ObserverFrame, around: Context<'_>) {
        if self.failure.is_some() {
            return;
        }
        if self.store.is_none() {
            let Some(world) = around.world else {
                return;
            };
            match Store::start(&self.folder, &header(&world.terrain, &frame.picture.models)) {
                Ok(store) => self.store = Some(store),
                Err(error) => {
                    log::warn!("Observer recording: {error}");
                    self.failure = Some(error);
                    return;
                }
            }
        }
        let out = self.feeder.push(frame, around);
        if let Some(store) = &self.store
            && !out.frames.is_empty()
        {
            store.write(out);
        }
    }

    /// Why the recording could not start, when it could not.
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// The newest read of the recording, when there is one the viewer has
    /// not seen.
    pub fn newest(&self) -> Option<Arc<replay::Recording>> {
        self.store.as_ref()?.newest()
    }
}

/// Whether an observer flight the host started is shown on the observer
/// screen: in a game with a lobby screen and no flight, and (slice F2-O3)
/// over the flight of a player whose aircraft the AI flies (`away`). Any
/// other flight, a flying player's included, has no watch to show.
pub fn shows_watch(flying: bool, away: bool, lobby_screen: bool) -> bool {
    if flying { away } else { lobby_screen }
}

/// Whether the viewer may open now, with the first frames recorded: over
/// the lobby on the main screen, or, for the player's own aircraft, over the
/// flight screen once no screen of the controls, graphics or sound is open
/// over it (`screens`).
pub fn viewer_may_open(away: bool, screen: Screen, screens: bool) -> bool {
    if away {
        screen == Screen::Flight && !screens
    } else {
        screen == Screen::Main
    }
}

impl App {
    /// The host started or ended the game's observer flight.
    pub(crate) fn observe_event(&mut self, started: bool) {
        if !started {
            self.end_observing();
            return;
        }
        // A watch is the screen of a game in its lobby, and (slice F2-O3) of
        // a flying player whose aircraft the AI flies, which the flight's
        // screen waits under; any other flight has no watch to show.
        let away = self.net_flight.is_some()
            && self
                .net
                .as_ref()
                .is_some_and(|s| s.client.ai_flies().is_some());
        if !shows_watch(self.net_flight.is_some(), away, self.lobby.screen.is_some()) {
            return;
        }
        self.observing = Some(Observing::new(Observing::folder()));
    }

    /// The watch's turn each frame: the newest picture goes into the
    /// recording, the camera's subject to the host, and what the recording
    /// has grown to into the viewer, which opens once there are frames.
    pub(crate) fn observe_turn(&mut self) {
        let Some(observing) = &mut self.observing else {
            return;
        };
        let Some(session) = &mut self.net else {
            return;
        };
        let now = session.now();
        if let Some(frame) = session.client.observer_frame(now) {
            observing.feed(
                &frame,
                Context {
                    roster: session.client.roster(),
                    world: session.client.mission(),
                },
            );
        }
        if let Some(failure) = observing.failure() {
            // Nothing can be shown: say so, and stop the watch. An away
            // player has no menu to come back from, so it takes its aircraft
            // back (stopping the watch would leave it to the AI).
            let mut words = format!("Could not show the mission: {failure}");
            if self.net_flight.is_none() {
                session.client.stop_watching();
            } else {
                // No menu to take the aircraft back from (slice F2-O4).
                session.client.back();
                words.push_str(" Taking your aircraft back from the AI.");
            }
            self.observing = None;
            self.message(words);
            return;
        }
        let live = self.replay.as_mut().filter(|replay| replay.viewer.live());
        if let Some(replay) = &live {
            session.client.watch(replay.viewer.camera_subject());
        }
        let Some(read) = observing.newest() else {
            return;
        };
        if let Some(replay) = live {
            replay.viewer.grow(read);
        } else if self.replay.is_none() {
            // An away player's own aircraft (slice F2-O3) is watched over
            // the flight, once no screen of the controls, graphics or sound
            // is open over it; any other watch over the lobby.
            let plane = session
                .client
                .ai_flies()
                .filter(|_| self.net_flight.is_some());
            let screens = self.controls.is_some()
                || self.graphics_screen.is_some()
                || self.sound_screen.is_some();
            if !viewer_may_open(plane.is_some(), self.screen, screens) {
                return;
            }
            let options = Options {
                aircraft: plane.filter(|plane| read.aircraft().any(|a| a.id == *plane)),
                ..Options::default()
            };
            match Viewer::open_live(read, session.resources(), &options) {
                Ok(viewer) => {
                    log::info!("Observer screen: watching the mission");
                    if plane.is_some() {
                        log::info!("Observer screen: watching the player's own aircraft");
                        self.put_flight_aside();
                    }
                    self.start_replay(viewer, None);
                }
                Err(error) => {
                    log::warn!("Observer screen: {error}");
                    let mut words = format!("Could not show the mission: {error}");
                    if plane.is_none() {
                        session.client.stop_watching();
                    } else {
                        // No menu to take the aircraft back from (slice
                        // F2-O4).
                        session.client.back();
                        words.push_str(" Taking your aircraft back from the AI.");
                    }
                    self.observing = None;
                    self.message(words);
                }
            }
        }
    }

    /// The flight's menu, map and held keys are put away as the observer
    /// screen opens over it: its controls are neutral, and nothing of them
    /// is left pressed or open for the flight that returns.
    fn put_flight_aside(&mut self) {
        self.flight_ui.menu = false;
        self.flight_ui.map.open = false;
        self.flight_ui.map.cancel_press();
        self.flight_ui.cancel_press();
        self.instruments.cancel_press();
        self.camera.keys.clear();
        self.release_trigger();
        self.input.release_keys();
    }

    /// The watch is over (the host ended it, the mission ended, the session
    /// did): the recording goes, and a live viewer on screen returns to the
    /// lobby.
    pub(crate) fn end_observing(&mut self) {
        self.observing = None;
        if self.replay.as_ref().is_some_and(|r| r.viewer.live()) {
            if self.net_flight.is_some() {
                log::info!("Observer screen: back to the flight");
            } else {
                log::info!("Observer screen: back to the lobby");
            }
            self.leave_live_replay();
        }
    }

    /// The player chose Stop Watching: the host is told, and the lobby shows.
    pub(crate) fn stop_observing(&mut self) {
        // An away player's menu has no Stop Watching row (slice F2-O4);
        // should one answer, it is Take Back Flight.
        if self.away_watching() {
            self.away_take_back();
            return;
        }
        if let Some(session) = &mut self.net {
            session.client.stop_watching();
        }
        self.end_observing();
        self.message("You stopped watching.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::playback::Playback;
    use crate::snapshot::{Damage, Draw, EffectPose, MarkPose};
    use tore_session::client::observe::WatchFlight;
    use tore_session::wire::messages::RosterPilot;
    use tore_sim::ai::launch::{Side, WingId};
    use tore_sim::combat::live::EffectKind;

    fn plane(id: u32, x: f64) -> AircraftPose {
        AircraftPose {
            id,
            aircraft: Some(AircraftId::F18),
            draw: Draw::Model(AircraftId::F18),
            position: [x, 10_000., 5_000. + f64::from(id)],
            velocity: [300., 0., 0.],
            airborne: true,
            damage: Damage {
                hp: 100,
                initial_hp: 100,
                ..Damage::default()
            },
            ..AircraftPose::default()
        }
    }

    fn ground(id: u32, hp: i32) -> AircraftPose {
        AircraftPose {
            id,
            position: [1_000., 0., 1_000.],
            damage: Damage {
                hp,
                initial_hp: 50,
                ..Damage::default()
            },
            crashed: hp <= 0,
            ..AircraftPose::default()
        }
    }

    fn missile(id: u32, owner: u32, x: f64) -> ProjectilePose {
        ProjectilePose {
            id,
            owner,
            weapon: "AIM9.JT".into(),
            shape: None,
            gun: false,
            tracer: false,
            position: [x, 10_000., 5_000.],
            previous: [x - 10., 10_000., 5_000.],
            direction: [1., 0., 0.],
            target: Some(3),
            incoming: false,
            speed_f8: 1_000 * 256,
        }
    }

    /// An observer frame showing `render` with `targets`.
    fn frame(render: f64, targets: Vec<AircraftPose>) -> ObserverFrame {
        ObserverFrame {
            flight: WatchFlight {
                flight: 1,
                delay_seconds: 0,
                first_tick: 0,
            },
            render_tick: render,
            picture: RenderSnapshot {
                tick: render as u64,
                player: AircraftPose {
                    id: NO_PLAYER,
                    ..AircraftPose::default()
                },
                targets,
                models: vec![AircraftId::F18],
                ..RenderSnapshot::default()
            },
            events: Vec::new(),
        }
    }

    fn roster(planes: &[(u32, bool)]) -> Roster {
        Roster {
            planes: planes
                .iter()
                .map(|&(id, friendly)| RosterPlane {
                    id,
                    wing: WingId::new(
                        if friendly {
                            Side::Friendly
                        } else {
                            Side::Enemy
                        },
                        0,
                    )
                    .unwrap(),
                    member: id as u8,
                    aircraft: AircraftId::F18,
                    pilot: if id == 0 {
                        RosterPilot::Human {
                            seat: 0,
                            callsign: "Viper".into(),
                        }
                    } else {
                        RosterPilot::Ai
                    },
                })
                .collect(),
        }
    }

    fn feed(feeder: &mut Feeder, frame: &ObserverFrame, roster: &Roster) -> Out {
        feeder.push(
            frame,
            Context {
                roster: Some(roster),
                world: None,
            },
        )
    }

    fn ticks(out: &Out) -> Vec<u64> {
        out.frames.iter().map(|f| f.tick).collect()
    }

    #[test]
    fn a_frame_is_made_for_every_host_tick_the_picture_passes() {
        let roster = roster(&[(0, true), (3, false)]);
        let mut feeder = Feeder::new();
        let at = |render: f64, x: f64| frame(render, vec![plane(0, x), plane(3, x + 500.)]);
        // The first picture gives one frame, at the tick it is in.
        let out = feed(&mut feeder, &at(100.5, 0.), &roster);
        assert_eq!(ticks(&out), [100]);
        assert_eq!(out.frames[0].aircraft.len(), 2);
        // Drawing slower than the host's ticks: the ticks between are the
        // two pictures blended, and each aircraft keeps its real id.
        let out = feed(&mut feeder, &at(103.5, 300.), &roster);
        assert_eq!(ticks(&out), [101, 102, 103]);
        let xs: Vec<f64> = out
            .frames
            .iter()
            .map(|f| f.aircraft[0].position[0])
            .collect();
        assert!(
            (xs[0] - 50.).abs() < 1e-9 && (xs[1] - 150.).abs() < 1e-9,
            "{xs:?}"
        );
        assert!((xs[2] - 250.).abs() < 1e-9, "{xs:?}");
        assert_eq!(
            out.frames[0]
                .aircraft
                .iter()
                .map(|a| a.id)
                .collect::<Vec<_>>(),
            [0, 3]
        );
        // A picture still inside the tick adds none; the next tick adds one.
        assert!(
            feed(&mut feeder, &at(103.9, 330.), &roster)
                .frames
                .is_empty()
        );
        assert_eq!(ticks(&feed(&mut feeder, &at(104.0, 360.), &roster)), [104]);
        // A long stall is not bridged: one frame at the new tick, and a gap.
        let out = feed(&mut feeder, &at(900.0, 9_000.), &roster);
        assert_eq!(ticks(&out), [900]);
        assert_eq!(
            ticks(&feed(&mut feeder, &at(901.2, 9_010.), &roster)),
            [901]
        );
        // A render clock that went back adds nothing.
        assert!(feed(&mut feeder, &at(850., 0.), &roster).frames.is_empty());
        // A drawn nothing is not a frame.
        assert!(
            feed(&mut feeder, &at(f64::NAN, 0.), &roster)
                .frames
                .is_empty()
        );
    }

    #[test]
    fn what_the_frames_name_is_registered_once() {
        let roster = roster(&[(0, true), (3, false)]);
        let mut feeder = Feeder::new();
        let mut first = frame(10.0, vec![plane(0, 0.), plane(3, 100.), ground(900, 50)]);
        first.picture.projectiles = vec![missile(70_001, 3, 50.)];
        let out = feed(&mut feeder, &first, &roster);
        // Aircraft, not the ground object, are registered, as the roster
        // names them.
        assert_eq!(out.aircraft.len(), 2);
        let viper = out.aircraft.iter().find(|a| a.id == 0).unwrap();
        assert_eq!((viper.label.as_str(), viper.human), ("Viper", true));
        assert_eq!(viper.side, replay::Side::Friendly);
        let foe = out.aircraft.iter().find(|a| a.id == 3).unwrap();
        assert_eq!((foe.human, foe.side), (false, replay::Side::Enemy));
        assert_eq!(foe.pt, "F18.PT");
        // The weapon is registered and the launch is an event.
        assert_eq!(out.weapons.len(), 1);
        assert_eq!(out.weapons[0].source, "AIM9.JT");
        assert_eq!(out.weapons[0].class, replay::WeaponClass::Missile);
        let launch = &out.frames[0].events[0];
        assert_eq!(launch.kind, vocab::kind::WEAPON_LAUNCH);
        assert_eq!(launch.subject, Some(3));
        assert_eq!(launch.id(vocab::field::PROJECTILE), Some(70_001));
        assert_eq!(launch.object, Some(3));
        let shot = &out.frames[0].projectiles[0];
        assert_eq!((shot.id, shot.owner, shot.weapon), (70_001, 3, 0));
        // Nothing is told twice: the same aircraft, weapon and flying shot.
        let mut second = frame(11.0, vec![plane(0, 10.), plane(3, 110.), ground(900, 50)]);
        second.picture.projectiles = vec![missile(70_001, 3, 80.)];
        let out = feed(&mut feeder, &second, &roster);
        assert!(out.aircraft.is_empty() && out.weapons.is_empty());
        assert!(out.frames[0].events.is_empty());
        assert_eq!(out.frames[0].projectiles[0].age, 1);
        // A new shot of the same weapon is another launch, with no new
        // registry entry.
        let mut third = frame(12.0, vec![plane(0, 20.), plane(3, 120.), ground(900, 50)]);
        third.picture.projectiles = vec![missile(70_001, 3, 90.), missile(70_002, 0, 5.)];
        let out = feed(&mut feeder, &third, &roster);
        assert!(out.weapons.is_empty());
        assert_eq!(out.frames[0].events.len(), 1);
        assert_eq!(out.frames[0].events[0].subject, Some(0));
        // An aircraft not in the roster (a revival's plane) is registered
        // with its type.
        let fourth = frame(13.0, vec![plane(0, 30.), plane(3, 130.), plane(9, 5.)]);
        let out = feed(&mut feeder, &fourth, &roster);
        assert_eq!(out.aircraft.len(), 1);
        assert_eq!(
            (out.aircraft[0].id, out.aircraft[0].pt.as_str()),
            (9, "F18.PT")
        );
    }

    #[test]
    fn effects_craters_losses_and_fallen_ground_objects_are_noted_once() {
        let roster = roster(&[(0, true), (3, false)]);
        let mut feeder = Feeder::new();
        let flash = EffectPose {
            kind: EffectKind::Hit,
            position: [1., 2., 3.],
            ticks: 30,
            blast: None,
        };
        let crater = MarkPose {
            kind: blast::MarkKind::Crater(2),
            position: [4., 0., 4.],
            age: 7,
            strength: 1.,
        };
        let mut first = frame(10.0, vec![plane(0, 0.), plane(3, 100.), ground(900, 50)]);
        let out = feed(&mut feeder, &first, &roster);
        assert!(out.frames[0].new_effects.is_empty() && out.frames[0].surface_hp.is_empty());
        // A hit, a crater, a loss and a fallen building, all in one picture.
        let mut lost = plane(3, 110.);
        lost.damage.hp = 0;
        lost.crashed = true;
        first = frame(11.0, vec![plane(0, 10.), lost.clone(), ground(900, 0)]);
        first.picture.effects = vec![flash];
        first.picture.marks = vec![crater];
        let out = feed(&mut feeder, &first, &roster);
        let made = &out.frames[0];
        assert_eq!(made.new_effects.len(), 2);
        assert_eq!(made.new_effects[0].duration_ticks, 30);
        assert_eq!(made.new_effects[1].kind, replay::EffectKind::Crater(2));
        assert_eq!(made.new_effects[1].duration_ticks, blast::FOREVER);
        assert_eq!(made.surface_hp, [(900, 0)]);
        let destroyed = made
            .events
            .iter()
            .find(|e| e.kind == vocab::kind::COMBAT_DESTROYED)
            .expect("the loss");
        assert_eq!(destroyed.subject, Some(3));
        assert!(
            !made
                .aircraft
                .iter()
                .find(|a| a.id == 3)
                .unwrap()
                .flags
                .alive
        );
        // The flash goes on playing with fewer ticks left, the crater and
        // the wreck stay: none is new.
        let mut next = frame(12.0, vec![plane(0, 20.), lost.clone(), ground(900, 0)]);
        next.picture.effects = vec![EffectPose { ticks: 29, ..flash }];
        next.picture.marks = vec![MarkPose { age: 8, ..crater }];
        // (A flash with fewer ticks left is another look to the feeder's
        // key, which names kind, explosion and place only.)
        let out = feed(&mut feeder, &next, &roster);
        let made = &out.frames[0];
        assert!(made.new_effects.is_empty(), "{:?}", made.new_effects);
        assert!(made.surface_hp.is_empty() && made.events.is_empty());
        // An aircraft that was already down when the watch began is not a
        // loss of the watch.
        let mut late = Feeder::new();
        let out = feed(&mut late, &frame(50.0, vec![plane(0, 0.), lost]), &roster);
        assert!(out.frames[0].events.is_empty());
    }

    /// The whole way: frames fed as a game's turns would feed them reach a
    /// recording the viewer's playback draws as the mission, with the
    /// aircraft in their real ids and no player, and the file is gone when
    /// the watch ends.
    #[test]
    fn the_fed_frames_grow_a_recording_the_viewers_playback_draws() {
        let roster = roster(&[(0, true), (3, false)]);
        let terrain = tore_world::test_support::terrain();
        let folder = crate::replay::tests::TempDir::new("observe-store");
        let store = Store::start(folder.path(), &header(&terrain, &[AircraftId::F18])).unwrap();
        let mut feeder = Feeder::new();
        // Five seconds of the mission, a picture every other tick.
        let mut render = 1_000.;
        while render < 1_600. {
            let mut shown = frame(
                render,
                vec![
                    plane(0, render * 2.),
                    plane(3, render * 2. + 500.),
                    ground(900, 50),
                ],
            );
            if render >= 1_300. {
                shown.picture.projectiles = vec![missile(70_001, 3, render)];
            }
            let out = feed(&mut feeder, &shown, &roster);
            store.write(out);
            render += 2.;
        }
        // The newest chunk is written once it is full: a fifth of a second
        // of frames waits for it.
        let read = store.wait_for(1_570);
        assert!(read.problems().is_empty(), "{:?}", read.problems());
        let header = read.header();
        let presentation = Presentation::from_header(header);
        assert_eq!(presentation.player, NO_PLAYER);
        assert_eq!(presentation.models, [AircraftId::F18]);
        assert!(presentation.slots >= 1_000);
        assert_eq!(header.mission.title(), "Observing");
        assert_eq!(read.first_tick(), Some(1_000));
        assert_eq!(read.aircraft().count(), 2);
        assert_eq!(read.weapons().count(), 1);
        let mut playback = Playback::new(Arc::clone(&read));
        let tick = read.last_tick().unwrap();
        let picture = playback.picture(tick, 1.);
        assert_eq!(picture.player.id, NO_PLAYER, "no plane is the player's");
        let ids: Vec<u32> = picture.targets.iter().map(|t| t.id).collect();
        assert_eq!(ids, [0, 3]);
        assert!(
            picture
                .targets
                .iter()
                .all(|t| t.aircraft == Some(AircraftId::F18))
        );
        assert_eq!(picture.projectiles.len(), 1);
        // The path is smooth: every tick has its frame, two feet apart as
        // the aircraft flew.
        let mut at = |tick: u64| playback.aircraft(tick, 0).unwrap().position[0];
        assert!((at(1_101) - at(1_100) - 2.).abs() < 1e-6);
        // The file is where the store says while it is being written, and
        // gone with it.
        let partial = store.partial();
        assert!(partial.exists());
        drop(store);
        assert!(!partial.exists());
    }

    /// An observer's own frames, from a real host and bot on the network
    /// simulator, make a recording of the whole stretch: every tick has its
    /// frame, the four planes keep their ids and are registered as the
    /// roster names them, and each is where the observer drew it.
    #[test]
    fn a_real_observers_frames_make_a_recording_of_the_whole_stretch() {
        let watched = tore_session::fixture::observed_fight(20);
        let client = &watched.client;
        assert!(
            watched.frames.len() > 900,
            "{} frames",
            watched.frames.len()
        );
        let folder = crate::replay::tests::TempDir::new("observe-real");
        let mut observing = Observing::new(folder.path().join("watch"));
        for frame in &watched.frames {
            observing.feed(
                frame,
                Context {
                    roster: client.roster(),
                    world: client.mission(),
                },
            );
        }
        let newest = watched.frames.last().unwrap().render_tick as u64;
        let read = observing
            .store
            .as_ref()
            .expect("the first frame started the recording")
            .wait_for(newest - 40);
        assert!(read.problems().is_empty(), "{:?}", read.problems());
        assert!(read.gaps().is_empty(), "every tick has a frame");
        // The client draws a moment before the mission's first tick too;
        // there is nothing to record then.
        let first = watched
            .frames
            .iter()
            .find(|f| f.render_tick >= 0.)
            .map(|f| f.render_tick as u64)
            .unwrap();
        assert_eq!(read.first_tick(), Some(first));
        let names: Vec<(u32, String)> = read.aircraft().map(|a| (a.id, a.label.clone())).collect();
        assert_eq!(names.len(), 4, "{names:?}");
        assert_eq!(names[0], (0, "Alpha".into()));
        assert!(names[1..].iter().all(|(_, label)| label.contains("-")));
        let presentation = Presentation::from_header(read.header());
        assert_eq!(
            (presentation.player, presentation.models.len()),
            (NO_PLAYER, 1)
        );
        // Each plane is where the observer drew it, to within what it flies
        // between the drawn instant and the tick's own.
        let mut playback = Playback::new(Arc::clone(&read));
        let mut checked = 0;
        for frame in watched.frames.iter().skip(60).step_by(37) {
            let tick = frame.render_tick.floor() as u64;
            if tick > read.last_tick().unwrap() {
                continue;
            }
            let picture = playback.picture(tick, 1.);
            assert_eq!(picture.player.id, NO_PLAYER);
            let ids: Vec<u32> = picture.targets.iter().map(|t| t.id).collect();
            assert_eq!(ids, [0, 1, 2, 3], "tick {tick}");
            for drawn in frame
                .picture
                .targets
                .iter()
                .filter(|t| t.aircraft.is_some())
            {
                let recorded = picture.target(drawn.id).unwrap();
                let off = (0..3)
                    .map(|i| (recorded.position[i] - drawn.position[i]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                assert!(
                    off < 12.,
                    "plane {} is {off} ft off at tick {tick}",
                    drawn.id
                );
                checked += 1;
            }
        }
        assert!(checked > 100, "{checked}");
    }

    /// Ten minutes of thirty aircraft: how big the file is and how long a
    /// read of it takes, which bounds how soon the viewer sees new frames.
    /// Slow in a debug build (about half a minute to make the frames): run by
    /// name in the full suite, `cargo test --release -p tore-app --bin
    /// tore-app observe::tests::ten_minutes -- --ignored --nocapture`.
    #[test]
    #[ignore = "ten minutes of frames: for the full run, and for the numbers in the design"]
    fn ten_minutes_of_thirty_aircraft_are_read_in_the_time_the_store_allows() {
        let folder = crate::replay::tests::TempDir::new("observe-ten");
        let terrain = tore_world::test_support::terrain();
        let store = Store::start(folder.path(), &header(&terrain, &[AircraftId::F18])).unwrap();
        let ids: Vec<(u32, bool)> = (0..30).map(|id| (id, id < 15)).collect();
        let roster = roster(&ids);
        let mut feeder = Feeder::new();
        let ticks = 10 * 60 * 120;
        for tick in 0..ticks {
            let t = tick as f64;
            let targets = (0..30)
                .map(|id| {
                    let a = t * 0.001 + f64::from(id);
                    let mut pose = plane(id, 100_000. + a.sin() * 20_000. + t * 2.);
                    pose.attitude = [a, 0.05 * a.sin(), 0.4 * a.cos()];
                    pose
                })
                .collect();
            store.write(feed(&mut feeder, &frame(t + 0.5, targets), &roster));
        }
        let read = store.wait_for(ticks - 60);
        let size = std::fs::metadata(store.partial()).unwrap().len();
        let started = Instant::now();
        let again = replay::Recording::open(store.partial()).unwrap();
        let took = started.elapsed();
        eprintln!(
            "ten minutes, 30 aircraft: {} chunks, {:.1} MB, a read takes {:?}",
            again.chunks().len(),
            size as f64 / 1e6,
            took
        );
        assert!(read.problems().is_empty());
        assert!(size < 40 << 20, "{size} bytes");
        assert!(took < Duration::from_secs(5), "{took:?}");
    }

    /// Who shows a watch and when the viewer opens (slice F2-O3).
    #[test]
    fn a_watch_is_shown_in_the_lobby_and_over_an_away_players_flight() {
        // A game in its lobby shows it, one with no lobby screen does not.
        assert!(shows_watch(false, false, true));
        assert!(!shows_watch(false, false, false));
        // A flying player's own watch is shown only while the AI flies its
        // aircraft, with or without a lobby screen.
        assert!(!shows_watch(true, false, true));
        assert!(!shows_watch(true, false, false));
        assert!(shows_watch(true, true, true));
        assert!(shows_watch(true, true, false));
        // The viewer opens over the lobby on the main screen, over the
        // flight for the away player's own aircraft, and waits while a
        // screen of the controls, graphics or sound is open over the flight.
        assert!(viewer_may_open(false, Screen::Main, false));
        assert!(!viewer_may_open(false, Screen::Flight, false));
        assert!(viewer_may_open(true, Screen::Flight, false));
        assert!(!viewer_may_open(true, Screen::Flight, true));
        assert!(!viewer_may_open(true, Screen::Main, false));
        assert!(!viewer_may_open(true, Screen::Replay, false));
    }

    /// A recording that cannot start says why once and is not tried again
    /// every frame; nothing is left behind.
    #[test]
    fn a_recording_that_cannot_start_says_so_and_is_not_retried() {
        let watched = tore_session::fixture::observed_fight(3);
        let folder = crate::replay::tests::TempDir::new("observe-failure");
        // The folder is a file: nothing can be made in it.
        let blocked = folder.path().join("blocked");
        std::fs::write(&blocked, b"a file").unwrap();
        let mut observing = Observing::new(blocked.clone());
        let around = Context {
            roster: watched.client.roster(),
            world: watched.client.mission(),
        };
        let frame = watched.frames.iter().find(|f| f.render_tick >= 0.).unwrap();
        observing.feed(frame, around);
        let failure = observing.failure().expect("it failed").to_owned();
        assert!(failure.contains("blocked"), "{failure}");
        observing.feed(frame, around);
        assert_eq!(observing.failure(), Some(failure.as_str()));
        assert!(observing.newest().is_none());
        assert_eq!(std::fs::read(&blocked).unwrap(), b"a file");
        // Without a mission to build the header from, the first frame waits.
        let mut waiting = Observing::new(folder.path().join("waiting"));
        waiting.feed(frame, Context::default());
        assert!(waiting.failure().is_none() && waiting.store.is_none());
    }
}
