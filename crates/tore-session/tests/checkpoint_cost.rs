//! What exact checkpoints cost on a 15 against 15 open mission with real data
//! (docs/formats/checkpoint.md, "Measurement"; stage H): the size of each
//! section and of the whole checkpoint at the start, in the first-minute
//! furball and after five minutes; the time to write one and to restore it
//! into a world fresh from the same spec; and the catch-up, a restore plus
//! 1,200 ticks (10 seconds) of re-stepping, against the plan's 1 to 3
//! seconds. The restored world must then code to the same bytes as the
//! original flown the same 1,200 ticks: the equivalence on real data.
//!
//! It also measures what delta coding against the previous checkpoint could
//! save: the shared records (aircraft configurations, weapon and sensor
//! records), which never change, and the share of the next checkpoint's
//! 64-byte blocks found anywhere in the previous one.
//!
//! The results of slice H9 are in docs/baselines/checkpoint-2026-10-05.md.
//! Run it in a release build, several times on a loaded machine:
//!
//! ```sh
//! TORE_DATA_DIR=$PWD/.local/mpb-data-host cargo test --release --locked \
//!     -p tore-session --test checkpoint_cost -- --ignored --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use tore_formats::aircraft::AircraftId;
use tore_sim::checkpoint::{Checkpoint, Models, to_bytes};
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

/// The minimum, median and maximum of `runs` timings of `work`, in
/// milliseconds.
fn spread_ms(runs: usize, mut work: impl FnMut()) -> [f64; 3] {
    let mut times: Vec<f64> = (0..runs)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    [times[0], times[runs / 2], times[runs - 1]]
}

fn shown([low, middle, high]: [f64; 3]) -> String {
    format!("{middle:.2} ms (min {low:.2}, max {high:.2})")
}

/// The share of `next`'s 64-byte blocks (at every 64th offset) that appear
/// anywhere in `previous`: an upper bound on what a plain byte delta could
/// skip.
fn blocks_found(previous: &[u8], next: &[u8]) -> f64 {
    const BLOCK: usize = 64;
    if previous.len() < BLOCK || next.len() < BLOCK {
        return 0.;
    }
    let known: BTreeSet<&[u8]> = previous.windows(BLOCK).collect();
    let blocks: Vec<&[u8]> = next.chunks_exact(BLOCK).collect();
    let found = blocks.iter().filter(|block| known.contains(*block)).count();
    found as f64 / blocks.len() as f64
}

/// `value` coded alone, its shared records after its body.
fn coded<T: Checkpoint>(value: &T, models: &Models) -> Vec<u8> {
    let coded = to_bytes(value, models).expect("it codes");
    let mut bytes = coded.body;
    for record in coded.records {
        bytes.extend(record);
    }
    bytes
}

/// The parts of a world coded alone, by name: each combat list, the
/// aircraft's and the scenery's target rows and, for each AI actor, its sensors, controller, memory and the rest
/// of it. A part whose coding is the same at the next mark is one a delta
/// against the previous checkpoint would not resend.
/// Each part's coding and the bytes it stands for.
type Parts = BTreeMap<String, (Vec<u8>, usize)>;

