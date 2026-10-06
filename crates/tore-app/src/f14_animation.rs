//! Exact F14.SH endpoints with documented, source-specific fitted mechanics.
//! Existing F14 geometry, sweep/vapor and donated-hook repairs are retained.
use crate::{AppResult, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 6] = [0x82e0, 0x82e6, 0x82ec, 0x82f8, 0x82fe, 0x8304];
const FLAME: [usize; 8] = [
    0x58c7, 0x58ee, 0x5915, 0x593c, 0x5963, 0x598a, 0x59b1, 0x59d8,
];
const BRAKE: [usize; 4] = [0x4c61, 0x4c78, 0x4c8f, 0x4ca6];
const RIGHT: [usize; 4] = [0x5509, 0x5528, 0x5547, 0x5566];
const LEFT: [usize; 4] = [0x55ca, 0x55e9, 0x5608, 0x5627];
const NOSE: [usize; 4] = [0x57a1, 0x57c0, 0x57df, 0x57fe];
const BRACE: [usize; 2] = [0x571e, 0x573d];
const PANEL: [usize; 2] = [0x56c3, 0x56da];
const HOOK: [usize; 2] = [0x5836, 0x584b];
const TAIL: [usize; 4] = [0x4828, 0x4888, 0x49c5, 0x4a22];
const FIN: [usize; 5] = [0x4a7b, 0x4a96, 0x4ac2, 0x4b19, 0x4b66];
const NOSE_ROOT: [f32; 3] = [0., 17., -1.];
const BRACE_ROOT: [f32; 3] = [0., 12., -1.];
const BRACE_JOINT: [f32; 3] = [0., 16., -2.5];
pub struct Rig {
    flaps: BTreeMap<usize, Vec<[f32; 3]>>,
}
pub fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}
impl Rig {
    pub fn load(bytes: &[u8], mut neutral: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 29462
            || neutral.faces.len() != 313
            || neutral.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F14.SH animation layout".into());
        }
        let original: BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
        let gear: Vec<_> = LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(PANEL)
            .collect();
        for (word, ids) in [
            (0x82e0, FLAME.as_slice()),
            (0x82e6, BRAKE.as_slice()),
            (0x82ec, gear.as_slice()),
            (0x82f8, HOOK.as_slice()),
        ] {
            let branch = Shape::with_state(bytes, &[(word, 1)].into())?;
            let active: BTreeSet<_> = branch.faces.iter().map(|f| f.address).collect();
            if !original.is_subset(&active)
                || active
                    .difference(&original)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != ids.iter().copied().collect()
            {
                return Err("unreviewed F14.SH device branch".into());
            }
            neutral.faces.extend(
                branch
                    .faces
                    .into_iter()
                    .filter(|f| ids.contains(&f.address)),
            );
        }
        let mut flaps = BTreeMap::new();
        for (word, bindings) in [
            (0x82fe, [(0x540d, 0x5486), (0x5434, 0x54a5)]),
            (0x8304, [(0x4fe9, 0x5062), (0x5010, 0x5081)]),
        ] {
            let down = Shape::with_state(bytes, &[(word, -1)].into())?;
            for (a, b) in bindings {
                let source = neutral
                    .faces
                    .iter()
                    .find(|f| f.address == a)
                    .ok_or("F14 flap missing")?;
                let target = down
                    .faces
                    .iter()
                    .find(|f| f.address == b)
                    .ok_or("F14 source flap endpoint missing")?;
                if source.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("unreviewed F14 flap topology".into());
                }
                let mapped = source
                    .positions
                    .iter()
                    .map(|p| {
                        target
                            .positions
                            .iter()
                            .find(|q| p[..2] == q[..2])
                            .copied()
                            .ok_or("F14 flap correspondence missing")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                flaps.insert(a, mapped);
            }
        }
        crate::f14_geometry::repair(&mut neutral)?;
        for f in neutral
            .faces
            .iter_mut()
            .filter(|f| matches!(f.address, 0x49c5 | 0x4a22))
        {
            for p in &mut f.positions {
                if p[0] == 6. && p[1] == -6. {
                    p[0] = 5.;
                }
            }
        }
        Ok((Self { flaps }, neutral))
    }
    pub fn faces(&self, f: &Face, s: &State) -> Vec<Face> {
        if !FIN.contains(&f.address) || s.rudder.abs() < 1e-8 {
            return vec![f.clone()];
        }
        let side = if f.positions[0][0] < 0. { -1. } else { 1. };
        let length = 53f64.sqrt();
        crate::aircraft_animation::split_surface(
            f,
            [side * 4., -11., 2.],
            [0., -2. / length, 7. / length],
            s.rudder.clamp(-1., 1.) * 0.35,
            |p| p[1] + 11. + (p[2] - 2.) * 2. / 7.,
        )
    }
    pub fn animate(&self, source: &Face, s: &State) -> Option<Face> {
        let mut f = source.clone();
        let a = f.address;
        let side = if f.positions.iter().map(|p| p[0]).sum::<f32>() < 0. {
            -1.
        } else {
            1.
        };
        if LEFT.contains(&a) || RIGHT.contains(&a) {
            if s.gear <= 0. {
                return None;
            }
            turn(
                &mut f,
                [side * 6., 1., 0.],
                [1., side, 0.],
                75f64.to_radians() * (1. - s.gear.clamp(0., 1.)),
            );
        } else if NOSE.contains(&a) {
            if s.gear <= 0. {
                return None;
            }
            nose(&mut f, s);
        } else if BRACE.contains(&a) {
            if s.gear <= 0. {
                return None;
            }
            follow_brace(source, &mut f, s);
        } else if PANEL.contains(&a) {
            if s.gear <= 0. {
                return None;
            }
            turn(
                &mut f,
                NOSE_ROOT,
                [0., 1., 0.],
                std::f64::consts::FRAC_PI_2 * (1. - s.gear.clamp(0., 1.)),
            );
        } else if BRAKE.contains(&a) {
            if s.brake <= 0. {
                return None;
            }
            turn(
                &mut f,
                [side * 2., -11., 1.],
                [1., side * 0.5, -side * 0.5],
                75f64.to_radians() * (1. - s.brake.clamp(0., 1.)),
            );
        } else if HOOK.contains(&a) {
            if s.hook <= 0. {
                return None;
            }
            turn(
                &mut f,
                [0., -5., -2.],
                [1., 0., 0.],
                -0.6 * (1. - s.hook.clamp(0., 1.)),
            );
        } else if flame(a) {
            if s.exhaust <= 0. {
                return None;
            }
            for p in &mut f.positions {
                p[1] = -14. + (p[1] + 14.) * s.exhaust as f32;
            }
        }
        if let Some(target) = self.flaps.get(&a) {
            let v = s.flaps.clamp(0., 1.) as f32;
            for (p, q) in f.positions.iter_mut().zip(target) {
                for i in 0..3 {
                    p[i] += (q[i] - p[i]) * v;
                }
            }
            if v != 0. {
                crate::aircraft_animation::update_normal(source, &mut f);
            }
        }
        if (0x4dba..=0x5434).contains(&a) {
            let angle = crate::additional_animation::sweep(s);
            for p in &mut f.positions {
                *p = crate::additional_animation::f14_wing_point(*p, side, angle);
            }
            // Retain the normal's original sweep rotation as well.
            if let Some(n) = &mut f.normal {
                let (sin, cos) = (-f64::from(side) * angle).sin_cos();
                let (x, y) = (n[0], n[2]);
                n[0] = x * cos as f32 - y * sin as f32;
                n[2] = x * sin as f32 + y * cos as f32;
            }
        }
        if TAIL.contains(&a) {
            // Synthetic callers and old retained shapes get the same repair.
            if matches!(a, 0x49c5 | 0x4a22) {
                for p in &mut f.positions {
                    if p[0] == 6. && p[1] == -6. {
                        p[0] = 5.;
                    }
                }
            }
            turn(
                &mut f,
                [side * 6., -10., 0.],
                [1., 0., 0.],
                -0.3 * s.elevator.clamp(-1., 1.) - f64::from(side) * 0.2 * s.aileron.clamp(-1., 1.),
            );
        }
        Some(f)
    }
}
fn turn(f: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    crate::additional_animation::turn(f, pivot, axis, angle);
}
fn nose(f: &mut Face, s: &State) {
    turn(
        f,
        NOSE_ROOT,
        [1., 0., 0.],
        110f64.to_radians() * (1. - s.gear.clamp(0., 1.)),
    );
    turn(f, NOSE_ROOT, [0., 0., 1.], -s.nosewheel_angle());
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn follow_brace(source: &Face, result: &mut Face, s: &State) {
    if s.gear == 1. && s.nosewheel_angle() == 0. {
        return;
    }
    let mut joint = source.clone();
    joint.positions = vec![BRACE_JOINT];
    nose(&mut joint, s);
    let from: [f32; 3] = std::array::from_fn(|i| BRACE_JOINT[i] - BRACE_ROOT[i]);
    let to: [f32; 3] = std::array::from_fn(|i| joint.positions[0][i] - BRACE_ROOT[i]);
    let old_length = dot(from, from).sqrt();
    let new_length = dot(to, to).sqrt();
    let a = from.map(|v| v / old_length);
    let b = to.map(|v| v / new_length);
    let k = cross(a, b);
    let c = dot(a, b);
    for (p, q) in source.positions.iter().zip(&mut result.positions) {
        let mut v: [f32; 3] = std::array::from_fn(|i| p[i] - BRACE_ROOT[i]);
        let stretch = dot(v, a) * (new_length / old_length - 1.);
        for i in 0..3 {
            v[i] += a[i] * stretch;
        }
        let first = cross(k, v);
        let second = cross(k, first);
        *q = std::array::from_fn(|i| BRACE_ROOT[i] + v[i] + first[i] + second[i] / (1. + c));
    }
    crate::aircraft_animation::update_normal(source, result);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rig() -> Rig {
        Rig {
            flaps: BTreeMap::new(),
        }
    }
    fn state() -> State {
        let mut s = State::new(&tore_world::test_support::profile(), [0., 5000., 0.]).unwrap();
        s.speed = 0.;
        s
    }
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let n = positions.len();
        Face {
            address,
            positions,
            colors: vec![23; n],
            uv: vec![[0.2, 0.8]; n],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([1., 0., 0.]),
            fog: Default::default(),
        }
    }
    fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
        dot(
            std::array::from_fn(|i| a[i] - b[i]),
            std::array::from_fn(|i| a[i] - b[i]),
        )
        .sqrt()
    }
    fn material(a: &Face, b: &Face) {
        assert_eq!(a.uv, b.uv);
        assert_eq!(a.colors, b.colors);
        assert_eq!(a.texture, b.texture);
        assert_eq!(a.subtype, b.subtype);
        assert!(b.positions.iter().flatten().all(|v| v.is_finite()));
    }
    fn rigid(a: &Face, b: &Face) {
        for i in 0..a.positions.len() {
            for j in i + 1..a.positions.len() {
                assert!(
                    (distance(a.positions[i], a.positions[j])
                        - distance(b.positions[i], b.positions[j]))
                    .abs()
                        < 1e-4
                );
            }
        }
        material(a, b);
    }
    #[test]
    fn tail_pitch_is_up_and_roll_opposes_with_shared_transverse_roots() {
        let r = rig();
        let mut s = state();
        for (a, side) in [(TAIL[0], -1.), (TAIL[2], 1.)] {
            let f = face(
                a,
                vec![
                    [side * 6., -10., 0.],
                    [side * 8., -10., 0.],
                    [side * 9., -15., 0.],
                ],
            );
            s.elevator = 1.;
            s.aileron = 0.;
            let g = r.animate(&f, &s).unwrap();
            assert_eq!(g.positions[0], f.positions[0]);
            assert_eq!(g.positions[1], f.positions[1]);
            assert!(g.positions[2][2] > 0.);
            rigid(&f, &g);
            s.elevator = 0.;
            s.aileron = 1.;
            let g = r.animate(&f, &s).unwrap();
            assert!(g.positions[2][2] * side > 0.);
            rigid(&f, &g);
        }
    }
    #[test]
    fn source_target_morph_preserves_front_and_only_changes_authored_trailing_point() {
        let f = face(
            0x540d,
            vec![
                [-19., -4., 1.],
                [-19., -3., 1.],
                [-4., -1., 1.],
                [-4., -3., 1.],
            ],
        );
        let mut target = f.positions.clone();
        target[3][2] = 0.;
        let r = Rig {
            flaps: [(f.address, target)].into(),
        };
        let mut s = state();
        for v in [0., 0.25, 0.5, 1.] {
            s.flaps = v;
            let g = r.animate(&f, &s).unwrap();
            for i in 0..3 {
                assert_eq!(
                    g.positions[i],
                    crate::additional_animation::f14_wing_point(f.positions[i], -1., 0.)
                );
            }
            assert!((g.positions[3][2] - (1.125 - v as f32)).abs() < 1e-5);
            material(&f, &g);
        }
    }
    #[test]
    fn wing_sweep_retains_compatibility_alignment_lift_and_front_root() {
        let r = rig();
        let mut s = state();
        s.speed = 700. * 1.68781;
        for (a, side) in [(0x5100, -1.), (0x4e00, 1.)] {
            let f = face(
                a,
                vec![
                    [side * 8. + if side < 0. { 1. } else { 0. }, 4., 1.],
                    [side * 21., -2., 1.],
                    [side * 13., -4., 1.],
                ],
            );
            let g = r.animate(&f, &s).unwrap();
            assert_eq!(g.positions[0], [side * 8., 4., 1.125]);
            for (p, q) in f.positions.iter().zip(&g.positions) {
                assert_eq!(
                    *q,
                    crate::additional_animation::f14_wing_point(
                        *p,
                        side,
                        crate::additional_animation::sweep(&s)
                    )
                );
            }
            rigid(&f, &g);
        }
    }
    #[test]
    fn split_fin_keeps_front_and_its_actual_hinge_at_both_yaw_signs() {
        let r = rig();
        let mut s = state();
        let f = face(
            FIN[2],
            vec![
                [-4., -6., 2.],
                [-4., -11., 2.],
                [-4., -13., 9.],
                [-4., -15., 9.],
            ],
        );
        for v in [-1., 1.] {
            s.rudder = v;
            let pieces = r.faces(&f, &s);
            for p in &f.positions[..3] {
                assert!(pieces.iter().any(|g| g.positions.contains(p)));
            }
            let moved = pieces
                .iter()
                .flat_map(|g| &g.positions)
                .find(|p| p[1] < -13.5)
                .unwrap();
            assert!((moved[0] + 4.) * v as f32 > 0.);
            for g in &pieces {
                assert_eq!(g.texture, f.texture);
                assert!(g.positions.iter().flatten().all(|v| v.is_finite()));
            }
        }
    }
    #[test]
    fn full_gear_assemblies_are_rigid_rooted_and_separate_across_401_poses() {
        let r = rig();
        let mut s = state();
        let wheels = [
            (LEFT[0], [-6., 1., 0.]),
            (RIGHT[0], [6., 1., 0.]),
            (NOSE[0], NOSE_ROOT),
        ]
        .map(|(a, p)| {
            face(
                a,
                vec![
                    p,
                    [p[0], p[1] + 1., p[2]],
                    [p[0], p[1] + 1., p[2] - 5.],
                    [p[0], p[1] - 1., p[2] - 5.],
                ],
            )
        });
        for i in 1..=401 {
            s.gear = f64::from(i) / 401.;
            let moved = wheels.each_ref().map(|f| r.animate(f, &s).unwrap());
            for (f, g) in wheels.iter().zip(&moved) {
                assert!(distance(f.positions[0], g.positions[0]) < 1e-4);
                rigid(f, g);
            }
            assert!(moved[0].positions.iter().all(|p| p[0] < -0.5));
            assert!(moved[1].positions.iter().all(|p| p[0] > 0.5));
        }
    }
    #[test]
    fn separate_brace_retains_upper_root_lower_wheel_joint_and_positive_area() {
        let r = rig();
        let mut s = state();
        let f = face(
            BRACE[0],
            vec![BRACE_ROOT, BRACE_ROOT, [0., 16., -3.], [0., 16., -2.]],
        );
        let wheel = face(NOSE[0], vec![BRACE_JOINT, [0., 17., -1.], [0., 17., -5.]]);
        let area = |f: &Face| {
            f.positions
                .iter()
                .zip(f.positions.iter().cycle().skip(1))
                .take(f.positions.len())
                .map(|(a, b)| a[1] * b[2] - b[1] * a[2])
                .sum::<f32>()
        };
        for i in 1..=401 {
            s.gear = f64::from(i) / 401.;
            let g = r.animate(&f, &s).unwrap();
            let w = r.animate(&wheel, &s).unwrap();
            assert_eq!(g.positions[0], BRACE_ROOT);
            assert_eq!(g.positions[1], BRACE_ROOT);
            assert!(
                distance(
                    std::array::from_fn(|k| (g.positions[2][k] + g.positions[3][k]) * 0.5),
                    w.positions[0]
                ) < 1e-4
            );
            assert!(area(&f) * area(&g) > 0.);
            material(&f, &g);
        }
    }
    #[test]
    fn separate_panel_and_brake_roots_remain_fixed_to_their_own_edges() {
        let r = rig();
        let mut s = state();
        let p = face(
            PANEL[0],
            vec![
                [0., 18., -1.],
                [0., 20., -1.],
                [0., 20., -2.],
                [0., 18., -2.],
            ],
        );
        let b = face(
            BRAKE[0],
            vec![[2., -11., 1.], [0., -12., 2.], [1., -13., 4.]],
        );
        for v in [0.000001, 0.25, 0.5, 1.] {
            s.gear = v;
            s.brake = v;
            for f in [&p, &b] {
                let g = r.animate(f, &s).unwrap();
                for i in 0..2 {
                    assert!(distance(f.positions[i], g.positions[i]) < 1e-4);
                }
                rigid(f, &g);
            }
        }
    }
    #[test]
    fn donated_hook_and_plume_keep_their_own_attachment_and_deployed_geometry() {
        let r = rig();
        let mut s = state();
        let h = face(
            HOOK[0],
            vec![[0., -5., -2.], [0., -12., -6.], [0., -12., -7.]],
        );
        let f = face(
            FLAME[0],
            vec![
                [4., -14., 1.],
                [4., -14., -1.],
                [4., -25., -1.],
                [4., -25., 1.],
            ],
        );
        for v in [0.25, 0.5, 1.] {
            s.hook = v;
            s.exhaust = v;
            let g = r.animate(&h, &s).unwrap();
            assert_eq!(g.positions[0], h.positions[0]);
            rigid(&h, &g);
            let g = r.animate(&f, &s).unwrap();
            assert_eq!(g.positions[..2], f.positions[..2]);
            material(&f, &g);
        }
    }
    #[test]
    fn inactive_devices_hide_only_at_zero_and_unrelated_body_is_static() {
        let r = rig();
        let mut s = state();
        s.gear = 0.;
        s.brake = 0.;
        s.hook = 0.;
        s.exhaust = 0.;
        for a in LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(PANEL)
            .chain(BRAKE)
            .chain(HOOK)
            .chain(FLAME)
        {
            assert!(
                r.animate(&face(a, vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]), &s)
                    .is_none()
            );
        }
        s.elevator = 1.;
        s.aileron = 1.;
        s.flaps = 1.;
        s.rudder = 1.;
        let f = face(0xf001, vec![[1., 2., 3.], [4., 2., 3.], [1., 6., 3.]]);
        assert_eq!(r.animate(&f, &s).unwrap().positions, f.positions);
    }
}
