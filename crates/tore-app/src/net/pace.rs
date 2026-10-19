//! When a menu with a session open pumps the network and when it draws.
//!
//! A joined or hosted session needs a turn every 10 ms or so (keepalives,
//! timeouts, the input clock), but a menu needs a frame only when something
//! on it changed. The pump runs from the event loop's wait on the session's
//! own wake; a frame is asked for when the pump brought news, when the
//! player did something, and on a slow heartbeat for what changes by itself
//! (a countdown, a search finishing). In flight nothing here is used.
use std::time::{Duration, Instant};
use tore_session::ClientPhase;
use tore_session::wire::messages::LobbyState;

/// The longest the session waits between turns.
pub(crate) const PUMP_MAX: Duration = Duration::from_millis(10);
/// The shortest, so a session asking for no wait cannot spin the loop.
const PUMP_MIN: Duration = Duration::from_millis(1);
/// A screen with nothing happening is redrawn 10 times a second, so what
/// changes by itself (a countdown, search results) shows within 100 ms.
pub(crate) const HEARTBEAT: Duration = Duration::from_millis(100);
/// While a game is connecting or loading the bar and timers move; 20 a second.
pub(crate) const PREPARING_FRAME: Duration = Duration::from_millis(50);
/// After the player's input, frames at the display's pace for a while so
/// button rockers and check lamps (40 ms a frame) animate smoothly.
pub(crate) const BUSY_FRAME: Duration = Duration::from_millis(16);
pub(crate) const BUSY_FOR: Duration = Duration::from_millis(600);
/// After news from the pump (another player's box ticked), a shorter spell:
/// a lamp walks four frames of 40 ms.
pub(crate) const NEWS_BUSY_FOR: Duration = Duration::from_millis(250);

/// What the pump last saw of the session, so a change shows at once.
#[derive(Default)]
struct Seen {
    lobby: Option<LobbyState>,
    unable: Option<String>,
    phase: Option<ClientPhase>,
}

pub(crate) struct MenuPace {
    /// The session's next turn is due.
    pump_at: Instant,
    /// The last time a frame was asked for.
    drawn_at: Instant,
    /// Frames at the display's pace until then.
    busy_until: Instant,
    /// The pump brought news that no frame has shown yet.
    dirty: bool,
    seen: Seen,
}

