//! Tests of the checkpoint coders' shared machinery: the standard types, the
//! macros, shared records, flight states and the readers' refusals.

use super::*;

/// No flight models: for values that hold no flight state.
static NONE: Models = Models { models: Vec::new() };
use crate::ai::DecisionRandom;
use crate::ai::controller::{Activity, FrameEvent, ThreatReport};
use crate::ai::experience::{ExperienceOrigin, ResolvedExperience};
use crate::ai::threat::SeekerClass;
use crate::ai::weapon_service::Rounds;
use crate::flight::State;
use crate::flight::integration_tests::profile;
use crate::models::FlightModel;
use crate::research::Surface;
use tore_input::PilotInput;

fn same<T: Checkpoint + PartialEq + std::fmt::Debug>(value: &T) -> Coded {
    let copy = round_trip(value, &NONE).unwrap();
    assert_eq!(&copy, value);
    to_bytes(value, &NONE).unwrap()
}

/// Bit patterns must survive, not just values.
#[test]
fn floats_and_integers_keep_every_bit() {
    let nan = f64::from_bits(0x7ff8_dead_beef_0001);
    for value in [
        0.,
        -0.,
        1.5,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN_POSITIVE / 3.,
        nan,
    ] {
        let copy: f64 = round_trip(&value, &NONE).unwrap();
        assert_eq!(copy.to_bits(), value.to_bits());
    }
    let nan32 = f32::from_bits(0x7fc0_1234);
    for value in [0f32, -0., 2.25, f32::NAN, nan32, f32::INFINITY] {
        let copy: f32 = round_trip(&value, &NONE).unwrap();
        assert_eq!(copy.to_bits(), value.to_bits());
    }
    same(&i64::MIN);
    same(&i8::MIN);
    same(&u64::MAX);
    same(&u32::MAX);
    same(&usize::MAX);
    same(&(true, -7i16, 300u16, ()));
}

#[test]
fn a_value_out_of_its_types_range_is_refused() {
    let coded = to_bytes(&300u32, &NONE).unwrap();
    assert!(from_bytes::<u8>(&coded, &NONE).is_err());
    let coded = to_bytes(&-1i64, &NONE).unwrap();
    assert!(from_bytes::<u32>(&coded, &NONE).is_err());
}

#[test]
fn collections_round_trip_in_order() {
    let long = "x".repeat(1000);
    same(&long);
    same(&"héllo, wingman".to_string());
    same(&vec![Some(1.5), None, Some(-2.)]);
    same(&VecDeque::from([3u8, 1, 2]));
    same(&BTreeMap::from([(3u32, "c".to_string()), (1, "a".into())]));
    same(&BTreeSet::from([9u64, 2, 7]));
    same(&[[1., 2., 3.], [4., 5., 6.]]);
    same(&Box::new(Some(vec![(1u8, 2i32)])));
    same(&Vec::<u32>::new());
}

#[test]
fn map_keys_out_of_order_are_refused() {
    // A two-entry map written by hand with its keys reversed.
    let mut s = Saver::new();
    s.count(2);
    5u32.save(&mut s, None).unwrap();
    1u8.save(&mut s, None).unwrap();
    3u32.save(&mut s, None).unwrap();
    2u8.save(&mut s, None).unwrap();
    let coded = Coded {
        body: s.finish_section(),
        records: Vec::new(),
    };
    assert!(from_bytes::<BTreeMap<u32, u8>>(&coded, &NONE).is_err());
    let mut s = Saver::new();
    s.count(2);
    4u32.save(&mut s, None).unwrap();
    4u32.save(&mut s, None).unwrap();
    let coded = Coded {
        body: s.finish_section(),
        records: Vec::new(),
    };
    assert!(from_bytes::<BTreeSet<u32>>(&coded, &NONE).is_err());
}

#[test]
fn a_count_the_bits_cannot_hold_is_refused_before_allocating() {
    let mut s = Saver::new();
    // Large, but within a 32-bit usize so the test builds for i686 too.
    s.count(1 << 30);
    let coded = Coded {
        body: s.finish_section(),
        records: Vec::new(),
    };
    assert!(from_bytes::<Vec<u8>>(&coded, &NONE).is_err());
    assert!(from_bytes::<String>(&coded, &NONE).is_err());
}

