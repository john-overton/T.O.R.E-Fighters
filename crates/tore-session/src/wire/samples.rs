//! Fixed sample contents of every section and message, shared by the golden,
//! round-trip and fuzz tests.

use super::entity::{
    AircraftState, DamageState, DebrisState, Devices, EngineState, Entity, EntityKey, EntityKind,
    EntityState, Motion, PilotState, ProjectileState, Status,
};
use super::events::{EventsSection, Rumble, SectionEvent, WireEvent};
use super::inputs::{Command, InputFrame, InputsSection, NumberedCommand};
use super::messages::{
    ContentRefused, Debrief, DebriefObjective, DebriefPilot, EndReason, Message, Mission,
    MissionEnded, Names, PilotStatus, Ready, Roster, RosterPilot, RosterPlane, Seated,
};
use super::names::NameIndex;
use super::priority::Relevance;
use super::snapshot::{EntitySender, SnapshotHeader};
use tore_formats::aircraft::AircraftId;
use tore_sim::acoustics;
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::ai::wing::{Formation, PlayerBreak, PlayerOrder};
use tore_sim::combat::blast::MarkKind;
use tore_sim::combat::ledger::Tally;
use tore_sim::combat::live::{self, DamageSection, EffectKind};
use tore_sim::flight::{PilotCommand, Switch};
use tore_sim::sensors::{Channel, Controls};
use tore_sim::{airport, ejection, wreck};
use tore_world::comms::Route;
use tore_world::mission::{LoadoutSpec, StationLoad};
use tore_world::resources::{Manifest, ManifestEntry};
use tore_world::seats::SeatCommand;
use tore_world::world::{AirportInput, OrderOutcome};

/// Every kind of command, seat and pilot.
pub fn commands() -> Vec<Command> {
    use SeatCommand as S;
    vec![
        Command::Seat(S::CycleWeapon { forward: true }),
        Command::Seat(S::Airport(AirportInput::NavMode)),
        Command::Seat(S::Airport(AirportInput::Command(
            airport::Command::SelectAirport(1234),
        ))),
        Command::Seat(S::Airport(AirportInput::Command(
            airport::Command::RequestLanding,
        ))),
        Command::Seat(S::Airport(AirportInput::Command(
            airport::Command::RepeatReply,
        ))),
        Command::Seat(S::Airport(AirportInput::Command(
            airport::Command::CancelApproach,
        ))),
        Command::Seat(S::Combat(live::Command::DesignateTarget(77))),
        Command::Seat(S::Combat(live::Command::Designate)),
        Command::Seat(S::Manual(live::Command::ToggleArm)),
        Command::Seat(S::Manual(live::Command::TargetHeat(3))),
        Command::Seat(S::Manual(live::Command::TargetDistance(40_000))),
        Command::Seat(S::RangeReset),
        Command::Seat(S::ReleaseChaff),
        Command::Seat(S::ReleaseFlare),
        Command::Seat(S::ReleaseTrigger),
        Command::Seat(S::RadioSilence),
        Command::Seat(S::WingRecipient(Some(2))),
        Command::Seat(S::WingRecipient(None)),
        Command::Seat(S::WingOrder(PlayerOrder::Break(PlayerBreak::High))),
        Command::Seat(S::WingOrder(PlayerOrder::Formation(Formation::LineAstern))),
        Command::Seat(S::WingOrder(PlayerOrder::LandAtSelected)),
        Command::Seat(S::WingFormationCycle),
        Command::Seat(S::TriggerKey {
            down: true,
            repeat: false,
            blocked: true,
        }),
        Command::Pilot(PilotCommand::Eject),
        Command::Pilot(PilotCommand::Toggle(Switch::Gear)),
        Command::Pilot(PilotCommand::Set(Switch::WaypointAutopilot, true)),
        Command::Pilot(PilotCommand::Throttle(0.5)),
        Command::Pilot(PilotCommand::AdjustThrottle(-0.05)),
    ]
}

