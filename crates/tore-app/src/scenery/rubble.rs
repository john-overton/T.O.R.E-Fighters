//! Destroyed buildings that retail draws no ruin for (B16,
//! docs/spec/surface-defenses.md, "Destroyed looks"): a ruin generated from
//! the intact shape when the scenery is built. Fitted, an agent decision
//! (2026-10-12): retail keeps a ruin inside most building shapes (the
//! damage branch) and a few `~` variants; the rest (city blocks, houses,
//! industry, sites) had none, and the object used to vanish.
//!
//! - **Collapse.** Every face of a simple shape is split into small
//!   triangles ([`SPLITS`])
//!   and every point sinks toward the shape's lowest point, keeping 6 to 30
//!   percent of its height ([`KEEP`]) by a fixed function of where it
//!   stands, so shared corners stay joined: jagged wall stubs with their own
//!   textures and a crumpled roof.
//! - **Char.** Most faces that face up or down (the fallen roof and
//!   floors, [`CHARRED_SHARE`] of them by a fixed pick) lose their texture
//!   and are drawn charred, in dark greys that differ a little face by face.
//! - **Scorch.** A dark footprint a quarter wider than the shape, a foot
//!   above its lowest point: the burnt ground round any destroyed building
//!   (also under a retail ruin).
//!
//! Presentation only: nothing reaches the simulation, the wire or a
//! recording; the same shape gives the same ruin on every machine.
use tore_formats::shape::{Face, Shape};

/// How much of its height a collapsed vertex keeps: from the first share
/// to the first plus the second.
pub(super) const KEEP: [f32; 2] = [0.06, 0.24];
/// A face whose normal is this close to vertical is charred.
const LEVEL: f32 = 0.7;
/// The charred faces' colour (sRGB, before lighting) and how much it varies.
pub(super) const CHAR: [f32; 3] = [0.17, 0.15, 0.13];
const CHAR_VARIES: f32 = 0.35;
/// The share of the level faces charred; the rest keep their texture.
const CHARRED_SHARE: f32 = 0.6;

/// A repeatable 0 to 1 from a vertex position.
fn noise(point: [f32; 3], salt: u32) -> f32 {
    let mut v = u64::from(salt).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for axis in point {
        v = (v ^ u64::from(axis.to_bits())).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        v ^= v >> 29;
    }
    (v >> 40) as f32 / (1u64 << 24) as f32
}

/// The shape's lowest and highest point, by the shared grounding rule
/// (`tore_world::grounding::height_range`).
fn lowest(shape: &Shape) -> Option<[f32; 2]> {
    tore_world::grounding::height_range(shape).map(|[low, high]| [low as f32, high as f32])
}

/// Whether a face (shape axes right, forward, up) lies near level.
fn level(face: &Face) -> bool {
    let p = &face.positions;
    if p.len() < 3 {
        return false;
    }
    let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    length > 0. && (n[2] / length).abs() >= LEVEL
}

/// A building's generated ruin: its collapsed walls, still textured, and
/// its charred level faces, each with its colour. `None` for a shape with
/// no faces.
pub(super) struct Ruin {
    pub walls: Shape,
    pub charred: Vec<(Face, [f32; 3])>,
}

/// A point of a piece: its position and texture coordinates.
type Corner = ([f32; 3], Option<[f32; 2]>);

/// How many times each triangle is split in four before it collapses, so
/// walls break along jagged tops and roofs crumple: twice for a shape of
/// up to `SPLITS[0]` faces, once up to `SPLITS[1]`, and not at all for a
/// bigger one, which has the detail already (a ruin stays within about
/// sixteen times its shape's triangles).
const SPLITS: [usize; 2] = [24, 96];

/// How many times `shape`'s triangles are split ([`SPLITS`]).
fn splits(shape: &Shape) -> usize {
    match shape.faces.len() {
        n if n <= SPLITS[0] => 2,
        n if n <= SPLITS[1] => 1,
        _ => 0,
    }
}

