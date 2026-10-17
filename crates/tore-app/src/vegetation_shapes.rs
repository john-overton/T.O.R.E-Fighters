//! Startup impostor sources: retail tree, bush, grove, palm and rock shapes
//! from the player's import, with supplied cutouts for every missing species. Retail textures are appended to the scenery's static art
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
    /// Where it came from: a retail shape name or a supplied sprite.
    pub source: String,
    /// First of four indexed impostor pages, generated once at scene startup.
    pub sprite_layer: usize,
    /// Packed generated art uses one of four 128 px cells, with a 2 px gutter.
    pub sprite_tile: Option<u8>,
    /// Encoded top-view atlas cell and its height relative to the grounded base.
    pub top: [f32; 2],
    pub mean_color: [f32; 3],
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
/// with none found uses its supplied-art fallback.
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
        lib.bake(pages, &first_layer, palette);
        lib
    }

    /// Supplied art fills every missing species, even in retail mode. No code-
    /// drawn tree, shrub or rock survives. Sahara replaces desert equivalents.
    pub fn add_pack(
        &mut self,
        mode: &str,
        region: crate::vegetation::Region,
        palette: &[[u8; 3]; 256],
        pages: &mut Vec<u8>,
        first: usize,
    ) {
        let base_start = pages.len();
        append_rgba(
            include_bytes!("../assets/vegetation/pack.rgba"),
            palette,
            pages,
        );
        let extra_first = first + 8;
        append_rgba(
            include_bytes!("../assets/vegetation/pass5/pack.rgba"),
            palette,
            pages,
        );
        let tropical = region.climate == 3;
        let desert = region.climate == 1;
        for species in Species::ALL {
            let (ids, extra): (&[usize], bool) = match species {
                Species::Dead => (&[8], true),
                Species::Acacia => (&[4, 5], true),
                Species::Palm | Species::PalmGrove if desert => (&[0, 1, 2, 3], true),
                Species::DryBush if desert => (&[7, 9, 11, 12, 14], true),
                Species::GreenBush if desert => (&[6, 13], true),
                Species::Stone if desert => (&[19, 20], true),
                Species::Boulder if desert => (&[17, 18], true),
                Species::Outcrop if desert => (&[15, 16], true),
                Species::Broadleaf | Species::Grove => {
                    (if tropical { &[21] } else { &[9, 11] }, false)
                }
                Species::Autumn => (&[10], false),
                Species::Conifer | Species::PineGrove => (&[15], false),
                Species::Palm | Species::PalmGrove => (&[19, 20], false),
                Species::DryBush => (&[3, 4, 5, 29], false),
                Species::GreenBush => (
                    if tropical {
                        &[22, 23, 24, 25]
                    } else {
                        &[12, 14]
                    },
                    false,
                ),
                Species::Stone => (&[18, 30], false),
                Species::Boulder => (&[17, 27], false),
                Species::Outcrop => (&[16, 26], false),
            };
            let retain = !self.variants[species as usize].is_empty()
                && !extra
                && (mode == "retail"
                    || (mode != "pack"
                        && matches!(
                            species,
                            Species::Conifer
                                | Species::Grove
                                | Species::PineGrove
                                | Species::PalmGrove
                        )));
            if retain {
                let top_id = 42 + ids[0];
                for &id in &self.variants[species as usize] {
                    self.meshes[usize::from(id)].top =
                        [packed_page(extra_first, top_id), canopy_height(species)];
                }
                continue;
            }
            self.variants[species as usize].clear();
            for &id in ids {
                let (name, radius) = if extra { EXTRA[id] } else { PACK[id] };
                let first_page = if extra { extra_first } else { first };
                let start = base_start + (first_page - first + id / 4) * 65536;
                let cell: Vec<_> = (0..128)
                    .flat_map(|y| {
                        let row = start + (id % 4 / 2 * 128 + y) * 256 + id % 2 * 128;
                        pages[row..row + 128].iter().copied()
                    })
                    .collect();
                let mean = indexed_mean(&cell, palette).unwrap_or([0.; 3]);
                let solid = matches!(
                    species,
                    Species::Stone | Species::Boulder | Species::Outcrop
                );
                self.variants[species as usize].push(self.meshes.len() as u16);
                self.meshes.push(Mesh {
                    range: 0..6,
                    radius,
                    look: if solid { Look::Solid } else { Look::Foliage },
                    recolour: solid.then_some(mean),
                    mean_color: mean,
                    source: format!("supplied/{name}"),
                    sprite_layer: first_page + id / 4,
                    sprite_tile: Some((id % 4) as u8),
                    top: [
                        packed_page(extra_first, if extra { id + 21 } else { id + 42 }),
                        canopy_height(species),
                    ],
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
            mesh.mean_color =
                indexed_mean(&pages[pages.len() - 4 * 65536..], palette).unwrap_or([0.; 3]);
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
        let card = self.vertices.clone();
        self.vertices.extend_from_slice(&card);
        self.vertices.extend_from_slice(&card);
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
            top: [0., 0.75],
            mean_color: [0.; 3],
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
fn packed_page(first: usize, id: usize) -> f32 {
    -((first * 4 + id + 1) as f32)
}
fn canopy_height(species: Species) -> f32 {
    match species {
        Species::Stone | Species::Boulder | Species::Outcrop => 0.65,
        Species::DryBush | Species::GreenBush => 0.65,
        _ => 0.80,
    }
}
fn append_rgba(rgba: &[u8], palette: &[[u8; 3]; 256], pages: &mut Vec<u8>) {
    let mut lookup = BTreeMap::new();
    for pixel in rgba.chunks_exact(4) {
        if pixel[3] < 128 {
            pages.push(255);
            continue;
        }
        let rgb: [u8; 3] = std::array::from_fn(|k| (f32::from(pixel[k]) * 0.78) as u8);
        pages.push(*lookup.entry(rgb).or_insert_with(|| {
            (0..255)
                .min_by_key(|&i| {
                    (0..3)
                        .map(|k| (i32::from(palette[i][k]) - i32::from(rgb[k])).pow(2))
                        .sum::<i32>()
                })
                .unwrap_or(0) as u8
        }));
    }
}
include!("../assets/vegetation/pass5/layout.rs");

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
                assert_eq!(pages.len(), 27 * 65536);
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
                assert!(!lib.variants[Species::Conifer as usize].is_empty());
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
    fn missing_imports_use_supplied_cutouts_for_every_species() {
        let mut pages = Vec::new();
        let mut lib = Library::build(
            &BTreeMap::new(),
            &BTreeMap::new(),
            &[[80; 3]; 256],
            &mut pages,
            |p| p.len() / 65536,
        );
        lib.add_pack(
            "mixed",
            crate::vegetation::region("UKR"),
            &[[80; 3]; 256],
            &mut pages,
            0,
        );
        assert_eq!(lib.vertices.len(), 18 * FLOATS);
        for species in Species::ALL {
            let mesh = &lib.meshes[usize::from(lib.mesh_for(species, 7))];
            assert!(mesh.source.starts_with("supplied/"));
            assert!(mesh.top[0] < 0.);
            assert!((0.5..1.).contains(&mesh.top[1]));
        }
    }
}
