//! Saab Gripen (`GRI.SH`) own-source rig. GRI-3: the two all-moving canards
//! (whole panels, pitch plus differential roll), the four delta elevon strips
//! (one angle per side, pitch plus differential roll plus flap droop) and the
//! rudder. GRI-4: the rigid main and nose gear with the main-gear door plates,
//! the two rear-fuselage airbrake plates and the afterburner plume.
//! See docs/spec/variety-animation.md "Gripen control fit" and "Gripen gear,
//! airbrake and plume fit", and docs/formats/aircraft-control-surfaces.md
//! "Gripen control surfaces (GRI-3, 2026-11-10)" and "Gripen gear, airbrakes
//! and plume (GRI-4, 2026-11-10)".
//!
//! GRI.SH ships six state words (the slot at 0x8092 is skipped, as in the whole
//! shape family): 0x8080 the plume, 0x8086 the two airbrake plates, 0x808c the
//! gear, 0x8098 and 0x809e the left and right flap words (each deletes that
//! wing's six elevon-strip faces and adds nothing) and 0x80a4 (swaps the rudder
//! quad for a copy turned about its leading edge). The rig never sets the flap
//! words or the rudder word: the strips and the neutral rudder stay drawn, and
//! are turned, so the elevons never vanish in flight. The canards are fixed
//! geometry in the source with no state word at all. There is no slat, canopy,
//! nozzle or hook word and none is invented.
//!
//! The Gripen draws at its own size (GRI-2: the identity axis scale), so the
//! hinge points below are plain shape units; `sp` still routes them through
//! `scale_point` so a later scale decision cannot detach a hinge.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};

const WORDS: [usize; 6] = [0x8080, 0x8086, 0x808c, 0x8098, 0x809e, 0x80a4];
const ID: AircraftId = AircraftId::Gripen;

/// Flap word 0x8098 deletes the left wing's elevon strips (inboard quad pair,
/// outboard two triangle pairs) and 0x809e the right wing's. Pinned at load and
/// never set: the rig keeps the strips drawn and turns them, one angle per side
/// for the inboard and the outboard strip (they share the seam x 26 on the
/// right, x -25 on the left).
pub(crate) const ELEVON_L: [usize; 6] = [0x6194, 0x61b3, 0x61d2, 0x61ed, 0x6208, 0x6223];
pub(crate) const ELEVON_R: [usize; 6] = [0x5fd2, 0x5ff1, 0x6010, 0x602b, 0x6046, 0x6061];
/// Rudder word 0x80a4: removes the neutral quad, adds the turned copy. Pinned,
/// never set: the rig turns the neutral quad (both windings) about its hinge.
pub(crate) const RUDDER: [usize; 2] = [0x5d92, 0x5db9];
const RUDDER_WORD: [usize; 2] = [0x5dff, 0x5e26];
/// Canards: fixed geometry in the source (no state word). Each side is one
/// pentagonal plate (two windings) and a root filler triangle (two windings)
/// that touches only the plate and the fuselage side, so the whole canard is
/// plate plus filler.
pub(crate) const CANARD_R: [usize; 4] = [0x212d, 0x2284, 0x22f3, 0x2311];
pub(crate) const CANARD_L: [usize; 4] = [0x24ae, 0x2553, 0x25c2, 0x25e0];
/// The reviewed corners (shape units) of the right canard plate and filler; the
/// left canard is the mirror with one root corner a unit off ((-7,37,1) for
/// (-8,37,1)).
const CANARD_PLATE_R: [[f32; 3]; 5] = [
    [10., 19., 1.],
    [22., 15., 2.],
    [22., 19., 2.],
    [8., 37., 1.],
    [8., 30., 1.],
];
const CANARD_PLATE_L: [[f32; 3]; 5] = [
    [-10., 19., 1.],
    [-22., 15., 2.],
    [-22., 19., 2.],
    [-7., 37., 1.],
    [-8., 30., 1.],
];
const CANARD_FILLER_R: [[f32; 3]; 3] = [[10., 19., 1.], [9., 19., 1.], [8., 30., 1.]];
const CANARD_FILLER_L: [[f32; 3]; 3] = [[-10., 19., 1.], [-9., 19., 1.], [-8., 30., 1.]];
/// Canard spindles: a lateral line through each root chord midpoint (right
/// (8,37,1) to (10,19,1), left (-7,37,1) to (-10,19,1)) tilted only by the
/// plate's own dihedral (z rises 1 over the 12 units from the root edge to the
/// tip). The left canard is turned by reflecting, turning about the mirrored
/// right-hand spindle, and reflecting back.
const CANARD_SPINDLE_R: [f32; 3] = [9., 28., 1.];
const CANARD_SPINDLE_L: [f32; 3] = [-8.5, 28., 1.];
const CANARD_AXIS: [f32; 3] = [12., 0., 1.];
/// Elevon hinge: the straight line y -27 along X through the span, at the mid
/// height of the strip's hinge vertices (z -2 and -1).
const ELEVON_HINGE: [f32; 3] = [0., -27., -1.5];
const ELEVON_AXIS: [f32; 3] = [1., 0., 0.];
/// Rudder hinge: the vertical leading edge of the quad, x 0, y -43.
const RUDDER_HINGE: [f32; 3] = [0., -43., 0.];
const RUDDER_AXIS: [f32; 3] = [0., 0., 1.];
const RUDDER_CORNERS: [[f32; 3]; 4] = [
    [0., -43., 6.],
    [0., -50., 6.],
    [0., -49., 23.],
    [0., -43., 23.],
];

/// Fitted magnitudes in radians, all agent decisions (2026-11-10) except that
/// the canards take roll as well as pitch (John, 2026-11-10: "Canards
/// definitely roll"). Elevons match the fleet's delta wings (the Mirage 2000:
/// 0.30 pitch, 0.20 roll, 0.40 flap droop); the canards use the Su-35's 0.25
/// pitch and a smaller differential roll, with a sum bound because their roots
/// open a gap against the intake side that grows with the angle.
pub(crate) const ELEVON_PITCH: f64 = 0.30;
pub(crate) const ELEVON_ROLL: f64 = 0.20;
pub(crate) const ELEVON_FLAP: f64 = 0.40;
pub(crate) const ELEVON_LIMIT: f64 = ELEVON_PITCH + ELEVON_ROLL;
pub(crate) const CANARD_PITCH: f64 = 0.25;
pub(crate) const CANARD_ROLL: f64 = 0.15;
pub(crate) const CANARD_LIMIT: f64 = 0.35;
pub(crate) const RUDDER_TRAVEL: f64 = 0.35;

