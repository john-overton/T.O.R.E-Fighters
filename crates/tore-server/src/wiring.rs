//! Where the server meets the host session: the one function that builds the
//! real [`Host`]. Slice D7a builds `tore_session::Host`; until it is merged
//! there is nothing to build, and the server says so instead of pretending to
//! run.
//!
//! The adapter for the real host is drafted below against the API the lead
//! sketched on 2026-09-30 (`tore_session::host`), behind `cfg(any())` so it is
//! kept but not compiled. To wire it in: add `tore-session` to this crate's
//! dependencies, delete the `cfg(any())` line, have `start_host` call
//! `session::start`, and fix whatever the merged API spells differently. The
//! guesses that most need checking are marked `CHECK`.

use crate::host::{Host, HostSetup};

/// Builds the host session for a prepared mission.
pub fn start_host(_setup: HostSetup) -> Result<Box<dyn Host>, String> {
    Err("This build has no host session yet (slice D7a). --check, --import and the configuration checks work; running a server does not.".into())
}

#[cfg(any())]
mod session {
    use super::*;
    use crate::{
        config::{AfterEnd, OpenPlanes, StartMode},
        host::{Event, PlayerFigures, Status, Time},
        socket::ServerSocket,
    };
    use std::{collections::VecDeque, time::Duration};
    use tore_net::Entropy;
    use tore_session::{BuildId, Host as Session, HostConfig, HostLog, LeaveReason, Phase};

    pub fn start(setup: HostSetup) -> Result<Box<dyn Host>, String> {
        let HostSetup {
            config: c,
            spec,
            resources,
            socket,
            version,
            commit,
        } = setup;
        // CHECK: the game must decide `release` the same way (a tagged release
        // build is stamped with TORE_BUILD_VERSION, as the app's version() reads it).
        let release = option_env!("TORE_BUILD_VERSION").is_some();
        let mut config = HostConfig::new(BuildId {
            version,
            commit,
            release,
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
        let host = Session::new(spec, resources, config).map_err(|error| error.to_string())?;
        Ok(Box::new(SessionHost {
            host,
            socket,
            events: VecDeque::new(),
        }))
    }

    struct SessionHost {
        host: Session,
        socket: ServerSocket,
        events: VecDeque<Event>,
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
            // CHECK: a timeout from `now`, as the sketch says ("for your socket
            // wait timeout"); if it is an absolute time, subtract `now`.
            self.host.next_wake(now)
        }

        fn take_events(&mut self) -> Vec<Event> {
            while let Some(entry) = self.host.poll_log() {
                self.events.push_back(match entry {
                    HostLog::Connected { address, callsign } => Event::Joined {
                        address: address.to_string(),
                        callsign,
                    },
                    HostLog::Refused {
                        address,
                        callsign,
                        reason,
                    } => Event::Refused {
                        address: address.to_string(),
                        callsign,
                        reason,
                    },
                    HostLog::ContentRefused { callsign, names } => Event::Refused {
                        address: String::new(),
                        callsign: Some(callsign),
                        reason: format!("content mismatch: {}", names.join(", ")),
                    },
                    HostLog::Seated {
                        seat,
                        callsign,
                        plane,
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
                    } => Event::Left {
                        seat,
                        callsign,
                        plane,
                        reason: match reason {
                            LeaveReason::Left => "left".into(),
                            LeaveReason::Silent => "no packet for 5 seconds".into(),
                            LeaveReason::Kicked => "kicked".into(),
                            LeaveReason::Disconnected(why) => format!("disconnected: {why}"),
                        },
                    },
                    HostLog::MissionStarted => Event::MissionStarted,
                    HostLog::MissionEnded { reason } => Event::MissionEnded {
                        reason: reason.to_string(),
                    },
                    HostLog::MissionRestarted => Event::MissionRestarted,
                    HostLog::Overloaded { ticks_behind } => Event::Note(format!(
                        "overloaded: the tick loop is {ticks_behind} ticks behind"
                    )),
                    HostLog::Stopped => Event::Note("host stopped".into()),
                });
            }
            self.events.drain(..).collect()
        }

        fn status(&mut self, now: Time) -> Status {
            let s = self.host.status(now);
            Status {
                tick: u64::from(s.tick),
                players: s.players,
                capacity: s.capacity,
                aircraft: s.aircraft,
                // CHECK: `load` is taken to be a fraction of one core.
                load_percent: s.load * 100.0,
                tick_ms: s.tick_cost_mean.as_secs_f64() * 1000.0,
                up_bytes_per_second: s.bytes_up_per_second,
                down_bytes_per_second: s.bytes_down_per_second,
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
                    loss_percent: p.loss.map(|loss| loss * 100.0), // CHECK: a fraction?
                    arrival_spread_ms: p.spread.as_secs_f64() * 1000.0,
                    input_margin_ticks: p.input_margin_ticks.map(f64::from),
                    inputs_repeated: p.inputs_repeated,
                    bytes_up_per_second: p.bytes_up_per_second,
                    bytes_down_per_second: p.bytes_down_per_second,
                })
                .collect()
        }

        fn kick(&mut self, seat: u8) -> Result<String, String> {
            let callsign = self
                .players()
                .into_iter()
                .find(|p| p.seat == Some(seat))
                .map(|p| p.callsign)
                .ok_or_else(|| format!("no player in seat {seat}"))?;
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

        fn stop(&mut self, now: Time) {
            self.host.stop();
            self.host.update(now);
            let _ = self.host.transmit(&mut self.socket);
        }

        fn finished(&self) -> bool {
            self.host.phase() == Phase::Stopped
        }
    }
}
