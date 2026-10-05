//! C130.SH source skins with explicitly fitted control partitions and rigid gear.
//! No control alias exists beyond gear. Imported instructions are never executed.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use tore_formats::shape::{Face, Shape};
const FIN: [usize; 2] = [0x201a, 0x21f2];
const LEFT_TAIL: [usize; 2] = [0x2497, 0x24b2];
const RIGHT_TAIL: [usize; 2] = [0x2757, 0x2772];
const WING: [usize; 12] = [
    0x1cb3, 0x1ccb, 0x1ce3, 0x1cfb, 0x21a1, 0x21bc, 0x2525, 0x259a, 0x25b2, 0x27b6, 0x2842, 0x2860,
];
const LEFT_GEAR: [usize; 2] = [0x2909, 0x292c];
const RIGHT_GEAR: [usize; 2] = [0x294f, 0x2972];
const NOSE: [usize; 2] = [0x2995, 0x29b0];
const EPS: f32 = 1e-4;
pub struct Rig;
fn fin_cut(p: [f32; 3]) -> f32 {
    p[1] + 67. - (p[2] - 11.) * 6. / 41.
}
fn tail_cut(p: [f32; 3]) -> f32 {
    p[1] + 64. + 0.1 * (p[0].abs() - 8.)
}
fn wing_cut(p: [f32; 3]) -> f32 {
    p[1] + 8. - (p[0].abs() - 28.) * 9. / 70.
}
fn tail(a: usize) -> bool {
    LEFT_TAIL.contains(&a) || RIGHT_TAIL.contains(&a)
}
fn gear(a: usize) -> bool {
    LEFT_GEAR.contains(&a) || RIGHT_GEAR.contains(&a) || NOSE.contains(&a)
}
fn split(f: &Face, d: impl Fn([f32; 3]) -> f32) -> Vec<Face> {
    crate::aircraft_animation::split_surface(f, [0.; 3], [1., 0., 0.], 0., d)
        .into_iter()
        .filter(|f| normal(&f.positions).is_some())
        .collect()
}
fn face(shape: &Shape, a: usize) -> AppResult<&Face> {
    shape
        .faces
        .iter()
        .find(|f| f.address == a)
        .ok_or_else(|| format!("C130.SH missing reviewed face {a:x}").into())
}
fn roots(shape: &Shape, ids: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &a in ids {
        if !points
            .iter()
            .all(|p| face(shape, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed C130.SH attachment {a:x}").into());
        }
    }
    Ok(())
}
fn in_span(f: &Face, low: f32, high: f32) -> bool {
    f.positions
        .iter()
        .all(|p| (low - EPS..=high + EPS).contains(&p[0].abs()))
}
fn wing_role(f: &Face) -> u8 {
    if !WING.contains(&f.address) || f.positions.iter().any(|p| wing_cut(*p) > EPS) {
        return 0;
    }
    if in_span(f, 14., 21.) || in_span(f, 33., 46.) {
        1
    } else if in_span(f, 60., 92.) {
        2
    } else {
        0
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 10812
            || shape.faces.len() != 144
            || shape.state_words != [0x3a30].into_iter().collect()
        {
            return Err("unreviewed C130.SH animation layout".into());
        }
        roots(
            &shape,
            &FIN,
            &[
                [0., -76., 11.],
                [0., -45., 12.],
                [0., -58., 52.],
                [0., -64., 52.],
            ],
        )?;
        roots(&shape, &LEFT_TAIL, &[[-8., -56., 9.], [-5., -74., 9.]])?;
        roots(&shape, &RIGHT_TAIL, &[[8., -56., 9.], [6., -74., 9.]])?;
        for a in WING {
            if face(&shape, a)?.positions.iter().any(|p| p[2] != 9.) {
                return Err("unreviewed C130.SH flat wing skin".into());
            }
        }
        let ids: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let down = Shape::with_state(bytes, &[(0x3a30, 1)].into())?;
        let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
        let expected: BTreeSet<_> = LEFT_GEAR
            .iter()
            .chain(&RIGHT_GEAR)
            .chain(&NOSE)
            .copied()
            .collect();
        if !ids.is_subset(&active)
            || active.difference(&ids).copied().collect::<BTreeSet<_>>() != expected
        {
            return Err("unreviewed C130.SH gear branch".into());
        }
        roots(&down, &LEFT_GEAR, &[[-8., -7., -12.], [-8., 5., -12.]])?;
        roots(&down, &RIGHT_GEAR, &[[9., -7., -12.], [9., 5., -12.]])?;
        roots(
            &down,
            &NOSE,
            &[
                [0., 49., -13.],
                [0., 42., -13.],
                [0., 42., -21.],
                [0., 49., -21.],
            ],
        )?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if FIN.contains(&f.address) {
                faces.extend(split(&f, fin_cut));
            } else if tail(f.address) {
                faces.extend(split(&f, tail_cut));
            } else if WING.contains(&f.address) {
                let mut pieces = vec![f];
                for boundary in [14., 21., 33., 46., 60., 92.] {
                    pieces = pieces
                        .into_iter()
                        .flat_map(|f| split(&f, |p| p[0].abs() - boundary))
                        .collect();
                }
                faces.extend(pieces.into_iter().flat_map(|f| split(&f, wing_cut)));
            } else {
                faces.push(f);
            }
        }
        faces.extend(down.faces.into_iter().filter(|f| gear(f.address)));
        shape.faces = faces;
        validate_gear(&shape)?;
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let a = source.address;
        if gear(a) {
            if state.gear <= 0. {
                return None;
            }
            gear_pose(&mut result, state.gear);
            return Some(result);
        }
        if FIN.contains(&a)
            && source.positions.iter().all(|p| fin_cut(*p) <= EPS)
            && state.rudder != 0.
        {
            turn(
                &mut result,
                [0., -67., 11.],
                [0., 6., 41.],
                0.35 * state.rudder.clamp(-1., 1.),
            );
            for (p, b) in result.positions.iter_mut().zip(&source.positions) {
                if fin_cut(*b).abs() <= EPS {
                    *p = *b;
                }
            }
        } else if tail(a)
            && source.positions.iter().all(|p| tail_cut(*p) <= EPS)
            && state.elevator != 0.
        {
            let side = if LEFT_TAIL.contains(&a) { -1. } else { 1. };
            turn(
                &mut result,
                [side * 8., -64., 9.],
                [1., -side * 0.1, 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, b) in result.positions.iter_mut().zip(&source.positions) {
                if tail_cut(*b).abs() <= EPS || b[1] == -74. && b[0].abs() < 8. {
                    *p = *b;
                }
            }
        } else {
            let role = wing_role(source);
            let side = if source.positions[0][0] < 0. { -1. } else { 1. };
            let angle = match role {
                1 => 0.45 * state.flaps.clamp(0., 1.),
                2 => -0.20 * f64::from(side) * state.aileron.clamp(-1., 1.),
                _ => 0.,
            };
            if angle != 0. {
                turn(
                    &mut result,
                    [side * 28., -8., 9.],
                    [1., side * 9. / 70., 0.],
                    angle,
                );
                for (p, b) in result.positions.iter_mut().zip(&source.positions) {
                    if wing_cut(*b).abs() <= EPS {
                        *p = *b;
                    }
                }
            }
        }
        if result.positions != source.positions {
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
fn gear_pose(face: &mut Face, deployed: f64) {
    let closing = 1. - deployed.clamp(0., 1.);
    if closing == 0. {
        return;
    }
    let original = face.positions.clone();
    let (pivot, axis, angle) = if LEFT_GEAR.contains(&face.address) {
        ([-8., -1., -12.], [0., 1., 0.], 150f64.to_radians())
    } else if RIGHT_GEAR.contains(&face.address) {
        ([9., -1., -12.], [0., 1., 0.], -150f64.to_radians())
    } else {
        ([0., 49., -13.], [1., 0., 0.], -std::f64::consts::FRAC_PI_2)
    };
    turn(face, pivot, axis, angle * closing);
    for (p, b) in face.positions.iter_mut().zip(original) {
        if if NOSE.contains(&face.address) {
            b == [0., 49., -13.]
        } else {
            b[2] == -12.
        } {
            *p = b;
        }
    }
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for sample in 0..=20 {
        let g = f64::from(sample) / 20.;
        for source in shape.faces.iter().filter(|f| gear(f.address)) {
            let mut moved = source.clone();
            gear_pose(&mut moved, g);
            for (i, a) in source.positions.iter().enumerate() {
                let b = moved.positions[i];
                if !b.iter().all(|v| v.is_finite()) || g == 1. && *a != b {
                    return Err("C130.SH invalid gear endpoint".into());
                }
                if LEFT_GEAR.contains(&source.address) && b[0] > -8. + EPS
                    || RIGHT_GEAR.contains(&source.address) && b[0] < 9. - EPS
                {
                    return Err("C130.SH main assemblies crossed inward".into());
                }
                for (j, c) in source.positions.iter().enumerate().skip(i + 1) {
                    if (distance(*a, *c) - distance(b, moved.positions[j])).abs() > EPS {
                        return Err("C130.SH rigid gear changed dimensions".into());
                    }
                }
                if g == 0. {
                    let inside = if NOSE.contains(&source.address) {
                        b[0].abs() < EPS
                            && (41. - EPS..=49. + EPS).contains(&b[1])
                            && (-13. - EPS..=-6. + EPS).contains(&b[2])
                    } else {
                        (-12. - EPS..=-2.7 + EPS).contains(&b[2])
                            && (-7. - EPS..=5. + EPS).contains(&b[1])
                            && if LEFT_GEAR.contains(&source.address) {
                                (-9.91..=-8. + EPS).contains(&b[0])
                            } else {
                                (9. - EPS..=10.91).contains(&b[0])
                            }
                    };
                    if !inside {
                        return Err(
                            "C130.SH complete gear exceeds fitted body stow envelope".into()
                        );
                    }
                }
            }
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
            subtype: 0xed,
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    #[test]
    fn complete_gear_cards_are_rigid_attached_and_separated_through_201_poses() {
        let source = vec![
            panel(
                0x2909,
                vec![
                    [-11., -7., -21.],
                    [-11., 5., -21.],
                    [-8., 5., -12.],
                    [-8., -7., -12.],
                ],
            ),
            panel(
                0x294f,
                vec![
                    [12., 5., -21.],
                    [9., 5., -12.],
                    [9., -7., -12.],
                    [12., -7., -21.],
                ],
            ),
            panel(
                0x2995,
                vec![
                    [0., 49., -13.],
                    [0., 42., -13.],
                    [0., 42., -21.],
                    [0., 49., -21.],
                ],
            ),
        ];
        let shape = Shape {
            faces: source.clone(),
            lines: Vec::new(),
            state_words: BTreeSet::new(),
        };
        validate_gear(&shape).unwrap();
        for i in 0..=200 {
            let g = f64::from(i) / 200.;
            for before in &source {
                let mut after = before.clone();
                gear_pose(&mut after, g);
                assert_eq!(after.uv, before.uv);
                for (a, p) in before.positions.iter().enumerate() {
                    if if NOSE.contains(&before.address) {
                        *p == [0., 49., -13.]
                    } else {
                        p[2] == -12.
                    } {
                        assert_eq!(*p, after.positions[a]);
                    }
                    for (b, q) in before.positions.iter().enumerate().skip(a + 1) {
                        assert!(
                            (distance(*p, *q) - distance(after.positions[a], after.positions[b]))
                                .abs()
                                < EPS
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn separate_flaps_and_ailerons_keep_their_own_fixed_edges() {
        for side in [-1., 1.] {
            for role in [1, 2] {
                let (lo, hi) = if role == 1 { (33., 46.) } else { (60., 92.) };
                let hinge = |x: f32| [side * x, -8. + (x - 28.) * 9. / 70., 9.];
                let a = hinge(lo);
                let b = hinge(hi);
                let source = panel(
                    0x2525,
                    vec![a, b, [b[0], b[1] - 4., 9.], [a[0], a[1] - 4., 9.]],
                );
                for value in [-1., 0., 1.] {
                    let mut s = state();
                    s.aileron = value;
                    s.flaps = 0.;
                    let moved = Rig.animate(&source, &s).unwrap();
                    assert_eq!(moved.positions[..2], source.positions[..2]);
                    if role == 1 {
                        assert_eq!(moved.positions, source.positions);
                    } else if value != 0. {
                        assert!((moved.positions[2][2] - 9.) * side * value as f32 > 0.);
                    }
                    s.aileron = 0.;
                    s.flaps = value.abs();
                    let flapped = Rig.animate(&source, &s).unwrap();
                    if role == 2 || value == 0. {
                        assert_eq!(flapped.positions, source.positions);
                    } else {
                        assert!(flapped.positions[2][2] < 9.);
                    }
                }
            }
        }
    }
    #[test]
    fn nacelle_corridors_and_unrelated_propeller_faces_remain_static() {
        let mut s = state();
        s.flaps = 1.;
        s.aileron = 1.;
        s.elevator = 1.;
        s.rudder = 1.;
        for (a, x) in [(0x2525, 28.), (0x2525, 53.), (0x257b, 52.)] {
            let source = panel(
                a,
                vec![
                    [x, -14., 9.],
                    [x + 1., -14., 9.],
                    [x + 1., -9., 9.],
                    [x, -9., 9.],
                ],
            );
            assert_eq!(
                Rig.animate(&source, &s).unwrap().positions,
                source.positions
            );
        }
    }
    #[test]
    fn fitted_tail_preserves_actual_root_and_signed_pitch() {
        for (a, side, root) in [(0x2497, -1., 5.), (0x2757, 1., 6.)] {
            let source = panel(
                a,
                vec![
                    [side * root, -74., 9.],
                    [side * 32., -70., 9.],
                    [side * 32., -66.4, 9.],
                    [side * 8., -64., 9.],
                ],
            );
            for value in [-1., 0., 1.] {
                let mut s = state();
                s.elevator = value;
                let moved = Rig.animate(&source, &s).unwrap();
                assert_eq!(moved.positions[0], source.positions[0]);
                assert_eq!(moved.positions[2..], source.positions[2..]);
                if value != 0. {
                    assert!((moved.positions[1][2] - 9.) * value as f32 > 0.);
                } else {
                    assert_eq!(moved.positions, source.positions);
                }
            }
        }
    }
}
