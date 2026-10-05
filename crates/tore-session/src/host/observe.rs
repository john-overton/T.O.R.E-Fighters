//! Observers on the host (stage F phase 2; docs/ARCHITECTURE.md, "The
//! observer view"; the bytes are docs/formats/net-protocol.md, "Observer
//! flights"): Observe and Observing, the observer flight's snapshots and the
//! delay ring. Built by slice F2-O1.
//!
//! A connection with no plane asks to watch the flying mission (message
//! 33) and names its camera's subject. Once its game is in the lobby the
//! host starts an **observer flight**: a new flight of the connection
//! (Observing, message 34, with the roster, the destroyed ground objects and
//! the tick the first snapshot shows), then ordinary snapshots with no own
//! plane, coded against the observer's own baselines with the camera's
//! relevance, and the mission-wide events. The stream stops at the
//! observer's Stop, when it takes a plane (before its Seated message) and at
//! the mission's end.
//!
//! **The picture** is the whole mission ([`from_world::observer_picture`]),
//! quantized once per tick it is needed and shared by every observer.
//!
//! **The delay.** In PvP the King's `observer-delay` holds the stream back:
//! the host keeps a ring of the quantized pictures (one each snapshot
//! interval, at the ticks whose number is a multiple of the ticks per
//! snapshot) and the mission-wide events with their ticks, for the delay,
//! and every observer's snapshots and events come from the ring at the
//! host's tick less the delay. Nothing newer leaves the host for an
//! observer. Messages about the mission at a tick that the slices send
//! observers (the scores, a revival's new plane) go through
//! [`Host::send_as_of`], which holds them the same way.

use super::sorting::{Timed, Tracker, Wide};
use super::{
    ConnectionId, FLIGHT_MESSAGE_BUDGET, Host, Life, SEATED_HOLD_TICKS, Stage, TICKS_PER_SECOND,
    queue,
};
use crate::settings::Mode;
use crate::wire::WireError;
use crate::wire::connection::HostConnection;
use crate::wire::entity::{Entity, EntityKind, EntityState};
use crate::wire::events::WireEvent;
use crate::wire::from_world::{self, NO_PLANE};
use crate::wire::messages::{Message, Observe, ObserverFlight, Observing, Subject};
use crate::wire::names::NameTable;
use crate::wire::priority::Relevance;
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tore_net::DisconnectReason;
use tore_world::seats::PlaneId;
use tore_world::snapshot::RenderSnapshot;
use tore_world::world::{Cue, TickOutput};

/// The host applies at most two camera changes a second from one
/// connection (the protocol's limit); a change sooner waits, and the newest
/// waiting one is applied when the interval has passed.
pub const CAMERA_INTERVAL: Duration = Duration::from_millis(500);

/// One connection's watch: asked for, and once its game is in the lobby,
/// streaming.
#[derive(Clone, Debug)]
pub(super) struct Watch {
    /// The camera's subject now.
    subject: Subject,
    /// A camera change waiting for [`CAMERA_INTERVAL`].
    pending: Option<Subject>,
    /// When the subject last changed.
    changed_at: Duration,
    /// The Observing message went out: the observer flight runs.
    started: bool,
    /// The stream comes from the delay ring.
    delayed: bool,
    /// The camera's point at the last snapshot: an aircraft subject's
    /// position, kept while the aircraft is gone.
    point: Option<[f64; 3]>,
    /// The tick the Observing message went out, while it waits to be
    /// acknowledged: the flight's 256-byte message budget starts after it.
    holding_since: Option<u64>,
    /// Delayed: the number of the next ring event this observer has not
    /// been given.
    next_event: u64,
    /// Delayed: messages about the mission at a tick, held until the
    /// stream shows that tick ([`Host::send_as_of`]).
    held: VecDeque<(u64, Message)>,
}

impl Watch {
    fn new(subject: Subject, now: Duration) -> Self {
        Self {
            subject,
            pending: None,
            changed_at: now,
            started: false,
            delayed: false,
            point: None,
            holding_since: None,
            next_event: 0,
            held: VecDeque::new(),
        }
    }

    /// The observer flight runs: the lobby state marks the player
    /// observing.
    pub(super) fn started(&self) -> bool {
        self.started
    }

