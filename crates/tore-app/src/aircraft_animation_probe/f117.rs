//! Independent F-117A whole-panel witnesses: the two ruddervator assemblies
//! (mixed pitch and yaw, 10 faces a side), the rigid gear (three legs, three
//! doors, the door skins) and the fitted weapon bays (doors cut from the belly, lined recess). The elevons, flaps,
//! airbrake and plume are not in the source, so the probe requires that those
//! controls move nothing. Measures the shared Airframe output and never repeats
//! the rig's algorithm. Witness points are named in shape units and scaled with
//! the aircraft's own per-axis scale (the probe geometry is the scaled shape).
use super::*;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::F117;
const TAIL_R: [usize; 10] = [
    0x22c5, 0x22d9, 0x231a, 0x232e, 0x2383, 0x244b, 0x2460, 0x2475, 0x24b3, 0x2e40,
];
const TAIL_L: [usize; 10] = [
    0x23ab, 0x23bd, 0x23d0, 0x23f7, 0x240b, 0x2425, 0x2489, 0x249e, 0x24f6, 0x255e,
];
const LEG_R: [usize; 6] = [0x3286, 0x32a1, 0x32bc, 0x32d7, 0x32f2, 0x3305];
const LEG_L: [usize; 6] = [0x33d8, 0x33f3, 0x340e, 0x3429, 0x3444, 0x3457];
const LEG_NOSE: [usize; 6] = [0x3765, 0x3784, 0x37e8, 0x3807, 0x3826, 0x3845];
const DOOR_R: [usize; 2] = [0x3351, 0x3366];
const DOOR_L: [usize; 2] = [0x34a3, 0x34b8];
const DOOR_NOSE: [usize; 2] = [0x3897, 0x38b0];
const SKIN: [usize; 7] = [0x3676, 0x36fa, 0x36a1, 0x36c0, 0x36df, 0x3719, 0x3657];
const GEAR: [usize; 31] = [
    0x3286, 0x32a1, 0x32bc, 0x32d7, 0x32f2, 0x3305, 0x33d8, 0x33f3, 0x340e, 0x3429, 0x3444, 0x3457,
    0x3765, 0x3784, 0x37e8, 0x3807, 0x3826, 0x3845, 0x3351, 0x3366, 0x34a3, 0x34b8, 0x3897, 0x38b0,
    0x3676, 0x36fa, 0x36a1, 0x36c0, 0x36df, 0x3719, 0x3657,
];
/// The six belly skins the two openings are cut from, and where the
/// fitted lining's addresses start.
const BELLY: [usize; 6] = [0x34e6, 0x2f0a, 0x2f1d, 0x2f5d, 0x2f8e, 0x3076];
const LINING_BASE: usize = 0x7000;
/// Lining faces: per bay a ceiling, four walls and a five-face rail.
const LINING: usize = 20;

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

/// Skin gap inside one group: shared vertices of the two windings of a plate
/// must move together.
fn group_skin_gap(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
}

fn anchor(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    point: [f32; 3],
    scale: f32,
    m: &mut Metrics,
) {
    let gap = pinned(reference, pose, ids, point, scale);
    m.reviewed_anchor_missing |= !gap.is_finite();
    if gap.is_finite() {
        m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
    }
}

fn unchanged(reference: &[Face], pose: &[Face], ids: &[usize], m: &mut Metrics) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    for (key, face) in &old {
        match new.get(key) {
            Some(after) if face.positions == after.positions => {}
            _ => m.reviewed_neutral_mismatch = true,
        }
    }
}

/// One assembly: rigid as a whole panel and its shared vertices together.
fn assembly(ids: &[usize], reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    group_skin_gap(reference, pose, ids, scale, m);
    let error = rigidity(reference, pose, ids, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
}

/// Trailing-edge root corner of each assembly (shape units), the vertex a pitch
/// raises and a right pedal swings toward +X.
fn edge(right: bool) -> [f32; 3] {
    sp(if right {
        [3., -79., -1.]
    } else {
        [-3., -79., -1.]
    })
}

fn tails(c: Control, value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (ids, right) in [(&TAIL_R[..], true), (&TAIL_L[..], false)] {
        assembly(ids, reference, pose, scale, m);
        if value == 0. {
            unchanged(reference, pose, ids, m);
            continue;
        }
        let witness = edge(right);
        let old = group(reference, ids);
        let new = group(pose, ids);
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            // A pull raises both trailing edges; a right pedal swings both
            // toward +X.
            let delta = if matches!(c, Control::Elevator) {
                q[2] - witness[2]
            } else {
                q[0] - witness[0]
            };
            m.reviewed_direction_failures += usize::from(delta * value.signum() as f32 <= EPSILON);
        }
    }
}

