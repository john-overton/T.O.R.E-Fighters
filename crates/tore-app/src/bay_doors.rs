//! The shared drawing of a fitted belly bay: its doors cut from the belly
//! skin and its lined recess, for the aircraft in
//! [`tore_sim::combat::bays::OPENINGS`] (the F-117 so far; the B-2 and B-52
//! add a row there and their face addresses in their own rig).
//! See docs/spec/stores-on-pylons.md, "F-117 weapon bays".
//!
//! The geometry is in shape units: a third of a foot a unit, right, forward
//! and up, the shape already scaled per axis. Stored normals are right, up and
//! forward, and the drawing culls by stored normal, not by winding.
use tore_formats::shape::Face;
use tore_sim::combat::bays::{Opening, RAIL_DROP, RAIL_HALF};

/// Shape units from feet.
pub(crate) fn units(feet: f64) -> f32 {
    (feet * 3.) as f32
}

/// The four door rectangles of `o` (left edge, right edge, hinge x, turn
/// sign), shape units: a door hinged on its left edge swings its free right
/// edge down with a positive turn about the forward axis. Each bay has two,
/// hinged at its inner and outer edges.
fn door_rectangles(o: &Opening) -> [(f32, f32, f32, f64); 4] {
    let (inner, outer) = (units(o.inner), units(o.outer));
    let middle = (inner + outer) / 2.;
    let c = units(o.centre);
    [
        (c - outer, c - middle, c - outer, 1.),
        (c - middle, c - inner, c - inner, -1.),
        (c + inner, c + middle, c + inner, 1.),
        (c + middle, c + outer, c + outer, -1.),
    ]
}

/// The leaves of the belly skin `face` at door travel `fraction` (0 shut, 1
/// open): both bays' two doors cut from it and turned about their hinge
/// lines, the rest of the skin kept whole.
pub(crate) fn belly_doors(face: &Face, fraction: f64, o: &Opening) -> Vec<Face> {
    crate::roster_animation::cut_doors(
        face,
        fraction,
        door_rectangles(o),
        units(o.aft),
        units(o.fore),
        units(o.belly),
    )
}

/// The skin `face` with both bays' openings cut out of it and nothing drawn
/// in them (the B-2's belly polygon, whose doors are the separate panels laid
/// under it, B2-5).
pub(crate) fn belly_hole(face: &Face, o: &Opening) -> Vec<Face> {
    crate::roster_animation::cut_door_parts(
        face,
        0.,
        door_rectangles(o),
        units(o.aft),
        units(o.fore),
        units(o.belly),
    )
    .1
}

/// A flat panel cut from `template`'s material: four corners, one stored
/// normal and one palette index, with no texture coordinates.
fn panel(template: &Face, points: [[f32; 3]; 4], normal: [f32; 3], color: u8) -> Face {
    let mut face = template.clone();
    face.positions = points.to_vec();
    face.colors = vec![color; 4];
    face.uv = Vec::new();
    face.normal = Some(normal);
    face
}

/// Palette indices of the recess surfaces (the shape's own bay-wall greys).
const WALL: u8 = 148;
const CEILING: u8 = 68;
const RAIL: u8 = 148;

/// The recess of every bay of `o` and its trapeze rail, drawn only while the
/// doors are open: a ceiling at the recess depth, four walls and a rail box
/// under the ceiling on the bay's centre line, all facing into the bay, so the
/// cut-out leaves no hole into the sky. `template` supplies the material (a
/// flat grey face of the shape).
pub(crate) fn lining(template: &Face, o: &Opening) -> Vec<Face> {
    lining_in(template, o, [WALL, CEILING, RAIL])
}

