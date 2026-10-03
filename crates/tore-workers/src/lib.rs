//! Scoped CPU work with ordered results and one shared production pool.
//!
//! Jobs may borrow their inputs and must be independent: they cannot rely on
//! which item finishes first. Collection results always retain input order.
//! Callers publish those results in their existing deterministic order.
//!
//! [`shared`] reads `TORE_WORKERS` once. Zero keeps work on the caller; the
//! default is available logical CPUs minus three, capped at eight workers,
//! with serial execution when that leaves fewer than two workers.
//! This is scheduling headroom, not a reservation of cores. Tests construct
//! their own [`Executor`] instead of changing the process environment.

use rayon::prelude::*;
use std::fmt;
use std::sync::OnceLock;

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

#[derive(Debug)]
enum Mode {
    Serial,
    Parallel(rayon::ThreadPool),
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
        let pool = rayon::ThreadPoolBuilder::new()
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
            .map_err(|error| BuildError(error.to_string()))?;
        Ok(Self {
            mode: Mode::Parallel(pool),
        })
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
        match self.mode {
            Mode::Serial => false,
            Mode::Parallel(_) => items >= min_items.max(2),
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
        match &self.mode {
            Mode::Parallel(pool) => pool.install(|| {
                items
                    .par_iter()
                    .enumerate()
                    .map(|(index, item)| map(index, item))
                    .collect()
            }),
            #[cfg(any(test, feature = "test-support"))]
            Mode::Shuffled(seed) => shuffled_map(items.iter().enumerate().collect(), *seed, map),
            Mode::Serial => unreachable!("serial executor never dispatches"),
        }
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
        match &self.mode {
            Mode::Parallel(pool) => pool.install(|| {
                items
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(index, item)| apply(index, item));
            }),
            #[cfg(any(test, feature = "test-support"))]
            Mode::Shuffled(seed) => {
                let mut jobs: Vec<_> = items.iter_mut().enumerate().collect();
                shuffle(&mut jobs, *seed);
                for (index, item) in jobs {
                    apply(index, item);
                }
            }
            Mode::Serial => unreachable!("serial executor never dispatches"),
        }
    }
}

/// One pool per process. Initialization is silent on success and happens on
/// first use. A bad setting or an OS thread-creation failure is diagnosed on
/// stderr and selects serial execution, preserving the game's availability.
pub fn shared() -> &'static Executor {
    static EXECUTOR: OnceLock<Executor> = OnceLock::new();
    EXECUTOR.get_or_init(|| {
        let available = std::thread::available_parallelism().map_or(1, usize::from);
        let setting = match std::env::var("TORE_WORKERS") {
            Ok(setting) => Some(setting),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                eprintln!("tore-workers: TORE_WORKERS is not valid text; using serial execution");
                return Executor::serial();
            }
        };
        let workers = match configured_workers(setting.as_deref(), available) {
            Ok(workers) => workers,
            Err(why) => {
                eprintln!("tore-workers: {why}; using serial execution");
                return Executor::serial();
            }
        };
        finish_startup(Executor::parallel(workers))
    })
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

fn finish_startup(result: Result<Executor, BuildError>) -> Executor {
    match result {
        Ok(executor) => executor,
        Err(why) => {
            eprintln!("tore-workers: could not start workers: {why}; using serial execution");
            Executor::serial()
        }
    }
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
