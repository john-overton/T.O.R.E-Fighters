//! FA-specific source groups with fitted presentation laws. See additional-aircraft spec.
use crate::{aircraft_animation::rotate, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{
    aircraft::AircraftId,
    shape::{Face, Shape},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Flame,
    Brake,
    Gear,
    Hook,
    Bay,
}
pub struct Rig {
    id: AircraftId,
    parts: BTreeMap<usize, Part>,
}
impl Rig {
    pub fn load(id: AircraftId, bytes: &[u8]) -> Result<(Self, Shape), Box<dyn std::error::Error>> {
        let (size, neutral_count, words, branches): (_, _, &[usize], &[(usize, Part, usize)]) =
            match id {
                AircraftId::F14 => (
                    29462,
                    313,
                    &[0x82e0, 0x82e6, 0x82ec, 0x82f8, 0x82fe, 0x8304],
                    &[
                        (0x82e0, Part::Flame, 8),
                        (0x82e6, Part::Brake, 4),
                        (0x82ec, Part::Gear, 16),
                        (0x82f8, Part::Hook, 2),
                    ],
                ),
                AircraftId::A4E => (
                    24506,
                    237,
                    &[0x6f90, 0x6f96, 0x6fa2, 0x6fa8, 0x6fae],
                    &[(0x6f90, Part::Brake, 12), (0x6f96, Part::Gear, 18)],
                ),
                AircraftId::X31 => (
                    21974,
                    225,
                    &[0x65a0, 0x65a6, 0x65b2, 0x65be, 0x65c4, 0x65ca],
                    &[
                        (0x65a0, Part::Flame, 4),
                        (0x65a6, Part::Brake, 4),
                        (0x65b2, Part::Gear, 22),
                    ],
                ),
                AircraftId::Mig29 => (
                    29290,
                    328,
                    &[0x8240, 0x8246, 0x824c, 0x8258, 0x825e],
                    &[
                        (0x8240, Part::Flame, 16),
                        (0x8246, Part::Brake, 4),
                        (0x824c, Part::Gear, 16),
                    ],
                ),
                AircraftId::Su27 => (
                    12838,
                    146,
                    &[0x41f0, 0x41f6, 0x41fc, 0x4208, 0x420e, 0x4214, 0x421a],
                    &[
                        (0x41f0, Part::Flame, 16),
                        (0x41f6, Part::Brake, 2),
                        (0x41fc, Part::Gear, 8),
                    ],
                ),
                AircraftId::Mig21 => (
                    14952,
                    159,
                    &[0x4a50, 0x4a56],
                    &[(0x4a50, Part::Flame, 8), (0x4a56, Part::Gear, 6)],
                ),
                AircraftId::Su25 => (
                    29626,
                    334,
                    &[0x8390, 0x8396, 0x83a2, 0x83a8, 0x83ae],
                    &[(0x8390, Part::Brake, 8), (0x8396, Part::Gear, 18)],
                ),
                AircraftId::Mig23 => (
                    23312,
                    219,
                    &[0x6ae0, 0x6ae6, 0x6af2, 0x6af8, 0x6afe],
                    &[(0x6ae0, Part::Flame, 6), (0x6ae6, Part::Gear, 18)],
                ),
                AircraftId::Su35 => (
                    27114,
                    329,
                    &[0x79c0, 0x79c6, 0x79cc, 0x79d8, 0x79de],
                    &[
                        (0x79c0, Part::Flame, 8),
                        (0x79c6, Part::Brake, 2),
                        (0x79cc, Part::Gear, 18),
                    ],
                ),
                AircraftId::F22 | AircraftId::Faxx => (
                    20012,
                    245,
                    &[0x5df0, 0x5dfc, 0x5e02, 0x5e0e, 0x5e1a, 0x5e20],
                    &[
                        (0x5df0, Part::Flame, 8),
                        (0x5dfc, Part::Bay, 14),
                        (0x5e02, Part::Brake, 4),
                        (0x5e0e, Part::Gear, 12),
                    ],
                ),
                _ => return Err("no additional aircraft rig for identity".into()),
            };
        let mut shape = Shape::parse(bytes)?;
        if tore_formats::module::code(bytes)?.0.len() != size
            || shape.faces.len() != neutral_count
            || shape.state_words != words.iter().copied().collect()
        {
            return Err("unreviewed FA shape layout; review aircraft rig before flying".into());
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
                return Err("unreviewed FA device branch".into());
            }
            for f in added {
                parts.insert(f.address, part);
                shape.faces.push(f);
            }
        }
        if id == AircraftId::A4E {
            for address in [0x573f, 0x575f] {
                parts.insert(address, Part::Hook);
            }
        }
        if shape
            .faces
            .iter()
            .any(|f| !f.texture.is_empty() && f.texture != format!("_{}.PIC", id.stem()))
        {
            return Err("unreviewed FA aircraft texture".into());
        }
        if id == AircraftId::Faxx {
            for face in concept_hook() {
                parts.insert(face.address, Part::Hook);
                shape.faces.push(face);
            }
        }
        Ok((Self { id, parts }, shape))
    }
    pub fn scale(&self) -> f32 {
        // FA F14 has header exponent 10; A4/F31 have 8. Retain the host's
        // fitted one-third-foot scale, applying the source exponent difference.
        tore_sim::combat::debris::scale(self.id) as f32
    }
    pub fn cold_nozzle(&self, address: usize) -> bool {
        match self.id {
            AircraftId::F14 => matches!(address, 0x48a6 | 0x48d5 | 0x48fc | 0x491b),
            AircraftId::X31 => address == 0x29f7,
            _ => false,
        }
    }
    pub fn faces(&self, f: &Face, s: &State) -> Vec<Face> {
        if roster_flame_root(self.id).is_some() {
            return crate::roster_animation::faces(self.id, f, s);
        }
        if self.id == AircraftId::A4E && a4_tail(f.address) && s.elevator.abs() >= 1e-8 {
            return crate::aircraft_animation::split_surface(
                f,
                [0., -54., 7.],
                [1., 0., 0.],
                -0.3 * s.elevator,
                |p| p[1] + 54.,
            );
        }
        if self.id != AircraftId::F14
            || !matches!(f.address, 0x4a7b | 0x4a96 | 0x4ac2 | 0x4b19 | 0x4b66)
            || s.rudder.abs() < 1e-8
        {
            return vec![f.clone()];
        }
        let side = if f.positions[0][0] < 0. { -1. } else { 1. };
        let length = 53f64.sqrt();
        crate::aircraft_animation::split_surface(
            f,
            [side * 4., -11., 2.],
            [0., -2. / length, 7. / length],
            s.rudder * 0.35,
            |p| p[1] + 11. + (p[2] - 2.) * 2. / 7.,
        )
    }
    pub fn animate(&self, source: &Face, s: &State) -> Option<Face> {
        let mut f = source.clone();
        let part = self.parts.get(&f.address).copied();
        if let Some(flame_root) = roster_flame_root(self.id) {
            return roster_device(self.id, f, part, s, flame_root);
        }
        let side = if f.positions.iter().map(|p| p[0]).sum::<f32>() < 0. {
            -1.
        } else {
            1.
        };
        let forward = f.positions.iter().map(|p| p[1]).sum::<f32>() / f.positions.len() as f32;
        match part {
            Some(Part::Flame) => {
                if s.exhaust <= 0. {
                    return None;
                }
                let nozzle = if self.id == AircraftId::F14 {
                    -14.
                } else {
                    -41.
                };
                for p in &mut f.positions {
                    p[1] = nozzle + (p[1] - nozzle) * s.exhaust as f32;
                }
                if self.id == AircraftId::X31 {
                    let [pitch, yaw] = vector_angles(s);
                    turn(&mut f, [0., nozzle, 0.], [1., 0., 0.], pitch);
                    turn(&mut f, [0., nozzle, 0.], [0., 0., 1.], -yaw);
                }
            }
            Some(Part::Gear) => {
                if s.gear <= 0. {
                    return None;
                }
                let pivot = match self.id {
                    AircraftId::F14 => {
                        if forward > 10. {
                            [0., 17., -1.]
                        } else {
                            [side * 5., 1., 0.]
                        }
                    }
                    AircraftId::A4E => {
                        if forward > 15. {
                            [0., 27., -8.]
                        } else {
                            [side * 9., -8., -8.]
                        }
                    }
                    _ => {
                        if forward > 15. {
                            [0., 33., -5.]
                        } else {
                            [side * 4., -12., -5.]
                        }
                    }
                };
                turn(
                    &mut f,
                    pivot,
                    [1., 0., 0.],
                    -std::f64::consts::FRAC_PI_2 * (1. - s.gear),
                );
            }
            Some(Part::Brake) => {
                if s.brake <= 0. {
                    return None;
                }
                // Retain the source deployed endpoint and hinge toward a fitted closed pose.
                if self.id == AircraftId::F14 {
                    turn(
                        &mut f,
                        [side * 2., -11., 1.],
                        [1., side * 0.5, -side * 0.5],
                        std::f64::consts::FRAC_PI_4 * (1. - s.brake),
                    );
                } else {
                    let pivot = if self.id == AircraftId::A4E {
                        [side * 5., -30., -3.]
                    } else {
                        [side * 6., -12., 1.]
                    };
                    turn(
                        &mut f,
                        pivot,
                        [0., 0., 1.],
                        side as f64 * 1.05 * (1. - s.brake),
                    );
                }
            }
            Some(Part::Hook) => {
                if self.id == AircraftId::A4E {
                    turn(&mut f, [0., -25., -8.], [1., 0., 0.], 0.9 * s.hook);
                } else {
                    if s.hook <= 0. {
                        return None;
                    }
                    // Quantization collapses the two source root vertices.
                    // A fitted 1/4-source-unit root width restores a visible
                    // triangle while preserving its center and tip.
                    let mut root = 0;
                    for p in &mut f.positions {
                        if p[1] == -5. && p[2] == -2. {
                            p[0] += if root == 0 { -0.125 } else { 0.125 };
                            root += 1;
                        }
                    }
                    turn(&mut f, [0., -5., -2.], [1., 0., 0.], -0.6 * (1. - s.hook));
                }
            }
            Some(Part::Bay) | None => {}
        }
        let a = f.address;
        match self.id {
            AircraftId::F14 => {
                if matches!(a, 0x540d | 0x5434 | 0x4fe9 | 0x5010) {
                    turn(
                        &mut f,
                        [side * 5., -1., 1.],
                        [1., -side / 9., 0.],
                        0.4 * s.flaps,
                    );
                }
                if (0x4dba..=0x5434).contains(&a) {
                    turn(
                        &mut f,
                        [if side < 0. { -4. } else { 5. }, -1., 1.],
                        [0., 0., 1.],
                        -side as f64 * sweep(s),
                    );
                }
                if matches!(a, 0x4828 | 0x4888 | 0x49c5 | 0x4a22) {
                    turn(
                        &mut f,
                        [side * 6., -10., 0.],
                        [1., 0., 0.],
                        -0.3 * s.elevator + side as f64 * 0.2 * s.aileron,
                    );
                }
            }
            AircraftId::A4E => {
                if matches!(a, 0x51c6 | 0x51ef | 0x4fde | 0x5007) {
                    turn(&mut f, [side * 5., -18., -6.], [1., 0., 0.], 0.4 * s.flaps);
                }
                if matches!(a, 0x376a | 0x37d4 | 0x37f4 | 0x3ae9 | 0x3ba2 | 0x3c8d) {
                    turn(
                        &mut f,
                        [side * 21., -18., -6.],
                        [1., 0., 0.],
                        side as f64 * 0.2 * s.aileron,
                    );
                }
                if matches!(a, 0x455c | 0x457d) {
                    turn(&mut f, [0., -45., 11.], [0., -10., 16.], 0.35 * s.rudder);
                }
            }
            AircraftId::X31 => {
                if x31_paddle(a) {
                    // Each paired face shares its two forward root vertices.
                    // Derive the hinge from imported geometry, preserving both skins.
                    let root_y = f
                        .positions
                        .iter()
                        .map(|p| p[1])
                        .fold(f32::NEG_INFINITY, f32::max);
                    let roots: Vec<_> = f
                        .positions
                        .iter()
                        .filter(|p| p[1] == root_y)
                        .copied()
                        .collect();
                    if roots.len() == 2 {
                        let pivot = std::array::from_fn(|i| (roots[0][i] + roots[1][i]) * 0.5);
                        let mut axis: [f32; 3] = std::array::from_fn(|i| roots[1][i] - roots[0][i]);
                        if axis[2] < 0. || (axis[2] == 0. && axis[0] < 0.) {
                            axis = axis.map(|v| -v);
                        }
                        let length = (axis[0] * axis[0] + axis[2] * axis[2]).sqrt();
                        let [pitch, yaw] = vector_angles(s);
                        let angle = ((f64::from(axis[0]) * pitch - f64::from(axis[2]) * yaw)
                            / f64::from(length))
                        .clamp(-15f64.to_radians(), 15f64.to_radians());
                        turn(&mut f, pivot, axis, angle);
                    }
                }
                if matches!(a, 0x42b3 | 0x42ca | 0x4258 | 0x426f) {
                    turn(
                        &mut f,
                        [side * 3., 54., 1.],
                        [1., 0., 0.],
                        0.35 * s.elevator,
                    );
                }
                if matches!(
                    a,
                    0x4773 | 0x4792 | 0x468c | 0x46ab | 0x3af8 | 0x3b17 | 0x3856 | 0x3789
                ) {
                    turn(
                        &mut f,
                        [side * 5., -17., -5.],
                        [1., 0., 0.],
                        0.4 * s.flaps - 0.3 * s.elevator + side as f64 * 0.2 * s.aileron,
                    );
                }
                if matches!(a, 0x451b | 0x4532) {
                    turn(&mut f, [0., -35., 8.], [0., -4., 11.], 0.35 * s.rudder);
                }
            }
            _ => unreachable!(),
        }
        Some(f)
    }
}
fn a4_tail(address: usize) -> bool {
    matches!(
        address,
        0x4345 | 0x4361 | 0x44b5 | 0x44d5 | 0x4967 | 0x4982 | 0x49dd | 0x49f8
    )
}
fn x31_paddle(address: usize) -> bool {
    matches!(address, 0x44b2 | 0x44da | 0x4406 | 0x442e | 0x4342 | 0x4381)
}
/// Shared fitted plume/paddle demand, independent of afterburner visibility.
fn vector_angles(s: &State) -> [f64; 2] {
    [1, 2].map(|i| {
        (s.auxiliary_rates[i] / std::f64::consts::FRAC_PI_2).clamp(-1., 1.) * 15f64.to_radians()
    })
}
/// Fitted visual sweep, additional 0..48 degrees over 400..700 knots.
/// Flaps hold the wings extended. This does not invent a new aerodynamic law.
pub fn sweep(s: &State) -> f64 {
    ((s.speed / 1.68781 - 400.) / 300.).clamp(0., 1.) * 48f64.to_radians() * (1. - s.flaps)
}
pub(crate) fn turn(f: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    let length = axis.iter().map(|v| v * v).sum::<f32>().sqrt();
    let axis = axis.map(|v| f64::from(v / length));
    for p in &mut f.positions {
        let v = rotate(std::array::from_fn(|i| p[i] - pivot[i]), axis, angle);
        *p = std::array::from_fn(|i| pivot[i] + v[i]);
    }
    if let Some(n) = f.normal {
        let n = rotate([n[0], n[2], n[1]], axis, angle);
        f.normal = Some([n[0], n[2], n[1]]);
    }
}

