//! Planes, pilots and seats: who flies which aircraft of a mission. Stage B of
//! the multiplayer plan; see docs/ARCHITECTURE.md, "Aircraft, pilots and
//! seats".
//!
//! A plane is one aircraft of the mission, the same for the whole mission. A
//! seat is one human. Every plane has one pilot, the AI or a seat, and a seat
//! flies at most one plane. Single player is seat 0 flying plane 0.

use crate::{
    comms,
    world::{AirportInput, replies::Reply},
};
use tore_sim::{
    ai::launch::{Side, WingId},
    ai::wing::PlayerOrder,
    combat::live,
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
    /// Nobody: a human's plane that was lost (destroyed, its pilot dead or
    /// ejected) and abandoned to the mission
    /// ([`crate::world::MissionCommand::Abandon`], stage F phase 2). Its
    /// cockpit steps on with neutral controls (the wreck falling, the
    /// escape), and nobody can take it.
    Lost,
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
    /// The wingman the seat's orders address, `None` for the whole wing.
    pub wing_recipient: Option<u8>,
}

/// Every plane of the mission with its pilot, in plane id order, and every
/// seat, in seat order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roster {
    planes: Vec<Plane>,
    seats: Vec<Seat>,
    /// Who the players are, by callsign (the lobby pass's follow-up F1).
    callsigns: Callsigns,
}

