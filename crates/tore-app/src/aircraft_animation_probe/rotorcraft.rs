//! Independent rigid-panel witnesses for reviewed helicopter rotor groups.
//! Mast centers and retained face identities come from the source geometry review.
use super::*;

struct Group {
    faces: &'static [usize],
    hub: [f32; 3],
    main: bool,
    yaw_sign: f32,
}
fn groups(id: AircraftId) -> &'static [Group] {
    match id {
        AircraftId::Ah64 => &[
            Group {
                faces: &[0x3553, 0x35b7, 0x41e6, 0x4255, 0x44c8, 0x4537],
                hub: [0., 5., 19.],
                main: true,
                yaw_sign: 0.,
            },
            Group {
                faces: &[0x4320, 0x4459],
                hub: [-5., -93., 18.],
                main: false,
                yaw_sign: 0.,
            },
        ],
        AircraftId::Mi24 => &[
            Group {
                faces: &[0x4d5f, 0x4db9, 0x4f2e, 0x4f88],
                hub: [0., -3., 23.],
                main: true,
                yaw_sign: 0.,
            },
            Group {
                faces: &[0x2f65, 0x30c1],
                hub: [-5., -112., 27.],
                main: false,
                yaw_sign: 0.,
            },
        ],
        AircraftId::Ch47 => &[
            Group {
                faces: &[0x2067, 0x3e0c],
                hub: [0., -45., 29.],
                main: true,
                yaw_sign: -1.,
            },
            Group {
                faces: &[0x3ae5, 0x3c63],
                hub: [0., 67., 15.],
                main: true,
                yaw_sign: 1.,
            },
        ],
        _ => &[],
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
pub(super) fn inspect_panel(
    before: &Face,
    after: &Face,
    hub: [f32; 3],
    scale: f32,
    metric: &mut Metrics,
) -> Option<V> {
    if before.positions.len() != after.positions.len() || before.positions.len() < 3 {
        metric.reviewed_anchor_missing = true;
        return None;
    }
    for i in 0..before.positions.len() {
        for j in i + 1..before.positions.len() {
            metric.reviewed_rigid_panel_error = metric.reviewed_rigid_panel_error.max(
                (distance(before.positions[i], before.positions[j])
                    - distance(after.positions[i], after.positions[j]))
                .abs()
                    * scale,
            );
        }
    }
    let Some((old, new)) = observed_frames(before, after) else {
        metric.reviewed_anchor_missing = true;
        return None;
    };
    let local = sub(hub.map(f64::from), before.positions[0].map(f64::from));
    let offset = map_vector(local, old, new);
    let observed: [f32; 3] =
        std::array::from_fn(|i| (after.positions[0][i] as f64 + offset[i]) as f32);
    metric.max_reviewed_anchor_gap = metric
        .max_reviewed_anchor_gap
        .max(distance(observed, hub) * scale);
    Some(map_vector([0., 0., 1.], old, new))
}
fn spin_plane_error(before: &Face, after: &Face) -> f32 {
    let Some((frame, _)) = observed_frames(before, after) else {
        return f32::INFINITY;
    };
    let origin = before.positions[0].map(f64::from);
    after
        .positions
        .iter()
        .map(|p| dot(sub(p.map(f64::from), origin), frame[2]).abs() as f32)
        .fold(0., f32::max)
}
pub(super) fn check(
    id: AircraftId,
    control: Control,
    value: f64,
    reference: &[Face],
    pose: &[Face],
    scale: f32,
    metric: &mut Metrics,
) {
    let definitions = groups(id);
    let applicable = matches!(
        control,
        Control::Elevator | Control::Aileron | Control::Rotor
    ) || id == AircraftId::Ch47 && matches!(control, Control::Rudder);
    if definitions.is_empty() || !applicable {
        return;
    }
    let old = keyed(reference);
    let new = keyed(pose);
    let mut active = std::collections::BTreeSet::new();
    for group in definitions {
        if !group.main && !matches!(control, Control::Rotor) {
            continue;
        }
        for address in group.faces {
            active.insert(*address);
            let key = (*address, 0);
            let (Some(before), Some(after)) = (old.get(&key), new.get(&key)) else {
                metric.reviewed_anchor_missing = true;
                continue;
            };
            if matches!(control, Control::Rotor)
                && spin_plane_error(before, after) * scale > EPSILON
            {
                metric.reviewed_direction_failures += 1;
            }
            if let Some(up) = inspect_panel(before, after, group.hub, scale, metric) {
                if value == 0. {
                    metric.reviewed_neutral_mismatch |= before
                        .positions
                        .iter()
                        .zip(&after.positions)
                        .any(|(a, b)| distance(*a, *b) * scale > EPSILON);
                } else if group.main {
                    let signed = match control {
                        Control::Elevator => Some(-up[1]),
                        Control::Aileron => Some(up[0]),
                        Control::Rudder => Some(up[0] * group.yaw_sign as f64),
                        _ => None,
                    };
                    if signed.is_some_and(|v| v * value.signum() <= EPSILON as f64) {
                        metric.reviewed_direction_failures += 1;
                    }
                }
            }
        }
        let selected_old: BTreeMap<_, _> = old
            .iter()
            .filter(|(key, _)| group.faces.contains(&key.0))
            .map(|(k, v)| (*k, *v))
            .collect();
        let selected_new: BTreeMap<_, _> = new
            .iter()
            .filter(|(key, _)| group.faces.contains(&key.0))
            .map(|(k, v)| (*k, *v))
            .collect();
        if let Some(gap) = shared_vertex_gaps(&selected_old, &selected_new, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
    }
    // Cyclic and spin must not drag a mast, fuselage, fixed gear or inactive tail.
    for (key, before) in old.iter().filter(|(key, _)| !active.contains(&key.0)) {
        metric.reviewed_direction_failures += usize::from(
            new.get(key)
                .is_none_or(|after| before.positions != after.positions),
        );
    }
}

fn signed_area(p: &[[f64; 2]]) -> f64 {
    p.iter()
        .zip(p.iter().cycle().skip(1))
        .take(p.len())
        .map(|(a, b)| a[0] * b[1] - a[1] * b[0])
        .sum()
}
fn side(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}
fn footprint(face: &Face) -> Vec<[f64; 2]> {
    let mut p: Vec<_> = face
        .positions
        .iter()
        .map(|p| [p[0] as f64, p[1] as f64])
        .collect();
    if signed_area(&p) < 0. {
        p.reverse();
    }
    p
}
fn overlap(mut p: Vec<[f64; 2]>, q: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    for (a, b) in q.iter().zip(q.iter().cycle().skip(1)).take(q.len()) {
        let mut out = Vec::new();
        for (x, y) in p.iter().zip(p.iter().cycle().skip(1)).take(p.len()) {
            let dx = side(*a, *b, *x);
            let dy = side(*a, *b, *y);
            if dx >= 0. {
                out.push(*x);
            }
            if (dx >= 0.) != (dy >= 0.) {
                let t = dx / (dx - dy);
                out.push(std::array::from_fn(|i| x[i] + t * (y[i] - x[i])));
            }
        }
        p = out;
        if p.is_empty() {
            break;
        }
    }
    p
}
fn height(face: &Face, p: [f64; 2]) -> Option<f64> {
    let (frame, _) = observed_frames(face, face)?;
    let normal = frame[2];
    if normal[2].abs() < 1e-6 {
        return None;
    }
    let origin = face.positions[0].map(f64::from);
    Some(origin[2] - (normal[0] * (p[0] - origin[0]) + normal[1] * (p[1] - origin[1])) / normal[2])
}
fn tandem_clearance(pose: &[Face]) -> Option<f64> {
    let rear = pose.iter().find(|f| f.address == 0x2067)?;
    let front = pose.iter().find(|f| f.address == 0x3ae5)?;
    let overlap = overlap(footprint(rear), footprint(front));
    let mut gap = f64::INFINITY;
    for p in overlap {
        gap = gap.min(height(rear, p)? - height(front, p)?);
    }
    Some(gap)
}
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let id = airframe.profile.id;
    let definitions = groups(id);
    if definitions.is_empty() {
        return Ok(Vec::new());
    }
    let scale = airframe.animation_scale();
    let original = airframe.animation_faces(neutral);
    let reference = keyed(&original);
    let values = [-1., -0.5, 0., 0.5, 1.];
    let yaw_values: &[f64] = if id == AircraftId::Ch47 {
        &values
    } else {
        &[0.]
    };
    let mut rows =
        String::from("pitch,roll,yaw,tick,anchor_error_ft,rigid_error_ft,tandem_gap_ft\n");
    let mut failures = Vec::new();
    let mut count = 0;
    let mut gap = f64::INFINITY;
    let mut max_anchor = 0f32;
    let mut max_rigid = 0f32;
    let mut worst = None;
    for pitch in values {
        for roll in values {
            for &yaw in yaw_values {
                for tick in 0..48 {
                    let mut state = neutral.clone();
                    state.elevator = pitch;
                    state.aileron = roll;
                    state.rudder = yaw;
                    state.engine = true;
                    state.throttle = 0.5;
                    state.ticks = tick;
                    let actual = airframe.animation_faces(&state);
                    let after = keyed(&actual);
                    let mut metric = Metrics::default();
                    for group in definitions {
                        for address in group.faces {
                            let key = (*address, 0);
                            if let (Some(before), Some(posed)) =
                                (reference.get(&key), after.get(&key))
                            {
                                inspect_panel(before, posed, group.hub, scale, &mut metric);
                            } else {
                                metric.reviewed_anchor_missing = true;
                            }
                        }
                    }
                    let clearance = if id == AircraftId::Ch47 {
                        tandem_clearance(&actual).map(|v| v * scale as f64)
                    } else {
                        Some(f64::INFINITY)
                    };
                    max_anchor = max_anchor.max(metric.max_reviewed_anchor_gap);
                    max_rigid = max_rigid.max(metric.reviewed_rigid_panel_error);
                    if let Some(clearance) = clearance
                        && clearance < gap
                    {
                        gap = clearance;
                        worst = Some(actual.clone());
                    }
                    let failed = metric.reviewed_anchor_missing
                        || metric.max_reviewed_anchor_gap > EPSILON
                        || metric.reviewed_rigid_panel_error > EPSILON
                        || clearance.is_none_or(|v| v <= 0.);
                    if failed && failures.len() < 8 {
                        failures.push(format!("combined rotor pitch{pitch}/roll{roll}/yaw{yaw}/tick{tick} attachment, rigidity or tandem overlap"));
                    }
                    writeln!(
                        rows,
                        "{pitch},{roll},{yaw},{tick},{},{},{}",
                        metric.max_reviewed_anchor_gap,
                        metric.reviewed_rigid_panel_error,
                        clearance.unwrap_or(f64::NAN)
                    )?;
                    count += 1;
                }
            }
        }
    }
    fs::write(out.join("rotor-combinations.csv"), rows)?;
    fs::write(
        out.join("rotor-combinations.json"),
        format!(
            "{{\"poses\":{count},\"checks_passed\":{},\"max_anchor_error_ft\":{max_anchor},\"max_rigid_error_ft\":{max_rigid},\"minimum_tandem_gap_ft\":{}}}\n",
            failures.is_empty(),
            if gap.is_finite() {
                gap.to_string()
            } else {
                "null".into()
            }
        ),
    )?;
    if let Some(worst) = worst {
        write_obj(&out.join("rotor-combined-worst.obj"), &worst)?;
        contact_sheet(&out.join("rotor-combined-worst.ppm"), &original, &[worst])?;
    }
    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic() -> Face {
        super::super::tests::face(1, vec![[3., 4., 5.], [15., 4., 5.], [3., 11., 5.]])
    }
    #[test]
    fn inferred_pivot_rejects_rigid_translation_and_distorted_blade() {
        let source = synthetic();
        let mut moved = source.clone();
        // An invented rigid 3-4-5 rotation around the first point.
        for p in &mut moved.positions {
            let y = p[1] - 4.;
            p[1] = 4. + 0.8 * y;
            p[2] = 5. + 0.6 * y;
        }
        let mut metric = Metrics::default();
        let up = inspect_panel(&source, &moved, [3., 4., 5.], 1., &mut metric).unwrap();
        assert_eq!(metric.max_reviewed_anchor_gap, 0.);
        assert!(metric.reviewed_rigid_panel_error < EPSILON);
        assert!(up[1] < -0.59 && up[2] > 0.79);
        for p in &mut moved.positions {
            p[0] += 1.;
        }
        let mut metric = Metrics::default();
        inspect_panel(&source, &moved, [3., 4., 5.], 1., &mut metric);
        assert!(metric.max_reviewed_anchor_gap > 0.9);
        moved.positions[1][0] += 2.;
        let mut metric = Metrics::default();
        inspect_panel(&source, &moved, [3., 4., 5.], 1., &mut metric);
        assert!(metric.reviewed_rigid_panel_error > 1.);
    }
    #[test]
    fn missing_rotor_panels_cannot_pass_attachment_review() {
        let mut metric = Metrics::default();
        check(
            AircraftId::Ah64,
            Control::Rotor,
            0.5,
            &[],
            &[],
            1.,
            &mut metric,
        );
        assert!(metric.reviewed_anchor_missing);
    }
    #[test]
    fn tandem_overlap_gate_rejects_crossed_rotor_planes() {
        let front = super::super::tests::face(
            0x3ae5,
            vec![[-3., -2., 0.], [3., -2., 0.], [3., 2., 0.], [-3., 2., 0.]],
        );
        let mut rear = front.clone();
        rear.address = 0x2067;
        for p in &mut rear.positions {
            p[2] = 2.;
        }
        assert_eq!(tandem_clearance(&[rear.clone(), front.clone()]), Some(2.));
        rear.positions[0][2] = -2.;
        rear.positions[3][2] = -2.;
        assert!(tandem_clearance(&[rear, front]).unwrap() < 0.);
    }
    #[test]
    fn spinning_about_wrong_axis_is_not_accepted_as_rigid_plane_motion() {
        let source = synthetic();
        let mut wrong = source.clone();
        for p in &mut wrong.positions {
            let y = p[1] - 4.;
            p[1] = 4. + 0.8 * y;
            p[2] = 5. + 0.6 * y;
        }
        assert!(spin_plane_error(&source, &wrong) > 1.);
        assert_eq!(spin_plane_error(&source, &source), 0.);
    }
}
