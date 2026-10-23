//! Open seating (`Seating::Open`, slice D3c): a mission built from a spec with
//! every plane on the AI, plane 0 included, that steps with no human, and
//! humans taking and giving back planes at any time, the last one leaving the
//! mission with nobody in it. Synthetic resources only.

use super::*;
use crate::{
    mission::{MissionSpec, Skill, Start},
    seats::Pilot,
    test_support::resources::{THEATER, resources},
};
use tore_formats::aircraft::AircraftId;
use tore_sim::{ai::launch::Side, attitude};

/// Friendly Wing 1 of `friendly` and the enemy's Wing 1 of `enemy`, the
/// enemy `separation_nm` ahead, airborne at 10,000 feet.
fn spec(friendly: usize, enemy: usize, separation_nm: u32) -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = friendly;
    spec.wings[3].count = enemy;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = separation_nm;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// One tick with the `commands` and an input for each seat in `seats`, the
/// trigger held when `held`, pulling gently into a climbing turn.
fn tick(
    world: &mut World,
    commands: &[MissionCommand],
    seats: &[u8],
    held: bool,
    out: &mut TickOutput,
) {
    let tick = world.tick();
    let inputs: Vec<SeatInput> = seats
        .iter()
        .map(|&seat| SeatInput {
            seat: SeatId(seat),
            tick,
            trigger: held,
            // Level while firing, a gentle turn otherwise.
            pilot: tore_sim::flight::PilotInput {
                pitch: if held { 0. } else { 0.2 },
                roll: match (held, tick % 480 < 240) {
                    (true, _) => 0.,
                    (false, true) => 0.3,
                    (false, false) => -0.3,
                },
                ..Default::default()
            },
            ..SeatInput::default()
        })
        .collect();
    world
        .step_with(commands, &inputs, out, |_, _| Ok(()))
        .unwrap();
}

/// The mission at one moment, to the bit: every human plane's flight, every
/// ownship, every aircraft row and every AI aircraft.
fn digest(world: &World) -> String {
    let mut text = format!("tick {}\n", world.tick());
    for c in &world.cockpits {
        let f = &c.flight;
        text += &format!(
            "cockpit {} {:?} {:?} {:?} {}\n",
            c.plane.0,
            f.position,
            f.velocity,
            [f.yaw, f.pitch, f.bank],
            f.fuel
        );
    }
    for o in world.combat.state.ownships() {
        text += &format!("ownship {} {} {:?}\n", o.aircraft, o.hp, o.ammo);
    }
    for t in world
        .combat
        .state
        .targets
        .iter()
        .filter(|t| t.role == tore_sim::combat::missiles::TargetRole::Aircraft)
    {
        text += &format!("row {} {:?} {:?} {}\n", t.id, t.position, t.velocity, t.hp);
    }
    for actor in world.ai_wings.as_ref().unwrap().mission().actors() {
        let f = actor.flight();
        text += &format!("actor {} {:?} {:?}\n", actor.id(), f.position, f.velocity);
    }
    text
}

/// One cockpit and one ownship for each human-flown plane, an AI actor and a
/// combat row for every other one, and seats that agree with the planes.
fn assert_consistent(world: &World) {
    let humans: Vec<u32> = world
        .roster
        .planes()
        .iter()
        .filter(|p| matches!(p.pilot, Pilot::Human(_)))
        .map(|p| p.id.0)
        .collect();
    let cockpits: Vec<u32> = world.cockpits.iter().map(|c| c.plane.0).collect();
    let ownships: Vec<u32> = world
        .combat
        .state
        .ownships()
        .iter()
        .map(|o| o.aircraft)
        .collect();
    assert_eq!(cockpits, humans);
    assert_eq!(ownships, humans);
    let wings = world.ai_wings.as_ref().unwrap();
    for plane in world.roster.planes() {
        let actor = wings.mission().actor(plane.id.0).is_some();
        let row = world
            .combat
            .state
            .targets
            .iter()
            .any(|t| t.id == plane.id.0);
        match plane.pilot {
            Pilot::Ai => assert!(actor && row, "AI plane {}", plane.id.0),
            Pilot::Human(_) | Pilot::Lost => {
                assert!(!actor && !row, "human or lost plane {}", plane.id.0)
            }
        }
    }
    assert_eq!(
        world.picture_plane_if_any(),
        world.cockpits.first().map(|c| c.plane)
    );
}

