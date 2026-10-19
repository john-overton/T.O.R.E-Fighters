//! The terrain mesh in square spatial chunks (optimization S13a), so each
//! view and each shadow cascade draws only the chunks it can see.
//!
//! The triangles are the same ones the scenery built; only their order in
//! the GPU buffer changes. Every triangle goes to the chunk under its
//! centroid, and keeps its source order inside that chunk. A chunk holds its
//! shadow-opaque triangles first (those whose texture page has no cut-out
//! texel, so the shadow pass can draw them depth-only), then the rest.
//!
//! A chunk is skipped only when its box, widened by [`MARGIN_FEET`], lies
//! wholly outside one clip plane, so none of its triangles could make a
//! fragment. Culling is therefore exact. The draw order (nearest chunk
//! first) can only matter where two triangles of different chunks cover a
//! sample at exactly the same depth.

use std::ops::Range;

/// Chunk side: eight 8,192 ft terrain cells (65,536 ft, two texture tiles).
pub const CHUNK_FEET: f64 = 65_536.;
/// Widening of every box before a clip test, feet. The GPU places vertices
/// in 32-bit floats relative to the render origin; its rounding is well
/// under a foot in a two-million-foot theater.
pub const MARGIN_FEET: f64 = 64.;
const FLOATS: usize = 10;
const TRIANGLE: usize = 3 * FLOATS;

/// An axis-aligned box in feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Bounds {
    pub const EMPTY: Self = Self {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    /// Everywhere: a box no clip volume rejects.
    pub const ALL: Self = Self {
        min: [f64::NEG_INFINITY; 3],
        max: [f64::INFINITY; 3],
    };
    pub fn is_empty(&self) -> bool {
        (0..3).any(|i| self.min[i] > self.max[i])
    }
    pub fn add(&mut self, p: [f64; 3]) {
        for (i, v) in p.into_iter().enumerate() {
            self.min[i] = self.min[i].min(v);
            self.max[i] = self.max[i].max(v);
        }
    }
    /// The box of every vertex position in `vertices` (ten floats each,
    /// position first). Non-finite positions make the box unbounded, so a
    /// batch holding one is never culled.
    pub fn of_vertices(vertices: &[f32]) -> Self {
        let mut bounds = Self::EMPTY;
        for v in vertices.chunks_exact(FLOATS) {
            let p = [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])];
            if p.iter().all(|x| x.is_finite()) {
                bounds.add(p);
            } else {
                return Self::ALL;
            }
        }
        bounds
    }
    /// This box moved by `-by` and widened by [`MARGIN_FEET`].
    pub fn relative(&self, by: [f64; 3]) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i] - by[i] - MARGIN_FEET),
            max: std::array::from_fn(|i| self.max[i] - by[i] + MARGIN_FEET),
        }
    }
    /// The nearest distance from the origin to this box (0 inside it).
    pub fn distance(&self) -> f64 {
        (0..3)
            .map(|i| (self.min[i].max(0.) + (-self.max[i]).max(0.)).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}

/// A convex clip volume as half-spaces `normal . p + offset >= 0`, with
/// `p` relative to the eye.
#[derive(Clone, Debug, Default)]
pub struct Clip {
    planes: Vec<([f64; 3], f64)>,
}

impl Clip {
    /// The world pass's perspective volume (`placed_vertex` and
    /// `world_depth_clip` in terrain.wgsl): `right`, `up`, `forward` and
    /// `zoom` as the scene uniform holds them, `aspect` the width over the
    /// drawn height, and the near and far planes along `forward`.
    pub fn perspective(
        right: [f32; 3],
        up: [f32; 3],
        forward: [f32; 3],
        zoom: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let f = 1.732_050_8 * f64::from(zoom);
        let r = right.map(|v| f64::from(v) * f / f64::from(aspect));
        let u = up.map(|v| f64::from(v) * f);
        let w = forward.map(f64::from);
        let sum = |a: [f64; 3], b: [f64; 3], s: f64| std::array::from_fn(|i| a[i] + s * b[i]);
        Self {
            planes: vec![
                (w, -f64::from(near.max(1.))),
                (w.map(|v| -v), f64::from(far)),
                (sum(w, r, -1.), 0.),
                (sum(w, r, 1.), 0.),
                (sum(w, u, -1.), 0.),
                (sum(w, u, 1.), 0.),
            ],
        }
    }
    /// A shadow cascade's orthographic volume from its projection rows
    /// (`surface_lighting::projection`): x and y in -1..1, depth in 0..1.
    pub fn orthographic(m: &[f32; 16]) -> Self {
        let row = |i: usize| [m[i], m[i + 1], m[i + 2]].map(f64::from);
        let (x, y, z) = (row(0), row(4), row(8));
        let (bx, by, bz) = (f64::from(m[3]), f64::from(m[7]), f64::from(m[11]));
        Self {
            planes: vec![
                (x.map(|v| -v), 1. - bx),
                (x, 1. + bx),
                (y.map(|v| -v), 1. - by),
                (y, 1. + by),
                (z, bz),
                (z.map(|v| -v), 1. - bz),
            ],
        }
    }
    /// Whether every point of `bounds` (already relative to the eye) is
    /// outside one plane.
    pub fn rejects(&self, bounds: &Bounds) -> bool {
        if bounds.is_empty() {
            return true;
        }
        self.planes.iter().any(|(n, d)| {
            let best: f64 = (0..3)
                .map(|i| {
                    n[i] * if n[i] >= 0. {
                        bounds.max[i]
                    } else {
                        bounds.min[i]
                    }
                })
                .sum();
            best + d < 0.
        })
    }
}

/// One chunk's triangles: `first..first + opaque` cast shadows without a
/// cut-out test, `first + opaque..first + count` with one.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub first: u32,
    pub opaque: u32,
    pub count: u32,
    pub bounds: Bounds,
}