#[test]
fn unread_bits_are_refused() {
    let coded = to_bytes(&(1u64 << 40), &NONE).unwrap();
    // Read as a bool, most of the bits stay unread.
    assert!(from_bytes::<bool>(&coded, &NONE).is_err());
}

#[test]
fn a_baseline_makes_unchanged_values_cheap() {
    let value = vec![[1.25f64, -3., 7e9]; 20];
    let mut s = Saver::new();
    value.save(&mut s, Some(&value)).unwrap();
    let against_itself = s.finish_section().len();
    let alone = to_bytes(&value, &NONE).unwrap().body.len();
    // One bit per float against itself, plus the count.
    assert!(against_itself <= 10, "{against_itself} bytes");
    assert!(alone > 100, "{alone} bytes");
    let mut s = Saver::new();
    value.save(&mut s, Some(&value)).unwrap();
    let body = s.finish_section();
    let mut l = Loader::new(&body, &[], &NONE);
    assert_eq!(Vec::<[f64; 3]>::load(&mut l, Some(&value)).unwrap(), value);
}

// ---------------------------------------------------------------------------
// The macros.

#[derive(Clone, Debug, PartialEq)]
struct Sample {
    hp: i32,
    position: [f64; 3],
    weapons: Vec<String>,
    big: Vec<u64>,
    scratch: Vec<u8>,
}
crate::checkpoint_struct!(Sample { hp, position } shared { weapons, big } skip { scratch = Vec::new() });

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Idle,
    Busy,
}
crate::checkpoint_enum!(Mode { Idle = 0, Busy = 7 });

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pair(u32, f64);
crate::checkpoint_tuple!(Pair(a, b));

#[test]
fn a_struct_codes_its_fields_shares_its_records_and_rebuilds_skipped_ones() {
    let sample = Sample {
        hp: 70,
        position: [1., 2., 3.],
        weapons: vec!["AIM9".into(), "AIM120".into()],
        big: vec![u64::MAX; 40],
        scratch: vec![1, 2, 3],
    };
    let copy: Sample = round_trip(&sample, &NONE).unwrap();
    assert_eq!(
        copy,
        Sample {
            scratch: Vec::new(),
            ..sample.clone()
        }
    );
    // Five copies share their records: two records, whatever the count.
    let coded = to_bytes(&vec![sample.clone(); 5], &NONE).unwrap();
    assert_eq!(coded.records.len(), 2);
    let one = to_bytes(&vec![sample; 1], &NONE).unwrap();
    assert_eq!(coded.records, one.records);
    assert!(coded.body.len() < one.body.len() * 5);
}

#[test]
fn an_enum_codes_its_fixed_numbers_and_refuses_others() {
    same(&Mode::Busy);
    same(&Mode::Idle);
    let mut s = Saver::new();
    s.writer().write_varint(3);
    let coded = Coded {
        body: s.finish_section(),
        records: Vec::new(),
    };
    assert!(from_bytes::<Mode>(&coded, &NONE).is_err());
    same(&Pair(4, -0.5));
}

#[test]
fn a_shared_record_read_as_another_type_or_ahead_of_itself_is_refused() {
    let coded = to_bytes(
        &Sample {
            hp: 1,
            position: [0.; 3],
            weapons: vec!["GUN".into()],
            big: vec![],
            scratch: vec![],
        },
        &NONE,
    )
    .unwrap();
    // The body names record 0 as a Vec<String>; read as a Vec<u64> instead.
    let mut l = Loader::new(&coded.body, &coded.records, &NONE);
    let _: i32 = Checkpoint::load(&mut l, None).unwrap();
    let _: [f64; 3] = Checkpoint::load(&mut l, None).unwrap();
    assert!(l.shared::<Vec<u64>>().is_err());
    // A record whose own shared fields name itself, record 0: a record may
    // name only earlier records, so damaged bytes cannot recurse.
    let mut s = Saver::new();
    s.writer()
        .write_bits(u64::from(type_tag::<Sample>()), 32)
        .unwrap();
    1i32.save(&mut s, None).unwrap();
    [0f64; 3].save(&mut s, None).unwrap();
    s.writer().write_varint(0);
    s.writer().write_varint(0);
    let records = vec![s.finish_section()];
    let mut s = Saver::new();
    s.writer().write_varint(0);
    let body = s.finish_section();
    let mut l = Loader::new(&body, &records, &NONE);
    assert!(l.shared::<Sample>().is_err());
}

