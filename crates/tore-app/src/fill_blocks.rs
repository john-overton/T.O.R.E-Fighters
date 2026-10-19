//! City fill blocks and sparse flag uploads (optimization slice S13b).
//!
//! The cull pass used to run one GPU thread per building of the theater
//! (up to 500,000) in every view. The buildings are now stored on the GPU in
//! spatial blocks of at most [`BLOCK`] records, each with a conservative
//! bounding sphere; per view the CPU keeps only the blocks that may hold a
//! drawn building and dispatches one workgroup per block. The per-building
//! test in `fill_cull.wgsl` is unchanged, so the drawn set is the same:
//! a block is dropped only when every building in it fails one of that
//! test's own rejections (draw distance, behind the eye, outside a side of
//! the view, under the smallest drawn size).
//!
//! Knocked-down buildings change one flag word per record. Instead of
//! re-uploading every record (16 MB at the cap), [`flag_writes`] writes only
//! the runs of records whose flags changed.

use std::ops::Range;

/// Records per block: one cull workgroup.
pub const BLOCK: usize = 256;
/// Side of the square cells that group buildings into blocks, feet. A block
/// never spans two cells, so its sphere stays small.
const CELL: f64 = 8_192.;
/// The most blocks a theater may have: one workgroup each in one dispatch
/// dimension (WebGPU's guaranteed 65,535, with room to spare).
pub const MAX_BLOCKS: usize = 60_000;

/// What the cull shader tests of one building, in world feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    /// Base centre (x, ground, z).
    pub pos: [f64; 3],
    /// The kit piece's height, footprint radius and on-screen size measure
    /// (`max(2 * radius, height)`).
    pub height: f64,
    pub radius: f64,
}

/// A run of GPU records and a sphere holding every building's test point
/// (its base centre raised by half its height).
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub records: Range<u32>,
    pub center: [f64; 3],
    pub radius: f64,
    /// The largest footprint radius and on-screen size measure inside.
    pub piece_radius: f64,
    pub size: f64,
}

/// The cull's view, as the uniform gives it to the shader (world feet for
/// the eye; the axes and tangents are the uniform's own f32 values).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub eye: [f64; 3],
    pub forward: [f64; 3],
    pub right: [f64; 3],
    pub up: [f64; 3],
    /// The building distance, feet.
    pub draw: f64,
    /// Half-view tangents across and up, pixels per unit size at unit
    /// distance, the smallest drawn size in pixels (`params.view`).
    pub view: [f64; 4],
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Morton interleave of two 5-bit numbers.
fn morton(x: u32, z: u32) -> u32 {
    let mut out = 0;
    for bit in 0..5 {
        out |= ((x >> bit) & 1) << (2 * bit) | ((z >> bit) & 1) << (2 * bit + 1);
    }
    out
}

/// The GPU order of the buildings (slot to building index) and the blocks
/// over it: buildings sorted by cell, then along a Z curve of 256 ft
/// squares inside it, then by index; blocks cut at each cell and every
/// [`BLOCK`] records. Cells double until there are at most
/// [`MAX_BLOCKS`].
pub fn arrange(buildings: &[Building]) -> (Vec<u32>, Vec<Block>) {
    let mut cell = CELL;
    loop {
        let (order, blocks) = arrange_in(buildings, cell);
        if blocks.len() <= MAX_BLOCKS || cell > 1e9 {
            return (order, blocks);
        }
        cell *= 2.;
    }
}

fn arrange_in(buildings: &[Building], cell: f64) -> (Vec<u32>, Vec<Block>) {
    let key = |b: &Building| {
        let cx = (b.pos[0] / cell).floor();
        let cz = (b.pos[2] / cell).floor();
        let fine =
            |v: f64, c: f64| (((v - c * cell) / cell * 32.).floor() as i64).clamp(0, 31) as u32;
        (
            cz as i64,
            cx as i64,
            morton(fine(b.pos[0], cx), fine(b.pos[2], cz)),
        )
    };
    let keys: Vec<_> = buildings.iter().map(key).collect();
    let mut order: Vec<u32> = (0..buildings.len() as u32).collect();
    order.sort_by_key(|&n| (keys[n as usize], n));
    let mut blocks = Vec::new();
    let mut start = 0;
    while start < order.len() {
        let cell_of = |slot: usize| (keys[order[slot] as usize].0, keys[order[slot] as usize].1);
        let mut end = start + 1;
        while end < order.len() && end - start < BLOCK && cell_of(end) == cell_of(start) {
            end += 1;
        }
        blocks.push(bound(buildings, &order[start..end], start as u32));
        start = end;
    }
    (order, blocks)
}

/// The test point the shader measures from: base centre, half height up.
fn point(b: &Building) -> [f64; 3] {
    [b.pos[0], b.pos[1] + b.height * 0.5, b.pos[2]]
}

