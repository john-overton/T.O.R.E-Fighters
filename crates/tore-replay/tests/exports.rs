//! Golden exports from a small synthetic recording, anomaly flags, and the
//! comparison of two recordings that diverge at a known tick.
//!
//! The golden recording is built from additions, multiplications,
//! divisions and square roots only (sine and cosine come from a short
//! series), so its numbers are identical on every platform.

mod common;

use common::temp_dir;
use std::path::{Path, PathBuf};
use tore_replay::export::{
    AcmiOptions, CompareOptions, JsonlOptions, SummaryOptions, Thresholds, compare, detect,
    write_acmi, write_diff, write_jsonl, write_summary,
};
use tore_replay::vocab::{channel, field, kind, node, outcome, unit};
use tore_replay::*;

const PI: f64 = std::f64::consts::PI;

/// Sine by series; exact to 1e-9 on -pi/2..pi/2 and the same everywhere.
fn sin(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    for n in 1..9 {
        term = -term * x2 / f64::from((2 * n) * (2 * n + 1));
        sum += term;
    }
    sum
}

fn cos(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = 1.;
    let mut sum = 1.;
    for n in 1..9 {
        term = -term * x2 / f64::from((2 * n - 1) * (2 * n));
        sum += term;
    }
    sum
}

const X0: f64 = 840_000.;
const Z0: f64 = 800_000.;

fn header() -> Header {
    Header {
        game_version: "0.1.0".into(),
        game_commit: "golden".into(),
        recorded_at: "2026-09-26T15:40:00Z".into(),
        mission: MissionKind::QuickMission,
        world: World {
            theater: "UKR".into(),
            theater_name: "Ukraine".into(),
            layout: "UKR.MM".into(),
            weather: Some(1),
            weather_name: "clear".into(),
            weather_seed: Some(42),
            time_of_day_s: 43_200.,
            wind_fps: [0., 0., -12.5],
            clouds: Clouds {
                module: "DAY2.LAY".into(),
                deck_ft: None,
            },
            extent_ft: Some([1_695_744., 1_630_208.]),
        },
        extra: vec![("flight_model".into(), "researched".into())],
        ..Header::default()
    }
}

fn registry() -> (Vec<AircraftInfo>, Vec<WeaponInfo>) {
    (
        vec![
            AircraftInfo {
                id: 0,
                pt: "F18.PT".into(),
                name: "F/A-18D".into(),
                label: "You".into(),
                side: Side::Friendly,
                wing: 1,
                member: 1,
                skill: String::new(),
                human: true,
            },
            AircraftInfo {
                id: 1,
                pt: "F18.PT".into(),
                name: "F/A-18D".into(),
                label: "Friendly 1-2".into(),
                side: Side::Friendly,
                wing: 1,
                member: 2,
                skill: "Regular".into(),
                human: false,
            },
            AircraftInfo {
                id: 2,
                pt: "MIG29.PT".into(),
                name: "MiG-29".into(),
                label: "Enemy 2-1".into(),
                side: Side::Enemy,
                wing: 2,
                member: 1,
                skill: "Veteran".into(),
                human: false,
            },
        ],
        vec![
            WeaponInfo {
                id: 1,
                source: "AIM120.PT".into(),
                shape: Some("AIM120.SH".into()),
                name: "AIM-120".into(),
                class: WeaponClass::Missile,
            },
            WeaponInfo {
                id: 2,
                source: "M61.PT".into(),
                shape: None,
                name: "M61A1".into(),
                class: WeaponClass::Gun,
            },
        ],
    )
}

