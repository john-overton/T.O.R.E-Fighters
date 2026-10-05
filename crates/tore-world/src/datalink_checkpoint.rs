//! The coder of the flight data link (the data link section;
//! docs/formats/checkpoint.md). Stage H slice H10.
//!
//! [`DataLink`] is a value restored whole: its members, the pictures with the
//! tick each was published, the locks with the tick each was first held, the
//! engagements, the assignments the leads gave, the two warning tables, and
//! the set of planes already announced. Every field of the picture's types is
//! named, so a field a later G slice adds does not compile until it is coded
//! here, in the same slice.
//!
//! Skipped:
//!
//! - `DataLink::journal`: why-record (changes the picture made, waiting for
//!   the host to drain them; nothing reads one back, and a restored link
//!   starts with none waiting).
//!
//! `DataLink::announced` only gates whether a plane is journaled as new, which
//! makes it a small journal gate, but it is coded: it costs a few bits and a
//! restored link then does not announce every plane to the journal again
//! (agent decision).
//!
//! `WingId`, its `Side` and `PlayerOrder` belong to `tore-sim`, which owns the
//! trait, so a coder here cannot be a trait impl for them: the functions below
//! code them by a fixed number each, with a `match` that names every variant
//! and no `_` arm, so a new variant fails to compile.

use super::{
    Assignment, DataLink, Fuel, Journal, Lock, Member, MemberStatus, Track, Weapons,
    picture::{Damage, FlightId, FlightPicture},
};
use tore_sim::{
    ai::{
        launch::{Side, WingId},
        wing::{Formation, PlayerApproach, PlayerBreak, PlayerOrder},
    },
    checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid},
};

tore_sim::checkpoint_enum!(Fuel {
    Normal = 0,
    Joker = 1,
    Bingo = 2,
    Fumes = 3,
    Out = 4,
});

tore_sim::checkpoint_enum!(Weapons {
    Missiles = 0,
    GunsOnly = 1,
    Winchester = 2,
});

tore_sim::checkpoint_enum!(Damage {
    None = 0,
    Light = 1,
    Heavy = 2,
});

tore_sim::checkpoint_struct!(Track {
    reporter,
    target,
    position,
    velocity,
    channel,
    observed,
});

tore_sim::checkpoint_struct!(MemberStatus {
    plane,
    fuel,
    weapons,
    damage,
});

tore_sim::checkpoint_struct!(Lock { target, since });

/// A flight (a wing's side and number) as a side number and the index.
fn save_flight_id(s: &mut Saver, flight: &FlightId) {
    let WingId { side, index } = flight;
    let side: u64 = match side {
        Side::Friendly => 0,
        Side::Enemy => 1,
    };
    s.writer().write_varint(side);
    s.writer().write_varint(u64::from(*index));
}

fn load_flight_id(l: &mut Loader<'_>) -> Result<FlightId, CheckpointError> {
    let side = match l.reader().read_varint()? {
        0 => Side::Friendly,
        1 => Side::Enemy,
        other => return invalid(format!("a wing side has no number {other}")),
    };
    let index = l.reader().read_varint()?;
    match u8::try_from(index) {
        Ok(index) => Ok(WingId { side, index }),
        Err(_) => invalid(format!("wing {index} does not fit a byte")),
    }
}

/// A player order as its number and the one variant that carries a choice.
fn save_order(s: &mut Saver, order: &PlayerOrder) -> Result<(), CheckpointError> {
    let (number, choice): (u64, u64) = match order {
        PlayerOrder::EngageMyTarget => (0, 0),
        PlayerOrder::ProtectMe => (1, 0),
        PlayerOrder::AttackOnContact => (2, 0),
        PlayerOrder::EngageFromFormation => (3, 0),
        PlayerOrder::Disengage => (4, 0),
        PlayerOrder::Break(direction) => (
            5,
            match direction {
                PlayerBreak::Left => 0,
                PlayerBreak::Right => 1,
                PlayerBreak::Low => 2,
                PlayerBreak::High => 3,
                PlayerBreak::Straight => 4,
            },
        ),
        PlayerOrder::Approach(direction) => (
            6,
            match direction {
                PlayerApproach::Left => 0,
                PlayerApproach::Right => 1,
                PlayerApproach::Low => 2,
                PlayerApproach::High => 3,
            },
        ),
        PlayerOrder::Formation(formation) => {
            s.writer().write_varint(7);
            return formation.save(s, None);
        }
        PlayerOrder::Spacing => (8, 0),
        PlayerOrder::Stacking => (9, 0),
        PlayerOrder::ControlToggle => (10, 0),
        PlayerOrder::BugOut => (11, 0),
        PlayerOrder::LandAtSelected => (12, 0),
    };
    s.writer().write_varint(number);
    if matches!(number, 5 | 6) {
        s.writer().write_varint(choice);
    }
    Ok(())
}