/// An Inputs section with every kind of change between ticks.
pub fn inputs() -> InputsSection {
    let first = InputFrame {
        pitch: 1000,
        roll: -32_767,
        yaw: 0,
        throttle_rate: 0,
        throttle: Some(40_000),
        trigger: false,
        sensors: Controls::default(),
    };
    let mut frames = vec![first];
    let mut next = first;
    next.pitch += 3;
    frames.push(next);
    frames.push(next);
    next.roll = 32_767;
    next.trigger = true;
    next.throttle = None;
    next.throttle_rate = -127;
    frames.push(next);
    next.sensors = Controls {
        channel: Channel::Infrared,
        range_index: 5,
        history: true,
    };
    next.yaw = -200;
    frames.push(next);
    InputsSection {
        newest_tick: 10_000,
        frames,
        view_offset: 37,
        interpolation_delay: 12,
        view_subject: Some(EntityKey {
            kind: EntityKind::Aircraft,
            id: 5,
        }),
        mismatch: 9_990,
        commands: commands()
            .into_iter()
            .enumerate()
            .map(|(index, command)| NumberedCommand {
                number: 65_530u16.wrapping_add(index as u16),
                tick: 10_000 - (index as u32 % 5),
                command: match command {
                    Command::Pilot(pilot) => Command::Pilot(super::inputs::quantize_command(pilot)),
                    seat => seat,
                },
            })
            .collect(),
    }
}

fn motion(seed: i64) -> Motion {
    Motion {
        position: [seed * 1_000 + 17, 320_000 + seed, -seed * 777],
        velocity: [seed * 64, -3, 50_000],
    }
}

/// One entity of each kind and shape.
pub fn entities() -> Vec<Entity> {
    vec![
        Entity {
            id: 1,
            state: EntityState::Aircraft(AircraftState {
                aircraft: Some(AircraftId::Mig29),
                motion: motion(1),
                attitude: [100, 65_000, 32_768],
                devices: Some(Devices {
                    levels: [255, 128, 0, 0, 3, 200],
                    surfaces: [-127, 0, 127],
                    speed: 3_200,
                    throttle: 230,
                }),
                engine: EngineState {
                    lit: true,
                    afterburner: true,
                    flame: true,
                    rates: [-40, 0, 4_096],
                },
                damage: DamageState {
                    hp: 120,
                    initial_hp: 150,
                    sections: [0, 5, 0, 25, 0, 0],
                    structural: Some(DamageSection::LeftWing),
                },
                status: Status {
                    airborne: true,
                    crashed: false,
                    wreck: None,
                },
            }),
        },
        Entity {
            id: 9,
            state: EntityState::Aircraft(AircraftState {
                aircraft: None,
                motion: motion(9),
                attitude: [0, 0, 0],
                devices: None,
                engine: EngineState::default(),
                damage: DamageState {
                    hp: 0,
                    initial_hp: 90,
                    ..DamageState::default()
                },
                status: Status {
                    airborne: false,
                    crashed: true,
                    wreck: Some(wreck::Phase::Grounded),
                },
            }),
        },
        Entity {
            id: 70_001,
            state: EntityState::Projectile(ProjectileState {
                owner: 9,
                weapon: NameIndex(3),
                shape: Some(NameIndex(4)),
                target: Some(0),
                aimed_at_player: true,
                motion: motion(2),
                direction: [1, 60_000],
            }),
        },
        Entity {
            id: 12,
            state: EntityState::Debris(DebrisState {
                owner: 12,
                model: Some(AircraftId::F18),
                variant: Some(1),
                motion: motion(3),
                attitude: [4, 5, 6],
            }),
        },
        Entity {
            id: 12,
            state: EntityState::Pilot(PilotState {
                owner: 12,
                motion: motion(4),
                heading: 12_345,
                phase: ejection::Phase::Parachute,
            }),
        },
    ]
}

/// A snapshot header with every field set.
pub fn header(tick: u32) -> SnapshotHeader {
    SnapshotHeader {
        tick,
        input_received: tick + 7,
        input_margin: -3,
        inputs_repeated: 2,
        commands_applied: 65_535,
        own_hash: Some(0x0123_4567_89AB_CDEF),
    }
}