/// The player flies a steady right turn: 60 degrees of right bank, heading
/// from north to east over four seconds at 800 ft/s.
fn player(tick: u64) -> AircraftState {
    let t = tick as f64 / 120.;
    let rate = PI / 8.;
    let speed = 800.;
    let radius = speed / rate;
    let heading = rate * t;
    let (s, c) = (sin(heading), cos(heading));
    AircraftState {
        id: 0,
        position: [X0 + radius * (1. - c), 20_000., Z0 + radius * s],
        attitude: [heading, 0., PI / 3.],
        velocity: [speed * s, 0., speed * c],
        airspeed: speed,
        g: 2.,
        devices: [0., 0., 0., 0., 0., 0.5, 0.25, 0.4, 0., speed, 0.9],
        heat: 0.8,
        flags: AircraftFlags {
            engine_on: true,
            airborne: true,
            alive: true,
            ..AircraftFlags::default()
        },
        wreck_phase: 0,
        fuel_lb: 9_000. - t * 2.5,
        controls: [0.25, 0.4, 0., 0.9],
        auxiliary_rates: [0.; 3],
        hp: 1_000,
        max_hp: 1_000,
        sections: [0; 6],
        structural_section: None,
    }
}

/// The enemy flies straight west until the missile hits at tick 300, then
/// falls as a wreck that is gone at tick 420.
fn enemy(tick: u64) -> AircraftState {
    let t = tick as f64 / 120.;
    let dead = tick >= 300;
    AircraftState {
        id: 2,
        position: [
            X0 + 6_000. - 600. * t,
            18_000. - if dead { (t - 2.5) * 400. } else { 0. },
            Z0 + 2_000.,
        ],
        attitude: [1.5 * PI, 0., 0.],
        velocity: [-600., if dead { -400. } else { 0. }, 0.],
        airspeed: 600.,
        g: 1.,
        devices: [0., 0., 0., 0., 0., 0.4, 0., 0., 0., 600., 0.7],
        heat: 0.6,
        flags: AircraftFlags {
            engine_on: !dead,
            airborne: true,
            alive: !dead,
            wreck_gone: tick >= 420,
            ..AircraftFlags::default()
        },
        wreck_phase: if dead { 1 } else { 0 },
        fuel_lb: 5_000. - t * 2.,
        controls: [0., 0., 0., 0.7],
        auxiliary_rates: [0.; 3],
        hp: if dead { 0 } else { 800 },
        max_hp: 800,
        sections: if dead { [0, 400, 0, 0, 0, 0] } else { [0; 6] },
        structural_section: None,
    }
}

/// The missile closes on the enemy from the launch point, arriving at tick
/// 300: a straight blend from where it started to where the enemy is now.
fn missile(tick: u64) -> [f64; 3] {
    let start = player(120).position;
    let target = enemy(tick).position;
    let f = (tick as f64 - 120.) / 180.;
    [0, 1, 2].map(|i| start[i] + (target[i] - start[i]) * f)
}

