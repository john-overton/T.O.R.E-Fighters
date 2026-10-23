//! Independent source-coordinate witnesses for the reviewed F-4 shape family.
use super::*;

struct Panel {
    faces: &'static [usize],
    roots: &'static [[f32; 3]],
    trailing: &'static [[f32; 3]],
    direction: f32,
}
macro_rules! panel {
    ($faces:expr, $roots:expr) => {
        Panel {
            faces: $faces,
            roots: $roots,
            trailing: &[],
            direction: 0.,
        }
    };
    ($faces:expr, $roots:expr, $trailing:expr, $direction:expr) => {
        Panel {
            faces: $faces,
            roots: $roots,
            trailing: $trailing,
            direction: $direction,
        }
    };
}

// Exact source identities/coordinates from the hash-verified geometry review.
// Direction expectations describe the independently documented fitted law.
// No angles, pivots, rotations or vertex morph algorithms are reproduced here.
fn panels(id: AircraftId, control: Control) -> Vec<Panel> {
    use AircraftId as Id;
    let j = matches!(id, Id::F4B | Id::F4J);
    match (j, id, control) {
        (true, _, Control::Rudder) => vec![panel!(
            &[0x4a55, 0x4a7c],
            &[[0., -79., 10.], [0., -88., 29.]],
            &[[0., -90., 9.], [0., -91., 29.]],
            1.
        )],
        (_, Id::F4E, Control::Rudder) => vec![panel!(
            &[0x41ef, 0x420e],
            &[[0., -57., 8.], [0., -62., 21.]],
            &[[0., -67., 8.], [0., -68., 21.]],
            1.
        )],
        (_, Id::F4G, Control::Rudder) => vec![panel!(
            &[0x4118, 0x413f],
            &[[0., -66., 9.], [0., -71., 23.]],
            &[[0., -75., 9.], [0., -77., 23.]],
            1.
        )],
        (true, _, Control::Elevator) => vec![
            panel!(
                &[0x346e, 0x3489],
                &[[-4., -66., 7.], [-1., -90., 7.]],
                &[[-28., -92., -5.], [-28., -87., -6.]],
                1.
            ),
            panel!(
                &[0x3613, 0x362e],
                &[[4., -65., 7.], [1., -90., 7.]],
                &[[28., -92., -5.], [28., -87., -6.]],
                1.
            ),
        ],
        (_, Id::F4E, Control::Elevator) => vec![
            panel!(
                &[0x2b15, 0x2b30],
                &[[-4., -45., 6.], [-1., -65., 6.]],
                &[[-23., -68., -4.], [-23., -64., -4.]],
                1.
            ),
            panel!(
                &[0x2405, 0x2420],
                &[[4., -45., 6.], [1., -65., 6.]],
                &[[23., -68., -4.], [23., -64., -4.]],
                1.
            ),
        ],
        (_, Id::F4G, Control::Elevator) => vec![
            panel!(
                &[0x281c, 0x282f],
                &[[-4., -54., 7.], [-1., -74., 7.]],
                &[[-24., -77., -3.], [-24., -72., -3.]],
                1.
            ),
            panel!(
                &[0x233f, 0x2352],
                &[[4., -54., 7.], [1., -74., 7.]],
                &[[23., -77., -3.], [23., -72., -3.]],
                1.
            ),
        ],
        (true, _, Control::Aileron) => vec![
            panel!(
                &[0x216c, 0x2199],
                &[[-43., -32., -8.], [-63., -39., -4.]],
                &[[-43., -41., -8.], [-63., -46., -4.]],
                -1.
            ),
            panel!(
                &[0x1eeb, 0x1f10, 0x1f94],
                &[[43., -32., -8.], [63., -39., -4.]],
                &[[43., -41., -8.], [63., -46., -4.]],
                1.
            ),
        ],
        (_, Id::F4E, Control::Aileron) => vec![
            panel!(
                &[0x31c7, 0x3104],
                &[[-39., -21., -5.], [-39., -21., -6.], [-57., -29., 0.]],
                &[[-39., -27., -5.]],
                -1.
            ),
            panel!(
                &[0x2d6a, 0x2d88],
                &[[39., -21., -5.], [39., -21., -6.], [57., -29., 0.]],
                &[[39., -27., -5.]],
                1.
            ),
        ],
        (_, Id::F4G, Control::Aileron) => vec![
            panel!(
                &[0x2d81, 0x2de9],
                &[[-40., -29., -5.], [-40., -29., -4.], [-59., -37., 2.]],
                &[[-40., -35., -4.]],
                -1.
            ),
            panel!(
                &[0x2a40, 0x2adb],
                &[[40., -29., -5.], [40., -29., -4.], [58., -37., 2.]],
                &[[40., -35., -4.]],
                1.
            ),
        ],
        (true, _, Control::Flaps) => vec![
            panel!(
                &[0x52af, 0x52d6],
                &[[-12., -23., -8.], [-43., -32., -8.]],
                &[[-12., -34., -8.], [-43., -41., -8.]],
                -1.
            ),
            panel!(
                &[0x519f, 0x51c6],
                &[[13., -23., -8.], [43., -32., -8.]],
                &[[13., -34., -8.], [43., -41., -8.]],
                -1.
            ),
        ],
        (_, Id::F4E, Control::Flaps) => vec![
            panel!(
                &[0x404a, 0x4069],
                &[
                    [-11., -15., -7.],
                    [-39., -21., -6.],
                    [-11., -15., -4.],
                    [-39., -21., -5.]
                ],
                &[[-11., -25., -5.], [-39., -27., -5.]],
                -1.
            ),
            panel!(
                &[0x3f53, 0x3f72],
                &[
                    [11., -15., -7.],
                    [39., -21., -6.],
                    [11., -15., -4.],
                    [39., -21., -5.]
                ],
                &[[11., -25., -5.], [39., -27., -5.]],
                -1.
            ),
        ],
        (_, Id::F4G, Control::Flaps) => vec![
            panel!(
                &[0x3f73, 0x3f92],
                &[
                    [-12., -22., -6.],
                    [-40., -29., -5.],
                    [-12., -22., -3.],
                    [-40., -29., -4.]
                ],
                &[[-12., -32., -4.], [-40., -35., -4.]],
                -1.
            ),
            panel!(
                &[0x3e9c, 0x3ebb],
                &[
                    [12., -22., -6.],
                    [40., -29., -5.],
                    [12., -22., -3.],
                    [40., -29., -4.]
                ],
                &[[12., -32., -4.], [40., -35., -4.]],
                -1.
            ),
        ],
        (true, _, Control::Brake) => vec![panel!(
            &[0x4b12, 0x4b31, 0x4b50, 0x4bde, 0x4bfd, 0x4c1c],
            &[
                [-3., 23., 10.],
                [-2., 23., 12.],
                [2., 23., 12.],
                [3., 23., 10.]
            ]
        )],
        (_, Id::F4E, Control::Brake) => vec![panel!(
            &[0x429c, 0x42b3],
            &[[-3., 27., 10.], [3., 27., 10.]]
        )],
        (_, Id::F4G, Control::Brake) => vec![panel!(
            &[0x41d5, 0x41ec],
            &[[-3., 21., 11.], [3., 21., 11.]]
        )],
        (true, _, Control::Hook) => vec![panel!(
            &[0x509f, 0x50be],
            &[[0., -55., -7.], [0., -56., -5.]]
        )],
        (true, _, Control::Gear) => vec![
            panel!(&[0x4c68, 0x4c87], &[[13., -11., -8.], [13., -3., -8.]]),
            panel!(
                &[0x4ceb, 0x4d0a, 0x4d29, 0x4d48],
                &[[29., -18., -8.], [29., -11., -8.]]
            ),
            panel!(&[0x4d94, 0x4db3], &[[-13., -11., -8.], [-13., -3., -8.]]),
            panel!(
                &[0x4e17, 0x4e36, 0x4e55, 0x4e74],
                &[[-29., -18., -8.], [-29., -11., -8.]]
            ),
            panel!(&[0x4ec0, 0x4edf], &[[-2., 68., -8.], [2., 68., -8.]]),
            panel!(&[0x4f2b, 0x4f4a], &[[2., 45., -8.], [2., 60., -8.]]),
            panel!(
                &[0x4fc6, 0x4fe5, 0x5004, 0x5023, 0x5042, 0x5061],
                &[[0., 62., -8.], [0., 68., -8.]]
            ),
        ],
        (_, Id::F4E, Control::Gear) => vec![
            panel!(&[0x4466, 0x4485], &[[22., -9., -6.], [22., 3., -6.]]),
            panel!(&[0x44d1, 0x44f0], &[[-21., -9., -6.], [-21., 3., -6.]]),
            panel!(&[0x453c, 0x455b], &[[0., 48., -7.], [0., 59., -7.]]),
        ],
        (_, Id::F4G, Control::Gear) => vec![
            panel!(&[0x438f, 0x43ae], &[[22., -14., -5.], [22., -2., -5.]]),
            panel!(&[0x43fa, 0x4419], &[[-22., -14., -5.], [-22., -2., -5.]]),
            panel!(&[0x4465, 0x4484], &[[0., 43., -6.], [0., 54., -6.]]),
        ],
        _ => Vec::new(),
    }
}