#[test]
fn an_open_mission_puts_every_plane_on_the_ai() {
    let map = resources();
    let spec = spec(2, 2, 2);
    let built = World::build(&spec, &map, Seating::Open, &mut Hooks::default()).unwrap();
    let world = built.world;
    // Friendly Wing 1 keeps its full count: four planes, all on the AI.
    assert_eq!(built.restarted.ai_aircraft, Some(4));
    assert!(world.cockpits.is_empty());
    assert!(world.roster.seats().is_empty());
    assert_eq!(world.comms.seats().count(), 0);
    assert!(world.combat.state.ownships().is_empty());
    assert_eq!(world.picture_plane_if_any(), None);
    assert_eq!(world.combat.render_plane(), None);
    let planes: Vec<(u32, Pilot)> = world
        .roster
        .planes()
        .iter()
        .map(|p| (p.id.0, p.pilot))
        .collect();
    assert_eq!(
        planes,
        [0, 1, 2, 3].map(|id| (id, Pilot::Ai)),
        "plane 0 is the AI's"
    );
    assert_eq!(
        world.roster.plane(PlaneId(0)).unwrap().slot,
        crate::seats::Slot::FRIENDLY_LEAD
    );
    let wings = world.ai_wings.as_ref().unwrap();
    let lead = wings.mission().actor(0).expect("plane 0 is an AI actor");
    assert!(lead.identity().is_leader());
    // Every AI aircraft flies the hybrid model.
    assert!(
        wings
            .mission()
            .actors()
            .iter()
            .all(|a| a.flight().research.is_some())
    );
    assert_eq!(wings.flight_model(), ai_wings::AiFlightModel::AllHybrid);
    // Plane 0 is a combat row like its wingmen, with its type, and starts where
    // single player's player would, the wingman in echelon behind it.
    let row = |id: u32| {
        world
            .combat
            .state
            .targets
            .iter()
            .find(|t| t.id == id)
            .unwrap()
    };
    assert_eq!(row(0).aircraft, Some(AircraftId::F18));
    let single = World::new(&spec, &map, Seating::SinglePlayer).unwrap();
    let player = single.cockpits[0].flight.position;
    let start = row(0).position;
    assert_eq!([start[0], start[2]], [player[0], player[2]]);
    assert!((start[1] - player[1]).abs() < 1.);
    let wingman = row(1).position;
    let behind = (wingman[0] - start[0]).hypot(wingman[2] - start[2]);
    assert!((300. ..2000.).contains(&behind), "{behind} ft");
    // The picture hides nothing: plane 0's type is drawn for it.
    assert!(
        world
            .combat
            .dummy_types()
            .iter()
            .any(|t| t.profile.id == AircraftId::F18)
    );
    // Plane 0 leads its wing once the mission steps.
    let mut world = world;
    world.step(&[], &mut TickOutput::default()).unwrap();
    let wings = world.ai_wings.as_ref().unwrap();
    assert_eq!(
        wings.mission().wing_leader(ai_wings::FRIENDLY_SIDE, 0),
        Some(0)
    );
}

#[test]
fn an_open_mission_refuses_what_a_networked_mission_does_not_fly() {
    let map = resources();
    let refused = |edit: &dyn Fn(&mut MissionSpec)| {
        let mut spec = spec(2, 2, 2);
        edit(&mut spec);
        match World::new(&spec, &map, Seating::Open) {
            Ok(_) => panic!("the mission built"),
            Err(error) => error.to_string(),
        }
    };
    assert!(refused(&|s| s.fixture_wings = true).contains("needs the AI"));
    assert!(refused(&|s| s.researched_flight = false).contains("hybrid"));
    assert!(refused(&|s| s.ai_flight_model = ai_wings::AiFlightModel::Standard).contains("hybrid"));
    let player = aircraft_type::AircraftType::load(&map, AircraftId::F18).unwrap();
    let standard = tore_sim::combat::loadout::Loadout::new(&player.profile, |name| {
        map.get(name)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    })
    .unwrap();
    let load = crate::mission::LoadoutSpec::of(&standard);
    assert!(refused(&|s| s.loadout = Some(load.clone())).contains("loadout"));
}

