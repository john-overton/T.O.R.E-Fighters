//! Su25 exact source position/UV endpoints with explicit fitted interpolation.
use crate::{
    AppResult,
    additional_animation::turn,
    aircraft_animation::{split_surface, update_normal},
    flight::State,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::FRAC_PI_2,
};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 5] = [0x8390, 0x8396, 0x83a2, 0x83a8, 0x83ae];
const TAIL: [usize; 4] = [0x56a9, 0x56d6, 0x5810, 0x58b7];
const RUDDER: [usize; 2] = [0x5e68, 0x5e8f];
const ROLL_R: [usize; 2] = [0x4b9d, 0x4c10];
const ROLL_L: [usize; 2] = [0x4f0e, 0x4fb4];
const MAIN_R: [usize; 4] = [0x5ac0, 0x5adf, 0x5afe, 0x5b1d];
const MAIN_L: [usize; 4] = [0x5c10, 0x5c2f, 0x5c4e, 0x5c6d];
const DOOR_R: [usize; 2] = [0x5a4d, 0x5a64];
const DOOR_L: [usize; 2] = [0x5b9d, 0x5bb4];
const NOSE: [usize; 4] = [0x5d60, 0x5d7f, 0x5d9e, 0x5dbd];
const BRACE: [usize; 2] = [0x5ced, 0x5d04];
struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Vec<[f32; 3]>,
    neutral_uv: Vec<[f32; 2]>,
    deployed_uv: Vec<[f32; 2]>,
}
impl Morph {
    fn bind(base: &Face, target: &Face, point: impl Fn([f32; 3]) -> [f32; 3]) -> AppResult<Self> {
        if base.positions.len() != 4
            || target.positions.len() != 4
            || base.uv.len() != 4
            || target.uv.len() != 4
            || base.texture != target.texture
            || base.subtype != target.subtype
            || base.colors != target.colors
        {
            return Err("unreviewed SU25.SH source material correspondence".into());
        }
        let mut deployed = Vec::new();
        let mut deployed_uv = Vec::new();
        for p in &base.positions {
            let q = point(*p);
            let matches: Vec<_> = target
                .positions
                .iter()
                .enumerate()
                .filter(|(_, p)| **p == q)
                .collect();
            if matches.len() != 1 {
                return Err("unreviewed SU25.SH source point correspondence".into());
            }
            let i = matches[0].0;
            deployed.push(q);
            deployed_uv.push(target.uv[i]);
        }
        Ok(Self {
            neutral: base.positions.clone(),
            deployed,
            neutral_uv: base.uv.clone(),
            deployed_uv,
        })
    }
    fn apply(&self, result: &mut Face, t: f64) {
        let t = t.clamp(0., 1.) as f32;
        result.positions = self
            .neutral
            .iter()
            .zip(&self.deployed)
            .map(|(p, q)| std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t))
            .collect();
        result.uv = self
            .neutral_uv
            .iter()
            .zip(&self.deployed_uv)
            .map(|(p, q)| std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t))
            .collect();
    }
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
    rudder: BTreeMap<usize, [Morph; 2]>,
}
const BRAKE: [usize; 8] = [
    0x6226, 0x623d, 0x6254, 0x626b, 0x6282, 0x6299, 0x62b0, 0x62c7,
];
const GEAR: [usize; 18] = [
    0x5ced, 0x5d04, 0x5b9d, 0x5bb4, 0x5ac0, 0x5adf, 0x5afe, 0x5b1d, 0x5d60, 0x5d7f, 0x5d9e, 0x5dbd,
    0x5a4d, 0x5a64, 0x5c10, 0x5c2f, 0x5c4e, 0x5c6d,
];
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("SU25.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("SU25.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed SU25.SH root{address:x}").into());
        }
    }
    Ok(())
}
fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let pose = Shape::with_state(bytes, &[(word, value)].into())?;
    let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed SU25.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn flap_target(p: [f32; 3]) -> [f32; 3] {
    if p[1] == -11. && p[0].abs() == 14. {
        [p[0], -11., 3.]
    } else if p[1] == -12. && p[0].abs() == 42. {
        [p[0], -11., 1.]
    } else {
        p
    }
}
fn rudder_target(p: [f32; 3], sign: f32) -> [f32; 3] {
    if p[1] == -62. {
        [sign * 5., -60., 7.]
    } else if p[1] == -65. {
        [sign * 2., -63., 27.]
    } else {
        p
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 29626
            || shape.faces.len() != 334
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed SU25.SH layout".into());
        }
        roots(&shape, &RUDDER, &[[0., -53., 7.], [0., -60., 27.]])?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        let mut rudder = BTreeMap::new();
        for (word, bindings) in [
            (0x83a2, [(0x60a1, 0x6057), (0x60c0, 0x6030)]),
            (0x83a8, [(0x5fbf, 0x5f75), (0x5fde, 0x5f4e)]),
        ] {
            let added = [bindings[0].1, bindings[1].1];
            let removed = [bindings[0].0, bindings[1].0];
            let down = branch(bytes, &original, word, -1, &added, &removed)?;
            branch(bytes, &original, word, 1, &[], &removed)?;
            for (a, b) in bindings {
                flaps.insert(
                    a,
                    Morph::bind(one(&shape, a)?, one(&down, b)?, flap_target)?,
                );
            }
        }
        let positive = branch(bytes, &original, 0x83ae, -1, &[0x5dfb, 0x5e22], &RUDDER)?;
        let negative = branch(bytes, &original, 0x83ae, 1, &[0x5ed5, 0x5efc], &RUDDER)?;
        for (a, p, n) in [(0x5e68, 0x5dfb, 0x5ed5), (0x5e8f, 0x5e22, 0x5efc)] {
            let base = one(&shape, a)?;
            rudder.insert(
                a,
                [
                    Morph::bind(base, one(&positive, p)?, |p| rudder_target(p, 1.))?,
                    Morph::bind(base, one(&negative, n)?, |p| rudder_target(p, -1.))?,
                ],
            );
        }
        for (word, ids) in [(0x8390, &BRAKE[..]), (0x8396, &GEAR[..])] {
            let pose = branch(bytes, &original, word, 1, ids, &[])?;
            shape
                .faces
                .extend(pose.faces.into_iter().filter(|f| ids.contains(&f.address)));
        }
        roots(&shape, &[0x5afe, 0x5b1d], &[[6., 1., -9.], [14., 1., -9.]])?;
        roots(
            &shape,
            &[0x5c4e, 0x5c6d],
            &[[-6., 1., -9.], [-14., 1., -9.]],
        )?;
        roots(
            &shape,
            &[0x5d9e, 0x5dbd],
            &[[-2., 41., -9.], [2., 41., -9.]],
        )?;
        roots(&shape, &BRACE, &[[-1., 51., -9.], [-1., 51., -11.]])?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if TAIL.contains(&f.address) {
                faces.extend(split_surface(&f, [0., -54., 3.], [1., 0., 0.], 0., |p| {
                    p[1] + 54.
                }));
            } else {
                faces.push(f);
            }
        }
        shape.faces = faces;
        Ok((Self { flaps, rudder }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if let Some(m) = self.flaps.get(&a) {
            m.apply(&mut result, state.flaps);
            update_normal(source, &mut result);
        } else if let Some(pair) = self.rudder.get(&a) {
            pair[usize::from(state.rudder < 0.)].apply(&mut result, state.rudder.abs());
            update_normal(source, &mut result);
        } else if TAIL.contains(&a) {
            if source.positions.iter().all(|p| p[1] <= -54. + 1e-4) {
                turn(
                    &mut result,
                    [0., -54., 3.],
                    [1., 0., 0.],
                    -0.30 * state.elevator.clamp(-1., 1.),
                );
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if (old[1] + 54.).abs() < 1e-4 {
                        *p = *old;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if ROLL_L.contains(&a) || ROLL_R.contains(&a) {
            let left = ROLL_L.contains(&a);
            let mut moved = source.clone();
            turn(
                &mut moved,
                [if left { -42. } else { 42. }, -8., 3.5],
                [if left { -33. } else { 33. }, -2., -2.],
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for ((p, q), old) in result
                .positions
                .iter_mut()
                .zip(moved.positions)
                .zip(&source.positions)
            {
                if old[1] <= -12. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(
                &mut result,
                state.gear,
                if NOSE.contains(&a) {
                    state.nosewheel_angle()
                } else {
                    0.
                },
            );
            update_normal(source, &mut result);
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            let upper = [0x6254, 0x626b, 0x62b0, 0x62c7].contains(&a);
            let angle = if upper {
                (5f64 / 6.).atan()
            } else {
                -(4f64 / 6.).atan()
            };
            turn(
                &mut result,
                [0., -8., 1.],
                [1., 0., 0.],
                angle * (1. - state.brake.clamp(0., 1.)),
            );
        }
        Some(result)
    }
}
fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let closing = 1. - gear.clamp(0., 1.);
    let a = f.address;
    if MAIN_R.contains(&a) || MAIN_L.contains(&a) {
        turn(
            f,
            [if MAIN_L.contains(&a) { -11. } else { 10. }, 1., -9.],
            [1., 0., 0.],
            -FRAC_PI_2 * closing,
        );
    } else if DOOR_R.contains(&a) || DOOR_L.contains(&a) {
        let left = DOOR_L.contains(&a);
        turn(
            f,
            [
                if left { -6. } else { 6. },
                if left { -2.5 } else { -1.5 },
                -10.,
            ],
            [0., 1., 0.],
            if left {
                -FRAC_PI_2 * closing
            } else {
                FRAC_PI_2 * closing
            },
        );
    } else if BRACE.contains(&a) {
        let old = f.positions.clone();
        turn(f, [0., 41., -9.], [1., 0., 0.], -FRAC_PI_2 * closing);
        for (p, q) in f.positions.iter_mut().zip(old) {
            if q[1] == 51. {
                *p = q;
            }
        }
    } else {
        turn(f, [0., 41., -9.], [0., 0., 1.], -steering);
        turn(f, [0., 41., -9.], [1., 0., 0.], -FRAC_PI_2 * closing);
    }
}
pub(crate) fn flame(_: usize) -> bool {
    false
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            address,
            colors: vec![17; positions.len()],
            uv: vec![[10., 1.]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            normal: Some([1., 0., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
            subtype: 0xed,
        }
    }
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
            rudder: BTreeMap::new(),
        }
    }
    fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
    }
    #[test]
    fn signed_rudder_keeps_diagonal_roots_and_exact_position_uv_endpoints() {
        let base = face(
            0x5e68,
            vec![
                [0., -62., 7.],
                [0., -53., 7.],
                [0., -60., 27.],
                [0., -65., 27.],
            ],
        );
        let mut positive = face(
            0x5dfb,
            vec![
                [0., -53., 7.],
                [0., -60., 27.],
                [2., -63., 27.],
                [5., -60., 7.],
            ],
        );
        positive.uv = vec![[10., 1.], [10., 1.], [12., 2.], [13., 3.]];
        let mut negative = face(
            0x5ed5,
            vec![
                [0., -60., 27.],
                [-2., -63., 27.],
                [-5., -60., 7.],
                [0., -53., 7.],
            ],
        );
        negative.uv = vec![[10., 1.], [8., 2.], [6., 3.], [10., 1.]];
        let pair = [
            Morph::bind(&base, &positive, |p| rudder_target(p, 1.)).unwrap(),
            Morph::bind(&base, &negative, |p| rudder_target(p, -1.)).unwrap(),
        ];
        let rig = Rig {
            flaps: BTreeMap::new(),
            rudder: [(base.address, pair)].into(),
        };
        let mut s = state();
        for v in [-1., -0.5, 0., 0.5, 1.] {
            s.rudder = v;
            let out = rig.animate(&base, &s).unwrap();
            assert_eq!(out.positions[1], base.positions[1]);
            assert_eq!(out.positions[2], base.positions[2]);
            assert_eq!(out.uv[1], base.uv[1]);
            assert_eq!(out.uv[2], base.uv[2]);
            if v == 0. {
                assert_eq!(out.positions, base.positions);
                assert_eq!(out.uv, base.uv);
            } else {
                assert!(out.positions[0][0] * v as f32 > 0.);
                assert!(out.uv.iter().any(|p| p[0] != 10.));
            }
            if v.abs() == 1. {
                let target = if v > 0. { &positive } else { &negative };
                for (p, uv) in out.positions.iter().zip(&out.uv) {
                    let i = target.positions.iter().position(|q| q == p).unwrap();
                    assert_eq!(*uv, target.uv[i]);
                }
            }
        }
    }
    #[test]
    fn paired_rudder_skins_have_shared_geometry_and_own_material_order() {
        let mut a = face(
            0x5e68,
            vec![
                [0., -62., 7.],
                [0., -53., 7.],
                [0., -60., 27.],
                [0., -65., 27.],
            ],
        );
        a.uv = vec![[10., 4.], [10., 3.], [10., 2.], [10., 1.]];
        let mut b = face(0x5e8f, a.positions.iter().rev().copied().collect());
        b.uv = a.uv.iter().rev().copied().collect();
        let mut ap = face(
            0x5dfb,
            a.positions
                .iter()
                .copied()
                .map(|p| rudder_target(p, 1.))
                .collect(),
        );
        ap.uv = vec![[13., 4.], [10., 3.], [10., 2.], [12., 1.]];
        let mut bp = face(0x5e22, ap.positions.iter().rev().copied().collect());
        bp.uv = ap.uv.iter().rev().copied().collect();
        let ma = Morph::bind(&a, &ap, |p| rudder_target(p, 1.)).unwrap();
        let mb = Morph::bind(&b, &bp, |p| rudder_target(p, 1.)).unwrap();
        for t in [0., 0.25, 0.5, 0.75, 1.] {
            let mut aa = a.clone();
            let mut bb = b.clone();
            ma.apply(&mut aa, t);
            mb.apply(&mut bb, t);
            assert_eq!(
                aa.positions,
                bb.positions.iter().rev().copied().collect::<Vec<_>>()
            );
            assert_eq!(aa.uv, bb.uv.iter().rev().copied().collect::<Vec<_>>());
            if t > 0. {
                assert!(aa.uv.iter().any(|uv| uv[0] != 10.));
                assert!(bb.uv.iter().any(|uv| uv[0] != 10.));
            }
            assert_eq!(aa.texture, a.texture);
            assert_eq!(bb.texture, b.texture);
        }
    }
    #[test]
    fn fixed_fin_and_forward_roll_triangles_never_move() {
        let rig = rig();
        let mut s = state();
        s.rudder = 1.;
        s.aileron = 1.;
        for a in [0x572f, 0x58ed, 0x4bed, 0x4f5e] {
            let source = face(a, vec![[0., -35., 11.], [0., -55., 35.], [0., -60., 27.]]);
            assert_eq!(
                rig.animate(&source, &s).unwrap().positions,
                source.positions
            );
        }
    }
    #[test]
    fn main_and_inner_panel_cards_are_rigid_separate_and_preserve_top_edges_at_401_samples() {
        for (a, source, bound) in [
            (
                0x5afe,
                face(
                    0x5afe,
                    vec![
                        [6., 1., -9.],
                        [14., 1., -9.],
                        [14., -4., -22.],
                        [6., -4., -22.],
                    ],
                ),
                6.,
            ),
            (
                0x5c4e,
                face(
                    0x5c4e,
                    vec![
                        [-6., 1., -9.],
                        [-6., -4., -22.],
                        [-14., -4., -22.],
                        [-14., 1., -9.],
                    ],
                ),
                6.,
            ),
            (
                0x5a4d,
                face(
                    0x5a4d,
                    vec![
                        [6., 6., -14.],
                        [6., 6., -10.],
                        [6., -9., -10.],
                        [6., -9., -14.],
                    ],
                ),
                2.,
            ),
            (
                0x5b9d,
                face(
                    0x5b9d,
                    vec![
                        [-6., -10., -14.],
                        [-6., 5., -14.],
                        [-6., 5., -10.],
                        [-6., -10., -10.],
                    ],
                ),
                2.,
            ),
        ] {
            for step in 0..=400 {
                let mut out = source.clone();
                gear_positions(&mut out, f64::from(step) / 400., 0.);
                let side = source.positions[0][0].signum();
                assert!(out.positions.iter().all(|p| p[0] * side >= bound - 1e-4));
                for (p, q) in source.positions.iter().zip(&out.positions) {
                    if (MAIN_R.contains(&a) || MAIN_L.contains(&a)) && p[1] == 1. && p[2] == -9.
                        || !MAIN_R.contains(&a) && !MAIN_L.contains(&a) && p[2] == -10.
                    {
                        assert_eq!(p, q);
                    }
                }
                for i in 0..4 {
                    for j in i + 1..4 {
                        assert!(
                            (d2(source.positions[i], source.positions[j])
                                - d2(out.positions[i], out.positions[j]))
                            .abs()
                                < 1e-3
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn nose_front_and_brace_body_edges_stay_fixed_with_rigid_distal_pin() {
        let nose = face(
            0x5d9e,
            vec![
                [-2., 39., -23.],
                [2., 39., -23.],
                [2., 41., -9.],
                [-2., 41., -9.],
            ],
        );
        let brace = face(
            0x5ced,
            vec![
                [-1., 35., -11.],
                [-1., 35., -9.],
                [-1., 51., -9.],
                [-1., 51., -11.],
            ],
        );
        for step in 0..=400 {
            let gear = f64::from(step) / 400.;
            let mut out = nose.clone();
            gear_positions(&mut out, gear, 0.);
            assert_eq!(out.positions[2], nose.positions[2]);
            assert_eq!(out.positions[3], nose.positions[3]);
            let mut out = brace.clone();
            gear_positions(&mut out, gear, 0.);
            assert_eq!(out.positions[2], brace.positions[2]);
            assert_eq!(out.positions[3], brace.positions[3]);
            assert!((d2(out.positions[0], out.positions[1]) - 4.).abs() < 1e-4);
        }
    }
    #[test]
    fn tail_cut_and_thick_roll_roots_are_fixed_with_signed_trailing_motion() {
        let rig = rig();
        let mut s = state();
        s.elevator = 1.;
        let source = face(
            0x56a9,
            vec![
                [3., -54., 3.],
                [25., -54., 3.],
                [25., -61., 3.],
                [2., -61., 3.],
            ],
        );
        let out = rig.animate(&source, &s).unwrap();
        assert_eq!(out.positions[0], source.positions[0]);
        assert_eq!(out.positions[1], source.positions[1]);
        assert!(out.positions[2][2] > 3.);
        s.elevator = 0.;
        for (a, side) in [(0x4b9d, 1.), (0x4f0e, -1.)] {
            let source = face(
                a,
                vec![
                    [side * 75., -13., 1.],
                    [side * 42., -12., 3.],
                    [side * 42., -8., 4.],
                    [side * 75., -10., 2.],
                ],
            );
            for value in [-1., 1.] {
                s.aileron = value;
                let out = rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[2], source.positions[2]);
                assert_eq!(out.positions[3], source.positions[3]);
                assert!((out.positions[0][2] - 1.) * value as f32 * side > 0.);
            }
        }
    }
}