/// `face` as triangles, each split `times` times at its edges' middles.
/// A middle depends only on its edge's ends, so faces that share an edge
/// share its points.
fn pieces(face: &Face, times: usize) -> Vec<Face> {
    let n = face.positions.len();
    if n < 3 {
        return Vec::new();
    }
    let uv = |k: usize| face.uv.get(k).copied();
    let mut out: Vec<[Corner; 3]> = (1..n - 1)
        .map(|t| [0, t, t + 1].map(|k| (face.positions[k], uv(k))))
        .collect();
    let mid = |a: &Corner, b: &Corner| -> Corner {
        (
            std::array::from_fn(|i| (a.0[i] + b.0[i]) / 2.),
            a.1.zip(b.1)
                .map(|(x, y)| [(x[0] + y[0]) / 2., (x[1] + y[1]) / 2.]),
        )
    };
    for _ in 0..times {
        out = out
            .iter()
            .flat_map(|[a, b, c]| {
                let (ab, bc, ca) = (mid(a, b), mid(b, c), mid(c, a));
                [[*a, ab, ca], [ab, *b, bc], [ca, bc, *c], [ab, bc, ca]]
            })
            .collect();
    }
    let color = face.colors.first().copied().unwrap_or(0);
    out.into_iter()
        .map(|corners| Face {
            positions: corners.iter().map(|c| c.0).collect(),
            colors: vec![color; 3],
            fog: face.fog,
            uv: if face.uv.len() == n {
                corners.iter().map(|c| c.1.unwrap_or([0.; 2])).collect()
            } else {
                Vec::new()
            },
            texture: face.texture.clone(),
            subtype: face.subtype,
            normal: None,
            address: face.address,
        })
        .collect()
}

/// How a shape breaks down: how much of its height a point keeps, which
/// faces char, and how many are torn away.
struct Style {
    keep: [f32; 2],
    /// Only faces near level are charred (a fallen roof and floors), or any.
    level_only: bool,
    charred_share: f32,
    /// The share of faces missing (ripped skin), 0 for a building.
    torn_share: f32,
}

/// A building: walls stubs and a charred roof ([`KEEP`], [`CHARRED_SHARE`]).
const BUILDING: Style = Style {
    keep: KEEP,
    level_only: true,
    charred_share: CHARRED_SHARE,
    torn_share: 0.,
};

/// A burnt aircraft hulk (John, 2026-10-13: "a destroyed parked aircraft
/// leaves a burnt hulk"; the numbers are the agent's, fitted by eye): the
/// aircraft's own shape settled onto its belly, keeping 30 to 60 percent of
/// its height, most of its skin charred whichever way it faces and a tenth
/// of it torn away.
const HULK: Style = Style {
    keep: [0.30, 0.30],
    level_only: false,
    charred_share: 0.85,
    torn_share: 0.10,
};

/// A building's generated ruin ([`BUILDING`]).
pub(super) fn collapse(shape: &Shape) -> Option<Ruin> {
    broken(shape, &BUILDING)
}

/// A parked aircraft's burnt hulk ([`HULK`]): charred, broken and sunk onto
/// its lowest point, so it lies where the aircraft stood.
pub(super) fn hulk(shape: &Shape) -> Option<Ruin> {
    broken(shape, &HULK)
}

