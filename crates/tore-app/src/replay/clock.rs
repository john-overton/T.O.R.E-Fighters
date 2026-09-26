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
        let delta = self.direction.sign() * self.speed() * TICKS_PER_SECOND * seconds;
        self.position = (self.position + delta).clamp(self.first as f64, self.last as f64);
        let at_end = match self.direction {
            Direction::Forward => self.position >= self.last as f64,
            Direction::Reverse => self.position <= self.first as f64,
        };
        if at_end {
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
            return;
        }
        match self.direction {
            Direction::Forward if self.position >= self.last as f64 => {
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
        self.speed = (self.speed + 1).min(SPEEDS.len() - 1);
    }

    /// Down: the next slower speed in the current direction.
    pub fn slower(&mut self) {
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
        if position.is_finite() {
            self.position = position.clamp(self.first as f64, self.last as f64);
        }
    }

    /// Home.
    pub fn start(&mut self) {
        self.seek(self.first as f64);
    }

    /// End.
    pub fn end(&mut self) {
        self.seek(self.last as f64);
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
    /// while paused.
    pub fn label(&self) -> String {
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
}
