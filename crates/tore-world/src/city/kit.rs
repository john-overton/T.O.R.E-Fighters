//! The fill kit (CF0 prototype): real-size buildings the fill places, cut
//! from the retail city cluster shapes (one piece per connected group of
//! faces) and the retail single buildings. Every piece keeps its retail
//! shape, faces and textures; only its scale is the placed object rule,
//! [`crate::terrain::placed_shape_scale`] (a third of the retail shape
//! scale).

use crate::resources::ResourceSource;
use std::collections::BTreeSet;

/// Single buildings, drawn whole: (object type, or a shape with no object
/// type). Shapes the import lacks are skipped.
pub const SINGLES: [&str; 18] = [
    "HOUS.OT",
    "HOUSB.OT",
    "STORE.OT",
    "APTLA.OT",
    "APTLB.OT",
    "APTLC.OT",
    "APTM.OT",
    "APTOLD.OT",
    "BLDG1.OT",
    "BLDG2.OT",
    "BLDG3.OT",
    "FCTYA.OT",
    "FCTYB.OT",
    "FACTD.OT",
    "RES1.SH",
    "RES2.SH",
    "SLUM01.SH",
    "SLUM02.SH",
];

/// City clusters cut into their buildings.
pub const CLUSTERS: [&str; 16] = [
    "CITY1.OT",
    "CITY2.OT",
    "CITY3.OT",
    "CTYBKA.OT",
    "CTYBKB.OT",
    "CTYBKC.OT",
    "CTYBKD.OT",
    "CTYBKE.OT",
    "CTYBKF.OT",
    "CTYBKG.OT",
    "TWNBKA.OT",
    "TWNBKB.OT",
    "TWNBKC.OT",
    "TWNBKD.OT",
    "TWNBKE.OT",
    "TWNBKF.OT",
];

/// A cut piece lower than this is ground dressing (a pad, a lot, a road),
/// not a building (feet, fitted).
pub const MIN_HEIGHT_FT: f64 = 12.;
/// A piece narrower than this is a wall or a sign, not a building (feet,
/// fitted).
pub const MIN_SIDE_FT: f64 = 24.;
/// A cut piece with a side longer than this is a block, not a building
/// (feet, fitted).
pub const MAX_SIDE_FT: f64 = 320.;

/// What a piece looks like from the street, by real size (fitted bands).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// Under 45 ft tall and at most 120 ft across.
    House,
    /// Under 45 ft tall and wider: sheds, halls, factories.
    Industrial,
    /// 45 to 100 ft tall.
    LowRise,
    /// 100 to 200 ft tall.
    Apartment,
    /// 200 ft tall or more.
    Tower,
}

impl Class {
    pub fn of(side_ft: f64, height_ft: f64) -> Self {
        if height_ft >= 200. {
            Self::Tower
        } else if height_ft >= 100. {
            Self::Apartment
        } else if height_ft >= 45. {
            Self::LowRise
        } else if side_ft > 120. {
            Self::Industrial
        } else {
            Self::House
        }
    }
}

/// One building of the kit, base-centred: its local origin is the centre of
/// its footprint at its lowest point.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// The object type or shape it came from.
    pub source: String,
    /// The shape resource its faces are read from.
    pub shape: String,
    /// The faces of [`tore_formats::shape::Shape::scenery`] that make it.
    pub faces: Vec<u32>,
    /// Its base centre in shape units, face-position order (right, forward,
    /// up).
    pub origin: [f64; 3],
    /// Feet per shape unit ([`crate::terrain::placed_shape_scale`]).
    pub scale: f64,
    /// Half its footprint, right and forward, whole feet rounded up.
    pub half: [i64; 2],
    /// Its height, whole feet rounded up.
    pub height: i64,
    pub class: Class,
    /// Cut from a cluster rather than a whole single.
    pub cut: bool,
}

/// Every piece the fill may place, in a fixed order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kit {
    pub pieces: Vec<Piece>,
}

fn shape_of(resources: &dyn ResourceSource, source: &str) -> Option<(String, f64)> {
    if let Some(stem) = source.strip_suffix(".SH") {
        let name = format!("{stem}.SH");
        let bytes = resources.get(&name)?;
        let scale =
            tore_formats::shape::object_scale(bytes).ok()? * crate::terrain::REAL_SIZE_FACTOR;
        return Some((name, scale));
    }
    let definition = tore_formats::static_object::Definition::parse(resources.get(source)?).ok()?;
    let name = definition.main_shape.clone()?;
    let bytes = resources.get(&name)?;
    let scale = crate::terrain::placed_shape_scale(&definition, bytes).ok()?;
    Some((name, scale))
}

