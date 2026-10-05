//! The data link's picture on the crowd fixture (docs/ARCHITECTURE.md, "Flight
//! data link", slice G0): the members and their radar flag, the engagements and
//! locks against the ownships and controllers they come from, the publishing
//! tick of the tracks, and that two runs agree. Nothing consumes the picture
//! yet, so the single-player fingerprints do not move.

use super::crowd::*;
use super::*;
use crate::datalink::{self, Entry, Fuel, Weapons};
use crate::seats::SeatCommand;
use std::collections::BTreeSet;
use tore_formats::aircraft::AircraftId;
use tore_input::{PilotCommand, PilotInput, Switch};
use tore_sim::{
    ai::weapon_service::{Phase, Rounds},
    combat::live::Command,
    datalink::has_radar,
};

/// The aircraft each human designates: an AI aircraft of the other side, which
/// the wing orders accept as a target.
fn enemy_of(plane: PlaneId) -> PlaneId {
    if plane.0 < 4 { E_AI[0] } else { F_AI[0] }
}

/// One seat's input: radar on at tick 10, and a designation of an AI aircraft
/// of the other side at tick 40.
fn script(world: &World, seat: SeatId, tick: usize) -> SeatInput {
    let plane = world.roster.seats()[usize::from(seat.0)].plane.unwrap();
    let mut pilot = PilotInput::default();
    if tick == 10 {
        pilot.commands.push(PilotCommand::Set(Switch::Radar, true));
    }
    let mut commands = Vec::new();
    if tick == 40 {
        commands.push(SeatCommand::Combat(Command::DesignateTarget(
            enemy_of(plane).0,
        )));
    }
    SeatInput {
        pilot,
        commands,
        ..SeatInput::default()
    }
}

/// The fixture's weapon records carry no capability flags, so the AI's stations
/// take none: it would never find a store eligible against an aircraft. Give
/// the gun and the missile the capability a real record's flags give them.
fn arm_the_ai(world: &mut World) {
    use tore_sim::ai::weapon_service::StoreCapability;
    let wings = world.ai_wings.as_mut().unwrap();
    for id in 1..=7 {
        let actor = wings.mission_mut().actor_mut(id).unwrap();
        let stations = actor.stations_mut();
        stations[0].guided = false;
        stations[0].capability = StoreCapability::GUN;
        stations[1].guided = true;
        stations[1].capability = StoreCapability::AIR_TO_AIR_MISSILE;
    }
}

/// The fight of `ai_mission`: the enemy wing is all AI and flies at the
/// friendly wing, whose members are the player (plane 0), a second human
/// (plane 1) and two AI aircraft. The wing led by a human joins a fight only
/// when ordered, so the AI aircraft that choose targets here are the enemy's.
fn fight_mission() -> World {
    let mut world = ai_mission();
    arm_the_ai(&mut world);
    world.take_plane(SeatId(1), F_HUMAN).unwrap();
    world
}

/// Steps the mission one tick with the script.
fn step(world: &mut World, tick: usize, out: &mut TickOutput) {
    let step_inputs = inputs(world, |seat| script(world, seat, tick));
    world.step(&step_inputs, out).unwrap();
}

fn pilot_flies(world: &World, plane: u32) -> bool {
    world.cockpits.iter().any(|c| c.plane.0 == plane)
}

#[test]
fn every_plane_is_a_member_with_the_radar_flag_of_its_aircraft() {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    step(&mut world, 0, &mut out);
    let members = world.datalink.members();
    let planes: Vec<u32> = members.iter().map(|m| m.plane).collect();
    assert_eq!(planes, [0, 1, 2, 3, 4, 5, 6, 7], "plane id order");
    for member in members {
        let aircraft = member.aircraft.expect("combat holds an aircraft for each");
        assert_eq!(member.radar, has_radar(aircraft), "plane {}", member.plane);
        assert_eq!(member.human, pilot_flies(&world, member.plane));
        assert!(member.alive);
    }
    // The fixture flies the F/A-18D everywhere, which has a radar.
    assert!(members.iter().all(|m| m.aircraft == Some(AircraftId::F18)));
    assert!(members.iter().all(|m| m.radar));
    // Each plane is announced once, with its radar flag.
    let announced: Vec<Entry> = world
        .datalink
        .take_journal()
        .into_iter()
        .filter(|entry| matches!(entry, Entry::Member { .. }))
        .collect();
    assert_eq!(announced.len(), 8);
    step(&mut world, 1, &mut out);
    assert!(
        world
            .datalink
            .take_journal()
            .iter()
            .all(|entry| !matches!(entry, Entry::Member { .. })),
        "a plane is announced once"
    );
}

