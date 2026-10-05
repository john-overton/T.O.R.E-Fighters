//! Reviewed shape branches with fitted continuous device motion.
//! See docs/spec/variety-animation.md. No imported code executes.
use crate::{AppResult, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{
    aircraft::AircraftId as Id,
    shape::{Face, Shape},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Gear,
    Flame,
    Brake,
    Hook,
    Rudder,
}
struct Spec {
    code: usize,
    faces: usize,
    words: &'static [usize],
    branches: &'static [(usize, Part, usize)],
    scale: f32,
}
fn specification(id: Id) -> Option<Spec> {
    let (code, faces, words, branches, scale): (_, _, &[usize], &[(usize, Part, usize)], _) =
        match id {
            Id::C130 => (10812, 144, &[0x3a30], &[(0x3a30, Part::Gear, 6)], 2. / 3.),
            Id::Ac130 => (19666, 339, &[0x5cc0], &[(0x5cc0, Part::Gear, 8)], 2. / 3.),
            Id::E3 => (
                26794,
                296,
                &[0x7880, 0x7886, 0x7892, 0x7898, 0x789e],
                &[(0x7886, Part::Gear, 11), (0x789e, Part::Rudder, 2)],
                2. / 3.,
            ),
            Id::Il76 => (
                28644,
                309,
                &[0x7fc0, 0x7fc6, 0x7fcc, 0x7fd2],
                &[(0x7fc6, Part::Gear, 20)],
                2. / 3.,
            ),
            Id::E2 => (
                29258,
                345,
                &[0x8220, 0x8226, 0x8232, 0x8238, 0x823e],
                &[(0x8226, Part::Gear, 10), (0x8232, Part::Hook, 2)],
                2. / 3.,
            ),
            Id::Av8 => (
                18014,
                270,
                &[0x5640, 0x564c, 0x5652],
                &[(0x5640, Part::Gear, 8)],
                1. / 3.,
            ),
            Id::Yak141 => (
                21134,
                308,
                &[0x6270, 0x627c, 0x6282],
                &[(0x6270, Part::Gear, 4)],
                1. / 3.,
            ),
            Id::V22 => (
                17204,
                160,
                &[0x5310, 0x531c, 0x5322],
                &[(0x5310, Part::Gear, 12)],
                1. / 3.,
            ),
            Id::Ah64 => (20460, 370, &[], &[], 1. / 3.),
            Id::Mi24 => (
                24994,
                387,
                &[0x7190, 0x7196],
                &[(0x7196, Part::Gear, 6)],
                1. / 3.,
            ),
            Id::Ch47 => (18329, 231, &[], &[], 1. / 3.),
            Id::Mig17 => (
                20602,
                273,
                &[0x6050, 0x6056, 0x605c, 0x6068, 0x606e],
                &[
                    (0x6050, Part::Flame, 5),
                    (0x6056, Part::Brake, 10),
                    (0x605c, Part::Gear, 22),
                ],
                1. / 3.,
            ),
            Id::F4B | Id::F4J => (
                25222,
                309,
                &[0x7250, 0x7256, 0x725c, 0x7268, 0x726e, 0x7274, 0x727a],
                &[
                    (0x7250, Part::Flame, 16),
                    (0x7256, Part::Brake, 6),
                    (0x725c, Part::Gear, 22),
                    (0x7268, Part::Hook, 2),
                    (0x727a, Part::Rudder, 2),
                ],
                1. / 3.,
            ),
            Id::F4E => (
                20000,
                213,
                &[0x5df0, 0x5df6, 0x5dfc, 0x5e08, 0x5e0e, 0x5e14],
                &[
                    (0x5df0, Part::Flame, 8),
                    (0x5df6, Part::Brake, 2),
                    (0x5dfc, Part::Gear, 6),
                    (0x5e14, Part::Rudder, 2),
                ],
                1. / 3.,
            ),
            Id::F4G => (
                21456,
                233,
                &[0x63a0, 0x63a6, 0x63ac, 0x63b8, 0x63be, 0x63c4],
                &[
                    (0x63a0, Part::Flame, 8),
                    (0x63a6, Part::Brake, 2),
                    (0x63ac, Part::Gear, 6),
                    (0x63c4, Part::Rudder, 2),
                ],
                1. / 3.,
            ),
            Id::A7 => (
                23270,
                261,
                &[0x6ab0, 0x6ab6, 0x6abc, 0x6ac8, 0x6ace, 0x6ad4, 0x6ada],
                &[
                    (0x6ab0, Part::Flame, 4),
                    (0x6ab6, Part::Brake, 10),
                    (0x6abc, Part::Gear, 26),
                    (0x6ac8, Part::Hook, 2),
                    (0x6ada, Part::Rudder, 2),
                ],
                1. / 3.,
            ),
            Id::F15 => (
                30762,
                346,
                &[0x8800, 0x8806, 0x880c, 0x8818, 0x881e],
                &[
                    (0x8800, Part::Flame, 8),
                    (0x8806, Part::Brake, 4),
                    (0x880c, Part::Gear, 22),
                ],
                1. / 3.,
            ),
            Id::F16C => (
                31488,
                356,
                &[0x8ad0, 0x8ad6, 0x8adc, 0x8ae8, 0x8aee, 0x8af4],
                &[
                    (0x8ad0, Part::Flame, 6),
                    (0x8ad6, Part::Brake, 8),
                    (0x8adc, Part::Gear, 25),
                    (0x8af4, Part::Rudder, 2),
                ],
                1. / 3.,
            ),
            Id::F104 => (
                28374,
                297,
                &[0x7ea0, 0x7ea6, 0x7eac, 0x7eb8, 0x7ebe, 0x7ec4, 0x7eca],
                &[
                    (0x7ea0, Part::Flame, 4),
                    (0x7ea6, Part::Brake, 8),
                    (0x7eac, Part::Gear, 24),
                    (0x7eb8, Part::Hook, 2),
                    (0x7eca, Part::Rudder, 2),
                ],
                1. / 3.,
            ),
            Id::A10 => (
                19854,
                303,
                &[0x5d70, 0x5d7c, 0x5d82],
                &[(0x5d70, Part::Gear, 20)],
                1. / 3.,
            ),
            Id::B747 => (
                31108,
                373,
                &[0x8960, 0x896c, 0x8972, 0x8978],
                &[(0x8960, Part::Gear, 36), (0x8978, Part::Rudder, 2)],
                2. / 3.,
            ),
            Id::A310 => (
                20868,
                248,
                &[0x6160, 0x616c, 0x6172, 0x6178],
                &[(0x6160, Part::Gear, 14), (0x6178, Part::Rudder, 2)],
                2. / 3.,
            ),
            _ => return None,
        };
    Some(Spec {
        code,
        faces,
        words,
        branches,
        scale,
    })
}

pub fn supported(id: Id) -> bool {
    specification(id).is_some()
}

#[derive(Clone, Copy)]
struct Group {
    part: Part,
    min: [f32; 3],
    max: [f32; 3],
    closed: bool,
}
enum SpecificRig {
    A7(crate::a7_animation::Rig),
    A310(crate::a310_animation::Rig),
    F4(Box<crate::f4_animation::Rig>),
    F15(crate::f15_animation::Rig),
    F16(crate::f16_animation::Rig),
    F104(crate::f104_animation::Rig),
    Mig17(crate::mig17_animation::Rig),
}
pub struct Rig {
    id: Id,
    groups: BTreeMap<usize, Group>,
    scale: f32,
    specific: Option<SpecificRig>,
}
impl Rig {
    pub fn load(id: Id, bytes: &[u8]) -> AppResult<(Self, Shape)> {
        let spec = specification(id).ok_or("unreviewed variety aircraft rig")?;
        let mut shape = Shape::parse(bytes)?;
        match id {
            Id::A7 => {
                let (rig, shape) = crate::a7_animation::Rig::load(bytes, shape)?;
                return Ok(Self::with_specific(
                    id,
                    spec.scale,
                    SpecificRig::A7(rig),
                    shape,
                ));
            }
            Id::F4B | Id::F4J | Id::F4E | Id::F4G => {
                let (rig, shape) = crate::f4_animation::Rig::load(id, bytes, shape)?;
                return Ok(Self::with_specific(
                    id,
                    spec.scale,
                    SpecificRig::F4(Box::new(rig)),
                    shape,
                ));
            }
            Id::F15 => {
                let (rig, shape) = crate::f15_animation::Rig::load(bytes, shape)?;
                return Ok(Self::with_specific(
                    id,
                    spec.scale,
                    SpecificRig::F15(rig),
                    shape,
                ));
            }
            Id::F16C => {
                let (rig, shape) = crate::f16_animation::Rig::load(bytes, shape)?;
                return Ok(Self::with_specific(
                    id,
                    spec.scale,
                    SpecificRig::F16(rig),
                    shape,
                ));
            }
            Id::Mig17 => {
                let (rig, shape) = crate::mig17_animation::Rig::load(bytes, shape)?;
                return Ok(Self::with_specific(
                    id,
                    spec.scale,
                    SpecificRig::Mig17(rig),
                    shape,
                ));
            }
            Id::A310 => {
                let (rig, shape) = crate::a310_animation::Rig::load(bytes, shape)?;
                return Ok(Self::with_specific(
                    id,
                    spec.scale,
                    SpecificRig::A310(rig),
                    shape,
                ));
            }
            Id::F104 => {
                let (rig, shape) = crate::f104_animation::Rig::load(bytes, shape)?;
                return Ok(Self::with_specific(
                    id,
                    spec.scale,
                    SpecificRig::F104(rig),
                    shape,
                ));
            }
            _ => {}
        }
        if tore_formats::module::code(bytes)?.0.len() != spec.code
            || shape.faces.len() != spec.faces
            || shape.state_words != spec.words.iter().copied().collect()
        {
            return Err(format!(
                "unreviewed {} shape layout; preserve source and review rig",
                id.label()
            )
            .into());
        }
        let neutral: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let mut removed = BTreeSet::new();
        let mut added = Vec::new();
        let mut groups = BTreeMap::new();
        for &(word, part, count) in spec.branches {
            let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
            let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
            let faces: Vec<_> = pose
                .faces
                .into_iter()
                .filter(|f| !neutral.contains(&f.address))
                .collect();
            if faces.len() != count {
                return Err(format!("unreviewed {} device branch {word:x}", id.label()).into());
            }
            if part == Part::Rudder {
                removed.extend(neutral.difference(&active).copied());
            } else {
                // F16.SH swaps eight closed airbrake skins for eight open
                // panels. Keep the closed branch until the device moves.
                for address in neutral.difference(&active) {
                    groups.insert(
                        *address,
                        Group {
                            part,
                            min: [0.; 3],
                            max: [0.; 3],
                            closed: true,
                        },
                    );
                }
            }
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for p in faces.iter().flat_map(|f| &f.positions) {
                for axis in 0..3 {
                    min[axis] = min[axis].min(p[axis]);
                    max[axis] = max[axis].max(p[axis]);
                }
            }
            for face in faces {
                groups.insert(
                    face.address,
                    Group {
                        part,
                        min,
                        max,
                        closed: false,
                    },
                );
                added.push(face);
            }
        }
        shape.faces.retain(|f| !removed.contains(&f.address));
        shape.faces.extend(added);
        Ok((
            Self {
                id,
                groups,
                scale: spec.scale,
                specific: None,
            },
            shape,
        ))
    }
    fn with_specific(id: Id, scale: f32, rig: SpecificRig, shape: Shape) -> (Self, Shape) {
        (
            Self {
                id,
                groups: BTreeMap::new(),
                scale,
                specific: Some(rig),
            },
            shape,
        )
    }
    pub fn scale(&self) -> f32 {
        self.scale
    }
    pub fn flame(&self, address: usize) -> bool {
        if let Some(rig) = &self.specific {
            return match rig {
                SpecificRig::A7(_) => crate::a7_animation::flame(address),
                SpecificRig::A310(_) => false,
                SpecificRig::F4(rig) => rig.flame(address),
                SpecificRig::F15(_) => crate::f15_animation::flame(address),
                SpecificRig::F16(_) => crate::f16_animation::flame(address),
                SpecificRig::F104(_) => crate::f104_animation::flame(address),
                SpecificRig::Mig17(_) => crate::mig17_animation::flame(address),
            };
        }
        self.groups
            .get(&address)
            .is_some_and(|g| g.part == Part::Flame)
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        if !crate::variety_rotors::keep_face(self.id, source.address) {
            return None;
        }
        let mut face = self.animate_devices(source, state)?;
        // Aircraft-specific control rigs must retain the shared propeller,
        // tiltrotor and manually selected gun articulation paths.
        crate::variety_rotors::animate(self.id, &mut face, state);
        if self.id == Id::Ac130 {
            animate_gun(&mut face, state);
        }
        Some(face)
    }
    fn animate_devices(&self, source: &Face, state: &State) -> Option<Face> {
        if let Some(rig) = &self.specific {
            return match rig {
                SpecificRig::A7(rig) => rig.animate(source, state),
                SpecificRig::A310(rig) => rig.animate(source, state),
                SpecificRig::F4(rig) => rig.animate(source, state),
                SpecificRig::F15(rig) => rig.animate(source, state),
                SpecificRig::F16(rig) => rig.animate(source, state),
                SpecificRig::F104(rig) => rig.animate(source, state),
                SpecificRig::Mig17(rig) => rig.animate(source, state),
            };
        }
        let mut face = source.clone();
        if let Some(group) = self.groups.get(&face.address) {
            if group.closed {
                let fraction = match group.part {
                    Part::Gear => state.gear,
                    Part::Brake => state.brake,
                    Part::Hook => state.hook,
                    Part::Flame => state.exhaust,
                    Part::Rudder => 1.,
                };
                return (fraction <= 0.).then_some(face);
            }
            let center: [f32; 3] = std::array::from_fn(|i| (group.min[i] + group.max[i]) * 0.5);
            match group.part {
                Part::Gear => {
                    if state.gear <= 0. {
                        return None;
                    }
                    let rise = (group.max[2] - group.min[2]).max(1.) * (1. - state.gear as f32);
                    for p in &mut face.positions {
                        p[2] += rise;
                    }
                }
                Part::Flame => {
                    if state.exhaust <= 0. {
                        return None;
                    }
                    for p in &mut face.positions {
                        p[1] = group.max[1] + (p[1] - group.max[1]) * state.exhaust as f32;
                    }
                }
                Part::Brake => {
                    if state.brake <= 0. {
                        return None;
                    }
                    crate::additional_animation::turn(
                        &mut face,
                        [center[0], group.max[1], group.min[2]],
                        [1., 0., 0.],
                        -(1. - state.brake) * 0.7,
                    );
                }
                Part::Hook => {
                    if state.hook <= 0. {
                        return None;
                    }
                    crate::additional_animation::turn(
                        &mut face,
                        [center[0], group.max[1], group.max[2]],
                        [1., 0., 0.],
                        (1. - state.hook) * 1.2,
                    );
                }
                Part::Rudder => {
                    crate::additional_animation::turn(
                        &mut face,
                        [center[0], group.max[1], group.min[2]],
                        [0., 0., 1.],
                        state.rudder * 0.35,
                    );
                }
            }
        }
        Some(face)
    }
}

/// Aim the original barrel axis at the same direction used for shot emission.
fn animate_gun(face: &mut Face, state: &State) {
    let groups: [&[usize]; 3] = [
        &[0x2850, 0x286b, 0x2886, 0x28a1],
        &[0x2439, 0x24e7, 0x2502, 0x2542],
        &[0x255d, 0x2578],
    ];
    let Some(slot) = groups
        .iter()
        .position(|group| group.contains(&face.address))
    else {
        return;
    };
    let [heading, elevation] = state.gun_aim[slot];
    // Old recordings and a plain headless flight have no combat mount pose.
    if heading == 0. && elevation == 0. {
        return;
    }
    use std::f64::consts::{FRAC_PI_2, PI};
    use tore_sim::combat::gunship::{PIVOTS_SOURCE, TIPS_SOURCE};
    let pivot = PIVOTS_SOURCE[slot];
    let from = tore_sim::attitude::unit(std::array::from_fn(|i| TIPS_SOURCE[slot][i] - pivot[i]));
    let (h, e) = (heading * PI, elevation * FRAC_PI_2);
    let to = [h.sin() * e.cos(), h.cos() * e.cos(), e.sin()];
    let axis = [
        from[1] * to[2] - from[2] * to[1],
        from[2] * to[0] - from[0] * to[2],
        from[0] * to[1] - from[1] * to[0],
    ];
    let length = tore_sim::attitude::dot(axis, axis).sqrt();
    if length > 1e-9 {
        crate::additional_animation::turn(
            face,
            pivot.map(|v| v as f32),
            axis.map(|v| (v / length) as f32),
            tore_sim::attitude::dot(from, to).clamp(-1., 1.).acos(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_airbrake_skin_returns_when_the_open_panel_stows() {
        let mut state = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        let rig = Rig {
            id: Id::F16C,
            scale: 1. / 3.,
            specific: None,
            groups: [
                (
                    1,
                    Group {
                        part: Part::Brake,
                        min: [0.; 3],
                        max: [1.; 3],
                        closed: true,
                    },
                ),
                (
                    2,
                    Group {
                        part: Part::Brake,
                        min: [0.; 3],
                        max: [1.; 3],
                        closed: false,
                    },
                ),
            ]
            .into(),
        };
        let mut face = Face {
            positions: vec![[0.; 3], [1., 0., 0.], [0., 1., 0.]],
            colors: vec![0; 3],
            fog: Default::default(),
            uv: vec![],
            texture: String::new(),
            subtype: 0,
            normal: None,
            address: 1,
        };
        for fraction in [0., 0.5, 1., 0.] {
            state.brake = fraction;
            face.address = 1;
            assert_eq!(rig.animate(&face, &state).is_some(), fraction == 0.);
            face.address = 2;
            assert_eq!(rig.animate(&face, &state).is_some(), fraction > 0.);
        }
    }
    #[test]
    fn animated_barrel_tip_matches_the_simulated_muzzle() {
        use tore_sim::combat::gunship::{PIVOTS_SOURCE, SOURCE_SCALE, TIPS_SOURCE};
        let mut state = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        for (slot, address) in [0x2850, 0x2439, 0x255d].into_iter().enumerate() {
            for (heading, elevation) in [(-0.5, 0.), (-0.6, 0.15), (-0.4, -0.2)] {
                state.gun_aim[slot] = [heading, elevation];
                let mut face = Face {
                    positions: vec![
                        PIVOTS_SOURCE[slot].map(|v| v as f32),
                        TIPS_SOURCE[slot].map(|v| v as f32),
                    ],
                    colors: vec![],
                    fog: Default::default(),
                    uv: vec![],
                    texture: String::new(),
                    subtype: 0,
                    normal: None,
                    address,
                };
                let rig = Rig {
                    id: Id::Ac130,
                    scale: SOURCE_SCALE as f32,
                    specific: None,
                    groups: BTreeMap::new(),
                };
                face = rig.animate(&face, &state).unwrap();
                let actual = [
                    f64::from(face.positions[1][0]),
                    f64::from(face.positions[1][2]),
                    f64::from(face.positions[1][1]),
                ]
                .map(|v| v * SOURCE_SCALE);
                let expected = tore_sim::combat::gunship::muzzle(
                    slot,
                    tore_world::combat::launcher(&state),
                    heading * std::f64::consts::PI,
                    elevation * std::f64::consts::FRAC_PI_2,
                );
                // The synthetic state has an arbitrary initial heading; compare
                // in its body frame using the same launcher transform.
                let basis = crate::attitude::Basis::new(state.yaw, state.pitch, state.bank);
                let offset = std::array::from_fn(|i| expected[i] - state.position[i]);
                let expected_local = [basis.right, basis.up, basis.forward]
                    .map(|axis| tore_sim::attitude::dot(axis, offset));
                for axis in 0..3 {
                    assert!(
                        (actual[axis] - expected_local[axis]).abs() < 1e-4,
                        "slot {slot}, axis {axis}: {actual:?} {expected_local:?}"
                    );
                }
            }
        }
    }
}
