//! Lead succession in the mission core (docs/ARCHITECTURE.md, "Lead
//! succession"), with a human lead and with an AI lead. John's rule: when a
//! flight lead is lost, a human in the flight takes the lead if there is one,
//! otherwise the next AI member does, and the flight re-forms on the new
//! leader. A human who takes the lead hears "You're the Wingleader now" five
//! seconds later, voiced by the previous leader when that pilot ejected alive
//! and a HUD line when not; an AI that takes the lead says nothing.
//!
//! Each test loses an aircraft the way the mission does: an AI aircraft by
//! its combat row's hit points, a human's by combat's own damage to its
//! ownship, and a pilot's ejection as `flight::State::eject` leaves the
//! flight (the fixture aircraft has no ejection seat, so it is set by hand).

use super::crowd::*;
use super::*;
use crate::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    comms::journal,
};
use tore_sim::combat::live;

fn step(world: &mut World, out: &mut TickOutput) {
    let step_inputs = inputs(world, |_| SeatInput::default());
    world.step(&step_inputs, out).unwrap();
}

/// Every radio call delivered and every journal entry made while a test
/// flies.
struct Log {
    radio: Vec<(usize, SeatId, comms::Call)>,
    journal: Vec<journal::Entry>,
}

impl Log {
    fn new() -> Self {
        Self {
            radio: Vec::new(),
            journal: Vec::new(),
        }
    }

    fn fly(&mut self, world: &mut World, ticks: usize) {
        let mut out = TickOutput::default();
        for _ in 0..ticks {
            let tick = world.tick() as usize;
            step(world, &mut out);
            self.radio
                .extend(radio_of(&out).into_iter().map(|(s, c)| (tick, s, c)));
            self.journal.extend(world.comms.take_journal());
        }
    }

    /// The calls about lead passing on.
    fn leadership_calls(&self) -> Vec<&(usize, SeatId, comms::Call)> {
        self.radio
            .iter()
            .filter(|r| matches!(r.2.origin.cause, journal::Cause::Leadership { .. }))
            .collect()
    }

    /// The journal's notes that lead passed on, in order: (new, previous,
    /// whether the previous leader's pilot lived).
    fn changes(&self) -> Vec<(u32, u32, bool)> {
        self.journal
            .iter()
            .filter_map(|entry| match entry.origin.cause {
                journal::Cause::Leadership {
                    new,
                    previous,
                    previous_pilot_alive,
                } if entry.outcome == journal::Outcome::Noted => {
                    Some((new, previous, previous_pilot_alive))
                }
                _ => None,
            })
            .collect()
    }
}

fn leader(world: &World, side: live::Side) -> Option<u32> {
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .wing_leader(side, 0)
}

/// The living AI followers among `planes` as (plane, place behind the leader,
/// flying a formation). The AI leader flies its own way and has no place.
fn flight(world: &World, planes: &[PlaneId]) -> Vec<(u32, u8, bool)> {
    let mission = world.ai_wings.as_ref().unwrap().mission();
    planes
        .iter()
        .filter_map(|plane| mission.actor(plane.0))
        .filter(|actor| actor.alive() && !actor.identity().is_leader())
        .map(|actor| {
            (
                actor.id(),
                actor.wing_slot(),
                actor.controller().formation_trace().is_some(),
            )
        })
        .collect()
}

/// Shoots down an AI plane: no hit points left on its combat row.
fn shoot_down_ai(world: &mut World, plane: PlaneId) {
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == plane.0)
        .expect("an AI row")
        .hp = 0;
}

/// Shoots down a human's plane the way combat does: one hit point left, then
/// damage that takes it, which destroys the ownship and crashes the flight.
fn shoot_down_human(world: &mut World, plane: PlaneId) {
    world.combat.state.ownship_mut(plane.0).unwrap().hp = 1;
    let launcher = combat::launcher(
        &world
            .cockpits
            .iter()
            .find(|c| c.plane == plane)
            .unwrap()
            .flight,
    );
    world
        .combat
        .command_for(plane.0, live::Command::DamagePlayer, launcher);
}

