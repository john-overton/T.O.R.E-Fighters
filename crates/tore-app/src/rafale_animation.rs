//! Reviewed RAF.SH parts with fitted hinges/control mixing, not native animation laws.
use crate::{aircraft_animation::rotate, attitude::dot, flight::State};
use tore_formats::shape::{Face, Shape};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    Body,
    Flame,
    Nozzle,
    BrakeLeft,
    BrakeRight,
    GearLeft,
    GearRight,
    GearNose,
    DoorLeft,
    DoorRight,
    DoorNose,
    FlapLeft,
    FlapRight,
    CanardLeft,
    CanardRight,
    Rudder,
}
pub fn part(address: usize) -> Part {
    use Part::*;
    match address {
        0x4265..=0x44ae => Flame,
        0x3032 | 0x3056 | 0x307a | 0x3098 => Nozzle,
        0x3e1e | 0x3e35 => BrakeRight,
        0x3ebb | 0x3ed2 => BrakeLeft,
        0x3c50 | 0x3c6f => GearLeft,
        0x3be5 | 0x3c04 => GearRight,
        0x3cbb | 0x3cda => GearNose,
        0x3b2f | 0x3b46 => DoorLeft,
        0x3ad4 | 0x3aeb => DoorRight,
        0x3b8a | 0x3ba1 => DoorNose,
        0x4190 | 0x41af => FlapLeft,
        0x40a9 | 0x40c8 => FlapRight,
        0x3d81 | 0x3d98 => CanardLeft,
        0x3d26 | 0x3d3d => CanardRight,
        0x3f08 | 0x3f27 => Rudder,
        _ => Body,
    }
}
/// Guard both gear branches before applying source-address-specific geometry rules.
pub fn validate(poses: &[Shape]) -> Result<(), String> {
    use Part::*;
    for (index, pose) in poses.iter().enumerate() {
        for (group, count) in [
            (Flame, 16),
            (Nozzle, 4),
            (BrakeLeft, 2),
            (BrakeRight, 2),
            (GearLeft, index * 2),
            (GearRight, index * 2),
            (GearNose, index * 2),
            (DoorLeft, index * 2),
            (DoorRight, index * 2),
            (DoorNose, index * 2),
            (FlapLeft, 2),
            (FlapRight, 2),
            (CanardLeft, 2),
            (CanardRight, 2),
            (Rudder, 2),
        ] {
            if pose
                .faces
                .iter()
                .filter(|f| part(f.address) == group)
                .count()
                != count
            {
                return Err(format!(
                    "unreviewed RAF animation group {group:?} in gear pose {index}"
                ));
            }
        }
    }
    for face in poses.iter().flat_map(|pose| &pose.faces) {
        let roots: Vec<[f32; 3]> = match part(face.address) {
            GearLeft => vec![[-4., 5., -7.], [-4., 15., -7.]],
            GearRight => vec![[4., 5., -7.], [4., 15., -7.]],
            DoorNose => vec![[-1., 52., -6.], [-1., 68., -5.]],
            FlapLeft | FlapRight => {
                if face.positions.iter().filter(|p| p[1] == -23.).count() != 2 {
                    return Err("unreviewed RAF flap attachment seams".into());
                }
                Vec::new()
            }
            _ => Vec::new(),
        };
        if !roots.iter().all(|p| face.positions.contains(p)) {
            return Err(format!("unreviewed RAF attachment {:x}", face.address));
        }
    }
    Ok(())
}