/// The acceptance run: 1,200 ticks with nobody aboard, then seat 0 takes plane
/// 0, flies it on scripted input for two seconds and gives it back, and the AI
/// flies on. Returns the digests along the way.
fn no_human_then_plane_zero(map: &std::collections::BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut world = World::new(&spec(2, 2, 5), map, Seating::Open).unwrap();
    let mut out = TickOutput::default();
    let mut digests = Vec::new();
    let start = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(0)
        .unwrap()
        .flight()
        .position;
    for _ in 0..1200 {
        world.step(&[], &mut out).unwrap();
    }
    assert_eq!(world.tick(), 1200);
    assert!(world.cockpits.is_empty());
    assert_eq!(world.combat.render_plane(), None, "nobody to draw for");
    let wings = world.ai_wings.as_ref().unwrap();
    let lead = wings.mission().actor(0).unwrap().flight();
    assert!(lead.position.iter().all(|v| v.is_finite()));
    assert!(
        (lead.position[0] - start[0]).hypot(lead.position[2] - start[2]) > 5000.,
        "the AI flew plane 0"
    );
    digests.push(digest(&world));

    // Seat 0 takes plane 0: the AI's lead becomes a human's.
    let take = MissionCommand::Take {
        seat: SeatId(0),
        plane: PlaneId(0),
    };
    let before = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(0)
        .unwrap()
        .flight()
        .clone();
    tick(&mut world, &[take], &[0], false, &mut out);
    assert_consistent(&world);
    assert_eq!(world.roster.seat_of(PlaneId(0)), Some(SeatId(0)));
    assert_eq!(world.picture_plane(), PlaneId(0));
    assert_eq!(world.combat.render_snapshot().player.id, 0);
    let flown = &world.cockpits[0].flight;
    assert!(
        (0..3)
            .map(|i| (flown.position[i] - before.position[i]).powi(2))
            .sum::<f64>()
            .sqrt()
            < before.speed / 120. * 2. + 1.,
        "one tick of flight"
    );
    digests.push(digest(&world));
    for step in 0..240 {
        tick(&mut world, &[], &[0], step % 60 < 20, &mut out);
    }
    assert!(world.combat.render_snapshot().player.id == 0);
    assert!(world.combat.previous_snapshot().is_some());
    digests.push(digest(&world));

    // The last human gives the last plane back; the AI flies on.
    tick(
        &mut world,
        &[MissionCommand::GiveBack { seat: SeatId(0) }],
        &[],
        false,
        &mut out,
    );
    assert_consistent(&world);
    assert!(world.cockpits.is_empty());
    assert!(world.combat.state.ownships().is_empty());
    assert_eq!(world.combat.render_plane(), None);
    assert_eq!(
        world.roster.seat(SeatId(0)).unwrap().plane,
        None,
        "the seat waits"
    );
    let handed = world.ai_wings.as_ref().unwrap().mission().actor(0).unwrap();
    assert!(handed.identity().is_leader());
    let position = handed.flight().position;
    for _ in 0..600 {
        world.step(&[], &mut out).unwrap();
    }
    let lead = world.ai_wings.as_ref().unwrap().mission().actor(0).unwrap();
    assert!(lead.alive());
    assert!(
        (lead.flight().position[0] - position[0]).hypot(lead.flight().position[2] - position[2])
            > 2000.
    );
    digests.push(digest(&world));
    digests
}

#[test]
fn an_open_mission_flies_with_nobody_and_hands_plane_zero_both_ways() {
    let map = resources();
    let first = no_human_then_plane_zero(&map);
    assert_eq!(first.len(), 4);
    // The same seed, the same run, to the bit.
    assert_eq!(first, no_human_then_plane_zero(&map));
}

/// A gun burst of a human's plane: the victim is put where the burst's first
/// round will be, as the stage B fight does (`fight_tests.rs`), since the
/// synthetic AI finds nobody to fight on its own.
struct Burst {
    start: usize,
    shooter: PlaneId,
    victim: PlaneId,
}

/// The synthetic gun's muzzle speed, feet per second, and the ticks its first
/// round takes to reach the victim.
const ROUND_FPS: f64 = 1032.;
const FLIGHT: f64 = 40.;
/// The trigger is held for two rounds; the victim is moved away after this.
const HELD: usize = 16;
const CLEAR: usize = 90;

/// A plane's flight, whoever flies it.
fn flight_of(world: &mut World, plane: PlaneId) -> &mut tore_sim::flight::State {
    if let Some(index) = world.cockpits.iter().position(|c| c.plane == plane) {
        return &mut world.cockpits[index].flight;
    }
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .mission_mut()
        .actor_mut(plane.0)
        .expect("an AI plane")
        .flight_mut()
}

/// Moves `victim` to `position`, flying as `like` does, row and all.
fn put(world: &mut World, victim: PlaneId, position: [f64; 3], like: &tore_sim::flight::State) {
    let flight = flight_of(world, victim);
    flight.position = position;
    flight.velocity = like.velocity;
    (flight.yaw, flight.pitch, flight.bank) = (like.yaw, like.pitch, like.bank);
    flight.speed = like.speed;
    if let Some(row) = world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == victim.0)
    {
        row.position = position;
    }
}

fn aim(world: &mut World, burst: &Burst) {
    let shooter = flight_of(world, burst.shooter).clone();
    let forward = attitude::Basis::new(shooter.yaw, shooter.pitch, shooter.bank).forward;
    let position = std::array::from_fn(|i| {
        shooter.position[i] + (forward[i] * ROUND_FPS - shooter.velocity[i]) / 120. * FLIGHT
    });
    put(world, burst.victim, position, &shooter);
}

