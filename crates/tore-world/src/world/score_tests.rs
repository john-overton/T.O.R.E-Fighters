//! Score facts in the mission core (slice F2-S; docs/ARCHITECTURE.md,
//! "Scoring"), on the crowd fixture: two humans and two AI aircraft on each
//! side (`crowd.rs`). A real gun burst proves the tick's wiring (combat's
//! strikes become damage, the shot-down human a kill and a loss); the other
//! cases set the world's state between ticks the way combat and the AI leave
//! it, so each rule is checked on its own.

use super::crowd::*;
use super::*;
use crate::score::{Fact, Facts, Flown, Recorder, Victim};
use crate::seats::Pilot;
use tore_sim::combat::{
    ledger::Kill,
    live::{FriendlyFire, Strike},
    missiles::TargetRole,
};

/// The seats of the crowd's humans, in [`HUMANS`] order.
const SEATS: [SeatId; 4] = [SeatId(0), SeatId(1), SeatId(2), SeatId(3)];

fn scoring_mission() -> World {
    let mut world = crowded_mission();
    world.combat.state.friendly_fire = FriendlyFire::On;
    world.set_scoring(true);
    world
}

/// One tick with neutral input for every seat, `firing` holding the trigger.
fn step(world: &mut World, firing: Option<SeatId>) -> Facts {
    let mut out = TickOutput::default();
    let step_inputs = inputs(world, |seat| SeatInput {
        trigger: Some(seat) == firing,
        ..SeatInput::default()
    });
    world.step(&step_inputs, &mut out).unwrap();
    world.take_score_facts()
}

fn human(plane: PlaneId) -> Flown {
    let seat = SEATS[HUMANS.iter().position(|p| *p == plane).unwrap()];
    Flown {
        plane,
        pilot: Pilot::Human(seat),
    }
}

fn ai(plane: PlaneId) -> Flown {
    Flown {
        plane,
        pilot: Pilot::Ai,
    }
}

fn plane_victim(flown: Flown) -> Victim {
    Victim {
        target: flown.plane.0,
        flown: Some(flown),
        aircraft: true,
    }
}

fn credit(owner: PlaneId, victim: PlaneId) -> Kill {
    Kill {
        owner: owner.0,
        victim: victim.0,
        category: 0x8000,
        aircraft: true,
    }
}

fn kills(facts: &[Fact]) -> Vec<Fact> {
    facts
        .iter()
        .filter(|f| matches!(f, Fact::Kill { .. }))
        .copied()
        .collect()
}

fn losses(facts: &[Fact]) -> Vec<Fact> {
    facts
        .iter()
        .filter(|f| matches!(f, Fact::Loss { .. }))
        .copied()
        .collect()
}

fn flight_of(world: &mut World, plane: PlaneId) -> &mut flight::State {
    if let Some(index) = world.cockpits.iter().position(|c| c.plane == plane) {
        return &mut world.cockpits[index].flight;
    }
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .mission_mut()
        .actor_mut(plane.0)
        .unwrap()
        .flight_mut()
}

