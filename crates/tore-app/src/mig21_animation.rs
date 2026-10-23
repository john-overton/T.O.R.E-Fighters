//! MiG21 own-source fixed attachments, slanted roll hinges and rigid asymmetric gear.
use crate::{
    AppResult,
    additional_animation::turn,
    aircraft_animation::{split_surface, update_normal},
    flight::State,
};
use std::{collections::BTreeSet, f64::consts::FRAC_PI_2};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 2] = [0x4a50, 0x4a56];
const TAIL: [usize; 8] = [
    0x2aae, 0x2ac9, 0x2b51, 0x2b6c, 0x2d5b, 0x2d7e, 0x2e50, 0x2ea1,
];
const FIN: [usize; 3] = [0x1bd3, 0x1e9a, 0x1ebd];
const BELLY: [usize; 3] = [0x2757, 0x276c, 0x2817];
pub struct Rig;
const FLAME: [usize; 8] = [
    0x32fa, 0x3321, 0x3348, 0x336b, 0x338e, 0x33b5, 0x33dc, 0x3403,
];
const GEAR: [usize; 6] = [0x3275, 0x3290, 0x31af, 0x31ca, 0x3212, 0x322d];
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("MIG21.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("MIG21.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed MIG21.SH root{address:x}").into());
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
        return Err(format!("unreviewed MIG21.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn brake_band(f: &Face) -> bool {
    BELLY.contains(&f.address)
        && f.positions
            .iter()
            .all(|p| p[1] >= -8. - 1e-4 && p[1] <= 12. + 1e-4)
}
fn gear_spec(a: usize) -> ([f32; 3], [f32; 3], f64) {
    match a {
        0x3275 | 0x3290 => ([0., 49.5, -7.5], [1., 0., 0.], -FRAC_PI_2),
        0x31af | 0x31ca => ([21., -5.5, -1.], [0., 1., 0.], FRAC_PI_2),
        _ => ([-20., -5.5, -1.], [0., 1., 0.], -FRAC_PI_2),
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 14952
            || shape.faces.len() != 159
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed MIG21.SH layout".into());
        }
        roots(
            &shape,
            &[0x2aae, 0x2ac9],
            &[[6., -37., -2.], [6., -47., -2.]],
        )?;
        roots(
            &shape,
            &[0x2b51, 0x2b6c],
            &[[6., -47., -2.], [5., -54., -2.], [6., -56., -2.]],
        )?;
        roots(
            &shape,
            &[0x2d5b, 0x2e50],
            &[[-6., -37., -2.], [-6., -47., -2.]],
        )?;
        roots(
            &shape,
            &[0x2d7e, 0x2ea1],
            &[[-6., -47., -2.], [-5., -54., -2.], [-5., -56., -2.]],
        )?;
        roots(&shape, &[0x2b1b], &[[7., -18., -2.], [25., -18., -2.]])?;
        roots(&shape, &[0x2e6b], &[[-7., -18., -2.], [-25., -18., -2.]])?;
        roots(&shape, &[0x2b36], &[[25., -15., -2.], [38., -19., -2.]])?;
        roots(&shape, &[0x2e86], &[[-25., -15., -2.], [-38., -19., -2.]])?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for (word, ids) in [(0x4a50, &FLAME[..]), (0x4a56, &GEAR[..])] {
            let pose = branch(bytes, &original, word, 1, ids, &[])?;
            shape
                .faces
                .extend(pose.faces.into_iter().filter(|f| ids.contains(&f.address)));
        }
        roots(
            &shape,
            &[0x31af, 0x31ca],
            &[[21., 0., -1.], [21., -11., -1.]],
        )?;
        roots(
            &shape,
            &[0x3212, 0x322d],
            &[[-20., 0., -1.], [-20., -11., -1.]],
        )?;
        roots(&shape, &[0x3275, 0x3290], &[[0., 53., -7.], [0., 46., -8.]])?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if FIN.contains(&f.address) {
                faces.extend(split_surface(&f, [0., -52., 0.], [0., 0., 1.], 0., |p| {
                    p[1] + 52.
                }));
            } else if BELLY.contains(&f.address) {
                for piece in split_surface(&f, [0.; 3], [1., 0., 0.], 0., |p| p[1] + 8.) {
                    if piece.positions.iter().all(|p| p[1] <= -8. + 1e-4) {
                        faces.push(piece);
                    } else {
                        faces.extend(split_surface(&piece, [0.; 3], [1., 0., 0.], 0., |p| {
                            p[1] - 12.
                        }));
                    }
                }
            } else {
                faces.push(f);
            }
        }
        shape.faces = faces;
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if TAIL.contains(&a) {
            let left = source.positions.iter().map(|p| p[0]).sum::<f32>() < 0.;
            let mut moved = source.clone();
            turn(
                &mut moved,
                [if left { -6. } else { 6. }, -49.5, -2.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() > 6. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if FIN.contains(&a) {
            if source.positions.iter().all(|p| p[1] <= -52. + 1e-4) {
                turn(
                    &mut result,
                    [0., -52., 0.],
                    [0., 0., 1.],
                    0.35 * state.rudder.clamp(-1., 1.),
                );
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if (old[1] + 52.).abs() < 1e-4 {
                        *p = *old;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if [0x2b1b, 0x2e6b].contains(&a) {
            turn(
                &mut result,
                [if a == 0x2b1b { 7. } else { -7. }, -18., -2.],
                [1., 0., 0.],
                0.40 * state.flaps.clamp(0., 1.),
            );
        } else if [0x2b36, 0x2e86].contains(&a) {
            let left = a == 0x2e86;
            turn(
                &mut result,
                [if left { -25. } else { 25. }, -15., -2.],
                [if left { -13. } else { 13. }, -4., 0.],
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if [
                    [-25., -15., -2.],
                    [-38., -19., -2.],
                    [25., -15., -2.],
                    [38., -19., -2.],
                ]
                .contains(old)
                {
                    *p = *old;
                }
            }
            update_normal(source, &mut result);
        } else if brake_band(source) {
            turn(
                &mut result,
                [0., 12., -8.],
                [1., 0., 0.],
                0.60 * state.brake.clamp(0., 1.),
            );
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if (old[1] - 12.).abs() < 1e-4 {
                    *p = *old;
                }
            }
            update_normal(source, &mut result);
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            let (pivot, axis, angle) = gear_spec(a);
            if [0x3275, 0x3290].contains(&a) {
                turn(&mut result, pivot, [0., 0., 1.], -state.nosewheel_angle());
            }
            turn(
                &mut result,
                pivot,
                axis,
                angle * (1. - state.gear.clamp(0., 1.)),
            );
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -56. + (p[1] + 56.) * state.exhaust.clamp(0., 1.) as f32;
            }
            update_normal(source, &mut result);
        }
        Some(result)
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
            uv: vec![[0.1, 0.9]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
            subtype: 0xed,
        }
    }
    fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
    }
    #[test]
    fn asymmetric_main_top_edges_remain_fixed_and_wheels_stay_separated_at_401_poses() {
        for (a, x, bound) in [(0x31af, 21., 4.), (0x3212, -20., 3.)] {
            let source = face(
                a,
                vec![[x, 0., -18.], [x, -11., -18.], [x, -11., -1.], [x, 0., -1.]],
            );
            let (pivot, axis, angle) = gear_spec(a);
            for step in 0..=400 {
                let mut out = source.clone();
                turn(&mut out, pivot, axis, angle * (1. - f64::from(step) / 400.));
                assert_eq!(out.positions[2], source.positions[2]);
                assert_eq!(out.positions[3], source.positions[3]);
                assert!(
                    out.positions
                        .iter()
                        .all(|p| p[0] * x.signum() >= bound - 1e-4)
                );
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
    fn own_nose_virtual_top_center_is_fixed_and_all_six_gear_faces_hide() {
        let source = face(
            0x3275,
            vec![
                [0., 53., -18.],
                [0., 53., -7.],
                [0., 46., -8.],
                [0., 46., -18.],
            ],
        );
        let (pivot, axis, angle) = gear_spec(source.address);
        for step in 0..=20 {
            let mut out = source.clone();
            turn(&mut out, pivot, axis, angle * (1. - f64::from(step) / 20.));
            let center: [f32; 3] =
                std::array::from_fn(|i| (out.positions[1][i] + out.positions[2][i]) * 0.5);
            assert!(d2(center, pivot) < 1e-8);
        }
        let mut s = state();
        s.gear = 0.;
        for a in GEAR {
            assert!(Rig.animate(&face(a, vec![[0.; 3]; 3]), &s).is_none());
        }
    }
    #[test]
    fn own_tail_inner_points_stay_fixed_with_signed_distal_pitch() {
        let mut s = state();
        for (a, x) in [(0x2aae, 21f32), (0x2d5b, -20f32)] {
            let source = face(
                a,
                vec![
                    [x.signum() * 6., -37., -2.],
                    [x.signum() * 6., -47., -2.],
                    [x, -62., -2.],
                    [x, -54., -2.],
                ],
            );
            for v in [-1., 0., 1.] {
                s.elevator = v;
                let out = Rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[0], source.positions[0]);
                assert_eq!(out.positions[1], source.positions[1]);
                if v != 0. {
                    for i in 2..4 {
                        assert!((out.positions[i][2] + 2.) * v as f32 > 0.);
                    }
                }
            }
        }
    }
    #[test]
    fn slanted_outer_roll_and_flap_hinges_stay_fixed_with_documented_sign() {
        let mut s = state();
        for (a, side) in [(0x2b36, 1.), (0x2e86, -1.)] {
            let source = face(
                a,
                vec![
                    [side * 38., -19., -2.],
                    [side * 25., -15., -2.],
                    [side * 25., -23., -2.],
                    [side * 38., -23., -2.],
                ],
            );
            for v in [-1., 0., 1.] {
                s.aileron = v;
                let out = Rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[0], source.positions[0]);
                assert_eq!(out.positions[1], source.positions[1]);
                if v != 0. {
                    assert!((out.positions[2][2] + 2.) * v as f32 * side > 0.);
                }
            }
        }
        s.aileron = 0.;
        let source = face(
            0x2b1b,
            vec![
                [7., -18., -2.],
                [25., -18., -2.],
                [25., -23., -2.],
                [7., -23., -2.],
            ],
        );
        s.flaps = 1.;
        let out = Rig.animate(&source, &s).unwrap();
        assert_eq!(out.positions[0], source.positions[0]);
        assert_eq!(out.positions[1], source.positions[1]);
        assert!(out.positions[2][2] < -2.);
    }
    #[test]
    fn thick_fitted_brake_front_is_fixed_and_outer_belly_pieces_stay_static() {
        let mut s = state();
        s.brake = 1.;
        let source = face(
            0x2757,
            vec![
                [2., 12., -8.],
                [2., -8., -8.],
                [5., -8., -6.],
                [5., 12., -6.],
            ],
        );
        let out = Rig.animate(&source, &s).unwrap();
        assert_eq!(out.positions[0], source.positions[0]);
        assert_eq!(out.positions[3], source.positions[3]);
        assert_ne!(out.positions[2], source.positions[2]);
        let outside = face(
            0x2757,
            vec![
                [2., 26., -8.],
                [2., 12., -8.],
                [5., 12., -6.],
                [5., 26., -6.],
            ],
        );
        assert_eq!(
            Rig.animate(&outside, &s).unwrap().positions,
            outside.positions
        );
    }
}
