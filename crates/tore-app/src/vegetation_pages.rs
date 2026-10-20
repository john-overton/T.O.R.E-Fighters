//! Coarse culling of tree pages (optimization round 2): each view sends
//! the cull pass only the pages of 256 objects that could hold a drawn one.
//!
//! A page keeps the box of its objects' cull points (base plus half the
//! height, as `vegetation_cull.wgsl` measures from) and its largest cull
//! radius (`max(radius, height / 2)`). A page is dropped only when every
//! point of that box fails one of the shader's own rejections with the
//! largest radius: beyond the draw distance, behind the eye, or outside one
//! side of the view. So no object the shader would draw is ever dropped, and
//! the objects it draws, their order aside, are the same.

/// One instance record: sixteen floats (`vegetation_renderer::pack`).
const RECORD: usize = 64;

/// A page's box of cull points in world feet, and its largest cull radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub radius: f64,
}

impl PageBounds {
    /// Never culled.
    pub const ALL: Self = Self {
        min: [f64::NEG_INFINITY; 3],
        max: [f64::INFINITY; 3],
        radius: f64::INFINITY,
    };

    /// The bounds of the instance records in `bytes`. A record with a
    /// non-finite position, height or radius makes the page never culled.
    pub fn of(bytes: &[u8]) -> Self {
        let mut b = Self {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
            radius: 0.,
        };
        let float = |r: &[u8], i: usize| {
            f64::from(f32::from_le_bytes([
                r[i * 4],
                r[i * 4 + 1],
                r[i * 4 + 2],
                r[i * 4 + 3],
            ]))
        };
        for r in bytes.chunks_exact(RECORD) {
            let height = float(r, 4);
            let p = [float(r, 0), float(r, 1) + height * 0.5, float(r, 2)];
            let radius = float(r, 14).max(height * 0.5);
            if !(p.iter().all(|v| v.is_finite()) && radius.is_finite()) {
                return Self::ALL;
            }
            for (i, v) in p.into_iter().enumerate() {
                b.min[i] = b.min[i].min(v);
                b.max[i] = b.max[i].max(v);
            }
            b.radius = b.radius.max(radius);
        }
        b
    }

    /// The bounds of each page-sized run of `bytes` (a chunk's records).
    pub fn pages(bytes: &[u8], page: usize) -> Vec<Self> {
        bytes.chunks(page * RECORD).map(Self::of).collect()
    }
}

/// One view as the cull pass sees it: the eye in world feet, the basis,
/// the half-view tangents (`params.view.xy`) and the draw distance.
#[derive(Clone, Copy, Debug)]
pub struct PageView {
    pub eye: [f64; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub tangents: [f32; 2],
    pub draw: f32,
}

/// Feet of slack on every test, plus [`SLACK_SHARE`] of the box's reach
/// from the eye: the shader works in 32-bit floats relative to the render
/// origin, whose rounding is under a foot in a two-million-foot theater.
const SLACK_FEET: f64 = 16.;
const SLACK_SHARE: f64 = 1e-4;

impl PageView {
    /// Whether no object of a page with these bounds can pass the cull.
    pub fn rejects(&self, b: &PageBounds) -> bool {
        if b.min.iter().chain(&b.max).any(|v| !v.is_finite()) || !b.radius.is_finite() {
            return false;
        }
        let lo: [f64; 3] = std::array::from_fn(|i| b.min[i] - self.eye[i]);
        let hi: [f64; 3] = std::array::from_fn(|i| b.max[i] - self.eye[i]);
        let reach = (0..3)
            .map(|i| lo[i].abs().max(hi[i].abs()))
            .fold(0., f64::max);
        let slack = SLACK_FEET + SLACK_SHARE * reach;
        // Beyond the draw distance (horizontal, as `dist` in the shader).
        let near = |i: usize| lo[i].max(0.) + (-hi[i]).max(0.);
        if near(0).hypot(near(2)) >= f64::from(self.draw) + slack {
            return true;
        }
        // The smallest value of `n . d` over the box.
        let least = |n: [f64; 3]| -> f64 {
            (0..3)
                .map(|i| n[i] * if n[i] >= 0. { lo[i] } else { hi[i] })
                .sum()
        };
        let f = self.forward.map(f64::from);
        // Behind the eye: every z below -r.
        if -least(f.map(|v| -v)) < -b.radius - slack {
            return true;
        }
        // Outside a side: |d . axis| - z * tangent above 1.6 r everywhere.
        let side = 1.6 * b.radius + slack;
        for (axis, tangent) in [(self.right, self.tangents[0]), (self.up, self.tangents[1])] {
            let t = f64::from(tangent);
            for sign in [1., -1.] {
                let n = std::array::from_fn(|i| sign * f64::from(axis[i]) - t * f[i]);
                if least(n) > side {
                    return true;
                }
            }
        }
        false
    }

