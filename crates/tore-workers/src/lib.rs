//! Scoped CPU work with ordered results and one shared production pool.
//!
//! Jobs may borrow their inputs and must be independent: they cannot rely on
//! which item finishes first. Collection results always retain input order.
//! Callers publish those results in their existing deterministic order.
//!
//! [`shared`] is the simulation's executor and is load-aware (slice S9): it
//! runs on the caller until a loop reports pressure through [`observe`] (time
//! compression above 1x, a tick over its budget, a loop that is late), then
//! fans out to at most four workers until a second passes without a report
//! of it. Its pool is built on first use, so a process that never needs it
//! never starts a worker thread. [`bulk`] is for one-off jobs (loading and
//! cache building), which use the whole automatic pool whenever they have
//! enough items and drop it afterwards.
//!
//! `TORE_WORKERS=N` forces a fixed count on both (zero is always serial),
//! capped at eight. Without it the automatic size is available logical CPUs
//! minus three, capped at eight, with serial execution when that leaves
//! fewer than two workers. This is scheduling headroom, not a reservation of
//! cores. Tests construct their own [`Executor`] instead of changing the
//! process environment.

mod governor;

pub use governor::{FAN_OUT_WORKERS, Governor, KEEP_UP, Load, TICK_BUDGET, governor, observe};

use rayon::prelude::*;
use std::fmt;
use std::sync::{Arc, OnceLock};

/// The fitted upper limit for one process, shared by simulation and rendering.
const MAX_WORKERS: usize = 8;

/// Failure to create an explicitly requested worker pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildError(String);

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for BuildError {}

/// A pool built when a collection first dispatches, and the gate that says
/// whether it may.
#[derive(Debug)]
struct Lazy {
    workers: usize,
    /// `None` dispatches whenever there are enough items; a governor
    /// dispatches only while it says the loop needs the workers.
    gate: Option<Arc<Governor>>,
    /// `None` inside records a refused build, after which jobs stay inline.
    pool: OnceLock<Option<rayon::ThreadPool>>,
}

#[derive(Debug)]
enum Mode {
    Serial,
    Parallel(rayon::ThreadPool),
    Lazy(Lazy),
    #[cfg(any(test, feature = "test-support"))]
    Shuffled(u64),
}

/// A scoped executor. Use [`shared`] in production and explicit instances in
/// tests. No job survives the method that started it, even during unwinding.
#[derive(Debug)]
pub struct Executor {
    mode: Mode,
}

impl Executor {
    /// Runs jobs on the caller in item order without starting any threads.
    pub const fn serial() -> Self {
        Self { mode: Mode::Serial }
    }

    /// Builds an independent pool. Zero selects serial execution; larger
    /// requests are capped at eight workers. An error is returned rather than
    /// silently disabling workers, so a test knows which path it exercised.
    pub fn parallel(workers: usize) -> Result<Self, BuildError> {
        let workers = workers.min(MAX_WORKERS);
        if workers == 0 {
            return Ok(Self::serial());
        }
        Ok(Self {
            mode: Mode::Parallel(build_pool(workers)?),
        })
    }

    /// Like [`Self::parallel`], but the pool is built when a collection first
    /// dispatches, so an executor that stays inline never starts a thread.
    /// If the pool then cannot be built, a diagnostic is printed once and
    /// jobs stay on the caller.
    pub fn lazy(workers: usize) -> Self {
        Self::lazy_gated(workers, None)
    }

    /// A lazy pool of `workers` that dispatches only while `governor` says the
    /// loops feeding it are under pressure ([`Governor::fans_out`]).
    pub fn governed(workers: usize, governor: Arc<Governor>) -> Self {
        Self::lazy_gated(workers, Some(governor))
    }

    fn lazy_gated(workers: usize, gate: Option<Arc<Governor>>) -> Self {
        let workers = workers.min(MAX_WORKERS);
        if workers == 0 {
            return Self::serial();
        }
        Self {
            mode: Mode::Lazy(Lazy {
                workers,
                gate,
                pool: OnceLock::new(),
            }),
        }
    }

