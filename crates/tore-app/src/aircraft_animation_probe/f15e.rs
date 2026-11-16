//! Independent F-15E witnesses for the F15E-4 devices: the eighteen rigid gear
//! plates (each main leg, wheel sheet and cross plate, turns as one body that
//! lays flat and folds forward; each inboard door turns about its top edge and
//! closes outward; the six nose plates fold forward as one body), the dorsal
//! speed brake (four faces, one piece, about its forward hinge) and the two
//! afterburner plumes. Measures the shared Airframe output and never repeats
//! the rig's algorithm. Witness points are named in shape units and scaled with
//! the aircraft's own per-axis scale (the probe geometry is the scaled shape).
//!
//! F15E-3 adds the control surfaces: the two all-moving stabilators (five faces
//! each, one rigid panel about a lateral spindle through the root chord
//! midpoint; pitch plus differential roll), the two ailerons (two faces each,
//! one rigid panel about the slanted hinge from the root pair's mid point to the
//! tip vertex) and the two inboard flaps (two faces each plus the lowered
//! branch's end triangle). The fins have no rudder motion (John, 2026-11-10):
//! the Rudder control is not-applicable and `check` requires every surface to
//! stay exactly at its neutral faces under a rudder command.
use super::*;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::F15E;
const DOOR_L: [usize; 2] = [0x50e3, 0x50fa];
const DOOR_R: [usize; 2] = [0x524f, 0x5266];
const LEG_L: [usize; 4] = [0x5156, 0x5175, 0x5194, 0x51b3];
const LEG_R: [usize; 4] = [0x52c2, 0x52e1, 0x5300, 0x531f];
const NOSE: [usize; 6] = [0x539b, 0x53ba, 0x53d9, 0x53f0, 0x5407, 0x5426];
const GEAR: [usize; 18] = [
    0x50e3, 0x50fa, 0x5156, 0x5175, 0x5194, 0x51b3, 0x524f, 0x5266, 0x52c2, 0x52e1, 0x5300, 0x531f,
    0x539b, 0x53ba, 0x53d9, 0x53f0, 0x5407, 0x5426,
];
const BRAKE: [usize; 4] = [0x568c, 0x56a3, 0x56ba, 0x56d1];
const FLAME: [usize; 8] = [
    0x574f, 0x5776, 0x579d, 0x57c4, 0x57eb, 0x5812, 0x5839, 0x5860,
];

const STAB_R: [usize; 5] = [0x4b5f, 0x4b86, 0x4ba7, 0x4c5d, 0x4c8a];
const STAB_L: [usize; 5] = [0x42f1, 0x4318, 0x4339, 0x43ef, 0x441c];
const AIL_R: [usize; 2] = [0x4b33, 0x4ac4];
const AIL_L: [usize; 2] = [0x42c5, 0x41d9];
const FLAP_R: [usize; 2] = [0x5470, 0x548f];
const FLAP_L: [usize; 2] = [0x5557, 0x5576];
const FLAP_END_R: usize = 0x5517;
const FLAP_END_L: usize = 0x55fe;

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

/// Largest change of any vertex-to-vertex distance inside one group.
pub(super) fn rigidity(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
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

pub(super) fn pinned(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    point: [f32; 3],
    scale: f32,
) -> f32 {
    let old: BTreeMap<_, _> = keyed(reference)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect();
    let new = keyed(pose);
    let moved = f4::witness_positions(&old, &new, point);
    if moved.is_empty() {
        f32::INFINITY
    } else {
        moved
            .iter()
            .map(|q| distance(point, *q) * scale)
            .fold(0f32, f32::max)
    }
}

pub(super) fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

pub(super) fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}

/// Skin gap inside one group: shared vertices of the two windings of a plate
/// must move together.
pub(super) fn group_skin_gap(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    scale: f32,
    m: &mut Metrics,
) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
}

