//! Independent B-1B whole-panel witnesses (B1B-3): the swing wing (17 faces a
//! side turned as one panel about the shared vertex line (+-10, 11)), the two
//! flap strips and their caps, the two tailerons, the rudder, the spoilers,
//! the gear plates, the interim bay walls and the plumes. Measures the shared
//! Airframe output and never repeats the rig's algorithm: the witnesses are
//! named source vertices, rigid-body distances and the seam on the pivot line.
//! Witness points are named in shape units (the B-1B scale is the identity,
//! 4/3 ft a unit is the probe's `scale`).
use super::*;
use crate::b1b_animation::{CAP_L, CAP_R, GEAR, STRIP_L, STRIP_R, TAIL_L, TAIL_R, WING_L, WING_R};

const RUDDER: [usize; 2] = [0x4c6b, 0x4c8a];
const SPOILER_R: [usize; 2] = [0x52fa, 0x5311];
const SPOILER_L: [usize; 2] = [0x5328, 0x533f];
const NOSE: [usize; 4] = [0x51a7, 0x51c6, 0x51e5, 0x5204];
const MAIN_R: [usize; 4] = [0x5025, 0x5044, 0x5063, 0x5082];
const MAIN_L: [usize; 4] = [0x50e6, 0x5105, 0x5124, 0x5143];
const BAY: [usize; 8] = [
    0x41fc, 0x4213, 0x422a, 0x4241, 0x4258, 0x426f, 0x4286, 0x429d,
];
const FLAME: [usize; 16] = [
    0x4d70, 0x4d97, 0x4dbe, 0x4de5, 0x4e0c, 0x4e33, 0x4e5a, 0x4e81, 0x4ea8, 0x4ecf, 0x4ef6, 0x4f1d,
    0x4f44, 0x4f6b, 0x4f92, 0x4fb9,
];

fn mirror(p: [f32; 3]) -> [f32; 3] {
    [-p[0], p[1], p[2]]
}

/// One side of the aircraft.
struct Side {
    wing: &'static [usize],
    strip: &'static [usize],
    cap: usize,
    tail: &'static [usize],
    spoiler: &'static [usize],
    right: bool,
}
const SIDES: [Side; 2] = [
    Side {
        wing: &WING_R,
        strip: &STRIP_R,
        cap: CAP_R,
        tail: &TAIL_R,
        spoiler: &SPOILER_R,
        right: true,
    },
    Side {
        wing: &WING_L,
        strip: &STRIP_L,
        cap: CAP_L,
        tail: &TAIL_L,
        spoiler: &SPOILER_L,
        right: false,
    },
];
impl Side {
    /// A right-hand source point on this side.
    fn fix(&self, p: [f32; 3]) -> [f32; 3] {
        if self.right { p } else { mirror(p) }
    }
    fn sign(&self) -> f32 {
        if self.right { 1. } else { -1. }
    }
    fn pivot(&self) -> [f32; 2] {
        [self.sign() * 10., 11.]
    }
}

fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

/// Largest change of any vertex-to-vertex distance inside one group (feet),
/// in all three axes, or in the plan view only (`planar`).
fn rigidity_of(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, planar: bool) -> f32 {
    let before = keyed(reference);
    let after = keyed(pose);
    let (mut old, mut now) = (Vec::new(), Vec::new());
    for (key, face) in before.iter().filter(|(k, _)| ids.contains(&k.0)) {
        let Some(after) = after.get(key) else {
            return f32::INFINITY;
        };
        if face.positions.len() != after.positions.len() {
            return f32::INFINITY;
        }
        old.extend(face.positions.iter().copied());
        now.extend(after.positions.iter().copied());
    }
    let d = |a: [f32; 3], b: [f32; 3]| {
        if planar {
            (a[0] - b[0]).hypot(a[1] - b[1])
        } else {
            distance(a, b)
        }
    };
    let mut error = 0f32;
    for i in 0..old.len() {
        for j in i + 1..old.len() {
            error = error.max((d(old[i], old[j]) - d(now[i], now[j])).abs() * scale);
        }
    }
    error
}
fn rigidity(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    rigidity_of(reference, pose, ids, scale, false)
}

fn pinned(reference: &[Face], pose: &[Face], ids: &[usize], point: [f32; 3], scale: f32) -> f32 {
    let old = group(reference, ids);
    let moved = f4::witness_positions(&old, &keyed(pose), point);
    if moved.is_empty() {
        f32::INFINITY
    } else {
        moved
            .iter()
            .map(|q| distance(point, *q) * scale)
            .fold(0f32, f32::max)
    }
}

