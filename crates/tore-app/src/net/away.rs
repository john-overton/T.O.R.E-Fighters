//! The game's part of the AI flying an idle player's aircraft (stage F
//! phase 2, slice F2-A; docs/ARCHITECTURE.md, "The AI flies an idle player's
//! aircraft").
//!
//! - **Away** is not a centred stick, which a player cruising hands-off also
//!   sends. The game is away while its controls are neutral because of a
//!   menu (the Esc or pause menu, a screen opened over the flight), the
//!   window lacking focus, or the loss of the controller it was flying with;
//!   once that has lasted the King's `idle-ai` time (5 minutes by default)
//!   it tells the host ([`tore_session::Client::away`]). A plane already lost
//!   is the revival rules', not the AI's.
//! - **While the AI flies** the plane the game shows it on the observer
//!   screen (slice F2-O3: the replay viewer's live mode on the stream the
//!   host starts for the away player, in `net/observe.rs`), opened over the
//!   flight, which waits underneath. [`BANNER`] is said every few seconds.
//! - **No flight input takes the plane back** (John, 2026-10-06, slice
//!   F2-O4: it is a flight sim and nobody should fly AFK). The player comes
//!   back on purpose from the observer screen's Escape menu
//!   ([`AwayRows`], `replay/pause.rs`): **Take Back Flight** while the
//!   reserved aircraft is alive, **Spawn in Aircraft** when none is left
//!   (the revival rules), and **Leave Game**.
//! - **The plane is no longer kept** (the AI lost it, or the King released
//!   it): the observer screen stays, with the menu's Spawn in Aircraft row.
//!   A flight with no observer screen to show (the recording could not
//!   start) takes the aircraft back at once, since there is no menu to do
//!   it from.

use crate::replay::pause::AwayRows;
use crate::{App, Screen};
use std::time::{Duration, Instant};
use tore_session::Controls;
use tore_session::client::revival::may_fly_again;
use tore_session::settings::Respawn;
use tore_session::settings::number;
use tore_session::wire::messages::LobbyState;
use tore_session::wire::messages::Revival;
use winit::event_loop::ActiveEventLoop;

/// A stick or rudder axis past this, either way, is a flight input (agent
/// decision: well past a resting stick's noise, well short of a turn).
pub const STICK_TOUCH: f64 = 0.1;
/// A throttle lever moved this far from where it was is a flight input.
pub const THROTTLE_TOUCH: f64 = 0.05;
/// While the AI flies the plane the banner is said this often (the HUD's
/// message lines fade).
pub const BANNER_EVERY: Duration = Duration::from_secs(5);
/// How long the game waits for its plane after the host stops keeping it
/// before it ends the flight: Back's Seated message may come a moment after
/// the end of the observer flight it follows.
pub const GONE_AFTER: Duration = Duration::from_secs(2);
/// What the screen says while the AI flies the plane.
pub const BANNER: &str =
    "The AI is flying your aircraft. Press Esc, then Take Back Flight, to fly it again.";
/// The same on the observer screen, whose top line is shorter.
pub const WATCH_BANNER: &str = "The AI is flying your aircraft. Esc: Take Back Flight.";
/// The observer screen's line when no aircraft is kept for the player.
pub const WATCH_BANNER_GONE: &str = "You have no aircraft. Esc: Spawn in Aircraft.";

/// What the game does about the idle rule this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Nothing,
    /// Tell the host the game has been away for the setting's seconds.
    Away,
    /// The host keeps the plane no longer and there is no observer screen to
    /// stay on: end the flight.
    End,
}

/// What one frame of the flight shows the idle rule.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    pub now: Instant,
    /// A menu or a screen is over the flight, or the window lacks focus:
    /// the controls are neutral.
    pub held: bool,
    /// The controls the frame read (the pilot's own when not `held`).
    pub controls: &'a Controls,
    /// The King's `idle-ai` seconds; `None` for never.
    pub idle_seconds: Option<u32>,
    /// The AI flies the plane, or Away was sent.
    pub away: bool,
    /// The player has a plane.
    pub seated: bool,
    /// The plane is lost: the revival rules have it.
    pub lost: bool,
    /// The observer screen is up over the flight: it stays when the host
    /// keeps the plane no more, for the menu's Spawn in Aircraft.
    pub watching: bool,
}