/// How far the stowed legs, doors and skins reach outside the skin, in feet:
/// for each gear vertex the height must lie between the lowest and highest
/// fixed face over its plan position (the neutral airframe, gear up); the worst
/// excess is returned (a vertex with no fixed face over it counts as 1000).
fn envelope(neutral: &[Face], pose: &[Face], scale: f32) -> f32 {
    let mut worst = 0f32;
    for face in pose.iter().filter(|f| GEAR.contains(&f.address)) {
        for p in &face.positions {
            let mut range: Option<(f32, f32)> = None;
            for fixed in neutral {
                for i in 1..fixed.positions.len().saturating_sub(1) {
                    let (a, b, c) = (
                        fixed.positions[0],
                        fixed.positions[i],
                        fixed.positions[i + 1],
                    );
                    let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
                    if d.abs() < 1e-9 {
                        continue;
                    }
                    let l1 = ((b[1] - c[1]) * (p[0] - c[0]) + (c[0] - b[0]) * (p[1] - c[1])) / d;
                    let l2 = ((c[1] - a[1]) * (p[0] - c[0]) + (a[0] - c[0]) * (p[1] - c[1])) / d;
                    let l3 = 1. - l1 - l2;
                    if l1.min(l2).min(l3) >= -1e-6 {
                        let z = l1 * a[2] + l2 * b[2] + l3 * c[2];
                        range = Some(range.map_or((z, z), |(lo, hi)| (lo.min(z), hi.max(z))));
                    }
                }
            }
            worst = worst.max(match range {
                Some((lo, hi)) => (lo - p[2]).max(p[2] - hi).max(0.) * scale,
                None => 1000.,
            });
        }
    }
    worst
}

/// Gear: each leg, door and the skin set is one rigid piece, hinged on its own
/// top edge (mains in, nose forward, doors in, skins lifting), grounded at 1 and
/// never lower than the gear-down pose while closing.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    let zs = sp([0., 0., 1.])[2];
    for ids in [
        &LEG_R[..],
        &LEG_L[..],
        &LEG_NOSE[..],
        &DOOR_R[..],
        &DOOR_L[..],
        &DOOR_NOSE[..],
        &SKIN[..],
    ] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
        group_skin_gap(reference, pose, ids, scale, m);
    }
    // Each hinge edge stays: a vertex of every piece on its top edge (the nose
    // leg turns about a line with no vertex on it).
    for (ids, points) in [
        (&LEG_R[..], vec![sp([30., -15., -4.])]),
        (&LEG_L[..], vec![sp([-31., -15., -4.])]),
        (&DOOR_R[..], vec![sp([23., -4., -4.]), sp([23., 4., -4.])]),
        (&DOOR_L[..], vec![sp([-25., -4., -4.]), sp([-25., 4., -4.])]),
        (&DOOR_NOSE[..], vec![sp([3., 39., -4.]), sp([3., 58., -4.])]),
    ] {
        for point in points {
            anchor(reference, pose, ids, point, scale, m);
        }
    }
    // Closing never lowers the plates.
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // Ground contact: the lowest point of the gear-down pose is the scaled
        // 21 unit low point the flight model stands the aircraft on.
        let want = -21. / 3. * zs;
        m.reviewed_wheel_failed |= (pose_low * scale - want).abs() > 1e-3;
    } else {
        // Direction: a main's foot moves inward and the nose foot forward.
        for (witness, ids, inward) in [
            (sp([28., -19., -21.]), &LEG_R[..], -1f32),
            (sp([-29., -19., -21.]), &LEG_L[..], 1.),
        ] {
            let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures += usize::from((q[0] - witness[0]) * inward <= 0.);
            }
        }
        let foot = sp([0., 43., -20.]);
        let qs = f4::witness_positions(&group(reference, &LEG_NOSE), &keyed(pose), foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[1] <= foot[1]);
        }
    }
}