fn bound(buildings: &[Building], members: &[u32], start: u32) -> Block {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut piece_radius = 0f64;
    let mut size = 0f64;
    for &n in members {
        let b = &buildings[n as usize];
        let p = point(b);
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
        piece_radius = piece_radius.max(b.radius);
        size = size.max((2. * b.radius).max(b.height));
    }
    let center = std::array::from_fn(|k| (lo[k] + hi[k]) * 0.5);
    let radius = members
        .iter()
        .map(|&n| length(sub(point(&buildings[n as usize]), center)))
        .fold(0f64, f64::max);
    Block {
        records: start..start + members.len() as u32,
        center,
        radius,
        piece_radius,
        size,
    }
}

/// Feet added to every sphere: the shader works in single precision
/// relative to the render origin (a few hundredths of a foot of error at
/// theater coordinates, far less than this), plus a share of the distance.
fn slack(distance: f64) -> f64 {
    16. + distance * 1e-4
}

/// Whether any building in `block` may pass the cull shader's view tests.
/// Never false for a block holding a building the shader would draw.
pub fn visible(block: &Block, view: &View) -> bool {
    let d = sub(block.center, view.eye);
    let dist = length(d);
    let r = block.radius + slack(dist);
    // Every building is at least dist - r away: past the draw distance.
    if dist - r >= view.draw {
        return false;
    }
    // Behind the eye: z < -radius for each.
    if dot(d, view.forward) + r < -block.piece_radius {
        return false;
    }
    // Outside one side: |x| > z * tangent + 1.5 radius for each, on the same
    // side (the plane through the eye with normal side - tangent * forward).
    for (axis, tangent) in [(view.right, view.view[0]), (view.up, view.view[1])] {
        for sign in [1., -1.] {
            let n: [f64; 3] = std::array::from_fn(|k| sign * axis[k] - tangent * view.forward[k]);
            if dot(n, d) - length(n) * r > 1.5 * block.piece_radius {
                return false;
            }
        }
    }
    // Under the smallest drawn size: px = size * scale / max(dist, 1).
    let nearest = (dist - r).max(1.);
    if view.view[3] > 0. && block.size * view.view[2] / nearest < view.view[3] {
        return false;
    }
    true
}

/// A building's flag word: hidden by a mission object (bit 0), knocked
/// down (bit 1).
pub fn flags(hidden: bool, flattened: bool) -> u32 {
    u32::from(hidden) | (u32::from(flattened) << 1)
}

/// Records closer than this many apart are written in one run.
const GAP: u32 = 8;
/// More runs than this are written as one span.
const MAX_RUNS: usize = 256;

