//! The seam between the server program and the host session.
//!
//! Everything the program does around the session (the run loop, the console,
//! the status line, the log) talks to a [`Host`]. The real one is built by
//! `wiring::start_host`, which is the only place that names the session crate.
//! Tests drive the run loop with a scripted host instead.
//!
//! The host owns the mission's lifecycle (waiting for the first player, the
//! time limit, the empty timeout, the end, the restart delay and the rebuild
//! from the spec); the program only tells it the time, forwards console
//! commands and reports what it says. The host reads no clock and opens no
//! socket: the program gives it the time and the sockets.

use crate::config::Config;
use std::{sync::Arc, time::Duration};
use tore_import::Resources;
use tore_net::ServerSocket;
use tore_world::mission::MissionSpec;

/// Time since the server's clock started. The same kind of value as
/// `tore_net::datagram::RealClock::now`.
pub type Time = Duration;

/// What a host is built from.
pub struct HostSetup {
    /// The settings, with `--port` and `--mission` applied.
    pub config: Config,
    /// The mission, parsed from the mission file. A restart builds from it again.
    pub spec: MissionSpec,
    /// The import, which the host builds the mission from.
    pub resources: Arc<Resources>,
    /// The bound sockets, which the host reads and writes.
    pub socket: ServerSocket,
    /// The build's version string, for the join gate.
    pub version: String,
    /// The build's commit, for the join gate.
    pub commit: String,
    /// The server's anonymous install id when `telemetry` is on, which the
    /// listing carries and its Reports name; `None` when it is off.
    pub install_id: Option<u64>,
}

/// Something that happened that the log records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A connection was accepted.
    Joined { address: String, callsign: String },
    /// A connection was refused, with the reason its game was shown. The
    /// callsign is known once the challenge was answered.
    Refused {
        address: String,
        callsign: Option<String>,
        reason: String,
    },
    /// A player took a plane, at the start or later.
    Seated {
        seat: u8,
        callsign: String,
        plane: u32,
    },
    /// A player left, or was removed, with the reason.
    Left {
        seat: Option<u8>,
        callsign: String,
        plane: Option<u32>,
        reason: String,
    },
    /// The mission started flying.
    MissionStarted,
    /// The mission was built again after its end or a console `restart`.
    MissionRestarted,
    /// The mission ended, with why (time limit, empty, console, ...).
    MissionEnded { reason: String },
    /// A chat line the host routed: who sent it, to which receiver, what it
    /// said, and how many other players heard it.
    Chat {
        callsign: String,
        receiver: String,
        text: String,
        heard: usize,
    },
    /// Anything else worth a line, such as an overload note.
    Note(String),
}

impl Event {
    /// The log line.
    pub fn text(&self) -> String {
        match self {
            Self::Joined { address, callsign } => format!("{address} joined as {callsign}"),
            Self::Refused {
                address,
                callsign: Some(callsign),
                reason,
            } => format!("{address} ({callsign}) refused: {reason}"),
            Self::Refused {
                address,
                callsign: None,
                reason,
            } => format!("{address} refused: {reason}"),
            Self::Seated {
                seat,
                callsign,
                plane,
            } => format!("seat {seat} {callsign} took plane {plane}"),
            Self::Left {
                seat,
                callsign,
                plane,
                reason,
            } => {
                let seat = seat.map_or_else(String::new, |seat| format!("seat {seat} "));
                let plane = plane.map_or_else(String::new, |plane| format!(" (plane {plane})"));
                format!("{seat}{callsign}{plane} left: {reason}")
            }
            Self::MissionStarted => "mission started".into(),
            Self::MissionRestarted => "mission restarted".into(),
            Self::MissionEnded { reason } => format!("mission ended: {reason}"),
            Self::Chat {
                callsign,
                receiver,
                text,
                heard,
            } => {
                let heard = match heard {
                    0 => "no one heard".to_owned(),
                    1 => "1 heard".to_owned(),
                    n => format!("{n} heard"),
                };
                format!("chat: {callsign} to {receiver} ({heard}): {text}")
            }
            Self::Note(text) => text.clone(),
        }
    }
}

/// What the status line shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    /// The host tick; 0 while the mission waits.
    pub tick: u64,
    pub players: usize,
    /// The most players the mission seats now (the smaller of `max-players`
    /// and the planes open to humans).
    pub capacity: usize,
    pub aircraft: usize,
    /// The share of one core the tick takes, in percent.
    pub load_percent: f64,
    /// The recent cost of one tick, in milliseconds.
    pub tick_ms: f64,
    pub up_bytes_per_second: f64,
    pub down_bytes_per_second: f64,
}

/// One connected player's figures, the ones a player's game writes to its own
/// diagnostics log.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerFigures {
    /// The player's lobby id, which `kick-player` takes.
    pub id: u8,
    /// `None` while the connection has no seat yet.
    pub seat: Option<u8>,
    pub callsign: String,
    pub address: String,
    /// The plane flown, when seated.
    pub plane: Option<u32>,
    pub round_trip_ms: f64,
    /// `None` until enough packets have been judged.
    pub loss_percent: Option<f64>,
    /// How far snapshot arrivals spread, in milliseconds.
    pub arrival_spread_ms: f64,
    /// The smallest input margin lately, in ticks.
    /// `None` until the player's inputs have arrived.
    pub input_margin_ticks: Option<f64>,
    /// Inputs the host had to repeat.
    pub inputs_repeated: u64,
    /// Bytes a second sent to and received from this player.
    pub bytes_up_per_second: f64,
    pub bytes_down_per_second: f64,
}

