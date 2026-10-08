//! Independent AC130 source/atlas witnesses. Gear attachment mechanics remain unknown.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 2] = [0x493b, 0x495a];
const RIGHT: [usize; 2] = [0x491c, 0x4979];
const NOSE: [usize; 4] = [0x4998, 0x49b7, 0x49d6, 0x49f5];
const WING: [usize; 12] = [
    0x272c, 0x273f, 0x3744, 0x3768, 0x330e, 0x3321, 0x4787, 0x47af, 0x33b0, 0x33c4, 0x45cc, 0x45e0,
];
pub(super) struct Sources {
    gear: Vec<Face>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let gear: Vec<_> = Shape::with_state(bytes, &[(0x5cc0, 1)].into())?
            .faces
            .into_iter()
            .filter(|f| {
                LEFT.contains(&f.address) || RIGHT.contains(&f.address) || NOSE.contains(&f.address)
            })
            .collect();
        if gear.len() != 8 {
            return Err("AC130 probe missing reviewed gear".into());
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
            faces: &[0x33ec, 0x3402],
            roots: vec![[0., -80., 6.], [0., -80., 40.]],
            trailing: vec![[0., -89., 6.], [0., -85., 38.]],
            sign: 1.,
        }],
        Control::Elevator => vec![
            Panel {
                faces: &[
                    0x26ce, 0x2764, 0x34a5, 0x34b7, 0x3568, 0x357b, 0x35a2, 0x35b6,
                ],
                roots: vec![[-10., -68., 4.], [-4., -90., 5.], [-40., -79.5, 5.]],
                trailing: vec![[-40., -84., 5.]],
                sign: 1.,
            },
            Panel {
                faces: &[0x3cc3, 0x3cd6, 0x3c97, 0x3caa],
                roots: vec![[10., -68., 4.], [4., -90., 5.], [40., -79.5, 5.]],
                trailing: vec![[40., -84., 5.]],
                sign: 1.,
            },
        ],
        Control::Flaps | Control::Aileron => [-1., 1.]
            .into_iter()
            .map(|side| {
                let flap = matches!(control, Control::Flaps);
                let (lo, hi) = if flap { (34., 45.) } else { (62., 90.) };
                let hinge = |x: f32| [side * x, -16. + (x - 26.) * 10. / 71., 6.];
                let trailing = |x: f32| {
                    [
                        side * x,
                        if flap {
                            -25. + (x - 26.) * 4. / if side < 0. { 26. } else { 25. }
                        } else if side < 0. {
                            -21. + (x - 52.) * 6. / 45.
                        } else {
                            -21. + (x - 51.) * 6. / 46.
                        },
                        6.,
                    ]
                };
                Panel {
                    faces: &WING,
                    roots: vec![hinge(lo), hinge(hi)],
                    trailing: vec![trailing(lo), trailing(hi)],
                    sign: if flap { -1. } else { side },
                }
            })
            .collect(),
        // No false fixed roots on wheel/fairing cards: source has no separate struts.
        // Their full rigidity, recess direction, source endpoints and body stow are witnessed.
        Control::Gear => vec![
            Panel {
                faces: &LEFT,
                roots: vec![],
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
                roots: vec![],
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
        .all(|p| p[1] + 16. - (p[0].abs() - 26.) * 10. / 71. <= EPSILON)
        && if matches!(c, Control::Flaps) {
            span(13., 20.) || span(34., 45.)
        } else {
            span(62., 90.)
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
            metric.reviewed_wheel_failed |=
                q[0].abs() > p[0].abs() + EPSILON || q[2] < p[2] - EPSILON || q[1] != p[1];
            for (j, r) in before.positions.iter().enumerate().skip(i + 1) {
                metric.reviewed_wheel_rigidity_error = metric
                    .reviewed_wheel_rigidity_error
                    .max((distance(*p, *r) - distance(q, actual.positions[j])).abs() * scale);
            }
            if travel < 1e-5 {
                metric.reviewed_wheel_failed |= if side == 2 {
                    !(-2. - 1e-3..=1. + 1e-3).contains(&q[0])
                        || !(40. - 1e-3..=44. + 1e-3).contains(&q[1])
                        || !(-12. - 1e-3..=-8. + 1e-3).contains(&q[2])
                } else {
                    (q[0].abs() - 9.).abs() > 1e-3
                        || !(-9. - 1e-3..=8. + 1e-3).contains(&q[1])
                        || !(-10. - 1e-3..=-4. + 1e-3).contains(&q[2])
                };
            }
        }
    }
    metric.reviewed_min_wheel_gap = Some(right - left);
    metric.reviewed_wheel_failed |= counts != [2, 2, 4]
        || right - left < 11.99
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}
