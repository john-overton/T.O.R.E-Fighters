//! MiG23 own source rudder/flap endpoints and explicit fitted sweep/tail controls.
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
const WORDS: [usize; 5] = [0x6ae0, 0x6ae6, 0x6af2, 0x6af8, 0x6afe];
const TAIL: [usize; 4] = [0x3543, 0x356b, 0x3730, 0x3758];
const RUDDER: [usize; 2] = [0x42f9, 0x4318];
const BRAKE_L: [usize; 3] = [0x3109, 0x313c, 0x31a7];
const BRAKE_R: [usize; 3] = [0x339f, 0x33f8, 0x347a];
const MAIN_R: [usize; 6] = [0x44eb, 0x450e, 0x4531, 0x4550, 0x456f, 0x458e];
const MAIN_L: [usize; 6] = [0x4610, 0x4633, 0x4656, 0x4675, 0x4694, 0x46b3];
const NOSE: [usize; 4] = [0x47c2, 0x47e9, 0x4810, 0x4837];
const BRACE: [usize; 2] = [0x474f, 0x4766];
const FLAME: [usize; 6] = [0x4386, 0x43b9, 0x43ec, 0x4413, 0x443a, 0x4461];
const GEAR: [usize; 18] = [
    0x474f, 0x4766, 0x4610, 0x4633, 0x4656, 0x4675, 0x4694, 0x46b3, 0x47c2, 0x47e9, 0x4810, 0x4837,
    0x44eb, 0x450e, 0x4531, 0x4550, 0x456f, 0x458e,
];
const WING: [usize; 25] = [
    0x3c9f, 0x3cc6, 0x3ce7, 0x3d08, 0x3e7e, 0x3ea5, 0x3d86, 0x3dad, 0x3dce, 0x3def, 0x3e1c, 0x3e3d,
    0x3f87, 0x3fae, 0x3fcf, 0x3ffc, 0x401d, 0x403f, 0x4066, 0x4087, 0x40a8, 0x40cf, 0x40e4, 0x4175,
    0x4194,
];
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
            return Err("unreviewed MIG23.SH source material correspondence".into());
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
                return Err("unreviewed MIG23.SH source point correspondence".into());
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
    fn apply(&self, result: &mut Face, t: f64, sign_uv: bool) {
        let active = t > 0.;
        let t = t.clamp(0., 1.) as f32;
        result.positions = self
            .neutral
            .iter()
            .zip(&self.deployed)
            .map(|(p, q)| std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t))
            .collect();
        result.uv = if sign_uv {
            if !active {
                self.neutral_uv.clone()
            } else {
                self.deployed_uv.clone()
            }
        } else {
            self.neutral_uv
                .iter()
                .zip(&self.deployed_uv)
                .map(|(p, q)| std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t))
                .collect()
        };
    }
}
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("MIG23.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("MIG23.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed MIG23.SH root{address:x}").into());
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
        return Err(format!("unreviewed MIG23.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
    rudder: BTreeMap<usize, [Morph; 2]>,
}
fn flap_target(p: [f32; 3]) -> [f32; 3] {
    if p[1] == -6. {
        [p[0], p[1], if p[0].abs() == 7. { 1. } else { 2. }]
    } else {
        p
    }
}
fn rudder_target(p: [f32; 3], sign: f32) -> [f32; 3] {
    if p[1] == -29. {
        [sign * if p[2] == 5. { 3. } else { 1. }, -28., p[2]]
    } else {
        p
    }
}
fn brake_band(f: &Face) -> bool {
    (BRAKE_L.contains(&f.address) || BRAKE_R.contains(&f.address))
        && f.positions
            .iter()
            .all(|p| p[1] >= -24. - 1e-4 && p[1] <= -16. + 1e-4)
}
fn sweep(state: &State) -> f64 {
    ((state.speed / 1.68781 - 400.) / 300.).clamp(0., 1.)
        * 40f64.to_radians()
        * (1. - state.flaps.clamp(0., 1.))
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 23312
            || shape.faces.len() != 219
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed MIG23.SH layout".into());
        }
        roots(&shape, &RUDDER, &[[0., -24., 5.], [0., -27., 13.]])?;
        roots(&shape, &[0x3cc6, 0x3dad], &[[9., 4., 3.]])?;
        roots(&shape, &[0x3fae, 0x4066], &[[-9., 4., 3.]])?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        let mut rudder = BTreeMap::new();
        for (word, bindings) in [
            (0x6af2, [(0x4175, 0x41da), (0x4194, 0x41f9)]),
            (0x6af8, [(0x3e7e, 0x3ee3), (0x3ea5, 0x3f0a)]),
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
        let positive = branch(bytes, &original, 0x6afe, 1, &[0x423f, 0x425e], &RUDDER)?;
        let negative = branch(bytes, &original, 0x6afe, -1, &[0x429c, 0x42bb], &RUDDER)?;
        for (a, p, n) in [(0x42f9, 0x423f, 0x429c), (0x4318, 0x425e, 0x42bb)] {
            let base = one(&shape, a)?;
            rudder.insert(
                a,
                [
                    Morph::bind(base, one(&positive, p)?, |p| rudder_target(p, 1.))?,
                    Morph::bind(base, one(&negative, n)?, |p| rudder_target(p, -1.))?,
                ],
            );
        }
        for (word, ids) in [(0x6ae0, &FLAME[..]), (0x6ae6, &GEAR[..])] {
            let pose = branch(bytes, &original, word, 1, ids, &[])?;
            shape
                .faces
                .extend(pose.faces.into_iter().filter(|f| ids.contains(&f.address)));
        }
        roots(
            &shape,
            &[0x44eb, 0x450e],
            &[[2., -1., -3.], [5., -1., -3.], [9., -1., -3.]],
        )?;
        roots(
            &shape,
            &[0x4610, 0x4633],
            &[[-2., -1., -3.], [-5., -1., -3.], [-9., -1., -3.]],
        )?;
        roots(&shape, &NOSE[..2], &[[-1., 27., -3.], [1., 27., -3.]])?;
        roots(&shape, &BRACE, &[[0., 29., -3.], [-1., 29., -5.]])?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if BRAKE_L.contains(&f.address) || BRAKE_R.contains(&f.address) {
                for piece in split_surface(&f, [0.; 3], [1., 0., 0.], 0., |p| p[1] + 24.) {
                    if piece.positions.iter().all(|p| p[1] <= -24. + 1e-4) {
                        faces.push(piece);
                    } else {
                        faces.extend(split_surface(&piece, [0.; 3], [1., 0., 0.], 0., |p| {
                            p[1] + 16.
                        }));
                    }
                }
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
            m.apply(&mut result, state.flaps, false);
            update_normal(source, &mut result);
        } else if let Some(pair) = self.rudder.get(&a) {
            pair[usize::from(state.rudder < 0.)].apply(&mut result, state.rudder.abs(), true);
            update_normal(source, &mut result);
        } else if TAIL.contains(&a) {
            let side = if a >= 0x3730 { -1. } else { 1. };
            let mut moved = source.clone();
            turn(
                &mut moved,
                [side * 3.5, -26., 1.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.)
                    - side as f64 * 0.10 * state.aileron.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() > 4. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if brake_band(source) {
            let left = BRAKE_L.contains(&a);
            let (pivot, axis) = if left {
                ([-43. / 11., -16., 2.5 / 11.], [0., 0., 25. / 11.])
            } else {
                ([40.5 / 11., -16., 2.5 / 11.], [-5. / 11., 0., 25. / 11.])
            };
            turn(
                &mut result,
                pivot,
                axis,
                if left {
                    -0.60 * state.brake.clamp(0., 1.)
                } else {
                    0.60 * state.brake.clamp(0., 1.)
                },
            );
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if (old[1] + 16.).abs() < 1e-4 {
                    *p = *old;
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
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -29. + (p[1] + 29.) * state.exhaust.clamp(0., 1.) as f32;
            }
            update_normal(source, &mut result);
        }
        if WING.contains(&a) {
            let side = if source.positions.iter().map(|p| p[0]).sum::<f32>() < 0. {
                -1.
            } else {
                1.
            };
            turn(
                &mut result,
                [side * 9., 4., 3.],
                [0., 0., 1.],
                -side as f64 * sweep(state),
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
            [if MAIN_L.contains(&a) { -3. } else { 3. }, -1., -3.],
            [1., 0., 0.],
            -FRAC_PI_2 * closing,
        );
    } else if BRACE.contains(&a) {
        let old = f.positions.clone();
        turn(f, [0., 27., -3.], [1., 0., 0.], -FRAC_PI_2 * closing);
        for (p, q) in f.positions.iter_mut().zip(old) {
            if q[1] == 29. {
                *p = q;
            }
        }
    } else {
        turn(f, [0., 27., -3.], [0., 0., 1.], -steering);
        turn(f, [0., 27., -3.], [1., 0., 0.], -FRAC_PI_2 * closing);
    }
}
pub(crate) fn flame(a: usize) -> bool {
    FLAME.contains(&a)
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
            uv: vec![[0.; 2]; positions.len()],
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
            rudder: BTreeMap::new(),
        }
    }
    fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
    }
    #[test]
    fn sign_selected_uv_avoids_half_demand_reflection_collapse_and_preserves_zero() {
        let mut base = face(
            0x42f9,
            vec![
                [0., -29., 5.],
                [0., -24., 5.],
                [0., -27., 13.],
                [0., -29., 13.],
            ],
        );
        base.uv = vec![[10., 0.], [10., 1.], [20., 1.], [20., 0.]];
        let mut positive = base.clone();
        positive.positions = base
            .positions
            .iter()
            .copied()
            .map(|p| rudder_target(p, 1.))
            .collect();
        positive.uv = vec![[20., 0.], [20., 1.], [10., 1.], [10., 0.]];
        let mut negative = base.clone();
        negative.positions = base
            .positions
            .iter()
            .copied()
            .map(|p| rudder_target(p, -1.))
            .collect();
        let pair = [
            Morph::bind(&base, &positive, |p| rudder_target(p, 1.)).unwrap(),
            Morph::bind(&base, &negative, |p| rudder_target(p, -1.)).unwrap(),
        ];
        let rig = Rig {
            flaps: BTreeMap::new(),
            rudder: [(base.address, pair)].into(),
        };
        let mut s = state();
        for v in [-1., -0.5, 0., 1e-12, 0.5, 1.] {
            s.rudder = v;
            let out = rig.animate(&base, &s).unwrap();
            assert_eq!(out.positions[1], base.positions[1]);
            assert_eq!(out.positions[2], base.positions[2]);
            assert_eq!(
                out.uv,
                if v > 0. {
                    positive.uv.clone()
                } else {
                    base.uv.clone()
                }
            );
            assert_ne!(out.uv[1][0], out.uv[2][0]);
            if v == 0. {
                assert_eq!(out.positions, base.positions);
            }
            if v.abs() == 1. {
                assert_eq!(
                    out.positions,
                    if v > 0. {
                        positive.positions.clone()
                    } else {
                        negative.positions.clone()
                    }
                );
            }
        }
    }
    #[test]
    fn differential_tail_keeps_inner_roots_and_has_consistent_pitch_roll_sign() {
        let rig = rig();
        let mut s = state();
        for (a, side) in [(0x3543, 1.), (0x3730, -1.)] {
            let source = face(
                a,
                vec![
                    [side * 3., -30., 1.],
                    [side * 10., -33., 1.],
                    [side * 13., -30., 1.],
                    [side * 4., -16., 1.],
                    [side * 3., -26., 1.],
                ],
            );
            for v in [-1., 0., 1.] {
                s.elevator = v;
                s.aileron = 0.;
                let out = rig.animate(&source, &s).unwrap();
                for i in [0, 3, 4] {
                    assert_eq!(out.positions[i], source.positions[i]);
                }
                if v != 0. {
                    assert!((out.positions[1][2] - 1.) * v as f32 > 0.);
                }
                s.elevator = 0.;
                s.aileron = v;
                let out = rig.animate(&source, &s).unwrap();
                if v != 0. {
                    assert!((out.positions[1][2] - 1.) * v as f32 * side > 0.);
                }
            }
        }
    }
    #[test]
    fn actual_wing_pivots_are_fixed_and_sweep_retains_its_documented_speed_flap_limits() {
        let rig = rig();
        let mut s = state();
        for (a, side) in [(0x3cc6, 1.), (0x3fae, -1.)] {
            let source = face(
                a,
                vec![
                    [side * 9., 4., 3.],
                    [side * 10., 2., 3.],
                    [side * 30., -2., 3.],
                ],
            );
            for (knots, angle) in [
                (300., 0.),
                (400., 0.),
                (550., 20.),
                (700., 40.),
                (900., 40.),
            ] {
                s.speed = knots * 1.68781;
                s.flaps = 0.;
                assert!((sweep(&s).to_degrees() - angle).abs() < 1e-8);
                let out = rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[0], source.positions[0]);
                assert!(
                    (d2(out.positions[0], out.positions[2])
                        - d2(source.positions[0], source.positions[2]))
                    .abs()
                        < 1e-3
                );
                s.flaps = 1.;
                assert_eq!(
                    rig.animate(&source, &s).unwrap().positions,
                    source.positions
                );
            }
        }
    }
    #[test]
    fn complete_main_cards_remain_rigid_separate_and_keep_source_top_line_at_401_samples() {
        for (a, side) in [(0x44eb, 1.), (0x4610, -1.)] {
            let source = face(
                a,
                vec![
                    [side * 2., -1., -3.],
                    [side * 2., -1., -10.],
                    [side * 9., -1., -10.],
                    [side * 9., -1., -3.],
                    [side * 5., -1., -3.],
                ],
            );
            for step in 0..=400 {
                let mut out = source.clone();
                gear_positions(&mut out, f64::from(step) / 400., 0.);
                assert!(out.positions.iter().all(|p| p[0] * side >= 2.));
                for i in [0, 3, 4] {
                    assert_eq!(out.positions[i], source.positions[i]);
                }
                for i in 0..5 {
                    for j in i + 1..5 {
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
    fn body_brace_and_beveled_brake_forecut_stay_attached() {
        let rig = rig();
        let mut s = state();
        s.brake = 1.;
        let source = face(
            0x3109,
            vec![
                [-43. / 11., -16., -10. / 11.],
                [-35. / 11., -24., -2. / 11.],
                [-35. / 11., -24., 3. / 11.],
                [-43. / 11., -16., 15. / 11.],
            ],
        );
        let out = rig.animate(&source, &s).unwrap();
        assert_eq!(out.positions[0], source.positions[0]);
        assert_eq!(out.positions[3], source.positions[3]);
        assert!(out.positions[1][0] < source.positions[1][0]);
        let brace = face(
            0x474f,
            vec![
                [0., 29., -3.],
                [-1., 29., -5.],
                [-1., 20., -5.],
                [0., 20., -3.],
            ],
        );
        for step in 0..=400 {
            let mut out = brace.clone();
            gear_positions(&mut out, f64::from(step) / 400., 0.);
            assert_eq!(out.positions[0], brace.positions[0]);
            assert_eq!(out.positions[1], brace.positions[1]);
            assert!((d2(out.positions[2], out.positions[3]) - 5.).abs() < 1e-4);
        }
    }
}
