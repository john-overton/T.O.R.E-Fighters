//! The air-to-ground vocabulary (slice RA): bomb releases with their packs,
//! duds, cluster bursts, guided weapon modes, the laser and ground
//! designations, and the rocket class. They are events and a weapon class,
//! not a format change: a recording that has only these is still format 2,
//! and one with surface content is still format 3. Synthetic recordings only.

mod common;

use common::*;
use tore_replay::export::{
    AcmiOptions, JsonlOptions, SummaryOptions, write_acmi, write_jsonl, write_summary,
};
use tore_replay::vocab::{self, delivery, designate, field, guidance, guide, kind, laser};
use tore_replay::*;

fn weapons() -> Vec<WeaponInfo> {
    let info = |id, source: &str, name: &str, class| WeaponInfo {
        id,
        source: source.into(),
        shape: None,
        name: name.into(),
        class,
    };
    vec![
        info(0, "MK82AIR.JT", "MK-82AIR7", WeaponClass::Bomb),
        info(1, "LAU61.JT", "LAU-61", WeaponClass::Rocket),
        info(2, "GBU10.JT", "GBU-10E", WeaponClass::Bomb),
        info(3, "CBU87.JT", "CBU-87", WeaponClass::Bomb),
    ]
}

fn aircraft() -> Vec<AircraftInfo> {
    vec![
        aircraft_info(0, Side::Friendly, "You"),
        aircraft_info(1, Side::Friendly, "Friendly 1-2"),
    ]
}

