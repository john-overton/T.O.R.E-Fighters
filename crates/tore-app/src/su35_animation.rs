//! Su35 own source endpoints, fixed root fits and independently separated gear.
use crate::{
    AppResult, additional_animation::turn, aircraft_animation::update_normal, flight::State,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::FRAC_PI_2,
};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 5] = [0x79c0, 0x79c6, 0x79cc, 0x79d8, 0x79de];
const TAIL_L: [usize; 2] = [0x5312, 0x535e];
const TAIL_R: [usize; 2] = [0x25f2, 0x2639];
const CANARD_L: [usize; 2] = [0x52b0, 0x52d7];
const CANARD_R: [usize; 2] = [0x256d, 0x2580];
const FIN: [usize; 4] = [0x2268, 0x4edf, 0x2301, 0x26a8];
const ROLL_L: [usize; 4] = [0x517b, 0x5198, 0x51d6, 0x52f0];
const ROLL_R: [usize; 4] = [0x24c0, 0x2537, 0x254f, 0x25a3];
const CAPS: [usize; 2] = [0x5857, 0x57d8];
const BRACE: [usize; 2] = [0x5680, 0x5697];
struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Vec<[f32; 3]>,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
const FLAME: [usize; 8] = [
    0x59e9, 0x5a08, 0x5a27, 0x5a46, 0x5a65, 0x5a84, 0x5aa3, 0x5ac2,
];
const BRAKE: [usize; 2] = [0x5944, 0x596b];
const GEAR: [usize; 18] = [
    0x5432, 0x5451, 0x5470, 0x548f, 0x54ae, 0x54cd, 0x5680, 0x5697, 0x56f3, 0x5712, 0x5731, 0x5750,
    0x5549, 0x5568, 0x5587, 0x55a6, 0x55c5, 0x55e4,
];
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("SU35.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("SU35.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed SU35.SH root{address:x}").into());
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
        return Err(format!("unreviewed SU35.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn roll_spec(a: usize) -> Option<(f32, [f32; 3], [f32; 3])> {
    if ROLL_L.contains(&a) {
        Some((50., [-50., -15., -0.5], [-21., 2., 0.]))
    } else if ROLL_R.contains(&a) {
        Some((49., [49., -15., -0.5], [22., 2., 0.]))
    } else {
        None
    }
}
fn roll_trailing(p: [f32; 3], inner: f32) -> bool {
    p[1] < -15. + (p[0].abs() - inner) * 2. / (71. - inner) - 1e-4
}
fn flap_target(p: [f32; 3]) -> [f32; 3] {
    if p[1] == -14. {
        [p[0], -13., -7.]
    } else if p[1] == -22. {
        [p[0], -20., -4.]
    } else {
        p
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 27114
            || shape.faces.len() != 329
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed SU35.SH layout".into());
        }
        roots(
            &shape,
            &TAIL_R,
            &[[20., -25., -5.], [17., -46., -5.], [24., -58., -5.]],
        )?;
        roots(
            &shape,
            &TAIL_L,
            &[[-20., -25., -5.], [-20., -46., -5.], [-24., -58., -5.]],
        )?;
        roots(&shape, &CANARD_R, &[[19., 38., 0.], [20., 53., 0.]])?;
        roots(&shape, &CANARD_L, &[[-20., 38., 0.], [-20., 53., 0.]])?;
        for (a, x) in [(0x2268, -20.), (0x4edf, -20.), (0x2301, 20.), (0x26a8, 20.)] {
            roots(
                &shape,
                &[a],
                &[[x, -4., 1.], [x, -40., 5.], [x, -42., 38.], [x, -30., 38.]],
            )?;
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let left = branch(
            bytes,
            &original,
            0x79d8,
            -1,
            &[0x5819, 0x5838, 0x5857],
            &[0x5897, 0x58b6],
        )?;
        branch(bytes, &original, 0x79d8, 1, &[], &[0x5897, 0x58b6])?;
        let right = branch(bytes, &original, 0x79de, -1, &[0x579a, 0x57b9, 0x57d8], &[])?;
        branch(bytes, &original, 0x79de, 1, &[], &[])?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if f.address == 0x24c0 {
                faces.extend(crate::aircraft_animation::split_surface(
                    &f,
                    [0.; 3],
                    [1., 0., 0.],
                    0.,
                    |p| p[0] - 49.,
                ));
            } else if FIN.contains(&f.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &f,
                    [f.positions[0][0], -30., 0.],
                    [0., 0., 1.],
                    0.,
                    |p| p[1] + 30.,
                ));
            } else {
                faces.push(f);
            }
        }
        shape.faces = faces;
        let mut flaps = BTreeMap::new();
        for (a, b, pose) in [
            (0x5897, 0x5819, &left),
            (0x58b6, 0x5838, &left),
            (0x24c0, 0x579a, &right),
            (0x251c, 0x57b9, &right),
        ] {
            let bases: Vec<_> = shape
                .faces
                .iter()
                .filter(|f| {
                    f.address == a
                        && (a != 0x24c0 || f.positions.iter().all(|p| p[0] <= 49. + 1e-4))
                })
                .collect();
            if bases.len() != 1 {
                return Err("unreviewed SU35.SH inboard flap partition".into());
            }
            let base = bases[0];
            let down = one(pose, b)?;
            let deployed: Vec<_> = base.positions.iter().copied().map(flap_target).collect();
            if base.positions.len() != 4
                || down.positions.len() != 4
                || deployed.iter().any(|p| !down.positions.contains(p))
            {
                return Err("unreviewed SU35.SH own flap correspondence".into());
            }
            flaps.insert(
                a,
                Morph {
                    neutral: base.positions.clone(),
                    deployed,
                },
            );
        }
        for (a, pose) in [(0x5857, &left), (0x57d8, &right)] {
            let mut cap = one(pose, a)?.clone();
            let deployed = cap.positions.clone();
            for p in &mut cap.positions {
                if p[2] == -4. {
                    p[1] = -22.;
                    p[2] = 0.;
                }
            }
            flaps.insert(
                a,
                Morph {
                    neutral: cap.positions.clone(),
                    deployed,
                },
            );
            shape.faces.push(cap);
        }
        for (word, ids) in [
            (0x79c0, &FLAME[..]),
            (0x79c6, &BRAKE[..]),
            (0x79cc, &GEAR[..]),
        ] {
            let pose = branch(bytes, &original, word, 1, ids, &[])?;
            shape
                .faces
                .extend(pose.faces.into_iter().filter(|f| ids.contains(&f.address)));
        }
        roots(
            &shape,
            &[0x5731, 0x5750],
            &[[-3., 57., -1.], [3., 57., -1.]],
        )?;
        roots(&shape, &BRACE, &[[-1., 73., -3.], [-1., 73., -7.]])?;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        let morph = self.flaps.get(&a).filter(|m| m.neutral == source.positions);
        if let Some(m) = morph {
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
        } else if TAIL_L.contains(&a) || TAIL_R.contains(&a) {
            let left = TAIL_L.contains(&a);
            let mut moved = source.clone();
            turn(
                &mut moved,
                [if left { -20. } else { 20. }, -42., -5.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() > 24. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if CANARD_L.contains(&a) || CANARD_R.contains(&a) {
            let left = CANARD_L.contains(&a);
            let mut moved = source.clone();
            turn(
                &mut moved,
                [if left { -20. } else { 19.5 }, 45.5, 0.],
                [1., 0., 0.],
                0.25 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() > 20. {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if FIN.contains(&a) {
            if source.positions.iter().all(|p| p[1] <= -30. + 1e-4) {
                turn(
                    &mut result,
                    [source.positions[0][0], -30., 0.],
                    [0., 0., 1.],
                    0.35 * state.rudder.clamp(-1., 1.),
                );
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if (old[1] + 30.).abs() < 1e-4 {
                        *p = *old;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if let Some((inner, pivot, axis)) = roll_spec(a) {
            if a != 0x24c0 || source.positions.iter().all(|p| p[0] >= 49. - 1e-4) {
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
                    if roll_trailing(*old, inner) {
                        *p = q;
                    }
                }
                update_normal(source, &mut result);
            }
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
                [0., 23., 8.],
                [1., 0., 0.],
                (10f64 / 21.).atan() * (1. - state.brake.clamp(0., 1.)),
            );
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -59. + (p[1] + 59.) * state.exhaust.clamp(0., 1.) as f32;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
fn gear_positions(face: &mut Face, gear: f64, steering: f64) {
    let a = face.address;
    let closing = 1. - gear.clamp(0., 1.);
    if BRACE.contains(&a) {
        let old = face.positions.clone();
        turn(face, [0., 57., -1.], [1., 0., 0.], -FRAC_PI_2 * closing);
        for (p, q) in face.positions.iter_mut().zip(old) {
            if q[1] == 73. {
                *p = q;
            }
        }
    } else if [0x56f3, 0x5712, 0x5731, 0x5750].contains(&a) {
        turn(face, [0., 57., -1.], [0., 0., 1.], -steering);
        turn(face, [0., 57., -1.], [1., 0., 0.], -FRAC_PI_2 * closing);
    } else {
        let side = if a < 0x5549 { 1. } else { -1. };
        turn(
            face,
            [side * 23., 3., -1.],
            [0., 1., 0.],
            side as f64 * FRAC_PI_2 * closing,
        );
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
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
        }
    }
    fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
    }
    #[test]
    fn tail_and_canard_distal_controls_move_with_their_authored_sign_and_keep_own_roots() {
        let mut s = state();
        let rig = rig();
        for (a, points, inner_limit, sign) in [
            (
                0x25f2,
                vec![
                    [47., -50., -5.],
                    [43., -59., -5.],
                    [24., -58., -5.],
                    [17., -46., -5.],
                    [20., -25., -5.],
                ],
                24.,
                1.,
            ),
            (
                0x5312,
                vec![
                    [-43., -59., -5.],
                    [-47., -50., -5.],
                    [-20., -25., -5.],
                    [-20., -46., -5.],
                    [-24., -58., -5.],
                ],
                24.,
                1.,
            ),
            (
                0x256d,
                vec![
                    [20., 53., 0.],
                    [19., 38., 0.],
                    [33., 32., 0.],
                    [33., 36., 0.],
                ],
                20.,
                -1.,
            ),
            (
                0x52b0,
                vec![
                    [-20., 38., 0.],
                    [-32., 32., 0.],
                    [-32., 36., 0.],
                    [-20., 53., 0.],
                ],
                20.,
                -1.,
            ),
        ] {
            let source = face(a, points);
            for v in [-1., 0., 1.] {
                s.elevator = v;
                let out = rig.animate(&source, &s).unwrap();
                for (p, q) in source.positions.iter().zip(out.positions) {
                    if p[0].abs() <= inner_limit {
                        assert_eq!(*p, q);
                    } else if v != 0. {
                        assert!((q[2] - p[2]) * sign * v as f32 > 0.);
                    } else {
                        assert_eq!(*p, q);
                    }
                }
            }
        }
    }
    #[test]
    fn right_upper_split_preserves_both_pieces_and_does_not_couple_flap_and_outer_roll() {
        let source = face(
            0x24c0,
            vec![
                [19., -14., 0.],
                [19., -4., 0.],
                [49., -15., 0.],
                [71., -27., 0.],
                [49., -22., 0.],
            ],
        );
        let parts =
            crate::aircraft_animation::split_surface(&source, [0.; 3], [1., 0., 0.], 0., |p| {
                p[0] - 49.
            });
        assert_eq!(parts.len(), 2);
        let inner = parts
            .iter()
            .find(|f| f.positions.iter().all(|p| p[0] <= 49.))
            .unwrap();
        let outer = parts
            .iter()
            .find(|f| f.positions.iter().any(|p| p[0] > 49.))
            .unwrap();
        let deployed = inner.positions.iter().copied().map(flap_target).collect();
        let rig = Rig {
            flaps: [(
                0x24c0,
                Morph {
                    neutral: inner.positions.clone(),
                    deployed,
                },
            )]
            .into(),
        };
        let mut s = state();
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                s.flaps = flap;
                s.aileron = roll;
                let a = rig.animate(inner, &s).unwrap();
                let b = rig.animate(outer, &s).unwrap();
                for (f, out) in [(inner, &a), (outer, &b)] {
                    for (p, q) in f.positions.iter().zip(&out.positions) {
                        if p[1] == -15. || p[1] == -4. {
                            assert_eq!(p, q);
                        }
                    }
                }
                s.aileron = 0.;
                assert_eq!(a.positions, rig.animate(inner, &s).unwrap().positions);
                s.aileron = roll;
                s.flaps = 0.;
                assert_eq!(b.positions, rig.animate(outer, &s).unwrap().positions);
                if roll != 0. {
                    let i = outer.positions.iter().position(|p| p[0] == 71.).unwrap();
                    assert!((b.positions[i][2] - outer.positions[i][2]) * roll as f32 > 0.);
                }
            }
        }
    }
    #[test]
    fn retained_main_fold_keeps_complete_cards_rigid_and_signed_bounds_at_401_samples() {
        for (a, side) in [(0x5432, 1.), (0x5549, -1.)] {
            let source = face(
                a,
                vec![
                    [side * 18., 3., -21.],
                    [side * 18., 3., -1.],
                    [side * 29., 3., -1.],
                    [side * 29., 3., -21.],
                ],
            );
            for step in 0..=400 {
                let mut out = source.clone();
                gear_positions(&mut out, f64::from(step) / 400., 0.);
                assert!(out.positions.iter().all(|p| p[0] * side >= 2.38));
                let hub: [f32; 3] = std::array::from_fn(|i| {
                    (out.positions[1][i] * 6. + out.positions[2][i] * 5.) / 11.
                });
                assert!(d2(hub, [side * 23., 3., -1.]) < 1e-8);
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
    fn nose_front_edge_and_brace_body_edge_stay_attached_with_rigid_four_unit_distal_pin() {
        let nose = face(
            0x5731,
            vec![
                [-3., 56., -22.],
                [3., 56., -22.],
                [3., 57., -1.],
                [-3., 57., -1.],
            ],
        );
        let brace = face(
            0x5680,
            vec![
                [-1., 53., -1.],
                [-1., 53., -5.],
                [-1., 73., -7.],
                [-1., 73., -3.],
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
            let mut out = nose.clone();
            gear_positions(&mut out, gear, 0.);
            assert_eq!(out.positions[2], nose.positions[2]);
            assert_eq!(out.positions[3], nose.positions[3]);
            let mut out = brace.clone();
            gear_positions(&mut out, gear, 0.);
            assert_eq!(out.positions[2], brace.positions[2]);
            assert_eq!(out.positions[3], brace.positions[3]);
            assert!((d2(out.positions[0], out.positions[1]) - 16.).abs() < 1e-4);
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
    fn vertical_rudder_cut_is_fixed_and_all_eighteen_gear_faces_hide() {
        let rig = rig();
        let mut s = state();
        s.rudder = 1.;
        let source = face(
            0x2268,
            vec![
                [-20., -30., 38.],
                [-20., -42., 38.],
                [-20., -40., 5.],
                [-20., -30., 3.888889],
            ],
        );
        let out = rig.animate(&source, &s).unwrap();
        assert_eq!(out.positions[0], source.positions[0]);
        assert_eq!(out.positions[3], source.positions[3]);
        assert_ne!(out.positions[1], source.positions[1]);
        s.gear = 0.;
        for a in GEAR {
            assert!(rig.animate(&face(a, vec![[0.; 3]; 3]), &s).is_none());
        }
    }
}
