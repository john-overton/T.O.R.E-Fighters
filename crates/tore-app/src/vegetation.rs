//! Theater vegetation (exploratory, 2026-10-16; sparse cached sprite placement). Presentation only: nothing here is collidable, and nothing
//! is written to the simulation, the wire, a replay or a checkpoint.
//! `TORE_VEGETATION=0` builds none of it. Unset or `1` builds it. Placement
//! is a function of the theater, the tile and the world position, so every
//! machine and every replay of a view shows the same scene.
//!
//! Where things grow is read from the retail tile art, per texel, with the
//! terrain's slope and height (fitted rules, agent decisions, recorded in
//! `docs/spec/terrain-detail.md`):
//!
//! - each theater's own greens are split by their tone: its darker greens
//!   are woodland, its lighter yellow-greens are fields, the rest meadow;
//! - woodland has tree patches, meadow carries copses, fields stay clear except for
//!   hedgerows along plot edges;
//! - hills and mountains gain trees up to a climate tree line, then thin;
//! - river banks and shores are denser; roads, towns, airfields and water
//!   stay clear;
//! - deserts have sparse dry scrub, green belts near water, and rock on stony ground
//!   and slopes; the dunes themselves are terrain shading
//!   (`terrain.wgsl`), which keeps them in the ground's own light.
//!
//! Full candidates are cached per world-anchored chunk, independent of distance.
//! Each object carries a rank for distance selection and fading on the GPU.

use std::{collections::BTreeMap, sync::Arc};
use tore_formats::theater::Environment;
use tore_world::{
    city::mask::{self, GREEN, LightMap, SITE, STREET, WATER},
    resources::ResourceSource,
    terrain::Terrain,
};

#[path = "vegetation_canopy.rs"]
pub mod canopy;
/// Feet in a nautical mile, matching the city fill.
#[path = "vegetation_coast.rs"]
mod coast;
#[path = "vegetation_grass.rs"]
pub(crate) mod grass;

pub const NM: f64 = 6_076.115_49;
const CELL: i64 = mask::TILE_FT / 4;
const TEXEL: i64 = mask::TEXEL_FT;
const TEXELS: usize = 256;
/// Placement chunk, feet. Objects and grass are cached per chunk.
pub const CHUNK: i64 = 2_048;
const SIDE: usize = (CHUNK / TEXEL) as usize;
const GRID: usize = SIDE + 2;
/// Grass patch, feet.
pub const PATCH: i64 = 32;
/// Water distance grid cell, feet.
const WATER_CELL: i64 = 512;

#[path = "vegetation_cache.rs"]
pub(crate) mod cache;

type Art = Arc<BTreeMap<String, Vec<u8>>>;

static ART: std::sync::RwLock<Option<Art>> = std::sync::RwLock::new(None);

/// The retail vegetation shapes and textures of the loaded import
/// (`tore_import::selection::VEGETATION_ART`), set each time an import is
/// decoded. Empty for an import made before they were kept.
pub fn set_art(art: BTreeMap<String, Vec<u8>>) {
    if let Ok(mut slot) = ART.write() {
        *slot = Some(Arc::new(art));
    }
}

pub fn art() -> Arc<BTreeMap<String, Vec<u8>>> {
    ART.read()
        .ok()
        .and_then(|slot| slot.clone())
        .unwrap_or_default()
}

/// Developer switch. Unset and `1` are on. `0` is off.
pub fn enabled() -> crate::AppResult<bool> {
    match std::env::var("TORE_VEGETATION").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("1") => Ok(true),
        Ok("0") => Ok(false),
        _ => Err("TORE_VEGETATION must be 0 or 1".into()),
    }
}

/// What a texel is, for detail texture and for what may stand on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Water = 0,
    Road = 2,
    Field = 4,
    Grass = 5,
    Forest = 6,
    Sand = 7,
    Rock = 8,
    Snow = 9,
    Scrub = 10,
    River = 11,
}
impl Kind {
    /// Something may stand here.
    fn open(self) -> bool {
        matches!(
            self,
            Self::Field
                | Self::Grass
                | Self::Forest
                | Self::Sand
                | Self::Rock
                | Self::Snow
                | Self::Scrub
        )
    }
    fn wet(self) -> bool {
        matches!(self, Self::Water | Self::River)
    }
}

/// Shader climate: 0 temperate, 1 desert, 2 arctic, 3 tropical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub code: &'static str,
    pub name: &'static str,
    pub climate: u8,
    /// 0 broadleaf, 1 conifer, 2 palm.
    pub trees: u8,
    /// Extra placement scale for this theater.
    pub cover: f32,
}

const fn r(code: &'static str, name: &'static str, climate: u8, trees: u8, cover: f32) -> Region {
    Region {
        code,
        name,
        climate,
        trees,
        cover,
    }
}

/// Fitted climates (2026-10-16), from each theater's region. The retail
/// files have no climate field. Humidity in `tore_world::climate` is the
/// same grouping: arid theaters are desert, Kurils arctic, Vietnam, Cuba
/// and Panama tropical, the rest temperate. Tree mix and cover are agent
/// choices on top of that.
pub const REGIONS: [Region; 16] = [
    r("UKR", "Ukraine", 0, 0, 1.0),
    r("FRA", "France", 0, 0, 1.0),
    r("BAL", "Baltics", 0, 1, 0.95),
    r("GRE", "Greece", 0, 0, 0.75),
    r("NSK", "South Korea", 0, 1, 1.0),
    r("VLA", "Vladivostok", 0, 1, 1.05),
    r("LFA", "Falklands", 0, 1, 0.3),
    r("EGY", "Egypt", 1, 0, 1.0),
    r("IRA", "Iraq", 1, 0, 1.0),
    r("PGU", "Persian Gulf", 1, 0, 0.9),
    r("SPA", "Pakistan", 1, 0, 1.0),
    r("KURILE", "Kuril Islands", 2, 1, 0.5),
    r("TVIET", "North Vietnam", 3, 2, 1.2),
    r("CUB", "Cuba", 3, 2, 1.1),
    r("APA", "Panama", 3, 2, 1.2),
    r("WTA", "Taiwan", 3, 0, 1.05),
];

pub fn region(code: &str) -> Region {
    let base = tore_formats::theater::base_theater(code).unwrap_or(code);
    REGIONS
        .into_iter()
        .find(|r| r.code == base)
        .unwrap_or(r("UKR", "Temperate", 0, 0, 1.0))
}

/// Climate id for the ground-detail shader.
pub fn climate_id(layout: &str) -> u8 {
    region(layout).climate
}

fn luma(c: [u8; 3]) -> i32 {
    (i32::from(c[0]) * 299 + i32::from(c[1]) * 587 + i32::from(c[2]) * 114) / 1000
}

/// A theater's own green tones: greens at or below `dark` luma are its
/// woodland, greens at or above `light` luma that are at least `yellow`
/// red to green are its fields. Measured per theater from its placed tiles
/// (fitted: the 28th and 70th luma percentiles of its greens).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    pub dark: i32,
    pub light: i32,
    pub yellow: f32,
}
impl Tone {
    /// The absolute thresholds, for a theater with too few greens to measure.
    pub const FIXED: Tone = Tone {
        dark: 55,
        light: 155,
        yellow: 0.92,
    };
}

fn is_green(rgb: [u8; 3]) -> bool {
    let [r, g, b] = rgb.map(i32::from);
    // A few points of blue still counts as green: Vladivostok's land is
    // greener than it is red, with blue only slightly above green.
    g >= r + 8 && g + 12 >= b
}

/// The kind a reference-palette colour is, with absolute thresholds.
#[cfg(test)]
pub fn kind_of(rgb: [u8; 3], index: u8, climate: u8) -> Kind {
    kind_toned(rgb, index, climate, Tone::FIXED)
}

/// The kind a reference-palette colour is in a theater of `tone`. `index`
/// 255 is the water cutout.
pub fn kind_toned(rgb: [u8; 3], index: u8, climate: u8, tone: Tone) -> Kind {
    if index == 255 {
        return Kind::Water;
    }
    let [r, g, b] = rgb.map(i32::from);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let sat = max - min;
    let l = luma(rgb);
    // A real blue, not the slight blue cast of a cold land palette
    // (Vladivostok's mean land is only a few points bluer than green).
    if b >= g + 15 && b >= r + 25 && l < 170 {
        return Kind::River;
    }
    if l >= 188 && sat <= 30 {
        return Kind::Snow;
    }
    if climate == 2 && l >= 150 && sat <= 40 && b + 10 >= g {
        return Kind::Snow;
    }
    let green = is_green(rgb);
    let dark_green = green && l <= tone.dark;
    let light_green = green && l >= tone.light && r as f32 >= g as f32 * tone.yellow;
    let yellow = g + 12 >= r && r >= b + 10 && (80..=205).contains(&l);
    let warm = r >= b + 20 && r + 18 >= g && g + 8 >= b && (70..=215).contains(&l);
    let grey = sat <= 24 && (40..=180).contains(&l);
    match climate {
        1 => {
            if green && l < 60 && g >= r + 22 {
                Kind::Forest
            } else if green && l < 140 {
                Kind::Scrub
            } else if grey && l < 100 {
                Kind::Rock
            } else if grey {
                Kind::Road
            } else {
                Kind::Sand
            }
        }
        2 => {
            if dark_green {
                Kind::Forest
            } else if green {
                Kind::Grass
            } else if grey && l < 130 {
                Kind::Rock
            } else {
                Kind::Snow
            }
        }
        3 => {
            if dark_green || (green && l < tone.light) {
                Kind::Forest
            } else if green || yellow {
                Kind::Grass
            } else if warm {
                Kind::Sand
            } else if grey {
                Kind::Road
            } else {
                Kind::Grass
            }
        }
        _ => {
            if dark_green {
                Kind::Forest
            } else if light_green {
                Kind::Field
            } else if green {
                Kind::Grass
            } else if yellow || (warm && sat < 55) {
                Kind::Field
            } else if grey && l < 105 {
                Kind::Rock
            } else if grey {
                Kind::Road
            } else {
                Kind::Field
            }
        }
    }
}

fn bare_kind(climate: u8) -> Kind {
    match climate {
        1 => Kind::Sand,
        2 => Kind::Snow,
        3 => Kind::Forest,
        _ => Kind::Grass,
    }
}

pub(crate) fn rotate(tu: usize, tv: usize, rotation: u8) -> (usize, usize) {
    match rotation % 4 {
        0 => (tu, 255 - tv),
        1 => (255 - tv, 255 - tu),
        2 => (255 - tu, tv),
        _ => (tv, tu),
    }
}

struct Picture {
    /// Palette index per texel, picture space, 256 square. 128-square art
    /// is nearest-expanded.
    indices: Vec<u8>,
}

/// Distance to water on a coarse grid.
struct WaterMap {
    x0: i64,
    z0: i64,
    cols: usize,
    rows: usize,
    /// Cells to the nearest water cell, capped at 255.
    dist: Vec<u8>,
}

impl WaterMap {
    fn empty() -> Self {
        Self {
            x0: 0,
            z0: 0,
            cols: 0,
            rows: 0,
            dist: Vec::new(),
        }
    }

    /// Feet to the nearest water or river, to the grid's resolution.
    fn feet(&self, x: f64, z: f64) -> f64 {
        let c = ((x as i64 - self.x0).div_euclid(WATER_CELL)) as isize;
        let r = ((z as i64 - self.z0).div_euclid(WATER_CELL)) as isize;
        if c < 0 || r < 0 || c as usize >= self.cols || r as usize >= self.rows {
            return f64::INFINITY;
        }
        let d = self.dist[r as usize * self.cols + c as usize];
        if d == 255 {
            f64::INFINITY
        } else {
            f64::from(d) * WATER_CELL as f64
        }
    }
}

/// Classified ground tiles for one layout.
pub struct Atlas {
    pub region: Region,
    pub tone: Tone,
    pub(crate) palette: [[u8; 3]; 256],
    pictures: Vec<Picture>,
    /// Placement key to (picture, rotation).
    placements: BTreeMap<(i32, i32), (u16, u8)>,
    water: WaterMap,
    coast: coast::Coast,
    pub seams: std::sync::Arc<crate::terrain_seams::Map>,
    /// The theater's median terrain height, feet: hills are measured from it.
    median_height: f32,
    /// Each terrain cell corner's palette index and the theater's columns:
    /// the colour the renderer shades ground with no picture placed.
    cells: Vec<u8>,
    cols: usize,
}

/// A procedural plot edge may border meadow, but never water or a road.
fn field_edge(a: Kind, b: Kind) -> bool {
    matches!(a, Kind::Field | Kind::Grass)
        && matches!(b, Kind::Field | Kind::Grass)
        && (a == Kind::Field || b == Kind::Field)
}

/// Fitted tree line by climate and tree mix: full cover below the first
/// height, none above the second (feet).
fn tree_line(region: Region) -> (f32, f32) {
    match (region.climate, region.trees) {
        (2, _) => (900., 2_400.),
        (3, _) => (7_500., 10_500.),
        (0, 1) => (3_800., 6_000.),
        _ => (4_500., 7_000.),
    }
}

impl Atlas {
    pub fn build(
        resources: &dyn ResourceSource,
        terrain: &Terrain,
    ) -> crate::AppResult<Option<Self>> {
        if !enabled()? {
            return Ok(None);
        }
        let mut atlas = Self::from_environment(resources, &terrain.environment, &terrain.layout);
        atlas.median_height = median_height(terrain);
        let theater = &terrain.theater;
        atlas.cols = theater.cols;
        atlas.cells = (0..theater.rows)
            .flat_map(|row| (0..theater.cols).map(move |col| theater.cell(col, row).color))
            .collect();
        atlas.coast = coast::Coast::build(&atlas);
        Ok(Some(atlas))
    }

    fn from_environment(
        resources: &dyn ResourceSource,
        environment: &Environment,
        layout: &str,
    ) -> Self {
        let region = region(layout);
        let palette = mask::reference_palette(resources, &environment.map).unwrap_or([[0; 3]; 256]);
        let base = tore_formats::theater::base_theater(layout).unwrap_or("");
        let mut pictures: BTreeMap<String, u16> = BTreeMap::new();
        let mut stored = Vec::new();
        let mut placements = BTreeMap::new();
        for (key, placement) in &environment.textures {
            let name = placement.resource_name(base);
            let index = if let Some(i) = pictures.get(&name) {
                *i
            } else {
                let Some(pic) = load_picture(resources, &name) else {
                    continue;
                };
                let i = stored.len() as u16;
                stored.push(pic);
                pictures.insert(name, i);
                i
            };
            placements.insert(*key, (index, placement.rotation));
        }
        Self::assemble(region, palette, stored, placements)
    }

