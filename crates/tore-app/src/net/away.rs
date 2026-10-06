//! The game's part of the AI flying an idle player's aircraft (stage F
//! phase 2, slice F2-A; docs/ARCHITECTURE.md, "The AI flies an idle player's
//! aircraft").
//!
//! - **Away** is not a centred stick, which a player cruising hands-off also
//!   sends. The game is away while its controls are neutral because of a
//!   menu (the Esc or pause menu, a screen opened over the flight), the
//!   window lacking focus, or the loss of the controller it was flying with;
//!   once that has lasted the King's `idle-ai` seconds it tells the host
//!   ([`tore_session::Client::away`]). A plane already lost is the revival
//!   rules', not the AI's.
//! - **While the AI flies** the plane the flight screen keeps its last
//!   picture (agent decision: the observer view that would show the plane
//!   flying on is slice F2-O2's) and says, every few seconds, [`BANNER`].
//! - **Back** at the first flight input with no menu up and the window
//!   focused ([`touched`]): the host gives the plane back, and its Seated
//!   message starts the flight again, as a revival's does.
//! - **The plane is no longer kept** (the AI lost it, the host says so): the
//!   flight ends, and a lobby screen's player is back in the lobby.

use crate::{App, Screen};
use std::time::{Duration, Instant};
use tore_session::Controls;
use tore_session::settings::number;
use tore_session::wire::messages::LobbyState;

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
    "The AI is flying your aircraft. Move the stick or press any flight key to take it back.";

/// What the game does about the idle rule this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Nothing,
    /// Tell the host the game has been away for the setting's seconds.
    Away,
    /// Tell the host the player is back at the controls.
    Back,
    /// The host keeps the plane no longer: end the flight.
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
            self.since = None;
            self.was_away = true;
            self.gone_since = None;
            return if touched { Act::Back } else { Act::Nothing };
        }
        if self.was_away {
            if f.seated {
                self.was_away = false;
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
/// trigger, or any pilot or seat command (a flight key).
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

impl App {
    /// The idle rule's turn, once the session's events are handled:
    /// Away when the game has been away long enough, Back at the first
    /// flight input, the banner, and the flight's end when the host keeps
    /// the plane no longer.
    pub(crate) fn net_idle(&mut self, controls: &Controls) {
        let held = self.screen != Screen::Flight || self.flight_ui.frozen() || !self.focused;
        let now = Instant::now();
        let (Some(flight), Some(session)) = (&mut self.net_flight, &mut self.net) else {
            return;
        };
        let client = &mut session.client;
        let act = flight.idle.step(Frame {
            now,
            held,
            controls,
            idle_seconds: idle_seconds(client.lobby()),
            away: client.ai_flies().is_some() || client.away_asked(),
            seated: client.seat().is_some(),
            lost: client.revival().is_some(),
        });
        let banner = client.ai_flies().is_some() && flight.idle.banner_due(now);
        match act {
            Act::Away => {
                log::info!("Network: away for the idle-ai seconds; the AI flies the plane");
                client.away();
            }
            Act::Back => {
                client.back();
                self.flight_ui
                    .message("Taking your aircraft back from the AI...");
            }
            Act::End => {
                client.stop_watching();
                self.end_net_flight();
                return;
            }
            Act::Nothing => {}
        }
        if banner {
            self.flight_ui.message(BANNER);
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
            idle_seconds: Some(10),
            away: false,
            seated: true,
            lost: false,
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
    fn away_after_the_settings_seconds_of_neutral_controls_then_back() {
        let start = Instant::now();
        let at = |s: u64| start + Duration::from_secs(s);
        let mut idle = Idle::default();
        let neutral = controls();
        // A hands-off cruise is not away.
        for s in 0..30 {
            assert_eq!(idle.step(frame(at(s), false, &neutral)), Act::Nothing);
        }
        // A menu for 9 seconds, closed, then 10: away once the tenth passes.
        for s in 30..40 {
            assert_eq!(idle.step(frame(at(s), true, &neutral)), Act::Nothing);
        }
        assert_eq!(idle.step(frame(at(40), false, &neutral)), Act::Nothing);
        for s in 41..51 {
            assert_eq!(idle.step(frame(at(s), true, &neutral)), Act::Nothing);
        }
        assert_eq!(idle.step(frame(at(51), true, &neutral)), Act::Away);
        // Away: the menu is still up, so a held stick is not yet back.
        let mut stick = controls();
        stick.pilot.pitch = 0.5;
        let away = |now, held, controls| Frame {
            away: true,
            seated: false,
            ..frame(now, held, controls)
        };
        assert_eq!(idle.step(away(at(52), true, &stick)), Act::Nothing);
        assert_eq!(idle.step(away(at(53), false, &neutral)), Act::Nothing);
        assert_eq!(idle.step(away(at(54), false, &stick)), Act::Back);
        // Seated again: flying, nothing more.
        assert_eq!(idle.step(frame(at(55), false, &stick)), Act::Nothing);
        assert_eq!(idle.step(frame(at(70), false, &neutral)), Act::Nothing);
    }

    #[test]
    fn a_lost_controller_counts_until_any_flight_input() {
        let start = Instant::now();
        let at = |s: u64| start + Duration::from_secs(s);
        let mut idle = Idle::default();
        let neutral = controls();
        idle.controller_lost();
        assert_eq!(idle.step(frame(at(0), false, &neutral)), Act::Nothing);
        assert_eq!(idle.step(frame(at(10), false, &neutral)), Act::Away);
        // Before the host answers, a key on the keyboard is back.
        let mut key = controls();
        key.trigger = true;
        let asked = Frame {
            away: true,
            ..frame(at(11), false, &key)
        };
        assert_eq!(idle.step(asked), Act::Back);
        // And the count no longer runs once the player flies again.
        idle.step(frame(at(12), false, &key));
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
    }

    #[test]
    fn the_setting_comes_from_the_lobby_state() {
        assert_eq!(idle_seconds(None), None);
        assert_eq!(seconds_in(&[]), None);
        assert_eq!(
            seconds_in(&[(number::MODE, 0), (number::IDLE_AI, 30)]),
            Some(30)
        );
        assert_eq!(seconds_in(&[(number::IDLE_AI, 0)]), None);
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
