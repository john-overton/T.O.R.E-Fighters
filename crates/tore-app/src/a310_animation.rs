//! A310.SH source control skins and explicitly fitted elevator and gear travel.
//! Source bytes remain inert. Continuous laws are agent-authored presentation fits.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};

const WORDS: [usize; 4] = [0x6160, 0x616c, 0x6172, 0x6178];
const RUDDER: [usize; 2] = [0x484b, 0x4872];
const TAIL_LEFT: [usize; 4] = [0x3be9, 0x3c04, 0x3c64, 0x3c80];
const TAIL_RIGHT: [usize; 4] = [0x2f6a, 0x2f85, 0x307a, 0x3096];
const ROLL_LEFT: [usize; 2] = [0x411a, 0x413b];
const ROLL_RIGHT: [usize; 2] = [0x32b3, 0x3575];
const MAIN_RIGHT: [usize; 6] = [0x48e4, 0x490b, 0x4932, 0x4951, 0x4970, 0x498b];
const MAIN_LEFT: [usize; 4] = [0x49f1, 0x4a18, 0x4a3f, 0x4a62];
const NOSE: [usize; 4] = [0x4aca, 0x4ae9, 0x4b08, 0x4b27];
const EPS: f32 = 1e-4;

struct Morph {
    neutral: Vec<[f32; 3]>,
    down: Vec<[f32; 3]>,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
#[derive(Clone, Copy, Default)]
struct Pose {
    gear: f64,
    flaps: f64,
    pitch: f64,
    roll: f64,
    yaw: f64,
}
fn face(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut matches = shape.faces.iter().filter(|f| f.address == address);
    let found = matches
        .next()
        .ok_or_else(|| format!("A310.SH missing reviewed face {address:x}"))?;
    if matches.next().is_some() {
        return Err(format!("A310.SH duplicate reviewed face {address:x}").into());
    }
    Ok(found)
}
fn require_points(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| face(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed A310.SH attachment {address:x}").into());
        }
    }
    Ok(())
}
fn branch(
    bytes: &[u8],
    original: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let shape = Shape::with_state(bytes, &[(word, value)].into())?;
    let active: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
    if active
        .difference(original)
        .copied()
        .collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || original
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed A310.SH branch {word:x}={value}").into());
    }
    Ok(shape)
}
fn tail(address: usize) -> bool {
    TAIL_LEFT.contains(&address) || TAIL_RIGHT.contains(&address)
}
fn gear(address: usize) -> bool {
    MAIN_LEFT.contains(&address) || MAIN_RIGHT.contains(&address) || NOSE.contains(&address)
}
fn pitch_distance(p: [f32; 3]) -> f32 {
    p[1] + 112. + 0.5 * (p[0].abs() - 7.)
}
fn wheel_cut(address: usize) -> f32 {
    if NOSE.contains(&address) { -13. } else { -12. }
}
fn split(source: &Face, distance: impl Fn([f32; 3]) -> f32) -> Vec<Face> {
    crate::aircraft_animation::split_surface(source, [0.; 3], [1., 0., 0.], 0., distance)
}

impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 20868
            || shape.faces.len() != 248
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed A310.SH animation layout".into());
        }
        require_points(&shape, &RUDDER, &[[0., -105., 17.], [0., -127., 57.]])?;
        for (addresses, side) in [(&TAIL_LEFT[..2], -1.), (&TAIL_RIGHT[..2], 1.)] {
            require_points(
                &shape,
                addresses,
                &[[side * 7., -95., 11.], [side * 3., -118., 11.]],
            )?;
        }
        for (addresses, side) in [(&TAIL_LEFT[2..], -1.), (&TAIL_RIGHT[2..], 1.)] {
            require_points(
                &shape,
                addresses,
                &[
                    [side * 18., -104., 11.],
                    [side * 18., -124., 11.],
                    [side * 41., -122., 11.],
                    [side * 41., -133., 11.],
                ],
            )?;
        }
        for (addresses, side) in [(&ROLL_LEFT, -1.), (&ROLL_RIGHT, 1.)] {
            require_points(
                &shape,
                addresses,
                &[[side * 45., -32., -1.], [side * 115., -56., 3.]],
            )?;
            for &address in addresses {
                let f = face(&shape, address)?;
                if f.positions.len() != 3
                    || !f
                        .positions
                        .iter()
                        .any(|p| p[0] == side * 45. && p[1] == -22. && [-2., 1.].contains(&p[2]))
                {
                    return Err("unreviewed A310.SH paired aileron hinge".into());
                }
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for (value, ids) in [(1, [0x4771, 0x4798]), (-1, [0x47de, 0x4805])] {
            let rudder = branch(bytes, &original, 0x6178, value, &ids, &RUDDER)?;
            require_points(&rudder, &ids, &[[0., -105., 17.], [0., -127., 57.]])?;
        }
        let mut flaps = BTreeMap::new();
        for (word, bindings) in [
            (
                0x616c,
                [
                    (0x4704, 0x46ba, [1, 2, 3, 0]),
                    (0x472b, 0x469b, [3, 0, 1, 2]),
                ],
            ),
            (
                0x6172,
                [
                    (0x4622, 0x45d8, [3, 0, 1, 2]),
                    (0x4649, 0x45b9, [2, 3, 0, 1]),
                ],
            ),
        ] {
            let removed: Vec<_> = bindings.iter().map(|b| b.0).collect();
            let added: Vec<_> = bindings.iter().map(|b| b.1).collect();
            let down = branch(bytes, &original, word, -1, &added, &removed)?;
            branch(bytes, &original, word, 1, &[], &removed)?;
            for (address, target, order) in bindings {
                let base = face(&shape, address)?;
                let deployed = face(&down, target)?;
                if base.positions.len() != 4 || deployed.positions.len() != 4 {
                    return Err("unreviewed A310.SH flap topology".into());
                }
                let down: Vec<_> = order.iter().map(|&i| deployed.positions[i]).collect();
                let mut anchors = 0;
                for (a, b) in base.positions.iter().zip(&down) {
                    if a[1] == -22. {
                        if a != b {
                            return Err("A310.SH flap source hinge moved".into());
                        }
                        anchors += 1;
                    } else if a[1] != -32. || b[1] != -31. || b[2] != a[2] - 4. {
                        return Err("unreviewed A310.SH flap down endpoint".into());
                    }
                }
                if anchors != 2 {
                    return Err("unreviewed A310.SH flap attachment count".into());
                }
                flaps.insert(
                    address,
                    Morph {
                        neutral: base.positions.clone(),
                        down,
                    },
                );
            }
        }
        let ids: Vec<_> = MAIN_RIGHT
            .iter()
            .chain(&MAIN_LEFT)
            .chain(&NOSE)
            .copied()
            .collect();
        let deployed = branch(bytes, &original, 0x6160, 1, &ids, &[])?;
        branch(bytes, &original, 0x6160, -1, &[], &[])?;
        for &address in &ids {
            require_points(&deployed, &[address], gear_roots(address))?;
        }
        let mut faces = Vec::new();
        for source in shape.faces {
            if tail(source.address) {
                faces.extend(split(&source, pitch_distance));
            } else {
                faces.push(source);
            }
        }
        for source in deployed.faces.into_iter().filter(|f| gear(f.address)) {
            if NOSE.contains(&source.address) {
                faces.push(source);
            } else {
                faces.extend(split(&source, |p| p[2] - wheel_cut(source.address)));
            }
        }
        shape.faces = faces;
        let rig = Self { flaps };
        validate_gear(&shape)?;
        rig.validate_controls(&shape)?;
        Ok((rig, shape))
    }

    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        self.transform(
            source,
            Pose {
                gear: state.gear,
                flaps: state.flaps,
                pitch: state.elevator,
                roll: state.aileron,
                yaw: state.rudder,
            },
        )
    }
    fn transform(&self, source: &Face, pose: Pose) -> Option<Face> {
        let mut result = source.clone();
        let address = source.address;
        if let Some(morph) = self.flaps.get(&address) {
            let fraction = pose.flaps.clamp(0., 1.) as f32;
            result.positions = morph
                .neutral
                .iter()
                .zip(&morph.down)
                .map(|(a, b)| std::array::from_fn(|i| a[i] + fraction * (b[i] - a[i])))
                .collect();
        } else if RUDDER.contains(&address) && pose.yaw != 0. {
            turn(
                &mut result,
                [0., -105., 17.],
                [0., -22., 40.],
                0.35 * pose.yaw.clamp(-1., 1.),
            );
            for (p, a) in result.positions.iter_mut().zip(&source.positions) {
                if [[0., -105., 17.], [0., -127., 57.]].contains(a) {
                    *p = *a;
                }
            }
        } else if tail(address) && pose.pitch != 0. {
            let side = if TAIL_LEFT.contains(&address) {
                -1.
            } else {
                1.
            };
            turn(
                &mut result,
                [side * 7., -112., 11.],
                [1., -side * 0.5, 0.],
                -0.30 * pose.pitch.clamp(-1., 1.),
            );
            for (p, a) in result.positions.iter_mut().zip(&source.positions) {
                if pitch_distance(*a) >= -EPS || *a == [side * 3., -118., 11.] {
                    *p = *a;
                }
            }
        } else if (ROLL_LEFT.contains(&address) || ROLL_RIGHT.contains(&address)) && pose.roll != 0.
        {
            let side = if ROLL_LEFT.contains(&address) {
                -1.
            } else {
                1.
            };
            turn(
                &mut result,
                [side * 45., -22., -0.5],
                [side * 70., -34., 3.5],
                -0.20 * pose.roll.clamp(-1., 1.),
            );
            for (p, a) in result.positions.iter_mut().zip(&source.positions) {
                if *a != [side * 45., -32., -1.] {
                    *p = *a;
                }
            }
        } else if gear(address) {
            if pose.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, pose.gear);
        }
        if result.positions != source.positions {
            update_normal(source, &mut result);
        }
        Some(result)
    }

    fn validate_controls(&self, shape: &Shape) -> AppResult<()> {
        for value in [-1., -0.5, 0., 0.5, 1.] {
            let mut seen: BTreeMap<(u8, [u32; 3]), [f32; 3]> = BTreeMap::new();
            let mut moved = [false; 4];
            for source in &shape.faces {
                let address = source.address;
                let group = if RUDDER.contains(&address) {
                    0
                } else if tail(address) {
                    1
                } else if ROLL_LEFT.contains(&address) || ROLL_RIGHT.contains(&address) {
                    2
                } else if self.flaps.contains_key(&address) {
                    3
                } else {
                    continue;
                };
                let output = self
                    .transform(
                        source,
                        Pose {
                            gear: 1.,
                            yaw: value,
                            pitch: value,
                            roll: value,
                            flaps: value.abs(),
                        },
                    )
                    .ok_or("A310.SH control disappeared")?;
                for (a, b) in source.positions.iter().zip(&output.positions) {
                    if !b.iter().all(|v| v.is_finite()) || (value == 0. && a != b) {
                        return Err("A310.SH invalid control or non-neutral zero".into());
                    }
                    moved[group as usize] |= distance_squared(*a, *b) > EPS * EPS;
                    let key = (group, a.map(f32::to_bits));
                    if seen
                        .insert(key, *b)
                        .is_some_and(|previous| distance_squared(previous, *b) > EPS * EPS)
                    {
                        return Err("A310.SH opposite control skins separated".into());
                    }
                    let fixed = match group {
                        0 => [[0., -105., 17.], [0., -127., 57.]].contains(a),
                        1 => pitch_distance(*a) >= -EPS || (a[0].abs() == 3. && a[1] == -118.),
                        2 => a[1] != -32.,
                        _ => a[1] == -22.,
                    };
                    if fixed && a != b {
                        return Err("A310.SH control attachment moved".into());
                    }
                    if value != 0. && !fixed && group == 1 && (b[2] - a[2]) * value as f32 <= 0. {
                        return Err("A310.SH pitch sides disagree".into());
                    }
                    if value != 0.
                        && !fixed
                        && group == 2
                        && (b[2] - a[2]) * value as f32 * a[0].signum() <= 0.
                    {
                        return Err("A310.SH roll sides disagree".into());
                    }
                }
            }
            if value != 0. && moved.contains(&false) {
                return Err("A310.SH required control did not move".into());
            }
        }
        Ok(())
    }
}

