//! AV8 source surfaces and four original nozzle cards with explicit fitted laws.
//! No imported instructions execute; source endpoints and memberships are guarded.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::FRAC_PI_2,
};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 3] = [0x5640, 0x564c, 0x5652];
const FIN: [usize; 2] = [0x3288, 0x329b];
const TAIL_L: [usize; 2] = [0x300d, 0x3021];
const TAIL_R: [usize; 2] = [0x2a4d, 0x2ac6];
const ROLL_L: [usize; 3] = [0x1bc7, 0x1bdf, 0x1bf3];
const ROLL_R: [usize; 3] = [0x1d35, 0x1d54, 0x1d9c];
const SWITCHED_GEAR: [usize; 8] = [
    0x4162, 0x4181, 0x41a0, 0x41bf, 0x421d, 0x423c, 0x425b, 0x427a,
];
const CENTRAL: [usize; 8] = [
    0x42f6, 0x4315, 0x4334, 0x4353, 0x4372, 0x4391, 0x43dd, 0x43fc,
];
const NOSE: [usize; 2] = [0x43dd, 0x43fc];
const NOSE_CUT: f32 = -13.;
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
        .ok_or_else(|| format!("AV8.SH missing face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("AV8.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| one(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed AV8.SH root{address:x}").into());
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
        return Err(format!("unreviewed AV8.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
fn rudder_distance(p: [f32; 3]) -> f32 {
    p[1] + 73. + 0.20 * (p[2] - 5.)
}
fn roll_spec(address: usize) -> Option<(f32, [f32; 3], [f32; 3])> {
    if ROLL_L.contains(&address) {
        Some((27., [-27., -33., 0.], [-22., 3., -3.]))
    } else if ROLL_R.contains(&address) {
        Some((28., [28., -33., 1.], [21., 3., -4.]))
    } else {
        None
    }
}
fn roll_distance(p: [f32; 3], inner: f32, axis: [f32; 3]) -> f32 {
    p[1] + 33. - (p[0].abs() - inner) * 3. / axis[0].abs()
}
fn nozzle(address: usize) -> Option<[f32; 3]> {
    match address {
        0x256c => Some([8., -14.5, -3.5]),
        0x26ac | 0x2c17 => Some([-8., -14.5, -3.5]),
        0x2745 => Some([11., 3., -2.5]),
        0x2ca8 => Some([-11., 3., -2.5]),
        _ => None,
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 18014
            || shape.faces.len() != 270
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed AV8.SH layout".into());
        }
        roots(&shape, &TAIL_L, &[[-3., -62., 2.], [-1., -79., 2.]])?;
        roots(&shape, &TAIL_R, &[[3., -62., 2.], [1., -79., 2.]])?;
        roots(
            &shape,
            &FIN,
            &[
                [0., -81., 25.],
                [0., -76., 23.],
                [0., -61., 10.],
                [0., -79., 5.],
            ],
        )?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        for (word, bindings) in [
            (
                0x564c,
                [
                    (0x40e7, 0x4092, [0, 3, 2, 1]),
                    (0x40fe, 0x40b1, [1, 0, 3, 2]),
                ],
            ),
            (
                0x5652,
                [
                    (0x3fe8, 0x3f93, [1, 0, 3, 2]),
                    (0x3fff, 0x3fb2, [0, 3, 2, 1]),
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
                    return Err("unreviewed AV8.SH flap topology".into());
                }
                let deployed: Vec<_> = order.into_iter().map(|i| down.positions[i]).collect();
                let side = if address >= 0x4000 { -1. } else { 1. };
                let forward = [
                    [side * 6., -21., 5.],
                    [if side < 0. { -25. } else { 26. }, -25., 1.],
                ];
                if !forward.iter().all(|p| base.positions.contains(p))
                    || base
                        .positions
                        .iter()
                        .zip(&deployed)
                        .any(|(a, b)| forward.contains(a) && a != b)
                {
                    return Err("unreviewed AV8.SH fixed flap edge".into());
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
        let gear = branch(bytes, &original, 0x5640, 1, &SWITCHED_GEAR, &[])?;
        for (address, points) in [
            (0x4162, vec![[-25., -30., 1.], [-25., -27., 1.]]),
            (0x4181, vec![[-25., -30., 1.], [-25., -27., 1.]]),
            (0x421d, vec![[25., -30., 1.], [25., -27., 1.]]),
            (0x423c, vec![[25., -30., 1.], [25., -27., 1.]]),
        ] {
            roots(&gear, &[address], &points)?;
        }
        roots(
            &shape,
            &[0x4372, 0x4391],
            &[[0., -24., -9.], [0., -22., -9.]],
        )?;
        roots(&shape, &NOSE, &[[0., 16., -9.], [0., 26., -9.]])?;
        for address in [0x256c, 0x26ac, 0x2c17, 0x2745, 0x2ca8] {
            let card = one(&shape, address)?;
            if card.texture != "_AV8.PIC"
                || card
                    .uv
                    .iter()
                    .any(|uv| uv[0] < 0. || uv[0] > 43. || uv[1] < 17. || uv[1] > 35.)
            {
                return Err("unreviewed AV8.SH nozzle card material".into());
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
                    [0., -73., 5.],
                    [0., -4., 20.],
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
            } else if NOSE.contains(&source.address) {
                faces.extend(crate::aircraft_animation::split_surface(
                    &source,
                    [0.; 3],
                    [1., 0., 0.],
                    0.,
                    |p| p[2] - NOSE_CUT,
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
                [0., -73., 5.],
                [0., -4., 20.],
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
                [if left { -2. } else { 2. }, -70.5, 2.],
                [1., 0., 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in result.positions.iter_mut().zip(moved.positions) {
                if p[0].abs() > 3. {
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
        } else if let Some(pivot) = nozzle(a) {
            turn(
                &mut result,
                pivot,
                [0., 0., 1.],
                -15f64.to_radians() * state.lift_controls.vector_yaw_actual.clamp(-1., 1.),
            );
            turn(
                &mut result,
                pivot,
                [1., 0., 0.],
                FRAC_PI_2 * state.lift_controls.vector_pitch_actual.clamp(0., 1.),
            );
        } else if SWITCHED_GEAR.contains(&a) || CENTRAL.contains(&a) {
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
    if SWITCHED_GEAR.contains(&a) {
        let side = if a < 0x4200 { -1. } else { 1. };
        let delta = [-side * 3. * second, 0., 14. * closing];
        let leg = matches!(a, 0x4162 | 0x4181 | 0x421d | 0x423c);
        for p in &mut face.positions {
            let weight = if leg {
                ((1. - p[2]) / 15.).clamp(0., 1.)
            } else {
                1.
            };
            for i in 0..3 {
                p[i] += delta[i] * weight;
            }
        }
    } else if NOSE.contains(&a) {
        let delta = [0., -8. * second, 15. * closing];
        for p in &mut face.positions {
            let weight = ((-9. - p[2]) / (-9. - NOSE_CUT)).clamp(0., 1.);
            for i in 0..3 {
                p[i] += delta[i] * weight;
            }
        }
    } else if matches!(a, 0x4372 | 0x4391) {
        for p in &mut face.positions {
            if p[2] == -16. {
                p[1] -= 4. * second;
                p[2] += 14. * closing;
            }
        }
    } else {
        for p in &mut face.positions {
            p[1] -= 4. * second;
            p[2] += 14. * closing;
        }
    }
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=20 {
        let gear = f64::from(step) / 20.;
        for source in shape
            .faces
            .iter()
            .filter(|f| SWITCHED_GEAR.contains(&f.address) || CENTRAL.contains(&f.address))
        {
            let mut result = source.clone();
            gear_positions(&mut result, gear);
            if result.positions.iter().flatten().any(|v| !v.is_finite()) {
                return Err("nonfinite AV8.SH gear".into());
            }
            let rigid = matches!(
                source.address,
                0x41a0 | 0x41bf | 0x425b | 0x427a | 0x42f6 | 0x4315 | 0x4334 | 0x4353
            ) || (NOSE.contains(&source.address)
                && source.positions.iter().all(|p| p[2] <= NOSE_CUT + 1e-4));
            if rigid {
                for i in 0..source.positions.len() {
                    for j in i + 1..source.positions.len() {
                        if (distance2(source.positions[i], source.positions[j])
                            - distance2(result.positions[i], result.positions[j]))
                        .abs()
                            > 1e-3
                        {
                            return Err("unreviewed AV8.SH rigid wheel deformation".into());
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
    fn four_nozzle_cards_rotate_rigidly_about_their_own_centers() {
        let rig = rig();
        let mut s = state();
        for address in [0x256c, 0x26ac, 0x2745, 0x2ca8] {
            let p = nozzle(address).unwrap();
            let source = face(
                address,
                vec![
                    [p[0], p[1] - 2., p[2] - 1.],
                    [p[0], p[1] + 2., p[2] - 1.],
                    [p[0], p[1] + 2., p[2] + 1.],
                    [p[0], p[1] - 2., p[2] + 1.],
                ],
            );
            for pitch in [0., 0.25, 0.5, 0.75, 1.] {
                s.lift_controls.vector_pitch_actual = pitch;
                for yaw in [-1., -0.5, 0., 0.5, 1.] {
                    s.lift_controls.vector_yaw_actual = yaw;
                    let result = rig.animate(&source, &s).unwrap();
                    let center: [f32; 3] = std::array::from_fn(|i| {
                        result.positions.iter().map(|p| p[i]).sum::<f32>() / 4.
                    });
                    assert!(distance2(center, p) < 1e-8);
                    assert_eq!(result.uv, source.uv);
                    for i in 0..4 {
                        for j in i + 1..4 {
                            assert!(
                                (distance2(source.positions[i], source.positions[j])
                                    - distance2(result.positions[i], result.positions[j]))
                                .abs()
                                    < 1e-3
                            );
                        }
                    }
                    if pitch == 1. && yaw == 0. {
                        assert!((result.positions[0][1] - result.positions[1][1]).abs() < 1e-4);
                        assert!(
                            (result.positions[0][2] - result.positions[1][2] + 4.).abs() < 1e-4
                        );
                    }
                    if pitch == 0. && yaw == 0. {
                        assert_eq!(result.positions, source.positions);
                    }
                }
            }
        }
    }
    #[test]
    fn fin_and_outer_roll_splits_preserve_fixed_source_regions() {
        let rig = rig();
        let mut s = state();
        s.rudder = 1.;
        s.aileron = 1.;
        let fin = face(
            0x3288,
            vec![
                [0., -69., 8.],
                [0., -81., 8.],
                [0., -82., 23.],
                [0., -74., 23.],
            ],
        );
        let pieces = crate::aircraft_animation::split_surface(
            &fin,
            [0., -73., 5.],
            [0., -4., 20.],
            0.,
            rudder_distance,
        );
        assert_eq!(pieces.len(), 2);
        assert_eq!(
            rig.animate(&pieces[0], &s).unwrap().positions,
            pieces[0].positions
        );
        assert_ne!(
            rig.animate(&pieces[1], &s).unwrap().positions,
            pieces[1].positions
        );
        for p in &pieces[1].positions {
            if rudder_distance(*p).abs() < 1e-4 {
                assert!(rig.animate(&pieces[1], &s).unwrap().positions.contains(p));
            }
        }
    }
    #[test]
    fn all_central_and_outrigger_gear_is_hidden_when_retracted() {
        let rig = rig();
        let mut s = state();
        s.gear = 0.;
        for address in SWITCHED_GEAR.into_iter().chain(CENTRAL) {
            assert!(rig.animate(&face(address, vec![[0.; 3]; 3]), &s).is_none());
        }
    }
    #[test]
    fn rigid_wheels_and_leg_cut_share_the_same_travel_at_21_positions() {
        let wheel = face(
            0x41a0,
            vec![
                [-25., -30., -14.],
                [-25., -26., -14.],
                [-25., -26., -17.],
                [-25., -30., -17.],
            ],
        );
        let leg = face(
            0x4162,
            vec![
                [-25., -30., 1.],
                [-25., -27., 1.],
                [-25., -27., -14.],
                [-25., -30., -14.],
            ],
        );
        for step in 0..=20 {
            let gear = f64::from(step) / 20.;
            let mut a = wheel.clone();
            let mut b = leg.clone();
            gear_positions(&mut a, gear);
            gear_positions(&mut b, gear);
            assert_eq!(b.positions[..2], leg.positions[..2]);
            assert_eq!(a.positions[0], b.positions[3]);
            for i in 0..4 {
                for j in i + 1..4 {
                    assert!(
                        (distance2(a.positions[i], a.positions[j])
                            - distance2(wheel.positions[i], wheel.positions[j]))
                        .abs()
                            < 1e-3
                    );
                }
            }
        }
    }
}
