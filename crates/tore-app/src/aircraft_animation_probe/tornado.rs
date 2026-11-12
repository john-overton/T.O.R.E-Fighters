//! Independent Tornado whole-panel witnesses (TOR-3): the swing wing (13 faces a
//! side turned as one rigid panel about the glove pivot), the two tailerons, the
//! trailing flap strips and the rudder block. Measures the shared Airframe
//! output and never repeats the rig's algorithm: the witnesses are named source
//! vertices, rigid-body distances and the glove seam. TOR-4 adds the three rigid
//! gear pieces (each main leg and the nose leg, folding forward), the two
//! airbrake plates and the two afterburner plumes, with a dense gear sweep, a
//! brake sweep and an exhaust sweep.
use super::*;
use crate::tornado_animation::{
    PIVOT_L, PIVOT_R, STRIP_L, STRIP_R, TAIL_L, TAIL_R, WING_L, WING_R,
};

const RUDDER: [usize; 2] = [0x5723, 0x5750];
const MAIN_L: [usize; 10] = [
    0x4705, 0x4724, 0x4743, 0x4762, 0x4781, 0x4798, 0x47af, 0x47c6, 0x47dd, 0x47fc,
];
const MAIN_R: [usize; 10] = [
    0x4908, 0x4927, 0x4946, 0x4965, 0x4984, 0x499b, 0x49b2, 0x49c9, 0x49e0, 0x49ff,
];
const NOSE: [usize; 6] = [0x4a97, 0x4ab6, 0x4ad5, 0x4af4, 0x4b13, 0x4b2a];
const GEAR: [usize; 26] = [
    0x4705, 0x4724, 0x4743, 0x4762, 0x4781, 0x4798, 0x47af, 0x47c6, 0x47dd, 0x47fc, 0x4908, 0x4927,
    0x4946, 0x4965, 0x4984, 0x499b, 0x49b2, 0x49c9, 0x49e0, 0x49ff, 0x4a97, 0x4ab6, 0x4ad5, 0x4af4,
    0x4b13, 0x4b2a,
];
const BRAKE_R: [usize; 2] = [0x57ec, 0x5803];
const BRAKE_L: [usize; 2] = [0x5889, 0x58a0];
const BRAKE: [usize; 4] = [0x57ec, 0x5803, 0x5889, 0x58a0];
const FLAME: [usize; 8] = [
    0x591e, 0x593d, 0x595c, 0x597b, 0x599a, 0x59b9, 0x59d8, 0x59f7,
];
/// Shape units to feet (the probe scale, header scale 2).
const FEET: f32 = 2. / 3.;

fn mirror(p: [f32; 3]) -> [f32; 3] {
    [-p[0], p[1], p[2]]
}