/// The players' callsigns as the mission knows them (the lobby pass's
/// follow-up F1): the callsign the host gave the player in each seat
/// ([`crate::world::MissionCommand::Callsign`]), and the callsign of the
/// player who flies each plane, or flew it last. A seat is reused by the
/// next player, so the host names a seat's player again before seating
/// it; a plane keeps the name of whoever flew it. Single player has none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Callsigns {
    pub(crate) seats: std::collections::BTreeMap<SeatId, String>,
    pub(crate) planes: std::collections::BTreeMap<PlaneId, String>,
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
                wing_recipient: None,
            }],
            callsigns: Callsigns::default(),
        }
    }

    /// An open mission: the AI flies every plane of `ai`, plane 0 included,
    /// and there is no seat yet. Humans join by taking planes
    /// ([`Self::take_plane`]).
    pub fn open(ai: impl IntoIterator<Item = (PlaneId, Slot)>) -> Self {
        let mut planes: Vec<Plane> = ai
            .into_iter()
            .map(|(id, slot)| Plane {
                id,
                slot,
                pilot: Pilot::Ai,
            })
            .collect();
        planes.sort_by_key(|plane| plane.id);
        debug_assert!(
            planes.windows(2).all(|pair| pair[0].id != pair[1].id),
            "two planes share an id"
        );
        Self {
            planes,
            seats: Vec::new(),
            callsigns: Callsigns::default(),
        }
    }

    /// A roster with a human in each of `humans`' planes, one seat each in
    /// seat order, and the AI in the planes of `ai`. For tests: the runtime
    /// way to put a human in a plane is a handoff.
    #[cfg(any(test, feature = "test-support"))]
    pub fn with_humans(
        humans: impl IntoIterator<Item = (PlaneId, Slot, SeatId, Option<comms::Crew>)>,
        ai: impl IntoIterator<Item = (PlaneId, Slot)>,
    ) -> Self {
        let mut planes = Vec::new();
        let mut seats = Vec::new();
        for (id, slot, seat, crew) in humans {
            planes.push(Plane {
                id,
                slot,
                pilot: Pilot::Human(seat),
            });
            seats.push(Seat {
                id: seat,
                plane: Some(id),
                crew,
                wing_recipient: None,
            });
        }
        planes.extend(ai.into_iter().map(|(id, slot)| Plane {
            id,
            slot,
            pilot: Pilot::Ai,
        }));
        planes.sort_by_key(|plane| plane.id);
        seats.sort_by_key(|seat| seat.id);
        debug_assert!(
            planes.windows(2).all(|pair| pair[0].id != pair[1].id),
            "two planes share an id"
        );
        debug_assert!(
            seats.windows(2).all(|pair| pair[0].id != pair[1].id),
            "two humans share a seat"
        );
        Self {
            planes,
            seats,
            callsigns: Callsigns::default(),
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

    /// Whether plane `id` flies for Redfor (an enemy wing's plane). A plane
    /// the roster does not hold is Blue's, as single player's plane is.
    pub fn redfor(&self, id: PlaneId) -> bool {
        self.plane(id)
            .is_some_and(|plane| plane.slot.wing.side.is_enemy())
    }

    /// Every seat, in seat order.
    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    pub fn seat(&self, id: SeatId) -> Option<&Seat> {
        self.seats.iter().find(|seat| seat.id == id)
    }

    /// Chooses the wingman the seat's orders address; `None` is the whole
    /// wing.
    pub fn set_wing_recipient(&mut self, id: SeatId, recipient: Option<u8>) {
        if let Some(seat) = self.seats.iter_mut().find(|seat| seat.id == id) {
            seat.wing_recipient = recipient;
        }
    }

    /// A human takes the AI-flown `plane`: `seat` flies it, joining the roster
    /// as a new seat with `crew` for its radio if it does not exist yet. The
    /// seat must be waiting (flying no plane).
    pub fn take_plane(
        &mut self,
        seat: SeatId,
        plane: PlaneId,
        crew: Option<comms::Crew>,
    ) -> Result<(), String> {
        let index = self
            .planes
            .binary_search_by_key(&plane, |p| p.id)
            .map_err(|_| format!("plane {} is not in the mission", plane.0))?;
        if self.planes[index].pilot != Pilot::Ai {
            return Err(format!("plane {} is not flown by the AI", plane.0));
        }
        match self.seats.iter_mut().find(|s| s.id == seat) {
            Some(existing) if existing.plane.is_some() => {
                return Err(format!("seat {} already flies a plane", seat.0));
            }
            Some(existing) => {
                existing.plane = Some(plane);
                existing.crew = crew;
                existing.wing_recipient = None;
            }
            None => {
                self.seats.push(Seat {
                    id: seat,
                    plane: Some(plane),
                    crew,
                    wing_recipient: None,
                });
                self.seats.sort_by_key(|s| s.id);
            }
        }
        self.planes[index].pilot = Pilot::Human(seat);
        if let Some(callsign) = self.callsigns.seats.get(&seat) {
            self.callsigns.planes.insert(plane, callsign.clone());
        }
        Ok(())
    }

    /// The host names the player in `seat` (the lobby pass's follow-up F1):
    /// the seat's callsign from now on, and its plane's if it flies one;
    /// each plane it takes from now on bears the name too.
    pub fn set_callsign(&mut self, seat: SeatId, callsign: String) {
        if let Some(plane) = self.seat(seat).and_then(|s| s.plane) {
            self.callsigns.planes.insert(plane, callsign.clone());
        }
        self.callsigns.seats.insert(seat, callsign);
    }

    /// The callsign of the player the host last named for `seat`.
    pub fn seat_callsign(&self, seat: SeatId) -> Option<&str> {
        self.callsigns.seats.get(&seat).map(String::as_str)
    }

    /// The callsign of the player who flies `plane`, or flew it last; `None`
    /// for a plane no named player has flown (the AI's, or single player's).
    pub fn plane_callsign(&self, plane: PlaneId) -> Option<&str> {
        self.callsigns.planes.get(&plane).map(String::as_str)
    }

    /// Every callsign the mission knows.
    pub fn callsigns(&self) -> &Callsigns {
        &self.callsigns
    }

    /// The seat gives its plane back to the AI. The seat stays, waiting.
    /// Returns the plane, or `None` when the seat flies none.
    pub fn release_plane(&mut self, seat: SeatId) -> Option<PlaneId> {
        let seat = self.seats.iter_mut().find(|s| s.id == seat)?;
        let plane = seat.plane.take()?;
        seat.wing_recipient = None;
        if let Some(entry) = self.planes.iter_mut().find(|p| p.id == plane) {
            entry.pilot = Pilot::Ai;
        }
        Some(plane)
    }

    /// Stage F phase 2's revival (slice F2-V): the seat leaves its lost
    /// plane, which nobody flies from now on ([`Pilot::Lost`]). The seat
    /// stays, waiting. Returns the plane, or `None` when the seat flies none.
    pub(crate) fn abandon_plane(&mut self, seat: SeatId) -> Option<PlaneId> {
        let seat = self.seats.iter_mut().find(|s| s.id == seat)?;
        let plane = seat.plane.take()?;
        seat.wing_recipient = None;
        if let Some(entry) = self.planes.iter_mut().find(|p| p.id == plane) {
            entry.pilot = Pilot::Lost;
        }
        Some(plane)
    }

    /// A plane the mission did not start with (a revival's), in id order.
    pub(crate) fn add_plane(&mut self, plane: Plane) -> Result<(), String> {
        match self.planes.binary_search_by_key(&plane.id, |p| p.id) {
            Ok(_) => Err(format!("plane {} is in the mission already", plane.id.0)),
            Err(at) => {
                self.planes.insert(at, plane);
                Ok(())
            }
        }
    }

    /// Takes a plane nobody flies out of the roster (a retired wreck).
    pub(crate) fn remove_plane(&mut self, plane: PlaneId) -> Option<Plane> {
        let at = self.planes.binary_search_by_key(&plane, |p| p.id).ok()?;
        (self.planes[at].pilot == Pilot::Lost).then(|| self.planes.remove(at))
    }

    /// Takes an AI wreck out of the roster for good (the lobby pass's slice
    /// R1: an AI wreck whose lineage respawned, retired to make room). The
    /// entry comes back with nobody as its pilot ([`Pilot::Lost`]).
    pub(crate) fn remove_ai_plane(&mut self, plane: PlaneId) -> Option<Plane> {
        let at = self.planes.binary_search_by_key(&plane, |p| p.id).ok()?;
        (self.planes[at].pilot == Pilot::Ai).then(|| {
            let mut entry = self.planes.remove(at);
            entry.pilot = Pilot::Lost;
            entry
        })
    }

    /// The seat flying `plane`, if a human flies it.
    pub fn seat_of(&self, plane: PlaneId) -> Option<SeatId> {
        match self.plane(plane)?.pilot {
            Pilot::Human(seat) => Some(seat),
            Pilot::Ai | Pilot::Lost => None,
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
    /// The seat's scope controls: channel, display range and contact history.
    /// The step sets them on the seat's flight before its commands, so a
    /// command given after a change sees it and the labels never lag.
    pub sensors: tore_sim::sensors::Controls,
    /// The AC-130 gunsight slew: normalized deflection, x right and y up,
    /// -127 to 127. The host integrates the look angles from it, so single
    /// player and multiplayer run the same slew. Ignored on other aircraft.
    pub sight: [i8; 2],
    /// The AC-130 target camera's zoom step, 1 to 6 (0: the default step).
    /// The host needs it for the slew rate and the Backslash pick radius.
    pub sight_zoom: u8,
    /// Commands given since the last tick, applied in this order at its start.
    pub commands: Vec<SeatCommand>,
    /// What the seat's screen showed when this input was sampled, for lag
    /// compensation: `None` means no rewind, as in single player, the AI
    /// probe and every local seat. See docs/ARCHITECTURE.md, "Hits and lag
    /// compensation".
    pub view: Option<SeatView>,
}

/// What a remote seat's screen showed for one input: the host tick it drew the
/// other aircraft at (V) for the input's tick (T), and its interpolation
/// delay. The gun rounds the seat fires on that tick are tested against the
/// aircraft as they were `T - V` ticks before, capped (see
/// [`crate::combat::gun_rewind`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeatView {
    /// The host tick the seat's screen showed (V).
    pub tick: u64,
    /// The seat's interpolation delay, ticks.
    pub interpolation_delay: u8,
}

/// A command a seat gives between ticks. The step applies each one at the
/// start of the tick, in the order given, to the plane the seat flies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeatCommand {
    /// A weapon-page button or the weapon keys: step the weapon selection
    /// forward or back. NAV mode follows the arming.
    CycleWeapon { forward: bool },
    /// A NAV mode switch or a tower request.
    Airport(AirportInput),
    /// A combat command exactly as combat takes it: the next, previous or
    /// visual designation, a designation by identity from a scope click, and
    /// the seeker mode or the release of the designation from the weapon
    /// display.
    Combat(live::Command),
    /// A combat command from a key, a button or the menu: arming, the seeker
    /// mode, clearing the designation, jettison and the range and development
    /// commands. It lets go of the trigger first and puts the payload weight
    /// right afterwards. Outside `--live-fire` only the arming, seeker,
    /// designation and AC-130 gun-group commands take effect.
    Manual(live::Command),
    /// Put a new target on the range (`--live-fire` only).
    RangeReset,
    /// Release one chaff cartridge; refused when the aircraft is destroyed,
    /// the pilot has ejected or it has no hit points.
    ReleaseChaff,
    /// Release one flare, with the same refusals as chaff.
    ReleaseFlare,
    /// Let go of the trigger, which many UI events do: a menu opening, a
    /// pause, a modifier key or the window losing focus.
    ReleaseTrigger,
    /// Toggle radio silence, which holds back the wing and crew calls.
    RadioSilence,
    /// Choose the wingman the seat's orders address (`None`: the whole wing).
    WingRecipient(Option<u8>),
    /// An Alt-key order to the seat's wing, addressed to the wingman the seat
    /// chose, or to all of them.
    WingOrder(PlayerOrder),
    /// Alt-T: order the wing into the formation after the one it flies.
    WingFormationCycle,
    /// The Space key. `blocked` is set when the game was paused, out of
    /// focus or a modifier key was held: the key then only lets go.
    TriggerKey {
        down: bool,
        repeat: bool,
        blocked: bool,
    },
    /// A wingman's reply or request to its flight (Alt+Shift+E, W, B or H;
    /// stage F phase 2). The step does nothing with it until slice F2-R
    /// builds the call.
    WingReply(Reply),
    /// Alt+N: start or stop monitoring the side's battle net, where the flight
    /// leads of the other flights repeat their contact reports and assignment
    /// calls (stage G, slice G8). Off at the start.
    BattleNet,
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

    #[test]
    fn a_seat_takes_an_ai_plane_and_gives_it_back() {
        let mut roster = Roster::single_player(
            None,
            [
                (PlaneId(1), slot(Side::Friendly, 0, 1)),
                (PlaneId(2), slot(Side::Friendly, 1, 0)),
            ],
        );
        // A new seat joins by taking a plane; a plane with a human is refused.
        assert!(roster.take_plane(SeatId(1), PlaneId(0), None).is_err());
        assert!(roster.take_plane(SeatId(1), PlaneId(9), None).is_err());
        roster
            .take_plane(SeatId(1), PlaneId(2), Some(comms::Crew::Rio))
            .unwrap();
        assert_eq!(roster.seat_of(PlaneId(2)), Some(SeatId(1)));
        assert_eq!(roster.seat(SeatId(1)).unwrap().plane, Some(PlaneId(2)));
        assert_eq!(roster.seat(SeatId(1)).unwrap().crew, Some(comms::Crew::Rio));
        // A seat flies one plane at a time.
        assert!(roster.take_plane(SeatId(1), PlaneId(1), None).is_err());
        // Giving it back leaves the seat waiting and the plane to the AI.
        assert_eq!(roster.release_plane(SeatId(1)), Some(PlaneId(2)));
        assert_eq!(roster.seat_of(PlaneId(2)), None);
        assert_eq!(roster.seat(SeatId(1)).unwrap().plane, None);
        assert_eq!(roster.release_plane(SeatId(1)), None);
        // The waiting seat takes another.
        roster.take_plane(SeatId(1), PlaneId(1), None).unwrap();
        assert_eq!(roster.seat_of(PlaneId(1)), Some(SeatId(1)));
        assert_eq!(roster.seats().len(), 2);
    }

    /// The lobby pass's follow-up F1: a seat's callsign names the plane it
    /// flies and each plane it takes; a plane keeps the name of whoever flew
    /// it last when the seat passes to the next player.
    #[test]
    fn callsigns_follow_seats_onto_their_planes() {
        let mut roster = Roster::open([
            (PlaneId(0), Slot::FRIENDLY_LEAD),
            (PlaneId(1), slot(Side::Friendly, 0, 1)),
            (PlaneId(2), slot(Side::Enemy, 0, 0)),
        ]);
        assert_eq!(roster.plane_callsign(PlaneId(0)), None);
        // Named before it takes a plane.
        roster.set_callsign(SeatId(0), "Viper".into());
        assert_eq!(roster.seat_callsign(SeatId(0)), Some("Viper"));
        roster.take_plane(SeatId(0), PlaneId(0), None).unwrap();
        assert_eq!(roster.plane_callsign(PlaneId(0)), Some("Viper"));
        // Named while it flies: its plane takes the name at once.
        roster.take_plane(SeatId(1), PlaneId(1), None).unwrap();
        assert_eq!(roster.plane_callsign(PlaneId(1)), None);
        roster.set_callsign(SeatId(1), "Hawk".into());
        assert_eq!(roster.plane_callsign(PlaneId(1)), Some("Hawk"));
        // Viper leaves; the seat's next player is another.
        roster.release_plane(SeatId(0));
        roster.set_callsign(SeatId(0), "Moth".into());
        roster.take_plane(SeatId(0), PlaneId(2), None).unwrap();
        assert_eq!(roster.plane_callsign(PlaneId(2)), Some("Moth"));
        assert_eq!(roster.plane_callsign(PlaneId(0)), Some("Viper"));
        // Single player names nobody.
        let single = Roster::single_player(None, []);
        assert_eq!(single.plane_callsign(PlaneId(0)), None);
        assert_eq!(single.callsigns(), &Callsigns::default());
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "seats_checkpoint.rs"]
mod checkpoint;
