//! Exploratory theater vegetation (2026-10-16). Presentation only: nothing
//! here is collidable, and nothing is written to the simulation, the wire,
//! a replay or a checkpoint. `TORE_VEGETATION=0` builds none of it. Unset
//! or `1` builds it. Placement is a function of the theater and the tile,
//! so every machine and every replay of a view shows the same scene.
//!
//! Ground kinds come from the retail tile's reference-palette colour, the
//! same way the city fill reads light roofs. The T2 class byte is not a
//! biome. Cities, streets, runways, roads and water stay clear.

use std::collections::BTreeMap;
use tore_formats::theater::Environment;
use tore_world::{
    city::mask::{self, GREEN, LightMap, SITE, STREET, WATER},
    resources::ResourceSource,
    terrain::Terrain,
};

/// Feet in a nautical mile, matching the city fill.
pub const NM: f64 = 6_076.115_49;
const TILE: i64 = mask::TILE_FT;
const CELL: i64 = TILE / 4;
const TEXELS: usize = 256;

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

/// Fitted climates (2026-10-16), from each theater's region. The retail
/// files have no climate field. Humidity in `tore_world::climate` is the
/// same grouping: arid theaters are desert, Kurils arctic, Vietnam, Cuba
/// and Panama tropical, the rest temperate. Tree mix and cover are agent
/// choices on top of that.
pub const REGIONS: [Region; 16] = [
    Region {
        code: "UKR",
        name: "Ukraine",
        climate: 0,
        trees: 0,
        cover: 1.0,
    },
    Region {
        code: "FRA",
        name: "France",
        climate: 0,
        trees: 0,
        cover: 1.0,
    },
    Region {
        code: "BAL",
        name: "Baltics",
        climate: 0,
        trees: 1,
        cover: 0.9,
    },
    Region {
        code: "GRE",
        name: "Greece",
        climate: 0,
        trees: 0,
        cover: 0.75,
    },
    Region {
        code: "NSK",
        name: "South Korea",
        climate: 0,
        trees: 1,
        cover: 1.0,
    },
    Region {
        code: "VLA",
        name: "Vladivostok",
        climate: 0,
        trees: 1,
        cover: 1.05,
    },
    Region {
        code: "LFA",
        name: "Falklands",
        climate: 0,
        trees: 1,
        cover: 0.4,
    },
    Region {
        code: "EGY",
        name: "Egypt",
        climate: 1,
        trees: 0,
        cover: 0.7,
    },
    Region {
        code: "IRA",
        name: "Iraq",
        climate: 1,
        trees: 0,
        cover: 0.7,
    },
    Region {
        code: "PGU",
        name: "Persian Gulf",
        climate: 1,
        trees: 0,
        cover: 0.65,
    },
    Region {
        code: "SPA",
        name: "Pakistan",
        climate: 1,
        trees: 0,
        cover: 0.75,
    },
    Region {
        code: "KURILE",
        name: "Kuril Islands",
        climate: 2,
        trees: 1,
        cover: 0.35,
    },
    Region {
        code: "TVIET",
        name: "North Vietnam",
        climate: 3,
        trees: 2,
        cover: 1.35,
    },
    Region {
        code: "CUB",
        name: "Cuba",
        climate: 3,
        trees: 2,
        cover: 1.2,
    },
    Region {
        code: "APA",
        name: "Panama",
        climate: 3,
        trees: 2,
        cover: 1.3,
    },
    Region {
        code: "WTA",
        name: "Taiwan",
        climate: 3,
        trees: 0,
        cover: 1.1,
    },
];

pub fn region(code: &str) -> Region {
    let base = tore_formats::theater::base_theater(code).unwrap_or(code);
    REGIONS
        .into_iter()
        .find(|r| r.code == base)
        .unwrap_or(Region {
            code: "UKR",
            name: "Temperate",
            climate: 0,
            trees: 0,
            cover: 1.0,
        })
}

/// Climate id for the ground-detail shader.
pub fn climate_id(layout: &str) -> u8 {
    region(layout).climate
}