    /// The pool a dispatch would use, building a lazy one first. `None` for
    /// the executors that never use one.
    fn pool(&self) -> Option<&rayon::ThreadPool> {
        match &self.mode {
            Mode::Parallel(pool) => Some(pool),
            Mode::Lazy(lazy) => lazy
                .pool
                .get_or_init(|| match build_pool(lazy.workers) {
                    Ok(pool) => Some(pool),
                    Err(why) => {
                        eprintln!("tore-workers: could not start workers: {why}; staying serial");
                        None
                    }
                })
                .as_ref(),
            Mode::Serial => None,
            #[cfg(any(test, feature = "test-support"))]
            Mode::Shuffled(_) => None,
        }
    }

    /// Executes jobs in a repeatable permuted order on the caller, while
    /// returning results in input order. This tests publication order without
    /// a process-wide override or a random dependency. It deliberately ignores
    /// collection thresholds so tests exercise preparation even for small jobs.
    #[cfg(any(test, feature = "test-support"))]
    pub const fn shuffled(seed: u64) -> Self {
        Self {
            mode: Mode::Shuffled(seed),
        }
    }

    /// Whether a collection will use its independent-job path. Callers may
    /// use this to avoid preparing owned job inputs when work stays serial.
    /// A single item cannot benefit from a parallel collection. The shuffled
    /// test executor intentionally prepares any nonempty collection.
    pub fn should_dispatch(&self, items: usize, min_items: usize) -> bool {
        match &self.mode {
            Mode::Serial => false,
            Mode::Parallel(_) => items >= min_items.max(2),
            Mode::Lazy(lazy) => {
                items >= min_items.max(2)
                    && lazy.gate.as_ref().is_none_or(|gate| gate.fans_out())
                    && self.pool().is_some()
            }
            #[cfg(any(test, feature = "test-support"))]
            Mode::Shuffled(_) => items != 0,
        }
    }

    /// Maps borrowed items, returning one result per item in input order.
    /// Below `min_items` jobs run inline; the threshold is a fitted choice
    /// owned by each caller, based on that job's measured cost.
    pub fn ordered_map<'a, T, R, F>(&self, items: &'a [T], min_items: usize, map: F) -> Vec<R>
    where
        T: Sync,
        R: Send,
        F: Fn(usize, &'a T) -> R + Sync,
    {
        if !self.should_dispatch(items.len(), min_items) {
            return items
                .iter()
                .enumerate()
                .map(|(index, item)| map(index, item))
                .collect();
        }
        if let Some(pool) = self.pool() {
            return pool.install(|| {
                items
                    .par_iter()
                    .enumerate()
                    .map(|(index, item)| map(index, item))
                    .collect()
            });
        }
        #[cfg(any(test, feature = "test-support"))]
        if let Mode::Shuffled(seed) = &self.mode {
            return shuffled_map(items.iter().enumerate().collect(), *seed, map);
        }
        unreachable!("an executor without a pool never dispatches")
    }

    /// Visits disjoint mutable items without allocating a result vector.
    pub fn for_each_mut<T, F>(&self, items: &mut [T], min_items: usize, apply: F)
    where
        T: Send,
        F: Fn(usize, &mut T) + Sync,
    {
        if !self.should_dispatch(items.len(), min_items) {
            for (index, item) in items.iter_mut().enumerate() {
                apply(index, item);
            }
            return;
        }
        if let Some(pool) = self.pool() {
            pool.install(|| {
                items
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(index, item)| apply(index, item));
            });
            return;
        }
        #[cfg(any(test, feature = "test-support"))]
        if let Mode::Shuffled(seed) = &self.mode {
            let mut jobs: Vec<_> = items.iter_mut().enumerate().collect();
            shuffle(&mut jobs, *seed);
            for (index, item) in jobs {
                apply(index, item);
            }
            return;
        }
        unreachable!("an executor without a pool never dispatches")
    }
}

