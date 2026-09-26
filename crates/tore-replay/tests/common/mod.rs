//! Synthetic fixtures shared by the integration tests. Every value here is
//! invented; no retail data is involved.
#![allow(dead_code)]

use std::f64::consts::{PI, TAU};
use std::path::PathBuf;
use tore_replay::vocab::{channel, field, kind, node, outcome, unit};
use tore_replay::*;

/// A fresh, empty directory for one test.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tore-replay-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Small deterministic generator (xorshift64*).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in -1..1.
    pub fn signed(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 52) as f64 - 1.
    }
}

pub fn header(theater: &str) -> Header {
    Header {
        game_version: "0.1.0".into(),
        game_commit: "test".into(),
        recorded_at: "2026-09-26T15:40:00Z".into(),
        mission: MissionKind::QuickMission,
        world: World {
            theater: theater.into(),
            theater_name: "Ukraine".into(),
            layout: format!("{theater}.MM"),
            weather: Some(1),
            weather_name: "clear".into(),
            weather_seed: Some(7),
            time_of_day_s: 43_200.,
            wind_fps: [10., 0., 0.],
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

pub fn aircraft_info(id: u32, side: Side, label: &str) -> AircraftInfo {
    AircraftInfo {
        id,
        pt: if side == Side::Friendly {
            "F18.PT"
        } else {
            "MIG29.PT"
        }
        .into(),
        name: if side == Side::Friendly {
            "F/A-18D"
        } else {
            "MiG-29"
        }
        .into(),
        label: label.into(),
        side,
        wing: 1 + (id / 4) as u16,
        member: 1 + (id % 4) as u16,
        skill: "Veteran".into(),
        human: id == 0,
    }
}

/// A simple kinematic aircraft: bank drives the turn rate, controls drive
/// bank, pitch and speed. Good enough to exercise every field.
#[derive(Clone)]
pub struct Flier {
    pub id: u32,
    pub position: [f64; 3],
    pub yaw: f64,
    pub pitch: f64,
    pub bank: f64,
    pub speed: f64,
    pub fuel: f64,
    pub violent: bool,
    pub gear: f64,
    pub hp: i32,
}

impl Flier {
    pub fn new(id: u32, position: [f64; 3], yaw: f64, violent: bool) -> Self {
        Self {
            id,
            position,
            yaw,
            pitch: 0.,
            bank: 0.,
            speed: 700.,
            fuel: 9_000.,
            violent,
            gear: 1.,
            hp: 1_000,
        }
    }

    /// Advances one tick and returns the recorded state.
    pub fn step(&mut self, tick: u64, rng: &mut Rng) -> AircraftState {
        let dt = 1. / 120.;
        let t = tick as f64 * dt;
        let (roll_rate, pitch_rate, accel) = if self.violent {
            // Hard, abrupt inputs every fifth of a second: rolls through
            // inverted, pull-ups through the vertical and speed swings.
            let phase = (tick / 24) as f64;
            let sign = if (tick / 150).is_multiple_of(2) {
                1.
            } else {
                -1.
            };
            (
                sign * (4. + 3. * (phase * 1.7).sin()),
                0.9 * (phase * 0.9).sin(),
                60. * (phase * 0.4).cos(),
            )
        } else {
            (
                0.3 * (t * 0.25).sin(),
                0.05 * (t * 0.1).sin(),
                5. * (t * 0.05).cos(),
            )
        };
        self.bank = wrap(self.bank + roll_rate * dt);
        self.pitch = (self.pitch + pitch_rate * dt).clamp(-1.5, 1.5);
        self.speed = (self.speed + accel * dt).clamp(150., 1_800.);
        let turn = 32.174 * self.bank.tan().clamp(-9., 9.) / self.speed;
        self.yaw = (self.yaw + turn * dt).rem_euclid(TAU);
        let forward = forward([self.yaw, self.pitch, self.bank]);
        let velocity = forward.map(|v| v * self.speed);
        for (p, v) in self.position.iter_mut().zip(velocity) {
            *p += v * dt;
        }
        self.fuel -= 0.02 + 0.01 * rng.signed().abs();
        if tick % 900 == 300 {
            self.gear = 1. - self.gear.round();
        }
        let gear = if self.gear > 0.5 {
            (self.gear + 0.004).min(1.)
        } else {
            (self.gear - 0.004).max(0.)
        };
        self.gear = gear;
        if tick % 2_000 == 1_999 {
            self.hp -= 37;
        }
        let g = (1. + (self.speed * turn / 32.174).powi(2)).sqrt();
        let pitch_control = (pitch_rate * 0.8).clamp(-1., 1.);
        let roll_control = (roll_rate * 0.2).clamp(-1., 1.);
        AircraftState {
            id: self.id,
            position: self.position,
            attitude: [self.yaw, self.pitch, self.bank],
            velocity,
            airspeed: self.speed + 3. * (t * 0.3).sin(),
            g,
            devices: [
                gear,
                0.,
                if self.violent {
                    0.5 + 0.5 * (t * 0.7).sin()
                } else {
                    0.
                },
                0.,
                0.,
                0.6 + 0.3 * (t * 0.2).sin(),
                pitch_control * 0.9,
                roll_control * 0.9,
                0.1 * (t * 1.3).sin(),
                self.speed,
                0.8 + 0.15 * (t * 0.05).sin(),
            ],
            heat: 0.7 + 0.2 * (t * 0.05).sin(),
            flags: AircraftFlags {
                engine_on: true,
                afterburner: self.speed < 600.,
                airborne: true,
                alive: true,
                ..AircraftFlags::default()
            },
            wreck_phase: 0,
            fuel_lb: self.fuel,
            controls: [
                pitch_control,
                roll_control,
                0.1 * (t * 1.1).sin(),
                0.8 + 0.15 * (t * 0.05).sin(),
            ],
            hp: self.hp,
            max_hp: 1_000,
            sections: [0, (1_000 - self.hp) / 2, 0, 0, 0, 0],
            structural_section: (self.hp < 900).then_some(2),
        }
    }
}

pub fn wrap(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

/// A recording with every kind of content, `ticks` long, starting at `start`.
pub struct Scenario {
    pub header: Header,
    pub aircraft: Vec<AircraftInfo>,
    pub weapons: Vec<WeaponInfo>,
    pub frames: Vec<Frame>,
    pub footer: Footer,
}

pub fn weapons() -> Vec<WeaponInfo> {
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
    ]
}

/// Aircraft 0 and 1 fly smoothly, 2 and 3 violently; missiles, gun rounds,
/// debris, a parachute, effects, puffs, events, trees and checksums appear.
pub fn rich_scenario(start: u64, ticks: u64, seed: u64) -> Scenario {
    let mut rng = Rng::new(seed);
    let mut fliers = vec![
        Flier::new(0, [800_000., 20_000., 800_000.], 0.3, false),
        Flier::new(1, [820_000., 21_000., 790_000.], 6.2, false),
        Flier::new(2, [900_000., 18_000., 810_000.], 3.1, true),
        Flier::new(3, [910_000., 25_000., 830_000.], 1.0, true),
    ];
    let infos = vec![
        aircraft_info(0, Side::Friendly, "You"),
        aircraft_info(1, Side::Friendly, "Friendly 1-2"),
        aircraft_info(2, Side::Enemy, "Enemy 2-1"),
        aircraft_info(3, Side::Enemy, "Enemy 2-2"),
    ];
    let mut frames = Vec::new();
    let mut missile: Option<ProjectileState> = None;
    let mut rounds: Vec<ProjectileState> = Vec::new();
    let mut next_round = 100u32;
    let mut previous_telemetry = 0.0f64;
    for tick in start..start + ticks {
        let local = tick - start;
        let mut frame = Frame {
            tick,
            ..Frame::default()
        };
        for flier in &mut fliers {
            frame.aircraft.push(flier.step(tick, &mut rng));
        }
        // A missile flies from aircraft 0 for three seconds every ten.
        if local % 1_200 == 60 {
            let shooter = &frame.aircraft[0];
            let direction = shooter.forward();
            missile = Some(ProjectileState {
                id: 1 << 24 | (local / 1_200) as u32,
                owner: 0,
                weapon: 1,
                target: Some(2),
                position: shooter.position,
                previous: shooter.position,
                direction,
                speed: 1_500.,
                tracer: false,
                incoming: false,
                age: 0,
                seeker: Some(Seeker {
                    acquired: false,
                    status: 1,
                    quality: 0.25,
                    target: Some(2),
                }),
            });
            frame.events.push(
                Event::new(kind::WEAPON_LAUNCH)
                    .with_subject(0)
                    .with_object(2)
                    .with(
                        field::PROJECTILE,
                        Value::Id(1 << 24 | (local / 1_200) as u32),
                    )
                    .with(field::WEAPON, Value::Id(1))
                    .with(field::RANGE_FT, 36_450.5)
                    .with(field::ASPECT_DEG, 35.),
            );
            frame.new_effects.push(EffectSpawn {
                kind: EffectKind::Launch,
                position: shooter.position,
                duration_ticks: 60,
            });
        }
        if let Some(m) = &mut missile {
            if m.age >= 360 {
                frame.events.push(
                    Event::new(kind::WEAPON_OUTCOME)
                        .with_subject(0)
                        .with_object(2)
                        .with(field::PROJECTILE, Value::Id(m.id))
                        .with(field::RESULT, outcome::MISSED)
                        .with(field::MISS_FT, 42.25),
                );
                missile = None;
            } else {
                m.previous = m.position;
                m.speed += 8.;
                m.direction = forward([0.3 + m.age as f64 * 0.002, 0.05, 0.]);
                for i in 0..3 {
                    m.position[i] += m.direction[i] * m.speed / 120.;
                }
                m.age += 1;
                if let Some(seeker) = &mut m.seeker {
                    seeker.acquired = m.age > 30;
                    seeker.quality = (m.age as f32 / 360.).min(1.);
                }
                frame.projectiles.push(m.clone());
                if local.is_multiple_of(8) {
                    frame.new_puffs.push(PuffSpawn {
                        layer: LAYER_SMOKE,
                        kind: PuffKind::Missile,
                        position: m.position,
                    });
                }
            }
        }
        // Gun bursts from aircraft 2.
        if local % 600 < 120 && local.is_multiple_of(6) {
            let shooter = &frame.aircraft[2];
            rounds.push(ProjectileState {
                id: next_round,
                owner: 2,
                weapon: 2,
                target: None,
                position: shooter.position,
                previous: shooter.position,
                direction: shooter.forward(),
                speed: 3_400.,
                tracer: next_round.is_multiple_of(5),
                incoming: false,
                age: 0,
                seeker: None,
            });
            next_round += 1;
        }
        rounds.retain(|r| r.age < 150);
        for r in &mut rounds {
            r.previous = r.position;
            for i in 0..3 {
                r.position[i] += r.direction[i] * r.speed / 120.;
            }
            r.position[1] -= 32.174 * r.age as f64 / 14_400.;
            r.age += 1;
            frame.projectiles.push(r.clone());
        }
        // Contrails from every aircraft each twelfth tick.
        if local.is_multiple_of(12) {
            for a in &frame.aircraft {
                for engine in [-6., 6.] {
                    frame.new_puffs.push(PuffSpawn {
                        layer: LAYER_CONTRAILS,
                        kind: PuffKind::Contrail,
                        position: [a.position[0] + engine, a.position[1], a.position[2] - 30.],
                    });
                }
            }
        }
        // Debris and a parachute after aircraft 3 is "hit" at 20 s.
        if local >= 2_400 {
            let age = (local - 2_400) as f64;
            for index in 0..3u32 {
                frame.debris.push(DebrisState {
                    owner: 3,
                    index,
                    position: [
                        910_000. + f64::from(index) * 20. + age * 0.5,
                        25_000. - age * age * 0.001,
                        830_000.,
                    ],
                    attitude: [
                        (age * 0.05 * f64::from(index + 1)).rem_euclid(TAU),
                        wrap(age * 0.03),
                        wrap(age * 0.11),
                    ],
                });
            }
            frame.escapees.push(EscapeeState {
                owner: 3,
                position: [910_050., 25_100. - age * 0.14, 830_000.],
                heading: (age * 0.01).rem_euclid(TAU),
                phase: if age > 240. { 2 } else { 1 },
            });
        }
        if local == 2_400 {
            frame.events.push(
                Event::new(kind::COMBAT_DESTROYED)
                    .with_subject(3)
                    .with_object(0)
                    .with(field::WEAPON, Value::Id(1)),
            );
            frame.surface_hp.push((0x4000_0001, 250));
            frame.surface_hp.push((0x4000_0002, 0));
        }
        if local % 300 == 7 {
            frame.events.push(
                Event::new(kind::COMMS_RADIO)
                    .with_subject(1)
                    .with(field::SPEAKER, "Friendly 1-2")
                    .with(field::HEARD, true)
                    .with(field::OUTCOME, outcome::DELIVERED)
                    .with(field::WAIT_S, 0.4)
                    .with(field::STEMS, "W12 FOX3")
                    .with_text("Fox three, one away"),
            );
        }
        // AI thinking at 10 per second and player telemetry at 30 per second.
        if local.is_multiple_of(12) {
            for a in &frame.aircraft[1..] {
                frame.trees.push(TreeSample {
                    subject: a.id,
                    channel: channel::AI_THOUGHT.into(),
                    nodes: vec![
                        Node::new(
                            0,
                            node::ACTIVITY,
                            if local % 2_400 < 1_200 {
                                "ATTACKING"
                            } else {
                                "FORMATION"
                            },
                        )
                        .with_note("leader released wing"),
                        Node::new(0, node::TARGET, Value::Id(0)),
                        Node::new(1, "Range", (a.position[2] / 6_076.).round() / 10.)
                            .with_unit(unit::NM),
                        Node::new(1, "Aspect", 35.).with_unit(unit::DEG),
                        Node::new(0, "Fuel", a.fuel_lb.round()).with_unit(unit::LB),
                    ],
                });
            }
        }
        if local.is_multiple_of(4) {
            let player = &frame.aircraft[0];
            let tas_kt = (player.airspeed / 1.687_81 * 10.).round() / 10.;
            previous_telemetry = tas_kt;
            let mut nodes = vec![
                Node::new(0, node::TAS, tas_kt).with_unit(unit::KT),
                Node::new(0, node::MACH, (tas_kt / 661.5 * 1_000.).round() / 1_000.),
                Node::new(0, node::AOA, 4.5 + (local % 40) as f64 * 0.1).with_unit(unit::DEG),
                Node::new(0, node::SIDESLIP, 0.25).with_unit(unit::DEG),
                Node::new(0, node::AGL, (player.position[1] - 1_200.).round()).with_unit(unit::FT),
                Node::new(0, node::LOAD, (player.g * 100.).round() / 100.).with_unit(unit::G),
            ];
            if local % 600 < 300 {
                nodes.push(
                    Node::new(1, "Low-speed G ceiling", 6.8)
                        .with_unit(unit::G)
                        .with_note("because speed near stall speed 142 kt"),
                );
            }
            nodes.push(Node::new(1, "Runway wind", Value::None));
            frame.trees.push(TreeSample {
                subject: 0,
                channel: channel::FLIGHT_TELEMETRY.into(),
                nodes,
            });
        }
        let _ = previous_telemetry;
        if tick.is_multiple_of(120) {
            frame.checksum = Some(state_checksum(&frame.aircraft));
        }
        frames.push(frame);
    }
    Scenario {
        header: header("UKR"),
        aircraft: infos,
        weapons: weapons(),
        frames,
        footer: Footer {
            end_tick: start + ticks,
            result: vec![
                ("outcome".into(), "victory".into()),
                ("kills".into(), "1".into()),
            ],
        },
    }
}

/// Writes a scenario and returns the finished file's path.
pub fn write(
    dir: &std::path::Path,
    name: &str,
    scenario: &Scenario,
    options: WriterOptions,
) -> PathBuf {
    let path = dir.join(name);
    let mut writer = Writer::create_with(&path, &scenario.header, options).unwrap();
    for info in &scenario.aircraft {
        writer.register_aircraft(info).unwrap();
    }
    for info in &scenario.weapons {
        writer.register_weapon(info).unwrap();
    }
    for frame in &scenario.frames {
        writer.push(frame).unwrap();
    }
    writer.finish(&scenario.footer).unwrap()
}

/// Largest wrapped angle difference.
pub fn angle_error(a: f64, b: f64) -> f64 {
    wrap(a - b).abs()
}
