//! Independent C130 geometry and source-atlas witnesses; no rig transform calls.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 2] = [0x2909, 0x292c];
const RIGHT: [usize; 2] = [0x294f, 0x2972];
const NOSE: [usize; 2] = [0x2995, 0x29b0];
const WING: [usize; 12] = [
    0x1cb3, 0x1ccb, 0x1ce3, 0x1cfb, 0x21a1, 0x21bc, 0x2525, 0x259a, 0x25b2, 0x27b6, 0x2842, 0x2860,
];
pub(super) struct Sources {
    gear: Vec<Face>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let gear: Vec<_> = Shape::with_state(bytes, &[(0x3a30, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect();
        if gear.len() != 6 {
            return Err("C130 probe missing reviewed gear".into());
        }
        Ok(Self { gear })
    }
}
struct Panel {
    faces: &'static [usize],
    roots: Vec<[f32; 3]>,
    trailing: Vec<[f32; 3]>,
    sign: f32,
}
fn panels(control: Control) -> Vec<Panel> {
    match control {
        Control::Rudder => vec![Panel {
            faces: &[0x201a, 0x21f2],
            roots: vec![
                [0., -84701. / 1265., 11. + (-84701. / 1265. + 76.) / 31.],
                [0., -61., 52.],
            ],
            trailing: vec![[0., -76., 11.], [0., -64., 52.]],
            sign: 1.,
        }],
        Control::Elevator => vec![
            Panel {
                faces: &[0x2497, 0x24b2],
                roots: vec![[-8., -56., 9.], [-5., -74., 9.], [-32., -66.4, 9.]],
                trailing: vec![[-32., -70., 9.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x2757, 0x2772],
                roots: vec![[8., -56., 9.], [6., -74., 9.], [32., -66.4, 9.]],
                trailing: vec![[32., -70., 9.]],
                sign: 1.,
            },
        ],
        Control::Flaps | Control::Aileron => [-1., 1.]
            .into_iter()
            .map(|side| {
                let (lo, hi) = if matches!(control, Control::Flaps) {
                    (33., 46.)
                } else {
                    (60., 92.)
                };
                let hinge = |x: f32| [side * x, -8. + (x - 28.) * 9. / 70., 9.];
                let trailing = |x: f32| {
                    [
                        side * x,
                        if side < 0. {
                            -14. + (x - 29.) * 10. / 69.
                        } else {
                            -13. + (x - 28.) * 9. / 70.
                        },
                        9.,
                    ]
                };
                Panel {
                    faces: &WING,
                    roots: vec![hinge(lo), hinge(hi)],
                    trailing: vec![trailing(lo), trailing(hi)],
                    sign: if matches!(control, Control::Flaps) {
                        -1.
                    } else {
                        side
                    },
                }
            })
            .collect(),
        Control::Gear => vec![
            Panel {
                faces: &LEFT,
                roots: vec![[-8., -7., -12.], [-8., 5., -12.]],
                trailing: vec![],
                sign: 0.,
            },
            Panel {
                faces: &RIGHT,
                roots: vec![[9., -7., -12.], [9., 5., -12.]],
                trailing: vec![],
                sign: 0.,
            },
            Panel {
                faces: &NOSE,
                roots: vec![[0., 49., -13.]],
                trailing: vec![],
                sign: 0.,
            },
        ],
        _ => Vec::new(),
    }
}
fn controlled(f: &Face, c: Control) -> bool {
    if !matches!(c, Control::Flaps | Control::Aileron) {
        return true;
    }
    let span = |lo: f32, hi: f32| {
        f.positions
            .iter()
            .all(|p| (lo - EPSILON..=hi + EPSILON).contains(&p[0].abs()))
    };
    f.positions
        .iter()
        .all(|p| p[1] + 8. - (p[0].abs() - 28.) * 9. / 70. <= EPSILON)
        && if matches!(c, Control::Flaps) {
            span(14., 21.) || span(33., 46.)
        } else {
            span(60., 92.)
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
        for root in &panel.roots {
            let points = witnesses(&before, &after, *root);
            metric.reviewed_anchor_missing |= points.is_empty();
            for p in points {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*root, p) * scale);
            }
        }
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
        if let Some(gap) = shared_vertex_gaps(&moving_before, &moving_after, scale).first() {
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
            for point in &panel.trailing {
                let actual = witnesses(&moving_before, &moving_after, *point);
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
            if side == 0 {
                left = left.max(q[0] * scale);
            } else if side == 1 {
                right = right.min(q[0] * scale);
            }
            for (j, r) in before.positions.iter().enumerate().skip(i + 1) {
                metric.reviewed_wheel_rigidity_error = metric
                    .reviewed_wheel_rigidity_error
                    .max((distance(*p, *r) - distance(q, actual.positions[j])).abs() * scale);
            }
            if travel < 1e-5 {
                metric.reviewed_wheel_failed |= if side == 2 {
                    q[0].abs() > 1e-3
                        || !(41. - 1e-3..=49. + 1e-3).contains(&q[1])
                        || !(-13. - 1e-3..=-6. + 1e-3).contains(&q[2])
                } else {
                    !(-7. - 1e-3..=5. + 1e-3).contains(&q[1])
                        || !(-12. - 1e-3..=-2.7 + 1e-3).contains(&q[2])
                        || if side == 0 {
                            !(-9.91..=-8. + 1e-3).contains(&q[0])
                        } else {
                            !(9. - 1e-3..=10.91).contains(&q[0])
                        }
                };
            }
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |= counts != [2, 2, 2]
        || right - left < 11.3
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}