pub(super) fn check(
    id: AircraftId,
    control: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    metric: &mut Metrics,
) {
    let (raw, reference, pose) = geometry;
    if matches!(control, Control::Gear) {
        let groups: (&[usize], &[usize]) = match id {
            AircraftId::F4B | AircraftId::F4J => (&[0x4d29, 0x4d48], &[0x4e55, 0x4e74]),
            AircraftId::F4E => (&[0x4466, 0x4485], &[0x44d1, 0x44f0]),
            AircraftId::F4G => (&[0x438f, 0x43ae], &[0x43fa, 0x4419]),
            _ => return,
        };
        // Authored inspection clearance, not recovered wheel-spacing behavior.
        wheel_separation(reference, pose, scale, value, metric, groups, 0.70);
    }
    if value == 0. && matches!(control, Control::Gear | Control::Brake | Control::Hook) {
        return;
    }
    let source = if matches!(
        control,
        Control::Rudder | Control::Elevator | Control::Flaps
    ) {
        raw
    } else {
        reference
    };
    let axis = if matches!(control, Control::Rudder) {
        0
    } else {
        2
    };
    for (side, panel) in panels(id, control).iter().enumerate() {
        let selected: Vec<_> = source
            .iter()
            .filter(|f| panel.faces.contains(&f.address))
            .cloned()
            .collect();
        let actual: Vec<_> = pose
            .iter()
            .filter(|f| panel.faces.contains(&f.address))
            .cloned()
            .collect();
        let old = keyed(&selected);
        let new = keyed(&actual);
        metric.reviewed_anchor_missing |= old.iter().any(|(key, face)| {
            new.get(key)
                .is_none_or(|actual| actual.positions.len() != face.positions.len())
        });
        for point in panel.roots {
            let matches = witness_positions(&old, &new, *point);
            if matches.is_empty() {
                metric.reviewed_anchor_missing = true;
            }
            for moved in matches {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*point, moved) * scale);
            }
        }
        let gaps = shared_vertex_gaps(&old, &new, scale);
        if let Some(gap) = gaps.first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
        if value == 0. {
            // Clipping may split a polygon, so require exact source vertex coverage
            // rather than equal face counts, vertex order or neutral triangulation.
            for original in raw.iter().filter(|f| panel.faces.contains(&f.address)) {
                for point in &original.positions {
                    metric.reviewed_neutral_mismatch |= !actual
                        .iter()
                        .filter(|f| f.address == original.address)
                        .any(|f| f.positions.contains(point));
                }
            }
        } else {
            for point in panel.trailing {
                let matches = witness_positions(&old, &new, *point);
                if matches.is_empty() {
                    metric.reviewed_direction_failures += 1;
                }
                for moved in matches {
                    let delta = (moved[axis] - point[axis]) * scale;
                    if axis == 2 && side < 2 {
                        metric.reviewed_control_z_delta[side] = delta;
                    }
                    if delta * panel.direction * value.signum() as f32 <= EPSILON {
                        metric.reviewed_direction_failures += 1;
                    }
                }
            }
        }
    }
}

