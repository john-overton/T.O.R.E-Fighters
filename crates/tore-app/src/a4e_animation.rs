//! Exact A4.SH geometry with fitted own seams and rigid whole gear assemblies.
//! Unmapped forward flap-branch strips remain an explicit research gap.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 5] = [0x6f90, 0x6f96, 0x6fa2, 0x6fa8, 0x6fae];
const GEAR: [usize; 18] = [
    0x528a, 0x52aa, 0x52d0, 0x52f9, 0x5354, 0x5374, 0x53ff, 0x5425, 0x5446, 0x546e, 0x54c9, 0x54e9,
    0x5543, 0x5563, 0x55cc, 0x55f4, 0x564f, 0x566f,
];
const BRAKES: [usize; 12] = [
    0x5824, 0x584a, 0x586c, 0x589c, 0x58bc, 0x58e2, 0x599f, 0x59c5, 0x59e7, 0x5a17, 0x5a3d, 0x5a66,
];
const RUDDER: [usize; 2] = [0x455c, 0x457d];
const RIGHT_ROLL: [usize; 3] = [0x376a, 0x37d4, 0x37f4];
const LEFT_ROLL: [usize; 3] = [0x3ae9, 0x3ba2, 0x3c8d];
const PIVOT: [f32; 3] = [0., -277. / 11., -83. / 11.];
const EPS: f32 = 1e-4;
struct Morph {
    neutral: Face,
    target: Face,
}
struct Hook {
    neutral: Face,
    target: Face,
    turned: Face,
    front: [usize; 2],
    marker: usize,
    angle: f64,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
    caps: BTreeMap<usize, Option<Morph>>,
    hooks: BTreeMap<usize, Hook>,
}
fn one(shape: &Shape, a: usize) -> AppResult<&Face> {
    let mut rows = shape.faces.iter().filter(|f| f.address == a);
    let f = rows
        .next()
        .ok_or_else(|| format!("A4.SH missing source face {a:x}"))?;
    if rows.next().is_some() {
        return Err("A4.SH duplicate source face".into());
    }
    Ok(f)
}
fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let s = Shape::with_state(bytes, &[(word, value)].into())?;
    let ids: BTreeSet<_> = s.faces.iter().map(|f| f.address).collect();
    if ids.difference(neutral).copied().collect::<BTreeSet<_>>() != added.iter().copied().collect()
        || neutral.difference(&ids).copied().collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed A4.SH branch {word:x}={value}").into());
    }
    Ok(s)
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
fn update(source: &Face, f: &mut Face) {
    crate::aircraft_animation::update_normal(source, f);
}
fn marker(mut f: Face, edge: [usize; 2]) -> AppResult<Face> {
    if f.positions.len() != 4 || f.uv.len() != 4 || f.colors.len() != 4 {
        return Err("A4.SH source hook topology changed".into());
    }
    let a = f.positions[edge[0]];
    let b = f.positions[edge[1]];
    let d: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let denom = d.iter().map(|v| v * v).sum::<f32>();
    let t = (0..3).map(|i| (PIVOT[i] - a[i]) * d[i]).sum::<f32>() / denom;
    let q = std::array::from_fn(|i| a[i] + t * d[i]);
    if !(0. ..=1.).contains(&t)
        || distance(q, PIVOT) > EPS
        || f.uv.len() != 4
        || f.colors.len() != 4
        || f.colors[edge[0]] != f.colors[edge[1]]
    {
        return Err("A4.SH source hook root correspondence changed".into());
    }
    let uv = std::array::from_fn(|i| f.uv[edge[0]][i] + t * (f.uv[edge[1]][i] - f.uv[edge[0]][i]));
    let color = f.colors[edge[0]];
    f.positions.insert(edge[1], PIVOT);
    f.uv.insert(edge[1], uv);
    f.colors.insert(edge[1], color);
    Ok(f)
}
impl Hook {
    fn new(neutral: Face, mut target: Face, edge: [usize; 2]) -> AppResult<Self> {
        target.address = neutral.address;
        let neutral = marker(neutral, edge)?;
        let target = marker(target, edge)?;
        let front = [edge[0], edge[1] + 1];
        let a = neutral.positions[front[0]];
        let b = target.positions[front[0]];
        let ay = f64::from(a[1] - PIVOT[1]);
        let az = f64::from(a[2] - PIVOT[2]);
        let by = f64::from(b[1] - PIVOT[1]);
        let bz = f64::from(b[2] - PIVOT[2]);
        let angle = (ay * bz - az * by).atan2(ay * by + az * bz);
        let mut turned = neutral.clone();
        turn(
            &mut turned,
            PIVOT,
            [1., 0., 0.],
            std::f64::consts::FRAC_PI_3,
        );
        Ok(Self {
            neutral,
            target,
            turned,
            front,
            marker: edge[1],
            angle,
        })
    }
    fn pose(&self, amount: f64) -> Face {
        let t = amount.clamp(0., 1.);
        if t == 0. {
            return self.neutral.clone();
        }
        if t == 1. {
            return self.target.clone();
        }
        let mut f = self.neutral.clone();
        turn(&mut f, PIVOT, [1., 0., 0.], std::f64::consts::FRAC_PI_3 * t);
        for (i, p) in f.positions.iter_mut().enumerate() {
            for (k, c) in p.iter_mut().enumerate() {
                *c += (self.target.positions[i][k] - self.turned.positions[i][k]) * t as f32;
            }
        }
        let mut front_pose = self.neutral.clone();
        turn(&mut front_pose, PIVOT, [1., 0., 0.], self.angle * t);
        for i in self.front {
            let initial = distance(self.neutral.positions[i], PIVOT);
            let target = distance(self.target.positions[i], PIVOT);
            let length = initial + (target - initial) * t as f32;
            f.positions[i] = std::array::from_fn(|k| {
                PIVOT[k] + (front_pose.positions[i][k] - PIVOT[k]) * length / initial
            });
        }
        f.positions[self.marker] = PIVOT;
        let a = self.front[0];
        let b = self.front[1];
        let ratio = distance(f.positions[a], PIVOT) / distance(f.positions[a], f.positions[b]);
        f.uv[self.marker] = std::array::from_fn(|k| f.uv[a][k] + ratio * (f.uv[b][k] - f.uv[a][k]));
        update(&self.neutral, &mut f);
        f
    }
}
impl Morph {
    fn pose(&self, amount: f64) -> Face {
        let t = amount.clamp(0., 1.) as f32;
        if t == 0. {
            return self.neutral.clone();
        }
        if t == 1. {
            return self.target.clone();
        }
        let mut f = self.neutral.clone();
        f.positions = self
            .neutral
            .positions
            .iter()
            .zip(&self.target.positions)
            .map(|(p, q)| std::array::from_fn(|i| p[i] + t * (q[i] - p[i])))
            .collect();
        f.uv = self
            .neutral
            .uv
            .iter()
            .zip(&self.target.uv)
            .map(|(p, q)| std::array::from_fn(|i| p[i] + t * (q[i] - p[i])))
            .collect();
        update(&self.neutral, &mut f);
        f
    }
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 24506
            || shape.faces.len() != 237
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed exact A4E/A4.SH layout".into());
        }
        let ids: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for a in RUDDER
            .into_iter()
            .chain(RIGHT_ROLL)
            .chain(LEFT_ROLL)
            .chain([0x44b5, 0x44d5, 0x4982, 0x49f8])
        {
            one(&shape, a)?;
        }
        let mut flaps = BTreeMap::new();
        let mut caps = BTreeMap::new();
        let mut added = Vec::new();
        for (word, all, pairs, moving, fixed) in [
            (
                0x6fa8,
                [
                    0x4d5c, 0x4d7d, 0x4d94, 0x4de6, 0x4e0f, 0x4e26, 0x50c6, 0x50dd, 0x50f4, 0x511d,
                    0x514b, 0x5167,
                ],
                [(0x51c6, 0x511d), (0x51ef, 0x50f4)],
                [0x50c6, 0x50dd],
                [0x514b, 0x5167],
            ),
            (
                0x6fae,
                [
                    0x4bd4, 0x4bf5, 0x4c16, 0x4c5e, 0x4c75, 0x4c9e, 0x4ede, 0x4ef5, 0x4f1e, 0x4f35,
                    0x4f63, 0x4f7f,
                ],
                [(0x4fde, 0x4f35), (0x5007, 0x4ef5)],
                [0x4ede, 0x4f1e],
                [0x4f63, 0x4f7f],
            ),
        ] {
            let removed = pairs.map(|p| p.0);
            let down = branch(bytes, &ids, word, -1, &all, &removed)?;
            branch(bytes, &ids, word, 1, &[], &removed)?;
            for (a, b) in pairs {
                let neutral = one(&shape, a)?.clone();
                let mut target = one(&down, b)?.clone();
                target.address = a;
                if neutral.positions.len() != 4
                    || target.positions.len() != 4
                    || neutral.uv.len() != 4
                    || target.uv.len() != 4
                {
                    return Err("A4.SH flap topology changed".into());
                }
                for (p, q) in neutral.positions.iter().zip(&mut target.positions) {
                    if p[1] == -18. {
                        *q = *p;
                    }
                }
                update(one(&down, b)?, &mut target);
                flaps.insert(a, Morph { neutral, target });
            }
            for a in moving {
                let mut target = one(&down, a)?.clone();
                for p in &mut target.positions {
                    if p[1] == -17. && p[2] == -8. {
                        p[1] = -18.;
                    }
                }
                let mut neutral = target.clone();
                for p in &mut neutral.positions {
                    if p[1] == -23. && p[2] == -12. {
                        p[1] = if p[0].abs() == 5. { -26. } else { -25. };
                        p[2] = -8.;
                    }
                }
                added.push(target.clone());
                caps.insert(a, Some(Morph { neutral, target }));
            }
            for a in fixed {
                added.push(one(&down, a)?.clone());
                caps.insert(a, None);
            }
            // The other six added forward strips have no reviewed continuous mapping.
        }
        let down = branch(bytes, &ids, 0x6fa2, 1, &[0x56ca, 0x56eb], &[0x573f, 0x575f])?;
        let mut hooks = BTreeMap::new();
        for (a, b, edge) in [(0x573f, 0x56eb, [2, 3]), (0x575f, 0x56ca, [0, 1])] {
            hooks.insert(
                a,
                Hook::new(one(&shape, a)?.clone(), one(&down, b)?.clone(), edge)?,
            );
        }
        for f in &mut shape.faces {
            if let Some(h) = hooks.get(&f.address) {
                *f = h.neutral.clone();
            }
        }
        for (word, group) in [(0x6f90, &BRAKES[..]), (0x6f96, &GEAR[..])] {
            let down = branch(bytes, &ids, word, 1, group, &[])?;
            added.extend(
                down.faces
                    .into_iter()
                    .filter(|f| group.contains(&f.address)),
            );
        }
        shape.faces.extend(added);
        Ok((Self { flaps, caps, hooks }, shape))
    }
    pub fn animate(&self, source: &Face, s: &State) -> Option<Face> {
        let a = source.address;
        if let Some(h) = self.hooks.get(&a) {
            return Some(h.pose(s.hook));
        }
        if let Some(m) = self.flaps.get(&a) {
            return Some(m.pose(s.flaps));
        }
        if let Some(cap) = self.caps.get(&a) {
            return (s.flaps > 0.).then(|| {
                cap.as_ref()
                    .map_or_else(|| source.clone(), |m| m.pose(s.flaps))
            });
        }
        let mut f = source.clone();
        if GEAR.contains(&a) {
            if s.gear <= 0. {
                return None;
            }
            gear(&mut f, s.gear);
            if matches!(a, 0x564f | 0x566f) && s.nosewheel_angle() != 0. {
                turn(&mut f, [0., 27., -8.], [0., 0., 1.], -s.nosewheel_angle());
            }
            return Some(f);
        }
        if BRAKES.contains(&a) {
            if s.brake <= 0. {
                return None;
            }
            brake(source, &mut f, s.brake);
            return Some(f);
        }
        if RUDDER.contains(&a) && s.rudder != 0. {
            turn(
                &mut f,
                [0., -45., 11.],
                [0., -10., 16.],
                0.35 * s.rudder.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if q[1] == -45. || *q == [0., -55., 27.] {
                    *p = *q;
                }
            }
            update(source, &mut f);
        } else if matches!(a, 0x44b5 | 0x44d5 | 0x4982 | 0x49f8) && s.elevator != 0. {
            let axis = if matches!(a, 0x44b5 | 0x44d5) {
                [17., 5., 0.]
            } else {
                [16., -5., 0.]
            };
            turn(
                &mut f,
                [0., -59., 7.],
                axis,
                -0.3 * s.elevator.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if *q == [0., -59., 7.] || q[1] == -54. {
                    *p = *q;
                }
            }
        } else if (RIGHT_ROLL.contains(&a) || LEFT_ROLL.contains(&a)) && s.aileron != 0. {
            let side = if RIGHT_ROLL.contains(&a) { 1. } else { -1. };
            turn(
                &mut f,
                [side * 21., -18., -7.],
                [18., side * 2., -side],
                -f64::from(side) * 0.2 * s.aileron.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if q[1] >= -18. {
                    *p = *q;
                }
            }
            update(source, &mut f);
        }
        Some(f)
    }
}
fn gear(f: &mut Face, amount: f64) {
    let c = 1. - amount.clamp(0., 1.);
    if c == 0. {
        return;
    }
    let a = f.address;
    if matches!(a, 0x5354 | 0x5374 | 0x54c9 | 0x54e9) {
        let side = if matches!(a, 0x5354 | 0x5374) {
            1.
        } else {
            -1.
        };
        let pivot = [side * 9., -8.5, -8.];
        turn(
            f,
            pivot,
            [1., 0., 0.],
            std::f64::consts::FRAC_PI_2 * (2. * c).min(1.),
        );
        if c > 0.5 {
            let half = 115f64.to_radians() / 2.;
            let factor = half.sin() / 769f64.sqrt();
            let (w, x, y, z) = (half.cos(), 15. * factor, 20. * factor, -12. * factor);
            let axis = [
                ((x - w) / 2f64.sqrt()) as f32,
                side * ((y - z) / 2f64.sqrt()) as f32,
                side * ((z + y) / 2f64.sqrt()) as f32,
            ];
            let angle = 2. * ((w + x) / 2f64.sqrt()).acos();
            turn(f, pivot, axis, angle * (2. * c - 1.));
        }
        return;
    }
    if matches!(a, 0x5543 | 0x5563 | 0x564f | 0x566f) {
        turn(f, [0., 27., -8.], [1., 0., 0.], -100f64.to_radians() * c);
        return;
    }
    let door = ((c - 0.75) / 0.25).clamp(0., 1.);
    let side = if matches!(a, 0x528a | 0x52aa | 0x52d0 | 0x52f9) {
        1.
    } else {
        -1.
    };
    let (pivot, axis, angle) = if matches!(a, 0x528a | 0x52aa | 0x53ff | 0x5446) {
        (
            [side * 11., 0., -6.],
            [0., 1., 0.],
            f64::from(side) * (std::f64::consts::PI - 7f64.atan()),
        )
    } else if matches!(a, 0x52d0 | 0x52f9 | 0x5425 | 0x546e) {
        (
            [side * 11., 0., -7.],
            [0., 1., 0.],
            f64::from(side) * std::f64::consts::FRAC_PI_2,
        )
    } else {
        (
            [-1., 24., -8.],
            [0., 18., -1.],
            -std::f64::consts::FRAC_PI_2,
        )
    };
    turn(f, pivot, axis, angle * door);
}
fn brake(source: &Face, f: &mut Face, amount: f64) {
    let a = f.address;
    let side = if a < 0x5900 { 1. } else { -1. };
    let moving = matches!(a, 0x589c | 0x58bc | 0x5a17 | 0x5a66);
    let actuator = matches!(a, 0x5824 | 0x586c | 0x59c5 | 0x59e7);
    if !moving && !actuator {
        return;
    }
    turn(
        f,
        [side * 5., -30., -3.],
        [0., 0., 1.],
        -f64::from(side) * 75f64.to_radians() * (1. - amount),
    );
    if actuator {
        for (p, q) in f.positions.iter_mut().zip(&source.positions) {
            if q[1] == -35. {
                *p = *q;
            }
        }
        update(source, f);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
            caps: BTreeMap::new(),
            hooks: BTreeMap::new(),
        }
    }
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let count = positions.len();
        Face {
            address,
            positions,
            colors: vec![0; count],
            uv: vec![[0., 0.]; count],
            texture: String::new(),
            subtype: 0x64,
            normal: None,
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    #[test]
    fn rudder_keeps_thick_own_roots_and_moves_shared_rear_together() {
        let mut s = state();
        s.rudder = 1.;
        let r = rig();
        let mut result = Vec::new();
        for (a, x) in [(0x455c, -1.), (0x457d, 1.)] {
            let f = face(
                a,
                vec![
                    [x, -45., 11.],
                    [0., -55., 27.],
                    [0., -60., 27.],
                    [0., -60., 12.],
                ],
            );
            let q = r.animate(&f, &s).unwrap();
            assert_eq!(q.positions[0], f.positions[0]);
            assert_eq!(q.positions[1], f.positions[1]);
            assert!(q.positions[2][0] > 0.);
            result.push(q);
        }
        assert_eq!(result[0].positions[2..], result[1].positions[2..]);
    }
    #[test]
    fn roll_sign_and_all_source_fronts_are_preserved() {
        let mut s = state();
        s.aileron = 1.;
        let r = rig();
        for (a, side) in [(0x376a, 1.), (0x3ae9, -1.)] {
            let f = face(
                a,
                vec![
                    [side * 21., -18., -6.],
                    [side * 35., -18., -7.],
                    [side * 40., -25., -8.],
                    [side * 21., -25., -8.],
                ],
            );
            let q = r.animate(&f, &s).unwrap();
            assert_eq!(&q.positions[..2], &f.positions[..2]);
            assert!((q.positions[2][2] + 8.) * side > 0.);
            assert!((q.positions[3][2] + 8.) * side > 0.);
        }
    }
    #[test]
    fn source_hook_ends_fixed_marker_and_bounded_shape_change() {
        let mut n = face(
            0x573f,
            vec![
                [0., -46., -7.],
                [0., -45., -4.],
                [0., -25., -7.],
                [0., -26., -10.],
            ],
        );
        n.uv = vec![[0., 0.], [0., 1.], [1., 1.], [1., 0.]];
        let mut d = n.clone();
        d.positions = vec![
            [0., -36., -25.],
            [0., -38., -23.],
            [0., -26., -7.],
            [0., -23., -9.],
        ];
        let h = Hook::new(n, d, [2, 3]).unwrap();
        assert_eq!(h.pose(0.).positions, h.neutral.positions);
        assert_eq!(h.pose(1.).positions, h.target.positions);
        for sample in 0..=200 {
            let f = h.pose(f64::from(sample) / 200.);
            assert_eq!(f.positions[h.marker], PIVOT);
            let a = f.positions[h.front[0]];
            let b = f.positions[h.front[1]];
            let cross =
                (a[1] - PIVOT[1]) * (b[2] - PIVOT[2]) - (a[2] - PIVOT[2]) * (b[1] - PIVOT[1]);
            assert!(cross.abs() < 1e-4);
            for i in 0..5 {
                for j in i + 1..5 {
                    assert!(
                        (distance(f.positions[i], f.positions[j])
                            - distance(h.neutral.positions[i], h.neutral.positions[j]))
                        .abs()
                            < 1.
                    );
                }
            }
        }
    }
    #[test]
    fn staged_whole_main_cards_remain_rigid_and_separated() {
        let f = face(
            0x5354,
            vec![
                [9., -5., -24.],
                [9., -11., -24.],
                [9., -11., -8.],
                [9., -5., -8.],
            ],
        );
        for sample in 0..=200 {
            let g = f64::from(sample) / 200.;
            let mut right = f.clone();
            let mut left = f.clone();
            left.address = 0x54c9;
            for p in &mut left.positions {
                p[0] = -p[0];
            }
            gear(&mut right, g);
            gear(&mut left, g);
            let gap = right
                .positions
                .iter()
                .map(|p| p[0])
                .fold(f32::INFINITY, f32::min)
                - left
                    .positions
                    .iter()
                    .map(|p| p[0])
                    .fold(f32::NEG_INFINITY, f32::max);
            assert!(gap > 3.);
            for i in 0..4 {
                for j in i + 1..4 {
                    assert!(
                        (distance(f.positions[i], f.positions[j])
                            - distance(right.positions[i], right.positions[j]))
                        .abs()
                            < EPS
                    );
                }
            }
        }
    }
    #[test]
    fn brakes_keep_body_eyes_and_follow_observed_leaf_ends() {
        let source = face(
            0x5824,
            vec![
                [9., -32., -4.],
                [5., -35., -4.],
                [5., -35., -3.],
                [9., -32., -3.],
            ],
        );
        let pin = face(
            0x589c,
            vec![[9., -32., -4.], [9., -32., -3.], [5., -30., 0.]],
        );
        for sample in 0..=20 {
            let value = f64::from(sample) / 20.;
            let mut actuator = source.clone();
            let mut leaf = pin.clone();
            brake(&source, &mut actuator, value);
            brake(&pin, &mut leaf, value);
            assert_eq!(actuator.positions[1], source.positions[1]);
            assert_eq!(actuator.positions[2], source.positions[2]);
            assert_eq!(actuator.positions[0], leaf.positions[0]);
            assert_eq!(actuator.positions[3], leaf.positions[1]);
        }
    }
    #[test]
    fn door_waits_for_wheel_then_keeps_its_own_edge() {
        let f = face(
            0x528a,
            vec![
                [12., 9., -13.],
                [12., 0., -13.],
                [11., 0., -6.],
                [11., 9., -6.],
            ],
        );
        let mut q = f.clone();
        gear(&mut q, 0.5);
        assert_eq!(q.positions, f.positions);
        gear(&mut q, 0.000001);
        for i in [2, 3] {
            assert!(distance(q.positions[i], f.positions[i]) < EPS);
        }
        assert!(q.positions[0][0] < 5.);
    }
}
