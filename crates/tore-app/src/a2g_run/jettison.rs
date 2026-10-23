//! Recorded, headless tank and selected-bomb jettison acceptance pass.
use super::{FPS_PER_KNOT, Options, Scene};
use crate::AppResult;
use tore_sim::combat::{blast::MarkKind, blast_scale, live::Command};
use tore_world::seats::SeatCommand;

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let aim = scene.target_row().ok_or("no target")?.position;
    let start = [
        aim[0],
        scene.ground(aim[0], aim[2]) + o.altitude,
        aim[2] - 20_000.,
    ];
    let speed = o.speed_kt * FPS_PER_KNOT;
    let initial = scene.world.combat.state.own().payload_lbs();
    let fuel = scene.world.cockpits[0].flight.systems.fuel.external_lbs();
    if fuel <= 0. {
        return Err("jettison probe needs installed tanks".into());
    }
    println!(
        "jettison: before payload {initial:.3} external-fuel {fuel:.3} rounds {}",
        scene.rounds()
    );
    let mut predictions = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut airborne = 0;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        scene.fly([start[0], start[1], start[2] + speed * t], 0., 0., speed);
        scene.select();
        if tick == 120 {
            scene.queue(SeatCommand::Manual(Command::JettisonTanks));
        }
        if tick == 360 {
            scene.queue(SeatCommand::Manual(Command::Jettison));
        }
        scene.step(false)?;
        for p in &scene.world.combat.state.projectiles {
            if seen.insert(p.id) {
                let motion = scene
                    .world
                    .combat
                    .state
                    .bomb_motion(p.id)
                    .ok_or("no inert fall")?;
                let prediction = tore_sim::combat::ballistics::predict_impact(
                    tore_sim::combat::ballistics::ReleaseState {
                        body: motion.body,
                        position: motion.position(0.),
                        velocity: motion.velocity(0.),
                    },
                    |x, z| scene.ground(x, z),
                )
                .ok_or("no predicted contact")?;
                println!(
                    "jettison: predicted id {} contact {:?} time {:.3}",
                    p.id, prediction.position, prediction.seconds
                );
                predictions.push(prediction.position);
                println!(
                    "jettison: release id {} time {:.3} origin {:?} velocity {:?}",
                    p.id,
                    t,
                    motion.position(0.),
                    motion.velocity(0.)
                );
            }
        }
        airborne = airborne.max(scene.world.combat.state.projectiles.len());
        if tick == 121 || tick == 361 {
            let f = &scene.world.cockpits[0].flight;
            println!(
                "jettison: after tick {tick} payload {:.3} external-fuel {:.3} internal-fuel {:.3} rounds {}",
                f.payload_lbs,
                f.systems.fuel.external_lbs(),
                f.fuel,
                scene.rounds()
            );
        }
        if scene
            .world
            .combat
            .state
            .effects
            .iter()
            .any(|e| e.scale != blast_scale::INERT)
        {
            return Err("jettison produced an armed effect".into());
        }
    }
    let state = &scene.world.combat.state;
    if !state.projectiles.is_empty() || seen.is_empty() || scene.rounds() != 0 {
        return Err("jettison stores did not all fall and land".into());
    }
    if state.marks.len() != seen.len()
        || state
            .marks
            .iter()
            .any(|m| !matches!(m.kind, MarkKind::Crater(tore_sim::combat::blast::POCK)))
    {
        return Err("jettison crater count or size differs".into());
    }
    for expected in predictions {
        let miss = state
            .marks
            .iter()
            .map(|mark| {
                mark.position
                    .iter()
                    .zip(expected)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .fold(f64::INFINITY, f64::min);
        println!("jettison: prediction-error {miss:.3} ft");
        if miss > 2. {
            return Err("jettison missed its predicted terrain contact".into());
        }
    }
    for mark in &state.marks {
        println!("jettison: crater {:?} scale {}", mark.position, mark.scale);
    }
    println!(
        "jettison: PASS released {} max-airborne {airborne} small-craters {} no-explosion",
        seen.len(),
        state.marks.len()
    );
    Ok(())
}