/// The extent of a group of faces: (min, max) on every axis.
pub(super) fn extent(pose: &[Face], ids: &[usize]) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in pose
        .iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
    {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

pub(super) fn pin(
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

/// Where `witness` (a vertex of the gear-down group `ids`) is in `pose`.
pub(super) fn follow(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    witness: [f32; 3],
) -> Vec<[f32; 3]> {
    f4::witness_positions(&group(reference, ids), &keyed(pose), witness)
}

/// The gear stays whole rigid pieces: each main leg (wheel sheet and cross
/// plate), each inboard door and the six nose plates. Doors keep their top
/// edge; legs and the nose have no vertex on their fold axes (the axes lie
/// behind and above every vertex). Gear down is the ground contact; closing
/// never lowers a plate; the legs and the nose fold forward, the doors close
/// outward, and near stow every piece is inside its bay.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    let zs = sp([0., 0., 1.])[2];
    for ids in [&LEG_R[..], &LEG_L[..], &DOOR_R[..], &DOOR_L[..], &NOSE[..]] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
        group_skin_gap(reference, pose, ids, scale, m);
    }
    // The door top edges are the hinges and stay.
    pin(
        reference,
        pose,
        &DOOR_R,
        &[sp([7., 9., -12.]), sp([7., -6., -12.])],
        scale,
        m,
    );
    pin(
        reference,
        pose,
        &DOOR_L,
        &[sp([-7., 9., -12.]), sp([-7., -6., -12.])],
        scale,
        m,
    );
    // Closing never lowers the gear: nothing hangs below the gear-down pose.
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // Ground contact: the lowest point of the gear-down pose is the
        // scaled 27 unit low point the flight model stands the aircraft on
        // (27 units at a third of a foot, times the height factor).
        let want = -27. / 3. * zs;
        m.reviewed_wheel_failed |= (pose_low * scale - want).abs() > 1e-3;
    } else {
        for side in [1f32, -1.] {
            let (leg, door) = if side > 0. {
                (&LEG_R[..], &DOOR_R[..])
            } else {
                (&LEG_L[..], &DOOR_L[..])
            };
            // A main wheel's lowest rear corner moves forward.
            let foot = sp([side * 13., -4., -27.]);
            let qs = follow(reference, pose, leg, foot);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures += usize::from(q[1] <= foot[1]);
            }
            // A door's lowest corner moves outward and never down.
            let toe = sp([side * 7., 1., -18.]);
            let qs = follow(reference, pose, door, toe);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures +=
                    usize::from((q[0] - toe[0]) * side <= 0. || q[2] < toe[2] - EPSILON);
            }
        }
        // The nose wheel's foot moves forward.
        let foot = sp([0., 62., -27.]);
        let qs = follow(reference, pose, &NOSE, foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[1] <= foot[1]);
        }
    }
    if value < 1e-5 {
        // Near stow every piece is inside the bay it folds into: the mains
        // between the keel (|x| 6) and the shelf wall (|x| 21), above z -9 and
        // below the wing skin, ahead of the leg root; the doors flat at the
        // belly line; the nose between the belly and the nose top.
        let tolerance = 1e-3;
        for (side, leg, door) in [
            (1f32, &LEG_R[..], &DOOR_R[..]),
            (-1., &LEG_L[..], &DOOR_L[..]),
        ] {
            let (lo, hi) = extent(pose, leg);
            let (a, b) = (sp([side * 6., -4., -9.]), sp([side * 21., 25., 0.]));
            for k in 0..3 {
                let (min, max) = (a[k].min(b[k]), a[k].max(b[k]));
                m.reviewed_wheel_failed |= lo[k] < min - tolerance || hi[k] > max + tolerance;
            }
            let (lo, hi) = extent(pose, door);
            let (a, b) = (sp([side * 6., -6., -12.]), sp([side * 15., 9., -12.]));
            for k in 0..3 {
                let (min, max) = (a[k].min(b[k]), a[k].max(b[k]));
                m.reviewed_wheel_failed |= lo[k] < min - tolerance || hi[k] > max + tolerance;
            }
        }
        let (lo, hi) = extent(pose, &NOSE);
        let (a, b) = (sp([-3., 44., -9.]), sp([3., 66., 9.]));
        for k in 0..3 {
            m.reviewed_wheel_failed |=
                lo[k] < a[k].min(b[k]) - 1e-3 || hi[k] > a[k].max(b[k]) + 1e-3;
        }
    }
}

/// The dorsal brake: open (1) is the source pose, closing turns the whole
/// four-face piece about its forward hinge (the line y 25, z 11), never raising
/// the free (aft) edge.
fn brake(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| BRAKE.contains(&f.address)));
        return;
    }
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
    group_skin_gap(reference, pose, &BRAKE, scale, m);
    let error = rigidity(reference, pose, &BRAKE, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The forward edge (y 25) is the hinge and never moves.
    pin(
        reference,
        pose,
        &BRAKE,
        &[sp([-3., 25., 11.]), sp([3., 25., 11.])],
        scale,
        m,
    );
    let old = group(reference, &BRAKE);
    let new = group(pose, &BRAKE);
    for witness in [sp([-3., -1., 30.]), sp([3., -1., 30.])] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[2] > witness[2] + EPSILON);
        }
    }
}

