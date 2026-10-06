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
pub(crate) fn rotate(v: [f32; 3], axis: [f64; 3], angle: f64) -> [f32; 3] {
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
        GearRight => (
            [9., -7., -6.],
            [1., 0.55, 0.],
            140f64.to_radians() * (1. - s.gear),
        ),
        GearLeft => (
            [-8., -7., -6.],
            [1., -0.46, 0.],
            140f64.to_radians() * (1. - s.gear),
        ),
        GearNose => ([0., 55., -6.], x, 130f64.to_radians() * (1. - s.gear)),
        DoorNose => (
            [-1., 53., -6.],
            y,
            -std::f64::consts::FRAC_PI_2 * (1. - (s.gear * 4.).min(1.)),
        ),
        FlapLeft => ([-13., -8., 3.5], [25., 2., 2.], s.flaps * 0.52),
        FlapRight => ([13., -7., 3.5], [26., -3., -2.], s.flaps * 0.52),
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
        GearLeft | GearRight => 1.,
        GearNose | DoorLeft | DoorRight | DoorNose => s.gear,
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
    if matches!(part, DoorLeft | DoorRight) {
        let closing = 1. - (s.gear * 4.).clamp(0., 1.);
        let original_x = if part == DoorLeft { -5. } else { 6. };
        for p in &mut result.positions {
            if p[2] == -15. {
                p[0] = original_x + (0.5 - original_x) * closing as f32;
                p[2] += 9. * closing as f32;
            }
        }
        if closing != 0. {
            update_normal(face, &mut result);
        }
        return Some(result);
    }
    if matches!(face.address, 0x4ee1 | 0x4f00) {
        animate_nose_brace(face, &mut result, 1. - s.gear.clamp(0., 1.));
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
    if matches!(part, FlapLeft | FlapRight) && s.flaps != 0. {
        let roots = if part == FlapLeft {
            [
                [-13., -8., 4.],
                [-38., -10., 2.],
                [-13., -8., 3.],
                [-38., -10., 1.],
            ]
        } else {
            [
                [13., -7., 4.],
                [39., -10., 2.],
                [13., -7., 3.],
                [39., -10., 1.],
            ]
        };
        for (a, b) in face.positions.iter().zip(&mut result.positions) {
            if roots.contains(a) {
                *b = *a;
            }
        }
        update_normal(face, &mut result);
    }
    if part == GearNose
        && crate::additional_animation::steerable_nose(
            tore_formats::aircraft::AircraftId::F18,
            face.address,
        )
    {
        crate::additional_animation::turn(
            &mut result,
            [0., 55., -6.],
            [0., 0., 1.],
            -s.nosewheel_angle(),
        );
    }
    Some(result)
}

fn animate_nose_brace(source: &Face, result: &mut Face, closing: f64) {
    if closing == 0. {
        return;
    }
    let root = [0., 44., -5.5];
    let wheel_root = [0., 55., -6.];
    let offset = rotate([0., -1., -5.], [1., 0., 0.], 130f64.to_radians() * closing);
    let target = [
        wheel_root[1] + offset[1] - root[1],
        wheel_root[2] + offset[2] - root[2],
    ];
    let original = [10f32, -5.5];
    let length = original.iter().map(|v| v * v).sum::<f32>().sqrt();
    let actual_length = target.iter().map(|v| v * v).sum::<f32>().sqrt();
    let axis = original.map(|v| v / length);
    let actual_axis = target.map(|v| v / actual_length);
    for p in &mut result.positions {
        let delta = [p[1] - root[1], p[2] - root[2]];
        let along = (delta[0] * axis[0] + delta[1] * axis[1]) * actual_length / length;
        let across = -delta[0] * axis[1] + delta[1] * axis[0];
        p[1] = root[1] + along * actual_axis[0] - across * actual_axis[1];
        p[2] = root[2] + along * actual_axis[1] + across * actual_axis[0];
    }
    update_normal(source, result);
}

/// Marks the rear member of each double-sided panel for the smooth renderer.
///
/// SH models close thin panels (the F/A-18 speed brake, fins, doors) with two
/// faces over the same vertices and opposite stored normals, relying on normal
/// culling to show one side. Smooth mode keeps every face for shadows and uses
/// the depth buffer instead, so equal-depth twins fight and the first drawn,
/// often the underside, wins. Hiding the twin that faces the viewer less
/// restores the culled appearance without changing any cast shadow, because
/// both twins cover the same area. `facing` is positive towards the viewer and
/// is only called for faces with a stored normal.
pub fn hidden_twins(faces: &[Face], facing: impl Fn(&Face) -> f32) -> Vec<bool> {
    // Quantize so split panels still pair after float clipping.
    let key = |f: &Face| {
        let mut k: Vec<[i32; 3]> = f
            .positions
            .iter()
            .map(|p| p.map(|v| (v * 64.).round() as i32))
            .collect();
        k.sort_unstable();
        k
    };
    let mut groups = std::collections::HashMap::<_, Vec<usize>>::new();
    for (i, f) in faces.iter().enumerate() {
        if f.normal.is_some() {
            groups.entry(key(f)).or_default().push(i);
        }
    }
    let mut hidden = vec![false; faces.len()];
    for members in groups.values() {
        for (n, &i) in members.iter().enumerate() {
            for &j in &members[n + 1..] {
                let (Some(a), Some(b)) = (faces[i].normal, faces[j].normal) else {
                    continue;
                };
                if dot(a.map(f64::from), b.map(f64::from)) >= 0. {
                    continue;
                }
                // Exactly one twin survives, even edge-on, so shadows stay whole.
                if facing(&faces[j]) > facing(&faces[i]) {
                    hidden[i] = true;
                } else {
                    hidden[j] = true;
                }
            }
        }
    }
    hidden
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
    split_surface(face, pivot, axis, s.rudder * 0.35, distance)
}

pub(crate) fn split_surface(
    face: &Face,
    pivot: [f32; 3],
    axis: [f64; 3],
    angle: f64,
    distance: impl Fn([f32; 3]) -> f32,
) -> Vec<Face> {
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
                let v = rotate(std::array::from_fn(|i| p[i] - pivot[i]), axis, angle);
                *p = std::array::from_fn(|i| pivot[i] + v[i]);
            }
            if let Some(n) = f.normal {
                let n = rotate([n[0], n[2], n[1]], axis, angle);
                f.normal = Some([n[0], n[2], n[1]]);
            }
        }
        result.push(f);
    }
    result
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
pub(crate) fn update_normal(source: &Face, result: &mut Face) {
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

    /// Synthetic double-sided panel behind the brake hinge: a textured
    /// underside and a flat top skin over the same vertices, as SH models do.
    fn brake_twins() -> [Face; 2] {
        let face = |address, positions: Vec<[f32; 3]>, textured: bool, normal| Face {
            fog: tore_formats::shape::FogMode::Enabled,
            address,
            colors: vec![20; positions.len()],
            uv: if textured {
                vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]]
            } else {
                Vec::new()
            },
            positions,
            texture: "SYNTHETIC".into(),
            subtype: if textured { 0xed } else { 0x63 },
            // Stored normals are X/up/forward.
            normal: Some(normal),
        };
        let quad = vec![
            [2., -28., 5.],
            [-2., -28., 5.],
            [-2., -40., 12.],
            [2., -40., 12.],
        ];
        let reversed = quad.iter().rev().copied().collect();
        [
            face(0x5059, quad, true, [0., -12., -7.]),
            face(0x5080, reversed, false, [0., 12., 7.]),
        ]
    }

    fn facing_from(camera: [f32; 3]) -> impl Fn(&Face) -> f32 {
        move |f| {
            let n = f.normal.unwrap();
            let p = f.positions[0];
            [n[0], n[2], n[1]]
                .iter()
                .zip(0..3)
                .map(|(n, i)| n * (camera[i] - p[i]))
                .sum()
        }
    }

    #[test]
    fn deployed_brake_shows_top_skin_above_and_underside_below() {
        let mut s =
            crate::flight::State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        for brake in [1., 0.5, 0.1] {
            s.brake = brake;
            let faces: Vec<_> = brake_twins()
                .iter()
                .map(|f| animate(f, &s).unwrap())
                .collect();
            let above = hidden_twins(&faces, facing_from([0., -34., 40.]));
            let below = hidden_twins(&faces, facing_from([0., -80., -10.]));
            for (hidden, top_visible) in [(above, true), (below, false)] {
                assert_eq!(hidden.iter().filter(|h| **h).count(), 1, "brake {brake}");
                let shown = &faces[hidden.iter().position(|h| !h).unwrap()];
                assert_eq!(shown.uv.is_empty(), top_visible, "brake {brake}");
                assert_eq!(shown.normal.unwrap()[1] > 0., top_visible, "brake {brake}");
            }
        }
    }

    #[test]
    fn twin_hiding_keeps_one_side_and_leaves_single_faces() {
        let [under, top] = brake_twins();
        // Edge-on still keeps exactly one side, so its shadow survives.
        assert_eq!(
            hidden_twins(&[under.clone(), top.clone()], |_| 0.),
            [false, true]
        );
        // A lone rear face and same-facing duplicates are not twins.
        assert_eq!(hidden_twins(std::slice::from_ref(&under), |_| -1.), [false]);
        assert_eq!(
            hidden_twins(&[top.clone(), top.clone()], |_| -1.),
            [false, false]
        );
        let mut moved = top;
        moved.positions[0][2] += 1.;
        assert_eq!(hidden_twins(&[under, moved], |_| -1.), [false, false]);
    }

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
    fn synthetic(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let n = positions.len();
        Face {
            address,
            positions,
            colors: vec![22; n],
            fog: Default::default(),
            uv: vec![[0., 0.]; n],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([1., 0., 0.]),
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    #[test]
    fn main_gear_repair_keeps_joint_rigid_and_separated_for_401_positions() {
        let mut s = state();
        for (a, pivot, x) in [
            (0x4bfa, [9., -7., -6.], 15.),
            (0x4d33, [-8., -7., -6.], -13.),
        ] {
            let f = synthetic(
                a,
                vec![pivot, [x, -6., -12.], [x, -6., -17.], [x, -9., -17.]],
            );
            for i in 0..=400 {
                s.gear = f64::from(i) / 400.;
                let g = animate(&f, &s).unwrap();
                assert_eq!(g.positions[0], pivot);
                assert!(
                    g.positions
                        .iter()
                        .all(|p| if x > 0. { p[0] > 0.45 } else { p[0] < -0.4 })
                );
                for j in 0..4 {
                    for k in j + 1..4 {
                        let dist = |a: [f32; 3], b: [f32; 3]| {
                            a.iter()
                                .zip(b)
                                .map(|(a, b)| (a - b).powi(2))
                                .sum::<f32>()
                                .sqrt()
                        };
                        assert!(
                            (dist(f.positions[j], f.positions[k])
                                - dist(g.positions[j], g.positions[k]))
                            .abs()
                                < 1e-4
                        );
                    }
                }
                if i == 400 {
                    assert_eq!(g.positions, f.positions);
                }
            }
        }
    }
    #[test]
    fn thick_flap_fronts_stay_fixed_and_shared_trailing_points_stay_joined() {
        let mut s = state();
        let roots = [
            [-13., -8., 4.],
            [-38., -10., 2.],
            [-13., -8., 3.],
            [-38., -10., 1.],
        ];
        let top = synthetic(
            0x525b,
            vec![roots[0], roots[1], [-30., -17., 2.], [-18., -17., 3.]],
        );
        let bottom = synthetic(
            0x5282,
            vec![roots[2], roots[3], top.positions[2], top.positions[3]],
        );
        for v in [0., 0.25, 0.5, 0.75, 1.] {
            s.flaps = v;
            let a = animate(&top, &s).unwrap();
            let b = animate(&bottom, &s).unwrap();
            for i in 0..2 {
                assert_eq!(a.positions[i], top.positions[i]);
                assert_eq!(b.positions[i], bottom.positions[i]);
            }
            for i in 2..4 {
                assert_eq!(a.positions[i], b.positions[i]);
            }
        }
    }
    #[test]
    fn telescoping_brace_has_fixed_painted_center_and_no_winding_flip_or_collapse() {
        let f = synthetic(
            0x4ee1,
            vec![
                [0., 44., -5.25],
                [0., 44., -5.75],
                [0., 54., -11.75],
                [0., 54., -10.25],
            ],
        );
        let area = |f: &Face| {
            f.positions
                .iter()
                .zip(f.positions.iter().cycle().skip(1))
                .take(f.positions.len())
                .map(|(a, b)| a[1] * b[2] - b[1] * a[2])
                .sum::<f32>()
        };
        let old = area(&f);
        let mut s = state();
        for i in 1..=401 {
            s.gear = f64::from(i) / 401.;
            let g = animate(&f, &s).unwrap();
            assert!(area(&g) * old > 0.);
            assert!(area(&g).abs() >= old.abs() - 1e-3);
            assert!(((g.positions[0][1] + g.positions[1][1]) * 0.5 - 44.).abs() < 1e-4);
            assert!(((g.positions[0][2] + g.positions[1][2]) * 0.5 + 5.5).abs() < 1e-4);
            if i == 401 {
                assert_eq!(g.positions, f.positions);
            }
        }
    }
    #[test]
    fn paired_door_morph_keeps_source_upper_edges_and_never_crosses_the_center_seam() {
        let f = synthetic(
            0x4b69,
            vec![
                [-5., -8., -15.],
                [-5., -2., -15.],
                [-3., -2., -6.],
                [-3., -8., -6.],
            ],
        );
        let g = synthetic(
            0x4abe,
            vec![
                [6., -8., -15.],
                [6., -2., -15.],
                [4., -2., -6.],
                [4., -8., -6.],
            ],
        );
        let mut s = state();
        for i in 1..=400 {
            s.gear = f64::from(i) / 1600.;
            let a = animate(&f, &s).unwrap();
            let b = animate(&g, &s).unwrap();
            for j in 2..4 {
                assert_eq!(a.positions[j], f.positions[j]);
                assert_eq!(b.positions[j], g.positions[j]);
            }
            assert!(a.positions.iter().all(|p| p[0] <= 0.5));
            assert!(b.positions.iter().all(|p| p[0] >= 0.5));
        }
    }
}