/// Authored concept geometry, not a retail mesh. Source-unit dimensions are
/// specified in docs/spec/fa-xx.md. Reserved addresses cannot collide with SH.
const CONCEPT_HOOK_ANGLE: f64 = 0.75;
fn concept_hook() -> Vec<Face> {
    // Deployed wheel bottoms are z=-23. The shoe extends one unit aft and
    // 1.3 units below the shank hinge, so include both in its rotated reach.
    let length = ((18. - 1.3 * CONCEPT_HOOK_ANGLE.cos()) / CONCEPT_HOOK_ANGLE.sin() - 1.) as f32;
    let end = -26. - length;
    let mut faces = Vec::new();
    for (lo, hi) in [
        ([-0.45, end, -5.45], [0.45, -26., -4.55]),
        ([-0.9, end - 1., -6.3], [0.9, end + 3., -5.]),
    ] {
        for axis in 0..3 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            for upper in [false, true] {
                let mut positions = [(false, false), (true, false), (true, true), (false, true)]
                    .map(|(a, b)| {
                        let mut p = lo;
                        p[axis] = if upper { hi[axis] } else { lo[axis] };
                        p[u] = if a { hi[u] } else { lo[u] };
                        p[v] = if b { hi[v] } else { lo[v] };
                        p
                    });
                if !upper {
                    positions.reverse();
                }
                let mut normal = [0.; 3];
                normal[axis] = if upper { 32767. } else { -32767. };
                faces.push(Face {
                    positions: positions.to_vec(),
                    colors: vec![55; 4],
                    fog: Default::default(),
                    uv: Vec::new(),
                    texture: String::new(),
                    subtype: 0x20,
                    normal: Some([normal[0], normal[2], normal[1]]),
                    address: usize::MAX - faces.len(),
                });
            }
        }
    }
    faces
}