/// The fitted bays: shut (0) the six belly skins are whole, nothing hangs
/// below the belly plane and no lining draws; open (above 0) the lining draws
/// and the four doors hang on their hinge lines (the inner and outer edges of
/// each bay at the belly plane) with their free edges `leaf * sin(90 degrees *
/// value)` below it.
fn bay(value: f64, _reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let o = tore_sim::combat::bays::opening(ID).expect("F-117 opening");
    let belly = sp([0., 0., -5.])[2];
    let lining = pose.iter().filter(|f| f.address >= LINING_BASE).count();
    let skins: Vec<&Face> = pose.iter().filter(|f| BELLY.contains(&f.address)).collect();
    let lowest = skins
        .iter()
        .flat_map(|f| f.positions.iter())
        .map(|p| p[2])
        .fold(f32::MAX, f32::min);
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(lining != 0 || skins.len() != BELLY.len() || lowest < belly - 1e-4);
        return;
    }
    m.reviewed_direction_failures += usize::from(lining != LINING);
    // A vertex stays on each hinge line at the belly plane.
    for feet in [o.inner, o.outer, -o.inner, -o.outer] {
        let hinge = feet as f32 / scale;
        let held = skins
            .iter()
            .flat_map(|f| f.positions.iter())
            .any(|p| (p[0] - hinge).abs() < 1e-3 && (p[2] - belly).abs() < 1e-3);
        m.reviewed_anchor_missing |= !held;
    }
    let leaf = ((o.outer - o.inner) / 2.) as f32 / scale;
    let hang = leaf * (value as f32 * std::f32::consts::FRAC_PI_2).sin();
    m.reviewed_direction_failures += usize::from((belly - lowest - hang).abs() > 2e-3);
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
        Control::Bay => bay(value, reference, pose, scale, m),
        Control::Elevator | Control::Rudder => tails(c, value, reference, pose, scale, m),
        _ => {}
    }
}