    fn assemble(
        region: Region,
        palette: [[u8; 3]; 256],
        pictures: Vec<Picture>,
        placements: BTreeMap<(i32, i32), (u16, u8)>,
    ) -> Self {
        let mut atlas = Self {
            region,
            tone: Tone::FIXED,
            palette,
            pictures,
            placements,
            water: WaterMap::empty(),
            coast: Default::default(),
            seams: Default::default(),
            median_height: 0.,
            cells: Vec::new(),
            cols: 0,
        };
        atlas.tone = atlas.measure_tone();
        atlas.water = atlas.water_map();
        atlas
    }

    /// The theater's green tones, from every placed texel.
    fn measure_tone(&self) -> Tone {
        if self.region.climate == 1 {
            return Tone::FIXED;
        }
        let mut uses = vec![0u64; self.pictures.len()];
        for (pic, _) in self.placements.values() {
            uses[usize::from(*pic)] += 1;
        }
        let mut histogram = [0u64; 256];
        let mut ratios = [0u64; 256];
        for (pic, n) in self.pictures.iter().zip(&uses) {
            if *n == 0 {
                continue;
            }
            let mut counts = [0u64; 256];
            for &i in &pic.indices {
                counts[usize::from(i)] += 1;
            }
            for (index, count) in counts.iter().enumerate() {
                let rgb = self.palette[index];
                if index == 255 || *count == 0 || !is_green(rgb) {
                    continue;
                }
                let l = luma(rgb);
                if l >= 188 {
                    continue;
                }
                histogram[l as usize] += count * n;
                let ratio = f32::from(rgb[0]) / f32::from(rgb[1]).max(1.);
                ratios[((ratio * 200.) as usize).min(255)] += count * n;
            }
        }
        let total: u64 = histogram.iter().sum();
        if total < 10_000 {
            return Tone::FIXED;
        }
        let percentile = |h: &[u64; 256], share: f64| {
            let goal = (total as f64 * share) as u64;
            let mut sum = 0;
            for (i, c) in h.iter().enumerate() {
                sum += c;
                if sum >= goal {
                    return i;
                }
            }
            255
        };
        Tone {
            dark: percentile(&histogram, 0.28) as i32,
            light: (percentile(&histogram, 0.70) as i32)
                .max(percentile(&histogram, 0.28) as i32 + 4),
            yellow: percentile(&ratios, 0.70) as f32 / 200.,
        }
    }

    /// Distance to water, 512 ft cells, over the placed tiles.
    fn water_map(&self) -> WaterMap {
        let Some(((kx0, kz0), (kx1, kz1))) =
            self.placements
                .keys()
                .fold(None, |acc: Option<((i32, i32), (i32, i32))>, &(x, z)| {
                    Some(match acc {
                        None => ((x, z), (x, z)),
                        Some(((a, b), (c, d))) => ((a.min(x), b.min(z)), (c.max(x), d.max(z))),
                    })
                })
        else {
            return WaterMap::empty();
        };
        // Keys step by four cells: one picture covers a whole tile.
        let per = (mask::TILE_FT / WATER_CELL) as usize;
        let block = TEXELS / per;
        let cols = ((kx1 - kx0) / 4 + 1) as usize * per;
        let rows = ((kz1 - kz0) / 4 + 1) as usize * per;
        // Each picture's wet blocks, picture space: at least one eighth of the block
        // wet, to retain narrower river branches; town exclusions still apply.
        let wet: Vec<Vec<bool>> = self
            .pictures
            .iter()
            .map(|pic| {
                let mut count = vec![0usize; per * per];
                for (i, &index) in pic.indices.iter().enumerate() {
                    if self.kind_index(index).wet() {
                        count[(i / TEXELS / block) * per + (i % TEXELS) / block] += 1;
                    }
                }
                count.iter().map(|&n| n * 8 >= block * block).collect()
            })
            .collect();
        let mut dist = vec![255u8; cols * rows];
        for (&(kx, kz), &(pic, rotation)) in &self.placements {
            for bv in 0..per {
                for bu in 0..per {
                    let (c, r) = rotate(bu * block, bv * block, rotation);
                    if wet[usize::from(pic)][(r / block) * per + c / block] {
                        let x = ((kx - kx0) / 4) as usize * per + bu;
                        let z = ((kz - kz0) / 4) as usize * per + bv;
                        dist[z * cols + x] = 0;
                    }
                }
            }
        }
        // Two chamfer passes.
        for z in 0..rows {
            for x in 0..cols {
                let mut d = dist[z * cols + x];
                if x > 0 {
                    d = d.min(dist[z * cols + x - 1].saturating_add(1));
                }
                if z > 0 {
                    d = d.min(dist[(z - 1) * cols + x].saturating_add(1));
                }
                dist[z * cols + x] = d;
            }
        }
        for z in (0..rows).rev() {
            for x in (0..cols).rev() {
                let mut d = dist[z * cols + x];
                if x + 1 < cols {
                    d = d.min(dist[z * cols + x + 1].saturating_add(1));
                }
                if z + 1 < rows {
                    d = d.min(dist[(z + 1) * cols + x].saturating_add(1));
                }
                dist[z * cols + x] = d;
            }
        }
        WaterMap {
            x0: i64::from(kx0) * CELL,
            z0: i64::from(kz0) * CELL,
            cols,
            rows,
            dist,
        }
    }

    /// A one-tile atlas for tests. `indices` is 256 square, picture space.
    #[cfg(test)]
    pub fn synthetic(indices: Vec<u8>, climate: u8) -> Self {
        let mut p = [[0; 3]; 256];
        // Forest green, a meadow green, a field tan, a road grey.
        p[10] = [20, 70, 24];
        p[12] = [80, 140, 60];
        p[20] = [180, 160, 90];
        p[40] = [150, 148, 146];
        p[255] = [0, 0, 80];
        let mut atlas = Self::assemble(
            r("TEST", "Test", climate, 0, 1.0),
            p,
            vec![Picture { indices }],
            BTreeMap::from([((0, 0), (0, 0))]),
        );
        atlas.tone = Tone::FIXED;
        atlas.coast = coast::Coast::build(&atlas);
        atlas
    }

    fn index_at(&self, x: i64, z: i64) -> Option<u8> {
        let key = LightMap::key(x, z);
        let Some((pic, rotation)) = self.placements.get(&key) else {
            return self.cell_index(x, z);
        };
        let tu = ((x - i64::from(key.0) * CELL) / TEXEL).clamp(0, 255) as usize;
        let tv = ((z - i64::from(key.1) * CELL) / TEXEL).clamp(0, 255) as usize;
        let (col, row) = rotate(tu, tv, *rotation);
        Some(self.pictures[*pic as usize].indices[row * TEXELS + col])
    }

    /// The nearest cell corner's colour, for ground with no picture.
    fn cell_index(&self, x: i64, z: i64) -> Option<u8> {
        if self.cols == 0 {
            return None;
        }
        let rows = self.cells.len() / self.cols;
        let col = ((x + CELL / 2) / CELL).clamp(0, self.cols as i64 - 1) as usize;
        let row = ((z + CELL / 2) / CELL).clamp(0, rows as i64 - 1) as usize;
        Some(self.cells[row * self.cols + col])
    }

    pub(crate) fn kind_index(&self, index: u8) -> Kind {
        kind_toned(
            self.palette[usize::from(index)],
            index,
            self.region.climate,
            self.tone,
        )
    }

    pub(crate) fn coast_blocked(&self, x: i64, z: i64) -> bool {
        self.coast.contains(x, z)
    }

    pub fn kind_at(&self, x: i64, z: i64) -> Kind {
        match self.index_at(x, z) {
            Some(index) => self.kind_index(index),
            None => bare_kind(self.region.climate),
        }
    }

    /// A reproducible beach/green boundary camera, independent of object art.
    pub fn coast_spot(&self) -> Option<Spot> {
        for &(kx, kz) in self.placements.keys() {
            for v in (0..256).step_by(8) {
                for u in (0..256).step_by(8) {
                    let x = i64::from(kx) * CELL + u * TEXEL;
                    let z = i64::from(kz) * CELL + v * TEXEL;
                    if !self.coast.contains(x, z) {
                        continue;
                    }
                    for (dx, dz) in [(1024, 0), (-1024, 0), (0, 1024), (0, -1024)] {
                        if !self.coast.contains(x + dx, z + dz)
                            && matches!(self.kind_at(x + dx, z + dz), Kind::Grass | Kind::Forest)
                        {
                            return Some(Spot {
                                x: x as f64,
                                z: z as f64,
                                heading: (-(dx as f64)).atan2(-(dz as f64)).to_degrees(),
                                label: "coast",
                            });
                        }
                    }
                }
            }
        }
        None
    }

    pub fn color_at(&self, x: i64, z: i64) -> [u8; 3] {
        match self.index_at(x, z) {
            Some(index) if index != 255 => self.palette[usize::from(index)],
            _ => match self.region.climate {
                1 => [186, 156, 96],
                2 => [210, 214, 216],
                3 => [36, 92, 40],
                _ => [78, 120, 52],
            },
        }
    }

    fn blocked(&self, world: &Terrain, x: i64, z: i64, kind: Kind) -> bool {
        if !kind.open() || self.coast.contains(x, z) {
            return true;
        }
        if paved(world, x as f64, z as f64) {
            return true;
        }
        if let Some(fill) = &world.city_fill {
            let flags = fill.layout.map.flags_at(x, z);
            if flags & WATER != 0 || (flags & (SITE | STREET) != 0 && flags & GREEN == 0) {
                return true;
            }
            if fill.layout.map.districts.iter().any(|d| {
                let r = d.half[0].max(d.half[1]).max(1);
                let dx = x - d.center[0];
                let dz = z - d.center[1];
                dx * dx + dz * dz <= r * r
            }) {
                return true;
            }
        }
        false
    }

    /// One texel's ground, looked up directly (the overlay and the preview
    /// spots); chunks read the same through a [`Table`].
    pub fn site(&self, world: &Terrain, x: f64, z: f64) -> Site {
        let xi = x.floor() as i64;
        let zi = z.floor() as i64;
        let kind = self.kind_at(xi, zi);
        let mut dark = 0;
        let mut field = 0;
        for dz in [-TEXEL, 0, TEXEL] {
            for dx in [-TEXEL, 0, TEXEL] {
                match self.kind_at(xi + dx, zi + dz) {
                    Kind::Forest => dark += 1,
                    Kind::Field => field += 1,
                    _ => {}
                }
            }
        }
        self.finish_site(
            world,
            x,
            z,
            Texel {
                kind,
                blocked: self.blocked(world, xi, zi, kind),
                color: self.color_at(xi, zi),
            },
            dark as f32 / 9.,
            field as f32 / 9.,
        )
    }

    fn finish_site(
        &self,
        world: &Terrain,
        x: f64,
        z: f64,
        t: Texel,
        wood: f32,
        field: f32,
    ) -> Site {
        let h = |dx: f64, dz: f64| world.height((x + dx) as f32, (z + dz) as f32);
        let height = h(0., 0.);
        let grad = [
            (h(64., 0.) - h(-64., 0.)) / 128.,
            (h(0., 64.) - h(0., -64.)) / 128.,
        ];
        let slope = grad[0].hypot(grad[1]).atan().to_degrees();
        let water = self.water.feet(x, z);
        let near_water = 1. - smoothstep(300., 2_600., water as f32);
        let w = [x as f32, z as f32];
        let (sea, bed, bank, gravel) = if self.region.climate == 1 {
            let sand = if t.kind == Kind::Sand { 1. } else { 0. };
            let bed = wadi(w, 1.0);
            let bank = (wadi(w, 2.6) - bed).max(0.);
            let stony = smoothstep(4., 10., slope).max(if t.kind == Kind::Rock { 0.8 } else { 0. });
            let sea = sand_sea(w) * sand * (1. - stony);
            let gravel =
                (1. - sea) * sand * smoothstep(0.35, 0.7, detail_noise(div(w, 7_000.), 0, 20));
            (sea, bed * sand, bank * sand, gravel)
        } else {
            (0., 0., 0., 0.)
        };
        Site {
            x,
            z,
            kind: t.kind,
            blocked: t.blocked,
            color: t.color,
            wood,
            field,
            height,
            grad,
            slope,
            near_water,
            sea,
            bed,
            bank,
            gravel,
        }
    }

    /// The chunk's texel table, with a one texel border.
    fn table(&self, world: &Terrain, cx: i64, cz: i64) -> Table {
        let mut t = Table {
            texels: [Texel {
                kind: Kind::Water,
                blocked: true,
                color: [0; 3],
            }; GRID * GRID],
        };
        let x0 = cx * CHUNK - TEXEL;
        let z0 = cz * CHUNK - TEXEL;
        for row in 0..GRID {
            for col in 0..GRID {
                let x = x0 + col as i64 * TEXEL + TEXEL / 2;
                let z = z0 + row as i64 * TEXEL + TEXEL / 2;
                let kind = self.kind_at(x, z);
                t.texels[row * GRID + col] = Texel {
                    kind,
                    blocked: self.blocked(world, x, z, kind),
                    color: self.color_at(x, z),
                };
            }
        }
        t
    }

    fn table_site(&self, world: &Terrain, table: &Table, cx: i64, cz: i64, x: f64, z: f64) -> Site {
        let col = (((x as i64) - cx * CHUNK).div_euclid(TEXEL) + 1).clamp(1, SIDE as i64) as usize;
        let row = (((z as i64) - cz * CHUNK).div_euclid(TEXEL) + 1).clamp(1, SIDE as i64) as usize;
        let mut dark = 0;
        let mut field = 0;
        for r in row - 1..=row + 1 {
            for c in col - 1..=col + 1 {
                match table.texels[r * GRID + c].kind {
                    Kind::Forest => dark += 1,
                    Kind::Field => field += 1,
                    _ => {}
                }
            }
        }
        self.finish_site(
            world,
            x,
            z,
            table.texels[row * GRID + col],
            dark as f32 / 9.,
            field as f32 / 9.,
        )
    }

