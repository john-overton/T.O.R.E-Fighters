//! Startup impostor sources: retail tree, bush, grove, palm and rock shapes
//! from the player's import where it has them, drawn fallbacks where it
//! does not. Retail textures are appended to the scenery's static art
//! pages at runtime; nothing retail is compiled in.
//!
//! Every mesh is normalised to a height of 1 standing on y = 0 (x east,
//! z north before the instance turns it), so an instance sets its height
//! in feet and keeps the mesh's proportions. The retail trees' flat ground
//! shadow quads are dropped. Vegetation does not cast dynamic shadows.
//!
//! Vertices are ten floats, the static art layout: position, page uv,
//! page (-1 untextured), normal, then the colour: a palette index for an
//! untextured retail face, or a negative part code for a drawn mesh
//! (`PART_*`), coloured from the instance.

use crate::vegetation::Species;
use std::{collections::BTreeMap, ops::Range};

pub const FLOATS: usize = 10;
/// Foliage, in the instance's colour.
pub const PART_LEAF: f32 = -1.;
/// Bark and dead wood.
pub const PART_WOOD: f32 = -2.;
/// Rock, in the instance's colour.
pub const PART_ROCK: f32 = -3.;

/// How a mesh is lit and coloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Leaves: lit mostly from above, so a card seen edge-on to the sun
    /// does not go black.
    Foliage,
    /// Solid: lit by its own normals. Rock and sand.
    Solid,
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub range: Range<u32>,
    /// Horizontal radius over height.
    pub radius: f32,
    pub look: Look,
    /// Recolour a retail texture to the instance's colour by its brightness
    /// (retail rock), with the texture's mean linear colour.
    pub recolour: Option<[f32; 3]>,
    /// Where it came from: a retail shape name, or "drawn".
    pub source: String,
    /// First of four indexed impostor pages, generated once at scene startup.
    pub sprite_layer: usize,
    /// Packed generated art uses one of four 128 px cells, with a 2 px gutter.
    pub sprite_tile: Option<u8>,
}

pub struct Library {
    pub vertices: Vec<f32>,
    pub meshes: Vec<Mesh>,
    /// Mesh indices for each species, in `Species::ALL` order.
    pub variants: Vec<Vec<u16>>,
}

/// How a retail shape becomes meshes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cut {
    /// The whole shape, standing on its lowest point.
    Whole,
    /// The whole shape, standing on its own origin plane (retail rock is
    /// partly buried).
    Buried,
    /// Each plant of a grove as its own mesh: faces are grouped by where
    /// they stand, and the tallest groups are kept.
    Plants,
}

/// Which retail shapes stand for each species, tried in order. A species
/// with none found uses its drawn fallback.
const RETAIL: &[(Species, &[&str], Cut)] = &[
    (Species::Broadleaf, &["TREEA.SH"], Cut::Whole),
    (Species::Autumn, &["TREEB.SH"], Cut::Whole),
    (Species::Dead, &["TREEC.SH"], Cut::Whole),
    (Species::Conifer, &["TREED.SH"], Cut::Whole),
    (Species::Palm, &["PLMGA.SH", "PLMGB.SH"], Cut::Plants),
    (
        Species::PalmGrove,
        &["PLMGA.SH", "PLMGB.SH", "PLMGC.SH"],
        Cut::Whole,
    ),
    (
        Species::Grove,
        &["GROVA.SH", "GROVB.SH", "GROVC.SH", "GROVD.SH"],
        Cut::Whole,
    ),
    (
        Species::PineGrove,
        &["GROVPA.SH", "GROVPB.SH", "GROVPC.SH"],
        Cut::Whole,
    ),
    (
        Species::DryBush,
        &["BUSH1.SH", "BUSH2.SH", "BUSH3.SH"],
        Cut::Whole,
    ),
    (Species::GreenBush, &["BUSH4.SH"], Cut::Whole),
    (Species::Stone, &["ROCK.SH"], Cut::Buried),
    (
        Species::Outcrop,
        &["ROCKA.SH", "ROCKB.SH", "ROCKC.SH", "ROCKE.SH"],
        Cut::Buried,
    ),
];

impl Library {
    /// `art` is the import's vegetation art, `resources` the theater's
    /// (its rock shapes). Textures go to `pages` after `first_layer`.
    pub fn build(
        art: &BTreeMap<String, Vec<u8>>,
        resources: &BTreeMap<String, Vec<u8>>,
        palette: &[[u8; 3]; 256],
        pages: &mut Vec<u8>,
        first_layer: impl Fn(&Vec<u8>) -> usize,
    ) -> Self {
        let mut lib = Self {
            vertices: Vec::new(),
            meshes: Vec::new(),
            variants: vec![Vec::new(); Species::ALL.len()],
        };
        let mut images = BTreeMap::new();
        for (species, names, cut) in RETAIL {
            for name in *names {
                let Some(bytes) = art.get(*name).or_else(|| resources.get(*name)) else {
                    continue;
                };
                let rock = matches!(species, Species::Stone | Species::Outcrop);
                let look = if rock { Look::Solid } else { Look::Foliage };
                match retail(
                    bytes,
                    *cut,
                    art,
                    resources,
                    palette,
                    pages,
                    &first_layer,
                    &mut images,
                ) {
                    Ok(meshes) => {
                        for (verts, radius, mean) in meshes {
                            let recolour = rock.then_some(mean);
                            lib.push(*species, verts, radius, look, recolour, name);
                        }
                    }
                    Err(e) => eprintln!("vegetation: {name} unusable: {e}"),
                }
            }
        }
        // Drawn species and fallbacks.
        for species in Species::ALL {
            let drawn: Vec<(Vec<f32>, Look)> = match species {
                Species::Broadleaf | Species::Autumn => vec![(broadleaf(), Look::Foliage)],
                Species::Dead => vec![(dead(), Look::Foliage)],
                Species::Conifer => vec![(conifer(), Look::Foliage)],
                Species::Palm => vec![(palm(), Look::Foliage)],
                Species::Grove | Species::PineGrove | Species::PalmGrove => {
                    vec![(grove(), Look::Foliage)]
                }
                Species::DryBush | Species::GreenBush => vec![(bush(), Look::Foliage)],
                Species::Acacia => vec![(acacia(1), Look::Foliage), (acacia(2), Look::Foliage)],
                Species::Stone => vec![
                    (rock(11, 0.55, 2), Look::Solid),
                    (rock(12, 0.7, 2), Look::Solid),
                ],
                Species::Boulder => vec![
                    (rock(21, 0.7, 2), Look::Solid),
                    (rock(22, 0.85, 2), Look::Solid),
                    (rock(23, 0.6, 2), Look::Solid),
                ],
                Species::Outcrop => vec![
                    (outcrop(31), Look::Solid),
                    (outcrop(32), Look::Solid),
                    (outcrop(33), Look::Solid),
                ],
            };
            // Stones keep drawn variants beside the single retail stone, for
            // variety; other species only fall back.
            let has_retail = !lib.variants[species as usize].is_empty();
            if has_retail && species != Species::Stone {
                continue;
            }
            for (verts, look) in drawn {
                let radius = radius_of(&verts);
                lib.push(species, verts, radius, look, None, "drawn");
            }
        }
        lib.bake(pages, &first_layer, palette);
        lib
    }

