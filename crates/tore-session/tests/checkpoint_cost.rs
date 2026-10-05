//! What exact checkpoints cost on a 15 against 15 open mission with real data
//! (docs/formats/checkpoint.md, "Measurement"; stage H): the size of each
//! section and of the whole checkpoint at the start, in the first-minute
//! furball and after five minutes; the time to write one and to restore it
//! into a world fresh from the same spec; and the catch-up, a restore plus
//! 1,200 ticks of re-stepping, against the plan's 1 to 3 seconds.
//!
//! Until every section is coded, the whole checkpoint reports the first
//! section that is not, and the test measures the sections that are. Run it
//! in a release build on a quiet machine:
//!
//! ```sh
//! TORE_DATA_DIR=$PWD/.local/mpb-data-host cargo test --release --locked \
//!     -p tore-session --test checkpoint_cost -- --ignored --nocapture
//! ```

use std::time::Instant;
use tore_formats::aircraft::AircraftId;
use tore_sim::checkpoint::CheckpointError;
use tore_world::checkpoint::Section;
use tore_world::mission::{MissionSpec, Skill, Start};
use tore_world::world::{Seating, TickOutput, World};

/// Five aircraft in each of the six wings: F/A-18Ds against MiG-29s, 10 nm
/// apart at 10,000 feet, as `host_load` flies.
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

/// The median of `runs` timings of `work`, in milliseconds.
fn median_ms(runs: usize, mut work: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..runs)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[runs / 2]
}

#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; run by hand in release"]
fn checkpoint_size_and_time_on_a_15_against_15_mission() {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let mut world = World::new(&spec(), &resources, Seating::Open).expect("the mission builds");
    println!(
        "planes {}, theater UKR, 10 nm apart, nobody connected",
        world.roster.planes().len()
    );
    let mut out = TickOutput::default();
    let mut stepped = 0;
    for mark in [0u64, 60 * 120, 300 * 120] {
        while stepped < mark {
            world.step(&[], &mut out).expect("the tick steps");
            stepped += 1;
        }
        let mut sizes = Vec::new();
        for section in Section::ALL {
            match world.checkpoint_sections(&[section]) {
                Ok(bytes) => {
                    let layout = tore_world::checkpoint::layout(&bytes).unwrap();
                    let body = layout.sections[0].1.len();
                    sizes.push(format!(
                        "{} {body} (+{} shared)",
                        section.name(),
                        layout.records_bytes
                    ));
                }
                Err(CheckpointError::NotCovered(what)) => {
                    sizes.push(format!("{} not coded ({what})", section.name()))
                }
                Err(error) => panic!("the {} section failed: {error}", section.name()),
            }
        }
        println!("tick {mark}: {}", sizes.join(", "));
        let bytes = match world.checkpoint() {
            Ok(bytes) => bytes,
            Err(error) => {
                println!("tick {mark}: no whole checkpoint yet: {error}");
                continue;
            }
        };
        let write = median_ms(5, || {
            world.checkpoint().unwrap();
        });
        let fresh = || World::new(&spec(), &resources, Seating::Open).unwrap();
        let mut restored = fresh();
        let started = Instant::now();
        restored.restore(&bytes).expect("the checkpoint restores");
        let restore = started.elapsed().as_secs_f64() * 1e3;
        let started = Instant::now();
        for _ in 0..1200 {
            restored.step(&[], &mut out).expect("the tick steps");
        }
        let catch_up = started.elapsed().as_secs_f64() * 1e3;
        println!(
            "tick {mark}: whole checkpoint {} bytes, written in {write:.2} ms, restored in \
             {restore:.2} ms, then 1,200 ticks in {catch_up:.0} ms",
            bytes.len()
        );
    }
}
