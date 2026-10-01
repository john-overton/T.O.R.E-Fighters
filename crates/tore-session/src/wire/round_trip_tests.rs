//! Every section and message round-trips, with seeded random contents.

use super::connection::{ClientConnection, HostConnection};
use super::entity::{
    AircraftState, DamageState, DebrisState, Devices, EngineState, Entity, EntityKey, EntityKind,
    EntityState, Motion, PilotState, ProjectileState, Status,
};
use super::events::{EventsSection, Rumble, SectionEvent, WireEvent};
use super::inputs::{Command, InputFrame, InputsSection, NumberedCommand, quantize_command};
use super::messages::Message;
use super::names::NameIndex;
use super::own_state::{OwnStateReceiver, OwnStateSender};
use super::priority::Relevance;
use super::snapshot::{EntityReceiver, EntitySender, RecordBody, SnapshotSection};
use super::{SECTION_EVENTS, SECTION_SNAPSHOT, samples};
use tore_formats::aircraft::AircraftId;
use tore_net::SplitMix64;
use tore_sim::acoustics;
use tore_sim::combat::blast::MarkKind;
use tore_sim::combat::live::{DamageSection, EffectKind};
use tore_sim::sensors::{Channel, Controls};
use tore_sim::{ejection, wreck};
use tore_world::comms::Route;
use tore_world::world::OrderOutcome;

pub(crate) fn pick<T: Copy>(rng: &mut SplitMix64, items: &[T]) -> T {
    items[rng.below(items.len() as u64) as usize]
}

fn signed(rng: &mut SplitMix64, magnitude: u64) -> i64 {
    rng.below(2 * magnitude + 1) as i64 - magnitude as i64
}

pub(crate) fn motion(rng: &mut SplitMix64) -> Motion {
    let scale = pick(rng, &[0u64, 10, 1_000, 1 << 20, 1 << 40]);
    Motion {
        position: std::array::from_fn(|_| signed(rng, scale)),
        velocity: std::array::from_fn(|_| signed(rng, scale.min(1 << 24))),
    }
}

fn angle(rng: &mut SplitMix64) -> u16 {
    rng.below(65_536) as u16
}

fn chance(rng: &mut SplitMix64) -> bool {
    rng.below(2) == 1
}

/// A random entity of `kind`.
pub(crate) fn entity(rng: &mut SplitMix64, kind: EntityKind, id: u32) -> Entity {
    let state = match kind {
        EntityKind::Aircraft => EntityState::Aircraft(AircraftState {
            aircraft: chance(rng).then(|| pick(rng, &AircraftId::SELECTABLE)),
            motion: motion(rng),
            attitude: [angle(rng), angle(rng), angle(rng)],
            devices: chance(rng).then(|| Devices {
                levels: std::array::from_fn(|_| rng.below(256) as u8),
                surfaces: std::array::from_fn(|_| signed(rng, 127) as i8),
                speed: signed(rng, 20_000) as i32,
                throttle: rng.below(256) as u8,
            }),
            engine: EngineState {
                lit: chance(rng),
                afterburner: chance(rng),
                flame: chance(rng),
                rates: std::array::from_fn(|_| signed(rng, 50_000) as i32),
            },
            damage: DamageState {
                hp: signed(rng, 1_000) as i32,
                initial_hp: rng.below(1_000) as i32,
                sections: std::array::from_fn(|_| rng.below(300) as i32),
                structural: chance(rng).then(|| {
                    pick(
                        rng,
                        &[
                            DamageSection::Nose,
                            DamageSection::Core,
                            DamageSection::Tail,
                            DamageSection::RightWing,
                        ],
                    )
                }),
            },
            status: Status {
                airborne: chance(rng),
                crashed: chance(rng),
                wreck: chance(rng).then(|| {
                    pick(
                        rng,
                        &[
                            wreck::Phase::Falling,
                            wreck::Phase::Grounded,
                            wreck::Phase::Exploded,
                        ],
                    )
                }),
            },
        }),
        EntityKind::Projectile => EntityState::Projectile(ProjectileState {
            owner: rng.below(64) as u32,
            weapon: NameIndex(rng.below(4096) as u16),
            shape: chance(rng).then(|| NameIndex(rng.below(4096) as u16)),
            target: chance(rng).then(|| rng.below(1 << 32) as u32),
            aimed_at_player: chance(rng),
            motion: motion(rng),
            direction: [angle(rng), angle(rng)],
        }),
        EntityKind::Debris => EntityState::Debris(DebrisState {
            owner: id,
            model: chance(rng).then(|| pick(rng, &AircraftId::SELECTABLE)),
            variant: chance(rng).then(|| rng.below(8) as u8),
            motion: motion(rng),
            attitude: [angle(rng), angle(rng), angle(rng)],
        }),
        EntityKind::Pilot => EntityState::Pilot(PilotState {
            owner: id,
            motion: motion(rng),
            heading: angle(rng),
            phase: pick(
                rng,
                &[
                    ejection::Phase::Seat,
                    ejection::Phase::Freefall,
                    ejection::Phase::Inflating,
                    ejection::Phase::Parachute,
                    ejection::Phase::Landed,
                    ejection::Phase::Impact,
                ],
            ),
        }),
    };
    Entity { id, state }
}

