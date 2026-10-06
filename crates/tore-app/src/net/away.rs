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
//! - **While the AI flies** the plane the game shows it on the observer
//!   screen (slice F2-O3: the replay viewer's live mode on the stream the
//!   host starts for the away player, in `net/observe.rs`), opened over the
//!   flight, which waits underneath. A flight with no observer screen to
//!   show (a recording that could not start, a screen of the controls,
//!   graphics or sound open) keeps its last picture. [`BANNER`] is said every
//!   few seconds on either.
//! - **Back** at the first flight input with no menu up and the window
//!   focused ([`touched`]): the host gives the plane back, and its Seated
//!   message starts the flight again, as a revival's does. On the observer
//!   screen the flight inputs are the flight's own: the controllers, and the
//!   keys the controls' profile binds to a flight action (the arrows, the
//!   rudder keys, Space, the throttle and system keys; [`App::away_watch_key`]).
//!   The viewer keeps every other key and the mouse. Instrument keys and
//!   clicks, which make seat commands in flight, are the viewer's here and
//!   do not count.
//! - **Stop Watching** in the observer screen's Escape menu leaves the
//!   aircraft to the AI and the game to its lobby when the game has a lobby
//!   screen and is not hosting; a game without one (or hosting, where
//!   leaving ends the game) takes the aircraft back instead.
//! - **The plane is no longer kept** (the AI lost it, the host says so): the
//!   flight ends, and a lobby screen's player is back in the lobby.

use crate::input::Input;
use crate::{App, OWN, Screen};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use tore_session::Controls;
use tore_session::settings::number;
use tore_session::wire::messages::LobbyState;
use winit::keyboard::ModifiersState;

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
/// The same on the observer screen, whose top line is shorter.
pub const WATCH_BANNER: &str =
    "The AI is flying your aircraft. Press a flight key to take it back.";

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
    /// A flight key went down on the observer screen since the last frame
    /// ([`watch_key`]).
    keyed: bool,
    /// Back was said for this handoff (the log and the message say it once).
    back_said: bool,
}

impl Idle {
    /// The controller the player was flying with is gone: the controls are
    /// neutral until any flight input.
    pub fn controller_lost(&mut self) {
        self.controller_lost = true;
    }

    /// A flight key went down on the observer screen: the next frame that is
    /// not held counts it as a flight input.
    pub fn keyed(&mut self) {
        self.keyed = true;
    }

    /// Whether this is the first Back of the handoff: the log and the
    /// message say it once, while the player holds the stick.
    pub fn first_back(&mut self) -> bool {
        !std::mem::replace(&mut self.back_said, true)
    }