fn load_order(l: &mut Loader<'_>) -> Result<PlayerOrder, CheckpointError> {
    let number = l.reader().read_varint()?;
    Ok(match number {
        0 => PlayerOrder::EngageMyTarget,
        1 => PlayerOrder::ProtectMe,
        2 => PlayerOrder::AttackOnContact,
        3 => PlayerOrder::EngageFromFormation,
        4 => PlayerOrder::Disengage,
        5 => PlayerOrder::Break(match l.reader().read_varint()? {
            0 => PlayerBreak::Left,
            1 => PlayerBreak::Right,
            2 => PlayerBreak::Low,
            3 => PlayerBreak::High,
            4 => PlayerBreak::Straight,
            other => return invalid(format!("a break has no direction {other}")),
        }),
        6 => PlayerOrder::Approach(match l.reader().read_varint()? {
            0 => PlayerApproach::Left,
            1 => PlayerApproach::Right,
            2 => PlayerApproach::Low,
            3 => PlayerApproach::High,
            other => return invalid(format!("an approach has no direction {other}")),
        }),
        7 => PlayerOrder::Formation(Formation::load(l, None)?),
        8 => PlayerOrder::Spacing,
        9 => PlayerOrder::Stacking,
        10 => PlayerOrder::ControlToggle,
        11 => PlayerOrder::BugOut,
        12 => PlayerOrder::LandAtSelected,
        other => return invalid(format!("a player order has no number {other}")),
    })
}

impl Checkpoint for Member {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Member {
            plane,
            flight,
            member,
            aircraft,
            radar,
            human,
            alive,
            position,
        } = self;
        plane.save(s, None)?;
        save_flight_id(s, flight);
        member.save(s, None)?;
        aircraft.save(s, None)?;
        radar.save(s, None)?;
        human.save(s, None)?;
        alive.save(s, None)?;
        position.save(s, None)
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let plane = Checkpoint::load(l, None)?;
        let flight = load_flight_id(l)?;
        Ok(Member {
            plane,
            flight,
            member: Checkpoint::load(l, None)?,
            aircraft: Checkpoint::load(l, None)?,
            radar: Checkpoint::load(l, None)?,
            human: Checkpoint::load(l, None)?,
            alive: Checkpoint::load(l, None)?,
            position: Checkpoint::load(l, None)?,
        })
    }
}

impl Checkpoint for FlightPicture {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let FlightPicture {
            flight,
            tick,
            tracks,
            status,
        } = self;
        save_flight_id(s, flight);
        tick.save(s, None)?;
        tracks.save(s, None)?;
        status.save(s, None)
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let flight = load_flight_id(l)?;
        Ok(FlightPicture {
            flight,
            tick: Checkpoint::load(l, None)?,
            tracks: Checkpoint::load(l, None)?,
            status: Checkpoint::load(l, None)?,
        })
    }
}

impl Checkpoint for Assignment {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Assignment {
            target,
            by,
            tick,
            order,
            acknowledged,
        } = self;
        target.save(s, None)?;
        by.save(s, None)?;
        tick.save(s, None)?;
        save_order(s, order)?;
        acknowledged.save(s, None)
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let target = Checkpoint::load(l, None)?;
        let by = Checkpoint::load(l, None)?;
        let tick = Checkpoint::load(l, None)?;
        let order = load_order(l)?;
        Ok(Assignment {
            target,
            by,
            tick,
            order,
            acknowledged: Checkpoint::load(l, None)?,
        })
    }
}

