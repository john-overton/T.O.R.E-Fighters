//! Tu-95 "Bear-H" (`TU95.SH`) own-source rig (T95-2 and T95-3): the whole
//! tailplane (pitch only), the one-strip rudder and the six flat wheel plates of
//! the gear word. The four propeller discs are not turned here: they spin through
//! `variety_rotors` at the shared rate like the C-130's.
//! See docs/formats/aircraft-variety.md "Tu-95 shape and rig (T95-2/3, 2026-11-15)".
//!
//! T95-5 adds the fitted weapon bay: the four belly triangles are cut into two
//! door leaves while the doors are off their seat and a lined recess draws then
//! (the shared `bay_doors` helpers, as the B-52's; docs/formats/aircraft-variety.md,
//! "Tu-95 Bear-H stations, bays and pylons (T95-5, 2026-11-16)").
//!
//! TU95.SH ships exactly one state word, 0x33b0, the gear: six flat wheel plates
//! in longitudinal planes (a main plate in each nacelle plane and a nose plate on
//! the keel), both windings each. It has no elevator, aileron, flap, airbrake,
//! bay, hook, burner or flame word and none is invented; the one source strip is
//! the rudder (a triangle behind the fin's hinge line). The tailplane is three
//! whole panels on the left and one on the right, none of which shares a vertex
//! with any other face (coordinator decision: pitch only, as a whole stabilator).
//!
//! The shape's per-axis scale is the identity (it draws at 2/3 ft a unit through
//! the rig scale); every hinge point is still passed through [`scale_point`].
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};
use tore_sim::combat::{bays::Opening, hardpoints::grid_cells, store_sizes::StoreSize};

const ID: AircraftId = AircraftId::Tu95;
const WORDS: [usize; 1] = [0x33b0];

/// The four belly triangles the fitted bay is cut from (T95-5): the keel's two
/// strips a side, y 0 to 71, x -5 to 6, z -7 on the centre line to -5 at the
/// sides. They are the whole skin under the opening (y 0 to 39 units, x
/// +/-4.125 units); a door leaf is whatever of them lies inside its rectangle.
/// Source faces in the order right outer, right inner, left outer, left inner,
/// each with its three corners in shape units.
pub(crate) const BELLY: [usize; 4] = [0x1a38, 0x1a50, 0x22a7, 0x22c1];
const BELLY_CORNERS: [[[f32; 3]; 3]; 4] = [
    [[6., 71., -5.], [0., 71., -7.], [6., 0., -5.]],
    [[0., 71., -7.], [1., 0., -7.], [6., 0., -5.]],
    [[0., 71., -7.], [-5., 71., -5.], [-5., 0., -5.]],
    [[0., 71., -7.], [-5., 0., -5.], [1., 0., -7.]],
];
/// The first address of the lining's faces (above every face of the shape).
pub(crate) const LINING_BASE: usize = 0x7000;
/// The face whose material the lining takes: the flat skin plate in front of
/// the opening (y 71 to 88, subtype 0x63, no texture coordinates).
const LINING_TEMPLATE: usize = 0x1998;
/// The shape's feet a unit (it draws at 2/3 ft a unit).
pub(crate) const UNIT: f64 = 2. / 3.;

fn lining(address: usize) -> bool {
    address >= LINING_BASE
}

/// The bay's furniture, feet: a rail under the ceiling on each lane's centre
/// line, running the whole bay. Lanes are the columns of the shared grid for the
/// AS-16, the bay's store (three abreast at 1.59 ft; fitted: stores of other
/// sizes space their own grid, so the furniture is not a measure of the load).
/// No posts: the bay is one row long.
fn furniture(o: &Opening) -> Vec<[f64; 3]> {
    let kickback = StoreSize::of_shape(Some("KIKBK.SH"));
    let ([abreast, _], pitch) = grid_cells(kickback, [2. * o.outer, o.fore - o.aft]);
    (0..abreast)
        .map(|k| (k as f64 - (abreast as f64 - 1.) / 2.) * pitch)
        .map(|x| [x, o.aft, o.fore])
        .collect()
}

