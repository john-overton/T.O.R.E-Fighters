//! F-15E Strike Eagle (`F15E.SH`) own-source rig: (F15E-4) the eighteen rigid
//! gear plates, the dorsal speed brake and the two afterburner plumes. The
//! control surfaces (stabilators, ailerons, flaps, fins) join this rig in
//! F15E-3 as further arms of [`Rig::animate`] and further load-time checks in
//! [`Rig::load`]; nothing here has to be restructured for them.
//! See docs/spec/variety-animation.md "F-15E gear, speed brake and plume fit"
//! and docs/formats/aircraft-control-surfaces.md "F-15E gear, speed brake and
//! plume (F15E-4, 2026-11-10)".
//!
//! F15E.SH ships five state words (the base order of the F-15 family, but a
//! different shape from the F-15C's, so nothing here is shared with
//! `f15_animation`): 0x80c0 the two afterburner flames, 0x80c6 the dorsal speed
//! brake, 0x80cc the gear, and 0x80d8 and 0x80de the left and right flap words.
//! The flap words are signed. Value 1 deletes the inboard flap wedge (two
//! faces) and adds nothing; value -1 swaps it for a lowered copy (two faces
//! whose trailing edge sits five units lower) and adds one end-closing
//! triangle. The rig never sets either: the neutral wedge stays drawn in every
//! pose, so no surface ever vanishes. F15E-3 owns the flap droop and may use the
//! reviewed -1 branch. There is no rudder, hook, slat, canopy, nozzle or
//! gear-door word and none is invented.
//!
//! The shape arrives scaled per axis (the F-15E has the identity scale until
//! John decides its size). Every point named here is in shape units and is
//! scaled with `scale_point`, so a part turned about it stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};

const WORDS: [usize; 5] = [0x80c0, 0x80c6, 0x80cc, 0x80d8, 0x80de];
const ID: AircraftId = AircraftId::F15E;

