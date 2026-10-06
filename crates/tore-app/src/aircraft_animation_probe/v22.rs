//! Independent V22 source controls, fitted attachments and shared conversion path.
use super::*;
const GEAR: [usize; 12] = [
    0x2a7e, 0x2aa6, 0x2ac2, 0x2adf, 0x2b4d, 0x2b69, 0x2b91, 0x2bad, 0x2c1c, 0x2c44, 0x2c60, 0x2c7d,
];
fn attachments(control: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match control {
        Control::Flaps | Control::Aileron => vec![
            (
                &[0x39de, 0x3a03, 0x2198],
                &[
                    [-20., -4., 6.],
                    [-95., 5., 6.],
                    [-20., -4., 15.],
                    [-95., 5., 15.],
                ],
            ),
            (
                &[0x3895, 0x38ba, 0x29cf],
                &[
                    [19., -4., 6.],
                    [94., 5., 6.],
                    [19., -4., 15.],
                    [94., 5., 15.],
                ],
            ),
        ],
        _ => Vec::new(),
    }
}
pub(super) fn check(
    control: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    metric: &mut Metrics,
) {
    let (raw, reference, pose) = geometry;
    if matches!(control, Control::Gear) && value == 0. {
        metric.reviewed_wheel_failed = pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    let source = if matches!(control, Control::Gear | Control::Elevator | Control::Rudder) {
        reference
    } else {
        raw
    };
    for (ids, points) in attachments(control) {
        let a: Vec<_> = source
            .iter()
            .filter(|f| ids.contains(&f.address))
            .cloned()
            .collect();
        let b: Vec<_> = pose
            .iter()
            .filter(|f| ids.contains(&f.address))
            .cloned()
            .collect();
        let old = keyed(&a);
        let new = keyed(&b);
        for p in points {
            let moved = f4::witness_positions(&old, &new, *p);
            metric.reviewed_anchor_missing |= moved.is_empty();
            for q in moved {
                metric.max_reviewed_anchor_gap =
                    metric.max_reviewed_anchor_gap.max(distance(*p, q) * scale);
            }
        }
        if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(g.gap);
        }
    }
    let signed: Vec<(&[usize], [f32; 3], f32)> = match control {
        Control::Elevator => vec![
            (&[0x23dc, 0x2463], [-36., -139., 1.], 1.),
            (&[0x2691, 0x2718], [36., -139., 1.], 1.),
        ],
        Control::Aileron => vec![
            (&[0x39de, 0x3a03], [-95., -6., 11.], -1.),
            (&[0x3895, 0x38ba], [94., -6., 11.], 1.),
        ],
        Control::Flaps => vec![
            (&[0x39de, 0x3a03], [-95., -6., 11.], -1.),
            (&[0x3895, 0x38ba], [94., -6., 11.], -1.),
        ],
        _ => Vec::new(),
    };
    if value != 0. {
        for (side, (ids, p, sign)) in signed.into_iter().enumerate() {
            let old: Vec<_> = source
                .iter()
                .filter(|f| ids.contains(&f.address))
                .cloned()
                .collect();
            let new: Vec<_> = pose
                .iter()
                .filter(|f| ids.contains(&f.address))
                .cloned()
                .collect();
            let moved = f4::witness_positions(&keyed(&old), &keyed(&new), p);
            metric.reviewed_direction_failures += usize::from(moved.is_empty());
            for q in moved {
                let delta = (q[2] - p[2]) * scale;
                metric.reviewed_direction_failures +=
                    usize::from(delta * sign * value.signum() as f32 <= EPSILON);
                if side < 2 {
                    metric.reviewed_control_z_delta[side] = delta;
                }
            }
        }
    }
    if matches!(control, Control::Rudder) {
        let old: Vec<_> = source
            .iter()
            .filter(|f| [0x248b, 0x24c1, 0x27c3, 0x2813].contains(&f.address))
            .cloned()
            .collect();
        let new: Vec<_> = pose
            .iter()
            .filter(|f| [0x248b, 0x24c1, 0x27c3, 0x2813].contains(&f.address))
            .cloned()
            .collect();
        let before = keyed(&old);
        let after = keyed(&new);
        for (key, face) in &before {
            for (i, p) in face.positions.iter().enumerate() {
                let d = p[1] + 128. + (4. / 29.) * (p[2] - 1.);
                if d.abs() <= 1e-4 {
                    if let Some(q) = after.get(key).and_then(|f| f.positions.get(i)) {
                        metric.max_reviewed_anchor_gap =
                            metric.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                    } else {
                        metric.reviewed_anchor_missing = true;
                    }
                }
            }
        }
        if let Some(g) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(g.gap);
        }
    }
    if matches!(control, Control::Conversion | Control::Rotor) {
        nacelles(reference, pose, scale, metric);
    }
    if matches!(control, Control::Elevator) {
        let old = keyed(reference);
        let new = keyed(pose);
        for (key, f) in &old {
            if [0x23dc, 0x2463, 0x2691, 0x2718].contains(&key.0) {
                for (i, p) in f.positions.iter().enumerate() {
                    if (p[1] + 123.).abs() < 1e-4 {
                        if let Some(q) = new.get(key).and_then(|f| f.positions.get(i)) {
                            metric.max_reviewed_anchor_gap =
                                metric.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                        } else {
                            metric.reviewed_anchor_missing = true;
                        }
                    }
                }
            }
        }
    }
    if matches!(control, Control::Gear) {
        wheels(reference, pose, value, scale, metric);
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
        let hub = if key.0 < 0x2b00 {
            [0., 82., -28.]
        } else if key.0 < 0x2c00 {
            [23., -4., -29.]
        } else {
            [-23., -4., -29.]
        };
        if let Some(p) = virtual_point(before, after, hub) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, hub) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
        if hub[0] != 0. {
            m.reviewed_wheel_failed |= after.positions.iter().any(|p| p[0] * hub[0].signum() < 12.);
        }
        if travel < 1e-5 {
            let (min, max) = if hub[0] == 0. {
                ([-2., 71., -32.], [2., 82., -25.])
            } else if hub[0] > 0. {
                ([19., -8., -29.], [27., 0., -19.])
            } else {
                ([-27., -8., -29.], [-19., 0., -19.])
            };
            m.reviewed_wheel_failed |= after
                .positions
                .iter()
                .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-3 || p[i] > max[i] + 1e-3));
        }
    }
    m.reviewed_wheel_failed |= count != 12 || m.reviewed_rigid_panel_error > EPSILON;
}
const NACELLE_L: [usize; 18] = [
    0x33f5, 0x3412, 0x342f, 0x344c, 0x3469, 0x348b, 0x34a9, 0x34cb, 0x34e9, 0x350b, 0x35f7, 0x3614,
    0x3631, 0x364e, 0x372c, 0x3746, 0x3760, 0x377a,
];
const NACELLE_R: [usize; 18] = [
    0x2e75, 0x2e92, 0x2eaf, 0x2ecc, 0x2ee9, 0x2f0b, 0x2f29, 0x2f4b, 0x2f69, 0x2f8b, 0x3077, 0x3094,
    0x30b1, 0x30ce, 0x31ac, 0x31c6, 0x31e0, 0x31fa,
];
fn nacelles(reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = keyed(reference);
    let new = keyed(pose);
    for (ids, blades, x) in [
        (&NACELLE_L, [0x3540, 0x3675], -102.),
        (&NACELLE_R, [0x2fc0, 0x30f5], 102.),
    ] {
        let mut seen = 0;
        let mut hub = None;
        let mut forward = None;
        for (key, before) in old.iter().filter(|(key, _)| ids.contains(&key.0)) {
            seen += 1;
            let Some(after) = new.get(key) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            rigid(before, after, scale, m);
            let pivot = [x, 0., 12.];
            if let Some(p) = virtual_point(before, after, pivot) {
                m.max_reviewed_anchor_gap =
                    m.max_reviewed_anchor_gap.max(distance(p, pivot) * scale);
            } else {
                m.reviewed_anchor_missing = true;
            }
            if hub.is_none() {
                hub = virtual_point(before, after, [x, 51., 16.]);
                forward =
                    observed_frames(before, after).map(|(a, b)| map_vector([0., 1., 0.], a, b));
            }
        }
        m.reviewed_anchor_missing |= seen != 18;
        for a in blades {
            let (Some(before), Some(after)) = (old.get(&(a, 0)), new.get(&(a, 0))) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            rigid(before, after, scale, m);
            if let (Some(p), Some(h)) = (virtual_point(before, after, [x, 51., 16.]), hub) {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, h) * scale);
            } else {
                m.reviewed_anchor_missing = true;
            }
            if let (Some((a, b)), Some(forward)) = (observed_frames(before, after), forward) {
                let actual = map_vector([0., 1., 0.], a, b);
                m.reviewed_direction_failures +=
                    usize::from((1. - dot(actual, forward)).abs() > 1e-5);
            } else {
                m.reviewed_anchor_missing = true;
            }
        }
        let a: BTreeMap<_, _> = old
            .iter()
            .filter(|(key, _)| ids.contains(&key.0))
            .map(|(k, v)| (*k, *v))
            .collect();
        let b: BTreeMap<_, _> = new
            .iter()
            .filter(|(key, _)| ids.contains(&key.0))
            .map(|(k, v)| (*k, *v))
            .collect();
        if let Some(g) = shared_vertex_gaps(&a, &b, scale).first() {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
        }
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
    for kind in ["flaperon", "conversion-rotor"] {
        let mut rows = String::from(
            "first,second,anchor_gap_ft,skin_gap_ft,rigid_error_ft,crossings,direction_failures\n",
        );
        let mut poses = Vec::new();
        for first in [0., 0.25, 0.5, 0.75, 1.] {
            for second in [-1., -0.5, 0., 0.5, 1.] {
                let mut state = neutral.clone();
                if kind == "flaperon" {
                    state.flaps = first;
                    state.aileron = second;
                } else {
                    state.lift_controls.conversion_actual = first;
                    state.engine = true;
                    state.throttle = 0.5;
                    state.ticks = ((second + 1.) * 12.) as u64;
                }
                let pose = airframe.animation_faces(&state);
                let mut m = measure(&original, &pose, scale);
                if kind == "flaperon" {
                    check(
                        Control::Flaps,
                        0.,
                        (&original, &original, &pose),
                        scale,
                        &mut m,
                    );
                    let mut base = neutral.clone();
                    base.flaps = first;
                    let reference = airframe.animation_faces(&base);
                    let old = keyed(&reference);
                    let new = keyed(&pose);
                    for (address, x, sign) in [(0x39de, -95., -1.), (0x38ba, 94., 1.)] {
                        let key = (address, 0);
                        if let (Some(before), Some(after)) = (old.get(&key), new.get(&key)) {
                            let index = before
                                .positions
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| p[0] == x)
                                .min_by(|(_, a), (_, b)| a[1].total_cmp(&b[1]))
                                .map(|(i, _)| i);
                            if let Some(i) = index {
                                if second != 0. {
                                    m.reviewed_direction_failures += usize::from(
                                        (after.positions[i][2] - before.positions[i][2])
                                            * sign
                                            * second.signum() as f32
                                            <= EPSILON,
                                    );
                                }
                                if first == 1. && second == 0. {
                                    let expected = [x, -4., 3.];
                                    m.reviewed_neutral_mismatch |=
                                        distance(after.positions[i], expected) * scale > EPSILON;
                                }
                            } else {
                                m.reviewed_anchor_missing = true;
                            }
                        } else {
                            m.reviewed_anchor_missing = true;
                        }
                    }
                } else {
                    nacelles(&original, &pose, scale, &mut m);
                }
                writeln!(
                    rows,
                    "{first},{second},{},{},{},{},{}",
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
                if !m.finite
                    || m.reviewed_anchor_missing
                    || m.max_reviewed_anchor_gap > EPSILON
                    || m.max_reviewed_skin_gap > EPSILON
                    || m.reviewed_rigid_panel_error > EPSILON
                    || !m.new_planar_crossings.is_empty()
                    || m.reviewed_direction_failures != 0
                    || m.reviewed_neutral_mismatch
                {
                    failures.push(format!("V22 {kind} {first}/{second} attachment, signed motion, rigid card or topology"));
                }
                poses.push(pose);
            }
        }
        fs::write(out.join(format!("{kind}-combinations.csv")), rows)?;
        contact_sheet(
            &out.join(format!("{kind}-combinations.ppm")),
            &original,
            &poses,
        )?;
    }
    Ok(failures)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn virtual_shaft_measurement_rejects_rigid_translation() {
        let source = super::super::tests::face(
            0x2b91,
            vec![
                [19., -4., -39.],
                [27., -4., -39.],
                [27., -4., -29.],
                [19., -4., -29.],
            ],
        );
        let mut result = source.clone();
        for p in &mut result.positions {
            p[0] = 46. - p[0];
            p[2] = -58. - p[2];
        }
        let hub = [23., -4., -29.];
        assert!(distance(virtual_point(&source, &result, hub).unwrap(), hub) < EPSILON);
        for p in &mut result.positions {
            p[0] += 2.;
        }
        assert!(distance(virtual_point(&source, &result, hub).unwrap(), hub) > 1.9);
    }
    #[test]
    fn missing_nacelle_or_gear_card_cannot_pass_the_review() {
        let mut m = Metrics::default();
        nacelles(&[], &[], 1., &mut m);
        assert!(m.reviewed_anchor_missing);
        let mut m = Metrics::default();
        wheels(&[], &[], 0.5, 1., &mut m);
        assert!(m.reviewed_wheel_failed);
    }
}
