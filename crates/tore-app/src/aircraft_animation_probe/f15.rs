//! Independent F15.SH attachment, direction and endpoint witnesses.
//! Branch geometry is decoded at runtime; no animation transform is duplicated.
use super::*;
use tore_formats::shape::Shape;

const MAIN_LEFT: [usize; 4] = [0x57db, 0x57fa, 0x5819, 0x5838];
const MAIN_RIGHT: [usize; 4] = [0x59a9, 0x59c8, 0x59e7, 0x5a06];
const NOSE: [usize; 6] = [0x5a82, 0x5aa1, 0x5ac0, 0x5ad7, 0x5aee, 0x5b0d];
const GEAR: [usize; 22] = [
    0x573a, 0x5751, 0x5768, 0x577f, 0x57db, 0x57fa, 0x5819, 0x5838, 0x5908, 0x591f, 0x5936, 0x594d,
    0x59a9, 0x59c8, 0x59e7, 0x5a06, 0x5a82, 0x5aa1, 0x5ac0, 0x5ad7, 0x5aee, 0x5b0d,
];
const BRAKE: [usize; 4] = [0x5d73, 0x5d8a, 0x5da1, 0x5db8];
#[cfg(test)]
const FLAPS: [usize; 4] = [0x5c3e, 0x5c5d, 0x5b57, 0x5b76];
const CLOSURES: [usize; 2] = [0x5ce5, 0x5bfe];

