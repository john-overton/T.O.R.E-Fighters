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
        GearLeft => ([-4., 10., -7.], y, -1.5 * (1. - s.gear)),
        GearRight => ([4., 10., -7.], y, 1.5 * (1. - s.gear)),
        GearNose => ([0., 60., -6.], x, -1.57 * (1. - s.gear)),
        DoorLeft => ([-2., 7., -7.], y, 1.57 * door),
        DoorRight => ([2., 7., -7.], y, -1.57 * door),
        DoorNose => ([-1., 52., -5.], y, -1.57 * door),
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
    Some(f)
}
