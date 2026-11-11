//! Independent Saab Gripen witnesses (GRI-4): the two main legs (strut and
//! wheel plate, turned together), the two main door pairs, the nose group, the
//! two rear-fuselage airbrake plates and the afterburner plume. Measures the
//! shared Airframe output and never repeats the rig's algorithm. The control
//! surfaces (canards, elevons, rudder) are GRI-3's and get no witness here;
//! `combinations` is where GRI-3 appends the pitch and roll grid.
use super::*;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::Gripen;
const LEG_R: [usize; 4] = [0x5bca, 0x5be9, 0x5c08, 0x5c2f];
const LEG_L: [usize; 4] = [0x59ec, 0x5a0b, 0x5a2a, 0x5a51];
const DOOR_R: [usize; 4] = [0x5b29, 0x5b40, 0x5b57, 0x5b6e];
const DOOR_L: [usize; 4] = [0x594b, 0x5962, 0x5979, 0x5990];
const NOSE: [usize; 6] = [0x5c9b, 0x5cba, 0x5cd9, 0x5cf8, 0x5d44, 0x5d5b];
const GEAR: [usize; 22] = [
    0x5bca, 0x5be9, 0x5c08, 0x5c2f, 0x59ec, 0x5a0b, 0x5a2a, 0x5a51, 0x5b29, 0x5b40, 0x5b57, 0x5b6e,
    0x594b, 0x5962, 0x5979, 0x5990, 0x5c9b, 0x5cba, 0x5cd9, 0x5cf8, 0x5d44, 0x5d5b,
];
const DOOR_WALL_R: [usize; 2] = [0x5b29, 0x5b40];
const DOOR_WALL_L: [usize; 2] = [0x594b, 0x5962];
const DOOR_ROOT_R: [usize; 2] = [0x5b57, 0x5b6e];
const DOOR_ROOT_L: [usize; 2] = [0x5979, 0x5990];
const BRAKE_R: [usize; 2] = [0x62ad, 0x62c4];
const BRAKE_L: [usize; 2] = [0x634a, 0x6361];
const BRAKE: [usize; 4] = [0x62ad, 0x62c4, 0x634a, 0x6361];
const FLAME: [usize; 4] = [0x63af, 0x63d6, 0x63fd, 0x6424];

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

/// Largest change of any vertex-to-vertex distance inside one group.
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

fn pinned(reference: &[Face], pose: &[Face], ids: &[usize], point: [f32; 3], scale: f32) -> f32 {
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

fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}

/// Skin gap inside one group: shared vertices of the windings of a piece must
/// move together.
fn group_skin_gap(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
}

/// Move of a reference vertex into the pose, as the displacement of every
/// pose vertex that started at `witness` (empty when the face is absent).
fn moves(reference: &[Face], pose: &[Face], ids: &[usize], witness: [f32; 3]) -> Vec<[f32; 3]> {
    f4::witness_positions(&group(reference, ids), &keyed(pose), witness)
}

/// Gear: every piece stays rigid, the foot of each leg folds inboard and
/// forward, the door pairs fold inboard, the nose folds aft, nothing ever
/// hangs below the gear-down pose, gear down stands the airframe on the
/// shape's 16 unit low point, and the stowed pose lies inside the hull.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    for ids in [
        &LEG_R[..],
        &LEG_L[..],
        &DOOR_WALL_R[..],
        &DOOR_WALL_L[..],
        &DOOR_ROOT_R[..],
        &DOOR_ROOT_L[..],
        &NOSE[..],
    ] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
        group_skin_gap(reference, pose, ids, scale, m);
    }
    // The door wall and root plate of one side share the edge x 13, z -4
    // (y -14 to 1): a gap between them is a seam opening.
    for (wall, root) in [(&DOOR_WALL_R, &DOOR_ROOT_R), (&DOOR_WALL_L, &DOOR_ROOT_L)] {
        let side = if wall[0] == DOOR_WALL_R[0] { 1f32 } else { -1. };
        for y in [1f32, -14.] {
            let p = sp([side * 13., y, -4.]);
            let a = moves(reference, pose, wall, p);
            let b = moves(reference, pose, root, p);
            m.reviewed_anchor_missing |= a.is_empty() || b.is_empty();
            for (u, v) in a.iter().zip(&b) {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(*u, *v) * scale);
            }
        }
    }
    // Closing never lowers a plate.
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // Ground contact: the lowest gear vertex is the shape's 16 unit low
        // point, a third of a foot a unit.
        let want = -16. / 3. * sp([0., 0., 1.])[2];
        m.reviewed_wheel_failed |= (pose_low * scale - want).abs() > 1e-3;
        return;
    }
    // Direction: each leg's foot corners move inboard and forward; each door
    // root plate's outer edge rises (it swings up and inboard, so it only
    // comes inboard after it passes the level); the nose foot moves aft.
    for (ids, witness, inboard) in [
        (&LEG_R[..], sp([7., -11., -16.]), -1f32),
        (&LEG_R[..], sp([2., -11., -16.]), -1.),
        (&LEG_L[..], sp([-6., -11., -16.]), 1.),
        (&LEG_L[..], sp([-1., -11., -16.]), 1.),
    ] {
        let qs = moves(reference, pose, ids, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures +=
                usize::from((q[0] - witness[0]) * inboard <= 0. || q[1] <= witness[1]);
        }
    }
    for (ids, witness) in [
        (&DOOR_ROOT_R[..], sp([13., -14., -4.])),
        (&DOOR_ROOT_L[..], sp([-13., -14., -4.])),
    ] {
        let qs = moves(reference, pose, ids, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[2] <= witness[2]);
        }
    }
    for foot in [sp([0., 38., -15.]), sp([0., 44., -15.])] {
        let qs = moves(reference, pose, &NOSE, foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[1] >= foot[1]);
        }
    }
    if value < 1e-5 {
        // Near stow every piece has come up inside the hull: the legs and
        // doors within the half span of the fuselage side (x 9) and above the
        // belly, the nose above its belly line and inside the nose section.
        let belly = sp([0., 0., -6.])[2];
        let nose_belly = sp([0., 0., -5.])[2];
        let side = sp([9., 0., 0.])[0];
        for (ids, floor) in [
            (&LEG_R[..], belly),
            (&LEG_L[..], belly),
            (&DOOR_R[..], nose_belly),
            (&DOOR_L[..], nose_belly),
            (&NOSE[..], nose_belly),
        ] {
            m.reviewed_wheel_failed |= lowest(pose, ids) < floor - 1e-3;
        }
        let reach = pose
            .iter()
            .filter(|f| {
                LEG_R.contains(&f.address)
                    || LEG_L.contains(&f.address)
                    || DOOR_R.contains(&f.address)
                    || DOOR_L.contains(&f.address)
            })
            .flat_map(|f| &f.positions)
            .map(|p| p[0].abs())
            .fold(0f32, f32::max);
        // The door wall is longer than the root plate and overshoots its hinge
        // by 1.3 units when folded flat on it.
        m.reviewed_wheel_failed |= reach > side + sp([1.5, 0., 0.])[0] + 1e-3;
    }
}