pub(super) struct Sources {
    gear: Vec<Face>,
    brake: Vec<Face>,
    flaps: BTreeMap<usize, (Face, [usize; 4])>,
    closures: Vec<Face>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let gear = branch(bytes, 0x880c, 1, &GEAR)?;
        let brake = branch(bytes, 0x8806, 1, &BRAKE)?;
        let mut flaps = BTreeMap::new();
        let mut closures = Vec::new();
        // These are reviewed source-face correspondences, not interpolation laws.
        for (word, pairs, closure) in [
            (
                0x8818,
                [
                    (0x5c3e, 0x5cc6, [0, 1, 2, 3]),
                    (0x5c5d, 0x5ca7, [1, 2, 3, 0]),
                ],
                0x5ce5,
            ),
            (
                0x881e,
                [
                    (0x5b57, 0x5bdf, [3, 0, 1, 2]),
                    (0x5b76, 0x5bc0, [3, 0, 1, 2]),
                ],
                0x5bfe,
            ),
        ] {
            let shape = Shape::with_state(bytes, &[(word, -1)].into())?;
            for (neutral, down, order) in pairs {
                let face = source_face(&shape.faces, down)?.clone();
                if face.positions.len() != 4 {
                    return Err("F15 probe source flap has unreviewed topology".into());
                }
                flaps.insert(neutral, (face, order));
            }
            closures.push(source_face(&shape.faces, closure)?.clone());
        }
        Ok(Self {
            gear,
            brake,
            flaps,
            closures,
        })
    }
}
fn source_face(faces: &[Face], address: usize) -> AppResult<&Face> {
    faces
        .iter()
        .find(|f| f.address == address)
        .ok_or_else(|| format!("F15 probe source missing {address:x}").into())
}
fn branch(bytes: &[u8], word: usize, value: i32, addresses: &[usize]) -> AppResult<Vec<Face>> {
    let faces: Vec<_> = Shape::with_state(bytes, &[(word, value)].into())?
        .faces
        .into_iter()
        .filter(|f| addresses.contains(&f.address))
        .collect();
    if faces.len() != addresses.len() {
        return Err("F15 probe source branch has missing/duplicate reviewed faces".into());
    }
    Ok(faces)
}
struct Panel {
    faces: &'static [usize],
    roots: &'static [[f32; 3]],
    trailing: &'static [[f32; 3]],
    sign: f32,
    central_roll_only: bool,
}
macro_rules! panel {
    ($faces:expr, $roots:expr) => {
        Panel {
            faces: $faces,
            roots: $roots,
            trailing: &[],
            sign: 0.,
            central_roll_only: false,
        }
    };
    ($faces:expr, $roots:expr, $trailing:expr, $sign:expr) => {
        Panel {
            faces: $faces,
            roots: $roots,
            trailing: $trailing,
            sign: $sign,
            central_roll_only: false,
        }
    };
}
fn panels(control: Control) -> Vec<Panel> {
    match control {
        Control::Rudder => vec![
            panel!(
                &[0x2470, 0x5282, 0x24b8, 0x52df],
                &[[-22., -57., 5.], [-22., -52., 43.]],
                &[[-22., -60., 43.], [-22., -60., 44.]],
                1.
            ),
            panel!(
                &[0x24de, 0x4e3c, 0x2526, 0x4e99],
                &[[21., -57., 5.], [21., -52., 43.]],
                &[[21., -60., 43.], [21., -60., 44.]],
                1.
            ),
        ],
        Control::Elevator => vec![
            panel!(
                &[0x55df, 0x560c],
                &[[-22., -68., -1.], [-47., -65., -1.]],
                &[[-42., -71., -1.]],
                1.
            ),
            panel!(
                &[0x51a4, 0x51d1],
                &[[21., -68., -1.], [47., -65., -1.]],
                &[[41., -71., -1.]],
                1.
            ),
        ],
        Control::Aileron => vec![
            Panel {
                faces: &[0x54d7, 0x5575],
                roots: &[
                    [-42., -21.6, 3.85],
                    [-42., -21.6, 2.15],
                    [-56., -24.4, 3.15],
                    [-56., -24.4, 2.85],
                ],
                trailing: &[[-42., -28.45, 3.], [-56., -30.55, 3.]],
                sign: -1.,
                central_roll_only: true,
            },
            Panel {
                faces: &[0x50f0, 0x5111],
                roots: &[
                    [42., -21.6, 3.85],
                    [42., -21.6, 2.15],
                    [56., -24.4, 3.15],
                    [56., -24.4, 2.85],
                ],
                trailing: &[[42., -28.45, 3.], [56., -30.55, 3.]],
                sign: 1.,
                central_roll_only: true,
            },
        ],
        Control::Flaps => vec![
            panel!(
                &[0x5c3e, 0x5c5d],
                &[
                    [-39., -21., 2.],
                    [-39., -21., 4.],
                    [-22., -21., 2.],
                    [-22., -21., 4.]
                ],
                &[[-39., -28., 3.], [-22., -28., 3.]],
                -1.
            ),
            panel!(
                &[0x5b57, 0x5b76],
                &[
                    [39., -21., 2.],
                    [39., -21., 4.],
                    [21., -21., 2.],
                    [21., -21., 4.]
                ],
                &[[39., -28., 3.], [21., -28., 3.]],
                -1.
            ),
        ],
        Control::Brake => vec![panel!(&BRAKE, &[[-3., 28., 11.], [3., 28., 11.]])],
        Control::Gear => vec![
            panel!(
                &MAIN_LEFT,
                &[
                    [-13., -2., -11.],
                    [-13., 8., -11.],
                    [-15., 3., -11.],
                    [-9., 3., -11.]
                ]
            ),
            panel!(
                &MAIN_RIGHT,
                &[
                    [13., -2., -11.],
                    [13., 8., -11.],
                    [9., 3., -11.],
                    [15., 3., -11.]
                ]
            ),
            panel!(&[0x5a82, 0x5aa1, 0x5aee, 0x5b0d], &[[0., 64., -8.]]),
            panel!(&[0x5ac0, 0x5ad7], &[[-2., 52., -8.], [2., 52., -8.]]),
            panel!(
                &[0x573a, 0x5751, 0x5768, 0x577f],
                &[[-7., -4., -12.], [-7., 11., -12.]]
            ),
            panel!(
                &[0x5908, 0x591f, 0x5936, 0x594d],
                &[[7., -4., -12.], [7., 11., -12.]]
            ),
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
fn central_roll(face: &Face) -> bool {
    face.positions
        .iter()
        .all(|p| (42. - EPSILON..=56. + EPSILON).contains(&p[0].abs()))
}
fn witnesses(
    before: &BTreeMap<FaceKey, &Face>,
    after: &BTreeMap<FaceKey, &Face>,
    point: [f32; 3],
) -> Vec<[f32; 3]> {
    before
        .iter()
        .flat_map(|(key, source)| {
            source.positions.iter().enumerate().filter_map(|(i, p)| {
                (distance(*p, point) <= EPSILON)
                    .then(|| after.get(key).and_then(|f| f.positions.get(i)).copied())
                    .flatten()
            })
        })
        .collect()
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
    if value == 0. && matches!(control, Control::Gear | Control::Brake) {
        let addresses = if matches!(control, Control::Gear) {
            GEAR.as_slice()
        } else {
            BRAKE.as_slice()
        };
        if pose.iter().any(|f| addresses.contains(&f.address)) {
            if matches!(control, Control::Gear) {
                metric.reviewed_wheel_failed = true;
            } else {
                metric.reviewed_direction_failures += 1;
            }
        }
        return;
    }
    for (side, panel) in panels(control).iter().enumerate() {
        // Keep original occurrence keys, including clipped pieces with the same address.
        let before = selected(reference, panel.faces);
        let after = selected(pose, panel.faces);
        metric.reviewed_anchor_missing |= before.is_empty()
            || before.iter().any(|(key, f)| {
                after
                    .get(key)
                    .is_none_or(|g| g.positions.len() != f.positions.len())
            });
        for point in panel.roots {
            let actual = witnesses(&before, &after, *point);
            metric.reviewed_anchor_missing |= actual.is_empty();
            for p in actual {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*point, p) * scale);
            }
        }
        let before: BTreeMap<_, _> = before
            .into_iter()
            .filter(|(_, f)| !panel.central_roll_only || central_roll(f))
            .collect();
        let after: BTreeMap<_, _> = after
            .into_iter()
            .filter(|(key, _)| before.contains_key(key))
            .collect();
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if value == 0. {
            // Source vertices must survive neutral clipping; triangulation/order may differ.
            for original in raw.iter().filter(|f| panel.faces.contains(&f.address)) {
                for point in &original.positions {
                    metric.reviewed_neutral_mismatch |= !pose
                        .iter()
                        .filter(|f| f.address == original.address)
                        .any(|f| f.positions.iter().any(|p| distance(*point, *p) <= EPSILON));
                }
            }
        } else {
            let axis = if matches!(control, Control::Rudder) {
                0
            } else {
                2
            };
            for point in panel.trailing {
                let actual = witnesses(&before, &after, *point);
                metric.reviewed_direction_failures += usize::from(actual.is_empty());
                for p in actual {
                    let delta = (p[axis] - point[axis]) * scale;
                    metric.reviewed_direction_failures +=
                        usize::from(delta * panel.sign * value.signum() as f32 <= EPSILON);
                    if axis == 2 && side < 2 {
                        metric.reviewed_control_z_delta[side] = delta;
                    }
                }
            }
        }
    }
    if matches!(control, Control::Flaps) {
        flap_endpoints(source, pose, value, scale, metric);
    }
    if value == 1. {
        match control {
            Control::Gear => deployed_coverage(&source.gear, pose, scale, metric),
            Control::Brake => deployed_coverage(&source.brake, pose, scale, metric),
            _ => {}
        }
    }
    if matches!(control, Control::Gear) {
        wheels(reference, pose, value, scale, metric);
    }
}
fn flap_endpoints(source: &Sources, pose: &[Face], value: f64, scale: f32, metric: &mut Metrics) {
    // Endpoint disagreement is a control-witness failure, reported with the
    // legacy direction_failures counter rather than hiding it behind MOVED.
    if value == 0. {
        metric.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| CLOSURES.contains(&f.address)));
        return;
    }
    if value != 1. {
        return;
    }
    for (&address, (down, order)) in &source.flaps {
        let actual: Vec<_> = pose.iter().filter(|f| f.address == address).collect();
        if actual.len() != 1 || actual[0].positions.len() != 4 {
            metric.reviewed_direction_failures += 1;
            continue;
        }
        for (i, target) in order.iter().enumerate() {
            metric.reviewed_direction_failures += usize::from(
                distance(actual[0].positions[i], down.positions[*target]) * scale > EPSILON,
            );
        }
    }
    for closure in &source.closures {
        let actual: Vec<_> = pose
            .iter()
            .filter(|f| f.address == closure.address)
            .collect();
        if actual.len() != 1 || actual[0].positions.len() != closure.positions.len() {
            metric.reviewed_direction_failures += 1;
            continue;
        }
        for (a, b) in actual[0].positions.iter().zip(&closure.positions) {
            metric.reviewed_direction_failures += usize::from(distance(*a, *b) * scale > EPSILON);
        }
    }
}
fn deployed_coverage(source: &[Face], pose: &[Face], scale: f32, metric: &mut Metrics) {
    for original in source {
        for point in &original.positions {
            metric.reviewed_direction_failures += usize::from(
                !pose
                    .iter()
                    .filter(|f| f.address == original.address)
                    .any(|f| {
                        f.positions
                            .iter()
                            .any(|p| distance(*point, *p) * scale <= EPSILON)
                    }),
            );
        }
    }
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    let after = keyed(pose);
    let mut counts = [0; 3];
    let mut right = f32::INFINITY;
    let mut left = f32::NEG_INFINITY;
    for (key, original) in keyed(reference) {
        let side = if MAIN_LEFT.contains(&key.0) {
            0
        } else if MAIN_RIGHT.contains(&key.0) {
            1
        } else if NOSE.contains(&key.0) {
            2
        } else {
            continue;
        };
        let cut = if side == 2 { -20. } else { -18. };
        if side != 2 && original.positions.iter().any(|p| p[2] > cut + EPSILON) {
            continue;
        }
        counts[side] += 1;
        let Some(actual) = after.get(&key) else {
            metric.reviewed_wheel_failed = true;
            continue;
        };
        if original.positions.len() != actual.positions.len() {
            metric.reviewed_wheel_failed = true;
            continue;
        }
        for p in &actual.positions {
            if side == 0 {
                left = left.max(p[0] * scale);
            } else if side == 1 {
                right = right.min(p[0] * scale);
            }
        }
        for i in 0..original.positions.len() {
            for j in i + 1..original.positions.len() {
                metric.reviewed_wheel_rigidity_error = metric.reviewed_wheel_rigidity_error.max(
                    (distance(original.positions[i], original.positions[j])
                        - distance(actual.positions[i], actual.positions[j]))
                    .abs()
                        * scale,
                );
            }
        }
        if travel < 1e-5 {
            // All 24 complete rigid nose/door corners fit neutral body sections.
            // This envelope is broader than the painted wheel footprint.
            let outside = actual.positions.iter().any(|p| {
                if side == 2 {
                    p[0].abs() > 2. + 1e-3
                        || !(39. - 1e-3..=64. + 1e-3).contains(&p[1])
                        || !(-8. - 1e-3..=6. + 1e-3).contains(&p[2])
                } else {
                    !(9. - 1e-3..=15. + 1e-3).contains(&p[0].abs())
                        || !(-15. - 1e-3..=-6. + 1e-3).contains(&p[1])
                        || !(-11. - 1e-3..=-1. + 1e-3).contains(&p[2])
                }
            });
            metric.reviewed_wheel_failed |= outside;
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |= counts != [4, 4, 6]
        || right < 0.35
        || left > -0.35
        || right - left < 0.70
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty_sources() -> Sources {
        Sources {
            gear: Vec::new(),
            brake: Vec::new(),
            flaps: BTreeMap::new(),
            closures: Vec::new(),
        }
    }
    fn synthetic(address: usize, points: Vec<[f32; 3]>) -> Face {
        super::super::tests::face(address, points)
    }
    #[test]
    fn source_neutral_coverage_allows_split_faces_and_detects_lost_source_vertices() {
        let raw = vec![synthetic(
            0x55df,
            vec![
                [-22., -68., -1.],
                [-47., -65., -1.],
                [-41., -72., -1.],
                [-22., -39., -1.],
            ],
        )];
        let split = vec![
            synthetic(
                0x55df,
                vec![
                    raw[0].positions[0],
                    raw[0].positions[1],
                    raw[0].positions[2],
                ],
            ),
            synthetic(
                0x55df,
                vec![
                    raw[0].positions[0],
                    raw[0].positions[1],
                    raw[0].positions[3],
                ],
            ),
            synthetic(
                0x51a4,
                vec![[21., -68., -1.], [47., -65., -1.], [40., -72., -1.]],
            ),
        ];
        let mut metric = Metrics::default();
        check(
            Control::Elevator,
            0.,
            (&raw, &split, &split),
            1. / 3.,
            &empty_sources(),
            &mut metric,
        );
        assert!(!metric.reviewed_neutral_mismatch);
        let mut missing = split.clone();
        missing[1].positions[2] = [0.; 3];
        let mut metric = Metrics::default();
        check(
            Control::Elevator,
            0.,
            (&raw, &split, &missing),
            1. / 3.,
            &empty_sources(),
            &mut metric,
        );
        assert!(metric.reviewed_neutral_mismatch);
    }
    #[test]
    fn source_down_endpoints_are_required_even_for_moved_flaps() {
        let mut sources = empty_sources();
        let down = synthetic(
            1,
            vec![[1., 2., 3.], [4., 2., 3.], [4., 1., -1.], [1., 1., -1.]],
        );
        sources.flaps.insert(FLAPS[0], (down.clone(), [0, 1, 2, 3]));
        let mut actual = down.clone();
        actual.address = FLAPS[0];
        let mut metric = Metrics::default();
        flap_endpoints(&sources, &[actual.clone()], 1., 1., &mut metric);
        assert_eq!(metric.reviewed_direction_failures, 0);
        actual.positions[2][2] -= 1.;
        let mut metric = Metrics::default();
        flap_endpoints(&sources, &[actual], 1., 1., &mut metric);
        assert!(metric.reviewed_direction_failures > 0);
    }
    #[test]
    fn lower_sheet_gate_rejects_missing_stretch_and_approved_stow_violations() {
        let mut source = Vec::new();
        for address in MAIN_LEFT.iter().chain(&MAIN_RIGHT).chain(&NOSE) {
            let (x, cut) = if MAIN_LEFT.contains(address) {
                (-13., -18.)
            } else if MAIN_RIGHT.contains(address) {
                (13., -18.)
            } else {
                (0., -20.)
            };
            source.push(synthetic(
                *address,
                vec![
                    [x - 1., 1., cut - 1.],
                    [x + 1., 1., cut - 1.],
                    [x + 1., 4., cut - 5.],
                    [x - 1., 4., cut - 5.],
                ],
            ));
        }
        let mut metric = Metrics::default();
        wheels(&source, &source, 0.5, 1. / 3., &mut metric);
        assert!(!metric.reviewed_wheel_failed);
        let mut metric = Metrics::default();
        wheels(
            &source,
            &source[..source.len() - 1],
            0.5,
            1. / 3.,
            &mut metric,
        );
        assert!(metric.reviewed_wheel_failed);
        let mut stretched = source.clone();
        stretched[0].positions[0][0] -= 1.;
        let mut metric = Metrics::default();
        wheels(&source, &stretched, 0.5, 1. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
        let mut metric = Metrics::default();
        wheels(&source, &source, 1e-6, 1. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
    }
}
