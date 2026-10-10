//! `--surface-preview OUT_DIR`: contact sheets of the surface-unit shapes the
//! bounded SH reader draws, for review against the original game. A
//! development aid for the surface objectives round: no window opens and
//! nothing is simulated.
//!
//! Each sheet shows one shape and its damaged `_A` shape from four sides
//! through the scenery projection, the pose static placements use, with a
//! scale bar in feet. A launcher sheet shows the dynamic (state) projection
//! with each loaded-round count. Carrier sheets place each hull's island and
//! deck parts from the FA.EXE carrier table, and a fleet scene lays out the
//! Clemenceau template `~QFFLT`. Shapes, textures, the palette and the label
//! font are read straight from the user's own `FA_1.LIB` and `FA_2.LIB` (the
//! remembered media source, `TORE_GAME_DIR`, or `gameassets/`), so the sheets
//! do not depend on what the import selected.
//!
//! The picture is a small software rasterizer: orthographic, depth tested,
//! textured faces sampled from their PIC (index 255 and masked texels are
//! cut out of transparent faces and filled with the face colour on opaque
//! ones, following the shape-file guide's face switches), plain faces
//! in their palette colour, and a fixed light that darkens faces turned away
//! from it so the form reads (the game's shader also shades by the surface
//! normal). Sprites face the viewer, as in the game.
use crate::AppResult;
use std::{collections::BTreeMap, path::Path};
use tore_formats::{
    Archive, Pic,
    carrier::{CARRIERS, Carrier, Deck, flight_deck},
    font::Font,
    shape::{DAMAGED_WORD, Face, Shape, contact_offset, loaded_count_word, object_scale},
};

const TILE: [usize; 2] = [560, 400];
const DETAIL: [usize; 2] = [1600, 900];
const HEADER: usize = 58;
/// Pixels the picture centre sits below the tile centre, clear of the captions.
const DROP: f32 = 16.;
const BACKGROUND: [u8; 3] = [88, 98, 110];