fn golden_frames() -> Vec<Frame> {
    let launch = 120;
    let mut frames = Vec::new();
    for tick in 0..480u64 {
        let mut frame = Frame {
            tick,
            aircraft: vec![player(tick), enemy(tick)],
            ..Frame::default()
        };
        if frame.aircraft[1].flags.wreck_gone {
            frame.aircraft.pop();
        }
        if (launch..300).contains(&tick) {
            let age = tick - launch;
            let position = missile(tick);
            let previous = if age == 0 {
                position
            } else {
                missile(tick - 1)
            };
            let ahead = missile(tick + 1);
            let step: Vec<f64> = (0..3).map(|i| ahead[i] - position[i]).collect();
            let length = (step[0] * step[0] + step[1] * step[1] + step[2] * step[2]).sqrt();
            let direction = [step[0] / length, step[1] / length, step[2] / length];
            frame.projectiles.push(ProjectileState {
                id: 1,
                owner: 0,
                weapon: 1,
                target: Some(2),
                position,
                previous,
                direction,
                speed: length * 120.,
                tracer: false,
                incoming: false,
                age: age as u32,
                seeker: Some(Seeker {
                    acquired: age > 60,
                    status: 2,
                    quality: 0.5,
                    target: Some(2),
                }),
            });
            if age.is_multiple_of(8) {
                frame.new_puffs.push(PuffSpawn {
                    layer: LAYER_SMOKE,
                    kind: PuffKind::Missile,
                    position,
                });
            }
        }
        match tick {
            0 => frame.events.push(
                Event::new(kind::AI_ACTIVITY)
                    .with_subject(2)
                    .with(field::FROM, "FORMATION")
                    .with(field::TO, "ATTACKING")
                    .with(field::REASON, "leader released wing"),
            ),
            120 => frame.events.push(
                Event::new(kind::WEAPON_LAUNCH)
                    .with_subject(0)
                    .with_object(2)
                    .with(field::PROJECTILE, Value::Id(1))
                    .with(field::WEAPON, Value::Id(1))
                    .with(field::CLASS, "missile")
                    .with(field::MODE, "radar")
                    .with(field::RANGE_FT, 5_450.)
                    .with(field::ASPECT_DEG, 35.)
                    .with(field::OFF_BORESIGHT_DEG, 12.)
                    .with(field::CLOSURE_KT, 820.)
                    .with(field::SHOOTER_ALT_FT, 20_000.)
                    .with(field::TARGET_ALT_FT, 18_000.)
                    .with(field::SHOOTER_SPEED_KT, 474.)
                    .with(field::TARGET_SPEED_KT, 355.),
            ),
            125 => frame.events.push(
                Event::new(kind::COMMS_RADIO)
                    .with_subject(0)
                    .with(field::SPEAKER, "You")
                    .with(field::HEARD, true)
                    .with(field::OUTCOME, outcome::DELIVERED)
                    .with(field::TRIGGER, "missile release")
                    .with(field::WAIT_S, 0.25)
                    .with_text("Fox three, one away"),
            ),
            130 => {
                frame.events.push(
                    Event::new(kind::COMMS_ORDER)
                        .with_subject(0)
                        .with(field::MESSAGE, 7i64)
                        .with(field::RECIPIENTS, Value::Ids(vec![1]))
                        .with(field::ORDER, "engage my target"),
                );
                frame.events.push(
                    Event::new(kind::COMMS_DELIVERY)
                        .with_subject(1)
                        .with_object(0)
                        .with(field::MESSAGE, 7i64)
                        .with(field::OUTCOME, outcome::REJECTED)
                        .with(field::REASON, "its sensors cannot see the target"),
                );
            }
            200 => frame.events.push(
                Event::new(kind::AI_ACTIVITY)
                    .with_subject(2)
                    .with(field::FROM, "ATTACKING")
                    .with(field::TO, "DEFENDING")
                    .with(field::REASON, "missile inbound, 1.2 nm"),
            ),
            250 => frame.new_effects.push(EffectSpawn {
                kind: EffectKind::Flare,
                position: enemy(250).position,
                duration_ticks: 120,
            }),
            300 => {
                frame.events.push(
                    Event::new(kind::WEAPON_OUTCOME)
                        .with_subject(0)
                        .with_object(2)
                        .with(field::PROJECTILE, Value::Id(1))
                        .with(field::RESULT, outcome::HIT)
                        .with(field::DAMAGE, 800i64)
                        .with(field::HP_AFTER, 0i64),
                );
                frame.events.push(
                    Event::new(kind::COMBAT_HIT)
                        .with_subject(0)
                        .with_object(2)
                        .with(field::PROJECTILE, Value::Id(1))
                        .with(field::WEAPON, Value::Id(1))
                        .with(field::DAMAGE, 800i64)
                        .with(field::HP_AFTER, 0i64),
                );
                frame.events.push(
                    Event::new(kind::COMBAT_DESTROYED)
                        .with_subject(2)
                        .with_object(0)
                        .with(field::WEAPON, Value::Id(1)),
                );
            }
            360 => frame
                .events
                .push(Event::new(kind::PLAYER_BOOKMARK).with_text("odd dip")),
            _ => {}
        }
        if tick.is_multiple_of(30) {
            let p = &frame.aircraft[0];
            frame.trees.push(TreeSample {
                subject: 0,
                channel: channel::FLIGHT_TELEMETRY.into(),
                nodes: vec![
                    Node::new(0, node::TAS, 474.).with_unit(unit::KT),
                    Node::new(0, node::MACH, 0.75),
                    Node::new(0, node::AOA, 6.5).with_unit(unit::DEG),
                    Node::new(0, node::SIDESLIP, 0.25).with_unit(unit::DEG),
                    Node::new(0, node::AGL, p.position[1] - 1_250.).with_unit(unit::FT),
                    Node::new(0, node::LOAD, 2.).with_unit(unit::G),
                    Node::new(1, "Bank hold", 60.)
                        .with_unit(unit::DEG)
                        .with_note("because the pilot holds right stick"),
                ],
            });
        }
        if tick.is_multiple_of(60) && tick < 300 {
            let activity = if tick < 200 { "ATTACKING" } else { "DEFENDING" };
            frame.trees.push(TreeSample {
                subject: 2,
                channel: channel::AI_THOUGHT.into(),
                nodes: vec![
                    Node::new(0, node::ACTIVITY, activity).with_note("since the last change"),
                    Node::new(0, node::TARGET, Value::Id(0)),
                    Node::new(1, "Range", 1. - tick as f64 / 1_200.).with_unit(unit::NM),
                ],
            });
        }
        if tick.is_multiple_of(120) {
            frame.checksum = Some(state_checksum(&frame.aircraft));
        }
        frames.push(frame);
    }
    frames
}

