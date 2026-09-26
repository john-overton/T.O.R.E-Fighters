//! A demonstration recording for looking at the viewer by eye before real
//! recordings exist: a minute over the imported Ukraine map with the
//! player's F/A-18D circling, three MiG-29s in trail, a missile shot and a
//! kill, smoke, contrails, effects, radio lines, orders and a bookmark, and
//! display trees for the debug panels. The flight paths, trees and messages
//! are synthetic (made-up values shaped like the real ones); only the world
//! comes from the player's imported media, read at run time. `TORE_REPLAY_DEMO_MINUTES` makes it longer and
//! `TORE_REPLAY_DEMO_AIRCRAFT` (4 to 17) busier, for timing the viewer.
//! Run it by hand:
//!
//! ```sh
//! TORE_DATA_DIR=... TORE_REPLAY_DEMO=/path/demo.tore-replay \
//!     cargo test --locked -p tore-app replay::demo -- --ignored
//! ```
use crate::replay::convert::Presentation;
use tore_formats::aircraft::AircraftId;
use tore_replay::{
    self as replay, AircraftFlags, AircraftInfo, AircraftState, DebrisState, EffectKind,
    EffectSpawn, EscapeeState, Event, Frame, Node, ProjectileState, PuffKind, PuffSpawn, Side,
    TreeSample, Value, WeaponClass, WeaponInfo, vocab,
};

const LAUNCH: u64 = 120 * 12;
const IMPACT: u64 = 120 * 18;
const MISSILE: u32 = 1;
/// The turn the player flies, feet and feet per second.
const RADIUS: f64 = 9_000.;
const SPEED: f64 = 650.;

struct Path {
    centre: [f64; 3],
    radius: f64,
    speed: f64,
    phase: f64,
}

impl Path {
    fn at(&self, tick: u64) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let t = tick as f64 / 120.;
        let angle = self.phase + self.speed * t / self.radius;
        let climb = (t * 0.15).sin() * 400.;
        let position = [
            self.centre[0] + self.radius * angle.cos(),
            self.centre[1] + climb,
            self.centre[2] + self.radius * angle.sin(),
        ];
        // Counter-clockwise seen from above: heading follows the tangent.
        let velocity = [
            -self.speed * angle.sin(),
            (t * 0.15).cos() * 400. * 0.15,
            self.speed * angle.cos(),
        ];
        let bank = (self.speed * self.speed / (32.174 * self.radius)).atan();
        let attitude = [
            velocity[0].atan2(velocity[2]),
            velocity[1].atan2(velocity[0].hypot(velocity[2])),
            -bank,
        ];
        (position, attitude, velocity)
    }
}

fn aircraft(id: u32, path: &Path, tick: u64, alive: bool) -> AircraftState {
    let (position, attitude, velocity) = path.at(tick);
    let g = 1. / (-attitude[2]).cos();
    AircraftState {
        id,
        position,
        attitude,
        velocity,
        airspeed: path.speed,
        g: if id == 0 { g + 3. } else { g },
        devices: [0., 0., 0., 0., 0., 0.4, -0.3, 0.2, 0., path.speed, 0.95],
        heat: 0.95,
        flags: AircraftFlags {
            engine_on: alive,
            afterburner: id == 0,
            airborne: true,
            alive,
            animated: true,
            crashed: !alive,
            ..Default::default()
        },
        wreck_phase: u8::from(!alive),
        fuel_lb: 8_000. - tick as f64 * 0.2,
        controls: [0.5, -0.3, 0., 0.95],
        hp: if alive { 100 } else { 0 },
        max_hp: 100,
        ..Default::default()
    }
}

