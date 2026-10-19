//! Opinionated GPU grass, requested 2026-10-17. No blade positions exist on the
//! CPU. The loader caches habitat and terrain lookup textures beside canopy.
use super::{Atlas, CELL, Kind, TEXEL, WATER_CELL};
use crate::{graphics::VegDensity, terrain::Terrain};
use std::collections::BTreeMap;
use tore_world::city::mask::{GREEN, LightMap, SITE, STREET, WATER};

#[path = "grass_blades.rs"]
pub mod gpu;

pub const VERTICES: u32 = 5;
pub const LEVELS: u32 = 4;
pub const PAGE: usize = 256;

/// Each density has its own world lattice. Four grids own disjoint subsets,
/// with every second cell in both axes assigned to the coarser grid.
pub fn spacing(density: VegDensity) -> f32 {
    match density {
        VegDensity::Low => 2.,
        VegDensity::Medium => 1.5,
        VegDensity::High => 1.,
        VegDensity::Ultra => 0.75,
    }
}
pub fn side(density: VegDensity, radius: f32) -> u32 {
    let minimum = match density {
        VegDensity::Low => 128,
        VegDensity::Medium => 192,
        VegDensity::High => 256,
        VegDensity::Ultra => 320,
    };
    // Two guard cells, rounded to eight. Even Ultra at 1,000 ft reaches its
    // advertised endpoint without exceeding the fixed four-grid budget.
    let required = (2. * radius / (spacing(density) * 8.)).ceil() as u32 + 4;
    minimum.max(required.div_ceil(8) * 8)
}
pub fn candidates(density: VegDensity, radius: f32) -> u32 {
    LEVELS * side(density, radius).pow(2)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map {
    /// Origin in feet, width/height in 512 ft blocks. Each RGBA texel encodes
    /// sixteen 128 ft habitats, two bits each: bare, meadow, crop, short.
    pub area: [f32; 4],
    pub pages_x: u32,
    pub mask: Vec<u8>,
    pub terrain_size: [u32; 2],
    /// Per terrain vertex: ground elevation, terrain picture page (or -1),
    /// picture rotation, untextured palette index. No retail bytes embedded.
    pub terrain: Vec<f32>,
}

/// No sand, rock or snow, including in a desert. An inland water belt only
/// qualifies where the existing art classes actually identify growing ground.
fn habitat(kind: Kind, climate: u8, water_ft: f32) -> u8 {
    if climate == 1 && water_ft >= 2600. {
        return 0;
    }
    match kind {
        Kind::Grass if climate != 1 && climate != 2 => 1,
        Kind::Field => 2,
        Kind::Grass | Kind::Forest | Kind::Scrub => 3,
        _ => 0,
    }
}
impl Map {
    pub fn build(
        atlas: &Atlas,
        world: &Terrain,
        layers: &BTreeMap<(i32, i32), usize>,
        land: Option<usize>,
    ) -> Self {
        let w = &atlas.water;
        let nx = w.cols.div_ceil(PAGE);
        let nz = w.rows.div_ceil(PAGE);
        let kinds: [Kind; 256] = std::array::from_fn(|i| atlas.kind_index(i as u8));
        let tiles: Vec<_> = (0..nx * nz).collect();
        let built = tore_workers::bulk().ordered_map(&tiles, 1, |_, &tile| {
            let mut bytes = vec![0; PAGE * PAGE * 4];
            // Lookup locality avoids repeating a BTree search and colour
            // classification for every texel of the same source picture.
            let mut last = None;
            let mut kind_at = |wx: i64, wz: i64| {
                let key = LightMap::key(wx, wz);
                let placed = match last {
                    Some((old, placed)) if old == key => placed,
                    _ => {
                        let placed = atlas.placements.get(&key).copied();
                        last = Some((key, placed));
                        placed
                    }
                };
                let index = if let Some((pic, rotation)) = placed {
                    let u = ((wx - i64::from(key.0) * CELL) / TEXEL).clamp(0, 255) as usize;
                    let v = ((wz - i64::from(key.1) * CELL) / TEXEL).clamp(0, 255) as usize;
                    let (x, z) = super::rotate(u, v, rotation);
                    Some(atlas.pictures[usize::from(pic)].indices[z * 256 + x])
                } else {
                    atlas.cell_index(wx, wz)
                };
                index.map_or_else(
                    || super::bare_kind(atlas.region.climate),
                    |i| kinds[usize::from(i)],
                )
            };
            let city = world.city_fill.as_ref().map(|f| f.layout.map.lookup());
            for z in tile / nx * PAGE..((tile / nx + 1) * PAGE).min(w.rows) {
                for x in tile % nx * PAGE..((tile % nx + 1) * PAGE).min(w.cols) {
                    let at = ((z % PAGE) * PAGE + x % PAGE) * 4;
                    for dz in 0..4 {
                        for dx in 0..4 {
                            let wx = w.x0 + x as i64 * WATER_CELL + dx * TEXEL;
                            let wz = w.z0 + z as i64 * WATER_CELL + dz * TEXEL;
                            let center = [wx + TEXEL / 2, wz + TEXEL / 2];
                            let kind = kind_at(center[0], center[1]);
                            let mut code = habitat(
                                kind,
                                atlas.region.climate,
                                atlas.water.feet(center[0] as f64, center[1] as f64) as f32,
                            );
                            if code == 0 {
                                continue;
                            }
                            // Art, beach and city flags are constant over the
                            // aligned 128 ft texel. Geometry exclusions rasterize
                            // only their own bounds below, not every map sample.
                            let flags = city.as_ref().map_or(0, |get| get(center[0], center[1]));
                            if atlas.coast.contains(center[0], center[1])
                                || flags & WATER != 0
                                || flags & (SITE | STREET) != 0 && flags & GREEN == 0
                            {
                                code = 0;
                            }
                            bytes[at + dz as usize] |= code << (dx * 2);
                        }
                    }
                }
            }
            bytes
        });
        let mut terrain = Vec::with_capacity(world.theater.cols * world.theater.rows * 4);
        for row in 0..world.theater.rows {
            for col in 0..world.theater.cols {
                let cell = world.theater.cell(col, row);
                let key = ((col as i32) & !3, (row as i32) & !3);
                let layer = layers
                    .get(&key)
                    .copied()
                    .or(if cell.color != 255 { land } else { None })
                    .map_or(-1., |v| v as f32);
                let rotation = world
                    .environment
                    .textures
                    .get(&key)
                    .map_or(0, |p| p.rotation);
                terrain.extend([
                    f32::from(cell.elevation) * tore_formats::theater::HEIGHT_FEET,
                    layer,
                    f32::from(rotation),
                    f32::from(cell.color),
                ]);
            }
        }
        let mut map = Self {
            area: [w.x0 as f32, w.z0 as f32, w.cols as f32, w.rows as f32],
            pages_x: nx as u32,
            mask: built.into_iter().flatten().collect(),
            terrain_size: [world.theater.cols as u32, world.theater.rows as u32],
            terrain,
        };
        map.exclude_airports(world);
        map.exclude_towns(world);
        map
    }

    fn exclude_airports(&mut self, world: &Terrain) {
        let boxes = world
            .airport_scene
            .objects
            .iter()
            .filter(|o| o.runway)
            .map(|o| o.bounds)
            .chain(world.airport_scene.runways.iter().map(|r| r.surface));
        for mut b in boxes {
            // Cover the complete source airport shape, including gaps between
            // pavement pieces. Half the texel diagonal conservatively rejects
            // every habitat texel intersecting the rotated airport footprint.
            b.half[0] += 220. + 91.;
            b.half[2] += 220. + 91.;
            let (sin, cos) = b.heading.sin_cos();
            let half = [
                cos.abs() * b.half[0] + sin.abs() * b.half[2],
                sin.abs() * b.half[0] + cos.abs() * b.half[2],
            ];
            let center = [b.center[0], b.center[2]];
            let low: [usize; 2] = std::array::from_fn(|k| {
                ((center[k] - half[k] - f64::from(self.area[k])) / 128.)
                    .floor()
                    .max(0.) as usize
            });
            let high: [usize; 2] = std::array::from_fn(|k| {
                (((center[k] + half[k] - f64::from(self.area[k])) / 128.)
                    .ceil()
                    .max(0.) as usize)
                    .min(self.area[k + 2] as usize * 4)
            });
            for z in low[1]..high[1] {
                for x in low[0]..high[0] {
                    if !b.contains_horizontal(
                        f64::from(self.area[0]) + (x as f64 + 0.5) * 128.,
                        f64::from(self.area[1]) + (z as f64 + 0.5) * 128.,
                    ) {
                        continue;
                    }
                    let (bx, bz) = (x / 4, z / 4);
                    let page = bz / PAGE * self.pages_x as usize + bx / PAGE;
                    let at = (page * PAGE * PAGE + bz % PAGE * PAGE + bx % PAGE) * 4 + z % 4;
                    self.mask[at] &= !(3 << (x % 4 * 2));
                }
            }
        }
    }

    fn exclude_towns(&mut self, world: &Terrain) {
        let Some(fill) = &world.city_fill else {
            return;
        };
        for district in &fill.layout.map.districts {
            self.exclude_circle(
                district.center.map(|v| v as f64),
                district.half[0].max(district.half[1]).max(1) as f64,
            );
        }
    }
    fn exclude_circle(&mut self, center: [f64; 2], radius: f64) {
        let radius = radius + 91.;
        let lo: [usize; 2] = std::array::from_fn(|k| {
            ((center[k] - radius - f64::from(self.area[k])) / 128.)
                .floor()
                .max(0.) as usize
        });
        let hi: [usize; 2] = std::array::from_fn(|k| {
            (((center[k] + radius - f64::from(self.area[k])) / 128.)
                .ceil()
                .max(0.) as usize)
                .min(self.area[k + 2] as usize * 4)
        });
        for z in lo[1]..hi[1] {
            for x in lo[0]..hi[0] {
                let dx = f64::from(self.area[0]) + (x as f64 + 0.5) * 128. - center[0];
                let dz = f64::from(self.area[1]) + (z as f64 + 0.5) * 128. - center[1];
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let (bx, bz) = (x / 4, z / 4);
                let page = bz / PAGE * self.pages_x as usize + bx / PAGE;
                let at = (page * PAGE * PAGE + bz % PAGE * PAGE + bx % PAGE) * 4 + z % 4;
                self.mask[at] &= !(3 << (x % 4 * 2));
            }
        }
    }

    /// Same packed lookup as the vertex shader, also used by CPU probe cameras.
    #[cfg(test)]
    pub fn at(&self, x: f64, z: f64) -> u8 {
        let x = ((x - f64::from(self.area[0])) / TEXEL as f64).floor() as i64;
        let z = ((z - f64::from(self.area[1])) / TEXEL as f64).floor() as i64;
        if x < 0 || z < 0 || x >= self.area[2] as i64 * 4 || z >= self.area[3] as i64 * 4 {
            return 0;
        }
        let (x, z) = (x as usize, z as usize);
        let (bx, bz) = (x / 4, z / 4);
        let page = bz / PAGE * self.pages_x as usize + bx / PAGE;
        let at = (page * PAGE * PAGE + bz % PAGE * PAGE + bx % PAGE) * 4;
        (self.mask[at + z % 4] >> (x % 4 * 2)) & 3
    }
}

/// The blade is authored from normalized bottom 0 to tip 1, matching the
/// shared sprite footing shape. The one constant lift comes from grounding,
/// never a separate per-blade CPU terrain placement path.
pub fn root_lift() -> f32 {
    super::sprite_ground(0., 1.)
}

#[cfg(test)]
mod tests {
    use super::*;
    // CPU reference only. Runtime generation lives wholly in grass_blades.wgsl.
    fn hash(cell: [i32; 2], salt: u32) -> u32 {
        let mut h = (cell[0] as u32).wrapping_mul(0x9e3779b9)
            ^ (cell[1] as u32).wrapping_mul(0x85ebca6b)
            ^ salt;
        h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
        h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
        h ^ (h >> 16)
    }
    fn cell(eye: [f32; 2], level: u32, index: u32, side: u32) -> [i32; 2] {
        let step = 1i32 << level;
        let offset = [index % side, index / side];
        std::array::from_fn(|k| {
            ((eye[k] / step as f32).floor() as i32 + offset[k] as i32 - side as i32 / 2) * step
        })
    }
    fn owned(c: [i32; 2], level: u32) -> bool {
        level == LEVELS - 1
            || (c[0] & ((1 << (level + 1)) - 1)) != 0
            || (c[1] & ((1 << (level + 1)) - 1)) != 0
    }
    #[test]
    fn grass_hash_and_world_cells_survive_camera_motion_and_lod() {
        assert_eq!(hash([0, 0], 0), 0);
        assert_eq!(hash([-71, 293], 17), 0xb0986c98); // pinned integer contract
        let side = side(VegDensity::High, 500.);
        let field = |eye| {
            let mut cells = BTreeMap::new();
            for level in 0..LEVELS {
                for i in 0..side * side {
                    let c = cell(eye, level, i, side);
                    if owned(c, level) {
                        assert!(
                            cells.insert(c, hash(c, 17)).is_none(),
                            "no duplicate blades"
                        );
                    }
                }
            }
            cells
        };
        let a = field([-1., 700.]);
        let b = field([31., 731.]);
        let mut shared = 0;
        for (c, seed) in a {
            if let Some(other) = b.get(&c) {
                assert_eq!(&seed, other);
                shared += 1;
            }
        }
        assert!(shared > 10_000);
    }
    #[test]
    fn grass_ring_snapping_handles_negative_cells_and_only_exchanges_faded_edges() {
        let n = side(VegDensity::High, 500.);
        for level in 0..LEVELS {
            let step = (1u32 << level) as f32;
            assert_eq!(
                cell([0.1; 2], level, 0, n),
                cell([step - 0.1; 2], level, 0, n)
            );
            assert_eq!(
                cell([-0.1; 2], level, 0, n),
                cell([-step + 0.1; 2], level, 0, n)
            );
            let last = cell([step - 0.01, 0.], level, 0, n)[0] as f32;
            let fade_radius = (n / 2 - 1) as f32 * step;
            assert!(step - last > fade_radius + step);
        }
    }
    #[test]
    fn grass_density_has_a_hard_vertex_budget() {
        assert_eq!(
            VegDensity::ALL.map(|d| candidates(d, 500.) * VERTICES),
            [327_680, 737_280, 1_310_720, 2_048_000]
        );
        for d in VegDensity::ALL {
            for radius in crate::graphics::GRASS_DISTANCES {
                assert!((side(d, radius as f32) / 2 - 1) as f32 * spacing(d) * 8. > radius as f32);
                assert!(candidates(d, radius as f32) * VERTICES <= 2_366_720);
            }
        }
    }
    #[test]
    fn grass_classification_excludes_water_sand_roads_rock_and_remote_desert() {
        for climate in 0..4 {
            for kind in [
                Kind::Water,
                Kind::River,
                Kind::Road,
                Kind::Sand,
                Kind::Rock,
                Kind::Snow,
            ] {
                assert_eq!(habitat(kind, climate, 0.), 0);
            }
        }
        assert_eq!(habitat(Kind::Grass, 0, 4000.), 1);
        assert_eq!(habitat(Kind::Field, 0, 4000.), 2);
        assert_eq!(habitat(Kind::Forest, 0, 4000.), 3);
        assert_eq!(habitat(Kind::Grass, 1, 2600.), 0);
        assert_eq!(habitat(Kind::Grass, 1, 512.), 3);
        assert_eq!(root_lift(), 0.);
    }
    #[test]
    fn grass_packed_lookup_preserves_texel_boundaries_and_coast() {
        let mut pixels = vec![12; 65536];
        for row in pixels.chunks_mut(256) {
            for (x, p) in row.iter_mut().enumerate() {
                *p = match x {
                    0..64 => 255,
                    64..80 => 20,
                    80..96 => 40,
                    _ => 12,
                };
            }
        }
        let atlas = Atlas::synthetic(pixels, 0);
        let world = tore_world::test_support::terrain();
        let map = Map::build(&atlas, &world, &BTreeMap::new(), None);
        for x in 0..96 {
            assert_eq!(map.at((x * 128 + 64) as f64, 3000.), 0);
        }
        assert_eq!(map.at(150. * 128., 3000.), 1);
        assert_eq!(map.at(-0.1, 3000.), 0);
        assert_eq!(map.at(32768., 3000.), 0);
    }
    #[test]
    fn grass_blade_mode_builds_no_legacy_patches_on_a_moving_camera() {
        let world = tore_world::test_support::terrain();
        let atlas = std::sync::Arc::new(Atlas::synthetic(vec![12; 65536], 0));
        let mut placement = super::super::Placement::new(atlas);
        for eye in [
            [3000., 3000.],
            [5000., 8000.],
            [-2000., 1000.],
            [3000., 3000.],
        ] {
            let (patches, done) = placement.grass(&world, eye, 0., Some(2));
            assert!(patches.is_empty() && done);
            assert_eq!(placement.grass_builds, 0);
            assert_eq!(placement.build_ns, 0);
        }
    }
    #[test]
    fn grass_airport_mask_rejects_the_entire_rotated_footprint() {
        let mut world = tore_world::test_support::terrain();
        let surface = tore_sim::airport::OrientedBox {
            center: [4096., 0., 4096.],
            half: [512., 1., 1400.],
            heading: 0.7,
            pitch: 0.,
            bank: 0.,
        };
        world.airport_scene.runways.push(tore_sim::airport::Runway {
            object: 1,
            airport: 1,
            name: "synthetic".into(),
            surface,
            approach_center: surface.center,
            elevation_ft: 0.,
            heading: surface.heading,
            length_ft: 2800.,
        });
        let map = Map::build(
            &Atlas::synthetic(vec![12; 65536], 0),
            &world,
            &BTreeMap::new(),
            None,
        );
        for x in (2000..6200).step_by(61) {
            for z in (2000..6200).step_by(61) {
                if surface.contains_horizontal(x as f64, z as f64) {
                    assert_eq!(map.at(x as f64, z as f64), 0);
                }
            }
        }
        assert_eq!(map.at(7000., 7000.), 1);
    }
    #[test]
    fn grass_town_boundary_conservatively_clears_intersected_texels() {
        let world = tore_world::test_support::terrain();
        let mut map = Map::build(
            &Atlas::synthetic(vec![12; 65536], 0),
            &world,
            &BTreeMap::new(),
            None,
        );
        map.exclude_circle([4096., 4096.], 1000.);
        for angle in 0..360 {
            let a = (angle as f64).to_radians();
            assert_eq!(map.at(4096. + a.sin() * 999., 4096. + a.cos() * 999.), 0);
        }
        assert_eq!(map.at(6000., 6000.), 1);
    }
    #[test]
    fn grass_ground_grid_matches_the_terrain_triangles() {
        let world = tore_world::test_support::terrain();
        let map = Map::build(
            &Atlas::synthetic(vec![12; 65536], 0),
            &world,
            &BTreeMap::new(),
            None,
        );
        for (u, v) in [(0., 0.), (0.2, 0.3), (0.7, 0.6), (0.5, 0.5)] {
            let h = |x, y| map.terrain[(y * map.terrain_size[0] as usize + x) * 4];
            let y = if u + v <= 1. {
                h(0, 0) + (h(1, 0) - h(0, 0)) * u + (h(0, 1) - h(0, 0)) * v
            } else {
                h(1, 1) + (h(0, 1) - h(1, 1)) * (1. - u) + (h(1, 0) - h(1, 1)) * (1. - v)
            };
            assert!(
                (y + root_lift() - world.height(u * CELL as f32, v * CELL as f32)).abs() < 0.001
            );
        }
    }
    #[test]
    fn grass_colour_uses_land_fallback_without_filling_water() {
        let mut world = tore_world::test_support::terrain();
        let atlas = Atlas::synthetic(vec![12; 65536], 0);
        let build = |world: &Terrain, layers: &BTreeMap<(i32, i32), usize>| {
            Map::build(&atlas, world, layers, Some(7))
        };
        assert_eq!(build(&world, &BTreeMap::new()).terrain[1], 7.);
        world.theater.cells[0].color = 255;
        assert_eq!(build(&world, &BTreeMap::new()).terrain[1], -1.);
        assert_eq!(build(&world, &BTreeMap::from([((0, 0), 3)])).terrain[1], 3.);
    }
}