/// `entity` a little later: its motion moved on, sometimes a slow field
/// changed, identity kept.
pub(crate) fn step(rng: &mut SplitMix64, entity: &Entity, ticks: u32) -> Entity {
    let mut next = *entity;
    let fresh = self::entity(rng, entity.state.kind(), entity.id);
    let wander = |rng: &mut SplitMix64, m: &mut Motion| {
        let predicted = m.predicted(ticks);
        for (i, predicted) in predicted.iter().enumerate() {
            let scale = pick(rng, &[0u64, 3, 100, 5_000]);
            let jitter = signed(rng, scale);
            m.position[i] = (*predicted as i64 + jitter).clamp(-(1 << 40), 1 << 40);
            let scale = pick(rng, &[0u64, 5, 2_000]);
            m.velocity[i] = (m.velocity[i] + signed(rng, scale)).clamp(-(1 << 40), 1 << 40);
        }
    };
    let slow = rng.below(4) == 0;
    match (&mut next.state, fresh.state) {
        (EntityState::Aircraft(a), EntityState::Aircraft(f)) => {
            wander(rng, &mut a.motion);
            a.attitude[rng.below(3) as usize] = angle(rng);
            if slow {
                a.devices = f.devices;
                a.engine = f.engine;
                a.damage = f.damage;
            }
            if let Some(d) = &mut a.devices {
                d.speed = (d.speed + signed(rng, 40) as i32).clamp(-20_000, 20_000);
            }
        }
        (EntityState::Projectile(p), EntityState::Projectile(f)) => {
            wander(rng, &mut p.motion);
            p.direction = f.direction;
        }
        (EntityState::Debris(d), EntityState::Debris(f)) => {
            wander(rng, &mut d.motion);
            d.attitude = f.attitude;
        }
        (EntityState::Pilot(p), EntityState::Pilot(f)) => {
            wander(rng, &mut p.motion);
            if slow {
                p.phase = f.phase;
            }
        }
        _ => unreachable!(),
    }
    next
}

fn frame(rng: &mut SplitMix64) -> InputFrame {
    InputFrame {
        pitch: signed(rng, 32_767) as i16,
        roll: signed(rng, 32_767) as i16,
        yaw: signed(rng, 32_767) as i16,
        throttle_rate: signed(rng, 127) as i8,
        throttle: chance(rng).then(|| rng.below(65_536) as u16),
        trigger: chance(rng),
        sensors: Controls {
            channel: pick(rng, &[Channel::Radar, Channel::Infrared, Channel::Visual]),
            range_index: rng.below(6) as usize,
            history: chance(rng),
        },
    }
}

