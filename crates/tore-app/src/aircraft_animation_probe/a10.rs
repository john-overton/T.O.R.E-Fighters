//! Independent A10.SH source endpoints and fitted-role witnesses.
//! No animation transform is repeated. Exposed main-wheel stow is an agent fit.
use super::*;
use tore_formats::shape::Shape;

const GEAR: [usize; 20] = [
    0x4a6c, 0x4a83, 0x4ac7, 0x4ade, 0x4b3a, 0x4b59, 0x4b78, 0x4b97, 0x4bfb, 0x4c1a, 0x4c39, 0x4c58,
    0x4ca4, 0x4cbb, 0x4cff, 0x4d16, 0x4d72, 0x4d91, 0x4db0, 0x4dcf,
];
const MAIN_LEFT: [usize; 4] = [0x4b3a, 0x4b59, 0x4b78, 0x4b97];
const MAIN_RIGHT: [usize; 4] = [0x4bfb, 0x4c1a, 0x4c39, 0x4c58];
const NOSE: [usize; 4] = [0x4d72, 0x4d91, 0x4db0, 0x4dcf];
const FIN: [usize; 6] = [0x2c58, 0x2c6c, 0x2c80, 0x3a2a, 0x3a3e, 0x3a52];
const CLOSURES: [usize; 4] = [0x4fd1, 0x4fe6, 0x4e81, 0x4e96];
const RAW_DOWN: [usize; 4] = [0x4f93, 0x4fb2, 0x4e43, 0x4e62];
const STATIC_TAIL: [usize; 4] = [0x2552, 0x29b8, 0x377a, 0x378d];