    /// Trees per candidate (one candidate per 900 square feet), 0..1.
    pub fn tree_density(&self, s: &Site) -> f32 {
        if s.blocked {
            return 0.;
        }
        let region = self.region;
        if region.climate == 1 {
            let green = matches!(s.kind, Kind::Scrub | Kind::Forest | Kind::Grass);
            let belt = if green {
                0.80 * s.near_water.powi(2)
            } else {
                0.
            };
            let oasis = 0.72 * s.near_water.powi(2) * if s.kind == Kind::Sand { 1. } else { 0. };
            let wadi = 0.10 * s.bank + 0.025 * s.bed;
            return (belt + oasis + wadi).min(1.) * region.cover;
        }
        let (full, none) = tree_line(region);
        let line = 1. - smoothstep(full, none, s.height);
        let hill = smoothstep(3., 13., s.slope);
        let relief = smoothstep(400., 2_500., s.height - self.median_height);
        let mut p = match s.kind {
            Kind::Forest => 0.70 + 0.25 * s.wood,
            Kind::Grass => {
                0.045 * s.wood.powf(0.7)
                    + 0.001
                    + 0.18
                        * smoothstep(
                            0.66,
                            0.8,
                            detail_noise(div([s.x as f32, s.z as f32], 1_400.), 0, 70),
                        )
            }
            Kind::Scrub => 0.12,
            Kind::Rock => 0.04,
            Kind::Snow => 0.01,
            Kind::Sand => 0.02,
            _ => 0.,
        };
        if s.kind == Kind::Field {
            // Fields stay clear; hedgerows are placed along plot edges, and
            // trees gather along a river through the fields.
            p = 0.03 * s.near_water;
        } else {
            p += (0.48 * hill + 0.18 * relief) * (1. - p);
            p += 0.08 * s.near_water * (1. - p);
        }
        // Field/forest tile boundaries get a 500 ft belt. Interior fields
        // stay clear; the existing grounding, tree line and slope gates apply.
        // The seam mask classifies the rendered LAND fallback too, whereas
        // an unplaced site's existing ecology can still come from T2 colour.
        p = p.max(0.85 * self.seams.hedge(s.x, s.z));
        // Tropical forest is closer and taller; arctic trees are scattered.
        let climate = match region.climate {
            3 => 1.15,
            2 => 0.45,
            _ => 1.,
        };
        (p * line * (1. - smoothstep(32., 48., s.slope)) * climate * region.cover).clamp(0., 1.)
    }

    /// Bushes per candidate (one per 480 square feet), 0..1.
    fn bush_density(&self, s: &Site) -> f32 {
        if s.blocked {
            return 0.;
        }
        if self.region.climate == 1 {
            let green = matches!(s.kind, Kind::Scrub | Kind::Forest | Kind::Grass);
            return (0.012
                + 0.10 * s.bank
                + 0.04 * s.bed
                + 0.025 * s.gravel
                + if green {
                    0.55 * s.near_water.powi(2)
                } else {
                    0.50 * s.near_water.powi(2)
                })
            .min(1.);
        }
        let edge = 4. * s.wood * (1. - s.wood);
        let p = match s.kind {
            Kind::Forest => 0.06 + 0.12 * edge,
            Kind::Grass => 0.035 + 0.15 * edge,
            Kind::Scrub | Kind::Rock => 0.12,
            Kind::Field => 0.,
            Kind::Snow => 0.02,
            _ => 0.02,
        };
        let (full, none) = tree_line(self.region);
        let alpine = smoothstep(full - 500., full + 500., s.height);
        ((p + 0.25 * alpine + 0.25 * s.near_water * if s.kind == Kind::Field { 0.3 } else { 1. })
            * (1. - smoothstep(none, none + 700., s.height))
            * (1. - smoothstep(32., 48., s.slope)))
        .min(1.)
    }

    /// Stones and boulders per candidate (one per 680 square feet), 0..1.
    fn rock_density(&self, s: &Site) -> f32 {
        if s.blocked {
            return 0.;
        }
        let steep = smoothstep(6., 18., s.slope);
        if self.region.climate == 1 {
            let stony = steep.max(if s.kind == Kind::Rock { 0.8 } else { 0. });
            return (0.5 * stony + 0.22 * s.gravel + 0.22 * s.bed + 0.1 * s.bank) * (1. - s.sea);
        }
        let p = match s.kind {
            Kind::Rock => 0.3,
            Kind::Snow => 0.08,
            Kind::Field => 0.,
            _ => 0.015,
        };
        (p + 0.12 * steep).min(1.)
    }

    /// The objects of one chunk drawn from `near` feet outwards, full
    /// density reaching `full` feet (0 and anything: every object).
    fn chunk(
        &self,
        world: &Terrain,
        cx: i64,
        cz: i64,
        near: f32,
        full: f32,
        density: f32,
    ) -> Vec<Inst> {
        let table = self.table(world, cx, cz);
        let mut out = Vec::new();
        let x0 = (cx * CHUNK) as f64;
        let z0 = (cz * CHUNK) as f64;
        let climate = self.region.climate;
        let share = share(near, full * Species::MAX_REACH);
        let kept = |species: Species| species.share(near, full);
        // Ultra adds candidates; lower settings thin them.
        let grid = |base: f64| base / f64::from(density.max(0.1)).sqrt();
        let scatter =
            |step: f64, salt: u32, out: &mut Vec<Inst>, f: &dyn Fn(&Site, u32) -> Option<Plant>| {
                let n = (CHUNK as f64 / step).floor().max(1.) as i64;
                let step = CHUNK as f64 / n as f64;
                for j in 0..n {
                    for i in 0..n {
                        let gx = (cx * n + i) as i32;
                        let gz = (cz * n + j) as i32;
                        let roll = hash(gx, gz, salt);
                        let u = unit(roll);
                        if u >= share {
                            continue;
                        }
                        let jx = if salt == 350 {
                            0.5
                        } else {
                            unit(hash(gx, gz, salt + 1))
                        };
                        let jz = if salt == 350 {
                            0.5
                        } else {
                            unit(hash(gx, gz, salt + 2))
                        };
                        let x = x0 + (i as f64 + jx as f64) * step;
                        let z = z0 + (j as f64 + jz as f64) * step;
                        let site = self.table_site(world, &table, cx, cz, x, z);
                        let Some(plant) = f(&site, roll) else {
                            continue;
                        };
                        // Kept while the candidate's roll is under the local
                        // density times its species' share; its rank is the
                        // roll over the density, so a farther share is a subset.
                        if plant.p <= 0. || u >= plant.p * kept(plant.species) {
                            continue;
                        }
                        let rank = u / plant.p;
                        out.push(self.instance(&site, plant, rank, roll));
                    }
                }
            };
        scatter(grid(42.), 100, &mut out, &|s, roll| {
            let p = self.tree_density(s);
            (p > 0.).then(|| Plant {
                p,
                ..self.tree(s, roll)
            })
        });
        scatter(grid(44.), 200, &mut out, &|s, roll| {
            let p = self.bush_density(s);
            (p > 0.).then(|| self.bush(s, roll, p))
        });
        scatter(grid(52.), 300, &mut out, &|s, roll| {
            let p = self.rock_density(s);
            (p > 0.).then(|| self.rock(s, roll, p))
        });
        if climate == 0 {
            scatter(grid(64.), 350, &mut out, &|s, roll| {
                if s.blocked || !matches!(s.kind, Kind::Field | Kind::Grass) {
                    return None;
                }
                let roadside = [(-96, 0), (96, 0), (0, -96), (0, 96)]
                    .iter()
                    .any(|&(dx, dz)| self.kind_at(s.x as i64 + dx, s.z as i64 + dz) == Kind::Road);
                let village = world.city_fill.as_ref().is_some_and(|fill| {
                    fill.layout.map.districts.iter().any(|d| {
                        let r = d.half[0].max(d.half[1]).max(1) as f64;
                        let distance = (s.x - d.center[0] as f64).hypot(s.z - d.center[1] as f64);
                        distance > r && distance < r + 350.
                    })
                });
                if !roadside && !village {
                    return None;
                }
                let mut plant = self.tree(s, roll);
                plant.p = if roadside { 0.80 } else { 0.55 };
                plant.height = if village {
                    18. + 12. * unit(roll >> 11)
                } else {
                    40. + 20. * unit(roll >> 11)
                };
                plant.width = if village { 0.8 } else { 0.45 };
                Some(plant)
            });
        }
        // Groves: one object for a clump of trees, deep in woodland, drawn
        // where single trees have thinned with distance.
        if climate != 1 {
            scatter(160., 400, &mut out, &|s, roll| {
                let p = if s.kind == Kind::Forest && s.wood > 0.7 {
                    self.tree_density(s) * 0.8
                } else {
                    0.
                };
                (p > 0.).then(|| Plant {
                    p,
                    species: if self.conifer_share(s) > unit(roll >> 3) {
                        Species::PineGrove
                    } else {
                        Species::Grove
                    },
                    height: 48. + 18. * unit(roll >> 9),
                    width: 1.,
                    tint: leaf_tint(s.color, roll),
                })
            });
        }
        // Palm groves: the green belts along desert water, and palm country
        // in the tropics.
        scatter(220., 450, &mut out, &|s, roll| {
            let green = matches!(s.kind, Kind::Scrub | Kind::Forest | Kind::Grass);
            let p = match climate {
                1 if green => 0.48 * s.near_water.powi(2),
                1 if s.kind == Kind::Sand => 0.42 * s.near_water.powi(2),
                3 if self.region.trees == 2 && s.kind == Kind::Forest => 0.1 + 0.3 * s.near_water,
                _ => 0.,
            };
            (p > 0. && !s.blocked).then(|| Plant {
                p,
                species: Species::PalmGrove,
                height: 45. + 22. * unit(roll >> 9),
                width: 0.75,
                tint: leaf_tint(s.color, roll),
            })
        });
        if climate == 1 {
            // Outcrops on slopes and stony ground, each ringed by boulders.
            let mut crags = Vec::new();
            scatter(256., 500, &mut crags, &|s, roll| {
                let stony =
                    smoothstep(5., 16., s.slope).max(if s.kind == Kind::Rock { 0.7 } else { 0. });
                let p = (0.35 * stony + 0.03 * s.gravel + 0.08 * s.bank) * (1. - s.sea);
                (p > 0. && !s.blocked).then(|| Plant {
                    p,
                    species: Species::Outcrop,
                    height: 10. + 28. * unit(roll >> 4).powi(2),
                    width: 1.5 + 1.2 * unit(roll >> 12),
                    tint: rock_tint(s.color, roll),
                })
            });
            let boulders = kept(Species::Boulder);
            for crag in crags {
                let around = if crag.rank < boulders {
                    5 + (crag.rank_bits() % 9)
                } else {
                    0
                };
                for k in 0..around {
                    let roll = hash(crag.pos[0] as i32, crag.pos[2] as i32, 600 + k);
                    let a = unit(roll) * std::f32::consts::TAU;
                    let d = crag.height * (1.5 + 4.5 * unit(roll >> 8));
                    let x = f64::from(crag.pos[0] + a.cos() * d);
                    let z = f64::from(crag.pos[2] + a.sin() * d);
                    let site = self.site(world, x, z);
                    if site.blocked || site.sea > 0.5 {
                        continue;
                    }
                    let plant = Plant {
                        p: 1.,
                        species: Species::Boulder,
                        height: 3.5 + 10. * unit(roll >> 16).powi(2),
                        width: 1.1 + 0.7 * unit(roll >> 20),
                        tint: rock_tint(site.color, roll),
                    };
                    out.push(self.instance(&site, plant, crag.rank, roll));
                }
                out.push(crag);
            }
        } else {
            self.hedgerows(world, &table, cx, cz, &kept, &mut out);
        }
        out
    }

    /// Hedgerows along the edges of the fields' 1,024 ft plots (the ground
    /// detail's plot grid), where both sides are field.
    fn hedgerows(
        &self,
        world: &Terrain,
        table: &Table,
        cx: i64,
        cz: i64,
        kept: &dyn Fn(Species) -> f32,
        out: &mut Vec<Inst>,
    ) {
        let share = kept(Species::Broadleaf).max(kept(Species::GreenBush));
        let plot = 1_024i64;
        let x0 = cx * CHUNK;
        let z0 = cz * CHUNK;
        for line in 0..(CHUNK / plot) * 2 {
            let vertical = line % 2 == 0;
            let k = line / 2;
            let (lx, lz) = if vertical {
                (x0 + k * plot, z0)
            } else {
                (x0, z0 + k * plot)
            };
            for seg in 0..CHUNK / plot {
                let sx = if vertical { lx } else { lx + seg * plot };
                let sz = if vertical { lz + seg * plot } else { lz };
                let id = hash(sx as i32, sz as i32, if vertical { 800 } else { 801 });
                if id % 100 >= 55 {
                    continue;
                }
                let step = 32.;
                let mut t = 0.;
                let mut i = 0u32;
                while t < plot as f64 {
                    let roll = hash(sx as i32, sz as i32, 900 + i);
                    i += 1;
                    let along = t + f64::from(unit(roll)) * 6.;
                    t += step;
                    let u = unit(roll >> 4);
                    let p = 0.9;
                    if u >= p * share {
                        continue;
                    }
                    let side = (f64::from(unit(roll >> 8)) - 0.5) * 6.;
                    let (x, z) = if vertical {
                        (sx as f64 + side, sz as f64 + along)
                    } else {
                        (sx as f64 + along, sz as f64 + side)
                    };
                    let probe = 70.;
                    let (ax, az, bx, bz) = if vertical {
                        (x - probe, z, x + probe, z)
                    } else {
                        (x, z - probe, x, z + probe)
                    };
                    let a = self.table_site(world, table, cx, cz, ax, az);
                    let b = self.table_site(world, table, cx, cz, bx, bz);
                    let site = self.table_site(world, table, cx, cz, x, z);
                    if !field_edge(a.kind, b.kind) || site.blocked {
                        continue;
                    }
                    let tree = !roll.is_multiple_of(4);
                    let plant = if tree {
                        Plant {
                            p,
                            height: 42. + 18. * unit(roll >> 12),
                            width: 0.42,
                            ..self.tree(&site, roll)
                        }
                    } else {
                        Plant {
                            p,
                            species: Species::GreenBush,
                            height: 9. + 5. * unit(roll >> 12),
                            width: 1.3,
                            tint: leaf_tint(site.color, roll),
                        }
                    };
                    if u >= p * kept(plant.species) {
                        continue;
                    }
                    out.push(self.instance(&site, plant, u / p, roll));
                }
            }
        }
    }

