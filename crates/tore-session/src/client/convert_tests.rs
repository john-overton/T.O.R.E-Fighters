//! A bot fight's capture converted into a replay, on the synthetic fixtures
//! (docs/ARCHITECTURE.md, "Converting a capture into a replay").

use super::Race;
use super::convert::{self, Conversion, End, FlightInfo, MAX_BRIDGE_TICKS};
use super::prediction::Trace;
use super::seen::FlightSeen;
use super::tests::{Rig, Shared, bot_script, host_address, spec};
use crate::wire::entity::{EntityKind, EntityState};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tore_net::Target;
use tore_net::master::Path;
use tore_net::sim::LinkConfig;
use tore_replay as replay;

/// What a fight left behind.
struct Fight {
    capture: Vec<u8>,
    /// The diagnostics log the game's client wrote.
    log: String,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    /// Where the host had every aircraft after each tick.
    truth: std::collections::HashMap<(u64, u32), [f64; 3]>,
    plane: u32,
}

/// Two bots fly a fight against a host at 150 ms and 2 percent loss for
/// `seconds`; the first one's capture and log are what comes out.
fn fight(seconds: u64) -> Fight {
    fight_through(seconds, false)
}

/// [`fight`], the first bot joining through a race of the host's addresses
/// (a join through the master, slice J2) when `raced`: its capture then names
/// the race right after the start.
fn fight_through(seconds: u64, raced: bool) -> Fight {
    fight_as(seconds, raced, false)
}

/// [`fight_through`], with the captured player (Alpha) joining after Bravo
/// when `second`, so it flies another plane than plane 0.
fn fight_as(seconds: u64, raced: bool, second: bool) -> Fight {
    let link = LinkConfig::for_round_trip(Duration::from_millis(150), 0.1, 0.02, 0.01);
    let mut rig = Rig::new(spec(2, 2, 2), link, 21);
    rig.watch = true;
    let capture = Shared::default();
    let early = second.then(|| rig.join(|c| c.callsign = "Bravo".into(), bot_script()));
    let a = rig.join(
        |c| {
            c.callsign = "Alpha".into();
            if raced {
                c.race = Some(Race {
                    targets: vec![
                        Target::new("10.0.0.99:26900".parse().unwrap(), Path::LocalNetwork),
                        Target::new(host_address(), Path::Punched),
                    ],
                    introduction: 0x51,
                });
            }
        },
        bot_script(),
    );
    rig.players[a].client.set_capture(Box::new(capture.clone()));
    let log = Shared::default();
    rig.players[a].client.set_diagnostics(Box::new(log.clone()));
    let b = early.unwrap_or_else(|| rig.join(|c| c.callsign = "Bravo".into(), bot_script()));
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(a) && r.seated(b)));
    rig.run(Duration::from_secs(seconds));
    let plane = rig.players[a].client.seat().expect("seated").1.0;
    for i in [a, b] {
        let now = rig.net.now();
        rig.players[i].client.leave_game(now);
    }
    assert!(rig.run_until(Duration::from_secs(8), |r| r.closed(a) && r.closed(b)));
    let capture = capture.0.lock().unwrap().clone();
    let log = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
    Fight {
        capture,
        log,
        resources: Arc::clone(&rig.resources),
        truth: rig.truth.clone(),
        plane,
    }
}

/// A fresh folder under the system's temporary one.
fn folder(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tore-convert-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write(conversion: &Conversion, flight: &FlightInfo, path: &std::path::Path) -> convert::Written {
    let header = conversion.header(
        flight,
        replay::World {
            theater: "UKR".into(),
            ..replay::World::default()
        },
        "0.0.0-test",
        "test",
        "2026-10-05T12:00:00Z",
    );
    conversion
        .write(flight, &header, path)
        .expect("the replay writes")
}

fn frames_of(recording: &replay::Recording) -> BTreeMap<u64, replay::Frame> {
    let (first, last) = (
        recording.first_tick().unwrap(),
        recording.last_tick().unwrap(),
    );
    recording
        .frames(first, last)
        .map(|frame| {
            let frame = frame.expect("a frame decodes");
            (frame.tick, frame)
        })
        .collect()
}

fn near(a: [f64; 3], b: [f64; 3], tolerance: f64) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= tolerance)
}

