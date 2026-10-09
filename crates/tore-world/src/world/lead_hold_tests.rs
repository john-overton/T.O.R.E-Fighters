//! The lead hold's state machine in the mission core (the lobby pass's slice
//! R2; docs/ARCHITECTURE.md, "Lead succession"), on an open mission built
//! from synthetic resources as a host builds it, with no enemy so nothing
//! but the tests loses an aircraft. Friendly Wing 1 is planes 0 to 3,
//! Friendly Wing 2 plane 4; a revival or a respawn adds plane 5 first.
//!
//! One test per transition of the plan's diagram (AiLed, HumanLed, HeldLead
//! and Empty), then `respawn none` (the hold off) and single player.

use super::*;
use crate::{
    ai_wings::FRIENDLY_SIDE,
    comms::journal,
    mission::{MissionSpec, Start},
    seats::{PlaneId, SeatId, SeatInput},
    test_support::resources::{THEATER, resources},
    world::{MissionCommand, Seating, TickOutput, revive::RevivalWeapons},
};
use tore_formats::aircraft::AircraftId;
use tore_sim::{ai::mission::LeadershipChange, sensors::FEET_PER_NAUTICAL_MILE};

const WING: WingId = WingId {
    side: Side::Friendly,
    index: 0,
};
const SEAT_0: SeatId = SeatId(0);
const SEAT_1: SeatId = SeatId(1);
/// The first plane a revival or a respawn adds.
const NEW: u32 = 5;

/// The open mission, every plane on the AI, the lead hold on when `hold`.
fn mission(hold: bool) -> World {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 4;
    spec.wings[1].count = 1;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    let mut world = World::new(&spec, &resources(), Seating::Open).unwrap();
    if hold {
        run(&mut world, &[MissionCommand::LeadHold { on: true }], 1);
    }
    world
}

/// What the ticks of [`run`] did.
#[derive(Default)]
struct Flown {
    changes: Vec<LeadershipChange>,
    /// The lead hold's HUD lines ("You lead ...").
    messages: Vec<(SeatId, String)>,
    /// The comms journal's notes that lead passed on (new, previous): made
    /// for each change the radio heard, which a stand-in's and a given-back
    /// lead's are not.
    notes: Vec<(u32, u32)>,
}

