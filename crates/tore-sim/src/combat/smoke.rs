//! Fitted fixed-step visual smoke. No collision, seeker or flight-force effects.
use crate::attitude::Vector;
use std::collections::{BTreeMap, VecDeque};

pub const CONTRAIL_LIFETIME_TICKS: u16 = 120 * 120;
pub const CONTRAIL_FADE_START_TICKS: u16 = 60 * 120;
/// All 30 Quick Mission aircraft, two engines each, ten puffs/s for two minutes.
pub const MAX_CONTRAIL_PUFFS: usize = 30 * 2 * 10 * 120;

/// Opinionated onset altitude in feet MSL. Stable per aircraft and sortie so
/// visual randomness is reproducible and never flickers between simulation ticks.
pub fn contrail_altitude_ft(sortie: u64, aircraft: u32) -> f64 {
    let mut value = sortie
        .wrapping_add(u64::from(aircraft).wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_add(0x632b_e59b_d9b4_e019);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    30000. + 5000. * (value >> 11) as f64 / ((1_u64 << 53) - 1) as f64
}

pub const MAX_PUFFS: usize = 8192;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Missile,
    Aircraft,
    Contrail,
}
impl Kind {
    pub fn lifetime(self) -> u16 {
        match self {
            Self::Missile => 480,
            Self::Aircraft => 960,
            Self::Contrail => CONTRAIL_LIFETIME_TICKS,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Puff {
    pub position: Vector,
    pub kind: Kind,
    pub age: u16,
}
impl Puff {
    pub fn radius(&self) -> f64 {
        let (initial, growth) = match self.kind {
            Kind::Missile | Kind::Contrail => (2., 3.),
            Kind::Aircraft => (8., 8.),
        };
        let seconds = f64::from(self.age) / 120.;
        initial
            + growth
                * if self.kind == Kind::Contrail {
                    seconds.min(4.)
                } else {
                    seconds
                }
    }
    pub fn opacity(&self) -> f32 {
        if self.kind == Kind::Contrail {
            return 0.65
                * ((f32::from(CONTRAIL_LIFETIME_TICKS) - f32::from(self.age))
                    / f32::from(CONTRAIL_LIFETIME_TICKS - CONTRAIL_FADE_START_TICKS))
                .clamp(0., 1.);
        }
        0.65 * (1. - f32::from(self.age) / f32::from(self.kind.lifetime()))
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Smoke {
    pub puffs: VecDeque<Puff>,
    ticks: u64,
    outlets: BTreeMap<u64, Vector>,
}
impl Smoke {
    /// Call exactly once per combat tick, even when the source list is empty.
    pub fn step(&mut self, sources: impl IntoIterator<Item = (Vector, Kind)>) {
        self.ticks += 1;
        for puff in &mut self.puffs {
            puff.age = puff.age.saturating_add(1);
            if puff.kind != Kind::Contrail {
                puff.position[1] += 2. / 120.;
            }
        }
        self.puffs.retain(|p| p.age < p.kind.lifetime());
        for (position, kind) in sources {
            if !self
                .ticks
                .is_multiple_of(if kind == Kind::Missile { 8 } else { 12 })
            {
                continue;
            }
            if self.puffs.len() == MAX_PUFFS {
                self.puffs.pop_front();
            }
            self.puffs.push_back(Puff {
                position,
                kind,
                age: 0,
            });
        }
    }
    /// App bridge supplies moving world-space engine outlets once per `step`.
    /// Source motion controls emission only; existing puffs expire by age.
    pub fn contrails(&mut self, sources: impl IntoIterator<Item = (u64, Vector)>) {
        let current: BTreeMap<_, _> = sources.into_iter().collect();
        if self.ticks.is_multiple_of(12) {
            for (&id, &position) in &current {
                if self
                    .outlets
                    .get(&id)
                    .is_none_or(|previous| *previous == position)
                {
                    continue;
                }
                if self.puffs.len() == MAX_CONTRAIL_PUFFS {
                    self.puffs.pop_front();
                }
                self.puffs.push_back(Puff {
                    position,
                    kind: Kind::Contrail,
                    age: 0,
                });
            }
        }
        self.outlets = current;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contrail_onset_is_bounded_stable_and_varies_by_aircraft_and_sortie() {
        for sortie in 0..4 {
            let altitudes: Vec<_> = (0..100)
                .map(|id| contrail_altitude_ft(sortie, id))
                .collect();
            assert!(altitudes.iter().all(|a| (30000. ..=35000.).contains(a)));
            assert!(altitudes.iter().any(|a| *a < 31000.));
            assert!(altitudes.iter().any(|a| *a > 34000.));
            for id in 0..100 {
                assert_eq!(altitudes[id as usize], contrail_altitude_ft(sortie, id));
                assert_ne!(altitudes[id as usize], contrail_altitude_ft(sortie + 1, id));
            }
        }
    }

    #[test]
    fn missile_size_is_halved_at_birth_and_during_growth() {
        let mut puff = Puff {
            position: [0.; 3],
            kind: Kind::Missile,
            age: 0,
        };
        assert_eq!(puff.radius(), 2.);
        puff.age = 240;
        assert_eq!(puff.radius(), 8.);
        puff.age = 479;
        assert_eq!(puff.radius(), (4. + 6. * 479. / 120.) * 0.5);
    }

    #[test]
    fn contrails_fade_after_one_minute_and_expire_at_two() {
        let mut smoke = Smoke::default();
        for tick in 1..=12 {
            smoke.step([]);
            smoke.contrails([
                (0, [0., 1000., f64::from(tick) * 5.]),
                (1, [10., 1000., f64::from(tick) * 20.]),
            ]);
        }
        assert_eq!(smoke.puffs.len(), 2);
        let positions: Vec<_> = smoke.puffs.iter().map(|p| p.position).collect();
        // Disappearing outlets must not accelerate the fade. These two puffs
        // were emitted at different speeds but have the same age and opacity.
        for age in 1..=14400 {
            smoke.step([]);
            smoke.contrails([]);
            if [480, 7200, 10800, 14399].contains(&age) {
                assert_eq!(
                    smoke.puffs.iter().map(|p| p.position).collect::<Vec<_>>(),
                    positions
                );
                let expected = match age {
                    10800 => 0.325,
                    14399 => 0.65 / 7200.,
                    _ => 0.65,
                };
                for puff in &smoke.puffs {
                    assert!((puff.opacity() - expected).abs() < 1e-6);
                }
            }
        }
        assert!(smoke.puffs.is_empty());
        assert!(smoke.outlets.is_empty());
        let mut puff = Puff {
            position: [0.; 3],
            kind: Kind::Contrail,
            age: 7201,
        };
        assert!(puff.opacity() < 0.65 && puff.opacity() > 0.64);
        puff.age = 14400;
        assert_eq!(puff.opacity(), 0.);
    }

    #[test]
    fn contrail_budget_keeps_the_fading_minute_for_a_full_mission() {
        assert_eq!(MAX_CONTRAIL_PUFFS, 60 * 10 * 120);
        let mut smoke = Smoke::default();
        smoke.puffs.resize(
            12000,
            Puff {
                position: [0.; 3],
                kind: Kind::Contrail,
                age: 7200,
            },
        );
        smoke.ticks = 11;
        smoke.outlets.insert(0, [0.; 3]);
        smoke.step([]);
        smoke.contrails([(0, [0., 0., 1.])]);
        assert_eq!(smoke.puffs.len(), 12001);
        assert_eq!(smoke.puffs[0].age, 7201);
        assert!(smoke.puffs[0].opacity() < 0.65);
        smoke.puffs.resize(
            MAX_CONTRAIL_PUFFS,
            Puff {
                position: [0.; 3],
                kind: Kind::Contrail,
                age: 14399,
            },
        );
        smoke.step([]);
        assert_eq!(smoke.puffs.len(), 12001);
    }

    #[test]
    fn independent_fixed_step_cadence_lifetime_and_capacity() {
        let mut smoke = Smoke::default();
        for tick in 1..=120 {
            smoke.step([
                ([0., 1000., f64::from(tick) * 10.], Kind::Missile),
                ([0., 2000., f64::from(tick) * 5.], Kind::Aircraft),
            ]);
        }
        assert_eq!(
            smoke
                .puffs
                .iter()
                .filter(|p| p.kind == Kind::Missile)
                .count(),
            15
        );
        assert_eq!(
            smoke
                .puffs
                .iter()
                .filter(|p| p.kind == Kind::Aircraft)
                .count(),
            10
        );
        assert!((smoke.puffs[0].position[1] - 1000. - 112. * 2. / 120.).abs() < 1e-8);
        for (kind, spacing) in [(Kind::Missile, 80.), (Kind::Aircraft, 60.)] {
            let positions: Vec<_> = smoke.puffs.iter().filter(|p| p.kind == kind).collect();
            assert!(positions.windows(2).all(|pair| {
                (pair[1].position[2] - pair[0].position[2] - spacing).abs() < 1e-8
            }));
        }
        for _ in 0..480 {
            smoke.step([]);
        }
        assert!(smoke.puffs.iter().all(|p| p.kind == Kind::Aircraft));
        for _ in 0..480 {
            smoke.step([]);
        }
        assert!(smoke.puffs.is_empty());
        for _ in 0..7 {
            smoke.step([]);
        }
        smoke.step((0..MAX_PUFFS + 4).map(|i| ([i as f64, 0., 0.], Kind::Missile)));
        assert_eq!(smoke.puffs.len(), MAX_PUFFS);
        assert_eq!(smoke.puffs[0].position[0], 4.);
    }
}
