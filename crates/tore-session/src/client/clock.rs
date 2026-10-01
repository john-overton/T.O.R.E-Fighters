//! The client's two clocks (docs/MULTIPLAYER.md, "Netcode numbers"):
//!
//! - [`InputClock`], the predicted tick: ahead of the host by the input
//!   margin, steered between 0.98 and 1.02 of real time from the margins the
//!   host reports, jumping only when more than 250 ms off.
//! - [`RenderClock`], the host tick the picture shows: behind the newest
//!   snapshot by the adaptive interpolation delay.
//!
//! Both read no clock: the caller passes the time in. Times are in ticks of
//! 1/120 s as `f64`.

use std::collections::VecDeque;
use std::time::Duration;

/// Ticks a second.
pub const TICKS_PER_SECOND: f64 = 120.;
/// The fastest and slowest the input clock runs, as a fraction of real time.
pub const RATE_LIMIT: f64 = 0.02;
/// How much the rate changes for each tick of margin error (agent decision:
/// two ticks off runs the clock at its limit).
pub const RATE_GAIN: f64 = 0.01;
/// The clock jumps when it is more than this far off (250 ms).
pub const JUMP_TICKS: f64 = 30.;
/// The window the smallest input margin is taken over.
pub const MARGIN_WINDOW: Duration = Duration::from_secs(2);
/// The window loss is judged over for the margins.
pub const LOSS_WINDOW: Duration = Duration::from_secs(10);
/// Loss above this adds one input interval to the input margin and raises
/// the interpolation margin.
pub const HIGH_LOSS: f64 = 0.01;
/// The interpolation delay: starts at 100 ms, adapts between 50 and 250 ms.
pub const DELAY_START: f64 = 12.;
pub const DELAY_MIN: f64 = 6.;
pub const DELAY_MAX: f64 = 30.;
/// The drawn time stays at least this many ticks behind the newest
/// snapshot, or [`INTERPOLATION_MARGIN_LOSSY`] while loss is high.
pub const INTERPOLATION_MARGIN: f64 = 2.;
pub const INTERPOLATION_MARGIN_LOSSY: f64 = 6.;
/// A delay changes by at most this many ticks for each tick of real time:
/// a tenth of real time.
pub const DELAY_SLIDE: f64 = 0.1;
/// How far a new snapshot moves the estimate of when snapshots arrive (agent
/// decision: a time constant of about 20 snapshots).
pub const ARRIVAL_GAIN: f64 = 0.05;

/// Real time in ticks.
pub fn ticks_of(time: Duration) -> f64 {
    time.as_secs_f64() * TICKS_PER_SECOND
}

/// The smallest value seen over a sliding window of time.
#[derive(Clone, Debug, Default)]
struct MinWindow {
    samples: VecDeque<(Duration, f64)>,
}

impl MinWindow {
    fn push(&mut self, now: Duration, value: f64, window: Duration) {
        while self.samples.back().is_some_and(|(_, v)| *v >= value) {
            self.samples.pop_back();
        }
        self.samples.push_back((now, value));
        self.prune(now, window);
    }

    fn prune(&mut self, now: Duration, window: Duration) {
        while self
            .samples
            .front()
            .is_some_and(|(at, _)| now.saturating_sub(*at) > window)
        {
            self.samples.pop_front();
        }
    }

    fn min(&self) -> Option<f64> {
        self.samples.front().map(|(_, v)| *v)
    }

    fn clear(&mut self) {
        self.samples.clear();
    }
}

/// Lost over lost plus delivered over a sliding window of time.
#[derive(Clone, Debug, Default)]
pub struct LossWindow {
    events: VecDeque<(Duration, u32, u32)>,
}

impl LossWindow {
    /// `lost` and `delivered` judged at `now`.
    pub fn note(&mut self, now: Duration, lost: u32, delivered: u32) {
        self.events.push_back((now, lost, delivered));
        self.prune(now);
    }

    fn prune(&mut self, now: Duration) {
        while self
            .events
            .front()
            .is_some_and(|(at, ..)| now.saturating_sub(*at) > LOSS_WINDOW)
        {
            self.events.pop_front();
        }
    }