/// Where the two big sections' bytes go, each part coded alone: combat's
/// smoke, contrails, aircraft and scenery rows, projectiles, effects, marks,
/// hit records, flares and chaff, debris and the kill ledger, the rest of the
/// section being mostly the rewind history (private to the state) and the
/// render history; and the AI actors' sensors (those of destroyed actors
/// apart), controllers and memories, the rest being their flights, stores,
/// orders and threat services.
fn breakdown(world: &World, combat_section: usize, ai_section: usize) -> (String, Parts) {
    let state = &world.combat.state;
    let none = Models::default();
    let mut models = Models::default();
    let actors = world
        .ai_wings
        .as_ref()
        .map_or(&[][..], |wings| wings.mission().actors());
    for actor in actors {
        models
            .insert(actor.identity().aircraft, actor.flight().import_model())
            .unwrap();
    }
    let aircraft: BTreeSet<u32> = world
        .roster
        .planes()
        .iter()
        .map(|plane| plane.id.0)
        .collect();
    let mut parts = Parts::new();
    let mut combat = vec![
        ("smoke", coded(&state.smoke, &none)),
        ("contrails", coded(&world.combat.contrails, &none)),
        ("projectiles", coded(&state.projectiles, &none)),
        ("effects", coded(&state.effects, &none)),
        ("marks", coded(&state.marks, &none)),
        ("hit records", coded(&state.history, &none)),
        ("flares and chaff", coded(&state.devices, &none)),
        ("debris", coded(&state.debris, &none)),
        ("ledger", coded(&state.ledger, &none)),
    ]
    .into_iter()
    .map(|(name, bytes)| {
        let size = bytes.len();
        parts.insert(name.to_string(), (bytes, size));
        (name, size)
    })
    .collect::<Vec<_>>();
    // The rows as the combat section codes them, each against the one
    // before: the aircraft's, and the scenery's, which never change.
    let (rows, scenery): (Vec<_>, Vec<_>) = state
        .targets
        .iter()
        .cloned()
        .partition(|row| aircraft.contains(&row.id));
    let (rows, scenery) = (coded(&rows, &none), coded(&scenery, &none));
    let (rows_size, scenery_size) = (rows.len(), scenery.len());
    parts.insert("aircraft rows".to_string(), (rows, rows_size));
    parts.insert("scenery rows".to_string(), (scenery, scenery_size));
    combat.push(("aircraft rows", rows_size));
    combat.push(("scenery rows", scenery_size));
    let listed: usize = combat.iter().map(|(_, bytes)| bytes).sum();
    combat.push(("rest", combat_section.saturating_sub(listed)));
    let mut ai = vec![
        ("sensors", 0),
        ("sensors of destroyed actors", 0),
        ("controllers", 0),
        ("memories", 0),
    ];
    let mut whole = 0;
    for actor in actors {
        let id = actor.id();
        let all = coded(actor, &models);
        let sensors = actor
            .sensors()
            .map_or_else(Vec::new, |sensors| coded(sensors, &none));
        let controller = coded(actor.controller(), &none);
        let memory = coded(actor.awareness(), &none);
        whole += all.len();
        ai[if actor.alive() { 0 } else { 1 }].1 += sensors.len();
        ai[2].1 += controller.len();
        ai[3].1 += memory.len();
        // The rest of the actor is the same when its whole coding is.
        let rest = all
            .len()
            .saturating_sub(sensors.len() + controller.len() + memory.len());
        parts.insert(format!("actor {id} rest"), (all, rest));
        for (name, bytes) in [
            ("sensors", sensors),
            ("controller", controller),
            ("memory", memory),
        ] {
            let size = bytes.len();
            parts.insert(format!("actor {id} {name}"), (bytes, size));
        }
    }
    let listed: usize = ai.iter().map(|(_, bytes)| bytes).sum();
    ai.push(("rest", ai_section.saturating_sub(listed)));
    let list = |parts: &[(&str, usize)]| {
        parts
            .iter()
            .map(|(name, bytes)| format!("{name} {bytes}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let text = format!(
        "combat {combat_section}: {}; AI wings {ai_section} ({} actors, {whole} coded one by \
         one): {}",
        list(&combat),
        actors.len(),
        list(&ai)
    );
    (text, parts)
}

/// The bytes of the parts in `now` coded exactly as in `before`.
fn unchanged(before: &Parts, now: &Parts) -> usize {
    now.iter()
        .filter(|(name, (bytes, _))| before.get(*name).is_some_and(|(was, _)| was == bytes))
        .map(|(_, (_, size))| size)
        .sum()
}

/// What the mission holds at a mark, for the sizes' context.
fn census(world: &World) -> String {
    let state = &world.combat.state;
    let missiles = state
        .projectiles
        .iter()
        .filter(|p| p.guidance.is_some())
        .count();
    let actors = world
        .ai_wings
        .as_ref()
        .map_or(&[][..], |wings| wings.mission().actors());
    let alive = actors.iter().filter(|actor| actor.alive()).count();
    format!(
        "{alive} of {} AI aircraft alive, {} target rows, {} projectiles ({missiles} guided), \
         {} smoke puffs, {} contrail puffs",
        actors.len(),
        state.targets.len(),
        state.projectiles.len(),
        state.smoke.puffs.len(),
        world.combat.contrails.puffs.len()
    )
}

#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; run by hand in release"]
fn checkpoint_size_and_time_on_a_15_against_15_mission() {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let fresh = || World::new(&spec(), &resources, Seating::Open).expect("the mission builds");
    let mut world = fresh();
    println!(
        "planes {}, theater UKR, 10 nm apart, nobody connected",
        world.roster.planes().len()
    );
    let mut out = TickOutput::default();
    let mut stepped = 0;
    let mut previous: Option<(u64, Vec<u8>, Parts)> = None;
    // Every 10 seconds of the first minute's furball, then after two and
    // five minutes.
    let marks = (0..=6).map(|n| n * 1200).chain([120 * 120, 300 * 120]);
    for mark in marks {
        while stepped < mark {
            world.step(&[], &mut out).expect("the tick steps");
            stepped += 1;
        }
        println!("tick {mark}: {}", census(&world));
        let bytes = world.checkpoint().expect("a whole checkpoint");
        let layout = tore_world::checkpoint::layout(&bytes).unwrap();
        let body = |section| layout.body(&bytes, section).map_or(0, <[u8]>::len);
        let parts: Vec<String> = layout
            .sections
            .iter()
            .map(|(section, range)| format!("{} {}", section.name(), range.len()))
            .collect();
        println!(
            "tick {mark}: whole checkpoint {} bytes: shared records {} ({} records), {}",
            bytes.len(),
            layout.records_bytes,
            layout.records.len(),
            parts.join(", ")
        );
        let (text, parts) = breakdown(&world, body(Section::Combat), body(Section::AiWings));
        println!("tick {mark}: {text}");
        if let Some((tick, earlier, earlier_parts)) = &previous {
            let earlier_layout = tore_world::checkpoint::layout(earlier).unwrap();
            println!(
                "tick {mark}: against tick {tick}: shared records the same: {}; parts coded \
                 as before {} bytes; {:.1} percent of the 64-byte blocks are in the earlier \
                 checkpoint",
                earlier_layout.records == layout.records,
                unchanged(earlier_parts, &parts),
                100. * blocks_found(earlier, &bytes)
            );
        }
        let write = spread_ms(9, || {
            world.checkpoint().unwrap();
        });
        let mut restores = Vec::new();
        for _ in 0..5 {
            let mut restored = fresh();
            let started = Instant::now();
            restored.restore(&bytes).expect("the checkpoint restores");
            restores.push(started.elapsed().as_secs_f64() * 1e3);
        }
        restores.sort_by(f64::total_cmp);
        let restore = [restores[0], restores[2], restores[4]];
        // The catch-up: a fresh restore and 1,200 ticks, and the original's
        // own 1,200 ticks, which must end in the same state.
        let started = Instant::now();
        let mut restored = fresh();
        let built = started.elapsed().as_secs_f64() * 1e3;
        let started = Instant::now();
        restored.restore(&bytes).expect("the checkpoint restores");
        let mut restored_out = TickOutput::default();
        for _ in 0..1200 {
            restored
                .step(&[], &mut restored_out)
                .expect("the tick steps");
        }
        let catch_up = started.elapsed().as_secs_f64() * 1e3;
        let started = Instant::now();
        for _ in 0..1200 {
            world.step(&[], &mut out).expect("the tick steps");
        }
        stepped += 1200;
        let original = started.elapsed().as_secs_f64() * 1e3;
        assert!(
            restored.checkpoint().unwrap() == world.checkpoint().unwrap(),
            "tick {mark}: the restored world flew 1,200 ticks differently"
        );
        println!(
            "tick {mark}: written in {}, restored in {} (the fresh world built in \
             {built:.0} ms), catch-up (restore and 1,200 ticks) {catch_up:.0} ms against the \
             original's 1,200 ticks in {original:.0} ms ({:.2} ms a tick); the same state after",
            shown(write),
            shown(restore),
            original / 1200.
        );
        previous = Some((mark, bytes, parts));
    }
}
