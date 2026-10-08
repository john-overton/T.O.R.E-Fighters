//! AC130.SH source skins with explicitly fitted control partitions and rigid gear.
//! No control alias exists beyond gear. Imported instructions are never executed.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use tore_formats::shape::{Face, Shape};
const FIN: [usize; 2] = [0x33ec, 0x3402];
const LEFT_TAIL: [usize; 8] = [
    0x26ce, 0x2764, 0x34a5, 0x34b7, 0x3568, 0x357b, 0x35a2, 0x35b6,
];
const RIGHT_TAIL: [usize; 4] = [0x3cc3, 0x3cd6, 0x3c97, 0x3caa];
const WING: [usize; 12] = [
    0x272c, 0x273f, 0x3744, 0x3768, 0x330e, 0x3321, 0x4787, 0x47af, 0x33b0, 0x33c4, 0x45cc, 0x45e0,
];
const LEFT_GEAR: [usize; 2] = [0x493b, 0x495a];
const RIGHT_GEAR: [usize; 2] = [0x491c, 0x4979];
const NOSE: [usize; 4] = [0x4998, 0x49b7, 0x49d6, 0x49f5];
const EPS: f32 = 1e-4;
pub struct Rig;
fn fin_cut(p: [f32; 3]) -> f32 {
    p[1] + 80.
}
fn tail_cut(p: [f32; 3]) -> f32 {
    p[1] + 78. + 0.05 * (p[0].abs() - 10.)
}
fn wing_cut(p: [f32; 3]) -> f32 {
    p[1] + 16. - (p[0].abs() - 26.) * 10. / 71.
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
        .ok_or_else(|| format!("AC130.SH missing reviewed face {a:x}").into())
}
fn roots(shape: &Shape, ids: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &a in ids {
        if !points
            .iter()
            .all(|p| face(shape, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed AC130.SH attachment {a:x}").into());
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
    if in_span(f, 13., 20.) || in_span(f, 34., 45.) {
        1
    } else if in_span(f, 62., 90.) {
        2
    } else {
        0
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 19666
            || shape.faces.len() != 339
            || shape.state_words != [0x5cc0].into_iter().collect()
        {
            return Err("unreviewed AC130.SH animation layout".into());
        }
        roots(
            &shape,
            &FIN,
            &[
                [0., -89., 6.],
                [0., -62., 6.],
                [0., -77., 40.],
                [0., -82., 40.],
            ],
        )?;
        roots(
            &shape,
            &LEFT_TAIL[..2],
            &[[-10., -68., 4.], [-4., -90., 5.]],
        )?;
        roots(&shape, &RIGHT_TAIL[..2], &[[10., -68., 4.], [4., -90., 5.]])?;
        for a in WING {
            if face(&shape, a)?.positions.iter().any(|p| p[2] != 6.) {
                return Err("unreviewed AC130.SH flat wing skin".into());
            }
        }
        let ids: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let down = Shape::with_state(bytes, &[(0x5cc0, 1)].into())?;
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
            return Err("unreviewed AC130.SH gear branch".into());
        }
        roots(&down, &LEFT_GEAR, &[[-11., -9., -14.], [-11., 8., -14.]])?;
        roots(&down, &RIGHT_GEAR, &[[12., -9., -14.], [12., 8., -14.]])?;
        roots(
            &down,
            &NOSE[..2],
            &[
                [1., 44., -16.],
                [1., 40., -16.],
                [1., 40., -20.],
                [1., 44., -20.],
            ],
        )?;
        roots(
            &down,
            &NOSE[2..],
            &[
                [-2., 44., -16.],
                [-2., 40., -16.],
                [-2., 40., -20.],
                [-2., 44., -20.],
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
                for boundary in [13., 20., 34., 45., 62., 90.] {
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
                [0., -80., 6.],
                [0., 0., 1.],
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
                [side * 10., -78., 4.5],
                [1., -side * 0.05, 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, b) in result.positions.iter_mut().zip(&source.positions) {
                if tail_cut(*b).abs() <= EPS || b[1] == -90. && b[0].abs() == 4. {
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
                    [side * 26., -16., 6.],
                    [1., side * 10. / 71., 0.],
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
    let closing = (1. - deployed.clamp(0., 1.)) as f32;
    let delta = if NOSE.contains(&face.address) {
        [0., 0., 8.]
    } else if LEFT_GEAR.contains(&face.address) {
        [2., 0., 10.]
    } else {
        [-3., 0., 10.]
    };
    // Source wheel/fairing and isolated nose wheel cards have no separate strut
    // or exact body attachment. Recess whole rigid cards, without invented rods.
    for p in &mut face.positions {
        for i in 0..3 {
            p[i] += closing * delta[i];
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
                    return Err("AC130.SH invalid gear endpoint".into());
                }
                if LEFT_GEAR.contains(&source.address) && b[0] > -9. + EPS
                    || RIGHT_GEAR.contains(&source.address) && b[0] < 9. - EPS
                {
                    return Err("AC130.SH main assemblies crossed inward".into());
                }
                for (j, c) in source.positions.iter().enumerate().skip(i + 1) {
                    if (distance(*a, *c) - distance(b, moved.positions[j])).abs() > EPS {
                        return Err("AC130.SH rigid gear changed dimensions".into());
                    }
                }
                if g == 0. {
                    let inside = if NOSE.contains(&source.address) {
                        (-2. - EPS..=1. + EPS).contains(&b[0])
                            && (40. - EPS..=44. + EPS).contains(&b[1])
                            && (-12. - EPS..=-8. + EPS).contains(&b[2])
                    } else {
                        (b[0].abs() - 9.).abs() < EPS
                            && (-9. - EPS..=8. + EPS).contains(&b[1])
                            && (-10. - EPS..=-4. + EPS).contains(&b[2])
                    };
                    if !inside {
                        return Err(
                            "AC130.SH complete gear exceeds fitted body stow envelope".into()
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
            subtype: 0x6c,
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    #[test]
    fn wheel_and_fairing_cards_recess_without_distortion_through_201_poses() {
        let mut faces = Vec::new();
        for (ids, x, y0, y1, z0, z1) in [
            (&LEFT_GEAR[..], -11., -9., 8., -20., -14.),
            (&RIGHT_GEAR[..], 12., -9., 8., -20., -14.),
            (&NOSE[..2], 1., 40., 44., -20., -16.),
            (&NOSE[2..], -2., 40., 44., -20., -16.),
        ] {
            for &a in ids {
                faces.push(panel(
                    a,
                    vec![[x, y0, z0], [x, y1, z0], [x, y1, z1], [x, y0, z1]],
                ));
            }
        }
        let shape = Shape {
            faces,
            lines: Vec::new(),
            state_words: BTreeSet::new(),
        };
        validate_gear(&shape).unwrap();
        for i in 0..=200 {
            let g = f64::from(i) / 200.;
            for source in &shape.faces {
                let mut moved = source.clone();
                gear_pose(&mut moved, g);
                assert_eq!(source.uv, moved.uv);
                for (j, p) in source.positions.iter().enumerate() {
                    assert!(moved.positions[j][2] >= p[2]);
                    assert!(moved.positions[j][0].abs() <= p[0].abs() + EPS);
                    for (k, q) in source.positions.iter().enumerate().skip(j + 1) {
                        assert!(
                            (distance(*p, *q) - distance(moved.positions[j], moved.positions[k]))
                                .abs()
                                < EPS
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn separate_flap_and_roll_panels_do_not_move_nacelle_corridors() {
        for side in [-1., 1.] {
            for role in [1, 2] {
                let (lo, hi) = if role == 1 { (34., 45.) } else { (62., 90.) };
                let hinge = |x: f32| [side * x, -16. + (x - 26.) * 10. / 71., 6.];
                let a = hinge(lo);
                let b = hinge(hi);
                let source = panel(
                    0x33b0,
                    vec![a, b, [b[0], b[1] - 5., 6.], [a[0], a[1] - 5., 6.]],
                );
                let mut s = state();
                s.aileron = 1.;
                s.flaps = 0.;
                let rolled = Rig.animate(&source, &s).unwrap();
                assert_eq!(rolled.positions[..2], source.positions[..2]);
                if role == 1 {
                    assert_eq!(rolled.positions, source.positions);
                } else {
                    assert!((rolled.positions[2][2] - 6.) * side > 0.);
                }
                s.aileron = 0.;
                s.flaps = 1.;
                let flapped = Rig.animate(&source, &s).unwrap();
                if role == 2 {
                    assert_eq!(flapped.positions, source.positions);
                } else {
                    assert!(flapped.positions[2][2] < 6.);
                }
            }
        }
        let mut s = state();
        s.aileron = 1.;
        s.flaps = 1.;
        for x in [26., 52.] {
            let f = panel(
                0x33b0,
                vec![
                    [x, -25., 6.],
                    [x + 1., -25., 6.],
                    [x + 1., -20., 6.],
                    [x, -20., 6.],
                ],
            );
            assert_eq!(Rig.animate(&f, &s).unwrap().positions, f.positions);
        }
    }
    #[test]
    fn gun_and_propeller_source_groups_are_left_for_shared_overlays() {
        let mut s = state();
        s.aileron = 1.;
        s.flaps = 1.;
        s.elevator = 1.;
        s.rudder = 1.;
        for a in [
            0x2850, 0x286b, 0x2886, 0x28a1, 0x2439, 0x24e7, 0x2502, 0x2542, 0x255d, 0x2578, 0x2b10,
            0x2e1d, 0x3f7a, 0x4287,
        ] {
            let f = panel(a, vec![[1., -20., 5.], [2., -20., 5.], [2., -21., 4.]]);
            assert_eq!(Rig.animate(&f, &s).unwrap().positions, f.positions);
        }
    }
    #[test]
    fn rudder_and_tail_zero_and_fixed_attachment_points_remain_exact() {
        let rudder = panel(
            0x33ec,
            vec![
                [0., -80., 6.],
                [0., -89., 6.],
                [0., -85., 38.],
                [0., -80., 40.],
            ],
        );
        for value in [-1., 0., 1.] {
            let mut s = state();
            s.rudder = value;
            let moved = Rig.animate(&rudder, &s).unwrap();
            assert_eq!(moved.positions[0], rudder.positions[0]);
            assert_eq!(moved.positions[3], rudder.positions[3]);
            if value != 0. {
                assert!(moved.positions[1][0] * value as f32 > 0.);
            } else {
                assert_eq!(moved.positions, rudder.positions);
            }
            s.elevator = value;
            for (a, side) in [(0x35a2, -1.), (0x3cc3, 1.)] {
                let f = panel(
                    a,
                    vec![
                        [side * 4., -90., 5.],
                        [side * 40., -84., 5.],
                        [side * 40., -79.5, 5.],
                        [side * 10., -78., 4.5],
                    ],
                );
                let q = Rig.animate(&f, &s).unwrap();
                assert_eq!(q.positions[0], f.positions[0]);
                assert_eq!(q.positions[2..], f.positions[2..]);
                if value != 0. {
                    assert!((q.positions[1][2] - 5.) * value as f32 > 0.);
                }
            }
        }
    }
}