    /// The fraction lost over the last 10 seconds; 0 before anything was
    /// judged.
    pub fn loss(&mut self, now: Duration) -> f64 {
        self.prune(now);
        let (lost, delivered) = self.events.iter().fold((0u64, 0u64), |(l, d), (_, a, b)| {
            (l + u64::from(*a), d + u64::from(*b))
        });
        if lost + delivered == 0 {
            0.
        } else {
            lost as f64 / (lost + delivered) as f64
        }
    }
}

/// The predicted tick's clock.
#[derive(Clone, Debug)]
pub struct InputClock {
    /// The clock, in ticks: the client steps every tick up to its whole part.
    position: f64,
    last: Option<Duration>,
    /// How far steering has moved the clock from real time since it started
    /// (jumps included), in ticks.
    steering: f64,
    /// The steering at past times, to judge each margin against the clock
    /// that sent the inputs it measures.
    history: VecDeque<(Duration, f64)>,
    /// Margins less the steering of their time.
    margins: MinWindow,
    rate: f64,
    jumps: u64,
}

impl Default for InputClock {
    fn default() -> Self {
        Self::new(0.)
    }
}

impl InputClock {
    /// A clock at `position`, running at real time.
    pub fn new(position: f64) -> Self {
        Self {
            position,
            last: None,
            steering: 0.,
            history: VecDeque::new(),
            margins: MinWindow::default(),
            rate: 1.,
            jumps: 0,
        }
    }

    /// Starts over at `position` at `now`, forgetting the margins.
    pub fn reset(&mut self, now: Duration, position: f64) {
        *self = Self::new(position);
        self.last = Some(now);
    }

    /// The clock's position in ticks.
    pub fn position(&self) -> f64 {
        self.position
    }

    /// The rate the clock runs at, as a fraction of real time.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Times the clock jumped.
    pub fn jumps(&self) -> u64 {
        self.jumps
    }

    /// Moves the clock on to `now` at its rate.
    pub fn advance(&mut self, now: Duration) {
        let last = *self.last.get_or_insert(now);
        if now > last {
            let dt = ticks_of(now - last);
            self.position += dt * self.rate;
            self.steering += dt * (self.rate - 1.);
            self.last = Some(now);
        }
        self.history.push_back((now, self.steering));
        while self.history.len() > 2
            && self
                .history
                .get(1)
                .is_some_and(|(at, _)| now.saturating_sub(*at) > MARGIN_WINDOW + LOSS_WINDOW)
        {
            self.history.pop_front();
        }
    }

    /// The steering at `time`, or the oldest known.
    fn steering_at(&self, time: Duration) -> f64 {
        self.history
            .iter()
            .rev()
            .find(|(at, _)| *at <= time)
            .or(self.history.front())
            .map_or(self.steering, |(_, s)| *s)
    }

    /// A snapshot that arrived at `now` reported the inputs' `margin` in
    /// ticks; the inputs it measures left about a `round_trip` before.
    pub fn margin(&mut self, now: Duration, margin: f64, round_trip: Duration) {
        let then = self.steering_at(now.saturating_sub(round_trip));
        self.margins.push(now, margin - then, MARGIN_WINDOW);
    }

    /// The smallest margin of the last two seconds as the clock stands now.
    pub fn smallest_margin(&mut self, now: Duration) -> Option<f64> {
        self.margins.prune(now, MARGIN_WINDOW);
        self.margins.min().map(|m| m + self.steering)
    }

    /// Sets the clock so the smallest margin is `target`, whatever the
    /// error: the first report after seating, since seating snaps anyway.
    /// Returns the jump, if any was needed.
    pub fn settle(&mut self, now: Duration, target: f64) -> Option<f64> {
        let margin = self.smallest_margin(now)?;
        let error = target - margin;
        self.margins.clear();
        if error.abs() < 1. {
            return None;
        }
        self.position += error;
        self.steering += error;
        self.jumps += 1;
        Some(error)
    }

    /// Steers towards a margin of `target` ticks: the rate within 2 percent
    /// of real time, or a jump when more than 250 ms off. Returns the jump.
    pub fn steer(&mut self, now: Duration, target: f64) -> Option<f64> {
        let Some(margin) = self.smallest_margin(now) else {
            self.rate = 1.;
            return None;
        };
        let error = target - margin;
        if error.abs() > JUMP_TICKS {
            self.position += error;
            self.steering += error;
            self.rate = 1.;
            self.jumps += 1;
            self.margins.clear();
            return Some(error);
        }
        self.rate = 1. + (error * RATE_GAIN).clamp(-RATE_LIMIT, RATE_LIMIT);
        None
    }
}