/// The idle rule's state for one flight.
#[derive(Clone, Debug, Default)]
pub struct Idle {
    /// The controls have been neutral since then.
    since: Option<Instant>,
    /// The controller the player flew with went, and nothing since.
    controller_lost: bool,
    /// The throttle lever's position while the player last flew.
    throttle: Option<f64>,
    /// When the banner was last said.
    banner: Option<Instant>,
    /// The AI flew the plane, and since when the host no longer keeps it.
    was_away: bool,
    gone_since: Option<Instant>,
}

impl Idle {
    /// The controller the player was flying with is gone: the controls are
    /// neutral until any flight input.
    pub fn controller_lost(&mut self) {
        self.controller_lost = true;
    }

    /// This frame's act.
    pub fn step(&mut self, f: Frame) -> Act {
        let touched = !f.held && touched(f.controls, self.throttle);
        if touched {
            self.controller_lost = false;
        }
        if f.away {
            // No flight input takes the plane back (John, 2026-10-06): the
            // player does it from the observer screen's menu.
            self.since = None;
            self.was_away = true;
            self.gone_since = None;
            return Act::Nothing;
        }
        if self.was_away {
            if f.seated || f.watching {
                // Seated again, or still on the observer screen: with no
                // plane kept the menu's Spawn in Aircraft is the way on.
                self.gone_since = None;
                if f.seated {
                    self.was_away = false;
                }
                if f.watching {
                    return Act::Nothing;
                }
            } else {
                let gone = *self.gone_since.get_or_insert(f.now);
                if f.now.duration_since(gone) >= GONE_AFTER {
                    self.was_away = false;
                    return Act::End;
                }
                return Act::Nothing;
            }
        }
        if !f.held
            && let Some(lever) = f.controls.pilot.throttle
        {
            self.throttle = Some(lever);
        }
        let neutral = f.held || self.controller_lost;
        let Some(seconds) = f.idle_seconds.filter(|_| neutral && f.seated && !f.lost) else {
            self.since = None;
            return Act::Nothing;
        };
        let since = *self.since.get_or_insert(f.now);
        if f.now.duration_since(since) >= Duration::from_secs(u64::from(seconds)) {
            self.since = None;
            Act::Away
        } else {
            Act::Nothing
        }
    }

    /// Whether the banner is due again at `now`.
    pub fn banner_due(&mut self, now: Instant) -> bool {
        let due = self
            .banner
            .is_none_or(|at| now.duration_since(at) >= BANNER_EVERY);
        if due {
            self.banner = Some(now);
        }
        due
    }
}

/// Whether `controls` hold a flight input: a stick or rudder past
/// [`STICK_TOUCH`], a throttle key, the throttle lever moved past
/// [`THROTTLE_TOUCH`] from `lever` (its last position while flying), the
/// trigger, or any pilot or seat command (a flight key). Used for the lost
/// controller only: a flight input no longer takes the plane back.
pub fn touched(controls: &Controls, lever: Option<f64>) -> bool {
    let pilot = &controls.pilot;
    [pilot.pitch, pilot.roll, pilot.yaw]
        .iter()
        .any(|axis| axis.abs() > STICK_TOUCH)
        || pilot.throttle_rate != 0.
        || pilot
            .throttle
            .zip(lever)
            .is_some_and(|(now, was)| (now - was).abs() > THROTTLE_TOUCH)
        || controls.trigger
        || !pilot.commands.is_empty()
        || !controls.commands.is_empty()
}

/// The King's `idle-ai` seconds from the lobby state; `None` for never or
/// before the first state.
pub fn idle_seconds(lobby: Option<&LobbyState>) -> Option<u32> {
    seconds_in(&lobby?.settings)
}

/// The `idle-ai` seconds in a lobby state's settings list.
fn seconds_in(settings: &[(u8, u32)]) -> Option<u32> {
    settings
        .iter()
        .find(|(n, _)| *n == number::IDLE_AI)
        .map(|(_, seconds)| *seconds)
        .filter(|seconds| *seconds != 0)
}

/// Why the revival rules allow no new aircraft now, in a few words for the
/// menu's dimmed Spawn in Aircraft row; `None` when they allow one
/// (`seconds_left` is the wait that remains).
pub fn spawn_block(revival: &Revival, seconds_left: u32) -> Option<String> {
    if may_fly_again(revival, seconds_left) {
        return None;
    }
    if revival.rule == Respawn::None {
        return Some("No revival in this game".into());
    }
    if revival.lives == Some(0) {
        return Some("No lives left".into());
    }
    if let Some(why) = revival.why.as_deref() {
        return Some(why.trim_end_matches('.').to_owned());
    }
    Some(format!(
        "You can fly again in {}:{:02}",
        seconds_left / 60,
        seconds_left % 60
    ))
}