tore_sim::checkpoint_struct!(DataLink {
    tick,
    members,
    pictures,
    locks,
    engaged,
    assignments,
    warned,
    seat_warned,
    announced,
} skip {
    // Why-record: the changes waiting for the host to drain; nothing reads
    // one back.
    journal = Journal::default(),
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seats::SeatId;
    use tore_formats::aircraft::AircraftId;
    use tore_sim::{
        ai::launch::{Side, WingId},
        checkpoint::{Models, round_trip, to_bytes},
        sensors::Channel,
    };

    /// A link with every table holding something.
    fn full() -> DataLink {
        let flight = |side, index| WingId { side, index };
        let mut link = DataLink {
            tick: 1_234,
            ..DataLink::default()
        };
        for (plane, human) in [(0, true), (1, false), (4, false)] {
            link.members.push(Member {
                plane,
                flight: flight(
                    if plane < 4 {
                        Side::Friendly
                    } else {
                        Side::Enemy
                    },
                    1,
                ),
                member: (plane % 4) as u8,
                aircraft: (plane != 4).then_some(AircraftId::F18),
                radar: plane != 4,
                human,
                alive: plane != 1,
                position: [f64::from(plane) * 100.5, -0.0, f64::NAN],
            });
        }
        link.pictures.push(FlightPicture {
            flight: flight(Side::Enemy, 2),
            tick: 1_200,
            tracks: vec![Track {
                reporter: 0,
                target: 4,
                position: [1., 2., 3.],
                velocity: [-4., 5., 6.5],
                channel: Channel::Radar,
                observed: 1_199,
            }],
            status: vec![MemberStatus {
                plane: 0,
                fuel: Fuel::Bingo,
                weapons: Weapons::GunsOnly,
                damage: Damage::Heavy,
            }],
        });
        link.locks.insert(
            0,
            Lock {
                target: 4,
                since: 900,
            },
        );
        link.engaged.insert(0, 4);
        link.engaged.insert(4, 0);
        for (receiver, order) in [
            (1, PlayerOrder::EngageMyTarget),
            (2, PlayerOrder::Break(PlayerBreak::Straight)),
            (3, PlayerOrder::Approach(PlayerApproach::High)),
            (5, PlayerOrder::Formation(Formation::LineAstern)),
            (6, PlayerOrder::LandAtSelected),
        ] {
            link.assignments.insert(
                receiver,
                Assignment {
                    target: 4,
                    by: 0,
                    tick: 1_000 + u64::from(receiver),
                    order,
                    acknowledged: receiver % 2 == 0,
                },
            );
        }
        link.warned.insert((0, 1, 4));
        link.warned.insert((1, 0, 5));
        link.seat_warned.insert(SeatId(2), 1_100);
        link.announced.extend([0, 1, 4]);
        link.journal.push(super::super::Entry::Member {
            tick: 1,
            plane: 0,
            radar: true,
        });
        link
    }

    fn assert_same(a: &DataLink, b: &DataLink) {
        assert_eq!(a.tick(), b.tick());
        assert_eq!(a.members(), b.members());
        assert_eq!(a.pictures(), b.pictures());
        assert_eq!(a.locks(), b.locks());
        assert_eq!(a.engagements(), b.engagements());
        assert_eq!(a.assignments(), b.assignments());
        assert_eq!(a.warned(), b.warned());
        assert_eq!(a.seat_warned(), b.seat_warned());
        assert_eq!(a.announced, b.announced);
    }

    #[test]
    fn a_link_with_every_table_filled_round_trips_and_leaves_the_journal() {
        let models = Models::default();
        let link = full();
        let copy = round_trip(&link, &models).unwrap();
        // The NaN in a position compares unequal to itself, so compare the
        // coding of the members, and the rest by value.
        assert_eq!(copy.members().len(), link.members().len());
        let bytes = |link: &DataLink| to_bytes(link, &models).unwrap();
        assert_eq!(bytes(&copy), bytes(&link));
        assert_eq!(copy.pictures(), link.pictures());
        assert_eq!(copy.locks(), link.locks());
        assert_eq!(copy.assignments(), link.assignments());
        assert_eq!(copy.warned(), link.warned());
        assert_eq!(copy.seat_warned(), link.seat_warned());
        assert_eq!(copy.announced, link.announced);
        assert_eq!(copy.tick(), 1_234);
        // The journal is a why-record: not coded.
        assert!(copy.journal.is_empty());
        assert!(!link.journal.is_empty());
    }

    #[test]
    fn an_empty_link_round_trips() {
        let link = DataLink::default();
        let copy = round_trip(&link, &Models::default()).unwrap();
        assert_same(&link, &copy);
    }

    #[test]
    fn every_player_order_round_trips() {
        let orders = [
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
        ]
        .into_iter()
        .chain(PlayerBreak::ALL.map(PlayerOrder::Break))
        .chain(PlayerApproach::ALL.map(PlayerOrder::Approach))
        .chain(Formation::ALL.map(PlayerOrder::Formation));
        let models = Models::default();
        for order in orders {
            let assignment = Assignment {
                target: 9,
                by: 3,
                tick: 77,
                order,
                acknowledged: true,
            };
            let copy = round_trip(&assignment, &models).unwrap();
            assert_eq!(copy, assignment, "{order:?}");
        }
    }

    #[test]
    fn a_number_that_is_no_order_side_or_wing_is_refused() {
        let models = Models::default();
        let mut s = Saver::new();
        s.writer().write_varint(13);
        let body = s.finish_section();
        let coded = tore_sim::checkpoint::Coded {
            body,
            records: Vec::new(),
        };
        let mut l = Loader::new(&coded.body, &coded.records, &models);
        assert!(load_order(&mut l).is_err());

        let mut s = Saver::new();
        s.writer().write_varint(2);
        s.writer().write_varint(0);
        let body = s.finish_section();
        let mut l = Loader::new(&body, &[], &models);
        assert!(load_flight_id(&mut l).is_err());

        let mut s = Saver::new();
        s.writer().write_varint(0);
        s.writer().write_varint(300);
        let body = s.finish_section();
        let mut l = Loader::new(&body, &[], &models);
        assert!(load_flight_id(&mut l).is_err());
    }
}