/// `entity` moved on by `ticks` ticks with every group changed.
pub fn moved(entity: &Entity, ticks: i64) -> Entity {
    let mut next = *entity;
    let bump = |m: &mut Motion| {
        for i in 0..3 {
            m.position[i] += m.velocity[i] * ticks / 240 + 5;
            m.velocity[i] -= 7;
        }
    };
    match &mut next.state {
        EntityState::Aircraft(a) => {
            bump(&mut a.motion);
            a.attitude[0] = a.attitude[0].wrapping_add(300);
            if let Some(d) = &mut a.devices {
                d.levels[2] = 255;
                d.speed += 40;
            }
            a.engine.rates[1] = -5;
            a.damage.hp -= 10;
            a.status.crashed = !a.status.crashed;
        }
        EntityState::Projectile(p) => {
            bump(&mut p.motion);
            p.direction[1] = p.direction[1].wrapping_add(1_000);
        }
        EntityState::Debris(d) => {
            bump(&mut d.motion);
            d.attitude[2] = 65_535;
        }
        EntityState::Pilot(p) => {
            bump(&mut p.motion);
            p.phase = ejection::Phase::Landed;
        }
    }
    next
}

/// A first snapshot (every record in full) and a second one against it
/// after the first is delivered, with the debris piece removed.
pub fn snapshots() -> (Vec<u8>, Vec<u8>) {
    let mut sender = EntitySender::new(4);
    let near = |e: &Entity| (*e, Relevance::NEAR);
    let first: Vec<_> = entities().iter().map(near).collect();
    let (full, _) = sender.build(&header(400), &first, 1_000).unwrap();
    sender.sent(1);
    sender.delivered(1);
    let mut second: Vec<_> = entities().iter().map(|e| near(&moved(e, 8))).collect();
    second.retain(|(e, _)| e.state.kind() != EntityKind::Debris);
    let (delta, _) = sender.build(&header(408), &second, 1_000).unwrap();
    (full, delta)
}

/// One event of every kind.
pub fn events() -> EventsSection {
    let list = vec![
        WireEvent::Message {
            text: "OVERSPEED".into(),
        },
        WireEvent::Radio {
            route: Route::Radio,
            important: true,
            label: "VIPER 2".into(),
            text: "Fox two!".into(),
            stems: vec![NameIndex(1), NameIndex(4095)],
        },
        WireEvent::Tower {
            stem: Some(NameIndex(2)),
        },
        WireEvent::Tower { stem: None },
        WireEvent::OrderVoice {
            stems: vec![NameIndex(7)],
        },
        WireEvent::OrderReply {
            order: PlayerOrder::BugOut,
            outcome: OrderOutcome::Refused {
                message: "No airport".into(),
            },
        },
        WireEvent::WeaponCycled,
        WireEvent::Release {
            sound: NameIndex(5),
            station: 3,
        },
        WireEvent::Launch {
            shooter: 4,
            projectile: 70_001,
            weapon: NameIndex(3),
        },
        WireEvent::Feedback {
            rumble: Rumble::Turbulence(200),
        },
        WireEvent::Feedback {
            rumble: Rumble::Crash,
        },
        WireEvent::YourAircraftExploded { on_impact: true },
        WireEvent::WingEjection {
            aircraft: 6,
            message: "WING 3 EJECTED".into(),
            friendly: false,
        },
        WireEvent::Effect {
            kind: EffectKind::Destroyed,
            position: [1, -2, 3_000_000],
            ticks: 480,
            blast: Some(12),
        },
        WireEvent::Mark {
            kind: MarkKind::Crater(2),
            position: [-5, 6, 7],
        },
        WireEvent::Mark {
            kind: MarkKind::Fire,
            position: [0, 0, 0],
        },
        WireEvent::GroundDestroyed { object: 1_000_123 },
        WireEvent::Countermeasure {
            aircraft: 2,
            flare: true,
            position: [10, 20, 30],
            velocity: [-64, 0, 64_000],
            attitude: [1, 2, 3],
            number: 99,
            left: Some(29),
        },
        WireEvent::GunBurst {
            shooter: 3,
            station: 0,
            length: None,
        },
        WireEvent::GunBurst {
            shooter: 3,
            station: 0,
            length: Some(0),
        },
        WireEvent::Sound {
            kind: acoustics::Kind::Blast(9),
            position: [1, 1, 1],
            arrived: true,
            from: Some(8),
        },
        WireEvent::Sound {
            kind: acoustics::Kind::SonicBoom,
            position: [1, 1, 1],
            arrived: false,
            from: None,
        },
    ];
    EventsSection {
        events: list
            .into_iter()
            .enumerate()
            .map(|(index, event)| SectionEvent {
                // Numbers wrap and skip, as acknowledged ones leave the queue.
                number: 65_500u16.wrapping_add(3 * index as u16),
                ticks_back: index as u32 * 11,
                event,
            })
            .collect(),
    }
}