#[test]
fn flights_are_the_wings_of_the_roster() {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    step(&mut world, 0, &mut out);
    let flights = world.datalink.flights();
    assert_eq!(flights.len(), 2);
    assert!(!flights[0].side.is_enemy() && flights[1].side.is_enemy());
    for member in world.datalink.members() {
        let wing = world.roster.plane(PlaneId(member.plane)).unwrap().slot;
        assert_eq!((member.flight, member.member), (wing.wing, wing.member));
    }
}

#[test]
fn tracks_change_only_on_ticks_divisible_by_thirty() {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    let mut previous = world.datalink.pictures().to_vec();
    let mut published = 0;
    let mut tracked = 0;
    for tick in 0..400 {
        step(&mut world, tick, &mut out);
        let now = world.tick();
        let pictures = world.datalink.pictures().to_vec();
        if now.is_multiple_of(datalink::PUBLISH_TICKS) {
            published += 1;
            assert!(pictures.iter().all(|picture| picture.tick == now));
            tracked += pictures.iter().map(|p| p.tracks.len()).sum::<usize>();
        } else {
            assert_eq!(pictures, previous, "tick {now} is not a publishing tick");
        }
        previous = pictures;
    }
    assert!(published >= 12, "{published}");
    // The humans' radars find the other wing, and each flight shares what it
    // holds, so the picture is not trivially empty.
    assert!(tracked > 0, "no flight ever published a track");
}

#[test]
fn a_flight_keeps_its_nearest_tracks_one_per_target() {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    for tick in 0..300 {
        step(&mut world, tick, &mut out);
    }
    for picture in world.datalink.pictures() {
        assert!(picture.tracks.len() <= datalink::FLIGHT_TRACKS);
        let targets: BTreeSet<u32> = picture.tracks.iter().map(|t| t.target).collect();
        assert_eq!(targets.len(), picture.tracks.len(), "one per target");
        // Only hostile aircraft: friendly planes are never tracks.
        let side = picture.flight.side;
        for track in &picture.tracks {
            let theirs = world.roster.plane(PlaneId(track.target)).unwrap();
            assert_ne!(theirs.slot.wing.side, side, "track {}", track.target);
            assert!(
                world
                    .datalink
                    .member(track.reporter)
                    .is_some_and(|m| m.flight == picture.flight)
            );
            assert!(track.observed <= picture.tick);
        }
    }
}

#[test]
fn locks_and_engagements_match_the_ownships_and_controllers() {
    let mut world = fight_mission();
    let mut out = TickOutput::default();
    let mut human_locks = 0;
    let mut ai_engaged = 0;
    let mut ai_locked = 0;
    for tick in 0..1500 {
        step(&mut world, tick, &mut out);
        for member in world.datalink.members().to_vec() {
            let lock = world.datalink.lock(member.plane).map(|l| l.target);
            let engaged = world.datalink.engaged(member.plane);
            if member.human {
                let acquired = world
                    .combat
                    .state
                    .ownship(member.plane)
                    .unwrap()
                    .sensors
                    .acquired();
                assert_eq!(lock, acquired, "tick {tick} human {}", member.plane);
                assert_eq!(engaged, acquired);
                human_locks += usize::from(lock.is_some());
            } else {
                let actor = world
                    .ai_wings
                    .as_ref()
                    .unwrap()
                    .mission()
                    .actor(member.plane)
                    .unwrap();
                let (target, locking) = if actor.alive() {
                    let controller = actor.controller();
                    (
                        controller.target(),
                        matches!(controller.weapon_phase(), Phase::Tracking | Phase::Fire),
                    )
                } else {
                    (None, false)
                };
                assert_eq!(engaged, target, "tick {tick} AI {}", member.plane);
                assert_eq!(lock, target.filter(|_| locking));
                ai_engaged += usize::from(engaged.is_some());
                ai_locked += usize::from(lock.is_some());
            }
        }
    }
    // The run exercises each source, not just the empty case.
    assert!(human_locks > 0, "no human ever held a lock");
    assert!(ai_engaged > 0, "no AI aircraft ever chose a target");
    assert!(ai_locked > 0, "no AI aircraft ever held a lock");
}

