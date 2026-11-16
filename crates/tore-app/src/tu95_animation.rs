//! Tu-95 "Bear-H" (`TU95.SH`) own-source rig (T95-2 and T95-3): the whole
//! tailplane (pitch only), the one-strip rudder and the six flat wheel plates of
//! the gear word. The four propeller discs are not turned here: they spin through
//! `variety_rotors` at the shared rate like the C-130's.
//! See docs/formats/aircraft-variety.md "Tu-95 shape and rig (T95-2/3, 2026-11-15)".
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

const ID: AircraftId = AircraftId::Tu95;
const WORDS: [usize; 1] = [0x33b0];

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

pub struct Rig;
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
        Ok((Self, shape))
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
        let rig = Rig;
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
}
