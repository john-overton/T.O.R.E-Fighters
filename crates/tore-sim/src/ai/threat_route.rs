//! A helicopter's way home past the threat rings it knows (slice H2, the
//! H4 follow-up; docs/spec/ai-strike.md "Helicopter threat awareness"). H4
//! found a helicopter shot down on its way home: "home" was a straight
//! line, whatever stood across it.
//!
//! - **Inside a ring** it knows, it first leaves by the shortest way out:
//!   straight away from the ring's centre to [`DETOUR_MARGIN_FT`] beyond its
//!   edge.
//! - **A ring across the way home**: it flies along the tangent from where it
//!   is to a circle [`DETOUR_MARGIN_FT`] outside the first ring across the
//!   line, on the side away from that ring's centre, and so round it; as soon
//!   as the line home is clear it flies straight home. Recomputed every
//!   step, which keeps it on the tangent and then on the circle.
//! - A ring around home itself cannot be flown round and is ignored here.
//!
//! A pure function of the rings and the two points: no memory, no dice. The
//! numbers are fitted (agent decisions, 2026-10-12).
use super::helicopter_threat::{Ring, clear};
use crate::attitude::Vector;

/// How far outside a ring's edge the way round it runs, ft (fitted).
pub const DETOUR_MARGIN_FT: f64 = 1_000.;
/// The point flown to is at least this far ahead, ft (fitted): far enough
/// that the steering never counts it reached.
pub const DETOUR_LEAD_FT: f64 = 3_000.;
/// How many rings in a row are tried for the first leg (fitted).
const MAX_ROUNDS: usize = 4;