fn luma(c: [u8; 3]) -> i32 {
    (i32::from(c[0]) * 299 + i32::from(c[1]) * 587 + i32::from(c[2]) * 114) / 1000
}

/// The kind a reference-palette colour is. `index` 255 is the water cutout.
pub fn kind_of(rgb: [u8; 3], index: u8, climate: u8) -> Kind {
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
    // A few points of blue still counts as green: Vladivostok's land is
    // greener than it is red, with blue only slightly above green.
    let green = g >= r + 8 && g + 12 >= b;
    // Temperate land sits in one olive band (luma about 60 to 80). Only the
    // darker tail is treated as wood; the rest is open country. The pictures
    // do not otherwise separate forest from field.
    let dark_green = green && l < 55;
    let yellow = g + 12 >= r && r >= b + 10 && (80..=205).contains(&l);
    let warm = r >= b + 20 && r + 18 >= g && g + 8 >= b && (70..=215).contains(&l);
    let grey = sat <= 24 && (40..=180).contains(&l);
    match climate {
        1 => {
            if dark_green && g >= r + 22 {
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
            if dark_green || (green && l < 145) {
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
            } else if green && l < 155 {
                Kind::Grass
            } else if yellow || (warm && sat < 55) {
                Kind::Field
            } else if grey && l < 105 {
                Kind::Rock
            } else if grey {
                Kind::Road
            } else if green {
                Kind::Grass
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

fn rotate(tu: usize, tv: usize, rotation: u8) -> (usize, usize) {
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

/// Classified ground tiles for one layout.
pub struct Atlas {
    pub region: Region,
    palette: [[u8; 3]; 256],
    pictures: Vec<Picture>,
    /// Placement key to (picture, rotation).
    placements: BTreeMap<(i32, i32), (u16, u8)>,
}

/// One placement cell: origin in feet and the cell's width.
#[derive(Clone, Copy)]
struct Cell {
    x: i64,
    z: i64,
    step: i64,
}

impl Atlas {
    pub fn build(
        resources: &dyn ResourceSource,
        terrain: &Terrain,
    ) -> crate::AppResult<Option<Self>> {
        if !enabled()? {
            return Ok(None);
        }
        Ok(Some(Self::from_environment(
            resources,
            &terrain.environment,
            &terrain.layout,
        )))
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
        Self {
            region,
            palette,
            pictures: stored,
            placements,
        }
    }

    /// A one-tile atlas for tests. `indices` is 256 square, picture space.
    #[cfg(test)]
    pub fn synthetic(indices: Vec<u8>, climate: u8) -> Self {
        Self {
            region: Region {
                code: "TEST",
                name: "Test",
                climate,
                trees: 0,
                cover: 1.0,
            },
            palette: {
                let mut p = [[0; 3]; 256];
                // Forest green, a field tan, and a road grey.
                p[10] = [40, 110, 36];
                p[20] = [180, 160, 90];
                p[40] = [150, 148, 146];
                p[255] = [0, 0, 80];
                p
            },
            pictures: vec![Picture { indices }],
            placements: BTreeMap::from([((0, 0), (0, 0))]),
        }
    }

    fn index_at(&self, x: i64, z: i64) -> Option<u8> {
        let key = LightMap::key(x, z);
        let (pic, rotation) = self.placements.get(&key)?;
        let tu = ((x - i64::from(key.0) * CELL) / mask::TEXEL_FT).clamp(0, 255) as usize;
        let tv = ((z - i64::from(key.1) * CELL) / mask::TEXEL_FT).clamp(0, 255) as usize;
        let (col, row) = rotate(tu, tv, *rotation);
        Some(self.pictures[*pic as usize].indices[row * TEXELS + col])
    }

    pub fn kind_at(&self, x: i64, z: i64) -> Kind {
        match self.index_at(x, z) {
            Some(index) => kind_of(self.palette[usize::from(index)], index, self.region.climate),
            None => bare_kind(self.region.climate),
        }
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

    /// Objects around `eye`, nearest first, capped. Deterministic for a
    /// given eye cell, distance and density.
    pub fn instances(
        &self,
        world: &Terrain,
        eye: [f64; 2],
        eye_alt: f64,
        draw_nm: f32,
        density: f32,
    ) -> Vec<Inst> {
        // Above the altitude fade there is nothing to draw.
        if eye_alt > 11_000. {
            return Vec::new();
        }
        let reach = f64::from(draw_nm) * NM;
        let scale = density * self.region.cover * if eye_alt > 5_000. { 0.55 } else { 1.0 };
        if scale <= 0. || reach < 1. {
            return Vec::new();
        }
        let step = 1_024i64;
        let x0 = ((eye[0] - reach) as i64).div_euclid(step) * step;
        let z0 = ((eye[1] - reach) as i64).div_euclid(step) * step;
        let x1 = ((eye[0] + reach) as i64).div_euclid(step) * step;
        let z1 = ((eye[1] + reach) as i64).div_euclid(step) * step;
        let mut cells = Vec::new();
        let mut x = x0;
        while x <= x1 {
            let mut z = z0;
            while z <= z1 {
                let dx = eye[0] - (x + step / 2) as f64;
                let dz = eye[1] - (z + step / 2) as f64;
                if dx * dx + dz * dz <= reach * reach {
                    cells.push((dx * dx + dz * dz, x, z));
                }
                z += step;
            }
            x += step;
        }
        cells.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut out = Vec::new();
        for (_, cx, cz) in cells {
            self.fill_cell(world, Cell { x: cx, z: cz, step }, scale, &mut out);
            if out.len() >= MAX_INSTANCES {
                break;
            }
        }
        self.fill_tufts(world, eye, scale, &mut out);
        if out.len() > MAX_INSTANCES {
            out.truncate(MAX_INSTANCES);
        }
        out
    }

    fn blocked(&self, world: &Terrain, x: i64, z: i64, kind: Kind) -> bool {
        if !kind.open() {
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

    fn fill_cell(&self, world: &Terrain, cell: Cell, scale: f32, out: &mut Vec<Inst>) {
        let Cell { x: cx, z: cz, step } = cell;
        let mid_x = cx + step / 2;
        let mid_z = cz + step / 2;
        let kind = self.kind_at(mid_x, mid_z);
        if self.blocked(world, mid_x, mid_z, kind) {
            return;
        }
        let shore = [step, 0, -step, 0]
            .iter()
            .zip([0, step, 0, -step])
            .any(|(dx, dz)| {
                matches!(
                    self.kind_at(mid_x + dx, mid_z + dz),
                    Kind::Water | Kind::River
                )
            });
        let n = |salt: u32| hash(cx as i32, cz as i32, salt);
        let mut want = match kind {
            Kind::Forest => (5.0, 3.0, 0.0, 0.15),
            Kind::Field => (0.35, 0.4, 0.0, 0.0),
            Kind::Grass => (4.5, 3.2, 0.2, 0.08),
            Kind::Sand => (0.15, 1.1, 0.7, 0.35),
            Kind::Scrub => (0.25, 1.8, 0.45, 0.4),
            Kind::Rock => (0.1, 0.3, 1.6, 0.2),
            Kind::Snow => (0.45, 0.2, 0.8, 0.05),
            _ => (0.0, 0.0, 0.0, 0.0),
        };
        if shore && kind.open() {
            want.0 += 2.2;
            want.1 += 3.0;
        }
        let trees = (want.0 * scale).round() as i32;
        let bushes = (want.1 * scale).round() as i32;
        let rocks = (want.2 * scale).round() as i32;
        let dead = (want.3 * scale).round() as i32;
        for i in 0..trees {
            self.push_object(world, cell, n(10 + i as u32), Object::Tree, out);
        }
        for i in 0..bushes {
            self.push_object(world, cell, n(40 + i as u32), Object::Bush, out);
        }
        for i in 0..rocks {
            self.push_object(world, cell, n(70 + i as u32), Object::Rock, out);
        }
        for i in 0..dead {
            self.push_object(world, cell, n(90 + i as u32), Object::Dead, out);
        }
        if kind == Kind::Sand && self.region.climate == 1 && n(3) % 5 < (scale * 2.) as u32 {
            self.push_object(world, cell, n(4), Object::Dune, out);
        }
        if kind == Kind::Field && scale >= 0.5 {
            let plot = n(1) >> 28;
            let west = hash((cx - step) as i32, cz as i32, 1) >> 28;
            if plot != west && self.kind_at(mid_x - step, mid_z) == Kind::Field {
                self.push_object(world, cell, n(2), Object::Hedge, out);
            }
        }
    }

    fn push_object(
        &self,
        world: &Terrain,
        cell: Cell,
        roll: u32,
        object: Object,
        out: &mut Vec<Inst>,
    ) {
        if out.len() >= MAX_INSTANCES {
            return;
        }
        let jx = (roll % 800) as i64 - 400;
        let jz = ((roll >> 8) % 800) as i64 - 400;
        let x = cell.x + cell.step / 2 + jx;
        let z = cell.z + cell.step / 2 + jz;
        let kind = self.kind_at(x, z);
        if self.blocked(world, x, z, kind) {
            return;
        }
        // Keep a dune off the river bank it was meant to avoid.
        if object == Object::Dune
            && matches!(kind, Kind::River | Kind::Grass | Kind::Forest | Kind::Field)
        {
            return;
        }
        let ground = self.color_at(x, z);
        let (mesh, sx, sy, color, trunk) = object.look(self.region, ground, roll);
        let y = world.height(x as f32, z as f32);
        out.push(Inst {
            x: x as f32,
            y,
            z: z as f32,
            yaw: (roll % 628) as f32 / 100.,
            scale_xz: sx,
            scale_y: sy,
            radius: sx.max(sy) * 0.65,
            kind: mesh,
            color,
            hash: (roll & 0xffff) as f32 / 65535.,
            trunk,
        });
    }

    /// Short grass only in the near ring, where it reads as speed.
    fn fill_tufts(&self, world: &Terrain, eye: [f64; 2], scale: f32, out: &mut Vec<Inst>) {
        if self.region.climate == 1 || self.region.climate == 2 {
            return;
        }
        let reach = 2_800f64;
        let step = 160i64;
        let x0 = ((eye[0] - reach) as i64).div_euclid(step) * step;
        let z0 = ((eye[1] - reach) as i64).div_euclid(step) * step;
        let mut x = x0;
        while x <= x0 + (reach as i64) * 2 {
            let mut z = z0;
            while z <= z0 + (reach as i64) * 2 {
                let mx = x + step / 2;
                let mz = z + step / 2;
                let dx = eye[0] - mx as f64;
                let dz = eye[1] - mz as f64;
                if dx * dx + dz * dz <= reach * reach {
                    let kind = self.kind_at(mx, mz);
                    if matches!(kind, Kind::Field | Kind::Grass)
                        && !self.blocked(world, mx, mz, kind)
                    {
                        let n = ((scale * 3.).round() as u32).clamp(1, 6);
                        for i in 0..n {
                            let roll = hash(x as i32, z as i32, 200 + i);
                            self.push_object(world, Cell { x, z, step }, roll, Object::Tuft, out);
                        }
                    }
                }
                z += step;
            }
            x += step;
        }
    }
}

/// Objects kept around the camera. The GPU ring is the same size.
pub const MAX_INSTANCES: usize = 24_000;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Object {
    Tree,
    Bush,
    Rock,
    Dead,
    Dune,
    Hedge,
    Tuft,
}
impl Object {
    /// Mesh slot, horizontal scale (feet), height (feet), linear colour, trunk flag.
    fn look(self, region: Region, ground: [u8; 3], roll: u32) -> (u32, f32, f32, [f32; 3], f32) {
        let vary = 0.85 + (roll % 30) as f32 / 100.;
        let leaf = |toward: [u8; 3]| lin(mix(ground, toward, 0.62));
        match self {
            Self::Tree => {
                let (slot, radius, h, toward) = match region.trees {
                    2 => (2u32, 22.0, 64.0, [30, 120, 40]),
                    1 => (0, 16.0, 62.0, [28, 70, 36]),
                    _ => {
                        if roll.is_multiple_of(5) {
                            (0, 18.0, 70.0, [28, 70, 36])
                        } else {
                            (1, 28.0, 58.0, [52, 130, 46])
                        }
                    }
                };
                (slot, radius * vary, h * vary, leaf(toward), 1.)
            }
            Self::Bush => (
                3,
                9. * vary,
                8. * vary,
                leaf(if region.climate == 1 {
                    [150, 124, 72]
                } else {
                    [40, 100, 36]
                }),
                0.,
            ),
            Self::Rock => (
                5,
                8. * vary,
                5.5 * vary,
                lin(mix(ground, [110, 104, 96], 0.45)),
                0.,
            ),
            Self::Dead => (6, 6. * vary, 32. * vary, lin([96, 82, 60]), 1.),
            Self::Dune => (
                7,
                160. * vary,
                22. * vary,
                lin(mix(ground, [214, 186, 120], 0.35)),
                0.,
            ),
            Self::Hedge => (8, 140., 11. * vary, leaf([36, 110, 32]), 0.),
            Self::Tuft => (4, 7. * vary, 11. * vary, leaf([60, 140, 48]), 0.),
        }
    }
}

/// One placed object, world feet.
#[derive(Clone, Copy, Debug)]
pub struct Inst {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub scale_xz: f32,
    pub scale_y: f32,
    pub radius: f32,
    pub kind: u32,
    pub color: [f32; 3],
    pub hash: f32,
    pub trunk: f32,
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

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| {
        (f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * t)
            .round()
            .clamp(0., 255.) as u8
    })
}

fn lin(c: [u8; 3]) -> [f32; 3] {
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
            let kind = kind_of(
                atlas.palette[usize::from(index)],
                index,
                atlas.region.climate,
            );
            counts[kind as usize] += 1;
        }
    }
    Shares {
        land,
        counts,
        pictures: atlas.pictures.len(),
        placements: atlas.placements.len(),
    }
}

/// `--vegetation-survey [THEATER ...]`: one line per theater.
pub fn survey() -> crate::AppResult<()> {
    let wanted: Vec<String> = std::env::args().skip(2).collect();
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    println!(
        "theater climate trees cover pictures placements land water field grass forest sand rock snow scrub river road"
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
            "{code} {} {} {} {:.2} {} {} {}% {}% {}% {}% {}% {}% {}% {}% {}% {}%",
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

/// A spot the preview can look at: world feet, and a heading in degrees.
pub struct Spot {
    pub x: f64,
    pub z: f64,
    pub heading: f64,
    pub label: &'static str,
}

/// A water texel and a nearby open point on the bank: water x, z, then land x, z.
fn bank(atlas: &Atlas, key: (i32, i32), pic: u16, rotation: u8) -> Option<(i64, i64, i64, i64)> {
    let indices = &atlas.pictures[pic as usize].indices;
    let mut row = 0usize;
    while row < TEXELS {
        let mut col = 0usize;
        while col < TEXELS {
            if indices[row * TEXELS + col] == 255 {
                let (tu, tv) = match rotation % 4 {
                    0 => (col, 255 - row),
                    1 => (255 - row, 255 - col),
                    2 => (255 - col, row),
                    _ => (row, col),
                };
                let wx = i64::from(key.0) * CELL + tu as i64 * mask::TEXEL_FT;
                let wz = i64::from(key.1) * CELL + tv as i64 * mask::TEXEL_FT;
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

/// Picks a textured field or sand view, a forest or dune edge, and a river.
/// Only placed pictures count: bare land is one flat colour and hides the
/// ground detail. Among equal kinds, the cell nearer the middle of the map
/// wins, so the camera is not stuck on the first tile.
pub fn spots(atlas: &Atlas, world: &Terrain) -> Vec<Spot> {
    let cols = world.theater.cols.max(2) as i64;
    let rows = world.theater.rows.max(2) as i64;
    let mid_x = cols * tore_formats::theater::CELL_FEET as i64 / 2;
    let mid_z = rows * tore_formats::theater::CELL_FEET as i64 / 2;
    // Rank, world x, world z, heading in degrees.
    let mut best: [Option<(i64, i64, i64, f64)>; 3] = [None; 3];
    for (&key, (pic, rotation)) in &atlas.placements {
        let px = i64::from(key.0) * CELL + CELL / 2;
        let pz = i64::from(key.1) * CELL + CELL / 2;
        let kind = atlas.kind_at(px, pz);
        let near = -((px - mid_x).abs() + (pz - mid_z).abs());
        let edge = atlas.kind_at(px + TILE, pz);
        let rank = |kind_score: i64| kind_score * 1_000_000_000 + near;
        let consider = |slot: usize,
                        score: i64,
                        at: (i64, i64),
                        heading: f64,
                        best: &mut [Option<(i64, i64, i64, f64)>; 3]| {
            if score <= 0 {
                return;
            }
            let ranked = rank(score);
            if best[slot].map(|(s, ..)| ranked > s).unwrap_or(true) {
                best[slot] = Some((ranked, at.0, at.1, heading));
            }
        };
        let score_open = match kind {
            Kind::Field | Kind::Grass => 3,
            Kind::Sand => 2,
            _ => 0,
        };
        let score_edge = match (kind, edge) {
            (Kind::Forest, Kind::Field | Kind::Grass) => 4,
            (Kind::Forest, _) => 2,
            (Kind::Sand, _) => 2,
            _ => 0,
        };
        consider(0, score_open, (px, pz), 35., &mut best);
        consider(1, score_edge, (px, pz), 35., &mut best);
        if let Some((wx, wz, lx, lz)) = bank(atlas, key, *pic, *rotation) {
            let heading = ((wz - lz) as f64).atan2((wx - lx) as f64).to_degrees() + 90.;
            consider(2, 3, (lx, lz), heading, &mut best);
        }
    }
    let mut out = Vec::new();
    let labels = ["open", "edge", "river"];
    for (i, spot) in best.iter().enumerate() {
        if let Some((_, px, pz, heading)) = spot {
            out.push(Spot {
                x: *px as f64,
                z: *pz as f64,
                heading: *heading,
                label: labels[i],
            });
        }
    }
    if out.is_empty() {
        out.push(Spot {
            x: mid_x as f64,
            z: mid_z as f64,
            heading: 20.,
            label: "center",
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(index: u8) -> Vec<u8> {
        vec![index; TEXELS * TEXELS]
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
    fn the_same_cell_places_the_same_objects_and_water_places_none() {
        let world = tore_world::test_support::terrain();
        let forest = Atlas::synthetic(solid(10), 0);
        // Palette index 10 is green, painted in synthetic().
        let eye = [4_000., 4_000.];
        let a = forest.instances(&world, eye, 500., 2., 1.);
        let b = forest.instances(&world, eye, 500., 2., 1.);
        assert!(!a.is_empty());
        assert_eq!(a.len(), b.len());
        assert_eq!(a[0].x.to_bits(), b[0].x.to_bits());
        assert_eq!(a[0].kind, b[0].kind);
        let on_tile = |i: &Inst| LightMap::key(i.x as i64, i.z as i64) == (0, 0);
        let water = Atlas::synthetic(solid(255), 0);
        assert!(
            water
                .instances(&world, eye, 500., 2., 1.)
                .iter()
                .all(|i| !on_tile(i))
        );
        let road = Atlas::synthetic(solid(40), 0);
        assert!(
            road.instances(&world, eye, 500., 2., 1.)
                .iter()
                .all(|i| !on_tile(i))
        );
    }

    #[test]
    fn ultra_places_more_than_low_and_altitude_places_none() {
        let world = tore_world::test_support::terrain();
        let forest = Atlas::synthetic(solid(10), 0);
        let eye = [4_000., 4_000.];
        let low = forest.instances(&world, eye, 500., 2., 0.4);
        let ultra = forest.instances(&world, eye, 500., 2., 1.65);
        assert!(ultra.len() > low.len(), "{} vs {}", ultra.len(), low.len());
        assert!(forest.instances(&world, eye, 20_000., 4., 1.).is_empty());
    }

    #[test]
    fn every_theater_has_a_region() {
        for code in tore_formats::quick_template::tables::THEATERS {
            assert_eq!(region(code).code, code);
        }
    }
}
