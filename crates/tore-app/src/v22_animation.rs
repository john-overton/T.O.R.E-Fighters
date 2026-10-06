//! V22 source-owned controls with explicit fitted flaperons and rigid gear cards.
//! Shared nacelle and rotor animation is deliberately left to postprocessing.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::{FRAC_PI_2, PI},
};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 3] = [0x5310, 0x531c, 0x5322];
const TAIL_L: [usize; 2] = [0x23dc, 0x2463];
const TAIL_R: [usize; 2] = [0x2691, 0x2718];
const FIN_L: [usize; 2] = [0x248b, 0x24c1];
const FIN_R: [usize; 2] = [0x27c3, 0x2813];
const GEAR: [usize; 12] = [
    0x2a7e, 0x2aa6, 0x2ac2, 0x2adf, 0x2b4d, 0x2b69, 0x2b91, 0x2bad, 0x2c1c, 0x2c44, 0x2c60, 0x2c7d,
];
struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Vec<[f32; 3]>,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("V22.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("V22.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed V22.SH root{address:x}").into());
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
        return Err(format!("unreviewed V22.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn fin_distance(p: [f32; 3]) -> f32 {
    p[1] + 128. + (4. / 29.) * (p[2] - 1.)
}
fn flaperon(address: usize) -> Option<(f32, [f32; 3], [f32; 3])> {
    if [0x39de, 0x3a03, 0x2198].contains(&address) {
        Some((20., [-20., -4., 10.5], [-75., 9., 0.]))
    } else if [0x3895, 0x38ba, 0x29cf].contains(&address) {
        Some((19., [19., -4., 10.5], [75., 9., 0.]))
    } else {
        None
    }
}
fn trailing(p: [f32; 3], inner: f32) -> bool {
    p[1] < -5. + (p[0].abs() - inner) * 9. / 75.
}
fn gear_spec(a: usize) -> ([f32; 3], [f32; 3], f64) {
    if a < 0x2b00 {
        ([0., 82., -28.], [1., 0., 0.], -FRAC_PI_2)
    } else if a < 0x2c00 {
        ([23., -4., -29.], [0., 1., 0.], PI)
    } else {
        ([-23., -4., -29.], [0., 1., 0.], -PI)
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 17204
            || shape.faces.len() != 160
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed V22.SH layout".into());
        }
        roots(
            &shape,
            &TAIL_L,
            &[
                [-12., -109., 1.],
                [-36., -109., 1.],
                [-36., -139., 1.],
                [-12., -139., 1.],
            ],
        )?;
        roots(
            &shape,
            &TAIL_R,
            &[
                [12., -109., 1.],
                [36., -109., 1.],
                [36., -139., 1.],
                [12., -139., 1.],
            ],
        )?;
        for (ids, x) in [(FIN_L, -36.), (FIN_R, 36.)] {
            roots(
                &shape,
                &ids,
                &[
                    [x, -139., 1.],
                    [x, -139., 30.],
                    [x, -125., 30.],
                    [x, -109., 1.],
                ],
            )?;
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        for (word, bindings, cap) in [
            (
                0x531c,
                [
                    (0x39de, 0x394e, [0, 1, 2, 3]),
                    (0x3a03, 0x3973, [3, 0, 1, 2]),
                ],
                0x393a,
            ),
            (
                0x5322,
                [
                    (0x3895, 0x382a, [0, 1, 2, 3]),
                    (0x38ba, 0x3805, [2, 3, 0, 1]),
                ],
                0x37f1,
            ),
        ] {
            let added = [bindings[0].1, bindings[1].1, cap];
            let removed = [bindings[0].0, bindings[1].0];
            let pose = branch(bytes, &original, word, -1, &added, &removed)?;
            branch(bytes, &original, word, 1, &[], &removed)?;
            for (a, b, order) in bindings {
                let source = one(&shape, a)?;
                let down = one(&pose, b)?;
                if source.positions.len() != 4 || down.positions.len() != 4 {
                    return Err("unreviewed V22.SH flap topology".into());
                }
                let deployed: Vec<_> = order.into_iter().map(|i| down.positions[i]).collect();
                let inner = if word == 0x531c { 20. } else { 19. };
                for (p, q) in source.positions.iter().zip(&deployed) {
                    if !trailing(*p, inner) && p != q {
                        return Err("unreviewed V22.SH fixed thick flap edge".into());
                    }
                    if trailing(*p, inner) && *q != [p[0], p[1] + 2., 3.] {
                        return Err("unreviewed V22.SH flap trailing endpoint".into());
                    }
                }
                flaps.insert(
                    a,
                    Morph {
                        neutral: source.positions.clone(),
                        deployed,
                    },
                );
            }
        }
        roots(
            &shape,
            &[0x2198],
            &[[-95., -6., 11.], [-95., 5., 6.], [-95., 5., 15.]],
        )?;
        roots(
            &shape,
            &[0x29cf],
            &[[94., -6., 11.], [94., 5., 6.], [94., 5., 15.]],
        )?;
        let gear = branch(bytes, &original, 0x5310, 1, &GEAR, &[])?;
        for a in GEAR {
            let f = one(&gear, a)?;
            if f.texture != "_V22.PIC" || f.positions.len() != 4 {
                return Err("unreviewed V22.SH original wheel cards".into());
            }
        }
        roots(
            &gear,
            &[0x2b4d, 0x2b69],
            &[[23., -8., -29.], [23., 0., -29.]],
        )?;
        roots(
            &gear,
            &[0x2b91, 0x2bad],
            &[[19., -4., -29.], [27., -4., -29.]],
        )?;
        roots(
            &gear,
            &[0x2c1c, 0x2c7d],
            &[[-23., -8., -29.], [-23., 0., -29.]],
        )?;
        roots(
            &gear,
            &[0x2c44, 0x2c60],
            &[[-27., -4., -29.], [-19., -4., -29.]],
        )?;
        roots(
            &gear,
            &[0x2a7e, 0x2adf],
            &[[-2., 82., -28.], [2., 82., -28.]],
        )?;
        roots(
            &gear,
            &[0x2aa6, 0x2ac2],
            &[[0., 79., -29.], [0., 86., -28.]],
        )?;
        shape
            .faces
            .extend(gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)));
        let mut faces = Vec::new();
        for f in shape.faces {
            if TAIL_L.contains(&f.address) || TAIL_R.contains(&f.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &f,
                    [0., -123., 1.],
                    [1., 0., 0.],
                    0.,
                    |p| p[1] + 123.,
                ));
            } else if FIN_L.contains(&f.address) || FIN_R.contains(&f.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &f,
                    [
                        if FIN_L.contains(&f.address) {
                            -36.
                        } else {
                            36.
                        },
                        -128.,
                        1.,
                    ],
                    [0., -4., 29.],
                    0.,
                    fin_distance,
                ));
            } else {
                faces.push(f);
            }
        }
        shape.faces = faces;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if TAIL_L.contains(&a) || TAIL_R.contains(&a) {
            if source.positions.iter().all(|p| p[1] <= -123. + 1e-4) {
                turn(
                    &mut result,
                    [0., -123., 1.],
                    [1., 0., 0.],
                    -0.30 * state.elevator.clamp(-1., 1.),
                );
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if (old[1] + 123.).abs() < 1e-4 {
                        *p = *old;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if FIN_L.contains(&a) || FIN_R.contains(&a) {
            if source.positions.iter().all(|p| fin_distance(*p) <= 1e-4) {
                turn(
                    &mut result,
                    [if FIN_L.contains(&a) { -36. } else { 36. }, -128., 1.],
                    [0., -4., 29.],
                    0.35 * state.rudder.clamp(-1., 1.),
                );
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if fin_distance(*old).abs() < 1e-4 {
                        *p = *old;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if let Some((inner, pivot, axis)) = flaperon(a) {
            let t = state.flaps.clamp(0., 1.) as f32;
            if let Some(m) = self.flaps.get(&a) {
                result.positions = m
                    .neutral
                    .iter()
                    .zip(&m.deployed)
                    .map(|(p, q)| std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t))
                    .collect();
            } else {
                for p in &mut result.positions {
                    if trailing(*p, inner) {
                        p[1] += 2. * t;
                        p[2] -= 8. * t;
                    }
                }
            }
            let mut turned = result.clone();
            turn(
                &mut turned,
                pivot,
                axis,
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for ((p, q), old) in result
                .positions
                .iter_mut()
                .zip(turned.positions)
                .zip(&source.positions)
            {
                if trailing(*old, inner) {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            let (pivot, axis, angle) = gear_spec(a);
            turn(
                &mut result,
                pivot,
                axis,
                angle * (1. - state.gear.clamp(0., 1.)),
            );
        }
        Some(result)
    }
}
#[cfg(test)]
fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}
fn normal(p: &[[f32; 3]]) -> Option<[f32; 3]> {
    let mut n = [0f64; 3];
    for (a, b) in p.iter().zip(p.iter().cycle().skip(1)).take(p.len()) {
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
        normal(&source.positions),
        normal(&result.positions),
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
            uv: vec![[0.1, 0.9]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
            subtype: 0xed,
        }
    }
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
        }
    }
    #[test]
    fn flaperon_has_exact_flap_endpoint_fixed_thick_leading_edges_and_coherent_tip_at_25_poses() {
        let left = face(
            0x39de,
            vec![
                [-20., -4., 6.],
                [-95., 5., 6.],
                [-95., -6., 11.],
                [-20., -14., 11.],
            ],
        );
        let down = vec![
            [-20., -4., 6.],
            [-95., 5., 6.],
            [-95., -4., 3.],
            [-20., -12., 3.],
        ];
        let top = face(
            0x3a03,
            vec![
                [-20., -14., 11.],
                [-95., -6., 11.],
                [-95., 5., 15.],
                [-20., -4., 15.],
            ],
        );
        let down_top = vec![
            [-20., -12., 3.],
            [-95., -4., 3.],
            [-95., 5., 15.],
            [-20., -4., 15.],
        ];
        let tip = face(
            0x2198,
            vec![
                [-95., 5., 15.],
                [-95., -6., 11.],
                [-95., 5., 6.],
                [-95., 27., 6.],
                [-95., 30., 11.],
                [-95., 27., 15.],
            ],
        );
        let rig = Rig {
            flaps: [
                (
                    left.address,
                    Morph {
                        neutral: left.positions.clone(),
                        deployed: down.clone(),
                    },
                ),
                (
                    top.address,
                    Morph {
                        neutral: top.positions.clone(),
                        deployed: down_top.clone(),
                    },
                ),
            ]
            .into(),
        };
        let mut s = state();
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                s.flaps = flap;
                s.aileron = roll;
                let lower = rig.animate(&left, &s).unwrap();
                let upper = rig.animate(&top, &s).unwrap();
                let cap = rig.animate(&tip, &s).unwrap();
                assert_eq!(lower.positions[0], left.positions[0]);
                assert_eq!(lower.positions[1], left.positions[1]);
                assert_eq!(upper.positions[2], top.positions[2]);
                assert_eq!(upper.positions[3], top.positions[3]);
                assert_eq!(lower.positions[2], upper.positions[1]);
                assert_eq!(lower.positions[2], cap.positions[1]);
                assert_eq!(lower.positions[3], upper.positions[0]);
                if roll == 0. && flap == 1. {
                    assert_eq!(lower.positions, down);
                    assert_eq!(upper.positions, down_top);
                }
                if flap == 0. && roll != 0. {
                    assert!((lower.positions[2][2] - 11.) * (roll as f32) < 0.);
                }
                assert!(lower.positions.iter().flatten().all(|v| v.is_finite()));
            }
        }
    }
    #[test]
    fn both_tail_partitions_have_fixed_cut_and_signed_pitch() {
        let rig = rig();
        let mut s = state();
        for (a, x) in [(0x23dc, -36.), (0x2691, 36.)] {
            let source = face(
                a,
                vec![
                    [x / 3., -109., 1.],
                    [x, -109., 1.],
                    [x, -139., 1.],
                    [x / 3., -139., 1.],
                ],
            );
            let parts = crate::aircraft_animation::split_surface(
                &source,
                [0., -123., 1.],
                [1., 0., 0.],
                0.,
                |p| p[1] + 123.,
            );
            for value in [-1., 0., 1.] {
                s.elevator = value;
                for f in &parts {
                    let out = rig.animate(f, &s).unwrap();
                    for (p, q) in f.positions.iter().zip(out.positions) {
                        if (p[1] + 123.).abs() < 1e-4 {
                            assert_eq!(*p, q);
                        } else if p[1] < -123. && value != 0. {
                            assert!((q[2] - p[2]) * value as f32 > 0.);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn complete_main_wheel_cards_preserve_virtual_shafts_and_never_cross_centerline_at_401_poses() {
        let rig = rig();
        let mut s = state();
        for (a, x) in [(0x2b91, 23.), (0x2c44, -23.)] {
            let source = face(
                a,
                vec![
                    [x - 4., -4., -39.],
                    [x + 4., -4., -39.],
                    [x + 4., -4., -29.],
                    [x - 4., -4., -29.],
                ],
            );
            for step in 0..=400 {
                s.gear = f64::from(step) / 400.;
                if step == 0 {
                    assert!(rig.animate(&source, &s).is_none());
                    continue;
                }
                let out = rig.animate(&source, &s).unwrap();
                let center: [f32; 3] =
                    std::array::from_fn(|i| (out.positions[2][i] + out.positions[3][i]) * 0.5);
                assert!(distance2(center, [x, -4., -29.]) < 1e-8);
                assert!(out.positions.iter().all(|p| p[0] * x.signum() > 12.));
                for i in 0..4 {
                    for j in i + 1..4 {
                        assert!(
                            (distance2(source.positions[i], source.positions[j])
                                - distance2(out.positions[i], out.positions[j]))
                            .abs()
                                < 1e-3
                        );
                    }
                }
                if step == 400 {
                    assert_eq!(out.positions, source.positions);
                }
            }
        }
    }
    #[test]
    fn nose_cards_fold_aft_rigidly_and_all_twelve_gear_faces_hide_at_zero() {
        let rig = rig();
        let mut s = state();
        s.gear = 0.;
        for a in GEAR {
            assert!(rig.animate(&face(a, vec![[0.; 3]; 3]), &s).is_none());
        }
        let source = face(
            0x2a7e,
            vec![
                [-2., 82., -39.],
                [2., 82., -39.],
                [2., 82., -28.],
                [-2., 82., -28.],
            ],
        );
        s.gear = 1e-8;
        let out = rig.animate(&source, &s).unwrap();
        assert_eq!(out.positions[2], source.positions[2]);
        assert_eq!(out.positions[3], source.positions[3]);
        for p in &out.positions {
            assert!(p[1] >= 71. - 1e-4 && p[1] <= 82. + 1e-4);
            assert!(p[2] >= -28. - 1e-4 && p[2] <= -28. + 1e-4);
        }
        assert!(
            (distance2(source.positions[0], source.positions[2])
                - distance2(out.positions[0], out.positions[2]))
            .abs()
                < 1e-3
        );
    }
}
