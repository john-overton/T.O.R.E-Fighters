//! Synthetic wall-time costs of the real executor, including worker waits.
//! Run without concurrent builds or other benchmarks:
//! `cargo test --release --locked -p tore-workers --test dispatch_probe -- --ignored --nocapture`.
//! This is a portable dispatch probe, not a game or graphics benchmark. The
//! timings select candidate thresholds; they are never pass/fail assertions.

use std::hint::black_box;
use std::sync::Barrier;
use std::thread;
use std::time::{Duration, Instant};
use tore_workers::Executor;

const ITEMS: usize = 30;
const SAMPLES: usize = 128;

fn work(mut value: u64, rounds: usize) -> u64 {
    for _ in 0..rounds {
        value = black_box(value)
            .wrapping_mul(6_364_136_223_846_793_005)
            .rotate_left(17)
            .wrapping_add(1_442_695_040_888_963_407);
    }
    value
}

fn sample_map(executor: &Executor, items: &[u64], rounds: usize) -> Duration {
    let start = Instant::now();
    let result = executor.ordered_map(items, 2, |_, value| work(*value, rounds));
    let elapsed = start.elapsed();
    black_box(result);
    elapsed
}

fn report(workers: usize, case: &str, mut samples: Vec<Duration>) {
    let mean_us = samples.iter().sum::<Duration>().as_secs_f64() * 1e6 / samples.len() as f64;
    samples.sort_unstable();
    let percentile =
        |percent: usize| samples[(samples.len() - 1) * percent / 100].as_secs_f64() * 1e6;
    println!(
        "workers={workers} case={case} samples={} mean_us={mean_us:.3} \
         p50_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3}",
        samples.len(),
        percentile(50),
        percentile(95),
        percentile(99),
        percentile(100),
    );
}

fn simultaneous_callers(executor: &Executor, items: &[u64], workers: usize) {
    let ready = Barrier::new(3);
    thread::scope(|scope| {
        let host = scope.spawn(|| {
            ready.wait();
            (0..SAMPLES)
                .map(|_| sample_map(executor, items, 512))
                .collect()
        });
        let frame = scope.spawn(|| {
            ready.wait();
            (0..SAMPLES)
                .map(|_| sample_map(executor, items, 4_096))
                .collect()
        });
        ready.wait();
        report(workers, "shared_host", host.join().unwrap());
        report(workers, "shared_frame", frame.join().unwrap());
    });
}

#[test]
#[ignore = "wall timing probe; run in release without concurrent workloads"]
fn dispatch_cost_and_shared_pool_contention() {
    println!(
        "worker probe os={} arch={} logical_cpus={} items={ITEMS}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        thread::available_parallelism().map_or(1, usize::from),
    );
    let items: Vec<_> = (0..ITEMS as u64).collect();
    for workers in [0, 1, 2, 4, 8] {
        let executor = Executor::parallel(workers).unwrap();
        // Exclude thread startup from dispatch samples and verify the
        // synthetic work before timing it.
        for _ in 0..16 {
            let result = executor.ordered_map(&items, 2, |_, value| work(*value, 512));
            assert_eq!(
                result,
                items
                    .iter()
                    .map(|value| work(*value, 512))
                    .collect::<Vec<_>>()
            );
        }
        for (case, rounds) in [
            ("busy_empty", 0),
            ("busy_small", 32),
            ("isolated_host", 512),
            ("isolated_frame", 4_096),
        ] {
            let samples = (0..SAMPLES)
                .map(|_| sample_map(&executor, &items, rounds))
                .collect();
            report(workers, case, samples);
        }
        let idle = (0..64)
            .map(|_| {
                thread::sleep(Duration::from_millis(4));
                sample_map(&executor, &items, 0)
            })
            .collect();
        report(workers, "idle_empty", idle);

        simultaneous_callers(&executor, &items, workers);
    }
}
