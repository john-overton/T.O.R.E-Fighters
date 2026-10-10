//! Aircraft carriers: the parts FA.EXE spawns with each carrier hull, and the
//! flight deck a hull shape's level faces make. Recorded facts and a rule
//! over the shape, no bytes: docs/formats/objects-and-shapes.md, "Carrier
//! parts and decks".
//!
//! The parts table is FA.EXE 1.02F data (names from `0x50cbd0`, offsets from
//! `0x50cbe8`, headings from `0x50cc08` for the Eisenhower; the other carriers
//! follow at `0x50cc18`, `0x50cc38` and `0x50cc80`). The spawning code turns
//! each offset by the carrier's attitude and adds it to the carrier's
//! position (`0x411d10`). Every height in the table is 0 except the
//! Clemenceau's catapult officer (20). Parts are skipped when the word at
//! `0x520a50` is 3 or 12 (not traced further).
use crate::shape::Shape;
use std::collections::BTreeMap;

/// A part spawned with a carrier: its shape, its offset from the carrier's
/// origin in world units (right, up, forward, turned with the carrier) and
/// its heading in binary angle units (65536 a turn).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub shape: &'static str,
    pub offset: [i16; 3],
    pub heading: i16,
}

/// A carrier hull and the parts spawned with it. The island is always the
/// last part, an OT whose damaged look is its own damage branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carrier {
    /// The hull shape, `CLEM.SH`.
    pub hull: &'static str,
    /// The hull's destroyed shape.
    pub damaged: &'static str,
    /// The NT record that names the hull.
    pub unit: &'static str,
    pub note: &'static str,
    pub parts: &'static [Attachment],
}

impl Carrier {
    /// The island: the last part.
    pub fn island(&self) -> Option<&Attachment> {
        self.parts.last()
    }
}

const fn part(shape: &'static str, offset: [i16; 3], heading: i16) -> Attachment {
    Attachment {
        shape,
        offset,
        heading,
    }
}

/// The carrier table in FA.EXE 1.02F.
pub const CARRIERS: [Carrier; 4] = [
    Carrier {
        hull: "NIMZ.SH",
        damaged: "NIMZ_A.SH",
        unit: "NIMZ.NT",
        note: "Eisenhower (NIMZ.NT)",
        parts: &[
            part("CATGUY.SH", [-15, 0, 1011], 32760),
            part("MULEA.SH", [292, 0, -408], -20384),
            part("MULEB.SH", [205, 0, -158], 4004),
            part("MULEC.SH", [-387, 0, -729], -3276),
            part("NIMZT.SH", [360, 0, -195], 0),
        ],
    },
    Carrier {
        hull: "KITT.SH",
        damaged: "KITT_A.SH",
        unit: "KITT.NT",
        note: "Kitty Hawk (KITT.NT)",
        parts: &[
            part("CATGUY.SH", [-15, 0, 1011], 32760),
            part("MULEA.SH", [252, 0, -408], -20384),
            part("MULEB.SH", [205, 0, -158], 4004),
            part("MULEC.SH", [-347, 0, -729], -3276),
            part("KITTT.SH", [300, 0, -190], 0),
        ],
    },
    Carrier {
        hull: "CLEM.SH",
        damaged: "CLEM_A.SH",
        unit: "CLEM.NT",
        note: "Clemenceau (CLEM.NT)",
        parts: &[
            part("CATGUY.SH", [70, 20, 1420], 32760),
            part("MULEA.SH", [330, 0, -700], -20384),
            part("MULEB.SH", [466, 0, 700], 4004),
            part("MULEC.SH", [-410, 0, -729], -3276),
            part("CLEMT.SH", [380, 0, 230], 0),
        ],
    },
    Carrier {
        hull: "WASP.SH",
        damaged: "WASP_A.SH",
        unit: "WASP.NT",
        note: "Wasp (WASP.NT)",
        parts: &[
            part("MULEA.SH", [80, 0, 320], -25116),
            part("WASPT.SH", [0, 0, 0], 0),
        ],
    },
];

/// The table's entry for a hull shape, if it is one of the four.
pub fn for_hull(shape: &str) -> Option<&'static Carrier> {
    CARRIERS
        .iter()
        .find(|carrier| carrier.hull.eq_ignore_ascii_case(shape))
}

