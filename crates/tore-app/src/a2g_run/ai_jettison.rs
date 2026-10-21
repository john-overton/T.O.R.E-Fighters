//! Recorded headless acceptance: a strike pilot perceives an attacking bandit
//! and drops only its tanks, keeping its bombs; ten seconds later its engine
//! is damaged and it sheds the bombs to get home (docs/spec/jettison.md).
use super::{Options, SEAT, Scene};
use crate::AppResult;

pub(super) fn run(
    scene: &mut Scene,
    o: &Options,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
) -> AppResult<()> {
    let aim = scene.target_row().ok_or("no target")?.position;
    let start = [
        aim[0],
        scene.ground(aim[0], aim[2]) + o.altitude,
        aim[2] - 40_000.,
    ];
    // The original player remains the recorder's observer. Temporarily take
    // its wingman to fit the strike load, then hand that aircraft back to AI.
    scene.fly(
        [start[0] + 1500., start[1], start[2] - 1500.],
        0.,
        0.,
        o.speed_kt * super::FPS_PER_KNOT,
    );
    let observer = scene.owner;
    let id = 1;
    scene.world.give_back_plane(SEAT)?;
    scene
        .world
        .take_plane(SEAT, tore_world::seats::PlaneId(id))?;
    scene.station = super::fit_store(&mut scene.world, id, o, resources)?;
    scene.fly(start, 0., 0., o.speed_kt * super::FPS_PER_KNOT);
    let before = scene.world.cockpits[0].flight.payload_lbs;
    scene.world.give_back_plane(SEAT)?;
    scene
        .world
        .take_plane(SEAT, tore_world::seats::PlaneId(observer))?;
    scene.owner = id;
    let wings = scene.world.ai_wings.as_mut().ok_or("no AI")?;
    wings.set_strike_order(id, Some(scene.target));
    let enemy = wings
        .mission()
        .actors()
        .iter()
        .find(|a| a.identity().side != wings.mission().actor(id).unwrap().identity().side)
        .map(|a| a.id())
        .ok_or("no bandit")?;
    // Start after the observer handoff, with its aircraft identity and picture.
    scene.step(false)?;
    if let Some(path) = &o.record {
        scene.record = Some(crate::surface_objective::Record::start(
            &scene.world,
            path,
            o.verify,
            o.aircraft.label(),
            "AI defensive jettison",
        )?);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut drops = Vec::new();
    // The damage tick: an engine power loss through the ordinary fault path.
    const DAMAGE: u64 = 120 + 10 * 120;
    for tick in 0..(o.seconds * 120.) as u64 {
        if tick == DAMAGE {
            let target = scene
                .world
                .combat
                .state
                .targets
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or("striker row lost")?;
            // Fault 5: engine power reduced.
            target.faults.counts[5] = target.faults.counts[5].saturating_add(1);
            println!("ai-jettison: damage tick={tick} fault=5");
        }
        if tick == 120 {
            let wings = scene.world.ai_wings.as_mut().unwrap();
            let p = wings.mission().actor(id).unwrap().flight().position;
            let enemy_flight = wings.actor_flight_mut(enemy).unwrap();
            enemy_flight.position = [p[0], p[1], p[2] + 2500.];
            enemy_flight.yaw = std::f64::consts::PI;
            enemy_flight.velocity = [0., 0., -400.];
            // Synthetic perceived attack evidence, not a hidden-target decision.
            wings.report_weapon_hits(&[tore_sim::combat::live::Event::Hit(id)]);
        }
        scene.step(false)?;
        for p in &scene.world.combat.state.projectiles {
            if p.owner == id
                && scene
                    .world
                    .combat
                    .state
                    .weapon(p)
                    .source
                    .starts_with("JETTISON-")
                && seen.insert(p.id)
            {
                let name = scene.world.combat.state.weapon(p).source.clone();
                println!("ai-jettison: drop tick={tick} id={} store={name}", p.id);
                drops.push((tick, name));
            }
        }
        if tick < 120 && !drops.is_empty() {
            return Err("cruise jettison".into());
        }
    }
    let actor = scene
        .world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(id)
        .ok_or("striker lost")?;
    let after = actor.flight().payload_lbs;
    let attack: Vec<_> = drops.iter().filter(|(t, _)| *t < DAMAGE).collect();
    if !attack.iter().any(|(_, n)| n.ends_with(".GAS")) {
        return Err(format!("the attack dropped no tanks: {drops:?}").into());
    }
    if attack.iter().any(|(_, n)| !n.ends_with(".GAS")) {
        return Err(format!("the attack shed weapons, not only tanks: {drops:?}").into());
    }
    if !drops
        .iter()
        .any(|(t, n)| *t >= DAMAGE && n.contains("MK82"))
        || after >= before
    {
        return Err(
            format!("no bombs shed after damage: {drops:?}, payload {before} -> {after}").into(),
        );
    }
    println!(
        "ai-jettison: attack drops={} (tanks only), damage drops={} return={:?}",
        attack.len(),
        drops.len() - attack.len(),
        actor.damage_return()
    );
    if scene
        .world
        .combat
        .state
        .projectiles
        .iter()
        .any(|p| seen.contains(&p.id))
    {
        return Err("inert stores still airborne".into());
    }
    println!(
        "ai-jettison: PASS drops={} payload={before:.3}->{after:.3} inert-landed",
        drops.len()
    );
    Ok(())
}
