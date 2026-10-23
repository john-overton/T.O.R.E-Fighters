//! F15.SH source skins and explicitly fitted surface partitions and linkage.
//! See docs/spec/variety-animation.md. Imported instructions never execute.
use crate::{AppResult, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};

const WORDS: [usize; 5] = [0x8800, 0x8806, 0x880c, 0x8818, 0x881e];
const FLAME: [usize; 8] = [
    0x5e36, 0x5e5d, 0x5e84, 0x5eab, 0x5ed2, 0x5ef9, 0x5f20, 0x5f47,
];
const BRAKE: [usize; 4] = [0x5d73, 0x5d8a, 0x5da1, 0x5db8];
const GEAR: [usize; 22] = [
    0x573a, 0x5751, 0x5768, 0x577f, 0x57db, 0x57fa, 0x5819, 0x5838, 0x5908, 0x591f, 0x5936, 0x594d,
    0x59a9, 0x59c8, 0x59e7, 0x5a06, 0x5a82, 0x5aa1, 0x5ac0, 0x5ad7, 0x5aee, 0x5b0d,
];
const MAIN_LEFT: [usize; 4] = [0x57db, 0x57fa, 0x5819, 0x5838];
const MAIN_RIGHT: [usize; 4] = [0x59a9, 0x59c8, 0x59e7, 0x5a06];
const NOSE: [usize; 6] = [0x5a82, 0x5aa1, 0x5ac0, 0x5ad7, 0x5aee, 0x5b0d];
const FIN: [usize; 8] = [
    0x2470, 0x5282, 0x24b8, 0x52df, 0x24de, 0x4e3c, 0x2526, 0x4e99,
];
const TAIL_LEFT: [usize; 2] = [0x55df, 0x560c];
const TAIL_RIGHT: [usize; 2] = [0x51a4, 0x51d1];
const ROLL: [usize; 4] = [0x54d7, 0x5575, 0x50f0, 0x5111];
const EPSILON: f32 = 1e-4;