    /// Add the generated pack to the same indexed texture array and draw stream.
    /// No cacti are selected: none of this branch's arid theaters is New World.
    pub fn add_pack(
        &mut self,
        mode: &str,
        region: crate::vegetation::Region,
        palette: &[[u8; 3]; 256],
        pages: &mut Vec<u8>,
        first: usize,
    ) {
        if mode == "retail" {
            return;
        }
        if !matches!(mode, "pack" | "mixed") {
            eprintln!("vegetation: invalid TORE_VEGETATION_SPRITES={mode}, using mixed");
        }
        let rgba = include_bytes!("../assets/vegetation/pack.rgba");
        // Quantize once into the theater palette. The shader uses live weather
        // palette rows, local leaf tint, sunlight, fog and existing night light.
        let mut lookup = BTreeMap::new();
        for pixel in rgba.chunks_exact(4) {
            if pixel[3] < 128 {
                pages.push(255);
                continue;
            }
            let rgb: [u8; 3] = std::array::from_fn(|k| (f32::from(pixel[k]) * 0.78) as u8);
            let index = *lookup.entry(rgb).or_insert_with(|| {
                (0..255)
                    .min_by_key(|&i| {
                        (0..3)
                            .map(|k| (i32::from(palette[i][k]) - i32::from(rgb[k])).pow(2))
                            .sum::<i32>()
                    })
                    .unwrap_or(0) as u8
            });
            pages.push(index);
        }
        let tropical = region.climate == 3;
        let desert = region.climate == 1;
        let choices: &[(Species, &[usize])] = &[
            (Species::Broadleaf, if tropical { &[21] } else { &[9, 11] }),
            (Species::Autumn, &[10]),
            (Species::Conifer, &[15]),
            (Species::Palm, &[19, 20]),
            (Species::DryBush, &[3, 4, 5, 29]),
            (
                Species::GreenBush,
                if tropical {
                    &[22, 23, 24, 25]
                } else {
                    &[12, 14]
                },
            ),
            (Species::Stone, if desert { &[8] } else { &[18, 30] }),
            (Species::Boulder, if desert { &[7] } else { &[17, 27] }),
            (Species::Outcrop, if desert { &[6] } else { &[16, 26] }),
            (Species::Grove, if tropical { &[21] } else { &[9, 11] }),
            (Species::PineGrove, &[15]),
            (Species::PalmGrove, &[19, 20]),
        ];
        for &(species, ids) in choices {
            // Mixed preserves the retail conifers and grove silhouettes.
            if mode != "pack"
                && matches!(
                    species,
                    Species::Conifer | Species::Grove | Species::PineGrove | Species::PalmGrove
                )
            {
                continue;
            }
            self.variants[species as usize].clear();
            for &id in ids {
                let (name, radius) = PACK[id];
                self.variants[species as usize].push(self.meshes.len() as u16);
                self.meshes.push(Mesh {
                    range: 0..6,
                    radius,
                    look: if matches!(
                        species,
                        Species::Stone | Species::Boulder | Species::Outcrop
                    ) {
                        Look::Solid
                    } else {
                        Look::Foliage
                    },
                    recolour: if matches!(
                        species,
                        Species::Stone | Species::Boulder | Species::Outcrop
                    ) {
                        let start = pages.len() - rgba.len() / 4 + id / 4 * 65536;
                        let cell: Vec<u8> = (0..128)
                            .flat_map(|y| {
                                let row = start + (id % 4 / 2 * 128 + y) * 256 + id % 2 * 128;
                                pages[row..row + 128].iter().copied()
                            })
                            .collect();
                        indexed_mean(&cell, palette)
                    } else {
                        None
                    },
                    source: format!("pack/{name}"),
                    sprite_layer: first + id / 4,
                    sprite_tile: Some((id % 4) as u8),
                });
            }
        }
    }

