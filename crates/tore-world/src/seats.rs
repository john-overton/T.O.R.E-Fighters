//! Planes, pilots and seats: who flies which aircraft of a mission. Stage B of
//! the multiplayer plan; see docs/ARCHITECTURE.md, "Aircraft, pilots and
//! seats".
//!
//! A plane is one aircraft of the mission, the same for the whole mission. A
//! seat is one human. Every plane has one pilot, the AI or a seat, and a seat
//! flies at most one plane. Single player is seat 0 flying plane 0.

use crate::{comms, world::AirportInput};
use tore_sim::{
    ai::launch::{Side, WingId},
    flight::PilotInput,
};

/// One aircraft of the mission, the same for the whole mission. The lead of
/// Friendly Wing 1 is plane 0 and the other aircraft count from 1 in the Quick
/// Mission's roster order, so a plane's id is also its combat target id and
/// its AI actor id. It names one aircraft, not an aircraft type: that is
/// `tore_formats::aircraft::AircraftId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaneId(pub u32);

/// One human at the mission. Single player is seat 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeatId(pub u8);

/// Where a plane sits in the mission's order of battle, fixed at setup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    /// The plane's side and wing.
    pub wing: WingId,
    /// The plane's place in its wing, from 0; the wing's first member is 0.
    pub member: u8,
}

impl Slot {
    /// The lead of Friendly Wing 1, where single player flies.
    pub const FRIENDLY_LEAD: Self = Self {
        wing: WingId {
            side: Side::Friendly,
            index: 0,
        },
        member: 0,
    };
}

/// Who flies a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pilot {
    /// The AI actor with the plane's id.
    Ai,
    /// A human, from this seat.
    Human(SeatId),
}

/// One plane of the mission and who flies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plane {
    pub id: PlaneId,
    pub slot: Slot,
    pub pilot: Pilot,
}

/// One human at the mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub id: SeatId,
    /// The plane this seat flies; `None` while it waits or watches.
    pub plane: Option<PlaneId>,
    /// Who the radio calls the crew of the seat's plane, when its type has a
    /// second seat.
    pub crew: Option<comms::Crew>,
}

/// Every plane of the mission with its pilot, in plane id order, and every
/// seat, in seat order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roster {
    planes: Vec<Plane>,
    seats: Vec<Seat>,
}

impl Roster {
    /// Single player: seat 0 flies plane 0, the lead of Friendly Wing 1, with
    /// `crew` for its radio, and the AI flies the planes of `ai`, which must not
    /// include plane 0.
    pub fn single_player(
        crew: Option<comms::Crew>,
        ai: impl IntoIterator<Item = (PlaneId, Slot)>,
    ) -> Self {
        let seat = SeatId(0);
        let mut planes = vec![Plane {
            id: PlaneId(0),
            slot: Slot::FRIENDLY_LEAD,
            pilot: Pilot::Human(seat),
        }];
        planes.extend(ai.into_iter().map(|(id, slot)| Plane {
            id,
            slot,
            pilot: Pilot::Ai,
        }));
        planes.sort_by_key(|plane| plane.id);
        debug_assert!(
            planes.windows(2).all(|pair| pair[0].id != pair[1].id),
            "two planes share an id"
        );
        Self {
            planes,
            seats: vec![Seat {
                id: seat,
                plane: Some(PlaneId(0)),
                crew,
            }],
        }
    }

    /// Every plane, in id order.
    pub fn planes(&self) -> &[Plane] {
        &self.planes
    }

    pub fn plane(&self, id: PlaneId) -> Option<&Plane> {
        self.planes
            .binary_search_by_key(&id, |plane| plane.id)
            .ok()
            .map(|index| &self.planes[index])
    }

    /// Every seat, in seat order.
    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    pub fn seat(&self, id: SeatId) -> Option<&Seat> {
        self.seats.iter().find(|seat| seat.id == id)
    }

    /// The seat flying `plane`, if a human flies it.
    pub fn seat_of(&self, plane: PlaneId) -> Option<SeatId> {
        match self.plane(plane)?.pilot {
            Pilot::Human(seat) => Some(seat),
            Pilot::Ai => None,
        }
    }
}

/// Everything one seat hands one tick.
#[derive(Clone, Debug, Default)]
pub struct SeatInput {
    pub seat: SeatId,
    /// The tick this input is for, which must be the world's next tick
    /// ([`crate::world::World::tick`]).
    pub tick: u64,
    /// Stick, throttle and pilot commands.
    pub pilot: PilotInput,
    /// The trigger, Space or the bound fire control, is held.
    pub trigger: bool,
    /// Commands given since the last tick, applied in this order at its start.
    pub commands: Vec<SeatCommand>,
}

/// A command a seat gives between ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeatCommand {
    /// A weapon-page button: step the weapon selection forward or back.
    CycleWeapon { forward: bool },
    /// A NAV mode switch or a tower request.
    Airport(AirportInput),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(side: Side, index: u8, member: u8) -> Slot {
        Slot {
            wing: WingId { side, index },
            member,
        }
    }

    #[test]
    fn single_player_flies_plane_zero_and_the_ai_the_rest() {
        let roster = Roster::single_player(
            Some(comms::Crew::Rio),
            [
                (PlaneId(3), slot(Side::Enemy, 0, 0)),
                (PlaneId(1), slot(Side::Friendly, 0, 1)),
            ],
        );
        let ids: Vec<_> = roster.planes().iter().map(|plane| plane.id.0).collect();
        assert_eq!(ids, [0, 1, 3]);
        assert_eq!(roster.plane(PlaneId(0)).unwrap().slot, Slot::FRIENDLY_LEAD);
        assert_eq!(roster.seat_of(PlaneId(0)), Some(SeatId(0)));
        assert_eq!(roster.seat_of(PlaneId(1)), None);
        assert_eq!(roster.plane(PlaneId(2)), None);
        let seat = roster.seat(SeatId(0)).unwrap();
        assert_eq!(seat.plane, Some(PlaneId(0)));
        assert_eq!(seat.crew, Some(comms::Crew::Rio));
        assert_eq!(roster.seats().len(), 1);
    }
}
