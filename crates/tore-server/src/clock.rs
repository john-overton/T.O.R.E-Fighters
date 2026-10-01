//! The real-time clock the run loop waits on. The host never reads a clock;
//! the program reads this one and hands it the time.
//!
//! The wait itself, sleeping until a little before the deadline and spinning
//! for the rest, is `tore_net::wait_until`, shared with the game's host
//! thread; this clock adds the wall clock for the log's dates.

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tore_net::{RealClock, Sleep};

pub use tore_net::{MAX_NAP, SPIN_MARGIN, wait_until};

/// Time, sleeping and the wall clock, so tests can fake all three.
pub trait Timer: Sleep {
    /// Seconds since 1970-01-01 UTC, for the log's dates.
    fn unix_seconds(&self) -> u64;
}

/// The system's clock.
pub struct RealTimer {
    clock: RealClock,
}

impl RealTimer {
    pub fn new() -> Self {
        Self {
            clock: RealClock::new(),
        }
    }
}

impl Sleep for RealTimer {
    fn now(&self) -> Duration {
        self.clock.now()
    }
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

impl Timer for RealTimer {
    fn unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    (year + i64::from(month <= 2), month, day)
}

/// `(date "YYYY-MM-DD", time "HH:MM:SS")` in UTC for a Unix time.
pub fn utc_stamp(unix_seconds: u64) -> (String, String) {
    let days = (unix_seconds / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    let in_day = unix_seconds % 86_400;
    (
        format!("{year:04}-{month:02}-{day:02}"),
        format!(
            "{:02}:{:02}:{:02}",
            in_day / 3_600,
            in_day / 60 % 60,
            in_day % 60
        ),
    )
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// A timer whose sleeps take exactly as long as asked, plus a chosen
    /// overshoot, and which notes each sleep.
    #[derive(Clone, Default)]
    pub struct FakeTimer {
        pub now: Rc<Cell<Duration>>,
        pub overshoot: Duration,
        pub unix: u64,
        pub sleeps: Rc<std::cell::RefCell<Vec<Duration>>>,
        pub spins: Rc<Cell<u32>>,
    }

    impl Sleep for FakeTimer {
        fn now(&self) -> Duration {
            self.now.get()
        }
        fn sleep(&mut self, duration: Duration) {
            self.sleeps.borrow_mut().push(duration);
            self.now.set(self.now.get() + duration + self.overshoot);
        }
        fn spin(&mut self) {
            self.spins.set(self.spins.get() + 1);
            // Each spin takes a microsecond.
            self.now.set(self.now.get() + Duration::from_micros(1));
        }
    }

    impl Timer for FakeTimer {
        fn unix_seconds(&self) -> u64 {
            self.unix + self.now.get().as_secs()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeTimer;
    use super::*;

    #[test]
    fn dates_match_known_days() {
        assert_eq!(utc_stamp(0), ("1970-01-01".into(), "00:00:00".into()));
        assert_eq!(
            utc_stamp(951_782_400 + 86_399),
            ("2000-02-29".into(), "23:59:59".into())
        );
        // 2026-09-30 12:34:56 UTC.
        assert_eq!(
            utc_stamp(1_790_771_696),
            ("2026-09-30".into(), "12:34:56".into())
        );
        assert_eq!(utc_stamp(1_798_761_599).0, "2026-12-31");
        assert_eq!(utc_stamp(1_798_761_600).0, "2027-01-01");
    }

    #[test]
    fn the_wait_sleeps_most_of_the_way_then_spins() {
        let mut timer = FakeTimer::default();
        let margin = Duration::from_micros(400);
        wait_until(&mut timer, Duration::from_millis(8), margin);
        assert_eq!(
            timer.sleeps.borrow().as_slice(),
            &[Duration::from_micros(7_600)]
        );
        assert_eq!(timer.spins.get(), 400);
        assert_eq!(timer.now.get(), Duration::from_millis(8));
    }

    #[test]
    fn a_late_sleep_does_not_loop_and_a_past_deadline_returns_at_once() {
        let mut timer = FakeTimer {
            overshoot: Duration::from_millis(15),
            ..Default::default()
        };
        wait_until(
            &mut timer,
            Duration::from_millis(8),
            Duration::from_millis(2),
        );
        assert_eq!(timer.sleeps.borrow().len(), 1);
        assert_eq!(timer.spins.get(), 0);
        let before = timer.now.get();
        wait_until(
            &mut timer,
            Duration::from_millis(1),
            Duration::from_millis(2),
        );
        assert_eq!(timer.now.get(), before);
    }

    #[test]
    fn a_short_wait_only_spins() {
        let mut timer = FakeTimer::default();
        wait_until(
            &mut timer,
            Duration::from_micros(100),
            Duration::from_micros(400),
        );
        assert!(timer.sleeps.borrow().is_empty());
        assert_eq!(timer.spins.get(), 100);
    }
}
