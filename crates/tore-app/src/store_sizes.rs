//! Real sizes for the weapon shapes drawn on pylons and in flight (slice BP2,
//! docs/spec/stores-on-pylons.md#store-sizes).
//!
//! The retail weapon shapes are about the right length, but their bodies and
//! fins are drawn 1.3 to 2 times too thick: shape coordinates are whole
//! source units of a third of a foot, so the thinnest body a shape can hold
//! is 0.67 ft across (an AIM-9 is 0.42 ft), and the fins are drawn larger
//! still. A few cluster dispensers are also drawn 40 to 70 percent too long.
//! On a pylon, next to an aircraft drawn at its true size, that reads as big
//! and bulky (John, 2026-10-12).
//!
//! Each listed shape is refitted to the real store: its length scales to the
//! real length, its body to the real diameter and its fin tips to the real
//! span, each axis separately, so a thin fin stays thin. The refit is
//! opinionated (John 2026-10-12 asked for real sizes); the numbers are public
//! dimensions, rounded, and the fitting rule is an agent decision.
use tore_formats::shape::{Face, Shape};

pub use tore_sim::combat::store_sizes::{Real, SIZES, real_size};

/// Where a store shape's body is and how far its fins reach, feet: the
/// body's centre across (right, up), its half width and half height, the
/// fins' half reach either way from that centre, and the shape's length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub centre: [f64; 2],
    pub half: [f64; 2],
    pub reach: [f64; 2],
    pub length: f64,
    /// Forward extent of the drawn faces, low and high.
    pub along: [f64; 2],
}

/// The texture-only faces (a missile's exhaust sheet), which are not drawn.
pub fn texture_only(face: &Face) -> bool {
    matches!(face.subtype, 0x4c | 0x5c | 0x6c | 0x7c)
}

/// Feet from source units (a third of a foot).
fn feet(q: [f32; 3]) -> [f64; 3] {
    q.map(|v| f64::from(v) / 3.)
}

/// Width and height of `faces`' cross-section at `forward` feet, as its
/// low and high corners (right, up); `None` where nothing crosses.
fn section<'a>(faces: impl Iterator<Item = &'a Face>, forward: f64) -> Option<[[f64; 2]; 2]> {
    let mut low = [f64::MAX; 2];
    let mut high = [f64::MIN; 2];
    for f in faces {
        let n = f.positions.len();
        for i in 0..n {
            let a = feet(f.positions[i]);
            let b = feet(f.positions[(i + 1) % n]);
            if (a[1] - forward) * (b[1] - forward) > 0. || (a[1] - b[1]).abs() < 1e-9 {
                continue;
            }
            let t = (forward - a[1]) / (b[1] - a[1]);
            let p = [a[0] + t * (b[0] - a[0]), a[2] + t * (b[2] - a[2])];
            for k in 0..2 {
                low[k] = low[k].min(p[k]);
                high[k] = high[k].max(p[k]);
            }
        }
    }
    (low[0] <= high[0]).then_some([low, high])
}

/// The body and fin reach of a store shape: the body is the second
/// thinnest of nine cross-sections between a quarter and three quarters of
/// its length (so wings and a tapering nose do not count as body), the fins
/// whatever reaches beyond it. `None` for a shape with no drawn faces.
pub fn body(shape: &Shape) -> Option<Body> {
    let drawn = || shape.faces.iter().filter(|f| !texture_only(f));
    let mut low = [f64::MAX; 3];
    let mut high = [f64::MIN; 3];
    for q in drawn().flat_map(|f| f.positions.iter()) {
        let p = feet(*q);
        for k in 0..3 {
            low[k] = low[k].min(p[k]);
            high[k] = high[k].max(p[k]);
        }
    }
    if low[0] > high[0] {
        return None;
    }
    let length = high[1] - low[1];
    let mut sections: Vec<[[f64; 2]; 2]> = (0..9)
        .filter_map(|i| section(drawn(), low[1] + (0.25 + f64::from(i) / 16.) * length))
        .collect();
    let fallback = [[low[0], low[2]], [high[0], high[2]]];
    sections.sort_by(|a, b| {
        let area = |s: &[[f64; 2]; 2]| (s[1][0] - s[0][0]) * (s[1][1] - s[0][1]);
        area(a).total_cmp(&area(b))
    });
    let [lo, hi] = sections
        .get(1)
        .or(sections.first())
        .copied()
        .unwrap_or(fallback);
    let centre = [(lo[0] + hi[0]) / 2., (lo[1] + hi[1]) / 2.];
    let half = [
        ((hi[0] - lo[0]) / 2.).max(0.05),
        ((hi[1] - lo[1]) / 2.).max(0.05),
    ];
    let reach = [
        (high[0] - centre[0]).max(centre[0] - low[0]).max(half[0]),
        (high[2] - centre[1]).max(centre[1] - low[2]).max(half[1]),
    ];
    Some(Body {
        centre,
        half,
        reach,
        length,
        along: [low[1], high[1]],
    })
}

