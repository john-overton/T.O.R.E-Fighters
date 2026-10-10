//! Independent MiG23 source materials, observed wing transforms and rooted controls.
use super::*;
use tore_formats::shape::Shape;
const FLAPS: [usize; 4] = [0x3e7e, 0x3ea5, 0x4175, 0x4194];
const TAIL: [usize; 4] = [0x3543, 0x356b, 0x3730, 0x3758];
const BRACE: [usize; 2] = [0x474f, 0x4766];
const NOSE: [usize; 4] = [0x47c2, 0x47e9, 0x4810, 0x4837];
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
            left: Shape::with_state(bytes, &[(0x6af2, -1)].into())?,
            right: Shape::with_state(bytes, &[(0x6af8, -1)].into())?,
            positive: Shape::with_state(bytes, &[(0x6afe, 1)].into())?,
            negative: Shape::with_state(bytes, &[(0x6afe, -1)].into())?,
        })
    }
}
fn face(faces: &[Face], a: usize) -> Option<&Face> {
    let mut fs = faces.iter().filter(|f| f.address == a);
    let f = fs.next()?;
    fs.next().is_none().then_some(f)
}
fn wing_point(s: &Sources, pose: &[Face], left: bool, p: [f32; 3]) -> Option<[f32; 3]> {
    let a = if left { 0x3fae } else { 0x3cc6 };
    virtual_point(face(&s.neutral.faces, a)?, face(pose, a)?, p)
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
            (0x3e7e, 0x3ee3, [1, 2, 3, 0], &s.right),
            (0x3ea5, 0x3f0a, [3, 0, 1, 2], &s.right),
            (0x4175, 0x41da, [2, 3, 0, 1], &s.left),
            (0x4194, 0x41f9, [0, 1, 2, 3], &s.left),
        ],
        Control::Rudder if value >= 0. => vec![
            (0x42f9, 0x423f, [2, 3, 0, 1], &s.positive),
            (0x4318, 0x425e, [2, 3, 0, 1], &s.positive),
        ],
        Control::Rudder => vec![
            (0x42f9, 0x429c, [1, 0, 3, 2], &s.negative),
            (0x4318, 0x42bb, [1, 0, 3, 2], &s.negative),
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
            let local: [f32; 3] = std::array::from_fn(|k| {
                base.positions[i][k] * (1. - t) + endpoint.positions[j][k] * t
            });
            let expected = if matches!(c, Control::Flaps) {
                wing_point(s, pose, a >= 0x4175, local)
            } else {
                Some(local)
            };
            if let Some(p) = expected {
                m.reviewed_neutral_mismatch |= distance(p, actual.positions[i]) * scale > EPSILON;
            } else {
                m.reviewed_anchor_missing = true;
            }
            for k in 0..2 {
                let uv = if matches!(c, Control::Rudder) {
                    if value == 0. {
                        base.uv[i][k]
                    } else {
                        endpoint.uv[j][k]
                    }
                } else {
                    base.uv[i][k] * (1. - t) + endpoint.uv[j][k] * t
                };
                error = error.max((uv - actual.uv[i][k]).abs());
            }
            m.reviewed_neutral_mismatch |= actual.colors.get(i) != endpoint.colors.get(j);
        }
    }
    m.reviewed_neutral_mismatch |= error > 1e-4;
    error
}
fn brake_parts(faces: &[Face]) -> Vec<Face> {
    faces
        .iter()
        .filter(|f| {
            [0x3109, 0x313c, 0x31a7, 0x339f, 0x33f8, 0x347a].contains(&f.address)
                && f.positions.iter().any(|p| (p[1] + 16.).abs() < 1e-4)
                && f.positions.iter().all(|p| p[1] <= -16. + 1e-4)
        })
        .cloned()
        .collect()
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
    materials(s, c, value, pose, scale, m);
    if matches!(c, Control::Elevator | Control::Aileron) {
        m.reviewed_anchor_missing |= reference
            .iter()
            .filter(|f| TAIL.contains(&f.address))
            .count()
            != 4
            || pose.iter().filter(|f| TAIL.contains(&f.address)).count() != 4;
        for (ids, side) in [(&[0x3543, 0x356b][..], 1.), (&[0x3730, 0x3758][..], -1.)] {
            let old: BTreeMap<_, _> = keyed(reference)
                .into_iter()
                .filter(|(k, _)| ids.contains(&k.0))
                .collect();
            let new: BTreeMap<_, _> = keyed(pose)
                .into_iter()
                .filter(|(k, _)| ids.contains(&k.0))
                .collect();
            for p in [
                [side * 3., -30., 1.],
                [side * 3., -26., 1.],
                [side * 4., -16., 1.],
            ] {
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
                let p = [side * 10., -33., 1.];
                let sign = if matches!(c, Control::Elevator) {
                    1.
                } else {
                    side
                };
                let qs = f4::witness_positions(&old, &new, p);
                m.reviewed_direction_failures += usize::from(qs.is_empty());
                for q in qs {
                    m.reviewed_direction_failures +=
                        usize::from((q[2] - p[2]) * sign * value.signum() as f32 <= EPSILON);
                }
            }
        }
    }
    if matches!(c, Control::Rudder) {
        for a in [0x42f9, 0x4318] {
            let (Some(before), Some(after)) = (face(reference, a), face(pose, a)) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            for (i, p) in before.positions.iter().enumerate() {
                if p[1] != -29. {
                    m.max_reviewed_anchor_gap = m
                        .max_reviewed_anchor_gap
                        .max(distance(*p, after.positions[i]) * scale);
                } else if value != 0. {
                    m.reviewed_direction_failures +=
                        usize::from(after.positions[i][0] * value.signum() as f32 <= EPSILON);
                }
            }
        }
        for a in [0x2a66, 0x2a89, 0x2b20] {
            let (Some(a), Some(b)) = (face(reference, a), face(pose, a)) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            m.reviewed_neutral_mismatch |= a.positions != b.positions || a.uv != b.uv;
        }
    }
    if matches!(c, Control::Flaps) {
        for a in FLAPS {
            let (Some(base), Some(actual)) = (face(&s.neutral.faces, a), face(pose, a)) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            for (i, p) in base.positions.iter().enumerate() {
                if p[1] != -6. {
                    if let Some(q) = wing_point(s, pose, a >= 0x4175, *p) {
                        m.max_reviewed_anchor_gap = m
                            .max_reviewed_anchor_gap
                            .max(distance(q, actual.positions[i]) * scale);
                    } else {
                        m.reviewed_anchor_missing = true;
                    }
                } else if value != 0. {
                    m.reviewed_direction_failures +=
                        usize::from(actual.positions[i][2] >= p[2] - EPSILON);
                }
            }
        }
    }
    if matches!(c, Control::Flaps | Control::Rudder) {
        let definitions: &[&[usize]] = if matches!(c, Control::Flaps) {
            &[&[0x3e7e, 0x3ea5], &[0x4175, 0x4194]]
        } else {
            &[&[0x42f9, 0x4318]]
        };
        for ids in definitions {
            let old: BTreeMap<_, _> = keyed(reference)
                .into_iter()
                .filter(|(k, _)| ids.contains(&k.0))
                .collect();
            let new: BTreeMap<_, _> = keyed(pose)
                .into_iter()
                .filter(|(k, _)| ids.contains(&k.0))
                .collect();
            if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
                m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
            }
        }
    }
    if matches!(c, Control::Sweep) {
        wings(s, pose, scale, m);
    }
    if matches!(c, Control::Brake) {
        let a = brake_parts(reference);
        let b = brake_parts(pose);
        let old = keyed(&a);
        let new = keyed(&b);
        m.reviewed_anchor_missing |= a.len() != 6 || b.len() != 6;
        for (key, f) in &old {
            for (i, p) in f.positions.iter().enumerate() {
                if (p[1] + 16.).abs() < 1e-4 {
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
    if matches!(c, Control::Gear) {
        wheels(reference, pose, value, scale, m);
    }
}
const GEAR: [usize; 18] = [
    0x474f, 0x4766, 0x4610, 0x4633, 0x4656, 0x4675, 0x4694, 0x46b3, 0x47c2, 0x47e9, 0x4810, 0x4837,
    0x44eb, 0x450e, 0x4531, 0x4550, 0x456f, 0x458e,
];
const WING: [usize; 25] = [
    0x3c9f, 0x3cc6, 0x3ce7, 0x3d08, 0x3e7e, 0x3ea5, 0x3d86, 0x3dad, 0x3dce, 0x3def, 0x3e1c, 0x3e3d,
    0x3f87, 0x3fae, 0x3fcf, 0x3ffc, 0x401d, 0x403f, 0x4066, 0x4087, 0x40a8, 0x40cf, 0x40e4, 0x4175,
    0x4194,
];
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
fn wings(s: &Sources, pose: &[Face], scale: f32, m: &mut Metrics) {
    for a in WING {
        if FLAPS.contains(&a) {
            continue;
        }
        let (Some(before), Some(after)) = (face(&s.neutral.faces, a), face(pose, a)) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        rigid(before, after, scale, m);
        let left = before.positions.iter().map(|p| p[0]).sum::<f32>() < 0.;
        let hub = [if left { -9. } else { 9. }, 4., 3.];
        if let Some(p) = virtual_point(before, after, hub) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, hub) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
        if let Some((old, new)) = observed_frames(before, after) {
            let x = map_vector([1., 0., 0.], old, new);
            let angle = x[1].atan2(x[0]);
            m.reviewed_direction_failures += usize::from(
                angle.abs() > 40f64.to_radians() + 1e-5
                    || if left { angle < -1e-5 } else { angle > 1e-5 },
            );
        } else {
            m.reviewed_anchor_missing = true;
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
            for (i, p) in before.positions.iter().enumerate() {
                if p[1] == 29. {
                    m.max_reviewed_anchor_gap = m
                        .max_reviewed_anchor_gap
                        .max(distance(*p, after.positions[i]) * scale);
                }
            }
            let ids: Vec<_> = before
                .positions
                .iter()
                .enumerate()
                .filter(|(_, p)| p[1] == 20.)
                .map(|(i, _)| i)
                .collect();
            if ids.len() != 2 {
                m.reviewed_anchor_missing = true;
            } else {
                m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(
                    (distance(after.positions[ids[0]], after.positions[ids[1]]) - 5f32.sqrt())
                        .abs()
                        * scale,
                );
            }
            continue;
        }
        count += 1;
        rigid(before, after, scale, m);
        let nose = NOSE.contains(&key.0);
        let left = key.0 >= 0x4610 && key.0 <= 0x46b3;
        let hub = if nose {
            [0., 27., -3.]
        } else {
            [if left { -3. } else { 3. }, -1., -3.]
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
                .any(|p| p[0] * hub[0].signum() < 2. - 1e-4);
        }
        if travel < 1e-5 {
            let (min, max) = if nose {
                ([-1., 20., -4.], [1., 27., -2.])
            } else if left {
                ([-9., -8., -5.], [-2., -1., 0.])
            } else {
                ([2., -8., -6.], [9., -1., -1.])
            };
            m.reviewed_wheel_failed |= after
                .positions
                .iter()
                .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-3 || p[i] > max[i] + 1e-3));
        }
    }
    m.reviewed_wheel_failed |= count != 16 || m.reviewed_rigid_panel_error > EPSILON;
}
fn combined_failed(m: &Metrics) -> bool {
    !m.finite
        || m.reviewed_anchor_missing
        || m.max_reviewed_anchor_gap > EPSILON
        || m.max_reviewed_skin_gap > EPSILON
        || m.reviewed_rigid_panel_error > EPSILON
        || !m.new_planar_crossings.is_empty()
        || m.reviewed_direction_failures != 0
        || m.reviewed_neutral_mismatch
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
        "knots,flaps,roll,anchor_gap_ft,skin_gap_ft,material_error_px,crossings,direction_failures\n",
    );
    for knots in [400., 475., 550., 625., 700.] {
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                let mut s = neutral.clone();
                s.speed = knots * 1.68781;
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
                wings(sources, &pose, scale, &mut m);
                let error = materials(sources, Control::Flaps, flap, &pose, scale, &mut m);
                writeln!(
                    rows,
                    "{knots},{flap},{roll},{},{},{error},{},{}",
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
                if combined_failed(&m) {
                    failures.push(format!("MiG23 sweep{knots}/flap{flap}/roll{roll} source material, wing attachment or direction"));
                }
                poses.push(pose);
            }
        }
    }
    fs::write(out.join("sweep-flap-roll-combinations.csv"), rows)?;
    contact_sheet(
        &out.join("sweep-flap-roll-combinations.ppm"),
        &original,
        &poses,
    )?;
    let mut rows = String::from("rudder,material_error_px,source_correspondence_passed\n");
    for value in [-1., -0.5, 0., 1e-6, 0.5, 1.] {
        let mut s = neutral.clone();
        s.rudder = value;
        let pose = airframe.animation_faces(&s);
        let mut m = Metrics::default();
        let error = materials(sources, Control::Rudder, value, &pose, scale, &mut m);
        let passed = finite(&pose) && !m.reviewed_anchor_missing && !m.reviewed_neutral_mismatch;
        writeln!(rows, "{value},{error},{passed}")?;
        if !passed {
            failures.push(format!(
                "MiG23 rudder{value} sign-selected source UV/material"
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
            billboards: Vec::new(),
            faces,
            lines: Vec::new(),
            state_words: Default::default(),
        }
    }
    #[test]
    fn sign_selected_uv_witness_rejects_intermediate_reflection_lerp_and_retains_neutral() {
        let mut base = super::super::tests::face(
            0x42f9,
            vec![
                [0., -29., 5.],
                [0., -24., 5.],
                [0., -27., 13.],
                [0., -29., 13.],
            ],
        );
        base.uv = vec![[10., 0.], [10., 1.], [20., 1.], [20., 0.]];
        let mut other = base.clone();
        other.address = 0x4318;
        other.positions.reverse();
        other.uv.reverse();
        let mut target = super::super::tests::face(
            0x423f,
            vec![
                [0., -27., 13.],
                [1., -28., 13.],
                [3., -28., 5.],
                [0., -24., 5.],
            ],
        );
        target.uv = vec![[10., 1.], [10., 0.], [20., 0.], [20., 1.]];
        let mut target_other = super::super::tests::face(
            0x425e,
            vec![
                [0., -24., 5.],
                [3., -28., 5.],
                [1., -28., 13.],
                [0., -27., 13.],
            ],
        );
        target_other.uv = vec![[20., 1.], [20., 0.], [10., 0.], [10., 1.]];
        let sources = Sources {
            neutral: shape(vec![base.clone(), other.clone()]),
            positive: shape(vec![target, target_other]),
            negative: shape(Vec::new()),
            left: shape(Vec::new()),
            right: shape(Vec::new()),
        };
        let mut m = Metrics::default();
        assert_eq!(
            materials(
                &sources,
                Control::Rudder,
                0.,
                &[base.clone(), other.clone()],
                1.,
                &mut m
            ),
            0.
        );
        assert!(!m.reviewed_neutral_mismatch);
        let mut actual = base.clone();
        actual.positions = vec![
            [1.5, -28.5, 5.],
            [0., -24., 5.],
            [0., -27., 13.],
            [0.5, -28.5, 13.],
        ];
        actual.uv = vec![[15., 0.], [15., 1.], [15., 1.], [15., 0.]];
        let mut actual_other = other.clone();
        actual_other.positions = actual.positions.iter().rev().copied().collect();
        actual_other.uv = actual.uv.iter().rev().copied().collect();
        let mut m = Metrics::default();
        assert!(
            materials(
                &sources,
                Control::Rudder,
                0.5,
                &[actual.clone(), actual_other.clone()],
                1.,
                &mut m
            ) > 4.
        );
        assert!(m.reviewed_neutral_mismatch);
        actual.uv = vec![[20., 0.], [20., 1.], [10., 1.], [10., 0.]];
        actual_other.uv = actual.uv.iter().rev().copied().collect();
        let mut m = Metrics::default();
        assert_eq!(
            materials(
                &sources,
                Control::Rudder,
                0.5,
                &[actual, actual_other],
                1.,
                &mut m
            ),
            0.
        );
        assert!(!m.reviewed_neutral_mismatch);
        assert!(!m.reviewed_anchor_missing);
    }
    #[test]
    fn combined_gate_rejects_nonfinite_geometry_even_with_zero_reviewed_gaps() {
        let reference = vec![super::super::tests::face(
            1,
            vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
        )];
        assert!(!combined_failed(&measure(&reference, &reference, 1.)));
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut pose = reference.clone();
            pose[0].positions[1][2] = invalid;
            let m = measure(&reference, &pose, 1.);
            assert!(!m.finite);
            // NaN distances need not increase these maxima. The finite flag
            // itself must make the coupled pose fail.
            assert_eq!(m.max_reviewed_anchor_gap, 0.);
            assert_eq!(m.max_reviewed_skin_gap, 0.);
            assert!(combined_failed(&m));
        }
        let mut pose = reference.clone();
        pose[0].normal = Some([f32::NAN, 0., 1.]);
        assert!(combined_failed(&measure(&reference, &pose, 1.)));
    }
}