/// Tailplane: the right panel (two windings) and the left side (a root panel and
/// an outer panel, two windings each). They are one continuous surface at
/// z 8 to 9 across the centreline and turn together about one lateral axis, so
/// the shared edges at x 0 and x -5 never open. Source faces, in the order right,
/// left root, left outer.
pub(crate) const TAIL: [usize; 6] = [0x20f1, 0x2104, 0x2196, 0x21a9, 0x23fb, 0x240f];
/// Lateral spindle (an X axis) through the root chord midpoint (y -94 to -116).
/// Fitted: the source has no stabilator hinge.
const TAIL_SPINDLE: [f32; 3] = [0., -105., 9.];
/// Pitch travel either way, radians (fitted, agent decision 2026-11-15). No
/// Tu-95 stabilizer range was found; 10 degrees moves the trailing tip 4.3 units
/// (2.9 ft) at the far corner.
pub(crate) const TAIL_TRAVEL: f64 = 10. * std::f64::consts::PI / 180.;

/// Tailplane angle about +X, radians, positive lowering the trailing edge. A pull
/// (positive elevator) raises it.
pub(crate) fn tailplane_angle(state: &State) -> f64 {
    -TAIL_TRAVEL * state.elevator.clamp(-1., 1.)
}

/// Rudder: one triangle in the fin plane x = 0 behind the fin, both windings.
/// Its forward edge, (0,-112,42) to (0,-121,3), is shared with the fixed fin
/// faces, so nothing folds; the third vertex (0,-124,42) is the only free one.
pub(crate) const RUDDER: [usize; 2] = [0x2117, 0x2135];
const RUDDER_BOTTOM: [f32; 3] = [0., -121., 3.];
const RUDDER_TOP: [f32; 3] = [0., -112., 42.];
/// Full travel each way, radians. Fitted (agent decision 2026-11-15): no real
/// Tu-95 rudder limit and no state word pose exist (the shape has no rudder
/// word). 25 degrees puts the free vertex 4.9 units (3.3 ft) out of the fin plane.
pub(crate) const RUDDER_TRAVEL: f64 = 25. * std::f64::consts::PI / 180.;

/// Gear word 0x33b0: the left main plate (x -33), the right main plate (x 34) and
/// the nose plate (x 0), two windings each.
pub(crate) const MAIN_LEFT: [usize; 2] = [0x29f9, 0x2a2f];
pub(crate) const MAIN_RIGHT: [usize; 2] = [0x2a14, 0x2a4a];
pub(crate) const NOSE: [usize; 2] = [0x2a65, 0x2a88];
pub(crate) const GEAR: [usize; 6] = [0x29f9, 0x2a2f, 0x2a14, 0x2a4a, 0x2a65, 0x2a88];
/// Retraction pivot of each plate: a lateral axis (an X line) through the plate's
/// forward top corner, where the leg would hinge. Jane's: all units retract
/// rearward, the mains into the nacelles. The plates are flat wheel profiles in
/// longitudinal planes, so the retraction turns each one in its own plane:
/// rearward and up, as a real wheel profile does. A pivot on the forward edge
/// means no point is ever below the gear-down pose. Fitted.
const MAIN_PIVOT_Y: f32 = 9.;
const MAIN_PIVOT_Z: f32 = -6.;
const NOSE_PIVOT_Y: f32 = 76.;
const NOSE_PIVOT_Z: f32 = -7.;
/// Retraction turn, 90 degrees (fitted). The mains end at y 1 to 9, z -6 to 0,
/// inside the nacelle section (underside z -7, ridge about z 0); the nose plate
/// ends at y 69 to 76, z -7 to -1, inside the fuselage section.
const STOW: f64 = FRAC_PI_2;
/// Retraction timing (fitted): by retraction progress `p = 1 - gear`, the mains
/// run over p 0 to 0.8 and the nose plate over p 0.2 to 1, so the nose folds last
/// and unfolds first.
const MAIN_END: f64 = 0.8;
const NOSE_START: f64 = 0.2;

/// Fraction of the stow turn a plate has made at `gear` (1 down, 0 up).
pub(crate) fn stow_fraction(nose: bool, gear: f64) -> f64 {
    let p = 1. - gear.clamp(0., 1.);
    if nose {
        ((p - NOSE_START) / (1. - NOSE_START)).clamp(0., 1.)
    } else {
        (p / MAIN_END).clamp(0., 1.)
    }
}

