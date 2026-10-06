//! Relevance bands and the priority accumulator: how often each client hears
//! about each entity (MULTIPLAYER.md, "Netcode numbers"; John, 2026-10-06:
//! every snapshot, 60 a second by default, near, and 4 a second for the
//! rest, raised from 30 and twice a second).
//!
//! Each entity's priority grows every snapshot by its band's weight and
//! resets when it is sent. An entity is **due** once its priority reaches the
//! snapshot rate: a near one every snapshot, a far one every
//! [`far_snapshots`] snapshots, 4 a second at most. The
//! host sends due entities in priority order until the entities' share of the
//! packet is full; what does not fit waits with its priority still growing,
//! so it goes first next time. A missile aimed at the player is always sent.
//!
//! *Agent decisions:* the weights are whole numbers (the snapshot rate for
//! the full-rate band and [`FAR_RATE`] for the rest, which is the table's 1
//! and 1/15 at 60 a second); since the priority starts again from nothing
//! when an entity is sent, a far one goes every `ceil(rate / 4)` snapshots,
//! evenly spaced: exactly 4 a second at 60, 40, 24, 20 and 12, 3.75 at 30
//! and 15, 3.33 at 10; an entity is sent only when due, so a far one costs
//! its bytes at its rate even when the packet has room; and an entity the
//! connection has never been sent is due at once.

use super::entity::EntityKind;
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE;

/// Beyond this many feet an entity is far, unless another rule keeps it
/// near: 20 nautical miles.
pub const NEAR_FT: f64 = 20. * FEET_PER_NAUTICAL_MILE;
/// A missile within this many feet is near: 10 nautical miles.
pub const MISSILE_NEAR_FT: f64 = 10. * FEET_PER_NAUTICAL_MILE;
/// The far band's weight per snapshot, and its most updates a second: 4
/// a second whatever the rate (John, 2026-10-06; twice a second before).
pub const FAR_RATE: u32 = 4;

/// Snapshots between two updates of a far entity at `ticks_per_snapshot`
/// ticks of 120 a second: the snapshot rate over [`FAR_RATE`], rounded up.
/// Both ends work it out from the ticks per snapshot the Accepted packet
/// carries, so it is on no wire of its own.
pub fn far_snapshots(ticks_per_snapshot: u32) -> u32 {
    let rate = (120 / ticks_per_snapshot.clamp(1, 120)).max(1);
    rate.div_ceil(FAR_RATE).max(1)
}

/// Ticks between two updates of a far entity: [`far_snapshots`] snapshots.
pub fn far_interval_ticks(ticks_per_snapshot: u32) -> u32 {
    far_snapshots(ticks_per_snapshot) * ticks_per_snapshot.clamp(1, 120)
}

/// What the caller knows about one entity for one connection: the band's
/// inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Relevance {
    /// Distance from the player's plane, feet.
    pub distance_ft: f64,
    /// A member of the player's own flight.
    pub own_flight: bool,
    /// A friendly sensor tracks it.
    pub friendly_tracked: bool,
    /// A missile aimed at the player's plane.
    pub aimed_at_player: bool,
    /// The player's view follows it (the target, wing, external or fly-by
    /// view's subject).
    pub viewed: bool,
}

impl Relevance {
    /// Everything at the full rate: for tests, and for an entity the caller
    /// wants at every snapshot.
    pub const NEAR: Self = Self {
        distance_ft: 0.,
        own_flight: false,
        friendly_tracked: false,
        aimed_at_player: false,
        viewed: false,
    };

    /// True for the full-rate band.
    pub fn full_rate(&self, kind: EntityKind) -> bool {
        self.own_flight
            || self.distance_ft <= NEAR_FT
            || self.friendly_tracked
            || self.aimed_at_player
            || (kind == EntityKind::Projectile && self.distance_ft <= MISSILE_NEAR_FT)
            || self.viewed
    }

    /// Never left out of a snapshot.
    pub fn forced(&self, kind: EntityKind) -> bool {
        kind == EntityKind::Projectile && self.aimed_at_player
    }

    /// The band's weight per snapshot at `snapshot_rate` snapshots a second.
    pub fn weight(&self, kind: EntityKind, snapshot_rate: u32) -> u32 {
        if self.full_rate(kind) {
            snapshot_rate
        } else {
            FAR_RATE.min(snapshot_rate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_follow_the_netcode_numbers() {
        let far = Relevance {
            distance_ft: NEAR_FT + 1.,
            ..Relevance::NEAR
        };
        assert!(!far.full_rate(EntityKind::Aircraft));
        assert_eq!(far.weight(EntityKind::Aircraft, 60), 4);
        assert_eq!(far.weight(EntityKind::Aircraft, 30), 4);
        assert!(Relevance::NEAR.full_rate(EntityKind::Aircraft));
        assert_eq!(Relevance::NEAR.weight(EntityKind::Pilot, 60), 60);
        for rule in [
            Relevance {
                own_flight: true,
                ..far
            },
            Relevance {
                friendly_tracked: true,
                ..far
            },
            Relevance {
                aimed_at_player: true,
                ..far
            },
            Relevance {
                viewed: true,
                ..far
            },
        ] {
            assert!(rule.full_rate(EntityKind::Aircraft));
        }
        let aimed = Relevance {
            aimed_at_player: true,
            ..far
        };
        assert!(aimed.forced(EntityKind::Projectile));
        assert!(!aimed.forced(EntityKind::Aircraft));
    }

    /// The far band's interval at every snapshot rate the host allows, as
    /// the accumulator spaces it: 4 a second at most, and at most the rate.
    #[test]
    fn the_far_band_is_four_a_second_at_most_at_every_rate() {
        let far = Relevance {
            distance_ft: NEAR_FT + 1.,
            ..Relevance::NEAR
        };
        let expected = [
            (10, 3, 36),
            (12, 3, 30),
            (15, 4, 32),
            (20, 5, 30),
            (24, 6, 30),
            (30, 8, 32),
            (40, 10, 30),
            (60, 15, 30),
        ];
        for (rate, snapshots, ticks) in expected {
            let tps = 120 / rate;
            assert_eq!(far_snapshots(tps), snapshots, "rate {rate}");
            assert_eq!(far_interval_ticks(tps), ticks, "rate {rate}");
            // The accumulator: grows by the weight, due at the rate, reset
            // to nothing when sent.
            let weight = far.weight(EntityKind::Aircraft, rate);
            let (mut priority, mut sent) = (0, Vec::new());
            for snapshot in 0..120 {
                priority += weight;
                if priority >= rate {
                    sent.push(snapshot);
                    priority = 0;
                }
            }
            assert!(
                sent.windows(2).all(|w| w[1] - w[0] == snapshots),
                "rate {rate}: {sent:?}"
            );
            assert!(snapshots * tps >= 30, "rate {rate}: over 4 a second");
        }
    }
}
