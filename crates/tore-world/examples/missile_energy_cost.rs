//! Real headless world: eight AI aircraft, sixteen replenished missiles and
//! eight envelope searches and one to eight candidate stations per aircraft.
//! Never opens a window or socket.
use std::{path::Path, time::Instant};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    attitude::unit,
    combat::{
        FallState,
        live::{Launcher, Projectile},
        missiles::{self, Flight, LaunchMode, Motion, Rules, seeker::Observation},
    },
};
use tore_world::{
    ai_wings::Preset,
    mission::{MissionSpec, Skill, WingSpec},
    seats::{SeatId, SeatInput},
    world::{Hooks, Seating, TickOutput, World},
};

// The optional host callback keeps platform instrumentation out of simulation.
#[cfg(target_os = "linux")]
fn thread_cpu_ns() -> Option<u64> {
    let time = rustix::time::clock_gettime(rustix::time::ClockId::ThreadCPUTime);
    u64::try_from(time.tv_sec)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u64::try_from(time.tv_nsec).ok()?)
}
#[cfg(not(target_os = "linux"))]
fn thread_cpu_ns() -> Option<u64> {
    None
}
fn percentile(samples: &mut [f64], p: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[((samples.len() - 1) as f64 * p).round() as usize]
}
fn station_geometry(
    mut launcher: Launcher,
    mut observed: Observation,
    station: usize,
) -> (Launcher, Observation) {
    // Distinct synthetic mounts prevent identical-input reuse from hiding
    // the cold cost of several candidate stations on the same aircraft.
    for i in 0..3 {
        launcher.position[i] += launcher.basis.right[i] * station as f64 * 4.;
    }
    observed.range = missiles::length(missiles::sub(observed.position, launcher.position));
    (launcher, observed)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let pack = args.get(1).ok_or(
        "usage: missile_energy_cost PACK [TICKS] [THEATER] [frozen|moving] [WEAPON] [STATIONS]",
    )?;
    let ticks = args
        .get(2)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(2400u64);
    let theater = args.get(3).map_or("UKR", String::as_str);
    let stations = args
        .get(6)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(1)
        .clamp(1, 8);
    let resources = tore_import::pack::read_pack(Path::new(pack))?;
    let mut spec = MissionSpec::new(theater, AircraftId::F16C);
    let natural = args.iter().any(|s| s == "--natural");
    let big = args.iter().any(|s| s == "--big");
    spec.preset = if natural { Preset::Free } else { Preset::Hold };
    spec.wings[1] = WingSpec {
        aircraft: AircraftId::F16C,
        count: 4,
        skill: Skill::Average,
    };
    spec.wings[3] = WingSpec {
        aircraft: AircraftId::F16C,
        count: 4,
        skill: Skill::Average,
    };
    if big {
        for wing in [0usize, 2, 4, 5] {
            spec.wings[wing] = WingSpec {
                aircraft: AircraftId::F16C,
                count: 4,
                skill: Skill::Average,
            };
        }
    }
    let mut world = World::build(
        &spec,
        &resources,
        Seating::SinglePlayer,
        &mut Hooks::default(),
    )?
    .world;
    let player_position = world.cockpits[0].flight.position;
    world
        .ai_wings
        .as_mut()
        .ok_or("no AI")?
        .apply_mission_preset(
            if natural { Preset::Free } else { Preset::Hold },
            player_position,
        );
    world.combat.state.weapon_rules = if args.iter().any(|s| s == "--spec") {
        Rules::Spec
    } else {
        Rules::Energy
    };
    world.combat.state.measure_prediction_cost = true;
    if cfg!(target_os = "linux") && thread_cpu_ns().is_some() {
        world.combat.state.prediction_cpu_clock = Some(thread_cpu_ns);
    }
    world.combat.state.own_mut().armed = false;
    let slots = world.ai_wings.as_ref().ok_or("no AI")?.slots().to_vec();
    if slots.len() != 8 && !natural {
        return Err(format!("expected eight AI, got {}", slots.len()).into());
    }
    let weapon_name = args.get(5).map_or("AIM120.JT", String::as_str);
    let mut weapon = Weapon::parse(
        weapon_name,
        resources.get(weapon_name).ok_or("missing stress weapon")?,
    )?;
    // Fuze/removal are real. Damage alone is suppressed to keep all eight
    // aircraft alive throughout the cost measurement.
    weapon.damage.by_class = [0; 5];
    let profile = missiles::Profile::for_weapon(&weapon).ok_or("no profile")?;
    let mut next = 0x2000_0000u32;
    let mut out = TickOutput::default();
    let mut cold = Vec::new();
    let mut moving = Vec::new();
    let mut warm = Vec::new();
    let mut cold_cpu = Vec::new();
    let mut moving_cpu = Vec::new();
    let mut max_pending = 0;
    let mut max_envelopes = 0;
    let mut frozen = None;
    let fixed = args.get(4).is_some_and(|s| s == "frozen");
    let mut answers = 0usize;
    let mut launched = std::collections::BTreeSet::new();
    let mut max_clients = 0;
    let mut refused = 0;
    let mut starved = 0;
    let mut max_wait = 0;
    println!("tick,estimate_ms,request_ms,missiles,pending,ready_envelopes");
    for tick in 0..ticks {
        let mut launchers = Vec::new();
        for slot in &slots {
            let flight = world
                .ai_wings
                .as_mut()
                .unwrap()
                .actor_flight_mut(slot.id)
                .ok_or("AI disappeared")?;
            let launcher = tore_world::combat::launcher(flight);
            if !natural && !launcher.alive {
                return Err(format!("AI {} stopped flying at tick {tick}", slot.id).into());
            }
            launchers.push(launcher);
        }
        let queries = if fixed {
            frozen.get_or_insert_with(|| launchers.clone()).clone()
        } else {
            launchers.clone()
        };
        let start = Instant::now();
        let observations: Vec<_> = (0..slots.len())
            .map(|i| {
                let target = (i + 4) % 8;
                let mut position = queries[target].position;
                let surface = profile.role == missiles::TargetRole::Surface;
                if surface {
                    position[1] =
                        f64::from(world.terrain.height(position[0] as f32, position[2] as f32))
                            + 10.;
                }
                Observation {
                    id: if surface { u32::MAX } else { slots[target].id },
                    position,
                    velocity: if surface {
                        [0.; 3]
                    } else {
                        queries[target].velocity
                    },
                    quality: 1.,
                    range: missiles::length(missiles::sub(position, queries[i].position)),
                    off_axis: 0.,
                }
            })
            .collect();
        // Reserved probe stations avoid replacing an actor's real launch jobs.
        for (i, slot) in slots.iter().enumerate().filter(|_| !natural) {
            for station in 0..stations {
                let (launcher, observed) = station_geometry(queries[i], observations[i], station);
                world.combat.state.request_energy_solution(
                    slot.id,
                    65535 - station,
                    &weapon,
                    launcher,
                    observed,
                );
            }
            world.combat.state.request_energy_range(
                slot.id,
                65535,
                &weapon,
                queries[i],
                observations[i],
            );
        }
        let requests = start.elapsed().as_secs_f64() * 1000.;
        while !natural
            && world
                .combat
                .state
                .projectiles
                .iter()
                .filter(|p| p.id >= 0x2000_0000)
                .count()
                < 16
        {
            let i = (next as usize) % 8;
            let target = (i + 4) % 8;
            let direction = unit(missiles::sub(
                launchers[target].position,
                launchers[i].position,
            ));
            let position = std::array::from_fn(|a| launchers[i].position[a] + direction[a] * 200.);
            let observed = Observation {
                id: slots[target].id,
                position: launchers[target].position,
                velocity: launchers[target].velocity,
                quality: 1.,
                range: missiles::length(missiles::sub(launchers[target].position, position)),
                off_axis: 0.,
            };
            world.combat.state.projectiles.push(Projectile {
                id: next,
                owner: slots[i].id,
                weapon: Some(weapon.clone()),
                guidance: Some(Flight::from_supported_launch(
                    profile,
                    LaunchMode::Boresight,
                    observed,
                    position,
                )),
                motion: Some(Motion::launch_rules(
                    &weapon,
                    launchers[i].velocity,
                    position[1],
                    Rules::Energy,
                )),
                guidance_ticks: Some(profile.guidance_ticks),
                age: 0,
                incoming: None,
                station: 1,
                position,
                previous: position,
                direction,
                speed_f8: (launchers[i].speed_fps * 256.) as i32,
                launched_t: (world.tick() / 30) as u16,
                target: Some(observed.id),
                fall: FallState::default(),
                gun_round: None,
                tracer: false,
            });
            next += 1;
        }
        let input = SeatInput {
            seat: SeatId(0),
            tick: world.tick(),
            ..Default::default()
        };
        world.step(&[input], &mut out)?;
        for p in &world.combat.state.projectiles {
            if p.guidance.is_some() {
                launched.insert(p.id);
            }
        }
        let (rejected, starving, wait) = world.combat.state.energy_prediction_admission_stats();
        refused = refused.max(rejected);
        starved = starved.max(starving);
        max_wait = max_wait.max(wait);
        let cost = world.combat.state.prediction_cost_ns as f64 / 1e6;
        let cpu = world
            .combat
            .state
            .prediction_cpu_cost_ns
            .map(|n| n as f64 / 1e6);
        if let Some(cpu) = cpu {
            if tick < 240 {
                cold_cpu.push(cpu)
            } else {
                moving_cpu.push(cpu)
            }
        }
        if cost > 2. {
            eprintln!("wall_outlier tick={tick} wall_ms={cost:.6} cpu_ms={cpu:?}");
        }
        let (pending, clients, envelopes) = world.combat.state.energy_prediction_counts();
        max_clients = max_clients.max(clients);
        max_pending = max_pending.max(pending);
        max_envelopes = max_envelopes.max(envelopes);
        if tick < 240 {
            cold.push(cost);
        } else {
            moving.push(cost);
        }
        let start = Instant::now();
        // Same-state frame readers: no job submission or simulation work.
        for (i, slot) in slots.iter().enumerate().filter(|_| !natural) {
            for station in 0..stations {
                let (launcher, observed) = station_geometry(queries[i], observations[i], station);
                answers += usize::from(
                    std::hint::black_box(world.combat.state.energy_solution(
                        slot.id,
                        65535 - station,
                        &weapon,
                        launcher,
                        observed,
                    ))
                    .is_some(),
                );
            }
        }
        warm.push(start.elapsed().as_secs_f64() * 1000.);
        if tick.is_multiple_of(30) {
            println!(
                "{tick},{cost:.6},{requests:.6},{},{pending},{envelopes}",
                world.combat.state.projectiles.len()
            );
        }
    }
    for (name, samples) in [
        ("cold", &mut cold),
        ("moving", &mut moving),
        ("warm_reads", &mut warm),
        ("cold_cpu", &mut cold_cpu),
        ("moving_cpu", &mut moving_cpu),
    ] {
        if samples.is_empty() {
            continue;
        }
        let p50 = percentile(samples, 0.5);
        let p95 = percentile(samples, 0.95);
        let max = samples.last().unwrap();
        eprintln!(
            "{name}: n={} p50_ms={p50:.6} p95_ms={p95:.6} max_ms={max:.6}",
            samples.len()
        );
    }
    eprintln!(
        "max_pending={max_pending} max_ready_envelopes={max_envelopes} successful_cached_reads={answers} frozen_queries={fixed} stations_per_aircraft={stations}"
    );
    eprintln!(
        "guided_launches={} ai={} max_clients={max_clients} refused={refused} starved={starved} max_admission_wait_ticks={max_wait}",
        launched.len(),
        slots.len()
    );
    eprintln!(
        "gate_samples={:?} kills={} tally={:?}",
        world.combat.state.prediction_gate_samples,
        world.combat.state.ledger.kills().len(),
        world.combat.state.ledger.total(|_| true)
    );
    if refused != 0 || starved != 0 {
        return Err("prediction admission regression".into());
    }
    Ok(())
}