pub(crate) fn inputs(rng: &mut SplitMix64) -> InputsSection {
    let count = 1 + rng.below(24) as usize;
    let mut frames = vec![frame(rng)];
    for _ in 1..count {
        let mut next = *frames.last().unwrap();
        if chance(rng) {
            let other = frame(rng);
            match rng.below(4) {
                0 => next.pitch = other.pitch,
                1 => next = other,
                2 => next.trigger = !next.trigger,
                _ => next.sensors = other.sensors,
            }
        }
        frames.push(next);
    }
    let newest_tick = 100 + rng.below(1 << 30) as u32;
    let commands = samples::commands();
    let command_count = rng.below(65) as usize;
    let first = rng.below(65_536) as u16;
    InputsSection {
        flight: rng.below(256) as u8,
        newest_tick,
        frames,
        view_offset: rng.below(256) as u8,
        interpolation_delay: rng.below(64) as u8,
        view_subject: chance(rng).then(|| EntityKey {
            kind: pick(rng, &EntityKind::ALL),
            id: rng.below(1 << 32) as u32,
        }),
        mismatch: rng.below(1 << 32) as u32,
        commands: (0..command_count)
            .map(|index| NumberedCommand {
                number: first.wrapping_add(index as u16),
                tick: newest_tick - rng.below(100) as u32,
                command: match pick(rng, &commands) {
                    Command::Pilot(pilot) => Command::Pilot(quantize_command(pilot)),
                    seat => seat,
                },
            })
            .collect(),
    }
}

pub(crate) fn event(rng: &mut SplitMix64) -> WireEvent {
    let text = |rng: &mut SplitMix64| "x".repeat(rng.below(256) as usize);
    let position = |rng: &mut SplitMix64| std::array::from_fn(|_| signed(rng, 1 << 40));
    let names = |rng: &mut SplitMix64| {
        (0..rng.below(33))
            .map(|_| NameIndex(rng.below(4096) as u16))
            .collect()
    };
    match rng.below(17) {
        0 => WireEvent::Message { text: text(rng) },
        1 => WireEvent::Radio {
            route: pick(rng, &[Route::Radio, Route::Airport, Route::Direct]),
            important: chance(rng),
            label: text(rng),
            text: text(rng),
            stems: names(rng),
        },
        2 => WireEvent::Tower {
            stem: chance(rng).then(|| NameIndex(rng.below(4096) as u16)),
        },
        3 => WireEvent::OrderVoice { stems: names(rng) },
        4 => WireEvent::OrderReply {
            order: tore_sim::ai::wing::PlayerOrder::Spacing,
            outcome: OrderOutcome::Given { message: text(rng) },
        },
        5 => WireEvent::WeaponCycled,
        6 => WireEvent::Release {
            sound: NameIndex(rng.below(4096) as u16),
            station: rng.below(256) as u8,
        },
        7 => WireEvent::Launch {
            shooter: rng.below(1 << 32) as u32,
            projectile: rng.below(1 << 32) as u32,
            weapon: NameIndex(rng.below(4096) as u16),
        },
        8 => WireEvent::Feedback {
            rumble: pick(
                rng,
                &[
                    Rumble::Turbulence(7),
                    Rumble::GunFired,
                    Rumble::Damage,
                    Rumble::Crash,
                ],
            ),
        },
        9 => WireEvent::YourAircraftExploded {
            on_impact: chance(rng),
        },
        10 => WireEvent::WingEjection {
            aircraft: rng.below(64) as u32,
            message: text(rng),
            friendly: chance(rng),
        },
        11 => WireEvent::Effect {
            kind: pick(
                rng,
                &[EffectKind::Flare, EffectKind::DebrisImpact, EffectKind::Hit],
            ),
            position: position(rng),
            ticks: rng.below(65_536) as u16,
            blast: chance(rng).then(|| rng.below(256) as u8),
        },
        12 => WireEvent::Mark {
            kind: if chance(rng) {
                MarkKind::Fire
            } else {
                MarkKind::Crater(rng.below(256) as u8)
            },
            position: position(rng),
        },
        13 => WireEvent::GroundDestroyed {
            object: rng.below(1 << 32) as u32,
        },
        14 => WireEvent::Countermeasure {
            aircraft: rng.below(64) as u32,
            flare: chance(rng),
            position: position(rng),
            velocity: position(rng),
            attitude: [angle(rng), angle(rng), angle(rng)],
            number: rng.next_u64(),
            left: chance(rng).then(|| rng.below(256) as u8),
        },
        15 => WireEvent::GunBurst {
            shooter: rng.below(64) as u32,
            station: rng.below(256) as u8,
            length: chance(rng).then(|| rng.below(1 << 31) as u32),
        },
        _ => WireEvent::Sound {
            kind: pick(
                rng,
                &[
                    acoustics::Kind::Impact,
                    acoustics::Kind::Blast(200),
                    acoustics::Kind::Flare,
                    acoustics::Kind::MissilePass,
                ],
            ),
            position: position(rng),
            arrived: chance(rng),
            from: chance(rng).then(|| rng.below(1 << 32) as u32),
        },
    }
}

