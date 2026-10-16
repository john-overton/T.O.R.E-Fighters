//! Optional imported simulation resources for runtime aircraft and store choices.
//! These immutable records are not mission state or part of its manifest.
//! The host validates the accepted choice against every peer's content catalogue.
use crate::{
    aircraft_type::{AircraftType, load_type},
    resources::ResourceSource,
};
use std::{cell::RefCell, collections::BTreeMap, sync::Arc};
use tore_formats::aircraft::AircraftId;

#[derive(Default)]
pub struct Resources {
    pub(crate) ai_defaults: [Option<AircraftId>; 6],
    pub(crate) types: Vec<Arc<AircraftType>>,
    pub(crate) loads: Vec<tore_sim::combat::loadout::Loadout>,
    pub(crate) records: BTreeMap<String, Vec<u8>>,
}

struct Reads<'a> {
    source: &'a dyn ResourceSource,
    records: RefCell<BTreeMap<String, Vec<u8>>>,
}
impl ResourceSource for Reads<'_> {
    fn get(&self, name: &str) -> Option<&Vec<u8>> {
        let bytes = self.source.optional(name)?;
        self.records
            .borrow_mut()
            .insert(name.to_owned(), bytes.clone());
        Some(bytes)
    }
    fn theater_catalog(&self) -> tore_formats::Result<Vec<(String, String)>> {
        self.source.theater_catalog()
    }
    fn theater_label(&self, code: &str) -> tore_formats::Result<Option<String>> {
        self.source.theater_label(code)
    }
}
impl Resources {
    pub fn load(source: &dyn ResourceSource, spec: &crate::mission::MissionSpec) -> Self {
        let reads = Reads {
            source,
            records: RefCell::new(source.optional_stores()),
        };
        let mut types = Vec::new();
        let mut loads = Vec::new();
        for id in AircraftId::SELECTABLE {
            if let Ok(kind) = load_type(&reads, id) {
                // Capture the standard load's sensor and equipment dependencies.
                if let Ok(load) = tore_sim::combat::loadout::Loadout::new(&kind.profile, |name| {
                    reads
                        .get(name)
                        .cloned()
                        .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
                }) {
                    loads.push(load);
                    types.push(kind);
                }
            }
        }
        Self {
            ai_defaults: std::array::from_fn(|index| {
                spec.player_choice[index].then_some(spec.wings[index].aircraft)
            }),
            types,
            loads,
            records: reads.records.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mission::{MissionSpec, Start},
        seats::{PlaneId, SeatId},
        world::{Seating, World, revive::RevivalWeapons},
    };
    #[test]
    fn changed_aircraft_and_loadout_restore_into_a_fresh_world_identically() {
        let resources = crate::test_support::resources::gunship_resources();
        let mut spec = MissionSpec::new(crate::test_support::resources::THEATER, AircraftId::F18);
        spec.start = Start::Airborne {
            altitude_ft: 10_000,
        };
        spec.wings[0].count = 2;
        let mut a = World::new(&spec, &resources, Seating::Open).unwrap();
        let mut b = World::new(&spec, &resources, Seating::Open).unwrap();
        let mut chosen = a
            .revival_loadout(AircraftId::Ac130, None, RevivalWeapons::Missiles)
            .unwrap();
        chosen.stations[0].quantity /= 2;
        let spawn = a
            .revival_spawn_excluding(
                Some(PlaneId(0)),
                AircraftId::Ac130,
                [10_000., 10_000., 10_000.],
                12_000.,
                Some(&chosen),
                RevivalWeapons::HalfGuns,
            )
            .unwrap();
        b.despawn_plane(PlaneId(0)).unwrap();
        let after_removal = b
            .revival_spawn_as(
                AircraftId::Ac130,
                [10_000., 10_000., 10_000.],
                12_000.,
                Some(&chosen),
                RevivalWeapons::HalfGuns,
            )
            .unwrap();
        assert_eq!(
            spawn, after_removal,
            "replacement placement excludes the old live AI"
        );
        assert_eq!(
            spawn.loadout.stations[0].quantity,
            chosen.stations[0].quantity.div_ceil(2)
        );
        let new = a
            .replacement_plane(PlaneId(0), AircraftId::Ac130, &spawn)
            .unwrap();
        let command = crate::world::MissionCommand::ReplacePlane {
            seat: SeatId(0),
            old: PlaneId(0),
            aircraft: new.aircraft,
            spawn: Box::new(new.spawn.clone()),
        };
        let models = tore_sim::checkpoint::Models::default();
        let bytes = tore_sim::checkpoint::to_bytes(&command, &models).unwrap();
        let replayed: crate::world::MissionCommand =
            tore_sim::checkpoint::from_bytes(&bytes, &models).unwrap();
        assert_eq!(command, replayed);
        for world in [&mut a, &mut b] {
            world.replace_plane(SeatId(0), PlaneId(0), &new).unwrap();
            assert!(world.combat.state.effects.is_empty());
            assert!(world.combat.state.marks.is_empty());
            assert!(world.plane_pose(PlaneId(0)).is_none());
        }
        assert_eq!(a.checkpoint().unwrap(), b.checkpoint().unwrap());
        let mut fresh = World::new(&spec, &resources, Seating::Open).unwrap();
        fresh.restore(&a.checkpoint().unwrap()).unwrap();
        assert_eq!(a.checkpoint().unwrap(), fresh.checkpoint().unwrap());
        assert_eq!(fresh.aircraft_of(new.plane), Some(AircraftId::Ac130));
        assert!(a.combat.state.targets.iter().all(|t| t.id != 0));
        let mut simultaneous = World::new(&spec, &resources, Seating::Open).unwrap();
        simultaneous.set_lead_hold(true);
        simultaneous
            .revival
            .lead_hold
            .owners
            .push(crate::world::lead_hold::Owned {
                wing: new.slot.wing,
                owner: crate::world::lead_hold::LeadOwner::Away(PlaneId(0)),
                plane: PlaneId(0),
                led: true,
            });
        for id in [0, 1] {
            simultaneous
                .apply_mission_command(&crate::world::MissionCommand::ReplacePlane {
                    seat: SeatId(id as u8),
                    old: PlaneId(id),
                    aircraft: AircraftId::Ac130,
                    spawn: Box::new(spawn.clone()),
                })
                .unwrap();
        }
        assert_eq!(
            simultaneous.lead_owner(new.slot.wing),
            Some(crate::world::lead_hold::LeadOwner::Seat(SeatId(0)))
        );
        assert_ne!(
            simultaneous.roster.seat(SeatId(0)).unwrap().plane,
            simultaneous.roster.seat(SeatId(1)).unwrap().plane
        );
    }
}