fn write(dir: &Path, name: &str, frames: &[Frame], footer: &Footer) -> PathBuf {
    let mut writer = Writer::create(dir.join(name), &header()).unwrap();
    let (aircraft, weapons) = registry();
    for a in &aircraft {
        writer.register_aircraft(a).unwrap();
    }
    for w in &weapons {
        writer.register_weapon(w).unwrap();
    }
    for frame in frames {
        writer.push(frame).unwrap();
    }
    writer.finish(footer).unwrap()
}

fn golden_footer() -> Footer {
    Footer {
        end_tick: 480,
        result: vec![
            ("outcome".into(), "victory".into()),
            ("kills".into(), "1".into()),
        ],
    }
}

fn golden_recording(dir: &Path) -> Recording {
    Recording::open(write(
        dir,
        "golden.tore-replay",
        &golden_frames(),
        &golden_footer(),
    ))
    .unwrap()
}

/// Compares with the committed golden file; `TORE_UPDATE_GOLDEN=1` rewrites it.
fn check_golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("TORE_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing {}; run with TORE_UPDATE_GOLDEN=1", path.display()))
        .replace('\r', "");
    if expected != actual {
        let line = expected
            .lines()
            .zip(actual.lines())
            .position(|(e, a)| e != a)
            .unwrap_or(expected.lines().count().min(actual.lines().count()));
        panic!(
            "{name} differs from the golden file at line {}:\n expected: {:?}\n actual:   {:?}",
            line + 1,
            expected.lines().nth(line),
            actual.lines().nth(line)
        );
    }
}

