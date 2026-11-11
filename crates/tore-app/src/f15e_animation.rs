//! F-15E Strike Eagle (`F15E.SH`) own-source rig: (F15E-4) the eighteen rigid
//! gear plates, the dorsal speed brake and the two afterburner plumes; (F15E-3)
//! the control surfaces: the two all-moving stabilators (whole panels with
//! pitch and differential roll), the two ailerons (whole panels about their
//! slanted hinge edges) and the two inboard flaps (whole panels, with the source
//! lowered flap's end triangle). There is no rudder: the fins stay put.
//! See docs/spec/variety-animation.md "F-15E gear, speed brake and plume fit" and
//! "F-15E control surfaces", and docs/formats/aircraft-control-surfaces.md
//! "F-15E gear, speed brake and plume (F15E-4, 2026-11-10)" and "F-15E control
//! surfaces (F15E-3, 2026-11-10)".
//!
//! F15E.SH ships five state words (the base order of the F-15 family, but a
//! different shape from the F-15C's, so nothing here is shared with
//! `f15_animation`): 0x80c0 the two afterburner flames, 0x80c6 the dorsal speed
//! brake, 0x80cc the gear, and 0x80d8 and 0x80de the left and right flap words.
//! The flap words are signed. Value 1 deletes the inboard flap wedge (two
//! faces) and adds nothing; value -1 swaps it for a lowered copy (two faces
//! whose trailing edge sits five units lower) and adds one end-closing
//! triangle. The rig never sets either word: the neutral wedge stays drawn in
//! every pose and turns whole (rigid) toward the reviewed lowered pose, and the
//! lowered branch's end triangle is drawn, rigid with the flap, while it is
//! down. There is no rudder, hook, slat, canopy, nozzle or gear-door word and
//! none is invented. The stabilators and ailerons have no word at all; they
//! are the neutral faces, found by address and checked at load.
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
/// The inboard flap wedges the flap words delete or lower. Each is a top and
/// a bottom quad (x 21 to 39, y -24 to -30, front edge z 1 and 4, trailing edge
/// z 3). They are the flap panels the rig turns whole.
pub(crate) const INBOARD_L: [usize; 2] = [0x5557, 0x5576];
pub(crate) const INBOARD_R: [usize; 2] = [0x5470, 0x548f];
/// The lowered flap branch's end triangle (word value -1): left `55fe`, right
/// `5517`, at the outboard end (x -39, 39). The rig draws it only while the flap
/// is down, with its trailing corner taken from the neutral wedge so it stays
/// rigid with the flap.
pub(crate) const FLAP_END_L: usize = 0x55fe;
pub(crate) const FLAP_END_R: usize = 0x5517;
/// Flap hinge: the lateral line through the front edge's mid height
/// (y -24, z 2.5; the edge runs z 1 to 4). Flaps down turn the whole wedge by
/// [`FLAP_DOWN`] about it, trailing edge down. Fitted to the lowered branch.
const FLAP_HINGE: [f32; 3] = [0., -24., 2.5];
/// Full flap travel, radians. Fitted (agent decision 2026-11-10): the rigid
/// turn that brings the wedge's trailing edge closest to the source's lowered
/// copy (trailing edge at y -29, z -2), about 0.74 units off; the source copy
/// is a foreshortened silhouette (chord 5 against 6), so no rigid turn matches
/// it exactly. Checked against the shape at load.
const FLAP_DOWN: f64 = 0.80;
/// Stabilators: the five faces of each all-moving panel (both windings; flat at
/// z -1, no anhedral; root chord y -41 to -70 on x 21 / -22, tip x 47 / -47).
/// Each turns whole about a lateral spindle through its root chord midpoint.
pub(crate) const STAB_R: [usize; 5] = [0x4b5f, 0x4b86, 0x4ba7, 0x4c5d, 0x4c8a];
pub(crate) const STAB_L: [usize; 5] = [0x42f1, 0x4318, 0x4339, 0x43ef, 0x441c];
const STAB_SPINDLE_Y: f32 = -55.5;
const STAB_SPINDLE_Z: f32 = -1.;
/// Ailerons: a top and a bottom quad per side (x 39 to 59). The front (hinge)
/// edge is slanted, from (39, -24) to (59, -27), and its root pair is 3 units
/// thick (z 1 and 4) while its tip is one vertex (z 3), so the vertices are not
/// collinear. The axis through the root pair's mid point (39, -24, 2.5) and the
/// tip vertex (59, -27, 3) keeps the tip vertex and the root mid point fixed and
/// moves each root vertex (1.5 units off the axis) a chord of
/// 2 * 1.5 * sin(angle / 2), about 0.30 units (0.10 ft) at full travel, the
/// least a rigid turn about any line can leave there.
pub(crate) const AIL_R: [usize; 2] = [0x4b33, 0x4ac4];
pub(crate) const AIL_L: [usize; 2] = [0x42c5, 0x41d9];
const AIL_ROOT: [f32; 3] = [39., -24., 2.5];
const AIL_TIP: [f32; 3] = [59., -27., 3.];
/// Full aileron travel, radians (the F-15C's, fitted).
const AIL_TRAVEL: f64 = 0.20;
/// Stabilator pitch and differential roll terms, radians (the F-15C's law,
/// fitted), and the clamp on their sum.
const STAB_PITCH: f64 = 0.30;
const STAB_ROLL: f64 = 0.10;
const STAB_LIMIT: f64 = 0.40;

