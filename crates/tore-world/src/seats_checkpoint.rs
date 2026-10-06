//! The coders of the roster: planes' pilots, seats, crews and wing recipients (the roster section).
//!
//! Stage H slice H2 (world shell). A restored roster replaces the fresh
//! world's, since handoffs change who flies which plane. The wing identity
//! types belong to `tore-sim`, which owns the trait, so a slot's wing is coded
//! field by field here.
//!
//! Stage K slice K0 adds the coders of what a seat hands a tick
//! ([`SeatInput`], [`SeatCommand`], [`SeatView`]), which the host's journal
//! carries to its standbys (docs/ARCHITECTURE.md, "The journal: one door
//! into the world"). Every field and variant is named with no catch-all, so a
//! field added to a seat's input or a variant added to a command fails to
//! compile here until the journal codes it. The command types that belong to
//! `tore-sim` (combat's commands, the wing orders, the tower's commands) are
//! coded variant by variant here for the same reason as the wing above.

use super::{Pilot, Plane, Roster, Seat, SeatCommand, SeatInput, SeatView, Slot};
use crate::seats::{PlaneId, SeatId};
use crate::world::AirportInput;
use crate::world::replies::Reply;
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::ai::wing::{PlayerApproach, PlayerBreak, PlayerOrder};
use tore_sim::airport::Command as TowerCommand;
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};
use tore_sim::combat::live::Command as Live;

impl Checkpoint for Slot {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Slot { wing, member } = self;
        let WingId { side, index } = wing;
        // Every variant, no catch-all, so a new side fails to compile here.
        s.writer().write_bool(match side {
            Side::Friendly => false,
            Side::Enemy => true,
        });
        index.save(s, None)?;
        member.save(s, None)
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let side = if l.reader().read_bool()? {
            Side::Enemy
        } else {
            Side::Friendly
        };
        let index = Checkpoint::load(l, None)?;
        let member = Checkpoint::load(l, None)?;
        Ok(Slot {
            wing: WingId { side, index },
            member,
        })
    }
}

impl Checkpoint for Pilot {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Pilot::Ai => s.writer().write_varint(0),
            Pilot::Human(seat) => {
                s.writer().write_varint(1);
                seat.save(s, None)?;
            }
            Pilot::Lost => s.writer().write_varint(2),
        }
        Ok(())
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        match l.reader().read_varint()? {
            0 => Ok(Pilot::Ai),
            1 => Ok(Pilot::Human(Checkpoint::load(l, None)?)),
            2 => Ok(Pilot::Lost),
            other => invalid(format!("a pilot has no variant {other}")),
        }
    }
}

tore_sim::checkpoint_struct!(Plane { id, slot, pilot });
tore_sim::checkpoint_struct!(Seat {
    id,
    plane,
    crew,
    wing_recipient,
});

impl Checkpoint for Roster {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Roster { planes, seats } = self;
        planes.save(s, None)?;
        seats.save(s, None)
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let planes: Vec<Plane> = Checkpoint::load(l, None)?;
        let seats: Vec<Seat> = Checkpoint::load(l, None)?;
        // The roster binary-searches planes by id and finds seats by id, so
        // damaged bytes that break the order or a cross reference are refused
        // here instead of misleading a later tick.
        if planes.windows(2).any(|pair| pair[0].id >= pair[1].id) {
            return invalid("the roster's planes are not in id order");
        }
        if seats.windows(2).any(|pair| pair[0].id >= pair[1].id) {
            return invalid("the roster's seats are not in seat order");
        }
        let plane_exists = |id: PlaneId| planes.binary_search_by_key(&id, |p| p.id).is_ok();
        let seat_exists = |id: SeatId| seats.binary_search_by_key(&id, |s| s.id).is_ok();
        if planes
            .iter()
            .any(|plane| matches!(plane.pilot, Pilot::Human(seat) if !seat_exists(seat)))
        {
            return invalid("a plane's pilot is a seat the roster does not hold");
        }
        if seats
            .iter()
            .any(|seat| seat.plane.is_some_and(|plane| !plane_exists(plane)))
        {
            return invalid("a seat flies a plane the roster does not hold");
        }
        Ok(Roster { planes, seats })
    }
}

// ----- Stage K: what a seat hands a tick (slice K0) ----------------------

