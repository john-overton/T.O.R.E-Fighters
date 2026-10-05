//! Reviewed A7.SH surfaces with explicit fitted continuous motion.
//! See docs/spec/variety-animation.md. Imported instructions never execute.
use crate::{AppResult, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};

const WORDS: [usize; 7] = [0x6ab0, 0x6ab6, 0x6abc, 0x6ac8, 0x6ace, 0x6ad4, 0x6ada];
const FLAME: [usize; 4] = [0x50cc, 0x50f3, 0x511a, 0x5141];
const BRAKE: [usize; 10] = [
    0x471d, 0x473c, 0x4753, 0x476a, 0x4789, 0x481f, 0x483e, 0x4855, 0x486c, 0x488b,
];
const GEAR: [usize; 26] = [
    0x4b19, 0x4b30, 0x4bb0, 0x4bcf, 0x4c4f, 0x4c76, 0x4c9d, 0x4cc4, 0x4ceb, 0x4d0a, 0x4d29, 0x4d48,
    0x4dd0, 0x4df7, 0x4e1e, 0x4e45, 0x4e6c, 0x4e8b, 0x4eaa, 0x4ec9, 0x4f49, 0x4f60, 0x4fbc, 0x4fdb,
    0x4ffa, 0x5019,
];
const HOOK: [usize; 2] = [0x5057, 0x5076];
const RUDDER: [usize; 2] = [0x4674, 0x4693];
const TAIL_LEFT: [usize; 2] = [0x2be6, 0x2c59];
const TAIL_RIGHT: [usize; 2] = [0x1f3a, 0x3506];
const ROLL_LEFT: [usize; 2] = [0x4280, 0x42a1];
const ROLL_RIGHT: [usize; 2] = [0x1cf9, 0x1d17];

pub fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}

struct Morph {
    neutral: Vec<[f32; 3]>,
    down: Vec<[f32; 3]>,
    closure: bool,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}

fn face(shape: &Shape, address: usize) -> AppResult<&Face> {
    shape
        .faces
        .iter()
        .find(|f| f.address == address)
        .ok_or_else(|| format!("A7.SH lacks reviewed face {address:x}").into())
}
fn anchors(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        let f = face(shape, address)?;
        if !points.iter().all(|p| f.positions.contains(p)) {
            return Err(format!("unreviewed A7.SH hinge at face {address:x}").into());
        }
    }
    Ok(())
}

