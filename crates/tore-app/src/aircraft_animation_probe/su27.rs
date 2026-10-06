//! Independent Su27 own roots, signed endpoints, slats and separated rigid wheels.
use super::*;
const TAIL: [usize; 4] = [0x16d5, 0x170f, 0x1928, 0x19be];
const FIN: [usize; 4] = [0x17f8, 0x1831, 0x1ae5, 0x1af9];
const GEAR: [usize; 8] = [
    0x2b74, 0x2b8f, 0x2bd7, 0x2bf2, 0x2c2c, 0x2c47, 0x2c8f, 0x2caa,
];
const SLATS: [usize; 4] = [0x2d29, 0x2d3c, 0x2ce4, 0x2cf7];
fn groups(c: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match c {
        Control::Elevator => vec![
            (&[0x16d5, 0x170f], &[[-21., -26., -6.], [-21., -56., -6.]]),
            (&[0x1928, 0x19be], &[[21., -26., -6.], [21., -56., -6.]]),
        ],
        Control::Flaps | Control::Aileron => vec![
            (&[0x2e82, 0x2e95], &[[-21., -10., -1.], [-78., -26., -1.]]),
            (&[0x2db3, 0x2dc6], &[[21., -10., -1.], [79., -26., -1.]]),
        ],
        Control::Gear => vec![(&[0x2c2c, 0x2c47], &[[-1., 74., -4.], [-1., 74., -7.]])],
        Control::Brake => vec![(&[0x2f69, 0x2f7c], &[[-6., 40., 11.], [7., 40., 11.]])],
        _ => Vec::new(),
    }
}
fn anchors(c: Control, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (ids, points) in groups(c) {
        let old: BTreeMap<_, _> = keyed(reference)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        for p in points {
            let qs = f4::witness_positions(&old, &new, *p);
            m.reviewed_anchor_missing |= qs.is_empty();
            for q in qs {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(*p, q) * scale);
            }
        }
        if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
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
    let (raw, reference, pose) = geometry;
    if matches!(c, Control::Gear) && value == 0. {
        m.reviewed_wheel_failed = pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    if matches!(c, Control::Brake) && value == 0. {
        m.reviewed_wheel_failed = pose.iter().any(|f| [0x2f69, 0x2f7c].contains(&f.address));
        return;
    }
    let source = if matches!(c, Control::Gear | Control::Brake | Control::Rudder) {
        reference
    } else {
        raw
    };
    anchors(c, source, pose, scale, m);
    let signed: Vec<(&[usize], [f32; 3], f32)> = match c {
        Control::Elevator => vec![
            (&[0x16d5, 0x170f], [-44., -59., -6.], 1.),
            (&[0x1928, 0x19be], [44., -59., -6.], 1.),
        ],
        Control::Aileron => vec![
            (&[0x2e82, 0x2e95], [-78., -31., -1.], -1.),
            (&[0x2db3, 0x2dc6], [79., -31., -1.], 1.),
        ],
        Control::Flaps => vec![
            (&[0x2e82, 0x2e95], [-78., -31., -1.], -1.),
            (&[0x2db3, 0x2dc6], [79., -31., -1.], -1.),
        ],
        _ => Vec::new(),
    };
    if value != 0. {
        for (side, (ids, p, sign)) in signed.into_iter().enumerate() {
            let old: BTreeMap<_, _> = keyed(source)
                .into_iter()
                .filter(|(k, _)| ids.contains(&k.0))
                .collect();
            let new: BTreeMap<_, _> = keyed(pose)
                .into_iter()
                .filter(|(k, _)| ids.contains(&k.0))
                .collect();
            let qs = f4::witness_positions(&old, &new, p);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                let delta = (q[2] - p[2]) * scale;
                m.reviewed_direction_failures +=
                    usize::from(delta * sign * value.signum() as f32 <= EPSILON);
                m.reviewed_control_z_delta[side] = delta;
            }
        }
    }
    if matches!(c, Control::Rudder | Control::Elevator) {
        let ids = if matches!(c, Control::Rudder) {
            &FIN[..]
        } else {
            &TAIL[..]
        };
        let old: BTreeMap<_, _> = keyed(source)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        if matches!(c, Control::Rudder) {
            for (key, f) in &old {
                for (i, p) in f.positions.iter().enumerate() {
                    if (p[1] + 43. + 0.25 * (p[2] + 5.)).abs() < 1e-4 {
                        if let Some(q) = new.get(key).and_then(|f| f.positions.get(i)) {
                            m.max_reviewed_anchor_gap =
                                m.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                        } else {
                            m.reviewed_anchor_missing = true;
                        }
                    }
                }
            }
        }
        if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
        }
    }
    if matches!(c, Control::Gear) {
        wheels(reference, pose, value, scale, m);
    }
    if matches!(c, Control::Flaps) {
        slats(pose, value, scale, m);
        if value == 1. {
            flap_endpoint(raw, pose, scale, m);
        }
    }
}
fn flap_endpoint(raw: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (ids, p) in [
        ([0x2e82, 0x2e95], [-78., -31., -1.]),
        ([0x2db3, 0x2dc6], [79., -31., -1.]),
    ] {
        let old: BTreeMap<_, _> = keyed(raw)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let qs = f4::witness_positions(&old, &new, p);
        m.reviewed_anchor_missing |= qs.is_empty();
        m.reviewed_neutral_mismatch |= qs
            .into_iter()
            .any(|q| distance(q, [p[0], p[1], -5.]) * scale > EPSILON);
    }
}
fn slats(pose: &[Face], flap: f64, scale: f32, m: &mut Metrics) {
    let active: Vec<_> = pose
        .iter()
        .filter(|f| SLATS.contains(&f.address))
        .cloned()
        .collect();
    if flap == 0. {
        m.reviewed_anchor_missing |= !active.is_empty();
        return;
    }
    m.reviewed_anchor_missing |= active.len() != 4;
    for (ids, roots, down) in [
        (
            [0x2d29, 0x2d3c],
            [[-22., 27., -1.], [-78., -14., -1.]],
            [[-22., 31., -6.], [-78., -11., -6.]],
        ),
        (
            [0x2ce4, 0x2cf7],
            [[22., 27., -1.], [79., -14., -1.]],
            [[23., 31., -6.], [79., -11., -6.]],
        ),
    ] {
        let fs: Vec<_> = active.iter().filter(|f| ids.contains(&f.address)).collect();
        for p in roots {
            for f in &fs {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(
                    f.positions
                        .iter()
                        .map(|q| distance(p, *q) * scale)
                        .fold(f32::INFINITY, f32::min),
                );
            }
        }
        if flap == 1. {
            for p in down {
                for f in &fs {
                    m.reviewed_neutral_mismatch |= f
                        .positions
                        .iter()
                        .all(|q| distance(p, *q) * scale > EPSILON);
                }
            }
        }
        if fs.len() == 2 {
            for p in &fs[0].positions {
                m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(
                    fs[1]
                        .positions
                        .iter()
                        .map(|q| distance(*p, *q) * scale)
                        .fold(f32::INFINITY, f32::min),
                );
            }
        }
    }
}
type V = [f64; 3];
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: V, b: V) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(v: V) -> Option<V> {
    let length = dot(v, v).sqrt();
    (length > 1e-8).then(|| v.map(|x| x / length))
}
fn frame(face: &Face, i: usize, j: usize) -> Option<[V; 3]> {
    let origin = face.positions.first()?.map(f64::from);
    let x = unit(sub(face.positions.get(i)?.map(f64::from), origin))?;
    let z = unit(cross(x, sub(face.positions.get(j)?.map(f64::from), origin)))?;
    Some([x, cross(z, x), z])
}
/// Infer the observed rigid orientation from independent point correspondences.
/// This does not call any animation transform or assume its rotation angle.
fn observed_frames(before: &Face, after: &Face) -> Option<([V; 3], [V; 3])> {
    for i in 1..before.positions.len() {
        for j in i + 1..before.positions.len() {
            if let Some(a) = frame(before, i, j) {
                return Some((a, frame(after, i, j)?));
            }
        }
    }
    None
}
fn map_vector(v: V, before: [V; 3], after: [V; 3]) -> V {
    std::array::from_fn(|i| {
        (0..3)
            .map(|axis| dot(v, before[axis]) * after[axis][i])
            .sum()
    })
}
fn virtual_point(before: &Face, after: &Face, p: [f32; 3]) -> Option<[f32; 3]> {
    let (a, b) = observed_frames(before, after)?;
    let offset = map_vector(
        sub(p.map(f64::from), before.positions[0].map(f64::from)),
        a,
        b,
    );
    Some(std::array::from_fn(|i| {
        (after.positions[0][i] as f64 + offset[i]) as f32
    }))
}
fn rigid(before: &Face, after: &Face, scale: f32, m: &mut Metrics) {
    if before.positions.len() != after.positions.len() {
        m.reviewed_anchor_missing = true;
        return;
    }
    for i in 0..before.positions.len() {
        for j in i + 1..before.positions.len() {
            m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(
                (distance(before.positions[i], before.positions[j])
                    - distance(after.positions[i], after.positions[j]))
                .abs()
                    * scale,
            );
        }
    }
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, m: &mut Metrics) {
    let old = keyed(reference);
    let new = keyed(pose);
    let mut count = 0;
    for (key, before) in &old {
        if !GEAR.contains(&key.0) {
            continue;
        }
        let Some(after) = new.get(key) else {
            m.reviewed_wheel_failed = true;
            continue;
        };
        if [0x2c2c, 0x2c47].contains(&key.0) {
            let points: Vec<_> = before
                .positions
                .iter()
                .enumerate()
                .filter(|(_, p)| p[1] == 57.)
                .map(|(i, _)| i)
                .collect();
            if points.len() != 2 {
                m.reviewed_anchor_missing = true;
            } else {
                m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(
                    (distance(after.positions[points[0]], after.positions[points[1]]) - 3.).abs()
                        * scale,
                );
            }
            continue;
        }
        count += 1;
        rigid(before, after, scale, m);
        let nose = key.0 >= 0x2c8f;
        let left = [0x2bd7, 0x2bf2].contains(&key.0);
        let hub = if nose {
            [0., 60., -3.]
        } else {
            [if left { -23. } else { 23. }, -9., 0.]
        };
        if let Some(p) = virtual_point(before, after, hub) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, hub) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
        if !nose {
            m.reviewed_wheel_failed |= after.positions.iter().any(|p| (p[0] - hub[0]).abs() > 1e-4);
        }
        if travel < 1e-5 {
            let (min, max) = if nose {
                ([0., 43., -7.], [0., 60., 6.])
            } else {
                ([hub[0], -32., -4.], [hub[0], -9., 4.])
            };
            m.reviewed_wheel_failed |= after
                .positions
                .iter()
                .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-3 || p[i] > max[i] + 1e-3));
        }
    }
    m.reviewed_wheel_failed |= count != 6 || m.reviewed_rigid_panel_error > EPSILON;
}
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut rows =
        String::from("flaps,roll,anchor_gap_ft,skin_gap_ft,crossings,direction_failures\n");
    let mut poses = Vec::new();
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut s = neutral.clone();
            s.flaps = flap;
            s.aileron = roll;
            let pose = airframe.animation_faces(&s);
            let mut m = measure(&original, &pose, scale);
            anchors(Control::Flaps, &original, &pose, scale, &mut m);
            slats(&pose, flap, scale, &mut m);
            if flap == 1. && roll == 0. {
                flap_endpoint(&original, &pose, scale, &mut m);
            }
            let mut base = neutral.clone();
            base.flaps = flap;
            let reference = airframe.animation_faces(&base);
            let old = keyed(&reference);
            let new = keyed(&pose);
            for (address, x, sign) in [(0x2e82, -78., -1.), (0x2db3, 79., 1.)] {
                let key = (address, 0);
                if let (Some(before), Some(after)) = (old.get(&key), new.get(&key)) {
                    let i = before
                        .positions
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p[0] == x)
                        .min_by(|(_, a), (_, b)| a[1].total_cmp(&b[1]))
                        .map(|(i, _)| i);
                    if let Some(i) = i {
                        if roll != 0. {
                            m.reviewed_direction_failures += usize::from(
                                (after.positions[i][2] - before.positions[i][2])
                                    * sign
                                    * roll.signum() as f32
                                    <= EPSILON,
                            );
                        }
                        m.reviewed_direction_failures += usize::from(
                            after.positions[i][2] < -9. - 1e-4 || after.positions[i][2] > 3. + 1e-4,
                        );
                    } else {
                        m.reviewed_anchor_missing = true;
                    }
                } else {
                    m.reviewed_anchor_missing = true;
                }
            }
            writeln!(
                rows,
                "{flap},{roll},{},{},{},{}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.max_reviewed_skin_gap > EPSILON
                || !m.new_planar_crossings.is_empty()
                || m.reviewed_direction_failures != 0
                || m.reviewed_neutral_mismatch
            {
                failures.push(format!("Su27 flap{flap}/roll{roll} attachment, source endpoint, combined bound or direction"));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("flaperon-combinations.csv"), rows)?;
    contact_sheet(&out.join("flaperon-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