/// The host session as the program drives it.
pub trait Host {
    /// Runs whatever is due at `now`: reads the sockets, steps the ticks that
    /// have come due, sends what the ticks produced.
    fn poll(&mut self, now: Time);

    /// How long from `now` until `poll` next has work that is not a packet:
    /// the next tick's deadline while flying, a restart delay's end, and so
    /// on. The program also polls at least every few milliseconds, to read
    /// the sockets.
    fn next_wake(&self, now: Time) -> Duration;

    /// What happened since the last call, oldest first.
    fn take_events(&mut self) -> Vec<Event>;

    /// The figures for a status line. The tick cost figures cover the time
    /// since the previous call.
    fn status(&mut self, now: Time) -> Status;

    /// Every connected player's figures, in seat order.
    fn players(&self) -> Vec<PlayerFigures>;

    /// Gives the seat's plane back to the AI and disconnects the player;
    /// returns the callsign, or why there is no such seat.
    fn kick(&mut self, seat: u8) -> Result<String, String>;

    /// Removes the player with lobby id `id`, in the lobby or flying, telling
    /// it `reason`; returns the callsign, or why there is no such player.
    fn kick_player(&mut self, id: u8, reason: &str) -> Result<String, String>;

    /// Ends the mission now, with debriefs.
    fn end_mission(&mut self);

    /// Ends the mission and starts it again at once.
    fn restart_mission(&mut self);

    /// Tells every player the server is stopping and sends what is queued.
    fn stop(&mut self, now: Time);

    /// True once the host has ended its last mission and will not start
    /// another (`after-end quit`).
    fn finished(&self) -> bool;

    /// Where the server's listing on the Internet Lobby stands, for the
    /// status line; `None` while it does not broadcast.
    fn listing(&self) -> Option<String> {
        None
    }

    /// The console's `broadcast on` and `broadcast off`: what to log, or why
    /// not.
    fn set_broadcast(&mut self, _on: bool, _now: Time) -> Result<String, String> {
        Err("this server cannot broadcast".into())
    }
}

#[cfg(test)]
pub mod scripted {
    //! A host the tests script: it counts ticks from a fake clock, says what
    //! it is told to say and records what the program asks of it.

    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    pub struct ScriptedHost {
        pub tick: u64,
        pub events: VecDeque<Event>,
        /// Events released once the clock reaches the time.
        pub scheduled: Vec<(Time, Event)>,
        pub players: Vec<PlayerFigures>,
        pub ended: u32,
        pub restarted: u32,
        pub stopped: Option<Time>,
        pub kicked: Vec<u8>,
        pub kicked_players: Vec<u8>,
        pub finish_at: Option<Time>,
        pub done: bool,
        pub polls: u32,
        pub flying: bool,
        /// What the console's `broadcast` set, if anything.
        pub broadcast: Option<bool>,
    }

    impl Host for ScriptedHost {
        fn poll(&mut self, now: Time) {
            self.polls += 1;
            if self.flying {
                self.tick = (now.as_nanos() * 120 / 1_000_000_000) as u64;
            }
            let due: Vec<_> = self
                .scheduled
                .iter()
                .filter(|(at, _)| *at <= now)
                .cloned()
                .collect();
            self.scheduled.retain(|(at, _)| *at > now);
            self.events.extend(due.into_iter().map(|(_, event)| event));
            if self.finish_at.is_some_and(|at| now >= at) {
                self.done = true;
            }
        }
        fn next_wake(&self, now: Time) -> Duration {
            Duration::from_nanos((self.tick + 1) * 1_000_000_000 / 120 + 1).saturating_sub(now)
        }
        fn take_events(&mut self) -> Vec<Event> {
            self.events.drain(..).collect()
        }
        fn status(&mut self, _now: Time) -> Status {
            Status {
                tick: self.tick,
                players: self.players.len(),
                capacity: 15,
                aircraft: 30,
                load_percent: 11.0,
                tick_ms: 1.1,
                up_bytes_per_second: 64_000.0,
                down_bytes_per_second: 7_000.0,
            }
        }
        fn players(&self) -> Vec<PlayerFigures> {
            self.players.clone()
        }
        fn kick(&mut self, seat: u8) -> Result<String, String> {
            match self.players.iter().position(|p| p.seat == Some(seat)) {
                Some(index) => {
                    self.kicked.push(seat);
                    Ok(self.players.remove(index).callsign)
                }
                None => Err(format!("no player in seat {seat}")),
            }
        }
        fn kick_player(&mut self, id: u8, _reason: &str) -> Result<String, String> {
            match self.players.iter().position(|p| p.id == id) {
                Some(index) => {
                    self.kicked_players.push(id);
                    Ok(self.players.remove(index).callsign)
                }
                None => Err(format!("no player has the lobby id {id}")),
            }
        }
        fn end_mission(&mut self) {
            self.ended += 1;
        }
        fn restart_mission(&mut self) {
            self.restarted += 1;
        }
        fn stop(&mut self, now: Time) {
            self.stopped = Some(now);
        }
        fn finished(&self) -> bool {
            self.done
        }
        fn listing(&self) -> Option<String> {
            (self.broadcast == Some(true)).then(|| "listed".to_owned())
        }
        fn set_broadcast(&mut self, on: bool, _now: Time) -> Result<String, String> {
            self.broadcast = Some(on);
            Ok(format!("broadcast {}", if on { "on" } else { "off" }))
        }
    }
}
