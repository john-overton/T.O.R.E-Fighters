use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;

fn executors() -> Vec<Executor> {
    let mut modes: Vec<_> = [0, 1, 2, 4, 8]
        .into_iter()
        .map(|workers| Executor::parallel(workers).unwrap())
        .collect();
    modes.extend([Executor::shuffled(0), Executor::shuffled(17)]);
    modes
}

#[test]
fn borrowed_maps_preserve_item_order_and_float_bits_in_every_mode() {
    let items = ["first", "second", "third", "fourth", "fifth"];
    let offset = 0.123_456_789_f64;
    let expected: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item, ((index as f64 + offset).sin()).to_bits()))
        .collect();
    for executor in executors() {
        // Results borrow the original items as well as reading stack data.
        let actual = executor.ordered_map(&items, 2, |index, item| {
            (item, ((index as f64 + offset).sin()).to_bits())
        });
        assert_eq!(actual, expected);
        let empty: Vec<usize> = executor.ordered_map(&[], 0, |_, item: &usize| *item);
        assert!(empty.is_empty());
    }
}

#[test]
fn disjoint_mutable_jobs_update_each_borrowed_item_once() {
    for executor in executors() {
        let mut items: Vec<_> = (0..64).collect();
        executor.for_each_mut(&mut items, 2, |index, item| *item += index * 2 + 7);
        assert_eq!(
            items,
            (0..64).map(|index| index * 3 + 7).collect::<Vec<_>>()
        );
        executor.for_each_mut::<usize, _>(&mut [], 0, |_, _| panic!("empty slice"));
    }
}

#[test]
fn shuffled_schedules_are_repeatable_and_not_input_order() {
    let items: Vec<_> = (0..32).collect();
    let run = |seed| {
        let visited = Mutex::new(Vec::new());
        let executor = Executor::shuffled(seed);
        assert!(executor.should_dispatch(items.len(), usize::MAX));
        let output = executor.ordered_map(&items, usize::MAX, |index, item| {
            visited.lock().unwrap().push(index);
            *item
        });
        assert_eq!(output, items);
        visited.into_inner().unwrap()
    };
    let first = run(0);
    assert_eq!(first, run(0));
    assert_ne!(first, items);
    assert_ne!(first, run(17));
    let mut sorted = first;
    sorted.sort_unstable();
    assert_eq!(sorted, items);
}

#[test]
fn thresholds_run_on_the_caller_and_explicit_worker_paths_really_dispatch() {
    let caller = thread::current().id();
    for count in [0, 1, 2, 4, 8] {
        let executor = Executor::parallel(count).unwrap();
        assert_eq!(worker_count(&executor), count);
        assert!(!executor.should_dispatch(1, 0));
        assert!(!executor.should_dispatch(2, 3));
        let inline = executor.ordered_map(&[1, 2], 3, |_, _| thread::current().id());
        assert_eq!(inline, [caller, caller]);
        let mut mutable_ids = [caller; 2];
        executor.for_each_mut(&mut mutable_ids, 3, |_, id| *id = thread::current().id());
        assert_eq!(mutable_ids, [caller; 2]);
        executor.for_each_mut(&mut mutable_ids, 2, |_, id| *id = thread::current().id());
        for id in mutable_ids {
            if count == 0 {
                assert_eq!(id, caller);
            } else {
                assert_ne!(id, caller);
            }
        }
        let actual = executor.ordered_map(&[1, 2], 2, |_, _| {
            let thread = thread::current();
            (thread.id(), thread.name().unwrap_or_default().to_owned())
        });
        for (id, name) in actual {
            if count == 0 {
                assert_eq!(id, caller);
            } else {
                assert_ne!(id, caller);
                assert!(name.starts_with("tore-worker-"));
            }
        }
    }
}

#[test]
fn nested_collections_complete_with_one_worker() {
    let (send, receive) = mpsc::channel();
    let thread = thread::spawn(move || {
        let executor = Executor::parallel(1).unwrap();
        let inputs = [1, 2, 3, 4];
        let result = executor.ordered_map(&inputs, 2, |_, outer| {
            let mut products = inputs;
            executor.for_each_mut(&mut products, 2, |_, inner| *inner *= outer);
            executor
                .ordered_map(&products, 2, |_, product| *product)
                .into_iter()
                .sum::<u32>()
        });
        send.send(result).unwrap();
    });
    let result = receive
        .recv_timeout(Duration::from_secs(10))
        .expect("nested work deadlocked on a single worker");
    assert_eq!(result, [10, 20, 30, 40]);
    thread.join().unwrap();
}

