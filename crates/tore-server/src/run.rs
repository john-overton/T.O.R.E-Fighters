//! The real-time run loop: give the host the time, hand it the console's
//! commands, report what it says, print a status line every `status-interval`
//! seconds and log each player's figures once a minute.

use crate::{
    clock::{MAX_NAP, SPIN_MARGIN, Timer, wait_until},
    console::{Command, HELP},
    host::{Host, Time},
    log::Logger,
    report::{figures_line, players_table, status_line},
};
use std::{sync::mpsc::Receiver, time::Duration};

/// How often each player's figures are logged.
pub const FIGURES_INTERVAL: Duration = Duration::from_secs(60);
/// The host's tick, which the loop wakes for.
const TICK: Duration = Duration::from_nanos(1_000_000_000 / tore_session::host::TICKS_PER_SECOND);

/// Why the loop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The console's `quit`.
    Quit,
    /// The host finished its last mission (`after-end quit`).
    Finished,
}

/// What the loop needs besides the host.
pub struct Loop<'a> {
    pub timer: &'a mut dyn Timer,
    pub log: &'a mut Logger,
    pub console: &'a Receiver<Command>,
    /// Seconds between status lines; 0 for none.
    pub status_interval: Duration,
    pub spin_margin: Duration,
}

impl<'a> Loop<'a> {
    pub fn new(
        timer: &'a mut dyn Timer,
        log: &'a mut Logger,
        console: &'a Receiver<Command>,
        status_interval_seconds: u32,
    ) -> Self {
        Self {
            timer,
            log,
            console,
            status_interval: Duration::from_secs(u64::from(status_interval_seconds)),
            spin_margin: SPIN_MARGIN,
        }
    }

    /// Runs until the console says `quit` or the host has finished.
    pub fn run(&mut self, host: &mut dyn Host) -> Ended {
        if let Some(line) = self.timer.keep_on_time(TICK) {
            self.log_line(&line);
        }
        let mut next_status =
            (!self.status_interval.is_zero()).then(|| self.timer.now() + self.status_interval);
        let mut next_figures = self.timer.now() + FIGURES_INTERVAL;
        loop {
            let now = self.timer.now();
            host.poll(now);
            self.report_events(host);
            if let Some(ended) = self.console_commands(host, now) {
                return ended;
            }
            if host.finished() {
                self.report_events(host);
                self.log_line("The last mission has ended. Stopping.");
                return Ended::Finished;
            }
            if let Some(due) = next_status
                && now >= due
            {
                self.log.print(&status_text(host, now));
                next_status = Some(now + self.status_interval);
            }
            if now >= next_figures {
                for player in host.players() {
                    let line = figures_line(&player);
                    self.log_file_only(&line);
                }
                next_figures = now + FIGURES_INTERVAL;
            }
            let mut wake = (now + host.next_wake(now)).min(now + MAX_NAP);
            for due in [next_status, Some(next_figures)].into_iter().flatten() {
                wake = wake.min(due);
            }
            wait_until(self.timer, wake, self.spin_margin);
        }
    }

    fn log_line(&mut self, text: &str) {
        let unix = self.timer.unix_seconds();
        self.log.log(unix, text);
    }

    fn log_file_only(&mut self, text: &str) {
        let unix = self.timer.unix_seconds();
        self.log.log_file_only(unix, text);
    }

    fn report_events(&mut self, host: &mut dyn Host) {
        for event in host.take_events() {
            self.log_line(&event.text());
        }
    }

    fn console_commands(&mut self, host: &mut dyn Host, now: Time) -> Option<Ended> {
        while let Ok(command) = self.console.try_recv() {
            match command {
                Command::Status => self.log.print(&status_text(host, now)),
                Command::Players => {
                    for line in players_table(&host.players()) {
                        self.log.print(&line);
                    }
                }
                Command::KickPlayer(id, reason) => match host.kick_player(id, &reason) {
                    Ok(callsign) => self.log_line(&format!("kicked player {id} {callsign}")),
                    Err(reason) => self.log.print(&reason),
                },
                Command::Kick(seat) => match host.kick(seat) {
                    Ok(callsign) => self.log_line(&format!("kicked seat {seat} {callsign}")),
                    Err(reason) => self.log.print(&reason),
                },
                Command::End => {
                    self.log_line("console: ending the mission");
                    host.end_mission();
                }
                Command::Restart => {
                    self.log_line("console: restarting the mission");
                    host.restart_mission();
                }
                Command::Quit => {
                    self.log_line("console: quit. Telling the players the server is stopping");
                    host.stop(now);
                    self.report_events(host);
                    self.log_line("Stopped");
                    return Some(Ended::Quit);
                }
                Command::Broadcast(on) => match host.set_broadcast(on, now) {
                    Ok(line) => self.log_line(&format!("console: {line}")),
                    Err(reason) => self.log.print(&reason),
                },
                Command::Help => self.log.print(HELP),
                Command::Invalid(message) => self.log.print(&message),
            }
        }
        None
    }
}

