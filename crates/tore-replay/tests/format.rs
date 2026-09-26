//! Format round trips, error bounds, damage handling and limits, on synthetic
//! recordings only.

mod common;

use common::*;
use std::f64::consts::TAU;
use tore_replay::precision::*;
use tore_replay::vocab::{channel, kind};
use tore_replay::*;

const EPS: f64 = 1e-9;

#[derive(Default, Debug)]
struct Worst {
    position: f64,
    angle: f64,
    velocity: f64,
    speed: f64,
    g: f64,
    unit: f64,
    signed: f64,
    fuel: f64,
    control: f64,
    direction: f64,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).abs()).fold(0., f64::max)
}

fn check_aircraft(truth: &AircraftState, read: &AircraftState, worst: &mut Worst) {
    assert_eq!(truth.id, read.id);
    let e = distance(truth.position, read.position);
    assert!(e <= POSITION_FT / 2. + EPS, "position error {e}");
    worst.position = worst.position.max(e);
    for i in 0..3 {
        let e = angle_error(truth.attitude[i], read.attitude[i]);
        assert!(e <= ANGLE_RAD / 2. + 1e-12, "attitude {i} error {e}");
        worst.angle = worst.angle.max(e);
    }
    assert!((0. ..TAU + EPS).contains(&read.attitude[0]), "yaw range");
    let e = distance(truth.velocity, read.velocity);
    assert!(e <= VELOCITY_FPS / 2. + EPS);
    worst.velocity = worst.velocity.max(e);
    let e = (truth.airspeed - read.airspeed).abs();
    assert!(e <= SPEED_FPS / 2. + EPS);
    worst.speed = worst.speed.max(e);
    let e = (truth.g - read.g).abs();
    assert!(e <= G / 2. + EPS);
    worst.g = worst.g.max(e);
    for slot in 0..DEVICE_COUNT {
        let e = (truth.devices[slot] - read.devices[slot]).abs();
        let (bound, cell) = match slot {
            device::ELEVATOR | device::AILERON | device::RUDDER => (SIGNED / 2., &mut worst.signed),
            device::SPEED => (SPEED_FPS / 2., &mut worst.speed),
            _ => (UNIT / 2., &mut worst.unit),
        };
        assert!(e <= bound + EPS, "device {slot} error {e}");
        *cell = cell.max(e);
    }
    let e = (truth.heat - read.heat).abs();
    assert!(e <= UNIT / 2. + EPS);
    let e = (truth.fuel_lb - read.fuel_lb).abs();
    assert!(e <= FUEL_LB / 2. + EPS);
    worst.fuel = worst.fuel.max(e);
    for i in 0..4 {
        let e = (truth.controls[i] - read.controls[i]).abs();
        assert!(e <= CONTROL / 2. + EPS);
        worst.control = worst.control.max(e);
    }
    assert_eq!(truth.flags, read.flags);
    assert_eq!(truth.wreck_phase, read.wreck_phase);
    assert_eq!(
        (
            truth.hp,
            truth.max_hp,
            truth.sections,
            truth.structural_section
        ),
        (read.hp, read.max_hp, read.sections, read.structural_section)
    );
}

fn check_projectile(truth: &ProjectileState, read: &ProjectileState, worst: &mut Worst) {
    assert_eq!(
        (
            truth.id,
            truth.owner,
            truth.weapon,
            truth.target,
            truth.tracer,
            truth.incoming,
            truth.age,
            truth.seeker
        ),
        (
            read.id,
            read.owner,
            read.weapon,
            read.target,
            read.tracer,
            read.incoming,
            read.age,
            read.seeker
        )
    );
    let e = distance(truth.position, read.position).max(distance(truth.previous, read.previous));
    assert!(e <= POSITION_FT / 2. + EPS, "projectile position error {e}");
    worst.position = worst.position.max(e);
    let e = distance(truth.direction, read.direction);
    assert!(e <= ANGLE_RAD, "direction error {e}");
    worst.direction = worst.direction.max(e);
    assert!((truth.speed - read.speed).abs() <= SPEED_FPS / 2. + EPS);
}

