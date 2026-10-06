//! Yak141 source surfaces and original nozzle shell with explicit fitted laws.
//! No imported instructions execute; source endpoints and memberships are guarded.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::FRAC_PI_2,
};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 3] = [0x6270, 0x627c, 0x6282];
const FIN: [usize; 4] = [0x42b7, 0x430a, 0x4342, 0x4366];
const TAIL_L: [usize; 3] = [0x4b07, 0x4b4f, 0x4b64];
const TAIL_R: [usize; 2] = [0x484c, 0x4894];
const ROLL_L: [usize; 3] = [0x4945, 0x4a67, 0x4ad9];
const ROLL_R: [usize; 3] = [0x453a, 0x462f, 0x4698];
const SWITCHED_GEAR: [usize; 4] = [0x4ba9, 0x4bc8, 0x4c14, 0x4c33];
const NOSE: [usize; 2] = [0x4c80, 0x4c9f];
const SHELL: [usize; 8] = [
    0x2401, 0x241e, 0x244f, 0x246c, 0x24b2, 0x2b4f, 0x355a, 0x3592,
];
const OUTLET: usize = 0x2489;
const NOZZLE_PIVOT: [f32; 3] = [0.5, -47., -3.];
struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Vec<[f32; 3]>,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("Y141.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("Y141.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed Y141.SH root{address:x}").into());
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
        return Err(format!("unreviewed Y141.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn rudder_distance(p: [f32; 3]) -> f32 {
    p[1] + 74. + (5. / 21.) * (p[2] - 6.)
}
fn fin_x(address: usize) -> f32 {
    if address < 0x4342 { 12. } else { -10. }
}
fn roll_spec(address: usize) -> Option<(f32, [f32; 3], [f32; 3])> {
    if ROLL_L.contains(&address) {
        Some((29., [-29., -28.5, 3.], [-25., -6., 0.]))
    } else if ROLL_R.contains(&address) {
        Some((30., [30., -28.5, 3.], [25., -6., 0.]))
    } else {
        None
    }
}
fn roll_distance(p: [f32; 3], inner: f32, _: [f32; 3]) -> f32 {
    p[1] + 28.5 + (p[0].abs() - inner) * 6. / 25.
}
fn gear_cut(address: usize) -> f32 {
    if NOSE.contains(&address) {
        -14.
    } else if address < 0x4c00 {
        -13.
    } else {
        -12.
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 21134
            || shape.faces.len() != 308
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed Y141.SH layout".into());
        }
        roots(
            &shape,
            &[0x4b07, 0x4b4f],
            &[[-12., -57., -1.], [-13., -82., -1.]],
        )?;
        roots(&shape, &[0x4b64], &[[-12., -57., -1.]])?;
        roots(&shape, &TAIL_R, &[[14., -57., -1.], [14., -82., -1.]])?;
        for &a in &FIN {
            let x = fin_x(a);
            roots(
                &shape,
                &[a],
                &[[x, -79., 6.], [x, -82., 27.], [x, -71., 27.], [x, -56., 6.]],
            )?;
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        for (word, bindings) in [
            (
                0x627c,
                [
                    (0x4dfe, 0x4dc8, [0, 3, 2, 1]),
                    (0x4e25, 0x4da9, [1, 0, 3, 2]),
                ],
            ),
            (
                0x6282,
                [
                    (0x4d3b, 0x4ce6, [0, 1, 2, 3]),
                    (0x4d62, 0x4cfd, [1, 2, 3, 0]),
                ],
            ),
        ] {
            let added: Vec<_> = bindings.iter().map(|b| b.1).collect();
            let removed: Vec<_> = bindings.iter().map(|b| b.0).collect();
            let pose = branch(bytes, &original, word, -1, &added, &removed)?;
            branch(bytes, &original, word, 1, &[], &removed)?;
            for (address, target, order) in bindings {
                let base = one(&shape, address)?;
                let down = one(&pose, target)?;
                if base.positions.len() != 4 || down.positions.len() != 4 {
                    return Err("unreviewed Y141.SH flap topology".into());
                }
                let deployed: Vec<_> = order.into_iter().map(|i| down.positions[i]).collect();
                let forward = if word == 0x627c {
                    [[-13., -26., 3.], [-29., -26., 3.]]
                } else {
                    [[14., -26., 3.], [30., -26., 3.]]
                };
                if !forward.iter().all(|p| base.positions.contains(p))
                    || base
                        .positions
                        .iter()
                        .zip(&deployed)
                        .any(|(a, b)| forward.contains(a) && a != b)
                {
                    return Err("unreviewed Y141.SH fixed flap edge".into());
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
        let gear = branch(bytes, &original, 0x6270, 1, &SWITCHED_GEAR, &[])?;
        roots(&gear, &[0x4ba9, 0x4bc8], &[[9., -28., -9.], [9., -6., -9.]])?;
        roots(
            &gear,
            &[0x4c14, 0x4c33],
            &[[-9., -28., -8.], [-9., -6., -8.]],
        )?;
        roots(&shape, &NOSE, &[[0., 53., -10.], [0., 63., -10.]])?;
        for a in SHELL.into_iter().chain([OUTLET]) {
            let f = one(&shape, a)?;
            if f.texture != "_Y141.PIC"
                || f.positions.iter().any(|p| !(p[1] == -47. || p[1] == -53.))
            {
                return Err("unreviewed Y141.SH original nozzle rings".into());
            }
        }
        shape.faces.extend(
            gear.faces
                .into_iter()
                .filter(|f| SWITCHED_GEAR.contains(&f.address)),
        );
        let mut faces = Vec::new();
        for source in shape.faces {
            if FIN.contains(&source.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &source,
                    [fin_x(source.address), -74., 6.],
                    [0., -5., 21.],
                    0.,
                    rudder_distance,
                ));
            } else if let Some((inner, pivot, axis)) = roll_spec(source.address) {
                for span in crate::aircraft_animation::split_surface(
                    &source,
                    [0.; 3],
                    [1., 0., 0.],
                    0.,
                    |p| p[0].abs() - inner,
                ) {
                    if span.positions.iter().all(|p| p[0].abs() >= inner - 1e-4) {
                        faces.extend(crate::aircraft_animation::split_surface(
                            &span,
                            pivot,
                            axis.map(f64::from),
                            0.,
                            |p| roll_distance(p, inner, axis),
                        ));
                    } else {
                        faces.push(span);
                    }
                }
            } else if NOSE.contains(&source.address) || SWITCHED_GEAR.contains(&source.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &source,
                    [0.; 3],
                    [1., 0., 0.],
                    0.,
                    |p| p[2] - gear_cut(source.address),
                ));
            } else {
                faces.push(source);
            }
        }
        shape.faces = faces;
        validate_gear(&shape)?;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let a = source.address;
        if let Some(m) = self.flaps.get(&a) {
            let t = state.flaps.clamp(0., 1.) as f32;
            result.positions = m
                .neutral
                .iter()
                .zip(&m.deployed)
                .map(|(p, q)| std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t))
                .collect();
            update_normal(source, &mut result);
        } else if FIN.contains(&a) && source.positions.iter().all(|p| rudder_distance(*p) <= 1e-4) {
            turn(
                &mut result,
                [fin_x(a), -74., 6.],
                [0., -5., 21.],
                0.35 * state.rudder.clamp(-1., 1.),
            );
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if rudder_distance(*old).abs() < 1e-4 {
                    *p = *old;
                }
            }
            update_normal(source, &mut result);
        } else if TAIL_L.contains(&a) || TAIL_R.contains(&a) {
            let left = TAIL_L.contains(&a);
            let mut moved = source.clone();
            turn(
                &mut moved,
                [if left { -12.5 } else { 14. }, -69.5, -1.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() > if left { 13. } else { 14. } {
                    *p = q;
                }
            }
            update_normal(source, &mut result);
        } else if let Some((inner, pivot, axis)) = roll_spec(a) {
            if source
                .positions
                .iter()
                .all(|p| p[0].abs() >= inner - 1e-4 && roll_distance(*p, inner, axis) <= 1e-4)
            {
                turn(
                    &mut result,
                    pivot,
                    axis,
                    -0.20 * state.aileron.clamp(-1., 1.),
                );
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if roll_distance(*old, inner, axis).abs() < 1e-4 {
                        *p = *old;
                    }
                }
                update_normal(source, &mut result);
            }
        } else if SHELL.contains(&a) || a == OUTLET {
            turn(
                &mut result,
                NOZZLE_PIVOT,
                [0., 0., 1.],
                -15f64.to_radians() * state.lift_controls.vector_yaw_actual.clamp(-1., 1.),
            );
            turn(
                &mut result,
                NOZZLE_PIVOT,
                [1., 0., 0.],
                FRAC_PI_2 * state.lift_controls.vector_pitch_actual.clamp(0., 1.),
            );
            if a != OUTLET {
                for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                    if old[1] == -47. {
                        *p = *old;
                    }
                }
            }
            update_normal(source, &mut result);
        } else if SWITCHED_GEAR.contains(&a) || NOSE.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear);
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
fn gear_positions(face: &mut Face, gear: f64) {
    let closing = (1. - gear.clamp(0., 1.)) as f32;
    let second = (2. * closing - 1.).clamp(0., 1.);
    let a = face.address;
    let (root, delta) = if NOSE.contains(&a) {
        (-10., [0., -6. * second, 15. * closing])
    } else if a < 0x4c00 {
        (-9., [-2. * second, 0., 16. * closing])
    } else {
        (-8., [2. * second, 0., 16. * closing])
    };
    let cut = gear_cut(a);
    for p in &mut face.positions {
        let weight = ((root - p[2]) / (root - cut)).clamp(0., 1.);
        for i in 0..3 {
            p[i] += delta[i] * weight;
        }
    }
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=20 {
        for source in shape
            .faces
            .iter()
            .filter(|f| SWITCHED_GEAR.contains(&f.address) || NOSE.contains(&f.address))
        {
            let mut result = source.clone();
            gear_positions(&mut result, f64::from(step) / 20.);
            if result.positions.iter().flatten().any(|v| !v.is_finite()) {
                return Err("nonfinite Y141.SH gear".into());
            }
            if source
                .positions
                .iter()
                .all(|p| p[2] <= gear_cut(source.address) + 1e-4)
            {
                for i in 0..source.positions.len() {
                    for j in i + 1..source.positions.len() {
                        if (distance2(source.positions[i], source.positions[j])
                            - distance2(result.positions[i], result.positions[j]))
                        .abs()
                            > 1e-3
                        {
                            return Err("Y141.SH wheel not rigid".into());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
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
pub(crate) fn flame(_: usize) -> bool {
    false
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
    #[test]
    fn nozzle_front_ring_is_fixed_and_original_outlet_stays_rigid_at_25_poses() {
        let mut state = state();
        let rig = rig();
        let outlet = face(
            OUTLET,
            vec![
                [1., -53., -8.],
                [-3., -53., -7.],
                [-4., -53., -3.],
                [-3., -53., 0.],
                [1., -53., 2.],
                [4., -53., 0.],
                [5., -53., -3.],
                [4., -53., -7.],
            ],
        );
        let shell = face(
            0x2401,
            vec![
                [-3., -53., 0.],
                [-3., -47., 0.],
                [1., -47., 2.],
                [1., -53., 2.],
            ],
        );
        for pitch in [0., 0.25, 0.5, 0.75, 1.] {
            for yaw in [-1., -0.5, 0., 0.5, 1.] {
                state.lift_controls.vector_pitch_actual = pitch;
                state.lift_controls.vector_yaw_actual = yaw;
                let out = rig.animate(&outlet, &state).unwrap();
                let sh = rig.animate(&shell, &state).unwrap();
                assert_eq!(out.uv, outlet.uv);
                for i in 0..outlet.positions.len() {
                    for j in i + 1..outlet.positions.len() {
                        assert!(
                            (distance2(outlet.positions[i], outlet.positions[j])
                                - distance2(out.positions[i], out.positions[j]))
                            .abs()
                                < 1e-3
                        );
                    }
                }
                for (p, q) in shell.positions.iter().zip(&sh.positions) {
                    if p[1] == -47. {
                        assert_eq!(p, q);
                    } else {
                        let i = outlet.positions.iter().position(|v| v == p).unwrap();
                        assert_eq!(*q, out.positions[i]);
                    }
                }
                if pitch == 0. && yaw == 0. {
                    assert_eq!(out.positions, outlet.positions);
                    assert_eq!(sh.positions, shell.positions);
                }
            }
        }
    }
    #[test]
    fn asymmetric_tail_roots_are_fixed_and_signed_pitch_is_coherent() {
        let rig = rig();
        let mut s = state();
        for (address, points, roots) in [
            (
                0x484c,
                vec![
                    [33., -75., -1.],
                    [33., -85., -1.],
                    [14., -82., -1.],
                    [14., -57., -1.],
                ],
                vec![[14., -82., -1.], [14., -57., -1.]],
            ),
            (
                0x4b07,
                vec![
                    [-13., -82., -1.],
                    [-32., -85., -1.],
                    [-32., -75., -1.],
                    [-12., -57., -1.],
                ],
                vec![[-13., -82., -1.], [-12., -57., -1.]],
            ),
        ] {
            let source = face(address, points);
            for value in [-1., -0.5, 0., 0.5, 1.] {
                s.elevator = value;
                let out = rig.animate(&source, &s).unwrap();
                for (p, q) in source.positions.iter().zip(&out.positions) {
                    if roots.contains(p) {
                        assert_eq!(p, q);
                    } else if value != 0. {
                        assert!((q[2] - p[2]) * value as f32 > 0.);
                    } else {
                        assert_eq!(p, q);
                    }
                }
            }
        }
    }
    #[test]
    fn both_fin_cuts_keep_attachment_points_fixed() {
        let rig = rig();
        let mut s = state();
        for a in [0x42b7, 0x4342] {
            let x = fin_x(a);
            let source = face(
                a,
                vec![[x, -79., 6.], [x, -82., 27.], [x, -71., 27.], [x, -56., 6.]],
            );
            let pieces = crate::aircraft_animation::split_surface(
                &source,
                [x, -74., 6.],
                [0., -5., 21.],
                0.,
                rudder_distance,
            );
            assert_eq!(pieces.len(), 2);
            for value in [-1., 0., 1.] {
                s.rudder = value;
                for part in &pieces {
                    let out = rig.animate(part, &s).unwrap();
                    for (p, q) in part.positions.iter().zip(out.positions) {
                        if rudder_distance(*p).abs() < 1e-4 {
                            assert_eq!(*p, q);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn all_six_gear_faces_hide_and_lower_wheels_remain_rigid_at_21_samples() {
        let rig = rig();
        let mut s = state();
        s.gear = 0.;
        for a in SWITCHED_GEAR.into_iter().chain(NOSE) {
            assert!(rig.animate(&face(a, vec![[0.; 3]; 3]), &s).is_none());
        }
        for (a, points) in [
            (
                0x4ba9,
                vec![
                    [9., -28., -9.],
                    [9., -6., -9.],
                    [14., -6., -21.],
                    [14., -28., -21.],
                ],
            ),
            (
                0x4c14,
                vec![
                    [-9., -6., -8.],
                    [-13., -6., -20.],
                    [-13., -28., -20.],
                    [-9., -28., -8.],
                ],
            ),
            (
                0x4c80,
                vec![
                    [0., 53., -10.],
                    [0., 63., -10.],
                    [0., 63., -21.],
                    [0., 53., -21.],
                ],
            ),
        ] {
            let source = face(a, points);
            let cut = gear_cut(a);
            let pieces =
                crate::aircraft_animation::split_surface(&source, [0.; 3], [1., 0., 0.], 0., |p| {
                    p[2] - cut
                });
            assert_eq!(pieces.len(), 2);
            for step in 0..=20 {
                let mut moved = Vec::new();
                for piece in &pieces {
                    let mut out = piece.clone();
                    gear_positions(&mut out, f64::from(step) / 20.);
                    if piece.positions.iter().all(|p| p[2] <= cut + 1e-4) {
                        for i in 0..piece.positions.len() {
                            for j in i + 1..piece.positions.len() {
                                assert!(
                                    (distance2(piece.positions[i], piece.positions[j])
                                        - distance2(out.positions[i], out.positions[j]))
                                    .abs()
                                        < 1e-3
                                );
                            }
                        }
                    }
                    moved.push(out);
                }
                for p in &pieces[0].positions {
                    if (p[2] - cut).abs() < 1e-4 {
                        let i = pieces[0].positions.iter().position(|q| q == p).unwrap();
                        let j = pieces[1].positions.iter().position(|q| q == p).unwrap();
                        assert_eq!(moved[0].positions[i], moved[1].positions[j]);
                    }
                }
            }
        }
    }
}