fn build_pool(workers: usize) -> Result<rayon::ThreadPool, BuildError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .thread_name(|index| format!("tore-worker-{index}"))
        .start_handler(|index| {
            if let tore_realtime_native::Outcome::Failed(why) =
                tore_realtime_native::interactive_thread()
            {
                eprintln!("tore-workers: worker {index} QoS request refused: {why}");
            }
        })
        .build()
        .map_err(|error| BuildError(error.to_string()))
}

/// What `TORE_WORKERS` and the machine ask for, resolved once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sizing {
    /// The pool size for bulk jobs, and for the shared executor when forced.
    workers: usize,
    /// `TORE_WORKERS` was set: a fixed count, never load-aware.
    forced: bool,
}

fn sizing() -> Sizing {
    static SIZING: OnceLock<Sizing> = OnceLock::new();
    *SIZING.get_or_init(|| {
        let available = std::thread::available_parallelism().map_or(1, usize::from);
        let serial = Sizing {
            workers: 0,
            forced: false,
        };
        let setting = match std::env::var("TORE_WORKERS") {
            Ok(setting) => Some(setting),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                eprintln!("tore-workers: TORE_WORKERS is not valid text; using serial execution");
                return serial;
            }
        };
        match configured_workers(setting.as_deref(), available) {
            Ok(workers) => Sizing {
                workers,
                forced: setting.is_some(),
            },
            Err(why) => {
                eprintln!("tore-workers: {why}; using serial execution");
                serial
            }
        }
    })
}

/// The simulation's executor, one per process, load-aware unless
/// `TORE_WORKERS` forces a count (see the crate documentation). Nothing is
/// started or printed on success; a bad setting is diagnosed on stderr and
/// selects serial execution, preserving the game's availability.
pub fn shared() -> &'static Executor {
    static EXECUTOR: OnceLock<Executor> = OnceLock::new();
    EXECUTOR.get_or_init(|| shared_for(sizing(), governor()))
}

fn shared_for(sizing: Sizing, governor: &Arc<Governor>) -> Executor {
    if sizing.forced {
        Executor::lazy(sizing.workers)
    } else {
        Executor::governed(sizing.workers.min(FAN_OUT_WORKERS), Arc::clone(governor))
    }
}

/// An executor for a one-off bulk job such as loading or cache building,
/// where the machine is otherwise idle and the whole automatic pool helps.
/// It starts its threads when it first dispatches and stops them when it is
/// dropped; use it for the length of one job, not to keep in a struct.
/// `TORE_WORKERS` sizes it like [`shared`]; the load governor does not gate it.
pub fn bulk() -> Executor {
    Executor::lazy(sizing().workers)
}

fn configured_workers(setting: Option<&str>, available: usize) -> Result<usize, String> {
    let workers = match setting {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| "TORE_WORKERS must be a nonnegative integer".to_owned())?,
        None => match available.saturating_sub(3) {
            0 | 1 => 0,
            workers => workers,
        },
    };
    Ok(workers.min(MAX_WORKERS))
}

#[cfg(any(test, feature = "test-support"))]
fn shuffled_map<T, R, F>(mut jobs: Vec<(usize, T)>, seed: u64, map: F) -> Vec<R>
where
    F: Fn(usize, T) -> R,
{
    let mut results: Vec<_> = std::iter::repeat_with(|| None).take(jobs.len()).collect();
    shuffle(&mut jobs, seed);
    for (index, item) in jobs {
        results[index] = Some(map(index, item));
    }
    results
        .into_iter()
        .map(|result| result.expect("every shuffled item ran once"))
        .collect()
}

#[cfg(any(test, feature = "test-support"))]
fn shuffle<T>(items: &mut [T], mut seed: u64) {
    for end in (1..items.len()).rev() {
        // A local scheduler seed only. This never consumes simulation RNG.
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        items.swap(end, (seed % (end as u64 + 1)) as usize);
    }
}

#[cfg(test)]
mod tests;
