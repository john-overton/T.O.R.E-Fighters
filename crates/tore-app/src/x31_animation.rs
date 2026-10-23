//! Exact F31.SH source endpoints and explicitly fitted X-31 EFM controls.
//! See docs/spec/variety-animation.md. Prototype rates are not VTOL axes.
use crate::{AppResult, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::FRAC_PI_2;
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 6] = [0x65a0, 0x65a6, 0x65b2, 0x65be, 0x65c4, 0x65ca];
const FLAME: [usize; 4] = [0x48a0, 0x48c7, 0x48ee, 0x4915];
const BRAKE: [usize; 4] = [0x47d0, 0x47e7, 0x4825, 0x483c];
const LEFT: [usize; 6] = [0x409b, 0x40ba, 0x40d9, 0x40f8, 0x4117, 0x4136];
const RIGHT: [usize; 6] = [0x3f84, 0x3fa3, 0x3fc2, 0x3fe1, 0x4000, 0x401f];
const NOSE: [usize; 2] = [0x41ed, 0x420c];
const BRACE: [usize; 2] = [0x4182, 0x41a1];
const MAIN_PANEL: [usize; 4] = [0x3e43, 0x3e5a, 0x3e9e, 0x3eb5];
const NOSE_DOOR: [usize; 2] = [0x3ef9, 0x3f10];
const OUTER_LEFT: [usize; 3] = [0x30f2, 0x31aa, 0x3af8];
const OUTER_RIGHT: [usize; 3] = [0x3856, 0x3789, 0x3b17];
const CANARD: [usize; 4] = [0x42b3, 0x42ca, 0x4258, 0x426f];
const PADDLE: [usize; 6] = [0x4342, 0x4381, 0x4406, 0x442e, 0x44b2, 0x44da];
struct Morph {
    neutral: Vec<[f32; 3]>,
    target: Vec<[f32; 3]>,
    closure: bool,
}
struct SignedMorph {
    neutral: Vec<[f32; 3]>,
    negative: Vec<[f32; 3]>,
    positive: Vec<[f32; 3]>,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
    rudder: BTreeMap<usize, SignedMorph>,
}
pub fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}
fn face(s: &Shape, a: usize) -> AppResult<&Face> {
    s.faces
        .iter()
        .find(|f| f.address == a)
        .ok_or_else(|| format!("F31.SH lacks reviewed face {a:x}").into())
}
fn edge(s: &Shape, ids: &[usize], points: [[f32; 3]; 2]) -> AppResult<()> {
    for &a in ids {
        if !points
            .iter()
            .all(|p| face(s, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed F31.SH root {a:x}").into());
        }
    }
    Ok(())
}
impl Rig {
    pub fn load(bytes: &[u8], mut neutral: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 21974
            || neutral.faces.len() != 225
            || neutral.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F31.SH animation layout".into());
        }
        let original: BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
        let gear: Vec<_> = LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(MAIN_PANEL)
            .chain(NOSE_DOOR)
            .collect();
        for (word, ids) in [
            (0x65a0, FLAME.as_slice()),
            (0x65a6, BRAKE.as_slice()),
            (0x65b2, gear.as_slice()),
        ] {
            let branch = Shape::with_state(bytes, &[(word, 1)].into())?;
            let active: BTreeSet<_> = branch.faces.iter().map(|f| f.address).collect();
            if !original.is_subset(&active)
                || active
                    .difference(&original)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != ids.iter().copied().collect()
            {
                return Err("unreviewed F31.SH device branch".into());
            }
            neutral.faces.extend(
                branch
                    .faces
                    .into_iter()
                    .filter(|f| ids.contains(&f.address)),
            );
        }
        let mut flaps = BTreeMap::new();
        for (word, bindings, closure) in [
            (
                0x65be,
                [
                    (0x4773, 0x46f5, [0, 1, 2, 3]),
                    (0x4792, 0x4714, [1, 2, 3, 0]),
                ],
                0x4733,
            ),
            (
                0x65c4,
                [
                    (0x468c, 0x460e, [0, 1, 2, 3]),
                    (0x46ab, 0x462d, [3, 0, 1, 2]),
                ],
                0x464c,
            ),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
            if original
                .difference(&active)
                .copied()
                .collect::<BTreeSet<_>>()
                != bindings.iter().map(|b| b.0).collect()
                || active
                    .difference(&original)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != bindings.iter().map(|b| b.1).chain([closure]).collect()
            {
                return Err("unreviewed F31.SH down flap branch".into());
            }
            for (a, target, order) in bindings {
                let base = face(&neutral, a)?;
                let target = face(&down, target)?;
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("unreviewed F31.SH flap topology".into());
                }
                let target: Vec<_> = order.map(|i| target.positions[i]).into();
                if base.positions.iter().filter(|p| p[1] == -17.).count() != 2
                    || base
                        .positions
                        .iter()
                        .zip(&target)
                        .any(|(a, b)| a[1] == -17. && a != b)
                {
                    return Err("unreviewed F31.SH flap leading seams".into());
                }
                flaps.insert(
                    a,
                    Morph {
                        neutral: base.positions.clone(),
                        target,
                        closure: false,
                    },
                );
            }
            let a = &flaps[&bindings[0].0];
            let b = &flaps[&bindings[1].0];
            for (p, q) in a.neutral.iter().zip(&a.target) {
                if let Some(i) = b.neutral.iter().position(|v| v == p)
                    && b.target[i] != *q
                {
                    return Err("unreviewed F31.SH flap twin seam".into());
                }
            }
            let closed = face(&down, closure)?.clone();
            let positions = closed
                .positions
                .iter()
                .map(|p| {
                    a.target
                        .iter()
                        .position(|q| q == p)
                        .map(|i| a.neutral[i])
                        .or_else(|| b.target.iter().position(|q| q == p).map(|i| b.neutral[i]))
                        .ok_or("unreviewed F31.SH flap closure")
                })
                .collect::<Result<Vec<_>, _>>()?;
            flaps.insert(
                closure,
                Morph {
                    neutral: positions,
                    target: closed.positions.clone(),
                    closure: true,
                },
            );
            neutral.faces.push(closed);
        }
        let negative = Shape::with_state(bytes, &[(0x65ca, -1)].into())?;
        let positive = Shape::with_state(bytes, &[(0x65ca, 1)].into())?;
        let mut rudder = BTreeMap::new();
        for (a, n, p, norder, porder) in [
            (0x451b, 0x45b5, 0x4568, [3, 0, 1, 2], [2, 1, 0, 3]),
            (0x4532, 0x45cc, 0x457f, [1, 2, 3, 0], [0, 3, 2, 1]),
        ] {
            let base = face(&neutral, a)?;
            let n = face(&negative, n)?;
            let p = face(&positive, p)?;
            if base.positions.len() != 4 || n.positions.len() != 4 || p.positions.len() != 4 {
                return Err("unreviewed F31.SH signed rudder topology".into());
            }
            let n: Vec<_> = norder.map(|i| n.positions[i]).into();
            let p: Vec<_> = porder.map(|i| p.positions[i]).into();
            for root in [[0., -35., 8.], [0., -39., 19.]] {
                let i = base
                    .positions
                    .iter()
                    .position(|p| *p == root)
                    .ok_or("F31.SH rudder root missing")?;
                if n[i] != root || p[i] != root {
                    return Err("unreviewed F31.SH signed rudder root".into());
                }
            }
            rudder.insert(
                a,
                SignedMorph {
                    neutral: base.positions.clone(),
                    negative: n,
                    positive: p,
                },
            );
        }
        for a in OUTER_LEFT
            .into_iter()
            .chain(OUTER_RIGHT)
            .chain(CANARD)
            .chain(PADDLE)
        {
            face(&neutral, a)?;
        }
        edge(
            &neutral,
            &[0x47d0, 0x47e7],
            [[6., -12., -2.], [6., -12., 4.]],
        )?;
        edge(
            &neutral,
            &[0x4825, 0x483c],
            [[-6., -12., -2.], [-6., -12., 4.]],
        )?;
        edge(
            &neutral,
            &MAIN_PANEL[..2],
            [[1., -14., -5.], [1., -1., -5.]],
        )?;
        edge(
            &neutral,
            &MAIN_PANEL[2..],
            [[0., -14., -5.], [0., -1., -5.]],
        )?;
        edge(&neutral, &NOSE_DOOR, [[-2., 24., -5.], [-2., 36., -5.]])?;
        Ok((Self { flaps, rudder }, neutral))
    }
    pub fn animate(&self, source: &Face, s: &State) -> Option<Face> {
        let mut f = source.clone();
        let a = f.address;
        if let Some(m) = self.flaps.get(&a) {
            let travel = s.flaps.clamp(0., 1.) as f32;
            if m.closure && travel == 0. {
                return None;
            }
            f.positions = m
                .neutral
                .iter()
                .zip(&m.target)
                .map(|(a, b)| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * travel))
                .collect();
            let side = if f.positions[0][0] < 0. { -1. } else { 1. };
            let source_positions = f.positions.clone();
            turn(
                &mut f,
                [0., -17., -5.5],
                [1., 0., 0.],
                -0.3 * s.elevator.clamp(-1., 1.) - side * 0.2 * s.aileron.clamp(-1., 1.),
            );
            for (p, q) in source_positions.iter().zip(&mut f.positions) {
                if p[1] == -17. {
                    *q = *p;
                }
            }
            if travel != 0. || s.elevator != 0. || s.aileron != 0. {
                crate::aircraft_animation::update_normal(source, &mut f);
            }
        } else if let Some(m) = self.rudder.get(&a) {
            let demand = s.rudder.clamp(-1., 1.);
            let target = if demand < 0. {
                &m.negative
            } else {
                &m.positive
            };
            let travel = demand.abs() as f32;
            f.positions = m
                .neutral
                .iter()
                .zip(target)
                .map(|(a, b)| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * travel))
                .collect();
            if travel != 0. {
                crate::aircraft_animation::update_normal(source, &mut f);
            }
        } else if OUTER_LEFT.contains(&a) || OUTER_RIGHT.contains(&a) {
            let side = if OUTER_LEFT.contains(&a) { -1. } else { 1. };
            let axis = [side * 11., 0., -0.5];
            turn(
                &mut f,
                [side * 20., -17., -5.5],
                axis,
                -f64::from(side) * 0.3 * s.elevator.clamp(-1., 1.) - 0.2 * s.aileron.clamp(-1., 1.),
            );
            for (p, q) in source.positions.iter().zip(&mut f.positions) {
                if p[1] == -17. {
                    *q = *p;
                }
            }
            if s.elevator != 0. || s.aileron != 0. {
                crate::aircraft_animation::update_normal(source, &mut f);
            }
        } else if CANARD.contains(&a) {
            turn(
                &mut f,
                [0., 54., 1.],
                [1., 0., 0.],
                0.35 * s.elevator.clamp(-1., 1.),
            );
        } else if PADDLE.contains(&a) {
            let root_y = source
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max);
            let roots: Vec<_> = source
                .positions
                .iter()
                .filter(|p| p[1] == root_y)
                .copied()
                .collect();
            if roots.len() == 2 {
                let pivot = std::array::from_fn(|i| (roots[0][i] + roots[1][i]) * 0.5);
                let mut axis: [f32; 3] = std::array::from_fn(|i| roots[1][i] - roots[0][i]);
                if axis[2] < 0. || (axis[2] == 0. && axis[0] < 0.) {
                    axis = axis.map(|v| -v);
                }
                let length = (axis[0] * axis[0] + axis[2] * axis[2]).sqrt();
                let [pitch, yaw] = vector_angles(s);
                let angle = ((f64::from(axis[0]) * pitch - f64::from(axis[2]) * yaw)
                    / f64::from(length))
                .clamp(-15f64.to_radians(), 15f64.to_radians());
                turn(&mut f, pivot, axis, angle);
            }
        } else if BRAKE.contains(&a) {
            let travel = s.brake.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            let side = if a == 0x47d0 || a == 0x47e7 { 1. } else { -1. };
            turn(
                &mut f,
                [side * 6., -12., 1.],
                [0., 0., 1.],
                -f64::from(side) * 1.05 * (1. - travel),
            );
        } else if LEFT.contains(&a) || RIGHT.contains(&a) {
            let travel = s.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            let side = if LEFT.contains(&a) { -1. } else { 1. };
            turn(
                &mut f,
                [side * 4., -12., -5.],
                [1., side * 0.55, 0.],
                130f64.to_radians() * (1. - travel),
            );
        } else if NOSE.contains(&a) {
            let travel = s.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            turn(
                &mut f,
                [0., 33., -5.],
                [1., 0., 0.],
                130f64.to_radians() * (1. - travel),
            );
            crate::additional_animation::turn(
                &mut f,
                [0., 33., -5.],
                [0., 0., 1.],
                -s.nosewheel_angle(),
            );
        } else if BRACE.contains(&a) {
            let travel = s.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            brace(source, &mut f, 1. - travel);
        } else if MAIN_PANEL.contains(&a) || NOSE_DOOR.contains(&a) {
            let travel = s.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            let (pivot, angle) = if NOSE_DOOR.contains(&a) {
                ([-2., 24., -5.], -FRAC_PI_2)
            } else if a == 0x3e43 || a == 0x3e5a {
                ([1., -14., -5.], -FRAC_PI_2)
            } else {
                ([0., -14., -5.], FRAC_PI_2)
            };
            turn(&mut f, pivot, [0., 1., 0.], angle * (1. - travel));
        } else if flame(a) {
            let travel = s.exhaust.max(0.) as f32;
            if travel == 0. {
                return None;
            }
            for p in &mut f.positions {
                p[1] = -41. + (p[1] + 41.) * travel;
            }
            let [pitch, yaw] = vector_angles(s);
            turn(&mut f, [0., -41., 0.], [1., 0., 0.], pitch);
            turn(&mut f, [0., -41., 0.], [0., 0., 1.], -yaw);
        }
        Some(f)
    }
}
fn vector_angles(s: &State) -> [f64; 2] {
    [1, 2].map(|i| (s.auxiliary_rates[i] / FRAC_PI_2).clamp(-1., 1.) * 15f64.to_radians())
}
fn brace(source: &Face, result: &mut Face, closing: f64) {
    if closing == 0. {
        return;
    }
    let root = [23.5f32, -5.];
    let offset = crate::aircraft_animation::rotate(
        [0., -0.5, -4.5],
        [1., 0., 0.],
        130f64.to_radians() * closing,
    );
    let destination = [33. + offset[1] - root[0], -5. + offset[2] - root[1]];
    let original = [9f32, -4.5];
    let length = original.iter().map(|v| v * v).sum::<f32>().sqrt();
    let new_length = destination.iter().map(|v| v * v).sum::<f32>().sqrt();
    let axis = original.map(|v| v / length);
    let actual = destination.map(|v| v / new_length);
    for p in &mut result.positions {
        let delta = [p[1] - root[0], p[2] - root[1]];
        let along = (delta[0] * axis[0] + delta[1] * axis[1]) * new_length / length;
        let across = -delta[0] * axis[1] + delta[1] * axis[0];
        p[1] = root[0] + along * actual[0] - across * actual[1];
        p[2] = root[1] + along * actual[1] + across * actual[0];
    }
    crate::aircraft_animation::update_normal(source, result);
}
fn turn(face: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    crate::additional_animation::turn(face, pivot, axis, angle);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
            rudder: BTreeMap::new(),
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0., 5000., 0.]).unwrap()
    }
    fn synthetic(a: usize, positions: Vec<[f32; 3]>) -> Face {
        let n = positions.len();
        Face {
            address: a,
            positions,
            colors: vec![22; n],
            fog: Default::default(),
            uv: vec![[0.25, 0.75]; n],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([1., 0., 0.]),
        }
    }
    fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
        a.iter()
            .zip(b)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }
    fn material(a: &Face, b: &Face) {
        assert_eq!(a.uv, b.uv);
        assert_eq!(a.colors, b.colors);
        assert_eq!(a.texture, b.texture);
        assert_eq!(a.subtype, b.subtype);
        assert!(b.positions.iter().flatten().all(|v| v.is_finite()));
    }
    #[test]
    fn omitted_left_upper_is_now_coherent_with_lower_and_roll_is_opposed() {
        let r = rig();
        let mut s = state();
        for v in [-1., 0., 1.] {
            s.aileron = v;
            for (upper, lower, x, sign) in [(0x30f2, 0x3af8, -25., -1.), (0x3856, 0x3b17, 25., 1.)]
            {
                let a = synthetic(
                    upper,
                    vec![
                        [x, -17., -5.75],
                        [x + 2., -17., -6.],
                        [x + 2., -20., -6.],
                        [x, -20., -6.],
                    ],
                );
                let b = synthetic(
                    lower,
                    vec![
                        [x, -17., -6.],
                        [x + 2., -17., -6.],
                        [x + 2., -20., -6.],
                        [x, -20., -6.],
                    ],
                );
                let p = r.animate(&a, &s).unwrap();
                let q = r.animate(&b, &s).unwrap();
                for i in 0..2 {
                    assert_eq!(p.positions[i], a.positions[i]);
                    assert_eq!(q.positions[i], b.positions[i]);
                }
                for i in 2..4 {
                    assert_eq!(p.positions[i], q.positions[i]);
                }
                if v != 0. {
                    assert!((p.positions[2][2] + 6.) * sign * (v as f32) > 0.);
                }
                material(&a, &p);
                material(&b, &q);
            }
        }
    }
    #[test]
    fn source_flap_morph_pins_distinct_fronts_and_shares_closure_during_pitch_roll() {
        let a = 0xf001;
        let b = 0xf002;
        let c = 0xf003;
        let upper = synthetic(
            a,
            vec![
                [-7., -17., -5.],
                [-16., -17., -5.],
                [-16., -20., -6.],
                [-7., -20., -6.],
            ],
        );
        let lower = synthetic(
            b,
            vec![
                [-7., -17., -6.],
                [-16., -17., -6.],
                upper.positions[2],
                upper.positions[3],
            ],
        );
        let target = vec![
            upper.positions[0],
            upper.positions[1],
            [-16., -19., -8.],
            [-7., -19., -8.],
        ];
        let lower_target = vec![lower.positions[0], lower.positions[1], target[2], target[3]];
        let closing = synthetic(c, vec![target[2], lower.positions[1], upper.positions[1]]);
        let r = Rig {
            flaps: [
                (
                    a,
                    Morph {
                        neutral: upper.positions.clone(),
                        target: target.clone(),
                        closure: false,
                    },
                ),
                (
                    b,
                    Morph {
                        neutral: lower.positions.clone(),
                        target: lower_target,
                        closure: false,
                    },
                ),
                (
                    c,
                    Morph {
                        neutral: vec![upper.positions[2], lower.positions[1], upper.positions[1]],
                        target: closing.positions.clone(),
                        closure: true,
                    },
                ),
            ]
            .into(),
            rudder: BTreeMap::new(),
        };
        let mut s = state();
        for v in [0., 0.25, 0.5, 1.] {
            s.flaps = v;
            for controls in [[0., 0.], [-1., 0.5], [1., -0.5]] {
                s.elevator = controls[0];
                s.aileron = controls[1];
                let p = r.animate(&upper, &s).unwrap();
                let q = r.animate(&lower, &s).unwrap();
                for i in 0..2 {
                    assert_eq!(p.positions[i], upper.positions[i]);
                    assert_eq!(q.positions[i], lower.positions[i]);
                }
                for i in 2..4 {
                    assert_eq!(p.positions[i], q.positions[i]);
                }
                if v == 0. {
                    assert!(r.animate(&closing, &s).is_none());
                } else {
                    let side = r.animate(&closing, &s).unwrap();
                    assert_eq!(side.positions[0], p.positions[2]);
                }
                if v == 1. && controls == [0., 0.] {
                    assert_eq!(p.positions, target);
                }
            }
        }
    }
    #[test]
    fn rudder_signed_endpoints_keep_forward_edge_and_material() {
        let f = synthetic(
            0xf004,
            vec![[0., 2., 1.], [0., 4., 1.], [0., 4., 6.], [0., 2., 6.]],
        );
        let negative = vec![f.positions[0], [-3., 3., 1.], [-2., 3., 6.], f.positions[3]];
        let positive = negative
            .iter()
            .map(|p| [-p[0], p[1], p[2]])
            .collect::<Vec<_>>();
        let r = Rig {
            flaps: BTreeMap::new(),
            rudder: [(
                f.address,
                SignedMorph {
                    neutral: f.positions.clone(),
                    negative: negative.clone(),
                    positive: positive.clone(),
                },
            )]
            .into(),
        };
        let mut s = state();
        for v in [-1., -0.5, 0., 0.5, 1.] {
            s.rudder = v;
            let g = r.animate(&f, &s).unwrap();
            assert_eq!(g.positions[0], f.positions[0]);
            assert_eq!(g.positions[3], f.positions[3]);
            if v == -1. {
                assert_eq!(g.positions, negative);
            }
            if v == 1. {
                assert_eq!(g.positions, positive);
            }
            if v == 0. {
                assert_eq!(g.positions, f.positions);
            }
            material(&f, &g);
        }
    }
    #[test]
    fn whole_wheels_hold_root_and_rigidity_for_401_positions() {
        let r = rig();
        let mut s = state();
        for (a, pivot, x) in [
            (LEFT[0], [-4., -12., -5.], -8.),
            (RIGHT[0], [4., -12., -5.], 8.),
            (NOSE[0], [0., 33., -5.], 0.),
        ] {
            let f = synthetic(
                a,
                vec![
                    pivot,
                    [x, pivot[1] - 1., pivot[2] - 3.],
                    [x, pivot[1] - 1., pivot[2] - 7.],
                    [x, pivot[1] + 2., pivot[2] - 7.],
                ],
            );
            for i in 1..=401 {
                s.gear = f64::from(i) / 401.;
                let g = r.animate(&f, &s).unwrap();
                assert_eq!(g.positions[0], pivot);
                for j in 0..4 {
                    for k in j + 1..4 {
                        assert!(
                            (distance(f.positions[j], f.positions[k])
                                - distance(g.positions[j], g.positions[k]))
                            .abs()
                                < 1e-4
                        );
                    }
                }
                if i == 401 {
                    assert_eq!(g.positions, f.positions);
                }
                material(&f, &g);
            }
        }
    }
    #[test]
    fn nose_brace_keeps_painted_top_center_and_tracks_actual_wheel_joint_without_flip() {
        let r = rig();
        let mut s = state();
        let f = synthetic(
            BRACE[0],
            vec![
                [0., 23.75, -4.5],
                [0., 32.75, -9.25],
                [0., 32.25, -9.75],
                [0., 23.25, -5.5],
            ],
        );
        let wheel = synthetic(
            NOSE[0],
            vec![
                [0., 32.5, -9.5],
                [0., 33., -5.],
                [0., 35., -5.],
                [0., 35., -12.],
            ],
        );
        let area = |f: &Face| {
            f.positions
                .iter()
                .zip(f.positions.iter().cycle().skip(1))
                .take(f.positions.len())
                .map(|(a, b)| a[1] * b[2] - b[1] * a[2])
                .sum::<f32>()
        };
        let original = area(&f);
        for i in 1..=401 {
            s.gear = f64::from(i) / 401.;
            let g = r.animate(&f, &s).unwrap();
            let w = r.animate(&wheel, &s).unwrap();
            let midpoint = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|k| (a[k] + b[k]) * 0.5);
            assert!(distance(midpoint(g.positions[0], g.positions[3]), [0., 23.5, -5.]) < 1e-4);
            assert!(distance(midpoint(g.positions[1], g.positions[2]), w.positions[0]) < 1e-4);
            assert!(area(&g) * original > 0.);
            assert!(area(&g).abs() >= original.abs() - 1e-3);
            material(&f, &g);
        }
    }
    #[test]
    fn prototype_paddles_use_real_rate_fields_without_vtol_axes_or_burner() {
        let r = rig();
        let mut s = state();
        s.exhaust = 0.;
        let f = synthetic(
            PADDLE[4],
            vec![
                [-1., -41., 5.],
                [2., -41., 5.],
                [2., -45., 5.],
                [-1., -45., 5.],
            ],
        );
        s.lift_controls.vector_pitch_actual = 1.;
        assert_eq!(r.animate(&f, &s).unwrap().positions, f.positions);
        for v in [-1., -0.5, 0., 0.5, 1.] {
            s.auxiliary_rates[1] = v * FRAC_PI_2;
            let g = r.animate(&f, &s).unwrap();
            assert_eq!(g.positions[0], f.positions[0]);
            assert_eq!(g.positions[1], f.positions[1]);
            if v != 0. {
                assert!((g.positions[2][2] - 5.) * (v as f32) < 0.);
            }
            material(&f, &g);
        }
    }
    #[test]
    fn separate_panel_upper_edges_stay_fixed_and_hidden_devices_stay_hidden_at_zero() {
        let r = rig();
        let mut s = state();
        let f = synthetic(
            MAIN_PANEL[0],
            vec![
                [1., -9., -5.],
                [1., -3., -5.],
                [1., -3., -9.],
                [1., -9., -9.],
            ],
        );
        for v in [0.000001, 0.25, 0.5, 1.] {
            s.gear = v;
            let g = r.animate(&f, &s).unwrap();
            assert_eq!(g.positions[0], f.positions[0]);
            assert_eq!(g.positions[1], f.positions[1]);
            material(&f, &g);
        }
        s.gear = 0.;
        s.brake = 0.;
        s.exhaust = 0.;
        for a in LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(MAIN_PANEL)
            .chain(NOSE_DOOR)
            .chain(BRAKE)
            .chain(FLAME)
        {
            assert!(r.animate(&synthetic(a, f.positions.clone()), &s).is_none());
        }
    }
    #[test]
    fn unrelated_body_and_outer_panels_remain_static_under_flap_only() {
        let r = rig();
        let mut s = state();
        s.flaps = 1.;
        let f = synthetic(0xf006, vec![[2., 3., 4.], [5., 3., 4.], [5., 7., 4.]]);
        assert_eq!(r.animate(&f, &s).unwrap().positions, f.positions);
        let g = synthetic(OUTER_LEFT[0], f.positions);
        assert_eq!(r.animate(&g, &s).unwrap().positions, g.positions);
    }
    #[test]
    fn combined_prototype_grid_keeps_paddle_roots_and_full_plume_rigid() {
        let r = rig();
        let paddle = synthetic(
            PADDLE[0],
            vec![
                [5., -41., -1.],
                [6., -41., 1.],
                [6., -45., 1.],
                [5., -45., -1.],
            ],
        );
        // Invented plume dimensions, with a vertex at the common root.
        let plume = synthetic(
            FLAME[0],
            vec![
                [0., -41., 0.],
                [2., -41., 0.],
                [0., -64., 0.],
                [-2., -41., 0.],
            ],
        );
        let mut s = state();
        s.exhaust = 1.;
        for pitch in [-1., -0.5, 0., 0.5, 1.] {
            for yaw in [-1., -0.5, 0., 0.5, 1.] {
                s.auxiliary_rates[1] = pitch * FRAC_PI_2;
                s.auxiliary_rates[2] = yaw * FRAC_PI_2;
                let a = r.animate(&paddle, &s).unwrap();
                let b = r.animate(&plume, &s).unwrap();
                assert!(distance(a.positions[0], paddle.positions[0]) < 1e-4);
                assert!(distance(a.positions[1], paddle.positions[1]) < 1e-4);
                assert!(distance(b.positions[0], plume.positions[0]) < 1e-4);
                for (source, pose) in [(&paddle, &a), (&plume, &b)] {
                    for i in 0..source.positions.len() {
                        for j in i + 1..source.positions.len() {
                            assert!(
                                (distance(source.positions[i], source.positions[j])
                                    - distance(pose.positions[i], pose.positions[j]))
                                .abs()
                                    < 1e-4
                            );
                        }
                    }
                    material(source, pose);
                }
            }
        }
    }
}
