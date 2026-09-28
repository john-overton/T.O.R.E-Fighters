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
    /// The column over a burning crash site (docs/spec/explosions.md).
    Burning,
}
impl Kind {
    pub fn lifetime(self) -> u16 {
        match self {
            Self::Missile => 480,
            Self::Aircraft => 960,
            Self::Contrail => CONTRAIL_LIFETIME_TICKS,
            Self::Burning => BURNING_LIFETIME_TICKS,
        }
    }
    /// How fast a puff drifts upward, in feet per second.
    pub fn rise(self) -> f64 {
        match self {
            Self::Contrail => 0.,
            Self::Burning => BURNING_RISE_FPS,
            Self::Missile | Self::Aircraft => 2.,
        }
    }
}
/// A crash-site column (John, 2026-09-28): ten puffs a second from 20 feet
/// above the fire, each rising at 20 knots within 5 degrees of straight up
/// and carried by the wind, full until 1,300 feet above the ground and gone
/// at 1,500.
pub const BURNING_RISE_FPS: f64 = 20. * 6076.12 / 3600.;
pub const BURNING_SPREAD_DEGREES: f64 = 5.;
pub const BURNING_SOURCE_FT: f64 = 20.;
const BURNING_FADE_FT: [f64; 2] = [1300. - BURNING_SOURCE_FT, 1500. - BURNING_SOURCE_FT];
pub const BURNING_LIFETIME_TICKS: u16 = (BURNING_FADE_FT[1] / BURNING_RISE_FPS * 120.) as u16 + 1;