/// The buffer writes that take the GPU records from the flattened set
/// `before` to `now` (both sorted building indices): `(byte offset, bytes)`
/// per run of changed slots, each record whole with its new flags.
/// `records` are the GPU-order records (eight words each, the flags word
/// last holding only the hidden bit), `slot_of` maps a building index to
/// its slot and `flattened` reports a building's new state.
pub fn flag_writes(
    records: &[u32],
    slot_of: &[u32],
    before: &[u32],
    now: &[u32],
    flattened: impl Fn(usize) -> bool,
    order: &[u32],
) -> Vec<(u64, Vec<u8>)> {
    // The symmetric difference of two sorted lists.
    let mut changed = Vec::new();
    let (mut a, mut b) = (0, 0);
    while a < before.len() || b < now.len() {
        match (before.get(a), now.get(b)) {
            (Some(x), Some(y)) if x == y => {
                a += 1;
                b += 1;
            }
            (Some(x), Some(y)) if x < y => {
                changed.push(slot_of[*x as usize]);
                a += 1;
            }
            (Some(_), Some(y)) => {
                changed.push(slot_of[*y as usize]);
                b += 1;
            }
            (Some(x), None) => {
                changed.push(slot_of[*x as usize]);
                a += 1;
            }
            (None, Some(y)) => {
                changed.push(slot_of[*y as usize]);
                b += 1;
            }
            (None, None) => break,
        }
    }
    if changed.is_empty() {
        return Vec::new();
    }
    changed.sort_unstable();
    let mut runs: Vec<Range<u32>> = Vec::new();
    for slot in changed {
        match runs.last_mut() {
            Some(run) if slot < run.end + GAP => run.end = slot + 1,
            _ => runs.push(slot..slot + 1),
        }
    }
    if runs.len() > MAX_RUNS {
        let span = runs[0].start..runs[runs.len() - 1].end;
        runs = vec![span];
    }
    runs.into_iter()
        .map(|run| {
            let mut bytes = Vec::with_capacity(run.len() * 32);
            for slot in run.clone() {
                let s = slot as usize;
                let words = &records[s * 8..s * 8 + 8];
                for &w in &words[..7] {
                    bytes.extend_from_slice(&w.to_le_bytes());
                }
                let word = (words[7] & 1) | (u32::from(flattened(order[s] as usize)) << 1);
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            (u64::from(run.start) * 32, bytes)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator (no new dependency).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next()
        }
    }

    /// The cull shader's per-building decision up to "drawn at all" (its
    /// early returns and the fade), in double precision.
    fn shader_draws(b: &Building, v: &View) -> bool {
        let d = sub(point(b), v.eye);
        let dist = length(d);
        if dist >= v.draw {
            return false;
        }
        let r = b.radius;
        let z = dot(d, v.forward);
        if z < -r {
            return false;
        }
        if dot(d, v.right).abs() > z * v.view[0] + r * 1.5 {
            return false;
        }
        if dot(d, v.up).abs() > z * v.view[1] + r * 1.5 {
            return false;
        }
        let px = (2. * r).max(b.height) * v.view[2] / dist.max(1.);
        if px < v.view[3] {
            return false;
        }
        let small = if v.view[3] <= 0. {
            1.
        } else {
            ((px - v.view[3]) / v.view[3]).clamp(0., 1.)
        };
        small > 0.
    }

    fn view(rng: &mut Rng, around: [f64; 2]) -> View {
        let (yaw, pitch, roll) = (
            rng.range(-3.2, 3.2),
            rng.range(-1.5, 1.5),
            rng.range(-0.5, 0.5),
        );
        let (sy, cy) = f64::sin_cos(yaw);
        let (sp, cp) = f64::sin_cos(pitch);
        let (sr, cr) = f64::sin_cos(roll);
        let zoom = rng.range(1., 8.);
        let f = 1.732_050_8 * zoom;
        let aspect = rng.range(1., 2.4);
        View {
            eye: [
                around[0] + rng.range(-40_000., 40_000.),
                rng.range(50., 30_000.),
                around[1] + rng.range(-40_000., 40_000.),
            ],
            forward: [sy * cp, sp, cy * cp],
            right: [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr],
            up: [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr],
            draw: rng.range(5_000., 200_000.),
            view: [
                aspect / f * 1.05,
                1. / f * 1.05,
                f * rng.range(100., 1100.),
                [0., 1., 1., 2.][(rng.next() * 4.) as usize % 4],
            ],
        }
    }

    fn town(rng: &mut Rng, n: usize) -> Vec<Building> {
        (0..n)
            .map(|_| {
                // Clusters and scattered houses over 60,000 ft.
                let (x, z) = if rng.next() < 0.7 {
                    (rng.range(-6_000., 6_000.), rng.range(-6_000., 6_000.))
                } else {
                    (rng.range(-30_000., 30_000.), rng.range(-30_000., 30_000.))
                };
                let height = rng.range(10., 600.);
                Building {
                    pos: [x.round(), rng.range(0., 400.), z.round()],
                    height,
                    radius: rng.range(15., 300.),
                }
            })
            .collect()
    }

    #[test]
    fn blocks_cover_every_building_once_in_cells() {
        let mut rng = Rng(7);
        let buildings = town(&mut rng, 20_000);
        let (order, blocks) = arrange(&buildings);
        let mut seen = vec![false; buildings.len()];
        for &n in &order {
            assert!(!seen[n as usize]);
            seen[n as usize] = true;
        }
        assert!(seen.iter().all(|s| *s));
        let mut next = 0;
        for block in &blocks {
            assert_eq!(block.records.start, next);
            assert!(block.records.len() <= BLOCK && !block.records.is_empty());
            next = block.records.end;
            for slot in block.records.clone() {
                let b = &buildings[order[slot as usize] as usize];
                assert!(length(sub(point(b), block.center)) <= block.radius + 1e-9);
                assert!(b.radius <= block.piece_radius);
                assert!((2. * b.radius).max(b.height) <= block.size);
            }
            // One cell: no block is wider than a cell's diagonal.
            assert!(block.radius <= CELL * 0.75 + 400.);
        }
        assert_eq!(next as usize, buildings.len());
    }

    /// The conservative bound: across random towns and views, no block
    /// holding a building the shader draws is dropped, and the coarse test
    /// still drops most blocks for views that see little.
    #[test]
    fn block_culling_never_drops_a_drawn_building() {
        let mut rng = Rng(11);
        let buildings = town(&mut rng, 30_000);
        let (order, blocks) = arrange(&buildings);
        let mut kept = 0usize;
        let mut total = 0usize;
        let mut drawn_total = 0usize;
        for _ in 0..400 {
            let v = view(&mut rng, [0., 0.]);
            for block in &blocks {
                total += 1;
                let keep = visible(block, &v);
                kept += usize::from(keep);
                for slot in block.records.clone() {
                    let b = &buildings[order[slot as usize] as usize];
                    if shader_draws(b, &v) {
                        drawn_total += 1;
                        assert!(keep, "a drawn building's block was culled: {b:?} {v:?}");
                    }
                }
            }
        }
        assert!(drawn_total > 0);
        assert!(kept * 2 < total, "kept {kept} of {total} blocks");
    }

    /// Buildings right at the edges of each test (the eye inside a block,
    /// a building on a side plane, at the draw distance, at the smallest
    /// size) keep their blocks.
    #[test]
    fn block_culling_keeps_buildings_on_each_edge() {
        let v = View {
            eye: [0., 100., 0.],
            forward: [0., 0., 1.],
            right: [1., 0., 0.],
            up: [0., 1., 0.],
            draw: 50_000.,
            view: [0.6, 0.6, 1000., 1.],
        };
        let one = |pos: [f64; 3], height: f64, radius: f64| {
            let b = Building {
                pos,
                height,
                radius,
            };
            let (_, blocks) = arrange(&[b]);
            (shader_draws(&b, &v), visible(&blocks[0], &v))
        };
        for (pos, h, r) in [
            ([0., 0., 49_990.], 10., 20.), // just inside the draw distance
            ([0., 100., -19.], 0., 20.),   // just behind, within its radius
            ([0.6 * 1000. + 29., 100., 1000.], 0., 20.), // on the right edge
            ([0., 100. + 0.6 * 1000. + 29., 1000.], 0., 20.), // on the top edge
            ([0., 95., 39_000.], 10., 20.), // at the smallest drawn size
            ([3., 90., 3.], 20., 30.),     // around the eye
        ] {
            let (draws, keep) = one(pos, h, r);
            assert!(!draws || keep, "{pos:?}");
        }
    }

    fn full_upload(records: &[u32], order: &[u32], flattened: &[u32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(records.len() * 4);
        for (s, words) in records.chunks_exact(8).enumerate() {
            for &w in &words[..7] {
                out.extend_from_slice(&w.to_le_bytes());
            }
            let flat = flattened.binary_search(&order[s]).is_ok();
            out.extend_from_slice(&flags(words[7] & 1 != 0, flat).to_le_bytes());
        }
        out
    }

    /// The sparse writes leave the buffer exactly as a full upload of the
    /// new state would: knock-downs, restores (a replay seek or restart),
    /// hidden buildings and wide areas included.
    #[test]
    fn sparse_flag_writes_match_a_full_upload() {
        let mut rng = Rng(3);
        let buildings = town(&mut rng, 5_000);
        let (order, _) = arrange(&buildings);
        let mut slot_of = vec![0u32; order.len()];
        for (s, &n) in order.iter().enumerate() {
            slot_of[n as usize] = s as u32;
        }
        let records: Vec<u32> = order
            .iter()
            .flat_map(|&n| {
                let hidden = n % 37 == 0;
                [
                    n,
                    n * 3,
                    n ^ 0x55,
                    7,
                    1,
                    2,
                    n.wrapping_mul(9),
                    flags(hidden, false),
                ]
            })
            .collect();
        let mut state: Vec<u32> = Vec::new();
        let mut gpu = full_upload(&records, &order, &state);
        let mut writes_total = 0;
        for step in 0..60 {
            // Knock down a circle, restore a few, or restore everything.
            let mut next = state.clone();
            match step % 6 {
                5 => next.clear(),
                4 => next.retain(|_| rng.next() < 0.7),
                _ => {
                    let c = [rng.range(-8_000., 8_000.), rng.range(-8_000., 8_000.)];
                    let r = rng.range(50., 3_000.);
                    for (n, b) in buildings.iter().enumerate() {
                        if (b.pos[0] - c[0]).hypot(b.pos[2] - c[1]) < r {
                            next.push(n as u32);
                        }
                    }
                    next.sort_unstable();
                    next.dedup();
                }
            }
            let writes = flag_writes(
                &records,
                &slot_of,
                &state,
                &next,
                |n| next.binary_search(&(n as u32)).is_ok(),
                &order,
            );
            for (offset, bytes) in &writes {
                assert_eq!(offset % 4, 0);
                assert_eq!(bytes.len() % 4, 0);
                gpu[*offset as usize..*offset as usize + bytes.len()].copy_from_slice(bytes);
                writes_total += bytes.len();
            }
            state = next;
            assert_eq!(gpu, full_upload(&records, &order, &state), "step {step}");
        }
        // Far less than 60 full uploads.
        assert!(writes_total < 60 * records.len() * 4 / 4);
        assert!(flag_writes(&records, &slot_of, &state, &state, |_| false, &order).is_empty());
    }
}