pub(crate) fn canard(address: usize) -> bool {
    CANARD_R.contains(&address) || CANARD_L.contains(&address)
}
pub(crate) fn elevon_right(address: usize) -> bool {
    ELEVON_R.contains(&address)
}
pub(crate) fn elevon_left(address: usize) -> bool {
    ELEVON_L.contains(&address)
}
pub(crate) fn rudder(address: usize) -> bool {
    RUDDER.contains(&address)
}

/// One canard angle in radians, positive raising the leading edge (and so
/// lowering the trailing edge). A nose-up command (positive elevator) raises
/// both leading edges; a roll-right command (positive aileron) lowers the
/// right canard's leading edge and raises the left's, so the right side loses
/// lift as the right wing goes down. The sum is bounded.
pub(crate) fn canard_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    (CANARD_PITCH * state.elevator.clamp(-1., 1.)
        - side * CANARD_ROLL * state.aileron.clamp(-1., 1.))
    .clamp(-CANARD_LIMIT, CANARD_LIMIT)
}

/// One elevon angle in radians, positive lowering the trailing edge, for both
/// strips of one side. Pitch raises both trailing edges for a positive
/// elevator; a positive aileron raises the right trailing edge and lowers the
/// left; pitch plus roll is bounded, then the flap droop lowers both.
pub(crate) fn elevon_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    let flight = -ELEVON_PITCH * state.elevator.clamp(-1., 1.)
        - side * ELEVON_ROLL * state.aileron.clamp(-1., 1.);
    flight.clamp(-ELEVON_LIMIT, ELEVON_LIMIT) + ELEVON_FLAP * state.flaps.clamp(0., 1.)
}

/// Reflect a face across the centre plane (the left side is the right side seen
/// in a mirror: reflect, turn, reflect back).
fn flip_x(f: &mut Face) {
    for p in &mut f.positions {
        p[0] = -p[0];
    }
    if let Some(n) = &mut f.normal {
        n[0] = -n[0];
    }
}

/// Whole-canard motion: plate and filler, left and right, turn rigidly about the
/// lateral spindle through the root chord midpoint.
pub(crate) fn canard_positions(f: &mut Face, angle: f64) {
    if angle == 0. {
        return;
    }
    let left = CANARD_L.contains(&f.address);
    let mut spindle = CANARD_SPINDLE_R;
    if left {
        flip_x(f);
        spindle = [
            -CANARD_SPINDLE_L[0],
            CANARD_SPINDLE_L[1],
            CANARD_SPINDLE_L[2],
        ];
    }
    turn(f, sp(spindle), sp(CANARD_AXIS), angle);
    if left {
        flip_x(f);
    }
}

/// Gear word 0x808c, 22 faces. Per main gear: the leg (a slanted strut plate
/// and the wheel plate across it, both windings each) and the door pair (the
/// flat root plate and the outer door wall, both windings each). The nose is
/// one group of six faces: the side plate, the cross plate and the wheel plate.
const LEG_R: [usize; 4] = [0x5bca, 0x5be9, 0x5c08, 0x5c2f];
const LEG_L: [usize; 4] = [0x59ec, 0x5a0b, 0x5a2a, 0x5a51];
const DOOR_R: [usize; 4] = [0x5b29, 0x5b40, 0x5b57, 0x5b6e];
const DOOR_L: [usize; 4] = [0x594b, 0x5962, 0x5979, 0x5990];
/// Of each door pair the outer wall (x 13 to 15, hanging) hinges on the root
/// plate (x 9 to 13, flat) along their shared edge (x 13, z -4).
const DOOR_WALL_R: [usize; 2] = [0x5b29, 0x5b40];
const DOOR_WALL_L: [usize; 2] = [0x594b, 0x5962];
const NOSE: [usize; 6] = [0x5c9b, 0x5cba, 0x5cd9, 0x5cf8, 0x5d44, 0x5d5b];
const GEAR: [usize; 22] = [
    0x5bca, 0x5be9, 0x5c08, 0x5c2f, 0x59ec, 0x5a0b, 0x5a2a, 0x5a51, 0x5b29, 0x5b40, 0x5b57, 0x5b6e,
    0x594b, 0x5962, 0x5979, 0x5990, 0x5c9b, 0x5cba, 0x5cd9, 0x5cf8, 0x5d44, 0x5d5b,
];

/// Main leg hinge (right side, shape units): a horizontal line through this
/// point whose direction is `LEG_AXIS_DEG` from the X axis toward Y, so the leg
/// swings forward and inward. The point is above the leg's top edge and behind
/// its rear edge so no corner dips below the gear-down pose while it folds.
/// Fitted (see the spec).
const LEG_HINGE: [f32; 3] = [4., -16., 0.];
const LEG_AXIS_DEG: f64 = 35.;
/// Centre of the leg's foot, the point the stow angle levels.
const LEG_FOOT: [f64; 3] = [5., -11., -10.5];
/// Main door hinges: the root plate turns a half turn about the line along Y
/// on its inboard edge (x 9, z -3), up and inboard, while the outer wall folds
/// back about the shared edge (x 13, z -4) onto the root plate, so the closed
/// pair lies flat inside the fuselage side; fitted.
const DOOR_HINGE: [f32; 3] = [9., 0., -3.];
const DOOR_FOLD_HINGE: [f32; 3] = [13., 0., -4.];
/// Nose hinge: a lateral axis through the leg's forward top corner; the leg
/// swings rearward and a little past level so the plates lie above the belly.
const NOSE_HINGE: [f32; 3] = [0., 44., -5.];
const NOSE_STOW: f64 = 100. * PI / 180.;
/// Vertical steering axis through the nose strut.
const NOSE_STEER: [f32; 3] = [0., 41., -5.];

/// Airbrake word 0x8086: right plate `62ad`/`62c4`, left plate `634a`/`6361`.
/// Each is one quad hinged on its FORWARD vertical edge (x +-9, y -38), the
/// free edge swung out to (+-13, -43).
const BRAKE_R: [usize; 2] = [0x62ad, 0x62c4];
const BRAKE_L: [usize; 2] = [0x634a, 0x6361];
const BRAKE: [usize; 4] = [0x62ad, 0x62c4, 0x634a, 0x6361];
const BRAKE_HINGE: [f32; 3] = [9., -38., 0.];
/// Rear fuselage side the closed plate lies along: from (9,-36) to (4,-56).
const SKIN: [f64; 2] = [-5., -20.];
/// Open free-edge direction from the hinge: (13,-43) minus (9,-38).
const OPEN: [f64; 2] = [4., -5.];