fn clear(world: &mut World, burst: &Burst) {
    let shooter = flight_of(world, burst.shooter).clone();
    let right = attitude::Basis::new(shooter.yaw, shooter.pitch, shooter.bank).right;
    let victim = flight_of(world, burst.victim).clone();
    let position = std::array::from_fn(|i| victim.position[i] + right[i] * 4000.);
    put(world, burst.victim, position, &victim);
}

/// Two seats take and give back planes in both wings through a fight, human
/// against AI and human against human; the last one gives its plane back and
/// the mission goes on with no human.
#[test]
fn two_seats_hand_planes_in_both_wings_through_a_fight_and_leave() {
    // Planes 0 to 2 are Friendly Wing 1, 3 to 5 the enemy's Wing 1.
    let schedule = |step: usize| match step {
        60 => Some(MissionCommand::Take {
            seat: SeatId(1),
            plane: PlaneId(0),
        }),
        61 => Some(MissionCommand::Take {
            seat: SeatId(2),
            plane: PlaneId(3),
        }),
        700 => Some(MissionCommand::GiveBack { seat: SeatId(1) }),
        800 => Some(MissionCommand::Take {
            seat: SeatId(1),
            plane: PlaneId(4),
        }),
        1000 => Some(MissionCommand::GiveBack { seat: SeatId(2) }),
        1100 => Some(MissionCommand::Take {
            seat: SeatId(2),
            plane: PlaneId(2),
        }),
        1500 => Some(MissionCommand::GiveBack { seat: SeatId(2) }),
        1700 => Some(MissionCommand::GiveBack { seat: SeatId(1) }),
        _ => None,
    };
    let bursts = [
        // A human shoots an AI aircraft, another human, and an AI aircraft
        // of its own former side.
        Burst {
            start: 200,
            shooter: PlaneId(0),
            victim: PlaneId(5),
        },
        Burst {
            start: 400,
            shooter: PlaneId(3),
            victim: PlaneId(0),
        },
        Burst {
            start: 900,
            shooter: PlaneId(4),
            victim: PlaneId(1),
        },
        Burst {
            start: 1250,
            shooter: PlaneId(2),
            victim: PlaneId(5),
        },
    ];
    let map = resources();
    let mut world = World::new(&spec(3, 3, 1), &map, Seating::Open).unwrap();
    let mut out = TickOutput::default();
    let mut seats: Vec<u8> = Vec::new();
    let mut handoffs = 0;
    let mut hits = Vec::new();
    for step in 0..2400usize {
        for burst in &bursts {
            if step == burst.start {
                aim(&mut world, burst);
            }
            if step == burst.start + CLEAR {
                clear(&mut world, burst);
            }
        }
        let held = bursts
            .iter()
            .any(|b| (b.start..b.start + HELD).contains(&step));
        let command = schedule(step);
        match command {
            Some(MissionCommand::Take { seat, plane }) => {
                world.can_take(seat, plane).unwrap();
                seats.push(seat.0);
                seats.sort_unstable();
                handoffs += 1;
            }
            Some(MissionCommand::GiveBack { seat }) => {
                world.can_give_back(seat).unwrap();
                seats.retain(|s| *s != seat.0);
                handoffs += 1;
            }
            _ => {}
        }
        let commands: Vec<MissionCommand> = command.into_iter().collect();
        tick(&mut world, &commands, &seats, held, &mut out);
        for outcome in &out.outcomes {
            if matches!(
                outcome.resolution,
                tore_sim::combat::ledger::Resolution::Hit(_)
            ) {
                hits.push(outcome.key.owner);
            }
        }
        if step % 100 == 0 || !commands.is_empty() {
            assert_consistent(&world);
        }
    }
    assert_consistent(&world);
    assert_eq!(handoffs, 8);
    // Every burst's shooter hit.
    for burst in &bursts {
        assert!(
            hits.contains(&burst.shooter.0),
            "plane {} hit",
            burst.shooter.0
        );
    }
    // The damage stays with the planes through the handoffs.
    let hurt = |id: u32| {
        let row = world
            .combat
            .state
            .targets
            .iter()
            .find(|t| t.id == id)
            .unwrap();
        row.hp < row.initial_hp
    };
    assert!(hurt(0) && hurt(1) && hurt(5));
    // Nobody is left: every seat waits, every plane is the AI's, and the
    // mission steps on.
    assert!(world.cockpits.is_empty());
    assert!(world.roster.seats().iter().all(|seat| seat.plane.is_none()));
    assert_eq!(world.roster.seats().len(), 2);
    for side in [Side::Friendly, Side::Enemy] {
        assert!(
            world
                .roster
                .planes()
                .iter()
                .any(|p| p.slot.wing.side == side)
        );
    }
    let tick_before = world.tick();
    for _ in 0..120 {
        world.step(&[], &mut out).unwrap();
    }
    assert_eq!(world.tick(), tick_before + 120);
}