// ---------------------------------------------------------------------------
// The shared leaf types.

#[test]
fn the_shared_leaf_types_round_trip() {
    for aircraft in [
        AircraftId::F18,
        AircraftId::Rafale,
        AircraftId::F14,
        AircraftId::A4E,
        AircraftId::X31,
        AircraftId::Mig29,
        AircraftId::Su27,
        AircraftId::Mig21,
        AircraftId::Su25,
        AircraftId::Mig23,
        AircraftId::Su35,
        AircraftId::F22,
        AircraftId::F22n,
        AircraftId::Faxx,
    ] {
        same(&aircraft);
    }
    same(&Activity::HoldingMarshal);
    same(&Activity::Destroyed);
    same(&ResolvedExperience {
        level: crate::ai::Experience::Ace,
        origin: ExperienceOrigin::QuickMission {
            selected: crate::ai::Experience::Average,
        },
    });
    same(&ExperienceOrigin::EnemyOverride);
    same(&FrameEvent::ThreatReported(ThreatReport {
        missile_id: 9,
        seeker: SeekerClass::Radar,
        launcher_id: 4,
        launcher_same_side: false,
        distance_at_launch_ft: 31_000.5,
        launch_tick: 1200,
    }));
    same(&FrameEvent::ActorRemoved(3));
    same(&vec![
        Rounds::Unlimited,
        Rounds::Finite(0),
        Rounds::Finite(512),
    ]);
    same(&crate::ai::targeting::Side(2));
    same(&crate::combat::missiles::Rules::Compatibility);
    same(&crate::cheats::Cheats {
        unlimited_fuel: true,
        ..Default::default()
    });
    // A pilot's controls with every command and switch (slice H9).
    use tore_input::{PilotCommand, Switch};
    let switches = [
        Switch::Gear,
        Switch::Flaps,
        Switch::Airbrake,
        Switch::Hook,
        Switch::Bay,
        Switch::Engine,
        Switch::Burner,
        Switch::Radar,
        Switch::Jammer,
        Switch::Autopilot,
        Switch::WaypointAutopilot,
    ];
    let mut commands = vec![
        PilotCommand::Eject,
        PilotCommand::Throttle(0.75),
        PilotCommand::AdjustThrottle(-0.1),
        PilotCommand::SetAxis(tore_input::FlightAxis::VectorPitch, 0.5),
        PilotCommand::SetAxis(tore_input::FlightAxis::VectorYaw, -0.5),
        PilotCommand::AdjustAxis(tore_input::FlightAxis::Conversion, -0.25),
        PilotCommand::AdjustAxis(tore_input::FlightAxis::Collective, 0.125),
        PilotCommand::NeutralVector,
    ];
    for switch in switches {
        commands.push(PilotCommand::Toggle(switch));
        commands.push(PilotCommand::Set(switch, true));
        commands.push(PilotCommand::Set(switch, false));
    }
    same(&PilotInput {
        pitch: 0.25,
        roll: -1.,
        yaw: -0.,
        throttle_rate: 0.5,
        throttle: Some(0.9),
        vector_pitch_rate: 0.5,
        vector_yaw_rate: -0.25,
        conversion_rate: 1.,
        collective_rate: -1.,
        vector_pitch: Some(0.3),
        vector_yaw: Some(-0.7),
        conversion: None,
        collective: Some(0.6),
        commands,
    });
    same(&PilotInput::default());
}

#[test]
fn a_restored_random_stream_draws_the_same_numbers() {
    let mut stream = DecisionRandom::seeded(0xdec0);
    for _ in 0..37 {
        stream.below(100);
    }
    let mut copy: DecisionRandom = round_trip(&stream, &NONE).unwrap();
    assert_eq!(copy, stream);
    for _ in 0..1000 {
        assert_eq!(copy.below(1000), stream.below(1000));
    }
}

// ---------------------------------------------------------------------------
// Flight states.

