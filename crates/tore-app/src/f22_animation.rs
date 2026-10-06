//! Exact F22/F22N donors, fitted attached controls and rigid painted gear.
//! FAXX keeps its explicit concept law; bays use the existing reviewed helpers.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{
    aircraft::AircraftId as Id,
    shape::{Face, Shape},
};
const EPS: f32 = 1e-4;
#[derive(Clone, Copy, PartialEq)]
enum Part {
    Flame,
    Brake,
    Gear,
    Hook,
    Bay,
}
pub struct Rig {
    id: Id,
    parts: BTreeMap<usize, Part>,
}
fn donor(a: usize, id: Id) -> usize {
    if id.source() == Id::F22n {
        a.saturating_sub(0xf8)
    } else {
        a
    }
}
fn inner(a: usize, id: Id) -> bool {
    matches!(
        donor(a, id),
        0x437f | 0x439e | 0x43fb | 0x4416 | 0x4576 | 0x4599 | 0x45f6
    )
}
fn outer(a: usize, id: Id) -> bool {
    matches!(donor(a, id), 0x43bd | 0x43dc | 0x45b8 | 0x45d7)
}
fn tail(a: usize, id: Id) -> bool {
    if id.source() == Id::F22n {
        matches!(a, 0x31eb | 0x31fd | 0x36d3 | 0x36e7 | 0x3ada | 0x3cff)
    } else {
        matches!(a, 0x32ba | 0x32cc | 0x35ad | 0x35c1 | 0x3987 | 0x3ba9)
    }
}
fn fin(a: usize, id: Id) -> bool {
    if id.source() == Id::F22n {
        matches!(a, 0x361d | 0x3670 | 0x38e4 | 0x3903 | 0x3926)
    } else {
        matches!(a, 0x34f7 | 0x354a | 0x37ff | 0x381e | 0x3841)
    }
}
fn side(f: &Face) -> f32 {
    if f.positions.iter().map(|p| p[0]).sum::<f32>() < 0. {
        -1.
    } else {
        1.
    }
}
fn fin_cut(p: [f32; 3]) -> f32 {
    p[1] + 35. - 0.25 * p[2]
}
fn split(f: &Face, d: impl Fn([f32; 3]) -> f32) -> Vec<Face> {
    crate::aircraft_animation::split_surface(f, [0.; 3], [1., 0., 0.], 0., d)
}
impl Rig {
    pub fn load(id: Id, bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        let naval = id.source() == Id::F22n;
        let (size, count, words, branches): (_, _, &[usize], &[(usize, Part, usize)]) = if naval {
            (
                20146,
                248,
                &[0x5e70, 0x5e7c, 0x5e82, 0x5e8e, 0x5e9a, 0x5ea0, 0x5ea6],
                &[
                    (0x5e70, Part::Flame, 8),
                    (0x5e7c, Part::Bay, 12),
                    (0x5e82, Part::Brake, 4),
                    (0x5e8e, Part::Gear, 12),
                    (0x5e9a, Part::Hook, 2),
                ],
            )
        } else {
            (
                20012,
                245,
                &[0x5df0, 0x5dfc, 0x5e02, 0x5e0e, 0x5e1a, 0x5e20],
                &[
                    (0x5df0, Part::Flame, 8),
                    (0x5dfc, Part::Bay, 14),
                    (0x5e02, Part::Brake, 4),
                    (0x5e0e, Part::Gear, 12),
                ],
            )
        };
        if !matches!(id, Id::F22 | Id::F22n | Id::Faxx)
            || tore_formats::module::code(bytes)?.0.len() != size
            || shape.faces.len() != count
            || shape.state_words != words.iter().copied().collect()
        {
            return Err("unreviewed F22-family shape".into());
        }
        let neutral: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut parts = BTreeMap::new();
        for &(word, part, count) in branches {
            let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
            let added: Vec<_> = pose
                .faces
                .into_iter()
                .filter(|f| !neutral.contains(&f.address))
                .collect();
            if added.len() != count {
                return Err(format!("unreviewed F22 branch {word:x}").into());
            }
            for f in added {
                parts.insert(f.address, part);
                shape.faces.push(f);
            }
        }
        // Original front seams are asymmetric and thick; guard both skin heights.
        for (a, roots) in [
            (0x437f, vec![[16., -28., 1.], [42., -22., 1.]]),
            (0x43fb, vec![[16., -28., -1.], [42., -22., 0.]]),
            (0x4599, vec![[-17., -28., 1.], [-43., -22., 1.]]),
            (0x4576, vec![[-17., -28., -1.], [-43., -22., 0.]]),
        ] {
            let address = a + if naval { 0xf8 } else { 0 };
            let f = shape
                .faces
                .iter()
                .find(|f| f.address == address)
                .ok_or("F22 missing front seam")?;
            if !roots.iter().all(|p| f.positions.contains(p)) {
                return Err("F22 source attachment changed".into());
            }
        }
        let mut prepared = Vec::new();
        for f in shape.faces {
            if fin(f.address, id) {
                if id == Id::Faxx {
                    continue;
                }
                for panel in split(&f, |p| p[2] - 6.) {
                    prepared.extend(split(&panel, fin_cut));
                }
            } else {
                prepared.push(f);
            }
        }
        if id == Id::Faxx {
            crate::additional_animation::concept_colors(&mut prepared);
        }
        shape.faces = prepared;
        Ok((Self { id, parts }, shape))
    }
    pub fn flame(&self, a: usize) -> bool {
        self.parts.get(&a) == Some(&Part::Flame)
    }
    pub fn faces(&self, f: &Face, s: &State) -> Vec<Face> {
        if self.parts.get(&f.address) == Some(&Part::Bay) {
            return crate::roster_animation::bay_lining(self.id.source(), f);
        }
        if self.id == Id::Faxx && inner(f.address, self.id) {
            let opening = (f64::from(side(f)) * s.rudder).clamp(0., 1.) * 0.6;
            let angles = if opening > 1e-8 {
                vec![0.4 * s.flaps - opening, 0.4 * s.flaps + opening]
            } else {
                vec![0.4 * s.flaps]
            };
            return angles.into_iter().map(|a| wing(f, self.id, a)).collect();
        }
        // Only reviewed belly faces are handed to the old clipping path. Passing fin
        // or concept flap faces here would animate them a second time.
        let belly = if self.id.source() == Id::F22n {
            matches!(f.address, 0x3755 | 0x36fd | 0x3716 | 0x39ae | 0x39dc)
        } else {
            matches!(f.address, 0x35d7 | 0x3601 | 0x362f | 0x3cc0 | 0x3d03)
        };
        if belly {
            crate::roster_animation::faces(self.id.source(), f, s)
        } else {
            vec![f.clone()]
        }
    }
    pub fn animate(&self, source: &Face, s: &State) -> Option<Face> {
        let mut f = source.clone();
        let a = f.address;
        match self.parts.get(&a) {
            Some(Part::Gear) => {
                if s.gear <= 0. {
                    return None;
                }
                gear(&mut f, self.id, s.gear);
                if matches!(donor(a, self.id), 0x41fd | 0x421c) && s.nosewheel_angle() != 0. {
                    turn(&mut f, [0., 63., -9.], [0., 0., 1.], -s.nosewheel_angle());
                }
                return Some(f);
            }
            Some(Part::Hook) => {
                if s.hook <= 1e-8 {
                    return None;
                }
                turn(&mut f, [0., -9., -9.], [1., 0., 0.], -0.9 * (1. - s.hook));
                return Some(f);
            }
            Some(Part::Brake) => {
                if s.brake <= 0. {
                    return None;
                }
                turn(&mut f, [0., -17., 5.], [1., 0., 0.], 0.7 * (1. - s.brake));
                for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                    if q[1] >= -17. {
                        *p = *q;
                    }
                }
                normal(source, &mut f);
                return Some(f);
            }
            Some(Part::Bay) => return (s.bay > 0.).then_some(f),
            Some(Part::Flame) => {
                if s.exhaust <= 0. {
                    return None;
                }
                for p in &mut f.positions {
                    p[1] = -48. + (p[1] + 48.) * s.exhaust as f32;
                }
                normal(source, &mut f);
                return Some(f);
            }
            None => {}
        }
        if inner(a, self.id) {
            if self.id != Id::Faxx {
                return Some(wing(source, self.id, 0.4 * s.flaps));
            }
            return Some(f);
        }
        if outer(a, self.id) {
            return Some(wing(
                source,
                self.id,
                -f64::from(side(source)) * 0.2 * s.aileron,
            ));
        }
        if tail(a, self.id) {
            let sign = side(source);
            let angle = -0.3 * s.elevator - f64::from(sign) * 0.1 * s.aileron;
            if angle != 0. {
                turn(&mut f, [sign * 18., -48., 1.], [1., 0., 0.], angle);
                for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                    if q[0].abs() <= 19. {
                        *p = *q;
                    }
                }
                normal(source, &mut f);
            }
        }
        if fin(a, self.id)
            && source
                .positions
                .iter()
                .all(|p| p[2] >= 6. - EPS && fin_cut(*p) <= EPS)
            && s.rudder != 0.
        {
            for p in &mut f.positions {
                let weight = ((p[2] - 6.) / 6.).clamp(0., 1.);
                p[0] += (0.35 * s.rudder).tan() as f32 * (-fin_cut(*p)).max(0.) * weight;
            }
            normal(source, &mut f);
        }
        Some(f)
    }
}
fn wing(source: &Face, id: Id, angle: f64) -> Face {
    let mut f = source.clone();
    if angle == 0. {
        return f;
    }
    let sign = side(source);
    let (pivot, axis) = if inner(source.address, id) {
        if sign < 0. {
            ([-17., -28., 0.], [26., -6., -0.5])
        } else {
            ([16., -28., 0.], [26., 6., 0.5])
        }
    } else if sign < 0. {
        ([-43., -22., 0.5], [17., 4., -0.5])
    } else {
        ([42., -22., 0.5], [17., -4., 0.5])
    };
    turn(&mut f, pivot, axis, angle);
    for (p, q) in f.positions.iter_mut().zip(&source.positions) {
        let front = if inner(source.address, id) {
            q[1] == -28. || q[1] == -22.
        } else {
            q[1] == -22. || q[1] == -26.
        };
        if front {
            *p = *q;
        }
    }
    normal(source, &mut f);
    f
}
fn gear(f: &mut Face, id: Id, g: f64) {
    let closing = 1. - g.clamp(0., 1.);
    if closing == 0. {
        return;
    }
    let a = donor(f.address, id);
    let (pivot, axis, angle) = match a {
        0x4135 | 0x4154 => (
            [-13., 2.25, -5.],
            [-12., -8., -1.],
            std::f64::consts::FRAC_PI_2,
        ),
        0x40ca | 0x40e9 => (
            [12., 2.25, -6.],
            [-12., 8., 1.],
            std::f64::consts::FRAC_PI_2,
        ),
        0x41fd | 0x421c => ([0., 63., -9.], [1., 0., 0.], -175f64.to_radians()),
        0x4014 | 0x402b => ([18., -5., -3.], [0., 28., 2.], std::f64::consts::FRAC_PI_2),
        0x406f | 0x4086 => (
            [-16., -5., -3.],
            [0., 28., 2.],
            -std::f64::consts::FRAC_PI_2,
        ),
        0x4192 | 0x41b1 => ([-2., 0., -8.], [0., 1., 0.], -std::f64::consts::FRAC_PI_2),
        _ => return,
    };
    turn(f, pivot, axis, angle * closing);
}
fn normal(source: &Face, f: &mut Face) {
    let Some(old) = source.normal else {
        return;
    };
    if f.positions.len() < 3 {
        return;
    }
    let p = f.positions[0];
    let mut candidate = None;
    for i in 1..f.positions.len() - 1 {
        let a = std::array::from_fn::<_, 3, _>(|k| f.positions[i][k] - p[k]);
        let b = std::array::from_fn::<_, 3, _>(|k| f.positions[i + 1][k] - p[k]);
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let len = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        if len > 1e-6 {
            candidate = Some(n.map(|v| v / len));
            break;
        }
    }
    if let Some(mut n) = candidate {
        if n[0] * old[0] + n[1] * old[2] + n[2] * old[1] < 0. {
            n = n.map(|v| -v);
        }
        let length = old.iter().map(|v| v * v).sum::<f32>().sqrt();
        f.normal = Some([n[0] * length, n[2] * length, n[1] * length]);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn face(a: usize, p: Vec<[f32; 3]>) -> Face {
        let n = p.len();
        Face {
            address: a,
            positions: p,
            colors: vec![0; n],
            uv: vec![[0., 0.]; n],
            texture: String::new(),
            subtype: 0x64,
            normal: None,
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    #[test]
    fn thick_flap_roots_remain_fixed_and_paired_trailing_points_agree() {
        let a = face(
            0x437f,
            vec![
                [42., -34., 0.],
                [17., -34., 1.],
                [16., -28., 1.],
                [42., -22., 1.],
            ],
        );
        let b = face(
            0x439e,
            vec![
                [42., -22., 0.],
                [17., -34., -1.],
                [24., -39., 0.],
                [42., -34., 0.],
            ],
        );
        for angle in [-1., -0.6, 0.4, 1.] {
            let aa = wing(&a, Id::F22, angle);
            let bb = wing(&b, Id::F22, angle);
            assert_eq!(aa.positions[2], a.positions[2]);
            assert_eq!(aa.positions[3], a.positions[3]);
            assert_eq!(bb.positions[0], b.positions[0]);
            assert_eq!(aa.positions[0], bb.positions[3]);
        }
    }
    #[test]
    fn every_main_card_is_rigid_at_201_poses() {
        let f = face(
            0x4135,
            vec![
                [-13., -7., -23.],
                [-13., 6., -23.],
                [-13., 6., -5.],
                [-13., -7., -5.],
            ],
        );
        for i in 0..=200 {
            let mut q = f.clone();
            gear(&mut q, Id::F22, f64::from(i) / 200.);
            for a in 0..4 {
                for b in 0..4 {
                    let dist = |p: [f32; 3], q: [f32; 3]| {
                        (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>()
                    };
                    assert!(
                        (dist(f.positions[a], f.positions[b])
                            - dist(q.positions[a], q.positions[b]))
                        .abs()
                            < 0.001
                    );
                }
            }
        }
    }
    #[test]
    fn concept_split_retains_midpoint_law_and_original_hinge() {
        let rig = Rig {
            id: Id::Faxx,
            parts: BTreeMap::new(),
        };
        let f = face(
            0x4477,
            vec![
                [42., -34., 0.],
                [17., -34., 1.],
                [16., -28., 1.],
                [42., -22., 1.],
            ],
        );
        let mut state = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        state.flaps = 1.;
        state.rudder = 1.;
        let leaves = rig.faces(&f, &state);
        assert_eq!(leaves.len(), 2);
        assert_eq!(leaves[0].positions, wing(&f, Id::Faxx, -0.2).positions);
        assert_eq!(leaves[1].positions, wing(&f, Id::Faxx, 1.).positions);
        for leaf in leaves {
            assert_eq!(leaf.positions[2], f.positions[2]);
            assert_eq!(leaf.positions[3], f.positions[3]);
        }
    }
    #[test]
    fn family_faces_then_animate_preserves_existing_bays_and_sentinel_lining() {
        for id in [Id::F22, Id::F22n, Id::Faxx] {
            let rig = Rig {
                id,
                parts: [(0xf000, Part::Bay)].into(),
            };
            let mut wall = face(
                0xf000,
                vec![
                    [-8., 7., -12.],
                    [-8., 51., -12.],
                    [-8., 51., -9.],
                    [-8., 7., -9.],
                ],
            );
            wall.normal = Some([32765., 0., 0.]);
            let belly = face(
                if id.source() == Id::F22 {
                    0x35d7
                } else {
                    0x3755
                },
                vec![
                    [-10., 0., -9.],
                    [10., 0., -9.],
                    [10., 60., -9.],
                    [-10., 60., -9.],
                ],
            );
            let sentinel = face(
                usize::MAX,
                vec![[-8., 7., -6.], [-2., 7., -6.], [-2., 51., -6.]],
            );
            let mut state = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
            for value in [0., 0.25, 0.5, 0.75, 1.] {
                state.bay = value;
                let actual: Vec<_> = rig
                    .faces(&wall, &state)
                    .into_iter()
                    .filter_map(|f| rig.animate(&f, &state))
                    .collect();
                let expected = if value == 0. {
                    vec![]
                } else {
                    crate::roster_animation::bay_lining(id.source(), &wall)
                };
                assert_eq!(actual.len(), expected.len());
                for (a, b) in actual.iter().zip(&expected) {
                    assert_eq!(a.positions, b.positions);
                    assert_eq!(a.uv, b.uv);
                    assert_eq!(a.colors, b.colors);
                }
                let actual: Vec<_> = rig
                    .faces(&belly, &state)
                    .into_iter()
                    .filter_map(|f| rig.animate(&f, &state))
                    .collect();
                let expected = crate::roster_animation::faces(id.source(), &belly, &state);
                assert_eq!(actual.len(), expected.len());
                for (a, b) in actual.iter().zip(&expected) {
                    assert_eq!(a.positions, b.positions);
                }
                if value == 0. {
                    assert_eq!(actual.len(), 1);
                    assert_eq!(actual[0].positions, belly.positions);
                }
                assert_eq!(
                    rig.animate(&sentinel, &state).unwrap().positions,
                    sentinel.positions
                );
            }
        }
    }
    #[test]
    fn upper_rear_rudder_vertex_moves_with_both_attachment_cuts_fixed() {
        let rig = Rig {
            id: Id::F22,
            parts: BTreeMap::new(),
        };
        let f = face(
            0x34f7,
            vec![[29., -37., 36.], [29., -26., 36.], [15., -33.5, 6.]],
        );
        let mut state = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        for yaw in [-1., 1.] {
            state.rudder = yaw;
            let q = rig.animate(&f, &state).unwrap();
            assert!((q.positions[0][0] - f.positions[0][0]) * yaw as f32 > 0.);
            assert_eq!(q.positions[1], f.positions[1]);
            assert_eq!(q.positions[2], f.positions[2]);
        }
    }
    #[test]
    fn brake_front_roots_and_center_seam_stay_joined() {
        let left = face(
            0x46d3,
            vec![
                [0., -29., 17.],
                [-3., -28., 18.],
                [-3., -15., 6.],
                [0., -17., 5.],
            ],
        );
        let right = face(
            0x4701,
            vec![
                [0., -29., 17.],
                [3., -28., 18.],
                [3., -15., 6.],
                [0., -17., 5.],
            ],
        );
        let rig = Rig {
            id: Id::F22,
            parts: [(0x46d3, Part::Brake), (0x4701, Part::Brake)].into(),
        };
        let mut state = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        for fraction in [0.000001, 0.25, 0.5, 0.75, 1.] {
            state.brake = fraction;
            let a = rig.animate(&left, &state).unwrap();
            let b = rig.animate(&right, &state).unwrap();
            assert_eq!(a.positions[0], b.positions[0]);
            for i in [2, 3] {
                assert_eq!(a.positions[i], left.positions[i]);
                assert_eq!(b.positions[i], right.positions[i]);
            }
            if fraction == 1. {
                assert_eq!(a.positions, left.positions);
                assert_eq!(b.positions, right.positions);
            }
        }
    }
}
