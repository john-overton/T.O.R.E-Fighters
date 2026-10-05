//! The equivalence scenarios of docs/formats/checkpoint.md: missions built
//! from synthetic fixtures, each with the inputs and fixture adjustments of
//! every tick, the tick N to checkpoint at and the M ticks to step on.
//!
//! Stage H0 has the scenarios today's fixtures allow: the single-player tick
//! mission, the crowd fixture's fight with four humans, and an open mission
//! with handoffs. Slice H8 adds the ground start, the AI landing and the
//! changing weather, and the state each scenario asserts at tick N.

use super::{World, crowd, tick_tests};
use crate::{
    mission::{MissionSpec, Skill, Start},
    seats::{PlaneId, SeatId, SeatInput},
    test_support::resources::{THEATER, resources},
    world::{MissionCommand, Seating},
};
use tore_formats::aircraft::AircraftId;
use tore_input::{PilotCommand, PilotInput, Switch};

/// One scenario. `drive` gets the world before step `step` (counted from the
/// build) and returns that step's mission commands and seat inputs; it may
/// adjust the world first, as the fixtures' own tests do. It must depend only
/// on the world and the step, so the original and the restored copy are
/// driven alike.
/// One step's mission commands and seat inputs.
pub(super) type Step = (Vec<MissionCommand>, Vec<SeatInput>);

pub(super) struct Scenario {
    pub name: &'static str,
    pub build: fn() -> World,
    pub drive: fn(&mut World, u64) -> Step,
    /// The step count at which the checkpoint is taken.
    pub at: u64,
    /// How many steps both copies then take.
    pub then: u64,
}

/// Every scenario, in a fixed order.
pub(super) fn all() -> Vec<Scenario> {
    vec![single_player(), crowd_fight(), open_handoffs()]
}

/// The full-tick fingerprint mission: a low-flying player with turbulence,
/// the airport service and a tower conversation, two AI aircraft a side and
/// two drones placed on the way.
pub(super) fn single_player() -> Scenario {
    fn drive(world: &mut World, step: u64) -> Step {
        match step {
            195 => tick_tests::place_drone(world, tick_tests::DRONES[0]),
            695 => tick_tests::place_drone(world, tick_tests::DRONES[1]),
            _ => {}
        }
        let mut input = tick_tests::script(step as usize);
        input.tick = world.tick();
        (Vec::new(), vec![input])
    }
    Scenario {
        name: "single player",
        build: tick_tests::mission,
        drive,
        at: 600,
        then: 600,
    }
}

/// The crowd fixture: four against four at 10,000 feet, two humans a side,
/// radars on, turning into each other with the guns in bursts while the AI
/// fights on its own.
pub(super) fn crowd_fight() -> Scenario {
    fn drive(world: &mut World, step: u64) -> Step {
        let inputs = crowd::inputs(world, |seat| {
            let side = if seat.0 < 2 { 1. } else { -1. };
            let mut pilot = PilotInput::default();
            if step == 10 {
                pilot.commands.push(PilotCommand::Set(Switch::Radar, true));
            }
            match step {
                120..180 => pilot.roll = 0.4 * side,
                180..300 => pilot.pitch = 0.3,
                300..360 => pilot.roll = -0.4 * side,
                800..860 => pilot.pitch = -0.2,
                _ => {}
            }
            SeatInput {
                pilot,
                trigger: (400..430).contains(&step) || (900..920).contains(&step),
                ..SeatInput::default()
            }
        });
        (Vec::new(), inputs)
    }
    Scenario {
        name: "crowd fight",
        build: crowd::crowded_mission,
        drive,
        at: 600,
        then: 600,
    }
}

/// An open mission built by `World::new` from the synthetic import, three
/// against three: seats take planes, one gives its plane back and another
/// takes one later, so the checkpoint's cockpits, ownships and AI actors
/// differ from the fresh world's.
pub(super) fn open_handoffs() -> Scenario {
    fn build() -> World {
        let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
        spec.wings[0].count = 3;
        spec.wings[3].count = 3;
        spec.wings[3].skill = Skill::Average;
        spec.separation_nm = 5;
        spec.start = Start::Airborne {
            altitude_ft: 10_000,
        };
        World::new(&spec, &resources(), Seating::Open).unwrap()
    }
    fn drive(world: &mut World, step: u64) -> Step {
        let planes: Vec<PlaneId> = world.roster.planes().iter().map(|p| p.id).collect();
        let commands = match step {
            60 => vec![MissionCommand::Take {
                seat: SeatId(0),
                plane: planes[0],
            }],
            61 => vec![MissionCommand::Take {
                seat: SeatId(1),
                plane: planes[planes.len() - 1],
            }],
            500 => vec![MissionCommand::GiveBack { seat: SeatId(0) }],
            640 => vec![MissionCommand::Take {
                seat: SeatId(2),
                plane: planes[1],
            }],
            _ => Vec::new(),
        };
        let mut flying: Vec<SeatId> = world
            .roster
            .seats()
            .iter()
            .filter(|seat| seat.plane.is_some())
            .map(|seat| seat.id)
            .collect();
        for command in &commands {
            match *command {
                MissionCommand::Take { seat, .. } => flying.push(seat),
                MissionCommand::GiveBack { seat } => flying.retain(|s| *s != seat),
                MissionCommand::Settings(_) => {}
            }
        }
        let tick = world.tick();
        let inputs = flying
            .into_iter()
            .map(|seat| SeatInput {
                seat,
                tick,
                trigger: (300..320).contains(&step),
                pilot: PilotInput {
                    pitch: 0.2,
                    roll: if step % 480 < 240 { 0.3 } else { -0.3 },
                    ..PilotInput::default()
                },
                ..SeatInput::default()
            })
            .collect();
        (commands, inputs)
    }
    Scenario {
        name: "open mission with handoffs",
        build,
        drive,
        at: 700,
        then: 600,
    }
}