struct Morph {
    neutral: Vec<[f32; 3]>,
    down: Vec<[f32; 3]>,
    closure: bool,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
pub fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}
fn face(shape: &Shape, address: usize) -> AppResult<&Face> {
    shape
        .faces
        .iter()
        .find(|f| f.address == address)
        .ok_or_else(|| format!("F15.SH lacks reviewed face {address:x}").into())
}
fn require_edge(shape: &Shape, addresses: &[usize], edge: [[f32; 3]; 2]) -> AppResult<()> {
    for &address in addresses {
        if !edge
            .iter()
            .all(|p| face(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed F15.SH attachment {address:x}").into());
        }
    }
    Ok(())
}
fn split(face: &Face, distance: impl Fn([f32; 3]) -> f32) -> Vec<Face> {
    crate::aircraft_animation::split_surface(face, [0.; 3], [1., 0., 0.], 0., distance)
}
fn fin_distance(p: [f32; 3]) -> f32 {
    p[1] + 57. - (p[2] - 5.) * 5. / 38.
}
fn tail_geometry(left: bool) -> ([f32; 3], [f32; 3]) {
    if left {
        ([-22., -68., -1.], [25., -3., 0.])
    } else {
        ([21., -68., -1.], [26., 3., 0.])
    }
}
fn tail_distance(p: [f32; 3]) -> f32 {
    let (pivot, axis) = tail_geometry(p[0] < 0.);
    p[1] - pivot[1] - (p[0] - pivot[0]) * axis[1] / axis[0]
}
fn roll_leading(p: [f32; 3]) -> bool {
    (p[1] + 21. + (p[0].abs() - 39.) * 4. / 20.).abs() < EPSILON
}
fn roll_panel(face: &Face) -> bool {
    face.positions
        .iter()
        .all(|p| (42. - EPSILON..=56. + EPSILON).contains(&p[0].abs()))
}

impl Rig {
    pub fn load(bytes: &[u8], mut neutral: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 30762
            || neutral.faces.len() != 346
            || neutral.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F15.SH animation layout".into());
        }
        require_edge(
            &neutral,
            &[0x2470, 0x5282],
            [[-22., -57., 5.], [-22., -52., 43.]],
        )?;
        require_edge(
            &neutral,
            &[0x24de, 0x4e3c],
            [[21., -57., 5.], [21., -52., 43.]],
        )?;
        require_edge(&neutral, &TAIL_LEFT, [[-22., -68., -1.], [-47., -65., -1.]])?;
        require_edge(&neutral, &TAIL_RIGHT, [[21., -68., -1.], [47., -65., -1.]])?;
        let original: BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
        for (word, expected) in [
            (0x8800, FLAME.as_slice()),
            (0x8806, BRAKE.as_slice()),
            (0x880c, GEAR.as_slice()),
        ] {
            let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
            let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
            if !original.is_subset(&active)
                || active
                    .difference(&original)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != expected.iter().copied().collect()
            {
                return Err(format!("unreviewed F15.SH device branch {word:x}").into());
            }
            neutral.faces.extend(
                pose.faces
                    .into_iter()
                    .filter(|f| expected.contains(&f.address)),
            );
        }
        require_edge(
            &neutral,
            &[0x5da1, 0x5db8],
            [[-3., 28., 11.], [3., 28., 11.]],
        )?;
        require_edge(
            &neutral,
            &[0x573a, 0x5751],
            [[-7., -4., -12.], [-7., 11., -12.]],
        )?;
        require_edge(
            &neutral,
            &[0x5908, 0x591f],
            [[7., -4., -12.], [7., 11., -12.]],
        )?;
        for &address in MAIN_LEFT.iter().chain(&MAIN_RIGHT).chain(&NOSE) {
            let root = if NOSE.contains(&address) { -8. } else { -11. };
            if face(&neutral, address)?
                .positions
                .iter()
                .filter(|p| p[2] == root)
                .count()
                != 2
            {
                return Err("unreviewed F15.SH gear upper attachment edge".into());
            }
        }
        let mut flaps = BTreeMap::new();
        for (word, bindings, closure, hinges) in [
            (
                0x8818,
                [
                    (0x5c3e, 0x5cc6, [0, 1, 2, 3]),
                    (0x5c5d, 0x5ca7, [1, 2, 3, 0]),
                ],
                0x5ce5,
                [
                    [-39., -21., 2.],
                    [-39., -21., 4.],
                    [-22., -21., 2.],
                    [-22., -21., 4.],
                ],
            ),
            (
                0x881e,
                [
                    (0x5b57, 0x5bdf, [3, 0, 1, 2]),
                    (0x5b76, 0x5bc0, [3, 0, 1, 2]),
                ],
                0x5bfe,
                [
                    [39., -21., 2.],
                    [39., -21., 4.],
                    [21., -21., 2.],
                    [21., -21., 4.],
                ],
            ),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
            if active
                .difference(&original)
                .copied()
                .collect::<BTreeSet<_>>()
                != bindings.iter().map(|b| b.1).chain([closure]).collect()
                || original
                    .difference(&active)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != bindings.iter().map(|b| b.0).collect()
            {
                return Err(format!("unreviewed F15.SH flap branch {word:x}").into());
            }
            for (address, deployed, order) in bindings {
                let base = face(&neutral, address)?;
                let target = face(&down, deployed)?;
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("unreviewed F15.SH flap topology".into());
                }
                let target: Vec<_> = order.map(|i| target.positions[i]).into();
                if base.positions.iter().filter(|p| hinges.contains(p)).count() != 2
                    || base
                        .positions
                        .iter()
                        .zip(&target)
                        .any(|(a, b)| hinges.contains(a) && a != b)
                {
                    return Err("unreviewed F15.SH flap hinge".into());
                }
                flaps.insert(
                    address,
                    Morph {
                        neutral: base.positions.clone(),
                        down: target,
                        closure: false,
                    },
                );
            }
            let a = &flaps[&bindings[0].0];
            let b = &flaps[&bindings[1].0];
            for (p, target) in a.neutral.iter().zip(&a.down) {
                if let Some(i) = b.neutral.iter().position(|q| q == p)
                    && b.down[i] != *target
                {
                    return Err("unreviewed F15.SH flap twin seam".into());
                }
            }
            let closing = face(&down, closure)?.clone();
            let mut closed = Vec::new();
            for p in &closing.positions {
                let original = a
                    .down
                    .iter()
                    .position(|q| q == p)
                    .map(|i| a.neutral[i])
                    .or_else(|| b.down.iter().position(|q| q == p).map(|i| b.neutral[i]))
                    .ok_or("unreviewed F15.SH flap end closure")?;
                closed.push(original);
            }
            flaps.insert(
                closure,
                Morph {
                    neutral: closed,
                    down: closing.positions.clone(),
                    closure: true,
                },
            );
            neutral.faces.push(closing);
        }
        neutral.faces = neutral
            .faces
            .into_iter()
            .flat_map(|f| {
                if FIN.contains(&f.address) {
                    split(&f, fin_distance)
                } else if TAIL_LEFT.contains(&f.address) || TAIL_RIGHT.contains(&f.address) {
                    split(&f, tail_distance)
                } else if ROLL.contains(&f.address) {
                    split(&f, |p| p[0].abs() - 42.)
                        .into_iter()
                        .flat_map(|piece| split(&piece, |p| p[0].abs() - 56.))
                        .collect()
                } else if MAIN_LEFT.contains(&f.address) || MAIN_RIGHT.contains(&f.address) {
                    split(&f, |p| p[2] + 18.)
                } else {
                    vec![f]
                }
            })
            .collect();
        verify_gear(&neutral)?;
        Ok((Self { flaps }, neutral))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let address = source.address;
        if let Some(morph) = self.flaps.get(&address) {
            let travel = state.flaps.clamp(0., 1.) as f32;
            if morph.closure && travel == 0. {
                return None;
            }
            result.positions = morph
                .neutral
                .iter()
                .zip(&morph.down)
                .map(|(a, b)| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * travel))
                .collect();
            update_normal(source, &mut result);
        } else if FIN.contains(&address)
            && source.positions.iter().all(|p| fin_distance(*p) <= EPSILON)
        {
            turn(
                &mut result,
                [source.positions[0][0], -57., 5.],
                [0., 5., 38.],
                state.rudder.clamp(-1., 1.) * 0.35,
            );
        } else if (TAIL_LEFT.contains(&address) || TAIL_RIGHT.contains(&address))
            && source
                .positions
                .iter()
                .all(|p| tail_distance(*p) <= EPSILON)
        {
            let (pivot, axis) = tail_geometry(TAIL_LEFT.contains(&address));
            turn(
                &mut result,
                pivot,
                axis,
                -state.elevator.clamp(-1., 1.) * 0.30,
            );
        } else if ROLL.contains(&address) && roll_panel(source) {
            let side = if source.positions[0][0] < 0. { -1. } else { 1. };
            turn(
                &mut result,
                [side * 42., -21.6, 3.],
                [side * 14., -2.8, 0.],
                -state.aileron.clamp(-1., 1.) * 0.20,
            );
            for (before, after) in source.positions.iter().zip(&mut result.positions) {
                if roll_leading(*before) {
                    *after = *before;
                }
            }
            update_normal(source, &mut result);
        } else if BRAKE.contains(&address) {
            let travel = state.brake.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            turn(
                &mut result,
                [0., 28., 11.],
                [1., 0., 0.],
                (19f64 / 27.).atan() * (1. - travel),
            );
        } else if GEAR.contains(&address) {
            let travel = state.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            gear(&mut result, travel);
            update_normal(source, &mut result);
        } else if flame(address) {
            let travel = state.exhaust.max(0.) as f32;
            if travel == 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = -60. + (p[1] + 60.) * travel;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}

