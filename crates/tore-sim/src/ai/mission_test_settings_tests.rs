//! The AI test settings (slice S7b): the default rate is the exact path, the
//! actors are staggered evenly, serial and worker steps agree at a slow rate,
//! lean sensors change nothing a decision reads, and a slow rate holds the
//! sensor timings.
use super::observation_tests::{compare_step, executors, fixture, scripted_changes, snapshot};
use super::*;
use crate::ai::test_settings::OBSERVATION_RATES_HZ;

fn settings(hz: u32, lean: bool) -> AiTestSettings {
    AiTestSettings::default()
        .with_observation_hz(hz)
        .expect("a listed rate")
        .with_lean_sensors(lean)
}

fn flight_bits(actor: &AiActor) -> Vec<u8> {
    let mut writer = tore_codec::BitWriter::new();
    actor.flight.write_exact(&mut writer, None).unwrap();
    writer.finish()
}

#[test]
fn the_default_rate_observes_every_actor_every_tick() {
    let mission = fixture(12, 3);
    assert!(mission.test_settings().is_default());
    for tick in 0..50 {
        for index in 0..12 {
            assert!(mission.observation_due(index, tick));
        }
    }
}

#[test]
fn actors_are_staggered_evenly_across_the_ticks() {
    for hz in OBSERVATION_RATES_HZ {
        let divisor = u64::from(settings(hz, false).divisor());
        for count in [1_usize, 4, 7, 12, 30] {
            let mut mission = fixture(count as u32, 5);
            mission.set_test_settings(settings(hz, false));
            let window = 12 * divisor;
            let mut per_tick = Vec::new();
            let mut per_actor = vec![0_u64; count];
            for tick in 0..window {
                let due: Vec<usize> = (0..count)
                    .filter(|index| mission.observation_due(*index, tick))
                    .collect();
                per_tick.push(due.len());
                for index in due {
                    per_actor[index] += 1;
                }
            }
            // Every actor observes once in every `divisor` ticks...
            assert!(
                per_actor.iter().all(|n| *n == 12),
                "{hz} Hz {count}: {per_actor:?}"
            );
            // ...and no tick carries more than one actor above the others'
            // share.
            let most = *per_tick.iter().max().unwrap();
            let least = *per_tick.iter().min().unwrap();
            assert!(most - least <= 1, "{hz} Hz {count}: {per_tick:?}");
        }
    }
}

/// The seeded mission's scripted changes. Stage 2 there asserts that an
/// aircraft already holds a target, which is a fact of observing every tick;
/// here it only damages the aircraft, and a second hit lands on a tick that
/// is not its aircraft's turn to observe.
fn scripted(mission: &mut AiMission, tick: u64, human: &mut Option<AiActor>) {
    match tick {
        2 => mission.actor_mut(4).unwrap().flight.systems.hit(34, 0.7),
        13 => mission.actor_mut(7).unwrap().flight.systems.hit(34, 0.7),
        _ => scripted_changes(mission, tick, human),
    }
}

#[test]
fn serial_and_worker_steps_agree_at_a_slow_rate() {
    for (hz, lean) in [(30, false), (30, true), (20, true), (60, false), (40, true)] {
        for workers in executors() {
            let mut reference = fixture(30, 7);
            reference.set_test_settings(settings(hz, lean));
            let mut actual = reference.clone();
            let (mut reference_human, mut actual_human) = (None, None);
            for tick in 0..28 {
                scripted(&mut reference, tick, &mut reference_human);
                scripted(&mut actual, tick, &mut actual_human);
                compare_step(
                    &mut reference,
                    &mut actual,
                    &workers,
                    reference_human.as_ref(),
                )
                .unwrap();
                assert_eq!(reference.take_journal(), actual.take_journal());
            }
        }
    }
}

/// Between two observations an actor holds its picture: the memory, the
/// sensors and the emitters stay as they were, while the lookout follows the
/// aircraft.
#[test]
fn a_tick_between_observations_holds_the_picture_and_refreshes_the_lookout() {
    let mut mission = fixture(8, 9);
    mission.set_test_settings(settings(30, false));
    let mut held_ticks = 0;
    let mut observed_ticks = 0;
    for _ in 0..40 {
        let tick = mission.tick;
        let before: Vec<_> = mission
            .actors
            .iter()
            .map(|a| {
                (
                    a.awareness.clone(),
                    a.sensors.clone(),
                    a.lookout,
                    a.flight.position,
                )
            })
            .collect();
        let world = snapshot(&mission, None);
        mission
            .step_using(
                &world,
                &super::tests::flat,
                &observation_tests::surface,
                TimeOfDay(tick),
                &tore_workers::Executor::serial(),
            )
            .unwrap();
        for (index, actor) in mission.actors.iter().enumerate() {
            if !actor.alive() || actor.dummy {
                continue;
            }
            let due = (tick + index as u64).is_multiple_of(4);
            let (awareness, sensors, lookout, position) = &before[index];
            if due {
                observed_ticks += 1;
                assert_eq!(
                    actor.sensors.as_ref().map(|s| s.tick()),
                    sensors.as_ref().map(|s| s.tick() + 4),
                    "an observation counts four ticks"
                );
            } else {
                held_ticks += 1;
                assert_eq!(&actor.awareness, awareness, "actor {index} tick {tick}");
                assert_eq!(&actor.sensors, sensors, "actor {index} tick {tick}");
                let now = actor.lookout.expect("a lookout");
                if let Some(then) = lookout {
                    assert_eq!(now.attention, then.attention);
                    assert_eq!(now.sector, ((tick / 40) % 6) as usize);
                }
                assert_eq!(
                    now.position, *position,
                    "the lookout is where the aircraft was"
                );
            }
        }
    }
    assert!(held_ticks > observed_ticks, "{held_ticks} {observed_ticks}");
}