/// `ticks` ticks, the first with `commands`, each with neutral input for
/// every seat that flies.
fn run(world: &mut World, commands: &[MissionCommand], ticks: usize) -> Flown {
    let mut flown = Flown::default();
    for tick in 0..ticks {
        let commands = if tick == 0 { commands } else { &[] };
        let mut flying: Vec<SeatId> = world
            .roster
            .seats()
            .iter()
            .filter(|seat| seat.plane.is_some())
            .map(|seat| seat.id)
            .collect();
        for command in commands {
            match command {
                MissionCommand::Take { seat, .. } | MissionCommand::ReviveLost { seat, .. } => {
                    flying.push(*seat)
                }
                MissionCommand::GiveBack { seat } | MissionCommand::Abandon { seat } => {
                    flying.retain(|s| s != seat)
                }
                _ => {}
            }
        }
        flying.sort();
        flying.dedup();
        let now = world.tick();
        let inputs: Vec<SeatInput> = flying
            .into_iter()
            .map(|seat| SeatInput {
                seat,
                tick: now,
                ..SeatInput::default()
            })
            .collect();
        let mut out = TickOutput::default();
        world
            .step_with(commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
        flown.changes.extend(
            world
                .ai_wings
                .as_ref()
                .unwrap()
                .last_output()
                .leadership
                .iter()
                .copied(),
        );
        // The lead hold's own HUD lines, not the AI wings' activity lines.
        for cue in out.cues {
            if let Cue::Message { seat, text } = cue
                && text.starts_with("You lead")
            {
                flown.messages.push((seat, text));
            }
        }
        for entry in world.comms.take_journal() {
            if let journal::Cause::Leadership { new, previous, .. } = entry.origin.cause
                && entry.outcome == journal::Outcome::Noted
            {
                flown.notes.push((new, previous));
            }
        }
    }
    flown
}

fn take(seat: SeatId, plane: u32) -> MissionCommand {
    MissionCommand::Take {
        seat,
        plane: PlaneId(plane),
    }
}

/// Wing 1's leader now.
fn leader(world: &World) -> Option<u32> {
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .wing_leader(FRIENDLY_SIDE, 0)
}

/// `plane` is lost: a human's pilot dies, an AI aircraft is shot down.
fn lose(world: &mut World, plane: u32) {
    if let Some(cockpit) = world.cockpits.iter_mut().find(|c| c.plane.0 == plane) {
        cockpit.flight.systems.pilot.dead = true;
    } else {
        world
            .combat
            .state
            .targets
            .iter_mut()
            .find(|t| t.id == plane)
            .unwrap()
            .hp = 0;
    }
}

/// `seat` revives in a new plane of its lost plane's wing, 10 nm off.
fn revive(world: &World, seat: SeatId) -> MissionCommand {
    let start = world.side_mean(Side::Friendly).unwrap();
    let spawn = world
        .revival_spawn(
            seat,
            start,
            10. * FEET_PER_NAUTICAL_MILE,
            None,
            RevivalWeapons::Missiles,
        )
        .unwrap();
    MissionCommand::Revive {
        seat,
        spawn: Box::new(spawn),
    }
}

/// The AI respawns `root`'s lineage at its side's mean place.
fn respawn(world: &World, root: u32) -> MissionCommand {
    let root = PlaneId(root);
    let at = world.side_mean(Side::Friendly).unwrap();
    let spawn = world
        .respawn_spawn(root, at, 0., &[], None, RevivalWeapons::Missiles)
        .unwrap();
    MissionCommand::Respawn {
        root,
        spawn: Box::new(spawn),
    }
}

fn owner(world: &World) -> Option<LeadOwner> {
    world.lead_owner(WING)
}

/// The one change of `flown`, as (leader, previous, acting, reclaimed).
fn only_change(flown: &Flown) -> (u32, u32, bool, bool) {
    assert_eq!(flown.changes.len(), 1, "{:?}", flown.changes);
    let c = flown.changes[0];
    (c.leader, c.previous, c.acting, c.reclaimed)
}

// ---------------------------------------------------------------------------
// AiLed.

/// AiLed to AiLed: in a flight with no human the lead passes down the
/// line, and a respawned aircraft joins last, as a wingman, and leads only
/// when every earlier member is gone (John's AI leadership loop).
#[test]
fn an_ai_flight_passes_the_lead_down_and_respawns_join_last() {
    let mut world = mission(true);
    run(&mut world, &[], 2);
    assert_eq!(leader(&world), Some(0));
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (1, 0, false, false));
    assert!(world.lead_owners().is_empty() && !world.lead_acting(WING));
    // Plane 0's lineage respawns: plane 5, member 4, a wingman of plane 1.
    let command = respawn(&world, 0);
    let flown = run(&mut world, &[command], 2);
    assert!(flown.changes.is_empty());
    assert_eq!(world.lineage_head(PlaneId(0)), PlaneId(NEW));
    assert_eq!(world.roster.plane(PlaneId(NEW)).unwrap().slot.member, 4);
    assert_eq!(leader(&world), Some(1));
    let actor = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(NEW)
        .unwrap();
    assert!(!actor.identity().is_leader());
    assert_eq!(actor.wing_slot(), 3, "behind planes 2 and 3");
    // The loop goes on: planes 2 and 3, then the respawn.
    for (lost, next) in [(1, 2), (2, 3), (3, NEW)] {
        lose(&mut world, lost);
        let flown = run(&mut world, &[], 2);
        assert_eq!(only_change(&flown), (next, lost, false, false));
    }
    assert!(world.lead_owners().is_empty());
}

