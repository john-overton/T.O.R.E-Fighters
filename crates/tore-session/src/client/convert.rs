//! A capture converted into a replay (docs/ARCHITECTURE.md, "Converting a
//! capture into a replay"; the format is docs/REPLAYS.md).
//!
//! [`observe`] runs the capture again offline with an observer
//! ([`super::seen`]) and the diagnostics writer on; [`Conversion::write`]
//! then builds one replay frame for every host tick of a flight, from the
//! states the client was given, and hands each to a `tore_replay::Writer`.
//!
//! Everything here is a pure function of the capture and the import: nothing
//! reads a clock, the system or an unordered map, so converting twice gives
//! the same bytes.

use super::capture::{self, CaptureError, kind as record_kind};
use super::seen::{FlightSeen, Observed, OwnSample};
use super::{Client, ClientEvent};
use crate::host::BuildId;
use crate::wire::entity::{EntityKey, EntityKind, EntityState, POSITION_STEP, radians};
use crate::wire::messages::RosterPilot;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tore_formats::aircraft::AircraftId;
use tore_replay as replay;
use tore_sim::attitude::Basis;
use tore_sim::combat::ledger::ShotKind;
use tore_sim::flight::DT;
use tore_sim::{ejection, wreck};
use tore_world::snapshot::DEVICES;
use tore_world::world::World;

pub(crate) mod events;
pub(crate) mod smooth;

use crate::client::interpolation::Sample;
use smooth::{Own, Track};

/// The longest span between two received states a curve is drawn across,
/// ticks (2 seconds). A longer silence leaves the entity out of the frames in
/// between.
pub const MAX_BRIDGE_TICKS: u32 = 240;

/// The header key naming the plane the player flew when the conversion gave
/// it the id 0 (the viewer looks for the player there).
pub const PLAYER_PLANE_KEY: &str = "net.player_plane";

/// Why a capture did not convert.
#[derive(Debug)]
pub enum ConvertError {
    /// The capture does not run (see [`CaptureError`]).
    Capture(CaptureError),
    /// The capture holds no flight with a player seated in it.
    NoFlight(String),
    /// The replay could not be written.
    Replay(replay::Error),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capture(error) => error.fmt(f),
            Self::NoFlight(why) => f.write_str(why),
            Self::Replay(error) => write!(f, "the replay could not be written: {error}"),
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<CaptureError> for ConvertError {
    fn from(error: CaptureError) -> Self {
        Self::Capture(error)
    }
}

impl From<replay::Error> for ConvertError {
    fn from(error: replay::Error) -> Self {
        Self::Replay(error)
    }
}

/// How the capture's join began, which the header keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartInfo {
    pub server: SocketAddr,
    pub callsign: String,
    pub build: BuildId,
    pub plane: Option<u32>,
}

/// A capture that ended inside a record or without its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    /// The byte the last whole record ended at, and the capture's size.
    pub at_byte: usize,
    pub of_bytes: usize,
    /// The client time of the last record, seconds.
    pub seconds: f64,
}

/// One line of the diagnostics log, with the host tick the client was
/// drawing when it was written.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagLine {
    /// Client seconds, from the line itself.
    pub seconds: f64,
    pub tick: Option<f64>,
    /// The line's tab-separated text, time and kind first.
    pub text: String,
}

/// How a flight ended, for the footer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// The player left it (End Mission or Leave).
    Left,
    /// The host ended the mission.
    MissionEnded(String),
    /// The connection closed.
    Closed(String),
    /// The capture stops first.
    Cut,
}

impl End {
    fn footer(&self) -> &'static str {
        match self {
            Self::Left => "end flight",
            Self::MissionEnded(_) => "end mission",
            Self::Closed(_) => "exit",
            Self::Cut => "cut",
        }
    }
}

/// A capture run again: everything the client was given, ready to write as
/// replays.
pub struct Conversion {
    client: Client,
    pub start: StartInfo,
    observed: Observed,
    /// `Some` when the capture stops short.
    pub cut: Option<Cut>,
    pub capture_format: u16,
    pub protocol: u16,
    diagnostics: Vec<DiagLine>,
    /// Client times the player left, the host ended a mission or the
    /// connection closed.
    ends: Vec<(Duration, End)>,
    last: Duration,
    models: Vec<AircraftId>,
}

/// What one flight of a conversion holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlightInfo {
    /// The flight's index among the capture's flights.
    pub index: usize,
    /// The plane the player flew.
    pub plane: u32,
    /// The ticks the replay covers.
    pub first_tick: u64,
    pub last_tick: u64,
    /// The aircraft types the player flew.
    pub aircraft: Option<AircraftId>,
}

/// What [`Conversion::write`] wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub path: PathBuf,
    pub frames: u64,
    pub aircraft: usize,
    pub seconds: f64,
    pub cut: Option<Cut>,
}

/// A shared byte sink the diagnostics log is written into.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<u8>>>);

