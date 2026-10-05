//! Revival: a player whose plane is lost flies again in a new aircraft of the
//! same type, just outside the battle. Stage F phase 2; see
//! docs/ARCHITECTURE.md, "Death, revival and lives".
//!
//! Slice F2-0 adds the types the host, the wire and the mission core share:
//! where a revived plane appears ([`Spawn`]) and what it may carry
//! ([`RevivalWeapons`]). The revival point, the weapons rule and the new
//! plane itself are slice F2-V's: until it lands,
//! [`super::MissionCommand::Revive`] is refused by the step.

use crate::mission::LoadoutSpec;

/// Where and how a revived plane appears, which the host chooses and every
/// client's copy of the mission repeats: the host's
/// [`super::MissionCommand::Revive`] and the wire's Spawned message carry it.
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    /// The world position, feet.
    pub position: [f64; 3],
    /// The heading, radians.
    pub heading_rad: f64,
    /// The airspeed, feet a second.
    pub speed_fps: f64,
    /// The stores, already cut by the revival weapons rule, with full fuel.
    pub loadout: LoadoutSpec,
}

/// The King's `revive-weapons` setting: what a revived aircraft carries of
/// the loadout its player chose (or the standard load). The wire codes it in
/// the setting's value, in this order (docs/formats/net-protocol.md,
/// "Settings by number").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RevivalWeapons {
    /// The loadout whole.
    #[default]
    Missiles,
    /// Every air-to-air missile station emptied; air-to-ground missiles,
    /// bombs and the gun kept.
    NoMissiles,
    /// The gun alone.
    Guns,
    /// The gun alone, with half its rounds, rounded up.
    HalfGuns,
}

impl RevivalWeapons {
    /// Every rule, in the setting's value order.
    pub const ALL: [RevivalWeapons; 4] = [
        RevivalWeapons::Missiles,
        RevivalWeapons::NoMissiles,
        RevivalWeapons::Guns,
        RevivalWeapons::HalfGuns,
    ];

    /// The rule for the setting's value, if it names one.
    pub fn from_value(value: u32) -> Option<Self> {
        Self::ALL.get(usize::try_from(value).ok()?).copied()
    }

    /// The setting's value for the rule.
    pub fn value(self) -> u32 {
        match self {
            RevivalWeapons::Missiles => 0,
            RevivalWeapons::NoMissiles => 1,
            RevivalWeapons::Guns => 2,
            RevivalWeapons::HalfGuns => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RevivalWeapons;

    #[test]
    fn revival_weapons_values_round_trip() {
        for rule in RevivalWeapons::ALL {
            assert_eq!(RevivalWeapons::from_value(rule.value()), Some(rule));
        }
        assert_eq!(RevivalWeapons::from_value(4), None);
    }
}
