//! The replay viewer's playback clock: where the playhead is, which way it
//! moves and how fast. The playhead is a fractional tick, so slow motion and
//! reverse draw smooth in-between pictures, and every picture is a function
//! of the playhead alone: playing backwards shows exactly what playing
//! forwards showed at the same moment.
//!
//! The speed ladder, the J/K/L video-editor keys and the doubling rules are
//! agent design choices (2026-09-26) for the replay viewer John requested on
//! 2026-09-26.

/// Simulation ticks per second of recording.
pub const TICKS_PER_SECOND: f64 = 120.;
/// Playback speeds, slowest first. The same ladder serves both directions.
pub const SPEEDS: [f64; 9] = [0.125, 0.25, 0.5, 0.75, 1., 2., 4., 8., 16.];
/// Ladder index of normal speed.
const NORMAL: usize = 4;
/// Ladder index where fast forward starts.
const FAST: usize = 5;
/// A live playhead plays this far behind the newest frame, ticks (one
/// second): the newest frames arrive in bursts, and the playhead must not
/// run dry between them.
pub const LIVE_LAG: f64 = 120.;
/// A following playhead that falls this far behind the newest frame, ticks
/// (three seconds), when frames arrive late and all at once, is brought back
/// to the live edge.
pub const LIVE_SLACK: f64 = 360.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

impl Direction {
    fn sign(self) -> f64 {
        match self {
            Self::Forward => 1.,
            Self::Reverse => -1.,
        }
    }
}

/// The playhead and how it moves.
#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    first: u64,
    last: u64,
    /// Fractional tick.
    position: f64,
    direction: Direction,
    /// Index into [`SPEEDS`].
    speed: usize,
    paused: bool,
    /// The recording grows as it plays (an observer's): its end is the
    /// newest frame, and a playhead that reaches it waits for more instead
    /// of stopping.
    live: bool,
    /// A live playhead the viewer has not moved: it stays at the live edge.
    /// Scrubbing, pausing, reversing or changing the speed clears it, and End
    /// sets it.
    follow: bool,
}

/// The ladder speed at least twice as fast as `index`, capped at the top.
fn doubled(index: usize) -> usize {
    let wanted = SPEEDS[index] * 2.;
    SPEEDS
        .iter()
        .position(|speed| *speed >= wanted)
        .unwrap_or(SPEEDS.len() - 1)
}

impl Clock {
    /// A clock over the recorded ticks `first` to `last`, playing forwards
    /// at normal speed from the start.
    pub fn new(first: u64, last: u64) -> Self {
        Self {
            first,
            last: last.max(first),
            position: first as f64,
            direction: Direction::Forward,
            speed: NORMAL,
            paused: false,
            live: false,
            follow: false,
        }
    }

    /// A clock over a recording that is still growing, playing forwards at
    /// the live edge: [`LIVE_LAG`] behind the newest frame `last`.
    pub fn new_live(first: u64, last: u64) -> Self {
        let mut clock = Self::new(first, last);
        clock.live = true;
        clock.follow = true;
        clock.position = (clock.last as f64 - LIVE_LAG).max(first as f64);
        clock
    }

    /// The recording grows as it plays.
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// A live playhead that stays at the live edge, playing forwards at
    /// normal speed: what the transport bar calls LIVE. Its speed is trimmed
    /// a little (see [`Clock::advance`]) to keep it [`LIVE_LAG`] behind the
    /// newest frame.
    pub fn following(&self) -> bool {
        self.live && self.follow && !self.paused
    }

    /// The newest frames and the oldest still kept are now `first` and
    /// `last`, in a live recording. A playhead that fell off the old end
    /// stays on the oldest frame; one that follows the live edge and has
    /// fallen more than [`LIVE_SLACK`] behind it (frames that came late, all
    /// at once) is brought back to [`LIVE_LAG`] behind.
    pub fn grow(&mut self, first: u64, last: u64) {
        self.first = first;
        self.last = last.max(first);
        self.position = self.position.clamp(self.first as f64, self.last as f64);
        if self.following() && self.last as f64 - self.position > LIVE_SLACK {
            self.position = (self.last as f64 - LIVE_LAG).max(self.first as f64);
        }
    }

    pub fn first(&self) -> u64 {
        self.first
    }

    pub fn last(&self) -> u64 {
        self.last
    }