/// A human's pilot ejects, leaving the flight as `flight::State::eject`
/// leaves it.
fn eject(world: &mut World, plane: PlaneId) {
    let flight = &mut world
        .cockpits
        .iter_mut()
        .find(|c| c.plane == plane)
        .unwrap()
        .flight;
    flight.escape = Some(tore_sim::ejection::Escape::new(
        flight.position,
        flight.velocity,
        attitude::Basis::new(flight.yaw, flight.pitch, flight.bank),
    ));
    flight.systems.pilot.ejected = true;
    flight.crashed = true;
}

/// An AI leader is destroyed: the next AI member leads that tick, the flight
/// re-forms behind it, and again when that one goes. Nobody hears anything of
/// it: an AI that takes the lead says nothing, and the journal only notes it.
#[test]
fn an_ai_leader_is_destroyed_and_the_next_ai_member_leads() {
    // The enemy wing is all AI: planes 4 to 7, plane 4 leading.
    let mut world = ai_mission();
    let mut log = Log::new();
    log.fly(&mut world, 60);
    let enemy = [PlaneId(4), PlaneId(5), PlaneId(6), PlaneId(7)];
    assert_eq!(leader(&world, ENEMY_SIDE), Some(4));
    assert_eq!(
        flight(&world, &enemy),
        [(5, 1, true), (6, 2, true), (7, 3, true)]
    );
    assert!(log.changes().is_empty(), "nothing changes while it lives");

    shoot_down_ai(&mut world, PlaneId(4));
    log.fly(&mut world, 1);
    assert_eq!(leader(&world, ENEMY_SIDE), Some(5), "lead passes that tick");
    // The flight re-forms: the followers close up in member order.
    assert_eq!(flight(&world, &enemy), [(6, 1, true), (7, 2, true)]);
    log.fly(&mut world, 120);
    assert!(
        flight(&world, &enemy).iter().all(|f| f.2),
        "the followers still fly on their leader"
    );

    // And again when the new leader goes.
    shoot_down_ai(&mut world, PlaneId(5));
    log.fly(&mut world, 1);
    assert_eq!(leader(&world, ENEMY_SIDE), Some(6));
    assert_eq!(flight(&world, &enemy), [(7, 1, true)]);
    // The other side's wing is not touched.
    assert_eq!(leader(&world, FRIENDLY_SIDE), Some(0));

    assert_eq!(log.changes(), [(5, 4, false), (6, 5, false)]);
    // Nobody hears a call about it, and nothing is queued for a seat.
    log.fly(&mut world, 900);
    assert!(log.leadership_calls().is_empty(), "{:?}", log.radio);
    assert!(
        log.journal
            .iter()
            .filter(|e| matches!(e.origin.cause, journal::Cause::Leadership { .. }))
            .all(|e| e.heard_by.is_empty() && e.outcome == journal::Outcome::Noted)
    );
}

/// The friendly wing with two humans, the second one member 3 of it, and two
/// AI members between them, so a lower-numbered AI aircraft could take the
/// lead but the human must. The enemy wing has two humans of its own, which
/// must hear nothing of it.
fn human_wingman_at_the_back() -> World {
    let mut world = ai_mission();
    for (seat, plane) in [(1, 3), (2, 4), (3, 5)] {
        world.take_plane(SeatId(seat), PlaneId(plane)).unwrap();
    }
    world
}

/// How the previous leader leaves.
#[derive(Clone, Copy, Debug)]
enum Exit {
    /// Shot down.
    Destroyed,
    /// The pilot ejected and is alive.
    Ejected,
}

/// Plane 0, the human lead, leaves; plane 3, the human at the back, leads
/// though AI planes 1 and 2 are lower-numbered. Returns the world after 900
/// more ticks, what it said, and the tick the lead passed.
fn the_human_wingman_leads(exit: Exit) -> (World, Log, usize) {
    let mut world = human_wingman_at_the_back();
    let mut log = Log::new();
    log.fly(&mut world, 60);
    assert_eq!(leader(&world, FRIENDLY_SIDE), Some(0));
    let friendly = [PlaneId(1), PlaneId(2)];
    assert_eq!(flight(&world, &friendly), [(1, 1, true), (2, 2, true)]);
    let before = world.tick() as usize;
    match exit {
        Exit::Destroyed => shoot_down_human(&mut world, F_LEAD),
        Exit::Ejected => eject(&mut world, F_LEAD),
    }
    log.fly(&mut world, 1);
    assert_eq!(
        leader(&world, FRIENDLY_SIDE),
        Some(3),
        "the human leads, {exit:?}"
    );
    // The AI wingmen fly on it, taking their places in member order behind it.
    assert_eq!(flight(&world, &friendly), [(1, 1, true), (2, 2, true)]);
    log.fly(&mut world, 900);
    (world, log, before)
}