fn pin(m: &mut Metrics, gap: f32) {
    m.reviewed_anchor_missing |= !gap.is_finite();
    if gap.is_finite() {
        m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
    }
}

/// Distance the midpoint of one vertex pair moved (feet).
fn pair_gap(
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    pair: ([f32; 3], [f32; 3]),
    scale: f32,
) -> Option<f32> {
    let find = |p: [f32; 3]| f4::witness_positions(old, new, p).into_iter().next();
    let (a, b) = (find(pair.0)?, find(pair.1)?);
    let middle: [f32; 3] = std::array::from_fn(|k| (pair.0[k] + pair.1[k]) / 2.);
    let now: [f32; 3] = std::array::from_fn(|k| (a[k] + b[k]) / 2.);
    Some(distance(middle, now) * scale)
}

fn pair_pin(
    m: &mut Metrics,
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    pair: ([f32; 3], [f32; 3]),
    scale: f32,
) {
    match pair_gap(old, new, pair, scale) {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
}

/// The fold check of rule 11: faces of a group that keep some vertices while
/// others move (a vertex on `hinge` may stay). Counts the faces that fold.
fn folds(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    hinge: &[[f32; 3]],
    scale: f32,
) -> usize {
    let before = keyed(reference);
    let after = keyed(pose);
    let mut count = 0;
    for (key, face) in before.iter().filter(|(k, _)| ids.contains(&k.0)) {
        let Some(moved) = after.get(key) else {
            count += 1;
            continue;
        };
        let (mut stayed, mut turned) = (false, false);
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            if hinge.contains(a) {
                continue;
            }
            if distance(*a, *b) * scale > EPSILON {
                turned = true;
            } else {
                stayed = true;
            }
        }
        count += usize::from(stayed && turned);
    }
    count
}

/// Faces outside every moving group that changed at all.
fn fixed_moved(reference: &[Face], pose: &[Face], movers: &[&[usize]], scale: f32) -> usize {
    let before = keyed(reference);
    let after = keyed(pose);
    before
        .iter()
        .filter(|(k, _)| !movers.iter().any(|ids| ids.contains(&k.0)))
        .filter(|(k, face)| {
            after.get(k).is_none_or(|moved| {
                moved.positions.len() != face.positions.len()
                    || face
                        .positions
                        .iter()
                        .zip(&moved.positions)
                        .any(|(a, b)| distance(*a, *b) * scale > EPSILON)
            })
        })
        .count()
}

fn unchanged(old: &BTreeMap<FaceKey, &Face>, new: &BTreeMap<FaceKey, &Face>) -> bool {
    old.iter().all(|(key, face)| {
        new.get(key)
            .is_some_and(|after| face.positions == after.positions)
    })
}

fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}
fn highest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::NEG_INFINITY, f32::max)
}
fn present(pose: &[Face], ids: &[usize]) -> usize {
    pose.iter().filter(|f| ids.contains(&f.address)).count()
}
fn face_at(pose: &[Face], address: usize) -> Option<&Face> {
    pose.iter().find(|f| f.address == address)
}

/// The taileron's trailing tip: the source's left quad ends at x -14, the
/// right one at 15.
fn tail_tip(side: &Side) -> [f32; 3] {
    [if side.right { 15. } else { -14. }, -40., 10.]
}

/// The taileron checks shared by the control sweep and the grid: whole quad
/// rigid, the root chord's midpoint (the spindle) stays, and the trailing tip
/// moves up for a positive `net` (up positive) command.
fn tail(side: &Side, net: f32, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.tail);
    let new = group(pose, side.tail);
    let error = rigidity(reference, pose, side.tail, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    pair_pin(m, &old, &new, ([0., -35., 10.], [0., -22., 10.]), scale);
    if net == 0. {
        m.reviewed_neutral_mismatch |= !unchanged(&old, &new);
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.tail, &[], scale);
    let witness = tail_tip(side);
    let qs = f4::witness_positions(&old, &new, witness);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        let delta = (q[2] - witness[2]) * scale;
        m.reviewed_direction_failures += usize::from(delta * net.signum() <= EPSILON);
    }
}

/// How open a spoiler should be (the restated mapping the probe checks the
/// drawing against): the brake plus 0.6 of the roll on the side going down.
fn expected_open(right: bool, roll: f64, brake: f64) -> f64 {
    let side = if right { roll.max(0.) } else { (-roll).max(0.) };
    (brake + 0.6 * side).min(1.)
}

