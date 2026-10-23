//! E2C.SH four fitted rudder strips, source flaperons and rigid gear/hook motion.
//! Pivot choices and mixing are fitted; source code is never executed.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 5] = [0x8220, 0x8226, 0x8232, 0x8238, 0x823e];
const OUTER_LEFT: [usize; 4] = [0x3d5a, 0x3d75, 0x3e0f, 0x3ea1];
const OUTER_RIGHT: [usize; 4] = [0x2f4b, 0x2f66, 0x2fc5, 0x2fe1];
const INNER_LEFT: [usize; 2] = [0x531d, 0x5340];
const INNER_RIGHT: [usize; 3] = [0x5388, 0x53ab, 0x53ca];
const LEFT_TAIL: [usize; 4] = [0x3dce, 0x3ebe, 0x3ed9, 0x3ef4];
const RIGHT_TAIL: [usize; 4] = [0x2f84, 0x3074, 0x308f, 0x30aa];
const LEFT_GEAR: [usize; 4] = [0x589e, 0x58bd, 0x58dc, 0x58fb];
const RIGHT_GEAR: [usize; 4] = [0x595f, 0x597e, 0x599d, 0x59bc];
const NOSE: [usize; 2] = [0x5a08, 0x5a27];
const HOOK: [usize; 2] = [0x5a65, 0x5a84];
const EPS: f32 = 1e-4;
struct Morph {
    neutral: Vec<[f32; 3]>,
    down: Vec<[f32; 3]>,
    closure: bool,
}
pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
fn face(shape: &Shape, a: usize) -> AppResult<&Face> {
    let mut fs = shape.faces.iter().filter(|f| f.address == a);
    let f = fs
        .next()
        .ok_or_else(|| format!("E2C.SH missing reviewed face {a:x}"))?;
    if fs.next().is_some() {
        return Err("E2C.SH duplicate reviewed face".into());
    }
    Ok(f)
}
fn roots(shape: &Shape, ids: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &a in ids {
        if !points
            .iter()
            .all(|p| face(shape, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("E2C.SH missing source attachment {a:x}").into());
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
    let ids: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
    if ids.difference(original).copied().collect::<BTreeSet<_>>() != added.iter().copied().collect()
        || original.difference(&ids).copied().collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed E2C.SH branch {word:x}={value}").into());
    }
    Ok(shape)
}
fn tail(a: usize) -> bool {
    LEFT_TAIL.contains(&a) || RIGHT_TAIL.contains(&a)
}
fn gear(a: usize) -> bool {
    LEFT_GEAR.contains(&a) || RIGHT_GEAR.contains(&a) || NOSE.contains(&a)
}
fn tail_cut(p: [f32; 3]) -> f32 {
    p[1] + 40. + 0.05 * p[0].abs()
}
fn tail_panel(f: &Face) -> bool {
    tail(f.address)
        && f.positions.iter().all(|p| tail_cut(*p) <= EPS)
        && [(2., 9.), (13., 19.)].into_iter().any(|(lo, hi)| {
            f.positions
                .iter()
                .all(|p| (lo - EPS..=hi + EPS).contains(&p[0].abs()))
        })
}
fn fin_pivot(a: usize) -> Option<[f32; 3]> {
    if OUTER_LEFT.contains(&a) {
        Some([-21., -42., 6.])
    } else if OUTER_RIGHT.contains(&a) {
        Some([21., -42., 6.])
    } else if INNER_LEFT.contains(&a) {
        Some([-11., -42., 5.])
    } else if INNER_RIGHT.contains(&a) {
        Some([11., -42., 5.])
    } else {
        None
    }
}
fn trailing(p: [f32; 3]) -> bool {
    p[0].abs() == 33. && p[1] == -9. || [56., 57.].contains(&p[0].abs()) && p[1] == -6.
}
fn split(f: &Face, d: impl Fn([f32; 3]) -> f32) -> Vec<Face> {
    crate::aircraft_animation::split_surface(f, [0.; 3], [1., 0., 0.], 0., d)
        .into_iter()
        .filter(|f| normal(&f.positions).is_some())
        .collect()
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 29258
            || shape.faces.len() != 345
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed E2C.SH animation layout".into());
        }
        roots(&shape, &OUTER_LEFT, &[[-21., -37., 6.], [-21., -45., 6.]])?;
        roots(&shape, &OUTER_RIGHT, &[[21., -37., 6.], [21., -45., 6.]])?;
        roots(
            &shape,
            &INNER_LEFT,
            &[[-11., -36., 5.], [-11., -42., 5.], [-10., -44., 15.]],
        )?;
        roots(
            &shape,
            &INNER_RIGHT[..2],
            &[[11., -36., 5.], [11., -44., 6.], [10., -44., 15.]],
        )?;
        roots(
            &shape,
            &INNER_RIGHT[2..],
            &[[11., -36., 5.], [11., -44., 6.], [11., -42., 5.]],
        )?;
        roots(
            &shape,
            &[0x2f84, 0x3074, 0x3ebe, 0x3ed9],
            &[[0., -35., 4.], [0., -45., 4.]],
        )?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut flaps = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, bindings, ends) in [
            (
                0x8238,
                [
                    (0x577d, 0x581d, [0, 1, 2, 3]),
                    (0x57a4, 0x57f6, [2, 3, 0, 1]),
                ],
                [0x5844],
            ),
            (
                0x823e,
                [
                    (0x5676, 0x5716, [1, 2, 3, 0]),
                    (0x569d, 0x56ef, [1, 2, 3, 0]),
                ],
                [0x573d],
            ),
        ] {
            let removed: Vec<_> = bindings.iter().map(|b| b.0).collect();
            let added: Vec<_> = bindings.iter().map(|b| b.1).chain(ends).collect();
            let down = branch(bytes, &original, word, -1, &added, &removed)?;
            branch(bytes, &original, word, 1, &[], &removed)?;
            let mut map = BTreeMap::new();
            for (a, b, order) in bindings {
                let base = face(&shape, a)?;
                let target = face(&down, b)?;
                if base.positions.len() != 4 || target.positions.len() != 4 {
                    return Err("E2C.SH unreviewed flap topology".into());
                }
                let target: Vec<_> = order.iter().map(|&i| target.positions[i]).collect();
                for (p, q) in base.positions.iter().zip(&target) {
                    if !trailing(*p) && p != q {
                        return Err("E2C.SH source flap attachment moved".into());
                    }
                    map.insert(q.map(f32::to_bits), *p);
                }
                flaps.insert(
                    a,
                    Morph {
                        neutral: base.positions.clone(),
                        down: target,
                        closure: false,
                    },
                );
            }
            for a in ends {
                let mut f = face(&down, a)?.clone();
                let deployed = f.positions.clone();
                let neutral: Vec<_> = deployed
                    .iter()
                    .map(|p| {
                        map.get(&p.map(f32::to_bits))
                            .copied()
                            .ok_or("E2C.SH unreviewed flap closure endpoint")
                    })
                    .collect::<Result<_, _>>()?;
                f.positions = neutral.clone();
                flaps.insert(
                    a,
                    Morph {
                        neutral,
                        down: deployed,
                        closure: true,
                    },
                );
                closures.push(f);
            }
        }
        let ids: Vec<_> = LEFT_GEAR
            .iter()
            .chain(&RIGHT_GEAR)
            .chain(&NOSE)
            .copied()
            .collect();
        let down = branch(bytes, &original, 0x8226, 1, &ids, &[])?;
        branch(bytes, &original, 0x8226, -1, &[], &[])?;
        roots(
            &down,
            &[0x589e, 0x58bd],
            &[[-16., 0., -7.], [-16., 2., -7.]],
        )?;
        roots(&down, &[0x595f, 0x597e], &[[16., 0., -7.], [16., 2., -7.]])?;
        roots(&down, &NOSE, &[[0., 27., -7.], [0., 30., -7.]])?;
        let hook = branch(bytes, &original, 0x8232, 1, &HOOK, &[])?;
        branch(bytes, &original, 0x8232, -1, &[], &[])?;
        roots(
            &hook,
            &HOOK,
            &[
                [0., -15., -8.],
                [0., -13., -8.],
                [0., -17., -14.],
                [0., -19., -14.],
            ],
        )?;
        let mut faces = Vec::new();
        for f in shape.faces {
            if fin_pivot(f.address).is_some() {
                faces.extend(split(&f, |p| p[1] + 42.));
            } else if tail(f.address) {
                let mut pieces = vec![f];
                for boundary in [2., 9., 13., 19.] {
                    pieces = pieces
                        .into_iter()
                        .flat_map(|f| split(&f, |p| p[0].abs() - boundary))
                        .collect();
                }
                faces.extend(pieces.into_iter().flat_map(|f| split(&f, tail_cut)));
            } else {
                faces.push(f);
            }
        }
        faces.extend(closures);
        faces.extend(down.faces.into_iter().filter(|f| gear(f.address)));
        faces.extend(hook.faces.into_iter().filter(|f| HOOK.contains(&f.address)));
        shape.faces = faces;
        validate_gear(&shape)?;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut f = source.clone();
        let a = f.address;
        if let Some(morph) = self.flaps.get(&a) {
            if morph.closure && state.flaps <= 0. && state.aileron == 0. {
                return None;
            }
            let amount = state.flaps.clamp(0., 1.) as f32;
            f.positions = morph
                .neutral
                .iter()
                .zip(&morph.down)
                .map(|(p, q)| std::array::from_fn(|i| p[i] + amount * (q[i] - p[i])))
                .collect();
            if state.aileron != 0. {
                let side = if morph.neutral[0][0] < 0. { -1. } else { 1. };
                let mut rolled = f.clone();
                turn(
                    &mut rolled,
                    [side * 33., -4., if side < 0. { 3.5 } else { 4. }],
                    [if side < 0. { -23. } else { 24. }, 0., 1.5],
                    -0.20 * state.aileron.clamp(-1., 1.),
                );
                for ((p, q), neutral) in f
                    .positions
                    .iter_mut()
                    .zip(rolled.positions)
                    .zip(&morph.neutral)
                {
                    if trailing(*neutral) {
                        *p = q;
                    }
                }
            }
        } else if let Some(pivot) = fin_pivot(a) {
            if source.positions.iter().all(|p| p[1] <= -42. + EPS) && state.rudder != 0. {
                turn(
                    &mut f,
                    pivot,
                    [0., 0., 1.],
                    0.35 * state.rudder.clamp(-1., 1.),
                );
                for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                    if (q[1] + 42.).abs() <= EPS {
                        *p = *q;
                    }
                }
            }
        } else if tail_panel(source) && state.elevator != 0. {
            let side = if LEFT_TAIL.contains(&a) { -1. } else { 1. };
            turn(
                &mut f,
                [0., -40., 4.],
                [1., -side * 0.05, 0.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if tail_cut(*q).abs() <= EPS {
                    *p = *q;
                }
            }
        } else if gear(a) {
            if state.gear <= 0. {
                return None;
            }
            gear_pose(&mut f, state.gear);
            return Some(f);
        }
        if HOOK.contains(&a) {
            if state.hook <= 0. {
                return None;
            }
            gear_pose(&mut f, state.hook);
            return Some(f);
        }
        if f.positions != source.positions {
            update_normal(source, &mut f);
        }
        Some(f)
    }
}
fn gear_pose(f: &mut Face, deployed: f64) {
    let closing = 1. - deployed.clamp(0., 1.);
    if closing == 0. {
        return;
    }
    let (pivot, angle) = if HOOK.contains(&f.address) {
        ([0., -13., -8.], -std::f64::consts::FRAC_PI_2)
    } else if NOSE.contains(&f.address) {
        ([0., 30., -7.], -std::f64::consts::FRAC_PI_2)
    } else {
        (
            [
                if LEFT_GEAR.contains(&f.address) {
                    -16.
                } else {
                    16.
                },
                0.,
                if LEFT_GEAR.contains(&f.address) {
                    -7.
                } else {
                    -6.
                },
            ],
            115f64.to_radians(),
        )
    };
    turn(f, pivot, [1., 0., 0.], angle * closing);
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for step in 0..=20 {
        let g = f64::from(step) / 20.;
        for source in shape
            .faces
            .iter()
            .filter(|f| gear(f.address) || HOOK.contains(&f.address))
        {
            let mut q = source.clone();
            gear_pose(&mut q, g);
            for (i, p) in source.positions.iter().enumerate() {
                let b = q.positions[i];
                if !b.iter().all(|v| v.is_finite()) || g == 1. && *p != b {
                    return Err("E2C.SH invalid device endpoint".into());
                }
                for (j, r) in source.positions.iter().enumerate().skip(i + 1) {
                    if (distance(*p, *r) - distance(b, q.positions[j])).abs() > EPS {
                        return Err("E2C.SH complete wheel/leg/hook card lost rigidity".into());
                    }
                }
                if g == 0. {
                    let (lo, hi) = if HOOK.contains(&source.address) {
                        ([0., -19., -8.], [0., -13., -2.])
                    } else if NOSE.contains(&source.address) {
                        ([0., 24., -7.], [0., 30., -4.])
                    } else if LEFT_GEAR.contains(&source.address) {
                        ([-16., -0.85, -7.], [-15., 6.35, -1.32])
                    } else {
                        ([15., 0., -5.58], [16., 7.26, 0.11])
                    };
                    if (0..3).any(|k| b[k] < lo[k] - EPS || b[k] > hi[k] + EPS) {
                        return Err("E2C.SH device exceeds source-body stow bounds".into());
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
    fn panel(a: usize, p: Vec<[f32; 3]>) -> Face {
        Face {
            address: a,
            colors: vec![17; p.len()],
            uv: vec![[0.25, 0.75]; p.len()],
            positions: p,
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
    fn all_four_fitted_rudders_keep_canted_cut_points_and_signed_motion() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        for (a, x, z) in [
            (0x3e0f, -21., 6.),
            (0x2fc5, 21., 6.),
            (0x531d, -11., 5.),
            (0x5388, 11., 5.),
        ] {
            let side = if x < 0. { -1. } else { 1. };
            let f = panel(
                a,
                vec![
                    [x, -42., z],
                    [x, -45., z],
                    [x - side, -45., 15.],
                    [x - side, -42., 15.],
                ],
            );
            for yaw in [-1., 0., 1.] {
                let mut s = state();
                s.rudder = yaw;
                s.elevator = 1.;
                let q = rig.animate(&f, &s).unwrap();
                assert_eq!(q.positions[0], f.positions[0]);
                assert_eq!(q.positions[3], f.positions[3]);
                if yaw != 0. {
                    assert!((q.positions[1][0] - f.positions[1][0]) * yaw as f32 > 0.);
                } else {
                    assert_eq!(q.positions, f.positions);
                }
            }
        }
    }
    #[test]
    fn fin_root_corridors_are_fixed_during_pitch() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let mut s = state();
        s.elevator = 1.;
        for (side, a) in [(-1., 0x3ebe), (1., 0x3074)] {
            for (x0, x1) in [(0., 2.), (9., 13.), (19., 21.)] {
                let f = panel(
                    a,
                    vec![
                        [side * x0, -45., 5.],
                        [side * x1, -45., 5.],
                        [side * x1, -41., 5.],
                        [side * x0, -41., 5.],
                    ],
                );
                assert_eq!(rig.animate(&f, &s).unwrap().positions, f.positions);
            }
            let f = panel(
                a,
                vec![
                    [side * 2., -40.1, 4.],
                    [side * 9., -40.45, 5.],
                    [side * 9., -45., 5.],
                    [side * 2., -45., 4.],
                ],
            );
            let q = rig.animate(&f, &s).unwrap();
            assert_eq!(q.positions[..2], f.positions[..2]);
            assert!(q.positions[2][2] > f.positions[2][2]);
        }
    }
    #[test]
    fn separate_main_wheel_leg_offsets_and_hook_shape_stay_rigid_for_201_poses() {
        let mut faces = Vec::new();
        for (side, leg, wheel) in [(-1., 0x589e, 0x58dc), (1., 0x595f, 0x599d)] {
            faces.push(panel(
                leg,
                vec![
                    [side * 16., 0., -7.],
                    [side * 16., 2., -7.],
                    [side * 16., 2., -12.],
                    [side * 16., 0., -12.],
                ],
            ));
            faces.push(panel(
                wheel,
                vec![
                    [side * 15., 0., -10.],
                    [side * 15., 3., -10.],
                    [side * 15., 3., -14.],
                    [side * 15., 0., -14.],
                ],
            ));
        }
        faces.push(panel(
            0x5a08,
            vec![
                [0., 30., -7.],
                [0., 27., -7.],
                [0., 27., -13.],
                [0., 30., -13.],
            ],
        ));
        faces.push(panel(
            0x5a65,
            vec![
                [0., -15., -8.],
                [0., -13., -8.],
                [0., -17., -14.],
                [0., -19., -14.],
            ],
        ));
        let shape = Shape {
            billboards: Vec::new(),
            faces,
            lines: Vec::new(),
            state_words: BTreeSet::new(),
        };
        validate_gear(&shape).unwrap();
        for step in 0..=200 {
            let g = f64::from(step) / 200.;
            let mut moved = shape.faces.clone();
            for f in &mut moved {
                gear_pose(f, g);
            }
            for start in [0, 2] {
                let old: Vec<_> = shape.faces[start..start + 2]
                    .iter()
                    .flat_map(|f| f.positions.iter())
                    .collect();
                let new: Vec<_> = moved[start..start + 2]
                    .iter()
                    .flat_map(|f| f.positions.iter())
                    .collect();
                for (i, p) in old.iter().enumerate() {
                    for (j, q) in old.iter().enumerate().skip(i + 1) {
                        assert!((distance(**p, **q) - distance(*new[i], *new[j])).abs() < EPS);
                    }
                }
            }
            assert_eq!(moved[0].positions[0], shape.faces[0].positions[0]);
            let mut pivot_marker =
                panel(0x595f, vec![[16., 0., -6.], [16., 1., -6.], [16., 1., -7.]]);
            gear_pose(&mut pivot_marker, g);
            assert_eq!(pivot_marker.positions[0], [16., 0., -6.]);
            assert_eq!(moved[5].positions[1], shape.faces[5].positions[1]);
        }
    }
    #[test]
    fn asymmetric_flaperons_reach_own_endpoints_and_keep_degenerate_left_cap() {
        let mut flaps = BTreeMap::new();
        let mut faces = Vec::new();
        for (side, a, outer, inner_top, tip_top, down_z) in [
            (-1., 0x57a4, 56., 4., 5., 0.),
            (1., 0x569d, 57., 5., 6., 1.),
        ] {
            let n = vec![
                [side * 33., -4., inner_top],
                [side * outer, -4., tip_top],
                [side * outer, -6., 5.],
                [side * 33., -9., 4.],
            ];
            let d = vec![
                [side * 33., -4., inner_top],
                [side * outer, -4., tip_top],
                [side * outer, -6., 3.],
                [side * 33., -8., down_z],
            ];
            faces.push(panel(a, n.clone()));
            flaps.insert(
                a,
                Morph {
                    neutral: n,
                    down: d,
                    closure: false,
                },
            );
        }
        let cn = vec![[-56., -4., 5.], [-56., -4., 5.], [-56., -6., 5.]];
        faces.push(panel(0x5844, cn.clone()));
        flaps.insert(
            0x5844,
            Morph {
                neutral: cn,
                down: vec![[-56., -4., 5.], [-56., -4., 5.], [-56., -6., 3.]],
                closure: true,
            },
        );
        let rig = Rig { flaps };
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            for roll in [-1., 0., 1.] {
                let mut s = state();
                s.flaps = flap;
                s.aileron = roll;
                for f in &faces[..2] {
                    let q = rig.animate(f, &s).unwrap();
                    assert_eq!(q.positions[..2], f.positions[..2]);
                    if flap == 1. && roll == 0. {
                        assert_eq!(q.positions, rig.flaps[&f.address].down);
                    }
                }
                let cap = rig.animate(&faces[2], &s);
                if flap == 0. && roll == 0. {
                    assert!(cap.is_none());
                } else {
                    let cap = cap.unwrap();
                    assert_eq!(cap.positions[0], cap.positions[1]);
                }
            }
        }
    }
}
