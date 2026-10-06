//! The sort warning (slice G6): two members of one flight hold a radar lock on
//! the same aircraft and the lead did not mean it. Each human of the pair is
//! told with a HUD line and a short beep; the AI's yield on the same edge
//! belongs to a later slice, which reads the same [`DataLink::warned`] table.
//!
//! The edge is the pair: it fires once when two living members of a flight
//! first both hold a lock on one aircraft, and fires again only after one of
//! them has let go and the pair forms anew. A seat hears at most one warning
//! every [`SORT_COOLDOWN_TICKS`]. All of it is plain state on the picture
//! (`warned`, `seat_warned`) and the tick, so it replays the same.
//!
//! Agent decisions: a pair the lead meant is never warned of, which is both
//! members assigned the aircraft, or one of them assigned it by the other. A
//! seat in an aircraft with no radar is warned like any other (John,
//! 2026-10-05: only the radar scope's marks go without a radar; such an
//! aircraft holds no lock, so in practice it is never in a pair).

use super::DataLink;
use crate::{
    comms::{Call, Kind, Phrase},
    radio_calls::{FLIGHTS, POSITIONS},
    seats::{Pilot, PlaneId, Roster, SeatId},
    world::Cue,
};
use std::collections::BTreeSet;

/// A seat hears one sort warning in this many ticks: ten seconds at 120 Hz.
pub const SORT_COOLDOWN_TICKS: u64 = 10 * 120;

/// The retail radar-link beep, played to the seat with each warning.
pub const SORT_BEEP: &str = "^BEEP2";

/// One warning for one seat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortWarning {
    pub seat: SeatId,
    /// The plane the seat flies.
    pub plane: u32,
    /// The flightmate locked on the same aircraft.
    pub other: u32,
    /// The aircraft both hold.
    pub target: u32,
    /// The HUD line.
    pub text: String,
}

impl SortWarning {
    /// The HUD line and the beep, for the seat.
    pub fn cues(&self) -> [Cue; 2] {
        let beep = Phrase {
            text: String::new(),
            stems: vec![SORT_BEEP.to_string()],
        };
        [
            Cue::Message {
                seat: self.seat,
                text: self.text.clone(),
            },
            Cue::Radio {
                seat: self.seat,
                call: Call::new("", beep, Kind::Important).direct(),
            },
        ]
    }
}

/// "Red two": the flight colour and the member's place, as the radio says it.
fn name_of(flight: u8, member: u8) -> String {
    let position = POSITIONS.get(usize::from(member)).copied().unwrap_or("?");
    match FLIGHTS.get(usize::from(flight)) {
        Some(colour) => format!("{colour} {position}"),
        None => format!("Flight {} {position}", u32::from(flight) + 1),
    }
}

impl DataLink {
    /// Finds the pairs that have just started to lock one aircraft and the
    /// warnings the seats of those pairs are owed. Call it after the AI step,
    /// when every member's lock is current.
    pub fn sort_warnings(&mut self, roster: &Roster) -> Vec<SortWarning> {
        // The living members holding a lock, in plane id order: usually few.
        let holders: Vec<(u32, super::FlightId, u32)> = self
            .members
            .iter()
            .filter(|member| member.alive)
            .filter_map(|member| {
                self.lock(member.plane)
                    .map(|lock| (member.plane, member.flight, lock.target))
            })
            .collect();
        let mut pairs: BTreeSet<(u32, u32, u32)> = BTreeSet::new();
        for (index, (a, flight, target)) in holders.iter().enumerate() {
            for (b, other_flight, other_target) in &holders[index + 1..] {
                if other_flight == flight && other_target == target {
                    pairs.insert((*a, *b, *target));
                }
            }
        }
        // A pair that let go is forgotten, so it can warn again.
        self.warned.retain(|pair| pairs.contains(pair));
        let mut warnings = Vec::new();
        for pair in pairs {
            if self.warned.contains(&pair) || self.meant(pair) {
                continue;
            }
            self.warned.insert(pair);
            let (a, b, target) = pair;
            for (plane, other) in [(a, b), (b, a)] {
                warnings.extend(self.warn_seat(roster, plane, other, target));
            }
        }
        warnings
    }

    /// Whether the lead meant the pair: both were assigned the aircraft, or
    /// one was assigned it by the other.
    fn meant(&self, (a, b, target): (u32, u32, u32)) -> bool {
        let given = |plane: u32| {
            self.assignments
                .get(&plane)
                .filter(|assignment| assignment.target == target)
        };
        let (of_a, of_b) = (given(a), given(b));
        (of_a.is_some() && of_b.is_some())
            || of_a.is_some_and(|assignment| assignment.by == b)
            || of_b.is_some_and(|assignment| assignment.by == a)
    }

