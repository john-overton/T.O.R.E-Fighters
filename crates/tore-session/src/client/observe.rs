//! The observer's side of the stream (stage F phase 2; docs/ARCHITECTURE.md,
//! "The observer view"; the bytes are docs/formats/net-protocol.md,
//! "Observer flights"). Built by slice F2-O1.
//!
//! A game with no plane watches the flying mission: [`Client::watch`] asks
//! the host with the camera's subject (an aircraft, a point or none), and
//! the host answers with the Observing message, which starts the connection's
//! **observer flight**. Its snapshots carry every entity of the mission and
//! no own plane; the client reads them as it reads a seated flight's, draws
//! them in the past with the same interpolation, and holds the mission-wide
//! events until the picture reaches their tick. [`Client::observer_frame`]
//! gives the picture: every aircraft as a target in its real plane id, the
//! ground objects, projectiles, debris, ejected pilots, effects and marks,
//! with an empty player pose. The observer screen (slice F2-O2) feeds these
//! frames to the replay viewer's live mode.
//!
//! The camera is sent again only when it changes: another subject, or a
//! point more than [`CAMERA_MOVE_FT`] from the one sent, and at most every
//! [`CAMERA_INTERVAL`] (the protocol's twice a second); a change in between
//! waits and goes with the next update.

use super::{Client, ClientEvent, ClientPhase};
use crate::wire::connection::FlightOrder;
use crate::wire::events::ReceivedEvent;
use crate::wire::from_world::NO_PLANE;
use crate::wire::messages::{Message, Observe, Observing, Subject};
use crate::wire::names::NameIndex;
use std::time::Duration;
use tore_world::snapshot::{AircraftPose, EffectPose, MarkPose, RenderSnapshot};

/// A point camera is sent again once it has moved this far from the point
/// last sent: 2 nautical miles.
pub const CAMERA_MOVE_FT: f64 = 2. * tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
/// The client sends the camera at most this often: twice a second.
pub const CAMERA_INTERVAL: Duration = Duration::from_millis(500);

/// The observer flight the host started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchFlight {
    /// The connection's flight number.
    pub flight: u8,
    /// How far behind the host's own time the stream runs, seconds.
    pub delay_seconds: u8,
    /// The host tick the first snapshot shows.
    pub first_tick: u32,
}

/// The game's watch: asked for, and once the host answers, its flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Watching {
    /// The camera's subject the game wants.
    pub subject: Subject,
    /// The observer flight, once the host has started it.
    pub flight: Option<WatchFlight>,
    /// The subject last sent, and when.
    sent: Option<Subject>,
    sent_at: Option<Duration>,
}

impl Watching {
    fn new(subject: Subject) -> Self {
        Self {
            subject,
            flight: None,
            sent: None,
            sent_at: None,
        }
    }

    /// The camera last sent to the host.
    pub fn sent(&self) -> Option<Subject> {
        self.sent
    }

    /// Whether the camera has changed enough from the one sent to send it
    /// again.
    fn changed(&self) -> bool {
        match (self.sent, self.subject) {
            (None, _) => true,
            (Some(Subject::Point(a)), Subject::Point(b)) => {
                let d: f64 = (0..3)
                    .map(|i| (f64::from(a[i]) - f64::from(b[i])).powi(2))
                    .sum::<f64>()
                    .sqrt();
                d > CAMERA_MOVE_FT
            }
            (Some(sent), now) => sent != now,
        }
    }
}

/// What an observer's game draws at one instant.
#[derive(Clone, Debug, PartialEq)]
pub struct ObserverFrame {
    /// The observer flight.
    pub flight: WatchFlight,
    /// The host tick the picture shows.
    pub render_tick: f64,
    /// Everything drawn at `render_tick`: every aircraft as a target in its
    /// real plane id, then the ground objects; the player pose is empty (no
    /// aircraft, id [`NO_PLANE`]).
    pub picture: RenderSnapshot,
    /// The mission-wide events released since the last frame, in tick
    /// order, once the picture reached them.
    pub events: Vec<ReceivedEvent>,
    /// The surface units not as the mission built them, at `render_tick`
    /// (protocol 22), as [`super::ClientFrame::surface_units`].
    pub surface_units: std::collections::BTreeMap<u32, crate::wire::events::SurfaceUnitView>,
}

impl Client {
    /// Watches the flying mission with the camera on `subject`, or moves the
    /// camera. The host answers with a [`ClientEvent::Observing`] (the
    /// observer flight starts) or a [`ClientEvent::Refused`]. While the game
    /// watches, the automatic ready takes no plane.
    pub fn watch(&mut self, subject: Subject) {
        if matches!(self.phase, ClientPhase::Connecting | ClientPhase::Closed) {
            return;
        }
        match &mut self.watching {
            Some(watching) => watching.subject = subject,
            None => self.watching = Some(Watching::new(subject)),
        }
        let now = self.now;
        self.send_camera(now);
    }

    /// Stops watching: the host ends the observer flight and the game is
    /// back in the lobby.
    pub fn stop_watching(&mut self) {
        if self.watching.take().is_some() {
            let now = self.now;
            self.request(now, Message::Observe(Observe::Stop));
        }
    }

    /// The game's watch, while it watches or has asked to.
    pub fn watching(&self) -> Option<&Watching> {
        self.watching.as_ref()
    }