/// Where a sheet's damaged row comes from.
enum Damaged {
    /// The shape has no damaged look.
    None,
    /// A separate `_A` shape, as every ship has.
    Shape(&'static str),
    /// The shape's own jump-to-damage branch (`shape::DAMAGED_WORD`), as the
    /// carrier towers carry their damaged island.
    Branch,
}

/// One sheet: a shape, its damaged look if it has one, and why it is shown.
struct Subject {
    shape: &'static str,
    damaged: Damaged,
    note: &'static str,
}

/// Shapes the reader learned in this round (slices S1 and S2).
const NEW: &[Subject] = &[
    Subject {
        shape: "KRIV.SH",
        damaged: Damaged::Shape("KRIV_A.SH"),
        note: "Krivak frigate (KRIVAK.NT)",
    },
    Subject {
        shape: "SOVR.SH",
        damaged: Damaged::Shape("SOVR_A.SH"),
        note: "Sovremennyy destroyer (SOVR.NT)",
    },
    Subject {
        shape: "SA3.SH",
        damaged: Damaged::None,
        note: "SA-3 Goa launcher (SA3.NT), no damaged shape",
    },
    Subject {
        shape: "SCD.SH",
        damaged: Damaged::None,
        note: "SCUD launcher (SCUD.NT), no damaged shape",
    },
    Subject {
        shape: "SOLDIER.SH",
        damaged: Damaged::None,
        note: "Soldier sprite (SOLDIER.NT), no damaged shape",
    },
    Subject {
        shape: "RUNNER.SH",
        damaged: Damaged::None,
        note: "Running man (RUNNER.NT), already read before this round",
    },
    Subject {
        shape: "NIMZ.SH",
        damaged: Damaged::Shape("NIMZ_A.SH"),
        note: "Eisenhower hull (NIMZ.NT), the island is ~NIMZT",
    },
    Subject {
        shape: "KITT.SH",
        damaged: Damaged::Shape("KITT_A.SH"),
        note: "Kitty Hawk hull (KITT.NT), the island is ~KITTT",
    },
    Subject {
        shape: "CLEM.SH",
        damaged: Damaged::Shape("CLEM_A.SH"),
        note: "Clemenceau hull (CLEM.NT), the island is ~CLEMT",
    },
    Subject {
        shape: "WASP.SH",
        damaged: Damaged::Shape("WASP_A.SH"),
        note: "Wasp hull (WASP.NT), the island is ~WASPT",
    },
    Subject {
        shape: "NIMZT.SH",
        damaged: Damaged::Branch,
        note: "Eisenhower island (~NIMZT.OT), damaged row is its own damage branch",
    },
    Subject {
        shape: "KITTT.SH",
        damaged: Damaged::Branch,
        note: "Kitty Hawk island (~KITTT.OT), damaged row is its own damage branch",
    },
    Subject {
        shape: "CLEMT.SH",
        damaged: Damaged::Branch,
        note: "Clemenceau island (~CLEMT.OT), damaged row is its own damage branch",
    },
    Subject {
        shape: "WASPT.SH",
        damaged: Damaged::Branch,
        note: "Wasp island (~WASPT.OT), damaged row is its own damage branch",
    },
];

/// Shapes that already projected; drawn so a before and after can be compared.
const EXISTING: [Subject; 4] = [
    Subject {
        shape: "KIEV.SH",
        damaged: Damaged::Shape("KIEV_A.SH"),
        note: "Kiev carrier, read before this round",
    },
    Subject {
        shape: "TICON.SH",
        damaged: Damaged::Shape("TICON_A.SH"),
        note: "Ticonderoga cruiser, read before this round",
    },
    Subject {
        shape: "SA6.SH",
        damaged: Damaged::None,
        note: "SA-6 launcher, read before this round",
    },
    Subject {
        shape: "ZSU23.SH",
        damaged: Damaged::None,
        note: "ZSU-23-4 Shilka, read before this round",
    },
];

/// Launchers whose rails empty as rounds leave, with each hardpoint the
/// shape asks about and the counts to draw.
const LAUNCHERS: [(&str, &[u8], &[i32]); 4] = [
    ("SA3.SH", &[0], &[0, 1, 2]),
    ("SCD.SH", &[0], &[0, 1]),
    ("CHAP.SH", &[0], &[0, 1, 2, 3, 4]),
    ("SA2.SH", &[0, 1, 2, 3, 4, 5], &[0, 1]),
];

/// A side the shape is seen from: degrees round from the bow toward the
/// right, and degrees above the horizon.
struct View {
    name: &'static str,
    azimuth: f32,
    elevation: f32,
}
const VIEWS: [View; 4] = [
    View {
        name: "bow quarter",
        azimuth: 35.,
        elevation: 25.,
    },
    View {
        name: "broadside",
        azimuth: 90.,
        elevation: 8.,
    },
    View {
        name: "stern quarter",
        azimuth: 145.,
        elevation: 25.,
    },
    View {
        name: "top",
        azimuth: 90.,
        elevation: 89.9,
    },
];

/// The user's retail archives, searched in order.
struct Media {
    archives: Vec<Archive>,
}
impl Media {
    fn open() -> AppResult<Self> {
        let path = match std::env::var_os("TORE_GAME_DIR") {
            Some(path) => path.into(),
            None => crate::assets::data_directory()
                .ok()
                .and_then(|dir| tore_import::media_source::remembered(&dir))
                .map(|(path, _)| path)
                .unwrap_or_else(|| "gameassets/fighters-anthology".into()),
        };
        let source = tore_import::media_source::MediaSource::detect(&path)?;
        Ok(Self {
            archives: vec![source.archive("FA_2.LIB")?, source.archive("FA_1.LIB")?],
        })
    }
    fn get(&self, name: &str) -> AppResult<Vec<u8>> {
        self.archives
            .iter()
            .find_map(|archive| archive.read(name).ok())
            .ok_or_else(|| format!("{name} is in neither FA_2.LIB nor FA_1.LIB").into())
    }
}

/// What a picture needs besides the shape.
struct Art {
    palette: [[u8; 3]; 256],
    textures: BTreeMap<String, Pic>,
    font: Font,
}
impl Art {
    fn texture(&mut self, media: &Media, name: &str) -> AppResult<()> {
        if !name.is_empty() && !self.textures.contains_key(name) {
            self.textures
                .insert(name.to_owned(), Pic::parse(&media.get(name)?)?);
        }
        Ok(())
    }
}

/// An RGB picture with a depth buffer.
struct Canvas {
    width: usize,
    height: usize,
    rgb: Vec<[u8; 3]>,
    depth: Vec<f32>,
}
impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            rgb: vec![BACKGROUND; width * height],
            depth: vec![f32::INFINITY; width * height],
        }
    }
    fn put(&mut self, x: i64, y: i64, color: [u8; 3]) {
        if (0..self.width as i64).contains(&x) && (0..self.height as i64).contains(&y) {
            self.rgb[y as usize * self.width + x as usize] = color;
        }
    }
    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, color: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, color);
            }
        }
    }
    /// Text in the game's WIN11 font at twice its pixel size, with a shadow.
    fn text(&mut self, font: &Font, text: &str, x: i64, y: i64, color: [u8; 3]) {
        for (offset, shade) in [(1, [16, 18, 22]), (0, color)] {
            let mut pen = x;
            for byte in text.bytes() {
                let Some(glyph) = font.glyphs.get(usize::from(byte)) else {
                    continue;
                };
                for &(gx, gy) in &glyph.pixels {
                    self.rect(
                        pen + 2 * gx as i64 + offset,
                        y + 2 * gy as i64 + offset,
                        2,
                        2,
                        shade,
                    );
                }
                pen += 2 * glyph.advance as i64;
            }
        }
    }
    fn blit(&mut self, other: &Canvas, x: usize, y: usize) {
        for row in 0..other.height {
            let at = (y + row) * self.width + x;
            self.rgb[at..at + other.width]
                .copy_from_slice(&other.rgb[row * other.width..(row + 1) * other.width]);
        }
    }
    fn png(&self, path: &Path) -> AppResult<()> {
        let rgba: Vec<u8> = self
            .rgb
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect();
        std::fs::write(
            path,
            crate::replay::png::encode_rgba(self.width as u32, self.height as u32, &rgba)?,
        )?;
        Ok(())
    }
}

