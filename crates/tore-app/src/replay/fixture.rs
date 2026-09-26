//! A synthetic recording for the viewer's tests: four aircraft on smooth
//! paths, one of them appearing late, a missile and a gun round, smoke,
//! contrails and effects, a building hit twice, a gap where the recorder fell
//! behind, and the events the timeline and subtitles read. No retail data.
use std::path::Path;
use tore_replay::{
    self as replay, AircraftFlags, AircraftInfo, AircraftState, EffectKind, EffectSpawn, Event,
    Frame, ProjectileState, PuffKind, PuffSpawn, Recording, Side, WeaponClass, WeaponInfo,
    WriterOptions, vocab,
};

pub const FIRST: u64 = 5;
pub const LAST: u64 = 900;
/// Ticks the recorder skipped, inclusive.
pub const GAP: (u64, u64) = (400, 459);
/// An aircraft that first appears in the middle.
pub const LATE: u32 = 3;
pub const LATE_FROM: u64 = 250;
pub const MISSILE: u32 = 7;
pub const LAUNCH: u64 = 130;
pub const IMPACT: u64 = 330;
pub const GUN_ROUND: u32 = 8;
/// A building hit at the first tick and destroyed at the second.
pub const SURFACE: [(u64, u32); 2] = [(300, 9_001), (320, 9_001)];
pub const KILL: u64 = 700;
pub const BOOKMARK: u64 = 600;
pub const ORDER: u64 = 50;
pub const HEARD: u64 = 140;
pub const UNHEARD: u64 = 150;

/// Where aircraft `id` is at `tick`: a gentle climbing turn.
pub fn position(id: u32, tick: u64) -> [f64; 3] {
    let t = tick as f64 / 120.;
    let a = t * 0.3 + f64::from(id);
    [
        100_000. + f64::from(id) * 3_000. + a.sin() * 4_000.,
        8_000. + f64::from(id) * 500. + t * 20.,
        100_000. + a.cos() * 4_000.,
    ]
}

fn aircraft(id: u32, tick: u64) -> AircraftState {
    let here = position(id, tick);
    let next = position(id, tick + 1);
    let velocity: [f64; 3] = std::array::from_fn(|i| (next[i] - here[i]) * 120.);
    let horizontal = velocity[0].hypot(velocity[2]);
    AircraftState {
        id,
        position: here,
        attitude: [
            velocity[0].atan2(velocity[2]),
            velocity[1].atan2(horizontal),
            0.6 + (tick as f64 * 0.01).sin() * 0.2,
        ],
        velocity,
        airspeed: horizontal,
        // The player pulls hard enough for wing vapor.
        g: if id == 0 { 6.5 } else { 2. },
        devices: [0., 0., 0., 0., 0., 0.3, 0.1, -0.1, 0., horizontal, 0.9],
        heat: 0.9,
        flags: AircraftFlags {
            engine_on: true,
            airborne: true,
            alive: true,
            animated: true,
            ..Default::default()
        },
        fuel_lb: 6_000. - tick as f64 * 0.1,
        controls: [0.2, 0.1, 0., 0.9],
        hp: if id == 2 && tick >= KILL { 0 } else { 100 },
        max_hp: 100,
        ..Default::default()
    }
}

fn projectile(id: u32, owner: u32, weapon: u32, tick: u64, from: u64) -> ProjectileState {
    let start = position(owner, from);
    let age = (tick - from) as f64;
    let at = |age: f64| [start[0] + age * 10., start[1], start[2] + age * 5.];
    ProjectileState {
        id,
        owner,
        weapon,
        target: Some(0),
        position: at(age),
        previous: at(age - 1.),
        direction: [2. / 5f64.sqrt(), 0., 1. / 5f64.sqrt()],
        speed: 1_341.6,
        incoming: true,
        age: (tick - from) as u32,
        ..Default::default()
    }
}