impl Write for Lines {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("poisoned"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Lines {
    fn take(&self) -> String {
        let mut bytes = self
            .0
            .lock()
            .map(|mut b| std::mem::take(&mut *b))
            .unwrap_or_default();
        String::from_utf8_lossy(&std::mem::take(&mut bytes)).into_owned()
    }
}

/// Runs `capture` again offline with the game data `resources` and keeps
/// what the client was given.
pub fn observe(
    capture: &[u8],
    resources: Arc<BTreeMap<String, Vec<u8>>>,
) -> Result<Conversion, CaptureError> {
    let mut reader = capture::Reader::new(capture)?;
    let (format, protocol) = (
        u16::from_le_bytes([capture[8], capture[9]]),
        u16::from_le_bytes([capture[10], capture[11]]),
    );
    let Some(capture::Record::Start {
        server,
        callsign,
        build,
        plane,
        ..
    }) = reader.next_record()?
    else {
        return Err(CaptureError::Damaged("no start record"));
    };
    let start = StartInfo {
        server,
        callsign,
        build,
        plane,
    };
    let lines = Lines::default();
    let mut diagnostics: Vec<DiagLine> = Vec::new();
    let mut ends: Vec<(Duration, End)> = Vec::new();
    let mut last = Duration::ZERO;
    let mut finished = false;
    let sink = lines.clone();
    let mut run = capture::run(
        capture,
        resources,
        &mut |client| {
            client.start_observing();
            client.set_diagnostics(Box::new(sink.clone()));
        },
        &mut |_| {},
        &mut |client, at, record| {
            last = last.max(at);
            let tick = client.render_tick();
            for line in lines.take().lines() {
                if line.starts_with("seconds\t") {
                    continue;
                }
                let seconds = line
                    .split('\t')
                    .next()
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(at.as_secs_f64());
                diagnostics.push(DiagLine {
                    seconds,
                    tick,
                    text: line.to_owned(),
                });
            }
            while let Some(event) = client.poll_event() {
                match event {
                    ClientEvent::MissionEnded(ended) => {
                        ends.push((at, End::MissionEnded(super::ended_text(&ended))));
                    }
                    ClientEvent::Closed(reason) => {
                        finished = true;
                        ends.push((at, End::Closed(super::describe(&reason))));
                    }
                    _ => {}
                }
            }
            if matches!(record, record_kind::LEAVE | record_kind::LEAVE_GAME) {
                ends.push((at, End::Left));
            }
            finished |= record == record_kind::DISCONNECT;
        },
    )?;
    // Whole when every record read and the session ended: the player quit
    // (a Disconnect record) or the connection closed.
    let cut_short = run.consumed < capture.len() || !finished;
    let observed = run.client.finish_observing().unwrap_or_default();
    let models = run.client.mission().map_or_else(Vec::new, |world| {
        let mut models: Vec<AircraftId> = world
            .combat
            .dummy_types()
            .iter()
            .map(|t| t.profile.id)
            .collect();
        models.dedup();
        models
    });
    let cut = cut_short.then_some(Cut {
        at_byte: run.consumed,
        of_bytes: capture.len(),
        seconds: last.as_secs_f64(),
    });
    for line in lines.take().lines() {
        if !line.starts_with("seconds\t") {
            diagnostics.push(DiagLine {
                seconds: line
                    .split('\t')
                    .next()
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(last.as_secs_f64()),
                tick: None,
                text: line.to_owned(),
            });
        }
    }
    Ok(Conversion {
        client: run.client,
        start,
        observed,
        cut,
        capture_format: format,
        protocol,
        diagnostics,
        ends,
        last,
        models,
    })
}

/// The window of host ticks a flight's replay covers: from the first tick a
/// snapshot showed after the player was seated (before it only the player's
/// own plane is known) to the newest tick a snapshot reached and the own
/// plane's states cover.
fn window(flight: &FlightSeen) -> Option<(u64, u64)> {
    let seat = flight.seat?;
    let ticks = || flight.snapshots.iter().map(|(tick, _)| u64::from(*tick));
    let first = ticks().filter(|tick| *tick >= u64::from(seat.tick)).min()?;
    let snapshots = ticks().max()?;
    let own = smooth::own_last_tick(&flight.trace)?;
    let last = snapshots.min(own);
    (last > first).then_some((first, last))
}

impl Conversion {
    /// What the client was given, for tests.
    #[cfg(test)]
    pub(crate) fn observed(&self) -> &Observed {
        &self.observed
    }

    /// The diagnostics lines the replayed client wrote, for tests.
    #[cfg(test)]
    pub(crate) fn diagnostic_lines(&self) -> &[DiagLine] {
        &self.diagnostics
    }

    /// The mission as the replayed client built it, for the header's world.
    pub fn mission(&self) -> Option<&World> {
        self.client.mission()
    }

    /// The flights that can be written as replays.
    pub fn flights(&self) -> Vec<FlightInfo> {
        self.observed
            .flights
            .iter()
            .enumerate()
            .filter_map(|(index, flight)| {
                let (first_tick, last_tick) = window(flight)?;
                let seat = flight.seat?;
                let aircraft = flight
                    .roster
                    .as_ref()
                    .and_then(|r| r.planes.iter().find(|p| p.id == seat.plane))
                    .map(|p| p.aircraft);
                Some(FlightInfo {
                    index,
                    plane: seat.plane,
                    first_tick,
                    last_tick,
                    aircraft,
                })
            })
            .collect()
    }

    /// Why the capture holds no flight, in words.
    pub fn no_flight_reason(&self) -> String {
        if self.observed.flights.is_empty() {
            "the capture holds no flight: the player never reached a mission".into()
        } else if self.observed.flights.iter().all(|f| f.seat.is_none()) {
            "the capture holds no flight: the player was never given a plane".into()
        } else {
            "the capture holds no flight long enough to replay: the player left it at once".into()
        }
    }

    /// How flight `index` ended.
    pub fn end_of(&self, index: usize) -> End {
        let Some(flight) = self.observed.flights.get(index) else {
            return End::Cut;
        };
        let next = self
            .observed
            .flights
            .get(index + 1)
            .map_or(Duration::MAX, |f| f.began);
        let ended = flight.ended.unwrap_or(self.last);
        self.ends
            .iter()
            .find(|(at, _)| *at >= flight.began && *at <= ended.max(flight.began) && *at < next)
            .map(|(_, end)| end.clone())
            .or_else(|| {
                self.ends
                    .iter()
                    .find(|(at, _)| *at > ended && *at < next)
                    .map(|(_, end)| end.clone())
            })
            .unwrap_or(End::Cut)
    }

    /// The diagnostics lines of flight `index`, with the tick each belongs
    /// to: the lines before the first flight belong to the first, and each
    /// flight owns the time up to the next one's start.
    fn diagnostics_of(&self, index: usize, first: u64, last: u64) -> Vec<(u64, &DiagLine)> {
        let flights = &self.observed.flights;
        let from = if index == 0 {
            0.
        } else {
            flights[index].began.as_secs_f64()
        };
        let to = flights
            .get(index + 1)
            .map_or(f64::INFINITY, |f| f.began.as_secs_f64());
        self.diagnostics
            .iter()
            .filter(|line| line.seconds >= from && line.seconds < to)
            .map(|line| {
                let tick = line
                    .tick
                    .map_or(first, |t| t.max(0.) as u64)
                    .clamp(first, last);
                (tick, line)
            })
            .collect()
    }

    /// The replay header for flight `index`: `world` is the mission's world
    /// as the caller's code describes it, `recorded_at` the time the capture
    /// started (UTC text) when its name says.
    pub fn header(
        &self,
        flight: &FlightInfo,
        world: replay::World,
        game_version: &str,
        game_commit: &str,
        recorded_at: &str,
    ) -> replay::Header {
        let seen = &self.observed.flights[flight.index];
        let ids = Ids::new(flight.plane);
        let mut extra: Vec<(String, String)> = vec![
            ("net.server".into(), self.start.server.to_string()),
            ("net.callsign".into(), self.start.callsign.clone()),
            (
                "net.build".into(),
                format!(
                    "{} ({}{})",
                    self.start.build.version,
                    self.start.build.commit,
                    if self.start.build.release {
                        ", release"
                    } else {
                        ""
                    }
                ),
            ),
            (
                "net.capture".into(),
                format!("format {}, protocol {}", self.capture_format, self.protocol),
            ),
            ("net.flight".into(), seen.flight.to_string()),
        ];
        if flight.plane != 0 {
            extra.push((PLAYER_PLANE_KEY.into(), flight.plane.to_string()));
        }
        if let Some(aircraft) = flight.aircraft {
            extra.push(("player.aircraft".into(), aircraft.selection_key().into()));
        }
        // How the viewer draws the aircraft: the loaded models, and the
        // ids up to the highest other aircraft.
        let slots = self
            .aircraft_ids(seen, &ids)
            .into_iter()
            .filter(|id| *id != 0)
            .max()
            .unwrap_or(0);
        let models: Vec<&str> = self.models.iter().map(|id| id.selection_key()).collect();
        extra.push(("draw.models".into(), models.join(",")));
        extra.push(("draw.slots".into(), slots.to_string()));
        extra.push((
            replay::model::FUEL_KEY.into(),
            replay::model::FUEL_WITH_EXTERNAL.into(),
        ));
        replay::Header {
            game_version: game_version.into(),
            game_commit: game_commit.into(),
            recorded_at: recorded_at.into(),
            mission: replay::MissionKind::Other("Network flight".into()),
            world,
            extra,
            ..replay::Header::default()
        }
    }

    /// Every aircraft id the flight names: the roster's and every aircraft
    /// entity's, mapped.
    fn aircraft_ids(&self, seen: &FlightSeen, ids: &Ids) -> BTreeSet<u32> {
        let mut out: BTreeSet<u32> = BTreeSet::new();
        if let Some(roster) = &seen.roster {
            out.extend(roster.planes.iter().map(|p| ids.map(p.id)));
        }
        out.extend(
            seen.states
                .keys()
                .filter(|k| k.kind == EntityKind::Aircraft)
                .map(|k| ids.map(k.id)),
        );
        if let Some(seat) = seen.seat {
            out.insert(ids.map(seat.plane));
        }
        out
    }

    /// The aircraft registry of a flight: the roster's planes by id, the
    /// player first as `You`.
    pub fn roster(&self, flight: &FlightInfo) -> Vec<replay::AircraftInfo> {
        let seen = &self.observed.flights[flight.index];
        let ids = Ids::new(flight.plane);
        let world = self.client.mission();
        let mut infos: BTreeMap<u32, replay::AircraftInfo> = BTreeMap::new();
        if let Some(roster) = &seen.roster {
            for plane in &roster.planes {
                infos.insert(
                    ids.map(plane.id),
                    roster_info(&ids, plane, flight.plane, world),
                );
            }
        }
        for key in seen
            .states
            .keys()
            .filter(|k| k.kind == EntityKind::Aircraft)
        {
            let id = ids.map(key.id);
            infos.entry(id).or_insert_with(|| {
                let aircraft = first_aircraft_type(seen, *key);
                replay::AircraftInfo {
                    id,
                    pt: aircraft
                        .map(|a| a.selection_key().to_owned())
                        .unwrap_or_default(),
                    name: aircraft.map(|a| a.label().to_owned()).unwrap_or_default(),
                    label: format!("Aircraft {id}"),
                    ..replay::AircraftInfo::default()
                }
            });
        }
        infos.entry(0).or_insert_with(|| replay::AircraftInfo {
            id: 0,
            label: "You".into(),
            human: true,
            skill: "Human".into(),
            side: replay::Side::Friendly,
            ..replay::AircraftInfo::default()
        });
        infos.into_values().collect()
    }

    /// The weapons the flight's projectiles are, by name, in the order the
    /// frames' ids number them.
    pub fn weapons(&self, flight: &FlightInfo) -> Vec<replay::WeaponInfo> {
        let seen = &self.observed.flights[flight.index];
        weapon_registry(seen, self.client.mission()).1
    }

    /// Builds every frame of flight `flight` in tick order and hands each to
    /// `sink`; a frame the sink refuses stops the run with its error.
    pub fn frames<E>(
        &self,
        flight: &FlightInfo,
        sink: &mut dyn FnMut(replay::Frame) -> Result<(), E>,
    ) -> Result<(), E> {
        let seen = &self.observed.flights[flight.index];
        let ids = Ids::new(flight.plane);
        let (first, last) = (flight.first_tick, flight.last_tick);
        let (weapon_ids, _) = weapon_registry(seen, self.client.mission());
        let own = Own::new(&seen.trace);
        let mut tracks: Vec<Track<'_>> = seen
            .states
            .iter()
            .map(|(key, states)| Track::new(*key, states))
            .collect();
        let launches = events::launches(seen);
        let mut events = events::Events::new(seen, ids, &weapon_ids, first, last);
        let net = self.net_events(flight);
        let mut net = net.into_iter().peekable();
        // The tick each aircraft's pilot was first seen out of it.
        let pilot_from: BTreeMap<u32, u32> = seen
            .states
            .iter()
            .filter(|(k, _)| k.kind == EntityKind::Pilot)
            .filter_map(|(k, states)| Some((k.id, states.first()?.0)))
            .collect();
        for tick in first..=last {
            let mut frame = replay::Frame {
                tick,
                ..replay::Frame::default()
            };
            if let Some(sample) = own.at(tick) {
                frame
                    .aircraft
                    .push(own_state(&sample, ids.map(sample.pose.id)));
            }
            let mut others: Vec<replay::AircraftState> = Vec::new();
            for track in &mut tracks {
                let Some(at) = track.at(tick) else {
                    continue;
                };
                let key = track.key;
                match (key.kind, at) {
                    (EntityKind::Aircraft, Sample::Aircraft(pose)) => {
                        let ejected = pilot_from
                            .get(&key.id)
                            .is_some_and(|from| u64::from(*from) <= tick);
                        others.push(aircraft_state(ids.map(key.id), pose, ejected));
                    }
                    (
                        EntityKind::Projectile,
                        Sample::Projectile(p, position, velocity, direction),
                    ) => {
                        frame.projectiles.push(projectile_state(
                            key.id,
                            &Projectile {
                                state: p,
                                position,
                                velocity,
                                direction,
                                first: track.first(),
                            },
                            &weapon_ids,
                            seen,
                            &launches,
                            tick,
                            ids,
                        ));
                    }
                    (EntityKind::Debris, Sample::Debris(_, position, attitude)) => {
                        frame.debris.push(replay::DebrisState {
                            owner: ids.map(key.id),
                            index: 0,
                            position,
                            attitude,
                        });
                    }
                    (EntityKind::Pilot, Sample::Pilot(p, position, heading)) => {
                        frame.escapees.push(replay::EscapeeState {
                            owner: ids.map(key.id),
                            position,
                            heading,
                            phase: escape_code(p.phase),
                        });
                    }
                    _ => {}
                }
            }
            others.sort_by_key(|a| a.id);
            frame.aircraft.extend(others);
            frame.aircraft.truncate(replay::limits::MAX_AIRCRAFT);
            frame.projectiles.truncate(replay::limits::MAX_PROJECTILES);
            frame.debris.truncate(replay::limits::MAX_DEBRIS);
            frame.escapees.truncate(replay::limits::MAX_ESCAPEES);
            events.fill(tick, &mut frame);
            while let Some((_, event)) = net.next_if(|(t, _)| *t <= tick) {
                frame.events.push(event);
            }
            sink(frame)?;
        }
        Ok(())
    }

    /// The `net.*` events of a flight, by tick.
    fn net_events(&self, flight: &FlightInfo) -> Vec<(u64, replay::Event)> {
        self.diagnostics_of(flight.index, flight.first_tick, flight.last_tick)
            .into_iter()
            .map(|(tick, line)| (tick, net_event(&line.text)))
            .collect()
    }

    /// The footer for flight `index`: how it ended, whether the capture was
    /// cut, and the network's figures over the flight.
    pub fn footer(&self, flight: &FlightInfo) -> replay::Footer {
        let mut result: Vec<(String, String)> = vec![("end".into(), {
            let end = self.end_of(flight.index);
            // A flight that the capture's last record ends is the cut one;
            // an earlier flight ended normally.
            let last = flight.index + 1 == self.observed.flights.len();
            if last && self.cut.is_some() && end == End::Cut {
                "cut".to_owned()
            } else {
                end.footer().to_owned()
            }
        })];
        if let End::MissionEnded(text) | End::Closed(text) = self.end_of(flight.index) {
            result.push(("reason".into(), text));
        }
        if let Some(cut) = &self.cut
            && flight.index + 1 == self.observed.flights.len()
        {
            result.push((
                "capture".into(),
                format!(
                    "cut short at byte {} of {} ({:.1} s of the client's time)",
                    cut.at_byte, cut.of_bytes, cut.seconds
                ),
            ));
        }
        let lines = self.diagnostics_of(flight.index, flight.first_tick, flight.last_tick);
        result.extend(net_summary(
            lines.iter().map(|(_, line)| line.text.as_str()),
            flight,
        ));
        replay::Footer {
            end_tick: flight.last_tick,
            result,
        }
    }

    /// Writes flight `flight` as the replay `path` with `header`: the writer
    /// makes `path.partial` and renames it when it is whole.
    pub fn write(
        &self,
        flight: &FlightInfo,
        header: &replay::Header,
        path: &Path,
    ) -> Result<Written, ConvertError> {
        self.write_with(flight, header, path, &mut Nothing)
    }

    /// [`Conversion::write`] with `regenerate` adding to every frame before
    /// it is written (the game's smoke, contrails and gun rounds).
    pub fn write_with(
        &self,
        flight: &FlightInfo,
        header: &replay::Header,
        path: &Path,
        regenerate: &mut dyn Regenerate,
    ) -> Result<Written, ConvertError> {
        let mut writer = replay::Writer::create(path, header)?;
        for info in self.roster(flight) {
            writer.register_aircraft(&info)?;
        }
        for weapon in self.weapons(flight) {
            writer.register_weapon(&weapon)?;
        }
        let mut frames = 0u64;
        self.frames(flight, &mut |mut frame| {
            frames += 1;
            for weapon in regenerate.frame(&mut frame) {
                writer.register_weapon(&weapon)?;
            }
            writer.push(&frame)
        })?;
        let finished = writer.finish(&self.footer(flight))?;
        Ok(Written {
            path: finished,
            frames,
            aircraft: self.roster(flight).len(),
            seconds: (flight.last_tick - flight.first_tick) as f64 / 120.,
            cut: self
                .cut
                .filter(|_| flight.index + 1 == self.observed.flights.len()),
        })
    }
}

/// What a caller adds to a flight's frames as they are written: the effects
/// the host does not send and the client makes again from the picture (smoke,
/// contrails and gun rounds, `replay/net_effects.rs` in the game). The
/// conversion hands over every frame in tick order, built and complete, and
/// writes what comes back. It must be a pure function of the frames it is
/// given, so converting twice gives the same bytes.
pub trait Regenerate {
    /// Adds to `frame` (`new_puffs`, `projectiles`) and returns the weapons
    /// its projectiles name that the conversion did not register, which the
    /// writer registers before the frame (a weapon returned again is the same
    /// weapon).
    fn frame(&mut self, frame: &mut replay::Frame) -> Vec<replay::WeaponInfo>;
}

/// Adds nothing: a conversion without regenerated effects.
struct Nothing;

impl Regenerate for Nothing {
    fn frame(&mut self, _: &mut replay::Frame) -> Vec<replay::WeaponInfo> {
        Vec::new()
    }
}

/// The id the replay gives each plane: the player's is 0, the viewer's
/// player, and plane 0 takes the player's id.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ids {
    own: u32,
}

impl Ids {
    pub(crate) fn new(own: u32) -> Self {
        Self { own }
    }
    pub(crate) fn map(self, id: u32) -> u32 {
        if id == self.own {
            0
        } else if id == 0 {
            self.own
        } else {
            id
        }
    }
}

fn first_aircraft_type(seen: &FlightSeen, key: EntityKey) -> Option<AircraftId> {
    seen.states
        .get(&key)?
        .iter()
        .find_map(|(_, state)| match state {
            EntityState::Aircraft(a) => a.aircraft,
            _ => None,
        })
}

fn roster_info(
    ids: &Ids,
    plane: &crate::wire::messages::RosterPlane,
    own: u32,
    world: Option<&World>,
) -> replay::AircraftInfo {
    use tore_sim::ai::launch::Side;
    let side = match plane.wing.side {
        Side::Friendly => replay::Side::Friendly,
        Side::Enemy => replay::Side::Enemy,
    };
    let (label, human, skill) = match &plane.pilot {
        _ if plane.id == own => ("You".to_owned(), true, "Human".to_owned()),
        RosterPilot::Human { callsign, .. } => (callsign.clone(), true, "Human".to_owned()),
        RosterPilot::Ai => (
            format!(
                "{} {}-{}",
                match plane.wing.side {
                    Side::Friendly => "Friendly",
                    Side::Enemy => "Enemy",
                },
                plane.wing.display_number(),
                u16::from(plane.member) + 1
            ),
            false,
            world
                .and_then(|w| w.ai_wings.as_ref())
                .and_then(|w| w.mission().actor(plane.id))
                .map(|actor| format!("{:?}", actor.experience().level))
                .unwrap_or_default(),
        ),
    };
    replay::AircraftInfo {
        id: ids.map(plane.id),
        pt: plane.aircraft.selection_key().to_owned(),
        name: plane.aircraft.label().to_owned(),
        label,
        side,
        wing: u16::from(plane.wing.display_number()),
        member: u16::from(plane.member) + 1,
        skill,
        human,
    }
}

/// The weapon names the projectiles carry, numbered in name order, and
/// their registry entries. A name that the mission's stations know gets the
/// station's identity; any other is a plain missile when it was aimed at
/// something, otherwise unclassified.
fn weapon_registry(
    seen: &FlightSeen,
    world: Option<&World>,
) -> (BTreeMap<String, u32>, Vec<replay::WeaponInfo>) {
    let mut names: BTreeMap<String, (Option<String>, bool)> = BTreeMap::new();
    for (key, states) in &seen.states {
        if key.kind != EntityKind::Projectile {
            continue;
        }
        for (_, state) in states {
            if let EntityState::Projectile(p) = state {
                let name = seen.names.name(p.weapon).unwrap_or_default().to_owned();
                let entry = names.entry(name).or_insert((None, false));
                if entry.0.is_none() {
                    entry.0 = p.shape.and_then(|s| seen.names.name(s)).map(str::to_owned);
                }
                entry.1 |= p.target.is_some();
            }
        }
    }
    let known: Vec<&tore_formats::weapons::Weapon> = world
        .map(|w| {
            w.combat
                .dummy_configurations()
                .iter()
                .flat_map(|c| c.stations.iter().map(|s| &s.weapon))
                .collect()
        })
        .unwrap_or_default();
    let mut ids = BTreeMap::new();
    let mut infos = Vec::new();
    for (id, (name, (shape, aimed))) in names.into_iter().enumerate() {
        let id = id as u32;
        let info = match known.iter().find(|w| w.source == name) {
            Some(weapon) => replay::WeaponInfo {
                id,
                source: weapon.source.clone(),
                shape: weapon.shape.clone(),
                name: weapon.hud_name.clone(),
                class: weapon_class(weapon),
            },
            None => replay::WeaponInfo {
                id,
                source: name.clone(),
                shape,
                name: name.clone(),
                class: if aimed {
                    replay::WeaponClass::Missile
                } else {
                    replay::WeaponClass::Other
                },
            },
        };
        ids.insert(name, id);
        infos.push(info);
    }
    (ids, infos)
}

/// The weapon class the single-player recorder gives a weapon.
fn weapon_class(weapon: &tore_formats::weapons::Weapon) -> replay::WeaponClass {
    if tore_sim::combat::live::is_gun(weapon) {
        return replay::WeaponClass::Gun;
    }
    match ShotKind::of(weapon) {
        ShotKind::AirToAir | ShotKind::AirToGround => replay::WeaponClass::Missile,
        ShotKind::Bomb => replay::WeaponClass::Bomb,
        ShotKind::Gun => replay::WeaponClass::Gun,
        ShotKind::Other if weapon.flags & 1 != 0 => replay::WeaponClass::Missile,
        ShotKind::Other => replay::WeaponClass::Other,
    }
}

// ----- States as the replay stores them -----------------------------------

/// A replay aircraft from a drawn pose and its flight data; the mapping the
/// game's own recorder uses (`replay/convert.rs` in the app, which a test
/// there compares this with).
pub fn pose_state(
    pose: &tore_world::snapshot::AircraftPose,
    data: &Flight,
) -> replay::AircraftState {
    replay::AircraftState {
        id: pose.id,
        position: pose.position,
        attitude: pose.attitude,
        velocity: pose.velocity,
        airspeed: data.airspeed,
        g: data.g,
        devices: pose.devices.unwrap_or([0.; DEVICES]),
        heat: heat(pose),
        flags: replay::AircraftFlags {
            engine_on: pose.engine.lit,
            afterburner: pose.engine.afterburner,
            airborne: pose.airborne,
            on_ground: data.on_ground,
            crashed: pose.crashed,
            wreck_gone: data.wreck_gone,
            alive: data.alive,
            ejected: data.ejected,
            animated: pose.devices.is_some(),
            flame: pose.engine.flame,
        },
        wreck_phase: wreck_code(pose.wreck),
        fuel_lb: data.fuel_lb,
        controls: data.controls,
        auxiliary_rates: pose.engine.rates,
        hp: pose.damage.hp,
        max_hp: pose.damage.initial_hp,
        sections: pose.damage.sections,
        structural_section: pose.damage.structural.map(|section| section as u8),
    }
}

/// The flight data a pose does not carry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flight {
    pub airspeed: f64,
    pub g: f64,
    pub fuel_lb: f64,
    pub controls: [f64; 4],
    pub on_ground: bool,
    pub alive: bool,
    pub ejected: bool,
    pub wreck_gone: bool,
}

