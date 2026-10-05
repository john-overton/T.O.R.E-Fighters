//! A host's side of introductions (slice J2, "Hole punching" in the
//! architecture guide): the Meet a master sends when a player asks to be
//! introduced, the punches it asks for and the Meet ack.
//!
//! On a Meet the host sends a transport [Punch](crate::packet::Punch) to each
//! of the player's addresses, five times 200 ms apart ([`PUNCH_COUNT`],
//! [`PUNCH_INTERVAL`]), and acknowledges the Meet. Leaving the host's
//! router, the punches open its mapping for the player's addresses, so the
//! player's Connect requests get in; a player whose router gave the host's
//! punches another port learns it from them. The host's transport answers
//! the Connect requests as it always does.
//!
//! *Agent decisions:*
//!
//! - The rendezvous sends the punches itself, on the game port beside the
//!   master's datagrams: it knows the game's protocol version from the
//!   build it lists, so the host loops (the dedicated server's and the
//!   hosting game's) need no change. The design's `Server::punch` is not
//!   needed.
//! - At most 10 Meets a second are acted on, counted over the last second;
//!   one over it is dropped unanswered, so the master's next try (250 ms
//!   later) may be acted on.
//! - A Meet repeated for an introduction already met (its ack was lost) is
//!   acknowledged again and never punched again: at most five punches per
//!   address per introduction. Introductions met are remembered for the
//!   master's 30 seconds, at most [`MAX_REMEMBERED`] of them.
//! - Addresses in the relayed prefix, port 0 and unspecified addresses are
//!   never punched; repeated addresses are punched once.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::Duration;

use super::candidate::canonical;
use super::packet::Meet;
use super::routed::is_relayed;
use super::{INTRODUCTION_LIFETIME, PUNCH_COUNT, PUNCH_INTERVAL};

/// Meets acted on in any one second, at most.
pub const MEETS_PER_SECOND: usize = 10;
/// Introductions remembered as met, at most (30 seconds of Meets at the
/// limit).
pub const MAX_REMEMBERED: usize = 300;

/// What became of a Meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetOutcome {
    /// Acted on: the punches are scheduled; acknowledge it.
    Punching,
    /// An introduction already met: acknowledge it again, punch nothing.
    Again,
    /// Over the limit of Meets a second: no answer.
    OverLimit,
}

#[derive(Debug, Clone)]
struct Punching {
    introduction: u64,
    addresses: Vec<SocketAddr>,
    next: Duration,
    left: u32,
}

/// The Meets a host has met and the punches still to send.
#[derive(Debug, Clone, Default)]
pub struct Meets {
    /// When each Meet of the last second was acted on.
    acted: VecDeque<Duration>,
    /// Introductions met, with when, oldest first.
    met: VecDeque<(u64, Duration)>,
    punching: Vec<Punching>,
}

impl Meets {
    /// A Meet from the master at `now`.
    pub fn receive(&mut self, now: Duration, meet: &Meet) -> MeetOutcome {
        self.forget(now);
        if self.met.iter().any(|(id, _)| *id == meet.introduction_id) {
            return MeetOutcome::Again;
        }
        if self.acted.len() >= MEETS_PER_SECOND {
            return MeetOutcome::OverLimit;
        }
        self.acted.push_back(now);
        if self.met.len() >= MAX_REMEMBERED {
            self.met.pop_front();
        }
        self.met.push_back((meet.introduction_id, now));
        let mut addresses: Vec<SocketAddr> = Vec::new();
        for candidate in &meet.candidates {
            let address = canonical(candidate.address);
            let punchable =
                address.port() != 0 && !address.ip().is_unspecified() && !is_relayed(address);
            if punchable && !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        if !addresses.is_empty() {
            self.punching.push(Punching {
                introduction: meet.introduction_id,
                addresses,
                next: now,
                left: PUNCH_COUNT,
            });
        }
        MeetOutcome::Punching
    }

    /// The punches due at `now`, each an address and the introduction id it
    /// carries.
    pub fn due(&mut self, now: Duration) -> Vec<(SocketAddr, u64)> {
        let mut due = Vec::new();
        for punching in &mut self.punching {
            while punching.left > 0 && punching.next <= now {
                due.extend(
                    punching
                        .addresses
                        .iter()
                        .map(|address| (*address, punching.introduction)),
                );
                punching.left -= 1;
                punching.next += PUNCH_INTERVAL;
            }
        }
        self.punching.retain(|p| p.left > 0);
        due
    }

    /// True while punches are still to go.
    pub fn punching(&self) -> bool {
        !self.punching.is_empty()
    }

    fn forget(&mut self, now: Duration) {
        while self
            .acted
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= Duration::from_secs(1))
        {
            self.acted.pop_front();
        }
        while self
            .met
            .front()
            .is_some_and(|(_, at)| now.saturating_sub(*at) >= INTRODUCTION_LIFETIME)
        {
            self.met.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::{Candidate, CandidateKind, MappingType};

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn meet(id: u64, addresses: &[&str]) -> Meet {
        Meet {
            introduction_id: id,
            mapping: MappingType::SamePort,
            candidates: addresses
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let kind = if i == 0 {
                        CandidateKind::Seen
                    } else {
                        CandidateKind::Local
                    };
                    Candidate::new(kind, a.parse().unwrap())
                })
                .collect(),
        }
    }