fn broken(shape: &Shape, style: &Style) -> Option<Ruin> {
    let [low, high] = lowest(shape)?;
    let height = high - low;
    let sink = |p: [f32; 3]| -> [f32; 3] {
        let keep = style.keep[0] + style.keep[1] * noise(p, 1);
        [p[0], p[1], low + (p[2] - low) * keep]
    };
    let mut walls = Shape {
        lines: Vec::new(),
        faces: Vec::new(),
        billboards: Vec::new(),
        state_words: shape.state_words.clone(),
    };
    let mut charred = Vec::new();
    let times = splits(shape);
    // A ground shadow decal is not a face of the building ([`super::surface_art`]).
    let faces = shape
        .faces
        .iter()
        .filter(|face| !super::surface_art::is_shadow_decal(face, low));
    for piece in faces.flat_map(|face| pieces(face, times)) {
        let mut fallen = piece.clone();
        fallen.positions = piece.positions.iter().map(|p| sink(*p)).collect();
        let middle: [f32; 3] =
            std::array::from_fn(|i| piece.positions.iter().map(|p| p[i]).sum::<f32>() / 3.);
        if noise(middle, 7) < style.torn_share {
            continue;
        }
        if height > 0.
            && (!style.level_only || level(&fallen))
            && noise(middle, 4) < style.charred_share
        {
            let shade = 1. - CHAR_VARIES / 2. + CHAR_VARIES * noise(middle, 2);
            fallen.texture.clear();
            fallen.uv.clear();
            charred.push((fallen, CHAR.map(|c| c * shade)));
        } else {
            walls.faces.push(fallen);
        }
    }
    Some(Ruin { walls, charred })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use tore_formats::shape::FogMode;

    fn face(positions: Vec<[f32; 3]>) -> Face {
        Face {
            colors: vec![3; positions.len()],
            positions,
            fog: FogMode::Enabled,
            uv: Vec::new(),
            texture: "WALL.PIC".into(),
            subtype: 0,
            normal: None,
            address: 0,
        }
    }

    /// A 10 by 10 house, 20 high from -5: a wall and a roof.
    fn house() -> Shape {
        Shape {
            lines: Vec::new(),
            faces: vec![
                face(vec![
                    [0., 0., -5.],
                    [10., 0., -5.],
                    [10., 0., 15.],
                    [0., 0., 15.],
                ]),
                face(vec![
                    [0., 0., 15.],
                    [10., 0., 15.],
                    [10., 10., 15.],
                    [0., 10., 15.],
                ]),
            ],
            billboards: Vec::new(),
            state_words: BTreeSet::new(),
        }
    }

    #[test]
    fn a_ruin_is_low_keeps_its_walls_textured_and_chars_its_roof() {
        let ruin = collapse(&house()).unwrap();
        // Two quads, each two triangles split twice: 64 pieces.
        assert_eq!(ruin.walls.faces.len() + ruin.charred.len(), 64);
        assert!(ruin.walls.faces.iter().all(|f| f.texture == "WALL.PIC"));
        assert!(
            !ruin.charred.is_empty(),
            "some of the fallen roof is charred"
        );
        assert!(
            ruin.charred
                .iter()
                .all(|(f, c)| f.texture.is_empty() && c.iter().all(|v| *v < 0.25))
        );
        let heights = || {
            ruin.walls
                .faces
                .iter()
                .chain(ruin.charred.iter().map(|(f, _)| f))
                .flat_map(|f| f.positions.iter().map(|p| p[2]))
        };
        let top = heights().fold(f32::MIN, f32::max);
        // At most 30 percent of 20 feet above the base at -5.
        assert!(top <= -5. + 20. * (KEEP[0] + KEEP[1]) + 1e-4, "{top}");
        assert!(top > -5., "{top}");
        // The lowest point stays: the ruin stands where the building stood.
        assert_eq!(heights().fold(f32::MAX, f32::min), -5.);
        // The same shape gives the same ruin, and pieces sharing an edge
        // keep its points joined.
        let again = collapse(&house()).unwrap();
        assert_eq!(again.walls.faces.len(), ruin.walls.faces.len());
        for (a, b) in again.walls.faces.iter().zip(&ruin.walls.faces) {
            assert_eq!(a.positions, b.positions);
        }
        let empty = Shape {
            lines: Vec::new(),
            faces: Vec::new(),
            billboards: Vec::new(),
            state_words: BTreeSet::new(),
        };
        assert!(collapse(&empty).is_none());
        // A detailed shape is not split further.
        let mut busy = house();
        busy.faces = vec![busy.faces[0].clone(); 200];
        assert_eq!(collapse(&busy).unwrap().walls.faces.len(), 400);
    }

    #[test]
    fn a_hulk_is_the_shape_sunk_to_a_belly_mostly_charred_and_partly_torn() {
        let mut plane = house();
        // A busy shape is not split, so every face is one piece.
        plane.faces = (0..200)
            .map(|n| {
                let at = n as f32;
                face(vec![
                    [at, 0., -5.],
                    [at + 1., 0., -5.],
                    [at + 1., 3., 15.],
                    [at, 3., 15.],
                ])
            })
            .collect();
        let hulk = hulk(&plane).unwrap();
        let (kept, charred) = (hulk.walls.faces.len(), hulk.charred.len());
        // A tenth torn away, most of the rest charred (a face is two
        // triangles at 400 pieces; none split).
        assert!(
            (300..=380).contains(&(kept + charred)),
            "{}",
            kept + charred
        );
        assert!(
            charred > 3 * kept,
            "{charred} charred of {}",
            kept + charred
        );
        let heights = hulk
            .walls
            .faces
            .iter()
            .chain(hulk.charred.iter().map(|(f, _)| f))
            .flat_map(|f| f.positions.iter().map(|p| p[2]));
        let top = heights.clone().fold(f32::MIN, f32::max);
        // At most 60 percent of 20 feet above the lowest point, at -5.
        assert!(top <= -5. + 20. * 0.6 + 1e-4, "{top}");
        assert_eq!(heights.fold(f32::MAX, f32::min), -5.);
    }

    #[test]
    fn split_pieces_keep_texture_coordinates_and_cover_the_face() {
        let mut quad = house().faces[0].clone();
        quad.uv = vec![[0., 0.], [10., 0.], [10., 20.], [0., 20.]];
        let parts = pieces(&quad, 2);
        assert_eq!(parts.len(), 32);
        // Every piece's corner is inside the face, its uv where it lies.
        for part in &parts {
            for (p, uv) in part.positions.iter().zip(&part.uv) {
                assert!((0. ..=10.).contains(&p[0]) && (-5. ..=15.).contains(&p[2]));
                assert!((uv[0] - p[0]).abs() < 1e-4 && (uv[1] - (p[2] + 5.)).abs() < 1e-4);
            }
        }
    }
}