    /// CPU orthographic rasterization at startup. The indexed pages share the
    /// scenery texture array, including its weather palette and transparency.
    fn bake(
        &mut self,
        pages: &mut Vec<u8>,
        first: &impl Fn(&Vec<u8>) -> usize,
        palette: &[[u8; 3]; 256],
    ) {
        for mesh in &mut self.meshes {
            mesh.sprite_layer = first(pages);
            let vertices = &self.vertices
                [mesh.range.start as usize * FLOATS..mesh.range.end as usize * FLOATS];
            for angle in 0..4 {
                let page = bake_view(vertices, mesh.radius, angle, pages, first(pages), palette);
                pages.extend(page);
            }
            if mesh.look == Look::Solid {
                // The source mesh no longer supplies colour or normals. All
                // rock sprites, including drawn fallbacks, need a brightness
                // reference or the local linear ground tint is multiplied by
                // an already dark texture, producing near-black silhouettes.
                mesh.recolour = indexed_mean(&pages[pages.len() - 4 * 65536..], palette);
            }
        }
        // All species use the same two triangles. Dimensions and page selection
        // travel with each instance, allowing a single indirect draw.
        self.vertices.clear();
        for [x, y] in [
            [-1., 0.],
            [-1., 1.],
            [1., 1.],
            [-1., 0.],
            [1., 1.],
            [1., 0.],
        ] {
            self.vertices
                .extend([x, y, 0., (x + 1.) * 0.5, 1. - y, 0., 0., 0., 1., 0.]);
        }
        for mesh in &mut self.meshes {
            mesh.range = 0..6;
        }
    }

    fn push(
        &mut self,
        species: Species,
        verts: Vec<f32>,
        radius: f32,
        look: Look,
        recolour: Option<[f32; 3]>,
        source: &str,
    ) {
        let start = (self.vertices.len() / FLOATS) as u32;
        self.vertices.extend(verts);
        let end = (self.vertices.len() / FLOATS) as u32;
        self.variants[species as usize].push(self.meshes.len() as u16);
        self.meshes.push(Mesh {
            range: start..end,
            radius,
            look,
            recolour,
            source: source.to_string(),
            sprite_layer: 0,
            sprite_tile: None,
        });
    }

    pub fn mesh_for(&self, species: Species, variant: u8) -> u16 {
        let list = &self.variants[species as usize];
        list[usize::from(variant) % list.len()]
    }

    /// One line per species: the meshes it draws and where they came from.
    pub fn report(&self) -> Vec<String> {
        Species::ALL
            .iter()
            .map(|s| {
                let sources: Vec<String> = self.variants[*s as usize]
                    .iter()
                    .map(|&m| {
                        let mesh = &self.meshes[usize::from(m)];
                        format!(
                            "{} ({} tris)",
                            mesh.source,
                            (mesh.range.end - mesh.range.start) / 3
                        )
                    })
                    .collect();
                format!("{s:?}: {}", sources.join(", "))
            })
            .collect()
    }
}