/// Orthographic camera over shape positions in feet (right, forward, up).
struct Camera {
    center: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    forward: [f32; 3],
    pixels_per_foot: f32,
    size: [usize; 2],
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn unit(a: [f32; 3]) -> [f32; 3] {
    let n = dot(a, a).sqrt();
    a.map(|v| v / n)
}
impl Camera {
    fn new(view: &View, center: [f32; 3], pixels_per_foot: f32, size: [usize; 2]) -> Self {
        let (az, el) = (view.azimuth.to_radians(), view.elevation.to_radians());
        let toward_eye = [az.sin() * el.cos(), az.cos() * el.cos(), el.sin()];
        let forward = toward_eye.map(|v| -v);
        let right = unit(cross(forward, [0., 0., 1.]));
        let up = cross(right, forward);
        Self {
            center,
            right,
            up,
            forward,
            pixels_per_foot,
            size,
        }
    }
    /// Screen x, screen y (down) and depth of a point in feet.
    fn project(&self, p: [f32; 3]) -> [f32; 3] {
        let d = [0, 1, 2].map(|i| p[i] - self.center[i]);
        [
            self.size[0] as f32 / 2. + dot(d, self.right) * self.pixels_per_foot,
            self.size[1] as f32 / 2. + DROP - dot(d, self.up) * self.pixels_per_foot,
            dot(d, self.forward),
        ]
    }
}

/// A projected shape in feet, with its sprites already turned to `camera`.
fn faces_for(shape: &Shape, scale: f32, camera: &Camera) -> Vec<Face> {
    let feet = |face: &Face| Face {
        positions: face
            .positions
            .iter()
            .map(|p| p.map(|v| v * scale))
            .collect(),
        ..face.clone()
    };
    let mut faces: Vec<Face> = shape.faces.iter().map(feet).collect();
    for sprite in &shape.billboards {
        faces.push(feet(&sprite.face(camera.right, camera.up)));
    }
    faces
}

fn draw(
    canvas: &mut Canvas,
    art: &Art,
    faces: &[Face],
    lines: &[([[f32; 3]; 2], u8)],
    camera: &Camera,
) {
    let light = unit([-0.45, 0.35, 0.82]);
    for face in faces {
        if face.positions.len() < 3 {
            continue;
        }
        let p = &face.positions;
        let normal = unit(cross(
            [0, 1, 2].map(|i| p[1][i] - p[0][i]),
            [0, 1, 2].map(|i| p[2][i] - p[0][i]),
        ));
        let lit = if normal.iter().all(|v| v.is_finite()) {
            0.55 + 0.45 * dot(normal, light).abs()
        } else {
            1.
        };
        let texture = (!face.uv.is_empty())
            .then(|| art.textures.get(&face.texture))
            .flatten();
        let flat = art.palette[usize::from(face.colors[0])];
        let opaque = face.subtype & 1 != 0 || matches!(face.subtype, 0xee | 0xfe);
        let corners: Vec<[f32; 3]> = p.iter().map(|q| camera.project(*q)).collect();
        for t in 1..corners.len() - 1 {
            let tri = [0, t, t + 1];
            let [a, b, c] = tri.map(|i| corners[i]);
            let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            if area.abs() < 1e-6 {
                continue;
            }
            let x0 = a[0].min(b[0]).min(c[0]).floor().max(0.) as i64;
            let x1 = a[0]
                .max(b[0])
                .max(c[0])
                .ceil()
                .min(canvas.width as f32 - 1.) as i64;
            let y0 = a[1].min(b[1]).min(c[1]).floor().max(0.) as i64;
            let y1 = a[1]
                .max(b[1])
                .max(c[1])
                .ceil()
                .min(canvas.height as f32 - 1.) as i64;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
                    let w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
                    let w2 = 1. - w0 - w1;
                    if w0 < 0. || w1 < 0. || w2 < 0. {
                        continue;
                    }
                    let z = w0 * a[2] + w1 * b[2] + w2 * c[2];
                    let at = y as usize * canvas.width + x as usize;
                    if z >= canvas.depth[at] {
                        continue;
                    }
                    let color = match texture {
                        Some(pic) => {
                            let [ua, ub, uc] = tri.map(|i| face.uv[i]);
                            let u = w0 * ua[0] + w1 * ub[0] + w2 * uc[0];
                            let v = w0 * ua[1] + w1 * ub[1] + w2 * uc[1];
                            let col = (u.floor() as i64).clamp(0, pic.width as i64 - 1) as usize;
                            // Texture rows count up from the bottom of the PIC.
                            let v = pic.height as f32 - v;
                            let row = (v.floor() as i64).clamp(0, pic.height as i64 - 1) as usize;
                            let texel = row * pic.width + col;
                            // Index 255 and masked texels are holes. A face the
                            // shape-file guide calls opaque (switch 12, or the
                            // ee/fe combinations) shows its own colour there; a
                            // transparent one is cut out, as in the game's shader.
                            if pic.mask.get(texel) == Some(&false) || pic.pixels[texel] == 255 {
                                if !opaque {
                                    continue;
                                }
                                flat
                            } else {
                                art.palette[usize::from(pic.pixels[texel])]
                            }
                        }
                        None => flat,
                    };
                    canvas.depth[at] = z;
                    canvas.rgb[at] = color.map(|v| (f32::from(v) * lit).min(255.) as u8);
                }
            }
        }
    }
    for (line, color) in lines {
        let [a, b] = line.map(|q| camera.project(q));
        let steps = (b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil().max(1.) as usize;
        for s in 0..=steps {
            let f = s as f32 / steps as f32;
            canvas.put(
                (a[0] + (b[0] - a[0]) * f) as i64,
                (a[1] + (b[1] - a[1]) * f) as i64,
                art.palette[usize::from(*color)],
            );
        }
    }
}