/// AiLed to HumanLed by the handoff: a human takes the leading plane and
/// owns the lead; another wing's AI lead is nobody's.
#[test]
fn a_human_who_takes_the_leading_plane_owns_the_lead() {
    let mut world = mission(true);
    let flown = run(&mut world, &[take(SEAT_0, 0)], 2);
    assert!(flown.changes.is_empty());
    assert_eq!(
        world.lead_owners(),
        [Owned {
            wing: WING,
            owner: LeadOwner::Seat(SEAT_0),
            plane: PlaneId(0),
            led: true,
        }]
    );
    assert_eq!(leader(&world), Some(0));
    assert!(!world.lead_acting(WING));
    assert_eq!(
        world.lead_owner(WingId {
            side: Side::Friendly,
            index: 1
        }),
        None
    );
}

/// AiLed to HumanLed: the AI lead is lost while a human of the flight
/// flies; the human leads, hears today's call, and owns the lead.
#[test]
fn an_ai_lead_lost_with_a_human_flying_makes_it_the_lead_and_owner() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 2)], 2);
    assert!(
        world.lead_owners().is_empty(),
        "a human wingman owns nothing"
    );
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (2, 0, false, false));
    assert_eq!(flown.notes, [(2, 0)], "today's call");
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_0)));
    assert!(world.lead_owners()[0].led);
}

/// AiLed to HeldLead, then HumanLed: the AI lead is lost while the flight's
/// only human waits to revive; the next AI stands in for the human, who
/// owns the lead and takes it on revival as a new lead (today's call).
#[test]
fn an_ai_lead_lost_while_the_only_human_waits_to_revive_is_held_for_it() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 2)], 2);
    lose(&mut world, 2);
    run(&mut world, &[], 2);
    assert!(world.lead_owners().is_empty(), "plane 0 still leads");
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (1, 0, false, false));
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_0)));
    assert!(!world.lead_owners()[0].led);
    assert!(
        world.lead_acting(WING),
        "plane 1 stands in from the next step"
    );
    let command = revive(&world, SEAT_0);
    let flown = run(&mut world, &[command], 2);
    assert_eq!(leader(&world), Some(NEW));
    assert_eq!(only_change(&flown), (NEW, 1, false, false));
    assert_eq!(flown.notes, [(NEW, 1)], "a new lead hears today's call");
    assert!(flown.messages.is_empty(), "no line for a new lead");
    assert!(world.lead_owners()[0].led);
}

// ---------------------------------------------------------------------------
// HumanLed and HeldLead.

/// HumanLed to HeldLead and back: the owner is lost and an AI wingman
/// stands in (no call), the flight flies its mission of opportunity, and
/// the owner keeps the lead while it waits (in flight or in the lobby with a
/// held seat, which the mission core cannot tell apart). Its revived plane
/// takes the lead back at once, with its HUD line, and the flight re-forms
/// on it in member order.
#[test]
fn a_lost_owner_is_stood_in_for_and_takes_the_lead_back_on_revival() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (1, 0, true, false));
    assert!(flown.notes.is_empty(), "no Wingleader call for a stand-in");
    assert!(flown.messages.is_empty(), "an AI stand-in reads nothing");
    assert!(world.lead_acting(WING));
    let wings = world.ai_wings.as_ref().unwrap();
    assert_eq!(wings.mission().opportunities().len(), 1);
    // A long wait changes nothing.
    run(&mut world, &[], 600);
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_0)));
    assert_eq!((leader(&world), world.lead_acting(WING)), (Some(1), true));
    // The owner revives in plane 5.
    let command = revive(&world, SEAT_0);
    let flown = run(&mut world, &[command], 2);
    assert_eq!(only_change(&flown), (NEW, 1, false, true));
    assert!(flown.notes.is_empty());
    assert_eq!(
        flown.messages,
        [(SEAT_0, crate::radio_calls::LEAD_AGAIN.to_owned())]
    );
    assert_eq!(world.lead_owners()[0].plane, PlaneId(NEW));
    assert!(!world.lead_acting(WING));
    let mission = world.ai_wings.as_ref().unwrap().mission();
    assert!(mission.opportunities().is_empty());
    let slots: Vec<u8> = [1, 2, 3]
        .iter()
        .map(|id| mission.actor(*id).unwrap().wing_slot())
        .collect();
    assert_eq!(slots, [1, 2, 3], "the flight re-forms in member order");
}