#[test]
fn independent_callers_can_share_one_pool() {
    for count in [1, 2, 4, 8] {
        let executor = Executor::parallel(count).unwrap();
        thread::scope(|scope| {
            for caller in 0..4 {
                let executor = &executor;
                scope.spawn(move || {
                    let inputs: Vec<_> = (0..32).collect();
                    for pass in 0..32 {
                        let mut items = inputs.clone();
                        executor
                            .for_each_mut(&mut items, 2, |_, value| *value = *value * 3 + caller);
                        let actual = executor.ordered_map(&items, 2, |_, value| (*value, pass));
                        let expected: Vec<_> = inputs
                            .iter()
                            .map(|value| (value * 3 + caller, pass))
                            .collect();
                        assert_eq!(actual, expected);
                    }
                });
            }
        });
    }
}

/// Borrowed output and in-flight work both release this guard on unwind.
struct ActiveJob<'a>(&'a AtomicUsize);

impl<'a> ActiveJob<'a> {
    fn enter(active: &'a AtomicUsize) -> Self {
        active.fetch_add(1, Ordering::SeqCst);
        Self(active)
    }
}

impl Drop for ActiveJob<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[test]
fn collections_finish_borrowed_work_before_panics_propagate() {
    for executor in executors() {
        let active = AtomicUsize::new(0);
        let mut items: Vec<_> = (0..32).collect();
        let result = catch_unwind(AssertUnwindSafe(|| {
            executor.ordered_map(&items, 2, |index, _| {
                let guard = ActiveJob::enter(&active);
                assert_ne!(index, 7, "map job failed");
                thread::yield_now();
                guard
            })
        }));
        assert!(result.is_err());
        assert_eq!(
            active.load(Ordering::SeqCst),
            0,
            "in-flight jobs and partial results must be dropped"
        );

        let result = catch_unwind(AssertUnwindSafe(|| {
            executor.for_each_mut(&mut items, 2, |index, value| {
                let _guard = ActiveJob::enter(&active);
                assert_ne!(index, 7, "mutable job failed");
                thread::yield_now();
                *value += 1;
            });
        }));
        assert!(result.is_err());
        assert_eq!(
            active.load(Ordering::SeqCst),
            0,
            "no borrowed job may outlive the call"
        );

        // A panicked job does not poison the shared execution machinery.
        assert_eq!(
            executor.ordered_map(&[3, 5], 2, |_, value| value * 2),
            [6, 10]
        );
    }
}

fn worker_count(executor: &Executor) -> usize {
    match &executor.mode {
        Mode::Parallel(pool) => pool.current_num_threads(),
        Mode::Lazy(lazy) => lazy
            .pool
            .get()
            .and_then(Option::as_ref)
            .map_or(0, rayon::ThreadPool::current_num_threads),
        _ => 0,
    }
}

fn pool_is_built(executor: &Executor) -> bool {
    matches!(&executor.mode, Mode::Lazy(lazy) if lazy.pool.get().is_some())
}

#[test]
fn configuration_reserves_headroom_caps_threads_and_honors_zero() {
    for (cpus, expected) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 2),
        (8, 5),
        (24, 8),
    ] {
        assert_eq!(configured_workers(None, cpus), Ok(expected));
    }
    for count in [0, 1, 2, 4, 8] {
        assert_eq!(configured_workers(Some(&count.to_string()), 24), Ok(count));
    }
    assert_eq!(configured_workers(Some("1000"), 1), Ok(MAX_WORKERS));
    for invalid in ["", "-1", "1.5", "workers", "99999999999999999999999999999"] {
        assert!(configured_workers(Some(invalid), 24).is_err());
    }
    let executor = Executor::parallel(usize::MAX).unwrap();
    assert_eq!(worker_count(&executor), MAX_WORKERS);
}

#[test]
fn a_refused_pool_build_keeps_jobs_inline_without_losing_results() {
    let executor = Executor {
        mode: Mode::Lazy(Lazy {
            workers: 2,
            gate: None,
            pool: OnceLock::from(None),
        }),
    };
    assert!(!executor.should_dispatch(8, 2));
    assert_eq!(
        executor.ordered_map(&[3, 5], 2, |_, value| value * 2),
        [6, 10]
    );
    let mut items = [1, 2, 3];
    executor.for_each_mut(&mut items, 2, |_, value| *value += 1);
    assert_eq!(items, [2, 3, 4]);
}

#[test]
fn a_lazy_pool_starts_no_threads_until_a_collection_dispatches() {
    let caller = thread::current().id();
    let executor = Executor::lazy(3);
    assert!(!pool_is_built(&executor));
    // Small and below-threshold collections stay inline and build nothing.
    assert!(!executor.should_dispatch(1, 0));
    assert_eq!(
        executor.ordered_map(&[1], 0, |_, _| thread::current().id()),
        [caller]
    );
    assert_eq!(
        executor.ordered_map(&[1, 2], 3, |_, _| thread::current().id()),
        [caller, caller]
    );
    assert!(!pool_is_built(&executor));
    let ids = executor.ordered_map(&[1, 2, 3, 4], 2, |_, _| thread::current().id());
    assert!(ids.iter().all(|id| *id != caller));
    assert!(pool_is_built(&executor));
    assert_eq!(worker_count(&executor), 3);
    // Zero workers is plain serial, with no pool to build.
    assert!(matches!(Executor::lazy(0).mode, Mode::Serial));
    assert_eq!(worker_count(&Executor::lazy(usize::MAX)), 0);
    assert!(matches!(
        &Executor::lazy(usize::MAX).mode,
        Mode::Lazy(lazy) if lazy.workers == MAX_WORKERS
    ));
}