#[test]
fn inputs_round_trip() {
    let sample = samples::inputs();
    assert_eq!(
        InputsSection::decode(&sample.encode().unwrap()).unwrap(),
        sample
    );
    let mut rng = SplitMix64::new(11);
    for _ in 0..2_000 {
        let section = inputs(&mut rng);
        let bytes = section.encode().unwrap();
        assert_eq!(InputsSection::decode(&bytes).unwrap(), section);
    }
}

#[test]
fn events_round_trip() {
    let sample = samples::events();
    assert_eq!(
        EventsSection::decode(&sample.encode().unwrap()).unwrap(),
        sample
    );
    let mut rng = SplitMix64::new(12);
    for _ in 0..500 {
        let mut number = rng.below(65_536) as u16;
        let events = (0..1 + rng.below(40))
            .map(|_| {
                number = number.wrapping_add(1 + rng.below(3) as u16);
                SectionEvent {
                    number,
                    ticks_back: rng.below(1 << 32) as u32,
                    event: event(&mut rng),
                }
            })
            .collect();
        let section = EventsSection { events };
        assert_eq!(
            EventsSection::decode(&section.encode().unwrap()).unwrap(),
            section
        );
    }
}

#[test]
fn messages_round_trip() {
    for message in samples::messages(vec![9; 600]) {
        let bytes = message.encode().unwrap();
        assert_eq!(
            Message::decode(message.kind(), &bytes).unwrap(),
            message,
            "{message:?}"
        );
    }
}

#[test]
fn full_records_round_trip() {
    let mut rng = SplitMix64::new(13);
    for _ in 0..300 {
        let mut entities: Vec<Entity> = Vec::new();
        for kind in EntityKind::ALL {
            let mut id = rng.below(1_000) as u32;
            for _ in 0..rng.below(10) {
                entities.push(entity(&mut rng, kind, id));
                id += 1 + rng.below(3_000) as u32;
            }
        }
        let removed = [EntityKey {
            kind: EntityKind::Pilot,
            id: 5_000_000,
        }];
        let header = samples::header(rng.below(1 << 31) as u32);
        let bytes = SnapshotSection::encode_full(&header, &entities, &removed).unwrap();
        let section = SnapshotSection::decode(&bytes).unwrap();
        assert_eq!(section.header, header);
        let mut expected: Vec<(EntityKey, Option<EntityState>)> = entities
            .iter()
            .map(|e| (e.key(), Some(e.state)))
            .chain(removed.iter().map(|k| (*k, None)))
            .collect();
        expected.sort_by_key(|(key, _)| *key);
        let got: Vec<(EntityKey, Option<EntityState>)> = section
            .records
            .iter()
            .map(|r| match &r.body {
                RecordBody::Full(state) => (r.key, Some(*state)),
                RecordBody::Removed => (r.key, None),
                RecordBody::Delta { .. } => panic!("no baselines here"),
            })
            .collect();
        assert_eq!(got, expected);
    }
}

