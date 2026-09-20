//! Fitted fixed-step visual smoke. No collision, seeker or flight-force effects.
use crate::attitude::Vector;
use std::collections::VecDeque;

pub const MAX_PUFFS: usize = 8192;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Missile,
    Aircraft,
}
impl Kind {
    fn emission_interval(self) -> u64 {
        match self {
            Self::Missile => 2,
            Self::Aircraft => 3,
        }
    }
    pub fn lifetime(self) -> u16 {
        match self {
            Self::Missile => 480,
            Self::Aircraft => 960,
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
            Kind::Missile => (4., 6.),
            Kind::Aircraft => (8., 8.),
        };
        initial + growth * f64::from(self.age) / 120.
    }
    pub fn opacity(&self) -> f32 {
        0.65 * (1. - f32::from(self.age) / f32::from(self.kind.lifetime()))
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Smoke {
    pub puffs: VecDeque<Puff>,
    ticks: u64,
}
impl Smoke {
    /// Call exactly once per combat tick, even when the source list is empty.
    pub fn step(&mut self, sources: impl IntoIterator<Item = (Vector, Kind)>) {
        self.ticks += 1;
        for puff in &mut self.puffs {
            puff.age += 1;
            puff.position[1] += 2. / 120.;
        }
        self.puffs.retain(|p| p.age < p.kind.lifetime());
        for (position, kind) in sources {
            if !self.ticks.is_multiple_of(kind.emission_interval()) {
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
}
#[cfg(test)]
mod tests {
    use super::*;
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
            60
        );
        assert_eq!(
            smoke
                .puffs
                .iter()
                .filter(|p| p.kind == Kind::Aircraft)
                .count(),
            40
        );
        assert!((smoke.puffs[0].position[1] - 1000. - 118. * 2. / 120.).abs() < 1e-8);
        for (kind, spacing) in [(Kind::Missile, 20.), (Kind::Aircraft, 15.)] {
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
        smoke.step([]);
        smoke.step((0..MAX_PUFFS + 4).map(|i| ([i as f64, 0., 0.], Kind::Missile)));
        assert_eq!(smoke.puffs.len(), MAX_PUFFS);
        assert_eq!(smoke.puffs[0].position[0], 4.);
    }
}