    /// The camera's subject now.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn subject(&self) -> Subject {
        self.subject
    }

    /// A new camera, applied now or, within [`CAMERA_INTERVAL`] of the last
    /// change, when the interval has passed.
    fn camera(&mut self, subject: Subject, now: Duration) {
        if subject == self.subject {
            self.pending = None;
        } else if now.saturating_sub(self.changed_at) >= CAMERA_INTERVAL {
            self.subject = subject;
            self.changed_at = now;
            self.pending = None;
        } else {
            self.pending = Some(subject);
        }
    }

    /// Applies the waiting camera change once its interval has passed.
    fn apply_pending(&mut self, now: Duration) {
        if let Some(subject) = self.pending
            && now.saturating_sub(self.changed_at) >= CAMERA_INTERVAL
        {
            self.subject = subject;
            self.changed_at = now;
            self.pending = None;
        }
    }
}

/// The observers' quantized picture of one tick: every entity of the
/// mission, projectile names numbered in the stream's own table.
#[derive(Debug)]
pub(super) struct Frame {
    pub tick: u64,
    pub entities: Vec<Entity>,
}

/// One mission-wide event in the delay ring.
#[derive(Clone, Debug)]
struct RingEvent {
    /// Its number in the ring, counting from 0.
    seq: u64,
    /// The host tick it became known: it leaves the host once the stream
    /// reaches this tick.
    release: u64,
    timed: Timed,
}

/// The delay ring (docs/ARCHITECTURE.md, "The observer view"): the
/// observers' pictures and the mission-wide events of the last delay, and
/// the mission as it stood at the ring's tail.
#[derive(Debug)]
pub(super) struct Ring {
    /// The delay, ticks.
    delay: u64,
    /// The tick of the first frame the ring records.
    first: u64,
    /// A frame every snapshot interval, oldest first.
    frames: VecDeque<Arc<Frame>>,
    events: VecDeque<RingEvent>,
    next_seq: u64,
    /// The ground objects destroyed and the craters and crash-site fires as
    /// of the newest tick folded out of the ring: what an observer who
    /// starts now is told stands, besides the events still in the ring.
    destroyed: BTreeSet<u32>,
    marks: Vec<Timed>,
}

impl Ring {
    /// The frame of `tick`, if the ring holds one.
    fn frame(&self, tick: u64) -> Option<Arc<Frame>> {
        self.frames
            .binary_search_by_key(&tick, |frame| frame.tick)
            .ok()
            .map(|index| Arc::clone(&self.frames[index]))
    }

    /// Folds every event that became known by `tick` into the mission as
    /// it stood, and forgets the frames up to it: nobody needs them again.
    fn fold(&mut self, tick: u64) {
        while self.events.front().is_some_and(|e| e.release <= tick) {
            let event = self.events.pop_front().expect("an event");
            match &event.timed.event {
                Wide::Event(WireEvent::GroundDestroyed { object }) => {
                    self.destroyed.insert(*object);
                }
                Wide::Event(WireEvent::Mark { .. }) => self.marks.push(event.timed),
                _ => {}
            }
        }
        while self.frames.front().is_some_and(|f| f.tick <= tick) {
            self.frames.pop_front();
        }
    }

    /// Bytes the ring holds, near enough: its frames' entities and its
    /// events, for the slice's measurement.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn bytes(&self) -> usize {
        let frames: usize = self
            .frames
            .iter()
            .map(|f| {
                std::mem::size_of::<Frame>() + f.entities.capacity() * std::mem::size_of::<Entity>()
            })
            .sum();
        let events = self.events.len() * std::mem::size_of::<RingEvent>()
            + self
                .events
                .iter()
                .map(|e| match &e.timed.event {
                    Wide::Launch { weapon, .. } => weapon.capacity(),
                    Wide::Event(WireEvent::WingEjection { message, .. }) => message.capacity(),
                    _ => 0,
                })
                .sum::<usize>();
        frames + events
    }

    /// Frames held.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn frames(&self) -> usize {
        self.frames.len()
    }
}

/// What the host keeps for every observer of the mission flying: the
/// pictures' names, the last picture taken (for debris and pilots'
/// velocities), the newest frame and, with a delay, the ring. Started
/// afresh with each mission.
#[derive(Debug, Default)]
pub(super) struct Stream {
    names: NameTable,
    last_picture: Option<RenderSnapshot>,
    live: Option<Arc<Frame>>,
    ring: Option<Ring>,
}

impl Stream {
    /// The delay ring, while the mission flies with a delay.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ring(&self) -> Option<&Ring> {
        self.ring.as_ref()
    }
}