    fn conifer_share(&self, s: &Site) -> f32 {
        let (full, none) = tree_line(self.region);
        let high = smoothstep(full * 0.35, (full + none) * 0.5, s.height);
        match (self.region.climate, self.region.trees) {
            (2, _) => 0.9,
            (3, _) => 0.,
            (_, 1) => 0.65 + 0.3 * high,
            _ => 0.12 + 0.7 * high,
        }
    }

    fn tree(&self, s: &Site, roll: u32) -> Plant {
        let pick = unit(roll >> 3);
        let size = unit(roll >> 11);
        let tint = leaf_tint(s.color, roll);
        let plant = |species, height: f32, width| Plant {
            p: 1.,
            species,
            height,
            width,
            tint,
        };
        if self.region.climate == 1 {
            let green = matches!(s.kind, Kind::Scrub | Kind::Forest | Kind::Grass);
            if (green && s.near_water > 0.2) || s.near_water > 0.6 {
                return if pick < 0.8 {
                    plant(Species::Palm, 34. + 26. * size, 1.)
                } else if pick < 0.9 {
                    plant(Species::Acacia, 20. + 12. * size, 1.)
                } else {
                    plant(Species::Broadleaf, 26. + 16. * size, 0.9)
                };
            }
            return if pick < 0.72 {
                plant(Species::Acacia, 14. + 14. * size, 1.)
            } else {
                Plant {
                    tint: [0.95, 0.9, 0.85],
                    ..plant(Species::Dead, 14. + 12. * size, 1.)
                }
            };
        }
        if self.region.trees == 2 && (s.near_water > 0.3 || s.height < 600.) && pick < 0.45 {
            return plant(Species::Palm, 40. + 30. * size, 1.);
        }
        if pick < self.conifer_share(s) {
            return plant(
                Species::Conifer,
                42. + 40. * size,
                0.9 + 0.2 * unit(roll >> 19),
            );
        }
        let rest = unit(roll >> 21);
        if rest < 0.07 && self.region.climate != 3 {
            plant(Species::Autumn, 34. + 24. * size, 1.)
        } else if rest < 0.085 {
            Plant {
                tint: [1.; 3],
                ..plant(Species::Dead, 26. + 18. * size, 1.)
            }
        } else {
            let tall = if self.region.climate == 3 { 1.25 } else { 1. };
            plant(
                Species::Broadleaf,
                (34. + 30. * size) * tall,
                0.9 + 0.3 * unit(roll >> 19),
            )
        }
    }

    fn bush(&self, s: &Site, roll: u32, p: f32) -> Plant {
        let size = unit(roll >> 11);
        let dry = if self.region.climate == 1 {
            s.near_water < 0.55
        } else {
            matches!(s.kind, Kind::Rock | Kind::Sand | Kind::Scrub)
                || (self.region.code == "GRE" && unit(roll >> 5) < 0.5)
        };
        if dry {
            Plant {
                p,
                species: Species::DryBush,
                height: 1.5 + 2.5 * size,
                width: 1.2,
                tint: [0.55, 0.43, 0.28],
            }
        } else {
            Plant {
                p,
                species: Species::GreenBush,
                height: 6. + 7. * size,
                width: 1.1,
                tint: leaf_tint(s.color, roll),
            }
        }
    }

    fn rock(&self, s: &Site, roll: u32, p: f32) -> Plant {
        let size = unit(roll >> 11);
        if size < 0.8 {
            Plant {
                p,
                species: Species::Stone,
                height: 0.8 + 2.2 * size,
                width: 1.0 + 0.8 * unit(roll >> 19),
                tint: rock_tint(s.color, roll),
            }
        } else {
            Plant {
                p,
                species: Species::Boulder,
                height: 3. + 8. * (size - 0.8) * 5.,
                width: 1.0 + 0.5 * unit(roll >> 19),
                tint: rock_tint(s.color, roll),
            }
        }
    }

    fn instance(&self, s: &Site, plant: Plant, rank: f32, roll: u32) -> Inst {
        let (scale, aspect) = instance_scale(plant.species, roll, s.wood, s.near_water);
        let height = plant.height * scale;
        Inst {
            pos: [s.x as f32, sprite_ground(s.height, height), s.z as f32],
            yaw: unit(roll >> 6) * std::f32::consts::TAU,
            height,
            width: plant.width * aspect,
            grad: s.grad,
            tint: plant.tint,
            rank: rank.clamp(0., 1.),
            species: plant.species,
            variant: (roll >> 24) as u8,
        }
    }

    /// The grass patches of one chunk with any cover.
    fn patches(&self, world: &Terrain, cx: i64, cz: i64) -> Vec<Patch> {
        let table = self.table(world, cx, cz);
        let n = CHUNK / PATCH;
        let mut out = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let x = (cx * CHUNK + i * PATCH + PATCH / 2) as f64;
                let z = (cz * CHUNK + j * PATCH + PATCH / 2) as f64;
                let site = self.table_site(world, &table, cx, cz, x, z);
                let (density, kind, height) = self.cover(&site);
                if density <= 0.01 {
                    continue;
                }
                out.push(Patch {
                    pos: [x as f32, sprite_ground(site.height, height), z as f32],
                    density,
                    grad: site.grad,
                    kind,
                    height,
                    color: lin(site.color),
                    hash: unit(hash((cx * n + i) as i32, (cz * n + j) as i32, 1_000)),
                });
            }
        }
        out
    }

    /// Ground cover at a site: share of full density, style and blade
    /// height (feet). Styles: 0 meadow, 1 crop, 2 woodland floor, 3 dry
    /// tufts and pebbles, 4 tall tropical grass, 5 tundra.
    fn cover(&self, s: &Site) -> (f32, f32, f32) {
        if s.blocked {
            return (0., 0., 0.);
        }
        let vary = 0.8 + 0.4 * detail_noise(div([s.x as f32, s.z as f32], 300.), 0, 71);
        match self.region.climate {
            1 => match s.kind {
                Kind::Scrub | Kind::Forest | Kind::Grass => (0.9, 1., 2.0 * vary),
                Kind::Sand => {
                    // Open sand stays bare. Tufts gather on wadi banks and
                    // beds, on gravel, and beside water.
                    let d = 0.55 * s.bank + 0.4 * s.bed + 0.3 * s.gravel + 0.5 * s.near_water;
                    ((d * (1. - 0.85 * s.sea)).min(0.7), 3., 1.2 * vary)
                }
                Kind::Rock => (0.25, 3., 0.9 * vary),
                _ => (0., 0., 0.),
            },
            2 => match s.kind {
                Kind::Grass | Kind::Forest => (0.6, 5., 0.9 * vary),
                Kind::Rock => (0.15, 5., 0.6),
                _ => (0., 0., 0.),
            },
            climate => {
                let tall = if climate == 3 { 1.5 } else { 1. };
                match s.kind {
                    Kind::Grass => (1., if climate == 3 { 4. } else { 0. }, 2.1 * vary * tall),
                    Kind::Field => (0.9, 1., 1.6 * vary),
                    Kind::Forest => (0.25, 2., 1.5 * vary * tall),
                    Kind::Scrub => (0.6, 0., 1.4 * vary),
                    Kind::Rock => (0.3, 3., 1.0 * vary),
                    Kind::Sand => (0.15, 3., 1.0),
                    _ => (0., 0., 0.),
                }
            }
        }
    }

    /// Top-down classification picture: `px` square pixels covering `size`
    /// feet around `center`, RGB. Water blue, blocked grey, field yellow,
    /// meadow light green, woodland dark green, rock brown; in a desert
    /// sand tan, sand sea orange, gravel grey-brown, wadi cyan, green belt
    /// green. Brightness carries the tree density; red dots are trees,
    /// white dots rocks.
    pub fn overlay(&self, world: &Terrain, center: [f64; 2], size: f64, px: usize) -> Vec<u8> {
        let mut out = vec![0u8; px * px * 3];
        let step = size / px as f64;
        for row in 0..px {
            for col in 0..px {
                let x = center[0] - size / 2. + (col as f64 + 0.5) * step;
                let z = center[1] + size / 2. - (row as f64 + 0.5) * step;
                let s = self.site(world, x, z);
                let trees = self.tree_density(&s);
                let class: [f32; 3] = if s.kind.wet() {
                    [40., 90., 200.]
                } else if s.blocked {
                    [130., 130., 130.]
                } else if self.region.climate == 1 {
                    if s.bed > 0.4 || s.bank > 0.4 {
                        [60., 200., 210.]
                    } else if matches!(s.kind, Kind::Scrub | Kind::Forest | Kind::Grass) {
                        [40., 160., 50.]
                    } else if s.slope > 6. || s.kind == Kind::Rock {
                        [120., 80., 60.]
                    } else if s.sea > 0.4 {
                        [235., 140., 40.]
                    } else if s.gravel > 0.4 {
                        [150., 130., 110.]
                    } else {
                        [220., 195., 140.]
                    }
                } else {
                    match s.kind {
                        Kind::Forest => [20., 90., 30.],
                        Kind::Field => [225., 205., 80.],
                        Kind::Grass => [120., 200., 90.],
                        Kind::Rock => [130., 100., 80.],
                        Kind::Snow => [240., 240., 245.],
                        Kind::Scrub => [150., 160., 80.],
                        _ => [200., 180., 130.],
                    }
                };
                let tile = s.color.map(f32::from);
                let lift = 0.75 + 0.5 * trees;
                for k in 0..3 {
                    out[(row * px + col) * 3 + k] =
                        ((class[k] * 0.7 + tile[k] * 0.3) * lift).clamp(0., 255.) as u8;
                }
            }
        }
        // The placed objects, as dots.
        let x0 = center[0] - size / 2.;
        let z1 = center[1] + size / 2.;
        let c0 = (x0 / CHUNK as f64).floor() as i64;
        let c1 = ((x0 + size) / CHUNK as f64).floor() as i64;
        let r0 = ((z1 - size) / CHUNK as f64).floor() as i64;
        let r1 = (z1 / CHUNK as f64).floor() as i64;
        for cz in r0..=r1 {
            for cx in c0..=c1 {
                for i in self.chunk(world, cx, cz, 0., 1., 1.) {
                    let col = ((f64::from(i.pos[0]) - x0) / step) as isize;
                    let row = ((z1 - f64::from(i.pos[2])) / step) as isize;
                    if col < 0 || row < 0 || col as usize >= px || row as usize >= px {
                        continue;
                    }
                    let dot = match i.species {
                        Species::Stone | Species::Boulder | Species::Outcrop => [255, 255, 255],
                        Species::DryBush | Species::GreenBush => [255, 160, 0],
                        _ => [230, 20, 20],
                    };
                    out[(row as usize * px + col as usize) * 3..][..3].copy_from_slice(&dot);
                }
            }
        }
        out
    }

    /// Horizontal positions of the chunk's objects at `density` (1 is High).
    /// `cache` must be used at that same density for the whole search.
    fn chunk_positions<'a>(
        &'a self,
        world: &Terrain,
        cache: &'a mut BTreeMap<(i64, i64), Vec<[f32; 2]>>,
        cx: i64,
        cz: i64,
        density: f32,
    ) -> &'a [[f32; 2]] {
        cache.entry((cx, cz)).or_insert_with(|| {
            self.chunk(world, cx, cz, 0., 1., density)
                .iter()
                .map(|inst| [inst.pos[0], inst.pos[2]])
                .collect()
        })
    }
}

/// One texel of a chunk's table.
#[derive(Clone, Copy)]
struct Texel {
    kind: Kind,
    blocked: bool,
    color: [u8; 3],
}

struct Table {
    texels: [Texel; GRID * GRID],
}

/// The ground at one point, everything placement reads.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub x: f64,
    pub z: f64,
    pub kind: Kind,
    pub blocked: bool,
    pub color: [u8; 3],
    /// Share of woodland texels around it, 0..1.
    pub wood: f32,
    /// Share of field texels around it, 0..1.
    pub field: f32,
    pub height: f32,
    /// Ground rise per foot east and north.
    pub grad: [f32; 2],
    pub slope: f32,
    /// 1 on a bank, 0 beyond about 2,600 ft from water.
    pub near_water: f32,
    /// Desert only: sand sea, wadi bed and bank, gravel plain, 0..1.
    pub sea: f32,
    pub bed: f32,
    pub bank: f32,
    pub gravel: f32,
}

/// Opinionated size spread requested by John, 2026-10-16. Independent hashes
/// avoid correlating shape, acceptance probability and size. Two uniform
/// samples produce a triangular distribution concentrated around the mean.
fn instance_scale(species: Species, seed: u32, wood: f32, water: f32) -> (f32, f32) {
    let a = unit(hash(seed as i32, 0, 1201));
    let b = unit(hash(seed as i32, 0, 1202));
    let tree = !matches!(
        species,
        Species::DryBush
            | Species::GreenBush
            | Species::Stone
            | Species::Boulder
            | Species::Outcrop
    );
    let spread = if tree { 0.4 } else { 0.65 };
    let habitat = if tree {
        if water > 0.65 {
            1.12
        } else if wood > 0.8 {
            1.08
        } else if wood > 0.05 && wood < 0.6 {
            0.78
        } else {
            1.
        }
    } else if wood > 0.05 && wood < 0.6 {
        0.8
    } else {
        1.
    };
    let scale = (1. + spread * (a + b - 1.)) * habitat;
    let aspect = 0.88 + 0.24 * unit(hash(seed as i32, 0, 1203));
    (scale, aspect)
}

struct Plant {
    /// Local density, 0..1.
    p: f32,
    species: Species,
    height: f32,
    width: f32,
    tint: [f32; 3],
}