/// Each plume grows from the nozzle exit plane (y -61): a root vertex stays
/// within one unit of the plane (it starts one unit behind it), the tips pull
/// forward as the demand falls, and shared edges stay together.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
        return;
    }
    group_skin_gap(reference, pose, &FLAME, scale, m);
    let old = group(reference, &FLAME);
    let new = group(pose, &FLAME);
    let plane = sp([0., -61., 0.])[1];
    let unit = (sp([0., -62., 0.])[1] - plane).abs();
    for witness in [
        sp([8., -62., -6.]),
        sp([8., -62., 5.]),
        sp([-8., -62., -6.]),
        sp([-8., -62., 5.]),
        sp([13., -62., -1.]),
        sp([4., -62., -1.]),
        sp([-13., -62., -1.]),
        sp([-4., -62., -1.]),
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_anchor_missing |= qs.is_empty();
        for q in qs {
            // Never farther behind the plane than the source root, never ahead of it.
            let off = (q[1] - plane).abs();
            if off > unit + EPSILON || q[1] > plane + EPSILON {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(off * scale);
            }
        }
    }
    for witness in [
        sp([8., -115., -6.]),
        sp([8., -115., 5.]),
        sp([-8., -115., -6.]),
        sp([-8., -115., 5.]),
        sp([13., -115., -1.]),
        sp([4., -115., -1.]),
        sp([-13., -115., -1.]),
        sp([-4., -115., -1.]),
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            if value < 1. {
                m.reviewed_direction_failures += usize::from(q[1] <= witness[1] + EPSILON);
            }
        }
    }
}

/// Faces of one group that differ from the reference mark a neutral mismatch.
pub(super) fn unchanged(reference: &[Face], pose: &[Face], ids: &[usize], m: &mut Metrics) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    if old.len() != new.len() {
        m.reviewed_neutral_mismatch = true;
    }
    for (key, face) in &old {
        match new.get(key) {
            Some(after) if face.positions == after.positions => {}
            _ => m.reviewed_neutral_mismatch = true,
        }
    }
}

/// Mid point of two reference corners followed into the pose, each corner
/// averaged over its occurrences first (corners occur in unequal numbers of
/// faces). Distance in feet from the reference mid point.
pub(super) fn midpoint_gap(
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    corners: [[f32; 3]; 2],
    scale: f32,
) -> Option<f32> {
    let mut means = Vec::new();
    for corner in corners {
        let moved = f4::witness_positions(old, new, corner);
        if moved.is_empty() {
            return None;
        }
        means.push(std::array::from_fn::<f32, 3, _>(|k| {
            moved.iter().map(|p| p[k]).sum::<f32>() / moved.len() as f32
        }));
    }
    let now: [f32; 3] = std::array::from_fn(|k| (means[0][k] + means[1][k]) / 2.);
    let was: [f32; 3] = std::array::from_fn(|k| (corners[0][k] + corners[1][k]) / 2.);
    Some(distance(now, was) * scale)
}

/// Whole-panel gates shared by a group: rigid, no skin gap between its faces.
pub(super) fn whole_panel(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    scale: f32,
    m: &mut Metrics,
) {
    group_skin_gap(reference, pose, ids, scale, m);
    let error = rigidity(reference, pose, ids, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
}

/// Direction of the net command on a witness's Z (feet times the sign).
pub(super) fn z_direction(
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    witness: [f32; 3],
    sign: f32,
    scale: f32,
    m: &mut Metrics,
) {
    let qs = f4::witness_positions(old, new, witness);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from((q[2] - witness[2]) * scale * sign <= EPSILON);
    }
}