/// The free-edge vertex height (shape units) of a spoiler's first winding.
fn spoiler_free_z(pose: &[Face], side: &Side) -> Option<f32> {
    // The first winding's free-edge vertex at (+-6, -4, 7): index 1 on the
    // right plate, 3 on the left (the mirrored winding).
    face_at(pose, side.spoiler[0]).map(|f| f.positions[if side.right { 1 } else { 3 }][2])
}

/// Spoiler checks against the open (source) pose `open_pose` at the fractions
/// `open = [right, left]`: drawn exactly when open, rigid, the hinge edge
/// stays on the glove skin, the free edge rises with the fraction.
fn spoilers(
    open: [f64; 2],
    reference: Option<&[Face]>,
    pose: &[Face],
    scale: f32,
    m: &mut Metrics,
) {
    for (side, fraction) in SIDES.iter().zip(open) {
        let drawn = present(pose, side.spoiler);
        m.reviewed_direction_failures += usize::from((drawn == 2) != (fraction > 0.));
        m.reviewed_direction_failures += usize::from(drawn != 0 && drawn != 2);
        let Some(reference) = reference else {
            continue;
        };
        if fraction <= 0. || drawn != 2 {
            continue;
        }
        // The reference holds both plates open (brake 1); they are rigid
        // between it and every other pose and the hinge edge stays.
        let error = rigidity(reference, pose, side.spoiler, scale);
        m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
        for p in [side.fix([4., 0., 4.]), side.fix([6., 0., 1.])] {
            pin(m, pinned(reference, pose, side.spoiler, p, scale));
        }
        let (now, open_z) = (
            spoiler_free_z(pose, side).unwrap_or(f32::NAN),
            spoiler_free_z(reference, side).unwrap_or(f32::NAN),
        );
        // Fully open is the source pose; less open is lower.
        if fraction >= 1. {
            m.reviewed_direction_failures += usize::from((now - open_z).abs() * scale > EPSILON);
        } else {
            m.reviewed_direction_failures += usize::from(now.is_nan() || now >= open_z - EPSILON);
        }
    }
}

/// The turned wing: every face moved as one panel (plan-view rigid, the slab
/// at most the fitted thinning), the shared pivot-line vertices stay, the tip
/// went aft and in, and no fixed face moved.
fn wing(side: &Side, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.wing);
    let new = group(pose, side.wing);
    let error = rigidity_of(reference, pose, side.wing, scale, true);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The slab thins by at most 0.03 units a side (0.04 ft): nothing moves in
    // height by more than that.
    let thin = old
        .iter()
        .flat_map(|(k, f)| {
            let new = &new;
            f.positions.iter().enumerate().map(move |(i, p)| {
                new.get(k)
                    .map_or(f32::INFINITY, |n| (n.positions[i][2] - p[2]).abs())
            })
        })
        .fold(0f32, f32::max);
    m.reviewed_direction_failures += usize::from(thin > 0.0301 || thin.is_nan());
    // The pivot-line vertices stay exactly.
    for z in [-1., 0., 1.] {
        let p = [side.pivot()[0], side.pivot()[1], z];
        let gap = pinned(reference, pose, side.wing, p, scale);
        pin(m, gap);
    }
    if !side_turned(side, &old, &new) {
        return;
    }
    let tip = side.fix([51., 2., 0.]);
    let qs = f4::witness_positions(&old, &new, tip);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        // Aft, and in toward the fuselage.
        m.reviewed_direction_failures += usize::from(q[1] >= tip[1] - EPSILON / scale);
        m.reviewed_direction_failures += usize::from(q[0].abs() >= tip[0].abs() - EPSILON / scale);
    }
}

fn side_turned(
    _side: &Side,
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
) -> bool {
    old.iter().any(|(k, f)| {
        new.get(k).is_some_and(|n| {
            f.positions
                .iter()
                .zip(&n.positions)
                .any(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) > 1e-4)
        })
    })
}

