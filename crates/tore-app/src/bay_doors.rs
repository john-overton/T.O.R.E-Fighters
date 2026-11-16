//! The shared drawing of a fitted belly bay: its doors cut from the belly
//! skin and its lined recess, for the aircraft in
//! [`tore_sim::combat::bays::OPENINGS`] (the F-117 and the B-2, the latter with
//! its face addresses in its own rig), and for the B-52's one centred bay (its
//! `OPENINGS` row has `centre` and `inner` 0, B52-5, folded onto the shared
//! table at the batch A integration; the Tu-26's is the same kind, T26-5): two doors hinged on the outer edges, a
//! lined recess and a rail per store lane.
//! See docs/spec/stores-on-pylons.md, "F-117 weapon bays" and "B-52 bay".
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

/// [`units_at`] for the aircraft rigs outside this module (F111-5).
pub(crate) fn units_at_pub(feet: f64, unit: f64) -> f32 {
    units_at(feet, unit)
}

/// Shape units from feet for a shape drawn at `unit` feet a unit.
fn units_at(feet: f64, unit: f64) -> f32 {
    (feet / unit) as f32
}

/// The two leaves of a centred bay (an [`Opening`] with `centre` and `inner`
/// 0, `outer` feet either side of the centre line, in a shape drawn at `unit`
/// feet a unit) cut from the belly skin `face` at door travel `fraction` (0
/// shut, 1 open): each half the width, hinged on its outer edge, its inner
/// edge swinging down; the rest of the skin kept whole.
pub(crate) fn centre_doors(face: &Face, fraction: f64, o: &Opening, unit: f64) -> Vec<Face> {
    let half = units_at(o.outer, unit);
    crate::roster_animation::cut_doors(
        face,
        fraction,
        [(-half, 0., -half, 1.), (0., half, half, -1.)],
        units_at(o.aft, unit),
        units_at(o.fore, unit),
        units_at(o.belly, unit),
    )
}

/// [`centre_doors`] for several centred bays one behind another, as the B-1B's
/// three (B1B-5): each bay's two leaves are cut from `face` (or from what the
/// bays before it left of it), a bay `face` does not reach adds nothing, and
/// the rest of the skin is kept whole. A bay that runs over the edge of `face`
/// is cut from each of the faces it crosses (the B-1B's forward bay crosses the
/// seam between its two belly polygons), the leaves of both turning about the
/// same hinge lines.
pub(crate) fn centre_doors_all(
    face: &Face,
    fraction: f64,
    bays: &[Opening],
    unit: f64,
) -> Vec<Face> {
    let mut leaves = Vec::new();
    let mut skin = vec![face.clone()];
    for o in bays {
        let half = units_at(o.outer, unit);
        let mut rest = Vec::new();
        for piece in skin {
            let (mut turned, kept) = crate::roster_animation::cut_door_parts(
                &piece,
                fraction,
                [(-half, 0., -half, 1.), (0., half, half, -1.)],
                units_at(o.aft, unit),
                units_at(o.fore, unit),
                units_at(o.belly, unit),
            );
            leaves.append(&mut turned);
            rest.extend(kept);
        }
        skin = rest;
    }
    leaves.extend(skin);
    leaves
}

/// The recess of a centred bay (as [`centre_doors`]) and a rail under its ceiling
/// at each of `rails` (feet right of the centre line, aft end, fore end, feet
/// forward) and a post from the floor to the ceiling at each of `posts`
/// (feet right, feet forward), drawn only while the doors are open: a
/// ceiling, four walls facing in, the rails and the posts, closed on every
/// side so no sky shows through.
pub(crate) fn centre_lining(
    template: &Face,
    o: &Opening,
    unit: f64,
    rails: &[[f64; 3]],
    posts: &[[f64; 2]],
) -> Vec<Face> {
    centre_lining_with(template, o, unit, rails, posts, o.belly, [WALL, CEILING, RAIL])
}

/// [`centre_lining`] with the palette indices of the walls, the ceiling and the
/// rails and posts chosen by the aircraft (the B-1B's recess is its own bay
/// plate's grey, 144, B1B-5).
pub(crate) fn centre_lining_in(
    template: &Face,
    o: &Opening,
    unit: f64,
    rails: &[[f64; 3]],
    posts: &[[f64; 2]],
    palette: [u8; 3],
) -> Vec<Face> {
    centre_lining_with(template, o, unit, rails, posts, o.belly, palette)
}