    /// The playhead as a fractional tick.
    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    /// Current speed as a multiple of real time.
    pub fn speed(&self) -> f64 {
        SPEEDS[self.speed]
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    /// The tick whose picture is drawn: the playhead rounded up. A whole
    /// playhead shows exactly that tick.
    pub fn tick(&self) -> u64 {
        (self.position.ceil().max(0.) as u64).clamp(self.first, self.last)
    }

    /// How far the picture has moved from the previous tick towards
    /// [`Clock::tick`], in (0, 1]. 1 is exactly the tick.
    pub fn alpha(&self) -> f64 {
        (1. - (self.tick() as f64 - self.position)).clamp(0., 1.)
    }

    /// Moves the playhead by `seconds` of real time. Playback stops at
    /// either end. True when the playhead moved.
    pub fn advance(&mut self, seconds: f64) -> bool {
        if self.paused || seconds.is_nan() || seconds <= 0. {
            return false;
        }
        let before = self.position;
        let mut rate = self.speed();
        if self.following() {
            // Bursts of new frames: a little slower when the playhead runs
            // near the edge, a little faster when it has fallen behind.
            let behind = self.last as f64 - self.position;
            if behind < LIVE_LAG / 2. {
                rate *= 0.9;
            } else if behind > LIVE_LAG * 1.5 {
                rate *= 1.1;
            }
        }
        let delta = self.direction.sign() * rate * TICKS_PER_SECOND * seconds;
        self.position = (self.position + delta).clamp(self.first as f64, self.last as f64);
        let at_end = match self.direction {
            Direction::Forward => self.position >= self.last as f64,
            Direction::Reverse => self.position <= self.first as f64,
        };
        // A live recording's end is only the newest frame so far.
        if at_end && !(self.live && self.direction == Direction::Forward) {
            self.paused = true;
        }
        self.position != before
    }

    /// Space: pause, or play on in the current direction and speed. Playing
    /// from the end the playhead is heading for starts over from the other
    /// end.
    pub fn toggle(&mut self) {
        if !self.paused {
            self.paused = true;
            self.follow = false;
            return;
        }
        match self.direction {
            Direction::Forward if self.position >= self.last as f64 && !self.live => {
                self.position = self.first as f64;
            }
            Direction::Reverse if self.position <= self.first as f64 => {
                self.position = self.last as f64;
            }
            _ => {}
        }
        self.paused = false;
    }

    /// K and the pause button.
    pub fn pause(&mut self) {
        self.paused = true;
        self.follow = false;
    }

    /// The play button: forwards at normal speed.
    pub fn play(&mut self) {
        self.run(Direction::Forward, NORMAL);
    }

    /// L: forwards at normal speed; pressed again while playing forwards,
    /// twice as fast, up to 16x.
    pub fn forward(&mut self) {
        self.again(Direction::Forward, NORMAL);
    }

    /// J and the reverse button: backwards at normal speed; pressed again
    /// while playing backwards, twice as fast, up to 16x.
    pub fn reverse(&mut self) {
        self.again(Direction::Reverse, NORMAL);
    }

    /// The fast forward button: forwards at 2x; pressed again, twice as
    /// fast, up to 16x.
    pub fn fast_forward(&mut self) {
        self.again(Direction::Forward, FAST);
    }

    /// Starts playing in `direction` at ladder index `start`; pressed again
    /// while already playing that way at `start` or faster, doubles. From
    /// slow motion it returns to `start`.
    fn again(&mut self, direction: Direction, start: usize) {
        if self.paused || self.direction != direction {
            self.run(direction, start);
        } else if self.speed < start {
            self.speed = start;
        } else {
            self.speed = doubled(self.speed);
        }
    }

    fn run(&mut self, direction: Direction, speed: usize) {
        self.follow = false;
        self.direction = direction;
        self.speed = speed;
        self.paused = true;
        self.toggle();
    }

    /// Plays at `speed` times real time, backwards when negative. False,
    /// changing nothing, when the speed is not on the ladder.
    pub fn set_speed(&mut self, speed: f64) -> bool {
        let Some(index) = SPEEDS.iter().position(|s| *s == speed.abs()) else {
            return false;
        };
        let direction = if speed < 0. {
            Direction::Reverse
        } else {
            Direction::Forward
        };
        self.run(direction, index);
        true
    }

    /// Up: the next faster speed in the current direction.
    pub fn faster(&mut self) {
        self.follow = false;
        self.speed = (self.speed + 1).min(SPEEDS.len() - 1);
    }

    /// Down: the next slower speed in the current direction.
    pub fn slower(&mut self) {
        self.follow = false;
        self.speed = self.speed.saturating_sub(1);
    }

    /// Moves the playhead by `seconds` of recording, keeping play or pause.
    pub fn jump(&mut self, seconds: f64) {
        self.seek(self.position + seconds * TICKS_PER_SECOND);
    }

    /// Pauses and moves by whole ticks, landing on a tick.
    pub fn step(&mut self, ticks: i64) {
        self.paused = true;
        let from = if ticks > 0 {
            self.position.floor()
        } else {
            self.position.ceil()
        };
        self.seek(from + ticks as f64);
    }

    /// Puts the playhead at `position`, a fractional tick, within the
    /// recording.
    pub fn seek(&mut self, position: f64) {
        self.follow = false;
        if position.is_finite() {
            self.position = position.clamp(self.first as f64, self.last as f64);
        }
    }

    /// Home.
    pub fn start(&mut self) {
        self.seek(self.first as f64);
    }

    /// End. In a live recording it is the live edge: playing forwards at
    /// normal speed, [`LIVE_LAG`] behind the newest frame.
    pub fn end(&mut self) {
        if self.live {
            self.direction = Direction::Forward;
            self.speed = NORMAL;
            self.paused = false;
            self.seek((self.last as f64 - LIVE_LAG).max(self.first as f64));
            self.follow = true;
        } else {
            self.seek(self.last as f64);
        }
    }

    /// PageUp and PageDown: the previous or next timeline marker. `markers`
    /// are ticks in increasing order. While playing forwards, "previous"
    /// skips a marker passed in the last half second, so pressing it
    /// repeatedly keeps going back. False when there is no such marker.
    pub fn marker(&mut self, markers: &[u64], next: bool) -> bool {
        let found = if next {
            markers
                .iter()
                .copied()
                .find(|tick| *tick as f64 > self.position + 0.5)
        } else {
            let grace = if !self.paused && self.direction == Direction::Forward {
                TICKS_PER_SECOND / 2.
            } else {
                0.5
            };
            markers
                .iter()
                .copied()
                .rev()
                .find(|tick| (*tick as f64) < self.position - grace)
        };
        if let Some(tick) = found {
            self.seek(tick as f64);
        }
        found.is_some()
    }

    /// The speed readout: "1x", "0.25x", "Reverse 2x", with "paused" added
    /// while paused, and "LIVE" at the live edge of a growing recording.
    pub fn label(&self) -> String {
        if self.following() {
            return "LIVE".into();
        }
        let speed = SPEEDS[self.speed];
        let number = if speed == 0.125 {
            "1/8x".to_owned()
        } else {
            format!("{speed}x")
        };
        let text = match self.direction {
            Direction::Forward => number,
            Direction::Reverse => format!("Rev {number}"),
        };
        if self.paused {
            format!("{text} paused")
        } else {
            text
        }
    }
}

/// Recording time as the mission timer shows it: `mm:ss.t`, from tick 0.
pub fn timestamp(tick: f64) -> String {
    let tenths = (tick.max(0.) / TICKS_PER_SECOND * 10.).floor() as u64;
    format!(
        "{:02}:{:02}.{}",
        tenths / 600,
        tenths / 10 % 60,
        tenths % 10
    )
}

/// A tick with thousands separators, as the mission timer shows it.
pub fn grouped(tick: u64) -> String {
    let digits = tick.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_runs_from_an_eighth_to_sixteen_times() {
        let mut clock = Clock::new(0, 100_000);
        assert_eq!(clock.speed(), 1.);
        for expected in [2., 4., 8., 16., 16.] {
            clock.faster();
            assert_eq!(clock.speed(), expected);
        }
        let mut seen = Vec::new();
        for _ in 0..10 {
            clock.slower();
            seen.push(clock.speed());
        }
        assert_eq!(seen, [8., 4., 2., 1., 0.75, 0.5, 0.25, 0.125, 0.125, 0.125]);
        assert_eq!(clock.label(), "1/8x");
        clock.reverse();
        assert_eq!(clock.label(), "Rev 1x");
        clock.pause();
        assert_eq!(clock.label(), "Rev 1x paused");
    }

    #[test]
    fn pressing_play_again_doubles_up_to_sixteen_times() {
        let mut clock = Clock::new(0, 100_000);
        clock.pause();
        clock.forward();
        assert_eq!((clock.direction(), clock.speed()), (Direction::Forward, 1.));
        for expected in [2., 4., 8., 16., 16.] {
            clock.forward();
            assert_eq!(clock.speed(), expected);
        }
        // J from forward play starts reverse at 1x, then doubles.
        clock.reverse();
        assert_eq!((clock.direction(), clock.speed()), (Direction::Reverse, 1.));
        clock.reverse();
        assert_eq!(clock.speed(), 2.);
        // Fast forward starts at 2x and doubles.
        clock.fast_forward();
        assert_eq!((clock.direction(), clock.speed()), (Direction::Forward, 2.));
        clock.fast_forward();
        assert_eq!(clock.speed(), 4.);
        // From slow motion, L returns to normal speed and fast forward to 2x.
        clock.play();
        clock.slower();
        assert_eq!(clock.speed(), 0.75);
        clock.forward();
        assert_eq!(clock.speed(), 1.);
        clock.slower();
        clock.slower();
        clock.slower();
        assert_eq!(clock.speed(), 0.25);
        clock.fast_forward();
        assert_eq!(clock.speed(), 2.);
        assert_eq!(doubled(0), 1);
        assert_eq!(doubled(3), 5);
        // The play button always means forwards at 1x.
        clock.reverse();
        clock.reverse();
        clock.play();
        assert_eq!((clock.direction(), clock.speed()), (Direction::Forward, 1.));
        assert!(!clock.paused());
    }

    #[test]
    fn the_playhead_moves_at_the_chosen_speed_and_stops_at_the_ends() {
        let mut clock = Clock::new(100, 1_300);
        assert!(clock.advance(0.5));
        assert_eq!(clock.position(), 160.);
        clock.fast_forward();
        clock.fast_forward();
        clock.advance(1.);
        assert_eq!(clock.position(), 160. + 4. * 120.);
        clock.advance(10.);
        assert_eq!(clock.position(), 1_300.);
        assert!(clock.paused());
        assert!(!clock.advance(1.));
        // Play at the end starts over.
        clock.toggle();
        assert_eq!(clock.position(), 100.);
        clock.end();
        clock.reverse();
        clock.slower();
        clock.slower();
        assert_eq!(clock.speed(), 0.5);
        clock.advance(0.25);
        assert_eq!(clock.position(), 1_300. - 15.);
        assert_eq!(clock.tick(), 1_285);
        clock.advance(0.01);
        assert_eq!(clock.tick(), 1_285);
        assert!((clock.alpha() - 0.4).abs() < 1e-9);
        clock.advance(100.);
        assert_eq!((clock.position(), clock.paused()), (100., true));
        // Reverse from the start starts over from the end.
        clock.toggle();
        assert_eq!(clock.position(), 1_300.);
        assert!(!clock.advance(0.));
        assert!(!clock.advance(f64::NAN));
    }

    #[test]
    fn a_speed_can_be_chosen_directly() {
        let mut clock = Clock::new(0, 1_000);
        clock.pause();
        assert!(clock.set_speed(-16.));
        assert_eq!(
            (clock.direction(), clock.speed(), clock.paused()),
            (Direction::Reverse, 16., false)
        );
        assert!(clock.set_speed(0.125));
        assert_eq!(
            (clock.direction(), clock.speed()),
            (Direction::Forward, 0.125)
        );
        assert!(!clock.set_speed(3.));
        assert!(!clock.set_speed(f64::NAN));
        assert_eq!(clock.speed(), 0.125);
    }

    #[test]
    fn steps_land_on_whole_ticks_and_pause() {
        let mut clock = Clock::new(0, 1_000);
        clock.seek(10.4);
        assert_eq!(clock.tick(), 11);
        clock.step(1);
        assert_eq!((clock.position(), clock.paused()), (11., true));
        assert_eq!(clock.alpha(), 1.);
        clock.seek(10.4);
        clock.step(-1);
        assert_eq!(clock.position(), 10.);
        clock.step(-1);
        assert_eq!(clock.position(), 9.);
        clock.start();
        clock.step(-1);
        assert_eq!(clock.position(), 0.);
        clock.end();
        clock.step(1);
        assert_eq!(clock.position(), 1_000.);
        // Jumps keep playing or paused as they were.
        clock.play();
        clock.start();
        clock.jump(5.);
        assert_eq!((clock.position(), clock.paused()), (600., false));
        clock.jump(-30.);
        assert_eq!(clock.position(), 0.);
        clock.seek(f64::INFINITY);
        assert_eq!(clock.position(), 0.);
    }

    #[test]
    fn markers_step_back_and_forth() {
        let markers = [100, 500, 900];
        let mut clock = Clock::new(0, 1_000);
        clock.pause();
        assert!(clock.marker(&markers, true));
        assert_eq!(clock.position(), 100.);
        assert!(clock.marker(&markers, true));
        assert_eq!(clock.position(), 500.);
        assert!(clock.marker(&markers, false));
        assert_eq!(clock.position(), 100.);
        assert!(!clock.marker(&markers, false));
        assert_eq!(clock.position(), 100.);
        // Playing forwards, a marker just passed is skipped.
        clock.seek(520.);
        clock.play();
        assert!(clock.marker(&markers, false));
        assert_eq!(clock.position(), 100.);
        clock.seek(950.);
        assert!(!clock.marker(&markers, true));
        assert!(!clock.marker(&[], true));
    }

    #[test]
    fn times_read_as_minutes_seconds_and_tenths() {
        assert_eq!(timestamp(0.), "00:00.0");
        assert_eq!(timestamp(120. * 61. + 59.), "01:01.4");
        assert_eq!(timestamp(120. * 3_600.), "60:00.0");
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(86_808), "86,808");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }

