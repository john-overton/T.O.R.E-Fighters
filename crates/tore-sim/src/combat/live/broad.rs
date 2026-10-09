//! A conservative first pass for the projectile contact search.
//!
//! Every round tests every aircraft and ground object it could touch. With up
//! to [`super::MAX_PROJECTILES`] rounds in the air, testing each round against
//! every target costs more than a tick has (agent decision, 2026-10-09, when
//! the cap rose from 256 to 5,000). This pass only skips pairs whose exact
//! test cannot succeed: it never reorders the pairs it keeps, so the search
//! finds the same first contact, in the same order, as testing every target.
//!
//! Each target is given a box in world space that holds every point its exact
//! test could report, for any round, as the round and the target both move
//! over the tick. A round's swept segment, widened by its own fuze radius,
//! that misses the box cannot touch the target. Targets whose volume cannot be
//! bounded (non-finite values, a skewed basis) are always kept.

use crate::{
    attitude::{Basis, Vector, dot},
    cheats::EASY_AIMING_HITBOX,
};

/// The furthest a point of the gun contact volume lies from the aircraft's
/// position, in multiples of its scaled radius: the corner of the wing boxes
/// is about 1.39 (`aircraft_contact`). Rounded up for safety.
const GUN_VOLUME_REACH: f64 = 1.5;
/// Added to every reach, in feet: far more than any rounding the exact tests
/// can show.
const MARGIN_FT: f64 = 1.;

/// An axis-aligned box: minimum then maximum corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Bounds {
    pub lo: Vector,
    pub hi: Vector,
}

impl Bounds {
    /// The segment's box widened by `reach`, or `None` when a value is not
    /// finite (the exact tests decide those).
    pub fn segment(from: Vector, to: Vector, reach: f64) -> Option<Self> {
        let finite = from.iter().chain(&to).all(|v| v.is_finite()) && reach.is_finite();
        finite.then(|| Self {
            lo: std::array::from_fn(|i| from[i].min(to[i]) - reach),
            hi: std::array::from_fn(|i| from[i].max(to[i]) + reach),
        })
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        (0..3).all(|i| self.lo[i] <= other.hi[i] && other.lo[i] <= self.hi[i])
    }
}

/// What the broad pass needs to know of one target.
#[derive(Clone, Copy, Debug)]
pub(super) struct Body {
    /// Where the target was before this tick's movement.
    pub previous: Vector,
    pub position: Vector,
    pub basis: Basis,
    pub radius: f64,
    /// The fixed solid box a ground object is tested against instead, as its
    /// centre and its half sizes along its own axes.
    pub solid: Option<(Vector, Vector)>,
}

impl Body {
    /// The world box holding everything this target's exact test can touch,
    /// or `None` when it cannot be bounded.
    pub fn bounds(&self) -> Option<Bounds> {
        if let Some((center, half)) = self.solid {
            // The oriented box lies inside the sphere through its corners.
            let reach = dot(half, half).sqrt() + MARGIN_FT;
            return Bounds::segment(center, center, reach);
        }
        if !self.radius.is_finite() || !orthonormal(self.basis) {
            return None;
        }
        let scaled = self.radius.abs() * EASY_AIMING_HITBOX.max(1.);
        // A sphere test reaches the scaled radius (the round adds its fuze);
        // the gun volume reaches its boxes, never under one foot.
        let reach = scaled.max(GUN_VOLUME_REACH * scaled.max(1.)) + MARGIN_FT;
        Bounds::segment(self.previous, self.position, reach)
    }
}

/// Whether the basis is a rotation, so distances in its frame are distances
/// in the world.
fn orthonormal(basis: Basis) -> bool {
    let axes = [basis.right, basis.up, basis.forward];
    (0..3).all(|i| {
        (0..3).all(|j| {
            let expected = if i == j { 1. } else { 0. };
            (dot(axes[i], axes[j]) - expected).abs() <= 1e-6
        })
    })
}

/// The targets of one tick, ordered along x for the search.
#[derive(Debug, Default)]
pub(super) struct Broad {
    /// Each bounded target's box and index, by the box's low x.
    boxes: Vec<(Bounds, usize)>,
    /// The widest box along x, which bounds how far back the search starts.
    widest: f64,
    /// Targets every round tests.
    always: Vec<usize>,
    count: usize,
}

impl Broad {
    pub fn new(bodies: impl IntoIterator<Item = Body>) -> Self {
        let mut broad = Self::default();
        for (index, body) in bodies.into_iter().enumerate() {
            broad.count += 1;
            match body.bounds() {
                Some(bounds) => {
                    broad.widest = broad.widest.max(bounds.hi[0] - bounds.lo[0]);
                    broad.boxes.push((bounds, index));
                }
                None => broad.always.push(index),
            }
        }
        broad
            .boxes
            .sort_by(|a, b| a.0.lo[0].total_cmp(&b.0.lo[0]).then(a.1.cmp(&b.1)));
        broad
    }