#[test]
fn locks_are_journaled_when_taken_and_dropped() {
    let mut world = fight_mission();
    let mut out = TickOutput::default();
    let mut journal = Vec::new();
    for tick in 0..1500 {
        step(&mut world, tick, &mut out);
        journal.extend(world.datalink.take_journal());
    }
    // Replaying the journal rebuilds the locks the picture holds now.
    let mut held: std::collections::BTreeMap<u32, u32> = Default::default();
    for entry in &journal {
        match *entry {
            Entry::Lock { plane, target, .. } => {
                assert!(held.insert(plane, target).is_none(), "lock over a lock");
            }
            Entry::Unlock { plane, target, .. } => {
                assert_eq!(held.remove(&plane), Some(target), "unlock of no lock");
            }
            Entry::Member { .. } => {}
        }
    }
    let now: std::collections::BTreeMap<u32, u32> = world
        .datalink
        .locks()
        .iter()
        .map(|(&plane, lock)| (plane, lock.target))
        .collect();
    assert_eq!(held, now);
    assert!(
        journal.iter().any(|e| matches!(e, Entry::Lock { .. })),
        "the fight made no lock"
    );
}

/// Everything the picture holds, as text, so two runs can be compared.
fn digest(world: &World) -> String {
    let link = &world.datalink;
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        link.members(),
        link.pictures(),
        link.locks(),
        link.engagements(),
        link.ai_input()
    )
}

fn run(ticks: usize) -> (String, Vec<Entry>) {
    let mut world = fight_mission();
    let mut out = TickOutput::default();
    let mut log = String::new();
    let mut journal = Vec::new();
    for tick in 0..ticks {
        step(&mut world, tick, &mut out);
        if tick % 30 == 0 || tick + 1 == ticks {
            log.push_str(&digest(&world));
        }
        journal.extend(world.datalink.take_journal());
    }
    (log, journal)
}

#[test]
fn two_runs_agree() {
    let first = run(900);
    let second = run(900);
    assert!(first == second, "the picture differs between two runs");
}

#[test]
fn the_ai_input_lists_only_the_humans_engagements() {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    for tick in 0..400 {
        step(&mut world, tick, &mut out);
    }
    let input = world.datalink.ai_input();
    assert_eq!(input.tick, world.tick());
    assert!(
        input
            .human_engagements
            .iter()
            .all(|e| pilot_flies(&world, e.plane))
    );
    assert_eq!(input.flights.len(), 2);
    for feed in &input.flights {
        let picture = world.datalink.picture(feed.flight).unwrap();
        assert_eq!(
            (feed.published, &feed.tracks),
            (picture.tick, &picture.tracks)
        );
    }
}

#[test]
fn member_state_follows_fuel_weapons_and_damage() {
    let mut world = fight_mission();
    let mut out = TickOutput::default();
    // Plane 1, a human wingman, empties every station and is hurt to half its
    // hit points; AI plane 3 shoots off its missiles but keeps its gun.
    let initial = {
        let own = world.combat.state.ownship_mut(1).unwrap();
        own.ammo.iter_mut().for_each(|a| *a = 0);
        let initial = own.configuration().damage_capacity;
        own.hp = initial / 2;
        initial
    };
    assert!(initial > 2);
    {
        let actor = world
            .ai_wings
            .as_mut()
            .unwrap()
            .mission_mut()
            .actor_mut(3)
            .unwrap();
        actor.stations_mut()[1].store.rounds = Rounds::Finite(0);
    }
    for tick in 0..31 {
        step(&mut world, tick, &mut out);
    }
    let status = |plane: u32| {
        world
            .datalink
            .pictures()
            .iter()
            .flat_map(|p| p.status.iter())
            .find(|s| s.plane == plane)
            .copied()
            .unwrap()
    };
    let hurt = status(1);
    assert_eq!(hurt.weapons, Weapons::Winchester);
    assert_eq!(hurt.damage, datalink::Damage::Heavy);
    assert_eq!(hurt.fuel, Fuel::Normal);
    assert_eq!(status(3).weapons, Weapons::GunsOnly);
    assert_eq!(status(2).weapons, Weapons::Missiles);
    assert_eq!(status(2).damage, datalink::Damage::None);
    // The fixture's missile record has no reviewed profile, so the leader
    // counts as carrying its gun only.
    assert_eq!(status(0).weapons, Weapons::GunsOnly);
}

#[test]
fn ai_members_publish_what_their_awareness_holds() {
    let mut world = fight_mission();
    let mut out = TickOutput::default();
    let mut reporters = BTreeSet::new();
    for tick in 0..600 {
        step(&mut world, tick, &mut out);
        for picture in world.datalink.pictures() {
            reporters.extend(picture.tracks.iter().map(|t| t.reporter));
        }
    }
    let by_ai = |plane: &&u32| world.datalink.member(**plane).is_some_and(|m| !m.human);
    assert!(
        reporters.iter().any(|plane| by_ai(&plane)),
        "no AI aircraft reported a track: {reporters:?}"
    );
    assert!(reporters.iter().any(|plane| !by_ai(&plane)));
}