pub(super) fn witness_positions(
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    point: [f32; 3],
) -> Vec<[f32; 3]> {
    old.iter()
        .flat_map(|(key, face)| {
            face.positions
                .iter()
                .enumerate()
                .filter_map(|(i, original)| {
                    if *original != point {
                        return None;
                    }
                    new.get(key).and_then(|face| face.positions.get(i)).copied()
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_f4_variant_gates_signed_controls_roots_and_opposite_skins() {
        for id in [
            AircraftId::F4B,
            AircraftId::F4J,
            AircraftId::F4E,
            AircraftId::F4G,
        ] {
            for control in [
                Control::Rudder,
                Control::Elevator,
                Control::Aileron,
                Control::Flaps,
            ] {
                let definitions = panels(id, control);
                let mut source = Vec::new();
                for panel in &definitions {
                    for &address in panel.faces {
                        let positions: Vec<_> = panel
                            .roots
                            .iter()
                            .chain(panel.trailing)
                            .copied()
                            .chain(std::iter::once([123., 456., 789.]))
                            .collect();
                        source.push(super::super::tests::face(address, positions));
                    }
                }
                let mut actual = source.clone();
                let axis = if matches!(control, Control::Rudder) {
                    0
                } else {
                    2
                };
                for panel in &definitions {
                    for face in actual
                        .iter_mut()
                        .filter(|face| panel.faces.contains(&face.address))
                    {
                        for point in &mut face.positions {
                            if panel.trailing.contains(point) {
                                point[axis] += panel.direction;
                            }
                        }
                    }
                }
                let mut result = Metrics::default();
                check(
                    id,
                    control,
                    1.,
                    (&source, &source, &actual),
                    1. / 3.,
                    &mut result,
                );
                assert!(!result.reviewed_anchor_missing, "{id:?} {control:?}");
                assert_eq!(result.max_reviewed_anchor_gap, 0.);
                assert_eq!(result.max_reviewed_skin_gap, 0.);
                assert_eq!(result.reviewed_direction_failures, 0);
                let mut wrong = Metrics::default();
                check(
                    id,
                    control,
                    1.,
                    (&source, &source, &source),
                    1. / 3.,
                    &mut wrong,
                );
                assert!(wrong.reviewed_direction_failures > 0);
            }
        }
    }
    #[test]
    fn split_piece_witnesses_are_all_checked_and_missing_pieces_fail() {
        let point = [1., 2., 3.];
        let source = vec![
            super::super::tests::face(7, vec![point, [0.; 3], [1.; 3]]),
            super::super::tests::face(7, vec![point, [4.; 3], [5.; 3]]),
        ];
        let mut actual = source.clone();
        actual[1].positions[0] = [9., 8., 7.];
        let positions = witness_positions(&keyed(&source), &keyed(&actual), point);
        assert_eq!(positions, [point, [9., 8., 7.]]);
    }
}