/// HumanLed to HeldLead: a flying human of the flight stands in before any
/// AI, reads its line naming the owner's plane, and is never made owner.
#[test]
fn a_human_stand_in_comes_first_and_reads_its_line() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 1);
    run(&mut world, &[take(SEAT_1, 2)], 2);
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (2, 0, true, false));
    assert!(flown.notes.is_empty());
    let members = crate::radio_calls::members(&world.roster, world.ai_wings.as_ref(), |_| true);
    let name = crate::radio_calls::label(members.iter().find(|m| m.id == 0).unwrap());
    assert_eq!(
        flown.messages,
        [(SEAT_1, crate::radio_calls::stand_in_line(Some(&name)))]
    );
    assert!(
        flown.messages[0].1.contains("until "),
        "{}",
        flown.messages[0].1
    );
    run(&mut world, &[], 120);
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_0)));
    let command = revive(&world, SEAT_0);
    let flown = run(&mut world, &[command], 2);
    assert_eq!(only_change(&flown), (NEW, 2, false, true));
    assert_eq!(
        flown.messages,
        [(SEAT_0, crate::radio_calls::LEAD_AGAIN.to_owned())]
    );
}

/// HeldLead to HeldLead: a lost stand-in passes the stand-in's place to
/// the next member flying.
#[test]
fn a_lost_stand_in_passes_to_the_next_stand_in() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    lose(&mut world, 0);
    run(&mut world, &[], 2);
    lose(&mut world, 1);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (2, 1, true, false));
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_0)));
}

/// HumanLed to HumanLed: the flying owner leaves the game (its plane goes
/// back to the AI, then the host says it left) and the next human of the
/// flight, flying, owns the lead and leads at once with today's call.
#[test]
fn an_owner_who_leaves_passes_the_lead_to_the_next_human_flying() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 1);
    run(&mut world, &[take(SEAT_1, 2)], 2);
    run(&mut world, &[MissionCommand::GiveBack { seat: SEAT_0 }], 1);
    assert_eq!(owner(&world), Some(LeadOwner::Away(PlaneId(0))));
    assert_eq!(leader(&world), Some(0), "the AI flies plane 0 and it leads");
    let left = MissionCommand::LeadLeft {
        owner: LeadOwner::Away(PlaneId(0)),
    };
    let flown = run(&mut world, &[left], 2);
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_1)));
    assert_eq!(only_change(&flown), (2, 0, false, false));
    assert_eq!(flown.notes, [(2, 0)], "the new lead hears today's call");
    assert!(world.lead_owners()[0].led);
}

/// HeldLead to HumanLed: the lost owner leaves the game while a human
/// stands in; the stand-in owns the lead and leads as itself.
#[test]
fn an_owner_who_leaves_while_a_human_stands_in_makes_it_the_owner() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 1);
    run(&mut world, &[take(SEAT_1, 2)], 2);
    lose(&mut world, 0);
    run(&mut world, &[], 2);
    assert!(world.lead_acting(WING));
    let left = [
        MissionCommand::LeadLeft {
            owner: LeadOwner::Seat(SEAT_0),
        },
        MissionCommand::Abandon { seat: SEAT_0 },
    ];
    let flown = run(&mut world, &left, 2);
    assert!(flown.changes.is_empty());
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_1)));
    assert_eq!((leader(&world), world.lead_acting(WING)), (Some(2), false));
}

/// HeldLead to HeldLead: the lost owner leaves the game and the next human
/// of the flight also waits to revive: it owns the held lead, and its
/// revived plane takes it as a new lead.
#[test]
fn an_owner_who_leaves_passes_the_held_lead_to_the_next_human_waiting() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 1);
    run(&mut world, &[take(SEAT_1, 2)], 2);
    lose(&mut world, 2);
    run(&mut world, &[], 2);
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (1, 0, true, false));
    let left = [
        MissionCommand::LeadLeft {
            owner: LeadOwner::Seat(SEAT_0),
        },
        MissionCommand::Abandon { seat: SEAT_0 },
    ];
    run(&mut world, &left, 2);
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_1)));
    assert_eq!((leader(&world), world.lead_acting(WING)), (Some(1), true));
    let command = revive(&world, SEAT_1);
    let flown = run(&mut world, &[command], 2);
    assert_eq!(only_change(&flown), (NEW, 1, false, false));
    assert_eq!(flown.notes, [(NEW, 1)]);
}