#[test]
fn a_governed_executor_dispatches_only_while_the_governor_says() {
    let caller = thread::current().id();
    let governor = Arc::new(Governor::new());
    let executor = Executor::governed(4, Arc::clone(&governor));
    let items: Vec<_> = (0..16).collect();
    let on_caller = |executor: &Executor| {
        executor
            .ordered_map(&items, 2, |_, _| thread::current().id())
            .iter()
            .all(|id| *id == caller)
    };
    // Calm: inline, and no thread was ever started.
    assert!(on_caller(&executor));
    assert!(!executor.should_dispatch(16, 2));
    assert!(!pool_is_built(&executor));
    governor.observe(Load::CALM);
    assert!(on_caller(&executor) && !pool_is_built(&executor));
    // Pressure from any one of the signals fans out, to the asked-for size.
    governor.observe(Load {
        compression: 2.,
        ..Load::CALM
    });
    assert!(executor.should_dispatch(16, 2));
    assert!(!on_caller(&executor));
    assert_eq!(worker_count(&executor), 4);
    // Back to serial when the hold ends; the pool stays parked, unused.
    governor.release();
    assert!(on_caller(&executor));
    assert_eq!(worker_count(&executor), 4);
}

#[test]
fn toggling_dispatch_between_collections_never_changes_a_result() {
    let governor = Arc::new(Governor::new());
    let executor = Executor::governed(4, Arc::clone(&governor));
    let serial = Executor::serial();
    let mut toggled: Vec<u64> = (0..257).collect();
    let mut reference = toggled.clone();
    let mut seen = Vec::new();
    for pass in 0..40u64 {
        if pass % 3 == 0 {
            governor.observe(Load {
                late: true,
                ..Load::CALM
            });
        } else if pass % 3 == 1 {
            governor.release();
        }
        let step = |index: usize, value: &u64| {
            (value
                .wrapping_mul(6_364_136_223_846_793_005)
                .rotate_left((index % 61) as u32)
                ^ pass)
                .wrapping_add(
                    f64::from_bits(*value | 0x3ff0_0000_0000_0000)
                        .sin()
                        .to_bits(),
                )
        };
        let next = executor.ordered_map(&toggled, 2, step);
        assert_eq!(next, serial.ordered_map(&reference, 2, step), "pass {pass}");
        executor.for_each_mut(&mut toggled, 2, |index, value| *value = step(index, value));
        serial.for_each_mut(&mut reference, 2, |index, value| {
            *value = step(index, value)
        });
        assert_eq!(toggled, reference, "pass {pass}");
        seen.push(executor.should_dispatch(toggled.len(), 2));
    }
    assert!(
        seen.contains(&true) && seen.contains(&false),
        "both paths ran"
    );
}

#[test]
fn the_shared_executor_is_forced_or_governed_by_the_setting() {
    let governor = Arc::new(Governor::new());
    let forced = shared_for(
        Sizing {
            workers: 6,
            forced: true,
        },
        &governor,
    );
    assert!(matches!(&forced.mode, Mode::Lazy(lazy) if lazy.workers == 6 && lazy.gate.is_none()));
    let off = shared_for(
        Sizing {
            workers: 0,
            forced: true,
        },
        &governor,
    );
    assert!(matches!(off.mode, Mode::Serial));
    for (available, expected) in [(24, 4), (8, 4), (7, 4), (5, 2)] {
        let workers = configured_workers(None, available).unwrap();
        let auto = shared_for(
            Sizing {
                workers,
                forced: false,
            },
            &governor,
        );
        assert!(
            matches!(&auto.mode, Mode::Lazy(lazy) if lazy.workers == expected && lazy.gate.is_some()),
            "{available} cpus"
        );
        assert!(!pool_is_built(&auto), "no threads until they are wanted");
    }
    let small = shared_for(
        Sizing {
            workers: configured_workers(None, 4).unwrap(),
            forced: false,
        },
        &governor,
    );
    assert!(matches!(small.mode, Mode::Serial), "four cpus stay serial");
}

#[test]
fn shared_returns_the_same_executor_to_simultaneous_callers() {
    let address = shared() as *const Executor as usize;
    thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(move || assert_eq!(shared() as *const Executor as usize, address));
        }
    });
}