/// The stabilators, ailerons and flaps for one combined command (`elevator`
/// and `aileron` in -1..=1, `flaps` in 0..=1). A surface with a zero net
/// command must be exactly the reference; one with a net command must be a
/// rigid whole panel with its anchor held, no skin gap and the trailing edge
/// moving the signed way. The rudder command is not a parameter: nothing may
/// react to it.
fn surfaces(
    reference: &[Face],
    pose: &[Face],
    scale: f32,
    m: &mut Metrics,
    elevator: f64,
    aileron: f64,
    flaps: f64,
) {
    // Stabilators: pull raises both trailing edges, positive aileron raises the
    // right and lowers the left. Pitch (3 parts) outweighs roll (1 part) in the
    // fitted law, so the sign is that of 3 * pitch + side * roll.
    for (ids, side, root, tip_in, tip_out) in [
        (
            &STAB_R[..],
            1f32,
            21f32,
            [41f32, -73., -1.],
            [47f32, -67., -1.],
        ),
        (&STAB_L[..], -1., -22., [-42., -73., -1.], [-47., -67., -1.]),
    ] {
        let net = 3. * elevator as f32 + side * aileron as f32;
        if net == 0. {
            unchanged(reference, pose, ids, m);
            continue;
        }
        whole_panel(reference, pose, ids, scale, m);
        let (old, new) = (group(reference, ids), group(pose, ids));
        // The root chord midpoint is the spindle: it never moves.
        let corners = [sp([root, -41., -1.]), sp([root, -70., -1.])];
        match midpoint_gap(&old, &new, corners, scale) {
            Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
            None => m.reviewed_anchor_missing = true,
        }
        // Both aft tip corners rise for a positive net command; the leading
        // root corner, ahead of the spindle, moves the other way.
        z_direction(&old, &new, sp(tip_in), net.signum(), scale, m);
        z_direction(&old, &new, sp(tip_out), net.signum(), scale, m);
        z_direction(&old, &new, sp([root, -41., -1.]), -net.signum(), scale, m);
    }
    for (ids, side) in [(&AIL_R[..], 1f32), (&AIL_L[..], -1.)] {
        let net = side * aileron as f32;
        if net == 0. {
            unchanged(reference, pose, ids, m);
            continue;
        }
        whole_panel(reference, pose, ids, scale, m);
        let (old, new) = (group(reference, ids), group(pose, ids));
        // The slanted hinge: the tip vertex and the root pair's mid point stay.
        let tip = sp([side * 59., -27., 3.]);
        let qs = f4::witness_positions(&old, &new, tip);
        m.reviewed_anchor_missing |= qs.is_empty();
        for q in qs {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(tip, q) * scale);
        }
        let corners = [sp([side * 39., -24., 1.]), sp([side * 39., -24., 4.])];
        match midpoint_gap(&old, &new, corners, scale) {
            Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
            None => m.reviewed_anchor_missing = true,
        }
        for witness in [sp([side * 59., -34., 3.]), sp([side * 39., -30., 3.])] {
            z_direction(&old, &new, witness, net.signum(), scale, m);
        }
    }
    for (ids, end, side, inner) in [
        (&FLAP_R[..], FLAP_END_R, 1f32, 21f32),
        (&FLAP_L[..], FLAP_END_L, -1., -22.),
    ] {
        let has_end = pose.iter().any(|f| f.address == end);
        if flaps == 0. {
            unchanged(reference, pose, ids, m);
            // The lowered end triangle exists only while the flap is down.
            m.reviewed_direction_failures += usize::from(has_end);
            continue;
        }
        whole_panel(reference, pose, ids, scale, m);
        let (old, new) = (group(reference, ids), group(pose, ids));
        // The front edge's mid height is the hinge line: both front pairs keep
        // their mid points.
        for x in [inner, side * 39.] {
            let corners = [sp([x, -24., 1.]), sp([x, -24., 4.])];
            match midpoint_gap(&old, &new, corners, scale) {
                Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
                None => m.reviewed_anchor_missing = true,
            }
        }
        for witness in [sp([inner, -30., 3.]), sp([side * 39., -30., 3.])] {
            z_direction(&old, &new, witness, -1., scale, m);
        }
        // The end triangle is drawn and sits on the wedge's own outboard
        // corners wherever they went.
        m.reviewed_direction_failures += usize::from(!has_end);
        if let Some(triangle) = pose.iter().find(|f| f.address == end) {
            // Each outboard wedge corner has a triangle vertex on it (the
            // triangle lists its corners in either order).
            m.reviewed_direction_failures += usize::from(triangle.positions.len() != 3);
            for corner in [
                sp([side * 39., -24., 1.]),
                sp([side * 39., -30., 3.]),
                sp([side * 39., -24., 4.]),
            ] {
                let qs = f4::witness_positions(&old, &new, corner);
                m.reviewed_anchor_missing |= qs.is_empty();
                for q in qs {
                    let gap = triangle
                        .positions
                        .iter()
                        .map(|&t| distance(q, t) * scale)
                        .fold(f32::INFINITY, f32::min);
                    m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(gap);
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
        Control::Gear => gear(value, reference, pose, scale, m),
        Control::Brake => brake(value, reference, pose, scale, m),
        Control::Exhaust => exhaust(value, reference, pose, scale, m),
        Control::Elevator => surfaces(reference, pose, scale, m, value, 0., 0.),
        Control::Aileron => surfaces(reference, pose, scale, m, 0., value, 0.),
        Control::Flaps => surfaces(reference, pose, scale, m, 0., 0., value),
        // No rudder motion (John, 2026-11-10): the fins and every other
        // surface stay at the neutral faces.
        Control::Rudder => surfaces(reference, pose, scale, m, 0., 0., 0.),
        _ => {}
    }
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_brake(airframe, neutral, out, scale)?);
    failures.extend(pitch_roll_grid(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_surfaces(airframe, neutral, out, &original, scale)?);
    seam_openings(airframe, neutral, out, &original, scale)?;
    Ok(failures)
}

/// Largest distance (feet) any vertex of the group that is shared with a face
/// outside the group in the reference moved (`only` keeps the vertices it
/// accepts): the opening of the moving/fixed seam. Reported and bounded, not a
/// failure (a hinged surface has to open).
pub(super) fn seam_opening(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    scale: f32,
    only: &dyn Fn([f32; 3]) -> bool,
) -> f32 {
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
            if only(*a) && fixed.contains(&quantized(*a)) {
                worst = worst.max(distance(*a, *b) * scale);
            }
        }
    }
    worst
}

