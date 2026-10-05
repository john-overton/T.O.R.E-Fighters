//! The coders of the roster: planes' pilots, seats, crews and wing recipients (the roster section).
//!
//! Stage H slice H2 (world shell). A restored roster replaces the fresh
//! world's, since handoffs change who flies which plane. The wing identity
//! types belong to `tore-sim`, which owns the trait, so a slot's wing is coded
//! field by field here.

use super::{Pilot, Plane, Roster, Seat, Slot};
use crate::seats::{PlaneId, SeatId};
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::Crew;
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
}