/// A scale bar of a round length about a quarter of the tile wide.
fn scale_bar(canvas: &mut Canvas, font: &Font, pixels_per_foot: f32) {
    let target = canvas.width as f32 / 4. / pixels_per_foot;
    let feet = [
        1., 2., 5., 10., 20., 25., 50., 100., 200., 250., 500., 1000.,
    ]
    .into_iter()
    .min_by(|a: &f32, b: &f32| (a / target).ln().abs().total_cmp(&(b / target).ln().abs()))
    .unwrap();
    let length = (feet * pixels_per_foot).round() as i64;
    let (x, y) = (14, canvas.height as i64 - 22);
    let white = [240, 240, 240];
    canvas.rect(x, y, length, 3, white);
    canvas.rect(x, y - 6, 2, 9, white);
    canvas.rect(x + length - 2, y - 6, 2, 9, white);
    canvas.text(font, &format!("{feet} FT"), x + length + 10, y - 9, white);
}

/// The feet bounds of a shape's faces and sprites in the scenery pose.
fn bounds(shape: &Shape, scale: f32) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    let sprites = shape
        .billboards
        .iter()
        .flat_map(|b| b.face([1., 0., 0.], [0., 0., 1.]).positions);
    for p in shape
        .faces
        .iter()
        .flat_map(|f| f.positions.clone())
        .chain(sprites)
    {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i] * scale);
            hi[i] = hi[i].max(p[i] * scale);
        }
    }
    (lo, hi)
}

struct Loaded {
    name: String,
    scale: f32,
    /// The shape's ground offset in feet at the scenery scale (F2 record
    /// word +8, what FA 0x42e0c0 reads to stand an object on the ground).
    contact: f32,
    shape: Shape,
}
fn load(
    media: &Media,
    art: &mut Art,
    name: &str,
    state: Option<&BTreeMap<usize, i32>>,
) -> AppResult<Loaded> {
    let bytes = media.get(name)?;
    let shape = match state {
        Some(state) => Shape::with_state(&bytes, state),
        None => Shape::scenery(&bytes),
    }
    .map_err(|error| format!("{name}: {error}"))?;
    for texture in shape
        .faces
        .iter()
        .filter(|f| !f.uv.is_empty())
        .map(|f| f.texture.clone())
        .chain(shape.billboards.iter().map(|b| b.texture.clone()))
    {
        art.texture(media, &texture)?;
    }
    Ok(Loaded {
        name: name.to_owned(),
        scale: object_scale(&bytes)? as f32,
        contact: f32::from(contact_offset(&bytes)?.unwrap_or(0)),
        shape,
    })
}

/// One tile: `loaded` from `view`, framed by `center` and `pixels_per_foot`.
fn tile(
    art: &Art,
    loaded: &Loaded,
    view: &View,
    center: [f32; 3],
    pixels_per_foot: f32,
    caption: &str,
    size: [usize; 2],
) -> Canvas {
    let mut canvas = Canvas::new(size[0], size[1]);
    let camera = Camera::new(view, center, pixels_per_foot, size);
    let faces = faces_for(&loaded.shape, loaded.scale, &camera);
    let lines: Vec<_> = loaded
        .shape
        .lines
        .iter()
        .map(|l| (l.positions.map(|p| p.map(|v| v * loaded.scale)), l.color))
        .collect();
    draw(&mut canvas, art, &faces, &lines, &camera);
    let count = if loaded.shape.billboards.is_empty() {
        format!("{} faces", loaded.shape.faces.len())
    } else {
        format!(
            "{} faces, {} sprites",
            loaded.shape.faces.len(),
            loaded.shape.billboards.len()
        )
    };
    canvas.text(
        &art.font,
        &format!("{} {}", loaded.name, view.name),
        10,
        8,
        [255, 255, 255],
    );
    canvas.text(
        &art.font,
        &format!("{caption} {count}"),
        10,
        30,
        [210, 225, 240],
    );
    scale_bar(&mut canvas, &art.font, pixels_per_foot);
    canvas
}