/// The host tick the picture shows.
#[derive(Clone, Debug)]
pub struct RenderClock {
    /// The newest snapshot tick is about `now` in ticks plus this.
    offset: Option<f64>,
    newest: Option<u32>,
    delay: f64,
    target: f64,
    /// The newest snapshot tick less the arrival estimate, over 2 seconds:
    /// the drawn time's lead over it is this plus the delay.
    slack: MinWindow,
    last: Option<Duration>,
    raised: Duration,
    render: Option<f64>,
}

impl Default for RenderClock {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderClock {
    /// No snapshot yet, the delay at 100 ms.
    pub fn new() -> Self {
        Self {
            offset: None,
            newest: None,
            delay: DELAY_START,
            target: DELAY_START,
            slack: MinWindow::default(),
            last: None,
            raised: Duration::ZERO,
            render: None,
        }
    }

    /// The interpolation delay now, ticks.
    pub fn delay(&self) -> f64 {
        self.delay
    }

    /// The newest snapshot tick received.
    pub fn newest(&self) -> Option<u32> {
        self.newest
    }

    /// The tick the picture shows, once a snapshot arrived.
    pub fn render(&self) -> Option<f64> {
        self.render
    }

    /// A snapshot of `tick` arrived at `now`.
    pub fn snapshot(&mut self, now: Duration, tick: u32) {
        if self.newest.is_some_and(|newest| tick <= newest) {
            return;
        }
        self.newest = Some(tick);
        let sample = f64::from(tick) - ticks_of(now);
        self.offset = Some(match self.offset {
            Some(offset) if (sample - offset).abs() <= JUMP_TICKS => {
                offset + (sample - offset) * ARRIVAL_GAIN
            }
            _ => sample,
        });
        if self.render.is_none() {
            self.last = Some(now);
            self.raised = now;
        }
        self.advance(now, INTERPOLATION_MARGIN);
    }

    /// Moves the drawn time on to `now`, adapting the delay to keep it
    /// `margin` ticks behind the newest snapshot.
    pub fn advance(&mut self, now: Duration, margin: f64) {
        let (Some(offset), Some(newest)) = (self.offset, self.newest) else {
            return;
        };
        let last = *self.last.get_or_insert(now);
        let dt = ticks_of(now.saturating_sub(last));
        self.last = Some(now.max(last));
        let lead = f64::from(newest) - (ticks_of(now) + offset);
        self.slack.push(now, lead, MARGIN_WINDOW);
        let needed = (margin - self.slack.min().unwrap_or(lead)).clamp(DELAY_MIN, DELAY_MAX);
        if needed > self.target {
            self.target = needed;
            self.raised = now;
        } else if needed + 1. < self.target && now.saturating_sub(self.raised) >= MARGIN_WINDOW {
            self.target = (needed + 0.5).clamp(DELAY_MIN, DELAY_MAX);
            self.raised = now;
        }
        let slide = dt * DELAY_SLIDE;
        self.delay += (self.target - self.delay).clamp(-slide, slide);
        let render = ticks_of(now) + offset - self.delay;
        self.render = Some(match self.render {
            // The drawn time never steps back a little; a big step back (a
            // new mission's clock) is taken.
            Some(previous) if render < previous && previous - render <= JUMP_TICKS => previous,
            _ => render,
        });
    }
}

/// An entity's extra delay, for the ones the host sends twice a second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExtraDelay {
    pub ticks: f64,
}

