//! Waiting for a real-time deadline, for a loop that drives a host on a fixed
//! clock: the dedicated server's main loop and the game's host thread (moved
//! here from `tore-server` in slice EF3, unchanged).
//!
//! Sleeping is coarse: on Windows a sleep of a millisecond can last 15, so the
//! wait sleeps until a little before the deadline and spins for the rest. The
//! host catches up any tick a late wake-up missed (agent decision; the margins
//! are fitted). Slice EF-X measured the wait on the CI runners: on Linux and
//! Windows it wakes for a 120 Hz tick a few microseconds late, at most 1.2 ms;
//! on macOS, whose timer coalescing lets a background process's sleep slip by
//! up to 75 ms, 1 to 8 ms late on average and up to 36 ms (docs/ARCHITECTURE.md,
//! "Sleep and wait accuracy on each system").

use std::time::Duration;

use crate::datagram::RealClock;

/// How long before a deadline the wait stops sleeping and spins. Windows'
/// default timer resolution is about 15.6 ms and the standard library cannot
/// raise it, so there the spin is long enough to ride out most of an
/// overshoot; elsewhere a sleep overshoots by a fraction of a millisecond.
#[cfg(windows)]
pub const SPIN_MARGIN: Duration = Duration::from_millis(2);
#[cfg(not(windows))]
pub const SPIN_MARGIN: Duration = Duration::from_micros(400);

/// The longest a host's loop goes without polling the host, its sockets and
/// its commands.
pub const MAX_NAP: Duration = Duration::from_millis(4);

/// Time and sleeping, so tests can fake both.
pub trait Sleep {
    /// Time since the clock was made.
    fn now(&self) -> Duration;
    /// Sleeps at least `duration`, and possibly longer.
    fn sleep(&mut self, duration: Duration);
    /// Gives the processor a hint while spinning.
    fn spin(&mut self) {
        std::hint::spin_loop();
    }
}

impl Sleep for RealClock {
    fn now(&self) -> Duration {
        RealClock::now(self)
    }
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Waits until `deadline` on `clock`: sleeps until `spin_margin` before it,
/// then spins.
pub fn wait_until<S: Sleep + ?Sized>(clock: &mut S, deadline: Duration, spin_margin: Duration) {
    loop {
        let now = clock.now();
        let Some(remaining) = deadline.checked_sub(now).filter(|d| !d.is_zero()) else {
            return;
        };
        if remaining > spin_margin {
            clock.sleep(remaining - spin_margin);
        } else {
            clock.spin();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock whose sleeps take exactly as long as asked plus an overshoot,
    /// and whose spins take a microsecond each.
    #[derive(Default)]
    struct Fake {
        now: Duration,
        overshoot: Duration,
        sleeps: Vec<Duration>,
        spins: u32,
    }

    impl Sleep for Fake {
        fn now(&self) -> Duration {
            self.now
        }
        fn sleep(&mut self, duration: Duration) {
            self.sleeps.push(duration);
            self.now += duration + self.overshoot;
        }
        fn spin(&mut self) {
            self.spins += 1;
            self.now += Duration::from_micros(1);
        }
    }

    #[test]
    fn the_wait_sleeps_most_of_the_way_then_spins() {
        let mut clock = Fake::default();
        wait_until(
            &mut clock,
            Duration::from_millis(8),
            Duration::from_micros(400),
        );
        assert_eq!(clock.sleeps, [Duration::from_micros(7_600)]);
        assert_eq!(clock.spins, 400);
        assert_eq!(clock.now, Duration::from_millis(8));
    }

    #[test]
    fn a_late_sleep_does_not_loop_and_a_past_deadline_returns_at_once() {
        let mut clock = Fake {
            overshoot: Duration::from_millis(15),
            ..Default::default()
        };
        wait_until(
            &mut clock,
            Duration::from_millis(8),
            Duration::from_millis(2),
        );
        assert_eq!((clock.sleeps.len(), clock.spins), (1, 0));
        let before = clock.now;
        wait_until(
            &mut clock,
            Duration::from_millis(1),
            Duration::from_millis(2),
        );
        assert_eq!(clock.now, before);
    }

    #[test]
    fn the_real_clock_waits_at_least_until_the_deadline() {
        let mut clock = RealClock::new();
        let deadline = clock.now() + Duration::from_millis(3);
        wait_until(&mut clock, deadline, SPIN_MARGIN);
        assert!(clock.now() >= deadline);
    }
}