tore_sim::checkpoint_struct!(SeatView {
    tick,
    interpolation_delay,
});

tore_sim::checkpoint_struct!(SeatInput {
    seat,
    tick,
    pilot,
    trigger,
    sensors,
    commands,
    view,
});

/// A number from a field-less choice, as a varint.
fn put(s: &mut Saver, number: u64) {
    s.writer().write_varint(number);
}

fn take(l: &mut Loader<'_>) -> Result<u64, CheckpointError> {
    Ok(l.reader().read_varint()?)
}

fn save_live(s: &mut Saver, command: Live) -> Result<(), CheckpointError> {
    match command {
        Live::NextWeapon => put(s, 0),
        Live::NextSelection => put(s, 1),
        Live::PreviousSelection => put(s, 2),
        Live::SelectNav => put(s, 3),
        Live::AdvanceFromEmpty => put(s, 4),
        Live::ToggleSeekerMode => put(s, 5),
        Live::CompatibilityWeapons => put(s, 6),
        Live::TargetHeat(heat) => {
            put(s, 7);
            heat.save(s, None)?;
        }
        Live::TargetDistance(feet) => {
            put(s, 8);
            feet.save(s, None)?;
        }
        Live::ClearRange => put(s, 9),
        Live::ToggleTargetRadar => put(s, 10),
        Live::Designate => put(s, 11),
        Live::DesignatePrevious => put(s, 12),
        Live::DesignateVisual => put(s, 13),
        Live::DesignateTarget(id) => {
            put(s, 14);
            id.save(s, None)?;
        }
        Live::ClearDesignation => put(s, 15),
        Live::ToggleArm => put(s, 16),
        Live::Jettison => put(s, 17),
        Live::ReplaceTarget => put(s, 18),
        Live::CycleClass => put(s, 19),
        Live::FailStation => put(s, 20),
        Live::DamagePlayer => put(s, 21),
        Live::Incoming => put(s, 22),
        Live::ToggleTargetJammer => put(s, 23),
        Live::ReleaseChaff => put(s, 24),
        Live::ReleaseFlare => put(s, 25),
    }
    Ok(())
}

fn load_live(l: &mut Loader<'_>) -> Result<Live, CheckpointError> {
    Ok(match take(l)? {
        0 => Live::NextWeapon,
        1 => Live::NextSelection,
        2 => Live::PreviousSelection,
        3 => Live::SelectNav,
        4 => Live::AdvanceFromEmpty,
        5 => Live::ToggleSeekerMode,
        6 => Live::CompatibilityWeapons,
        7 => Live::TargetHeat(Checkpoint::load(l, None)?),
        8 => Live::TargetDistance(Checkpoint::load(l, None)?),
        9 => Live::ClearRange,
        10 => Live::ToggleTargetRadar,
        11 => Live::Designate,
        12 => Live::DesignatePrevious,
        13 => Live::DesignateVisual,
        14 => Live::DesignateTarget(Checkpoint::load(l, None)?),
        15 => Live::ClearDesignation,
        16 => Live::ToggleArm,
        17 => Live::Jettison,
        18 => Live::ReplaceTarget,
        19 => Live::CycleClass,
        20 => Live::FailStation,
        21 => Live::DamagePlayer,
        22 => Live::Incoming,
        23 => Live::ToggleTargetJammer,
        24 => Live::ReleaseChaff,
        25 => Live::ReleaseFlare,
        other => return invalid(format!("a combat command has no variant {other}")),
    })
}

fn save_break(s: &mut Saver, way: PlayerBreak) {
    put(
        s,
        match way {
            PlayerBreak::Left => 0,
            PlayerBreak::Right => 1,
            PlayerBreak::Low => 2,
            PlayerBreak::High => 3,
            PlayerBreak::Straight => 4,
        },
    );
}

fn load_break(l: &mut Loader<'_>) -> Result<PlayerBreak, CheckpointError> {
    Ok(match take(l)? {
        0 => PlayerBreak::Left,
        1 => PlayerBreak::Right,
        2 => PlayerBreak::Low,
        3 => PlayerBreak::High,
        4 => PlayerBreak::Straight,
        other => return invalid(format!("a break has no variant {other}")),
    })
}