#[test]
fn a_human_leader_is_destroyed_and_the_human_wingman_leads() {
    let (world, log, changed) = the_human_wingman_leads(Exit::Destroyed);
    assert_eq!(log.changes(), [(3, 0, false)]);
    // One call, to the new leader's seat alone, five seconds after: nobody is
    // left to say it, so it is text with no voice.
    let calls = log.leadership_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let (tick, seat, call) = calls[0];
    assert_eq!(*seat, SeatId(1), "plane 3's seat");
    assert_eq!(call.kind, comms::Kind::Important);
    assert_eq!(call.label, "Flight");
    assert!(call.stems.is_empty(), "{call:?}");
    assert!(call.text.contains("WNGLDR"), "{call:?}");
    assert!(
        (changed + 600..changed + 606).contains(tick),
        "delivered at tick {tick}, lead passed at {changed}"
    );
    assert!(world.cockpits[0].flight.crashed);
}

#[test]
fn a_human_leader_ejects_alive_and_the_call_is_voiced_by_that_pilot() {
    let (_, log, changed) = the_human_wingman_leads(Exit::Ejected);
    assert_eq!(log.changes(), [(3, 0, true)]);
    let calls = log.leadership_calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let (tick, seat, call) = calls[0];
    assert_eq!(*seat, SeatId(1), "plane 3's seat alone: {calls:?}");
    assert_eq!(call.kind, comms::Kind::Important);
    // The words are the recorded call, in the ejected leader's name.
    assert_eq!(call.stems, ["^WNGLDR"], "{call:?}");
    assert_eq!(call.label, "Red one");
    assert_eq!(call.origin.speaker, Some(0));
    assert!((changed + 600..changed + 606).contains(tick), "tick {tick}");
}

/// A human leader is destroyed with only AI wingmen: the first living AI
/// member leads, not the lowest-numbered one when that one is already gone,
/// and the flight re-forms behind it. Nobody hears a call.
#[test]
fn a_human_leader_is_destroyed_with_only_ai_wingmen_and_the_first_living_one_leads() {
    let mut world = ai_mission();
    let mut log = Log::new();
    let friendly = [PlaneId(1), PlaneId(2), PlaneId(3)];
    log.fly(&mut world, 60);
    assert_eq!(leader(&world, FRIENDLY_SIDE), Some(0));
    assert_eq!(
        flight(&world, &friendly),
        [(1, 1, true), (2, 2, true), (3, 3, true)]
    );
    // The first wingman goes first: the human still leads.
    shoot_down_ai(&mut world, PlaneId(1));
    log.fly(&mut world, 30);
    assert_eq!(leader(&world, FRIENDLY_SIDE), Some(0));
    assert!(log.changes().is_empty());
    // Then the human leader.
    shoot_down_human(&mut world, F_LEAD);
    log.fly(&mut world, 1);
    assert_eq!(
        leader(&world, FRIENDLY_SIDE),
        Some(2),
        "the first living AI member"
    );
    assert_eq!(flight(&world, &friendly), [(3, 1, true)]);
    log.fly(&mut world, 120);
    assert!(flight(&world, &friendly).iter().all(|f| f.2));
    // And again when the AI leader goes.
    shoot_down_ai(&mut world, PlaneId(2));
    log.fly(&mut world, 1);
    assert_eq!(leader(&world, FRIENDLY_SIDE), Some(3));
    assert_eq!(flight(&world, &friendly), []);
    assert_eq!(log.changes(), [(2, 0, false), (3, 2, false)]);
    // The only human is the dead leader, and no call is made to it.
    log.fly(&mut world, 900);
    assert!(
        log.leadership_calls().is_empty(),
        "{:?}",
        log.leadership_calls()
    );
    // The enemy wing kept its own leader throughout.
    assert_eq!(leader(&world, ENEMY_SIDE), Some(4));
}