impl MenuPace {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            pump_at: now,
            drawn_at: now.checked_sub(HEARTBEAT).unwrap_or(now),
            busy_until: now,
            dirty: false,
            seen: Seen::default(),
        }
    }

    /// Whether the session's turn is due.
    pub(crate) fn pump_due(&self, now: Instant) -> bool {
        now >= self.pump_at
    }

    /// The turn was taken: the next one is due when the session says
    /// (`wake`, at most [`PUMP_MAX`]). `news` is whether handling its events
    /// ended or changed anything.
    pub(crate) fn pumped(
        &mut self,
        now: Instant,
        wake: Duration,
        news: bool,
        lobby: Option<&LobbyState>,
        unable: Option<&str>,
        phase: Option<ClientPhase>,
    ) {
        self.pump_at = now + wake.clamp(PUMP_MIN, PUMP_MAX);
        let mut changed = news;
        if self.seen.lobby.as_ref() != lobby {
            self.seen.lobby = lobby.cloned();
            changed = true;
        }
        if self.seen.unable.as_deref() != unable {
            self.seen.unable = unable.map(str::to_owned);
            changed = true;
        }
        if self.seen.phase != phase {
            self.seen.phase = phase;
            changed = true;
        }
        if changed {
            // Frames at the display's pace for a moment: a lamp walks to its
            // new state (a ready box another player ticked).
            self.busy_until = self.busy_until.max(now + NEWS_BUSY_FOR);
        }
        self.dirty |= changed;
    }

    /// The player pressed, moved, scrolled or typed.
    pub(crate) fn activity(&mut self, now: Instant) {
        self.busy_until = self.busy_until.max(now + BUSY_FOR);
    }

    fn gap(&self, now: Instant, preparing: bool) -> Duration {
        if preparing {
            PREPARING_FRAME
        } else if now < self.busy_until {
            BUSY_FRAME
        } else {
            HEARTBEAT
        }
    }

    /// Whether a frame should be asked for now. Asking is recorded.
    pub(crate) fn redraw_due(&mut self, now: Instant, preparing: bool) -> bool {
        if self.dirty || now >= self.drawn_at + self.gap(now, preparing) {
            self.dirty = false;
            self.drawn_at = now;
            true
        } else {
            false
        }
    }

    /// A loading screen's frame: at [`PREPARING_FRAME`], whatever else.
    pub(crate) fn loading_redraw_due(&mut self, now: Instant) -> bool {
        if now >= self.drawn_at + PREPARING_FRAME {
            self.drawn_at = now;
            true
        } else {
            false
        }
    }

    /// The next moment a frame is due, or the pump too when `pumping`.
    pub(crate) fn next_due(&self, now: Instant, preparing: bool, pumping: bool) -> Instant {
        let frame = self.drawn_at + self.gap(now, preparing);
        if pumping {
            self.pump_at.min(frame)
        } else {
            frame
        }
    }

    /// The next moment the pump is due.
    pub(crate) fn next_pump(&self) -> Instant {
        self.pump_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby_screen::preview::sample;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn quiet(pace: &mut MenuPace, at: Instant, lobby: Option<&LobbyState>) {
        pace.pumped(at, ms(10), false, lobby, None, Some(ClientPhase::Lobby));
    }

    #[test]
    fn the_pump_runs_on_the_sessions_wake_between_1_and_10_ms() {
        let t0 = Instant::now();
        let mut pace = MenuPace::new(t0);
        assert!(pace.pump_due(t0), "the first turn is at once");
        quiet(&mut pace, t0, None);
        assert!(!pace.pump_due(t0 + ms(9)));
        assert!(pace.pump_due(t0 + ms(10)));
        pace.pumped(t0, ms(250), false, None, None, None);
        assert_eq!(pace.next_pump(), t0 + PUMP_MAX, "never later than 10 ms");
        pace.pumped(t0, Duration::ZERO, false, None, None, None);
        assert_eq!(pace.next_pump(), t0 + PUMP_MIN, "never a spin");
        pace.pumped(t0, ms(4), false, None, None, None);
        assert_eq!(pace.next_pump(), t0 + ms(4));
    }

    #[test]
    fn an_idle_menu_draws_on_a_10_hz_heartbeat_not_every_pump() {
        let t0 = Instant::now();
        let mut pace = MenuPace::new(t0);
        let lobby = sample(1);
        quiet(&mut pace, t0, Some(&lobby));
        // Seeing the lobby for the first time is news.
        assert!(pace.redraw_due(t0, false));
        // Once the lamps have settled, a second of pumps is ten frames.
        let t0 = t0 + ms(1000);
        let mut draws = 0;
        for step in 1..=100u64 {
            let now = t0 + ms(step * 10);
            quiet(&mut pace, now, Some(&lobby));
            if pace.redraw_due(now, false) {
                draws += 1;
            }
        }
        // One second of 10 ms pumps: about ten frames, not a hundred.
        assert!((9..=11).contains(&draws), "{draws} frames in a second");
    }

    #[test]
    fn a_lobby_change_an_unable_word_or_a_phase_draws_at_once() {
        let t0 = Instant::now();
        let mut pace = MenuPace::new(t0);
        let mut lobby = sample(1);
        quiet(&mut pace, t0, Some(&lobby));
        assert!(pace.redraw_due(t0, false));
        let now = t0 + ms(10);
        quiet(&mut pace, now, Some(&lobby));
        assert!(!pace.redraw_due(now, false), "nothing changed");
        // Another player's ready box.
        lobby.players[0].ready = !lobby.players[0].ready;
        let now = t0 + ms(20);
        quiet(&mut pace, now, Some(&lobby));
        assert!(pace.redraw_due(now, false));
        // The client says its game cannot play the mission.
        let now = t0 + ms(30);
        pace.pumped(
            now,
            ms(10),
            false,
            Some(&lobby),
            Some("no"),
            Some(ClientPhase::Lobby),
        );
        assert!(pace.redraw_due(now, false));
        // Handled events count even when the lobby is as it was.
        let now = t0 + ms(40);
        pace.pumped(
            now,
            ms(10),
            true,
            Some(&lobby),
            Some("no"),
            Some(ClientPhase::Lobby),
        );
        assert!(pace.redraw_due(now, false));
        // The phase moves on.
        let now = t0 + ms(50);
        pace.pumped(
            now,
            ms(10),
            false,
            Some(&lobby),
            Some("no"),
            Some(ClientPhase::Loading),
        );
        assert!(pace.redraw_due(now, false));
    }

    #[test]
    fn input_draws_at_the_display_pace_for_a_moment_then_the_heartbeat() {
        let t0 = Instant::now();
        let mut pace = MenuPace::new(t0);
        assert!(pace.redraw_due(t0, false));
        pace.activity(t0);
        assert!(!pace.redraw_due(t0 + ms(10), false));
        assert!(pace.redraw_due(t0 + ms(16), false));
        assert!(pace.redraw_due(t0 + ms(32), false));
        assert_eq!(pace.next_due(t0 + ms(32), false, false), t0 + ms(48));
        // Long after the last input the gap is the heartbeat again.
        let late = t0 + BUSY_FOR + ms(200);
        assert!(pace.redraw_due(late, false));
        assert!(!pace.redraw_due(late + ms(50), false));
        assert!(pace.redraw_due(late + HEARTBEAT, false));
    }

    #[test]
    fn connecting_and_loading_draw_at_20_hz() {
        let t0 = Instant::now();
        let mut pace = MenuPace::new(t0);
        assert!(pace.redraw_due(t0, true));
        assert!(!pace.redraw_due(t0 + ms(49), true));
        assert!(pace.redraw_due(t0 + ms(50), true));
        assert!(!pace.loading_redraw_due(t0 + ms(60)));
        assert!(pace.loading_redraw_due(t0 + ms(100)));
    }

    #[test]
    fn the_wait_is_the_earlier_of_the_pump_and_the_frame() {
        let t0 = Instant::now();
        let mut pace = MenuPace::new(t0);
        assert!(pace.redraw_due(t0, false));
        quiet(&mut pace, t0, None);
        // Past the spell of frames the first sight of the session started.
        let t0 = t0 + ms(1000);
        assert!(pace.redraw_due(t0, false));
        quiet(&mut pace, t0, None);
        assert_eq!(pace.next_due(t0, false, true), t0 + ms(10));
        // With no session the stale pump time is not waited for.
        assert_eq!(pace.next_due(t0, false, false), t0 + HEARTBEAT);
        // A pump far off (the session says so) leaves the heartbeat to wake.
        pace.pump_at = t0 + ms(500);
        assert_eq!(pace.next_due(t0, false, true), t0 + HEARTBEAT);
    }
}