/// Puts `victim` where `shooter`'s first round will be 40 ticks after it
/// leaves at 1,032 feet a second, flying alongside, as `fight_tests.rs`
/// aims its bursts.
fn aim(world: &mut World, shooter: PlaneId, victim: PlaneId) {
    let from = flight_of(world, shooter).clone();
    let forward = attitude::Basis::new(from.yaw, from.pitch, from.bank).forward;
    let position: [f64; 3] = std::array::from_fn(|i| {
        from.position[i] + (forward[i] * 1032. - from.velocity[i]) / 120. * 40.
    });
    let flight = flight_of(world, victim);
    flight.position = position;
    flight.velocity = from.velocity;
    (flight.yaw, flight.pitch, flight.bank) = (from.yaw, from.pitch, from.bank);
    flight.speed = from.speed;
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

#[test]
fn scoring_is_off_until_the_host_turns_it_on_and_off_records_nothing() {
    let mut world = crowded_mission();
    assert!(!world.scoring());
    world.combat.state.ownship_mut(E_LEAD.0).unwrap().hp = 0;
    let facts = step(&mut world, None);
    assert!(facts.facts.is_empty(), "{facts:?}");
    world.set_scoring(true);
    assert!(world.scoring());
    let facts = step(&mut world, None);
    assert_eq!(facts.tick, 1, "the facts name the tick just stepped");
    assert!(!facts.facts.is_empty(), "the lost plane is recorded now");
    world.set_scoring(false);
    assert!(world.take_score_facts().facts.is_empty());
}

#[test]
fn a_gun_burst_records_its_damage_and_the_human_it_shoots_down_with_the_pilot_aboard() {
    let mut world = scoring_mission();
    // Nearly finished, so two rounds bring it down.
    world.combat.state.ownship_mut(E_LEAD.0).unwrap().hp = 2;
    let capacity = world
        .combat
        .state
        .ownship(E_LEAD.0)
        .unwrap()
        .configuration()
        .damage_capacity;
    aim(&mut world, F_LEAD, E_LEAD);
    let mut facts = Vec::new();
    for tick in 0..160 {
        let firing = (tick < 16).then_some(SEATS[0]);
        let stepped = step(&mut world, firing);
        assert_eq!(stepped.tick, tick as u64);
        facts.extend(stepped.facts);
    }
    let damage: Vec<f64> = facts
        .iter()
        .filter_map(|f| match f {
            Fact::Damage {
                shooter: Some(shooter),
                victim,
                fraction,
            } if *shooter == human(F_LEAD) && *victim == plane_victim(human(E_LEAD)) => {
                Some(*fraction)
            }
            _ => None,
        })
        .collect();
    assert!(!damage.is_empty(), "the burst hit: {facts:?}");
    // The hits took the two hit points left, as fractions of the whole.
    let total: f64 = damage.iter().sum();
    assert!(
        (total - 2. / f64::from(capacity)).abs() < 1e-12,
        "{damage:?} of {capacity}"
    );
    assert_eq!(
        kills(&facts),
        [Fact::Kill {
            shooter: Some(human(F_LEAD)),
            victim: plane_victim(human(E_LEAD)),
            pilot_aboard: true,
        }]
    );
    assert_eq!(
        losses(&facts),
        [Fact::Loss {
            plane: E_LEAD,
            seat: SEATS[2],
        }]
    );
}

#[test]
fn a_human_lost_after_ejecting_is_a_kill_without_the_pilot_and_a_loss() {
    let mut world = scoring_mission();
    world.combat.state.ledger.damaged(credit(F_HUMAN, E_HUMAN));
    flight_of(&mut world, E_HUMAN).systems.pilot.ejected = true;
    let facts = step(&mut world, None).facts;
    assert_eq!(
        kills(&facts),
        [Fact::Kill {
            shooter: Some(human(F_HUMAN)),
            victim: plane_victim(human(E_HUMAN)),
            pilot_aboard: false,
        }]
    );
    assert_eq!(
        losses(&facts),
        [Fact::Loss {
            plane: E_HUMAN,
            seat: SEATS[3],
        }]
    );
    // Recorded once: the wreck does not count again.
    for _ in 0..10 {
        assert!(kills(&step(&mut world, None).facts).is_empty());
    }
}

#[test]
fn an_ai_shooter_is_credited_as_the_ai_and_an_ai_victim_makes_no_loss() {
    let mut world = scoring_mission();
    // An AI aircraft shoots down a human.
    world.combat.state.ledger.kill(credit(E_AI[0], F_HUMAN));
    world.combat.state.ownship_mut(F_HUMAN.0).unwrap().hp = 0;
    // A human shoots down an AI aircraft.
    world.combat.state.ledger.kill(credit(F_LEAD, E_AI[1]));
    world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == E_AI[1].0)
        .unwrap()
        .hp = 0;
    let facts = step(&mut world, None).facts;
    assert_eq!(
        kills(&facts),
        [
            Fact::Kill {
                shooter: Some(ai(E_AI[0])),
                victim: plane_victim(human(F_HUMAN)),
                pilot_aboard: true,
            },
            Fact::Kill {
                shooter: Some(human(F_LEAD)),
                victim: plane_victim(ai(E_AI[1])),
                pilot_aboard: true,
            },
        ]
    );
    assert_eq!(
        losses(&facts),
        [Fact::Loss {
            plane: F_HUMAN,
            seat: SEATS[1],
        }],
        "only a human's plane is a loss"
    );
}