/// HumanLed to AiLed: the flying owner leaves and no other human is in the
/// flight: the AI keeps the plane and the lead, and the AI loop goes on.
#[test]
fn an_owner_who_leaves_a_flight_of_ai_leaves_it_to_the_ai_loop() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    run(&mut world, &[MissionCommand::GiveBack { seat: SEAT_0 }], 1);
    let left = MissionCommand::LeadLeft {
        owner: LeadOwner::Away(PlaneId(0)),
    };
    let flown = run(&mut world, &[left], 2);
    assert!(flown.changes.is_empty());
    assert!(world.lead_owners().is_empty());
    assert_eq!((leader(&world), world.lead_acting(WING)), (Some(0), false));
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (1, 0, false, false));
}

/// HeldLead to AiLed: the lost owner leaves and no other human is in the
/// flight: the stand-in becomes the ordinary lead.
#[test]
fn an_owner_who_leaves_while_the_ai_stands_in_leaves_it_the_lead() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    lose(&mut world, 0);
    run(&mut world, &[], 2);
    let left = [
        MissionCommand::LeadLeft {
            owner: LeadOwner::Seat(SEAT_0),
        },
        MissionCommand::Abandon { seat: SEAT_0 },
    ];
    run(&mut world, &left, 2);
    assert!(world.lead_owners().is_empty());
    assert_eq!((leader(&world), world.lead_acting(WING)), (Some(1), false));
    lose(&mut world, 1);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (2, 1, false, false));
}

/// HumanLed while away: the owner's plane goes to the AI (away), which
/// keeps its lead and the owner; taking it back changes nothing.
#[test]
fn an_away_owner_keeps_the_lead_and_takes_its_plane_back_unchanged() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    let flown = run(&mut world, &[MissionCommand::GiveBack { seat: SEAT_0 }], 30);
    assert!(flown.changes.is_empty());
    assert_eq!(owner(&world), Some(LeadOwner::Away(PlaneId(0))));
    assert_eq!((leader(&world), world.lead_acting(WING)), (Some(0), false));
    // Back, in another seat.
    let flown = run(&mut world, &[take(SeatId(3), 0)], 2);
    assert!(flown.changes.is_empty());
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SeatId(3))));
    assert_eq!(leader(&world), Some(0));
}

/// HumanLed while away, then HeldLead: the AI loses the away owner's plane,
/// an AI wingman stands in, and the player back revives from the lost plane
/// (slice K5's ReviveLost) and takes the lead back.
#[test]
fn an_away_owner_whose_plane_is_lost_takes_the_lead_back_on_return() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    run(&mut world, &[MissionCommand::GiveBack { seat: SEAT_0 }], 2);
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (1, 0, true, false));
    assert_eq!(owner(&world), Some(LeadOwner::Away(PlaneId(0))));
    let start = world.side_mean(Side::Friendly).unwrap();
    let spawn = world
        .revival_spawn_from(
            PlaneId(0),
            start,
            10. * FEET_PER_NAUTICAL_MILE,
            None,
            RevivalWeapons::Missiles,
        )
        .unwrap();
    let back = MissionCommand::ReviveLost {
        seat: SeatId(2),
        plane: PlaneId(0),
        spawn: Box::new(spawn),
    };
    let flown = run(&mut world, &[back], 2);
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SeatId(2))));
    assert_eq!(only_change(&flown), (NEW, 1, false, true));
    assert_eq!(
        flown.messages,
        [(SeatId(2), crate::radio_calls::LEAD_AGAIN.to_owned())]
    );
}