/// [`lining`] with the palette indices of the walls, the ceiling and the rail
/// chosen by the aircraft (the B-2's recess is its own bay grey, 147).
pub(crate) fn lining_in(template: &Face, o: &Opening, [wall, ceiling, rail]: [u8; 3]) -> Vec<Face> {
    const N: f32 = 32765.;
    let (aft, fore) = (units(o.aft), units(o.fore));
    let floor = units(o.belly);
    let (outer_roof, inner_roof) = (units(o.belly + o.depth), units(o.belly + o.inner_depth));
    let c = units(o.centre);
    let mut faces = Vec::new();
    for sign in [-1f32, 1.] {
        let (a, b) = (c + sign * units(o.inner), c + sign * units(o.outer));
        let (x0, x1) = (a.min(b), a.max(b));
        // The roof is a plane under the skin: `outer_roof` over the outer
        // wall, `inner_roof` over the inner, level when they are equal.
        let roof = |x: f32| {
            let t = (x - b) / (a - b);
            outer_roof + (inner_roof - outer_roof) * t
        };
        let slope = (roof(x1) - roof(x0)) / (x1 - x0);
        let tilt = (1. + slope * slope).sqrt();
        // The inner wall faces outboard, the outer wall inboard.
        let (inner_normal, outer_normal) = (sign * N, -sign * N);
        faces.push(panel(
            template,
            [
                [x0, aft, roof(x0)],
                [x1, aft, roof(x1)],
                [x1, fore, roof(x1)],
                [x0, fore, roof(x0)],
            ],
            [slope * N / tilt, -N / tilt, 0.],
            ceiling,
        ));
        faces.push(panel(
            template,
            [
                [a, aft, floor],
                [a, fore, floor],
                [a, fore, roof(a)],
                [a, aft, roof(a)],
            ],
            [inner_normal, 0., 0.],
            wall,
        ));
        faces.push(panel(
            template,
            [
                [b, aft, floor],
                [b, fore, floor],
                [b, fore, roof(b)],
                [b, aft, roof(b)],
            ],
            [outer_normal, 0., 0.],
            wall,
        ));
        faces.push(panel(
            template,
            [
                [x0, aft, floor],
                [x1, aft, floor],
                [x1, aft, roof(x1)],
                [x0, aft, roof(x0)],
            ],
            [0., 0., N],
            wall,
        ));
        faces.push(panel(
            template,
            [
                [x0, fore, floor],
                [x1, fore, floor],
                [x1, fore, roof(x1)],
                [x0, fore, roof(x0)],
            ],
            [0., 0., -N],
            wall,
        ));
        // The trapeze rail: a box hung from the ceiling on the centre line
        // (none where the opening has no rail).
        if o.rail[0] == o.rail[1] {
            continue;
        }
        let centre = (a + b) / 2.;
        let roof = roof(centre);
        let (r0, r1) = (centre - units(RAIL_HALF), centre + units(RAIL_HALF));
        let (y0, y1) = (units(o.rail[0]), units(o.rail[1]));
        let low = roof - units(RAIL_DROP);
        faces.push(panel(
            template,
            [[r0, y0, low], [r1, y0, low], [r1, y1, low], [r0, y1, low]],
            [0., -N, 0.],
            rail,
        ));
        for (x, normal) in [(r0, -N), (r1, N)] {
            faces.push(panel(
                template,
                [[x, y0, low], [x, y1, low], [x, y1, roof], [x, y0, roof]],
                [normal, 0., 0.],
                rail,
            ));
        }
        for (y, normal) in [(y0, -N), (y1, N)] {
            faces.push(panel(
                template,
                [[r0, y, low], [r1, y, low], [r1, y, roof], [r0, y, roof]],
                [0., 0., normal],
                rail,
            ));
        }
    }
    faces
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::aircraft::AircraftId;

    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            address,
            colors: vec![31; positions.len()],
            uv: Vec::new(),
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0x61,
            normal: Some([0., -32765., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn opening() -> Opening {
        tore_sim::combat::bays::opening(AircraftId::F117).unwrap()
    }
    fn area(f: &Face) -> f64 {
        let n = f.positions.len();
        (0..n)
            .map(|i| {
                let (a, b) = (f.positions[i], f.positions[(i + 1) % n]);
                f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1])
            })
            .sum::<f64>()
            .abs()
            / 2.
    }

    #[test]
    fn shut_doors_keep_the_belly_whole_and_open_doors_hang_on_their_edges() {
        let o = opening();
        let belly = units(o.belly);
        // A belly quad wider than both bays and longer than them.
        let skin = face(
            0x1000,
            vec![
                [-40., -40., belly],
                [40., -40., belly],
                [40., 40., belly],
                [-40., 40., belly],
            ],
        );
        let shut = belly_doors(&skin, 0., &o);
        assert!((shut.iter().map(area).sum::<f64>() - 80. * 80.).abs() < 1e-2);
        assert!(
            shut.iter()
                .all(|f| f.positions.iter().all(|p| p[2] == belly))
        );
        let open = belly_doors(&skin, 1., &o);
        // Skin area is kept: whole belly or a door leaf, never lost. A leaf
        // that hangs vertically has no plan area, so only the rest remains
        // in plan, and the four leaves hang below the belly.
        let hanging: Vec<_> = open
            .iter()
            .filter(|f| f.positions.iter().any(|p| p[2] < belly - 1.))
            .collect();
        assert_eq!(hanging.len(), 4);
        let depth = units((o.outer - o.inner) / 2.);
        for leaf in hanging {
            let low = leaf.positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
            assert!((low - (belly - depth)).abs() < 1e-3, "{low}");
            // Hung on an edge of the bay: x is the inner or the outer edge.
            let x = leaf.positions.iter().map(|p| p[0].abs());
            for x in x {
                assert!(
                    (x - units(o.inner)).abs() < 1e-3 || (x - units(o.outer)).abs() < 1e-3,
                    "{x}"
                );
            }
        }
    }

    #[test]
    fn the_lined_recess_is_closed_on_every_side() {
        let o = opening();
        let template = face(0x2000, vec![[0.; 3]; 3]);
        let faces = lining(&template, &o);
        // Per bay: ceiling and four walls, and a five-face rail.
        assert_eq!(faces.len(), 20);
        let roof = units(o.belly + o.depth);
        for bay in [-1f32, 1.] {
            let in_bay = |f: &&Face| f.positions.iter().all(|p| p[0] * bay >= 0.);
            let ceiling: Vec<_> = faces
                .iter()
                .filter(in_bay)
                .filter(|f| f.positions.iter().all(|p| p[2] == roof))
                .collect();
            assert_eq!(ceiling.len(), 1);
            let want = f64::from(units(o.outer - o.inner)) * f64::from(units(o.fore - o.aft));
            assert!((area(ceiling[0]) - want).abs() < 1e-2);
            // Four walls reach from the belly plane to the roof.
            let walls = faces
                .iter()
                .filter(in_bay)
                .filter(|f| {
                    f.positions.iter().any(|p| p[2] == units(o.belly))
                        && f.positions.iter().any(|p| p[2] == roof)
                })
                .count();
            assert_eq!(walls, 4);
        }
        // Every wall faces into its bay: the stored normal points from the
        // wall toward the bay's middle.
        for f in faces.iter().filter(|f| {
            f.positions.iter().any(|p| p[2] == units(o.belly))
                && f.positions.iter().any(|p| p[2] == roof)
                && f.colors[0] == WALL
        }) {
            let n = f.normal.unwrap();
            let c = f
                .positions
                .iter()
                .fold([0f32; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
            let c = c.map(|v| v / 4.);
            let middle = [
                c[0].signum() * (units(o.inner) + units(o.outer)) / 2.,
                (units(o.aft) + units(o.fore)) / 2.,
            ];
            let toward = [middle[0] - c[0], middle[1] - c[1]];
            // Stored normal is right, up, forward.
            assert!(
                n[0] * toward[0] + n[2] * toward[1] > 0.,
                "{:?}",
                f.positions
            );
        }
    }
}