/// The menu's rows for an away player, from what the game knows.
pub fn menu_rows(
    own_aircraft: bool,
    revival: Option<(&Revival, u32)>,
    leave_ends_game: bool,
) -> AwayRows {
    AwayRows {
        own_aircraft,
        // A released aircraft brings no revival: the usual take rules say.
        spawn_blocked: revival
            .filter(|_| !own_aircraft)
            .and_then(|(revival, left)| spawn_block(revival, left)),
        leave_ends_game,
    }
}

impl App {
    /// The idle rule's turn, once the session's events are handled:
    /// Away when the game has been away long enough, the banner, the
    /// observer menu's rows, and the flight's end when the host keeps the
    /// plane no longer and nothing is on screen to show it.
    pub(crate) fn net_idle(&mut self, controls: &Controls) {
        let held = self.idle_held();
        let watching = self.away_watching();
        let now = Instant::now();
        let (Some(flight), Some(session)) = (&mut self.net_flight, &mut self.net) else {
            return;
        };
        let client = &mut session.client;
        let own = client.ai_flies().is_some();
        let act = flight.idle.step(Frame {
            now,
            held,
            controls,
            idle_seconds: idle_seconds(client.lobby()),
            away: own || client.away_asked(),
            seated: client.seat().is_some(),
            lost: client.revival().is_some(),
            watching,
        });
        let planeless = watching && !own && client.seat().is_none();
        let banner = (own || planeless) && flight.idle.banner_due(now);
        match act {
            Act::Away => {
                log::info!("Network: away for the idle-ai time; the AI flies the plane");
                client.away();
            }
            Act::End => {
                client.stop_watching();
                self.end_net_flight();
                return;
            }
            Act::Nothing => {}
        }
        if banner {
            let text = match (self.screen == Screen::Replay, planeless) {
                (true, true) => WATCH_BANNER_GONE,
                (true, false) => WATCH_BANNER,
                (false, _) => BANNER,
            };
            self.away_say(text);
        }
        self.away_menu_turn();
    }

    /// The observer screen's Escape menu offers what the game's state allows
    /// (slice F2-O4): Take Back Flight, Spawn in Aircraft or neither, and
    /// Leave Game.
    fn away_menu_turn(&mut self) {
        if !self.away_watching() {
            return;
        }
        let Some(session) = &self.net else {
            return;
        };
        let now = session.now();
        let client = &session.client;
        let revival = client
            .revival()
            .map(|kept| (&kept.revival, kept.seconds_left(now)));
        let rows = menu_rows(
            client.ai_flies().is_some(),
            revival,
            session.leaving_ends_game(),
        );
        if let Some(replay) = &mut self.replay {
            replay.viewer.set_away_menu(Some(rows));
        }
    }

    /// Says `text` where the player looks: on the observer screen while it
    /// watches the aircraft, else on the flight's HUD.
    fn away_say(&mut self, text: &str) {
        match self
            .replay
            .as_mut()
            .filter(|_| self.screen == Screen::Replay)
        {
            Some(replay) => replay.viewer.message(text),
            None => self.flight_ui.message(text),
        }
    }

    /// Whether the controls count as neutral this frame, for the idle rule: a
    /// menu or a screen is over the flight, or the window lacks focus.
    fn idle_held(&self) -> bool {
        self.screen != Screen::Flight || self.flight_ui.frozen() || !self.focused
    }

    /// Whether the observer screen shows the player's own aircraft while it
    /// is away (slice F2-O3): the flight waits under it. It stays up after
    /// the AI loses the aircraft, for the menu's Spawn in Aircraft
    /// (slice F2-O4).
    pub(crate) fn away_watching(&self) -> bool {
        self.screen == Screen::Replay
            && self.net_flight.is_some()
            && self.replay.as_ref().is_some_and(|r| r.viewer.live())
    }

    /// Whether the controls' input is paused, as the screen decides: any
    /// screen but the flight pauses it, a menu or the pause over the flight
    /// too.
    pub(crate) fn input_paused(&self) -> bool {
        self.screen != Screen::Flight || self.flight_ui.frozen()
    }

