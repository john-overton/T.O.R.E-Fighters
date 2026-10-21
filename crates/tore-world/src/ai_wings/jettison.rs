//! Apply AI decisions through the same inert separation used by human seats.
use super::*;
use tore_sim::ai::{jettison::Drop, weapon_service::Rounds};

pub(super) fn apply(
    wings: &mut AiWings,
    state: &mut live::State,
    requests: &[(u32, Drop)],
) -> WorldResult<()> {
    for &(id, drop) in requests {
        let Some(actor) = wings.mission.actor_mut(id) else {
            continue;
        };
        if !actor.alive() {
            continue;
        }
        let Some(config) = wings.configs.get_mut(&id) else {
            continue;
        };
        // A tank drop takes every external tank; an ordnance drop leaves
        // them (docs/spec/jettison.md).
        let tanks = drop != Drop::Ordnance
            && config
                .tanks
                .iter()
                .any(|t| t.quantity > 0 && t.store.is_some());
        let ordnance = drop != Drop::Tanks;
        let mut stations: Vec<usize> = if ordnance {
            config
                .stations
                .iter()
                .enumerate()
                .filter(|(i, s)| {
                    !s.internal
                        && !live::is_gun(&s.weapon)
                        && (live::is_strike_store(&s.weapon)
                            || (live::engages_surface(&s.weapon)
                                && !live::engages_aircraft(&s.weapon)))
                        && actor
                            .stations()
                            .iter()
                            .any(|a| usize::from(a.station.0) == *i && !a.is_empty())
                })
                .map(|(i, _)| i)
                .collect()
        } else {
            Vec::new()
        };
        if ordnance {
            stations.extend(config.designator_pods.iter().map(|p| p.station));
            stations.sort_unstable();
            stations.dedup();
        }
        let had_designator = !config.designator_pods.is_empty();
        if !tanks && stations.is_empty() {
            continue;
        }
        let side = state
            .targets
            .iter()
            .find(|t| t.id == id)
            .map_or(live::NO_SIDE, |t| t.side);
        let mut own = live::Ownship::new(id, side, config.clone(), true)?;
        own.ammo.fill(0);
        for station in actor.stations() {
            if let Some(ammo) = own.ammo.get_mut(usize::from(station.station.0)) {
                *ammo = match station.rounds() {
                    Rounds::Finite(n) => n.min(0x7fff) as u16,
                    Rounds::Unlimited => 0x7fff,
                };
            }
        }
        if let Some(kept) = state.kept(id) {
            own.restore(kept, false, false);
        }
        let before = own.payload_lbs();
        state.jettison_ai(
            &mut own,
            crate::combat::launcher(actor.flight()),
            tanks,
            &stations,
        );
        let removed = before - own.payload_lbs();
        // The configuration counts full tank fuel; undo the burn offset when
        // removing that capacity, exactly as the human command does.
        let used = if tanks {
            actor.flight().systems.used_external_lbs()
        } else {
            0.
        };
        let payload = (actor.flight().payload_lbs - removed + used).max(0.);
        actor.flight_mut().set_payload(payload)?;
        if tanks {
            actor.flight_mut().systems.fuel.jettison_all();
        }
        for station in actor.stations_mut() {
            if stations.contains(&usize::from(station.station.0)) {
                station.store.rounds = Rounds::Finite(0);
            }
        }
        *config = own.configuration().clone();
        if had_designator && config.designator_pods.is_empty() {
            if let Some(cockpit) = state.ai_cockpit_mut(id, side, config) {
                cockpit.sight = own.cockpit.sight;
                cockpit.laser = Default::default();
            }
            state.set_ai_laser_on(id, false);
        }

        if state.kept(id).is_some() {
            state.keep(id, own.kept());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{aircraft, combat_fixture, payload, spawned};
    use tore_sim::combat::{
        hardpoints::Rack,
        live::{TankStation, TankStore},
    };

    /// An AI aircraft (id 1) with an AAM, an A-G missile pair, a gun and a
    /// 1,100 lb tank that has burned 300 lb: the state, the bridge and the
    /// loaded payload.
    fn loaded() -> (live::State, AiWings, f64) {
        let state = combat_fixture(true);
        let mut config = state.own().configuration().clone();
        // Three distinct stores: an AAM, an A-G missile, and a gun.
        let mut ag = config.stations[0].clone();
        ag.weapon.flags = live::SURFACE_TARGET_FLAG;
        ag.weapon.weight = 500;
        ag.count = 2;
        ag.internal = false;
        config.stations[0].weapon.flags = 0x30000; // AAM with a secondary surface flag stays.
        config.stations[0].internal = false;
        config.stations.push(ag);
        let mut gun = config.stations[0].clone();
        gun.weapon.flags = 1;
        gun.weapon.source = "M61.JT".into();
        config.stations.push(gun);
        config.tanks = vec![TankStation {
            hardpoint: 0,
            mount: [0., -3., 0.],
            quantity: 1,
            rack: Rack::default(),
            store: Some(TankStore {
                source: "TEST.GAS".into(),
                name: "Tank".into(),
                tank: tore_formats::weapons::Tank {
                    empty_weight: 100,
                    fuel_weight: 1000,
                    flags: 1,
                },
            }),
        }];
        config.refresh_tanks().unwrap();
        let mut wings =
            AiWings::build_with(&payload(None), &spawned(), 0, |_| Ok((aircraft(), None))).unwrap();
        let id = 1;
        let specs = super::super::station_specs(&config, false);
        let mass = super::super::payload_lbs(&config, &specs);
        let a = wings.mission.actor_mut(id).unwrap();
        a.set_stations(specs);
        a.flight_mut().set_payload(mass - 300.).unwrap();
        a.flight_mut().systems.fuel =
            tore_sim::aircraft_systems::Fuel::new(config.external_fuel_lbs);
        a.flight_mut().systems.fuel.external[0] -= 300.;
        a.flight_mut().enable_research(1).unwrap();
        wings.configs.insert(id, config);
        (state, wings, mass)
    }

    #[test]
    fn an_ordnance_drop_keeps_the_tanks_air_missiles_and_gun() {
        let (mut state, mut wings, mass) = loaded();
        let id = 1;
        let fuel = wings
            .mission
            .actor(id)
            .unwrap()
            .flight()
            .systems
            .external_lbs();
        apply(&mut wings, &mut state, &[(id, Drop::Ordnance)]).unwrap();
        let a = wings.mission.actor(id).unwrap();
        assert!(a.stations()[1].is_empty(), "the A-G pair goes");
        assert!(!a.stations()[0].is_empty(), "the AAM stays");
        assert!(!a.stations()[2].is_empty(), "the gun stays");
        assert_eq!(a.flight().systems.external_lbs(), fuel, "the tank stays");
        assert!((a.flight().payload_lbs - (mass - 300. - 1000.)).abs() < 1e-9);
        let config = wings.configuration(id).unwrap();
        assert!(config.tanks.iter().all(|t| t.quantity == 1));
        assert!(
            state
                .projectiles
                .iter()
                .all(|p| !state.weapon(p).source.ends_with(".GAS"))
        );
        // Nothing left to shed: a repeated request is a no-op.
        let count = state.projectiles.len();
        assert_eq!(count, 2);
        apply(&mut wings, &mut state, &[(id, Drop::Ordnance)]).unwrap();
        assert_eq!(state.projectiles.len(), count);
    }

    #[test]
    fn drops_once_preserving_air_missiles_and_guns_and_removes_burned_tank_mass() {
        let (mut state, mut wings, mass) = loaded();
        let id = 1;
        let a = wings.mission.actor(id).unwrap();
        let loaded_stall = a.flight().target_stall_scale();
        let internal = a.flight().fuel;
        let ammo = a.stations()[0].rounds();
        // The radar signature follows the live loadout (docs/radar.md,
        // "External stores"): the tank, then the strike stores, stop counting.
        let mut rows = spawned();
        let carried = |wings: &AiWings, rows: &mut [live::Target]| {
            wings.mirror_pose_out(rows);
            let row = rows.iter().find(|t| t.id == id).unwrap();
            assert_eq!(row.configuration.stores, wings.external_stores(id));
            row.configuration.stores
        };
        let loaded = carried(&wings, &mut rows);
        apply(&mut wings, &mut state, &[(id, Drop::Tanks)]).unwrap();
        let dry = carried(&wings, &mut rows);
        assert_eq!(dry.count, loaded.count - 1.);
        assert!((loaded.lbs - dry.lbs - 1100.).abs() < 1e-9);
        let a = wings.mission.actor(id).unwrap();
        assert_eq!(a.flight().payload_lbs, mass - 1100.);
        assert_eq!(a.flight().systems.fuel.external_lbs(), 0.);
        assert_eq!(a.flight().fuel, internal);
        assert_eq!(a.stations()[0].rounds(), ammo);
        assert!(!a.stations()[1].is_empty());
        apply(&mut wings, &mut state, &[(id, Drop::CombatLoad)]).unwrap();
        let clean = carried(&wings, &mut rows);
        assert_eq!(clean.count, dry.count - 2.);
        assert!((dry.lbs - clean.lbs - 1000.).abs() < 1e-9);
        let a = wings.mission.actor(id).unwrap();
        assert!(a.stations()[1].is_empty());
        assert!(!a.stations()[0].is_empty());
        assert!(!a.stations()[2].is_empty());
        assert!(a.flight().payload_lbs < mass - 1100.);
        assert!(a.flight().target_stall_scale() < loaded_stall);
        let count = state.projectiles.len();
        assert!(count >= 2);
        assert!(
            state
                .projectiles
                .iter()
                .all(|p| p.owner == id && state.weapon(p).source.starts_with("JETTISON-"))
        );
        state.take_sound_events();
        apply(&mut wings, &mut state, &[(id, Drop::CombatLoad)]).unwrap();
        assert_eq!(state.projectiles.len(), count);
        assert!(state.take_sound_events().is_empty());
    }
}
