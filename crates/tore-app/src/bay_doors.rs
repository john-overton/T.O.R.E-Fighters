//! The shared drawing of a fitted belly bay: its doors cut from the belly
//! skin and its lined recess, for the aircraft in
//! [`tore_sim::combat::bays::OPENINGS`] (the F-117 so far; the B-2 adds a row
//! there and its face addresses in its own rig), and for the B-52's one
//! centred bay ([`tore_sim::combat::bays::CENTRE_BAYS`], B52-5): two doors
//! hinged on the outer edges, a lined recess and a rail per store lane.
//! See docs/spec/stores-on-pylons.md, "F-117 weapon bays" and "B-52 bay".
//!
//! The geometry is in shape units: a third of a foot a unit, right, forward
//! and up, the shape already scaled per axis. Stored normals are right, up and
//! forward, and the drawing culls by stored normal, not by winding.
use tore_formats::shape::Face;
use tore_sim::combat::bays::{CentreBay, Opening, RAIL_DROP, RAIL_HALF};

/// Shape units from feet.
pub(crate) fn units(feet: f64) -> f32 {
    (feet * 3.) as f32
}

/// The leaves of the belly skin `face` at door travel `fraction` (0 shut, 1
/// open): both bays' two doors cut from it and turned about their hinge
/// lines, the rest of the skin kept whole.
pub(crate) fn belly_doors(face: &Face, fraction: f64, o: &Opening) -> Vec<Face> {
    let (inner, outer) = (units(o.inner), units(o.outer));
    let middle = (inner + outer) / 2.;
    // (left edge, right edge, hinge x, turn sign): a door hinged on its left
    // edge swings its free right edge down with a positive turn about the
    // forward axis.
    let doors = [
        (-outer, -middle, -outer, 1.),
        (-middle, -inner, -inner, -1.),
        (inner, middle, inner, 1.),
        (middle, outer, outer, -1.),
    ];
    crate::roster_animation::cut_doors(
        face,
        fraction,
        doors,
        units(o.aft),
        units(o.fore),
        units(o.belly),
    )
}

/// Shape units from feet for a shape drawn at `unit` feet a unit.
fn units_at(feet: f64, unit: f64) -> f32 {
    (feet / unit) as f32
}

/// The two leaves of a centred bay ([`CentreBay`]) cut from the belly skin
/// `face` at door travel `fraction` (0 shut, 1 open): each half the width,
/// hinged on its outer edge, its inner edge swinging down; the rest of the
/// skin kept whole.
pub(crate) fn centre_doors(face: &Face, fraction: f64, o: &CentreBay) -> Vec<Face> {
    let half = units_at(o.half, o.unit);
    crate::roster_animation::cut_doors(
        face,
        fraction,
        [(-half, 0., -half, 1.), (0., half, half, -1.)],
        units_at(o.aft, o.unit),
        units_at(o.fore, o.unit),
        units_at(o.belly, o.unit),
    )
}