/// Four horizontal views retain the shape's silhouette without runtime mesh work.
fn bake_view(
    vertices: &[f32],
    radius: f32,
    angle: usize,
    pages: &[u8],
    layers: usize,
    palette: &[[u8; 3]; 256],
) -> Vec<u8> {
    let mut out = vec![255u8; 256 * 256];
    let mut depth = vec![f32::NEG_INFINITY; 256 * 256];
    let (sn, cs) = (angle as f32 * std::f32::consts::FRAC_PI_2).sin_cos();
    let radius = radius.max(0.05) * 1.02;
    // Pages may follow other scene textures; locate their logical layer zero.
    let offset = layers * 65536 - pages.len();
    let nearest = |rgb: [u8; 3]| -> u8 {
        (0..255)
            .min_by_key(|&i| {
                (0..3)
                    .map(|k| {
                        let d = i32::from(palette[i][k]) - i32::from(rgb[k]);
                        d * d
                    })
                    .sum::<i32>()
            })
            .unwrap_or(0) as u8
    };
    let parts = [
        nearest([53, 89, 43]),
        nearest([78, 61, 45]),
        nearest([111, 91, 72]),
    ];
    for tri in vertices.chunks_exact(FLOATS * 3) {
        let v: Vec<&[f32]> = tri.chunks_exact(FLOATS).collect();
        let p: Vec<[f32; 3]> = v
            .iter()
            .map(|v| {
                [
                    ((v[0] * cs - v[2] * sn) / radius * 0.5 + 0.5) * 254. + 1.,
                    (1. - v[1]) * 254. + 1.,
                    v[0] * sn + v[2] * cs,
                ]
            })
            .collect();
        let edge = |a: [f32; 3], b: [f32; 3], x: f32, y: f32| {
            (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
        };
        let area = edge(p[0], p[1], p[2][0], p[2][1]);
        if area.abs() < 0.0001 {
            continue;
        }
        let bounds = |axis: usize| {
            let lo = p
                .iter()
                .map(|v| v[axis])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .clamp(0., 255.) as usize;
            let hi = p
                .iter()
                .map(|v| v[axis])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .clamp(0., 255.) as usize;
            lo..=hi
        };
        for y in bounds(1) {
            for x in bounds(0) {
                let a = edge(p[1], p[2], x as f32 + 0.5, y as f32 + 0.5) / area;
                let b = edge(p[2], p[0], x as f32 + 0.5, y as f32 + 0.5) / area;
                let c = 1. - a - b;
                if a < -0.0001 || b < -0.0001 || c < -0.0001 {
                    continue;
                }
                let blend = |k: usize| a * v[0][k] + b * v[1][k] + c * v[2][k];
                let z = a * p[0][2] + b * p[1][2] + c * p[2][2];
                let at = y * 256 + x;
                if z <= depth[at] {
                    continue;
                }
                let index = if v[0][5] >= 0. {
                    let u = (blend(3) * 256.).floor().clamp(0., 255.) as usize;
                    let w = (blend(4) * 256.).floor().clamp(0., 255.) as usize;
                    let address = v[0][5] as usize * 65536 + w * 256 + u;
                    address
                        .checked_sub(offset)
                        .and_then(|i| pages.get(i))
                        .copied()
                        .unwrap_or(255)
                } else if v[0][9] < 0. {
                    parts[(-v[0][9] - 1.) as usize % 3]
                } else {
                    v[0][9] as u8
                };
                if index != 255 {
                    out[at] = index;
                    depth[at] = z;
                }
            }
        }
    }
    out
}

/// Mean visible linear albedo of an actual baked sprite, not its source sheet.
fn indexed_mean(pixels: &[u8], palette: &[[u8; 3]; 256]) -> Option<[f32; 3]> {
    let mut sum = [0.; 3];
    let mut count = 0.;
    for &index in pixels {
        if index == 255 {
            continue;
        }
        let rgb = crate::vegetation::lin(palette[usize::from(index)]);
        for k in 0..3 {
            sum[k] += rgb[k];
        }
        count += 1.;
    }
    if count == 0. {
        return None;
    }
    let mean = sum.map(|x| x / count);
    (mean.iter().sum::<f32>() > 0.0001).then_some(mean)
}

type Images = BTreeMap<String, Option<(crate::static_art::Image, [f32; 3])>>;

type Built = (Vec<f32>, f32, [f32; 3]);

/// A retail shape as normalised meshes: vertices, radius over height, and
/// the mean linear colour of its textures.
#[allow(clippy::too_many_arguments)]
fn retail(
    bytes: &[u8],
    cut: Cut,
    art: &BTreeMap<String, Vec<u8>>,
    resources: &BTreeMap<String, Vec<u8>>,
    palette: &[[u8; 3]; 256],
    pages: &mut Vec<u8>,
    first_layer: &impl Fn(&Vec<u8>) -> usize,
    images: &mut Images,
) -> crate::AppResult<Vec<Built>> {
    let shape = tore_formats::shape::Shape::scenery(bytes)?;
    if shape.faces.is_empty() {
        return Ok(Vec::new());
    }
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in shape.faces.iter().flat_map(|f| &f.positions) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let full = (hi[2] - lo[2]).max(1e-3);
    // A flat face at the very bottom is a painted ground shadow.
    let ground =
        |f: &tore_formats::shape::Face| f.positions.iter().all(|p| p[2] <= lo[2] + full * 0.03);
    let kept: Vec<&tore_formats::shape::Face> = shape
        .faces
        .iter()
        .filter(|f| f.positions.len() >= 3 && !ground(f))
        .collect();
    if kept.is_empty() {
        return Ok(Vec::new());
    }
    let groups = if cut == Cut::Plants {
        plants(&kept, hi[2])
    } else {
        vec![kept]
    };
    let mut out = Vec::new();
    for group in groups {
        if let Some(mesh) = normalise(
            &group,
            cut,
            art,
            resources,
            palette,
            pages,
            first_layer,
            images,
        )? {
            out.push(mesh);
        }
    }
    Ok(out)
}

/// Splits a grove into plants: faces whose centres stand within 18 units
/// of each other belong together. Keeps the three largest plants that
/// reach at least 60 percent of the grove's height.
fn plants<'a>(
    faces: &[&'a tore_formats::shape::Face],
    top: f32,
) -> Vec<Vec<&'a tore_formats::shape::Face>> {
    let centre = |f: &tore_formats::shape::Face| {
        let n = f.positions.len() as f32;
        let s = f
            .positions
            .iter()
            .fold([0f32; 2], |a, p| [a[0] + p[0], a[1] + p[1]]);
        [s[0] / n, s[1] / n]
    };
    let centres: Vec<[f32; 2]> = faces.iter().map(|f| centre(f)).collect();
    let mut parent: Vec<usize> = (0..faces.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for i in 0..faces.len() {
        for j in i + 1..faces.len() {
            let d = (centres[i][0] - centres[j][0]).hypot(centres[i][1] - centres[j][1]);
            if d < 18. {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<&tore_formats::shape::Face>> = BTreeMap::new();
    for (i, f) in faces.iter().enumerate() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(f);
    }
    let mut list: Vec<Vec<&tore_formats::shape::Face>> = groups
        .into_values()
        .filter(|g| {
            let t = g
                .iter()
                .flat_map(|f| &f.positions)
                .map(|p| p[2])
                .fold(f32::NEG_INFINITY, f32::max);
            t >= top * 0.6 && g.len() >= 3
        })
        .collect();
    list.sort_by_key(|g| std::cmp::Reverse(g.len()));
    list.truncate(3);
    list
}

#[allow(clippy::too_many_arguments)]
fn normalise(
    kept: &[&tore_formats::shape::Face],
    cut: Cut,
    art: &BTreeMap<String, Vec<u8>>,
    resources: &BTreeMap<String, Vec<u8>>,
    palette: &[[u8; 3]; 256],
    pages: &mut Vec<u8>,
    first_layer: &impl Fn(&Vec<u8>) -> usize,
    images: &mut Images,
) -> crate::AppResult<Option<Built>> {
    let mut base = f32::INFINITY;
    let mut top = f32::NEG_INFINITY;
    let mut xl = f32::INFINITY;
    let mut xh = f32::NEG_INFINITY;
    let mut yl = f32::INFINITY;
    let mut yh = f32::NEG_INFINITY;
    for p in kept.iter().flat_map(|f| &f.positions) {
        base = base.min(p[2]);
        top = top.max(p[2]);
        xl = xl.min(p[0]);
        xh = xh.max(p[0]);
        yl = yl.min(p[1]);
        yh = yh.max(p[1]);
    }
    if cut == Cut::Buried {
        base = 0.;
    }
    let height = (top - base).max(1e-3);
    let cx = (xl + xh) * 0.5;
    let cy = (yl + yh) * 0.5;
    let s = 1. / height;
    let mut out = Vec::new();
    let mut mean = [0f32; 4];
    let mut radius = 0f32;
    for &face in kept {
        let image = if face.texture.is_empty() || face.uv.is_empty() {
            None
        } else {
            let name = face.texture.to_ascii_uppercase();
            if !images.contains_key(&name) {
                let loaded = match art.get(&name).or_else(|| resources.get(&name)) {
                    Some(bytes) => {
                        let pic = tore_formats::Pic::parse(bytes)?;
                        let mut sum = [0f32; 4];
                        for (p, m) in pic.pixels.iter().zip(&pic.mask) {
                            if *m && *p != 255 {
                                let c = crate::vegetation::lin(palette[usize::from(*p)]);
                                sum[0] += c[0];
                                sum[1] += c[1];
                                sum[2] += c[2];
                                sum[3] += 1.;
                            }
                        }
                        let n = sum[3].max(1.);
                        let first = first_layer(pages);
                        let image = crate::static_art::Image::append(&pic, pages, first)?;
                        Some((image, [sum[0] / n, sum[1] / n, sum[2] / n]))
                    }
                    None => None,
                };
                images.insert(name.clone(), loaded);
            }
            images[&name]
        };
        if !face.texture.is_empty() && image.is_none() {
            return Err(format!("missing vegetation cutout {}", face.texture).into());
        }
        let local = |p: [f32; 3]| [(p[0] - cx) * s, (p[2] - base) * s, (p[1] - cy) * s];
        let a = local(face.positions[0]);
        let mut normal = [0f32; 3];
        for w in face.positions[1..].windows(2) {
            let b = local(w[0]);
            let c = local(w[1]);
            let n = cross(sub(b, a), sub(c, a));
            normal = [normal[0] + n[0], normal[1] + n[1], normal[2] + n[2]];
        }
        let normal = normalize(normal);
        for triangle in 1..face.positions.len() - 1 {
            let mut points = [[0f32; FLOATS]; 3];
            for (slot, i) in [0, triangle, triangle + 1].into_iter().enumerate() {
                let p = local(face.positions[i]);
                radius = radius.max(p[0].hypot(p[2]));
                let uv = face.uv.get(i).copied().unwrap_or([0.; 2]);
                let uv = image.map_or([0., 0.], |(image, _)| {
                    [uv[0] + 0.5, image.height as f32 - 0.5 - uv[1]]
                });
                points[slot] = [
                    p[0],
                    p[1],
                    p[2],
                    uv[0],
                    uv[1],
                    -1.,
                    normal[0],
                    normal[1],
                    normal[2],
                    f32::from(face.colors.get(i).copied().unwrap_or(0)),
                ];
            }
            match image {
                Some((image, m)) => {
                    image.triangle(points, &mut out, usize::MAX)?;
                    mean = [mean[0] + m[0], mean[1] + m[1], mean[2] + m[2], mean[3] + 1.];
                }
                None => out.extend(points.into_iter().flatten()),
            }
        }
    }
    let n = mean[3].max(1.);
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some((
        out,
        radius.max(0.05),
        [mean[0] / n, mean[1] / n, mean[2] / n],
    )))
}

fn radius_of(verts: &[f32]) -> f32 {
    verts
        .chunks_exact(FLOATS)
        .map(|v| v[0].hypot(v[2]))
        .fold(0.05, f32::max)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(u: [f32; 3], v: [f32; 3]) -> [f32; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

fn normalize(n: [f32; 3]) -> [f32; 3] {
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-8 {
        [0., 1., 0.]
    } else {
        [n[0] / l, n[1] / l, n[2] / l]
    }
}

/// A flat-shaded triangle in a drawn part.
fn tri(out: &mut Vec<f32>, a: [f32; 3], b: [f32; 3], c: [f32; 3], part: f32) {
    let n = normalize(cross(sub(b, a), sub(c, a)));
    for p in [a, b, c] {
        out.extend([p[0], p[1], p[2], 0., 0., -1., n[0], n[1], n[2], part]);
    }
}

/// Small deterministic noise for drawn shapes.
fn jitter(seed: u32, i: u32) -> f32 {
    let mut n = seed.wrapping_mul(0x9e37_79b1) ^ i.wrapping_mul(0x85eb_ca6b);
    n ^= n >> 15;
    n = n.wrapping_mul(0x2c1b_3c6d);
    n ^= n >> 12;
    (n & 0xffff) as f32 / 65_535.
}

fn trunk(out: &mut Vec<f32>, r0: f32, r1: f32, y0: f32, y1: f32, lean: [f32; 2], sides: u32) {
    for i in 0..sides {
        let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let p = |a: f32, r: f32, y: f32, t: f32| {
            [a.cos() * r + lean[0] * t, y, a.sin() * r + lean[1] * t]
        };
        let (b0, b1, t0, t1) = (
            p(a0, r0, y0, 0.),
            p(a1, r0, y0, 0.),
            p(a0, r1, y1, 1.),
            p(a1, r1, y1, 1.),
        );
        tri(out, b0, t0, b1, PART_WOOD);
        tri(out, b1, t0, t1, PART_WOOD);
    }
}

/// A lumpy foliage ball: an octahedron subdivided once, pushed out by noise.
fn blob(out: &mut Vec<f32>, center: [f32; 3], radius: [f32; 3], seed: u32, part: f32) {
    let shell = sphere(seed, 0.22);
    for t in shell.chunks_exact(3) {
        let p = |v: [f32; 3]| {
            [
                center[0] + v[0] * radius[0],
                center[1] + v[1] * radius[1],
                center[2] + v[2] * radius[2],
            ]
        };
        tri(out, p(t[0]), p(t[1]), p(t[2]), part);
    }
}

/// Eight source triangles from an octahedron, each vertex
/// pushed in or out by up to `rough`, deterministic by `seed`.
fn sphere(seed: u32, rough: f32) -> Vec<[f32; 3]> {
    let v = [
        [1., 0., 0.],
        [-1., 0., 0.],
        [0., 1., 0.],
        [0., -1., 0.],
        [0., 0., 1.],
        [0., 0., -1.],
    ];
    let faces = [
        (0, 2, 4),
        (4, 2, 1),
        (1, 2, 5),
        (5, 2, 0),
        (4, 3, 0),
        (1, 3, 4),
        (5, 3, 1),
        (0, 3, 5),
    ];
    let tris: Vec<[[f32; 3]; 3]> = faces.iter().map(|&(a, b, c)| [v[a], v[b], v[c]]).collect();
    // Displace by a hash of the rounded direction, so shared vertices move together.
    let push = |p: [f32; 3]| {
        let key = |x: f32| ((x * 1000.).round() as i32) as u32;
        let h = jitter(
            seed,
            key(p[0]).wrapping_mul(73) ^ key(p[1]).wrapping_mul(151) ^ key(p[2]).wrapping_mul(283),
        );
        let s = 1. + (h - 0.5) * 2. * rough;
        [p[0] * s, p[1] * s, p[2] * s]
    };
    tris.into_iter().flat_map(|t| t.map(push)).collect()
}

fn broadleaf() -> Vec<f32> {
    let mut o = Vec::new();
    trunk(&mut o, 0.05, 0.035, 0., 0.5, [0., 0.], 6);
    blob(&mut o, [0., 0.66, 0.], [0.42, 0.32, 0.42], 1, PART_LEAF);
    blob(&mut o, [0.18, 0.56, 0.1], [0.28, 0.24, 0.28], 2, PART_LEAF);
    blob(&mut o, [-0.16, 0.6, -0.12], [0.3, 0.26, 0.3], 3, PART_LEAF);
    blob(&mut o, [0.02, 0.84, 0.02], [0.26, 0.17, 0.26], 4, PART_LEAF);
    o
}

fn conifer() -> Vec<f32> {
    let mut o = Vec::new();
    trunk(&mut o, 0.035, 0.02, 0., 0.3, [0., 0.], 5);
    for (y0, y1, r) in [(0.14, 0.52, 0.27), (0.34, 0.74, 0.21), (0.56, 1.0, 0.14)] {
        let sides = 8;
        for i in 0..sides {
            let a0 = i as f32 / sides as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / sides as f32 * std::f32::consts::TAU;
            let rr = |k: u32| r * (0.85 + 0.3 * jitter(7, k + (y0 * 100.) as u32));
            let p0 = [a0.cos() * rr(i), y0, a0.sin() * rr(i)];
            let p1 = [a1.cos() * rr(i + 1), y0, a1.sin() * rr(i + 1)];
            tri(&mut o, [0., y1, 0.], p1, p0, PART_LEAF);
            tri(&mut o, p0, p1, [0., y0 + 0.02, 0.], PART_LEAF);
        }
    }
    o
}

fn palm() -> Vec<f32> {
    let mut o = Vec::new();
    let lean = [0.06, 0.02];
    trunk(&mut o, 0.03, 0.022, 0., 0.86, lean, 6);
    let top = [lean[0], 0.86, lean[1]];
    for i in 0..9 {
        let a = i as f32 / 9. * std::f32::consts::TAU + jitter(5, i) * 0.4;
        let (s, c) = a.sin_cos();
        let reach = 0.36 + 0.08 * jitter(6, i);
        let mid = [
            top[0] + c * reach * 0.5,
            top[1] + 0.07,
            top[2] + s * reach * 0.5,
        ];
        let tip = [top[0] + c * reach, top[1] - 0.16, top[2] + s * reach];
        let side = [-s * 0.07, 0., c * 0.07];
        let l = [mid[0] + side[0], mid[1], mid[2] + side[2]];
        let r = [mid[0] - side[0], mid[1], mid[2] - side[2]];
        tri(&mut o, top, l, r, PART_LEAF);
        tri(&mut o, l, tip, r, PART_LEAF);
    }
    o
}

fn dead() -> Vec<f32> {
    let mut o = Vec::new();
    trunk(&mut o, 0.04, 0.02, 0., 0.85, [0.03, 0.], 5);
    for (i, (y, a, l)) in [
        (0.45, 0.4, 0.35),
        (0.6, 2.6, 0.3),
        (0.72, 4.4, 0.25),
        (0.35, 5.5, 0.2),
    ]
    .into_iter()
    .enumerate()
    {
        let (s, c) = (a as f32).sin_cos();
        let from = [0.015 * i as f32, y, 0.];
        let to = [c * l, y + l * 0.55, s * l];
        let w = [-s * 0.012, 0., c * 0.012];
        tri(
            &mut o,
            [from[0] + w[0], from[1], from[2] + w[2]],
            to,
            [from[0] - w[0], from[1], from[2] - w[2]],
            PART_WOOD,
        );
        tri(
            &mut o,
            [from[0], from[1] + 0.02, from[2]],
            to,
            [from[0], from[1] - 0.02, from[2]],
            PART_WOOD,
        );
    }
    o
}

/// Flat-topped acacia: a short forked trunk under a wide, thin canopy.
fn acacia(seed: u32) -> Vec<f32> {
    let mut o = Vec::new();
    trunk(&mut o, 0.04, 0.03, 0., 0.45, [0., 0.], 5);
    for k in 0..3 {
        let a = k as f32 * 2.1 + jitter(seed, k);
        let d = [a.cos() * 0.35, a.sin() * 0.35];
        trunk(&mut o, 0.025, 0.015, 0.4, 0.8, d, 4);
    }
    let sides = 12;
    let rim = |i: u32| {
        let a = i as f32 / sides as f32 * std::f32::consts::TAU;
        let r = 0.75 + 0.3 * jitter(seed + 9, i % sides);
        [
            a.cos() * r,
            0.8 + 0.06 * jitter(seed + 3, i % sides),
            a.sin() * r,
        ]
    };
    for i in 0..sides {
        let (p, q) = (rim(i), rim(i + 1));
        tri(&mut o, [0., 1.0, 0.], q, p, PART_LEAF);
        tri(&mut o, p, q, [0., 0.74, 0.], PART_LEAF);
    }
    o
}

fn bush() -> Vec<f32> {
    let mut o = Vec::new();
    blob(&mut o, [0., 0.42, 0.], [0.7, 0.45, 0.7], 41, PART_LEAF);
    blob(&mut o, [0.35, 0.3, 0.2], [0.45, 0.32, 0.45], 42, PART_LEAF);
    fit(&mut o, 0.);
    o
}

fn grove() -> Vec<f32> {
    let mut o = Vec::new();
    for (i, (x, z, h)) in [
        (0., 0., 1.),
        (1.1, 0.4, 0.85),
        (-0.9, 0.7, 0.9),
        (0.4, -1.0, 0.8),
        (-0.6, -0.8, 0.95),
    ]
    .into_iter()
    .enumerate()
    {
        trunk(&mut o, 0.05, 0.035, 0., 0.45 * h, [x, z], 5);
        let tree = |c: [f32; 3], r: [f32; 3], s: u32, o: &mut Vec<f32>| blob(o, c, r, s, PART_LEAF);
        tree(
            [x, 0.66 * h, z],
            [0.5, 0.34 * h, 0.5],
            60 + i as u32,
            &mut o,
        );
    }
    o
}

/// A boulder or stone: a rough sphere flattened and sunk a little into the
/// ground, `squash` its height over its width.
fn rock(seed: u32, squash: f32, _detail: u32) -> Vec<f32> {
    let mut o = Vec::new();
    let shell = sphere(seed, 0.28);
    let stretch = 0.8 + 0.4 * jitter(seed, 99);
    for t in shell.chunks_exact(3) {
        let p = |v: [f32; 3]| {
            // Flatter underneath, so it sits.
            let y = if v[1] < 0. { v[1] * 0.4 } else { v[1] };
            [
                v[0] * 0.5 / squash * stretch,
                (y + 0.3) / 1.3,
                v[2] * 0.5 / squash,
            ]
        };
        tri(&mut o, p(t[0]), p(t[1]), p(t[2]), PART_ROCK);
    }
    fit(&mut o, 0.12);
    o
}

/// Rescales a flat-shaded drawn mesh's heights to run from `-sink` to 1,
/// recomputing its normals.
fn fit(verts: &mut [f32], sink: f32) {
    let (lo, hi) = verts
        .chunks_exact(FLOATS)
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v[1]), hi.max(v[1]))
        });
    let span = (hi - lo).max(1e-4);
    for v in verts.chunks_exact_mut(FLOATS) {
        v[1] = (v[1] - lo) / span * (1. + sink) - sink;
    }
    for t in verts.chunks_exact_mut(FLOATS * 3) {
        let p = |k: usize| [t[k * FLOATS], t[k * FLOATS + 1], t[k * FLOATS + 2]];
        let n = normalize(cross(sub(p(1), p(0)), sub(p(2), p(0))));
        for k in 0..3 {
            t[k * FLOATS + 6..k * FLOATS + 9].copy_from_slice(&n);
        }
    }
}