    /// The warning `plane`'s seat is owed for `other`'s lock on `target`, if
    /// a human with a radar flies it and its last warning is old enough.
    fn warn_seat(
        &mut self,
        roster: &Roster,
        plane: u32,
        other: u32,
        target: u32,
    ) -> Option<SortWarning> {
        let member = self.member(plane)?;
        if !member.human {
            return None;
        }
        let Pilot::Human(seat) = roster.plane(PlaneId(plane))?.pilot else {
            return None;
        };
        if self
            .seat_warned
            .get(&seat)
            .is_some_and(|last| self.tick.saturating_sub(*last) < SORT_COOLDOWN_TICKS)
        {
            return None;
        }
        let mate = self.member(other)?;
        let text = format!(
            "Sort: {} is locked on your target.",
            name_of(mate.flight.index, mate.member)
        );
        self.seat_warned.insert(seat, self.tick);
        Some(SortWarning {
            seat,
            plane,
            other,
            target,
            text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datalink::{Assignment, Lock, Member};
    use crate::seats::Slot;
    use tore_sim::ai::{
        launch::{Side, WingId},
        wing::PlayerOrder,
    };

    const RED: WingId = WingId {
        side: Side::Friendly,
        index: 0,
    };
    const BLUE: WingId = WingId {
        side: Side::Friendly,
        index: 1,
    };

    fn member(plane: u32, flight: WingId, number: u8, human: bool, radar: bool) -> Member {
        Member {
            plane,
            flight,
            member: number,
            aircraft: None,
            radar,
            human,
            alive: true,
            position: [0.; 3],
        }
    }

    /// Red one (plane 0, seat 0) and Red two (plane 1, seat 1) are human,
    /// Red three (plane 2) is AI; Blue one (plane 3) is AI.
    fn fixture() -> (DataLink, Roster) {
        let link = DataLink {
            members: vec![
                member(0, RED, 0, true, true),
                member(1, RED, 1, true, true),
                member(2, RED, 2, false, true),
                member(3, BLUE, 0, false, true),
            ],
            tick: 1000,
            ..DataLink::default()
        };
        let slot = |wing: WingId, member: u8| Slot { wing, member };
        let roster = Roster::with_humans(
            [
                (PlaneId(0), slot(RED, 0), SeatId(0), None),
                (PlaneId(1), slot(RED, 1), SeatId(1), None),
            ],
            [(PlaneId(2), slot(RED, 2)), (PlaneId(3), slot(BLUE, 0))],
        );
        (link, roster)
    }

    fn lock(link: &mut DataLink, plane: u32, target: u32) {
        link.locks.insert(plane, Lock { target, since: 0 });
    }

    fn texts(warnings: &[SortWarning]) -> Vec<(u8, &str)> {
        warnings
            .iter()
            .map(|w| (w.seat.0, w.text.as_str()))
            .collect()
    }

    #[test]
    fn two_humans_locking_one_aircraft_are_each_told_of_the_other() {
        let (mut link, roster) = fixture();
        lock(&mut link, 0, 50);
        lock(&mut link, 1, 50);
        let warnings = link.sort_warnings(&roster);
        assert_eq!(
            texts(&warnings),
            [
                (0, "Sort: Red two is locked on your target."),
                (1, "Sort: Red one is locked on your target."),
            ]
        );
        assert_eq!(warnings[0].target, 50);
        assert_eq!((warnings[0].plane, warnings[0].other), (0, 1));
    }

    #[test]
    fn the_warning_is_a_hud_line_and_a_direct_beep_for_the_seat() {
        let (mut link, roster) = fixture();
        lock(&mut link, 0, 50);
        lock(&mut link, 2, 50);
        let warnings = link.sort_warnings(&roster);
        assert_eq!(
            texts(&warnings),
            [(0, "Sort: Red three is locked on your target.")]
        );
        let [message, radio] = warnings[0].cues();
        assert!(matches!(&message, Cue::Message { seat, text }
            if *seat == SeatId(0) && text == "Sort: Red three is locked on your target."));
        match radio {
            Cue::Radio { seat, call } => {
                assert_eq!(seat, SeatId(0));
                assert_eq!(call.stems, [SORT_BEEP]);
                assert_eq!(call.route, crate::comms::Route::Direct);
                assert!(call.text.is_empty() && call.label.is_empty());
            }
            other => panic!("expected the beep, got {other:?}"),
        }
    }

    #[test]
    fn the_edge_fires_once_and_again_after_the_pair_lets_go() {
        let (mut link, roster) = fixture();
        lock(&mut link, 0, 50);
        lock(&mut link, 2, 50);
        assert_eq!(link.sort_warnings(&roster).len(), 1);
        assert!(link.warned().contains(&(0, 2, 50)));
        link.tick += 1;
        assert!(
            link.sort_warnings(&roster).is_empty(),
            "held locks warn once"
        );
        // Plane 2 lets go and the table forgets the pair.
        link.locks.remove(&2);
        link.tick += 1;
        assert!(link.sort_warnings(&roster).is_empty());
        assert!(link.warned().is_empty());
        // Locking again, after the seat's cooldown, warns again.
        lock(&mut link, 2, 50);
        link.tick += SORT_COOLDOWN_TICKS;
        assert_eq!(link.sort_warnings(&roster).len(), 1);
    }

    #[test]
    fn a_seat_hears_one_warning_in_ten_seconds() {
        let (mut link, roster) = fixture();
        lock(&mut link, 0, 50);
        lock(&mut link, 2, 50);
        assert_eq!(link.sort_warnings(&roster).len(), 1);
        // A second pair on another aircraft, one tick short of ten seconds
        // after the first: the pair is recorded, the seat is not told.
        lock(&mut link, 1, 60);
        lock(&mut link, 2, 60);
        link.locks.insert(
            0,
            Lock {
                target: 70,
                since: 0,
            },
        );
        link.tick += SORT_COOLDOWN_TICKS - 1;
        let warnings = link.sort_warnings(&roster);
        // Seat 1 is warned (its first); seat 0 holds nothing in common.
        assert_eq!(
            texts(&warnings),
            [(1, "Sort: Red three is locked on your target.")]
        );
        // Plane 2 had a second pair, (1, 2, 60), which the AI member makes
        // no seat hear. Seat 1 now waits ten seconds.
        lock(&mut link, 0, 80);
        lock(&mut link, 1, 80);
        link.locks.insert(
            2,
            Lock {
                target: 90,
                since: 0,
            },
        );
        link.tick += 1;
        let warnings = link.sort_warnings(&roster);
        assert_eq!(
            texts(&warnings),
            [(0, "Sort: Red two is locked on your target.")],
            "seat 1 was told a tick ago"
        );
        assert!(
            link.warned().contains(&(0, 1, 80)),
            "the pair is recorded all the same"
        );
    }

    #[test]
    fn a_pair_in_different_flights_or_on_different_aircraft_is_not_a_clash() {
        let (mut link, roster) = fixture();
        lock(&mut link, 0, 50);
        lock(&mut link, 3, 50);
        lock(&mut link, 1, 51);
        assert!(link.sort_warnings(&roster).is_empty());
        assert!(link.warned().is_empty());
    }

    #[test]
    fn a_dead_member_is_not_a_clash() {
        let (mut link, roster) = fixture();
        lock(&mut link, 0, 50);
        lock(&mut link, 2, 50);
        link.members[2].alive = false;
        assert!(link.sort_warnings(&roster).is_empty());
    }

    fn assigned(link: &mut DataLink, receiver: u32, target: u32, by: u32) {
        link.assignments.insert(
            receiver,
            Assignment {
                target,
                by,
                tick: 0,
                order: PlayerOrder::EngageMyTarget,
                acknowledged: false,
            },
        );
    }

    #[test]
    fn a_pair_the_lead_meant_is_not_warned_of() {
        // Both assigned the aircraft.
        let (mut link, roster) = fixture();
        lock(&mut link, 1, 50);
        lock(&mut link, 2, 50);
        assigned(&mut link, 1, 50, 0);
        assigned(&mut link, 2, 50, 0);
        assert!(link.sort_warnings(&roster).is_empty());
        // The lead locks what it assigned to a wingman.
        let (mut link, roster) = fixture();
        lock(&mut link, 0, 50);
        lock(&mut link, 1, 50);
        assigned(&mut link, 1, 50, 0);
        assert!(link.sort_warnings(&roster).is_empty());
        assert!(link.warned().is_empty());
        // Only one wingman was assigned it: the other's lock is a clash.
        let (mut link, roster) = fixture();
        lock(&mut link, 1, 50);
        lock(&mut link, 2, 50);
        assigned(&mut link, 1, 50, 0);
        assert_eq!(link.sort_warnings(&roster).len(), 1);
    }

    #[test]
    fn a_seat_with_no_radar_is_warned_like_any_other() {
        // John, 2026-10-05: only the radar scope's marks go without a radar.
        let (mut link, roster) = fixture();
        link.members[0].radar = false;
        lock(&mut link, 0, 50);
        lock(&mut link, 1, 50);
        let warnings = link.sort_warnings(&roster);
        assert_eq!(
            texts(&warnings),
            [
                (0, "Sort: Red two is locked on your target."),
                (1, "Sort: Red one is locked on your target.")
            ]
        );
        assert!(link.warned().contains(&(0, 1, 50)));
    }

    #[test]
    fn names_are_the_radios_words() {
        assert_eq!(name_of(0, 0), "Red one");
        assert_eq!(name_of(1, 3), "Blue four");
        assert_eq!(name_of(9, 0), "Flight 10 one");
    }
}