/// Frame every shape in `group` alike from `view`: the first one's centre
/// and the largest scale at which all of them fit the tile.
fn frame(group: &[&Loaded], view: &View, size: [usize; 2]) -> ([f32; 3], f32) {
    let (lo, hi) = bounds(&group[0].shape, group[0].scale);
    let center = [0, 1, 2].map(|i| (lo[i] + hi[i]) / 2.);
    let camera = Camera::new(view, center, 1., size);
    let mut reach = [1e-3f32; 2];
    for loaded in group {
        for face in faces_for(&loaded.shape, loaded.scale, &camera) {
            for p in face.positions {
                let [x, y, _] = camera.project(p);
                reach[0] = reach[0].max((x - size[0] as f32 / 2.).abs());
                reach[1] = reach[1].max((y - size[1] as f32 / 2. - DROP).abs());
            }
        }
    }
    let room = [size[0] as f32 * 0.44, size[1] as f32 * 0.5 - 70.];
    (center, (room[0] / reach[0]).min(room[1] / reach[1]))
}

/// Overall length at the scenery scale and at the aircraft renderer's
/// one-third-foot convention, for the sheet header.
fn lengths(loaded: &Loaded) -> String {
    let (lo, hi) = bounds(&loaded.shape, loaded.scale);
    let length = (0..3).map(|i| hi[i] - lo[i]).fold(0., f32::max);
    format!(
        "Largest extent {length:.0} ft at the scenery scale ({:.0} ft at one third of a foot per unit, the aircraft convention)",
        length / 3.
    )
}

fn sheet(out: &Path, media: &Media, art: &mut Art, subject: &Subject, tag: &str) -> AppResult<()> {
    let main = load(media, art, subject.shape, None)?;
    let damaged = match subject.damaged {
        Damaged::None => None,
        Damaged::Shape(name) => Some(load(media, art, name, None)?),
        Damaged::Branch => {
            let state = BTreeMap::from([(DAMAGED_WORD, 1)]);
            let mut loaded = load(media, art, subject.shape, Some(&state))?;
            loaded.name = format!("{} damage branch", subject.shape);
            Some(loaded)
        }
    };
    let group: Vec<&Loaded> = std::iter::once(&main).chain(damaged.as_ref()).collect();
    let rows = group.len();
    let mut canvas = Canvas::new(TILE[0] * VIEWS.len(), HEADER + TILE[1] * rows);
    canvas.text(
        &art.font,
        &format!(
            "{}: {} ({tag}, scenery projection)",
            subject.shape, subject.note
        ),
        10,
        8,
        [255, 255, 255],
    );
    canvas.text(&art.font, &lengths(&main), 10, 32, [210, 225, 240]);
    let frames: Vec<_> = VIEWS.iter().map(|view| frame(&group, view, TILE)).collect();
    for (row, loaded) in group.iter().enumerate() {
        for (column, view) in VIEWS.iter().enumerate() {
            let (center, ppf) = frames[column];
            let caption = if row == 0 { "intact," } else { "damaged," };
            let picture = tile(art, loaded, view, center, ppf, caption, TILE);
            canvas.blit(&picture, column * TILE[0], HEADER + row * TILE[1]);
        }
        println!(
            "Surface preview: {} {} faces, {} lines, {} sprites",
            loaded.name,
            loaded.shape.faces.len(),
            loaded.shape.lines.len(),
            loaded.shape.billboards.len()
        );
    }
    let stem = subject.shape.trim_end_matches(".SH");
    let path = out.join(format!("{tag}-{stem}.png"));
    canvas.png(&path)?;
    println!("Surface preview: {}", path.display());
    // Close views of the intact and damaged shapes for detail.
    for (row, loaded) in group.iter().enumerate() {
        for view in [&VIEWS[0], &VIEWS[2]] {
            let (center, ppf) = frame(&group, view, DETAIL);
            let caption = if row == 0 { "intact," } else { "damaged," };
            let picture = tile(art, loaded, view, center, ppf, caption, DETAIL);
            let name = format!(
                "{tag}-{stem}-{}-{}.png",
                if row == 0 { "intact" } else { "damaged" },
                view.name.replace(' ', "-")
            );
            picture.png(&out.join(name))?;
        }
    }
    Ok(())
}

fn launcher_sheet(out: &Path, media: &Media, art: &mut Art) -> AppResult<()> {
    let columns = LAUNCHERS.iter().map(|l| l.2.len()).max().unwrap_or(1);
    let mut canvas = Canvas::new(TILE[0] * columns, HEADER + TILE[1] * LAUNCHERS.len());
    canvas.text(
        &art.font,
        "Launchers through the state projection, by rounds loaded on each hardpoint",
        10,
        8,
        [255, 255, 255],
    );
    for (row, (name, hardpoints, counts)) in LAUNCHERS.iter().enumerate() {
        let full: BTreeMap<_, _> = hardpoints
            .iter()
            .map(|h| (loaded_count_word(*h), i32::MAX))
            .collect();
        let reference = load(media, art, name, Some(&full))?;
        let (center, ppf) = frame(&[&reference], &VIEWS[0], TILE);
        for (column, count) in counts.iter().enumerate() {
            let state: BTreeMap<_, _> = hardpoints
                .iter()
                .map(|h| (loaded_count_word(*h), *count))
                .collect();
            let loaded = load(media, art, name, Some(&state))?;
            let caption = format!("{count} loaded,");
            let picture = tile(art, &loaded, &VIEWS[0], center, ppf, &caption, TILE);
            canvas.blit(&picture, column * TILE[0], HEADER + row * TILE[1]);
        }
    }
    let path = out.join("launchers-loaded.png");
    canvas.png(&path)?;
    println!("Surface preview: {}", path.display());
    Ok(())
}