/// Compares a decoded frame with the frame that was written. `key` frames
/// (the first of each chunk) must match exactly.
fn check_frame(truth: &Frame, read: &Frame, key: bool, worst: &mut Worst) {
    assert_eq!(truth.tick, read.tick);
    assert_eq!(truth.aircraft.len(), read.aircraft.len());
    assert_eq!(truth.projectiles.len(), read.projectiles.len());
    if key {
        assert_eq!(
            truth.aircraft, read.aircraft,
            "keyframe aircraft at {}",
            truth.tick
        );
        assert_eq!(truth.projectiles, read.projectiles);
        assert_eq!(truth.debris, read.debris);
        assert_eq!(truth.escapees, read.escapees);
    }
    for (t, r) in truth.aircraft.iter().zip(&read.aircraft) {
        check_aircraft(t, r, worst);
    }
    for (t, r) in truth.projectiles.iter().zip(&read.projectiles) {
        check_projectile(t, r, worst);
    }
    assert_eq!(truth.debris.len(), read.debris.len());
    for (t, r) in truth.debris.iter().zip(&read.debris) {
        assert_eq!((t.owner, t.index), (r.owner, r.index));
        assert!(distance(t.position, r.position) <= POSITION_FT / 2. + EPS);
        for i in 0..3 {
            assert!(angle_error(t.attitude[i], r.attitude[i]) <= ANGLE_RAD / 2. + 1e-12);
        }
    }
    assert_eq!(truth.escapees.len(), read.escapees.len());
    for (t, r) in truth.escapees.iter().zip(&read.escapees) {
        assert_eq!((t.owner, t.phase), (r.owner, r.phase));
        assert!(distance(t.position, r.position) <= POSITION_FT / 2. + EPS);
        assert!(angle_error(t.heading, r.heading) <= ANGLE_RAD / 2. + 1e-12);
    }
    assert_eq!(truth.new_effects.len(), read.new_effects.len());
    for (t, r) in truth.new_effects.iter().zip(&read.new_effects) {
        assert_eq!((t.kind, t.duration_ticks), (r.kind, r.duration_ticks));
        assert!(distance(t.position, r.position) <= POSITION_FT / 2. + EPS);
    }
    assert_eq!(truth.new_puffs.len(), read.new_puffs.len());
    for (t, r) in truth.new_puffs.iter().zip(&read.new_puffs) {
        assert_eq!((t.kind, t.layer), (r.kind, r.layer));
        assert!(distance(t.position, r.position) <= POSITION_FT / 2. + EPS);
    }
    assert_eq!(truth.surface_hp, read.surface_hp);
    assert_eq!(truth.events, read.events);
    assert_eq!(truth.trees, read.trees);
    assert_eq!(truth.checksum, read.checksum);
}

