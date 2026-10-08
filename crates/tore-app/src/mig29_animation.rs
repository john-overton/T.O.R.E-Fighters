//! MiG29 source-specific repair: fixed control attachments and rigid separated gear.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::FRAC_PI_2,
};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 5] = [0x8240, 0x8246, 0x824c, 0x8258, 0x825e];
const TAIL: [usize; 8] = [
    0x474f, 0x4776, 0x479e, 0x47c5, 0x4cf6, 0x4dbc, 0x4d1d, 0x4de3,
];
const FIN_R: [usize; 2] = [0x334e, 0x3371];
const FIN_L: [usize; 2] = [0x4d6c, 0x4d93];
const ROLL_R: [usize; 2] = [0x445c, 0x44fc];
const ROLL_L: [usize; 2] = [0x4b23, 0x4c61];
const CAPS: [usize; 2] = [0x51d7, 0x52de];
struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Vec<[f32; 3]>,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
const FLAME: [usize; 16] = [
    0x553d, 0x5564, 0x558b, 0x55b2, 0x55d9, 0x5600, 0x5627, 0x564e, 0x5675, 0x569c, 0x56c3, 0x56ea,
    0x5711, 0x5738, 0x575f, 0x5786,
];
const BRAKE: [usize; 4] = [0x53db, 0x53f2, 0x5478, 0x548f];
const GEAR: [usize; 16] = [
    0x50e2, 0x5101, 0x5120, 0x513f, 0x4ecc, 0x4eeb, 0x4f0a, 0x4f29, 0x4f48, 0x4f67, 0x4fe3, 0x5002,
    0x5021, 0x5040, 0x505f, 0x507e,
];
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("MIG29.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("MIG29.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed MIG29.SH root{address:x}").into());
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
        return Err(format!("unreviewed MIG29.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn fin_distance(p: [f32; 3]) -> f32 {
    p[1] + 32. + 0.20 * (p[2] - 1.)
}
fn fin_hinge(left: bool) -> ([f32; 3], [f32; 3]) {
    let lower = [
        if left { -17. } else { 16. + 9. / 30.6 },
        -41. + 29. * 9. / 30.6,
        1. + 8. * 9. / 30.6,
    ];
    let upper = [
        if left { -19. } else { 19. },
        -41. + 5. * 2.2 / 5.4,
        35. + 2. * 2.2 / 5.4,
    ];
    (lower, std::array::from_fn(|i| upper[i] - lower[i]))
}
fn roll_spec(a: usize) -> Option<([f32; 3], [f32; 3])> {
    if ROLL_L.contains(&a) {
        Some(([-36., -8., 0.5], [-21., -3., 0.]))
    } else if ROLL_R.contains(&a) {
        Some(([36., -8., 0.5], [21., -3., 0.]))
    } else {
        None
    }
}
fn roll_trailing(p: [f32; 3]) -> bool {
    p[1] < -8. - (p[0].abs() - 36.) * 3. / 21. - 1e-4
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 29290
            || shape.faces.len() != 328
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed MIG29.SH layout".into());
        }
        roots(
            &shape,
            &FIN_R,
            &[
                [16., -41., 1.],
                [19., -41., 35.],
                [19., -36., 37.],
                [17., -12., 9.],
            ],
        )?;
        roots(
            &shape,
            &FIN_L,
            &[
                [-17., -41., 1.],
                [-19., -41., 35.],
                [-19., -36., 37.],
                [-17., -12., 9.],
            ],
        )?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        let mut caps = Vec::new();
        for (word, bindings, cap) in [
            (
                0x8258,
                [
                    (0x531e, 0x5290, [2, 3, 0, 1]),
                    (0x5345, 0x52b7, [1, 2, 3, 0]),
                ],
                0x52de,
            ),
            (
                0x825e,
                [
                    (0x5217, 0x5189, [3, 0, 1, 2]),
                    (0x523e, 0x51b0, [0, 1, 2, 3]),
                ],
                0x51d7,
            ),
        ] {
            let added = [bindings[0].1, bindings[1].1, cap];
            let removed = [bindings[0].0, bindings[1].0];
            let pose = branch(bytes, &original, word, -1, &added, &removed)?;
            branch(bytes, &original, word, 1, &[], &removed)?;
            for (a, b, order) in bindings {
                let base = one(&shape, a)?;
                let down = one(&pose, b)?;
                if base.positions.len() != 4 || down.positions.len() != 4 {
                    return Err("unreviewed MIG29.SH flap topology".into());
                }
                let deployed: Vec<_> = order.into_iter().map(|i| down.positions[i]).collect();
                for (p, q) in base.positions.iter().zip(&deployed) {
                    if p[1] >= -8. && p != q {
                        return Err("unreviewed MIG29.SH fixed thick flap edge".into());
                    }
                    if p[1] < -8. {
                        let expected = if p[0].abs() == 17. {
                            [p[0], -11., -5.]
                        } else {
                            [p[0], -13., -4.]
                        };
                        if *q != expected {
                            return Err("unreviewed MIG29.SH flap endpoint".into());
                        }
                    }
                }
                flaps.insert(
                    a,
                    Morph {
                        neutral: base.positions.clone(),
                        deployed,
                    },
                );
            }
            let mut f = one(&pose, cap)?.clone();
            let deployed = f.positions.clone();
            for p in &mut f.positions {
                if p[2] == -4. {
                    p[1] = -14.;
                    p[2] = 0.;
                }
            }
            flaps.insert(
                cap,
                Morph {
                    neutral: f.positions.clone(),
                    deployed,
                },
            );
            caps.push(f);
        }
        for (word, ids) in [
            (0x8240, &FLAME[..]),
            (0x8246, &BRAKE[..]),
            (0x824c, &GEAR[..]),
        ] {
            let pose = branch(bytes, &original, word, 1, ids, &[])?;
            shape
                .faces
                .extend(pose.faces.into_iter().filter(|f| ids.contains(&f.address)));
        }
        roots(&shape, &[0x4ecc, 0x4eeb], &[[13., 1., 0.], [21., 1., 0.]])?;
        roots(&shape, &[0x4fe3, 0x5002], &[[-13., 1., 0.], [-21., 1., 0.]])?;
        roots(
            &shape,
            &[0x5120, 0x513f],
            &[[-3., 47., -2.], [3., 47., -2.]],
        )?;
        shape.faces.extend(caps);
        let mut faces = Vec::new();
        for f in shape.faces {
            if TAIL.contains(&f.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &f,
                    [0., -41., 0.],
                    [1., 0., 0.],
                    0.,
                    |p| p[1] + 41.,
                ));
            } else if FIN_L.contains(&f.address) || FIN_R.contains(&f.address) {
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
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if let Some(m) = self.flaps.get(&a) {
            if CAPS.contains(&a) && state.flaps <= 0. {
                return None;
            }
            let t = state.flaps.clamp(0., 1.) as f32;
            result.positions = m
                .neutral
                .iter()
                .zip(&m.deployed)
                .map(|(p, q)| std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t))
                .collect();
            update_normal(source, &mut result);
        } else if TAIL.contains(&a) && source.positions.iter().all(|p| p[1] <= -41. + 1e-4) {
            turn(
                &mut result,
                [0., -41., 0.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if (old[1] + 41.).abs() < 1e-4 {
                    *p = *old;
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
        } else if let Some((pivot, axis)) = roll_spec(a) {
            let mut moved = source.clone();
            turn(
                &mut moved,
                pivot,
                axis,
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for ((p, q), old) in result
                .positions
                .iter_mut()
                .zip(moved.positions)
                .zip(&source.positions)
            {
                if roll_trailing(*old) {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            let nose = a >= 0x50e2;
            let left = (0x4fe3..0x50e2).contains(&a);
            let pivot = if nose {
                [0., 47., -2.]
            } else {
                [if left { -17. } else { 17. }, 1., 0.]
            };
            if nose {
                turn(&mut result, pivot, [0., 0., 1.], -state.nosewheel_angle());
            }
            turn(
                &mut result,
                pivot,
                [1., 0., 0.],
                -FRAC_PI_2 * (1. - state.gear.clamp(0., 1.)),
            );
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            let upper = a >= 0x5478;
            let (pivot, angle) = if upper {
                ([0., -7., 7.], (9f64 / 7.).atan())
            } else {
                ([0., -8., -1.], -(8f64 / 6.).atan())
            };
            turn(
                &mut result,
                pivot,
                [1., 0., 0.],
                angle * (1. - state.brake.clamp(0., 1.)),
            );
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -41. + (p[1] + 41.) * state.exhaust.clamp(0., 1.) as f32;
            }
            update_normal(source, &mut result);
        }
        Some(result)
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
    fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
    }
    #[test]
    fn main_cards_fold_aft_without_crossing_at_401_samples() {
        let rig = rig();
        let mut s = state();
        for (a, x) in [(0x4ecc, 17.), (0x4fe3, -17.)] {
            let source = face(
                a,
                vec![
                    [x - 4., 1., -20.],
                    [x - 4., 1., 0.],
                    [x + 4., 1., 0.],
                    [x + 4., 1., -20.],
                ],
            );
            for step in 1..=400 {
                s.gear = f64::from(step) / 400.;
                let out = rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[1], source.positions[1]);
                assert_eq!(out.positions[2], source.positions[2]);
                assert!(out.positions.iter().all(|p| p[0] * x.signum() >= 13.));
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
            }
        }
        s.gear = 0.;
        for a in GEAR {
            assert!(rig.animate(&face(a, vec![[0.; 3]; 3]), &s).is_none());
        }
    }
    #[test]
    fn forward_tail_region_and_exact_cut_stay_fixed_with_signed_aft_pitch() {
        let rig = rig();
        let mut s = state();
        let source = face(
            0x4cf6,
            vec![
                [-30., -54., 0.],
                [-38., -45., 0.],
                [-25., -30., 0.],
                [-17., -41., 0.],
            ],
        );
        let parts = crate::aircraft_animation::split_surface(
            &source,
            [0., -41., 0.],
            [1., 0., 0.],
            0.,
            |p| p[1] + 41.,
        );
        for value in [-1., 0., 1.] {
            s.elevator = value;
            for piece in &parts {
                let out = rig.animate(piece, &s).unwrap();
                for (p, q) in piece.positions.iter().zip(out.positions) {
                    if p[1] >= -41. - 1e-4 {
                        assert_eq!(*p, q);
                    } else if value != 0. {
                        assert!((q[2] - p[2]) * value as f32 > 0.);
                    }
                }
            }
        }
    }
    #[test]
    fn asymmetric_fin_hinge_points_remain_fixed_for_both_skins() {
        let rig = rig();
        let mut s = state();
        for (left, a) in [(false, 0x334e), (true, 0x4d6c)] {
            let (pivot, axis) = fin_hinge(left);
            let upper = std::array::from_fn(|i| pivot[i] + axis[i]);
            let source = face(
                a,
                vec![
                    pivot,
                    upper,
                    [if left { -19. } else { 19. }, -41., 35.],
                    [if left { -17. } else { 16. }, -41., 1.],
                ],
            );
            for value in [-1., -0.5, 0., 0.5, 1.] {
                s.rudder = value;
                let out = rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[0], pivot);
                assert_eq!(out.positions[1], upper);
                if value == 0. {
                    assert_eq!(out.positions, source.positions);
                } else {
                    assert_ne!(out.positions[2], source.positions[2]);
                }
            }
        }
    }
    #[test]
    fn separate_outer_roll_has_correct_sign_and_keeps_thick_front_edges() {
        let rig = rig();
        let mut s = state();
        for (a, sign) in [(0x445c, 1.), (0x4c61, -1.)] {
            let source = face(
                a,
                vec![
                    [sign * 36., -14., 0.],
                    [sign * 36., -8., 1.],
                    [sign * 57., -11., 1.],
                    [sign * 61., -16., 0.],
                ],
            );
            for value in [-1., 0., 1.] {
                s.aileron = value;
                let out = rig.animate(&source, &s).unwrap();
                assert_eq!(out.positions[1], source.positions[1]);
                assert_eq!(out.positions[2], source.positions[2]);
                if value != 0. {
                    assert!(
                        (out.positions[0][2] - source.positions[0][2]) * value as f32 * sign > 0.
                    );
                }
            }
        }
    }
}