impl ExtraDelay {
    /// Slides towards `target` over `dt` ticks of real time, at a tenth of
    /// real time.
    pub fn slide(&mut self, target: f64, dt: f64) {
        let step = dt * DELAY_SLIDE;
        self.ticks += (target - self.ticks).clamp(-step, step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn the_input_clock_settles_on_its_margin_within_its_rate_limits() {
        // A host that steps at real time; inputs reach it 50 ms after they
        // leave, so a clock at position P sends tick P, which arrives when
        // the host's next tick is (t + 50 ms) in ticks.
        let mut clock = InputClock::default();
        clock.reset(Duration::ZERO, 20.);
        let mut snapshots = 0;
        let mut margins = Vec::new();
        for ms in 0..20_000u64 {
            let now = Duration::from_millis(ms);
            clock.advance(now);
            assert!((0.98..=1.02).contains(&clock.rate()));
            if ms % 33 == 0 && ms > 100 {
                // The margin of an input sent 100 ms ago (round trip).
                let sent = now - 100 * MS;
                let position_then = clock.position() - ticks_of(100 * MS) * clock.rate();
                let margin = (position_then - ticks_of(sent + 50 * MS)).floor();
                clock.margin(now, margin, 100 * MS);
                clock.steer(now, 3.);
                snapshots += 1;
                if ms > 15_000 {
                    margins.push(margin);
                }
            }
        }
        assert!(snapshots > 500);
        let low = margins.iter().copied().fold(f64::MAX, f64::min);
        let high = margins.iter().copied().fold(f64::MIN, f64::max);
        assert!((2. ..=5.).contains(&low), "settled margin {low} to {high}");
        assert!(high <= 6., "settled margin {low} to {high}");
    }

    #[test]
    fn the_input_clock_jumps_only_past_250_ms() {
        let mut clock = InputClock::default();
        clock.reset(Duration::ZERO, 100.);
        clock.advance(10 * MS);
        clock.margin(10 * MS, -20., Duration::ZERO);
        assert_eq!(clock.steer(10 * MS, 3.), None);
        assert!((clock.rate() - 1.02).abs() < 1e-12);
        clock.margin(20 * MS, -40., Duration::ZERO);
        assert_eq!(clock.steer(20 * MS, 3.), Some(43.));
        assert_eq!(clock.jumps(), 1);
    }

    #[test]
    fn the_render_clock_keeps_its_margin_behind_the_newest_snapshot() {
        // Snapshots every 4 ticks, each arriving 40 ms after its tick plus
        // 0 to 16 ms, so some overtake others.
        let mut arrivals: Vec<(u64, u32)> = (0..300u32)
            .map(|n| {
                let tick = n * 4;
                let sent_ms = u64::from(tick) * 1000 / 120;
                (sent_ms + 40 + u64::from(n * 7919 % 17), tick)
            })
            .collect();
        arrivals.sort();
        let mut clock = RenderClock::new();
        let mut worst = f64::MAX;
        let mut delays = Vec::new();
        let mut next = 0;
        for ms in 0..10_000u64 {
            let now = Duration::from_millis(ms);
            while next < arrivals.len() && arrivals[next].0 <= ms {
                clock.snapshot(now, arrivals[next].1);
                next += 1;
            }
            if next == arrivals.len() {
                break;
            }
            clock.advance(now, INTERPOLATION_MARGIN);
            if ms > 3000 {
                let render = clock.render().unwrap();
                worst = worst.min(f64::from(clock.newest().unwrap()) - render);
                delays.push(clock.delay());
            }
        }
        assert!(
            worst >= INTERPOLATION_MARGIN - 1.5,
            "drawn time came within {worst} ticks of the newest snapshot"
        );
        let delay = delays.last().copied().unwrap();
        assert!((DELAY_MIN..=DELAY_MAX).contains(&delay), "{delay}");
    }

    #[test]
    fn extra_delays_slide_at_a_tenth_of_real_time() {
        let mut extra = ExtraDelay::default();
        extra.slide(60., 120.);
        assert!((extra.ticks - 12.).abs() < 1e-12);
        extra.slide(60., 1000.);
        assert_eq!(extra.ticks, 60.);
    }

    #[test]
    fn loss_is_judged_over_ten_seconds() {
        let mut loss = LossWindow::default();
        loss.note(Duration::ZERO, 1, 9);
        assert!((loss.loss(Duration::from_secs(1)) - 0.1).abs() < 1e-12);
        loss.note(Duration::from_secs(5), 0, 10);
        assert!((loss.loss(Duration::from_secs(9)) - 0.05).abs() < 1e-12);
        assert_eq!(loss.loss(Duration::from_secs(16)), 0.);
    }
}