    #[test]
    fn five_punches_per_address_200_ms_apart() {
        let mut meets = Meets::default();
        let start = Duration::from_secs(50);
        let m = meet(7, &["203.0.113.9:40000", "192.168.1.9:40000"]);
        assert_eq!(meets.receive(start, &m), MeetOutcome::Punching);
        let mut sent: Vec<(Duration, SocketAddr, u64)> = Vec::new();
        let mut now = start;
        while now <= start + Duration::from_secs(3) {
            for (to, id) in meets.due(now) {
                sent.push((now, to, id));
            }
            now += ms(10);
        }
        assert!(!meets.punching());
        for address in ["203.0.113.9:40000", "192.168.1.9:40000"] {
            let times: Vec<Duration> = sent
                .iter()
                .filter(|(_, to, id)| *to == address.parse().unwrap() && *id == 7)
                .map(|(at, _, _)| *at - start)
                .collect();
            assert_eq!(times, [ms(0), ms(200), ms(400), ms(600), ms(800)]);
        }
        // The same Meet again (its ack was lost) is acknowledged, never
        // punched again.
        assert_eq!(meets.receive(now, &m), MeetOutcome::Again);
        assert!(meets.due(now + ms(1000)).is_empty());
    }

    #[test]
    fn at_most_ten_meets_a_second_are_acted_on() {
        let mut meets = Meets::default();
        let start = Duration::from_secs(5);
        let acted = (0..25)
            .filter(|i| {
                meets.receive(start + ms(*i * 20), &meet(*i, &["203.0.113.9:1"]))
                    == MeetOutcome::Punching
            })
            .count();
        assert_eq!(acted, MEETS_PER_SECOND);
        // A second after the first, there is room again.
        assert_eq!(
            meets.receive(start + ms(1000), &meet(100, &["203.0.113.9:1"])),
            MeetOutcome::Punching
        );
        // Only the acted Meets punch, at most five times each.
        let due = meets.due(start + ms(5000));
        let mut ids: Vec<u64> = due.iter().map(|(_, id)| *id).collect();
        ids.dedup();
        assert_eq!(ids.len(), MEETS_PER_SECOND + 1);
        assert_eq!(due.len(), (MEETS_PER_SECOND + 1) * PUNCH_COUNT as usize);
    }

    #[test]
    fn relayed_unspecified_and_repeated_addresses_are_never_punched() {
        let mut meets = Meets::default();
        let m = meet(
            1,
            &[
                "[100::1:0:7]:0",
                "0.0.0.0:5",
                "203.0.113.9:0",
                "203.0.113.9:7",
                "[::ffff:203.0.113.9]:7",
            ],
        );
        meets.receive(Duration::ZERO, &m);
        assert_eq!(
            meets.due(Duration::ZERO),
            [("203.0.113.9:7".parse().unwrap(), 1)]
        );
    }

    #[test]
    fn introductions_are_forgotten_after_30_seconds() {
        let mut meets = Meets::default();
        let m = meet(9, &["203.0.113.9:1"]);
        meets.receive(Duration::ZERO, &m);
        assert_eq!(meets.receive(ms(29_999), &m), MeetOutcome::Again);
        assert_eq!(
            meets.receive(INTRODUCTION_LIFETIME, &m),
            MeetOutcome::Punching
        );
    }
}
