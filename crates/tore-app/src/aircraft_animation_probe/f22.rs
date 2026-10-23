//! Independent source seams, whole gear assemblies and FAXX split-law witnesses.
use super::*;
use std::collections::BTreeSet;
use tore_formats::{aircraft::AircraftId as Id, shape::Shape};
pub(super) struct Sources {
    id: Id,
    neutral: Vec<Face>,
    gear: Vec<Face>,
    brake: Vec<Face>,
    hook: Vec<Face>,
}
impl Sources {
    pub(super) fn load(id: Id, bytes: &[u8]) -> AppResult<Self> {
        let n = id.source() == Id::F22n;
        let neutral = Shape::parse(bytes)?.faces;
        let base: BTreeSet<_> = neutral.iter().map(|f| f.address).collect();
        let branch = |word| -> AppResult<Vec<Face>> {
            Ok(Shape::with_state(bytes, &[(word, 1)].into())?
                .faces
                .into_iter()
                .filter(|f| !base.contains(&f.address))
                .collect())
        };
        let gear = branch(if n { 0x5e8e } else { 0x5e0e })?;
        let brake = branch(if n { 0x5e82 } else { 0x5e02 })?;
        let hook = if n { branch(0x5e9a)? } else { vec![] };
        if gear.len() != 12 || brake.len() != 4 || hook.len() != if n { 2 } else { 0 } {
            return Err("F22 independent source groups changed".into());
        }
        Ok(Self {
            id,
            neutral,
            gear,
            brake,
            hook,
        })
    }
}
fn base(a: usize, id: Id) -> usize {
    if id.source() == Id::F22n {
        a.saturating_sub(0xf8)
    } else {
        a
    }
}
fn inner(a: usize, id: Id) -> bool {
    matches!(
        base(a, id),
        0x437f | 0x439e | 0x43fb | 0x4416 | 0x4576 | 0x4599 | 0x45f6
    )
}
fn outer(a: usize, id: Id) -> bool {
    matches!(base(a, id), 0x43bd | 0x43dc | 0x45b8 | 0x45d7)
}
fn tail(a: usize, id: Id) -> bool {
    if id.source() == Id::F22n {
        matches!(a, 0x31eb | 0x31fd | 0x36d3 | 0x36e7 | 0x3ada | 0x3cff)
    } else {
        matches!(a, 0x32ba | 0x32cc | 0x35ad | 0x35c1 | 0x3987 | 0x3ba9)
    }
}
fn fin(a: usize, id: Id) -> bool {
    if id.source() == Id::F22n {
        matches!(a, 0x361d | 0x3670 | 0x38e4 | 0x3903 | 0x3926)
    } else {
        matches!(a, 0x34f7 | 0x354a | 0x37ff | 0x381e | 0x3841)
    }
}
fn controlled(a: usize, id: Id, c: Control) -> bool {
    match c {
        Control::Flaps => inner(a, id),
        Control::Elevator => tail(a, id),
        Control::Aileron => outer(a, id) || tail(a, id),
        Control::Rudder => fin(a, id),
        _ => false,
    }
}
fn anchor(p: [f32; 3], a: usize, id: Id, c: Control) -> bool {
    match c {
        Control::Flaps => p[1] == -28. || p[1] == -22.,
        Control::Aileron if outer(a, id) => p[1] == -22. || p[1] == -26.,
        Control::Elevator | Control::Aileron => p[0].abs() <= 19.,
        Control::Rudder => p[2] <= 6. + EPSILON || (p[1] + 35. - 0.25 * p[2]).abs() <= EPSILON,
        _ => false,
    }
}
fn endpoint(originals: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for f in originals {
        let found = pose.iter().find(|g| g.address == f.address);
        m.reviewed_neutral_mismatch |= found.is_none_or(|g| {
            g.positions.len() != f.positions.len()
                || f.positions
                    .iter()
                    .zip(&g.positions)
                    .any(|(p, q)| distance(*p, *q) * scale > EPSILON)
        });
    }
}
pub(super) fn check(
    c: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    source: &Sources,
    m: &mut Metrics,
) {
    let (_, reference, pose) = geometry;
    let id = source.id;
    if matches!(c, Control::Gear | Control::Brake | Control::Hook) {
        let originals = match c {
            Control::Gear => &source.gear,
            Control::Brake => &source.brake,
            _ => &source.hook,
        };
        if originals.is_empty() {
            return;
        }
        if value == 0. {
            m.reviewed_wheel_failed |= pose
                .iter()
                .any(|f| originals.iter().any(|g| g.address == f.address));
            return;
        }
        if value == 1. {
            endpoint(originals, pose, scale, m);
        }
        devices(c, value, reference, pose, source, scale, m);
        return;
    }
    if id == Id::Faxx && matches!(c, Control::Flaps | Control::Rudder) {
        concept(reference, pose, source, scale, m);
        if matches!(c, Control::Rudder) {
            return;
        }
    }
    let before: BTreeMap<_, _> = keyed(reference)
        .into_iter()
        .filter(|(k, _)| controlled(k.0, id, c))
        .collect();
    let after: BTreeMap<_, _> = keyed(pose)
        .into_iter()
        .filter(|(k, _)| controlled(k.0, id, c))
        .collect();
    if before.is_empty() {
        if matches!(
            c,
            Control::Elevator | Control::Aileron | Control::Flaps | Control::Rudder
        ) {
            m.reviewed_anchor_missing = true;
        }
        return;
    }
    if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(gap.gap);
    }
    for (k, f) in &before {
        let Some(g) = after.get(k) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        if f.positions.len() != g.positions.len() {
            m.reviewed_anchor_missing = true;
            continue;
        }
        for (p, q) in f.positions.iter().zip(&g.positions) {
            if anchor(*p, k.0, id, c) {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
            } else if value != 0. {
                let axis = if matches!(c, Control::Rudder) { 0 } else { 2 };
                let moving = if matches!(c, Control::Rudder) {
                    p[2] > 6. + EPSILON && p[1] + 35. - 0.25 * p[2] < -1.
                } else {
                    true
                };
                if moving {
                    let expected = match c {
                        Control::Flaps => -1.,
                        Control::Aileron => p[0].signum(),
                        _ => 1.,
                    };
                    let delta = (q[axis] - p[axis]) * scale;
                    m.reviewed_direction_failures +=
                        usize::from(delta * expected * value.signum() as f32 <= EPSILON);
                    if axis == 2 {
                        m.reviewed_control_z_delta[usize::from(p[0] > 0.)] = delta;
                    }
                }
            }
        }
    }
    if value == 0. {
        for f in source
            .neutral
            .iter()
            .filter(|f| controlled(f.address, id, c))
        {
            for p in &f.positions {
                m.reviewed_neutral_mismatch |=
                    !pose.iter().filter(|g| g.address == f.address).any(|g| {
                        g.positions
                            .iter()
                            .any(|q| distance(*p, *q) * scale <= EPSILON)
                    });
            }
        }
    }
}
// Concept leaves deliberately separate. Check each leaf's source attachments and
// compare corresponding upper/lower faces within that leaf, never across leaves.
fn concept(reference: &[Face], pose: &[Face], source: &Sources, scale: f32, m: &mut Metrics) {
    m.reviewed_direction_failures += usize::from(pose.iter().any(|f| fin(f.address, source.id)));
    for side in [-1., 1.] {
        let originals: Vec<_> = reference
            .iter()
            .filter(|f| inner(f.address, source.id) && f.positions[0][0].signum() == side)
            .collect();
        let counts: Vec<_> = originals
            .iter()
            .map(|f| pose.iter().filter(|g| g.address == f.address).count())
            .collect();
        m.reviewed_anchor_missing |= counts.is_empty()
            || counts.iter().any(|c| *c != counts[0] || *c == 0)
            || !matches!(counts.first(), Some(1 | 2));
        for leaf in 0..counts.first().copied().unwrap_or(0) {
            let mut before = BTreeMap::new();
            let mut after = BTreeMap::new();
            for f in &originals {
                let Some(g) = pose.iter().filter(|g| g.address == f.address).nth(leaf) else {
                    m.reviewed_anchor_missing = true;
                    continue;
                };
                before.insert((f.address, 0), *f);
                after.insert((f.address, 0), g);
                if f.positions.len() != g.positions.len() {
                    m.reviewed_anchor_missing = true;
                    continue;
                }
                for (p, q) in f.positions.iter().zip(&g.positions) {
                    if p[1] == -28. || p[1] == -22. {
                        m.max_reviewed_anchor_gap =
                            m.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                    }
                }
            }
            if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
                m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(gap.gap);
            }
        }
    }
}
fn affine_anchor(
    reference: &[Face],
    pose: &[Face],
    address: usize,
    pivot: [f32; 3],
) -> Option<[f32; 3]> {
    let f = reference.iter().find(|f| f.address == address)?;
    let g = pose.iter().find(|f| f.address == address)?;
    if f.positions.len() < 3 || g.positions.len() != f.positions.len() {
        return None;
    }
    if let Some(i) = f
        .positions
        .iter()
        .position(|p| distance(*p, pivot) <= EPSILON)
    {
        return g.positions.get(i).copied();
    }
    let sub = |a: [f32; 3], b: [f32; 3]| std::array::from_fn::<_, 3, _>(|i| a[i] - b[i]);
    let dot = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
    let u = sub(f.positions[1], f.positions[0]);
    let v = sub(f.positions[2], f.positions[0]);
    let p = sub(pivot, f.positions[0]);
    let uu = dot(u, u);
    let vv = dot(v, v);
    let uv = dot(u, v);
    let d = uu * vv - uv * uv;
    if d.abs() < 1e-6 {
        return None;
    }
    let b = (dot(p, u) * vv - dot(p, v) * uv) / d;
    let c = (dot(p, v) * uu - dot(p, u) * uv) / d;
    Some(std::array::from_fn(|i| {
        g.positions[0][i]
            + b * (g.positions[1][i] - g.positions[0][i])
            + c * (g.positions[2][i] - g.positions[0][i])
    }))
}