/// Over a long run the sensors' own clock keeps pace with the mission's: an
/// observation every fourth tick stands for four.
#[test]
fn a_slow_rate_keeps_the_sensor_clock_with_the_mission() {
    let mut mission = fixture(8, 4);
    mission.set_test_settings(settings(30, true));
    for _ in 0..200 {
        let tick = mission.tick;
        let world = snapshot(&mission, None);
        mission
            .step_using(
                &world,
                &super::tests::flat,
                &observation_tests::surface,
                TimeOfDay(tick),
                &tore_workers::Executor::serial(),
            )
            .unwrap();
    }
    for actor in &mission.actors {
        let Some(sensors) = actor.sensors.as_ref() else {
            continue;
        };
        if !actor.alive() {
            continue;
        }
        let behind = 200_u64.abs_diff(sensors.tick());
        assert!(
            behind <= 3,
            "actor {}: sensors at {}",
            actor.id(),
            sensors.tick()
        );
    }
}

/// Lean sensors drop what no decision reads: the whole mission flies, decides
/// and journals exactly as it does with the full sensors.
#[test]
fn lean_sensors_change_nothing_a_decision_reads() {
    let mut full = fixture(30, 21);
    let mut lean = full.clone();
    lean.set_test_settings(settings(120, true));
    let (mut full_human, mut lean_human) = (None, None);
    let mut compared_emitters = 0;
    for tick in 0..60 {
        scripted(&mut full, tick, &mut full_human);
        scripted(&mut lean, tick, &mut lean_human);
        let world = snapshot(&full, full_human.as_ref());
        let a = full.step_using(
            &world,
            &super::tests::flat,
            &observation_tests::surface,
            TimeOfDay(tick),
            &tore_workers::Executor::serial(),
        );
        let b = lean.step_using(
            &world,
            &super::tests::flat,
            &observation_tests::surface,
            TimeOfDay(tick),
            &tore_workers::Executor::serial(),
        );
        assert_eq!(a, b, "tick {tick}");
        assert_eq!(full.take_journal(), lean.take_journal());
        for (left, right) in full.actors.iter().zip(&lean.actors) {
            assert_eq!(flight_bits(left), flight_bits(right), "actor {}", left.id());
            assert_eq!(left.awareness, right.awareness);
            assert_eq!(left.controller.target(), right.controller.target());
            assert_eq!(left.observed_attacks, right.observed_attacks);
            let (Some(full_sensors), Some(lean_sensors)) =
                (left.sensors.as_ref(), right.sensors.as_ref())
            else {
                continue;
            };
            assert_eq!(full_sensors.contacts(), lean_sensors.contacts());
            assert_eq!(full_sensors.selected(), lean_sensors.selected());
            assert_eq!(full_sensors.acquired(), lean_sensors.acquired());
            assert!(lean_sensors.visual().is_empty());
            assert!(lean_sensors.map_contacts().is_empty());
            assert!(right.received_emitters.is_empty());
            compared_emitters += left.received_emitters.len();
        }
    }
    assert!(
        compared_emitters > 0,
        "the fixture hears emitters to leave out"
    );
}

/// The emitters lean sensors build on demand are the ones the full
/// observation builds, and the warning that needs them names the same
/// launcher either way.
#[test]
fn emitters_built_on_demand_are_the_ones_observation_builds() {
    let mut mission = fixture(10, 2);
    let world = snapshot(&mission, None);
    let mut heard = 0;
    for index in 0..mission.actors.len() {
        let actor = &mut mission.actors[index];
        if actor.sensors.is_none() {
            continue;
        }
        let sight = observation::SightLines::new(actor.flight.position, &super::tests::flat, None);
        actor.observe_sighted(0, &world, &sight);
        let built: Vec<u32> = actor.received_emitters.iter().map(|e| e.id).collect();
        let on_demand = actor.emitter_ids(&world, &sight, settings(120, true));
        let kept = actor.emitter_ids(&world, &sight, settings(120, false));
        assert_eq!(on_demand, built, "actor {index}");
        assert_eq!(kept, built);
        heard += built.len();
    }
    assert!(heard > 0, "the fixture hears emitters");
}