/// What an object is. The renderer maps each to a retail shape when the
/// import has one, or to a drawn fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Species {
    Broadleaf,
    Autumn,
    Dead,
    Conifer,
    Palm,
    PalmGrove,
    Grove,
    PineGrove,
    DryBush,
    GreenBush,
    Acacia,
    Stone,
    Boulder,
    Outcrop,
}
impl Species {
    pub const ALL: [Self; 14] = [
        Self::Broadleaf,
        Self::Autumn,
        Self::Dead,
        Self::Conifer,
        Self::Palm,
        Self::PalmGrove,
        Self::Grove,
        Self::PineGrove,
        Self::DryBush,
        Self::GreenBush,
        Self::Acacia,
        Self::Stone,
        Self::Boulder,
        Self::Outcrop,
    ];
    /// All species have the same protected detail radius.
    pub fn reach(self) -> f32 {
        1.
    }
    pub const MAX_REACH: f32 = 1.;
    pub fn share(self, dist: f32, full: f32) -> f32 {
        share(dist, full)
    }
}

/// One placed object, world feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inst {
    pub pos: [f32; 3],
    pub yaw: f32,
    /// Height in feet; the mesh keeps its own proportions.
    pub height: f32,
    /// Horizontal stretch over the mesh's proportions.
    pub width: f32,
    /// Ground rise per foot east and north, for shearing to the slope.
    pub grad: [f32; 2],
    /// Linear colour multiplier (a target colour for recoloured rock).
    pub tint: [f32; 3],
    /// Drawn while the distance's share is above this.
    pub rank: f32,
    pub species: Species,
    pub variant: u8,
}
impl Inst {
    fn rank_bits(&self) -> u32 {
        (self.rank * 65_535.) as u32 ^ self.pos[0].to_bits()
    }
}

/// One grass patch, [`PATCH`] feet square, world feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Patch {
    pub pos: [f32; 3],
    pub density: f32,
    pub grad: [f32; 2],
    pub kind: f32,
    pub height: f32,
    pub color: [f32; 3],
    pub hash: f32,
}

/// The share of full density kept at `dist` feet when full density
/// reaches `full` feet (mirrored by the cull shader).
pub fn share(dist: f32, full: f32) -> f32 {
    if dist <= full {
        1.
    } else {
        (full / dist).powf(1.6)
    }
}

/// Fixed geographic object selections, built in bounded batches around the camera.
pub struct Placement {
    atlas: Arc<Atlas>,
    chunks: BTreeMap<(i64, i64), Chunk>,
    grass: BTreeMap<(i64, i64), Arc<Vec<Patch>>>,
    density: f32,
    /// Objects kept at most.
    budget: usize,
    chunk_budget: usize,
    ahead: [f64; 2],
    available: f32,
    /// Bumped whenever the drawn set changes.
    pub generation: u64,
    full: f32,
    eye_cell: Option<(i64, i64)>,
    settled: Option<(i64, i64, u32, u32)>,
    /// Object chunks built, for cache diagnostics.
    pub builds: u64,
    /// Grass chunks built, for cache diagnostics.
    pub grass_builds: u64,
    /// CPU time spent inside object-chunk and grass-chunk builds, nanoseconds.
    pub build_ns: u64,
}

struct Chunk {
    objects: Vec<Inst>,
}

/// Objects kept by default (the High density's budget).
pub const DEFAULT_BUDGET: usize = 262_144;
/// Grass patches kept at most.
pub const MAX_PATCHES: usize = 24_576;

impl Placement {
    pub fn new(atlas: Arc<Atlas>) -> Self {
        Self {
            atlas,
            chunks: BTreeMap::new(),
            grass: BTreeMap::new(),
            density: 1.,
            budget: DEFAULT_BUDGET,
            chunk_budget: usize::MAX,
            ahead: [0.; 2],
            available: 0.,
            generation: 0,
            full: 0.,
            eye_cell: None,
            settled: None,
            builds: 0,
            grass_builds: 0,
            build_ns: 0,
        }
    }

    /// Drops cached object and grass chunks so the next view fills them again.
    pub fn reset_cache(&mut self) {
        self.chunks.clear();
        self.grass.clear();
        self.settled = None;
        self.eye_cell = None;
        self.full = 0.;
        self.available = 0.;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn set_density(&mut self, density: f32) {
        if (density - self.density).abs() > 1e-4 {
            self.density = density;
            self.chunks.clear();
            self.settled = None;
            self.generation += 1;
        }
    }

    pub fn set_budget(&mut self, budget: usize) {
        if budget != self.budget {
            self.budget = budget;
            self.settled = None;
            self.generation += 1;
        }
    }

    /// Compatibility diagnostic: detail distance is never squeezed.
    pub fn squeezed(&self, full: f32) -> f32 {
        full
    }

    /// Brings the chunks within `draw` feet up to date for an eye at
    /// `eye`, prioritizing full detail within `full` feet.
    /// Builds at most `budget` chunks (all when
    /// `None`). Returns whether everything is current.
    pub fn update(
        &mut self,
        world: &Terrain,
        eye: [f64; 2],
        full: f32,
        draw: f32,
        budget: Option<usize>,
    ) -> bool {
        let cx = (eye[0] / CHUNK as f64).floor() as i64;
        let cz = (eye[1] / CHUNK as f64).floor() as i64;
        let key = (cx, cz, full.to_bits(), draw.to_bits());
        if self.settled == Some(key) {
            return true;
        }
        self.settled = None;
        // A stable ring for the entire camera cell, with a guard patch.
        let eye = [
            (cx as f64 + 0.5) * CHUNK as f64,
            (cz as f64 + 0.5) * CHUNK as f64,
        ];
        let draw = draw + 2. * CHUNK as f32;
        // Fixed geographic quotas reserve room for the entire guarded ring.
        // They depend only on settings, never on camera distance or local load.
        let radius = (draw / CHUNK as f32).ceil() as i64 + 1;
        let count = (-radius..=radius)
            .flat_map(|z| (-radius..=radius).map(move |x| (x, z)))
            .filter(|&(x, z)| chunk_distance([CHUNK as f64 * 0.5; 2], x, z) <= draw)
            .count();
        let quota = self.budget / count.max(1);
        if quota != self.chunk_budget {
            self.reset_cache();
            self.chunk_budget = quota;
        }
        let done = self.pass(world, eye, full, draw, budget);
        if done {
            self.settled = Some(key);
        }
        done
    }

    pub(super) fn prefetch(&mut self, ahead: [f64; 2]) {
        self.ahead = ahead;
    }

    fn pass(
        &mut self,
        world: &Terrain,
        eye: [f64; 2],
        full: f32,
        draw: f32,
        budget: Option<usize>,
    ) -> bool {
        let cell = (
            (eye[0] / CHUNK as f64).floor() as i64,
            (eye[1] / CHUNK as f64).floor() as i64,
        );
        if self.eye_cell != Some(cell) {
            self.eye_cell = Some(cell);
            self.generation += 1;
        }
        if self.full != full {
            self.full = full;
            self.generation += 1;
        }
        let c0x = ((eye[0] - f64::from(draw)) / CHUNK as f64).floor() as i64;
        let c1x = ((eye[0] + f64::from(draw)) / CHUNK as f64).floor() as i64;
        let c0z = ((eye[1] - f64::from(draw)) / CHUNK as f64).floor() as i64;
        let c1z = ((eye[1] + f64::from(draw)) / CHUNK as f64).floor() as i64;
        let mut wanted: Vec<(f32, (i64, i64))> = Vec::new();
        for cz in c0z..=c1z {
            for cx in c0x..=c1x {
                let d = chunk_distance(eye, cx, cz);
                if d <= draw {
                    wanted.push((d, (cx, cz)));
                }
            }
        }
        let ahead = [eye[0] + self.ahead[0], eye[1] + self.ahead[1]];
        wanted.sort_by(|a, b| {
            let score = |v: &(f32, (i64, i64))| {
                if v.0 <= full {
                    v.0
                } else {
                    full + chunk_distance(ahead, v.1.0, v.1.1)
                }
            };
            (a.0 > full)
                .cmp(&(b.0 > full))
                .then_with(|| score(a).total_cmp(&score(b)))
                .then(a.1.cmp(&b.1))
        });
        let keep: std::collections::BTreeSet<(i64, i64)> = wanted.iter().map(|w| w.1).collect();
        let before = self.chunks.len();
        self.chunks.retain(|k, _| keep.contains(k));
        let mut changed = self.chunks.len() != before;
        let missing: Vec<_> = wanted
            .iter()
            .filter(|(_, key)| !self.chunks.contains_key(key))
            .take(budget.unwrap_or(usize::MAX))
            .map(|(_, key)| *key)
            .collect();
        let started = std::time::Instant::now();
        let chunks = tore_workers::shared().ordered_map(&missing, 2, |_, key| {
            let mut objects = self.atlas.chunk(world, key.0, key.1, 0., 1., self.density);
            if objects.len() > self.chunk_budget {
                objects.sort_by(|a, b| {
                    a.rank
                        .total_cmp(&b.rank)
                        .then(a.pos[0].total_cmp(&b.pos[0]))
                        .then(a.pos[2].total_cmp(&b.pos[2]))
                });
                objects.truncate(self.chunk_budget);
            }
            (*key, Chunk { objects })
        });
        if !chunks.is_empty() {
            self.build_ns = self
                .build_ns
                .saturating_add(started.elapsed().as_nanos() as u64);
        }
        for (key, chunk) in chunks {
            if std::env::var_os("TORE_VEG_STREAM_TRACE").is_some() {
                eprintln!("vegetation build chunk={key:?}");
            }
            self.chunks.insert(key, chunk);
            self.builds += 1;
            changed = true;
        }
        let mut done = true;
        let mut available = draw;
        for (d, key) in &wanted {
            if !self.chunks.contains_key(key) {
                done = false;
                available = available.min(*d);
            }
        }
        self.available = available;
        if changed {
            self.generation += 1;
        }
        done
    }

    /// Complete cached circle around this eye, before publication/upload.
    pub(super) fn available(&self, eye: [f64; 2]) -> f32 {
        let center = eye.map(|v| ((v / CHUNK as f64).floor() + 0.5) * CHUNK as f64);
        (self.available - (eye[0] - center[0]).hypot(eye[1] - center[1]) as f32).max(0.)
    }

    /// Feet to the farthest chunk [`Self::objects`] keeps.
    pub fn kept_reach(&self, eye: [f64; 2]) -> f32 {
        let mut keys: Vec<(f32, usize)> = self
            .chunks
            .iter()
            .map(|(&(cx, cz), c)| (chunk_distance(eye, cx, cz), c.objects.len()))
            .collect();
        keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut total = 0;
        let mut reach = 0.;
        for (d, n) in keys {
            if total + n > self.budget {
                break;
            }
            total += n;
            reach = d;
        }
        reach
    }

    /// The current objects, nearest chunks first, capped.
    #[cfg(test)]
    pub fn objects(&self, eye: [f64; 2]) -> Vec<Inst> {
        self.object_chunks(eye)
            .into_iter()
            .flat_map(|(_, v)| v)
            .collect()
    }

    /// Same nearest-first selection, retaining chunk identity for streaming.
    pub(super) fn object_chunks(&self, eye: [f64; 2]) -> Vec<((i64, i64), Vec<Inst>)> {
        let mut keys: Vec<_> = self
            .chunks
            .keys()
            .map(|&(x, z)| (chunk_distance(eye, x, z), (x, z)))
            .collect();
        keys.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut out = Vec::new();
        for (_, key) in keys {
            let objects = self.chunks[&key].objects.clone();
            if !objects.is_empty() {
                out.push((key, objects));
            }
        }
        out
    }

    /// Grass patches within `radius` feet, building at most `budget`
    /// chunks. Returns the patches and whether everything was built.
    pub fn grass(
        &mut self,
        world: &Terrain,
        eye: [f64; 2],
        radius: f32,
        budget: Option<usize>,
    ) -> (Vec<Patch>, bool) {
        if radius <= 0. {
            self.grass.clear();
            return (Vec::new(), true);
        }
        let r = f64::from(radius);
        let c0x = ((eye[0] - r) / CHUNK as f64).floor() as i64;
        let c1x = ((eye[0] + r) / CHUNK as f64).floor() as i64;
        let c0z = ((eye[1] - r) / CHUNK as f64).floor() as i64;
        let c1z = ((eye[1] + r) / CHUNK as f64).floor() as i64;
        let mut wanted = Vec::new();
        for cz in c0z..=c1z {
            for cx in c0x..=c1x {
                let d = chunk_distance(eye, cx, cz);
                if d <= radius {
                    wanted.push((d, (cx, cz)));
                }
            }
        }
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let keep: std::collections::BTreeSet<(i64, i64)> = wanted.iter().map(|w| w.1).collect();
        self.grass.retain(|k, _| keep.contains(k));
        let mut built = 0;
        let mut done = true;
        let mut out = Vec::new();
        for (_, key) in wanted {
            if !self.grass.contains_key(&key) {
                if budget.is_some_and(|b| built >= b) {
                    done = false;
                    continue;
                }
                let started = std::time::Instant::now();
                let patches = self.atlas.patches(world, key.0, key.1);
                self.build_ns = self.build_ns.saturating_add(
                    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
                );
                self.grass.insert(key, Arc::new(patches));
                self.grass_builds += 1;
                built += 1;
            }
            let patches = &self.grass[&key];
            for p in patches.iter() {
                let dx = f64::from(p.pos[0]) - eye[0];
                let dz = f64::from(p.pos[2]) - eye[1];
                if dx * dx + dz * dz <= r * r {
                    out.push(*p);
                }
            }
            if out.len() >= MAX_PATCHES {
                out.truncate(MAX_PATCHES);
                break;
            }
        }
        (out, done)
    }
}

/// Feet from `eye` to the nearest point of chunk `(cx, cz)`.
#[cfg(test)]
fn distance_fade(distance: f32, full: f32, draw: f32) -> f32 {
    if distance <= full {
        1.
    } else {
        1. - smoothstep(full, draw.max(full + 1.), distance)
    }
}

fn chunk_distance(eye: [f64; 2], cx: i64, cz: i64) -> f32 {
    let x0 = (cx * CHUNK) as f64;
    let z0 = (cz * CHUNK) as f64;
    let dx = (x0 - eye[0]).max(eye[0] - x0 - CHUNK as f64).max(0.);
    let dz = (z0 - eye[1]).max(eye[1] - z0 - CHUNK as f64).max(0.);
    dx.hypot(dz) as f32
}

fn median_height(world: &Terrain) -> f32 {
    let cell = tore_formats::theater::CELL_FEET;
    let mut h = Vec::new();
    for row in 0..world.theater.rows {
        for col in 0..world.theater.cols {
            h.push(world.height(col as f32 * cell, row as f32 * cell));
        }
    }
    if h.is_empty() {
        return 0.;
    }
    h.sort_by(f32::total_cmp);
    h[h.len() / 2]
}

/// Both sprite layers have a normalized upright quad from zero to one.
/// Keep footing in the shared grounding path even when its correction is zero.
fn sprite_ground(terrain: f32, height: f32) -> f32 {
    static BOTTOM: std::sync::LazyLock<f64> = std::sync::LazyLock::new(|| {
        tore_world::grounding::sprites_bottom(&[tore_formats::shape::Billboard {
            center: [0., 0., 0.5],
            size: [1., 1.],
            texture: String::new(),
            uv: None,
            fog: Default::default(),
            address: 0,
        }])
        .expect("vegetation quad has a bottom")
    });
    terrain - (*BOTTOM as f32) * height
}

fn leaf_tint(ground: [u8; 3], roll: u32) -> [f32; 3] {
    // Leaves lean a little toward the ground's own green, so a theater's
    // palette carries into its trees, and vary in brightness.
    let g = lin(ground);
    let mean = (g[0] + g[1] + g[2]) / 3.;
    let vary = 0.86 + 0.24 * unit(roll >> 13);
    std::array::from_fn(|k| {
        let rel = if mean > 0.002 {
            (g[k] / mean / [0.8, 1.35, 0.85][k]).clamp(0.75, 1.3)
        } else {
            1.
        };
        vary * (1. + (rel - 1.) * 0.35)
    })
}

fn rock_tint(ground: [u8; 3], roll: u32) -> [f32; 3] {
    // Rock takes the local ground's hue, a little greyer; its own faces
    // give the contrast. One in five carries a dark desert varnish.
    let base = lin(mix(ground, [150, 138, 124], 0.2));
    let varnish = if unit(roll >> 24) < 0.2 { 0.55 } else { 1. };
    let vary = (0.78 + 0.27 * unit(roll >> 14)) * varnish;
    base.map(|v| v * vary)
}

fn hash(x: i32, z: i32, salt: u32) -> u32 {
    let mut n = (x as u32).wrapping_mul(0x9e37_79b1)
        ^ (z as u32).wrapping_mul(0x85eb_ca6b)
        ^ salt.wrapping_mul(0xc2b2_ae35);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb_352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846c_a68b);
    n ^= n >> 16;
    n
}

