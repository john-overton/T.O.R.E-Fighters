//! M17.SH source roots with explicitly fitted moving surfaces and gear linkage.
//! See docs/spec/variety-animation.md. Imported instructions never execute.
use crate::{AppResult, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::shape::{Face, Shape};

const WORDS: [usize; 5] = [0x6050, 0x6056, 0x605c, 0x6068, 0x606e];
const FLAME: [usize; 5] = [0x4c86, 0x4cad, 0x4cd4, 0x4cfb, 0x4d22];
const BRAKE: [usize; 10] = [
    0x44af, 0x44ce, 0x44f5, 0x4514, 0x4535, 0x45c2, 0x45e1, 0x4608, 0x4627, 0x4648,
];
const GEAR: [usize; 22] = [
    0x4690, 0x46af, 0x4713, 0x4732, 0x4751, 0x4770, 0x47bc, 0x47db, 0x483f, 0x485e, 0x487d, 0x489c,
    0x491c, 0x4933, 0x49b3, 0x49d2, 0x4a16, 0x4a35, 0x4aaf, 0x4ace, 0x4aed, 0x4b0c,
];
const YAW: [usize; 8] = [
    0x22b7, 0x3aea, 0x23e7, 0x4181, 0x2423, 0x41e4, 0x2495, 0x434a,
];
const YAW_TRAILING: [[f32; 3]; 3] = [[0., -50., 4.], [0., -63., 23.], [0., -60., 25.]];
const TAIL: [usize; 4] = [0x2f06, 0x2f27, 0x3b11, 0x3b44];
const ROLL_LEFT: [usize; 2] = [0x2f9a, 0x2fbd];
const ROLL_RIGHT: [usize; 2] = [0x3bb5, 0x3bdc];
const FLAPS: [usize; 4] = [0x2e24, 0x2f54, 0x33e8, 0x3b92];
const NOSE: [usize; 2] = [0x4a16, 0x4a35];
const NOSE_PIVOT: [f32; 3] = [0., 34.2973, -6.];

pub fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}
pub struct Rig {
    flame_root: f32,
}
fn face(shape: &Shape, address: usize) -> AppResult<&Face> {
    shape
        .faces
        .iter()
        .find(|f| f.address == address)
        .ok_or_else(|| format!("M17.SH lacks reviewed face {address:x}").into())
}
fn anchors(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &address in addresses {
        if !points
            .iter()
            .all(|p| face(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed M17.SH attachment at {address:x}").into());
        }
    }
    Ok(())
}
impl Rig {
    pub fn load(bytes: &[u8], mut neutral: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 20602
            || neutral.faces.len() != 273
            || neutral.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed M17.SH animation layout".into());
        }
        anchors(&neutral, &[0x22b7], &[[-1., -41., 4.], [-1., -57., 23.]])?;
        anchors(&neutral, &[0x3aea], &[[1., -41., 4.], [1., -57., 23.]])?;
        anchors(&neutral, &TAIL, &[[0., -43., 15.], [0., -55., 15.]])?;
        for (addresses, points) in [
            (ROLL_LEFT.as_slice(), [[-28., -21., -1.], [-38., -30., -2.]]),
            (ROLL_RIGHT.as_slice(), [[28., -21., -1.], [38., -30., -2.]]),
            (&FLAPS[..2], [[-13., -7., -1.], [-28., -21., -1.]]),
            (&FLAPS[2..], [[13., -7., -1.], [28., -21., -1.]]),
        ] {
            anchors(&neutral, addresses, &points)?;
        }
        for &address in &YAW {
            face(&neutral, address)?;
        }
        let original: BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
        let mut flame_root = f32::NEG_INFINITY;
        for (word, expected) in [
            (0x6050, FLAME.as_slice()),
            (0x6056, BRAKE.as_slice()),
            (0x605c, GEAR.as_slice()),
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
                return Err(format!("unreviewed M17.SH device branch {word:x}").into());
            }
            for f in pose
                .faces
                .into_iter()
                .filter(|f| expected.contains(&f.address))
            {
                if flame(f.address) {
                    for p in &f.positions {
                        flame_root = flame_root.max(p[1]);
                    }
                }
                neutral.faces.push(f);
            }
        }
        // The source's down panels are additive, and their front coordinates
        // differ from neutral. Validate their trailing targets, then retain
        // neutral front seams instead of overlaying both sets of skins.
        for (word, expected, side) in [
            (0x6068, [0x4be6, 0x4c0d, 0x4c2c], -1.),
            (0x606e, [0x4b56, 0x4b7d, 0x4b9c], 1.),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
            if !original.is_subset(&active)
                || active
                    .difference(&original)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != expected.into_iter().collect()
            {
                return Err("unreviewed M17.SH additive down-flap branch".into());
            }
            anchors(
                &down,
                &expected[..2],
                &[[side * 13., -7., -4.], [side * 28., -20., -5.]],
            )?;
        }
        anchors(
            &neutral,
            &[0x44ce, 0x44f5],
            &[[4., -30., -4.], [5., -31., 0.]],
        )?;
        anchors(
            &neutral,
            &[0x45e1, 0x4608],
            &[[-4., -30., -4.], [-5., -31., 0.]],
        )?;
        anchors(
            &neutral,
            &[0x4713, 0x4732],
            &[[20., 0., -2.], [20., 7., -2.]],
        )?;
        anchors(
            &neutral,
            &[0x483f, 0x485e],
            &[[-20., 0., -2.], [-20., 7., -2.]],
        )?;
        anchors(&neutral, &NOSE, &[[0., 29., -6.], [0., 36., -6.]])?;
        // The atlas's painted top attachment is UV225,143. The rectangle's
        // other top corners are transparent, so they are not mechanical roots.
        // Add an on-edge marker without changing the source surface or UV map.
        for source in neutral
            .faces
            .iter_mut()
            .filter(|f| NOSE.contains(&f.address))
        {
            if source.positions.len() != 4 || source.uv.len() != 4 {
                return Err("unreviewed M17.SH nose cutout topology".into());
            }
            let edge = (0..4)
                .find(|&i| source.positions[i][2] == -6. && source.positions[(i + 1) % 4][2] == -6.)
                .ok_or("M17.SH nose attachment edge missing")?;
            let next = (edge + 1) % 4;
            let uv = [source.uv[edge], source.uv[next]];
            if !uv.contains(&[216., 143.]) || !uv.contains(&[253., 143.]) {
                return Err("unreviewed M17.SH nose painted-root UV mapping".into());
            }
            let fraction = (NOSE_PIVOT[1] - source.positions[edge][1])
                / (source.positions[next][1] - source.positions[edge][1]);
            let color = (f32::from(source.colors[edge])
                + (f32::from(source.colors[next]) - f32::from(source.colors[edge])) * fraction)
                .round() as u8;
            source.positions.insert(edge + 1, NOSE_PIVOT);
            source.uv.insert(edge + 1, [225., 143.]);
            source.colors.insert(edge + 1, color);
        }
        Ok((Self { flame_root }, neutral))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let a = source.address;
        if YAW.contains(&a) {
            constrained_turn(
                source,
                &mut result,
                [0., -41., 4.],
                [0., -16., 19.],
                state.rudder.clamp(-1., 1.) * 0.35,
                |p| !YAW_TRAILING.contains(&p),
            );
        } else if TAIL.contains(&a) {
            constrained_turn(
                source,
                &mut result,
                [0., -49., 15.],
                [1., 0., 0.],
                -state.elevator.clamp(-1., 1.) * 0.30,
                |p| [[0., -43., 15.], [0., -55., 15.]].contains(&p),
            );
        } else if ROLL_LEFT.contains(&a) || ROLL_RIGHT.contains(&a) {
            let side = if ROLL_LEFT.contains(&a) { -1. } else { 1. };
            let root = [side * 28., if side < 0. { -12.5 } else { -13. }, -1.];
            let tip = [side * 42., -23., -2.5];
            constrained_turn(
                source,
                &mut result,
                root,
                std::array::from_fn(|i| tip[i] - root[i]),
                -state.aileron.clamp(-1., 1.) * 0.20,
                |p| ![[side * 28., -21., -1.], [side * 38., -30., -2.]].contains(&p),
            );
        } else if FLAPS.contains(&a) {
            let travel = state.flaps.clamp(0., 1.) as f32;
            for p in &mut result.positions {
                let side = p[0].signum();
                let target = if *p == [side * 13., -7., -1.] {
                    Some([side * 13., -7., -4.])
                } else if *p == [side * 28., -21., -1.] {
                    Some([side * 28., -20., -5.])
                } else {
                    None
                };
                if let Some(target) = target {
                    *p = std::array::from_fn(|i| p[i] + (target[i] - p[i]) * travel);
                }
            }
            if travel > 0. {
                update_normal(source, &mut result);
            }
        } else if BRAKE.contains(&a) {
            let travel = state.brake.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            if ![0x44af, 0x45c2].contains(&a) {
                let side = if a < 0x45c2 { 1. } else { -1. };
                let brace = [0x4514, 0x4535, 0x4627, 0x4648].contains(&a);
                constrained_turn(
                    source,
                    &mut result,
                    [side * 4., -30., -4.],
                    [side, -1., 4.],
                    -f64::from(side) * (6.5f64 / 6.).atan() * (1. - travel),
                    |p| brace && p != [side * 11., -36., -7.],
                );
            }
        } else if GEAR.contains(&a) {
            let travel = state.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            animate_gear(source, &mut result, 1. - travel);
        } else if flame(a) {
            let travel = state.exhaust.max(0.) as f32;
            if travel == 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = self.flame_root + (p[1] - self.flame_root) * travel;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
fn animate_gear(source: &Face, result: &mut Face, closing: f64) {
    let a = source.address;
    if NOSE.contains(&a) {
        // Whole painted wheel/strut folds rigidly about its atlas-supported
        // attachment. The -100-degree aft fold is fitted, not recovered.
        turn(
            result,
            NOSE_PIVOT,
            [1., 0., 0.],
            -100f64.to_radians() * closing,
        );
        return;
    }
    let motion = match a {
        0x4713 | 0x4732 => ([20., 3.5, -2.], [0., 1., 0.], FRAC_PI_2),
        0x483f | 0x485e => ([-20., 3.5, -2.], [0., 1., 0.], -FRAC_PI_2),
        0x4690 | 0x46af => ([7., 6., -2.], [0., 1., 0.], -FRAC_PI_2),
        0x47bc | 0x47db => ([-7., 6., -2.], [0., 1., 0.], FRAC_PI_2),
        0x4751 | 0x4770 => ([21., 5., -2.], [0., 1., 0.], FRAC_PI_2),
        0x487d | 0x489c => ([-21., 5., -2.], [0., 1., 0.], -FRAC_PI_2),
        0x491c | 0x4933 => ([2., 32., -6.], [0., 8., 1.], 0.70),
        0x49b3 | 0x49d2 => ([-2., 32., -6.], [0., 8., 1.], -0.70),
        _ => return,
    };
    turn(result, motion.0, motion.1, motion.2 * closing);
}

fn constrained_turn(
    source: &Face,
    result: &mut Face,
    pivot: [f32; 3],
    axis: [f32; 3],
    angle: f64,
    fixed: impl Fn([f32; 3]) -> bool,
) {
    if angle == 0. {
        return;
    }
    turn(result, pivot, axis, angle);
    for (a, b) in source.positions.iter().zip(&mut result.positions) {
        if fixed(*a) {
            *b = *a;
        }
    }
    update_normal(source, result);
}
fn turn(face: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    if angle == 0. {
        return;
    }
    let length = axis
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    let axis = axis.map(|v| f64::from(v) / length);
    let (sin, cos) = angle.sin_cos();
    let rotate = |p: [f32; 3]| -> [f32; 3] {
        let p = p.map(f64::from);
        let dot = axis.iter().zip(p).map(|(a, b)| a * b).sum::<f64>();
        let cross = [
            axis[1] * p[2] - axis[2] * p[1],
            axis[2] * p[0] - axis[0] * p[2],
            axis[0] * p[1] - axis[1] * p[0],
        ];
        std::array::from_fn(|i| (p[i] * cos + cross[i] * sin + axis[i] * dot * (1. - cos)) as f32)
    };
    for p in &mut face.positions {
        let v = rotate(std::array::from_fn(|i| p[i] - pivot[i]));
        *p = std::array::from_fn(|i| pivot[i] + v[i]);
    }
    if let Some(n) = face.normal {
        let n = rotate([n[0], n[2], n[1]]);
        face.normal = Some([n[0], n[2], n[1]]);
    }
}
fn polygon_normal(points: &[[f32; 3]]) -> Option<[f32; 3]> {
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
    (len > 1e-9).then(|| n.map(|v| (v / len) as f32))
}
fn update_normal(source: &Face, result: &mut Face) {
    let (Some(old), Some(reference), Some(mut n)) = (
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
        n = n.map(|v| -v);
    }
    result.normal = Some([n[0], n[2], n[1]]);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rig() -> Rig {
        Rig { flame_root: 0. }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn synthetic(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let len = positions.len();
        Face {
            address,
            positions,
            colors: vec![22; len],
            fog: Default::default(),
            uv: vec![[0.25, 0.75]; len],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([1., 0., 0.]),
        }
    }
    fn close(a: [f32; 3], b: [f32; 3]) {
        assert!(
            a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4),
            "{a:?} != {b:?}"
        );
    }
    fn material(a: &Face, b: &Face) {
        assert_eq!(a.colors, b.colors);
        assert_eq!(a.uv, b.uv);
        assert_eq!(a.texture, b.texture);
        assert_eq!(a.subtype, b.subtype);
        assert!(b.positions.iter().flatten().all(|v| v.is_finite()));
        assert!(b.normal.unwrap().iter().all(|v| v.is_finite()));
    }
    fn edge_lengths(f: &Face) -> Vec<f32> {
        f.positions
            .iter()
            .zip(f.positions.iter().cycle().skip(1))
            .take(f.positions.len())
            .map(|(a, b)| {
                a.iter()
                    .zip(b)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .collect()
    }
    #[test]
    fn thick_yaw_roots_stay_distinct_and_common_trailing_points_stay_joined() {
        let r = rig();
        let mut s = state();
        let left_roots = [[-1., -41., 4.], [-1., -57., 23.]];
        let right_roots = [[1., -41., 4.], [1., -57., 23.]];
        let f = synthetic(0x22b7, vec![left_roots[0], left_roots[1], YAW_TRAILING[0]]);
        let g = synthetic(
            0x3aea,
            vec![right_roots[0], right_roots[1], YAW_TRAILING[0]],
        );
        for input in [-1., -0.5, 0., 0.5, 1.] {
            s.rudder = input;
            let a = r.animate(&f, &s).unwrap();
            let b = r.animate(&g, &s).unwrap();
            for i in 0..2 {
                close(a.positions[i], left_roots[i]);
                close(b.positions[i], right_roots[i]);
            }
            close(a.positions[2], b.positions[2]);
            if input == 0. {
                assert_eq!(a.positions, f.positions);
                assert_eq!(b.positions, g.positions);
            } else {
                assert_eq!(a.positions[2][0].signum(), input.signum() as f32);
            }
            material(&f, &a);
            material(&g, &b);
        }
    }
    #[test]
    fn yaw_cap_and_underside_follow_the_same_trailing_motion_without_moving_forward_vertices() {
        let r = rig();
        let mut s = state();
        s.rudder = 0.8;
        let main = synthetic(
            0x22b7,
            vec![[-1., -41., 4.], [-1., -57., 23.], YAW_TRAILING[0]],
        );
        let cap = synthetic(
            0x2495,
            vec![[-1., -41., 4.], [0., -47., 3.], YAW_TRAILING[0]],
        );
        let moved = r.animate(&cap, &s).unwrap();
        close(moved.positions[0], cap.positions[0]);
        close(moved.positions[1], cap.positions[1]);
        close(
            moved.positions[2],
            r.animate(&main, &s).unwrap().positions[2],
        );
        let upper = synthetic(
            0x2423,
            vec![[-1., -57., 23.], [0., -56., 25.], YAW_TRAILING[2]],
        );
        let moved = r.animate(&upper, &s).unwrap();
        close(moved.positions[0], upper.positions[0]);
        close(moved.positions[1], upper.positions[1]);
        assert_ne!(moved.positions[2], upper.positions[2]);
    }
    #[test]
    fn pitch_roots_are_fixed_and_both_sides_have_matching_signs() {
        let r = rig();
        let mut s = state();
        for input in [-1., -0.5, 0., 0.5, 1.] {
            s.elevator = input;
            for (address, side) in [(TAIL[0], -1.), (TAIL[2], 1.)] {
                let f = synthetic(
                    address,
                    vec![[0., -43., 15.], [0., -55., 15.], [side * 12., -61., 15.]],
                );
                let moved = r.animate(&f, &s).unwrap();
                close(moved.positions[0], f.positions[0]);
                close(moved.positions[1], f.positions[1]);
                if input != 0. {
                    assert_eq!(
                        (moved.positions[2][2] - 15.).signum(),
                        input.signum() as f32
                    );
                }
                material(&f, &moved);
            }
        }
    }
    #[test]
    fn bounded_roll_is_opposed_and_does_not_move_forward_edges_or_other_controls() {
        let r = rig();
        let mut s = state();
        for input in [-1., -0.5, 0., 0.5, 1.] {
            s.aileron = input;
            for (address, side) in [(ROLL_LEFT[0], -1.), (ROLL_RIGHT[0], 1.)] {
                let root = [side * 28., -13., -2.];
                let tip = [side * 42., -23., -3.];
                let trail = [side * 28., -21., -1.];
                let f = synthetic(address, vec![root, tip, trail]);
                let moved = r.animate(&f, &s).unwrap();
                close(moved.positions[0], root);
                close(moved.positions[1], tip);
                if input != 0. {
                    assert_eq!(
                        (moved.positions[2][2] - trail[2]).signum(),
                        (input as f32 * side).signum()
                    );
                }
            }
            let f = synthetic(
                FLAPS[0],
                vec![[-13., -1., -1.], [-28., -13., -2.], [-28., -21., -1.]],
            );
            assert_eq!(r.animate(&f, &s).unwrap().positions, f.positions);
        }
    }
    #[test]
    fn flap_approximation_keeps_neutral_front_edges_and_uses_source_trailing_targets() {
        let r = rig();
        let mut s = state();
        let a = [-13., -1., -1.];
        let b = [-28., -13., -2.];
        let trail = [-28., -21., -1.];
        let f = synthetic(FLAPS[0], vec![a, b, trail]);
        let twin = synthetic(FLAPS[1], vec![[-13., 0., 0.], [-28., -12., -1.], trail]);
        for travel in [0., 0.25, 0.5, 0.75, 1.] {
            s.flaps = travel;
            let moved = r.animate(&f, &s).unwrap();
            let other = r.animate(&twin, &s).unwrap();
            close(moved.positions[0], a);
            close(moved.positions[1], b);
            close(other.positions[0], twin.positions[0]);
            close(other.positions[1], twin.positions[1]);
            close(moved.positions[2], other.positions[2]);
            if travel == 0. {
                assert_eq!(moved.positions, f.positions);
            }
            if travel == 1. {
                close(moved.positions[2], [-28., -20., -5.]);
            }
            material(&f, &moved);
        }
    }
    #[test]
    fn brake_brace_roots_and_shared_panel_tip_remain_coherent() {
        let r = rig();
        let mut s = state();
        let panel = synthetic(
            0x44ce,
            vec![
                [4., -30., -4.],
                [5., -31., 0.],
                [11., -36., -7.],
                [11., -38., -5.],
            ],
        );
        let brace = synthetic(
            0x4514,
            vec![
                [2., -31., -6.],
                [4., -30., -4.],
                [11., -36., -7.],
                [3., -31., -6.],
            ],
        );
        for travel in [0.1, 0.25, 0.5, 0.75, 1.] {
            s.brake = travel;
            let p = r.animate(&panel, &s).unwrap();
            let b = r.animate(&brace, &s).unwrap();
            close(p.positions[0], panel.positions[0]);
            close(p.positions[1], panel.positions[1]);
            close(b.positions[0], brace.positions[0]);
            close(b.positions[1], brace.positions[1]);
            close(p.positions[2], b.positions[2]);
            if travel == 1. {
                assert_eq!(p.positions, panel.positions);
                assert_eq!(b.positions, brace.positions);
            } else {
                assert!(p.positions[2][0] < panel.positions[2][0]);
            }
        }
        s.brake = 0.;
        assert!(r.animate(&panel, &s).is_none());
    }
    #[test]
    fn main_wheel_panels_hold_roots_remain_rigid_and_do_not_cross_at_21_poses() {
        for sample in 0..=20 {
            let closing = sample as f64 / 20.;
            let mut separation = 0.;
            for (address, side) in [(0x4713, 1.), (0x483f, -1.)] {
                let f = synthetic(
                    address,
                    vec![
                        [side * 20., 0., -2.],
                        [side * 20., 7., -2.],
                        [side * 20., 7., -11.],
                        [side * 20., 0., -11.],
                    ],
                );
                let mut moved = f.clone();
                animate_gear(&f, &mut moved, closing);
                close(moved.positions[0], f.positions[0]);
                close(moved.positions[1], f.positions[1]);
                for (a, b) in edge_lengths(&f).iter().zip(edge_lengths(&moved)) {
                    assert!((a - b).abs() < 1e-4);
                }
                let nearest_center = moved
                    .positions
                    .iter()
                    .map(|p| p[0] * side)
                    .fold(f32::INFINITY, f32::min);
                assert!(nearest_center > 1.05);
                separation += nearest_center;
                if closing == 1. {
                    assert!(moved.positions.iter().all(|p| (p[2] + 2.).abs() < 1e-4
                        && (9. - 1e-4..=20. + 1e-4).contains(&p[0].abs())));
                }
                material(&f, &moved);
            }
            assert!(separation > 2.1);
        }
    }
    #[test]
    fn whole_nose_assembly_is_rigid_and_holds_the_painted_root_at_21_poses() {
        let r = rig();
        let mut state = state();
        let source = synthetic(
            NOSE[0],
            vec![
                [0., 30., -13.],
                [0., 36., -13.],
                [0., 36., -6.],
                NOSE_PIVOT,
                [0., 30., -6.],
            ],
        );
        for sample in 0..=20 {
            let closing = sample as f64 / 20.;
            let mut moved = source.clone();
            animate_gear(&source, &mut moved, closing);
            close(moved.positions[3], NOSE_PIVOT);
            for i in 0..source.positions.len() {
                for j in i + 1..source.positions.len() {
                    let distance = |a: [f32; 3], b: [f32; 3]| {
                        a.iter()
                            .zip(b)
                            .map(|(a, b)| (a - b).powi(2))
                            .sum::<f32>()
                            .sqrt()
                    };
                    assert!(
                        (distance(source.positions[i], source.positions[j])
                            - distance(moved.positions[i], moved.positions[j]))
                        .abs()
                            < 1e-4
                    );
                }
            }
            if closing > 0. {
                assert_ne!(moved.positions[2], source.positions[2]);
                assert_ne!(moved.positions[4], source.positions[4]);
            }
            if closing == 1. {
                assert!(
                    moved
                        .positions
                        .iter()
                        .all(|p| (26. ..=36.).contains(&p[1]) && (-8. ..=1.).contains(&p[2]))
                );
            }
            material(&source, &moved);
            state.gear = (1. - closing).max(1e-6);
            assert!(r.animate(&source, &state).is_some());
        }
        state.gear = 0.;
        assert!(r.animate(&source, &state).is_none());
    }
    #[test]
    fn original_deployed_gear_and_unrelated_body_materials_are_unchanged() {
        let r = rig();
        let mut s = state();
        s.gear = 1.;
        let panel = synthetic(
            NOSE[0],
            vec![
                [0., 29., -6.],
                [0., 36., -6.],
                [0., 35., -11.],
                [0., 31., -11.],
            ],
        );
        assert_eq!(r.animate(&panel, &s).unwrap().positions, panel.positions);
        let body = synthetic(1, vec![[3., 2., 1.], [4., 1., 2.], [2., 2., 3.]]);
        s.elevator = 1.;
        s.aileron = 1.;
        s.rudder = 1.;
        s.flaps = 1.;
        s.brake = 1.;
        assert_eq!(r.animate(&body, &s).unwrap().positions, body.positions);
    }
    #[test]
    fn rigid_nose_fold_never_crosses_or_collapses_and_twins_remain_coherent() {
        fn crossed(face: &Face) -> bool {
            let points: Vec<_> = face.positions.iter().map(|p| [p[1], p[2]]).collect();
            let orient = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
                (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
            };
            let intersects = |a, b, c, d| {
                orient(a, b, c) * orient(a, b, d) < -1e-8
                    && orient(c, d, a) * orient(c, d, b) < -1e-8
            };
            for i in 0..points.len() {
                let next_i = (i + 1) % points.len();
                for j in i + 1..points.len() {
                    let next_j = (j + 1) % points.len();
                    if next_i == j || next_j == i {
                        continue;
                    }
                    if intersects(points[i], points[next_i], points[j], points[next_j]) {
                        return true;
                    }
                }
            }
            false
        }
        // Deliberately invented coordinates with the old connector's topology.
        let bowtie = synthetic(
            NOSE[0],
            vec![[0., 0., 0.], [0., 6., 0.], [0., 0.5, 2.], [0., 6.5, 2.]],
        );
        assert!(crossed(&bowtie));
        let source = synthetic(
            NOSE[0],
            vec![
                [0., 30., -13.],
                [0., 36., -13.],
                [0., 36., -6.],
                NOSE_PIVOT,
                [0., 30., -6.],
            ],
        );
        let twin = synthetic(NOSE[1], source.positions.iter().rev().copied().collect());
        let mut samples: Vec<_> = (0..=20).map(|step| step as f64 / 20.).collect();
        samples.extend([0.20, 0.25, 0.30, 2. / 8.8]);
        for closing in samples {
            let mut moved = source.clone();
            let mut other = twin.clone();
            animate_gear(&source, &mut moved, closing);
            animate_gear(&twin, &mut other, closing);
            assert!(!crossed(&moved), "connector crossed at closing={closing}");
            assert!(!crossed(&other));
            close(moved.positions[3], NOSE_PIVOT);
            let area = |face: &Face| {
                face.positions
                    .iter()
                    .zip(face.positions.iter().cycle().skip(1))
                    .take(face.positions.len())
                    .map(|(a, b)| a[1] * b[2] - b[1] * a[2])
                    .sum::<f32>()
                    .abs()
                    * 0.5
            };
            assert!((area(&moved) - area(&source)).abs() < 1e-3);
            for (a, b) in moved.positions.iter().zip(other.positions.iter().rev()) {
                close(*a, *b);
            }
            material(&source, &moved);
        }
    }
}