#[test]
fn golden_jsonl() {
    let dir = temp_dir("golden-jsonl");
    let recording = golden_recording(&dir);
    let mut out = Vec::new();
    let stats = write_jsonl(&recording, &JsonlOptions::default(), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    for line in text.lines() {
        assert!(
            line.starts_with("{\"type\":\"") && line.ends_with('}'),
            "{line}"
        );
    }
    assert_eq!(stats.samples, 8);
    assert!(stats.anomalies >= 1);
    check_golden("golden.jsonl", &text);

    // A range and id filter keeps only what was asked for.
    let mut out = Vec::new();
    let options = JsonlOptions {
        ids: Some(vec![2]),
        sample_hz: 2.,
        trees: false,
        ..JsonlOptions::default()
    }
    .seconds(Some(1.), Some(2.));
    write_jsonl(&recording, &options, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let samples: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("\"type\":\"sample\""))
        .collect();
    assert_eq!(samples.len(), 3);
    assert!(samples.iter().all(|l| l.contains("\"id\":2")));
    assert!(!text.contains("\"type\":\"tree\""));
    assert!(!text.contains("Fox three"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn golden_summary() {
    let dir = temp_dir("golden-summary");
    let recording = golden_recording(&dir);
    let mut out = Vec::new();
    write_summary(&recording, &SummaryOptions::default(), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    check_golden("golden-summary.txt", &text);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn golden_acmi_and_the_roll_sign() {
    let dir = temp_dir("golden-acmi");
    let recording = golden_recording(&dir);
    let mut out = Vec::new();
    let stats = write_acmi(&recording, &AcmiOptions::default(), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with("FileType=text/acmi/tacview\nFileVersion=2.2\n"));
    assert_eq!(stats.objects, 4, "player, enemy, missile and flare");
    // The player banks right wing down in a right turn: Tacview's roll is
    // positive when rolling to the right, so every sample says +60.
    let player: Vec<Vec<&str>> = text
        .lines()
        .filter(|l| l.starts_with("10000000000,T="))
        .map(|l| {
            l["10000000000,T=".len()..]
                .split(',')
                .next()
                .unwrap()
                .split('|')
                .collect()
        })
        .collect();
    assert_eq!(player[0].len(), 9);
    assert_eq!(player[0][3], "60", "roll in the first sample");
    assert!(
        player[1..].iter().all(|t| t[3].is_empty()),
        "roll never changes"
    );
    let yaws: Vec<f64> = player.iter().filter_map(|t| t[5].parse().ok()).collect();
    assert!(
        yaws.windows(2).all(|w| w[1] > w[0]),
        "heading grows through the turn"
    );
    assert!(*yaws.last().unwrap() > 80.);
    // Ids are hex and never zero; the wreck and the flare are removed.
    assert!(text.lines().any(|l| l == "-10000000002"));
    assert!(text.lines().any(|l| l == "-30000000001"));
    check_golden("golden.txt.acmi", &text);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn anchors_follow_the_header_and_can_be_overridden() {
    let dir = temp_dir("anchors");
    let recording = golden_recording(&dir);
    let mut out = Vec::new();
    let options = AcmiOptions {
        anchor: Some([10., 20.]),
        sample_hz: 1.,
        ..AcmiOptions::default()
    };
    write_acmi(&recording, &options, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("0,ReferenceLongitude=20\n"));
    assert!(text.contains("0,ReferenceLatitude=10\n"));
    // Noon at 20 E is 10:40 UTC.
    assert!(text.contains("0,ReferenceTime=2026-09-26T10:40:00Z\n"));
    let _ = std::fs::remove_dir_all(dir);
}

/// A recording built to trip every anomaly check once.
#[test]
fn anomalies_are_found_with_their_numbers() {
    let dir = temp_dir("anomalies");
    let mut writer = Writer::create(dir.join("odd.tore-replay"), &header()).unwrap();
    let (aircraft, _) = registry();
    for a in &aircraft {
        writer.register_aircraft(a).unwrap();
    }
    for tick in 0..48_000u64 {
        // Straight and level at 800 ft/s, north.
        let mut p = player(0);
        p.position[2] += tick as f64 * 800. / 120.;
        p.velocity = [0., 0., 800.];
        p.attitude = [0., 0., 0.];
        p.g = 1.;
        let mut e = enemy(0);
        e.position[0] -= tick as f64 * 5.;
        e.velocity = [-600., 0., 0.];
        match tick {
            100 => p.position[0] += 500.,
            200 => p.attitude[2] = -1.0,
            300..=302 => p.g = 11.5,
            400 => p.airspeed = f64::NAN,
            _ => {}
        }
        if (500..740).contains(&tick) {
            p.controls[1] = if (tick / 12).is_multiple_of(2) {
                0.9
            } else {
                -0.9
            };
        }
        if tick >= 47_000 {
            e.fuel_lb = 0.;
        }
        let mut frame = Frame {
            tick,
            aircraft: vec![p, e],
            ..Frame::default()
        };
        if tick == 1_000 {
            frame.events.push(
                Event::new(kind::AI_ACTIVITY)
                    .with_subject(2)
                    .with(field::FROM, "FORMATION")
                    .with(field::TO, "PATROL"),
            );
        }
        if (2_000..2_600).contains(&tick) && tick.is_multiple_of(100) {
            frame.events.push(
                Event::new(kind::AI_TARGET)
                    .with_subject(2)
                    .with(field::TO, Value::Id(tick as u32 % 3)),
            );
        }
        match tick {
            3_000 => frame.events.push(
                Event::new(kind::WEAPON_LAUNCH)
                    .with_subject(2)
                    .with_object(0)
                    .with(field::PROJECTILE, Value::Id(1 << 24)),
            ),
            3_060 => frame.events.push(
                Event::new(kind::WEAPON_TRACK_LOST)
                    .with_subject(2)
                    .with_object(0)
                    .with(field::PROJECTILE, Value::Id(1 << 24))
                    .with(field::REASON, "notch"),
            ),
            4_000 => frame.events.push(
                Event::new(kind::COMMS_DELIVERY)
                    .with_subject(1)
                    .with_object(0)
                    .with(field::OUTCOME, outcome::REJECTED)
                    .with(field::REASON, "cannot see the target"),
            ),
            4_100 => frame.events.push(
                Event::new(kind::COMMS_RADIO)
                    .with_subject(2)
                    .with(field::OUTCOME, outcome::DROPPED)
                    .with(field::REASON, "queue full")
                    .with_text("Engaging"),
            ),
            4_200 => frame.events.push(
                Event::new(kind::COMMS_CREW)
                    .with(field::OUTCOME, outcome::SUPPRESSED)
                    .with(field::REASON, "6 s cooldown")
                    .with_text("Missile warning"),
            ),
            4_300 => frame.events.push(
                Event::new(kind::COMMS_RADIO)
                    .with_subject(0)
                    .with(field::OUTCOME, outcome::DELIVERED)
                    .with(field::WAIT_S, 4.5)
                    .with_text("Tally"),
            ),
            4_400 | 4_500 | 4_600 => frame.events.push(
                Event::new(kind::COMMS_RADIO)
                    .with_subject(2)
                    .with(field::OUTCOME, outcome::DELIVERED)
                    .with_text("Bandit, bandit"),
            ),
            5_000 => frame
                .events
                .push(Event::new(kind::AIRCRAFT_CRASHED).with_subject(2)),
            6_000 => frame.events.push(
                Event::new(kind::FLIGHT_G_LIMIT)
                    .with_subject(0)
                    .with(field::ASKED, f64::INFINITY),
            ),
            _ => {}
        }
        if tick == 7_000 {
            frame.trees.push(TreeSample {
                subject: 0,
                channel: channel::FLIGHT_TELEMETRY.into(),
                nodes: vec![Node::new(0, node::AGL, -40.).with_unit(unit::FT)],
            });
        }
        writer.push(&frame).unwrap();
    }
    let recording = Recording::open(writer.finish(&Footer::default()).unwrap()).unwrap();
    let anomalies = detect(&recording, &Thresholds::default()).unwrap();
    let find = |kind_name: &str| {
        anomalies
            .iter()
            .filter(|a| a.kind == kind_name)
            .collect::<Vec<_>>()
    };
    use tore_replay::export::anomaly::kinds;
    let teleport = find(kinds::TELEPORT);
    assert_eq!(teleport.len(), 2, "out and back: {teleport:?}");
    assert_eq!(teleport[0].tick, 100);
    assert!(
        teleport[0].detail.contains("moved 500"),
        "{}",
        teleport[0].detail
    );
    let jump = find(kinds::ATTITUDE_JUMP);
    assert!(jump.iter().any(|a| a.tick == 200), "{jump:?}");
    let g = find(kinds::G_EXCESS);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].tick, 300);
    assert!(
        g[0].detail.contains("11.5 G") && g[0].detail.contains("3 ticks"),
        "{}",
        g[0].detail
    );
    assert!(
        find(kinds::NON_FINITE)
            .iter()
            .any(|a| a.tick == 400 && a.detail.contains("airspeed"))
    );
    assert!(find(kinds::NON_FINITE).iter().any(|a| a.tick == 6_000));
    assert!(!find(kinds::CONTROL_OSCILLATION).is_empty());
    let stuck = find(kinds::AI_STUCK);
    assert_eq!(stuck.len(), 1);
    assert_eq!(stuck[0].tick, 1_000);
    assert!(stuck[0].detail.contains("PATROL"));
    assert_eq!(find(kinds::AI_FLIPPING).len(), 1);
    assert_eq!(find(kinds::TRACK_LOST_EARLY)[0].tick, 3_060);
    assert_eq!(find(kinds::ORDER_REJECTED)[0].tick, 4_000);
    assert_eq!(find(kinds::CALL_DROPPED)[0].tick, 4_100);
    assert_eq!(find(kinds::CALL_SUPPRESSED)[0].tick, 4_200);
    assert_eq!(find(kinds::LONG_WAIT)[0].tick, 4_300);
    assert_eq!(find(kinds::REPEATED_CALL)[0].tick, 4_400);
    assert_eq!(find(kinds::CRASH_UNDAMAGED)[0].tick, 5_000);
    assert_eq!(find(kinds::BELOW_TERRAIN)[0].tick, 7_000);
    let fuel = find(kinds::FUEL_EXHAUSTED);
    assert_eq!(fuel.len(), 1);
    assert_eq!(fuel[0].tick, 47_000);
    // Nothing is flagged in a clean flight.
    let clean = golden_recording(&dir);
    let flags = detect(&clean, &Thresholds::default()).unwrap();
    let kinds_found: Vec<&str> = flags.iter().map(|a| a.kind).collect();
    assert_eq!(kinds_found, vec![kinds::ORDER_REJECTED], "{flags:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn comparing_recordings_finds_the_first_divergence() {
    let dir = temp_dir("diff");
    let frames = golden_frames();
    let a = Recording::open(write(&dir, "a.tore-replay", &frames, &golden_footer())).unwrap();
    // B matches A until tick 250, when the enemy is 10 ft further east, and
    // it launches a second missile at tick 400.
    let mut changed = frames.clone();
    for frame in &mut changed[250..] {
        if let Some(enemy) = frame.aircraft.get_mut(1) {
            enemy.position[0] += 10.;
        }
        if frame.checksum.is_some() {
            frame.checksum = Some(state_checksum(&frame.aircraft));
        }
    }
    changed[400].events.push(
        Event::new(kind::WEAPON_LAUNCH)
            .with_subject(0)
            .with(field::PROJECTILE, Value::Id(2))
            .with(field::WEAPON, Value::Id(1)),
    );
    let b = Recording::open(write(&dir, "b.tore-replay", &changed, &golden_footer())).unwrap();
    let same = compare(&a, &a, &CompareOptions::default()).unwrap();
    assert!(same.identical(), "{same:?}");
    let mut out = Vec::new();
    let comparison = write_diff(&a, &b, &CompareOptions::default(), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(!comparison.identical());
    assert_eq!(comparison.last_matching_checksum, Some(240));
    assert_eq!(comparison.first_checksum_mismatch, Some(360));
    let divergence = comparison.divergence.as_ref().unwrap();
    assert_eq!(divergence.tick, 250);
    assert_eq!(divergence.aircraft.len(), 1);
    assert_eq!(divergence.aircraft[0].id, 2);
    assert!(divergence.aircraft[0].detail.starts_with("10 ft apart"));
    let launches = comparison
        .events
        .iter()
        .find(|e| e.category == "launches")
        .unwrap();
    assert_eq!((launches.count_a, launches.count_b), (1, 2));
    assert_eq!(launches.first.as_ref().unwrap().0, 1);
    assert!(
        text.contains("first differs at 0:03.0 (tick 360)"),
        "{text}"
    );
    assert!(
        text.contains("at 0:02.0 (tick 250):\n  Enemy 2-1: 10 ft apart"),
        "{text}"
    );
    check_golden("golden-diff.txt", &text);
    let _ = std::fs::remove_dir_all(dir);
}