/// One side of the aircraft: its wing, strip and taileron faces.
struct Side {
    wing: &'static [usize],
    strip: &'static [usize],
    tail: &'static [usize],
    right: bool,
}
const SIDES: [Side; 2] = [
    Side {
        wing: &WING_R,
        strip: &STRIP_R,
        tail: &TAIL_R,
        right: true,
    },
    Side {
        wing: &WING_L,
        strip: &STRIP_L,
        tail: &TAIL_L,
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
    /// The glove vertices shared with the fixed fillets (x, y, z), as the shape
    /// has them: the left glove is one unit further out.
    fn glove(&self) -> [[f32; 3]; 5] {
        let x = |a: f32, b: f32| if self.right { a } else { b };
        [
            [x(9., -10.), 1., 0.],
            [x(9., -10.), 1., 1.],
            [x(10., -11.), -1., 0.],
            [x(10., -11.), -1., 1.],
            [x(10., -11.), -4., 0.],
        ]
    }
    /// The wing tip vertex, the witness of the sweep direction.
    fn tip(&self) -> [f32; 3] {
        [if self.right { 33. } else { -34. }, -10., 0.]
    }
    fn strip_tip(&self) -> [f32; 3] {
        [if self.right { 30. } else { -31. }, -7., 0.]
    }
    fn strip_trailing(&self) -> [f32; 3] {
        [if self.right { 30. } else { -31. }, -9., 0.]
    }
    fn strip_root(&self) -> ([f32; 3], [f32; 3]) {
        let x = if self.right { 10. } else { -11. };
        ([x, -1., 0.], [x, -1., 1.])
    }
}

fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

/// Largest change of any vertex-to-vertex distance inside one group (feet).
fn rigidity(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
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
    let mut error = 0f32;
    for i in 0..old.len() {
        for j in i + 1..old.len() {
            error = error.max((distance(old[i], old[j]) - distance(now[i], now[j])).abs() * scale);
        }
    }
    error
}

/// Skin gap inside one group: shared vertices of its faces must move together.
fn group_skin_gap(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
}

/// How far a vertex that stays on a hinge moved (feet); infinity when missing.
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

/// The fold check of rule 11: faces of a group that keep some vertices while
/// others move. A vertex on the hinge (`hinge`, exact source vertices) may stay
/// while the rest move; any other mix is a fold. Counts the faces that fold.
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

/// Faces outside every moving group that changed at all: a fixed face must
/// never move with a control.
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

/// The glove seam of one wing: how far each wing vertex shared with a fixed
/// fillet moved (feet), and the outboard (opening) and inboard (overlap) parts.
#[derive(Clone, Copy, Default)]
struct GloveSeam {
    separation: f32,
    opening: f32,
    overlap: f32,
}
fn glove_seam(reference: &[Face], pose: &[Face], side: &Side, scale: f32) -> GloveSeam {
    let before = keyed(reference);
    let after = keyed(pose);
    let mut seam = GloveSeam::default();
    for (key, face) in before.iter().filter(|(k, _)| side.wing.contains(&k.0)) {
        let Some(moved) = after.get(key) else {
            continue;
        };
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            if side.glove().contains(a) {
                seam.separation = seam.separation.max(distance(*a, *b) * scale);
                let outboard = (b[0] - a[0]) * side.sign() * scale;
                seam.opening = seam.opening.max(outboard);
                seam.overlap = seam.overlap.max(-outboard);
            }
        }
    }
    seam
}

/// The taileron checks shared by the control sweep and the grid. `net` is the
/// sign of the commanded trailing-edge motion (up positive) for this side.
fn tail(side: &Side, net: f32, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.tail);
    let new = group(pose, side.tail);
    group_skin_gap(reference, pose, side.tail, scale, m);
    let error = rigidity(reference, pose, side.tail, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The spindle runs through the midpoint of the root chord (x 6, y -25 to -20).
    let pair = (side.fix([6., -25., -2.]), side.fix([6., -20., -2.]));
    match pair_gap(&old, &new, pair, scale) {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
    if net == 0. {
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.tail, &[], scale);
    // The trailing tip moves up for a positive command.
    for witness in [side.fix([18., -27., -2.]), side.fix([18., -23., -2.])] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            let delta = (q[2] - witness[2]) * scale;
            m.reviewed_direction_failures += usize::from(delta * net.signum() <= EPSILON);
        }
    }
}

/// The trailing strips: rigid, hinged on the leading edge, drooping for flaps.
fn strips(side: &Side, value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.strip);
    let new = group(pose, side.strip);
    group_skin_gap(reference, pose, side.strip, scale, m);
    let error = rigidity(reference, pose, side.strip, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The outer hinge corner stays; the root pair's midpoint stays on the axis.
    let gap = pinned(reference, pose, side.strip, side.strip_tip(), scale);
    m.reviewed_anchor_missing |= !gap.is_finite();
    if gap.is_finite() {
        m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
    }
    match pair_gap(&old, &new, side.strip_root(), scale) {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
    if value == 0. {
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.strip, &[side.strip_tip()], scale);
    let witness = side.strip_trailing();
    let qs = f4::witness_positions(&old, &new, witness);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from((q[2] - witness[2]) * scale >= -EPSILON);
    }
}

fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, &RUDDER);
    let new = group(pose, &RUDDER);
    group_skin_gap(reference, pose, &RUDDER, scale, m);
    let error = rigidity(reference, pose, &RUDDER, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The two vertices on the leading edge stay.
    let hinge = [[0., -21., 4.], [0., -26., 12.]];
    for point in hinge {
        let gap = pinned(reference, pose, &RUDDER, point, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
        }
    }
    if value == 0. {
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, &RUDDER, &hinge, scale);
    // Trailing vertices swing to +X for a positive command.
    for witness in [[0., -25., 3.], [0., -31., 12.]] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures +=
                usize::from((q[0] - witness[0]) * value.signum() as f32 <= EPSILON);
        }
    }
}