/// An AI aircraft's thinking, shaped like the real tree.
fn thought(id: u32, tick: u64) -> Vec<Node> {
    use vocab::unit as u;
    let t = tick as f64 / 120.;
    let range = 6.4 - t * 0.02 + f64::from(id) * 0.3;
    let attacking = tick >= 240;
    vec![
        Node::new(0, "Mission", "Free engagement").with_note("stance: engage assigned"),
        Node::new(
            0,
            "Activity",
            if attacking { "ATTACKING" } else { "FORMATION" },
        )
        .with_note(if attacking {
            "leader released the wing after an attack report"
        } else {
            "holding the leader's formation"
        }),
        Node::new(1, "Since", if attacking { t - 2. } else { t }).with_unit(u::S),
        Node::new(0, "Target", Value::Id(0))
            .with_note("priority Assigned, score 18,400 (next: Enemy 2-2, 26,100)"),
        Node::new(1, "Range", range).with_unit(u::NM),
        Node::new(1, "Aspect", 35. + t).with_unit(u::DEG),
        Node::new(1, "Off nose", 12.).with_unit(u::DEG),
        Node::new(1, "Closure", 820.).with_unit(u::KT),
        Node::new(1, "Height", -2_300.).with_unit(u::FT),
        Node::new(0, "Weapon", "AA-10 on station 3"),
        Node::new(1, "Phase", "Tracking"),
        Node::new(1, "Fire?", "not yet")
            .with_note(format!("outside max range ({range:.1} nm > 5.8 nm)")),
        Node::new(0, "Defense", "none").with_note("no missile inbound"),
        Node::new(0, "Motion", "tactics")
            .with_note("choice Pursuit (roll 37 < 50), situation offensive"),
        Node::new(0, "Steering", Value::None),
        Node::new(1, "Heading", 132.).with_unit(u::DEG),
        Node::new(1, "G asked", 6.4).with_unit(u::G),
        Node::new(1, "G delivered", 5.9)
            .with_unit(u::G)
            .with_note("the structural limit"),
        Node::new(0, "Controls", Value::None),
        Node::new(1, "Pitch", 0.62),
        Node::new(1, "Roll", -0.4),
        Node::new(1, "Rudder", 0.),
        Node::new(1, "Throttle", 0.95),
        Node::new(0, "Fuel", 2_140. - t * 3.)
            .with_unit(u::LB)
            .with_note("state normal"),
    ]
}

/// The player's flight-model telemetry, shaped like the real tree.
fn telemetry(tick: u64) -> Vec<Node> {
    use vocab::{node as n, unit as u};
    let t = tick as f64 / 120.;
    vec![
        Node::new(0, "Air", Value::None),
        Node::new(1, n::TAS, 480. + (t * 0.3).sin() * 20.).with_unit(u::KT),
        Node::new(1, n::MACH, 0.78),
        Node::new(1, n::AOA, 12.3).with_unit(u::DEG),
        Node::new(1, n::SIDESLIP, 0.4).with_unit(u::DEG),
        Node::new(1, "Dynamic pressure", 610.)
            .with_unit(u::LB_FT2)
            .with_note("measured, not a cause"),
        Node::new(1, n::AGL, 11_200.).with_unit(u::FT),
        Node::new(0, "Load", Value::None),
        Node::new(1, n::LOAD, 5.4).with_unit(u::G),
        Node::new(1, "G asked", 7.1).with_unit(u::G),
        Node::new(1, n::G_LIMIT, 7.5).with_unit(u::G),
        Node::new(0, "Power", Value::None),
        Node::new(1, "Thrust", 16_200.).with_unit(u::LB),
        Node::new(1, "Altitude lapse", 0.84).with_unit(u::RATIO),
        Node::new(1, "Fuel", 8_120. - t * 4.).with_unit(u::LB),
        Node::new(0, "Drag", 9_300.).with_unit(u::LB),
        Node::new(1, "Stores", 1_100.).with_unit(u::LB),
        Node::new(1, "Pull", 2_600.).with_unit(u::LB),
        Node::new(1, "Slip", 400.).with_unit(u::LB),
        Node::new(0, "Effects applied this tick", Value::None),
        Node::new(1, "Low-speed G ceiling", "limit 6.8 G")
            .with_note("speed near stall speed 142 kt"),
        Node::new(1, "Regional damage", "lift -6%, roll authority x0.88")
            .with_note("left wing 35%"),
        Node::new(1, "Stall scaling", "lift x0.72, controls x0.60")
            .with_note("departure mode Stalled"),
    ]
}

/// The missile's guidance, shaped like the real tree.
fn guidance(tick: u64) -> Vec<Node> {
    use vocab::unit as u;
    let flown = (tick - LAUNCH) as f64 / 120.;
    vec![
        Node::new(0, "Weapon", "AIM-9M"),
        Node::new(0, "Seeker", "tracking")
            .with_note("infrared, the target inside the 25 deg gimbal"),
        Node::new(1, "Target", Value::Id(3)),
        Node::new(1, "Lock quality", 0.86),
        Node::new(0, "Range", 9_000. - flown * 1_400.).with_unit(u::FT),
        Node::new(0, "Closing", 1_150.).with_unit(u::KT),
        Node::new(0, "Steering", "proportional navigation").with_note("navigation gain 4"),
        Node::new(1, "Commanded", 18.2).with_unit(u::G),
        Node::new(1, "Limit", 30.).with_unit(u::G),
        Node::new(0, "Time of flight", flown).with_unit(u::S),
    ]
}

