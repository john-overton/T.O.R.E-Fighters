//! The `--ai-roster-probe-ticks` command-line check: every aircraft and
//! experience level flies a two-wing mission against imported media and prints
//! one line each. It is an app harness, not mission state.
use crate::{AppResult, ai_wings::AiWings, combat, terrain::Terrain};
use std::collections::BTreeMap;
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_sim::{attitude::Basis, combat::live, flight};

/// Imported-media validation, separate from synthetic model tests. No retail
/// bytes or generated assets are written by this probe.
pub fn roster_probe(
    ticks: usize,
    resources: &BTreeMap<String, Vec<u8>>,
    world: &Terrain,
) -> AppResult<()> {
    use tore_sim::ai::{
        Experience,
        launch::{self, WingId, WingSelection, resolve_wings},
    };
    for id in AircraftId::ALL {
        let aircraft = Aircraft::parse(
            resources
                .get(id.pt())
                .ok_or_else(|| format!("missing {}", id.pt()))?,
        )?;
        let config = live::Configuration::from_source(&aircraft, |name| {
            resources
                .get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })?;
        for level in Experience::ALL {
            let wings = resolve_wings(
                &[
                    WingSelection {
                        wing: WingId::new(launch::Side::Friendly, 0)?,
                        aircraft: id,
                        count: 1,
                        skill_level: level.index() as i32,
                    },
                    WingSelection {
                        wing: WingId::new(launch::Side::Enemy, 0)?,
                        aircraft: id,
                        count: 1,
                        skill_level: level.index() as i32,
                    },
                ],
                None,
            )?;
            let mut combat = live::State::new(config.clone(), true)?;
            combat.add_dummy(
                &config,
                [512.0, 30000.0, -512.0],
                Basis::new(0.0, 0.0, 0.0),
                live::Side(1),
            );
            combat.add_dummy(
                &config,
                [0.0, 30000.0, 30000.0],
                Basis::new(std::f64::consts::PI, 0.0, 0.0),
                live::Side(2),
            );
            let mut bridge = AiWings::build(&wings, &combat.targets, false, resources)?;
            for actor in bridge.mission().actors() {
                if actor.stations().len() != config.stations.len()
                    || actor
                        .stations()
                        .iter()
                        .zip(&config.stations)
                        .any(|(station, imported)| {
                            station.employment_zone != Some(imported.weapon.seeker.zones[1])
                                || station.rounds()
                                    != tore_sim::ai::weapon_service::Rounds::Finite(u32::from(
                                        imported.count,
                                    ))
                        })
                {
                    return Err(format!("AI inventory/envelope mismatch for {}", id.pt()).into());
                }
            }
            let mut player = flight::State::new(&aircraft, [0.0, 30000.0, 0.0])?;
            let mut peak_roll = 0.0f64;
            let mut peak_turn = 0.0f64;
            for _ in 0..ticks {
                let before: Vec<_> = bridge
                    .mission()
                    .actors()
                    .iter()
                    .map(|a| (a.alive(), a.flight().clone()))
                    .collect();
                player.step(&flight::PilotInput::default(), |x, z| {
                    f64::from(world.height(x as f32, z as f32))
                });
                combat.step(
                    &[tore_sim::combat::live::OwnshipInput {
                        aircraft: combat.own().aircraft,
                        held: false,
                        launcher: combat::launcher(&player),
                    }],
                    |x, z| f64::from(world.height(x as f32, z as f32)),
                );
                bridge.step(&mut combat, &player, world)?;
                for (actor, (was_alive, mut replay)) in bridge.mission().actors().iter().zip(before)
                {
                    let bank = replay.bank;
                    let yaw = replay.yaw;
                    if was_alive && actor.alive() {
                        replay.damage_fraction = actor.flight().damage_fraction;
                        replay.payload_lbs = actor.flight().payload_lbs;
                        replay.step(actor.last_input(), |x, z| {
                            f64::from(world.height(x as f32, z as f32))
                        });
                        if &replay != actor.flight() {
                            return Err(
                                format!("AI input replay diverged {} {level:?}", id.pt()).into()
                            );
                        }
                    }
                    let f = actor.flight();
                    if !f.position.iter().all(|v| v.is_finite()) {
                        return Err(format!("nonfinite {} {level:?}", id.pt()).into());
                    }
                    let rate = |delta: f64| {
                        ((delta.to_degrees() + 180.0).rem_euclid(360.0) - 180.0).abs() * 120.0
                    };
                    peak_roll = peak_roll.max(rate(f.bank - bank));
                    peak_turn = peak_turn.max(rate(f.yaw - yaw));
                }
            }
            println!(
                "AI roster {} {level:?}: ticks={ticks} models=2 radar={} ir={} stores={} gun={} projectiles={} dropped={} peak_roll={peak_roll:.3} peak_turn={peak_turn:.3} PASS",
                id.pt(),
                config.sensors.radar.is_some(),
                config.sensors.infrared.is_some(),
                config.stations.len(),
                config.stations.iter().any(|s| s.weapon.source == id.gun()),
                bridge.realised_launches,
                bridge.dropped_launches
            );
        }
    }
    Ok(())
}