/// [`centre_lining`] with the walls and posts standing from `floor` feet up
/// instead of the opening's belly plane: a belly that hangs lower than the
/// hinge plane along its keel (the Tu-26's, T26-5) has its recess closed
/// down to the keel line.
pub(crate) fn centre_lining_from(
    template: &Face,
    o: &Opening,
    unit: f64,
    rails: &[[f64; 3]],
    posts: &[[f64; 2]],
    floor: f64,
) -> Vec<Face> {
    centre_lining_with(template, o, unit, rails, posts, floor, [WALL, CEILING, RAIL])
}

/// The lining with both the floor height and the palette chosen (unified at the
/// batch B and C integration, 2026-11-16: B1B-5's palette and T26-5's floor).
fn centre_lining_with(
    template: &Face,
    o: &Opening,
    unit: f64,
    rails: &[[f64; 3]],
    posts: &[[f64; 2]],
    floor: f64,
    [wall_color, ceiling_color, rail_color]: [u8; 3],
) -> Vec<Face> {
    const N: f32 = 32765.;
    let u = |feet: f64| units_at(feet, unit);
    let (aft, fore, half) = (u(o.aft), u(o.fore), u(o.outer));
    let (floor, roof) = (u(floor), u(o.belly + o.depth));
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
            ceiling_color,
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
            wall_color,
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
            wall_color,
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
            wall_color,
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
            wall_color,
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
            rail_color,
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
                rail_color,
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
                rail_color,
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
    fn a_centred_bay_cuts_two_leaves_hinged_on_the_outer_edges_and_lines_a_closed_recess() {
        let o = tore_sim::combat::bays::opening(AircraftId::B52).unwrap();
        // The B-52 draws at 2/3 ft a unit: 1.5 units a foot.
        let unit = 2. / 3.;
        let belly = units_at(o.belly, unit);
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
        let shut = centre_doors(&skin, 0., &o, unit);
        assert!((shut.iter().map(area).sum::<f64>() - 80. * 110.).abs() < 1e-1);
        assert!(
            shut.iter()
                .all(|f| f.positions.iter().all(|p| p[2] == belly))
        );
        let open = centre_doors(&skin, 1., &o, unit);
        let hanging: Vec<_> = open
            .iter()
            .filter(|f| f.positions.iter().any(|p| p[2] < belly - 1.))
            .collect();
        // Two leaves, each 3 ft wide, hung on x = +/-3 ft (4.5 units).
        assert_eq!(hanging.len(), 2);
        let half = units_at(o.outer, unit);
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
        let lining = centre_lining(&template, &o, unit, &rails, &[[0., 0.]]);
        assert_eq!(lining.len(), 5 + 5 * rails.len() + 5);
        let roof = units_at(o.belly + o.depth, unit);
        let ceiling: Vec<_> = lining
            .iter()
            .filter(|f| f.positions.iter().all(|p| p[2] == roof))
            .collect();
        assert_eq!(ceiling.len(), 1);
        let want =
            f64::from(units_at(2. * o.outer, unit)) * f64::from(units_at(o.fore - o.aft, unit));
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

    /// T26-5: a lining that stands from a floor lower than the hinge plane (the
    /// Tu-26's keel strip is 1.33 ft under the outer panels) reaches that floor
    /// with its four walls and keeps the ceiling where the opening puts it.
    #[test]
    fn a_centred_lining_can_stand_from_a_lower_floor() {
        let o = tore_sim::combat::bays::opening(AircraftId::Tu26).unwrap();
        let unit = 4. / 3.;
        let template = face(0x2000, vec![[0.; 3]; 3]);
        let floor = -4. * unit;
        let lining = centre_lining_from(&template, &o, unit, &[], &[], floor);
        assert_eq!(lining.len(), 5);
        assert_eq!(
            lining.len(),
            centre_lining(&template, &o, unit, &[], &[]).len()
        );
        let roof = units_at(o.belly + o.depth, unit);
        let low = units_at(floor, unit);
        assert!((low + 4.).abs() < 1e-4);
        let walls: Vec<_> = lining
            .iter()
            .filter(|f| f.positions.iter().any(|p| p[2] == low))
            .collect();
        assert_eq!(walls.len(), 4);
        assert!(
            walls
                .iter()
                .all(|f| f.positions.iter().any(|p| p[2] == roof))
        );
        // The ceiling is the opening's, not the floor's.
        assert_eq!(
            lining
                .iter()
                .filter(|f| f.positions.iter().all(|p| p[2] == roof))
                .count(),
            1
        );
        // The same lining from the hinge plane stands 1 unit higher.
        let above = centre_lining(&template, &o, unit, &[], &[]);
        assert!(
            above
                .iter()
                .any(|f| f.positions.iter().any(|p| (p[2] + 3.).abs() < 1e-4))
        );
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