    /// Take Back Flight on the observer screen's menu: the host gives the
    /// reserved aircraft back, with its stores and damage as the AI left
    /// them (slice F2-O4).
    pub(crate) fn away_take_back(&mut self) {
        let Some(session) = &mut self.net else {
            return;
        };
        session.client.back();
        log::info!("Network: Take Back Flight; taking the aircraft back from the AI");
        self.away_say("Taking your aircraft back from the AI...");
    }

    /// Spawn in Aircraft on the observer screen's menu: no reserved aircraft
    /// is left, so the player flies again by the revival rules, as the host
    /// brought them in F2-A and K5 (the AI lost the aircraft while the
    /// player was away). When the King released the aircraft no revival is
    /// noted, and the player takes any free aircraft by the usual take
    /// rules instead (agent decision: no new rules).
    pub(crate) fn away_spawn(&mut self) {
        let Some(session) = &mut self.net else {
            return;
        };
        if session.client.revival().is_some() {
            session.client.revive();
        } else {
            session.client.ready(None);
        }
        log::info!("Network: Spawn in Aircraft");
        self.away_say("Flying again...");
    }

    /// Leave Game on the observer screen's menu. The game's player leaves
    /// the session: another's game and a dedicated server's just see it go
    /// (the aircraft stays with the AI, as End Mission leaves it: the host
    /// frees the reservation of an away player who says goodbye). A hosting
    /// player hands the game over to a ready standby first; with none, the
    /// menu has asked to confirm and the game ends for everyone.
    pub(crate) fn away_leave_game(&mut self, event_loop: &ActiveEventLoop) {
        let Some(session) = &mut self.net else {
            return;
        };
        log::info!(
            "Network: Leave Game ({})",
            match (session.hosting(), session.leaving_ends_game()) {
                (false, _) => "not the host",
                (true, false) => "handing the game over",
                (true, true) => "ending the game",
            }
        );
        session.hand_over_for_leave();
        self.net_ending = Some("You left the game.".into());
        self.end_session(event_loop);
    }

    /// The observer screen of the player's own aircraft goes with its
    /// flight: the recording and the viewer are put away.
    pub(crate) fn close_away_watch(&mut self) {
        self.observing = None;
        if self.replay.as_ref().is_some_and(|r| r.viewer.live()) {
            self.close_viewer();
        }
    }

    /// The controller the player flew with went: the controls are neutral,
    /// which counts as away.
    pub(crate) fn net_controller_lost(&mut self) {
        if let Some(flight) = &mut self.net_flight {
            flight.idle.controller_lost();
        }
    }

    /// End Mission while the AI flies the plane, or after the AI lost it, in
    /// a game with a lobby screen: the host has no flight of the player's
    /// to end, so the game leaves the plane to the AI, stops watching and is
    /// back in the lobby. `false` when the player flies, and Leave goes to
    /// the host as ever.
    pub(crate) fn leave_away(&mut self) -> bool {
        let Some(session) = &mut self.net else {
            return false;
        };
        if self.net_flight.is_none()
            || !session.lobby_screen
            || session.hosting()
            || session.client.seat().is_some()
        {
            return false;
        }
        session.client.stop_watching();
        self.end_net_flight();
        self.message("You left your aircraft to the AI.");
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controls() -> Controls {
        Controls::default()
    }

    fn frame(now: Instant, held: bool, controls: &Controls) -> Frame<'_> {
        Frame {
            now,
            held,
            controls,
            idle_seconds: Some(600),
            away: false,
            seated: true,
            lost: false,
            watching: false,
        }
    }

    #[test]
    fn a_flight_input_is_a_stick_past_rest_a_key_or_the_lever_moved() {
        let mut c = controls();
        assert!(!touched(&c, None));
        c.pilot.pitch = 0.05;
        c.pilot.yaw = -0.08;
        assert!(!touched(&c, None), "a resting stick's noise");
        c.pilot.roll = -0.3;
        assert!(touched(&c, None));
        let mut c = controls();
        c.pilot.throttle = Some(0.8);
        assert!(!touched(&c, Some(0.78)));
        assert!(!touched(&c, None));
        assert!(touched(&c, Some(0.6)));
        let mut c = controls();
        c.pilot.throttle_rate = 1.;
        assert!(touched(&c, None));
        let mut c = controls();
        c.trigger = true;
        assert!(touched(&c, None));
        let mut c = controls();
        c.pilot
            .commands
            .push(tore_sim::flight::PilotCommand::Toggle(
                tore_sim::flight::Switch::Gear,
            ));
        assert!(touched(&c, None));
    }