/// The status line, with where the listing stands while the server
/// broadcasts: `..., broadcast: listed, seen at 203.0.113.5:26900`.
fn status_text(host: &mut dyn Host, now: Time) -> String {
    let line = status_line(&host.status(now));
    match host.listing() {
        Some(listing) => format!("{line}, broadcast: {listing}"),
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clock::fake::FakeTimer,
        host::{Event, PlayerFigures, scripted::ScriptedHost},
        log::tests::{Shared, scratch},
    };
    use std::{fs, sync::mpsc::channel};

    struct Rig {
        timer: FakeTimer,
        console: Shared,
        folder: std::path::PathBuf,
    }

    impl Rig {
        fn new(name: &str) -> Self {
            Self {
                timer: FakeTimer {
                    unix: 1_790_771_696,
                    ..Default::default()
                },
                console: Shared::default(),
                folder: scratch(name),
            }
        }
        fn logger(&self) -> Logger {
            Logger::new(self.folder.join("logs"), Box::new(self.console.clone()))
        }
        fn file(&self) -> String {
            fs::read_to_string(self.folder.join("logs/server-2026-09-30.log")).unwrap_or_default()
        }
    }

    impl Drop for Rig {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.folder);
        }
    }

    fn player(seat: u8, callsign: &str) -> PlayerFigures {
        PlayerFigures {
            id: seat,
            seat: Some(seat),
            callsign: callsign.into(),
            plane: Some(u32::from(seat)),
            ..Default::default()
        }
    }

    #[test]
    fn events_reach_the_log_and_the_host_finishing_ends_the_loop() {
        let mut rig = Rig::new("run-events");
        let mut host = ScriptedHost {
            flying: true,
            finish_at: Some(Duration::from_secs(2)),
            ..Default::default()
        };
        host.scheduled = vec![
            (
                Duration::from_millis(100),
                Event::Joined {
                    address: "127.0.0.1:5000".into(),
                    callsign: "Viper".into(),
                },
            ),
            (
                Duration::from_millis(200),
                Event::Seated {
                    seat: 1,
                    callsign: "Viper".into(),
                    plane: 0,
                },
            ),
            (
                Duration::from_millis(900),
                Event::Left {
                    seat: Some(1),
                    callsign: "Viper".into(),
                    plane: Some(0),
                    reason: "left".into(),
                },
            ),
        ];
        let (_keep, console) = channel();
        let mut log = rig.logger();
        let ended = Loop::new(&mut rig.timer, &mut log, &console, 0).run(&mut host);
        assert_eq!(ended, Ended::Finished);
        let file = rig.file();
        let lines: Vec<&str> = file.lines().collect();
        assert!(
            lines[0].ends_with("127.0.0.1:5000 joined as Viper"),
            "{file}"
        );
        assert!(lines[1].ends_with("seat 1 Viper took plane 0"));
        assert!(lines[2].ends_with("seat 1 Viper (plane 0) left: left"));
        assert!(lines[3].ends_with("The last mission has ended. Stopping."));
        assert!(lines[0].starts_with("2026-09-30 12:34:56"), "{}", lines[0]);
    }

    #[test]
    fn what_keeps_the_loop_on_time_is_logged_once_as_it_starts() {
        let mut rig = Rig::new("run-on-time");
        rig.timer.on_time = Some("macOS real-time scheduling on".into());
        let mut host = ScriptedHost {
            flying: true,
            finish_at: Some(Duration::from_millis(100)),
            ..Default::default()
        };
        let (_keep, console) = channel();
        let mut log = rig.logger();
        Loop::new(&mut rig.timer, &mut log, &console, 0).run(&mut host);
        assert_eq!(
            rig.timer.on_time_tick,
            Some(Duration::from_nanos(8_333_333))
        );
        let file = rig.file();
        assert_eq!(file.matches("macOS real-time scheduling on").count(), 1);
        assert!(
            file.lines()
                .next()
                .unwrap()
                .ends_with("macOS real-time scheduling on")
        );
    }

    #[test]
    fn the_loop_polls_often_and_advances_the_ticks_in_real_time() {
        let mut rig = Rig::new("run-pace");
        let mut host = ScriptedHost {
            flying: true,
            finish_at: Some(Duration::from_secs(1)),
            ..Default::default()
        };
        let (_keep, console) = channel();
        let mut log = rig.logger();
        Loop::new(&mut rig.timer, &mut log, &console, 0).run(&mut host);
        // The fake clock reached one second; the host saw a tick at 120 Hz and
        // was polled at least every 4 ms, about 8.3 ms apart per tick.
        assert_eq!(host.tick, 120);
        assert!(host.polls >= 120 && host.polls < 600, "{}", host.polls);
    }

    #[test]
    fn status_lines_print_on_their_interval_and_not_in_the_log() {
        let mut rig = Rig::new("run-status");
        let mut host = ScriptedHost {
            flying: true,
            finish_at: Some(Duration::from_millis(35_500)),
            ..Default::default()
        };
        let (_keep, console) = channel();
        let mut log = rig.logger();
        Loop::new(&mut rig.timer, &mut log, &console, 10).run(&mut host);
        let console_text = rig.console.text();
        let statuses: Vec<&str> = console_text
            .lines()
            .filter(|l| l.contains(" tick "))
            .collect();
        assert_eq!(statuses.len(), 3, "{console_text}");
        assert!(statuses[0].starts_with("00:00:10 tick 1200 players 0/15 aircraft 30"));
        assert!(!rig.file().contains(" tick "));
    }

    #[test]
    fn a_zero_interval_prints_no_status_lines() {
        let mut rig = Rig::new("run-nostatus");
        let mut host = ScriptedHost {
            flying: true,
            finish_at: Some(Duration::from_secs(25)),
            ..Default::default()
        };
        let (_keep, console) = channel();
        let mut log = rig.logger();
        Loop::new(&mut rig.timer, &mut log, &console, 0).run(&mut host);
        assert!(!rig.console.text().contains(" tick "));
    }

    #[test]
    fn each_players_figures_are_logged_once_a_minute() {
        let mut rig = Rig::new("run-figures");
        let mut host = ScriptedHost {
            flying: true,
            finish_at: Some(Duration::from_secs(125)),
            players: vec![player(1, "Viper"), player(2, "Cobra")],
            ..Default::default()
        };
        let (_keep, console) = channel();
        let mut log = rig.logger();
        Loop::new(&mut rig.timer, &mut log, &console, 0).run(&mut host);
        let file = rig.file();
        assert_eq!(file.matches("figures seat 1 Viper").count(), 2, "{file}");
        assert_eq!(file.matches("figures seat 2 Cobra").count(), 2);
        // Figures are for the file; the console does not repeat them.
        assert!(!rig.console.text().contains("figures"));
    }

    #[test]
    fn console_commands_reach_the_host_and_quit_stops_it_cleanly() {
        let mut rig = Rig::new("run-console");
        let mut host = ScriptedHost {
            flying: true,
            players: vec![
                player(1, "Viper"),
                player(2, "Cobra"),
                PlayerFigures {
                    id: 7,
                    callsign: "Hawk".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (sender, console) = channel();
        for line in [
            "status",
            "players",
            "kick 2",
            "kick 9",
            "kick-player 7 Wrong slot",
            "kick-player 8",
            "end",
            "restart",
            "broadcast on",
            "status",
            "bogus",
            "help",
            "quit",
        ] {
            sender.send(crate::console::parse(line).unwrap()).unwrap();
        }
        let mut log = rig.logger();
        let ended = Loop::new(&mut rig.timer, &mut log, &console, 0).run(&mut host);
        assert_eq!(ended, Ended::Quit);
        assert_eq!(host.kicked, vec![2]);
        assert_eq!(host.kicked_players, vec![7], "a lobby player, by its id");
        assert_eq!((host.ended, host.restarted), (1, 1));
        assert_eq!(host.broadcast, Some(true));
        assert_eq!(host.stopped, Some(Duration::ZERO));
        let text = rig.console.text();
        for needle in [
            "tick 0 players 3/15",
            "Viper",
            "kicked seat 2 Cobra",
            "no player in seat 9",
            "kicked player 7 Hawk",
            "no player has the lobby id 8",
            "ending the mission",
            "restarting the mission",
            "console: broadcast on",
            "down 7 KB/s, broadcast: listed",
            "unknown command `bogus`",
            "Commands: status",
            "Stopped",
        ] {
            assert!(text.contains(needle), "{needle} in {text}");
        }
        assert!(rig.file().contains("console: quit"));
    }
}