fn tally(seed: u32) -> Tally {
    Tally {
        launched: seed,
        hit: seed / 2,
        damage: seed * 10,
        missed: 1,
        spoofed: 0,
        jammed: 2,
    }
}

fn pilot(seed: u32) -> DebriefPilot {
    DebriefPilot {
        status: PilotStatus::Ejected,
        damage: 0.625,
        landing_grade: Some(88),
        cause: Some("overspeed".into()),
        kills: [seed, 0, 0, 1, 0, 0, 0, 0, 3, 0],
        friendly_fire: 1,
        air_to_air: tally(seed),
        air_to_ground: tally(1),
        gun: tally(300),
        bombs: tally(0),
        enemy_aam: tally(4),
        enemy_sam: tally(0),
        enemy_gun: tally(7),
        enemy_aaa: tally(0),
    }
}

/// A roster of three planes, one a human.
pub fn roster() -> Roster {
    Roster {
        planes: vec![
            RosterPlane {
                id: 0,
                wing: WingId::new(Side::Friendly, 0).unwrap(),
                member: 0,
                aircraft: AircraftId::F18,
                pilot: RosterPilot::Human {
                    seat: 1,
                    callsign: "Viper_2".into(),
                },
            },
            RosterPlane {
                id: 1,
                wing: WingId::new(Side::Friendly, 0).unwrap(),
                member: 1,
                aircraft: AircraftId::Faxx,
                pilot: RosterPilot::Ai,
            },
            RosterPlane {
                id: 16,
                wing: WingId::new(Side::Enemy, 2).unwrap(),
                member: 3,
                aircraft: AircraftId::Su35,
                pilot: RosterPilot::Ai,
            },
        ],
    }
}

/// One message of every kind; `exact` stands for a coded exact state.
pub fn messages(exact: Vec<u8>) -> Vec<Message> {
    vec![
        Message::Mission(Mission {
            spec: "theater UKR\ncondition clear\n".repeat(20),
            manifest: Manifest {
                entries: vec![
                    ManifestEntry {
                        name: "F18.PT".into(),
                        hash: Some(0xDEAD_BEEF_0000_0001),
                    },
                    ManifestEntry {
                        name: "MISSING.PT".into(),
                        hash: None,
                    },
                ],
            },
            host_tick: 123_456,
            contrail_sortie: 3,
        }),
        Message::ContentRefused(ContentRefused {
            names: vec!["UKR.T2".into(), "F18.PT".into()],
        }),
        Message::Ready(Ready { plane: Some(4) }),
        Message::Ready(Ready { plane: None }),
        Message::SeatRefused("plane taken".into()),
        Message::Seated(Box::new(Seated {
            seat: 2,
            plane: 3,
            tick: 7_200,
            exact,
            loadout: LoadoutSpec {
                fuel_lbs: 10_860.5,
                cheat: false,
                stations: vec![
                    StationLoad {
                        weapon: "AIM9M.JT".into(),
                        count: 2,
                        quantity: 2,
                    },
                    StationLoad {
                        weapon: "M61.JT".into(),
                        count: 1,
                        quantity: 578,
                    },
                ],
            },
            roster: roster(),
            destroyed: vec![1_000_001, 1_000_020],
        })),
        Message::Roster(roster()),
        Message::Names(Names {
            first: 12,
            names: vec!["AIM120.JT".into(), "FOX3".into()],
        }),
        Message::Notice("Mission restarts in 30 seconds".into()),
        Message::Leave,
        Message::Debrief(Box::new(Debrief {
            success: true,
            objectives: vec![
                DebriefObjective::Destroy {
                    destroyed: 2,
                    total: 3,
                },
                DebriefObjective::Protect {
                    protected: 1,
                    total: 1,
                },
            ],
            elapsed_seconds: 912,
            player: pilot(2),
            wingman: Some(pilot(0)),
        })),
        Message::MissionEnded(MissionEnded {
            reason: EndReason::TimeLimit,
            next_in_seconds: Some(30),
        }),
    ]
}
