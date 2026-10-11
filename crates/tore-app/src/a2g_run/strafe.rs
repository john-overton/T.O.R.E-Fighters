//! The `strafe` attack: Tab, the gun key, held through a pass with a bomb
//! (or any other store that is not a gun) selected (docs/spec/air-to-ground.md,
//! "The gun key").
//!
//! The pass is the `release` pass; the weapon key selects the attack station,
//! then Tab goes down as a combat command, stays down `--hold` seconds (2 by
//! default) and comes up. It prints the gun's station, the selection before
//! and after, the rounds at the gun and at the selected store, and how many
//! gun rounds left (and how many were tracers). Nothing here fires the
//! selected store: the trigger is never pressed.
use super::ccip::Pass;
use super::{Options, Scene};
use crate::AppResult;
use std::collections::BTreeSet;
use tore_sim::combat::live::{self, Command};
use tore_world::seats::SeatCommand;

pub(super) fn run(scene: &mut Scene, o: &Options) -> AppResult<()> {
    let own = scene.world.combat.state.own();
    let gun = live::key_gun_station(own.configuration(), |i| own.rounds(i))
        .ok_or("the aircraft has no gun")?;
    if gun == scene.station {
        return Err("the attack station is the gun; give --station N for a store".into());
    }
    let pass = Pass::new(scene, o)?;
    let hold = if o.hold > 0.5 { o.hold } else { 2. };
    let gun_start = scene.world.combat.state.own().rounds(gun);
    let store_start = scene.rounds();
    println!(
        "a2g-run: strafe gun station {gun} {} rounds {gun_start}; store station {} {} rounds {store_start}",
        scene.world.combat.state.own().configuration().stations[gun]
            .weapon
            .source,
        scene.station,
        scene.station().weapon.source,
    );
    let mut down: Option<f64> = None;
    let mut up: Option<f64> = None;
    let mut seen = BTreeSet::new();
    let mut fired = 0;
    let mut tracers = 0;
    let mut selected_when_down = None;
    for tick in 0..(o.seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        pass.fly(scene, t);
        scene.select();
        if down.is_none() && scene.selected() {
            scene.queue(SeatCommand::Combat(Command::GunKeyDown));
            down = Some(t);
            selected_when_down = Some(scene.world.combat.state.own().selected);
            println!(
                "a2g-run: strafe key down t={t:.3} selected {}",
                scene.station
            );
        }
        if up.is_none() && down.is_some_and(|at| t >= at + hold) {
            scene.queue(SeatCommand::Combat(Command::GunKeyUp));
            up = Some(t);
            println!("a2g-run: strafe key up t={t:.3}");
        }
        scene.step(false)?;
        let fresh: Vec<(u32, bool)> = scene
            .world
            .combat
            .state
            .projectiles
            .iter()
            .filter(|p| p.owner == scene.owner && p.station == gun && !seen.contains(&p.id))
            .map(|p| (p.id, p.tracer))
            .collect();
        for (id, tracer) in fresh {
            seen.insert(id);
            fired += 1;
            tracers += usize::from(tracer);
        }
        if up.is_some_and(|at| t > at + 1.) {
            break;
        }
    }
    let own = scene.world.combat.state.own();
    println!(
        "a2g-run: strafe gun {gun_start}->{} fired {fired} tracers {tracers} store {store_start}->{} selected {} (key down on {}) bombs-or-store-rounds-fired {}",
        own.rounds(gun),
        scene.rounds(),
        own.selected,
        selected_when_down.map_or("never".into(), |s| s.to_string()),
        scene
            .world
            .combat
            .state
            .projectiles
            .iter()
            .filter(|p| p.owner == scene.owner && p.station == scene.station)
            .count(),
    );
    if down.is_none() {
        println!("a2g-run: no key");
    }
    Ok(())
}