/// Display trees at their rates, and message traffic between the
/// aircraft, for the debug panels.
fn debug_samples(frame: &mut Frame, tick: u64, count: u32) {
    let tree = |subject, channel: &str, nodes| TreeSample {
        subject,
        channel: channel.into(),
        nodes,
    };
    if tick.is_multiple_of(12) {
        for id in 1..count.min(4) {
            frame
                .trees
                .push(tree(id, vocab::channel::AI_THOUGHT, thought(id, tick)));
        }
    }
    if tick.is_multiple_of(4) {
        frame
            .trees
            .push(tree(0, vocab::channel::FLIGHT_TELEMETRY, telemetry(tick)));
    }
    if (LAUNCH..IMPACT).contains(&tick) && tick.is_multiple_of(12) {
        frame.trees.push(tree(
            MISSILE,
            vocab::channel::WEAPON_GUIDANCE,
            guidance(tick),
        ));
    }
    let event = |kind: &str| Event::new(kind);
    let events = &mut frame.events;
    match tick {
        250 => {
            events.push(
                event(vocab::kind::COMMS_DELIVERY)
                    .with_subject(2)
                    .with_object(1)
                    .with("outcome", "applied"),
            );
            events.push(
                event(vocab::kind::COMMS_DELIVERY)
                    .with_subject(3)
                    .with_object(1)
                    .with("outcome", "rejected")
                    .with("reason", "holding formation since a recall"),
            );
        }
        260 => events.push(
            event(vocab::kind::COMMS_REPORT)
                .with_subject(2)
                .with("recipients", vec![1u32])
                .with("about", Value::Id(0))
                .with("kept_s", 2.)
                .with("outcome", "delivered")
                .with_text("attack on Enemy 2-2 by You"),
        ),
        t if t == 120 * 8 => events.push(
            event(vocab::kind::AUDIO_MUSIC)
                .with("from", "cruise")
                .with("to", "air combat")
                .with("reason", "designated enemy inside 40,000 ft"),
        ),
        t if t == 120 * 9 => events.push(
            event(vocab::kind::COMMS_HUD)
                .with("outcome", "delivered")
                .with_text("Radar missile selected"),
        ),
        t if t == LAUNCH - 60 => events.push(
            event(vocab::kind::AUDIO_TONE)
                .with_subject(0)
                .with("tone", "seeker lock")
                .with("on", true)
                .with("reason", "infrared missile selected, bore target tracked"),
        ),
        t if t == 120 * 14 => events.push(
            event(vocab::kind::COMMS_RADIO)
                .with_subject(2)
                .with("speaker", "Enemy 2-2")
                .with("heard", false)
                .with("outcome", "suppressed")
                .with("reason", "same-shooter limit (8 s)")
                .with_text("Fox one"),
        ),
        t if (120 * 20..120 * 21).contains(&t) && t.is_multiple_of(6) => events.push(
            event(vocab::kind::AUDIO_RELEASE)
                .with_subject(0)
                .with("sound", "&GUN.5K"),
        ),
        _ => {}
    }
}

