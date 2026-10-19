//! Read-only low-pass observer. Counts population changes inside the detail
//! circle, excluding normal entry across its boundary and camera frustum edges.
use std::collections::BTreeMap;
pub type Id = [u32; 4];
pub type Population = BTreeMap<Id, f32>;
#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub near: usize,
    pub appeared: usize,
    pub faded: usize,
}
#[derive(Default)]
pub struct Monitor {
    previous: Option<(Population, [f64; 2])>,
}
impl Monitor {
    pub fn sample(&mut self, population: Population, eye: [f64; 2], full: f32) -> Counts {
        let mut counts = Counts {
            near: population.len(),
            faded: population.values().filter(|&&a| a < 0.999).count(),
            appeared: 0,
        };
        if let Some((previous, old_eye)) = &self.previous {
            counts.appeared = population
                .keys()
                .filter(|id| {
                    let x = f64::from(f32::from_bits(id[0]));
                    let z = f64::from(f32::from_bits(id[2]));
                    !previous.contains_key(*id)
                        && (x - old_eye[0]).hypot(z - old_eye[1]) < f64::from(full) - 1.
                })
                .count();
        }
        self.previous = Some((population, eye));
        counts
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observer_counts_interior_appearance_but_not_boundary_entry() {
        let id = |x: f32| [x.to_bits(), 0, 0, 0];
        let mut monitor = Monitor::default();
        monitor.sample(BTreeMap::from([(id(0.), 1.)]), [0., 0.], 100.);
        let counts = monitor.sample(
            BTreeMap::from([(id(0.), 1.), (id(10.), 0.7), (id(105.), 1.)]),
            [10., 0.],
            100.,
        );
        assert_eq!((counts.near, counts.appeared, counts.faded), (3, 1, 1));
    }
}