    /// The targets a round may touch, in ascending index order, from its
    /// swept segment widened by its fuze ([`Bounds::segment`]). With `None`
    /// (a round the pass cannot bound) every target is returned.
    pub fn candidates(&self, segment: Option<Bounds>, out: &mut Vec<usize>) {
        out.clear();
        let Some(segment) = segment else {
            out.extend(0..self.count);
            return;
        };
        out.extend_from_slice(&self.always);
        let start = self
            .boxes
            .partition_point(|(b, _)| b.lo[0] < segment.lo[0] - self.widest);
        for (bounds, index) in &self.boxes[start..] {
            if bounds.lo[0] > segment.hi[0] {
                break;
            }
            if bounds.overlaps(&segment) {
                out.push(*index);
            }
        }
        out.sort_unstable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(position: Vector, radius: f64) -> Body {
        Body {
            previous: position,
            position,
            basis: Basis::new(0.3, 0.2, 0.1),
            radius,
            solid: None,
        }
    }

    fn all(broad: &Broad, from: Vector, to: Vector, fuze: f64) -> Vec<usize> {
        let mut out = Vec::new();
        broad.candidates(Bounds::segment(from, to, fuze), &mut out);
        out
    }

    #[test]
    fn keeps_near_targets_in_index_order_and_drops_far_ones() {
        let broad = Broad::new([
            body([5000., 0., 0.], 20.),
            body([0., 0., 0.], 20.),
            body([10., 5., 0.], 20.),
            body([0., 4000., 0.], 20.),
        ]);
        assert_eq!(all(&broad, [-50., 0., 0.], [50., 0., 0.], 0.), [1, 2]);
        // A fuze widens the round's reach.
        assert_eq!(all(&broad, [0., 3900., 0.], [0., 3910., 0.], 100.), [3]);
        assert!(all(&broad, [0., 3900., 0.], [0., 3910., 0.], 0.).is_empty());
    }

    #[test]
    fn a_moving_target_is_bounded_over_its_whole_move() {
        let mut moving = body([1000., 0., 0.], 10.);
        moving.previous = [0., 0., 0.];
        let broad = Broad::new([moving]);
        assert_eq!(all(&broad, [500., 0., 0.], [501., 0., 0.], 0.), [0]);
    }

    #[test]
    fn unbounded_targets_and_rounds_are_always_tested() {
        let mut skewed = body([1e6, 0., 0.], 10.);
        skewed.basis.right = [2., 0., 0.];
        let mut nan = body([0., 0., 0.], f64::NAN);
        nan.previous = [0., 0., 0.];
        let broad = Broad::new([body([1e6, 1e6, 1e6], 10.), skewed, nan]);
        assert_eq!(all(&broad, [0., 0., 0.], [1., 0., 0.], 0.), [1, 2]);
        assert_eq!(all(&broad, [f64::NAN, 0., 0.], [1., 0., 0.], 0.), [0, 1, 2]);
    }

    #[test]
    fn a_wide_ground_box_is_found_from_its_far_end() {
        let mut runway = body([0., 0., 0.], 0.);
        runway.solid = Some(([0., 0., 0.], [5000., 10., 100.]));
        let broad = Broad::new([body([-20_000., 0., 0.], 10.), runway]);
        assert_eq!(all(&broad, [4900., 5., 0.], [4910., 5., 0.], 0.), [1]);
    }

    /// The pass keeps every pair the exact tests can report, for gun rounds
    /// against aircraft volumes and fuzed rounds against spheres.
    #[test]
    fn every_exact_contact_is_kept() {
        let mut seed = 0x2545_f491_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 20_000) as f64 / 100. - 100.
        };
        let mut checked = 0;
        for _ in 0..20_000 {
            let previous = [next(), next(), next()];
            let position = [next(), next(), next()];
            let radius = next().abs() / 4.;
            let basis = Basis::new(next(), next() / 50., next());
            let target = Body {
                previous,
                position,
                basis,
                radius,
                solid: None,
            };
            let broad = Broad::new([target]);
            let from = [next(), next(), next()];
            let to = [next(), next(), next()];
            let fuze = next().abs() / 10.;
            let kept = !all(&broad, from, to, fuze).is_empty();
            for hitbox in [1., EASY_AIMING_HITBOX] {
                let relative = std::array::from_fn(|i| from[i] + position[i] - previous[i]);
                let gun =
                    super::super::aircraft_contact(relative, to, position, basis, radius * hitbox);
                let start = std::array::from_fn(|i| from[i] - previous[i]);
                let end = std::array::from_fn(|i| to[i] - position[i]);
                let sphere = super::super::segment_sphere(start, end, radius * hitbox + fuze);
                if gun.is_some() || sphere.is_some() {
                    checked += 1;
                    assert!(kept, "a contact was dropped: {target:?} {from:?} {to:?}");
                }
            }
        }
        assert!(
            checked > 100,
            "too few contacts to mean anything: {checked}"
        );
    }

    /// The same for a ground object's solid box, at any orientation.
    #[test]
    fn every_ground_box_contact_is_kept() {
        let mut seed = 0x9e37_79b9_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 20_000) as f64 / 100. - 100.
        };
        let mut checked = 0;
        for _ in 0..20_000 {
            let solid = crate::airport::OrientedBox {
                center: [next(), next(), next()],
                half: [
                    next().abs() + 0.1,
                    next().abs() / 4. + 0.1,
                    next().abs() + 0.1,
                ],
                heading: next(),
                pitch: next() / 50.,
                bank: next() / 50.,
            };
            let mut target = body([1e7, 1e7, 1e7], 1.);
            target.solid = Some((solid.center, solid.half));
            let broad = Broad::new([target]);
            let from = [next(), next(), next()];
            let to = [next(), next(), next()];
            if solid.segment_fraction(from, to).is_some() {
                checked += 1;
                assert_eq!(all(&broad, from, to, 0.), [0], "{solid:?} {from:?} {to:?}");
            }
        }
        assert!(
            checked > 100,
            "too few contacts to mean anything: {checked}"
        );
    }
}