/// The mission-wide event of an AI pilot's ejection cue.
fn ejection(cue: &Cue, tick: u64) -> Option<Timed> {
    match cue {
        Cue::WingEjection {
            id,
            message,
            friendly,
        } => Some(Timed {
            tick,
            event: Wide::Event(WireEvent::WingEjection {
                aircraft: *id,
                message: message.clone(),
                friendly: *friendly,
            }),
        }),
        _ => None,
    }
}

/// The first tick after `after` whose number modulo `tps` is `phase`.
fn next_phase(after: u64, tps: u64, phase: u64) -> u64 {
    let next = after + 1;
    next + (phase + tps - next % tps) % tps
}

impl Host {
    /// The observers' delay of the mission, in ticks: the King's
    /// `observer-delay` in PvP, none in co-op (the setting is PvP's).
    pub(super) fn observer_delay_ticks(&self) -> u64 {
        if self.settings.mode() == Mode::Pvp {
            u64::from(self.settings.observer_delay_seconds()) * TICKS_PER_SECOND
        } else {
            0
        }
    }

    /// A connection starts or stops watching, or moves its camera (message
    /// 33).
    pub(super) fn observe_request(
        &mut self,
        connection: ConnectionId,
        observe: Observe,
    ) -> Result<(), String> {
        match observe {
            Observe::Stop => {
                self.stop_watch(connection, true);
                Ok(())
            }
            Observe::Watch(subject) => self.start_watch(connection, subject),
        }
    }

    /// Starts `connection` watching with the camera on `subject`, or moves
    /// its camera: refused unless the mission flies and the connection
    /// flies no plane. A player leaving its plane starts watching once it
    /// is back in the lobby; the observer flight starts at the first tick
    /// that finds it there. Slice F2-A's away player can start here too.
    pub(super) fn start_watch(
        &mut self,
        connection: ConnectionId,
        subject: Subject,
    ) -> Result<(), String> {
        if !matches!(self.life, Life::Flying) {
            return Err("The mission is not flying; watch once it flies.".into());
        }
        if let Subject::Aircraft(plane) = subject
            && self.world.roster.plane(PlaneId(plane)).is_none()
        {
            return Err(format!("There is no plane {plane}."));
        }
        let now = self.now;
        let Some(peer) = self.peers.get_mut(&connection) else {
            return Ok(());
        };
        match peer.stage {
            Stage::Taking { .. } | Stage::Seated => {
                return Err("Leave your aircraft before you watch.".into());
            }
            Stage::Closing { .. } => return Ok(()),
            Stage::Lobby | Stage::Leaving => {}
        }
        match &mut peer.watch {
            Some(watch) => watch.camera(subject, now),
            None => peer.watch = Some(Watch::new(subject, now)),
        }
        Ok(())
    }

    /// Ends `connection`'s watch: Observing's end goes to it when its
    /// flight had started and `tell`, and the messages held for the delay
    /// go out, since it watches no more.
    pub(super) fn stop_watch(&mut self, connection: ConnectionId, tell: bool) {
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        let Some(watch) = peer.watch.take() else {
            return;
        };
        if !watch.started || matches!(peer.stage, Stage::Closing { .. }) {
            return;
        }
        // The host's log says so (slice F2-1's lobby event).
        let callsign = peer.callsign.clone();
        self.lobby_log(callsign, super::LobbyEvent::Watching(false));
        if tell {
            self.send(connection, &Message::Observing(Box::new(Observing::Ended)));
        }
        for (_, message) in watch.held {
            self.send(connection, &message);
        }
        self.server
            .set_message_budget(connection, tore_net::MAX_DATAGRAM);
        self.lobby_dirty = true;
    }

    /// `connection` takes a plane: its watch ends before its Seated message.
    pub(super) fn observe_seated(&mut self, connection: ConnectionId) {
        self.stop_watch(connection, true);
    }

    /// The mission ends: every watch ends before the results and Mission
    /// ended, and the observers' pictures and ring are dropped.
    pub(super) fn observe_end(&mut self) {
        let ids: Vec<ConnectionId> = self
            .peers
            .iter()
            .filter(|(_, peer)| peer.watch.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.stop_watch(id, true);
        }
        self.stream = Stream::default();
    }