/// Flap strips: rigid wedges hinged on the leading-edge mid-line (the mid-points
/// of the (13, 1) and (42, 0) top and bottom pairs stay), the trailing edge
/// goes down and forward, the cap is drawn exactly while the flaps are out and
/// shares the strip's end vertices.
fn strips(side: &Side, value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.strip);
    let new = group(pose, side.strip);
    let error = rigidity(reference, pose, side.strip, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    for (top, bottom) in [
        ([13., 1., 1.], [13., 1., -1.]),
        ([42., 0., 1.], [42., 0., -1.]),
    ] {
        pair_pin(m, &old, &new, (side.fix(top), side.fix(bottom)), scale);
    }
    let cap = face_at(pose, side.cap);
    m.reviewed_direction_failures += usize::from(cap.is_some() != (value > 0.));
    if value == 0. {
        m.reviewed_neutral_mismatch |= !unchanged(&old, &new);
        return;
    }
    let witness = side.fix([13., -3., 0.]);
    let qs = f4::witness_positions(&old, &new, witness);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from((q[2] - witness[2]) * scale >= -EPSILON);
        m.reviewed_direction_failures += usize::from(q[1] <= witness[1] - EPSILON);
    }
    // The cap's three vertices are the strip's own end vertices, moved with it.
    if let (Some(cap), Some(top), Some(bottom)) = (
        cap,
        face_at(pose, side.strip[0]),
        face_at(pose, side.strip[1]),
    ) {
        let strip: Vec<[f32; 3]> = top
            .positions
            .iter()
            .chain(&bottom.positions)
            .copied()
            .collect();
        m.reviewed_direction_failures += cap
            .positions
            .iter()
            .filter(|c| !strip.iter().any(|s| distance(**c, *s) * scale <= EPSILON))
            .count();
    }
}

fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, &RUDDER);
    let new = group(pose, &RUDDER);
    let error = rigidity(reference, pose, &RUDDER, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    let hinge = [[0., -30., 10.], [0., -34., 18.]];
    for point in hinge {
        pin(m, pinned(reference, pose, &RUDDER, point, scale));
    }
    if value == 0. {
        m.reviewed_neutral_mismatch |= !unchanged(&old, &new);
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, &RUDDER, &hinge, scale);
    // The trailing vertices swing to +X for a positive command.
    for witness in [[0., -35., 10.], [0., -37., 18.]] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures +=
                usize::from((q[0] - witness[0]) * value.signum() as f32 <= EPSILON);
        }
    }
}

/// The gear plates stay whole rigid pieces, stow inboard (mains) and aft
/// (nose), never hang below the gear-down pose, and at stow lie inside the
/// belly (z -1 up to the roof).
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= present(pose, &GEAR) != 0;
        return;
    }
    for ids in [&NOSE[..], &MAIN_R[..], &MAIN_L[..]] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
    }
    let (reference_low, pose_low) = (lowest(reference, &GEAR), lowest(pose, &GEAR));
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // The plate's foot: 8 units under the reference point (10.67 ft).
        m.reviewed_wheel_failed |= (pose_low + 8.).abs() > 1e-3;
        return;
    }
    // Direction: a main's foot moves inboard, the nose's foot moves aft.
    for (ids, witness, inward) in [
        (&MAIN_R[..], [2., -5., -8.], -1f32),
        (&MAIN_L[..], [-2., -5., -8.], 1.),
    ] {
        let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from((q[0] - witness[0]) * inward <= 0.);
        }
    }
    let foot = [0., 44., -8.];
    let qs = f4::witness_positions(&group(reference, &NOSE), &keyed(pose), foot);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from(q[1] >= foot[1]);
    }
    if value < 1e-5 {
        // Near stow every piece lies inside the closed belly: at or above the
        // belly polygon (z -1) and under the roof skin (z 6 at the centre).
        for ids in [&NOSE[..], &MAIN_R[..], &MAIN_L[..]] {
            m.reviewed_wheel_failed |= lowest(pose, ids) < -1. - 1e-3;
            m.reviewed_wheel_failed |= highest(pose, ids) > 6. + 1e-3;
        }
        // The nose leg stays on the centre line section (x within 2).
        m.reviewed_wheel_failed |= pose
            .iter()
            .filter(|f| NOSE.contains(&f.address))
            .flat_map(|f| &f.positions)
            .any(|p| p[0].abs() > 2. + 1e-3);
    }
}

/// Interim bay walls: drawn only while the bay is open, the belly edge (z -1)
/// stays on the belly polygon, the lowest point is -1 - 3 value.
fn bay(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures += usize::from(present(pose, &BAY) != 0);
        return;
    }
    m.reviewed_direction_failures += usize::from(present(pose, &BAY) != 8);
    for point in [
        [3., 21., -1.],
        [3., 10., -1.],
        [3., 22., -1.],
        [3., 32., -1.],
        [-3., 10., -1.],
        [-3., 32., -1.],
    ] {
        pin(m, pinned(reference, pose, &BAY, point, scale));
    }
    let want = -1. - 3. * value as f32;
    m.reviewed_direction_failures += usize::from((lowest(pose, &BAY) - want).abs() > 1e-3);
}