/// Groups of faces that share a vertex position (exact shape units).
fn components(shape: &tore_formats::shape::Shape) -> Vec<Vec<u32>> {
    let n = shape.faces.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut first = std::collections::BTreeMap::<[u32; 3], usize>::new();
    for (f, face) in shape.faces.iter().enumerate() {
        for p in &face.positions {
            let key = p.map(f32::to_bits);
            match first.get(&key) {
                Some(&g) => {
                    let (a, b) = (find(&mut parent, f), find(&mut parent, g));
                    if a != b {
                        parent[a.max(b)] = a.min(b);
                    }
                }
                None => {
                    first.insert(key, f);
                }
            }
        }
    }
    let mut groups = std::collections::BTreeMap::<usize, Vec<u32>>::new();
    for f in 0..n {
        let root = find(&mut parent, f);
        groups.entry(root).or_default().push(f as u32);
    }
    groups.into_values().collect()
}

fn piece(
    source: &str,
    shape_name: &str,
    shape: &tore_formats::shape::Shape,
    faces: Vec<u32>,
    scale: f64,
    cut: bool,
) -> Option<Piece> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for &f in &faces {
        let face = &shape.faces[f as usize];
        if face.positions.len() < 3 {
            continue;
        }
        for p in &face.positions {
            for axis in 0..3 {
                lo[axis] = lo[axis].min(f64::from(p[axis]));
                hi[axis] = hi[axis].max(f64::from(p[axis]));
            }
        }
    }
    if lo.iter().chain(&hi).any(|v| !v.is_finite()) {
        return None;
    }
    let size: [f64; 3] = std::array::from_fn(|axis| (hi[axis] - lo[axis]) * scale);
    if size[2] < MIN_HEIGHT_FT
        || size[0].max(size[1]) > MAX_SIDE_FT
        || size[0].min(size[1]) < MIN_SIDE_FT
    {
        return None;
    }
    let half = [(size[0] * 0.5).ceil() as i64, (size[1] * 0.5).ceil() as i64];
    Some(Piece {
        source: source.to_owned(),
        shape: shape_name.to_owned(),
        faces,
        origin: [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, lo[2]],
        scale,
        half: [half[0].max(1), half[1].max(1)],
        height: size[2].ceil() as i64,
        class: Class::of(size[0].max(size[1]), size[2]),
        cut,
    })
}

impl Kit {
    /// The singles whole and the clusters cut, from what the import holds.
    /// Identical cut buildings (same source, face count and size) keep one
    /// copy.
    pub fn load(resources: &dyn ResourceSource) -> Self {
        let mut pieces = Vec::new();
        let mut seen = BTreeSet::new();
        for (sources, cut) in [(&SINGLES[..], false), (&CLUSTERS[..], true)] {
            for source in sources {
                let Some((shape_name, scale)) = shape_of(resources, source) else {
                    continue;
                };
                let Some(Ok(shape)) = resources
                    .get(&shape_name)
                    .map(|b| tore_formats::shape::Shape::scenery(b))
                else {
                    continue;
                };
                let groups = if cut {
                    components(&shape)
                } else {
                    vec![(0..shape.faces.len() as u32).collect()]
                };
                for faces in groups {
                    let Some(p) = piece(source, &shape_name, &shape, faces, scale, cut) else {
                        continue;
                    };
                    if seen.insert((p.source.clone(), p.faces.len(), p.half, p.height)) {
                        pieces.push(p);
                    }
                }
            }
        }
        Self { pieces }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::shape::{Face, FogMode, Shape};

    fn quad(points: [[f32; 3]; 4]) -> Face {
        Face {
            positions: points.to_vec(),
            colors: vec![0; 4],
            fog: FogMode::default(),
            uv: Vec::new(),
            texture: String::new(),
            subtype: 0,
            normal: None,
            address: 0,
        }
    }

    fn cube(x: f32, size: f32, height: f32) -> Vec<Face> {
        let (a, b) = (x, x + size);
        vec![
            quad([[a, 0., 0.], [b, 0., 0.], [b, 0., height], [a, 0., height]]),
            quad([
                [b, 0., 0.],
                [b, size, 0.],
                [b, size, height],
                [b, 0., height],
            ]),
            quad([
                [a, 0., height],
                [b, 0., height],
                [b, size, height],
                [a, size, height],
            ]),
        ]
    }

    #[test]
    fn two_separate_boxes_cut_into_two_base_centred_pieces() {
        let mut faces = cube(0., 30., 60.);
        faces.extend(cube(100., 60., 300.));
        // A flat pad is ground dressing, not a building.
        faces.push(quad([
            [200., 0., 0.],
            [260., 0., 0.],
            [260., 60., 0.],
            [200., 60., 0.],
        ]));
        let shape = Shape {
            lines: Vec::new(),
            faces,
            billboards: Vec::new(),
            state_words: Default::default(),
        };
        let groups = components(&shape);
        assert_eq!(groups.len(), 3);
        let pieces: Vec<Piece> = groups
            .into_iter()
            .filter_map(|g| piece("T.OT", "T.SH", &shape, g, 1., true))
            .collect();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].origin, [15., 15., 0.]);
        assert_eq!(pieces[0].half, [15, 15]);
        assert_eq!(pieces[0].height, 60);
        assert_eq!(pieces[0].class, Class::LowRise);
        assert_eq!(pieces[1].origin, [130., 30., 0.]);
        assert_eq!(pieces[1].class, Class::Tower);
    }
}
