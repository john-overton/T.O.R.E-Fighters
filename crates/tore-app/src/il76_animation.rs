//! IL76.SH static rudder candidate, source flaps and independent fitted roll/gear.
//! Surface assignments and pivots are fitted; source code is never executed.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 4] = [0x7fc0, 0x7fc6, 0x7fcc, 0x7fd2];
const RUDDER: [usize; 2] = [0x4fa3, 0x4fca];
const LEFT_TAIL: [usize; 2] = [0x46fb, 0x4774];
const RIGHT_TAIL: [usize; 2] = [0x45b5, 0x463a];
const LEFT_ROLL: [usize; 2] = [0x409d, 0x40b4];
const RIGHT_ROLL: [usize; 2] = [0x38eb, 0x38ff];
const LEFT_GEAR: [usize; 8] = [
    0x4901, 0x4920, 0x497d, 0x499c, 0x49f9, 0x4a18, 0x4a37, 0x4a56,
];
const RIGHT_GEAR: [usize; 8] = [
    0x48c3, 0x48e2, 0x493f, 0x495e, 0x4a75, 0x4a94, 0x4ab3, 0x4ad2,
];
const NOSE: [usize; 4] = [0x4885, 0x48a4, 0x49bb, 0x49da];
const EPS: f32 = 1e-4;
struct Morph {
    neutral: Vec<[f32; 3]>,
    down: Vec<[f32; 3]>,
    closure: bool,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
fn face(shape: &Shape, a: usize) -> AppResult<&Face> {
    let mut fs = shape.faces.iter().filter(|f| f.address == a);
    let f = fs
        .next()
        .ok_or_else(|| format!("IL76.SH missing reviewed face {a:x}"))?;
    if fs.next().is_some() {
        return Err("IL76.SH duplicate reviewed face".into());
    }
    Ok(f)
}
fn roots(shape: &Shape, ids: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &a in ids {
        if !points
            .iter()
            .all(|p| face(shape, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("IL76.SH missing source attachment {a:x}").into());
        }
    }
    Ok(())
}
fn branch(
    bytes: &[u8],
    original: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let shape = Shape::with_state(bytes, &[(word, value)].into())?;
    let ids: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
    if ids.difference(original).copied().collect::<BTreeSet<_>>() != added.iter().copied().collect()
        || original.difference(&ids).copied().collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed IL76.SH branch {word:x}={value}").into());
    }
    Ok(shape)
}
fn tail(a: usize) -> bool {
    LEFT_TAIL.contains(&a) || RIGHT_TAIL.contains(&a)
}
fn gear(a: usize) -> bool {
    LEFT_GEAR.contains(&a) || RIGHT_GEAR.contains(&a) || NOSE.contains(&a)
}
fn tail_cut(p: [f32; 3]) -> f32 {
    p[1] + 113. + 0.30 * (p[0].abs() - 2.)
}
fn trailing(p: [f32; 3]) -> bool {
    p[0].abs() == 38. && p[1] == -24. || p[0].abs() == 86. && p[1] == -36.
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 28644
            || shape.faces.len() != 309
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed IL76.SH animation layout".into());
        }
        roots(&shape, &RUDDER, &[[0., -96., 20.], [0., -111., 57.]])?;
        roots(
            &shape,
            &LEFT_TAIL,
            &[
                [-1., -99., 58.],
                [-1., -120., 58.],
                [-35., -122., 60.],
                [-35., -129., 60.],
            ],
        )?;
        roots(
            &shape,
            &RIGHT_TAIL,
            &[
                [2., -99., 58.],
                [2., -120., 58.],
                [35., -122., 60.],
                [35., -129., 60.],
            ],
        )?;
        for (ids, side) in [(&LEFT_ROLL, -1.), (&RIGHT_ROLL, 1.)] {
            roots(
                &shape,
                ids,
                &[[side * 86., -36., 4.], [side * 123., -46., 1.]],
            )?;
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, bindings, ends) in [
            (
                0x7fcc,
                [
                    (0x4e58, 0x4ee0, [1, 2, 3, 0]),
                    (0x4e77, 0x4ec1, [1, 2, 3, 0]),
                ],
                [0x4eff],
            ),
            (
                0x7fd2,
                [
                    (0x4d71, 0x4df9, [3, 0, 1, 2]),
                    (0x4d90, 0x4dda, [3, 0, 1, 2]),
                ],
                [0x4e18],
            ),
        ] {
            let removed: Vec<_> = bindings.iter().map(|b| b.0).collect();
            let added: Vec<_> = bindings.iter().map(|b| b.1).chain(ends).collect();
            let down = branch(bytes, &original, word, -1, &added, &removed)?;
            branch(bytes, &original, word, 1, &[], &removed)?;
            let mut map = BTreeMap::new();
            for (a, b, order) in bindings {
                let base = face(&shape, a)?;
                let target = face(&down, b)?;
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("IL76.SH unreviewed flap topology".into());
                }
                let target: Vec<_> = order.iter().map(|&i| target.positions[i]).collect();
                for (p, q) in base.positions.iter().zip(&target) {
                    if !trailing(*p) && p != q {
                        return Err("IL76.SH source flap attachment moved".into());
                    }
                    map.insert(q.map(f32::to_bits), *p);
                }
                flaps.insert(
                    a,
                    Morph {
                        neutral: base.positions.clone(),
                        down: target,
                        closure: false,
                    },
                );
            }
            for a in ends {
                let mut f = face(&down, a)?.clone();
                let deployed = f.positions.clone();
                let neutral: Vec<_> = deployed
                    .iter()
                    .map(|p| {
                        map.get(&p.map(f32::to_bits))
                            .copied()
                            .ok_or("IL76.SH unreviewed flap closure endpoint")
                    })
                    .collect::<Result<_, _>>()?;
                f.positions = neutral.clone();
                flaps.insert(
                    a,
                    Morph {
                        neutral,
                        down: deployed,
                        closure: true,
                    },
                );
                closures.push(f);
            }
        }
        let ids: Vec<_> = LEFT_GEAR
            .iter()
            .chain(&RIGHT_GEAR)
            .chain(&NOSE)
            .copied()
            .collect();
        let down = branch(bytes, &original, 0x7fc6, 1, &ids, &[])?;
        branch(bytes, &original, 0x7fc6, -1, &[], &[])?;
        roots(
            &down,
            &[0x49f9, 0x4a18],
            &[[-6., -19., -18.], [-11., -19., -18.]],
        )?;
        roots(
            &down,
            &[0x4a37, 0x4a56],
            &[[-6., -2., -18.], [-11., -2., -18.]],
        )?;
        roots(
            &down,
            &[0x4a75, 0x4a94],
            &[[6., -19., -18.], [12., -19., -18.]],
        )?;
        roots(
            &down,
            &[0x4ab3, 0x4ad2],
            &[[6., -2., -18.], [12., -2., -18.]],
        )?;
        roots(
            &down,
            &[0x49bb, 0x49da],
            &[[-3., 45., -15.], [3., 45., -15.]],
        )?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if tail(f.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &f,
                    [0.; 3],
                    [1., 0., 0.],
                    0.,
                    tail_cut,
                ));
            } else {
                faces.push(f);
            }
        }
        faces.extend(closures);
        faces.extend(down.faces.into_iter().filter(|f| gear(f.address)));
        shape.faces = faces;
        validate_gear(&shape)?;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut f = source.clone();
        let a = f.address;
        if let Some(morph) = self.flaps.get(&a) {
            if morph.closure && state.flaps <= 0. {
                return None;
            }
            let amount = state.flaps.clamp(0., 1.) as f32;
            f.positions = morph
                .neutral
                .iter()
                .zip(&morph.down)
                .map(|(p, q)| std::array::from_fn(|i| p[i] + amount * (q[i] - p[i])))
                .collect();
        } else if RUDDER.contains(&a) && state.rudder != 0. {
            turn(
                &mut f,
                [0., -96., 20.],
                [0., -15., 37.],
                0.35 * state.rudder.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if [[0., -96., 20.], [0., -111., 57.]].contains(q) {
                    *p = *q;
                }
            }
        } else if tail(a)
            && source.positions.iter().all(|p| tail_cut(*p) <= EPS)
            && state.elevator != 0.
        {
            let side = if LEFT_TAIL.contains(&a) { -1. } else { 1. };
            turn(
                &mut f,
                [side * 2., -113., 58.],
                [1., -side * 0.3, 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if tail_cut(*q).abs() <= EPS || [-1., 2.].contains(&q[0]) && q[1] == -120. {
                    *p = *q;
                }
            }
        } else if (LEFT_ROLL.contains(&a) || RIGHT_ROLL.contains(&a)) && state.aileron != 0. {
            let side = if LEFT_ROLL.contains(&a) { -1. } else { 1. };
            turn(
                &mut f,
                [side * 88., -34., 4.],
                [side * 35., -12., -3.],
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if q[0].abs() != 86. || q[1] != -36. {
                    *p = *q;
                }
            }
        } else if gear(a) {
            if state.gear <= 0. {
                return None;
            }
            gear_pose(&mut f, state.gear);
            return Some(f);
        }
        if f.positions != source.positions {
            update_normal(source, &mut f);
        }
        Some(f)
    }
}
fn gear_pose(f: &mut Face, deployed: f64) {
    let u = 1. - deployed.clamp(0., 1.);
    if u == 0. {
        return;
    }
    let (pivot, axis, angle) = if LEFT_GEAR.contains(&f.address) {
        ([-7.5, 0., -17.], [0., 1., 0.], std::f64::consts::PI)
    } else if RIGHT_GEAR.contains(&f.address) {
        ([7.5, 0., -17.], [0., 1., 0.], -std::f64::consts::PI)
    } else {
        ([0., 45., -15.], [1., 0., 0.], -std::f64::consts::PI)
    };
    turn(f, pivot, axis, angle * u);
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for sample in 0..=20 {
        let g = f64::from(sample) / 20.;
        let mut left = f32::NEG_INFINITY;
        let mut right = f32::INFINITY;
        for source in shape.faces.iter().filter(|f| gear(f.address)) {
            let mut moved = source.clone();
            gear_pose(&mut moved, g);
            for (i, p) in source.positions.iter().enumerate() {
                let q = moved.positions[i];
                if !q.iter().all(|x| x.is_finite()) || g == 1. && *p != q {
                    return Err("IL76.SH invalid gear endpoint".into());
                }
                for (j, r) in source.positions.iter().enumerate().skip(i + 1) {
                    if (distance(*p, *r) - distance(q, moved.positions[j])).abs() > EPS {
                        return Err("IL76.SH whole gear card lost rigidity".into());
                    }
                }
                if p[2] <= -20. {
                    if LEFT_GEAR.contains(&source.address) {
                        left = left.max(q[0]);
                    } else if RIGHT_GEAR.contains(&source.address) {
                        right = right.min(q[0]);
                    }
                }
                if g == 0. {
                    let (lo, hi) = if LEFT_GEAR.contains(&source.address) {
                        ([-9., -24., -16.], [-4., 4., -9.])
                    } else if RIGHT_GEAR.contains(&source.address) {
                        ([3., -24., -16.], [9., 4., -9.])
                    } else {
                        ([-3., 39., -15.], [3., 51., -5.])
                    };
                    if (0..3).any(|k| q[k] < lo[k] - 0.001 || q[k] > hi[k] + 0.001) {
                        return Err("IL76.SH gear exceeds reviewed stow envelope".into());
                    }
                }
            }
        }
        if right - left < 6.99 {
            return Err("IL76.SH lower gear card margins crossed".into());
        }
    }
    Ok(())
}

