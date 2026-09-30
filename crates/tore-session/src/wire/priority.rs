//! Relevance bands and the priority accumulator: how often each client hears
//! about each entity (MULTIPLAYER.md, "Netcode numbers"; John, 2026-09-30:
//! 30 a second near, twice a second for the rest).
//!
//! Each entity's priority grows every snapshot by its band's weight and
//! resets when it is sent. An entity is **due** once its priority reaches the
//! snapshot rate: a near one every snapshot, a far one twice a second. The
//! host sends due entities in priority order until the entities' share of the
//! packet is full; what does not fit waits with its priority still growing,
//! so it goes first next time. A missile aimed at the player is always sent.
//!
//! *Agent decisions:* the weights are whole numbers (the snapshot rate for
//! the full-rate band and 2 for the rest, which is the table's 1 and 1/15 at
//! 30 a second, and twice a second at every rate); an entity is sent only
//! when due, so a far one costs its bytes twice a second even when the packet
//! has room; and an entity the connection has never been sent is due at
//! once.

use super::entity::EntityKind;
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE;

/// Beyond this many feet an entity is far, unless another rule keeps it
/// near: 20 nautical miles.
pub const NEAR_FT: f64 = 20. * FEET_PER_NAUTICAL_MILE;
/// A missile within this many feet is near: 10 nautical miles.
pub const MISSILE_NEAR_FT: f64 = 10. * FEET_PER_NAUTICAL_MILE;
/// The far band's weight per snapshot: twice a second whatever the rate.
pub const FAR_WEIGHT: u32 = 2;

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
            FAR_WEIGHT
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
        assert_eq!(far.weight(EntityKind::Aircraft, 30), 2);
        assert!(Relevance::NEAR.full_rate(EntityKind::Aircraft));
        assert_eq!(Relevance::NEAR.weight(EntityKind::Pilot, 30), 30);
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
}
