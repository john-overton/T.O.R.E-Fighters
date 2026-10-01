//! The host's tick cost on a 15 against 15 open mission with nobody
//! connected, against the dedicated server's budget of 20 percent of one core
//! (docs/ARCHITECTURE.md, the D7 row of "How stage D lands").
//!
//! Reads a real import through `TORE_DATA_DIR`, starts a host at once
//! (`start now`) and drives its clock tick by tick for ten minutes of
//! simulated time, then prints the mean and longest tick and the load. A
//! second test steps the same mission's `World` alone, for comparison. Run
//! them in a release build:
//!
//! ```sh
//! TORE_DATA_DIR=$PWD/.local/mpb-data-host cargo test --release -p tore-session \
//!     --test host_load -- --ignored --nocapture
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use tore_formats::aircraft::AircraftId;
use tore_net::Entropy;
use tore_session::{BuildId, Host, HostConfig, HostLog, Phase, StartMode};
use tore_world::mission::{MissionSpec, Skill, Start};

const MINUTES: u64 = 10;

/// Five aircraft in each of the six wings: F/A-18Ds against MiG-29s, 10 nm
/// apart at 10,000 feet.
fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new("UKR", AircraftId::F18);
    for (index, wing) in spec.wings.iter_mut().enumerate() {
        wing.count = 5;
        wing.skill = Skill::Average;
        if index >= 3 {
            wing.aircraft = AircraftId::Mig29;
        }
    }
    spec.separation_nm = 10;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; run by hand in release"]
fn a_15_against_15_mission_with_nobody_connected_fits_the_budget() {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let config = HostConfig {
        start: StartMode::Now,
        entropy: Entropy::Seeded(1),
        ..HostConfig::new(BuildId {
            version: "test".into(),
            commit: "test".into(),
            release: false,
        })
    };
    let mut host = Host::new(spec(), Arc::new(resources), config).expect("the host starts");
    println!(
        "planes {}, theater UKR, 10 nm apart, {MINUTES} minutes, nobody connected",
        host.world().roster.planes().len()
    );
    let ticks = MINUTES * 60 * 120;
    let started = Instant::now();
    let mut minute = Vec::new();
    for tick in 0..ticks {
        // Each update is due exactly one tick later.
        let now = Duration::from_nanos((u128::from(tick) * 1_000_000_000).div_ceil(120) as u64);
        host.update(now);
        if (tick + 1) % (60 * 120) == 0 {
            let status = host.status(now);
            minute.push(status);
        }
    }
    let wall = started.elapsed();
    assert_eq!(host.world().tick(), ticks);
    assert_eq!(host.phase(), Phase::Flying);
    let logs: Vec<HostLog> = std::iter::from_fn(|| host.poll_log()).collect();
    assert!(
        !logs
            .iter()
            .any(|l| matches!(l, HostLog::Fault { .. } | HostLog::Overloaded { .. })),
        "{logs:?}"
    );
    for (index, status) in minute.iter().enumerate() {
        println!(
            "minute {:2}: mean {:>7.3} ms, longest {:>7.3} ms, load {:>5.1}%",
            index + 1,
            status.tick_cost_mean.as_secs_f64() * 1e3,
            status.tick_cost_max.as_secs_f64() * 1e3,
            status.load * 100.
        );
    }
    let mean = wall.as_secs_f64() / ticks as f64;
    let load = mean * 120.;
    let longest = minute
        .iter()
        .map(|s| s.tick_cost_max)
        .max()
        .unwrap_or_default();
    println!(
        "whole run: {ticks} ticks in {:.1} s, {:.3} ms a tick, load {:.1}% of one core, \
         longest tick {:.3} ms",
        wall.as_secs_f64(),
        mean * 1e3,
        load * 100.,
        longest.as_secs_f64() * 1e3
    );
    assert!(load < 0.2, "load {:.1}% over the 20% budget", load * 100.);
}

#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; run by hand in release"]
fn the_same_mission_stepped_alone_for_comparison() {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let mut world =
        tore_world::world::World::new(&spec(), &resources, tore_world::world::Seating::Open)
            .expect("the mission builds");
    let mut out = tore_world::world::TickOutput::default();
    for minute in 0..MINUTES {
        let started = Instant::now();
        let mut longest = Duration::ZERO;
        for _ in 0..60 * 120 {
            let tick = Instant::now();
            world.step(&[], &mut out).expect("the tick steps");
            longest = longest.max(tick.elapsed());
        }
        let mean = started.elapsed().as_secs_f64() / (60. * 120.);
        println!(
            "world alone, minute {:2}: mean {:>7.3} ms, longest {:>7.3} ms, load {:>5.1}%",
            minute + 1,
            mean * 1e3,
            longest.as_secs_f64() * 1e3,
            mean * 120. * 100.
        );
    }
}