/// A layered outcrop: two or three stacked, jittered tiers, each narrower,
/// like weathered sandstone, with a fallen block beside it.
fn outcrop(seed: u32) -> Vec<f32> {
    let mut o = Vec::new();
    let tiers = 2 + (seed % 2);
    let sides = 9;
    let mut y = -0.08;
    let mut radius = 0.9;
    for t in 0..tiers {
        let h = (1.08 / tiers as f32) * (0.8 + 0.4 * jitter(seed, t + 50));
        let ring = |i: u32, r: f32| {
            let a = i as f32 / sides as f32 * std::f32::consts::TAU;
            let rr = r * (0.7 + 0.45 * jitter(seed + t, i % sides));
            [a.cos() * rr * 1.2, a.sin() * rr]
        };
        let top_r = radius * (0.72 + 0.15 * jitter(seed, t + 70));
        for i in 0..sides {
            let (a, b) = (ring(i, radius), ring(i + 1, radius));
            let (c, d) = (ring(i, top_r), ring(i + 1, top_r));
            let y1 = (y + h).min(1.);
            tri(
                &mut o,
                [a[0], y, a[1]],
                [c[0], y1, c[1]],
                [b[0], y, b[1]],
                PART_ROCK,
            );
            tri(
                &mut o,
                [b[0], y, b[1]],
                [c[0], y1, c[1]],
                [d[0], y1, d[1]],
                PART_ROCK,
            );
            // Cap, slightly domed.
            tri(
                &mut o,
                [0., y1 + 0.03, 0.],
                [d[0], y1, d[1]],
                [c[0], y1, c[1]],
                PART_ROCK,
            );
        }
        y += h * 0.92;
        radius = top_r * 0.95;
    }
    let shell = rock(seed + 5, 0.7, 2);
    for v in shell.chunks_exact(FLOATS) {
        let mut v = v.to_vec();
        v[0] = v[0] * 0.35 + 1.15;
        v[1] *= 0.3;
        v[2] = v[2] * 0.35 + 0.3;
        o.extend(v);
    }
    o
}