fn save_approach(s: &mut Saver, way: PlayerApproach) {
    put(
        s,
        match way {
            PlayerApproach::Left => 0,
            PlayerApproach::Right => 1,
            PlayerApproach::Low => 2,
            PlayerApproach::High => 3,
        },
    );
}

fn load_approach(l: &mut Loader<'_>) -> Result<PlayerApproach, CheckpointError> {
    Ok(match take(l)? {
        0 => PlayerApproach::Left,
        1 => PlayerApproach::Right,
        2 => PlayerApproach::Low,
        3 => PlayerApproach::High,
        other => return invalid(format!("an approach has no variant {other}")),
    })
}

fn save_order(s: &mut Saver, order: PlayerOrder) -> Result<(), CheckpointError> {
    match order {
        PlayerOrder::EngageMyTarget => put(s, 0),
        PlayerOrder::ProtectMe => put(s, 1),
        PlayerOrder::AttackOnContact => put(s, 2),
        PlayerOrder::EngageFromFormation => put(s, 3),
        PlayerOrder::Disengage => put(s, 4),
        PlayerOrder::Break(way) => {
            put(s, 5);
            save_break(s, way);
        }
        PlayerOrder::Approach(way) => {
            put(s, 6);
            save_approach(s, way);
        }
        PlayerOrder::Formation(formation) => {
            put(s, 7);
            formation.save(s, None)?;
        }
        PlayerOrder::Spacing => put(s, 8),
        PlayerOrder::Stacking => put(s, 9),
        PlayerOrder::ControlToggle => put(s, 10),
        PlayerOrder::BugOut => put(s, 11),
        PlayerOrder::LandAtSelected => put(s, 12),
    }
    Ok(())
}

fn load_order(l: &mut Loader<'_>) -> Result<PlayerOrder, CheckpointError> {
    Ok(match take(l)? {
        0 => PlayerOrder::EngageMyTarget,
        1 => PlayerOrder::ProtectMe,
        2 => PlayerOrder::AttackOnContact,
        3 => PlayerOrder::EngageFromFormation,
        4 => PlayerOrder::Disengage,
        5 => PlayerOrder::Break(load_break(l)?),
        6 => PlayerOrder::Approach(load_approach(l)?),
        7 => PlayerOrder::Formation(Checkpoint::load(l, None)?),
        8 => PlayerOrder::Spacing,
        9 => PlayerOrder::Stacking,
        10 => PlayerOrder::ControlToggle,
        11 => PlayerOrder::BugOut,
        12 => PlayerOrder::LandAtSelected,
        other => return invalid(format!("a wing order has no variant {other}")),
    })
}

fn save_airport(s: &mut Saver, input: AirportInput) -> Result<(), CheckpointError> {
    match input {
        AirportInput::NavMode => put(s, 0),
        AirportInput::Command(command) => match command {
            TowerCommand::SelectAirport(airport) => {
                put(s, 1);
                airport.save(s, None)?;
            }
            TowerCommand::RequestLanding => put(s, 2),
            TowerCommand::RepeatReply => put(s, 3),
            TowerCommand::CancelApproach => put(s, 4),
        },
    }
    Ok(())
}

fn load_airport(l: &mut Loader<'_>) -> Result<AirportInput, CheckpointError> {
    Ok(match take(l)? {
        0 => AirportInput::NavMode,
        1 => AirportInput::Command(TowerCommand::SelectAirport(Checkpoint::load(l, None)?)),
        2 => AirportInput::Command(TowerCommand::RequestLanding),
        3 => AirportInput::Command(TowerCommand::RepeatReply),
        4 => AirportInput::Command(TowerCommand::CancelApproach),
        other => return invalid(format!("an airport input has no variant {other}")),
    })
}

fn save_reply(s: &mut Saver, reply: Reply) {
    put(
        s,
        match reply {
            Reply::Engaging => 0,
            Reply::Winchester => 1,
            Reply::BingoFuel => 2,
            Reply::NeedHelp => 3,
        },
    );
}

fn load_reply(l: &mut Loader<'_>) -> Result<Reply, CheckpointError> {
    Ok(match take(l)? {
        0 => Reply::Engaging,
        1 => Reply::Winchester,
        2 => Reply::BingoFuel,
        3 => Reply::NeedHelp,
        other => return invalid(format!("a wing reply has no variant {other}")),
    })
}