/// A turned wing: every face of the panel moved as one rigid body, the tip went
/// aft and in, and no fixed face moved.
fn wing(side: &Side, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.wing);
    let new = group(pose, side.wing);
    group_skin_gap(reference, pose, side.wing, scale, m);
    let error = rigidity(reference, pose, side.wing, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    let moved = old
        .iter()
        .any(|(k, f)| new.get(k).is_some_and(|n| n.positions != f.positions));
    if !moved {
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.wing, &[], scale);
    let tip = side.tip();
    let qs = f4::witness_positions(&old, &new, tip);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        // Aft, and in toward the fuselage.
        m.reviewed_direction_failures += usize::from(q[1] >= tip[1] - EPSILON / scale);
        m.reviewed_direction_failures += usize::from(q[0].abs() >= tip[0].abs() - EPSILON / scale);
    }
}

/// The lowest z of a group in a pose (shape units).
fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}

/// Where `witness` (a vertex of the reference group `ids`) is in `pose`.
fn follow(reference: &[Face], pose: &[Face], ids: &[usize], witness: [f32; 3]) -> Vec<[f32; 3]> {
    f4::witness_positions(&group(reference, ids), &keyed(pose), witness)
}

/// Pin the named vertices of a group: they must not move at all.
fn pin(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    points: &[[f32; 3]],
    scale: f32,
    m: &mut Metrics,
) {
    for &point in points {
        let gap = pinned(reference, pose, ids, point, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
        }
    }
}

/// The bottom and top of the airframe's skin at (x, y), from the neutral pose:
/// the lowest and highest z of any face whose plan view covers the point
/// (the planar height of the face there). `None` outside every face.
fn skin_span(neutral: &[Face], x: f32, y: f32) -> Option<(f32, f32)> {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for face in neutral {
        let p = &face.positions;
        let mut inside = false;
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
                inside = !inside;
            }
        }
        if !inside {
            continue;
        }
        // The plane through the first three non-collinear vertices.
        let (p0, mut plane) = (p[0], None);
        for i in 1..p.len().saturating_sub(1) {
            let u = [p[i][0] - p0[0], p[i][1] - p0[1], p[i][2] - p0[2]];
            let v = [
                p[i + 1][0] - p0[0],
                p[i + 1][1] - p0[1],
                p[i + 1][2] - p0[2],
            ];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            if n[2].abs() > 1e-6 {
                plane = Some(n);
                break;
            }
        }
        if let Some(n) = plane {
            let z = p0[2] - (n[0] * (x - p0[0]) + n[1] * (y - p0[1])) / n[2];
            lo = lo.min(z);
            hi = hi.max(z);
        }
    }
    lo.is_finite().then_some((lo, hi))
}

/// Stowed pieces lie inside the airframe: every vertex and edge midpoint of the
/// group, nudged 0.1 units inward from the fuselage side wall, should sit between
/// the skin's bottom and top there. Returns the worst overshoot in shape units
/// (0 when every sample is inside; infinity when a sample has no skin at all).
fn hull_overshoot(neutral: &[Face], pose: &[Face], ids: &[usize]) -> f32 {
    let mut worst = 0f32;
    for face in pose.iter().filter(|f| ids.contains(&f.address)) {
        let p = &face.positions;
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            for t in [0f32, 0.5] {
                let q: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
                let x = q[0] - q[0].signum() * 0.1;
                worst = worst.max(match skin_span(neutral, x, q[1] + 1e-3) {
                    Some((lo, hi)) => (lo - q[2]).max(q[2] - hi).max(0.),
                    None => f32::INFINITY,
                });
            }
        }
    }
    worst
}