#[test]
fn a_plane_lost_with_nobody_to_credit_or_by_its_own_hand_has_no_shooter() {
    let mut world = scoring_mission();
    // A crash with no hit on it.
    flight_of(&mut world, F_LEAD).crashed = true;
    // Its own last hit (a bomb's blast, say) credits nobody.
    world.combat.state.ledger.damaged(credit(E_LEAD, E_LEAD));
    world.combat.state.ownship_mut(E_LEAD.0).unwrap().hp = 0;
    let facts = step(&mut world, None).facts;
    let shooters: Vec<(u32, Option<Flown>)> = kills(&facts)
        .iter()
        .map(|f| match f {
            Fact::Kill {
                shooter, victim, ..
            } => (victim.target, *shooter),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(shooters, [(F_LEAD.0, None), (E_LEAD.0, None)]);
    assert_eq!(losses(&facts).len(), 2);
}

#[test]
fn ground_kills_are_not_aircraft_and_damage_is_a_fraction_of_the_whole() {
    let mut world = scoring_mission();
    // A ground object far from the fight, with hit points of its own.
    let mut ground = crate::test_support::target(2001, [0., 0., 60_000.], 0.);
    ground.role = TargetRole::Surface;
    ground.aircraft = None;
    ground.airborne = false;
    (ground.hp, ground.initial_hp) = (40, 160);
    world.combat.state.targets.push(ground.clone());
    let row = world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == E_AI[0].0)
        .unwrap()
        .clone();
    let capacity = world
        .combat
        .state
        .ownship(E_HUMAN.0)
        .unwrap()
        .configuration()
        .damage_capacity;
    let strike = |owner: PlaneId, victim: u32, amount: i32, destroyed: bool| Strike {
        owner: owner.0,
        victim,
        weapon_flags: 0x80,
        destroyed,
        amount,
    };
    let mut recorder = Recorder::default();
    recorder.record(
        &world,
        7,
        &[
            strike(F_LEAD, ground.id, ground.hp, true),
            strike(F_HUMAN, E_AI[0].0, 25, false),
            strike(E_LEAD, E_HUMAN.0, 30, false),
            // A strike that took nothing (the victim had nothing left).
            strike(E_LEAD, E_HUMAN.0, 0, false),
        ],
    );
    let facts = recorder.take();
    assert_eq!(facts.tick, 7);
    let damage = |owner: Flown, victim: Victim, fraction: f64| Fact::Damage {
        shooter: Some(owner),
        victim,
        fraction,
    };
    let ground_victim = Victim {
        target: ground.id,
        flown: None,
        aircraft: false,
    };
    assert_eq!(
        facts.facts,
        [
            damage(
                human(F_LEAD),
                ground_victim,
                f64::from(ground.hp) / f64::from(ground.initial_hp)
            ),
            damage(
                human(F_HUMAN),
                plane_victim(ai(E_AI[0])),
                25. / f64::from(row.initial_hp)
            ),
            damage(
                human(E_LEAD),
                plane_victim(human(E_HUMAN)),
                30. / f64::from(capacity)
            ),
            Fact::Kill {
                shooter: Some(human(F_LEAD)),
                victim: ground_victim,
                pilot_aboard: false,
            },
        ]
    );
    // A ground object is recorded once.
    recorder.record(&world, 8, &[strike(F_LEAD, ground.id, 1, true)]);
    assert!(kills(&recorder.take().facts).is_empty());
    assert_eq!(recorder.recorded().collect::<Vec<_>>(), [ground.id]);
}

#[test]
fn the_facts_change_nothing_the_mission_does() {
    let run = |scoring: bool| {
        let mut world = crowded_mission();
        world.set_scoring(scoring);
        world.combat.state.ownship_mut(E_LEAD.0).unwrap().hp = 2;
        aim(&mut world, F_LEAD, E_LEAD);
        let mut out = TickOutput::default();
        let mut trace = Vec::new();
        for tick in 0..200 {
            let step_inputs = inputs(&world, |seat| SeatInput {
                trigger: tick < 16 && seat == SEATS[0],
                ..SeatInput::default()
            });
            world.step(&step_inputs, &mut out).unwrap();
            let radio = radio_of(&out).len();
            trace.push((
                world
                    .cockpits
                    .iter()
                    .map(|c| c.flight.position)
                    .collect::<Vec<_>>(),
                world
                    .combat
                    .state
                    .ownships()
                    .iter()
                    .map(|o| o.hp)
                    .collect::<Vec<_>>(),
                out.events.len(),
                radio,
            ));
        }
        trace
    };
    assert_eq!(run(false), run(true));
}