impl Chunk {
    pub fn all(&self) -> Range<u32> {
        self.first..self.first + self.count
    }
    pub fn opaque(&self) -> Range<u32> {
        self.first..self.first + self.opaque
    }
    pub fn cutout(&self) -> Range<u32> {
        self.first + self.opaque..self.first + self.count
    }
}

/// The chunks of the reordered terrain buffer.
pub struct Chunks {
    pub chunks: Vec<Chunk>,
}

/// Splits the terrain triangles (`vertices`, ten floats a vertex, and the
/// matching `normals`, three a vertex) into chunks, returning the reordered
/// vertices and normals with them. `opaque(layer)` says whether a triangle
/// with that texture layer casts a shadow everywhere.
pub fn build(
    vertices: &[f32],
    normals: &[f32],
    opaque: impl Fn(f32) -> bool,
) -> (Vec<f32>, Vec<f32>, Chunks) {
    let triangles = vertices.len() / TRIANGLE;
    assert_eq!(normals.len(), triangles * 9, "a normal per vertex");
    let mut keyed: Vec<((i64, i64, bool), usize)> = (0..triangles)
        .map(|t| {
            let v = &vertices[t * TRIANGLE..(t + 1) * TRIANGLE];
            let centre = |axis: usize| {
                (f64::from(v[axis]) + f64::from(v[FLOATS + axis]) + f64::from(v[2 * FLOATS + axis]))
                    / 3.
            };
            let cell = |axis| {
                let c = (centre(axis) / CHUNK_FEET).floor();
                if c.is_finite() { c as i64 } else { 0 }
            };
            ((cell(2), cell(0), !opaque(v[5])), t)
        })
        .collect();
    // Stable: source order within each chunk and class.
    keyed.sort_by_key(|(key, _)| *key);
    let mut out = Chunks { chunks: Vec::new() };
    let mut sorted = Vec::with_capacity(vertices.len());
    let mut sorted_normals = Vec::with_capacity(normals.len());
    let mut current: Option<(i64, i64)> = None;
    for ((z, x, cutout), t) in keyed {
        if current != Some((z, x)) {
            current = Some((z, x));
            out.chunks.push(Chunk {
                first: (sorted.len() / FLOATS) as u32,
                opaque: 0,
                count: 0,
                bounds: Bounds::EMPTY,
            });
        }
        let chunk = out.chunks.last_mut().expect("pushed above");
        let v = &vertices[t * TRIANGLE..(t + 1) * TRIANGLE];
        for p in v.chunks_exact(FLOATS) {
            chunk.bounds.add([p[0], p[1], p[2]].map(f64::from));
        }
        chunk.count += 3;
        if !cutout {
            chunk.opaque += 3;
        }
        sorted.extend_from_slice(v);
        sorted_normals.extend_from_slice(&normals[t * 9..(t + 1) * 9]);
    }
    (sorted, sorted_normals, out)
}

