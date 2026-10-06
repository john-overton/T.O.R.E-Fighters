//! Independent MiG21 own hinges, asymmetric shaft centers and fitted brake anchors.
use super::*;
const GEAR: [usize; 6] = [0x3275, 0x3290, 0x31af, 0x31ca, 0x3212, 0x322d];
const FIN: [usize; 3] = [0x1bd3, 0x1e9a, 0x1ebd];
fn anchors(c: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match c {
        Control::Elevator => vec![
            (&[0x2aae, 0x2ac9], &[[6., -37., -2.], [6., -47., -2.]]),
            (
                &[0x2b51, 0x2b6c],
                &[[6., -47., -2.], [5., -54., -2.], [6., -56., -2.]],
            ),
            (&[0x2d5b, 0x2e50], &[[-6., -37., -2.], [-6., -47., -2.]]),
            (
                &[0x2d7e, 0x2ea1],
                &[[-6., -47., -2.], [-5., -54., -2.], [-5., -56., -2.]],
            ),
        ],
        Control::Flaps => vec![
            (&[0x2b1b], &[[7., -18., -2.], [25., -18., -2.]]),
            (&[0x2e6b], &[[-7., -18., -2.], [-25., -18., -2.]]),
        ],
        Control::Aileron => vec![
            (&[0x2b36], &[[25., -15., -2.], [38., -19., -2.]]),
            (&[0x2e86], &[[-25., -15., -2.], [-38., -19., -2.]]),
        ],
        Control::Brake => vec![
            (&[0x2757], &[[2., 12., -8.], [5., 12., -6.]]),
            (&[0x276c], &[[-2., 12., -8.], [-6., 12., -6.]]),
            (&[0x2817], &[[-2., 12., -8.], [2., 12., -8.]]),
        ],
        Control::Gear => vec![
            (&[0x31af, 0x31ca], &[[21., 0., -1.], [21., -11., -1.]]),
            (&[0x3212, 0x322d], &[[-20., 0., -1.], [-20., -11., -1.]]),
        ],
        _ => Vec::new(),
    }
}
fn brake_parts(faces: &[Face]) -> Vec<Face> {
    faces
        .iter()
        .filter(|f| {
            [0x2757, 0x276c, 0x2817].contains(&f.address)
                && f.positions.iter().any(|p| (p[1] - 12.).abs() < 1e-4)
                && f.positions.iter().all(|p| p[1] <= 12. + 1e-4)
        })
        .cloned()
        .collect()
}
pub(super) fn check(
    c: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    m: &mut Metrics,
) {
    let (_, reference, pose) = geometry;
    if matches!(c, Control::Gear) && value == 0. {
        m.reviewed_wheel_failed = pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    let original_brake = brake_parts(reference);
    let actual_brake = brake_parts(pose);
    let (reference, pose) = if matches!(c, Control::Brake) {
        (&original_brake[..], &actual_brake[..])
    } else {
        (reference, pose)
    };
    for (ids, points) in anchors(c) {
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
    let signed: Vec<(&[usize], [f32; 3], f32)> = match c {
        Control::Elevator => vec![
            (&[0x2aae, 0x2ac9, 0x2b51, 0x2b6c], [21., -62., -2.], 1.),
            (&[0x2d5b, 0x2d7e, 0x2e50, 0x2ea1], [-20., -62., -2.], 1.),
        ],
        Control::Aileron => vec![
            (&[0x2b36], [38., -23., -2.], 1.),
            (&[0x2e86], [-38., -23., -2.], -1.),
        ],
        Control::Flaps => vec![
            (&[0x2b1b], [7., -23., -2.], -1.),
            (&[0x2e6b], [-7., -23., -2.], -1.),
        ],
        _ => Vec::new(),
    };
    if value != 0. {
        for (side, (ids, p, sign)) in signed.into_iter().enumerate() {
            let old: BTreeMap<_, _> = keyed(reference)
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
    if matches!(c, Control::Rudder) {
        let old: BTreeMap<_, _> = keyed(reference)
            .into_iter()
            .filter(|(k, _)| FIN.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| FIN.contains(&k.0))
            .collect();
        for (key, f) in &old {
            for (i, p) in f.positions.iter().enumerate() {
                if (p[1] + 52.).abs() < 1e-4 {
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
    if matches!(c, Control::Brake) {
        m.reviewed_anchor_missing |= reference.len() != 3 || pose.len() != 3;
        if let Some(g) = shared_vertex_gaps(&keyed(reference), &keyed(pose), scale).first() {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
        }
    }
    if matches!(c, Control::Gear) {
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
        let nose = [0x3275, 0x3290].contains(&key.0);
        let left = [0x3212, 0x322d].contains(&key.0);
        let hub = if nose {
            [0., 49.5, -7.5]
        } else {
            [if left { -20. } else { 21. }, -5.5, -1.]
        };
        if let Some(p) = virtual_point(before, after, hub) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, hub) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
        if !nose {
            let bound = if left { 3. } else { 4. };
            m.reviewed_wheel_failed |= after
                .positions
                .iter()
                .any(|p| p[0] * hub[0].signum() < bound - 1e-4);
        }
        if travel < 1e-5 {
            let (min, max) = if nose {
                ([0., 39., -11.], [0., 50., -4.])
            } else if left {
                ([-20., -11., -1.], [-3., 0., -1.])
            } else {
                ([4., -11., -1.], [21., 0., -1.])
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
            if m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.max_reviewed_skin_gap > EPSILON
                || !m.new_planar_crossings.is_empty()
                || m.reviewed_direction_failures != 0
            {
                failures.push(format!(
                    "MiG21 flap{flap}/roll{roll} own hinge or direction"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
