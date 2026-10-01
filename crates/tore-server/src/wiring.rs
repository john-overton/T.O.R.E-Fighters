//! Where the server meets the host session: the one function that builds the
//! real [`Host`], `tore_session::Host`, and the adapter that lets the run loop
//! drive it. The host reads no clock and opens no socket; this adapter hands it
//! the time and the bound sockets.

use crate::{
    app::is_release,
    config::{AfterEnd, OpenPlanes, StartMode},
    host::{Event, Host, HostSetup, PlayerFigures, Status, Time},
};
use std::{collections::VecDeque, time::Duration};
use tore_net::{Entropy, ServerSocket};
use tore_session::{BuildId, HostConfig, HostLog, LeaveReason, Phase};

/// Builds the host session for a prepared mission.
pub fn start_host(setup: HostSetup) -> Result<Box<dyn Host>, String> {
    let HostSetup {
        config: c,
        spec,
        resources,
        socket,
        version,
        commit,
    } = setup;
    let mut config = HostConfig::new(BuildId {
        version,
        commit,
        release: is_release(),
    });
    config.name = c.name;
    config.password = c.password;
    config.max_players = usize::from(c.max_players);
    config.open_planes = match c.open_planes {
        OpenPlanes::Friendly => tore_session::OpenPlanes::Friendly,
        OpenPlanes::All => tore_session::OpenPlanes::All,
        OpenPlanes::List(planes) => tore_session::OpenPlanes::List(planes),
    };
    config.snapshot_rate = c.snapshot_rate;
    config.start = match c.start {
        StartMode::FirstPlayer => tore_session::StartMode::FirstPlayer,
        StartMode::Now => tore_session::StartMode::Now,
    };
    config.time_limit = (c.time_limit_minutes > 0)
        .then(|| Duration::from_secs(u64::from(c.time_limit_minutes) * 60));
    config.empty_timeout = Duration::from_secs(u64::from(c.empty_timeout_seconds));
    config.after_end = match c.after_end {
        AfterEnd::Restart => tore_session::AfterEnd::Restart,
        AfterEnd::Quit => tore_session::AfterEnd::Quit,
    };
    config.restart_delay = Duration::from_secs(u64::from(c.restart_delay_seconds));
    config.entropy = Entropy::System;
    // The program has already refused the switch; the host refuses it too.
    config.retail_stall_speeds = false;
    let host = tore_session::Host::new(spec, resources, config)
        .map_err(|error| format!("The host session cannot start: {error}"))?;
    Ok(Box::new(SessionHost {
        host,
        socket,
        events: VecDeque::new(),
    }))
}

struct SessionHost {
    host: tore_session::Host,
    socket: ServerSocket,
    events: VecDeque<Event>,
}

fn leave_text(reason: LeaveReason) -> String {
    match reason {
        LeaveReason::Left => "left".into(),
        LeaveReason::Silent => "no packet for 5 seconds".into(),
        LeaveReason::Kicked => "kicked".into(),
        LeaveReason::MissionEnded => "the mission ended".into(),
        LeaveReason::HostLeft => "the host left the game".into(),
        LeaveReason::Replaced => "replaced by a new connection from the same address".into(),
        LeaveReason::Disconnected(why) => format!("disconnected ({why:?})"),
    }
}

fn end_text(reason: tore_session::wire::messages::EndReason) -> &'static str {
    use tore_session::wire::messages::EndReason;
    match reason {
        EndReason::EveryoneLeft => "everyone left",
        EndReason::TimeLimit => "the time limit",
        EndReason::ServerStopping => "the server is stopping",
        EndReason::EndedByServer => "ended from the console",
        EndReason::HostLeft => "the host left the game",
    }
}