/// The gear stays three whole rigid pieces: each main leg (ten faces) and the
/// six nose faces. No vertex of a piece stays while another moves (every vertex
/// turns about a lateral axis that lies in the bay), gear down is the ground
/// contact, closing never lowers a plate, the feet fold forward, and near stow
/// every piece is inside the airframe (checked against the neutral skin in the
/// dense sweep, which supplies it).
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    for ids in [&MAIN_R[..], &MAIN_L[..], &NOSE[..]] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
        group_skin_gap(reference, pose, ids, scale, m);
        m.reviewed_direction_failures += folds(reference, pose, ids, &[], scale);
    }
    m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&GEAR], scale) != 0;
    // Closing never lowers the gear: nothing hangs below the gear-down pose.
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // Ground contact: the lowest point of the gear-down pose is the shape's
        // -11 units, 7.33 ft at the header scale 2/3 ft a unit (the low point
        // the flight model stands the aircraft on).
        m.reviewed_wheel_failed |= (pose_low * scale + 11. * FEET).abs() > 1e-3;
        return;
    }
    // A main wheel's lowest forward corner and the nose foot move forward.
    for side in [1f32, -1.] {
        let ids = if side > 0. { &MAIN_R[..] } else { &MAIN_L[..] };
        let foot = [side * 6., -1., -11.];
        let qs = follow(reference, pose, ids, foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[1] <= foot[1]);
        }
    }
    let foot = [0., 29., -11.];
    let qs = follow(reference, pose, &NOSE, foot);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from(q[1] <= foot[1]);
    }
}

/// The airbrake: one plate a side (two windings), open (1) is the source pose,
/// closing turns each whole plate about its forward edge (y -7, z 2), which
/// stays; the free edge (y -10) only comes down.
fn brake(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| BRAKE.contains(&f.address)));
        return;
    }
    m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&BRAKE], scale) != 0;
    if value == 1. {
        let old = group(reference, &BRAKE);
        let new = group(pose, &BRAKE);
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
    }
    for (ids, side) in [(&BRAKE_R[..], 1f32), (&BRAKE_L[..], -1.)] {
        group_skin_gap(reference, pose, ids, scale, m);
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
        // Two vertices on the hinge stay, two turn: no fold.
        let (near, far) = if side > 0. {
            ([5., -7., 2.], [2., -7., 2.])
        } else {
            ([-2., -7., 2.], [-6., -7., 2.])
        };
        pin(reference, pose, ids, &[near, far], scale, m);
        m.reviewed_direction_failures += folds(reference, pose, ids, &[near, far], scale);
        let free = if side > 0. {
            [[2., -10., 4.], [5., -10., 4.]]
        } else {
            [[-5., -10., 4.], [-3., -10., 4.]]
        };
        for witness in free {
            let qs = follow(reference, pose, ids, witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                // Never raised above the open pose, and level with the hinge or
                // above it (the closed pose is flat at z 2).
                m.reviewed_direction_failures +=
                    usize::from(q[2] > witness[2] + EPSILON || q[2] < 2. - EPSILON);
            }
        }
    }
}