fn heat(pose: &tore_world::snapshot::AircraftPose) -> f64 {
    if !pose.engine.lit {
        0.
    } else if pose.engine.afterburner {
        1.
    } else {
        pose.devices
            .map_or(0., |devices| devices[DEVICES - 1].clamp(0., 1.))
    }
}

fn wreck_code(phase: Option<wreck::Phase>) -> u8 {
    match phase {
        None => 0,
        Some(wreck::Phase::Falling) => 1,
        Some(wreck::Phase::Grounded) => 2,
        Some(wreck::Phase::Exploded) => 3,
    }
}

fn escape_code(phase: ejection::Phase) -> u8 {
    match phase {
        ejection::Phase::Seat => 0,
        ejection::Phase::Freefall => 1,
        ejection::Phase::Inflating => 2,
        ejection::Phase::Parachute => 3,
        ejection::Phase::Landed => 4,
        ejection::Phase::Impact => 5,
    }
}

fn own_state(sample: &OwnSample, id: u32) -> replay::AircraftState {
    let mut pose = sample.pose.clone();
    pose.id = id;
    pose_state(
        &pose,
        &Flight {
            airspeed: sample.airspeed,
            g: sample.g,
            fuel_lb: sample.fuel_lb,
            controls: sample.controls,
            on_ground: sample.on_ground,
            alive: sample.alive,
            ejected: sample.ejected,
            wreck_gone: sample.wreck_gone,
        },
    )
}