fn horizontal(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

/// The point to fly toward first on the way from `own` to `home` (world
/// X/Z), or `None` when the straight line home is clear of every ring.
pub fn detour(rings: &[Ring], own: Vector, home: Vector) -> Option<[f64; 2]> {
    let rings: Vec<Ring> = rings
        .iter()
        .filter(|r| horizontal(r.centre, home) >= r.radius_ft)
        .copied()
        .collect();
    // Inside a ring: the shortest way out first.
    if let Some(ring) = rings
        .iter()
        .filter(|r| horizontal(r.centre, own) < r.radius_ft)
        .min_by(|a, b| {
            (a.radius_ft - horizontal(a.centre, own))
                .total_cmp(&(b.radius_ft - horizontal(b.centre, own)))
                .then(a.id.cmp(&b.id))
        })
    {
        let d = horizontal(ring.centre, own);
        let (ux, uz) = if d > 1. {
            ((own[0] - ring.centre[0]) / d, (own[2] - ring.centre[2]) / d)
        } else {
            let h = horizontal(own, home).max(1.);
            ((home[0] - own[0]) / h, (home[2] - own[2]) / h)
        };
        let out = ring.radius_ft + DETOUR_MARGIN_FT;
        return Some([ring.centre[0] + ux * out, ring.centre[2] + uz * out]);
    }
    if clear(&rings, own, home) {
        return None;
    }
    let mut target = home;
    let mut point = None;
    for _ in 0..MAX_ROUNDS {
        let Some(ring) = first_across(&rings, own, target) else {
            break;
        };
        let p = round(ring, own, home);
        point = Some(p);
        target = [p[0], own[1], p[1]];
    }
    point
}

/// The first ring across the line from `own` to `to`: the one whose centre
/// projects nearest `own` along it.
fn first_across(rings: &[Ring], own: Vector, to: Vector) -> Option<Ring> {
    let (dx, dz) = (to[0] - own[0], to[2] - own[2]);
    let length2 = (dx * dx + dz * dz).max(1.);
    rings
        .iter()
        .filter(|r| !clear(std::slice::from_ref(r), own, to))
        .min_by(|a, b| {
            let t =
                |r: &Ring| ((r.centre[0] - own[0]) * dx + (r.centre[2] - own[2]) * dz) / length2;
            t(a).total_cmp(&t(b)).then(a.id.cmp(&b.id))
        })
        .copied()
}

/// The point ahead on the way round `ring` from `own`, passing on the side
/// away from its centre as seen on the line to `home`.
fn round(ring: Ring, own: Vector, home: Vector) -> [f64; 2] {
    let radius = ring.radius_ft + DETOUR_MARGIN_FT;
    let (cx, cz) = (ring.centre[0] - own[0], ring.centre[2] - own[2]);
    let d = cx.hypot(cz).max(1.);
    // Bearings run clockwise from +Z. The centre right of the line home
    // (a negative cross product) is passed on the left, by turning the
    // bearing anticlockwise from it, and the other way round.
    let (hx, hz) = (home[0] - own[0], home[2] - own[2]);
    let cross = hx * cz - hz * cx;
    let side = if cross >= 0. { 1. } else { -1. };
    let to_centre = cx.atan2(cz);
    let (bearing, reach) = if d > radius {
        // The tangent to the circle.
        let alpha = (radius / d).asin();
        (to_centre + side * alpha, (d * d - radius * radius).sqrt())
    } else {
        // Already on the circle: along it.
        (
            to_centre + side * std::f64::consts::FRAC_PI_2 * 1.1,
            DETOUR_LEAD_FT,
        )
    };
    let reach = reach.max(DETOUR_LEAD_FT);
    [
        own[0] + bearing.sin() * reach,
        own[2] + bearing.cos() * reach,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(id: u32, x: f64, z: f64, radius: f64) -> Ring {
        Ring {
            id,
            centre: [x, 0., z],
            radius_ft: radius,
        }
    }

    #[test]
    fn a_clear_way_home_is_flown_straight() {
        let rings = [ring(1, 20_000., 30_000., 5_000.)];
        assert_eq!(detour(&rings, [0.; 3], [0., 0., 60_000.]), None);
    }

    #[test]
    fn a_ring_across_the_way_home_is_passed_on_the_far_side_and_never_entered() {
        // A ring just right of the line north: it goes round the left.
        let rings = [ring(1, 1_000., 30_000., 8_000.)];
        let home = [0., 0., 60_000.];
        let first = detour(&rings, [0.; 3], home).unwrap();
        assert!(first[0] < 0., "{first:?}");
        // Following the detour step by step it never enters the ring and
        // ends on the straight line home.
        let mut own = [0.; 3];
        for _ in 0..2_000 {
            let aim = detour(&rings, own, home).map_or(home, |p| [p[0], 0., p[1]]);
            let (dx, dz) = (aim[0] - own[0], aim[2] - own[2]);
            let d = dx.hypot(dz);
            if horizontal(own, home) < 100. {
                break;
            }
            own = [own[0] + dx / d * 100., 0., own[2] + dz / d * 100.];
            assert!(horizontal(own, rings[0].centre) >= 8_000., "{own:?}");
        }
        assert!(horizontal(own, home) < 200., "{own:?}");
    }

    #[test]
    fn inside_a_ring_it_leaves_by_the_shortest_way_out_first() {
        let rings = [ring(7, 0., 10_000., 8_000.)];
        // 3,000 ft east of the centre, home due north beyond the ring.
        let point = detour(&rings, [3_000., 0., 10_000.], [0., 0., 60_000.]).unwrap();
        assert!((point[0] - 9_000.).abs() < 1e-6 && (point[1] - 10_000.).abs() < 1e-6);
    }

    #[test]
    fn a_ring_around_home_is_ignored_and_the_result_is_repeatable() {
        let rings = [ring(1, 0., 60_000., 10_000.), ring(2, 0., 30_000., 6_000.)];
        let home = [0., 0., 60_000.];
        let a = detour(&rings, [0.; 3], home);
        assert_eq!(a, detour(&rings, [0.; 3], home));
        // Only the ring across the way counts.
        assert!(a.is_some());
        assert_eq!(detour(&rings[..1], [0.; 3], home), None);
    }
}
