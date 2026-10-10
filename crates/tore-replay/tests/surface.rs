//! Format 3: the ground target and airfield scene in the header, surface
//! poses, launcher and magazine changes, debris pieces, flak and the surface
//! registry, on synthetic recordings only; and the guarantee that a world
//! without any of them is still written as the format 2 file it always was.

mod common;

use common::*;
use tore_replay::export::{JsonlOptions, SummaryOptions, write_jsonl, write_summary};
use tore_replay::precision::*;
use tore_replay::vocab::{field, kind};
use tore_replay::*;

fn target() -> GroundTarget {
    GroundTarget {
        stem: "QUCOL".into(),
        aaa: 2,
        sam: 3,
        seed: 41,
        enemy_nationality: 4,
        night_stealth: false,
        jitter: true,
        relocate: true,
        separation_nm: 60,
    }
}

fn surface_header() -> Header {
    let mut header = header("UKR");
    header.world.ground_target = Some(target());
    header
}

fn unit(id: u32, tick: u64) -> SurfaceState {
    // Driving east at 20 ft/s on a gentle curve; unit 1 stands still.
    let t = tick as f64 / 120.;
    if id == 0x5000_0001 {
        return SurfaceState {
            id,
            position: [10_000., 12., 20_000.],
            attitude: [1.0, 0., 0.],
            wrecked: tick >= 300,
        };
    }
    SurfaceState {
        id,
        position: [50_000. + 20. * t, 40., 60_000. + 0.5 * t * t],
        attitude: [1.57 + 0.01 * t, 0.02, -0.01],
        wrecked: false,
    }
}

fn frames() -> Vec<Frame> {
    (0..600)
        .map(|tick| {
            let mut frame = Frame {
                tick,
                aircraft: vec![],
                surface: vec![unit(0x5000_0001, tick), unit(0x5000_0002, tick)],
                ..Frame::default()
            };
            if tick == 100 {
                frame.surface_stock = vec![SurfaceStock {
                    unit: 0x5000_0001,
                    mount: 1,
                    loaded: 2,
                    reserve: None,
                }];
            }
            if tick == 450 {
                frame.surface_stock = vec![
                    SurfaceStock {
                        unit: 0x5000_0001,
                        mount: 1,
                        loaded: 3,
                        reserve: None,
                    },
                    SurfaceStock {
                        unit: 0x5000_0002,
                        mount: 0,
                        loaded: 0,
                        reserve: Some(2),
                    },
                ];
            }
            if (300..360).contains(&tick) {
                frame.debris = vec![DebrisState {
                    owner: 0x5000_0001,
                    index: 0,
                    position: [10_000., 20. + (tick - 300) as f64, 20_000.],
                    attitude: [0.; 3],
                }];
                frame.debris_pieces = vec![(0x5000_0001, 0, 1)];
            }
            if tick == 305 {
                frame.new_effects = vec![
                    EffectSpawn {
                        kind: EffectKind::Flak { explosion: 27 },
                        position: [1., 2., 3.],
                        duration_ticks: 120,
                    },
                    EffectSpawn {
                        kind: EffectKind::Flak { explosion: 28 },
                        position: [4., 5., 6.],
                        duration_ticks: 120,
                    },
                ];
            }
            if tick == 310 {
                frame.surface_hp = vec![(0x5000_0001, 0)];
                frame.events = vec![
                    Event::new(kind::SURFACE_WRECK)
                        .with_subject(0x5000_0001)
                        .with(field::BURNING, true)
                        .with(field::FIRE_FT, 30.),
                ];
            }
            if tick == 520 {
                // Unit 2 leaves the list; the list is edited, not rewritten.
                frame.surface.truncate(1);
            }
            if tick > 520 {
                frame.surface.truncate(1);
            }
            frame
        })
        .collect()
}

