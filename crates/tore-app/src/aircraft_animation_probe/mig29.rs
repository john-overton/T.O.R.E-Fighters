//! Independent MiG29 source endpoints, fitted cut attachments and rigid gear bounds.
use super::*;
const TAIL: [usize; 8] = [
    0x474f, 0x4776, 0x479e, 0x47c5, 0x4cf6, 0x4dbc, 0x4d1d, 0x4de3,
];
const FIN: [usize; 4] = [0x334e, 0x3371, 0x4d6c, 0x4d93];
const GEAR: [usize; 16] = [
    0x50e2, 0x5101, 0x5120, 0x513f, 0x4ecc, 0x4eeb, 0x4f0a, 0x4f29, 0x4f48, 0x4f67, 0x4fe3, 0x5002,
    0x5021, 0x5040, 0x505f, 0x507e,
];
const BRAKE: [usize; 4] = [0x53db, 0x53f2, 0x5478, 0x548f];
fn anchor_groups(c: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match c {
        Control::Flaps => vec![
            (
                &[0x5217, 0x523e],
                &[
                    [17., -6., 2.],
                    [17., -6., 0.],
                    [36., -8., 1.],
                    [36., -8., 0.],
                ],
            ),
            (
                &[0x531e, 0x5345],
                &[
                    [-17., -6., 2.],
                    [-17., -6., 0.],
                    [-36., -8., 1.],
                    [-36., -8., 0.],
                ],
            ),
        ],
        Control::Aileron => vec![
            (
                &[0x445c, 0x44fc],
                &[
                    [36., -8., 1.],
                    [36., -8., 0.],
                    [57., -11., 1.],
                    [57., -11., 0.],
                ],
            ),
            (
                &[0x4b23, 0x4c61],
                &[
                    [-36., -8., 1.],
                    [-36., -8., 0.],
                    [-57., -11., 1.],
                    [-57., -11., 0.],
                ],
            ),
        ],
        Control::Brake => vec![
            (&[0x53db, 0x53f2], &[[-4., -8., -1.], [4., -8., -1.]]),
            (&[0x5478, 0x548f], &[[-5., -7., 7.], [4., -7., 7.]]),
        ],
        _ => Vec::new(),
    }
}
pub(super) fn check(
    control: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    m: &mut Metrics,
) {
    let (raw, reference, pose) = geometry;
    if matches!(control, Control::Gear) && value == 0. {
        m.reviewed_wheel_failed = pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    if matches!(control, Control::Brake) && value == 0. {
        m.reviewed_wheel_failed = pose.iter().any(|f| BRAKE.contains(&f.address));
        return;
    }
    let source = if matches!(
        control,
        Control::Gear | Control::Brake | Control::Elevator | Control::Rudder
    ) {
        reference
    } else {
        raw
    };
    for (ids, points) in anchor_groups(control) {
        let old: BTreeMap<_, _> = keyed(source)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        for p in points {
            let q = f4::witness_positions(&old, &new, *p);
            m.reviewed_anchor_missing |= q.is_empty();
            for q in q {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(*p, q) * scale);
            }
        }
        if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
        }
    }
    let signed: Vec<(&[usize], [f32; 3], f32)> = match control {
        Control::Elevator => vec![
            (&[0x474f, 0x4776, 0x479e, 0x47c5], [35., -54., 0.], 1.),
            (&[0x4cf6, 0x4dbc, 0x4d1d, 0x4de3], [-36., -54., 0.], 1.),
        ],
        Control::Aileron => vec![
            (&[0x445c, 0x44fc], [57., -15., 0.], 1.),
            (&[0x4b23, 0x4c61], [-57., -15., 0.], -1.),
        ],
        Control::Flaps => vec![
            (&[0x5217, 0x523e], [17., -13., 0.], -1.),
            (&[0x531e, 0x5345], [-17., -13., 0.], -1.),
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
    if matches!(control, Control::Elevator | Control::Rudder) {
        let ids = if matches!(control, Control::Elevator) {
            &TAIL[..]
        } else {
            &FIN[..]
        };
        let old: BTreeMap<_, _> = keyed(source)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        for (key, f) in &old {
            for (i, p) in f.positions.iter().enumerate() {
                let d = if matches!(control, Control::Elevator) {
                    p[1] + 41.
                } else {
                    p[1] + 32. + 0.20 * (p[2] - 1.)
                };
                if d.abs() < 1e-4 {
                    if let Some(q) = new.get(key).and_then(|f| f.positions.get(i)) {
                        m.max_reviewed_anchor_gap =
                            m.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                    } else {
                        m.reviewed_anchor_missing = true;
                    }
                }
            }
        }
        if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
        }
    }
    if matches!(control, Control::Flaps) {
        for (a, ids) in [(0x51d7, [0x5217, 0x523e]), (0x52de, [0x531e, 0x5345])] {
            let cap = pose.iter().find(|f| f.address == a);
            if value == 0. {
                m.reviewed_anchor_missing |= cap.is_some();
            } else if let Some(cap) = cap {
                for p in &cap.positions {
                    m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(
                        pose.iter()
                            .filter(|f| ids.contains(&f.address))
                            .flat_map(|f| &f.positions)
                            .map(|q| distance(*p, *q) * scale)
                            .fold(f32::INFINITY, f32::min),
                    );
                }
            } else {
                m.reviewed_anchor_missing = true;
            }
        }
        if value == 1. {
            for (ids, p, q) in [
                ([0x5217, 0x523e], [17., -13., 0.], [17., -11., -5.]),
                ([0x531e, 0x5345], [-17., -13., 0.], [-17., -11., -5.]),
            ] {
                let old: BTreeMap<_, _> = keyed(raw)
                    .into_iter()
                    .filter(|(k, _)| ids.contains(&k.0))
                    .collect();
                let new: BTreeMap<_, _> = keyed(pose)
                    .into_iter()
                    .filter(|(k, _)| ids.contains(&k.0))
                    .collect();
                let ps = f4::witness_positions(&old, &new, p);
                m.reviewed_anchor_missing |= ps.is_empty();
                m.reviewed_neutral_mismatch |=
                    ps.into_iter().any(|v| distance(v, q) * scale > EPSILON);
            }
        }
    }
    if matches!(control, Control::Gear) {
        wheels(reference, pose, value, scale, m);
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
        count += 1;
        let Some(after) = new.get(key) else {
            m.reviewed_wheel_failed = true;
            continue;
        };
        rigid(before, after, scale, m);
        let nose = key.0 >= 0x50e2;
        let left = key.0 >= 0x4fe3 && key.0 < 0x50e2;
        let hub = if nose {
            [0., 47., -2.]
        } else {
            [if left { -17. } else { 17. }, 1., 0.]
        };
        if let Some(p) = virtual_point(before, after, hub) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, hub) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
        if !nose {
            m.reviewed_wheel_failed |= after
                .positions
                .iter()
                .any(|p| p[0] * hub[0].signum() < 13. - 1e-4);
        }
        if travel < 1e-5 {
            let (min, max) = if nose {
                ([-3., 29., -14.], [3., 47., 3.])
            } else if left {
                ([-21., -19., -6.], [-13., 1., 5.])
            } else {
                ([13., -19., -6.], [21., 1., 5.])
            };
            m.reviewed_wheel_failed |= after
                .positions
                .iter()
                .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-3 || p[i] > max[i] + 1e-3));
        }
    }
    m.reviewed_wheel_failed |= count != 16 || m.reviewed_rigid_panel_error > EPSILON;
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
            check(
                Control::Flaps,
                flap,
                (&original, &original, &pose),
                scale,
                &mut m,
            );
            check(
                Control::Aileron,
                roll,
                (&original, &original, &pose),
                scale,
                &mut m,
            );
            writeln!(
                rows,
                "{flap},{roll},{},{},{},{}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !m.finite
                || m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.max_reviewed_skin_gap > EPSILON
                || !m.new_planar_crossings.is_empty()
                || m.reviewed_direction_failures != 0
                || m.reviewed_neutral_mismatch
            {
                failures.push(format!(
                    "MiG29 flap{flap}/roll{roll} attachment, source endpoint or direction"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