/// Several loaded shapes as one, in feet: each turned by its heading (binary
/// angle units) about the up axis and moved by its offset in feet (right,
/// forward, up).
fn assemble(name: String, parts: &[(&Loaded, [f32; 3], i16)]) -> Loaded {
    let mut shape = Shape {
        lines: Vec::new(),
        faces: Vec::new(),
        billboards: Vec::new(),
        state_words: Default::default(),
    };
    for (loaded, offset, heading) in parts {
        let turn = f32::from(*heading) * std::f32::consts::TAU / 65536.;
        let (sin, cos) = turn.sin_cos();
        let place = |p: [f32; 3]| -> [f32; 3] {
            let p = p.map(|v| v * loaded.scale);
            [
                offset[0] + p[0] * cos + p[1] * sin,
                offset[1] + p[1] * cos - p[0] * sin,
                offset[2] + p[2],
            ]
        };
        for face in &loaded.shape.faces {
            shape.faces.push(Face {
                positions: face.positions.iter().map(|p| place(*p)).collect(),
                normal: None,
                ..face.clone()
            });
        }
        for line in &loaded.shape.lines {
            let mut line = line.clone();
            line.positions = line.positions.map(place);
            shape.lines.push(line);
        }
        for sprite in &loaded.shape.billboards {
            let mut sprite = sprite.clone();
            sprite.center = place(sprite.center);
            sprite.size = sprite.size.map(|v| v * loaded.scale);
            shape.billboards.push(sprite);
        }
    }
    Loaded {
        name,
        scale: 1.,
        contact: 0.,
        shape,
    }
}

/// A carrier hull with its parts in place, intact or damaged, in feet at the
/// scenery scale, with the intact hull's deck.
fn carrier_assembly(
    media: &Media,
    art: &mut Art,
    carrier: &Carrier,
    damaged: bool,
) -> AppResult<(Loaded, Deck)> {
    let hull = load(media, art, carrier.hull, None)?;
    let deck = flight_deck(&hull.shape).ok_or("carrier hull has no level deck")?;
    let hull = if damaged {
        load(media, art, carrier.damaged, None)?
    } else {
        hull
    };
    let deck_feet = deck.height * hull.scale;
    let mut parts = Vec::new();
    for (index, attachment) in carrier.parts.iter().enumerate() {
        let island = index + 1 == carrier.parts.len();
        let loaded = if island && damaged {
            let state = BTreeMap::from([(DAMAGED_WORD, 1)]);
            load(media, art, attachment.shape, Some(&state))?
        } else {
            load(media, art, attachment.shape, None)?
        };
        let [right, up, forward] = attachment.offset.map(f32::from);
        let lift = deck_feet + up - loaded.contact;
        parts.push((loaded, [right, forward, lift], attachment.heading));
    }
    let mut all: Vec<(&Loaded, [f32; 3], i16)> = vec![(&hull, [0.; 3], 0)];
    all.extend(parts.iter().map(|(l, o, h)| (l, *o, *h)));
    let name = if damaged {
        format!("{} with damaged island", carrier.damaged)
    } else {
        format!("{} with island and deck parts", carrier.hull)
    };
    Ok((assemble(name, &all), deck))
}

fn carrier_sheet(out: &Path, media: &Media, art: &mut Art, carrier: &Carrier) -> AppResult<()> {
    let (intact, deck) = carrier_assembly(media, art, carrier, false)?;
    let (damaged, _) = carrier_assembly(media, art, carrier, true)?;
    let group = [&intact, &damaged];
    let mut canvas = Canvas::new(TILE[0] * VIEWS.len(), HEADER + TILE[1] * 2);
    canvas.text(
        &art.font,
        &format!(
            "{}: hull, island and deck parts placed from the FA.EXE carrier table",
            carrier.note
        ),
        10,
        8,
        [255, 255, 255],
    );
    canvas.text(&art.font, &lengths(&intact), 10, 32, [210, 225, 240]);
    let frames: Vec<_> = VIEWS.iter().map(|view| frame(&group, view, TILE)).collect();
    for (row, loaded) in group.iter().enumerate() {
        for (column, view) in VIEWS.iter().enumerate() {
            let (center, ppf) = frames[column];
            let caption = if row == 0 { "intact," } else { "damaged," };
            let picture = tile(art, loaded, view, center, ppf, caption, TILE);
            canvas.blit(&picture, column * TILE[0], HEADER + row * TILE[1]);
        }
    }
    let stem = carrier.hull.trim_end_matches(".SH");
    let path = out.join(format!("carrier-{stem}.png"));
    canvas.png(&path)?;
    println!("Surface preview: {}", path.display());
    for (row, loaded) in group.iter().enumerate() {
        for view in [&VIEWS[0], &VIEWS[2], &VIEWS[3]] {
            let (center, ppf) = frame(&group, view, DETAIL);
            let caption = if row == 0 { "intact," } else { "damaged," };
            let picture = tile(art, loaded, view, center, ppf, caption, DETAIL);
            let name = format!(
                "carrier-{stem}-{}-{}.png",
                if row == 0 { "intact" } else { "damaged" },
                view.name.replace(' ', "-")
            );
            picture.png(&out.join(name))?;
        }
    }
    let scale = load(media, art, carrier.hull, None)?.scale;
    let (lo, hi) = bounds(&intact.shape, 1.);
    let outline: Vec<String> = deck
        .outline
        .iter()
        .map(|p| format!("({:.0},{:.0})", p[0], p[1]))
        .collect();
    println!(
        "Surface preview: {} deck at {:.0} units ({:.0} ft at the scenery scale, {:.0} ft at one third), {:.0} square units of level deck, outline (right,forward) in units {}",
        carrier.hull,
        deck.height,
        deck.height * scale,
        deck.height * scale / 3.,
        deck.area,
        outline.join(" ")
    );
    println!(
        "Surface preview: {} with parts spans right {:.0}..{:.0}, forward {:.0}..{:.0}, up {:.0}..{:.0} ft at the scenery scale",
        carrier.hull, lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
    );
    Ok(())
}