/// Agent-fitted travel between reviewed source device endpoints. No foreign rig offsets.
fn roster_flame_root(id: AircraftId) -> Option<f32> {
    match id {
        AircraftId::Mig29 => Some(-41.),
        AircraftId::Su27 => Some(-60.),
        AircraftId::Mig21 => Some(-56.),
        AircraftId::Su25 => Some(0.),
        AircraftId::Mig23 => Some(-29.),
        AircraftId::Su35 => Some(-59.),
        AircraftId::F22 | AircraftId::Faxx => Some(-48.),
        _ => None,
    }
}
fn roster_device(
    id: AircraftId,
    mut f: Face,
    part: Option<Part>,
    s: &State,
    flame_root: f32,
) -> Option<Face> {
    let scaling = match part {
        Some(Part::Flame) if s.exhaust > 0. => Some((1, flame_root, s.exhaust as f32)),
        Some(Part::Gear) if s.gear > 0. => {
            crate::roster_animation::gear(id, &mut f, s.gear);
            None
        }
        Some(Part::Brake) if s.brake > 0. => {
            crate::roster_animation::brake(id, &mut f, s.brake);
            None
        }
        Some(Part::Hook) if id == AircraftId::Faxx && s.hook > 1e-8 => {
            turn(
                &mut f,
                [0., -26., -5.],
                [1., 0., 0.],
                CONCEPT_HOOK_ANGLE * s.hook,
            );
            None
        }
        Some(Part::Bay) if s.bay > 0. => None,
        Some(_) => return None,
        None => {
            crate::roster_animation::animate(id, &mut f, s);
            None
        }
    };
    if let Some((axis, root, fraction)) = scaling {
        for p in &mut f.positions {
            p[axis] = root + (p[axis] - root) * fraction;
        }
        // Normals transform by inverse transpose under nonuniform scaling.
        if let Some(n) = &mut f.normal {
            let old_length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
            // Normal storage is right/up/forward; positions are right/forward/up.
            let normal_axis = [0, 2, 1][axis];
            n[normal_axis] /= fraction.max(f32::MIN_POSITIVE);
            let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
            if length.is_finite() && length > 0. {
                for v in n {
                    *v *= old_length / length;
                }
            }
        }
    }
    Some(f)
}

