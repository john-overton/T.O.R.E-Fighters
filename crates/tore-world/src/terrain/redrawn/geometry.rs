//! Convex polygons in the runway frame (feet, `[x, z]`): the pieces a
//! redrawn airport's pavement is cut into. Every polygon here is convex and
//! wound counter-clockwise seen from above with x right and z forward
//! (positive signed area).
pub type Point = [f64; 2];
pub type Poly = Vec<Point>;

/// Below this many square feet a piece is a sliver and is dropped.
pub const SLIVER_AREA: f64 = 0.01;

pub fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}
pub fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
pub fn scale(a: Point, s: f64) -> Point {
    [a[0] * s, a[1] * s]
}
pub fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
pub fn cross(a: Point, b: Point) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
pub fn length(a: Point) -> f64 {
    a[0].hypot(a[1])
}
pub fn unit(a: Point) -> Point {
    let l = length(a);
    [a[0] / l, a[1] / l]
}
/// The direction a relative heading in degrees points: 0 is +z, 90 is +x.
pub fn direction(heading_deg: f64) -> Point {
    let (s, c) = heading_deg.to_radians().sin_cos();
    [s, c]
}
/// The right-hand normal of a direction (x right, z forward).
pub fn right_of(d: Point) -> Point {
    [d[1], -d[0]]
}

pub fn signed_area(poly: &[Point]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| cross(poly[i], poly[(i + 1) % n]))
        .sum::<f64>()
        * 0.5
}

/// Counter-clockwise order (positive area).
pub fn ccw(mut poly: Poly) -> Poly {
    if signed_area(&poly) < 0. {
        poly.reverse();
    }
    poly
}

/// The rectangle around segment `a`..`b`, `half` either side, extended
/// `extend` past each end.
pub fn band(a: Point, b: Point, half: f64, extend: f64) -> Poly {
    let d = unit(sub(b, a));
    let n = right_of(d);
    let a = sub(a, scale(d, extend));
    let b = add(b, scale(d, extend));
    ccw(vec![
        sub(a, scale(n, half)),
        add(a, scale(n, half)),
        add(b, scale(n, half)),
        sub(b, scale(n, half)),
    ])
}

pub fn rect(min: Point, max: Point) -> Poly {
    vec![min, [max[0], min[1]], max, [min[0], max[1]]]
}

/// The part of `poly` where `dot(normal, p) <= offset`.
pub fn clip(poly: &[Point], normal: Point, offset: f64) -> Poly {
    let mut out = Vec::new();
    let Some(&last) = poly.last() else {
        return out;
    };
    let mut a = last;
    let mut da = dot(normal, a) - offset;
    for &b in poly {
        let db = dot(normal, b) - offset;
        if (da <= 0.) != (db <= 0.) {
            let t = da / (da - db);
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
        if db <= 0. {
            out.push(b);
        }
        a = b;
        da = db;
    }
    out
}

/// The half-planes (normal, offset) whose intersection is the convex
/// counter-clockwise `poly`: inside is `dot(normal, p) <= offset`.
fn half_planes(poly: &[Point]) -> Vec<(Point, f64)> {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % n];
            // Outward normal of a counter-clockwise edge.
            let normal = unit(right_of(sub(b, a)));
            (normal, dot(normal, a))
        })
        .collect()
}

pub fn intersect(a: &[Point], b: &[Point]) -> Poly {
    let mut out = a.to_vec();
    for (normal, offset) in half_planes(b) {
        out = clip(&out, normal, offset);
        if out.len() < 3 {
            return Vec::new();
        }
    }
    out
}

pub fn overlap_area(a: &[Point], b: &[Point]) -> f64 {
    let piece = intersect(a, b);
    if piece.len() < 3 {
        0.
    } else {
        signed_area(&piece).abs()
    }
}

/// `a` without `b`: convex pieces, none overlapping `b` or each other.
pub fn minus(a: &[Point], b: &[Point]) -> Vec<Poly> {
    if overlap_area(a, b) <= SLIVER_AREA {
        return vec![a.to_vec()];
    }
    let mut out = Vec::new();
    let mut rest = a.to_vec();
    for (normal, offset) in half_planes(b) {
        // The part outside this edge is free of `b`; keep it and go on with
        // the part inside.
        let outside = clip(&rest, scale(normal, -1.), -offset);
        if outside.len() >= 3 && signed_area(&outside).abs() > SLIVER_AREA {
            out.push(outside);
        }
        rest = clip(&rest, normal, offset);
        if rest.len() < 3 {
            break;
        }
    }
    out
}

pub fn minus_all(a: &[Point], covered: &[Poly]) -> Vec<Poly> {
    let mut pieces = vec![a.to_vec()];
    for b in covered {
        pieces = pieces.into_iter().flat_map(|p| minus(&p, b)).collect();
    }
    pieces
}

pub fn contains(poly: &[Point], p: Point) -> bool {
    half_planes(poly)
        .into_iter()
        .all(|(normal, offset)| dot(normal, p) <= offset + 1e-6)
}

/// The axis-aligned extent of some polygons.
pub fn bounds<'p>(polys: impl IntoIterator<Item = &'p Poly>) -> Option<(Point, Point)> {
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for p in polys.into_iter().flatten() {
        for axis in 0..2 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    min[0].is_finite().then_some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_minus_a_crossing_band_leaves_two_pieces_of_the_right_area() {
        let square = rect([0., 0.], [100., 100.]);
        let road = band([-50., 50.], [150., 50.], 10., 0.);
        let pieces = minus(&square, &road);
        let area: f64 = pieces.iter().map(|p| signed_area(p)).sum();
        assert!((area - 8_000.).abs() < 1e-6, "{area}");
        for p in &pieces {
            assert!(signed_area(p) > 0.);
            assert!(overlap_area(p, &road) < 1e-6);
        }
    }

    #[test]
    fn an_angled_band_minus_a_square_keeps_the_rest() {
        let strip = band([0., 0.], [1000., 1000.], 50., 0.);
        let square = rect([400., 400.], [600., 600.]);
        let total = signed_area(&strip);
        let cut = overlap_area(&strip, &square);
        let area: f64 = minus(&strip, &square).iter().map(|p| signed_area(p)).sum();
        assert!((area - (total - cut)).abs() < 1e-6);
        assert!(contains(&strip, [500., 500.]) && !contains(&strip, [500., 700.]));
    }
}