/// Gear word 0x80cc adds 18 faces: per main leg one inboard door plate (the
/// plane x = +-7, y -6 to 9, z -18 to -12), one wheel sheet (the plane
/// x = +-13, y -4 to 6, z -27 to -11) and one cross plate (the plane y = 1,
/// x +-9 to +-15, z -27 to -11), each in both windings; and for the nose one
/// wheel sheet (x = 0, y 48 to 62, z -27 to -8), one strut plate (x +-2,
/// y 46 to 50, z -21 to -8) and one cross plate (x -2 to 1, y 51 to 53,
/// z -27 to -8), each in both windings.
pub(crate) const DOOR_L: [usize; 2] = [0x50e3, 0x50fa];
pub(crate) const DOOR_R: [usize; 2] = [0x524f, 0x5266];
pub(crate) const LEG_L: [usize; 4] = [0x5156, 0x5175, 0x5194, 0x51b3];
pub(crate) const LEG_R: [usize; 4] = [0x52c2, 0x52e1, 0x5300, 0x531f];
pub(crate) const NOSE: [usize; 6] = [0x539b, 0x53ba, 0x53d9, 0x53f0, 0x5407, 0x5426];
pub(crate) const GEAR: [usize; 18] = [
    0x50e3, 0x50fa, 0x5156, 0x5175, 0x5194, 0x51b3, 0x524f, 0x5266, 0x52c2, 0x52e1, 0x5300, 0x531f,
    0x539b, 0x53ba, 0x53d9, 0x53f0, 0x5407, 0x5426,
];
/// Door hinge: the top edge of the door plate, a line along Y at z = -12 on the
/// plate's own x (right side; the left mirrors it). The door closes outward.
const DOOR_HINGE_X: f32 = 7.;
const DOOR_HINGE_Z: f32 = -12.;
/// Leg stow: the wheel sheet and its cross plate turn as ONE rigid body, first
/// about a vertical axis through the wheel centre (x 12, y 1) by 90 degrees,
/// which lays the wheel flat, then about a lateral axis (a line along X at
/// y -4, z -9, behind and above every leg vertex) by 90 degrees forward. The
/// fold axis lies behind every vertex, so no part of the wheel dips below its
/// gear-down height while it closes. Fitted.
const LEG_TWIST_X: f32 = 12.;
const LEG_TWIST_Y: f32 = 1.;
const LEG_FOLD_Y: f32 = -4.;
const LEG_FOLD_Z: f32 = -9.;
/// Nose stow: all six nose plates turn as one rigid body forward by 90 degrees
/// about a lateral axis (a line along X at y 46, z -8) behind and above every
/// nose vertex, and steer about a vertical axis through the strut (x 0, y 52).
const NOSE_FOLD: [f32; 3] = [0., 46., -8.];
const NOSE_STEER: [f32; 3] = [0., 52., 0.];
/// Brake word 0x80c6: a main plate (y 20 to -1, rising aft from z 16 to 30,
/// x -6..5 at the front, -3..3 at the free edge) and a front closing plate (y 20
/// to 25, falling from z 16 to 11), each in both windings. The front plate meets
/// the dorsal skin on the line y 25, z 11, the hinge; the whole four-face piece
/// turns about it.
pub(crate) const BRAKE: [usize; 4] = [0x568c, 0x56a3, 0x56ba, 0x56d1];
const BRAKE_HINGE: [f32; 3] = [0., 25., 11.];
/// The dorsal ridge the closed brake lies on: the source spine vertices
/// (0, 20, 10) and (0, -10, 6) (faces 0x24b3 and 0x24c5).
const SPINE: [[f32; 3]; 2] = [[0., 20., 10.], [0., -10., 6.]];
/// The brake free edge's mid point, open, (0, -1, 30).
const BRAKE_FREE: [f32; 3] = [0., -1., 30.];
/// Flame word 0x80c0: two plumes of four plates each (a vertical plate at
/// x = +-8 and a horizontal plate at z = -1 centred on it, both windings),
/// from y -62 back to y -115. The nozzle exit discs lie at y -61.
pub(crate) const FLAME: [usize; 8] = [
    0x574f, 0x5776, 0x579d, 0x57c4, 0x57eb, 0x5812, 0x5839, 0x5860,
];
const FLAME_ROOT: f32 = -61.;
const FLAME_TIP: f32 = -115.;
/// The inboard flap wedges the flap words delete or lower.
const INBOARD_L: [usize; 2] = [0x5557, 0x5576];
const INBOARD_R: [usize; 2] = [0x5470, 0x548f];

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("F15E.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("F15E.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

/// The shape with one state word set to `value`; the faces it adds and removes
/// must be exactly the reviewed ones.
fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let pose = Shape::with_state(bytes, &[(word, value)].into())?;
    let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed F15E.SH branch {word:x}={value}").into());
    }
    Ok(pose)
}

/// A scaled vertex equals a shape-unit point (the same single `f32` product
/// `Shape::scaled` formed, so the comparison is exact).
fn is(p: [f32; 3], shape_units: [f32; 3]) -> bool {
    p == scale_point(ID, shape_units)
}

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

/// Closed speed brake angle, radians: the turn about the hinge that carries the
/// free edge's mid point onto the dorsal ridge (the line through the source
/// spine vertices), solved on the scaled shape where the turned point meets the
/// ridge line. Fitted.
pub(crate) fn brake_closed() -> f64 {
    let hinge = sp(BRAKE_HINGE);
    let (a, b) = (sp(SPINE[0]), sp(SPINE[1]));
    let free = sp(BRAKE_FREE);
    let (dy, dz) = (f64::from(free[1] - hinge[1]), f64::from(free[2] - hinge[2]));
    // Height of the turned free edge above the ridge under it.
    let above = |angle: f64| {
        let (s, c) = angle.sin_cos();
        let y = f64::from(hinge[1]) + dy * c - dz * s;
        let z = f64::from(hinge[2]) + dy * s + dz * c;
        let ridge = f64::from(a[2])
            + (y - f64::from(a[1])) / f64::from(b[1] - a[1]) * f64::from(b[2] - a[2]);
        z - ridge
    };
    // Open the free edge is above the ridge and a turn lowers it, once past the
    // reviewed start the other way round; bisect on the lowering turn.
    let (mut low, mut high) = (0., 1.2);
    for _ in 0..60 {
        let mid = (low + high) / 2.;
        if above(mid) > 0. {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) / 2.
}

/// The ridge height at `y` on the scaled shape (for the tests).
#[cfg(test)]
fn spine_height_at(y: f32) -> f32 {
    let (a, b) = (sp(SPINE[0]), sp(SPINE[1]));
    a[2] + (y - a[1]) / (b[1] - a[1]) * (b[2] - a[2])
}

/// Whole-piece gear motion, `gear` 1 the source pose and 0 stowed. Each main
/// leg (wheel sheet and cross plate) is one rigid body that lays flat and folds
/// forward; each inboard door turns about its own top edge and closes outward;
/// the six nose plates are one rigid body that folds forward and steers.
/// Hidden (not drawn) at gear 0 by [`Rig::animate`].
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let quarter = FRAC_PI_2 * (1. - gear.clamp(0., 1.));
    let a = f.address;
    if NOSE.contains(&a) {
        if steering != 0. {
            turn(f, sp(NOSE_STEER), sp([0., 0., 1.]), -steering);
        }
        turn(f, sp(NOSE_FOLD), sp([1., 0., 0.]), quarter);
        return;
    }
    let right = LEG_R.contains(&a) || DOOR_R.contains(&a);
    let side = if right { 1. } else { -1. };
    if DOOR_R.contains(&a) || DOOR_L.contains(&a) {
        turn(
            f,
            sp([side as f32 * DOOR_HINGE_X, 0., DOOR_HINGE_Z]),
            sp([0., 1., 0.]),
            -side * quarter,
        );
    } else {
        turn(
            f,
            sp([side as f32 * LEG_TWIST_X, LEG_TWIST_Y, 0.]),
            sp([0., 0., 1.]),
            side * quarter,
        );
        turn(
            f,
            sp([0., LEG_FOLD_Y, LEG_FOLD_Z]),
            sp([1., 0., 0.]),
            quarter,
        );
    }
}

pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 28906
            || shape.faces.len() != 350
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F15E.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words: value 1 deletes exactly the inboard wedge, value -1
        // swaps it for a lowered wedge plus one end triangle. They pin the
        // wedges as the surfaces the source treats as flaps; the rig never
        // sets either word (F15E-3 owns the flap droop).
        branch(bytes, &original, 0x80d8, 1, &[], &INBOARD_L)?;
        branch(bytes, &original, 0x80de, 1, &[], &INBOARD_R)?;
        branch(
            bytes,
            &original,
            0x80d8,
            -1,
            &[0x55c0, 0x55df, 0x55fe],
            &INBOARD_L,
        )?;
        branch(
            bytes,
            &original,
            0x80de,
            -1,
            &[0x54d9, 0x54f8, 0x5517],
            &INBOARD_R,
        )?;
        // The gear, brake and flame words add exactly the reviewed plates and
        // remove nothing. Each plate is one whole four-corner quad (both
        // windings) whose corners are checked against the reviewed source
        // values, so the rig refuses a shape that differs.
        let gear = branch(bytes, &original, 0x80cc, 1, &GEAR, &[])?.scaled(axis);
        let brake = branch(bytes, &original, 0x80c6, 1, &BRAKE, &[])?.scaled(axis);
        let flame = branch(bytes, &original, 0x80c0, 1, &FLAME, &[])?.scaled(axis);
        let corners = |ids: &[usize], shape: &Shape, allowed: &[[f32; 3]]| -> AppResult<()> {
            for &address in ids {
                let face = one(shape, address)?;
                if face.positions.len() != 4
                    || !face
                        .positions
                        .iter()
                        .all(|&p| allowed.iter().any(|&q| is(p, q)))
                {
                    return Err(format!("unreviewed F15E.SH plate {address:x}").into());
                }
            }
            Ok(())
        };
        for (door, leg, x) in [(&DOOR_R, &LEG_R, 1.), (&DOOR_L, &LEG_L, -1.)] {
            corners(
                &door[..],
                &gear,
                &[
                    [x * 7., 1., -18.],
                    [x * 7., 9., -12.],
                    [x * 7., -6., -12.],
                    [x * 7., -6., -17.],
                ],
            )?;
            // Wheel sheet and cross plate: both are corners of the one leg box
            // (x 9 to 15, y -4 to 6 and 1, z -27 to -11).
            let sheet = |ids: &[usize], points: [[f32; 3]; 4]| corners(ids, &gear, &points);
            sheet(
                &leg[..2],
                [
                    [x * 13., -4., -27.],
                    [x * 13., 6., -27.],
                    [x * 13., 6., -11.],
                    [x * 13., -4., -11.],
                ],
            )?;
            sheet(
                &leg[2..],
                [
                    [x * 15., 1., -27.],
                    [x * 9., 1., -27.],
                    [x * 9., 1., -11.],
                    [x * 15., 1., -11.],
                ],
            )?;
        }
        corners(
            &NOSE[..2],
            &gear,
            &[
                [0., 48., -8.],
                [0., 62., -8.],
                [0., 62., -27.],
                [0., 48., -27.],
            ],
        )?;
        corners(
            &NOSE[2..4],
            &gear,
            &[
                [-2., 46., -21.],
                [2., 46., -21.],
                [2., 50., -8.],
                [-2., 50., -8.],
            ],
        )?;
        corners(
            &NOSE[4..],
            &gear,
            &[
                [-2., 51., -27.],
                [1., 51., -27.],
                [1., 53., -8.],
                [-2., 53., -8.],
            ],
        )?;
        corners(
            &BRAKE[..2],
            &brake,
            &[
                [5., 20., 16.],
                [-6., 20., 16.],
                [-3., -1., 30.],
                [3., -1., 30.],
            ],
        )?;
        corners(
            &BRAKE[2..],
            &brake,
            &[
                [5., 20., 16.],
                [-6., 20., 16.],
                [-3., 25., 11.],
                [3., 25., 11.],
            ],
        )?;
        // The dorsal ridge the closed brake lies on must be in the neutral shape.
        for (address, vertices) in [(0x24b3, &SPINE[..]), (0x24c5, &SPINE[..])] {
            let face = one(&shape, address)?;
            if !vertices.iter().all(|&v| face.positions.contains(&sp(v))) {
                return Err(format!("unreviewed F15E.SH spine face {address:x}").into());
            }
        }
        // The plume: two vertical and two horizontal plates per side, each
        // four corners, all between the first plate row y -62 and the tip
        // y -115 inside the reviewed envelope; the exit plane stays put.
        let (mut root, mut tip) = (f32::NEG_INFINITY, f32::INFINITY);
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 {
                return Err(format!("unreviewed F15E.SH plume plate {address:x}").into());
            }
            for p in &face.positions {
                root = root.max(p[1]);
                tip = tip.min(p[1]);
            }
        }
        if root != sp([0., -62., 0.])[1] || tip != sp([0., FLAME_TIP, 0.])[1] {
            return Err("unreviewed F15E.SH plume extent".into());
        }
        for source in gear
            .faces
            .into_iter()
            .filter(|f| GEAR.contains(&f.address))
            .chain(
                brake
                    .faces
                    .into_iter()
                    .filter(|f| BRAKE.contains(&f.address)),
            )
            .chain(
                flame
                    .faces
                    .into_iter()
                    .filter(|f| FLAME.contains(&f.address)),
            )
        {
            shape.faces.push(source);
        }
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        // F15E-3 adds the stabilators, ailerons, flaps and fins as arms above
        // these device arms; each turns whole about its own hinge.
        if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            // Open (brake 1) is the source pose; closing turns the whole piece
            // about its forward hinge down onto the spine.
            turn(
                &mut result,
                sp(BRAKE_HINGE),
                sp([1., 0., 0.]),
                brake_closed() * (1. - state.brake.clamp(0., 1.)),
            );
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            // Each plume grows from the nozzle exit plane; every plate scales
            // from the same root, so the shared edges stay together.
            let root = sp([0., FLAME_ROOT, 0.])[1];
            for p in &mut result.positions {
                p[1] = root + (p[1] - root) * state.exhaust.clamp(0., 1.) as f32;
            }
        }
        Some(result)
    }
}
pub(crate) fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    /// A synthetic face whose vertices are named in shape units and scaled the
    /// way the loader scales the real shape.
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        let positions: Vec<[f32; 3]> = points.iter().map(|&p| sp(p)).collect();
        Face {
            address,
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0x4c,
            normal: Some([1., 0., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn rig() -> Rig {
        Rig
    }
    fn spread(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let d =
            |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt();
        let mut worst = 0f32;
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                worst = worst.max((d(a[i], a[j]) - d(b[i], b[j])).abs());
            }
        }
        worst
    }
    fn mirror(p: [f32; 3]) -> [f32; 3] {
        [-p[0], p[1], p[2]]
    }
    /// The source plates of the right leg (wheel sheet and cross plate) and the
    /// right door, in shape units.
    fn right_leg() -> Vec<Face> {
        vec![
            face(
                0x52c2,
                &[
                    [13., 6., -27.],
                    [13., 6., -11.],
                    [13., -4., -11.],
                    [13., -4., -27.],
                ],
            ),
            face(
                0x5300,
                &[
                    [9., 1., -27.],
                    [9., 1., -11.],
                    [15., 1., -11.],
                    [15., 1., -27.],
                ],
            ),
        ]
    }
    fn left_leg() -> Vec<Face> {
        let mut out = right_leg();
        for (f, address) in out.iter_mut().zip([0x5156, 0x5194]) {
            f.address = address;
            f.positions = f.positions.iter().map(|&p| mirror(p)).collect();
        }
        out
    }
    fn right_door() -> Face {
        face(
            0x524f,
            &[
                [7., -6., -17.],
                [7., 1., -18.],
                [7., 9., -12.],
                [7., -6., -12.],
            ],
        )
    }
    fn nose() -> Vec<Face> {
        vec![
            face(
                0x539b,
                &[
                    [0., 48., -8.],
                    [0., 62., -8.],
                    [0., 62., -27.],
                    [0., 48., -27.],
                ],
            ),
            face(
                0x53d9,
                &[
                    [-2., 46., -21.],
                    [2., 46., -21.],
                    [2., 50., -8.],
                    [-2., 50., -8.],
                ],
            ),
            face(
                0x5407,
                &[
                    [-2., 51., -27.],
                    [1., 51., -27.],
                    [1., 53., -8.],
                    [-2., 53., -8.],
                ],
            ),
        ]
    }
    fn all_points(faces: &[Face]) -> Vec<[f32; 3]> {
        faces.iter().flat_map(|f| f.positions.clone()).collect()
    }
    fn posed(rig: &Rig, faces: &[Face], s: &State) -> Vec<Face> {
        faces.iter().map(|f| rig.animate(f, s).unwrap()).collect()
    }
    #[test]
    fn every_gear_plate_hides_at_zero_and_shows_the_source_pose_at_one() {
        let rig = rig();
        let mut s = state();
        for plate in right_leg().into_iter().chain(nose()).chain([right_door()]) {
            s.gear = 0.;
            assert!(rig.animate(&plate, &s).is_none());
            s.gear = 1.;
            assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        }
    }
    #[test]
    fn a_main_leg_turns_as_one_rigid_body_that_never_dips_and_stows_in_the_bay() {
        let rig = rig();
        let mut s = state();
        for (leg, side) in [(right_leg(), 1f32), (left_leg(), -1.)] {
            let source = all_points(&leg);
            let mut last_low = f32::NEG_INFINITY;
            // From stow up to down the lowest point only falls toward the gear-down
            // low point, never below it.
            for step in 0..=200 {
                let gear = f64::from(200 - step) / 200.;
                s.gear = gear.max(1e-6);
                let out = all_points(&posed(&rig, &leg, &s));
                assert!(spread(&source, &out) < 2e-3, "not rigid at {gear}");
                let low = out.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min);
                assert!(low >= sp([0., 0., -27.])[2] - 1e-3, "dips at {gear}: {low}");
                assert!(low >= last_low - 1e-3, "low point rose then fell at {gear}");
                last_low = low;
            }
            // Stowed (a hair above zero): lying flat inside the bay between the
            // keel and the shelf wall, above the belly skin.
            s.gear = 1e-6;
            let out = all_points(&posed(&rig, &leg, &s));
            for p in &out {
                assert!(p[0] * side >= sp([6., 0., 0.])[0] - 1e-2, "{p:?}");
                assert!(p[0] * side <= sp([21., 0., 0.])[0] + 1e-2, "{p:?}");
                assert!(p[2] >= sp([0., 0., -9.])[2] - 1e-2, "{p:?}");
                assert!(p[2] <= sp([0., 0., 0.])[2] + 1e-2, "{p:?}");
                assert!(p[1] >= sp([0., -4., 0.])[1] - 1e-2, "{p:?}");
                assert!(p[1] <= sp([0., 25., 0.])[1], "{p:?}");
            }
            // The wheel sheet lies flat: constant height at stow.
            let sheet = &posed(&rig, &leg[..1], &s)[0];
            let z = sheet.positions[0][2];
            assert!(sheet.positions.iter().all(|p| (p[2] - z).abs() < 1e-3));
        }
    }
    #[test]
    fn the_left_leg_is_the_mirror_of_the_right_leg_at_every_gear_value() {
        let rig = rig();
        let mut s = state();
        for gear in [0.9, 0.5, 0.2, 1e-6] {
            s.gear = gear;
            let (r, l) = (
                all_points(&posed(&rig, &right_leg(), &s)),
                all_points(&posed(&rig, &left_leg(), &s)),
            );
            for (a, b) in r.iter().zip(&l) {
                for (got, want) in b.iter().zip(mirror(*a)) {
                    assert!((got - want).abs() < 1e-3, "{gear}: {a:?} {b:?}");
                }
            }
        }
    }
    #[test]
    fn a_main_leg_retracts_forward() {
        let rig = rig();
        let mut s = state();
        let leg = right_leg();
        let foot = leg[0].positions[3];
        for gear in [0.9, 0.6, 0.3, 1e-6] {
            s.gear = gear;
            let out = posed(&rig, &leg, &s);
            assert!(
                out[0].positions[3][1] > foot[1] + 1e-3,
                "{gear}: not forward"
            );
        }
    }
    #[test]
    fn a_door_turns_about_its_top_edge_and_closes_outward_flat() {
        let rig = rig();
        let mut s = state();
        let door = right_door();
        let mut left = right_door();
        left.address = 0x50e3;
        left.positions = left.positions.iter().map(|&p| mirror(p)).collect();
        for plate in [&door, &left] {
            let side = plate.positions[2][0].signum();
            for gear in [0.75, 0.5, 0.25, 1e-6] {
                s.gear = gear;
                let out = rig.animate(plate, &s).unwrap();
                assert!(spread(&plate.positions, &out.positions) < 1e-3);
                // The top edge (the two z -12 corners) is the hinge and stays.
                for k in [2usize, 3] {
                    for axis in 0..3 {
                        assert!(
                            (out.positions[k][axis] - plate.positions[k][axis]).abs() < 1e-3,
                            "top edge moved at {gear}"
                        );
                    }
                }
                // The foot moves outward and up, never down.
                let foot = out.positions[1];
                assert!((foot[0] - plate.positions[1][0]) * side > 1e-3, "{gear}");
                assert!(foot[2] >= plate.positions[1][2] - 1e-4);
            }
            s.gear = 1e-6;
            let out = rig.animate(plate, &s).unwrap();
            for p in &out.positions {
                assert!(
                    (p[2] - sp([0., 0., DOOR_HINGE_Z])[2]).abs() < 1e-2,
                    "not flat"
                );
            }
        }
    }
    #[test]
    fn the_six_nose_plates_fold_forward_as_one_body_and_stay_inside_the_nose() {
        let rig = rig();
        let mut s = state();
        let plates = nose();
        let source = all_points(&plates);
        let belly = sp([0., 0., -9.])[2];
        for step in 0..=100 {
            s.gear = (f64::from(100 - step) / 100.).max(1e-6);
            let out = all_points(&posed(&rig, &plates, &s));
            assert!(spread(&source, &out) < 2e-3, "not rigid at {}", s.gear);
            assert!(
                out.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min)
                    >= sp([0., 0., -27.])[2] - 1e-3
            );
        }
        // The foot swings forward and up (the gear retracts forward).
        let foot = plates[0].positions[3];
        s.gear = 0.5;
        assert!(posed(&rig, &plates, &s)[0].positions[3][1] > foot[1] + 1e-3);
        // Stowed: above the belly, within the nose cross section and the fuselage top.
        s.gear = 1e-6;
        for p in all_points(&posed(&rig, &plates, &s)) {
            assert!(p[2] >= belly - 1e-2, "{p:?}");
            assert!(p[2] <= sp([0., 0., 9.])[2], "{p:?}");
            assert!(p[0].abs() <= sp([3., 0., 0.])[0]);
            assert!(
                p[1] >= sp([0., 44., 0.])[1] && p[1] <= sp([0., 66., 0.])[1],
                "{p:?}"
            );
        }
    }
    #[test]
    fn nose_steering_turns_every_nose_plate_about_the_strut_and_stays_rigid() {
        let plates = nose();
        let mut turned = plates.clone();
        for f in &mut turned {
            gear_positions(f, 1., 0.4);
        }
        let (a, b) = (all_points(&plates), all_points(&turned));
        assert!(spread(&a, &b) < 1e-3);
        assert!(a != b);
        for (p, q) in a.iter().zip(&b) {
            assert!((p[2] - q[2]).abs() < 1e-4, "steering changed a height");
        }
    }
    fn brake_plates() -> Vec<Face> {
        vec![
            face(
                0x568c,
                &[
                    [5., 20., 16.],
                    [-6., 20., 16.],
                    [-3., -1., 30.],
                    [3., -1., 30.],
                ],
            ),
            face(
                0x56ba,
                &[
                    [5., 20., 16.],
                    [-6., 20., 16.],
                    [-3., 25., 11.],
                    [3., 25., 11.],
                ],
            ),
        ]
    }
    #[test]
    fn the_closed_brake_lies_on_the_spine_and_the_open_pose_is_the_source_piece() {
        let rig = rig();
        let mut s = state();
        let plates = brake_plates();
        s.brake = 0.;
        assert!(rig.animate(&plates[0], &s).is_none());
        s.brake = 1.;
        for f in &plates {
            assert_eq!(rig.animate(f, &s).unwrap().positions, f.positions);
        }
        let source = all_points(&plates);
        let hinge = sp(BRAKE_HINGE);
        for brake in [0.75, 0.5, 0.25, 1e-6] {
            s.brake = brake;
            let out = posed(&rig, &plates, &s);
            assert!(spread(&source, &all_points(&out)) < 1e-3, "{brake}");
            // The hinge edge (the two y 25 corners of the front plate) stays.
            for k in [2usize, 3] {
                for axis in 0..3 {
                    assert!(
                        (out[1].positions[k][axis] - plates[1].positions[k][axis]).abs() < 1e-3
                    );
                }
            }
            // Closing only lowers the free edge.
            assert!(out[0].positions[2][2] <= plates[0].positions[2][2] + 1e-4);
        }
        // Closed: the free edge's mid point lies on the dorsal ridge.
        s.brake = 1e-9;
        let out = rig.animate(&plates[0], &s).unwrap();
        let mid_y = (out.positions[2][1] + out.positions[3][1]) / 2.;
        let mid_z = (out.positions[2][2] + out.positions[3][2]) / 2.;
        assert!(
            (mid_z - spine_height_at(mid_y)).abs() < 1e-3,
            "off the ridge"
        );
        assert!(mid_y < hinge[1], "free edge not aft of the hinge");
    }
    #[test]
    fn each_plume_grows_from_the_nozzle_exit_and_keeps_its_shared_edges_together() {
        let rig = rig();
        let mut s = state();
        let vertical = face(
            0x574f,
            &[
                [8., -62., -6.],
                [8., -62., 5.],
                [8., -115., 5.],
                [8., -115., -6.],
            ],
        );
        let flat = face(
            0x5839,
            &[
                [13., -115., -1.],
                [4., -115., -1.],
                [4., -62., -1.],
                [13., -62., -1.],
            ],
        );
        s.exhaust = 0.;
        assert!(rig.animate(&vertical, &s).is_none());
        let root = sp([0., FLAME_ROOT, 0.])[1];
        let full = sp([0., FLAME_TIP, 0.])[1] - root;
        for exhaust in [0.25, 0.5, 1.] {
            s.exhaust = exhaust;
            let (a, b) = (
                rig.animate(&vertical, &s).unwrap(),
                rig.animate(&flat, &s).unwrap(),
            );
            // The same y grows the same way in both plates of a plume, so
            // the shared edge (x 8, z -1) stays where the plates cross.
            assert_eq!(a.positions[0][1], b.positions[3][1]);
            assert!((a.positions[2][1] - root - full * exhaust as f32).abs() < 1e-3);
        }
        s.exhaust = 1.;
        assert_eq!(
            rig.animate(&vertical, &s).unwrap().positions,
            vertical.positions
        );
    }
    #[test]
    fn faces_outside_the_devices_pass_through_unchanged() {
        let rig = rig();
        let s = state();
        let wedge = face(
            0x5470,
            &[
                [39., -24., 1.],
                [21., -24., 1.],
                [21., -30., 3.],
                [39., -30., 3.],
            ],
        );
        assert_eq!(rig.animate(&wedge, &s).unwrap().positions, wedge.positions);
    }
}
