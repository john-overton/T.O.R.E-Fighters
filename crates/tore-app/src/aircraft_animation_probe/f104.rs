//! Independent F104 source roots, signed controls and authored stow witnesses.
//! Measures the shared Airframe output, never repeats animation algorithms.
use super::*;
const FLAPS: [usize; 4] = [0x5850, 0x5879, 0x56f2, 0x571b];
const LEFT_WHEEL: [usize; 2] = [0x5dec, 0x5e59];
const RIGHT_WHEEL: [usize; 2] = [0x5bf2, 0x5c5f];
const NOSE: [usize; 4] = [0x5f8a, 0x5faa, 0x5fd6, 0x5ff6];
const MAIN: [usize; 20] = [
    0x5bc6, 0x5bf2, 0x5c1e, 0x5c3e, 0x5c5f, 0x5c80, 0x5cac, 0x5ccc, 0x5cfc, 0x5d1c, 0x5dc0, 0x5dec,
    0x5e18, 0x5e38, 0x5e59, 0x5e7a, 0x5eaa, 0x5eca, 0x5ef6, 0x5f16,
];
fn roots(control: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match control {
        Control::Rudder => vec![(&[0x5646, 0x566e], &[[0., -53., 9.], [0., -55., 21.]])],
        Control::Elevator => vec![(
            &[0x2f9e, 0x2fbb, 0x1f4d, 0x1f6b],
            &[[0., -49., 21.], [0., -73., 21.]],
        )],
        Control::Flaps | Control::Aileron => vec![
            (&[0x5850], &[[-30., -15., -4.], [-9., -15., -2.]]),
            (&[0x5879], &[[-30., -15., -3.], [-9., -15., -1.]]),
            (&[0x56f2], &[[30., -15., -4.], [9., -15., -2.]]),
            (&[0x571b], &[[30., -15., -3.], [9., -15., -1.]]),
        ],
        Control::Hook => vec![(&[0x6134, 0x6154], &[[0., -32., -6.], [0., -35., -6.]])],
        Control::Brake => vec![
            (&[0x5a05, 0x5a25], &[[0., 24., 7.], [2., 24., 7.]]),
            (&[0x5a59, 0x5a79], &[[-2., 24., 7.], [-4., 24., 6.]]),
            (&[0x5aad, 0x5b22], &[[0., 24., 7.], [-2., 24., 7.]]),
            (&[0x5ae1, 0x5b01], &[[2., 24., 7.], [4., 24., 6.]]),
        ],
        Control::Gear => vec![
            (&[0x5c1e, 0x5c3e], &[[3., -21., -5.], [5., -21., -5.]]),
            (&[0x5e18, 0x5e38], &[[-3., -21., -5.], [-5., -21., -5.]]),
            (
                &[0x5cac, 0x5ccc],
                &[[8., -21., -3.], [14., -21., -3.], [5., -21., -5.]],
            ),
            (
                &[0x5ef6, 0x5f16],
                &[[-8., -21., -3.], [-14., -21., -3.], [-5., -21., -5.]],
            ),
            (&[0x5cfc, 0x5d1c], &[[3., -22., -5.], [3., -20., -5.]]),
            (&[0x5eaa, 0x5eca], &[[-3., -22., -5.], [-3., -20., -5.]]),
            (&[0x5f8a, 0x5faa], &[[0., 35., -6.], [0., 40., -6.]]),
            (&[0x5fd6, 0x5ff6], &[[-2., 38., -6.], [2., 38., -6.]]),
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
    if value == 0. && matches!(control, Control::Gear | Control::Hook | Control::Brake) {
        return;
    }
    let (raw, reference, pose) = geometry;
    let source = if matches!(control, Control::Gear | Control::Hook | Control::Brake) {
        reference
    } else {
        raw
    };
    for (addresses, points) in roots(control) {
        let before: Vec<_> = source
            .iter()
            .filter(|f| addresses.contains(&f.address))
            .cloned()
            .collect();
        let after: Vec<_> = pose
            .iter()
            .filter(|f| addresses.contains(&f.address))
            .cloned()
            .collect();
        let old = keyed(&before);
        let new = keyed(&after);
        metric.reviewed_anchor_missing |= old.iter().any(|(key, f)| {
            new.get(key)
                .is_none_or(|g| g.positions.len() != f.positions.len())
        });
        for p in points {
            let moved = f4::witness_positions(&old, &new, *p);
            metric.reviewed_anchor_missing |= moved.is_empty();
            for q in moved {
                metric.max_reviewed_anchor_gap =
                    metric.max_reviewed_anchor_gap.max(distance(*p, q) * scale);
            }
        }
        if let Some(gap) = shared_vertex_gaps(&old, &new, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if value == 0. {
            metric.reviewed_neutral_mismatch |= before.iter().any(|f| {
                !after
                    .iter()
                    .any(|g| g.address == f.address && g.positions == f.positions)
            });
        }
    }
    if matches!(control, Control::Flaps | Control::Aileron) {
        let selected: Vec<_> = raw
            .iter()
            .filter(|f| FLAPS.contains(&f.address))
            .cloned()
            .collect();
        let actual: Vec<_> = pose
            .iter()
            .filter(|f| FLAPS.contains(&f.address))
            .cloned()
            .collect();
        if let Some(gap) = shared_vertex_gaps(&keyed(&selected), &keyed(&actual), scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if let Some(closure) = pose.iter().find(|f| f.address == 0x5920) {
            metric.reviewed_anchor_missing |= !closure.positions.contains(&[-30., -15., -4.])
                || !closure.positions.contains(&[-30., -15., -3.]);
            let moved =
                f4::witness_positions(&keyed(&selected), &keyed(&actual), [-30., -19., -4.]);
            if let Some(p) = closure.positions.first() {
                for q in moved {
                    metric.max_reviewed_skin_gap =
                        metric.max_reviewed_skin_gap.max(distance(*p, q) * scale);
                }
            } else {
                metric.reviewed_anchor_missing = true;
            }
        }
    }
    if value != 0. {
        let witnesses: Vec<(&[usize], [f32; 3], usize, f32)> = match control {
            Control::Rudder => vec![(&[0x5646, 0x566e], [0., -68., 9.], 0, 1.)],
            Control::Elevator => vec![
                (&[0x2f9e, 0x2fbb], [-22., -64., 21.], 2, 1.),
                (&[0x1f4d, 0x1f6b], [22., -64., 21.], 2, 1.),
                (&[0x2f9e, 0x2fbb], [-22., -57., 21.], 2, -1.),
                (&[0x1f4d, 0x1f6b], [22., -57., 21.], 2, -1.),
            ],
            Control::Flaps => vec![
                (&FLAPS, [-8., -23., -1.], 2, -1.),
                (&FLAPS, [8., -23., -1.], 2, -1.),
            ],
            Control::Aileron => vec![
                (&FLAPS, [-8., -23., -1.], 2, -1.),
                (&FLAPS, [8., -23., -1.], 2, 1.),
            ],
            _ => Vec::new(),
        };
        for (side, (addresses, point, axis, sign)) in witnesses.into_iter().enumerate() {
            let old: Vec<_> = source
                .iter()
                .filter(|f| addresses.contains(&f.address))
                .cloned()
                .collect();
            let new: Vec<_> = pose
                .iter()
                .filter(|f| addresses.contains(&f.address))
                .cloned()
                .collect();
            let moved = f4::witness_positions(&keyed(&old), &keyed(&new), point);
            metric.reviewed_direction_failures += usize::from(moved.is_empty());
            for p in moved {
                let delta = (p[axis] - point[axis]) * scale;
                metric.reviewed_direction_failures +=
                    usize::from(delta * sign * value.signum() as f32 <= EPSILON);
                if axis == 2 && side < 2 {
                    metric.reviewed_control_z_delta[side] = delta;
                }
            }
        }
    }
    if matches!(control, Control::Gear) {
        wheels(reference, pose, value, scale, metric);
    }
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    wheel_separation(
        reference,
        pose,
        scale,
        travel,
        metric,
        (&RIGHT_WHEEL, &LEFT_WHEEL),
        0.70,
    );
    let current = keyed(pose);
    let mut count = 0;
    for (key, source) in keyed(reference) {
        let main = LEFT_WHEEL.contains(&key.0) || RIGHT_WHEEL.contains(&key.0);
        let nose = NOSE.contains(&key.0) && source.positions.iter().all(|p| p[2] <= -9. + 1e-4);
        if !main && !nose {
            continue;
        }
        count += 1;
        let Some(actual) = current.get(&key) else {
            metric.reviewed_wheel_failed = true;
            continue;
        };
        if source.positions.len() != actual.positions.len() {
            metric.reviewed_wheel_failed = true;
            continue;
        }
        for i in 0..source.positions.len() {
            for j in i + 1..source.positions.len() {
                metric.reviewed_wheel_rigidity_error = metric.reviewed_wheel_rigidity_error.max(
                    (distance(source.positions[i], source.positions[j])
                        - distance(actual.positions[i], actual.positions[j]))
                    .abs()
                        * scale,
                );
            }
        }
        if travel < 1e-5 {
            let (min, max) = if RIGHT_WHEEL.contains(&key.0) {
                ([8.5, -24., -2.], [14.5, -18., -2.])
            } else if LEFT_WHEEL.contains(&key.0) {
                ([-14.5, -24., -2.], [-8.5, -19., -2.])
            } else {
                ([-2., 28., -5.], [2., 33., -1.])
            };
            metric.reviewed_wheel_failed |= actual
                .positions
                .iter()
                .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-3 || p[i] > max[i] + 1e-3));
        }
    }
    metric.reviewed_wheel_failed |= count != 8 || metric.reviewed_wheel_rigidity_error > EPSILON;
    // The source main wheel circles are distinct from the lower leg/bridge sheets.
    // All visible main geometry must stay on its own side, even if circles do.
    for face in pose.iter().filter(|f| MAIN.contains(&f.address)) {
        let side = if face.address >= 0x5dc0 { -1. } else { 1. };
        metric.reviewed_wheel_failed |= face.positions.iter().any(|p| p[0] * side * scale < 0.35);
    }
}
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut rows = String::from("flap,roll,finite,anchor_gap_ft,skin_gap_ft,direction_failures\n");
    let mut failures = Vec::new();
    let mut poses = Vec::new();
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        let mut state = neutral.clone();
        state.flaps = flap;
        let base = airframe.animation_faces(&state);
        let base_map = keyed(&base);
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            state.aileron = roll;
            let actual = airframe.animation_faces(&state);
            let after = keyed(&actual);
            let mut metric = measure(&original, &actual, scale);
            check(
                Control::Flaps,
                flap,
                (&original, &original, &actual),
                scale,
                &mut metric,
            );
            metric.reviewed_direction_failures = 0;
            if roll != 0. {
                for (point, sign) in [([-8., -23., -1.], -1.), ([8., -23., -1.], 1.)] {
                    for (key, face) in keyed(&original)
                        .into_iter()
                        .filter(|(key, _)| FLAPS.contains(&key.0))
                    {
                        for (i, _) in face
                            .positions
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| **p == point)
                        {
                            let delta = after
                                .get(&key)
                                .and_then(|f| f.positions.get(i))
                                .zip(base_map.get(&key).and_then(|f| f.positions.get(i)))
                                .map(|(a, b)| a[2] - b[2]);
                            if delta.is_none_or(|d| d * sign * roll as f32 <= EPSILON) {
                                metric.reviewed_direction_failures += 1;
                            }
                        }
                    }
                }
            }
            writeln!(
                rows,
                "{flap},{roll},{},{},{},{}",
                metric.finite,
                metric.max_reviewed_anchor_gap,
                metric.max_reviewed_skin_gap,
                metric.reviewed_direction_failures
            )?;
            if !metric.finite
                || metric.reviewed_anchor_missing
                || metric.max_reviewed_anchor_gap > EPSILON
                || metric.max_reviewed_skin_gap > EPSILON
                || metric.reviewed_direction_failures > 0
            {
                failures.push(format!(
                    "combined flap{flap}/roll{roll} attachment,skin or direction witness"
                ));
            }
            poses.push(actual);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