    #[test]
    fn away_after_the_settings_time_of_neutral_controls_and_no_flight_input_takes_it_back() {
        let start = Instant::now();
        let at = |s: u64| start + Duration::from_secs(s);
        let mut idle = Idle::default();
        let neutral = controls();
        // A hands-off cruise is not away.
        for s in 0..30 {
            assert_eq!(idle.step(frame(at(s), false, &neutral)), Act::Nothing);
        }
        // A menu for 599 seconds, closed, then 600 (the longest setting):
        // away once the last passes; the first count was cut short.
        for s in 30..629 {
            assert_eq!(idle.step(frame(at(s), true, &neutral)), Act::Nothing);
        }
        assert_eq!(idle.step(frame(at(629), false, &neutral)), Act::Nothing);
        for s in 630..1230 {
            assert_eq!(idle.step(frame(at(s), true, &neutral)), Act::Nothing);
        }
        assert_eq!(idle.step(frame(at(1230), true, &neutral)), Act::Away);
        // Away: a stick, a key or the trigger does not take the plane back
        // (John, 2026-10-06): the menu does, and the idle rule says nothing.
        let mut stick = controls();
        stick.pilot.pitch = 0.5;
        let away = |now, held, controls| Frame {
            away: true,
            seated: false,
            ..frame(now, held, controls)
        };
        stick.trigger = true;
        assert_eq!(idle.step(away(at(1231), true, &stick)), Act::Nothing);
        assert_eq!(idle.step(away(at(1232), false, &neutral)), Act::Nothing);
        assert_eq!(idle.step(away(at(1233), false, &stick)), Act::Nothing);
        // Seated again by the menu's Take Back Flight: flying, nothing more.
        assert_eq!(idle.step(frame(at(1234), false, &stick)), Act::Nothing);
        assert_eq!(idle.step(frame(at(1300), false, &neutral)), Act::Nothing);
    }

    #[test]
    fn a_lost_controller_counts_until_any_flight_input() {
        let start = Instant::now();
        let at = |s: u64| start + Duration::from_secs(s);
        let mut idle = Idle::default();
        let neutral = controls();
        idle.controller_lost();
        assert_eq!(idle.step(frame(at(0), false, &neutral)), Act::Nothing);
        assert_eq!(idle.step(frame(at(600), false, &neutral)), Act::Away);
        // Before the host answers, a key is not a way back either.
        let mut key = controls();
        key.trigger = true;
        let asked = Frame {
            away: true,
            ..frame(at(601), false, &key)
        };
        assert_eq!(idle.step(asked), Act::Nothing);
        // And the count no longer runs once the player flies again.
        idle.step(frame(at(602), false, &key));
        assert_eq!(idle.step(frame(at(30), false, &neutral)), Act::Nothing);
    }

    #[test]
    fn never_a_lost_plane_or_no_plane_is_not_away() {
        let start = Instant::now();
        let neutral = controls();
        for case in 0..3 {
            let mut idle = Idle::default();
            for s in 0..60 {
                let mut f = frame(start + Duration::from_secs(s), true, &neutral);
                match case {
                    0 => f.idle_seconds = None,
                    1 => f.lost = true,
                    _ => f.seated = false,
                }
                assert_eq!(idle.step(f), Act::Nothing, "case {case}");
            }
        }
    }