/// The shape a PT or NT names in its `:shape` block.
fn named_shape(text: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(text);
    let mut lines = text.lines().skip_while(|l| l.trim() != ":shape").skip(1);
    let name = lines.next()?.trim().strip_prefix("string \"")?;
    Some(name.trim_end_matches('"').to_ascii_uppercase())
}

/// The Clemenceau fleet template `~QFFLT` laid out at its recorded positions:
/// the carrier with its parts, its escorts and the aircraft parked on its
/// deck (stood on the deck by their ground offset, as the carrier's parts
/// are). Escort
/// placeholders take the first unit of the list for the template theater's
/// default enemy group; the surface round's resolution rules (W1) pick
/// among them.
fn fleet_scene(out: &Path, media: &Media, art: &mut Art) -> AppResult<()> {
    use tore_formats::quick_template::{ObjectKind, Template, tables};
    let template = Template::parse("~QFFLT.M", &media.get("~QFFLT.M")?)?;
    let theater = tables::TEMPLATES
        .iter()
        .position(|list| list.contains(&"QFFLT"))
        .ok_or("QFFLT is in no theater list")?;
    let group = tables::group_of(tables::ENEMY_NATIONALITY[theater]).ok_or("no group")?;
    let carrier = &CARRIERS[2];
    let (clem, deck) = carrier_assembly(media, art, carrier, false)?;
    let deck_feet = deck.height * load(media, art, carrier.hull, None)?.scale;
    let origin = template
        .objects
        .iter()
        .find(|o| matches!(&o.kind, ObjectKind::Named(name) if name == "CLEM.NT"))
        .ok_or("no CLEM in ~QFFLT")?
        .position;
    let mut escorts: Vec<(Loaded, [f32; 3], i16)> = Vec::new();
    let mut parked: Vec<(Loaded, [f32; 3], i16)> = Vec::new();
    for object in &template.objects {
        let offset = [
            (object.position[0] - origin[0]) as f32,
            (object.position[2] - origin[2]) as f32,
            0.,
        ];
        let resource = match (&object.kind, object.placeholder()) {
            (_, Some(placeholder)) => tables::equipment(placeholder, group)
                .and_then(|list| list.first())
                .map(|name| name.to_string()),
            (ObjectKind::Named(name), None) if name != "CLEM.NT" => Some(name.clone()),
            _ => None,
        };
        let Some(resource) = resource else { continue };
        let shape = named_shape(&media.get(&resource)?)
            .ok_or_else(|| format!("{resource} names no shape"))?;
        // Template angles are degrees; binary angle units turn the other way.
        let heading = (-object.angles[0] as f32 * 65536. / 360.) as i16;
        let loaded = load(media, art, &shape, None)?;
        if resource.ends_with(".PT") {
            let lift = deck_feet - loaded.contact;
            parked.push((loaded, [offset[0], offset[1], lift], heading));
        } else {
            escorts.push((loaded, offset, heading));
        }
    }
    let mut all: Vec<(&Loaded, [f32; 3], i16)> = vec![(&clem, [0.; 3], 0)];
    all.extend(parked.iter().map(|(l, o, h)| (l, *o, *h)));
    let close = assemble("CLEM.SH with parked aircraft".into(), &all);
    all.extend(escorts.iter().map(|(l, o, h)| (l, *o, *h)));
    let fleet = assemble("~QFFLT fleet".into(), &all);
    let top = &VIEWS[3];
    let oblique = View {
        name: "fleet quarter",
        azimuth: 35.,
        elevation: 30.,
    };
    // At fleet range the ships are a few pixels long: ring and name each one.
    let mut marks = vec![([0f32; 3], "CLEM.SH".to_owned())];
    marks.extend(escorts.iter().map(|(l, o, _)| (*o, l.name.clone())));
    for (view, name) in [(top, "top"), (&oblique, "quarter")] {
        let (center, ppf) = frame(&[&fleet], view, DETAIL);
        let mut picture = tile(art, &fleet, view, center, ppf, "fleet,", DETAIL);
        let camera = Camera::new(view, center, ppf, DETAIL);
        for (at, label) in &marks {
            let [x, y, _] = camera.project(*at);
            for step in 0..180 {
                let turn = step as f32 * std::f32::consts::TAU / 180.;
                let (sin, cos) = turn.sin_cos();
                let (px, py) = ((x + 30. * cos) as i64, (y + 30. * sin) as i64);
                picture.rect(px, py, 2, 2, [255, 214, 90]);
            }
            let text = label.trim_end_matches(".SH");
            picture.text(&art.font, text, x as i64 + 36, y as i64 - 8, [255, 214, 90]);
        }
        picture.png(&out.join(format!("fleet-QFFLT-{name}.png")))?;
    }
    for view in [&VIEWS[0], &VIEWS[2], top] {
        let (center, ppf) = frame(&[&close], view, DETAIL);
        let picture = tile(art, &close, view, center, ppf, "parked aircraft,", DETAIL);
        let name = format!("fleet-QFFLT-carrier-{}.png", view.name.replace(' ', "-"));
        picture.png(&out.join(name))?;
    }
    println!(
        "Surface preview: ~QFFLT {} escorts and {} parked aircraft around CLEM",
        escorts.len(),
        parked.len()
    );
    Ok(())
}

