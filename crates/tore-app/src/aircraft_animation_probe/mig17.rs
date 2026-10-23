//! Independent M17.SH source attachments and explicitly fitted control witnesses.
//! The flap fit preserves neutral fronts, so only its source trailing targets
//! are down-endpoint requirements. No animation transform is duplicated here.
use super::*;
use tore_formats::shape::Shape;

const GEAR: [usize; 22] = [
    0x4690, 0x46af, 0x4713, 0x4732, 0x4751, 0x4770, 0x47bc, 0x47db, 0x483f, 0x485e, 0x487d, 0x489c,
    0x491c, 0x4933, 0x49b3, 0x49d2, 0x4a16, 0x4a35, 0x4aaf, 0x4ace, 0x4aed, 0x4b0c,
];
const BRAKE: [usize; 10] = [
    0x44af, 0x44ce, 0x44f5, 0x4514, 0x4535, 0x45c2, 0x45e1, 0x4608, 0x4627, 0x4648,
];
const MAIN_RIGHT: [usize; 2] = [0x4713, 0x4732];
const MAIN_LEFT: [usize; 2] = [0x483f, 0x485e];
const NOSE: [usize; 2] = [0x4a16, 0x4a35];
const RAW_DOWN: [usize; 6] = [0x4be6, 0x4c0d, 0x4c2c, 0x4b56, 0x4b7d, 0x4b9c];
const YAW: [usize; 8] = [
    0x22b7, 0x3aea, 0x23e7, 0x4181, 0x2423, 0x41e4, 0x2495, 0x434a,
];
const STATIC_WELLS: [usize; 4] = [0x4aaf, 0x4ace, 0x4aed, 0x4b0c];

