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
        _ => 0,
    }
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
fn startup_failure_falls_back_without_losing_results() {
    let executor = finish_startup(Err(BuildError("injected thread creation refusal".into())));
    assert_eq!(worker_count(&executor), 0);
    assert_eq!(
        executor.ordered_map(&[3, 5], 2, |_, value| value * 2),
        [6, 10]
    );
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