/// A command by its variant's number and its fields, with no baseline: a
/// tick's commands are few, and most ticks have none.
impl Checkpoint for SeatCommand {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match *self {
            SeatCommand::CycleWeapon { forward } => {
                put(s, 0);
                forward.save(s, None)?;
            }
            SeatCommand::Airport(input) => {
                put(s, 1);
                save_airport(s, input)?;
            }
            SeatCommand::Combat(command) => {
                put(s, 2);
                save_live(s, command)?;
            }
            SeatCommand::Manual(command) => {
                put(s, 3);
                save_live(s, command)?;
            }
            SeatCommand::RangeReset => put(s, 4),
            SeatCommand::ReleaseChaff => put(s, 5),
            SeatCommand::ReleaseFlare => put(s, 6),
            SeatCommand::ReleaseTrigger => put(s, 7),
            SeatCommand::RadioSilence => put(s, 8),
            SeatCommand::WingRecipient(recipient) => {
                put(s, 9);
                recipient.save(s, None)?;
            }
            SeatCommand::WingOrder(order) => {
                put(s, 10);
                save_order(s, order)?;
            }
            SeatCommand::WingFormationCycle => put(s, 11),
            SeatCommand::TriggerKey {
                down,
                repeat,
                blocked,
            } => {
                put(s, 12);
                down.save(s, None)?;
                repeat.save(s, None)?;
                blocked.save(s, None)?;
            }
            SeatCommand::WingReply(reply) => {
                put(s, 13);
                save_reply(s, reply);
            }
        }
        Ok(())
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match take(l)? {
            0 => SeatCommand::CycleWeapon {
                forward: Checkpoint::load(l, None)?,
            },
            1 => SeatCommand::Airport(load_airport(l)?),
            2 => SeatCommand::Combat(load_live(l)?),
            3 => SeatCommand::Manual(load_live(l)?),
            4 => SeatCommand::RangeReset,
            5 => SeatCommand::ReleaseChaff,
            6 => SeatCommand::ReleaseFlare,
            7 => SeatCommand::ReleaseTrigger,
            8 => SeatCommand::RadioSilence,
            9 => SeatCommand::WingRecipient(Checkpoint::load(l, None)?),
            10 => SeatCommand::WingOrder(load_order(l)?),
            11 => SeatCommand::WingFormationCycle,
            12 => SeatCommand::TriggerKey {
                down: Checkpoint::load(l, None)?,
                repeat: Checkpoint::load(l, None)?,
                blocked: Checkpoint::load(l, None)?,
            },
            13 => SeatCommand::WingReply(load_reply(l)?),
            other => return invalid(format!("a seat command has no variant {other}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::Crew;
    use tore_sim::ai::wing::{PlayerApproach, PlayerBreak};
    use tore_sim::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    fn slot(side: Side, index: u8, member: u8) -> Slot {
        Slot {
            wing: WingId { side, index },
            member,
        }
    }

    /// A roster that has lived: humans in two seats (one with a crew and a
    /// wingman chosen), a waiting seat, a lost plane and AI planes of both
    /// sides.
    fn lived_in() -> Roster {
        let mut roster = Roster::with_humans(
            [
                (PlaneId(0), Slot::FRIENDLY_LEAD, SeatId(0), Some(Crew::Rio)),
                (
                    PlaneId(4),
                    slot(Side::Friendly, 1, 1),
                    SeatId(3),
                    Some(Crew::CoPilot),
                ),
            ],
            [
                (PlaneId(1), slot(Side::Friendly, 0, 1)),
                (PlaneId(2), slot(Side::Friendly, 0, 2)),
                (PlaneId(5), slot(Side::Enemy, 0, 0)),
                (PlaneId(6), slot(Side::Enemy, 2, 3)),
            ],
        );
        roster.set_wing_recipient(SeatId(0), Some(2));
        // A seat that gave its plane back waits; another plane was lost.
        roster.take_plane(SeatId(7), PlaneId(1), None).unwrap();
        roster.release_plane(SeatId(7));
        roster
            .planes
            .iter_mut()
            .find(|plane| plane.id == PlaneId(6))
            .unwrap()
            .pilot = Pilot::Lost;
        roster
    }

    #[test]
    fn a_lived_in_roster_round_trips_equal() {
        let roster = lived_in();
        assert!(roster.planes().iter().any(|p| p.pilot == Pilot::Lost));
        assert!(roster.seats().iter().any(|s| s.plane.is_none()));
        assert!(roster.seats().iter().any(|s| s.wing_recipient.is_some()));
        let copy = round_trip(&roster, &Models::default()).unwrap();
        assert_eq!(copy, roster);
        // The single-player and the open rosters too.
        for roster in [
            Roster::single_player(None, [(PlaneId(1), slot(Side::Enemy, 1, 0))]),
            Roster::open([(PlaneId(0), Slot::FRIENDLY_LEAD)]),
            Roster::open([]),
        ] {
            assert_eq!(round_trip(&roster, &Models::default()).unwrap(), roster);
        }
    }

    #[test]
    fn a_roster_that_breaks_the_worlds_rules_is_refused() {
        let models = Models::default();
        let refused = |roster: &Roster, why: &str| {
            let coded = to_bytes(roster, &models).unwrap();
            assert!(from_bytes::<Roster>(&coded, &models).is_err(), "{why}");
        };
        let mut roster = lived_in();
        roster.planes.swap(0, 1);
        refused(&roster, "planes out of order");
        let mut roster = lived_in();
        roster.seats.swap(0, 1);
        refused(&roster, "seats out of order");
        let mut roster = lived_in();
        roster.planes[1].pilot = Pilot::Human(SeatId(99));
        refused(&roster, "a pilot from no seat");
        let mut roster = lived_in();
        roster.seats[0].plane = Some(PlaneId(99));
        refused(&roster, "a seat in no plane");
    }

    #[test]
    fn damaged_roster_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        let coded = to_bytes(&lived_in(), &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            let _ = from_bytes::<Roster>(&shorter, &models);
        }
        for bit in 0..coded.body.len() * 8 {
            let mut flipped = coded.clone();
            flipped.body[bit / 8] ^= 1 << (bit % 8);
            let _ = from_bytes::<Roster>(&flipped, &models);
        }
    }

    /// Every seat command, each variant and each field's every choice once.
    fn every_command() -> Vec<SeatCommand> {
        use tore_sim::ai::wing::Formation;
        let live = [
            Live::NextWeapon,
            Live::NextSelection,
            Live::PreviousSelection,
            Live::SelectNav,
            Live::AdvanceFromEmpty,
            Live::ToggleSeekerMode,
            Live::CompatibilityWeapons,
            Live::TargetHeat(200),
            Live::TargetDistance(70_000),
            Live::ClearRange,
            Live::ToggleTargetRadar,
            Live::Designate,
            Live::DesignatePrevious,
            Live::DesignateVisual,
            Live::DesignateTarget(1_000_003),
            Live::ClearDesignation,
            Live::ToggleArm,
            Live::Jettison,
            Live::ReplaceTarget,
            Live::CycleClass,
            Live::FailStation,
            Live::DamagePlayer,
            Live::Incoming,
            Live::ToggleTargetJammer,
            Live::ReleaseChaff,
            Live::ReleaseFlare,
        ];
        let mut orders = vec![
            PlayerOrder::EngageMyTarget,
            PlayerOrder::ProtectMe,
            PlayerOrder::AttackOnContact,
            PlayerOrder::EngageFromFormation,
            PlayerOrder::Disengage,
            PlayerOrder::Spacing,
            PlayerOrder::Stacking,
            PlayerOrder::ControlToggle,
            PlayerOrder::BugOut,
            PlayerOrder::LandAtSelected,
        ];
        orders.extend(PlayerBreak::ALL.map(PlayerOrder::Break));
        orders.extend(PlayerApproach::ALL.map(PlayerOrder::Approach));
        orders.extend(Formation::ALL.map(PlayerOrder::Formation));
        let mut commands = vec![
            SeatCommand::CycleWeapon { forward: true },
            SeatCommand::CycleWeapon { forward: false },
            SeatCommand::Airport(AirportInput::NavMode),
            SeatCommand::Airport(AirportInput::Command(TowerCommand::SelectAirport(12))),
            SeatCommand::Airport(AirportInput::Command(TowerCommand::RequestLanding)),
            SeatCommand::Airport(AirportInput::Command(TowerCommand::RepeatReply)),
            SeatCommand::Airport(AirportInput::Command(TowerCommand::CancelApproach)),
            SeatCommand::RangeReset,
            SeatCommand::ReleaseChaff,
            SeatCommand::ReleaseFlare,
            SeatCommand::ReleaseTrigger,
            SeatCommand::RadioSilence,
            SeatCommand::WingRecipient(None),
            SeatCommand::WingRecipient(Some(3)),
            SeatCommand::WingFormationCycle,
        ];
        commands.extend(live.iter().map(|&c| SeatCommand::Combat(c)));
        commands.extend(live.iter().map(|&c| SeatCommand::Manual(c)));
        commands.extend(orders.into_iter().map(SeatCommand::WingOrder));
        for down in [false, true] {
            for repeat in [false, true] {
                for blocked in [false, true] {
                    commands.push(SeatCommand::TriggerKey {
                        down,
                        repeat,
                        blocked,
                    });
                }
            }
        }
        commands.extend(Reply::ALL.map(SeatCommand::WingReply));
        commands
    }

    /// A seat's input with every field away from its default.
    fn busy_input() -> SeatInput {
        use tore_input::{PilotCommand, Switch};
        SeatInput {
            seat: SeatId(5),
            tick: 7_201,
            pilot: tore_sim::flight::PilotInput {
                pitch: -0.25,
                roll: 0.5,
                yaw: 0.125,
                throttle_rate: -1.,
                throttle: Some(0.75),
                commands: vec![
                    PilotCommand::Toggle(Switch::Gear),
                    PilotCommand::Set(Switch::Burner, true),
                    PilotCommand::Eject,
                ],
            },
            trigger: true,
            sensors: tore_sim::sensors::Controls::default(),
            commands: every_command(),
            view: Some(SeatView {
                tick: 7_190,
                interpolation_delay: 11,
            }),
        }
    }

    #[test]
    fn every_seat_command_round_trips_alone_and_in_an_input() {
        let models = Models::default();
        let commands = every_command();
        assert!(commands.len() > 80, "{} commands", commands.len());
        for command in &commands {
            let copy = round_trip(command, &models).unwrap();
            assert_eq!(&copy, command);
        }
        let input = busy_input();
        let copy = round_trip(&input, &models).unwrap();
        assert_eq!(format!("{copy:?}"), format!("{input:?}"));
        // The single-player default too: no view, nothing held.
        let idle = SeatInput::default();
        let copy = round_trip(&idle, &models).unwrap();
        assert_eq!(format!("{copy:?}"), format!("{idle:?}"));
    }

    #[test]
    fn an_input_against_the_seats_last_costs_little_when_little_changed() {
        let models = Models::default();
        let first = SeatInput {
            commands: Vec::new(),
            ..busy_input()
        };
        // The next tick: the same stick and switches, no new commands.
        let mut next = SeatInput {
            tick: first.tick + 1,
            ..first.clone()
        };
        next.pilot.commands.clear();
        let mut s = Saver::with_models(models.clone());
        next.save(&mut s, Some(&first)).unwrap();
        let against = s.finish_section();
        let alone = to_bytes(&next, &models).unwrap().body;
        assert!(
            against.len() * 3 < alone.len(),
            "{} bytes against the last, {} alone",
            against.len(),
            alone.len()
        );
        let mut l = Loader::new(&against, &[], &models);
        let back = SeatInput::load(&mut l, Some(&first)).unwrap();
        l.finish().unwrap();
        assert_eq!(format!("{back:?}"), format!("{next:?}"));
    }

    #[test]
    fn damaged_input_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        let coded = to_bytes(&busy_input(), &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            assert!(from_bytes::<SeatInput>(&shorter, &models).is_err());
        }
        for bit in 0..coded.body.len() * 8 {
            let mut flipped = coded.clone();
            flipped.body[bit / 8] ^= 1 << (bit % 8);
            let _ = from_bytes::<SeatInput>(&flipped, &models);
        }
        // A variant number past the last is refused by name.
        let mut s = Saver::new();
        s.writer().write_varint(14);
        let body = s.finish_section();
        let mut l = Loader::new(&body, &[], &models);
        assert!(SeatCommand::load(&mut l, None).is_err());
    }
}