#[test]
fn records_against_baselines_round_trip() {
    let mut rng = SplitMix64::new(14);
    let mut host = EntitySender::new(4);
    let mut client = EntityReceiver::new(4);
    let mut world: Vec<Entity> = (0..20)
        .map(|i| entity(&mut rng, EntityKind::ALL[i % 4], i as u32 * 3))
        .collect();
    for snapshot in 0..400u32 {
        let tick = 1_000 + snapshot * 4;
        world = world.iter().map(|e| step(&mut rng, e, 4)).collect();
        let entities: Vec<_> = world.iter().map(|e| (*e, Relevance::NEAR)).collect();
        let (bytes, report) = host
            .build(&samples::header(tick), &entities, 4_000)
            .unwrap();
        assert_eq!(report.waiting, 0);
        host.sent(snapshot as u16);
        let section = SnapshotSection::decode(&bytes).unwrap();
        let received = client.receive(&section);
        assert!(received.unresolved.is_empty());
        for e in &world {
            assert_eq!(client.state(e.key(), tick), Some(&e.state));
        }
        // Acknowledge two snapshots in three.
        if snapshot % 3 != 2 {
            host.delivered(snapshot as u16);
        } else {
            host.lost(snapshot as u16);
        }
        if snapshot > 2 {
            assert!(
                report.full == 0,
                "snapshot {snapshot} sent {} in full",
                report.full
            );
        }
    }
}

#[test]
fn a_connection_carries_snapshots_events_and_names() {
    let mut host = HostConnection::new(4);
    let mut client = ClientConnection::for_flight(4, 0);
    let weapon = host.names.intern("AIM120.JT").unwrap();
    let names = host.names.take_new().unwrap();
    host.event(
        396,
        &WireEvent::Launch {
            shooter: 1,
            projectile: 70_001,
            weapon,
        },
    )
    .unwrap();
    let entities: Vec<_> = samples::entities()
        .iter()
        .map(|e| (*e, Relevance::NEAR))
        .collect();
    let packet = host
        .snapshot(&samples::header(400), &entities, 256)
        .unwrap();
    assert!(packet.bytes() <= tore_net::MAX_DATAGRAM - 256 - 3);
    host.sent(1);
    for (kind, body) in packet.sections() {
        assert!(client.check(kind, body));
    }
    let sections = packet.sections();
    let (header, received) = client.snapshot(sections[0].1).unwrap();
    assert_eq!(header, samples::header(400));
    assert_eq!(received.updated.len(), entities.len());
    assert_eq!(sections[1].0, SECTION_EVENTS);
    assert_eq!(sections[0].0, SECTION_SNAPSHOT);
    // The event names a weapon the client does not know yet: it waits.
    assert!(
        client
            .events(sections[1].1, header.tick)
            .unwrap()
            .is_empty()
    );
    let released = client.names(&names).unwrap();
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].tick, 396);
    // Delivered: the event is acknowledged and the next packet codes
    // against the baselines.
    host.delivered(1);
    assert!(host.events.is_empty());
    let moved: Vec<_> = samples::entities()
        .iter()
        .map(|e| (samples::moved(e, 4), Relevance::NEAR))
        .collect();
    let next = host.snapshot(&samples::header(404), &moved, 256).unwrap();
    assert!(next.events.is_none());
    assert_eq!(next.entities.full, 0);
    assert!(next.snapshot.len() < packet.snapshot.len());
    let (_, received) = client.snapshot(&next.snapshot).unwrap();
    assert_eq!(
        received.updated,
        moved.iter().map(|(e, _)| *e).collect::<Vec<_>>()
    );
}