/// A column puff's sideways drift in feet per second: a direction within
/// [`BURNING_SPREAD_DEGREES`] of vertical, drawn from its exact release
/// point (to the 1/32 foot a recording keeps) so a replay drifts it the same.
pub fn burning_drift(release: Vector) -> [f64; 2] {
    let mut value = 0x51_7cc1_b727_220a_u64;
    for axis in release {
        value = (value ^ ((axis * 32.).round() as i64 as u64)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value ^= value >> 29;
    }
    let unit = |bits: u64| (bits >> 11) as f64 / (1_u64 << 53) as f64;
    let spread = BURNING_SPREAD_DEGREES.to_radians().tan() * BURNING_RISE_FPS;
    // Evenly over the cone's circle, not bunched at its center.
    let reach = spread * unit(value).sqrt();
    let (sin, cos) =
        (std::f64::consts::TAU * unit(value.wrapping_mul(0x9e37_79b9_7f4a_7c15))).sin_cos();
    [reach * cos, reach * sin]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Puff {
    pub position: Vector,
    pub kind: Kind,
    pub age: u16,
    /// Sideways drift, feet per second; only a crash-site column drifts.
    pub drift: [f64; 2],
}
impl Puff {
    pub fn radius(&self) -> f64 {
        let (initial, growth) = match self.kind {
            Kind::Missile | Kind::Contrail => (2., 3.),
            Kind::Aircraft => (8., 8.),
            Kind::Burning => (8., 3.),
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
        if self.kind == Kind::Burning {
            let risen = f64::from(self.age) / 120. * BURNING_RISE_FPS;
            let [start, end] = BURNING_FADE_FT;
            return 0.5 * ((end - risen) / (end - start)).clamp(0., 1.) as f32;
        }
        0.65 * (1. - f32::from(self.age) / f32::from(self.kind.lifetime()))
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Smoke {
    pub puffs: VecDeque<Puff>,
    /// The mission wind, world feet per second, set by the host. Every puff
    /// and contrail puff is carried with it (John, 2026-09-28).
    pub wind: Vector,
    ticks: u64,
    outlets: BTreeMap<u64, Vector>,
}
impl Smoke {
    /// Call exactly once per combat tick, even when the source list is empty.
    pub fn step(&mut self, sources: impl IntoIterator<Item = (Vector, Kind)>) {
        self.ticks += 1;
        for puff in &mut self.puffs {
            puff.age = puff.age.saturating_add(1);
            puff.position[1] += puff.kind.rise() / 120.;
            puff.position[0] += puff.drift[0] / 120.;
            puff.position[2] += puff.drift[1] / 120.;
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
            let wind = [self.wind[0], self.wind[2]];
            let (position, drift) = if kind == Kind::Burning {
                // Each release a little apart, so each draws its own drift.
                let n = self.ticks as f64;
                let release = [
                    position[0] + ((n * 0.618_034).fract() - 0.5) * 4.,
                    position[1],
                    position[2] + ((n * 0.414_214).fract() - 0.5) * 4.,
                ];
                let cone = burning_drift(release);
                (release, [cone[0] + wind[0], cone[1] + wind[1]])
            } else {
                (position, wind)
            };
            self.puffs.push_back(Puff {
                position,
                kind,
                age: 0,
                drift,
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
                    drift: [self.wind[0], self.wind[2]],
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
    fn a_crash_column_rises_at_20_knots_in_a_5_degree_cone_with_the_wind_and_fades_by_1500_ft() {
        let fire = [1000., 500., 2000.];
        let source = [fire[0], fire[1] + BURNING_SOURCE_FT, fire[2]];
        let mut smoke = Smoke::default();
        for _ in 0..6000 {
            smoke.step([(source, Kind::Burning)]);
        }
        // Ten a second, each gone by 1,500 feet above the ground.
        assert_eq!(
            smoke.puffs.len(),
            usize::from(BURNING_LIFETIME_TICKS).div_ceil(12)
        );
        assert!((BURNING_RISE_FPS - 33.756).abs() < 0.001);
        let cone = BURNING_SPREAD_DEGREES.to_radians().tan();
        let mut widest: f64 = 0.;
        for puff in &smoke.puffs {
            let risen = puff.position[1] - source[1];
            assert!((risen - f64::from(puff.age) / 120. * BURNING_RISE_FPS).abs() < 1e-6);
            let side = (puff.position[0] - source[0]).hypot(puff.position[2] - source[2]);
            assert!(side <= risen * cone + 2.9, "{side} {risen}");
            widest = widest.max(side / risen.max(1.));
            let above_ground = puff.position[1] - fire[1];
            let expected = 0.5 * ((1500. - above_ground) / 200.).clamp(0., 1.) as f32;
            assert!((puff.opacity() - expected).abs() < 1e-3, "{above_ground}");
        }
        assert!(widest > cone * 0.6, "puffs spread through the cone");
        assert!(smoke.puffs.iter().all(|p| p.position[1] - fire[1] < 1500.));
        assert!(smoke.puffs.iter().any(|p| p.position[1] - fire[1] > 1400.));
        // The wind carries every puff downwind at its own speed.
        let mut windy = Smoke {
            wind: [30., 0., -10.],
            ..Smoke::default()
        };
        for _ in 0..600 {
            windy.step([(source, Kind::Burning)]);
        }
        let oldest = windy.puffs.front().unwrap();
        let seconds = f64::from(oldest.age) / 120.;
        let x = oldest.position[0] - source[0];
        assert!((x - 30. * seconds).abs() <= seconds * BURNING_RISE_FPS * cone + 2.1);
        // A replay can recover a puff's cone from where it was released.
        let release = [1234.5, 520., -77.25];
        assert_eq!(burning_drift(release), burning_drift(release));
    }

    #[test]
    fn all_smoke_and_contrails_drift_with_the_wind() {
        let mut smoke = Smoke {
            wind: [20., 0., -5.],
            ..Smoke::default()
        };
        for _ in 0..240 {
            smoke.step([
                ([0., 1000., 0.], Kind::Aircraft),
                ([0., 2000., 0.], Kind::Missile),
            ]);
            smoke.contrails([(0, [0., 30000., smoke.ticks as f64])]);
        }
        for puff in &smoke.puffs {
            let seconds = f64::from(puff.age) / 120.;
            assert!((puff.position[0] - 20. * seconds).abs() < 1e-6, "{puff:?}");
            assert_eq!(puff.drift, [20., -5.]);
        }
        let kinds: std::collections::BTreeSet<_> = smoke
            .puffs
            .iter()
            .map(|p| format!("{:?}", p.kind))
            .collect();
        assert_eq!(kinds.len(), 3);
    }

    #[test]
    fn missile_size_is_halved_at_birth_and_during_growth() {
        let mut puff = Puff {
            position: [0.; 3],
            kind: Kind::Missile,
            age: 0,
            drift: [0.; 2],
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
            drift: [0.; 2],
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
                drift: [0.; 2],
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
                drift: [0.; 2],
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