    /// Sends `message`, news of the mission at `tick`, to `connection`: now,
    /// or to an observer watching with a delay once its stream shows that
    /// tick ("Scores to an observer are delayed the same"). For the slices
    /// that send such news to observers (F2-S's scores, F2-V's new planes).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn send_as_of(&mut self, connection: ConnectionId, tick: u64, message: Message) {
        if let Some(watch) = self
            .peers
            .get_mut(&connection)
            .and_then(|peer| peer.watch.as_mut())
            && watch.started
            && watch.delayed
        {
            watch.held.push_back((tick, message));
            return;
        }
        self.send(connection, &message);
    }

    /// After the seated players' snapshots: the ring's frame and events,
    /// the waiting camera changes, the observers' events and snapshots, and
    /// the watches that start.
    pub(super) fn observe_tick(
        &mut self,
        tick: u64,
        now: Duration,
        out: &TickOutput,
        wide: &[Timed],
    ) {
        let tps = u64::from(self.config.ticks_per_snapshot());
        let delay = self.observer_delay_ticks();
        if self.stream.ring.as_ref().is_some_and(|r| r.delay != delay) {
            self.stream.ring = None;
        }
        let watching = self.peers.values().any(|peer| peer.watch.is_some());
        if delay == 0 && !watching {
            return;
        }
        // The tick's mission-wide events, the AI's ejections with them.
        let mut events: Vec<Timed> = wide.to_vec();
        events.extend(out.cues.iter().filter_map(|cue| ejection(cue, tick)));
        if delay > 0 {
            self.record(tick, tps, delay, events.clone());
        }

        let mut failed = Vec::new();
        let ids: Vec<ConnectionId> = self
            .peers
            .iter()
            .filter(|(_, peer)| peer.watch.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in &ids {
            let peer = self.peers.get_mut(id).expect("a watching peer");
            if matches!(peer.stage, Stage::Closing { .. }) {
                peer.watch = None;
                continue;
            }
            let watch = peer.watch.as_mut().expect("a watch");
            watch.apply_pending(now);
            // Without a delay the tick's events are queued at once, as a
            // seated player's are.
            if delay == 0 && watch.started && peer.stage == Stage::Lobby {
                for timed in &events {
                    if !queue(peer, timed.tick, &timed.event) {
                        failed.push(*id);
                        break;
                    }
                }
            }
        }

        // The snapshots due now: with a delay every observer's at the
        // ticks the ring records; without, each at its lobby id's phase.
        let due: Vec<ConnectionId> = ids
            .iter()
            .copied()
            .filter(|id| {
                self.peers.get(id).is_some_and(|peer| {
                    peer.stage == Stage::Lobby
                        && peer.watch.as_ref().is_some_and(|w| w.started)
                        && if delay > 0 {
                            tick.is_multiple_of(tps)
                        } else {
                            tick % tps == crate::wire::snapshot_phase(peer.lobby.id, tps as u32)
                        }
                })
            })
            .collect();
        let frame = if due.is_empty() {
            None
        } else if delay > 0 {
            tick.checked_sub(delay)
                .and_then(|shown| self.stream.ring.as_ref()?.frame(shown))
        } else {
            self.live_frame(tick)
        };
        if let Some(frame) = frame {
            for id in due {
                if failed.contains(&id) {
                    continue;
                }
                if self.watch_snapshot(id, tick, now, &frame).is_err() {
                    failed.push(id);
                }
            }
        }
        if delay > 0
            && tick.is_multiple_of(tps)
            && let (Some(shown), Some(ring)) = (tick.checked_sub(delay), self.stream.ring.as_mut())
        {
            ring.fold(shown);
        }

        // The watches whose games are in the lobby now start.
        for id in ids {
            let starts = self.peers.get(&id).is_some_and(|peer| {
                peer.stage == Stage::Lobby && peer.watch.as_ref().is_some_and(|w| !w.started)
            });
            if starts {
                self.begin_watch(id, tick, tps, delay);
            }
        }
        for id in failed {
            self.server.disconnect(id, DisconnectReason::ProtocolError);
        }
    }

    /// The ring's share of a tick: made at the mission's first tick with a
    /// delay, then the tick's events and, each snapshot interval, a frame.
    fn record(&mut self, tick: u64, tps: u64, delay: u64, events: Vec<Timed>) {
        if self.stream.ring.is_none() {
            // The mission as it stood before this tick, whose events go in
            // the ring.
            let new_objects: BTreeSet<u32> = events
                .iter()
                .filter_map(|t| match t.event {
                    Wide::Event(WireEvent::GroundDestroyed { object }) => Some(object),
                    _ => None,
                })
                .collect();
            let new_marks: Vec<&Wide> = events
                .iter()
                .map(|t| &t.event)
                .filter(|e| matches!(e, Wide::Event(WireEvent::Mark { .. })))
                .collect();
            let marks = Tracker::standing(&self.world, tick)
                .into_iter()
                .filter(|t| {
                    matches!(t.event, Wide::Event(WireEvent::Mark { .. }))
                        && !new_marks.contains(&&t.event)
                })
                .collect();
            self.stream.ring = Some(Ring {
                delay,
                first: tick.div_ceil(tps) * tps,
                frames: VecDeque::new(),
                events: VecDeque::new(),
                next_seq: 0,
                destroyed: self
                    .tracker
                    .destroyed()
                    .difference(&new_objects)
                    .copied()
                    .collect(),
                marks,
            });
        }
        let frame = if tick.is_multiple_of(tps) {
            self.build_frame(tick)
        } else {
            None
        };
        let ring = self.stream.ring.as_mut().expect("the ring");
        for timed in events {
            ring.events.push_back(RingEvent {
                seq: ring.next_seq,
                release: tick,
                timed,
            });
            ring.next_seq += 1;
        }
        if let Some(frame) = frame {
            ring.frames.push_back(frame);
        }
    }

    /// The observers' frame of `tick`, taken once however many observers
    /// it serves.
    fn live_frame(&mut self, tick: u64) -> Option<Arc<Frame>> {
        if let Some(frame) = &self.stream.live
            && frame.tick == tick
        {
            return Some(Arc::clone(frame));
        }
        let frame = self.build_frame(tick)?;
        self.stream.live = Some(Arc::clone(&frame));
        Some(frame)
    }

    /// The whole mission's picture after `tick`'s step, quantized.
    fn build_frame(&mut self, tick: u64) -> Option<Arc<Frame>> {
        let picture = from_world::observer_picture(&self.world);
        let entities = from_world::entities(
            &picture,
            self.stream.last_picture.as_ref(),
            NO_PLANE,
            &mut self.stream.names,
        )
        .ok()?;
        self.stream.last_picture = Some(picture);
        Some(Arc::new(Frame { tick, entities }))
    }

    /// The observer flight of `connection` starts at `tick`: a new flight,
    /// the mission as it stands at the first snapshot's tick (the delay's
    /// tail with a delay), and the Observing message.
    fn begin_watch(&mut self, connection: ConnectionId, tick: u64, tps: u64, delay: u64) {
        // The tick the first snapshot shows, the ground objects destroyed
        // by then, the events that describe the mission at that tick, and
        // the first ring event this observer gets.
        let (first, destroyed, standing, next_event) = if delay > 0 {
            let Some(ring) = self.stream.ring.as_ref() else {
                return;
            };
            let first_due = next_phase(tick, tps, 0).max(ring.first + delay);
            (
                first_due - delay,
                ring.destroyed.iter().copied().collect::<Vec<u32>>(),
                ring.marks.clone(),
                ring.events.front().map_or(ring.next_seq, |e| e.seq),
            )
        } else {
            let Some(peer) = self.peers.get(&connection) else {
                return;
            };
            let phase = crate::wire::snapshot_phase(peer.lobby.id, tps as u32);
            (
                next_phase(tick, tps, phase),
                self.tracker.destroyed().iter().copied().collect(),
                Tracker::standing(&self.world, tick),
                0,
            )
        };
        let roster = self.roster();
        let Some(peer) = self.peers.get_mut(&connection) else {
            return;
        };
        peer.flight = peer.flight.wrapping_add(1);
        peer.wire = HostConnection::for_flight(tps as u32, peer.flight);
        peer.picture = None;
        let flight = peer.flight;
        let mut behind = false;
        for Timed { tick, event } in &standing {
            behind |= !queue(peer, *tick, event);
        }
        let Some(watch) = peer.watch.as_mut() else {
            return;
        };
        watch.started = true;
        watch.delayed = delay > 0;
        watch.holding_since = Some(tick);
        watch.next_event = next_event;
        watch.point = None;
        let observing = Message::Observing(Box::new(Observing::Started(ObserverFlight {
            flight,
            delay_seconds: (delay / TICKS_PER_SECOND).min(255) as u8,
            tick: first as u32,
            roster,
            destroyed,
        })));
        self.send(connection, &observing);
        self.lobby_dirty = true;
        if let Some(peer) = self.peers.get(&connection) {
            let callsign = peer.callsign.clone();
            self.lobby_log(callsign, super::LobbyEvent::Watching(true));
        }
        if behind {
            self.server
                .disconnect(connection, DisconnectReason::ProtocolError);
        }
    }

    /// One observer's snapshot of `frame` at the host's `tick`: with a delay
    /// first the messages and events the stream has reached; then the
    /// entities by the camera's relevance, the new names and the packet.
    fn watch_snapshot(
        &mut self,
        connection: ConnectionId,
        tick: u64,
        now: Duration,
        frame: &Frame,
    ) -> Result<(), WireError> {
        let Host {
            peers,
            stream,
            server,
            ..
        } = self;
        let Some(peer) = peers.get_mut(&connection) else {
            return Ok(());
        };
        let Some(watch) = peer.watch.as_mut() else {
            return Ok(());
        };
        // With a delay, what the stream has now reached.
        let mut release = Vec::new();
        let mut events = Vec::new();
        if watch.delayed {
            while watch.held.front().is_some_and(|(at, _)| *at <= frame.tick) {
                release.push(watch.held.pop_front().expect("a message").1);
            }
            if let Some(ring) = stream.ring.as_ref() {
                let from = watch.next_event;
                for event in ring
                    .events
                    .iter()
                    .filter(|e| e.seq >= from && e.release <= frame.tick)
                {
                    watch.next_event = event.seq + 1;
                    events.push(event.timed.clone());
                }
            }
        }
        let subject = watch.subject;
        let kept = watch.point;
        for timed in &events {
            if !queue(peer, timed.tick, &timed.event) {
                return Err(WireError::Invalid("events"));
            }
        }
        for message in release {
            let body = message.encode()?;
            if server
                .send_message(connection, message.kind(), &body)
                .is_err()
            {
                return Err(WireError::Invalid("message queue"));
            }
        }
        // The camera: an aircraft subject's place in the picture shown,
        // kept while it is gone, or the point asked for; none, and every
        // entity is near.
        let point = match subject {
            Subject::Aircraft(plane) => frame
                .entities
                .iter()
                .find(|e| e.id == plane && e.state.kind() == EntityKind::Aircraft)
                .map(|e| e.state.motion().position_ft())
                .or(kept),
            Subject::Point(p) => Some(p.map(f64::from)),
            Subject::None => None,
        };
        let mut entities = Vec::with_capacity(frame.entities.len());
        for entity in &frame.entities {
            let mut entity = *entity;
            if let EntityState::Projectile(p) = &mut entity.state {
                let name = |index| {
                    stream
                        .names
                        .name(index)
                        .ok_or(WireError::Invalid("name"))
                        .map(str::to_owned)
                };
                p.weapon = peer.wire.names.intern(&name(p.weapon)?)?;
                p.shape = match p.shape {
                    Some(shape) => Some(peer.wire.names.intern(&name(shape)?)?),
                    None => None,
                };
            }
            let at = entity.state.motion().position_ft();
            let relevance = Relevance {
                distance_ft: point.map_or(0., |c| {
                    (0..3).map(|i| (at[i] - c[i]).powi(2)).sum::<f64>().sqrt()
                }),
                viewed: matches!(subject, Subject::Aircraft(plane)
                    if plane == entity.id && entity.state.kind() == EntityKind::Aircraft),
                ..Relevance::NEAR
            };
            entities.push((entity, relevance));
        }
        // New names go first, in a message the packet can carry.
        if let Some(names) = peer.wire.names.take_new() {
            let message = Message::Names(names);
            let body = message.encode()?;
            if server
                .send_message(connection, message.kind(), &body)
                .is_err()
            {
                return Err(WireError::Invalid("message queue"));
            }
        }
        let messages = server
            .messages_due_bytes(connection, now)
            .min(FLIGHT_MESSAGE_BUDGET);
        let packet = peer
            .wire
            .observer_snapshot(frame.tick as u32, &entities, messages)?;
        match server.send_payload(now, connection, &packet.sections()) {
            Ok(sequence) => peer.wire.sent(sequence),
            Err(_) => peer.wire.discard(),
        }
        let watch = peer.watch.as_mut().expect("a watch");
        watch.point = point;
        // The Observing message first: the flight's budget for messages
        // starts once it is acknowledged, as a seated player's does.
        if let Some(since) = watch.holding_since {
            let acknowledged = server
                .stats(connection)
                .is_some_and(|stats| stats.messages_queued == 0);
            if acknowledged || tick.saturating_sub(since) >= SEATED_HOLD_TICKS {
                watch.holding_since = None;
                server.set_message_budget(connection, FLIGHT_MESSAGE_BUDGET);
            }
        }
        Ok(())
    }
}