pub(super) fn surfaces_pass(m: &Metrics) -> bool {
    m.finite
        && !m.reviewed_anchor_missing
        && m.max_reviewed_anchor_gap <= EPSILON
        && m.max_reviewed_skin_gap <= EPSILON
        && m.reviewed_rigid_panel_error <= EPSILON
        && m.reviewed_direction_failures == 0
        && !m.reviewed_neutral_mismatch
        && m.new_planar_crossings.is_empty()
}

/// The 25 pitch and roll poses: every stabilator and aileron a whole rigid
/// panel with its anchor held, nothing else moved, every trailing edge on the
/// sign of its net command.
fn pitch_roll_grid(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut rows = String::from(
        "pitch,roll,finite,anchor_gap_ft,skin_gap_ft,rigidity_ft,stab_seam_ft,aileron_seam_ft,crossings,direction_failures,passed\n",
    );
    let mut poses = Vec::new();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.aileron = roll;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(original, &pose, scale);
            surfaces(original, &pose, scale, &mut m, pitch, roll, 0.);
            // Flaps stay up in this grid: the wedges must be untouched.
            unchanged(original, &pose, &FLAP_R, &mut m);
            unchanged(original, &pose, &FLAP_L, &mut m);
            let all = |_: [f32; 3]| true;
            let stab = seam_opening(original, &pose, &STAB_R, scale, &all)
                .max(seam_opening(original, &pose, &STAB_L, scale, &all));
            let ail = seam_opening(original, &pose, &AIL_R, scale, &all)
                .max(seam_opening(original, &pose, &AIL_L, scale, &all));
            let passed = surfaces_pass(&m);
            writeln!(
                rows,
                "{pitch},{roll},{},{},{},{},{stab},{ail},{},{},{passed}",
                m.finite,
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "F-15E pitch{pitch}/roll{roll}: whole-panel anchor, skin, rigidity or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("pitch-roll-combinations.ppm"), original, &poses)?;
    Ok(failures)
}

/// Dense sweeps of the three surface controls (21 values each: elevator and
/// aileron -1 to 1 in 0.1 steps, flaps 0 to 1 in 0.05 steps): every pose a
/// whole rigid panel, anchors held, skin gap 0, the signed direction.
fn dense_surfaces(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    for (name, control, low, step) in [
        ("elevator", Control::Elevator, -1., 0.1),
        ("aileron", Control::Aileron, -1., 0.1),
        ("flaps", Control::Flaps, 0., 0.05),
    ] {
        let mut csv = String::from(
            "value,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,direction_failures,passed\n",
        );
        for i in 0..=20 {
            let value = low + f64::from(i) * step;
            let value = (value * 1000.).round() / 1000.;
            let mut state = neutral.clone();
            control.apply(&mut state, value);
            let pose = airframe.animation_faces(&state);
            let mut m = measure(original, &pose, scale);
            check(control, value, (original, original, &pose), scale, &mut m);
            let passed = surfaces_pass(&m);
            writeln!(
                csv,
                "{value},{},{},{},{},{},{passed}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "F-15E dense {name} value{value}: whole-panel, anchor or direction gate"
                ));
            }
        }
        fs::write(out.join(format!("dense-{name}.csv")), csv)?;
    }
    Ok(failures)
}

