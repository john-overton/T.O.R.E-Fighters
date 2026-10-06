//! Independent Su35 own-source witnesses, including every split24c0 piece.
use super::*;
const GEAR: [usize; 18] = [
    0x5432, 0x5451, 0x5470, 0x548f, 0x54ae, 0x54cd, 0x5680, 0x5697, 0x56f3, 0x5712, 0x5731, 0x5750,
    0x5549, 0x5568, 0x5587, 0x55a6, 0x55c5, 0x55e4,
];
const FIN: [usize; 4] = [0x2268, 0x4edf, 0x2301, 0x26a8];
fn selected(c: Control, faces: &[Face], left: bool) -> Vec<Face> {
    let ids: &[usize] = match (c, left) {
        (Control::Flaps, false) => &[0x24c0, 0x251c],
        (Control::Flaps, true) => &[0x5897, 0x58b6],
        (Control::Aileron, false) => &[0x24c0, 0x2537, 0x254f, 0x25a3],
        (Control::Aileron, true) => &[0x517b, 0x5198, 0x51d6, 0x52f0],
        _ => &[],
    };
    faces
        .iter()
        .filter(|f| {
            ids.contains(&f.address)
                && (f.address != 0x24c0
                    || f.positions.iter().any(|p| p[0] > 60.) == matches!(c, Control::Aileron))
        })
        .cloned()
        .collect()
}
fn anchor_groups(c: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match c {
        Control::Elevator => vec![
            (
                &[0x25f2, 0x2639],
                &[[20., -25., -5.], [17., -46., -5.], [24., -58., -5.]],
            ),
            (
                &[0x5312, 0x535e],
                &[[-20., -25., -5.], [-20., -46., -5.], [-24., -58., -5.]],
            ),
            (&[0x256d, 0x2580], &[[19., 38., 0.], [20., 53., 0.]]),
            (&[0x52b0, 0x52d7], &[[-20., 38., 0.], [-20., 53., 0.]]),
        ],
        Control::Gear => vec![
            (&[0x5680, 0x5697], &[[-1., 73., -3.], [-1., 73., -7.]]),
            (&[0x5731, 0x5750], &[[-3., 57., -1.], [3., 57., -1.]]),
        ],
        Control::Brake => vec![(&[0x5944, 0x596b], &[[-5., 23., 8.], [7., 23., 8.]])],
        _ => Vec::new(),
    }
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
    if matches!(c, Control::Brake) && value == 0. {
        m.reviewed_wheel_failed = pose.iter().any(|f| [0x5944, 0x596b].contains(&f.address));
        return;
    }
    for (ids, points) in anchor_groups(c) {
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
    if matches!(c, Control::Flaps | Control::Aileron) {
        for left in [true, false] {
            let old_faces = selected(c, reference, left);
            let new_faces = selected(c, pose, left);
            let old = keyed(&old_faces);
            let new = keyed(&new_faces);
            let (inner, outer) = if left { (-20., -50.) } else { (19., 49.) };
            let points = if matches!(c, Control::Flaps) {
                vec![
                    [inner, -4., 0.],
                    [inner, -4., -1.],
                    [outer, -15., 0.],
                    [outer, -15., -1.],
                ]
            } else {
                let inner = if left { -50. } else { 49. };
                let outer = if left { -71. } else { 71. };
                vec![
                    [inner, -15., 0.],
                    [inner, -15., -1.],
                    [outer, -13., 0.],
                    [outer, -13., -1.],
                ]
            };
            for p in points {
                let qs = f4::witness_positions(&old, &new, p);
                m.reviewed_anchor_missing |= qs.is_empty();
                for q in qs {
                    m.max_reviewed_anchor_gap =
                        m.max_reviewed_anchor_gap.max(distance(p, q) * scale);
                }
            }
            if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
                m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
            }
            if value != 0. {
                let p = if matches!(c, Control::Flaps) {
                    [inner, -14., 0.]
                } else {
                    [if left { -71. } else { 71. }, -27., 0.]
                };
                let sign = if matches!(c, Control::Flaps) || left {
                    -1.
                } else {
                    1.
                };
                let qs = f4::witness_positions(&old, &new, p);
                m.reviewed_direction_failures += usize::from(qs.is_empty());
                for q in qs {
                    m.reviewed_direction_failures +=
                        usize::from((q[2] - p[2]) * sign * value.signum() as f32 <= EPSILON);
                }
            }
            if matches!(c, Control::Flaps) && value == 1. {
                for (p, q) in [
                    ([inner, -14., 0.], [inner, -13., -7.]),
                    ([outer, -22., 0.], [outer, -20., -4.]),
                ] {
                    let qs = f4::witness_positions(&old, &new, p);
                    m.reviewed_anchor_missing |= qs.is_empty();
                    m.reviewed_neutral_mismatch |=
                        qs.into_iter().any(|p| distance(p, q) * scale > EPSILON);
                }
            }
        }
    }
    if matches!(c, Control::Elevator) && value != 0. {
        for (ids, p, sign) in [
            (&[0x25f2, 0x2639][..], [43., -59., -5.], 1.),
            (&[0x5312, 0x535e][..], [-43., -59., -5.], 1.),
            (&[0x256d, 0x2580][..], [33., 32., 0.], -1.),
            (&[0x52b0, 0x52d7][..], [-32., 32., 0.], -1.),
        ] {
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
                m.reviewed_direction_failures +=
                    usize::from((q[2] - p[2]) * sign * value.signum() as f32 <= EPSILON);
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
                if (p[1] + 30.).abs() < 1e-4 {
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
    if matches!(c, Control::Flaps) {
        for (a, left) in [(0x5857, true), (0x57d8, false)] {
            if let Some(cap) = pose.iter().find(|f| f.address == a) {
                if value == 0. {
                    m.reviewed_anchor_missing = true;
                }
                let fs = selected(Control::Flaps, pose, left);
                for p in &cap.positions {
                    m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(
                        fs.iter()
                            .flat_map(|f| &f.positions)
                            .map(|q| distance(*p, *q) * scale)
                            .fold(f32::INFINITY, f32::min),
                    );
                }
            } else if value != 0. {
                m.reviewed_anchor_missing = true;
            }
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
        let Some(after) = new.get(key) else {
            m.reviewed_wheel_failed = true;
            continue;
        };
        if [0x5680, 0x5697].contains(&key.0) {
            let ids: Vec<_> = before
                .positions
                .iter()
                .enumerate()
                .filter(|(_, p)| p[1] == 53.)
                .map(|(i, _)| i)
                .collect();
            if ids.len() != 2 {
                m.reviewed_anchor_missing = true;
            } else {
                m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(
                    (distance(after.positions[ids[0]], after.positions[ids[1]]) - 4.).abs() * scale,
                );
            }
            continue;
        }
        count += 1;
        rigid(before, after, scale, m);
        let nose = [0x56f3, 0x5712, 0x5731, 0x5750].contains(&key.0);
        let left = key.0 >= 0x5549 && key.0 < 0x5680;
        let hub = if nose {
            [0., 57., -1.]
        } else {
            [if left { -23. } else { 23. }, 3., -1.]
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
                .any(|p| p[0] * hub[0].signum() < 2.38);
        }
        if travel < 1e-5 {
            let (min, max) = if nose {
                ([-3., 36., -3.], [3., 57., 14.])
            } else if left {
                ([-23., -3., -7.], [-3., 9., 4.])
            } else {
                ([3., -3., -7.], [23., 10., 4.])
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
            if m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.max_reviewed_skin_gap > EPSILON
                || !m.new_planar_crossings.is_empty()
                || m.reviewed_direction_failures != 0
                || m.reviewed_neutral_mismatch
            {
                failures.push(format!(
                    "Su35 flap{flap}/roll{roll} own-role attachment, source endpoint or direction"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_face_selection_preserves_both_roles_instead_of_overwriting_address() {
        let a = super::super::tests::face(
            0x24c0,
            vec![
                [19., -14., 0.],
                [19., -4., 0.],
                [49., -15., 0.],
                [49., -22., 0.],
            ],
        );
        let b = super::super::tests::face(
            0x24c0,
            vec![[49., -15., 0.], [71., -27., 0.], [49., -22., 0.]],
        );
        let faces = vec![a.clone(), b.clone()];
        assert_eq!(
            selected(Control::Flaps, &faces, false)[0].positions,
            a.positions
        );
        assert_eq!(
            selected(Control::Aileron, &faces, false)[0].positions,
            b.positions
        );
    }
}
