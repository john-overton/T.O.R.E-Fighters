//! F104 source-owned geometry with fitted T-tail, flaperon and gear travel.
//! Exact neutral/device endpoints are bounded at load. No imported code executes.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::FRAC_PI_2,
};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 7] = [0x7ea0, 0x7ea6, 0x7eac, 0x7eb8, 0x7ebe, 0x7ec4, 0x7eca];
const RUDDER: [usize; 2] = [0x5646, 0x566e];
const TAIL: [usize; 4] = [0x2f9e, 0x2fbb, 0x1f4d, 0x1f6b];
const FLAME: [usize; 4] = [0x605b, 0x6083, 0x60b7, 0x60df];
const BRAKE: [usize; 8] = [
    0x5a05, 0x5a25, 0x5a59, 0x5a79, 0x5aad, 0x5ae1, 0x5b01, 0x5b22,
];
const HOOK: [usize; 2] = [0x6134, 0x6154];
const GEAR: [usize; 24] = [
    0x5bc6, 0x5bf2, 0x5c1e, 0x5c3e, 0x5c5f, 0x5c80, 0x5cac, 0x5ccc, 0x5cfc, 0x5d1c, 0x5dc0, 0x5dec,
    0x5e18, 0x5e38, 0x5e59, 0x5e7a, 0x5eaa, 0x5eca, 0x5ef6, 0x5f16, 0x5f8a, 0x5faa, 0x5fd6, 0x5ff6,
];
const RIGHT_WHEEL: [usize; 2] = [0x5bf2, 0x5c5f];
const LEFT_WHEEL: [usize; 2] = [0x5dec, 0x5e59];
const NOSE: [usize; 4] = [0x5f8a, 0x5faa, 0x5fd6, 0x5ff6];
const NOSE_CUT: f32 = -9.;
struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Vec<[f32; 3]>,
    closure: bool,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut matches = shape.faces.iter().filter(|f| f.address == address);
    let result = matches
        .next()
        .ok_or_else(|| format!("F104.SH missing reviewed face{address:x}"))?;
    if matches.next().is_some() {
        return Err(format!("F104.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        let face = one(shape, address)?;
        if !points.iter().all(|p| face.positions.contains(p)) {
            return Err(format!("unreviewed F104.SH attachment{address:x}").into());
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
    let shape = Shape::with_state(bytes, &[(word, value)].into())?;
    let active: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed F104.SH branch{word:x}={value}").into());
    }
    Ok(shape)
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 28374
            || shape.faces.len() != 297
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F104.SH layout".into());
        }
        roots(&shape, &RUDDER, &[[0., -53., 9.], [0., -55., 21.]])?;
        roots(&shape, &TAIL, &[[0., -49., 21.], [0., -73., 21.]])?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for (value, added) in [(1, [0x552d, 0x5556]), (-1, [0x55c1, 0x55ea])] {
            let pose = branch(bytes, &original, 0x7eca, value, &added, &RUDDER)?;
            roots(&pose, &added, &[[0., -53., 9.], [0., -55., 21.]])?;
        }
        let left = branch(
            bytes,
            &original,
            0x7ebe,
            -1,
            &[0x58f7, 0x5920, 0x5937],
            &[0x5850, 0x5879],
        )?;
        let right = branch(
            bytes,
            &original,
            0x7ec4,
            -1,
            &[0x57a1, 0x57ca],
            &[0x56f2, 0x571b],
        )?;
        branch(bytes, &original, 0x7ebe, 1, &[], &[0x5850, 0x5879])?;
        branch(bytes, &original, 0x7ec4, 1, &[], &[0x56f2, 0x571b])?;
        let mut flaps = BTreeMap::new();
        for (address, target, order, pose) in [
            (0x5850, 0x58f7, [1, 2, 3, 0], &left),
            (0x5879, 0x5937, [2, 3, 0, 1], &left),
            (0x56f2, 0x57ca, [1, 2, 3, 0], &right),
            (0x571b, 0x57a1, [1, 2, 3, 0], &right),
        ] {
            let base = one(&shape, address)?;
            let down = one(pose, target)?;
            if base.positions.len() != 4 || down.positions.len() != 4 {
                return Err("unreviewed F104.SH flap topology".into());
            }
            let deployed: Vec<_> = order.into_iter().map(|i| down.positions[i]).collect();
            let side = if address >= 0x5800 { -1. } else { 1. };
            for (a, b) in base.positions.iter().zip(&deployed) {
                if a[1] == -15. {
                    if a != b {
                        return Err(format!(
                            "unreviewed F104.SH flap forward edge{address:x}: {a:?}->{b:?}"
                        )
                        .into());
                    }
                } else if ![[side * 8., -23., -1.], [side * 30., -19., -4.]].contains(a)
                    || *b != [a[0], a[1], -6.]
                {
                    return Err(
                        format!("unreviewed F104.SH flap trailing endpoint{address:x}").into(),
                    );
                }
            }
            flaps.insert(
                address,
                Morph {
                    neutral: base.positions.clone(),
                    deployed,
                    closure: false,
                },
            );
        }
        let closure = one(&left, 0x5920)?.clone();
        if closure.positions != [[-30., -19., -6.], [-30., -15., -4.], [-30., -15., -3.]] {
            return Err("unreviewed F104.SH original left flap closure".into());
        }
        flaps.insert(
            closure.address,
            Morph {
                neutral: vec![[-30., -19., -4.], [-30., -15., -4.], [-30., -15., -3.]],
                deployed: closure.positions.clone(),
                closure: true,
            },
        );
        shape.faces.push(closure);
        let gear = branch(bytes, &original, 0x7eac, 1, &GEAR, &[])?;
        roots(
            &gear,
            &[0x5c1e, 0x5c3e],
            &[[3., -21., -5.], [5., -21., -5.]],
        )?;
        roots(
            &gear,
            &[0x5e18, 0x5e38],
            &[[-3., -21., -5.], [-5., -21., -5.]],
        )?;
        roots(
            &gear,
            &[0x5cac, 0x5ccc],
            &[[8., -21., -3.], [14., -21., -3.], [5., -21., -5.]],
        )?;
        roots(
            &gear,
            &[0x5ef6, 0x5f16],
            &[[-8., -21., -3.], [-14., -21., -3.], [-5., -21., -5.]],
        )?;
        roots(
            &gear,
            &[0x5cfc, 0x5d1c],
            &[[3., -22., -5.], [3., -20., -5.]],
        )?;
        roots(
            &gear,
            &[0x5eaa, 0x5eca],
            &[[-3., -22., -5.], [-3., -20., -5.]],
        )?;
        roots(&gear, &[0x5f8a, 0x5faa], &[[0., 35., -6.], [0., 40., -6.]])?;
        roots(&gear, &[0x5fd6, 0x5ff6], &[[-2., 38., -6.], [2., 38., -6.]])?;
        let hook = branch(bytes, &original, 0x7eb8, 1, &HOOK, &[])?;
        roots(&hook, &HOOK, &[[0., -32., -6.], [0., -35., -6.]])?;
        let brake = branch(bytes, &original, 0x7ea6, 1, &BRAKE, &[])?;
        for &address in &BRAKE {
            if one(&brake, address)?
                .positions
                .iter()
                .filter(|p| p[1] == 24.)
                .count()
                != 2
            {
                return Err("unreviewed F104.SH brake root edge".into());
            }
        }
        let flame = branch(bytes, &original, 0x7ea0, 1, &FLAME, &[])?;
        if flame
            .faces
            .iter()
            .filter(|f| FLAME.contains(&f.address))
            .flat_map(|f| &f.positions)
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            != -61.
        {
            return Err("unreviewed F104.SH flame root".into());
        }
        for (group, ids) in [
            (&hook, HOOK.as_slice()),
            (&brake, BRAKE.as_slice()),
            (&flame, FLAME.as_slice()),
        ] {
            shape.faces.extend(
                group
                    .faces
                    .iter()
                    .filter(|f| ids.contains(&f.address))
                    .cloned(),
            );
        }
        for source in gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)) {
            if NOSE.contains(&source.address) {
                shape.faces.extend(crate::aircraft_animation::split_surface(
                    &source,
                    [0.; 3],
                    [1., 0., 0.],
                    0.,
                    |p| p[2] - NOSE_CUT,
                ));
            } else {
                shape.faces.push(source);
            }
        }
        validate_gear(&shape)?;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let address = source.address;
        if let Some(morph) = self.flaps.get(&address) {
            if morph.closure && state.flaps <= 0. && state.aileron == 0. {
                return None;
            }
            let amount = state.flaps.clamp(0., 1.) as f32;
            result.positions = morph
                .neutral
                .iter()
                .zip(&morph.deployed)
                .map(|(a, b)| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * amount))
                .collect();
            let side = if morph.neutral.iter().any(|p| p[0] < 0.) {
                -1.
            } else {
                1.
            };
            let mut rolled = result.clone();
            turn(
                &mut rolled,
                [side * 9., -15., -1.5],
                [side * 21., 0., -2.],
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for ((p, q), original) in result
                .positions
                .iter_mut()
                .zip(rolled.positions)
                .zip(&morph.neutral)
            {
                if original[1] < -15. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if RUDDER.contains(&address) {
            turn(
                &mut result,
                [0., -53., 9.],
                [0., -2., 12.],
                0.35 * state.rudder.clamp(-1., 1.),
            );
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if [[0., -53., 9.], [0., -55., 21.]].contains(old) {
                    *p = *old;
                }
            }
            update_normal(source, &mut result);
        } else if TAIL.contains(&address) {
            let mut pitched = source.clone();
            turn(
                &mut pitched,
                [0., -61., 21.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(pitched.positions) {
                if p[0] != 0. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if HOOK.contains(&address) {
            if state.hook <= 0. {
                return None;
            }
            let mut folded = source.clone();
            turn(
                &mut folded,
                [0., -33.5, -6.],
                [1., 0., 0.],
                -1.05 * (1. - state.hook.clamp(0., 1.)),
            );
            for (p, q) in result.positions.iter_mut().zip(folded.positions) {
                if ![[0., -32., -6.], [0., -35., -6.]].contains(p) {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if BRAKE.contains(&address) {
            if state.brake <= 0. {
                return None;
            }
            let mut folded = source.clone();
            turn(
                &mut folded,
                [0., 24., 7.],
                [1., 0., 0.],
                (2f64 / 3.).atan() * (1. - state.brake.clamp(0., 1.)),
            );
            for (p, q) in result.positions.iter_mut().zip(folded.positions) {
                if p[1] != 24. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if GEAR.contains(&address) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear);
            update_normal(source, &mut result);
        } else if FLAME.contains(&address) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -61. + (p[1] + 61.) * state.exhaust.max(0.) as f32;
            }
        }
        Some(result)
    }
}
fn gear_positions(face: &mut Face, gear: f64) {
    let closing = (1. - gear.clamp(0., 1.)) as f32;
    let second = (2. * closing - 1.).clamp(0., 1.);
    let a = face.address;
    if NOSE.contains(&a) {
        let delta = [
            0.,
            -7. * second,
            if closing <= 0.5 {
                8. * closing
            } else {
                4. + 4. * second
            },
        ];
        for p in &mut face.positions {
            let weight = ((-6. - p[2]) / (-6. - NOSE_CUT)).clamp(0., 1.);
            for i in 0..3 {
                p[i] += delta[i] * weight;
            }
        }
        return;
    }
    let side = if a >= 0x5dc0 { -1. } else { 1. };
    let pivot = [side * 13., if side > 0. { -21. } else { -21.5 }, -10.];
    let mut moved = face.clone();
    turn(
        &mut moved,
        pivot,
        [0., 1., 0.],
        f64::from(side * second) * FRAC_PI_2,
    );
    let delta = [
        -side * 1.5 * second,
        0.,
        if closing <= 0.5 {
            4. * closing
        } else {
            2. + 6. * second
        },
    ];
    for p in &mut moved.positions {
        for i in 0..3 {
            p[i] += delta[i];
        }
    }
    let rigid = RIGHT_WHEEL.contains(&a)
        || LEFT_WHEEL.contains(&a)
        || matches!(a, 0x5bc6 | 0x5c80 | 0x5dc0 | 0x5e7a);
    for (p, q) in face.positions.iter_mut().zip(moved.positions) {
        let inner_leg_corner =
            matches!(a, 0x5c1e | 0x5c3e | 0x5e18 | 0x5e38) && p[0].abs() == 3. && p[2] == -13.;
        let shared = *p == [side * 5., -21., -5.];
        let (root, end) = if matches!(a, 0x5cac | 0x5ccc | 0x5ef6 | 0x5f16) {
            (-3., -13.)
        } else if matches!(a, 0x5cfc | 0x5d1c | 0x5eaa | 0x5eca) {
            (-5., -10.)
        } else {
            (-5., -13.)
        };
        let weight = if rigid {
            1.
        } else if shared {
            0.
        } else {
            ((root - p[2]) / (root - end)).clamp(0., 1.)
        };
        *p = std::array::from_fn(|i| p[i] + (q[i] - p[i]) * weight);
        // The independently moving wheel target would pull this inner
        // connector corner through its own fixed root chord midway through
        // folding. Keep it one quarter source unit below that chord.
        if inner_leg_corner {
            p[2] = p[2].min(-5.25);
        }
    }
}
fn gear_roots(a: usize) -> &'static [[f32; 3]] {
    match a {
        0x5c1e | 0x5c3e => &[[3., -21., -5.], [5., -21., -5.]],
        0x5e18 | 0x5e38 => &[[-3., -21., -5.], [-5., -21., -5.]],
        0x5cac | 0x5ccc => &[[8., -21., -3.], [14., -21., -3.], [5., -21., -5.]],
        0x5ef6 | 0x5f16 => &[[-8., -21., -3.], [-14., -21., -3.], [-5., -21., -5.]],
        0x5cfc | 0x5d1c => &[[3., -22., -5.], [3., -20., -5.]],
        0x5eaa | 0x5eca => &[[-3., -22., -5.], [-3., -20., -5.]],
        0x5f8a | 0x5faa => &[[0., 35., -6.], [0., 40., -6.]],
        0x5fd6 | 0x5ff6 => &[[-2., 38., -6.], [2., 38., -6.]],
        _ => &[],
    }
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=20 {
        let gear = f64::from(step) / 20.;
        for source in shape.faces.iter().filter(|f| GEAR.contains(&f.address)) {
            let mut result = source.clone();
            gear_positions(&mut result, gear);
            if result.positions.iter().flatten().any(|v| !v.is_finite()) {
                return Err("nonfinite F104.SH fitted gear".into());
            }
            for (a, b) in source.positions.iter().zip(&result.positions) {
                if gear_roots(source.address).contains(a) && a != b {
                    return Err("unreviewed F104.SH fitted gear root movement".into());
                }
            }
            if !NOSE.contains(&source.address)
                && result
                    .positions
                    .iter()
                    .any(|p| p[0] * if source.address >= 0x5dc0 { -1. } else { 1. } < 1.05)
            {
                return Err("unreviewed F104.SH main lower geometry crossing".into());
            }
            let wheel = RIGHT_WHEEL.contains(&source.address)
                || LEFT_WHEEL.contains(&source.address)
                || (NOSE.contains(&source.address)
                    && source.positions.iter().all(|p| p[2] <= NOSE_CUT + 1e-4));
            if !wheel {
                continue;
            }
            for i in 0..source.positions.len() {
                for j in i + 1..source.positions.len() {
                    if (distance_squared(source.positions[i], source.positions[j])
                        - distance_squared(result.positions[i], result.positions[j]))
                    .abs()
                        > 1e-3
                    {
                        return Err("unreviewed F104.SH wheel dimension change".into());
                    }
                }
            }
            if gear == 0. {
                let (min, max) = if RIGHT_WHEEL.contains(&source.address) {
                    ([8.5, -24., -2.], [14.5, -18., -2.])
                } else if LEFT_WHEEL.contains(&source.address) {
                    ([-14.5, -24., -2.], [-8.5, -19., -2.])
                } else {
                    ([-2., 28., -5.], [2., 33., -1.])
                };
                if result
                    .positions
                    .iter()
                    .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-4 || p[i] > max[i] + 1e-4))
                {
                    return Err("unreviewed F104.SH wheel stow bounds".into());
                }
            }
        }
    }
    Ok(())
}
fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
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
    let length = n.iter().map(|v| v * v).sum::<f64>().sqrt();
    (length > 1e-9).then(|| n.map(|v| (v / length) as f32))
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
pub(crate) fn flame(address: usize) -> bool {
    FLAME.contains(&address)
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
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
        }
    }
    #[test]
    fn signed_t_tail_and_rudder_keep_their_own_fixed_edges() {
        let rig = rig();
        let mut s = state();
        for command in [-1., -0.5, 0., 0.5, 1.] {
            s.elevator = command;
            s.rudder = command;
            for (address, side) in [(0x2f9e, -1.), (0x1f4d, 1.)] {
                let original = face(
                    address,
                    vec![
                        [0., -49., 21.],
                        [0., -73., 21.],
                        [side * 20., -64., 21.],
                        [side * 20., -55., 21.],
                    ],
                );
                let result = rig.animate(&original, &s).unwrap();
                assert_eq!(result.positions[..2], original.positions[..2]);
                if command == 0. {
                    assert_eq!(result.positions, original.positions);
                } else {
                    assert!((result.positions[2][2] - 21.) * command as f32 > 0.);
                    assert!((result.positions[3][2] - 21.) * (command as f32) < 0.);
                }
            }
            let original = face(
                0x5646,
                vec![[0., -53., 9.], [0., -55., 21.], [0., -64., 15.]],
            );
            let result = rig.animate(&original, &s).unwrap();
            assert_eq!(result.positions[..2], original.positions[..2]);
            if command == 0. {
                assert_eq!(result.positions, original.positions);
            } else {
                assert!(result.positions[2][0] * command as f32 > 0.);
            }
        }
    }
    #[test]
    fn authored_shared_flaperons_keep_twins_together_through_combined_commands() {
        let mut rig = rig();
        let mut pairs = Vec::new();
        for (a, b, side) in [(0x5850, 0x5879, -1.), (0x56f2, 0x571b, 1.)] {
            let points = vec![
                [side * 9., -15., -1.5],
                [side * 12., -22., -2.],
                [side * 27., -20., -4.],
            ];
            let mut pair = Vec::new();
            for (id, positions) in [
                (a, points.clone()),
                (b, points.iter().rev().copied().collect()),
            ] {
                let deployed = positions
                    .iter()
                    .map(|p| if p[1] < -15. { [p[0], p[1], -6.] } else { *p })
                    .collect();
                rig.flaps.insert(
                    id,
                    Morph {
                        neutral: positions.clone(),
                        deployed,
                        closure: false,
                    },
                );
                pair.push(face(id, positions));
            }
            pairs.push(pair);
        }
        let mut s = state();
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            s.flaps = flap;
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                s.aileron = roll;
                for pair in &pairs {
                    let result = rig.animate(&pair[0], &s).unwrap();
                    let other = rig.animate(&pair[1], &s).unwrap();
                    assert_eq!(
                        result.positions,
                        other.positions.iter().rev().copied().collect::<Vec<_>>()
                    );
                    assert_eq!(result.positions[0], pair[0].positions[0]);
                    assert_eq!(result.uv, pair[0].uv);
                    if roll == 0. {
                        if flap == 0. {
                            assert_eq!(result.positions, pair[0].positions);
                        }
                        if flap == 1. {
                            assert_eq!(result.positions, rig.flaps[&pair[0].address].deployed);
                        }
                    } else {
                        let mut baseline = s.clone();
                        baseline.aileron = 0.;
                        let base = rig.animate(&pair[0], &baseline).unwrap();
                        assert!(
                            (result.positions[1][2] - base.positions[1][2])
                                * roll as f32
                                * pair[0].positions[1][0].signum()
                                > 0.
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn hook_and_brake_constraints_preserve_attachment_and_deployed_geometry() {
        let rig = rig();
        let mut s = state();
        let hook = face(
            0x6134,
            vec![[0., -32., -6.], [0., -35., -6.], [0., -42., -12.]],
        );
        let brake = face(
            0x5a59,
            vec![[0., 24., 7.], [3., 24., 6.], [3., 10., 14.], [0., 10., 16.]],
        );
        for fraction in [0., 0.25, 0.5, 0.75, 1.] {
            s.hook = fraction;
            s.brake = fraction;
            if fraction == 0. {
                assert!(rig.animate(&hook, &s).is_none());
                assert!(rig.animate(&brake, &s).is_none());
                continue;
            }
            for source in [&hook, &brake] {
                let result = rig.animate(source, &s).unwrap();
                assert_eq!(result.positions[..2], source.positions[..2]);
                assert_eq!(result.uv, source.uv);
                if fraction == 1. {
                    assert_eq!(result.positions, source.positions);
                }
            }
            if fraction < 1. {
                assert!(rig.animate(&hook, &s).unwrap().positions[2][2] > hook.positions[2][2]);
            }
        }
    }
    #[test]
    fn main_wheels_are_rigid_at_21_samples_and_nose_cut_has_no_connector_gap() {
        for (address, side) in [(0x5bf2, 1.), (0x5dec, -1.)] {
            let source = face(
                address,
                vec![
                    [side * 13., -23., -12.],
                    [side * 13., -20., -12.],
                    [side * 13., -20., -8.],
                    [side * 13., -23., -8.],
                ],
            );
            for step in 0..=20 {
                let mut result = source.clone();
                gear_positions(&mut result, f64::from(step) / 20.);
                assert!(result.positions.iter().all(|p| p[0] * side > 1.05));
                for i in 0..source.positions.len() {
                    for j in i + 1..source.positions.len() {
                        assert!(
                            (distance_squared(source.positions[i], source.positions[j])
                                - distance_squared(result.positions[i], result.positions[j]))
                            .abs()
                                < 1e-3
                        );
                    }
                }
            }
        }
        let upper = face(
            0x5f8a,
            vec![
                [0., 35., -6.],
                [0., 40., -6.],
                [0., 40., -9.],
                [0., 35., -9.],
            ],
        );
        let lower = face(
            0x5f8a,
            vec![
                [0., 35., -9.],
                [0., 40., -9.],
                [0., 40., -12.],
                [0., 35., -12.],
            ],
        );
        for step in 0..=20 {
            let gear = f64::from(step) / 20.;
            let mut a = upper.clone();
            let mut b = lower.clone();
            gear_positions(&mut a, gear);
            gear_positions(&mut b, gear);
            assert_eq!(a.positions[..2], upper.positions[..2]);
            assert_eq!([a.positions[3], a.positions[2]], b.positions[..2]);
        }
    }
    #[test]
    fn inner_main_connector_corner_stays_below_its_fixed_root_chord() {
        for (address, side) in [(0x5c1e, 1.), (0x5e18, -1.)] {
            let source = face(
                address,
                vec![
                    [side * 5., -20., -5.],
                    [side * 12., -20., -13.],
                    [side * 3., -20., -13.],
                    [side * 3., -20., -5.],
                ],
            );
            for step in 0..=400 {
                let mut result = source.clone();
                gear_positions(&mut result, f64::from(step) / 400.);
                assert_eq!(result.positions[0], source.positions[0]);
                assert_eq!(result.positions[3], source.positions[3]);
                assert!(result.positions[2][2] <= -5.25);
            }
        }
    }
}