/// The seam opening of every moving surface at full travel (feet): where a
/// whole rigid panel leaves the fixed skin it hinges against.
fn seam_openings(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<()> {
    let mut rows = String::from("command,surface,hinge_edge_opening_ft,other_opening_ft\n");
    type Edit = fn(&mut State);
    let commands: [(&str, Edit); 6] = [
        ("pitch-up", |s| s.elevator = 1.),
        ("pitch-down", |s| s.elevator = -1.),
        ("roll-right", |s| s.aileron = 1.),
        ("roll-left", |s| s.aileron = -1.),
        ("flaps-down", |s| s.flaps = 1.),
        ("pitch-up-roll-right", |s| {
            s.elevator = 1.;
            s.aileron = 1.;
        }),
    ];
    // The hinge edge of a stabilator is its root (|x| at most 22.5); of an
    // aileron or a flap, its front edge (y at least -27.5).
    let root = sp([22.5, 0., 0.])[0];
    let front = sp([0., -27.5, 0.])[1];
    let at_root = move |p: [f32; 3]| p[0].abs() <= root;
    let at_front = move |p: [f32; 3]| p[1] >= front;
    type Hinge<'a> = &'a dyn Fn([f32; 3]) -> bool;
    let surfaces: [(&str, &[usize], Hinge); 6] = [
        ("stabilator-right", &STAB_R, &at_root),
        ("stabilator-left", &STAB_L, &at_root),
        ("aileron-right", &AIL_R, &at_front),
        ("aileron-left", &AIL_L, &at_front),
        ("flap-right", &FLAP_R, &at_front),
        ("flap-left", &FLAP_L, &at_front),
    ];
    for (command, edit) in commands {
        let mut state = neutral.clone();
        edit(&mut state);
        let pose = airframe.animation_faces(&state);
        for (name, ids, hinge) in surfaces {
            let edge = seam_opening(original, &pose, ids, scale, hinge);
            let other = seam_opening(original, &pose, ids, scale, &|p| !hinge(p));
            writeln!(rows, "{command},{name},{edge},{other}")?;
        }
    }
    fs::write(out.join("seam-opening.csv"), rows)?;
    Ok(())
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, hinged, grounded at 1, folded forward and inside its bay near 0.
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
    let mut csv =
        String::from("value,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,checks_passed\n");
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
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{},{},{passed}",
            m.max_reviewed_anchor_gap,
            m.max_reviewed_skin_gap,
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "F-15E dense gear value{value}: rigid, hinge, direction or stow gate"
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

/// Brake sweep: the closed pose (a near-zero sample) lays the free edge's mid
/// point on the dorsal ridge within a hundredth of a foot, and every pose is
/// rigid with the hinge in place.
fn dense_brake(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut open = neutral.clone();
    Control::Brake.apply(&mut open, 1.);
    let reference = airframe.animation_faces(&open);
    let ridge = {
        let (a, b) = (sp([0., 20., 10.]), sp([0., -10., 6.]));
        move |y: f32| a[2] + (y - a[1]) / (b[1] - a[1]) * (b[2] - a[2])
    };
    let mut csv = String::from("value,rigidity_ft,hinge_gap_ft,free_edge_off_spine_ft,passed\n");
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let rigid = rigidity(&reference, &pose, &BRAKE, scale);
        let hinge_gap = pinned(&reference, &pose, &BRAKE, sp([3., 25., 11.]), scale);
        let (mut y, mut z, mut n) = (0f32, 0f32, 0f32);
        for q in follow(&reference, &pose, &BRAKE, sp([3., -1., 30.]))
            .into_iter()
            .chain(follow(&reference, &pose, &BRAKE, sp([-3., -1., 30.])))
        {
            y += q[1];
            z += q[2];
            n += 1.;
        }
        let off = if n > 0. {
            ((z / n) - ridge(y / n)).abs() * scale
        } else {
            f32::INFINITY
        };
        let closed = value < 1e-5;
        let passed = rigid <= EPSILON && hinge_gap <= EPSILON && (!closed || off <= 0.01);
        writeln!(csv, "{value},{rigid},{hinge_gap},{off},{passed}")?;
        if !passed {
            failures.push(format!(
                "F-15E brake value{value}: rigid, hinge or spine gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}