/// Whole-plate gear motion for one face, `gear` 1 the source pose.
pub(crate) fn gear_positions(f: &mut Face, gear: f64) {
    let nose = NOSE.contains(&f.address);
    let (y, z) = if nose {
        (NOSE_PIVOT_Y, NOSE_PIVOT_Z)
    } else {
        (MAIN_PIVOT_Y, MAIN_PIVOT_Z)
    };
    let x = f.positions.first().map_or(0., |p| p[0]);
    let angle = STOW * stow_fraction(nose, gear);
    if angle != 0. {
        // About +X a positive turn moves a point below the pivot forward; the
        // plates fold rearward, so the turn is negative.
        let pivot = scale_point(ID, [0., y, z]);
        turn(f, [x, pivot[1], pivot[2]], [1., 0., 0.], -angle);
    }
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("TU95.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("TU95.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

fn is(p: [f32; 3], shape_units: [f32; 3]) -> bool {
    p == scale_point(ID, shape_units)
}

/// Every vertex of `face` is one of `allowed` (shape units).
fn only(face: &Face, count: usize, allowed: &[[f32; 3]]) -> bool {
    face.positions.len() == count
        && face
            .positions
            .iter()
            .all(|&p| allowed.iter().any(|&q| is(p, q)))
}

pub struct Rig {
    bay: Opening,
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 9148
            || shape.faces.len() != 155
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed TU95.SH layout".into());
        }
        // Tailplane: the reviewed corners of each panel.
        for (address, corners) in [
            (
                0x20f1,
                &[
                    [0., -116., 9.],
                    [0., -94., 9.],
                    [33., -121., 8.],
                    [33., -130., 8.],
                ][..],
            ),
            (
                0x2104,
                &[
                    [0., -116., 9.],
                    [33., -130., 8.],
                    [33., -121., 8.],
                    [0., -94., 9.],
                ],
            ),
            (
                0x2196,
                &[
                    [0., -94., 9.],
                    [-5., -98., 9.],
                    [-5., -118., 9.],
                    [0., -116., 9.],
                ],
            ),
            (
                0x21a9,
                &[
                    [0., -116., 9.],
                    [-5., -118., 9.],
                    [-5., -98., 9.],
                    [0., -94., 9.],
                ],
            ),
            (
                0x23fb,
                &[
                    [-5., -118., 9.],
                    [-33., -130., 8.],
                    [-33., -121., 8.],
                    [-5., -98., 9.],
                ],
            ),
            (
                0x240f,
                &[
                    [-5., -98., 9.],
                    [-33., -121., 8.],
                    [-33., -130., 8.],
                    [-5., -118., 9.],
                ],
            ),
        ] {
            if !only(one(&shape, address)?, 4, corners) {
                return Err(format!("unreviewed TU95.SH tailplane face {address:x}").into());
            }
        }
        // Rudder: the reviewed triangle, both hinge vertices present.
        for address in RUDDER {
            let face = one(&shape, address)?;
            if !only(face, 3, &[RUDDER_TOP, RUDDER_BOTTOM, [0., -124., 42.]])
                || !face.positions.contains(&scale_point(ID, RUDDER_TOP))
                || !face.positions.contains(&scale_point(ID, RUDDER_BOTTOM))
            {
                return Err(format!("unreviewed TU95.SH rudder face {address:x}").into());
            }
        }
        // The belly: the four reviewed triangles, whole, with the opening
        // wholly inside their span (y 0 to 71 units, x -5 to 6).
        let opening = tore_sim::combat::bays::opening(ID).ok_or("Tu-95 bay opening missing")?;
        for (address, corners) in BELLY.into_iter().zip(BELLY_CORNERS) {
            if !only(one(&shape, address)?, 3, &corners) {
                return Err(format!("unreviewed TU95.SH belly face {address:x}").into());
            }
        }
        let units = |feet: f64| (feet / UNIT) as f32;
        if units(opening.outer) > 5. || units(opening.aft) < 0. || units(opening.fore) > 71. {
            return Err("the Tu-95 bay opening leaves its belly skin".into());
        }
        // The recess takes the material of the flat plate ahead of the opening.
        let template = one(&shape, LINING_TEMPLATE)?.clone();
        let rails = furniture(&opening);
        let lining_faces = crate::bay_doors::centre_lining(&template, &opening, UNIT, &rails, &[])
            .into_iter()
            .enumerate()
            .map(|(k, mut f)| {
                f.address = LINING_BASE + k;
                f
            });
        // The gear word adds exactly the six plates and removes nothing.
        let neutral: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let down = Shape::with_state(bytes, &[(0x33b0, 1)].into())?;
        let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
        if !neutral.is_subset(&active)
            || active
                .difference(&neutral)
                .copied()
                .collect::<BTreeSet<_>>()
                != GEAR.into_iter().collect()
        {
            return Err("unreviewed TU95.SH gear branch".into());
        }
        let down = down.scaled(crate::variety_animation::axis_scale(ID));
        for (ids, x, y, top) in [
            (MAIN_LEFT, -33., [3., 9.], -6.),
            (MAIN_RIGHT, 34., [3., 9.], -6.),
            (NOSE, 0., [70., 76.], -7.),
        ] {
            for address in ids {
                let corners = [
                    [x, y[0], -14.],
                    [x, y[1], -14.],
                    [x, y[0], top],
                    [x, y[1], top],
                ];
                if !only(one(&down, address)?, 4, &corners) {
                    return Err(format!("unreviewed TU95.SH wheel plate {address:x}").into());
                }
            }
        }
        shape
            .faces
            .extend(down.faces.into_iter().filter(|f| GEAR.contains(&f.address)));
        shape.faces.extend(lining_faces);
        Ok((Self { bay: opening }, shape))
    }
    /// The faces a source face becomes: with the bay doors off their seat the
    /// four belly triangles are cut into the two door leaves, turned down about
    /// their hinges by `state.bay` (the doors' 1 second travel is the flight's);
    /// the lining draws only then.
    pub fn faces(&self, source: &Face, state: &State) -> Vec<Face> {
        let a = source.address;
        if BELLY.contains(&a) && state.bay > 0. {
            return crate::bay_doors::centre_doors(source, state.bay, &self.bay, UNIT);
        }
        if lining(a) && state.bay <= 0. {
            return Vec::new();
        }
        vec![source.clone()]
    }

    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if TAIL.contains(&a) {
            let angle = tailplane_angle(state);
            if angle != 0. {
                turn(
                    &mut result,
                    scale_point(ID, TAIL_SPINDLE),
                    scale_point(ID, [1., 0., 0.]),
                    angle,
                );
            }
            return Some(result);
        }
        if RUDDER.contains(&a) {
            let angle = RUDDER_TRAVEL * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(
                    &mut result,
                    scale_point(ID, RUDDER_BOTTOM),
                    scale_point(
                        ID,
                        std::array::from_fn(|k| RUDDER_TOP[k] - RUDDER_BOTTOM[k]),
                    ),
                    angle,
                );
            }
            return Some(result);
        }
        if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear);
            return Some(result);
        }
        if lining(a) && state.bay <= 0. {
            return None;
        }
        Some(result)
    }
}
/// The Tu-95 has no afterburner and no flame word.
pub(crate) fn flame(_address: usize) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    /// A synthetic face whose vertices are named in shape units.
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        let positions: Vec<[f32; 3]> = points.iter().map(|&p| scale_point(ID, p)).collect();
        Face {
            address,
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0x61,
            normal: Some([0., 0., 1.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn distance(p: [f32; 3], q: [f32; 3]) -> f32 {
        (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt()
    }
    fn spread(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let mut worst = 0f32;
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                worst = worst.max((distance(a[i], a[j]) - distance(b[i], b[j])).abs());
            }
        }
        worst
    }
    /// One test for the whole rig, on synthetic faces named in shape units: the
    /// tailplane pitches as one piece about its spindle, the rudder turns about
    /// its forward edge, the gear plates fold rearward into the nacelle and the
    /// fuselage and never dip below the gear-down pose, and the propeller discs
    /// (through `variety_rotors`) turn about their hubs the same way, the split
    /// disc's two halves together.
    #[test]
    fn the_tu95_rig_turns_whole_panels_about_their_hinges() {
        let rig = Rig {
            bay: tore_sim::combat::bays::opening(ID).unwrap(),
        };
        let moved = |s: &State, f: &Face| rig.animate(f, s).unwrap();
        let s0 = state();

        // Tailplane: a pull raises the trailing tip by 10 degrees, a push lowers
        // it, the whole surface stays one piece (the shared root edges move the
        // same), the spindle stays, and roll moves nothing.
        let right = face(
            0x20f1,
            &[
                [0., -116., 9.],
                [0., -94., 9.],
                [33., -121., 8.],
                [33., -130., 8.],
            ],
        );
        let left = face(
            0x2196,
            &[
                [0., -94., 9.],
                [-5., -98., 9.],
                [-5., -118., 9.],
                [0., -116., 9.],
            ],
        );
        let mut s = s0.clone();
        s.elevator = 1.;
        let (up_r, up_l) = (moved(&s, &right), moved(&s, &left));
        assert!(spread(&right.positions, &up_r.positions) < 1e-3);
        let rise = up_r.positions[3][2] - right.positions[3][2];
        assert!((rise - 25. * TAIL_TRAVEL.sin() as f32).abs() < 0.02 && rise > 0.);
        // The shared root edge (0,-94) and (0,-116) lands in the same place.
        assert!(distance(up_r.positions[1], up_l.positions[0]) < 1e-3);
        assert!(distance(up_r.positions[0], up_l.positions[3]) < 1e-3);
        s.elevator = -1.;
        assert!(moved(&s, &right).positions[3][2] < right.positions[3][2]);
        s.elevator = 7.;
        assert_eq!(moved(&s, &right).positions, up_r.positions, "clamped");
        s.elevator = 0.;
        s.aileron = 1.;
        s.flaps = 1.;
        assert_eq!(moved(&s, &right).positions, right.positions);

        // Rudder: the hinge vertices stay, the free vertex goes to +X for a
        // positive command and -X for a negative one, within the travel.
        let rudder = face(
            0x2117,
            &[[0., -112., 42.], [0., -124., 42.], [0., -121., 3.]],
        );
        let mut s = s0.clone();
        s.rudder = 1.;
        let turned = moved(&s, &rudder);
        assert!(spread(&rudder.positions, &turned.positions) < 1e-3);
        for k in [0, 2] {
            assert!(distance(turned.positions[k], rudder.positions[k]) < 1e-3);
        }
        let x = turned.positions[1][0];
        assert!((4.8..5.1).contains(&x), "free vertex x {x}");
        s.rudder = -1.;
        assert!((moved(&s, &rudder).positions[1][0] + x).abs() < 1e-3);
        s.rudder = 0.;
        assert_eq!(moved(&s, &rudder).positions, rudder.positions);

        // Gear: hidden at 0, the source pose at 1, rigid between, never below
        // the gear-down pose, and at the stow every plate lies inside the
        // nacelle (z -7 to 0.3, y 0 to 47 at the mains) or the fuselage.
        let mains = [
            (
                face(
                    0x2a14,
                    &[
                        [34., 9., -14.],
                        [34., 3., -14.],
                        [34., 3., -6.],
                        [34., 9., -6.],
                    ],
                ),
                true,
            ),
            (
                face(
                    0x29f9,
                    &[
                        [-33., 9., -14.],
                        [-33., 3., -14.],
                        [-33., 3., -6.],
                        [-33., 9., -6.],
                    ],
                ),
                true,
            ),
            (
                face(
                    0x2a65,
                    &[
                        [0., 70., -14.],
                        [0., 70., -7.],
                        [0., 76., -7.],
                        [0., 76., -14.],
                    ],
                ),
                false,
            ),
        ];
        let mut s = s0;
        s.gear = 0.;
        for (plate, _) in &mains {
            assert!(rig.animate(plate, &s).is_none());
        }
        s.gear = 1.;
        for (plate, _) in &mains {
            assert_eq!(moved(&s, plate).positions, plate.positions);
        }
        let low = scale_point(ID, [0., 0., -14.])[2];
        for (plate, _) in &mains {
            let mut last = f32::NEG_INFINITY;
            for gear in [0.95, 0.8, 0.6, 0.4, 0.2, 0.05, 1e-6] {
                s.gear = gear;
                let out = moved(&s, plate);
                assert!(spread(&plate.positions, &out.positions) < 1e-3);
                assert!(out.positions.iter().all(|p| p[2] >= low - 1e-3));
                // The lowest point only ever rises as the plate folds.
                let floor = out
                    .positions
                    .iter()
                    .map(|p| p[2])
                    .fold(f32::INFINITY, f32::min);
                assert!(floor >= last - 1e-3);
                last = floor;
            }
        }
        // Timing: near stow both plate kinds are fully folded; at gear 0.5 the
        // mains are 62.5 percent of the way and the nose 37.5 percent.
        assert!((stow_fraction(false, 0.5) - 0.625).abs() < 1e-9);
        assert!((stow_fraction(true, 0.5) - 0.375).abs() < 1e-9);
        assert_eq!(stow_fraction(true, 1.), 0.);
        assert_eq!(stow_fraction(false, 1e-6), 1.);
        s.gear = 1e-6;
        let inside_nacelle =
            |p: [f32; 3]| (-7.0..=0.3).contains(&p[2]) && (0. ..=48.).contains(&p[1]);
        for (plate, main) in &mains {
            for p in moved(&s, plate).positions {
                if *main {
                    assert!(inside_nacelle(p), "{p:?}");
                } else {
                    assert!(
                        (-7.01..=3.).contains(&p[2]) && (60. ..=77.).contains(&p[1]),
                        "{p:?}"
                    );
                }
            }
        }

        // Propellers (the shared `variety_rotors` path): every disc turns about
        // its hub the same way, the hub stays, and the two halves of the split
        // left outboard disc stay together at the same angle.
        let mut s = state();
        s.engine = true;
        s.throttle = 0.5;
        s.ticks = 3;
        let spin = |address: usize, points: &[[f32; 3]]| {
            let mut f = face(address, points);
            f.positions = points.to_vec();
            crate::variety_rotors::animate(ID, &mut f, &s);
            f
        };
        let angle_of = |f: &Face, hub: [f32; 3], start: [f32; 3]| {
            let (a0, a1) = (
                (start[2] - hub[2]).atan2(start[0] - hub[0]),
                (f.positions[0][2] - hub[2]).atan2(f.positions[0][0] - hub[0]),
            );
            (a1 - a0).rem_euclid(std::f32::consts::TAU)
        };
        let mut angles = Vec::new();
        for (address, hub, start) in [
            (0x1aa2, [35.25, 47.5, -1.5], [46., 48., 8.]),
            (0x1d20, [65.5, 30., -2.5], [76., 30., 7.]),
            (0x243f, [-34., 47.5, -1.5], [-23., 48., 8.]),
            (0x2610, [-64., 30., -2.5], [-53., 30., 7.]),
            (0x2850, [-64., 30., -2.5], [-53., 30., -12.]),
        ] {
            let f = spin(address, &[start, hub]);
            assert!(distance(f.positions[1], hub) < 1e-4, "hub {address:x}");
            assert!((distance(f.positions[0], hub) - distance(start, hub)).abs() < 1e-3);
            angles.push(angle_of(&f, hub, start));
        }
        assert!(angles[0] > 0.1);
        for a in &angles {
            assert!((a - angles[0]).abs() < 1e-3, "{angles:?}");
        }
        // One phase per facing panel: the first image stays, the other three go.
        assert!(crate::variety_rotors::keep_face(ID, 0x1aa2));
        assert!(!crate::variety_rotors::keep_face(ID, 0x1ac1));
        assert!(crate::variety_rotors::keep_face(ID, 0x28d5));
        assert!(!crate::variety_rotors::keep_face(ID, 0x2932));
    }
    /// T95-5: the fitted bay. Shut, the four belly triangles are whole and the
    /// lining is gone; open, the leaves hang from the hinge lines 2.75 ft (4.125
    /// units) either side of the centre line, their free edges a leaf's width
    /// (4.125 units) under the hinge height, and the lining draws: a ceiling,
    /// four walls and a rail for each of the three lanes of AS-16.
    #[test]
    fn the_tu95_bay_cuts_its_belly_into_door_leaves_and_lines_a_closed_recess() {
        let rig = Rig {
            bay: tore_sim::combat::bays::opening(ID).unwrap(),
        };
        let bay = rig.bay;
        let belly: Vec<Face> = BELLY
            .into_iter()
            .zip(BELLY_CORNERS)
            .map(|(address, corners)| face(address, &corners))
            .collect();
        let hinge_z = (bay.belly / UNIT) as f32;
        assert!((hinge_z + 5.55).abs() < 1e-4);
        // A belly triangle's area in plan, shape units squared.
        let plan = |f: &Face| {
            let n = f.positions.len();
            (0..n)
                .map(|i| {
                    let (a, b) = (f.positions[i], f.positions[(i + 1) % n]);
                    f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1])
                })
                .sum::<f64>()
                .abs()
                / 2.
        };
        let whole: f64 = belly.iter().map(plan).sum();
        let mut s = state();
        s.bay = 0.;
        let shut: Vec<Face> = belly.iter().flat_map(|f| rig.faces(f, &s)).collect();
        assert_eq!(shut.len(), 4);
        assert!(
            shut.iter()
                .zip(&belly)
                .all(|(a, b)| a.positions == b.positions)
        );
        // Shut, the lining draws nothing and is never animated.
        let ceiling = face(
            LINING_BASE,
            &[[-4.125, 0., 2.], [4.125, 0., 2.], [4.125, 39., 2.]],
        );
        assert!(rig.faces(&ceiling, &s).is_empty() && rig.animate(&ceiling, &s).is_none());
        // Open: the leaves hang, the rest of the skin stays in its plane.
        s.bay = 1.;
        let open: Vec<Face> = belly.iter().flat_map(|f| rig.faces(f, &s)).collect();
        let hanging: Vec<&Face> = open
            .iter()
            .filter(|f| f.positions.iter().any(|p| p[2] < hinge_z - 1.8))
            .collect();
        assert!(hanging.len() >= 2, "{} leaf pieces", hanging.len());
        let low = hanging
            .iter()
            .flat_map(|f| f.positions.iter())
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        assert!((low - (hinge_z - 4.125)).abs() < 1e-3, "{low}");
        // The skin outside the leaves keeps its area in plan (a hanging leaf has
        // none): the opening takes 5 ft by 26 ft, 8.25 by 39 units.
        let rest: f64 = open
            .iter()
            .filter(|f| !f.positions.iter().any(|p| p[2] < hinge_z - 1.8))
            .map(plan)
            .sum();
        assert!(
            rest < whole && rest > whole - 8.25 * 39. - 1e-3,
            "{rest} of {whole}"
        );
        // A vertex of the leaves stays near each hinge line at x +/-4.125 units
        // (the V skin is up to 0.2 unit off the mean hinge height there; a leaf
        // is several pieces, only some of them touch the hinge).
        for side in [4.125f32, -4.125] {
            assert!(
                hanging
                    .iter()
                    .flat_map(|f| &f.positions)
                    .any(|p| { (p[0] - side).abs() < 0.4 && (p[2] - hinge_z).abs() < 0.4 }),
                "no vertex on the hinge at {side}"
            );
        }
        // Half open the free edges hang on the quarter turn's sine and cosine:
        // a leaf's lowest point is a point of the keel's V (1.1 to 1.45 units under
        // the hinge height) turned by the angle.
        s.bay = 0.5;
        let half = belly.iter().flat_map(|f| rig.faces(f, &s));
        let low = half
            .flat_map(|f| f.positions)
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        let turn = FRAC_PI_2 as f32 * 0.5;
        let at = |dz: f32| hinge_z + dz * turn.cos() - 4.125 * turn.sin();
        assert!(
            low > at(-1.46) - 0.02 && low < at(-1.1) + 0.02,
            "{low} in {} to {}",
            at(-1.46),
            at(-1.1)
        );
        // The lining: a ceiling, four walls and five faces a lane rail, drawn
        // while the bay is off its seat, closed under the roof.
        s.bay = 1.;
        assert_eq!(rig.faces(&ceiling, &s).len(), 1);
        let template = face(LINING_TEMPLATE, &[[0.; 3]; 4]);
        let rails = furniture(&bay);
        assert_eq!(rails.len(), 3, "three lanes of AS-16 abreast");
        let faces = crate::bay_doors::centre_lining(&template, &bay, UNIT, &rails, &[]);
        assert_eq!(faces.len(), 5 + 5 * 3);
        let roof = ((bay.belly + bay.depth) / UNIT) as f32;
        assert_eq!(
            faces
                .iter()
                .filter(|f| f.positions.iter().all(|p| p[2] == roof))
                .count(),
            1
        );
    }
}
