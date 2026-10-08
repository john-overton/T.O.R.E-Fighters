//! Independent F16C source attachment and authored-control witnesses.
//! Reads the shared drawing output; contains no animation transforms.
use super::*;
const FLAPS: [usize; 8] = [
    0x60d3, 0x60fa, 0x6121, 0x6142, 0x5f45, 0x5f6c, 0x5f93, 0x5fb4,
];
const LEFT_WHEELS: [usize; 4] = [0x5a30, 0x5a5d, 0x5a8a, 0x5ab1];
const RIGHT_WHEELS: [usize; 4] = [0x5c30, 0x5c5d, 0x5c8a, 0x5cb1];
const NOSE_WHEELS: [usize; 4] = [0x5e92, 0x5eb1, 0x5ed0, 0x5eef];

fn roots(control: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match control {
        Control::Rudder => vec![(&[0x6323, 0x634a], &[[0., -40., 11.], [0., -54., 37.]])],
        Control::Elevator => vec![
            (&[0x5606, 0x564b], &[[-10., -32., 1.], [-10., -59., 1.]]),
            (&[0x52cd, 0x52e6], &[[10., -32., 1.], [10., -59., 1.]]),
        ],
        Control::Flaps | Control::Aileron => vec![
            (&[0x60d3], &[[-40., -15., 0.], [-14., -12., 0.]]),
            (&[0x60fa], &[[-40., -15., 2.], [-14., -12., 2.]]),
            (&[0x6121], &[[-14., -12., 2.], [-11., -12., 2.]]),
            (&[0x6142], &[[-14., -12., 0.], [-11., -12., 0.]]),
            (&[0x5f45], &[[40., -15., 0.], [14., -12., 0.]]),
            (&[0x5f6c], &[[40., -15., 2.], [14., -12., 2.]]),
            (&[0x5f93], &[[14., -12., 2.], [11., -12., 2.]]),
            (&[0x5fb4], &[[14., -12., 0.], [11., -12., 0.]]),
        ],
        Control::Gear => vec![
            (&[0x5a30, 0x5a5d], &[[-9., -6., -5.]]),
            (&[0x5c30, 0x5c5d], &[[9., -6., -5.]]),
            (&[0x5a8a, 0x5ab1], &[[-6., -9., -8.], [-6., 0., -8.]]),
            (&[0x5c8a, 0x5cb1], &[[6., -9., -8.], [6., 0., -8.]]),
            (&[0x5e92, 0x5eb1], &[[0., 30., -8.], [0., 36., -8.]]),
            (&[0x5ed0, 0x5eef], &[[-2., 33., -8.], [2., 33., -8.]]),
            (&[0x5e1f, 0x5e36], &[[0., 20., -7.], [0., 21., -7.]]),
            (&[0x5989, 0x59a0], &[[-8., -10., -4.], [-8., 7., -4.]]),
            (&[0x5b89, 0x5ba0], &[[9., -11., -4.], [9., 6., -4.]]),
            (&[0x5d89], &[[4., 37., -8.], [4., 20., -8.]]),
            (&[0x5da0], &[[4., 30., -8.], [4., 20., -8.]]),
            (&[0x5dbf], &[[4., 37., -8.], [4., 30., -8.]]),
        ],
        Control::Brake => vec![
            (&[0x63c0, 0x63d7, 0x63ee, 0x6405], &[[-11., -48., 1.]]),
            (&[0x641c, 0x6433, 0x644a, 0x6461], &[[11., -48., 1.]]),
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
    if value == 0. && matches!(control, Control::Gear | Control::Brake) {
        return;
    }
    let source = if matches!(control, Control::Gear | Control::Brake) {
        reference
    } else {
        raw
    };
    for (addresses, points) in roots(control) {
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
        let before = keyed(&old);
        let after = keyed(&new);
        metric.reviewed_anchor_missing |= before.iter().any(|(key, f)| {
            after
                .get(key)
                .is_none_or(|g| g.positions.len() != f.positions.len())
        });
        for point in points {
            let moved = f4::witness_positions(&before, &after, *point);
            metric.reviewed_anchor_missing |= moved.is_empty();
            for p in moved {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*point, p) * scale);
            }
        }
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
    }
    let paired: Vec<_> = match control {
        Control::Flaps | Control::Aileron => FLAPS.to_vec(),
        Control::Elevator => vec![0x5606, 0x564b, 0x52cd, 0x52e6],
        Control::Rudder => vec![0x6323, 0x634a],
        _ => Vec::new(),
    };
    let old: Vec<_> = source
        .iter()
        .filter(|f| paired.contains(&f.address))
        .cloned()
        .collect();
    let new: Vec<_> = pose
        .iter()
        .filter(|f| paired.contains(&f.address))
        .cloned()
        .collect();
    if let Some(gap) = shared_vertex_gaps(&keyed(&old), &keyed(&new), scale).first() {
        metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
    }
    if value == 0. {
        metric.reviewed_neutral_mismatch |= old.iter().any(|f| {
            !new.iter()
                .any(|g| g.address == f.address && g.positions == f.positions)
        });
    } else {
        let witnesses: Vec<(&[usize], [f32; 3], usize, f32)> = match control {
            Control::Rudder => vec![(&[0x6323, 0x634a], [0., -49., 11.], 0, 1.)],
            Control::Elevator => vec![
                (&[0x5606, 0x564b], [-26., -59., -3.], 2, 1.),
                (&[0x52cd, 0x52e6], [26., -59., -3.], 2, 1.),
            ],
            Control::Aileron => vec![
                (&FLAPS, [-40., -21., 1.], 2, -1.),
                (&FLAPS, [40., -21., 1.], 2, 1.),
            ],
            Control::Flaps => vec![
                (&FLAPS, [-40., -21., 1.], 2, -1.),
                (&FLAPS, [40., -21., 1.], 2, -1.),
            ],
            _ => Vec::new(),
        };
        for (side, (addresses, point, axis, sign)) in witnesses.into_iter().enumerate() {
            let selected: Vec<_> = source
                .iter()
                .filter(|f| addresses.contains(&f.address))
                .cloned()
                .collect();
            let actual: Vec<_> = pose
                .iter()
                .filter(|f| addresses.contains(&f.address))
                .cloned()
                .collect();
            let moved = f4::witness_positions(&keyed(&selected), &keyed(&actual), point);
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
    let after = keyed(pose);
    let mut right = f32::INFINITY;
    let mut left = f32::NEG_INFINITY;
    let mut groups = [0; 3];
    for (key, source) in keyed(reference) {
        let side = if LEFT_WHEELS.contains(&key.0) {
            0
        } else if RIGHT_WHEELS.contains(&key.0) {
            1
        } else if NOSE_WHEELS.contains(&key.0) {
            2
        } else {
            continue;
        };
        if source.positions.iter().any(|p| p[2] > -14. + 1e-4) {
            continue;
        }
        groups[side] += 1;
        let Some(actual) = after.get(&key) else {
            metric.reviewed_wheel_failed = true;
            continue;
        };
        if actual.positions.len() != source.positions.len() {
            metric.reviewed_wheel_failed = true;
            continue;
        }
        for p in &actual.positions {
            if side == 0 {
                left = left.max(p[0] * scale);
            } else if side == 1 {
                right = right.min(p[0] * scale);
            }
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
            let (min, max) = match side {
                0 => ([-9.5, -9., -3.], [-1.5, 0., 4.]),
                1 => ([1.3, -9., -3.], [10.3, 0., 4.]),
                _ => ([-2., 19.5, -4.5], [2., 25.5, 2.5]),
            };
            metric.reviewed_wheel_failed |= actual
                .positions
                .iter()
                .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-3 || p[i] > max[i] + 1e-3));
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |= groups != [4, 4, 4]
        || right < 0.35
        || left > -0.35
        || right - left < 0.70
        || metric.reviewed_wheel_rigidity_error > EPSILON;
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
        let baseline = airframe.animation_faces(&state);
        let baseline_map = keyed(&baseline);
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            state.aileron = roll;
            let actual = airframe.animation_faces(&state);
            let actual_map = keyed(&actual);
            let mut metric = measure(&original, &actual, scale);
            check(
                Control::Flaps,
                flap,
                (&original, &original, &actual),
                scale,
                &mut metric,
            );
            // Combined signed roll must be measured from this flap's zero-roll
            // geometry, never from an unrelated neutral flap position.
            metric.reviewed_direction_failures = 0;
            if roll != 0. {
                for (point, sign) in [([-40., -21., 1.], -1.), ([40., -21., 1.], 1.)] {
                    for (key, face) in keyed(&original)
                        .into_iter()
                        .filter(|(key, _)| FLAPS.contains(&key.0))
                    {
                        for (i, p) in face
                            .positions
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| **p == point)
                        {
                            let _ = p;
                            let dz = actual_map
                                .get(&key)
                                .and_then(|f| f.positions.get(i))
                                .zip(baseline_map.get(&key).and_then(|f| f.positions.get(i)))
                                .map(|(a, b)| a[2] - b[2]);
                            if dz.is_none_or(|dz| dz * sign * roll as f32 <= EPSILON) {
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

#[cfg(test)]
mod tests {
    use super::*;
    fn fixtures() -> Vec<Face> {
        let mut faces = Vec::new();
        for (ids, side) in [(&LEFT_WHEELS, -1.), (&RIGHT_WHEELS, 1.)] {
            for &id in ids {
                faces.push(super::super::tests::face(
                    id,
                    vec![
                        [side * 7., -4., -15.],
                        [side * 10., -4., -15.],
                        [side * 10., -2., -19.],
                        [side * 7., -2., -19.],
                    ],
                ));
            }
        }
        for id in NOSE_WHEELS {
            faces.push(super::super::tests::face(
                id,
                vec![
                    [-1., 31., -15.],
                    [1., 31., -15.],
                    [1., 34., -19.],
                    [-1., 34., -19.],
                ],
            ));
        }
        faces
    }
    #[test]
    fn near_stow_requires_its_envelope_even_when_rigid_sheets_are_separated() {
        let source = fixtures();
        let mut mid = Metrics::default();
        wheels(&source, &source, 0.5, 1. / 3., &mut mid);
        assert!(!mid.reviewed_wheel_failed);
        let mut stow = Metrics::default();
        wheels(&source, &source, 1e-6, 1. / 3., &mut stow);
        assert!(stow.reviewed_wheel_failed);
        assert_eq!(stow.reviewed_wheel_rigidity_error, 0.);
    }
    #[test]
    fn every_lower_piece_is_required_and_dimension_changes_are_rejected() {
        let source = fixtures();
        let mut missing = source.clone();
        missing.pop();
        let mut metric = Metrics::default();
        wheels(&source, &missing, 0.5, 1. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
        let mut stretched = source.clone();
        stretched[0].positions[0][2] -= 2.;
        let mut metric = Metrics::default();
        wheels(&source, &stretched, 0.5, 1. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
        assert!(metric.reviewed_wheel_rigidity_error > EPSILON);
    }
}