/// The events of a pass: a pack, a salvo, a dud, a cluster burst, a laser
/// bomb that follows a buddy's spot and goes blind, a JDAM, the laser and the
/// designations.
fn events() -> Vec<(u64, Event)> {
    let release = Event::new(kind::WEAPON_RELEASE)
        .with_subject(0)
        .with(field::WEAPON, Value::Id(0))
        .with(field::CLASS, "bomb")
        .with(field::STATION, 1)
        .with(field::COUNT, 3)
        .with(field::PROJECTILE, Value::Id(10))
        .with(field::PROJECTILES, vec![10, 11, 12])
        .with(field::DELIVERY, delivery::CCIP)
        .with(field::LEFT, 0)
        .with_object(0x5000_0001)
        .with(field::RANGE_FT, 18_228.);
    let rocket = Event::new(kind::WEAPON_RELEASE)
        .with_subject(0)
        .with(field::WEAPON, Value::Id(1))
        .with(field::CLASS, "rocket")
        .with(field::STATION, 2)
        .with(field::COUNT, 1)
        .with(field::PROJECTILE, Value::Id(20))
        .with(field::PROJECTILES, vec![20])
        .with(field::DELIVERY, delivery::ROCKET)
        .with(field::SALVO, 2)
        .with(field::LEFT, 17);
    let dud = Event::new(kind::WEAPON_DUD)
        .with_subject(0)
        .with(field::PROJECTILE, Value::Id(11))
        .with(field::WEAPON, Value::Id(0))
        .with(field::ARM_S, 1.)
        .with(field::FLOWN_S, 0.42)
        .with(
            field::REASON,
            "it landed 0.4 s after release, before its 1.00 s fuze armed",
        );
    let cluster = Event::new(kind::WEAPON_CLUSTER)
        .with_subject(0)
        .with(field::PROJECTILE, Value::Id(30))
        .with(field::WEAPON, Value::Id(3))
        .with(field::EXPLOSION, 20);
    let guided = |from: Option<&str>, mode: &str| {
        let mut e = Event::new(kind::WEAPON_GUIDED)
            .with_subject(0)
            .with(field::PROJECTILE, Value::Id(40))
            .with(field::WEAPON, Value::Id(2))
            .with(field::GUIDANCE, guidance::LASER)
            .with(field::MODE, mode)
            .with(field::RELEASED, from.is_none())
            .with(field::SPOT_OWNER, Value::Id(1))
            .with(field::OWN, false);
        if let Some(from) = from {
            e = e.with(field::FROM, from);
        }
        e.with(
            field::REASON,
            if from.is_none() {
                "it follows Friendly 1-2's laser spot (buddy lasing)"
            } else {
                "Friendly 1-2's laser went off"
            },
        )
    };
    let lasing = Event::new(kind::PLAYER_LASER)
        .with_subject(1)
        .with(field::STATE, laser::LASING)
        .with(field::ON, true)
        .with(field::HOW, "by hand")
        .with_object(0x5000_0001);
    let pinned = Event::new(kind::PLAYER_DESIGNATE)
        .with_subject(0)
        .with(field::CHANNEL, designate::PIN)
        .with(field::X_FT, 1.)
        .with(field::Y_FT, 2.)
        .with(field::Z_FT, 3.);
    let designated = Event::new(kind::PLAYER_DESIGNATE)
        .with_subject(0)
        .with_object(0x5000_0001)
        .with(field::CHANNEL, designate::RADAR);
    let outcome = Event::new(kind::WEAPON_OUTCOME)
        .with_subject(0)
        .with(field::PROJECTILE, Value::Id(40))
        .with(field::RESULT, vocab::outcome::HIT)
        .with(field::DAMAGE, 200)
        .with(field::REASON, "it was guided on Friendly 1-2's laser");
    let spot = Event::new(kind::PLAYER_LASER_SPOT)
        .with_subject(1)
        .with_object(0x5000_0001)
        .with(field::X_FT, 10.)
        .with(field::Y_FT, 0.)
        .with(field::Z_FT, 20.);
    let aim = Event::new(kind::WEAPON_AIM)
        .with_subject(0)
        .with(field::PROJECTILE, Value::Id(40))
        .with(field::SPOT_OWNER, Value::Id(1))
        .with(field::X_FT, 10.)
        .with(field::Y_FT, 0.)
        .with(field::Z_FT, 20.);
    let impact = Event::new(kind::WEAPON_IMPACT)
        .with_subject(0)
        .with(field::PROJECTILE, Value::Id(40))
        .with(field::GUIDED, true)
        .with(field::MODE, guide::BLIND)
        .with(field::MISS_FT, 940.5)
        .with(
            field::REASON,
            "it landed 940 ft from the point it was steering for",
        );
    vec![
        (10, designated),
        (11, lasing),
        (12, pinned),
        (100, release),
        (130, rocket),
        (160, dud),
        (200, guided(None, guide::GUIDING)),
        (210, spot),
        (222, aim),
        (240, guided(Some(guide::GUIDING), guide::BLIND)),
        (250, impact),
        (260, cluster),
        (300, outcome),
        // A surface gun's round on the unit names no victim; the unit's
        // wreck names the gun as its killer.
        (
            300,
            Event::new(kind::WEAPON_OUTCOME)
                .with_subject(0x5000_0002)
                .with(field::PROJECTILE, Value::Id(50))
                .with(field::RESULT, vocab::outcome::HIT)
                .with(field::DAMAGE, 100),
        ),
        (
            300,
            Event::new(kind::SURFACE_WRECK)
                .with_subject(0x5000_0001)
                .with_object(0x5000_0002)
                .with(field::REASON, "it was hit by a gun"),
        ),
    ]
}

fn recording(dir: &std::path::Path) -> Recording {
    let header = header("UKR");
    let path = dir.join("ground.tore-replay");
    let mut writer = Writer::create(&path, &header).unwrap();
    assert_eq!(writer.version(), 2, "events need no format change");
    for info in aircraft() {
        writer.register_aircraft(&info).unwrap();
    }
    for info in weapons() {
        writer.register_weapon(&info).unwrap();
    }
    let mut events = events();
    for tick in 0..400u64 {
        let mut frame = Frame {
            tick,
            ..Frame::default()
        };
        while events.first().is_some_and(|(at, _)| *at == tick) {
            frame.events.push(events.remove(0).1);
        }
        writer.push(&frame).unwrap();
    }
    Recording::open(writer.finish(&Footer::default()).unwrap()).unwrap()
}

