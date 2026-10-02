//! A probe of how late sleeps and a host's 120 Hz wait wake, without and with
//! the real-time policy and the activity (slice EF-M, after EF-X's probe).
//! It asserts nothing: run it by hand or on a CI runner with
//! `cargo test --locked -p tore-realtime-native --test wait_probe -- --ignored --nocapture`.
//! The figures are in docs/ARCHITECTURE.md, "Sleep and wait accuracy on each
//! system".

use std::time::{Duration, Instant};
use tore_net::{MAX_NAP, RealClock, SPIN_MARGIN, wait_until};
use tore_realtime_native::{Activity, real_time_thread, summary};

const TICK: Duration = Duration::from_nanos(8_333_333);
const SLEEPS: u32 = 100;
const TICKS: u32 = 360;

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

/// Mean and worst of `samples`, in milliseconds.
fn spread(samples: &[Duration]) -> (f64, f64) {
    let total: Duration = samples.iter().sum();
    let worst = samples.iter().max().copied().unwrap_or_default();
    (ms(total) / samples.len() as f64, ms(worst))
}

fn sleeps(length: Duration) -> (f64, f64) {
    let taken: Vec<Duration> = (0..SLEEPS)
        .map(|_| {
            let start = Instant::now();
            std::thread::sleep(length);
            start.elapsed()
        })
        .collect();
    spread(&taken)
}

/// A host's loop: wake at each tick or after `MAX_NAP`, whichever is first,
/// through the shared wait; how late each tick is noticed.
fn ticks() -> (f64, f64) {
    let mut clock = RealClock::new();
    let start = clock.now();
    let mut late = Vec::new();
    let mut tick = 1;
    while tick <= TICKS {
        let now = clock.now();
        let due = start + TICK * tick;
        if now >= due {
            late.push(now - due);
            tick += 1;
            continue;
        }
        wait_until(&mut clock, due.min(now + MAX_NAP), SPIN_MARGIN);
    }
    spread(&late)
}

fn busy(length: Duration) {
    let start = Instant::now();
    let mut spins = 0u64;
    while start.elapsed() < length {
        spins = std::hint::black_box(spins.wrapping_add(1));
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Plain,
    ActivityOnly,
    Policy,
    PolicyAfterBurst,
}

fn probe(phase: Phase) -> String {
    std::thread::spawn(move || {
        let activity = match phase {
            Phase::Plain => None,
            _ => Some(Activity::begin("T.O.R.E-Fighters wait probe")),
        };
        let thread = match phase {
            Phase::Policy | Phase::PolicyAfterBurst => Some(real_time_thread(TICK)),
            _ => None,
        };
        if let (Phase::ActivityOnly, true) = (phase, cfg!(target_os = "macos")) {
            // The activity's power assertion, as the system lists it.
            if let Ok(out) = std::process::Command::new("pmset")
                .args(["-g", "assertions"])
                .output()
            {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    if line.contains("wait probe") {
                        println!("assertion: {}", line.trim());
                    }
                }
            }
        }
        if let Phase::PolicyAfterBurst = phase {
            // A mission rebuilt on the loop's thread: half a second without
            // blocking.
            busy(Duration::from_millis(500));
        }
        let applied = match (&thread, &activity) {
            (Some(thread), Some(activity)) => summary(thread, activity.outcome()),
            (None, Some(activity)) => summary(
                &tore_realtime_native::Outcome::NotNeeded,
                activity.outcome(),
            ),
            _ => None,
        }
        .unwrap_or_else(|| "nothing applied".into());
        let (one_mean, one_worst) = sleeps(Duration::from_millis(1));
        let (sixteen_mean, sixteen_worst) = sleeps(Duration::from_millis(16));
        let (late_mean, late_worst) = ticks();
        format!(
            "{label:<20} 1 ms sleep {one_mean:.2} / {one_worst:.2} ms | \
             16 ms sleep {sixteen_mean:.2} / {sixteen_worst:.2} ms | \
             120 Hz tick late {late_mean_us:.1} us / {late_worst:.3} ms | {applied}",
            label = match phase {
                Phase::Plain => "plain",
                Phase::ActivityOnly => "activity",
                Phase::Policy => "policy+activity",
                Phase::PolicyAfterBurst => "policy after burst",
            },
            late_mean_us = late_mean * 1e3,
        )
    })
    .join()
    .unwrap()
}

#[test]
#[ignore = "a timing probe for the CI runners; it asserts nothing"]
fn how_late_sleeps_and_the_host_wait_wake() {
    println!("wait probe on {}", std::env::consts::OS);
    for phase in [
        Phase::Plain,
        Phase::ActivityOnly,
        Phase::Policy,
        Phase::PolicyAfterBurst,
    ] {
        println!("probe: {}", probe(phase));
    }
}