    /// The entries of `table` (first, count pairs) whose page `bounds` this
    /// view may draw from, in table order, into `out`.
    pub fn visible(&self, table: &[u32], bounds: &[PageBounds], out: &mut Vec<u32>) {
        out.clear();
        for (i, entry) in table.chunks_exact(2).enumerate() {
            if bounds.get(i).is_none_or(|b| !self.rejects(b)) {
                out.extend_from_slice(entry);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn range(&mut self, a: f64, b: f64) -> f64 {
            a + (b - a) * self.next()
        }
    }

    fn record(pos: [f32; 3], height: f32, radius: f32) -> Vec<u8> {
        let mut v = [0f32; 16];
        v[0..3].copy_from_slice(&pos);
        v[4] = height;
        v[14] = radius;
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    /// The cull shader's decision for one record, in 32-bit floats as the
    /// GPU takes it (`vegetation_cull.wgsl` `cull`).
    fn shader_draws(view: &PageView, origin: [f64; 3], r: &[u8]) -> bool {
        let f = |i: usize| f32::from_le_bytes([r[i * 4], r[i * 4 + 1], r[i * 4 + 2], r[i * 4 + 3]]);
        let o = origin.map(|v| v as f32);
        let eye: [f32; 3] = std::array::from_fn(|i| (view.eye[i] - origin[i]) as f32);
        let height = f(4);
        let rel = [f(0) - o[0], f(1) - o[1], f(2) - o[2]];
        let d = [
            rel[0] - eye[0],
            rel[1] + height * 0.5 - eye[1],
            rel[2] - eye[2],
        ];
        let dot = |a: [f32; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
        let dist = (d[0] * d[0] + d[2] * d[2]).sqrt();
        if dist >= view.draw {
            return false;
        }
        let r = f(14).max(height * 0.5);
        let z = dot(view.forward);
        if z < -r {
            return false;
        }
        if dot(view.right).abs() > z * view.tangents[0] + r * 1.6 {
            return false;
        }
        dot(view.up).abs() <= z * view.tangents[1] + r * 1.6
    }

    fn basis(yaw: f64, pitch: f64, roll: f64) -> [[f32; 3]; 3] {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sr, cr) = roll.sin_cos();
        let f = [sy * cp, sp, cy * cp];
        let r0 = [cy, 0., -sy];
        let u0 = [-sy * sp, cp, -cy * sp];
        let r: [f64; 3] = std::array::from_fn(|i| r0[i] * cr + u0[i] * sr);
        let u: [f64; 3] = std::array::from_fn(|i| u0[i] * cr - r0[i] * sr);
        [f, r, u].map(|v| v.map(|x| x as f32))
    }

    #[test]
    fn page_culling_never_drops_a_drawn_tree() {
        let mut rng = Rng(7);
        let origin = [1_000_000., 0., 800_000.];
        // 600 pages of up to 256 trees in 2,048 ft chunks over 60,000 ft.
        let mut pages = Vec::new();
        for _ in 0..600 {
            let cx = rng.range(940_000., 1_060_000.).floor();
            let cz = rng.range(740_000., 860_000.).floor();
            let n = 1 + (rng.next() * 256.) as usize;
            let mut bytes = Vec::new();
            for _ in 0..n {
                let pos = [
                    (cx + rng.range(0., 2048.)) as f32,
                    rng.range(0., 3000.) as f32,
                    (cz + rng.range(0., 2048.)) as f32,
                ];
                bytes.extend(record(
                    pos,
                    rng.range(2., 120.) as f32,
                    rng.range(0., 60.) as f32,
                ));
            }
            pages.push(bytes);
        }
        let bounds: Vec<PageBounds> = pages.iter().map(|p| PageBounds::of(p)).collect();
        let table: Vec<u32> = (0..pages.len() as u32)
            .flat_map(|i| [i * 256, 256])
            .collect();
        let (mut kept, mut total, mut out) = (0, 0, Vec::new());
        for _ in 0..400 {
            let [forward, right, up] = basis(
                rng.range(0., std::f64::consts::TAU),
                rng.range(-1.5, 1.5),
                rng.range(-1., 1.),
            );
            let f = 1.732_050_8 * rng.range(0.25, 8.) as f32;
            let aspect = rng.range(0.5, 2.5) as f32;
            let view = PageView {
                eye: [
                    rng.range(950_000., 1_050_000.),
                    rng.range(0., 30_000.),
                    rng.range(750_000., 850_000.),
                ],
                forward,
                right,
                up,
                tangents: [aspect / f, 1. / f],
                draw: rng.range(2_000., 50_000.) as f32,
            };
            view.visible(&table, &bounds, &mut out);
            for (i, page) in pages.iter().enumerate() {
                let listed = out.chunks_exact(2).any(|e| e[0] == i as u32 * 256);
                let drawn = page
                    .chunks_exact(RECORD)
                    .any(|r| shader_draws(&view, origin, r));
                assert!(
                    listed || !drawn,
                    "page {i} holds a drawn tree but was culled"
                );
            }
            kept += out.len() / 2;
            total += pages.len();
        }
        assert!(kept * 3 < total, "most pages are culled: {kept} of {total}");
    }

    #[test]
    fn page_edges_and_unbounded_pages_are_kept() {
        let view = PageView {
            eye: [0.; 3],
            forward: [0., 0., 1.],
            right: [1., 0., 0.],
            up: [0., 1., 0.],
            tangents: [1., 1.],
            draw: 10_000.,
        };
        // One tree just inside the draw distance, right ahead.
        let at = record([0., -5., 9_999.], 10., 0.);
        assert!(!view.rejects(&PageBounds::of(&at)));
        // Just beyond it, and well behind the eye.
        assert!(view.rejects(&PageBounds::of(&record([0., -5., 10_100.], 10., 0.))));
        assert!(view.rejects(&PageBounds::of(&record([0., 0., -500.], 10., 5.))));
        // A tree whose crown reaches across the side plane is kept.
        assert!(!view.rejects(&PageBounds::of(&record([1_040., 0., 1_000.], 50., 30.))));
        assert!(view.rejects(&PageBounds::of(&record([1_200., 0., 1_000.], 50., 30.))));
        // A non-finite record never culls its page.
        let nan = record([f32::NAN, 0., 1e9], 10., 0.);
        assert_eq!(PageBounds::of(&nan), PageBounds::ALL);
        assert!(!view.rejects(&PageBounds::ALL));
        // Pages split at the page size, each with its own box.
        let mut two = record([0., 0., 100.], 10., 0.);
        two.extend(record([0., 0., -9_000.], 10., 0.));
        let pages = PageBounds::pages(&two, 1);
        assert_eq!(pages.len(), 2);
        let mut out = Vec::new();
        view.visible(&[0, 1, 1, 1], &pages, &mut out);
        assert_eq!(out, [0, 1]);
    }
}