fn gear_roots(address: usize) -> &'static [[f32; 3]] {
    match address {
        0x48e4 | 0x490b => &[[6., -29., -7.], [6., -17., -7.]],
        0x4932 | 0x4951 => &[[1., -23., -9.], [11., -23., -5.]],
        0x4970 | 0x498b => &[[1., -23., -9.], [11., -23., -5.], [8., -23., -5.]],
        0x49f1 | 0x4a18 => &[[-6., -29., -7.], [-6., -17., -7.]],
        0x4a3f | 0x4a62 => &[[-1., -23., -9.], [-8., -23., -5.], [-11., -23., -5.]],
        0x4aca | 0x4ae9 => &[[0., 58., -9.], [0., 66., -6.]],
        0x4b08 | 0x4b27 => &[[-2., 63., -8.], [2., 63., -8.]],
        _ => &[],
    }
}
fn gear_positions(face: &mut Face, deployed: f64) {
    let closing = 1. - deployed.clamp(0., 1.);
    if NOSE.contains(&face.address) {
        if closing == 0. {
            return;
        }
        let source = face.positions.clone();
        turn(
            face,
            [0., 63., -8.],
            [1., 0., 0.],
            -186f64.to_radians() * closing,
        );
        // The front atlas view provides the attachment line. The side quad's
        // broad sloped upper corners are image margins, not independent hinges.
        for (p, original) in face.positions.iter_mut().zip(source) {
            if original[1] == 63. && original[2] == -8. {
                *p = original;
            }
        }
        return;
    }
    let closing = closing as f32;
    let aft = (closing * 2.).min(1.);
    let lift = (closing * 2. - 1.).max(0.);
    // Clear the complete fixed upper chord aft before the wheel rises past it.
    // Both crossed atlas views receive the same displacement field.
    for p in &mut face.positions {
        let weight = ((-9. - p[2]) / 3.).clamp(0., 1.);
        p[1] -= 16. * aft * weight;
        p[2] += 19. * lift * weight;
    }
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=20 {
        let deployed = step as f64 / 20.;
        let mut seen: BTreeMap<(u8, [u32; 3]), [f32; 3]> = BTreeMap::new();
        for source in shape.faces.iter().filter(|f| gear(f.address)) {
            let mut output = source.clone();
            gear_positions(&mut output, deployed);
            let nose = NOSE.contains(&source.address);
            let assembly = if nose {
                0
            } else if MAIN_LEFT.contains(&source.address) {
                1
            } else {
                2
            };
            for (a, b) in source.positions.iter().zip(&output.positions) {
                if !b.iter().all(|v| v.is_finite())
                    || (deployed == 1. && a != b)
                    || ((!nose && a[2] >= -9. || nose && a[1] == 63. && a[2] == -8.) && a != b)
                    || (!nose && b[0].abs() < 1. - EPS)
                {
                    return Err("A310.SH invalid gear endpoint, root or separation".into());
                }
                if seen
                    .insert((assembly, a.map(f32::to_bits)), *b)
                    .is_some_and(|previous| distance_squared(previous, *b) > EPS * EPS)
                {
                    return Err("A310.SH crossed gear skins separated".into());
                }
            }
            if !nose
                && !source
                    .positions
                    .iter()
                    .all(|p| p[2] <= wheel_cut(source.address) + EPS)
            {
                continue;
            }
            for (i, a) in source.positions.iter().enumerate() {
                for (j, b) in source.positions.iter().enumerate().skip(i + 1) {
                    if (distance_squared(*a, *b)
                        - distance_squared(output.positions[i], output.positions[j]))
                    .abs()
                        > 0.002
                    {
                        return Err("A310.SH wheel region lost rigidity".into());
                    }
                }
            }
            if deployed == 0.
                && output.positions.iter().any(|p| {
                    if nose {
                        p[0].abs() > 2. + EPS
                            || !(59. - EPS..=71. + EPS).contains(&p[1])
                            || !(-10. - EPS..=6. + EPS).contains(&p[2])
                            || p[2] < -10. + 0.25 * (p[1] - 59.) - EPS
                    } else {
                        p[0].abs() > 11. + EPS
                            || !(-45. - EPS..=-33. + EPS).contains(&p[1])
                            || !(-2. - EPS..=7. + EPS).contains(&p[2])
                    }
                })
            {
                return Err("A310.SH wheels exceed fitted stow envelope".into());
            }
        }
    }
    Ok(())
}
fn polygon_normal(points: &[[f32; 3]]) -> Option<[f32; 3]> {
    let mut normal = [0f64; 3];
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        normal[0] += f64::from(a[1] - b[1]) * f64::from(a[2] + b[2]);
        normal[1] += f64::from(a[2] - b[2]) * f64::from(a[0] + b[0]);
        normal[2] += f64::from(a[0] - b[0]) * f64::from(a[1] + b[1]);
    }
    let length = normal.iter().map(|v| v * v).sum::<f64>().sqrt();
    (length > 1e-9).then(|| normal.map(|v| (v / length) as f32))
}
fn update_normal(source: &Face, result: &mut Face) {
    let (Some(old), Some(reference), Some(mut normal)) = (
        source.normal,
        polygon_normal(&source.positions),
        polygon_normal(&result.positions),
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
        normal = normal.map(|v| -v);
    }
    result.normal = Some([normal[0], normal[2], normal[1]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn panel(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            address,
            colors: vec![17; positions.len()],
            uv: positions
                .iter()
                .map(|p| [p[0] * 0.25, p[1] * 0.5])
                .collect(),
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
        }
    }
    fn pose() -> Pose {
        Pose {
            gear: 1.,
            ..Pose::default()
        }
    }
    #[test]
    fn signed_rudder_starts_neutral_and_keeps_diagonal_source_edge() {
        let original = panel(
            RUDDER[0],
            vec![
                [0., -105., 17.],
                [0., -115., 17.],
                [0., -130., 57.],
                [0., -127., 57.],
            ],
        );
        for value in [-1., -0.5, 0., 0.5, 1.] {
            let moved = rig()
                .transform(
                    &original,
                    Pose {
                        yaw: value,
                        ..pose()
                    },
                )
                .unwrap();
            assert_eq!(moved.positions[0], original.positions[0]);
            assert_eq!(moved.positions[3], original.positions[3]);
            assert_eq!(moved.uv, original.uv);
            assert_eq!(moved.colors, original.colors);
            if value == 0. {
                assert_eq!(moved.positions, original.positions);
            } else {
                assert!(moved.positions[1][0] * value as f32 > 0.);
            }
        }
    }
    #[test]
    fn clipped_elevators_keep_both_root_endpoints_and_forward_tail_fixed() {
        for (address, side) in [(TAIL_LEFT[0], -1.), (TAIL_RIGHT[0], 1.)] {
            let source = panel(
                address,
                vec![
                    [side * 3., -118., 11.],
                    [side * 18., -124., 11.],
                    [side * 18., -104., 11.],
                    [side * 7., -95., 11.],
                ],
            );
            let pieces = split(&source, pitch_distance);
            assert_eq!(pieces.len(), 2);
            for value in [-1., -0.5, 0., 0.5, 1.] {
                let mut moving = 0;
                for piece in &pieces {
                    let result = rig()
                        .transform(
                            piece,
                            Pose {
                                pitch: value,
                                ..pose()
                            },
                        )
                        .unwrap();
                    assert_eq!(result.uv, piece.uv);
                    for (a, b) in piece.positions.iter().zip(result.positions) {
                        if pitch_distance(*a) >= -EPS || a[0].abs() == 3. {
                            assert_eq!(*a, b);
                        } else if value != 0. {
                            assert!((b[2] - a[2]) * value as f32 > 0.);
                            moving += 1;
                        } else {
                            assert_eq!(*a, b);
                        }
                    }
                }
                assert!(value == 0. || moving > 0);
            }
        }
    }
    #[test]
    fn opposite_roll_skins_share_trailing_point_and_ignore_flap_input() {
        for (addresses, side) in [(&ROLL_LEFT, -1.), (&ROLL_RIGHT, 1.)] {
            for value in [-1., -0.5, 0., 0.5, 1.] {
                let mut trailing = Vec::new();
                for (&address, height) in addresses.iter().zip([1., -2.]) {
                    let source = panel(
                        address,
                        vec![
                            [side * 45., -22., height],
                            [side * 115., -56., 3.],
                            [side * 45., -32., -1.],
                        ],
                    );
                    let control = Pose {
                        roll: value,
                        ..pose()
                    };
                    let moved = rig().transform(&source, control).unwrap();
                    let with_flaps = rig()
                        .transform(
                            &source,
                            Pose {
                                flaps: 1.,
                                ..control
                            },
                        )
                        .unwrap();
                    assert_eq!(moved.positions, with_flaps.positions);
                    assert_eq!(moved.positions[..2], source.positions[..2]);
                    if value != 0. {
                        assert!((moved.positions[2][2] + 1.) * value as f32 * side > 0.);
                    }
                    trailing.push(moved.positions[2]);
                }
                assert_eq!(trailing[0], trailing[1]);
            }
        }
    }
    #[test]
    fn flap_endpoint_interpolation_keeps_hinges_and_is_independent_of_roll() {
        let original = panel(
            0x4649,
            vec![
                [11., -22., -1.],
                [45., -22., 1.],
                [45., -32., -1.],
                [11., -32., -3.],
            ],
        );
        let target = vec![
            [11., -22., -1.],
            [45., -22., 1.],
            [45., -31., -5.],
            [11., -31., -7.],
        ];
        let rig = Rig {
            flaps: [(
                original.address,
                Morph {
                    neutral: original.positions.clone(),
                    down: target.clone(),
                },
            )]
            .into(),
        };
        for fraction in [0., 0.25, 0.5, 0.75, 1.] {
            let moved = rig
                .transform(
                    &original,
                    Pose {
                        flaps: fraction,
                        roll: 1.,
                        ..pose()
                    },
                )
                .unwrap();
            assert_eq!(moved.positions[..2], original.positions[..2]);
            assert_eq!(moved.positions[2][1], -32. + fraction as f32);
            assert_eq!(moved.positions[2][2], -1. - 4. * fraction as f32);
            if fraction == 1. {
                assert_eq!(moved.positions, target);
            }
            assert_eq!(moved.uv, original.uv);
        }
    }
    #[test]
    fn crossed_gear_views_keep_rigid_lower_wheels_and_anchored_linkage_at_21_positions() {
        let mut faces = Vec::new();
        for (side_ids, front_ids, side) in [
            ([0x48e4, 0x490b], [0x4932, 0x4951], 1.),
            ([0x49f1, 0x4a18], [0x4a3f, 0x4a62], -1.),
        ] {
            for address in side_ids {
                let source = panel(
                    address,
                    vec![
                        [side * 6., -29., -7.],
                        [side * 6., -17., -7.],
                        [side * 8., -17., -21.],
                        [side * 8., -29., -21.],
                    ],
                );
                faces.extend(split(&source, |p| p[2] + 12.));
            }
            for address in front_ids {
                let source = panel(
                    address,
                    vec![
                        [side, -23., -9.],
                        [side * 11., -23., -5.],
                        [side * 11., -23., -21.],
                        [side, -23., -21.],
                    ],
                );
                faces.extend(split(&source, |p| p[2] + 12.));
            }
        }
        for address in NOSE {
            let source = if address <= 0x4ae9 {
                panel(
                    address,
                    vec![
                        [0., 58., -9.],
                        [0., 66., -6.],
                        [0., 66., -21.],
                        [0., 58., -21.],
                    ],
                )
            } else {
                panel(
                    address,
                    vec![
                        [-2., 63., -8.],
                        [2., 63., -8.],
                        [2., 63., -21.],
                        [-2., 63., -21.],
                    ],
                )
            };
            faces.push(source);
        }
        let shape = Shape {
            billboards: Vec::new(),
            faces,
            lines: Vec::new(),
            state_words: BTreeSet::new(),
        };
        validate_gear(&shape).unwrap();
        for source in &shape.faces {
            assert!(rig().transform(source, Pose::default()).is_none());
            let full = rig().transform(source, pose()).unwrap();
            assert_eq!(full.positions, source.positions);
        }
    }
}