fn devices(
    c: Control,
    value: f64,
    reference: &[Face],
    pose: &[Face],
    source: &Sources,
    scale: f32,
    m: &mut Metrics,
) {
    let n = source.id.source() == Id::F22n;
    let off = if n { 0xf8 } else { 0 };
    let anchors: Vec<_> = match c {
        Control::Gear => vec![
            (0x4135 + off, [-13., 2.25, -5.]),
            (0x40ca + off, [12., 2.25, -6.]),
            (0x41fd + off, [0., 63., -9.]),
            (0x4014 + off, [18., -5., -3.]),
            (0x4014 + off, [18., 23., -1.]),
            (0x406f + off, [-16., -5., -3.]),
            (0x406f + off, [-16., 23., -1.]),
            (0x4192 + off, [-2., 55., -8.]),
            (0x4192 + off, [-2., 74., -8.]),
        ],
        Control::Brake => vec![
            (0x46d3 + off, [0., -17., 5.]),
            (0x46d3 + off, [-3., -15., 6.]),
            (0x4701 + off, [3., -15., 6.]),
        ],
        Control::Hook => vec![(0x40a1, [0., -9., -9.])],
        _ => vec![],
    };
    for (a, p) in anchors {
        if let Some(q) = affine_anchor(reference, pose, a, p) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, q) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
    }
    let originals = match c {
        Control::Gear => &source.gear,
        Control::Brake => &source.brake,
        _ => &source.hook,
    };
    let before: BTreeMap<_, _> = keyed(reference)
        .into_iter()
        .filter(|(k, _)| originals.iter().any(|f| f.address == k.0))
        .collect();
    let after: BTreeMap<_, _> = keyed(pose)
        .into_iter()
        .filter(|(k, _)| originals.iter().any(|f| f.address == k.0))
        .collect();
    if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(gap.gap);
    }
    let mut left = f32::NEG_INFINITY;
    let mut right = f32::INFINITY;
    for f in originals {
        let Some(g) = pose.iter().find(|g| g.address == f.address) else {
            m.reviewed_wheel_failed = true;
            continue;
        };
        if f.positions.len() != g.positions.len() {
            m.reviewed_wheel_failed = true;
            continue;
        }
        for (i, p) in f.positions.iter().enumerate() {
            for (j, q) in f.positions.iter().enumerate().skip(i + 1) {
                if !matches!(c, Control::Brake) {
                    m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(
                        (distance(*p, *q) - distance(g.positions[i], g.positions[j])).abs() * scale,
                    );
                }
            }
            if matches!(c, Control::Gear) {
                let a = base(f.address, source.id);
                let q = g.positions[i];
                if matches!(a, 0x4135 | 0x4154) {
                    left = left.max(q[0] * scale);
                } else if matches!(a, 0x40ca | 0x40e9) {
                    right = right.min(q[0] * scale);
                }
                if value < 1e-5 {
                    let bounds = match a {
                        0x4135 | 0x4154 => Some(([-17.90, -16.22, -8.06], [-2.09, 3.40, 2.33])),
                        0x40ca | 0x40e9 => Some(([1.58, -15.35, -9.06], [16.89, 3.40, 1.33])),
                        0x41fd | 0x421c => Some(([0., 58.79, -9.27], [0., 66.99, 5.30])),
                        _ => None,
                    };
                    if let Some((lo, hi)) = bounds {
                        m.reviewed_wheel_failed |=
                            (0..3).any(|k| q[k] < lo[k] - EPSILON || q[k] > hi[k] + EPSILON);
                    }
                }
            }
        }
    }
    if matches!(c, Control::Gear) {
        m.reviewed_min_wheel_gap = Some(right - left);
        m.reviewed_wheel_failed |= right - left < 1.20;
    }
    m.reviewed_wheel_failed |= m.reviewed_wheel_rigidity_error > EPSILON;
}
// Recover the signed angle from a free source trailing point and the reviewed
// rightward seam. This checks the concept's numerical law without calling its rig.
fn leaf_angle(p: [f32; 3], q: [f32; 3], side: f32) -> f64 {
    let pivot = if side < 0. {
        [-17., -28., 0.]
    } else {
        [16., -28., 0.]
    };
    let length = (26_f64.powi(2) + 6_f64.powi(2) + 0.5_f64.powi(2)).sqrt();
    let axis = [
        26. / length,
        f64::from(side) * 6. / length,
        f64::from(side) * 0.5 / length,
    ];
    let projected = |v: [f32; 3]| {
        let r = std::array::from_fn::<_, 3, _>(|i| f64::from(v[i] - pivot[i]));
        let dot = (0..3).map(|i| r[i] * axis[i]).sum::<f64>();
        std::array::from_fn::<_, 3, _>(|i| r[i] - axis[i] * dot)
    };
    let a = projected(p);
    let b = projected(q);
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let sine = (0..3).map(|i| cross[i] * axis[i]).sum::<f64>();
    let cosine = (0..3).map(|i| a[i] * b[i]).sum::<f64>();
    sine.atan2(cosine)
}
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    source: &Sources,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = vec![];
    let mut poses = vec![];
    let mut rows = String::from("flap,yaw,anchor_gap_ft,skin_gap_ft,failures\n");
    let mut tail_rows = String::from("pitch,roll,anchor_gap_ft,skin_gap_ft,failures\n");
    let mut tail_poses = Vec::new();
    let tail_before: BTreeMap<_, _> = keyed(&original)
        .into_iter()
        .filter(|(k, _)| tail(k.0, source.id))
        .collect();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.aileron = roll;
            let actual = airframe.animation_faces(&state);
            let tail_after: BTreeMap<_, _> = keyed(&actual)
                .into_iter()
                .filter(|(k, _)| tail(k.0, source.id))
                .collect();
            let mut m = measure(&original, &actual, scale);
            m.reviewed_anchor_missing |= tail_before.is_empty();
            if let Some(gap) = shared_vertex_gaps(&tail_before, &tail_after, scale).first() {
                m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(gap.gap);
            }
            for (key, f) in &tail_before {
                let Some(g) = tail_after.get(key) else {
                    m.reviewed_anchor_missing = true;
                    continue;
                };
                if f.positions.len() != g.positions.len() {
                    m.reviewed_anchor_missing = true;
                    continue;
                }
                for (p, q) in f.positions.iter().zip(&g.positions) {
                    let effective = 0.3 * pitch + 0.1 * f64::from(p[0].signum()) * roll;
                    if p[0].abs() <= 19. {
                        m.max_reviewed_anchor_gap =
                            m.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                    } else if effective.abs() < 1e-8 {
                        m.reviewed_direction_failures +=
                            usize::from(distance(*p, *q) * scale > EPSILON);
                    } else {
                        m.reviewed_direction_failures +=
                            usize::from(f64::from(q[2] - p[2]) * effective <= f64::from(EPSILON));
                    }
                }
            }
            let failed = !m.finite
                || m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.max_reviewed_skin_gap > EPSILON
                || m.reviewed_direction_failures > 0
                || !m.new_planar_crossings.is_empty();
            writeln!(
                tail_rows,
                "{pitch},{roll},{},{},{}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                usize::from(failed)
            )?;
            if failed {
                failures.push(format!(
                    "F22-family combined pitch{pitch}/roll{roll} tail witnesses"
                ));
            }
            tail_poses.push(actual);
        }
    }
    fs::write(out.join("pitch-roll-combinations.csv"), tail_rows)?;
    contact_sheet(
        &out.join("pitch-roll-combinations.ppm"),
        &original,
        &tail_poses,
    )?;
    if source.id != Id::Faxx {
        return Ok(failures);
    }
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        for yaw in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.flaps = flap;
            state.rudder = yaw;
            let actual = airframe.animation_faces(&state);
            let mut m = measure(&original, &actual, scale);
            concept(&original, &actual, source, scale, &mut m);
            for side in [-1., 1.] {
                let expected = if side * yaw > 0. { 2 } else { 1 };
                for f in original.iter().filter(|f| {
                    inner(f.address, source.id) && f.positions[0][0].signum() as f64 == side
                }) {
                    m.reviewed_direction_failures += usize::from(
                        actual.iter().filter(|g| g.address == f.address).count() != expected,
                    );
                }
            }
            for side in [-1_f32, 1.] {
                let address = if side < 0. { 0x4691 } else { 0x4477 };
                let source_face = original.iter().find(|f| f.address == address);
                let opening = (f64::from(side) * yaw).max(0.) * 0.6;
                let expected = if opening > 0. {
                    vec![0.4 * flap - opening, 0.4 * flap + opening]
                } else {
                    vec![0.4 * flap]
                };
                if let Some(source_face) = source_face {
                    for (leaf, expected_angle) in expected.iter().enumerate() {
                        let actual_point = actual
                            .iter()
                            .filter(|f| f.address == address)
                            .nth(leaf)
                            .and_then(|f| f.positions.first());
                        m.reviewed_direction_failures +=
                            usize::from(actual_point.is_none_or(|q| {
                                (leaf_angle(source_face.positions[0], *q, side) - expected_angle)
                                    .abs()
                                    > 1e-5
                            }));
                    }
                } else {
                    m.reviewed_anchor_missing = true;
                }
            }
            let failed = !m.finite
                || m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.max_reviewed_skin_gap > EPSILON
                || m.reviewed_direction_failures > 0
                || !m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{flap},{yaw},{},{},{}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                usize::from(failed)
            )?;
            if failed {
                failures.push(format!(
                    "FAXX flap{flap}/yaw{yaw} independent split-leaf contract"
                ));
            }
            poses.push(actual);
        }
    }
    fs::write(out.join("flap-yaw-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-yaw-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