/// One axis of the refit: an offset `d` from the body centre, where the
/// body's half size `half` becomes `radius` and the fin tips at `reach`
/// become `tip`, linearly between.
fn refit(d: f64, half: f64, reach: f64, radius: f64, tip: f64) -> f64 {
    let a = d.abs();
    let r = if a <= half || reach - half < 0.05 {
        a * radius / half
    } else {
        radius + (a - half) * (tip - radius) / (reach - half)
    };
    r.copysign(d)
}

/// `shape` refitted to `real` (with `span` across its fins): length about
/// its origin (where the store is released from, so a store does not move
/// when it leaves), body and fins about the body's centre line, which stays
/// where it is.
pub fn fitted(shape: &Shape, real: &Real, span: f64) -> Shape {
    let Some(b) = body(shape) else {
        return copy(shape);
    };
    let along = real.length / b.length.max(0.1);
    let radius = real.diameter / 2.;
    let tip = (span / 2.).max(radius);
    let mut out = copy(shape);
    for face in &mut out.faces {
        for q in &mut face.positions {
            let p = feet(*q);
            let right = b.centre[0] + refit(p[0] - b.centre[0], b.half[0], b.reach[0], radius, tip);
            let up = b.centre[1] + refit(p[2] - b.centre[1], b.half[1], b.reach[1], radius, tip);
            *q = [right, p[1] * along, up].map(|v| (v * 3.) as f32);
        }
        face.normal = None;
    }
    out
}

/// A copy of `shape` (the format crate's shape is not `Clone`).
fn copy(shape: &Shape) -> Shape {
    Shape {
        lines: shape.lines.clone(),
        faces: shape.faces.clone(),
        billboards: shape.billboards.clone(),
        state_words: shape.state_words.clone(),
    }
}

/// The shapes a weapon shape `name` draws as: on a pylon, when that differs
/// from in flight, and in flight. Unlisted shapes keep their retail size.
pub fn variants(name: &str, shape: Shape) -> (Option<Shape>, Shape) {
    match real_size(name) {
        Some(real) => match real.flight_span {
            Some(span) => (
                Some(fitted(&shape, &real, real.span)),
                fitted(&shape, &real, span),
            ),
            None => (None, fitted(&shape, &real, real.span)),
        },
        None => (None, shape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::combat::store_sizes::real;

    /// A synthetic store: a square body `width` across and `length` long
    /// (source units) with one flat fin pair spanning `fins`.
    pub(crate) fn store(length: f32, width: f32, fins: f32) -> Shape {
        let face = |positions: Vec<[f32; 3]>| Face {
            colors: vec![1; positions.len()],
            positions,
            fog: Default::default(),
            uv: Vec::new(),
            texture: String::new(),
            subtype: 0,
            normal: None,
            address: 0,
        };
        let h = width / 2.;
        let (a, b) = (-length / 2., length / 2.);
        let mut faces = Vec::new();
        for (x, z) in [(h, h), (-h, h), (-h, -h), (h, -h)] {
            faces.push(face(vec![[x, a, z], [x, b, z], [-z, b, x], [-z, a, x]]));
        }
        let f = fins / 2.;
        faces.push(face(vec![
            [-f, a, 0.],
            [f, a, 0.],
            [f, a + 3., 0.],
            [-f, a + 3., 0.],
        ]));
        Shape {
            lines: Vec::new(),
            faces,
            billboards: Vec::new(),
            state_words: Default::default(),
        }
    }

    #[test]
    fn a_fat_store_is_refitted_to_its_real_body_fins_and_length() {
        // 27 units long (9 ft), 2 units across (0.67 ft), fins 6 (2 ft).
        let shape = store(27., 2., 6.);
        let b = body(&shape).unwrap();
        assert!((b.half[0] - 1. / 3.).abs() < 1e-6);
        assert!((b.reach[0] - 1.).abs() < 1e-6);
        let real = real(9.35, 0.42, 1.5);
        let fitted = fitted(&shape, &real, real.span);
        let f = body(&fitted).unwrap();
        assert!((f.length - 9.35).abs() < 0.01, "{f:?}");
        assert!((f.half[0] - 0.21).abs() < 0.01, "{f:?}");
        assert!((f.reach[0] - 0.75).abs() < 0.01, "{f:?}");
        // Thin stays thin: the fin's own plane is not thickened.
        assert!((f.reach[1] - 0.21).abs() < 0.01, "{f:?}");
    }

    #[test]
    fn every_listed_size_is_plausible_and_unique() {
        for (i, (name, real)) in SIZES.iter().enumerate() {
            assert!(real.length > 3. && real.length < 25., "{name}");
            assert!(real.diameter > 0.2 && real.diameter < 2.2, "{name}");
            assert!(real.span >= real.diameter * 0.9, "{name}");
            assert!(
                SIZES[i + 1..].iter().all(|(other, _)| other != name),
                "{name} listed twice"
            );
        }
        assert_eq!(real_size("sidem.sh").map(|r| r.length), Some(9.35));
        assert!(real_size("NOPE.SH").is_none());
    }
}