#[cfg(test)]
mod tests {
    #[test]
    fn concept_hook_hides_stowed_and_rotates_rigidly_downward() {
        let mut s = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        let faces = concept_hook();
        assert_eq!(faces.len(), 12);
        let rig = Rig {
            id: AircraftId::Faxx,
            parts: faces.iter().map(|f| (f.address, Part::Hook)).collect(),
        };
        s.hook = 1.;
        let lowest = faces
            .iter()
            .flat_map(|f| rig.animate(f, &s).unwrap().positions)
            .map(|p| p[2])
            .fold(f32::INFINITY, f32::min);
        assert!(
            (lowest + 23.).abs() < 1e-4,
            "hook tip must meet the level wheel plane"
        );
        s.hook = 0.;
        for source in &faces {
            assert!(rig.animate(source, &s).is_none());
            for fraction in [0.5, 1.] {
                s.hook = fraction;
                let moved = rig.animate(source, &s).unwrap();
                for (p, q) in source.positions.iter().zip(&moved.positions) {
                    let radius = |p: &[f32; 3]| (p[1] + 26.).powi(2) + (p[2] + 5.).powi(2);
                    assert!((radius(p) - radius(q)).abs() < 0.001);
                    assert_eq!(p[0], q[0]);
                    if p[1] < -41. {
                        assert!(q[2] < -10.);
                    }
                }
                assert_eq!(moved.colors, source.colors);
            }
            s.hook = 0.;
            assert!(rig.animate(source, &s).is_none());
        }
    }