pub fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let [out] = args.as_slice() else {
        return Err("--surface-preview OUTPUT_DIRECTORY".into());
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let media = Media::open()?;
    let raw = media.get("PALETTE.PAL")?;
    if raw.len() != 768 || raw.iter().any(|v| *v > 63) {
        return Err("invalid PALETTE.PAL".into());
    }
    let mut art = Art {
        palette: std::array::from_fn(|i| {
            std::array::from_fn(|j| ((u16::from(raw[i * 3 + j]) * 255 + 31) / 63) as u8)
        }),
        textures: BTreeMap::new(),
        font: Font::parse(&media.get("WIN11.FNT")?)?,
    };
    for subject in NEW {
        sheet(out, &media, &mut art, subject, "new")?;
    }
    for subject in &EXISTING {
        sheet(out, &media, &mut art, subject, "existing")?;
    }
    launcher_sheet(out, &media, &mut art)?;
    for carrier in &CARRIERS {
        carrier_sheet(out, &media, &mut art, carrier)?;
    }
    fleet_scene(out, &media, &mut art)?;
    // Each texture as the shapes see it, holes (index 255 or masked) magenta.
    let textures = out.join("textures");
    std::fs::create_dir_all(&textures)?;
    for (name, pic) in &art.textures {
        let mut canvas = Canvas::new(pic.width, pic.height);
        for (i, index) in pic.pixels.iter().enumerate() {
            canvas.rgb[i] = if *index == 255 || pic.mask.get(i) == Some(&false) {
                [255, 0, 255]
            } else {
                art.palette[usize::from(*index)]
            };
        }
        canvas.png(&textures.join(format!("{}.png", name.trim_end_matches(".PIC"))))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_put_the_bow_where_a_reviewer_expects() {
        let bow = [0., 100., 0.];
        let top = Camera::new(&VIEWS[3], [0.; 3], 1., TILE);
        assert!(top.project(bow)[0] > TILE[0] as f32 / 2. + 90.);
        let broadside = Camera::new(&VIEWS[1], [0.; 3], 1., TILE);
        assert!(broadside.project(bow)[0] > TILE[0] as f32 / 2. + 90.);
        // From the bow quarter the bow is nearer the eye than the stern.
        let quarter = Camera::new(&VIEWS[0], [0.; 3], 1., TILE);
        assert!(quarter.project(bow)[2] < quarter.project([0., -100., 0.])[2]);
    }

    #[test]
    fn faces_fill_their_palette_colour_with_depth() {
        let mut palette = [[0; 3]; 256];
        palette[7] = [200, 0, 0];
        palette[9] = [0, 0, 200];
        let art = Art {
            palette,
            textures: BTreeMap::new(),
            font: Font {
                height: 1,
                glyphs: Vec::new(),
            },
        };
        let square = |z: f32, color: u8| Face {
            positions: vec![
                [-10., z, -10.],
                [10., z, -10.],
                [10., z, 10.],
                [-10., z, 10.],
            ],
            colors: vec![color; 4],
            fog: Default::default(),
            uv: Vec::new(),
            texture: String::new(),
            subtype: 0x41,
            normal: None,
            address: 0,
        };
        // Seen from ahead, the square at forward 5 hides the one behind it.
        let view = View {
            name: "ahead",
            azimuth: 0.,
            elevation: 0.,
        };
        let camera = Camera::new(&view, [0.; 3], 4., [100, 100]);
        let mut canvas = Canvas::new(100, 100);
        draw(
            &mut canvas,
            &art,
            &[square(-5., 9), square(5., 7)],
            &[],
            &camera,
        );
        let middle = canvas.rgb[(50 + DROP as usize) * 100 + 50];
        assert!(middle[0] > 100 && middle[2] == 0, "{middle:?}");
        assert_eq!(canvas.rgb[0], BACKGROUND);
    }
}