impl Chunks {
    /// The chunks `clip` keeps, nearest first (ties in buffer order), with
    /// the boxes taken relative to `eye`.
    pub fn front_to_back(&self, eye: [f64; 3], clip: &Clip, out: &mut Vec<Range<u32>>) {
        let mut kept: Vec<(f64, usize)> = self
            .chunks
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let b = c.bounds.relative(eye);
                (!clip.rejects(&b)).then(|| (b.distance(), i))
            })
            .collect();
        kept.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        out.clear();
        out.extend(kept.into_iter().map(|(_, i)| self.chunks[i].all()));
    }
    /// The shadow-opaque and cut-out ranges of the chunks `clip` keeps, in
    /// buffer order, adjacent ranges merged.
    pub fn casters(
        &self,
        eye: [f64; 3],
        clip: &Clip,
        opaque: &mut Vec<Range<u32>>,
        cutout: &mut Vec<Range<u32>>,
    ) {
        opaque.clear();
        cutout.clear();
        for c in &self.chunks {
            if clip.rejects(&c.bounds.relative(eye)) {
                continue;
            }
            push_merged(opaque, c.opaque());
            push_merged(cutout, c.cutout());
        }
    }
}

fn push_merged(ranges: &mut Vec<Range<u32>>, range: Range<u32>) {
    if range.is_empty() {
        return;
    }
    match ranges.last_mut() {
        Some(last) if last.end == range.start => last.end = range.end,
        _ => ranges.push(range),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertex(x: f32, y: f32, z: f32, layer: f32) -> [f32; 10] {
        [x, y, z, 0., 0., layer, 0., 0., 0., 7.]
    }
    fn triangle(x: f32, z: f32, size: f32, layer: f32) -> Vec<f32> {
        [
            vertex(x, 0., z, layer),
            vertex(x, 0., z + size, layer),
            vertex(x + size, 0., z, layer),
        ]
        .concat()
    }
    fn normals_for(vertices: &[f32]) -> Vec<f32> {
        // A distinct normal per vertex so the reorder can be followed.
        (0..vertices.len() / FLOATS)
            .flat_map(|i| [i as f32, 1., -(i as f32)])
            .collect()
    }

    fn grid() -> Vec<f32> {
        // Cells of 8,192 ft over 24 x 24 cells, two triangles each, in the
        // scenery's row order, alternating textured layers 0 and 1.
        let mut v = Vec::new();
        for y in 0..24 {
            for x in 0..24 {
                let (fx, fz) = (x as f32 * 8192., y as f32 * 8192.);
                let layer = ((x / 4 + y / 4) % 2) as f32;
                v.extend(triangle(fx, fz, 8192., layer));
                v.extend(
                    [
                        vertex(fx + 8192., 0., fz, layer),
                        vertex(fx, 0., fz + 8192., layer),
                        vertex(fx + 8192., 0., fz + 8192., layer),
                    ]
                    .concat(),
                );
            }
        }
        v
    }

    /// Every triangle survives the split exactly once, with its own normals,
    /// and keeps its source order within its chunk and class.
    #[test]
    fn chunks_hold_every_triangle_once_in_source_order() {
        let vertices = grid();
        let normals = normals_for(&vertices);
        let (sorted, sorted_normals, chunks) = build(&vertices, &normals, |layer| layer == 0.);
        assert_eq!(sorted.len(), vertices.len());
        assert_eq!(chunks.chunks.len(), 9);
        let source: Vec<&[f32]> = vertices.chunks_exact(TRIANGLE).collect();
        let index = |t: &[f32]| source.iter().position(|s| *s == t).unwrap();
        let mut seen = vec![false; source.len()];
        for c in &chunks.chunks {
            assert_eq!(c.count % 3, 0);
            for range in [c.opaque(), c.cutout()] {
                let mut last = None;
                for t in range.step_by(3) {
                    let at = t as usize * FLOATS;
                    let tri = &sorted[at..at + TRIANGLE];
                    let i = index(tri);
                    assert!(!seen[i]);
                    seen[i] = true;
                    assert!(last < Some(i), "source order kept");
                    last = Some(i);
                    assert_eq!(
                        &sorted_normals[t as usize * 3..t as usize * 3 + 9],
                        &normals[i * 9..i * 9 + 9]
                    );
                    let opaque = tri[5] == 0.;
                    assert_eq!(opaque, t < c.first + c.opaque);
                    for p in tri.chunks_exact(FLOATS) {
                        for (a, value) in p[..3].iter().map(|v| f64::from(*v)).enumerate() {
                            assert!(c.bounds.min[a] <= value && value <= c.bounds.max[a]);
                        }
                    }
                }
            }
        }
        assert!(seen.iter().all(|s| *s));
    }

    /// A triangle on a chunk edge goes with its centroid; runway-cutout
    /// pieces of one cell never split across chunks.
    #[test]
    fn a_cell_stays_in_one_chunk() {
        let mut v = triangle(65_536. - 8192., 0., 8192., -1.);
        v.extend(triangle(65_536. - 10., 10., 5., -1.));
        let normals = normals_for(&v);
        let (_, _, chunks) = build(&v, &normals, |_| true);
        assert_eq!(chunks.chunks.len(), 1);
    }

    fn view(yaw: f32, pitch: f32) -> Clip {
        let camera = crate::camera::Camera {
            yaw,
            pitch,
            ..crate::camera::Camera::new()
        };
        let u = camera.uniform(16. / 9., [0.; 4], [0; 3]);
        Clip::perspective(
            [u[4], u[5], u[6]],
            [u[8], u[9], u[10]],
            [u[12], u[13], u[14]],
            u[11],
            u[3],
            1.,
            2_200_000.,
        )
    }

    /// Brute force against the shader's clip test: a box is rejected only
    /// when none of a dense set of its points is inside the volume, and a
    /// box straddling a plane is always kept.
    #[test]
    fn frustum_rejects_only_boxes_wholly_outside() {
        let clip = view(0.4, -0.1);
        let camera = crate::camera::Camera {
            yaw: 0.4,
            pitch: -0.1,
            ..crate::camera::Camera::new()
        };
        let u = camera.uniform(16. / 9., [0.; 4], [0; 3]);
        let dot = |a: &[f32], p: [f64; 3]| (0..3).map(|i| f64::from(a[i]) * p[i]).sum::<f64>();
        let inside = |p: [f64; 3]| {
            let f = 1.732_050_8 * f64::from(u[11]);
            let z = dot(&u[12..15], p);
            let x = dot(&u[4..7], p) * f / f64::from(u[3]);
            let y = dot(&u[8..11], p) * f;
            (1. ..=2_200_000.).contains(&z) && x.abs() <= z && y.abs() <= z
        };
        let mut rejected = 0;
        for i in -6..6 {
            for j in -6..6 {
                for k in -2..2 {
                    let min = [i as f64 * 40_000., k as f64 * 20_000., j as f64 * 40_000.];
                    let b = Bounds {
                        min,
                        max: [min[0] + 40_000., min[1] + 20_000., min[2] + 40_000.],
                    };
                    let any = (0..=8).any(|a| {
                        (0..=8).any(|c| {
                            (0..=8).any(|e| {
                                inside(std::array::from_fn(|n| {
                                    let t = [a, c, e][n] as f64 / 8.;
                                    b.min[n] + (b.max[n] - b.min[n]) * t
                                }))
                            })
                        })
                    });
                    if clip.rejects(&b) {
                        rejected += 1;
                        assert!(!any, "a visible box was culled: {b:?}");
                    }
                }
            }
        }
        assert!(
            rejected > 200,
            "the test boxes exercise culling: {rejected}"
        );
    }

    /// The far plane and the near plane cull along the view axis.
    #[test]
    fn far_and_behind_chunks_are_culled() {
        let clip = view(0., 0.);
        let facing = crate::camera::Camera {
            yaw: 0.,
            pitch: 0.,
            ..crate::camera::Camera::new()
        }
        .facing()
        .map(f64::from);
        let at = |d: f64| {
            let c = facing.map(|v| v * d);
            Bounds {
                min: c.map(|v| v - 10.),
                max: c.map(|v| v + 10.),
            }
        };
        assert!(!clip.rejects(&at(100.)));
        assert!(!clip.rejects(&at(2_100_000.)));
        assert!(clip.rejects(&at(2_300_000.)));
        assert!(clip.rejects(&at(-100.)));
    }

    /// A cascade keeps exactly the chunks under its square, whatever the
    /// sun, and the depth range along the sun.
    #[test]
    fn cascade_volume_matches_its_projection() {
        for m in [
            crate::surface_lighting::test_projection([0., 1., 0.], 8192.),
            crate::surface_lighting::test_projection([0.6, 0.64, 0.48], 131_072.),
        ] {
            let clip = Clip::orthographic(&m);
            let project = |p: [f64; 3]| -> [f64; 3] {
                std::array::from_fn(|r| {
                    (0..3).map(|i| f64::from(m[r * 4 + i]) * p[i]).sum::<f64>()
                        + f64::from(m[r * 4 + 3])
                })
            };
            let inside = |p: [f64; 3]| {
                let q = project(p);
                q[0].abs() <= 1. && q[1].abs() <= 1. && (0. ..=1.).contains(&q[2])
            };
            let extent = f64::from(m[13]);
            let mut kept = 0;
            for i in -8..8 {
                for j in -8..8 {
                    let step = extent / 2.;
                    let min = [i as f64 * step, -3000., j as f64 * step];
                    let b = Bounds {
                        min,
                        max: [min[0] + step, 3000., min[2] + step],
                    };
                    let any = (0..=6).any(|a| {
                        (0..=6).any(|c| {
                            (0..=6).any(|e| {
                                inside(std::array::from_fn(|n| {
                                    let t = [a, c, e][n] as f64 / 6.;
                                    b.min[n] + (b.max[n] - b.min[n]) * t
                                }))
                            })
                        })
                    });
                    if clip.rejects(&b) {
                        assert!(!any, "a casting box was culled: {b:?}");
                    } else {
                        kept += 1;
                    }
                }
            }
            assert!(kept > 0 && kept < 256, "kept {kept} of 256");
        }
    }

    /// Nearest chunk first; the chunk under the eye leads.
    #[test]
    fn chunks_draw_front_to_back() {
        let vertices = grid();
        let normals = normals_for(&vertices);
        let (_, _, chunks) = build(&vertices, &normals, |_| true);
        let everything = Clip::default();
        let eye = [150_000., 0., 150_000.];
        let mut order = Vec::new();
        chunks.front_to_back(eye, &everything, &mut order);
        assert_eq!(order.len(), chunks.chunks.len());
        let distance = |r: &Range<u32>| {
            let c = chunks.chunks.iter().find(|c| c.all() == *r).unwrap();
            c.bounds.relative(eye).distance()
        };
        assert_eq!(distance(&order[0]), 0.);
        for pair in order.windows(2) {
            assert!(distance(&pair[0]) <= distance(&pair[1]));
        }
        // The same ranges as the whole buffer, once each.
        let mut all: Vec<u32> = order.iter().flat_map(|r| r.clone()).collect();
        all.sort_unstable();
        assert_eq!(
            all,
            (0..(vertices.len() / FLOATS) as u32).collect::<Vec<_>>()
        );
    }

    /// Caster ranges merge when adjacent and cover each kept chunk once.
    #[test]
    fn caster_ranges_split_opaque_from_cutout() {
        let vertices = grid();
        let normals = normals_for(&vertices);
        let (sorted, _, chunks) = build(&vertices, &normals, |layer| layer == 0.);
        let (mut opaque, mut cutout) = (Vec::new(), Vec::new());
        chunks.casters([0.; 3], &Clip::default(), &mut opaque, &mut cutout);
        let total: u32 = opaque.iter().chain(&cutout).map(|r| r.len() as u32).sum();
        assert_eq!(total as usize, vertices.len() / FLOATS);
        for r in &opaque {
            for t in r.clone().step_by(3) {
                assert_eq!(sorted[t as usize * FLOATS + 5], 0.);
            }
        }
        for r in &cutout {
            for t in r.clone().step_by(3) {
                assert_eq!(sorted[t as usize * FLOATS + 5], 1.);
            }
        }
    }

    #[test]
    fn batch_bounds_cover_every_vertex_and_never_cull_non_finite() {
        let v = [vertex(1., 2., 3., 0.), vertex(-4., 5., 6., 0.)].concat();
        let b = Bounds::of_vertices(&v);
        assert_eq!(b.min, [-4., 2., 3.]);
        assert_eq!(b.max, [1., 5., 6.]);
        assert!(Bounds::of_vertices(&[]).is_empty());
        let odd = [vertex(1., 2., 3., 0.), vertex(f32::NAN, 0., 0., 0.)].concat();
        let b = Bounds::of_vertices(&odd);
        assert!(!view(0., 0.).rejects(&b.relative([0.; 3])));
    }
}
