//! Fitted beach mask. Light sand is the landward boundary; every colour on its
//! seaward side is excluded. Dark landward Vietnam bands remain eligible.
use super::{Atlas, TEXEL, WATER_CELL};

#[derive(Default)]
pub(super) struct Coast {
    x0: i64,
    z0: i64,
    cols: usize,
    rows: usize,
    blocked: Vec<bool>,
}
impl Coast {
    pub(super) fn build(atlas: &Atlas) -> Self {
        let w = &atlas.water;
        if w.cols == 0 || w.rows == 0 {
            return Self::default();
        }
        let per = (WATER_CELL / TEXEL) as usize;
        let cols = w.cols * per;
        let rows = w.rows * per;
        let mut wet = vec![false; w.cols * w.rows];
        for z in 0..w.rows {
            for x in 0..w.cols {
                // Only actual water cutouts, never a blue/green palette guess.
                let mut n = 0;
                for dz in 0..per {
                    for dx in 0..per {
                        if atlas.index_at(
                            w.x0 + ((x * per + dx) as i64) * TEXEL + TEXEL / 2,
                            w.z0 + ((z * per + dz) as i64) * TEXEL + TEXEL / 2,
                        ) == Some(255)
                        {
                            n += 1;
                        }
                    }
                }
                wet[z * w.cols + x] = n * 4 >= per * per * 3;
            }
        }
        // A water interior at least 2,560 ft wide seeds large water. Narrow
        // rivers retain their green belts. This is a fitted width, not geography.
        let mut sea = vec![false; wet.len()];
        if w.cols >= 5 && w.rows >= 5 {
            for z in 2..w.rows - 2 {
                for x in 2..w.cols - 2 {
                    sea[z * w.cols + x] =
                        (z - 2..=z + 2).all(|r| (x - 2..=x + 2).all(|c| wet[r * w.cols + c]));
                }
            }
        }
        let mut dist = vec![255u8; cols * rows];
        let mut sand = vec![false; cols * rows];
        for z in 0..rows {
            for x in 0..cols {
                let at = z * cols + x;
                if sea[(z / per) * w.cols + x / per] {
                    dist[at] = 0;
                }
                if let Some(i) = atlas.index_at(
                    w.x0 + x as i64 * TEXEL + TEXEL / 2,
                    w.z0 + z as i64 * TEXEL + TEXEL / 2,
                ) {
                    sand[at] = i != 255 && light_sand(atlas.palette[usize::from(i)]);
                }
            }
        }
        distance(&mut dist, cols, rows);
        let blocked = seaward(&sand, &dist, cols, rows);
        Self {
            x0: w.x0,
            z0: w.z0,
            cols,
            rows,
            blocked,
        }
    }
    pub(super) fn contains(&self, x: i64, z: i64) -> bool {
        let c = (x - self.x0).div_euclid(TEXEL);
        let r = (z - self.z0).div_euclid(TEXEL);
        c >= 0
            && r >= 0
            && (c as usize) < self.cols
            && (r as usize) < self.rows
            && self.blocked[r as usize * self.cols + c as usize]
    }
}
/// Clear reference-palette sand, including white. Dark bands are not seeds.
fn light_sand([r, g, b]: [u8; 3]) -> bool {
    let [r, g, b] = [r, g, b].map(i32::from);
    (299 * r + 587 * g + 114 * b) / 1000 >= 105
        && r >= g * 92 / 100
        && g >= b * 9 / 10
        && r - b >= -8
}
fn distance(d: &mut [u8], cols: usize, rows: usize) {
    for z in 0..rows {
        for x in 0..cols {
            let at = z * cols + x;
            if x > 0 {
                d[at] = d[at].min(d[at - 1].saturating_add(1));
            }
            if z > 0 {
                d[at] = d[at].min(d[at - cols].saturating_add(1));
            }
        }
    }
    for z in (0..rows).rev() {
        for x in (0..cols).rev() {
            let at = z * cols + x;
            if x + 1 < cols {
                d[at] = d[at].min(d[at + 1].saturating_add(1));
            }
            if z + 1 < rows {
                d[at] = d[at].min(d[at + cols].saturating_add(1));
            }
        }
    }
}
fn seaward(sand: &[bool], dist: &[u8], cols: usize, rows: usize) -> Vec<bool> {
    // Sand within 8,192 ft of a large-water interior starts the beach. Walk
    // only toward water, so a dark band behind the sand is never swallowed.
    let mut out: Vec<bool> = sand.iter().zip(dist).map(|(&s, &d)| s && d <= 64).collect();
    let mut bands = vec![Vec::new(); 65];
    for (at, &d) in dist.iter().enumerate() {
        if d <= 64 {
            bands[usize::from(d)].push(at);
        }
    }
    for d in (1..=64).rev() {
        for &at in &bands[d] {
            if !out[at] {
                continue;
            }
            let x = at % cols;
            let z = at / cols;
            for next in [
                x.checked_sub(1).map(|_| at - 1),
                (x + 1 < cols).then_some(at + 1),
                z.checked_sub(1).map(|_| at - cols),
                (z + 1 < rows).then_some(at + cols),
            ]
            .into_iter()
            .flatten()
            {
                if dist[next] < dist[at] {
                    out[next] = true;
                }
            }
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_placement_layer_obeys_the_same_beach_boundary() {
        let mut pixels = vec![12; 65536];
        for z in 0..256 {
            for x in 0..256 {
                pixels[z * 256 + x] = match x {
                    0..96 => 255,
                    96..112 => 12,
                    112..128 => 20,
                    128..144 => 10,
                    _ => 12,
                };
            }
        }
        let a = Atlas::synthetic(pixels, 0);
        let mut world = tore_world::test_support::terrain();
        for c in &mut world.theater.cells {
            c.elevation = 0;
        }
        for x in [100, 120] {
            let site = a.site(&world, f64::from(x) * 128., 16000.);
            assert!(site.blocked);
            assert_eq!(a.tree_density(&site), 0.);
            assert_eq!(a.bush_density(&site), 0.);
            assert_eq!(a.rock_density(&site), 0.);
            assert_eq!(a.cover(&site).0, 0.);
        }
        let inland = a.site(&world, 136. * 128., 16000.);
        assert!(!inland.blocked, "dark band behind sand stays eligible");
        assert!(a.tree_density(&inland) > 0.);
    }

    #[test]
    fn sand_boundary_excludes_seaward_green_but_keeps_dark_inland_band() {
        assert!(light_sand([200, 185, 140]));
        assert!(light_sand([240, 240, 235]));
        assert!(!light_sand([40, 32, 21]));
        assert!(!light_sand([48, 112, 52]));
        // Sea, blue shallows, green patch, sand, dark band, green land.
        let mask = seaward(
            &[false, false, false, true, false, false],
            &[0, 1, 2, 3, 4, 5],
            6,
            1,
        );
        assert_eq!(mask, [true, true, true, true, false, false]);
        assert_eq!(
            seaward(&[false, true, false], &[255, 255, 255], 3, 1),
            [false; 3]
        );
    }
    #[test]
    fn narrow_river_does_not_become_a_beach() {
        let mut pixels = vec![12; 65536];
        for y in 0..256 {
            for x in 125..128 {
                pixels[y * 256 + x] = 255;
            }
        }
        let a = Atlas::synthetic(pixels, 0);
        assert!(!a.coast.contains(120 * 128, 120 * 128));
    }
}