/// The recess of a centred bay ([`CentreBay`]) and a rail under its ceiling
/// at each of `rails` (feet right of the centre line, aft end, fore end, feet
/// forward) and a post from the floor to the ceiling at each of `posts`
/// (feet right, feet forward), drawn only while the doors are open: a
/// ceiling, four walls facing in, the rails and the posts, closed on every
/// side so no sky shows through.
pub(crate) fn centre_lining(
    template: &Face,
    o: &CentreBay,
    rails: &[[f64; 3]],
    posts: &[[f64; 2]],
) -> Vec<Face> {
    const N: f32 = 32765.;
    let u = |feet: f64| units_at(feet, o.unit);
    let (aft, fore, half) = (u(o.aft), u(o.fore), u(o.half));
    let (floor, roof) = (u(o.belly), u(o.belly + o.depth));
    let mut faces = vec![
        panel(
            template,
            [
                [-half, aft, roof],
                [half, aft, roof],
                [half, fore, roof],
                [-half, fore, roof],
            ],
            [0., -N, 0.],
            CEILING,
        ),
        // The left wall faces right, the right wall left.
        panel(
            template,
            [
                [-half, aft, floor],
                [-half, fore, floor],
                [-half, fore, roof],
                [-half, aft, roof],
            ],
            [N, 0., 0.],
            WALL,
        ),
        panel(
            template,
            [
                [half, aft, floor],
                [half, fore, floor],
                [half, fore, roof],
                [half, aft, roof],
            ],
            [-N, 0., 0.],
            WALL,
        ),
        // The aft wall faces forward, the fore wall aft.
        panel(
            template,
            [
                [-half, aft, floor],
                [half, aft, floor],
                [half, aft, roof],
                [-half, aft, roof],
            ],
            [0., 0., N],
            WALL,
        ),
        panel(
            template,
            [
                [-half, fore, floor],
                [half, fore, floor],
                [half, fore, roof],
                [-half, fore, roof],
            ],
            [0., 0., -N],
            WALL,
        ),
    ];
    // A box from the ceiling down, or up from the floor to it: four sides
    // and the lower end (5 faces).
    let mut boxed = |x: [f32; 2], y: [f32; 2], z: [f32; 2]| {
        let (end, other, sign) = (z[0], z[1], -N);
        faces.push(panel(
            template,
            [
                [x[0], y[0], end],
                [x[1], y[0], end],
                [x[1], y[1], end],
                [x[0], y[1], end],
            ],
            [0., sign, 0.],
            RAIL,
        ));
        for (x, normal) in [(x[0], -N), (x[1], N)] {
            faces.push(panel(
                template,
                [
                    [x, y[0], end],
                    [x, y[1], end],
                    [x, y[1], other],
                    [x, y[0], other],
                ],
                [normal, 0., 0.],
                RAIL,
            ));
        }
        for (y, normal) in [(y[0], -N), (y[1], N)] {
            faces.push(panel(
                template,
                [
                    [x[0], y, end],
                    [x[1], y, end],
                    [x[1], y, other],
                    [x[0], y, other],
                ],
                [0., 0., normal],
                RAIL,
            ));
        }
    };
    for &[x, y0, y1] in rails {
        boxed(
            [u(x - RAIL_HALF), u(x + RAIL_HALF)],
            [u(y0), u(y1)],
            [roof - u(RAIL_DROP), roof],
        );
    }
    for &[x, y] in posts {
        boxed(
            [u(x - RAIL_HALF / 2.), u(x + RAIL_HALF / 2.)],
            [u(y - RAIL_HALF / 2.), u(y + RAIL_HALF / 2.)],
            [floor, roof],
        );
    }
    faces
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
    const N: f32 = 32765.;
    let (aft, fore) = (units(o.aft), units(o.fore));
    let (floor, roof) = (units(o.belly), units(o.belly + o.depth));
    let mut faces = Vec::new();
    for sign in [-1f32, 1.] {
        let (a, b) = (sign * units(o.inner), sign * units(o.outer));
        let (x0, x1) = (a.min(b), a.max(b));
        // The inner wall faces outboard, the outer wall inboard.
        let (inner_normal, outer_normal) = (sign * N, -sign * N);
        faces.push(panel(
            template,
            [
                [x0, aft, roof],
                [x1, aft, roof],
                [x1, fore, roof],
                [x0, fore, roof],
            ],
            [0., -N, 0.],
            CEILING,
        ));
        faces.push(panel(
            template,
            [
                [a, aft, floor],
                [a, fore, floor],
                [a, fore, roof],
                [a, aft, roof],
            ],
            [inner_normal, 0., 0.],
            WALL,
        ));
        faces.push(panel(
            template,
            [
                [b, aft, floor],
                [b, fore, floor],
                [b, fore, roof],
                [b, aft, roof],
            ],
            [outer_normal, 0., 0.],
            WALL,
        ));
        faces.push(panel(
            template,
            [
                [x0, aft, floor],
                [x1, aft, floor],
                [x1, aft, roof],
                [x0, aft, roof],
            ],
            [0., 0., N],
            WALL,
        ));
        faces.push(panel(
            template,
            [
                [x0, fore, floor],
                [x1, fore, floor],
                [x1, fore, roof],
                [x0, fore, roof],
            ],
            [0., 0., -N],
            WALL,
        ));
        // The trapeze rail: a box hung from the ceiling on the centre line.
        let centre = (a + b) / 2.;
        let (r0, r1) = (centre - units(RAIL_HALF), centre + units(RAIL_HALF));
        let (y0, y1) = (units(o.rail[0]), units(o.rail[1]));
        let low = roof - units(RAIL_DROP);
        faces.push(panel(
            template,
            [[r0, y0, low], [r1, y0, low], [r1, y1, low], [r0, y1, low]],
            [0., -N, 0.],
            RAIL,
        ));
        for (x, normal) in [(r0, -N), (r1, N)] {
            faces.push(panel(
                template,
                [[x, y0, low], [x, y1, low], [x, y1, roof], [x, y0, roof]],
                [normal, 0., 0.],
                RAIL,
            ));
        }
        for (y, normal) in [(y0, -N), (y1, N)] {
            faces.push(panel(
                template,
                [[r0, y, low], [r1, y, low], [r1, y, roof], [r0, y, roof]],
                [0., 0., normal],
                RAIL,
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
    fn a_centred_bay_cuts_two_leaves_hinged_on_the_outer_edges_and_lines_a_closed_recess() {
        let o = tore_sim::combat::bays::centre_bay(AircraftId::B52).unwrap();
        // The B-52 draws at 2/3 ft a unit: 1.5 units a foot.
        let belly = units_at(o.belly, o.unit);
        assert!((belly + 9.).abs() < 1e-4);
        let skin = face(
            0x46ae,
            vec![
                [-40., -40., belly],
                [40., -40., belly],
                [40., 70., belly],
                [-40., 70., belly],
            ],
        );
        let shut = centre_doors(&skin, 0., &o);
        assert!((shut.iter().map(area).sum::<f64>() - 80. * 110.).abs() < 1e-1);
        assert!(
            shut.iter()
                .all(|f| f.positions.iter().all(|p| p[2] == belly))
        );
        let open = centre_doors(&skin, 1., &o);
        let hanging: Vec<_> = open
            .iter()
            .filter(|f| f.positions.iter().any(|p| p[2] < belly - 1.))
            .collect();
        // Two leaves, each 3 ft wide, hung on x = +/-3 ft (4.5 units).
        assert_eq!(hanging.len(), 2);
        let half = units_at(o.half, o.unit);
        for leaf in hanging {
            let low = leaf.positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
            assert!((low - (belly - half)).abs() < 1e-3, "{low}");
            assert!(
                leaf.positions
                    .iter()
                    .all(|p| (p[0].abs() - half).abs() < 1e-3)
            );
        }
        // The lining: ceiling, four walls, five faces a rail.
        let template = face(0x2000, vec![[0.; 3]; 3]);
        let rails = [[-1.42, -8., 0.], [0., -8., 0.], [1.42, -8., 0.]];
        let lining = centre_lining(&template, &o, &rails, &[[0., 0.]]);
        assert_eq!(lining.len(), 5 + 5 * rails.len() + 5);
        let roof = units_at(o.belly + o.depth, o.unit);
        let ceiling: Vec<_> = lining
            .iter()
            .filter(|f| f.positions.iter().all(|p| p[2] == roof))
            .collect();
        assert_eq!(ceiling.len(), 1);
        let want =
            f64::from(units_at(2. * o.half, o.unit)) * f64::from(units_at(o.fore - o.aft, o.unit));
        assert!((area(ceiling[0]) - want).abs() < 1e-1);
        // Four walls and the four sides of the one post stand from the belly
        // plane to the roof.
        let tall = lining
            .iter()
            .filter(|f| {
                f.positions.iter().any(|p| p[2] == belly)
                    && f.positions.iter().any(|p| p[2] == roof)
            })
            .count();
        assert_eq!(tall, 4 + 4);
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
