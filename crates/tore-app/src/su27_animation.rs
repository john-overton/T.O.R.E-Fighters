//! Su27 own source flaperon endpoints, anchored controls and rigid separated wheels.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::{collections::BTreeSet, f64::consts::FRAC_PI_2};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 7] = [0x41f0, 0x41f6, 0x41fc, 0x4208, 0x420e, 0x4214, 0x421a];
const TAIL_L: [usize; 2] = [0x16d5, 0x170f];
const TAIL_R: [usize; 2] = [0x1928, 0x19be];
const FIN_L: [usize; 2] = [0x17f8, 0x1831];
const FIN_R: [usize; 2] = [0x1ae5, 0x1af9];
const FLAPS_L: [usize; 2] = [0x2e82, 0x2e95];
const FLAPS_R: [usize; 2] = [0x2db3, 0x2dc6];
const BRACE: [usize; 2] = [0x2c2c, 0x2c47];
pub struct Rig;
const FLAME: [usize; 16] = [
    0x3026, 0x3049, 0x306c, 0x308f, 0x30b2, 0x30d5, 0x30f8, 0x311b, 0x313e, 0x3159, 0x3174, 0x318f,
    0x31aa, 0x31c5, 0x31e0, 0x31fb,
];
const BRAKE: [usize; 2] = [0x2f69, 0x2f7c];
const GEAR: [usize; 8] = [
    0x2b74, 0x2b8f, 0x2c2c, 0x2c47, 0x2c8f, 0x2caa, 0x2bd7, 0x2bf2,
];
const SLATS: [usize; 4] = [0x2d29, 0x2d3c, 0x2ce4, 0x2cf7];
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("SU27.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("SU27.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed SU27.SH root{address:x}").into());
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
        return Err(format!("unreviewed SU27.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn fin_distance(p: [f32; 3]) -> f32 {
    p[1] + 43. + 0.25 * (p[2] + 5.)
}
fn fin_hinge(left: bool) -> ([f32; 3], [f32; 3]) {
    let lower = [
        if left { -21. } else { 22. } + 12. / 37.,
        -55. + 36. * 12. / 37.,
        -5. + 4. * 12. / 37.,
    ];
    let upper = [
        if left { -21. } else { 22. },
        -58. + 7. * 4.75 / 8.25,
        36. + 5. * 4.75 / 8.25,
    ];
    (lower, std::array::from_fn(|i| upper[i] - lower[i]))
}
fn trail(p: [f32; 3], left: bool) -> bool {
    p[1] < -10. - (p[0].abs() - 21.) * 16. / if left { 57. } else { 58. } - 1e-4
}
fn slat_upper(p: [f32; 3]) -> [f32; 3] {
    if p[0].abs() < 30. {
        [p[0].signum() * 22., 27., -1.]
    } else {
        [p[0], -14., -1.]
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 12838
            || shape.faces.len() != 146
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed SU27.SH layout".into());
        }
        roots(&shape, &TAIL_L, &[[-21., -26., -6.], [-21., -56., -6.]])?;
        roots(&shape, &TAIL_R, &[[21., -26., -6.], [21., -56., -6.]])?;
        roots(
            &shape,
            &FIN_L,
            &[
                [-21., -55., -5.],
                [-21., -58., 36.],
                [-21., -51., 41.],
                [-20., -19., -1.],
            ],
        )?;
        roots(
            &shape,
            &FIN_R,
            &[
                [22., -55., -5.],
                [22., -58., 36.],
                [22., -51., 41.],
                [23., -19., -1.],
            ],
        )?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for (word, ids, down, up) in [
            (0x420e, FLAPS_L, [0x2e3d, 0x2e50], [0x2ec7, 0x2eda]),
            (0x4214, FLAPS_R, [0x2d6e, 0x2d81], [0x2df8, 0x2e0b]),
        ] {
            for (value, added, z) in [(-1, down, -5.), (1, up, 3.)] {
                let pose = branch(bytes, &original, word, value, &added, &ids)?;
                for a in ids {
                    let base = one(&shape, a)?;
                    if base.positions.len() != 4 {
                        return Err("unreviewed SU27.SH flap topology".into());
                    }
                    let expected: Vec<_> = base
                        .positions
                        .iter()
                        .map(|p| {
                            if trail(*p, word == 0x420e) {
                                [p[0], p[1], z]
                            } else {
                                *p
                            }
                        })
                        .collect();
                    if !added.iter().any(|b| {
                        one(&pose, *b).is_ok_and(|f| {
                            f.positions.len() == 4
                                && expected.iter().all(|p| f.positions.contains(p))
                        })
                    }) {
                        return Err("unreviewed SU27.SH signed flap endpoints".into());
                    }
                }
            }
        }
        for (word, ids) in [
            (0x41f0, &FLAME[..]),
            (0x41f6, &BRAKE[..]),
            (0x41fc, &GEAR[..]),
            (0x421a, &SLATS[..]),
        ] {
            let pose = branch(bytes, &original, word, 1, ids, &[])?;
            shape
                .faces
                .extend(pose.faces.into_iter().filter(|f| ids.contains(&f.address)));
        }
        branch(bytes, &original, 0x41fc, -1, &BRACE, &[])?;
        roots(
            &shape,
            &[0x2b74, 0x2b8f],
            &[[23., -5., 0.], [23., -13., 0.]],
        )?;
        roots(
            &shape,
            &[0x2bd7, 0x2bf2],
            &[[-23., -5., 0.], [-23., -13., 0.]],
        )?;
        roots(&shape, &BRACE, &[[-1., 74., -4.], [-1., 74., -7.]])?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if FIN_L.contains(&f.address) || FIN_R.contains(&f.address) {
                let (pivot, axis) = fin_hinge(FIN_L.contains(&f.address));
                faces.extend(crate::aircraft_animation::split_surface(
                    &f,
                    pivot,
                    axis.map(f64::from),
                    0.,
                    fin_distance,
                ));
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
        if TAIL_L.contains(&a) || TAIL_R.contains(&a) {
            let left = TAIL_L.contains(&a);
            let mut moved = source.clone();
            turn(
                &mut moved,
                [if left { -21. } else { 21. }, -41., -6.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() > 21. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if FIN_L.contains(&a) || FIN_R.contains(&a) {
            if source.positions.iter().all(|p| fin_distance(*p) <= 1e-4) {
                let (pivot, axis) = fin_hinge(FIN_L.contains(&a));
                turn(&mut result, pivot, axis, 0.35 * state.rudder.clamp(-1., 1.));
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if fin_distance(*old).abs() < 1e-4 {
                        *p = *old;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if FLAPS_L.contains(&a) || FLAPS_R.contains(&a) {
            let left = FLAPS_L.contains(&a);
            let side = if left { -1. } else { 1. };
            for p in &mut result.positions {
                if trail(*p, left) {
                    p[2] = -1. - 4. * state.flaps.clamp(0., 1.) as f32
                        + 4. * side * state.aileron.clamp(-1., 1.) as f32;
                }
            }
            update_normal(source, &mut result);
        } else if SLATS.contains(&a) {
            if state.flaps <= 0. {
                return None;
            }
            let t = state.flaps.clamp(0., 1.) as f32;
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if old[2] == -6. {
                    let q = slat_upper(*old);
                    *p = std::array::from_fn(|i| q[i] + (old[i] - q[i]) * t);
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
                if BRACE.contains(&a) {
                    0.
                } else {
                    state.nosewheel_angle()
                },
            );
            update_normal(source, &mut result);
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            turn(
                &mut result,
                [0., 40., 11.],
                [1., 0., 0.],
                (14f64 / 22.).atan() * (1. - state.brake.clamp(0., 1.)),
            );
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -60. + (p[1] + 60.) * state.exhaust.clamp(0., 1.) as f32;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
fn gear_positions(face: &mut Face, gear: f64, steering: f64) {
    let a = face.address;
    let angle = -FRAC_PI_2 * (1. - gear.clamp(0., 1.));
    if [0x2b74, 0x2b8f, 0x2bd7, 0x2bf2].contains(&a) {
        let side = if a < 0x2bd7 { 1. } else { -1. };
        turn(face, [side * 23., -9., 0.], [1., 0., 0.], angle);
    } else if BRACE.contains(&a) {
        let old = face.positions.clone();
        turn(face, [0., 60., -3.], [1., 0., 0.], angle);
        for (p, q) in face.positions.iter_mut().zip(old) {
            if q[1] == 74. {
                *p = q;
            }
        }
    } else {
        turn(face, [0., 60., -3.], [0., 0., 1.], -steering);
        turn(face, [0., 60., -3.], [1., 0., 0.], angle);
    }
}
pub(crate) fn flame(a: usize) -> bool {
    FLAME.contains(&a)
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
            normal: Some([1., 0., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
            subtype: 0xed,
        }
    }
    fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
    }
    #[test]
    fn asymmetric_tail_roots_stay_fixed_and_both_distal_controls_have_signed_pitch() {
        let mut s = state();
        for (a, side) in [(0x16d5, -1.), (0x1928, 1.)] {
            let source = face(
                a,
                vec![
                    [side * 49., -51., -6.],
                    [side * 44., -59., -6.],
                    [side * 21., -56., -6.],
                    [side * 21., -26., -6.],
                ],
            );
            for v in [-1., 0., 1.] {
                s.elevator = v;
                let out = Rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[2], source.positions[2]);
                assert_eq!(out.positions[3], source.positions[3]);
                for i in 0..2 {
                    if v != 0. {
                        assert!((out.positions[i][2] - source.positions[i][2]) * v as f32 > 0.);
                    } else {
                        assert_eq!(out.positions[i], source.positions[i]);
                    }
                }
            }
        }
    }
    #[test]
    fn signed_source_flaperon_endpoints_and_fixed_leading_edges_hold_at_25_combinations() {
        let mut s = state();
        for (a, side) in [(0x2e82, -1.), (0x2db3, 1.)] {
            let outer = if side < 0. { 78. } else { 79. };
            let source = face(
                a,
                vec![
                    [side * 21., -10., -1.],
                    [side * outer, -26., -1.],
                    [side * outer, -31., -1.],
                    [side * 22., -19., -1.],
                ],
            );
            for flap in [0., 0.25, 0.5, 0.75, 1.] {
                for roll in [-1., -0.5, 0., 0.5, 1.] {
                    s.flaps = flap;
                    s.aileron = roll;
                    let out = Rig.animate(&source, &s).unwrap();
                    assert_eq!(out.positions[0], source.positions[0]);
                    assert_eq!(out.positions[1], source.positions[1]);
                    for i in 2..4 {
                        assert!(out.positions[i][2] >= -9. && out.positions[i][2] <= 3.);
                        if roll == 0. && flap == 1. {
                            assert_eq!(out.positions[i][2], -5.);
                        }
                        if flap == 0. && roll != 0. {
                            assert!((out.positions[i][2] + 1.) * roll as f32 * side > 0.);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn own_slat_roots_are_fixed_and_deployed_endpoint_exact() {
        let mut s = state();
        let source = face(
            0x2ce4,
            vec![
                [23., 31., -6.],
                [79., -11., -6.],
                [79., -14., -1.],
                [22., 27., -1.],
            ],
        );
        s.flaps = 0.;
        assert!(Rig.animate(&source, &s).is_none());
        for v in [0.25, 0.5, 0.75, 1.] {
            s.flaps = v;
            let out = Rig.animate(&source, &s).unwrap();
            assert_eq!(out.positions[2], source.positions[2]);
            assert_eq!(out.positions[3], source.positions[3]);
            if v == 1. {
                assert_eq!(out.positions, source.positions);
            } else {
                assert!(out.positions[0][2] > -6. && out.positions[0][2] < -1.);
            }
        }
    }
    #[test]
    fn main_cards_are_rigid_and_separate_while_brace_has_fixed_body_edge_and_rigid_distal_pin() {
        let main = face(
            0x2b74,
            vec![
                [23., -5., 0.],
                [23., -13., 0.],
                [23., -13., -23.],
                [23., -5., -23.],
            ],
        );
        let brace = face(
            0x2c2c,
            vec![
                [-1., 74., -7.],
                [-1., 57., -7.],
                [-1., 57., -4.],
                [-1., 74., -4.],
            ],
        );
        fn side(a: [f32; 3], b: [f32; 3], p: [f32; 3]) -> f32 {
            (b[1] - a[1]) * (p[2] - a[2]) - (b[2] - a[2]) * (p[1] - a[1])
        }
        fn cross(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) -> bool {
            side(a, b, c) * side(a, b, d) < -1e-6 && side(c, d, a) * side(c, d, b) < -1e-6
        }
        for step in 0..=400 {
            let gear = f64::from(step) / 400.;
            let mut out = main.clone();
            gear_positions(&mut out, gear, 0.);
            let hub: [f32; 3] =
                std::array::from_fn(|i| (out.positions[0][i] + out.positions[1][i]) * 0.5);
            assert!(d2(hub, [23., -9., 0.]) < 1e-8);
            assert!(out.positions.iter().all(|p| p[0] == 23.));
            for i in 0..4 {
                for j in i + 1..4 {
                    assert!(
                        (d2(main.positions[i], main.positions[j])
                            - d2(out.positions[i], out.positions[j]))
                        .abs()
                            < 1e-3
                    );
                }
            }
            let mut out = brace.clone();
            gear_positions(&mut out, gear, 0.);
            assert_eq!(out.positions[0], brace.positions[0]);
            assert_eq!(out.positions[3], brace.positions[3]);
            assert!((d2(out.positions[1], out.positions[2]) - 9.).abs() < 1e-4);
            assert!(!cross(
                out.positions[0],
                out.positions[1],
                out.positions[2],
                out.positions[3]
            ));
            assert!(!cross(
                out.positions[1],
                out.positions[2],
                out.positions[3],
                out.positions[0]
            ));
        }
    }
    #[test]
    fn own_canted_fin_cut_points_remain_fixed_on_both_sides() {
        let mut s = state();
        s.rudder = 1.;
        for (left, a) in [(true, 0x17f8), (false, 0x1ae5)] {
            let (p, axis) = fin_hinge(left);
            let q = std::array::from_fn(|i| p[i] + axis[i]);
            let source = face(
                a,
                vec![
                    p,
                    q,
                    [if left { -21. } else { 22. }, -58., 36.],
                    [if left { -21. } else { 22. }, -55., -5.],
                ],
            );
            let out = Rig.animate(&source, &s).unwrap();
            assert_eq!(out.positions[0], p);
            assert_eq!(out.positions[1], q);
            assert_ne!(out.positions[2], source.positions[2]);
        }
    }
}