    use super::*;
    fn face(address: usize) -> Face {
        Face {
            positions: vec![[0., 0., 0.], [3., 0., 0.], [0., 4., 0.]],
            colors: vec![1; 3],
            uv: vec![[0., 0.], [1., 0.], [0., 1.]],
            texture: String::new(),
            subtype: 0,
            normal: Some([0., 1., 0.]),
            address,
            fog: Default::default(),
        }
    }
    #[test]
    fn roster_devices_keep_roots_and_preserve_each_source_endpoint() {
        let mut state = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        for id in AircraftId::ALL {
            let Some(flame_root) = roster_flame_root(id) else {
                continue;
            };
            let rig = Rig {
                id,
                parts: [(7, Part::Gear), (8, Part::Flame), (9, Part::Brake)].into(),
            };
            for (address, axis, root) in [(7, 2, 0.), (8, 1, flame_root)] {
                let mut source = face(address);
                source.normal = Some([0., 1., 1.]);
                source.positions[0][axis] = root;
                source.positions[1][axis] = root - 10.;
                state.gear = 1.;
                state.exhaust = 1.;
                assert_eq!(
                    rig.animate(&source, &state).unwrap().positions,
                    source.positions
                );
                state.gear = 0.5;
                state.exhaust = 0.5;
                let half = rig.animate(&source, &state).unwrap();
                assert_eq!(half.uv, source.uv);
                if address == 7 {
                    let distance = |a: [f32; 3], b: [f32; 3]| {
                        a.iter().zip(b).map(|(a, b)| (a - b).powi(2)).sum::<f32>()
                    };
                    for i in 0..3 {
                        let j = (i + 1) % 3;
                        assert!(
                            (distance(half.positions[i], half.positions[j])
                                - distance(source.positions[i], source.positions[j]))
                            .abs()
                                < 0.001
                        );
                    }
                    assert_ne!(half.positions, source.positions);
                } else {
                    assert_eq!(half.positions[0], source.positions[0]);
                    assert_eq!(half.positions[1][axis], root - 5.);
                    let n = half.normal.unwrap();
                    assert!((n[2] / n[1] - 2.).abs() < 1e-6);
                    assert!((n.iter().map(|v| v * v).sum::<f32>() - 2.).abs() < 1e-6);
                }
                state.gear = 0.;
                state.exhaust = 0.;
                assert!(rig.animate(&source, &state).is_none());
            }
            state.brake = 0.;
            assert!(rig.animate(&face(9), &state).is_none());
            state.brake = 1.;
            assert_eq!(
                rig.animate(&face(9), &state).unwrap().positions,
                face(9).positions
            );
            assert_eq!(
                rig.animate(&face(10), &state).unwrap().positions,
                face(10).positions
            );
        }
    }
    #[test]
    fn device_endpoints_keep_source_geometry_and_hide_retracted_parts() {
        let rig = Rig {
            id: AircraftId::F14,
            parts: [(7, Part::Gear), (8, Part::Flame)].into(),
        };
        let mut state = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        state.gear = 1.;
        state.exhaust = 1.;
        let source = face(7);
        assert_eq!(
            rig.animate(&source, &state).unwrap().positions,
            source.positions
        );
        state.gear = 0.;
        state.exhaust = 0.;
        assert!(rig.animate(&source, &state).is_none());
        assert!(rig.animate(&face(8), &state).is_none());
    }
    #[test]
    fn a4_elevator_crosses_mesh_diagonals_without_moving_forward_stabilizer() {
        let rig = Rig {
            id: AircraftId::A4E,
            parts: BTreeMap::new(),
        };
        let mut state = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        state.elevator = 1.;
        for address in [
            0x4345, 0x4361, 0x44b5, 0x44d5, 0x4967, 0x4982, 0x49dd, 0x49f8,
        ] {
            let mut source = face(address);
            source.positions = vec![[0., -60., 7.], [4., -54., 7.], [0., -48., 7.]];
            let pieces = rig.faces(&source, &state);
            assert_eq!(pieces.len(), 2);
            let points: Vec<_> = pieces.iter().flat_map(|f| &f.positions).collect();
            assert!(points.contains(&&[0., -48., 7.]));
            assert!(points.contains(&&[4., -54., 7.]));
            assert!(!points.contains(&&[0., -60., 7.]));
            assert!(points.iter().any(|p| p[2] > 7.));
            for piece in pieces {
                assert_eq!(
                    rig.animate(&piece, &state).unwrap().positions,
                    piece.positions
                );
                assert_eq!(piece.positions.len(), piece.uv.len());
            }
        }
    }
    #[test]
    fn paddles_share_plume_demand_preserve_hinges_and_move_without_burner() {
        let rig = Rig {
            id: AircraftId::X31,
            parts: BTreeMap::new(),
        };
        let mut state = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        for (addresses, axis) in [
            ([0x44b2, 0x44da], [1., 0., 0.]),
            ([0x4406, 0x442e], [-1., 0., 2.]),
            ([0x4342, 0x4381], [1., 0., 2.]),
        ] {
            let mut source = face(addresses[0]);
            source.positions = vec![[0.; 3], axis, [axis[0], -5., axis[2]], [0., -5., 0.]];
            let mut back = source.clone();
            back.address = addresses[1];
            back.positions.reverse();
            state.auxiliary_rates = [0.; 3];
            assert_eq!(
                rig.animate(&source, &state).unwrap().positions,
                source.positions
            );
            state.auxiliary_rates = [0., std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2];
            assert_eq!(state.exhaust, 0.);
            let moved = rig.animate(&source, &state).unwrap();
            let mut rear = rig.animate(&back, &state).unwrap().positions;
            rear.reverse();
            assert_eq!(moved.positions, rear);
            for i in 0..2 {
                for j in 0..3 {
                    assert!((moved.positions[i][j] - source.positions[i][j]).abs() < 1e-6);
                }
            }
            assert_ne!(moved.positions[2], source.positions[2]);
            assert_eq!(moved.uv, source.uv);
        }
        let stationary = face(0x29f7);
        assert_eq!(
            rig.animate(&stationary, &state).unwrap().positions,
            stationary.positions
        );
    }
    #[test]
    fn x31_vector_plume_follows_live_rates_and_keeps_its_root() {
        let rig = Rig {
            id: AircraftId::X31,
            parts: [(8, Part::Flame)].into(),
        };
        let mut state = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        state.exhaust = 1.;
        let mut source = face(8);
        source.positions = vec![[0., -41., 0.], [0., -51., 0.], [1., -51., 0.]];
        assert_eq!(
            rig.animate(&source, &state).unwrap().positions,
            source.positions
        );
        state.auxiliary_rates[1] = std::f64::consts::FRAC_PI_2;
        let pitched = rig.animate(&source, &state).unwrap();
        assert_eq!(pitched.positions[0], source.positions[0]);
        assert!(pitched.positions[1][2] < 0.);
        state.auxiliary_rates = [0., 0., std::f64::consts::FRAC_PI_2];
        let yawed = rig.animate(&source, &state).unwrap();
        assert!(yawed.positions[1][0] < 0.);
        assert_eq!(yawed.uv, source.uv);
        state.exhaust = 0.;
        assert!(rig.animate(&source, &state).is_none());
    }
    #[test]
    fn sweep_respects_speed_and_flaps_and_rotations_preserve_shape() {
        let mut state = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        state.flaps = 0.;
        for (knots, degrees) in [
            (300., 0.),
            (400., 0.),
            (550., 24.),
            (700., 48.),
            (900., 48.),
        ] {
            state.speed = knots * 1.68781;
            assert!((sweep(&state).to_degrees() - degrees).abs() < 1e-9);
        }
        state.flaps = 1.;
        assert_eq!(sweep(&state), 0.);
        let mut f = face(0);
        let original = f.clone();
        turn(&mut f, [0.; 3], [0., 0., 10.], 0.7);
        assert_eq!(f.positions[0], [0.; 3]);
        for (a, b) in f.positions.iter().zip(&original.positions) {
            assert!(
                (a.iter().map(|v| v * v).sum::<f32>() - b.iter().map(|v| v * v).sum::<f32>()).abs()
                    < 1e-4
            );
        }
        assert_eq!(f.uv, original.uv);
        assert!((f.normal.unwrap().iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 1e-5);
    }
}