#[test]
fn own_states_round_trip_against_acknowledged_ones() {
    use tore_world::mission::{MissionSpec, Start};
    use tore_world::seats::SeatInput;
    use tore_world::test_support::resources::{THEATER, resources};
    use tore_world::world::plane::{ExactState, OwnPlane};
    use tore_world::world::{Seating, TickOutput, World};

    let map = resources();
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    let mut world = World::new(&spec, &map, Seating::SinglePlayer).unwrap();
    let model = tore_sim::models::AircraftModel::for_aircraft(
        &tore_world::aircraft_type::AircraftType::load(&map, AircraftId::F18)
            .unwrap()
            .profile,
    )
    .unwrap();
    let mut host = OwnStateSender::new();
    let mut client = OwnStateReceiver::new();
    let mut out = TickOutput::default();
    let mut sizes = Vec::new();
    for tick in 0..480u32 {
        let input = SeatInput {
            tick: world.tick(),
            pilot: tore_sim::flight::PilotInput {
                pitch: (f64::from(tick) * 0.05).sin() * 0.3,
                ..Default::default()
            },
            ..SeatInput::default()
        };
        world.step(&[input], &mut out).unwrap();
        if tick % 20 != 0 {
            continue;
        }
        let terms = out.terms.first().map(|(_, terms)| *terms);
        let state = ExactState::of(&OwnPlane::of(&world.cockpits[0]), terms.as_ref());
        let bytes = host.build(tick, &state).unwrap();
        let sequence = tick as u16;
        host.sent(sequence);
        let (header, decoded) = client.receive(&bytes, &model).unwrap();
        assert_eq!(header.tick, tick);
        assert_eq!(decoded, state);
        assert_eq!(decoded.hash().unwrap(), state.hash().unwrap());
        sizes.push((header.back, bytes.len()));
        // Every other one is acknowledged.
        if tick % 40 == 0 {
            host.delivered(sequence);
        } else {
            host.lost(sequence);
        }
    }
    assert_eq!(sizes[0].0, 0, "the first has no baseline");
    assert!(sizes[1..].iter().all(|(back, _)| *back > 0));
    let (_, first) = sizes[0];
    assert!(sizes[2..].iter().all(|(_, len)| *len < first), "{sizes:?}");
}

#[test]
fn entities_keep_their_share_whatever_else_waits() {
    let mut rng = SplitMix64::new(15);
    let mut host = HostConnection::new(4);
    // A long radio call per tick for a while: far more than a packet holds.
    for tick in 0..900 {
        host.event(
            tick,
            &WireEvent::Radio {
                route: Route::Radio,
                important: false,
                label: "VIPER 3".into(),
                text: "y".repeat(200),
                stems: Vec::new(),
            },
        )
        .unwrap();
    }
    // Thirty aircraft a few seconds into a fight, as the budget test
    // measures them: small motions against a baseline.
    let mut world: Vec<Entity> = (1..=30)
        .map(|id| {
            let mut e = entity(&mut rng, EntityKind::Aircraft, id);
            if let EntityState::Aircraft(a) = &mut e.state {
                a.motion.position = [
                    signed(&mut rng, 1 << 25),
                    320_000,
                    signed(&mut rng, 1 << 25),
                ];
                a.motion.velocity = [signed(&mut rng, 60_000), 0, signed(&mut rng, 60_000)];
            }
            e
        })
        .collect();
    for snapshot in 0..60u32 {
        for e in &mut world {
            if let EntityState::Aircraft(a) = &mut e.state {
                let predicted = a.motion.predicted(4);
                for (i, predicted) in predicted.iter().enumerate() {
                    a.motion.position[i] = *predicted as i64 + signed(&mut rng, 3);
                    a.motion.velocity[i] += signed(&mut rng, 40);
                    a.attitude[i] = a.attitude[i].wrapping_add(signed(&mut rng, 50) as u16);
                }
                if let Some(d) = &mut a.devices {
                    d.speed += signed(&mut rng, 8) as i32;
                }
            }
        }
        let entities: Vec<_> = world.iter().map(|e| (*e, Relevance::NEAR)).collect();
        let packet = host
            .snapshot(&samples::header(1_000 + 4 * snapshot), &entities, 256)
            .unwrap();
        assert!(packet.shares.entities >= super::space::ENTITIES_MIN);
        // The events fill what is left; the oldest goes even when it needs
        // the messages' room.
        assert!(packet.events.is_some());
        assert!(
            packet.bytes() <= tore_net::MAX_DATAGRAM,
            "{}",
            packet.bytes()
        );
        if packet.entities.waiting > 0 {
            // Short of room only while first records are sent in full, and
            // then the entities use their whole share, events or not.
            assert!(snapshot < 5, "snapshot {snapshot}");
            assert!(
                packet.snapshot.len() + 64 >= packet.shares.entities,
                "{}",
                packet.snapshot.len()
            );
        }
        host.sent(snapshot as u16);
        host.delivered(snapshot as u16);
    }
}
