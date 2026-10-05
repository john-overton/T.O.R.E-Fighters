//! Independent A310 source attachment, endpoint, direction and atlas witnesses.
//! Reads inert branch geometry; never imports the rig's transform or constants.
use super::*;
use tore_formats::shape::Shape;

const LEFT: [usize; 4] = [0x49f1, 0x4a18, 0x4a3f, 0x4a62];
const RIGHT: [usize; 6] = [0x48e4, 0x490b, 0x4932, 0x4951, 0x4970, 0x498b];
const NOSE: [usize; 4] = [0x4aca, 0x4ae9, 0x4b08, 0x4b27];

pub(super) struct Sources {
    gear: Vec<Face>,
    flaps: BTreeMap<usize, (Face, [usize; 4])>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let shape = Shape::with_state(bytes, &[(0x6160, 1)].into())?;
        let gear: Vec<_> = shape
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect();
        if gear.len() != 14 {
            return Err("A310 probe gear branch does not contain 14 reviewed faces".into());
        }
        let mut flaps = BTreeMap::new();
        for (word, pairs) in [
            (
                0x616c,
                [
                    (0x4704, 0x46ba, [1, 2, 3, 0]),
                    (0x472b, 0x469b, [3, 0, 1, 2]),
                ],
            ),
            (
                0x6172,
                [
                    (0x4622, 0x45d8, [3, 0, 1, 2]),
                    (0x4649, 0x45b9, [2, 3, 0, 1]),
                ],
            ),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            for (neutral, deployed, order) in pairs {
                let face = down
                    .faces
                    .iter()
                    .find(|f| f.address == deployed)
                    .ok_or("A310 probe missing flap down face")?
                    .clone();
                if face.positions.len() != 4 {
                    return Err("A310 probe flap topology changed".into());
                }
                flaps.insert(neutral, (face, order));
            }
        }
        Ok(Self { gear, flaps })
    }
}
struct Panel {
    faces: &'static [usize],
    roots: &'static [[f32; 3]],
    trailing: &'static [[f32; 3]],
    sign: f32,
}
macro_rules! panel {
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
        Control::Rudder => vec![panel!(
            &[0x484b, 0x4872],
            &[[0., -105., 17.], [0., -127., 57.]],
            &[[0., -115., 17.], [0., -130., 57.]],
            1.
        )],
        Control::Elevator => vec![
            panel!(
                &[0x3be9, 0x3c04, 0x3c64, 0x3c80],
                &[[-7., -95., 11.], [-3., -118., 11.]],
                &[[-18., -124., 11.], [-41., -133., 11.]],
                1.
            ),
            panel!(
                &[0x2f6a, 0x2f85, 0x307a, 0x3096],
                &[[7., -95., 11.], [3., -118., 11.]],
                &[[18., -124., 11.], [41., -133., 11.]],
                1.
            ),
        ],
        Control::Aileron => vec![
            panel!(
                &[0x411a, 0x413b],
                &[[-45., -22., 1.], [-45., -22., -2.], [-115., -56., 3.]],
                &[[-45., -32., -1.]],
                -1.
            ),
            panel!(
                &[0x32b3, 0x3575],
                &[[45., -22., 1.], [45., -22., -2.], [115., -56., 3.]],
                &[[45., -32., -1.]],
                1.
            ),
        ],
        Control::Flaps => vec![
            panel!(
                &[0x4704, 0x472b],
                &[
                    [-45., -22., -2.],
                    [-45., -22., 1.],
                    [-9., -22., -5.],
                    [-11., -22., -1.]
                ],
                &[[-11., -32., -3.], [-45., -32., -1.]],
                -1.
            ),
            panel!(
                &[0x4622, 0x4649],
                &[
                    [45., -22., -2.],
                    [45., -22., 1.],
                    [9., -22., -5.],
                    [11., -22., -1.]
                ],
                &[[11., -32., -3.], [45., -32., -1.]],
                -1.
            ),
        ],
        Control::Gear => vec![
            panel!(
                &LEFT,
                &[
                    [-6., -29., -7.],
                    [-6., -17., -7.],
                    [-1., -23., -9.],
                    [-8., -23., -5.],
                    [-11., -23., -5.]
                ],
                &[],
                0.
            ),
            panel!(
                &RIGHT,
                &[
                    [6., -29., -7.],
                    [6., -17., -7.],
                    [1., -23., -9.],
                    [8., -23., -5.],
                    [11., -23., -5.]
                ],
                &[],
                0.
            ),
            // Atlas side-view bounding corners are transparent image margins.
            // This front-view line attaches the complete rigid nose assembly.
            panel!(&NOSE, &[[-2., 63., -8.], [2., 63., -8.]], &[], 0.),
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
    if matches!(control, Control::Gear) && value == 0. {
        metric.reviewed_wheel_failed |= pose.iter().any(|f| {
            LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
        });
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
        for root in panel.roots {
            let points = witnesses(&before, &after, *root);
            metric.reviewed_anchor_missing |= points.is_empty();
            for p in points {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*root, p) * scale);
            }
        }
        if let Some(gap) = shared_vertex_gaps(&before, &after, scale).first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if value == 0. {
            coverage(
                &raw.iter()
                    .filter(|f| panel.faces.contains(&f.address))
                    .cloned()
                    .collect::<Vec<_>>(),
                pose,
                scale,
                metric,
            );
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
    if matches!(control, Control::Flaps) && value == 1. {
        for (&address, (down, order)) in &source.flaps {
            let actual: Vec<_> = pose.iter().filter(|f| f.address == address).collect();
            if actual.len() != 1 || actual[0].positions.len() != 4 {
                metric.reviewed_direction_failures += 1;
                continue;
            }
            for (i, &j) in order.iter().enumerate() {
                metric.reviewed_direction_failures += usize::from(
                    distance(actual[0].positions[i], down.positions[j]) * scale > EPSILON,
                );
            }
        }
    }
    if matches!(control, Control::Gear) {
        if value == 1. {
            coverage(&source.gear, pose, scale, metric);
        }
        wheels(reference, pose, value, scale, metric);
    }
}
fn coverage(source: &[Face], pose: &[Face], scale: f32, metric: &mut Metrics) {
    for face in source {
        for point in &face.positions {
            metric.reviewed_neutral_mismatch |=
                !pose.iter().filter(|f| f.address == face.address).any(|f| {
                    f.positions
                        .iter()
                        .any(|p| distance(*point, *p) * scale <= EPSILON)
                });
        }
    }
}
fn wheels(reference: &[Face], pose: &[Face], travel: f64, scale: f32, metric: &mut Metrics) {
    let after = keyed(pose);
    let mut counts = [0; 3];
    let mut left = f32::NEG_INFINITY;
    let mut right = f32::INFINITY;
    for (key, original) in keyed(reference) {
        let side = if LEFT.contains(&key.0) {
            0
        } else if RIGHT.contains(&key.0) {
            1
        } else if NOSE.contains(&key.0) {
            2
        } else {
            continue;
        };
        // Original atlas crops place main wheel tops below -13. The -12
        // plane conservatively encloses both front and side wheel images.
        // Every nose strut/wheel/brace polygon is now rigid, not just its tyre.
        if side != 2 && original.positions.iter().any(|p| p[2] > -12. + EPSILON) {
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
            metric.reviewed_wheel_failed |= actual.positions.iter().any(|p| {
                if side == 2 {
                    // Source underside runs from +/-3,59,-10 to +/-2,71,-7.
                    // All rigid nose pixels AND quad margins must fit above it.
                    p[0].abs() > 2. + 1e-3
                        || !(59. - 1e-3..=71. + 1e-3).contains(&p[1])
                        || !(-10. - 1e-3..=6. + 1e-3).contains(&p[2])
                        || p[2] < -10. + 0.25 * (p[1] - 59.) - 1e-3
                } else {
                    p[0].abs() > 11. + 1e-3
                        || !(-45. - 1e-3..=-33. + 1e-3).contains(&p[1])
                        || !(-2. - 1e-3..=7. + 1e-3).contains(&p[2])
                }
            });
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |= counts != [4, 4, 4]
        || right < 0.65
        || left > -0.65
        || right - left < 1.30
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic(address: usize, positions: Vec<[f32; 3]>) -> Face {
        super::super::tests::face(address, positions)
    }
    #[test]
    fn rejected_translation_bow_tie_is_detected_independently() {
        let source = synthetic(
            0x4aca,
            vec![
                [0., 58., -9.],
                [0., 66., -6.],
                [0., 66., -13.],
                [0., 58., -13.],
            ],
        );
        let crossed = synthetic(
            0x4aca,
            vec![
                [0., 58., -9.],
                [0., 66., -6.],
                [0., 66., -7.],
                [0., 58., -7.],
            ],
        );
        assert!(!planar_crossing(&source, 2. / 3.));
        assert!(planar_crossing(&crossed, 2. / 3.));
    }
    #[test]
    fn gear_witness_rejects_missing_rigid_nose_and_changed_strut_dimensions() {
        let mut source = Vec::new();
        for (ids, x) in [(&LEFT[..], -6.), (&RIGHT[..4], 6.), (&NOSE[..], 0.)] {
            for &address in ids {
                source.push(synthetic(
                    address,
                    vec![
                        [x - 1., 60., -14.],
                        [x + 1., 60., -14.],
                        [x + 1., 63., -20.],
                        [x - 1., 63., -20.],
                    ],
                ));
            }
        }
        let mut metric = Metrics::default();
        wheels(&source, &source, 0.5, 2. / 3., &mut metric);
        assert!(!metric.reviewed_wheel_failed);
        let mut changed = source.clone();
        changed.last_mut().unwrap().positions[0][2] += 1.;
        let mut metric = Metrics::default();
        wheels(&source, &changed, 0.5, 2. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
        let mut metric = Metrics::default();
        wheels(
            &source,
            &source[..source.len() - 1],
            0.5,
            2. / 3.,
            &mut metric,
        );
        assert!(metric.reviewed_wheel_failed);
    }
    #[test]
    fn gear_stow_requires_source_nose_underside_clearance() {
        let mut reference = Vec::new();
        let mut stowed = Vec::new();
        for (ids, x, nose) in [
            (&LEFT[..], -6., false),
            (&RIGHT[..4], 6., false),
            (&NOSE[..], 0., true),
        ] {
            for &address in ids {
                let y = if nose { 60. } else { -23. };
                let original = synthetic(
                    address,
                    vec![
                        [x - 1., y, -14.],
                        [x + 1., y, -14.],
                        [x + 1., y + 3., -20.],
                        [x - 1., y + 3., -20.],
                    ],
                );
                let mut moved = original.clone();
                for p in &mut moved.positions {
                    p[1] -= if nose { 0. } else { 16. };
                    p[2] += if nose { 14. } else { 19. };
                }
                reference.push(original);
                stowed.push(moved);
            }
        }
        let mut metric = Metrics::default();
        wheels(&reference, &stowed, 1e-6, 2. / 3., &mut metric);
        assert!(!metric.reviewed_wheel_failed);
        for f in stowed.iter_mut().filter(|f| NOSE.contains(&f.address)) {
            for p in &mut f.positions {
                p[2] -= 5.;
            }
        }
        let mut metric = Metrics::default();
        wheels(&reference, &stowed, 1e-6, 2. / 3., &mut metric);
        assert!(metric.reviewed_wheel_failed);
        assert!(metric.reviewed_wheel_rigidity_error < EPSILON);
    }
}