    /// A live clock over a recording that grows: it starts a second behind
    /// the newest frame and keeps there.
    #[test]
    fn a_live_playhead_follows_a_growing_recording_and_never_passes_it() {
        let mut clock = Clock::new_live(1_000, 2_000);
        assert!(clock.is_live() && clock.following());
        assert_eq!(clock.position(), 2_000. - LIVE_LAG);
        assert_eq!(clock.label(), "LIVE");
        // Playing at 1x with the recording growing 1x keeps the lag.
        let mut last = 2_000;
        for _ in 0..600 {
            last += 2;
            clock.grow(1_000, last);
            assert!(clock.advance(1. / 60.));
        }
        let behind = last as f64 - clock.position();
        assert!(
            (LIVE_LAG / 2. ..=LIVE_LAG * 1.5).contains(&behind),
            "{behind}"
        );
        assert!(clock.following());
        // Frames that stop arriving: the playhead waits at the newest
        // frame and does not pause, then goes on when more come.
        for _ in 0..600 {
            clock.advance(1. / 60.);
        }
        assert_eq!(clock.position(), last as f64);
        assert!(!clock.paused());
        clock.grow(1_000, last + 240);
        assert!(clock.advance(0.5));
        assert!(clock.position() > last as f64);
        // Play from the end of a live recording does not start over.
        clock.pause();
        clock.seek(clock.last() as f64);
        clock.toggle();
        assert_eq!(clock.position(), clock.last() as f64);
    }