fn normal(points: &[[f32; 3]]) -> Option<[f32; 3]> {
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
    (len > 1e-8).then(|| n.map(|v| (v / len) as f32))
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

#[cfg(test)]
mod tests {
    use super::*;
    fn panel(a: usize, p: Vec<[f32; 3]>) -> Face {
        Face {
            address: a,
            colors: vec![17; p.len()],
            uv: vec![[0.25, 0.75]; p.len()],
            positions: p,
            texture: "SYNTHETIC".into(),
            subtype: 0x6c,
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    #[test]
    fn separate_roll_keeps_thick_hinges_and_ignores_flaps() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        for (ids, side) in [(&LEFT_ROLL, -1.), (&RIGHT_ROLL, 1.)] {
            for roll in [-1., 0., 1.] {
                let mut points = Vec::new();
                for (&a, z) in ids.iter().zip([5., 3.]) {
                    let f = panel(
                        a,
                        vec![
                            [side * 88., -34., z],
                            [side * 123., -46., 1.],
                            [side * 86., -36., 4.],
                        ],
                    );
                    let mut s = state();
                    s.aileron = roll;
                    s.flaps = 1.;
                    let q = rig.animate(&f, &s).unwrap();
                    assert_eq!(q.positions[..2], f.positions[..2]);
                    s.flaps = 0.;
                    assert_eq!(q.positions, rig.animate(&f, &s).unwrap().positions);
                    if roll != 0. {
                        assert!((q.positions[2][2] - 4.) * side * roll as f32 > 0.);
                    }
                    points.push(q.positions[2]);
                }
                assert_eq!(points[0], points[1]);
            }
        }
    }
    #[test]
    fn exact_flap_endpoint_and_closure_are_independent_of_roll() {
        let neutral = vec![
            [41., -21., 4.],
            [38., -24., 7.],
            [86., -36., 4.],
            [88., -34., 3.],
        ];
        let down = vec![
            [41., -21., 4.],
            [39., -23., 5.],
            [86., -35., 2.],
            [88., -34., 3.],
        ];
        let f = panel(0x4d71, neutral.clone());
        let cn = vec![[86., -36., 4.], [88., -34., 5.], [88., -34., 3.]];
        let cd = vec![[86., -35., 2.], [88., -34., 5.], [88., -34., 3.]];
        let cap = panel(0x4e18, cn.clone());
        let rig = Rig {
            flaps: [
                (
                    f.address,
                    Morph {
                        neutral,
                        down: down.clone(),
                        closure: false,
                    },
                ),
                (
                    cap.address,
                    Morph {
                        neutral: cn,
                        down: cd.clone(),
                        closure: true,
                    },
                ),
            ]
            .into(),
        };
        for roll in [-1., 0., 1.] {
            let mut s = state();
            s.aileron = roll;
            s.flaps = 1.;
            assert_eq!(rig.animate(&f, &s).unwrap().positions, down);
            assert_eq!(rig.animate(&cap, &s).unwrap().positions, cd);
            s.flaps = 0.;
            assert!(rig.animate(&cap, &s).is_none());
            assert_eq!(rig.animate(&f, &s).unwrap().positions, f.positions);
        }
    }
    #[test]
    fn all_five_crossed_gear_assemblies_preserve_full_rigid_cards() {
        let mut faces = Vec::new();
        for (side, ids) in [(-1., &LEFT_GEAR[..]), (1., &RIGHT_GEAR[..])] {
            for &a in ids {
                let y = if matches!(
                    a,
                    0x4901 | 0x4920 | 0x49f9 | 0x4a18 | 0x48c3 | 0x48e2 | 0x4a75 | 0x4a94
                ) {
                    -19.
                } else {
                    -2.
                };
                let (lo, hi) = if side < 0. { (-11., -6.) } else { (6., 12.) };
                faces.push(panel(
                    a,
                    vec![[lo, y, -18.], [hi, y, -18.], [hi, y, -25.], [lo, y, -25.]],
                ));
            }
        }
        for a in NOSE {
            faces.push(panel(
                a,
                vec![
                    [-3., 45., -15.],
                    [3., 45., -15.],
                    [3., 45., -25.],
                    [-3., 45., -25.],
                ],
            ));
        }
        let shape = Shape {
            faces,
            lines: Vec::new(),
            state_words: BTreeSet::new(),
        };
        validate_gear(&shape).unwrap();
        for i in 0..=200 {
            for f in &shape.faces {
                let mut q = f.clone();
                gear_pose(&mut q, f64::from(i) / 200.);
                for (j, p) in f.positions.iter().enumerate() {
                    for (k, r) in f.positions.iter().enumerate().skip(j + 1) {
                        assert!(
                            (distance(*p, *r) - distance(q.positions[j], q.positions[k])).abs()
                                < EPS
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn static_rudder_assignment_starts_neutral_and_preserves_its_actual_edge() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let f = panel(
            0x4fa3,
            vec![
                [0., -96., 20.],
                [0., -110., 20.],
                [0., -125., 57.],
                [0., -111., 57.],
            ],
        );
        for yaw in [-1., 0., 1.] {
            let mut s = state();
            s.rudder = yaw;
            let q = rig.animate(&f, &s).unwrap();
            assert_eq!(q.positions[0], f.positions[0]);
            assert_eq!(q.positions[3], f.positions[3]);
            if yaw == 0. {
                assert_eq!(q.positions, f.positions);
            } else {
                assert!(q.positions[1][0] * yaw as f32 > 0.);
            }
        }
    }
}