impl SessionHost {
    fn drain_log(&mut self) {
        while let Some(entry) = self.host.poll_log() {
            self.events.push_back(match entry {
                HostLog::Connected {
                    address, callsign, ..
                } => Event::Joined {
                    address: address.to_string(),
                    callsign,
                },
                HostLog::Refused {
                    address,
                    callsign,
                    reason,
                    ..
                } => Event::Refused {
                    address: address.to_string(),
                    callsign: Some(callsign).filter(|c| !c.is_empty()),
                    reason,
                },
                HostLog::ContentRefused {
                    callsign, names, ..
                } => Event::Refused {
                    address: String::new(),
                    callsign: Some(callsign),
                    reason: format!("content mismatch: {}", names.join(", ")),
                },
                HostLog::SeatRefused {
                    callsign, reason, ..
                } => Event::Note(format!("{callsign} was refused a plane: {reason}")),
                HostLog::Seated {
                    seat,
                    callsign,
                    plane,
                    ..
                } => Event::Seated {
                    seat,
                    callsign,
                    plane,
                },
                HostLog::Left {
                    seat,
                    callsign,
                    plane,
                    reason,
                    ..
                } => Event::Left {
                    seat,
                    callsign,
                    plane,
                    reason: leave_text(reason),
                },
                HostLog::MissionStarted { .. } => Event::MissionStarted,
                HostLog::MissionEnded { reason, .. } => Event::MissionEnded {
                    reason: end_text(reason).into(),
                },
                HostLog::MissionRestarted { .. } => Event::MissionRestarted,
                HostLog::Overloaded { ticks_behind, .. } => Event::Note(format!(
                    "overloaded: the tick loop is {ticks_behind} ticks behind real time"
                )),
                HostLog::Fault { text, .. } => Event::Note(format!("fault: {text}")),
                // The program logs "Stopped" itself.
                HostLog::Stopped { .. } => continue,
                HostLog::Lobby {
                    callsign, event, ..
                } => Event::Note(format!("{callsign} {event}")),
            });
        }
    }
}

impl Host for SessionHost {
    fn poll(&mut self, now: Time) {
        // A socket error is not fatal to a server: note it and go on.
        if let Err(error) = self.host.receive_from(now, &mut self.socket) {
            self.events
                .push_back(Event::Note(format!("receive failed: {error}")));
        }
        self.host.update(now);
        if let Err(error) = self.host.transmit(&mut self.socket) {
            self.events
                .push_back(Event::Note(format!("send failed: {error}")));
        }
    }

    fn next_wake(&self, now: Time) -> Duration {
        self.host.next_wake(now)
    }

    fn take_events(&mut self) -> Vec<Event> {
        self.drain_log();
        self.events.drain(..).collect()
    }

    fn status(&mut self, now: Time) -> Status {
        let s = self.host.status(now);
        Status {
            tick: s.tick,
            players: s.players,
            capacity: s.capacity,
            aircraft: s.aircraft,
            load_percent: s.load * 100.0,
            tick_ms: s.tick_cost_mean.as_secs_f64() * 1000.0,
            up_bytes_per_second: s.bytes_up_per_second as f64,
            down_bytes_per_second: s.bytes_down_per_second as f64,
        }
    }

    fn players(&self) -> Vec<PlayerFigures> {
        self.host
            .players()
            .into_iter()
            .map(|p| PlayerFigures {
                seat: p.seat,
                callsign: p.callsign,
                address: p.address.to_string(),
                plane: p.plane,
                round_trip_ms: p.round_trip.as_secs_f64() * 1000.0,
                loss_percent: p.loss.map(|loss| loss * 100.0),
                arrival_spread_ms: p.spread.as_secs_f64() * 1000.0,
                input_margin_ticks: p.input_margin_ticks.map(f64::from),
                inputs_repeated: p.inputs_repeated,
                bytes_up_per_second: p.bytes_up_per_second as f64,
                bytes_down_per_second: p.bytes_down_per_second as f64,
            })
            .collect()
    }

    fn kick(&mut self, seat: u8) -> Result<String, String> {
        let callsign = self
            .players()
            .into_iter()
            .find(|p| p.seat == Some(seat))
            .map(|p| p.callsign)
            .unwrap_or_default();
        self.host
            .kick(seat)
            .map(|()| callsign)
            .map_err(|error| error.to_string())
    }

    fn end_mission(&mut self) {
        self.host.end();
    }

    fn restart_mission(&mut self) {
        self.host.restart();
    }

    fn stop(&mut self, _now: Time) {
        self.host.stop();
        // Sends the disconnects the stop queued.
        if let Err(error) = self.host.transmit(&mut self.socket) {
            self.events
                .push_back(Event::Note(format!("send failed: {error}")));
        }
    }

    fn finished(&self) -> bool {
        self.host.phase() == Phase::Stopped
    }
}