pub(super) struct Sources {
    gear: Vec<Face>,
    brake: Vec<Face>,
    flap_targets: [[f32; 3]; 4],
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let gear = source_branch(bytes, 0x605c, 1, &GEAR)?;
        let brake = source_branch(bytes, 0x6056, 1, &BRAKE)?;
        let mut flap_targets = [[0.; 3]; 4];
        for (side, (word, address, indices)) in [(0x6068, 0x4be6, [0, 3]), (0x606e, 0x4b56, [0, 1])]
            .into_iter()
            .enumerate()
        {
            let shape = Shape::with_state(bytes, &[(word, -1)].into())?;
            let face = shape
                .faces
                .iter()
                .find(|f| f.address == address)
                .ok_or("M17 probe source down flap missing")?;
            if face.positions.len() < 4 {
                return Err("M17 probe source down flap topology changed".into());
            }
            for (i, index) in indices.into_iter().enumerate() {
                flap_targets[side * 2 + i] = face.positions[index];
            }
        }
        Ok(Self {
            gear,
            brake,
            flap_targets,
        })
    }
}
fn source_branch(
    bytes: &[u8],
    word: usize,
    value: i32,
    addresses: &[usize],
) -> AppResult<Vec<Face>> {
    let faces: Vec<_> = Shape::with_state(bytes, &[(word, value)].into())?
        .faces
        .into_iter()
        .filter(|f| addresses.contains(&f.address))
        .collect();
    if faces.len() != addresses.len() {
        return Err("M17 probe source branch missing/duplicates reviewed faces".into());
    }
    Ok(faces)
}
struct Panel {
    faces: &'static [usize],
    roots: &'static [[f32; 3]],
    trailing: &'static [[f32; 3]],
    sign: f32,
}
macro_rules! panel {
    ($faces:expr,$roots:expr) => {
        Panel {
            faces: $faces,
            roots: $roots,
            trailing: &[],
            sign: 0.,
        }
    };
    ($faces:expr,$roots:expr,$trailing:expr,$sign:expr) => {
        Panel {
            faces: $faces,
            roots: $roots,
            trailing: $trailing,
            sign: $sign,
        }
    };
}
fn panels(control: Control) -> Vec<Panel> {
    match control {
        Control::Rudder => vec![
            panel!(
                &[0x22b7, 0x23e7, 0x2423, 0x2495],
                &[
                    [-1., -41., 4.],
                    [-1., -57., 23.],
                    [0., -56., 25.],
                    [0., -47., 3.]
                ],
                &[[0., -50., 4.], [0., -63., 23.], [0., -60., 25.]],
                1.
            ),
            panel!(
                &[0x3aea, 0x4181, 0x41e4, 0x434a],
                &[
                    [1., -41., 4.],
                    [1., -57., 23.],
                    [0., -56., 25.],
                    [0., -47., 3.]
                ],
                &[[0., -50., 4.], [0., -63., 23.], [0., -60., 25.]],
                1.
            ),
        ],
        Control::Elevator => vec![
            panel!(
                &[0x2f06, 0x2f27],
                &[[0., -43., 15.], [0., -55., 15.]],
                &[[-14., -64., 15.], [-16., -60., 15.]],
                1.
            ),
            panel!(
                &[0x3b11, 0x3b44],
                &[[0., -43., 15.], [0., -55., 15.]],
                &[[14., -64., 15.], [16., -60., 15.]],
                1.
            ),
        ],
        Control::Aileron => vec![
            panel!(
                &[0x2f9a, 0x2fbd],
                &[
                    [-28., -12., -1.],
                    [-42., -23., -2.],
                    [-28., -13., -2.],
                    [-42., -23., -3.]
                ],
                &[[-28., -21., -1.], [-38., -30., -2.]],
                -1.
            ),
            panel!(
                &[0x3bb5, 0x3bdc],
                &[
                    [28., -13., -1.],
                    [42., -23., -2.],
                    [28., -13., -2.],
                    [42., -23., -3.]
                ],
                &[[28., -21., -1.], [38., -30., -2.]],
                1.
            ),
        ],
        Control::Flaps => vec![
            panel!(
                &[0x2e24, 0x2f54],
                &[
                    [-13., -1., -1.],
                    [-28., -13., -2.],
                    [-13., 0., 0.],
                    [-28., -12., -1.]
                ],
                &[[-13., -7., -1.], [-28., -21., -1.]],
                -1.
            ),
            panel!(
                &[0x33e8, 0x3b92],
                &[
                    [13., -1., -1.],
                    [28., -13., -2.],
                    [13., -1., 0.],
                    [28., -13., -1.]
                ],
                &[[13., -7., -1.], [28., -21., -1.]],
                -1.
            ),
        ],
        Control::Brake => vec![
            panel!(
                &[0x44ce, 0x44f5, 0x4514, 0x4535],
                &[[4., -30., -4.], [5., -31., 0.], [2., -31., -6.]],
                &[[11., -36., -7.]],
                1.
            ),
            panel!(
                &[0x45e1, 0x4608, 0x4627, 0x4648],
                &[[-4., -30., -4.], [-5., -31., 0.], [-2., -31., -6.]],
                &[[-11., -36., -7.]],
                -1.
            ),
        ],
        Control::Gear => vec![
            panel!(&MAIN_RIGHT, &[[20., 0., -2.], [20., 7., -2.]]),
            panel!(&MAIN_LEFT, &[[-20., 0., -2.], [-20., 7., -2.]]),
            panel!(&[0x4690, 0x46af], &[[7., 2., -2.], [7., 10., -2.]]),
            panel!(&[0x47bc, 0x47db], &[[-7., 2., -2.], [-7., 10., -2.]]),
            panel!(&[0x4751, 0x4770], &[[21., 1., -2.], [21., 9., -2.]]),
            panel!(&[0x487d, 0x489c], &[[-21., 1., -2.], [-21., 9., -2.]]),
            panel!(&[0x491c, 0x4933], &[[2., 32., -6.], [2., 40., -5.]]),
            panel!(&[0x49b3, 0x49d2], &[[-2., 32., -6.], [-2., 40., -5.]]),
            panel!(&NOSE, &[[0., 34.2973, -6.]]),
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
        let ids = if matches!(control, Control::Gear) {
            GEAR.as_slice()
        } else {
            BRAKE.as_slice()
        };
        if pose.iter().any(|f| ids.contains(&f.address)) {
            if matches!(control, Control::Gear) {
                metric.reviewed_wheel_failed = true;
            } else {
                metric.reviewed_direction_failures += 1;
            }
        }
        return;
    }
    for (side, panel) in panels(control).iter().enumerate() {
        let before = selected(reference, panel.faces);
        let after = selected(pose, panel.faces);
        metric.reviewed_anchor_missing |= before.is_empty()
            || before.iter().any(|(key, f)| {
                after
                    .get(key)
                    .is_none_or(|g| g.positions.len() != f.positions.len())
            });
        for point in panel.roots {
            let actual = f4::witness_positions(&before, &after, *point);
            metric.reviewed_anchor_missing |= actual.is_empty();
            for p in actual {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*point, p) * scale);
            }
        }
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if value == 0. {
            for original in raw.iter().filter(|f| panel.faces.contains(&f.address)) {
                for point in &original.positions {
                    metric.reviewed_neutral_mismatch |= !pose
                        .iter()
                        .filter(|f| f.address == original.address)
                        .any(|f| f.positions.iter().any(|p| distance(*point, *p) <= EPSILON));
                }
            }
        } else if !matches!(control, Control::Brake) || value < 1. {
            let axis = if matches!(control, Control::Rudder | Control::Brake) {
                0
            } else {
                2
            };
            for point in panel.trailing {
                let actual = f4::witness_positions(&before, &after, *point);
                metric.reviewed_direction_failures += usize::from(actual.is_empty());
                for p in actual {
                    let delta = (p[axis] - point[axis]) * scale;
                    let expected = if matches!(control, Control::Brake) {
                        -panel.sign
                    } else {
                        panel.sign * value.signum() as f32
                    };
                    metric.reviewed_direction_failures += usize::from(delta * expected <= EPSILON);
                    if axis == 2 && side < 2 {
                        metric.reviewed_control_z_delta[side] = delta;
                    }
                }
            }
        }
    }
    if matches!(control, Control::Rudder) {
        // Distinct thick roots have different X. Their common trailing/cap
        // points must nevertheless be shared across BOTH sides.
        let before = selected(reference, &YAW);
        let after = selected(pose, &YAW);
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
    }
    if matches!(control, Control::Flaps) {
        metric.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| RAW_DOWN.contains(&f.address)));
        if value == 1. {
            for (side, panel) in panels(control).iter().enumerate() {
                let before = selected(reference, panel.faces);
                let after = selected(pose, panel.faces);
                for (i, point) in panel.trailing.iter().enumerate() {
                    let actual = f4::witness_positions(&before, &after, *point);
                    metric.reviewed_direction_failures += usize::from(actual.is_empty());
                    for p in actual {
                        metric.reviewed_direction_failures += usize::from(
                            distance(p, source.flap_targets[side * 2 + i]) * scale > EPSILON,
                        );
                    }
                }
            }
        }
    }
    if matches!(control, Control::Brake | Control::Gear) {
        let static_ids = if matches!(control, Control::Gear) {
            STATIC_WELLS.as_slice()
        } else {
            &[0x44af, 0x45c2]
        };
        let before = selected(reference, static_ids);
        let after = selected(pose, static_ids);
        metric.reviewed_anchor_missing |= before.is_empty()
            || before.iter().any(|(key, face)| {
                after
                    .get(key)
                    .is_none_or(|actual| actual.positions.len() != face.positions.len())
            });
        for (key, f) in before {
            if let Some(g) = after.get(&key) {
                for (a, b) in f.positions.iter().zip(&g.positions) {
                    metric.max_reviewed_anchor_gap =
                        metric.max_reviewed_anchor_gap.max(distance(*a, *b) * scale);
                }
            }
        }
        if value == 1. {
            let deployed = if matches!(control, Control::Gear) {
                &source.gear
            } else {
                &source.brake
            };
            for f in deployed {
                for point in &f.positions {
                    metric.reviewed_direction_failures +=
                        usize::from(!pose.iter().filter(|g| g.address == f.address).any(|g| {
                            g.positions
                                .iter()
                                .any(|p| distance(*point, *p) * scale <= EPSILON)
                        }));
                }
            }
        }
    }
    if matches!(control, Control::Gear) {
        wheels(reference, pose, value, scale, metric);
    }
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    metric.reviewed_wheel_failed |= pose
        .iter()
        .filter(|face| NOSE.contains(&face.address))
        .any(nose_strip_crosses_itself);
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
            metric.reviewed_wheel_failed |= actual.positions.iter().any(|p| {
                if side == 2 {
                    p[0].abs() > 1e-3
                        || !(26. - 1e-3..=36. + 1e-3).contains(&p[1])
                        || !(-8. - 1e-3..=1. + 1e-3).contains(&p[2])
                } else {
                    !(9. - 1e-3..=20. + 1e-3).contains(&p[0].abs())
                        || !(-1e-3..=7. + 1e-3).contains(&p[1])
                        || (p[2] + 2.).abs() > 1e-3
                }
            });
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |= counts != [2, 2, 2]
        || right < 0.35
        || left > -0.35
        || right - left < 0.70
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}