    /// Leaving the live edge and coming back with End.
    #[test]
    fn pausing_and_scrubbing_leave_live_and_end_returns_to_it() {
        let mut clock = Clock::new_live(0, 10_000);
        clock.pause();
        assert!(!clock.following());
        assert_eq!(clock.label(), "1x paused");
        clock.grow(0, 11_000);
        assert_eq!(clock.position(), 10_000. - LIVE_LAG);
        // Stepping back and scrubbing keep the recording growing under it.
        clock.step(-1);
        assert!(!clock.following());
        clock.seek(100.);
        clock.play();
        assert!(!clock.following());
        assert_eq!(clock.label(), "1x");
        // A fast playhead is not live even at the edge.
        clock.end();
        assert!(clock.following());
        clock.faster();
        assert!(!clock.following());
        // End: the edge again, forwards at normal speed.
        clock.reverse();
        clock.end();
        assert!(clock.following());
        assert_eq!((clock.direction(), clock.speed()), (Direction::Forward, 1.));
        assert_eq!(clock.position(), 11_000. - LIVE_LAG);
        // The oldest frames fall out of the window: a playhead behind them
        // waits at the new start.
        clock.pause();
        clock.seek(500.);
        clock.grow(2_000, 12_000);
        assert_eq!((clock.first(), clock.position()), (2_000, 2_000.));
        // A recording that is not live ends where it ends.
        let mut plain = Clock::new(0, 100);
        assert!(!plain.is_live());
        plain.seek(100.);
        plain.end();
        assert_eq!(plain.position(), 100.);
        assert!(!plain.following());
        plain.advance(1.);
        assert!(plain.paused());
    }
}