/// An owner whose revival lands in another wing (an `ai-slot` revival
/// elsewhere) has left the flight: the next human of it owns the lead.
#[test]
fn an_owner_who_flies_in_another_wing_has_left_the_flight() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 1);
    run(&mut world, &[take(SEAT_1, 2)], 2);
    lose(&mut world, 0);
    run(&mut world, &[], 2);
    let elsewhere = [MissionCommand::Abandon { seat: SEAT_0 }, take(SEAT_0, 4)];
    run(&mut world, &elsewhere, 2);
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_1)));
    assert_eq!((leader(&world), world.lead_acting(WING)), (Some(2), false));
    // The owner now leads its new wing, as its own.
    assert_eq!(
        world.lead_owner(WingId {
            side: Side::Friendly,
            index: 1
        }),
        Some(LeadOwner::Seat(SEAT_0))
    );
}

// ---------------------------------------------------------------------------
// Empty.

/// Empty to AiLed: a flight with nobody flying keeps its lost lead until
/// its first respawn arrives, which leads.
#[test]
fn an_empty_ai_flight_is_led_by_its_first_respawn() {
    let mut world = mission(true);
    run(&mut world, &[], 2);
    for plane in 0..4 {
        lose(&mut world, plane);
    }
    let flown = run(&mut world, &[], 2);
    assert!(flown.changes.is_empty(), "nobody flies to take the lead");
    assert_eq!(leader(&world), Some(0));
    let command = respawn(&world, 2);
    let flown = run(&mut world, &[command], 2);
    assert_eq!(only_change(&flown), (NEW, 0, false, false));
}

/// Empty to HumanLed: the owner and every wingman are lost; the owner's
/// revived plane takes the lead back.
#[test]
fn an_empty_flight_is_led_again_by_its_revived_owner() {
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    for plane in 0..4 {
        lose(&mut world, plane);
    }
    let flown = run(&mut world, &[], 2);
    assert!(flown.changes.is_empty());
    let command = revive(&world, SEAT_0);
    let flown = run(&mut world, &[command], 2);
    assert_eq!(only_change(&flown), (NEW, 0, false, true));
    assert_eq!(owner(&world), Some(LeadOwner::Seat(SEAT_0)));
}

// ---------------------------------------------------------------------------
// The hold off.

/// `respawn none` (the hold off): today's rule. A lost human lead's
/// successor is the ordinary lead, nobody owns anything, and a revived
/// human is a wingman. Turning the hold off clears its owners.
#[test]
fn with_the_hold_off_the_succession_is_todays() {
    let mut world = mission(false);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    assert!(!world.lead_hold() && world.lead_owners().is_empty());
    lose(&mut world, 0);
    let flown = run(&mut world, &[], 2);
    assert_eq!(only_change(&flown), (1, 0, false, false));
    assert_eq!(flown.notes, [(1, 0)]);
    let command = revive(&world, SEAT_0);
    let flown = run(&mut world, &[command], 2);
    assert!(flown.changes.is_empty());
    assert_eq!(leader(&world), Some(1));
    let ai = world.ai_wings.as_ref().unwrap().mission();
    assert!(ai.lead_claims().is_empty());
    // On, then off again.
    let mut world = mission(true);
    run(&mut world, &[take(SEAT_0, 0)], 2);
    assert!(!world.lead_owners().is_empty());
    run(&mut world, &[MissionCommand::LeadHold { on: false }], 2);
    assert!(world.lead_owners().is_empty());
    let ai = world.ai_wings.as_ref().unwrap().mission();
    assert!(ai.lead_claims().is_empty());
}

/// Single player never holds a lead: no claim reaches its AI mission and
/// nobody stands in when its AI leads pass on.
#[test]
fn single_player_never_holds_a_lead() {
    let mut world = super::super::tick_tests::mission();
    let mut out = TickOutput::default();
    for _ in 0..240 {
        let inputs = [SeatInput {
            seat: SeatId(0),
            tick: world.tick(),
            ..SeatInput::default()
        }];
        world.step(&inputs, &mut out).unwrap();
        let wings = world.ai_wings.as_ref().unwrap();
        assert!(wings.mission().lead_claims().is_empty());
        assert!(
            wings
                .last_output()
                .leadership
                .iter()
                .all(|c| !c.acting && !c.reclaimed)
        );
    }
    assert!(!world.lead_hold() && world.lead_owners().is_empty());
}