fn nose_strip_crosses_itself(face: &Face) -> bool {
    if face.positions.len() < 4 {
        return false;
    }
    let p: Vec<_> = face.positions.iter().map(|p| [p[1], p[2]]).collect();
    let orient = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let crosses = |a, b, c, d| {
        orient(a, b, c) * orient(a, b, d) < -EPSILON * EPSILON
            && orient(c, d, a) * orient(c, d, b) < -EPSILON * EPSILON
    };
    for i in 0..p.len() {
        let next_i = (i + 1) % p.len();
        for j in i + 1..p.len() {
            let next_j = (j + 1) % p.len();
            if next_i == j || next_j == i {
                continue;
            }
            if crosses(p[i], p[next_i], p[j], p[next_j]) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic(a: usize, p: Vec<[f32; 3]>) -> Face {
        super::super::tests::face(a, p)
    }
    fn sources() -> Sources {
        Sources {
            gear: Vec::new(),
            brake: Vec::new(),
            flap_targets: [
                [-13., -7., -4.],
                [-28., -20., -5.],
                [13., -7., -4.],
                [28., -20., -5.],
            ],
        }
    }
    #[test]
    fn crossed_connector_is_rejected_even_if_the_lower_wheel_stays_rigid() {
        let crossed = synthetic(
            NOSE[0],
            vec![[0., 0., 0.], [0., 4., 0.], [0., 0., 2.], [0., 4., 2.]],
        );
        let ordinary = synthetic(
            NOSE[0],
            vec![[0., 0., 0.], [0., 4., 0.], [0., 4., 2.], [0., 0., 2.]],
        );
        assert!(nose_strip_crosses_itself(&crossed));
        assert!(!nose_strip_crosses_itself(&ordinary));
    }
    #[test]
    fn down_trailing_target_does_not_authorize_moving_the_neutral_flap_front() {
        let old = vec![
            synthetic(
                0x2e24,
                vec![
                    [-13., -1., -1.],
                    [-28., -13., -2.],
                    [-13., -7., -1.],
                    [-28., -21., -1.],
                    [-19.25, -8.75, 1.125],
                ],
            ),
            synthetic(
                0x2f54,
                vec![
                    [-13., 0., 0.],
                    [-28., -12., -1.],
                    [-13., -7., -1.],
                    [-28., -21., -1.],
                    [-19.25, -8.75, 1.125],
                ],
            ),
            synthetic(
                0x33e8,
                vec![
                    [13., -1., -1.],
                    [28., -13., -2.],
                    [13., -7., -1.],
                    [28., -21., -1.],
                    [19.25, -8.75, 1.125],
                ],
            ),
            synthetic(
                0x3b92,
                vec![
                    [13., -1., 0.],
                    [28., -13., -1.],
                    [13., -7., -1.],
                    [28., -21., -1.],
                    [19.25, -8.75, 1.125],
                ],
            ),
        ];
        let mut actual = old.clone();
        for (i, f) in actual.iter_mut().enumerate() {
            let side = usize::from(i >= 2);
            f.positions[2] = sources().flap_targets[side * 2];
            f.positions[3] = sources().flap_targets[side * 2 + 1];
        }
        let mut metric = Metrics::default();
        check(
            Control::Flaps,
            1.,
            (&old, &old, &actual),
            1. / 3.,
            &sources(),
            &mut metric,
        );
        assert!(!metric.reviewed_anchor_missing);
        assert_eq!(metric.max_reviewed_anchor_gap, 0.);
        assert_eq!(metric.reviewed_direction_failures, 0);
        actual[0].positions[0][2] -= 1.;
        let mut metric = Metrics::default();
        check(
            Control::Flaps,
            1.,
            (&old, &old, &actual),
            1. / 3.,
            &sources(),
            &mut metric,
        );
        assert!(metric.max_reviewed_anchor_gap > EPSILON);
    }
    #[test]
    fn wheel_gate_rejects_missing_stretched_and_out_of_stow_geometry() {
        let mut source = Vec::new();
        for address in MAIN_LEFT.iter().chain(&MAIN_RIGHT).chain(&NOSE) {
            let x = if MAIN_LEFT.contains(address) {
                -20.
            } else if MAIN_RIGHT.contains(address) {
                20.
            } else {
                0.
            };
            source.push(synthetic(
                *address,
                vec![
                    [x, 1., -10.5],
                    [x, 4., -10.5],
                    [x, 4., -13.5],
                    [x, 1., -13.5],
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
        stretched[0].positions[0][1] -= 2.;
        let mut metric = Metrics::default();
        wheels(&source, &stretched, 0.5, 1. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
        let mut metric = Metrics::default();
        wheels(&source, &source, 1e-6, 1. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
    }
}