#[test]
#[ignore = "needs imported media; writes TORE_REPLAY_DEMO"]
fn demo() {
    let Some(out) = std::env::var_os("TORE_REPLAY_DEMO") else {
        panic!("set TORE_REPLAY_DEMO to the recording to write");
    };
    let data = crate::assets::data_directory().unwrap();
    let assets = crate::assets::Assets::load(&data).unwrap();
    let resources = &assets.theater_resources;
    let world = crate::terrain::World::for_mission(resources, "UKR", Some(0)).unwrap();
    let centre = crate::terrain::Camera::for_world(&world)
        .position
        .map(f64::from);
    let ground = f64::from(world.height(centre[0] as f32, centre[2] as f32));
    let centre = [centre[0], ground + 9_000., centre[2]];
    let number = |name: &str, default: u64| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let ticks = 120 * 60 * number("TORE_REPLAY_DEMO_MINUTES", 1).clamp(1, 60);
    let count = number("TORE_REPLAY_DEMO_AIRCRAFT", 4).clamp(4, 17) as u32;
    let presentation = Presentation {
        models: vec![AircraftId::Mig29],
        slots: count - 1,
    };
    let header = replay::Header {
        game_version: crate::version::version().into(),
        recorded_at: "2026-09-26T12:00:00Z".into(),
        mission: replay::MissionKind::QuickMission,
        world: world.identity(),
        extra: presentation.extras(),
        ..Default::default()
    };
    let mut writer = replay::Writer::create(std::path::PathBuf::from(out), &header).unwrap();
    let info = |id: u32, pt: &str, name: &str, label: &str, side, wing, member| AircraftInfo {
        id,
        pt: pt.into(),
        name: name.into(),
        label: label.into(),
        side,
        wing,
        member,
        skill: "Veteran".into(),
        human: id == 0,
    };
    let labels: Vec<String> = (4..count).map(|id| format!("Enemy 3-{}", id - 3)).collect();
    for a in [
        info(0, "F18.PT", "F/A-18D", "You", Side::Friendly, 1, 1),
        info(1, "MIG29.PT", "MiG-29", "Enemy 2-1", Side::Enemy, 2, 1),
        info(2, "MIG29.PT", "MiG-29", "Enemy 2-2", Side::Enemy, 2, 2),
        info(3, "MIG29.PT", "MiG-29", "Enemy 2-3", Side::Enemy, 2, 3),
    ]
    .into_iter()
    .chain((4..count).map(|id| {
        info(
            id,
            "MIG29.PT",
            "MiG-29",
            &labels[(id - 4) as usize],
            Side::Enemy,
            3,
            (id - 3) as u16,
        )
    })) {
        writer.register_aircraft(&a).unwrap();
    }
    writer
        .register_weapon(&WeaponInfo {
            id: 0,
            source: "AIM9M.JT".into(),
            shape: Some("AIM9.SH".into()),
            name: "AIM-9M".into(),
            class: WeaponClass::Missile,
        })
        .unwrap();
    let paths: Vec<Path> = (0..count)
        .map(|i| Path {
            centre: if i < 4 {
                centre
            } else {
                [centre[0] + 12_000., centre[1] + 1_500., centre[2] + 8_000.]
            },
            radius: RADIUS + f64::from(i) * 250.,
            speed: SPEED,
            phase: -f64::from(i) * 0.45,
        })
        .collect();
    let mut kill_at = None;
    for tick in 0..=ticks {
        let mut frame = Frame {
            tick,
            ..Default::default()
        };
        for (id, path) in paths.iter().enumerate() {
            let alive = !(id == 3 && tick >= IMPACT);
            let mut state = aircraft(id as u32, path, tick, alive);
            if !alive {
                // The wreck falls from where it was hit.
                let (at, attitude, velocity) = path.at(IMPACT);
                let t = (tick - IMPACT) as f64 / 120.;
                state.position = [
                    at[0] + velocity[0] * t * 0.6,
                    (at[1] - 16. * t * t).max(ground),
                    at[2] + velocity[2] * t * 0.6,
                ];
                if state.position[1] <= ground {
                    state.flags.airborne = false;
                    state.wreck_phase = 2;
                    state.position = [
                        at[0] + velocity[0] * 0.6 * (at[1] - ground).max(0.).sqrt() / 4.,
                        ground,
                        at[2] + velocity[2] * 0.6 * (at[1] - ground).max(0.).sqrt() / 4.,
                    ];
                }
                state.attitude = [attitude[0], -0.3 - t * 0.2, attitude[2] + t * 1.5];
                if tick.is_multiple_of(8) && state.flags.airborne {
                    frame.new_puffs.push(PuffSpawn {
                        layer: replay::LAYER_SMOKE,
                        kind: PuffKind::Aircraft,
                        position: state.position,
                    });
                }
                kill_at.get_or_insert(tick);
            }
            if tick.is_multiple_of(12) && alive {
                frame.new_puffs.push(PuffSpawn {
                    layer: replay::LAYER_CONTRAILS,
                    kind: PuffKind::Contrail,
                    position: state.position,
                });
            }
            frame.aircraft.push(state);
        }
        if (IMPACT..IMPACT + 240).contains(&tick) {
            let (at, attitude, _) = paths[3].at(IMPACT);
            let t = (tick - IMPACT) as f64 / 120.;
            frame.debris.push(DebrisState {
                owner: 3,
                index: 0,
                position: [at[0] + 40. * t, at[1] - 20. * t * t, at[2] - 30. * t],
                attitude: [attitude[0] + t, t * 2., t * 3.],
            });
        }
        if tick >= IMPACT + 600 {
            let (at, _, _) = paths[3].at(IMPACT);
            let t = (tick - IMPACT - 600) as f64 / 120.;
            frame.escapees.push(EscapeeState {
                owner: 3,
                position: [
                    at[0] + 200.,
                    (at[1] - 1_500. - t * 18.).max(ground + 3.),
                    at[2],
                ],
                heading: 0.7,
                phase: 3,
            });
        }
        if (LAUNCH..IMPACT).contains(&tick) {
            let (from, _, _) = paths[0].at(LAUNCH);
            let (to, _, _) = paths[3].at(IMPACT);
            let f = |tick: u64| (tick - LAUNCH) as f64 / (IMPACT - LAUNCH) as f64;
            let lerp = |f: f64| -> [f64; 3] {
                std::array::from_fn(|i| {
                    from[i]
                        + (to[i] - from[i]) * f * f
                        + if i == 1 { f * (1. - f) * 1_500. } else { 0. }
                })
            };
            let (position, previous) = (lerp(f(tick)), lerp(f(tick).max(1. / 720.) - 1. / 720.));
            let d: [f64; 3] = std::array::from_fn(|i| position[i] - previous[i]);
            let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
            frame.projectiles.push(ProjectileState {
                id: MISSILE,
                owner: 0,
                weapon: 0,
                target: Some(3),
                position,
                previous,
                direction: d.map(|v| v / length),
                speed: length * 120.,
                age: (tick - LAUNCH) as u32,
                ..Default::default()
            });
            if tick.is_multiple_of(8) {
                frame.new_puffs.push(PuffSpawn {
                    layer: replay::LAYER_SMOKE,
                    kind: PuffKind::Missile,
                    position,
                });
            }
        }
        let event = |kind: &str| Event::new(kind);
        let effect = |kind, position, duration_ticks| EffectSpawn {
            kind,
            position,
            duration_ticks,
        };
        match tick {
            240 => frame.events.push(
                event(vocab::kind::COMMS_ORDER)
                    .with_subject(1)
                    .with("recipients", vec![2u32, 3])
                    .with("order", "free selection")
                    .with("outcome", "applied")
                    .with("reason", "leader perceived an attack (missile from You)")
                    .with_text("Enemy 2-1 releases the wing"),
            ),
            600 => frame.events.push(
                event(vocab::kind::COMMS_RADIO)
                    .with_subject(1)
                    .with("speaker", "Enemy 2-1")
                    .with("heard", false)
                    .with("outcome", "delivered")
                    .with("reason", "speaker not in your flight")
                    .with_text("Contact, bandit north"),
            ),
            LAUNCH => {
                frame.events.push(
                    event(vocab::kind::WEAPON_LAUNCH)
                        .with_subject(0)
                        .with_object(3)
                        .with("projectile", replay::Value::Id(MISSILE)),
                );
                frame.events.push(
                    event(vocab::kind::COMMS_RADIO)
                        .with_subject(0)
                        .with("speaker", "You")
                        .with("heard", true)
                        .with("outcome", "delivered")
                        .with("trigger", "infrared missile release at Enemy 2-3")
                        .with("wait_s", 0.4)
                        .with_text("Fox two"),
                );
                frame
                    .new_effects
                    .push(effect(EffectKind::Launch, paths[0].at(tick).0, 45));
            }
            IMPACT => {
                frame.events.push(
                    event(vocab::kind::COMBAT_DESTROYED)
                        .with_subject(3)
                        .with_object(0),
                );
                frame.events.push(
                    event(vocab::kind::COMMS_CREW)
                        .with_subject(0)
                        .with("speaker", "WSO")
                        .with_text("Splash one!"),
                );
                frame
                    .new_effects
                    .push(effect(EffectKind::Destroyed, paths[3].at(tick).0, 240));
            }
            t if t == 120 * 25 => frame
                .events
                .push(event(vocab::kind::PLAYER_BOOKMARK).with_text("demo bookmark")),
            t if t == 120 * 30 => frame.events.push(
                event(vocab::kind::COMMS_TOWER)
                    .with("speaker", "Tower")
                    .with("heard", true)
                    .with_text("Cleared to land runway two seven"),
            ),
            _ => {}
        }
        if tick == 100 {
            frame
                .events
                .push(event(vocab::kind::AI_TARGET).with_subject(1).with_object(0));
        }
        debug_samples(&mut frame, tick, count);
        writer.push(&frame).unwrap();
    }
    let path = writer
        .finish(&replay::Footer {
            end_tick: ticks,
            result: vec![("outcome".into(), "success".into())],
        })
        .unwrap();
    println!("Demo recording: {} (kill at {kill_at:?})", path.display());
}