fn registry() -> Vec<SurfaceInfo> {
    vec![
        SurfaceInfo {
            id: 0x5000_0001,
            name: "SA-6".into(),
            label: "SA-6 #1".into(),
            side: Side::Enemy,
            hit_points: 100,
            position: [10_000., 12., 20_000.],
        },
        SurfaceInfo {
            id: 0x5000_0002,
            name: "T-80".into(),
            label: "T-80 #2".into(),
            side: Side::Enemy,
            hit_points: 650,
            position: [50_000., 40., 60_000.],
        },
    ]
}

fn write(dir: &std::path::Path, name: &str, header: &Header) -> Recording {
    let path = dir.join(name);
    let mut writer = Writer::create(&path, header).unwrap();
    for info in registry() {
        writer.register_surface_unit(&info).unwrap();
    }
    for frame in frames() {
        writer.push(&frame).unwrap();
    }
    Recording::open(writer.finish(&Footer::default()).unwrap()).unwrap()
}

#[test]
fn the_ground_target_and_airfield_scene_round_trip_in_a_format_3_header() {
    let dir = temp_dir("surface-header");
    let mut header = surface_header();
    header.world.airfield_scene = 7;
    header.world.surface = true;
    let recording = write(&dir, "h.tore-replay", &header);
    let read = recording.header();
    assert_eq!(read.format_version, 3);
    assert_eq!(read.world.ground_target, Some(target()));
    assert_eq!(read.world.airfield_scene, 7);
    assert!(read.world.surface);
    // The airfield scene is left out of the header when it is the retail 0.
    let mut plain = surface_header();
    plain.world.airfield_scene = 0;
    let recording = write(&dir, "p.tore-replay", &plain);
    assert_eq!(recording.header().world.airfield_scene, 0);
    let bytes = std::fs::read(dir.join("p.tore-replay")).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("world.ground_target=QUCOL,2,3,41,4,0,1,1,60"));
    assert!(!text.contains("world.airfield_scene"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn surface_poses_stock_pieces_flak_and_names_round_trip() {
    let dir = temp_dir("surface-tracks");
    let recording = write(&dir, "t.tore-replay", &surface_header());
    assert_eq!(recording.header().format_version, 3);
    let truth = frames();
    for tick in [
        0, 1, 99, 100, 119, 120, 121, 305, 310, 449, 450, 519, 520, 521, 599,
    ] {
        let frame = recording.frame(tick).unwrap().unwrap();
        let wanted = &truth[tick as usize];
        assert_eq!(frame.surface.len(), wanted.surface.len(), "tick {tick}");
        for (read, truth) in frame.surface.iter().zip(&wanted.surface) {
            assert_eq!(read.id, truth.id);
            assert_eq!(read.wrecked, truth.wrecked, "tick {tick}");
            for i in 0..3 {
                assert!(
                    (read.position[i] - truth.position[i]).abs() <= POSITION_FT / 2. + 1e-9,
                    "tick {tick} position {i}: {:?} against {:?}",
                    read.position,
                    truth.position
                );
                let angle = (read.attitude[i] - truth.attitude[i]).abs();
                assert!(angle <= ANGLE_RAD / 2. + 1e-9, "tick {tick} angle {i}");
            }
        }
        assert_eq!(frame.surface_stock, wanted.surface_stock, "tick {tick}");
        assert_eq!(frame.debris_pieces, wanted.debris_pieces, "tick {tick}");
        assert_eq!(frame.surface_hp, wanted.surface_hp, "tick {tick}");
        assert_eq!(frame.new_effects.len(), wanted.new_effects.len());
        for (read, truth) in frame.new_effects.iter().zip(&wanted.new_effects) {
            assert_eq!(read.kind, truth.kind);
        }
    }
    // A chunk decodes on its own: the unit that stands still costs nothing
    // but still reads back exactly, and the wrecked flag flips on its tick.
    let still = |tick: u64| {
        recording
            .frame(tick)
            .unwrap()
            .unwrap()
            .surface
            .into_iter()
            .find(|u| u.id == 0x5000_0001)
            .unwrap()
    };
    assert!(!still(299).wrecked && still(300).wrecked);
    assert_eq!(still(250).position, [10_000., 12., 20_000.]);
    // The registry and its names.
    let info = recording.surface_info(0x5000_0002).unwrap();
    assert_eq!((info.name.as_str(), info.hit_points), ("T-80", 650));
    assert_eq!(recording.surface_units().count(), 2);
    assert!(
        recording.problems().is_empty(),
        "{:?}",
        recording.problems()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_world_without_surface_content_is_still_a_format_2_file() {
    let dir = temp_dir("surface-v2");
    let path = dir.join("v2.tore-replay");
    let mut writer = Writer::create(&path, &header("UKR")).unwrap();
    assert_eq!(writer.version(), 2);
    // Nothing of format 3 goes into it.
    assert!(writer.register_surface_unit(&registry()[0]).is_err());
    let mut frame = frames().remove(0);
    assert!(writer.push(&frame).is_err(), "surface poses need format 3");
    frame.surface.clear();
    frame.new_effects = vec![EffectSpawn {
        kind: EffectKind::Flak { explosion: 27 },
        position: [0.; 3],
        duration_ticks: 1,
    }];
    assert!(writer.push(&frame).is_err(), "flak needs format 3");
    frame.new_effects.clear();
    writer.push(&frame).unwrap();
    let recording = Recording::open(writer.finish(&Footer::default()).unwrap()).unwrap();
    assert_eq!(recording.header().format_version, 2);
    assert_eq!(&std::fs::read(&path).unwrap()[8..10], &2u16.to_le_bytes());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_format_2_reader_path_refuses_format_3_surface_flags_in_a_format_2_file() {
    // A frame whose flags promise surface tracks, in a file that says format
    // 2, is damage, not a track: format 2 had only the hit point flag.
    let dir = temp_dir("surface-flags");
    let recording = write(&dir, "f.tore-replay", &surface_header());
    let path = dir.join("f.tore-replay");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[8..10].copy_from_slice(&2u16.to_le_bytes());
    let relabelled = Recording::from_bytes(bytes).unwrap();
    assert_eq!(relabelled.header().format_version, 2);
    let result = relabelled.decode_chunk(0);
    assert!(result.is_err(), "{result:?}");
    drop(recording);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn flak_has_its_own_effect_codes() {
    for explosion in 15..=38 {
        let kind = EffectKind::Flak { explosion };
        assert_eq!(EffectKind::from_code(kind.code()), kind);
        assert_eq!(kind.explosion(), Some(explosion));
        assert_eq!(kind.name(), "flak");
    }
    // The codes before and after the flak range are the old ones.
    assert_eq!(EffectKind::from_code(31), EffectKind::Other(31));
    assert_eq!(EffectKind::from_code(56), EffectKind::Other(56));
    assert_eq!(EffectKind::Flak { explosion: 27 }.code(), 44);
    assert_eq!(EffectKind::Flak { explosion: 28 }.code(), 45);
}

#[test]
fn the_exports_name_surface_units_and_list_their_poses_and_stock() {
    let dir = temp_dir("surface-exports");
    // The wreck event and a hit by the SA-6 name the unit, not a raw id.
    let path = dir.join("e.tore-replay");
    let mut writer = Writer::create(&path, &surface_header()).unwrap();
    for info in registry() {
        writer.register_surface_unit(&info).unwrap();
    }
    for mut frame in frames() {
        if frame.tick == 320 {
            frame.events.push(
                Event::new(kind::COMBAT_DESTROYED)
                    .with_subject(7)
                    .with_object(0x5000_0001)
                    .with(field::REASON, "it was shot down by an SA-6"),
            );
            frame.events.push(
                Event::new(kind::SURFACE_REARM)
                    .with_subject(0x5000_0001)
                    .with(field::LOADED, 3),
            );
        }
        writer.push(&frame).unwrap();
    }
    let recording = Recording::open(writer.finish(&Footer::default()).unwrap()).unwrap();
    let mut out = Vec::new();
    write_summary(&recording, &SummaryOptions::default(), &mut out).unwrap();
    let summary = String::from_utf8(out).unwrap();
    assert!(summary.contains("Surface units"), "{summary}");
    assert!(
        summary.contains("SA-6 #1 (SA-6; enemy; 100 hp)"),
        "{summary}"
    );
    assert!(summary.contains("rearmed 1 times"), "{summary}");
    assert!(summary.contains("destroyed at 0:02.5"), "{summary}");
    assert!(
        summary.contains("was destroyed by SA-6 #1"),
        "the killer is named: {summary}"
    );
    assert!(
        !summary.contains("surface object"),
        "no raw id is left: {summary}"
    );
    let mut out = Vec::new();
    write_jsonl(&recording, &JsonlOptions::default(), &mut out).unwrap();
    let log = String::from_utf8(out).unwrap();
    assert!(log.contains(r#""ground_target":{"stem":"QUCOL""#));
    assert!(log.contains(r#""type":"surface_unit","id":1342177281,"name":"SA-6""#));
    assert!(log.contains(r#""type":"surface","tick":0"#));
    assert!(log.contains(r#""type":"surface_stock","tick":100"#));
    let _ = std::fs::remove_dir_all(dir);
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

/// The format 2 writer's output for a world with no surface content, hashed
/// when the format 3 change was made on the writer before it: the bytes of a
/// recording that does not use format 3 must not move.
#[test]
fn a_recording_without_surface_content_is_byte_for_byte_what_format_2_wrote() {
    let dir = temp_dir("surface-bytes");
    let scenario = rich_scenario(1_000, 400, 5);
    let path = dir.join("h.tore-replay");
    let options = WriterOptions {
        chunk_ticks: 60,
        ..WriterOptions::default()
    };
    let mut writer = Writer::create_with(&path, &scenario.header, options).unwrap();
    for info in &scenario.aircraft {
        writer.register_aircraft(info).unwrap();
    }
    for weapon in &scenario.weapons {
        writer.register_weapon(weapon).unwrap();
    }
    for frame in &scenario.frames {
        writer.push(frame).unwrap();
    }
    let bytes = std::fs::read(writer.finish(&scenario.footer).unwrap()).unwrap();
    assert_eq!(
        (fnv(&bytes), bytes.len()),
        (643_258_352_877_265_443, 57_176)
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A format 2 file the writer before format 3 made (40 ticks of the rich
/// scenario in 8 tick chunks) still opens, plays and exports.
#[test]
fn a_format_2_file_still_opens_plays_and_exports() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/format2.tore-replay"),
    )
    .unwrap();
    assert_eq!(&bytes[8..10], &2u16.to_le_bytes());
    let recording = Recording::from_bytes(bytes).unwrap();
    assert_eq!(recording.header().format_version, 2);
    assert!(recording.complete() && recording.problems().is_empty());
    assert_eq!(recording.header().world.ground_target, None);
    assert_eq!(recording.header().world.airfield_scene, 0);
    assert_eq!(recording.surface_units().count(), 0);
    let scenario = rich_scenario(1_000, 40, 5);
    assert_eq!(recording.frame_count() as usize, scenario.frames.len());
    for truth in &scenario.frames {
        let frame = recording.frame(truth.tick).unwrap().unwrap();
        assert_eq!(frame.aircraft.len(), truth.aircraft.len());
        assert!(frame.surface.is_empty() && frame.surface_stock.is_empty());
        assert_eq!(frame.surface_hp, truth.surface_hp);
        assert_eq!(frame.events, truth.events);
    }
    let mut out = Vec::new();
    write_summary(&recording, &SummaryOptions::default(), &mut out).unwrap();
    let summary = String::from_utf8(out).unwrap();
    assert!(summary.contains("format 2"), "{summary}");
    assert!(!summary.contains("Surface units"));
}
