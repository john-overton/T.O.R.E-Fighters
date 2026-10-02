//! Keeping a fixed-rate loop on time where the system would let it slip: the
//! game's host thread and the dedicated server's loop, which wake at each
//! 120 Hz tick (slice EF-M).
//!
//! macOS coalesces the timers of a process it does not treat as in the
//! foreground, so their sleeps can last tens of milliseconds longer than they
//! ask: the CI runners (utility QoS), a hosting game that App Nap slows while
//! its window is hidden, a server that launchd starts. Two calls answer it,
//! both no-ops elsewhere:
//!
//! - [`real_time_thread`] gives the calling thread a Mach time-constraint
//!   policy, whose timers the kernel does not coalesce;
//! - [`Activity::begin`] holds an `NSProcessInfo` activity, latency-critical
//!   and user-initiated, so App Nap leaves the process alone while it lasts.
//!
//! See docs/ARCHITECTURE.md, "Sleep and wait accuracy on each system".
#[cfg(target_os = "macos")]
mod macos;

use std::time::Duration;

/// What a call did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It took.
    On,
    /// This system needs nothing: only macOS does anything.
    NotNeeded,
    /// It was refused, and why.
    Failed(String),
}

/// The time-constraint policy for a loop of a given period, in nanoseconds.
/// The kernel takes the three in its own time units and treats the period as
/// a hint: the loop may wake more often (the hosts wake at least every 4 ms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeConstraint {
    /// How often the loop needs the processor: one tick.
    pub period: Duration,
    /// How much of each period it may run before the scheduler may let
    /// another real-time thread in: half a tick, which covers a host's tick
    /// as measured (1.3 ms with 30 aircraft, 3.5 ms in the AI's opening
    /// fight, docs/DEDICATED-SERVER.md, "Performance") and the wait's 0.4 ms
    /// spin.
    pub computation: Duration,
    /// How soon after the period starts that work must be done: one tick,
    /// before the next is due.
    pub constraint: Duration,
}

impl TimeConstraint {
    /// The policy for a loop that wakes every `period` (agent decision; the
    /// fractions are fitted to the host's measured cost).
    pub fn for_period(period: Duration) -> Self {
        Self {
            period,
            computation: period / 2,
            constraint: period,
        }
    }
}

/// Gives the calling thread a time-constraint policy for a loop of `period`,
/// preemptible, for the rest of its life. Call it once the loop's long work
/// (building a mission) is done: a real-time thread that runs for long
/// without blocking is demoted by the kernel for a while.
pub fn real_time_thread(period: Duration) -> Outcome {
    #[cfg(target_os = "macos")]
    return match macos::set_time_constraint(TimeConstraint::for_period(period)) {
        Ok(()) => Outcome::On,
        Err(why) => Outcome::Failed(why),
    };
    #[cfg(not(target_os = "macos"))]
    {
        let _ = period;
        Outcome::NotNeeded
    }
}

/// An `NSProcessInfo` activity, latency-critical and user-initiated, held
/// until this is dropped. User-initiated keeps the Mac from idle sleep while
/// players are connected. Not `Send`: begin and drop it on one thread.
pub struct Activity {
    #[cfg(target_os = "macos")]
    _token: Option<macos::Activity>,
    outcome: Outcome,
}

impl Activity {
    /// Begins the activity; `reason` is what Activity Monitor and `pmset
    /// -g assertions` show.
    pub fn begin(reason: &str) -> Self {
        #[cfg(target_os = "macos")]
        return match macos::Activity::begin(reason) {
            Ok(token) => Self {
                _token: Some(token),
                outcome: Outcome::On,
            },
            Err(why) => Self {
                _token: None,
                outcome: Outcome::Failed(why),
            },
        };
        #[cfg(not(target_os = "macos"))]
        {
            let _ = reason;
            Self {
                outcome: Outcome::NotNeeded,
            }
        }
    }

    /// Whether it took.
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
}

/// The one line a loop logs about what it was given, or `None` where
/// nothing was needed: "macOS real-time scheduling on" when both took,
/// otherwise what did not and why.
pub fn summary(thread: &Outcome, activity: &Outcome) -> Option<String> {
    let thread_part = match thread {
        Outcome::On => Some("macOS real-time scheduling on".to_owned()),
        Outcome::Failed(why) => Some(format!("macOS real-time scheduling off: {why}")),
        Outcome::NotNeeded => None,
    };
    let activity_part = match (thread, activity) {
        (Outcome::On, Outcome::On) | (_, Outcome::NotNeeded) => None,
        (_, Outcome::On) => Some("App Nap held off".to_owned()),
        (_, Outcome::Failed(why)) => Some(format!("App Nap not held off: {why}")),
    };
    match (thread_part, activity_part) {
        (Some(thread), Some(activity)) => Some(format!("{thread}; {activity}")),
        (one, other) => one.or(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_120_hz_loop_gets_half_a_tick_of_work_due_by_the_next_tick() {
        let tick = Duration::from_secs(1) / 120;
        let policy = TimeConstraint::for_period(tick);
        assert_eq!(policy.period, Duration::from_nanos(8_333_333));
        assert_eq!(policy.computation, Duration::from_nanos(4_166_666));
        assert_eq!(policy.constraint, Duration::from_nanos(8_333_333));
    }

    #[test]
    fn the_summary_names_what_took_and_why_not() {
        let failed = || Outcome::Failed("refused (5)".into());
        assert_eq!(
            summary(&Outcome::On, &Outcome::On).as_deref(),
            Some("macOS real-time scheduling on")
        );
        assert_eq!(summary(&Outcome::NotNeeded, &Outcome::NotNeeded), None);
        assert_eq!(
            summary(&failed(), &Outcome::On).as_deref(),
            Some("macOS real-time scheduling off: refused (5); App Nap held off")
        );
        assert_eq!(
            summary(&Outcome::On, &failed()).as_deref(),
            Some("macOS real-time scheduling on; App Nap not held off: refused (5)")
        );
    }

    /// The calls take on macOS (the CI runners run this) and do nothing
    /// elsewhere.
    #[test]
    fn the_policy_and_the_activity_take_on_macos_only() {
        let (thread, activity) = std::thread::spawn(|| {
            let activity = Activity::begin("T.O.R.E-Fighters test");
            let thread = real_time_thread(Duration::from_secs(1) / 120);
            let outcome = activity.outcome().clone();
            drop(activity);
            (thread, outcome)
        })
        .join()
        .unwrap();
        let expected = if cfg!(target_os = "macos") {
            Outcome::On
        } else {
            Outcome::NotNeeded
        };
        assert_eq!((thread, activity), (expected.clone(), expected));
    }
}