pub(super) struct Sources {
    gear: Vec<Face>,
    down: BTreeMap<usize, Vec<[f32; 3]>>,
    closures: Vec<Face>,
    nose_painted: Vec<(usize, [f32; 2])>,
    body: Vec<Face>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8], atlas_bytes: &[u8]) -> AppResult<Self> {
        let atlas = tore_formats::Pic::parse(atlas_bytes)?;
        if atlas.width != 256 || atlas.height != 409 {
            return Err("unreviewed A10 probe atlas dimensions".into());
        }
        let body = Shape::parse(bytes)?
            .faces
            .into_iter()
            .filter(|f| f.positions.iter().all(|p| p[0].abs() <= 7.))
            .collect();
        let gear: Vec<_> = Shape::with_state(bytes, &[(0x5d70, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| GEAR.contains(&f.address))
            .collect();
        if gear.len() != GEAR.len() {
            return Err("A10 probe gear source missing/duplicates faces".into());
        }
        let mut down = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, bindings, ends) in [
            (
                0x5d7c,
                [
                    (0x5050, 0x4f93, [0, 1, 2, 3]),
                    (0x506f, 0x4fb2, [3, 0, 1, 2]),
                ],
                [0x4fd1, 0x4fe6],
            ),
            (
                0x5d82,
                [
                    (0x4f00, 0x4e43, [0, 1, 2, 3]),
                    (0x4f1f, 0x4e62, [1, 2, 3, 0]),
                ],
                [0x4e81, 0x4e96],
            ),
        ] {
            let source = Shape::with_state(bytes, &[(word, -1)].into())?;
            for (neutral, address, order) in bindings {
                let f = source
                    .faces
                    .iter()
                    .find(|f| f.address == address)
                    .ok_or("A10 probe down flap missing")?;
                if f.positions.len() != 4 {
                    return Err("A10 probe down flap topology changed".into());
                }
                down.insert(neutral, order.map(|i| f.positions[i]).into());
            }
            for address in ends {
                closures.push(
                    source
                        .faces
                        .iter()
                        .find(|f| f.address == address)
                        .ok_or("A10 probe source closure missing")?
                        .clone(),
                );
            }
        }
        let mut nose_painted = Vec::new();
        for f in gear.iter().filter(|f| NOSE.contains(&f.address)) {
            if f.uv.len() != f.positions.len() || f.uv.is_empty() {
                return Err("A10 probe nose UV topology changed".into());
            }
            let lo = std::array::from_fn::<_, 2, _>(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min) as usize
            });
            let hi = std::array::from_fn::<_, 2, _>(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::NEG_INFINITY, f32::max) as usize
            });
            if hi[0] >= atlas.width || hi[1] >= atlas.height {
                return Err("A10 probe nose UV outside original atlas".into());
            }
            for v in lo[1]..=hi[1] {
                for u in lo[0]..=hi[0] {
                    let index = (atlas.height - 1 - v) * atlas.width + u;
                    let uv = [u as f32, v as f32];
                    if atlas.mask[index]
                        && atlas.pixels[index] != 255
                        && painted_world(f, uv).is_some()
                    {
                        nose_painted.push((f.address, uv));
                    }
                }
            }
        }
        if nose_painted.is_empty() {
            return Err("A10 probe nose atlas has no painted samples".into());
        }
        Ok(Self {
            gear,
            down,
            closures,
            nose_painted,
            body,
        })
    }
}
struct Panel {
    faces: &'static [usize],
    roots: &'static [[f32; 3]],
    trailing: &'static [[f32; 3]],
    sign: f32,
}
fn panels(control: Control) -> Vec<Panel> {
    match control {
        Control::Elevator => vec![
            Panel {
                faces: &[0x28f5, 0x29cd],
                roots: &[[4., -66., 1.], [28., -66., 1.]],
                trailing: &[[4., -72., 1.], [24., -72., 1.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x36b7, 0x3765],
                roots: &[[-5., -66., 1.], [-29., -66., 1.]],
                trailing: &[[-5., -72., 1.], [-25., -72., 1.]],
                sign: 1.,
            },
        ],
        Control::Aileron => vec![
            Panel {
                faces: &[0x3a0f, 0x3901],
                roots: &[
                    [-25., -6., -3.],
                    [-48., -6., -1.],
                    [-25., -6., -4.],
                    [-48., -6., -2.],
                ],
                trailing: &[[-25., -16., -3.], [-48., -14., -2.]],
                sign: -1.,
            },
            Panel {
                faces: &[0x2b41, 0x2bd3],
                roots: &[
                    [24., -6., -3.],
                    [47., -6., -1.],
                    [24., -6., -4.],
                    [47., -6., -2.],
                ],
                trailing: &[[24., -16., -3.], [47., -14., -2.]],
                sign: 1.,
            },
        ],
        Control::Flaps => vec![
            Panel {
                faces: &[0x5050, 0x506f],
                roots: &[
                    [-48., -6., -1.],
                    [-73., -6., 0.],
                    [-48., -6., -2.],
                    [-73., -6., -1.],
                ],
                trailing: &[[-48., -14., -2.], [-73., -11., -1.]],
                sign: -1.,
            },
            Panel {
                faces: &[0x4f00, 0x4f1f],
                roots: &[
                    [47., -6., -1.],
                    [73., -6., 0.],
                    [47., -6., -2.],
                    [73., -6., -1.],
                ],
                trailing: &[[47., -14., -2.], [73., -11., -1.]],
                sign: -1.,
            },
        ],
        Control::Gear => vec![
            Panel {
                faces: &MAIN_LEFT,
                roots: &[[-23., -1., -8.]],
                trailing: &[],
                sign: 0.,
            },
            Panel {
                faces: &MAIN_RIGHT,
                roots: &[[21., -1., -8.]],
                trailing: &[],
                sign: 0.,
            },
            Panel {
                faces: &NOSE,
                roots: &[[0., 38., -4.]],
                trailing: &[],
                sign: 0.,
            },
            Panel {
                faces: &[0x4a6c, 0x4a83],
                roots: &[[-24., -5., -9.], [-18., -5., -9.]],
                trailing: &[],
                sign: 0.,
            },
            Panel {
                faces: &[0x4ac7, 0x4ade],
                roots: &[[18., -5., -9.], [24., -5., -9.]],
                trailing: &[],
                sign: 0.,
            },
            Panel {
                faces: &[0x4ca4, 0x4cbb],
                roots: &[[3., 40., -4.], [3., 57., -4.]],
                trailing: &[],
                sign: 0.,
            },
            Panel {
                faces: &[0x4cff, 0x4d16],
                roots: &[[-2., 35., -4.], [4., 35., -4.]],
                trailing: &[],
                sign: 0.,
            },
        ],
        _ => Vec::new(),
    }
}
fn selected<'a>(faces: &'a [Face], addresses: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(key, _)| addresses.contains(&key.0))
        .collect()
}
fn covered(original: &[Face], pose: &[Face], metric: &mut Metrics) {
    for f in original {
        for p in &f.positions {
            metric.reviewed_neutral_mismatch |= !pose
                .iter()
                .filter(|g| g.address == f.address)
                .any(|g| g.positions.iter().any(|q| distance(*p, *q) <= EPSILON));
        }
    }
}
fn pin(
    before: &BTreeMap<FaceKey, &Face>,
    after: &BTreeMap<FaceKey, &Face>,
    point: [f32; 3],
    scale: f32,
    metric: &mut Metrics,
) {
    let actual = f4::witness_positions(before, after, point);
    metric.reviewed_anchor_missing |= actual.is_empty();
    for p in actual {
        metric.max_reviewed_anchor_gap = metric
            .max_reviewed_anchor_gap
            .max(distance(point, p) * scale);
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
    for (side, panel) in panels(control).iter().enumerate() {
        if matches!(control, Control::Gear) && value == 0. && side >= 2 {
            metric.reviewed_wheel_failed |= pose.iter().any(|f| panel.faces.contains(&f.address));
            continue;
        }
        let before = selected(reference, panel.faces);
        let after = selected(pose, panel.faces);
        metric.reviewed_anchor_missing |= before.is_empty()
            || before.iter().any(|(key, f)| {
                after
                    .get(key)
                    .is_none_or(|g| g.positions.len() != f.positions.len())
            });
        for &root in panel.roots {
            pin(&before, &after, root, scale, metric);
        }
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if value == 0. && !matches!(control, Control::Gear) {
            covered(
                &raw.iter()
                    .filter(|f| panel.faces.contains(&f.address))
                    .cloned()
                    .collect::<Vec<_>>(),
                pose,
                metric,
            );
        } else if !panel.trailing.is_empty() {
            for &p in panel.trailing {
                let actual = f4::witness_positions(&before, &after, p);
                metric.reviewed_direction_failures += usize::from(actual.is_empty());
                for q in actual {
                    let delta = (q[2] - p[2]) * scale;
                    metric.reviewed_direction_failures +=
                        usize::from(delta * panel.sign * value.signum() as f32 <= EPSILON);
                    metric.reviewed_control_z_delta[side] = delta;
                }
            }
        }
    }
    if matches!(control, Control::Elevator) {
        let before = selected(reference, &STATIC_TAIL);
        let after = selected(pose, &STATIC_TAIL);
        metric.reviewed_anchor_missing |= before.len() != STATIC_TAIL.len();
        for f in before.values() {
            for &p in &f.positions {
                pin(&before, &after, p, scale, metric);
            }
        }
    }
    if matches!(control, Control::Rudder) {
        let before = selected(reference, &FIN);
        let after = selected(pose, &FIN);
        metric.reviewed_anchor_missing |= before.len() != 12
            || before.iter().any(|(key, f)| {
                after
                    .get(key)
                    .is_none_or(|g| g.positions.len() != f.positions.len())
            });
        for (key, f) in &before {
            let Some(g) = after.get(key) else {
                continue;
            };
            for (p, q) in f.positions.iter().zip(&g.positions) {
                if p[1] >= -63. - EPSILON {
                    metric.max_reviewed_anchor_gap =
                        metric.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                } else if value != 0. {
                    metric.reviewed_direction_failures +=
                        usize::from((q[0] - p[0]) * scale * value.signum() as f32 <= EPSILON);
                }
            }
        }
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if value == 0. {
            covered(
                &raw.iter()
                    .filter(|f| FIN.contains(&f.address))
                    .cloned()
                    .collect::<Vec<_>>(),
                pose,
                metric,
            );
        }
    }
    if matches!(control, Control::Flaps) {
        metric.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| RAW_DOWN.contains(&f.address)));
        metric.reviewed_direction_failures +=
            usize::from(CLOSURES.iter().any(|a| {
                pose.iter().filter(|f| f.address == *a).count() != usize::from(value > 0.)
            }));
        if value > 0. {
            // Closures must stay joined to their own source upper/lower skin,
            // not to a distinct neighboring inner roll surface.
            for (a, skin) in [
                (0x4fd1, [0x5050, 0x506f]),
                (0x4fe6, [0x5050, 0x506f]),
                (0x4e81, [0x4f00, 0x4f1f]),
                (0x4e96, [0x4f00, 0x4f1f]),
            ] {
                if let Some(f) = pose.iter().find(|f| f.address == a) {
                    for p in &f.positions {
                        metric.reviewed_anchor_missing |=
                            !pose.iter().filter(|g| skin.contains(&g.address)).any(|g| {
                                g.positions
                                    .iter()
                                    .any(|q| distance(*p, *q) * scale <= EPSILON)
                            });
                    }
                }
            }
        }
        if value == 1. {
            for (a, positions) in &source.down {
                metric.reviewed_direction_failures += usize::from(!pose.iter().any(|f| {
                    f.address == *a
                        && f.positions.len() == positions.len()
                        && f.positions
                            .iter()
                            .zip(positions)
                            .all(|(a, b)| distance(*a, *b) * scale <= EPSILON)
                }));
            }
            covered(&source.closures, pose, metric);
        }
    }
    if matches!(control, Control::Gear) {
        if value == 1. {
            covered(&source.gear, pose, metric);
        }
        wheels(reference, pose, value, scale, metric);
        if value > 0. && value < 1e-5 {
            for (address, uv) in &source.nose_painted {
                metric.reviewed_wheel_failed |= pose
                    .iter()
                    .find(|f| f.address == *address)
                    .and_then(|f| painted_world(f, *uv))
                    .is_none_or(|p| !inside_sections(p, &source.body));
            }
        }
    }
}
// Original opaque UV samples are mapped through the actual posed face. This
// checks body containment without reconstructing the rig's rotation law.
fn painted_world(face: &Face, uv: [f32; 2]) -> Option<[f32; 3]> {
    if face.uv.len() != face.positions.len() || face.uv.len() < 3 {
        return None;
    }
    for i in 1..face.uv.len() - 1 {
        let (a, b, c) = (face.uv[0], face.uv[i], face.uv[i + 1]);
        let den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if den.abs() < 1e-9 {
            continue;
        }
        let x = ((b[1] - c[1]) * (uv[0] - c[0]) + (c[0] - b[0]) * (uv[1] - c[1])) / den;
        let y = ((c[1] - a[1]) * (uv[0] - c[0]) + (a[0] - c[0]) * (uv[1] - c[1])) / den;
        let z = 1. - x - y;
        if x.min(y).min(z) >= -1e-6 {
            return Some(std::array::from_fn(|k| {
                x * face.positions[0][k] + y * face.positions[i][k] + z * face.positions[i + 1][k]
            }));
        }
    }
    None
}
fn orient(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn hull(mut points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    let chain = |iter: Vec<[f32; 2]>| {
        let mut result = Vec::new();
        for p in iter {
            while result.len() > 1
                && orient(result[result.len() - 2], result[result.len() - 1], p) <= 0.
            {
                result.pop();
            }
            result.push(p);
        }
        result.pop();
        result
    };
    let mut lower = chain(points.clone());
    points.reverse();
    lower.extend(chain(points));
    lower
}
fn inside_sections(point: [f32; 3], faces: &[Face]) -> bool {
    let mut points = Vec::new();
    for f in faces {
        for (a, b) in f
            .positions
            .iter()
            .zip(f.positions.iter().cycle().skip(1))
            .take(f.positions.len())
        {
            if a[1] != b[1] && a[1].min(b[1]) <= point[1] && point[1] <= a[1].max(b[1]) {
                let t = (point[1] - a[1]) / (b[1] - a[1]);
                points.push([a[0] + (b[0] - a[0]) * t, a[2] + (b[2] - a[2]) * t]);
            }
        }
    }
    let boundary = hull(points);
    boundary.len() >= 3
        && boundary
            .iter()
            .zip(boundary.iter().cycle().skip(1))
            .take(boundary.len())
            .all(|(a, b)| orient(*a, *b, [point[0], point[2]]) >= -1e-3)
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    let after = keyed(pose);
    let mut counts = [0; 3];
    let mut left = f32::NEG_INFINITY;
    let mut right = f32::INFINITY;
    for (side, ids) in [MAIN_LEFT.as_slice(), MAIN_RIGHT.as_slice(), NOSE.as_slice()]
        .into_iter()
        .enumerate()
    {
        if side == 2 && travel == 0. {
            continue;
        }
        let mut originals: Vec<[f32; 3]> = Vec::new();
        let mut actuals: Vec<[f32; 3]> = Vec::new();
        for (key, f) in selected(reference, ids) {
            counts[side] += 1;
            let Some(g) = after.get(&key) else {
                metric.reviewed_wheel_failed = true;
                continue;
            };
            if f.positions.len() != g.positions.len() {
                metric.reviewed_wheel_failed = true;
                continue;
            }
            originals.extend(f.positions.iter().copied());
            actuals.extend(g.positions.iter().copied());
            for p in &g.positions {
                if side == 0 {
                    left = left.max(p[0] * scale);
                } else if side == 1 {
                    right = right.min(p[0] * scale);
                }
                if travel < 1e-5 {
                    let inside = if side == 2 {
                        (-3.001..=3.001).contains(&p[0])
                            && (37.12..=56.26).contains(&p[1])
                            && (-6.97..=4.06).contains(&p[2])
                    } else {
                        let x = if side == 0 {
                            -26.001..=-19.999
                        } else {
                            18.999..=24.001
                        };
                        x.contains(&p[0])
                            && (-1.001..=13.001).contains(&p[1])
                            && (-12.001..=-3.999).contains(&p[2])
                    };
                    metric.reviewed_wheel_failed |= !inside;
                }
            }
        }
        // Cross-face distances prove the entire crossed assembly stays rigid,
        // rather than merely each independently rotating rectangle.
        for i in 0..originals.len() {
            for j in i + 1..originals.len() {
                metric.reviewed_wheel_rigidity_error = metric.reviewed_wheel_rigidity_error.max(
                    (distance(originals[i], originals[j]) - distance(actuals[i], actuals[j])).abs()
                        * scale,
                );
            }
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |= counts != [4, 4, if travel == 0. { 0 } else { 4 }]
        || right < 6.3
        || left > -6.6
        || right - left < 13.0
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic(a: usize, p: Vec<[f32; 3]>) -> Face {
        super::super::tests::face(a, p)
    }
    #[test]
    fn atlas_samples_follow_actual_uv_face_and_body_section_contains_only_its_interior() {
        let mut face = synthetic(
            0xf002,
            vec![[0., 0., 2.], [0., 4., 2.], [0., 4., -2.], [0., 0., -2.]],
        );
        face.uv = vec![[0., 0.], [8., 0.], [8., 8.], [0., 8.]];
        assert_eq!(painted_world(&face, [2., 4.]), Some([0., 1., 0.]));
        face.positions.insert(1, [0., 1., 2.]);
        face.uv.insert(1, [2., 0.]);
        assert_eq!(painted_world(&face, [2., 4.]), Some([0., 1., 0.]));
        assert!(painted_world(&face, [9., 4.]).is_none());
        let body = [
            synthetic(
                0xf003,
                vec![
                    [-2., 0., -3.],
                    [-2., 10., -3.],
                    [-2., 10., 3.],
                    [-2., 0., 3.],
                ],
            ),
            synthetic(
                0xf004,
                vec![[2., 0., -3.], [2., 10., -3.], [2., 10., 3.], [2., 0., 3.]],
            ),
        ];
        assert!(inside_sections([0., 5., 0.], &body));
        assert!(!inside_sections([3., 5., 0.], &body));
        assert!(!inside_sections([0., 12., 0.], &body));
    }
    #[test]
    fn missing_exposed_stow_wheel_is_a_failure() {
        let mut source = Vec::new();
        for ids in [MAIN_LEFT, MAIN_RIGHT] {
            for a in ids {
                let x = if MAIN_LEFT.contains(&a) { -23. } else { 21. };
                source.push(synthetic(
                    a,
                    vec![[x, 0., -8.], [x, 2., -8.], [x, 2., -10.], [x, 0., -10.]],
                ));
            }
        }
        let mut metric = Metrics::default();
        wheels(&source, &source, 0., 1. / 3., &mut metric);
        assert!(!metric.reviewed_wheel_failed);
        let mut metric = Metrics::default();
        wheels(
            &source,
            &source[..source.len() - 1],
            0.,
            1. / 3.,
            &mut metric,
        );
        assert!(metric.reviewed_wheel_failed);
        let mut stretched = source.clone();
        stretched[0].positions[0][1] -= 1.;
        let mut metric = Metrics::default();
        wheels(&source, &stretched, 0., 1. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
    }
    #[test]
    fn neutral_coverage_uses_address_and_points_across_split_occurrences() {
        let source = synthetic(
            0xf001,
            vec![[0., 0., 0.], [0., 4., 0.], [0., 4., 6.], [0., 0., 6.]],
        );
        let a = synthetic(
            source.address,
            vec![
                source.positions[0],
                [0., 2., 0.],
                [0., 2., 6.],
                source.positions[3],
            ],
        );
        let b = synthetic(
            source.address,
            vec![
                [0., 2., 0.],
                source.positions[1],
                source.positions[2],
                [0., 2., 6.],
            ],
        );
        let mut metric = Metrics::default();
        covered(std::slice::from_ref(&source), &[a.clone(), b], &mut metric);
        assert!(!metric.reviewed_neutral_mismatch);
        let mut metric = Metrics::default();
        covered(&[source], &[a], &mut metric);
        assert!(metric.reviewed_neutral_mismatch);
    }
}