/// Another aircraft at a tick, from the pose the client would draw there and
/// what the host does not send as the format's nominal values (1 G, no fuel,
/// idle controls; the speed device or the ground speed for airspeed).
fn aircraft_state(
    id: u32,
    mut pose: tore_world::snapshot::AircraftPose,
    ejected: bool,
) -> replay::AircraftState {
    pose.id = id;
    let [x, y, z] = pose.velocity;
    let speed = (x * x + y * y + z * z).sqrt();
    let device = pose.devices.map_or(0., |d| d[DEVICES - 2]);
    pose_state(
        &pose,
        &Flight {
            airspeed: if device > 0. { device } else { speed },
            g: 1.,
            fuel_lb: 0.,
            controls: [0.; 4],
            on_ground: !pose.airborne && !pose.crashed,
            alive: pose.damage.hp > 0 && !pose.crashed && !ejected,
            ejected,
            wreck_gone: matches!(pose.wreck, Some(wreck::Phase::Exploded)),
        },
    )
}

/// A projectile at a tick: its state, where the curve puts it and the tick
/// of its first state.
struct Projectile {
    state: crate::wire::entity::ProjectileState,
    position: [f64; 3],
    velocity: [f64; 3],
    direction: [f64; 2],
    first: u32,
}

fn projectile_state(
    id: u32,
    at: &Projectile,
    weapon_ids: &BTreeMap<String, u32>,
    seen: &FlightSeen,
    launches: &BTreeMap<u32, u32>,
    tick: u64,
    ids: Ids,
) -> replay::ProjectileState {
    let p = &at.state;
    let [azimuth, elevation] = at.direction;
    let name = seen.names.name(p.weapon).unwrap_or_default();
    let started = launches
        .get(&id)
        .map_or(u64::from(at.first), |launch| u64::from(*launch));
    replay::ProjectileState {
        id,
        owner: ids.map(p.owner),
        weapon: weapon_ids.get(name).copied().unwrap_or(0),
        target: p.target.map(|t| ids.map(t)),
        position: at.position,
        previous: std::array::from_fn(|i| at.position[i] - at.velocity[i] * DT),
        direction: [
            elevation.cos() * azimuth.sin(),
            elevation.sin(),
            elevation.cos() * azimuth.cos(),
        ],
        speed: {
            let [x, y, z] = at.velocity;
            (x * x + y * y + z * z).sqrt()
        },
        tracer: false,
        incoming: p.aimed_at_player,
        age: tick.saturating_sub(started).min(u64::from(u32::MAX)) as u32,
        seeker: None,
    }
}