/// The airbrake plates: open (1) is the source pose; closing turns each plate
/// whole about its forward vertical edge, which never moves, and the free edge
/// only comes inboard.
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
    for (ids, side) in [(&BRAKE_R[..], 1f32), (&BRAKE_L[..], -1.)] {
        group_skin_gap(reference, pose, ids, scale, m);
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
        for point in [sp([side * 9., -38., 1.]), sp([side * 9., -38., -4.])] {
            let gap = pinned(reference, pose, ids, point, scale);
            m.reviewed_anchor_missing |= !gap.is_finite();
            if gap.is_finite() {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
            }
        }
        for witness in [sp([side * 13., -43., 0.]), sp([side * 13., -43., -3.])] {
            let qs = moves(reference, pose, ids, witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures += usize::from((q[0] - witness[0]) * side > EPSILON);
            }
        }
    }
}

/// The plume grows from the nozzle exit plane (y -61): the exit corners stay,
/// the tips pull forward as the demand falls, and shared edges stay together.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
        return;
    }
    group_skin_gap(reference, pose, &FLAME, scale, m);
    let old = group(reference, &FLAME);
    let new = group(pose, &FLAME);
    for witness in [
        sp([0., -61., 1.]),
        sp([0., -61., -4.]),
        sp([3., -61., -2.]),
        sp([-2., -61., -2.]),
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_anchor_missing |= qs.is_empty();
        for q in qs {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(witness, q) * scale);
        }
    }
    for witness in [
        sp([0., -85., 1.]),
        sp([0., -85., -4.]),
        sp([3., -85., -2.]),
        sp([-2., -85., -2.]),
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
        // GRI-3: canard, elevon and rudder witnesses.
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
    // GRI-3 appends the pitch and roll grid here.
    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_brake(airframe, neutral, out, scale)?);
    Ok(failures)
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, grounded at 1 and tucked inside the hull near 0.
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
                "Gripen dense gear value{value}: rigid, direction, stow or crossing gate"
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

/// Airbrake sweep: the closed pose (a near-zero sample) lays the free edge
/// along the rear fuselage side line, within a hundredth of a foot of the
/// half unit offset the hinge has, and every pose is rigid and hinged.
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
    let mut csv = String::from("value,rigidity_ft,hinge_gap_ft,free_edge_off_side_ft,passed\n");
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let mut rigid = 0f32;
        let mut hinge_gap = 0f32;
        let mut off = 0f32;
        for (ids, side) in [(&BRAKE_R[..], 1f32), (&BRAKE_L[..], -1.)] {
            rigid = rigid.max(rigidity(&reference, &pose, ids, scale));
            for point in [sp([side * 9., -38., 1.]), sp([side * 9., -38., -4.])] {
                hinge_gap = hinge_gap.max(pinned(&reference, &pose, ids, point, scale));
            }
            // The free edge's distance from the side line through the hinge
            // and the fuselage skin direction (-5, -20).
            let hinge = sp([side * 9., -38., 0.]);
            let free = moves(&reference, &pose, ids, sp([side * 13., -43., 0.]));
            for q in free {
                let d = [(q[0] - hinge[0]) * side, q[1] - hinge[1]];
                let skin = [-5f32 / 425f32.sqrt(), -20f32 / 425f32.sqrt()];
                off = off.max((d[0] * skin[1] - d[1] * skin[0]).abs() * scale);
            }
        }
        let closed = value < 1e-5;
        let passed = rigid <= EPSILON && hinge_gap <= EPSILON && (!closed || off <= 0.01);
        writeln!(csv, "{value},{rigid},{hinge_gap},{off},{passed}")?;
        if !passed {
            failures.push(format!(
                "Gripen brake value{value}: rigid, hinge or side-line gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}
