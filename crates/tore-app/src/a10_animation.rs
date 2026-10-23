//! A10.SH reviewed source skins and explicitly fitted controls/gear.
//! See docs/spec/variety-animation.md. Imported instructions never execute.
use crate::{AppResult, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{FRAC_PI_2, PI};
use tore_formats::shape::{Face, Shape};

const WORDS: [usize; 3] = [0x5d70, 0x5d7c, 0x5d82];
const GEAR: [usize; 20] = [
    0x4a6c, 0x4a83, 0x4ac7, 0x4ade, 0x4b3a, 0x4b59, 0x4b78, 0x4b97, 0x4bfb, 0x4c1a, 0x4c39, 0x4c58,
    0x4ca4, 0x4cbb, 0x4cff, 0x4d16, 0x4d72, 0x4d91, 0x4db0, 0x4dcf,
];
const MAIN_LEFT: [usize; 4] = [0x4b3a, 0x4b59, 0x4b78, 0x4b97];
const MAIN_RIGHT: [usize; 4] = [0x4bfb, 0x4c1a, 0x4c39, 0x4c58];
const NOSE: [usize; 4] = [0x4d72, 0x4d91, 0x4db0, 0x4dcf];
const FIN: [usize; 6] = [0x2c58, 0x2c6c, 0x2c80, 0x3a2a, 0x3a3e, 0x3a52];
const TAIL: [usize; 4] = [0x28f5, 0x29cd, 0x36b7, 0x3765];
const ROLL_LEFT: [usize; 2] = [0x3a0f, 0x3901];
const ROLL_RIGHT: [usize; 2] = [0x2b41, 0x2bd3];
const EPSILON: f32 = 1e-4;
struct Morph {
    neutral: Vec<[f32; 3]>,
    down: Vec<[f32; 3]>,
    closure: bool,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
pub fn flame(_address: usize) -> bool {
    false
}
fn face(shape: &Shape, address: usize) -> AppResult<&Face> {
    shape
        .faces
        .iter()
        .find(|f| f.address == address)
        .ok_or_else(|| format!("A10.SH lacks reviewed face {address:x}").into())
}
fn edge(shape: &Shape, addresses: &[usize], points: [[f32; 3]; 2]) -> AppResult<()> {
    for &a in addresses {
        if !points
            .iter()
            .all(|p| face(shape, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed A10.SH attachment {a:x}").into());
        }
    }
    Ok(())
}
impl Rig {
    pub fn load(bytes: &[u8], mut neutral: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 19854
            || neutral.faces.len() != 303
            || neutral.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed A10.SH animation layout".into());
        }
        edge(&neutral, &TAIL[..2], [[4., -66., 1.], [28., -66., 1.]])?;
        edge(&neutral, &TAIL[2..], [[-5., -66., 1.], [-29., -66., 1.]])?;
        for (a, roots) in [
            (0x2b41, [[24., -6., -3.], [47., -6., -1.]]),
            (0x2bd3, [[24., -6., -4.], [47., -6., -2.]]),
            (0x3a0f, [[-25., -6., -3.], [-48., -6., -1.]]),
            (0x3901, [[-25., -6., -4.], [-48., -6., -2.]]),
        ] {
            edge(&neutral, &[a], roots)?;
        }
        for a in FIN {
            let f = face(&neutral, a)?;
            if !f.positions.iter().any(|p| p[1] < -63.) || !f.positions.iter().any(|p| p[1] > -63.)
            {
                return Err("unreviewed A10.SH fin partition".into());
            }
        }
        let original: BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
        let deployed = Shape::with_state(bytes, &[(0x5d70, 1)].into())?;
        let active: BTreeSet<_> = deployed.faces.iter().map(|f| f.address).collect();
        if !original.is_subset(&active)
            || active
                .difference(&original)
                .copied()
                .collect::<BTreeSet<_>>()
                != GEAR.into_iter().collect()
        {
            return Err("unreviewed A10.SH gear branch".into());
        }
        neutral.faces.extend(
            deployed
                .faces
                .into_iter()
                .filter(|f| GEAR.contains(&f.address)),
        );
        let mut flaps = BTreeMap::new();
        for (word, bindings, closures) in [
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
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
            if active
                .difference(&original)
                .copied()
                .collect::<BTreeSet<_>>()
                != bindings.iter().map(|b| b.1).chain(closures).collect()
                || original
                    .difference(&active)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != bindings.iter().map(|b| b.0).collect()
            {
                return Err("unreviewed A10.SH signed flap branch".into());
            }
            for (address, target, order) in bindings {
                let base = face(&neutral, address)?;
                let target = face(&down, target)?;
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("unreviewed A10.SH flap topology".into());
                }
                let target: Vec<_> = order.map(|i| target.positions[i]).into();
                if base.positions.iter().filter(|p| p[1] == -6.).count() != 2
                    || base
                        .positions
                        .iter()
                        .zip(&target)
                        .any(|(a, b)| a[1] == -6. && a != b)
                {
                    return Err("unreviewed A10.SH fixed flap hinge".into());
                }
                flaps.insert(
                    address,
                    Morph {
                        neutral: base.positions.clone(),
                        down: target,
                        closure: false,
                    },
                );
            }
            let a = &flaps[&bindings[0].0];
            let b = &flaps[&bindings[1].0];
            for (p, target) in a.neutral.iter().zip(&a.down) {
                if let Some(i) = b.neutral.iter().position(|q| q == p)
                    && b.down[i] != *target
                {
                    return Err("unreviewed A10.SH flap twin seam".into());
                }
            }
            let first = (a.neutral.clone(), a.down.clone());
            let second = (b.neutral.clone(), b.down.clone());
            for closure in closures {
                let closing = face(&down, closure)?.clone();
                let closed = closing
                    .positions
                    .iter()
                    .map(|p| {
                        first
                            .1
                            .iter()
                            .position(|q| q == p)
                            .map(|i| first.0[i])
                            .or_else(|| second.1.iter().position(|q| q == p).map(|i| second.0[i]))
                            .ok_or("unreviewed A10.SH flap end closure")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                flaps.insert(
                    closure,
                    Morph {
                        neutral: closed,
                        down: closing.positions.clone(),
                        closure: true,
                    },
                );
                neutral.faces.push(closing);
            }
        }
        for f in neutral.faces.iter_mut().filter(|f| {
            MAIN_LEFT.contains(&f.address)
                || MAIN_RIGHT.contains(&f.address)
                || NOSE.contains(&f.address)
        }) {
            let pivot = if MAIN_LEFT.contains(&f.address) {
                [-23., -1., -8.]
            } else if MAIN_RIGHT.contains(&f.address) {
                [21., -1., -8.]
            } else {
                [0., 38., -4.]
            };
            insert_root(f, pivot)?;
        }
        edge(
            &neutral,
            &[0x4a6c, 0x4a83],
            [[-24., -5., -9.], [-18., -5., -9.]],
        )?;
        edge(
            &neutral,
            &[0x4ac7, 0x4ade],
            [[18., -5., -9.], [24., -5., -9.]],
        )?;
        edge(
            &neutral,
            &[0x4ca4, 0x4cbb],
            [[3., 40., -4.], [3., 57., -4.]],
        )?;
        edge(
            &neutral,
            &[0x4cff, 0x4d16],
            [[-2., 35., -4.], [4., 35., -4.]],
        )?;
        neutral.faces = neutral
            .faces
            .into_iter()
            .flat_map(|f| {
                if FIN.contains(&f.address) {
                    crate::aircraft_animation::split_surface(&f, [0.; 3], [1., 0., 0.], 0., |p| {
                        p[1] + 63.
                    })
                } else {
                    vec![f]
                }
            })
            .collect();
        Ok((Self { flaps }, neutral))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let a = source.address;
        if let Some(m) = self.flaps.get(&a) {
            let travel = state.flaps.clamp(0., 1.) as f32;
            if m.closure && travel == 0. {
                return None;
            }
            result.positions = m
                .neutral
                .iter()
                .zip(&m.down)
                .map(|(a, b)| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * travel))
                .collect();
            if travel > 0. {
                update_normal(source, &mut result);
            }
        } else if FIN.contains(&a) {
            for p in &mut result.positions {
                if p[1] < -63. {
                    p[0] += (state.rudder.clamp(-1., 1.) * 0.35).tan() as f32 * (-63. - p[1]);
                }
            }
            if state.rudder != 0. {
                update_normal(source, &mut result);
            }
        } else if TAIL.contains(&a) {
            turn(
                &mut result,
                [0., -66., 1.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
        } else if ROLL_LEFT.contains(&a) || ROLL_RIGHT.contains(&a) {
            let (pivot, axis) = if ROLL_LEFT.contains(&a) {
                ([-25., -6., -3.5], [-23., 0., 2.])
            } else {
                ([24., -6., -3.5], [23., 0., 2.])
            };
            turn(
                &mut result,
                pivot,
                axis,
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for (p, q) in source.positions.iter().zip(&mut result.positions) {
                if p[1] == -6. {
                    *q = *p;
                }
            }
            if state.aileron != 0. {
                update_normal(source, &mut result);
            }
        } else if GEAR.contains(&a) {
            let travel = state.gear.clamp(0., 1.);
            if travel == 0. && !MAIN_LEFT.contains(&a) && !MAIN_RIGHT.contains(&a) {
                return None;
            }
            animate_gear(&mut result, 1. - travel);
        }
        Some(result)
    }
}
fn insert_root(f: &mut Face, pivot: [f32; 3]) -> AppResult<()> {
    if f.positions.len() != 4 || f.uv.len() != 4 {
        return Err("unreviewed A10.SH wheel cutout topology".into());
    }
    for i in 0..4 {
        let j = (i + 1) % 4;
        let a = f.positions[i];
        let b = f.positions[j];
        if a[2] != pivot[2] || b[2] != pivot[2] {
            continue;
        }
        let axis = if a[0] != b[0] { 0 } else { 1 };
        let t = (pivot[axis] - a[axis]) / (b[axis] - a[axis]);
        if !(0. ..=1.).contains(&t)
            || distance(std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t), pivot) > EPSILON
        {
            continue;
        }
        let uv = std::array::from_fn(|k| f.uv[i][k] + (f.uv[j][k] - f.uv[i][k]) * t);
        let color = (f32::from(f.colors[i]) + (f32::from(f.colors[j]) - f32::from(f.colors[i])) * t)
            .round() as u8;
        f.positions.insert(i + 1, pivot);
        f.uv.insert(i + 1, uv);
        f.colors.insert(i + 1, color);
        return Ok(());
    }
    Err("A10.SH painted-root marker is not on source top edge".into())
}
fn animate_gear(f: &mut Face, closing: f64) {
    let a = f.address;
    let (pivot, axis, angle) = if MAIN_LEFT.contains(&a) {
        ([-23., -1., -8.], [1., 0., 0.], FRAC_PI_2)
    } else if MAIN_RIGHT.contains(&a) {
        ([21., -1., -8.], [1., 0., 0.], FRAC_PI_2)
    } else if NOSE.contains(&a) {
        ([0., 38., -4.], [1., 0., 0.], 100f64.to_radians())
    } else if [0x4a6c, 0x4a83, 0x4ac7, 0x4ade].contains(&a) {
        ([0., -5., -9.], [1., 0., 0.], PI - 5f64.atan())
    } else if [0x4cff, 0x4d16].contains(&a) {
        ([0., 35., -4.], [1., 0., 0.], PI - 11f64.atan())
    } else {
        ([3., 40., -4.], [0., 1., 0.], FRAC_PI_2)
    };
    turn(f, pivot, axis, angle * closing);
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn turn(face: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    if angle == 0. {
        return;
    }
    let length = axis
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    let axis = axis.map(|v| f64::from(v) / length);
    let (sin, cos) = angle.sin_cos();
    let rotate = |p: [f32; 3]| -> [f32; 3] {
        let p = p.map(f64::from);
        let dot = axis.iter().zip(p).map(|(a, b)| a * b).sum::<f64>();
        let cross = [
            axis[1] * p[2] - axis[2] * p[1],
            axis[2] * p[0] - axis[0] * p[2],
            axis[0] * p[1] - axis[1] * p[0],
        ];
        std::array::from_fn(|i| (p[i] * cos + cross[i] * sin + axis[i] * dot * (1. - cos)) as f32)
    };
    for p in &mut face.positions {
        let v = rotate(std::array::from_fn(|i| p[i] - pivot[i]));
        *p = std::array::from_fn(|i| pivot[i] + v[i]);
    }
    if let Some(n) = face.normal {
        let n = rotate([n[0], n[2], n[1]]);
        face.normal = Some([n[0], n[2], n[1]]);
    }
}
fn polygon_normal(points: &[[f32; 3]]) -> Option<[f32; 3]> {
    let mut n = [0f64; 3];
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        n[0] += f64::from(a[1] - b[1]) * f64::from(a[2] + b[2]);
        n[1] += f64::from(a[2] - b[2]) * f64::from(a[0] + b[0]);
        n[2] += f64::from(a[0] - b[0]) * f64::from(a[1] + b[1]);
    }
    let len = n.iter().map(|v| v * v).sum::<f64>().sqrt();
    (len > 1e-9).then(|| n.map(|v| (v / len) as f32))
}
fn update_normal(source: &Face, result: &mut Face) {
    let (Some(old), Some(reference), Some(mut n)) = (
        source.normal,
        polygon_normal(&source.positions),
        polygon_normal(&result.positions),
    ) else {
        return;
    };
    if [old[0], old[2], old[1]]
        .iter()
        .zip(reference)
        .map(|(a, b)| a * b)
        .sum::<f32>()
        < 0.
    {
        n = n.map(|v| -v);
    }
    result.normal = Some([n[0], n[2], n[1]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
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
    fn close(a: [f32; 3], b: [f32; 3]) {
        assert!(distance(a, b) < 1e-4, "{a:?} != {b:?}");
    }
    fn material(a: &Face, b: &Face) {
        assert_eq!(a.colors, b.colors);
        assert_eq!(a.uv, b.uv);
        assert_eq!(a.texture, b.texture);
        assert_eq!(a.subtype, b.subtype);
        assert!(b.positions.iter().flatten().all(|v| v.is_finite()));
    }
    #[test]
    fn pitch_pins_span_seam_and_raises_both_trailing_sides() {
        let r = rig();
        let mut s = state();
        for x in [-12., 14.] {
            let f = synthetic(
                TAIL[0],
                vec![
                    [x, -66., 1.],
                    [x + 3., -66., 1.],
                    [x + 3., -70., 1.],
                    [x, -70., 1.],
                ],
            );
            for v in [-1., 0., 1.] {
                s.elevator = v;
                let g = r.animate(&f, &s).unwrap();
                close(f.positions[0], g.positions[0]);
                close(f.positions[1], g.positions[1]);
                if v != 0. {
                    assert!((g.positions[2][2] - 1.) * v as f32 > 0.);
                } else {
                    assert_eq!(f.positions, g.positions);
                }
                material(&f, &g);
            }
        }
    }
    #[test]
    fn roll_has_opposed_trailing_motion_and_preserves_distinct_front_seams() {
        let r = rig();
        let mut s = state();
        let mut z = [0.; 2];
        for v in [-1., 0., 1.] {
            s.aileron = v;
            for (i, (a, x)) in [(ROLL_LEFT[0], -32.), (ROLL_RIGHT[0], 31.)]
                .into_iter()
                .enumerate()
            {
                let top = synthetic(
                    a,
                    vec![
                        [x, -6., -2.75],
                        [x + 2., -6., -2.5],
                        [x + 2., -10., -3.],
                        [x, -10., -3.],
                    ],
                );
                let bottom = synthetic(
                    a,
                    vec![
                        [x, -6., -3.75],
                        [x + 2., -6., -3.5],
                        [x + 2., -10., -3.],
                        [x, -10., -3.],
                    ],
                );
                let p = r.animate(&top, &s).unwrap();
                let q = r.animate(&bottom, &s).unwrap();
                for j in 0..2 {
                    close(p.positions[j], top.positions[j]);
                    close(q.positions[j], bottom.positions[j]);
                }
                for j in 2..4 {
                    close(p.positions[j], q.positions[j]);
                }
                z[i] = p.positions[2][2] + 3.;
                material(&top, &p);
                material(&bottom, &q);
            }
            if v != 0. {
                assert!(z[0] * (v as f32) < 0.);
                assert!(z[1] * v as f32 > 0.);
            } else {
                assert_eq!(z, [0.; 2]);
            }
        }
    }
    #[test]
    fn aft_fin_shear_keeps_fixed_root_and_thick_art_registration() {
        let r = rig();
        let mut s = state();
        let f = synthetic(
            FIN[0],
            vec![
                [8., -60., 4.],
                [8., -63., 4.],
                [8., -67., 9.],
                [8., -63., 9.],
            ],
        );
        let g = synthetic(
            FIN[1],
            f.positions
                .iter()
                .map(|p| [p[0] + 1.25, p[1], p[2]])
                .collect(),
        );
        for v in [-1., 0., 1.] {
            s.rudder = v;
            let a = r.animate(&f, &s).unwrap();
            let b = r.animate(&g, &s).unwrap();
            for i in [0, 1, 3] {
                close(a.positions[i], f.positions[i]);
                close(b.positions[i], g.positions[i]);
            }
            for i in 0..4 {
                close(
                    [
                        a.positions[i][0] + 1.25,
                        a.positions[i][1],
                        a.positions[i][2],
                    ],
                    b.positions[i],
                );
            }
            if v != 0. {
                assert!((a.positions[2][0] - 8.) * v as f32 > 0.);
            }
            material(&f, &a);
            material(&g, &b);
        }
    }
    #[test]
    fn flap_interpolation_keeps_source_fronts_and_independent_closures() {
        let a = 0xf001;
        let b = 0xf002;
        let f = synthetic(
            a,
            vec![[2., 3., 1.], [4., 3., 1.], [4., 0., 1.], [2., 0., 1.]],
        );
        let target = vec![[2., 3., 1.], [4., 3., 1.], [4., 1., -2.], [2., 1., -2.]];
        let closing = synthetic(b, vec![target[0], target[3], [2., 3., 0.]]);
        let r = Rig {
            flaps: [
                (
                    a,
                    Morph {
                        neutral: f.positions.clone(),
                        down: target.clone(),
                        closure: false,
                    },
                ),
                (
                    b,
                    Morph {
                        neutral: vec![f.positions[0], f.positions[3], [2., 3., 0.]],
                        down: closing.positions.clone(),
                        closure: true,
                    },
                ),
            ]
            .into(),
        };
        let mut s = state();
        for v in [0., 0.25, 0.5, 1.] {
            s.flaps = v;
            let p = r.animate(&f, &s).unwrap();
            close(p.positions[0], f.positions[0]);
            close(p.positions[1], f.positions[1]);
            if v == 0. {
                assert!(r.animate(&closing, &s).is_none());
                assert_eq!(p.positions, f.positions);
            } else {
                let q = r.animate(&closing, &s).unwrap();
                close(q.positions[1], p.positions[3]);
            }
            if v == 1. {
                assert_eq!(p.positions, target);
            }
            material(&f, &p);
        }
    }
    #[test]
    fn whole_gear_keeps_painted_root_and_rigidity_through_401_poses() {
        let r = rig();
        let mut s = state();
        for (a, pivot) in [
            (MAIN_LEFT[0], [-23., -1., -8.]),
            (MAIN_RIGHT[0], [21., -1., -8.]),
            (NOSE[0], [0., 38., -4.]),
        ] {
            let f = synthetic(
                a,
                vec![
                    pivot,
                    [pivot[0], pivot[1] - 2., pivot[2]],
                    [pivot[0], pivot[1] - 2., pivot[2] - 7.],
                    [pivot[0], pivot[1] + 3., pivot[2] - 7.],
                ],
            );
            for step in 1..=401 {
                s.gear = f64::from(step) / 401.;
                let p = r.animate(&f, &s).unwrap();
                close(p.positions[0], pivot);
                for i in 0..4 {
                    for j in i + 1..4 {
                        assert!(
                            (distance(f.positions[i], f.positions[j])
                                - distance(p.positions[i], p.positions[j]))
                            .abs()
                                < 1e-4
                        );
                    }
                }
                if step == 401 {
                    assert_eq!(p.positions, f.positions);
                }
                material(&f, &p);
            }
        }
    }
    #[test]
    fn only_fitted_exposed_main_assemblies_remain_visible_at_zero() {
        let r = rig();
        let mut s = state();
        s.gear = 0.;
        for a in GEAR {
            let f = synthetic(
                a,
                vec![[2., 1., -2.], [2., 3., -2.], [2., 3., -5.], [2., 1., -5.]],
            );
            assert_eq!(
                r.animate(&f, &s).is_some(),
                MAIN_LEFT.contains(&a) || MAIN_RIGHT.contains(&a)
            );
        }
    }
    #[test]
    fn root_marker_preserves_outline_and_interpolates_material() {
        let mut f = synthetic(
            0xf001,
            vec![[0., 0., 2.], [0., 4., 2.], [0., 4., -3.], [0., 0., -3.]],
        );
        f.uv = vec![[0., 0.], [8., 0.], [8., 10.], [0., 10.]];
        insert_root(&mut f, [0., 1., 2.]).unwrap();
        assert_eq!(f.positions[1], [0., 1., 2.]);
        assert_eq!(f.uv[1], [2., 0.]);
        assert_eq!(f.positions.len(), 5);
        assert_eq!(f.colors.len(), 5);
        assert!(
            insert_root(
                &mut synthetic(
                    0,
                    vec![[0., 0., 2.], [0., 4., 2.], [0., 4., -3.], [0., 0., -3.]]
                ),
                [1., 1., 2.]
            )
            .is_err()
        );
    }
    #[test]
    fn unsupported_device_demands_do_not_fabricate_art_or_move_body() {
        let r = rig();
        let mut s = state();
        s.brake = 1.;
        s.hook = 1.;
        s.exhaust = 2.;
        let f = synthetic(0xf001, vec![[1., 2., 3.], [4., 2., 3.], [4., 5., 3.]]);
        assert_eq!(r.animate(&f, &s).unwrap().positions, f.positions);
        assert!(!flame(f.address));
    }
}
