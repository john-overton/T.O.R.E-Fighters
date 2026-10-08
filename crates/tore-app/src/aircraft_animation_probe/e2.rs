//! Independent E2C/E2 source endpoints, fitted attachment and mixed-control witnesses.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 4] = [0x589e, 0x58bd, 0x58dc, 0x58fb];
const RIGHT: [usize; 4] = [0x595f, 0x597e, 0x599d, 0x59bc];
const NOSE: [usize; 2] = [0x5a08, 0x5a27];
const HOOK: [usize; 2] = [0x5a65, 0x5a84];
const FLAPS: [usize; 4] = [0x577d, 0x57a4, 0x5676, 0x569d];
const LEFT_TAIL: [usize; 4] = [0x3dce, 0x3ebe, 0x3ed9, 0x3ef4];
const RIGHT_TAIL: [usize; 4] = [0x2f84, 0x3074, 0x308f, 0x30aa];
pub(super) struct Sources {
    gear: Vec<Face>,
    hook: Vec<Face>,
    flaps: BTreeMap<usize, (Face, [usize; 4])>,
    closures: Vec<(Face, Vec<[f32; 3]>)>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let neutral = Shape::parse(bytes)?;
        let gear: Vec<_> = Shape::with_state(bytes, &[(0x8226, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect();
        if gear.len() != 10 {
            return Err("E2C probe missing gear faces".into());
        }
        let hook: Vec<_> = Shape::with_state(bytes, &[(0x8232, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| HOOK.contains(&f.address))
            .collect();
        if hook.len() != 2 {
            return Err("E2C probe missing source hook".into());
        }
        let mut flaps = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, pairs, ends) in [
            (
                0x8238,
                [
                    (0x577d, 0x581d, [0, 1, 2, 3]),
                    (0x57a4, 0x57f6, [2, 3, 0, 1]),
                ],
                [0x5844],
            ),
            (
                0x823e,
                [
                    (0x5676, 0x5716, [1, 2, 3, 0]),
                    (0x569d, 0x56ef, [1, 2, 3, 0]),
                ],
                [0x573d],
            ),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let mut mapping = BTreeMap::new();
            for (a, b, order) in pairs {
                let base = one(&neutral.faces, a)?;
                let target = one(&down.faces, b)?.clone();
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("E2C probe flap topology changed".into());
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
                            .ok_or("E2C probe closure lacks neutral correspondence")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                closures.push((f, n));
            }
        }
        Ok(Self {
            gear,
            hook,
            flaps,
            closures,
        })
    }
}
fn one(faces: &[Face], a: usize) -> AppResult<&Face> {
    faces
        .iter()
        .find(|f| f.address == a)
        .ok_or_else(|| format!("E2C probe missing source face {a:x}").into())
}
struct Panel {
    faces: &'static [usize],
    roots: Vec<[f32; 3]>,
    trailing: Vec<[f32; 3]>,
    sign: f32,
}
fn panels(c: Control) -> Vec<Panel> {
    match c {
        Control::Rudder => vec![
            Panel {
                faces: &[0x3d5a, 0x3d75, 0x3e0f, 0x3ea1],
                roots: vec![
                    [-22., -42., -6.],
                    [-21., -42., 6.],
                    [-20., -42., 15.],
                    [-21., -37., 6.],
                ],
                trailing: vec![[-22., -45., -6.], [-21., -45., 6.], [-20., -45., 15.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x531d, 0x5340],
                roots: vec![[-11., -42., 5.], [-10., -42., 15.], [-11., -36., 5.]],
                trailing: vec![[-11., -44., 6.], [-10., -44., 15.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x5388, 0x53ab, 0x53ca],
                roots: vec![[11., -42., 5.], [10., -42., 15.], [11., -36., 5.]],
                trailing: vec![[11., -44., 6.], [10., -44., 15.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x2f4b, 0x2f66, 0x2fc5, 0x2fe1],
                roots: vec![
                    [22., -42., -6.],
                    [21., -42., 6.],
                    [20., -42., 15.],
                    [21., -37., 6.],
                ],
                trailing: vec![[22., -45., -6.], [21., -45., 6.], [20., -45., 15.]],
                sign: 1.,
            },
        ],
        Control::Elevator => [-1., 1.]
            .into_iter()
            .map(|side| Panel {
                faces: if side < 0. { &LEFT_TAIL } else { &RIGHT_TAIL },
                roots: vec![
                    [0., -35., 4.],
                    [0., -45., 4.],
                    [side * 11., -36., 5.],
                    [side * 11., -42., 5.],
                    [side * 21., -37., 6.],
                    [side * 21., -45., 6.],
                ],
                trailing: vec![
                    [side * 9., -45., 4. + 18. / 21.],
                    [side * 19., -45., 4. + 38. / 21.],
                ],
                sign: 1.,
            })
            .collect(),
        Control::Flaps | Control::Aileron => vec![
            Panel {
                faces: &[0x577d, 0x57a4],
                roots: vec![[-33., -4., 3.], [-33., -4., 4.], [-56., -4., 5.]],
                trailing: vec![[-33., -9., 4.], [-56., -6., 5.]],
                sign: -1.,
            },
            Panel {
                faces: &[0x5676, 0x569d],
                roots: vec![
                    [33., -4., 3.],
                    [33., -4., 5.],
                    [57., -4., 5.],
                    [57., -4., 6.],
                ],
                trailing: vec![[33., -9., 4.], [57., -6., 5.]],
                sign: if matches!(c, Control::Flaps) { -1. } else { 1. },
            },
        ],
        Control::Gear => vec![
            Panel {
                faces: &LEFT,
                roots: vec![[-16., 0., -7.]],
                trailing: vec![],
                sign: 0.,
            },
            Panel {
                faces: &RIGHT,
                roots: vec![],
                trailing: vec![],
                sign: 0.,
            },
            Panel {
                faces: &NOSE,
                roots: vec![[0., 30., -7.]],
                trailing: vec![],
                sign: 0.,
            },
        ],
        Control::Hook => vec![Panel {
            faces: &HOOK,
            roots: vec![[0., -13., -8.]],
            trailing: vec![],
            sign: 0.,
        }],
        _ => Vec::new(),
    }
}
fn controlled(f: &Face, c: Control) -> bool {
    if !matches!(c, Control::Elevator) {
        return true;
    }
    f.positions
        .iter()
        .all(|p| p[1] + 40. + 0.05 * p[0].abs() <= EPSILON)
        && [(2., 9.), (13., 19.)].into_iter().any(|(lo, hi)| {
            f.positions
                .iter()
                .all(|p| (lo - EPSILON..=hi + EPSILON).contains(&p[0].abs()))
        })
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
        let moving_before: BTreeMap<_, _> = before
            .into_iter()
            .filter(|(_, f)| controlled(f, control))
            .collect();
        let moving_after: BTreeMap<_, _> = after
            .into_iter()
            .filter(|(k, _)| moving_before.contains_key(k))
            .collect();
        if let Some(gap) = shared_vertex_gaps(&moving_before, &moving_after, scale).first() {
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
    if matches!(control, Control::Gear | Control::Hook) && value == 0. {
        metric.reviewed_wheel_failed |= pose.iter().any(|f| {
            if matches!(control, Control::Hook) {
                HOOK.contains(&f.address)
            } else {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            }
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
                let moving_before: BTreeMap<_, _> = before
                    .iter()
                    .filter(|(_, f)| controlled(f, control))
                    .map(|(k, f)| (*k, *f))
                    .collect();
                let moving_after: BTreeMap<_, _> = after
                    .iter()
                    .filter(|(k, _)| moving_before.contains_key(k))
                    .map(|(k, f)| (*k, *f))
                    .collect();
                let actual = witnesses(&moving_before, &moving_after, *p);
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
    if matches!(control, Control::Gear | Control::Hook) {
        let originals = if matches!(control, Control::Hook) {
            &source.hook
        } else {
            &source.gear
        };
        if value == 1. {
            for f in originals {
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
        devices(reference, pose, value, control, scale, metric);
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
fn devices(
    reference: &[Face],
    pose: &[Face],
    travel: f64,
    control: Control,
    scale: f32,
    metric: &mut Metrics,
) {
    if matches!(control, Control::Gear) {
        let pivot = [16., 0., -6.];
        if let Some(actual) = affine_anchor(reference, pose, 0x595f, pivot) {
            metric.max_reviewed_anchor_gap = metric
                .max_reviewed_anchor_gap
                .max(distance(pivot, actual) * scale);
        } else {
            metric.reviewed_anchor_missing = true;
        }
    }
    let groups: Vec<&[usize]> = if matches!(control, Control::Hook) {
        vec![&HOOK]
    } else {
        vec![&LEFT, &RIGHT, &NOSE]
    };
    let after = keyed(pose);
    let mut left = f32::NEG_INFINITY;
    let mut right = f32::INFINITY;
    for ids in groups {
        let before = selected(reference, ids);
        let mut old_points = Vec::new();
        let mut new_points = Vec::new();
        metric.reviewed_wheel_failed |= before.len() != ids.len();
        for (k, f) in before {
            let Some(g) = after.get(&k) else {
                metric.reviewed_wheel_failed = true;
                continue;
            };
            if f.positions.len() != g.positions.len() {
                metric.reviewed_wheel_failed = true;
                continue;
            }
            for (p, q) in f.positions.iter().zip(&g.positions) {
                old_points.push(*p);
                new_points.push(*q);
                if LEFT.contains(&k.0) {
                    left = left.max(q[0] * scale);
                } else if RIGHT.contains(&k.0) {
                    right = right.min(q[0] * scale);
                }
                if travel < 1e-5 {
                    let (lo, hi) = if HOOK.contains(&k.0) {
                        ([0., -19., -8.], [0., -13., -2.])
                    } else if NOSE.contains(&k.0) {
                        ([0., 24., -7.], [0., 30., -4.])
                    } else if LEFT.contains(&k.0) {
                        ([-16., -0.85, -7.], [-15., 6.35, -1.32])
                    } else {
                        ([15., 0., -5.58], [16., 7.26, 0.11])
                    };
                    metric.reviewed_wheel_failed |=
                        (0..3).any(|i| q[i] < lo[i] - 1e-3 || q[i] > hi[i] + 1e-3);
                }
            }
        }
        // Cross-card distances are essential: the wheels lie one source unit beside
        // their leg planes, so individually rigid but detached cards must fail.
        for (i, p) in old_points.iter().enumerate() {
            for (j, q) in old_points.iter().enumerate().skip(i + 1) {
                metric.reviewed_wheel_rigidity_error = metric
                    .reviewed_wheel_rigidity_error
                    .max((distance(*p, *q) - distance(new_points[i], new_points[j])).abs() * scale);
            }
        }
    }
    if matches!(control, Control::Gear) {
        metric.reviewed_min_wheel_gap = Some(right - left);
        metric.reviewed_wheel_failed |= right - left < 19.99;
    }
    metric.reviewed_wheel_failed |= metric.reviewed_wheel_rigidity_error > EPSILON;
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
                        .filter(|(_, p)| p[0].abs() == 33. && p[1] == -9.)
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
                    "E2C combined flap{flap}/roll{roll} geometry witness"
                ));
            }
            poses.push(actual);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    let mut paired_rows = String::from("pitch,yaw,anchor_gap_ft,skin_gap_ft,failures\n");
    let mut paired_poses = Vec::new();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for yaw in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.rudder = yaw;
            let actual = airframe.animation_faces(&state);
            let mut metric = measure(&original, &actual, scale);
            check(
                Control::Elevator,
                pitch,
                (&original, &original, &actual),
                scale,
                source,
                &mut metric,
            );
            check(
                Control::Rudder,
                yaw,
                (&original, &original, &actual),
                scale,
                source,
                &mut metric,
            );
            let failed = !metric.finite
                || metric.reviewed_anchor_missing
                || metric.max_reviewed_anchor_gap > EPSILON
                || metric.max_reviewed_skin_gap > EPSILON
                || metric.reviewed_direction_failures > 0
                || metric.reviewed_neutral_mismatch
                || !metric.new_planar_crossings.is_empty();
            writeln!(
                paired_rows,
                "{pitch},{yaw},{},{},{}",
                metric.max_reviewed_anchor_gap,
                metric.max_reviewed_skin_gap,
                usize::from(failed)
            )?;
            if failed {
                failures.push(format!(
                    "E2C combined pitch{pitch}/yaw{yaw} attachment witness"
                ));
            }
            paired_poses.push(actual);
        }
    }
    fs::write(out.join("pitch-yaw-combinations.csv"), paired_rows)?;
    contact_sheet(
        &out.join("pitch-yaw-combinations.ppm"),
        &original,
        &paired_poses,
    )?;
    let mut state = neutral.clone();
    state.hook = 1.;
    let deployed = airframe.animation_faces(&state);
    state.hook = 1e-6;
    let stowed = airframe.animation_faces(&state);
    let mut metric = measure(&deployed, &stowed, scale);
    check(
        Control::Hook,
        1e-6,
        (&deployed, &deployed, &stowed),
        scale,
        source,
        &mut metric,
    );
    if !metric.finite
        || metric.reviewed_wheel_failed
        || metric.reviewed_anchor_missing
        || metric.max_reviewed_anchor_gap > EPSILON
        || metric.max_reviewed_skin_gap > EPSILON
        || !metric.new_planar_crossings.is_empty()
    {
        failures.push("E2C near-zero rigid hook stow witness".into());
    }
    contact_sheet(&out.join("hook-stow.ppm"), &deployed, &[stowed])?;
    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixtures() -> Vec<Face> {
        let mut faces = Vec::new();
        for (ids, side) in [(&LEFT, -1.), (&RIGHT, 1.)] {
            for (i, &a) in ids.iter().enumerate() {
                let (x, y, z) = if i < 2 {
                    (side * 16., 2., -12.)
                } else {
                    (side * 15., 3., -14.)
                };
                let top = if i < 2 { -7. } else { -10. };
                faces.push(super::super::tests::face(
                    a,
                    vec![[x, 0., top], [x, y, top], [x, y, z], [x, 0., z]],
                ));
            }
        }
        for a in NOSE {
            faces.push(super::super::tests::face(
                a,
                vec![
                    [0., 30., -7.],
                    [0., 27., -7.],
                    [0., 27., -13.],
                    [0., 30., -13.],
                ],
            ));
        }
        faces
    }
    #[test]
    fn wheel_leg_separation_fails_even_when_each_card_and_both_wheel_skins_stay_rigid() {
        let source = fixtures();
        let mut metric = Metrics::default();
        devices(&source, &source, 0.5, Control::Gear, 2. / 3., &mut metric);
        assert!(!metric.reviewed_wheel_failed);
        let mut detached = source.clone();
        for f in detached
            .iter_mut()
            .filter(|f| [0x58dc, 0x58fb].contains(&f.address))
        {
            for p in &mut f.positions {
                p[1] += 1.;
            }
        }
        let mut metric = Metrics::default();
        devices(&source, &detached, 0.5, Control::Gear, 2. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
        assert!(metric.reviewed_wheel_rigidity_error > 0.1);
    }
}