#[test]
fn round_trip_is_exact_where_exact_and_bounded_where_quantized() {
    let dir = temp_dir("round-trip");
    let scenario = rich_scenario(1, 3_600, 11);
    let path = write(
        &dir,
        "trip.tore-replay",
        &scenario,
        WriterOptions::default(),
    );
    let recording = Recording::open(&path).unwrap();
    assert!(recording.complete());
    assert!(
        recording.problems().is_empty(),
        "{:?}",
        recording.problems()
    );
    assert_eq!(recording.header(), &scenario.header);
    assert_eq!(recording.footer(), Some(&scenario.footer));
    assert_eq!(recording.first_tick(), Some(1));
    assert_eq!(recording.last_tick(), Some(3_600));
    assert_eq!(recording.frame_count(), 3_600);
    assert_eq!(recording.chunks().len(), 30);
    assert!(recording.gaps().is_empty());
    assert_eq!(
        recording.aircraft().cloned().collect::<Vec<_>>(),
        scenario.aircraft
    );
    assert_eq!(
        recording.weapons().cloned().collect::<Vec<_>>(),
        scenario.weapons
    );
    let events: Vec<_> = scenario
        .frames
        .iter()
        .flat_map(|f| f.events.iter().map(move |e| (f.tick, e.clone())))
        .collect();
    assert_eq!(
        recording
            .events()
            .iter()
            .map(|e| (e.tick, e.event.clone()))
            .collect::<Vec<_>>(),
        events
    );
    let checksums: Vec<_> = scenario
        .frames
        .iter()
        .filter_map(|f| f.checksum.map(|c| (f.tick, c)))
        .collect();
    assert_eq!(recording.checksums(), checksums.as_slice());
    let mut worst = Worst::default();
    let mut read = 0;
    for (truth, frame) in scenario.frames.iter().zip(recording.frames(0, u64::MAX)) {
        let frame = frame.unwrap();
        let key = recording
            .chunks()
            .iter()
            .any(|c| c.first_tick == frame.tick);
        check_frame(truth, &frame, key, &mut worst);
        read += 1;
    }
    assert_eq!(read, scenario.frames.len());
    // Random access agrees with forward iteration.
    for tick in [1, 2, 119, 120, 121, 1_777, 3_600] {
        let frame = recording.frame(tick).unwrap().unwrap();
        check_frame(
            &scenario.frames[tick as usize - 1],
            &frame,
            tick % 120 == 1,
            &mut worst,
        );
    }
    assert!(recording.frame(0).unwrap().is_none());
    assert!(recording.frame(3_601).unwrap().is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn ten_minutes_of_smooth_and_violent_flight_do_not_drift() {
    let mut rng = Rng::new(3);
    let mut smooth = Flier::new(0, [100_000., 30_000., 100_000.], 0., false);
    let mut violent = Flier::new(1, [120_000., 15_000., 90_000.], 3.0, true);
    let dir = temp_dir("drift");
    let path = dir.join("drift.tore-replay");
    let mut writer = Writer::create(&path, &header("UKR")).unwrap();
    let mut truth = Vec::new();
    for tick in 0..72_000u64 {
        let frame = Frame {
            tick,
            aircraft: vec![smooth.step(tick, &mut rng), violent.step(tick, &mut rng)],
            ..Frame::default()
        };
        writer.push(&frame).unwrap();
        truth.push(frame);
    }
    let path = writer.finish(&Footer::default()).unwrap();
    let recording = Recording::open(&path).unwrap();
    let mut worst = Worst::default();
    for (t, frame) in truth.iter().zip(recording.frames(0, u64::MAX)) {
        let frame = frame.unwrap();
        let key = frame.tick.is_multiple_of(120);
        check_frame(t, &frame, key, &mut worst);
    }
    // The violent flier really did roll through inverted and turn through north.
    assert!(truth.iter().any(|f| f.aircraft[1].attitude[2].abs() > 3.));
    println!("worst errors over 72,000 ticks: {worst:?}");
    let _ = std::fs::remove_dir_all(dir);
}

fn small_file(dir: &std::path::Path, finish: bool) -> (Vec<u8>, Scenario) {
    let scenario = rich_scenario(1_000, 40, 5);
    let path = dir.join("small.tore-replay");
    let options = WriterOptions {
        chunk_ticks: 8,
        ..WriterOptions::default()
    };
    let mut writer = Writer::create_with(&path, &scenario.header, options).unwrap();
    for info in &scenario.aircraft {
        writer.register_aircraft(info).unwrap();
    }
    for frame in &scenario.frames {
        writer.push(frame).unwrap();
    }
    let path = if finish {
        writer.finish(&scenario.footer).unwrap()
    } else {
        let partial = writer.partial_path().to_path_buf();
        drop(writer);
        partial
    };
    (std::fs::read(path).unwrap(), scenario)
}

/// Chunk boundaries from the documented layout: 12-byte prelude, then
/// 32-byte chunk headers with the body length at byte 8.
fn chunk_offsets(bytes: &[u8]) -> Vec<(usize, u8, usize)> {
    let mut out = Vec::new();
    let mut pos = 12;
    while pos + 32 <= bytes.len() && &bytes[pos..pos + 4] == b"TORC" {
        let len = u32::from_le_bytes(bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
        out.push((pos, bytes[pos + 4], len));
        pos += 32 + len;
    }
    out
}

#[test]
fn truncation_at_every_byte_keeps_whole_chunks_and_never_panics() {
    let dir = temp_dir("truncate");
    let (bytes, scenario) = small_file(&dir, true);
    let chunks = chunk_offsets(&bytes);
    let data: Vec<_> = chunks.iter().filter(|c| c.1 == 2).collect();
    assert_eq!(data.len(), 5);
    let header_end = chunks[0].0 + 32 + chunks[0].2;
    for n in 0..bytes.len() {
        let result = Recording::from_bytes(bytes[..n].to_vec());
        if n < header_end {
            assert!(result.is_err(), "a cut header must not open ({n})");
            continue;
        }
        let recording = result.unwrap();
        assert!(!recording.complete(), "cut at {n} must read as incomplete");
        let whole = data.iter().filter(|c| c.0 + 32 + c.2 <= n).count();
        assert_eq!(recording.chunks().len(), whole, "cut at {n}");
        assert_eq!(recording.frame_count(), whole as u64 * 8);
        for (i, frame) in recording.frames(0, u64::MAX).enumerate() {
            assert_eq!(frame.unwrap().tick, scenario.frames[i].tick);
        }
    }
    let full = Recording::from_bytes(bytes).unwrap();
    assert!(full.complete());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_partial_file_keeps_every_frame_pushed() {
    let dir = temp_dir("partial");
    let (bytes, scenario) = small_file(&dir, false);
    let recording = Recording::from_bytes(bytes).unwrap();
    assert!(!recording.complete());
    assert!(
        recording.problems().is_empty(),
        "{:?}",
        recording.problems()
    );
    assert!(recording.footer().is_none());
    assert_eq!(recording.frame_count(), scenario.frames.len() as u64);
    assert_eq!(recording.aircraft().count(), 4);
    let _ = std::fs::remove_dir_all(dir);
}

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Rewrites a chunk's length and checksum after its body changed.
fn reseal(bytes: &mut [u8], pos: usize, len: usize) {
    bytes[pos + 8..pos + 12].copy_from_slice(&(len as u32).to_le_bytes());
    let hash = fnv1a(
        fnv1a(0xcbf2_9ce4_8422_2325, &bytes[pos..pos + 24]),
        &bytes[pos + 32..pos + 32 + len],
    );
    bytes[pos + 24..pos + 32].copy_from_slice(&hash.to_le_bytes());
}

#[test]
fn a_damaged_chunk_is_skipped_and_reported() {
    let dir = temp_dir("damage");
    for finish in [true, false] {
        let (mut bytes, scenario) = small_file(&dir, finish);
        let chunks = chunk_offsets(&bytes);
        let (pos, _, len) = *chunks.iter().filter(|c| c.1 == 2).nth(2).unwrap();
        bytes[pos + 32 + len / 2] ^= 0x40;
        let recording = Recording::from_bytes(bytes).unwrap();
        assert_eq!(
            recording.problems().len(),
            if finish { 3 } else { 1 },
            "{:?}",
            recording.problems()
        );
        assert!(recording.problems()[0].contains("checksum"));
        assert_eq!(recording.chunks().len(), 4);
        assert_eq!(recording.gaps(), vec![(1_016, 1_023)]);
        let ticks: Vec<u64> = recording
            .frames(0, u64::MAX)
            .map(|f| f.unwrap().tick)
            .collect();
        let expected: Vec<u64> = scenario
            .frames
            .iter()
            .map(|f| f.tick)
            .filter(|t| !(1_016..1_024).contains(t))
            .collect();
        assert_eq!(ticks, expected);
        assert_eq!(recording.complete(), finish);
        std::fs::remove_file(dir.join("small.tore-replay")).ok();
        std::fs::remove_file(dir.join("small.tore-replay.partial")).ok();
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_damaged_chunk_header_stops_a_file_without_an_index() {
    let dir = temp_dir("damage-header");
    let (mut bytes, _) = small_file(&dir, false);
    let chunks = chunk_offsets(&bytes);
    let (pos, _, _) = chunks[3];
    bytes[pos] = b'X';
    let recording = Recording::from_bytes(bytes).unwrap();
    assert_eq!(recording.chunks().len(), 2);
    assert_eq!(recording.problems().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unknown_sections_and_chunk_kinds_are_skipped() {
    let dir = temp_dir("unknown");
    let (bytes, scenario) = small_file(&dir, false);
    let chunks = chunk_offsets(&bytes);
    let (pos, _, len) = chunks[1];
    // Append section 42 with three bytes to the first data chunk.
    let mut edited = bytes[..pos + 32 + len].to_vec();
    edited.extend_from_slice(&[42, 3, 9, 9, 9]);
    reseal(&mut edited, pos, len + 5);
    // Insert a whole chunk of an unknown kind after it.
    let mut unknown = b"TORC".to_vec();
    unknown.extend_from_slice(&[77, 0, 0, 0]);
    unknown.extend_from_slice(&[0; 24]);
    unknown.extend_from_slice(b"future");
    reseal(&mut unknown, 0, 6);
    edited.extend_from_slice(&unknown);
    edited.extend_from_slice(&bytes[pos + 32 + len..]);
    let recording = Recording::from_bytes(edited).unwrap();
    assert!(
        recording.problems().is_empty(),
        "{:?}",
        recording.problems()
    );
    assert_eq!(recording.frame_count(), 40);
    let first = recording.frame(1_000).unwrap().unwrap();
    assert_eq!(first.aircraft, scenario.frames[0].aircraft);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn newer_versions_and_foreign_files_are_refused_clearly() {
    let dir = temp_dir("version");
    let (mut bytes, _) = small_file(&dir, true);
    bytes[8] = 2;
    match Recording::from_bytes(bytes.clone()) {
        Err(Error::Unsupported(message)) => assert!(message.contains("newer")),
        other => panic!("expected unsupported, got {other:?}"),
    }
    bytes[..8].copy_from_slice(b"NOTAFILE");
    assert!(matches!(
        Recording::from_bytes(bytes),
        Err(Error::Corrupt(_))
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_partial_file_is_renamed_when_finished() {
    let dir = temp_dir("rename");
    let path = dir.join("2026-09-26_1540_UKR_F18.tore-replay");
    let mut writer = Writer::create(&path, &header("UKR")).unwrap();
    assert_eq!(writer.partial_path(), partial_path(&path));
    assert!(writer.partial_path().exists());
    assert!(!path.exists());
    writer
        .push(&Frame {
            tick: 1,
            ..Frame::default()
        })
        .unwrap();
    let partial = writer.partial_path().to_path_buf();
    let finished = writer.finish(&Footer::default()).unwrap();
    assert_eq!(finished, path);
    assert!(path.exists());
    assert!(!partial.exists());
    assert!(
        Writer::create(&path, &header("UKR")).is_err(),
        "never overwrite"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn limits_are_refused_and_the_writer_keeps_going() {
    let dir = temp_dir("limits");
    let path = dir.join("limits.tore-replay");
    let mut writer = Writer::create(&path, &header("UKR")).unwrap();
    let plane = |id| AircraftState {
        id,
        ..AircraftState::default()
    };
    writer
        .push(&Frame {
            tick: 10,
            aircraft: vec![plane(0)],
            ..Frame::default()
        })
        .unwrap();
    let too_many = Frame {
        tick: 11,
        aircraft: (0..65).map(plane).collect(),
        ..Frame::default()
    };
    let bad_frames = [
        too_many,
        Frame {
            tick: 10,
            ..Frame::default()
        },
        Frame {
            tick: 11,
            aircraft: vec![plane(3), plane(3)],
            ..Frame::default()
        },
        Frame {
            tick: 11,
            events: vec![Event::new(kind::SYSTEM_NOTE).with_text("x".repeat(1_025))],
            ..Frame::default()
        },
        Frame {
            tick: 11,
            events: vec![Event::new("")],
            ..Frame::default()
        },
        Frame {
            tick: 11,
            trees: vec![TreeSample {
                subject: 0,
                channel: channel::AI_THOUGHT.into(),
                nodes: vec![Node::new(32, "too deep", 1.)],
            }],
            ..Frame::default()
        },
        Frame {
            tick: 11,
            trees: vec![TreeSample {
                subject: 0,
                channel: channel::AI_THOUGHT.into(),
                nodes: vec![Node::default(); 4_097],
            }],
            ..Frame::default()
        },
        Frame {
            tick: 11,
            new_puffs: vec![
                PuffSpawn {
                    layer: 0,
                    kind: PuffKind::Missile,
                    position: [0.; 3],
                };
                4_097
            ],
            ..Frame::default()
        },
        Frame {
            tick: 11,
            new_effects: vec![EffectSpawn {
                kind: EffectKind::Other(3),
                position: [0.; 3],
                duration_ticks: 1,
            }],
            ..Frame::default()
        },
        Frame {
            tick: 11,
            events: vec![
                Event::new(kind::COMMS_ORDER).with("recipients", Value::Ids(vec![1; 1_025])),
            ],
            ..Frame::default()
        },
    ];
    for frame in &bad_frames {
        match writer.push(frame) {
            Err(Error::Invalid(message)) => assert!(!message.is_empty()),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
    let info = aircraft_info(0, Side::Friendly, "You");
    writer.register_aircraft(&info).unwrap();
    writer.register_aircraft(&info).unwrap();
    let mut other = info.clone();
    other.label = "Someone else".into();
    assert!(writer.register_aircraft(&other).is_err());
    writer
        .push(&Frame {
            tick: 11,
            aircraft: vec![plane(0)],
            ..Frame::default()
        })
        .unwrap();
    let path = writer.finish(&Footer::default()).unwrap();
    let recording = Recording::open(&path).unwrap();
    assert_eq!(recording.frame_count(), 2);
    assert_eq!(recording.aircraft_info(0), Some(&info));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_jump_in_ticks_reads_back_as_a_gap() {
    let dir = temp_dir("gap");
    let path = dir.join("gap.tore-replay");
    let mut writer = Writer::create(&path, &header("UKR")).unwrap();
    let mut flier = Flier::new(0, [0., 1_000., 0.], 0., false);
    let mut rng = Rng::new(1);
    for tick in (1..=100).chain(200..=300) {
        writer
            .push(&Frame {
                tick,
                aircraft: vec![flier.step(tick, &mut rng)],
                ..Frame::default()
            })
            .unwrap();
    }
    let recording = Recording::open(writer.finish(&Footer::default()).unwrap()).unwrap();
    assert_eq!(recording.gaps(), vec![(101, 199)]);
    assert!(recording.frame(150).unwrap().is_none());
    assert_eq!(recording.frame(200).unwrap().unwrap().tick, 200);
    assert_eq!(recording.frames(90, 210).count(), 11 + 11);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn non_finite_and_extreme_values_are_kept_bit_for_bit() {
    let dir = temp_dir("nan");
    let path = dir.join("nan.tore-replay");
    let mut writer = Writer::create(&path, &header("UKR")).unwrap();
    let mut flier = Flier::new(0, [0., 1_000., 0.], 0., false);
    let mut rng = Rng::new(9);
    let mut truth = Vec::new();
    for tick in 0..30 {
        let mut state = flier.step(tick, &mut rng);
        if (10..13).contains(&tick) {
            state.position[1] = f64::from_bits(0x7ff8_0000_0000_1234);
            state.g = f64::INFINITY;
            state.devices[device::GEAR] = f64::NAN;
        }
        if tick == 20 {
            state.position[0] = 1e300;
            state.velocity[2] = -0.;
        }
        let frame = Frame {
            tick,
            aircraft: vec![state],
            ..Frame::default()
        };
        writer.push(&frame).unwrap();
        truth.push(frame);
    }
    let recording = Recording::open(writer.finish(&Footer::default()).unwrap()).unwrap();
    for (t, frame) in truth.iter().zip(recording.frames(0, u64::MAX)) {
        let frame = frame.unwrap();
        // Non-finite or out-of-range values force exact key records, and so
        // does the tick after them, whose prediction base was not usable.
        let keyed = (10..=13).contains(&frame.tick) || (20..=21).contains(&frame.tick);
        let r = &frame.aircraft[0];
        let t = &t.aircraft[0];
        if keyed {
            assert_eq!(t.position[1].to_bits(), r.position[1].to_bits());
            assert_eq!(t.position[0].to_bits(), r.position[0].to_bits());
            assert_eq!(t.g.to_bits(), r.g.to_bits());
            assert_eq!(t.velocity[2].to_bits(), r.velocity[2].to_bits());
        } else {
            assert!(distance(t.position, r.position) <= POSITION_FT / 2. + EPS);
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tree_lookup_finds_the_latest_sample_at_or_before_a_tick() {
    let dir = temp_dir("trees");
    let scenario = rich_scenario(1, 600, 2);
    let path = write(
        &dir,
        "trees.tore-replay",
        &scenario,
        WriterOptions::default(),
    );
    let recording = Recording::open(&path).unwrap();
    for tick in [1, 4, 5, 120, 121, 122, 300, 599, 600, 5_000] {
        let expected = scenario
            .frames
            .iter()
            .filter(|f| f.tick <= tick)
            .flat_map(|f| {
                f.trees
                    .iter()
                    .filter(|t| t.subject == 0 && t.channel == channel::FLIGHT_TELEMETRY)
                    .map(move |t| (f.tick, t.clone()))
            })
            .next_back();
        assert_eq!(
            recording.tree(0, channel::FLIGHT_TELEMETRY, tick).unwrap(),
            expected,
            "tick {tick}"
        );
    }
    assert!(
        recording
            .tree(0, channel::AI_THOUGHT, 600)
            .unwrap()
            .is_none()
    );
    assert!(
        recording
            .tree(9, channel::AI_THOUGHT, 600)
            .unwrap()
            .is_none()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn smoke_rebuilds_from_release_times() {
    let dir = temp_dir("smoke");
    let scenario = rich_scenario(1, 2_000, 4);
    let path = write(
        &dir,
        "smoke.tore-replay",
        &scenario,
        WriterOptions::default(),
    );
    let recording = Recording::open(&path).unwrap();
    let tick = 1_500;
    let live = recording.live_puffs(tick).unwrap();
    let expected: Vec<_> = scenario
        .frames
        .iter()
        .filter(|f| f.tick <= tick)
        .flat_map(|f| f.new_puffs.iter().map(move |p| (f.tick, p)))
        .filter(|(t, p)| tick - t < p.kind.lifetime_ticks().unwrap())
        .collect();
    assert_eq!(live.len(), expected.len());
    for (puff, (t, truth)) in live
        .iter()
        .filter(|p| p.layer == LAYER_SMOKE)
        .zip(expected.iter().filter(|(_, p)| p.layer == LAYER_SMOKE))
    {
        assert_eq!(puff.spawn_tick, *t);
        let risen = truth.position[1] + 2. * (tick - t) as f64 / 120.;
        assert!((puff.position[1] - risen).abs() <= POSITION_FT / 2. + EPS);
    }
    let effects = recording.live_effects(1_290, 600).unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].kind, EffectKind::Launch);
    let _ = std::fs::remove_dir_all(dir);
}

/// Measures bytes per aircraft-tick on a realistic synthetic mission: 17
/// aircraft, contrails, AI trees at 10 per second, player telemetry at 30 per
/// second, one missile at a time and gun bursts. Printed for the report and
/// bounded so a regression shows up.
#[test]
fn size_per_aircraft_tick_on_a_realistic_mission() {
    let dir = temp_dir("size");
    let ticks = 7_200u64;
    let mut rng = Rng::new(17);
    let mut fliers: Vec<Flier> = (0..17)
        .map(|i| {
            Flier::new(
                i,
                [
                    700_000. + f64::from(i) * 3_000.,
                    15_000. + f64::from(i) * 700.,
                    700_000.,
                ],
                f64::from(i) * 0.37,
                i % 5 == 4,
            )
        })
        .collect();
    let path = dir.join("size.tore-replay");
    let mut writer = Writer::create(&path, &header("UKR")).unwrap();
    for i in 0..17 {
        writer
            .register_aircraft(&aircraft_info(
                i,
                if i < 8 { Side::Friendly } else { Side::Enemy },
                "x",
            ))
            .unwrap();
    }
    for tick in 0..ticks {
        let aircraft: Vec<_> = fliers.iter_mut().map(|f| f.step(tick, &mut rng)).collect();
        let mut frame = Frame {
            tick,
            aircraft,
            ..Frame::default()
        };
        if tick.is_multiple_of(12) {
            for a in frame.aircraft.clone() {
                frame.new_puffs.push(PuffSpawn {
                    layer: LAYER_CONTRAILS,
                    kind: PuffKind::Contrail,
                    position: a.position,
                });
                if a.id > 0 {
                    frame.trees.push(TreeSample {
                        subject: a.id,
                        channel: channel::AI_THOUGHT.into(),
                        nodes: vec![
                            Node::new(0, "Activity", "ATTACKING"),
                            Node::new(1, "Range", (a.position[0] / 607.6).round() / 10.),
                            Node::new(1, "Closure", (a.airspeed / 1.688).round()),
                            Node::new(0, "Fuel", a.fuel_lb.round()),
                            Node::new(0, "Controls", (a.controls[0] * 100.).round() / 100.),
                        ],
                    });
                }
            }
        }
        if tick.is_multiple_of(4) {
            let p = &frame.aircraft[0];
            frame.trees.push(TreeSample {
                subject: 0,
                channel: channel::FLIGHT_TELEMETRY.into(),
                nodes: vec![
                    Node::new(0, "TAS", (p.airspeed / 1.688 * 10.).round() / 10.),
                    Node::new(0, "G", (p.g * 100.).round() / 100.),
                    Node::new(0, "AoA", 5.),
                    Node::new(0, "Fuel", p.fuel_lb.round()),
                ],
            });
        }
        if tick.is_multiple_of(120) {
            frame.checksum = Some(state_checksum(&frame.aircraft));
        }
        writer.push(&frame).unwrap();
    }
    let path = writer.finish(&Footer::default()).unwrap();
    let total = std::fs::metadata(&path).unwrap().len();

    // The same flight with aircraft only, for the per-aircraft cost alone.
    let mut rng = Rng::new(17);
    let mut fliers2: Vec<Flier> = (0..17)
        .map(|i| {
            Flier::new(
                i,
                [
                    700_000. + f64::from(i) * 3_000.,
                    15_000. + f64::from(i) * 700.,
                    700_000.,
                ],
                f64::from(i) * 0.37,
                i % 5 == 4,
            )
        })
        .collect();
    let path2 = dir.join("aircraft-only.tore-replay");
    let mut writer = Writer::create(&path2, &header("UKR")).unwrap();
    for tick in 0..ticks {
        let aircraft: Vec<_> = fliers2.iter_mut().map(|f| f.step(tick, &mut rng)).collect();
        writer
            .push(&Frame {
                tick,
                aircraft,
                ..Frame::default()
            })
            .unwrap();
    }
    let path2 = writer.finish(&Footer::default()).unwrap();
    let aircraft_only = std::fs::metadata(&path2).unwrap().len();
    let aircraft_ticks = 17 * ticks;
    let per_tick = total as f64 / aircraft_ticks as f64;
    let per_tick_aircraft = aircraft_only as f64 / aircraft_ticks as f64;
    let ten_minutes_mb = per_tick * 17. * 72_000. / 1e6;
    println!(
        "size: {total} bytes for 17 aircraft x {ticks} ticks = {per_tick:.2} bytes per aircraft-tick with trees and smoke; \
         {per_tick_aircraft:.2} for aircraft alone; about {ten_minutes_mb:.1} MB per 10 minutes"
    );
    assert!(
        per_tick_aircraft < 20.,
        "aircraft cost {per_tick_aircraft:.2} bytes per tick"
    );
    assert!(
        per_tick < 30.,
        "total cost {per_tick:.2} bytes per aircraft-tick"
    );
    let _ = std::fs::remove_dir_all(dir);
}
