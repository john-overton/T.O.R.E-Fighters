//! Real headless world: eight AI aircraft, sixteen replenished missiles and
//! eight requested launch/envelope solutions. Never opens a window or socket.
use std::{path::Path, time::Instant};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::{
    attitude::unit,
    combat::{
        FallState,
        live::Projectile,
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let pack = args
        .get(1)
        .ok_or("usage: missile_energy_cost PACK [TICKS] [THEATER]")?;
    let ticks = args
        .get(2)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(2400u64);
    let theater = args.get(3).map_or("UKR", String::as_str);
    let resources = tore_import::pack::read_pack(Path::new(pack))?;
    let mut spec = MissionSpec::new(theater, AircraftId::F16C);
    spec.preset = Preset::Hold;
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
        .apply_mission_preset(Preset::Hold, player_position);
    world.combat.state.weapon_rules = Rules::Energy;
    world.combat.state.measure_prediction_cost = true;
    if cfg!(target_os = "linux") && thread_cpu_ns().is_some() {
        world.combat.state.prediction_cpu_clock = Some(thread_cpu_ns);
    }
    world.combat.state.own_mut().armed = false;
    let slots = world.ai_wings.as_ref().ok_or("no AI")?.slots().to_vec();
    if slots.len() != 8 {
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
            launchers.push(tore_world::combat::launcher(flight));
        }
        let queries = if fixed {
            frozen.get_or_insert_with(|| launchers.clone()).clone()
        } else {
            launchers.clone()
        };
        let request_cpu_start = thread_cpu_ns();
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
        // A reserved probe station avoids replacing an actor's real launch job.
        for (i, slot) in slots.iter().enumerate() {
            world.combat.state.request_energy_solution(
                slot.id,
                65535,
                &weapon,
                queries[i],
                observations[i],
            );
            world.combat.state.request_energy_range(
                slot.id,
                65535,
                &weapon,
                queries[i],
                observations[i],
            );
        }
        let requests = start.elapsed().as_secs_f64() * 1000.;
        let request_cpu = request_cpu_start
            .zip(thread_cpu_ns())
            .and_then(|(a, b)| b.checked_sub(a));
        while world.combat.state.projectiles.len() < 16 {
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
        let cost = world.combat.state.prediction_cost_ns as f64 / 1e6 + requests;
        let cpu = world
            .combat
            .state
            .prediction_cpu_cost_ns
            .zip(request_cpu)
            .map(|(a, b)| a.saturating_add(b) as f64 / 1e6);
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
        let (pending, _, envelopes) = world.combat.state.energy_prediction_counts();
        max_pending = max_pending.max(pending);
        max_envelopes = max_envelopes.max(envelopes);
        if tick < 240 {
            cold.push(cost);
        } else {
            moving.push(cost);
        }
        let start = Instant::now();
        // Same-state frame readers: no job submission or simulation work.
        for (i, slot) in slots.iter().enumerate() {
            let observed = observations[i];
            answers += usize::from(
                std::hint::black_box(
                    world
                        .combat
                        .state
                        .energy_solution(slot.id, 65535, &weapon, queries[i], observed),
                )
                .is_some(),
            );
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
        "max_pending={max_pending} max_ready_envelopes={max_envelopes} successful_cached_reads={answers} frozen_queries={fixed}"
    );
    Ok(())
}