/// 0..1 from the low 24 bits.
/// A uniform 0..1 from any bits of a roll: scrambled first, so a roll
/// shifted down to a few bits still spreads over the whole range.
fn unit(n: u32) -> f32 {
    let mut n = n.wrapping_mul(0x9e37_79b9) ^ 0x5bd1_e995;
    n ^= n >> 15;
    n = n.wrapping_mul(0x85eb_ca6b);
    n ^= n >> 13;
    (n >> 8) as f32 / 16_777_216.
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn div(w: [f32; 2], s: f32) -> [f32; 2] {
    [w[0] / s, w[1] / s]
}

/// `terrain.wgsl` `detail_hash`.
pub fn detail_hash(c: [i32; 2], salt: u32) -> f32 {
    let mut h = (c[0] as u32).wrapping_mul(0x8da6_b343)
        ^ (c[1] as u32).wrapping_mul(0xd816_3841)
        ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.
}

/// `terrain.wgsl` `detail_noise`.
pub fn detail_noise(p: [f32; 2], period: i32, salt: u32) -> f32 {
    let i = [p[0].floor(), p[1].floor()];
    let f = [p[0] - i[0], p[1] - i[1]];
    let u = f.map(|f| f * f * (3. - 2. * f));
    let m = if period > 0 { period - 1 } else { -1 };
    let c = [i[0] as i32, i[1] as i32];
    let at = |dx: i32, dz: i32| detail_hash([(c[0] + dx) & m, (c[1] + dz) & m], salt);
    let a = at(0, 0);
    let b = at(1, 0);
    let d = at(0, 1);
    let e = at(1, 1);
    let lerp = |x: f32, y: f32, t: f32| x + (y - x) * t;
    lerp(lerp(a, b, u[0]), lerp(d, e, u[0]), u[1])
}

/// `terrain.wgsl` `sand_sea`.
pub fn sand_sea(w: [f32; 2]) -> f32 {
    let n = detail_noise(div(w, 36_000.), 0, 11) * 0.7 + detail_noise(div(w, 9_000.), 0, 12) * 0.3;
    smoothstep(0.42, 0.60, n)
}

/// `terrain.wgsl` `wadi`.
pub fn wadi(w: [f32; 2], widen: f32) -> f32 {
    let warp = [
        detail_noise(div(w, 5_200.), 0, 13) - 0.5,
        detail_noise(div(w, 5_200.), 0, 14) - 0.5,
    ];
    let p = [
        w[0] / 14_000. + warp[0] * 0.9,
        w[1] / 14_000. + warp[1] * 0.9,
    ];
    let n = detail_noise(p, 0, 15) - 0.5;
    let width = (0.008 + 0.006 * detail_noise(div(w, 3_000.), 0, 16)) * widen;
    1. - smoothstep(width * 0.45, width, n.abs())
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t)
            .round()
            .clamp(0., 255.) as u8
    })
}

pub fn lin(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| {
        let x = f32::from(v) / 255.;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    })
}

fn load_picture(resources: &dyn ResourceSource, name: &str) -> Option<Picture> {
    let bytes = resources.get(name)?;
    let pic = tore_formats::Pic::parse(bytes).ok()?;
    if pic.palette.is_empty() && pic.width == 256 && pic.height == 256 {
        let indices = pic
            .pixels
            .iter()
            .zip(&pic.mask)
            .map(|(p, m)| if *m { *p } else { 255 })
            .collect();
        return Some(Picture { indices });
    }
    if pic.palette.is_empty() && pic.width == 128 && pic.height == 128 {
        let mut indices = vec![255u8; TEXELS * TEXELS];
        for row in 0..128 {
            for col in 0..128 {
                let src = if pic.mask[row * 128 + col] {
                    pic.pixels[row * 128 + col]
                } else {
                    255
                };
                for dy in 0..2 {
                    for dx in 0..2 {
                        indices[(row * 2 + dy) * TEXELS + col * 2 + dx] = src;
                    }
                }
            }
        }
        return Some(Picture { indices });
    }
    None
}

fn paved(world: &Terrain, x: f64, z: f64) -> bool {
    let scene = &world.airport_scene;
    if scene.runway_surface(x, z).is_some() {
        return true;
    }
    [(220., 0.), (-220., 0.), (0., 220.), (0., -220.)]
        .into_iter()
        .any(|(dx, dz)| scene.runway_surface(x + dx, z + dz).is_some())
}

/// Shares of each kind across a theater's placed tiles, for the survey.
pub struct Shares {
    pub land: usize,
    pub counts: [usize; 12],
    pub pictures: usize,
    pub placements: usize,
    pub tone: Tone,
}

pub fn shares(resources: &dyn ResourceSource, environment: &Environment, layout: &str) -> Shares {
    let atlas = Atlas::from_environment(resources, environment, layout);
    let mut counts = [0usize; 12];
    let mut land = 0usize;
    for (pic, _) in atlas.placements.values() {
        for &index in &atlas.pictures[*pic as usize].indices {
            if index == 255 {
                counts[Kind::Water as usize] += 1;
                continue;
            }
            land += 1;
            counts[atlas.kind_index(index) as usize] += 1;
        }
    }
    Shares {
        land,
        counts,
        pictures: atlas.pictures.len(),
        placements: atlas.placements.len(),
        tone: atlas.tone,
    }
}

/// `--vegetation-survey [THEATER ...]`: one line per theater.
pub fn survey() -> crate::AppResult<()> {
    let wanted: Vec<String> = std::env::args().skip(2).collect();
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    println!(
        "theater climate trees cover pictures placements tone(dark,light,yellow) water field grass forest sand rock snow scrub river road"
    );
    for code in tore_formats::quick_template::tables::THEATERS {
        if !wanted.is_empty() && !wanted.iter().any(|w| w.eq_ignore_ascii_case(code)) {
            continue;
        }
        let layout = format!("{code}.MM");
        let Some(bytes) = resources.get(&layout) else {
            println!("{code}: no layout");
            continue;
        };
        let environment = Environment::parse(bytes)?;
        let region = region(code);
        let s = shares(resources, &environment, &layout);
        let pct = |k: Kind| {
            if s.land == 0 {
                0
            } else {
                s.counts[k as usize] * 100 / s.land
            }
        };
        let water = s.counts[Kind::Water as usize];
        let total = s.land + water;
        println!(
            "{code} {} {} {} {:.2} {} {} ({},{},{:.2}) {}% {}% {}% {}% {}% {}% {}% {}% {}% {}%",
            region.name,
            match region.climate {
                1 => "desert",
                2 => "arctic",
                3 => "tropical",
                _ => "temperate",
            },
            match region.trees {
                1 => "conifer",
                2 => "palm",
                _ => "broadleaf",
            },
            region.cover,
            s.pictures,
            s.placements,
            s.tone.dark,
            s.tone.light,
            s.tone.yellow,
            if total == 0 { 0 } else { water * 100 / total },
            pct(Kind::Field),
            pct(Kind::Grass),
            pct(Kind::Forest),
            pct(Kind::Sand),
            pct(Kind::Rock),
            pct(Kind::Snow),
            pct(Kind::Scrub),
            pct(Kind::River),
            pct(Kind::Road),
        );
    }
    Ok(())
}

/// A spot the preview can look at: world feet, and a heading in degrees
/// (0 north, clockwise).
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub x: f64,
    pub z: f64,
    pub heading: f64,
    pub label: &'static str,
}

/// Picks the views the preview renders, by scoring sample points over the
/// placed tiles: woodland, field, a hillside, a river bank, open ground;
/// and in a desert a dune field, rocky ground and an oasis or river. Bare
/// land is one flat colour and is skipped. Ties go to the point nearer
/// the middle of the map.
pub fn spots(atlas: &Atlas, world: &Terrain) -> Vec<Spot> {
    let cols = world.theater.cols.max(2) as i64;
    let rows = world.theater.rows.max(2) as i64;
    let mid_x = (cols * tore_formats::theater::CELL_FEET as i64 / 2) as f64;
    let mid_z = (rows * tore_formats::theater::CELL_FEET as i64 / 2) as f64;
    let desert = atlas.region.climate == 1;
    let labels: &[&'static str] = if desert {
        &["open", "dunes", "rocky", "oasis", "wadi", "river"]
    } else {
        &["open", "wood", "field", "hill", "meadow", "river"]
    };
    let mut best: Vec<Option<(f32, Spot)>> = vec![None; labels.len()];
    let step = 2_048i64;
    for &key in atlas.placements.keys() {
        for j in 0..CELL / step {
            for i in 0..CELL / step {
                let x = (i64::from(key.0) * CELL + i * step + step / 2) as f64;
                let z = (i64::from(key.1) * CELL + j * step + step / 2) as f64;
                let s = atlas.site(world, x, z);
                if s.blocked {
                    continue;
                }
                let center = -((x - mid_x).abs() + (z - mid_z).abs()) as f32 * 1e-7;
                // Score a ring of points around the site for broad areas.
                let ring = |f: &dyn Fn(&Site) -> f32| {
                    let mut sum = 0.;
                    for (dx, dz) in [
                        (0., 0.),
                        (1500., 0.),
                        (-1500., 0.),
                        (0., 1500.),
                        (0., -1500.),
                    ] {
                        sum += f(&atlas.site(world, x + dx, z + dz));
                    }
                    sum / 5.
                };
                let uphill = f64::from(s.grad[0])
                    .atan2(f64::from(s.grad[1]))
                    .to_degrees();
                let scores: Vec<(f32, f64)> = if desert {
                    let sand = |s: &Site| {
                        if s.kind == Kind::Sand && !s.blocked {
                            1.
                        } else {
                            0.
                        }
                    };
                    vec![
                        (
                            ring(&|s| sand(s) * (1. - s.sea) * (1. - s.gravel) * (1. - s.bed)),
                            35.,
                        ),
                        (ring(&|s| s.sea * sand(s)), 35. + 90.),
                        (
                            ring(&|s| {
                                smoothstep(4., 12., s.slope).max(if s.kind == Kind::Rock {
                                    0.7
                                } else {
                                    0.
                                })
                            }),
                            uphill,
                        ),
                        (
                            ring(&|s| {
                                if s.blocked {
                                    -0.6
                                } else if matches!(s.kind, Kind::Scrub | Kind::Forest | Kind::Grass)
                                {
                                    0.5 + s.near_water
                                } else {
                                    0.
                                }
                            }),
                            35.,
                        ),
                        (ring(&|s| s.bank.max(s.bed * 0.5) * sand(s)), 35.),
                        (0., 0.),
                    ]
                } else {
                    vec![
                        (
                            ring(&|s| {
                                if matches!(s.kind, Kind::Grass | Kind::Field) {
                                    1.
                                } else {
                                    0.
                                }
                            }),
                            35.,
                        ),
                        (ring(&|s| s.wood * (1. - smoothstep(4., 10., s.slope))), 35.),
                        (ring(&|s| s.field * (1. - smoothstep(3., 8., s.slope))), 35.),
                        (
                            if s.height < tree_line(atlas.region).0 {
                                ring(&|s| {
                                    smoothstep(3., 15., s.slope) * (0.3 + atlas.tree_density(s))
                                })
                            } else {
                                0.
                            },
                            uphill,
                        ),
                        (
                            ring(&|s| {
                                if s.kind == Kind::Grass {
                                    (1. - s.wood) * (1. - smoothstep(3., 8., s.slope))
                                } else {
                                    0.
                                }
                            }),
                            35.,
                        ),
                        (0., 0.),
                    ]
                };
                for (slot, (score, heading)) in scores.into_iter().enumerate() {
                    let ranked = score + center;
                    if score > 0.05 && best[slot].is_none_or(|(b, _)| ranked > b) {
                        best[slot] = Some((
                            ranked,
                            Spot {
                                x,
                                z,
                                heading,
                                label: labels[slot],
                            },
                        ));
                    }
                }
            }
        }
    }
    // A river or shore: a land point about 1,500 ft from water, facing it.
    let river = labels.len() - 1;
    for (&key, (pic, rotation)) in &atlas.placements {
        if let Some((wx, wz, lx, lz)) = bank(atlas, key, *pic, *rotation) {
            let s = atlas.site(world, lx as f64, lz as f64);
            if s.blocked {
                continue;
            }
            let center = -((lx as f64 - mid_x).abs() + (lz as f64 - mid_z).abs()) as f32 * 1e-7;
            let green = if matches!(s.kind, Kind::Grass | Kind::Forest | Kind::Scrub) {
                0.5
            } else {
                0.
            };
            let ranked = 1. + green + center;
            if best[river].is_none_or(|(b, _)| ranked > b) {
                let heading = ((wx - lx) as f64).atan2((wz - lz) as f64).to_degrees();
                best[river] = Some((
                    ranked,
                    Spot {
                        x: lx as f64,
                        z: lz as f64,
                        heading,
                        label: labels[river],
                    },
                ));
            }
        }
    }
    let mut out: Vec<Spot> = best.into_iter().flatten().map(|(_, s)| s).collect();
    if out.is_empty() {
        out.push(Spot {
            x: mid_x,
            z: mid_z,
            heading: 20.,
            label: "center",
        });
    }
    out
}