    #[test]
    fn a_plane_no_longer_kept_ends_the_flight_unless_it_comes_back() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let neutral = controls();
        let away = |now| Frame {
            away: true,
            seated: false,
            ..frame(now, false, &neutral)
        };
        let gone = |now| Frame {
            seated: false,
            ..frame(now, false, &neutral)
        };
        // Back: the plane comes a moment after the observer flight ends.
        let mut idle = Idle::default();
        idle.step(away(at(0)));
        assert_eq!(idle.step(gone(at(100))), Act::Nothing);
        assert_eq!(idle.step(frame(at(400), false, &neutral)), Act::Nothing);
        assert_eq!(idle.step(gone(at(5_000))), Act::Nothing);
        // The AI lost it: no plane comes.
        let mut idle = Idle::default();
        idle.step(away(at(0)));
        assert_eq!(idle.step(gone(at(100))), Act::Nothing);
        assert_eq!(idle.step(gone(at(2_200))), Act::End);
        assert_eq!(idle.step(gone(at(2_300))), Act::Nothing);
        // The AI lost it with the observer screen up: the screen stays, for
        // the menu's Spawn in Aircraft (slice F2-O4), however long.
        let mut idle = Idle::default();
        idle.step(away(at(0)));
        let watching = |now| Frame {
            watching: true,
            ..gone(now)
        };
        for ms in [100, 2_200, 60_000, 600_000] {
            assert_eq!(idle.step(watching(at(ms))), Act::Nothing, "{ms}");
        }
        // The menu's choice seats the player: flying again.
        assert_eq!(idle.step(frame(at(600_100), false, &neutral)), Act::Nothing);
        assert_eq!(idle.step(gone(at(700_000))), Act::Nothing);
    }

    #[test]
    fn the_setting_comes_from_the_lobby_state() {
        assert_eq!(idle_seconds(None), None);
        assert_eq!(seconds_in(&[]), None);
        assert_eq!(
            seconds_in(&[(number::MODE, 0), (number::IDLE_AI, 300)]),
            Some(300)
        );
        assert_eq!(seconds_in(&[(number::IDLE_AI, 0)]), None);
    }

    fn revival(rule: Respawn, lives: Option<u8>, wait: u32, why: Option<&str>) -> Revival {
        Revival {
            rule,
            lives,
            wait_seconds: wait,
            why: why.map(str::to_owned),
        }
    }

    #[test]
    fn spawn_in_aircraft_is_dimmed_with_the_revival_rules_reason() {
        let free = revival(Respawn::Revive, Some(2), 0, None);
        assert_eq!(spawn_block(&free, 0), None);
        let none = revival(Respawn::None, None, 0, Some("No revival in this game."));
        assert_eq!(
            spawn_block(&none, 0).as_deref(),
            Some("No revival in this game")
        );
        let out = revival(Respawn::AiSlot, Some(0), 0, Some("No lives left."));
        assert_eq!(spawn_block(&out, 0).as_deref(), Some("No lives left"));
        let waiting = revival(Respawn::Revive, Some(1), 90, None);
        assert_eq!(
            spawn_block(&waiting, 45).as_deref(),
            Some("You can fly again in 0:45")
        );
        assert_eq!(spawn_block(&waiting, 0), None, "the wait is over");
        let room = revival(
            Respawn::Revive,
            None,
            0,
            Some("Waiting for room for another aircraft."),
        );
        assert_eq!(
            spawn_block(&room, 0).as_deref(),
            Some("Waiting for room for another aircraft")
        );
    }

    #[test]
    fn the_menus_rows_follow_the_aircraft_the_rules_and_who_hosts() {
        let none = revival(Respawn::None, None, 0, Some("No revival in this game."));
        let ok = revival(Respawn::Revive, None, 0, None);
        // The reserved aircraft is alive: Take Back Flight, whatever the
        // rules say.
        let alive = menu_rows(true, Some((&none, 0)), false);
        assert!(alive.own_aircraft && alive.spawn_blocked.is_none());
        // Lost while away: Spawn in Aircraft, dimmed under `none`, free
        // under a revival rule.
        assert!(!menu_rows(false, Some((&none, 0)), false).own_aircraft);
        assert!(
            menu_rows(false, Some((&none, 0)), false)
                .spawn_blocked
                .is_some()
        );
        assert_eq!(menu_rows(false, Some((&ok, 0)), false).spawn_blocked, None);
        // Released by the King: no revival, Spawn is free to try.
        assert_eq!(menu_rows(false, None, false).spawn_blocked, None);
        // Leaving ends the game only for a host with no ready standby.
        assert!(menu_rows(true, None, true).leave_ends_game);
        assert!(!menu_rows(true, None, false).leave_ends_game);
    }

    #[test]
    fn the_banner_is_said_every_few_seconds() {
        let start = Instant::now();
        let mut idle = Idle::default();
        assert!(idle.banner_due(start));
        assert!(!idle.banner_due(start + Duration::from_secs(4)));
        assert!(idle.banner_due(start + BANNER_EVERY));
    }
}