/// Plume word 0x8080: four double-sided plates, two crossed pairs, from the
/// nozzle exit plane y -61 back to y -85.
const FLAME: [usize; 4] = [0x63af, 0x63d6, 0x63fd, 0x6424];
const FLAME_ROOT: f32 = -61.;
const FLAME_TIP: f32 = -85.;

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

/// Stow angle of the main leg, radians: the turn about the leg hinge that lays
/// the leg's foot centre level with the hinge (worked out on the shape, so a
/// change of hinge or axis keeps the leg level).
pub(crate) fn leg_stow() -> f64 {
    let axis = LEG_AXIS_DEG.to_radians();
    let hinge = sp(LEG_HINGE);
    let v: [f64; 3] = std::array::from_fn(|i| LEG_FOOT[i] - f64::from(hinge[i]));
    let c = axis.cos() * v[1] - axis.sin() * v[0];
    f64::atan2(-v[2], c)
}

/// Fold of the outer door wall onto the root plate, radians about +Y (clockwise
/// seen from the right, through the hanging pose): from the wall's source
/// direction (2, -5) in the X-Z plane round to the direction back along the
/// root plate (-4, 1).
pub(crate) fn door_fold() -> f64 {
    (f64::atan2(-5., 2.) - f64::atan2(1., -4.)).rem_euclid(2. * PI)
}

/// Closed airbrake angle, radians (positive turns the right plate's free edge
/// toward the fuselage about +Z, negative magnitude as applied): the turn that
/// lays the plate along the rear fuselage side line.
pub(crate) fn brake_closed() -> f64 {
    (f64::atan2(SKIN[1], SKIN[0]) - f64::atan2(OPEN[1], OPEN[0])).abs()
}

/// Whole-piece gear motion. Each main leg (strut plate and wheel plate) turns
/// as one rigid piece about an oblique horizontal hinge and folds forward and
/// inward; each main door pair is two rigid plates, the root plate turning a
/// half turn about its inboard edge and the outer wall folding back on it; the nose group turns about a lateral axis at its forward top
/// corner and folds rearward, and steers about a vertical axis. The source pose
/// is gear 1. Faces are hidden at gear 0 by [`Rig::animate`].
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let closing = 1. - gear.clamp(0., 1.);
    let a = f.address;
    let left = LEG_L.contains(&a) || DOOR_L.contains(&a);
    if NOSE.contains(&a) {
        if steering != 0. {
            turn(f, sp(NOSE_STEER), sp([0., 0., 1.]), -steering);
        }
        turn(f, sp(NOSE_HINGE), sp([1., 0., 0.]), -NOSE_STOW * closing);
        return;
    }
    // The left side is the right side seen in a mirror: reflect, turn, reflect.
    if left {
        flip_x(f);
    }
    if LEG_R.contains(&a) || LEG_L.contains(&a) {
        let axis = LEG_AXIS_DEG.to_radians();
        turn(
            f,
            sp(LEG_HINGE),
            sp([axis.cos() as f32, axis.sin() as f32, 0.]),
            leg_stow() * closing,
        );
    } else {
        if DOOR_WALL_R.contains(&a) || DOOR_WALL_L.contains(&a) {
            turn(
                f,
                sp(DOOR_FOLD_HINGE),
                sp([0., 1., 0.]),
                door_fold() * closing,
            );
        }
        turn(f, sp(DOOR_HINGE), sp([0., 1., 0.]), -PI * closing);
    }
    if left {
        flip_x(f);
    }
}

/// Whole-plate airbrake motion: open (brake 1) is the source pose; closing
/// turns the plate about its forward vertical edge onto the fuselage side.
pub(crate) fn brake_positions(f: &mut Face, brake: f64) {
    let closing = 1. - brake.clamp(0., 1.);
    let right = BRAKE_R.contains(&f.address);
    let hinge = sp(if right {
        BRAKE_HINGE
    } else {
        [-BRAKE_HINGE[0], BRAKE_HINGE[1], BRAKE_HINGE[2]]
    });
    // The right plate's free edge turns clockwise (about +Z) toward the side
    // line, the left plate's counterclockwise.
    let sign = if right { -1. } else { 1. };
    turn(f, hinge, sp([0., 0., 1.]), sign * brake_closed() * closing);
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("GRI.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("GRI.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
    let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed GRI.SH branch {word:x}").into());
    }
    Ok(pose)
}

