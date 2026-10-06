//! Independent B747 source-surface, atlas endpoint and whole-assembly witnesses.
use super::*;
use tore_formats::shape::Shape;
const RUDDER: [usize; 2] = [0x62ec, 0x6316];
const LEFT_TAIL: [usize; 11] = [
    0x3823, 0x3845, 0x3863, 0x3ae6, 0x3b08, 0x3b26, 0x557e, 0x55a9, 0x55cf, 0x57af, 0x57cd,
];
const RIGHT_TAIL: [usize; 8] = [
    0x46e7, 0x472f, 0x4755, 0x5512, 0x5530, 0x5651, 0x5670, 0x5692,
];
const LEFT_ROLL: [usize; 4] = [0x2937, 0x2976, 0x2e64, 0x2ea1];
const RIGHT_ROLL: [usize; 4] = [0x4bd9, 0x4c1c, 0x5187, 0x51aa];
const LEFT: [usize; 16] = [
    0x5f16, 0x5f3d, 0x5f64, 0x5f8b, 0x5fb2, 0x5fd1, 0x5ff0, 0x600f, 0x602e, 0x604d, 0x606c, 0x608b,
    0x60aa, 0x60c9, 0x60e8, 0x6107,
];
const RIGHT: [usize; 16] = [
    0x5c31, 0x5c58, 0x5c7f, 0x5ca6, 0x5ccd, 0x5cec, 0x5d0b, 0x5d2a, 0x5d49, 0x5d68, 0x5d87, 0x5da6,
    0x5dc5, 0x5de4, 0x5e03, 0x5e22,
];
const NOSE: [usize; 4] = [0x616b, 0x618a, 0x61a9, 0x61c8];

