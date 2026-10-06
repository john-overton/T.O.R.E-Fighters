//! Independent Su25 source position/material correspondences and mechanical witnesses.
use super::*;
use tore_formats::shape::Shape;
const GEAR: [usize; 18] = [
    0x5ced, 0x5d04, 0x5b9d, 0x5bb4, 0x5ac0, 0x5adf, 0x5afe, 0x5b1d, 0x5d60, 0x5d7f, 0x5d9e, 0x5dbd,
    0x5a4d, 0x5a64, 0x5c10, 0x5c2f, 0x5c4e, 0x5c6d,
];
const MAIN_R: [usize; 4] = [0x5ac0, 0x5adf, 0x5afe, 0x5b1d];
const MAIN_L: [usize; 4] = [0x5c10, 0x5c2f, 0x5c4e, 0x5c6d];
const DOOR_R: [usize; 2] = [0x5a4d, 0x5a64];
const DOOR_L: [usize; 2] = [0x5b9d, 0x5bb4];
const NOSE: [usize; 4] = [0x5d60, 0x5d7f, 0x5d9e, 0x5dbd];
const BRACE: [usize; 2] = [0x5ced, 0x5d04];
pub(super) struct Sources {
    neutral: Shape,
    left: Shape,
    right: Shape,
    positive: Shape,
    negative: Shape,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        Ok(Self {
            neutral: Shape::parse(bytes)?,
            left: Shape::with_state(bytes, &[(0x83a2, -1)].into())?,
            right: Shape::with_state(bytes, &[(0x83a8, -1)].into())?,
            positive: Shape::with_state(bytes, &[(0x83ae, -1)].into())?,
            negative: Shape::with_state(bytes, &[(0x83ae, 1)].into())?,
        })
    }
}
fn face(faces: &[Face], a: usize) -> Option<&Face> {
    let mut fs = faces.iter().filter(|f| f.address == a);
    let f = fs.next()?;
    fs.next().is_none().then_some(f)
}
fn materials(
    s: &Sources,
    c: Control,
    value: f64,
    pose: &[Face],
    scale: f32,
    m: &mut Metrics,
) -> f32 {
    let definitions: Vec<(usize, usize, [usize; 4], &Shape)> = match c {
        Control::Flaps => vec![
            (0x5fbf, 0x5f75, [3, 0, 1, 2], &s.right),
            (0x5fde, 0x5f4e, [2, 3, 0, 1], &s.right),
            (0x60a1, 0x6057, [0, 1, 2, 3], &s.left),
            (0x60c0, 0x6030, [1, 2, 3, 0], &s.left),
        ],
        Control::Rudder if value >= 0. => vec![
            (0x5e68, 0x5dfb, [3, 0, 1, 2], &s.positive),
            (0x5e8f, 0x5e22, [1, 2, 3, 0], &s.positive),
        ],
        Control::Rudder => vec![
            (0x5e68, 0x5ed5, [2, 3, 0, 1], &s.negative),
            (0x5e8f, 0x5efc, [2, 3, 0, 1], &s.negative),
        ],
        _ => Vec::new(),
    };
    let t = value.abs().clamp(0., 1.) as f32;
    let mut error = 0f32;
    for (a, b, order, target) in definitions {
        let (Some(base), Some(endpoint), Some(actual)) = (
            face(&s.neutral.faces, a),
            face(&target.faces, b),
            face(pose, a),
        ) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        if base.positions.len() != 4
            || endpoint.positions.len() != 4
            || actual.positions.len() != 4
            || base.uv.len() != 4
            || endpoint.uv.len() != 4
            || actual.uv.len() != 4
        {
            m.reviewed_anchor_missing = true;
            continue;
        }
        m.reviewed_neutral_mismatch |= actual.texture != base.texture
            || actual.texture != endpoint.texture
            || actual.subtype != base.subtype
            || actual.subtype != endpoint.subtype
            || actual.colors != base.colors;
        for (i, j) in order.into_iter().enumerate() {
            let p: [f32; 3] = std::array::from_fn(|k| {
                base.positions[i][k] * (1. - t) + endpoint.positions[j][k] * t
            });
            m.reviewed_neutral_mismatch |= distance(p, actual.positions[i]) * scale > EPSILON;
            for k in 0..2 {
                let uv = base.uv[i][k] * (1. - t) + endpoint.uv[j][k] * t;
                error = error.max((uv - actual.uv[i][k]).abs());
            }
            m.reviewed_neutral_mismatch |= actual.colors.get(i) != endpoint.colors.get(j);
        }
        if matches!(c, Control::Rudder) && t > 0. {
            let span = actual
                .uv
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max)
                - actual.uv.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
            m.reviewed_neutral_mismatch |= span <= 1e-6;
        }
    }
    m.reviewed_neutral_mismatch |= error > 1e-4;
    error
}
fn groups(c: Control) -> Vec<(&'static [usize], &'static [[f32; 3]])> {
    match c {
        Control::Rudder => vec![(&[0x5e68, 0x5e8f], &[[0., -53., 7.], [0., -60., 27.]])],
        Control::Flaps => vec![
            (
                &[0x5fbf, 0x5fde],
                &[
                    [14., -7., 5.],
                    [14., -7., 6.],
                    [42., -8., 3.],
                    [42., -8., 4.],
                ],
            ),
            (
                &[0x60a1, 0x60c0],
                &[
                    [-14., -7., 5.],
                    [-14., -7., 6.],
                    [-42., -8., 3.],
                    [-42., -8., 4.],
                ],
            ),
        ],
        Control::Aileron => vec![
            (
                &[0x4b9d, 0x4c10],
                &[
                    [42., -8., 3.],
                    [42., -8., 4.],
                    [75., -10., 1.],
                    [75., -10., 2.],
                ],
            ),
            (
                &[0x4f0e, 0x4fb4],
                &[
                    [-42., -8., 3.],
                    [-42., -8., 4.],
                    [-75., -10., 1.],
                    [-75., -10., 2.],
                ],
            ),
        ],
        Control::Gear => vec![
            (&[0x5afe, 0x5b1d], &[[6., 1., -9.], [14., 1., -9.]]),
            (&[0x5c4e, 0x5c6d], &[[-6., 1., -9.], [-14., 1., -9.]]),
            (&[0x5d9e, 0x5dbd], &[[-2., 41., -9.], [2., 41., -9.]]),
            (&[0x5ced, 0x5d04], &[[-1., 51., -9.], [-1., 51., -11.]]),
            (&[0x5a4d, 0x5a64], &[[6., 6., -10.], [6., -9., -10.]]),
            (&[0x5b9d, 0x5bb4], &[[-6., -10., -10.], [-6., 5., -10.]]),
        ],
        Control::Brake => vec![
            (
                &[0x6254, 0x626b, 0x6282, 0x6299],
                &[[75., -8., 1.], [77., -8., 1.]],
            ),
            (
                &[0x6226, 0x623d, 0x62b0, 0x62c7],
                &[[-75., -8., 1.], [-76., -8., 1.]],
            ),
        ],
        _ => Vec::new(),
    }
}
pub(super) fn check(
    s: &Sources,
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
        m.reviewed_wheel_failed = pose.iter().any(|f| {
            [
                0x6226, 0x623d, 0x6254, 0x626b, 0x6282, 0x6299, 0x62b0, 0x62c7,
            ]
            .contains(&f.address)
        });
        return;
    }
    materials(s, c, value, pose, scale, m);
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
    let signed: Vec<(&[usize], [f32; 3], f32)> = match c {
        Control::Elevator => vec![
            (&[0x56a9, 0x56d6], [25., -61., 3.], 1.),
            (&[0x5810, 0x58b7], [-25., -61., 3.], 1.),
        ],
        Control::Flaps => vec![
            (&[0x5fbf, 0x5fde], [14., -11., 5.], -1.),
            (&[0x60a1, 0x60c0], [-14., -11., 5.], -1.),
        ],
        Control::Aileron => vec![
            (&[0x4b9d, 0x4c10], [75., -13., 1.], 1.),
            (&[0x4f0e, 0x4fb4], [-75., -13., 1.], -1.),
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
    if matches!(c, Control::Elevator) {
        let ids = [0x56a9, 0x56d6, 0x5810, 0x58b7];
        let old: BTreeMap<_, _> = keyed(reference)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        for (key, f) in &old {
            for (i, p) in f.positions.iter().enumerate() {
                if (p[1] + 54.).abs() < 1e-4 {
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
    let fixed: &[usize] = match c {
        Control::Rudder => &[0x572f, 0x58ed],
        Control::Aileron => &[0x4bed, 0x4f5e],
        _ => &[],
    };
    for a in fixed {
        let (Some(before), Some(after)) = (face(reference, *a), face(pose, *a)) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        m.reviewed_neutral_mismatch |= before.positions != after.positions
            || before.uv != after.uv
            || before.texture != after.texture;
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
        if BRACE.contains(&key.0) {
            let ids: Vec<_> = before
                .positions
                .iter()
                .enumerate()
                .filter(|(_, p)| p[1] == 35.)
                .map(|(i, _)| i)
                .collect();
            if ids.len() != 2 {
                m.reviewed_anchor_missing = true;
            } else {
                m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(
                    (distance(after.positions[ids[0]], after.positions[ids[1]]) - 2.).abs() * scale,
                );
            }
            continue;
        }
        count += 1;
        rigid(before, after, scale, m);
        let nose = NOSE.contains(&key.0);
        let left = MAIN_L.contains(&key.0) || DOOR_L.contains(&key.0);
        let panel = DOOR_L.contains(&key.0) || DOOR_R.contains(&key.0);
        let main = MAIN_L.contains(&key.0) || MAIN_R.contains(&key.0);
        if !nose && !panel && !main {
            m.reviewed_wheel_failed = true;
            continue;
        }
        let hub = if nose {
            [0., 41., -9.]
        } else if panel {
            [
                if left { -6. } else { 6. },
                if left { -2.5 } else { -1.5 },
                -10.,
            ]
        } else {
            [if left { -11. } else { 10. }, 1., -9.]
        };
        if let Some(p) = virtual_point(before, after, hub) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, hub) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
        if !nose {
            let bound = if panel { 2. } else { 6. };
            m.reviewed_wheel_failed |= after
                .positions
                .iter()
                .any(|p| p[0] * hub[0].signum() < bound - 1e-4);
        }
        if travel < 1e-5 {
            let (min, max) = if nose {
                ([-2., 27., -11.], [2., 41., -4.])
            } else if panel && left {
                ([-6., -10., -10.], [-2., 5., -10.])
            } else if panel {
                ([2., -9., -10.], [6., 6., -10.])
            } else if left {
                ([-14., -12., -10.], [-6., 1., -1.])
            } else {
                ([6., -12., -10.], [14., 1., -1.])
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
    sources: &Sources,
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut poses = Vec::new();
    let mut rows = String::from(
        "flaps,roll,anchor_gap_ft,skin_gap_ft,material_error_px,crossings,direction_failures\n",
    );
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut s = neutral.clone();
            s.flaps = flap;
            s.aileron = roll;
            let pose = airframe.animation_faces(&s);
            let mut m = measure(&original, &pose, scale);
            check(
                sources,
                Control::Flaps,
                flap,
                (&original, &original, &pose),
                scale,
                &mut m,
            );
            check(
                sources,
                Control::Aileron,
                roll,
                (&original, &original, &pose),
                scale,
                &mut m,
            );
            let error = materials(sources, Control::Flaps, flap, &pose, scale, &mut m);
            writeln!(
                rows,
                "{flap},{roll},{},{},{error},{},{}",
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
                    "Su25 flap{flap}/roll{roll} source position/material or own hinge"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    let mut rows = String::from("rudder,material_error_px,source_correspondence_passed\n");
    for value in [-1., -0.5, 0., 0.5, 1.] {
        let mut s = neutral.clone();
        s.rudder = value;
        let pose = airframe.animation_faces(&s);
        let mut m = Metrics::default();
        let error = materials(sources, Control::Rudder, value, &pose, scale, &mut m);
        let passed = !m.reviewed_anchor_missing && !m.reviewed_neutral_mismatch;
        writeln!(rows, "{value},{error},{passed}")?;
        if !passed {
            failures.push(format!(
                "Su25 rudder{value} source position/UV/material correspondence"
            ));
        }
    }
    fs::write(out.join("rudder-material-correspondence.csv"), rows)?;
    Ok(failures)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn shape(faces: Vec<Face>) -> Shape {
        Shape {
            faces,
            lines: Vec::new(),
            state_words: Default::default(),
        }
    }
    #[test]
    fn source_witness_rejects_collapsed_deflected_uv_and_swapped_skin_material() {
        let mut base = super::super::tests::face(
            0x5e68,
            vec![
                [0., -62., 7.],
                [0., -53., 7.],
                [0., -60., 27.],
                [0., -65., 27.],
            ],
        );
        base.uv = vec![[10., 4.], [10., 3.], [10., 2.], [10., 1.]];
        let mut other = base.clone();
        other.address = 0x5e8f;
        other.positions.reverse();
        other.uv.reverse();
        let mut target = super::super::tests::face(
            0x5dfb,
            vec![
                [0., -53., 7.],
                [0., -60., 27.],
                [2., -63., 27.],
                [5., -60., 7.],
            ],
        );
        target.uv = vec![[10., 3.], [10., 2.], [12., 1.], [13., 4.]];
        let mut target_other = target.clone();
        target_other.address = 0x5e22;
        target_other.positions.reverse();
        target_other.uv.reverse();
        let sources = Sources {
            neutral: shape(vec![base.clone(), other.clone()]),
            positive: shape(vec![target, target_other]),
            negative: shape(Vec::new()),
            left: shape(Vec::new()),
            right: shape(Vec::new()),
        };
        let mut posed = base.clone();
        posed.positions = vec![
            [5., -60., 7.],
            [0., -53., 7.],
            [0., -60., 27.],
            [2., -63., 27.],
        ];
        let mut posed_other = other.clone();
        posed_other.positions = posed.positions.iter().rev().copied().collect();
        let mut metric = Metrics::default();
        assert!(
            materials(
                &sources,
                Control::Rudder,
                1.,
                &[posed.clone(), posed_other.clone()],
                1.,
                &mut metric
            ) > 1.
        );
        assert!(metric.reviewed_neutral_mismatch);
        posed.uv = vec![[13., 4.], [10., 3.], [10., 2.], [12., 1.]];
        posed_other.uv = posed.uv.iter().rev().copied().collect();
        let mut metric = Metrics::default();
        assert_eq!(
            materials(
                &sources,
                Control::Rudder,
                1.,
                &[posed.clone(), posed_other.clone()],
                1.,
                &mut metric
            ),
            0.
        );
        assert!(!metric.reviewed_neutral_mismatch);
        assert!(!metric.reviewed_anchor_missing);
        posed_other.texture = "WRONG-SKIN".into();
        let mut metric = Metrics::default();
        materials(
            &sources,
            Control::Rudder,
            1.,
            &[posed, posed_other],
            1.,
            &mut metric,
        );
        assert!(metric.reviewed_neutral_mismatch);
    }
}
