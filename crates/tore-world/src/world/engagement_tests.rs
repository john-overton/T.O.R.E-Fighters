//! The AI's engagement table against the live scans it replaced (slice G1,
//! docs/ARCHITECTURE.md "How the AI reads the picture"), on whole worlds: the
//! crowd fixture's fight (four humans, AI wingmen ordered to attack), the
//! crowd fixture's AI fight with seats handing planes back and taking enemy
//! planes mid-fight, and the open mission whose seats take and give back
//! planes. The audit in `tore_sim::ai::link::audit` compares the table with
//! the old scans at every AI actor's turn of every tick; each tick also checks
//! the one engagement and lock rule against the inline tests `locks_on`,
//! `aiming_at` and the picture used before.

use super::checkpoint_scenarios::{self, Step};
use super::crowd::{self, E_AI, E_LEAD, F_HUMAN};
use super::*;
use crate::seats::SeatCommand;
use tore_input::{PilotCommand, PilotInput, Switch};
use tore_sim::ai::{
    link::{self, audit},
    weapon_service::{Phase, StoreCapability},
    wing::PlayerOrder,
};

/// What one audited run saw besides the audit's own counts.
#[derive(Debug, Default)]
struct Seen {
    /// AI actor ticks with an engagement, and with a lock.
    engaged: u64,
    locked: u64,
    /// AI actors dead at the end.
    dead: usize,
}

/// `world` stepped `steps` times under the audit, `drive` giving each step's
/// commands and inputs.
fn audited(
    mut world: World,
    steps: u64,
    mut drive: impl FnMut(&mut World, u64) -> Step,
) -> (audit::Report, Seen) {
    let mut out = TickOutput::default();
    let mut seen = Seen::default();
    audit::start();
    for step in 0..steps {
        let (commands, inputs) = drive(&mut world, step);
        world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap_or_else(|error| panic!("step {step}: {error}"));
        let wings = world.ai_wings.as_ref().expect("an AI mission");
        for actor in wings.mission().actors() {
            // The rules as `locks_on`, `aiming_at` and the picture wrote them
            // before slice G1.
            let target = actor.controller().target().filter(|_| actor.alive());
            let lock = target.filter(|_| {
                matches!(
                    actor.controller().weapon_phase(),
                    Phase::Tracking | Phase::Fire
                )
            });
            assert_eq!(link::engagement_of(actor), target, "step {step}");
            assert_eq!(link::lock_of(actor), lock, "step {step}");
            seen.engaged += u64::from(target.is_some());
            seen.locked += u64::from(lock.is_some());
        }
    }
    let report = audit::finish();
    seen.dead = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actors()
        .iter()
        .filter(|a| !a.alive())
        .count();
    (report, seen)
}

/// The fixture's weapon records carry no capability flags; give the AI's gun
/// and missile the ones a real record's flags give them, as the data link's
/// tests do, so the AI fights.
fn arm_the_ai(world: &mut World) {
    let wings = world.ai_wings.as_mut().unwrap();
    for id in 1..=7 {
        if let Some(actor) = wings.mission_mut().actor_mut(id) {
            let stations = actor.stations_mut();
            stations[0].guided = false;
            stations[0].capability = StoreCapability::GUN;
            stations[1].guided = true;
            stations[1].capability = StoreCapability::AIR_TO_AIR_MISSILE;
        }
    }
}

/// Every flying seat's input: radar on at step 10, the guns in two bursts,
/// and from the leaders listed an attack order at step 30.
fn crowd_inputs(world: &World, step: u64, leaders: &[SeatId]) -> Vec<SeatInput> {
    crowd::inputs(world, |seat| {
        let mut pilot = PilotInput::default();
        if step == 10 {
            pilot.commands.push(PilotCommand::Set(Switch::Radar, true));
        }
        if (120..240).contains(&step) {
            pilot.pitch = 0.2;
        }
        let commands = if step == 30 && leaders.contains(&seat) {
            vec![SeatCommand::WingOrder(PlayerOrder::AttackOnContact)]
        } else {
            Vec::new()
        };
        SeatInput {
            pilot,
            commands,
            trigger: (400..430).contains(&step) || (900..920).contains(&step),
            ..SeatInput::default()
        }
    })
}

#[test]
fn engagement_table_equals_the_live_scan_in_the_crowd_fight() {
    // Two humans a side; both leaders order their AI wingmen to attack.
    let mut world = crowd::crowded_mission();
    arm_the_ai(&mut world);
    let (report, seen) = audited(world, 1200, |world, step| {
        (
            Vec::new(),
            crowd_inputs(world, step, &[SeatId(0), SeatId(2)]),
        )
    });
    // Four AI wingmen, every tick.
    assert!(report.wing_checks >= 4 * 1200, "{report:?}");
    assert!(report.wing_attacking > 0, "{report:?} {seen:?}");
    assert!(seen.engaged > 0 && seen.locked > 0, "{seen:?}");
}

#[test]
fn engagement_table_equals_the_live_scan_through_mid_fight_handoffs() {
    // The enemy wing is all AI and its leader an AI; the friendly lead orders
    // its wing in. Seat 1 flies plane 1, hands it back to the AI mid-fight,
    // then takes an enemy AI wingman. The enemy leader crashes, so the lead
    // passes on mid-fight, and later an enemy wingman.
    let mut world = crowd::ai_mission();
    arm_the_ai(&mut world);
    world.take_plane(SeatId(1), F_HUMAN).unwrap();
    let (report, seen) = audited(world, 1500, |world, step| {
        match step {
            600 => world.give_back_plane(SeatId(1)).unwrap(),
            700 => crash(world, E_LEAD),
            1100 => crash(world, E_AI[0]),
            900 => {
                let alive = world
                    .ai_wings
                    .as_ref()
                    .and_then(|w| w.mission().actor(E_AI[1].0))
                    .is_some_and(|a| a.alive());
                if alive {
                    world.take_plane(SeatId(1), E_AI[1]).unwrap();
                }
            }
            _ => {}
        }
        (Vec::new(), crowd_inputs(world, step, &[SeatId(0)]))
    });
    assert!(report.wing_attacking > 0, "{report:?} {seen:?}");
    assert!(
        report.leader_checks > 0 && report.leader_targets > 0,
        "{report:?} {seen:?}"
    );
    assert!(seen.engaged > 0 && seen.locked > 0, "{seen:?}");
    assert!(seen.dead >= 2, "{seen:?}");
}

/// AI plane `plane` crashes.
fn crash(world: &mut World, plane: PlaneId) {
    let wings = world.ai_wings.as_mut().unwrap();
    wings
        .mission_mut()
        .actor_mut(plane.0)
        .unwrap()
        .flight_mut()
        .crashed = true;
}

#[test]
fn engagement_table_equals_the_live_scan_in_the_open_mission_with_handoffs() {
    // Built by `World::new` from the synthetic import; the wings do not close
    // within the run, so this covers the leader reads through the handoffs.
    let scenario = checkpoint_scenarios::open_handoffs();
    let (report, _) = audited(
        (scenario.build)(),
        scenario.at + scenario.then,
        scenario.drive,
    );
    assert!(report.wing_checks > 0, "{report:?}");
    assert!(report.leader_checks > 0, "{report:?}");
}