/// Shape units between the wedge's trailing corner turned by [`FLAP_DOWN`] and
/// the source's lowered trailing corner (y -29, z -2).
pub(crate) fn flap_fit_error() -> f64 {
    let (dy, dz) = (
        f64::from(-30. - FLAP_HINGE[1]),
        f64::from(3. - FLAP_HINGE[2]),
    );
    let (sin, cos) = FLAP_DOWN.sin_cos();
    let y = f64::from(FLAP_HINGE[1]) + dy * cos - dz * sin;
    let z = f64::from(FLAP_HINGE[2]) + dy * sin + dz * cos;
    ((y + 29.).powi(2) + (z + 2.).powi(2)).sqrt()
}

/// The stabilator turn for an elevator and an aileron command, radians about
/// +X (negative raises the trailing edge). Positive aileron raises the right
/// trailing edge and lowers the left.
pub(crate) fn stabilator_angle(side: f64, elevator: f64, aileron: f64) -> f64 {
    (-STAB_PITCH * elevator.clamp(-1., 1.) - side * STAB_ROLL * aileron.clamp(-1., 1.))
        .clamp(-STAB_LIMIT, STAB_LIMIT)
}

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
        let lowered_l = branch(
            bytes,
            &original,
            0x80d8,
            -1,
            &[0x55c0, 0x55df, FLAP_END_L],
            &INBOARD_L,
        )?
        .scaled(axis);
        let lowered_r = branch(
            bytes,
            &original,
            0x80de,
            -1,
            &[0x54d9, 0x54f8, FLAP_END_R],
            &INBOARD_R,
        )?
        .scaled(axis);
        // The end triangle is the lowered copy's, re-seated on the neutral
        // wedge's trailing corner so it turns with the flap.
        if flap_fit_error() > 1. {
            return Err("F15E.SH flap travel too far from the lowered branch".into());
        }
        let mut flap_ends = Vec::new();
        for (lowered, end, x) in [
            (&lowered_l, FLAP_END_L, -1f32),
            (&lowered_r, FLAP_END_R, 1.),
        ] {
            let mut triangle = one(lowered, end)?.clone();
            // The corners in any order: the lowered trailing corner (y -29,
            // z -2) and the two front corners (z 1 and 4).
            let lowered_corner = sp([x * 39., -29., -2.]);
            let reviewed = [
                sp([x * 39., -24., 1.]),
                lowered_corner,
                sp([x * 39., -24., 4.]),
            ];
            if triangle.positions.len() != 3
                || !reviewed.iter().all(|r| triangle.positions.contains(r))
            {
                return Err(format!("unreviewed F15E.SH flap end {end:x}").into());
            }
            for p in &mut triangle.positions {
                if *p == lowered_corner {
                    *p = sp([x * 39., -30., 3.]);
                }
            }
            flap_ends.push(triangle);
        }
        // Every wedge, aileron and stabilator is a whole panel of reviewed
        // faces; the corners are checked so a different shape is refused. The
        // stabilators are flat at z -1 (no anhedral) and each panel's faces
        // cover exactly its seven corners.
        let panel = |ids: &[usize], allowed: &[[f32; 3]], sides: &[usize]| -> AppResult<()> {
            let mut seen = BTreeSet::new();
            for (&address, &count) in ids.iter().zip(sides) {
                let face = one(&shape, address)?;
                if face.positions.len() != count
                    || !face
                        .positions
                        .iter()
                        .all(|&p| allowed.iter().any(|&q| is(p, q)))
                {
                    return Err(format!("unreviewed F15E.SH panel face {address:x}").into());
                }
                seen.extend(face.positions.iter().map(|p| p.map(f32::to_bits)));
            }
            if seen.len() != allowed.len() {
                return Err(format!("unreviewed F15E.SH panel {:x}", ids[0]).into());
            }
            Ok(())
        };
        for x in [1f32, -1.] {
            let m = |v: [f32; 3]| [x * v[0], v[1], v[2]];
            let inner = if x > 0. { 21. } else { -22. };
            let stab = if x > 0. { &STAB_R } else { &STAB_L };
            let corners = if x > 0. {
                [
                    [47., -67., -1.],
                    [41., -73., -1.],
                    [21., -70., -1.],
                    [30., -50., -1.],
                    [21., -41., -1.],
                    [30., -47., -1.],
                    [47., -65., -1.],
                ]
            } else {
                [
                    [-47., -67., -1.],
                    [-42., -73., -1.],
                    [-22., -70., -1.],
                    [-31., -50., -1.],
                    [-22., -41., -1.],
                    [-31., -47., -1.],
                    [-47., -65., -1.],
                ]
            };
            panel(stab, &corners, &[4, 3, 4, 5, 4])?;
            let ail = if x > 0. { &AIL_R } else { &AIL_L };
            let wedge = if x > 0. { &INBOARD_R } else { &INBOARD_L };
            panel(
                ail,
                &[
                    m([39., -30., 3.]),
                    m([59., -34., 3.]),
                    m([59., -27., 3.]),
                    m([39., -24., 1.]),
                    m([39., -24., 4.]),
                ],
                &[4, 4],
            )?;
            panel(
                wedge,
                &[
                    m([39., -24., 1.]),
                    [inner, -24., 1.],
                    [inner, -30., 3.],
                    m([39., -30., 3.]),
                    [inner, -24., 4.],
                    m([39., -24., 4.]),
                ],
                &[4, 4],
            )?;
        }
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
        shape.faces.extend(flap_ends);
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if STAB_R.contains(&a) || STAB_L.contains(&a) {
            // All-moving stabilator: the whole five-face panel, both windings,
            // turns rigidly about a lateral spindle through its root chord
            // midpoint. The panel is flat (z -1, no anhedral), so the spindle is
            // square to the fuselage. Pitch pulls both trailing edges up; the
            // fitted differential roll raises the right and lowers the left for
            // a positive aileron. No corner is pinned, so a small root gap to
            // the fuselage is accepted (the aft root corner, 14.5 units from
            // the spindle, is the only vertex shared with a fixed face).
            let left = STAB_L.contains(&a);
            let (root_x, side) = if left { (-22., -1.) } else { (21., 1.) };
            let angle = stabilator_angle(side, state.elevator, state.aileron);
            if angle != 0. {
                turn(
                    &mut result,
                    sp([root_x, STAB_SPINDLE_Y, STAB_SPINDLE_Z]),
                    sp([1., 0., 0.]),
                    angle,
                );
            }
        } else if AIL_R.contains(&a) || AIL_L.contains(&a) {
            // Aileron: the whole wedge (both windings) turns about its slanted
            // hinge edge, the line from the root pair's mid point to the tip
            // vertex, outward. Positive aileron raises the right trailing edge
            // and lowers the left (a turn of -aileron about the outward axis on
            // both sides).
            let side = if AIL_R.contains(&a) { 1. } else { -1. };
            let root = sp([side * AIL_ROOT[0], AIL_ROOT[1], AIL_ROOT[2]]);
            let tip = sp([side * AIL_TIP[0], AIL_TIP[1], AIL_TIP[2]]);
            let angle = -state.aileron.clamp(-1., 1.) * AIL_TRAVEL;
            if angle != 0. {
                turn(
                    &mut result,
                    root,
                    std::array::from_fn(|k| tip[k] - root[k]),
                    angle,
                );
            }
        } else if INBOARD_R.contains(&a)
            || INBOARD_L.contains(&a)
            || a == FLAP_END_R
            || a == FLAP_END_L
        {
            // Flap: the whole wedge turns trailing edge down about the lateral
            // line through its front edge's mid height; the source's lowered
            // end triangle is drawn only while the flap is down.
            let travel = state.flaps.clamp(0., 1.);
            if (a == FLAP_END_R || a == FLAP_END_L) && travel <= 0. {
                return None;
            }
            if travel > 0. {
                turn(
                    &mut result,
                    sp(FLAP_HINGE),
                    sp([1., 0., 0.]),
                    travel * FLAP_DOWN,
                );
            }
        } else if GEAR.contains(&a) {
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
    fn stab_panel(left: bool) -> Vec<Face> {
        let x = |v: f32| if left { -v } else { v };
        let (ids, tip, root) = if left {
            (STAB_L, 42., 22.)
        } else {
            (STAB_R, 41., 21.)
        };
        vec![
            face(
                ids[0],
                &[
                    [x(47.), -67., -1.],
                    [x(tip), -73., -1.],
                    [x(root), -70., -1.],
                    [x(30.), -50., -1.],
                ],
            ),
            face(
                ids[1],
                &[
                    [x(root), -70., -1.],
                    [x(root), -41., -1.],
                    [x(30.), -50., -1.],
                ],
            ),
            face(
                ids[3],
                &[
                    [x(tip), -73., -1.],
                    [x(47.), -67., -1.],
                    [x(30.), -50., -1.],
                    [x(root), -41., -1.],
                    [x(root), -70., -1.],
                ],
            ),
        ]
    }
    fn mean(points: &[[f32; 3]]) -> [f32; 3] {
        std::array::from_fn(|k| points.iter().map(|p| p[k]).sum::<f32>() / points.len() as f32)
    }
    #[test]
    fn stabilators_turn_as_whole_rigid_panels_about_the_root_chord_midpoint() {
        let rig = rig();
        let mut s = state();
        for (left, side) in [(false, 1f32), (true, -1.)] {
            let panel = stab_panel(left);
            let source = all_points(&panel);
            let root = if left { -22. } else { 21. };
            for control in [-1., -0.5, 0., 0.5, 1.] {
                for (elevator, aileron) in [(control, 0.), (0., control), (control, 1.)] {
                    s.elevator = elevator;
                    s.aileron = aileron;
                    let moved = posed(&rig, &panel, &s);
                    let out = all_points(&moved);
                    assert!(spread(&source, &out) < 2e-3, "not rigid");
                    // The two root corners keep their mid point.
                    let (a, b) = (moved[1].positions[0], moved[1].positions[1]);
                    let middle = mean(&[a, b]);
                    let want = sp([root, STAB_SPINDLE_Y, STAB_SPINDLE_Z]);
                    for k in 0..3 {
                        assert!((middle[k] - want[k]).abs() < 1e-4);
                    }
                    // The trailing tip corner follows the net command: up for
                    // pull, and up on the right for a positive aileron.
                    let net =
                        (0.30 * elevator + f64::from(side) * 0.10 * aileron).clamp(-0.40, 0.40);
                    let dz = moved[2].positions[0][2] - panel[2].positions[0][2];
                    if net == 0. {
                        assert_eq!(out, source);
                    } else {
                        assert!(f64::from(dz) * net > 0., "{left} {elevator} {aileron}");
                    }
                    for (m, p) in moved.iter().zip(&panel) {
                        assert_eq!(m.uv, p.uv);
                    }
                }
            }
        }
    }
    #[test]
    fn stabilator_travel_is_the_clamped_pitch_plus_roll_law() {
        assert_eq!(stabilator_angle(1., 1., 0.), -0.30);
        assert_eq!(stabilator_angle(1., 0., 1.), -0.10);
        assert_eq!(stabilator_angle(-1., 0., 1.), 0.10);
        assert!((stabilator_angle(1., 1., 1.) + 0.40).abs() < 1e-12);
        assert!((stabilator_angle(-1., 1., 1.) + 0.20).abs() < 1e-12);
        assert_eq!(stabilator_angle(1., 9., 9.), -0.40);
    }
    fn aileron(left: bool) -> Vec<Face> {
        let x = |v: f32| if left { -v } else { v };
        let ids = if left { AIL_L } else { AIL_R };
        vec![
            face(
                ids[0],
                &[
                    [x(39.), -30., 3.],
                    [x(59.), -34., 3.],
                    [x(59.), -27., 3.],
                    [x(39.), -24., 1.],
                ],
            ),
            face(
                ids[1],
                &[
                    [x(59.), -34., 3.],
                    [x(39.), -30., 3.],
                    [x(39.), -24., 4.],
                    [x(59.), -27., 3.],
                ],
            ),
        ]
    }
    #[test]
    fn ailerons_turn_whole_about_the_slanted_hinge_and_roll_oppositely() {
        let rig = rig();
        let mut s = state();
        let tip_aft = sp([59., -34., 3.])[2];
        for (left, side) in [(false, 1f32), (true, -1.)] {
            let panel = aileron(left);
            let source = all_points(&panel);
            for value in [-1., -0.5, 0., 0.5, 1.] {
                s.aileron = value;
                let moved = posed(&rig, &panel, &s);
                let out = all_points(&moved);
                assert!(spread(&source, &out) < 2e-3, "not rigid");
                // The hinge tip vertex and the root pair's mid point stay.
                let tip = sp([side * 59., -27., 3.]);
                for (before, after) in panel[0].positions.iter().zip(&moved[0].positions) {
                    if *before == tip {
                        for k in 0..3 {
                            assert!((after[k] - tip[k]).abs() < 1e-4);
                        }
                    }
                }
                let middle = mean(&[moved[0].positions[3], moved[1].positions[2]]);
                let want = sp([side * 39., -24., 2.5]);
                for k in 0..3 {
                    assert!((middle[k] - want[k]).abs() < 1e-4);
                }
                // Positive aileron raises the right aft tip and lowers the left.
                let dz = moved[0].positions[1][2] - tip_aft;
                if value == 0. {
                    assert_eq!(out, source);
                } else {
                    assert!(dz * side * value as f32 > 0., "{left} {value}");
                }
                // The root vertices (1.5 units off the axis) swing a chord of
                // 2 * 1.5 * sin(angle / 2) at most.
                let root_top = moved[0].positions[3];
                let before = panel[0].positions[3];
                let gap = (0..3)
                    .map(|k| (root_top[k] - before[k]).powi(2))
                    .sum::<f32>()
                    .sqrt();
                assert!(gap <= 3. * (0.1 * value.abs() as f32).sin() + 1e-3);
            }
        }
    }
    #[test]
    fn flaps_turn_whole_toward_the_lowered_branch_and_draw_its_end_triangle() {
        let rig = rig();
        let mut s = state();
        assert!(flap_fit_error() < 0.8);
        let top = face(
            0x5470,
            &[
                [39., -24., 1.],
                [21., -24., 1.],
                [21., -30., 3.],
                [39., -30., 3.],
            ],
        );
        let bottom = face(
            0x548f,
            &[
                [21., -24., 4.],
                [39., -24., 4.],
                [39., -30., 3.],
                [21., -30., 3.],
            ],
        );
        let end = face(
            FLAP_END_R,
            &[[39., -24., 1.], [39., -30., 3.], [39., -24., 4.]],
        );
        let wedge = [top.clone(), bottom.clone()];
        s.flaps = 0.;
        assert_eq!(posed(&rig, &wedge, &s)[0].positions, top.positions);
        assert!(rig.animate(&end, &s).is_none());
        let mut last = 3f32;
        for flaps in [0.25, 0.5, 0.75, 1.] {
            s.flaps = flaps;
            let moved = posed(&rig, &wedge, &s);
            assert!(spread(&all_points(&wedge), &all_points(&moved)) < 2e-3);
            // The trailing edge only falls; the front edge's mid height stays.
            let trailing = moved[0].positions[3][2];
            assert!(trailing < last);
            last = trailing;
            let front = mean(&[moved[0].positions[0], moved[1].positions[1]]);
            let want = sp([39., -24., 2.5]);
            for k in 0..3 {
                assert!((front[k] - want[k]).abs() < 1e-4);
            }
            // The end triangle is rigid with the wedge's own end corners.
            let closing = rig.animate(&end, &s).unwrap();
            assert_eq!(closing.positions[0], moved[0].positions[0]);
            assert_eq!(closing.positions[1], moved[0].positions[3]);
            assert_eq!(closing.positions[2], moved[1].positions[1]);
        }
        // Full travel lands near the source's lowered trailing corner (y -29,
        // z -2).
        let corner = sp([39., -29., -2.]);
        let got = posed(&rig, &wedge, &s)[0].positions[3];
        let off = (0..3)
            .map(|k| (got[k] - corner[k]).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(off < 0.8, "{off}");
    }
    #[test]
    fn the_fins_and_the_rudder_command_leave_every_surface_still() {
        let rig = rig();
        let mut s = state();
        s.rudder = 1.;
        let all: Vec<Face> = stab_panel(false)
            .into_iter()
            .chain(aileron(true))
            .chain([face(
                0x4ae3,
                &[
                    [21., -59., 5.],
                    [21., -62., 43.],
                    [21., -54., 43.],
                    [21., -28., 3.],
                ],
            )])
            .collect();
        for f in &all {
            assert_eq!(rig.animate(f, &s).unwrap().positions, f.positions);
        }
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
