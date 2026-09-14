//! Fitted presentation rig over reviewed F18.SH polygons, not native animation laws.
use crate::{
    attitude::{Basis, dot},
    flight::State,
};
use tore_formats::shape::Face;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    Body,
    Flame,
    Nozzle,
    Brake,
    Hook,
    GearLeft,
    GearRight,
    GearNose,
    DoorLeft,
    DoorRight,
    DoorNose,
    FlapLeft,
    FlapRight,
    TailLeft,
    TailRight,
}
pub fn part(address: usize) -> Part {
    use Part::*;
    match address {
        0x5310..=0x5421 => Flame,
        0x3ff7 | 0x4026 | 0x404d | 0x4068 => Nozzle,
        0x5059 | 0x5080 => Brake,
        0x4a03 | 0x4a22 => Hook,
        0x4b69 | 0x4b80 => DoorLeft,
        0x4abe | 0x4ad5 => DoorRight,
        0x4e86 | 0x4e9d => DoorNose,
        0x4bfa..=0x4cb1 => GearRight,
        0x4d33..=0x4dea => GearLeft,
        0x4ee1 | 0x4f00 | 0x4f64 | 0x4f83 | 0x4fa2 | 0x4fc1 => GearNose,
        0x525b | 0x5282 => FlapLeft,
        0x5154 | 0x517b => FlapRight,
        0x449f | 0x453c | 0x4563 | 0x458a => TailLeft,
        0x486b | 0x4894 | 0x4992 | 0x49b9 => TailRight,
        _ => Body,
    }
}
fn rotate(v: [f32; 3], axis: [f64; 3], angle: f64) -> [f32; 3] {
    let r = Basis::new(0., 0., 0.).rotated(axis.map(|x| x * angle));
    std::array::from_fn(|i| {
        (r.right[i] * v[0] as f64 + r.up[i] * v[1] as f64 + r.forward[i] * v[2] as f64) as f32
    })
}
/// Source-space pivot/axis. Endpoint geometry remains exact at full deployment.
fn hinge(part: Part, s: &State) -> ([f32; 3], [f64; 3], f64) {
    use Part::*;
    let x = [1., 0., 0.];
    let y = [0., 1., 0.];
    match part {
        Brake => ([0., -28., 5.], x, (10f64 / 18.).atan() * (1. - s.brake)),
        Hook => ([0., -37.5, -3.], x, -(18f64 / 11.5).atan() * (1. - s.hook)),
        GearRight => ([8., -7., -6.], y, 1.45 * (1. - s.gear)),
        GearLeft => ([-7., -7., -6.], y, -1.45 * (1. - s.gear)),
        GearNose => (
            [0., 55., -6.],
            x,
            -std::f64::consts::FRAC_PI_2 * (1. - s.gear),
        ),
        DoorLeft => ([-3., 0., -6.], y, -1.35 * (1. - (s.gear * 4.).min(1.))),
        DoorRight => ([4., 0., -6.], y, 1.35 * (1. - (s.gear * 4.).min(1.))),
        DoorNose => (
            [-1., 53., -6.],
            y,
            -std::f64::consts::FRAC_PI_2 * (1. - (s.gear * 4.).min(1.)),
        ),
        FlapLeft => ([-13., -8., 4.], [25., 2., 2.], s.flaps * 0.52),
        FlapRight => ([13., -7., 4.], [26., -3., -2.], s.flaps * 0.52),
        TailLeft => ([-11., -43., 0.], x, -s.elevator * 0.30 + s.aileron * 0.20),
        TailRight => ([11., -43., 0.], x, -s.elevator * 0.30 - s.aileron * 0.20),
        _ => ([0.; 3], x, 0.),
    }
}
pub fn animate(face: &Face, s: &State) -> Option<Face> {
    use Part::*;
    let part = part(face.address);
    let fraction = match part {
        Flame => s.exhaust,
        Brake => s.brake,
        Hook => s.hook,
        GearLeft | GearRight | GearNose | DoorLeft | DoorRight | DoorNose => s.gear,
        _ => 1.,
    };
    if fraction <= 0. {
        return None;
    }
    let mut result = face.clone();
    if part == Flame {
        for p in &mut result.positions {
            p[1] = -60. + (p[1] + 60.) * s.exhaust as f32;
        }
        return Some(result);
    }
    let (pivot, axis, angle) = hinge(part, s);
    if angle != 0. {
        let length = dot(axis, axis).sqrt();
        let axis = axis.map(|x| x / length);
        for p in &mut result.positions {
            let v = rotate(std::array::from_fn(|i| p[i] - pivot[i]), axis, angle);
            *p = std::array::from_fn(|i| pivot[i] + v[i]);
        }
        if let Some(n) = result.normal {
            // SH positions are X/forward/up; stored normals are X/up/forward.
            let n = rotate([n[0], n[2], n[1]], axis, angle);
            result.normal = Some([n[0], n[2], n[1]]);
        }
    }
    Some(result)
}

