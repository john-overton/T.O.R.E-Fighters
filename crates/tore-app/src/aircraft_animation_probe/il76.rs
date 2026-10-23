//! Independent IL76 source endpoints, fitted pivots and independent-control witnesses.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 8] = [
    0x4901, 0x4920, 0x497d, 0x499c, 0x49f9, 0x4a18, 0x4a37, 0x4a56,
];
const RIGHT: [usize; 8] = [
    0x48c3, 0x48e2, 0x493f, 0x495e, 0x4a75, 0x4a94, 0x4ab3, 0x4ad2,
];
const NOSE: [usize; 4] = [0x4885, 0x48a4, 0x49bb, 0x49da];
const FLAPS: [usize; 4] = [0x4e58, 0x4e77, 0x4d71, 0x4d90];
pub(super) struct Sources {
    gear: Vec<Face>,
    flaps: BTreeMap<usize, (Face, [usize; 4])>,
    closures: Vec<(Face, Vec<[f32; 3]>)>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let neutral = Shape::parse(bytes)?;
        let gear: Vec<_> = Shape::with_state(bytes, &[(0x7fc6, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect();
        if gear.len() != 20 {
            return Err("IL76 probe missing gear faces".into());
        }
        let mut flaps = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, pairs, ends) in [
            (
                0x7fcc,
                [
                    (0x4e58, 0x4ee0, [1, 2, 3, 0]),
                    (0x4e77, 0x4ec1, [1, 2, 3, 0]),
                ],
                [0x4eff],
            ),
            (
                0x7fd2,
                [
                    (0x4d71, 0x4df9, [3, 0, 1, 2]),
                    (0x4d90, 0x4dda, [3, 0, 1, 2]),
                ],
                [0x4e18],
            ),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let mut mapping = BTreeMap::new();
            for (a, b, order) in pairs {
                let base = one(&neutral.faces, a)?;
                let target = one(&down.faces, b)?.clone();
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("IL76 probe flap topology changed".into());
                }
                for (i, &j) in order.iter().enumerate() {
                    mapping.insert(target.positions[j].map(f32::to_bits), base.positions[i]);
                }
                flaps.insert(a, (target, order));
            }
            for a in ends {
                let f = one(&down.faces, a)?.clone();
                let n = f
                    .positions
                    .iter()
                    .map(|p| {
                        mapping
                            .get(&p.map(f32::to_bits))
                            .copied()
                            .ok_or("IL76 probe closure lacks neutral correspondence")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                closures.push((f, n));
            }
        }
        Ok(Self {
            gear,
            flaps,
            closures,
        })
    }
}
fn one(faces: &[Face], a: usize) -> AppResult<&Face> {
    faces
        .iter()
        .find(|f| f.address == a)
        .ok_or_else(|| format!("IL76 probe missing source face {a:x}").into())
}
struct Panel {
    faces: &'static [usize],
    roots: Vec<[f32; 3]>,
    trailing: Vec<[f32; 3]>,
    sign: f32,
}
fn panels(c: Control) -> Vec<Panel> {
    match c {
        Control::Rudder => vec![Panel {
            faces: &[0x4fa3, 0x4fca],
            roots: vec![[0., -96., 20.], [0., -111., 57.]],
            trailing: vec![[0., -110., 20.], [0., -125., 57.]],
            sign: 1.,
        }],
        Control::Elevator => vec![
            Panel {
                faces: &[0x46fb, 0x4774],
                roots: vec![[-1., -99., 58.], [-1., -120., 58.], [-35., -122.9, 60.]],
                trailing: vec![[-35., -129., 60.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x45b5, 0x463a],
                roots: vec![[2., -99., 58.], [2., -120., 58.], [35., -122.9, 60.]],
                trailing: vec![[35., -129., 60.]],
                sign: 1.,
            },
        ],
        Control::Aileron => [-1., 1.]
            .into_iter()
            .map(|side| Panel {
                faces: if side < 0. {
                    &[0x409d, 0x40b4]
                } else {
                    &[0x38eb, 0x38ff]
                },
                roots: vec![
                    [side * 88., -34., 3.],
                    [side * 88., -34., 5.],
                    [side * 123., -46., 1.],
                ],
                trailing: vec![[side * 86., -36., 4.]],
                sign: side,
            })
            .collect(),
        Control::Flaps => [-1., 1.]
            .into_iter()
            .map(|side| Panel {
                faces: if side < 0. {
                    &[0x4e58, 0x4e77]
                } else {
                    &[0x4d71, 0x4d90]
                },
                roots: vec![
                    [side * 41., -21., 4.],
                    [side * 41., -21., 8.],
                    [side * 88., -34., 3.],
                    [side * 88., -34., 5.],
                ],
                trailing: vec![[side * 38., -24., 7.], [side * 86., -36., 4.]],
                sign: -1.,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn selected<'a>(faces: &'a [Face], addresses: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(key, _)| addresses.contains(&key.0))
        .collect()
}
fn witnesses(
    before: &BTreeMap<FaceKey, &Face>,
    after: &BTreeMap<FaceKey, &Face>,
    point: [f32; 3],
) -> Vec<[f32; 3]> {
    before
        .iter()
        .flat_map(|(key, f)| {
            f.positions.iter().enumerate().filter_map(|(i, p)| {
                (distance(*p, point) <= EPSILON)
                    .then(|| after.get(key).and_then(|g| g.positions.get(i)).copied())
                    .flatten()
            })
        })
        .collect()
}
fn attachments(
    reference: &[Face],
    pose: &[Face],
    control: Control,
    scale: f32,
    metric: &mut Metrics,
) {
    for panel in panels(control) {
        let before = selected(reference, panel.faces);
        let after = selected(pose, panel.faces);
        metric.reviewed_anchor_missing |= before.is_empty()
            || before.iter().any(|(k, f)| {
                after
                    .get(k)
                    .is_none_or(|g| g.positions.len() != f.positions.len())
            });
        for p in panel.roots {
            let actual = witnesses(&before, &after, p);
            metric.reviewed_anchor_missing |= actual.is_empty();
            for q in actual {
                metric.max_reviewed_anchor_gap =
                    metric.max_reviewed_anchor_gap.max(distance(p, q) * scale);
            }
        }
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
    }
}
pub(super) fn check(
    control: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    source: &Sources,
    metric: &mut Metrics,
) {
    let (raw, reference, pose) = geometry;
    if matches!(control, Control::Gear) && value == 0. {
        metric.reviewed_wheel_failed |= pose.iter().any(|f| {
            LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
        });
        return;
    }
    attachments(reference, pose, control, scale, metric);
    for (side, panel) in panels(control).iter().enumerate() {
        let before = selected(reference, panel.faces);
        let after = selected(pose, panel.faces);
        if value == 0. {
            for f in raw.iter().filter(|f| panel.faces.contains(&f.address)) {
                for p in &f.positions {
                    metric.reviewed_neutral_mismatch |=
                        !pose.iter().filter(|g| g.address == f.address).any(|g| {
                            g.positions
                                .iter()
                                .any(|q| distance(*p, *q) * scale <= EPSILON)
                        });
                }
            }
        } else {
            let axis = if matches!(control, Control::Rudder) {
                0
            } else {
                2
            };
            for p in &panel.trailing {
                let actual = witnesses(&before, &after, *p);
                metric.reviewed_direction_failures += usize::from(actual.is_empty());
                for q in actual {
                    let d = (q[axis] - p[axis]) * scale;
                    metric.reviewed_direction_failures +=
                        usize::from(d * panel.sign * value.signum() as f32 <= EPSILON);
                    if axis == 2 && side < 2 {
                        metric.reviewed_control_z_delta[side] = d;
                    }
                }
            }
        }
    }
    if matches!(control, Control::Flaps) {
        closures(source, reference, pose, value != 0., scale, metric);
    }
    if matches!(control, Control::Flaps) && value == 1. {
        endpoints(source, pose, scale, metric);
    }
    if matches!(control, Control::Gear) {
        if value == 1. {
            for f in &source.gear {
                for p in &f.positions {
                    metric.reviewed_neutral_mismatch |=
                        !pose.iter().filter(|g| g.address == f.address).any(|g| {
                            g.positions
                                .iter()
                                .any(|q| distance(*p, *q) * scale <= EPSILON)
                        });
                }
            }
        }
        wheels(reference, pose, value, scale, metric);
    }
}
fn endpoints(source: &Sources, pose: &[Face], scale: f32, metric: &mut Metrics) {
    for (&a, (down, order)) in &source.flaps {
        let Ok(actual) = one(pose, a) else {
            metric.reviewed_direction_failures += 1;
            continue;
        };
        if actual.positions.len() != 4 {
            metric.reviewed_direction_failures += 1;
            continue;
        }
        for (i, &j) in order.iter().enumerate() {
            metric.reviewed_direction_failures +=
                usize::from(distance(actual.positions[i], down.positions[j]) * scale > EPSILON);
        }
    }
    for (f, _) in &source.closures {
        let Ok(actual) = one(pose, f.address) else {
            metric.reviewed_direction_failures += 1;
            continue;
        };
        metric.reviewed_direction_failures +=
            usize::from(actual.positions.len() != f.positions.len());
        for (p, q) in actual.positions.iter().zip(&f.positions) {
            metric.reviewed_direction_failures += usize::from(distance(*p, *q) * scale > EPSILON);
        }
    }
}
fn closures(
    source: &Sources,
    reference: &[Face],
    pose: &[Face],
    active: bool,
    scale: f32,
    metric: &mut Metrics,
) {
    let before = selected(reference, &FLAPS);
    let after = selected(pose, &FLAPS);
    for (f, neutral) in &source.closures {
        let actual = pose.iter().find(|g| g.address == f.address);
        if !active {
            metric.reviewed_anchor_missing |= actual.is_some();
            continue;
        }
        let Some(actual) = actual else {
            metric.reviewed_anchor_missing = true;
            continue;
        };
        if actual.positions.len() != neutral.len() {
            metric.reviewed_anchor_missing = true;
            continue;
        }
        for (p, q) in neutral.iter().zip(&actual.positions) {
            let matches = witnesses(&before, &after, *p);
            metric.reviewed_anchor_missing |= matches.is_empty();
            for expected in matches {
                metric.max_reviewed_skin_gap = metric
                    .max_reviewed_skin_gap
                    .max(distance(expected, *q) * scale);
            }
        }
    }
}
// Reconstruct fitted pivots affinely from independent source-plane coordinates.
// This verifies their fixed location without importing the production rotation.
fn affine_anchor(
    reference: &[Face],
    pose: &[Face],
    address: usize,
    pivot: [f32; 3],
) -> Option<[f32; 3]> {
    let f = reference.iter().find(|f| f.address == address)?;
    let g = pose.iter().find(|f| f.address == address)?;
    if f.positions.len() < 3 || g.positions.len() != f.positions.len() {
        return None;
    }
    let sub = |a: [f32; 3], b: [f32; 3]| std::array::from_fn::<_, 3, _>(|i| a[i] - b[i]);
    let dot = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
    let u = sub(f.positions[1], f.positions[0]);
    let v = sub(f.positions[2], f.positions[0]);
    let p = sub(pivot, f.positions[0]);
    let uu = dot(u, u);
    let vv = dot(v, v);
    let uv = dot(u, v);
    let d = uu * vv - uv * uv;
    if d.abs() < 1e-6 {
        return None;
    }
    let b = (dot(p, u) * vv - dot(p, v) * uv) / d;
    let c = (dot(p, v) * uu - dot(p, u) * uv) / d;
    Some(std::array::from_fn(|i| {
        g.positions[0][i]
            + b * (g.positions[1][i] - g.positions[0][i])
            + c * (g.positions[2][i] - g.positions[0][i])
    }))
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    for (a, pivot) in [
        (0x49f9, [-7.5, -19., -17.]),
        (0x4a37, [-7.5, -2., -17.]),
        (0x4a75, [7.5, -19., -17.]),
        (0x4ab3, [7.5, -2., -17.]),
        (0x49bb, [0., 45., -15.]),
    ] {
        if let Some(actual) = affine_anchor(reference, pose, a, pivot) {
            metric.max_reviewed_anchor_gap = metric
                .max_reviewed_anchor_gap
                .max(distance(pivot, actual) * scale);
        } else {
            metric.reviewed_anchor_missing = true;
        }
    }
    let after = keyed(pose);
    let mut counts = [0; 3];
    let mut left = f32::NEG_INFINITY;
    let mut right = f32::INFINITY;
    for (key, before) in keyed(reference) {
        let side = if LEFT.contains(&key.0) {
            0
        } else if RIGHT.contains(&key.0) {
            1
        } else if NOSE.contains(&key.0) {
            2
        } else {
            continue;
        };
        counts[side] += 1;
        let Some(actual) = after.get(&key) else {
            metric.reviewed_wheel_failed = true;
            continue;
        };
        if before.positions.len() != actual.positions.len() {
            metric.reviewed_wheel_failed = true;
            continue;
        }
        for (i, p) in before.positions.iter().enumerate() {
            let q = actual.positions[i];
            if p[2] <= -20. {
                if side == 0 {
                    left = left.max(q[0] * scale);
                } else if side == 1 {
                    right = right.min(q[0] * scale);
                }
            }
            for (j, r) in before.positions.iter().enumerate().skip(i + 1) {
                metric.reviewed_wheel_rigidity_error = metric
                    .reviewed_wheel_rigidity_error
                    .max((distance(*p, *r) - distance(q, actual.positions[j])).abs() * scale);
            }
            if travel < 1e-5 {
                let (lo, hi) = match side {
                    0 => ([-9., -24., -16.], [-4., 4., -9.]),
                    1 => ([3., -24., -16.], [9., 4., -9.]),
                    _ => ([-3., 39., -15.], [3., 51., -5.]),
                };
                metric.reviewed_wheel_failed |=
                    (0..3).any(|k| q[k] < lo[k] - 1e-3 || q[k] > hi[k] + 1e-3);
            }
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    // Conservative complete lower-card bounds, independently checked against source pods.
    metric.reviewed_wheel_failed |= counts != [8, 8, 4]
        || right - left < 4.66
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    source: &Sources,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut rows = String::from(
        "flap,roll,finite,anchor_gap_ft,skin_gap_ft,direction_failures,new_planar_crossings\n",
    );
    let mut failures = Vec::new();
    let mut poses = Vec::new();
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.flaps = flap;
            state.aileron = roll;
            let actual = airframe.animation_faces(&state);
            let mut metric = measure(&original, &actual, scale);
            check(
                Control::Flaps,
                flap,
                (&original, &original, &actual),
                scale,
                source,
                &mut metric,
            );
            check(
                Control::Aileron,
                roll,
                (&original, &original, &actual),
                scale,
                source,
                &mut metric,
            );
            writeln!(
                rows,
                "{flap},{roll},{},{},{},{},{}",
                metric.finite,
                metric.max_reviewed_anchor_gap,
                metric.max_reviewed_skin_gap,
                metric.reviewed_direction_failures,
                metric.new_planar_crossings.len()
            )?;
            if !metric.finite
                || metric.reviewed_anchor_missing
                || metric.max_reviewed_anchor_gap > EPSILON
                || metric.max_reviewed_skin_gap > EPSILON
                || metric.reviewed_direction_failures > 0
                || metric.reviewed_neutral_mismatch
                || !metric.new_planar_crossings.is_empty()
            {
                failures.push(format!("IL76 independent flap{flap}/roll{roll} witness"));
            }
            poses.push(actual);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