    /// This frame's act.
    pub fn step(&mut self, f: Frame) -> Act {
        let keyed = std::mem::take(&mut self.keyed);
        if !f.away {
            self.back_said = false;
        }
        let touched = !f.held && (keyed || touched(f.controls, self.throttle));
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

/// Whether Stop Watching on the observer screen of the player's own aircraft
/// leaves it to the AI and goes to the lobby (a game with a lobby screen
/// that the player does not host), rather than taking it back.
pub fn leaves_to_lobby(lobby_screen: bool, hosting: bool) -> bool {
    lobby_screen && !hosting
}

/// What the flight does with a key on the observer screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Taken {
    /// The viewer's own key.
    No,
    /// The flight's, nothing more (a release the profile holds).
    Yes,
    /// The flight's, and a flight input: the plane is wanted back.
    Touch,
}

/// Whether `name` is one of the flight's primary keys: the stick (the
/// arrows, unless Shift makes them look), the rudder (Z, X, End, Page Down),
/// the trigger (Space) and the throttle settings (1 to 8). Agent decision
/// (slice F2-O3): the viewer's own letters (t, f, g, h, o, x on its panels
/// and the drone's W, A, S, D, E, Q) stay the viewer's, so the flight's
/// systems and weapons keys are not read on this screen; a key the
/// controls' profile binds on the keyboard is, whatever it is.
pub fn flight_key(name: &str, modifiers: ModifiersState) -> bool {
    if modifiers.control_key() || modifiers.alt_key() || modifiers.super_key() {
        return false;
    }
    match name {
        "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" => !modifiers.shift_key(),
        "z" | "x" | "End" | "PageDown" | "Space" => true,
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" => true,
        _ => false,
    }
}

/// A key on the observer screen of an away player's aircraft: the flight's
/// primary keys ([`flight_key`]) and the keys the controls' profile binds
/// on the keyboard are the flight's, and their presses are flight inputs.
/// A release the profile holds goes to it, so nothing sticks. A press with
/// a menu of the viewer's open (`menu`), or with Alt or the system key held
/// (the window's own shortcuts), is the viewer's.
pub fn watch_key(
    input: &mut Input,
    name: &str,
    pressed: bool,
    repeat: bool,
    modifiers: ModifiersState,
    menu: bool,
) -> Taken {
    if !pressed {
        return if input.key(name, false, modifiers) {
            Taken::Yes
        } else {
            Taken::No
        };
    }
    if menu || modifiers.alt_key() || modifiers.super_key() {
        return Taken::No;
    }
    let bound = if repeat {
        input.claimed(name)
    } else {
        input.key(name, true, modifiers)
    };
    if bound || flight_key(name, modifiers) {
        Taken::Touch
    } else {
        Taken::No
    }
}

/// The pilot's controls on the observer screen, from the profile's state:
/// the axes, the throttle, the trigger and the commands queued since the
/// last frame, for [`touched`].
pub fn watch_frame(
    input: &mut Input,
    keys: &BTreeSet<String>,
    throttle: f64,
    sensors: tore_sim::sensors::Controls,
) -> Controls {
    let (pilot, _) = input.frame(keys, throttle);
    Controls {
        pilot,
        trigger: input.resolver.held("fire"),
        sensors,
        commands: Vec::new(),
        view_subject: None,
    }
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
        let held = self.idle_held();
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
                if flight.idle.first_back() {
                    log::info!("Network: a flight input; taking the aircraft back from the AI");
                    self.away_say("Taking your aircraft back from the AI...");
                }
            }
            Act::End => {
                client.stop_watching();
                self.end_net_flight();
                return;
            }
            Act::Nothing => {}
        }
        if banner {
            let text = if self.screen == Screen::Replay {
                WATCH_BANNER
            } else {
                BANNER
            };
            self.away_say(text);
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
    /// menu or a screen is over the flight, or the window lacks focus. On the
    /// observer screen the menus are the viewer's own and the screens the
    /// ones opened from its Escape menu.
    fn idle_held(&self) -> bool {
        if self.away_watching() {
            return self.watch_menu_open() || !self.focused;
        }
        self.screen != Screen::Flight || self.flight_ui.frozen() || !self.focused
    }

    /// Whether the observer screen shows the plane the AI flies for the
    /// player (slice F2-O3): the flight waits under it.
    pub(crate) fn away_watching(&self) -> bool {
        self.screen == Screen::Replay
            && self.net_flight.is_some()
            && self.replay.as_ref().is_some_and(|r| r.viewer.live())
            && self
                .net
                .as_ref()
                .is_some_and(|s| s.client.ai_flies().is_some())
    }

    /// Whether the observer screen has a menu or a screen open that takes
    /// the keys: the viewer's Escape menu or right-click menu, or the
    /// controls, graphics or sound screen opened from it.
    fn watch_menu_open(&self) -> bool {
        self.controls.is_some()
            || self.graphics_screen.is_some()
            || self.sound_screen.is_some()
            || self.replay.as_ref().is_some_and(|r| r.viewer.menu_open())
    }

    /// Whether the controls' input is paused, as the screen decides: any
    /// screen but the flight pauses it, a menu or the pause over the flight
    /// too; the observer screen of an away player's aircraft does not, so
    /// the controllers and the flight's keys are read ([`touched`]), until
    /// a menu or screen of the viewer's opens.
    pub(crate) fn input_paused(&self) -> bool {
        if self.away_watching() {
            return self.watch_menu_open();
        }
        self.screen != Screen::Flight || self.flight_ui.frozen()
    }

    /// The pilot's controls on the observer screen: what the controllers and
    /// the flight's keys held or queued say, as a tick of the flight takes
    /// them, for [`touched`]. Nothing of it is sent: the AI flies the plane.
    pub(crate) fn watch_controls(&mut self) -> Controls {
        let sensors = self.instruments.controls();
        let throttle = self.world.cockpits[OWN].flight.throttle;
        watch_frame(&mut self.input, &self.camera.keys, throttle, sensors)
    }

    /// A key on the observer screen of an away player's aircraft: a flight
    /// key ([`watch_key`]) is the flight's, and the viewer does not see it;
    /// its press is a flight input for the idle rule. `true` when the key
    /// was taken.
    pub(crate) fn away_watch_key(&mut self, name: &str, pressed: bool, repeat: bool) -> bool {
        if !self.away_watching() {
            return false;
        }
        let menu = self.watch_menu_open();
        match watch_key(&mut self.input, name, pressed, repeat, self.modifiers, menu) {
            Taken::No => false,
            Taken::Yes => true,
            Taken::Touch => {
                if let Some(flight) = &mut self.net_flight {
                    flight.idle.keyed();
                }
                true
            }
        }
    }

    /// A controller or key action the controls' profile resolved while the
    /// observer screen is up: a pilot command (a button) is queued for the
    /// frame's controls, as the flight queues it.
    pub(crate) fn away_watch_action(&mut self, action: tore_input::Action) {
        if !self.away_watching() || self.watch_menu_open() {
            return;
        }
        if let tore_input::Action::Pilot(command) = action {
            self.input.queue(command);
        }
    }

    /// The observer screen's Stop Watching, for an away player: the aircraft
    /// is left to the AI and the game goes to its lobby, when it has a lobby
    /// screen and is not hosting (leaving the game ends a hosted one); else
    /// the aircraft is taken back. Agent decision (slice F2-O3).
    pub(crate) fn stop_away_watch(&mut self) {
        let Some(session) = &mut self.net else {
            return;
        };
        if leaves_to_lobby(session.lobby_screen, session.hosting()) {
            session.client.stop_watching();
            self.end_net_flight();
            self.message("You left your aircraft to the AI.");
        } else {
            session.client.back();
            log::info!("Network: Stop Watching; taking the aircraft back from the AI");
            self.away_say("Taking your aircraft back from the AI...");
        }
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

    fn sensors() -> tore_sim::sensors::Controls {
        tore_sim::sensors::Controls::default()
    }

    /// The controls of one frame on the observer screen.
    fn frame_of(input: &mut Input) -> Controls {
        watch_frame(input, &BTreeSet::new(), 0.5, sensors())
    }

    fn key(input: &mut Input, name: &str, pressed: bool) -> Taken {
        watch_key(input, name, pressed, false, ModifiersState::empty(), false)
    }

    fn with(modifier: ModifiersState) -> ModifiersState {
        let mut modifiers = ModifiersState::empty();
        modifiers.set(modifier, true);
        modifiers
    }

    #[test]
    fn the_flights_primary_keys_are_flight_inputs_on_the_observer_screen() {
        let mut input = Input::new(None, false).unwrap();
        for name in [
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "z",
            "x",
            "End",
            "PageDown",
            "Space",
            "1",
            "5",
            "8",
        ] {
            assert_eq!(key(&mut input, name, true), Taken::Touch, "{name}");
            // The viewer sees nothing more of it; the release is the
            // viewer's, which ignores it.
            assert_eq!(key(&mut input, name, false), Taken::No, "{name}");
        }
        // A held key repeating is still a flight input.
        let repeat = |input: &mut Input, name| {
            watch_key(input, name, true, true, ModifiersState::empty(), false)
        };
        assert_eq!(repeat(&mut input, "ArrowLeft"), Taken::Touch);
        // Shift makes the arrows look, which is not a flight input.
        let shift = with(ModifiersState::SHIFT);
        assert_eq!(
            watch_key(&mut input, "ArrowLeft", true, false, shift, false),
            Taken::No
        );
        assert!(!flight_key("9", ModifiersState::empty()));
        assert!(!flight_key("0", ModifiersState::empty()));
    }

    #[test]
    fn the_viewers_own_keys_stay_the_viewers() {
        let mut input = Input::new(None, false).unwrap();
        for name in [
            "Tab", "Escape", "Home", "t", "f", "g", "h", "o", "r", "F1", "p", "m", "w", "a", "e",
            "PageUp", "j", "k", "l", "9", "=",
        ] {
            assert_eq!(key(&mut input, name, true), Taken::No, "{name}");
        }
        // A menu of the viewer's takes every press, flight keys included.
        let menu = |input: &mut Input, name| {
            watch_key(input, name, true, false, ModifiersState::empty(), true)
        };
        assert_eq!(menu(&mut input, "ArrowUp"), Taken::No);
        assert_eq!(menu(&mut input, "Space"), Taken::No);
        // So do the window's own shortcuts: Alt and the system key, and a
        // Control chord is not the stick.
        for modifier in [ModifiersState::ALT, ModifiersState::SUPER] {
            let held = with(modifier);
            assert_eq!(
                watch_key(&mut input, "ArrowUp", true, false, held, false),
                Taken::No
            );
        }
        let control = with(ModifiersState::CONTROL);
        assert_eq!(
            watch_key(&mut input, "ArrowUp", true, false, control, false),
            Taken::No
        );
    }

    #[test]
    fn a_key_the_profile_binds_is_the_flights_and_its_release_lands() {
        let folder = crate::replay::tests::TempDir::new("away-watch-profile");
        let path = folder.path().join("controls.conf");
        std::fs::write(&path, "tore-input 1\nbind keyboard g gear press\n").unwrap();
        let mut input = Input::new(Some(&path), false).unwrap();
        // G is bound: a flight input, and the poll hands its command over.
        assert_eq!(key(&mut input, "g", true), Taken::Touch);
        let (actions, _, _) = input.poll();
        let commands: Vec<_> = actions
            .into_iter()
            .filter_map(|action| match action {
                tore_input::Action::Pilot(command) => Some(command),
                _ => None,
            })
            .collect();
        assert!(!commands.is_empty(), "the key made no pilot command");
        // The screen queues it for the frame, as the flight does.
        for command in commands {
            input.queue(command);
        }
        let frame = frame_of(&mut input);
        assert!(!frame.pilot.commands.is_empty());
        assert!(touched(&frame, None));
        assert!(!touched(&frame_of(&mut input), None), "taken once");
        // The profile holds the key: its release is the flight's, even
        // with a menu of the viewer's open by then; a key it does not hold
        // is the viewer's.
        assert_eq!(
            watch_key(&mut input, "g", false, false, ModifiersState::empty(), true),
            Taken::Yes
        );
        assert_eq!(key(&mut input, "g", false), Taken::No);
    }

    #[test]
    fn a_key_goes_into_the_next_frame_not_held_and_then_is_spent() {
        let start = Instant::now();
        let neutral = controls();
        let mut idle = Idle::default();
        let away = |now, held| Frame {
            away: true,
            seated: false,
            ..frame(now, held, &neutral)
        };
        // A key goes down with a menu up: no input, and not remembered.
        idle.keyed();
        assert_eq!(idle.step(away(start, true)), Act::Nothing);
        assert_eq!(idle.step(away(start, false)), Act::Nothing);
        // A key with the view free: Back, once.
        idle.keyed();
        assert_eq!(idle.step(away(start, false)), Act::Back);
        assert_eq!(idle.step(away(start, false)), Act::Nothing);
    }

    #[test]
    fn a_controllers_stick_and_trigger_are_read_with_no_key() {
        let folder = crate::replay::tests::TempDir::new("away-watch-stick");
        let path = folder.path().join("controls.conf");
        std::fs::write(
            &path,
            "tore-input 1\nbind stick x roll axis\nbind stick trigger fire hold\n",
        )
        .unwrap();
        let mut input = Input::new(Some(&path), false).unwrap();
        let send = |input: &mut Input, control: &str, value: f64, baseline: bool| {
            input.resolver.event(tore_input::Event {
                device: "stick".into(),
                control: control.into(),
                value,
                baseline,
            });
        };
        send(&mut input, "x", 0., true);
        send(&mut input, "trigger", 0., true);
        assert!(!touched(&frame_of(&mut input), None));
        // A stick pushed over is a flight input; so is the trigger.
        send(&mut input, "x", 0.8, false);
        let pushed = frame_of(&mut input);
        assert!(pushed.pilot.roll.abs() > STICK_TOUCH, "{:?}", pushed.pilot);
        assert!(touched(&pushed, None));
        send(&mut input, "x", 0., false);
        assert!(!touched(&frame_of(&mut input), None));
        send(&mut input, "trigger", 1., false);
        let fire = frame_of(&mut input);
        assert!(fire.trigger && touched(&fire, None));
    }

    #[test]
    fn back_is_said_once_for_a_handoff() {
        let start = Instant::now();
        let neutral = controls();
        let mut idle = Idle::default();
        let away = Frame {
            away: true,
            seated: false,
            ..frame(start, false, &neutral)
        };
        idle.step(away);
        assert!(idle.first_back());
        idle.step(away);
        assert!(!idle.first_back());
        // Flying again, and away another time: said again.
        idle.step(frame(start, false, &neutral));
        idle.step(away);
        assert!(idle.first_back());
    }

    #[test]
    fn stop_watching_leaves_to_the_lobby_or_takes_the_aircraft_back() {
        assert!(leaves_to_lobby(true, false));
        assert!(!leaves_to_lobby(true, true), "leaving ends a hosted game");
        assert!(!leaves_to_lobby(false, false), "no lobby to go to");
        assert!(!leaves_to_lobby(false, true));
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