/// A place the low pass can fly through, with the High-density instance
/// count inside one nautical mile.
#[derive(Clone, Debug)]
pub struct DensityCandidate {
    pub x: f64,
    pub z: f64,
    pub heading: f64,
    pub label: String,
    pub instances: usize,
}

/// The densest High-density stand within one nautical mile.
///
/// Candidates are the preview [`spots`] plus up to eight separated samples
/// of tree, bush and rock cover. The count is the cached chunk's full
/// candidate set (no distance thinning) clipped to the disk. Ties prefer a
/// named spot.
pub fn densest(atlas: &Atlas, world: &Terrain) -> (DensityCandidate, Vec<DensityCandidate>) {
    let mut pending: Vec<(f64, f64, f64, String)> = spots(atlas, world)
        .into_iter()
        .map(|spot| (spot.x, spot.z, spot.heading, spot.label.to_string()))
        .collect();
    let mut scored = Vec::new();
    for &key in atlas.placements.keys() {
        for (u, v) in [(0.25f64, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
            let x = (f64::from(key.0) + u) * CELL as f64;
            let z = (f64::from(key.1) + v) * CELL as f64;
            let site = atlas.site(world, x, z);
            let score = atlas.tree_density(&site)
                + 0.5 * atlas.bush_density(&site)
                + 0.25 * atlas.rock_density(&site);
            if score > 0.05 {
                scored.push((score, x, z));
            }
        }
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.total_cmp(&b.1)));
    let mut taken = 0usize;
    for (_, x, z) in scored {
        if taken == 8 {
            break;
        }
        let near = pending.iter().any(|(px, pz, _, _)| {
            let dx = px - x;
            let dz = pz - z;
            dx * dx + dz * dz < NM * NM
        });
        if near {
            continue;
        }
        pending.push((x, z, 35., "scan".to_string()));
        taken += 1;
    }
    let mut cache = BTreeMap::new();
    let mut found = Vec::new();
    for (x, z, heading, label) in pending {
        found.push(DensityCandidate {
            instances: instances_within(
                atlas,
                world,
                &mut cache,
                x,
                z,
                NM,
                crate::graphics::VegDensity::High.scale(),
            ),
            x,
            z,
            heading,
            label,
        });
    }
    found.sort_by(|a, b| {
        b.instances
            .cmp(&a.instances)
            .then((a.label == "scan").cmp(&(b.label == "scan")))
            .then(a.label.cmp(&b.label))
    });
    let winner = found.first().cloned().unwrap_or(DensityCandidate {
        x: 0.,
        z: 0.,
        heading: 35.,
        label: "center".to_string(),
        instances: 0,
    });
    (winner, found)
}

pub fn count_high(atlas: &Atlas, world: &Terrain, x: f64, z: f64) -> usize {
    instances_within(
        atlas,
        world,
        &mut BTreeMap::new(),
        x,
        z,
        NM,
        crate::graphics::VegDensity::High.scale(),
    )
}

/// High-density objects (`density` 1) whose positions lie within `radius` feet.
fn instances_within(
    atlas: &Atlas,
    world: &Terrain,
    cache: &mut BTreeMap<(i64, i64), Vec<[f32; 2]>>,
    x: f64,
    z: f64,
    radius: f64,
    density: f32,
) -> usize {
    let r2 = radius * radius;
    let c0x = ((x - radius) / CHUNK as f64).floor() as i64;
    let c1x = ((x + radius) / CHUNK as f64).floor() as i64;
    let c0z = ((z - radius) / CHUNK as f64).floor() as i64;
    let c1z = ((z + radius) / CHUNK as f64).floor() as i64;
    let mut n = 0usize;
    for cz in c0z..=c1z {
        for cx in c0x..=c1x {
            for pos in atlas.chunk_positions(world, cache, cx, cz, density) {
                let dx = f64::from(pos[0]) - x;
                let dz = f64::from(pos[1]) - z;
                if dx * dx + dz * dz <= r2 {
                    n += 1;
                }
            }
        }
    }
    n
}