impl Rig {
    pub fn load(bytes: &[u8], mut neutral: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 23270
            || neutral.faces.len() != 261
            || neutral.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed A7.SH animation layout".into());
        }
        anchors(&neutral, &RUDDER, &[[0., -65., 38.], [0., -53., 9.]])?;
        anchors(&neutral, &TAIL_LEFT, &[[-6., -55., 0.], [-28., -71., 0.]])?;
        anchors(&neutral, &TAIL_RIGHT, &[[6., -55., 0.], [28., -71., 0.]])?;
        anchors(&neutral, &ROLL_LEFT, &[[-33., -25., 5.], [-57., -31., 3.]])?;
        anchors(&neutral, &ROLL_RIGHT, &[[34., -25., 5.], [57., -31., 3.]])?;
        let original: BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
        for (word, expected) in [
            (0x6ab0, FLAME.as_slice()),
            (0x6ab6, BRAKE.as_slice()),
            (0x6abc, GEAR.as_slice()),
            (0x6ac8, HOOK.as_slice()),
        ] {
            let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
            let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
            if !original.is_subset(&active)
                || active
                    .difference(&original)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != expected.iter().copied().collect()
            {
                return Err(format!("unreviewed A7.SH device branch {word:x}").into());
            }
            neutral.faces.extend(
                pose.faces
                    .into_iter()
                    .filter(|f| expected.contains(&f.address)),
            );
        }
        anchors(&neutral, &HOOK, &[[0., -16., -14.], [0., -18., -11.]])?;
        anchors(
            &neutral,
            &[0x473c, 0x4753],
            &[[7., -20., -4.], [7., -20., 3.]],
        )?;
        anchors(
            &neutral,
            &[0x483e, 0x4855],
            &[[-7., -20., -4.], [-7., -20., 3.]],
        )?;
        anchors(
            &neutral,
            &[0x4d29, 0x4d48],
            &[[6., -3., -11.], [6., -1., -11.]],
        )?;
        anchors(
            &neutral,
            &[0x4eaa, 0x4ec9],
            &[[-6., -3., -11.], [-6., -1., -11.]],
        )?;
        verify_main_gear(&neutral)?;
        let mut flaps = BTreeMap::new();
        for (word, bindings, closure, hinge, trailing) in [
            (
                0x6ace,
                [
                    (0x4a6a, 0x4a03, [1, 2, 3, 0]),
                    (0x4a91, 0x49dc, [2, 3, 0, 1]),
                ],
                0x4a2a,
                [
                    [-7., -7., 6.],
                    [-7., -7., 9.],
                    [-33., -19., 4.],
                    [-33., -19., 6.],
                ],
                [-33., -25., 5.],
            ),
            (
                0x6ad4,
                [
                    (0x4963, 0x48fc, [2, 3, 0, 1]),
                    (0x498a, 0x48d5, [1, 2, 3, 0]),
                ],
                0x4923,
                [
                    [7., -7., 6.],
                    [7., -7., 9.],
                    [34., -19., 4.],
                    [34., -19., 6.],
                ],
                [34., -25., 5.],
            ),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
            let expected: BTreeSet<_> = bindings.iter().map(|b| b.1).chain([closure]).collect();
            let removed: BTreeSet<_> = bindings.iter().map(|b| b.0).collect();
            if active
                .difference(&original)
                .copied()
                .collect::<BTreeSet<_>>()
                != expected
                || original
                    .difference(&active)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != removed
            {
                return Err(format!("unreviewed A7.SH flap branch {word:x}").into());
            }
            for (address, down_address, order) in bindings {
                let base = face(&neutral, address)?;
                let deployed = face(&down, down_address)?;
                if base.positions.len() != 4 || deployed.positions.len() != 4 {
                    return Err("unreviewed A7.SH flap skin topology".into());
                }
                let target: Vec<_> = order.map(|i| deployed.positions[i]).into();
                if base
                    .positions
                    .iter()
                    .zip(&target)
                    .any(|(a, b)| hinge.contains(a) && a != b)
                    || base.positions.iter().filter(|p| hinge.contains(p)).count() != 2
                {
                    return Err("unreviewed A7.SH flap skin seam".into());
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
            let first = &flaps[&bindings[0].0];
            let second = &flaps[&bindings[1].0];
            for (p, target) in first.neutral.iter().zip(&first.down) {
                if let Some(i) = second.neutral.iter().position(|other| other == p)
                    && second.down[i] != *target
                {
                    return Err("unreviewed A7.SH shared flap trailing vertex".into());
                }
            }
            let closing = face(&down, closure)?.clone();
            if closing.positions.len() != 3
                || closing
                    .positions
                    .iter()
                    .filter(|p| hinge.contains(p))
                    .count()
                    != 2
            {
                return Err("unreviewed A7.SH flap end closure".into());
            }
            flaps.insert(
                closure,
                Morph {
                    neutral: closing
                        .positions
                        .iter()
                        .map(|p| if hinge.contains(p) { *p } else { trailing })
                        .collect(),
                    down: closing.positions.clone(),
                    closure: true,
                },
            );
            neutral.faces.push(closing);
        }
        Ok((Self { flaps }, neutral))
    }

    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let address = source.address;
        if let Some(morph) = self.flaps.get(&address) {
            let travel = state.flaps.clamp(0., 1.) as f32;
            if morph.closure && travel == 0. {
                return None;
            }
            result.positions = morph
                .neutral
                .iter()
                .zip(&morph.down)
                .map(|(a, b)| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * travel))
                .collect();
            update_normal(source, &mut result);
        } else if RUDDER.contains(&address) {
            turn(
                &mut result,
                [0., -53., 9.],
                [0., -12., 29.],
                state.rudder.clamp(-1., 1.) * 0.35,
            );
        } else if TAIL_LEFT.contains(&address) || TAIL_RIGHT.contains(&address) {
            let side = if TAIL_LEFT.contains(&address) {
                -1.
            } else {
                1.
            };
            // Both axes point right, so positive pitch moves both sides together.
            turn(
                &mut result,
                [side * 6., -55., 0.],
                [22., -side * 16., 0.],
                -state.elevator.clamp(-1., 1.) * 0.30,
            );
        } else if ROLL_LEFT.contains(&address) || ROLL_RIGHT.contains(&address) {
            let side = if ROLL_LEFT.contains(&address) {
                -1.
            } else {
                1.
            };
            let x = if side < 0. { -33. } else { 34. };
            let mut moving = source.clone();
            turn(
                &mut moving,
                [x, -19., 5.],
                [side * 57. - x, -12., -2.],
                // The source seam axes point outwards on each side. The same
                // signed angle therefore gives opposite trailing-edge travel.
                -state.aileron.clamp(-1., 1.) * 0.20,
            );
            for (p, moved) in result.positions.iter_mut().zip(moving.positions) {
                if *p == [x, -25., 5.] {
                    *p = moved;
                }
            }
            update_normal(source, &mut result);
        } else if BRAKE.contains(&address) {
            let travel = state.brake.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            let side = if address < 0x4800 { 1. } else { -1. };
            let fixed_cavity = matches!(address, 0x471d | 0x481f);
            if !fixed_cavity {
                let mut moving = source.clone();
                turn(
                    &mut moving,
                    [side * 7., -20., 0.],
                    [0., 0., 1.],
                    -side as f64 * (7f64 / 8.).atan() * (1. - travel),
                );
                let brace = matches!(address, 0x476a | 0x4789 | 0x486c | 0x488b);
                for (p, moved) in result.positions.iter_mut().zip(moving.positions) {
                    if !brace || p[0] != side * 7. {
                        *p = moved;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if HOOK.contains(&address) {
            let travel = state.hook.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            let mut moving = source.clone();
            turn(
                &mut moving,
                [0., -17., -12.5],
                [1., 0., 0.],
                (1. - travel) * 1.2,
            );
            for (p, moved) in result.positions.iter_mut().zip(moving.positions) {
                if ![[0., -16., -14.], [0., -18., -11.]].contains(p) {
                    *p = moved;
                }
            }
            update_normal(source, &mut result);
        } else if GEAR.contains(&address) {
            let travel = state.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            if matches!(address, 0x4c4f..=0x4d48 | 0x4dd0..=0x4ec9) {
                main_gear(&mut result, travel);
                update_normal(source, &mut result);
                return Some(result);
            }
            let closing = 1. - travel;
            let (pivot, axis, angle) = match address {
                0x4b19 | 0x4b30 => (
                    [1., -1., -13.],
                    [0., 1., 0.],
                    -std::f64::consts::FRAC_PI_2 * closing,
                ),
                0x4bb0 | 0x4bcf => (
                    [-1., -1., -13.],
                    [0., 1., 0.],
                    std::f64::consts::FRAC_PI_2 * closing,
                ),
                0x4f49 | 0x4f60 => (
                    [-2., 54., -12.],
                    [0., -15., -1.],
                    -std::f64::consts::FRAC_PI_2 * closing,
                ),
                _ => (
                    [0., 46., -12.],
                    [1., 0., 0.],
                    -std::f64::consts::FRAC_PI_2 * closing,
                ),
            };
            turn(&mut result, pivot, axis, angle);
        } else if FLAME.contains(&address) {
            let travel = state.exhaust.max(0.) as f32;
            if travel == 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -66. + (p[1] + 66.) * travel;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}

/// Fitted two-stage linkage, not recovered gear mechanics. First bring each
/// wheel inward while tilting it, then lift it into the body. All upper skin
/// edges stay fixed; the lower linkage deforms and each wheel remains rigid.
/// One point law for all main-gear skins also preserves their shared vertices.
fn main_gear(face: &mut Face, deployed: f64) {
    let inward = (2. * (1. - deployed)).clamp(0., 1.);
    let lift = (1. - 2. * deployed).clamp(0., 1.);
    let (sin, cos) = (0.70 * inward).sin_cos();
    let center_x = 11. - 8. * inward;
    let center_z = -22. + 12. * lift;
    for p in &mut face.positions {
        let weight = ((-13. - p[2] as f64) / 6.).clamp(0., 1.);
        let lower_x = p[0] as f64 / 11. * (center_x + (p[2] as f64 + 22.) * sin);
        let lower_z = center_z + (p[2] as f64 + 22.) * cos;
        p[0] = (p[0] as f64 + (lower_x - p[0] as f64) * weight) as f32;
        p[2] = (p[2] as f64 + (lower_z - p[2] as f64) * weight) as f32;
    }
}

fn verify_main_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=20 {
        let deployed = step as f64 / 20.;
        for source in shape
            .faces
            .iter()
            .filter(|f| matches!(f.address, 0x4c4f..=0x4d48 | 0x4dd0..=0x4ec9))
        {
            let side = if source.address <= 0x4d48 { 1. } else { -1. };
            let mut moved = source.clone();
            main_gear(&mut moved, deployed);
            for (before, after) in source.positions.iter().zip(&moved.positions) {
                if after[0] * side <= 0.
                    || (before[2] >= -13. && before != after)
                    || (deployed == 1. && before != after)
                {
                    return Err("unreviewed A7.SH main gear attachment or separation".into());
                }
            }
            if !matches!(source.address, 0x4ceb | 0x4d0a | 0x4e6c | 0x4e8b) {
                continue;
            }
            if moved.positions.iter().any(|p| p[0] * side < 1.05)
                || (deployed == 0.
                    && moved
                        .positions
                        .iter()
                        .any(|p| p[0].abs() > 5. || !(-13. ..=-7.).contains(&p[2])))
            {
                return Err("unreviewed A7.SH wheel separation or fitted stow envelope".into());
            }
            for i in 0..source.positions.len() {
                for j in i + 1..source.positions.len() {
                    if (distance_squared(source.positions[i], source.positions[j])
                        - distance_squared(moved.positions[i], moved.positions[j]))
                    .abs()
                        > 1e-3
                    {
                        return Err("unreviewed A7.SH wheel dimension change".into());
                    }
                }
            }
        }
    }
    Ok(())
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum()
}

fn turn(face: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    if angle == 0. {
        return;
    }
    let length = axis.iter().map(|v| (*v as f64).powi(2)).sum::<f64>().sqrt();
    let axis = axis.map(|v| v as f64 / length);
    let (sin, cos) = angle.sin_cos();
    let rotate = |v: [f32; 3]| -> [f32; 3] {
        let v = v.map(f64::from);
        let dot = std::iter::zip(axis, v).map(|(a, b)| a * b).sum::<f64>();
        let cross = [
            axis[1] * v[2] - axis[2] * v[1],
            axis[2] * v[0] - axis[0] * v[2],
            axis[0] * v[1] - axis[1] * v[0],
        ];
        std::array::from_fn(|i| (v[i] * cos + cross[i] * sin + axis[i] * dot * (1. - cos)) as f32)
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
        n[0] += (a[1] - b[1]) as f64 * (a[2] + b[2]) as f64;
        n[1] += (a[2] - b[2]) as f64 * (a[0] + b[0]) as f64;
        n[2] += (a[0] - b[0]) as f64 * (a[1] + b[1]) as f64;
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
    let old = [old[0], old[2], old[1]];
    if std::iter::zip(old, reference)
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
    fn synthetic(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let length = positions.len();
        Face {
            address,
            positions,
            colors: vec![20; length],
            fog: Default::default(),
            uv: vec![[0.25, 0.75]; length],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([1., 0., 0.]),
        }
    }
    fn close(a: [f32; 3], b: [f32; 3]) {
        assert!(
            a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5),
            "{a:?} != {b:?}"
        );
    }
    fn material_and_normal(source: &Face, moved: &Face) {
        assert_eq!(source.uv, moved.uv);
        assert_eq!(source.colors, moved.colors);
        assert_eq!(source.texture, moved.texture);
        assert_eq!(source.subtype, moved.subtype);
        let n = moved.normal.unwrap();
        assert!((n.iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 1e-5);
    }
    #[test]
    fn a7_neutral_rudder_twins_share_the_diagonal_hinge_at_every_input() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let hinge = [[0., -53., 9.], [0., -65., 38.]];
        let a = synthetic(RUDDER[0], vec![hinge[0], hinge[1], [0., -74., 32.]]);
        let b = synthetic(RUDDER[1], a.positions.iter().rev().copied().collect());
        let mut s = state();
        for input in [-1., -0.5, 0., 0.5, 1.] {
            s.rudder = input;
            let first = rig.animate(&a, &s).unwrap();
            let second = rig.animate(&b, &s).unwrap();
            close(first.positions[0], hinge[0]);
            close(first.positions[1], hinge[1]);
            for (p, q) in first.positions.iter().zip(second.positions.iter().rev()) {
                close(*p, *q);
            }
            if input == 0. {
                assert_eq!(first.positions, a.positions);
            } else {
                assert_ne!(first.positions[2], a.positions[2]);
            }
            material_and_normal(&a, &first);
            let fixed = synthetic(0x3e09, a.positions.clone());
            assert_eq!(rig.animate(&fixed, &s).unwrap().positions, fixed.positions);
        }
    }
    #[test]
    fn a7_flap_morph_preserves_both_skin_hinges_and_shared_trailing_vertices() {
        // Invented small double-skin panel, not an embedded retail shape.
        let lower = vec![[2., 0., 0.], [8., 0., 0.], [8., -4., 1.], [2., -4., 1.]];
        let upper = vec![[2., 0., 2.], [8., 0., 2.], [8., -4., 1.], [2., -4., 1.]];
        let down_lower = vec![lower[0], lower[1], [8., -3., -3.], [2., -3., -3.]];
        let down_upper = vec![upper[0], upper[1], down_lower[2], down_lower[3]];
        let rig = Rig {
            flaps: [
                (
                    0x4a6a,
                    Morph {
                        neutral: lower.clone(),
                        down: down_lower.clone(),
                        closure: false,
                    },
                ),
                (
                    0x4a91,
                    Morph {
                        neutral: upper.clone(),
                        down: down_upper,
                        closure: false,
                    },
                ),
            ]
            .into(),
        };
        let a = synthetic(0x4a6a, lower);
        let b = synthetic(0x4a91, upper);
        let mut s = state();
        for input in [-1., -0.5, 0., 0.5, 1.] {
            s.flaps = input;
            let first = rig.animate(&a, &s).unwrap();
            let second = rig.animate(&b, &s).unwrap();
            for i in 0..2 {
                assert_eq!(first.positions[i], a.positions[i]);
                assert_eq!(second.positions[i], b.positions[i]);
            }
            for i in 2..4 {
                assert_eq!(first.positions[i], second.positions[i]);
            }
            if input <= 0. {
                assert_eq!(first.positions, a.positions);
            }
            if input == 1. {
                assert_eq!(first.positions, down_lower);
            }
            material_and_normal(&a, &first);
            material_and_normal(&b, &second);
        }
    }
    #[test]
    fn a7_pitch_and_roll_keep_both_hinge_edges_and_twin_trailing_vertices() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let mut s = state();
        for input in [-1., -0.5, 0., 0.5, 1.] {
            s.elevator = input;
            s.aileron = input;
            for (addresses, hinge) in [
                (TAIL_LEFT, [[-6., -55., 0.], [-28., -71., 0.]]),
                (TAIL_RIGHT, [[6., -55., 0.], [28., -71., 0.]]),
            ] {
                let a = synthetic(
                    addresses[0],
                    vec![hinge[0], hinge[1], [hinge[0][0], -63., 0.]],
                );
                let b = synthetic(addresses[1], a.positions.iter().rev().copied().collect());
                let first = rig.animate(&a, &s).unwrap();
                let second = rig.animate(&b, &s).unwrap();
                close(first.positions[0], hinge[0]);
                close(first.positions[1], hinge[1]);
                for (p, q) in first.positions.iter().zip(second.positions.iter().rev()) {
                    close(*p, *q);
                }
                if input != 0. {
                    assert_ne!(first.positions[2], a.positions[2]);
                    assert!(first.positions[2][2] * input as f32 > 0.);
                }
                material_and_normal(&a, &first);
            }
            for (addresses, x, tip) in [(ROLL_LEFT, -33., -57.), (ROLL_RIGHT, 34., 57.)] {
                let a = synthetic(
                    addresses[0],
                    vec![
                        [x, -19., 4.],
                        [tip, -31., 3.],
                        [x, -25., 5.],
                        [x + 1., -23., 7.],
                    ],
                );
                let b = synthetic(
                    addresses[1],
                    vec![
                        [x, -19., 6.],
                        [x, -25., 5.],
                        [tip, -31., 3.],
                        [x + 1., -23., 7.],
                    ],
                );
                let first = rig.animate(&a, &s).unwrap();
                let second = rig.animate(&b, &s).unwrap();
                assert_eq!(&first.positions[..2], &a.positions[..2]);
                assert_eq!(second.positions[0], b.positions[0]);
                assert_eq!(second.positions[2], b.positions[2]);
                assert_eq!(first.positions[2], second.positions[1]);
                if input != 0. {
                    assert_ne!(first.positions[2], a.positions[2]);
                    let side = if x < 0. { -1. } else { 1. };
                    assert!((first.positions[2][2] - 5.) * input as f32 * side > 0.);
                }
                material_and_normal(&a, &first);
                material_and_normal(&b, &second);
            }
        }
    }
    #[test]
    fn a7_devices_keep_roots_and_separate_left_right_gear() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let mut s = state();
        let right = synthetic(
            0x4d29,
            vec![[6., -3., -11.], [6., -1., -11.], [9., -2., -22.]],
        );
        let left = synthetic(
            0x4eaa,
            right
                .positions
                .iter()
                .map(|p| [-p[0], p[1], p[2]])
                .collect(),
        );
        let hook = synthetic(
            0x5057,
            vec![[0., -16., -14.], [0., -18., -11.], [0., -30., -24.]],
        );
        let brake = synthetic(
            0x473c,
            vec![[7., -20., -4.], [7., -20., 3.], [14., -28., 0.]],
        );
        for travel in [0.25, 0.5, 0.75, 1.] {
            s.gear = travel;
            s.hook = travel;
            s.brake = travel;
            let r = rig.animate(&right, &s).unwrap();
            let l = rig.animate(&left, &s).unwrap();
            for i in 0..2 {
                close(r.positions[i], right.positions[i]);
                close(l.positions[i], left.positions[i]);
            }
            for (r, l) in r.positions.iter().zip(&l.positions) {
                close([-r[0], r[1], r[2]], *l);
            }
            for f in [&hook, &brake] {
                let result = rig.animate(f, &s).unwrap();
                for i in 0..2 {
                    close(result.positions[i], f.positions[i]);
                }
                if travel == 1. {
                    assert_eq!(result.positions, f.positions);
                }
                material_and_normal(f, &result);
            }
            if travel < 1. {
                assert!(r.positions[2][0] < right.positions[2][0]);
                assert!(l.positions[2][0] > left.positions[2][0]);
            }
        }
        s.gear = 0.;
        s.brake = 0.;
        s.hook = 0.;
        for f in [&right, &left, &hook, &brake] {
            assert!(rig.animate(f, &s).is_none());
        }
    }

    #[test]
    fn a7_main_gear_linkage_keeps_rigid_wheels_separated_inside_the_fitted_stow_envelope() {
        // Invented wheel extent of two units, not the retail three-unit skin.
        let wheel = synthetic(
            0x4ceb,
            vec![
                [11., -4., -24.],
                [11., -4., -20.],
                [11., 0., -20.],
                [11., 0., -24.],
            ],
        );
        let attachments = vec![
            [6., -3., -11.],
            [6., -1., -11.],
            [2., -2., -13.],
            [3., -2., -12.],
            [11., -2., -11.],
        ];
        for (deployed, center) in [
            (1., [11., -2., -22.]),
            (0.75, [7., -2., -22.]),
            (0.5, [3., -2., -22.]),
            (0.25, [3., -2., -16.]),
            (0., [3., -2., -10.]),
        ] {
            let mut right = wheel.clone();
            let mut left = synthetic(
                0x4e6c,
                wheel
                    .positions
                    .iter()
                    .map(|p| [-p[0], p[1], p[2]])
                    .collect(),
            );
            main_gear(&mut right, deployed);
            main_gear(&mut left, deployed);
            let mean = std::array::from_fn(|i| {
                right.positions.iter().map(|p| p[i]).sum::<f32>() / right.positions.len() as f32
            });
            close(mean, center);
            assert!(right.positions.iter().all(|p| p[0] > 1.));
            assert!(left.positions.iter().all(|p| p[0] < -1.));
            for (a, b) in right.positions.iter().zip(&left.positions) {
                close([-a[0], a[1], a[2]], *b);
            }
            // Wheel edge lengths remain rigid throughout the fitted linkage.
            for i in 0..wheel.positions.len() {
                let j = (i + 1) % wheel.positions.len();
                let distance = |p: &[f32; 3], q: &[f32; 3]| {
                    p.iter().zip(q).map(|(a, b)| (a - b).powi(2)).sum::<f32>()
                };
                assert!(
                    (distance(&wheel.positions[i], &wheel.positions[j])
                        - distance(&right.positions[i], &right.positions[j]))
                    .abs()
                        < 1e-4
                );
            }
            let mut top = synthetic(0x4d29, attachments.clone());
            main_gear(&mut top, deployed);
            assert_eq!(top.positions, attachments);
            if deployed == 0. {
                assert!(
                    right
                        .positions
                        .iter()
                        .all(|p| p[0] < 5. && (-13. ..=-7.).contains(&p[2]))
                );
            }
        }
        // A synthetic cloud samples the whole lower-linkage envelope, including
        // points not present in the retail polygons. The load-time gate checks
        // all interpreted source vertices at the same 21 deployment samples.
        let mut lower = Vec::new();
        for x in [0.5, 3.5, 7.5, 11.] {
            for z in [-25., -21., -17., -13., -11.] {
                lower.push([x, -2., z]);
            }
        }
        let cloud_right = synthetic(0x4c4f, lower.clone());
        let cloud_left = synthetic(0x4dd0, lower.iter().map(|p| [-p[0], p[1], p[2]]).collect());
        let wheel_left = synthetic(
            0x4e6c,
            wheel
                .positions
                .iter()
                .map(|p| [-p[0], p[1], p[2]])
                .collect(),
        );
        verify_main_gear(&Shape {
            faces: vec![wheel, wheel_left, cloud_right, cloud_left],
            lines: vec![],
            state_words: Default::default(),
        })
        .unwrap();
        for step in 0..=20 {
            let inward = (2. * (1. - step as f64 / 20.)).clamp(0., 1.);
            // Analytic bound for the reviewed three-unit retail wheel extent.
            let minimum_x = 11. - 8. * inward - 3. * (0.70 * inward).sin();
            assert!(minimum_x > 1.05);
        }
    }
}