fn model() -> AircraftModel {
    AircraftModel::for_aircraft(&profile()).unwrap()
}

fn airborne() -> State {
    let mut s = State::from_model(model(), [0., 5000., 0.]);
    s.enable_research(7).unwrap();
    s.payload_lbs = 800.;
    for tick in 0..600 {
        let input = PilotInput {
            pitch: if (100..200).contains(&tick) { 0.5 } else { 0. },
            roll: if (300..360).contains(&tick) { 0.6 } else { 0. },
            ..PilotInput::default()
        };
        s.step_surface(&input, |_, _| Surface::runway(0.));
    }
    s
}

#[test]
fn a_flight_state_round_trips_through_its_aircraft_and_flies_on_identically() {
    let mut original = airborne();
    assert!(
        original.stall_scale() != 1.,
        "the weight never scaled the envelopes"
    );
    assert_eq!(original.import_model(), model());
    let mut models = Models::default();
    models.insert(AircraftId::F18, model()).unwrap();
    models.insert(AircraftId::F18, model()).unwrap();
    assert_eq!(models.len(), 1);

    // With no models a flight cannot be coded.
    assert!(save_flight(&mut Saver::new(), &original, AircraftId::F18).is_err());
    let mut s = Saver::with_models(models.clone());
    save_flight(&mut s, &original, AircraftId::F18).unwrap();
    let body = s.finish_section();
    let mut l = Loader::new(&body, &[], &models);
    let (aircraft, mut copy) = load_flight(&mut l).unwrap();
    l.finish().unwrap();
    assert_eq!(aircraft, AircraftId::F18);
    assert_eq!(copy, original);
    for tick in 0..240 {
        let input = PilotInput {
            roll: if tick < 60 { -0.4 } else { 0. },
            ..PilotInput::default()
        };
        original.step_surface(&input, |_, _| Surface::runway(0.));
        copy.step_surface(&input, |_, _| Surface::runway(0.));
        assert_eq!(copy, original, "tick {tick}");
    }

    // A flight whose aircraft has no model in the table is refused.
    let mut s = Saver::with_models(models.clone());
    assert!(save_flight(&mut s, &original, AircraftId::Mig29).is_err());

    // Two models under one identity (test fixtures have them): each flight
    // codes its own model's ordinal, the same in any table holding both.
    let mut other = model();
    let mut configuration = other.configuration().clone();
    configuration.ejection_seat = !configuration.ejection_seat;
    other.set_configuration(configuration).unwrap();
    let mut both = Models::default();
    both.insert(AircraftId::F18, other.clone()).unwrap();
    both.insert(AircraftId::F18, model()).unwrap();
    let mut reversed = Models::default();
    reversed.insert(AircraftId::F18, model()).unwrap();
    reversed.insert(AircraftId::F18, other.clone()).unwrap();
    assert_eq!(both.len(), 2);
    assert_eq!(
        both.ordinal(AircraftId::F18, &other),
        reversed.ordinal(AircraftId::F18, &other)
    );
    let second = State::from_model(other, [0., 3000., 0.]);
    let mut s = Saver::with_models(both);
    save_flight(&mut s, &original, AircraftId::F18).unwrap();
    save_flight(&mut s, &second, AircraftId::F18).unwrap();
    let body = s.finish_section();
    let mut l = Loader::new(&body, &[], &reversed);
    assert_eq!(load_flight(&mut l).unwrap().1, original);
    assert_eq!(load_flight(&mut l).unwrap().1, second);
    l.finish().unwrap();
}