    /// The camera again when it changed and the interval allows.
    fn send_camera(&mut self, now: Duration) {
        let Some(watching) = &self.watching else {
            return;
        };
        let due = watching
            .sent_at
            .is_none_or(|at| now.saturating_sub(at) >= CAMERA_INTERVAL);
        if !due || !watching.changed() {
            return;
        }
        let subject = watching.subject;
        if let Some(watching) = &mut self.watching {
            watching.sent = Some(subject);
            watching.sent_at = Some(now);
        }
        self.request(now, Message::Observe(Observe::Watch(subject)));
    }

    /// Each update: a camera change that waited for the interval goes now.
    pub(super) fn observe_update(&mut self, now: Duration) {
        self.send_camera(now);
    }

    /// The host refused the watch: the game is not watching.
    pub(super) fn watch_refused(&mut self) {
        self.watching = None;
    }

    /// The Observing message: the observer flight starts (a new flight of
    /// the connection, its roster and the destroyed ground objects) or ends.
    pub(super) fn observing_message(&mut self, observing: Observing) {
        match &observing {
            Observing::Started(started) => {
                let current = self.wire.as_ref().and_then(|wire| wire.flight);
                if FlightOrder::of(started.flight, current) == FlightOrder::Later {
                    self.begin_flight(started.flight);
                }
                for &object in &started.destroyed {
                    self.destroyed.insert(object, 0);
                }
                self.roster = Some(started.roster.clone());
                let flight = WatchFlight {
                    flight: started.flight,
                    delay_seconds: started.delay_seconds,
                    first_tick: started.tick,
                };
                // The host may start a watch the game did not ask for (the
                // AI flying an away player's aircraft): its camera is none.
                let watching = self.watching.get_or_insert_with(|| Watching {
                    sent: Some(Subject::None),
                    ..Watching::new(Subject::None)
                });
                watching.flight = Some(flight);
                self.log(
                    "observing",
                    &[
                        "start",
                        &started.flight.to_string(),
                        &started.delay_seconds.to_string(),
                        &started.tick.to_string(),
                    ],
                );
                self.event(ClientEvent::Roster);
            }
            Observing::Ended => {
                self.watching = None;
                self.log("observing", &["end"]);
            }
        }
        self.event(ClientEvent::Observing(Box::new(observing)));
    }

    /// What the observer's game draws at `now`: `None` until the observer
    /// flight has started and its first snapshot arrived. The events since
    /// the last frame come with it.
    pub fn observer_frame(&mut self, now: Duration) -> Option<ObserverFrame> {
        let flight = self.watching.as_ref()?.flight?;
        self.now = self.now.max(now);
        let now = self.now;
        let margin = self.interpolation_margin(now);
        self.render_clock.advance(now, margin);
        let lossy = self.downstream.loss(now) > super::HIGH_LOSS;
        self.interp.advance(now, lossy);
        let render = self.render_clock.render()?;
        while self
            .held
            .front()
            .is_some_and(|e| f64::from(e.tick) <= render)
        {
            let event = self.held.pop_front().expect("an event");
            self.released.push(event);
        }
        let wire = &self.wire;
        let name = |index: NameIndex| {
            wire.as_ref()
                .and_then(|w| w.names.name(index))
                .unwrap_or_default()
                .to_owned()
        };
        let drawn = self.interp.draw(render, NO_PLANE, &name);
        self.stats.frames += 1;
        self.stats.entity_frames += drawn.entities as u64;
        self.stats.extrapolated += drawn.extrapolated as u64;
        self.stats.far_frames += drawn.far as u64;
        self.stats.far_extrapolated += drawn.far_extrapolated as u64;

        let surface_units = self.surface_at(render);
        let mission = self.mission.as_ref()?;
        let mut targets = drawn.aircraft;
        targets.extend(self.ground_at(&mission.ground, &drawn.surface, &surface_units, render));
        self.effects
            .retain(|e| f64::from(e.tick) + f64::from(e.ticks) > render);
        let effects = self
            .effects
            .iter()
            .filter(|e| f64::from(e.tick) <= render)
            .map(|e| EffectPose {
                kind: e.kind,
                position: e.position,
                ticks: (f64::from(e.ticks) - (render - f64::from(e.tick))).max(0.) as u16,
                blast: e.blast,
            })
            .collect();
        let marks = self
            .marks
            .iter()
            .filter(|(tick, ..)| f64::from(*tick) <= render)
            .map(|(tick, kind, position)| MarkPose {
                kind: *kind,
                position: *position,
                age: (render - f64::from(*tick)).max(0.) as u64,
                strength: 1.,
            })
            .collect();
        let picture = RenderSnapshot {
            tick: render.max(0.) as u64,
            player: AircraftPose {
                id: NO_PLANE,
                ..AircraftPose::default()
            },
            targets,
            projectiles: drawn.projectiles,
            effects,
            marks,
            debris: drawn.debris,
            pilots: drawn.pilots,
            models: mission.models.clone(),
            surface: self.moving_at(drawn.surface, render),
        };
        Some(ObserverFrame {
            flight,
            render_tick: render,
            picture,
            events: std::mem::take(&mut self.released),
            surface_units,
        })
    }
}