/// Plumes: drawn only while the exhaust is on, rooted on the nozzle plane
/// (y -17), the tip at -17 - 11 value.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures += usize::from(present(pose, &FLAME) != 0);
        return;
    }
    m.reviewed_direction_failures += usize::from(present(pose, &FLAME) != 16);
    for point in [
        [8., -17., -3.],
        [-8., -17., -3.],
        [5., -17., -2.],
        [-5., -17., -3.],
        [6., -17., -3.],
        [-9., -17., -3.],
    ] {
        pin(m, pinned(reference, pose, &FLAME, point, scale));
    }
    let tip = pose
        .iter()
        .filter(|f| FLAME.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    m.reviewed_direction_failures += usize::from((tip - (-17. - 11. * value as f32)).abs() > 1e-3);
}

pub(super) fn check(
    c: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    m: &mut Metrics,
) {
    let (_, reference, pose) = geometry;
    match c {
        Control::Elevator | Control::Aileron => {
            for side in &SIDES {
                // Elevator up raises both; a positive aileron raises the right.
                let net = if value == 0. {
                    0.
                } else if matches!(c, Control::Aileron) {
                    side.sign() * value.signum() as f32
                } else {
                    value.signum() as f32
                };
                tail(side, net, reference, pose, scale, m);
            }
            let mut movers: Vec<&[usize]> = vec![&TAIL_R, &TAIL_L];
            if matches!(c, Control::Aileron) {
                // The roll spoilers open on the side going down.
                spoilers(
                    [
                        expected_open(true, value, 0.),
                        expected_open(false, value, 0.),
                    ],
                    None,
                    pose,
                    scale,
                    m,
                );
                movers.extend([&SPOILER_R[..], &SPOILER_L[..]]);
            }
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &movers, scale) != 0;
        }
        Control::Flaps => {
            for side in &SIDES {
                strips(side, value, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |= fixed_moved(
                reference,
                pose,
                &[&STRIP_R, &STRIP_L, &[CAP_R], &[CAP_L]],
                scale,
            ) != 0;
        }
        Control::Rudder => {
            rudder(value, reference, pose, scale, m);
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&RUDDER], scale) != 0;
        }
        Control::Sweep => {
            for side in &SIDES {
                wing(side, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |=
                fixed_moved(reference, pose, &[&WING_R, &WING_L], scale) != 0;
            if value == 0. {
                m.reviewed_neutral_mismatch |= geometry_hash(reference) != geometry_hash(pose);
            }
        }
        Control::Gear => gear(value, reference, pose, scale, m),
        Control::Bay => bay(value, reference, pose, scale, m),
        Control::Exhaust => exhaust(value, reference, pose, scale, m),
        Control::Brake => {
            // Reference is brake 1 (both spoilers open).
            spoilers([value, value], Some(reference), pose, scale, m);
        }
        _ => {}
    }
}

fn passed(m: &Metrics) -> bool {
    m.finite
        && !m.reviewed_anchor_missing
        && m.max_reviewed_anchor_gap <= EPSILON
        && m.max_reviewed_skin_gap <= EPSILON
        && m.reviewed_rigid_panel_error <= EPSILON
        && m.reviewed_direction_failures == 0
        && !m.reviewed_neutral_mismatch
        && m.new_planar_crossings.is_empty()
}

fn at_knots(neutral: &State, knots: f64) -> State {
    let mut state = neutral.clone();
    state.speed = knots * 1.68781;
    state
}

/// Angle (radians) the wing tip turned about its pivot between two poses, the
/// witness that measures the sweep without repeating the schedule.
fn tip_turn(reference: &[Face], pose: &[Face], side: &Side) -> Option<f64> {
    let old = group(reference, side.wing);
    let new = group(pose, side.wing);
    let tip = side.fix([51., 2., 0.]);
    let q = *f4::witness_positions(&old, &new, tip).first()?;
    let pivot = side.pivot();
    let a = f64::from(tip[1] - pivot[1]).atan2(f64::from(tip[0] - pivot[0]));
    let b = f64::from(q[1] - pivot[1]).atan2(f64::from(q[0] - pivot[0]));
    let d = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    Some(if side.right { -d } else { d })
}

/// The widest the wing reaches across the aircraft in a pose, feet (the
/// swept span: twice the largest |x| of every wing vertex).
fn span_ft(pose: &[Face], scale: f32) -> f32 {
    2. * pose
        .iter()
        .filter(|f| WING_R.contains(&f.address) || WING_L.contains(&f.address))
        .flat_map(|f| f.positions.iter().map(|p| p[0].abs()))
        .fold(0f32, f32::max)
        * scale
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let spoiler_all: Vec<usize> = SPOILER_R.iter().chain(&SPOILER_L).copied().collect();
    // The 25-pose pitch and roll grid at the two ends of the sweep.
    for (knots, name) in [(200., "15deg"), (650., "67deg")] {
        let spread = at_knots(neutral, knots);
        let original = airframe.animation_faces(&spread);
        let mut rows = String::from(
            "pitch,roll,finite,anchor_gap_ft,rigidity_ft,fixed_faces_moved,crossings,direction_failures\n",
        );
        let mut poses = Vec::new();
        for pitch in [-1., -0.5, 0., 0.5, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                let mut state = spread.clone();
                state.elevator = pitch;
                state.aileron = roll;
                let pose = airframe.animation_faces(&state);
                let mut m = measure(&original, &pose, scale);
                for side in &SIDES {
                    // The law's weights are 0.16 for pitch and 0.09 for roll
                    // (clamped at 0.22, which neither sum reaches in sign).
                    let net = (16. * pitch + 9. * f64::from(side.sign()) * roll) as f32;
                    tail(side, net, &original, &pose, scale, &mut m);
                }
                spoilers(
                    [
                        expected_open(true, roll, 0.),
                        expected_open(false, roll, 0.),
                    ],
                    None,
                    &pose,
                    scale,
                    &mut m,
                );
                let fixed = fixed_moved(&original, &pose, &[&TAIL_R, &TAIL_L, &spoiler_all], scale);
                // The wing (swept or spread, with the roll spoilers on the
                // glove) never moves with pitch or roll.
                let wing_moved = group(&original, &WING_R).iter().any(|(k, f)| {
                    group(&pose, &WING_R)
                        .get(k)
                        .is_none_or(|n| n.positions != f.positions)
                });
                m.reviewed_neutral_mismatch |= fixed != 0 || wing_moved;
                writeln!(
                    rows,
                    "{pitch},{roll},{},{},{},{fixed},{},{}",
                    m.finite,
                    m.max_reviewed_anchor_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
                if !passed(&m) {
                    failures.push(format!(
                        "B-1B {name} pitch{pitch}/roll{roll}: whole-quad spindle, rigidity, spoiler or direction witness"
                    ));
                }
                poses.push(pose);
            }
        }
        fs::write(
            out.join(format!("pitch-roll-combinations-{name}.csv")),
            rows,
        )?;
        contact_sheet(
            &out.join(format!("pitch-roll-combinations-{name}.ppm")),
            &original,
            &poses,
        )?;
    }

    // The sweep against the speed schedule and the flaps.
    let original = airframe.animation_faces(&at_knots(neutral, 200.));
    let mut rows = String::from(
        "knots,flaps,expected_turn_deg,measured_turn_deg_right,measured_turn_deg_left,planar_rigidity_ft,pivot_gap_ft,swept_span_ft,passed\n",
    );
    let mut poses = Vec::new();
    for knots in [200., 280., 340., 400., 460., 520., 600.] {
        for flaps in [0., 0.25, 0.5, 0.75, 1.] {
            let mut state = at_knots(neutral, knots);
            state.flaps = flaps;
            let pose = airframe.animation_faces(&state);
            let m = measure(&original, &pose, scale);
            // The documented law: spread to 280 kt, fully swept (52.5 degree
            // turn) from 520 kt, flaps hold it spread.
            let expected = ((knots - 280.) / (520. - 280.)).clamp(0., 1.) * 52.5 * (1. - flaps);
            let mut measured = [0f64; 2];
            let mut rigid = 0f32;
            let mut gap = 0f32;
            for (k, side) in SIDES.iter().enumerate() {
                // The panel without its strip, which droops with the flaps.
                let panel: Vec<usize> = side
                    .wing
                    .iter()
                    .copied()
                    .filter(|a| !side.strip.contains(a))
                    .collect();
                rigid = rigid.max(rigidity_of(&original, &pose, &panel, scale, true));
                for z in [-1., 0.] {
                    let p = [side.pivot()[0], side.pivot()[1], z];
                    gap = gap.max(pinned(&original, &pose, side.wing, p, scale));
                }
                measured[k] = tip_turn(&original, &pose, side)
                    .unwrap_or(f64::NAN)
                    .to_degrees();
            }
            let ok = m.finite
                && rigid <= EPSILON
                && gap <= EPSILON
                && m.new_planar_crossings.is_empty()
                && measured.iter().all(|t| (t - expected).abs() < 0.01);
            writeln!(
                rows,
                "{knots},{flaps},{expected:.3},{:.3},{:.3},{rigid},{gap},{:.2},{ok}",
                measured[0],
                measured[1],
                span_ft(&pose, scale)
            )?;
            if !ok {
                failures.push(format!(
                    "B-1B sweep {knots} kt/flaps {flaps}: expected {expected:.2} deg, measured {:.2} / {:.2}, rigidity {rigid}, pivot gap {gap}",
                    measured[0], measured[1]
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("sweep-flap-combinations.csv"), rows)?;
    contact_sheet(&out.join("sweep-flap-combinations.ppm"), &original, &poses)?;

    // The swept panel at each leading-edge sweep: the span narrows
    // monotonically, the seam on the pivot line stays closed, and the wings
    // are mirrors of each other.
    let mut rows = String::from(
        "sweep_deg,turn_deg,swept_span_ft,tip_x_ft,tip_y_ft,pivot_gap_ft,mirror_error_ft,root_inboard_min_x_ft\n",
    );
    let mut last_span = f32::INFINITY;
    for degrees in [15., 20., 25., 30., 35., 40., 45., 50., 55., 60., 65., 67.5] {
        let knots = tore_sim::combat::swing::B1B.speed_for_sweep(degrees) / 1.68781;
        let pose = airframe.animation_faces(&at_knots(neutral, knots));
        let span = span_ft(&pose, scale);
        let mut gap = 0f32;
        let mut tip = [0f32; 3];
        for side in &SIDES {
            for z in [-1., 0.] {
                let p = [side.pivot()[0], side.pivot()[1], z];
                gap = gap.max(pinned(&original, &pose, side.wing, p, scale));
            }
            if side.right {
                let old = group(&original, side.wing);
                if let Some(q) = f4::witness_positions(&old, &keyed(&pose), [51., 2., 0.]).first() {
                    tip = *q;
                }
            }
        }
        let points = |ids: &[usize]| -> Vec<[f32; 3]> {
            pose.iter()
                .filter(|f| ids.contains(&f.address))
                .flat_map(|f| f.positions.iter().copied())
                .collect()
        };
        let (right, left) = (points(&WING_R), points(&WING_L));
        let nearest = |p: [f32; 3], set: &[[f32; 3]]| {
            set.iter()
                .map(|q| distance(mirror(p), *q))
                .fold(f32::INFINITY, f32::min)
        };
        let mirrored = right
            .iter()
            .map(|p| nearest(*p, &left))
            .fold(0f32, f32::max)
            * scale;
        let root_in = right.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) * scale;
        writeln!(
            rows,
            "{degrees},{},{span},{},{},{gap},{mirrored},{root_in}",
            degrees - 15.,
            tip[0] * scale,
            tip[1] * scale
        )?;
        if gap > EPSILON {
            failures.push(format!(
                "B-1B pivot-line seam opens at {degrees} deg: {gap} ft"
            ));
        }
        if mirrored > 1e-3 {
            failures.push(format!(
                "B-1B wings are not mirrored at {degrees} deg: {mirrored} ft"
            ));
        }
        if span > last_span + 1e-3 {
            failures.push(format!("B-1B span widens at {degrees} deg"));
        }
        last_span = span;
    }
    fs::write(out.join("sweep-seam.csv"), rows)?;

    // Roll against the speed brake: each spoiler open exactly as the mapping
    // says and the same fraction always draws the same height.
    let mut rows = String::from(
        "roll,brake,open_right,open_left,finite,anchor_gap_ft,rigidity_ft,crossings,direction_failures\n",
    );
    let mut poses = Vec::new();
    let mut heights: BTreeMap<i64, f32> = BTreeMap::new();
    let mut brake_one = neutral.clone();
    brake_one.brake = 1.;
    let open_pose = airframe.animation_faces(&brake_one);
    let base = airframe.animation_faces(neutral);
    for roll in [-1., -0.5, 0., 0.5, 1.] {
        for brake in [0., 0.25, 0.5, 0.75, 1.] {
            let mut state = neutral.clone();
            state.aileron = roll;
            state.brake = brake;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&base, &pose, scale);
            let open = [
                expected_open(true, roll, brake),
                expected_open(false, roll, brake),
            ];
            spoilers(open, Some(&open_pose), &pose, scale, &mut m);
            for (side, fraction) in SIDES.iter().zip(open) {
                if let Some(z) = spoiler_free_z(&pose, side) {
                    let entry = heights
                        .entry((fraction * 1000.).round() as i64)
                        .or_insert(z);
                    if (*entry - z).abs() * scale > EPSILON {
                        failures.push(format!(
                            "B-1B spoiler height differs for open fraction {fraction}"
                        ));
                    }
                }
            }
            writeln!(
                rows,
                "{roll},{brake},{},{},{},{},{},{},{}",
                open[0],
                open[1],
                m.finite,
                m.max_reviewed_anchor_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !(m.finite
                && !m.reviewed_anchor_missing
                && m.max_reviewed_anchor_gap <= EPSILON
                && m.reviewed_rigid_panel_error <= EPSILON
                && m.reviewed_direction_failures == 0
                && m.new_planar_crossings.is_empty())
            {
                failures.push(format!(
                    "B-1B roll{roll}/brake{brake}: spoiler hinge, rigidity or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("roll-brake-combinations.csv"), rows)?;
    contact_sheet(&out.join("roll-brake-combinations.ppm"), &base, &poses)?;
    // The free edge rises as the fraction grows.
    let rising: Vec<f32> = heights.values().copied().collect();
    if rising.windows(2).any(|w| w[1] < w[0] - 1e-4) {
        failures.push("B-1B spoiler free edge falls as the open fraction grows".into());
    }

    // Seams at full travel, reported not failed: the flap strip and the
    // rudder against the fixed neighbours.
    let mut rows = String::from("surface,command,gap_ft\n");
    let corner = |ids: &[usize], pose: &[Face], point: [f32; 3]| -> f32 {
        let old = group(&original, ids);
        f4::witness_positions(&old, &keyed(pose), point)
            .first()
            .map_or(f32::NAN, |q| distance(point, *q) * scale)
    };
    for (name, rudder_command) in [("rudder-right", 1.), ("rudder-left", -1.)] {
        let mut state = at_knots(neutral, 200.);
        state.rudder = rudder_command;
        let pose = airframe.animation_faces(&state);
        // The rudder's trailing-bottom corner shares (0, -35, 10) with the
        // two taileron roots: how far it leaves them.
        writeln!(
            rows,
            "{name},full,{}",
            corner(&RUDDER, &pose, [0., -35., 10.])
        )?;
    }
    for side in &SIDES {
        let mut state = at_knots(neutral, 200.);
        state.flaps = 1.;
        let pose = airframe.animation_faces(&state);
        writeln!(
            rows,
            "flap-strip-{},full,{}",
            if side.right { "right" } else { "left" },
            corner(side.strip, &pose, side.fix([13., -3., 0.]))
        )?;
    }
    for (name, elevator, aileron) in [
        ("pitch-up", 1., 0.),
        ("pitch-down", -1., 0.),
        ("roll-right", 0., 1.),
        ("pitch-up+roll-right", 1., 1.),
    ] {
        let mut state = at_knots(neutral, 200.);
        state.elevator = elevator;
        state.aileron = aileron;
        let pose = airframe.animation_faces(&state);
        for side in &SIDES {
            writeln!(
                rows,
                "taileron-{}-{name},trailing tip,{}",
                if side.right { "right" } else { "left" },
                corner(side.tail, &pose, tail_tip(side))
            )?;
        }
    }
    fs::write(out.join("seam-opening.csv"), rows)?;

    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    Ok(failures)
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, never lower than gear down, grounded at 1 and tucked near 0.
fn dense_gear(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut gear_endpoint = neutral.clone();
    Control::Gear.apply(&mut gear_endpoint, 1.);
    let reference = airframe.animation_faces(&gear_endpoint);
    let mut csv = String::from("value,anchor_gap_ft,rigidity_ft,crossings,checks_passed\n");
    let mut values: Vec<_> = std::iter::once(1e-6)
        .chain((0..=200).map(|i| f64::from(i) / 200.))
        .collect();
    values.sort_by(f64::total_cmp);
    let mut poses = Vec::new();
    for (i, value) in values.into_iter().enumerate() {
        let mut state = neutral.clone();
        state.gear = value;
        let pose = airframe.animation_faces(&state);
        let mut m = measure(&reference, &pose, scale);
        check(
            Control::Gear,
            value,
            (original, &reference, &pose),
            scale,
            &mut m,
        );
        let ok = m.finite
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{},{ok}",
            m.max_reviewed_anchor_gap,
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !ok {
            failures.push(format!(
                "B-1B dense gear value{value}: rigid, direction, stow or ground gate"
            ));
        }
        write_obj(&out.join(format!("dense-gear-{i}.obj")), &pose)?;
        if i <= 1 || i % 10 == 1 {
            poses.push(pose);
        }
    }
    fs::write(out.join("dense-gear.csv"), csv)?;
    contact_sheet(&out.join("dense-gear.ppm"), &reference, &poses)?;
    Ok(failures)
}