/// Each plume grows from the nozzle exit plane (y -24): the root edge starts
/// one unit behind it and is pulled up to it as the demand falls, the tips pull
/// forward, x and z never change, and the shared edges stay together.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
        return;
    }
    m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&FLAME], scale) != 0;
    group_skin_gap(reference, pose, &FLAME, scale, m);
    let old = group(reference, &FLAME);
    let new = group(pose, &FLAME);
    let (plane, d) = (-24f32, value as f32);
    for (root, tip) in [(-25f32, -43f32)] {
        for witness in [
            [-3., root, 1.],
            [-3., root, -3.],
            [4., root, 1.],
            [4., root, -3.],
            [-5., root, -1.],
            [-2., root, -1.],
            [2., root, -1.],
            [5., root, -1.],
        ] {
            let qs = f4::witness_positions(&old, &new, witness);
            m.reviewed_anchor_missing |= qs.is_empty();
            for q in qs {
                // The root edge sits `value` units behind the exit plane.
                let off = (q[1] - (plane - d)).abs() * scale;
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(off);
                m.reviewed_direction_failures +=
                    usize::from(q[0] != witness[0] || q[2] != witness[2] || q[1] > plane + EPSILON);
            }
        }
        for witness in [
            [-3., tip, 1.],
            [-3., tip, -3.],
            [4., tip, 1.],
            [4., tip, -3.],
            [-5., tip, -1.],
            [-2., tip, -1.],
            [2., tip, -1.],
            [5., tip, -1.],
        ] {
            let qs = f4::witness_positions(&old, &new, witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                let want = plane + (tip - plane) * d;
                m.max_reviewed_anchor_gap =
                    m.max_reviewed_anchor_gap.max((q[1] - want).abs() * scale);
                if value < 1. {
                    m.reviewed_direction_failures += usize::from(q[1] <= tip + EPSILON);
                }
            }
        }
    }
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
            m.reviewed_neutral_mismatch |=
                fixed_moved(reference, pose, &[&TAIL_R, &TAIL_L], scale) != 0;
        }
        Control::Flaps => {
            for side in &SIDES {
                strips(side, value, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |=
                fixed_moved(reference, pose, &[&STRIP_R, &STRIP_L], scale) != 0;
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
        Control::Brake => brake(value, reference, pose, scale, m),
        Control::Exhaust => exhaust(value, reference, pose, scale, m),
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
    let tip = side.tip();
    let q = *f4::witness_positions(&old, &new, tip).first()?;
    let pivot = if side.right { PIVOT_R } else { PIVOT_L };
    let a = f64::from(tip[1] - pivot[1]).atan2(f64::from(tip[0] - pivot[0]));
    let b = f64::from(q[1] - pivot[1]).atan2(f64::from(q[0] - pivot[0]));
    // Wrap the difference, then count a clockwise turn (the right wing's sweep)
    // and a counter-clockwise one (the left's) as positive.
    let d = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    Some(if side.right { -d } else { d })
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    // The 25-pose pitch and roll grid at the two ends of the sweep.
    for (knots, name) in [(200., "25deg"), (650., "67deg")] {
        let spread = at_knots(neutral, knots);
        let original = airframe.animation_faces(&spread);
        let mut rows = String::from(
            "pitch,roll,finite,anchor_gap_ft,skin_gap_ft,rigidity_ft,fixed_faces_moved,crossings,direction_failures\n",
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
                    // The law's weights are 0.20 for pitch and 0.08 for roll.
                    let net = (5. * pitch + 2. * f64::from(side.sign()) * roll) as f32;
                    tail(side, net, &original, &pose, scale, &mut m);
                }
                let fixed = fixed_moved(&original, &pose, &[&TAIL_R, &TAIL_L], scale);
                m.reviewed_neutral_mismatch |= fixed != 0;
                writeln!(
                    rows,
                    "{pitch},{roll},{},{},{},{},{fixed},{},{}",
                    m.finite,
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
                if !passed(&m) {
                    failures.push(format!(
                        "Tornado {name} pitch{pitch}/roll{roll}: whole-panel spindle, rigidity, fold or direction witness"
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

    // The sweep against the speed schedule and the flaps: the wing panel
    // (without its strips, which droop on their own) stays rigid, the turn
    // follows the documented law, and the flaps hold the wing out.
    let original = airframe.animation_faces(&at_knots(neutral, 200.));
    let mut rows = String::from(
        "knots,flaps,expected_turn_deg,measured_turn_deg_right,measured_turn_deg_left,rigidity_ft,skin_gap_ft,seam_separation_ft,seam_opening_ft,seam_overlap_ft,passed\n",
    );
    let mut poses = Vec::new();
    for knots in [200., 250., 304., 400., 500., 600., 700.] {
        for flaps in [0., 0.25, 0.5, 0.75, 1.] {
            let mut state = at_knots(neutral, knots);
            state.flaps = flaps;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            let (from, to) = (
                tore_sim::combat::swing::SWEEP_FROM_KNOTS,
                tore_sim::combat::swing::SWEEP_TO_KNOTS,
            );
            let expected = ((knots - from) / (to - from)).clamp(0., 1.) * 42. * (1. - flaps);
            let mut measured = [0f64; 2];
            let mut seam = GloveSeam::default();
            let mut rigid = 0f32;
            for (k, side) in SIDES.iter().enumerate() {
                // Without the strips: they droop independently with the flaps.
                let panel: Vec<usize> = side
                    .wing
                    .iter()
                    .copied()
                    .filter(|a| !side.strip.contains(a))
                    .collect();
                rigid = rigid.max(rigidity(&original, &pose, &panel, scale));
                group_skin_gap(&original, &pose, &panel, scale, &mut m);
                measured[k] = tip_turn(&original, &pose, side)
                    .unwrap_or(f64::NAN)
                    .to_degrees();
                let s = glove_seam(&original, &pose, side, scale);
                seam.separation = seam.separation.max(s.separation);
                seam.opening = seam.opening.max(s.opening);
                seam.overlap = seam.overlap.max(s.overlap);
            }
            let ok = m.finite
                && rigid <= EPSILON
                && m.max_reviewed_skin_gap <= EPSILON
                && m.new_planar_crossings.is_empty()
                && measured.iter().all(|t| (t - expected).abs() < 0.01);
            writeln!(
                rows,
                "{knots},{flaps},{expected:.3},{:.3},{:.3},{rigid},{},{},{},{},{ok}",
                measured[0],
                measured[1],
                m.max_reviewed_skin_gap,
                seam.separation,
                seam.opening,
                seam.overlap
            )?;
            if !ok {
                failures.push(format!(
                    "Tornado sweep {knots} kt/flaps {flaps}: expected {expected:.2} deg, measured {:.2} / {:.2}, rigidity {rigid}",
                    measured[0], measured[1]
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("sweep-flap-combinations.csv"), rows)?;
    contact_sheet(&out.join("sweep-flap-combinations.ppm"), &original, &poses)?;

    // Glove seam at each sweep angle: how far the shared wing vertices pull away
    // from the fixed fillets (separation), and the outboard (visible opening)
    // and inboard (overlap into the glove) parts, per side, in feet.
    let mut rows = String::from(
        "sweep_deg,turn_deg,side,seam_separation_ft,seam_opening_ft,seam_overlap_ft,bound_ft\n",
    );
    for degrees in [25., 30., 35., 40., 45., 50., 55., 60., 65., 67.] {
        // Pick the speed that gives this sweep angle in the documented schedule.
        let knots = tore_sim::combat::swing::speed_for_sweep(degrees) / 1.68781;
        let pose = airframe.animation_faces(&at_knots(neutral, knots));
        let turn = degrees - 25.;
        for side in &SIDES {
            let seam = glove_seam(&original, &pose, side, scale);
            // The enclosing circle's radius is 2.55 units about the pivot.
            let bound = 2. * 2.5495 * (turn.to_radians() / 2.).sin() as f32 * FEET;
            writeln!(
                rows,
                "{degrees},{turn},{},{},{},{},{bound}",
                if side.right { "right" } else { "left" },
                seam.separation,
                seam.opening,
                seam.overlap
            )?;
            if seam.separation > bound + 1e-3 {
                failures.push(format!(
                    "Tornado glove seam at {degrees} deg exceeds its enclosing-circle bound"
                ));
            }
        }
    }
    fs::write(out.join("sweep-seam.csv"), rows)?;

    // Taileron forward corner at the limit, and the seam openings of the strip
    // and rudder corners at full travel.
    let mut rows = String::from("surface,command,corner,swing_ft\n");
    let corner = |ids: &[usize], pose: &[Face], point: [f32; 3]| -> f32 {
        let old = group(&original, ids);
        f4::witness_positions(&old, &keyed(pose), point)
            .first()
            .map_or(f32::NAN, |q| distance(point, *q) * scale)
    };
    for (name, elevator, aileron) in [
        ("pitch-up", 1., 0.),
        ("pitch-down", -1., 0.),
        ("roll-right", 0., 1.),
        ("pitch-up+roll-right", 1., 1.),
        ("pitch-down+roll-right", -1., 1.),
    ] {
        let mut state = at_knots(neutral, 200.);
        state.elevator = elevator;
        state.aileron = aileron;
        let pose = airframe.animation_faces(&state);
        for side in &SIDES {
            for (label, point) in [
                ("forward corner (7,-8,-2)", [7., -8., -2.]),
                ("tip leading (18,-23,-2)", [18., -23., -2.]),
                ("tip trailing (18,-27,-2)", [18., -27., -2.]),
            ] {
                writeln!(
                    rows,
                    "taileron-{},{name},{label},{}",
                    if side.right { "right" } else { "left" },
                    corner(side.tail, &pose, side.fix(point))
                )?;
            }
        }
    }
    fs::write(out.join("taileron-corner-swing.csv"), rows)?;

    let mut rows = String::from("surface,command,seam_opening_ft\n");
    for (name, edit) in [
        ("flap-strip-right", 0),
        ("flap-strip-left", 1),
        ("rudder-right", 2),
        ("rudder-left", 3),
    ] {
        let mut state = at_knots(neutral, 200.);
        match edit {
            0 | 1 => state.flaps = 1.,
            2 => state.rudder = 1.,
            _ => state.rudder = -1.,
        }
        let pose = airframe.animation_faces(&state);
        let ids: &[usize] = match edit {
            0 => &STRIP_R,
            1 => &STRIP_L,
            _ => &RUDDER,
        };
        writeln!(
            rows,
            "{name},full,{}",
            seam_opening(&original, &pose, ids, scale)
        )?;
    }
    fs::write(out.join("seam-opening.csv"), rows)?;

    failures.extend(pylon_poses(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_brake(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_exhaust(airframe, neutral, out, scale)?);
    Ok(failures)
}

/// Largest distance (feet) any group vertex that was shared with a fixed face
/// in the reference moved: the opening of the moving/fixed seam. Reported, not
/// a failure (a hinge vertex a little off its axis must move).
fn seam_opening(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    let before = keyed(reference);
    let after = keyed(pose);
    let fixed: std::collections::BTreeSet<[i64; 3]> = before
        .iter()
        .filter(|(k, _)| !ids.contains(&k.0))
        .flat_map(|(_, f)| f.positions.iter().map(|p| quantized(*p)))
        .collect();
    let mut worst = 0f32;
    for (key, face) in before.iter().filter(|(k, _)| ids.contains(&k.0)) {
        let Some(moved) = after.get(key) else {
            continue;
        };
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            if fixed.contains(&quantized(*a)) {
                worst = worst.max(distance(*a, *b) * scale);
            }
        }
    }
    worst
}

/// The wing-station interface for TOR-5 (`tornado_animation::wing_station`):
/// a mount given on a wing vertex lands exactly on that vertex of the swept
/// panel, the pylon swivels back by the panel's turn and the store keeps its
/// heading. Writes `pylon-poses.csv`.
fn pylon_poses(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    use crate::tornado_animation::{sweep_turn, wing_station};
    let mut failures = Vec::new();
    let mut rows = String::from(
        "knots,side,sweep_deg,turn_deg,swivel_deg,heading_deg,mount_right_ft,mount_up_ft,mount_fwd_ft,vertex_gap_ft\n",
    );
    for knots in [200., 300., 400., 500., 600., 700.] {
        let state = at_knots(neutral, knots);
        let pose = airframe.animation_faces(&state);
        for side in &SIDES {
            let tip = side.tip();
            let old = group(original, side.wing);
            let new = group(&pose, side.wing);
            let Some(q) = f4::witness_positions(&old, &new, tip).first().copied() else {
                failures.push("Tornado pylon witness: tip vertex missing".into());
                continue;
            };
            // The mount named on the tip vertex, in feet right, up and forward.
            let mount = [tip[0], tip[2], tip[1]].map(|v| f64::from(v * FEET));
            let station = wing_station(side.right, mount, sweep_turn(&state));
            let gap = (station.mount[0] - f64::from(q[0] * scale))
                .hypot(station.mount[2] - f64::from(q[1] * scale));
            writeln!(
                rows,
                "{knots},{},{:.3},{:.3},{:.3},{:.3},{:.4},{:.4},{:.4},{gap}",
                if side.right { "right" } else { "left" },
                crate::tornado_animation::sweep_degrees(station.turn),
                station.turn.to_degrees(),
                station.swivel.to_degrees(),
                station.heading.to_degrees(),
                station.mount[0],
                station.mount[1],
                station.mount[2],
            )?;
            if gap > f64::from(EPSILON)
                || station.heading != 0.
                || (station.turn + station.swivel * if side.right { 1. } else { -1. }).abs() > 1e-12
            {
                failures.push(format!(
                    "Tornado wing station off the swept wing ({} side, {knots} kt): gap {gap} ft",
                    if side.right { "right" } else { "left" }
                ));
            }
        }
    }
    fs::write(out.join("pylon-poses.csv"), rows)?;
    Ok(failures)
}

/// The most a stowed vertex may sit outside the neutral skin, feet: the left
/// upper skin is 0.2 units lower than the right (the source is not a mirror), so
/// the left main foot corner overshoots it by 0.17 ft at full stow.
const STOW_TOLERANCE_FT: f32 = 0.2;

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, whole, grounded at 1, folding forward, and near stow every piece
/// inside the airframe's skin within the stow tolerance.
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
    let mut csv = String::from(
        "value,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,worst_overshoot_outside_skin_ft,lowest_ft,checks_passed\n",
    );
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
        // Near stow (and at stow) every piece lies inside the airframe.
        let outside = if value < 1e-5 {
            hull_overshoot(original, &pose, &GEAR) * scale
        } else {
            0.
        };
        let low = lowest(&pose, &GEAR) * scale;
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && !m.reviewed_neutral_mismatch
            && outside <= STOW_TOLERANCE_FT
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{},{},{outside},{low},{passed}",
            m.max_reviewed_anchor_gap,
            m.max_reviewed_skin_gap,
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "Tornado dense gear value{value}: rigid, whole, ground, direction, hull or stow gate"
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

/// Brake sweep: every pose rigid with the hinge in place, the open pose is the
/// source plate, and the closed pose (a near-zero sample) lays the free edge flat
/// at the hinge height; the plate's hover over the skin there is reported.
fn dense_brake(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut open = neutral.clone();
    Control::Brake.apply(&mut open, 1.);
    let reference = airframe.animation_faces(&open);
    let mut csv = String::from(
        "value,rigidity_ft,hinge_gap_ft,free_edge_height_units,free_edge_hover_over_skin_ft,checks_passed\n",
    );
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let mut m = measure(&reference, &pose, scale);
        check(
            Control::Brake,
            value,
            (original, &reference, &pose),
            scale,
            &mut m,
        );
        let rigid = BRAKE
            .iter()
            .map(|&a| rigidity(&reference, &pose, &[a], scale))
            .fold(0f32, f32::max);
        // The free edge height (mean of its four vertices) and its hover over the skin.
        let (mut height, mut hover, mut n) = (0f32, 0f32, 0f32);
        for w in [
            ([2., -10., 4.], BRAKE_R),
            ([5., -10., 4.], BRAKE_R),
            ([-5., -10., 4.], BRAKE_L),
            ([-3., -10., 4.], BRAKE_L),
        ] {
            for q in follow(&reference, &pose, &w.1, w.0) {
                height += q[2];
                n += 1.;
                if let Some((_, top)) = skin_span(original, q[0] - q[0].signum() * 0.02, q[1]) {
                    hover = hover.max((q[2] - top) * scale);
                }
            }
        }
        let height = if n > 0. { height / n } else { f32::NAN };
        let hinge_gap = m.max_reviewed_anchor_gap;
        let closed = value < 1e-5;
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && rigid <= EPSILON
            && hinge_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_neutral_mismatch
            && (!closed || (height - 2.).abs() * scale <= 0.01);
        writeln!(csv, "{value},{rigid},{hinge_gap},{height},{hover},{passed}")?;
        if !passed {
            failures.push(format!(
                "Tornado brake value{value}: rigid, hinge or flat-closed gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}

/// Exhaust sweep: both plumes grow from the exit plane to their full length at
/// 1, the root edge at `value` units behind the plane, the tips at `19 * value`.
fn dense_exhaust(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut full = neutral.clone();
    Control::Exhaust.apply(&mut full, 1.);
    let reference = airframe.animation_faces(&full);
    let original = airframe.animation_faces(neutral);
    let mut csv = String::from("value,anchor_gap_ft,skin_gap_ft,checks_passed\n");
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.exhaust = value;
        let pose = airframe.animation_faces(&state);
        let mut m = measure(&reference, &pose, scale);
        check(
            Control::Exhaust,
            value,
            (&original, &reference, &pose),
            scale,
            &mut m,
        );
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_neutral_mismatch;
        writeln!(
            csv,
            "{value},{},{},{passed}",
            m.max_reviewed_anchor_gap, m.max_reviewed_skin_gap
        )?;
        if !passed {
            failures.push(format!("Tornado exhaust value{value}: plume gate"));
        }
    }
    fs::write(out.join("exhaust-sweep.csv"), csv)?;
    Ok(failures)
}