/// Every face of `ids` in `shape` is a four-corner quad whose corners all lie
/// in the reviewed box (shape units, inclusive) and which together reach every
/// side of it: the rig refuses a shape that differs from the review.
fn quads_in_box(
    shape: &Shape,
    ids: &[usize],
    low: [f32; 3],
    high: [f32; 3],
    what: &str,
) -> AppResult<()> {
    let (low, high) = (sp(low), sp(high));
    let (mut seen_low, mut seen_high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for &address in ids {
        let face = one(shape, address)?;
        if face.positions.len() != 4 {
            return Err(format!("unreviewed GRI.SH {what} face {address:x}").into());
        }
        for p in &face.positions {
            for k in 0..3 {
                if p[k] < low[k] - 1e-4 || p[k] > high[k] + 1e-4 {
                    return Err(format!("unreviewed GRI.SH {what} face {address:x}").into());
                }
                seen_low[k] = seen_low[k].min(p[k]);
                seen_high[k] = seen_high[k].max(p[k]);
            }
        }
    }
    if (0..3).any(|k| (seen_low[k] - low[k]).abs() > 1e-4 || (seen_high[k] - high[k]).abs() > 1e-4)
    {
        return Err(format!("unreviewed GRI.SH {what} extent").into());
    }
    Ok(())
}

/// The face's vertices are exactly the reviewed corners (same set, no other
/// vertex), in any order and winding.
fn corners_are(shape: &Shape, address: usize, corners: &[[f32; 3]], what: &str) -> AppResult<()> {
    let face = one(shape, address)?;
    let wanted: Vec<[f32; 3]> = corners.iter().map(|&p| sp(p)).collect();
    if face.positions.len() != wanted.len()
        || !face.positions.iter().all(|p| wanted.contains(p))
        || !wanted.iter().all(|p| face.positions.contains(p))
    {
        return Err(format!("unreviewed GRI.SH {what} face {address:x}").into());
    }
    Ok(())
}

/// Every vertex of the face is one of the reviewed points.
fn corners_within(shape: &Shape, address: usize, points: &[[f32; 3]], what: &str) -> AppResult<()> {
    let face = one(shape, address)?;
    if !matches!(face.positions.len(), 3 | 4)
        || !face
            .positions
            .iter()
            .all(|p| points.iter().any(|&q| *p == sp(q)))
    {
        return Err(format!("unreviewed GRI.SH {what} face {address:x}").into());
    }
    Ok(())
}

/// The vertices of one wing's elevon strips (inboard quad pair, outboard triangle
/// pairs): the hinge line y -27 at z -2 and z -1, the seam at x 26 (right) or
/// -25 (left), the inboard trailing corner and the outboard trailing corner.
fn elevon_points(right: bool) -> Vec<[f32; 3]> {
    let (root, seam, tip) = if right {
        (12., 26., 40.)
    } else {
        (-12., -25., -40.)
    };
    vec![
        [root, -27., -2.],
        [root, -27., -1.],
        [root, -33., -2.],
        [seam, -27., -2.],
        [seam, -27., -1.],
        [seam, -32., -2.],
        [tip, -27., -2.],
        [tip, -31., -2.],
    ]
}

pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 28848
            || shape.faces.len() != 383
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed GRI.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // Control surfaces (GRI-3): the rig refuses a shape that differs from the
        // review. Each canard is exactly its plate and its filler; each elevon
        // face lies on the reviewed strip points; the rudder is the reviewed quad.
        for (ids, plate, filler) in [
            (CANARD_R, &CANARD_PLATE_R[..], &CANARD_FILLER_R[..]),
            (CANARD_L, &CANARD_PLATE_L[..], &CANARD_FILLER_L[..]),
        ] {
            corners_are(&shape, ids[0], plate, "canard plate")?;
            corners_are(&shape, ids[1], plate, "canard plate")?;
            corners_are(&shape, ids[2], filler, "canard filler")?;
            corners_are(&shape, ids[3], filler, "canard filler")?;
        }
        for (ids, right) in [(&ELEVON_R, true), (&ELEVON_L, false)] {
            let points = elevon_points(right);
            for &address in ids {
                corners_within(&shape, address, &points, "elevon")?;
            }
        }
        for &address in &RUDDER {
            corners_are(&shape, address, &RUDDER_CORNERS, "rudder")?;
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words remove exactly one wing's six strip faces and add
        // nothing, 0x8098 the left and 0x809e the right; the rudder word swaps
        // the neutral quad for a turned copy. Pinned for GRI-3; never set.
        branch(bytes, &original, 0x8098, &[], &ELEVON_L)?;
        branch(bytes, &original, 0x809e, &[], &ELEVON_R)?;
        branch(bytes, &original, 0x80a4, &RUDDER_WORD, &RUDDER)?;
        // The gear, airbrake and plume words add exactly the reviewed plates
        // and remove nothing.
        let gear = branch(bytes, &original, 0x808c, &GEAR, &[])?.scaled(axis);
        let brake = branch(bytes, &original, 0x8086, &BRAKE, &[])?.scaled(axis);
        let flame = branch(bytes, &original, 0x8080, &FLAME, &[])?.scaled(axis);
        // Each group sits in its reviewed box and reaches all of it.
        quads_in_box(&gear, &LEG_R, [2., -14., -16.], [7., -8., -5.], "right leg")?;
        // The left wheel plate sits one unit nearer the centreline than the
        // right one mirrored (x -6 to -1 against 2 to 7): source asymmetry.
        quads_in_box(
            &gear,
            &LEG_L,
            [-6., -14., -16.],
            [-1., -8., -5.],
            "left leg",
        )?;
        quads_in_box(
            &gear,
            &DOOR_R,
            [9., -14., -9.],
            [15., 1., -3.],
            "right door",
        )?;
        quads_in_box(
            &gear,
            &DOOR_L,
            [-15., -14., -9.],
            [-9., 1., -3.],
            "left door",
        )?;
        quads_in_box(&gear, &NOSE, [-2., 38., -15.], [2., 44., -5.], "nose")?;
        quads_in_box(
            &brake,
            &BRAKE_R,
            [9., -43., -4.],
            [13., -38., 1.],
            "right brake",
        )?;
        quads_in_box(
            &brake,
            &BRAKE_L,
            [-13., -43., -4.],
            [-9., -38., 1.],
            "left brake",
        )?;
        // The plume is two crossed pairs of plates; the exit plane is the root
        // that stays.
        let (mut root, mut tip) = (f32::NEG_INFINITY, f32::INFINITY);
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 {
                return Err(format!("unreviewed GRI.SH plume plate {address:x}").into());
            }
            for p in &face.positions {
                root = root.max(p[1]);
                tip = tip.min(p[1]);
            }
        }
        if root != sp([0., FLAME_ROOT, 0.])[1] || tip != sp([0., FLAME_TIP, 0.])[1] {
            return Err("unreviewed GRI.SH plume extent".into());
        }
        let wanted = |address: usize| {
            GEAR.contains(&address) || BRAKE.contains(&address) || FLAME.contains(&address)
        };
        for source in gear
            .faces
            .into_iter()
            .chain(brake.faces)
            .chain(flame.faces)
            .filter(|f| wanted(f.address) && !original.contains(&f.address))
        {
            shape.faces.push(source);
        }
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if canard(a) {
            // Plate and filler turn together about the lateral spindle.
            canard_positions(&mut result, canard_angle(CANARD_R.contains(&a), state));
        } else if elevon_right(a) || elevon_left(a) {
            // One angle per side, shared by the inboard and the outboard strip
            // (they share the seam), turns every vertex about the hinge line.
            let angle = elevon_angle(elevon_right(a), state);
            if angle != 0. {
                turn(&mut result, sp(ELEVON_HINGE), sp(ELEVON_AXIS), angle);
            }
        } else if rudder(a) {
            // The whole quad (both windings) about its vertical leading edge:
            // a positive command swings the trailing edge toward +X.
            let angle = RUDDER_TRAVEL * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(&mut result, sp(RUDDER_HINGE), sp(RUDDER_AXIS), angle);
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
            brake_positions(&mut result, state.brake);
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            // The plume grows from the nozzle exit plane; every plate scales
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
/// The faces the nozzle material and the contrail read: the two exit discs at
/// y -61 (subtype 0x64), the octagon above z -3 and the hexagon below it.
pub(crate) const NOZZLE: [usize; 2] = [0x49a6, 0x49d5];

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    /// A synthetic face whose vertices are named in shape units.
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        let positions: Vec<[f32; 3]> = points.iter().map(|&p| sp(p)).collect();
        Face {
            address,
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([0., 0., 1.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn mirror(points: &[[f32; 3]]) -> Vec<[f32; 3]> {
        points.iter().map(|p| [-p[0], p[1], p[2]]).collect()
    }
    /// The right leg's two plates, in shape units.
    fn leg() -> Vec<Face> {
        let strut = [
            [4., -8., -5.],
            [4., -14., -5.],
            [6., -14., -16.],
            [6., -8., -16.],
        ];
        let wheel = [
            [2., -11., -5.],
            [2., -11., -16.],
            [7., -11., -16.],
            [7., -11., -5.],
        ];
        vec![
            face(0x5bca, &strut),
            face(0x5c08, &wheel),
            face(0x5be9, &strut.into_iter().rev().collect::<Vec<_>>()),
            face(0x5c2f, &wheel.into_iter().rev().collect::<Vec<_>>()),
        ]
    }
    fn doors() -> Vec<Face> {
        let outer = [
            [13., 1., -4.],
            [13., -14., -4.],
            [15., -14., -9.],
            [15., 1., -9.],
        ];
        let root = [
            [9., -14., -3.],
            [13., -14., -4.],
            [13., 1., -4.],
            [9., 1., -3.],
        ];
        vec![face(0x5b29, &outer), face(0x5b57, &root)]
    }
    fn nose() -> Vec<Face> {
        vec![
            face(
                0x5c9b,
                &[
                    [0., 38., -15.],
                    [0., 38., -5.],
                    [0., 44., -5.],
                    [0., 44., -15.],
                ],
            ),
            face(
                0x5cd9,
                &[
                    [1., 40., -15.],
                    [1., 41., -5.],
                    [-1., 41., -5.],
                    [-1., 40., -15.],
                ],
            ),
            face(
                0x5d44,
                &[
                    [2., 42., -11.],
                    [-2., 42., -11.],
                    [-2., 43., -5.],
                    [2., 43., -5.],
                ],
            ),
        ]
    }
    fn posed(faces: &[Face], gear: f64, steer: f64) -> Vec<Face> {
        faces
            .iter()
            .map(|f| {
                let mut f = f.clone();
                gear_positions(&mut f, gear, steer);
                f
            })
            .collect()
    }
    fn points(faces: &[Face]) -> Vec<[f32; 3]> {
        faces.iter().flat_map(|f| f.positions.clone()).collect()
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
    /// The reviewed control-surface faces in shape units (the retail corners).
    fn canard_faces(right: bool) -> Vec<Face> {
        let plate = if right {
            CANARD_PLATE_R.to_vec()
        } else {
            CANARD_PLATE_L.to_vec()
        };
        let filler = if right {
            CANARD_FILLER_R.to_vec()
        } else {
            CANARD_FILLER_L.to_vec()
        };
        let ids = if right { CANARD_R } else { CANARD_L };
        let rev = |v: &[[f32; 3]]| v.iter().rev().copied().collect::<Vec<_>>();
        vec![
            face(ids[0], &plate),
            face(ids[1], &rev(&plate)),
            face(ids[2], &filler),
            face(ids[3], &rev(&filler)),
        ]
    }
    fn elevon_faces(right: bool) -> Vec<Face> {
        let s = if right { 1. } else { -1. };
        let (seam, ids) = if right {
            (26., ELEVON_R)
        } else {
            (-25., ELEVON_L)
        };
        let root = s * 12.;
        let tip = s * 40.;
        vec![
            // Inboard quad, both layers.
            face(
                ids[0],
                &[
                    [root, -27., -2.],
                    [root, -33., -2.],
                    [seam, -32., -2.],
                    [seam, -27., -2.],
                ],
            ),
            face(
                ids[1],
                &[
                    [root, -33., -2.],
                    [root, -27., -1.],
                    [seam, -27., -1.],
                    [seam, -32., -2.],
                ],
            ),
            // Outboard triangles.
            face(
                ids[2],
                &[[seam, -32., -2.], [tip, -31., -2.], [seam, -27., -2.]],
            ),
            face(
                ids[3],
                &[[seam, -27., -1.], [tip, -31., -2.], [seam, -32., -2.]],
            ),
            face(
                ids[4],
                &[[tip, -31., -2.], [tip, -27., -2.], [seam, -27., -2.]],
            ),
            face(
                ids[5],
                &[[tip, -31., -2.], [seam, -27., -1.], [tip, -27., -2.]],
            ),
        ]
    }
    fn rudder_faces() -> Vec<Face> {
        let quad = RUDDER_CORNERS.to_vec();
        let rev: Vec<_> = quad.iter().rev().copied().collect();
        vec![face(RUDDER[0], &quad), face(RUDDER[1], &rev)]
    }
    fn animated(rig: &Rig, faces: &[Face], s: &State) -> Vec<Face> {
        faces.iter().map(|f| rig.animate(f, s).unwrap()).collect()
    }
    #[test]
    fn the_flap_words_and_rudder_word_are_pinned_and_every_surface_is_neutral_at_rest() {
        // 6 elevon strips per wing, a 2 face rudder quad and its 2 face copy.
        assert_eq!(ELEVON_L.len() + ELEVON_R.len(), 12);
        assert!(ELEVON_L.iter().all(|a| !ELEVON_R.contains(a)));
        assert_eq!(RUDDER.len(), RUDDER_WORD.len());
        let rig = Rig;
        let s = state();
        let all: Vec<Face> = canard_faces(true)
            .into_iter()
            .chain(canard_faces(false))
            .chain(elevon_faces(true))
            .chain(elevon_faces(false))
            .chain(rudder_faces())
            .collect();
        for f in &all {
            assert_eq!(
                rig.animate(f, &s).unwrap().positions,
                f.positions,
                "{:x} is drawn at its source pose at rest",
                f.address
            );
        }
    }
    #[test]
    fn each_canard_turns_whole_about_its_root_midpoint_with_plate_and_filler() {
        let rig = Rig;
        for right in [true, false] {
            let faces = canard_faces(right);
            let spindle = sp(if right {
                CANARD_SPINDLE_R
            } else {
                CANARD_SPINDLE_L
            });
            for (pitch, roll) in [
                (1., 0.),
                (-1., 0.),
                (0., 1.),
                (0., -1.),
                (0.5, -0.5),
                (1., 1.),
            ] {
                let mut s = state();
                s.elevator = pitch;
                s.aileron = roll;
                let moved = animated(&rig, &faces, &s);
                // One rigid body: plate and filler together.
                let want = points(&faces);
                let got = points(&moved);
                assert!(spread(&want, &got) < 2e-4, "rigid at {pitch}/{roll}");
                // The shared vertex of plate and filler stays shared.
                let root = sp([if right { 10. } else { -10. }, 19., 1.]);
                let ends: Vec<[f32; 3]> = faces
                    .iter()
                    .zip(&moved)
                    .flat_map(|(a, b)| {
                        a.positions
                            .iter()
                            .zip(&b.positions)
                            .filter(|(p, _)| **p == root)
                            .map(|(_, q)| *q)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                assert_eq!(ends.len(), 4);
                for q in &ends {
                    for k in 0..3 {
                        assert!((q[k] - ends[0][k]).abs() < 1e-5);
                    }
                }
                // The spindle point is the invariant: a point there stays.
                let mut probe = face(CANARD_R[0], &[[0., 0., 0.]]);
                probe.address = if right { CANARD_R[0] } else { CANARD_L[0] };
                probe.positions = vec![spindle];
                let at = rig.animate(&probe, &s).unwrap().positions[0];
                for k in 0..3 {
                    assert!((at[k] - spindle[k]).abs() < 1e-4, "spindle moved");
                }
            }
        }
    }
    #[test]
    fn the_canards_pitch_together_and_roll_differentially() {
        let rig = Rig;
        let lead = |right: bool, s: &State| {
            // The forward root corner, the leading edge point nearest the nose.
            let x = if right { 8. } else { -7. };
            let faces = canard_faces(right);
            let moved = animated(&rig, &faces, s);
            let i = faces[0]
                .positions
                .iter()
                .position(|p| *p == sp([x, 37., 1.]))
                .unwrap();
            moved[0].positions[i][2] - faces[0].positions[i][2]
        };
        let trail = |right: bool, s: &State| {
            let x = if right { 22. } else { -22. };
            let faces = canard_faces(right);
            let moved = animated(&rig, &faces, s);
            let i = faces[0]
                .positions
                .iter()
                .position(|p| *p == sp([x, 15., 2.]))
                .unwrap();
            moved[0].positions[i][2] - faces[0].positions[i][2]
        };
        let mut s = state();
        // Nose up: both leading edges rise, both tips fall.
        s.elevator = 1.;
        for right in [true, false] {
            assert!(lead(right, &s) > 0.5 && trail(right, &s) < -0.5);
        }
        // Left and right turn by the same angle for pitch (the left forward root
        // corner is a unit nearer the spindle, so its rise is a little less).
        assert!((canard_angle(true, &s) - canard_angle(false, &s)).abs() < 1e-12);
        assert!((lead(true, &s) - lead(false, &s)).abs() < 0.5);
        s.elevator = -1.;
        for right in [true, false] {
            assert!(lead(right, &s) < -0.5 && trail(right, &s) > 0.5);
        }
        // Roll right (positive aileron): the right leading edge falls, the left
        // rises; roll left is the reverse.
        s.elevator = 0.;
        s.aileron = 1.;
        assert!(lead(true, &s) < -0.3 && lead(false, &s) > 0.3);
        assert!(trail(true, &s) > 0.3 && trail(false, &s) < -0.3);
        s.aileron = -1.;
        assert!(lead(true, &s) > 0.3 && lead(false, &s) < -0.3);
        // Pitch plus roll: the angles add and the sum is bounded.
        let mut s = state();
        s.elevator = 1.;
        s.aileron = -1.;
        assert!((canard_angle(true, &s) - CANARD_LIMIT).abs() < 1e-12);
        assert!((canard_angle(false, &s) - (CANARD_PITCH - CANARD_ROLL)).abs() < 1e-12);
        s.elevator = 0.5;
        s.aileron = -0.5;
        assert!(
            (canard_angle(true, &s) - 0.5 * (CANARD_PITCH + CANARD_ROLL)).abs() < 1e-12,
            "inside the bound the terms simply add"
        );
        // The flap lever and the rudder do not move the canards.
        let mut s = state();
        s.flaps = 1.;
        s.rudder = 1.;
        assert_eq!(canard_angle(true, &s), 0.);
        assert_eq!(canard_angle(false, &s), 0.);
    }
    #[test]
    fn both_strips_of_each_elevon_take_one_angle_and_stay_one_rigid_wedge() {
        let rig = Rig;
        for right in [true, false] {
            let faces = elevon_faces(right);
            for (e, a, f) in [
                (1., 0., 0.),
                (-1., 0., 0.),
                (0., 1., 0.),
                (0., -1., 0.),
                (0., 0., 1.),
                (0.5, -1., 1.),
                (1., 1., 0.5),
            ] {
                let mut s = state();
                s.elevator = e;
                s.aileron = a;
                s.flaps = f;
                let moved = animated(&rig, &faces, &s);
                assert!(
                    spread(&points(&faces), &points(&moved)) < 2e-4,
                    "rigid at {e}/{a}/{f}"
                );
                // The seam vertices are the same point in both strips.
                let seam_x = if right { 26. } else { -25. };
                let mut seen: Vec<[f32; 3]> = Vec::new();
                for (src, out) in faces.iter().zip(&moved) {
                    for (p, q) in src.positions.iter().zip(&out.positions) {
                        if *p == sp([seam_x, -27., -2.]) {
                            seen.push(*q);
                        }
                    }
                }
                assert!(seen.len() >= 3);
                for q in &seen {
                    for k in 0..3 {
                        assert!((q[k] - seen[0][k]).abs() < 1e-5, "seam opened");
                    }
                }
                // The midpoint of every hinge pair stays on the hinge line.
                for x in [12., seam_x.abs()] {
                    let x = if right { x } else { -x };
                    let find = |z: f32| -> [f32; 3] {
                        for (src, out) in faces.iter().zip(&moved) {
                            for (p, q) in src.positions.iter().zip(&out.positions) {
                                if *p == sp([x, -27., z]) {
                                    return *q;
                                }
                            }
                        }
                        panic!("hinge vertex ({x},-27,{z}) missing");
                    };
                    let (a, b) = (find(-2.), find(-1.));
                    let mid = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2., (a[2] + b[2]) / 2.];
                    let want = sp([x, -27., -1.5]);
                    for k in 0..3 {
                        assert!((mid[k] - want[k]).abs() < 1e-4, "hinge pair midpoint");
                    }
                }
            }
        }
    }
    #[test]
    fn the_elevons_follow_the_fleet_signs_and_the_clamp() {
        let trailing = |right: bool, s: &State| {
            let rig = Rig;
            let faces = elevon_faces(right);
            let x = if right { 40. } else { -40. };
            let moved = animated(&rig, &faces, s);
            let i = faces[2]
                .positions
                .iter()
                .position(|p| *p == sp([x, -31., -2.]))
                .unwrap();
            moved[2].positions[i][2] - faces[2].positions[i][2]
        };
        let mut s = state();
        s.elevator = 1.;
        // Nose up raises both trailing edges.
        assert!(trailing(true, &s) > 0.5 && trailing(false, &s) > 0.5);
        s.elevator = -1.;
        assert!(trailing(true, &s) < -0.5 && trailing(false, &s) < -0.5);
        // Roll right raises the right trailing edge and lowers the left.
        let mut s = state();
        s.aileron = 1.;
        assert!(trailing(true, &s) > 0.5 && trailing(false, &s) < -0.5);
        // Flaps lower both.
        let mut s = state();
        s.flaps = 1.;
        assert!(trailing(true, &s) < -0.5 && trailing(false, &s) < -0.5);
        // Pitch plus roll is bounded before the flap droop.
        let mut s = state();
        s.elevator = 1.;
        s.aileron = 1.;
        assert!((elevon_angle(true, &s) + ELEVON_LIMIT).abs() < 1e-12);
        s.flaps = 1.;
        assert!((elevon_angle(true, &s) - (ELEVON_FLAP - ELEVON_LIMIT)).abs() < 1e-12);
        // The aileron law is odd about the centre line.
        let mut s = state();
        s.aileron = 0.4;
        assert!((elevon_angle(true, &s) + elevon_angle(false, &s)).abs() < 1e-12);
    }
    #[test]
    fn the_rudder_quad_turns_whole_about_its_leading_edge() {
        let rig = Rig;
        let faces = rudder_faces();
        for command in [1., -1., 0.4] {
            let mut s = state();
            s.rudder = command;
            let moved = animated(&rig, &faces, &s);
            assert!(spread(&points(&faces), &points(&moved)) < 2e-4);
            for (src, out) in faces.iter().zip(&moved) {
                for (p, q) in src.positions.iter().zip(&out.positions) {
                    if p[1] == sp([0., -43., 0.])[1] {
                        // The leading edge (y -43) is the hinge and stays.
                        for k in 0..3 {
                            assert!((p[k] - q[k]).abs() < 1e-4, "hinge moved");
                        }
                    } else {
                        // A positive command swings the trailing edge to +X;
                        // heights are unchanged.
                        assert!((q[0] - p[0]) * (command as f32).signum() > 0.5);
                        assert!((q[2] - p[2]).abs() < 1e-4);
                    }
                }
            }
        }
    }
    #[test]
    fn the_gear_down_pose_is_the_source_pose_and_gear_up_hides_every_plate() {
        let rig = Rig;
        let mut s = state();
        s.gear = 1.;
        for f in leg().iter().chain(&doors()).chain(&nose()) {
            let drawn = rig.animate(f, &s).unwrap();
            for (a, b) in drawn.positions.iter().zip(&f.positions) {
                for k in 0..3 {
                    assert!((a[k] - b[k]).abs() < 1e-5);
                }
            }
        }
        s.gear = 0.;
        for f in leg().iter().chain(&doors()).chain(&nose()) {
            assert!(rig.animate(f, &s).is_none());
        }
    }
    #[test]
    fn each_gear_piece_turns_rigidly_at_every_gear_value() {
        let (legs, doors, noses) = (leg(), doors(), nose());
        for gear in [1., 0.9, 0.6, 0.3, 0.05, 1e-4] {
            // The leg and the nose are one piece each; the door wall and the
            // root plate are one piece each.
            let pieces = [
                legs.clone(),
                vec![doors[0].clone()],
                vec![doors[1].clone()],
                noses.clone(),
            ];
            for piece in &pieces {
                let want = points(piece);
                let got = points(&posed(piece, gear, 0.3));
                assert!(spread(&want, &got) < 2e-4, "gear {gear}");
            }
        }
    }
    #[test]
    fn the_stowed_main_leg_lies_level_inside_the_fuselage_and_never_dips() {
        let legs = leg();
        let start_low = points(&legs)
            .iter()
            .map(|p| p[2])
            .fold(f32::INFINITY, f32::min);
        assert_eq!(start_low, -16.);
        for step in 0..=100 {
            let gear = f64::from(step) / 100.;
            let low = points(&posed(&legs, gear, 0.))
                .iter()
                .map(|p| p[2])
                .fold(f32::INFINITY, f32::min);
            assert!(low >= start_low - 1e-3, "gear {gear} dips to {low}");
        }
        let stowed = points(&posed(&legs, 1e-9, 0.));
        for p in &stowed {
            assert!(p[2] > -6. && p[0].abs() < 9., "{p:?} is outside the hull");
        }
        // The foot centre is level with the hinge when stowed, forward of its
        // gear-down position and inboard of it.
        let foot = {
            let mut f = face(0x5c08, &[[5., -11., -10.5]]);
            gear_positions(&mut f, 1e-9, 0.);
            f.positions[0]
        };
        assert!((foot[2] - LEG_HINGE[2]).abs() < 1e-3);
        assert!(foot[1] > -11. && foot[0] < 5.);
    }
    #[test]
    fn the_left_gear_is_the_exact_mirror_of_the_right() {
        for gear in [1., 0.7, 0.2, 0.01] {
            let right = posed(&leg(), gear, 0.);
            let left_src: Vec<Face> = [
                (0x59ec, mirror(&[[4., -8., -5.], [4., -14., -5.]])),
                (0x5a2a, mirror(&[[2., -11., -5.], [7., -11., -16.]])),
            ]
            .into_iter()
            .map(|(a, pts)| {
                let mut p = pts;
                p.push([0., -9., -9.]);
                face(a, &p)
            })
            .collect();
            let mut mirrored = left_src.clone();
            for f in &mut mirrored {
                gear_positions(f, gear, 0.);
            }
            // A left point and its right twin, turned, are mirror images.
            let r = {
                let mut f = face(0x5bca, &[[4., -8., -5.], [4., -14., -5.], [0., -9., -9.]]);
                gear_positions(&mut f, gear, 0.);
                f
            };
            for (m, rp) in mirrored[0].positions.iter().zip(&r.positions) {
                assert!((m[0] + rp[0]).abs() < 1e-4, "{m:?} {rp:?}");
                assert!((m[1] - rp[1]).abs() < 1e-4 && (m[2] - rp[2]).abs() < 1e-4);
            }
            assert_eq!(right.len(), 4);
        }
    }
    #[test]
    fn the_door_wall_stays_on_the_root_plate_edge_and_lies_on_it_when_stowed() {
        for gear in [1., 0.8, 0.5, 0.2, 1e-9] {
            let pair = posed(&doors(), gear, 0.);
            // The shared edge (13, 1, -4) and (13, -14, -4) stays together.
            for y in [1., -14.] {
                let ends: Vec<[f32; 3]> = pair
                    .iter()
                    .flat_map(|f| {
                        let src = doors()
                            .into_iter()
                            .find(|d| d.address == f.address)
                            .unwrap();
                        f.positions
                            .iter()
                            .zip(src.positions)
                            .filter(move |(_, s)| *s == sp([13., y, -4.]))
                            .map(|(p, _)| *p)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                assert_eq!(ends.len(), 2);
                for (a, b) in ends[0].iter().zip(&ends[1]) {
                    assert!((a - b).abs() < 1e-4, "gear {gear}");
                }
            }
        }
        // Stowed, the whole pair is inside the side of the fuselage (x 9) apart
        // from the wall's 1.3 unit overshoot past the hinge, above the belly.
        for p in points(&posed(&doors(), 1e-9, 0.)) {
            assert!(p[0] <= 10.5 && p[2] > -5. && p[2] < 4., "{p:?}");
        }
    }
    #[test]
    fn the_nose_folds_above_the_belly() {
        for p in points(&posed(&nose(), 1e-9, 0.)) {
            assert!(p[2] >= -5.0001, "{p:?}");
            assert!(p[1] < 46. && p[1] > 33., "{p:?}");
        }
    }
    #[test]
    fn the_nose_folds_aft_and_steers_about_its_strut() {
        let mut f = face(0x5c9b, &[[0., 38., -15.]]);
        gear_positions(&mut f, 0.5, 0.);
        assert!(f.positions[0][1] < 38.);
        // The foot of the wheel plate steers to the side by the wheel angle.
        let mut f = face(0x5d44, &[[2., 42., -11.]]);
        gear_positions(&mut f, 1., 0.2);
        assert!(f.positions[0][1] != 42. && (f.positions[0][2] + 11.).abs() < 1e-4);
        let mut f = face(0x5d44, &[[2., 42., -11.]]);
        gear_positions(&mut f, 1., 0.);
        assert_eq!(f.positions[0], sp([2., 42., -11.]));
    }
    fn plate(right: bool) -> Face {
        let s = if right { 1. } else { -1. };
        face(
            if right { 0x62ad } else { 0x634a },
            &[
                [s * 13., -43., 0.],
                [s * 9., -38., 1.],
                [s * 9., -38., -4.],
                [s * 13., -43., -3.],
            ],
        )
    }
    #[test]
    fn each_airbrake_plate_turns_whole_about_its_forward_edge_onto_the_side() {
        let rig = Rig;
        for right in [true, false] {
            let source = plate(right);
            let mut s = state();
            s.brake = 1.;
            assert_eq!(
                rig.animate(&source, &s).unwrap().positions,
                source.positions
            );
            s.brake = 0.;
            assert!(rig.animate(&source, &s).is_none());
            let side = if right { 1. } else { -1. };
            for brake in [0.75, 0.5, 0.1, 1e-6] {
                s.brake = brake;
                let got = rig.animate(&source, &s).unwrap();
                assert!(spread(&source.positions, &got.positions) < 2e-4);
                // The forward edge is the hinge and does not move.
                for k in [1, 2] {
                    let p = got.positions[k];
                    let q = source.positions[k];
                    assert!((0..3).all(|i| (p[i] - q[i]).abs() < 1e-4), "hinge {k}");
                }
                // The free edge closes inboard and never swings further out.
                for k in [0, 3] {
                    assert!(got.positions[k][0] * side < source.positions[k][0] * side);
                }
            }
            // Closed, the free edge lies along the rear fuselage side line
            // (9,-36) to (4,-56), one half unit outside it (the hinge is).
            s.brake = 1e-9;
            let got = rig.animate(&source, &s).unwrap();
            let free = got.positions[0];
            let line_x = |y: f32| 9. + (y + 36.) * 0.25;
            assert!(
                (free[0] * side - line_x(free[1]) - 0.5).abs() < 0.12,
                "{free:?}"
            );
        }
    }
    #[test]
    fn the_plume_grows_from_the_one_nozzle_exit_plane() {
        let rig = Rig;
        let source = face(
            0x63af,
            &[
                [0., -85., -4.],
                [0., -85., 1.],
                [0., -61., 1.],
                [0., -61., -4.],
            ],
        );
        let mut s = state();
        s.exhaust = 0.;
        assert!(rig.animate(&source, &s).is_none());
        s.exhaust = 1.;
        assert_eq!(
            rig.animate(&source, &s).unwrap().positions,
            source.positions
        );
        s.exhaust = 0.5;
        let half = rig.animate(&source, &s).unwrap();
        assert_eq!(half.positions[2][1], -61.);
        assert_eq!(half.positions[0][1], -73.);
        assert!(flame(0x6424) && !flame(0x62ad));
    }
    #[test]
    fn the_nozzle_faces_are_the_two_exit_discs() {
        assert_eq!(NOZZLE.len(), 2);
        assert!(
            NOZZLE
                .iter()
                .all(|a| !GEAR.contains(a) && !FLAME.contains(a))
        );
    }
}
