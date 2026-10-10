//! Fixed sample contents of every section and message, shared by the golden,
//! round-trip and fuzz tests.

use super::chat::{ChatFrom, ChatLine, ChatSend, Quick, Receiver, Standing};
use super::entity::{
    AircraftState, DamageState, DebrisState, Devices, EngineState, Entity, EntityKey, EntityKind,
    EntityState, Motion, PilotState, ProjectileState, RotorState, Status,
};
use super::events::{EventsSection, LinkEvent, Rumble, SectionEvent, WireEvent};
use super::inputs::{Command, InputFrame, InputsSection, NumberedCommand};
use super::messages::{
    Build, Content, ContentGaps, ContentItem, ContentRefused, Debrief, DebriefObjective,
    DebriefPilot, EndReason, Gap, GapPlayer, Goodbye, Importer, ItemKind, Kick, Loadout,
    LobbyPhase, LobbyPlayer, LobbySlot, LobbyState, Lock, Message, Mission, MissionEnded, Names,
    Observe, ObserverFlight, Observing, PasswordChange, PilotStatus, PlayerScore, ResultRow,
    ResultStatus, Results, Revival, Roster, RosterPilot, RosterPlane, Scores, Seated, SetReady,
    SettingsChange, Shots, SideScore, Slot, SlotLock, SlotRequest, Spawned, StandbyMark, StartRule,
    Subject, TakePlane, Winner,
};
use super::migration::{
    self, Backlog, BacklogCommand, BacklogTick, CandidateReport, CheckResult, HostMoving,
    Processor, ReachPeers, ReachReport, ReachResult, ReachTarget, ReachTest, Reached, Resume,
    Resumed, ResumedFlight, StandbyState, StandbyStatus, Succession, Successor, TakenOver,
    TokenGrant, UploadTest,
};
use super::names::NameIndex;
use super::priority::Relevance;
use super::snapshot::{EntitySender, SnapshotHeader};
use super::{Path, Platform};
use crate::journal;
use crate::settings::{Fight, KillOwner, Respawn, ScoreTally};
use tore_formats::aircraft::AircraftId;
use tore_input::pilot::{LiftCommand, NozzlePreset, StabilityLevel, TrimAxis};
use tore_net::master::{Candidate, CandidateKind, MappingType};
use tore_sim::acoustics;
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::ai::wing::{Formation, PlayerBreak, PlayerOrder};
use tore_sim::combat::blast::MarkKind;
use tore_sim::combat::ledger::Tally;
use tore_sim::combat::live::{self, DamageSection, EffectKind};
use tore_sim::flight::{PilotCommand, Switch};
use tore_sim::sensors::{Channel, Controls};
use tore_sim::{airport, ejection, wreck};
use tore_world::comms::{Net, Route};
use tore_world::datalink::ClearReason;
use tore_world::mission::{LoadoutSpec, StationLoad};
use tore_world::resources::{Manifest, ManifestEntry};
use tore_world::seats::SeatCommand;
use tore_world::seats::SeatInput;
use tore_world::world::replies::Reply;
use tore_world::world::revive::Spawn;
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
        // Protocol 15: the sort (wing order 13) and the battle net (23).
        Command::Seat(S::WingOrder(PlayerOrder::Sort)),
        Command::Seat(S::BattleNet),
        Command::Seat(S::WingFormationCycle),
        Command::Seat(S::TriggerKey {
            down: true,
            repeat: false,
            blocked: true,
        }),
        // Protocol 8: every wing reply.
        Command::Seat(S::WingReply(Reply::Engaging)),
        Command::Seat(S::WingReply(Reply::Winchester)),
        Command::Seat(S::WingReply(Reply::BingoFuel)),
        Command::Seat(S::WingReply(Reply::NeedHelp)),
        Command::Pilot(PilotCommand::Eject),
        Command::Pilot(PilotCommand::Toggle(Switch::Gear)),
        Command::Pilot(PilotCommand::Set(Switch::WaypointAutopilot, true)),
        Command::Pilot(PilotCommand::Throttle(0.5)),
        Command::Pilot(PilotCommand::AdjustThrottle(-0.05)),
        Command::Pilot(PilotCommand::SetAxis(
            tore_input::pilot::FlightAxis::Collective,
            0.6,
        )),
        Command::Pilot(PilotCommand::AdjustAxis(
            tore_input::pilot::FlightAxis::Conversion,
            -0.1,
        )),
        Command::Pilot(PilotCommand::NeutralVector),
        Command::Seat(S::Combat(live::Command::NextGunGroup)),
        Command::Seat(S::Combat(live::Command::ToggleGunGroup)),
        // Protocol 19: hover hold and every powered-lift sub-code.
        Command::Pilot(PilotCommand::Toggle(Switch::HoverHold)),
        Command::Pilot(PilotCommand::Lift(LiftCommand::SetStability(
            StabilityLevel::Attitude,
        ))),
        Command::Pilot(PilotCommand::Lift(LiftCommand::CycleStability)),
        Command::Pilot(PilotCommand::Lift(LiftCommand::TrimSet)),
        Command::Pilot(PilotCommand::Lift(LiftCommand::TrimAdjust(
            TrimAxis::Roll,
            -0.02,
        ))),
        Command::Pilot(PilotCommand::Lift(LiftCommand::TrimCentre)),
        Command::Pilot(PilotCommand::Lift(LiftCommand::NozzleStep { down: true })),
        Command::Pilot(PilotCommand::Lift(LiftCommand::NozzlePreset(
            NozzlePreset::Vertical,
        ))),
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
        powered_lift: super::inputs::PoweredLiftInput {
            rates: [1, -2, 3, -4],
            positions: [Some(12345), Some(-23456), Some(0), Some(32767)],
        },
        sight: [5, -5],
        sight_zoom: 3,
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
    // The gunsight slewing at full deflection, zoomed in all the way, then
    // idle (protocol 21).
    next.sight = [-127, 127];
    next.sight_zoom = 6;
    frames.push(next);
    next.sight = [0, 0];
    next.sight_zoom = 0;
    frames.push(next);
    InputsSection {
        flight: 3,
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
                    lift_levels: [12, 127, 254],
                    vector_yaw: -45,
                    gun_aim: [-16_384, 0, -17_920, 2_560, -20_480, -7_680],
                    gun_group: 5,
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
                rotor: None,
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
                rotor: None,
            }),
        },
        // A rotorcraft: its rotor speed and disk tilts (slice P7b).
        Entity {
            id: 10,
            state: EntityState::Aircraft(AircraftState {
                aircraft: Some(AircraftId::Ch47),
                motion: motion(10),
                attitude: [4_000, 300, 65_200],
                devices: None,
                engine: EngineState {
                    lit: true,
                    ..EngineState::default()
                },
                damage: DamageState {
                    hp: 200,
                    initial_hp: 200,
                    ..DamageState::default()
                },
                status: Status {
                    airborne: true,
                    crashed: false,
                    wreck: None,
                },
                rotor: Some(RotorState {
                    speed: 987,
                    tilt: [[12, -3], [-127, 127]],
                }),
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
        flight: 3,
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
            net: Net::Wing,
            label: "VIPER 2".into(),
            text: "Fox two!".into(),
            stems: vec![NameIndex(1), NameIndex(4095)],
        },
        // Protocol 15: a call heard on the battle net.
        WireEvent::Radio {
            route: Route::Radio,
            important: false,
            net: Net::Battle,
            label: "Net Blue one".into(),
            text: "Blue, Contact, bandit".into(),
            stems: vec![NameIndex(9)],
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
        // Protocol 15: every data link change.
        WireEvent::Link(LinkEvent::Member {
            plane: 2,
            radar: false,
        }),
        WireEvent::Link(LinkEvent::Lock {
            plane: 1,
            target: 9,
        }),
        WireEvent::Link(LinkEvent::Unlock {
            plane: 1,
            target: 9,
        }),
        WireEvent::Link(LinkEvent::Assign {
            plane: 2,
            target: 12,
            by: 0,
            order: PlayerOrder::Sort,
        }),
        WireEvent::Link(LinkEvent::Clear {
            plane: 2,
            target: 12,
            why: ClearReason::TargetLost,
        }),
        WireEvent::Link(LinkEvent::Acknowledge {
            plane: 2,
            target: 12,
        }),
        WireEvent::Link(LinkEvent::SortWarning {
            plane: 0,
            other: 1,
            target: 70_000,
        }),
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
            number: 7,
        }),
        Message::ContentRefused(ContentRefused {
            mission: 7,
            names: vec!["UKR.T2".into(), "F18.PT".into()],
            reason: "Your game data differs".into(),
            flight: false,
        }),
        Message::TakePlane(TakePlane {
            mission: 7,
            plane: Some(4),
        }),
        Message::TakePlane(TakePlane {
            mission: 7,
            plane: None,
        }),
        Message::SeatRefused("plane taken".into()),
        Message::Seated(Box::new(Seated {
            flight: 3,
            seat: 2,
            plane: 3,
            tick: 7_200,
            exact,
            loadout: LoadoutSpec {
                tanks: None,
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
            flight: 3,
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
        Message::Slot(Slot {
            mission: 7,
            request: SlotRequest::Take(2),
        }),
        Message::Slot(Slot {
            mission: 7,
            request: SlotRequest::Any,
        }),
        Message::Slot(Slot {
            mission: 7,
            request: SlotRequest::Leave,
        }),
        // The lobby pass (protocol 20): a side's first free slot.
        Message::Slot(Slot {
            mission: 7,
            request: SlotRequest::Side(Side::Friendly),
        }),
        Message::Slot(Slot {
            mission: 7,
            request: SlotRequest::Side(Side::Enemy),
        }),
        Message::Loadout(Box::new(Loadout {
            mission: 7,
            plane: 2,
            loadout: Some(LoadoutSpec {
                tanks: Some(vec![tore_world::mission::TankLoad {
                    hardpoint: 3,
                    tank: "F16_370.GAS".into(),
                    quantity: 1,
                }]),
                fuel_lbs: 6_000.,
                cheat: false,
                stations: vec![StationLoad {
                    weapon: "M61.JT".into(),
                    count: 578,
                    quantity: 300,
                }],
            }),
        })),
        Message::Loadout(Box::new(Loadout {
            mission: 7,
            plane: 2,
            loadout: None,
        })),
        Message::SetReady(SetReady {
            mission: 7,
            ready: true,
        }),
        Message::ChangeMission("tore-mission 1\ntheater UKR\n".into()),
        Message::Start,
        Message::Kick(Kick {
            player: 2,
            reason: "AFK".into(),
        }),
        Message::EndMission,
        Message::Lobby(Box::new(lobby())),
        Message::Refused {
            request: super::messages::kind::START,
            reason: "Not ready: Viper.".into(),
        },
        Message::Goodbye(Goodbye::Kicked("AFK".into())),
        Message::Goodbye(Goodbye::HostLeft),
        Message::FlightLoadouts(super::messages::FlightLoadouts {
            loadouts: vec![(
                2,
                LoadoutSpec {
                    tanks: None,
                    fuel_lbs: 6_000.,
                    cheat: false,
                    stations: vec![StationLoad {
                        weapon: "AIM9X.JT".into(),
                        count: 2,
                        quantity: 2,
                    }],
                },
            )],
            manifest: Manifest {
                entries: vec![ManifestEntry {
                    name: "AIM9X.JT".into(),
                    hash: Some(0x1234_5678_9ABC_DEF0),
                }],
            },
        }),
        Message::ChatSend(ChatSend::typed(
            Receiver::Friendlies,
            "Break left, bandit high",
        )),
        Message::ChatSend(ChatSend {
            receiver: Receiver::Target,
            text: "Missile inbound! Break!".into(),
            quick: Some(Quick {
                number: 11,
                sound: Some("^MISSBRK.5K".into()),
            }),
        }),
        Message::ChatLine(ChatLine {
            from: ChatFrom::Player {
                callsign: "Cobra".into(),
                standing: Standing::Own,
                you: false,
            },
            receiver: Receiver::Wing,
            text: "Engaging".into(),
            sound: None,
        }),
        Message::ChatLine(ChatLine {
            from: ChatFrom::Player {
                callsign: "Hawk".into(),
                standing: Standing::Enemy,
                you: false,
            },
            receiver: Receiver::Target,
            text: "Eat hot lead".into(),
            sound: Some("^HOTLEAD.5K".into()),
        }),
        Message::ChatLine(ChatLine::system("No one hears you.")),
    ]
    .into_iter()
    .chain(phase_two_messages())
    .chain(compatibility_messages())
    .chain(migration_messages())
    .collect()
}

/// Scores with a player of each side, one with no side yet, and a winner.
pub fn scores() -> Scores {
    Scores {
        tally: ScoreTally::Ratio,
        fight: Fight::Sides,
        seconds_left: Some(312),
        kill_limit: 5,
        kill_owner: KillOwner::Side,
        players: vec![
            PlayerScore {
                id: 0,
                callsign: "Viper".into(),
                side: Some(Side::Friendly),
                kills: 3,
                losses: 1,
                damage: 2_750,
            },
            PlayerScore {
                id: 3,
                callsign: "Hawk".into(),
                side: Some(Side::Enemy),
                kills: 2,
                losses: 2,
                damage: 1_400,
            },
            PlayerScore {
                id: 4,
                callsign: "Lynx".into(),
                side: None,
                kills: 0,
                losses: 0,
                damage: 0,
            },
        ],
        sides: [
            SideScore {
                kills: 3,
                losses: 1,
                damage: 2_750,
            },
            SideScore {
                kills: 2,
                losses: 2,
                damage: 1_400,
            },
        ],
        winner: Winner::Side(Side::Friendly),
    }
}

/// One message of every phase 2 kind (protocol 8), each variant of its
/// choices at least once.
pub fn phase_two_messages() -> Vec<Message> {
    let row = |plane: u32, status: ResultStatus, callsign: Option<&str>| ResultRow {
        plane,
        wing: WingId::new(Side::Enemy, 1).unwrap(),
        member: 2,
        aircraft: AircraftId::Su35,
        callsign: callsign.map(str::to_owned),
        status,
        damage: if status == ResultStatus::Alive {
            125
        } else {
            1_000
        },
        aircraft_kills: plane % 3,
        other_kills: 1,
        friendly_fire: 0,
        air_to_air: Shots {
            launched: 4,
            hit: 2,
        },
        gun: Shots {
            launched: 320,
            hit: 41,
        },
        air_to_ground: Shots::default(),
    };
    let mut free_for_all = scores();
    free_for_all.fight = Fight::FreeForAll;
    free_for_all.tally = ScoreTally::Damage;
    free_for_all.kill_owner = KillOwner::Player;
    free_for_all.seconds_left = None;
    free_for_all.winner = Winner::Player(3);
    let mut draw = scores();
    draw.tally = ScoreTally::Kills;
    draw.kill_owner = KillOwner::Total;
    draw.kill_limit = 0;
    draw.winner = Winner::Draw;
    let mut none_yet = scores();
    none_yet.winner = Winner::NoneYet;
    vec![
        Message::PassCrown(3),
        Message::Settings(Box::new(SettingsChange {
            values: vec![(1, 1), (10, 255), (16, 600)],
            name: Some("Viper's PvP".into()),
            password: Some(PasswordChange::Set("hunter2".into())),
        })),
        Message::Settings(Box::new(SettingsChange {
            values: Vec::new(),
            name: None,
            password: Some(PasswordChange::Clear),
        })),
        Message::Settings(Box::default()),
        Message::SlotLock(Box::new(SlotLock {
            mission: 7,
            plane: 4,
            lock: Lock::Reserved("Hawk".into()),
        })),
        Message::SlotLock(Box::new(SlotLock {
            mission: 7,
            plane: 5,
            lock: Lock::Closed,
        })),
        Message::SlotLock(Box::new(SlotLock {
            mission: 7,
            plane: 5,
            lock: Lock::Open,
        })),
        Message::Revive { mission: 7 },
        Message::Revival(Box::new(Revival {
            rule: Respawn::Revive,
            lives: Some(2),
            wait_seconds: 45,
            why: None,
        })),
        Message::Revival(Box::new(Revival {
            rule: Respawn::AiSlot,
            lives: None,
            wait_seconds: 0,
            why: Some("Waiting for room for another aircraft.".into()),
        })),
        Message::Revival(Box::new(Revival {
            rule: Respawn::None,
            lives: Some(0),
            wait_seconds: 0,
            why: Some("No lives left.".into()),
        })),
        Message::Spawned(Box::new(Spawned {
            plane: 40,
            tick: 86_400,
            wing: WingId::new(Side::Enemy, 2).unwrap(),
            member: 4,
            aircraft: AircraftId::Su35,
            spawn: Spawn {
                position: [-60_761.25, 20_000., 121_522.5],
                heading_rad: -2.356_194_490_192_345,
                speed_fps: 760.5,
                loadout: LoadoutSpec {
                    tanks: None,
                    fuel_lbs: 20_700.,
                    cheat: false,
                    stations: vec![StationLoad {
                        weapon: "GSH301.JT".into(),
                        count: 1,
                        quantity: 75,
                    }],
                },
            },
        })),
        Message::Scores(Box::new(scores())),
        Message::Scores(Box::new(free_for_all)),
        Message::Scores(Box::new(draw)),
        Message::Scores(Box::new(none_yet)),
        Message::Results(Box::new(Results {
            reason: EndReason::KillLimit,
            rows: vec![
                row(0, ResultStatus::Alive, Some("Viper")),
                row(7, ResultStatus::Ejected, None),
                row(9, ResultStatus::Dead, Some("Hawk")),
                row(40, ResultStatus::Retired, Some("Hawk")),
            ],
            scores: Some(scores()),
        })),
        Message::Results(Box::new(Results {
            reason: EndReason::TimeLimit,
            rows: Vec::new(),
            scores: None,
        })),
        Message::Observe(Observe::Stop),
        Message::Observe(Observe::Watch(Subject::None)),
        Message::Observe(Observe::Watch(Subject::Aircraft(16))),
        Message::Observe(Observe::Watch(Subject::Point([-120_000, 15_000, 98_765]))),
        Message::Observing(Box::new(Observing::Started(ObserverFlight {
            flight: 4,
            delay_seconds: 30,
            tick: 72_000,
            roster: roster(),
            destroyed: vec![1_000_001],
        }))),
        Message::Observing(Box::new(Observing::Ended)),
        Message::Away,
        Message::Back,
        Message::MissionEnded(MissionEnded {
            reason: EndReason::KillLimit,
            next_in_seconds: None,
        }),
    ]
}

/// A lobby with a King, a player flying, one unable, one more waiting, and
/// three slots: the four players cover every platform, four of the six
/// connection paths (protocol 12) and every Fighters Anthology build
/// (protocol 10), the slots every lock and every reservation, the players
/// every observing, away and standby mark (protocol 13), and the settings
/// are PvP's defaults (protocol 8).
pub fn lobby() -> LobbyState {
    LobbyState {
        name: "Viper's game".into(),
        summary: "UKR, clear, airborne at 20000 ft: F/A-18D Hornet x4 against nobody".into(),
        mission: 7,
        phase: LobbyPhase::Flying,
        start: StartRule::King,
        king: Some(0),
        host: Some(0),
        you: 1,
        players: vec![
            LobbyPlayer {
                id: 0,
                callsign: "Viper".into(),
                slot: Some(0),
                ready: true,
                loadout: true,
                flying: true,
                observing: false,
                away: false,
                unable: None,
                platform: Platform::Linux,
                path: Path::LocalNetwork,
                build: Build::V102F,
                standby: StandbyMark::None,
            },
            LobbyPlayer {
                id: 1,
                callsign: "Cobra".into(),
                slot: Some(1),
                ready: false,
                loadout: false,
                flying: false,
                observing: true,
                away: false,
                unable: None,
                platform: Platform::Windows,
                path: Path::Punched,
                build: Build::V10,
                standby: StandbyMark::First,
            },
            LobbyPlayer {
                id: 3,
                callsign: "Hawk".into(),
                slot: None,
                ready: false,
                loadout: false,
                flying: false,
                observing: false,
                away: true,
                unable: Some("Your game data differs".into()),
                platform: Platform::MacOs,
                path: Path::Relay,
                build: Build::Unknown,
                standby: StandbyMark::Second,
            },
            LobbyPlayer {
                id: 4,
                callsign: "Lynx".into(),
                slot: None,
                ready: false,
                loadout: false,
                flying: false,
                observing: true,
                away: true,
                unable: None,
                platform: Platform::Unknown,
                path: Path::Ipv6,
                build: Build::V102F,
                standby: StandbyMark::None,
            },
        ],
        slots: vec![
            LobbySlot {
                plane: 0,
                wing: WingId::new(Side::Friendly, 0).unwrap(),
                member: 0,
                aircraft: AircraftId::F18,
                holder: Some(0),
                lock: Lock::Open,
                reserved: None,
            },
            LobbySlot {
                plane: 1,
                wing: WingId::new(Side::Friendly, 0).unwrap(),
                member: 1,
                aircraft: AircraftId::F18,
                holder: Some(1),
                lock: Lock::Reserved("Cobra".into()),
                reserved: Some("Cobra".into()),
            },
            LobbySlot {
                plane: 4,
                wing: WingId::new(Side::Friendly, 1).unwrap(),
                member: 0,
                aircraft: AircraftId::F14,
                holder: None,
                lock: Lock::Closed,
                reserved: Some("Hawk".into()),
            },
        ],
        // The idle time stays the 10 seconds the sample was first made with:
        // the registry's default moved to 5 minutes (slice F2-O4), which is
        // a value and not a change of the wire's bytes.
        settings: crate::settings::Store::defaults(crate::settings::Mode::Pvp)
            .lobby_list()
            .into_iter()
            .map(|(n, v)| {
                (
                    n,
                    if n == crate::settings::number::IDLE_AI {
                        10
                    } else {
                        v
                    },
                )
            })
            .collect(),
        mission_locked: true,
    }
}

/// An AC-130 gunsight pinned on a ground point, every value on the wire's
/// grid (protocol 21).
pub fn gunsight() -> tore_world::readout::GunsightReadout {
    use std::f64::consts::TAU;
    use tore_sim::combat::gunship::{Notice, Sight, SightNotice};
    use tore_sim::combat::gunship_impact::Impact;
    use tore_sim::combat::live::Readiness;
    let pin = [52_000.5, 200., -31_000.25];
    tore_world::readout::GunsightReadout {
        sight: Sight::Pinned(pin),
        look: [-TAU / 4., -TAU / 16.],
        returning: false,
        aim: Some(pin),
        impacts: [
            Some(Impact::Ground {
                point: [52_010., 195.5, -30_990.125],
                seconds: 4.5,
                range_ft: 7_000.,
            }),
            None,
            Some(Impact::Spent {
                point: [51_000., 900., -30_000.],
                seconds: 10.,
                range_ft: 13_000.,
            }),
        ],
        impacts_tick: 400,
        status: [Readiness::Ready, Readiness::TerrainMask, Readiness::GunArc],
        notice: Some(SightNotice {
            notice: Notice::DropToSlew,
            tick: 390,
        }),
        zoom: 4,
    }
}

/// A cockpit readout with every group and list filled, built by hand.
pub fn readout() -> tore_world::readout::CockpitReadout {
    use tore_sim::combat::live::{Readiness, SeekerTone};
    use tore_sim::combat::missiles::{FiringBand, LaunchMode, seeker};
    use tore_sim::combat::threats::{EvidenceSource, GuidanceClass, ThreatRecord};
    use tore_sim::sensors::passive::{Emitter, Symbol};
    use tore_sim::sensors::{Contact, Plot, Strobe, Support};
    use tore_world::readout::*;
    use tore_world::snapshot::Damage;
    use tore_world::target_window::{Pilot, TargetBrief, TargetObjective};
    let contact = |id: u32, x: f64| Contact {
        id,
        channel: Channel::Radar,
        bearing_rad: 0.,
        elevation_rad: 0.,
        distance_ft: 0.,
        position: [x, 12_000., -x * 2.],
        velocity: [800., -10.25, 3.5],
        track_eligible: id.is_multiple_of(2),
        destroyed: false,
    };
    let observation = seeker::Observation {
        id: 7,
        position: [1_000.5, 9_000., -2_000.25],
        velocity: [600., 1., -2.],
        quality: 0.75,
        off_axis: 0.0625,
        range: 12_345.,
    };
    let row = TargetRow {
        id: 7,
        aircraft: Some(AircraftId::Su27),
        position: [1_000.5, 9_000., -2_000.25],
        velocity: [600., 1., -2.],
        damage: Damage {
            hp: 80,
            initial_hp: 100,
            sections: [0, 5, 0, 15, 0, 0],
            structural: None,
        },
    };
    CockpitReadout {
        plane: 3,
        tick: 401,
        stores: Stores {
            selected: 2,
            armed: true,
            launch_mode: LaunchMode::Boresight,
            ammo: vec![578, 2, 0x8001, 4],
            loaded: 0b1111,
            gun_aim: [-0.5, 0., -0.4, 0.2, -0.6, -0.3],
            gun_group: 5,
        },
        gunsight: Some(gunsight()),
        seeker: SeekerReadout {
            status: seeker::Status::Locked,
            target: Some(7),
            observation: Some(observation),
            tone: Some(SeekerTone {
                strength: 0.5,
                ground: false,
                radar: false,
                locked: true,
            }),
        },
        estimates: Estimates {
            readiness: Readiness::Ready,
            guidance_available: true,
            can_lock: true,
            observation: Some(observation),
            max_range: Some(48_000.),
            band: Some(FiringBand {
                minimum: 3_000.,
                maximum: 30_000.,
            }),
            in_range: true,
            hit_percent: 72,
            solution_seconds: Some(11.5),
        },
        targets: Targets {
            designated: Some(7),
            display: Some(row.clone()),
            view: Some(row),
        },
        sensors: SensorReadout {
            tick: 399,
            selected: Some(7),
            acquired: None,
            selected_support: Some(Support::Tracked),
            available: [true, true, true],
            operating: [true, false, true],
            radar_track_nmi: Some(20.),
            contacts: vec![contact(7, 1_000.), contact(12, -40_000.)],
            plots: vec![Plot {
                id: 30,
                channel: Channel::Infrared,
                bearing_rad: 0.5,
                elevation_rad: -0.125,
                distance_ft: 60_000.,
                position: [5., 6., 7.],
                age: 120,
            }],
            strobes: vec![Strobe::presented(44, -1., 0.25, 0.5, 0.03125, 0.125)],
            trails: vec![Trail {
                id: 7,
                start: 0,
                len: 3,
            }],
            trail_points: vec![[0., 0., 0.], [10., 0., 5.], [20., 1., 10.]],
        },
        visual: vec![contact(12, -40_000.)],
        map: vec![MapRow {
            contact: contact(12, -40_000.),
            identified: true,
            airborne: true,
            aircraft: Some(AircraftId::Mig29),
        }],
        rwr: RwrReadout {
            emitters: vec![Emitter {
                id: 12,
                bearing_rad: 3.,
                distance_nmi: Some(8.25),
                symbol: Symbol::Aircraft,
                received: 0.5,
            }],
            missiles: vec![ThreatRecord {
                missile_id: 70_001,
                source: EvidenceSource::Visual,
                observed_tick: 380,
                bearing_deg: -45.5,
                position: Some([1., 2., 3.]),
                velocity: Some([-1_500., 0., 2.]),
                guidance_class: Some(GuidanceClass::Infrared),
                targeting_receiver: true,
                was_targeting_receiver: true,
                stale: false,
                radar_bearing_deg: None,
            }],
            inbound: vec![InboundMissile {
                id: 70_001,
                position: [1., 2., 3.],
                seeker_class: 2,
                aim120: false,
            }],
            locks: vec![3],
        },
        damage: DamageReadout {
            hp: 90,
            damage: 10,
            subsystem_counts: std::array::from_fn(|i| (i % 3) as u8),
            last_subsystem: Some(4),
            radar_failed: false,
            visual_failed: false,
            infrared_failed: true,
            rwr_failed: false,
            ecm_failed: false,
            shots: 300,
            hits: 12,
            kills: 1,
        },
        countermeasures: Countermeasures {
            chaff: 30,
            flares: 28,
        },
        airport: AirportReadout {
            nav_mode: false,
            service: None,
        },
        target_window: Some(TargetBrief {
            id: 7,
            objective: Some(TargetObjective::Destroy),
            pilot: Pilot::Ai {
                activity: tore_sim::ai::controller::Activity::Attacking,
                skill: 3,
                aims_at_viewer: true,
            },
        }),
        music: MusicReadout {
            designated_enemy: Some((7, [1_000.5, 9_000., -2_000.25])),
            aiming: vec![7, 9],
            succeeded: false,
            home: false,
        },
        // Protocol 15: the data link's share (slice G7).
        link: LinkReadout {
            radar: true,
            assigned: Some(LinkAssigned {
                target: 7,
                by: 0,
                acknowledged: false,
            }),
            tracks: vec![
                LinkTrack {
                    target: 7,
                    position: [1_000.4, 9_000., -2_000.],
                    velocity: [800., 0., -12.25],
                    source: tore_world::datalink::TrackSource::Own,
                },
                LinkTrack {
                    target: 9,
                    position: [40_000., 12_000., 60_000.],
                    velocity: [-600., 4., 0.],
                    source: tore_world::datalink::TrackSource::Network,
                },
            ],
            marks: vec![
                LinkMark {
                    target: 7,
                    lockers: 0b10,
                    net_lock: None,
                    assigned_to: 0b100,
                },
                LinkMark {
                    target: 9,
                    lockers: 0,
                    net_lock: Some(MemberRef {
                        flight: 1,
                        member: 2,
                    }),
                    assigned_to: 0,
                },
            ],
            mates: vec![
                LinkMate {
                    plane: 1,
                    member: 1,
                    fuel: tore_world::datalink::Fuel::Bingo,
                    weapons: tore_world::datalink::Weapons::GunsOnly,
                    damage: tore_world::datalink::Damage::Light,
                },
                LinkMate {
                    plane: 2,
                    member: 2,
                    fuel: tore_world::datalink::Fuel::Normal,
                    weapons: tore_world::datalink::Weapons::Missiles,
                    damage: tore_world::datalink::Damage::None,
                },
            ],
        },
    }
}

/// A snapshot carrying [`readout`] in full, then one carrying it moved on
/// against the first, delivered.
pub fn readout_snapshots() -> (Vec<u8>, Vec<u8>) {
    let mut host = super::connection::HostConnection::new(4);
    let first = readout();
    let packet = host
        .snapshot_with_readout(&header(400), &[], Some(&first), 0)
        .unwrap();
    host.sent(1);
    host.delivered(1);
    let mut second = readout();
    second.tick += 4;
    second.sensors.contacts[0].position[0] += 3_210.;
    second.stores.ammo[0] -= 20;
    second.sensors.contacts.pop();
    // The gunsight slewing the pin along.
    if let Some(gunsight) = &mut second.gunsight {
        gunsight.look[0] += std::f64::consts::TAU / 1_024.;
        gunsight.aim = gunsight.aim.map(|a| [a[0] + 40., a[1], a[2]]);
        gunsight.sight = tore_sim::combat::gunship::Sight::Pinned(gunsight.aim.unwrap());
        gunsight.impacts_tick = 404;
    }
    // The link: a track moved on its publishing tick, the assignment locked.
    second.link.tracks[0].position[0] += 200.;
    if let Some(assigned) = &mut second.link.assigned {
        assigned.acknowledged = true;
    }
    let later = host
        .snapshot_with_readout(&header(404), &[], Some(&second), 0)
        .unwrap();
    (packet.snapshot, later.snapshot)
}

/// A content of every item kind, sorted as the wire wants it, from a 1.02F
/// import made by a known T.O.R.E (protocol 10).
pub fn content() -> Content {
    let item = |kind: ItemKind, key: &str, digest: u64| ContentItem {
        kind,
        key: key.into(),
        digest,
    };
    Content {
        build: Build::V102F,
        importer: Some(Importer {
            version: "0.1.4".into(),
            commit: "48d62dac".into(),
        }),
        items: vec![
            item(ItemKind::Aircraft, "F18.PT", 0x0123_4567_89AB_CDEF),
            item(ItemKind::Aircraft, "SU27.PT", 0xFEDC_BA98_7654_3210),
            item(ItemKind::Aircraft, "faxx", 1),
            item(ItemKind::Theater, "UKR", 0x8000_0000_0000_0000),
            item(ItemKind::Theater, "VIET", u64::MAX),
            item(ItemKind::Weapon, "AGM65G.JT", 0),
            item(ItemKind::Weapon, "AIM9X.JT", 0x1234_5678_9ABC_DEF0),
            item(ItemKind::Shared, "", 0xCBF2_9CE4_8422_2325),
        ],
    }
}

/// Content gaps of each shape: a player lacking an aircraft, two players
/// of whom one has a weapon with another digest, an item the host lacks
/// that names nobody, and the shared item (protocol 10).
pub fn content_gaps() -> ContentGaps {
    let lacks = |id: u8| GapPlayer { id, differs: false };
    ContentGaps {
        host_build: Build::V10,
        host_importer: Some(Importer {
            version: "0.1.3".into(),
            commit: "fb9c2ec".into(),
        }),
        gaps: vec![
            Gap {
                kind: ItemKind::Aircraft,
                key: "SU27.PT".into(),
                label: "Su-27 Flanker".into(),
                host_lacks: false,
                players: vec![lacks(3)],
            },
            Gap {
                kind: ItemKind::Theater,
                key: "VIET".into(),
                label: String::new(),
                host_lacks: true,
                players: Vec::new(),
            },
            Gap {
                kind: ItemKind::Weapon,
                key: "AGM65G.JT".into(),
                label: "AGM-65G".into(),
                host_lacks: false,
                players: vec![
                    lacks(1),
                    GapPlayer {
                        id: 4,
                        differs: true,
                    },
                ],
            },
            Gap {
                kind: ItemKind::Shared,
                key: String::new(),
                label: "shared flight data".into(),
                host_lacks: false,
                players: vec![GapPlayer {
                    id: 3,
                    differs: true,
                }],
            },
        ],
    }
}

/// One message of every stage L kind (protocol 10): a full Content, one of
/// an import that says nothing of itself, gaps of each shape, and the empty
/// gaps a lobby with none is sent once.
pub fn compatibility_messages() -> Vec<Message> {
    vec![
        Message::Content(Box::new(content())),
        Message::Content(Box::new(Content {
            build: Build::Unknown,
            importer: None,
            items: vec![ContentItem {
                kind: ItemKind::Shared,
                key: String::new(),
                digest: 42,
            }],
        })),
        Message::ContentGaps(Box::new(content_gaps())),
        Message::ContentGaps(Box::new(ContentGaps {
            host_build: Build::V102F,
            host_importer: None,
            gaps: Vec::new(),
        })),
    ]
}

// ----- Protocol 13: stage K, host migration and rejoin ------------------

fn address(text: &str) -> std::net::SocketAddr {
    text.parse().unwrap()
}

/// A Candidate report with every own candidate kind (protocol 13).
pub fn candidate_report() -> CandidateReport {
    CandidateReport {
        may_host: true,
        platform: Platform::Linux,
        processor: Processor::X86_64,
        candidates: vec![
            Candidate::new(CandidateKind::Local, address("192.168.1.20:26900")),
            Candidate::new(CandidateKind::Mapped, address("203.0.113.7:26900")),
            Candidate::new(CandidateKind::GlobalIpv6, address("[2001:db8::20]:26900")),
        ],
        mapping: MappingType::SamePort,
        cpu_micros: 1_850,
        cpu_mission: 7,
    }
}

/// A Backlog of a weaving stick, a trigger pull and two commands.
pub fn backlog() -> Backlog {
    let frames = inputs().frames;
    Backlog {
        flight: 9,
        first_tick: 72_000,
        ticks: frames
            .iter()
            .enumerate()
            .map(|(index, frame)| BacklogTick {
                frame: *frame,
                view_offset: 12 + index as u8 % 3,
                interpolation_delay: 10,
            })
            .collect(),
        commands: vec![
            BacklogCommand {
                offset: 0,
                command: Command::Seat(SeatCommand::ReleaseFlare),
            },
            BacklogCommand {
                offset: frames.len() as u32 - 1,
                command: Command::Pilot(PilotCommand::Toggle(Switch::Gear)),
            },
        ],
    }
}

/// One message of every stage K kind (protocol 13), and each kind's other
/// shapes: an empty succession, a player resuming who is not flying.
pub fn migration_messages() -> Vec<Message> {
    let records = standby_records();
    let mut writer = crate::journal::StreamWriter::new();
    let mut messages = vec![
        Message::Token(TokenGrant {
            token: tore_net::Token(0x0123_4567_89AB_CDEF_FEDC_BA98_7654_3210),
            life_seconds: migration::limits::TOKEN_LIFE_SECONDS,
        }),
        Message::Candidate(Box::new(candidate_report())),
        Message::Candidate(Box::new(CandidateReport {
            may_host: false,
            platform: Platform::Unknown,
            processor: Processor::Unknown,
            candidates: Vec::new(),
            mapping: MappingType::Unknown,
            cpu_micros: 0,
            cpu_mission: 0,
        })),
        Message::ReachTest(Box::new(ReachTest {
            test: 41,
            candidates: vec![
                ReachTarget {
                    player: 2,
                    addresses: vec![address("192.168.1.20:26900"), address("203.0.113.7:26900")],
                },
                ReachTarget {
                    player: 5,
                    addresses: vec![address("[2001:db8::5]:26900")],
                },
            ],
        })),
        Message::ReachPeers(Box::new(ReachPeers {
            test: 41,
            players: vec![ReachTarget {
                player: 0,
                addresses: vec![address("198.51.100.4:40000"), address("10.0.0.4:26900")],
            }],
        })),
        Message::ReachReport(Box::new(ReachReport {
            test: 41,
            results: vec![
                ReachResult {
                    player: 2,
                    reached: Some(Reached {
                        address: 1,
                        round_trip_ms: 48,
                    }),
                },
                ReachResult {
                    player: 5,
                    reached: None,
                },
            ],
        })),
        Message::UploadTest(UploadTest {
            test: 42,
            rate: 98_000,
            length_ms: 1_000,
        }),
        Message::Succession(Box::new(Succession {
            standbys: vec![
                Successor {
                    player: 2,
                    warm: true,
                    addresses: vec![
                        Candidate::new(CandidateKind::Seen, address("203.0.113.7:51000")),
                        Candidate::new(CandidateKind::Local, address("192.168.1.20:26900")),
                    ],
                },
                Successor {
                    player: 5,
                    warm: false,
                    addresses: vec![Candidate::new(
                        CandidateKind::GlobalIpv6,
                        address("[2001:db8::5]:26900"),
                    )],
                },
            ],
        })),
        Message::Succession(Box::default()),
    ];
    messages.extend(
        records
            .iter()
            .map(|record| Message::StandbyRecord(writer.encode(record).unwrap())),
    );
    messages.extend([
        Message::StandbyStatus(StandbyStatus {
            newest_tick: 72_010,
            state: StandbyState::Warm,
            step_micros: 2_400,
            check_tick: 72_000,
            check: CheckResult::Equal,
            needs_checkpoint: false,
        }),
        Message::StandbyStatus(StandbyStatus {
            state: StandbyState::Behind,
            check: CheckResult::Different,
            needs_checkpoint: true,
            ..StandbyStatus::default()
        }),
        Message::Resume(Resume {
            flight: 8,
            newest_tick: 72_030,
            mission: 7,
            mission_hash: 0xCBF2_9CE4_8422_2325,
            watching: false,
        }),
        Message::Resumed(Box::new(Resumed::Flying(ResumedFlight {
            flight: 9,
            seat: 2,
            plane: 4,
            tick: 72_000,
            last_command: 311,
            exact: vec![1, 2, 3, 255],
            destroyed: vec![1_000_001, 7],
        }))),
        Message::Resumed(Box::new(Resumed::NotFlying)),
        Message::Backlog(Box::new(backlog())),
        Message::HostMoving(HostMoving {
            standby: 2,
            last_tick: 72_000,
        }),
        Message::TakenOver(TakenOver {
            new_host: 2,
            tick: 72_000,
        }),
        Message::Release(4),
        Message::Rejoin(tore_net::Token(u128::MAX - 1)),
    ]);
    messages
}

/// A seat's input of the journal's sample tick.
fn journal_input(seat: u8, tick: u64, pitch: f64, commands: Vec<SeatCommand>) -> SeatInput {
    SeatInput {
        seat: tore_world::seats::SeatId(seat),
        tick,
        pilot: tore_sim::flight::PilotInput {
            pitch,
            roll: -0.25,
            throttle: Some(0.8),
            ..Default::default()
        },
        trigger: seat == 1,
        sensors: Controls::default(),
        sight: [0; 2],
        sight_zoom: 0,
        commands,
        view: Some(tore_world::seats::SeatView {
            tick: tick - 12,
            interpolation_delay: 10,
        }),
    }
}

/// One standby record of every type, in a stream's order: an Appoint, a
/// Flight, Ticks with every change, a take, a revival and two seats, a
/// checkpoint's begin and a chunk, a State, a Check, a Handover, Ended and
/// a Dismiss.
pub fn standby_records() -> Vec<journal::Record> {
    use journal::{
        Appoint, Change, Check, CheckpointBegin, CheckpointChunk, FlightRecord, Part, Record,
        StatePart, Tick, Ticks,
    };
    use tore_world::seats::{PlaneId, SeatId};
    use tore_world::world::MissionCommand;
    use tore_world::world::revive::RevivalWeapons;
    let mut first = Tick::new(1_200);
    first.changes = vec![
        Change::Scoring(true),
        Change::StoreCut {
            plane: PlaneId(6),
            weapons: RevivalWeapons::HalfGuns,
        },
    ];
    first.mission = vec![
        MissionCommand::Take {
            seat: SeatId(1),
            plane: PlaneId(6),
        },
        MissionCommand::Revive {
            seat: SeatId(2),
            spawn: Box::new(Spawn {
                position: [1.0e5, 2.5e4, 12_000.],
                heading_rad: 1.5,
                speed_fps: 700.,
                loadout: LoadoutSpec {
                    tanks: None,
                    fuel_lbs: 10_000.,
                    cheat: false,
                    stations: vec![StationLoad {
                        weapon: "AIM120.JT".into(),
                        count: 2,
                        quantity: 2,
                    }],
                },
            }),
        },
    ];
    first.push_input(
        journal_input(0, 1_200, 0.5, vec![SeatCommand::ReleaseChaff]),
        17,
    );
    first.push_input(journal_input(1, 1_200, 0., Vec::new()), 0);
    first.push_input(journal_input(2, 1_200, 0., Vec::new()), 0);
    let mut second = Tick::new(1_201);
    second.push_input(journal_input(0, 1_201, 0.5, Vec::new()), 17);
    second.push_input(
        journal_input(
            1,
            1_201,
            0.125,
            vec![SeatCommand::WingOrder(PlayerOrder::Break(
                PlayerBreak::Left,
            ))],
        ),
        1,
    );
    vec![
        Record::Appoint(Appoint {
            role: StandbyMark::First,
            warm: true,
            check_every: journal::CHECK_EVERY_TICKS,
            checkpoint_every: journal::CHECKPOINT_EVERY_TICKS,
            mission: 7,
        }),
        Record::Flight(FlightRecord {
            mission: 7,
            spec_hash: 0x1122_3344_5566_7788,
            identity: 0x99AA_BBCC_DDEE_FF00,
        }),
        Record::Ticks(Ticks {
            first: 1_200,
            ticks: vec![first, second],
        }),
        Record::CheckpointBegin(CheckpointBegin {
            tick: 1_202,
            length: 5_000,
            chunks: 2,
        }),
        Record::CheckpointChunk(CheckpointChunk {
            index: 1,
            bytes: vec![0xAB; 904],
        }),
        Record::State(StatePart {
            part: Part::Scores,
            tick: 1_201,
            bytes: vec![9, 8, 7],
        }),
        Record::Check(Check {
            tick: 1_200,
            hash: 0xFEED_FACE_CAFE_BEEF,
        }),
        Record::Handover { last_tick: 1_210 },
        Record::Ended(EndReason::HostLeft),
        Record::Dismiss,
    ]
}