/// Slice B6: a two-seater's second chute is world state, so a checkpoint
/// keeps it though the wire's exact state does not. Restored mid-descent,
/// both chutes fall on exactly as the original's, to the ground.
#[test]
fn a_two_seaters_second_chute_survives_a_checkpoint_mid_descent() {
    use crate::ejection::Phase;
    // PLANE flags: 0x10 an ejection seat, 0x4 a second crew member.
    let mut profile = profile();
    profile.fields.get_mut("flags").unwrap().value = "20".into();
    let two_seat = AircraftModel::for_aircraft(&profile).unwrap();
    let mut models = Models::default();
    models.insert(AircraftId::F14, two_seat.clone()).unwrap();
    let mut original = State::from_model(two_seat, [0., 1500., 0.]);
    original.velocity = [0., 0., 400.];
    original.speed = 400.;
    assert!(original.eject());
    let ground = |_: f64, _: f64| 0.;
    for _ in 0..400 {
        original.step(&PilotInput::default(), ground);
    }
    let crew = original.crew_escape.clone().expect("a second chute");
    assert!(
        matches!(crew.phase, Phase::Inflating | Phase::Parachute),
        "mid-descent: {crew:?}"
    );

    let mut s = Saver::with_models(models.clone());
    save_flight(&mut s, &original, AircraftId::F14).unwrap();
    let body = s.finish_section();
    let mut l = Loader::new(&body, &[], &models);
    let (_, mut copy) = load_flight(&mut l).unwrap();
    l.finish().unwrap();
    assert_eq!(copy.crew_escape, original.crew_escape);
    assert_eq!(copy, original);
    // Room for the landing's line (nobody drains the messages here).
    original.systems.messages.clear();
    copy.systems.messages.clear();
    for tick in 0..120 * 120 {
        original.step(&PilotInput::default(), ground);
        copy.step(&PilotInput::default(), ground);
        assert_eq!(copy, original, "tick {tick}");
    }
    assert_eq!(copy.crew_escape.unwrap().phase, Phase::Landed);
    assert!(
        copy.systems
            .messages
            .iter()
            .any(|m| m == "Crew member landed safely"),
        "the restored copy tells of the crew member's landing"
    );

    // A flight with no second chute codes none.
    let mut s = Saver::with_models(models.clone());
    let mut single = original.clone();
    single.crew_escape = None;
    save_flight(&mut s, &single, AircraftId::F14).unwrap();
    let body = s.finish_section();
    let mut l = Loader::new(&body, &[], &models);
    assert!(load_flight(&mut l).unwrap().1.crew_escape.is_none());
    l.finish().unwrap();
}

// ---------------------------------------------------------------------------
// Damaged bytes.

#[derive(Clone, Debug, PartialEq)]
struct Composite {
    names: Vec<String>,
    map: BTreeMap<u32, (f64, Option<u8>)>,
    set: BTreeSet<u16>,
    deque: VecDeque<[i32; 2]>,
    mode: Mode,
    record: Sample,
    activity: Activity,
}
crate::checkpoint_struct!(Composite { names, map, set, deque, mode, activity } shared { record });

#[test]
fn damaged_and_random_bytes_are_refused_without_a_panic() {
    let value = Composite {
        names: vec!["Viper 1-1".into(), "Hornet".into()],
        map: BTreeMap::from([(1, (2.5, Some(3))), (9, (-1., None))]),
        set: BTreeSet::from([4, 8, 15]),
        deque: VecDeque::from([[1, -2], [3, -4]]),
        mode: Mode::Busy,
        record: Sample {
            hp: 3,
            position: [9.; 3],
            weapons: vec!["MK82".into()],
            big: vec![7; 3],
            scratch: vec![],
        },
        activity: Activity::Landing,
    };
    let coded = to_bytes(&value, &NONE).unwrap();
    assert_eq!(
        from_bytes::<Composite>(&coded, &NONE).unwrap().names,
        value.names
    );
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for cut in 0..coded.body.len() {
        let mut short = coded.clone();
        short.body.truncate(cut);
        let _ = from_bytes::<Composite>(&short, &NONE);
    }
    for _ in 0..10_000 {
        let mut damaged = coded.clone();
        let at = next() as usize % damaged.body.len();
        damaged.body[at] ^= 1 << (next() % 8);
        if next() % 4 == 0 && !damaged.records.is_empty() {
            let record = next() as usize % damaged.records.len();
            if !damaged.records[record].is_empty() {
                let at = next() as usize % damaged.records[record].len();
                damaged.records[record][at] ^= 1 << (next() % 8);
            }
        }
        let _ = from_bytes::<Composite>(&damaged, &NONE);
        let random = Coded {
            body: (0..(next() % 300)).map(|_| next() as u8).collect(),
            records: vec![(0..(next() % 64)).map(|_| next() as u8).collect()],
        };
        let _ = from_bytes::<Composite>(&random, &NONE);
    }
}