/// The flat top that parts and parked aircraft stand on: the height shared
/// by the largest area of level faces, in shape units, the area of those
/// faces in square shape units, and their convex outline (right, forward),
/// counter-clockwise. A rule over the shape (fitted): the deck is the
/// largest level surface; the game's own deck test is not traced.
#[derive(Clone, Debug, PartialEq)]
pub struct Deck {
    pub height: f32,
    pub area: f32,
    pub outline: Vec<[f32; 2]>,
}

impl Deck {
    /// Whether `point` (right, forward, shape units) lies on the outline,
    /// edges included.
    pub fn contains(&self, point: [f32; 2]) -> bool {
        let n = self.outline.len();
        if n < 3 {
            return false;
        }
        (0..n).all(|i| {
            let (a, b) = (self.outline[i], self.outline[(i + 1) % n]);
            (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]) >= 0.
        })
    }
}

/// The deck of a hull shape, `None` when it has no level face.
pub fn flight_deck(shape: &Shape) -> Option<Deck> {
    let mut levels: BTreeMap<i64, (f32, Vec<[f32; 2]>)> = BTreeMap::new();
    for face in &shape.faces {
        let p = &face.positions;
        if p.len() < 3 {
            continue;
        }
        let z = p[0][2];
        if p.iter().any(|q| (q[2] - z).abs() > 1e-3) {
            continue;
        }
        let twice: f32 = (0..p.len())
            .map(|i| {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                a[0] * b[1] - b[0] * a[1]
            })
            .sum();
        let level = levels.entry((z * 1000.).round() as i64).or_default();
        level.0 += twice.abs() / 2.;
        level.1.extend(p.iter().map(|q| [q[0], q[1]]));
    }
    let (key, (area, points)) = levels.into_iter().max_by(|a, b| a.1.0.total_cmp(&b.1.0))?;
    Some(Deck {
        height: key as f32 / 1000.,
        area,
        outline: convex_hull(points),
    })
}

/// Andrew's monotone chain, counter-clockwise from the lowest-left point.
pub fn convex_hull(mut points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let turn = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut hull: Vec<[f32; 2]> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let ordered: Vec<[f32; 2]> = if pass == 0 {
            points.clone()
        } else {
            points.iter().rev().copied().collect()
        };
        for p in ordered {
            while hull.len() >= start + 2
                && turn(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::Face;

    fn face(points: &[[f32; 3]]) -> Face {
        Face {
            positions: points.to_vec(),
            colors: vec![0; points.len()],
            fog: Default::default(),
            uv: Vec::new(),
            texture: String::new(),
            subtype: 0x41,
            normal: None,
            address: 0,
        }
    }

    #[test]
    fn flight_deck_is_the_largest_level_area_with_its_convex_outline() {
        let shape = Shape {
            lines: Vec::new(),
            faces: vec![
                face(&[[0., 0., 10.], [4., 0., 10.], [4., 8., 10.], [0., 8., 10.]]),
                face(&[[4., 0., 10.], [6., 2., 10.], [4., 8., 10.]]),
                face(&[[0., 0., 20.], [1., 0., 20.], [1., 1., 20.]]),
                face(&[[0., 0., 0.], [9., 0., 0.], [9., 0., 30.]]),
            ],
            billboards: Vec::new(),
            state_words: Default::default(),
        };
        let deck = flight_deck(&shape).unwrap();
        assert_eq!(deck.height, 10.);
        assert_eq!(deck.area, 40.);
        assert_eq!(
            deck.outline,
            vec![[0., 0.], [4., 0.], [6., 2.], [4., 8.], [0., 8.]]
        );
        assert!(deck.contains([2., 4.]));
        assert!(deck.contains([0., 0.]), "edges count");
        assert!(deck.contains([5., 2.]));
        assert!(!deck.contains([6., 6.]));
        assert!(!deck.contains([-1., 4.]));
    }

    #[test]
    fn the_table_names_each_hull_once_with_its_island_last() {
        for carrier in &CARRIERS {
            assert_eq!(for_hull(carrier.hull), Some(carrier));
            assert_eq!(
                carrier.island().unwrap().shape,
                format!("{}T.SH", carrier.hull.trim_end_matches(".SH"))
            );
            assert_eq!(
                carrier.unit,
                carrier.hull.replace(".SH", ".NT"),
                "the NT names its hull alike"
            );
        }
        assert_eq!(for_hull("clem.sh").unwrap().hull, "CLEM.SH");
        assert!(for_hull("KIEV.SH").is_none());
    }
}