pub fn animate(face: &Face, s: &State) -> Option<Face> {
    use Part::*;
    let group = part(face.address);
    if (group == Flame && s.exhaust <= 0.)
        || (matches!(group, BrakeLeft | BrakeRight) && s.brake <= 0.)
        || (matches!(
            group,
            GearLeft | GearRight | GearNose | DoorLeft | DoorRight | DoorNose
        ) && s.gear <= 0.)
    {
        return None;
    }
    let mut f = face.clone();
    if group == Flame {
        for p in &mut f.positions {
            p[1] = -51. + (p[1] + 51.) * s.exhaust as f32;
        }
        return Some(f);
    }
    let x = [1., 0., 0.];
    let y = [0., 1., 0.];
    let door = 1. - (s.gear * 4.).min(1.);
    let (pivot, axis, angle) = match group {
        BrakeRight => ([2., -3., 4.], [6., 0., -2.], 0.85 * (1. - s.brake)),
        BrakeLeft => ([-2., -3., 4.], [6., 0., 2.], 0.85 * (1. - s.brake)),
        GearLeft => (
            [-4., 14.545455, -7.],
            x,
            -std::f64::consts::FRAC_PI_2 * (1. - s.gear),
        ),
        GearRight => (
            [4., 14.545455, -7.],
            x,
            -std::f64::consts::FRAC_PI_2 * (1. - s.gear),
        ),
        GearNose => ([0., 60., -6.], x, -1.57 * (1. - s.gear)),
        DoorLeft => ([-2., 7., -7.], y, 1.57 * door),
        DoorRight => ([2., 7., -7.], y, -1.57 * door),
        DoorNose => ([-1., 52., -6.], [0., 16., 1.], -1.57 * door),
        FlapLeft => (
            [-10., -23., -1.],
            x,
            s.flaps * 0.40 - s.elevator * 0.30 + s.aileron * 0.20,
        ),
        FlapRight => (
            [10., -23., -1.],
            x,
            s.flaps * 0.40 - s.elevator * 0.30 - s.aileron * 0.20,
        ),
        CanardLeft => ([-10., 35., 1.], x, s.elevator * 0.35),
        CanardRight => ([10., 35., 1.], x, s.elevator * 0.35),
        Rudder => ([0., -36., 5.], [0., -8., 25.], s.rudder * 0.35),
        _ => ([0.; 3], x, 0.),
    };
    if angle != 0. {
        let length = dot(axis, axis).sqrt();
        let axis = axis.map(|v| v / length);
        for p in &mut f.positions {
            let v = rotate(std::array::from_fn(|i| p[i] - pivot[i]), axis, angle);
            *p = std::array::from_fn(|i| pivot[i] + v[i]);
        }
        if let Some(n) = f.normal {
            let n = rotate([n[0], n[2], n[1]], axis, angle);
            f.normal = Some([n[0], n[2], n[1]]);
        }
    }
    if angle != 0. && matches!(group, FlapLeft | FlapRight) {
        for (original, moved) in face.positions.iter().zip(&mut f.positions) {
            if original[1] == -23. {
                *moved = *original;
            }
        }
        crate::aircraft_animation::update_normal(face, &mut f);
    }
    if group == GearNose {
        crate::additional_animation::turn(
            &mut f,
            [0., 60., -6.],
            [0., 0., 1.],
            -s.nosewheel_angle(),
        );
    }
    Some(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let n = positions.len();
        Face {
            address,
            positions,
            colors: vec![12; n],
            uv: vec![],
            texture: String::new(),
            subtype: 0,
            normal: Some([0., 1., 0.]),
            fog: Default::default(),
        }
    }
    #[test]
    fn main_cards_keep_their_side_and_painted_fore_attachment() {
        let mut s = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        for (address, x) in [(0x3c50, -4.), (0x3be5, 4.)] {
            let source = face(
                address,
                vec![
                    [x, 14.545455, -7.],
                    [x, 7., -7.],
                    [x, 7., -16.],
                    [x, 14.545455, -16.],
                ],
            );
            for step in 1..=400 {
                s.gear = step as f64 / 400.;
                let moved = animate(&source, &s).unwrap();
                assert_eq!(moved.positions[0], source.positions[0]);
                assert!(moved.positions.iter().all(|p| p[0] == x));
                assert!(!crate::aircraft_animation_probe::planar_crossing(
                    &moved, 1.
                ));
            }
        }
    }
    #[test]
    fn thick_flap_fronts_stay_fixed_through_all_mixed_commands() {
        let source = face(
            0x40a9,
            vec![
                [11., -23., 0.],
                [51., -23., -1.],
                [51., -29., -2.],
                [11., -30., -2.],
            ],
        );
        let mut s = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        for flap in [0., 0.5, 1.] {
            for pitch in [-1., 0., 1.] {
                for roll in [-1., 0., 1.] {
                    s.flaps = flap;
                    s.elevator = pitch;
                    s.aileron = roll;
                    let moved = animate(&source, &s).unwrap();
                    assert_eq!(&moved.positions[..2], &source.positions[..2]);
                    assert!(!crate::aircraft_animation_probe::planar_crossing(
                        &moved, 1.
                    ));
                    assert!(moved.normal.unwrap().iter().all(|v| v.is_finite()));
                }
            }
        }
    }
}
