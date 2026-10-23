//! Independent AWACS/E3 source endpoints, fitted attachment and mixed-control witnesses.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 4] = [0x4336, 0x435d, 0x4384, 0x43a3];
const RIGHT: [usize; 4] = [0x4265, 0x428c, 0x42b3, 0x42d2];
const NOSE: [usize; 3] = [0x4407, 0x442e, 0x4455];
const FLAPS: [usize; 4] = [0x5003, 0x5022, 0x4eb9, 0x4ed8];
pub(super) struct Sources {
    gear: Vec<Face>,
    flaps: BTreeMap<usize, (Face, [usize; 4])>,
    closures: Vec<(Face, Vec<[f32; 3]>)>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let neutral = Shape::parse(bytes)?;
        let gear: Vec<_> = Shape::with_state(bytes, &[(0x7886, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect();
        if gear.len() != 11 {
            return Err("AWACS probe missing gear faces".into());
        }
        let mut flaps = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, pairs, ends) in [
            (
                0x7892,
                [
                    (0x5003, 0x50ba, [2, 3, 0, 1]),
                    (0x5022, 0x50d9, [1, 2, 3, 0]),
                ],
                [0x50f8, 0x510d],
            ),
            (
                0x7898,
                [
                    (0x4eb9, 0x4f70, [2, 3, 0, 1]),
                    (0x4ed8, 0x4f8f, [3, 0, 1, 2]),
                ],
                [0x4fae, 0x4fc3],
            ),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let mut mapping = BTreeMap::new();
            for (a, b, order) in pairs {
                let base = one(&neutral.faces, a)?;
                let target = one(&down.faces, b)?.clone();
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("AWACS probe flap topology changed".into());
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
                            .ok_or("AWACS probe closure lacks neutral correspondence")
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
        .ok_or_else(|| format!("AWACS probe missing source face {a:x}").into())
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
            faces: &[0x51db, 0x51f2],
            roots: vec![[0., -104., 13.], [0., -109., 38.]],
            trailing: vec![[0., -112., 13.], [0., -117., 38.]],
            sign: 1.,
        }],
        Control::Elevator => vec![
            Panel {
                faces: &[0x2f7a, 0x2f8d],
                roots: vec![[-7., -87., 9.], [-2., -113., 9.], [-38., -116.3, 12.]],
                trailing: vec![[-38., -125., 12.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x2fcc, 0x2fdf],
                roots: vec![[7., -87., 9.], [2., -113., 9.], [39., -116.6, 12.]],
                trailing: vec![[39., -125., 12.]],
                sign: 1.,
            },
        ],
        Control::Flaps | Control::Aileron => [-1., 1.]
            .into_iter()
            .map(|side| Panel {
                faces: if side < 0. {
                    &[0x5003, 0x5022]
                } else {
                    &[0x4eb9, 0x4ed8]
                },
                roots: vec![
                    [side * 48., -19., 1.],
                    [side * 48., -19., -2.],
                    [side * 104., -45., 4.],
                    [side * 104., -45., 3.],
                ],
                trailing: vec![[side * 45., -22., -1.], [side * 101., -48., 4.]],
                sign: if matches!(c, Control::Flaps) {
                    -1.
                } else {
                    side
                },
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
    if matches!(control, Control::Flaps | Control::Aileron) {
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
        (0x4384, [-4.22, -26., -3.94]),
        (0x42b3, [4.525, -26., -3.65]),
        (0x4407, [0., 72., -7.]),
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
            if p[2] <= -10. {
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
                    0 => ([-6.44, -34., -5.88], [-0.44, -18., 10.12]),
                    1 => ([0.05, -34., -6.3], [6.05, -18., 10.7]),
                    _ => ([-2., 67., -7.], [2., 75., 5.]),
                };
                metric.reviewed_wheel_failed |=
                    (0..3).any(|k| q[k] < lo[k] - 1e-3 || q[k] > hi[k] + 1e-3);
            }
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    // This is the conservative raw-card envelope. Independently decoded opaque
    // lower-region atlas pixels have a >=1.11098ft gap through 201 source poses.
    metric.reviewed_wheel_failed |= counts != [4, 4, 3]
        || right - left < 0.30
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
    let original_map = selected(&original, &FLAPS);
    let mut rows = String::from(
        "flap,roll,finite,anchor_gap_ft,skin_gap_ft,direction_failures,new_planar_crossings\n",
    );
    let mut failures = Vec::new();
    let mut poses = Vec::new();
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        let mut state = neutral.clone();
        state.flaps = flap;
        state.aileron = 0.;
        let baseline = airframe.animation_faces(&state);
        let baseline_map = keyed(&baseline);
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            state.aileron = roll;
            let actual = airframe.animation_faces(&state);
            let actual_map = keyed(&actual);
            let mut metric = measure(&original, &actual, scale);
            attachments(&original, &actual, Control::Flaps, scale, &mut metric);
            closures(
                source,
                &original,
                &actual,
                flap != 0. || roll != 0.,
                scale,
                &mut metric,
            );
            if flap == 1. && roll == 0. {
                endpoints(source, &actual, scale, &mut metric);
            }
            if roll != 0. {
                for (key, f) in &original_map {
                    for (i, p) in f
                        .positions
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p[0].abs() == 45. && p[1] == -22.)
                    {
                        let delta = actual_map
                            .get(key)
                            .and_then(|g| g.positions.get(i))
                            .zip(baseline_map.get(key).and_then(|g| g.positions.get(i)))
                            .map(|(a, b)| (a[2] - b[2]) * p[0].signum() * roll as f32);
                        metric.reviewed_direction_failures +=
                            usize::from(delta.is_none_or(|v| v <= EPSILON));
                    }
                }
            }
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
                || !metric.new_planar_crossings.is_empty()
            {
                failures.push(format!(
                    "AWACS combined flap{flap}/roll{roll} geometry witness"
                ));
            }
            poses.push(actual);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