// ----- The diagnostics ------------------------------------------------------

/// A diagnostics line as an event: `net.stats` for a stats line with its
/// figures as fields, `net.event` for any other with its kind and text.
fn net_event(line: &str) -> replay::Event {
    use super::diagnostics::STATS_FIELDS;
    let mut parts = line.split('\t');
    let seconds = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    if kind == "stats" {
        let mut event = replay::Event::new(replay::vocab::kind::NET_STATS)
            .with("client_seconds", seconds.parse::<f64>().unwrap_or(0.));
        for (name, value) in STATS_FIELDS.iter().zip(parts) {
            if let Ok(number) = value.parse::<f64>() {
                event = event.with(name, number);
            }
        }
        event
    } else {
        replay::Event::new(replay::vocab::kind::NET_EVENT)
            .with("client_seconds", seconds.parse::<f64>().unwrap_or(0.))
            .with("kind", kind)
            .with_text(parts.collect::<Vec<_>>().join(" "))
    }
}

/// The flight's network figures for the footer: means over the stats lines.
fn net_summary<'a>(
    lines: impl Iterator<Item = &'a str>,
    flight: &FlightInfo,
) -> Vec<(String, String)> {
    use super::diagnostics::STATS_FIELDS;
    let column = |name: &str| {
        STATS_FIELDS
            .iter()
            .position(|f| *f == name)
            .expect("a field")
    };
    let (rtt, loss, snap, corr, mism) = (
        column("round_trip_ms"),
        column("loss_percent"),
        column("snapshot_loss_percent"),
        column("corrections"),
        column("mismatches"),
    );
    let (mut n, mut rtt_sum, mut rtt_max) = (0u64, 0., 0f64);
    let (mut loss_sum, mut loss_n) = (0., 0u64);
    let mut snap_sum = 0.;
    let (mut corrections, mut mismatches) = (0u64, 0u64);
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.get(1) != Some(&"stats") {
            continue;
        }
        let get = |i: usize| fields.get(2 + i).and_then(|v| v.parse::<f64>().ok());
        n += 1;
        if let Some(v) = get(rtt) {
            rtt_sum += v;
            rtt_max = rtt_max.max(v);
        }
        if let Some(v) = get(loss) {
            loss_sum += v;
            loss_n += 1;
        }
        snap_sum += get(snap).unwrap_or(0.);
        corrections += get(corr).unwrap_or(0.) as u64;
        mismatches += get(mism).unwrap_or(0.) as u64;
    }
    let mean = |sum: f64, n: u64| if n == 0 { 0. } else { sum / n as f64 };
    vec![
        (
            "net.seconds".into(),
            format!(
                "{:.1}",
                (flight.last_tick - flight.first_tick) as f64 / 120.
            ),
        ),
        ("net.rtt_ms_mean".into(), format!("{:.1}", mean(rtt_sum, n))),
        ("net.rtt_ms_max".into(), format!("{rtt_max:.1}")),
        (
            "net.loss_percent_mean".into(),
            format!("{:.2}", mean(loss_sum, loss_n)),
        ),
        (
            "net.snapshot_loss_percent_mean".into(),
            format!("{:.2}", mean(snap_sum, n)),
        ),
        ("net.corrections".into(), corrections.to_string()),
        ("net.mismatches".into(), mismatches.to_string()),
    ]
}

/// A position in feet from the wire's steps.
pub(crate) fn feet(position: &[i64; 3]) -> [f64; 3] {
    position.map(|q| q as f64 * POSITION_STEP)
}

/// The yaw, pitch and bank of a wire attitude, in radians.
pub(crate) fn angles(attitude: [u16; 3]) -> [f64; 3] {
    attitude.map(radians)
}

/// A unit basis for an attitude, as the countermeasure event keeps it.
pub(crate) fn basis(attitude: [f64; 3]) -> Basis {
    Basis::new(attitude[0], attitude[1], attitude[2])
}