include!("../assets/vegetation/pack_layout.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rock_brightness_reference_ignores_cutouts_and_keeps_ground_tint() {
        let mut palette = [[0; 3]; 256];
        palette[17] = [100, 90, 70];
        let mean = indexed_mean(&[255, 17, 17, 255], &palette).unwrap();
        assert_eq!(mean, crate::vegetation::lin(palette[17]));
        // Uniform rock texture / its reference = 1, preserving local tint.
        let local = [0.08, 0.1, 0.04];
        let restored: [f32; 3] = std::array::from_fn(|k| local[k] * mean[k] / mean[k]);
        for k in 0..3 {
            assert!((restored[k] - local[k]).abs() < 1e-6);
        }
        assert_eq!(indexed_mean(&[255, 255], &palette), None);
    }

    #[test]
    fn missing_cutout_rejects_source_instead_of_baking_black_triangles() {
        let face = tore_formats::shape::Face {
            positions: vec![[0., 0., 0.], [1., 0., 1.], [-1., 0., 1.]],
            colors: vec![0; 3],
            fog: Default::default(),
            uv: vec![[0., 0.]; 3],
            texture: "MISSING.PIC".into(),
            subtype: 0,
            normal: None,
            address: 0,
        };
        let empty = BTreeMap::new();
        let result = normalise(
            &[&face],
            Cut::Whole,
            &empty,
            &empty,
            &[[0; 3]; 256],
            &mut Vec::new(),
            &|p| p.len() / 65536,
            &mut BTreeMap::new(),
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing vegetation cutout")
        );
    }

    #[test]
    fn pack_selection_has_valid_pages_and_never_selects_new_world_cacti() {
        for mode in ["retail", "pack", "mixed"] {
            for code in ["UKR", "EGY", "PGU", "IRA", "TVIET"] {
                let mut lib = Library {
                    vertices: vec![0.; 6 * FLOATS],
                    meshes: Vec::new(),
                    variants: vec![Vec::new(); Species::ALL.len()],
                };
                let mut pages = Vec::new();
                lib.add_pack(
                    mode,
                    crate::vegetation::region(code),
                    &[[80; 3]; 256],
                    &mut pages,
                    7,
                );
                if mode == "retail" {
                    assert!(pages.is_empty());
                    continue;
                }
                assert_eq!(pages.len(), 8 * 65536);
                for mesh in &lib.meshes {
                    assert!(
                        !mesh.source.contains("cactus")
                            && !mesh.source.contains("saguaro")
                            && !mesh.source.contains("prickly")
                    );
                    let tile = usize::from(mesh.sprite_tile.unwrap());
                    let page = &pages[(mesh.sprite_layer - 7) * 65536..][..65536];
                    assert!((0..128).any(|y| {
                        (0..128)
                            .any(|x| page[(tile / 2 * 128 + y) * 256 + tile % 2 * 128 + x] != 255)
                    }));
                    assert_eq!(mesh.range, 0..6);
                }
                if mode == "mixed" {
                    assert!(lib.variants[Species::Conifer as usize].is_empty());
                }
            }
        }
    }

    #[test]
    fn side_views_select_the_surface_toward_the_camera() {
        let mut vertices = Vec::new();
        for (x, color) in [(-0.25, 21.), (0.25, 42.)] {
            for [z, y] in [
                [-0.4, 0.],
                [-0.4, 1.],
                [0.4, 1.],
                [-0.4, 0.],
                [0.4, 1.],
                [0.4, 0.],
            ] {
                vertices.extend([x, y, z, 0., 0., -1., 1., 0., 0., color]);
            }
        }
        let palette = [[0; 3]; 256];
        let east = bake_view(&vertices, 0.5, 1, &[], 0, &palette);
        let west = bake_view(&vertices, 0.5, 3, &[], 0, &palette);
        assert_eq!(east[128 * 256 + 128], 42);
        assert_eq!(west[128 * 256 + 128], 21);
    }

    #[test]
    fn impostor_preserves_indexed_cutouts_and_does_not_embed_source_pages() {
        let mut vertices = Vec::new();
        for [x, y] in [
            [-0.5, 0.],
            [-0.5, 1.],
            [0.5, 1.],
            [-0.5, 0.],
            [0.5, 1.],
            [0.5, 0.],
        ] {
            vertices.extend([x, y, 0., x + 0.5, 1. - y, 3., 0., 0., 1., 0.]);
        }
        let mut source = vec![255; 65536];
        for y in 64..192 {
            for x in 64..192 {
                source[y * 256 + x] = 17;
            }
        }
        let baked = bake_view(&vertices, 0.5, 0, &source, 4, &[[0; 3]; 256]);
        assert_eq!(baked[128 * 256 + 128], 17);
        assert_eq!(baked[10 * 256 + 10], 255);
        assert!(baked.iter().filter(|&&x| x == 17).count() > 14000);
    }

    #[test]
    fn sprite_species_selects_stable_views_and_shared_quad() {
        let mut pages = Vec::new();
        let lib = Library::build(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &[[80; 3]; 256],
            &mut pages,
            |p| p.len() / 65536,
        );
        assert_eq!(lib.vertices.len(), 6 * FLOATS);
        for species in Species::ALL {
            let id = lib.mesh_for(species, 7);
            assert!(lib.variants[species as usize].contains(&id));
            let mesh = &lib.meshes[usize::from(id)];
            assert_eq!(mesh.range, 0..6);
            for angle in 0..4 {
                let page = &pages[(mesh.sprite_layer + angle) * 65536..][..65536];
                assert!(page.iter().any(|&p| p != 255), "{species:?}, view {angle}");
            }
        }
    }

    #[test]
    fn without_an_import_every_species_has_a_drawn_mesh() {
        let mut pages = Vec::new();
        let lib = Library::build(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &[[0; 3]; 256],
            &mut pages,
            |p| p.len() / 65536,
        );
        assert_eq!(pages.len(), lib.meshes.len() * 4 * 65536);
        for s in Species::ALL {
            let m = lib.mesh_for(s, 7);
            let mesh = &lib.meshes[usize::from(m)];
            assert!(!mesh.range.is_empty(), "{s:?}");
            assert_eq!(mesh.source, "drawn");
        }
    }

    #[test]
    fn drawn_meshes_stand_on_the_ground_and_reach_height_one() {
        for verts in [
            broadleaf(),
            conifer(),
            palm(),
            acacia(1),
            bush(),
            rock(1, 0.6, 2),
            outcrop(31),
        ] {
            let (lo, hi) = verts
                .chunks_exact(FLOATS)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v[1]), hi.max(v[1]))
                });
            assert!((-0.15..=0.05).contains(&lo), "{lo}");
            assert!((0.85..=1.12).contains(&hi), "{hi}");
        }
    }
}