fn frame(tick: u64) -> Frame {
    let mut frame = Frame {
        tick,
        aircraft: [0, 1, 2]
            .into_iter()
            .chain((tick >= LATE_FROM).then_some(LATE))
            .map(|id| aircraft(id, tick))
            .collect(),
        ..Default::default()
    };
    if (LAUNCH..IMPACT).contains(&tick) {
        frame
            .projectiles
            .push(projectile(MISSILE, 1, 1, tick, LAUNCH));
        if tick.is_multiple_of(8) {
            frame.new_puffs.push(PuffSpawn {
                layer: replay::LAYER_SMOKE,
                kind: PuffKind::Missile,
                position: frame.projectiles[0].position,
            });
        }
    }
    if (200..220).contains(&tick) {
        frame
            .projectiles
            .push(projectile(GUN_ROUND, 0, 0, tick, 200));
    }
    if tick.is_multiple_of(12) {
        frame.new_puffs.push(PuffSpawn {
            layer: replay::LAYER_CONTRAILS,
            kind: PuffKind::Contrail,
            position: position(2, tick),
        });
    }
    let effect = |kind, id| EffectSpawn {
        kind,
        position: position(id, tick),
        duration_ticks: if kind == EffectKind::Destroyed {
            240
        } else {
            45
        },
    };
    match tick {
        LAUNCH => frame.new_effects.push(effect(EffectKind::Launch, 1)),
        IMPACT => frame.new_effects.push(effect(EffectKind::Hit, 0)),
        KILL => frame.new_effects.push(effect(EffectKind::Destroyed, 2)),
        _ => {}
    }
    for (when, id) in SURFACE {
        if tick == when {
            let hp = if when == SURFACE[0].0 { 50 } else { 0 };
            frame.surface_hp.push((id, hp));
        }
    }
    let event = |kind: &str| Event::new(kind);
    frame.events = match tick {
        20 => vec![event(vocab::kind::AI_TARGET).with_subject(1).with_object(0)],
        ORDER => vec![
            event(vocab::kind::COMMS_ORDER)
                .with_subject(0)
                .with("order", "engage my target")
                .with_text("Engage my target"),
        ],
        100 => vec![event(vocab::kind::AI_TARGET).with_subject(0).with_object(2)],
        LAUNCH => vec![
            event(vocab::kind::WEAPON_LAUNCH)
                .with_subject(1)
                .with_object(0)
                .with("projectile", replay::Value::Id(MISSILE)),
        ],
        HEARD => vec![
            event(vocab::kind::COMMS_RADIO)
                .with_subject(1)
                .with("speaker", "Enemy 1-1")
                .with("heard", true)
                .with_text("Fox two"),
        ],
        UNHEARD => vec![
            event(vocab::kind::COMMS_RADIO)
                .with_subject(2)
                .with("speaker", "Enemy 1-2")
                .with("heard", false)
                .with_text("Contact"),
        ],
        BOOKMARK => vec![event(vocab::kind::PLAYER_BOOKMARK).with_text("odd")],
        KILL => vec![
            event(vocab::kind::COMBAT_DESTROYED)
                .with_subject(2)
                .with_object(0),
        ],
        _ => Vec::new(),
    };
    frame
}

/// Writes the synthetic recording into `dir` and opens it.
pub fn recording(dir: &Path, name: &str) -> Recording {
    let path = dir.join(format!("{name}.tore-replay"));
    let mut writer = replay::Writer::create_with(
        &path,
        &replay::Header::default(),
        WriterOptions {
            chunk_ticks: 120,
            sync_ticks: 3_600,
        },
    )
    .unwrap();
    let info = |id: u32, pt: &str, name: &str, label: &str, side| AircraftInfo {
        id,
        pt: pt.into(),
        name: name.into(),
        label: label.into(),
        side,
        wing: 1,
        member: id as u16,
        human: id == 0,
        ..Default::default()
    };
    for aircraft in [
        info(0, "F18.PT", "F/A-18D", "You", Side::Friendly),
        info(1, "MIG29.PT", "MiG-29", "Enemy 1-1", Side::Enemy),
        info(2, "SU27.PT", "Su-27", "Enemy 1-2", Side::Enemy),
        info(LATE, "MIG29.PT", "MiG-29", "Enemy 1-3", Side::Enemy),
    ] {
        writer.register_aircraft(&aircraft).unwrap();
    }
    let weapon = |id, name: &str, class| WeaponInfo {
        id,
        source: format!("{name}.JT"),
        shape: None,
        name: name.into(),
        class,
    };
    writer
        .register_weapon(&weapon(0, "GUN", WeaponClass::Gun))
        .unwrap();
    writer
        .register_weapon(&weapon(1, "AA10", WeaponClass::Missile))
        .unwrap();
    for tick in FIRST..=LAST {
        if (GAP.0..=GAP.1).contains(&tick) {
            continue;
        }
        writer.push(&frame(tick)).unwrap();
    }
    let path = writer.finish(&replay::Footer::default()).unwrap();
    let recording = Recording::open(path).unwrap();
    assert!(recording.complete() && recording.problems().is_empty());
    recording
}
