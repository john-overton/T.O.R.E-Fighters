//! F16C source-owned surfaces with explicitly fitted flaperon and gear travel.
//! Source identities/endpoints are bounded at load. Imported instructions never run.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::{FRAC_PI_2, PI},
};
use tore_formats::shape::{Face, Shape};

const WORDS: [usize; 6] = [0x8ad0, 0x8ad6, 0x8adc, 0x8ae8, 0x8aee, 0x8af4];
const RUDDER: [usize; 2] = [0x6323, 0x634a];
const TAIL_LEFT: [usize; 2] = [0x5606, 0x564b];
const TAIL_RIGHT: [usize; 2] = [0x52cd, 0x52e6];
const FLAME: [usize; 6] = [0x65a6, 0x65d9, 0x660c, 0x6633, 0x665a, 0x6681];
const BRAKE_CLOSED: [usize; 8] = [
    0x64af, 0x64c4, 0x64d9, 0x64ee, 0x6503, 0x6518, 0x652d, 0x6542,
];
const BRAKE_OPEN: [usize; 8] = [
    0x63c0, 0x63d7, 0x63ee, 0x6405, 0x641c, 0x6433, 0x644a, 0x6461,
];
const GEAR: [usize; 25] = [
    0x5989, 0x59a0, 0x59b7, 0x59ce, 0x5a30, 0x5a5d, 0x5a8a, 0x5ab1, 0x5b89, 0x5ba0, 0x5bb7, 0x5bce,
    0x5c30, 0x5c5d, 0x5c8a, 0x5cb1, 0x5d89, 0x5da0, 0x5dbf, 0x5e1f, 0x5e36, 0x5e92, 0x5eb1, 0x5ed0,
    0x5eef,
];
const MAIN_LEFT: [usize; 4] = [0x5a30, 0x5a5d, 0x5a8a, 0x5ab1];
const MAIN_RIGHT: [usize; 4] = [0x5c30, 0x5c5d, 0x5c8a, 0x5cb1];
const NOSE_WHEEL: [usize; 4] = [0x5e92, 0x5eb1, 0x5ed0, 0x5eef];
const WHEEL_CUT: f32 = -14.;

struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Vec<[f32; 3]>,
}
pub fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}

pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
    brakes: BTreeMap<usize, Morph>,
}
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut matches = shape.faces.iter().filter(|f| f.address == address);
    let face = matches
        .next()
        .ok_or_else(|| format!("F16.SH missing reviewed face {address:x}"))?;
    if matches.next().is_some() {
        return Err(format!("F16.SH duplicate reviewed face {address:x}").into());
    }
    Ok(face)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed F16.SH attachment at {address:x}").into());
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
        return Err(format!("unreviewed F16.SH branch {word:x}={value}").into());
    }
    Ok(pose)
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 31488
            || shape.faces.len() != 356
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F16.SH animation layout".into());
        }
        roots(&shape, &RUDDER, &[[0., -40., 11.], [0., -54., 37.]])?;
        roots(&shape, &TAIL_LEFT, &[[-10., -32., 1.], [-10., -59., 1.]])?;
        roots(&shape, &TAIL_RIGHT, &[[10., -32., 1.], [10., -59., 1.]])?;
        let neutral: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for (value, ids) in [(1, [0x6249, 0x6270]), (-1, [0x62b6, 0x62dd])] {
            let pose = branch(bytes, &neutral, 0x8af4, value, &ids, &RUDDER)?;
            roots(&pose, &ids, &[[0., -40., 11.], [0., -54., 37.]])?;
        }
        let mut flaps = BTreeMap::new();
        for (word, bindings) in [
            (
                0x8ae8,
                [
                    (0x60d3, 0x61c1, [2, 3, 0, 1]),
                    (0x60fa, 0x619a, [2, 3, 0, 1]),
                    (0x6121, 0x61e8, [1, 2, 0, 0]),
                    (0x6142, 0x6209, [1, 2, 0, 0]),
                ],
            ),
            (
                0x8aee,
                [
                    (0x5f45, 0x6033, [3, 0, 1, 2]),
                    (0x5f6c, 0x600c, [1, 2, 3, 0]),
                    (0x5f93, 0x605a, [2, 0, 1, 0]),
                    (0x5fb4, 0x607b, [1, 2, 0, 0]),
                ],
            ),
        ] {
            let added: Vec<_> = bindings.iter().map(|b| b.1).collect();
            let removed: Vec<_> = bindings.iter().map(|b| b.0).collect();
            let down = branch(bytes, &neutral, word, -1, &added, &removed)?;
            branch(bytes, &neutral, word, 1, &[], &removed)?;
            for (address, target, order) in bindings {
                roots(&shape, &[address], flap_roots(address))?;
                let base = one(&shape, address)?;
                let target = one(&down, target)?;
                if base.positions.len() != target.positions.len()
                    || !(3..=4).contains(&base.positions.len())
                {
                    return Err("unreviewed F16.SH flap topology".into());
                }
                let deployed: Vec<_> = order[..base.positions.len()]
                    .iter()
                    .map(|&i| target.positions[i])
                    .collect();
                for (index, (a, b)) in base.positions.iter().zip(&deployed).enumerate() {
                    if trailing(*a) {
                        if ![11., 40.].contains(&a[0].abs()) || *b != [a[0], -20., -4.] {
                            return Err(format!("unreviewed F16.SH flap trailing endpoint at {address:x} vertex{index}: {a:?} -> {b:?}").into());
                        }
                    } else if a != b {
                        return Err(format!("unreviewed F16.SH flap fixed forward edge at {address:x} vertex{index}: {a:?} -> {b:?}").into());
                    }
                }
                flaps.insert(
                    address,
                    Morph {
                        neutral: base.positions.clone(),
                        deployed,
                    },
                );
            }
        }
        let gear = branch(bytes, &neutral, 0x8adc, 1, &GEAR, &[])?;
        roots(&gear, &[0x5a30, 0x5a5d], &[[-9., -6., -5.]])?;
        roots(&gear, &[0x5c30, 0x5c5d], &[[9., -6., -5.]])?;
        roots(&gear, &[0x5a8a, 0x5ab1], &[[-6., -9., -8.], [-6., 0., -8.]])?;
        roots(&gear, &[0x5c8a, 0x5cb1], &[[6., -9., -8.], [6., 0., -8.]])?;
        roots(&gear, &[0x5e92, 0x5eb1], &[[0., 30., -8.], [0., 36., -8.]])?;
        roots(&gear, &[0x5ed0, 0x5eef], &[[-2., 33., -8.], [2., 33., -8.]])?;
        roots(&gear, &[0x5e1f, 0x5e36], &[[0., 20., -7.], [0., 21., -7.]])?;
        roots(
            &gear,
            &[0x5989, 0x59a0],
            &[[-8., -10., -4.], [-8., 7., -4.]],
        )?;
        roots(&gear, &[0x5b89, 0x5ba0], &[[9., -11., -4.], [9., 6., -4.]])?;
        let brake = branch(bytes, &neutral, 0x8ad6, 1, &BRAKE_OPEN, &BRAKE_CLOSED)?;
        let mut brakes = BTreeMap::new();
        let mut closed_vertices = BTreeSet::new();
        for address in BRAKE_CLOSED {
            closed_vertices.extend(
                one(&shape, address)?
                    .positions
                    .iter()
                    .map(|p| p.map(f32::to_bits)),
            );
        }
        for address in BRAKE_OPEN {
            let source = one(&brake, address)?;
            if source.positions.len() != 4 {
                return Err("unreviewed F16.SH brake topology".into());
            }
            let neutral: Vec<_> = source
                .positions
                .iter()
                .map(|p| {
                    let mut p = *p;
                    if p[1] == -54. {
                        p[1] = -56.;
                        p[2] = 1.;
                    } else if p[1] == -48. && p[0].abs() == 7. {
                        p[2] = 2.;
                    }
                    p
                })
                .collect();
            if neutral
                .iter()
                .any(|p| !closed_vertices.contains(&p.map(f32::to_bits)))
            {
                return Err("unreviewed F16.SH closed brake footprint".into());
            }
            brakes.insert(
                address,
                Morph {
                    neutral,
                    deployed: source.positions.clone(),
                },
            );
        }
        let flame = branch(bytes, &neutral, 0x8ad0, 1, &FLAME, &[])?;
        for source in flame.faces.iter().filter(|f| FLAME.contains(&f.address)) {
            if source
                .positions
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max)
                > -59.
            {
                return Err("unreviewed F16.SH flame forward root".into());
            }
        }
        shape.faces.extend(
            brake
                .faces
                .into_iter()
                .filter(|f| BRAKE_OPEN.contains(&f.address)),
        );
        shape.faces.extend(
            flame
                .faces
                .into_iter()
                .filter(|f| FLAME.contains(&f.address)),
        );
        for source in gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)) {
            if MAIN_LEFT.contains(&source.address)
                || MAIN_RIGHT.contains(&source.address)
                || NOSE_WHEEL.contains(&source.address)
            {
                shape.faces.extend(crate::aircraft_animation::split_surface(
                    &source,
                    [0.; 3],
                    [1., 0., 0.],
                    0.,
                    |p| p[2] - WHEEL_CUT,
                ));
            } else {
                shape.faces.push(source);
            }
        }
        let rig = Self { flaps, brakes };
        validate_gear(&shape)?;
        Ok((rig, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let address = source.address;
        if let Some(morph) = self.flaps.get(&address) {
            morph_positions(&mut result, morph, state.flaps);
            let side = if source.positions.iter().any(|p| p[0] < 0.) {
                -1.
            } else {
                1.
            };
            let mut rolled = result.clone();
            turn(
                &mut rolled,
                [side * 14., -12., 1.],
                [side * 26., -3., 0.],
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for ((p, moved), neutral) in result
                .positions
                .iter_mut()
                .zip(rolled.positions)
                .zip(&morph.neutral)
            {
                if trailing(*neutral) {
                    *p = moved;
                }
            }
            update_normal(source, &mut result);
        } else if RUDDER.contains(&address) {
            turn(
                &mut result,
                [0., -40., 11.],
                [0., -14., 26.],
                0.35 * state.rudder.clamp(-1., 1.),
            );
            for (p, original) in result.positions.iter_mut().zip(&source.positions) {
                if [[0., -40., 11.], [0., -54., 37.]].contains(original) {
                    *p = *original;
                }
            }
            update_normal(source, &mut result);
        } else if TAIL_LEFT.contains(&address) || TAIL_RIGHT.contains(&address) {
            let side = if TAIL_LEFT.contains(&address) {
                -1.
            } else {
                1.
            };
            let mut moved = source.clone();
            turn(
                &mut moved,
                [side * 10., -45.5, 1.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() != 10. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if BRAKE_CLOSED.contains(&address) {
            if state.brake > 0. {
                return None;
            }
        } else if let Some(morph) = self.brakes.get(&address) {
            if state.brake <= 0. {
                return None;
            }
            morph_positions(&mut result, morph, state.brake);
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
                p[1] = -59. + (p[1] + 59.) * state.exhaust.max(0.) as f32;
            }
        }
        Some(result)
    }
}
fn trailing(p: [f32; 3]) -> bool {
    p[1] == -21. && p[2] == 1.
}
fn flap_roots(address: usize) -> &'static [[f32; 3]] {
    match address {
        0x60d3 => &[[-40., -15., 0.], [-14., -12., 0.]],
        0x60fa => &[[-40., -15., 2.], [-14., -12., 2.]],
        0x6121 => &[[-14., -12., 2.], [-11., -12., 2.]],
        0x6142 => &[[-14., -12., 0.], [-11., -12., 0.]],
        0x5f45 => &[[40., -15., 0.], [14., -12., 0.]],
        0x5f6c => &[[40., -15., 2.], [14., -12., 2.]],
        0x5f93 => &[[14., -12., 2.], [11., -12., 2.]],
        0x5fb4 => &[[14., -12., 0.], [11., -12., 0.]],
        _ => &[],
    }
}
fn morph_positions(result: &mut Face, morph: &Morph, value: f64) {
    let value = value.clamp(0., 1.) as f32;
    result.positions = morph
        .neutral
        .iter()
        .zip(&morph.deployed)
        .map(|(a, b)| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * value))
        .collect();
}
fn gear_positions(face: &mut Face, gear: f64) {
    let u = (1. - gear.clamp(0., 1.)) as f32;
    let second = (2. * u - 1.).clamp(0., 1.);
    let a = face.address;
    let (nose_base, nose_delta, nose_pin_angle) = nose_motion(u, second);
    if matches!(a, 0x5e1f | 0x5e36) {
        // These are the painted brace's actual one-unit distal pin edge.
        // Rotate it as a rigid pair and keep the original upper pins fixed.
        let mut pins = face.clone();
        turn(&mut pins, [0., 32., -13.5], [1., 0., 0.], -nose_pin_angle);
        for (original, moved) in face.positions.iter_mut().zip(pins.positions) {
            if original[1] == 32. {
                *original = std::array::from_fn(|i| moved[i] + nose_base[i]);
            }
        }
        return;
    }
    let main = MAIN_LEFT.contains(&a) || MAIN_RIGHT.contains(&a);
    let nose = NOSE_WHEEL.contains(&a);
    if main || nose {
        let delta = if main {
            [
                if MAIN_LEFT.contains(&a) {
                    2.5 * second
                } else {
                    -2.7 * second
                },
                0.,
                if u <= 0.5 { 20. * u } else { 10. + 8. * second },
            ]
        } else {
            nose_delta
        };
        let root = if matches!(a, 0x5a30 | 0x5a5d | 0x5c30 | 0x5c5d) {
            -5.
        } else {
            -8.
        };
        for p in &mut face.positions {
            let weight = ((root - p[2]) / (root - WHEEL_CUT)).clamp(0., 1.);
            for i in 0..3 {
                p[i] += delta[i] * weight;
            }
        }
    } else if matches!(a, 0x5989 | 0x59a0 | 0x59b7 | 0x59ce) {
        turn(
            face,
            [-8., -1.5, -4.],
            [0., 1., 0.],
            -3. * PI / 4. * f64::from(second),
        );
    } else if matches!(a, 0x5b89 | 0x5ba0 | 0x5bb7 | 0x5bce) {
        turn(
            face,
            [9., -2.5, -4.],
            [0., 1., 0.],
            (PI - (4f64 / 5.).atan()) * f64::from(second),
        );
    } else if matches!(a, 0x5d89 | 0x5da0 | 0x5dbf) {
        turn(
            face,
            [4., 28.5, -8.],
            [0., 1., 0.],
            FRAC_PI_2 * f64::from(second),
        );
    }
}
fn nose_motion(closing: f32, second: f32) -> ([f32; 3], [f32; 3], f64) {
    let base = [
        0.,
        -10. * second,
        if closing <= 0.5 {
            16. * closing
        } else {
            8. + 8. * second
        },
    ];
    let angle = FRAC_PI_2 * f64::from((closing / 0.30).clamp(0., 1.));
    let (sin, cos) = angle.sin_cos();
    // The lower Z-14 brace pin is exactly on the wheel-sheet cut. Its
    // rigid-edge rotation therefore defines the cut's translation too.
    (
        base,
        [
            0.,
            base[1] - 0.5 * sin as f32,
            base[2] + 0.5 * (1. - cos as f32),
        ],
        angle,
    )
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=20 {
        let gear = f64::from(step) / 20.;
        for source in shape.faces.iter().filter(|f| GEAR.contains(&f.address)) {
            let mut moved = source.clone();
            gear_positions(&mut moved, gear);
            if moved.positions.iter().flatten().any(|v| !v.is_finite()) {
                return Err("nonfinite F16.SH fitted gear".into());
            }
            for (original, p) in source.positions.iter().zip(&moved.positions) {
                if gear_roots(source.address).contains(original) && original != p {
                    return Err("unreviewed F16.SH fitted gear root movement".into());
                }
            }
            if source.positions.iter().any(|p| p[2] > WHEEL_CUT + 1e-4) {
                continue;
            }
            let main = MAIN_LEFT.contains(&source.address) || MAIN_RIGHT.contains(&source.address);
            if main {
                let side = if MAIN_LEFT.contains(&source.address) {
                    -1.
                } else {
                    1.
                };
                if moved.positions.iter().any(|p| p[0] * side < 1.05) {
                    return Err("unreviewed F16.SH fitted main wheel separation".into());
                }
            }
            if gear == 0. {
                let (min, max) = if MAIN_LEFT.contains(&source.address) {
                    ([-9.5, -9., -3.], [-1.5, 0., 4.])
                } else if MAIN_RIGHT.contains(&source.address) {
                    ([1.3, -9., -3.], [10.3, 0., 4.])
                } else {
                    ([-2., 19.5, -4.5], [2., 25.5, 2.5])
                };
                if moved
                    .positions
                    .iter()
                    .any(|p| (0..3).any(|i| p[i] < min[i] - 1e-4 || p[i] > max[i] + 1e-4))
                {
                    return Err("unreviewed F16.SH fitted wheel stow bounds".into());
                }
            }
            for i in 0..source.positions.len() {
                for j in i + 1..source.positions.len() {
                    if (distance_squared(source.positions[i], source.positions[j])
                        - distance_squared(moved.positions[i], moved.positions[j]))
                    .abs()
                        > 1e-3
                    {
                        return Err("unreviewed F16.SH fitted wheel dimension change".into());
                    }
                }
            }
        }
    }
    Ok(())
}
fn gear_roots(address: usize) -> &'static [[f32; 3]] {
    match address {
        0x5a30 | 0x5a5d => &[[-9., -6., -5.]],
        0x5c30 | 0x5c5d => &[[9., -6., -5.]],
        0x5a8a | 0x5ab1 => &[[-6., -9., -8.], [-6., 0., -8.]],
        0x5c8a | 0x5cb1 => &[[6., -9., -8.], [6., 0., -8.]],
        0x5e92 | 0x5eb1 => &[[0., 30., -8.], [0., 36., -8.]],
        0x5ed0 | 0x5eef => &[[-2., 33., -8.], [2., 33., -8.]],
        0x5e1f | 0x5e36 => &[[0., 20., -7.], [0., 21., -7.]],
        0x5989 | 0x59a0 => &[[-8., -10., -4.], [-8., 7., -4.]],
        0x5b89 | 0x5ba0 => &[[9., -11., -4.], [9., 6., -4.]],
        0x5d89 => &[[4., 37., -8.], [4., 20., -8.]],
        0x5da0 => &[[4., 30., -8.], [4., 20., -8.]],
        0x5dbf => &[[4., 37., -8.], [4., 30., -8.]],
        _ => &[],
    }
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
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            address,
            colors: vec![17; positions.len()],
            uv: vec![[0.25, 0.75]; positions.len()],
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
            brakes: BTreeMap::new(),
        }
    }
    #[test]
    fn signed_tail_and_rudder_controls_keep_their_own_exact_roots() {
        let rig = rig();
        let mut s = state();
        for control in [-1., -0.5, 0., 0.5, 1.] {
            s.elevator = control;
            s.rudder = control;
            for (address, side) in [(0x5606, -1.), (0x52cd, 1.)] {
                let original = face(
                    address,
                    vec![
                        [side * 10., -32., 1.],
                        [side * 10., -59., 1.],
                        [side * 22., -55., 0.],
                    ],
                );
                let moved = rig.animate(&original, &s).unwrap();
                assert_eq!(moved.positions[..2], original.positions[..2]);
                if control == 0. {
                    assert_eq!(moved.positions, original.positions);
                } else {
                    assert!(
                        (moved.positions[2][2] - original.positions[2][2]) * control as f32 > 0.
                    );
                }
                assert_eq!(moved.uv, original.uv);
            }
            let original = face(
                0x6323,
                vec![[0., -40., 11.], [0., -54., 37.], [0., -51., 17.]],
            );
            let moved = rig.animate(&original, &s).unwrap();
            assert_eq!(moved.positions[..2], original.positions[..2]);
            if control == 0. {
                assert_eq!(moved.positions, original.positions);
            } else {
                assert!(moved.positions[2][0] * control as f32 > 0.);
            }
        }
    }
    #[test]
    fn combined_flap_and_roll_preserve_forward_edges_and_paired_skins() {
        let mut rig = rig();
        let mut faces = Vec::new();
        for (address, opposite, side) in [(0x60d3, 0x60fa, -1.), (0x5f45, 0x5f6c, 1.)] {
            let points = vec![
                [side * 14., -12., 2.],
                [side * 23., -21., 1.],
                [side * 11., -21., 1.],
            ];
            for (id, positions) in [
                (address, points.clone()),
                (opposite, points.iter().rev().copied().collect()),
            ] {
                let deployed = positions
                    .iter()
                    .map(|p| if trailing(*p) { [p[0], -20., -4.] } else { *p })
                    .collect();
                rig.flaps.insert(
                    id,
                    Morph {
                        neutral: positions.clone(),
                        deployed,
                    },
                );
                faces.push(face(id, positions));
            }
        }
        let mut s = state();
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            s.flaps = flap;
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                s.aileron = roll;
                for pair in faces.chunks_exact(2) {
                    let a = rig.animate(&pair[0], &s).unwrap();
                    let b = rig.animate(&pair[1], &s).unwrap();
                    assert_eq!(
                        a.positions,
                        b.positions.iter().rev().copied().collect::<Vec<_>>()
                    );
                    assert_eq!(a.positions[0], pair[0].positions[0]);
                    assert_eq!(a.uv, pair[0].uv);
                    assert!(a.positions.iter().flatten().all(|v| v.is_finite()));
                    if roll == 0. {
                        if flap == 0. {
                            assert_eq!(a.positions, pair[0].positions);
                        }
                        if flap == 1. {
                            assert_eq!(a.positions, rig.flaps[&pair[0].address].deployed);
                        }
                    } else {
                        let mut base = s.clone();
                        base.aileron = 0.;
                        let baseline = rig.animate(&pair[0], &base).unwrap();
                        let sign = pair[0].positions[1][0].signum();
                        assert!(
                            (a.positions[1][2] - baseline.positions[1][2]) * roll as f32 * sign
                                > 0.
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn rigid_lower_wheels_and_upper_connectors_share_the_cut_and_stay_separated() {
        for (address, side) in [(0x5a30, -1.), (0x5c30, 1.)] {
            let original = face(
                address,
                vec![
                    [side * 7., -4., -14.],
                    [side * 10., -4., -14.],
                    [side * 10., -4., -19.],
                    [side * 7., -4., -19.],
                ],
            );
            let root = [side * 9., -6., -5.];
            let connector = face(
                address,
                vec![root, original.positions[0], original.positions[1]],
            );
            for step in 0..=20 {
                let gear = f64::from(step) / 20.;
                let mut wheel = original.clone();
                gear_positions(&mut wheel, gear);
                let mut upper = connector.clone();
                gear_positions(&mut upper, gear);
                assert_eq!(upper.positions[0], root);
                assert_eq!(upper.positions[1..], wheel.positions[..2]);
                assert!(wheel.positions.iter().all(|p| p[0] * side >= 1.05));
                for i in 0..wheel.positions.len() {
                    for j in i + 1..wheel.positions.len() {
                        assert!(
                            (distance_squared(wheel.positions[i], wheel.positions[j])
                                - distance_squared(original.positions[i], original.positions[j]))
                            .abs()
                                < 1e-3
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn brake_closed_ownership_and_geometry_endpoints_do_not_overlay() {
        let mut rig = rig();
        let open = face(
            0x63c0,
            vec![
                [-7., -54., 8.],
                [-10., -54., 8.],
                [-11., -48., 1.],
                [-7., -48., 1.],
            ],
        );
        let neutral = vec![
            [-7., -56., 1.],
            [-10., -56., 1.],
            [-11., -48., 1.],
            [-7., -48., 2.],
        ];
        rig.brakes.insert(
            open.address,
            Morph {
                neutral,
                deployed: open.positions.clone(),
            },
        );
        let closed = face(0x64af, vec![[0.; 3]; 3]);
        let mut s = state();
        for travel in [0., 0.25, 0.5, 0.75, 1.] {
            s.brake = travel;
            assert_eq!(rig.animate(&closed, &s).is_some(), travel == 0.);
            assert_eq!(rig.animate(&open, &s).is_some(), travel > 0.);
            if travel > 0. {
                let result = rig.animate(&open, &s).unwrap();
                assert_eq!(result.positions[2], open.positions[2]);
                if travel == 1. {
                    assert_eq!(result.positions, open.positions);
                }
            }
        }
    }
    #[test]
    fn painted_nose_brace_keeps_rigid_distal_pin_width_and_wheel_cut_attachment() {
        let brace = face(
            0x5e1f,
            vec![
                [0., 19., -7.],
                [0., 20., -7.],
                [0., 32., -13.],
                [0., 32., -14.],
            ],
        );
        let cut = face(
            0x5e92,
            vec![[0., 32., -14.], [0., 33., -17.], [1., 33., -17.]],
        );
        for step in 0..=400 {
            let gear = f64::from(step) / 400.;
            let mut moved = brace.clone();
            gear_positions(&mut moved, gear);
            let mut wheel = cut.clone();
            gear_positions(&mut wheel, gear);
            assert_eq!(moved.positions[..2], brace.positions[..2]);
            assert!((distance_squared(moved.positions[2], moved.positions[3]) - 1.).abs() < 1e-4);
            assert!(distance_squared(moved.positions[3], wheel.positions[0]) < 1e-8);
        }
    }
}