#[test]
fn the_air_to_ground_events_and_the_rocket_class_round_trip_in_format_2() {
    let dir = temp_dir("ground-events");
    let recording = recording(&dir);
    assert_eq!(recording.header().format_version, 2);
    assert!(
        recording.problems().is_empty(),
        "{:?}",
        recording.problems()
    );
    let classes: Vec<WeaponClass> = (0..4)
        .map(|id| recording.weapon_info(id).unwrap().class)
        .collect();
    assert_eq!(
        classes,
        [
            WeaponClass::Bomb,
            WeaponClass::Rocket,
            WeaponClass::Bomb,
            WeaponClass::Bomb
        ]
    );
    let read: Vec<Event> = recording.events().iter().map(|e| e.event.clone()).collect();
    let expected: Vec<Event> = events().into_iter().map(|(_, e)| e).collect();
    assert_eq!(read, expected);
    // The pack keeps its order: a round's place in the list is its index.
    let pack = read
        .iter()
        .find(|e| e.kind == kind::WEAPON_RELEASE)
        .unwrap();
    assert_eq!(
        pack.get(field::PROJECTILES).and_then(Value::as_ids),
        Some([10u32, 11, 12].as_slice())
    );
    for event in &read {
        assert!(
            vocab::kind::ALL.contains(&event.kind.as_str()),
            "{}",
            event.kind
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_exports_say_what_the_pilot_and_the_weapons_did() {
    let dir = temp_dir("ground-exports");
    let recording = recording(&dir);
    let mut out = Vec::new();
    write_summary(&recording, &SummaryOptions::default(), &mut out).unwrap();
    let summary = String::from_utf8(out).unwrap();
    for line in [
        "You released 3 x MK-82AIR7 from station 1 at surface object 0x50000001 at 3.0 nm (ccip), 0 left",
        "You released LAU-61 from station 2 (rocket, rocket 2 of the salvo), 17 left",
        "shot 11 from You was a dud because it landed 0.4 s after release, before its 1.00 s fuze armed",
        "shot 40 from You left guiding on Friendly 1-2's laser because it follows Friendly 1-2's laser spot (buddy lasing)",
        "shot 40 from You guidance guiding -> blind because Friendly 1-2's laser went off",
        "shot 40 from You landed: it landed 940 ft from the point it was steering for",
        "shot 30 from You burst and scattered its submunitions",
        "shot 40 from You: hit, damage 200 because it was guided on Friendly 1-2's laser",
        // A surface hit is only the shot's outcome and its kill only the
        // unit's wreck: both still count for the shooter.
        "surface object 0x50000002\n  shots 0 | hits 1 | kills 1",
    ] {
        assert!(summary.contains(line), "missing {line:?} in:\n{summary}");
    }
    let mut out = Vec::new();
    write_jsonl(&recording, &JsonlOptions::default(), &mut out).unwrap();
    let log = String::from_utf8(out).unwrap();
    for kind in [
        kind::WEAPON_RELEASE,
        kind::WEAPON_DUD,
        kind::WEAPON_CLUSTER,
        kind::WEAPON_GUIDED,
        kind::PLAYER_LASER,
        kind::PLAYER_DESIGNATE,
        kind::PLAYER_LASER_SPOT,
        kind::WEAPON_AIM,
        kind::WEAPON_IMPACT,
    ] {
        assert!(log.contains(&format!(r#""kind":"{kind}""#)), "{kind}");
    }
    assert!(log.contains(r#""projectiles":[10,11,12]"#));
    let mut out = Vec::new();
    write_acmi(&recording, &AcmiOptions::default(), &mut out).unwrap();
    let acmi = String::from_utf8(out).unwrap();
    assert!(acmi.contains("was a dud"), "Tacview shows the dud");
    let _ = std::fs::remove_dir_all(dir);
}
