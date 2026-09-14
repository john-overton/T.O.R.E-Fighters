//! Resolved FA HARDCanLoad/StoreWeight arithmetic; no string lookup in updates.
use super::invalid;
use tore_formats::{
    Result,
    aircraft::Hardpoint,
    weapons::{Countermeasures, Tank, Weapon},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreKind {
    Projectile {
        flags: u32,
        signature: u8,
        pod_count: i16,
        phoenix_family: bool,
    },
    Tank {
        flags: u8,
    },
    Equipment {
        flags: u8,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Store {
    pub kind: StoreKind,
    pub weight: i32,
}
impl Store {
    pub fn weapon(w: &Weapon) -> Self {
        Self {
            kind: StoreKind::Projectile {
                flags: w.flags,
                signature: w.seeker.signature,
                pod_count: w.burst.projectiles_in_pod,
                phoenix_family: w.source.starts_with("AIM54") || w.source.starts_with("AAML"),
            },
            weight: w.weight,
        }
    }
    pub fn tank(t: Tank) -> Self {
        Self {
            kind: StoreKind::Tank { flags: t.flags },
            weight: i32::from(t.empty_weight).wrapping_add(t.fuel_weight),
        }
    }
    pub fn countermeasures(c: Countermeasures) -> Self {
        Self {
            kind: StoreKind::Equipment { flags: c.flags },
            weight: i32::from(c.weight),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Station {
    pub flags: u16,
    pub capacity: i16,
    pub weight_class: u8,
}
impl Station {
    pub fn from_source(h: &Hardpoint) -> Result<Self> {
        Ok(Self {
            flags: h.flags as u16,
            capacity: i16::try_from(h.count).map_err(|_| invalid("station count exceeds word"))?,
            weight_class: u8::try_from(h.weight_class)
                .map_err(|_| invalid("station weight class exceeds byte"))?,
        })
    }
    /// FA 0x452980. Caller resolves the exact native default-name comparison
    /// once when constructing a compatibility matrix. Return is native capacity,
    /// not a complete mission/year/stock availability decision.
    pub fn allowed_count(self, store: Store, matches_default: bool) -> i32 {
        let h = self.flags;
        let allowed = if h & 8 != 0 {
            matches_default
        } else if matches_default {
            true
        } else {
            match store.kind {
                StoreKind::Projectile {
                    flags,
                    signature,
                    pod_count,
                    phoenix_family,
                } => {
                    if flags & 2 == 0
                        || (pod_count > 1 && h & 0x400 == 0)
                        || (h & 0x1000 != 0 && flags & 0x10000 != 0 && flags & 0x20000 == 0)
                    {
                        false
                    } else {
                        match signature {
                            1 | 2 => h & 0x80 != 0,
                            3 if phoenix_family => h & 0x10 != 0,
                            3 if flags & 0x200 != 0 => h & 0x40 != 0,
                            3 => h & 0x20 != 0,
                            _ => h & 0x100 != 0,
                        }
                    }
                }
                StoreKind::Tank { flags } => (h & 1 == 0 || flags & 1 != 0) && h & 0x200 != 0,
                StoreKind::Equipment { flags } => (h & 1 == 0 || flags & 1 != 0) && h & 0x400 != 0,
            }
        };
        if !allowed {
            return 0;
        }
        let weight = store.weight as i16;
        if self.weight_class == 0 || weight == 0 {
            i32::from(self.capacity)
        } else {
            (i32::from(self.weight_class) * 100 / i32::from(weight)).min(i32::from(self.capacity))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_defaults_fixed_stations_pod_masks_and_weight_caps() {
        let missile = Store {
            kind: StoreKind::Projectile {
                flags: 2,
                signature: 2,
                pod_count: 1,
                phoenix_family: false,
            },
            weight: 300,
        };
        let s = Station {
            flags: 0x80,
            capacity: 4,
            weight_class: 10,
        };
        assert_eq!(s.allowed_count(missile, false), 3);
        assert_eq!(Station { flags: 8, ..s }.allowed_count(missile, false), 0);
        assert_eq!(Station { flags: 8, ..s }.allowed_count(missile, true), 3);
        let pod = Store {
            kind: StoreKind::Projectile {
                flags: 2,
                signature: 0,
                pod_count: 20,
                phoenix_family: false,
            },
            weight: 300,
        };
        assert_eq!(Station { flags: 0x100, ..s }.allowed_count(pod, false), 0);
        assert_eq!(Station { flags: 0x500, ..s }.allowed_count(pod, false), 3);
    }
    #[test]
    fn special_radar_family_and_tanks_keep_distinct_source_rules() {
        let s = Station {
            flags: 0x20,
            capacity: 2,
            weight_class: 0,
        };
        let w = Store {
            kind: StoreKind::Projectile {
                flags: 2,
                signature: 3,
                pod_count: 1,
                phoenix_family: true,
            },
            weight: 100,
        };
        assert_eq!(s.allowed_count(w, false), 0);
        assert_eq!(Station { flags: 0x10, ..s }.allowed_count(w, false), 2);
        let tank = Store::tank(Tank {
            empty_weight: 100,
            fuel_weight: 900,
            flags: 1,
        });
        assert_eq!(
            Station {
                flags: 0x201,
                weight_class: 10,
                ..s
            }
            .allowed_count(tank, false),
            1
        );
        assert_eq!(Station { flags: 0x400, ..s }.allowed_count(tank, false), 0);
    }
}