fn turn(face: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    if angle != 0. {
        crate::additional_animation::turn(face, pivot, axis, angle);
    }
}
fn main_linkage(source: &Face, deployed: f64) -> Face {
    let closing = 1. - deployed;
    let side = if source.positions[0][0] < 0. { -1. } else { 1. };
    let pivot = [side * 13., 3., -18.];
    let shift = [0., -9. * closing as f32, 12. * closing as f32];
    let (root, cut) = (-11., -18.);
    let mut target = source.clone();
    turn(
        &mut target,
        pivot,
        [1., 0., 0.],
        -std::f64::consts::FRAC_PI_2 * closing,
    );
    for (p, before) in target.positions.iter_mut().zip(&source.positions) {
        let weight = ((root - before[2]) / (root - cut)).clamp(0., 1.);
        *p = std::array::from_fn(|i| before[i] + (p[i] + shift[i] - before[i]) * weight);
    }
    target
}
fn gear(face: &mut Face, deployed: f64) {
    let source = face.clone();
    let address = face.address;
    if NOSE.contains(&address) {
        // The side image paints a diagonal brace reaching the forward top
        // corner at Y=64. Fold the complete wheel/strut around that attachment.
        // Its transparent rectangle corners are not additional fixed joints.
        // The separate untextured door retains its own source hinge at Y=52.
        let y = if matches!(address, 0x5ac0 | 0x5ad7) {
            52.
        } else {
            64.
        };
        turn(
            face,
            [0., y, -8.],
            [1., 0., 0.],
            -std::f64::consts::FRAC_PI_2 * (1. - deployed),
        );
    } else if MAIN_LEFT.contains(&address) || MAIN_RIGHT.contains(&address) {
        face.positions = main_linkage(&source, deployed).positions;
    } else {
        let side = if source.positions[0][0] < 0. { -1. } else { 1. };
        let mut door = source.clone();
        turn(
            &mut door,
            [side * 7., -4., -12.],
            [0., 1., 0.],
            -side as f64 * std::f64::consts::FRAC_PI_2 * (1. - deployed),
        );
        let brace = matches!(address, 0x5768 | 0x577f | 0x5936 | 0x594d);
        if !brace {
            face.positions = door.positions;
        } else {
            let lower = main_linkage(&source, deployed);
            for ((p, d), l) in face
                .positions
                .iter_mut()
                .zip(door.positions)
                .zip(lower.positions)
            {
                *p = if p[0] == side * 7. && p[2] == -15. {
                    d
                } else {
                    l
                };
            }
        }
    }
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum()
}
fn verify_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=400 {
        let deployed = step as f64 / 400.;
        for source in shape.faces.iter().filter(|f| GEAR.contains(&f.address)) {
            let mut moved = source.clone();
            gear(&mut moved, deployed);
            let nose = NOSE.contains(&source.address);
            let main = MAIN_LEFT.contains(&source.address) || MAIN_RIGHT.contains(&source.address);
            let root = if nose { -8. } else { -11. };
            let cut = if nose { -20. } else { -18. };
            for (a, b) in source.positions.iter().zip(&moved.positions) {
                if !b.iter().all(|v| v.is_finite())
                    || (deployed == 1. && a != b)
                    || (main && a[2] == root && a != b)
                {
                    return Err("unreviewed F15.SH gear endpoint or upper anchor".into());
                }
                if !nose && b[0] * if a[0] < 0. { -1. } else { 1. } <= 0. {
                    return Err("F15.SH fitted main gear crosses centerline".into());
                }
            }
            if !(nose || main && source.positions.iter().all(|p| p[2] <= cut + EPSILON)) {
                continue;
            }
            for i in 0..source.positions.len() {
                for j in i + 1..source.positions.len() {
                    if (distance_squared(source.positions[i], source.positions[j])
                        - distance_squared(moved.positions[i], moved.positions[j]))
                    .abs()
                        > 0.01
                    {
                        return Err("F15.SH fitted lower wheel sheet is not rigid".into());
                    }
                }
            }
            if deployed == 0.
                && moved.positions.iter().any(|p| {
                    if nose {
                        p[0].abs() > 2.01
                            || !(38.99..=64.01).contains(&p[1])
                            || !(-8.01..=6.01).contains(&p[2])
                    } else {
                        !(8.99..=15.01).contains(&p[0].abs())
                            || !(-15.01..=-5.99).contains(&p[1])
                            || !(-11.01..=-0.99).contains(&p[2])
                    }
                })
            {
                return Err("F15.SH fitted lower gear exceeds reviewed stow envelope".into());
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
        n[0] += (a[1] - b[1]) as f64 * (a[2] + b[2]) as f64;
        n[1] += (a[2] - b[2]) as f64 * (a[0] + b[0]) as f64;
        n[2] += (a[0] - b[0]) as f64 * (a[1] + b[1]) as f64;
    }
    let length = n.iter().map(|v| v * v).sum::<f64>().sqrt();
    (length > 1e-9).then(|| n.map(|v| (v / length) as f32))
}
fn update_normal(source: &Face, result: &mut Face) {
    let (Some(reference), Some(before), Some(mut after)) = (
        source.normal,
        normal(&source.positions),
        normal(&result.positions),
    ) else {
        return;
    };
    let reference = [reference[0], reference[2], reference[1]];
    if reference
        .iter()
        .zip(before)
        .map(|(a, b)| a * b)
        .sum::<f32>()
        < 0.
    {
        after = after.map(|v| -v);
    }
    result.normal = Some([after[0], after[2], after[1]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let count = positions.len();
        Face {
            address,
            positions,
            colors: vec![20; count],
            uv: vec![[0.25, 0.75]; count],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([0., 1., 0.]),
            fog: Default::default(),
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn close(a: [f32; 3], b: [f32; 3]) {
        assert!(
            a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4),
            "{a:?} != {b:?}"
        );
    }
    fn area(face: &Face) -> f32 {
        face.positions
            .iter()
            .zip(face.positions.iter().cycle().skip(1))
            .take(face.positions.len())
            .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
            .sum::<f32>()
            .abs()
            * 0.5
    }
    #[test]
    fn bounded_roll_partition_preserves_source_coverage_and_fixed_strips() {
        let source = synthetic(
            ROLL[0],
            vec![
                [-38., -20.8, 3.],
                [-60., -25.2, 3.],
                [-60., -34., 3.],
                [-38., -30., 3.],
            ],
        );
        let pieces: Vec<_> = split(&source, |p| p[0].abs() - 42.)
            .into_iter()
            .flat_map(|f| split(&f, |p| p[0].abs() - 56.))
            .collect();
        assert_eq!(pieces.len(), 3);
        assert!((pieces.iter().map(area).sum::<f32>() - area(&source)).abs() < 1e-3);
        assert_eq!(pieces.iter().filter(|f| roll_panel(f)).count(), 1);
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let mut s = state();
        s.aileron = 1.;
        for piece in pieces {
            let moved = rig.animate(&piece, &s).unwrap();
            assert_eq!(moved.uv, piece.uv);
            assert_eq!(moved.texture, piece.texture);
            if roll_panel(&piece) {
                assert_ne!(moved.positions, piece.positions);
                for (before, after) in piece.positions.iter().zip(moved.positions) {
                    if roll_leading(*before) {
                        close(*before, after);
                    }
                }
            } else {
                assert_eq!(moved.positions, piece.positions);
            }
        }
    }
    #[test]
    fn rudder_and_tail_hinges_hold_and_signed_controls_use_both_sides() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let mut s = state();
        for value in [-1., -0.5, 0., 0.5, 1.] {
            s.rudder = value;
            s.elevator = value;
            for (address, x) in [(FIN[0], -22.), (FIN[4], 21.)] {
                // The third point is invented, not a copied source polygon.
                let f = synthetic(address, vec![[x, -57., 5.], [x, -52., 43.], [x, -63., 17.]]);
                let moved = rig.animate(&f, &s).unwrap();
                close(f.positions[0], moved.positions[0]);
                close(f.positions[1], moved.positions[1]);
                if value == 0. {
                    assert_eq!(moved.positions, f.positions);
                } else {
                    assert!((moved.positions[2][0] - x) * value as f32 > 0.);
                }
            }
            for (address, left) in [(TAIL_LEFT[0], true), (TAIL_RIGHT[0], false)] {
                let (pivot, axis) = tail_geometry(left);
                let end = if left {
                    [-47., -65., -1.]
                } else {
                    [47., -65., -1.]
                };
                let f = synthetic(
                    address,
                    vec![pivot, end, [if left { -32. } else { 32. }, -74., -1.]],
                );
                let moved = rig.animate(&f, &s).unwrap();
                close(pivot, moved.positions[0]);
                close(end, moved.positions[1]);
                assert!(axis[0] > 0.);
                if value == 0. {
                    assert_eq!(moved.positions, f.positions);
                } else {
                    assert!((moved.positions[2][2] + 1.) * value as f32 > 0.);
                }
            }
        }
    }
    #[test]
    fn roll_is_opposed_and_keeps_each_leading_skin_edge() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let mut s = state();
        for value in [-1., -0.5, 0., 0.5, 1.] {
            s.aileron = value;
            for (address, side) in [(ROLL[0], -1.), (ROLL[2], 1.)] {
                let f = synthetic(
                    address,
                    vec![
                        [side * 42., -21.6, 3.],
                        [side * 56., -24.4, 3.],
                        [side * 56., -32., 3.],
                        [side * 42., -30., 3.],
                    ],
                );
                let moved = rig.animate(&f, &s).unwrap();
                close(f.positions[0], moved.positions[0]);
                close(f.positions[1], moved.positions[1]);
                if value == 0. {
                    assert_eq!(moved.positions, f.positions);
                } else {
                    assert!((moved.positions[2][2] - 3.) * side * value as f32 > 0.);
                }
            }
        }
    }
    #[test]
    fn flap_morph_preserves_twin_seams_and_exact_endpoints() {
        let neutral = vec![[-2., 0., 0.], [2., 0., 0.], [2., -3., 0.], [-2., -3., 0.]];
        let down = vec![[-2., 0., 0.], [2., 0., 0.], [2., -2., -2.], [-2., -2., -2.]];
        let rig = Rig {
            flaps: [
                (
                    9001,
                    Morph {
                        neutral: neutral.clone(),
                        down: down.clone(),
                        closure: false,
                    },
                ),
                (
                    9002,
                    Morph {
                        neutral: neutral.iter().rev().copied().collect(),
                        down: down.iter().rev().copied().collect(),
                        closure: false,
                    },
                ),
            ]
            .into(),
        };
        let a = synthetic(9001, neutral.clone());
        let b = synthetic(9002, neutral.iter().rev().copied().collect());
        let mut s = state();
        for travel in [0., 0.25, 0.5, 0.75, 1.] {
            s.flaps = travel;
            let moved = rig.animate(&a, &s).unwrap();
            let twin = rig.animate(&b, &s).unwrap();
            assert_eq!(
                moved.positions,
                twin.positions.into_iter().rev().collect::<Vec<_>>()
            );
            close(moved.positions[0], neutral[0]);
            close(moved.positions[1], neutral[1]);
            if travel == 0. {
                assert_eq!(moved.positions, neutral);
            }
            if travel == 1. {
                assert_eq!(moved.positions, down);
            }
        }
    }
    #[test]
    fn nose_assembly_folds_rigidly_at_forward_attachment() {
        let source = synthetic(
            NOSE[0],
            vec![
                [0., 64., -8.],
                [0., 50., -8.],
                [0., 50., -26.],
                [0., 64., -26.],
            ],
        );
        for step in 0..=400 {
            let mut moved = source.clone();
            gear(&mut moved, step as f64 / 400.);
            close(source.positions[0], moved.positions[0]);
            for i in 0..4 {
                for j in i + 1..4 {
                    assert!(
                        (distance_squared(source.positions[i], source.positions[j])
                            - distance_squared(moved.positions[i], moved.positions[j]))
                        .abs()
                            < 0.001
                    );
                }
            }
        }
    }
    #[test]
    fn split_gear_keeps_roots_and_rigid_lower_sheets_through_twenty_one_poses() {
        // Deliberately synthetic sheet widths and longitudinal placement.
        let source = synthetic(
            MAIN_RIGHT[0],
            vec![
                [9., 1., -11.],
                [15., 1., -11.],
                [15., 1., -25.],
                [9., 1., -25.],
            ],
        );
        let pieces = split(&source, |p| p[2] + 18.);
        assert_eq!(pieces.len(), 2);
        for step in 0..=20 {
            let travel = step as f64 / 20.;
            let moved: Vec<_> = pieces.iter().map(|p| main_linkage(p, travel)).collect();
            for (before, after) in pieces.iter().zip(&moved) {
                assert!(after.positions.iter().all(|p| p[0] > 0.));
                for (a, b) in before.positions.iter().zip(&after.positions) {
                    if a[2] == -11. {
                        close(*a, *b);
                    }
                    if travel == 1. {
                        close(*a, *b);
                    }
                }
                if before.positions.iter().all(|p| p[2] <= -18.) {
                    for i in 0..before.positions.len() {
                        for j in 0..before.positions.len() {
                            assert!(
                                (distance_squared(before.positions[i], before.positions[j])
                                    - distance_squared(after.positions[i], after.positions[j]))
                                .abs()
                                    < 1e-3
                            );
                        }
                    }
                }
            }
            for (index, p) in pieces[0].positions.iter().enumerate() {
                if let Some(other) = pieces[1].positions.iter().position(|q| q == p) {
                    close(moved[0].positions[index], moved[1].positions[other]);
                }
            }
        }
    }
}