/// Split the original fin at a fitted trailing-rudder hinge, retaining UVs.
/// Native partition and deflection schedule remain unverified.
pub fn rudder_faces(face: &Face, s: &State) -> Vec<Face> {
    if !matches!(face.address, 0x5467 | 0x548e | 0x54d4 | 0x54fb) || s.rudder.abs() < 1e-8 {
        return vec![face.clone()];
    }
    let side = if face.positions[0][0] < 0. { -1. } else { 1. };
    let pivot = [side * 8., -32., 4.];
    let axis = [if side < 0. { -11. } else { 12. }, -7., 26.];
    let length = dot(axis, axis).sqrt();
    let axis = axis.map(|x| x / length);
    let distance = |p: [f32; 3]| p[1] + 32. + (p[2] - 4.) * 7. / 26.;
    let mut result = Vec::new();
    for moving in [false, true] {
        let mut f = face.clone();
        f.positions.clear();
        f.colors.clear();
        f.uv.clear();
        for i in 0..face.positions.len() {
            let j = (i + 1) % face.positions.len();
            let a = face.positions[i];
            let b = face.positions[j];
            let da = distance(a);
            let db = distance(b);
            let inside = if moving { da <= 0. } else { da >= 0. };
            if inside {
                f.positions.push(a);
                f.colors.push(face.colors[i]);
                if !face.uv.is_empty() {
                    f.uv.push(face.uv[i]);
                }
            }
            if (da < 0. && db > 0.) || (da > 0. && db < 0.) {
                let t = da / (da - db);
                f.positions
                    .push(std::array::from_fn(|k| a[k] + t * (b[k] - a[k])));
                f.colors.push(face.colors[i]);
                if !face.uv.is_empty() {
                    f.uv.push(std::array::from_fn(|k| {
                        face.uv[i][k] + t * (face.uv[j][k] - face.uv[i][k])
                    }));
                }
            }
        }
        if f.positions.len() < 3 {
            continue;
        }
        if moving {
            for p in &mut f.positions {
                let v = rotate(
                    std::array::from_fn(|i| p[i] - pivot[i]),
                    axis,
                    s.rudder * 0.35,
                );
                *p = std::array::from_fn(|i| pivot[i] + v[i]);
            }
            if let Some(n) = f.normal {
                let n = rotate([n[0], n[2], n[1]], axis, s.rudder * 0.35);
                f.normal = Some([n[0], n[2], n[1]]);
            }
        }
        result.push(f);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotations_keep_hinges_fixed_and_normals_unit() {
        for angle in [-1.57, -0.5, 0., 0.5, 1.57] {
            let v = rotate([0., -10., 0.], [1., 0., 0.], angle);
            assert!((v.iter().map(|x| x * x).sum::<f32>() - 100.).abs() < 1e-4);
            assert_eq!(rotate([0.; 3], [1., 0., 0.], angle), [0.; 3]);
            if angle > 0. {
                assert!(v[2] < 0.);
            }
        }
    }
}
