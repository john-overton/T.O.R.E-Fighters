//! Independent Yak141 source attachments, original nozzle cards and complete gear.
use super::*;
const NOZZLES: [usize; 9] = [
    0x2401, 0x241e, 0x244f, 0x246c, 0x24b2, 0x2b4f, 0x355a, 0x3592, 0x2489,
];
const GEAR: [usize; 6] = [0x4ba9, 0x4bc8, 0x4c14, 0x4c33, 0x4c80, 0x4c9f];
fn attachments(control: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match control {
        Control::Elevator => vec![
            (&[0x4b07, 0x4b4f], &[[-12., -57., -1.], [-13., -82., -1.]]),
            (&[0x4b64], &[[-12., -57., -1.]]),
            (&[0x484c, 0x4894], &[[14., -57., -1.], [14., -82., -1.]]),
        ],
        Control::Flaps => vec![
            (&[0x4dfe, 0x4e25], &[[-13., -26., 3.], [-29., -26., 3.]]),
            (&[0x4d3b, 0x4d62], &[[14., -26., 3.], [30., -26., 3.]]),
        ],
        Control::Aileron => vec![
            (
                &[0x4945, 0x4a67, 0x4ad9],
                &[[-29., -28.5, 3.], [-54., -34.5, 3.]],
            ),
            (
                &[0x453a, 0x462f, 0x4698],
                &[[30., -28.5, 3.], [55., -34.5, 3.]],
            ),
        ],
        Control::Gear => vec![
            (&[0x4ba9, 0x4bc8], &[[9., -28., -9.], [9., -6., -9.]]),
            (&[0x4c14, 0x4c33], &[[-9., -28., -8.], [-9., -6., -8.]]),
            (&[0x4c80, 0x4c9f], &[[0., 53., -10.], [0., 63., -10.]]),
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
    let source = if matches!(control, Control::Gear | Control::Aileron | Control::Rudder) {
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
    if matches!(control, Control::Elevator) {
        for ids in [&[0x4b07, 0x4b4f, 0x4b64][..], &[0x484c, 0x4894][..]] {
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
            if let Some(g) = shared_vertex_gaps(&keyed(&a), &keyed(&b), scale).first() {
                metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(g.gap);
            }
        }
    }
    let signed: Vec<(&[usize], [f32; 3], f32)> = match control {
        Control::Elevator => vec![
            (&[0x4b07, 0x4b4f, 0x4b64], [-32., -85., -1.], 1.),
            (&[0x484c, 0x4894], [33., -85., -1.], 1.),
        ],
        Control::Aileron => vec![
            (&[0x4945, 0x4a67, 0x4ad9], [-54., -36., 3.], -1.),
            (&[0x453a, 0x462f, 0x4698], [55., -36., 3.], 1.),
        ],
        Control::Flaps => vec![
            (&[0x4dfe, 0x4e25], [-29., -31., 3.], -1.),
            (&[0x4d3b, 0x4d62], [30., -31., 3.], -1.),
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
            .filter(|f| [0x42b7, 0x430a, 0x4342, 0x4366].contains(&f.address))
            .cloned()
            .collect();
        let new: Vec<_> = pose
            .iter()
            .filter(|f| [0x42b7, 0x430a, 0x4342, 0x4366].contains(&f.address))
            .cloned()
            .collect();
        let before = keyed(&old);
        let after = keyed(&new);
        for (key, face) in &before {
            for (i, p) in face.positions.iter().enumerate() {
                let d = p[1] + 74. + (5. / 21.) * (p[2] - 6.);
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
    if matches!(control, Control::VectorPitch | Control::VectorYaw) {
        nozzles(raw, pose, scale, metric);
    }
    if matches!(control, Control::Gear) {
        wheels(reference, pose, value, scale, metric);
    }
}
fn nozzles(raw: &[Face], pose: &[Face], scale: f32, metric: &mut Metrics) {
    let original: Vec<_> = raw
        .iter()
        .filter(|f| NOZZLES.contains(&f.address))
        .cloned()
        .collect();
    let actual: Vec<_> = pose
        .iter()
        .filter(|f| NOZZLES.contains(&f.address))
        .cloned()
        .collect();
    let old = keyed(&original);
    let new = keyed(&actual);
    metric.reviewed_anchor_missing |= old.len() != 9 || new.len() != 9;
    for (key, source) in &old {
        let Some(face) = new.get(key) else {
            metric.reviewed_anchor_missing = true;
            continue;
        };
        metric.reviewed_anchor_missing |= source.positions.len() != face.positions.len();
        for (p, q) in source.positions.iter().zip(&face.positions) {
            if p[1] == -47. {
                metric.max_reviewed_anchor_gap =
                    metric.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
            }
        }
        if key.0 == 0x2489 {
            for i in 0..source.positions.len() {
                for j in i + 1..source.positions.len() {
                    metric.reviewed_wheel_rigidity_error =
                        metric.reviewed_wheel_rigidity_error.max(
                            (distance(source.positions[i], source.positions[j])
                                - distance(face.positions[i], face.positions[j]))
                            .abs()
                                * scale,
                        );
                }
            }
        }
    }
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(g.gap);
    }
    metric.reviewed_wheel_failed |= metric.reviewed_wheel_rigidity_error > EPSILON;
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    let after = keyed(pose);
    let mut count = 0;
    for (key, source) in keyed(reference) {
        if !GEAR.contains(&key.0) {
            continue;
        }
        let cut = match key.0 {
            0x4ba9 | 0x4bc8 => -13.,
            0x4c14 | 0x4c33 => -12.,
            _ => -14.,
        };
        if !source.positions.iter().all(|p| p[2] <= cut + 1e-4) {
            continue;
        }
        count += 1;
        let Some(actual) = after.get(&key) else {
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
        let side = if key.0 < 0x4c00 {
            1.
        } else if key.0 < 0x4c80 {
            -1.
        } else {
            0.
        };
        if side != 0. {
            metric.reviewed_wheel_failed |= actual.positions.iter().any(|p| p[0] * side < 8.3);
        }
        if travel < 1e-5 {
            let (min, max) = match key.0 {
                0x4ba9 | 0x4bc8 => ([8.6667, -28., -5.], [12., -6., 3.]),
                0x4c14 | 0x4c33 => ([-11., -28., -4.], [-8.3333, -6., 4.]),
                _ => ([0., 47., -6.], [0., 57., 1.]),
            };
            metric.reviewed_wheel_failed |= actual
                .positions
                .iter()
                .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-3 || p[i] > max[i] + 1e-3));
        }
    }
    metric.reviewed_wheel_failed |= count != 6 || metric.reviewed_wheel_rigidity_error > EPSILON;
}
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let mut failures = Vec::new();
    let mut poses = Vec::new();
    let mut csv = String::from("pitch,yaw,front_ring_gap_ft,rigidity_error_ft,skin_gap_ft\n");
    for pitch in [0., 0.25, 0.5, 0.75, 1.] {
        for yaw in [-1., -0.5, 0., 0.5, 1.] {
            let mut s = neutral.clone();
            s.lift_controls.vector_pitch_actual = pitch;
            s.lift_controls.vector_yaw_actual = yaw;
            let faces = airframe.animation_faces(&s);
            let mut m = measure(&original, &faces, airframe.animation_scale());
            nozzles(&original, &faces, airframe.animation_scale(), &mut m);
            writeln!(
                csv,
                "{pitch},{yaw},{},{},{}",
                m.max_reviewed_anchor_gap, m.reviewed_wheel_rigidity_error, m.max_reviewed_skin_gap
            )?;
            if m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.reviewed_wheel_failed
                || m.max_reviewed_skin_gap > EPSILON
                || !m.new_planar_crossings.is_empty()
            {
                failures.push(format!(
                    "nozzle pitch{pitch}/yaw{yaw} fixed front ring/rigid outlet/skin witness"
                ));
            }
            poses.push(faces);
        }
    }
    fs::write(out.join("nozzle-combinations.csv"), csv)?;
    contact_sheet(&out.join("nozzle-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