const FLAPS: [usize; 4] = [0x64d4, 0x64f3, 0x63eb, 0x640a];
pub(super) struct Sources {
    gear: Vec<Face>,
    neutral: Vec<Face>,
    deployed: Vec<Face>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let neutral = Shape::parse(bytes)?
            .faces
            .into_iter()
            .filter(|f| FLAPS.contains(&f.address))
            .collect::<Vec<_>>();
        let gear = Shape::with_state(bytes, &[(0x8960, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect::<Vec<_>>();
        let mut deployed = Vec::new();
        for (word, ids) in [
            (0x896c, [0x6454, 0x6473, 0x648e]),
            (0x8972, [0x636b, 0x638a, 0x63a5]),
        ] {
            deployed.extend(
                Shape::with_state(bytes, &[(word, -1)].into())?
                    .faces
                    .into_iter()
                    .filter(|f| ids.contains(&f.address))
                    .map(|mut f| {
                        f.address = match f.address {
                            0x6454 => 0x64d4,
                            0x6473 | 0x648e => 0x64f3,
                            0x636b => 0x63eb,
                            _ => 0x640a,
                        };
                        f
                    }),
            );
        }
        if neutral.len() != 4 || deployed.len() != 6 || gear.len() != 36 {
            return Err("B747 probe source groups changed".into());
        }
        Ok(Self {
            gear,
            neutral,
            deployed,
        })
    }
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
            faces: &RUDDER,
            roots: vec![[0., -155., 15.], [0., -186., 58.]],
            trailing: vec![[0., -176., 15.], [0., -195., 58.]],
            sign: 1.,
        }],
        Control::Elevator => [-1., 1.]
            .into_iter()
            .map(|side| Panel {
                faces: if side < 0. { &LEFT_TAIL } else { &RIGHT_TAIL },
                roots: vec![[side * 8., -139., 12.], [side * 4., -176., 12.]],
                trailing: vec![[side * 51., -194., 16.]],
                sign: 1.,
            })
            .collect(),
        Control::Aileron => [-1., 1.]
            .into_iter()
            .map(|side| Panel {
                faces: if side < 0. { &LEFT_ROLL } else { &RIGHT_ROLL },
                roots: vec![],
                trailing: vec![],
                sign: side,
            })
            .collect(),
        Control::Flaps => [-1., 1.]
            .into_iter()
            .map(|side| Panel {
                faces: if side < 0. {
                    &[0x64d4, 0x64f3]
                } else {
                    &[0x63eb, 0x640a]
                },
                roots: vec![
                    [side * 15., -4., -5.],
                    [side * 15., -3., -12.],
                    [side * 60., -18., 0.],
                    [side * 60., -18., -5.],
                ],
                trailing: vec![[side * 15., -21., -8.], [side * 60., -35., -3.]],
                sign: -1.,
            })
            .collect(),
        _ => vec![],
    }
}
fn cut(p: [f32; 3], c: Control) -> f32 {
    if matches!(c, Control::Elevator) {
        p[1] + 166. + 0.5 * (p[0].abs() - 8.)
    } else {
        p[1] + 57. + (p[0].abs() - 113.) * 23. / 41.
    }
}
fn controlled(f: &Face, c: Control) -> bool {
    match c {
        Control::Elevator => f.positions.iter().all(|p| cut(*p, c) <= EPSILON),
        Control::Aileron => f.positions.iter().all(|p| {
            (118. - EPSILON..=145. + EPSILON).contains(&p[0].abs()) && cut(*p, c) <= EPSILON
        }),
        _ => true,
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
        if matches!(control, Control::Elevator | Control::Aileron) {
            let mut count = 0;
            for (key, f) in &before {
                for (i, p) in f
                    .positions
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| cut(**p, control).abs() <= EPSILON)
                {
                    count += 1;
                    if let Some(q) = after.get(key).and_then(|g| g.positions.get(i)) {
                        metric.max_reviewed_anchor_gap =
                            metric.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                    } else {
                        metric.reviewed_anchor_missing = true;
                    }
                }
            }
            metric.reviewed_anchor_missing |= count < 4;
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
    if matches!(control, Control::Gear) {
        if value == 0. {
            metric.reviewed_wheel_failed |= pose.iter().any(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            });
            return;
        }
        if value == 1. {
            for f in &source.gear {
                let found = pose.iter().find(|g| g.address == f.address);
                metric.reviewed_neutral_mismatch |= found.is_none_or(|g| {
                    g.positions != f.positions
                        || g.uv != f.uv
                        || g.subtype != f.subtype
                        || g.colors != f.colors
                });
            }
        }
        devices(reference, pose, value, scale, metric);
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
            let moving_before: BTreeMap<_, _> = before
                .into_iter()
                .filter(|(_, f)| controlled(f, control))
                .collect();
            let moving_after: BTreeMap<_, _> = after
                .into_iter()
                .filter(|(k, _)| moving_before.contains_key(k))
                .collect();
            let mut trailing = panel.trailing.clone();
            if matches!(control, Control::Aileron) {
                trailing.extend(
                    moving_before
                        .values()
                        .flat_map(|f| f.positions.iter())
                        .filter(|p| cut(**p, control) < -1.)
                        .copied(),
                );
                metric.reviewed_direction_failures += usize::from(trailing.is_empty());
            }
            for p in trailing {
                let actual = witnesses(&moving_before, &moving_after, p);
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
    if matches!(control, Control::Flaps) && (value == 0. || value == 1.) {
        let expected = if value == 0. {
            &source.neutral
        } else {
            &source.deployed
        };
        metric.reviewed_neutral_mismatch |= !surface_match(expected, pose, scale);
    }
}
// Samples triangle interiors and interpolated art, not merely polygon corners.
fn barycentric(p: [f32; 3], t: [[f32; 3]; 3]) -> Option<[f64; 3]> {
    let u = std::array::from_fn::<_, 3, _>(|i| f64::from(t[1][i] - t[0][i]));
    let v = std::array::from_fn::<_, 3, _>(|i| f64::from(t[2][i] - t[0][i]));
    let w = std::array::from_fn::<_, 3, _>(|i| f64::from(p[i] - t[0][i]));
    let dot = |a: [f64; 3], b: [f64; 3]| a.into_iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let uu = dot(u, u);
    let vv = dot(v, v);
    let uv = dot(u, v);
    let det = uu * vv - uv * uv;
    if det.abs() < 1e-12 {
        return None;
    }
    let b = (dot(w, u) * vv - dot(w, v) * uv) / det;
    let c = (dot(w, v) * uu - dot(w, u) * uv) / det;
    let weights = [1. - b - c, b, c];
    if weights.iter().any(|x| *x < -1e-5) {
        return None;
    }
    let q = std::array::from_fn(|i| {
        (0..3).map(|j| weights[j] * f64::from(t[j][i])).sum::<f64>() as f32
    });
    (distance(p, q) < EPSILON).then_some(weights)
}
fn covered(source: &[Face], target: &[Face], scale: f32) -> bool {
    for f in source.iter().filter(|f| FLAPS.contains(&f.address)) {
        for i in 1..f.positions.len() - 1 {
            let ids = [0, i, i + 1];
            for a in 0..=4 {
                for b in 0..=4 - a {
                    let w = [a as f32 / 4., b as f32 / 4., 1. - (a + b) as f32 / 4.];
                    let p = std::array::from_fn(|d| {
                        (0..3).map(|j| w[j] * f.positions[ids[j]][d]).sum::<f32>()
                    });
                    let uv: [f32; 2] =
                        std::array::from_fn(|d| (0..3).map(|j| w[j] * f.uv[ids[j]][d]).sum());
                    let color = (0..3)
                        .map(|j| w[j] * f32::from(f.colors[ids[j]]))
                        .sum::<f32>();
                    let found = target
                        .iter()
                        .filter(|g| {
                            g.address == f.address
                                && g.texture == f.texture
                                && g.subtype == f.subtype
                                && g.fog == f.fog
                        })
                        .any(|g| {
                            (1..g.positions.len() - 1).any(|k| {
                                let gi = [0, k, k + 1];
                                let Some(v) = barycentric(p, gi.map(|j| g.positions[j])) else {
                                    return false;
                                };
                                let color2 = (0..3)
                                    .map(|j| v[j] * f64::from(g.colors[gi[j]]))
                                    .sum::<f64>();
                                (f64::from(color) - color2).abs() < 0.001
                                    && (0..2).all(|d| {
                                        (f64::from(uv[d])
                                            - (0..3)
                                                .map(|j| v[j] * f64::from(g.uv[gi[j]][d]))
                                                .sum::<f64>())
                                        .abs()
                                            < 0.001
                                    })
                            })
                        });
                    if !found || !scale.is_finite() {
                        return false;
                    }
                }
            }
        }
    }
    true
}
fn surface_match(expected: &[Face], actual: &[Face], scale: f32) -> bool {
    covered(expected, actual, scale) && covered(actual, expected, scale)
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

fn devices(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    for (a, pivot) in [
        (0x5ccd, [6.5, -27., -12.5]),
        (0x5d0b, [6.5, -9., -12.5]),
        (0x5fb2, [-6.5, -27., -12.5]),
        (0x5ff0, [-6.5, -9., -12.5]),
        (0x616b, [0., 100.5, -16.]),
    ] {
        if let Some(q) = affine_anchor(reference, pose, a, pivot) {
            metric.max_reviewed_anchor_gap = metric
                .max_reviewed_anchor_gap
                .max(distance(pivot, q) * scale);
        } else {
            metric.reviewed_anchor_missing = true;
        }
    }
    let mut left = f32::NEG_INFINITY;
    let mut right = f32::INFINITY;
    let after = keyed(pose);
    for ids in [&LEFT[..], &RIGHT[..], &NOSE[..]] {
        let before = selected(reference, ids);
        metric.reviewed_wheel_failed |= before.len() != ids.len();
        let mut old = vec![];
        let mut new = vec![];
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
                old.push(*p);
                new.push(*q);
                if p[2] <= -23. {
                    if LEFT.contains(&k.0) {
                        left = left.max(q[0] * scale);
                    } else if RIGHT.contains(&k.0) {
                        right = right.min(q[0] * scale);
                    }
                }
                if travel < 1e-5 {
                    let (lo, hi) = if NOSE.contains(&k.0) {
                        ([-3., 93.49, -16.91], [3., 106.78, -0.79])
                    } else if LEFT.contains(&k.0) {
                        ([-11., -35., -15.], [-2., -1., 5.])
                    } else {
                        ([2., -35., -15.], [11., -1., 5.])
                    };
                    metric.reviewed_wheel_failed |=
                        (0..3).any(|i| q[i] < lo[i] - 1e-3 || q[i] > hi[i] + 1e-3);
                }
            }
        }
        // Includes cross-card and fore/aft bogie distances. Independently rigid,
        // detached struts or shifted wheels cannot pass this assembly witness.
        for (i, p) in old.iter().enumerate() {
            for (j, q) in old.iter().enumerate().skip(i + 1) {
                metric.reviewed_wheel_rigidity_error = metric
                    .reviewed_wheel_rigidity_error
                    .max((distance(*p, *q) - distance(new[i], new[j])).abs() * scale);
            }
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |=
        right - left < 3.99 * scale || metric.reviewed_wheel_rigidity_error > EPSILON;
}
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    source: &Sources,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = vec![];
    let mut poses = vec![];
    let mut rows = String::from("flap,roll,anchor_gap_ft,skin_gap_ft,failures\n");
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.flaps = flap;
            state.aileron = roll;
            let actual = airframe.animation_faces(&state);
            let mut m = measure(&original, &actual, scale);
            for (c, v) in [(Control::Flaps, flap), (Control::Aileron, roll)] {
                check(c, v, (&original, &original, &actual), scale, source, &mut m);
            }
            let failed = !m.finite
                || m.reviewed_anchor_missing
                || m.max_reviewed_anchor_gap > EPSILON
                || m.max_reviewed_skin_gap > EPSILON
                || m.reviewed_direction_failures > 0
                || m.reviewed_neutral_mismatch
                || !m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{flap},{roll},{},{},{}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                usize::from(failed)
            )?;
            if failed {
                failures.push(format!("B747 independent flap{flap}/roll{roll} witnesses"));
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
    #[test]
    fn endpoint_witness_rejects_opposite_diagonal_and_wrong_atlas() {
        let mut f = super::super::tests::face(
            0x640a,
            vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 1.], [0., 2., 0.]],
        );
        f.uv = vec![[0., 0.], [2., 0.], [2., 2.], [0., 2.]];
        let tri = |ids: [usize; 3]| {
            let mut g = f.clone();
            g.positions = ids.map(|i| f.positions[i]).to_vec();
            g.uv = ids.map(|i| f.uv[i]).to_vec();
            g.colors = ids.map(|i| f.colors[i]).to_vec();
            g
        };
        let expected = vec![tri([0, 1, 3]), tri([1, 2, 3])];
        assert!(surface_match(&expected, &expected, 1.));
        assert!(!surface_match(&expected, &[f], 1.));
        let mut wrong = expected.clone();
        wrong[0].uv[0][0] += 4.;
        assert!(!surface_match(&expected, &wrong, 1.));
    }
}
