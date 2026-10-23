//! Stage H slice H1's round trips on the crowd fixture (docs/formats/
//! checkpoint.md): every ownship's configuration, sensors and missile threat
//! service, every AI actor's sensors, and the world's dummy configurations,
//! at ticks 300, 600 and 900 of the crowd fight. The coders themselves, and
//! the 600-tick step-on tests, are in `tore-sim` beside the types.

use super::checkpoint_scenarios::{self, Scenario};
use super::{TickOutput, World};
use tore_sim::checkpoint::{Models, from_bytes, round_trip, to_bytes};
use tore_sim::combat::live::Configuration;
use tore_sim::combat::threats::ThreatService;
use tore_sim::sensors::Sensors;

/// Steps `world` from its `from`th step up to (not including) step `to`.
fn advance(world: &mut World, scenario: &Scenario, from: u64, to: u64) {
    let mut out = TickOutput::default();
    for step in from..to {
        let (commands, inputs) = (scenario.drive)(world, step);
        world
            .step_with(&commands, &inputs, &mut out, |_, _| Ok(()))
            .unwrap();
    }
}

fn text<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

#[test]
fn the_crowds_records_and_sensors_round_trip_at_300_600_and_900() {
    let scenario = checkpoint_scenarios::crowd_fight();
    let models = Models::default();
    let mut world = (scenario.build)();
    let mut done = 0;
    let (mut contacts, mut tracked, mut threats, mut actors) = (0, 0, 0, 0);
    for tick in [300, 600, 900] {
        advance(&mut world, &scenario, done, tick);
        done = tick;

        // Every human's ownship: its configuration (a shared record with its
        // weapons and sensor profiles), its sensors and its warnings.
        let mut shared = Vec::new();
        for own in world.combat.state.ownships() {
            let config = own.configuration();
            let copy: Configuration = round_trip(config, &models).unwrap();
            assert_eq!(
                text(&copy),
                text(config),
                "ownship {} at {tick}",
                own.aircraft
            );
            shared.push(config.clone());

            let copy: Sensors = round_trip(&own.sensors, &models).unwrap();
            assert_eq!(
                copy, own.sensors,
                "ownship {} sensors at {tick}",
                own.aircraft
            );
            contacts += own.sensors.contacts().len();
            tracked += usize::from(own.sensors.acquired().is_some());

            let service: &ThreatService = &own.missile_threats;
            let copy: ThreatService = round_trip(service, &models).unwrap();
            assert!(copy.records().eq(service.records()));
            threats += service.records().count();
        }
        assert!(!shared.is_empty());
        // Equal weapons across the crowd's aircraft code once: coding every
        // configuration together costs fewer records than coding each alone.
        let all = to_bytes(&shared, &models).unwrap();
        let alone: usize = shared
            .iter()
            .map(|config| to_bytes(config, &models).unwrap().records.len())
            .sum();
        assert!(
            shared.len() < 2 || all.records.len() < alone,
            "{} records together, {alone} alone, for {} configurations",
            all.records.len(),
            shared.len()
        );
        let sensor_bytes: usize = world
            .combat
            .state
            .ownships()
            .iter()
            .map(|own| to_bytes(&own.sensors, &models).unwrap().body.len())
            .sum();
        println!(
            "tick {tick}: {} configurations in {} bytes and {} records of {} bytes, \
             {sensor_bytes} bytes of ownship sensors",
            shared.len(),
            all.body.len(),
            all.records.len(),
            all.records.iter().map(Vec::len).sum::<usize>(),
        );
        let copy: Vec<Configuration> = from_bytes(&all, &models).unwrap();
        assert_eq!(text(&copy), text(&shared));

        // Every AI actor's sensors.
        let mission = world.ai_wings.as_ref().unwrap().mission();
        for actor in mission.actors() {
            if let Some(sensors) = actor.sensors() {
                let copy: Sensors = round_trip(sensors, &models).unwrap();
                assert_eq!(&copy, sensors, "an actor's sensors at {tick}");
                actors += 1;
            }
        }
        for config in world.combat.dummy_configurations() {
            let copy: Configuration = round_trip(config, &models).unwrap();
            assert_eq!(text(&copy), text(config));
        }
    }
    // The fixture really held what the coders carry.
    assert!(contacts > 0, "no ownship ever held a radar contact");
    assert!(actors > 0, "no AI actor had sensors");
    // Missile warnings and weapon tracks depend on the fight; report them.
    println!(
        "crowd: {contacts} contacts, {tracked} tracks, {threats} threats, {actors} actor suites"
    );
}