pub(super) fn triangle_distance(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let sub = |u: [f32; 3], v: [f32; 3]| [u[0] - v[0], u[1] - v[1], u[2] - v[2]];
    let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let at = |o: [f32; 3], d: [f32; 3], t: f32| [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0. && d2 <= 0. {
        return distance(p, a);
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0. && d4 <= d3 {
        return distance(p, b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return distance(p, at(a, ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0. && d5 <= d6 {
        return distance(p, c);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return distance(p, at(a, ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return distance(p, at(b, sub(c, b), w));
    }
    let denominator = 1. / (va + vb + vc);
    let (v, w) = (vb * denominator, vc * denominator);
    let q = [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ];
    distance(p, q)
}

/// The root seam. The assembly shares no vertex with the fuselage, so a turn
/// opens (or buries) the root. Measured, not gated: for each root vertex the
/// distance to the nearest fixed face, and how far the vertex moved. Returns
/// the worst of each, in feet.
fn root_seam(neutral: &[Face], pose: &[Face], right: bool, scale: f32) -> (f32, f32) {
    let ids: &[usize] = if right { &TAIL_R } else { &TAIL_L };
    let roots: [[f32; 3]; 4] = if right {
        [
            [1., -55., 3.],
            [1., -68., 0.],
            [3., -68., 0.],
            [3., -79., -1.],
        ]
    } else {
        [
            [-2., -55., 3.],
            [-1., -68., 0.],
            [-3., -68., 0.],
            [-3., -79., -1.],
        ]
    };
    let old = group(neutral, ids);
    let new = group(pose, ids);
    let fixed: Vec<&Face> = neutral
        .iter()
        .filter(|f| !TAIL_R.contains(&f.address) && !TAIL_L.contains(&f.address))
        .collect();
    let (mut gap, mut moved) = (0f32, 0f32);
    for root in roots {
        let root = sp(root);
        for q in f4::witness_positions(&old, &new, root) {
            moved = moved.max(distance(root, q) * scale);
            let mut nearest = f32::INFINITY;
            for face in &fixed {
                for i in 1..face.positions.len().saturating_sub(1) {
                    nearest = nearest.min(triangle_distance(
                        q,
                        face.positions[0],
                        face.positions[i],
                        face.positions[i + 1],
                    ));
                }
            }
            gap = gap.max(nearest * scale);
        }
    }
    (gap, moved)
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut rows = String::from(
        "pitch,yaw,finite,skin_gap_ft,rigidity_ft,right_seam_gap_ft,left_seam_gap_ft,crossings,direction_failures\n",
    );
    let mut poses = Vec::new();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for yaw in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.rudder = yaw;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            for (ids, right) in [(&TAIL_R[..], true), (&TAIL_L[..], false)] {
                assembly(ids, &original, &pose, scale, &mut m);
                // Direction: the net raise of a trailing edge has the sign of
                // 3 * pitch (a pull raises both) and, for yaw, 2 * yaw lowering
                // the right panel and raising the left; equal terms cancel.
                let net = 3. * pitch + if right { -2. } else { 2. } * yaw;
                if net != 0. {
                    let witness = edge(right);
                    let qs =
                        f4::witness_positions(&group(&original, ids), &group(&pose, ids), witness);
                    m.reviewed_direction_failures += usize::from(qs.is_empty());
                    for q in qs {
                        m.reviewed_direction_failures +=
                            usize::from((q[2] - witness[2]) * net.signum() as f32 <= EPSILON);
                    }
                }
            }
            let (right_gap, _) = root_seam(&original, &pose, true, scale);
            let (left_gap, _) = root_seam(&original, &pose, false, scale);
            let passed = m.finite
                && m.max_reviewed_skin_gap <= EPSILON
                && m.reviewed_rigid_panel_error <= EPSILON
                && m.reviewed_direction_failures == 0
                && m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{pitch},{yaw},{},{},{},{right_gap},{left_gap},{},{}",
                m.finite,
                m.max_reviewed_skin_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "F-117 pitch{pitch}/yaw{yaw}: whole-assembly rigidity, skin or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-yaw-combinations.csv"), rows)?;
    contact_sheet(&out.join("pitch-yaw-combinations.ppm"), &original, &poses)?;
    // The root seam at the named poses, measured next to the grid: the gap is
    // the distance from the worst root vertex to the nearest fixed face, the
    // move is how far that vertex travelled.
    let mut rows =
        String::from("pose,elevator,rudder,right_gap_ft,right_move_ft,left_gap_ft,left_move_ft\n");
    for (name, elevator, rudder) in [
        ("neutral", 0., 0.),
        ("pitch-up", 1., 0.),
        ("pitch-down", -1., 0.),
        ("yaw-right", 0., 1.),
        ("yaw-left", 0., -1.),
        ("pitch-up-yaw-right", 1., 1.),
        ("pitch-up-yaw-left", 1., -1.),
        ("pitch-down-yaw-right", -1., 1.),
        ("pitch-down-yaw-left", -1., -1.),
    ] {
        let mut state = neutral.clone();
        state.elevator = elevator;
        state.rudder = rudder;
        let pose = airframe.animation_faces(&state);
        let (rg, rm) = root_seam(&original, &pose, true, scale);
        let (lg, lm) = root_seam(&original, &pose, false, scale);
        writeln!(rows, "{name},{elevator},{rudder},{rg},{rm},{lg},{lm}")?;
    }
    fs::write(out.join("seam-gap.csv"), rows)?;
    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_bay(airframe, neutral, out, scale)?);
    Ok(failures)
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, hinged, grounded at 1 and tucked inside the skin near 0.
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
        "value,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,skin_overshoot_ft,checks_passed\n",
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
        // Fully stowed (every piece closed, doors included) the pieces lie
        // inside the skin, within 0.3 ft (the measured worst case is the
        // nose strut's top corner under the cockpit roof, see the docs).
        let outside = if value < 1e-5 {
            envelope(original, &pose, scale)
        } else {
            0.
        };
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && outside <= 0.3
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{},{},{outside},{passed}",
            m.max_reviewed_anchor_gap,
            m.max_reviewed_skin_gap,
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "F-117 dense gear value{value}: rigid, hinge, direction, stow or skin gate"
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

/// Bay sweep: the doors swing between shut (a near-zero sample) and hanging,
/// on their hinges and at the sine of the travel at every value.
fn dense_bay(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut open = neutral.clone();
    Control::Bay.apply(&mut open, 1.);
    let reference = airframe.animation_faces(&open);
    let mut csv = String::from("value,anchor_gap_ft,rigidity_ft,direction_failures,passed\n");
    for value in [0., 1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.bay = value;
        let pose = airframe.animation_faces(&state);
        let mut m = measure(&reference, &pose, scale);
        bay(value, &reference, &pose, scale, &mut m);
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.reviewed_rigid_panel_error <= EPSILON
            && m.reviewed_direction_failures == 0;
        writeln!(
            csv,
            "{value},{},{},{},{passed}",
            m.max_reviewed_anchor_gap, m.reviewed_rigid_panel_error, m.reviewed_direction_failures
        )?;
        if !passed {
            failures.push(format!(
                "F-117 bay value{value}: rigid, hinge or direction gate"
            ));
        }
    }
    fs::write(out.join("bay-sweep.csv"), csv)?;
    Ok(failures)
}
