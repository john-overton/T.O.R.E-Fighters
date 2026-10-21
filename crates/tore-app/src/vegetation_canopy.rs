//! Runtime, palette-indexed forest coverage. Generated from the very same
//! site exclusions, tree probabilities, species choices and supplied/retail art
//! used by placement. Cached locally, never part of game state.
use super::{Atlas, Species, WATER_CELL, hash, lin};
use crate::{terrain::Terrain, vegetation_shapes::Library};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Map {
    /// World x/z origin and dimensions in 512 ft samples.
    pub area: [f32; 4],
    pub first: usize,
    pub pages_x: usize,
}
impl Map {
    pub fn build(
        atlas: &Atlas,
        world: &Terrain,
        library: &Library,
        palette: &[[u8; 3]; 256],
        pages: &mut Vec<u8>,
        first: usize,
    ) -> Self {
        #[cfg(test)]
        if std::env::var_os("TORE_VEG_CANOPY_COMPARE").is_some() {
            let mut serial = Vec::new();
            let started = std::time::Instant::now();
            let expected = Self::build_with(
                atlas,
                world,
                library,
                palette,
                &mut serial,
                first,
                &tore_workers::Executor::serial(),
            );
            crate::loading::measured("vegetation canopy serial reference", started.elapsed());
            let mut parallel = Vec::new();
            let started = std::time::Instant::now();
            let actual = Self::build_with(
                atlas,
                world,
                library,
                palette,
                &mut parallel,
                first,
                &tore_workers::bulk(),
            );
            crate::loading::measured("vegetation canopy parallel", started.elapsed());
            assert_eq!(expected, actual);
            assert_eq!(serial, parallel);
            pages.extend(parallel);
            return actual;
        }
        Self::build_with(
            atlas,
            world,
            library,
            palette,
            pages,
            first,
            &tore_workers::bulk(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build_with(
        atlas: &Atlas,
        world: &Terrain,
        library: &Library,
        palette: &[[u8; 3]; 256],
        pages: &mut Vec<u8>,
        first: usize,
        workers: &tore_workers::Executor,
    ) -> Self {
        let w = &atlas.water;
        // Keep John's desert shading intact. Only woody non-desert land uses
        // the forest horizon treatment; the sparse desert objects still fade.
        if w.cols == 0 || w.rows == 0 || atlas.region.climate == 1 {
            return Self::default();
        }
        let nx = w.cols.div_ceil(128);
        let ny = w.rows.div_ceil(256);
        let pal = palette.map(lin);
        // Independent pages keep all floating point operations in their original
        // site order. Palette memoization is local, with deterministic tie breaks.
        let tiles: Vec<_> = (0..nx * ny).collect();
        let built = workers.ordered_map(&tiles, 2, |_, &tile| {
            let mut page = vec![0; 65536];
            let mut colors = BTreeMap::<[u8; 3], u8>::new();
            for z in tile / nx * 256..((tile / nx + 1) * 256).min(w.rows) {
                for x in tile % nx * 128..((tile % nx + 1) * 128).min(w.cols) {
                    let mut cover = 0.;
                    let mut color = [0.; 3];
                    // Four sites retain narrow field/coast boundaries within a cell.
                    for (dx, dz) in [(128, 128), (384, 128), (128, 384), (384, 384)] {
                        let wx = w.x0 + x as i64 * WATER_CELL + dx;
                        let wz = w.z0 + z as i64 * WATER_CELL + dz;
                        let site = atlas.site(world, wx as f64, wz as f64);
                        let p = atlas.tree_density(&site);
                        if p <= 0. {
                            continue;
                        }
                        let roll = hash(wx as i32, wz as i32, 93);
                        let tree = atlas.tree(&site, roll);
                        if tree.species == Species::Dead {
                            continue;
                        }
                        let mesh = &library.meshes
                            [usize::from(library.mesh_for(tree.species, (roll >> 24) as u8))];
                        // Expected projected crown area per 42 ft candidate cell.
                        let crown = std::f32::consts::PI
                            * (tree.height * tree.width * mesh.radius * 0.85).powi(2)
                            / (42. * 42.);
                        let weight = p * crown.min(4.);
                        cover += weight;
                        for (k, c) in color.iter_mut().enumerate() {
                            *c += mesh.mean_color[k] * tree.tint[k] * weight;
                        }
                    }
                    let key = if cover > 0. {
                        std::array::from_fn(|k| {
                            ((color[k] / cover).clamp(0., 1.) * 255.).round() as u8
                        })
                    } else {
                        [0; 3]
                    };
                    let index = *colors.entry(key).or_insert_with(|| {
                        (0..255)
                            .min_by(|&a, &b| {
                                let distance = |i: usize| {
                                    (0..3)
                                        .map(|k| (pal[i][k] - f32::from(key[k]) / 255.).powi(2))
                                        .sum::<f32>()
                                };
                                distance(a).total_cmp(&distance(b))
                            })
                            .unwrap_or(0) as u8
                    });
                    let at = (z % 256) * 256 + x % 128 * 2;
                    // Expected crown coverage encoded 0..4, then one actual palette index.
                    page[at] = (cover * 0.25 * 63.75).clamp(0., 255.).round() as u8;
                    page[at + 1] = index;
                }
            }
            page
        });
        for page in built {
            pages.extend(page);
        }
        Self {
            area: [w.x0 as f32, w.z0 as f32, w.cols as f32, w.rows as f32],
            first,
            pages_x: nx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parallel_canopy_is_byte_identical_to_serial() {
        let world = tore_world::test_support::terrain();
        let mut atlas = Atlas::synthetic(vec![10; 65536], 0);
        atlas.water.cols = 129;
        atlas.water.rows = 257;
        atlas.water.dist.resize(129 * 257, 255);
        let palette = [[80; 3]; 256];
        let mut art = Vec::new();
        let mut lib = Library::build(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &palette,
            &mut art,
            |p| p.len() / 65536,
        );
        lib.add_pack("pack", super::super::region("UKR"), &palette, &mut art, 0);
        let mut serial = Vec::new();
        let mut parallel = Vec::new();
        let a = Map::build_with(
            &atlas,
            &world,
            &lib,
            &palette,
            &mut serial,
            7,
            &tore_workers::Executor::serial(),
        );
        let b = Map::build_with(
            &atlas,
            &world,
            &lib,
            &palette,
            &mut parallel,
            7,
            &tore_workers::Executor::parallel(4).unwrap(),
        );
        assert_eq!(a, b);
        assert_eq!(serial, parallel);
        assert_eq!(serial.len(), 4 * 65536);
    }
    #[test]
    fn canopy_uses_sprite_colour_and_the_same_forest_field_and_water_rules() {
        let mut world = tore_world::test_support::terrain();
        for c in &mut world.theater.cells {
            c.elevation = 0;
        }
        let palette = [[80; 3]; 256];
        let mut art = Vec::new();
        let mut lib = Library::build(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &palette,
            &mut art,
            |p| p.len() / 65536,
        );
        lib.add_pack("pack", super::super::region("UKR"), &palette, &mut art, 0);
        for (index, covered) in [(10, true), (20, false), (255, false)] {
            let atlas = Atlas::synthetic(vec![index; 65536], 0);
            let mut pages = Vec::new();
            let map = Map::build(&atlas, &world, &lib, &palette, &mut pages, 7);
            assert_eq!(map.first, 7);
            let at = 32 * 256 + 32 * 2;
            assert_eq!(pages[at] > 0, covered);
            if covered {
                assert_eq!(palette[usize::from(pages[at + 1])], [80; 3]);
            }
        }
        let desert = Atlas::synthetic(vec![20; 65536], 1);
        let mut pages = Vec::new();
        let map = Map::build(&desert, &world, &lib, &palette, &mut pages, 7);
        assert_eq!(map.pages_x, 0);
        assert!(pages.is_empty(), "desert ground stays unchanged");
    }
}