#[test]
fn a_fight_capture_converts_into_a_smooth_replay_through_every_received_state() {
    let fight = fight(20);
    let conversion = convert::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
    assert!(conversion.cut.is_none(), "a finished capture is not cut");
    let flights = conversion.flights();
    assert_eq!(flights.len(), 1, "one flight");
    let flight = &flights[0];
    assert!(flight.last_tick - flight.first_tick > 20 * 120 - 600);
    let dir = folder("smooth");
    let path = dir.join("fight.tore-replay");
    let written = write(&conversion, flight, &path);
    assert_eq!(written.frames, flight.last_tick - flight.first_tick + 1);

    let recording = replay::Recording::open(&path).expect("the replay decodes");
    assert!(recording.complete(), "{:?}", recording.problems());
    assert!(
        recording.problems().is_empty(),
        "{:?}",
        recording.problems()
    );
    assert_eq!(recording.first_tick(), Some(flight.first_tick));
    assert_eq!(recording.last_tick(), Some(flight.last_tick));
    let header = recording.header();
    assert_eq!(header.extra("net.callsign"), Some("Alpha"));
    assert!(header.extra("net.server").is_some());
    let frames = frames_of(&recording);

    let seen = &conversion.observed().flights[0];
    let mut checked = 0u64;
    let mut moving = 0u64;
    for (key, states) in &seen.states {
        if key.kind != EntityKind::Aircraft {
            continue;
        }
        // The replay keeps every plane's real id.
        let id = key.id;
        // Through every received state, within the format's precision.
        for (tick, state) in states {
            let tick = u64::from(*tick);
            let Some(frame) = frames.get(&tick) else {
                continue;
            };
            let EntityState::Aircraft(received) = state else {
                continue;
            };
            let at = frame
                .aircraft
                .iter()
                .find(|a| a.id == id)
                .unwrap_or_else(|| panic!("aircraft {id} at tick {tick}"));
            assert!(
                near(at.position, received.motion.position_ft(), 1. / 64. + 1e-9),
                "aircraft {id} at {tick}: {:?} against {:?}",
                at.position,
                received.motion.position_ft()
            );
            checked += 1;
        }
        // And never a step larger than its speed allows between them.
        for pair in states.windows(2) {
            let ((t0, s0), (t1, s1)) = (pair[0], pair[1]);
            if t1 - t0 > MAX_BRIDGE_TICKS {
                continue;
            }
            let (EntityState::Aircraft(a), EntityState::Aircraft(b)) = (s0, s1) else {
                continue;
            };
            let speed = |v: [f64; 3]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
            let (p0, p1) = (a.motion.position_ft(), b.motion.position_ft());
            let travelled =
                speed(std::array::from_fn(|i| p1[i] - p0[i])) * 120. / f64::from(t1 - t0);
            let allowed = speed(a.motion.velocity_fps())
                .max(speed(b.motion.velocity_fps()))
                .max(travelled)
                * 1.5
                / 120.
                + 0.1;
            for tick in u64::from(t0)..u64::from(t1) {
                let (Some(f0), Some(f1)) = (frames.get(&tick), frames.get(&(tick + 1))) else {
                    continue;
                };
                let (Some(x0), Some(x1)) = (
                    f0.aircraft.iter().find(|a| a.id == id),
                    f1.aircraft.iter().find(|a| a.id == id),
                ) else {
                    continue;
                };
                let step = speed(std::array::from_fn(|i| x1.position[i] - x0.position[i]));
                assert!(
                    step <= allowed,
                    "aircraft {id} stepped {step} ft at tick {tick}, allowed {allowed}"
                );
                moving += 1;
            }
        }
    }
    assert!(checked > 1000, "{checked} received states were checked");
    assert!(moving > 1000, "{moving} steps were checked");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_own_aircraft_is_the_hosts_state_at_every_exact_state() {
    let fight = fight(20);
    let conversion = convert::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
    let flight = conversion.flights().remove(0);
    assert_eq!(flight.plane, fight.plane);
    let dir = folder("own");
    let path = dir.join("own.tore-replay");
    write(&conversion, &flight, &path);
    let recording = replay::Recording::open(&path).unwrap();
    let frames = frames_of(&recording);
    let seen = &conversion.observed().flights[0];
    let mut exact = 0;
    let mut against_host = 0;
    for step in &seen.trace {
        let Trace::Host { sample, .. } = step else {
            continue;
        };
        let Some(frame) = frames.get(&sample.tick) else {
            continue;
        };
        let own = frame
            .aircraft
            .iter()
            .find(|a| a.id == flight.plane)
            .expect("the player");
        assert!(
            near(own.position, sample.pose.position, 1. / 64. + 1e-9),
            "tick {}: {:?} against the host's {:?}",
            sample.tick,
            own.position,
            sample.pose.position
        );
        exact += 1;
        // The host's own world agrees.
        if let Some(truth) = fight.truth.get(&(sample.tick, fight.plane)) {
            assert!(
                near(own.position, *truth, 1. / 64. + 1e-6),
                "tick {}",
                sample.tick
            );
            against_host += 1;
        }
    }
    assert!(exact >= 18, "{exact} exact states, about one a second");
    assert!(
        against_host >= 18,
        "{against_host} compared with the host's world"
    );
    // The player keeps its plane's id in the replay, and the roster says so.
    let you = recording
        .aircraft_info(flight.plane)
        .expect("a roster entry for the player");
    assert_eq!(you.label, "You");
    assert!(you.human);
    assert_eq!(recording.aircraft().count(), 4);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_diagnostics_ride_in_the_replay_and_match_the_log() {
    let fight = fight(20);
    let conversion = convert::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
    let flight = conversion.flights().remove(0);
    // The conversion's replayed client writes the same lines the game's did.
    let original: Vec<&str> = fight.log.lines().skip(1).collect();
    let rewritten: Vec<&str> = conversion
        .diagnostic_lines()
        .iter()
        .map(|line| line.text.as_str())
        .collect();
    assert_eq!(original, rewritten);
    let dir = folder("diag");
    let path = dir.join("diag.tore-replay");
    write(&conversion, &flight, &path);
    let recording = replay::Recording::open(&path).unwrap();
    let stats: Vec<_> = recording
        .events()
        .iter()
        .filter(|e| e.event.kind == replay::vocab::kind::NET_STATS)
        .collect();
    let stats_lines = original.iter().filter(|l| l.contains("\tstats\t")).count();
    assert_eq!(stats.len(), stats_lines);
    assert!(stats_lines >= 20);
    // A figure reads back as the log says it.
    let first_log: Vec<&str> = original
        .iter()
        .find(|l| l.contains("\tstats\t"))
        .unwrap()
        .split('\t')
        .collect();
    let rtt: f64 = first_log[2].parse().unwrap();
    assert_eq!(stats[0].event.num("round_trip_ms"), Some(rtt));
    assert!(
        recording
            .events()
            .iter()
            .any(|e| e.event.kind == replay::vocab::kind::NET_EVENT
                && e.event.string("kind") == Some("seated"))
    );
    let footer = recording.footer().expect("a footer");
    let value = |key: &str| {
        footer
            .result
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(value("end"), Some("end flight"));
    let mean: f64 = value("net.rtt_ms_mean").unwrap().parse().unwrap();
    assert!(mean > 100. && mean < 250., "mean round trip {mean}");
    assert!(value("net.corrections").is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn converting_twice_gives_the_same_bytes() {
    let fight = fight(10);
    let dir = folder("twice");
    let mut files = Vec::new();
    for n in 0..2 {
        let conversion = convert::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
        let flight = conversion.flights().remove(0);
        let path = dir.join(format!("{n}.tore-replay"));
        write(&conversion, &flight, &path);
        files.push(std::fs::read(&path).unwrap());
    }
    assert!(files[0].len() > 10_000);
    assert!(files[0] == files[1], "the two replays differ");
    let _ = std::fs::remove_dir_all(dir);
}

/// A join through the master (slice J2) names its race right after the start:
/// the conversion runs the client from it and gets the same flight as a
/// direct join's, and the replay carries the host the race chose.
#[test]
fn a_capture_of_a_raced_join_converts_like_a_direct_one() {
    let fight = fight_through(10, true);
    let mut reader = super::capture::Reader::new(&fight.capture).unwrap();
    assert!(matches!(
        reader.next_record().unwrap(),
        Some(super::capture::Record::Start { .. })
    ));
    assert!(matches!(
        reader.next_record().unwrap(),
        Some(super::capture::Record::Race(_))
    ));
    let conversion = convert::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
    assert!(conversion.cut.is_none(), "a finished capture is not cut");
    let flights = conversion.flights();
    assert_eq!(flights.len(), 1, "one flight");
    let dir = folder("raced");
    let path = dir.join("raced.tore-replay");
    let written = write(&conversion, &flights[0], &path);
    assert!(written.frames > 10 * 120 - 600);
    let recording = replay::Recording::open(&path).expect("the replay decodes");
    assert!(recording.complete(), "{:?}", recording.problems());
    assert_eq!(recording.header().extra("net.callsign"), Some("Alpha"));
    assert_eq!(
        recording.header().extra("net.server"),
        Some(host_address().to_string().as_str()),
        "the host the race chose"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_cut_capture_converts_to_its_last_whole_record_and_says_so() {
    let fight = fight(10);
    let dir = folder("cut");
    // Cut in the middle of a record, well into the flight.
    let at = fight.capture.len() * 3 / 5;
    let cut = &fight.capture[..at];
    let conversion = convert::observe(cut, Arc::clone(&fight.resources)).unwrap();
    let cut_info = conversion.cut.expect("a cut capture says so");
    assert!(cut_info.at_byte <= at && at - cut_info.at_byte < 70_000);
    assert_eq!(cut_info.of_bytes, at);
    let flight = conversion.flights().remove(0);
    let path = dir.join("cut.tore-replay");
    let written = write(&conversion, &flight, &path);
    assert!(written.cut.is_some());
    let recording = replay::Recording::open(&path).unwrap();
    assert!(recording.complete());
    let footer = recording.footer().unwrap();
    let result = |key: &str| {
        footer
            .result
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(result("end").as_deref(), Some("cut"));
    assert!(result("capture").unwrap().starts_with("cut short at byte"));
    // Shorter than the whole flight's replay.
    let whole = convert::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
    let whole = whole.flights().remove(0);
    assert!(flight.last_tick < whole.last_tick);
    // A cut before any plane was given leaves nothing to replay, and says why.
    let early = convert::observe(&fight.capture[..3000], Arc::clone(&fight.resources)).unwrap();
    assert!(early.flights().is_empty());
    assert!(early.no_flight_reason().contains("capture holds no flight"));
    assert!(matches!(early.end_of(0), End::Cut));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_player_on_another_plane_than_zero_keeps_its_real_id() {
    // Alpha joins second, so it flies plane 1 (plane 0 is Bravo's).
    let fight = fight_as(12, false, true);
    assert_ne!(fight.plane, 0, "the captured player is not on plane 0");
    let conversion = convert::observe(&fight.capture, Arc::clone(&fight.resources)).unwrap();
    let flight = conversion.flights().remove(0);
    assert_eq!(flight.plane, fight.plane);
    let dir = folder("seat");
    let path = dir.join("seat.tore-replay");
    write(&conversion, &flight, &path);
    let recording = replay::Recording::open(&path).unwrap();
    let header = recording.header();
    let plane = flight.plane.to_string();
    // The viewer follows draw.player, and net.player_plane says the same.
    assert_eq!(header.extra("draw.player"), Some(plane.as_str()));
    assert_eq!(
        header.extra(convert::PLAYER_PLANE_KEY),
        Some(plane.as_str())
    );
    assert_eq!(convert::DRAW_PLAYER_KEY, "draw.player");
    // The roster names the seat's own plane `You` and plane 0 as another.
    let you = recording.aircraft_info(flight.plane).expect("the player");
    assert_eq!((you.label.as_str(), you.human), ("You", true));
    let zero = recording.aircraft_info(0).expect("plane 0");
    assert_ne!(zero.label, "You");
    // Every frame lists both under their own ids.
    let frames = frames_of(&recording);
    let frame = frames.values().last().unwrap();
    assert!(frame.aircraft.iter().any(|a| a.id == flight.plane));
    assert!(frame.aircraft.iter().any(|a| a.id == 0));
    // The slots reach the highest other plane, the player's aside.
    let slots: u32 = header.extra("draw.slots").unwrap().parse().unwrap();
    assert!(slots >= 3, "{slots}");
    let _ = std::fs::remove_dir_all(dir);
}

#[allow(dead_code)]
fn flights_of(conversion: &Conversion) -> &[FlightSeen] {
    &conversion.observed().flights
}

// ----- The curves, on states made by hand ---------------------------------

mod curves {
    use super::*;
    use crate::client::convert::smooth::{Own, Track};
    use crate::client::interpolation::Sample;
    use crate::client::seen::OwnSample;
    use crate::wire::entity::{
        AircraftState, DebrisState, EntityKey, EntityState, Motion, PilotState, ProjectileState,
    };
    use crate::wire::names::NameIndex;
    use tore_world::snapshot::AircraftPose;

    fn aircraft(position: [f64; 3], velocity: [f64; 3]) -> EntityState {
        EntityState::Aircraft(AircraftState {
            aircraft: Some(tore_formats::aircraft::AircraftId::F18),
            motion: Motion::of(position, velocity),
            ..AircraftState::default()
        })
    }

    fn key(id: u32) -> EntityKey {
        EntityKey {
            kind: EntityKind::Aircraft,
            id,
        }
    }

    fn position(sample: Sample) -> [f64; 3] {
        match sample {
            Sample::Aircraft(pose) => pose.position,
            Sample::Projectile(_, position, ..) | Sample::Debris(_, position, _) => position,
            Sample::Pilot(_, position, _) => position,
        }
    }

    #[test]
    fn a_track_follows_its_states_and_never_guesses_beyond_them() {
        // 600 ft/s along x, a state every 4 ticks, and then silence.
        let states: Vec<(u32, EntityState)> = (0..=5)
            .map(|n| {
                (
                    100 + n * 4,
                    aircraft([f64::from(n) * 20., 5000., 0.], [600., 0., 0.]),
                )
            })
            .collect();
        let mut track = Track::new(key(3), &states);
        assert!(track.at(99).is_none(), "nothing before its first state");
        for tick in 100..=120u64 {
            let at = position(track.at(tick).expect("inside its states"));
            let truth = (tick - 100) as f64 * 5.;
            assert!(
                (at[0] - truth).abs() < 0.04,
                "tick {tick}: {} against {truth}",
                at[0]
            );
        }
        assert!(track.at(121).is_none(), "nothing after its last state");
        assert!(track.at(400).is_none());
    }

    #[test]
    fn a_silence_longer_than_two_seconds_is_not_bridged() {
        let states = vec![
            (0u32, aircraft([0.; 3], [100., 0., 0.])),
            (240, aircraft([200., 0., 0.], [100., 0., 0.])),
            (481, aircraft([400., 0., 0.], [100., 0., 0.])),
        ];
        let mut track = Track::new(key(1), &states);
        assert!(track.at(120).is_some(), "exactly two seconds is bridged");
        assert!(track.at(240).is_some());
        assert!(track.at(300).is_none(), "241 ticks is not");
        assert!(
            track.at(481).is_some(),
            "a state is always shown at its own tick"
        );
    }

    #[test]
    fn a_far_entity_is_drawn_smoothly_between_its_updates_not_held() {
        // 60 ticks apart, turning: the middle lies on the curve, not on either end.
        let states = vec![
            (0u32, aircraft([0., 0., 0.], [500., 0., 0.])),
            (60, aircraft([250., 0., 250.], [0., 0., 500.])),
        ];
        let mut track = Track::new(key(9), &states);
        let middle = position(track.at(30).unwrap());
        assert!(
            middle[0] > 100. && middle[0] < 250. && middle[2] > 20.,
            "{middle:?}"
        );
    }

    #[test]
    fn projectiles_debris_and_pilots_follow_their_states_too() {
        let projectile = |x: f64| {
            EntityState::Projectile(ProjectileState {
                owner: 2,
                weapon: NameIndex(0),
                shape: None,
                target: Some(1),
                aimed_at_player: false,
                motion: Motion::of([x, 3000., 0.], [1200., 0., 0.]),
                direction: [0x4000, 0],
            })
        };
        let states = vec![(10u32, projectile(0.)), (14, projectile(40.))];
        let mut track = Track::new(
            EntityKey {
                kind: EntityKind::Projectile,
                id: 77,
            },
            &states,
        );
        match track.at(12).unwrap() {
            Sample::Projectile(state, position, velocity, _) => {
                assert!((position[0] - 20.).abs() < 0.1);
                assert_eq!(state.target, Some(1));
                assert!((velocity[0] - 1200.).abs() < 1e-9);
            }
            _ => panic!("a projectile"),
        }
        let debris = vec![(
            0u32,
            EntityState::Debris(DebrisState {
                owner: 4,
                model: None,
                variant: None,
                motion: Motion::of([1., 2., 3.], [0., -32., 0.]),
                attitude: [0; 3],
            }),
        )];
        let mut track = Track::new(
            EntityKey {
                kind: EntityKind::Debris,
                id: 4,
            },
            &debris,
        );
        assert!(matches!(track.at(0), Some(Sample::Debris(..))));
        assert!(track.at(1).is_none());
        let pilot = vec![(
            5u32,
            EntityState::Pilot(PilotState {
                owner: 4,
                motion: Motion::of([0.; 3], [0.; 3]),
                heading: 0,
                phase: tore_sim::ejection::Phase::Parachute,
            }),
        )];
        let mut track = Track::new(
            EntityKey {
                kind: EntityKind::Pilot,
                id: 4,
            },
            &pilot,
        );
        assert!(track.at(4).is_none() && track.at(5).is_some());
    }

    fn sample(tick: u64, x: f64) -> OwnSample {
        OwnSample {
            tick,
            pose: AircraftPose {
                position: [x, 5000., 0.],
                velocity: [600., 0., 0.],
                ..AircraftPose::default()
            },
            airspeed: 600.,
            g: 1.,
            fuel_lb: 100.,
            controls: [0.; 4],
            on_ground: false,
            alive: true,
            ejected: false,
            wreck_gone: false,
        }
    }

    #[test]
    fn a_wrong_prediction_is_spread_back_over_the_span_before_the_exact_state() {
        // Seated at tick 0, predicted 5 ft a tick for 120 ticks; the host's
        // state at tick 120 is 12 ft further along than the prediction.
        let mut trace = vec![Trace::Host {
            sample: sample(0, 0.),
            differs: false,
        }];
        for tick in 1..=130 {
            trace.push(Trace::Stepped(sample(tick, tick as f64 * 5.)));
        }
        trace.push(Trace::Host {
            sample: sample(120, 612.),
            differs: true,
        });
        // The restart steps the ticks after it again from the host's state.
        for tick in 121..=130 {
            trace.push(Trace::Stepped(sample(
                tick,
                612. + (tick - 120) as f64 * 5.,
            )));
        }
        let own = Own::new(&trace);
        let x = |tick| own.at(tick).unwrap().pose.position[0];
        assert_eq!(x(0), 0., "exact at the earlier exact state");
        assert_eq!(x(120), 612., "exactly the host's at the later one");
        // Halfway, half the error has been added; nothing jumps.
        assert!((x(60) - (300. + 6.)).abs() < 1e-9);
        let worst_step = (0..120).map(|t| (x(t + 1) - x(t)).abs()).fold(0., f64::max);
        assert!(
            worst_step < 5.2,
            "no step is larger than 5.1 ft: {worst_step}"
        );
        assert_eq!(x(125), 612. + 25.);
    }

    #[test]
    fn an_exact_state_equal_to_the_prediction_changes_nothing() {
        let mut trace = vec![Trace::Host {
            sample: sample(0, 0.),
            differs: false,
        }];
        for tick in 1..=240 {
            trace.push(Trace::Stepped(sample(tick, tick as f64 * 5.)));
        }
        trace.push(Trace::Host {
            sample: sample(120, 600.),
            differs: false,
        });
        let own = Own::new(&trace);
        for tick in [1u64, 60, 119, 120, 121, 240] {
            assert_eq!(own.at(tick).unwrap().pose.position[0], tick as f64 * 5.);
        }
    }

    #[test]
    fn a_tick_nobody_predicted_lies_on_a_curve_between_its_neighbours() {
        // The host's state came for a tick far ahead: nothing was predicted
        // in between.
        let trace = vec![
            Trace::Host {
                sample: sample(0, 0.),
                differs: false,
            },
            Trace::Host {
                sample: sample(120, 600.),
                differs: true,
            },
        ];
        let own = Own::new(&trace);
        let at = own.at(60).unwrap().pose.position[0];
        assert!((at - 300.).abs() < 1e-6, "{at}");
        assert!(own.at(121).is_none());
    }
}

mod events {
    use super::*;
    use crate::client::convert::events::{Events, datalink_event};
    use crate::client::seen::SeenEvent;
    use crate::wire::events::{ReceivedEvent, WireEvent};
    use crate::wire::messages::Names;
    use crate::wire::names::{NameIndex, ReceivedNames};
    use tore_replay::vocab::{field, kind};
    use tore_sim::combat::blast::MarkKind;
    use tore_sim::combat::live::EffectKind;
    use tore_world::comms::Route;

    fn seen(events: Vec<(u32, WireEvent)>) -> FlightSeen {
        let mut names = ReceivedNames::new();
        names
            .apply(&Names {
                flight: 1,
                first: 0,
                names: vec!["^CLRLAND".into(), "AIM-120C".into(), "FLARE.WAV".into()],
            })
            .unwrap();
        FlightSeen {
            events: events
                .into_iter()
                .enumerate()
                .map(|(n, (tick, event))| SeenEvent {
                    arrived: Duration::from_millis(tick as u64 * 8),
                    event: ReceivedEvent {
                        number: n as u16,
                        tick,
                        event,
                    },
                })
                .collect(),
            names,
            ..FlightSeen::default()
        }
    }

    fn run(seen: &FlightSeen, player: u32, first: u64, last: u64) -> Vec<replay::Frame> {
        let weapons = BTreeMap::from([("AIM-120C".to_owned(), 5u32)]);
        let mut events = Events::new(seen, player, &weapons, first, last);
        (first..=last)
            .map(|tick| {
                let mut frame = replay::Frame {
                    tick,
                    ..replay::Frame::default()
                };
                events.fill(tick, &mut frame);
                frame
            })
            .collect()
    }

    #[test]
    fn the_hosts_events_become_the_replays_on_the_ticks_the_host_gave() {
        let seen = seen(vec![
            (
                10,
                WireEvent::Message {
                    text: "Radar on".into(),
                },
            ),
            (
                12,
                WireEvent::Radio {
                    route: Route::Radio,
                    important: true,
                    net: tore_world::comms::Net::Wing,
                    label: "Friendly 1-2".into(),
                    text: "Cleared to land".into(),
                    stems: vec![NameIndex(0)],
                },
            ),
            (
                14,
                WireEvent::Launch {
                    shooter: 3,
                    projectile: 70_001,
                    weapon: NameIndex(1),
                },
            ),
            (
                15,
                WireEvent::Effect {
                    kind: EffectKind::Launch,
                    position: [32, 64, 96],
                    ticks: 20,
                    blast: None,
                },
            ),
            (
                16,
                WireEvent::Mark {
                    kind: MarkKind::Crater(4),
                    position: [0, 0, 0],
                },
            ),
            (17, WireEvent::GroundDestroyed { object: 900 }),
            (
                18,
                WireEvent::Countermeasure {
                    aircraft: 3,
                    flare: true,
                    position: [32, 32, 32],
                    velocity: [64, 0, 0],
                    attitude: [0, 0, 0],
                    number: 7,
                    left: Some(29),
                },
            ),
            (
                19,
                WireEvent::GunBurst {
                    shooter: 0,
                    station: 1,
                    length: Some(30),
                },
            ),
            (
                20,
                WireEvent::Release {
                    sound: NameIndex(2),
                    station: 2,
                },
            ),
            // Before the replay starts and after it ends: left out.
            (
                3,
                WireEvent::Message {
                    text: "early".into(),
                },
            ),
            (
                99,
                WireEvent::Message {
                    text: "late".into(),
                },
            ),
        ]);
        // The player flew plane 3: the seat's lines are plane 3's, and so
        // is the launch of its own shooter.
        let frames = run(&seen, 3, 5, 40);
        let at = |tick: u64| &frames[(tick - 5) as usize];
        let hud = &at(10).events[0];
        assert_eq!(hud.kind, kind::COMMS_HUD);
        assert_eq!(hud.text, "Radar on");
        assert_eq!(hud.subject, Some(3));
        let radio = &at(12).events[0];
        assert_eq!(radio.kind, kind::COMMS_RADIO);
        assert_eq!(radio.string(field::STEMS), Some("^CLRLAND"));
        assert_eq!(radio.string(field::SPEAKER), Some("Friendly 1-2"));
        assert!(replay::vocab::heard(radio), "a line the player heard");
        let launch = &at(14).events[0];
        assert_eq!(launch.kind, kind::WEAPON_LAUNCH);
        assert_eq!(launch.subject, Some(3), "the shooter is the player");
        assert_eq!(launch.get(field::WEAPON), Some(&replay::Value::Id(5)));
        let effect = &at(15).new_effects[0];
        assert_eq!(effect.kind, replay::EffectKind::Launch);
        assert_eq!(effect.position, [1., 2., 3.]);
        assert_eq!(effect.duration_ticks, 20);
        let crater = &at(16).new_effects[0];
        assert_eq!(crater.kind, replay::EffectKind::Crater(4));
        assert_eq!(at(17).surface_hp, vec![(900, 0)]);
        let flare = &at(18).events[0];
        assert_eq!(flare.kind, kind::COMBAT_COUNTERMEASURE);
        assert_eq!(flare.string(field::DECOY), Some("flare"));
        assert_eq!(flare.num(field::NUMBER), Some(7.));
        assert_eq!(flare.num("vx_fps"), Some(1.));
        assert_eq!(flare.num("x_ft"), Some(1.));
        assert_eq!(flare.num(field::LEFT), Some(29.));
        assert_eq!(flare.subject, Some(3));
        let burst = &at(19).events[0];
        assert_eq!(burst.kind, kind::WEAPON_GUN_BURST);
        assert_eq!(burst.subject, Some(0), "plane 0 keeps its own id");
        assert_eq!(at(20).events[0].string(field::SOUND), Some("FLARE.WAV"));
        let all: usize = frames.iter().map(|f| f.events.len()).sum();
        assert_eq!(all, 7, "the early and the late event are left out");
    }

    /// The host's Link events become the replay's `datalink.*` events, as
    /// the single-player recorder writes them (slice G7).
    #[test]
    fn the_hosts_link_events_become_the_replays_datalink_events() {
        use crate::wire::events::LinkEvent;
        use tore_sim::ai::wing::PlayerOrder;
        use tore_world::datalink::ClearReason;
        let changes = [
            LinkEvent::Member {
                plane: 1,
                radar: false,
            },
            LinkEvent::Lock {
                plane: 1,
                target: 9,
            },
            LinkEvent::Unlock {
                plane: 1,
                target: 9,
            },
            LinkEvent::Assign {
                plane: 1,
                target: 9,
                by: 0,
                order: PlayerOrder::Sort,
            },
            LinkEvent::Acknowledge {
                plane: 1,
                target: 9,
            },
            LinkEvent::Clear {
                plane: 1,
                target: 9,
                why: ClearReason::TargetLost,
            },
            LinkEvent::SortWarning {
                plane: 0,
                other: 1,
                target: 9,
            },
        ];
        let seen = seen(
            changes
                .iter()
                .enumerate()
                .map(|(i, change)| (10 + i as u32, WireEvent::Link(*change)))
                .collect(),
        );
        let frames = run(&seen, 0, 5, 40);
        let at = |tick: u64| &frames[(tick - 5) as usize].events[0];
        for (i, change) in changes.iter().enumerate() {
            let tick = 10 + i as u64;
            assert_eq!(*at(tick), datalink_event(&change.entry(tick)));
        }
        let member = at(10);
        assert_eq!(member.kind, kind::DATALINK_MEMBER);
        assert_eq!(
            (member.subject, member.get(field::RADAR)),
            (Some(1), Some(&replay::Value::Bool(false)))
        );
        let assign = at(13);
        assert_eq!(assign.kind, kind::DATALINK_ASSIGN);
        assert_eq!((assign.subject, assign.object), (Some(0), Some(1)));
        assert_eq!(assign.get(field::TARGET), Some(&replay::Value::Id(9)));
        assert_eq!(assign.string(field::ORDER), Some("Sort"));
        let clear = at(15);
        assert_eq!(clear.kind, kind::DATALINK_CLEAR);
        assert_eq!(clear.string(field::REASON), Some("target lost"));
        let warning = at(16);
        assert_eq!(warning.kind, kind::DATALINK_SORT_WARNING);
        assert_eq!(warning.get(field::OTHER), Some(&replay::Value::Id(1)));
        assert_eq!(at(11).kind, kind::DATALINK_LOCK);
        assert_eq!(at(12).kind, kind::DATALINK_UNLOCK);
        assert_eq!(at(14).kind, kind::DATALINK_ACKNOWLEDGE);
    }
}
