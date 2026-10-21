//! Load-aware dispatch: when the shared executor may fan out.
//!
//! The simulation is cheap enough to run on one thread at 1x, and fanning a
//! tick out to a pool costs more CPU than it saves wall time while the loop
//! keeps up (measured for slice S9: eight workers added about a quarter more
//! CPU for a tenth less wall time on a 15 against 15 fight, and a dedicated
//! server flying 12 AI woke 7 times as often). So the shared executor stays
//! serial until the loop reports pressure, and fans out to at most
//! [`FAN_OUT_WORKERS`] workers for [`KEEP_UP`] after the last report of it.
//!
//! Pressure is any of: time compression above 1x, a tick that cost more than
//! its [`TICK_BUDGET`], or a loop that is late (the caller says what late
//! means for its loop). The loops report once per frame or batch of ticks
//! through [`observe`]. Only the dispatch changes: every job is independent
//! and results are published in input order, so a run is bit for bit the same
//! whichever way each collection was dispatched.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// The simulation tick budget at 120 Hz.
pub const TICK_BUDGET: Duration = Duration::from_nanos(8_333_333);

/// How long after the last report of pressure the executor stays fanned out.
/// A second of keeping up returns it to serial; the wait is the hysteresis
/// that stops a loop hovering at its budget from flapping every frame.
pub const KEEP_UP: Duration = Duration::from_secs(1);

/// The most workers the load-aware executor uses.
pub const FAN_OUT_WORKERS: usize = 4;

/// What one frame or batch of ticks of a loop tells the governor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    /// The time compression the loop runs at (1 is real time).
    pub compression: f64,
    /// What the most expensive tick (or the average tick of the batch) cost
    /// in wall time.
    pub tick_cost: Duration,
    /// The loop is behind: more than a tick is owed, or a frame ran long.
    /// Each loop defines this for itself.
    pub late: bool,
}

impl Load {
    /// A loop at 1x with cheap ticks that is on time.
    pub const CALM: Self = Self {
        compression: 1.,
        tick_cost: Duration::ZERO,
        late: false,
    };

    /// Whether this report asks for the workers.
    pub fn pressed(&self) -> bool {
        self.compression > 1. + 1e-9 || self.tick_cost > TICK_BUDGET || self.late
    }
}

/// The fan-out state: the moment (nanoseconds after the governor's start)
/// until which workers may be used. Lock free; reports and queries come from
/// any thread.
#[derive(Debug)]
pub struct Governor {
    epoch: Instant,
    /// Zero until the first report of pressure.
    fan_until: AtomicU64,
}

impl Governor {
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
            fan_until: AtomicU64::new(0),
        }
    }

    /// Records a loop's report now.
    pub fn observe(&self, load: Load) {
        self.observe_at(self.epoch.elapsed(), load);
    }

    /// [`Self::observe`] at `now` after the governor's start.
    pub fn observe_at(&self, now: Duration, load: Load) {
        if load.pressed() {
            let until = u64::try_from((now + KEEP_UP).as_nanos()).unwrap_or(u64::MAX);
            self.fan_until.fetch_max(until, Ordering::Relaxed);
        }
    }

    /// Whether collections may fan out now.
    pub fn fans_out(&self) -> bool {
        // The common case, a loop that never reported pressure, needs no clock.
        self.fan_until.load(Ordering::Relaxed) != 0 && self.fans_out_at(self.epoch.elapsed())
    }

    /// [`Self::fans_out`] at `now` after the governor's start.
    pub fn fans_out_at(&self, now: Duration) -> bool {
        let until = self.fan_until.load(Ordering::Relaxed);
        u64::try_from(now.as_nanos()).unwrap_or(u64::MAX) < until
    }

    /// Back to serial at once, as after a second of keeping up.
    pub fn release(&self) {
        self.fan_until.store(0, Ordering::Relaxed);
    }
}

impl Default for Governor {
    fn default() -> Self {
        Self::new()
    }
}

/// The process's governor, the one [`crate::shared`] consults.
pub fn governor() -> &'static Arc<Governor> {
    static GOVERNOR: OnceLock<Arc<Governor>> = OnceLock::new();
    GOVERNOR.get_or_init(|| Arc::new(Governor::new()))
}

/// Reports a loop's load to the process's governor.
pub fn observe(load: Load) {
    governor().observe(load);
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn calm_loops_never_fan_out() {
        let governor = Governor::new();
        for step in 0..100 {
            governor.observe_at(ms(step * 8), Load::CALM);
            assert!(!governor.fans_out_at(ms(step * 8)));
        }
        assert!(!governor.fans_out());
    }

    #[test]
    fn each_kind_of_pressure_asks_for_workers() {
        let compressed = Load {
            compression: 2.,
            ..Load::CALM
        };
        let slow_tick = Load {
            tick_cost: TICK_BUDGET + Duration::from_micros(1),
            ..Load::CALM
        };
        let late = Load {
            late: true,
            ..Load::CALM
        };
        for load in [compressed, slow_tick, late] {
            assert!(load.pressed());
            let governor = Governor::new();
            governor.observe_at(ms(500), load);
            assert!(governor.fans_out_at(ms(500)));
        }
        // At the edges: exactly 1x and exactly the budget are not pressure.
        assert!(
            !Load {
                compression: 1.,
                tick_cost: TICK_BUDGET,
                late: false
            }
            .pressed()
        );
        // Slower than real time is not pressure either.
        assert!(
            !Load {
                compression: 0.5,
                ..Load::CALM
            }
            .pressed()
        );
    }

    #[test]
    fn it_returns_to_serial_after_a_second_of_keeping_up() {
        let governor = Governor::new();
        let late = Load {
            late: true,
            ..Load::CALM
        };
        governor.observe_at(ms(100), late);
        assert!(governor.fans_out_at(ms(101)));
        // Calm reports do not shorten the hold.
        for t in (101..1_100).step_by(8) {
            governor.observe_at(ms(t), Load::CALM);
            assert!(governor.fans_out_at(ms(t)), "still fanned at {t} ms");
        }
        assert!(!governor.fans_out_at(ms(1_100)), "a second after the last");
        // Pressure again extends it from the new report, never shortens it.
        governor.observe_at(ms(2_000), late);
        governor.observe_at(ms(1_500), late);
        assert!(governor.fans_out_at(ms(2_900)));
        assert!(!governor.fans_out_at(ms(3_000)));
    }

    #[test]
    fn a_hovering_loop_does_not_flap_every_frame() {
        let governor = Governor::new();
        let mut changes = 0;
        let mut fanned = false;
        // Pressure on every fifth frame of 16 ms for ten seconds.
        for frame in 0..625u64 {
            let now = ms(frame * 16);
            let load = Load {
                late: frame % 5 == 0,
                ..Load::CALM
            };
            governor.observe_at(now, load);
            let now_fanned = governor.fans_out_at(now);
            changes += usize::from(now_fanned != fanned);
            fanned = now_fanned;
        }
        assert_eq!(changes, 1, "fanned once and stayed");
    }

    #[test]
    fn release_goes_serial_at_once() {
        let governor = Governor::new();
        governor.observe(Load {
            compression: 8.,
            ..Load::CALM
        });
        assert!(governor.fans_out());
        governor.release();
        assert!(!governor.fans_out());
    }
}
