//! AWACS.SH neutral rudder, source flap endpoints and fitted flaperon/gear motion.
//! Pivot choices and mixing are fitted; source code is never executed.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 5] = [0x7880, 0x7886, 0x7892, 0x7898, 0x789e];
const RUDDER: [usize; 2] = [0x51db, 0x51f2];
const LEFT_TAIL: [usize; 2] = [0x2f7a, 0x2f8d];
const RIGHT_TAIL: [usize; 2] = [0x2fcc, 0x2fdf];
const LEFT_GEAR: [usize; 4] = [0x4336, 0x435d, 0x4384, 0x43a3];
const RIGHT_GEAR: [usize; 4] = [0x4265, 0x428c, 0x42b3, 0x42d2];
const NOSE: [usize; 3] = [0x4407, 0x442e, 0x4455];
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
        .ok_or_else(|| format!("AWACS.SH missing reviewed face {a:x}"))?;
    if fs.next().is_some() {
        return Err("AWACS.SH duplicate reviewed face".into());
    }
    Ok(f)
}
fn roots(shape: &Shape, ids: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &a in ids {
        if !points
            .iter()
            .all(|p| face(shape, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("AWACS.SH missing source attachment {a:x}").into());
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
        return Err(format!("unreviewed AWACS.SH branch {word:x}={value}").into());
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
    p[1] + 107. + 0.30 * (p[0].abs() - 7.)
}
fn trailing(p: [f32; 3]) -> bool {
    p[0].abs() == 45. && p[1] == -22. || p[0].abs() == 101. && p[1] == -48.
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 26794
            || shape.faces.len() != 296
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed AWACS.SH animation layout".into());
        }
        roots(&shape, &RUDDER, &[[0., -104., 13.], [0., -109., 38.]])?;
        roots(
            &shape,
            &LEFT_TAIL,
            &[
                [-7., -87., 9.],
                [-2., -113., 9.],
                [-38., -111., 12.],
                [-38., -125., 12.],
            ],
        )?;
        roots(
            &shape,
            &RIGHT_TAIL,
            &[
                [7., -87., 9.],
                [2., -113., 9.],
                [39., -111., 12.],
                [39., -125., 12.],
            ],
        )?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for (value, ids) in [(1, [0x5141, 0x5158]), (-1, [0x518e, 0x51a5])] {
            let pose = branch(bytes, &original, 0x789e, value, &ids, &RUDDER)?;
            roots(&pose, &ids, &[[0., -104., 13.], [0., -109., 38.]])?;
        }
        let mut flaps = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, bindings, ends) in [
            (
                0x7892,
                [
                    (0x5003, 0x50ba, [2, 3, 0, 1]),
                    (0x5022, 0x50d9, [1, 2, 3, 0]),
                ],
                [0x50f8, 0x510d],
            ),
            (
                0x7898,
                [
                    (0x4eb9, 0x4f70, [2, 3, 0, 1]),
                    (0x4ed8, 0x4f8f, [3, 0, 1, 2]),
                ],
                [0x4fae, 0x4fc3],
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
                    return Err("AWACS.SH unreviewed flap topology".into());
                }
                let target: Vec<_> = order.iter().map(|&i| target.positions[i]).collect();
                for (p, q) in base.positions.iter().zip(&target) {
                    if !trailing(*p) && p != q {
                        return Err("AWACS.SH source flap attachment moved".into());
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
                            .ok_or("AWACS.SH unreviewed flap closure endpoint")
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
        let down = branch(bytes, &original, 0x7886, 1, &ids, &[])?;
        branch(bytes, &original, 0x7886, -1, &[], &[])?;
        roots(
            &down,
            &[0x42b3, 0x42d2],
            &[[3., -26., -5.], [9., -26., -1.]],
        )?;
        roots(
            &down,
            &[0x4384, 0x43a3],
            &[[-2., -26., -5.], [-8., -26., -2.]],
        )?;
        roots(&down, &[0x4407, 0x442e], &[[0., 69., -7.], [0., 77., -7.]])?;
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
            if morph.closure && state.flaps <= 0. && state.aileron == 0. {
                return None;
            }
            let amount = state.flaps.clamp(0., 1.) as f32;
            f.positions = morph
                .neutral
                .iter()
                .zip(&morph.down)
                .map(|(p, q)| std::array::from_fn(|i| p[i] + amount * (q[i] - p[i])))
                .collect();
            if state.aileron != 0. {
                let side = if morph.neutral[0][0] < 0. { -1. } else { 1. };
                let mut rolled = f.clone();
                turn(
                    &mut rolled,
                    [side * 48., -19., -0.5],
                    [side * 56., -26., 4.],
                    -0.20 * state.aileron.clamp(-1., 1.),
                );
                for ((p, q), neutral) in f
                    .positions
                    .iter_mut()
                    .zip(rolled.positions)
                    .zip(&morph.neutral)
                {
                    if trailing(*neutral) {
                        *p = q;
                    }
                }
            }
        } else if RUDDER.contains(&a) && state.rudder != 0. {
            turn(
                &mut f,
                [0., -104., 13.],
                [0., -5., 25.],
                0.35 * state.rudder.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if [[0., -104., 13.], [0., -109., 38.]].contains(q) {
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
                [side * 7., -107., 9.],
                [1., -side * 0.3, 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if tail_cut(*q).abs() <= EPS || q[0].abs() == 2. && q[1] == -113. {
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
        ([-4.22, -26., -3.94], [0., 1., 0.], std::f64::consts::PI)
    } else if RIGHT_GEAR.contains(&f.address) {
        ([4.525, -26., -3.65], [0., 1., 0.], -std::f64::consts::PI)
    } else {
        ([0., 72., -7.], [1., 0., 0.], -std::f64::consts::PI)
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
                    return Err("AWACS.SH invalid gear endpoint".into());
                }
                for (j, r) in source.positions.iter().enumerate().skip(i + 1) {
                    if (distance(*p, *r) - distance(q, moved.positions[j])).abs() > EPS {
                        return Err("AWACS.SH whole gear card lost rigidity".into());
                    }
                }
                if p[2] <= -10. {
                    if LEFT_GEAR.contains(&source.address) {
                        left = left.max(q[0]);
                    } else if RIGHT_GEAR.contains(&source.address) {
                        right = right.min(q[0]);
                    }
                }
                if g == 0. {
                    let (lo, hi) = if LEFT_GEAR.contains(&source.address) {
                        ([-6.44, -34., -5.88], [-0.44, -18., 10.12])
                    } else if RIGHT_GEAR.contains(&source.address) {
                        ([0.05, -34., -6.3], [6.05, -18., 10.7])
                    } else {
                        ([-2., 67., -7.], [2., 75., 5.])
                    };
                    if (0..3).any(|k| q[k] < lo[k] - 0.001 || q[k] > hi[k] + 0.001) {
                        return Err("AWACS.SH gear exceeds reviewed stow envelope".into());
                    }
                }
            }
        }
        if right - left < 0.48 {
            return Err("AWACS.SH lower gear card margins crossed".into());
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
    fn panel(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            address,
            colors: vec![17; positions.len()],
            uv: vec![[0.25, 0.75]; positions.len()],
            positions,
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
    fn flaperon_mix_keeps_both_thick_hinges_and_common_trailing_points() {
        let mut rig = Rig {
            flaps: BTreeMap::new(),
        };
        let mut faces = Vec::new();
        for (side, ids) in [(-1., [0x5003, 0x5022]), (1., [0x4eb9, 0x4ed8])] {
            for (a, inner, outer) in [(ids[0], 1., 4.), (ids[1], -2., 3.)] {
                let neutral = vec![
                    [side * 48., -19., inner],
                    [side * 104., -45., outer],
                    [side * 101., -48., 4.],
                    [side * 45., -22., -1.],
                ];
                let down = vec![
                    [side * 48., -19., inner],
                    [side * 104., -45., outer],
                    [side * 102., -49., 1.],
                    [side * 45., -23., -3.],
                ];
                faces.push(panel(a, neutral.clone()));
                rig.flaps.insert(
                    a,
                    Morph {
                        neutral,
                        down,
                        closure: false,
                    },
                );
            }
        }
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                let mut s = state();
                s.flaps = flap;
                s.aileron = roll;
                let mut tips = BTreeMap::new();
                for f in &faces {
                    let q = rig.animate(f, &s).unwrap();
                    assert_eq!(q.positions[..2], f.positions[..2]);
                    assert_eq!(q.uv, f.uv);
                    let side = if f.positions[0][0] < 0. { -1 } else { 1 };
                    if let Some(previous) = tips.insert(side, q.positions[2..].to_vec()) {
                        assert_eq!(previous, q.positions[2..]);
                    }
                    if roll == 0. && flap == 1. {
                        assert_eq!(q.positions, rig.flaps[&f.address].down);
                    }
                    let mut baseline = s.clone();
                    baseline.aileron = 0.;
                    let b = rig.animate(f, &baseline).unwrap();
                    if roll != 0. {
                        assert!(
                            (q.positions[3][2] - b.positions[3][2]) * roll as f32 * side as f32
                                > 0.
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn source_closures_are_hidden_only_at_rest_and_share_neutral_mapping() {
        let neutral = vec![[48., -19., 1.], [45., -22., -1.], [48., -19., -2.]];
        let source = panel(0x4fae, neutral.clone());
        let rig = Rig {
            flaps: [(
                source.address,
                Morph {
                    neutral,
                    down: vec![[48., -19., 1.], [45., -23., -3.], [48., -19., -2.]],
                    closure: true,
                },
            )]
            .into(),
        };
        let mut s = state();
        s.flaps = 0.;
        s.aileron = 0.;
        assert!(rig.animate(&source, &s).is_none());
        s.aileron = 1.;
        let moved = rig.animate(&source, &s).unwrap();
        assert_eq!(moved.positions[0], source.positions[0]);
        assert_eq!(moved.positions[2], source.positions[2]);
        assert_ne!(moved.positions[1], source.positions[1]);
    }
    #[test]
    fn full_crossed_gear_assemblies_are_rigid_at_201_positions() {
        let definitions = [
            (
                0x4265,
                vec![
                    [7., -18., -3.],
                    [7., -34., -3.],
                    [7., -34., -18.],
                    [7., -18., -18.],
                ],
            ),
            (
                0x42b3,
                vec![
                    [3., -26., -18.],
                    [9., -26., -18.],
                    [9., -26., -1.],
                    [3., -26., -5.],
                ],
            ),
            (
                0x4336,
                vec![
                    [-5., -18., -18.],
                    [-5., -18., -3.],
                    [-5., -34., -3.],
                    [-5., -34., -18.],
                ],
            ),
            (
                0x4384,
                vec![
                    [-2., -26., -18.],
                    [-2., -26., -5.],
                    [-8., -26., -2.],
                    [-8., -26., -18.],
                ],
            ),
            (
                0x4407,
                vec![
                    [0., 69., -19.],
                    [0., 69., -7.],
                    [0., 77., -7.],
                    [0., 77., -19.],
                ],
            ),
            (
                0x4455,
                vec![
                    [-2., 74., -19.],
                    [2., 74., -19.],
                    [2., 74., -7.],
                    [-2., 74., -7.],
                ],
            ),
        ];
        let shape = Shape {
            faces: definitions.into_iter().map(|(a, p)| panel(a, p)).collect(),
            lines: Vec::new(),
            state_words: BTreeSet::new(),
        };
        validate_gear(&shape).unwrap();
        for i in 0..=200 {
            let g = f64::from(i) / 200.;
            for f in &shape.faces {
                let mut q = f.clone();
                gear_pose(&mut q, g);
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
    fn neutral_rudder_and_tail_roots_survive_signed_controls() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let rudder = panel(
            0x51db,
            vec![
                [0., -112., 13.],
                [0., -104., 13.],
                [0., -109., 38.],
                [0., -117., 38.],
            ],
        );
        for value in [-1., 0., 1.] {
            let mut s = state();
            s.rudder = value;
            let f = rig.animate(&rudder, &s).unwrap();
            assert_eq!(f.positions[1..3], rudder.positions[1..3]);
            if value != 0. {
                assert!(f.positions[0][0] * value as f32 > 0.);
            } else {
                assert_eq!(f.positions, rudder.positions);
            }
            s.elevator = value;
            for (a, side) in [(0x2f7a, -1.), (0x2fcc, 1.)] {
                let source = panel(
                    a,
                    vec![
                        [side * 2., -113., 9.],
                        [side * 38., -125., 12.],
                        [side * 38., -116.3, 12.],
                        [side * 7., -107., 9.],
                    ],
                );
                let f = rig.animate(&source, &s).unwrap();
                assert_eq!(f.positions[0], source.positions[0]);
                assert_eq!(f.positions[2..], source.positions[2..]);
                if value != 0. {
                    assert!((f.positions[1][2] - 12.) * value as f32 > 0.);
                }
            }
        }
    }
}