/// A water texel and a nearby open point on the bank: water x, z, then land x, z.
fn bank(atlas: &Atlas, key: (i32, i32), pic: u16, rotation: u8) -> Option<(i64, i64, i64, i64)> {
    let indices = &atlas.pictures[pic as usize].indices;
    let mut row = 0usize;
    while row < TEXELS {
        let mut col = 0usize;
        while col < TEXELS {
            if atlas.kind_index(indices[row * TEXELS + col]).wet() {
                let (tu, tv) = match rotation % 4 {
                    0 => (col, 255 - row),
                    1 => (255 - row, 255 - col),
                    2 => (255 - col, row),
                    _ => (row, col),
                };
                let wx = i64::from(key.0) * CELL + tu as i64 * TEXEL;
                let wz = i64::from(key.1) * CELL + tv as i64 * TEXEL;
                for (dx, dz) in [(1_500, 0), (-1_500, 0), (0, 1_500), (0, -1_500)] {
                    let lx = wx + dx;
                    let lz = wz + dz;
                    if atlas.kind_at(lx, lz).open() {
                        return Some((wx, wz, lx, lz));
                    }
                }
            }
            col += 16;
        }
        row += 16;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test terrain levelled, so slope and relief add no trees.
    fn flat() -> Terrain {
        let mut world = tore_world::test_support::terrain();
        for cell in &mut world.theater.cells {
            cell.elevation = 0;
        }
        world
    }

    fn solid(index: u8) -> Vec<u8> {
        vec![index; TEXELS * TEXELS]
    }

    fn placed(atlas: Atlas, eye: [f64; 2], density: f32) -> Vec<Inst> {
        let world = flat();
        let mut placement = Placement::new(Arc::new(atlas));
        placement.set_density(density);
        assert!(placement.update(&world, eye, 3_000., 6_000., None));
        placement.objects(eye)
    }

    #[test]
    fn fade_is_a_smooth_per_instance_distance_gradient() {
        let draw = 10000.;
        assert_eq!(distance_fade(7000., 7000., draw), 1.);
        assert!((distance_fade(8500., 7000., draw) - 0.5).abs() < 1e-6);
        assert_eq!(distance_fade(10000., 7000., draw), 0.);
        let values: Vec<_> = (0..=100)
            .map(|n| distance_fade(7000. + n as f32 * 30., 7000., draw))
            .collect();
        assert!(
            values
                .windows(2)
                .all(|w| w[0] >= w[1] && w[0] - w[1] < 0.016)
        );
        // Two neighbours across a 2,048 ft patch boundary have nearly equal fade.
        assert!(
            (distance_fade(8191.9, 7000., draw) - distance_fade(8192.1, 7000., draw)).abs() < 0.001
        );
        assert!(include_str!("vegetation_cull.wgsl").contains(
            "smoothstep(params.tiers.x, max(params.tiers.x + 1.0, params.tiers.y), dist)"
        ));
    }
    #[test]
    fn detail_population_is_distance_independent_and_fully_opaque() {
        let world = flat();
        let mut placement = Placement::new(Arc::new(Atlas::synthetic(solid(10), 0)));
        placement.set_budget(4096);
        let eye = [16384., 16384.];
        assert!(placement.update(&world, eye, 4000., 8000., None));
        let original: BTreeMap<_, _> = placement.object_chunks(eye).into_iter().collect();
        for moved in [[18440., 16384.], [14000., 17000.], eye] {
            assert!(placement.update(&world, moved, 4000., 8000., None));
            for (key, objects) in placement.object_chunks(moved) {
                if let Some(before) = original.get(&key) {
                    assert_eq!(&objects, before);
                }
                for inst in objects {
                    let distance =
                        (inst.pos[0] - moved[0] as f32).hypot(inst.pos[2] - moved[1] as f32);
                    if distance <= 4000. {
                        assert_eq!(distance_fade(distance, 4000., 8000.), 1.);
                    }
                }
            }
        }
        for species in Species::ALL {
            for distance in [0., 1., 1000., 3999., 4000.] {
                assert_eq!(species.share(distance, 4000.), 1.);
                assert_eq!(distance_fade(distance, 4000., 8000.), 1.);
            }
        }
    }
    #[test]
    fn near_chunks_precede_forward_prefetch_and_cover_every_visible_point() {
        let world = flat();
        let eye = [16384., 16384.];
        let mut placement = Placement::new(Arc::new(Atlas::synthetic(solid(10), 0)));
        placement.prefetch([4096., 0.]);
        let full = 4000.;
        while !placement.update(&world, eye, full, 8000., Some(3)) {
            let available = placement.available(eye);
            for z in 4..12 {
                for x in 4..12 {
                    if chunk_distance(eye, x, z) < available {
                        assert!(placement.chunks.contains_key(&(x, z)));
                    }
                }
            }
            // Outer work cannot jump ahead of missing detail chunks.
            let center = [17408.; 2];
            if placement
                .chunks
                .keys()
                .any(|&(x, z)| chunk_distance(center, x, z) > full)
            {
                for z in 4..12 {
                    for x in 4..12 {
                        if chunk_distance(center, x, z) <= full {
                            assert!(placement.chunks.contains_key(&(x, z)));
                        }
                    }
                }
            }
        }
        assert!(placement.available(eye) >= 8000.);
    }
    #[test]
    fn a_small_budget_keeps_a_whole_ring_instead_of_a_chunk_prefix() {
        let mut placement = Placement::new(Arc::new(Atlas::synthetic(solid(10), 0)));
        placement.set_budget(4096);
        placement.update(&flat(), [16384., 16384.], 2000., 4000., None);
        let chunks = placement.object_chunks([16384., 16384.]);
        assert!(chunks.iter().map(|(_, v)| v.len()).sum::<usize>() <= 4096);
        assert!(chunks.iter().any(|((x, _), _)| *x < 8));
        assert!(chunks.iter().any(|((x, _), _)| *x > 8));
        assert!(placement.available([16384., 16384.]) > 4000.);
    }
    #[test]
    fn instance_size_distribution_is_seeded_natural_and_habitat_aware() {
        let sizes: Vec<_> = (0..10000)
            .map(|seed| instance_scale(Species::Broadleaf, seed, 0., 0.).0)
            .collect();
        let mean = sizes.iter().sum::<f32>() / sizes.len() as f32;
        assert!((mean - 1.).abs() < 0.01);
        assert!(sizes.iter().all(|s| (0.6..=1.4).contains(s)));
        assert!(sizes.iter().filter(|&&s| (0.8..1.2).contains(&s)).count() > 7000);
        assert!(sizes.iter().copied().fold(1., f32::min) < 0.65);
        assert!(sizes.iter().copied().fold(1., f32::max) > 1.35);
        for seed in 0..100 {
            let edge = instance_scale(Species::Broadleaf, seed, 0.3, 0.);
            let interior = instance_scale(Species::Broadleaf, seed, 1., 0.);
            let river = instance_scale(Species::Broadleaf, seed, 0., 1.);
            assert!(edge.0 < interior.0 && interior.0 < river.0);
            assert!((0.88..=1.12).contains(&edge.1));
            assert_eq!(edge, instance_scale(Species::Broadleaf, seed, 0.3, 0.));
        }
        let world = flat();
        let atlas = Atlas::synthetic(solid(10), 0);
        let site = atlas.site(&world, 4000., 4000.);
        let inst = atlas.instance(&site, atlas.tree(&site, 42), 0.5, 42);
        assert_eq!(inst.pos[1], sprite_ground(site.height, inst.height));
        let bush: Vec<_> = (0..1000)
            .map(|seed| instance_scale(Species::DryBush, seed, 0., 0.).0)
            .collect();
        assert!(bush.iter().any(|&s| s < 0.5) && bush.iter().any(|&s| s > 1.5));
    }

    #[test]
    fn field_plants_follow_plot_edges_and_leave_interiors_open() {
        let plants = placed(Atlas::synthetic(solid(20), 0), [4000., 4000.], 1.);
        let rows: Vec<_> = plants
            .iter()
            .filter(|p| {
                p.pos[0] > 512.
                    && p.pos[0] < 7500.
                    && p.pos[2] > 512.
                    && p.pos[2] < 7500.
                    && matches!(
                        p.species,
                        Species::Broadleaf
                            | Species::Autumn
                            | Species::Conifer
                            | Species::Dead
                            | Species::GreenBush
                    )
            })
            .collect();
        assert!(rows.len() > 40);
        for plant in rows {
            let dx = plant.pos[0].rem_euclid(1024.);
            let dz = plant.pos[2].rem_euclid(1024.);
            assert!(dx.min(1024. - dx).min(dz.min(1024. - dz)) <= 3.1);
        }
    }

    #[test]
    fn water_belts_hedges_and_mountain_tree_line() {
        let world = flat();
        let desert = Atlas::synthetic(solid(20), 1);
        let mut site = desert.site(&world, 4000., 4000.);
        site.kind = Kind::Sand;
        site.bank = 0.;
        site.bed = 0.;
        site.near_water = 1.;
        let bank = desert.tree_density(&site);
        assert!(bank > 0.65);
        assert_eq!(desert.bush(&site, 42, 1.).species, Species::GreenBush);
        site.near_water = 0.5;
        assert!(desert.tree_density(&site) < bank * 0.3);
        site.near_water = 0.;
        assert_eq!(desert.tree_density(&site), 0.);
        assert!(field_edge(Kind::Field, Kind::Grass));
        assert!(field_edge(Kind::Field, Kind::Field));
        assert!(!field_edge(Kind::Field, Kind::Road));
        assert!(!field_edge(Kind::Field, Kind::Water));
        let forest = Atlas::synthetic(solid(10), 0);
        site.kind = Kind::Forest;
        site.wood = 1.;
        site.height = 3000.;
        site.slope = 18.;
        assert!(forest.tree_density(&site) > 0.9);
        site.height = 7000.;
        assert_eq!(forest.tree_density(&site), 0.);
        assert!(forest.bush_density(&site) > 0.);
        site.height = 7800.;
        assert_eq!(forest.bush_density(&site), 0.);
        // Mapped thin delta branches must seed the coarse water-distance grid.
        let mut pixels = solid(20);
        for y in 0..256 {
            for x in 124..128 {
                pixels[y * 256 + x] = 255;
            }
        }
        let delta = Atlas::synthetic(pixels, 1);
        assert!(delta.water.dist.contains(&0));
    }

    #[test]
    fn moving_camera_builds_only_chunks_entering_the_ring() {
        let world = flat();
        let mut cache = Placement::new(Arc::new(Atlas::synthetic(solid(10), 0)));
        cache.set_budget(2048);
        let full = 3000.;
        let draw = 6000.;
        let mut eye = [4000., 4000.];
        let settle = |cache: &mut Placement, eye| {
            for _ in 0..200 {
                if cache.update(&world, eye, full, draw, Some(3)) {
                    return;
                }
            }
            panic!("stationary placement did not converge");
        };
        settle(&mut cache, eye);
        for dx in [20., 2048., 2048., -2048.] {
            let before: std::collections::BTreeSet<_> = cache.chunks.keys().copied().collect();
            let builds = cache.builds;
            eye[0] += dx;
            settle(&mut cache, eye);
            let after: std::collections::BTreeSet<_> = cache.chunks.keys().copied().collect();
            assert_eq!(
                cache.builds - builds,
                after.difference(&before).count() as u64
            );
            let builds = cache.builds;
            let generation = cache.generation;
            for _ in 0..120 {
                assert!(cache.update(&world, eye, full, draw, Some(3)));
                assert_eq!((cache.builds, cache.generation), (builds, generation));
            }
        }
        // Reversing before a ring finishes must refill evicted chunks, not
        // reuse the stale settled key from the previous camera cell.
        assert!(!cache.update(&world, [eye[0] + CHUNK as f64, eye[1]], full, draw, Some(3)));
        let builds = cache.builds;
        settle(&mut cache, eye);
        assert!(cache.builds > builds);
        assert!(cache.available(eye) >= draw);
    }

    #[test]
    fn resident_patches_are_built_once_across_camera_and_lod_changes() {
        let world = flat();
        let mut cache = Placement::new(Arc::new(Atlas::synthetic(solid(10), 0)));
        assert!(cache.update(&world, [4000., 4000.], 3000., 6000., None));
        let builds = cache.builds;
        let original = cache.chunks[&(1, 1)].objects.clone();
        assert!(cache.update(&world, [4000., 4000.], 3000., 6000., None));
        assert_eq!(builds, cache.builds);
        assert!(cache.update(&world, [4100., 4000.], 5000., 6000., None));
        assert_eq!(original, cache.chunks[&(1, 1)].objects);
        let retained = cache.chunks.len();
        let builds = cache.builds;
        assert!(cache.update(&world, [4100., 4000.], 1000., 6000., None));
        assert_eq!(builds, cache.builds);
        assert_eq!(retained, cache.chunks.len());
    }

    #[test]
    fn sparse_desert_scrub_is_small_and_dry_and_fields_stay_open() {
        let world = flat();
        let desert = Atlas::synthetic(solid(20), 1);
        let mut site = desert.site(&world, 4000., 4000.);
        site.near_water = 0.;
        site.bank = 0.;
        site.bed = 0.;
        site.gravel = 0.;
        assert_eq!(desert.tree_density(&site), 0.);
        assert!((0.01..0.02).contains(&desert.bush_density(&site)));
        let bush = desert.bush(&site, 123456, 1.);
        assert_eq!(bush.species, Species::DryBush);
        assert!(bush.height <= 4. && bush.tint[0] > bush.tint[1]);
        let temperate = Atlas::synthetic(solid(20), 0);
        site.kind = Kind::Field;
        assert_eq!(temperate.tree_density(&site), 0.);
        assert_eq!(temperate.bush_density(&site), 0.);
        site.kind = Kind::Forest;
        site.wood = 1.;
        assert!(temperate.tree_density(&site) >= 0.9);
        site.height = 0.;
        site.slope = 0.;
        let tropical = Atlas::synthetic(solid(10), 3);
        let arctic = Atlas::synthetic(solid(10), 2);
        assert!((temperate.tree_density(&site) - 0.95).abs() < 0.001);
        assert!((tropical.tree_density(&site) - 1.0).abs() < 0.001);
        assert!((arctic.tree_density(&site) - 0.4275).abs() < 0.001);
    }

    #[test]
    fn colours_sort_into_the_kinds_a_player_would_name() {
        assert_eq!(kind_of([0, 0, 0], 255, 0), Kind::Water);
        assert_eq!(kind_of([30, 40, 140], 40, 0), Kind::River);
        assert_eq!(kind_of([20, 70, 24], 12, 0), Kind::Forest);
        assert_eq!(kind_of([70, 140, 50], 14, 0), Kind::Grass);
        assert_eq!(kind_of([170, 160, 70], 16, 0), Kind::Field);
        assert_eq!(kind_of([190, 160, 90], 18, 1), Kind::Sand);
        assert_eq!(kind_of([90, 88, 86], 22, 0), Kind::Rock);
        assert_eq!(kind_of([220, 222, 224], 30, 2), Kind::Snow);
        assert_eq!(kind_of([16, 80, 20], 12, 3), Kind::Forest);
    }

    #[test]
    fn a_theaters_own_darker_greens_are_woodland_and_lighter_ones_fields() {
        let tone = Tone {
            dark: 75,
            light: 95,
            yellow: 0.7,
        };
        // The same olive reads as wood in a light theater, meadow in a dark one.
        assert_eq!(kind_toned([50, 90, 40], 1, 0, tone), Kind::Forest);
        assert_eq!(kind_toned([50, 90, 40], 1, 0, Tone::FIXED), Kind::Grass);
        assert_eq!(kind_toned([95, 120, 60], 1, 0, tone), Kind::Field);
        assert_eq!(kind_toned([60, 105, 50], 1, 0, tone), Kind::Grass);
    }

    #[test]
    fn rust_noise_matches_the_shader_hash() {
        // Values from the WGSL definitions, evaluated by hand once.
        let a = detail_hash([0, 0], 0);
        assert!((0. ..1.).contains(&a));
        assert_ne!(detail_hash([1, 0], 11), detail_hash([0, 1], 11));
        // Periodic noise wraps on its period.
        let p = detail_noise([3.25, 7.5], 8, 40);
        let q = detail_noise([11.25, 15.5], 8, 40);
        assert!((p - q).abs() < 1e-6);
        for w in [[1.0e5, 2.0e5], [4.4e5, 1.2e5]] {
            assert!((0. ..=1.).contains(&sand_sea(w)));
            assert!((0. ..=1.).contains(&wadi(w, 1.)));
        }
    }

    #[test]
    fn woodland_is_dense_fields_are_clear_and_water_and_roads_place_nothing() {
        let eye = [4_000., 4_000.];
        let on_tile =
            |i: &Inst| LightMap::key(i.pos[0].floor() as i64, i.pos[2].floor() as i64) == (0, 0);
        let trees = |list: &[Inst]| {
            list.iter()
                .filter(|i| {
                    on_tile(i)
                        && matches!(
                            i.species,
                            Species::Broadleaf | Species::Conifer | Species::Autumn | Species::Dead
                        )
                })
                .count()
        };
        let wood = placed(Atlas::synthetic(solid(10), 0), eye, 1.);
        let field = placed(Atlas::synthetic(solid(20), 0), eye, 1.);
        let meadow = placed(Atlas::synthetic(solid(12), 0), eye, 1.);
        assert!(
            trees(&wood) > 2 * trees(&field).max(1),
            "{} vs {}",
            trees(&wood),
            trees(&field)
        );
        assert!(
            trees(&wood) > 2 * trees(&meadow),
            "{} vs {}",
            trees(&wood),
            trees(&meadow)
        );
        let wet: Vec<_> = placed(Atlas::synthetic(solid(255), 0), eye, 1.)
            .into_iter()
            .filter(|i| on_tile(i))
            .collect();
        assert!(
            wet.is_empty(),
            "{} on water, first {:?}",
            wet.len(),
            wet.first()
        );
        assert!(
            placed(Atlas::synthetic(solid(40), 0), eye, 1.)
                .iter()
                .all(|i| !on_tile(i))
        );
    }

    #[test]
    fn chunk_selection_matches_original_flat_cull_input() {
        let world = flat();
        let mut placement = Placement::new(Arc::new(Atlas::synthetic(solid(10), 0)));
        for density in [0.4, 1., 1.65] {
            placement.set_density(density);
            placement.set_budget(12_000);
            for eye in [[4_000., 4_000.], [6_100., 4_900.], [9_000., 4_900.]] {
                placement.update(&world, eye, 3_000., 6_000., None);
                let legacy = placement.objects(eye);
                let streamed: Vec<_> = placement
                    .object_chunks(eye)
                    .into_iter()
                    .flat_map(|(_, v)| v)
                    .collect();
                assert!(!legacy.is_empty());
                assert_eq!(streamed, legacy);
            }
        }
    }

    #[test]
    fn the_same_view_places_the_same_objects() {
        let eye = [4_000., 4_000.];
        let a = placed(Atlas::synthetic(solid(10), 0), eye, 1.);
        let b = placed(Atlas::synthetic(solid(10), 0), eye, 1.);
        assert!(!a.is_empty());
        assert_eq!(a, b);
    }

    #[test]
    fn ultra_places_more_than_high_and_high_more_than_low() {
        let count = |density: crate::graphics::VegDensity| {
            let mut placement = Placement::new(Arc::new(Atlas::synthetic(solid(10), 0)));
            placement.set_density(density.scale());
            placement.set_budget(density.object_budget());
            let eye = [4000., 4000.];
            assert!(placement.update(&flat(), eye, 3000., 6000., None));
            placement.objects(eye).len()
        };
        let low = count(crate::graphics::VegDensity::Low);
        let high = count(crate::graphics::VegDensity::High);
        let ultra = count(crate::graphics::VegDensity::Ultra);
        assert!(ultra as f32 > high as f32 * 1.3, "{ultra} vs {high}");
        assert!(high as f32 > low as f32 * 1.5, "{high} vs {low}");
    }

    #[test]
    fn a_farther_share_is_a_subset_of_a_nearer_one() {
        let atlas = Atlas::synthetic(solid(10), 0);
        let world = flat();
        let full = 4_000.;
        let near = atlas.chunk(&world, 1, 1, 0., full, 1.);
        let far = atlas.chunk(&world, 1, 1, 9_000., full, 1.);
        assert!(!far.is_empty() && far.len() < near.len());
        for f in &far {
            assert!(near.contains(f));
            assert!(f.rank < f.species.share(9_000., full));
        }
    }

    #[test]
    fn species_share_the_same_outer_distance_rule() {
        let atlas = Atlas::synthetic(solid(10), 0);
        let world = flat();
        let full = 4_000.;
        let near = atlas.chunk(&world, 1, 1, 0., full, 1.);
        let far = atlas.chunk(&world, 1, 1, 6_000., full, 1.);
        let count = |list: &[Inst], s: Species| list.iter().filter(|i| i.species == s).count();
        let small = [Species::Stone, Species::DryBush, Species::GreenBush];
        let trees = [Species::Broadleaf, Species::Autumn, Species::Conifer];
        let (small_near, small_far): (usize, usize) = small.iter().fold((0, 0), |(a, b), &s| {
            (a + count(&near, s), b + count(&far, s))
        });
        let (trees_near, trees_far): (usize, usize) = trees.iter().fold((0, 0), |(a, b), &s| {
            (a + count(&near, s), b + count(&far, s))
        });
        assert!(
            trees_near > 0 && small_near > 0,
            "{trees_near} trees, {small_near} small"
        );
        let tree_kept = trees_far as f32 / trees_near as f32;
        let small_kept = small_far as f32 / small_near.max(1) as f32;
        assert!(
            (small_kept - tree_kept).abs() < 0.1,
            "small {small_kept} vs trees {tree_kept}"
        );
    }

    #[test]
    fn grass_covers_meadow_and_not_water() {
        let world = flat();
        let meadow = Atlas::synthetic(solid(12), 0);
        let water = Atlas::synthetic(solid(255), 0);
        let mut a = Placement::new(Arc::new(meadow));
        let mut b = Placement::new(Arc::new(water));
        let eye = [4_000., 4_000.];
        let (grass, done) = a.grass(&world, eye, 1_500., None);
        assert!(done);
        // Every 32 ft patch within the radius is covered.
        let full = std::f32::consts::PI * 1_500f32.powi(2) / (PATCH * PATCH) as f32;
        assert!(grass.len() as f32 > full * 0.9, "{} of {full}", grass.len());
        assert!(
            b.grass(&world, eye, 1_500., None)
                .0
                .iter()
                .all(|p| { LightMap::key(p.pos[0] as i64, p.pos[2] as i64) != (0, 0) })
        );
    }

    #[test]
    fn every_theater_has_a_region() {
        for code in tore_formats::quick_template::tables::THEATERS {
            assert_eq!(region(code).code, code);
        }
    }
}
